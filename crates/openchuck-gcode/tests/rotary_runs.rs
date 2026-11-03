// SPDX-License-Identifier: AGPL-3.0-or-later
//! Gate 2 with combined pure-A blocks (GC-04 `combine_rotary_runs`, GC-08,
//! GC-17; decision 0648, accepted).
//!
//! One G-code block may stand for several consecutive IR poses only when it
//! turns A alone and every pose lies on it exactly: the same written X, Y and
//! Z, its A on the way from the block's start to its end, in order. Each
//! pose is then compared, and timed, as if the block ended there.
#![allow(clippy::unwrap_used, clippy::float_cmp, clippy::indexing_slicing)]
use openchuck_gcode::{
    CompareError, Dialect, TimingRule, Tolerances, compare, compare_with_moves, read, replay,
};
use openchuck_types::geometry::Point3;
use openchuck_types::ir::{MoveKind, PathTags, Segment, ToolPose, Toolpath, ToolpathGroup};
use openchuck_types::kinematics::{
    MachinePosition, RotaryFrame, tool_axis_in_model, tool_tip_in_model,
};
use openchuck_types::machine::{ASense, RotaryAxis};
use openchuck_types::units::{Degrees, FeedRate, Millimetres, Seconds};

fn frame() -> RotaryFrame {
    RotaryFrame {
        axis: RotaryAxis::X,
        sense: ASense::Positive,
        origin: Point3::ORIGIN,
    }
}

fn at(z: f64, a: f64) -> ToolPose {
    ToolPose {
        tip: tool_tip_in_model(
            &frame(),
            &MachinePosition {
                xyz: Point3::new(0.0, 0.0, z),
                a: Degrees(a),
            },
        ),
        axis: tool_axis_in_model(&frame(), Degrees(a)),
        a: Degrees(a),
    }
}

/// From A0 at Z30, fed A-only steps to each of `ends` (A, Z), one second each.
fn path(ends: &[(f64, f64)]) -> Toolpath {
    Toolpath {
        groups: vec![ToolpathGroup {
            rotary_reset_after: Vec::new(),
            tags: PathTags {
                setup: "s".into(),
                operation: "o".into(),
                tool: "t".into(),
            },
            start: at(30.0, 0.0),
            segments: ends
                .iter()
                .map(|&(a, z)| Segment {
                    kind: MoveKind::Cutting,
                    end: at(z, a),
                    duration: Seconds(1.0),
                    contact_length: Millimetres(1.0),
                    contact_feed: Some(FeedRate(300.0)),
                })
                .collect(),
        }],
    }
}

/// Eight 5 degree steps to A40, at the fixed point.
fn ring() -> Toolpath {
    path(
        &(1..=8)
            .map(|k| (5.0 * f64::from(k), 30.0))
            .collect::<Vec<_>>(),
    )
}

fn check(text: &str, toolpath: &Toolpath) -> Result<openchuck_gcode::Comparison, CompareError> {
    compare(
        &replay(&read(text).unwrap(), Dialect::Linuxcnc).unwrap(),
        toolpath,
        &frame(),
        TimingRule::Linuxcnc,
        Tolerances::for_decimals(3, 3),
    )
}

const HEAD: &str = "G90 G21\nG54\nG0 X0 Y0 Z30 A0\n";

#[test]
fn req_gc_08_replay_accepts_an_exact_combined_rotation() {
    // 300 degrees a minute: each 5 degrees takes the IR's one second.
    let text = format!("{HEAD}G1 A40 F300\nM2\n");
    let report = check(&text, &ring()).unwrap();
    assert_eq!(report.poses, 9);
    assert_eq!(report.ir_a_travel, 40.0);
    assert_eq!(report.gcode_a_travel, 40.0);
    assert!((report.gcode_cut_seconds - 8.0).abs() < 1e-9);
    assert!(report.max_time_error < 1e-9);

    // Observers see each pose reached on its part of the block, timed as
    // that part (operation checks rely on the reached words).
    let mut reached = Vec::new();
    compare_with_moves(
        &replay(&read(&text).unwrap(), Dialect::Linuxcnc).unwrap(),
        &ring(),
        &frame(),
        TimingRule::Linuxcnc,
        Tolerances::for_decimals(3, 3),
        &mut |m| {
            if let Some(s) = m.segment {
                reached.push((
                    s,
                    m.movement.from[3],
                    m.movement.to[3],
                    m.seconds,
                    m.movement.line,
                ));
            }
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(reached.len(), 8);
    for (k, (s, from, to, seconds, line)) in reached.into_iter().enumerate() {
        assert_eq!(s, k);
        assert_eq!(from, 5.0 * k as f64);
        assert_eq!(to, 5.0 * (k + 1) as f64);
        assert!((seconds.unwrap() - 1.0).abs() < 1e-9);
        assert_eq!(line, 4);
    }

    // The same rotation still matches when written step by step.
    let steps: String = (1..=8).map(|k| format!("G1 A{} F300\n", 5 * k)).collect();
    assert_eq!(
        check(&format!("{HEAD}{steps}M2\n"), &ring()).unwrap().poses,
        9
    );
}

#[test]
fn req_gc_08_replay_refuses_a_combined_block_that_does_not_pass_every_pose() {
    // Beyond the IR's last pose.
    assert!(check(&format!("{HEAD}G1 A45 F300\nM2\n"), &ring()).is_err());
    // Short of it: the last pose is never reached.
    assert!(check(&format!("{HEAD}G1 A35 F300\nM2\n"), &ring()).is_err());
    // The other way round to the same orientation.
    assert!(check(&format!("{HEAD}G1 A-320 F300\nM2\n"), &ring()).is_err());
    // A linear axis moves as well: not a pure rotation, so the poses on the
    // way are not on it.
    assert!(check(&format!("{HEAD}G1 Z29.9 A40 F300\nM2\n"), &ring()).is_err());
    // An IR pose off the fixed point in the middle of the run.
    let mut ends: Vec<(f64, f64)> = (1..=8).map(|k| (5.0 * f64::from(k), 30.0)).collect();
    ends[3].1 = 29.0;
    assert!(check(&format!("{HEAD}G1 A40 F300\nM2\n"), &path(&ends)).is_err());
    // IR poses that go back on themselves cannot lie on one block in order.
    let back = path(&[(5.0, 30.0), (10.0, 30.0), (5.0, 30.0), (40.0, 30.0)]);
    assert!(check(&format!("{HEAD}G1 A40 F300\nM2\n"), &back).is_err());
    // The wrong rate: each part takes twice the IR's time.
    let slow = check(&format!("{HEAD}G1 A40 F150\nM2\n"), &ring()).unwrap_err();
    assert!(matches!(slow, CompareError::Tolerance(_)), "{slow}");
}
