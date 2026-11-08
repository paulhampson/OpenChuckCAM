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
