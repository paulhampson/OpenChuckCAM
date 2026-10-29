// SPDX-License-Identifier: AGPL-3.0-or-later
//! The material preset of an operation: the tool's cutting data for one
//! stock material, chosen in New operation and in the operation inspector
//! (issue 197; TL-05, TL-07, TL-09, NFR-07, GC-07; decision record 0230).

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
use openchuck_app::preset::{PresetState, operation_preset, preset_for, set_operation_preset};
use openchuck_app::project::id::{OperationId, SetupId, ToolId};
use openchuck_app::project::{
    Item, Operation, OrderKey, ParameterValue, Placement, Setup, ToolSnapshot,
};
use openchuck_app::setup::{MachineProfile, Millimetres, Stock};
use openchuck_app::tool_library::{operation_spindle, set_operation_spindle};
use openchuck_app::{Application, Command, Threads};
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
            helical_pass_start: false,
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
    let dir = std::env::temp_dir().join(format!("openchuck-preset-{name}-{}", std::process::id()));
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
    cutting_data.insert(
        "Steel".to_owned(),
        CuttingData {
            spindle_speed: SpindleSpeed(6000.0),
            feed_per_tooth: Millimetres(0.01),
            plunge_feed: FeedRate(60.0),
            stepover: Millimetres(0.3),
            stepdown: Millimetres(0.5),
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
    app.register_strategy(Arc::new(OneRing)).unwrap();
    (dir, app, op)
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

fn preset(app: &Application, op: OperationId) -> openchuck_app::preset::MaterialPreset {
    operation_preset(app.project(), app.strategies(), op).unwrap()
}

fn choose(app: &mut Application, op: OperationId, material: Option<&str>) {
    let c = set_operation_preset(app.project(), app.strategies(), op, material).unwrap();
    app.execute(c).unwrap();
}

fn number(app: &Application, op: OperationId, name: &str) -> Option<f64> {
    match app.project().operation(op).unwrap().parameters.get(name) {
        Some(ParameterValue::Number(n)) => Some(*n),
        _ => None,
    }
}

fn set_setup_material(app: &mut Application, material: Option<&str>) {
    let mut setup = app.find_setup("Setup 1").unwrap().clone();
    setup.material = material.map(str::to_owned);
    app.execute(Command::Replace(Item::Setup(setup))).unwrap();
}

/// The preset defaults to the setup's stock material and offers only the
/// materials the tool has cutting data for (the owner's answer 1 on issue
/// 197).
#[test]
fn req_tl_05_req_tl_09_the_preset_defaults_to_the_setups_material_and_offers_the_tools() {
    let (dir, app, op) = project("default");
    let p = preset(&app, op);
    assert_eq!(p.state, PresetState::Applied);
    assert_eq!(p.material.as_deref(), Some("oak"));
    assert_eq!((p.chosen, p.differs, p.message), (None, None, None));
    let materials: Vec<Option<&str>> = p.choices.iter().map(|c| c.material.as_deref()).collect();
    // The setup's material once, first; then the tool's others.
    assert_eq!(materials, [None, Some("Steel")]);
    assert!(p.choices[0].label.contains("oak"), "{}", p.choices[0].label);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A material chosen for the operation fills its cutting values and is
/// marked when it is not the setup's; choosing the setup's material again
/// stores nothing (answer 1).
#[test]
fn req_tl_05_a_chosen_material_fills_the_values_and_is_marked_when_not_the_setups() {
    let (dir, mut app, op) = project("chosen");
    // 10000 x 2 x 0.02 for oak.
    assert_eq!(first_cut_feed(&app), 400.0);
    choose(&mut app, op, Some(" steel "));
    let stored = app.project().operation(op).unwrap().material.clone();
    assert_eq!(stored.as_deref(), Some("Steel"), "the tool's spelling");
    // 6000 x 2 x 0.01 for steel; the spindle speed follows too.
    assert_eq!(first_cut_feed(&app), 120.0);
    let spindle = operation_spindle(app.project(), op).unwrap();
    assert_eq!(spindle.from_tool, Some((6000.0, "Steel".to_owned())));
    let p = preset(&app, op);
    assert_eq!(p.state, PresetState::Applied);
    let why = p.differs.unwrap();
    assert!(why.contains("Steel") && why.contains("oak"), "{why}");
    // The setup's material, by its own entry or by name, stores none.
    for setup in [None, Some("OAK")] {
        choose(&mut app, op, Some("Steel"));
        choose(&mut app, op, setup);
        assert_eq!(app.project().operation(op).unwrap().material, None);
        assert_eq!(first_cut_feed(&app), 400.0);
        assert!(preset(&app, op).differs.is_none());
    }
    // A chosen material stays the operation's when the setup's changes.
    choose(&mut app, op, Some("Steel"));
    set_setup_material(&mut app, None);
    let p = preset(&app, op);
    assert_eq!(p.state, PresetState::Applied);
    assert!(p.differs.unwrap().contains("no stock material"));
    assert_eq!(first_cut_feed(&app), 120.0);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A tool with no cutting data for the material applies no preset and says
/// so, naming the tool and the material; the operation's values are left as
/// they are, and a material the tool lacks cannot be chosen (answer 2;
/// NFR-07).
#[test]
fn req_tl_05_req_nfr_07_a_tool_without_data_for_the_material_applies_no_preset_and_says_so() {
    let (dir, mut app, op) = project("no-data");
    set_setup_material(&mut app, Some("Softwood"));
    let p = preset(&app, op);
    assert_eq!(p.state, PresetState::NoData);
    let message = p.message.unwrap();
    for part in ["\"4 mm ball\"", "\"Softwood\"", "no preset", "tool library"] {
        assert!(message.contains(part), "{part}: {message}");
    }
    // Its own spindle speed stays when the setup's material is chosen again.
    let c = set_operation_spindle(app.project(), app.strategies(), op, Some("9000")).unwrap();
    app.execute(c).unwrap();
    choose(&mut app, op, None);
    assert_eq!(
        operation_spindle(app.project(), op).unwrap().set,
        Some(9000.0)
    );
    let before = app.project().clone();
    let e = set_operation_preset(app.project(), app.strategies(), op, Some("Brass"))
        .unwrap_err()
        .to_string();
    assert!(e.contains("Brass") && e.contains("no cutting data"), "{e}");
    assert_eq!(*app.project(), before);
    // No material at all says where to set one.
    set_setup_material(&mut app, None);
    let p = preset(&app, op);
    assert_eq!(p.state, PresetState::NoMaterial);
    assert!(p.message.unwrap().contains("stock material"));
    assert!(operation_preset(app.project(), app.strategies(), OperationId::new()).is_none());
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Editing a cutting value drops the preset: the values it gave become the
/// operation's own, so the toolpath does not change, and the operation shows
/// no preset; choosing a material applies its preset again (answer 3).
#[test]
fn req_tl_05_editing_a_cutting_value_drops_the_preset_and_keeps_its_values() {
    let (dir, mut app, op) = project("drop");
    choose(&mut app, op, Some("Steel"));
    app.set_parameter(op, names::STEPOVER, Some(ParameterValue::Number(0.25)))
        .unwrap();
    let p = preset(&app, op);
    assert_eq!(p.state, PresetState::Dropped);
    assert!(p.message.unwrap().contains("No preset"));
    let edited = app.project().operation(op).unwrap().clone();
    assert_eq!(edited.spindle_speed, Some(SpindleSpeed(6000.0)));
    assert_eq!(number(&app, op, names::STEPOVER), Some(0.25));
    assert_eq!(number(&app, op, names::FEED), Some(120.0));
    assert_eq!(number(&app, op, names::PLUNGE_FEED), Some(60.0));
    assert_eq!(first_cut_feed(&app), 120.0);
    // One undoable step.
    assert!(app.undo().unwrap());
    assert_eq!(preset(&app, op).state, PresetState::Applied);
    assert!(app.redo().unwrap());
    // Changing the tool's data no longer moves the operation's values.
    let mut tool = app.project().tools()[0].clone();
    tool.cutting_data.get_mut("Steel").unwrap().plunge_feed = FeedRate(99.0);
    app.execute(Command::Replace(Item::Tool(tool))).unwrap();
    assert_eq!(number(&app, op, names::PLUNGE_FEED), Some(60.0));
    // Choosing a material again clears the operation's own cutting values.
    choose(&mut app, op, Some("Steel"));
    let again = app.project().operation(op).unwrap().clone();
    assert_eq!(again.spindle_speed, None);
    for name in [names::FEED, names::PLUNGE_FEED, names::STEPOVER] {
        assert!(!again.parameters.contains_key(name), "{name}");
    }
    assert_eq!(preset(&app, op).state, PresetState::Applied);
    // Clearing a value (↺) is not an edit and copies nothing.
    app.set_parameter(op, names::FEED, None).unwrap();
    assert_eq!(preset(&app, op).state, PresetState::Applied);
    // A parameter that is not a cutting value leaves the preset alone.
    app.set_parameter(op, names::TOLERANCE, Some(ParameterValue::Number(0.02)))
        .unwrap();
    assert_eq!(preset(&app, op).state, PresetState::Applied);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A new spindle speed drops the preset too, and the feed it copies is
/// worked out at the new speed, so the chip load stays the preset's
/// (decision record 0196 item 2).
#[test]
fn req_tl_05_a_spindle_speed_edit_drops_the_preset_and_keeps_the_chip_load() {
    let (dir, mut app, op) = project("speed");
    let c = set_operation_spindle(app.project(), app.strategies(), op, Some("5000")).unwrap();
    app.execute(c).unwrap();
    assert_eq!(preset(&app, op).state, PresetState::Dropped);
    // 5000 x 2 x 0.02 for oak.
    assert_eq!(number(&app, op, names::FEED), Some(200.0));
    assert_eq!(number(&app, op, names::STEPOVER), Some(0.8));
    assert_eq!(first_cut_feed(&app), 200.0);
    // Taking the speed away (↺) copies nothing.
    let c = set_operation_spindle(app.project(), app.strategies(), op, None).unwrap();
    app.execute(c).unwrap();
    assert_eq!(app.project().operation(op).unwrap().spindle_speed, None);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// New operation takes the preset in its one undoable step, and refuses a
/// material its tool has no data for (TL-07: the material comes from the
/// project's copy of the tool).
#[test]
fn req_tl_05_req_ui_06_a_new_operation_takes_its_preset_in_one_step() {
    let (dir, mut app, op) = project("new");
    let existing = app.project().operation(op).unwrap().clone();
    let shown = preset_for(app.project(), existing.setup, existing.tool, Some("Steel"));
    assert_eq!(shown.state, PresetState::Applied);
    assert!(shown.differs.is_some());
    let none = preset_for(app.project(), existing.setup, None, None);
    assert_eq!(none.state, PresetState::NoTool);
    let new = app
        .add_operation_on(
            "Steel ring",
            "one_ring",
            existing.setup,
            existing.tool,
            None,
            Some("steel"),
        )
        .unwrap();
    assert_eq!(
        app.project().operation(new).unwrap().material.as_deref(),
        Some("Steel")
    );
    assert!(app.undo().unwrap());
    assert!(app.project().operation(new).is_none());
    let count = app.project().operations().len();
    let e = app
        .add_operation_on(
            "Brass",
            "one_ring",
            existing.setup,
            existing.tool,
            None,
            Some("Brass"),
        )
        .unwrap_err()
        .to_string();
    assert!(e.contains("Brass"), "{e}");
    assert_eq!(app.project().operations().len(), count);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// An operation with no chosen material machines and posts as before: the
/// setup's material gives its values, so no G-code changes (GC-07).
#[test]
fn req_gc_07_an_operation_with_the_setups_preset_regenerates_as_before() {
    let (dir, mut app, op) = project("same");
    let before = app.regenerate().unwrap().toolpath.clone();
    choose(&mut app, op, None);
    assert_eq!(app.regenerate().unwrap().toolpath, before);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A tool changed to one without the chosen material keeps the choice in
/// the list and applies no preset, and a tool with no cutting data at all
/// says what to add (answer 2).
#[test]
fn req_tl_05_a_new_tool_without_the_chosen_material_applies_no_preset() {
    let (dir, mut app, op) = project("tool-change");
    choose(&mut app, op, Some("Steel"));
    let mut bare = app.project().tools()[0].clone();
    bare.id = ToolId::new();
    bare.name = "Bare".into();
    bare.controller.number = 2;
    bare.cutting_data.clear();
    let bare_id = bare.id;
    app.execute(Command::Add {
        item: Item::Tool(bare),
        placement: Placement::End,
    })
    .unwrap();
    let c = openchuck_app::tool_library::set_operation_tool(app.project(), op, bare_id).unwrap();
    app.execute(c).unwrap();
    let p = preset(&app, op);
    assert_eq!(p.state, PresetState::NoData);
    assert!(
        p.choices
            .iter()
            .any(|c| c.material.as_deref() == Some("Steel")),
        "the choice is still listed"
    );
    let message = p.message.unwrap();
    assert!(
        message.contains("\"Bare\" has no cutting data for \"Steel\"")
            && message.contains("Add cutting data"),
        "{message}"
    );
    choose(&mut app, op, None);
    set_setup_material(&mut app, None);
    let p = preset(&app, op);
    assert_eq!(p.state, PresetState::NoMaterial);
    assert!(p.message.unwrap().contains("\"Bare\" has no cutting data"));
    std::fs::remove_dir_all(&dir).unwrap();
}
