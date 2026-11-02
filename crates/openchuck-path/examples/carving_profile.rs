// SPDX-License-Identifier: AGPL-3.0-or-later
//! Times a finishing strategy on the carving test piece of issue #382
//! (`openchuck_testkit::parts::CarvedRelief::STANDARD`) with a V-bit, through
//! the public API (NFR-04): the whole generation, the passes and the
//! assembly, how many drops the strategy made and the time inside them, the
//! moves made and a hash of the moves, which must be the same at every thread
//! count (NFR-06, GC-07).
//!
//! ```text
//! cargo run --release -p openchuck-path --example carving_profile -- \
//!     [--strategy NAME] [--stepover MM] [--spacing MM] [--threads N] [--runs N] [--tip MM]
//! ```
//!
//! Defaults: axial finishing, a 0.03 mm stepover, the mesh at 0.1 mm, all
//! threads, one run, a 60 degree V-bit 3.175 mm across with a 0.1 mm tip.
#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::collections::BTreeMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use openchuck_path::cutter::{Contact, ToolLine};
use openchuck_path::params::names;
use openchuck_path::{
    AcceptAll, AnalyticStock, MeshSurface, ParameterValue, Registry, RotaryMapping, StrategyInput,
    SurfaceSource, generate,
};
use openchuck_testkit::parts::{CarvedRelief, RadialPart};
use openchuck_types::geometry::{Aabb, Point3, UnitVec3};
use openchuck_types::ir::PathTags;
use openchuck_types::job::{CancelToken, JobContext, Progress, ProgressSink};
use openchuck_types::machine::{Clearance, MachineProfile};
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::tool::{RevolvedProfile, Tool, ToolShape};
use openchuck_types::units::{Degrees, Millimetres};

/// The mesh, counting the drops made on it and the time they took.
#[derive(Debug)]
struct Counted {
    inner: MeshSurface,
    drops: AtomicU64,
    nanos: AtomicU64,
}

impl SurfaceSource for Counted {
    fn bounds(&self) -> Aabb {
        self.inner.bounds()
    }
    fn project(&self, point: Point3) -> Option<openchuck_path::SurfacePoint> {
        self.inner.project(point)
    }
    fn cast_ray(
        &self,
        origin: Point3,
        direction: UnitVec3,
        max_distance: f64,
    ) -> Option<openchuck_path::RayHit> {
        self.inner.cast_ray(origin, direction, max_distance)
    }
    fn tessellation(&self) -> &TriangleMesh {
        self.inner.tessellation()
    }
    fn drop_tool(&self, profile: &RevolvedProfile, line: &ToolLine) -> Option<Contact> {
        let started = Instant::now();
        let contact = self.inner.drop_tool(profile, line);
        let took = u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX);
        self.nanos.fetch_add(took, Ordering::Relaxed);
        self.drops.fetch_add(1, Ordering::Relaxed);
        contact
    }
}

/// The stage timings the library reports.
#[derive(Default)]
struct Stages {
    fitting: AtomicU64,
    assembly: AtomicU64,
}

impl ProgressSink for Stages {
    fn report(&self, _: Progress) {}
    fn wants_timings(&self) -> bool {
        true
    }
    fn timing(&self, stage: &str, elapsed: Duration) {
        let counter = match stage {
            "fitting" => &self.fitting,
            "assembly" => &self.assembly,
            _ => return,
        };
        let nanos = u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX);
        counter.fetch_add(nanos, Ordering::Relaxed);
    }
}

fn seconds(nanos: &AtomicU64) -> f64 {
    Duration::from_nanos(nanos.load(Ordering::Relaxed)).as_secs_f64()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut strategy = "axial_finishing".to_owned();
    let (mut stepover, mut spacing, mut tip): (f64, f64, f64) = (0.03, 0.1, 0.1);
    let (mut threads, mut runs) = (0_usize, 1_usize);
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let value = args.next().ok_or_else(|| format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--strategy" => strategy = value,
            "--stepover" => stepover = value.parse()?,
            "--spacing" => spacing = value.parse()?,
            "--tip" => tip = value.parse()?,
            "--threads" => threads = value.parse()?,
            "--runs" => runs = value.parse()?,
            _ => return Err(format!("unknown flag {flag}").into()),
        }
    }
    // Finer than 0.01 mm the mesh would hold tens of millions of triangles.
    if !(spacing.is_finite() && spacing >= 0.01) {
        return Err(format!("--spacing must be 0.01 mm or more, not {spacing}").into());
    }
    let part = CarvedRelief::STANDARD;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let (rings, around) = (
        (part.length / spacing).round() as u32 + 1,
        (std::f64::consts::TAU * part.radius / spacing).round() as u32,
    );
    let mesh = part.mesh(rings, around);
    let triangles = mesh.triangles.len();
    let tool = Tool {
        shape: ToolShape::VBit {
            included_angle: Degrees(60.0),
            tip_diameter: Millimetres(tip),
        },
        diameter: Millimetres(3.175),
        flute_length: Millimetres(8.0),
        overall_length: Millimetres(40.0),
        shank_diameter: Millimetres(3.175),
        flute_count: 1,
        material: None,
        centre_cutting: None,
    }
    .profile()?;
    let machine = MachineProfile::example_x_rotary();
    let stock_radius = part.radius + part.depth + 1.5;
    let stock = AnalyticStock::Cylinder {
        radius: stock_radius,
        x_start: -1.0,
        x_end: part.length + 1.0,
    };
    let tags = PathTags {
        operation: "carving".to_owned(),
        setup: "setup".to_owned(),
        tool: "v-bit".to_owned(),
    };
    let values: BTreeMap<String, ParameterValue> = [
        (names::STEPOVER, stepover),
        (names::FEED, 300.0),
        (names::PLUNGE_FEED, 100.0),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), ParameterValue::Number(v)))
    .collect();
    let registry = Registry::builtin();
    let chosen = registry.get(&strategy)?;
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()?;
    println!(
        "{strategy}: stepover {stepover} mm, mesh {spacing} mm ({triangles} triangles), \
         V-bit tip {tip} mm, {} threads",
        pool.current_num_threads()
    );
    let surface = Counted {
        inner: MeshSurface::new(&mesh)?,
        drops: AtomicU64::new(0),
        nanos: AtomicU64::new(0),
    };
    for run in 1..=runs {
        surface.drops.store(0, Ordering::Relaxed);
        surface.nanos.store(0, Ordering::Relaxed);
        let stages = Stages::default();
        let cancel = CancelToken::new();
        let context = JobContext::new(&cancel, &stages);
        let input = StrategyInput {
            surface: &surface,
            stock: &stock,
            collision: &AcceptAll,
            tool: &tool,
            rotary: RotaryMapping::of_profile(&machine),
            clearance: Clearance {
                radius: Millimetres(stock_radius + 5.0),
                safe_z: Millimetres(stock_radius + 15.0),
            },
            max_feeds: machine.max_feeds,
            tags: &tags,
            entry_safety: None,
            facing: None,
            keep_out: &[],
            shank: None,
        };
        let started = Instant::now();
        let path = pool.install(|| generate(chosen, &input, &values, &context))?;
        let total = started.elapsed().as_secs_f64();
        let mut hasher = DefaultHasher::new();
        format!("{:?}", path.group).hash(&mut hasher);
        println!(
            "run {run}: {total:.2} s, drops {} ({:.2} s inside, summed over threads), \
             fitting {:.2} s (summed), assembly {:.2} s, {} moves, hash {:016x}",
            surface.drops.load(Ordering::Relaxed),
            seconds(&surface.nanos),
            seconds(&stages.fitting),
            seconds(&stages.assembly),
            path.group.segments.len(),
            hasher.finish(),
        );
    }
    Ok(())
}
