// SPDX-License-Identifier: AGPL-3.0-or-later
//! Times STL pencil clean-up on carried stock at one thread and at more
//! (issue #344, OI-990; NFR-04, NFR-06, GC-07). It mirrors form (a) of
//! decision 0211's full-sequence M3: the owner's blisk as an STL, placed
//! from X 10 on the rotary axis, a 42 mm cylinder of stock, rotary roughing
//! with a 6 mm flat end, rest machining with a 2 mm ball, then pencil
//! clean-up with a 2 mm flat end, each on the stock the ones before it left
//! (simulated with the project's dexel cell, as the application carries it).
//!
//! ```text
//! cargo run --release -p openchuck-app --example stl_pencil_threads -- \
//!     [--stl FILE] [--runs N] [--threads 1,4] [--full]
//! ```
//!
//! The stock after roughing and rest is made once, at all threads, and every
//! pencil run starts from a copy of it; `--full` instead runs the whole
//! sequence in every run, timing each operation. Each run prints the
//! pencil generation time, the drops made and their time summed over
//! threads, the fitting stage and a hash of the path, and the medians come
//! last. The paths must be identical at every thread count, and the run
//! stops if they are not. The STL is `testdata/stl/blisk.stl` by default.

#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::cast_precision_loss,
    clippy::too_many_lines
)]

use std::collections::BTreeMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use openchuck_app::model::TransformStep;
use openchuck_app::project::id::{OperationId, SetupId, ToolId};
use openchuck_app::project::{
    Item, Operation, OrderKey, ParameterValue, Placement, Project, Setup, ToolSnapshot,
};
use openchuck_app::setup::{Millimetres, Stock, builtin};
use openchuck_app::{Application, Command, Threads};
use openchuck_import::ImportOptions;
use openchuck_path::surface::{RayHit, SurfacePoint};
use openchuck_path::{
    AcceptAll, AnalyticStock, Contact, MeshSurface, RotaryMapping, StockModel, StrategyInput,
    SurfaceSource, ToolLine, generate,
};
use openchuck_sim::{DexelStock, Simulator};
use openchuck_types::geometry::{Aabb, Point3, UnitVec3, Vec3};
use openchuck_types::ir::{PathTags, ToolpathGroup};
use openchuck_types::job::{CancelToken, JobContext, NoProgress, Progress, ProgressSink};
use openchuck_types::machine::MachineProfile;
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::model::ModelGeometry;
use openchuck_types::tool::{ControllerMapping, CuttingData, RevolvedProfile, Tool, ToolShape};
use openchuck_types::units::{FeedRate, SpindleSpeed};

type Error = Box<dyn std::error::Error>;

fn repo(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

struct Args {
    stl: PathBuf,
    runs: usize,
    threads: Vec<usize>,
    full: bool,
}

fn args() -> Result<Args, Error> {
    let mut out = Args {
        stl: repo("testdata/stl/blisk.stl"),
        runs: 5,
        threads: vec![
            1,
            std::thread::available_parallelism().map_or(1, usize::from),
        ],
        full: false,
    };
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--stl" => out.stl = PathBuf::from(it.next().ok_or("--stl FILE")?),
            "--runs" => out.runs = it.next().ok_or("--runs N")?.parse()?,
            "--threads" => {
                out.threads = it
                    .next()
                    .ok_or("--threads 1,4")?
                    .split(',')
                    .map(str::parse)
                    .collect::<Result<_, _>>()?;
            }
            "--full" => out.full = true,
            other => return Err(format!("unknown argument {other}").into()),
        }
    }
    Ok(out)
}

/// A surface that counts and times its drops.
struct Timed<'a> {
    inner: &'a dyn SurfaceSource,
    drops: AtomicU64,
    drop_nanos: AtomicU64,
}

impl std::fmt::Debug for Timed<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Timed").finish_non_exhaustive()
    }
}

impl SurfaceSource for Timed<'_> {
    fn bounds(&self) -> Aabb {
        self.inner.bounds()
    }
    fn project(&self, point: Point3) -> Option<SurfacePoint> {
        self.inner.project(point)
    }
    fn distance(&self, point: Point3) -> Option<f64> {
        self.inner.distance(point)
    }
    fn cast_ray(&self, origin: Point3, direction: UnitVec3, max_distance: f64) -> Option<RayHit> {
        self.inner.cast_ray(origin, direction, max_distance)
    }
    fn tessellation(&self) -> &TriangleMesh {
        self.inner.tessellation()
    }
    fn drop_tool(&self, profile: &RevolvedProfile, line: &ToolLine) -> Option<Contact> {
        let started = Instant::now();
        let contact = self.inner.drop_tool(profile, line);
        let took = u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX);
        self.drops.fetch_add(1, Ordering::Relaxed);
        self.drop_nanos.fetch_add(took, Ordering::Relaxed);
        contact
    }
}

/// Every stage the library times, summed.
#[derive(Default)]
struct Stages {
    times: std::sync::Mutex<BTreeMap<String, Duration>>,
}

impl ProgressSink for Stages {
    fn report(&self, _: Progress) {}
    fn wants_timings(&self) -> bool {
        true
    }
    fn timing(&self, stage: &str, elapsed: Duration) {
        if let Ok(mut t) = self.times.lock() {
            *t.entry(stage.to_owned()).or_default() += elapsed;
        }
    }
}

fn n(v: f64) -> ParameterValue {
    ParameterValue::Number(v)
}

fn tool(name: &str, number: u32, shape: ToolShape, diameter: f64, flutes: f64) -> ToolSnapshot {
    let mut cutting_data = BTreeMap::new();
    cutting_data.insert(
        "Softwood".to_owned(),
        CuttingData {
            spindle_speed: SpindleSpeed(12000.0),
            feed_per_tooth: Millimetres(0.02083333333),
            plunge_feed: FeedRate(200.0),
            stepover: Millimetres(0.3),
            stepdown: Millimetres(2.0),
        },
    );
    ToolSnapshot {
        id: ToolId::new(),
        order: OrderKey::default(),
        name: name.into(),
        tool: Tool {
            shape,
            diameter: Millimetres(diameter),
            flute_length: Millimetres(flutes),
            overall_length: Millimetres(50.0),
            shank_diameter: Millimetres(diameter.max(3.175)),
            flute_count: 2,
            material: None,
            centre_cutting: Some(true),
        },
        controller: ControllerMapping {
            number,
            length_offset: None,
            diameter_offset: None,
        },
        cutting_data,
    }
}

/// An operation: its label, strategy, tool and parameters.
type OpSpec = (
    &'static str,
    &'static str,
    ToolSnapshot,
    Vec<(&'static str, ParameterValue)>,
);

/// Decision 0211's roughing, rest and pencil operations, as in
/// `step_accuracy` without `--quick`.
fn operations() -> Vec<OpSpec> {
    let flat6 = tool("Z1 O-flute metal 6x17", 116, ToolShape::FlatEnd, 6.0, 17.0);
    let ball2 = tool("Z1 ball 2x12", 205, ToolShape::BallEnd, 2.0, 12.0);
    let flat2 = tool("Z1 O-flute metal 2x8", 102, ToolShape::FlatEnd, 2.0, 8.0);
    vec![
        (
            "roughing",
            "rotary_roughing",
            flat6,
            vec![
                ("entry", ParameterValue::Text("plunge".into())),
                ("feed", n(200.0)),
                ("plunge_feed", n(500.0)),
                ("stepdown", n(1.0)),
                ("stepover", n(2.0)),
                ("stock_to_leave", n(0.3)),
            ],
        ),
        (
            "rest",
            "rest_machining",
            ball2,
            vec![("feed", n(500.0)), ("stepover", n(0.3))],
        ),
        ("pencil", "pencil_finishing", flat2, Vec::new()),
    ]
}

fn stl(path: &Path) -> Result<(TriangleMesh, Vec3, Aabb), Error> {
    let bytes = std::fs::read(path)?;
    let cancel = CancelToken::new();
    let imported = openchuck_import::import_bytes(
        "blisk.stl",
        &bytes,
        &ImportOptions::default(),
        &JobContext::new(&cancel, &NoProgress),
    )?;
    let ModelGeometry::Mesh(mut mesh) = imported.geometry else {
        return Err("not a mesh".into());
    };
    let b = openchuck_geom::summary(&mesh).bounds.ok_or("empty STL")?;
    // On the rotary axis, from X 10, as decision 0211 places the blisk.
    let shift = Vec3::new(
        10.0 - b.min.x,
        -(b.min.y + b.max.y) / 2.0,
        -(b.min.z + b.max.z) / 2.0,
    );
    for v in &mut mesh.vertices {
        *v = *v + shift;
    }
    let bounds = openchuck_geom::summary(&mesh).bounds.ok_or("empty STL")?;
    Ok((mesh, shift, bounds))
}

/// The project, for the operations' parameter values and the simulation
/// cell.
fn project(
    dir: &Path,
    model: &Path,
    shift: Vec3,
    bounds: &Aabb,
) -> Result<(Application, Vec<OperationId>), Error> {
    if dir.exists() {
        std::fs::remove_dir_all(dir)?;
    }
    let mut app = Application::create(dir, "blisk", Threads::All)?;
    std::fs::create_dir_all(dir.join("models"))?;
    let file = dir.join("models").join("blisk.stl");
    std::fs::copy(model, &file)?;
    let placement = [TransformStep::Translate {
        x: shift.x,
        y: shift.y,
        z: shift.z,
    }];
    let (model, _) = app.add_model_with(&file, None, &placement)?;
    app.execute(Command::SetMachine(Some(Box::new(
        builtin("makera-z1").ok_or("no Z1")??,
    ))))?;
    let setup_id = SetupId::new();
    let mut setup = Setup::new(setup_id, "Setup 1");
    setup.part = Some(model);
    setup.material = Some("Softwood".into());
    let radius = [bounds.min.y, bounds.max.y, bounds.min.z, bounds.max.z]
        .into_iter()
        .map(f64::abs)
        .fold(0.0, f64::max);
    setup.stock = Some(Stock::Cylinder {
        diameter: Millimetres(2.0 * radius + 2.0),
        length: Millimetres(bounds.max.x - bounds.min.x + 2.0),
        x_start: Millimetres(bounds.min.x - 1.0),
    });
    let mut items = vec![Item::Setup(setup)];
    let mut ids = Vec::new();
    for (label, strategy, snapshot, parameters) in operations() {
        let tool_id = snapshot.id;
        items.push(Item::Tool(snapshot));
        let id = OperationId::new();
        ids.push(id);
        items.push(Item::Operation(Operation {
            enabled: true,
            id,
            order: OrderKey::default(),
            name: label.to_owned(),
            setup: setup_id,
            strategy: strategy.into(),
            tool: Some(tool_id),
            selections: Vec::new(),
            boundary: None,
            avoid: Vec::new(),
            check: Vec::new(),
            restriction_options: Default::default(),
            spindle_speed: None,
            material: None,
            parameters: parameters
                .iter()
                .map(|(k, v)| ((*k).to_owned(), v.clone()))
                .collect(),
        }));
    }
    for item in items {
        app.execute(Command::Add {
            item,
            placement: Placement::End,
        })?;
    }
    Ok((app, ids))
}

struct Job<'a> {
    project: &'a Project,
    registry: &'a openchuck_path::Registry,
    machine: &'a MachineProfile,
    rotary: RotaryMapping,
    bounds: Aabb,
    surface: &'a MeshSurface,
}

struct Timing {
    generate: Duration,
    drops: u64,
    drop_time: Duration,
    stages: BTreeMap<String, Duration>,
    simulate: Duration,
    hash: u64,
}

impl Job<'_> {
    /// Operation `id` on `stock`; simulated onto `simulator` when given.
    fn operation(
        &self,
        id: OperationId,
        stock: &dyn StockModel,
    ) -> Result<(ToolpathGroup, RevolvedProfile, Timing), Error> {
        let operation = self.project.operation(id).ok_or("no operation")?;
        let setup = self.project.setups().first().ok_or("no setup")?;
        let snapshot = self
            .project
            .tools()
            .iter()
            .find(|t| Some(t.id) == operation.tool)
            .ok_or("no tool")?;
        let profile = snapshot.tool.profile()?;
        let values =
            openchuck_app::operation::parameter_values(self.project, operation, self.registry)?;
        let strategy = self.registry.get(&operation.strategy)?;
        let timed = Timed {
            inner: self.surface,
            drops: AtomicU64::new(0),
            drop_nanos: AtomicU64::new(0),
        };
        let tags = PathTags {
            operation: operation.id.to_string(),
            setup: setup.id.to_string(),
            tool: snapshot.id.to_string(),
        };
        let input = StrategyInput {
            surface: &timed,
            stock,
            collision: &AcceptAll,
            tool: &profile,
            rotary: self.rotary,
            clearance: setup.clearance.unwrap_or(self.machine.clearance),
            max_feeds: self.machine.max_feeds,
            tags: &tags,
            keep_out: &[],
            entry_safety: None,
            facing: None,
        };
        let cancel = CancelToken::new();
        let stages = Stages::default();
        let context = JobContext::new(&cancel, &stages);
        let started = Instant::now();
        let path = generate(strategy, &input, &values.values, &context)?;
        let generate_time = started.elapsed();
        // The moves only: the tags hold this run's random identifiers.
        let mut h = DefaultHasher::new();
        format!(
            "{:?}{:?}{:?}",
            path.group.start, path.group.segments, path.group.rotary_reset_after
        )
        .hash(&mut h);
        let timing = Timing {
            generate: generate_time,
            drops: timed.drops.load(Ordering::Relaxed),
            drop_time: Duration::from_nanos(timed.drop_nanos.load(Ordering::Relaxed)),
            stages: stages.times.into_inner().unwrap_or_default(),
            simulate: Duration::ZERO,
            hash: h.finish(),
        };
        Ok((path.group, profile, timing))
    }

    fn simulator(&self) -> Result<Simulator, Error> {
        let setup = self.project.setups().first().ok_or("no setup")?;
        let stock_def = setup.stock.as_ref().ok_or("no stock")?;
        let cell = self.project.simulation().cell.0;
        Ok(Simulator::without_checkpoints(
            DexelStock::from_stock(stock_def, Some(self.bounds), cell)?,
            self.machine,
        ))
    }

    /// Roughing and rest, each simulated: the stock pencil works on, and
    /// their timings.
    fn carried(&self, ids: &[OperationId]) -> Result<(DexelStock, Vec<Timing>), Error> {
        let setup = self.project.setups().first().ok_or("no setup")?;
        let analytic =
            AnalyticStock::from_stock(setup.stock.as_ref().ok_or("no stock")?, Some(self.bounds))?;
        let mut simulator = self.simulator()?;
        let mut timings = Vec::new();
        for (i, id) in ids.iter().enumerate() {
            let carried: &dyn StockModel = if i == 0 { &analytic } else { simulator.stock() };
            let (group, profile, mut timing) = self.operation(*id, carried)?;
            let cancel = CancelToken::new();
            let context = JobContext::new(&cancel, &NoProgress);
            let started = Instant::now();
            simulator.run_group(&group, &profile, &context)?;
            timing.simulate = started.elapsed();
            timings.push(timing);
        }
        Ok((simulator.stock().clone(), timings))
    }
}

/// Minor page faults and system CPU seconds of this process so far, from
/// `/proc/self/stat` (Linux only; zeros elsewhere).
fn faults_and_sys() -> (u64, f64) {
    let Ok(stat) = std::fs::read_to_string("/proc/self/stat") else {
        return (0, 0.0);
    };
    // The fields after the command name, which is in brackets.
    let Some(rest) = stat.rsplit_once(')').map(|r| r.1) else {
        return (0, 0.0);
    };
    let f: Vec<&str> = rest.split_whitespace().collect();
    // Fields 10 (minflt) and 15 (stime) of proc(5), counted from 1 at the
    // pid; here from 0 at the state, field 3.
    let minflt = f.get(7).and_then(|v| v.parse().ok()).unwrap_or(0);
    let stime: f64 = f.get(12).and_then(|v| v.parse().ok()).unwrap_or(0.0);
    (minflt, stime / 100.0)
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    v.get(v.len() / 2).copied().unwrap_or(f64::NAN)
}

fn main() -> Result<(), Error> {
    let args = args()?;
    let machine = builtin("makera-z1").ok_or("no Z1")??;
    let rotary = RotaryMapping::of_profile(&machine);
    let (mesh, shift, bounds) = stl(&args.stl)?;
    let bytes = std::fs::read(&args.stl)?;
    println!("# stl_pencil_threads (issue #344, OI-990)");
    println!(
        "stl: {} ({} triangles, {} bytes), moved by {shift:?}",
        args.stl.display(),
        mesh.triangles.len(),
        bytes.len()
    );
    println!(
        "placed: X {:.3} to {:.3}, Y {:.3} to {:.3}, Z {:.3} to {:.3}",
        bounds.min.x, bounds.max.x, bounds.min.y, bounds.max.y, bounds.min.z, bounds.max.z
    );
    println!(
        "cores: {}",
        std::thread::available_parallelism().map_or(1, usize::from)
    );
    println!(
        "build: {}",
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        }
    );
    let dir = std::env::temp_dir().join(format!("stl-pencil-threads-{}", std::process::id()));
    let (app, ids) = project(&dir, &args.stl, shift, &bounds)?;
    let surface = MeshSurface::new(&mesh)?;
    let job = Job {
        project: app.project(),
        registry: app.strategies(),
        machine: &machine,
        rotary,
        bounds,
        surface: &surface,
    };
    let (prior, pencil) = ids.split_at(2);
    let pencil = pencil.first().copied().ok_or("no pencil")?;
    let pool = |threads: usize| rayon::ThreadPoolBuilder::new().num_threads(threads).build();

    let carried = if args.full {
        None
    } else {
        let started = Instant::now();
        let (stock, _) = pool(0)?.install(|| job.carried(prior).map_err(|e| e.to_string()))?;
        println!(
            "carried stock (roughing, rest, simulated) made once in {:.1} s",
            started.elapsed().as_secs_f64()
        );
        Some(stock)
    };

    println!(
        "run | threads | operation | generate s | drops | drop s (summed) | fitting s | simulate s | path hash"
    );
    let mut reference = None;
    let mut summary = Vec::new();
    for &threads in &args.threads {
        let mut times = Vec::new();
        for run in 1..=args.runs {
            let (stock, before) = match &carried {
                Some(stock) => (stock.clone(), Vec::new()),
                None => pool(threads)?.install(|| job.carried(prior).map_err(|e| e.to_string()))?,
            };
            for (label, t) in ["roughing", "rest"].iter().zip(&before) {
                println!(
                    "run {run} | {threads} | {label} | {:.6} | {} | {:.6} | {:.6} | {:.6} | {:016x}",
                    t.generate.as_secs_f64(),
                    t.drops,
                    t.drop_time.as_secs_f64(),
                    t.stages.get("fitting").map_or(0.0, Duration::as_secs_f64),
                    t.simulate.as_secs_f64(),
                    t.hash
                );
            }
            let threads_pool = pool(threads)?;
            let (faults, sys) = faults_and_sys();
            let (_, _, t) = threads_pool
                .install(|| job.operation(pencil, &stock).map_err(|e| e.to_string()))?;
            let (faults_after, sys_after) = faults_and_sys();
            println!(
                "    minor page faults {}, system CPU {:.2} s",
                faults_after - faults,
                sys_after - sys
            );
            println!(
                "run {run} | {threads} | pencil | {:.6} | {} | {:.6} | {:.6} | - | {:016x}",
                t.generate.as_secs_f64(),
                t.drops,
                t.drop_time.as_secs_f64(),
                t.stages.get("fitting").map_or(0.0, Duration::as_secs_f64),
                t.hash
            );
            for (stage, d) in &t.stages {
                if stage != "fitting" {
                    println!("    stage {stage}: {:.6} s", d.as_secs_f64());
                }
            }
            match reference {
                None => reference = Some(t.hash),
                Some(h) if h != t.hash => {
                    return Err(format!("pencil path differs at {threads} threads").into());
                }
                Some(_) => {}
            }
            times.push((t.generate.as_secs_f64(), t.drop_time.as_secs_f64()));
        }
        let generate = median(times.iter().map(|t| t.0).collect());
        let drops = median(times.iter().map(|t| t.1).collect());
        summary.push((threads, generate, drops));
    }
    println!("\nthreads | pencil generate median s | drop s median (summed)");
    for (threads, generate, drops) in &summary {
        println!("{threads} | {generate:.6} | {drops:.6}");
    }
    if let (Some(one), Some(last)) = (summary.first(), summary.last()) {
        println!(
            "speed-up {} to {} threads: {:.4}x",
            one.0,
            last.0,
            one.1 / last.1
        );
    }
    println!(
        "pencil path identical at every thread count: {:016x}",
        reference.unwrap_or(0)
    );
    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}
