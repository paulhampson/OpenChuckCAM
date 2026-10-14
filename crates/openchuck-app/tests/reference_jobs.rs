// SPDX-License-Identifier: AGPL-3.0-or-later
//! The reference job set for the G-code gates (verification plan section 6;
//! work breakdown slice 6 item 12; decision record 0020 item 8): three small
//! analytic parts, each machined by helical finishing with a 3 mm ball in oak
//! on the Makera Z1 profile. Every job posts with both shipped posts, passes
//! gates 1 and 2, and matches its golden file. Gate 3 replays the LinuxCNC
//! programs in the simulator; gates 4 and 5 are run on the machine by the
//! operator (slice 20).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_cmp
)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use openchuck_app::project::id::{OperationId, SetupId, ToolId};
use openchuck_app::project::{
    Item, Operation, OrderKey, ParameterValue, Placement, Setup, ToolSnapshot,
};
use openchuck_app::setup::{Millimetres, Stock, builtin};
use openchuck_app::{Application, Command, PostSelection, Threads};
use openchuck_testkit::parts::{Cone, HelicalGroove, RadialPart, Sphere};
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::tool::{ControllerMapping, CuttingData, Tool, ToolShape};
use openchuck_types::units::{FeedRate, SpindleSpeed};

struct Job {
    name: &'static str,
    part: Box<dyn RadialPart>,
    radius: f64,
}

fn jobs() -> Vec<Job> {
    vec![
        Job {
            name: "groove",
            part: Box::new(HelicalGroove {
                radius: 12.0,
                length: 30.0,
                pitch: 12.0,
                depth: 1.5,
            }),
            radius: 12.0,
        },
        Job {
            name: "cone",
            part: Box::new(Cone {
                start_radius: 10.0,
                end_radius: 5.0,
                length: 30.0,
            }),
            radius: 10.0,
        },
        Job {
            name: "sphere",
            part: Box::new(Sphere { radius: 10.0 }),
            radius: 10.0,
        },
    ]
}

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

fn build(job: &Job, tag: &str) -> (PathBuf, Application) {
    let dir = std::env::temp_dir().join(format!(
        "openchuck-ref-{tag}-{}-{}",
        job.name,
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let mut app = Application::create(&dir, job.name, Threads::Exactly(2)).unwrap();
    std::fs::create_dir_all(dir.join("models")).unwrap();
    let (x0, x1) = job.part.x_range();
    let mesh = job.part.mesh(121, 144);
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
        diameter: Millimetres(2.0 * job.radius + 2.0),
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
        },
        controller: ControllerMapping {
            number: 1,
            length_offset: None,
            diameter_offset: None,
        },
        cutting_data,
    };
    let mut parameters = BTreeMap::new();
    parameters.insert("max_angle_step".to_owned(), ParameterValue::Number(5.0));
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

fn golden(name: &str, text: &str) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/golden/reference")
        .join(name);
    openchuck_testkit::golden::check(&path, text).unwrap_or_else(|m| panic!("{m}"));
}

#[test]
fn req_gc_09_req_gc_10_req_nfr_08_reference_jobs_pass_gates_1_and_2_and_match_their_golden_files() {
    for job in jobs() {
        let (dir, mut app) = build(&job, "gates");
        for (post, ext) in [("makera-z1", "nc"), ("linuxcnc", "ngc")] {
            app.set_post(Some(PostSelection::parse(post)));
            let result = app.regenerate().unwrap();
            let report = &result.reports[0];
            assert!(
                report.is_clean(),
                "{} / {post}: {:?} {:?}",
                job.name,
                &report.findings[..report.findings.len().min(5)],
                report.comparison
            );
            let (_, text) = &result.gcode[0];
            // The title names the project, which is fixed; the operation id is not, so
            // the golden file keeps the operation comment's id out.
            let stable: String = text
                .lines()
                .map(|l| {
                    if l.starts_with("(operation ") {
                        "(operation <id>)"
                    } else {
                        l
                    }
                })
                .map(|l| format!("{l}\n"))
                .collect();
            golden(&format!("{}_{post}.{ext}", job.name), &stable);
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn req_gc_07_reference_jobs_are_identical_at_1_and_n_threads() {
    for job in jobs() {
        let (dir, mut app) = build(&job, "threads");
        app.set_post(Some(PostSelection::parse("makera-z1")));
        let first = app.regenerate().unwrap().gcode;
        app.save().unwrap();
        for threads in [1, 4] {
            let mut again = Application::open(&dir, Threads::Exactly(threads)).unwrap();
            again.set_post(Some(PostSelection::parse("makera-z1")));
            assert_eq!(
                again.regenerate().unwrap().gcode,
                first,
                "{} at {threads}",
                job.name
            );
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
}
