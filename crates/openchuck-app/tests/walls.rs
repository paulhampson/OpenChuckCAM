// SPDX-License-Identifier: AGPL-3.0-or-later
//! Walls in the target pass the G-code gates (OP-13, OP-14, GC-07, GC-09,
//! NFR-06; decision record 0050, closing OI-209). A wall is a step in the
//! target the tool climbs or drops, as where a retaining tab is narrower
//! than the part's end face or a held region is wider than the part; here
//! the walls are in the part's mesh, so the job runs on any setup. Helical
//! finishing with a 3 mm ball in oak on the Makera Z1 profile, as the
//! reference jobs.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use openchuck_app::project::id::{OperationId, SetupId, ToolId};
use openchuck_app::project::{
    Item, Operation, OrderKey, ParameterValue, Placement, Setup, ToolSnapshot,
};
use openchuck_app::setup::{Millimetres, Stock, builtin};
use openchuck_app::{Application, Command, PostSelection, Threads};
use openchuck_testkit::parts::revolve;
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::tool::{ControllerMapping, CuttingData, Tool, ToolShape};
use openchuck_types::units::{FeedRate, SpindleSpeed};

fn write_stl(mesh: &TriangleMesh, path: &Path) {
    let mut text = String::from("solid part\n");
    for t in &mesh.triangles {
        let v: Vec<_> = t.iter().map(|i| mesh.vertices[*i as usize]).collect();
        let n = {
            let (a, b) = (v[1] - v[0], v[2] - v[0]);
            let c = a.cross(b);
            let len = c.length();
            if len > 0.0 { c * (1.0 / len) } else { c }
        };
        text.push_str(&format!(
            "facet normal {:e} {:e} {:e}\n outer loop\n",
            n.x, n.y, n.z
        ));
        for p in v {
            text.push_str(&format!("  vertex {:e} {:e} {:e}\n", p.x, p.y, p.z));
        }
        text.push_str(" endloop\nendfacet\n");
    }
    text.push_str("endsolid part\n");
    std::fs::write(path, text).unwrap();
}

fn build(name: &str, profile: &[(f64, f64)], radius: f64) -> (PathBuf, Application) {
    let dir = std::env::temp_dir().join(format!(
        "openchuck-walls-{name}-{}-{}",
        std::process::id(),
        std::thread::current()
            .name()
            .unwrap_or("main")
            .replace("::", "-")
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let mut app = Application::create(&dir, name, Threads::Exactly(2)).unwrap();
    std::fs::create_dir_all(dir.join("models")).unwrap();
    let x0 = profile.first().unwrap().0;
    let x1 = profile.last().unwrap().0;
    let mesh = revolve(profile, 144);
    write_stl(&mesh, &dir.join("models/part.stl"));
    let (model, _) = app.add_model(&dir.join("models/part.stl"), None).unwrap();
    let machine = builtin("makera-z1").unwrap().unwrap();
    app.execute(Command::SetMachine(Some(Box::new(machine))))
        .unwrap();
    let setup_id = SetupId::new();
    let mut setup = Setup::new(setup_id, "Setup 1");
    setup.part = Some(model);
    setup.material = Some("oak".into());
    setup.stock = Some(Stock::Cylinder {
        diameter: Millimetres(2.0 * radius + 2.0),
        length: Millimetres(x1 - x0 + 2.0),
        x_start: Millimetres(x0 - 1.0),
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
            stepdown: Millimetres(1.0),
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
    let parameters: BTreeMap<String, ParameterValue> =
        [("max_angle_step".to_owned(), ParameterValue::Number(5.0))].into();
    for item in [
        Item::Setup(setup),
        Item::Tool(snapshot),
        Item::Operation(Operation {
            enabled: true,
            id: OperationId::new(),
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
            parameters,
        }),
    ] {
        app.execute(Command::Add {
            item,
            placement: Placement::End,
        })
        .unwrap();
    }
    (dir, app)
}

/// Parts with walls, as meshes of revolution `(x, radius)` about X: the
/// layout of the owner's default tabs at the cone's wide end (held region,
/// a frustum tab narrowing to the part, a climb of 2.5 mm onto its end
/// face), and at its narrow end a drop of 2.5 mm onto a narrower tab and a
/// climb of 5 mm onto a held region.
fn parts() -> [(&'static str, Vec<(f64, f64)>); 2] {
    [
        (
            "head",
            vec![
                (-12.0, 10.0),
                (-8.0, 10.0),
                (0.0, 5.0),
                (0.0, 10.0),
                (30.0, 5.0),
            ],
        ),
        (
            "tail",
            vec![
                (0.0, 10.0),
                (30.0, 5.0),
                (30.0, 2.5),
                (38.0, 5.0),
                (38.0, 10.0),
                (40.0, 10.0),
            ],
        ),
    ]
}

#[test]
fn req_op_13_req_op_14_req_gc_09_walls_pass_gates_1_and_2() {
    for (name, profile) in parts() {
        let (dir, mut app) = build(name, &profile, 10.0);
        for post in ["makera-z1", "linuxcnc"] {
            app.set_post(Some(PostSelection::parse(post)));
            let result = app.regenerate().unwrap();
            let report = &result.reports[0];
            // Before the micro-moves at a wall were merged, gate 1 found
            // `F600.1` against the Z limit of 600 and gate 2 a move whose
            // time was 2 to 7 % off the IR's (OI-209).
            assert!(
                report.is_clean(),
                "{name} / {post}: {:?} {:?}",
                &report.findings[..report.findings.len().min(5)],
                report.comparison
            );
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn req_gc_07_req_nfr_06_walls_give_identical_gcode_at_1_2_and_n_threads() {
    for (name, profile) in parts() {
        let (dir, mut app) = build(name, &profile, 10.0);
        app.set_post(Some(PostSelection::parse("makera-z1")));
        let first = app.regenerate().unwrap().gcode;
        app.save().unwrap();
        for threads in openchuck_testkit::determinism::thread_counts() {
            let mut again = Application::open(&dir, Threads::Exactly(threads)).unwrap();
            again.set_post(Some(PostSelection::parse("makera-z1")));
            assert_eq!(
                again.regenerate().unwrap().gcode,
                first,
                "{name} at {threads}"
            );
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
}
