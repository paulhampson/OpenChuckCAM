// SPDX-License-Identifier: AGPL-3.0-or-later
//! Measures what the exact STEP surface would gain over its tessellation,
//! strategy by strategy (lane STEPALL-M; decision record 0209 item 4.1,
//! decision record 0211; issue #257). It changes nothing: it reads
//! `testdata/step/valid/blisk.step` and the STEP corpus, makes the owner's
//! operations through the pathing library and the application, and prints:
//!
//! - **M1**, the tessellation's distance from its exact faces at each
//!   deflection, by face kind, measured on the triangles (corners, edge
//!   middles and a grid of each), never taken from the kernel's setting
//!   (decision 0003, OI-65); the facet normals' error (OI-76); triangles and
//!   import time.
//! - **M2**, each strategy's cut against the exact faces: the deepest cut
//!   into them and the largest gap above them beyond the stock to leave, at
//!   every fed pose and at the middle and quarter points of every move
//!   (decisions 0115 and 0133), by the bounded checker
//!   (`openchuck_testkit::contact`, issue #346): each station's clearance is
//!   an interval proven to hold the exact value, against the exact faces
//!   and the tessellation the tool was dropped on. The forms compared are
//!   (a) the owner's `blisk.stl`, when its path is given, (b) the STEP part
//!   dropping on the import's tessellation, (c) the STEP part dropping on a
//!   finer tessellation, and (d) the STEP part with exact contacts for the
//!   strategies that take them (lane STEPALL-X2, decision 0277): what the
//!   application makes today. M2 fails when any station's
//!   interval cannot be closed below the tolerance; `--raw DIR` keeps one
//!   file of raw observations per form and operation, with provenance.
//! - **M3**, time: each form's generation, drops, fitting, assembly and stock
//!   simulation, plus complete job wall time. At 1 thread and at all threads,
//!   medians require at least five successful runs. For ball-end operations,
//!   an optional probe estimates exact-drop cost by refining sampled drops
//!   against the exact faces.
//! - **M4**, determinism: each form's toolpath at 1, 2 and all threads, and
//!   each form's posted G-code, with (a) and (d) checked through the application, byte
//!   for byte (GC-07, NFR-06); and the library's path of form (d) against
//!   the application's, so the harness measures what the application makes.
//!
//! ```text
//! cargo run --release -p openchuck-app --features kernel-tests --example step_accuracy -- \
//!     [--stl OWNER_BLISK_STL] [--runs N] [--skip-probe] [--only m1,m2,m3,m4] [--ops roughing,helical,...] \
//!     [--quick] [--parts cylinder,blisk,...] [--deflections 0.01,0.001] [--angular RAD] [--fine MM] [--forms a,b,c,d] [--threads 1,all] [--sim-tilt DEG] [--sim-step DEG] [--part NAME] [--keep DIR]
//! ```
//!
//! The owner's `blisk.stl` is `docs/trial-feedback-projects/blisk.stl` on
//! branch `claude/upbeat-davinci-jk46yd`; it is not in `testdata/` (OI-832
//! covers `blisk.step` only), so form (a) runs only when its path is given.
//! `--keep DIR` requires a fresh destination; existing paths, including the
//! derived STL project path, are refused without replacement.
//! `--quick` coarsens every stepover for a smoke run; its numbers are not the
//! record's.

#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::too_many_lines
)]

mod bounded;
mod checks;
mod measure;
mod posting;
mod timing;
mod workspace;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use openchuck_app::model::{Axis, Degrees, TransformStep};
use openchuck_app::project::id::{OperationId, SetupId, ToolId};
use openchuck_app::project::{
    Item, Operation, OrderKey, ParameterValue, Placement, Project, Setup, ToolSnapshot,
};
use openchuck_app::setup::{Millimetres, Stock, builtin};
use openchuck_app::{Application, Command, PostSelection, Threads};
use openchuck_geom::transform::{brep_placement, transform_brep};
use openchuck_import::step::StepImporter;
use openchuck_import::{ImportOptions, ModelImporter};
use openchuck_path::surface::{FaceRef, RayHit, SurfacePoint};
use openchuck_path::{
    AcceptAll, AnalyticStock, BrepSurface, Contact, Hole, MeshSurface, PencilFinishing,
    RotaryMapping, StockModel, Strategy, StrategyInput, SurfaceSource, ToolLine, generate,
};
use openchuck_sim::{DexelStock, Simulator};
use openchuck_types::brep::{BrepModel, TessellationSettings};
use openchuck_types::geometry::{Aabb, Point3, UnitVec3, Vec3};
use openchuck_types::ir::{MoveKind, PathTags, ToolpathGroup};
use openchuck_types::job::{CancelToken, JobContext, NoProgress};
use openchuck_types::machine::MachineProfile;
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::model::ModelGeometry;
use openchuck_types::tool::{ControllerMapping, CuttingData, RevolvedProfile, Tool, ToolShape};
use openchuck_types::units::{FeedRate, SpindleSpeed};

use measure::Station;
use openchuck_testkit::contact::{ConvexTool, Geometry};

type Error = Box<dyn std::error::Error>;

/// The deflections of M1, mm (decision 0209 item 4.1).
const DEFLECTIONS: [f64; 4] = [0.01, 0.005, 0.0025, 0.001];

/// The tessellation's share of the 0.01 mm toolpath budget (verification
/// plan 4.1).
const SHARE: f64 = 0.005;

fn repo(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

struct Args {
    stl: Option<PathBuf>,
    runs: usize,
    skip_probe: bool,
    only: Vec<String>,
    ops: Vec<String>,
    quick: bool,
    parts: Vec<String>,
    deflections: Vec<f64>,
    fine: Option<f64>,
    angular: f64,
    forms: Vec<char>,
    threads: Vec<Threads>,
    sim_tilt: f64,
    sim_step: f64,
    part: String,
    keep: Option<PathBuf>,
    raw: Option<PathBuf>,
}

fn args() -> Result<Args, Error> {
    let mut out = Args {
        stl: None,
        runs: 5,
        skip_probe: false,
        only: vec!["m1".into(), "m2".into(), "m3".into(), "m4".into()],
        ops: Vec::new(),
        quick: false,
        parts: [
            "cylinder",
            "cone",
            "sphere",
            "torus",
            "bspline_face",
            "filleted_box",
            "revolved_spline",
            "blisk",
        ]
        .map(str::to_owned)
        .to_vec(),
        deflections: DEFLECTIONS.to_vec(),
        fine: None,
        angular: TessellationSettings::default().angular,
        forms: vec!['a', 'b', 'c', 'd'],
        threads: vec![Threads::Exactly(1), Threads::All],
        sim_tilt: 10.0,
        sim_step: 2.0,
        part: "blisk".to_owned(),
        keep: None,
        raw: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--stl" => out.stl = Some(PathBuf::from(it.next().ok_or("--stl FILE")?)),
            "--runs" => out.runs = it.next().ok_or("--runs N")?.parse()?,
            "--only" => {
                out.only = it
                    .next()
                    .ok_or("--only LIST")?
                    .split(',')
                    .map(str::to_owned)
                    .collect();
            }
            "--ops" => {
                out.ops = it
                    .next()
                    .ok_or("--ops LIST")?
                    .split(',')
                    .map(str::to_owned)
                    .collect();
            }
            "--quick" => out.quick = true,
            "--skip-probe" => out.skip_probe = true,
            "--parts" => {
                out.parts = it
                    .next()
                    .ok_or("--parts LIST")?
                    .split(',')
                    .map(str::to_owned)
                    .collect();
            }
            "--deflections" => {
                out.deflections = it
                    .next()
                    .ok_or("--deflections LIST")?
                    .split(',')
                    .map(str::parse)
                    .collect::<Result<_, _>>()?;
            }
            "--forms" => {
                out.forms = it
                    .next()
                    .ok_or("--forms LIST")?
                    .split(',')
                    .filter_map(|f| f.chars().next())
                    .collect();
            }
            "--threads" => {
                out.threads = it
                    .next()
                    .ok_or("--threads LIST")?
                    .split(',')
                    .map(|t| match t {
                        "all" => Ok(Threads::All),
                        n => n.parse().map(Threads::Exactly),
                    })
                    .collect::<Result<_, _>>()?;
            }
            "--keep" => out.keep = Some(PathBuf::from(it.next().ok_or("--keep DIR")?)),
            "--raw" => out.raw = Some(PathBuf::from(it.next().ok_or("--raw DIR")?)),
            "--part" => out.part = it.next().ok_or("--part NAME")?,
            "--sim-tilt" => out.sim_tilt = it.next().ok_or("--sim-tilt DEG")?.parse()?,
            "--sim-step" => out.sim_step = it.next().ok_or("--sim-step DEG")?.parse()?,
            "--angular" => out.angular = it.next().ok_or("--angular RAD")?.parse()?,
            "--fine" => out.fine = Some(it.next().ok_or("--fine MM")?.parse()?),
            other => return Err(format!("unknown argument {other}").into()),
        }
    }
    if out.runs == 0 {
        return Err("--runs must be positive".into());
    }
    Ok(out)
}

fn main() -> Result<(), Error> {
    let args = args()?;
    let cores = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
    println!("# step_accuracy (decision 0211)");
    println!("cpu:   {}", cpu_model());
    println!("cores: {cores}");
    println!(
        "build: {}",
        if cfg!(debug_assertions) {
            "debug (times are not the record's)"
        } else {
            "release"
        }
    );
    let wants = |m: &str| args.only.iter().any(|o| o == m);

    // M1: the corpus, then the blisk.
    let mut fine = None;
    if wants("m1") {
        println!("\n## M1: tessellation against the exact faces");
        println!(
            "part | deflection mm | kind | faces | triangles | measured | samples | max mm | p99 mm | to trimmed face mm | closed form mm | normal deg | closest us | import s"
        );
        println!("angular deflection: {} rad", args.angular);
        for part in &args.parts {
            for &d in &args.deflections {
                let file = repo(&format!("testdata/step/valid/{part}.step"));
                let (model, took) = import_at(&file, d, args.angular)?;
                let worst = print_m1(part, d, &model, took);
                if part == "blisk" && worst <= SHARE && fine.is_none() {
                    fine = Some(d);
                }
            }
        }
        println!(
            "finest needed: {}",
            fine.map_or(
                "none of the tested settings is within the 0.005 mm share".to_owned(),
                |d| format!(
                    "{d} mm is the coarsest deflection within the 0.005 mm share on every face"
                )
            )
        );
    }
    // Form (c): the deflection given, else the coarsest M1 found within the
    // share, else the finest of M1's.
    let fine = args.fine.or(fine).unwrap_or(0.001);

    if !(wants("m2") || wants("m3") || wants("m4")) {
        return Ok(());
    }
    // M4 checks the application against the library's form (d): without it
    // the equivalence checks would be skipped unnoticed.
    if wants("m4") && !args.forms.contains(&APP_FORM) {
        return Err(format!(
            "M4 compares the application with form ({APP_FORM}); add it to --forms"
        )
        .into());
    }
    let machine = builtin("makera-z1").ok_or("no Z1")??;
    let rotary = RotaryMapping::of_profile(&machine);
    let part_path = repo(&format!("testdata/step/valid/{}.step", args.part));
    println!("\npart: {}", part_path.display());
    let (model_b, _) = import(&part_path, TessellationSettings::default().linear)?;
    let placement = placement(&model_b)?;
    let placed_b = place(&model_b, &placement)?;
    let (model_c, _) = import_at(&part_path, fine, args.angular)?;
    let placed_c = place(&model_c, &placement)?;
    let bounds = brep_bounds(&placed_b);
    println!(
        "part placed: X {:.3} to {:.3}, Y {:.3} to {:.3}, Z {:.3} to {:.3}",
        bounds.min.x, bounds.max.x, bounds.min.y, bounds.max.y, bounds.min.z, bounds.max.z
    );
    let stl = match &args.stl {
        Some(path) => Some(owner_stl(path, &bounds)?),
        None => None,
    };
    if let Some((mesh, shift)) = &stl {
        println!("owner's blisk.stl moved by {shift:?} to sit on the STEP part");
        let exact = BrepSurface::new(&placed_b)?;
        let mut worst: f64 = 0.0;
        for v in &mesh.vertices {
            if let Some(d) = exact.distance(*v) {
                worst = worst.max(d);
            }
        }
        println!(
            "owner's blisk.stl: {} triangles, vertices up to {worst:.4} mm from the exact faces",
            mesh.triangles.len()
        );
    }

    // The project: the owner's tools and operations, on the STEP part.
    let ops = operations(&args);
    // Existing destinations are refused, including the derived STL project.
    // With `--keep` the project stays, saved, for `operation_times` (lane
    // BUG-TABDEV's example) to time through the application.
    let dir = args.keep.clone().unwrap_or_else(|| {
        std::env::temp_dir().join(format!("openchuck-step-accuracy-{}", std::process::id()))
    });
    let (mut app, ids) = project(&dir, &part_path, &placement, &bounds, &ops)?;

    let mut forms: Vec<(&str, Form<'_>)> = vec![
        (
            "b (STEP, import tessellation 0.01 mm)",
            Form::Brep(BrepSurface::new(&placed_b)?, None, &placed_b),
        ),
        (
            "c (STEP, finer tessellation)",
            Form::Brep(BrepSurface::new(&placed_c)?, None, &placed_c),
        ),
        (
            "d (STEP, exact contacts where the strategy takes them)",
            Form::Brep(
                BrepSurface::new(&placed_b)?,
                Some(Box::new(
                    BrepSurface::new(&placed_b)?.with_exact_drops(true),
                )),
                &placed_b,
            ),
        ),
    ];
    if let Some((mesh, _)) = &stl {
        forms.insert(
            0,
            ("a (owner's blisk.stl)", Form::Mesh(MeshSurface::new(mesh)?)),
        );
    }
    println!("form c deflection: {fine} mm and {} rad", args.angular);

    let exact = if wants("m2") {
        let g = Geometry::of_faces(&placed_b)?;
        println!(
            "M2 bounded checker: {} faces in {} starting pieces; tolerance {} mm, margin {} mm, budget {} pieces a station",
            g.faces(),
            g.leaves(),
            bounded::TOLERANCE,
            bounded::MARGIN,
            bounded::BUDGET
        );
        Some(g)
    } else {
        None
    };
    if let Some(raw) = &args.raw {
        workspace::create_destination(raw)?;
    }
    let mut proven_error: Option<f64> = None;
    let job = Job {
        project: app.project(),
        registry: app.strategies(),
        ids: &ids,
        machine: &machine,
        rotary,
        part_bounds: bounds,
    };

    let post_job = job.post_job()?;
    let mut library_posts = BTreeMap::new();
    let mut library_b = None;
    // The operations each form's strategies refused, by the form's letter.
    let mut refused: BTreeMap<char, Vec<bool>> = BTreeMap::new();
    for (name, form) in forms
        .iter()
        .filter(|(name, _)| name.chars().next().is_some_and(|c| args.forms.contains(&c)))
    {
        let tessellation = if wants("m2") {
            Some(Geometry::of_mesh(form.surface().tessellation())?)
        } else {
            None
        };
        println!(
            "\n## form {name}: {} triangles",
            form.surface().tessellation().triangles.len()
        );
        if wants("m2") || wants("m3") {
            let run = job.run(form, Threads::All)?;
            if name.starts_with(APP_FORM) {
                library_b = Some(
                    run.groups
                        .iter()
                        .filter_map(|g| g.clone().ok())
                        .collect::<Vec<_>>(),
                );
            }
            if let Some(letter) = name.chars().next() {
                refused.insert(letter, run.groups.iter().map(Result::is_err).collect());
            }
            if wants("m2") {
                println!("### M2: the cut against the exact faces");
                println!(
                    "operation | stock mm | stations | deepest cut mm | proven at most | (at poses) | largest gap mm | drop error: deepest in | furthest out | proven at most | contact stations | widest interval mm | cut within {TOOLPATH_TOLERANCE} mm | gap within {TOOLPATH_TOLERANCE} mm"
                );
                for (op, group) in ops.iter().zip(&run.groups) {
                    let group = match group {
                        Ok(g) => g,
                        Err(e) => {
                            println!("{} | {} | refused: {e}", op.label, op.leave);
                            continue;
                        }
                    };
                    let observations = m2(
                        &job,
                        op,
                        group,
                        exact.as_ref().ok_or("missing M2 faces")?,
                        tessellation.as_ref().ok_or("missing M2 tessellation")?,
                    )?;
                    let m = bounded::summarise(&observations, op.leave);
                    println!(
                        "{} | {} | {} | {:.4} | {:.4} | {:.4} | {:.4} | {:.4} | {:.4} | {:.4} | {} | {:.6} | {} | {}",
                        op.label,
                        op.leave,
                        m.stations,
                        m.deepest,
                        m.deepest_bound,
                        m.deepest_at_poses,
                        m.gap,
                        m.error_in,
                        m.error_out,
                        m.drop_bound,
                        m.contacts,
                        m.proven_error,
                        checks::verdict(m.deepest_bound, TOOLPATH_TOLERANCE),
                        checks::verdict(m.gap, TOOLPATH_TOLERANCE)
                    );
                    proven_error = Some(proven_error.unwrap_or(0.0).max(m.proven_error));
                    if let Some(raw) = &args.raw {
                        let letter = name.chars().next().unwrap_or('x');
                        let header = provenance(&args, name, fine, op, &part_path);
                        let file = raw.join(format!("m2-{letter}-{}.tsv", op.label));
                        std::fs::write(&file, bounded::raw(&header, &observations))?;
                    }
                }
            }
            if wants("m3") {
                println!("### M3: time (median of {} runs)", args.runs);
                println!(
                    "operation | threads | generate s | drops | drop s (summed over threads) | exact queries | simulate s | fitting s (inclusive summed scopes) | assembly s | status"
                );
                for &threads in &args.threads {
                    // Each run is printed as it ends, so a run cut short
                    // loses only itself; the medians follow the last run.
                    let mut runs: Vec<Run> = Vec::new();
                    for k in 0..args.runs {
                        let run = job.run(form, threads)?;
                        for ((op, t), result) in ops.iter().zip(&run.timings).zip(&run.groups) {
                            println!(
                                "run {} | {} | {} | {:.6} | {} | {:.6} | {} | {:.6} | {:.6} | {:.6} | {}",
                                k + 1,
                                op.label,
                                threads_name(threads, cores),
                                t.generate.as_secs_f64(),
                                t.drops,
                                t.drop_time.as_secs_f64(),
                                t.exact_queries,
                                t.simulate.as_secs_f64(),
                                t.fitting.as_secs_f64(),
                                t.assembly.as_secs_f64(),
                                if result.is_ok() {
                                    "success"
                                } else {
                                    "REFUSED (time to refusal)"
                                },
                            );
                        }
                        println!(
                            "run {} | job wall time | {} | {:.6} s | {}",
                            k + 1,
                            threads_name(threads, cores),
                            run.total.as_secs_f64(),
                            if run.groups.iter().all(Result::is_ok) {
                                "success"
                            } else {
                                "REFUSED sequence"
                            }
                        );
                        runs.push(run);
                    }
                    for (i, op) in ops.iter().enumerate() {
                        let pick = |f: &dyn Fn(&Timing) -> f64| {
                            timing::successful_median(
                                &runs
                                    .iter()
                                    .map(|r| {
                                        r.groups
                                            .get(i)
                                            .filter(|g| g.is_ok())
                                            .and_then(|_| r.timings.get(i))
                                            .map(f)
                                    })
                                    .collect::<Vec<_>>(),
                            )
                            .map_or_else(|| "unavailable".to_owned(), |v| format!("{v:.6}"))
                        };
                        let t = runs.first().and_then(|r| r.timings.get(i));
                        println!(
                            "{} | {} | {} | {} | {} | {} | {} | {} | {}",
                            op.label,
                            threads_name(threads, cores),
                            pick(&|t| t.generate.as_secs_f64()),
                            t.map_or(0, |t| t.drops),
                            pick(&|t| t.drop_time.as_secs_f64()),
                            t.map_or(0, |t| t.exact_queries),
                            pick(&|t| t.simulate.as_secs_f64()),
                            pick(&|t| t.fitting.as_secs_f64()),
                            pick(&|t| t.assembly.as_secs_f64()),
                        );
                    }
                    let totals: Vec<Option<f64>> = runs
                        .iter()
                        .map(|r| {
                            (!r.groups.is_empty() && r.groups.iter().all(Result::is_ok))
                                .then(|| r.timings.iter().map(|t| t.generate.as_secs_f64()).sum())
                        })
                        .collect();
                    println!(
                        "job generation median | {} | {}",
                        threads_name(threads, cores),
                        timing::successful_median(&totals).map_or_else(
                            || "unavailable: requires at least five complete successful runs"
                                .to_owned(),
                            |v| format!("{v:.6} s")
                        )
                    );
                    let wall: Vec<Option<f64>> = runs
                        .iter()
                        .map(|r| {
                            (!r.groups.is_empty() && r.groups.iter().all(Result::is_ok))
                                .then_some(r.total.as_secs_f64())
                        })
                        .collect();
                    println!(
                        "job wall median | {} | {}",
                        threads_name(threads, cores),
                        timing::successful_median(&wall).map_or_else(
                            || "unavailable: requires at least five complete successful runs"
                                .to_owned(),
                            |v| format!("{v:.6} s")
                        )
                    );
                }
                if let Form::Brep(surface, None, _) = form
                    && !args.skip_probe
                {
                    println!("### M3: an exact ball drop, probed on sampled drop lines");
                    println!(
                        "operation | drops kept | refined | not bracketed | drop on tessellation us | distance query us | exact ball drop us | offset p50 mm | p99 | exact above, max | below, max | estimated exact-surface generate s (all threads) | ratio"
                    );
                    for (i, op) in ops.iter().enumerate() {
                        if !op.ball {
                            continue;
                        }
                        let Some(t) = run.timings.get(i) else {
                            continue;
                        };
                        let probe = probe_exact_ball(surface, &op.profile()?, &t.lines);
                        let per_drop = t.drop_time.as_secs_f64() / t.drops.max(1) as f64;
                        // The drops' share of the run, replaced by exact drops.
                        let wall = t.generate.as_secs_f64();
                        let share = (t.drop_time.as_secs_f64() / cores as f64).min(wall);
                        let estimate =
                            wall - share + share * probe.exact_us / (per_drop * 1e6).max(1e-9);
                        println!(
                            "{} | {} | {} | {} | {:.1} | {:.1} | {:.1} | {:.4} | {:.4} | {:.4} | {:.4} | {:.2} | {:.2}",
                            op.label,
                            t.lines.len(),
                            probe.refined,
                            probe.at_band,
                            per_drop * 1e6,
                            probe.distance_us,
                            probe.exact_us,
                            probe.p50,
                            probe.p99,
                            probe.above,
                            probe.below,
                            estimate,
                            estimate / wall.max(1e-9)
                        );
                    }
                }
            }
        }
        if wants("m4") {
            println!("### M4: the toolpath at 1, 2 and {cores} threads");
            let first = job.run(form, Threads::Exactly(1))?;
            if let Some(letter) = name.chars().next() {
                refused.insert(letter, first.groups.iter().map(Result::is_err).collect());
            }
            if name.starts_with(APP_FORM) && library_b.is_none() {
                library_b = Some(first.groups.iter().filter_map(|g| g.clone().ok()).collect());
            }
            let one = serde_json::to_string(&first.groups)?;
            let connected = job.connected_groups(&first.groups)?;
            let mut posted = BTreeMap::new();
            for post in POSTS {
                let text = posting::post_run(&connected, &post_job, post)?;
                if let Some(letter) = name.chars().next() {
                    library_posts.insert((letter, post), text.clone());
                }
                posted.insert(post, text);
            }
            for n in [2, cores] {
                let again = job.run(form, Threads::Exactly(n))?;
                checks::identical(
                    &one,
                    &serde_json::to_string(&again.groups)?,
                    &format!("form {name}, {n} threads, serialized toolpath"),
                )?;
                let connected = job.connected_groups(&again.groups)?;
                for post in POSTS {
                    checks::identical(
                        posted.get(post).ok_or("no library program")?,
                        &posting::post_run(&connected, &post_job, post)?,
                        &format!("form {name}, {n} threads, posted G-code ({post})"),
                    )?;
                }
                println!(
                    "{n} threads: toolpath and posted G-code identical (posts: {})",
                    POSTS.join(", ")
                );
            }
        }
    }

    if wants("m4") {
        println!("\n## M4: the application's G-code at 1, 2 and {cores} threads");
        // The application stops at a refused operation, so those are turned
        // off (UI-06): the rest is generated as the library made it.
        disable(&mut app, &ids, refused.get(&APP_FORM))?;
        app.save()?;
        for post in POSTS {
            let mut reference = None;
            for n in [1, 2, cores] {
                let mut again = Application::open(&dir, Threads::Exactly(n))?;
                again.set_post(Some(PostSelection::parse(post)));
                let started = Instant::now();
                let result = again.regenerate()?;
                let took = started.elapsed();
                let clean = result.reports.iter().all(|r| r.is_clean());
                let gcode = result.gcode.clone();
                if let Some(library) = library_posts.get(&(APP_FORM, post)) {
                    checks::identical(&1, &gcode.len(), "form d application file count")?;
                    let (_, text) = gcode.first().ok_or("no form d G-code")?;
                    checks::identical(
                        library,
                        text,
                        &format!("form d library/application G-code ({post})"),
                    )?;
                }
                checks::clean(
                    clean,
                    &format!("application, {post}, {n} threads, replay gates"),
                )?;
                if let Some(expected) = &reference {
                    checks::identical(
                        expected,
                        &gcode,
                        &format!("application, {post}, {n} threads, G-code"),
                    )?;
                }
                println!(
                    "application, {post}, {n} threads: regenerate {:.1} s, gates clean, G-code identical",
                    took.as_secs_f64()
                );
                if n == cores
                    && let Some(library) = &library_b
                {
                    // The paths agree to rounding: OI-1110 (decision 0277).
                    let largest = checks::within_rounding(
                        &serde_json::to_value(library)?,
                        &serde_json::to_value(&result.canonical_toolpath.groups)?,
                        PATH_ROUNDING,
                        "form d application path",
                    )?;
                    println!(
                        "the library's form d path is the application's{}",
                        if largest > 0.0 {
                            format!(" to {largest:e} mm (OI-1110)")
                        } else {
                            String::new()
                        }
                    );
                }
                reference.get_or_insert(gcode);
            }
        }
        if let Some(path) = &args.stl {
            let mut reference = None;
            let stl_dir = dir.with_extension("stl");
            let (mut stl_app, stl_ids) = project_stl(&stl_dir, path, &stl, &bounds, &ops)?;
            disable(&mut stl_app, &stl_ids, refused.get(&'a'))?;
            stl_app.save()?;
            for n in [1, 2, cores] {
                let mut again = Application::open(&stl_dir, Threads::Exactly(n))?;
                again.set_post(Some(PostSelection::parse("makera-z1")));
                let started = Instant::now();
                let result = again.regenerate()?;
                let gcode = result.gcode.clone();
                if let Some(library) = library_posts.get(&('a', "makera-z1")) {
                    checks::identical(&1, &gcode.len(), "form a application file count")?;
                    let (_, text) = gcode.first().ok_or("no form a G-code")?;
                    checks::identical(library, text, "form a library/application G-code")?;
                }
                checks::clean(
                    result.reports.iter().all(|r| r.is_clean()),
                    &format!("form a, {n} threads, replay gates"),
                )?;
                if let Some(expected) = &reference {
                    checks::identical(expected, &gcode, &format!("form a, {n} threads, G-code"))?;
                }
                println!(
                    "form a, {n} threads: regenerate {:.1} s, gates clean, G-code identical",
                    started.elapsed().as_secs_f64()
                );
                reference.get_or_insert(gcode);
            }
            if args.keep.is_none() {
                let _ = std::fs::remove_dir_all(stl_dir);
            }
        }
    }
    if args.keep.is_none() {
        let _ = std::fs::remove_dir_all(dir);
    } else {
        app.save()?;
        println!("project kept in {}", dir.display());
    }
    if wants("m2") {
        // Every station was bounded, or `m2` failed above; qualification
        // still requires the widest interval to be under 0.001 mm.
        checks::sampling_bound(proven_error)?;
        println!(
            "M2 qualified: widest interval {:.6} mm (under 0.001 mm)",
            proven_error.unwrap_or(0.0)
        );
    }
    Ok(())
}

/// Turns off the operations of `ids` that `refused` marks.
fn disable(
    app: &mut Application,
    ids: &[OperationId],
    refused: Option<&Vec<bool>>,
) -> Result<(), Error> {
    let Some(refused) = refused else {
        return Ok(());
    };
    for (id, off) in ids.iter().zip(refused) {
        if *off && let Some(op) = app.project().operation(*id) {
            let mut op = op.clone();
            op.enabled = false;
            println!(
                "operation '{}' is turned off: the strategy refused it",
                op.name
            );
            app.execute(Command::Replace(Item::Operation(op)))?;
        }
    }
    Ok(())
}

fn cpu_model() -> String {
    std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("model name"))
                .and_then(|l| l.split(':').nth(1))
                .map(|m| m.trim().to_owned())
        })
        .unwrap_or_else(|| "unknown".to_owned())
}

fn threads_name(threads: Threads, cores: usize) -> String {
    match threads {
        Threads::Exactly(n) => n.to_string(),
        Threads::All => format!("all ({cores})"),
    }
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    v.get(v.len() / 2).copied().unwrap_or(0.0)
}

/// The STEP file at `path`, imported by the kernel at `linear` mm and the
/// default angular deflection, and how long the import took.
fn import(path: &Path, linear: f64) -> Result<(BrepModel, Duration), Error> {
    import_at(path, linear, TessellationSettings::default().angular)
}

/// [`import`] at an angular deflection of `angular` radians.
fn import_at(path: &Path, linear: f64, angular: f64) -> Result<(BrepModel, Duration), Error> {
    let bytes = std::fs::read(path)?;
    let cancel = CancelToken::new();
    let started = Instant::now();
    let imported = StepImporter::new().import(
        &bytes,
        &ImportOptions {
            tessellation: Some(TessellationSettings { linear, angular }),
            ..ImportOptions::default()
        },
        &JobContext::new(&cancel, &NoProgress),
    )?;
    let took = started.elapsed();
    let ModelGeometry::Brep(model) = imported.geometry else {
        return Err(format!("{} is not a B-rep", path.display()).into());
    };
    Ok((model, took))
}

fn print_m1(part: &str, d: f64, model: &BrepModel, took: Duration) -> f64 {
    let report = measure::tessellation_error(model, 4);
    let mut worst: f64 = 0.0;
    for k in &report {
        println!(
            "{part} | {d} | {} | {} | {} | {} | {} | {:.5} | {:.5} | {:.5} | {} | {:.2} | {:.1} | {:.2}",
            k.kind,
            k.faces,
            k.triangles,
            k.measured,
            k.samples,
            k.max,
            k.p99,
            k.trimmed,
            k.closed_form.map_or("-".to_owned(), |c| format!("{c:.5}")),
            k.normal_degrees,
            k.closest_us,
            took.as_secs_f64()
        );
        worst = worst.max(k.max);
    }
    worst
}

/// The owner's placement of the blisk: on the rotary axis, from X 10 as the
/// owner's projects place `blisk.stl` (decision 0209 *Context*).
fn placement(model: &BrepModel) -> Result<Vec<TransformStep>, Error> {
    let b = brep_bounds(model);
    let spans = [b.max.x - b.min.x, b.max.y - b.min.y, b.max.z - b.min.z];
    // The disk's axis is its longest extent.
    let mut steps = Vec::new();
    if spans[2] > spans[0] && spans[2] > spans[1] {
        steps.push(TransformStep::Rotate {
            axis: Axis::Y,
            angle: Degrees(90.0),
        });
    } else if spans[1] > spans[0] && spans[1] > spans[2] {
        steps.push(TransformStep::Rotate {
            axis: Axis::Z,
            angle: Degrees(-90.0),
        });
    }
    steps.push(TransformStep::CentreOnRotaryAxis);
    let rotated = place(model, &steps)?;
    let x0 = brep_bounds(&rotated).min.x;
    steps.push(TransformStep::Translate {
        x: 10.0 - x0,
        y: 0.0,
        z: 0.0,
    });
    Ok(steps)
}

fn place(model: &BrepModel, steps: &[TransformStep]) -> Result<BrepModel, Error> {
    let map = brep_placement(model, steps)?;
    Ok(transform_brep(model, &map)?)
}

fn brep_bounds(model: &BrepModel) -> Aabb {
    let mut lo = Point3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
    let mut hi = Point3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for f in &model.faces {
        for v in &f.tessellation.vertices {
            lo = Point3::new(lo.x.min(v.x), lo.y.min(v.y), lo.z.min(v.z));
            hi = Point3::new(hi.x.max(v.x), hi.y.max(v.y), hi.z.max(v.z));
        }
    }
    Aabb { min: lo, max: hi }
}

/// The owner's mesh, moved so its bounding box sits on the STEP part's.
fn owner_stl(path: &Path, bounds: &Aabb) -> Result<(TriangleMesh, Vec3), Error> {
    let bytes = std::fs::read(path)?;
    let cancel = CancelToken::new();
    let imported = openchuck_import::import_bytes(
        "blisk.stl",
        &bytes,
        &ImportOptions::default(),
        &JobContext::new(&cancel, &NoProgress),
    )?;
    let ModelGeometry::Mesh(mut mesh) = imported.geometry else {
        return Err("the owner's STL is not a mesh".into());
    };
    let summary = openchuck_geom::summary(&mesh);
    let b = summary.bounds.ok_or("empty STL")?;
    let centre = |a: &Aabb| {
        Vec3::new(
            (a.min.x + a.max.x) / 2.0,
            (a.min.y + a.max.y) / 2.0,
            (a.min.z + a.max.z) / 2.0,
        )
    };
    let shift = centre(bounds) - centre(&b);
    for v in &mut mesh.vertices {
        *v = *v + shift;
    }
    Ok((mesh, shift))
}

/// One of the owner's operations, with its tool.
struct Op {
    label: &'static str,
    strategy: &'static str,
    tool: ToolSnapshot,
    parameters: Vec<(&'static str, ParameterValue)>,
    leave: f64,
    ball: bool,
}

impl Op {
    fn profile(&self) -> Result<RevolvedProfile, Error> {
        Ok(self.tool.tool.profile()?)
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

/// The owner's operations on the blisk (decision 0209 *Context*; the
/// projects of `docs/trial-feedback-projects/` on the trial branch), in
/// machining order. Indexed and simultaneous finishing, which the owner's
/// projects do not use, and rest machining take the helical finish's 2 mm
/// ball and stepover.
fn operations(args: &Args) -> Vec<Op> {
    let quick = |v: f64, q: f64| if args.quick { q } else { v };
    let flat6 = tool("Z1 O-flute metal 6x17", 116, ToolShape::FlatEnd, 6.0, 17.0);
    let ball2 = tool("Z1 ball 2x12", 205, ToolShape::BallEnd, 2.0, 12.0);
    let flat2 = tool("Z1 O-flute metal 2x8", 102, ToolShape::FlatEnd, 2.0, 8.0);
    let tapered = tool(
        "Tapered ball r0.25 5 deg",
        14,
        ToolShape::TaperedBall {
            tip_radius: Millimetres(0.25),
            taper_half_angle: openchuck_types::units::Degrees(5.0),
        },
        3.175,
        20.0,
    );
    let all = vec![
        Op {
            label: "roughing",
            strategy: "rotary_roughing",
            tool: flat6,
            parameters: vec![
                ("entry", ParameterValue::Text("plunge".into())),
                ("feed", n(200.0)),
                ("plunge_feed", n(500.0)),
                ("stepdown", n(1.0)),
                ("stepover", n(2.0)),
                ("stock_to_leave", n(0.3)),
            ],
            leave: 0.3,
            ball: false,
        },
        Op {
            label: "rest",
            strategy: "rest_machining",
            tool: ball2.clone(),
            parameters: vec![("feed", n(500.0)), ("stepover", n(quick(0.3, 1.0)))],
            leave: 0.0,
            ball: true,
        },
        Op {
            label: "pencil",
            strategy: "pencil_finishing",
            tool: flat2,
            parameters: Vec::new(),
            leave: 0.0,
            ball: false,
        },
        Op {
            label: "helical",
            strategy: "helical_finishing",
            tool: ball2.clone(),
            parameters: vec![
                ("feed", n(500.0)),
                ("max_angle_step", n(2.0)),
                ("stepover", n(quick(0.3, 1.0))),
            ],
            leave: 0.0,
            ball: true,
        },
        Op {
            label: "axial",
            strategy: "axial_finishing",
            tool: tapered,
            parameters: vec![
                ("feed", n(500.0)),
                ("plunge_feed", n(200.0)),
                ("stepover", n(quick(0.12, 1.0))),
            ],
            leave: 0.0,
            ball: false,
        },
        Op {
            label: "indexed",
            strategy: "indexed_finishing",
            tool: ball2.clone(),
            parameters: vec![
                ("angles", ParameterValue::Text("0, 90, 180, 270".into())),
                ("feed", n(500.0)),
                ("stepover", n(quick(0.3, 1.0))),
            ],
            leave: 0.0,
            ball: true,
        },
        Op {
            label: "simultaneous",
            strategy: "simultaneous_finishing",
            tool: ball2,
            parameters: vec![
                ("feed", n(500.0)),
                ("max_angle_step", n(args.sim_step)),
                ("max_tilt", n(args.sim_tilt)),
                ("stepover", n(quick(0.3, 1.0))),
            ],
            leave: 0.0,
            ball: true,
        },
    ];
    if args.ops.is_empty() {
        all
    } else {
        all.into_iter()
            .filter(|o| args.ops.iter().any(|w| w == o.label))
            .collect()
    }
}

/// The setup's stock: a cylinder 1 mm larger in radius than the part and
/// 1 mm longer at each end (42 mm across for the blisk, as lane BM's blisk
/// project, decision 0115).
fn stock(bounds: &Aabb) -> Stock {
    let radius = [bounds.min.y, bounds.max.y, bounds.min.z, bounds.max.z]
        .into_iter()
        .map(f64::abs)
        .fold(0.0, f64::max);
    Stock::Cylinder {
        diameter: Millimetres(2.0 * radius + 2.0),
        length: Millimetres(bounds.max.x - bounds.min.x + 2.0),
        x_start: Millimetres(bounds.min.x - 1.0),
    }
}

/// A project in `dir` with the model file at `model_file` placed by
/// `placement`, the Z1, a setup in softwood and `ops`.
fn project(
    dir: &Path,
    model_file: &Path,
    placement: &[TransformStep],
    bounds: &Aabb,
    ops: &[Op],
) -> Result<(Application, Vec<OperationId>), Error> {
    workspace::create_destination(dir)?;
    let mut app = Application::create(dir, "blisk", Threads::All)?;
    std::fs::create_dir_all(dir.join("models"))?;
    let name = model_file.file_name().ok_or("no file name")?;
    let file = dir.join("models").join(name);
    std::fs::copy(model_file, &file)?;
    let (model, _) = app.add_model_with(&file, None, placement)?;
    app.execute(Command::SetMachine(Some(Box::new(
        builtin("makera-z1").ok_or("no Z1")??,
    ))))?;
    let setup_id = SetupId::new();
    let mut setup = Setup::new(setup_id, "Setup 1");
    setup.part = Some(model);
    setup.material = Some("Softwood".into());
    setup.stock = Some(stock(bounds));
    let mut items = vec![Item::Setup(setup)];
    let mut tools: Vec<ToolSnapshot> = Vec::new();
    let mut ids = Vec::new();
    for op in ops {
        let tool_id = match tools.iter().find(|t| t.name == op.tool.name) {
            Some(t) => t.id,
            None => {
                tools.push(op.tool.clone());
                items.push(Item::Tool(op.tool.clone()));
                op.tool.id
            }
        };
        let id = OperationId::new();
        ids.push(id);
        items.push(Item::Operation(Operation {
            enabled: true,
            id,
            order: OrderKey::default(),
            name: op.label.to_owned(),
            setup: setup_id,
            strategy: op.strategy.into(),
            tool: Some(tool_id),
            selections: Vec::new(),
            boundary: None,
            avoid: Vec::new(),
            check: Vec::new(),
            restriction_options: Default::default(),
            // The tool's cutting data for the setup's material (decision 0196).
            spindle_speed: None,
            material: None,
            // Face free ends off, as the measured project was saved before
            // it (decision record 0344): the measures are of the strategies.
            parameters: ["helical_finishing", "axial_finishing", "rest_machining"]
                .contains(&op.strategy)
                .then(|| ("face_free_ends".to_owned(), ParameterValue::Flag(false)))
                .into_iter()
                .chain(
                    op.parameters
                        .iter()
                        .map(|(k, v)| ((*k).to_owned(), v.clone())),
                )
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

/// The same project on the owner's STL, moved onto the STEP part.
fn project_stl(
    dir: &Path,
    path: &Path,
    stl: &Option<(TriangleMesh, Vec3)>,
    bounds: &Aabb,
    ops: &[Op],
) -> Result<(Application, Vec<OperationId>), Error> {
    let shift = stl.as_ref().map_or(Vec3::ZERO, |s| s.1);
    let placement = [TransformStep::Translate {
        x: shift.x,
        y: shift.y,
        z: shift.z,
    }];
    project(dir, path, &placement, bounds, ops)
}

/// The toolpath tolerance of every measured operation (the default, OP-11):
/// M2 says whether the proven deepest cut and the largest gap are within it
/// (lane STEPALL-X2).
const TOOLPATH_TOLERANCE: f64 = 0.01;

/// The built-in posts M4 writes and replays (lane STEPALL-X2).
const POSTS: [&str; 2] = ["makera-z1", "linuxcnc"];

/// How far, in millimetres, a number of the library's path may differ from
/// the application's (OI-1110): a thousandth of the 0.001 mm written
/// resolution. The blisk's full sequence differs by 1.35e-9 mm (axial
/// finishing's contact length, after the stock carried from the operations
/// before it), with byte-identical G-code.
const PATH_ROUNDING: f64 = 1e-6;

/// The library form the application's output equals: the STEP part with
/// exact contacts for the strategies that take them (decision 0277).
const APP_FORM: char = 'd';

/// The surface a form drops the tool on: for a STEP form, its faces with
/// drops on the tessellation, and for form (d) the same faces with exact
/// ball drops, which the operations whose strategy takes exact contacts use
/// (decision 0277), as the application does.
enum Form<'a> {
    Brep(BrepSurface, Option<Box<BrepSurface>>, &'a BrepModel),
    Mesh(MeshSurface),
}

impl Form<'_> {
    fn surface(&self) -> &dyn SurfaceSource {
        match self {
            Self::Brep(s, _, _) => s,
            Self::Mesh(s) => s,
        }
    }

    /// The surface `strategy` drops on in this form.
    fn surface_for(&self, strategy: &dyn Strategy) -> &dyn SurfaceSource {
        match self {
            Self::Brep(_, Some(exact), _) if strategy.exact_contact() => exact.as_ref(),
            _ => self.surface(),
        }
    }
}

/// One drop the probe looks at again: the line and the tessellation's
/// answer.
#[derive(Debug, Clone, Copy)]
struct Line {
    line: ToolLine,
    contact: Contact,
}

/// A surface that counts and times the queries made of it, and keeps every
/// `KEEP`th drop line (chosen by its coordinates, so the same lines are kept
/// at any thread count).
struct Timed<'a> {
    inner: &'a dyn SurfaceSource,
    drops: AtomicU64,
    drop_nanos: AtomicU64,
    exact: AtomicU64,
    lines: Mutex<Vec<Line>>,
}

const KEEP: u64 = 97;

impl<'a> Timed<'a> {
    fn new(inner: &'a dyn SurfaceSource) -> Self {
        Self {
            inner,
            drops: AtomicU64::new(0),
            drop_nanos: AtomicU64::new(0),
            exact: AtomicU64::new(0),
            lines: Mutex::new(Vec::new()),
        }
    }
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
        self.exact.fetch_add(1, Ordering::Relaxed);
        self.inner.project(point)
    }
    fn distance(&self, point: Point3) -> Option<f64> {
        self.exact.fetch_add(1, Ordering::Relaxed);
        self.inner.distance(point)
    }
    fn within(&self, point: Point3, max_distance: f64) -> bool {
        self.exact.fetch_add(1, Ordering::Relaxed);
        self.inner.within(point, max_distance)
    }
    fn drop_accuracy(&self) -> f64 {
        self.inner.drop_accuracy()
    }
    fn cast_ray(&self, origin: Point3, direction: UnitVec3, max_distance: f64) -> Option<RayHit> {
        self.exact.fetch_add(1, Ordering::Relaxed);
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
        let key = line.origin.x.to_bits()
            ^ line.origin.y.to_bits().rotate_left(21)
            ^ line.origin.z.to_bits().rotate_left(42);
        if let Some(c) = contact
            && key.is_multiple_of(KEEP)
            && let Ok(mut lines) = self.lines.lock()
        {
            lines.push(Line {
                line: *line,
                contact: c,
            });
        }
        contact
    }
    fn holes(&self) -> &[Hole] {
        self.inner.holes()
    }
    fn exact_faces(&self) -> Vec<FaceRef<'_>> {
        self.inner.exact_faces()
    }
}

/// Optional diagnostic scopes; fitting scopes can overlap across worker threads.
#[derive(Default)]
struct StageTimes {
    fitting: AtomicU64,
    assembly: AtomicU64,
}
impl openchuck_types::job::ProgressSink for StageTimes {
    fn report(&self, _: openchuck_types::job::Progress) {}
    fn wants_timings(&self) -> bool {
        true
    }
    fn timing(&self, stage: &str, elapsed: Duration) {
        let counter = match stage {
            "fitting" => &self.fitting,
            "assembly" => &self.assembly,
            _ => return,
        };
        counter.fetch_add(
            u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX),
            Ordering::Relaxed,
        );
    }
}

/// What the run of one operation took.
#[derive(Debug, Clone, Default)]
struct Timing {
    generate: Duration,
    drops: u64,
    drop_time: Duration,
    exact_queries: u64,
    simulate: Duration,
    fitting: Duration,
    assembly: Duration,
    lines: Vec<Line>,
}

struct Run {
    /// Each operation's moves, or the strategy's refusal (NFR-07): a
    /// refused operation cuts nothing and the next works on the same stock.
    groups: Vec<Result<ToolpathGroup, String>>,
    timings: Vec<Timing>,
    /// Entire sequential job, including stock construction and simulation.
    total: Duration,
}

/// Everything the library needs for the operations, from the project.
struct Job<'a> {
    project: &'a Project,
    registry: &'a openchuck_path::Registry,
    ids: &'a [OperationId],
    machine: &'a MachineProfile,
    rotary: RotaryMapping,
    part_bounds: Aabb,
}

impl Job<'_> {
    /// Mirror this harness project's single-setup application post context.
    /// M4 compares the resulting bytes with application output before using
    /// this context to establish form-c posting determinism.
    fn post_job(&self) -> Result<openchuck_post::PostJob, Error> {
        use openchuck_post::ToolSetting;
        use openchuck_types::kinematics::RotaryFrame;

        let setup = self.project.setups().first().ok_or("no setup")?;
        let mut tools = BTreeMap::new();
        let mut operation_names = BTreeMap::new();
        let mut operation_spindle = BTreeMap::new();
        for id in self.ids {
            let operation = self.project.operation(*id).ok_or("no operation")?;
            let snapshot = self
                .project
                .tools()
                .iter()
                .find(|t| Some(t.id) == operation.tool)
                .ok_or("no tool")?;
            let material = operation
                .material
                .as_ref()
                .or(setup.material.as_ref())
                .ok_or("no material")?;
            let (_, data) =
                openchuck_types::tool::find_cutting_data(&snapshot.cutting_data, material)
                    .ok_or("no cutting data")?;
            let speed = operation.spindle_speed.unwrap_or(data.spindle_speed).0;
            operation_names.insert(id.to_string(), operation.name.clone());
            operation_spindle.insert(id.to_string(), speed);
            tools
                .entry(snapshot.id.to_string())
                .or_insert_with(|| ToolSetting {
                    number: snapshot.controller.number,
                    name: snapshot.name.clone(),
                    spindle_rpm: speed,
                });
        }
        Ok(openchuck_post::PostJob {
            block_time: openchuck_post::BlockTimeSettings {
                min_block_ms: self.machine.min_block_ms,
                // This workspace uses a validated machine profile (<=4096).
                block_window: self.machine.block_window as u32,
                mode: self.machine.short_block_mode,
            },
            a_range: self.machine.a_range,
            safe_z: setup.clearance.unwrap_or(self.machine.clearance).safe_z.0,
            frame: RotaryFrame::work(self.machine, &setup.frame),
            tools,
            operation_names,
            operation_spindle,
            title: format!("{} - {}", self.project.name(), setup.name),
            revision: None,
            generator: Some(format!("OpenChuckCAM {}", env!("CARGO_PKG_VERSION"))),
            work_offset: Some(format!("{:?}", setup.frame.work_offset.register)),
        })
    }

    /// Mirror production operation-boundary assembly for this single-setup
    /// fixture, whose setup has no held region, tabs or fixtures. Generation
    /// comparisons stay canonical; only posting receives connected copies.
    fn connected_groups(
        &self,
        groups: &[Result<ToolpathGroup, String>],
    ) -> Result<Vec<Result<ToolpathGroup, String>>, Error> {
        use openchuck_path::program::{ConnectionOptions, connect_group};
        let setup = self.project.setups().first().ok_or("no setup")?;
        let stock = AnalyticStock::from_stock(
            setup.stock.as_ref().ok_or("no stock")?,
            Some(self.part_bounds),
        )?;
        let bounds = self.part_bounds;
        let radius = bounds
            .min
            .y
            .abs()
            .max(bounds.max.y.abs())
            .hypot(bounds.min.z.abs().max(bounds.max.z.abs()));
        let options = ConnectionOptions {
            machine: self.machine,
            clearance: setup.clearance.unwrap_or(self.machine.clearance),
            enclosing_radius: radius.max(stock.max_radius().ok_or("no stock radius")?),
            required_start_a: None,
        };
        let cancel = CancelToken::new();
        let context = JobContext::new(&cancel, &NoProgress);
        let mut result = Vec::new();
        let mut previous = None;
        for group in groups {
            match group {
                Ok(group) => {
                    let connected =
                        connect_group(previous.as_ref(), group, &options, &context)?.group;
                    previous = Some(connected.clone());
                    result.push(Ok(connected));
                }
                Err(error) => result.push(Err(error.clone())),
            }
        }
        Ok(result)
    }

    /// The operations in order on `form`, each on the stock the ones before
    /// it left, as the application's pipeline carries it (SIM-06).
    fn run(&self, form: &Form<'_>, threads: Threads) -> Result<Run, Error> {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(match threads {
                Threads::All => 0,
                Threads::Exactly(n) => n,
            })
            .build()?;
        pool.install(|| self.run_here(form).map_err(|e| e.to_string()))
            .map_err(Into::into)
    }

    fn run_here(&self, form: &Form<'_>) -> Result<Run, Error> {
        let job_started = Instant::now();
        let cancel = CancelToken::new();
        let stages = StageTimes::default();
        let context = JobContext::new(&cancel, &stages);
        let setup = self.project.setups().first().ok_or("no setup")?;
        let stock_def = setup.stock.as_ref().ok_or("no stock")?;
        let cell = self.project.simulation().cell.0;
        let mut simulator = Simulator::without_checkpoints(
            DexelStock::from_stock(stock_def, Some(self.part_bounds), cell)?,
            self.machine,
        );
        let analytic = AnalyticStock::from_stock(stock_def, Some(self.part_bounds))?;
        let mut groups = Vec::new();
        let mut timings = Vec::new();
        for (i, id) in self.ids.iter().enumerate() {
            let operation = self.project.operation(*id).ok_or("no operation")?;
            let snapshot = self
                .project
                .tools()
                .iter()
                .find(|t| Some(t.id) == operation.tool)
                .ok_or("no tool")?;
            let profile = snapshot.tool.profile()?;
            let values =
                openchuck_app::operation::parameter_values(self.project, operation, self.registry)?;
            let registered = self.registry.get(&operation.strategy)?;
            let timed = Timed::new(form.surface_for(registered));
            let tags = PathTags {
                operation: operation.id.to_string(),
                setup: setup.id.to_string(),
                tool: snapshot.id.to_string(),
            };
            let carried: &dyn StockModel = if i == 0 { &analytic } else { simulator.stock() };
            let input = StrategyInput {
                surface: &timed,
                stock: carried,
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
            let started = Instant::now();
            // The application rebuilds B-rep corners for each pencil operation
            // (decision 0226). Include that preparation in each generation
            // timing, rather than caching it outside the measured run.
            let pencil_on_brep = match form {
                Form::Brep(_, _, model) if operation.strategy == openchuck_path::pencil::NAME => {
                    Some(PencilFinishing::on_brep(model)?)
                }
                _ => None,
            };
            let strategy: &dyn Strategy = pencil_on_brep
                .as_ref()
                .map_or(registered, |pencil| pencil as &dyn Strategy);
            let path = generate(strategy, &input, &values.values, &context);
            let generated = started.elapsed();
            let started = Instant::now();
            if let Ok(path) = &path {
                simulator.run_group(&path.group, &profile, &context)?;
            }
            let simulated = started.elapsed();
            let mut lines = timed.lines.into_inner().unwrap_or_default();
            lines.sort_by(|a, b| {
                let k = |l: &Line| [l.line.origin.x, l.line.origin.y, l.line.origin.z];
                k(a).partial_cmp(&k(b)).unwrap_or(std::cmp::Ordering::Equal)
            });
            timings.push(Timing {
                generate: generated,
                drops: timed.drops.load(Ordering::Relaxed),
                drop_time: Duration::from_nanos(timed.drop_nanos.load(Ordering::Relaxed)),
                exact_queries: timed.exact.load(Ordering::Relaxed),
                simulate: simulated,
                fitting: Duration::from_nanos(stages.fitting.swap(0, Ordering::Relaxed)),
                assembly: Duration::from_nanos(stages.assembly.swap(0, Ordering::Relaxed)),
                lines,
            });
            groups.push(path.map(|p| p.group).map_err(|e| e.to_string()));
        }
        Ok(Run {
            groups,
            timings,
            total: job_started.elapsed(),
        })
    }
}

/// M2 for one operation's moves: every fed pose and the quarter, middle
/// and three-quarter points of every fed move (decisions 0115 and 0133),
/// each bounded against the exact faces and against the tessellation the
/// tool was dropped on.
fn m2(
    job: &Job<'_>,
    op: &Op,
    group: &ToolpathGroup,
    exact: &Geometry,
    tess: &Geometry,
) -> Result<Vec<bounded::Observation>, Error> {
    let tool = ConvexTool::new(&op.profile()?)?;
    let mut stops = Vec::new();
    let mut last = group.start;
    for s in &group.segments {
        if s.kind.is_fed() {
            let cut = s.kind == MoveKind::Cutting;
            for (k, st) in Station::along(&job.rotary, &last, &s.end)
                .into_iter()
                .enumerate()
            {
                stops.push(bounded::Stop {
                    tip: st.tip,
                    axis: st.axis,
                    place: bounded::Place::Quarter(k as u8 + 1),
                    cut,
                });
            }
            let st = Station::at(&s.end);
            stops.push(bounded::Stop {
                tip: st.tip,
                axis: st.axis,
                place: bounded::Place::Pose,
                cut,
            });
        }
        last = s.end;
    }
    Ok(bounded::observe(
        exact,
        tess,
        &tool,
        &stops,
        &bounded::settings(),
    )?)
}

/// The provenance written above each raw M2 file.
fn provenance(args: &Args, form: &str, fine: f64, op: &Op, part: &Path) -> String {
    let source = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_owned());
    let t = &op.tool.tool;
    format!(
        "step_accuracy M2, bounded checker (issue #346; decision 0209 item 4.1)\n\
         source: {source}\n\
         part: {}\n\
         stl: {}\n\
         form: {form}; form c deflection {fine} mm and {} rad; form b the import's default\n\
         operation: {} ({}); stock to leave {} mm; quick {}\n\
         tool: {:?}, diameter {} mm, flute {} mm\n\
         checker: tolerance {} mm, margin {} mm, budget {} pieces; slack {} mm\n\
         values: signed clearance, mm (negative inside the tool); upper is found at a point of the geometry, lower is proven",
        part.display(),
        args.stl
            .as_ref()
            .map_or_else(|| "none".to_owned(), |p| p.display().to_string()),
        args.angular,
        op.label,
        op.strategy,
        op.leave,
        args.quick,
        t.shape,
        t.diameter.0,
        t.flute_length.0,
        bounded::TOLERANCE,
        bounded::MARGIN,
        bounded::BUDGET,
        openchuck_testkit::contact::SLACK,
    )
}

struct Probe {
    /// Drop lines refined.
    refined: usize,
    /// Of those, how many ended at the edge of the band (no contact found
    /// inside it).
    at_band: usize,
    /// Time of one exact ball drop, single thread, microseconds (median).
    exact_us: f64,
    /// Time of one `distance` query, microseconds (median).
    distance_us: f64,
    /// How far the exact contact lies above the tessellation's along the
    /// tool line, mm: positive where the tessellation drop goes too deep.
    p50: f64,
    p99: f64,
    above: f64,
    below: f64,
}

/// At most this many drop lines are refined per operation, a fixed stride of
/// those kept, one thread: the probe prices one drop; it does not redo the job.
const PROBE_LINES: usize = 200;

/// For a ball end: sampled drops refined against the exact faces by
/// bisection on the distance from the ball's centre to them, in a band
/// round the tessellation's answer (decision 0209 item 4.2, seed and refine;
/// decision 0015). Not the exact drop of lane STEPALL-X: a measuring probe
/// of what one costs per drop with today's exact queries
/// (`BrepSurface::distance`), and how far the seed lies from it.
fn probe_exact_ball(surface: &BrepSurface, profile: &RevolvedProfile, lines: &[Line]) -> Probe {
    let radius = profile.cutting_radius();
    const BAND: f64 = 0.05;
    const STEPS: usize = 20;
    let stride = lines.len().div_ceil(PROBE_LINES).max(1);
    let mut drop_us = Vec::new();
    let mut query_us = Vec::new();
    let mut offsets = Vec::new();
    let mut at_band = 0;
    for l in lines.iter().step_by(stride) {
        let axis = l.line.axis.get();
        let started = Instant::now();
        let mut queries = 0_u32;
        let mut gap = |offset: f64| {
            queries += 1;
            let centre = l.line.origin + axis * (offset + radius);
            surface.distance(centre).map(|d| d - radius)
        };
        let (mut lo, mut hi) = (l.contact.offset - BAND, l.contact.offset + BAND);
        let (Some(top), Some(bottom)) = (gap(hi), gap(lo)) else {
            continue;
        };
        // Above the band the ball must be clear and below it in the part.
        let bracketed = top > 0.0 && bottom <= 0.0;
        if bracketed {
            for _ in 0..STEPS {
                let mid = 0.5 * (lo + hi);
                match gap(mid) {
                    Some(g) if g > 0.0 => hi = mid,
                    Some(_) => lo = mid,
                    None => break,
                }
            }
        }
        let took = started.elapsed().as_secs_f64() * 1e6;
        drop_us.push(took);
        query_us.push(took / f64::from(queries.max(1)));
        if bracketed {
            offsets.push(hi - l.contact.offset);
        } else {
            at_band += 1;
        }
    }
    let mut abs: Vec<f64> = offsets.iter().map(|d| d.abs()).collect();
    abs.sort_by(f64::total_cmp);
    let q = |v: &[f64], f: f64| {
        v.get(((v.len().saturating_sub(1)) as f64 * f).round() as usize)
            .copied()
            .unwrap_or(0.0)
    };
    Probe {
        refined: offsets.len(),
        at_band,
        exact_us: median(drop_us),
        distance_us: median(query_us),
        p50: q(&abs, 0.5),
        p99: q(&abs, 0.99),
        above: offsets.iter().copied().fold(0.0, f64::max),
        below: offsets.iter().map(|d| -d).fold(0.0, f64::max),
    }
}
