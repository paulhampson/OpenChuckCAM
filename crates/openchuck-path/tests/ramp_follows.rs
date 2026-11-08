// SPDX-License-Identifier: AGPL-3.0-or-later
//! A ramp entry follows the pass's own toolpath (OP-11, OP-13, OP-15,
//! SET-09; decision record 0543, proposed; issue #639). Synthetic fixtures:
//! the cylinder and cone of the issue's probe, not the owner's projects.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing
)]

mod support;

use std::collections::BTreeMap;

use openchuck_path::params::names;
use openchuck_path::roughing::parameter as roughing;
use openchuck_path::{
    GeneratedPath, HelicalFinishing, MeshSurface, ParameterValue, RotaryRoughing, Strategy,
};
use openchuck_testkit::parts::{Cone, Cylinder, RadialPart};
use openchuck_types::ir::{MoveKind, ToolPose};
use openchuck_types::tool::ToolShape;
use support::{Fixture, ball, generate, run, tool};

const ANGLE: f64 = 3.0;

fn n(x: f64) -> ParameterValue {
    ParameterValue::Number(x)
}

fn t(s: &str) -> ParameterValue {
    ParameterValue::Text(s.to_owned())
}

fn finishing(shape: &str) -> BTreeMap<String, ParameterValue> {
    let mut v = BTreeMap::new();
    v.insert(names::STEPOVER.into(), n(2.0));
    v.insert(names::FEED.into(), n(300.0));
    v.insert(names::PLUNGE_FEED.into(), n(100.0));
    v.insert(names::ENTRY.into(), t("ramp"));
    v.insert(names::RAMP_ANGLE.into(), n(ANGLE));
    v.insert("pass_shape".into(), t(shape));
    v
}

fn roughing_values(shape: &str) -> BTreeMap<String, ParameterValue> {
    let mut v = BTreeMap::new();
    v.insert(names::STEPOVER.into(), n(3.0));
    v.insert(names::FEED.into(), n(600.0));
    v.insert(names::PLUNGE_FEED.into(), n(200.0));
    v.insert(names::STOCK_TO_LEAVE.into(), n(0.5));
    v.insert(roughing::STEPDOWN.into(), n(2.0));
    v.insert(names::ENTRY.into(), t("ramp"));
    v.insert(names::RAMP_ANGLE.into(), n(ANGLE));
    v.insert("pass_shape".into(), t(shape));
    v
}

fn cylinder() -> Fixture {
    let cyl = Cylinder {
        radius: 15.0,
        length: 40.0,
    };
    Fixture::new(
        MeshSurface::new(&cyl.mesh(3, 72)).unwrap(),
        16.0,
        &ball(6.0),
    )
}

fn cone() -> Fixture {
    let cone = Cone {
        start_radius: 8.0,
        end_radius: 12.0,
        length: 30.0,
    };
    Fixture::new(
        MeshSurface::new(&cone.mesh(40, 180)).unwrap(),
        16.0,
        &tool(ToolShape::FlatEnd, 6.0),
    )
}

fn make(
    f: &Fixture,
    s: &dyn Strategy,
    v: &BTreeMap<String, ParameterValue>,
    keep: &[(f64, f64)],
) -> std::result::Result<GeneratedPath, String> {
    let mut input = f.input();
    input.keep_out = keep;
    run(|ctx| generate(s, &input, v, ctx)).map_err(|e| e.to_string())
}

/// Each entry: the pose before it and its `LeadIn` and `Plunge` moves, up
/// to the first cutting move after it.
fn entries(path: &GeneratedPath) -> Vec<(ToolPose, Vec<ToolPose>, ToolPose)> {
    let segs = &path.group.segments;
    let mut out = Vec::new();
    let mut prev = path.group.start;
    let mut i = 0;
    while i < segs.len() {
        if matches!(segs[i].kind, MoveKind::LeadIn | MoveKind::Plunge) {
            let before = prev;
            let mut moves = Vec::new();
            while i < segs.len() && matches!(segs[i].kind, MoveKind::LeadIn | MoveKind::Plunge) {
                moves.push(segs[i].end);
                prev = segs[i].end;
                i += 1;
            }
            out.push((before, moves, prev));
            continue;
        }
        prev = segs[i].end;
        i += 1;
    }
    out
}

/// Checks one ring entry: it holds the first cut point's X and Y, turns A
/// one way only (the pass's), descends no more steeply than the angle on
/// the tip's real path (arc length at the tip's radius), and ends at the
/// first cut point.
fn assert_ring_ramp(f: &Fixture, before: ToolPose, moves: &[ToolPose], head: ToolPose, sign: f64) {
    assert!(moves.len() > 1, "a ring ramp has more than one move");
    let goal = f.rotary.to_axis(head.tip, head.a);
    let tan = ANGLE.to_radians().tan();
    let mut p = before;
    for m in moves {
        let a = f.rotary.to_axis(m.tip, m.a);
        assert!(
            (a.along - goal.along).abs() < 1e-6,
            "X held: {} vs {}",
            a.along,
            goal.along
        );
        assert!((a.across - goal.across).abs() < 1e-6, "Y held");
        let da = m.a.0 - p.a.0;
        assert!(da * sign >= -1e-9, "A turns the pass's way only: {da}");
        let q = f.rotary.to_axis(p.tip, p.a);
        let radius = a.height.min(q.height);
        let run = da.abs().to_radians() * radius;
        let drop = q.height - a.height;
        assert!(
            drop <= tan * run + 1e-6,
            "descent within {ANGLE} degrees: drop {drop} over {run}"
        );
        p = *m;
    }
    assert!(p.tip.distance(head.tip) < 1e-9 && (p.a.0 - head.a.0).abs() < 1e-9);
    let first = f.rotary.to_axis(moves[0].tip, moves[0].a);
    let start = f.rotary.to_axis(before.tip, before.a);
    assert!(
        (start.along - goal.along).abs() < 1e-6,
        "the ramp starts over the ring"
    );
    assert!(first.height < start.height + 1e-9);
}

/// The way A turns on the first cutting move after `head` that turns it.
fn pass_sign(path: &GeneratedPath, head: ToolPose) -> f64 {
    let segs = &path.group.segments;
    let at = segs
        .iter()
        .position(|s| s.kind == MoveKind::LeadIn && s.end == head)
        .unwrap();
    segs[at..]
        .iter()
        .find(|s| s.kind == MoveKind::Cutting && (s.end.a.0 - head.a.0).abs() > 1e-6)
        .map(|s| (s.end.a.0 - head.a.0).signum())
        .unwrap()
}

#[test]
fn req_op_11_req_op_15_a_ring_is_ramped_around_itself_turning_the_pass_s_way() {
    let f = cylinder();
    let path = make(&f, &HelicalFinishing, &finishing("rings"), &[]).unwrap();
    let all = entries(&path);
    assert!(!all.is_empty());
    let (before, moves, head) = &all[0];
    assert_ring_ramp(&f, *before, moves, *head, pass_sign(&path, *head));
}

#[test]
fn req_op_11_req_set_09_a_ring_ramp_beside_a_held_region_generates() {
    // The issue's probe: refused on main, the ramp running back along X
    // over the held region.
    let f = cylinder();
    let path = make(&f, &HelicalFinishing, &finishing("rings"), &[(-25.0, -4.0)]).unwrap();
    let (before, moves, head) = &entries(&path)[0];
    assert_ring_ramp(&f, *before, moves, *head, pass_sign(&path, *head));
}

#[test]
fn req_op_11_req_set_09_continuous_roughing_in_rings_ramps_every_layer_round_its_ring() {
    let f = cone();
    for keep in [&[][..], &[(-25.0, -4.0)][..]] {
        let path = make(&f, &RotaryRoughing, &roughing_values("rings"), keep).unwrap();
        let all = entries(&path);
        assert!(all.len() >= 3, "{} entries", all.len());
        for (before, moves, head) in all.iter().take(3) {
            assert_ring_ramp(&f, *before, moves, *head, pass_sign(&path, *head));
        }
    }
}

#[test]
fn req_op_11_ring_ramps_are_the_same_at_one_two_and_many_threads() {
    let f = cone();
    let v = roughing_values("rings");
    let at = |threads: usize| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap()
            .install(|| make(&f, &RotaryRoughing, &v, &[]).unwrap().group.segments)
    };
    let one = at(1);
    assert_eq!(one, at(2));
    assert_eq!(one, at(8));
}

/// Checks every ramp of `path` that turns A (a ring or a spiral entry);
/// counts them and the plunges.
fn turning_ramps(f: &Fixture, path: &GeneratedPath) -> (usize, usize) {
    let (mut ramps, mut plunges) = (0, 0);
    for (before, moves, head) in entries(path) {
        if moves.len() == 1 {
            plunges += 1;
            continue;
        }
        ramps += 1;
        assert_ring_ramp(f, before, &moves, head, pass_sign(path, head));
    }
    (ramps, plunges)
}

#[test]
fn req_op_11_req_set_09_a_spiral_is_ramped_round_the_axis_at_its_start_x() {
    // Decision 0543 D2 (a). Refused on main with the held region.
    let f = cylinder();
    for keep in [&[][..], &[(-25.0, -4.0)][..]] {
        let path = make(&f, &HelicalFinishing, &finishing("helix"), keep).unwrap();
        let (ramps, plunges) = turning_ramps(&f, &path);
        assert_eq!((ramps, plunges), (1, 0), "{keep:?}");
    }
}

#[test]
fn req_op_11_req_set_09_continuous_roughing_spiral_layers_ramp_and_a_single_point_plunges_when_allowed()
 {
    let f = cone();
    let mut v = roughing_values("helix");
    // Pass 4 is a single point: refused unless allowed (decision 0543 D7).
    let err = make(&f, &RotaryRoughing, &v, &[]).unwrap_err();
    assert!(err.contains("is a single point"), "{err}");
    assert!(err.contains("Allow single-point plunge"), "{err}");
    v.insert(
        names::ALLOW_SINGLE_POINT_PLUNGE.into(),
        ParameterValue::Flag(true),
    );
    for keep in [&[][..], &[(-25.0, -4.0)][..]] {
        let path = make(&f, &RotaryRoughing, &v, keep).unwrap();
        let (ramps, plunges) = turning_ramps(&f, &path);
        assert!(
            ramps >= 3 && plunges >= 1,
            "{ramps} ramps, {plunges} plunges"
        );
        assert!(
            path.notices
                .iter()
                .any(|n| n.text.contains("single points")),
            "{:?}",
            path.notices
        );
    }
}

/// One pass: a short step along X at A 0 (shorter than the tool radius),
/// then a whole ring, as roughing in Rings gives after a step (found on the
/// public baluster example).
#[derive(Debug)]
struct StepThenRing;

impl Strategy for StepThenRing {
    fn name(&self) -> &str {
        "test_step_then_ring"
    }
    fn label(&self) -> &str {
        "Step then ring (test)"
    }
    fn description(&self) -> &str {
        "A strategy for tests."
    }
    fn schema(&self) -> openchuck_path::ParameterSchema {
        openchuck_path::ParameterSchema::with_common(Vec::new())
    }
    fn passes(
        &self,
        input: &openchuck_path::StrategyInput<'_>,
        _: &openchuck_path::Parameters,
        _: &openchuck_types::job::JobContext<'_>,
    ) -> openchuck_path::Result<Vec<openchuck_path::Pass>> {
        use openchuck_types::units::{Degrees, Millimetres};
        let at = |x: f64, a: f64| {
            let line = input
                .rotary
                .tool_line(Millimetres(x), Degrees(a), Millimetres(0.0));
            input
                .surface
                .drop_tool(input.tool, &line)
                .map(|c| openchuck_path::CutPoint {
                    tip: c.tip,
                    a: Degrees(a),
                    contact: c.contact,
                })
        };
        let points = [(10.0, 0.0), (10.5, 0.0), (11.0, 0.0)]
            .into_iter()
            .chain((1..=72).map(|k| (11.0, 5.0 * f64::from(k))))
            .filter_map(|(x, a)| at(x, a))
            .collect();
        Ok(vec![openchuck_path::Pass {
            helical_pass_start: false,
            low_link: false,
            points,
            stock: None,
            rapids: Vec::new(),
        }])
    }
}

#[test]
fn req_op_11_req_op_15_a_step_shorter_than_the_tool_radius_is_ramped_on_along_the_pass() {
    // Decision 0543 D3: the back-and-forth continues over the pass's own
    // path past the short step, A reversing during the entry only, and a
    // note counts the reversals.
    let f = cylinder();
    let mut v = finishing("rings");
    v.remove("pass_shape");
    let path = make(&f, &StepThenRing, &v, &[]).unwrap();
    let all = entries(&path);
    let (before, moves, head) = &all[0];
    assert!(moves.len() > 2, "{}", moves.len());
    assert_eq!(moves.last(), Some(head));
    // Every move is over the pass's own path: X between 10 and 11 mm, A
    // within the ring's turn.
    for m in std::iter::once(before).chain(moves.iter()) {
        let p = f.rotary.to_axis(m.tip, m.a);
        assert!((10.0 - 1e-6..=11.0 + 1e-6).contains(&p.along), "{p:?}");
        assert!((-1e-6..=360.0 + 1e-6).contains(&m.a.0), "{}", m.a.0);
    }
    assert!(
        path.notices.iter().any(|n| n.text.contains("reversing A")),
        "{:?}",
        path.notices
    );
}

#[test]
fn req_op_11_req_nfr_07_a_ring_too_close_to_the_axis_for_ten_turns_is_refused_by_name() {
    // A 1 mm cylinder at 0.5 degrees: a 2 mm descent round a ring of about
    // 1 mm radius needs far more than 10 turns (decision 0543 D4).
    let cyl = Cylinder {
        radius: 1.0,
        length: 20.0,
    };
    let f = Fixture::new(MeshSurface::new(&cyl.mesh(3, 72)).unwrap(), 4.0, &ball(1.0));
    let mut v = finishing("rings");
    v.insert(names::RAMP_ANGLE.into(), n(0.5));
    v.insert(names::STEPOVER.into(), n(0.5));
    let err = make(&f, &HelicalFinishing, &v, &[]).unwrap_err();
    assert!(
        err.contains("Ramp entry: pass 1")
            && err.contains("more than the 10 allowed")
            && err.contains("Raise Ramp angle"),
        "{err}"
    );
}

/// One pass: a whole ring with the tool tip on the rotary axis (an end
/// face), as helical finishing makes at a free end.
#[derive(Debug)]
struct RingOnAxis;

impl Strategy for RingOnAxis {
    fn name(&self) -> &str {
        "test_ring_on_axis"
    }
    fn label(&self) -> &str {
        "Ring on the axis (test)"
    }
    fn description(&self) -> &str {
        "A strategy for tests."
    }
    fn schema(&self) -> openchuck_path::ParameterSchema {
        openchuck_path::ParameterSchema::with_common(Vec::new())
    }
    fn passes(
        &self,
        input: &openchuck_path::StrategyInput<'_>,
        _: &openchuck_path::Parameters,
        _: &openchuck_types::job::JobContext<'_>,
    ) -> openchuck_path::Result<Vec<openchuck_path::Pass>> {
        use openchuck_path::rotary::AxisPosition;
        use openchuck_types::units::Degrees;
        // At X 45 mm, beyond the cylinder's end, on the axis.
        let points = (0..=36)
            .map(|k| {
                let a = Degrees(10.0 * f64::from(k));
                let tip = input.rotary.to_model(
                    AxisPosition {
                        along: 45.0,
                        across: 0.0,
                        height: 0.0,
                    },
                    a,
                );
                openchuck_path::CutPoint {
                    tip,
                    a,
                    contact: tip,
                }
            })
            .collect();
        Ok(vec![openchuck_path::Pass {
            helical_pass_start: false,
            low_link: false,
            points,
            stock: None,
            rapids: Vec::new(),
        }])
    }
}

#[test]
fn req_op_11_req_op_15_a_ring_on_the_axis_descends_there_while_a_turns() {
    // Decision 0543 section 4, point 2, the owner's option (b). The stock
    // (radius 16 mm) reaches X 46 mm here, so the tip has 16 + 1 mm to
    // descend: at 3 degrees at the 3 mm radius of the 6 mm ball that is
    // 17 / (tan 3 deg * 2 pi * 3) = 17.2 turns, more than 10 (refused); at
    // 10 degrees, 5.1 turns.
    let f = Fixture::new(
        MeshSurface::new(
            &Cylinder {
                radius: 15.0,
                length: 40.0,
            }
            .mesh(3, 72),
        )
        .unwrap(),
        16.0,
        &ball(6.0),
    );
    let mut stock_long = f.input();
    let long = openchuck_path::AnalyticStock::Cylinder {
        radius: 16.0,
        x_start: -1.0,
        x_end: 46.0,
    };
    stock_long.stock = &long;
    let mut v = finishing("rings");
    v.remove("pass_shape");
    let err = run(|ctx| generate(&RingOnAxis, &stock_long, &v, ctx))
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("starts on the rotary axis") && err.contains("more than the 10 allowed"),
        "{err}"
    );
    v.insert(names::RAMP_ANGLE.into(), n(10.0));
    for allow in [false, true] {
        v.insert(
            names::ALLOW_SINGLE_POINT_PLUNGE.into(),
            ParameterValue::Flag(allow),
        );
        let path = run(|ctx| generate(&RingOnAxis, &stock_long, &v, ctx)).unwrap();
        let segs = &path.group.segments;
        let first_cut = segs
            .iter()
            .position(|s| s.kind == MoveKind::Cutting)
            .unwrap();
        let descent: Vec<_> = segs[..first_cut]
            .iter()
            .filter(|s| s.kind == MoveKind::Plunge)
            .collect();
        assert!(descent.len() > 10, "{}", descent.len());
        // On the axis all the way down, A rising to 0, the height falling.
        let mut previous: Option<(f64, f64)> = None;
        for s in &descent {
            let p = f.rotary.to_axis(s.end.tip, s.end.a);
            assert!(p.across.hypot(p.height) < 17.0 + 1e-6);
            assert!((p.along - 45.0).abs() < 1e-6 && p.across.abs() < 1e-6);
            if let Some((a, h)) = previous {
                assert!(s.end.a.0 > a && p.height < h + 1e-9);
            }
            previous = Some((s.end.a.0, p.height));
        }
        let turned = descent.last().unwrap().end.a.0 - descent.first().unwrap().end.a.0;
        assert!((turned / 360.0 - 5.1).abs() < 0.3, "{turned}");
        assert!(
            path.notices
                .iter()
                .any(|n| n.text.contains("starts on the rotary axis")),
            "{:?}",
            path.notices
        );
    }
}

/// The steeper ramp near the axis (decision 0543 section 4, point 3, the
/// owner's decision (iii)): a ring that would need more than 10 turns at the
/// Ramp angle is ramped at the angle that fits 10 turns, never steeper than
/// the Maximum ramp angle; past it the pass is refused naming that setting.
#[test]
fn req_op_11_req_nfr_07_steeper_ramp_near_the_axis_fits_ten_turns_up_to_the_maximum() {
    let cyl = Cylinder {
        radius: 1.0,
        length: 20.0,
    };
    let f = Fixture::new(MeshSurface::new(&cyl.mesh(3, 72)).unwrap(), 4.0, &ball(1.0));
    let mut v = finishing("rings");
    v.insert(names::RAMP_ANGLE.into(), n(0.5));
    v.insert(names::STEPOVER.into(), n(0.5));
    v.insert(
        names::RAMP_STEEPER_NEAR_AXIS.into(),
        ParameterValue::Flag(true),
    );
    v.insert(names::RAMP_ANGLE_MAX.into(), n(15.0));
    let path = make(&f, &HelicalFinishing, &v, &[]).unwrap();
    let (before, moves, head) = &entries(&path)[0];
    // Within 10 turns, and steeper than 0.5 degrees on its real path.
    let turned = (head.a.0 - before.a.0).abs();
    assert!(turned <= 3600.0 + 1e-6, "{turned}");
    assert!(turned > 1000.0, "{turned}");
    let drop =
        f.rotary.to_axis(before.tip, before.a).height - f.rotary.to_axis(head.tip, head.a).height;
    assert!(drop > 0.0);
    assert!(moves.len() > 10);
    assert!(
        path.notices
            .iter()
            .any(|n| n.text.contains("steeper") && n.text.contains("degrees")),
        "{:?}",
        path.notices
    );
    // A maximum below what 10 turns need is refused naming it.
    v.insert(names::RAMP_ANGLE_MAX.into(), n(0.6));
    let err = make(&f, &HelicalFinishing, &v, &[]).unwrap_err();
    assert!(
        err.contains("Maximum ramp angle") && err.contains("more than the 10 allowed"),
        "{err}"
    );
    // Off, the Ramp angle is kept and the pass refused as before.
    v.insert(
        names::RAMP_STEEPER_NEAR_AXIS.into(),
        ParameterValue::Flag(false),
    );
    let err = make(&f, &HelicalFinishing, &v, &[]).unwrap_err();
    assert!(err.contains("Raise Ramp angle"), "{err}");
}
