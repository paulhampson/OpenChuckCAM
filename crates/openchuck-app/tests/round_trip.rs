// SPDX-License-Identifier: AGPL-3.0-or-later
//! Gates 1 and 2: the posted G-code is read back, replayed through the shared
//! kinematics and compared with the IR (GC-01, GC-08, OP-13; verification plan
//! section 6; work breakdown slice 6 items 7 and 11).
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing
)]

mod support;

use openchuck_gcode::{
    CheckConfig, CompareError, Dialect, TimingRule, Tolerances, compare, read, replay, static_check,
};
use openchuck_post::{PostDefinition, PostProcessor, TemplatePost};
use openchuck_types::geometry::Point3;
use openchuck_types::kinematics::RotaryFrame;
use openchuck_types::machine::{ASense, RotaryAxis};
use support::*;

fn config() -> CheckConfig {
    CheckConfig {
        travel: [(-10.0, 200.0), (-200.0, 200.0), (-200.0, 100.0)],
        a_range: None,
        max_feed: 1200.0,
        safe_z: 12.0,
        spindle_off_at_tool_change: true,
        require_work_offset: true,
        allow_inverse_time: false,
    }
}

fn gate_2(
    text: &str,
    dialect: Dialect,
    rule: TimingRule,
    frame: RotaryFrame,
) -> Result<openchuck_gcode::Comparison, CompareError> {
    let program = read(text).unwrap();
    let replayed = replay(&program, dialect).unwrap();
    compare(
        &replayed,
        &small_part(),
        &frame,
        rule,
        Tolerances::for_decimals(3, 3),
    )
}

#[test]
fn req_gc_01_req_gc_10_linuxcnc_post_round_trips_within_rounding() {
    let text = builtin_post("linuxcnc")
        .post(&small_part(), &job())
        .unwrap()
        .text;
    let c = gate_2(&text, Dialect::Linuxcnc, TimingRule::Linuxcnc, job().frame).unwrap();
    assert!(c.poses > 25);
    assert!(c.max_position_error < 0.0009, "{c:?}");
    assert!(c.max_a_error < 0.001, "{c:?}");
    assert!((c.ir_a_travel - c.gcode_a_travel).abs() < 0.03, "{c:?}");
    assert!(
        (c.gcode_cut_seconds - c.ir_cut_seconds).abs() / c.ir_cut_seconds < 0.002,
        "{c:?}"
    );
}

#[test]
fn req_gc_01_req_gc_09_z1_post_round_trips_under_the_makera_rule() {
    // The unwind is `G92.4 A0 S0`: no motion, and the IR's whole-turn rapid is matched by it.
    let text = builtin_post("makera-z1")
        .post(&small_part(), &job())
        .unwrap()
        .text;
    let c = gate_2(&text, Dialect::MakeraZ1, TimingRule::Makera, job().frame).unwrap();
    assert!(c.max_position_error < 0.0009, "{c:?}");
    assert!(
        (c.gcode_cut_seconds - c.ir_cut_seconds).abs() / c.ir_cut_seconds < 0.005,
        "{c:?}"
    );
}

#[test]
fn req_op_13_the_naive_contact_feed_is_caught_by_the_time_check() {
    // SP-3 measured the raw contact feed 37 % too fast on rotary moves.
    let manifest = std::fs::read_to_string(repo("posts/linuxcnc/post.toml")).unwrap();
    let template = std::fs::read_to_string(repo("posts/linuxcnc/main.post")).unwrap();
    let naive = manifest.replace("feed_rule = \"linuxcnc\"", "feed_rule = \"contact\"");
    let post = TemplatePost::new(PostDefinition::from_parts("naive", &naive, &template).unwrap());
    let text = post.post(&small_part(), &job()).unwrap().text;
    let e = gate_2(&text, Dialect::Linuxcnc, TimingRule::Linuxcnc, job().frame).unwrap_err();
    assert!(matches!(e, CompareError::Tolerance(_)), "{e}");
    assert!(e.to_string().contains("time"), "{e}");
}

#[test]
fn req_gc_08_gate_2_catches_a_flipped_a_sign_and_a_wrong_frame() {
    let text = builtin_post("linuxcnc")
        .post(&small_part(), &job())
        .unwrap()
        .text;
    // The post wrote positive A as the profile defines it; reading it as the
    // opposite sense puts the tip somewhere else.
    let flipped = RotaryFrame {
        sense: ASense::Negative,
        ..job().frame
    };
    let e = gate_2(&text, Dialect::Linuxcnc, TimingRule::Linuxcnc, flipped).unwrap_err();
    assert!(e.to_string().contains("tip error"), "{e}");
    // A rotary axis offset the post did not know about.
    let moved = RotaryFrame {
        origin: Point3::new(0.0, 0.5, 0.0),
        axis: RotaryAxis::X,
        ..job().frame
    };
    assert!(gate_2(&text, Dialect::Linuxcnc, TimingRule::Linuxcnc, moved).is_err());
}

#[test]
fn req_gc_08_gate_2_catches_a_changed_position_and_a_dropped_move() {
    let text = builtin_post("linuxcnc")
        .post(&small_part(), &job())
        .unwrap()
        .text;
    let bad = text.replacen("X1.000 Y0.000 Z-10.000", "X1.002 Y0.000 Z-10.000", 1);
    assert_ne!(bad, text);
    let e = gate_2(&bad, Dialect::Linuxcnc, TimingRule::Linuxcnc, job().frame).unwrap_err();
    assert!(e.to_string().contains("tip error"), "{e}");
    let dropped: String = text
        .lines()
        .filter(|l| !l.starts_with("X1.000 "))
        .map(|l| format!("{l}\n"))
        .collect();
    assert!(
        gate_2(
            &dropped,
            Dialect::Linuxcnc,
            TimingRule::Linuxcnc,
            job().frame
        )
        .is_err()
    );
}

#[test]
fn req_gc_08_gate_1_passes_both_posts_and_fails_seeded_faults() {
    let c = config();
    for (id, dialect) in [
        ("linuxcnc", Dialect::Linuxcnc),
        ("makera-z1", Dialect::MakeraZ1),
    ] {
        let text = builtin_post(id).post(&small_part(), &job()).unwrap().text;
        let r = replay(&read(&text).unwrap(), dialect).unwrap();
        assert_eq!(static_check(&r, &c), [], "{id}");
        // A feed removed from the first cutting move.
        let no_feed = text.replacen(" F100.0", "", 1);
        let r = replay(&read(&no_feed).unwrap(), dialect).unwrap();
        assert!(
            static_check(&r, &c)
                .iter()
                .any(|f| f.kind == openchuck_gcode::FindingKind::MissingFeed),
            "{id}"
        );
        // Wrong units.
        let inches = text.replace("G21", "G20");
        let r = replay(&read(&inches).unwrap(), dialect).unwrap();
        assert!(
            static_check(&r, &c)
                .iter()
                .any(|f| f.kind == openchuck_gcode::FindingKind::Units),
            "{id}"
        );
        // A beyond a limit.
        let limited = CheckConfig {
            a_range: Some((-90.0, 90.0)),
            ..config()
        };
        let r = replay(&read(&text).unwrap(), dialect).unwrap();
        assert!(
            static_check(&r, &limited)
                .iter()
                .any(|f| f.kind == openchuck_gcode::FindingKind::AxisLimit),
            "{id}"
        );
    }
}
