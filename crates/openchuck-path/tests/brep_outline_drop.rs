// SPDX-License-Identifier: AGPL-3.0-or-later
//! Exact drops of tool outlines other than a ball: the tapered ball of axial
//! finishing and the flat end of pencil clean-up (lane STEPALL-X3, issue
//! #350; OI-81; requirement IMP-02, OP-04, OP-09, OP-11, OP-12, OP-23,
//! NFR-05, NFR-06, NFR-07; decision 0338), checked against independent
//! answers written here: closed forms, and brute-force samplings of the
//! faces with the tool's lift written out again. No kernel is involved.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::cast_precision_loss
)]

use std::f64::consts::TAU;

use openchuck_path::{
    BrepSurface, DROP_TOLERANCE, ExactFace, ParametricFace, SurfaceSource, ToolLine, Uv,
};
use openchuck_types::brep::{
    BSplinePatch, BrepFace, BrepModel, FaceId, Frame, Surface, TrimLoop, UvPoint,
};
use openchuck_types::geometry::{Point3, UnitVec3, Vec3};
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::tool::profile::{ProfilePoint, ProfileSegment, RevolvedProfile, ToolProfile};

// ---- builders -------------------------------------------------------------

fn p3(x: f64, y: f64, z: f64) -> Point3 {
    Point3::new(x, y, z)
}

fn z_frame() -> Frame {
    Frame {
        origin: Point3::ORIGIN,
        x: Vec3::new(1.0, 0.0, 0.0),
        y: Vec3::new(0.0, 1.0, 0.0),
        z: Vec3::new(0.0, 0.0, 1.0),
    }
}

/// A deterministic sequence in [0, 1).
fn lcg(seed: &mut u64) -> f64 {
    *seed = seed
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    (*seed >> 11) as f64 / (1u64 << 53) as f64
}

fn one_triangle(a: Point3, b: Point3, c: Point3) -> TriangleMesh {
    TriangleMesh {
        vertices: vec![a, b, c],
        triangles: vec![[0, 1, 2]],
    }
}

fn face(
    geometry: Surface,
    uv: [f64; 4],
    trims: &[Vec<(f64, f64)>],
    tessellation: TriangleMesh,
) -> BrepFace {
    BrepFace {
        id: FaceId(7),
        surface: geometry.kind(),
        body: 0,
        edges: vec![],
        geometry,
        reversed: false,
        uv_bounds: uv,
        trims: trims
            .iter()
            .map(|l| TrimLoop(l.iter().map(|&(u, v)| UvPoint { u, v }).collect()))
            .collect(),
        tessellation,
    }
}

/// A wavy rational bicubic patch over 6 x 5 poles with uneven knots: several
/// hills and hollows, so a point above it has several local nearest points.
fn wavy_patch() -> BSplinePatch {
    let (nu, nv) = (6usize, 5usize);
    let mut poles = Vec::new();
    let mut weights = Vec::new();
    for i in 0..nu {
        for j in 0..nv {
            let z = 6.0 * ((i as f64) * 1.7).sin() * ((j as f64) * 1.3).cos();
            poles.push(p3(8.0 * i as f64, 8.0 * j as f64, z));
            weights.push(1.0 + 0.6 * ((i + 2 * j) % 3) as f64);
        }
    }
    BSplinePatch {
        degree_u: 3,
        degree_v: 3,
        knots_u: vec![0.0, 0.0, 0.0, 0.0, 0.3, 0.45, 1.0, 1.0, 1.0, 1.0],
        knots_v: vec![0.0, 0.0, 0.0, 0.0, 0.6, 1.0, 1.0, 1.0, 1.0],
        poles_u: 6,
        poles_v: 5,
        poles,
        weights,
    }
}

/// A polynomial patch, quadratic in u with the middle knot doubled, whose
/// poles make a sharp ridge along u = 0.5: a crease inside the face, where
/// the highest contact need not be a point the ball rests on tangentially.
fn ridge_patch() -> BSplinePatch {
    let (nu, nv) = (5usize, 4usize);
    let mut poles = Vec::new();
    for i in 0..nu {
        for j in 0..nv {
            let z = [0.0, 3.0, 9.0, 3.0, 0.0][i] + 1.5 * ((j as f64) * 1.1).sin();
            poles.push(p3(10.0 * i as f64, 12.0 * j as f64, z));
        }
    }
    BSplinePatch {
        degree_u: 2,
        degree_v: 3,
        knots_u: vec![0.0, 0.0, 0.0, 0.5, 0.5, 1.0, 1.0, 1.0],
        knots_v: vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
        poles_u: 5,
        poles_v: 4,
        poles,
        weights: vec![1.0; 20],
    }
}

/// The trimmed faces of every kind: the surface, its parameter box, and a
/// wavy outer loop with a hole, in (u, v).
/// A named face of each kind with its loops in (u, v).
type Kind<F> = (&'static str, F, Vec<Vec<(f64, f64)>>);

fn kind_faces() -> Vec<Kind<BrepFace>> {
    let cone = Surface::Cone {
        frame: z_frame(),
        radius: 6.0,
        semi_angle: 0.4,
    };
    let quad = |box_: [f64; 4]| {
        let (cu, cv) = (0.5 * (box_[0] + box_[1]), 0.5 * (box_[2] + box_[3]));
        let (ru, rv) = (0.45 * (box_[1] - box_[0]), 0.45 * (box_[3] - box_[2]));
        let outer: Vec<(f64, f64)> = (0..400)
            .map(|i| {
                let a = TAU * i as f64 / 400.0;
                let k = 1.0 + 0.08 * (5.0 * a).sin();
                (cu + ru * k * a.cos(), cv + rv * k * a.sin())
            })
            .collect();
        let hole: Vec<(f64, f64)> = (0..120)
            .rev()
            .map(|i| {
                let a = TAU * i as f64 / 120.0;
                (cu + 0.3 * ru * a.cos(), cv + 0.25 * rv * a.sin())
            })
            .collect();
        vec![outer, hole]
    };
    let tri = one_triangle(p3(0.0, 0.0, 0.0), p3(1.0, 0.0, 0.0), p3(0.0, 1.0, 0.0));
    let mut out = Vec::new();
    for (name, geometry, box_) in [
        (
            "plane",
            Surface::Plane { frame: z_frame() },
            [-10.0, 10.0, -8.0, 8.0],
        ),
        (
            "cylinder",
            Surface::Cylinder {
                frame: z_frame(),
                radius: 7.0,
            },
            [0.2, 5.5, -6.0, 6.0],
        ),
        ("cone", cone, [0.1, 6.0, -4.0, 9.0]),
        (
            "sphere",
            Surface::Sphere {
                frame: z_frame(),
                radius: 9.0,
            },
            [0.0, 6.0, -1.4, 1.4],
        ),
        (
            "torus",
            Surface::Torus {
                frame: z_frame(),
                major: 12.0,
                minor: 4.0,
            },
            [0.3, 5.9, -2.8, 2.8],
        ),
        (
            "bspline",
            Surface::BSpline(wavy_patch()),
            [0.0, 1.0, 0.0, 1.0],
        ),
        (
            "creased",
            Surface::BSpline(ridge_patch()),
            [0.0, 1.0, 0.0, 1.0],
        ),
        (
            "polynomial",
            Surface::BSpline(BSplinePatch {
                weights: vec![1.0; 30],
                ..wavy_patch()
            }),
            [0.0, 1.0, 0.0, 1.0],
        ),
    ] {
        let loops = quad(box_);
        out.push((name, face(geometry, box_, &loops, tri.clone()), loops));
    }
    out
}

fn kinds() -> Vec<Kind<ExactFace>> {
    kind_faces()
        .into_iter()
        .map(|(n, f, l)| (n, ExactFace::new(&f).unwrap(), l))
        .collect()
}

// ---- the tools, written out again -----------------------------------------

/// A tool outline as the tests see it: its profile, and its lower envelope
/// `h(rho)` written out independently of the library (`None` beyond its
/// radius).
struct Tool {
    name: &'static str,
    profile: RevolvedProfile,
    h: Box<dyn Fn(f64) -> Option<f64> + Sync>,
    radius: f64,
}

fn profile(segments: Vec<ProfileSegment>) -> RevolvedProfile {
    ToolProfile { segments }.validate().unwrap()
}

fn pt(r: f64, z: f64) -> ProfilePoint {
    ProfilePoint::new(r, z)
}

/// A flat end of radius `r` with straight flutes.
fn flat(r: f64) -> Tool {
    Tool {
        name: "flat",
        profile: profile(vec![
            ProfileSegment::Line { to: pt(r, 0.0) },
            ProfileSegment::Line { to: pt(r, 4.0 * r) },
        ]),
        h: Box::new(move |rho| (rho <= r).then_some(0.0)),
        radius: r,
    }
}

/// A tapered ball: a ball of radius `rt` at the tip, then a taper of half
/// angle `degrees` tangent to it out to radius `r` (as the tool library
/// builds one).
fn tapered(rt: f64, degrees: f64, r: f64) -> Tool {
    let a = degrees.to_radians();
    let (tr, tz) = (rt * a.cos(), rt * (1.0 - a.sin()));
    let top = tz + (r - tr) / a.tan();
    Tool {
        name: "tapered",
        profile: profile(vec![
            ProfileSegment::Arc {
                to: pt(tr, tz),
                centre: pt(0.0, rt),
            },
            ProfileSegment::Line { to: pt(r, top) },
        ]),
        h: Box::new(move |rho| {
            if rho > r {
                None
            } else if rho <= tr {
                Some(rt - (rt * rt - rho * rho).max(0.0).sqrt())
            } else {
                Some(tz + (rho - tr) / a.tan())
            }
        }),
        radius: r,
    }
}

/// A V-bit with a flat at its tip: a joint between pieces where the slope
/// jumps, so the tool has an edge circle inside its radius.
fn flat_tipped_v(tip: f64, degrees: f64, r: f64) -> Tool {
    let k = 1.0 / degrees.to_radians().tan();
    Tool {
        name: "flat-tipped V",
        profile: profile(vec![
            ProfileSegment::Line { to: pt(tip, 0.0) },
            ProfileSegment::Line {
                to: pt(r, (r - tip) * k),
            },
        ]),
        h: Box::new(move |rho| {
            if rho > r {
                None
            } else {
                Some((rho - tip).max(0.0) * k)
            }
        }),
        radius: r,
    }
}

/// The highest tip offset along `line` at which the tool holds `p`.
fn lift(line: &ToolLine, tool: &Tool, p: Point3) -> Option<f64> {
    let a = line.axis.get();
    let w = p - line.origin;
    let z = w.dot(a);
    let rho = (w - a * z).length();
    (tool.h)(rho).map(|h| z - h)
}

/// The highest lift of a dense sampling of the trimmed face and its loops:
/// never above the true drop.
fn sampled_drop(
    f: &ExactFace,
    loops: &[Vec<(f64, f64)>],
    line: &ToolLine,
    tool: &Tool,
) -> Option<f64> {
    let (lo, hi) = f.uv_bounds();
    let mut best: Option<f64> = None;
    let mut take = |p: Point3| {
        if let Some(s) = lift(line, tool, p) {
            best = Some(best.map_or(s, |b: f64| b.max(s)));
        }
    };
    let n = 180;
    for i in 0..=n {
        for j in 0..=n {
            let uv = Uv {
                u: lo.u + (hi.u - lo.u) * i as f64 / n as f64,
                v: lo.v + (hi.v - lo.v) * j as f64 / n as f64,
            };
            if f.contains(uv)
                && let Some(sp) = f.point_normal(uv)
            {
                take(sp.point);
            }
        }
    }
    for l in loops {
        for k in 0..l.len() {
            let (a, b) = (l[k], l[(k + 1) % l.len()]);
            for s in 0..=20 {
                let t = f64::from(s) / 20.0;
                let uv = Uv {
                    u: a.0 + (b.0 - a.0) * t,
                    v: a.1 + (b.1 - a.1) * t,
                };
                if let Some(sp) = f.point_normal(uv) {
                    take(sp.point);
                }
            }
        }
    }
    best
}

fn one_face(f: BrepFace) -> BrepModel {
    BrepModel {
        faces: vec![f],
        edges: vec![],
        vertices: vec![],
        tessellation: Default::default(),
    }
}

/// Checks one exact drop against a lower bound `want` on the true drop:
/// never below it (less the tolerance), and a real contact, a point of the
/// face that the tool holds at the answer, so never above the true drop.
fn check(
    what: &str,
    f: &ExactFace,
    tool: &Tool,
    line: &ToolLine,
    got: Option<openchuck_path::ExactDrop>,
    want: Option<f64>,
) -> bool {
    match (got, want) {
        (None, None) => false,
        (Some(g), Some(w)) => {
            let s = g.contact.offset;
            assert!(
                s >= w - DROP_TOLERANCE - 1e-9,
                "{what}: {s} below a sample at {w}"
            );
            let c = g.contact.contact;
            assert!(
                f.closest(c).unwrap().distance < 1e-6,
                "{what}: off the face"
            );
            let l = lift(line, tool, c).unwrap_or(f64::NEG_INFINITY);
            assert!(
                (l - s).abs() <= DROP_TOLERANCE + 1e-9 || g.raised > 0.0,
                "{what}: the contact lifts the tool to {l}, not {s}"
            );
            true
        }
        // A sampling can miss a face the tool only grazes; the exact drop
        // must then be a real contact all the same.
        (Some(g), None) => {
            let l = lift(line, tool, g.contact.contact).unwrap_or(f64::NEG_INFINITY);
            assert!(
                (l - g.contact.offset).abs() <= DROP_TOLERANCE + 1e-9,
                "{what}: a contact the samples missed is not real"
            );
            false
        }
        (None, Some(w)) => panic!("{what}: no exact drop, sampled {w}"),
    }
}

fn tools() -> Vec<Tool> {
    vec![
        // The owner's axial finishing tool (decision 0211): 3.175 mm, a
        // 0.25 mm tip, 5 degrees.
        tapered(0.25, 5.0, 1.5875),
        // A wider taper, so the flank meets the faces at other slopes.
        tapered(0.6, 25.0, 3.0),
        // The owner's pencil tool: a 2 mm flat end.
        flat(1.0),
        flat(2.5),
        flat_tipped_v(0.4, 40.0, 2.0),
    ]
}

#[test]
fn req_op_12_exact_outline_drop_of_every_kind_is_at_or_above_brute_force() {
    let mut seed = 43;
    for tool in tools() {
        assert!(
            openchuck_path::has_exact_drop(&tool.profile),
            "{}",
            tool.name
        );
        for ((name, bf, loops), (_, f, _)) in kind_faces().into_iter().zip(kinds()) {
            let surface = BrepSurface::new(&one_face(bf)).unwrap();
            let b = f.bounding_box();
            let c = p3(
                0.5 * (b.min.x + b.max.x),
                0.5 * (b.min.y + b.max.y),
                0.5 * (b.min.z + b.max.z),
            );
            let size = (b.max - b.min).length();
            let mut hits = 0;
            for k in 0..24 {
                let axis = UnitVec3::new(Vec3::new(
                    lcg(&mut seed) - 0.5,
                    lcg(&mut seed) - 0.5,
                    lcg(&mut seed) + 0.3,
                ))
                .unwrap();
                let line = ToolLine {
                    origin: p3(
                        c.x + 0.6 * size * (lcg(&mut seed) - 0.5),
                        c.y + 0.6 * size * (lcg(&mut seed) - 0.5),
                        c.z + 0.6 * size * (lcg(&mut seed) - 0.5),
                    ),
                    axis,
                };
                let want = sampled_drop(&f, &loops, &line, &tool);
                let got = surface.drop_exact(&tool.profile, &line);
                let what = format!("{} {} on {name}, line {k}", tool.name, tool.radius);
                if check(&what, &f, &tool, &line, got, want) {
                    hits += 1;
                }
            }
            assert!(hits > 5, "{} on {name}: only {hits} lines touch", tool.name);
        }
    }
}

/// A large untrimmed plane through `p0` with unit normal `n`.
fn plane(p0: Point3, n: Vec3) -> BrepFace {
    let z = UnitVec3::new(n).unwrap().get();
    let seed = if z.x.abs() < 0.9 {
        Vec3::new(1.0, 0.0, 0.0)
    } else {
        Vec3::new(0.0, 1.0, 0.0)
    };
    let x = UnitVec3::new(seed - z * seed.dot(z)).unwrap().get();
    let y = z.cross(x);
    face(
        Surface::Plane {
            frame: Frame {
                origin: p0,
                x,
                y,
                z,
            },
        },
        [-60.0, 60.0, -60.0, 60.0],
        &[],
        one_triangle(p0, p0 + x, p0 + y),
    )
}

/// The drop onto an unbounded plane in closed form: the tool touches the
/// plane where `h(rho) c - rho m` is least over its outline (`c` the cosine
/// between the axis and the normal, `m` the sine), found here by a fine
/// sampling of the outline polished by golden sections.
fn plane_drop(tool: &Tool, line: &ToolLine, p0: Point3, n: Vec3) -> f64 {
    let n = UnitVec3::new(n).unwrap().get();
    let a = line.axis.get();
    let c = a.dot(n);
    let m = (1.0 - c * c).max(0.0).sqrt();
    let g = |rho: f64| (tool.h)(rho).unwrap() * c - rho * m;
    let steps = 200_000;
    let mut best = (f64::INFINITY, 0.0);
    for i in 0..=steps {
        let rho = tool.radius * f64::from(i) / f64::from(steps);
        let v = g(rho);
        if v < best.0 {
            best = (v, rho);
        }
    }
    let h = tool.radius / f64::from(steps);
    let (mut lo, mut hi) = ((best.1 - h).max(0.0), (best.1 + h).min(tool.radius));
    for _ in 0..200 {
        let (x1, x2) = (lo + 0.382 * (hi - lo), lo + 0.618 * (hi - lo));
        if g(x1) < g(x2) {
            hi = x2;
        } else {
            lo = x1;
        }
    }
    let least = g(lo).min(g(hi)).min(g(0.0)).min(g(tool.radius));
    ((p0 - line.origin).dot(n) - least) / c
}

#[test]
fn req_op_12_outline_drops_on_a_tilted_plane_match_the_closed_form() {
    // Flat bottom, flat rim, tapered tip and tapered flank in turn, as the
    // plane tilts from square to the axis to nearly along it.
    let mut seed = 7;
    for tool in tools() {
        for k in 0..30 {
            let tilt = (85.0 * f64::from(k) / 29.0).to_radians();
            let turn = TAU * lcg(&mut seed);
            let n = Vec3::new(tilt.sin() * turn.cos(), tilt.sin() * turn.sin(), tilt.cos());
            let p0 = p3(lcg(&mut seed), lcg(&mut seed), lcg(&mut seed) - 0.5);
            let surface = BrepSurface::new(&one_face(plane(p0, n))).unwrap();
            let line = ToolLine {
                origin: p3(0.3, -0.2, 20.0),
                axis: UnitVec3::Z,
            };
            let want = plane_drop(&tool, &line, p0, n);
            let got = surface.drop_exact(&tool.profile, &line).unwrap();
            assert!(
                (got.contact.offset - want).abs() <= DROP_TOLERANCE,
                "{} tilt {k}: {} against {want}, raised {}, looked {}",
                tool.name,
                got.contact.offset,
                got.raised,
                got.looked
            );
        }
    }
}

#[test]
fn req_op_11_a_tapered_ball_grown_for_stock_to_leave_drops_exactly() {
    // Stock to leave grows the tool (OP-11). The grown tapered ball is a
    // larger ball and a taper offset from the first: it has an exact drop,
    // and on a plane it is the original tool's drop onto the plane moved out
    // by the stock, less the stock (its tip lies the stock below the
    // original tool's).
    let tool = tapered(0.25, 5.0, 1.5875);
    let leave = 0.2;
    let grown = openchuck_path::cutter::grow_profile(&tool.profile, leave).unwrap();
    assert!(openchuck_path::has_exact_drop(&grown));
    for tilt in [0.0_f64, 10.0, 40.0, 70.0, 84.0] {
        let t = tilt.to_radians();
        let n = Vec3::new(t.sin(), 0.0, t.cos());
        let p0 = p3(0.2, 0.1, -0.3);
        let surface = BrepSurface::new(&one_face(plane(p0, n))).unwrap();
        let line = ToolLine {
            origin: p3(0.3, -0.2, 20.0),
            axis: UnitVec3::Z,
        };
        let want = plane_drop(&tool, &line, p0 + n * leave, n) - leave;
        let got = surface.drop_exact(&grown, &line).unwrap();
        assert!(
            (got.contact.offset - want).abs() <= DROP_TOLERANCE,
            "tilt {tilt}: {} against {want}",
            got.contact.offset
        );
    }
}

#[test]
fn req_op_12_a_flat_end_across_a_cylinder_seam_matches_the_closed_form() {
    // A whole cylinder of radius 6 along X, its seam (u = 0) on top: the
    // flat end of radius 1 on a line square to it is held by the top
    // generator when the line is within the radius of it, and otherwise by
    // its rim where the rim is nearest the top.
    let frame = Frame {
        origin: Point3::ORIGIN,
        x: Vec3::new(0.0, 0.0, 1.0),
        y: Vec3::new(0.0, 1.0, 0.0),
        z: Vec3::new(-1.0, 0.0, 0.0),
    };
    let rc = 6.0;
    let f = face(
        Surface::Cylinder { frame, radius: rc },
        [0.0, TAU, -10.0, 10.0],
        &[],
        one_triangle(p3(0.0, 0.0, 6.0), p3(1.0, 0.0, 6.0), p3(0.0, 1.0, 6.0)),
    );
    let surface = BrepSurface::new(&one_face(f)).unwrap();
    let tool = flat(1.0);
    for y0 in [-3.0, -1.2, -1.0, -0.4, 0.0, 0.3, 0.999, 2.5, 4.9] {
        let line = ToolLine {
            origin: p3(1.5, y0, 0.0),
            axis: UnitVec3::Z,
        };
        let near = (f64::abs(y0) - 1.0).max(0.0);
        let want = (rc * rc - near * near).sqrt();
        let got = surface.drop_exact(&tool.profile, &line).unwrap();
        assert!(
            (got.contact.offset - want).abs() <= DROP_TOLERANCE,
            "y {y0}: {} against {want}",
            got.contact.offset
        );
    }
}

#[test]
fn req_op_12_a_tool_over_a_face_edge_rests_on_the_edge() {
    // A plane rising along X, trimmed at x = 0: a flat end or a tapered
    // ball reaching over the trim edge is held by the edge, not by the
    // plane beyond it, at the edge's height under the outline.
    let slope = 0.3_f64;
    let n = Vec3::new(-slope, 0.0, 1.0);
    let f = BrepFace {
        trims: vec![TrimLoop(
            [(-20.0, -20.0), (0.0, -20.0), (0.0, 20.0), (-20.0, 20.0)]
                .iter()
                .map(|&(u, v)| UvPoint { u, v })
                .collect(),
        )],
        ..plane(Point3::ORIGIN, n)
    };
    let surface = BrepSurface::new(&one_face(f)).unwrap();
    let e = surface.faces()[0].clone();
    for tool in tools() {
        for x0 in [-3.0, -1.0, -0.5, 0.0, 0.4, 0.9] {
            let line = ToolLine {
                origin: p3(x0, 0.37, 10.0),
                axis: UnitVec3::Z,
            };
            // The face is the plane's points with their first parameter at
            // most 0; sample the edge and the face densely.
            let mut want: Option<f64> = None;
            for i in 0..=4000 {
                for (u, v) in [(0.0, -5.0 + 10.0 * f64::from(i) / 4000.0)] {
                    if let Some(sp) = e.point_normal(Uv { u, v })
                        && let Some(s) = lift(&line, &tool, sp.point)
                    {
                        want = Some(want.map_or(s, |w: f64| w.max(s)));
                    }
                }
            }
            let inside = e.uv_bounds();
            for i in 0..=300 {
                for j in 0..=300 {
                    let uv = Uv {
                        u: inside.0.u.max(-6.0) * f64::from(i) / 300.0,
                        v: -5.0 + 10.0 * f64::from(j) / 300.0,
                    };
                    if let Some(sp) = e.point_normal(uv)
                        && let Some(s) = lift(&line, &tool, sp.point)
                    {
                        want = Some(want.map_or(s, |w: f64| w.max(s)));
                    }
                }
            }
            let got = surface.drop_exact(&tool.profile, &line);
            let what = format!("{} at x {x0}", tool.name);
            check(&what, &e, &tool, &line, got, want);
        }
    }
}

#[test]
fn req_op_12_a_flat_end_rests_on_the_point_of_a_cone() {
    // The apex of a sharp cone pointing up: a flat end over it rests on the
    // point wherever the point is under the bottom.
    let down = Frame {
        origin: Point3::ORIGIN,
        x: Vec3::new(1.0, 0.0, 0.0),
        y: Vec3::new(0.0, -1.0, 0.0),
        z: Vec3::new(0.0, 0.0, -1.0),
    };
    let cone = face(
        Surface::Cone {
            frame: down,
            radius: 0.0,
            semi_angle: 0.3,
        },
        [0.0, TAU, 0.0, 10.0],
        &[],
        one_triangle(p3(0.0, 0.0, 0.0), p3(1.0, 0.0, -3.0), p3(0.0, 1.0, -3.0)),
    );
    let surface = BrepSurface::new(&one_face(cone)).unwrap();
    let tool = flat(1.0);
    for dx in [0.0, 0.05, 0.5, 0.95] {
        let line = ToolLine {
            origin: p3(dx, 0.1 * dx, 10.0),
            axis: UnitVec3::Z,
        };
        let got = surface.drop_exact(&tool.profile, &line).unwrap();
        // The tip at the point, 10 below the line's origin.
        assert!(
            (got.contact.offset + 10.0).abs() < 1e-9,
            "dx {dx}: {}",
            got.contact.offset
        );
        assert!(
            got.contact.contact.distance(Point3::ORIGIN) < 1e-9,
            "dx {dx}"
        );
    }
}

#[test]
fn req_op_12_a_trimmed_cone_without_its_point_holds_no_tool_there() {
    // The same sharp cone, its parameter box reaching the point (v = 0) but
    // its trim keeping only 2 <= v <= 10: the point is not on the face, so
    // a flat end over it is held by the trimmed face's top rim, below.
    let down = Frame {
        origin: Point3::ORIGIN,
        x: Vec3::new(1.0, 0.0, 0.0),
        y: Vec3::new(0.0, -1.0, 0.0),
        z: Vec3::new(0.0, 0.0, -1.0),
    };
    let ring: Vec<(f64, f64)> = vec![(0.0, 2.0), (TAU, 2.0), (TAU, 10.0), (0.0, 10.0)];
    let cone = face(
        Surface::Cone {
            frame: down,
            radius: 0.0,
            semi_angle: 0.3,
        },
        [0.0, TAU, 0.0, 10.0],
        std::slice::from_ref(&ring),
        one_triangle(p3(0.0, 0.0, -2.0), p3(1.0, 0.0, -3.0), p3(0.0, 1.0, -3.0)),
    );
    let surface = BrepSurface::new(&one_face(cone)).unwrap();
    let f = surface.faces()[0].clone();
    let tool = flat(1.0);
    for dx in [0.0, 0.3] {
        let line = ToolLine {
            origin: p3(dx, 0.1 * dx, 10.0),
            axis: UnitVec3::Z,
        };
        let got = surface.drop_exact(&tool.profile, &line).unwrap();
        // Never at the point, which the trim leaves out.
        assert!(
            got.contact.contact.distance(Point3::ORIGIN) > 0.1,
            "dx {dx}: {:?}",
            got.contact.contact
        );
        let want = sampled_drop(&f, std::slice::from_ref(&ring), &line, &tool);
        check(&format!("dx {dx}"), &f, &tool, &line, Some(got), want);
    }
}

#[test]
fn req_nfr_06_exact_outline_drop_is_the_same_on_every_thread_and_run() {
    let (_, bf, _) = kind_faces().into_iter().nth(6).unwrap();
    let surface = BrepSurface::new(&one_face(bf)).unwrap();
    let lines: Vec<ToolLine> = (0..200)
        .map(|k| ToolLine {
            origin: p3(1.0 + 0.2 * f64::from(k), 2.0 + 0.1 * f64::from(k), 30.0),
            axis: UnitVec3::new(Vec3::new(0.1, -0.2, 1.0)).unwrap(),
        })
        .collect();
    for tool in [tapered(0.25, 5.0, 1.5875), flat(1.0)] {
        let once: Vec<_> = lines
            .iter()
            .map(|l| surface.drop_exact(&tool.profile, l))
            .collect();
        std::thread::scope(|scope| {
            let handles: Vec<_> = (0..4)
                .map(|_| {
                    scope.spawn(|| {
                        lines
                            .iter()
                            .rev()
                            .map(|l| surface.drop_exact(&tool.profile, l))
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            for h in handles {
                let mut got = h.join().unwrap();
                got.reverse();
                assert_eq!(got, once);
            }
        });
        assert!(once.iter().filter(|d| d.is_some()).count() > 50);
    }
}

#[test]
fn req_nfr_07_outlines_without_an_exact_drop_and_surface_drops_stay_on_the_tessellation() {
    let model = openchuck_testkit::brep::banded_cylinder(10.0, 30.0, &[10.0, 20.0]);
    let surface = BrepSurface::new(&model).unwrap().with_exact_drops(true);
    let mesh_only = BrepSurface::new(&model).unwrap();
    let line = ToolLine {
        origin: p3(15.0, 0.0, 0.0),
        axis: UnitVec3::new(Vec3::new(0.0, 0.5, 1.0)).unwrap(),
    };
    // A bull nose's corner is an arc off the axis, upright at the rim; a
    // flat end grown for stock to leave becomes one; a sharp V-bit has a
    // point on the axis; a concave custom profile cannot be bounded by the
    // concave lift. None has an exact drop: each drops on the tessellation.
    let bull = profile(vec![
        ProfileSegment::Line { to: pt(0.7, 0.0) },
        ProfileSegment::Arc {
            to: pt(1.0, 0.3),
            centre: pt(0.7, 0.3),
        },
        ProfileSegment::Line { to: pt(1.0, 4.0) },
    ]);
    let grown_flat = openchuck_path::cutter::grow_profile(&flat(1.0).profile, 0.1).unwrap();
    let sharp_v = profile(vec![ProfileSegment::Line { to: pt(1.0, 1.0) }]);
    let concave = profile(vec![
        ProfileSegment::Line { to: pt(0.5, 0.0) },
        ProfileSegment::Arc {
            to: pt(1.0, 0.5),
            centre: pt(1.0, 0.0),
        },
    ]);
    for (name, p) in [
        ("bull nose", bull),
        ("grown flat", grown_flat),
        ("sharp V", sharp_v),
        ("concave", concave),
    ] {
        assert!(!openchuck_path::has_exact_drop(&p), "{name}");
        assert!(surface.drop_exact(&p, &line).is_none(), "{name}");
        assert_eq!(
            surface.drop_tool(&p, &line),
            mesh_only.drop_tool(&p, &line),
            "{name}"
        );
    }
    // A line that is not a number gives no answer, not a panic.
    let bad = ToolLine {
        origin: p3(f64::NAN, 0.0, 0.0),
        axis: UnitVec3::Z,
    };
    for tool in tools() {
        assert!(surface.drop_exact(&tool.profile, &bad).is_none());
        // The outline drop is not adopted (OI-1351): a surface asked for
        // exact drops still drops these tools on the tessellation.
        assert_eq!(
            surface.drop_tool(&tool.profile, &line),
            mesh_only.drop_tool(&tool.profile, &line),
            "{}",
            tool.name
        );
    }
}

#[test]
fn req_op_02_req_op_23_exact_outline_contacts_are_judged_on_exact_face_sets() {
    // OI-833 for the outline (decisions 0277 and 0338): flat-end and
    // tapered-ball exact contacts lie on the exact faces, off the triangles,
    // and the region and avoid sets judge them as exact faces, ready for a
    // strategy that adopts them (OI-1351). A check set drops the tool on
    // itself (`drop_tool`), which is the outline drop only once adopted.
    use openchuck_path::RegionConstraint;
    let model = openchuck_testkit::brep::banded_cylinder(10.0, 30.0, &[10.0, 20.0]);
    let part = BrepSurface::new(&model).unwrap().with_exact_drops(true);
    let band = |i: u32| {
        BrepSurface::with_faces(&model, &[FaceId(i)])
            .unwrap()
            .with_exact_drops(true)
    };
    let (first, middle) = (band(0), band(1));
    for tool in [flat(1.0), tapered(0.25, 5.0, 1.5875)] {
        let region = RegionConstraint {
            region: Some(&middle),
            ..RegionConstraint::default()
        };
        let avoid = RegionConstraint {
            avoid: Some(&first),
            ..RegionConstraint::default()
        };
        for a in (0..36).map(|k| 10.0 * f64::from(k) + 1.3) {
            for (x, in_middle) in [(5.0, false), (15.0, true), (25.0, false)] {
                let line = radial(x, a);
                let c = part.drop_exact(&tool.profile, &line).unwrap().contact;
                // The contact is exact: on the cylinder of radius 10.
                assert!(
                    (c.contact.y.hypot(c.contact.z) - 10.0).abs() < 1e-6,
                    "{} x {x} a {a}",
                    tool.name
                );
                let p = &tool.profile;
                assert_eq!(region.allows(c.contact, c.tip, p, &line), in_middle);
                assert_eq!(avoid.allows(c.contact, c.tip, p, &line), x > 10.0);
            }
        }
    }
}

fn radial(x: f64, degrees: f64) -> ToolLine {
    let (s, c) = degrees.to_radians().sin_cos();
    ToolLine {
        origin: p3(x, 0.0, 0.0),
        axis: UnitVec3::new(Vec3::new(0.0, c, s)).unwrap(),
    }
}
