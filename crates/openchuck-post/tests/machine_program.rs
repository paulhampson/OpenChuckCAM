// SPDX-License-Identifier: AGPL-3.0-or-later
//! Neutral machine program and feed rules (GC-01, GC-04, GC-07, OP-13).
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing
)]

use std::collections::BTreeMap;

use openchuck_post::{
    ConvertOptions, FeedRule, FeedWord, MoveMetrics, PostError, Step, ToolSetting,
    to_machine_program,
};
use openchuck_types::geometry::{Point3, UnitVec3};
use openchuck_types::ir::{MoveKind, PathTags, Segment, ToolPose, Toolpath, ToolpathGroup};
use openchuck_types::machine::MachineProfile;
use openchuck_types::units::{Degrees, FeedRate, Millimetres, Seconds};

fn pose(x: f64, y: f64, z: f64, a: f64) -> ToolPose {
    ToolPose {
        tip: Point3::new(x, y, z),
        axis: UnitVec3::Z,
        a: Degrees(a),
    }
}

fn seg(kind: MoveKind, end: ToolPose, duration: f64) -> Segment {
    let fed = kind.is_fed();
    Segment {
        kind,
        end,
        contact_feed: fed.then_some(FeedRate(300.0)),
        duration: Seconds(duration),
        contact_length: Millimetres(0.0),
    }
}

fn path(segments: Vec<Segment>) -> Toolpath {
    Toolpath {
        groups: vec![ToolpathGroup {
            tags: PathTags {
                operation: "op1".into(),
                setup: "s1".into(),
                tool: "t1".into(),
            },
            start: pose(0.0, 0.0, 30.0, 0.0),
            segments,
        }],
    }
}

fn options(rule: FeedRule) -> ConvertOptions<MachineProfile> {
    let mut tools = BTreeMap::new();
    tools.insert(
        "t1".to_string(),
        ToolSetting {
            number: 3,
            name: "4 mm ball".into(),
            spindle_rpm: 10000.0,
        },
    );
    ConvertOptions {
        frame: MachineProfile::example_x_rotary(),
        feed_rule: rule,
        max_feed: None,
        tools,
    }
}

fn metrics(length: f64, delta_a: f64, end_radius: f64, duration: f64) -> MoveMetrics {
    MoveMetrics {
        length,
        delta_a,
        end_radius,
        contact_feed: 300.0,
        duration,
    }
}

fn per_minute(word: Option<FeedWord>) -> f64 {
    match word {
        Some(FeedWord::PerMinute(f)) => f,
        other => panic!("expected a per-minute feed, got {other:?}"),
    }
}

#[test]
fn req_gc_04_req_op_13_linuxcnc_rule_uses_xyz_length_or_a_travel() {
    // 10 mm in 2 s is 300 mm/min; an A-only move of 90 degrees in 3 s is 1800 deg/min.
    let f = per_minute(FeedRule::Linuxcnc.feed(&metrics(10.0, 45.0, 10.0, 2.0)));
    assert!((f - 300.0).abs() < 1e-9);
    let f = per_minute(FeedRule::Linuxcnc.feed(&metrics(0.0, 90.0, 10.0, 3.0)));
    assert!((f - 1800.0).abs() < 1e-9);
}

#[test]
fn req_gc_04_req_op_13_makera_rule_matches_the_firmware_formula() {
    // r = 10: P = 2 pi 10 + 30 = 92.83. Mixed, 10 mm and 90 degrees: max(10, 90 P / 360 = 23.2) = 23.2 mm.
    let p = 2.0 * std::f64::consts::PI * 10.0 + 30.0;
    let f = per_minute(FeedRule::Makera.feed(&metrics(10.0, 90.0, 10.0, 2.0)));
    assert!((f - 90.0 * p / 360.0 * 30.0).abs() < 1e-9, "{f}");
    // When the length is longer it wins.
    let f = per_minute(FeedRule::Makera.feed(&metrics(50.0, 90.0, 10.0, 2.0)));
    assert!((f - 1500.0).abs() < 1e-9);
    // A-only: dA * min(P, 360) / 360. A large radius caps P at 360.
    let f = per_minute(FeedRule::Makera.feed(&metrics(0.0, 90.0, 100.0, 3.0)));
    assert!((f - 90.0 * 20.0).abs() < 1e-9, "{f}");
}

#[test]
fn req_gc_04_inverse_time_and_contact_rules() {
    assert_eq!(
        FeedRule::InverseTime.feed(&metrics(10.0, 0.0, 10.0, 2.0)),
        Some(FeedWord::InverseTime(30.0))
    );
    assert_eq!(
        FeedRule::Contact.feed(&metrics(10.0, 0.0, 10.0, 2.0)),
        Some(FeedWord::PerMinute(300.0))
    );
    for rule in [
        FeedRule::Linuxcnc,
        FeedRule::Makera,
        FeedRule::InverseTime,
        FeedRule::Contact,
    ] {
        assert_eq!(FeedRule::parse(rule.name()), Some(rule));
        assert_eq!(
            rule.feed(&metrics(0.0, 0.0, 10.0, 1.0)),
            None,
            "no motion has no feed"
        );
    }
    assert_eq!(FeedRule::parse("g94"), None);
}

#[test]
fn req_gc_01_converts_moves_to_work_frame_positions_with_a() {
    let toolpath = path(vec![
        seg(MoveKind::Plunge, pose(5.0, 0.0, 12.0, 0.0), 1.0),
        seg(MoveKind::Cutting, pose(15.0, 0.0, 12.0, 0.0), 2.0),
    ]);
    let program = to_machine_program(&toolpath, &options(FeedRule::Linuxcnc)).unwrap();
    assert!(matches!(program.steps[0], Step::Comment { .. }));
    assert!(matches!(
        program.steps[1],
        Step::ToolChange { number: 3, .. }
    ));
    assert!(matches!(program.steps[2], Step::SpindleOn { rpm } if (rpm - 10000.0).abs() < 1e-9));
    let Step::Feed {
        kind: MoveKind::Cutting,
        feed,
        target,
    } = &program.steps[5]
    else {
        panic!("{:?}", program.steps);
    };
    assert_eq!(*feed, FeedWord::PerMinute(300.0));
    assert!((target.a).abs() < 1e-12);
}

#[test]
fn req_gc_01_a_rapid_by_whole_turns_at_the_same_tip_is_an_unwind() {
    let toolpath = path(vec![
        seg(MoveKind::Rapid, pose(0.0, 0.0, 30.0, 720.0), 0.5),
        seg(MoveKind::Rapid, pose(0.0, 0.0, 30.0, 740.0), 0.5),
    ]);
    let program = to_machine_program(&toolpath, &options(FeedRule::Linuxcnc)).unwrap();
    let unwinds = program
        .steps
        .iter()
        .filter(|s| matches!(s, Step::Unwind { .. }))
        .count();
    assert_eq!(unwinds, 1, "{:?}", program.steps);
    // A 20 degree turn is a real rapid.
    assert!(
        matches!(program.steps.last(), Some(Step::Rapid { target }) if (target.a - 740.0).abs() < 1e-12)
    );
}

#[test]
fn req_nfr_07_bad_input_is_refused_with_a_message() {
    let toolpath = path(vec![seg(MoveKind::Cutting, pose(5.0, 0.0, 12.0, 0.0), 1.0)]);
    let mut o = options(FeedRule::Linuxcnc);
    o.tools.clear();
    let e = to_machine_program(&toolpath, &o).unwrap_err();
    assert!(matches!(e, PostError::UnknownTool { .. }), "{e}");
    assert!(e.to_string().contains("t1"));

    let mut o = options(FeedRule::Linuxcnc);
    o.max_feed = Some(0.0);
    assert!(to_machine_program(&toolpath, &o).is_err());

    let zero = path(vec![seg(MoveKind::Cutting, pose(5.0, 0.0, 12.0, 0.0), 0.0)]);
    let e = to_machine_program(&zero, &options(FeedRule::Linuxcnc)).unwrap_err();
    assert!(e.to_string().contains("no duration"), "{e}");
}

#[test]
fn req_gc_04_max_feed_lowers_and_counts() {
    let toolpath = path(vec![seg(
        MoveKind::Cutting,
        pose(10.0, 0.0, 30.0, 0.0),
        1.0,
    )]);
    let mut o = options(FeedRule::Linuxcnc);
    o.max_feed = Some(400.0);
    let program = to_machine_program(&toolpath, &o).unwrap();
    assert_eq!(program.clamped_feeds, 1);
    assert!(
        matches!(program.steps.last(), Some(Step::Feed { feed: FeedWord::PerMinute(f), .. }) if (*f - 400.0).abs() < 1e-12)
    );
}

#[test]
fn req_gc_07_converting_twice_gives_the_same_program() {
    let toolpath = path(vec![
        seg(MoveKind::Cutting, pose(10.0, 3.0, 12.0, 33.0), 1.0),
        seg(MoveKind::Cutting, pose(12.0, 3.5, 12.0, 80.0), 1.0),
    ]);
    let a = to_machine_program(&toolpath, &options(FeedRule::Makera)).unwrap();
    let b = to_machine_program(&toolpath, &options(FeedRule::Makera)).unwrap();
    assert_eq!(a, b);
}
