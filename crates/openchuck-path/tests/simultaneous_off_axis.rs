// SPDX-License-Identifier: AGPL-3.0-or-later
//! Simultaneous finishing of sections that do not surround the rotary axis
//! (OP-26, with OP-05, OP-06, OP-12, NFR-06; decision record 0363 item 3;
//! issue #411).
//!
//! A ray from outside towards the rotary axis meets a round pin centred off
//! the axis over only part of each turn, so the spiral broke into a pass per
//! turn and never met the pin's side facing the axis. The checks here are
//! independent of the strategy: points on the exact pin, measured against
//! the tool's outline (`openchuck-testkit`) at every fed pose of the path.
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
use std::path::PathBuf;

use openchuck_path::params::names;
use openchuck_path::rotary::AxisPosition;
use openchuck_path::simultaneous::parameter;
use openchuck_path::{
    GeneratedPath, MeshSurface, ParameterValue, RotaryMapping, SimultaneousFinishing, generate,
};
use openchuck_testkit::gouge::{Outline, Pose};
use openchuck_testkit::parts::{Cylinder, RadialPart};
use openchuck_types::geometry::{Point3, Vec3};
use openchuck_types::ir::MoveKind;
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::tool::ToolShape;
use openchuck_types::units::Degrees;
use support::{Fixture, ball, read_binary_stl, run, tool};

const TOLERANCE: f64 = 0.01;
/// The Z1 3.175 mm ball and its hardwood stepover (diameter / 7).
const DIAMETER: f64 = 3.175;
const STEPOVER: f64 = 0.453_571;

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

/// A round pin: its centre across the axis, radius and X range.
#[derive(Debug, Clone, Copy)]
struct Pin {
    centre: (f64, f64),
    radius: f64,
    x: (f64, f64),
}

impl Pin {
    /// Points on the pin, `step` mm apart along X and round, kept `margin`
    /// mm from its ends.
    fn points(&self, step: f64, margin: f64) -> Vec<Point3> {
        let (lo, hi) = (self.x.0 + margin, self.x.1 - margin);
        let nx = ((hi - lo) / step).ceil() as u32;
        let na = (std::f64::consts::TAU * self.radius / step).ceil() as u32;
        let mut out = Vec::new();
        for i in 0..=nx {
            let x = lo + (hi - lo) * f64::from(i) / f64::from(nx);
            for j in 0..na {
                let t = std::f64::consts::TAU * f64::from(j) / f64::from(na);
                out.push(Point3::new(
                    x,
                    self.centre.0 + self.radius * t.cos(),
                    self.centre.1 + self.radius * t.sin(),
                ));
            }
        }
        out
    }

    /// The pin's outward normal across X nearest `p`.
    fn normal(&self, p: Point3) -> Vec3 {
        let d = Vec3::new(0.0, p.y - self.centre.0, p.z - self.centre.1);
        d * (1.0 / d.length())
    }
}

/// The tool at the end of every cutting move and at `k - 1` points between,
/// or more where the move turns A by more than half a degree, on the
/// straight machine move.
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

/// For each point, its signed distance to the nearest tool surface over the
/// poses: positive where material is left, negative where the tool cut into
/// it. Capped at `cap`.
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

/// The cutting passes: runs of cutting moves between other moves, each as
/// the tips' X range.
fn passes(path: &GeneratedPath) -> Vec<(f64, f64)> {
    let mut out: Vec<(f64, f64)> = Vec::new();
    let mut open = false;
    for s in &path.group.segments {
        if s.kind == MoveKind::Cutting {
            let x = s.end.tip.x;
            match (open, out.last_mut()) {
                (true, Some(r)) => *r = (r.0.min(x), r.1.max(x)),
                _ => out.push((x, x)),
            }
            open = true;
        } else {
            open = false;
        }
    }
    out
}

/// The worst in-plane angle, in degrees, between the tool axis and the pin's
/// exact normal at the ball's contact, over the cuts in `x`.
fn axis_error(path: &GeneratedPath, pin: &Pin, x: (f64, f64), rho: f64) -> (f64, usize) {
    let (mut worst, mut count) = (0.0_f64, 0);
    for s in path
        .group
        .segments
        .iter()
        .filter(|s| s.kind == MoveKind::Cutting && s.end.tip.x > x.0 && s.end.tip.x < x.1)
    {
        let a = s.end.axis.get();
        let n = pin.normal(s.end.tip + a * rho);
        let d = (a.z.atan2(a.y) - n.z.atan2(n.y)).to_degrees();
        let d = d - 360.0 * (d / 360.0).round();
        worst = worst.max(d.abs());
        count += 1;
    }
    (worst, count)
}

/// A cylinder of radius 5 over X 0 to 20, centred 8 mm off the axis.
fn off_axis_cylinder() -> TriangleMesh {
    let mut mesh = Cylinder {
        radius: 5.0,
        length: 20.0,
    }
    .mesh(81, 720);
    for v in &mut mesh.vertices {
        *v = Point3::new(v.x, v.y, v.z + 8.0);
    }
    mesh
}

fn crankshaft() -> TriangleMesh {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/stl/crankshaft.stl");
    read_binary_stl(&path)
}

/// The crankshaft's three pins (`stl/crankshaft.stl`, the owner's part as
/// placed in its project).
const CRANK_PINS: [Pin; 3] = [
    Pin {
        centre: (0.0, 10.0),
        radius: 7.5,
        x: (0.0, 10.0),
    },
    Pin {
        centre: (0.0, -10.0),
        radius: 7.5,
        x: (15.0, 25.0),
    },
    Pin {
        centre: (0.0, 10.0),
        radius: 7.5,
        x: (30.0, 40.0),
    },
];

#[test]
fn req_op_26_req_op_06_an_off_axis_cylinder_is_driven_round_its_own_centre() {
    // An analytic answer: on the normal, the tool's line passes through the
    // cylinder's own centre line, so it is 8 sin(angle) across the rotary
    // axis; the whole round is cut in one pass; nothing is cut into it.
    let f = Fixture::new(
        MeshSurface::new(&off_axis_cylinder()).unwrap(),
        14.0,
        &ball(DIAMETER),
    );
    let path = run(|ctx| {
        generate(
            &SimultaneousFinishing,
            &f.input(),
            &values(&[(parameter::MAX_TILT, 0.0)]),
            ctx,
        )
    })
    .unwrap();
    let pin = Pin {
        centre: (0.0, 8.0),
        radius: 5.0,
        x: (0.0, 20.0),
    };
    // The tool's line to the cylinder's centre line, across X.
    let mut off = 0.0_f64;
    for s in path
        .group
        .segments
        .iter()
        .filter(|s| s.kind == MoveKind::Cutting && s.end.tip.x > 3.0 && s.end.tip.x < 17.0)
    {
        let a = s.end.axis.get();
        let d = Vec3::new(0.0, pin.centre.0 - s.end.tip.y, pin.centre.1 - s.end.tip.z);
        off = off.max((d - a * d.dot(a)).length());
    }
    let (worst, count) = axis_error(&path, &pin, (3.0, 17.0), 0.5 * DIAMETER);
    eprintln!(
        "off-axis cylinder: the tool's line passes within {off:.4} mm of its centre; tool axis \
         within {worst:.4} degrees of the normal over {count} cuts; passes {:?}",
        passes(&path)
    );
    assert!(count > 1000, "{count}");
    assert!(off < 0.02, "{off}");
    assert!(worst < 0.1, "{worst}");
    // One pass covers the middle of the cylinder.
    let middle: Vec<_> = passes(&path)
        .into_iter()
        .filter(|(a, b)| *b > 3.0 && *a < 17.0)
        .collect();
    assert_eq!(middle.len(), 1, "{middle:?}");
    let outline = Outline::new(&f.tool);
    let left = left_on(
        &outline,
        &poses_along(&path, &f.rotary, 4),
        &pin.points(0.25, 0.5 * DIAMETER + 0.5),
        1.0,
    );
    let (most, deepest) = left
        .iter()
        .fold((f64::NEG_INFINITY, f64::INFINITY), |(m, d), l| {
            (m.max(*l), d.min(*l))
        });
    eprintln!("off-axis cylinder: most left {most:.4} mm, deepest {deepest:.4} mm");
    // The ball's scallop at this stepover on a radius of 5 is about
    // 0.02 mm; the side facing the axis is cut like the rest.
    assert!(most <= 0.03 + TOLERANCE, "{most}");
    assert!(deepest >= -TOLERANCE, "{deepest}");
    assert!(
        path.notes
            .iter()
            .any(|n| n.message.contains("does not surround the rotary axis")),
        "{:?}",
        path.notes
    );
}

#[test]
fn req_op_26_req_op_12_the_crankshaft_pins_are_cut_all_round_in_one_pass_each() {
    let f = Fixture::new(
        MeshSurface::new(&crankshaft()).unwrap(),
        18.0,
        &ball(DIAMETER),
    );
    // A stepover whose scallop on the pins (radius 7.5) is within the
    // tolerance, so that the pins are covered within it (verification plan,
    // OP-26).
    let v = values(&[(names::STEPOVER, 0.3)]);
    let path = run(|ctx| generate(&SimultaneousFinishing, &f.input(), &v, ctx)).unwrap();
    let outline = Outline::new(&f.tool);
    let poses = poses_along(&path, &f.rotary, 4);
    let runs = passes(&path);
    for pin in &CRANK_PINS {
        // Away from the faces at its ends, where the ball's side rests on the
        // web and the turns of decision 0297 finish the face.
        let inner = (pin.x.0 + 2.5, pin.x.1 - 2.5);
        let inner = if pin.x.0 == 0.0 {
            (0.5, inner.1)
        } else {
            inner
        };
        let inner = if pin.x.1 == 40.0 {
            (inner.0, 39.5)
        } else {
            inner
        };
        let over: Vec<_> = runs
            .iter()
            .filter(|(a, b)| *b > inner.0 && *a < inner.1)
            .collect();
        assert_eq!(over.len(), 1, "pin at X {:?}: passes {over:?}", pin.x);
        let (worst, count) = axis_error(&path, pin, inner, 0.5 * DIAMETER);
        let left = left_on(
            &outline,
            &poses,
            &Pin { x: inner, ..*pin }.points(0.25, 0.0),
            1.0,
        );
        let (most, deepest) = left
            .iter()
            .fold((f64::NEG_INFINITY, f64::INFINITY), |(m, d), l| {
                (m.max(*l), d.min(*l))
            });
        eprintln!(
            "crankshaft pin at X {:?}: tool axis within {worst:.3} degrees of the exact normal \
             over {count} cuts; most left {most:.4} mm, deepest {deepest:.4} mm",
            pin.x
        );
        // A round needs no smoothing: the tool stays on the normal, within
        // the smoothed mesh normal's error (measured 0.096 degrees).
        assert!(worst <= 0.5, "{worst}");
        // Covered all round within the tolerance.
        assert!(most <= TOLERANCE, "pin at X {:?}: {most}", pin.x);
        // Nothing is cut into the round beyond the tolerance.
        assert!(deepest >= -TOLERANCE, "pin at X {:?}: {deepest}", pin.x);
    }
}

#[test]
fn req_op_26_req_nfr_06_off_axis_sections_are_identical_at_1_2_and_n_threads() {
    let f = Fixture::new(
        MeshSurface::new(&off_axis_cylinder()).unwrap(),
        14.0,
        &tool(ToolShape::BallEnd, DIAMETER),
    );
    let v = values(&[(names::STEPOVER, 1.5)]);
    let runs = openchuck_testkit::determinism::check(|| {
        let path = run(|ctx| generate(&SimultaneousFinishing, &f.input(), &v, ctx)).unwrap();
        format!("{:?} {:?}", path.group, path.notes).into_bytes()
    })
    .unwrap();
    assert_eq!(runs.len(), 4);
}

/// The crankshaft's inside corners where a web's face across the axis meets
/// a pin (`stl/crankshaft.stl`): the face's X, the pin, and the side of the
/// face the pin lies on. They are on the pin's side facing the rotary axis:
/// elsewhere the pin runs into the web's round end (decision record 0410).
const CRANK_CORNERS: [(f64, Pin, f64); 4] = [
    (10.0, CRANK_PINS[0], -1.0),
    (15.0, CRANK_PINS[1], 1.0),
    (25.0, CRANK_PINS[1], -1.0),
    (30.0, CRANK_PINS[2], 1.0),
];

/// Points within `band` mm of the inside corner at `face_x` beside `pin`,
/// on the pin and on the face, over the 40 degrees of the pin facing the
/// rotary axis, kept 0.02 mm off the corner line itself. The web joins its
/// two round ends by a waist about 7 mm wide round the axis, so the face
/// beside the pin is narrow: 40 degrees keeps the points on it.
fn inner_corner(face_x: f64, pin: &Pin, side: f64, band: f64) -> Vec<Point3> {
    let mut out = Vec::new();
    // Away from the axis is the direction of the pin's centre from it.
    let away = pin.centre.1.atan2(pin.centre.0);
    let steps = 12;
    for i in 0..=24 {
        let t = away + std::f64::consts::PI + (f64::from(i) / 24.0 - 0.5).to_radians() * 40.0;
        let (s, c) = t.sin_cos();
        for j in 1..=steps {
            let d = 0.02 + (band - 0.02) * f64::from(j - 1) / f64::from(steps - 1);
            // On the pin, `d` along X from the face.
            out.push(Point3::new(
                face_x + side * d,
                pin.centre.0 + pin.radius * c,
                pin.centre.1 + pin.radius * s,
            ));
            // On the face, `d` out from the pin.
            out.push(Point3::new(
                face_x,
                pin.centre.0 + (pin.radius + d) * c,
                pin.centre.1 + (pin.radius + d) * s,
            ));
        }
    }
    out
}

/// The Z1 3.175 mm tool with the owner's 25 mm flutes.
fn z1(shape: ToolShape) -> openchuck_types::tool::Tool {
    openchuck_types::tool::Tool {
        flute_length: openchuck_types::units::Millimetres(25.0),
        ..tool(shape, DIAMETER)
    }
}

#[test]
fn req_op_26_req_op_04_a_flat_end_finishes_the_inner_corners_of_the_crankshaft() {
    // Issue #439: the face turns of decision 0297 kept the tool's line
    // through the rotary axis, which meets a pin's side facing the axis up
    // to about 60 degrees from its normal, so a flat end rode on its rim and
    // left 0.3 to 1.2 mm in these corners. Turned round the pin's own
    // centre (decision record 0410) its side lies on the face and its
    // bottom on the pin, at any stepover: here the coarse 2.2 mm the owner's
    // operation took from its tool's cutting data.
    let f = Fixture::new(
        MeshSurface::new(&crankshaft()).unwrap(),
        18.0,
        &z1(ToolShape::FlatEnd),
    );
    let v = values(&[(names::STEPOVER, 2.2)]);
    let path = run(|ctx| generate(&SimultaneousFinishing, &f.input(), &v, ctx)).unwrap();
    let outline = Outline::new(&f.tool);
    let poses = poses_along(&path, &f.rotary, 4);
    for (face_x, pin, side) in &CRANK_CORNERS {
        let left = left_on(
            &outline,
            &poses,
            &inner_corner(*face_x, pin, *side, 0.5),
            2.0,
        );
        let (most, deepest) = left
            .iter()
            .fold((f64::NEG_INFINITY, f64::INFINITY), |(m, d), l| {
                (m.max(*l), d.min(*l))
            });
        eprintln!(
            "crankshaft inner corner at X {face_x}: most left {most:.4} mm, deepest {deepest:.4} mm"
        );
        assert!(most <= 2.0 * TOLERANCE, "corner at X {face_x}: {most}");
        assert!(deepest >= -TOLERANCE, "corner at X {face_x}: {deepest}");
    }
}

#[test]
fn req_op_26_req_op_04_face_turns_in_an_off_axis_zone_go_round_the_section_centre() {
    // Each face turn beside a pin is a pass at one X round the whole turn;
    // its tool lines pass through the pin's own centre, as the spiral's do
    // there (decision record 0410), not through the rotary axis.
    let f = Fixture::new(
        MeshSurface::new(&crankshaft()).unwrap(),
        18.0,
        &z1(ToolShape::FlatEnd),
    );
    let v = values(&[(names::STEPOVER, 2.2)]);
    let path = run(|ctx| generate(&SimultaneousFinishing, &f.input(), &v, ctx)).unwrap();
    let r = 0.5 * DIAMETER;
    for (face_x, pin, side) in &CRANK_CORNERS {
        // The face turn's X: the tool's side on the face.
        let x = face_x + side * r;
        let cuts: Vec<_> = path
            .group
            .segments
            .iter()
            .filter(|s| s.kind == MoveKind::Cutting && (s.end.tip.x - x).abs() < 0.05)
            .collect();
        let turned: f64 = cuts
            .windows(2)
            .map(|w| (w[1].end.a.0 - w[0].end.a.0).abs())
            .sum();
        assert!(
            turned >= 359.0,
            "face at X {face_x}: {turned} degrees at X {x}"
        );
        let mut worst = 0.0_f64;
        for s in &cuts {
            // Distance across X from the pin's centre line to the tool line.
            let a = s.end.axis.get();
            let d = Vec3::new(0.0, pin.centre.0 - s.end.tip.y, pin.centre.1 - s.end.tip.z);
            let off = (d - a * d.dot(a)).length();
            worst = worst.max(off);
        }
        eprintln!(
            "face turn at X {x:.3}: {} cuts, tool line within {worst:.4} mm of the pin's centre",
            cuts.len()
        );
        assert!(worst <= 0.1, "face at X {face_x}: {worst}");
    }
}

#[test]
fn req_op_26_a_ball_end_notes_the_corner_it_cannot_finish() {
    // A ball cannot reach into a corner sharper than its radius: it leaves
    // up to r (sqrt 2 - 1) across a square corner. The note says so in plain
    // words and that a flat end can finish it (decision record 0410).
    let v = values(&[(names::STEPOVER, 1.0)]);
    let note = |shape: ToolShape| {
        let f = Fixture::new(MeshSurface::new(&crankshaft()).unwrap(), 18.0, &z1(shape));
        let path = run(|ctx| generate(&SimultaneousFinishing, &f.input(), &v, ctx)).unwrap();
        path.notes
            .iter()
            .map(|n| n.message.clone())
            .filter(|m| m.contains("inside corner"))
            .collect::<Vec<_>>()
    };
    let ball = note(ToolShape::BallEnd);
    assert_eq!(ball.len(), 1, "{ball:?}");
    let expected = format!("{:.2} mm", 0.5 * DIAMETER * (2.0_f64.sqrt() - 1.0));
    assert!(ball[0].contains(&expected), "{}", ball[0]);
    assert!(ball[0].contains("a flat end can finish"), "{}", ball[0]);
    for x in ["10.000", "15.000", "25.000", "30.000"] {
        assert!(ball[0].contains(x), "{x}: {}", ball[0]);
    }
    assert!(note(ToolShape::FlatEnd).is_empty());
}

#[test]
fn req_op_26_no_corner_note_where_every_section_surrounds_the_axis() {
    // A stepped shaft on the axis has inside corners at its shoulder, but
    // its turns stay through the axis, as before, and no note is made.
    let shaft = openchuck_testkit::parts::revolve(
        &[(0.0, 5.0), (10.0, 5.0), (10.0, 8.0), (20.0, 8.0)],
        180,
    );
    let f = Fixture::new(
        MeshSurface::new(&shaft).unwrap(),
        18.0,
        &z1(ToolShape::BallEnd),
    );
    let v = values(&[(names::STEPOVER, 1.0)]);
    let path = run(|ctx| generate(&SimultaneousFinishing, &f.input(), &v, ctx)).unwrap();
    assert!(
        path.notes
            .iter()
            .all(|n| !n.message.contains("inside corner")),
        "{:?}",
        path.notes
    );
}
