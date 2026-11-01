// SPDX-License-Identifier: AGPL-3.0-or-later
//! Rings in place of the simultaneous spiral (issue #440; decision records
//! 0390, 0420 and 0421; OP-05, OP-06, OP-11, OP-12, OP-14, OP-15, OP-26,
//! GC-07, NFR-06, NFR-07).
//!
//! With **Passes** set to **Rings**, the spiral pattern cuts one whole turn
//! at each fixed X, driven as the spiral is (the smoothed A, the lead and
//! tilt, round the section's own centre where the section does not surround
//! the axis), and steps along the part between them. With the setting absent
//! or **Spiral**, the path is exactly as before.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

mod support;
use support::generate;

use std::collections::BTreeMap;

use openchuck_path::helical::parameter::{HELIX, RINGS};
use openchuck_path::params::names;
use openchuck_path::rotary::reversals;
use openchuck_path::simultaneous::parameter;
use openchuck_path::{
    GeneratedPath, MeshSurface, ParameterKind, ParameterValue, SimultaneousFinishing, Strategy,
    SurfaceSource,
};
use openchuck_testkit::gouge::{Outline, Pose, coverage, max_gouge};
use openchuck_testkit::parts::{Cone, Cylinder, LobedVase, RadialPart};
use openchuck_types::ir::{MoveKind, Segment, ToolPose};
use openchuck_types::units::Degrees;
use support::{Fixture, ball, run};

fn num(x: f64) -> ParameterValue {
    ParameterValue::Number(x)
}

fn text(s: &str) -> ParameterValue {
    ParameterValue::Text(s.to_owned())
}

fn values(pitch: f64, extra: &[(&str, ParameterValue)]) -> BTreeMap<String, ParameterValue> {
    let mut v: BTreeMap<String, ParameterValue> = [
        (names::STEPOVER, pitch),
        (names::FEED, 300.0),
        (names::PLUNGE_FEED, 100.0),
    ]
    .into_iter()
    .map(|(k, x)| (k.to_owned(), num(x)))
    .collect();
    for (k, x) in extra {
        v.insert((*k).to_owned(), x.clone());
    }
    v
}

fn ringed(mut v: BTreeMap<String, ParameterValue>) -> BTreeMap<String, ParameterValue> {
    v.insert(parameter::PASS_SHAPE.to_owned(), text(RINGS));
    v
}

fn fixture<P: RadialPart + ?Sized>(part: &P, rings: u32, around: u32) -> Fixture {
    let surface = MeshSurface::new(&part.mesh(rings, around)).unwrap();
    let b = surface.bounds();
    let r = [b.max.y, b.max.z, b.min.y, b.min.z]
        .into_iter()
        .fold(0.0_f64, |m, v| m.max(v.abs()));
    Fixture::new(surface, r + 1.0, &ball(6.0))
}

fn path(
    f: &Fixture,
    v: &BTreeMap<String, ParameterValue>,
) -> openchuck_path::Result<GeneratedPath> {
    run(|ctx| generate(&SimultaneousFinishing, &f.input(), v, ctx))
}

/// The cutting segments with the pose each starts from.
fn cuts(p: &GeneratedPath) -> Vec<(ToolPose, Segment)> {
    let mut prev = p.group.start;
    let mut out = Vec::new();
    for s in &p.group.segments {
        if s.kind == MoveKind::Cutting {
            out.push((prev, *s));
        }
        prev = s.end;
    }
    out
}

/// The X of each run of cutting moves at constant X, with the A it turned.
fn ring_xs(p: &GeneratedPath) -> Vec<(f64, f64)> {
    let mut out: Vec<(f64, f64)> = Vec::new();
    for (from, s) in cuts(p) {
        if (s.end.tip.x - from.tip.x).abs() > 1e-9 {
            continue;
        }
        let da = (s.end.a.0 - from.a.0).abs();
        match out.last_mut() {
            Some((x, turned)) if (*x - s.end.tip.x).abs() < 1e-9 => *turned += da,
            _ => out.push((s.end.tip.x, da)),
        }
    }
    out.retain(|(_, turned)| *turned > 1e-9);
    out
}

fn fed(p: &GeneratedPath) -> Vec<Pose> {
    p.group
        .segments
        .iter()
        .filter(|s| matches!(s.kind, MoveKind::Cutting | MoveKind::Plunge))
        .map(|s| Pose {
            tip: s.end.tip,
            axis: s.end.axis.get(),
        })
        .collect()
}

/// The A of each run of cutting moves.
fn cut_runs(p: &GeneratedPath) -> Vec<Vec<Degrees>> {
    let mut runs: Vec<Vec<Degrees>> = Vec::new();
    let mut prev = p.group.start;
    let mut cutting = false;
    for s in &p.group.segments {
        let now = s.kind == MoveKind::Cutting;
        if now && !cutting {
            runs.push(vec![prev.a]);
        }
        if now && let Some(r) = runs.last_mut() {
            r.push(s.end.a);
        }
        cutting = now;
        prev = s.end;
    }
    runs
}

fn lobed() -> LobedVase {
    LobedVase {
        base: 14.0,
        wave: 3.0,
        wave_length: 40.0,
        lobe: 1.5,
        lobes: 3,
        length: 20.0,
    }
}

#[test]
fn req_op_05_the_spiral_pattern_offers_rings_defaulting_to_spiral() {
    let schema = SimultaneousFinishing.schema();
    let spec = schema.get(parameter::PASS_SHAPE).unwrap();
    assert_eq!(spec.label, "Passes");
    assert!(spec.description.contains("Spiral") && spec.description.contains("Rings"));
    match &spec.kind {
        ParameterKind::Choice { options, default } => {
            assert_eq!(options, &[HELIX.to_owned(), RINGS.to_owned()]);
            assert_eq!(default, HELIX);
        }
        other => panic!("{other:?}"),
    }
    let shown = |pattern: Option<&str>| {
        let v: BTreeMap<String, ParameterValue> = pattern
            .map(|p| (parameter::PATTERN.to_owned(), text(p)))
            .into_iter()
            .collect();
        SimultaneousFinishing
            .schema_for(&v)
            .get(parameter::PASS_SHAPE)
            .is_some()
    };
    assert!(shown(None));
    assert!(shown(Some("spiral")));
    assert!(!shown(Some("axial")));
}

#[test]
fn req_op_05_rings_turn_whole_turns_at_constant_x_no_wider_apart_than_the_stepover() {
    let part = Cylinder {
        radius: 15.0,
        length: 20.0,
    };
    let f = fixture(&part, 3, 720);
    for (pitch, rotation, n) in [(1.0, "positive", 20_usize), (1.5, "negative", 14)] {
        let p = path(
            &f,
            &ringed(values(pitch, &[(parameter::ROTATION, text(rotation))])),
        )
        .unwrap();
        let found = ring_xs(&p);
        assert_eq!(found.len(), n + 1, "pitch {pitch}: {found:?}");
        let spacing = 20.0 / n as f64;
        assert!(spacing <= pitch + 1e-12);
        for (k, (x, turned)) in found.iter().enumerate() {
            assert!((x - spacing * k as f64).abs() < 1e-6, "ring {k} at {x}");
            // A whole turn, to within the mesh: each end of a ring is at the
            // smoothed normal of the facets there (720 round the part), up
            // to a quarter degree from the nominal angle.
            assert!(
                (turned - 360.0).abs() <= 0.5 + 1e-9,
                "ring {k} turned {turned}"
            );
        }
        // A turns the Rotation way on every ring and never reverses in a
        // cut (OP-15).
        for run in cut_runs(&p) {
            assert_eq!(reversals(&run, 1e-9), 0, "pitch {pitch}");
        }
        assert!(
            p.notes.iter().any(|n| n.message.contains("rings")),
            "{:?}",
            p.notes
        );
    }
}

#[test]
fn req_op_05_req_gc_07_the_spiral_is_unchanged_when_rings_are_not_chosen() {
    let f = fixture(&lobed(), 41, 720);
    let missing = path(&f, &values(1.0, &[])).unwrap();
    let spiral = path(&f, &values(1.0, &[(parameter::PASS_SHAPE, text(HELIX))])).unwrap();
    assert_eq!(missing, spiral);
    assert!(ring_xs(&spiral).is_empty());
    // The axial pattern ignores the setting.
    let axial = values(1.0, &[(parameter::PATTERN, text("axial"))]);
    assert_eq!(
        path(&f, &axial).unwrap(),
        path(&f, &ringed(axial.clone())).unwrap()
    );
}

#[test]
fn req_op_05_req_op_12_rings_leave_no_more_than_the_spiral_and_never_gouge() {
    let parts: Vec<(Box<dyn RadialPart>, u32, u32)> = vec![
        (
            Box::new(Cone {
                start_radius: 18.0,
                end_radius: 8.0,
                length: 20.0,
            }),
            21,
            720,
        ),
        (Box::new(lobed()), 41, 720),
    ];
    for (part, rings_n, around) in &parts {
        let f = fixture(part.as_ref(), *rings_n, *around);
        // Poses no more than two degrees apart round the rings, so the
        // checker, which looks only at the poses, sees the cut between them.
        let v = values(
            0.5,
            &[
                (parameter::MAX_TILT, num(10.0)),
                (parameter::MAX_ANGLE_STEP, num(2.0)),
            ],
        );
        let (x0, x1) = part.x_range();
        let mid = 0.5 * (x0 + x1);
        let outline = Outline::new(&f.tool);
        // The checks look at the middle of the part, away from the ends.
        let worst = |p: &GeneratedPath| {
            let poses: Vec<Pose> = fed(p)
                .into_iter()
                .filter(|q| (q.tip.x - mid).abs() < 5.0)
                .collect();
            let g = max_gouge(&outline, part.as_ref(), &poses, 0.15);
            assert!(g.depth <= 0.01, "{}: gouge {}", part.name(), g.depth);
            coverage(
                &outline,
                part.as_ref(),
                &poses,
                (mid - 2.0, mid + 2.0),
                0.2,
                0.05,
            )
        };
        let spiral = worst(&path(&f, &v).unwrap());
        let rings = worst(&path(&f, &ringed(v)).unwrap());
        assert_eq!(rings.uncovered, 0, "{}: {rings:?}", part.name());
        assert!(
            rings.worst <= spiral.worst + 2e-3,
            "{}: {rings:?} {spiral:?}",
            part.name()
        );
    }
}

#[test]
fn req_nfr_06_req_gc_07_rings_are_identical_at_1_2_and_n_threads() {
    let f = fixture(&lobed(), 41, 720);
    let v = ringed(values(1.0, &[(parameter::MAX_TILT, num(10.0))]));
    let runs = openchuck_testkit::determinism::check(|| {
        format!("{:?}", path(&f, &v).unwrap().group).into_bytes()
    })
    .unwrap();
    assert_eq!(runs.len(), 4);
}

#[test]
fn req_nfr_07_an_unknown_pass_shape_is_refused_by_name() {
    let f = fixture(
        &Cylinder {
            radius: 15.0,
            length: 10.0,
        },
        3,
        360,
    );
    let e = path(&f, &values(1.0, &[(parameter::PASS_SHAPE, text("zigzag"))]))
        .unwrap_err()
        .to_string();
    assert!(e.contains(parameter::PASS_SHAPE), "{e}");
}

/// Every cutting move either turns A at constant X (a ring) or moves along
/// X with A held still (a step), and the number of separate cuts.
fn rings_and_steps(p: &GeneratedPath, what: &str) -> usize {
    rings_and_maybe_steps(p, what, true)
}

/// As [`rings_and_steps`]; with `need_steps` false a path with no steps
/// (every step split away) is allowed.
fn rings_and_maybe_steps(p: &GeneratedPath, what: &str, need_steps: bool) -> usize {
    let (mut steps, mut turning) = (0, 0);
    for (from, s) in cuts(p) {
        let dx = s.end.tip.x - from.tip.x;
        let da = s.end.a.0 - from.a.0;
        // On a part whose surface slopes along X the tip of a ring moves a
        // little along X with the normal (the tool sits off the contact by
        // its radius), so a ring is told by A turning and a step by A held.
        if da.abs() < 1e-9 {
            if dx.abs() > 1e-9 {
                steps += 1;
            }
        } else {
            assert!(
                dx.abs() < 0.1,
                "{what}: a move turned A by {da} and moved {dx} along X at {:?}",
                s.end.tip
            );
            turning += 1;
        }
    }
    assert!(
        (steps > 0 || !need_steps) && turning > 0,
        "{what}: {steps} steps, {turning}"
    );
    let mut count = 0;
    let mut cutting = false;
    for s in &p.group.segments {
        let now = s.kind == MoveKind::Cutting;
        if now && !cutting {
            count += 1;
        }
        cutting = now;
    }
    count
}

#[test]
fn req_op_05_req_op_15_steps_hold_the_a_the_ring_ended_at_in_one_cut() {
    // The owner's choice D3 (decision record 0421, option 1): the step
    // holds the A the ring ended at; with nothing to split them the rings
    // and steps are one cut, with the lead or without it.
    let part = Cylinder {
        radius: 15.0,
        length: 10.0,
    };
    let f = fixture(&part, 3, 720);
    for (lead, start) in [(0.0, 0.0), (5.0, 30.0), (-5.0, -45.0)] {
        let p = path(
            &f,
            &ringed(values(
                1.0,
                &[
                    (parameter::LEAD_ANGLE, num(lead)),
                    (parameter::START_ANGLE, num(start)),
                ],
            )),
        )
        .unwrap();
        let what = format!("lead {lead}");
        assert_eq!(rings_and_steps(&p, &what), 1, "{what}");
        // A builds up one whole turn a ring (OP-15).
        let a: Vec<f64> = cuts(&p)
            .iter()
            .flat_map(|(from, s)| [from.a.0, s.end.a.0])
            .collect();
        let span = a.iter().copied().fold(f64::NEG_INFINITY, f64::max)
            - a.iter().copied().fold(f64::INFINITY, f64::min);
        assert!((span - 11.0 * 360.0).abs() < 1.0, "{what}: {span}");
    }
}

#[test]
fn req_op_05_req_op_06_on_a_lobed_part_steps_hold_a_and_the_tilt_stays_in_bounds() {
    // The section turns along X on the vase, so a held A leans from the
    // normal along the step; where it would lean more than the lead and
    // the tilt the step is split, never cut leaning further.
    let f = fixture(&lobed(), 41, 720);
    // With no tilt allowed a ring cannot start at the A the step held where
    // the normal there is not that angle; it starts on its own normal and
    // the shared services link the step to it.
    for tilt in [10.0, 2.0, 0.0] {
        let p = path(
            &f,
            &ringed(values(1.0, &[(parameter::MAX_TILT, num(tilt))])),
        )
        .unwrap();
        let cuts = rings_and_maybe_steps(&p, &format!("tilt {tilt}"), tilt > 0.0);
        if tilt == 0.0 {
            // Every ring is its own cut, joined by the shared services.
            assert!(cuts > 1, "{cuts}");
        }
        assert!(p.notes.iter().any(|n| n.message.contains("rings")));
    }
}

#[test]
fn req_nfr_07_rings_refuse_an_absurd_stepover_by_name() {
    // The count of points is checked before any ring is placed, so a
    // stepover too fine to cut is refused by name, never counted out
    // (review of #447).
    let f = fixture(
        &Cylinder {
            radius: 15.0,
            length: 10.0,
        },
        3,
        360,
    );
    for stepover in [1e-20, f64::MIN_POSITIVE] {
        let e = path(&f, &ringed(values(stepover, &[])))
            .unwrap_err()
            .to_string();
        assert!(e.contains(names::STEPOVER), "{stepover}: {e}");
    }
}
