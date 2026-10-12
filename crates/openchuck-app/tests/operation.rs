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
            id: op,
            order: OrderKey::default(),
            name: "Ring".into(),
            setup: setup_id,
            strategy: "one_ring".into(),
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
