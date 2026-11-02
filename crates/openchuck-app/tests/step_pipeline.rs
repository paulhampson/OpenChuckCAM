// SPDX-License-Identifier: AGPL-3.0-or-later
//! STEP parts through the application (decision record 0056; OI-219, OI-226):
//! a STEP file is added to a project, placed, and machined on its B-rep by
//! radial drilling (OP-10) and edge following on its own edges (OP-08). The
//! `drilling` reference job (docs/reference-jobs.md) is built here, from the
//! OpenCASCADE fixture `rotary_blank_drilled`: it passes gates 1 and 2,
//! matches its golden files, and gate 3 (`cargo xtask replay`) replays its
//! LinuxCNC program. So is the `swarf` reference job (decision record 0111,
//! OI-238), from the fixture `hex_bar`: its six flats, picked in the scene,
//! are cut with the side of a flat end mill (OP-07). Needs the kernel:
//! `--features kernel-tests`.
#![cfg(feature = "kernel-tests")]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use openchuck_app::model::{Axis, Degrees, ModelKind, TransformStep};
use openchuck_app::project::id::{OperationId, SetupId, ToolId};
use openchuck_app::project::{
    Item, Operation, OrderKey, ParameterValue, Placement, Setup, ToolSnapshot,
};
use openchuck_app::setup::{Millimetres, Stock, builtin};
use openchuck_app::{Application, Command, PostSelection, Threads};
use openchuck_types::ir::MoveKind;
use openchuck_types::tool::{ControllerMapping, CuttingData, Tool, ToolShape};
use openchuck_types::units::{FeedRate, SpindleSpeed};

/// The fixture: a 24 mm rod along X, 60 mm long, with a 6 mm hole across it
/// at X 30 opening towards +Z (testdata/step/generate).
const FIXTURE: &str = "rotary_blank_drilled";

/// The placement of the reference job: turned 30 degrees about the rotary
/// axis, so the hole is drilled at A other than zero (IMP-05 on a B-rep).
const TURN: f64 = 30.0;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/step/valid")
        .join(format!("{name}.step"))
}

fn tool(name: &str, tool: Tool, feed_per_tooth: f64, rpm: f64, plunge: f64) -> ToolSnapshot {
    let mut cutting_data = BTreeMap::new();
    cutting_data.insert(
        "oak".to_owned(),
        CuttingData {
            spindle_speed: SpindleSpeed(rpm),
            feed_per_tooth: Millimetres(feed_per_tooth),
            plunge_feed: FeedRate(plunge),
            stepover: Millimetres(1.0),
            stepdown: Millimetres(1.0),
        },
    );
    ToolSnapshot {
        id: ToolId::new(),
        order: OrderKey::default(),
        name: name.into(),
        tool,
        controller: ControllerMapping {
            number: 1,
            length_offset: None,
            diameter_offset: None,
        },
        cutting_data,
    }
}

/// A 6 mm, 118 degree drill: 6000 rpm, 0.03 mm per tooth, two flutes.
fn drill() -> ToolSnapshot {
    tool(
        "6 mm drill",
        Tool {
            shape: ToolShape::Drill {
                point_angle: Degrees(118.0),
            },
            diameter: Millimetres(6.0),
            flute_length: Millimetres(40.0),
            overall_length: Millimetres(80.0),
            shank_diameter: Millimetres(6.0),
            flute_count: 2,
            material: None,
            centre_cutting: Some(true),
        },
        0.03,
        6000.0,
        120.0,
    )
}

/// The reference jobs' 3 mm ball in oak.
fn ball() -> ToolSnapshot {
    tool(
        "3 mm ball",
        Tool {
            shape: ToolShape::BallEnd,
            diameter: Millimetres(3.0),
            flute_length: Millimetres(8.0),
            overall_length: Millimetres(40.0),
            shank_diameter: Millimetres(3.0),
            flute_count: 2,
            material: None,
            centre_cutting: None,
        },
        0.02,
        12000.0,
        150.0,
    )
}

/// What a job is built from: the project's and the operation's names, the
/// fixture and the stock.
struct Job {
    project: &'static str,
    operation: &'static str,
    fixture: &'static str,
    stock: Stock,
}

/// The `drilling` job: stock 1 mm over the rod's radius and each end.
fn drilling_job() -> Job {
    Job {
        project: "drilling",
        operation: "Drill",
        fixture: FIXTURE,
        stock: Stock::Cylinder {
            diameter: Millimetres(26.0),
            length: Millimetres(62.0),
            x_start: Millimetres(-1.0),
        },
    }
}

/// A project in a new folder with the fixture copied into `models/`, placed
/// by `transform`, on the Makera Z1 in oak, with one operation.
fn build(
    tag: &str,
    transform: &[TransformStep],
    strategy: &str,
    snapshot: ToolSnapshot,
    parameters: &[(&str, ParameterValue)],
) -> (PathBuf, Application) {
    build_job(
        tag,
        drilling_job(),
        transform,
        strategy,
        snapshot,
        parameters,
    )
}

/// As [`build`], for `job`.
fn build_job(
    tag: &str,
    job: Job,
    transform: &[TransformStep],
    strategy: &str,
    snapshot: ToolSnapshot,
    parameters: &[(&str, ParameterValue)],
) -> (PathBuf, Application) {
    let dir = std::env::temp_dir().join(format!("openchuck-step-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut app = Application::create(&dir, job.project, Threads::Exactly(2)).unwrap();
    std::fs::create_dir_all(dir.join("models")).unwrap();
    let file = dir.join(format!("models/{}.step", job.fixture));
    std::fs::copy(fixture(job.fixture), &file).unwrap();
    let (model, inspection) = app.add_model_with(&file, None, transform).unwrap();
    assert_eq!(inspection.format.kind(), ModelKind::Brep);
    app.execute(Command::SetMachine(Some(Box::new(
        builtin("makera-z1").unwrap().unwrap(),
    ))))
    .unwrap();
    let setup_id = SetupId::new();
    let mut setup = Setup::new(setup_id, "Setup 1");
    setup.part = Some(model);
    setup.material = Some("oak".into());
    setup.stock = Some(job.stock);
    let tool_id = snapshot.id;
    for item in [
        Item::Setup(setup),
        Item::Tool(snapshot),
        Item::Operation(Operation {
            enabled: true,
            id: OperationId::new(),
            order: OrderKey::default(),
            name: job.operation.into(),
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
        }),
    ] {
        app.execute(Command::Add {
            item,
            placement: Placement::End,
        })
        .unwrap();
    }
    (dir, app)
}

fn turned() -> Vec<TransformStep> {
    vec![TransformStep::Rotate {
        axis: Axis::X,
        angle: Degrees(TURN),
    }]
}

/// The `drilling` reference job: the hole in four pecks with full retracts.
fn drilling(tag: &str) -> (PathBuf, Application) {
    build(
        tag,
        &turned(),
        "radial_drilling",
        drill(),
        &[("peck_depth", ParameterValue::Number(8.0))],
    )
}

fn golden(name: &str, text: &str) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/golden/reference")
        .join(name);
    openchuck_testkit::golden::check(&path, text).unwrap_or_else(|m| panic!("{m}"));
}

/// The committed project folders (OI-913, OI-922): the STEP reference jobs
/// are committed whole, their STEP files being small (decision record 0004).
fn projects_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/projects")
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).unwrap();
        }
    }
}

/// The committed project of STEP reference job `name` (OI-922): with
/// `OPENCHUCK_BLESS_PROJECTS=1` first rewritten from `build` (new item IDs,
/// so review that only they changed), then copied out, opened and
/// regenerated with both posts: every program passes gates 1 and 2 and
/// matches its golden file (GC-07, NFR-08).
fn committed(name: &str, build: impl FnOnce(&str) -> (PathBuf, Application)) {
    let target = projects_dir().join(name);
    if std::env::var_os("OPENCHUCK_BLESS_PROJECTS").is_some() {
        let (dir, app) = build(&format!("bless-{name}"));
        app.save().unwrap();
        let _ = std::fs::remove_dir_all(&target);
        copy_tree(&dir, &target);
        std::fs::remove_dir_all(dir).unwrap();
    }
    let dir = std::env::temp_dir().join(format!(
        "openchuck-step-committed-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    copy_tree(&target, &dir);
    let mut app = Application::open(&dir, Threads::Exactly(2)).unwrap();
    for (post, ext) in [("makera-z1", "nc"), ("linuxcnc", "ngc")] {
        app.set_post(Some(PostSelection::parse(post)));
        let result = app.regenerate().unwrap();
        let report = &result.reports[0];
        assert!(
            report.is_clean(),
            "{name} / {post}: {:?} {:?}",
            &report.findings[..report.findings.len().min(5)],
            report.comparison
        );
        golden(&format!("{name}_{post}.{ext}"), &stable(&result.gcode[0].1));
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_07_req_nfr_08_the_committed_drilling_project_writes_the_golden_programs() {
    committed("drilling", drilling);
}

#[test]
fn req_gc_07_req_nfr_08_the_committed_swarf_project_writes_the_golden_programs() {
    committed("swarf", swarf);
}

/// The program with the operation's id, which is new on every run, left out.
fn stable(text: &str) -> String {
    text.lines()
        .map(|l| {
            if l.starts_with("(operation ") {
                "(operation <id>)"
            } else {
                l
            }
        })
        .map(|l| format!("{l}\n"))
        .collect()
}

#[test]
fn req_imp_02_req_imp_05_a_step_model_is_added_inspected_and_placed() {
    let (dir, app) = build("inspect", &turned(), "radial_drilling", drill(), &[]);
    let id = app.project().models()[0].id;
    let inspection = app.inspect_model(id).unwrap();
    // Placed: the rod's ends stay at X 0 and 60, and it stays round the axis.
    let extent = inspection.extent.unwrap();
    assert!(extent.bounds.min.x.abs() < 1e-6 && (extent.bounds.max.x - 60.0).abs() < 1e-6);
    assert!((extent.radius.0 - 12.0).abs() < 0.02, "{}", extent.radius.0);
    assert!(inspection.summary.triangles > 0);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_op_10_req_gc_09_req_gc_10_the_drilling_reference_job_passes_gates_1_and_2_and_matches_its_golden_files()
 {
    let (dir, mut app) = drilling("gates");
    for (post, ext) in [("makera-z1", "nc"), ("linuxcnc", "ngc")] {
        app.set_post(Some(PostSelection::parse(post)));
        let result = app.regenerate().unwrap();
        let report = &result.reports[0];
        assert!(
            report.is_clean(),
            "{post}: {:?} {:?}",
            &report.findings[..report.findings.len().min(5)],
            report.comparison
        );
        // One hole, at the A that brings the turned hole up under the tool.
        let group = &result.toolpath.groups[0];
        let cuts: Vec<_> = group
            .segments
            .iter()
            .filter(|s| s.kind == MoveKind::Cutting)
            .collect();
        assert!(!cuts.is_empty());
        let a = cuts[0].end.a.0;
        assert!(
            cuts.iter().all(|s| (s.end.a.0 - a).abs() < 1e-9),
            "A moves within the hole"
        );
        assert!(
            ((a + TURN).rem_euclid(360.0)).min((-(a + TURN)).rem_euclid(360.0)) < 1e-6
                || ((a - TURN).rem_euclid(360.0)).min((TURN - a).rem_euclid(360.0)) < 1e-6,
            "A {a}"
        );
        golden(
            &format!("drilling_{post}.{ext}"),
            &stable(&result.gcode[0].1),
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_07_req_nfr_06_the_drilling_reference_job_is_identical_at_1_and_n_threads() {
    let (dir, mut app) = drilling("threads");
    app.set_post(Some(PostSelection::parse("makera-z1")));
    let first = app.regenerate().unwrap().gcode;
    app.save().unwrap();
    for threads in [1, 4] {
        let mut again = Application::open(&dir, Threads::Exactly(threads)).unwrap();
        again.set_post(Some(PostSelection::parse("makera-z1")));
        assert_eq!(again.regenerate().unwrap().gcode, first, "at {threads}");
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_op_08_a_step_part_is_engraved_along_its_b_rep_edges_through_the_application() {
    let depth = 0.2;
    let (dir, mut app) = build(
        "edges",
        &[],
        "edge_following",
        ball(),
        &[("depth", ParameterValue::Number(depth))],
    );
    app.set_post(Some(PostSelection::parse("linuxcnc")));
    let result = app.regenerate().unwrap();
    assert!(
        result.reports[0].is_clean(),
        "{:?}",
        result.reports[0].findings
    );
    // The rod's two rims and the hole's two rims, all on the 24 mm wall: every
    // cutting move is the depth below it.
    let cuts: Vec<_> = result.toolpath.groups[0]
        .segments
        .iter()
        .filter(|s| s.kind == MoveKind::Cutting)
        .collect();
    assert!(!cuts.is_empty());
    for s in cuts {
        let r = s.end.tip.y.hypot(s.end.tip.z);
        assert!((r - (12.0 - depth)).abs() <= 0.02, "tip at radius {r}");
    }
    // The B-rep default crease angle and the operation's shortest chain are
    // used, so no warning says they are not.
    assert!(
        !result.notes.iter().any(|n| n.contains("crease")),
        "{:?}",
        result.notes
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_imp_04_a_closed_step_part_is_checked_closed_and_an_open_one_shows_its_holes() {
    use openchuck_app::model::inspect_file;
    use openchuck_types::job::{CancelToken, JobContext, NoProgress};
    // The faces are tessellated one by one; their shared corners are joined
    // before the mesh checks (decision 0056), so a closed part is one closed
    // shell with its volume, and only a real opening is a hole.
    let cancel = CancelToken::new();
    let context = JobContext::new(&cancel, &NoProgress);
    let inspect = |name: &str| {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../testdata/step/valid")
            .join(format!("{name}.step"));
        inspect_file(&path, None, &[], &context).unwrap().report
    };
    for (name, shells, volume) in [
        ("box", 1, 20.0 * 30.0 * 40.0),
        (
            "sphere",
            1,
            4.0 / 3.0 * std::f64::consts::PI * 15.0_f64.powi(3),
        ),
        ("filleted_box", 1, 23_339.8),
        ("rotary_blank_drilled", 1, 26_460.1),
        ("multi_body", 3, 18_212.1),
    ] {
        let report = inspect(name);
        assert!(report.is_clean(), "{name}: {:?}", report.problems());
        assert_eq!(report.shells, shells, "{name}");
        let v = report.volume.unwrap();
        assert!((v - volume).abs() < 2e-3 * volume, "{name}: volume {v}");
    }
    // The revolved spline is an open shell: its two rims are holes.
    assert_eq!(inspect("revolved_spline").holes.len(), 2);
}

/// The swarf fixture: a hexagonal bar along X, 20 mm across the flats, from
/// X 0 to 40, with a flat on top (testdata/step/generate).
const HEX: &str = "hex_bar";
const ACROSS_FLATS: f64 = 20.0;
const HEX_LENGTH: f64 = 40.0;
/// The swarf tool's diameter.
const FLAT_END: f64 = 6.0;

/// A 6 mm flat end mill with 15 mm of flute, longer than a flat is wide
/// (11.55 mm), so each flat is one pass (OI-239): 12000 rpm, 0.02 mm per
/// tooth, two flutes.
fn flat_end() -> ToolSnapshot {
    tool(
        "6 mm flat end",
        Tool {
            shape: ToolShape::FlatEnd,
            diameter: Millimetres(FLAT_END),
            flute_length: Millimetres(15.0),
            overall_length: Millimetres(50.0),
            shank_diameter: Millimetres(FLAT_END),
            flute_count: 2,
            material: None,
            centre_cutting: None,
        },
        0.02,
        12000.0,
        150.0,
    )
}

/// The outward normals of the bar's six flats, in the YZ plane: the top
/// flat's is +Z, and they are 60 degrees apart.
fn flat_normals() -> Vec<[f64; 2]> {
    (0..6)
        .map(|k| {
            let t = (90.0 + 60.0 * f64::from(k)).to_radians();
            [t.cos(), t.sin()]
        })
        .collect()
}

/// The `swarf` reference job (decision record 0111): the bar as placed in
/// the file, in round stock 24 mm across (just over the 23.09 mm across its
/// corners) from X -1 to 41, its six flats picked in the scene from outside
/// and saved as one face set, cut with the flank of the 6 mm flat end.
fn swarf(tag: &str) -> (PathBuf, Application) {
    let (dir, mut app) = build_job(
        tag,
        Job {
            project: "swarf",
            operation: "Swarf",
            fixture: HEX,
            stock: Stock::Cylinder {
                diameter: Millimetres(24.0),
                length: Millimetres(HEX_LENGTH + 2.0),
                x_start: Millimetres(-1.0),
            },
        },
        &[],
        "swarf",
        flat_end(),
        &[],
    );
    let model = app.project().models()[0].id;
    let view = app.start_scene(None).wait().unwrap();
    let picker = view.picking.picker().expect("the scene can be picked on");
    let flats: std::collections::BTreeSet<_> = flat_normals()
        .iter()
        .map(|[ny, nz]| {
            let pick = picker
                .pick([HEX_LENGTH / 2.0, 100.0 * ny, 100.0 * nz], [0.0, -ny, -nz])
                .unwrap()
                .expect("the ray meets the flat");
            assert_eq!(pick.model, Some(model));
            pick.face.expect("a pick on a STEP part names its face")
        })
        .collect();
    assert_eq!(flats.len(), 6, "six flats, six faces");
    let set = app.save_selection("Flats", model, flats).unwrap();
    let mut operation = app.project().operations()[0].clone();
    operation.selections = vec![set];
    app.execute(Command::Replace(Item::Operation(operation)))
        .unwrap();
    (dir, app)
}

#[test]
fn req_op_07_req_gc_09_req_gc_10_the_swarf_reference_job_passes_gates_1_and_2_and_matches_its_golden_files()
 {
    let (dir, mut app) = swarf("swarf-gates");
    for (post, ext) in [("makera-z1", "nc"), ("linuxcnc", "ngc")] {
        app.set_post(Some(PostSelection::parse(post)));
        let result = app.regenerate().unwrap();
        let report = &result.reports[0];
        assert!(
            report.is_clean(),
            "{post}: {:?} {:?}",
            &report.findings[..report.findings.len().min(5)],
            report.comparison
        );
        // Every cutting move has the tool's side on a flat: the tip is one
        // tool radius out from the flat's plane, along its normal.
        let group = &result.toolpath.groups[0];
        let cuts: Vec<_> = group
            .segments
            .iter()
            .filter(|s| s.kind == MoveKind::Cutting)
            .collect();
        assert!(!cuts.is_empty());
        let out = ACROSS_FLATS / 2.0 + FLAT_END / 2.0;
        let normals = flat_normals();
        let mut a_of: [Option<f64>; 6] = [None; 6];
        for s in &cuts {
            let tip = s.end.tip;
            let (k, reach) = normals
                .iter()
                .map(|[ny, nz]| tip.y * ny + tip.z * nz)
                .enumerate()
                .fold((0, f64::MIN), |m, (k, r)| if r > m.1 { (k, r) } else { m });
            assert!((reach - out).abs() < 0.02, "tip {reach} from the axis");
            // A stays put along a flat.
            let a = s.end.a.0.rem_euclid(360.0);
            let first = *a_of[k].get_or_insert(a);
            assert!((a - first).abs() < 1e-6, "flat {k}: A {first} then {a}");
        }
        // Every flat is cut. Opposite flats have parallel rulings, so they
        // are cut at the same A, as the walls either side of the upright
        // tool: three A positions, 60 degrees apart.
        let a_of: Vec<f64> = a_of.iter().map(|a| a.expect("every flat is cut")).collect();
        for k in 0..3 {
            assert!((a_of[k] - a_of[k + 3]).abs() < 1e-6, "A {a_of:?}");
        }
        let mut angles = a_of[..3].to_vec();
        angles.sort_by(f64::total_cmp);
        for w in angles.windows(2) {
            assert!((w[1] - w[0] - 60.0).abs() < 1e-6, "A {angles:?}");
        }
        golden(&format!("swarf_{post}.{ext}"), &stable(&result.gcode[0].1));
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_07_req_nfr_06_the_swarf_reference_job_is_identical_at_1_and_n_threads() {
    let (dir, mut app) = swarf("swarf-threads");
    app.set_post(Some(PostSelection::parse("linuxcnc")));
    let first = app.regenerate().unwrap().gcode;
    app.save().unwrap();
    for threads in [1, 4] {
        let mut again = Application::open(&dir, Threads::Exactly(threads)).unwrap();
        again.set_post(Some(PostSelection::parse("linuxcnc")));
        assert_eq!(again.regenerate().unwrap().gcode, first, "at {threads}");
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_op_07_req_sim_05_the_swarf_reference_job_cuts_the_flats_and_nothing_of_the_part() {
    let (dir, app) = swarf("swarf-sim");
    let result = app.simulate().unwrap();
    let setup = &result.simulation.unwrap().setups[0];
    assert!(setup.operations.iter().all(|o| o.rapid_cuts.is_empty()));
    let d = setup.deviation.as_ref().unwrap();
    // Nothing is cut into the part, the bar's ends included: the tool stays
    // off its neighbouring faces (OI-237 is the gate's scope, not a gouge
    // here).
    assert_eq!(d.overcut_count, 0, "{:?}", d.worst_overcut);
    // Each flat is cut to within a cell (0.1 mm) along its length, away from
    // the ends, where the round stock beyond the bar is not the job's.
    let flats: Vec<f64> = d
        .points
        .iter()
        .filter(|p| p.normal.x.abs() < 1e-6 && p.point.x > 1.0 && p.point.x < HEX_LENGTH - 1.0)
        .map(|p| p.value)
        .collect();
    assert!(!flats.is_empty());
    let worst = flats.iter().copied().fold(f64::MIN, f64::max);
    assert!(worst <= 0.1 + 0.01, "material left on a flat: {worst}");
    std::fs::remove_dir_all(dir).unwrap();
}
