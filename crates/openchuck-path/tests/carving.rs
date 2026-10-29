// SPDX-License-Identifier: AGPL-3.0-or-later
//! Fine finishing of a carving with a V-bit (issue #382; NFR-04, NFR-06,
//! GC-07): the carving test piece (`openchuck_testkit::parts::CarvedRelief`)
//! finished by axial finishing over the whole part, in a selected region and
//! next to a held region and a tab, and by helical finishing in a region.
//! Each path must be the same at 1, 2 and N threads, and the same, bit for
//! bit, as the path before the speed-ups of issue #382, kept as a hash in
//! `testdata/golden/carving/` (written with `OPENCHUCK_BLESS=1`
//! from the code before them). Native floating point differs between
//! platforms (OI-319), so the golden hashes are compared on Linux only.
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

mod support;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use openchuck_path::params::names;
use openchuck_path::{
    AcceptAll, AnalyticStock, AxialFinishing, CompositeSurface, GeneratedPath, HelicalFinishing,
    MeshSurface, ParameterValue, RegionConstraint, RotaryMapping, Solid, StrategyInput,
    SurfaceSource, generate, generate_in_region,
};
use openchuck_testkit::parts::{CarvedRelief, RadialPart};
use openchuck_types::geometry::{Aabb, Point3, UnitVec3};
use openchuck_types::machine::Clearance;
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::tool::{RevolvedProfile, Tool, ToolShape};
use openchuck_types::units::{Degrees, Millimetres};
use support::{ball, machine, run, tags};

/// The carving test piece with its vertices `spacing` mm apart.
fn relief(spacing: f64) -> TriangleMesh {
    let part = CarvedRelief::STANDARD;
    let rings = (part.length / spacing).round() as u32 + 1;
    let around = (std::f64::consts::TAU * part.radius / spacing).round() as u32;
    part.mesh(rings, around)
}

/// A 60 degree V-bit with a 0.1 mm tip, as the owner carves with.
fn v_bit() -> Tool {
    Tool {
        shape: ToolShape::VBit {
            included_angle: Degrees(60.0),
            tip_diameter: Millimetres(0.1),
        },
        ..ball(3.175)
    }
}

/// The triangles of `mesh` whose corners all satisfy `keep`.
fn pick(mesh: &TriangleMesh, keep: impl Fn(Point3) -> bool) -> TriangleMesh {
    TriangleMesh {
        vertices: mesh.vertices.clone(),
        triangles: mesh
            .triangles
            .iter()
            .copied()
            .filter(|t| t.iter().all(|&i| keep(mesh.vertices[i as usize])))
            .collect(),
    }
}

fn values(stepover: f64) -> BTreeMap<String, ParameterValue> {
    [
        (names::STEPOVER, stepover),
        (names::FEED, 300.0),
        (names::PLUNGE_FEED, 100.0),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), ParameterValue::Number(v)))
    .collect()
}

/// The stock radius for the relief: clear of its highest stroke.
fn stock_radius() -> f64 {
    let part = CarvedRelief::STANDARD;
    part.radius + part.depth + 1.5
}

/// A surface with the V-bit, round stock clear of the relief and the
/// example machine.
struct Job<S> {
    surface: S,
    stock: AnalyticStock,
    tool: RevolvedProfile,
    tags: openchuck_types::ir::PathTags,
}

impl<S: SurfaceSource> Job<S> {
    fn new(surface: S) -> Self {
        Self {
            surface,
            stock: AnalyticStock::Cylinder {
                radius: stock_radius(),
                x_start: -1.0,
                x_end: CarvedRelief::STANDARD.length + 1.0,
            },
            tool: v_bit().profile().unwrap(),
            tags: tags(),
        }
    }

    fn input(&self) -> StrategyInput<'_> {
        let machine = machine();
        StrategyInput {
            surface: &self.surface,
            stock: &self.stock,
            collision: &AcceptAll,
            tool: &self.tool,
            rotary: RotaryMapping::of_profile(&machine),
            clearance: Clearance {
                radius: Millimetres(stock_radius() + 5.0),
                safe_z: Millimetres(stock_radius() + 15.0),
            },
            max_feeds: machine.max_feeds,
            tags: &self.tags,
            entry_safety: None,
            facing: None,
            keep_out: &[],
        }
    }
}

/// Runs `job` at 1, 2 and N threads and checks the path is the same at each
/// (NFR-06) and the same as its golden hash `name` (GC-07).
fn check(name: &str, job: impl Fn() -> GeneratedPath + Sync) {
    let runs = openchuck_testkit::determinism::check(|| format!("{:?}", job().group).into_bytes())
        .unwrap();
    let hash = &runs[0].hash;
    let golden = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/golden/carving")
        .join(format!("{name}.txt"));
    if openchuck_testkit::golden::blessing() {
        std::fs::create_dir_all(golden.parent().unwrap()).unwrap();
        std::fs::write(&golden, format!("{hash}\n")).unwrap();
        return;
    }
    if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        let want = std::fs::read_to_string(&golden).unwrap();
        assert_eq!(
            hash,
            want.trim(),
            "{name}: the path differs from the golden one"
        );
    }
}

#[test]
fn req_gc_07_req_nfr_06_axial_finishing_of_a_carving_is_unchanged_at_any_thread_count() {
    let f = Job::new(MeshSurface::new(&relief(0.25)).unwrap());
    check("axial_whole", || {
        run(|ctx| generate(&AxialFinishing, &f.input(), &values(0.4), ctx)).unwrap()
    });
}

#[test]
fn req_gc_07_req_nfr_06_axial_finishing_of_a_carved_region_is_unchanged_at_any_thread_count() {
    let mesh = relief(0.25);
    let f = Job::new(MeshSurface::new(&mesh).unwrap());
    let middle = MeshSurface::new(&pick(&mesh, |p| p.x >= 11.0 && p.x <= 15.0)).unwrap();
    let region = RegionConstraint {
        region: Some(&middle),
        ..Default::default()
    };
    check("axial_region", || {
        run(|ctx| generate_in_region(&AxialFinishing, &f.input(), &region, &values(1.0), ctx))
            .unwrap()
    });
}

#[test]
fn req_gc_07_req_nfr_06_helical_finishing_of_a_carved_region_is_unchanged_at_any_thread_count() {
    let mesh = relief(0.25);
    let f = Job::new(MeshSurface::new(&mesh).unwrap());
    let middle = MeshSurface::new(&pick(&mesh, |p| p.x >= 12.0 && p.x <= 14.0)).unwrap();
    let region = RegionConstraint {
        region: Some(&middle),
        ..Default::default()
    };
    check("helical_region", || {
        run(|ctx| generate_in_region(&HelicalFinishing, &f.input(), &region, &values(0.5), ctx))
            .unwrap()
    });
}

/// The relief with a held region before it and a tab between, as a setup
/// holds a carving (SET-09, OP-21): the solids are below the carving where
/// the tool works on it, and above its end where the tool comes near them.
#[test]
fn req_gc_07_req_nfr_06_axial_finishing_next_to_a_held_region_and_a_tab_is_unchanged() {
    let model: Arc<dyn SurfaceSource> = Arc::new(MeshSurface::new(&relief(0.25)).unwrap());
    let held = Solid::cuboid(Aabb {
        min: Point3::new(-10.0, -15.0, -15.0),
        max: Point3::new(-4.0, 15.0, 15.0),
    })
    .unwrap();
    let tab = Solid::cylinder(Point3::new(-4.0, 0.0, 0.0), UnitVec3::X, 4.0, 5.0).unwrap();
    let surface = CompositeSurface::new(model, vec![held, tab]);
    let f = Job::new(surface);
    check("axial_held", || {
        run(|ctx| generate(&AxialFinishing, &f.input(), &values(1.5), ctx)).unwrap()
    });
}
