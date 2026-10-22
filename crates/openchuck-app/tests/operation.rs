// SPDX-License-Identifier: AGPL-3.0-or-later
//! Operations through the application: mapping the project to the pathing
//! library, regeneration and its errors (OP-01, OP-11, OP-16; decision
//! record 0018 items 7 and 10).

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
use std::sync::Arc;

use openchuck_app::model::TransformStep;
use openchuck_app::operation::parameter_values;
use openchuck_app::project::id::{OperationId, SetupId, ToolId};
use openchuck_app::project::{
    Item, Operation, OrderKey, ParameterValue, Placement, Setup, ToolSnapshot,
};
use openchuck_app::setup::{MachineProfile, Millimetres, Stock};
use openchuck_app::{AppError, Application, Command, Threads};
use openchuck_path::params::names;
use openchuck_path::rest::parameter;
use openchuck_path::{CutPoint, ParameterSchema, Parameters, Pass, Strategy, StrategyInput};
use openchuck_types::ir::MoveKind;
use openchuck_types::job::JobContext;
use openchuck_types::tool::{ControllerMapping, CuttingData, Tool, ToolShape};
use openchuck_types::units::{Degrees, FeedRate, SpindleSpeed};

/// One ring of cuts at model X = 0 (a test strategy, OP-16).
#[derive(Debug)]
struct OneRing;

impl Strategy for OneRing {
    fn name(&self) -> &str {
        "one_ring"
    }
    fn label(&self) -> &str {
        "One ring"
    }

    fn description(&self) -> &str {
        "A strategy for tests."
    }
    fn schema(&self) -> ParameterSchema {
        ParameterSchema::with_common(Vec::new())
    }
    fn passes(
        &self,
        input: &StrategyInput<'_>,
        _: &Parameters,
        _: &JobContext<'_>,
    ) -> openchuck_path::Result<Vec<Pass>> {
        let points = (0..=36)
            .filter_map(|i| {
                let a = Degrees(10.0 * f64::from(i));
                let line = input
                    .rotary
                    .tool_line(Millimetres(0.0), a, Millimetres(0.0));
                let c = input.surface.drop_tool(input.tool, &line)?;
                Some(CutPoint {
                    tip: c.tip,
                    a,
                    contact: c.contact,
                })
            })
            .collect();
        Ok(vec![Pass {
            points,
            stock: None,
            rapids: Vec::new(),
        }])
    }
}

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
            enabled: true,
            id: op,
            order: OrderKey::default(),
            name: "Ring".into(),
            setup: setup_id,
            strategy: "one_ring".into(),
            tool: Some(tool_id),
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
    (dir, app, op)
}

#[test]
fn req_op_01_an_operation_regenerates_identically_at_any_time() {
    let (dir, mut app, _) = project("regen");
    // Without the strategy the operation is refused by name, not skipped.
    let e = app.regenerate().unwrap_err().to_string();
    assert!(e.contains("one_ring"), "{e}");
    app.register_strategy(Arc::new(OneRing)).unwrap();
    let first = app.regenerate().unwrap();
    assert_eq!(first.toolpath.groups.len(), 1);
    let group = &first.toolpath.groups[0];
    assert!(
        group
            .segments
            .iter()
            .filter(|s| s.kind == MoveKind::Cutting)
            .count()
            > 30
    );
    // The cube's faces are 5 mm from the axis; the tip never goes inside.
    for s in group
        .segments
        .iter()
        .filter(|s| s.kind == MoveKind::Cutting)
    {
        let r = s.end.tip.y.hypot(s.end.tip.z);
        assert!(r >= 5.0 - 1e-9, "{r}");
    }
    // Saved, reopened and regenerated: the same toolpath.
    app.save().unwrap();
    let mut again = Application::open(&dir, Threads::Exactly(3)).unwrap();
    again.register_strategy(Arc::new(OneRing)).unwrap();
    assert_eq!(again.regenerate().unwrap().toolpath, first.toolpath);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn req_op_11_feeds_and_stepover_default_from_the_tool_cutting_data() {
    let (dir, mut app, op) = project("defaults");
    app.register_strategy(Arc::new(OneRing)).unwrap();
    let operation = app.project().operation(op).unwrap().clone();
    let resolved = parameter_values(app.project(), &operation, app.strategies()).unwrap();
    let values = resolved.values;
    assert!(resolved.notes.is_empty(), "{:?}", resolved.notes);
    // 10000 rpm x 2 flutes x 0.02 mm.
    assert_eq!(
        values[names::FEED],
        openchuck_path::ParameterValue::Number(400.0)
    );
    assert_eq!(
        values[names::PLUNGE_FEED],
        openchuck_path::ParameterValue::Number(150.0)
    );
    assert_eq!(
        values[names::STEPOVER],
        openchuck_path::ParameterValue::Number(0.8)
    );
    let group = &app.regenerate().unwrap().toolpath.groups[0];
    let cut = group
        .segments
        .iter()
        .find(|s| s.kind == MoveKind::Cutting)
        .unwrap();
    assert_eq!(cut.contact_feed.unwrap().0, 400.0);

    // A value the operation sets wins over the cutting data, and a bad one
    // is refused naming the parameter.
    app.execute(Command::SetParameter {
        operation: op,
        name: names::FEED.into(),
        value: Some(ParameterValue::Number(250.0)),
    })
    .unwrap();
    let group = &app.regenerate().unwrap().toolpath.groups[0];
    let cut = group
        .segments
        .iter()
        .find(|s| s.kind == MoveKind::Cutting)
        .unwrap();
    assert_eq!(cut.contact_feed.unwrap().0, 250.0);
    // The override is reported, naming the value, the material and the tool;
    // an operation value equal to the material's says nothing.
    let notes = app.regenerate().unwrap().notes;
    assert_eq!(notes.len(), 1, "{notes:?}");
    for part in ["'Ring'", "feed 250", "400", "'4 mm ball'", "'oak'"] {
        assert!(notes[0].contains(part), "{part}: {}", notes[0]);
    }
    app.execute(Command::SetParameter {
        operation: op,
        name: names::FEED.into(),
        value: Some(ParameterValue::Number(400.0)),
    })
    .unwrap();
    assert!(app.regenerate().unwrap().notes.is_empty());
    app.execute(Command::SetParameter {
        operation: op,
        name: names::TOLERANCE.into(),
        value: Some(ParameterValue::Number(-1.0)),
    })
    .unwrap();
    let e = app.regenerate().unwrap_err().to_string();
    assert!(e.contains("tolerance"), "{e}");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn req_op_01_missing_pieces_are_named() {
    let (dir, mut app, op) = project("missing");
    app.register_strategy(Arc::new(OneRing)).unwrap();
    let mut operation = app.project().operation(op).unwrap().clone();
    operation.tool = None;
    app.execute(Command::Replace(Item::Operation(operation)))
        .unwrap();
    let e = app.regenerate().unwrap_err();
    assert!(matches!(e, AppError::Operation { .. }), "{e}");
    assert!(
        e.to_string().contains("'Ring'") && e.to_string().contains("no tool"),
        "{e}"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

/// The first cutting move's contact feed after regenerating.
fn first_cut_feed(app: &Application) -> f64 {
    let regeneration = app.regenerate().unwrap();
    regeneration.toolpath.groups[0]
        .segments
        .iter()
        .find(|s| s.kind == MoveKind::Cutting)
        .unwrap()
        .contact_feed
        .unwrap()
        .0
}

fn set_material(app: &mut Application, material: Option<&str>) {
    let mut setup = app.find_setup("Setup 1").unwrap().clone();
    setup.material = material.map(str::to_owned);
    app.execute(Command::Replace(Item::Setup(setup))).unwrap();
}

#[test]
fn req_op_11_req_tl_05_each_material_gives_its_own_feed_and_a_missing_one_is_named() {
    let (dir, mut app, op) = project("materials");
    app.register_strategy(Arc::new(OneRing)).unwrap();
    // Give the tool a second material with different cutting data.
    let mut snapshot = app.project().tools()[0].clone();
    snapshot.cutting_data.insert(
        "steel".into(),
        CuttingData {
            spindle_speed: SpindleSpeed(6000.0),
            feed_per_tooth: Millimetres(0.01),
            plunge_feed: FeedRate(60.0),
            stepover: Millimetres(0.3),
            stepdown: Millimetres(0.5),
        },
    );
    app.execute(Command::Replace(Item::Tool(snapshot))).unwrap();

    // 10000 x 2 x 0.02 for oak, 6000 x 2 x 0.01 for steel.
    assert_eq!(first_cut_feed(&app), 400.0);
    set_material(&mut app, Some("steel"));
    assert_eq!(first_cut_feed(&app), 120.0);
    let operation = app.project().operation(op).unwrap().clone();
    let values = parameter_values(app.project(), &operation, app.strategies())
        .unwrap()
        .values;
    assert_eq!(
        values[names::PLUNGE_FEED],
        openchuck_path::ParameterValue::Number(60.0)
    );
    assert_eq!(
        values[names::STEPOVER],
        openchuck_path::ParameterValue::Number(0.3)
    );

    // A name that differs only by case or spacing finds the same data
    // (decision 0196).
    for material in ["Steel", " STEEL "] {
        set_material(&mut app, Some(material));
        assert_eq!(first_cut_feed(&app), 120.0, "{material}");
    }

    // A material the tool has no data for is named, with the tool, and the
    // matching rule is said.
    for material in ["pine", "Stee l"] {
        set_material(&mut app, Some(material));
        let e = app.regenerate().unwrap_err();
        assert!(matches!(e, AppError::Operation { .. }), "{e}");
        let text = e.to_string();
        for part in [
            "'Ring'",
            "feed",
            "plunge_feed",
            "stepover",
            &format!("'{material}'"),
            "'4 mm ball'",
            "ignoring case",
        ] {
            assert!(text.contains(part), "{part}: {text}");
        }
    }

    // No material on the setup says so.
    set_material(&mut app, None);
    let text = app.regenerate().unwrap_err().to_string();
    assert!(text.contains("no stock material"), "{text}");

    // Setting the three values on the operation is the way out, and no note
    // is raised because no cutting data is overridden.
    for (name, value) in [
        (names::FEED, 200.0),
        (names::PLUNGE_FEED, 50.0),
        (names::STEPOVER, 0.5),
    ] {
        app.execute(Command::SetParameter {
            operation: op,
            name: name.into(),
            value: Some(ParameterValue::Number(value)),
        })
        .unwrap();
    }
    assert_eq!(first_cut_feed(&app), 200.0);
    assert!(app.regenerate().unwrap().notes.is_empty());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn req_op_05_req_op_01_simultaneous_finishing_runs_from_a_project_with_the_builtin_strategy() {
    let (dir, mut app, op) = project("simultaneous");
    assert!(app.strategies().get("simultaneous_finishing").is_ok());
    let mut operation = app.project().operation(op).unwrap().clone();
    operation.strategy = "simultaneous_finishing".into();
    app.execute(Command::Replace(Item::Operation(operation)))
        .unwrap();
    let result = app.regenerate().unwrap();
    let group = &result.toolpath.groups[0];
    let cutting: Vec<_> = group
        .segments
        .iter()
        .filter(|s| s.kind == MoveKind::Cutting)
        .collect();
    assert!(cutting.len() > 500, "{}", cutting.len());
    assert!(group.segments.iter().all(|s| s.end.tip.is_finite()));
    // The part is a mesh, and the notes say the tolerance is judged on it.
    assert!(
        result.notes.iter().any(|n| n.contains("against the mesh")),
        "{:?}",
        result.notes
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn req_op_04_req_op_01_helical_finishing_runs_from_a_project_with_the_builtin_strategy() {
    let (dir, mut app, op) = project("helical");
    // The library's own strategy needs no registration.
    assert!(app.strategies().get("helical_finishing").is_ok());
    let mut operation = app.project().operation(op).unwrap().clone();
    operation.strategy = "helical_finishing".into();
    app.execute(Command::Replace(Item::Operation(operation)))
        .unwrap();
    let first = app.regenerate().unwrap();
    let group = &first.toolpath.groups[0];
    // The tool's default stepover for oak (0.8 mm) is the pitch: the cube is
    // 10 mm long, so A turns 360 * 10 / 0.8 = 4500 degrees along it.
    let cutting: Vec<_> = group
        .segments
        .iter()
        .filter(|s| s.kind == MoveKind::Cutting)
        .collect();
    assert!(cutting.len() > 500, "{}", cutting.len());
    let span = cutting.last().unwrap().end.a.0 - group.start.a.0;
    assert!((span - 4500.0).abs() < 1e-6, "{span}");
    assert!(cutting.iter().all(|s| s.contact_feed.unwrap().0 == 400.0));
    assert!(group.segments.iter().all(|s| s.end.tip.is_finite()));

    // Stock to leave raises the tip off the cube's faces by that much.
    let flat_tip = cutting
        .iter()
        .map(|s| s.end.tip.y.hypot(s.end.tip.z))
        .fold(f64::INFINITY, f64::min);
    app.execute(Command::SetParameter {
        operation: op,
        name: names::STOCK_TO_LEAVE.into(),
        value: Some(ParameterValue::Number(0.5)),
    })
    .unwrap();
    let left = app.regenerate().unwrap();
    let left_tip = left.toolpath.groups[0]
        .segments
        .iter()
        .filter(|s| s.kind == MoveKind::Cutting)
        .map(|s| s.end.tip.y.hypot(s.end.tip.z))
        .fold(f64::INFINITY, f64::min);
    assert!(
        (left_tip - flat_tip - 0.5).abs() < 0.02,
        "{flat_tip} {left_tip}"
    );

    // A saved and reopened project regenerates the same path at any pool size.
    app.execute(Command::SetParameter {
        operation: op,
        name: names::STOCK_TO_LEAVE.into(),
        value: None,
    })
    .unwrap();
    app.save().unwrap();
    let again = Application::open(&dir, Threads::Exactly(3)).unwrap();
    assert_eq!(again.regenerate().unwrap().toolpath, first.toolpath);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn req_ui_06_duplicate_and_reorder_are_undoable() {
    let (dir, mut app, op) = project("tree");
    let command = openchuck_app::duplicate_operation(app.project(), op).unwrap();
    app.execute(command).unwrap();
    let names: Vec<_> = app
        .project()
        .operations()
        .iter()
        .map(|o| o.name.clone())
        .collect();
    assert_eq!(names, ["Ring", "Ring (copy)"]);
    let (original, copy) = (
        &app.project().operations()[0],
        &app.project().operations()[1],
    );
    assert_ne!(copy.id, original.id);
    assert_eq!(
        (&copy.strategy, copy.tool, copy.setup, &copy.parameters),
        (
            &original.strategy,
            original.tool,
            original.setup,
            &original.parameters
        )
    );
    let copy_id = copy.id;

    // Reorder: the copy moves first.
    app.execute(Command::Move {
        id: openchuck_app::project::ItemId::Operation(copy_id),
        index: 0,
    })
    .unwrap();
    assert_eq!(app.project().operations()[0].id, copy_id);
    // Undo the move, then the duplicate.
    app.undo().unwrap();
    assert_eq!(app.project().operations()[1].id, copy_id);
    app.undo().unwrap();
    assert_eq!(app.project().operations().len(), 1);
    assert!(openchuck_app::duplicate_operation(app.project(), OperationId::new()).is_err());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn req_ui_06_disabled_operations_are_skipped_and_saved() {
    let (dir, mut app, op) = project("enable");
    app.register_strategy(Arc::new(OneRing)).unwrap();
    app.execute(openchuck_app::duplicate_operation(app.project(), op).unwrap())
        .unwrap();
    assert_eq!(app.regenerate().unwrap().toolpath.groups.len(), 2);

    // Disabled: left out of regeneration (and so of simulation and posting).
    app.execute(Command::SetOperationEnabled {
        operation: op,
        enabled: false,
    })
    .unwrap();
    let regeneration = app.regenerate().unwrap();
    assert_eq!(regeneration.toolpath.groups.len(), 1);
    assert_ne!(
        regeneration.toolpath.groups[0].tags.operation,
        op.to_string()
    );

    // Saved as one line, written only while disabled.
    app.save().unwrap();
    let file = dir.join(format!("operations/{op}.toml"));
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(text.contains("enabled = false"), "{text}");
    let reopened = Application::open(&dir, Threads::Exactly(1)).unwrap();
    assert!(!reopened.project().operation(op).unwrap().enabled);

    // Undo enables it again, and the line goes.
    app.undo().unwrap();
    assert!(app.project().operation(op).unwrap().enabled);
    app.save().unwrap();
    assert!(!std::fs::read_to_string(&file).unwrap().contains("enabled"));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn req_ui_07_form_shows_units_defaults_and_sources() {
    use openchuck_app::form::{FieldKind, Source};
    use openchuck_app::project::ParameterValue;
    let (dir, mut app, op) = project("form");
    app.register_strategy(Arc::new(OneRing)).unwrap();
    let form = app.parameter_form(op).unwrap();
    assert_eq!(form.strategy, "one_ring");
    let field = |name: &str| form.fields.iter().find(|f| f.name == name).unwrap().clone();

    // Tolerance: millimetres, above zero, the strategy's default.
    let tolerance = field(names::TOLERANCE);
    assert!(matches!(
        tolerance.kind,
        FieldKind::Number {
            unit: "mm",
            min: Some(0.0),
            min_exclusive: true,
            ..
        }
    ));
    assert_eq!(tolerance.source, Source::Default);
    assert_eq!(tolerance.set, None);
    // Feed: from the tool's cutting data for oak, in mm/min.
    let feed = field(names::FEED);
    assert_eq!(feed.source, Source::CuttingData);
    assert!(matches!(
        feed.kind,
        FieldKind::Number { unit: "mm/min", .. }
    ));
    assert!(matches!(feed.value, Some(ParameterValue::Number(v)) if v > 0.0));
    // Retract: a choice with its default.
    let retract = field(names::RETRACT);
    assert!(matches!(&retract.kind, FieldKind::Choice { options } if options.len() == 2));
    assert_eq!(
        retract.value,
        Some(ParameterValue::Text("clearance".into()))
    );

    // A value the operation sets shows as its own.
    app.set_parameter(op, names::STEPOVER, Some(ParameterValue::Number(0.5)))
        .unwrap();
    let form = app.parameter_form(op).unwrap();
    let stepover = form
        .fields
        .iter()
        .find(|f| f.name == names::STEPOVER)
        .unwrap();
    assert_eq!(stepover.source, Source::Operation);
    assert_eq!(stepover.set, Some(ParameterValue::Number(0.5)));
    assert!(
        form.notes.iter().any(|n| n.contains("overrides")),
        "{:?}",
        form.notes
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn req_ui_07_invalid_parameter_is_refused_and_leaves_the_project_unchanged() {
    use openchuck_app::project::ParameterValue;
    let (dir, mut app, op) = project("invalid");
    app.register_strategy(Arc::new(OneRing)).unwrap();
    let before = app.project().clone();
    let undo_before = app.can_undo();
    for (name, value) in [
        (names::TOLERANCE, ParameterValue::Number(0.0)), // must be above zero
        (names::TOLERANCE, ParameterValue::Number(f64::NAN)),
        (names::STOCK_TO_LEAVE, ParameterValue::Number(-1.0)),
        (names::RETRACT, ParameterValue::Text("sideways".into())),
        (names::FEED, ParameterValue::Flag(true)),
        ("no_such_parameter", ParameterValue::Number(1.0)),
    ] {
        let e = app
            .set_parameter(op, name, Some(value))
            .unwrap_err()
            .to_string();
        assert!(e.contains(name), "{name}: {e}");
        assert_eq!(app.project(), &before, "{name}");
        assert_eq!(app.can_undo(), undo_before);
    }
    // A valid value is applied as one undoable step; clearing it is allowed.
    app.set_parameter(op, names::TOLERANCE, Some(ParameterValue::Number(0.02)))
        .unwrap();
    assert_ne!(app.project(), &before);
    app.set_parameter(op, names::TOLERANCE, None).unwrap();
    app.undo().unwrap();
    app.undo().unwrap();
    assert_eq!(app.project(), &before);
    assert!(app.parameter_form(OperationId::new()).is_err());
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Adds two copies of `op` after it and returns the three IDs in order.
fn three_operations(app: &mut Application, op: OperationId) -> Vec<OperationId> {
    let original = app.project().operation(op).unwrap().clone();
    let mut ids = vec![op];
    for n in 2..=3 {
        let mut copy = original.clone();
        copy.id = OperationId::new();
        copy.name = format!("Ring {n}");
        ids.push(copy.id);
        app.execute(Command::Add {
            item: Item::Operation(copy),
            placement: Placement::End,
        })
        .unwrap();
    }
    ids
}

#[test]
fn req_nfr_04_editing_operation_n_regenerates_n_and_later_operations_only() {
    let (dir, mut app, op) = project("chain");
    app.register_strategy(Arc::new(OneRing)).unwrap();
    let ids = three_operations(&mut app, op);

    let first = app.regenerate().unwrap();
    assert_eq!(first.generated, ids);
    assert!(first.reused.is_empty());

    // Nothing changed: everything is reused and the result is identical.
    let second = app.regenerate().unwrap();
    assert!(second.generated.is_empty());
    assert_eq!(second.reused, ids);
    assert_eq!(second.toolpath, first.toolpath);

    // Edit each position in turn.
    for (n, id) in ids.iter().enumerate() {
        app.execute(Command::SetParameter {
            operation: *id,
            name: names::STOCK_TO_LEAVE.into(),
            value: Some(ParameterValue::Number(0.1 * (n as f64 + 1.0))),
        })
        .unwrap();
        let r = app.regenerate().unwrap();
        assert_eq!(r.reused, ids[..n], "edit at {n}");
        assert_eq!(r.generated, ids[n..], "edit at {n}");
    }

    // Undoing the last edit returns operation 3 to an earlier state, which the
    // cache still holds (default depth), so nothing is regenerated.
    app.undo().unwrap();
    let after_undo = app.regenerate().unwrap();
    assert!(after_undo.generated.is_empty());
    assert_eq!(after_undo.reused, ids);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn req_nfr_04_a_shared_input_or_changed_part_file_invalidates_every_operation() {
    let (dir, mut app, op) = project("shared");
    app.register_strategy(Arc::new(OneRing)).unwrap();
    let ids = three_operations(&mut app, op);
    app.regenerate().unwrap();

    // The machine is read by every operation.
    let mut machine = MachineProfile::example_x_rotary();
    machine.clearance.safe_z = Millimetres(machine.clearance.safe_z.0 + 1.0);
    app.execute(Command::SetMachine(Some(Box::new(machine))))
        .unwrap();
    assert_eq!(app.regenerate().unwrap().generated, ids);
    assert!(app.regenerate().unwrap().generated.is_empty());

    // So is the part file: a changed file is never served from the cache.
    let file = dir.join("models/cube.stl");
    let text = std::fs::read_to_string(&file).unwrap();
    std::fs::write(&file, format!("{text}\n")).unwrap();
    assert_eq!(app.regenerate().unwrap().generated, ids);

    // A failed generation is not cached.
    app.execute(Command::SetParameter {
        operation: ids[1],
        name: "no_such_parameter".into(),
        value: Some(ParameterValue::Number(1.0)),
    })
    .unwrap();
    assert!(app.regenerate().is_err());
    assert!(app.regenerate().is_err());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn req_ui_09_a_cancelled_regeneration_stops_and_a_later_one_completes() {
    let (dir, mut app, op) = project("cancel");
    app.register_strategy(Arc::new(OneRing)).unwrap();
    let ids = three_operations(&mut app, op);
    let handle = app.start_regenerate();
    handle.cancel();
    // Cancelled before or during the run: an error, never a partial result.
    let _ = handle.wait();
    let done = app.regenerate().unwrap();
    assert_eq!(done.generated.len() + done.reused.len(), ids.len());
    assert_eq!(done.toolpath.groups.len(), 3);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn req_nfr_04_the_number_of_results_kept_per_operation_is_configurable() {
    let (dir, mut app, op) = project("depth");
    app.register_strategy(Arc::new(OneRing)).unwrap();
    assert_eq!(app.cache_versions(), openchuck_app::DEFAULT_CACHE_VERSIONS);
    let set = |app: &mut Application, v: f64| {
        app.execute(Command::SetParameter {
            operation: op,
            name: names::STOCK_TO_LEAVE.into(),
            value: Some(ParameterValue::Number(v)),
        })
        .unwrap();
        app.regenerate().unwrap()
    };

    // Keeping one result: going back regenerates.
    app.set_cache_versions(1);
    set(&mut app, 0.1);
    set(&mut app, 0.2);
    assert_eq!(set(&mut app, 0.1).generated, [op]);

    // Keeping two: the previous state is still there.
    app.set_cache_versions(2);
    set(&mut app, 0.2);
    assert_eq!(set(&mut app, 0.1).reused, [op]);
    // Three states cycle through two slots, so the oldest is gone.
    set(&mut app, 0.3);
    assert_eq!(set(&mut app, 0.2).generated, [op]);

    // Lowering the number drops the older results at once.
    app.set_cache_versions(1);
    assert_eq!(set(&mut app, 0.3).generated, [op]);

    // Zero turns caching off.
    app.set_cache_versions(0);
    assert_eq!(app.regenerate().unwrap().generated, [op]);
    assert_eq!(app.regenerate().unwrap().generated, [op]);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A strategy that changes the part file while it generates, once.
#[derive(Debug)]
struct Rewriter {
    file: PathBuf,
    done: std::sync::atomic::AtomicBool,
}

impl Strategy for Rewriter {
    fn name(&self) -> &str {
        "rewriter"
    }
    fn label(&self) -> &str {
        "Rewriter"
    }

    fn description(&self) -> &str {
        "A strategy for tests."
    }
    fn schema(&self) -> ParameterSchema {
        ParameterSchema::with_common(Vec::new())
    }
    fn passes(
        &self,
        input: &StrategyInput<'_>,
        parameters: &Parameters,
        context: &JobContext<'_>,
    ) -> openchuck_path::Result<Vec<Pass>> {
        if !self.done.swap(true, std::sync::atomic::Ordering::SeqCst) {
            let text = std::fs::read_to_string(&self.file).unwrap();
            std::fs::write(&self.file, format!("{text}\n")).unwrap();
        }
        OneRing.passes(input, parameters, context)
    }
}

#[test]
fn req_nfr_04_a_run_in_which_the_part_file_changed_is_refused_and_not_cached() {
    let (dir, mut app, op) = project("rewrite");
    let file = dir.join("models/cube.stl");
    let original = std::fs::read(&file).unwrap();
    app.register_strategy(Arc::new(Rewriter {
        file: file.clone(),
        done: std::sync::atomic::AtomicBool::new(false),
    }))
    .unwrap();
    let mut operation = app.project().operation(op).unwrap().clone();
    operation.strategy = "rewriter".into();
    app.execute(Command::Replace(Item::Operation(operation)))
        .unwrap();

    // The file changes during generation: the run is refused, and nothing is
    // stored.
    let e = app.regenerate().unwrap_err().to_string();
    assert!(e.contains("changed while regenerating"), "{e}");
    // Put back as it was: nothing made from the changed file may be reused.
    std::fs::write(&file, original).unwrap();
    let again = app.regenerate().unwrap();
    assert_eq!(again.generated, [op]);
    assert!(again.reused.is_empty());
    // Now stable, so it is cached.
    assert_eq!(app.regenerate().unwrap().reused, [op]);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn req_nfr_04_a_cache_hit_is_not_mixed_with_results_from_a_changed_part_file() {
    let (dir, mut app, op) = project("mixed");
    let file = dir.join("models/cube.stl");
    app.register_strategy(Arc::new(OneRing)).unwrap();
    app.register_strategy(Arc::new(Rewriter {
        file: file.clone(),
        done: std::sync::atomic::AtomicBool::new(false),
    }))
    .unwrap();
    // Operation 1 is cached from an earlier run.
    app.regenerate().unwrap();
    let mut second = app.project().operation(op).unwrap().clone();
    second.id = OperationId::new();
    second.name = "Rewrites".into();
    second.strategy = "rewriter".into();
    app.execute(Command::Add {
        item: Item::Operation(second),
        placement: Placement::End,
    })
    .unwrap();
    // Operation 1 hits; operation 2 changes the file while it generates, so
    // the old result for operation 1 must not be returned beside it.
    let e = app.regenerate().unwrap_err().to_string();
    assert!(e.contains("changed while regenerating"), "{e}");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn req_nfr_04_disabling_an_operation_regenerates_those_after_it_and_enabling_reuses_them() {
    let (dir, mut app, op) = project("disable");
    app.register_strategy(Arc::new(OneRing)).unwrap();
    let ids = three_operations(&mut app, op);
    app.regenerate().unwrap();

    // The middle one leaves the chain: the one before is reused, the one
    // after is made again (the stock it sees has changed).
    app.execute(Command::SetOperationEnabled {
        operation: ids[1],
        enabled: false,
    })
    .unwrap();
    let r = app.regenerate().unwrap();
    assert_eq!(r.reused, [ids[0]]);
    assert_eq!(r.generated, [ids[2]]);

    // Enabled again: its own results were kept, so nothing is regenerated.
    app.execute(Command::SetOperationEnabled {
        operation: ids[1],
        enabled: true,
    })
    .unwrap();
    let r = app.regenerate().unwrap();
    assert!(r.generated.is_empty(), "{:?}", r.generated);
    assert_eq!(r.reused, ids);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn req_op_03_req_op_11_roughing_runs_from_a_project_and_takes_its_stepdown_from_the_cutting_data() {
    let (dir, mut app, op) = project("roughing");
    assert!(app.strategies().get("rotary_roughing").is_ok());
    let mut operation = app.project().operation(op).unwrap().clone();
    operation.strategy = "rotary_roughing".into();
    app.execute(Command::Replace(Item::Operation(operation.clone())))
        .unwrap();
    // The tool's default stepdown for oak (1.0 mm) is filled in.
    let resolved = parameter_values(app.project(), &operation, app.strategies()).unwrap();
    assert_eq!(
        resolved.values.get("stepdown"),
        Some(&openchuck_path::ParameterValue::Number(1.0))
    );
    let result = app.regenerate().unwrap();
    let group = &result.toolpath.groups[0];
    assert!(group.segments.iter().any(|s| s.kind == MoveKind::Cutting));
    assert!(group.segments.iter().all(|s| s.end.tip.is_finite()));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn req_op_09_rest_machining_says_when_no_earlier_operation_has_cut() {
    let (dir, mut app, op) = project("rest-alone");
    assert!(app.strategies().get("rest_machining").is_ok());
    assert!(app.strategies().get("pencil_finishing").is_ok());
    let mut operation = app.project().operation(op).unwrap().clone();
    operation.strategy = "rest_machining".into();
    app.execute(Command::Replace(Item::Operation(operation)))
        .unwrap();
    let result = app.regenerate().unwrap();
    assert!(
        result
            .notes
            .iter()
            .any(|n| n.contains("no earlier operation of this setup has cut")),
        "{:?}",
        result.notes
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn req_op_09_req_sim_06_rest_machining_after_an_operation_is_generated_on_its_stock() {
    let (dir, mut app, op) = project("rest-after");
    app.register_strategy(Arc::new(OneRing)).unwrap();
    let mut rest = app.project().operation(op).unwrap().clone();
    rest.id = OperationId::new();
    rest.name = "Rest".into();
    rest.strategy = "rest_machining".into();
    app.execute(Command::Add {
        item: Item::Operation(rest),
        placement: Placement::End,
    })
    .unwrap();
    // The rest operation is made against the stock the first one left, so
    // it does not say that nothing has cut before it.
    let first = app.regenerate().unwrap();
    assert_eq!(first.generated.len(), 2);
    assert!(
        first
            .notes
            .iter()
            .any(|n| n.contains("rest machining:") && n.contains("material left")),
        "{:?}",
        first.notes
    );
    assert!(
        first
            .notes
            .iter()
            .all(|n| !n.contains("no earlier operation")),
        "{:?}",
        first.notes
    );

    // Ordering and dependency (slice 15 item 4): editing the operation before
    // it makes the rest operation again; editing the rest operation leaves the
    // one before it alone.
    let ids: Vec<OperationId> = app.project().operations().iter().map(|o| o.id).collect();
    app.execute(Command::SetParameter {
        operation: ids[0],
        name: names::STOCK_TO_LEAVE.into(),
        value: Some(ParameterValue::Number(0.3)),
    })
    .unwrap();
    let after_first = app.regenerate().unwrap();
    assert_eq!(after_first.generated, ids);
    app.execute(Command::SetParameter {
        operation: ids[1],
        name: parameter::MIN_REMAINING.into(),
        value: Some(ParameterValue::Number(0.2)),
    })
    .unwrap();
    let after_rest = app.regenerate().unwrap();
    assert_eq!(after_rest.reused, [ids[0]]);
    assert_eq!(after_rest.generated, [ids[1]]);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// The project of [`project`] with a rest machining operation after the
/// ring: the rest operation is made against the stock the ring leaves
/// (SIM-06). Returns the two IDs in order.
fn ring_then_rest(name: &str) -> (PathBuf, Application, Vec<OperationId>) {
    let (dir, mut app, op) = project(name);
    app.register_strategy(Arc::new(OneRing)).unwrap();
    let mut rest = app.project().operation(op).unwrap().clone();
    rest.id = OperationId::new();
    rest.name = "Rest".into();
    rest.strategy = "rest_machining".into();
    app.execute(Command::Add {
        item: Item::Operation(rest),
        placement: Placement::End,
    })
    .unwrap();
    let ids = app.project().operations().iter().map(|o| o.id).collect();
    (dir, app, ids)
}

#[test]
fn req_nfr_04_req_sim_06_editing_a_later_operation_reuses_the_earlier_one_and_its_stock() {
    // Issue 239: the operation before the edited one is upstream of it. Its
    // path and the stock it leaves come from the cache: it is neither made
    // again nor run through the stock simulation again.
    let (dir, mut app, ids) = ring_then_rest("reuse-stock");
    let first = app.regenerate().unwrap();
    assert_eq!(first.generated, ids);
    // The ring's stock is needed by the rest operation; the rest operation's
    // stock is needed by nothing, so it is not simulated.
    assert_eq!(first.simulated, [ids[0]]);

    // Nothing changed: nothing is made or simulated.
    let again = app.regenerate().unwrap();
    assert_eq!(again.reused, ids);
    assert!(again.simulated.is_empty(), "{:?}", again.simulated);
    assert_eq!(again.toolpath, first.toolpath);

    app.execute(Command::SetParameter {
        operation: ids[1],
        name: parameter::MIN_REMAINING.into(),
        value: Some(ParameterValue::Number(0.2)),
    })
    .unwrap();
    let edited = app.regenerate().unwrap();
    assert_eq!(edited.reused, [ids[0]]);
    assert_eq!(edited.generated, [ids[1]]);
    assert!(edited.simulated.is_empty(), "{:?}", edited.simulated);

    // A simulation runs only the edited operation through the stock; the
    // ring's stock is taken from the cache.
    app.execute(Command::SetParameter {
        operation: ids[1],
        name: parameter::MIN_REMAINING.into(),
        value: Some(ParameterValue::Number(0.25)),
    })
    .unwrap();
    let simulated = app.simulate().unwrap();
    assert_eq!(simulated.reused, [ids[0]]);
    assert_eq!(simulated.simulated, [ids[1]]);
    let report = simulated.simulation.unwrap();
    assert_eq!(report.setups.len(), 1);
    assert_eq!(report.setups[0].operations.len(), 2);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn req_nfr_04_req_sim_06_editing_an_earlier_operation_makes_and_simulates_the_later_one_again() {
    // The rest operation reads the stock the ring leaves, so a change to the
    // ring makes it again on the new stock, and both are simulated again.
    let (dir, mut app, ids) = ring_then_rest("upstream-change");
    let first = app.simulate().unwrap();
    assert_eq!(first.simulated, ids);
    app.execute(Command::SetParameter {
        operation: ids[0],
        name: names::STOCK_TO_LEAVE.into(),
        value: Some(ParameterValue::Number(0.3)),
    })
    .unwrap();
    let after = app.simulate().unwrap();
    assert_eq!(after.generated, ids);
    assert!(after.reused.is_empty());
    assert_eq!(after.simulated, ids);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn req_nfr_04_a_cached_operation_whose_stock_was_not_kept_is_simulated_once_then_reused() {
    // A run that does not need an operation's stock leaves none in the
    // cache: generating up to the ring alone, then the whole project, runs
    // the ring through the stock once and keeps it from then on.
    let (dir, app, ids) = ring_then_rest("stock-later");
    let until = app.regenerate_until(ids[0]).unwrap();
    assert_eq!(until.generated, [ids[0]]);
    assert!(until.simulated.is_empty());
    let all = app.regenerate().unwrap();
    assert_eq!(all.reused, [ids[0]]);
    assert_eq!(all.generated, [ids[1]]);
    assert_eq!(all.simulated, [ids[0]]);
    let again = app.simulate().unwrap();
    assert_eq!(again.reused, ids);
    assert_eq!(again.simulated, [ids[1]]);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn req_gc_07_req_nfr_06_reused_paths_and_stock_give_byte_identical_output() {
    // Reusing the ring's cached path and the stock it leaves gives the same
    // G-code, notes and simulation as making everything again with the cache
    // off.
    let (dir, mut app, ids) = ring_then_rest("reuse-identical");
    app.set_post(Some(openchuck_app::PostSelection::Builtin(
        "makera-z1".into(),
    )));
    app.simulate().unwrap();
    app.execute(Command::SetParameter {
        operation: ids[1],
        name: parameter::MIN_REMAINING.into(),
        value: Some(ParameterValue::Number(0.2)),
    })
    .unwrap();
    let reused = app.simulate().unwrap();
    assert_eq!(reused.reused, [ids[0]]);
    assert_eq!(reused.simulated, [ids[1]]);
    assert!(!reused.gcode.is_empty());

    app.set_cache_versions(0);
    let fresh = app.simulate().unwrap();
    assert_eq!(fresh.generated, ids);
    assert_eq!(fresh.simulated, ids);
    assert_eq!(reused.gcode, fresh.gcode);
    assert_eq!(reused.notes, fresh.notes);
    assert_eq!(reused.toolpath, fresh.toolpath);
    assert_eq!(reused.simulation, fresh.simulation);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn req_gc_07_a_rename_after_reuse_names_the_operations_as_they_are_now() {
    // Names are not cache inputs (issue 209): a reused operation's stock
    // is reported under its current name and its setup's current name.
    let (dir, mut app, ids) = ring_then_rest("reuse-rename");
    app.simulate().unwrap();
    let mut renamed = app.project().operation(ids[0]).unwrap().clone();
    renamed.name = "Renamed ring".into();
    app.execute(Command::Replace(Item::Operation(renamed)))
        .unwrap();
    let after = app.simulate().unwrap();
    assert_eq!(after.reused, ids);
    let report = after.simulation.unwrap();
    assert_eq!(report.setups[0].operations[0].operation, "Renamed ring");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn req_nfr_04_req_sim_05_simulating_again_with_nothing_changed_does_not_compare_again() {
    // OI-627: the comparison of a setup's final stock with the part is kept
    // with the cached stock it was made from, under the same key, so a run
    // whose final stock comes from the cache does not compare it again.
    let (dir, mut app, ids) = ring_then_rest("reuse-compare");
    let setup = app.project().operation(ids[0]).unwrap().setup;
    let first = app.simulate().unwrap();
    assert_eq!(first.compared, [setup]);
    let deviation = first.simulation.as_ref().unwrap().setups[0]
        .deviation
        .clone();
    assert!(deviation.is_some(), "the cube is compared with its part");

    // Nothing changed: the same numbers, without comparing again.
    let again = app.simulate().unwrap();
    assert_eq!(again.reused, ids);
    assert!(again.simulated.is_empty(), "{:?}", again.simulated);
    assert!(again.compared.is_empty(), "{:?}", again.compared);
    assert_eq!(again.simulation, first.simulation);

    // A rename is not an input: still not compared, and named as it is now.
    let mut renamed = app.project().operation(ids[1]).unwrap().clone();
    renamed.name = "Renamed rest".into();
    app.execute(Command::Replace(Item::Operation(renamed)))
        .unwrap();
    let after_rename = app.simulate().unwrap();
    assert!(after_rename.compared.is_empty());
    let report = after_rename.simulation.unwrap();
    assert_eq!(report.setups[0].operations[1].operation, "Renamed rest");
    assert_eq!(report.setups[0].deviation, deviation);

    // A change to the last operation leaves a new stock, which is compared.
    app.execute(Command::SetParameter {
        operation: ids[1],
        name: parameter::MIN_REMAINING.into(),
        value: Some(ParameterValue::Number(0.2)),
    })
    .unwrap();
    let edited = app.simulate().unwrap();
    assert_eq!(edited.simulated, [ids[1]]);
    assert_eq!(edited.compared, [setup]);

    // Generating without a simulation compares nothing.
    assert!(app.regenerate().unwrap().compared.is_empty());

    // With the cache off every simulation compares, and gives the same
    // numbers as the kept comparison (NFR-06).
    let kept = app.simulate().unwrap();
    assert!(kept.compared.is_empty());
    app.set_cache_versions(0);
    let fresh = app.simulate().unwrap();
    assert_eq!(fresh.compared, [setup]);
    assert_eq!(fresh.simulation, kept.simulation);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn req_op_11_a_plunge_is_refused_for_a_tool_that_states_it_cannot_cut_straight_down() {
    let (dir, mut app, op) = project("centre-cutting");
    let mut operation = app.project().operation(op).unwrap().clone();
    operation.strategy = "rotary_roughing".into();
    app.execute(Command::Replace(Item::Operation(operation.clone())))
        .unwrap();
    let mut snapshot = app
        .project()
        .tools()
        .iter()
        .find(|t| Some(t.id) == operation.tool)
        .unwrap()
        .clone();
    snapshot.tool.centre_cutting = Some(false);
    app.execute(Command::Replace(Item::Tool(snapshot))).unwrap();
    let err = app.regenerate().unwrap_err().to_string();
    assert!(err.contains("cannot cut straight down"), "{err}");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn req_op_11_a_missing_stepdown_is_explained_like_the_other_cutting_data() {
    let (dir, mut app, op) = project("stepdown-missing");
    let mut operation = app.project().operation(op).unwrap().clone();
    operation.strategy = "rotary_roughing".into();
    for (name, value) in [
        (names::FEED, 400.0),
        (names::PLUNGE_FEED, 150.0),
        (names::STEPOVER, 1.0),
    ] {
        operation
            .parameters
            .insert(name.to_owned(), ParameterValue::Number(value));
    }
    app.execute(Command::Replace(Item::Operation(operation)))
        .unwrap();
    set_material(&mut app, Some("pine"));
    let text = app.regenerate().unwrap_err().to_string();
    for part in [
        "stepdown",
        "no cutting data for the material 'pine'",
        "'4 mm ball'",
    ] {
        assert!(text.contains(part), "{part}: {text}");
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn req_op_07_req_nfr_07_swarf_on_a_mesh_part_says_it_is_b_rep_only() {
    // Swarf reads rulings from exact faces, so it is B-rep only (decision
    // 0021, OI-96): on a mesh part the form says so before anything is run,
    // and generating names the part and what to do (OI-238).
    let (dir, mut app, op) = project("swarf-mesh");
    let mut operation = app.project().operation(op).unwrap().clone();
    operation.strategy = "swarf".into();
    app.execute(Command::Replace(Item::Operation(operation)))
        .unwrap();
    let form = app.parameter_form(op).unwrap();
    let note = form
        .notes
        .iter()
        .find(|n| n.contains("B-rep only"))
        .unwrap_or_else(|| panic!("no B-rep only note: {:?}", form.notes));
    assert!(note.contains("STEP") && note.contains("cube.stl"), "{note}");
    let e = app.regenerate().unwrap_err();
    assert!(matches!(e, AppError::Operation { .. }), "{e}");
    let e = e.to_string();
    assert!(
        e.contains("'Ring'") && e.contains("B-rep only") && e.contains("cube.stl"),
        "{e}"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn req_op_16_req_op_07_req_op_10_strategies_that_need_a_step_part_are_disabled_with_the_reason_on_an_stl_part()
 {
    // Decision 0174 (issue #206): on a setup whose part is a mesh, every
    // strategy whose registration says it needs a STEP part is listed
    // disabled with the reason, not hidden, and cannot be added; the others
    // stay available. The list without a setup marks none.
    let (dir, mut app, op) = project("brep-only-list");
    let setup = app.project().operation(op).unwrap().setup;
    let choices = app.strategy_choices_for(Some(setup));
    let unavailable: Vec<&str> = choices
        .iter()
        .filter(|c| c.unavailable.is_some())
        .map(|c| c.name.as_str())
        .collect();
    assert_eq!(unavailable, ["swarf", "radial_drilling"]);
    for c in &choices {
        if let Some(why) = &c.unavailable {
            assert!(
                why.starts_with("Needs a STEP model") && why.contains("cube.stl"),
                "{}: {why}",
                c.name
            );
        }
    }
    assert!(
        choices
            .iter()
            .any(|c| c.name == "edge_following" && c.unavailable.is_none())
    );
    assert_eq!(choices.len(), app.strategy_choices().len());
    assert!(
        app.strategy_choices()
            .iter()
            .all(|c| c.unavailable.is_none())
    );
    let count = app.project().operations().len();
    let e = app
        .add_operation("Walls", "swarf", setup, None)
        .unwrap_err()
        .to_string();
    assert!(
        e.contains("needs a STEP model") && e.contains("cube.stl"),
        "{e}"
    );
    assert_eq!(app.project().operations().len(), count);
    assert!(
        app.add_operation("Edges", "edge_following", setup, None)
            .is_ok()
    );
    // An operation that already uses one (made before the rule, or its part
    // replaced since) is in error at once, before anything is generated.
    let mut operation = app.project().operation(op).unwrap().clone();
    operation.strategy = "radial_drilling".into();
    app.execute(Command::Replace(Item::Operation(operation)))
        .unwrap();
    let overview = app.overview(None, &openchuck_app::JobLog::default());
    let status = &overview.items[&openchuck_app::overview::ItemRef::Operation(op)];
    assert_eq!(
        status.state,
        openchuck_app::overview::ItemState::Error,
        "{status:?}"
    );
    assert!(
        status.cause.contains("B-rep only") && status.cause.contains("cube.stl"),
        "{}",
        status.cause
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn req_op_16_ui_06_the_strategy_list_is_ordered_and_each_entry_describes_itself() {
    // Decision 0204 (issue #195): one list, roughing first, finishing next,
    // the specialist strategies last, each with its description; the same
    // text is what the operation panel shows (issue #242).
    let (dir, app, _op) = project("strategy-list");
    let choices = app.strategy_choices();
    let groups: Vec<_> = choices.iter().map(|c| c.group).collect();
    let mut sorted = groups.clone();
    sorted.sort();
    assert_eq!(groups, sorted);
    assert_eq!(choices[0].name, "rotary_roughing");
    assert_eq!(choices.last().unwrap().name, "radial_drilling");
    for c in &choices {
        assert!(!c.description.is_empty(), "{}", c.name);
        assert_eq!(app.strategy_description(&c.name), c.description);
    }
    assert_eq!(app.strategy_description("no_such_strategy"), "");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn req_op_11_req_op_03_a_new_roughing_operation_starts_with_a_stock_to_leave() {
    // Decision 0205, option B (the owner's choice, issue 223): rotary
    // roughing added with a tool starts with 10 percent of the tool's
    // diameter, stored on the operation like any value the user set. The
    // 4 mm tool gives 0.4 mm; other strategies, and an operation with no
    // tool yet, start with nothing set.
    let (dir, mut app, op) = project("roughing-default");
    let setup = app.project().operation(op).unwrap().setup;
    let tool = app.project().operation(op).unwrap().tool;
    let leave = |app: &Application, id| {
        app.project()
            .operation(id)
            .unwrap()
            .parameters
            .get("stock_to_leave")
            .cloned()
    };
    let rough = app
        .add_operation("Rough", "rotary_roughing", setup, tool)
        .unwrap();
    assert_eq!(
        leave(&app, rough),
        Some(openchuck_app::project::ParameterValue::Number(0.4))
    );
    let finish = app
        .add_operation("Finish", "helical_finishing", setup, tool)
        .unwrap();
    assert_eq!(leave(&app, finish), None);
    let no_tool = app
        .add_operation("Rough 2", "rotary_roughing", setup, None)
        .unwrap();
    assert_eq!(leave(&app, no_tool), None);
    // One undoable step: undoing removes the operation, value and all.
    app.undo().unwrap();
    assert!(app.project().operation(no_tool).is_none());
    std::fs::remove_dir_all(&dir).unwrap();
}

/// `op` turned into `strategy` with the given values.
fn set_strategy(
    app: &mut Application,
    op: OperationId,
    strategy: &str,
    values: &[(&str, ParameterValue)],
) {
    let mut operation = app.project().operation(op).unwrap().clone();
    operation.strategy = strategy.into();
    operation.parameters = values
        .iter()
        .map(|(k, v)| ((*k).to_owned(), v.clone()))
        .collect();
    app.execute(Command::Replace(Item::Operation(operation)))
        .unwrap();
}

#[test]
fn req_op_24_the_default_distance_past_the_stock_follows_the_tool() {
    use openchuck_path::extend::parameter as extend;
    let (dir, mut app, op) = project("extend-default");
    set_strategy(&mut app, op, "helical_finishing", &[]);
    // A tenth of the 4 mm ball's diameter (decision 0212 part B item 2),
    // given to the strategy and shown in the inspector as the default.
    let operation = app.project().operation(op).unwrap().clone();
    let resolved = parameter_values(app.project(), &operation, app.strategies()).unwrap();
    assert_eq!(
        resolved.values.get(extend::EXTEND_DISTANCE),
        Some(&openchuck_path::ParameterValue::Number(0.4))
    );
    let form = app.parameter_form(op).unwrap();
    let field = form
        .fields
        .iter()
        .find(|f| f.name == extend::EXTEND_DISTANCE)
        .unwrap();
    assert_eq!(field.value, Some(ParameterValue::Number(0.4)));
    assert_eq!(field.source, openchuck_app::form::Source::Default);
    // A value set on the operation wins.
    app.execute(Command::SetParameter {
        operation: op,
        name: extend::EXTEND_DISTANCE.into(),
        value: Some(ParameterValue::Number(1.5)),
    })
    .unwrap();
    let operation = app.project().operation(op).unwrap().clone();
    let resolved = parameter_values(app.project(), &operation, app.strategies()).unwrap();
    assert_eq!(
        resolved.values.get(extend::EXTEND_DISTANCE),
        Some(&openchuck_path::ParameterValue::Number(1.5))
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn req_op_24_req_nfr_12_a_path_past_the_machines_travel_is_refused() {
    use openchuck_path::extend::parameter as extend;
    use openchuck_types::kinematics::RotaryFrame;
    let (dir, mut app, op) = project("extend-travel");
    let far = [(extend::EXTEND, ParameterValue::Text(extend::FAR_END.into()))];
    set_strategy(&mut app, op, "helical_finishing", &far);
    let first = app.regenerate().unwrap();
    // The furthest the extended path goes along the axis, in machine X.
    let mut machine = app.project().machine().unwrap().clone();
    let frame = RotaryFrame::of_profile(&machine);
    let furthest = first.toolpath.groups[0]
        .segments
        .iter()
        .map(|s| frame.on_axis(Millimetres(s.end.tip.x)).x)
        .fold(f64::NEG_INFINITY, f64::max);
    // A machine whose travel stops half a millimetre short of it.
    machine.travel.max.x = furthest - 0.5;
    app.execute(Command::SetMachine(Some(Box::new(machine))))
        .unwrap();
    let e = app.regenerate().unwrap_err().to_string();
    assert!(
        e.contains("beyond the machine's travel") && e.contains("0.500 mm"),
        "{e}"
    );
    // Not extended, the same operation fits.
    set_strategy(&mut app, op, "helical_finishing", &[]);
    assert!(app.regenerate().is_ok());
    std::fs::remove_dir_all(&dir).unwrap();
}
