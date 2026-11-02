// SPDX-License-Identifier: AGPL-3.0-or-later
//! Picking in the viewport's scene, what a click selects, and the selection
//! panel's view model (UI-04, UI-05, OP-02; work breakdown slice 10 items 1
//! and 6, review checklist; decision record 0036).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_cmp
)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use openchuck_app::model::TransformStep;
use openchuck_app::pick::PickMode;
use openchuck_app::project::id::{ModelId, OperationId, SetupId};
use openchuck_app::project::{Item, Operation, OrderKey, Placement, Setup};
use openchuck_app::scene::{SceneRole, triangles_mesh};
use openchuck_app::setup::{MachineProfile, Millimetres, Stock};
use openchuck_app::{Application, Command, SelectionRole, Threads};

/// A project with the 10 mm corpus cube centred on the rotary axis, a machine,
/// a setup with the cube as its part, and one helical finishing operation
/// with no tool yet.
fn project(name: &str) -> (PathBuf, Application, OperationId, ModelId) {
    let dir = std::env::temp_dir().join(format!("openchuck-pick-{name}-{}", std::process::id()));
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
    setup.stock = Some(Stock::Cylinder {
        diameter: Millimetres(20.0),
        length: Millimetres(14.0),
        x_start: Millimetres(-7.0),
    });
    let op = OperationId::new();
    for item in [
        Item::Setup(setup),
        Item::Operation(Operation {
            id: op,
            order: OrderKey::default(),
            name: "Ring".into(),
            setup: setup_id,
            strategy: "helical_finishing".into(),
            tool: None,
            enabled: true,
            selections: Vec::new(),
            boundary: None,
            avoid: Vec::new(),
            check: Vec::new(),
            restriction_options: Default::default(),
            spindle_speed: None,
            material: None,
            parameters: BTreeMap::new(),
        }),
    ] {
        app.execute(Command::Add {
            item,
            placement: Placement::End,
        })
        .unwrap();
    }
    (dir, app, op, model)
}

fn set(items: &[u32]) -> BTreeSet<u32> {
    items.iter().copied().collect()
}

#[test]
fn req_ui_04_the_scene_picks_the_triangle_under_a_ray_on_the_placed_part() {
    let (dir, app, _, model) = project("scene");
    let view = app.start_scene(None).wait().unwrap();
    let picker = view.picking.picker().expect("the part can be picked");
    // The cube is centred on the axis: x from 0 to 10, y and z from -5 to 5.
    // Straight down onto the top face (z = 5).
    let hit = picker
        .pick([3.0, 1.0, 40.0], [0.0, 0.0, -1.0])
        .unwrap()
        .expect("the ray meets the top face");
    assert_eq!(hit.model, Some(model));
    assert_eq!(view.meshes[hit.mesh].role, SceneRole::Part);
    assert!((hit.distance - 35.0).abs() < 1e-9, "{hit:?}");
    let normal = hit.normal.unwrap();
    assert!((normal.z - 1.0).abs() < 1e-9, "{normal:?}");
    // The triangle is one of the two that the same direction selects.
    let top = app
        .select_region(
            model,
            &openchuck_app::RegionRequest::ByDirection {
                direction: normal,
                max_angle: 10.0,
            },
        )
        .unwrap();
    assert_eq!(top.len(), 2);
    assert!(top.contains(&hit.triangle));
    // A ray that misses, and a ray that is not a ray.
    assert_eq!(
        picker.pick([300.0, 0.0, 40.0], [0.0, 0.0, -1.0]).unwrap(),
        None
    );
    assert!(picker.pick([0.0; 3], [0.0; 3]).is_err());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn req_ui_04_each_pick_mode_selects_what_it_says() {
    let (dir, app, _, model) = project("modes");
    let view = app.start_scene(None).wait().unwrap();
    let picker = view.picking.picker().unwrap();
    let hit = picker
        .pick([3.0, 1.0, 40.0], [0.0, 0.0, -1.0])
        .unwrap()
        .unwrap();
    let select = |mode: PickMode| match mode.request(&hit) {
        None => set(&[hit.triangle]),
        Some(request) => app.select_region(model, &request).unwrap(),
    };
    assert_eq!(select(PickMode::Triangle), set(&[hit.triangle]));
    assert_eq!(select(PickMode::Connected).len(), 12);
    // At 30 degrees a cube's faces are not joined: the face alone.
    let face = select(PickMode::SmoothPatch {
        max_edge_angle: 30.0,
    });
    assert_eq!(face.len(), 2);
    assert!(face.contains(&hit.triangle));
    // At 90 degrees they are.
    assert_eq!(
        select(PickMode::SmoothPatch {
            max_edge_angle: 90.0
        })
        .len(),
        12
    );
    assert_eq!(select(PickMode::SameDirection { max_angle: 10.0 }), face);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn req_ui_04_a_selection_is_drawn_as_its_own_compact_mesh() {
    let (dir, app, _, model) = project("highlight");
    let view = app.start_scene(None).wait().unwrap();
    let part = &view.meshes[0];
    assert_eq!(part.model, Some(model));
    let top = set(&[2, 3]);
    let patch = triangles_mesh(&part.mesh, &top);
    assert_eq!(patch.triangles.len(), 2);
    // Two triangles of a square share two corners: four vertices, not eight.
    assert_eq!(patch.vertices.len(), 4);
    for (i, corners) in patch.triangles.iter().enumerate() {
        let original = part.mesh.triangle([2, 3][i]).unwrap();
        let drawn = patch.triangle(i).unwrap();
        assert_eq!(drawn, original);
        assert!(corners.iter().all(|&c| (c as usize) < 4));
    }
    // A triangle the mesh does not have is left out.
    assert!(triangles_mesh(&part.mesh, &set(&[99])).triangles.is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn req_ui_05_the_panel_lists_sets_with_their_uses_and_roles() {
    let (dir, mut app, op, model) = project("panel");
    let top = app.save_selection("Top", model, set(&[2, 3])).unwrap();
    let side = app.save_selection("Side", model, set(&[8, 9])).unwrap();
    let summaries = app.selection_summaries(&[]);
    assert_eq!(summaries.len(), 2);
    assert_eq!(summaries[0].name, "Top");
    assert_eq!(summaries[0].count, 2);
    assert_eq!(summaries[0].kind, "triangles");
    assert_eq!(summaries[0].model_path.as_deref(), Some("models/cube.stl"));
    assert!(summaries[0].uses.is_empty());
    assert!(summaries[0].problem.is_none());

    app.set_operation_selection(op, SelectionRole::Machine, top, true)
        .unwrap();
    app.set_operation_selection(op, SelectionRole::Avoid, side, true)
        .unwrap();
    // Putting it in again changes nothing and adds no undo step.
    let before = app.project().clone();
    app.set_operation_selection(op, SelectionRole::Avoid, side, true)
        .unwrap();
    assert_eq!(*app.project(), before);
    let summaries = app.selection_summaries(&[]);
    assert_eq!(summaries[0].uses[0].name, "Ring");
    assert_eq!(summaries[0].uses[0].roles, [SelectionRole::Machine]);
    assert_eq!(summaries[1].uses[0].roles, [SelectionRole::Avoid]);

    // A boundary holds one set: the second replaces the first.
    app.set_operation_selection(op, SelectionRole::Boundary, top, true)
        .unwrap();
    app.set_operation_selection(op, SelectionRole::Boundary, side, true)
        .unwrap();
    assert_eq!(app.project().operation(op).unwrap().boundary, Some(side));
    // Taking out a set that is not the boundary leaves it.
    app.set_operation_selection(op, SelectionRole::Boundary, top, false)
        .unwrap();
    assert_eq!(app.project().operation(op).unwrap().boundary, Some(side));
    app.set_operation_selection(op, SelectionRole::Boundary, side, false)
        .unwrap();
    assert_eq!(app.project().operation(op).unwrap().boundary, None);

    // The whole is undoable.
    assert!(app.undo().unwrap());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn req_ui_05_a_set_an_operation_uses_cannot_be_deleted_until_it_is_taken_out() {
    let (dir, mut app, op, model) = project("delete");
    let top = app.save_selection("Top", model, set(&[2, 3])).unwrap();
    app.set_operation_selection(op, SelectionRole::Machine, top, true)
        .unwrap();
    let message = app.delete_selection(top).unwrap_err().to_string();
    assert!(
        message.contains("Top") && message.contains("Ring"),
        "{message}"
    );
    app.set_operation_selection(op, SelectionRole::Machine, top, false)
        .unwrap();
    app.delete_selection(top).unwrap();
    assert!(app.selection_summaries(&[]).is_empty());
    // Deleting is one undoable step.
    assert!(app.undo().unwrap());
    assert_eq!(app.selection_summaries(&[]).len(), 1);
    assert!(
        app.delete_selection(openchuck_app::project::id::SelectionSetId::new())
            .is_err()
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn req_op_02_a_set_made_on_another_version_of_the_model_shows_a_warning() {
    let (dir, mut app, _, model) = project("warning");
    app.save_selection("Top", model, set(&[2, 3])).unwrap();
    assert!(app.selection_summaries(&[])[0].problem.is_none());

    // The file changes on disk: the set no longer matches what the project
    // knows, and the panel says so.
    let path = dir.join("models/cube.stl");
    let mut text = std::fs::read_to_string(&path).unwrap();
    text.push('\n');
    std::fs::write(&path, text).unwrap();
    let warnings = app.model_file_warnings();
    assert_eq!(warnings.len(), 1);
    let problem = app.selection_summaries(&warnings)[0]
        .problem
        .clone()
        .unwrap();
    assert!(problem.contains("models/cube.stl"), "{problem}");
    assert!(problem.contains("changed"), "{problem}");

    // The model is removed from the project's view of the set: a set whose
    // hash is not the model's, as after accepting the changed file.
    let mut edited = app.project().models()[0].clone();
    edited.hash = openchuck_app::project::ContentHash::of_bytes(b"another version");
    app.execute(Command::Replace(Item::Model(edited))).unwrap();
    let problem = app.selection_summaries(&[])[0].problem.clone().unwrap();
    assert!(problem.contains("different version"), "{problem}");
    assert!(problem.contains("select again"), "{problem}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn req_op_02_a_set_from_another_model_is_not_put_in_an_operation() {
    let (dir, mut app, op, model) = project("othermodel");
    let cube = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/stl/cube_ascii.stl");
    std::fs::copy(cube, dir.join("models/other.stl")).unwrap();
    let (other, _) = app.add_model(&dir.join("models/other.stl"), None).unwrap();
    assert_ne!(other, model);
    let foreign = app.save_selection("Foreign", other, set(&[2, 3])).unwrap();
    let own = app.save_selection("Own", model, set(&[2, 3])).unwrap();

    let before = app.project().clone();
    let error = app
        .set_operation_selection(op, SelectionRole::Machine, foreign, true)
        .unwrap_err();
    assert!(error.to_string().contains("different model"), "{error}");
    assert_eq!(*app.project(), before);
    app.set_operation_selection(op, SelectionRole::Machine, own, true)
        .unwrap();
    // Taking a set out is never refused.
    app.set_operation_selection(op, SelectionRole::Machine, foreign, false)
        .unwrap();
    let _ = std::fs::remove_dir_all(dir);
}
