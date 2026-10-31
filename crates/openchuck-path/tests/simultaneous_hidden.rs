// SPDX-License-Identifier: AGPL-3.0-or-later
//! Simultaneous finishing of surfaces that a ray from outside towards the
//! rotary axis (or towards a section's own centre, OP-26) does not meet
//! first: undercuts, the walls of slots, the sides of separate pieces facing
//! each other (OP-20, with OP-06, OP-12, NFR-06; decision record 0368; issue
//! #413).
//!
//! The parts are prisms along X and a surface of revolution, whose exact
//! surface is known: the checks measure points of it against the tool's
//! outline (`openchuck-testkit`) at every fed pose of the path, independently
//! of the strategy.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::print_stderr
)]

mod support;

use std::collections::BTreeMap;
use std::f64::consts::{FRAC_PI_2, TAU};

use openchuck_path::params::names;
use openchuck_path::rotary::AxisPosition;
use openchuck_path::simultaneous::parameter;
use openchuck_path::{
    GeneratedPath, MeshSurface, ParameterValue, RotaryMapping, Shank, SimultaneousFinishing,
    StrategyInput, UnreachedReason, generate,
};
use openchuck_testkit::gouge::{Outline, Pose};
use openchuck_testkit::parts::{join, prism, revolve};
use openchuck_types::geometry::{Point3, Vec3};
use openchuck_types::ir::MoveKind;
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::tool::profile::{ProfilePoint, ProfileSegment, ToolProfile};
use openchuck_types::tool::{RevolvedProfile, ToolShape};
use openchuck_types::units::{Degrees, Millimetres};
use support::{Fixture, ball, read_binary_stl, run, tool};

const TOLERANCE: f64 = 0.01;
/// The Z1 3.175 mm ball.
const DIAMETER: f64 = 3.175;
const STEPOVER: f64 = 0.3;
/// The scallop a 3.175 mm ball leaves on a flat at [`STEPOVER`] (0.007 mm),
/// with the tolerance.
const LEFT: f64 = 0.02;

fn values(extra: &[(&str, f64)]) -> BTreeMap<String, ParameterValue> {
    [
        (names::STEPOVER, STEPOVER),
        (names::FEED, 600.0),
        (names::PLUNGE_FEED, 150.0),
        (names::TOLERANCE, TOLERANCE),
    ]
    .into_iter()
    .chain(extra.iter().copied())
    .map(|(k, x)| (k.to_owned(), ParameterValue::Number(x)))
    .collect()
}

/// The section of a round shaft of radius `r` with a slot along X cut into
/// its top (+Z), `width` wide with its flat bottom at `bottom` from the
/// axis, counter-clockwise seen from +X.
fn keyway_section(r: f64, width: f64, bottom: f64) -> Vec<(f64, f64)> {
    let half = 0.5 * width;
    let start = (half / r).asin();
    let n = 360;
    let mut out: Vec<(f64, f64)> = (0..=n)
        .map(|k| {
            let t = FRAC_PI_2 + start + (TAU - 2.0 * start) * f64::from(k) / f64::from(n);
            (r * t.cos(), r * t.sin())
        })
        .collect();
    out.push((half, bottom));
    out.push((-half, bottom));
    out
}

/// A circle of `n` points about `(cy, cz)`, counter-clockwise.
fn circle(cy: f64, cz: f64, r: f64, n: u32) -> Vec<(f64, f64)> {
    (0..n)
        .map(|k| {
            let t = TAU * f64::from(k) / f64::from(n);
            (cy + r * t.cos(), cz + r * t.sin())
        })
        .collect()
}

/// The tool at the end of every cutting move and at points between, on the
/// straight machine move (as `simultaneous_off_axis.rs` takes them).
fn poses_along(path: &GeneratedPath, rotary: &RotaryMapping, k: u32) -> Vec<Pose> {
    let mut out = Vec::new();
    let mut from = path.group.start;
    for s in &path.group.segments {
        if s.kind == MoveKind::Cutting {
            let a = rotary.to_axis(from.tip, from.a);
            let b = rotary.to_axis(s.end.tip, s.end.a);
            let k = k.max((2.0 * (s.end.a.0 - from.a.0).abs()).ceil() as u32);
            for i in 1..=k {
                let t = f64::from(i) / f64::from(k);
                let lerp = |p: f64, q: f64| p + t * (q - p);
                let angle = Degrees(lerp(from.a.0, s.end.a.0));
                let tip = rotary.to_model(
                    AxisPosition {
                        along: lerp(a.along, b.along),
                        across: lerp(a.across, b.across),
                        height: lerp(a.height, b.height),
                    },
                    angle,
                );
                out.push(Pose {
                    tip,
                    axis: rotary.tool_axis(angle).get(),
                });
            }
        }
        from = s.end;
    }
    out
}

/// [`poses_along`] the cutting moves of the passes on hidden surfaces: each
/// stays on one section of the part across X, the stepover apart from
/// `x0` to `x1` with the first half a step in (decision record 0368); the
/// main passes advance along X.
fn poses_across(
    path: &GeneratedPath,
    rotary: &RotaryMapping,
    k: u32,
    (x0, x1, stepover): (f64, f64, f64),
) -> Vec<Pose> {
    let n = ((x1 - x0) / stepover).ceil();
    let dx = (x1 - x0) / n;
    let on_section = |x: f64| {
        let i = ((x - x0) / dx - 0.5).round();
        (x - (x0 + (i + 0.5) * dx)).abs() < 1e-7
    };
    let mut kept = path.clone();
    let mut from = path.group.start;
    let mut segments = Vec::new();
    for s in &path.group.segments {
        let across =
            s.kind == MoveKind::Cutting && on_section(s.end.tip.x) && on_section(from.tip.x);
        let mut s = *s;
        if !across {
            s.kind = MoveKind::Linking;
        }
        segments.push(s);
        from = s.end;
    }
    kept.group.segments = segments;
    poses_along(&kept, rotary, k)
}

/// For each point, its distance to the nearest tool surface over the poses:
/// positive where material is left, negative where the tool cut into it.
/// Capped at `cap`.
fn left_on(outline: &Outline, poses: &[Pose], points: &[Point3], cap: f64) -> Vec<f64> {
    let mut sorted: Vec<&Pose> = poses.iter().collect();
    sorted.sort_by(|a, b| a.tip.x.total_cmp(&b.tip.x));
    let reach = outline.radius() + cap;
    points
        .iter()
        .map(|p| {
            let start = sorted.partition_point(|q| q.tip.x < p.x - reach);
            let mut best = cap;
            for q in sorted[start..]
                .iter()
                .take_while(|q| q.tip.x <= p.x + reach)
            {
                let d = *p - q.tip;
                let z = d.dot(q.axis);
                let radial = (d - q.axis * z).length();
                if radial - outline.radius() > best {
                    continue;
                }
                best = best.min(outline.clearance(radial, z));
            }
            best
        })
        .collect()
}

/// The point of `points` cut deepest, for the messages.
fn deepest_at(points: &[Point3], left: &[f64]) -> Option<(Point3, f64)> {
    points
        .iter()
        .zip(left)
        .min_by(|a, b| a.1.total_cmp(b.1))
        .map(|(p, l)| (*p, *l))
}

/// The most left and the deepest cut over `left`.
fn extremes(left: &[f64]) -> (f64, f64) {
    left.iter()
        .fold((f64::NEG_INFINITY, f64::INFINITY), |(m, d), l| {
            (m.max(*l), d.min(*l))
        })
}

/// Points `step` apart on the straight segment from `a` to `b` of a
/// section, at X from `x0` to `x1`.
fn on_segment(a: (f64, f64), b: (f64, f64), x: (f64, f64), step: f64) -> Vec<Point3> {
    let len = (b.0 - a.0).hypot(b.1 - a.1);
    let nu = (len / step).ceil().max(1.0) as u32;
    let nx = ((x.1 - x.0) / step).ceil().max(1.0) as u32;
    let mut out = Vec::new();
    for i in 0..=nx {
        let px = x.0 + (x.1 - x.0) * f64::from(i) / f64::from(nx);
        for j in 0..=nu {
            let t = f64::from(j) / f64::from(nu);
            out.push(Point3::new(
                px,
                a.0 + t * (b.0 - a.0),
                a.1 + t * (b.1 - a.1),
            ));
        }
    }
    out
}

/// Points on every edge of a closed section polygon.
fn on_polygon(poly: &[(f64, f64)], x: (f64, f64), step: f64) -> Vec<Point3> {
    poly.iter()
        .zip(poly.iter().cycle().skip(1))
        .flat_map(|(a, b)| on_segment(*a, *b, x, step))
        .collect()
}

fn generate_on(f: &Fixture, input: &StrategyInput<'_>, extra: &[(&str, f64)]) -> GeneratedPath {
    let v = values(extra);
    let start = std::time::Instant::now();
    let path = run(|ctx| generate(&SimultaneousFinishing, input, &v, ctx)).unwrap();
    eprintln!(
        "generated {} moves in {:.2} s; notes {:?}; unreached {:?}",
        path.group.segments.len(),
        start.elapsed().as_secs_f64(),
        path.notes.iter().map(|n| &n.message).collect::<Vec<_>>(),
        path.unreached
    );
    let _ = f;
    path
}

/// The 20 mm keyway shaft: radius 10, a slot `width` wide and 3 deep.
fn keyway(width: f64) -> (TriangleMesh, Vec<(f64, f64)>) {
    let section = keyway_section(10.0, width, 7.0);
    (prism(&section, 0.0, 20.0, 40), section)
}

#[test]
fn req_op_20_req_op_12_the_walls_of_a_keyway_are_finished_and_nothing_is_cut_in() {
    // A ray from outside towards the axis never meets the walls of a slot
    // along X: they are parallel to its centre line. Dropped from the slot's
    // bottom, a ball bridges onto the wall's top edge and leaves the wall
    // (a 4 mm slot, nearly the tool's width, would be cleaned by the drop
    // alone). The 3.175 mm ball fits the 6 mm slot, so the walls are
    // finished with the tool's side, its axis leaning as far towards their
    // normal as the slot's far edge allows.
    let (mesh, section) = keyway(6.0);
    let f = Fixture::new(MeshSurface::new(&mesh).unwrap(), 14.0, &ball(DIAMETER));
    let path = generate_on(&f, &f.input(), &[]);
    let outline = Outline::new(&f.tool);
    let poses = poses_along(&path, &f.rotary, 4);
    let x = (3.0, 17.0);
    // The walls above the strip a ball cannot reach in the sharp corner at
    // the slot's bottom (its radius), and below their top edge.
    let r = 0.5 * DIAMETER;
    let mut walls = on_segment((3.0, 7.0 + r + 0.1), (3.0, 9.4), x, 0.2);
    walls.extend(on_segment((-3.0, 7.0 + r + 0.1), (-3.0, 9.4), x, 0.2));
    let left = left_on(&outline, &poses, &walls, 1.0);
    let (most, deepest) = extremes(&left);
    eprintln!(
        "keyway walls: most left {most:.4} mm, deepest {deepest:.4} mm at {:?}",
        deepest_at(&walls, &left)
    );
    assert!(most <= LEFT, "{most}");
    assert!(deepest >= -TOLERANCE, "{deepest}");
    // Nothing anywhere on the part is cut into beyond the tolerance.
    let all = on_polygon(&section, x, 0.5);
    let left = left_on(&outline, &poses, &all, 1.0);
    let (_, deepest) = extremes(&left);
    if deepest < -TOLERANCE
        && let Some((p, _)) = deepest_at(&all, &left)
    {
        for (i, q, l) in cutting(&outline, &path, &f.rotary, p, TOLERANCE) {
            let seg = &path.group.segments;
            eprintln!(
                "move {i} cuts {l:.4}: {q:?}; from {:?} to {:?}; before {:?}",
                seg.get(i.wrapping_sub(1)).map(|s| (s.end.tip, s.end.a)),
                (seg[i].end.tip, seg[i].end.a),
                seg.get(i.wrapping_sub(2)).map(|s| (s.end.tip, s.end.a))
            );
        }
    }
    assert!(
        deepest >= -TOLERANCE,
        "{deepest} at {:?}",
        deepest_at(&all, &left)
    );
    // The corner strips beside the walls the tool finished are rest
    // machining's, not unreachable surfaces: nothing is reported.
    assert!(path.unreached.is_empty(), "{:?}", path.unreached);
    assert!(path.warnings.is_empty(), "{:?}", path.warnings);
    assert!(
        path.notes
            .iter()
            .any(|n| n.message.contains("hidden from the drive")),
        "{:?}",
        path.notes
    );
}

#[test]
fn req_op_20_a_slot_narrower_than_the_tool_is_reported_and_not_cut_into() {
    let (mesh, section) = keyway(2.5);
    let f = Fixture::new(MeshSurface::new(&mesh).unwrap(), 14.0, &ball(DIAMETER));
    let path = generate_on(&f, &f.input(), &[]);
    let narrow: Vec<_> = path
        .unreached
        .iter()
        .filter(|u| u.reason == UnreachedReason::TooNarrow)
        .collect();
    assert!(!narrow.is_empty(), "{:?}", path.unreached);
    // The walls are 3 mm deep and 20 mm long; the corner rule keeps them.
    let area: f64 = narrow.iter().map(|u| u.area).sum();
    assert!(area > 60.0 && area < 150.0, "{area}");
    for u in &narrow {
        assert!(u.x.0 >= -0.5 && u.x.1 <= 20.5, "{u:?}");
        // The slot is on top (+Z): its walls face the tool at A near 0
        // (the machine's sense), whatever the sense, within a quarter turn.
        let mid = 0.5 * (u.a.0 + u.a.1);
        let mid = mid - 360.0 * (mid / 360.0).round();
        assert!(mid.abs() < 90.0, "{u:?}");
    }
    let warning = path
        .warnings
        .iter()
        .find(|w| w.message.contains("cannot reach"))
        .unwrap();
    assert!(
        warning.message.contains("too narrow"),
        "{}",
        warning.message
    );
    assert!(
        warning
            .message
            .contains("check them in the simulation before running the program"),
        "{}",
        warning.message
    );
    let outline = Outline::new(&f.tool);
    let poses = poses_along(&path, &f.rotary, 4);
    let (_, deepest) = extremes(&left_on(
        &outline,
        &poses,
        &on_polygon(&section, (3.0, 17.0), 0.5),
        1.0,
    ));
    assert!(deepest >= -TOLERANCE, "{deepest}");
}

#[test]
fn req_op_20_under_the_lips_of_a_dovetail_groove_is_blocked_and_reported() {
    // A groove round the axis, wider at its bottom (X 10 to 20, radius 6)
    // than at its opening (X 11 to 19): the bottom under each lip, and the
    // lips' undersides, can only be reached by a tool leaning along the
    // rotary axis.
    let profile = [
        (0.0, 10.0),
        (11.0, 10.0),
        (10.0, 6.0),
        (20.0, 6.0),
        (19.0, 10.0),
        (30.0, 10.0),
    ];
    let mesh = revolve(&profile, 360);
    let f = Fixture::new(MeshSurface::new(&mesh).unwrap(), 14.0, &ball(DIAMETER));
    let path = generate_on(&f, &f.input(), &[]);
    let blocked: Vec<_> = path
        .unreached
        .iter()
        .filter(|u| u.reason == UnreachedReason::Blocked)
        .collect();
    for under in [(10.2, 10.8), (19.2, 19.8)] {
        assert!(
            blocked
                .iter()
                .any(|u| u.x.0 <= under.1 && u.x.1 >= under.0 && u.a.1 - u.a.0 >= 359.0),
            "nothing blocked all round over X {under:?}: {:?}",
            path.unreached
        );
    }
    assert!(
        path.warnings.iter().any(|w| w.message.contains("blocked")),
        "{:?}",
        path.warnings
    );
    // Nothing is cut into the groove, its lips or the shaft.
    let outline = Outline::new(&f.tool);
    let poses = poses_along(&path, &f.rotary, 4);
    let mut points = Vec::new();
    for pair in profile.windows(2) {
        let ((xa, ra), (xb, rb)) = (pair[0], pair[1]);
        let n = (((xb - xa).hypot(rb - ra)) / 0.25).ceil() as u32;
        for i in 0..=n {
            let t = f64::from(i) / f64::from(n);
            let (px, pr) = (xa + t * (xb - xa), ra + t * (rb - ra));
            if !(1.0..=29.0).contains(&px) {
                continue;
            }
            for k in 0..120 {
                let a = TAU * f64::from(k) / 120.0;
                points.push(Point3::new(px, pr * a.cos(), pr * a.sin()));
            }
        }
    }
    let (_, deepest) = extremes(&left_on(&outline, &poses, &points, 1.0));
    assert!(deepest >= -TOLERANCE, "{deepest}");
}

/// Two round pins of radius `r` side by side at the same X, centred at
/// Y -/+ `apart`/2 and Z 6, over X 0 to 15.
fn twin_pins(r: f64, apart: f64) -> TriangleMesh {
    join(&[
        prism(&circle(-0.5 * apart, 6.0, r, 360), 0.0, 15.0, 30),
        prism(&circle(0.5 * apart, 6.0, r, 360), 0.0, 15.0, 30),
    ])
}

/// Points on a round pin, away from its ends.
fn pin_points(cy: f64, cz: f64, r: f64, x: (f64, f64)) -> Vec<Point3> {
    let mut out = Vec::new();
    let nx = ((x.1 - x.0) / 0.25).ceil() as u32;
    for i in 0..=nx {
        let px = x.0 + (x.1 - x.0) * f64::from(i) / f64::from(nx);
        for (y, z) in circle(cy, cz, r, 180) {
            out.push(Point3::new(px, y, z));
        }
    }
    out
}

#[test]
fn req_op_20_two_pins_side_by_side_are_cut_all_round() {
    // OI-1431: the section is two separate pieces; the centre of their box
    // lies between them, so the sides facing each other are hidden from it.
    // The gap (4 mm) is wider than the tool.
    let f = Fixture::new(
        MeshSurface::new(&twin_pins(4.0, 12.0)).unwrap(),
        16.0,
        &ball(DIAMETER),
    );
    let path = generate_on(&f, &f.input(), &[]);
    let outline = Outline::new(&f.tool);
    let poses = poses_along(&path, &f.rotary, 4);
    let x = (0.5 * DIAMETER + 1.0, 15.0 - 0.5 * DIAMETER - 1.0);
    for cy in [-6.0, 6.0] {
        let points = pin_points(cy, 6.0, 4.0, x);
        let left = left_on(&outline, &poses, &points, 1.0);
        let (most, deepest) = extremes(&left);
        let worst = points
            .iter()
            .zip(&left)
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map(|(p, l)| (*p, *l));
        eprintln!(
            "pin at Y {cy}: most left {most:.4} mm at {worst:?}, deepest {deepest:.4} mm at {:?}",
            deepest_at(&points, &left)
        );
        assert!(most <= LEFT, "pin at Y {cy}: {most}");
        assert!(deepest >= -TOLERANCE, "pin at Y {cy}: {deepest}");
    }
    assert!(path.unreached.is_empty(), "{:?}", path.unreached);
}

#[test]
fn req_op_20_a_gap_narrower_than_the_tool_between_two_pins_is_reported() {
    // Pins of radius 5, 2 mm apart: the strip where they face each other is
    // too narrow for the 3.175 mm ball.
    let f = Fixture::new(
        MeshSurface::new(&twin_pins(5.0, 12.0)).unwrap(),
        18.0,
        &ball(DIAMETER),
    );
    let path = generate_on(&f, &f.input(), &[]);
    assert!(
        path.unreached
            .iter()
            .any(|u| u.reason == UnreachedReason::TooNarrow && u.x.1 - u.x.0 > 8.0),
        "{:?}",
        path.unreached
    );
    let outline = Outline::new(&f.tool);
    let poses = poses_along(&path, &f.rotary, 4);
    for cy in [-6.0, 6.0] {
        let (_, deepest) = extremes(&left_on(
            &outline,
            &poses,
            &pin_points(cy, 6.0, 5.0, (2.0, 13.0)),
            1.0,
        ));
        assert!(deepest >= -TOLERANCE, "pin at Y {cy}: {deepest}");
    }
}

/// A flat blade along X, `thick` thick, from `inner` to `outer` from the
/// axis at polar angle `angle` degrees, counter-clockwise seen from +X.
fn blade(angle: f64, inner: f64, outer: f64, thick: f64) -> Vec<(f64, f64)> {
    let (s, c) = angle.to_radians().sin_cos();
    let at = |u: f64, v: f64| (u * c - v * s, u * s + v * c);
    let h = 0.5 * thick;
    vec![at(inner, -h), at(outer, -h), at(outer, h), at(inner, h)]
}

#[test]
fn req_op_20_blades_without_a_hub_are_finished_on_both_sides() {
    // Three separate blades at one X, radial: a ray towards the axis meets
    // only their outer edges, never their sides.
    let blades: Vec<Vec<(f64, f64)>> = [90.0, 210.0, 330.0]
        .iter()
        .map(|a| blade(*a, 3.0, 12.0, 2.0))
        .collect();
    let mesh = join(
        &blades
            .iter()
            .map(|b| prism(b, 0.0, 15.0, 30))
            .collect::<Vec<_>>(),
    );
    let f = Fixture::new(MeshSurface::new(&mesh).unwrap(), 16.0, &ball(DIAMETER));
    let path = generate_on(&f, &f.input(), &[]);
    let outline = Outline::new(&f.tool);
    let poses = poses_along(&path, &f.rotary, 4);
    let x = (3.0, 12.0);
    // Where the rays aimed at the section's centre (OP-26) meet a side
    // squarely enough, the main spiral finishes it, with its own scallop
    // there (up to 0.024 mm measured); the rest is finished by the passes
    // on hidden surfaces.
    const LEFT: f64 = 0.03;
    for (k, b) in blades.iter().enumerate() {
        // The two sides, between 5 and 11 mm from the axis.
        let side = |p: (f64, f64), q: (f64, f64)| {
            let lerp = |t: f64| (p.0 + t * (q.0 - p.0), p.1 + t * (q.1 - p.1));
            on_segment(lerp(2.0 / 9.0), lerp(8.0 / 9.0), x, 0.25)
        };
        let mut points = side(b[0], b[1]);
        points.extend(side(b[3], b[2]));
        let left = left_on(&outline, &poses, &points, 1.0);
        let (most, deepest) = extremes(&left);
        let worst = points
            .iter()
            .zip(&left)
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map(|(p, l)| (*p, *l));
        eprintln!(
            "blade {k}: most left {most:.4} mm at {worst:?}, deepest {deepest:.4} mm at {:?}; \
             {} points left over {LEFT}",
            deepest_at(&points, &left),
            left.iter().filter(|l| **l > LEFT).count()
        );
        assert!(most <= LEFT, "blade {k}: {most}");
        assert!(deepest >= -TOLERANCE, "blade {k}: {deepest}");
        let (_, deepest) = extremes(&left_on(&outline, &poses, &on_polygon(b, x, 0.25), 1.0));
        assert!(deepest >= -TOLERANCE, "blade {k}: {deepest}");
    }
}

#[test]
fn req_op_20_a_wall_needing_more_lean_than_the_limit_is_reported_with_its_parameter() {
    // The keyway's walls need the tool to lean far from their normal (the
    // side of the tool); a limit of 30 degrees leaves them, and says which
    // parameter to change.
    let (mesh, _) = keyway(6.0);
    let f = Fixture::new(MeshSurface::new(&mesh).unwrap(), 14.0, &ball(DIAMETER));
    let path = generate_on(&f, &f.input(), &[(parameter::MAX_HIDDEN_LEAN, 30.0)]);
    let lean: Vec<_> = path
        .unreached
        .iter()
        .filter_map(|u| match u.reason {
            UnreachedReason::NeedsLean { degrees } => Some((u, degrees)),
            _ => None,
        })
        .collect();
    assert!(!lean.is_empty(), "{:?}", path.unreached);
    for (u, degrees) in &lean {
        assert!(*degrees > 30.0 && *degrees <= 90.0 + 1e-9, "{u:?}");
    }
    let warning = path
        .warnings
        .iter()
        .find(|w| w.message.contains("cannot reach"))
        .unwrap();
    assert_eq!(
        warning.parameter.as_deref(),
        Some(parameter::MAX_HIDDEN_LEAN)
    );
    assert!(
        warning.message.contains("more than the largest lean"),
        "{}",
        warning.message
    );
    // The walls stay uncut.
    let outline = Outline::new(&f.tool);
    let poses = poses_along(&path, &f.rotary, 4);
    let wall = on_segment((3.0, 8.0), (3.0, 8.4), (5.0, 15.0), 0.5);
    let (most, _) = extremes(&left_on(&outline, &poses, &wall, 1.0));
    assert!(most > 0.1, "{most}");
}

/// A 1 mm ball whose flutes are 1.5 mm long, on a 3.175 mm shank.
fn small_ball() -> (openchuck_types::tool::Tool, RevolvedProfile) {
    let small = openchuck_types::tool::Tool {
        flute_length: Millimetres(1.5),
        shank_diameter: Millimetres(DIAMETER),
        ..ball(1.0)
    };
    let body = ToolProfile {
        segments: vec![
            ProfileSegment::Arc {
                to: ProfilePoint::new(0.5, 0.5),
                centre: ProfilePoint::new(0.0, 0.5),
            },
            ProfileSegment::Line {
                to: ProfilePoint::new(0.5, 1.5),
            },
            ProfileSegment::Line {
                to: ProfilePoint::new(0.5 * DIAMETER, 1.5),
            },
        ],
    }
    .validate()
    .unwrap();
    (small, body)
}

#[test]
fn req_op_20_req_op_12_a_shank_wider_than_the_cutter_never_meets_the_part() {
    // A 3 mm slot: the 1 mm ball reaches into it, its 3.175 mm shank does
    // not, so the walls deeper than the flutes are left and reported, and on
    // every pass, the main ones included, the shank never meets the part:
    // a pass breaks where it would, and the breaks are said (decision record
    // 0368, the owner's answer on OI-1452).
    let (mesh, section) = keyway(3.0);
    let (small, body) = small_ball();
    let f = Fixture::new(MeshSurface::new(&mesh).unwrap(), 14.0, &small);
    let input = StrategyInput {
        shank: Some(Shank {
            radius: 0.5 * DIAMETER,
            height: 1.5,
        }),
        ..f.input()
    };
    let path = generate_on(&f, &input, &[(names::STEPOVER, 0.15)]);
    let poses = poses_along(&path, &f.rotary, 4);
    let hidden = poses_across(&path, &f.rotary, 4, (0.0, 20.0, 0.15));
    assert!(hidden.len() > 1000, "{}", hidden.len());
    let all = on_polygon(&section, (3.0, 17.0), 0.5);
    let left = left_on(&Outline::new(&body), &poses, &all, 1.0);
    let (_, deepest) = extremes(&left);
    eprintln!(
        "shank: deepest {deepest:.4} mm at {:?}",
        deepest_at(&all, &left)
    );
    if deepest < -TOLERANCE
        && let Some((p, _)) = deepest_at(&all, &left)
    {
        for (i, q, l) in cutting(&Outline::new(&body), &path, &f.rotary, p, TOLERANCE)
            .into_iter()
            .take(6)
        {
            let seg = &path.group.segments;
            eprintln!(
                "move {i} cuts {l:.4}: {q:?}; from {:?} to {:?}",
                seg.get(i.wrapping_sub(1))
                    .map(|s| (s.kind, s.end.tip, s.end.a)),
                (seg[i].kind, seg[i].end.tip, seg[i].end.a),
            );
        }
    }
    assert!(deepest >= -TOLERANCE, "{deepest}");
    assert!(
        path.unreached
            .iter()
            .any(|u| u.reason == UnreachedReason::TooNarrow),
        "{:?}",
        path.unreached
    );
    // The top of each wall, within the flutes' reach, is finished.
    let wall = on_segment((1.5, 9.0), (1.5, 9.3), (5.0, 15.0), 0.2);
    let (most, _) = extremes(&left_on(&Outline::new(&f.tool), &poses, &wall, 1.0));
    assert!(most <= LEFT, "{most}");
    // The main passes break where the shank would meet the slot's edges,
    // and the warning says where.
    assert!(!path.shank_breaks.is_empty());
    for b in &path.shank_breaks {
        assert!(b.x >= -0.5 && b.x <= 20.5, "{b:?}");
    }
    let warning = path
        .warnings
        .iter()
        .find(|w| w.message.contains("shank"))
        .unwrap();
    assert!(
        warning
            .message
            .contains("check them in the simulation before running the program"),
        "{}",
        warning.message
    );
}

#[test]
fn req_op_12_req_op_20_a_wider_shank_that_never_touches_changes_nothing() {
    // A plain cylinder with the 1 mm ball on its 3.175 mm shank: the shank
    // never meets the part, so the path is the one without a shank, with no
    // break and no warning.
    let mesh = revolve(&[(0.0, 10.0), (20.0, 10.0)], 360);
    let (small, _) = small_ball();
    let f = Fixture::new(MeshSurface::new(&mesh).unwrap(), 14.0, &small);
    let with = StrategyInput {
        shank: Some(Shank {
            radius: 0.5 * DIAMETER,
            height: 1.5,
        }),
        ..f.input()
    };
    let v = values(&[(names::STEPOVER, 1.0)]);
    let a = run(|ctx| generate(&SimultaneousFinishing, &with, &v, ctx)).unwrap();
    let b = run(|ctx| generate(&SimultaneousFinishing, &f.input(), &v, ctx)).unwrap();
    assert!(a.shank_breaks.is_empty());
    assert!(a.warnings.is_empty(), "{:?}", a.warnings);
    assert_eq!(a.group, b.group);
}

#[test]
fn req_op_20_req_op_06_on_a_hidden_wall_the_tool_axis_is_the_reachable_one_nearest_the_normal() {
    // With no smoothing, the tool on a keyway's wall leans from the slot's
    // direction towards the wall's normal as far as the slot's far edge
    // allows: turning it 3 degrees further towards the normal, about the
    // ball's centre, makes the tool meet the part.
    let (mesh, section) = keyway(6.0);
    let f = Fixture::new(MeshSurface::new(&mesh).unwrap(), 14.0, &ball(DIAMETER));
    let path = generate_on(&f, &f.input(), &[(parameter::MAX_TILT, 0.0)]);
    let outline = Outline::new(&f.tool);
    let r = 0.5 * DIAMETER;
    let mut checked = 0;
    // The passes on hidden walls run across X at one X (each a section of
    // the part, decision record 0368); the main passes advance along X.
    let segments = &path.group.segments;
    for s in segments
        .iter()
        .zip(segments.iter().skip(1))
        .filter_map(|(p, s)| {
            (s.kind == MoveKind::Cutting
                && p.kind == MoveKind::Cutting
                && (s.end.tip.x - p.end.tip.x).abs() < 1e-9
                && s.end.tip.x > 4.0
                && s.end.tip.x < 16.0)
                .then_some(s)
        })
    {
        let a = s.end.axis.get();
        let centre = s.end.tip + a * r;
        // On a wall: the ball's centre a radius from it, inside the slot.
        let wall = if (centre.y - (3.0 - r)).abs() < 0.02 {
            1.0
        } else if (centre.y + (3.0 - r)).abs() < 0.02 {
            -1.0
        } else {
            continue;
        };
        if centre.z < 7.0 + r + 0.2 || centre.z > 9.0 {
            continue;
        }
        // The wall's normal points into the slot, at -wall along Y.
        let n_angle = if wall > 0.0 { 180.0_f64 } else { 0.0 };
        let a_angle = a.z.atan2(a.y).to_degrees();
        let towards = {
            let d = n_angle - a_angle;
            (d - 360.0 * (d / 360.0).round()).signum()
        };
        let turned = (a_angle + 3.0 * towards).to_radians();
        let axis = Vec3::new(0.0, turned.cos(), turned.sin());
        let pose = Pose {
            tip: centre + (-(axis * r)),
            axis,
        };
        let near: Vec<Point3> = on_polygon(&section, (centre.x - 2.0, centre.x + 2.0), 0.1)
            .into_iter()
            .filter(|p| p.z > 6.0 && p.y.abs() < 6.0)
            .collect();
        let (_, deepest) = extremes(&left_on(&outline, &[pose], &near, 1.0));
        assert!(
            deepest < -TOLERANCE,
            "turned 3 degrees nearer the normal at {centre:?} (axis at {a_angle:.2} degrees), the \
             tool still clears the part ({deepest})"
        );
        checked += 1;
    }
    eprintln!("nearest reachable axis checked at {checked} poses");
    assert!(checked > 50, "{checked}");
}

#[test]
fn req_op_20_req_nfr_06_hidden_surfaces_are_identical_at_1_2_and_n_threads() {
    let (mesh, _) = keyway(6.0);
    let f = Fixture::new(
        MeshSurface::new(&mesh).unwrap(),
        14.0,
        &tool(ToolShape::BallEnd, DIAMETER),
    );
    let v = values(&[(names::STEPOVER, 1.0)]);
    let runs = openchuck_testkit::determinism::check(|| {
        let path = run(|ctx| generate(&SimultaneousFinishing, &f.input(), &v, ctx)).unwrap();
        format!(
            "{:?} {:?} {:?} {:?}",
            path.group, path.notes, path.warnings, path.unreached
        )
        .into_bytes()
    })
    .unwrap();
    assert_eq!(runs.len(), 4);
}

#[test]
fn req_op_20_a_part_with_nothing_hidden_gets_no_hidden_passes_or_reports() {
    // A plain cylinder: every point is met by a ray from outside, so the
    // path, notes and warnings are those of the drive alone.
    let mesh = revolve(&[(0.0, 10.0), (20.0, 10.0)], 360);
    let f = Fixture::new(MeshSurface::new(&mesh).unwrap(), 14.0, &ball(DIAMETER));
    let path = generate_on(&f, &f.input(), &[(names::STEPOVER, 1.0)]);
    assert!(path.unreached.is_empty());
    assert!(path.warnings.is_empty());
    assert!(
        !path
            .notes
            .iter()
            .any(|n| n.message.contains("hidden from the drive")),
        "{:?}",
        path.notes
    );
}

/// The poses that cut `p` deeper than `depth`, with the index of the cutting
/// move each lies on, for the messages.
fn cutting(
    outline: &Outline,
    path: &GeneratedPath,
    rotary: &RotaryMapping,
    p: Point3,
    depth: f64,
) -> Vec<(usize, Pose, f64)> {
    let mut out = Vec::new();
    let mut from = path.group.start;
    for (i, s) in path.group.segments.iter().enumerate() {
        if s.kind == MoveKind::Cutting {
            let one = GeneratedPath {
                group: openchuck_types::ir::ToolpathGroup {
                    segments: vec![*s],
                    start: from,
                    ..path.group.clone()
                },
                ..path.clone()
            };
            for q in poses_along(&one, rotary, 16) {
                let l = left_on(outline, &[q], &[p], 1.0)[0];
                if l < -depth {
                    out.push((i, q, l));
                }
            }
        }
        from = s.end;
    }
    out
}

#[test]
fn req_op_20_req_op_12_the_crankshaft_is_finished_where_reached_and_nothing_is_cut_in() {
    // The owner's crankshaft (`stl/crankshaft.stl`): its pins are driven
    // round their own centres (OP-26); what the drive's rays do not meet
    // between the webs and the pins is finished or reported, and nothing is
    // cut into the part beyond the tolerance at any of its vertices.
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/stl/crankshaft.stl");
    let mesh = read_binary_stl(&path);
    let f = Fixture::new(MeshSurface::new(&mesh).unwrap(), 18.0, &ball(DIAMETER));
    let generated = generate_on(&f, &f.input(), &[]);
    let outline = Outline::new(&f.tool);
    let poses = poses_along(&generated, &f.rotary, 4);
    let points: Vec<Point3> = mesh
        .vertices
        .iter()
        .copied()
        .filter(|p| p.x > 0.5 && p.x < 39.5)
        .collect();
    let left = left_on(&outline, &poses, &points, 1.0);
    let (_, deepest) = extremes(&left);
    eprintln!(
        "crankshaft: {} vertices, deepest {deepest:.4} mm at {:?}; {} areas unreached",
        points.len(),
        deepest_at(&points, &left),
        generated.unreached.len()
    );
    assert!(deepest >= -TOLERANCE, "{deepest}");
    for u in &generated.unreached {
        assert!(u.x.0 >= -0.5 && u.x.1 <= 40.5 && u.area > 0.0, "{u:?}");
    }
}

#[test]
fn req_op_20_req_op_02_only_areas_inside_the_operations_boundary_are_reported() {
    // The 2.5 mm slot, with the operation contained within X 0 to 10: the
    // walls beyond are not this operation's, and are not reported.
    let (mesh, _) = keyway(2.5);
    let f = Fixture::new(MeshSurface::new(&mesh).unwrap(), 14.0, &ball(DIAMETER));
    let region = openchuck_path::RegionConstraint {
        boundary: Some(openchuck_path::Window {
            x_min: 0.0,
            x_max: 10.0,
            arc: None,
        }),
        ..openchuck_path::RegionConstraint::default()
    };
    let v = values(&[(names::STEPOVER, 0.6)]);
    let path = run(|ctx| {
        openchuck_path::generate_in_region(&SimultaneousFinishing, &f.input(), &region, &v, ctx)
    })
    .unwrap();
    eprintln!("unreached in the boundary: {:?}", path.unreached);
    assert!(
        path.unreached
            .iter()
            .any(|u| u.reason == UnreachedReason::TooNarrow),
        "{:?}",
        path.unreached
    );
    for u in &path.unreached {
        assert!(u.x.1 <= 10.0 + 0.6, "{u:?}");
    }
}
