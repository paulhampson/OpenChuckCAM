// SPDX-License-Identifier: AGPL-3.0-or-later
//! Exact combining of consecutive pure-A blocks (GC-04 post option
//! `combine_rotary_runs`, GC-17; decision 0648, accepted; issue 503).
//!
//! A run of fed moves that turn A only, at the same written X, Y and Z, in
//! the same direction, of the same kind and with the same written feed word,
//! is written as one block: the controller turns through the same angles at
//! the same rate, but plans one motion instead of one per step.
#![allow(clippy::unwrap_used, clippy::float_cmp, clippy::indexing_slicing)]
mod support;

use openchuck_post::{
    ADirection, BlockTimeSettings, PostDefinition, PostProcessor, TemplatePost, builtin,
};
use openchuck_types::geometry::Point3;
use openchuck_types::ir::{MoveKind, PathTags, Segment, ToolPose, Toolpath, ToolpathGroup};
use openchuck_types::kinematics::{
    MachinePosition, RotaryFrame, tool_axis_in_model, tool_tip_in_model,
};
use openchuck_types::machine::{ASense, RotaryAxis, ShortBlockMode};
use openchuck_types::units::{Degrees, FeedRate, Millimetres, Seconds};

fn frame() -> RotaryFrame {
    RotaryFrame {
        axis: RotaryAxis::X,
        sense: ASense::Positive,
        origin: Point3::ORIGIN,
    }
}

/// The IR pose with the tool at machine X `x`, Z `z` (Y 0) and A `a`.
fn at(x: f64, z: f64, a: f64) -> ToolPose {
    ToolPose {
        tip: tool_tip_in_model(
            &frame(),
            &MachinePosition {
                xyz: Point3::new(x, 0.0, z),
                a: Degrees(a),
            },
        ),
        axis: tool_axis_in_model(&frame(), Degrees(a)),
        a: Degrees(a),
    }
}

fn seg(kind: MoveKind, end: ToolPose, duration: f64) -> Segment {
    Segment {
        kind,
        end,
        contact_feed: kind.is_fed().then_some(FeedRate(300.0)),
        duration: Seconds(duration),
        contact_length: Millimetres(1.0),
    }
}

/// A plunge to Z10 at X5, then `steps` (end A, seconds, kind, Z) and a
/// retract.
fn path(steps: &[(f64, f64, MoveKind, f64)]) -> Toolpath {
    let mut segments = vec![seg(MoveKind::Plunge, at(5.0, 10.0, 0.0), 1.0)];
    for &(a, t, kind, z) in steps {
        segments.push(seg(kind, at(5.0, z, a), t));
    }
    let last = steps.last().map_or(0.0, |s| s.0);
    segments.push(seg(MoveKind::Linking, at(5.0, 30.0, last), 1.0));
    Toolpath {
        groups: vec![ToolpathGroup {
            rotary_reset_after: Vec::new(),
            tags: PathTags {
                operation: "ring".into(),
                setup: "s1".into(),
                tool: "t1".into(),
            },
            start: at(5.0, 30.0, 0.0),
            segments,
        }],
    }
}

/// A whole turn in `n` equal cutting steps, `seconds` each, at Z10.
fn ring(n: usize, seconds: f64) -> Vec<(f64, f64, MoveKind, f64)> {
    let step = 360.0 / n as f64;
    (1..=n)
        .map(|k| (step * k as f64, seconds, MoveKind::Cutting, 10.0))
        .collect()
}

fn post(id: &str, combine: bool) -> TemplatePost {
    let mut definition = builtin(id).unwrap().unwrap();
    definition.options.combine_rotary_runs = combine;
    TemplatePost::new(definition)
}

/// The written blocks that move A only: A (and perhaps F) and nothing else,
/// whether or not the block repeats a modal `G1`.
fn pure_a_lines(text: &str) -> Vec<String> {
    text.lines()
        .filter(|line| {
            let words: Vec<&str> = line.split_whitespace().filter(|w| *w != "G1").collect();
            !words.is_empty()
                && words.iter().any(|w| w.starts_with('A'))
                && words
                    .iter()
                    .all(|w| w.starts_with('A') || w.starts_with('F'))
        })
        .map(str::to_owned)
        .collect()
}

#[test]
fn req_gc_04_combine_rotary_runs_writes_an_equal_feed_ring_as_one_block() {
    for id in ["makera-z1", "linuxcnc"] {
        let p = path(&ring(72, 0.1));
        let job = support::job();
        let on = post(id, true).post(&p, &job).unwrap();
        let off = post(id, false).post(&p, &job).unwrap();
        let on_lines = pure_a_lines(&on.text);
        assert_eq!(pure_a_lines(&off.text).len(), 72, "{id}");
        assert_eq!(on_lines.len(), 1, "{id}: {on_lines:?}");
        assert!(on_lines[0].starts_with("A360.000"), "{id}: {on_lines:?}");
        // The same rate, so the same estimated time and no change of feed.
        assert!(
            (on.estimated_seconds - off.estimated_seconds).abs() < 1e-9,
            "{id}"
        );
        assert_eq!(
            on.text.lines().count() + 71,
            off.text.lines().count(),
            "{id}"
        );
        // Deterministic (GC-07).
        assert_eq!(on.text, post(id, true).post(&p, &job).unwrap().text);
    }
}

#[test]
fn req_gc_04_combine_rotary_runs_is_on_in_the_built_in_posts_and_off_when_a_post_omits_it() {
    for id in ["makera-z1", "linuxcnc"] {
        assert!(
            builtin(id).unwrap().unwrap().options.combine_rotary_runs,
            "{id}"
        );
    }
    let dir = support::posts_path("makera-z1");
    let manifest = std::fs::read_to_string(dir.join("post.toml")).unwrap();
    let template = std::fs::read_to_string(dir.join("main.post")).unwrap();
    let without: String = manifest
        .lines()
        .filter(|l| !l.starts_with("combine_rotary_runs"))
        .map(|l| format!("{l}\n"))
        .collect();
    assert_ne!(without, manifest);
    let definition = PostDefinition::from_parts("test", &without, &template).unwrap();
    assert!(!definition.options.combine_rotary_runs);
    let p = path(&ring(72, 0.1));
    let text = TemplatePost::new(definition)
        .post(&p, &support::job())
        .unwrap()
        .text;
    assert_eq!(pure_a_lines(&text).len(), 72);
}

#[test]
fn req_gc_04_combine_rotary_runs_keeps_a_full_turn_going_the_same_way_round() {
    // Backwards round the part, and forwards written with A reversed: the
    // combined block's absolute A must still be a whole turn away, never the
    // start angle again (which the controller would read as no motion).
    let back: Vec<_> = ring(72, 0.1)
        .iter()
        .map(|s| (-s.0, s.1, s.2, s.3))
        .collect();
    let text = post("makera-z1", true)
        .post(&path(&back), &support::job())
        .unwrap()
        .text;
    let lines = pure_a_lines(&text);
    assert_eq!(lines.len(), 1);
    assert!(lines[0].starts_with("A-360.000"), "{lines:?}");

    let mut definition = builtin("linuxcnc").unwrap().unwrap();
    definition.options.combine_rotary_runs = true;
    definition.options.a_direction = ADirection::Reversed;
    let text = TemplatePost::new(definition)
        .post(&path(&ring(72, 0.1)), &support::job())
        .unwrap()
        .text;
    let lines = pure_a_lines(&text);
    assert_eq!(lines.len(), 1);
    assert!(lines[0].starts_with("A-360.000"), "{lines:?}");

    // Two whole turns in one run stay two turns.
    let mut two = ring(72, 0.1);
    two.extend(ring(72, 0.1).iter().map(|s| (s.0 + 360.0, s.1, s.2, s.3)));
    let text = post("linuxcnc", true)
        .post(&path(&two), &support::job())
        .unwrap()
        .text;
    let lines = pure_a_lines(&text);
    assert_eq!(lines.len(), 1);
    assert!(lines[0].starts_with("A720.000"), "{lines:?}");
}

#[test]
fn req_gc_04_combine_rotary_runs_splits_where_anything_else_changes() {
    use MoveKind::{Cutting, Linking};
    let job = support::job();
    let count = |steps: &[(f64, f64, MoveKind, f64)]| {
        pure_a_lines(
            &post("makera-z1", true)
                .post(&path(steps), &job)
                .unwrap()
                .text,
        )
        .len()
    };
    // A change of feed: the second half turns at half the rate.
    let mut feed: Vec<_> = (1..=4)
        .map(|k| (5.0 * k as f64, 0.1, Cutting, 10.0))
        .collect();
    feed.extend((5..=8).map(|k| (5.0 * k as f64, 0.2, Cutting, 10.0)));
    assert_eq!(count(&feed), 2);
    // A change of Z in the middle: two pure-A runs either side of it.
    let mut z: Vec<_> = (1..=4)
        .map(|k| (5.0 * k as f64, 0.1, Cutting, 10.0))
        .collect();
    z.push((20.0, 0.1, Cutting, 9.0));
    z.extend((5..=8).map(|k| (5.0 * k as f64, 0.1, Cutting, 9.0)));
    assert_eq!(count(&z), 2);
    // A change of kind.
    let mut kind: Vec<_> = (1..=4)
        .map(|k| (5.0 * k as f64, 0.1, Cutting, 10.0))
        .collect();
    kind.extend((5..=8).map(|k| (5.0 * k as f64, 0.1, Linking, 10.0)));
    assert_eq!(count(&kind), 2);
    // A reversal: forwards to 40, then back to 20.
    let mut turn: Vec<_> = (1..=8)
        .map(|k| (5.0 * k as f64, 0.1, Cutting, 10.0))
        .collect();
    turn.extend((1..=4).map(|k| (40.0 - 5.0 * k as f64, 0.1, Cutting, 10.0)));
    assert_eq!(count(&turn), 2);
}

#[test]
fn req_gc_04_combine_rotary_runs_does_not_cross_a_rotary_counter_reset() {
    // Two turns with a declared clear boundary between them (after a
    // retract, as the IR requires): the Z1 post writes its counter reset
    // there, and each turn is one block on its own side of it.
    // The first part turns twice, so the reset has whole turns to drop.
    let mut first = ring(72, 0.1);
    first.extend(ring(72, 0.1).iter().map(|s| (s.0 + 360.0, s.1, s.2, s.3)));
    let mut p = path(&first);
    let segments = &mut p.groups[0].segments;
    let retract = segments.len() - 1;
    segments.push(seg(MoveKind::Plunge, at(5.0, 10.0, 720.0), 1.0));
    for (a, t, kind, z) in ring(72, 0.1) {
        segments.push(seg(kind, at(5.0, z, a + 720.0), t));
    }
    segments.push(seg(MoveKind::Linking, at(5.0, 30.0, 1080.0), 1.0));
    p.groups[0].rotary_reset_after = vec![retract];
    let text = post("makera-z1", true)
        .post(&p, &support::job())
        .unwrap()
        .text;
    let lines = pure_a_lines(&text);
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(lines[0].starts_with("A720.000"), "{lines:?}");
    assert!(lines[1].starts_with("A360.000"), "{lines:?}");
    let reset = text.find("G92.4 A0 S0").unwrap();
    let first = text.find(&lines[0]).unwrap();
    let second = text.rfind(&lines[1]).unwrap();
    assert!(first < reset && reset < second, "{text}");
}

fn settings(mode: ShortBlockMode) -> BlockTimeSettings {
    BlockTimeSettings {
        min_block_ms: 7.0,
        block_window: 1,
        mode,
    }
}

#[test]
fn req_gc_17_report_counts_the_blocks_as_written_after_combining() {
    // Thirty 1 degree steps of 2 ms each: short blocks one by one, but one
    // 60 ms block once combined.
    let steps: Vec<_> = (1..=30)
        .map(|k| (k as f64, 0.002, MoveKind::Cutting, 10.0))
        .collect();
    let p = path(&steps);
    let mut job = support::job();
    job.block_time = settings(ShortBlockMode::Report);
    let off = post("makera-z1", false).post(&p, &job).unwrap();
    let on = post("makera-z1", true).post(&p, &job).unwrap();
    assert_eq!(off.short_blocks[0].short_moves, 30);
    assert_eq!(on.short_blocks[0].short_moves, 0);
    assert_eq!(on.short_blocks[0].added_seconds, 0.0);
}

#[test]
fn req_gc_17_slow_down_spreads_a_combined_blocks_time_over_its_segments() {
    // Three 1 degree steps of 1 ms each: still short once combined (3 ms),
    // so Slow down lowers the one combined block's feed to last 7 ms, and the
    // verification path's three segments share that time in proportion.
    let steps: Vec<_> = (1..=3)
        .map(|k| (k as f64, 0.001, MoveKind::Cutting, 10.0))
        .collect();
    let p = path(&steps);
    let mut job = support::job();
    job.block_time = settings(ShortBlockMode::SlowDown);
    let on = post("makera-z1", true).post(&p, &job).unwrap();
    assert_eq!(pure_a_lines(&on.text).len(), 1);
    assert_eq!(on.short_blocks[0].slowed_moves, 1);
    let original = &p.groups[0].segments;
    let adjusted = &on.verification_path.groups[0].segments;
    for (a, b) in original.iter().zip(adjusted) {
        assert_eq!(a.end, b.end);
        assert_eq!(a.kind, b.kind);
    }
    let ring: f64 = adjusted[1..=3].iter().map(|s| s.duration.0).sum();
    assert!(ring >= 0.007 - 1e-9, "{ring}");
    assert!(ring < 0.0075, "{ring}");
    for s in &adjusted[1..=3] {
        assert!((s.duration.0 - ring / 3.0).abs() < 1e-9);
    }
}
