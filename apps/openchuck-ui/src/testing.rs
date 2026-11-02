// SPDX-License-Identifier: AGPL-3.0-or-later
//! Test inputs shared by the interface's unit tests: a small project with a
//! part, a tool and an operation, built through the application.

#![cfg(test)]
#![allow(clippy::unwrap_used)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use openchuck_app::model::TransformStep;
use openchuck_app::project::id::{OperationId, SetupId, ToolId};
use openchuck_app::project::{
    Item, Operation, OrderKey, ParameterValue, Placement, Setup, ToolSnapshot,
};
use openchuck_app::setup::{MachineProfile, Millimetres, Stock};
use openchuck_app::tools::{
    ControllerMapping, CuttingData, FeedRate, SpindleSpeed, Tool, ToolShape,
};
use openchuck_app::{Application, Command, PostSelection, Threads};

/// A project with the corpus cube centred on the rotary axis, the example
/// machine, an oak cylinder of stock, a 4 mm ball tool and one helical
/// finishing operation (Face free ends off), with the LinuxCNC post chosen.
pub fn cube_project(name: &str) -> (PathBuf, Application, OperationId, SetupId) {
    let cube = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/stl/cube_ascii.stl");
    model_project(name, &std::fs::read_to_string(cube).unwrap())
}

/// [`cube_project`] with a tube in place of the cube, as the owner's carving
/// is (issue 478; its public stand-in): X -5 to 5, Ø18, with a Ø14 bore
/// along the rotary axis, which no tool on the machine can reach, in stock
/// from X -7 that ends at the tube's far end, as a faced end does.
pub fn tube_project(name: &str) -> (PathBuf, Application, OperationId, SetupId) {
    let (dir, mut app, op, setup_id) = model_project(name, &tube_stl(9.0, 7.0, -5.0, 5.0));
    let mut setup = app.project().setup(setup_id).unwrap().clone();
    setup.stock = Some(Stock::Cylinder {
        diameter: Millimetres(20.0),
        length: Millimetres(12.0),
        x_start: Millimetres(-7.0),
    });
    app.execute(Command::Replace(Item::Setup(setup))).unwrap();
    (dir, app, op, setup_id)
}

/// An ASCII STL of a closed tube along X from `x0` to `x1`, outer radius
/// `outer`, bore radius `inner`, 64 facets round.
pub fn tube_stl(outer: f64, inner: f64, x0: f64, x1: f64) -> String {
    use std::fmt::Write as _;
    let n = 64;
    let at = |x: f64, r: f64, i: usize| {
        let t = std::f64::consts::TAU * (i % n) as f64 / n as f64;
        [x, r * t.cos(), r * t.sin()]
    };
    let mut out = String::from("solid tube\n");
    for i in 0..n {
        let j = i + 1;
        let (o0i, o0j) = (at(x0, outer, i), at(x0, outer, j));
        let (o1i, o1j) = (at(x1, outer, i), at(x1, outer, j));
        let (i0i, i0j) = (at(x0, inner, i), at(x0, inner, j));
        let (i1i, i1j) = (at(x1, inner, i), at(x1, inner, j));
        for f in [
            [o0i, o0j, o1i],
            [o0j, o1j, o1i],
            [i0i, i1i, i0j],
            [i0j, i1i, i1j],
            [o0i, i0i, i0j],
            [o0i, i0j, o0j],
            [o1i, o1j, i1j],
            [o1i, i1j, i1i],
        ] {
            out.push_str(" facet normal 0 0 0\n  outer loop\n");
            for v in f {
                writeln!(out, "   vertex {} {} {}", v[0], v[1], v[2]).unwrap();
            }
            out.push_str("  endloop\n endfacet\n");
        }
    }
    out.push_str("endsolid tube\n");
    out
}

/// The project of [`cube_project`] round the model whose STL text is
/// `stl`.
fn model_project(name: &str, stl: &str) -> (PathBuf, Application, OperationId, SetupId) {
    let dir = std::env::temp_dir().join(format!("openchuck-ui-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut app = Application::create(&dir, name, Threads::Exactly(2)).unwrap();
    std::fs::create_dir_all(dir.join("models")).unwrap();
    std::fs::write(dir.join("models/cube.stl"), stl).unwrap();
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
            // Not about free ends (decision 0344): the stock past the
            // cube's ends would otherwise be faced, with its warning.
            parameters: BTreeMap::from([
                ("face_free_ends".to_owned(), ParameterValue::Flag(false)),
                ("adaptive_spacing".to_owned(), ParameterValue::Flag(false)),
            ]),
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
