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
        Ok(vec![Pass { points }])
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

    // A material the tool has no data for is named, with the tool, and
    // names must match exactly.
    for material in ["pine", "Steel"] {
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
            "exactly",
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
