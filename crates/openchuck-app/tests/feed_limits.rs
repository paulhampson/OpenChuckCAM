// SPDX-License-Identifier: AGPL-3.0-or-later
//! The feed word stays within the machine's limits and the written feed
//! keeps each move's time (OP-13, GC-09, GC-10, GC-07, NFR-06; decision
//! record 0076). Lane Z's STEP strategy tests found both defects (OI-236);
//! neither is STEP-specific, so these jobs are meshes of the same corpus
//! shapes: the box (20 by 30, 40 long), the filleted box (its long edges
//! rounded to 3 mm; 12 long, to keep roughing short) and the revolved
//! spline. Each is machined with a 3 mm
//! ball in oak on the Makera Z1 profile, as the reference jobs are, and
//! posted with both shipped posts.
//!
//! - Where the ball rolls round a corner of a square part, A turns at its
//!   limit while the tip hardly moves. The Z1 counts A's travel round the
//!   part in its F word (decision record 0007), so the word went above the
//!   linear limit: gate 1 found `F1432.7` against 1200 here (lane Z,
//!   `F1426` on the STL box).
//! - Moves that are almost all A have a small F on LinuxCNC, and one written
//!   decimal of `F4.2` is over 1 % of the move's time: gate 2 found 1.04 to
//!   3.95 %.
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
use openchuck_types::geometry::Point3;
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

/// A closed prism along X from `x0` to `x1` whose section is `outline`, a
/// convex polygon `(y, z)` about the axis in counter-clockwise order.
fn prism(outline: &[(f64, f64)], x0: f64, x1: f64) -> TriangleMesh {
    let n = u32::try_from(outline.len()).unwrap();
    let mut vertices = Vec::new();
    for x in [x0, x1] {
        vertices.extend(outline.iter().map(|&(y, z)| Point3::new(x, y, z)));
    }
    let mut triangles = Vec::new();
    for j in 0..n {
        let k = (j + 1) % n;
        triangles.push([j, n + k, n + j]);
        triangles.push([j, k, n + k]);
    }
    for (ring, x, outward_positive) in [(0, x0, false), (n, x1, true)] {
        let centre = u32::try_from(vertices.len()).unwrap();
        vertices.push(Point3::new(x, 0.0, 0.0));
        for j in 0..n {
            let k = (j + 1) % n;
            if outward_positive {
                triangles.push([centre, ring + j, ring + k]);
            } else {
                triangles.push([centre, ring + k, ring + j]);
            }
        }
    }
    TriangleMesh {
        vertices,
        triangles,
    }
}

/// A 20 by 30 rectangle about the axis, its corners rounded to `fillet`
/// (none at 0) with `steps` segments each.
fn rectangle(fillet: f64, steps: u32) -> Vec<(f64, f64)> {
    let (hy, hz) = (10.0, 15.0);
    let mut out = Vec::new();
    for (cy, cz, start) in [
        (hy - fillet, hz - fillet, 0.0),
        (-(hy - fillet), hz - fillet, 90.0),
        (-(hy - fillet), -(hz - fillet), 180.0),
        (hy - fillet, -(hz - fillet), 270.0),
    ] {
        if fillet == 0.0 {
            out.push((cy, cz));
            continue;
        }
        for i in 0..=steps {
            let a = (start + 90.0 * f64::from(i) / f64::from(steps)).to_radians();
            out.push((cy + fillet * a.cos(), cz + fillet * a.sin()));
        }
    }
    out
}

/// The revolved spline's profile `(x, radius)`: a Catmull-Rom curve through
/// the corpus fixture's points (radius 8, 14, 10, 13 and 9 at 0, 8, 18, 28
/// and 40 mm; `testdata/step/generate`), sampled every half millimetre.
fn spline_profile() -> Vec<(f64, f64)> {
    let p = [
        (0.0, 8.0),
        (8.0, 14.0),
        (18.0, 10.0),
        (28.0, 13.0),
        (40.0, 9.0),
    ];
    let at = |i: isize| p[i.clamp(0, 4) as usize];
    let mut out = Vec::new();
    for i in 0..4_isize {
        let (p0, p1, p2, p3) = (at(i - 1), at(i), at(i + 1), at(i + 2));
        let steps = ((p2.0 - p1.0) * 2.0) as u32;
        for s in 0..steps {
            let t = f64::from(s) / f64::from(steps);
            let c = |a: f64, b: f64, c: f64, d: f64| {
                0.5 * (2.0 * b
                    + (c - a) * t
                    + (2.0 * a - 5.0 * b + 4.0 * c - d) * t * t
                    + (3.0 * b - a - 3.0 * c + d) * t * t * t)
            };
            out.push((c(p0.0, p1.0, p2.0, p3.0), c(p0.1, p1.1, p2.1, p3.1)));
        }
    }
    out.push(p[4]);
    out
}

/// One job: a part, the widest the part reaches from the axis, and its
/// operations in order.
struct Job {
    name: &'static str,
    mesh: TriangleMesh,
    radius: f64,
    operations: Vec<(&'static str, Vec<(&'static str, f64)>)>,
}

fn jobs() -> Vec<Job> {
    let corner = 10.0_f64.hypot(15.0);
    vec![
        Job {
            name: "box_helical",
            mesh: prism(&rectangle(0.0, 0), 0.0, 40.0),
            radius: corner,
            operations: vec![("helical_finishing", vec![("max_angle_step", 10.0)])],
        },
        Job {
            name: "filleted_box_helical",
            mesh: prism(&rectangle(3.0, 8), 0.0, 12.0),
            radius: corner,
            operations: vec![("helical_finishing", vec![("max_angle_step", 10.0)])],
        },
        Job {
            name: "spline_simultaneous",
            mesh: revolve(&spline_profile(), 144),
            radius: 14.0,
            operations: vec![("simultaneous_finishing", vec![("max_angle_step", 10.0)])],
        },
        Job {
            name: "filleted_box_rough_rest",
            mesh: prism(&rectangle(3.0, 8), 0.0, 12.0),
            radius: corner,
            operations: vec![
                ("rotary_roughing", vec![("stock_to_leave", 0.5)]),
                ("rest_machining", vec![("max_angle_step", 10.0)]),
            ],
        },
    ]
}

fn build(job: &Job, tag: &str) -> (PathBuf, Application) {
    let dir = std::env::temp_dir().join(format!(
        "openchuck-feed-{tag}-{}-{}",
        job.name,
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let mut app = Application::create(&dir, job.name, Threads::Exactly(2)).unwrap();
    std::fs::create_dir_all(dir.join("models")).unwrap();
    write_stl(&job.mesh, &dir.join("models/part.stl"));
    let (model, _) = app.add_model(&dir.join("models/part.stl"), None).unwrap();
    let machine = builtin("makera-z1").unwrap().unwrap();
    app.execute(Command::SetMachine(Some(Box::new(machine))))
        .unwrap();
    let (x0, x1) = job
        .mesh
        .vertices
        .iter()
        .fold((f64::MAX, f64::MIN), |(lo, hi), v| {
            (lo.min(v.x), hi.max(v.x))
        });
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
            stepdown: Millimetres(2.0),
        },
    );
    let mut items = vec![
        Item::Setup(setup),
        Item::Tool(ToolSnapshot {
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
                centre_cutting: Some(true),
            },
            controller: ControllerMapping {
                number: 1,
                length_offset: None,
                diameter_offset: None,
            },
            cutting_data,
        }),
    ];
    for (i, (strategy, parameters)) in job.operations.iter().enumerate() {
        items.push(Item::Operation(Operation {
            enabled: true,
            id: OperationId::new(),
            order: OrderKey::default(),
            name: format!("{strategy} {i}"),
            setup: setup_id,
            strategy: (*strategy).into(),
            tool: Some(tool_id),
            selections: Vec::new(),
            boundary: None,
            avoid: Vec::new(),
            check: Vec::new(),
            restriction_options: Default::default(),
            spindle_speed: None,
            parameters: parameters
                .iter()
                .map(|(k, v)| ((*k).to_owned(), ParameterValue::Number(*v)))
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
    (dir, app)
}

/// The highest F word on a cutting move (`G1`) of a program.
fn highest_feed(text: &str) -> f64 {
    text.lines()
        .filter_map(|l| l.split_whitespace().find_map(|w| w.strip_prefix('F')))
        .filter_map(|f| f.parse::<f64>().ok())
        .fold(0.0, f64::max)
}

#[test]
fn req_op_13_req_gc_09_req_gc_10_square_and_spline_parts_pass_gates_1_and_2_on_both_posts() {
    let limit = builtin("makera-z1").unwrap().unwrap().max_feeds.linear.0;
    let mut failures = Vec::new();
    for job in jobs() {
        let (dir, mut app) = build(&job, "gates");
        for post in ["makera-z1", "linuxcnc"] {
            app.set_post(Some(PostSelection::parse(post)));
            let result = app
                .regenerate()
                .unwrap_or_else(|e| panic!("{} / {post}: {e}", job.name));
            // Before decision 0076: on the Z1, `F1432.7` against 1200 on the
            // box and `F1360.8` on the filleted box; on LinuxCNC, lane Z
            // found moves 1 to 4 % off the IR's time on its corpus parts.
            for report in result.reports.iter().filter(|r| !r.is_clean()) {
                failures.push(format!(
                    "{} / {post}: {:?} {:?}",
                    job.name,
                    &report.findings[..report.findings.len().min(3)],
                    report.comparison
                ));
            }
            for (_, text) in &result.gcode {
                if highest_feed(text) > limit {
                    failures.push(format!("{} / {post}: F{}", job.name, highest_feed(text)));
                }
            }
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn req_gc_07_req_nfr_06_feed_limited_jobs_give_identical_gcode_at_1_2_and_n_threads() {
    // Roughing is left out to keep the test short: the bound is worked out
    // per move, and roughing's own determinism tests cover the rest.
    for job in jobs().into_iter().filter(|j| !j.name.contains("rough")) {
        let (dir, mut app) = build(&job, "threads");
        let mut first = Vec::new();
        for post in ["makera-z1", "linuxcnc"] {
            app.set_post(Some(PostSelection::parse(post)));
            first.push(app.regenerate().unwrap().gcode);
        }
        app.save().unwrap();
        for threads in openchuck_testkit::determinism::thread_counts() {
            let mut again = Application::open(&dir, Threads::Exactly(threads)).unwrap();
            for (post, first) in ["makera-z1", "linuxcnc"].into_iter().zip(&first) {
                again.set_post(Some(PostSelection::parse(post)));
                assert_eq!(
                    &again.regenerate().unwrap().gcode,
                    first,
                    "{} / {post} at {threads}",
                    job.name
                );
            }
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn req_op_13_req_gc_09_req_prj_04_an_older_z1_copy_without_an_id_keeps_the_feed_word_within_the_limit()
 {
    // `Command::SetMachine` stores the profile with no library ID. Before
    // schema 13 such a Z1 copy had no `rotary_in_feed`, and the migration
    // knew the Z1 only by its ID, so the box's corners wrote `F1432.7` on the
    // Z1 again (OI-253; decision records 0076 item 3 and 0109).
    let limit = builtin("makera-z1").unwrap().unwrap().max_feeds.linear.0;
    let job = jobs()
        .into_iter()
        .find(|j| j.name == "box_helical")
        .unwrap();
    let (dir, app) = build(&job, "no-id");
    app.save().unwrap();
    drop(app);
    let machine = std::fs::read_to_string(dir.join("machine.toml")).unwrap();
    assert!(!machine.contains("\nid = "), "{machine}");
    let older: String = machine
        .lines()
        .filter(|l| !l.starts_with("rotary_in_feed"))
        .map(|l| {
            if l.starts_with("schema = ") {
                "schema = 12\n".to_owned()
            } else {
                format!("{l}\n")
            }
        })
        .collect();
    std::fs::write(dir.join("machine.toml"), older).unwrap();

    let mut app = Application::open(&dir, Threads::Exactly(2)).unwrap();
    app.set_post(Some(PostSelection::parse("makera-z1")));
    let result = app.regenerate().unwrap();
    for report in &result.reports {
        assert!(
            report.is_clean(),
            "{:?} {:?}",
            &report.findings[..report.findings.len().min(3)],
            report.comparison
        );
    }
    for (_, text) in &result.gcode {
        assert!(highest_feed(text) <= limit, "F{}", highest_feed(text));
    }
    std::fs::remove_dir_all(dir).unwrap();
}
