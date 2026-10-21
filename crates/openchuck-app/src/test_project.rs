// SPDX-License-Identifier: AGPL-3.0-or-later
//! A small project for the crate's own tests of the pipeline and its cache:
//! the corpus cube centred on the rotary axis, the example machine, an oak
//! cylinder of stock, a ball tool with oak cutting data, and operations added
//! by the test.

#![allow(clippy::unwrap_used)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use openchuck_project::id::{OperationId, SetupId, ToolId};
use openchuck_project::{Item, Operation, OrderKey, Placement, Setup, ToolSnapshot};
use openchuck_types::machine::MachineProfile;
use openchuck_types::stock::Stock;
use openchuck_types::tool::{ControllerMapping, CuttingData, Tool, ToolShape};
use openchuck_types::transform::TransformStep;
use openchuck_types::units::{FeedRate, Millimetres, SpindleSpeed};

use crate::{Application, Command, Threads};

/// The project, its folder (removed when dropped) and the setup and tool.
pub(crate) struct TestProject {
    pub(crate) dir: PathBuf,
    pub(crate) app: Application,
    pub(crate) setup: SetupId,
    pub(crate) tool: ToolId,
}

impl Drop for TestProject {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

impl TestProject {
    pub(crate) fn new(name: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("openchuck-app-unit-{name}-{}", std::process::id()));
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
        let setup = SetupId::new();
        let mut s = Setup::new(setup, "Setup 1");
        s.part = Some(model);
        s.material = Some("oak".into());
        s.stock = Some(Stock::Cylinder {
            diameter: Millimetres(20.0),
            length: Millimetres(14.0),
            x_start: Millimetres(-7.0),
        });
        let tool = ToolId::new();
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
            id: tool,
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
        for item in [Item::Setup(s), Item::Tool(snapshot)] {
            app.execute(Command::Add {
                item,
                placement: Placement::End,
            })
            .unwrap();
        }
        Self {
            dir,
            app,
            setup,
            tool,
        }
    }

    /// Adds an operation of `strategy` at the end and returns its ID.
    pub(crate) fn add_operation(&mut self, name: &str, strategy: &str) -> OperationId {
        let id = OperationId::new();
        self.app
            .execute(Command::Add {
                item: Item::Operation(Operation {
                    enabled: true,
                    id,
                    order: OrderKey::default(),
                    name: name.into(),
                    setup: self.setup,
                    strategy: strategy.into(),
                    tool: Some(self.tool),
                    selections: Vec::new(),
                    boundary: None,
                    avoid: Vec::new(),
                    check: Vec::new(),
                    parameters: BTreeMap::new(),
                }),
                placement: Placement::End,
            })
            .unwrap();
        id
    }
}
