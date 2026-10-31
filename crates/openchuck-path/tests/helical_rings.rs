// SPDX-License-Identifier: AGPL-3.0-or-later
//! Helical finishing in rings (issue #423; decision record 0390; OP-04,
//! OP-11, OP-12, OP-14, OP-15, GC-07, NFR-06, NFR-07).
//!
//! With **Passes** set to **Rings**, each turn is one whole turn of A at a
//! fixed X, and the tool steps along the part at the Start angle to the
//! next ring, still cutting on the surface, with no retract between rings.
//! With the setting absent or **Spiral**, the helix is exactly as before.
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

use openchuck_path::helical::parameter;
use openchuck_path::params::names;
use openchuck_path::rotary::reversals;
use openchuck_path::{GeneratedPath, HelicalFinishing, MeshSurface, ParameterValue, SurfaceSource};
use openchuck_testkit::gouge::{Outline, Pose, coverage, max_gouge};
use openchuck_testkit::parts::{Cone, Cylinder, RadialPart};
use openchuck_types::ir::{MoveKind, Segment, ToolPose};
use openchuck_types::units::Degrees;
use support::{Fixture, ball, run};

const RINGS: &str = "rings";

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

fn rings(pitch: f64, extra: &[(&str, ParameterValue)]) -> BTreeMap<String, ParameterValue> {
    let mut v = values(pitch, extra);
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

fn helical(f: &Fixture, v: &BTreeMap<String, ParameterValue>) -> GeneratedPath {
    run(|ctx| generate(&HelicalFinishing, &f.input(), v, ctx)).unwrap()
}

/// The cutting segments with the pose each starts from.
fn cuts(path: &GeneratedPath) -> Vec<(ToolPose, Segment)> {
    let mut prev = path.group.start;
    let mut out = Vec::new();
    for s in &path.group.segments {
        if s.kind == MoveKind::Cutting {
            out.push((prev, *s));
        }
        prev = s.end;
    }
    out
}

/// The number of separate cuts: runs of cutting moves with something else
/// (a retract, a link, an entry) between them.
fn separate_cuts(path: &GeneratedPath) -> usize {
    let mut count = 0;
    let mut cutting = false;
    for s in &path.group.segments {
        let now = s.kind == MoveKind::Cutting;
        if now && !cutting {
            count += 1;
        }
        cutting = now;
    }
    count
}

/// The X of each ring, in cutting order: the runs of cutting moves at
/// constant X that turn A by a whole turn, with the A each turned.
fn ring_xs(path: &GeneratedPath) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    let (mut x, mut turned) = (f64::NAN, 0.0);
    let flush = |x: f64, turned: f64, out: &mut Vec<(f64, f64)>| {
        if turned > 1e-9 {
            out.push((x, turned));
        }
    };
    for (from, s) in cuts(path) {
        let still = (s.end.tip.x - from.tip.x).abs() < 1e-9;
        if still && (s.end.tip.x - x).abs() < 1e-9 {
            turned += (s.end.a.0 - from.a.0).abs();
        } else {
            flush(x, turned, &mut out);
            (x, turned) = if still {
                (s.end.tip.x, (s.end.a.0 - from.a.0).abs())
            } else {
                (f64::NAN, 0.0)
            };
        }
    }
    flush(x, turned, &mut out);
    out
}

#[test]
fn req_op_04_rings_turn_at_constant_x_and_step_at_constant_a() {
    let part = Cylinder {
        radius: 15.0,
        length: 40.0,
    };
    let f = fixture(&part, 3, 1440);
    for (pitch, rotation, start, n) in [
        (1.0, "positive", 0.0, 40_usize),
        (1.5, "negative", 30.0, 27),
        (2.5, "positive", -90.0, 16),
    ] {
        let path = helical(
            &f,
            &rings(
                pitch,
                &[
                    (parameter::ROTATION, text(rotation)),
                    (parameter::START_ANGLE, num(start)),
                ],
            ),
        );
        let sign = if rotation == "negative" { -1.0 } else { 1.0 };
        // Every cutting move either turns A at constant X (a ring) or moves
        // along X at constant A (a step), and the steps are at the Start
        // angle.
        for (from, s) in cuts(&path) {
            let dx = s.end.tip.x - from.tip.x;
            let da = s.end.a.0 - from.a.0;
            assert!(dx >= -1e-12, "pitch {pitch}: X never goes back ({dx})");
            if dx.abs() > 1e-9 {
                assert!(da.abs() < 1e-9, "pitch {pitch}: a step turned A by {da}");
                let at = (s.end.a.0 - start).rem_euclid(360.0);
                assert!(
                    !(1e-6..=360.0 - 1e-6).contains(&at),
                    "pitch {pitch}: a step at A {}",
                    s.end.a.0
                );
            } else {
                assert!(da * sign >= -1e-9, "pitch {pitch}: A turns the set way");
            }
            // The ball rides the cylinder.
            let r = s.end.tip.y.hypot(s.end.tip.z);
            assert!((r - 15.0).abs() < 2e-3, "{r}");
        }
        // Rings evenly spaced from one end of the part to the other, no
        // wider apart than the stepover, each a whole turn.
        let found = ring_xs(&path);
        assert_eq!(found.len(), n + 1, "pitch {pitch}: {found:?}");
        let spacing = 40.0 / n as f64;
        assert!(spacing <= pitch + 1e-12);
        for (k, (x, turned)) in found.iter().enumerate() {
            assert!(
                (x - spacing * k as f64).abs() < 1e-9,
                "pitch {pitch}: ring {k} at {x}"
            );
            assert!(
                (turned - 360.0).abs() < 1e-6,
                "pitch {pitch}: ring {k} turned {turned}"
            );
        }
        // One continuous cut: no retract or link between rings (OP-14).
        assert_eq!(separate_cuts(&path), 1, "pitch {pitch}");
    }
}

#[test]
fn req_op_04_req_op_11_the_scallop_sets_the_ring_spacing_for_a_ball() {
    let part = Cylinder {
        radius: 15.0,
        length: 20.0,
    };
    let f = fixture(&part, 3, 720);
    // A 6 mm ball and a 0.01 mm cusp: 2 sqrt(2 rho h) = 0.4895 mm, less than
    // the 1 mm stepover, so 41 gaps.
    let path = helical(&f, &rings(1.0, &[(parameter::SCALLOP, num(0.01))]));
    let found = ring_xs(&path);
    assert_eq!(found.len(), 42, "{} rings", found.len());
    let gap = found[1].0 - found[0].0;
    assert!(gap <= 2.0 * (2.0 * 3.0 * 0.01_f64).sqrt(), "{gap}");
}

#[test]
fn req_op_04_spiral_is_unchanged_when_rings_are_not_chosen() {
    let part = Cone {
        start_radius: 18.0,
        end_radius: 8.0,
        length: 20.0,
    };
    let f = fixture(&part, 21, 720);
    let missing = helical(&f, &values(1.0, &[]));
    let spiral = helical(&f, &values(1.0, &[(parameter::PASS_SHAPE, text("helix"))]));
    assert_eq!(
        format!("{:?}", missing.group),
        format!("{:?}", spiral.group)
    );
    assert_eq!(missing.notes, spiral.notes);
    // And the spiral is not rings: X moves on every cut.
    assert!(ring_xs(&spiral).is_empty());
}

#[test]
fn req_op_04_the_setting_is_a_choice_of_spiral_or_rings_defaulting_to_spiral() {
    use openchuck_path::Strategy;
    use openchuck_path::params::ParameterKind;
    let schema = HelicalFinishing.schema();
    let spec = schema.get(parameter::PASS_SHAPE).unwrap();
    assert_eq!(spec.label, "Passes");
    match &spec.kind {
        ParameterKind::Choice { options, default } => {
            assert_eq!(options, &["helix".to_owned(), RINGS.to_owned()]);
            assert_eq!(default, "helix");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn req_op_12_req_op_11_rings_cover_the_surface_without_gouging() {
    let parts: Vec<(Box<dyn RadialPart>, u32, u32)> = vec![
        (
            Box::new(Cylinder {
                radius: 15.0,
                length: 30.0,
            }),
            3,
            1440,
        ),
        (
            Box::new(Cone {
                start_radius: 18.0,
                end_radius: 8.0,
                length: 30.0,
            }),
            31,
            1080,
        ),
    ];
    for (part, rings_n, around) in &parts {
        let (x0, x1) = part.x_range();
        let f = fixture(part.as_ref(), *rings_n, *around);
        let path = helical(&f, &rings(0.5, &[(parameter::MAX_ANGLE_STEP, num(1.0))]));
        let all: Vec<Pose> = path
            .group
            .segments
            .iter()
            .filter(|s| matches!(s.kind, MoveKind::Cutting | MoveKind::Plunge))
            .map(|s| Pose {
                tip: s.end.tip,
                axis: s.end.axis.get(),
            })
            .collect();
        let outline = Outline::new(&f.tool);
        let mid = 0.5 * (x0 + x1);
        let near: Vec<Pose> = all
            .iter()
            .filter(|p| (p.tip.x - mid).abs() < 6.0)
            .step_by(3)
            .copied()
            .collect();
        let g = max_gouge(&outline, part.as_ref(), &near, 0.15);
        assert!(g.depth <= 0.002, "{}: gouge {}", part.name(), g.depth);
        let c = coverage(
            &outline,
            part.as_ref(),
            &all,
            (mid - 3.0, mid + 3.0),
            0.2,
            0.02,
        );
        assert_eq!(c.uncovered, 0, "{}: {c:?}", part.name());
    }
}

#[test]
fn req_op_11_rings_leave_the_stock_to_leave_everywhere() {
    let part = Cylinder {
        radius: 15.0,
        length: 20.0,
    };
    let f = fixture(&part, 3, 720);
    let path = helical(&f, &rings(1.0, &[(names::STOCK_TO_LEAVE, num(0.3))]));
    for (_, s) in cuts(&path) {
        let r = s.end.tip.y.hypot(s.end.tip.z);
        assert!((r - 15.3).abs() < 2e-3, "{r}");
    }
}

#[test]
fn req_op_15_rings_turn_one_way_with_no_reversal() {
    let part = Cylinder {
        radius: 15.0,
        length: 10.0,
    };
    let f = fixture(&part, 3, 720);
    for rotation in ["positive", "negative"] {
        let path = helical(&f, &rings(1.0, &[(parameter::ROTATION, text(rotation))]));
        let cuts = cuts(&path);
        let angles: Vec<Degrees> = cuts
            .first()
            .map(|(from, _)| from.a)
            .into_iter()
            .chain(cuts.iter().map(|(_, s)| s.end.a))
            .collect();
        assert_eq!(reversals(&angles, 1e-9), 0, "{rotation}");
        // A builds up by a turn a ring, as the spiral's does.
        let a: Vec<f64> = angles.iter().map(|d| d.0).collect();
        let span = a.iter().copied().fold(f64::NEG_INFINITY, f64::max)
            - a.iter().copied().fold(f64::INFINITY, f64::min);
        assert!((span - 11.0 * 360.0).abs() < 1e-6, "{rotation}: {span}");
    }
}

#[test]
fn req_nfr_06_req_gc_07_rings_are_identical_at_1_2_and_n_threads() {
    let part = Cone {
        start_radius: 18.0,
        end_radius: 8.0,
        length: 20.0,
    };
    let f = fixture(&part, 21, 720);
    let v = rings(1.0, &[]);
    let runs = openchuck_testkit::determinism::check(|| {
        let path = run(|ctx| generate(&HelicalFinishing, &f.input(), &v, ctx)).unwrap();
        format!("{:?}", path.group).into_bytes()
    })
    .unwrap();
    assert_eq!(runs.len(), 4);
}

#[test]
fn req_nfr_07_an_unknown_pass_shape_is_refused_by_name() {
    let part = Cylinder {
        radius: 15.0,
        length: 20.0,
    };
    let f = fixture(&part, 3, 720);
    let e = run(|ctx| {
        generate(
            &HelicalFinishing,
            &f.input(),
            &values(1.0, &[(parameter::PASS_SHAPE, text("zigzag"))]),
            ctx,
        )
    })
    .unwrap_err()
    .to_string();
    assert!(e.contains(parameter::PASS_SHAPE), "{e}");
    // An absurd job is refused, not run, in rings as in a spiral.
    let e = run(|ctx| generate(&HelicalFinishing, &f.input(), &rings(1e-5, &[]), ctx))
        .unwrap_err()
        .to_string();
    assert!(e.contains("increase the stepover"), "{e}");
}
