// SPDX-License-Identifier: AGPL-3.0-or-later
//! The bounded tool-contact checker (OP-12, NFR-05, NFR-12; decision 0209
//! item 4.1 M2, decision 0211, issue #346): every answer is an interval that
//! holds the exact smallest clearance between the tool and the face, and is
//! narrower than the stated tolerance, or the check fails. Each case has a
//! closed-form answer computed here from the geometry alone.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::cast_precision_loss,
    clippy::float_cmp
)]

use std::f64::consts::{FRAC_PI_2, PI, TAU};

use openchuck_testkit::contact::{Bounded, ContactError, ConvexTool, Geometry, Settings};
use openchuck_types::brep::{
    BSplinePatch, BrepFace, BrepModel, FaceId, Frame, Surface, SurfaceKind, TrimLoop, UvPoint,
};
use openchuck_types::geometry::{Point3, Vec3};
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::tool::profile::ProfilePiece;
use openchuck_types::tool::{
    ProfilePoint, ProfileSegment, RevolvedProfile, Tool, ToolProfile, ToolShape,
};
use openchuck_types::units::{Degrees, Millimetres};

/// The tolerance every interval must meet: well under decision 0209's
/// 0.001 mm.
const TOL: f64 = 0.0005;

fn tool(shape: ToolShape, diameter: f64, flute: f64) -> RevolvedProfile {
    Tool {
        shape,
        diameter: Millimetres(diameter),
        flute_length: Millimetres(flute),
        overall_length: Millimetres(50.0),
        shank_diameter: Millimetres(diameter),
        flute_count: 2,
        material: None,
        centre_cutting: Some(true),
    }
    .profile()
    .unwrap()
}

fn flat2() -> RevolvedProfile {
    tool(ToolShape::FlatEnd, 2.0, 8.0)
}

fn ball2() -> RevolvedProfile {
    tool(ToolShape::BallEnd, 2.0, 12.0)
}

/// The tapered ball of the owner's axial finishing (decision 0211).
fn tapered() -> RevolvedProfile {
    tool(
        ToolShape::TaperedBall {
            tip_radius: Millimetres(0.25),
            taper_half_angle: Degrees(5.0),
        },
        3.175,
        20.0,
    )
}

fn face(geometry: Surface, uv_bounds: [f64; 4], trims: Vec<TrimLoop>) -> BrepFace {
    BrepFace {
        id: FaceId(0),
        surface: geometry_kind(&geometry),
        body: 0,
        edges: vec![],
        geometry,
        reversed: false,
        uv_bounds,
        trims,
        tessellation: TriangleMesh::default(),
    }
}

fn geometry_kind(s: &Surface) -> SurfaceKind {
    s.kind()
}

fn model(faces: Vec<BrepFace>) -> BrepModel {
    let faces = faces
        .into_iter()
        .enumerate()
        .map(|(i, f)| BrepFace {
            id: FaceId(i as u32),
            ..f
        })
        .collect();
    BrepModel {
        faces,
        ..BrepModel::default()
    }
}

fn settings() -> Settings {
    Settings {
        margin: 1.0,
        tolerance: TOL,
        budget: 2_000_000,
    }
}

/// The interval holds `expected` and is no wider than the tolerance.
fn assert_certifies(b: &Bounded, expected: f64) {
    assert!(
        b.lower <= expected + 1e-9 && expected <= b.upper + 1e-9,
        "{expected} is outside [{}, {}]",
        b.lower,
        b.upper
    );
    assert!(b.error() <= TOL + 1e-9, "interval {} wide", b.error());
    assert!(b.error() >= 0.0);
}

const S: f64 = std::f64::consts::FRAC_1_SQRT_2;

/// The analytic plane of decision 0211's flat-cutter counterexample:
/// z = x - 0.98, 0.95 <= x <= 1.05, 0 <= y <= 0.05.
fn counterexample_plane(trims: Vec<TrimLoop>) -> BrepFace {
    face(
        Surface::Plane {
            frame: Frame {
                origin: Point3::new(0.95, 0.0, -0.03),
                x: Vec3::new(S, 0.0, S),
                y: Vec3::new(0.0, 1.0, 0.0),
                z: Vec3::new(-S, 0.0, S),
            },
        },
        [0.0, 0.1 * 2.0_f64.sqrt(), 0.0, 0.05],
        trims,
    )
}

fn up() -> Vec3 {
    Vec3::new(0.0, 0.0, 1.0)
}

#[test]
fn req_op_12_flat_cutter_counterexample_is_detected() {
    // Decision 0211: a 2 mm flat cutter, tip at the origin, axis +Z. The
    // signed clearance max(r - 1, -z) on this face is smallest, -0.01 mm, at
    // (0.99, 0, 0.01). The 0.05 mm sample grid read -0.00333 mm there.
    let g = Geometry::of_faces(&model(vec![counterexample_plane(vec![])])).unwrap();
    let t = ConvexTool::new(&flat2()).unwrap();
    let b = g.clearance(&t, Point3::ORIGIN, up(), &settings()).unwrap();
    assert_certifies(&b, -0.01);
    let at = b.at.unwrap();
    assert!((at.x - 0.99).abs() < 0.01, "deepest at {at:?}");
}

#[test]
fn req_op_12_flat_cutter_counterexample_on_the_tessellation() {
    // The same plane as two triangles: the mesh check is bounded too.
    let mesh = TriangleMesh {
        vertices: vec![
            Point3::new(0.95, 0.0, -0.03),
            Point3::new(1.05, 0.0, 0.07),
            Point3::new(1.05, 0.05, 0.07),
            Point3::new(0.95, 0.05, -0.03),
        ],
        triangles: vec![[0, 1, 2], [0, 2, 3]],
    };
    let g = Geometry::of_mesh(&mesh).unwrap();
    let t = ConvexTool::new(&flat2()).unwrap();
    let b = g.clearance(&t, Point3::ORIGIN, up(), &settings()).unwrap();
    assert_certifies(&b, -0.01);
}

#[test]
fn req_op_12_trims_bound_the_face() {
    // Trimmed to x <= 0.985 (u <= 0.035 * sqrt 2): the deepest point moves
    // to the trim, where max(x - 1, 0.98 - x) is -0.005 mm.
    let w = 0.035 * 2.0_f64.sqrt();
    let trim = TrimLoop(vec![
        UvPoint { u: 0.0, v: 0.0 },
        UvPoint { u: w, v: 0.0 },
        UvPoint { u: w, v: 0.05 },
        UvPoint { u: 0.0, v: 0.05 },
    ]);
    let g = Geometry::of_faces(&model(vec![counterexample_plane(vec![trim])])).unwrap();
    let t = ConvexTool::new(&flat2()).unwrap();
    let b = g.clearance(&t, Point3::ORIGIN, up(), &settings()).unwrap();
    assert_certifies(&b, -0.005);
}

#[test]
fn req_op_12_a_hole_in_the_face_is_not_checked() {
    // A hole round the deepest point (x from 0.98 to 1.0, all y): the
    // deepest left is at the hole's edges, x = 0.98 and x = 1.0, where
    // max(x - 1, 0.98 - x) is 0 at both.
    let u = |x: f64| (x - 0.95) / S;
    let outer = TrimLoop(vec![
        UvPoint { u: 0.0, v: 0.0 },
        UvPoint { u: u(1.05), v: 0.0 },
        UvPoint {
            u: u(1.05),
            v: 0.05,
        },
        UvPoint { u: 0.0, v: 0.05 },
    ]);
    let hole = TrimLoop(vec![
        UvPoint {
            u: u(0.98),
            v: -0.01,
        },
        UvPoint {
            u: u(0.98),
            v: 0.06,
        },
        UvPoint { u: u(1.0), v: 0.06 },
        UvPoint {
            u: u(1.0),
            v: -0.01,
        },
    ]);
    let g = Geometry::of_faces(&model(vec![counterexample_plane(vec![outer, hole])])).unwrap();
    let t = ConvexTool::new(&flat2()).unwrap();
    let b = g.clearance(&t, Point3::ORIGIN, up(), &settings()).unwrap();
    assert_certifies(&b, 0.0);
}

/// A cylinder of radius 10 about the X axis, u = 0 along +Y.
fn x_cylinder(u: [f64; 2], trims: Vec<TrimLoop>) -> BrepFace {
    face(
        Surface::Cylinder {
            frame: Frame {
                origin: Point3::ORIGIN,
                x: Vec3::new(0.0, 1.0, 0.0),
                y: Vec3::new(0.0, 0.0, 1.0),
                z: Vec3::new(1.0, 0.0, 0.0),
            },
            radius: 10.0,
        },
        [u[0], u[1], -5.0, 5.0],
        trims,
    )
}

/// A ball's tip `d` above the cylinder (negative into it), the tool
/// pointing at the axis from angle `angle` (from +Y towards +Z), its centre
/// `offset` along X. The clearance is the centre's distance from the
/// surface less the ball's radius.
fn ball_over_cylinder(
    geometry: &Geometry,
    profile: &RevolvedProfile,
    angle: f64,
    d: f64,
) -> Bounded {
    let radial = Vec3::new(0.0, angle.cos(), angle.sin());
    let tip = Point3::new(0.3, 0.0, 0.0) + radial * (10.0 + d);
    let t = ConvexTool::new(profile).unwrap();
    geometry.clearance(&t, tip, radial, &settings()).unwrap()
}

#[test]
fn req_op_12_ball_on_a_cylinder() {
    let g = Geometry::of_faces(&model(vec![x_cylinder([0.0, TAU], vec![])])).unwrap();
    for d in [-0.02, -0.004, 0.0, 0.003, 0.3] {
        for angle in [0.0, 0.7, FRAC_PI_2, 3.0, 5.5] {
            let b = ball_over_cylinder(&g, &ball2(), angle, d);
            assert_certifies(&b, d);
        }
    }
}

#[test]
fn req_op_12_tapered_ball_tip_on_a_cylinder() {
    let g = Geometry::of_faces(&model(vec![x_cylinder([0.0, TAU], vec![])])).unwrap();
    for d in [-0.02, 0.0, 0.002] {
        let b = ball_over_cylinder(&g, &tapered(), 1.1, d);
        assert_certifies(&b, d);
    }
}

#[test]
fn req_op_12_trims_across_the_seam_of_a_periodic_face() {
    // The trim runs from u = -0.3 to 0.3 although the box is 0 to 2 pi: the
    // face wraps, as `openchuck-path` reads it (one period either side).
    let trim = TrimLoop(vec![
        UvPoint { u: -0.3, v: -5.0 },
        UvPoint { u: 0.3, v: -5.0 },
        UvPoint { u: 0.3, v: 5.0 },
        UvPoint { u: -0.3, v: 5.0 },
    ]);
    let g = Geometry::of_faces(&model(vec![x_cylinder([0.0, TAU], vec![trim])])).unwrap();
    assert_certifies(&ball_over_cylinder(&g, &ball2(), -0.2, -0.02), -0.02);
    assert_certifies(&ball_over_cylinder(&g, &ball2(), 0.2, 0.004), 0.004);
    // Opposite the strip nothing is within the margin.
    let far = ball_over_cylinder(&g, &ball2(), PI, -0.02);
    assert!(far.at.is_none() && far.lower >= 1.0 - TOL, "{far:?}");
}

#[test]
fn req_op_12_ball_on_a_sphere_cone_and_torus() {
    let frame = Frame {
        origin: Point3::new(1.0, -2.0, 0.5),
        x: Vec3::new(1.0, 0.0, 0.0),
        y: Vec3::new(0.0, 1.0, 0.0),
        z: Vec3::new(0.0, 0.0, 1.0),
    };
    let ball = ConvexTool::new(&ball2()).unwrap();
    // A sphere of radius 5: from direction n the clearance is the centre's
    // distance less 5 less 1.
    let sphere = Geometry::of_faces(&model(vec![face(
        Surface::Sphere { frame, radius: 5.0 },
        [0.0, TAU, -FRAC_PI_2, FRAC_PI_2],
        vec![],
    )]))
    .unwrap();
    for (n, d) in [
        (Vec3::new(0.3, 0.4, 0.866), -0.01),
        (Vec3::new(-1.0, 0.2, -0.1), 0.002),
    ] {
        let n = n * (1.0 / n.length());
        let tip = frame.origin + n * (5.0 + d);
        assert_certifies(&sphere.clearance(&ball, tip, n, &settings()).unwrap(), d);
    }
    // A torus, major 10, minor 2, approached at its outer equator.
    let torus = Geometry::of_faces(&model(vec![face(
        Surface::Torus {
            frame,
            major: 10.0,
            minor: 2.0,
        },
        [0.0, TAU, 0.0, TAU],
        vec![],
    )]))
    .unwrap();
    for (angle, d) in [(0.4_f64, -0.015), (2.0, 0.003)] {
        let radial = Vec3::new(angle.cos(), angle.sin(), 0.0);
        let tip = frame.origin + radial * (12.0 + d);
        assert_certifies(
            &torus.clearance(&ball, tip, radial, &settings()).unwrap(),
            d,
        );
    }
    // A cone, radius 4 at v = 0, semi-angle 0.3, the tool along its surface
    // normal at (u, v) = (1.0, 2.0).
    let (a, r0) = (0.3_f64, 4.0);
    let cone = Geometry::of_faces(&model(vec![face(
        Surface::Cone {
            frame,
            radius: r0,
            semi_angle: a,
        },
        [0.0, TAU, -3.0, 6.0],
        vec![],
    )]))
    .unwrap();
    let (u, v) = (1.0_f64, 2.0);
    let radial = Vec3::new(u.cos(), u.sin(), 0.0);
    let on = frame.origin + radial * (r0 + v * a.sin()) + Vec3::new(0.0, 0.0, v * a.cos());
    let normal = radial * a.cos() + Vec3::new(0.0, 0.0, -a.sin());
    for d in [-0.012, 0.004] {
        let tip = on + normal * d;
        assert_certifies(&cone.clearance(&ball, tip, normal, &settings()).unwrap(), d);
    }
}

/// The smallest of `l(r, z) = z cos(phi) - r sin(phi)` over a convex
/// profile region: at a piece's end or where an arc's normal points along
/// the gradient's opposite. The lowest point of a tool tilted by `phi` lies
/// that far below its tip.
fn lowest_below_tip(profile: &RevolvedProfile, phi: f64) -> f64 {
    let l = |p: ProfilePoint| p.height.0 * phi.cos() - p.radius.0 * phi.sin();
    let mut best = 0.0_f64;
    for piece in profile.pieces() {
        best = best.min(l(piece.start())).min(l(piece.end()));
        if let ProfilePiece::Arc {
            start,
            end,
            centre,
            radius,
            ..
        } = *piece
        {
            // The arc's point whose outward normal is (sin phi, -cos phi).
            let angle = (-phi.cos()).atan2(phi.sin());
            let angle_of = |p: ProfilePoint| {
                (p.height.0 - centre.height.0).atan2(p.radius.0 - centre.radius.0)
            };
            let (a0, a1) = (angle_of(start), angle_of(end));
            if angle >= a0.min(a1) && angle <= a0.max(a1) {
                let p = ProfilePoint::new(
                    centre.radius.0 + radius * angle.cos(),
                    centre.height.0 + radius * angle.sin(),
                );
                best = best.min(l(p));
            }
        }
    }
    best
}

#[test]
fn req_op_12_tilted_tools_clear_a_plane_by_their_lowest_point() {
    // A tool tilted by phi about Y above the plane z = 0: its clearance is
    // the height of its lowest point, on the ball tip, the taper's flank or
    // the flat end's rim.
    let plane = Geometry::of_faces(&model(vec![face(
        Surface::Plane {
            frame: Frame {
                origin: Point3::new(-30.0, -30.0, 0.0),
                x: Vec3::new(1.0, 0.0, 0.0),
                y: Vec3::new(0.0, 1.0, 0.0),
                z: Vec3::new(0.0, 0.0, 1.0),
            },
        },
        [0.0, 60.0, 0.0, 60.0],
        vec![],
    )]))
    .unwrap();
    for (profile, phi) in [
        (tapered(), 0.5),
        (tapered(), 88.0_f64.to_radians()),
        (ball2(), 1.2),
        (flat2(), 0.3),
    ] {
        let axis = Vec3::new(phi.sin(), 0.0, phi.cos());
        let t = ConvexTool::new(&profile).unwrap();
        for gap in [0.0021, 0.04] {
            let tip = Point3::new(0.0, 0.0, gap - lowest_below_tip(&profile, phi));
            let b = plane.clearance(&t, tip, axis, &settings()).unwrap();
            assert_certifies(&b, gap);
        }
    }
}

/// The counterexample plane as a rational B-spline with nonuniform knots,
/// nonuniform pole spacing and unequal weights: the same surface, a very
/// uneven parametrisation.
fn nonuniform_plane() -> BrepFace {
    let s = [0.0, 0.01, 0.02, 0.4, 0.45, 0.9, 1.0];
    let w = [1.0, 2.0, 0.5, 1.0, 3.0, 1.0, 1.0];
    let mut poles = Vec::new();
    let mut weights = Vec::new();
    for (si, wi) in s.iter().zip(w) {
        let x = 0.95 + 0.1 * si;
        for y in [0.0, 0.05] {
            poles.push(Point3::new(x, y, x - 0.98));
            weights.push(wi);
        }
    }
    face(
        Surface::BSpline(BSplinePatch {
            degree_u: 3,
            degree_v: 1,
            knots_u: vec![0.0, 0.0, 0.0, 0.0, 0.1, 0.15, 0.9, 1.0, 1.0, 1.0, 1.0],
            knots_v: vec![0.0, 0.0, 1.0, 1.0],
            poles_u: 7,
            poles_v: 2,
            poles,
            weights,
        }),
        [0.0, 1.0, 0.0, 1.0],
        vec![],
    )
}

#[test]
fn req_op_12_nonuniform_b_spline_face() {
    let g = Geometry::of_faces(&model(vec![nonuniform_plane()])).unwrap();
    let t = ConvexTool::new(&flat2()).unwrap();
    let b = g.clearance(&t, Point3::ORIGIN, up(), &settings()).unwrap();
    assert_certifies(&b, -0.01);
}

/// A rational quadratic B-spline cylinder of radius 10 about X, from 0 to
/// 90 degrees in two uneven spans (0 to 30 and 30 to 90 degrees, knots at 0,
/// 0.2 and 1), from x = -5 to 5.
fn nurbs_cylinder() -> BrepFace {
    let p = |deg: f64, scale: f64| {
        let a = deg.to_radians();
        (10.0 * scale * a.cos(), 10.0 * scale * a.sin())
    };
    let c15 = 15.0_f64.to_radians().cos();
    let c30 = 30.0_f64.to_radians().cos();
    let ring = [
        (p(0.0, 1.0), 1.0),
        (p(15.0, 1.0 / c15), c15),
        (p(30.0, 1.0), 1.0),
        (p(60.0, 1.0 / c30), c30),
        (p(90.0, 1.0), 1.0),
    ];
    let mut poles = Vec::new();
    let mut weights = Vec::new();
    for ((y, z), w) in ring {
        for x in [-5.0, 5.0] {
            poles.push(Point3::new(x, y, z));
            weights.push(w);
        }
    }
    face(
        Surface::BSpline(BSplinePatch {
            degree_u: 2,
            degree_v: 1,
            knots_u: vec![0.0, 0.0, 0.0, 0.2, 0.2, 1.0, 1.0, 1.0],
            knots_v: vec![0.0, 0.0, 1.0, 1.0],
            poles_u: 5,
            poles_v: 2,
            poles,
            weights,
        }),
        [0.0, 1.0, 0.0, 1.0],
        vec![],
    )
}

#[test]
fn req_op_12_rational_b_spline_with_uneven_spans() {
    let g = Geometry::of_faces(&model(vec![nurbs_cylinder()])).unwrap();
    for angle in [0.2_f64, 0.5, 1.0, 1.4] {
        for d in [-0.02, 0.003] {
            let b = ball_over_cylinder(&g, &ball2(), angle, d);
            assert_certifies(&b, d);
        }
    }
}

#[test]
fn req_op_12_several_faces_take_the_nearest() {
    // The cylinder and, 0.5 mm further out, nothing nearer: the ball sees
    // the cylinder's clearance; a second face 0.01 mm under the tip wins.
    let near = face(
        Surface::Plane {
            frame: Frame {
                origin: Point3::new(-1.0, -1.0, 10.29),
                x: Vec3::new(1.0, 0.0, 0.0),
                y: Vec3::new(0.0, 1.0, 0.0),
                z: Vec3::new(0.0, 0.0, 1.0),
            },
        },
        [0.0, 2.0, 0.0, 2.0],
        vec![],
    );
    let g = Geometry::of_faces(&model(vec![x_cylinder([0.0, TAU], vec![]), near])).unwrap();
    // Tip at z = 10.3: the plane at 10.29 is 0.01 below it.
    let b = ball_over_cylinder(&g, &ball2(), FRAC_PI_2, 0.3);
    assert_certifies(&b, 0.01);
}

#[test]
fn req_op_12_no_bound_no_answer() {
    let g = Geometry::of_faces(&model(vec![counterexample_plane(vec![])])).unwrap();
    let t = ConvexTool::new(&flat2()).unwrap();
    // Too small a budget to close the interval.
    let tight = Settings {
        budget: 3,
        ..settings()
    };
    let e = g.clearance(&t, Point3::ORIGIN, up(), &tight).unwrap_err();
    assert!(matches!(e, ContactError::Budget { .. }));
    assert!(e.to_string().contains("only known to lie in"), "{e}");
    // A tolerance of zero, or not finite, is not a bound.
    for tolerance in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        let s = Settings {
            tolerance,
            ..settings()
        };
        assert!(g.clearance(&t, Point3::ORIGIN, up(), &s).is_err());
    }
    // A pose that is not finite has no clearance.
    assert!(
        g.clearance(&t, Point3::new(f64::NAN, 0.0, 0.0), up(), &settings())
            .is_err()
    );
    assert!(
        g.clearance(&t, Point3::ORIGIN, Vec3::ZERO, &settings())
            .is_err()
    );
}

#[test]
fn req_op_12_a_concave_tool_has_no_bound() {
    // A corner-rounding cutter's concave arc: the clearance is not convex,
    // so no bound is claimed.
    let profile = ToolProfile {
        segments: vec![
            ProfileSegment::Line {
                to: ProfilePoint::new(0.5, 0.0),
            },
            ProfileSegment::Arc {
                to: ProfilePoint::new(1.0, 0.5),
                centre: ProfilePoint::new(1.0, 0.0),
            },
            ProfileSegment::Line {
                to: ProfilePoint::new(1.0, 5.0),
            },
        ],
    }
    .validate()
    .unwrap();
    let e = ConvexTool::new(&profile).unwrap_err();
    assert!(matches!(e, ContactError::NotConvex(_)));
    assert!(e.to_string().contains("not convex"), "{e}");
}

#[test]
fn req_op_12_a_face_without_positive_weights_has_no_bound() {
    let mut f = nonuniform_plane();
    if let Surface::BSpline(p) = &mut f.geometry {
        p.weights[3] = -1.0;
    }
    assert!(matches!(
        Geometry::of_faces(&model(vec![f])),
        Err(ContactError::Face { .. })
    ));
    let mut f = counterexample_plane(vec![]);
    f.uv_bounds = [0.0, f64::NAN, 0.0, 1.0];
    assert!(Geometry::of_faces(&model(vec![f])).is_err());
    // NFR-07: every malformed face is refused with a reason, not evaluated.
    type Break = Box<dyn Fn(&mut BrepFace)>;
    let broken: Vec<Break> = vec![
        Box::new(|f| {
            if let Surface::BSpline(p) = &mut f.geometry {
                p.knots_u.pop();
            }
        }),
        Box::new(|f| {
            if let Surface::BSpline(p) = &mut f.geometry {
                p.knots_u.swap(4, 6);
            }
        }),
        Box::new(|f| {
            if let Surface::BSpline(p) = &mut f.geometry {
                p.poles.pop();
            }
        }),
        Box::new(|f| {
            if let Surface::BSpline(p) = &mut f.geometry {
                p.poles[0].x = f64::INFINITY;
            }
        }),
        Box::new(|f| {
            if let Surface::BSpline(p) = &mut f.geometry {
                p.degree_u = 9;
            }
        }),
        Box::new(|f| f.uv_bounds = [2.0, 3.0, 0.0, 1.0]),
        Box::new(|f| {
            f.trims = vec![TrimLoop(vec![
                UvPoint { u: 0.0, v: 0.0 },
                UvPoint {
                    u: f64::NAN,
                    v: 0.5,
                },
                UvPoint { u: 1.0, v: 1.0 },
            ])];
        }),
    ];
    for (k, b) in broken.iter().enumerate() {
        let mut f = nonuniform_plane();
        b(&mut f);
        let e = Geometry::of_faces(&model(vec![f])).unwrap_err();
        assert!(matches!(e, ContactError::Face { .. }), "{k}: {e}");
        assert!(e.to_string().contains("face 0"), "{k}: {e}");
    }
    let mut f = counterexample_plane(vec![]);
    if let Surface::Plane { frame } = &mut f.geometry {
        frame.x = Vec3::new(1.0, 1.0, 0.0);
    }
    assert!(Geometry::of_faces(&model(vec![f])).is_err());
    let cone = face(
        Surface::Cone {
            frame: Frame {
                origin: Point3::ORIGIN,
                x: Vec3::new(1.0, 0.0, 0.0),
                y: Vec3::new(0.0, 1.0, 0.0),
                z: Vec3::new(0.0, 0.0, 1.0),
            },
            radius: 1.0,
            semi_angle: 2.0,
        },
        [0.0, 1.0, 0.0, 1.0],
        vec![],
    );
    assert!(Geometry::of_faces(&model(vec![cone])).is_err());
    let sphere = face(
        Surface::Sphere {
            frame: Frame {
                origin: Point3::ORIGIN,
                x: Vec3::new(1.0, 0.0, 0.0),
                y: Vec3::new(0.0, 1.0, 0.0),
                z: Vec3::new(0.0, 0.0, 1.0),
            },
            radius: -1.0,
        },
        [0.0, 1.0, 0.0, 1.0],
        vec![],
    );
    assert!(Geometry::of_faces(&model(vec![sphere])).is_err());
    // A triangle naming a missing vertex.
    let mesh = TriangleMesh {
        vertices: vec![Point3::ORIGIN],
        triangles: vec![[0, 1, 2]],
    };
    let e = Geometry::of_mesh(&mesh).unwrap_err();
    assert!(e.to_string().contains("no finite corner"), "{e}");
}

#[test]
fn req_nfr_06_answers_do_not_depend_on_the_thread() {
    // The same pose gives the same interval, bit for bit, on any thread.
    let g = Geometry::of_faces(&model(vec![
        nonuniform_plane(),
        x_cylinder([0.0, TAU], vec![]),
    ]))
    .unwrap();
    let t = ConvexTool::new(&ball2()).unwrap();
    let tip = Point3::new(0.2, 0.1, 10.01);
    let first = g.clearance(&t, tip, up(), &settings()).unwrap();
    let others: Vec<Bounded> = std::thread::scope(|s| {
        let hs: Vec<_> = (0..4)
            .map(|_| s.spawn(|| g.clearance(&t, tip, up(), &settings()).unwrap()))
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    for o in others {
        assert_eq!(o.upper.to_bits(), first.upper.to_bits());
        assert_eq!(o.lower.to_bits(), first.lower.to_bits());
    }
}

/// A small deterministic generator for the random cases.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
    fn range(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.next()
    }
}

/// Cox-de Boor basis of degree `p` at `t`, written separately from the
/// checker's blossoms.
fn basis(knots: &[f64], p: usize, i: usize, t: f64) -> f64 {
    if p == 0 {
        let last = knots[knots.len() - 1];
        return if (knots[i] <= t && t < knots[i + 1])
            || (t == last && knots[i + 1] == last && knots[i] < last)
        {
            1.0
        } else {
            0.0
        };
    }
    let mut out = 0.0;
    let d1 = knots[i + p] - knots[i];
    if d1 > 0.0 {
        out += (t - knots[i]) / d1 * basis(knots, p - 1, i, t);
    }
    let d2 = knots[i + p + 1] - knots[i + 1];
    if d2 > 0.0 {
        out += (knots[i + p + 1] - t) / d2 * basis(knots, p - 1, i + 1, t);
    }
    out
}

fn evaluate(p: &BSplinePatch, u: f64, v: f64) -> Point3 {
    let (nu, nv) = (p.poles_u as usize, p.poles_v as usize);
    let (mut x, mut y, mut z, mut w) = (0.0, 0.0, 0.0, 0.0);
    for i in 0..nu {
        let bu = basis(&p.knots_u, p.degree_u as usize, i, u);
        if bu == 0.0 {
            continue;
        }
        for j in 0..nv {
            let b = bu * basis(&p.knots_v, p.degree_v as usize, j, v) * p.weights[i * nv + j];
            let q = p.poles[i * nv + j];
            x += b * q.x;
            y += b * q.y;
            z += b * q.z;
            w += b;
        }
    }
    Point3::new(x / w, y / w, z / w)
}

/// The signed distance of `q` from a flat end mill of radius `r` and
/// flute `h`, tip at `tip`, axis +Z, in closed form.
fn flat_clearance(q: Point3, tip: Point3, r: f64, h: f64) -> f64 {
    let w = q - tip;
    let rho = w.x.hypot(w.y);
    let (dr, dz) = (rho - r, (-w.z).max(w.z - h));
    if dr <= 0.0 && dz <= 0.0 {
        dr.max(dz)
    } else {
        dr.max(0.0).hypot(dz.max(0.0))
    }
}

#[test]
fn req_op_12_bounds_hold_against_dense_samples_on_random_rational_patches() {
    // Random bicubic-by-quadratic rational patches with uneven knots and
    // weights, a flat end mill placed over each: the proven lower bound is
    // never above the smallest of 160,000 independently evaluated samples,
    // and the found value is within the tolerance of it or below.
    let mut rng = Lcg(346);
    let flat = ConvexTool::new(&flat2()).unwrap();
    for case in 0..12 {
        let (nu, nv) = (6, 4);
        let mut knots_u = vec![0.0; 4];
        let (a, b) = (rng.range(0.05, 0.4), rng.range(0.5, 0.95));
        knots_u.extend([a, b]);
        knots_u.extend([1.0; 4]);
        let knots_v = vec![0.0, 0.0, 0.0, rng.range(0.1, 0.9), 1.0, 1.0, 1.0];
        let mut poles = Vec::new();
        let mut weights = Vec::new();
        for i in 0..nu {
            for j in 0..nv {
                poles.push(Point3::new(
                    i as f64 * 0.6 + rng.range(-0.1, 0.1),
                    j as f64 * 0.8 + rng.range(-0.1, 0.1),
                    rng.range(-0.4, 0.4),
                ));
                weights.push(rng.range(0.5, 2.0));
            }
        }
        let patch = BSplinePatch {
            degree_u: 3,
            degree_v: 2,
            knots_u,
            knots_v,
            poles_u: nu as u32,
            poles_v: nv as u32,
            poles,
            weights,
        };
        let n = 400;
        let samples: Vec<Point3> = (0..=n)
            .flat_map(|i| (0..=n).map(move |j| (i as f64 / n as f64, j as f64 / n as f64)))
            .map(|(u, v)| evaluate(&patch, u, v))
            .collect();
        let g = Geometry::of_faces(&model(vec![face(
            Surface::BSpline(patch),
            [0.0, 1.0, 0.0, 1.0],
            vec![],
        )]))
        .unwrap();
        for _ in 0..4 {
            let tip = Point3::new(
                rng.range(0.5, 2.5),
                rng.range(0.5, 2.0),
                rng.range(-0.5, 0.3),
            );
            let sampled = samples
                .iter()
                .map(|q| flat_clearance(*q, tip, 1.0, 8.0))
                .fold(f64::INFINITY, f64::min)
                .min(1.0);
            let b = g.clearance(&flat, tip, up(), &settings()).unwrap();
            assert!(
                b.lower <= sampled + 1e-9,
                "case {case}: {b:?} above {sampled}"
            );
            assert!(
                b.upper <= sampled + TOL + 1e-9,
                "case {case}: {b:?} vs {sampled}"
            );
            assert!(b.error() <= TOL + 1e-9);
            if let Some(at) = b.at {
                assert!((flat_clearance(at, tip, 1.0, 8.0) - b.upper).abs() < 1e-9);
            }
        }
    }
}

#[test]
fn req_op_12_a_trims_top_edge_inside_the_box_bounds_the_face() {
    // The counterexample plane widened to 0 <= y <= 0.5, trimmed to
    // y <= 0.1 by a loop whose top edge lies inside the parameter box and
    // whose sides lie outside it. A flat cutter at y = 0.4 would cut the
    // untrimmed plane 0.01 mm deep; the trimmed face, all of it at y <= 0.1,
    // stays clear of it. The exact clearance is the smallest distance from
    // the face's nearest line, y = 0.1, to the tool, found here by dense
    // sampling along x (the distance grows away from y = 0.1).
    let u = |x: f64| (x - 0.95) / S;
    let mut f = counterexample_plane(vec![TrimLoop(vec![
        UvPoint { u: -0.01, v: 0.0 },
        UvPoint { u: u(1.06), v: 0.0 },
        UvPoint { u: u(1.06), v: 0.1 },
        UvPoint { u: -0.01, v: 0.1 },
    ])]);
    f.uv_bounds[3] = 0.5;
    let g = Geometry::of_faces(&model(vec![f])).unwrap();
    let t = ConvexTool::new(&flat2()).unwrap();
    let tip = Point3::new(0.0, 0.4, 0.0);
    let exact = (0..=100_000)
        .map(|i| {
            let x = 0.95 + 0.1 * f64::from(i) / 100_000.0;
            flat_clearance(Point3::new(x, 0.1, x - 0.98), tip, 1.0, 8.0)
        })
        .fold(f64::INFINITY, f64::min);
    assert!(exact > 0.015 && exact < 0.025, "{exact}");
    let b = g.clearance(&t, tip, up(), &settings()).unwrap();
    assert!(b.at.is_some(), "the face was dropped: {b:?}");
    assert!(b.lower <= exact + 1e-6, "{b:?} against {exact}");
    assert!(b.upper <= exact + TOL + 1e-6, "{b:?} against {exact}");
    assert!(b.error() <= TOL + 1e-9);
}
