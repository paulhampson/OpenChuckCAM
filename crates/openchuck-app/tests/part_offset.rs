// SPDX-License-Identifier: AGPL-3.0-or-later
//! The setup's part offset ("Offset along X") moves the part along the
//! rotary axis against the stock, the workholding and the work zero, and the
//! toolpaths, the simulated stock and the G-code follow it (SET-05, NFR-12;
//! GitHub issue #175; decision record 0201). A zero offset changes nothing,
//! byte for byte (GC-07, NFR-06).
//!
//! The check is independent of how the offset is applied: a setup offset of
//! `d` must give exactly what moving the model by `d` in its own placement
//! steps gives, since both put the same part at the same place against the
//! same stock and work zero.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_cmp
)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use openchuck_app::model::TransformStep;
use openchuck_app::project::id::{ModelId, OperationId, SetupId, ToolId};
use openchuck_app::project::{
    Item, Operation, OrderKey, ParameterValue, Placement, Setup, ToolSnapshot,
};
use openchuck_app::setup::{MachineProfile, Millimetres, Stock};
use openchuck_app::simulation::SimulationSettings;
use openchuck_app::{Application, Command, PostSelection, Regeneration, Threads};
use openchuck_testkit::parts::{Cone, RadialPart};
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::tool::{ControllerMapping, CuttingData, Tool, ToolShape};
use openchuck_types::units::{FeedRate, SpindleSpeed};

/// How far the setup moves the part, in millimetres.
const OFFSET: f64 = 12.5;

fn write_stl(mesh: &TriangleMesh, path: &Path) {
    let mut text = String::from("solid part\n");
    for t in &mesh.triangles {
        text.push_str("facet normal 0 0 0\n outer loop\n");
        for i in t {
            let p = mesh.vertices[*i as usize];
            text.push_str(&format!("  vertex {:e} {:e} {:e}\n", p.x, p.y, p.z));
        }
        text.push_str(" endloop\nendfacet\n");
    }
    text.push_str("endsolid part\n");
    std::fs::write(path, text).unwrap();
}

struct Built {
    dir: PathBuf,
    app: Application,
    setup: SetupId,
    model: ModelId,
}

impl Drop for Built {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// A cone (X 0 to 30) in a long oak cylinder of stock (X -20 to 60), on the
/// example machine with the LinuxCNC post, roughed with a 3 mm ball.
fn build(name: &str) -> Built {
    let dir = std::env::temp_dir().join(format!("openchuck-offset-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut app = Application::create(&dir, name, Threads::Exactly(2)).unwrap();
    std::fs::create_dir_all(dir.join("models")).unwrap();
    let cone = Cone {
        start_radius: 9.0,
        end_radius: 5.0,
        length: 30.0,
    };
    write_stl(&cone.mesh(31, 36), &dir.join("models/part.stl"));
    let (model, _) = app.add_model(&dir.join("models/part.stl"), None).unwrap();
    app.execute(Command::SetMachine(Some(Box::new(
        MachineProfile::example_x_rotary(),
    ))))
    .unwrap();
    app.execute(Command::SetSimulation(SimulationSettings {
        cell: Millimetres(0.5),
    }))
    .unwrap();
    app.set_post(Some(PostSelection::parse("linuxcnc")));
    let setup_id = SetupId::new();
    let mut setup = Setup::new(setup_id, "Setup 1");
    setup.part = Some(model);
    setup.material = Some("oak".into());
    setup.stock = Some(Stock::Cylinder {
        diameter: Millimetres(24.0),
        length: Millimetres(80.0),
        x_start: Millimetres(-20.0),
    });
    let tool_id = ToolId::new();
    let mut cutting_data = BTreeMap::new();
    cutting_data.insert(
        "oak".to_owned(),
        CuttingData {
            spindle_speed: SpindleSpeed(12000.0),
            feed_per_tooth: Millimetres(0.02),
            plunge_feed: FeedRate(150.0),
            stepover: Millimetres(2.0),
            stepdown: Millimetres(2.0),
        },
    );
    let snapshot = ToolSnapshot {
        id: tool_id,
        order: OrderKey::default(),
        name: "3 mm ball".into(),
        tool: Tool {
            shape: ToolShape::BallEnd,
            diameter: Millimetres(3.0),
            flute_length: Millimetres(8.0),
            overall_length: Millimetres(40.0),
            shank_diameter: Millimetres(3.0),
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
    let operation = Operation {
        enabled: true,
        id: OperationId::new(),
        order: OrderKey::default(),
        name: "Rough".into(),
        setup: setup_id,
        strategy: "rotary_roughing".into(),
        tool: Some(tool_id),
        selections: Vec::new(),
        boundary: None,
        avoid: Vec::new(),
        check: Vec::new(),
        restriction_options: Default::default(),
        spindle_speed: None,
        parameters: [("stock_to_leave".to_owned(), ParameterValue::Number(0.5))]
            .into_iter()
            .collect(),
    };
    for item in [
        Item::Setup(setup),
        Item::Tool(snapshot),
        Item::Operation(operation),
    ] {
        app.execute(Command::Add {
            item,
            placement: Placement::End,
        })
        .unwrap();
    }
    Built {
        dir,
        app,
        setup: setup_id,
        model,
    }
}

impl Built {
    fn set_offset(&mut self, offset: f64) {
        let mut setup = self.app.project().setup(self.setup).unwrap().clone();
        setup.frame.part_offset = Millimetres(offset);
        self.app
            .execute(Command::Replace(Item::Setup(setup)))
            .unwrap();
    }

    fn set_placement(&mut self, transform: Vec<TransformStep>) {
        self.app
            .execute(Command::SetModelTransform {
                model: self.model,
                transform,
            })
            .unwrap();
    }
}

fn gcode(run: &Regeneration) -> &str {
    assert_eq!(run.gcode.len(), 1, "one setup, one file");
    &run.gcode[0].1
}

#[test]
fn req_set_05_req_gc_07_the_part_offset_moves_the_toolpath_the_simulation_and_the_g_code() {
    let mut b = build("moves");
    let still = b.app.simulate().unwrap();

    // The setup moves the part by OFFSET.
    b.set_offset(OFFSET);
    let moved = b.app.simulate().unwrap();

    // The same part moved by its own placement instead, with no setup offset.
    b.set_offset(0.0);
    b.set_placement(vec![TransformStep::Translate {
        x: OFFSET,
        y: 0.0,
        z: 0.0,
    }]);
    let placed = b.app.simulate().unwrap();

    // The offset reaches the toolpath, the simulated stock and the G-code,
    // and in each it agrees exactly with moving the part by placement.
    assert_ne!(
        gcode(&moved),
        gcode(&still),
        "the offset changes the G-code"
    );
    assert_ne!(moved.toolpath, still.toolpath);
    assert_eq!(moved.toolpath, placed.toolpath);
    assert_eq!(gcode(&moved), gcode(&placed));
    let (m, p, s) = (
        &moved.simulation.as_ref().unwrap().setups[0],
        &placed.simulation.as_ref().unwrap().setups[0],
        &still.simulation.as_ref().unwrap().setups[0],
    );
    assert_eq!(m.stock, p.stock, "the simulated stock follows the part");
    assert_eq!(m.deviation, p.deviation);
    assert_ne!(m.stock, s.stock);
    // The stock itself stays put: the same material before cutting.
    assert_eq!(m.initial_volume, s.initial_volume);
    // The cut follows the part: the part's far end (X 30 unmoved) is now at
    // X 42.5, so the tool reaches further along the axis by the offset.
    let reach = |run: &Regeneration| {
        run.toolpath
            .groups
            .iter()
            .flat_map(|g| &g.segments)
            .filter(|s| s.kind.is_fed() && s.end.tip.y.hypot(s.end.tip.z) < 10.0)
            .map(|s| s.end.tip.x)
            .fold(f64::NEG_INFINITY, f64::max)
    };
    assert!(
        reach(&moved) > reach(&still) + OFFSET - 2.0,
        "{} vs {}",
        reach(&moved),
        reach(&still)
    );
}

#[test]
fn req_gc_07_req_nfr_06_a_zero_offset_leaves_the_g_code_byte_identical() {
    let mut b = build("zero");
    let before = b.app.regenerate().unwrap();
    for zero in [0.0, -0.0] {
        b.set_offset(zero);
        let again = b.app.regenerate().unwrap();
        assert_eq!(again.toolpath, before.toolpath);
        assert_eq!(gcode(&again).as_bytes(), gcode(&before).as_bytes());
    }
}
