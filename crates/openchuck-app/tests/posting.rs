// SPDX-License-Identifier: AGPL-3.0-or-later
//! Posting a project's toolpath to G-code files and checking them (GC-02,
//! GC-05, GC-07, GC-08, GC-11, GC-14; decision record 0020 item 4).

// Test code may unwrap, index and panic (NFR-07 applies to library code).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_cmp
)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use openchuck_app::model::TransformStep;
use openchuck_app::project::id::{OperationId, SetupId, ToolId};
use openchuck_app::project::{Item, Operation, OrderKey, Placement, Setup, ToolSnapshot};
use openchuck_app::setup::{MachineProfile, Millimetres, Stock};
use openchuck_app::{Application, Command, PostSelection, Threads};
use openchuck_types::tool::{ControllerMapping, CuttingData, Tool, ToolShape};
use openchuck_types::units::{FeedRate, SpindleSpeed};

fn ball() -> Tool {
    Tool {
        shape: ToolShape::BallEnd,
        diameter: Millimetres(4.0),
        flute_length: Millimetres(10.0),
        overall_length: Millimetres(40.0),
        shank_diameter: Millimetres(4.0),
        flute_count: 2,
        material: None,
    }
}

/// A project with the corpus cube (10 mm) centred on the rotary axis, the
/// example machine, an oak cylinder of stock, a ball tool with oak cutting
/// data, and one operation using the test strategy.
fn project(name: &str) -> (PathBuf, Application, OperationId) {
    let dir = std::env::temp_dir().join(format!("openchuck-op-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut app = Application::create(&dir, name, Threads::Exactly(2)).unwrap();
    std::fs::create_dir_all(dir.join("models")).unwrap();
    let cube = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/stl/cube_ascii.stl");
    std::fs::copy(cube, dir.join("models/cube.stl")).unwrap();
    let (model, _) = app.add_model(&dir.join("models/cube.stl"), None).unwrap();
    app.execute(Command::SetModelTransform {
        model,
        transform: vec![TransformStep::CentreOnRotaryAxis],
    })
    .unwrap();
    app.execute(Command::SetMachine(Some(Box::new(
        MachineProfile::example_x_rotary(),
    ))))
    .unwrap();
    let setup_id = SetupId::new();
    let mut setup = Setup::new(setup_id, "Setup 1");
    setup.part = Some(model);
    setup.material = Some("oak".into());
    setup.stock = Some(Stock::Cylinder {
        diameter: Millimetres(20.0),
        length: Millimetres(14.0),
        x_start: Millimetres(-7.0),
    });
    let tool_id = ToolId::new();
    let tool = ball();
    let mut cutting_data = BTreeMap::new();
    cutting_data.insert(
        "oak".to_owned(),
        CuttingData {
            spindle_speed: SpindleSpeed(10000.0),
            feed_per_tooth: Millimetres(0.02),
            plunge_feed: FeedRate(150.0),
            stepover: Millimetres(0.8),
            stepdown: Millimetres(1.0),
        },
    );
    let snapshot = ToolSnapshot {
        id: tool_id,
        order: OrderKey::default(),
        name: "4 mm ball".into(),
        tool,
        controller: ControllerMapping {
            number: 1,
            length_offset: None,
            diameter_offset: None,
        },
        cutting_data,
    };
    let op = OperationId::new();
    for item in [
        Item::Setup(setup),
        Item::Tool(snapshot),
        Item::Operation(Operation {
            id: op,
            order: OrderKey::default(),
            name: "Finish".into(),
            setup: setup_id,
            strategy: "helical_finishing".into(),
            tool: Some(tool_id),
            selections: Vec::new(),
            parameters: BTreeMap::new(),
        }),
    ] {
        app.execute(Command::Add {
            item,
            placement: Placement::End,
        })
        .unwrap();
    }
    (dir, app, op)
}

fn with_post(app: &mut Application, post: &str) {
    app.set_post(Some(PostSelection::parse(post)));
}

#[test]
fn req_gc_11_a_project_posts_to_g_code_that_passes_gates_1_and_2() {
    let (dir, mut app, _) = project("post-z1");
    for (post, ext) in [("linuxcnc", "ngc"), ("makera-z1", "nc")] {
        with_post(&mut app, post);
        let result = app.regenerate().unwrap();
        assert_eq!(result.gcode.len(), 1, "one setup, one file");
        let (name, text) = &result.gcode[0];
        assert_eq!(name, &format!("01-setup-1.{ext}"));
        assert!(text.starts_with("(post-z1 - Setup 1)\n"), "{}", &text[..60]);
        assert!(text.contains("T1 M6"), "the snapshot's tool number");
        assert!(
            text.contains("S10000 M3"),
            "spindle speed from the oak cutting data"
        );
        if std::env::var("DUMP").is_ok() {
            std::fs::write(format!("/tmp/dump-{post}.txt"), text).unwrap();
        }
        let report = &result.reports[0];
        assert!(report.is_clean(), "{post}: {report:?}");
        assert!(report.moves > 100);
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_07_the_same_project_gives_the_same_bytes_at_any_thread_count() {
    let (dir, mut app, _) = project("post-det");
    with_post(&mut app, "makera-z1");
    let first = app.regenerate().unwrap().gcode;
    app.save().unwrap();
    let again = Application::open(&dir, Threads::Exactly(1)).unwrap();
    let mut again = again;
    with_post(&mut again, "makera-z1");
    assert_eq!(again.regenerate().unwrap().gcode, first);
    let mut wide = Application::open(&dir, Threads::Exactly(4)).unwrap();
    with_post(&mut wide, "makera-z1");
    assert_eq!(wide.regenerate().unwrap().gcode, first);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_14_posting_uses_the_projects_tool_numbers_and_refuses_missing_cutting_data() {
    let (dir, mut app, _) = project("post-tool");
    with_post(&mut app, "linuxcnc");
    // A different material on the setup: no cutting data, so no spindle speed.
    let setup = app.project().setups()[0].clone();
    let mut changed = setup;
    changed.material = Some("pine".into());
    // Replacing the setup would also lose the feeds, so the error comes from regeneration.
    app.execute(Command::Replace(Item::Setup(changed))).unwrap();
    let e = app.regenerate().unwrap_err().to_string();
    assert!(e.contains("pine"), "{e}");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_02_an_unknown_post_is_refused_naming_the_choices() {
    let (dir, mut app, _) = project("post-unknown");
    app.set_post(Some(PostSelection::Builtin("nope".into())));
    let e = app.regenerate().unwrap_err().to_string();
    assert!(
        e.contains("nope") && e.contains("makera-z1") && e.contains("linuxcnc"),
        "{e}"
    );
    app.set_post(Some(PostSelection::Folder(dir.join("missing-post"))));
    assert!(app.regenerate().is_err());
    std::fs::remove_dir_all(dir).unwrap();
}
