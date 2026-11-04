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

#[test]
fn req_tl_04_req_nfr_07_finishing_the_crankshaft_blank_with_25_mm_of_flute_is_refused() {
    // The owner's case (#530): the corner of the box stands past the flutes
    // above the tip, most below the axis on the pins' sides facing it.
    let app = crankshaft("blank", 25.0, &[finish()]);
    let e = app.simulate().map(|_| ()).unwrap_err().to_string();
    assert!(e.contains("25.000 mm of flute"), "{e}");
    assert!(e.contains("35.000 x 35.000 mm box"), "{e}");
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
}
