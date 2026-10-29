// SPDX-License-Identifier: AGPL-3.0-or-later
//! OP-24/GC-09: reached-move identity for independent Facing verification.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::float_cmp)]
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
fn pose(x: f64, a: f64) -> ToolPose {
    pose_at(x, a, 30.)
}
fn pose_at(x: f64, a: f64, r: f64) -> ToolPose {
    ToolPose {
        tip: tool_tip_in_model(
            &frame(),
            &MachinePosition {
                xyz: Point3::new(x, 0., r),
                a: Degrees(a),
            },
        ),
        axis: tool_axis_in_model(&frame(), Degrees(a)),
        a: Degrees(a),
    }
}

#[test]
fn req_gc_08_req_gc_09_fed_observations_use_independent_controller_durations() {
    for (rule, r, x, a, mode, feed, seconds) in [
        (TimingRule::Linuxcnc, 30., 3., 90., "G94", 180., 1.),
        (TimingRule::Linuxcnc, 30., 0., 90., "G94", 180., 30.),
        (TimingRule::InverseTime, 30., 3., 90., "G93", 60., 1.),
        (
            TimingRule::Makera,
            30.,
            3.,
            90.,
            "G94",
            600.,
            0.025 * (60. * std::f64::consts::PI + 30.),
        ),
        (
            TimingRule::Makera,
            30.,
            0.,
            90.,
            "G94",
            600.,
            0.025 * (60. * std::f64::consts::PI + 30.),
        ),
        (
            TimingRule::Makera,
            0.5,
            0.,
            90.,
            "G94",
            600.,
            0.025 * (2. * std::f64::consts::PI + 30.),
        ),
        (TimingRule::Makera, 100., 0., 90., "G94", 600., 9.),
    ] {
        let path = Toolpath {
            groups: vec![ToolpathGroup {
                rotary_reset_after: Vec::new(),
                tags: PathTags {
                    operation: "fed".into(),
                    setup: "s".into(),
                    tool: "t".into(),
                },
                start: pose_at(0., 0., r),
                segments: vec![Segment {
                    kind: MoveKind::Cutting,
                    end: pose_at(x, a, r),
                    duration: Seconds(seconds),
                    contact_feed: Some(FeedRate(600.)),
                    contact_length: Millimetres(0.),
                }],
            }],
        };
        let text = format!("G90 G21 G54\nG0 X0 Y0 Z{r} A0\n{mode}\nG1 X{x} A{a} F{feed}\nM2\n");
        let replay = replay(&read(&text).unwrap(), Dialect::MakeraZ1).unwrap();
        let mut observed = None;
        let result = compare_with_moves(
            &replay,
            &path,
            &frame(),
            rule,
            Tolerances::for_decimals(3, 3),
            &mut |m| {
                if m.segment == Some(0) {
                    observed = m.seconds;
                }
                Ok(())
            },
        )
        .unwrap();
        assert!((observed.unwrap() - seconds).abs() < 1e-10);
        assert!((result.gcode_cut_seconds - seconds).abs() < 1e-10);
        assert_eq!(
            result,
            compare(
                &replay,
                &path,
                &frame(),
                rule,
                Tolerances::for_decimals(3, 3)
            )
            .unwrap()
        );
        for f in ["", " F0", " F-1"] {
            let invalid = format!("G90 G21 G54\nG0 X0 Y0 Z{r} A0\n{mode}\nG1 X{x} A{a}{f}\nM2\n");
            let replay = openchuck_gcode::replay(&read(&invalid).unwrap(), Dialect::MakeraZ1);
            if let Ok(replay) = replay {
                assert!(
                    compare_with_moves(
                        &replay,
                        &path,
                        &frame(),
                        rule,
                        Tolerances::for_decimals(3, 3),
                        &mut |_| Ok(())
                    )
                    .is_err()
                );
            }
        }
    }
}
fn fixture() -> (Toolpath, String) {
    let group = |name: &str, start, end| ToolpathGroup {
        rotary_reset_after: Vec::new(),
        tags: PathTags {
            operation: name.into(),
            setup: "s".into(),
            tool: "t".into(),
        },
        start: pose(0., start),
        segments: vec![
            Segment {
                kind: MoveKind::Rapid,
                end: pose(0.0001, start),
                duration: Seconds(1.),
                contact_feed: None,
                contact_length: Millimetres(0.),
            },
            Segment {
                kind: MoveKind::Rapid,
                end: pose(1., end),
                duration: Seconds(1.),
                contact_feed: None,
                contact_length: Millimetres(0.),
            },
        ],
    };
    (
        Toolpath {
            groups: vec![group("first", 710., 720.), group("facing", 720., 730.)],
        },
        "G90 G21\nG54\nG0 X0 Y0 Z30 A710\nX1 A720\nG92.4 A0 S0\nX0 A0\nX1 A10\nM2\n".into(),
    )
}

#[test]
fn req_gc_08_observations_remain_provisional_until_position_angle_time_and_total_travel_pass() {
    let group = ToolpathGroup {
        rotary_reset_after: Vec::new(),
        tags: PathTags {
            operation: "checks".into(),
            setup: "s".into(),
            tool: "t".into(),
        },
        start: pose(0., 0.),
        segments: vec![Segment {
            kind: MoveKind::Cutting,
            end: pose(3., 90.),
            duration: Seconds(1.),
            contact_feed: Some(FeedRate(600.)),
            contact_length: Millimetres(3.),
        }],
    };
    let path = Toolpath {
        groups: vec![group],
    };
    for (body, expected) in [
        ("G1 X3.1 A90 F180", "tip error"),
        ("G1 X3 A90.01 F180", "A error"),
        ("G1 X3 A90 F90", "time"),
    ] {
        let text = format!("G90 G21 G54\nG0 X0 Y0 Z30 A0\n{body}\nM2\n");
        let replay = replay(&read(&text).unwrap(), Dialect::Linuxcnc).unwrap();
        let mut tolerances = Tolerances::for_decimals(3, 3);
        if expected == "A error" {
            tolerances.position_mm = 1.;
        }
        let mut observed = false;
        let error = compare_with_moves(
            &replay,
            &path,
            &frame(),
            TimingRule::Linuxcnc,
            tolerances,
            &mut |m| {
                observed |= m.segment.is_some();
                Ok(())
            },
        )
        .unwrap_err();
        assert!(
            observed,
            "An observation must not be mistaken for final success"
        );
        assert!(error.to_string().contains(expected), "{error}");
    }
    let mut rotary = path.clone();
    rotary.groups[0].segments.clear();
    for a in [10., 0.] {
        rotary.groups[0].segments.push(Segment {
            kind: MoveKind::Cutting,
            end: pose(0., a),
            duration: Seconds(1.),
            contact_feed: Some(FeedRate(600.)),
            contact_length: Millimetres(0.),
        });
    }
    // Each A endpoint differs by only0.0009deg (<0.001), but two moves
    // accumulate0.0036deg extra travel, exceeding the three-pose0.003 budget.
    let text = "G90 G21 G54\nG0 X0 Y0 Z30 A-0.0009\nG1 A10.0009 F600\nA-0.0009\nM2\n";
    let replay = replay(&read(text).unwrap(), Dialect::Linuxcnc).unwrap();
    let error = compare_with_moves(
        &replay,
        &rotary,
        &frame(),
        TimingRule::Linuxcnc,
        Tolerances::for_decimals(3, 3),
        &mut |_| Ok(()),
    )
    .unwrap_err();
    assert!(error.to_string().contains("A travel"), "{error}");
}
#[test]
fn req_op_24_req_gc_09_reached_moves_keep_operation_segment_and_physical_counter_identity() {
    let (path, text) = fixture();
    let motion = replay(&read(&text).unwrap(), Dialect::MakeraZ1).unwrap();
    let mut reached = Vec::new();
    let result = compare_with_moves(
        &motion,
        &path,
        &frame(),
        TimingRule::Makera,
        Tolerances::for_decimals(3, 3),
        &mut |m| {
            reached.push((
                m.group,
                m.segment,
                m.movement.to,
                m.coordinate_shift,
                m.seconds,
            ));
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(
        result,
        compare(
            &motion,
            &path,
            &frame(),
            TimingRule::Makera,
            Tolerances::for_decimals(3, 3)
        )
        .unwrap()
    );
    assert_eq!(reached.len(), 4);
    assert_eq!((reached[0].0, reached[0].1), (0, None));
    assert_eq!(
        (reached[1].0, reached[1].1),
        (0, Some(1)),
        "Sub-resolution segment has no executed move"
    );
    assert_eq!((reached[2].0, reached[2].1), (1, None));
    assert_eq!((reached[3].0, reached[3].1), (1, Some(1)));
    assert_eq!(reached[3].2[3], 10.);
    assert_eq!(reached[3].3, 720.);
    assert!(
        reached.iter().all(|m| m.4.is_none()),
        "Rapids have no programmed feed duration"
    );
}
#[test]
fn req_op_24_req_nfr_04_observer_failure_refuses_the_whole_comparison() {
    let (path, text) = fixture();
    let motion = replay(&read(&text).unwrap(), Dialect::MakeraZ1).unwrap();
    let result = compare_with_moves(
        &motion,
        &path,
        &frame(),
        TimingRule::Makera,
        Tolerances::for_decimals(3, 3),
        &mut |_| Err(CompareError::Structure("cancel verification".into())),
    );
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("cancel verification")
    );
}
#[test]
fn req_op_24_req_gc_09_observation_does_not_waive_extra_missing_or_wrong_moves() {
    let (path, text) = fixture();
    for text in [
        text.replace("X1 A10", "X1 A370"),
        text.replace("X1 A10\n", ""),
        text.replace("M2", "X2\nM2"),
    ] {
        let motion = replay(&read(&text).unwrap(), Dialect::MakeraZ1).unwrap();
        assert!(
            compare_with_moves(
                &motion,
                &path,
                &frame(),
                TimingRule::Makera,
                Tolerances::for_decimals(3, 3),
                &mut |_| Ok(())
            )
            .is_err()
        );
    }
}
