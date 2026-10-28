// SPDX-License-Identifier: AGPL-3.0-or-later
//! Physical turns and controller counter changes are distinct (decision 0241).
#![allow(clippy::unwrap_used, clippy::float_cmp)]
use openchuck_gcode::{Dialect, TimingRule, Tolerances, compare, compare_with_moves, read, replay};
use openchuck_types::geometry::Point3;
use openchuck_types::ir::{MoveKind, PathTags, Segment, ToolPose, Toolpath, ToolpathGroup};
use openchuck_types::kinematics::{
    MachinePosition, RotaryFrame, tool_axis_in_model, tool_tip_in_model,
};
use openchuck_types::machine::{ASense, RotaryAxis};
use openchuck_types::units::{Degrees, Millimetres, Seconds};

fn frame() -> RotaryFrame {
    RotaryFrame {
        axis: RotaryAxis::X,
        sense: ASense::Positive,
        origin: Point3::ORIGIN,
    }
}
fn pose(a: f64) -> ToolPose {
    ToolPose {
        tip: tool_tip_in_model(
            &frame(),
            &MachinePosition {
                xyz: Point3::new(0.0, 0.0, 30.0),
                a: Degrees(a),
            },
        ),
        axis: tool_axis_in_model(&frame(), Degrees(a)),
        a: Degrees(a),
    }
}
fn group(start: f64, end: f64) -> ToolpathGroup {
    ToolpathGroup {
        rotary_reset_after: Vec::new(),
        tags: PathTags {
            setup: "s".into(),
            operation: "o".into(),
            tool: "t".into(),
        },
        start: pose(start),
        segments: vec![Segment {
            kind: MoveKind::Rapid,
            end: pose(end),
            duration: Seconds(1.0),
            contact_length: Millimetres(0.0),
            contact_feed: None,
        }],
    }
}

#[test]
fn req_gc_09_boundary_reset_has_no_ir_move_and_preserves_subsequent_absolute_a() {
    for (before, after) in [
        (720.0, 0.0),
        (-720.0, 0.0),
        (593275.0, 355.0),
        (-593275.0, -355.0),
        (360.0, 360.0),
        (-360.0, -360.0),
    ] {
        let path = Toolpath {
            groups: vec![group(before - 10.0, before), group(before, before + 10.0)],
        };
        let text = format!(
            "G90 G21\nG54\nG0 X0 Y0 Z30 A{}\nA{before}\nG92.4 A0 S0\nA{}\nG92.4 A0 S0\nM2\n",
            before - 10.0,
            after + 10.0
        );
        let motion = replay(&read(&text).unwrap(), Dialect::MakeraZ1).unwrap();
        let report = compare(
            &motion,
            &path,
            &frame(),
            TimingRule::Makera,
            Tolerances::for_decimals(3, 3),
        )
        .unwrap();
        assert_eq!(report.ir_a_travel, 20.0);
        assert_eq!(report.gcode_a_travel, 20.0);
        let wrong = text.replacen(
            &format!("A{}\nG92.4", after + 10.0),
            &format!("A{}\nG92.4", after + 370.0),
            1,
        );
        assert!(
            compare(
                &replay(&read(&wrong).unwrap(), Dialect::MakeraZ1).unwrap(),
                &path,
                &frame(),
                TimingRule::Makera,
                Tolerances::for_decimals(3, 3)
            )
            .is_err()
        );
    }
}

#[test]
fn req_gc_01_req_gc_10_explicit_physical_full_turns_are_checked_as_travel() {
    let path = Toolpath {
        groups: vec![group(720.0, 0.0)],
    };
    let source = read("G90 G21\nG54\nG0 X0 Y0 Z30 A720\nA0\nM2\n").unwrap();
    let report = compare(
        &replay(&source, Dialect::Linuxcnc).unwrap(),
        &path,
        &frame(),
        TimingRule::Linuxcnc,
        Tolerances::for_decimals(3, 3),
    )
    .unwrap();
    assert_eq!(report.ir_a_travel, 720.0);
    assert_eq!(report.gcode_a_travel, 720.0);
    // A counter reset may not stand in for that physical motion.
    let substituted = read("G90 G21\nG54\nG0 X0 Y0 Z30 A720\nG92.4 A0 S0\nM2\n").unwrap();
    assert!(
        compare(
            &replay(&substituted, Dialect::MakeraZ1).unwrap(),
            &path,
            &frame(),
            TimingRule::Makera,
            Tolerances::for_decimals(3, 3)
        )
        .is_err()
    );
}

#[test]
fn req_op_15_req_gc_09_declared_pass_resets_preserve_signed_turns_and_exact_boundaries() {
    for before in [720.0_f64, -720.0, 593275.0, -593275.0, 360.0, -360.0] {
        let after = if before.abs() > 360.0 {
            before % 360.0
        } else {
            before
        };
        let mut g = group(before - 10.0, before);
        // An existing retract may round to no motion. Its boundary must attach
        // to the last reached pose, without consuming an extra IR/G-code move.
        let stationary = g.segments[0];
        g.segments.push(stationary);
        g.segments.push(Segment {
            end: pose(before + 10.0),
            ..stationary
        });
        g.rotary_reset_after = vec![1];
        let path = Toolpath { groups: vec![g] };
        let text = format!(
            "G90 G21\nG54\nG0 X0 Y0 Z30 A{}\nA{before}\nG92.4 A0 S0\nA{}\nM2\n",
            before - 10.0,
            after + 10.0
        );
        let run = |text: &str, p: &Toolpath| {
            compare(
                &replay(&read(text).unwrap(), Dialect::MakeraZ1).unwrap(),
                p,
                &frame(),
                TimingRule::Makera,
                Tolerances::for_decimals(3, 3),
            )
        };
        let c = run(&text, &path).unwrap();
        assert_eq!(c.ir_a_travel, 20.0);
        assert_eq!(c.gcode_a_travel, 20.0);
        // FACING's observer must retain source identities and reconstruct
        // physical A across an AROT pass reset, including the omitted retract.
        let motion = replay(&read(&text).unwrap(), Dialect::MakeraZ1).unwrap();
        let mut observed = Vec::new();
        let observed_comparison = compare_with_moves(
            &motion,
            &path,
            &frame(),
            TimingRule::Makera,
            Tolerances::for_decimals(3, 3),
            &mut |m| {
                observed.push((m.group, m.segment, m.movement.to[3] + m.coordinate_shift));
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(observed_comparison, c);
        assert_eq!(
            observed,
            vec![
                (0, None, before - 10.0),
                (0, Some(0), before),
                (0, Some(2), before + 10.0),
            ]
        );
        let mut undeclared = path.clone();
        undeclared.groups[0].rotary_reset_after.clear();
        assert!(
            run(&text, &undeclared).is_err(),
            "an arbitrary gap must not authorize a reset"
        );
        let wrong = text.replacen(
            &format!("A{}\nM2", after + 10.0),
            &format!("A{}\nM2", after + 370.0),
            1,
        );
        assert!(
            run(&wrong, &path).is_err(),
            "whole-turn errors after a reset must be detected"
        );
    }
}

#[test]
fn req_gc_09_req_nfr_07_invalid_reset_metadata_refuses_before_facing_observation() {
    let mut g = group(710.0, 720.0);
    // A final segment cannot declare an internal pass boundary.
    g.rotary_reset_after = vec![0];
    let path = Toolpath { groups: vec![g] };
    let motion = replay(
        &read("G90 G21 G54\nG0 X0 Y0 Z30 A710\nA720\nM2\n").unwrap(),
        Dialect::MakeraZ1,
    )
    .unwrap();
    let mut observed = 0;
    let result = compare_with_moves(
        &motion,
        &path,
        &frame(),
        TimingRule::Makera,
        Tolerances::for_decimals(3, 3),
        &mut |_| {
            observed += 1;
            Ok(())
        },
    );
    assert!(result.is_err(), "malformed boundary metadata must refuse");
    assert_eq!(observed, 0, "invalid IR must not reach the observer");
}
