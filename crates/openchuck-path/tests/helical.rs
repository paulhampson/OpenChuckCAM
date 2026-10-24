// SPDX-License-Identifier: AGPL-3.0-or-later
//! Helical finishing of the whole model (OP-04, with OP-05 kinematics, OP-12,
//! OP-13, OP-15; work breakdown slice 5 item 5 and its review checklist;
//! decision record 0018 item 12).
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing
)]

mod support;

use std::collections::BTreeMap;

use openchuck_path::helical::{CHORD_FACTOR, parameter};
use openchuck_path::params::names;
use openchuck_path::rotary::{AxisPosition, reversals};
use openchuck_path::{
    GeneratedPath, HelicalFinishing, MeshSurface, ParameterValue, PathError, SurfaceSource,
    generate,
};
use openchuck_testkit::gouge::{Outline, Pose, coverage, max_gouge, min_clearance};
use openchuck_testkit::parts::{
    Cone, Cylinder, HelicalGroove, RadialPart, RuledHyperboloid, Sphere,
};
use openchuck_types::ir::{MoveKind, ToolPose};
use openchuck_types::tool::ToolShape;
use openchuck_types::units::{Degrees, Millimetres};
use support::{Fixture, ball, run, tool};

fn values(pitch: f64, extra: &[(&str, ParameterValue)]) -> BTreeMap<String, ParameterValue> {
    let mut v: BTreeMap<String, ParameterValue> = [
        (names::STEPOVER, pitch),
        (names::FEED, 300.0),
        (names::PLUNGE_FEED, 100.0),
    ]
    .into_iter()
    .map(|(k, x)| (k.to_owned(), ParameterValue::Number(x)))
    .collect();
    for (k, x) in extra {
        v.insert((*k).to_owned(), x.clone());
    }
    v
}

fn num(x: f64) -> ParameterValue {
    ParameterValue::Number(x)
}

/// A fixture around a radial part: stock a little larger than the part.
fn fixture<P: RadialPart + ?Sized>(
    part: &P,
    rings: u32,
    around: u32,
    tool_shape: &openchuck_types::tool::Tool,
) -> Fixture {
    let surface = MeshSurface::new(&part.mesh(rings, around)).unwrap();
    let b = surface.bounds();
    let r = b
        .max
        .y
        .abs()
        .max(b.max.z.abs())
        .max(b.min.y.abs())
        .max(b.min.z.abs());
    Fixture::new(surface, r + 1.0, tool_shape)
}

fn helical(fixture: &Fixture, v: &BTreeMap<String, ParameterValue>) -> GeneratedPath {
    run(|ctx| generate(&HelicalFinishing, &fixture.input(), v, ctx)).unwrap()
}

/// The cutting segments with the pose each starts from.
fn cuts(path: &GeneratedPath) -> Vec<(ToolPose, openchuck_types::ir::Segment)> {
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

fn poses(path: &GeneratedPath) -> Vec<Pose> {
    path.group
        .segments
        .iter()
        .filter(|s| matches!(s.kind, MoveKind::Cutting | MoveKind::Plunge))
        .map(|s| Pose {
            tip: s.end.tip,
            axis: s.end.axis.get(),
        })
        .collect()
}

#[test]
fn req_op_04_helix_pitch_and_a_per_x_match_the_analytic_values_on_a_cylinder() {
    let part = Cylinder {
        radius: 15.0,
        length: 40.0,
    };
    let f = fixture(&part, 3, 1440, &ball(6.0));
    for (pitch, rotation, start) in [
        (1.0, "positive", 0.0),
        (0.5, "negative", 30.0),
        (2.5, "positive", -90.0),
    ] {
        let path = helical(
            &f,
            &values(
                pitch,
                &[
                    (parameter::ROTATION, ParameterValue::Text(rotation.into())),
                    (parameter::START_ANGLE, num(start)),
                ],
            ),
        );
        let sign = if rotation == "negative" { -1.0 } else { 1.0 };
        let cuts = cuts(&path);
        assert!(cuts.len() > 100);
        let mut x_prev = None;
        for (from, s) in &cuts {
            let dx = s.end.tip.x - from.tip.x;
            let da = s.end.a.0 - from.a.0;
            // The analytic helix: 360 degrees every `pitch` mm of X.
            assert!(
                (da - sign * 360.0 * dx / pitch).abs() < 1e-6,
                "pitch {pitch}: dx {dx}, dA {da}"
            );
            assert!(dx >= -1e-12, "X never goes back");
            // The ball rides the cylinder: the tip is R from the axis.
            let r = s.end.tip.y.hypot(s.end.tip.z);
            assert!((r - 15.0).abs() < 2e-3, "{r}");
            x_prev = Some(s.end.tip.x);
        }
        // From one end of the part to the other.
        let first = cuts.first().unwrap();
        let last = cuts.last().unwrap();
        assert!((first.0.tip.x - 0.0).abs() < 1e-9 && (last.1.end.tip.x - 40.0).abs() < 1e-9);
        let span = last.1.end.a.0 - first.0.a.0;
        assert!((span - sign * 360.0 * 40.0 / pitch).abs() < 1e-6, "{span}");
        assert!((first.0.a.0.rem_euclid(360.0) - start.rem_euclid(360.0)).abs() < 1e-6);
        assert!(x_prev.is_some());
        // A keeps going one way: no reversals, and no NaN anywhere.
        let angles: Vec<Degrees> = cuts.iter().map(|(_, s)| s.end.a).collect();
        assert_eq!(reversals(&angles, 1e-9), 0);
        for s in &path.group.segments {
            assert!(s.end.tip.is_finite() && s.end.a.0.is_finite() && s.duration.0.is_finite());
        }
        // Safe retract retains the last cutting angle (decision 0238).
        let end = path.group.segments.last().unwrap().end.a;
        assert_eq!(end, cuts.last().unwrap().1.end.a);
    }
}

#[test]
fn req_op_04_req_op_13_contact_feed_is_the_programmed_feed_on_every_cut() {
    let part = Cylinder {
        radius: 15.0,
        length: 20.0,
    };
    let f = fixture(&part, 3, 1440, &ball(6.0));
    let path = helical(&f, &values(1.0, &[]));
    for (_, s) in cuts(&path) {
        assert_eq!(s.contact_feed.unwrap().0, 300.0);
        // Duration is contact length over the feed: the documented formula
        // t = 60 L / f.
        assert!((s.duration.0 - 60.0 * s.contact_length.0 / 300.0).abs() < 1e-9);
    }
    // On a cylinder the contact moves as the part turns: dx axially and
    // R dA round, so the contact length is their hypotenuse.
    let (from, s) = cuts(&path)[10];
    let dx = s.end.tip.x - from.tip.x;
    let arc = 15.0 * (s.end.a.0 - from.a.0).to_radians();
    assert!((s.contact_length.0 - dx.hypot(arc)).abs() < 2e-2 * s.contact_length.0.max(1e-3));
}

/// The error of the straight machine move between two poses at `fractions`
/// of the way, for a ball of radius `rho` over a sphere of radius `radius`
/// about the origin: the tool centre's distance from the sphere's centre
/// minus the radius of the sphere plus the ball (along the surface normal).
fn sphere_errors(
    f: &Fixture,
    from: &ToolPose,
    to: &ToolPose,
    radius: f64,
    rho: f64,
    leave: f64,
    fractions: &[f64],
) -> Vec<f64> {
    let (p0, p1) = (
        f.rotary.to_axis(from.tip, from.a),
        f.rotary.to_axis(to.tip, to.a),
    );
    fractions
        .iter()
        .map(|t| {
            let at = AxisPosition {
                along: p0.along + t * (p1.along - p0.along),
                across: p0.across + t * (p1.across - p0.across),
                height: p0.height + t * (p1.height - p0.height),
            };
            let a = Degrees(from.a.0 + t * (to.a.0 - from.a.0));
            // Ball centre in the model frame (the sphere is at the origin).
            let tip = f.rotary.to_model(at, a);
            let centre = tip + f.rotary.tool_axis(a).get() * rho;
            centre.to_vec().length() - (radius + leave + rho)
        })
        .collect()
}

#[test]
fn req_op_04_req_nfr_05_path_fitting_stays_inside_its_budget_on_a_sphere() {
    // Tolerance 0.01 mm: 0.005 for tessellation and 0.005 for path fitting
    // (verification plan 4.1). The mesh here is 2e-4 mm off the exact sphere.
    let r = 20.0;
    let rho = 3.0;
    let part = Sphere { radius: r };
    let f = fixture(&part, 361, 360, &ball(2.0 * rho));
    let path = helical(&f, &values(1.0, &[]));
    let fractions = [0.1, 0.25, 0.4, 0.5, 0.6, 0.75, 0.9];
    let (mut worst_node, mut worst_move) = (0.0_f64, 0.0_f64);
    let cuts = cuts(&path);
    assert!(cuts.len() > 1000, "{} cutting moves", cuts.len());
    for (from, s) in &cuts {
        let at_ends = sphere_errors(&f, from, &s.end, r, rho, 0.0, &[0.0, 1.0]);
        worst_node = worst_node.max(at_ends.iter().fold(0.0, |m, e| m.max(e.abs())));
        let inside = sphere_errors(&f, from, &s.end, r, rho, 0.0, &fractions);
        worst_move = worst_move.max(inside.iter().fold(0.0, |m, e| m.max(e.abs())));
    }
    eprintln!(
        "sphere: {} cutting moves, worst error at points {worst_node:.5} mm, inside moves {worst_move:.5} mm",
        cuts.len()
    );
    // The points sit on the mesh, which is within its own error of the sphere.
    assert!(worst_node < 5e-4, "{worst_node}");
    // Inside the moves: the path-fitting budget, with the mesh error on top.
    assert!(worst_move <= 0.005 + 5e-4, "{worst_move}");
    const { assert!(CHORD_FACTOR * 0.01 + 5e-4 < 0.005) };
}

#[test]
fn req_op_04_a_tighter_tolerance_adds_points_and_tightens_the_fit() {
    let r = 20.0;
    let rho = 3.0;
    let part = Sphere { radius: r };
    let f = fixture(&part, 361, 360, &ball(2.0 * rho));
    let mut counts = Vec::new();
    let mut errors = Vec::new();
    // A wide pitch and big angle steps make long moves, so the sphere's
    // curvature along X (sagitta about 0.0085 mm per move) needs points.
    for tol in [0.04, 0.01, 0.0025] {
        let path = helical(
            &f,
            &values(
                8.0,
                &[
                    (names::TOLERANCE, num(tol)),
                    (parameter::MAX_ANGLE_STEP, num(60.0)),
                ],
            ),
        );
        let cuts = cuts(&path);
        let mut worst = 0.0_f64;
        for (from, s) in &cuts {
            for e in sphere_errors(&f, from, &s.end, r, rho, 0.0, &[0.25, 0.5, 0.75]) {
                worst = worst.max(e.abs());
            }
        }
        // Path fitting uses CHORD_FACTOR of the tolerance, plus the mesh.
        assert!(worst <= CHORD_FACTOR * tol + 5e-4, "tol {tol}: {worst}");
        eprintln!(
            "tolerance {tol}: {} cutting moves, worst fit error {worst:.5} mm",
            cuts.len()
        );
        counts.push(cuts.len());
        errors.push(worst);
    }
    assert!(counts[0] < counts[1] && counts[1] < counts[2], "{counts:?}");
    assert!(errors[2] < errors[0], "{errors:?}");
}

#[test]
fn req_op_11_stock_to_leave_leaves_the_stock_everywhere() {
    let leave = 0.3;
    // A cylinder, a sphere and a cone, each with several tool shapes: the
    // independent checker measures the clearance of every pose to the exact
    // surface, which must be the stock to leave.
    let shapes: Vec<(&str, openchuck_types::tool::Tool)> = vec![
        ("ball", ball(6.0)),
        ("flat", tool(ToolShape::FlatEnd, 6.0)),
        (
            "bull-nose",
            tool(
                ToolShape::BullNose {
                    corner_radius: Millimetres(1.0),
                },
                6.0,
            ),
        ),
        (
            "v-bit",
            tool(
                ToolShape::VBit {
                    included_angle: Degrees(60.0),
                    tip_diameter: Millimetres(0.0),
                },
                6.0,
            ),
        ),
    ];
    let cylinder = Cylinder {
        radius: 15.0,
        length: 30.0,
    };
    let cone = Cone {
        start_radius: 18.0,
        end_radius: 9.0,
        length: 30.0,
    };
    for (label, t) in &shapes {
        for part in [&cylinder as &dyn RadialPart, &cone] {
            let f = fixture(part, 61, 1440, t);
            let path = helical(&f, &values(3.0, &[(names::STOCK_TO_LEAVE, num(leave))]));
            let outline = Outline::new(&f.tool);
            let all = poses(&path);
            let mut worst_gouge = f64::INFINITY;
            let mut touching = 0usize;
            for pose in all.iter().step_by(7) {
                let c = min_clearance(&outline, part, pose, 0.05);
                worst_gouge = worst_gouge.min(c);
                if (c - leave).abs() < 0.01 {
                    touching += 1;
                }
            }
            // Never closer than the stock (less the mesh's own error), and
            // the tool does sit on the left-over surface.
            assert!(
                worst_gouge >= leave - 0.01,
                "{label} on {}: clearance {worst_gouge}",
                part.name()
            );
            assert!(
                touching > 20,
                "{label} on {}: {touching} poses touch",
                part.name()
            );
        }
    }
}

#[test]
fn req_op_11_stock_to_leave_is_refused_for_a_concave_tool_and_bad_values() {
    use openchuck_types::tool::{ProfilePoint, ProfileSegment, ToolProfile};
    let part = Cylinder {
        radius: 15.0,
        length: 20.0,
    };
    // A flat with a concave (corner-rounding) arc.
    let concave = tool(
        ToolShape::Custom {
            profile: ToolProfile {
                segments: vec![
                    ProfileSegment::Line {
                        to: ProfilePoint::new(2.0, 0.0),
                    },
                    // Concave: the centre is outside the tool, beside the end.
                    ProfileSegment::Arc {
                        to: ProfilePoint::new(3.0, 1.0),
                        centre: ProfilePoint::new(3.0, 0.0),
                    },
                ],
            },
        },
        6.0,
    );
    let f = fixture(&part, 3, 720, &concave);
    // With no stock the tool works.
    assert!(
        run(|ctx| generate(&HelicalFinishing, &f.input(), &values(4.0, &[]), ctx)).is_ok(),
        "a concave tool still finishes with no stock to leave"
    );
    let e = run(|ctx| {
        generate(
            &HelicalFinishing,
            &f.input(),
            &values(4.0, &[(names::STOCK_TO_LEAVE, num(0.2))]),
            ctx,
        )
    })
    .unwrap_err()
    .to_string();
    assert!(
        e.contains("not convex") && e.contains("stock to leave"),
        "{e}"
    );
    let e = openchuck_path::cutter::grow_profile(&f.tool, -1.0)
        .unwrap_err()
        .to_string();
    assert!(e.contains("not negative"), "{e}");
}

#[test]
fn req_op_12_the_independent_checker_finds_no_gouge_and_full_coverage_on_the_oracle_parts() {
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
        (Box::new(Sphere { radius: 16.0 }), 361, 360),
        (
            Box::new(HelicalGroove {
                radius: 15.0,
                length: 30.0,
                pitch: 20.0,
                depth: 2.0,
            }),
            181,
            720,
        ),
        (
            Box::new(RuledHyperboloid {
                end_radius: 15.0,
                length: 30.0,
                twist: 1.0,
            }),
            181,
            720,
        ),
    ];
    for (part, rings, around) in &parts {
        let (x0, x1) = part.x_range();
        let f = fixture(part.as_ref(), *rings, *around, &ball(6.0));
        // Points at most 1 degree apart (about 0.26 mm), so the tool between
        // two of them cuts nothing they do not (coverage needs dense poses).
        let path = helical(&f, &values(0.5, &[(parameter::MAX_ANGLE_STEP, num(1.0))]));
        let all = poses(&path);
        assert!(all.len() > 10_000, "{}: {} poses", part.name(), all.len());
        let outline = Outline::new(&f.tool);
        // No gouge beyond the mesh's own error (verification plan 4.1).
        let near: Vec<Pose> = all
            .iter()
            .filter(|p| (p.tip.x - 0.5 * (x0 + x1)).abs() < 6.0)
            .copied()
            .collect();
        let near: Vec<Pose> = near.into_iter().step_by(3).collect();
        let g = max_gouge(&outline, part.as_ref(), &near, 0.15);
        assert!(g.depth <= 0.002, "{}: gouge {}", part.name(), g.depth);
        // Full coverage: every exact surface point is within 0.02 mm of the
        // tool (the scallop of a 3 mm ball at 0.5 mm stepover is 0.01 mm).
        let mid = 0.5 * (x0 + x1);
        let c = coverage(
            &outline,
            part.as_ref(),
            &all,
            (mid - 3.0, mid + 3.0),
            0.2,
            0.02,
        );
        assert_eq!(c.uncovered, 0, "{}: {c:?}", part.name());
        // Seeded faults are caught: drop a stretch of the helix, or cut too deep.
        let gap: Vec<Pose> = all
            .iter()
            .filter(|p| (p.tip.x - mid).abs() > 1.0)
            .copied()
            .collect();
        let c = coverage(
            &outline,
            part.as_ref(),
            &gap,
            (mid - 3.0, mid + 3.0),
            0.2,
            0.02,
        );
        assert!(
            c.uncovered > 0,
            "{}: missing stretch not found",
            part.name()
        );
        let deep: Vec<Pose> = all
            .iter()
            .map(|p| Pose {
                tip: p.tip + p.axis * -0.03,
                axis: p.axis,
            })
            .collect();
        assert!(
            max_gouge(
                &outline,
                part.as_ref(),
                &deep
                    .iter()
                    .filter(|p| (p.tip.x - 0.5 * (x0 + x1)).abs() < 2.0)
                    .copied()
                    .collect::<Vec<_>>(),
                0.15
            )
            .depth
                > 0.02,
            "{}: gouge not found",
            part.name()
        );
    }
}

#[test]
fn req_op_13_feed_at_the_contact_is_within_two_percent_on_a_grooved_part() {
    // The mesh contact jumps from facet to facet (decision record 0015); the
    // shared services smooth it, and the finest meshes keep the IR's contact
    // length within the 2 % of verification plan 4.1 (OI-77).
    let part = HelicalGroove {
        radius: 15.0,
        length: 14.0,
        pitch: 20.0,
        depth: 2.0,
    };
    let rho = 3.0;
    let rings: u32 = std::env::var("RINGS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(561);
    let around: u32 = std::env::var("AROUND")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1440);
    let f = fixture(&part, rings, around, &ball(2.0 * rho));
    let path = helical(&f, &values(1.0, &[(parameter::MAX_ANGLE_STEP, num(1.0))]));
    let feed = 300.0;
    let cuts = cuts(&path);
    assert!(cuts.len() > 300, "{}", cuts.len());
    let nearest = |centre: openchuck_types::geometry::Point3| {
        let at = |x: f64, t: f64| {
            let r = part.radius_at(x, t);
            openchuck_types::geometry::Point3::new(x, r * t.cos(), r * t.sin())
        };
        let (mut x, mut t) = (centre.x, centre.z.atan2(centre.y));
        let mut best = at(x, t).distance(centre);
        let (mut sx, mut st) = (0.5, 0.05);
        while sx > 1e-10 {
            let mut moved = false;
            for (dx, dt) in [(sx, 0.0), (-sx, 0.0), (0.0, st), (0.0, -st)] {
                let d = at(x + dx, t + dt).distance(centre);
                if d < best {
                    best = d;
                    x += dx;
                    t += dt;
                    moved = true;
                }
            }
            if !moved {
                sx *= 0.5;
                st *= 0.5;
            }
        }
        at(x, t)
    };
    let mut worst = 0.0_f64;
    let mut all_errors = Vec::new();
    // Judged away from the ends of the part: within a tool radius of an end
    // the ball rolls over the rim (x < 4 and x > 10 on this 14 mm part), whose contact is not smooth, and within
    // two moves of the ends of the pass the contact smoothing has a shorter
    // window (assemble.rs).
    let judged: Vec<_> = cuts
        .iter()
        .filter(|(from, s)| from.tip.x > 4.0 && s.end.tip.x < 10.0)
        .collect();
    assert!(judged.len() > 1500, "{}", judged.len());
    for (from, s) in judged.iter().step_by(4).copied() {
        let (p, q) = (
            f.rotary.to_axis(from.tip, from.a),
            f.rotary.to_axis(s.end.tip, s.end.a),
        );
        let mut length = 0.0;
        let mut last = None;
        for k in 0..=20 {
            let t = f64::from(k) / 20.0;
            let at = AxisPosition {
                along: p.along + t * (q.along - p.along),
                across: p.across + t * (q.across - p.across),
                height: p.height + t * (q.height - p.height),
            };
            let a = Degrees(from.a.0 + t * (s.end.a.0 - from.a.0));
            let tip = f.rotary.to_model(at, a);
            let contact = nearest(tip + f.rotary.tool_axis(a).get() * rho);
            if let Some(c) = last {
                length += openchuck_types::geometry::Point3::distance(c, contact);
            }
            last = Some(contact);
        }
        let effective = 60.0 * length / s.duration.0;
        if (effective - feed).abs() / feed > worst {
            eprintln!(
                "worst so far {:.4}: x {:.3} a {:.2} contact_length {:.5} exact length {length:.5} dx {:.4} da {:.3}",
                (effective - feed).abs() / feed,
                s.end.tip.x,
                s.end.a.0,
                s.contact_length.0,
                s.end.tip.x - from.tip.x,
                s.end.a.0 - from.a.0
            );
        }
        worst = worst.max((effective - feed).abs() / feed);
        all_errors.push((effective - feed).abs() / feed);
    }
    all_errors.sort_by(f64::total_cmp);
    eprintln!(
        "effective feed error on {rings} x {around}: median {:.4}, 95th {:.4}, worst {:.4}",
        all_errors[all_errors.len() / 2],
        all_errors[all_errors.len() * 95 / 100],
        worst
    );
    assert!(
        worst <= 0.02,
        "worst effective feed error {:.2}%",
        100.0 * worst
    );
}

#[test]
fn req_nfr_06_helical_finishing_is_identical_at_1_2_and_n_threads() {
    let part = Cone {
        start_radius: 18.0,
        end_radius: 8.0,
        length: 20.0,
    };
    let f = fixture(&part, 21, 720, &ball(6.0));
    let v = values(1.0, &[]);
    let runs = openchuck_testkit::determinism::check(|| {
        let path = run(|ctx| generate(&HelicalFinishing, &f.input(), &v, ctx)).unwrap();
        format!("{:?}", path.group).into_bytes()
    })
    .unwrap();
    assert_eq!(runs.len(), 4);
}

#[test]
fn req_nfr_07_bad_input_is_refused_with_a_message() {
    let part = Cylinder {
        radius: 15.0,
        length: 20.0,
    };
    let f = fixture(&part, 3, 720, &ball(6.0));
    let go = |v: &BTreeMap<String, ParameterValue>| {
        run(|ctx| generate(&HelicalFinishing, &f.input(), v, ctx)).map(|_| ())
    };
    // The stepover is required; the cutting data supplies it in a project.
    let mut v = values(1.0, &[]);
    v.remove(names::STEPOVER);
    let e = go(&v).unwrap_err().to_string();
    assert!(e.contains("stepover") && e.contains("required"), "{e}");
    for (name, bad) in [
        (parameter::MAX_ANGLE_STEP, num(0.0)),
        (parameter::MAX_ANGLE_STEP, num(120.0)),
        (parameter::ROTATION, ParameterValue::Text("sideways".into())),
        (names::STOCK_TO_LEAVE, num(-0.1)),
        (names::TOLERANCE, num(0.0)),
    ] {
        let e = go(&values(1.0, &[(name, bad)])).unwrap_err().to_string();
        assert!(e.contains(name), "{name}: {e}");
    }
    // An absurd job is refused, not run: 0.00001 mm pitch.
    let e = go(&values(1e-5, &[])).unwrap_err();
    assert!(matches!(e, PathError::Parameter { .. }), "{e}");
    assert!(e.to_string().contains("increase the stepover"), "{e}");
}

#[test]
fn req_op_05_req_op_06_every_helical_pose_round_trips_through_the_independent_oracle() {
    use openchuck_types::kinematics::{RotaryFrame, machine_xyz};
    use openchuck_types::machine::ASense;
    // The shared forward kinematics map each IR pose to a machine position;
    // the test-only oracle, written separately, maps it back to the tool tip
    // (architecture section 9). Both senses of A, and a limited range.
    let part = Cone {
        start_radius: 18.0,
        end_radius: 8.0,
        length: 20.0,
    };
    for sense in [ASense::Positive, ASense::Negative] {
        let mut f = fixture(&part, 21, 720, &ball(6.0));
        f.machine.a_sense = sense;
        f.rotary = openchuck_path::RotaryMapping::of_profile(&f.machine);
        let path = helical(&f, &values(1.0, &[]));
        let frame = RotaryFrame::of_profile(&f.machine);
        let oracle = openchuck_testkit::oracle::OracleMachine {
            origin_x: frame.origin.x,
            origin_y: frame.origin.y,
            origin_z: frame.origin.z,
            sense: sense.sign(),
        };
        let mut checked = 0;
        for s in
            std::iter::once(&path.group.start).chain(path.group.segments.iter().map(|s| &s.end))
        {
            let m = machine_xyz(&frame, s.tip, s.a);
            let back = openchuck_testkit::oracle::model_tip(&oracle, m.x, m.y, m.z, s.a.0);
            let tip = [s.tip.x, s.tip.y, s.tip.z];
            assert!(
                openchuck_testkit::oracle::distance(back, tip) < 1e-6,
                "{sense:?}: pose {checked}"
            );
            // The tool axis is machine +Z at that A.
            assert!((f.rotary.tool_axis(s.a).get() - s.axis.get()).length() < 1e-12);
            checked += 1;
        }
        assert!(checked > 300, "{checked} poses");
    }
}

/// A number at six decimals with no negative zero, so a last-bit difference
/// between platforms does not show.
fn six(x: f64) -> String {
    let r = (x * 1e6).round() / 1e6;
    format!("{:.6}", if r == 0.0 { 0.0 } else { r })
}

#[test]
fn req_nfr_08_the_helical_toolpath_matches_its_golden_file() {
    // A small cone, a 4 mm ball, 2 mm pitch, 30 degree steps: every move of
    // the group at six decimals. The same file must match on every platform
    // (verification plan policy 7; OI-64).
    let part = Cone {
        start_radius: 12.0,
        end_radius: 8.0,
        length: 8.0,
    };
    let f = fixture(&part, 9, 720, &ball(4.0));
    let path = helical(&f, &values(2.0, &[(parameter::MAX_ANGLE_STEP, num(30.0))]));
    let mut text =
        String::from("# helical finishing, cone r 12 to 8 over 8 mm, ball 4 mm, pitch 2\n");
    text.push_str("# kind tip_x tip_y tip_z a feed seconds\n");
    for s in &path.group.segments {
        text.push_str(&format!(
            "{:?} {} {} {} {} {} {}\n",
            s.kind,
            six(s.end.tip.x),
            six(s.end.tip.y),
            six(s.end.tip.z),
            six(s.end.a.0),
            s.contact_feed.map_or_else(|| "-".to_owned(), |f| six(f.0)),
            six(s.duration.0),
        ));
    }
    let golden = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/golden/helical_cone.txt");
    openchuck_testkit::golden::check(&golden, &text).unwrap();
}

#[test]
fn req_op_04_req_op_11_helical_scallop_sets_the_pitch_for_a_ball() {
    let part = Cylinder {
        radius: 15.0,
        length: 10.0,
    };
    let f = fixture(&part, 3, 1440, &ball(6.0));
    // A 3 mm ball leaving 0.01 mm cusps: pitch 2 sqrt(2 rho h - h^2).
    let h: f64 = 0.01;
    let pitch = 2.0 * (2.0 * 3.0 * h - h * h).sqrt();
    let path = helical(&f, &values(50.0, &[(parameter::SCALLOP, num(h))]));
    let cuts = cuts(&path);
    let (from, s) = cuts[10];
    let dx = s.end.tip.x - from.tip.x;
    let da = s.end.a.0 - from.a.0;
    assert!((da - 360.0 * dx / pitch).abs() < 1e-6, "{da} {dx} {pitch}");
}
