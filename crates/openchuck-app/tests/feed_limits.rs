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
use openchuck_types::ir::MoveKind;
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
                flute_length: Millimetres(30.0), // reaches the stock it cuts (decision record 0509)
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
            material: None,
            parameters: free_ends_off(strategy)
                .into_iter()
                .chain(
                    parameters
                        .iter()
                        .map(|(k, v)| ((*k).to_owned(), ParameterValue::Number(*v))),
                )
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

/// The highest F word of a program on moves that move X, Y or Z (`linear`
/// true), or on moves of A alone (`linear` false), whose feed is held to A's
/// own limit (decision record 0076 item 2). A move of A alone has no X, Y or
/// Z word; a face's turn with the tool's tip on the rotary axis is one
/// (decision record 0297).
fn highest_feed(text: &str, linear: bool) -> f64 {
    text.lines()
        .filter(|l| {
            let moves_xyz = l
                .split_whitespace()
                .any(|w| w.starts_with('X') || w.starts_with('Y') || w.starts_with('Z'));
            let moves_a = l.split_whitespace().any(|w| w.starts_with('A'));
            if linear {
                moves_xyz || !moves_a
            } else {
                moves_a && !moves_xyz
            }
        })
        .filter_map(|l| l.split_whitespace().find_map(|w| w.strip_prefix('F')))
        .filter_map(|f| f.parse::<f64>().ok())
        .fold(0.0, f64::max)
}

#[test]
fn req_op_13_req_gc_09_req_gc_10_square_and_spline_parts_pass_gates_1_and_2_on_both_posts() {
    let feeds = builtin("makera-z1").unwrap().unwrap().max_feeds;
    let (limit, rotary_limit) = (feeds.linear.0, feeds.rotary.0);
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
                if highest_feed(text, true) > limit {
                    failures.push(format!(
                        "{} / {post}: F{}",
                        job.name,
                        highest_feed(text, true)
                    ));
                }
                if highest_feed(text, false) > rotary_limit {
                    let f = highest_feed(text, false);
                    failures.push(format!("{} / {post}: F{f} on A alone", job.name));
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
    let (dir, mut app) = build(&job, "no-id");
    // Build a real pre-GC-17 copy before labelling it as schema 12.
    let mut profile = app.project().machine().unwrap().clone();
    profile.min_block_ms = 0.0;
    profile.block_window = 1;
    profile.short_block_mode = openchuck_types::machine::ShortBlockMode::Off;
    for field in [
        openchuck_types::machine::ProfileField::MinBlockMs,
        openchuck_types::machine::ProfileField::BlockWindow,
        openchuck_types::machine::ProfileField::ShortBlockMode,
    ] {
        profile.provenance.remove(&field);
    }
    app.execute(Command::SetMachine(Some(Box::new(profile))))
        .unwrap();
    app.save().unwrap();
    drop(app);
    let machine = std::fs::read_to_string(dir.join("machine.toml")).unwrap();
    assert!(!machine.contains("\nid = "), "{machine}");
    let mut older: toml::Table = machine.parse().unwrap();
    older.insert("schema".into(), toml::Value::Integer(12));
    let profile = older.get_mut("machine").unwrap().as_table_mut().unwrap();
    // Build an actual schema-12 machine: newer limit/configuration fields
    // and their provenance did not exist yet (0076, 0231, 0222).
    for field in [
        "min_spindle_rpm",
        "max_spindle_rpm",
        "min_block_ms",
        "block_window",
        "short_block_mode",
        "automatic_tool_length",
        "spindle_to_axis",
        "recommended_min_safe_z",
    ] {
        profile.remove(field);
    }
    profile
        .get_mut("max_feeds")
        .unwrap()
        .as_table_mut()
        .unwrap()
        .remove("rotary_in_feed");
    if let Some(provenance) = profile
        .get_mut("provenance")
        .and_then(toml::Value::as_table_mut)
    {
        for field in [
            "spindle_range",
            "min_block_ms",
            "block_window",
            "short_block_mode",
            "tool_length_measurement",
            "spindle_to_axis",
            "recommended_min_safe_z",
        ] {
            provenance.remove(field);
        }
    }
    let older = toml::to_string_pretty(&older).unwrap();
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
        assert!(
            highest_feed(text, true) <= limit,
            "F{}",
            highest_feed(text, true)
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
}

/// Face free ends off for the strategies that have it: these tests are not
/// about free ends, and a new operation faces them (decision record 0344,
/// OI-1396).
fn free_ends_off(strategy: &str) -> BTreeMap<String, ParameterValue> {
    let mut out = BTreeMap::new();
    if ["helical_finishing", "axial_finishing", "rest_machining"].contains(&strategy) {
        out.insert("face_free_ends".to_owned(), ParameterValue::Flag(false));
    }
    // Adaptive spacing off too: not what this checks, and a new operation
    // has it on (decision record 0239).
    if [
        "helical_finishing",
        "axial_finishing",
        "indexed_finishing",
        "simultaneous_finishing",
    ]
    .contains(&strategy)
    {
        out.insert("adaptive_spacing".to_owned(), ParameterValue::Flag(false));
    }
    out
}

#[test]
fn req_nfr_12_req_gc_07_req_ui_12_the_feed_summary_matches_the_per_move_notes_by_operation_id() {
    // Issue 432 and 418: the run holds one summary per slowed operation, keyed
    // by id, counting exactly the per-move notes, which stay as they were.
    let mut seen = 0;
    for job in jobs().into_iter().filter(|j| !j.name.contains("rough")) {
        let (dir, app) = build(&job, "summary");
        let run = app.regenerate().unwrap();
        for (id, limit) in &run.feed_limits {
            let name = &app.project().operation(*id).unwrap().name;
            let prefix = format!("Feed limit (operation '{name}'): ");
            let notes = run
                .notes
                .iter()
                .filter(|n| {
                    n.starts_with(&prefix) && openchuck_app::note_groups::is_feed_limit_note(n)
                })
                .count();
            assert_eq!(limit.moves, notes, "{}", job.name);
            assert!(limit.worst_feed < limit.worst_target, "{}", job.name);
            assert!(limit.lowest <= limit.worst_feed, "{}", job.name);
            seen += 1;
        }
        let again = app.regenerate().unwrap();
        assert_eq!(again.feed_limits, run.feed_limits, "{}", job.name);
        std::fs::remove_dir_all(dir).unwrap();
    }
    assert!(seen > 0, "no job was slowed by the machine's feed limits");
}

/// A move's words in a program line: the letter and its number.
fn word(line: &str, letter: char) -> Option<f64> {
    line.split_whitespace()
        .find_map(|w| w.strip_prefix(letter))
        .and_then(|v| v.parse().ok())
}

/// The Makera Z1's time in seconds for one fed move of the program, as its
/// firmware works it out: the F word counts A's travel round the part,
/// `max(L, dA * P / 360) / F`, or `dA * min(P, 360) / 360 / F` for A alone,
/// with `P = 2 pi r + 30` and `r` the end point's distance from the axis
/// (decision record 0007), and then no axis goes faster than its own limit:
/// X and Y 1200 mm/min, Z 600 and A `a_limit` deg/min (`Robot.cpp`, the
/// rate scaling of each actuator; decision records 0013 and 0647). Returns
/// the time and the A rate the F word alone asks for, in deg/min.
fn z1_firmware_seconds(from: [f64; 4], to: [f64; 4], f: f64, a_limit: f64) -> (f64, f64) {
    let d: Vec<f64> = from.iter().zip(&to).map(|(a, b)| (b - a).abs()).collect();
    let line = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    let p = std::f64::consts::TAU * to[1].hypot(to[2]) + 30.0;
    let counted = if line > 0.0 {
        line.max(d[3] * p / 360.0)
    } else {
        d[3] * p.min(360.0) / 360.0
    };
    let rule = 60.0 * counted / f;
    let axes = 60.0 * (d[0] / 1200.0).max(d[1] / 1200.0).max(d[2] / 600.0).max(d[3] / a_limit);
    (rule.max(axes), 60.0 * d[3] / rule)
}

/// The Z1's controller turns A at most 30 deg/s, 1800 deg/min (firmware
/// `Robot.cpp:295-298`; the owner's `M203.1` on firmware 1.1.3 prints
/// `A: 30`; decision record 0647). Helical finishing of the box in steps of
/// at most 5 degrees rolls the ball round its corners, where the contact feed would turn
/// A faster than that: each such move is timed at 1800 deg/min (GC-04,
/// GC-09), its F word asks the firmware for no more, and the time estimate
/// is what the firmware's rule gives for the written program (GC-06, GC-12).
/// With the old 3600 deg/min the F words asked A for up to 3600 and the
/// estimate was short.
#[test]
fn req_gc_04_req_gc_09_req_gc_06_req_gc_12_z1_moves_turn_a_at_most_1800_deg_per_min_and_are_timed_so()
 {
    let a_limit = builtin("makera-z1").unwrap().unwrap().max_feeds.rotary.0;
    assert_eq!(a_limit, 1800.0);
    let corner = 10.0_f64.hypot(15.0);
    let job = Job {
        name: "box_helical_5",
        mesh: prism(&rectangle(0.0, 0), 0.0, 20.0),
        radius: corner,
        operations: vec![("helical_finishing", vec![("max_angle_step", 5.0)])],
    };
    let (dir, mut app) = build(&job, "a-limit");
    app.set_post(Some(PostSelection::parse("makera-z1")));
    let result = app.regenerate().unwrap();

    // The IR: no move turns A faster than its limit, and the moves whose
    // contact feed asked for more are held at exactly the limit.
    let (mut asked, mut held, mut fed_seconds) = (0, 0, 0.0);
    for group in &result.toolpath.groups {
        let mut a = group.start.a.0;
        for s in &group.segments {
            let da = (s.end.a.0 - a).abs();
            a = s.end.a.0;
            if s.kind.is_fed() {
                fed_seconds += s.duration.0;
            }
            if da < 1e-9 {
                continue;
            }
            let at_limit = 60.0 * da / a_limit;
            assert!(
                s.duration.0 >= at_limit * (1.0 - 1e-9),
                "{:?}: {da} deg in {} s",
                s.kind,
                s.duration.0
            );
            if let (MoveKind::Cutting, Some(f)) = (s.kind, s.contact_feed) {
                if 60.0 * s.contact_length.0 / f.0 < at_limit {
                    asked += 1;
                    if (s.duration.0 - at_limit).abs() <= 1e-6 * at_limit {
                        held += 1;
                    }
                }
            }
        }
    }
    assert!(asked > 0 && held > 0, "asked {asked}, held {held}");

    // The written program, timed as the firmware times it.
    let (_, text) = &result.gcode[0];
    let (mut pos, mut f, mut fed) = ([0.0_f64; 4], 0.0, false);
    let (mut z1_seconds, mut steps, mut fastest) = (0.0, 0, 0.0_f64);
    for line in text.lines().filter(|l| !l.starts_with('(')) {
        if line.contains("G92.4") {
            pos[3] = word(line, 'A').unwrap_or(pos[3]);
            continue;
        }
        if line.contains("G28") || line.contains("G53") {
            break;
        }
        if line.contains("G0") {
            fed = false;
        }
        if line.contains("G1") {
            fed = true;
        }
        f = word(line, 'F').unwrap_or(f);
        let mut to = pos;
        for (i, letter) in ['X', 'Y', 'Z', 'A'].into_iter().enumerate() {
            to[i] = word(line, letter).unwrap_or(to[i]);
        }
        if fed && to != pos {
            let (seconds, a_rate) = z1_firmware_seconds(pos, to, f, a_limit);
            z1_seconds += seconds;
            // A helical step (at most 5 degrees; 2.5 where the path is
            // refined): the F word asks for no more than A's limit, to the
            // written decimals (F to 0.5 %, A to 0.001 deg).
            let da = (to[3] - pos[3]).abs();
            if da >= 1.0 {
                steps += 1;
                fastest = fastest.max(a_rate);
                assert!(a_rate <= a_limit * 1.006, "{line}: A at {a_rate:.0} deg/min");
            }
        }
        pos = to;
    }
    assert!(steps > 100, "{steps} helical steps");
    assert!(fastest > 0.99 * a_limit, "fastest A {fastest:.0} deg/min");
    // The estimate is the firmware's time for the written program.
    let off = (z1_seconds - fed_seconds) / fed_seconds;
    assert!(
        off.abs() < 0.005,
        "the Z1 takes {z1_seconds:.1} s, estimated {fed_seconds:.1} s"
    );
    std::fs::remove_dir_all(dir).unwrap();
}
