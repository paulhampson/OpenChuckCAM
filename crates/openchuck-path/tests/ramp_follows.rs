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
                .any(|n| n.text.contains("is a single point")
                    || n.text.contains("are single points")),
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
            plunge_fed: Vec::new(),
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
    // Said in the singular where there is one (no "1 times", no "passes 1").
    assert!(
        path.notices
            .iter()
            .all(|n| !n.text.contains(" 1 times") && !n.text.contains("passes 1 ")),
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
            plunge_fed: Vec::new(),
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

/// Roughing-like passes that carry their stock: whole rings at X `x`, one
/// pass each, at the tip radii `radii` (outer first).
#[derive(Debug)]
struct NarrowingRings {
    x: f64,
    radii: Vec<f64>,
}

impl Strategy for NarrowingRings {
    fn name(&self) -> &str {
        "test_narrowing_rings"
    }
    fn label(&self) -> &str {
        "Narrowing rings (test)"
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
        let stock: std::sync::Arc<dyn openchuck_path::StockModel> =
            std::sync::Arc::new(openchuck_path::AnalyticStock::Cylinder {
                radius: 16.0,
                x_start: -1.0,
                x_end: 47.0,
            });
        Ok(self
            .radii
            .iter()
            .map(|&r| {
                let points = (0..=36)
                    .map(|k| {
                        let a = Degrees(10.0 * f64::from(k));
                        let tip = input.rotary.to_model(
                            AxisPosition {
                                along: self.x,
                                across: 0.0,
                                height: r,
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
                openchuck_path::Pass {
                    helical_pass_start: false,
                    low_link: false,
                    points,
                    stock: Some(openchuck_path::PassStock(stock.clone())),
                    rapids: Vec::new(),
                    plunge_fed: Vec::new(),
                }
            })
            .collect())
    }
}

/// Plunge to remove face nubs (decision 0543 section 4, point 3, the
/// owner's decision (iii)): roughing rings round the axis no wider than the
/// Face nub diameter, where the part leaves nothing inside them, are
/// replaced by a plunge to the axis while the part turns; where the part is
/// there, the rings are kept.
#[test]
fn req_op_11_req_op_03_face_nubs_are_plunged_where_the_part_leaves_nothing() {
    let f = cylinder();
    let mut long = f.input();
    let stock = openchuck_path::AnalyticStock::Cylinder {
        radius: 16.0,
        x_start: -1.0,
        x_end: 47.0,
    };
    long.stock = &stock;
    let mut v = finishing("rings");
    v.remove("pass_shape");
    v.insert(names::RAMP_ANGLE.into(), n(10.0));
    v.insert(names::PLUNGE_FACE_NUBS.into(), ParameterValue::Flag(true));
    // Beyond the cylinder's end (X 40): rings at 2.5, 1.5 and 0.6 mm, all
    // within the 6 mm ball's 6 mm nub.
    let rings = NarrowingRings {
        x: 45.0,
        radii: vec![2.5, 1.5, 0.6],
    };
    let path = run(|ctx| generate(&rings, &long, &v, ctx)).unwrap();
    let segs = &path.group.segments;
    let plunges: Vec<_> = segs.iter().filter(|s| s.kind == MoveKind::Plunge).collect();
    assert_eq!(plunges.len(), 1, "one plunge clears the nub");
    let bottom = f.rotary.to_axis(plunges[0].end.tip, plunges[0].end.a);
    assert!(bottom.height.abs() < 1e-6 && (bottom.along - 45.0).abs() < 1e-6);
    // No ring is ramped or cut away from the axis.
    for s in segs
        .iter()
        .filter(|s| matches!(s.kind, MoveKind::Cutting | MoveKind::LeadIn))
    {
        let p = f.rotary.to_axis(s.end.tip, s.end.a);
        assert!(p.across.hypot(p.height) < 1e-6, "{p:?}");
    }
    // The part turns a whole turn with the tip on the axis.
    let turned: f64 = segs
        .iter()
        .filter(|s| s.kind == MoveKind::Cutting)
        .zip(std::iter::once(plunges[0]).chain(segs.iter().filter(|s| s.kind == MoveKind::Cutting)))
        .map(|(s, p)| (s.end.a.0 - p.end.a.0).abs())
        .sum();
    assert!((turned - 360.0).abs() < 1e-6, "{turned}");
    assert!(
        path.notices
            .iter()
            .any(|n| n.text.contains("nub") && n.text.contains("1 plunge")),
        "{:?}",
        path.notices
    );
    // On the part (X 20, a cylinder of 15 mm), rings no wider than the nub
    // would cut the part: they are not nub passes, and are ramped as rings.
    let thin = Fixture::new(
        MeshSurface::new(
            &Cylinder {
                radius: 1.0,
                length: 40.0,
            }
            .mesh(3, 72),
        )
        .unwrap(),
        16.0,
        &ball(6.0),
    );
    let mut on_part = thin.input();
    on_part.stock = &stock;
    let rings = NarrowingRings {
        x: 20.0,
        radii: vec![2.5],
    };
    let path = run(|ctx| generate(&rings, &on_part, &v, ctx)).unwrap();
    assert!(
        path.group
            .segments
            .iter()
            .all(|s| s.kind != MoveKind::Plunge),
        "the part is inside the nub: no plunge"
    );
}

/// One line pass whose first point lies 1 mm below the rest, so the ramp's
/// last leg follows the pass's own drop to it (decision 0543 D4, answer A).
#[derive(Debug)]
struct DropBeforeHead;

impl Strategy for DropBeforeHead {
    fn name(&self) -> &str {
        "test_drop_before_head"
    }
    fn label(&self) -> &str {
        "Drop before head (test)"
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
        let points = (0..=40)
            .map(|k| {
                let along = 10.0 + 0.5 * f64::from(k);
                let height = if k == 0 { 15.0 } else { 16.0 };
                let tip = input.rotary.to_model(
                    AxisPosition {
                        along,
                        across: 0.0,
                        height,
                    },
                    Degrees(0.0),
                );
                openchuck_path::CutPoint {
                    tip,
                    a: Degrees(0.0),
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
            plunge_fed: Vec::new(),
        }])
    }
}

#[test]
fn req_op_11_req_nfr_07_the_steep_drop_note_is_singular_for_one_pass() {
    let f = cylinder();
    let mut v = finishing("rings");
    v.remove("pass_shape");
    let path = make(&f, &DropBeforeHead, &v, &[]).unwrap();
    let note = path
        .notices
        .iter()
        .find(|n| n.text.contains("more steeply than the ramp angle"))
        .unwrap_or_else(|| panic!("{:?}", path.notices));
    assert!(
        note.text.starts_with("pass 1 drops more steeply")
            && note.text.contains("its ramp follows")
            && note.text.contains("as the pass itself does"),
        "{}",
        note.text
    );
}

/// One roughing pass that joins a ring over the part (X 20, outside the nub)
/// to two small rings past the part's end (X 45), as roughing in Rings joins
/// a layer into one pass.
#[derive(Debug)]
struct LayerPastTheEnd;

impl Strategy for LayerPastTheEnd {
    fn name(&self) -> &str {
        "test_layer_past_the_end"
    }
    fn label(&self) -> &str {
        "Layer past the end (test)"
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
        let at = |along: f64, height: f64, a: f64| {
            let tip = input.rotary.to_model(
                AxisPosition {
                    along,
                    across: 0.0,
                    height,
                },
                Degrees(a),
            );
            openchuck_path::CutPoint {
                tip,
                a: Degrees(a),
                contact: tip,
            }
        };
        let mut points: Vec<openchuck_path::CutPoint> = (0..=36)
            .map(|k| at(20.0, 15.5, 10.0 * f64::from(k)))
            .collect();
        // The step along X at the ring's end angle, then the small rings.
        points.extend((1..=10).map(|k| at(20.0 + 2.5 * f64::from(k), 15.5, 360.0)));
        points.push(at(45.0, 1.0, 360.0));
        points.extend((1..=36).map(|k| at(45.0, 1.0, 360.0 + 10.0 * f64::from(k))));
        let stock: std::sync::Arc<dyn openchuck_path::StockModel> =
            std::sync::Arc::new(openchuck_path::AnalyticStock::Cylinder {
                radius: 16.0,
                x_start: -1.0,
                x_end: 47.0,
            });
        Ok(vec![openchuck_path::Pass {
            helical_pass_start: false,
            low_link: false,
            points,
            stock: Some(openchuck_path::PassStock(stock)),
            rapids: Vec::new(),
            plunge_fed: Vec::new(),
        }])
    }
}

#[test]
fn req_op_11_req_op_03_a_nub_inside_a_joined_roughing_layer_is_plunged() {
    // The layer's small ring past the part's end is a nub (the part leaves
    // nothing round the axis there) and is plunged; the ring over the part
    // is still cut. Off, the program has no plunge.
    let f = cylinder();
    let mut input = f.input();
    let stock = openchuck_path::AnalyticStock::Cylinder {
        radius: 16.0,
        x_start: -1.0,
        x_end: 47.0,
    };
    input.stock = &stock;
    let mut v = finishing("rings");
    v.remove("pass_shape");
    v.insert(names::RAMP_ANGLE.into(), n(10.0));
    let plunges = |path: &GeneratedPath| {
        path.group
            .segments
            .iter()
            .filter(|s| s.kind == MoveKind::Plunge)
            .count()
    };
    let off = run(|ctx| generate(&LayerPastTheEnd, &input, &v, ctx)).unwrap();
    assert_eq!(plunges(&off), 0);
    v.insert(names::PLUNGE_FACE_NUBS.into(), ParameterValue::Flag(true));
    let on = run(|ctx| generate(&LayerPastTheEnd, &input, &v, ctx)).unwrap();
    assert_eq!(plunges(&on), 1);
    let segs = &on.group.segments;
    // The ring over the part is still cut at 15.5 mm; nothing is cut at
    // 1 mm from the axis.
    let cut_at = |r: f64| {
        segs.iter().any(|s| {
            let p = f.rotary.to_axis(s.end.tip, s.end.a);
            s.kind == MoveKind::Cutting && (p.across.hypot(p.height) - r).abs() < 1e-6
        })
    };
    assert!(cut_at(15.5));
    assert!(!cut_at(1.0));
    assert!(
        on.notices.iter().any(|n| n.text.contains("nub")),
        "{:?}",
        on.notices
    );
}

/// Rotary roughing in Rings on a cylinder of 5 mm radius, the stock (radius
/// 8 mm) running 10 mm past its flat free end: past the end the layers ring
/// down towards the axis, and with Plunge to remove face nubs the rings no
/// wider than the tool are plunged instead (decision 0543 section 4, point 3).
#[test]
fn req_op_11_req_op_03_roughing_past_a_flat_free_end_plunges_its_nub() {
    let part = Cylinder {
        radius: 5.0,
        length: 30.0,
    };
    let f = Fixture::new(
        MeshSurface::new(&part.mesh(3, 72)).unwrap(),
        8.0,
        &tool(ToolShape::FlatEnd, 3.0),
    );
    let stock = openchuck_path::AnalyticStock::Cylinder {
        radius: 8.0,
        x_start: -1.0,
        x_end: 40.0,
    };
    let mut input = f.input();
    input.stock = &stock;
    let mut v = roughing_values("rings");
    v.insert(names::STEPOVER.into(), n(1.5));
    v.insert(roughing::STEPDOWN.into(), n(1.0));
    v.insert(names::RAMP_ANGLE.into(), n(10.0));
    v.insert(
        names::RAMP_STEEPER_NEAR_AXIS.into(),
        ParameterValue::Flag(true),
    );
    v.insert(
        names::ALLOW_SINGLE_POINT_PLUNGE.into(),
        ParameterValue::Flag(true),
    );
    v.insert("extend".into(), t("far end"));
    let off = run(|ctx| generate(&RotaryRoughing, &input, &v, ctx)).unwrap();
    v.insert(names::PLUNGE_FACE_NUBS.into(), ParameterValue::Flag(true));
    let on = run(|ctx| generate(&RotaryRoughing, &input, &v, ctx)).unwrap();
    let near_axis = |path: &GeneratedPath| {
        path.group
            .segments
            .iter()
            .filter(|s| s.kind == MoveKind::Cutting)
            .filter(|s| {
                let p = f.rotary.to_axis(s.end.tip, s.end.a);
                p.across.hypot(p.height) <= 1.5 + 1e-6 && p.across.hypot(p.height) > 1e-6
            })
            .count()
    };
    assert!(
        near_axis(&off) > 0,
        "roughing rings near the axis past the end"
    );
    assert_ne!(off.group, on.group);
    assert!(
        on.notices.iter().any(|n| n.text.contains("nub")),
        "{:?}",
        on.notices
    );
    assert_eq!(near_axis(&on), 0, "no ring left within the nub");
}

/// One pass through the given (X, A) points, the tool dropped onto the part
/// at each, A 0 to start.
#[derive(Debug)]
struct Through(Vec<(f64, f64)>);

impl Strategy for Through {
    fn name(&self) -> &str {
        "test_through"
    }
    fn label(&self) -> &str {
        "Through points (test)"
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
        let points = self
            .0
            .iter()
            .filter_map(|&(x, a)| {
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
            })
            .collect();
        Ok(vec![openchuck_path::Pass {
            helical_pass_start: false,
            low_link: false,
            points,
            stock: None,
            rapids: Vec::new(),
            plunge_fed: Vec::new(),
        }])
    }
}

#[test]
fn req_op_11_req_op_15_a_partial_ring_is_ramped_back_and_forth_over_its_own_arc() {
    // Decision 0543 D3: an arc shorter than a turn has no ring before its
    // first point to ramp round, so the ramp goes back and forth over the
    // arc itself, A reversing during the entry only, with a note counting
    // the reversals; D4: within the angle on the tip's real path.
    let f = cylinder();
    let mut v = finishing("rings");
    v.remove("pass_shape");
    let arc = Through((0..=18).map(|k| (20.0, 5.0 * f64::from(k))).collect());
    let path = make(&f, &arc, &v, &[]).unwrap();
    let all = entries(&path);
    let (before, moves, head) = &all[0];
    assert!(moves.len() > 2, "{}", moves.len());
    assert_eq!(moves.last(), Some(head));
    let tan = ANGLE.to_radians().tan();
    let mut p = *before;
    let mut turns = Vec::new();
    for m in moves {
        let at = f.rotary.to_axis(m.tip, m.a);
        assert!((at.along - 20.0).abs() < 1e-6, "{at:?}");
        assert!((-1e-6..=90.0 + 1e-6).contains(&m.a.0), "{}", m.a.0);
        let from = f.rotary.to_axis(p.tip, p.a);
        let radius = 0.5 * (from.across.hypot(from.height) + at.across.hypot(at.height));
        let travel = radius * (m.a.0 - p.a.0).abs().to_radians();
        let drop = from.across.hypot(from.height) - at.across.hypot(at.height);
        assert!(drop <= travel * tan + 1e-6, "{drop} over {travel}");
        turns.push((m.a.0 - p.a.0).signum());
        p = *m;
    }
    assert!(
        turns.windows(2).any(|w| w[0] != w[1]),
        "A reverses during the entry: {turns:?}"
    );
    assert!(
        path.notices.iter().any(|n| n.text.contains("reversing A")),
        "{:?}",
        path.notices
    );
}

#[test]
fn req_op_11_req_nfr_07_a_ring_ramp_outside_a_limited_a_range_is_refused_by_name() {
    // Decision 0543 D4: the ramp round a ring turns A before the pass's
    // first point, and that extra A is counted against a limited A range.
    let mut f = cylinder();
    f.rotary.range = openchuck_types::machine::ARange::Limited {
        min: openchuck_types::units::Degrees(0.0),
        max: openchuck_types::units::Degrees(360.0),
    };
    let mut v = finishing("rings");
    v.remove("pass_shape");
    let ring = Through((0..=72).map(|k| (20.0, 5.0 * f64::from(k))).collect());
    let err = make(&f, &ring, &v, &[]).unwrap_err();
    assert!(
        err.contains("Ramp entry: pass 1 needs A from")
            && err.contains("outside the machine's A range of 0.000 to 360.000 degrees")
            && err.contains("Widen A range"),
        "{err}"
    );
}

#[test]
fn req_op_11_req_nfr_07_a_pass_shorter_than_ten_tolerances_is_refused_by_name() {
    // Decision 0543 D3, as amended for issue #687 (accepted): the
    // back-and-forth needs at least ten path tolerances of the pass's own
    // path to go over, never more than the tool radius. A shorter line is
    // continued over the rest of the pass (point 1); where the whole pass is
    // shorter, the ramp is refused, naming the stretch, the minimum and the
    // other entries.
    let f = cylinder();
    let mut v = finishing("rings");
    v.remove("pass_shape");
    // Shorter than the tool radius (3 mm) but longer than ten tolerances
    // (0.1 mm at the default): ramped, and listed for the host's check of a
    // tool that cannot cut straight down.
    let path = make(&f, &Through(vec![(20.0, 0.0), (20.5, 0.0)]), &v, &[]).unwrap();
    assert!(
        path.group
            .segments
            .iter()
            .any(|s| s.kind == MoveKind::LeadIn)
    );
    assert_eq!(path.straight_plunges, vec![0]);
    // Shorter than ten tolerances: refused.
    let err = make(&f, &Through(vec![(20.0, 0.0), (20.05, 0.0)]), &v, &[]).unwrap_err();
    assert!(
        err.contains("Ramp entry: pass 1 starts with a stretch of 0.050 mm")
            && err.contains("shorter than the 0.100 mm (ten times the tolerance)")
            && err.contains("Set Entry to Plunge or Helix"),
        "{err}"
    );
    // Ten tolerances longer than the tool radius: the radius is the minimum.
    v.insert(names::TOLERANCE.into(), n(0.4));
    let err = make(&f, &Through(vec![(20.0, 0.0), (20.5, 0.0)]), &v, &[]).unwrap_err();
    assert!(
        err.contains("Ramp entry: pass 1 starts with a stretch of 0.500 mm")
            && err.contains("shorter than the 3.000 mm (the tool radius)"),
        "{err}"
    );
}

#[test]
fn req_op_11_req_nfr_07_the_path_lists_the_passes_entered_by_a_straight_plunge_into_the_stock() {
    // Decision 0543 D7 and section 4, points 2 and 3; decision 0544 item 7:
    // a host refuses these for a tool that cannot cut straight down, so the
    // path lists exactly them, and not a ramp that follows the pass.
    let f = cylinder();
    let mut v = finishing("rings");
    v.remove("pass_shape");
    // A ring ramped round itself: nothing plunges.
    let ring = Through((0..=72).map(|k| (20.0, 5.0 * f64::from(k))).collect());
    assert!(
        make(&f, &ring, &v, &[])
            .unwrap()
            .straight_plunges
            .is_empty()
    );
    // A line ramped back and forth: nothing plunges.
    let line = Through((0..=20).map(|k| (10.0 + f64::from(k), 0.0)).collect());
    assert!(
        make(&f, &line, &v, &[])
            .unwrap()
            .straight_plunges
            .is_empty()
    );
    // A single point, plunged with Allow single-point plunge on.
    v.insert(
        names::ALLOW_SINGLE_POINT_PLUNGE.into(),
        ParameterValue::Flag(true),
    );
    let point = Through(vec![(20.0, 0.0)]);
    assert_eq!(make(&f, &point, &v, &[]).unwrap().straight_plunges, [0]);
    // A ring on the rotary axis, descended while A turns.
    let mut long = f.input();
    let stock = openchuck_path::AnalyticStock::Cylinder {
        radius: 16.0,
        x_start: -1.0,
        x_end: 46.0,
    };
    long.stock = &stock;
    v.insert(names::RAMP_ANGLE.into(), n(10.0));
    let path = run(|ctx| generate(&RingOnAxis, &long, &v, ctx)).unwrap();
    assert_eq!(path.straight_plunges, [0]);
    // Face nubs plunged beyond the part's end.
    let mut nub = f.input();
    let stock = openchuck_path::AnalyticStock::Cylinder {
        radius: 16.0,
        x_start: -1.0,
        x_end: 47.0,
    };
    nub.stock = &stock;
    v.insert(names::PLUNGE_FACE_NUBS.into(), ParameterValue::Flag(true));
    let rings = NarrowingRings {
        x: 45.0,
        radii: vec![2.5, 1.5, 0.6],
    };
    let path = run(|ctx| generate(&rings, &nub, &v, ctx)).unwrap();
    assert!(!path.straight_plunges.is_empty());
}

/// Continuous rotary roughing of the public blisk (`testdata/stl/blisk.stl`)
/// in rings with the 3 mm flat end mill of the blisk example: stock to leave
/// 0, stepover and stepdown 1 mm, Entry Ramp at 3 degrees, with Allow
/// single-point plunge (as in the issue's comment) and Steeper ramp near
/// the axis on, up to 20 degrees (without it the rings 4 mm from the axis
/// are refused by name, needing 12.9 turns; at 15 degrees, the ring 1 mm
/// from it, needing 11.9).
#[test]
fn req_op_11_req_op_03_roughing_the_blisk_in_rings_ramps_beside_its_end_face_and_over_short_steps()
{
    let f = Fixture::new(
        MeshSurface::new(&support::blisk()).unwrap(),
        21.0,
        &tool(ToolShape::FlatEnd, 3.0),
    );
    let mut v = roughing_values("rings");
    v.insert(roughing::FORM.into(), t("continuous"));
    v.insert(names::STEPOVER.into(), n(1.0));
    v.insert(roughing::STEPDOWN.into(), n(1.0));
    v.insert(names::STOCK_TO_LEAVE.into(), n(0.0));
    v.insert(names::RAMP_ANGLE.into(), n(3.0));
    v.insert(names::RAMP_ANGLE_MAX.into(), n(20.0));
    for flag in [
        names::ALLOW_SINGLE_POINT_PLUNGE,
        names::RAMP_STEEPER_NEAR_AXIS,
    ] {
        v.insert(flag.into(), ParameterValue::Flag(true));
    }
    // Issue #687: a ring beside the end face of the blisk's disc runs with
    // the tool edge-on to the face. Dropped along the tool axis, a tool
    // touching a face sideways scores the height of the rim above it, so
    // the pass's own points failed the check its ramp was held to (pass 3
    // was refused, "ramps over its own path, yet the tool would cut … into
    // the finished surface"). A ramp over the pass's own path is no deeper
    // than the pass beneath it (decision 0543 section 3), and is checked
    // against that floor.
    // Decision 0543 D3 as amended (accepted): later passes start with steps
    // along the length between the blades shorter than the 1.5 mm tool
    // radius (pass 467 was refused, "shorter than the 1.500 mm (the tool
    // radius)"). The minimum is ten path tolerances: those passes ramp, and
    // are listed for the host's check of a tool that cannot cut straight
    // down.
    let path = make(&f, &RotaryRoughing, &v, &[]).unwrap();
    assert!(
        path.group
            .segments
            .iter()
            .any(|s| s.kind == MoveKind::LeadIn)
    );
    assert!(
        path.straight_plunges.len() > 1,
        "{:?}",
        path.straight_plunges
    );
}

/// Decision record 0644: a ramp entry refusal names its check and the
/// setting its first remedy changes, so the application shows it in the one
/// form and links to the setting.
#[test]
fn req_ui_12_ramp_entry_refusals_carry_their_topic_and_setting() {
    let f = cylinder();
    let mut v = finishing("rings");
    v.remove("pass_shape");
    // Shorter than ten path tolerances (decision 0543 point 4).
    let short = Through(vec![(20.0, 0.0), (20.05, 0.0)]);
    let mut input = f.input();
    input.keep_out = &[];
    match run(|ctx| generate(&short, &input, &v, ctx)).unwrap_err() {
        openchuck_path::PathError::Refused {
            topic,
            text,
            setting,
        } => {
            assert_eq!(topic, openchuck_path::topic::RAMP_ENTRY);
            assert!(text.starts_with("pass 1 starts with a stretch"), "{text}");
            assert_eq!(setting.as_deref(), Some(names::ENTRY));
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
}

/// One pass that turns A while it moves along X, as swarf along a blade
/// does: a spiral to the pass-shape reader. `round` is the strategy's
/// [`Strategy::ramps_round_the_part`].
#[derive(Debug)]
struct AlongTheFace {
    round: bool,
}

impl Strategy for AlongTheFace {
    fn name(&self) -> &str {
        "test_along_the_face"
    }
    fn label(&self) -> &str {
        "Along the face (test)"
    }
    fn description(&self) -> &str {
        "A strategy for tests."
    }
    fn schema(&self) -> openchuck_path::ParameterSchema {
        openchuck_path::ParameterSchema::with_common(Vec::new())
    }
    fn ramps_round_the_part(&self) -> bool {
        self.round
    }
    fn passes(
        &self,
        input: &openchuck_path::StrategyInput<'_>,
        parameters: &openchuck_path::Parameters,
        context: &openchuck_types::job::JobContext<'_>,
    ) -> openchuck_path::Result<Vec<openchuck_path::Pass>> {
        Through(
            (0..=40)
                .map(|k| (10.0 + 0.25 * f64::from(k), f64::from(k)))
                .collect(),
        )
        .passes(input, parameters, context)
    }
}

#[test]
fn req_op_11_req_op_07_a_swarf_like_pass_ramps_along_its_own_path_never_round_the_part() {
    // Issue #778 (decision 0645): a pass that turns A while it moves along
    // X reads as a spiral, and its ramp turned A round the part at the
    // pass's start X before the first point; on the blisk that swept the
    // tool through the other blades. A strategy whose passes do not cut all
    // round the part (swarf) has its ramp go back and forth over the pass's
    // own path instead: every entry pose lies between the pass's own X and
    // A, A reverses during the entry only, and the descent is within the
    // ramp angle.
    let f = cylinder();
    let mut v = finishing("rings");
    v.remove("pass_shape");
    let round = make(&f, &AlongTheFace { round: true }, &v, &[]).unwrap();
    let (_, moves, _) = entries(&round).into_iter().next().unwrap();
    assert!(
        moves.iter().any(|m| m.a.0 < -1.0),
        "a strategy that cuts all round may ramp round the part"
    );
    let path = make(&f, &AlongTheFace { round: false }, &v, &[]).unwrap();
    let all = entries(&path);
    let (before, moves, head) = &all[0];
    assert!(moves.len() > 2, "a ramp, not a plunge: {}", moves.len());
    assert_eq!(moves.last(), Some(head));
    for m in moves {
        let at = f.rotary.to_axis(m.tip, m.a);
        assert!((-1e-6..=40.0 + 1e-6).contains(&m.a.0), "A {}", m.a.0);
        assert!(
            (10.0 - 1e-6..=20.0 + 1e-6).contains(&at.along),
            "X {}",
            at.along
        );
        // On the pass's own tool line: X and A move together as the pass's.
        assert!(
            (at.along - (10.0 + 0.25 * m.a.0)).abs() < 1e-6,
            "{at:?} at A {}",
            m.a.0
        );
    }
    let tan = ANGLE.to_radians().tan();
    let mut p = *before;
    for m in moves {
        let (from, at) = (f.rotary.to_axis(p.tip, p.a), f.rotary.to_axis(m.tip, m.a));
        let radius = 0.5 * (from.across.hypot(from.height) + at.across.hypot(at.height));
        let travel = (at.along - from.along).hypot(at.across - from.across)
            + radius * (m.a.0 - p.a.0).abs().to_radians();
        let drop = from.across.hypot(from.height) - at.across.hypot(at.height);
        assert!(drop <= travel * tan + 1e-6, "{drop} over {travel}");
        p = *m;
    }
    // The note states the reversals; it gives no "too short a stretch"
    // reason, which a pass ramped along its face does not have (#781 review).
    assert!(
        path.notices
            .iter()
            .filter(|n| n.text.contains("reversing A"))
            .all(|n| !n.text.contains("too short")),
        "{:?}",
        path.notices
    );
    assert!(
        path.notices.iter().any(|n| n.text.contains("reversing A")),
        "{:?}",
        path.notices
    );
}

/// [`RingOnAxis`] from a strategy that does not ramp round the part.
#[derive(Debug)]
struct RingOnAxisNotRound;

impl Strategy for RingOnAxisNotRound {
    fn name(&self) -> &str {
        "test_ring_on_axis_not_round"
    }
    fn label(&self) -> &str {
        "Ring on the axis, not round (test)"
    }
    fn description(&self) -> &str {
        "A strategy for tests."
    }
    fn schema(&self) -> openchuck_path::ParameterSchema {
        openchuck_path::ParameterSchema::with_common(Vec::new())
    }
    fn ramps_round_the_part(&self) -> bool {
        false
    }
    fn passes(
        &self,
        input: &openchuck_path::StrategyInput<'_>,
        parameters: &openchuck_path::Parameters,
        context: &openchuck_types::job::JobContext<'_>,
    ) -> openchuck_path::Result<Vec<openchuck_path::Pass>> {
        RingOnAxis.passes(input, parameters, context)
    }
}

#[test]
fn req_op_11_req_nfr_07_a_pass_on_the_axis_not_ramped_round_the_part_is_listed_as_a_plunge() {
    // #781 review (decision 0645; 0543 D7): a strategy that does not ramp
    // round the part has no descent on the axis planned, so a pass that
    // starts on the rotary axis inside the stock is plunged, and listed for
    // the host's check of a tool that cannot cut straight down.
    let f = cylinder();
    let mut long = f.input();
    let stock = openchuck_path::AnalyticStock::Cylinder {
        radius: 16.0,
        x_start: -1.0,
        x_end: 46.0,
    };
    long.stock = &stock;
    let mut v = finishing("rings");
    v.remove("pass_shape");
    let path = run(|ctx| generate(&RingOnAxisNotRound, &long, &v, ctx)).unwrap();
    assert_eq!(path.straight_plunges, [0]);
    // Beyond the stock there is nothing to plunge into: not listed.
    let mut short = f.input();
    let stock = openchuck_path::AnalyticStock::Cylinder {
        radius: 16.0,
        x_start: -1.0,
        x_end: 41.0,
    };
    short.stock = &stock;
    let path = run(|ctx| generate(&RingOnAxisNotRound, &short, &v, ctx)).unwrap();
    assert!(
        path.straight_plunges.is_empty(),
        "{:?}",
        path.straight_plunges
    );
}
