// SPDX-License-Identifier: AGPL-3.0-or-later
//! STEP parts through the application (decision record 0056; OI-219, OI-226):
//! a STEP file is added to a project, placed, and machined on its B-rep by
//! radial drilling (OP-10) and edge following on its own edges (OP-08). The
//! `drilling` reference job (docs/reference-jobs.md) is built here, from the
//! OpenCASCADE fixture `rotary_blank_drilled`: it passes gates 1 and 2,
//! matches its golden files, and gate 3 (`cargo xtask replay`) replays its
//! LinuxCNC program. Needs the kernel: `--features kernel-tests`.
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

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/step/valid")
        .join(format!("{FIXTURE}.step"))
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

/// A project in a new folder with the fixture copied into `models/`, placed
/// by `transform`, on the Makera Z1 in oak, with one operation.
fn build(
    tag: &str,
    transform: &[TransformStep],
    strategy: &str,
    snapshot: ToolSnapshot,
    parameters: &[(&str, ParameterValue)],
) -> (PathBuf, Application) {
    let dir = std::env::temp_dir().join(format!("openchuck-step-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut app = Application::create(&dir, "drilling", Threads::Exactly(2)).unwrap();
    std::fs::create_dir_all(dir.join("models")).unwrap();
    let file = dir.join(format!("models/{FIXTURE}.step"));
    std::fs::copy(fixture(), &file).unwrap();
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
    setup.stock = Some(Stock::Cylinder {
        diameter: Millimetres(26.0),
        length: Millimetres(62.0),
        x_start: Millimetres(-1.0),
    });
    let tool_id = snapshot.id;
    for item in [
        Item::Setup(setup),
        Item::Tool(snapshot),
        Item::Operation(Operation {
            enabled: true,
            id: OperationId::new(),
            order: OrderKey::default(),
            name: "Drill".into(),
            setup: setup_id,
            strategy: strategy.into(),
            tool: Some(tool_id),
            selections: Vec::new(),
            boundary: None,
            avoid: Vec::new(),
            check: Vec::new(),
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
