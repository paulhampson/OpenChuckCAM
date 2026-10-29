// SPDX-License-Identifier: AGPL-3.0-or-later
//! The exact face and trim queries of lane STEPALL-X1 (issue #348; decision
//! 0211 item 3; OI-839, OI-81; requirement IMP-02, NFR-05, NFR-06, NFR-07):
//! the trim test and the boundary search on long trim loops, checked against
//! independent brute-force answers written here. No kernel is involved.
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
    BrepSurface, ExactFace, FacePosition, ParametricFace, SurfaceSource, ToolLine, Uv,
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

/// A star-shaped outline of `n` points about (cx, cy) with a wavy radius:
/// thousands of short segments, as a blade outline on the blisk's planes.
fn star(cx: f64, cy: f64, r: f64, wave: f64, n: usize, phase: f64) -> Vec<(f64, f64)> {
    (0..n)
        .map(|i| {
            let a = TAU * i as f64 / n as f64;
            let rr = r * (1.0 + wave * (7.0 * a + phase).sin());
            (cx + rr * a.cos(), cy + rr * a.sin())
        })
        .collect()
}

/// The even-odd rule over every segment of every loop, written out.
fn reference_inside(loops: &[Vec<(f64, f64)>], u: f64, v: f64) -> bool {
    let mut odd = false;
    for l in loops {
        for i in 0..l.len() {
            let (x0, y0) = l[(i + l.len() - 1) % l.len()];
            let (x1, y1) = l[i];
            if (y1 > v) != (y0 > v) && u < (x0 - x1) * (v - y1) / (y0 - y1) + x1 {
                odd = !odd;
            }
        }
    }
    odd
}

/// The distance from `q` to the segment `a b` in 3D.
fn segment_distance(q: Point3, a: Point3, b: Point3) -> f64 {
    let d = b - a;
    let t = ((q - a).dot(d) / d.dot(d)).clamp(0.0, 1.0);
    q.distance(a + d * t)
}

/// A plane face (z = 0, u = x, v = y) trimmed to a wavy outer outline with
/// two wavy holes, 6,000 segments in all.
fn holed_plane() -> (ExactFace, Vec<Vec<(f64, f64)>>) {
    let mut hole_a = star(-12.0, 0.0, 6.0, 0.1, 1000, 0.3);
    hole_a.reverse();
    let mut hole_b = star(13.0, 2.0, 5.0, 0.15, 1000, 1.1);
    hole_b.reverse();
    let loops = vec![star(0.0, 0.0, 30.0, 0.05, 4000, 0.0), hole_a, hole_b];
    let f = face(
        Surface::Plane { frame: z_frame() },
        [-40.0, 40.0, -40.0, 40.0],
        &loops,
        one_triangle(p3(0.0, 0.0, 0.0), p3(1.0, 0.0, 0.0), p3(0.0, 1.0, 0.0)),
    );
    (ExactFace::new(&f).unwrap(), loops)
}

// ---- the trim test --------------------------------------------------------

#[test]
fn req_imp_02_indexed_trim_test_matches_the_even_odd_rule() {
    let (f, loops) = holed_plane();
    let mut seed = 11;
    let mut inside = 0;
    for _ in 0..20_000 {
        let (u, v) = (-45.0 + 90.0 * lcg(&mut seed), -45.0 + 90.0 * lcg(&mut seed));
        let want = reference_inside(&loops, u, v);
        assert_eq!(f.contains(Uv { u, v }), want, "at ({u}, {v})");
        inside += usize::from(want);
    }
    // Both sides were exercised.
    assert!(inside > 4_000 && inside < 16_000, "{inside}");
    // Points on the loop vertices themselves and on their v exactly: the
    // strip edges and the half-open crossing rule.
    for l in &loops {
        for &(u, v) in l.iter().step_by(37) {
            for (du, dv) in [(0.0, 0.0), (1e-9, 0.0), (-1e-9, 0.0), (0.0, 1e-12)] {
                let (u, v) = (u + du, v + dv);
                assert_eq!(
                    f.contains(Uv { u, v }),
                    reference_inside(&loops, u, v),
                    "at ({u}, {v})"
                );
            }
        }
    }
    // Outside the loops' v range, and not a number.
    assert!(!f.contains(Uv { u: 0.0, v: 39.0 }));
    assert!(!f.contains(Uv {
        u: 0.0,
        v: f64::NAN
    }));
}

// ---- the boundary search --------------------------------------------------

#[test]
fn req_imp_02_boundary_search_on_long_loops_matches_brute_force() {
    let (f, loops) = holed_plane();
    let on_plane = |(u, v): (f64, f64)| p3(u, v, 0.0);
    let mut seed = 5;
    for k in 0..300 {
        // Points over the holes and outside the outline, at a height, so the
        // answer is on a loop.
        let (u, v) = match k % 3 {
            0 => (-12.0 + 4.0 * lcg(&mut seed), -2.0 + 4.0 * lcg(&mut seed)),
            1 => (13.0 + 3.0 * lcg(&mut seed), 1.0 + 3.0 * lcg(&mut seed)),
            _ => {
                let a = TAU * lcg(&mut seed);
                (34.0 * a.cos(), 34.0 * a.sin())
            }
        };
        let q = p3(u, v, 0.5 + 2.0 * lcg(&mut seed));
        assert!(!reference_inside(&loops, u, v));
        let want = loops
            .iter()
            .flat_map(|l| {
                (0..l.len()).map(move |i| {
                    segment_distance(q, on_plane(l[(i + l.len() - 1) % l.len()]), on_plane(l[i]))
                })
            })
            .fold(f64::INFINITY, f64::min);
        let got = f.closest(q).unwrap();
        assert!(
            (got.distance - want).abs() < 1e-9,
            "{k}: {} against {want}",
            got.distance
        );
        assert!((got.point.point.distance(q) - got.distance).abs() < 1e-9);
    }
}

// ---- OI-839: a point on the face's own boundary -----------------------------

/// The corpus cone's large-end face as the importer gives it
/// (`testdata/step/valid/cone.step`, face 0): radius 12 at v = 0, the axis
/// along -z, a frame whose y is -y, and a trim loop that is the parameter
/// box itself, so its whole rim is one segment from u = 2 pi back to u = 0.
fn corpus_cone() -> ExactFace {
    let (r, a) = (12.0_f64, 0.260_602_391_747_f64);
    let (u0, v0) = (-4.138_911_435_802_583_6e-13, -31.048_349_392_520_052);
    let loops = vec![vec![
        (0.0, -31.048_349_392_520_05),
        (TAU, v0),
        (TAU, 0.0),
        (u0, 0.0),
    ]];
    let frame = Frame {
        origin: Point3::ORIGIN,
        x: Vec3::new(1.0, 0.0, 0.0),
        y: Vec3::new(0.0, -1.0, 0.0),
        z: Vec3::new(0.0, 0.0, -1.0),
    };
    let f = face(
        Surface::Cone {
            frame,
            radius: r,
            semi_angle: a,
        },
        [u0, TAU, v0, 0.0],
        &loops,
        one_triangle(p3(r, 0.0, 0.0), p3(0.0, r, 0.0), p3(0.0, 0.0, 30.0)),
    );
    ExactFace::new(&f).unwrap()
}

#[test]
fn req_imp_02_closest_of_a_point_on_the_cone_rim_is_itself() {
    let f = corpus_cone();
    let mut worst = (0.0_f64, 0.0_f64);
    // The rim's tessellation vertices: on the circle z = 0 of radius 12, in
    // the frame's u, which runs from +x towards -y.
    for k in 0..1112 {
        let u = TAU * k as f64 / 1112.0;
        let q = p3(12.0 * u.cos(), -12.0 * u.sin(), 0.0);
        let c = f.closest(q).unwrap();
        if c.distance > worst.0 {
            worst = (c.distance, u);
        }
    }
    assert!(
        worst.0 < 1e-9,
        "a rim point is answered {} mm away (u = {})",
        worst.0,
        worst.1
    );
}

// ---- every kind, trimmed, against brute force --------------------------------

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

/// The nearest of a dense sampling of the trimmed face and of its loops:
/// no nearer than the true nearest point.
fn sampled(f: &ExactFace, loops: &[Vec<(f64, f64)>], q: Point3) -> f64 {
    let (lo, hi) = f.uv_bounds();
    let mut best = f64::INFINITY;
    let n = 200;
    for i in 0..=n {
        for j in 0..=n {
            let uv = Uv {
                u: lo.u + (hi.u - lo.u) * i as f64 / n as f64,
                v: lo.v + (hi.v - lo.v) * j as f64 / n as f64,
            };
            if f.contains(uv)
                && let Some(sp) = f.point_normal(uv)
            {
                best = best.min(sp.point.distance(q));
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
                    best = best.min(sp.point.distance(q));
                }
            }
        }
    }
    best
}

#[test]
fn req_imp_02_trimmed_closest_of_every_kind_is_no_further_than_brute_force() {
    let mut seed = 23;
    for (name, f, loops) in kinds() {
        let b = f.bounding_box();
        let (c, size) = (
            p3(
                0.5 * (b.min.x + b.max.x),
                0.5 * (b.min.y + b.max.y),
                0.5 * (b.min.z + b.max.z),
            ),
            (b.max - b.min).length(),
        );
        for k in 0..25 {
            let q = p3(
                c.x + size * (lcg(&mut seed) - 0.5),
                c.y + size * (lcg(&mut seed) - 0.5),
                c.z + size * (lcg(&mut seed) - 0.5),
            );
            let got = f.closest(q).unwrap();
            let want = sampled(&f, &loops, q);
            // The answer is a point of the face at the distance it gives...
            assert!(
                (got.point.point.distance(q) - got.distance).abs() < 1e-9,
                "{name} {k}"
            );
            assert!(
                f.contains(got.uv) || on_loops(&f, &loops, got.point.point),
                "{name} {k}: off the face"
            );
            // ...and no sampled point beats it.
            assert!(
                got.distance <= want + 1e-7,
                "{name} {k}: {} but a sample is at {want}",
                got.distance
            );
            // Limited and seeded searches agree.
            let within = f.closest_within(q, got.distance + 1e-9).unwrap();
            assert!((within.distance - got.distance).abs() < 2e-6, "{name} {k}");
            assert!(
                f.closest_within(q, got.distance - 1e-3)
                    .is_none_or(|c| c.distance <= got.distance - 1e-3)
            );
            let seeded = f
                .closest_from(q, Uv { u: 0.5, v: 0.5 }, f64::INFINITY)
                .unwrap();
            assert!((seeded.distance - got.distance).abs() < 2e-6, "{name} {k}");
        }
    }
}

/// Whether `p` lies on the image of the loops (within a hair).
fn on_loops(f: &ExactFace, loops: &[Vec<(f64, f64)>], p: Point3) -> bool {
    let mut best = f64::INFINITY;
    for l in loops {
        for k in 0..l.len() {
            let (a, b) = (l[k], l[(k + 1) % l.len()]);
            for s in 0..=2000 {
                let t = f64::from(s) / 2000.0;
                if let Some(sp) = f.point_normal(Uv {
                    u: a.0 + (b.0 - a.0) * t,
                    v: a.1 + (b.1 - a.1) * t,
                }) {
                    best = best.min(sp.point.distance(p));
                }
            }
            if best < 1e-3 {
                return true;
            }
        }
    }
    false
}

#[test]
fn req_nfr_07_queries_refuse_invalid_points_and_limits() {
    for (_, f, _) in kinds() {
        let q = p3(f64::NAN, 0.0, 0.0);
        assert!(f.closest(q).is_none());
        assert!(f.closest_within(p3(0.0, 0.0, 0.0), f64::NAN).is_none());
        assert!(f.closest_within(p3(0.0, 0.0, 0.0), -1.0).is_none());
        assert!(
            f.closest_from(
                p3(0.0, 0.0, 0.0),
                Uv {
                    u: f64::NAN,
                    v: 0.0
                },
                f64::INFINITY
            )
            .is_some()
        );
    }
}

#[test]
fn req_nfr_06_queries_are_deterministic() {
    let a: Vec<f64> = kinds()
        .iter()
        .flat_map(|(_, f, _)| {
            (0..20).map(move |k| {
                f.closest(p3(f64::from(k) - 7.0, 3.0, 2.0))
                    .unwrap()
                    .distance
            })
        })
        .collect();
    let b: Vec<f64> = kinds()
        .iter()
        .flat_map(|(_, f, _)| {
            (0..20).map(move |k| {
                f.closest(p3(f64::from(k) - 7.0, 3.0, 2.0))
                    .unwrap()
                    .distance
            })
        })
        .collect();
    assert_eq!(a, b);
}

// ---- the faces within the tool's reach ---------------------------------------

/// Every kind of face, each shifted along x by 60 mm from the last, so a tool
/// line reaches some and not others; ids 0 to 5.
fn spread_model() -> BrepModel {
    let mut faces = Vec::new();
    for (k, (_, mut f, _)) in kind_faces().into_iter().enumerate() {
        let dx = 60.0 * k as f64;
        let shift = |p: Point3| p3(p.x + dx, p.y, p.z);
        f.geometry = match f.geometry {
            Surface::BSpline(mut b) => {
                for p in &mut b.poles {
                    *p = shift(*p);
                }
                Surface::BSpline(b)
            }
            Surface::Plane { mut frame } => {
                frame.origin = shift(frame.origin);
                Surface::Plane { frame }
            }
            Surface::Cylinder { mut frame, radius } => {
                frame.origin = shift(frame.origin);
                Surface::Cylinder { frame, radius }
            }
            Surface::Cone {
                mut frame,
                radius,
                semi_angle,
            } => {
                frame.origin = shift(frame.origin);
                Surface::Cone {
                    frame,
                    radius,
                    semi_angle,
                }
            }
            Surface::Sphere { mut frame, radius } => {
                frame.origin = shift(frame.origin);
                Surface::Sphere { frame, radius }
            }
            Surface::Torus {
                mut frame,
                major,
                minor,
            } => {
                frame.origin = shift(frame.origin);
                Surface::Torus {
                    frame,
                    major,
                    minor,
                }
            }
            other => other,
        };
        for v in &mut f.tessellation.vertices {
            *v = shift(*v);
        }
        f.id = FaceId(u32::try_from(k).unwrap());
        faces.push(f);
    }
    BrepModel {
        faces,
        ..BrepModel::default()
    }
}

fn ball(radius: f64) -> RevolvedProfile {
    ToolProfile {
        segments: vec![
            ProfileSegment::Arc {
                to: ProfilePoint::new(radius, radius),
                centre: ProfilePoint::new(0.0, radius),
            },
            ProfileSegment::Line {
                to: ProfilePoint::new(radius, 4.0 * radius),
            },
        ],
    }
    .validate()
    .unwrap()
}

/// The distance from `p` to the line through `o` along the unit `d`.
fn line_distance(p: Point3, o: Point3, d: Vec3) -> f64 {
    let w = p - o;
    (w - d * w.dot(d)).length()
}

#[test]
fn req_imp_02_faces_in_reach_keep_every_face_the_tool_can_touch() {
    let model = spread_model();
    let source = BrepSurface::new(&model).unwrap();
    let mut seed = 31;
    let (mut kept, mut asked) = (0, 0);
    for k in 0..60 {
        let radius = 0.5 + 6.0 * lcg(&mut seed);
        let profile = ball(radius);
        let d = Vec3::new(
            lcg(&mut seed) - 0.5,
            lcg(&mut seed) - 0.5,
            lcg(&mut seed) - 0.5,
        );
        let d = d * (1.0 / d.length());
        let o = p3(
            300.0 * lcg(&mut seed) - 10.0,
            30.0 * (lcg(&mut seed) - 0.5),
            30.0 * (lcg(&mut seed) - 0.5),
        );
        let line = ToolLine {
            origin: o,
            axis: UnitVec3::new(d).unwrap(),
        };
        let listed = source.faces_in_reach(&profile, &line);
        for (i, f) in source.faces().iter().enumerate() {
            // Brute force: the nearest sampled point of the trimmed face.
            let (lo, hi) = f.uv_bounds();
            let mut near = f64::INFINITY;
            for a in 0..=120 {
                for b in 0..=120 {
                    let uv = Uv {
                        u: lo.u + (hi.u - lo.u) * f64::from(a) / 120.0,
                        v: lo.v + (hi.v - lo.v) * f64::from(b) / 120.0,
                    };
                    if f.contains(uv)
                        && let Some(sp) = f.point_normal(uv)
                    {
                        near = near.min(line_distance(sp.point, o, d));
                    }
                }
            }
            if near <= radius {
                assert!(
                    listed.contains(&i),
                    "line {k}: face {i} is {near} from the line, within {radius}"
                );
            }
        }
        kept += listed.len();
        asked += source.faces().len();
    }
    // The bound leaves faces out: it is not the whole model every time.
    assert!(kept < asked / 2, "{kept} of {asked}");
}

#[test]
fn req_imp_02_faces_near_a_point_and_limited_projection() {
    let model = spread_model();
    let source = BrepSurface::new(&model).unwrap();
    let mut seed = 41;
    for _ in 0..60 {
        let q = p3(
            320.0 * lcg(&mut seed) - 10.0,
            30.0 * (lcg(&mut seed) - 0.5),
            30.0 * (lcg(&mut seed) - 0.5),
        );
        let all = source.project(q).unwrap();
        let d = all.point.distance(q);
        let face = all.position.unwrap().face;
        // The nearest face is among those near enough.
        let near = source.faces_near(q, d + 1e-9);
        assert!(near.iter().any(|&i| source.faces()[i].number() == face));
        // A limit at or above the answer finds it; one below finds nothing.
        let within = source.project_within(q, d + 1e-9).unwrap();
        assert!((within.point.distance(q) - d).abs() < 2e-6);
        assert!(source.project_within(q, d - 1e-3).is_none());
        // A hint, good or bad, gives the same answer.
        for hint in [
            all.position.unwrap(),
            FacePosition {
                face: (face + 3) % 6,
                uv: Uv { u: 0.5, v: 0.5 },
            },
        ] {
            let hinted = source.project_from(q, hint, f64::INFINITY).unwrap();
            assert!((hinted.point.distance(q) - d).abs() < 2e-6);
        }
    }
}

#[test]
fn req_imp_02_a_point_on_a_loop_segment_is_answered_on_it() {
    // A point at the image of a loop segment's middle is the boundary
    // search's own first bound; it must still be the answer (a seam of the
    // corpus's rotary blank once came back with none).
    for (name, f, loops) in kinds() {
        for l in &loops {
            for k in (0..l.len()).step_by(7) {
                let (a, b) = (l[k], l[(k + 1) % l.len()]);
                let uv = Uv {
                    u: 0.5 * (a.0 + b.0),
                    v: 0.5 * (a.1 + b.1),
                };
                let Some(p) = f.point_normal(uv) else {
                    continue;
                };
                let c = f.closest(p.point);
                assert!(
                    c.is_some_and(|c| c.distance < 1e-9),
                    "{name}: segment {k} answered {:?}",
                    c.map(|c| c.distance)
                );
            }
        }
    }
    // The seam of the corpus cone's face: a box loop, asked at its middle.
    let f = corpus_cone();
    let p = f
        .point_normal(Uv {
            u: TAU,
            v: -15.524_174_696_260_026,
        })
        .unwrap();
    assert!(f.closest(p.point).is_some_and(|c| c.distance < 1e-9));
}

#[test]
fn req_imp_02_a_model_query_answers_beside_a_loop_off_its_face_box() {
    // A trim loop that strays outside its face's parameter box: its points
    // lie outside the face's surface box, yet bound a whole-model query
    // (the corpus `filleted_box` once answered nothing there).
    let mut faces = Vec::new();
    for (k, (_, mut f, _)) in kind_faces().into_iter().take(2).enumerate() {
        f.id = FaceId(u32::try_from(k).unwrap());
        faces.push(f);
    }
    // The plane's loop runs 20 mm past its box on one side.
    faces[0].uv_bounds = [-10.0, 10.0, -8.0, 8.0];
    faces[0].trims = vec![TrimLoop(
        [(-9.0, -7.0), (30.0, -7.0), (30.0, 7.0), (-9.0, 7.0)]
            .iter()
            .map(|&(u, v)| UvPoint { u, v })
            .collect(),
    )];
    let model = BrepModel {
        faces,
        ..BrepModel::default()
    };
    let source = BrepSurface::new(&model).unwrap();
    let mut seed = 77;
    for _ in 0..200 {
        let q = p3(
            60.0 * lcg(&mut seed) - 20.0,
            40.0 * lcg(&mut seed) - 20.0,
            20.0 * lcg(&mut seed) - 10.0,
        );
        let all = source.project(q);
        assert!(all.is_some(), "no answer at {q:?}");
        let best = source
            .faces()
            .iter()
            .filter_map(|f| f.closest(q))
            .map(|c| c.distance)
            .fold(f64::INFINITY, f64::min);
        assert!((all.unwrap().point.distance(q) - best).abs() < 2e-6);
    }
}

#[test]
fn req_imp_02_a_patch_with_a_full_multiplicity_knot_still_finds_its_closest_point() {
    // A ruled patch whose interior u knot has multiplicity two, above its
    // degree: the surface jumps there. Each span is still a Bezier piece, so
    // the cells hold it; the grid search of before missed its nearest point
    // (4.34 mm against a sample at 3.74 mm).
    let mut poles = Vec::new();
    for (i, x) in [0.0, 10.0, 10.0, 20.0].iter().enumerate() {
        for j in 0..3 {
            let z = if i == 2 { 4.0 } else { 0.0 } + f64::from(j) * 0.5;
            poles.push(p3(*x, 6.0 * f64::from(j), z));
        }
    }
    let patch = BSplinePatch {
        degree_u: 1,
        degree_v: 2,
        knots_u: vec![0.0, 0.0, 0.5, 0.5, 1.0, 1.0],
        knots_v: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        poles_u: 4,
        poles_v: 3,
        poles,
        weights: vec![1.0; 12],
    };
    let f = face(
        Surface::BSpline(patch),
        [0.0, 1.0, 0.0, 1.0],
        &[],
        one_triangle(p3(0.0, 0.0, 0.0), p3(20.0, 0.0, 0.0), p3(0.0, 12.0, 0.0)),
    );
    let f = ExactFace::new(&f).expect("the evaluator accepts a full-multiplicity interior knot");
    let mut seed = 13;
    for _ in 0..30 {
        let q = p3(
            25.0 * lcg(&mut seed) - 2.0,
            14.0 * lcg(&mut seed) - 1.0,
            10.0 * lcg(&mut seed) - 3.0,
        );
        let got = f.closest(q).unwrap();
        let want = sampled(&f, &[], q);
        assert!(
            got.distance <= want + 1e-7,
            "{} against {want}",
            got.distance
        );
    }
}

#[test]
fn req_nfr_07_reach_queries_refuse_invalid_input() {
    let source = BrepSurface::new(&spread_model()).unwrap();
    let nan = p3(f64::NAN, 0.0, 0.0);
    assert!(source.faces_near(nan, 1.0).is_empty());
    assert!(source.faces_near(p3(0.0, 0.0, 0.0), f64::NAN).is_empty());
    let line = ToolLine {
        origin: nan,
        axis: UnitVec3::new(Vec3::new(0.0, 0.0, 1.0)).unwrap(),
    };
    assert!(source.faces_in_reach(&ball(1.0), &line).is_empty());
    assert!(source.project_within(nan, 1.0).is_none());
    assert!(source.project_within(p3(0.0, 0.0, 0.0), f64::NAN).is_none());
}
