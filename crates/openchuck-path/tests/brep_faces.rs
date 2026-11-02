// SPDX-License-Identifier: AGPL-3.0-or-later
//! The exact B-rep `SurfaceSource` (requirement IMP-02; decision record 0017;
//! work breakdown slice 11): analytic and B-spline faces built by hand,
//! checked against closed forms, brute-force sampling and each other. No
//! kernel is involved.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::cast_precision_loss
)]

use std::f64::consts::{FRAC_PI_2, PI, TAU};

use openchuck_path::brep::SEARCH_TOLERANCE;
use openchuck_path::{BrepSurface, ExactFace, ParametricFace, SurfaceSource, ToolLine, Uv};
use openchuck_types::brep::{
    BSplinePatch, BrepFace, BrepModel, FaceId, Frame, Surface, TrimLoop, UvPoint,
};
use openchuck_types::geometry::{Point3, UnitVec3, Vec3};
use openchuck_types::mesh::TriangleMesh;
use proptest::prelude::*;

// ---- builders -------------------------------------------------------------

fn p3(x: f64, y: f64, z: f64) -> Point3 {
    Point3::new(x, y, z)
}

fn unit(x: f64, y: f64, z: f64) -> UnitVec3 {
    UnitVec3::new(Vec3::new(x, y, z)).unwrap()
}

/// A right-handed frame; `y` is `z` cross `x`.
fn frame(o: Point3, x: Vec3, z: Vec3) -> Frame {
    Frame {
        origin: o,
        x,
        y: z.cross(x),
        z,
    }
}

fn z_frame(o: Point3) -> Frame {
    frame(o, Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.0))
}

fn grid_mesh(f: impl Fn(f64, f64) -> Point3, nu: usize, nv: usize) -> TriangleMesh {
    let mut m = TriangleMesh::default();
    for i in 0..=nu {
        for j in 0..=nv {
            m.vertices
                .push(f(i as f64 / nu as f64, j as f64 / nv as f64));
        }
    }
    let at = |i: usize, j: usize| u32::try_from(i * (nv + 1) + j).unwrap();
    for i in 0..nu {
        for j in 0..nv {
            m.triangles.push([at(i, j), at(i + 1, j), at(i + 1, j + 1)]);
            m.triangles.push([at(i, j), at(i + 1, j + 1), at(i, j + 1)]);
        }
    }
    m
}

fn face(
    id: u32,
    geometry: Surface,
    uv: [f64; 4],
    trims: Vec<Vec<(f64, f64)>>,
    reversed: bool,
    tessellation: TriangleMesh,
) -> BrepFace {
    BrepFace {
        id: FaceId(id),
        surface: geometry.kind(),
        body: 0,
        edges: vec![],
        geometry,
        reversed,
        uv_bounds: uv,
        trims: trims
            .into_iter()
            .map(|l| TrimLoop(l.into_iter().map(|(u, v)| UvPoint { u, v }).collect()))
            .collect(),
        tessellation,
    }
}

fn circle(r: f64, n: usize) -> Vec<(f64, f64)> {
    (0..n)
        .map(|i| {
            let a = TAU * i as f64 / n as f64;
            (r * a.cos(), r * a.sin())
        })
        .collect()
}

fn cylinder_face(id: u32, o: Point3, r: f64, length: f64, reversed: bool) -> BrepFace {
    let tess = grid_mesh(
        |s, t| {
            p3(
                o.x + r * (TAU * s).cos(),
                o.y + r * (TAU * s).sin(),
                o.z + length * t,
            )
        },
        96,
        1,
    );
    face(
        id,
        Surface::Cylinder {
            frame: z_frame(o),
            radius: r,
        },
        [0.0, TAU, 0.0, length],
        vec![],
        reversed,
        tess,
    )
}

fn exact(face: &BrepFace) -> ExactFace {
    ExactFace::new(face).unwrap()
}

/// Brute-force minimum distance over a (u, v) sampling of `face` that lies on
/// the face.
fn sampled_min(face: &ExactFace, q: Point3, n: usize) -> f64 {
    let (lo, hi) = face.uv_bounds();
    let mut best = f64::INFINITY;
    for i in 0..=n {
        for j in 0..=n {
            let uv = Uv {
                u: lo.u + (hi.u - lo.u) * i as f64 / n as f64,
                v: lo.v + (hi.v - lo.v) * j as f64 / n as f64,
            };
            if face.contains(uv)
                && let Some(sp) = face.point_normal(uv)
            {
                best = best.min(sp.point.distance(q));
            }
        }
    }
    best
}

/// A deterministic sequence in [0, 1).
fn lcg(seed: &mut u64) -> f64 {
    *seed = seed
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    (*seed >> 11) as f64 / (1u64 << 53) as f64
}

// ---- B-spline builders ------------------------------------------------------

const W: f64 = std::f64::consts::FRAC_1_SQRT_2;

/// The 9-pole rational circle: unit control points, weights and knots.
fn circle_poles() -> (Vec<f64>, Vec<(f64, f64)>, Vec<f64>) {
    let knots = vec![
        0.0, 0.0, 0.0, 0.25, 0.25, 0.5, 0.5, 0.75, 0.75, 1.0, 1.0, 1.0,
    ];
    let pts = vec![
        (1.0, 0.0),
        (1.0, 1.0),
        (0.0, 1.0),
        (-1.0, 1.0),
        (-1.0, 0.0),
        (-1.0, -1.0),
        (0.0, -1.0),
        (1.0, -1.0),
        (1.0, 0.0),
    ];
    let w = vec![1.0, W, 1.0, W, 1.0, W, 1.0, W, 1.0];
    (knots, pts, w)
}

/// A sphere of radius `r` about `c` as a rational biquadratic patch: u around
/// the axis, v from the south to the north pole. Both poles are singular.
fn sphere_patch(c: Point3, r: f64) -> BSplinePatch {
    let (ku, circ, wu) = circle_poles();
    let meridian = [(0.0, -r), (r, -r), (r, 0.0), (r, r), (0.0, r)];
    let wv = [1.0, W, 1.0, W, 1.0];
    let mut poles = Vec::new();
    let mut weights = Vec::new();
    for (i, (cx, cy)) in circ.iter().enumerate() {
        for (j, (rho, z)) in meridian.iter().enumerate() {
            poles.push(p3(c.x + rho * cx, c.y + rho * cy, c.z + z));
            weights.push(wu[i] * wv[j]);
        }
    }
    BSplinePatch {
        degree_u: 2,
        degree_v: 2,
        knots_u: ku,
        knots_v: vec![0.0, 0.0, 0.0, 0.5, 0.5, 1.0, 1.0, 1.0],
        poles_u: 9,
        poles_v: 5,
        poles,
        weights,
    }
}

/// The unwrapped form of a cylinder about the z axis of radius `r` and height
/// `h`: a rational quadratic circle by a linear segment.
fn cylinder_patch(r: f64, h: f64) -> BSplinePatch {
    let (ku, circ, wu) = circle_poles();
    let mut poles = Vec::new();
    let mut weights = Vec::new();
    for (i, (cx, cy)) in circ.iter().enumerate() {
        for z in [0.0, h] {
            poles.push(p3(r * cx, r * cy, z));
            weights.push(wu[i]);
        }
    }
    BSplinePatch {
        degree_u: 2,
        degree_v: 1,
        knots_u: ku,
        knots_v: vec![0.0, 0.0, 1.0, 1.0],
        poles_u: 9,
        poles_v: 2,
        poles,
        weights,
    }
}

/// A quarter of a cylinder of radius `r` from the +x axis to the +y axis.
fn quarter_cylinder_patch(r: f64, h: f64) -> BSplinePatch {
    let mut poles = Vec::new();
    let mut weights = Vec::new();
    for (p, w) in [((r, 0.0), 1.0), ((r, r), W), ((0.0, r), 1.0)] {
        for z in [0.0, h] {
            poles.push(p3(p.0, p.1, z));
            weights.push(w);
        }
    }
    BSplinePatch {
        degree_u: 2,
        degree_v: 1,
        knots_u: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        knots_v: vec![0.0, 0.0, 1.0, 1.0],
        poles_u: 3,
        poles_v: 2,
        poles,
        weights,
    }
}

/// A biquadratic Bezier patch over a 3 by 3 grid with the given heights.
fn bezier_patch(heights: [[f64; 3]; 3]) -> BSplinePatch {
    let mut poles = Vec::new();
    for (i, row) in heights.iter().enumerate() {
        for (j, h) in row.iter().enumerate() {
            poles.push(p3(5.0 * i as f64, 5.0 * j as f64, *h));
        }
    }
    BSplinePatch {
        degree_u: 2,
        degree_v: 2,
        knots_u: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        knots_v: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        poles_u: 3,
        poles_v: 3,
        poles,
        weights: vec![1.0; 9],
    }
}

fn bezier_point(heights: &[[f64; 3]; 3], u: f64, v: f64) -> Point3 {
    let b = |t: f64| [(1.0 - t).powi(2), 2.0 * t * (1.0 - t), t * t];
    let (bu, bv) = (b(u), b(v));
    let mut p = Vec3::ZERO;
    for i in 0..3 {
        for j in 0..3 {
            let w = bu[i] * bv[j];
            p = p + Vec3::new(5.0 * i as f64, 5.0 * j as f64, heights[i][j]) * w;
        }
    }
    p3(p.x, p.y, p.z)
}

fn patch_face(id: u32, patch: BSplinePatch, sample: impl Fn(f64, f64) -> Point3) -> BrepFace {
    face(
        id,
        Surface::BSpline(patch),
        [0.0, 1.0, 0.0, 1.0],
        vec![],
        false,
        grid_mesh(sample, 24, 24),
    )
}

fn sphere_sample(c: Point3, r: f64) -> impl Fn(f64, f64) -> Point3 {
    move |s, t| {
        let (u, v) = (TAU * s, PI * (t - 0.5));
        p3(
            c.x + r * v.cos() * u.cos(),
            c.y + r * v.cos() * u.sin(),
            c.z + r * v.sin(),
        )
    }
}

// ---- analytic faces -----------------------------------------------------------

#[test]
fn req_imp_02_plane_disc_with_a_hole_misses_through_the_hole() {
    let tess = grid_mesh(|s, t| p3(20.0 * s - 10.0, 20.0 * t - 10.0, 0.0), 8, 8);
    let f = exact(&face(
        0,
        Surface::Plane {
            frame: z_frame(Point3::ORIGIN),
        },
        [-10.0, 10.0, -10.0, 10.0],
        vec![circle(10.0, 96), circle(3.0, 96)],
        false,
        tess,
    ));
    let down = unit(0.0, 0.0, -1.0);
    // Through the hole: no hit. Through the ring: a hit with the face's number.
    assert!(f.ray_hits(p3(0.0, 0.0, 10.0), down, 100.0).is_empty());
    assert!(f.ray_hits(p3(1.0, -2.0, 10.0), down, 100.0).is_empty());
    let hits = f.ray_hits(p3(6.0, 0.0, 10.0), down, 100.0);
    assert_eq!(hits.len(), 1);
    assert!((hits[0].distance - 10.0).abs() < 1e-12);
    assert!(hits[0].normal.get().z > 0.999_999);
    assert_eq!(hits[0].position.unwrap().face, 0);
    // Outside the disc: no hit.
    assert!(f.ray_hits(p3(12.0, 0.0, 10.0), down, 100.0).is_empty());
    // Trim test.
    assert!(f.contains(Uv { u: 6.0, v: 0.0 }));
    assert!(!f.contains(Uv { u: 0.0, v: 0.0 }));
    assert!(!f.contains(Uv { u: 11.0, v: 0.0 }));
    // Closest point over the hole is on the hole's rim, not on the plane.
    let c = f.closest(p3(0.0, 0.0, 5.0)).unwrap();
    assert!(
        (c.distance - (25.0f64 + 9.0).sqrt()).abs() < 5e-3,
        "{}",
        c.distance
    );
    assert!(c.point.point.z.abs() < 1e-12);
    // Closest from above the ring is straight down.
    let c = f.closest(p3(6.0, 0.5, 4.0)).unwrap();
    assert!((c.distance - 4.0).abs() < 1e-12);
    assert!((c.point.point.x - 6.0).abs() < 1e-12);
}

#[test]
fn req_imp_02_cylinder_exact_vs_closed_form() {
    let (r, h) = (12.0, 30.0);
    let f = exact(&cylinder_face(7, p3(1.0, 2.0, 3.0), r, h, false));
    let mut seed = 1;
    let mut worst = 0.0_f64;
    for _ in 0..200 {
        let q = p3(
            -40.0 + 80.0 * lcg(&mut seed),
            -40.0 + 80.0 * lcg(&mut seed),
            3.0 + h * lcg(&mut seed),
        );
        let c = f.closest(q).unwrap();
        let rho = ((q.x - 1.0).hypot(q.y - 2.0) - r).abs();
        worst = worst.max((c.distance - rho).abs());
        // The reported point is on the cylinder and the position reproduces it.
        let back = f.point_normal(c.point.position.unwrap().uv).unwrap();
        assert!(back.point.distance(c.point.point) < 1e-12);
        assert!(
            ((c.point.point.x - 1.0).hypot(c.point.point.y - 2.0) - r).abs() < 1e-12,
            "on the surface"
        );
        assert_eq!(c.point.position.unwrap().face, 7);
    }
    assert!(worst < 1e-12, "closest-point error {worst:e}");
    // Ray along x at height 10 and y = 2: hits at x = 1 -/+ 12.
    let hits = f.ray_hits(p3(-50.0, 2.0, 13.0), unit(1.0, 0.0, 0.0), 200.0);
    assert_eq!(hits.len(), 2);
    assert!((hits[0].point.x + 11.0).abs() < 1e-12 && (hits[1].point.x - 13.0).abs() < 1e-12);
    assert!(hits[0].normal.get().x < -0.999_999 && hits[1].normal.get().x > 0.999_999);
    // A hit across the seam (u = 0 at +x) is found even though the box ends there.
    assert!(
        f.ray_hits(p3(50.0, 2.0, 13.0), unit(-1.0, 0.0, 0.0), 200.0)
            .len()
            == 2
    );
    // Above the top: the closest point is on the rim.
    let c = f.closest(p3(1.0 + 20.0, 2.0, 3.0 + h + 5.0)).unwrap();
    assert!((c.distance - (8.0f64 * 8.0 + 25.0).sqrt()).abs() < 1e-7);
}

#[test]
fn req_imp_02_reversed_face_flips_normals() {
    let o = Point3::ORIGIN;
    let outward = exact(&cylinder_face(0, o, 10.0, 20.0, false));
    let inward = exact(&cylinder_face(0, o, 10.0, 20.0, true));
    let q = p3(30.0, 0.0, 10.0);
    let a = outward.closest(q).unwrap().point.normal.get();
    let b = inward.closest(q).unwrap().point.normal.get();
    assert!(a.x > 0.999_999 && b.x < -0.999_999);
    let ha = outward.ray_hits(q, unit(-1.0, 0.0, 0.0), 100.0);
    let hb = inward.ray_hits(q, unit(-1.0, 0.0, 0.0), 100.0);
    assert!(ha[0].normal.get().x > 0.9 && hb[0].normal.get().x < -0.9);
    let pa = outward.point_normal(Uv { u: 1.0, v: 5.0 }).unwrap();
    let pb = inward.point_normal(Uv { u: 1.0, v: 5.0 }).unwrap();
    assert!((pa.normal.get() + pb.normal.get()).length() < 1e-12);
}

#[test]
fn req_imp_02_mirrored_frame_uses_the_given_y_and_flips_with_reversed() {
    // y = -(z x x): a left-handed frame, as the kernel allows. The surface is
    // the same cylinder, but du x dv now points inward.
    let (x, z) = (Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.0));
    let mirrored = Frame {
        origin: Point3::ORIGIN,
        x,
        y: -z.cross(x),
        z,
    };
    let build = |reversed| {
        exact(&face(
            0,
            Surface::Cylinder {
                frame: mirrored,
                radius: 10.0,
            },
            [0.0, TAU, 0.0, 20.0],
            vec![],
            reversed,
            grid_mesh(
                |s, t| p3(10.0 * (TAU * s).cos(), -10.0 * (TAU * s).sin(), 20.0 * t),
                64,
                1,
            ),
        ))
    };
    let (plain, flipped) = (build(false), build(true));
    // u = pi / 2 is the frame's y, which is the world -Y direction.
    let q = p3(0.0, -30.0, 10.0);
    for (f, expect_out) in [(&plain, false), (&flipped, true)] {
        let c = f.closest(q).unwrap();
        assert!(c.point.point.distance(p3(0.0, -10.0, 10.0)) < 1e-12);
        assert!((c.uv.u - FRAC_PI_2).abs() < 1e-12, "u = {}", c.uv.u);
        let outward = c.point.normal.get().y < 0.0;
        assert_eq!(outward, expect_out);
        let hit = f
            .ray_hits(q, unit(0.0, 1.0, 0.0), 100.0)
            .into_iter()
            .next()
            .unwrap();
        assert_eq!(hit.normal.get().y < 0.0, expect_out);
        let pn = f.point_normal(c.uv).unwrap();
        assert!(pn.point.distance(c.point.point) < 1e-12);
        assert_eq!(pn.normal.get().y < 0.0, expect_out);
    }
}

#[test]
fn req_imp_02_cone_rays_and_closest() {
    let (r, a) = (5.0_f64, 0.3_f64);
    let h = 12.0;
    let tess = grid_mesh(
        |s, t| {
            let v = 10.0 * t;
            let w = r + v * a.sin();
            p3(w * (TAU * s).cos(), w * (TAU * s).sin(), v * a.cos())
        },
        96,
        4,
    );
    let f = exact(&face(
        0,
        Surface::Cone {
            frame: z_frame(Point3::ORIGIN),
            radius: r,
            semi_angle: a,
        },
        [0.0, TAU, 0.0, 10.0],
        vec![],
        false,
        tess,
    ));
    let _ = h;
    let z = 4.0;
    let hit = f
        .ray_hits(p3(20.0, 0.0, z), unit(-1.0, 0.0, 0.0), 100.0)
        .into_iter()
        .next()
        .unwrap();
    let rho = r + z * a.tan();
    assert!((hit.point.x - rho).abs() < 1e-12);
    let expect = Vec3::new(a.cos(), 0.0, -a.sin());
    assert!((hit.normal.get() - expect).length() < 1e-12);
    // Closest from the outside is on the generator; matches sampling.
    let mut seed = 5;
    for _ in 0..50 {
        let q = p3(
            -30.0 + 60.0 * lcg(&mut seed),
            -30.0 + 60.0 * lcg(&mut seed),
            -5.0 + 20.0 * lcg(&mut seed),
        );
        let c = f.closest(q).unwrap();
        let s = sampled_min(&f, q, 400);
        assert!(c.distance <= s + 1e-9, "{} > {}", c.distance, s);
        assert!(
            s - c.distance < 0.05,
            "closest {} sampled {}",
            c.distance,
            s
        );
    }
}

#[test]
fn req_imp_02_sphere_pole_closest_and_ray_through_the_pole() {
    let (c, r) = (p3(1.0, -2.0, 3.0), 20.0);
    let f = exact(&face(
        0,
        Surface::Sphere {
            frame: z_frame(c),
            radius: r,
        },
        [0.0, TAU, -FRAC_PI_2, FRAC_PI_2],
        vec![],
        false,
        grid_mesh(sphere_sample(c, r), 48, 24),
    ));
    // Over the north pole, on the axis, and just off it.
    for q in [
        p3(1.0, -2.0, 60.0),
        p3(1.3, -2.1, 60.0),
        p3(1.0, -2.0, -50.0),
    ] {
        let got = f.closest(q).unwrap();
        let expect = ((q - c).length() - r).abs();
        assert!((got.distance - expect).abs() < 1e-12, "{}", got.distance);
        assert!((got.point.point - c).length() - r < 1e-12);
    }
    let pole = f.closest(p3(1.0, -2.0, 60.0)).unwrap();
    assert!(pole.point.point.distance(p3(1.0, -2.0, 23.0)) < 1e-12);
    assert!(pole.point.normal.get().z > 0.999_999);
    // A ray straight down through both poles.
    let hits = f.ray_hits(p3(1.0, -2.0, 60.0), unit(0.0, 0.0, -1.0), 200.0);
    assert_eq!(hits.len(), 2);
    assert!((hits[0].point.z - 23.0).abs() < 1e-9 && (hits[1].point.z + 17.0).abs() < 1e-9);
    assert!(hits[0].normal.get().z > 0.999_999 && hits[1].normal.get().z < -0.999_999);
}

#[test]
fn req_imp_02_torus_ray_hits_all_four_walls() {
    let (major, minor) = (10.0, 3.0);
    let f = exact(&face(
        0,
        Surface::Torus {
            frame: z_frame(Point3::ORIGIN),
            major,
            minor,
        },
        [0.0, TAU, 0.0, TAU],
        vec![],
        false,
        grid_mesh(
            |s, t| {
                let (u, v) = (TAU * s, TAU * t);
                let w = major + minor * v.cos();
                p3(w * u.cos(), w * u.sin(), minor * v.sin())
            },
            48,
            24,
        ),
    ));
    let hits = f.ray_hits(p3(-100.0, 0.0, 0.0), unit(1.0, 0.0, 0.0), 300.0);
    let xs: Vec<f64> = hits.iter().map(|h| h.point.x).collect();
    assert_eq!(xs.len(), 4, "{xs:?}");
    for (got, want) in xs.iter().zip([-13.0, -7.0, 7.0, 13.0]) {
        assert!((got - want).abs() < 1e-9, "{got} vs {want}");
    }
    // Outer wall faces outwards, inner wall inwards.
    assert!(hits[0].normal.get().x < -0.999 && hits[1].normal.get().x > 0.999);
    // Closest point: radial distance to the tube.
    let q = p3(20.0, 5.0, 9.0);
    let c = f.closest(q).unwrap();
    let rho = q.x.hypot(q.y) - major;
    let expect = rho.hypot(q.z) - minor;
    assert!((c.distance - expect).abs() < 1e-12);
}

// ---- B-spline faces ------------------------------------------------------------

#[test]
fn req_imp_02_bspline_patch_matches_its_bezier_form_and_hits_lie_on_it() {
    let heights = [[0.0, 2.0, 0.0], [1.0, 6.0, 1.0], [0.0, 2.0, 0.0]];
    let f = exact(&patch_face(3, bezier_patch(heights), |s, t| {
        bezier_point(&heights, s, t)
    }));
    for (u, v) in [(0.0, 0.0), (0.3, 0.7), (0.5, 0.5), (1.0, 0.2), (0.9, 1.0)] {
        let sp = f.point_normal(Uv { u, v }).unwrap();
        assert!(sp.point.distance(bezier_point(&heights, u, v)) < 1e-13);
        assert!((sp.normal.get().length() - 1.0).abs() < 1e-12);
    }
    // A vertical ray hits once; the point lies on the patch and the normal points up.
    let hit = f
        .ray_hits(p3(4.0, 6.0, 30.0), unit(0.0, 0.0, -1.0), 100.0)
        .into_iter()
        .next()
        .unwrap();
    let uv = hit.position.unwrap().uv;
    assert!(
        bezier_point(&heights, uv.u, uv.v).distance(hit.point) < 1e-8,
        "hit off the patch"
    );
    assert!((hit.point.x - 4.0).abs() < 1e-8 && (hit.point.y - 6.0).abs() < 1e-8);
    assert!(hit.normal.get().z > 0.5);
    // Closest points are no worse than a dense sampling and not far better.
    let mut seed = 11;
    for _ in 0..40 {
        let q = p3(
            -5.0 + 20.0 * lcg(&mut seed),
            -5.0 + 20.0 * lcg(&mut seed),
            -8.0 + 20.0 * lcg(&mut seed),
        );
        let c = f.closest(q).unwrap();
        let s = sampled_min(&f, q, 200);
        assert!(c.distance <= s + 1e-9, "{} > {}", c.distance, s);
        assert!(s - c.distance < 0.1);
    }
}

#[test]
fn req_imp_02_rational_quarter_cylinder_equals_the_closed_form() {
    let (r, h) = (8.0, 10.0);
    let quarter = exact(&patch_face(0, quarter_cylinder_patch(r, h), |s, t| {
        let a = FRAC_PI_2 * s;
        p3(r * a.cos(), r * a.sin(), h * t)
    }));
    // Surface points are on the cylinder everywhere.
    for i in 0..=20 {
        for j in 0..=5 {
            let sp = quarter
                .point_normal(Uv {
                    u: f64::from(i) / 20.0,
                    v: f64::from(j) / 5.0,
                })
                .unwrap();
            let p = sp.point;
            assert!((p.x.hypot(p.y) - r).abs() < 1e-14);
            // The normal is radial.
            let n = sp.normal.get();
            assert!((n.x * p.y - n.y * p.x).abs() < 1e-12 && n.x * p.x + n.y * p.y > 0.0);
        }
    }
    // Closest points and ray hits against the closed form.
    let mut worst = 0.0_f64;
    let mut seed = 3;
    for _ in 0..100 {
        let a = FRAC_PI_2 * lcg(&mut seed);
        let rho = 1.0 + 30.0 * lcg(&mut seed);
        let q = p3(rho * a.cos(), rho * a.sin(), h * lcg(&mut seed));
        let c = quarter.closest(q).unwrap();
        worst = worst.max((c.distance - (rho - r).abs()).abs());
    }
    assert!(worst < 1e-9, "closest-point error {worst:e}");
    let hit = quarter
        .ray_hits(p3(30.0, 30.0, 5.0), unit(-1.0, -1.0, 0.0), 100.0)
        .into_iter()
        .next()
        .unwrap();
    assert!((hit.point.x.hypot(hit.point.y) - r).abs() < 1e-9);
    assert!((hit.point.x - hit.point.y).abs() < 1e-9);
    // A ray outside the quarter misses.
    assert!(
        quarter
            .ray_hits(p3(-30.0, 30.0, 5.0), unit(1.0, -1.0, 0.0), 100.0)
            .is_empty()
    );
}

#[test]
fn req_imp_02_unwrapped_nurbs_cylinder_equals_the_analytic_cylinder() {
    let (r, h) = (9.0, 16.0);
    let analytic = exact(&cylinder_face(0, Point3::ORIGIN, r, h, false));
    let nurbs = exact(&patch_face(0, cylinder_patch(r, h), |s, t| {
        p3(r * (TAU * s).cos(), r * (TAU * s).sin(), h * t)
    }));
    let mut seed = 21;
    let mut worst = 0.0_f64;
    for _ in 0..100 {
        let q = p3(
            -25.0 + 50.0 * lcg(&mut seed),
            -25.0 + 50.0 * lcg(&mut seed),
            -5.0 + 26.0 * lcg(&mut seed),
        );
        let (a, n) = (analytic.closest(q).unwrap(), nurbs.closest(q).unwrap());
        worst = worst.max((a.distance - n.distance).abs());
        // Same point when it is not on the axis, and the same normal.
        if q.x.hypot(q.y) > 0.5 {
            assert!(
                a.point.point.distance(n.point.point) < 1e-8,
                "q {q:?} a {:?} n {:?}",
                a.point.point,
                n.point.point
            );
            assert!((a.point.normal.get() - n.point.normal.get()).length() < 1e-8);
        }
    }
    assert!(
        worst < 1e-9,
        "analytic and NURBS closest differ by {worst:e}"
    );
    // Rays: same hits.
    for k in 0..20 {
        let a = TAU * f64::from(k) / 20.0 + 0.1;
        let o = p3(40.0 * a.cos(), 40.0 * a.sin(), 8.0);
        let d = unit(-a.cos() + 0.1, -a.sin(), 0.05);
        let (ha, hn) = (analytic.ray_hits(o, d, 200.0), nurbs.ray_hits(o, d, 200.0));
        assert_eq!(ha.len(), hn.len());
        for (x, y) in ha.iter().zip(&hn) {
            assert!((x.distance - y.distance).abs() < 1e-8);
            assert!((x.normal.get() - y.normal.get()).length() < 1e-8);
        }
    }
}

#[test]
fn req_imp_02_bspline_sphere_closest_survives_the_poles() {
    // OI-72: Gauss-Newton from a grid stalled at the poles of a rational
    // sphere, worst 0.48 mm. Damped steps and the pattern search must not.
    let (c, r) = (p3(2.0, -1.0, 4.0), 20.0);
    let f = exact(&patch_face(0, sphere_patch(c, r), sphere_sample(c, r)));
    let mut worst = 0.0_f64;
    let mut cases = vec![
        p3(2.0, -1.0, 60.0),
        p3(2.0, -1.0, -60.0),
        p3(2.0, -1.0, 4.0),
        p3(2.3, -1.0, 40.0),
        p3(2.0, -0.8, -35.0),
        p3(2.01, -1.01, 24.5),
        p3(1.9, -1.2, 23.9),
    ];
    // Many points near either pole, at assorted angles and heights.
    let mut seed = 99;
    for _ in 0..300 {
        let a = TAU * lcg(&mut seed);
        let off = 3.0 * lcg(&mut seed);
        let z = if lcg(&mut seed) < 0.5 { 1.0 } else { -1.0 };
        cases.push(p3(
            c.x + off * a.cos(),
            c.y + off * a.sin(),
            c.z + z * (10.0 + 40.0 * lcg(&mut seed)),
        ));
    }
    for q in cases {
        let got = f.closest(q).unwrap();
        let expect = ((q - c).length() - r).abs();
        worst = worst.max((got.distance - expect).abs());
        assert!(
            (got.point.point - c).length() - r < 1e-9,
            "reported point off the sphere"
        );
    }
    assert!(worst < 1e-9, "worst pole error {worst:e} mm");
    // The normal at the pole comes from the limit, not a zero cross product.
    let n = f
        .point_normal(Uv { u: 0.3, v: 1.0 })
        .map(|s| s.normal.get());
    assert!(n.is_some_and(|n| n.z > 0.999_99));
    // A ray through the pole, either way.
    for dir in [-1.0, 1.0] {
        let o = p3(c.x, c.y, c.z - dir * 60.0);
        let hits = f.ray_hits(o, unit(0.0, 0.0, dir), 200.0);
        assert_eq!(hits.len(), 2, "{hits:?}");
        assert!((hits[0].distance - 40.0).abs() < 1e-6);
        assert!((hits[1].distance - 80.0).abs() < 1e-6);
    }
}

#[test]
fn req_imp_02_bspline_sphere_accuracy_numbers() {
    let (c, r) = (p3(0.0, 0.0, 0.0), 25.0);
    let f = exact(&patch_face(0, sphere_patch(c, r), sphere_sample(c, r)));
    let mut seed = 7;
    let (mut worst_d, mut worst_ray) = (0.0_f64, 0.0_f64);
    for _ in 0..500 {
        let q = p3(
            -60.0 + 120.0 * lcg(&mut seed),
            -60.0 + 120.0 * lcg(&mut seed),
            -60.0 + 120.0 * lcg(&mut seed),
        );
        let got = f.closest(q).unwrap();
        worst_d = worst_d.max((got.distance - (q.to_vec().length() - r).abs()).abs());
    }
    for _ in 0..300 {
        let a = TAU * lcg(&mut seed);
        let b = PI * (lcg(&mut seed) - 0.5);
        let target = p3(
            r * 0.9 * b.cos() * a.cos(),
            r * 0.9 * b.cos() * a.sin(),
            r * 0.9 * b.sin(),
        );
        let o = p3(80.0, 10.0, -20.0);
        let d = unit(target.x - o.x, target.y - o.y, target.z - o.z);
        for h in f.ray_hits(o, d, 400.0) {
            worst_ray = worst_ray.max((h.point.to_vec().length() - r).abs());
        }
    }
    eprintln!("sphere patch: closest error {worst_d:e} mm, ray-hit radius error {worst_ray:e} mm");
    assert!(worst_d < 1e-9 && worst_ray < 1e-9);
}

// ---- the source ------------------------------------------------------------------

fn capped_cylinder() -> BrepModel {
    let (r, h) = (10.0, 20.0);
    let side = cylinder_face(0, Point3::ORIGIN, r, h, false);
    let cap = |id: u32, z: f64, down: bool| {
        // The bottom cap faces -z: its frame is flipped about x.
        let fr = if down {
            frame(
                p3(0.0, 0.0, z),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.0, 0.0, -1.0),
            )
        } else {
            z_frame(p3(0.0, 0.0, z))
        };
        let mut tess = TriangleMesh::default();
        tess.vertices.push(p3(0.0, 0.0, z));
        for (x, y) in circle(r, 48) {
            tess.vertices.push(p3(x, y, z));
        }
        for i in 0..48u32 {
            let (a, b) = (1 + i, 1 + (i + 1) % 48);
            tess.triangles
                .push(if down { [0, b, a] } else { [0, a, b] });
        }
        face(
            id,
            Surface::Plane { frame: fr },
            [-r, r, -r, r],
            vec![circle(r, 96)],
            false,
            tess,
        )
    };
    BrepModel {
        faces: vec![side, cap(1, 0.0, true), cap(2, h, false)],
        ..BrepModel::default()
    }
}

#[test]
fn req_imp_02_source_project_and_cast_ray_report_the_face() {
    let model = capped_cylinder();
    let s = BrepSurface::new(&model).unwrap();
    let top = s.project(p3(5.0, 0.0, 30.0)).unwrap();
    assert_eq!(top.position.unwrap().face, 2);
    assert!(top.point.distance(p3(5.0, 0.0, 20.0)) < 1e-9 && top.normal.get().z > 0.999_999);
    let bottom = s.project(p3(2.0, 1.0, -9.0)).unwrap();
    assert_eq!(bottom.position.unwrap().face, 1);
    assert!(bottom.normal.get().z < -0.999_999);
    let side = s.project(p3(30.0, 0.0, 10.0)).unwrap();
    assert_eq!(side.position.unwrap().face, 0);
    assert!(side.point.distance(p3(10.0, 0.0, 10.0)) < 1e-9);
    assert!((s.distance(p3(30.0, 0.0, 10.0)).unwrap() - 20.0).abs() < 1e-9);
    // Rays: from above onto the top cap, from the side onto the wall, and
    // from the side below the part onto nothing.
    let h = s
        .cast_ray(p3(1.0, 1.0, 50.0), unit(0.0, 0.0, -1.0), 100.0)
        .unwrap();
    assert_eq!(h.position.unwrap().face, 2);
    assert!((h.distance - 30.0).abs() < 1e-12);
    let h = s
        .cast_ray(p3(-50.0, 0.0, 10.0), unit(1.0, 0.0, 0.0), 100.0)
        .unwrap();
    assert_eq!(h.position.unwrap().face, 0);
    assert!((h.point.x + 10.0).abs() < 1e-12 && h.normal.get().x < -0.999_999);
    assert!(
        s.cast_ray(p3(-50.0, 0.0, -5.0), unit(1.0, 0.0, 0.0), 100.0)
            .is_none()
    );
    assert!(
        s.cast_ray(p3(-50.0, 0.0, 10.0), unit(1.0, 0.0, 0.0), 30.0)
            .is_none()
    );
    // Up through the bottom cap first.
    let h = s
        .cast_ray(p3(0.0, 0.0, -50.0), unit(0.0, 0.0, 1.0), 100.0)
        .unwrap();
    assert_eq!(h.position.unwrap().face, 1);
    assert!(h.normal.get().z < -0.999_999);
    // Bounds hold the part; the tessellation is the faces joined.
    let b = s.bounds();
    assert!(b.contains(p3(10.0, 10.0, 0.0)) && b.contains(p3(-10.0, -10.0, 20.0)));
    let (joined, owner) = model.joined_tessellation();
    assert_eq!(s.tessellation(), &joined);
    assert_eq!(s.face_of_triangle(), owner.as_slice());
}

#[test]
fn req_imp_02_source_restricted_to_selected_faces() {
    let model = capped_cylinder();
    let s = BrepSurface::with_faces(&model, &[FaceId(0)]).unwrap();
    assert_eq!(s.faces().len(), 1);
    // Above the top: the only face left is the wall, whose rim is closest.
    let p = s.project(p3(5.0, 0.0, 30.0)).unwrap();
    assert_eq!(p.position.unwrap().face, 0);
    assert!(
        p.point.distance(p3(10.0, 0.0, 20.0)) < 1e-6,
        "{:?}",
        p.point
    );
    // A ray down the axis misses the wall.
    assert!(
        s.cast_ray(p3(0.0, 0.0, 50.0), unit(0.0, 0.0, -1.0), 100.0)
            .is_none()
    );
    assert_eq!(s.tessellation().triangles.len(), 96 * 2);
    assert!(BrepSurface::with_faces(&model, &[FaceId(9)]).is_err());
    assert!(BrepSurface::with_faces(&model, &[]).is_err());
}

#[test]
fn req_imp_02_source_drop_tool_falls_back_to_the_tessellation() {
    use openchuck_types::tool::{Tool, ToolShape};
    use openchuck_types::units::Millimetres;
    let s = BrepSurface::new(&capped_cylinder()).unwrap();
    let tool = Tool {
        shape: ToolShape::BallEnd,
        diameter: Millimetres(6.0),
        flute_length: Millimetres(12.0),
        overall_length: Millimetres(50.0),
        shank_diameter: Millimetres(6.0),
        flute_count: 2,
        material: None,
        centre_cutting: None,
    }
    .profile()
    .unwrap();
    let line = ToolLine {
        origin: p3(2.0, 1.0, 0.0),
        axis: UnitVec3::Z,
    };
    let c = s.drop_tool(&tool, &line).unwrap();
    assert!((c.tip.z - 20.0).abs() < 1e-9);
}

#[test]
fn req_imp_02_invalid_faces_are_errors_not_panics() {
    let good = cylinder_face(4, Point3::ORIGIN, 5.0, 5.0, false);
    let mut f = good.clone();
    f.geometry = Surface::Cylinder {
        frame: z_frame(Point3::ORIGIN),
        radius: f64::NAN,
    };
    assert!(ExactFace::new(&f).is_err());
    let mut f = good.clone();
    f.uv_bounds = [1.0, 0.0, 0.0, 1.0];
    assert!(ExactFace::new(&f).is_err());
    let mut f = good.clone();
    f.trims = vec![TrimLoop(vec![UvPoint { u: 0.0, v: 0.0 }])];
    assert!(ExactFace::new(&f).is_err());
    let mut f = good.clone();
    f.tessellation.triangles.push([0, 1, 9999]);
    assert!(ExactFace::new(&f).is_err());
    let mut f = good;
    f.geometry = Surface::Cylinder {
        frame: Frame {
            origin: Point3::ORIGIN,
            x: Vec3::new(2.0, 0.0, 0.0),
            y: Vec3::new(0.0, 1.0, 0.0),
            z: Vec3::new(0.0, 0.0, 1.0),
        },
        radius: 5.0,
    };
    assert!(ExactFace::new(&f).is_err());
    // A B-spline with unclamped knots or a missing pole is refused.
    let mut p = bezier_patch([[0.0; 3]; 3]);
    p.knots_u = vec![0.0, 0.1, 0.2, 0.8, 0.9, 1.0];
    assert!(ExactFace::new(&patch_face(1, p, |s, t| p3(s, t, 0.0))).is_err());
    let mut p = bezier_patch([[0.0; 3]; 3]);
    p.poles.pop();
    assert!(ExactFace::new(&patch_face(1, p, |s, t| p3(s, t, 0.0))).is_err());
    // An empty model is an error.
    assert!(BrepSurface::new(&BrepModel::default()).is_err());
    // Non-finite queries give no answer.
    let e = exact(&cylinder_face(0, Point3::ORIGIN, 5.0, 5.0, false));
    assert!(e.closest(p3(f64::NAN, 0.0, 0.0)).is_none());
    assert!(
        e.point_normal(Uv {
            u: f64::NAN,
            v: 0.0
        })
        .is_none()
    );
    assert!(!e.contains(Uv {
        u: 0.0,
        v: f64::INFINITY
    }));
}

#[test]
fn req_imp_02_answers_are_deterministic() {
    let model = capped_cylinder();
    let s = BrepSurface::new(&model).unwrap();
    let sphere = exact(&patch_face(
        0,
        sphere_patch(Point3::ORIGIN, 10.0),
        sphere_sample(Point3::ORIGIN, 10.0),
    ));
    let q = p3(3.3, -4.1, 22.7);
    let (a, b) = (s.project(q), s.project(q));
    assert_eq!(a, b);
    let o = p3(-40.0, 1.0, 9.0);
    let d = unit(1.0, 0.01, 0.0);
    assert_eq!(s.cast_ray(o, d, 100.0), s.cast_ray(o, d, 100.0));
    assert_eq!(
        sphere.closest(q).map(|c| (c.distance, c.uv)),
        sphere.closest(q).map(|c| (c.distance, c.uv))
    );
    assert_eq!(sphere.ray_hits(o, d, 100.0), sphere.ray_hits(o, d, 100.0));
}

#[test]
fn req_imp_02_closest_beside_a_closed_bspline_seam_is_within_the_search_tolerance() {
    // Issue #467: the sphere patch's u = 0 and u = 1 edges meet at +X, and a
    // B-spline face carries no period, so a point 0.0027 mm off that seam is
    // answered on the u = 0 edge, 9e-8 mm further than the sphere's own
    // nearest point: within the search's tolerance (decision record 0260),
    // never further. On the seam itself, and off it, the answer is exact.
    let (c, r) = (p3(1.0, 2.0, -3.0), 18.0);
    let sphere = exact(&patch_face(0, sphere_patch(c, r), sphere_sample(c, r)));
    for qy in [1.997_284_164_910_329_7, 2.0, 1.99, 2.01] {
        let q = p3(38.568_016_006_474_1, qy, 0.0);
        let want = ((q - c).length() - r).abs();
        let got = sphere.closest(q).unwrap();
        assert!(
            (got.distance - want).abs() <= SEARCH_TOLERANCE,
            "qy {qy}: {} against {want}",
            got.distance
        );
        assert!(
            got.distance >= want - 1e-12,
            "qy {qy}: nearer than the sphere"
        );
        assert!(
            (got.point.point.distance(q) - got.distance).abs() < 1e-9,
            "qy {qy}: the point is not at the distance given"
        );
    }
}

// ---- properties ---------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn req_imp_02_prop_closest_is_no_further_than_any_sampled_point(
        qx in -60.0..60.0f64, qy in -60.0..60.0f64, qz in -60.0..60.0f64,
    ) {
        let (c, r) = (p3(1.0, 2.0, -3.0), 18.0);
        let sphere = exact(&patch_face(0, sphere_patch(c, r), sphere_sample(c, r)));
        let torus = exact(&face(
            1,
            Surface::Torus { frame: z_frame(Point3::ORIGIN), major: 12.0, minor: 4.0 },
            [0.0, TAU, 0.0, TAU],
            vec![],
            false,
            grid_mesh(|s, t| {
                let (u, v) = (TAU * s, TAU * t);
                let w = 12.0 + 4.0 * v.cos();
                p3(w * u.cos(), w * u.sin(), 4.0 * v.sin())
            }, 48, 24),
        ));
        let q = p3(qx, qy, qz);
        for f in [&sphere, &torus] {
            let got = f.closest(q).unwrap().distance;
            prop_assert!(got <= sampled_min(f, q, 60) + 1e-9);
        }
        // The closed form for the sphere, within the search's tolerance:
        // the patch closes on itself at +X without a period, so beside that
        // seam the answer may lie on the patch's edge (issue #467).
        let got = sphere.closest(q).unwrap().distance;
        prop_assert!((got - ((q - c).length() - r).abs()).abs() <= SEARCH_TOLERANCE);
    }

    #[test]
    fn req_imp_02_prop_ray_hits_lie_on_the_surface(
        ox in -50.0..50.0f64, oy in -50.0..50.0f64, oz in -50.0..50.0f64,
        tx in -8.0..8.0f64, ty in -8.0..8.0f64, tz in -8.0..8.0f64,
    ) {
        let (c, r) = (p3(0.0, 0.0, 0.0), 15.0);
        let patch = exact(&patch_face(0, sphere_patch(c, r), sphere_sample(c, r)));
        let torus = exact(&face(
            1,
            Surface::Torus { frame: z_frame(Point3::ORIGIN), major: 12.0, minor: 4.0 },
            [0.0, TAU, 0.0, TAU],
            vec![],
            false,
            grid_mesh(|s, t| {
                let (u, v) = (TAU * s, TAU * t);
                let w = 12.0 + 4.0 * v.cos();
                p3(w * u.cos(), w * u.sin(), 4.0 * v.sin())
            }, 48, 24),
        ));
        let d = Vec3::new(tx - ox, ty - oy, tz - oz);
        prop_assume!(d.length() > 1e-3);
        let dir = UnitVec3::new(d).unwrap();
        let o = p3(ox, oy, oz);
        for h in patch.ray_hits(o, dir, 400.0) {
            prop_assert!(((h.point - c).length() - r).abs() < 1e-8);
            prop_assert!(((h.point - o).length() - h.distance).abs() < 1e-7);
        }
        for h in torus.ray_hits(o, dir, 400.0) {
            let rho = h.point.x.hypot(h.point.y) - 12.0;
            prop_assert!((rho.hypot(h.point.z) - 4.0).abs() < 1e-7);
            prop_assert!(((h.point - o).length() - h.distance).abs() < 1e-7);
        }
    }
}
