// SPDX-License-Identifier: AGPL-3.0-or-later
//! GC-09/10: gate 2 keeps its existing budget for combined XYZ/A rounding.
#![allow(clippy::unwrap_used)]
use openchuck_gcode::{Dialect, TimingRule, Tolerances, compare, read, replay};
use openchuck_types::geometry::{Point3, UnitVec3};
use openchuck_types::ir::{PathTags, ToolPose, Toolpath, ToolpathGroup};
use openchuck_types::kinematics::RotaryFrame;
use openchuck_types::machine::{ASense, RotaryAxis};
use openchuck_types::units::Degrees;

#[test]
fn req_gc_09_req_gc_10_original_rotary_rounding_failure_stays_rejected() {
    // Original issue #324 pose, transformed independently by sin/cos about X.
    let path = Toolpath {
        groups: vec![ToolpathGroup {
            tags: PathTags {
                operation: "rounding".into(),
                setup: "s".into(),
                tool: "t".into(),
            },
            start: ToolPose {
                tip: Point3::new(21.558251369535647, 38.606261929758226, -23.12048468371006),
                axis: UnitVec3::Z,
                a: Degrees(120.93751838129586),
            },
            segments: vec![],
        }],
    };
    let frame = RotaryFrame {
        axis: RotaryAxis::X,
        sense: ASense::Positive,
        origin: Point3::ORIGIN,
    };
    for (dialect, timing) in [
        (Dialect::MakeraZ1, TimingRule::Makera),
        (Dialect::Linuxcnc, TimingRule::Linuxcnc),
    ] {
        let check = |y| {
            let text = format!("G90 G21\nG0 X21.558 Y{y} Z45.000 A120.938\nM2\n");
            let r = replay(&read(&text).unwrap(), dialect).unwrap();
            compare(&r, &path, &frame, timing, Tolerances::for_decimals(3, 3))
        };
        let error = check("-0.016").unwrap_err();
        assert!(error.to_string().contains("tip error"), "{error}");
        let corrected = check("-0.017").unwrap();
        assert!(corrected.max_position_error < 0.000867);
    }
}
