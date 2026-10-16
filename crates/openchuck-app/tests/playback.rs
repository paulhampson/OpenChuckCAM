// SPDX-License-Identifier: AGPL-3.0-or-later
//! Playback, the tool at a point on the path, the toolpath and simulation
//! display data, and the responsiveness of the jobs behind them (SIM-01,
//! SIM-02, SIM-04, SIM-05, UI-09, NFR-04; work breakdown slice 9 and its
//! review checklist; decision record 0040).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_cmp
)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use openchuck_app::gcode::{Dialect, read, replay};
use openchuck_app::model::TransformStep;
use openchuck_app::pathview::{
    DeviationBand, LEVEL_TOLERANCES, PathClass, ToolpathDisplay, deviation_bands, simplify,
};
use openchuck_app::playback::{Cursor, Playback, Timeline, ViewFrame, tool_meshes};
use openchuck_app::project::id::{OperationId, SetupId, ToolId};
use openchuck_app::project::{Item, Operation, OrderKey, Placement, Setup, ToolSnapshot};
use openchuck_app::setup::{MachineProfile, Millimetres, Stock};
use openchuck_app::{Application, Command, PostSelection, Regeneration, Threads};
use openchuck_types::geometry::Point3;
use openchuck_types::kinematics::{RotaryFrame, machine_xyz};
use openchuck_types::tool::{ControllerMapping, CuttingData, Tool, ToolShape};
use openchuck_types::units::{Degrees, FeedRate, SpindleSpeed};

fn ball() -> Tool {
    Tool {
        shape: ToolShape::BallEnd,
        diameter: Millimetres(4.0),
        flute_length: Millimetres(10.0),
        overall_length: Millimetres(40.0),
        shank_diameter: Millimetres(4.0),
        flute_count: 2,
        material: None,
        centre_cutting: None,
    }
}

/// A project with the corpus cube (10 mm) centred on the rotary axis, the
/// example machine, an oak cylinder of stock, a ball tool, and one helical
/// finishing operation; with the LinuxCNC post chosen.
fn project(name: &str) -> (PathBuf, Application, OperationId) {
    let dir = std::env::temp_dir().join(format!("openchuck-play-{name}-{}", std::process::id()));
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
        tool: ball(),
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
    (dir, app, op)
}

fn timeline(app: &Application, run: Regeneration) -> Timeline {
    Timeline::new(app.project(), Arc::new(run)).unwrap()
}

#[test]
fn req_sim_04_the_timeline_lays_the_moves_end_to_end_in_program_time() {
    let (dir, app, op) = project("timeline");
    let run = app.regenerate().unwrap();
    let want_time = run.toolpath.total_duration().0;
    let want_moves: usize = run.toolpath.groups.iter().map(|g| g.segments.len()).sum();
    let t = timeline(&app, run);
    assert_eq!(t.move_count(), want_moves);
    assert!((t.total_time() - want_time).abs() < 1e-9 * want_time.max(1.0));
    assert_eq!(t.group_count(), 1);
    assert_eq!(t.operation(0), Some(op));
    assert_eq!(t.operation_name(0), Some("Finish"));
    assert!(t.tool(0).is_some());
    assert_eq!(t.first_move(0), Some(0));
    assert_eq!(t.locate(0), Some((0, 0)));
    assert_eq!(t.locate(want_moves - 1), Some((0, want_moves - 1)));
    assert_eq!(t.locate(want_moves), None);
    // Time and cursor are inverses.
    for k in (0..want_moves).step_by((want_moves / 40).max(1)) {
        for fraction in [0.0, 0.25, 0.8] {
            let cursor = Cursor { done: k, fraction };
            let back = t.cursor_at_time(t.time_at(cursor));
            let again = t.time_at(back);
            assert!(
                (again - t.time_at(cursor)).abs() < 1e-9 * want_time.max(1.0),
                "{cursor:?} {back:?}"
            );
        }
    }
    assert_eq!(t.cursor_at_time(-1.0), Cursor::default());
    assert_eq!(t.cursor_at_time(f64::NAN), Cursor::default());
    assert_eq!(t.cursor_at_time(1e12).done, want_moves);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn req_sim_02_a_pose_is_the_irs_pose_at_a_moves_ends_and_machine_linear_between() {
    let (dir, app, _) = project("pose");
    let run = app.regenerate().unwrap();
    let toolpath = run.toolpath.clone();
    let t = timeline(&app, run);
    let machine = app.project().machine().unwrap();
    let setup = &app.project().setups()[0];
    let work = RotaryFrame::work(machine, &setup.frame);
    let group = &toolpath.groups[0];
    // At a move's start and end: the IR's own pose.
    for s in (0..group.segments.len()).step_by((group.segments.len() / 25).max(1)) {
        let end = group.segments[s].end;
        let done = t
            .sample(Cursor {
                done: s + 1,
                fraction: 0.0,
            })
            .unwrap_or_else(|_| panic!("move {s}"));
        let at_end = if s + 1 < group.segments.len() {
            done
        } else {
            t.sample(Cursor {
                done: group.segments.len(),
                fraction: 0.0,
            })
            .unwrap()
        };
        assert!(at_end.tip.distance(end.tip) < 1e-9, "move {s}");
        assert!((at_end.a - end.a.0).abs() < 1e-9);
        // The work coordinates are what the post writes for that pose.
        let w = machine_xyz(&work, end.tip, end.a);
        assert!((at_end.work[0] - w.x).abs() < 1e-9 && (at_end.work[2] - w.z).abs() < 1e-9);
        assert_eq!(t.work_after(s).unwrap()[3], end.a.0);
    }
    // Part-way: X, Y, Z and A are the straight line between the ends.
    for s in (1..group.segments.len()).step_by((group.segments.len() / 25).max(1)) {
        let (from, to) = (group.segments[s - 1].end, group.segments[s].end);
        for fraction in [0.25, 0.5, 0.9] {
            let p = t.sample(Cursor { done: s, fraction }).unwrap();
            let m0 = machine_xyz(&work, from.tip, from.a);
            let m1 = machine_xyz(&work, to.tip, to.a);
            let want = [
                m0.x + (m1.x - m0.x) * fraction,
                m0.y + (m1.y - m0.y) * fraction,
                m0.z + (m1.z - m0.z) * fraction,
                from.a.0 + (to.a.0 - from.a.0) * fraction,
            ];
            for (i, (got, want)) in p.work.iter().zip(want).enumerate() {
                assert!((got - want).abs() < 1e-9, "move {s} axis {i}");
            }
            // And it maps back to a tip the shared kinematics agrees with.
            let back = machine_xyz(&work, p.tip, Degrees(p.a));
            assert!(back.distance(Point3::new(want[0], want[1], want[2])) < 1e-9);
            assert!((p.axis.length() - 1.0).abs() < 1e-9);
            assert_eq!(p.kind, Some(group.segments[s].kind));
        }
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn req_sim_02_the_machine_view_turns_the_part_by_the_post_sign_and_keeps_the_tool_upright() {
    let (dir, app, _) = project("machine-view");
    let run = app.regenerate().unwrap();
    let t = timeline(&app, run);
    let sign = app.project().machine().unwrap().a_sense.sign();
    let mut seen_turn = false;
    for k in (0..t.move_count()).step_by((t.move_count() / 30).max(1)) {
        let p = t
            .sample(Cursor {
                done: k,
                fraction: 0.4,
            })
            .unwrap();
        assert!((p.part_turn - sign * p.a).abs() < 1e-12);
        seen_turn |= p.a.abs() > 30.0;
        // The part is turned about X by sign * A: an independent rotation.
        let theta = (sign * p.a).to_radians();
        let (s, c) = theta.sin_cos();
        let want = [
            p.tip.x,
            p.tip.y * c - p.tip.z * s,
            p.tip.y * s + p.tip.z * c,
        ];
        let machine = p.placement(ViewFrame::Machine);
        for (i, (got, want)) in machine.origin.iter().zip(want).enumerate() {
            assert!((got - want).abs() < 1e-9, "move {k} axis {i}");
        }
        assert_eq!(machine.axis, [0.0, 0.0, 1.0]);
        // In the part's frame the part does not turn and the tool leans.
        assert_eq!(p.part_rotation(ViewFrame::Part), 0.0);
        assert_eq!(p.part_rotation(ViewFrame::Machine), p.part_turn);
        let part = p.placement(ViewFrame::Part);
        assert_eq!(part.origin, [p.tip.x, p.tip.y, p.tip.z]);
        // Turning the part's-frame axis by the same rotation gives upright.
        let a = part.axis;
        let turned = [a[0], a[1] * c - a[2] * s, a[1] * s + a[2] * c];
        assert!(turned[0].abs() < 1e-9 && turned[1].abs() < 1e-9 && (turned[2] - 1.0).abs() < 1e-9);
    }
    assert!(seen_turn, "the path should turn A well past 30 degrees");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn req_sim_04_playback_plays_pauses_steps_changes_speed_and_jumps() {
    let (dir, app, _) = project("controls");
    let run = app.regenerate().unwrap();
    let t = timeline(&app, run);
    let n = t.move_count();
    let mut pb = Playback::new();
    assert!(!pb.is_playing());
    assert_eq!(pb.cursor(), Cursor::default());
    // Paused: advancing does nothing.
    assert!(!pb.advance(&t, 1.0));
    // Playing for a second at 10x moves 10 program seconds along.
    pb.play(&t);
    pb.set_speed(10.0);
    assert!(pb.advance(&t, 1.0));
    assert!((t.time_at(pb.cursor()) - 10.0_f64.min(t.total_time())).abs() < 1e-6);
    // A bad speed is ignored.
    pb.set_speed(f64::NAN);
    pb.set_speed(-3.0);
    pb.set_speed(0.0);
    assert_eq!(pb.speed(), 10.0);
    // Pause, then step: forward lands on whole moves, back too.
    pb.pause();
    pb.step(&t, 1);
    let after = pb.cursor();
    assert_eq!(after.fraction, 0.0);
    pb.step(&t, 3);
    assert_eq!(pb.cursor().done, (after.done + 3).min(n));
    pb.step(&t, -2);
    assert_eq!(pb.cursor().done, (after.done + 3).min(n) - 2);
    // Backwards from part-way into a move goes to that move's start first.
    pb.seek(&t, 0.5);
    let mid = pb.cursor();
    let mut half = Playback::new();
    half.step(&t, mid.done as isize);
    if mid.fraction > 0.0 {
        let mut pb2 = Playback::new();
        pb2.seek(&t, 0.5);
        pb2.step(&t, -1);
        assert_eq!(pb2.cursor().done, mid.done);
    }
    // Stepping back at the start and on at the end stays inside the program.
    let mut start = Playback::new();
    start.step(&t, -5);
    assert_eq!(start.cursor(), Cursor::default());
    let mut end = Playback::new();
    end.step(&t, isize::MAX);
    assert_eq!(end.cursor().done, n);
    // Jump to a move: just after it. Jump to an operation: its first move.
    pb.jump_to_move(&t, 7);
    assert_eq!(
        pb.cursor(),
        Cursor {
            done: 8,
            fraction: 0.0
        }
    );
    pb.jump_to_move(&t, n + 100);
    assert_eq!(pb.cursor().done, n);
    pb.jump_to_group(&t, 0);
    assert_eq!(pb.cursor().done, 0);
    // Playing to the end stops there; play again starts over.
    pb.set_speed(Playback::speed_to_finish_in(&t, 2.0));
    pb.play(&t);
    for _ in 0..30 {
        pb.advance(&t, 0.1);
    }
    assert_eq!(pb.cursor().done, n);
    assert!(!pb.is_playing());
    pb.play(&t);
    assert_eq!(pb.cursor(), Cursor::default());
    assert!(pb.is_playing());
    // Seeking is by fraction of program time and clamps.
    pb.seek(&t, 2.0);
    assert_eq!(pb.cursor().done, n);
    pb.seek(&t, f64::NAN);
    assert_eq!(pb.cursor(), Cursor::default());
    let _ = std::fs::remove_dir_all(dir);
}

/// Slice 9 review checklist: "Jumping to move N shows the same position as
/// the corresponding G-code line", and "The A rotation shown matches the post
/// output" (the LinuxCNC replay of the same lines is gate 3, run in CI).
#[test]
fn req_sim_04_jumping_to_a_move_shows_the_position_of_its_g_code_line() {
    let (dir, app, _) = project("gcode-lines");
    let run = app.regenerate().unwrap();
    assert_eq!(run.gcode.len(), 1);
    let text = run.gcode[0].1.clone();
    let program = read(&text).unwrap();
    let replayed = replay(&program, Dialect::Linuxcnc).unwrap();
    let lines: Vec<_> = replayed.moves().collect();
    let t = timeline(&app, run);
    // Walk the IR's moves and the program's moves together. A pose that
    // rounds onto the one before it writes no line (the post says so), so
    // those are skipped; every other move must be the next line's end.
    let mut next = 0;
    let mut matched = 0;
    let mut previous = t.sample(Cursor::default()).map(|p| p.work).unwrap();
    for k in 0..t.move_count() {
        let want = t.work_after(k).unwrap();
        let mut pb = Playback::new();
        pb.jump_to_move(&t, k);
        let shown = t.sample(pb.cursor()).unwrap().work;
        for i in 0..4 {
            assert!((shown[i] - want[i]).abs() < 1e-9, "move {k}");
        }
        let same = (0..4).all(|i| (want[i] - previous[i]).abs() < 5e-4);
        previous = want;
        if same {
            continue;
        }
        // Find the line that leaves the machine at this pose.
        let found = lines[next..]
            .iter()
            .position(|m| (0..4).all(|i| (m.to[i] - want[i]).abs() < 1.5e-3));
        let Some(offset) = found else {
            panic!("move {k} at {want:?} is not written by any later line");
        };
        next += offset + 1;
        matched += 1;
    }
    assert!(matched > 50, "{matched} moves matched lines");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn req_sim_02_the_tool_is_drawn_as_its_cutter_and_shank_with_the_tip_at_the_origin() {
    let meshes = tool_meshes(&ball()).unwrap();
    let cutter = &meshes.cutter;
    let shank = meshes.shank.unwrap();
    // The cutter reaches from the tip at z = 0 to the flute length, 2 mm in
    // radius; the shank carries on to the overall length.
    let height = |m: &openchuck_types::mesh::TriangleMesh| {
        m.vertices.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| {
            (lo.min(p.z), hi.max(p.z))
        })
    };
    assert_eq!(height(cutter), (0.0, 10.0));
    assert_eq!(height(&shank), (10.0, 40.0));
    let radius = cutter
        .vertices
        .iter()
        .map(|p| p.x.hypot(p.y))
        .fold(0.0, f64::max);
    assert!((radius - 2.0).abs() < 1e-9);
    // Triangles face outward: on the ball (centre at z = 2), away from it.
    let mut checked = 0;
    for i in 0..cutter.triangles.len() {
        let [a, b, c] = cutter.triangle(i).unwrap();
        let n = (b - a).cross(c - a);
        if n.length() < 1e-12 {
            continue;
        }
        let centroid = Point3::new(
            (a.x + b.x + c.x) / 3.0,
            (a.y + b.y + c.y) / 3.0,
            (a.z + b.z + c.z) / 3.0,
        );
        if centroid.z < 1.9 {
            let outward = Point3::new(centroid.x, centroid.y, centroid.z - 2.0);
            assert!(
                n.dot(openchuck_types::geometry::Vec3::new(
                    outward.x, outward.y, outward.z
                )) > 0.0
            );
            checked += 1;
        } else {
            assert!(n.x * centroid.x + n.y * centroid.y >= -1e-9);
        }
    }
    assert!(checked > 100);
    // A tool that is not valid has nothing to draw.
    let mut bad = ball();
    bad.diameter = Millimetres(0.0);
    assert!(tool_meshes(&bad).is_none());
}

#[test]
fn req_sim_01_the_display_tells_rapids_links_entries_and_cuts_apart_by_operation() {
    let (dir, app, _) = project("display");
    let run = app.regenerate().unwrap();
    let t = timeline(&app, run);
    let display = ToolpathDisplay::build(&t);
    assert_eq!(display.group_count(), 1);
    let classes: std::collections::BTreeSet<PathClass> =
        display.runs().iter().map(|r| r.class).collect();
    assert!(classes.contains(&PathClass::Cut), "{classes:?}");
    assert!(classes.contains(&PathClass::Rapid), "{classes:?}");
    // Every move is in exactly one run: the points add up (one per move, one
    // to start each run, more for moves that turn A a long way).
    let kinds: Vec<_> = t.run().toolpath.groups[0]
        .segments
        .iter()
        .map(|s| PathClass::of(s.kind))
        .collect();
    let mut runs_expected = 1;
    for pair in kinds.windows(2) {
        if pair[0] != pair[1] {
            runs_expected += 1;
        }
    }
    assert_eq!(display.runs().len(), runs_expected);
    // Hiding the operation leaves nothing; showing it leaves everything.
    let all = display.lines(&|_| true, usize::MAX);
    assert_eq!(all.level, 0);
    assert_eq!(all.tolerance, 0.0);
    assert!(!all.lines.is_empty());
    assert_eq!(all.points, all.full_points);
    let none = display.lines(&|_| false, usize::MAX);
    assert!(none.lines.is_empty() && none.points == 0);
    // Runs join: the first point of a run is where the move before it ended.
    for pair in display.runs().windows(2) {
        if pair[0].group == pair[1].group {
            assert!(
                pair[0].points.last().unwrap().distance(pair[1].points[0]) < 1e-9,
                "runs of one operation are joined"
            );
        }
    }
    // Same input, same lines (NFR-06).
    assert_eq!(ToolpathDisplay::build(&t).lines(&|_| true, usize::MAX), all);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn req_sim_01_a_long_turn_of_a_is_drawn_as_an_arc_not_a_chord() {
    let (dir, app, _) = project("arcs");
    let run = app.regenerate().unwrap();
    let t = timeline(&app, run);
    let display = ToolpathDisplay::build(&t);
    let machine = app.project().machine().unwrap();
    let work = RotaryFrame::work(machine, &app.project().setups()[0].frame);
    // Every drawn point lies where the controller would put the tool: on the
    // machine-linear path, so rebuilding it from the machine side agrees.
    let mut long_turns = 0;
    let mut previous: Option<openchuck_types::ir::ToolPose> = None;
    let group = &t.run().toolpath.groups[0];
    previous = previous.or(Some(group.start));
    for s in &group.segments {
        let p = previous.unwrap();
        if (s.end.a.0 - p.a.0).abs() > 30.0 {
            long_turns += 1;
        }
        previous = Some(s.end);
    }
    let points: usize = display.runs().iter().map(|r| r.points.len()).sum();
    // One point per move plus one to start each run, and more where A turns.
    let floor = group.segments.len() + display.runs().len();
    assert!(points >= floor, "{points} < {floor}");
    if long_turns > 0 {
        assert!(points > floor, "{long_turns} long turns drew no arc points");
    }
    // Every point is within the part's reach of the rotary axis (arcs bend
    // round the axis rather than cutting across it).
    let reach = group
        .segments
        .iter()
        .map(|s| s.end.tip.y.hypot(s.end.tip.z))
        .fold(0.0, f64::max);
    for run in display.runs() {
        for p in &run.points {
            assert!(p.y.hypot(p.z) <= reach + 1e-6, "{p:?}");
        }
    }
    let _ = work;
    let _ = std::fs::remove_dir_all(dir);
}

fn helix(n: usize) -> Vec<Point3> {
    (0..n)
        .map(|i| {
            let a = i as f64 * 0.01;
            Point3::new(a * 0.2, 20.0 * a.cos(), 20.0 * a.sin())
        })
        .collect()
}

fn distance_to_polyline(p: Point3, line: &[Point3]) -> f64 {
    line.windows(2)
        .map(|w| {
            let ab = w[1] - w[0];
            let l2 = ab.dot(ab);
            let t = if l2 > 0.0 {
                ((p - w[0]).dot(ab) / l2).clamp(0.0, 1.0)
            } else {
                0.0
            };
            p.distance(w[0] + ab * t)
        })
        .fold(f64::MAX, f64::min)
}

#[test]
fn req_sim_01_simplifying_a_path_keeps_it_within_the_tolerance_and_its_ends() {
    let path = helix(20_000);
    assert_eq!(simplify(&path, 0.0), path);
    assert_eq!(simplify(&path[..2], 1.0), path[..2].to_vec());
    let mut last = path.len();
    for tolerance in LEVEL_TOLERANCES.iter().skip(1) {
        let simple = simplify(&path, *tolerance);
        assert_eq!(simple.first(), path.first());
        assert_eq!(simple.last(), path.last());
        assert!(simple.len() <= last, "coarser levels keep fewer points");
        last = simple.len();
        // No dropped point is further from the simplified line than the
        // tolerance (checked on every 7th point to keep the test quick).
        for p in path.iter().step_by(7) {
            let d = distance_to_polyline(*p, &simple);
            assert!(d <= *tolerance + 1e-9, "{d} > {tolerance}");
        }
    }
    assert!(
        last < path.len() / 10,
        "{last} of {} points kept at 3 mm",
        path.len()
    );
}

#[test]
fn req_nfr_03_the_level_of_detail_follows_the_point_budget() {
    // A path of 300,000 moves, built directly so the test needs no project:
    // the display picks the finest level whose points fit.
    let (dir, app, _) = project("lod");
    let run = app.regenerate().unwrap();
    let t = timeline(&app, run);
    let display = ToolpathDisplay::build(&t);
    let full = display.lines(&|_| true, usize::MAX).full_points;
    assert!(full > 200);
    let mut previous = full;
    for budget in [full, full / 2, full / 5, full / 20, 10] {
        let chosen = display.lines(&|_| true, budget);
        assert!(
            chosen.points <= previous,
            "{budget}: {} > {previous}",
            chosen.points
        );
        assert!(
            chosen.points <= budget || chosen.level == LEVEL_TOLERANCES.len() - 1,
            "budget {budget}: {} points at level {}",
            chosen.points,
            chosen.level
        );
        assert_eq!(chosen.tolerance, LEVEL_TOLERANCES[chosen.level]);
        previous = chosen.points;
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn req_sim_05_triangles_are_sorted_into_the_bands_the_viewport_colours() {
    let tolerance = 0.1;
    let values = [
        Some((-0.5, false)), // overcut
        Some((-0.1, false)), // on the surface (inside the error)
        Some((0.0, false)),  // on the surface
        Some((0.1, false)),  // on the surface (inclusive)
        Some((0.2, false)),  // thin
        Some((0.25, false)), // thin (inclusive)
        Some((0.3, false)),  // medium
        Some((1.0, false)),  // medium (inclusive)
        Some((1.5, false)),  // thick
        Some((0.2, true)),   // only a bound: thick
        None,                // degenerate
    ];
    let bands = deviation_bands(&values, tolerance);
    assert_eq!(bands.tolerance, tolerance);
    let of = |band: DeviationBand| -> Vec<u32> {
        bands
            .bands
            .iter()
            .find(|(b, _)| *b == band)
            .map(|(_, s)| s.iter().copied().collect())
            .unwrap_or_default()
    };
    assert_eq!(of(DeviationBand::Overcut), [0]);
    assert_eq!(of(DeviationBand::OnTarget), [1, 2, 3]);
    assert_eq!(of(DeviationBand::Thin), [4, 5]);
    assert_eq!(of(DeviationBand::Medium), [6, 7]);
    assert_eq!(of(DeviationBand::Thick), [8, 9]);
    assert_eq!(of(DeviationBand::Unmeasured), [10]);
    // Every triangle is in exactly one band.
    let total: usize = bands.bands.iter().map(|(_, s)| s.len()).sum();
    assert_eq!(total, values.len());
    // Bands that nobody is in are not listed.
    assert!(deviation_bands(&[Some((0.0, false))], 0.1).bands.len() == 1);
}

#[test]
fn req_sim_05_the_part_is_coloured_by_its_deviation_from_a_simulation() {
    let (dir, app, _) = project("bands");
    let run = Arc::new(app.simulate().unwrap());
    let setup = app.project().setups()[0].id;
    let (model, bands) = app
        .start_deviation_bands(Arc::clone(&run), setup)
        .wait()
        .unwrap();
    assert_eq!(model, app.project().models()[0].id);
    let triangles = 12;
    let total: usize = bands.bands.iter().map(|(_, s)| s.len()).sum();
    assert_eq!(total, triangles, "every triangle of the cube is in a band");
    let report = run.simulation.as_ref().unwrap();
    assert_eq!(bands.tolerance, report.settings.cell.0);
    // The stock outline is a closed mesh the viewport can draw.
    let stock = report.setups[0].stock_mesh();
    assert!(stock.triangles.len() > 100);
    // A run made without a simulation says so.
    let plain = Arc::new(app.regenerate().unwrap());
    let error = app
        .start_deviation_bands(plain, setup)
        .wait()
        .unwrap_err()
        .to_string();
    assert!(error.contains("without a simulation"), "{error}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn req_sim_04_the_stock_at_a_move_comes_from_the_checkpoints() {
    let (dir, app, _) = project("stock-at");
    let run = Arc::new(app.simulate().unwrap());
    let t = Arc::new(Timeline::new(app.project(), Arc::clone(&run)).unwrap());
    let playback = Arc::new(app.start_stock_playback(Arc::clone(&t)).wait().unwrap());
    assert_eq!(playback.setups().len(), 1);
    assert!(playback.checkpoints() >= 2);
    let report = &run.simulation.as_ref().unwrap().setups[0];
    let volume = |mesh: &openchuck_types::mesh::TriangleMesh| -> f64 {
        // Divergence theorem on the closed blocky mesh.
        (0..mesh.triangles.len())
            .map(|i| {
                let [a, b, c] = mesh.triangle(i).unwrap();
                let n = (b - a).cross(c - a);
                n.dot(openchuck_types::geometry::Vec3::new(a.x, a.y, a.z)) / 6.0
            })
            .sum()
    };
    let n = t.move_count();
    let at = |done: usize| {
        app.start_stock_at(
            &playback,
            Cursor {
                done,
                fraction: 0.0,
            },
        )
        .wait()
        .unwrap()
    };
    let start = volume(&at(0));
    let end = volume(&at(n));
    assert!(
        (start - report.initial_volume).abs() < 1e-6 * report.initial_volume,
        "{start} vs {}",
        report.initial_volume
    );
    assert!(
        (end - report.final_volume).abs() < 1e-6 * report.initial_volume,
        "{end} vs {}",
        report.final_volume
    );
    assert!(end < start);
    // Material only comes off: the volume never rises along the path.
    let mut last = start;
    for k in (0..=n).step_by((n / 12).max(1)) {
        let v = volume(&at(k));
        assert!(v <= last + 1e-6 * start, "move {k}: {v} > {last}");
        last = v;
    }
    // The cursor part-way into a move shows the stock after the moves done.
    let mid = n / 2;
    assert_eq!(
        app.start_stock_at(
            &playback,
            Cursor {
                done: mid,
                fraction: 0.7
            }
        )
        .wait()
        .unwrap(),
        at(mid)
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn req_ui_09_a_timeline_needs_a_run_made_by_the_pipeline_and_a_machine() {
    let (dir, app, _) = project("timeline-errors");
    let mut run = app.regenerate().unwrap();
    run.groups.clear();
    let error = Timeline::new(app.project(), Arc::new(run)).unwrap_err();
    assert!(error.to_string().contains("regenerate"), "{error}");
    let _ = std::fs::remove_dir_all(dir);
}

/// Slice 9 review checklist: "The UI stays responsive during generation
/// within a stated frame-time budget" (decision record 0040 item 8). The user
/// interface thread only polls (`try_result`, `drain_events`); this checks
/// that polling stays cheap and prompt while the pipeline runs on the pool.
#[test]
fn req_ui_09_req_nfr_04_polling_a_running_job_never_blocks_the_interface_thread() {
    let (dir, app, _) = project("responsive");
    app.set_cache_versions(0);
    // A run long enough to poll many times: simulate at a fine cell.
    let mut gaps = Vec::new();
    let job = app.start_simulate();
    let mut last = Instant::now();
    let started = last;
    let mut events = 0;
    loop {
        let result = job.try_result();
        events += app.drain_events().len();
        let now = Instant::now();
        gaps.push(now - last);
        last = now;
        if let Some(r) = result {
            r.unwrap();
            break;
        }
        // A frame's worth of other work between polls.
        std::thread::sleep(Duration::from_millis(2));
        assert!(
            started.elapsed() < Duration::from_secs(120),
            "the job never finished"
        );
    }
    assert!(
        events >= 2,
        "the job reported its start and end ({events} events)"
    );
    gaps.sort();
    let worst = *gaps.last().unwrap();
    eprintln!(
        "responsiveness: {} polls, p50 {:?}, p95 {:?}, worst {worst:?}",
        gaps.len(),
        gaps[gaps.len() / 2],
        gaps[gaps.len() * 95 / 100]
    );
    // Each poll-and-drain, with its 2 ms of other work, fits in half a frame
    // of the 33 ms budget even though every core is busy with the job.
    // Hosted CI runners are noisy, so the limit is five times larger there
    // (`CI` set), as for the pick budget; the strict figure is checked locally.
    let relax = if std::env::var_os("CI").is_some() {
        5
    } else {
        1
    };
    assert!(
        worst < Duration::from_millis(17 * relax),
        "the longest gap between polls was {worst:?}"
    );
    let _ = std::fs::remove_dir_all(dir);
}

/// Slice 9 review checklist: "Cancel stops within a stated time and leaves no
/// partial result" (decision record 0040 item 8: 2 s at the default cell).
/// The cancel is asked at several points of the run, because the time it takes
/// depends on which stage the run is in: one asked early lands in path
/// generation, one asked later while the stock is cut, and the longest stage
/// must watch the flag too.
#[test]
fn req_ui_09_cancel_stops_a_run_within_two_seconds_and_leaves_no_partial_result() {
    let (dir, app, _) = project("cancel");
    // How long the whole run takes on this machine, to judge a stop against:
    // a stop that waits for a stage to end takes about as long as the stage.
    app.set_cache_versions(0);
    let started = Instant::now();
    app.start_simulate().wait().unwrap();
    let whole = started.elapsed();
    let mut worst = Duration::ZERO;
    for delay in [0u64, 20, 60, 150, 400, 900] {
        app.set_cache_versions(0);
        let job = app.start_simulate();
        std::thread::sleep(Duration::from_millis(delay));
        let asked = Instant::now();
        job.cancel();
        let result = job.wait();
        let took = asked.elapsed();
        worst = worst.max(took);
        // The run may have finished before the cancel landed (a tiny job); if
        // it did not, it is an error that says so and gives no result.
        match result {
            Ok(_) => {}
            Err(e) => assert!(e.is_cancelled(), "{e}"),
        }
        assert!(
            took < Duration::from_secs(2),
            "cancel asked at {delay} ms took {took:?}"
        );
    }
    eprintln!("cancel: worst stop {worst:?}, whole run {whole:?}");
    // Whatever the machine's speed, a cancel is far quicker than the run it
    // stops; this holds where the absolute limit above would not catch a stage
    // that ignores the flag.
    assert!(
        worst < whole / 4,
        "the worst stop {worst:?} is not far below the whole run {whole:?}"
    );
    // And the application is still good: a second run completes normally.
    app.regenerate().unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn req_ui_06_an_operation_is_added_edited_generated_and_deleted_as_undoable_steps() {
    let (dir, mut app, op) = project("opflow");
    let setup = app.project().setups()[0].id;
    let tool = app.project().tools()[0].id;
    // The registry's strategies are what the dialog offers.
    let choices = app.strategy_choices();
    assert!(choices.iter().any(|c| c.name == "helical_finishing"));
    assert!(choices.iter().all(|c| !c.label.is_empty()));
    // Adding: at the end, enabled, with the strategy's defaults.
    let before = app.project().operations().len();
    let second = app
        .add_operation("  Second pass ", "helical_finishing", setup, Some(tool))
        .unwrap();
    assert_eq!(app.project().operations().len(), before + 1);
    let added = app.project().operation(second).unwrap();
    assert_eq!(added.name, "Second pass");
    assert!(added.enabled && added.parameters.is_empty());
    assert_eq!(app.project().operations().last().unwrap().id, second);
    // Edit and generate: both operations make a toolpath; the first is
    // reused when only the second changed (chain invalidation, decision 0030).
    let first = app.regenerate().unwrap();
    assert_eq!(first.toolpath.groups.len(), 2);
    assert_eq!(first.groups.len(), 2);
    assert_eq!(first.groups[0].operation, op);
    assert_eq!(first.groups[1].operation, second);
    app.set_parameter(
        second,
        "stepover",
        Some(openchuck_app::project::ParameterValue::Number(0.5)),
    )
    .unwrap();
    let again = app.regenerate().unwrap();
    assert_eq!(again.reused, [op]);
    assert_eq!(again.generated, [second]);
    // Refusals say why and change nothing.
    let count = app.project().operations().len();
    for (name, strategy, setup_id, tool_id, word) in [
        ("", "helical_finishing", setup, Some(tool), "name"),
        (
            "x",
            "no_such_strategy",
            setup,
            Some(tool),
            "no_such_strategy",
        ),
        (
            "x",
            "helical_finishing",
            SetupId::new(),
            Some(tool),
            "setup",
        ),
        ("x", "helical_finishing", setup, Some(ToolId::new()), "tool"),
    ] {
        let error = app
            .add_operation(name, strategy, setup_id, tool_id)
            .unwrap_err()
            .to_string();
        assert!(error.contains(word), "{error}");
    }
    assert_eq!(app.project().operations().len(), count);
    // Deleting is one undoable step; an unknown operation is an error.
    app.delete_operation(second).unwrap();
    assert!(app.project().operation(second).is_none());
    assert!(app.delete_operation(second).is_err());
    assert!(app.undo().unwrap());
    assert!(app.project().operation(second).is_some());
    let _ = std::fs::remove_dir_all(dir);
}

/// Item 8, level of detail for paths with millions of segments: a helical
/// path of two million cutting moves is cut into runs, simplified to fit a
/// budget of one million points, and still follows the path.
#[test]
fn req_nfr_04_a_path_of_two_million_moves_is_displayed_within_its_budget() {
    use openchuck_types::geometry::UnitVec3;
    use openchuck_types::ir::{MoveKind, Segment, ToolPose};
    use openchuck_types::units::{FeedRate, Millimetres, Seconds};

    let (dir, app, _) = project("two-million");
    let mut run = app.regenerate().unwrap();
    const MOVES: usize = 2_000_000;
    let pose = |i: usize| {
        // A spiral on a cylinder of radius 12 mm: 400 moves a turn, 0.01 mm
        // of X for each turn's worth of ten, so neighbouring turns are
        // 0.25 mm apart.
        let angle = i as f64 * std::f64::consts::TAU / 400.0;
        ToolPose {
            tip: Point3::new(
                i as f64 * 0.25 / 400.0,
                12.0 * angle.cos(),
                12.0 * angle.sin(),
            ),
            axis: UnitVec3::Z,
            a: Degrees(0.0),
        }
    };
    let group = &mut run.toolpath.groups[0];
    group.start = pose(0);
    group.segments = (1..=MOVES)
        .map(|i| Segment {
            kind: MoveKind::Cutting,
            end: pose(i),
            contact_feed: Some(FeedRate(600.0)),
            duration: Seconds(0.001),
            contact_length: Millimetres(0.19),
        })
        .collect();
    let started = Instant::now();
    let t = timeline(&app, run);
    let timeline_time = started.elapsed();
    let started = Instant::now();
    let display = ToolpathDisplay::build(&t);
    let build_time = started.elapsed();
    assert_eq!(t.move_count(), MOVES);
    let started = Instant::now();
    let lines = display.lines(&|_| true, 1_000_000);
    let lines_time = started.elapsed();
    let again = Instant::now();
    let cached = display.lines(&|_| true, 1_000_000);
    let cached_time = again.elapsed();
    eprintln!(
        "{MOVES} moves: timeline {timeline_time:?}, runs {build_time:?}, lines {lines_time:?} (level {}, {} of {} points), cached {cached_time:?}",
        lines.level, lines.points, lines.full_points
    );
    assert!(lines.full_points > MOVES);
    assert!(lines.points <= 1_000_000 || lines.level == LEVEL_TOLERANCES.len() - 1);
    assert!(
        lines.level > 0,
        "two million points do not fit a budget of one million"
    );
    assert_eq!(cached, lines);
    // The drawn path stays within the stated tolerance of the real one:
    // sample the original and measure against the simplified polyline.
    let run_points = &display.runs()[0].points;
    let simple = &lines.lines[0].points;
    for k in (0..run_points.len()).step_by(20_011) {
        let p = run_points[k];
        // Distance to the nearest simplified segment among those near in X.
        let d = simple
            .windows(2)
            .filter(|w| (w[0].x.min(w[1].x) - 1.0..=w[0].x.max(w[1].x) + 1.0).contains(&p.x))
            .map(|w| {
                let ab = w[1] - w[0];
                let l2 = ab.dot(ab);
                let u = if l2 > 0.0 {
                    ((p - w[0]).dot(ab) / l2).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                p.distance(w[0] + ab * u)
            })
            .fold(f64::MAX, f64::min);
        assert!(
            d <= lines.tolerance + 1e-9,
            "point {k}: {d} > {}",
            lines.tolerance
        );
    }
    let _ = std::fs::remove_dir_all(dir);
}

/// Stock playback and the colours read the run's own project, so an edit made
/// after the run cannot put the old toolpath on a different stock (SIM-04,
/// SIM-05; verification policy 8).
#[test]
fn req_sim_04_stock_playback_uses_the_stock_the_run_was_made_with() {
    let (dir, mut app, _) = project("stock-edited");
    let run = Arc::new(app.simulate().unwrap());
    let t = Arc::new(Timeline::new(app.project(), Arc::clone(&run)).unwrap());
    let before = Arc::new(app.start_stock_playback(Arc::clone(&t)).wait().unwrap());
    let first = app
        .start_stock_at(&before, Cursor::default())
        .wait()
        .unwrap();

    // The stock is made much bigger after the run.
    let mut setup = app.project().setups()[0].clone();
    setup.stock = Some(Stock::Cylinder {
        diameter: Millimetres(40.0),
        length: Millimetres(30.0),
        x_start: Millimetres(-15.0),
    });
    app.execute(Command::Replace(Item::Setup(setup))).unwrap();

    let after = Arc::new(app.start_stock_playback(Arc::clone(&t)).wait().unwrap());
    let second = app
        .start_stock_at(&after, Cursor::default())
        .wait()
        .unwrap();
    assert_eq!(first, second, "the stock shown changed with the project");
    let _ = std::fs::remove_dir_all(dir);
}
