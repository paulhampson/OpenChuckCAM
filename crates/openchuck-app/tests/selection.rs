// SPDX-License-Identifier: AGPL-3.0-or-later
//! Selections through the application: picking, named sets, saving and
//! reopening them, and limiting an operation to them (UI-04, UI-05, PRJ-06,
//! OP-02; work breakdown slice 10 and its review checklist).

// Test code may unwrap, index and panic (NFR-07 applies to library code).
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
use openchuck_app::project::file::selection_inputs_hash;
use openchuck_app::project::id::{ModelId, OperationId, SelectionSetId, SetupId, ToolId};
use openchuck_app::project::{
    Item, MeshEdge, Operation, OrderKey, Placement, SelectionTarget, Setup, ToolSnapshot,
};
use openchuck_app::setup::{MachineProfile, Millimetres, Stock};
use openchuck_app::{
    AppError, Application, Command, Event, PostSelection, RegionRequest, SelectionEdit, Threads,
};
use openchuck_types::geometry::{Point3, Vec3};
use openchuck_types::ir::{MoveKind, ToolpathGroup};
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
        centre_cutting: None,
    }
}

/// A project with the corpus cube (10 mm) centred on the rotary axis, the
/// example machine, an oak cylinder of stock, a ball tool with oak cutting
/// data, and one helical finishing operation.
fn project(name: &str) -> (PathBuf, Application, OperationId, ModelId) {
    let dir = std::env::temp_dir().join(format!("openchuck-sel-{name}-{}", std::process::id()));
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
            name: "Ring".into(),
            setup: setup_id,
            strategy: "helical_finishing".into(),
            tool: Some(tool_id),
            enabled: true,
            selections: Vec::new(),
            boundary: None,
            avoid: Vec::new(),
            check: Vec::new(),
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

const UP: Vec3 = Vec3 {
    x: 0.0,
    y: 0.0,
    z: 1.0,
};

/// The triangles of the top (+Z) face of the cube.
fn top_face(app: &Application, model: ModelId) -> BTreeSet<u32> {
    app.select_region(
        model,
        &RegionRequest::ByDirection {
            direction: UP,
            max_angle: 10.0,
        },
    )
    .unwrap()
}

fn err<T: std::fmt::Debug>(r: Result<T, AppError>) -> String {
    r.unwrap_err().to_string()
}

fn set_roles(app: &mut Application, op: OperationId, change: impl FnOnce(&mut Operation)) {
    let mut edited = app.project().operation(op).unwrap().clone();
    change(&mut edited);
    app.execute(Command::Replace(Item::Operation(edited)))
        .unwrap();
}

fn cuts(group: &ToolpathGroup) -> Vec<(Point3, Point3)> {
    // (tool tip, ball centre) of every cutting move's end; the ball is 2 mm in radius.
    group
        .segments
        .iter()
        .filter(|s| s.kind == MoveKind::Cutting)
        .map(|s| {
            let a = s.end.axis.get();
            let tip = s.end.tip;
            (
                tip,
                Point3::new(tip.x + 2.0 * a.x, tip.y + 2.0 * a.y, tip.z + 2.0 * a.z),
            )
        })
        .collect()
}

/// How far `p` is from the top face of the placed cube (a 10 mm square at
/// z = 5, from x = 0 to 10, centred on the axis in Y and Z).
fn distance_to_top(p: Point3) -> f64 {
    let cx = p.x.clamp(0.0, 10.0);
    let cy = p.y.clamp(-5.0, 5.0);
    ((p.x - cx).powi(2) + (p.y - cy).powi(2) + (p.z - 5.0).powi(2)).sqrt()
}

/// How far a ball (radius 2) at `centre` is from touching the top face, mm.
fn gap_to_top(centre: Point3) -> f64 {
    (distance_to_top(centre) - 2.0).abs()
}

#[test]
fn req_ui_04_the_cube_has_the_expected_counts_for_each_way_of_picking() {
    let (dir, app, _, model) = project("counts");
    let count = |request: RegionRequest| app.select_region(model, &request).unwrap().len();
    // 12 triangles: two to a face.
    assert_eq!(count(RegionRequest::FloodFill { seeds: vec![0] }), 12);
    assert_eq!(
        count(RegionRequest::FloodFillByAngle {
            seeds: vec![0],
            max_edge_angle: 30.0
        }),
        2
    );
    assert_eq!(
        count(RegionRequest::FloodFillByAngle {
            seeds: vec![0],
            max_edge_angle: 90.0
        }),
        12
    );
    assert_eq!(top_face(&app, model).len(), 2);
    assert_eq!(
        count(RegionRequest::ByDirection {
            direction: UP,
            max_angle: 90.0
        }),
        10
    );
    // Bad requests are errors that say what is wrong.
    let e = app
        .select_region(model, &RegionRequest::FloodFill { seeds: vec![12] })
        .unwrap_err()
        .to_string();
    assert!(e.contains("triangle 12"), "{e}");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn req_ui_04_req_ui_09_selecting_a_region_is_a_reported_job() {
    let (dir, app, _, model) = project("region-job");
    app.drain_events();
    let request = RegionRequest::ByDirection {
        direction: UP,
        max_angle: 90.0,
    };
    let picked = app.start_select_region(model, &request).unwrap().wait();
    assert_eq!(picked.unwrap(), app.select_region(model, &request).unwrap());
    let events = app.drain_events();
    assert!(
        matches!(events.first(), Some(Event::JobStarted { .. })),
        "{events:?}"
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::JobFinished { error: None, .. })),
        "{events:?}"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn req_ui_05_req_prj_06_select_save_close_and_reopen_gives_the_same_set() {
    let (dir, mut app, _, model) = project("reopen");
    let top = top_face(&app, model);
    let id = app.save_selection("Top", model, top.clone()).unwrap();
    app.save().unwrap();
    drop(app);

    let again = Application::open(&dir, Threads::Exactly(2)).unwrap();
    let set = again
        .project()
        .selection_sets()
        .iter()
        .find(|s| s.id == id)
        .unwrap();
    assert_eq!(set.name, "Top");
    assert_eq!(set.model, model);
    assert_eq!(set.target, SelectionTarget::Triangles(top));
    let reference = again
        .project()
        .models()
        .iter()
        .find(|m| m.id == model)
        .unwrap();
    assert_eq!(set.model_hash, reference.hash);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn req_ui_05_saving_and_editing_are_undoable_and_checked() {
    let (dir, mut app, _, model) = project("edit");
    let top = top_face(&app, model);
    let id = app.save_selection("Top", model, top.clone()).unwrap();
    assert!(app.can_undo());
    assert!(app.undo().unwrap());
    assert!(app.project().selection_sets().is_empty());
    assert!(app.redo().unwrap());
    assert_eq!(app.project().selection_sets().len(), 1);

    let one: BTreeSet<u32> = top.iter().take(1).copied().collect();
    app.edit_selection(id, SelectionEdit::Remove(one.clone().into()))
        .unwrap();
    let triangles = |app: &Application| match &app.project().selection_sets()[0].target {
        SelectionTarget::Triangles(t) => t.clone(),
        other => panic!("{other:?}"),
    };
    assert_eq!(triangles(&app).len(), 1);
    app.edit_selection(id, SelectionEdit::Add(one.into()))
        .unwrap();
    assert_eq!(triangles(&app), top);
    app.edit_selection(id, SelectionEdit::Rename("Roof".into()))
        .unwrap();
    assert_eq!(app.project().selection_sets()[0].name, "Roof");
    app.undo().unwrap();
    assert_eq!(app.project().selection_sets()[0].name, "Top");

    // Refused: empty, beyond the model, and no such set.
    assert!(
        err(app.save_selection("None", model, BTreeSet::<u32>::new())).contains("no triangles")
    );
    assert!(err(app.save_selection("Far", model, BTreeSet::from([12]))).contains("triangle 12"));
    assert!(
        err(app.edit_selection(id, SelectionEdit::Replace(BTreeSet::<u32>::new().into())))
            .contains("no triangles")
    );
    assert!(
        err(app.edit_selection(SelectionSetId::new(), SelectionEdit::Rename("x".into())))
            .contains("no selection set")
    );
    assert_eq!(triangles(&app), top);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn req_ui_05_editing_a_set_marks_exactly_the_operations_that_use_it() {
    let (dir, mut app, op, model) = project("dependants");
    let top = top_face(&app, model);
    let roof = app.save_selection("Top", model, top.clone()).unwrap();
    let other = app
        .save_selection(
            "Bottom",
            model,
            app.select_region(
                model,
                &RegionRequest::ByDirection {
                    direction: Vec3 {
                        x: 0.0,
                        y: 0.0,
                        z: -1.0,
                    },
                    max_angle: 10.0,
                },
            )
            .unwrap(),
        )
        .unwrap();
    // A second operation uses the sets in other roles; a third uses none.
    let setup = app.project().operation(op).unwrap().setup;
    let tool = app.project().operation(op).unwrap().tool;
    let add = |app: &mut Application, name: &str| {
        let mut o = app.project().operation(op).unwrap().clone();
        o.id = OperationId::new();
        o.name = name.into();
        o.setup = setup;
        o.tool = tool;
        let id = o.id;
        app.execute(Command::Add {
            item: Item::Operation(o),
            placement: Placement::End,
        })
        .unwrap();
        id
    };
    let second = add(&mut app, "Second");
    let third = add(&mut app, "Third");
    set_roles(&mut app, op, |o| o.selections = vec![roof]);
    set_roles(&mut app, second, |o| o.avoid = vec![roof]);
    set_roles(&mut app, third, |o| o.check = vec![other]);

    let hash = |app: &Application, id| selection_inputs_hash(app.project(), id).unwrap();
    let before = [hash(&app, op), hash(&app, second), hash(&app, third)];
    let one: BTreeSet<u32> = top.iter().take(1).copied().collect();
    let affected = app
        .edit_selection(roof, SelectionEdit::Remove(one.into()))
        .unwrap();
    assert_eq!(affected, vec![op, second]);
    let after = [hash(&app, op), hash(&app, second), hash(&app, third)];
    assert_ne!(before[0], after[0]);
    assert_ne!(before[1], after[1]);
    assert_eq!(before[2], after[2]);
    // A rename changes no operation's inputs; undo restores the old hash.
    app.edit_selection(roof, SelectionEdit::Rename("Lid".into()))
        .unwrap();
    assert_eq!(hash(&app, op), after[0]);
    app.undo().unwrap();
    app.undo().unwrap();
    assert_eq!(hash(&app, op), before[0]);
    // The role matters, not only the set: moving the set between roles changes the hash.
    set_roles(&mut app, op, |o| {
        o.selections.clear();
        o.avoid = vec![roof];
    });
    assert_ne!(hash(&app, op), before[0]);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn req_op_02_an_operation_cuts_only_on_its_selected_region() {
    let (dir, mut app, op, model) = project("region");
    let whole = app.regenerate().unwrap().toolpath.groups[0].clone();
    let free = cuts(&whole);
    assert!(
        free.iter().any(|(_, c)| gap_to_top(*c) > 0.5),
        "the whole part is cut"
    );

    let roof = app
        .save_selection("Top", model, top_face(&app, model))
        .unwrap();
    set_roles(&mut app, op, |o| o.selections = vec![roof]);
    let limited = app.regenerate().unwrap().toolpath.groups[0].clone();
    let cut = cuts(&limited);
    assert!(!cut.is_empty());
    assert!(cut.len() < free.len());
    for (tip, centre) in &cut {
        assert!(
            gap_to_top(*centre) < 2e-4,
            "tip {tip:?} is not cutting the top face"
        );
    }
    // The same regeneration twice is byte for byte the same (GC-07).
    assert_eq!(app.regenerate().unwrap().toolpath.groups[0], limited);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn req_op_02_an_avoid_surface_is_never_cut() {
    let (dir, mut app, op, model) = project("avoid");
    let roof = app
        .save_selection("Top", model, top_face(&app, model))
        .unwrap();
    set_roles(&mut app, op, |o| o.avoid = vec![roof]);
    let cut = cuts(&app.regenerate().unwrap().toolpath.groups[0]);
    assert!(!cut.is_empty(), "the other faces are cut");
    for (tip, centre) in &cut {
        assert!(
            gap_to_top(*centre) > 1e-10,
            "tip {tip:?} touches the avoid surface"
        );
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn req_op_02_a_check_surface_is_not_touched_and_a_boundary_limits_the_cut() {
    let (dir, mut app, op, model) = project("check");
    let roof = app
        .save_selection("Top", model, top_face(&app, model))
        .unwrap();
    set_roles(&mut app, op, |o| o.check = vec![roof]);
    let cut = cuts(&app.regenerate().unwrap().toolpath.groups[0]);
    assert!(!cut.is_empty());
    assert!(cut.iter().all(|(_, c)| gap_to_top(*c) > 1e-10));

    // A boundary of the top face limits the cut to its window: X and angle.
    set_roles(&mut app, op, |o| {
        o.check.clear();
        o.boundary = Some(roof);
    });
    let cut = cuts(&app.regenerate().unwrap().toolpath.groups[0]);
    assert!(!cut.is_empty());
    for (_, centre) in &cut {
        // Where the ball touches the cube: the nearest point of the cube.
        let contact = Point3::new(
            centre.x.clamp(0.0, 10.0),
            centre.y.clamp(-5.0, 5.0),
            centre.z.clamp(-5.0, 5.0),
        );
        assert!(contact.x >= -1e-3 && contact.x <= 10.0 + 1e-3);
        let angle = contact.z.atan2(contact.y).to_degrees();
        assert!(angle > 45.0 - 0.1 && angle < 135.0 + 0.1, "angle = {angle}");
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn req_op_02_a_reference_that_cannot_be_resolved_stops_the_operation() {
    let (dir, mut app, op, model) = project("unresolved");
    let roof = app
        .save_selection("Roof", model, top_face(&app, model))
        .unwrap();
    set_roles(&mut app, op, |o| o.selections = vec![roof]);
    app.regenerate().unwrap();
    let says = |app: &Application, parts: &[&str]| {
        let e = app.regenerate().unwrap_err().to_string();
        for part in parts {
            assert!(e.contains(part), "{part}: {e}");
        }
    };

    // A triangle the model does not have.
    let set = |app: &Application| app.project().selection_sets()[0].clone();
    let mut bad = set(&app);
    bad.target = SelectionTarget::Triangles(BTreeSet::from([0, 999]));
    app.execute(Command::Replace(Item::SelectionSet(bad)))
        .unwrap();
    says(&app, &["'Ring'", "'Roof'", "triangle 999"]);
    // A set that selects nothing.
    let mut empty = set(&app);
    empty.target = SelectionTarget::Triangles(BTreeSet::new());
    app.execute(Command::Replace(Item::SelectionSet(empty)))
        .unwrap();
    says(&app, &["'Roof'", "no triangles"]);
    app.undo().unwrap();
    app.undo().unwrap();
    app.regenerate().unwrap();

    // A face selection, which a mesh does not have: refused when edited into
    // the project, so it cannot reach the operation.
    let mut faces = set(&app);
    faces.target = SelectionTarget::Faces(BTreeSet::new());
    assert!(
        app.execute(Command::Replace(Item::SelectionSet(faces)))
            .is_err()
    );

    // The model file changed after the set was made.
    let file = dir.join("models/cube.stl");
    let mut text = std::fs::read_to_string(&file).unwrap();
    text.push('\n');
    std::fs::write(&file, text).unwrap();
    says(&app, &["'Ring'", "'Roof'", "different version"]);
    assert!(
        app.save_selection("Again", model, top_face(&app, model))
            .is_err()
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn req_op_02_a_set_on_another_model_is_flagged() {
    let (dir, mut app, op, model) = project("other-model");
    let roof = app
        .save_selection("Roof", model, top_face(&app, model))
        .unwrap();
    std::fs::copy(dir.join("models/cube.stl"), dir.join("models/copy.stl")).unwrap();
    let (copy, _) = app.add_model(&dir.join("models/copy.stl"), None).unwrap();
    let mut elsewhere = app.project().selection_sets()[0].clone();
    elsewhere.model = copy;
    app.execute(Command::Replace(Item::SelectionSet(elsewhere)))
        .unwrap();
    set_roles(&mut app, op, |o| o.selections = vec![roof]);
    let e = app.regenerate().unwrap_err().to_string();
    assert!(e.contains("'Roof'") && e.contains("different model"), "{e}");
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A strategy that does not look at the region it is given.
#[derive(Debug)]
struct Careless;

impl openchuck_path::Strategy for Careless {
    fn name(&self) -> &str {
        "careless"
    }
    fn label(&self) -> &str {
        "Careless"
    }
    fn schema(&self) -> openchuck_path::ParameterSchema {
        openchuck_path::ParameterSchema::with_common(Vec::new())
    }
    fn passes(
        &self,
        _: &openchuck_path::StrategyInput<'_>,
        _: &openchuck_path::Parameters,
        _: &openchuck_types::job::JobContext<'_>,
    ) -> openchuck_path::Result<Vec<openchuck_path::Pass>> {
        Ok(Vec::new())
    }
}

#[test]
fn req_op_02_rotary_roughing_removes_stock_only_above_its_selected_region() {
    let (dir, mut app, op, model) = project("roughing-region");
    set_roles(&mut app, op, |o| o.strategy = "rotary_roughing".into());
    let free = cuts(&app.regenerate().unwrap().toolpath.groups[0]);
    assert!(!free.is_empty());

    let roof = app
        .save_selection("Top", model, top_face(&app, model))
        .unwrap();
    set_roles(&mut app, op, |o| o.selections = vec![roof]);
    let limited = app.regenerate().unwrap().toolpath.groups[0].clone();
    let cut = cuts(&limited);
    assert!(!cut.is_empty());
    assert!(cut.len() < free.len());
    // The ball rides above the roof: its centre lies within the angular reach
    // of the top face (45 to 135 degrees about X), with a tool radius of room.
    for (tip, centre) in &cut {
        let angle = centre.z.atan2(centre.y).to_degrees();
        assert!(
            (30.0..=150.0).contains(&angle),
            "tip {tip:?} is cutting away from the top face (angle {angle})"
        );
    }
    // Deterministic (GC-07).
    assert_eq!(app.regenerate().unwrap().toolpath.groups[0], limited);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn req_op_02_a_strategy_that_ignores_regions_is_refused_not_trusted() {
    let (dir, mut app, op, model) = project("careless");
    app.register_strategy(std::sync::Arc::new(Careless))
        .unwrap();
    let roof = app
        .save_selection("Roof", model, top_face(&app, model))
        .unwrap();
    set_roles(&mut app, op, |o| {
        o.strategy = "careless".into();
        o.selections = vec![roof];
    });
    let e = app.regenerate().unwrap_err().to_string();
    assert!(
        e.contains("'careless'") && e.contains("cannot yet be limited"),
        "{e}"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn req_op_11_a_selection_is_left_its_own_stock_in_place_of_the_operations() {
    let (dir, mut app, op, model) = project("leave");
    let roof = app
        .save_selection("Top", model, top_face(&app, model))
        .unwrap();
    set_roles(&mut app, op, |o| {
        o.selections = vec![roof];
        // A whole number, as a hand-edited project file may hold it: it is
        // still the operation's 1 mm.
        o.parameters.insert(
            "stock_to_leave".into(),
            openchuck_app::project::ParameterValue::Integer(1),
        );
    });
    let gaps = |app: &mut Application| -> Vec<f64> {
        let group = app.regenerate().unwrap().toolpath.groups[0].clone();
        cuts(&group).iter().map(|(_, c)| gap_to_top(*c)).collect()
    };
    // Without an amount of its own the operation's 1 mm applies.
    let before = selection_inputs_hash(app.project(), op).unwrap();
    for g in gaps(&mut app) {
        assert!((g - 1.0).abs() < 2e-3, "{g}");
    }
    // The set's 0.3 mm replaces it on the selection; the hash changes, so the
    // cache regenerates.
    let marked = app
        .edit_selection(roof, SelectionEdit::StockToLeave(Some(0.3)))
        .unwrap();
    assert_eq!(marked, vec![op]);
    assert_ne!(selection_inputs_hash(app.project(), op).unwrap(), before);
    let g = gaps(&mut app);
    assert!(!g.is_empty());
    // Away from the cube's edges the selection keeps 0.3 mm; beside an edge
    // the ball also rides the neighbouring faces' 1 mm, so it is never
    // closer than 0.3 mm and never further than 1 mm.
    for g in &g {
        assert!((0.3 - 2e-3..=1.0 + 2e-3).contains(g), "{g}");
    }
    // The ball is 2 mm in radius on a 10 mm face, so the part of the face
    // clear of the 1 mm beside the edges is a middle strip: some cuts keep
    // the selection's own 0.3 mm (the path has few points on a flat strip).
    // Beside an edge the rest of the part is left the operation's 1 mm (not
    // 0, as when a whole-number value was read as no value).
    assert!(
        g.iter().any(|g| (*g - 1.0).abs() < 2e-3),
        "no cut rides the rest of the part at 1 mm"
    );
    let own = g.iter().filter(|g| (**g - 0.3).abs() < 2e-3).count();
    assert!(own >= 3, "{own} of {} cuts keep 0.3 mm", g.len());
    // Bad amounts are refused; clearing goes back to the operation's.
    for bad in [-0.1, f64::NAN] {
        assert!(
            err(app.edit_selection(roof, SelectionEdit::StockToLeave(Some(bad))))
                .contains("zero or more")
        );
    }
    app.edit_selection(roof, SelectionEdit::StockToLeave(None))
        .unwrap();
    assert_eq!(selection_inputs_hash(app.project(), op).unwrap(), before);
    // The edit is one undoable command.
    app.edit_selection(roof, SelectionEdit::StockToLeave(Some(0.3)))
        .unwrap();
    app.undo().unwrap();
    assert_eq!(app.project().selection_sets()[0].stock_to_leave, None);
    std::fs::remove_dir_all(&dir).unwrap();
}

// Per-selection stock to leave for rotary roughing: the largest amount over
// the whole part, with a note (OP-03, OP-11; decision 0058).

/// Makes `op` a rotary roughing operation leaving `operation_leave`, limited
/// to the set `top` (the cube's top face) and `rest` (every other triangle),
/// so that it covers the whole part. Returns the two sets.
fn roughing_in_two_sets(
    app: &mut Application,
    op: OperationId,
    model: ModelId,
    operation_leave: f64,
) -> (SelectionSetId, SelectionSetId) {
    let roof = top_face(app, model);
    let others: BTreeSet<u32> = (0..12).filter(|t| !roof.contains(t)).collect();
    let top = app.save_selection("Top", model, roof).unwrap();
    let rest = app.save_selection("Rest", model, others).unwrap();
    set_roles(app, op, |o| {
        o.strategy = "rotary_roughing".into();
        o.selections = vec![top, rest];
        o.parameters.insert(
            "stock_to_leave".into(),
            openchuck_app::project::ParameterValue::Number(operation_leave),
        );
    });
    (top, rest)
}

fn set_leave(app: &mut Application, op: OperationId, leave: f64) {
    set_roles(app, op, |o| {
        o.parameters.insert(
            "stock_to_leave".into(),
            openchuck_app::project::ParameterValue::Number(leave),
        );
    });
}

/// The moves and the G-code of a run, without the project's IDs.
fn program(app: &Application) -> (Vec<ToolpathGroup>, Vec<(String, String)>, Vec<String>) {
    let r = app.regenerate().unwrap();
    assert!(!r.gcode.is_empty());
    (r.toolpath.groups, r.gcode, r.notes)
}

fn leave_notes(notes: &[String]) -> Vec<&String> {
    notes
        .iter()
        .filter(|n| n.contains("roughing leaves"))
        .collect()
}

#[test]
fn req_op_11_req_op_03_a_larger_selection_stock_to_leave_raises_roughing_over_the_whole_part() {
    let (dir, mut app, op, model) = project("rough-leave-larger");
    app.set_post(Some(PostSelection::Builtin("makera-z1".into())));
    let (top, _) = roughing_in_two_sets(&mut app, op, model, 0.3);
    app.edit_selection(top, SelectionEdit::StockToLeave(Some(0.8)))
        .unwrap();
    let (raised, gcode, notes) = program(&app);
    // The note names the value used, the selection that set it and the
    // operation's own value.
    let said = leave_notes(&notes);
    assert_eq!(said.len(), 1, "{notes:?}");
    for part in ["'Ring'", "0.8 mm", "'Top'", "0.3 mm"] {
        assert!(said[0].contains(part), "{part}: {}", said[0]);
    }
    // The same program, byte for byte, as the operation leaving 0.8 mm over
    // the whole part: the floor, the tool-drop heights and so the invariants
    // of decision 0033 (same sites per layer, no cut deeper than a stepdown,
    // a second pass cuts nothing) are those of a plain 0.8 mm roughing.
    app.edit_selection(top, SelectionEdit::StockToLeave(None))
        .unwrap();
    set_leave(&mut app, op, 0.8);
    let (plain, plain_gcode, plain_notes) = program(&app);
    assert_eq!(raised, plain);
    assert_eq!(gcode, plain_gcode);
    assert!(leave_notes(&plain_notes).is_empty(), "{plain_notes:?}");
    // And not the program of the operation's 0.3 mm.
    set_leave(&mut app, op, 0.3);
    assert_ne!(program(&app).1, gcode);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn req_op_11_req_op_03_a_smaller_or_equal_selection_stock_to_leave_changes_nothing() {
    let (dir, mut app, op, model) = project("rough-leave-smaller");
    app.set_post(Some(PostSelection::Builtin("makera-z1".into())));
    let (top, rest) = roughing_in_two_sets(&mut app, op, model, 0.5);
    let (alone, gcode, _) = program(&app);
    for (set, leave) in [(top, 0.2), (rest, 0.0), (top, 0.5)] {
        app.edit_selection(set, SelectionEdit::StockToLeave(Some(leave)))
            .unwrap();
        let (moves, text, notes) = program(&app);
        assert_eq!(moves, alone, "{leave}");
        assert_eq!(text, gcode, "{leave}");
        assert!(leave_notes(&notes).is_empty(), "{notes:?}");
        app.edit_selection(set, SelectionEdit::StockToLeave(None))
            .unwrap();
    }
    // Two sets with amounts: the largest is named, the operation's value is
    // below both.
    app.edit_selection(top, SelectionEdit::StockToLeave(Some(0.6)))
        .unwrap();
    app.edit_selection(rest, SelectionEdit::StockToLeave(Some(0.9)))
        .unwrap();
    let (_, _, notes) = program(&app);
    let said = leave_notes(&notes);
    assert_eq!(said.len(), 1, "{notes:?}");
    assert!(
        said[0].contains("0.9 mm") && said[0].contains("'Rest'"),
        "{}",
        said[0]
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn req_op_11_req_nfr_07_a_strategy_without_leave_zones_other_than_roughing_is_still_refused() {
    let (dir, mut app, op, model) = project("leave-refused");
    let (top, _) = roughing_in_two_sets(&mut app, op, model, 0.3);
    set_roles(&mut app, op, |o| {
        o.strategy = "simultaneous_finishing".into()
    });
    assert!(
        !app.strategies()
            .get("simultaneous_finishing")
            .unwrap()
            .honours_leave_zones()
    );
    app.edit_selection(top, SelectionEdit::StockToLeave(Some(0.8)))
        .unwrap();
    let e = err(app.regenerate());
    // The strategy is named by its label, and no requirement ID is shown
    // (specification section 15).
    let label = app.strategy_label("simultaneous_finishing");
    assert!(
        e.contains(&format!("'{label}'")) && !e.contains("OP-11") && !e.contains("_finishing"),
        "{e}"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn req_op_11_req_op_03_roughing_leaves_at_least_the_largest_and_finishing_each_its_own() {
    let (dir, mut app, op, model) = project("rough-leave-sim");
    // The cube runs from X 0 to 10: stock over all of it, so that every face
    // has material to leave.
    let mut setup = app.project().setups()[0].clone();
    setup.stock = Some(Stock::Cylinder {
        diameter: Millimetres(20.0),
        length: Millimetres(14.0),
        x_start: Millimetres(-2.0),
    });
    app.execute(Command::Replace(Item::Setup(setup))).unwrap();
    let (top, _) = roughing_in_two_sets(&mut app, op, model, 0.3);
    app.edit_selection(top, SelectionEdit::StockToLeave(Some(0.8)))
        .unwrap();
    // Helical finishing on the same sets after it, which leaves each its own
    // amount; off for now, to see what roughing alone leaves.
    let base = app.project().operation(op).unwrap().clone();
    let finish = Operation {
        id: OperationId::new(),
        name: "Finish".into(),
        strategy: "helical_finishing".into(),
        ..base.clone()
    };
    let finish_id = finish.id;
    app.execute(Command::Add {
        item: Item::Operation(finish),
        placement: Placement::End,
    })
    .unwrap();
    app.execute(Command::SetOperationEnabled {
        operation: finish_id,
        enabled: false,
    })
    .unwrap();
    let report = app.simulate().unwrap().simulation.unwrap();
    let setup = &report.setups[0];
    let removed = setup.operations[0].removed_volume;
    println!("roughing removed {removed:.2} mm3");
    assert!(removed > 100.0, "{removed}");
    // Everywhere at least the largest amount, to within the 0.1 mm cell.
    let d = setup.deviation.as_ref().unwrap();
    let least = d
        .points
        .iter()
        .map(|p| p.value)
        .fold(f64::INFINITY, f64::min);
    println!("least left after roughing {least:.3} mm");

    assert!(!d.points.is_empty());
    assert!(least >= 0.8 - 0.1, "{least}");

    // A second roughing pass over its own result finds nothing to cut
    // (decision 0033; with selections, decision 0043 item 5 says so).
    app.execute(Command::Add {
        item: Item::Operation(Operation {
            id: OperationId::new(),
            name: "Again".into(),
            ..base
        }),
        placement: Placement::End,
    })
    .unwrap();
    let e = err(app.simulate());
    assert!(e.contains("no cut to make"), "{e}");
    app.undo().unwrap();

    // Finishing then cuts each selection to its own amount: the middle of the
    // top face keeps 0.8 mm, the middle of the other side faces 0.3 mm.
    app.execute(Command::SetOperationEnabled {
        operation: finish_id,
        enabled: true,
    })
    .unwrap();
    let report = app.simulate().unwrap().simulation.unwrap();
    let d = report.setups[0].deviation.as_ref().unwrap();
    let (lo, hi) = d
        .points
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| {
            (lo.min(p.point.x), hi.max(p.point.x))
        });
    let mid_x = 0.5 * (lo + hi);
    let middle = |p: &&openchuck_app::sim::DeviationPoint| {
        p.normal.x.abs() < 1e-9
            && (p.point.x - mid_x).abs() < 2.0
            && p.point.y.abs().min(p.point.z.abs()) < 2.0
    };
    let mut on_top = Vec::new();
    let mut on_sides = Vec::new();
    for p in d.points.iter().filter(middle) {
        if p.normal.z > 0.5 {
            on_top.push(p.value);
        } else {
            on_sides.push(p.value);
        }
    }
    let range = |v: &[f64]| {
        v.iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), x| {
                (lo.min(*x), hi.max(*x))
            })
    };
    println!(
        "left on the top {:?}, on the sides {:?}",
        range(&on_top),
        range(&on_sides)
    );
    assert!(!on_top.is_empty() && !on_sides.is_empty());
    // Within the cell and the stepover's scallop.
    assert!(on_top.iter().all(|v| (v - 0.8).abs() <= 0.15), "{on_top:?}");
    assert!(
        on_sides.iter().all(|v| (v - 0.3).abs() <= 0.15),
        "{on_sides:?}"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn req_gc_07_req_nfr_06_req_op_11_roughing_with_a_selection_leave_is_the_same_at_any_thread_count()
{
    let (dir, mut app, op, model) = project("rough-leave-threads");
    let (top, _) = roughing_in_two_sets(&mut app, op, model, 0.3);
    app.edit_selection(top, SelectionEdit::StockToLeave(Some(0.8)))
        .unwrap();
    app.save().unwrap();
    let run = |threads: usize| {
        let mut app = Application::open(&dir, Threads::Exactly(threads)).unwrap();
        app.set_post(Some(PostSelection::Builtin("makera-z1".into())));
        program(&app)
    };
    let one = run(1);
    assert!(!leave_notes(&one.2).is_empty());
    for threads in [2, 4] {
        assert!(run(threads) == one, "{threads} threads");
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

/// The top front edge of the placed cube (y = -5, z = 5, x from 0 to 10),
/// picked in the scene and saved as a set of mesh edges (schema 12; OI-204,
/// decision record 0066).
fn front_edge(app: &mut Application, model: ModelId) -> (SelectionSetId, BTreeSet<MeshEdge>) {
    use openchuck_app::pick::{EdgeRef, PickMode, PickTolerance};
    let view = app.start_scene(None).wait().unwrap();
    let picker = view.picking.picker().unwrap();
    let reach = PickTolerance {
        base: 0.3,
        per_distance: 0.0,
    };
    let pick = picker
        .pick_edge([5.0, -4.9, 40.0], [0.0, 0.0, -1.0], reach)
        .unwrap()
        .expect("the pointer is over the edge");
    // A cube's corners join three edges, so its chain is the edge alone.
    let chosen = picker.select_edges(&pick, PickMode::Connected);
    assert_eq!(chosen.len(), 1);
    let edges: BTreeSet<MeshEdge> = chosen
        .into_iter()
        .map(|e| match e {
            EdgeRef::Mesh(m) => m,
            EdgeRef::Brep(_) => panic!("a mesh has no B-rep edges"),
        })
        .collect();
    let id = app
        .save_selection("Front edge", model, edges.clone())
        .unwrap();
    (id, edges)
}

#[test]
fn req_op_08_req_prj_06_chosen_mesh_edges_survive_reopening_and_are_the_only_ones_followed() {
    let (dir, mut app, op, model) = project("mesh-edges");
    let (id, edges) = front_edge(&mut app, model);
    app.save().unwrap();
    drop(app);

    // Reopened, the set names the same edges of the same file.
    let mut app = Application::open(&dir, Threads::Exactly(2)).unwrap();
    let set = app
        .project()
        .selection_sets()
        .iter()
        .find(|s| s.id == id)
        .unwrap()
        .clone();
    assert_eq!(set.target, SelectionTarget::MeshEdges(edges));
    assert_eq!(set.target.kind_name(), "edges");

    // Edge following on it engraves that edge and no other.
    set_roles(&mut app, op, |o| {
        o.strategy = "edge_following".into();
        o.selections = vec![id];
    });
    let result = app.regenerate().unwrap();
    let tips: Vec<Point3> = result.toolpath.groups[0]
        .segments
        .iter()
        .filter(|s| s.kind == MoveKind::Cutting)
        .map(|s| s.end.tip)
        .collect();
    assert!(!tips.is_empty());
    for tip in &tips {
        // On the edge's radial plane (y = -5, z = 5 is at 135 degrees from
        // +Y; the tip is cut in radially from it).
        let angle = tip.z.atan2(tip.y).to_degrees();
        assert!((angle - 135.0).abs() < 1e-6, "{tip:?} at {angle} degrees");
        assert!((-1e-6..=10.0 + 1e-6).contains(&tip.x), "{tip:?}");
    }
    assert!(tips.iter().any(|t| t.x < 1.0) && tips.iter().any(|t| t.x > 9.0));

    // As a boundary, or for a strategy that does not follow edges, it is
    // refused with a message naming it.
    set_roles(&mut app, op, |o| {
        o.strategy = "helical_finishing".into();
    });
    let e = app.regenerate().unwrap_err().to_string();
    assert!(e.contains("'Front edge'") && e.contains("edges"), "{e}");
    set_roles(&mut app, op, |o| {
        o.strategy = "edge_following".into();
        o.selections = Vec::new();
        o.boundary = Some(id);
    });
    let e = app.regenerate().unwrap_err().to_string();
    assert!(e.contains("'Front edge'"), "{e}");
    // Followed, it takes no boundary, avoid or check set of any kind, and no
    // stock to leave: each would be dropped, so each is refused (OP-02).
    let roof = top_face(&app, model);
    let avoid = app.save_selection("Roof", model, roof).unwrap();
    set_roles(&mut app, op, |o| {
        o.selections = vec![id];
        o.boundary = None;
        o.avoid = vec![avoid];
    });
    let e = app.regenerate().unwrap_err().to_string();
    assert!(e.contains("'Roof'") && e.contains("avoid"), "{e}");
    set_roles(&mut app, op, |o| o.avoid = Vec::new());
    app.edit_selection(id, SelectionEdit::StockToLeave(Some(0.5)))
        .unwrap();
    let e = app.regenerate().unwrap_err().to_string();
    assert!(
        e.contains("'Front edge'") && e.contains("stock to leave"),
        "{e}"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn req_ui_05_mesh_edge_sets_are_checked_and_edited_by_kind() {
    let (dir, mut app, _, model) = project("mesh-edge-edit");
    let (id, edges) = front_edge(&mut app, model);
    // A triangle beyond the model, or a side above 2, is refused.
    let beyond: BTreeSet<MeshEdge> = [MeshEdge::new(99, 0).unwrap()].into();
    assert!(err(app.save_selection("Beyond", model, beyond)).contains("triangle 99"));
    // Faces of a mesh are refused; an edit of another kind is refused.
    let faces: BTreeSet<openchuck_types::brep::FaceId> = [openchuck_types::brep::FaceId(0)].into();
    assert!(err(app.save_selection("Faces", model, faces)).contains("no faces"));
    let triangles: BTreeSet<u32> = [0].into();
    assert!(err(app.edit_selection(id, SelectionEdit::Add(triangles.into()))).contains("one kind"));
    // Removing its only edge would empty it.
    assert!(
        err(app.edit_selection(id, SelectionEdit::Remove(edges.clone().into())))
            .contains("no edges")
    );
    // Adding another edge is an undoable step.
    let more: BTreeSet<MeshEdge> = [MeshEdge::new(0, 0).unwrap()].into();
    app.edit_selection(id, SelectionEdit::Add(more.into()))
        .unwrap();
    let count = |app: &Application| {
        app.project()
            .selection_sets()
            .iter()
            .find(|s| s.id == id)
            .unwrap()
            .target
            .len()
    };
    assert_eq!(count(&app), edges.len() + 1);
    app.undo().unwrap();
    assert_eq!(count(&app), edges.len());
    // Keeping tangent edges is for a STEP model's edges only; the project's
    // default is an undoable command (decision record 0066).
    assert!(
        err(app.edit_selection(id, SelectionEdit::KeepTangentEdges(Some(true))))
            .contains("STEP model's edges")
    );
    assert!(!app.project().keep_tangent_edges());
    app.execute(Command::SetKeepTangentEdges(true)).unwrap();
    assert!(app.project().keep_tangent_edges());
    app.undo().unwrap();
    assert!(!app.project().keep_tangent_edges());
    // So is turning off the warning before a plain click of the other kind
    // resets the working selection (decision record 0066 item 3).
    assert!(app.project().warn_on_selection_reset());
    app.execute(Command::SetWarnOnSelectionReset(false))
        .unwrap();
    assert!(!app.project().warn_on_selection_reset());
    app.undo().unwrap();
    assert!(app.project().warn_on_selection_reset());
    // A set's IDs name the file it was made on: once the file changes, an
    // edit of its target is refused rather than checked against another file.
    let file = dir.join("models/cube.stl");
    let mut text = std::fs::read_to_string(&file).unwrap();
    text.push('\n');
    std::fs::write(&file, text).unwrap();
    let more: BTreeSet<MeshEdge> = [MeshEdge::new(1, 0).unwrap()].into();
    assert!(err(app.edit_selection(id, SelectionEdit::Add(more.into()))).contains("has changed"));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn req_nfr_04_req_ui_05_saving_a_set_is_a_reported_job_with_the_same_result() {
    let (dir, mut app, _, model) = project("save-job");
    let top = top_face(&app, model);
    app.drain_events();
    let job = app.start_save_selection("Top", model, top.clone()).unwrap();
    // Nothing is stored until the caller finishes the save on its own thread.
    let checked = job.wait().unwrap();
    assert!(app.project().selection_sets().is_empty());
    let events = app.drain_events();
    assert!(
        matches!(events.first(), Some(Event::JobStarted { .. })),
        "{events:?}"
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::JobFinished { error: None, .. })),
        "{events:?}"
    );
    let id = app.finish_save_selection(checked).unwrap();
    assert_eq!(
        app.project().selection_sets()[0].target,
        SelectionTarget::Triangles(top.clone())
    );
    // The synchronous call gives the same set apart from its fresh ID (GC-07).
    let again = app.save_selection("Top", model, top).unwrap();
    let sets = app.project().selection_sets();
    assert_ne!(id, again);
    assert_eq!(sets[0].target, sets[1].target);
    assert_eq!(sets[0].model_hash, sets[1].model_hash);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn req_nfr_04_a_failing_or_cancelled_save_stores_nothing() {
    let (dir, app, _, model) = project("save-job-fail");
    // A bad target fails in the job, not before it.
    let job = app
        .start_save_selection("Far", model, BTreeSet::from([12u32]))
        .unwrap();
    assert!(err(job.wait()).contains("triangle 12"));
    // A cancelled job gives a cancelled error, so there is nothing to store.
    let job = app
        .start_save_selection("Top", model, top_face(&app, model))
        .unwrap();
    job.cancel();
    let _ = job.wait();
    assert!(app.project().selection_sets().is_empty());
    // No such model fails at once.
    assert!(
        app.start_save_selection("x", ModelId::new(), BTreeSet::from([0u32]))
            .is_err()
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn req_nfr_04_req_ui_05_an_edit_of_the_target_is_a_job_and_the_rest_is_ready_at_once() {
    use openchuck_app::EditStart;
    let (dir, mut app, _, model) = project("edit-job");
    let top = top_face(&app, model);
    let id = app.save_selection("Top", model, top.clone()).unwrap();
    let one: BTreeSet<u32> = top.iter().take(1).copied().collect();
    assert!(matches!(
        app.start_edit_selection(id, SelectionEdit::Rename("Roof".into()))
            .unwrap(),
        EditStart::Done(_)
    ));
    let EditStart::Checking(job) = app
        .start_edit_selection(id, SelectionEdit::Remove(one.clone().into()))
        .unwrap()
    else {
        panic!("a change of target is checked as a job");
    };
    let checked = job.wait().unwrap();
    assert_eq!(app.project().selection_sets()[0].target.len(), top.len());
    app.finish_edit_selection(checked).unwrap();
    assert_eq!(
        app.project().selection_sets()[0].target.len(),
        top.len() - 1
    );
    // An edit that is wrong fails in the job.
    let EditStart::Checking(job) = app
        .start_edit_selection(id, SelectionEdit::Add(BTreeSet::from([12u32]).into()))
        .unwrap()
    else {
        panic!("checked as a job");
    };
    assert!(err(job.wait()).contains("triangle 12"));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn req_nfr_04_an_edit_made_from_an_older_set_is_refused_when_finished() {
    use openchuck_app::EditStart;
    let (dir, mut app, _, model) = project("edit-stale");
    let top = top_face(&app, model);
    let id = app.save_selection("Top", model, top.clone()).unwrap();
    let one: BTreeSet<u32> = top.iter().take(1).copied().collect();
    let EditStart::Checking(job) = app
        .start_edit_selection(id, SelectionEdit::Remove(one.into()))
        .unwrap()
    else {
        panic!("checked as a job");
    };
    let checked = job.wait().unwrap();
    // The set is renamed while the check ran: finishing would undo the rename.
    app.edit_selection(id, SelectionEdit::Rename("Roof".into()))
        .unwrap();
    assert!(err(app.finish_edit_selection(checked)).contains("changed while the edit"));
    assert_eq!(app.project().selection_sets()[0].name, "Roof");
    assert_eq!(app.project().selection_sets()[0].target.len(), top.len());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn req_ui_05_req_prj_06_replacing_a_stale_set_makes_it_current_and_add_and_remove_stay_refused() {
    let (dir, mut app, op, model) = project("stale-replace");
    let roof = app
        .save_selection("Roof", model, top_face(&app, model))
        .unwrap();
    set_roles(&mut app, op, |o| o.selections = vec![roof]);
    let current = app.project().models()[0].hash;
    // The set was made on an older version of the file.
    let mut old = app.project().selection_sets()[0].clone();
    old.model_hash = openchuck_app::project::ContentHash::of_bytes(b"an older version");
    app.execute(Command::Replace(Item::SelectionSet(old.clone())))
        .unwrap();
    let problem = |app: &Application| app.selection_summaries(&[])[0].problem.clone();
    assert!(problem(&app).unwrap().contains("different version"));
    // Adding to it or taking from it would mix numbers of two versions.
    let top = top_face(&app, model);
    let other: BTreeSet<u32> = (0..12).filter(|t| !top.contains(t)).take(1).collect();
    let member: BTreeSet<u32> = top.iter().copied().take(1).collect();
    assert!(err(app.edit_selection(roof, SelectionEdit::Add(other.into()))).contains("changed"));
    assert!(
        err(app.edit_selection(roof, SelectionEdit::Remove(member.into()))).contains("changed")
    );
    assert_eq!(app.project().selection_sets()[0].model_hash, old.model_hash);
    // Picking the same triangles again replaces the set all the same: it is
    // current, keeps its ID and its operations.
    let users = app
        .edit_selection(roof, SelectionEdit::Replace(top.clone().into()))
        .unwrap();
    assert_eq!(users, vec![op]);
    let set = app.project().selection_sets()[0].clone();
    assert_eq!(set.id, roof);
    assert_eq!(set.model_hash, current);
    assert_eq!(set.target, SelectionTarget::Triangles(top.clone()));
    assert!(problem(&app).is_none());
    // Undo puts the old, stale set back.
    app.undo().unwrap();
    assert_eq!(app.project().selection_sets()[0].model_hash, old.model_hash);
    // A file that changed since it was added is not selected on.
    let file = dir.join("models/cube.stl");
    let mut text = std::fs::read_to_string(&file).unwrap();
    text.push('\n');
    std::fs::write(&file, text).unwrap();
    let refused = err(app.edit_selection(roof, SelectionEdit::Replace(top.into())));
    assert!(
        refused.contains("has changed since it was added"),
        "{refused}"
    );
    assert_eq!(app.project().selection_sets()[0].model_hash, old.model_hash);
    std::fs::remove_dir_all(&dir).unwrap();
}
