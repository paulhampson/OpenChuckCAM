// SPDX-License-Identifier: AGPL-3.0-or-later
//! The flute check through the application (TL-04, SET-03, NFR-07; issue
//! #530; decision record 0509): the owner's case on the public CrankShaft
//! model in 35 x 35 mm box stock with 25 mm of flute. Finishing the blank
//! is refused; roughing it first and then finishing it is not, so the check
//! makes no new refusal of a valid job on the setup's own stock.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use openchuck_app::project::id::{OperationId, SetupId, ToolId};
use openchuck_app::project::{
    Item, Operation, OrderKey, ParameterValue, Placement, Setup, ToolSnapshot,
};
use openchuck_app::setup::{Millimetres, Stock, builtin};
use openchuck_app::simulation::SimulationSettings;
use openchuck_app::{Application, Command, Threads};
use openchuck_types::geometry::{Aabb, Point3};
use openchuck_types::tool::{ControllerMapping, CuttingData, Tool, ToolShape};
use openchuck_types::units::{FeedRate, SpindleSpeed};

/// A project of the CrankShaft model in 35 x 35 mm box stock with a 3.175
/// mm flat end mill of `flute` mm and the operations given, in order.
fn crankshaft(name: &str, flute: f64, operations: &[(&str, Vec<(&str, f64)>)]) -> Application {
    let dir = std::env::temp_dir().join(format!("openchuck-flutes-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut app = Application::create(&dir, name, Threads::Exactly(2)).unwrap();
    let stl = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/stl/crankshaft.stl");
    let (model, _) = app.add_model(&stl, None).unwrap();
    let machine = builtin("makera-z1").unwrap().unwrap();
    app.execute(Command::SetMachine(Some(Box::new(machine))))
        .unwrap();
    app.execute(Command::SetSimulation(SimulationSettings {
        cell: Millimetres(1.0),
    }))
    .unwrap();
    let setup_id = SetupId::new();
    let mut setup = Setup::new(setup_id, "Setup 1");
    setup.part = Some(model);
    setup.material = Some("oak".into());
    setup.stock = Some(Stock::Box {
        bounds: Aabb {
            min: Point3::new(-1.0, -17.5, -17.5),
            max: Point3::new(41.0, 17.5, 17.5),
        },
    });
    let tool_id = ToolId::new();
    let cutting_data = BTreeMap::from([(
        "oak".to_owned(),
        CuttingData {
            spindle_speed: SpindleSpeed(12000.0),
            feed_per_tooth: Millimetres(0.02),
            plunge_feed: FeedRate(150.0),
            stepover: Millimetres(1.0),
            stepdown: Millimetres(2.0),
        },
    )]);
    let tool = ToolSnapshot {
        id: tool_id,
        order: OrderKey::default(),
        name: "3.175 mm flat".into(),
        tool: Tool {
            shape: ToolShape::FlatEnd,
            diameter: Millimetres(3.175),
            flute_length: Millimetres(flute),
            overall_length: Millimetres(flute + 13.0),
            shank_diameter: Millimetres(3.175),
            flute_count: 2,
            material: None,
            centre_cutting: Some(true),
        },
        controller: ControllerMapping {
            number: 1,
            length_offset: None,
            diameter_offset: None,
        },
        cutting_data,
    };
    let mut items = vec![Item::Setup(setup), Item::Tool(tool)];
    for (k, (strategy, parameters)) in operations.iter().enumerate() {
        items.push(Item::Operation(Operation {
            enabled: true,
            id: OperationId::new(),
            order: OrderKey::default(),
            name: format!("Op {}", k + 1),
            setup: setup_id,
            strategy: (*strategy).into(),
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
                .map(|(k, v)| ((*k).to_owned(), ParameterValue::Number(*v)))
                .chain([("adaptive_spacing".to_owned(), ParameterValue::Flag(false))])
                .collect(),
        }));
    }
    for item in items {
        app.execute(Command::Add {
            item,
            placement: Placement::End,
        })
        .unwrap();
    }
    app
}

fn finish() -> (&'static str, Vec<(&'static str, f64)>) {
    (
        "simultaneous_finishing",
        vec![("stepover", 2.5), ("tolerance", 0.1)],
    )
}

fn rough() -> (&'static str, Vec<(&'static str, f64)>) {
    (
        "rotary_roughing",
        vec![
            ("stock_to_leave", 0.3),
            ("stepdown", 4.0),
            ("stepover", 2.5),
        ],
    )
}

/// The flute check's warnings among the run's paths (decision record 0522).
fn flute_warnings(run: &openchuck_app::Regeneration) -> Vec<String> {
    run.groups
        .iter()
        .flat_map(|g| g.warnings.iter())
        .filter(|w| w.message.starts_with(openchuck_path::flutes::WARNING_LEAD))
        .map(|w| w.message.clone())
        .collect()
}

/// The flute warnings in Messages, as the user reads them.
fn flute_messages(app: &Application) -> Vec<openchuck_app::overview::Message> {
    app.overview(None, &openchuck_app::JobLog::default())
        .messages
        .into_iter()
        .filter(|m| m.text.starts_with("Flute length"))
        .collect()
}

#[test]
fn req_tl_04_req_nfr_12_finishing_the_crankshaft_blank_with_25_mm_of_flute_is_warned_by_name() {
    // The owner's case (#530): the corner of the box stands past the flutes
    // above the tip, most below the axis on the pins' sides facing it. By
    // the owner's decision of 6 Nov 2026 (decision record 0522) it is
    // made, with a warning that names the operation, the tool, the flutes
    // and the stock; never silent, never a refusal.
    let app = crankshaft("blank", 25.0, &[finish()]);
    let run = app.simulate().unwrap_or_else(|e| panic!("{e}"));
    let warnings = flute_warnings(&run);
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0].contains("25.000 mm of flute"), "{warnings:?}");
    assert!(
        warnings[0].contains("35.000 x 35.000 mm box"),
        "{warnings:?}"
    );
    let messages = flute_messages(&app);
    assert_eq!(messages.len(), 1, "{messages:?}");
    let text = &messages[0].text;
    assert!(
        text.starts_with(
            "Flute length (operation 'Op 1', tool '3.175 mm flat'): the stock stands up to "
        ),
        "{text}"
    );
    assert!(text.contains("so the shank may meet the stock"), "{text}");
    assert!(text.contains("check it before running it"), "{text}");
    assert!(!text.contains("safe"), "{text}");
    assert_eq!(
        messages[0].severity,
        openchuck_app::overview::Severity::Warning
    );
    assert!(
        messages[0].source_name.contains("Op 1"),
        "{}",
        messages[0].source_name
    );
}

#[test]
fn req_tl_04_req_set_03_roughing_then_finishing_the_crankshaft_with_25_mm_of_flute_is_not_refused()
{
    // The same tool after roughing: the roughing layers hold the Stepdown to
    // the flutes, and the finishing reads the stock roughing left, held to
    // its bound (the farthest radius left less the tip's height plus 1 mm,
    // decision record 0509): within the 25 mm of flute, so no new refusal.
    let app = crankshaft("roughed", 25.0, &[rough(), finish()]);
    let run = app.simulate().unwrap_or_else(|e| panic!("{e}"));
    assert!(!run.toolpath.groups.is_empty());
    assert!(
        flute_warnings(&run).is_empty(),
        "{:?}",
        flute_warnings(&run)
    );
}

/// The CrankShaft project with a held start and default tabs (OP-21, SET-09;
/// issue #547): box stock from X -12 to 46, 4 mm gripped at the start,
/// roughed leaving 0.3 mm, then finished by axial finishing with a tool of
/// 8 mm of flute.
fn held_and_roughed(name: &str) -> Application {
    held_and_roughed_with(name, 8.0)
}

/// [`held_and_roughed`] with `flute` mm of flute.
fn held_and_roughed_with(name: &str, flute: f64) -> Application {
    use openchuck_app::setup::{HeldRegion, Workholding};
    let rough = (
        "rotary_roughing",
        vec![
            ("stock_to_leave", 0.3),
            ("stepdown", 4.0),
            ("stepover", 2.5),
        ],
    );
    let finish = (
        "axial_finishing",
        vec![("stepover", 2.5), ("tolerance", 0.1)],
    );
    let mut app = crankshaft(name, flute, &[rough, finish]);
    let mut setup = app.project().setups().first().unwrap().clone();
    setup.stock = Some(Stock::Box {
        bounds: Aabb {
            min: Point3::new(-12.0, -17.5, -17.5),
            max: Point3::new(46.0, 17.5, 17.5),
        },
    });
    setup.workholding = Workholding {
        start: Some(HeldRegion {
            length: Millimetres(4.0),
            diameter: None,
        }),
        end: None,
        tabs: Vec::new(),
    };
    app.execute(Command::Replace(Item::Setup(setup.clone())))
        .unwrap();
    setup.workholding.tabs = app.start_default_tabs(setup.id).wait().unwrap();
    app.execute(Command::Replace(Item::Setup(setup))).unwrap();
    app
}

#[test]
fn req_tl_04_req_nfr_12_a_finish_past_the_flutes_after_roughing_is_made_with_a_warning() {
    // The owner's case (#547) on the public CrankShaft model: the stock
    // roughing left is held to a bound (OI-2025) past the 8 mm of flute.
    // Before, generation stopped there, with no operation named; now the
    // finish is made and the warning names it and its tool, says the depth
    // is a bound, and offers Finishing layers.
    let app = held_and_roughed("held-roughed");
    let run = app.simulate().unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(flute_warnings(&run).len(), 1, "{:?}", flute_warnings(&run));
    let messages = flute_messages(&app);
    assert_eq!(messages.len(), 1, "{messages:?}");
    let text = &messages[0].text;
    assert!(
        text.starts_with(
            "Flute length (operation 'Op 2', tool '3.175 mm flat'): the stock may stand up to "
        ),
        "{text}"
    );
    assert!(text.contains("(a bound, "), "{text}");
    assert!(
        text.contains("what the earlier operations in this setup left"),
        "{text}"
    );
    assert!(text.contains("or turn on Finishing layers"), "{text}");
    assert!(
        messages[0].source_name.contains("Op 2"),
        "{}",
        messages[0].source_name
    );
    // Writing the program says it too, with the operation's name.
    let checks = app.export_checks(Some(&run), None, None);
    let said = checks
        .checks
        .iter()
        .filter(|c| c.text.contains("Flute length"))
        .map(|c| c.text.clone())
        .collect::<Vec<_>>();
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(
        said[0].starts_with(
            "Operation 'Op 2' was made with a warning: Flute length (operation 'Op 2', tool"
        ),
        "{said:?}"
    );
}

#[test]
fn req_nfr_07_a_refusal_from_generation_names_its_operation() {
    // A Stepdown past the flutes is still refused (OI-153), and the refusal
    // names the operation (#547) as well as the parameter.
    let app = crankshaft(
        "named",
        3.0,
        &[(
            "rotary_roughing",
            vec![
                ("stock_to_leave", 0.3),
                ("stepdown", 4.0),
                ("stepover", 2.5),
            ],
        )],
    );
    let e = app.simulate().map(|_| ()).unwrap_err().to_string();
    assert!(
        e.starts_with("operation 'Op 1' cannot be generated: "),
        "{e}"
    );
    assert!(e.contains("3.000 mm of flute"), "{e}");
    assert!(e.contains("parameter stepdown"), "{e}");
}

#[test]
fn req_tl_04_req_nfr_12_a_finish_with_flutes_past_the_stock_in_reach_is_not_warned_of() {
    // The same job with 21 mm of flute (decision 0524): the held start's
    // corners, 24.749 mm from the axis, are out of the finish's reach, and
    // the stock in its reach, the crank's webs beside the journals, stands
    // about 18.2 mm above the tip at most. Before, the bound from the
    // farthest material anywhere warned of 27.900 mm.
    let app = held_and_roughed_with("held-21", 21.0);
    let run = app.simulate().unwrap_or_else(|e| panic!("{e}"));
    assert!(
        flute_warnings(&run).is_empty(),
        "{:?}",
        flute_warnings(&run)
    );
}
