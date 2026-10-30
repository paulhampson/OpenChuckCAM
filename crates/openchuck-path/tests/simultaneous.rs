// SPDX-License-Identifier: AGPL-3.0-or-later
//! Simultaneous 4-axis finishing (OP-05, OP-06, with OP-12, OP-13, OP-15;
//! work breakdown slice 13 and its review checklist; decision record 0036).
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing
)]

mod support;

use std::collections::BTreeMap;

use openchuck_path::params::names;
use openchuck_path::rotary::reversals;
use openchuck_path::simultaneous::parameter;
use openchuck_path::{
    GeneratedPath, HelicalFinishing, MeshSurface, ParameterValue, PathError, SimultaneousFinishing,
    SurfaceSource, generate,
};
use openchuck_testkit::gouge::{Outline, Pose, coverage, max_gouge};
use openchuck_testkit::parts::{Cone, Cylinder, LobedVase, RadialPart, Sphere};
use openchuck_types::geometry::Point3;
use openchuck_types::ir::{MoveKind, ToolPose};
use openchuck_types::tool::{ProfilePoint, ProfileSegment, ToolProfile, ToolShape};
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

fn text(x: &str) -> ParameterValue {
    ParameterValue::Text(x.to_owned())
}

fn fixture<P: RadialPart + ?Sized>(
    part: &P,
    rings: u32,
    around: u32,
    tool: &openchuck_types::tool::Tool,
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
    Fixture::new(surface, r + 1.0, tool)
}

fn simultaneous(f: &Fixture, v: &BTreeMap<String, ParameterValue>) -> GeneratedPath {
    run(|ctx| generate(&SimultaneousFinishing, &f.input(), v, ctx)).unwrap()
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

/// The polar angle (degrees) of a direction's part perpendicular to X.
fn polar(v: [f64; 3]) -> f64 {
    v[2].atan2(v[1]).to_degrees()
}

fn wrap(d: f64) -> f64 {
    d - 360.0 * (d / 360.0).round()
}

/// The ball centre's nearest exact surface point, by a pattern search.
fn nearest<P: RadialPart>(part: &P, centre: Point3) -> (f64, f64) {
    let at = |x: f64, t: f64| {
        let r = part.radius_at(x, t);
        Point3::new(x, r * t.cos(), r * t.sin())
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
    (x, t)
}

/// The worst angle, in degrees, between each cut's tool axis and the exact
/// in-plane normal at its contact, less `expected` (signed), away from the
/// part's ends; a ball of radius `rho` finds the contact from its centre.
fn axis_errors(
    part: &LobedVase,
    path: &GeneratedPath,
    rho: f64,
    expected: f64,
) -> (f64, f64, usize) {
    let (mut worst, mut slope, mut count) = (0.0_f64, 0.0_f64, 0);
    for (_, s) in cuts(path)
        .iter()
        .filter(|(_, s)| s.end.tip.x > 3.0 && s.end.tip.x < part.length - 3.0)
    {
        let centre = s.end.tip + s.end.axis.get() * rho;
        let (x, t) = nearest(part, centre);
        let n = part.normal(x, t);
        let a = s.end.axis.get();
        let d = wrap(polar([a.x, a.y, a.z]) - polar(n)) - expected;
        worst = worst.max(d.abs());
        slope = slope.max(n[0].abs().asin().to_degrees());
        count += 1;
    }
    (worst, slope, count)
}

fn a_of(path: &GeneratedPath) -> Vec<Degrees> {
    cuts(path).iter().map(|(_, s)| s.end.a).collect()
}

#[test]
fn req_op_06_the_tool_axis_follows_the_normal_within_the_tilt_bound() {
    // SP-4's lobed part: its normal leaves the plane through the axis by up
    // to 22 degrees, so following it needs A and Y to move with X and Z.
    let part = lobed();
    let rho = 3.0;
    let f = fixture(&part, 401, 1440, &ball(2.0 * rho));
    // Max tilt 0: the axis is the exact in-plane normal to within the
    // smoothed mesh normal's error (OI-67).
    let path = simultaneous(&f, &values(1.0, &[(parameter::MAX_TILT, num(0.0))]));
    let (worst, slope, count) = axis_errors(&part, &path, rho, 0.0);
    eprintln!(
        "max tilt 0: worst tool-axis error {worst:.4} degrees over {count} cuts; X slope up to {slope:.2}"
    );
    assert!(count > 1500, "{count}");
    assert!(worst <= 0.1, "{worst}");
    // The X slope is what A cannot follow, and is reported.
    assert!(slope > 10.0, "{slope}");
    assert!(
        path.notes
            .iter()
            .any(|n| n.message.contains("slopes along the rotary axis")),
        "{:?}",
        path.notes
    );
    assert!(
        path.notes
            .iter()
            .any(|n| n.message.contains("against the mesh"))
    );
    // Max tilt 10: never more than 10 degrees off.
    let path = simultaneous(&f, &values(1.0, &[(parameter::MAX_TILT, num(10.0))]));
    let (worst, _, _) = axis_errors(&part, &path, rho, 0.0);
    eprintln!("max tilt 10: worst tool-axis error {worst:.4} degrees");
    assert!(worst <= 10.1, "{worst}");
    // A lead of 15 degrees leans the axis towards the travel, both ways.
    for rotation in ["positive", "negative"] {
        let path = simultaneous(
            &f,
            &values(
                1.0,
                &[
                    (parameter::MAX_TILT, num(0.0)),
                    (parameter::LEAD_ANGLE, num(15.0)),
                    (parameter::ROTATION, text(rotation)),
                ],
            ),
        );
        // The drive angle turns against A by the sense (A = sense (90 - phi)),
        // so A turning positive moves the contact the negative way round.
        let r = if rotation == "positive" { 1.0 } else { -1.0 };
        let travel = -f.machine.a_sense.sign() * r;
        let (worst, _, _) = axis_errors(&part, &path, rho, 15.0 * travel);
        eprintln!("lead 15 {rotation}: worst error {worst:.4} degrees");
        assert!(worst <= 0.1, "{rotation}: {worst}");
    }
}

#[test]
fn req_op_15_smoothing_has_fewer_a_reversals_than_following_the_normal() {
    // Deep lobes: following the normal turns A back on every lobe flank.
    let part = LobedVase {
        lobe: 3.0,
        ..lobed()
    };
    let f = fixture(&part, 201, 1440, &ball(6.0));
    let free = a_of(&simultaneous(
        &f,
        &values(1.0, &[(parameter::MAX_TILT, num(0.0))]),
    ));
    let mut previous = reversals(&free, 0.01);
    eprintln!("A reversals: {previous} following the normal");
    assert!(
        previous > 20,
        "the lobes should turn A back when it follows the normal"
    );
    let mut smooth = Vec::new();
    for tilt in [5.0, 10.0, 20.0] {
        let path = simultaneous(&f, &values(1.0, &[(parameter::MAX_TILT, num(tilt))]));
        smooth = a_of(&path);
        let r = reversals(&smooth, 0.01);
        eprintln!("A reversals within {tilt} degrees: {r}; {:?}", path.notes);
        assert!(r <= previous, "{r} reversals at {tilt} against {previous}");
        previous = r;
    }
    assert!(previous < reversals(&free, 0.01) / 2);
    // A stays continuous: no step of half a turn.
    for w in smooth.windows(2) {
        assert!((w[1].0 - w[0].0).abs() < 30.0, "{w:?}");
    }
}

#[test]
fn req_op_05_on_a_surface_of_revolution_the_path_is_the_helical_one() {
    // The in-plane normal of a round part points at the axis, so the tool
    // stays over the axis and A is the helical A; on the lobed part it is
    // not, and the difference is recorded (review checklist).
    let sphere = Sphere { radius: 16.0 };
    let f = fixture(&sphere, 361, 720, &ball(6.0));
    let path = simultaneous(&f, &values(1.0, &[]));
    let (mut worst, mut middle) = (0.0_f64, 0.0_f64);
    for (_, s) in cuts(&path) {
        let across = f.rotary.to_axis(s.end.tip, s.end.a).across.abs();
        worst = worst.max(across);
        if s.end.tip.x.abs() < 12.0 {
            middle = middle.max(across);
        }
    }
    eprintln!(
        "sphere: the tool leaves the axis by up to {middle:.4} mm within 12 mm of the equator, {worst:.4} mm overall"
    );
    // Near the poles the normal is almost along X and its in-plane part is
    // only as good as the mesh.
    assert!(middle < 0.005 && worst < 0.05, "{middle} {worst}");
    let part = lobed();
    let f = fixture(&part, 201, 1440, &ball(6.0));
    let path = simultaneous(&f, &values(1.0, &[(parameter::MAX_TILT, num(0.0))]));
    let sense = f.machine.a_sense.sign();
    let (mut a_off, mut y_off) = (0.0_f64, 0.0_f64);
    for (_, s) in cuts(&path) {
        let (_, t) = nearest(&part, s.end.tip + s.end.axis.get() * 3.0);
        let through = sense * (90.0 - t.to_degrees());
        a_off = a_off.max(wrap(s.end.a.0 - through).abs());
        y_off = y_off.max(f.rotary.to_axis(s.end.tip, s.end.a).across.abs());
    }
    let h = run(|ctx| generate(&HelicalFinishing, &f.input(), &values(1.0, &[]), ctx)).unwrap();
    let h_off = cuts(&h)
        .iter()
        .map(|(_, s)| f.rotary.to_axis(s.end.tip, s.end.a).across.abs())
        .fold(0.0_f64, f64::max);
    eprintln!(
        "lobed part: A differs from the through-axis angle by up to {a_off:.2} degrees, the tool \
         is up to {y_off:.3} mm across the axis (helical: {h_off:.3})"
    );
    assert!(a_off > 10.0 && y_off > 1.0 && h_off < 1e-9);
}

#[test]
fn req_op_12_the_independent_checker_finds_no_gouge_on_the_reference_parts() {
    // Sphere-like, tapered and S-curve parts (slice 13 item 7) and the
    // lobed part, with a 6 mm ball; the checker samples the exact surface.
    let parts: Vec<(Box<dyn RadialPart>, u32, u32)> = vec![
        (Box::new(Sphere { radius: 16.0 }), 361, 720),
        (
            Box::new(Cone {
                start_radius: 18.0,
                end_radius: 8.0,
                length: 30.0,
            }),
            31,
            1080,
        ),
        (
            Box::new(LobedVase {
                lobe: 0.0,
                ..lobed()
            }),
            401,
            720,
        ),
        (Box::new(lobed()), 401, 1440),
    ];
    for (part, rings, around) in &parts {
        let (x0, x1) = part.x_range();
        let mid = 0.5 * (x0 + x1);
        let f = fixture(part.as_ref(), *rings, *around, &ball(6.0));
        let path = simultaneous(&f, &values(0.5, &[(parameter::MAX_ANGLE_STEP, num(1.0))]));
        let all = poses(&path);
        let outline = Outline::new(&f.tool);
        let near: Vec<Pose> = all
            .iter()
            .filter(|p| (p.tip.x - mid).abs() < 2.5)
            .step_by(4)
            .copied()
            .collect();
        let g = max_gouge(&outline, part.as_ref(), &near, 0.15);
        eprintln!(
            "{}: {} poses, gouge {:.5} mm",
            part.name(),
            all.len(),
            g.depth
        );
        assert!(g.depth <= 0.002, "{}: gouge {g:?}", part.name());
        let c = coverage(
            &outline,
            part.as_ref(),
            &all,
            (mid - 1.5, mid + 1.5),
            0.2,
            0.02,
        );
        assert_eq!(c.uncovered, 0, "{}: {c:?}", part.name());
        // A seeded fault is caught.
        let deep: Vec<Pose> = near
            .iter()
            .map(|p| Pose {
                tip: p.tip + p.axis * -0.03,
                axis: p.axis,
            })
            .collect();
        assert!(max_gouge(&outline, part.as_ref(), &deep, 0.15).depth > 0.02);
    }
}

fn custom_concave() -> openchuck_types::tool::Tool {
    tool(
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
    )
}

#[test]
fn req_op_12_every_tool_profile_finishes_the_lobed_part_without_gouging() {
    // Tool-centre solving for every profile (slice 13 item 2): the normal
    // offset seeds convex tools, the drop places every one, and a concave
    // custom profile is dropped along the line through the drive point.
    let part = lobed();
    let shapes = vec![
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
            "tapered ball",
            tool(
                ToolShape::TaperedBall {
                    tip_radius: Millimetres(1.0),
                    taper_half_angle: Degrees(20.0),
                },
                6.0,
            ),
        ),
        (
            "V-bit",
            tool(
                ToolShape::VBit {
                    included_angle: Degrees(90.0),
                    tip_diameter: Millimetres(0.0),
                },
                6.0,
            ),
        ),
        (
            "chamfer",
            tool(
                ToolShape::Chamfer {
                    included_angle: Degrees(60.0),
                    tip_diameter: Millimetres(1.0),
                },
                6.0,
            ),
        ),
        (
            "drill",
            tool(
                ToolShape::Drill {
                    point_angle: Degrees(118.0),
                },
                6.0,
            ),
        ),
        ("custom concave", custom_concave()),
    ];
    let mesh = part.mesh(201, 1440);
    for (name, t) in &shapes {
        let surface = MeshSurface::new(&mesh).unwrap();
        let f = Fixture::new(surface, 19.0, t);
        let path = simultaneous(&f, &values(2.0, &[]));
        // A flat bottom on a surface sloping along X touches with its
        // corner, so its tip sits up to a tool radius along X from the
        // contact: judge the whole middle of the part, not a window round one X.
        let near: Vec<Pose> = poses(&path)
            .into_iter()
            .filter(|p| p.tip.x > 2.0 && p.tip.x < 18.0)
            .step_by(6)
            .collect();
        assert!(near.len() > 100, "{name}: {}", near.len());
        let g = max_gouge(&Outline::new(&f.tool), &part, &near, 0.15);
        eprintln!("{name}: gouge {:.5} mm over {} poses", g.depth, near.len());
        assert!(g.depth <= 0.002, "{name}: {g:?}");
    }
}

#[test]
fn req_op_11_stock_to_leave_is_left_everywhere() {
    let part = lobed();
    let f = fixture(&part, 201, 1440, &ball(6.0));
    let path = simultaneous(&f, &values(1.0, &[(names::STOCK_TO_LEAVE, num(0.3))]));
    let outline = Outline::new(&f.tool);
    let near: Vec<Pose> = poses(&path)
        .into_iter()
        .filter(|p| (p.tip.x - 10.0).abs() < 3.0)
        .step_by(5)
        .collect();
    let mut least = f64::INFINITY;
    for p in &near {
        least = least.min(openchuck_testkit::gouge::min_clearance(
            &outline, &part, p, 0.1,
        ));
    }
    assert!((0.29..0.31).contains(&least), "{least}");
}

#[test]
fn req_op_13_feed_at_the_contact_is_within_two_percent_on_the_lobed_part() {
    // The contact moves over the surface while X, Y, Z and A move together;
    // the exact contact path under the machine's linear motion is measured
    // against the programmed feed (verification plan 4.1), away from the ends.
    let part = lobed();
    let rho = 3.0;
    let f = fixture(&part, 801, 2880, &ball(2.0 * rho));
    let feed = 300.0;
    let path = simultaneous(&f, &values(1.0, &[(parameter::MAX_ANGLE_STEP, num(1.0))]));
    let cuts = cuts(&path);
    let judged: Vec<_> = cuts
        .iter()
        .filter(|(from, s)| from.tip.x > 5.0 && s.end.tip.x < 15.0)
        .collect();
    assert!(judged.len() > 2000, "{}", judged.len());
    let at = |x: f64, t: f64| {
        let r = part.radius_at(x, t);
        Point3::new(x, r * t.cos(), r * t.sin())
    };
    let mut errors = Vec::new();
    for (from, s) in judged.iter().step_by(7).copied() {
        let (p, q) = (
            f.rotary.to_axis(from.tip, from.a),
            f.rotary.to_axis(s.end.tip, s.end.a),
        );
        let mut length = 0.0;
        let mut last = None;
        for k in 0..=20 {
            let t = f64::from(k) / 20.0;
            let pos = openchuck_path::rotary::AxisPosition {
                along: p.along + t * (q.along - p.along),
                across: p.across + t * (q.across - p.across),
                height: p.height + t * (q.height - p.height),
            };
            let a = Degrees(from.a.0 + t * (s.end.a.0 - from.a.0));
            let tip = f.rotary.to_model(pos, a);
            let (x, th) = nearest(&part, tip + f.rotary.tool_axis(a).get() * rho);
            let contact = at(x, th);
            if let Some(c) = last {
                length += Point3::distance(c, contact);
            }
            last = Some(contact);
        }
        errors.push((60.0 * length / s.duration.0 - feed).abs() / feed);
    }
    errors.sort_by(f64::total_cmp);
    let worst = errors.last().copied().unwrap_or(0.0);
    eprintln!(
        "effective feed error: median {:.4}, 95th {:.4}, worst {worst:.4} over {} moves",
        errors[errors.len() / 2],
        errors[errors.len() * 95 / 100],
        errors.len()
    );
    assert!(
        worst <= 0.02,
        "worst effective feed error {:.2}%",
        100.0 * worst
    );
}

#[test]
fn req_op_05_req_op_06_every_pose_round_trips_through_the_independent_oracle() {
    use openchuck_types::kinematics::{RotaryFrame, machine_xyz};
    use openchuck_types::machine::ASense;
    let part = LobedVase {
        length: 6.0,
        ..lobed()
    };
    for sense in [ASense::Positive, ASense::Negative] {
        let mut f = fixture(&part, 61, 720, &ball(6.0));
        f.machine.a_sense = sense;
        f.rotary = openchuck_path::RotaryMapping::of_profile(&f.machine);
        let path = simultaneous(&f, &values(1.0, &[]));
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
            assert!((f.rotary.tool_axis(s.a).get() - s.axis.get()).length() < 1e-12);
            checked += 1;
        }
        assert!(checked > 300, "{checked} poses");
        // The machine Y moves: this is simultaneous XYZA, not a tool over the axis.
        let ys: Vec<f64> = path
            .group
            .segments
            .iter()
            .filter(|s| s.kind == MoveKind::Cutting)
            .map(|s| machine_xyz(&frame, s.end.tip, s.end.a).y)
            .collect();
        let spread = ys.iter().copied().fold(f64::MIN, f64::max)
            - ys.iter().copied().fold(f64::MAX, f64::min);
        assert!(spread > 2.0, "{sense:?}: Y spread {spread}");
    }
}

#[test]
fn req_op_05_axial_passes_follow_the_normal_and_fit_a_limited_a_range() {
    use openchuck_types::machine::ARange;
    let part = LobedVase {
        length: 10.0,
        ..lobed()
    };
    let rho = 3.0;
    let mut f = fixture(&part, 201, 1440, &ball(2.0 * rho));
    f.machine.a_range = ARange::Limited {
        min: Degrees(-200.0),
        max: Degrees(200.0),
    };
    f.rotary = openchuck_path::RotaryMapping::of_profile(&f.machine);
    let axial = values(
        3.0,
        &[
            (parameter::PATTERN, text("axial")),
            (parameter::MAX_TILT, num(0.0)),
        ],
    );
    let path = simultaneous(&f, &axial);
    let all = cuts(&path);
    assert!(!all.is_empty());
    for (_, s) in &all {
        assert!((-200.0..=200.0).contains(&s.end.a.0), "{}", s.end.a.0);
    }
    // Passes run along X: each starts near one end of the part and ends at
    // the other, alternating.
    let worst = {
        let (mut worst, mut slope, mut count) = (0.0_f64, 0.0_f64, 0);
        for (_, s) in all
            .iter()
            .filter(|(_, s)| s.end.tip.x > 3.0 && s.end.tip.x < 7.0)
        {
            let (x, t) = nearest(&part, s.end.tip + s.end.axis.get() * rho);
            let n = part.normal(x, t);
            let a = s.end.axis.get();
            worst = worst.max(wrap(polar([a.x, a.y, a.z]) - polar(n)).abs());
            slope = slope.max(n[0].abs());
            count += 1;
        }
        assert!(count > 100 && slope > 0.0);
        worst
    };
    assert!(worst <= 0.1, "{worst}");
    // A spiral turns A through many turns and cannot fit the limited range.
    let e = run(|ctx| generate(&SimultaneousFinishing, &f.input(), &values(1.0, &[]), ctx))
        .unwrap_err();
    assert!(matches!(e, PathError::RotaryLimit(_)), "{e}");
    // A lead needs travel round the axis.
    let mut lead = axial.clone();
    lead.insert(parameter::LEAD_ANGLE.to_owned(), num(5.0));
    let e = run(|ctx| generate(&SimultaneousFinishing, &f.input(), &lead, ctx))
        .unwrap_err()
        .to_string();
    assert!(e.contains("lead angle"), "{e}");
}

#[test]
fn req_nfr_06_simultaneous_finishing_is_identical_at_1_2_and_n_threads() {
    let part = LobedVase {
        length: 8.0,
        ..lobed()
    };
    let f = fixture(&part, 81, 720, &ball(6.0));
    let v = values(1.0, &[]);
    let runs = openchuck_testkit::determinism::check(|| {
        let path = run(|ctx| generate(&SimultaneousFinishing, &f.input(), &v, ctx)).unwrap();
        format!("{:?} {:?}", path.group, path.notes).into_bytes()
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
        run(|ctx| generate(&SimultaneousFinishing, &f.input(), v, ctx)).map(|_| ())
    };
    for (name, value) in [
        (parameter::MAX_TILT, num(-1.0)),
        (parameter::MAX_TILT, num(45.0)),
        (parameter::LEAD_ANGLE, num(40.0)),
        (parameter::MAX_ANGLE_STEP, num(0.0)),
        (parameter::PATTERN, text("zigzag")),
        (parameter::ROTATION, text("sideways")),
        (names::TOLERANCE, num(0.0)),
        (names::STOCK_TO_LEAVE, num(-0.1)),
    ] {
        let e = go(&values(1.0, &[(name, value.clone())]))
            .unwrap_err()
            .to_string();
        assert!(e.contains(name), "{name} {value:?}: {e}");
    }
    let e = go(&values(1e-6, &[])).unwrap_err().to_string();
    assert!(e.contains("stepover"), "{e}");
    // A concave tool cannot leave stock.
    let f = fixture(&part, 3, 720, &custom_concave());
    let e = run(|ctx| {
        generate(
            &SimultaneousFinishing,
            &f.input(),
            &values(2.0, &[(names::STOCK_TO_LEAVE, num(0.2))]),
            ctx,
        )
    })
    .unwrap_err()
    .to_string();
    assert!(e.contains("not convex"), "{e}");
}

#[test]
fn req_op_16_the_strategy_is_built_in_and_its_schema_starts_with_the_common_parameters() {
    let registry = openchuck_path::Registry::builtin();
    let s = registry.get(openchuck_path::simultaneous::NAME).unwrap();
    let schema = s.schema();
    let names: Vec<&str> = schema.parameters.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(
        names.first(),
        Some(&openchuck_path::params::names::TOLERANCE)
    );
    for p in [
        parameter::PATTERN,
        parameter::LEAD_ANGLE,
        parameter::MAX_TILT,
        parameter::MAX_ANGLE_STEP,
    ] {
        assert!(names.contains(&p), "{p}");
    }
    assert!(s.honours_region());
}

/// A B-rep cylinder face about the rotary axis (X), radius `r`, from x = 0
/// to `length`, with a coarse tessellation for the tool drop (OI-81).
fn brep_cylinder(r: f64, length: f64) -> openchuck_types::brep::BrepModel {
    use openchuck_types::brep::{BrepFace, BrepModel, FaceId, Frame, Surface};
    use openchuck_types::geometry::Vec3;
    use openchuck_types::mesh::TriangleMesh;
    let (nu, nv) = (720usize, 40usize);
    let mut tess = TriangleMesh::default();
    for i in 0..=nu {
        for j in 0..=nv {
            let a = std::f64::consts::TAU * i as f64 / nu as f64;
            tess.vertices.push(Point3::new(
                length * j as f64 / nv as f64,
                r * a.cos(),
                r * a.sin(),
            ));
        }
    }
    let at = |i: usize, j: usize| u32::try_from(i * (nv + 1) + j).unwrap();
    for i in 0..nu {
        for j in 0..nv {
            tess.triangles
                .push([at(i, j), at(i + 1, j + 1), at(i + 1, j)]);
            tess.triangles
                .push([at(i, j), at(i, j + 1), at(i + 1, j + 1)]);
        }
    }
    let z = Vec3::new(1.0, 0.0, 0.0);
    let x = Vec3::new(0.0, 1.0, 0.0);
    let geometry = Surface::Cylinder {
        frame: Frame {
            origin: Point3::ORIGIN,
            x,
            y: z.cross(x),
            z,
        },
        radius: r,
    };
    BrepModel {
        faces: vec![BrepFace {
            id: FaceId(0),
            surface: geometry.kind(),
            body: 0,
            edges: vec![],
            geometry,
            reversed: false,
            uv_bounds: [0.0, std::f64::consts::TAU, 0.0, length],
            trims: vec![],
            tessellation: tess,
        }],
        ..BrepModel::default()
    }
}

#[test]
fn req_op_05_exact_faces_drive_the_path_with_their_own_normals() {
    // Slice 13 item 1: on a B-rep source the drive points and normals come
    // from the exact faces; the tool drop still uses the tessellation (OI-81).
    let model = brep_cylinder(12.0, 10.0);
    let surface = openchuck_path::BrepSurface::new(&model).unwrap();
    let hit = surface
        .cast_ray(
            Point3::new(5.0, 20.0, 0.0),
            openchuck_types::geometry::UnitVec3::new(openchuck_types::geometry::Vec3::new(
                0.0, -1.0, 0.0,
            ))
            .unwrap(),
            30.0,
        )
        .unwrap();
    assert!(
        hit.position.is_some() && hit.normal.get().y > 0.999_999,
        "{hit:?}"
    );
    // The fixture's stock, tool and machine, with the exact source in place
    // of its mesh.
    let f = Fixture::new(
        MeshSurface::new(surface.tessellation()).unwrap(),
        13.0,
        &ball(6.0),
    );
    let mut input = f.input();
    input.surface = &surface;
    let path = run(|ctx| generate(&SimultaneousFinishing, &input, &values(1.0, &[]), ctx)).unwrap();
    let c = cuts(&path);
    assert!(c.len() > 100, "{}", c.len());
    for (_, s) in c
        .iter()
        .filter(|(_, s)| s.end.tip.x > 2.0 && s.end.tip.x < 8.0)
    {
        let across = f.rotary.to_axis(s.end.tip, s.end.a).across;
        assert!(across.abs() < 1e-6, "{across}");
        let r = s.end.tip.y.hypot(s.end.tip.z);
        assert!((r - 12.0).abs() < 0.01, "{r}");
    }
    assert!(
        !path
            .notes
            .iter()
            .any(|n| n.message.contains("against the mesh")),
        "{:?}",
        path.notes
    );
}

#[test]
fn req_op_02_cuts_stay_on_the_selected_region() {
    use openchuck_path::generate_in_region;
    use openchuck_path::region::RegionConstraint;
    let mesh = Cylinder {
        radius: 15.0,
        length: 40.0,
    }
    .mesh(81, 720);
    let f = Fixture::new(MeshSurface::new(&mesh).unwrap(), 16.0, &ball(6.0));
    let upper = openchuck_types::mesh::TriangleMesh {
        vertices: mesh.vertices.clone(),
        triangles: (0..mesh.triangles.len())
            .filter(|&t| mesh.triangle(t).unwrap().iter().all(|c| c.x >= 20.0 - 1e-9))
            .map(|t| mesh.triangles[t])
            .collect(),
    };
    let upper = MeshSurface::new(&upper).unwrap();
    let region = RegionConstraint {
        region: Some(&upper),
        ..Default::default()
    };
    let path = run(|ctx| {
        generate_in_region(
            &SimultaneousFinishing,
            &f.input(),
            &region,
            &values(1.0, &[]),
            ctx,
        )
    })
    .unwrap();
    let tips: Vec<Point3> = cuts(&path).iter().map(|(_, s)| s.end.tip).collect();
    assert!(!tips.is_empty());
    let lo = tips.iter().map(|p| p.x).fold(f64::MAX, f64::min);
    let hi = tips.iter().map(|p| p.x).fold(f64::MIN, f64::max);
    assert!(lo >= 20.0 - 1e-3, "lowest cut at x = {lo}");
    assert!(lo <= 20.0 + 0.05, "the cut stops short at x = {lo}");
    assert!(hi >= 40.0 - 0.1, "highest cut at x = {hi}");
}
