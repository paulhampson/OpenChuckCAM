// SPDX-License-Identifier: AGPL-3.0-or-later
//! Test inputs shared by the interface's unit tests: a small project with a
//! part, a tool and an operation, built through the application.

#![cfg(test)]
#![allow(clippy::unwrap_used)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use openchuck_app::model::TransformStep;
use openchuck_app::project::id::{OperationId, SetupId, ToolId};
use openchuck_app::project::{Item, Operation, OrderKey, Placement, Setup, ToolSnapshot};
use openchuck_app::setup::{MachineProfile, Millimetres, Stock};
use openchuck_app::tools::{
    ControllerMapping, CuttingData, FeedRate, SpindleSpeed, Tool, ToolShape,
};
use openchuck_app::{Application, Command, PostSelection, Threads};

/// A project with the corpus cube centred on the rotary axis, the example
/// machine, an oak cylinder of stock, a 4 mm ball tool and one helical
/// finishing operation, with the LinuxCNC post chosen.
pub fn cube_project(name: &str) -> (PathBuf, Application, OperationId, SetupId) {
    let dir = std::env::temp_dir().join(format!("openchuck-ui-{name}-{}", std::process::id()));
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
        tool: Tool {
            shape: ToolShape::BallEnd,
            diameter: Millimetres(4.0),
            flute_length: Millimetres(10.0),
            overall_length: Millimetres(40.0),
            shank_diameter: Millimetres(4.0),
            flute_count: 2,
            material: None,
            centre_cutting: None,
        },
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
            name: "Finish".into(),
            setup: setup_id,
            strategy: "helical_finishing".into(),
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
    app.set_post(Some(PostSelection::parse("linuxcnc")));
    (dir, app, op, setup_id)
}
