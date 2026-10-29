// SPDX-License-Identifier: AGPL-3.0-or-later
//! Exact ball drops (lane STEPALL-X2, issue #349; OI-81; requirement IMP-02,
//! OP-03, NFR-05, NFR-06, NFR-07), checked against independent brute-force
//! answers written here. No kernel is involved.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::cast_precision_loss
)]

use std::f64::consts::TAU;

use openchuck_path::{BrepSurface, DROP_TOLERANCE, ExactFace, ParametricFace, SurfaceSource, ToolLine, Uv};
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


/// A ball end of radius `r` with straight flutes `4 r` long.
fn ball(r: f64) -> RevolvedProfile {
    ToolProfile {
        segments: vec![
            ProfileSegment::Arc {
                to: ProfilePoint::new(r, r),
                centre: ProfilePoint::new(0.0, r),
            },
            ProfileSegment::Line {
                to: ProfilePoint::new(r, 4.0 * r),
            },
        ],
    }
    .validate()
    .unwrap()
}

/// The highest tip offset at which the ball holds `p`, as the module
/// defines it, written out again.
fn lift(line: &ToolLine, r: f64, p: Point3) -> Option<f64> {
    let a = line.axis.get();
    let w = p - (line.origin + a * r);
    let z = w.dot(a);
    let rho2 = w.dot(w) - z * z;
    (rho2 <= r * r).then(|| z + (r * r - rho2).max(0.0).sqrt())
}

/// The highest lift of a dense sampling of the trimmed face and its loops:
/// never above the true drop.
fn sampled_drop(f: &ExactFace, loops: &[Vec<(f64, f64)>], line: &ToolLine, r: f64) -> Option<f64> {
    let (lo, hi) = f.uv_bounds();
    let mut best: Option<f64> = None;
    let mut take = |p: Point3| {
        if let Some(s) = lift(line, r, p) {
            best = Some(best.map_or(s, |b: f64| b.max(s)));
        }
    };
    let n = 400;
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

#[test]
fn req_op_03_exact_ball_drop_of_every_kind_is_at_or_above_brute_force() {
    let mut seed = 41;
    let r = 1.5;
    for ((name, bf, loops), (_, f, _)) in kind_faces().into_iter().zip(kinds()) {
        let model = BrepModel {
            faces: vec![bf],
            edges: vec![],
            vertices: vec![],
            tessellation: Default::default(),
        };
        let surface = BrepSurface::new(&model).unwrap();
        let b = f.bounding_box();
        let c = p3(
            0.5 * (b.min.x + b.max.x),
            0.5 * (b.min.y + b.max.y),
            0.5 * (b.min.z + b.max.z),
        );
        let size = (b.max - b.min).length();
        let mut hits = 0;
        for k in 0..40 {
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
            let want = sampled_drop(&f, &loops, &line, r);
            let got = surface.drop_ball(&ball(r), &line);
            match (got, want) {
                (None, None) => {}
                (Some(g), Some(w)) => {
                    hits += 1;
                    let s = g.contact.offset;
                    assert!(s >= w - DROP_TOLERANCE - 1e-9, "{name} {k}: {s} below a sample at {w}");
                    assert!(s - w < 0.02, "{name} {k}: {s} far above the samples' {w}");
                    // The contact is a point of the face the ball holds.
                    let l = lift(&line, r, g.contact.contact).unwrap();
                    assert!((l - s).abs() <= DROP_TOLERANCE + 1e-9 || g.raised > 0.0, "{name} {k}");
                }
                (g, w) => panic!("{name} {k}: exact {g:?}, sampled {w:?}"),
            }
        }
        assert!(hits > 5, "{name}: only {hits} lines touch");
    }
}
