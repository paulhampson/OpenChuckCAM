// SPDX-License-Identifier: AGPL-3.0-or-later
//! The scene view model the viewport draws (UI-02, UI-03; work breakdown
//! slice 8 items 1 and 2; decision record 0024), and the application state a
//! front end needs (unsaved changes, polling a job).

// Test code may unwrap, index and panic (NFR-07 applies to library code).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_cmp
)]

use std::path::{Path, PathBuf};

use openchuck_app::model::TransformStep;
use openchuck_app::project::id::SetupId;
use openchuck_app::project::{Item, Placement, Setup};
use openchuck_app::scene::{SceneRole, SceneView};
use openchuck_app::setup::{ASense, MachineProfile, Millimetres, Stock};
use openchuck_app::{Application, Command, Threads};

/// A project with the corpus cube (10 mm) centred on the rotary axis and one
/// setup with cylinder stock, work zero at X 3.
fn project(name: &str) -> (PathBuf, Application, SetupId) {
    let dir = std::env::temp_dir().join(format!("openchuck-scene-{name}-{}", std::process::id()));
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
    let id = SetupId::new();
    let mut setup = Setup::new(id, "Setup 1");
    setup.part = Some(model);
    setup.stock = Some(Stock::Cylinder {
        diameter: Millimetres(20.0),
        length: Millimetres(14.0),
        x_start: Millimetres(-2.0),
    });
    setup.frame.work_offset.x = Millimetres(3.0);
    app.execute(Command::Add {
        item: Item::Setup(setup),
        placement: Placement::End,
    })
    .unwrap();
    (dir, app, id)
}

fn scene(app: &Application) -> SceneView {
    app.start_scene(None).wait().unwrap()
}

fn lines(view: &SceneView, role: SceneRole) -> Vec<&Vec<openchuck_app::setup::Point3>> {
    view.lines
        .iter()
        .filter(|l| l.role == role)
        .map(|l| &l.points)
        .collect()
}

#[test]
fn req_ui_03_part_stock_axis_and_work_zero_are_drawn_together() {
    let (_dir, mut app, id) = project("together");
    app.execute(Command::SetMachine(Some(Box::new(
        MachineProfile::example_x_rotary(),
    ))))
    .unwrap();
    let view = scene(&app);
    assert_eq!(view.setup, Some(id));
    assert!(view.notes.is_empty(), "{:?}", view.notes);

    // The part: the placed cube, its 12 feature edges.
    assert_eq!(view.meshes.len(), 1);
    assert_eq!(view.meshes[0].role, SceneRole::Part);
    assert_eq!(view.meshes[0].mesh.triangles.len(), 12);
    assert_eq!(view.meshes[0].feature_edges.len(), 12);

    // The stock: two rims at X -2 and X 12 and four lengthwise lines, radius 10.
    let stock = lines(&view, SceneRole::Stock);
    assert_eq!(stock.len(), 6);
    for p in stock.iter().flat_map(|l| l.iter()) {
        assert!((p.y.hypot(p.z) - 10.0).abs() < 1e-9);
        assert!(p.x == -2.0 || p.x == 12.0, "{p:?}");
    }

    // The rotary axis lies on model X and runs past both ends of the stock.
    let axis = lines(&view, SceneRole::RotaryAxis);
    assert_eq!(axis.len(), 1);
    let (a, b) = (axis[0][0], axis[0][1]);
    assert!(a.y == 0.0 && a.z == 0.0 && b.y == 0.0 && b.z == 0.0);
    assert!(a.x < -2.0 && b.x > 12.0);

    // The work coordinate system starts at the work zero, on the axis at X 3.
    for role in [SceneRole::WorkX, SceneRole::WorkY, SceneRole::WorkZ] {
        let l = lines(&view, role);
        assert_eq!(l.len(), 1);
        assert_eq!((l[0][0].x, l[0][0].y, l[0][0].z), (3.0, 0.0, 0.0));
    }
    let texts: Vec<&str> = view.labels.iter().map(|l| l.text.as_str()).collect();
    assert_eq!(texts, ["A+", "X", "Y", "Z"]);

    // The bounds hold the stock and the origin.
    let bounds = view.bounds.unwrap();
    assert!(bounds.min.x <= -2.0 && bounds.max.x >= 12.0);
    assert!(bounds.min.y <= -10.0 && bounds.max.z >= 10.0);
}

#[test]
fn req_set_01_req_ui_03_a_arrow_follows_the_documented_convention() {
    // Positive sense (the Z1, confirmed on the machine, OI-03): positive A
    // turns the top of the part toward -Y. The arrow starts at the top (+Z)
    // and a quarter of the way round (90 of its 270 degrees) points to -Y.
    let (_dir, mut app, _) = project("a-arrow");
    let mut profile = MachineProfile::example_x_rotary();
    profile.a_sense = ASense::Positive;
    app.execute(Command::SetMachine(Some(Box::new(profile.clone()))))
        .unwrap();
    let view = scene(&app);
    let arc = lines(&view, SceneRole::ADirection)[0].clone();
    assert_eq!(arc.len(), 55);
    let (start, quarter) = (arc[0], arc[18]);
    assert!(start.y.abs() < 1e-9 && start.z > 0.0, "{start:?}");
    assert!(quarter.z.abs() < 1e-9 && quarter.y < 0.0, "{quarter:?}");
    assert!(
        view.a_direction
            .as_deref()
            .unwrap()
            .contains("faces -Y at A 90"),
        "{:?}",
        view.a_direction
    );

    // The negative sense turns the other way.
    profile.a_sense = ASense::Negative;
    app.execute(Command::SetMachine(Some(Box::new(profile))))
        .unwrap();
    let view = scene(&app);
    let quarter = lines(&view, SceneRole::ADirection)[0][18];
    assert!(quarter.y > 0.0, "{quarter:?}");
}

#[test]
fn req_ui_03_missing_machine_and_part_are_noted_not_errors() {
    let (_dir, mut app, id) = project("notes");
    // No machine: no arrow, a note.
    let view = scene(&app);
    assert!(lines(&view, SceneRole::ADirection).is_empty());
    assert!(view.a_direction.is_none());
    assert!(view.notes.iter().any(|n| n.contains("no machine profile")));

    // A setup with no part still shows its stock, axis and work zero.
    let mut setup = app.project().setup(id).unwrap().clone();
    setup.part = None;
    app.execute(Command::Replace(Item::Setup(setup))).unwrap();
    let view = scene(&app);
    assert!(view.meshes.is_empty());
    assert_eq!(lines(&view, SceneRole::Stock).len(), 6);
    assert!(view.notes.iter().any(|n| n.contains("no part")));

    // An unknown setup is an error.
    assert!(app.start_scene(Some(SetupId::new())).wait().is_err());
}

#[test]
fn req_ui_08_modified_follows_edits_undo_and_save() {
    let (_dir, mut app, _) = project("modified");
    // Created then edited, not yet saved.
    assert!(app.is_modified());
    app.save().unwrap();
    assert!(!app.is_modified());
    app.execute(Command::RenameProject("renamed".to_owned()))
        .unwrap();
    assert!(app.is_modified());
    // Undoing back to the saved state is unmodified again.
    app.undo().unwrap();
    assert!(!app.is_modified());
    app.redo().unwrap();
    assert!(app.is_modified());
}

#[test]
fn req_nfr_04_scene_job_can_be_polled() {
    let (_dir, app, _) = project("poll");
    let handle = app.start_scene(None);
    let result = loop {
        if let Some(result) = handle.try_result() {
            break result;
        }
        std::thread::yield_now();
    };
    assert!(result.is_ok());
    // A result is handed over once.
    assert!(handle.try_result().unwrap().is_err());
}

#[test]
fn req_nfr_06_scene_is_deterministic() {
    let (_dir, mut app, _) = project("determinism");
    app.execute(Command::SetMachine(Some(Box::new(
        MachineProfile::example_x_rotary(),
    ))))
    .unwrap();
    let first = scene(&app);
    assert_eq!(scene(&app), first);
}
