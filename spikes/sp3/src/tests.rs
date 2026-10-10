//! SP-3 pass-criteria checks. Names carry requirement IDs (AGENTS.md convention).

use crate::ir::Kind;
use crate::machine::Machine;
use crate::oracle::{ang_diff, Oracle};
use crate::path::{self, design, model_point, TIP_R};
use crate::post::{self, FeedMode};
use crate::reader::{self, MoveKind};
use crate::{assumed_time_min, oracle_for};

fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

const M: Machine = Machine::REFERENCE;

fn replayed(mode: FeedMode) -> (path::Plan, reader::Replay) {
    let plan = path::plan(M.a_sign);
    let text = post::post(&plan.segments, &M, mode).text;
    let rep = reader::replay(&text).expect("parse");
    (plan, rep)
}

/// Kinematic round trip: shared model vs independent oracle, negative A and the 0/360 seam (OP-05, OP-06).
#[test]
fn req_op_05_oracle_agrees_with_shared_model() {
    let o = oracle_for(&M);
    for &a in &[-725.0, -540.0, -360.0, -359.999, -180.0, -90.0, -1e-9, 0.0, 1e-9, 90.0, 179.999, 180.0, 359.999, 360.0, 360.001, 540.0, 725.0] {
        for &p in &[[10.0, 14.8, 0.0], [-3.0, 0.0, 14.8], [7.0, -9.0, -9.0], [0.0, 0.0, 0.0]] {
            let machine = M.to_machine(p, a);
            let back = o.tool_tip_in_model(machine[0], machine[1], machine[2], a);
            assert!(dist(back, p) < 1e-9, "A={a} p={p:?} back={back:?}");
            // And the shared inverse agrees with the oracle.
            let back2 = M.to_model(machine, a);
            assert!(dist(back, back2) < 1e-9);
        }
    }
}

/// A flipped A sign in the shared model must be caught by the oracle (slice 0 review item).
#[test]
fn req_op_06_oracle_detects_flipped_a_sign() {
    let o = oracle_for(&M);
    let wrong = Machine { a_sign: -M.a_sign, ..M };
    let p = [10.0, 14.8, 3.0];
    let machine = wrong.to_machine(p, 37.0);
    let seen = o.tool_tip_in_model(machine[0], machine[1], machine[2], 37.0);
    assert!(dist(seen, p) > 1.0, "flip not detected: {seen:?}");
    // A symmetric part hides it: a point on the axis does not move.
    let axis_pt = [10.0, 0.0, 0.0];
    let m2 = wrong.to_machine(axis_pt, 37.0);
    assert!(dist(o.tool_tip_in_model(m2[0], m2[1], m2[2], 37.0), axis_pt) < 1e-9);
}

/// Closed-form oracle: top-contact A and XYZ agree with what the planner/post produce.
#[test]
fn req_op_05_closed_form_top_contact() {
    let o = oracle_for(&M);
    let plan = path::plan(M.a_sign);
    let mut cuts = 0;
    for s in plan.segments.iter().filter(|s| s.kind == Kind::Cut) {
        let theta = s.tip[2].atan2(s.tip[1]).to_degrees();
        let (xyz_exp, a_exp) = o.expected_top_contact(s.tip[0], theta, TIP_R);
        let xyz = M.to_machine(s.tip, s.a_deg);
        assert!(dist(xyz, xyz_exp) < 1e-9, "{xyz:?} vs {xyz_exp:?}");
        assert!(ang_diff(s.a_deg, a_exp).abs() < 1e-9);
        cuts += 1;
    }
    assert!(cuts > 100);
}

/// Tool axis stays vertical (-Z in machine) for every segment of the 3+1 wrap (OP-05).
#[test]
fn req_op_05_tool_axis_vertical_in_machine() {
    for s in path::plan(M.a_sign).segments.iter() {
        let ax = M.axis_to_machine(s.axis, s.a_deg);
        assert!(dist(ax, [0.0, 0.0, 1.0]) < 1e-12, "{ax:?}"); // tip->holder is +Z
    }
}

/// Posted G-code, read back, reaches the IR positions (GC-02, GC-08, GC-10 replay gate).
#[test]
fn req_gc_08_replay_matches_ir_nodes() {
    for mode in [FeedMode::Compensated, FeedMode::InverseTime] {
        let (plan, rep) = replayed(mode);
        assert_eq!(rep.moves.len(), plan.segments.len());
        let o = oracle_for(&M);
        let mut worst: f64 = 0.0;
        for (mv, s) in rep.moves.iter().zip(&plan.segments) {
            let tip = o.tool_tip_in_model(mv.to.xyz[0], mv.to.xyz[1], mv.to.xyz[2], mv.to.a);
            worst = worst.max(dist(tip, s.tip));
            assert_eq!(mv.kind == MoveKind::Rapid, s.contact_feed.is_none());
        }
        assert!(worst < 1e-4, "{mode:?} worst={worst}"); // 4-decimal output precision
    }
}

/// Between nodes the machine interpolates X and A linearly; the real motion is a helix. Its
/// distance from the intended sinusoid must stay inside the toolpath tolerance (NFR-05).
#[test]
fn req_nfr_05_path_between_nodes_within_tolerance() {
    let (plan, rep) = replayed(FeedMode::Compensated);
    let o = oracle_for(&M);
    let cuts: Vec<_> = rep.moves.iter().zip(&plan.segments).filter(|(_, s)| s.kind == Kind::Cut).collect();
    let mut worst: f64 = 0.0;
    for (i, (mv, _)) in cuts.iter().enumerate() {
        let u0 = if i == 0 { 0.0 } else { plan.cut_u[i - 1] };
        let u1 = plan.cut_u[i];
        for k in 1..8 {
            let t = k as f64 / 8.0;
            let lerp = |a: f64, b: f64| a + t * (b - a);
            let p = o.tool_tip_in_model(
                lerp(mv.from.xyz[0], mv.to.xyz[0]),
                lerp(mv.from.xyz[1], mv.to.xyz[1]),
                lerp(mv.from.xyz[2], mv.to.xyz[2]),
                lerp(mv.from.a, mv.to.a),
            );
            // Nearest point on the true curve, searched densely inside this segment's u range.
            let best = (0..=400)
                .map(|j| {
                    let u = u0 + (u1 - u0) * j as f64 / 400.0;
                    let (x, th) = design(u);
                    dist(p, model_point(x, th, TIP_R))
                })
                .fold(f64::MAX, f64::min);
            worst = worst.max(best);
        }
    }
    assert!(worst < 0.01, "worst={worst}");
    eprintln!("max deviation between nodes: {worst:.5} mm");
}

/// Wrap and unwinding (OP-15): A is continuous through the 0/360 seam, the cut spans more than a
/// turn, and the final unwind returns to the same model-frame point.
#[test]
fn req_op_15_wrap_and_unwind() {
    let plan = path::plan(M.a_sign);
    let cuts: Vec<_> = plan.segments.iter().filter(|s| s.kind == Kind::Cut).collect();
    let mut prev = plan.segments[1].a_deg;
    for s in &cuts {
        assert!((s.a_deg - prev).abs() < 45.0, "A jump of {} deg inside a cut", s.a_deg - prev);
        prev = s.a_deg;
    }
    let span = cuts.last().unwrap().a_deg - cuts[0].a_deg;
    assert!((span.abs() - (540.0 - 0.0)).abs() < 6.0, "span={span}");
    // Unwind: last two segments are the retract then a rapid A-only change by whole turns.
    let n = plan.segments.len();
    let (ret, unw) = (&plan.segments[n - 2], &plan.segments[n - 1]);
    let turns = (ret.a_deg - unw.a_deg) / 360.0;
    assert!((turns - turns.round()).abs() < 1e-9 && turns.round() != 0.0, "turns={turns}");
    assert!(dist(ret.tip, unw.tip) < 1e-12);
    assert!(unw.a_deg.abs() <= 180.0);
    // Same physical position before and after, modulo whole turns.
    let m1 = M.to_machine(ret.tip, ret.a_deg);
    let m2 = M.to_machine(unw.tip, unw.a_deg);
    assert!(dist(m1, m2) < 1e-9);
}

/// Compensated feed (OP-13): the contact point moves at the IR feed in both G94 and G93, and the
/// naive baseline (raw feed, ignoring rotation) is wrong by a large, measurable amount.
#[test]
fn req_op_13_compensated_feed_at_contact_point() {
    let totals = |mode| {
        let (plan, rep) = replayed(mode);
        let (mut ir, mut re) = (0.0, 0.0);
        for (mv, s) in rep.moves.iter().zip(&plan.segments) {
            if s.kind == Kind::Cut {
                ir += s.duration_s.unwrap();
                re += assumed_time_min(mv).unwrap() * 60.0;
            }
        }
        (ir, re)
    };
    for mode in [FeedMode::Compensated, FeedMode::InverseTime] {
        let (ir, re) = totals(mode);
        assert!((re / ir - 1.0).abs() < 0.001, "{mode:?}: ir={ir} replay={re}");
    }
    let (ir, re) = totals(FeedMode::NaiveContactFeed);
    assert!((re / ir - 1.0).abs() > 0.2, "naive baseline should be clearly off, got {}", re / ir);
}

/// Contact-point speed measured in the model frame from replayed positions equals the IR feed.
#[test]
fn req_op_13_contact_speed_from_replay() {
    let (plan, rep) = replayed(FeedMode::InverseTime);
    let o = oracle_for(&M);
    for (mv, s) in rep.moves.iter().zip(&plan.segments).filter(|(_, s)| s.kind == Kind::Cut) {
        let a = o.tool_tip_in_model(mv.from.xyz[0], mv.from.xyz[1], mv.from.xyz[2], mv.from.a);
        let b = o.tool_tip_in_model(mv.to.xyz[0], mv.to.xyz[1], mv.to.xyz[2], mv.to.a);
        let chord = dist(a, b); // model-frame chord <= helix length
        let t_min = assumed_time_min(mv).unwrap();
        let speed = chord / t_min;
        // Chord is within chord-tolerance sag of the helix; allow 0.5 %.
        assert!((speed / s.contact_feed.unwrap() - 1.0).abs() < 0.01, "speed {speed}");
    }
}

/// SP-5 overlap: every code our post emits is parsed, with line numbers.
#[test]
fn req_gc_08_reader_parses_every_emitted_code() {
    let (_, rep) = replayed(FeedMode::InverseTime);
    for c in ["G21", "G90", "G17", "G54", "G94", "G93", "T1", "M6", "M3", "M5", "M30", "G0", "G1"] {
        assert!(rep.codes_seen.iter().any(|x| x == c), "missing {c}: {:?}", &rep.codes_seen[..12.min(rep.codes_seen.len())]);
    }
    assert_eq!(rep.comments, 1);
    assert!(rep.moves.iter().all(|m| m.line > 4));
}

/// Determinism (GC-07, NFR-06): same input, same bytes.
#[test]
fn req_gc_07_output_is_deterministic() {
    let a = post::post(&path::plan(M.a_sign).segments, &M, FeedMode::Compensated).text;
    let b = post::post(&path::plan(M.a_sign).segments, &M, FeedMode::Compensated).text;
    assert_eq!(a, b);
}

/// The sign flip also changes the G-code, and replaying it through the oracle exposes the error.
#[test]
fn req_gc_09_wrong_a_sign_in_post_fails_oracle_replay() {
    let plan = path::plan(M.a_sign);
    let wrong = Machine { a_sign: -M.a_sign, ..M };
    let text = post::post(&plan.segments, &wrong, FeedMode::Compensated).text;
    let rep = reader::replay(&text).unwrap();
    let o = oracle_for(&M);
    let worst = rep
        .moves
        .iter()
        .zip(&plan.segments)
        .map(|(mv, s)| dist(o.tool_tip_in_model(mv.to.xyz[0], mv.to.xyz[1], mv.to.xyz[2], mv.to.a), s.tip))
        .fold(0.0, f64::max);
    assert!(worst > 1.0, "worst={worst}");
}

/// Oracle type used without the shared model compiles standalone (documentation of independence).
#[test]
fn oracle_has_no_machine_dependency() {
    let o = Oracle { sigma: 1.0, axis_y: 0.0, axis_z: 0.0 };
    let p = o.tool_tip_in_model(1.0, 0.0, 10.0, 90.0); // machine +Z at A=90 -> model +Y? check sign
    assert!(dist(p, [1.0, 10.0, 0.0]) < 1e-9, "{p:?}");
}
