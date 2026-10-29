// SPDX-License-Identifier: AGPL-3.0-or-later
//! Exact ball drops (lane STEPALL-X2, issue #349; OI-81; requirement IMP-02,
//! OP-02, OP-11, OP-12, OP-23, NFR-05, NFR-06, NFR-07; decision 0277), checked
//! against independent brute-force answers written here. No kernel is involved.
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
    let n = 250;
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
fn req_op_12_exact_ball_drop_of_every_kind_is_at_or_above_brute_force() {
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
        for k in 0..60 {
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
                    assert!(
                        s >= w - DROP_TOLERANCE - 1e-9,
                        "{name} {k}: {s} below a sample at {w}"
                    );
                    // ...and the answer is a real contact: a point of the
                    // face the ball holds at that tip, so never above the
                    // true drop.
                    let c = g.contact.contact;
                    assert!(
                        f.closest(c).unwrap().distance < 1e-6,
                        "{name} {k}: off the face"
                    );
                    // The contact is a point of the face the ball holds.
                    let l = lift(&line, r, g.contact.contact).unwrap();
                    assert!(
                        (l - s).abs() <= DROP_TOLERANCE + 1e-9 || g.raised > 0.0,
                        "{name} {k}"
                    );
                }
                (g, w) => panic!("{name} {k}: exact {g:?}, sampled {w:?}"),
            }
        }
        assert!(hits > 5, "{name}: only {hits} lines touch");
    }
}

fn radial_line(x: f64, degrees: f64) -> ToolLine {
    let (s, c) = degrees.to_radians().sin_cos();
    ToolLine {
        origin: p3(x, 0.0, 0.0),
        axis: UnitVec3::new(Vec3::new(0.0, c, s)).unwrap(),
    }
}

#[test]
fn req_nfr_06_exact_ball_drop_is_the_same_on_every_thread_and_run() {
    let (_, bf, _) = kind_faces().into_iter().nth(6).unwrap();
    let model = BrepModel {
        faces: vec![bf],
        edges: vec![],
        vertices: vec![],
        tessellation: Default::default(),
    };
    let surface = BrepSurface::new(&model).unwrap();
    let lines: Vec<ToolLine> = (0..200)
        .map(|k| ToolLine {
            origin: p3(1.0 + 0.2 * f64::from(k), 2.0 + 0.1 * f64::from(k), 30.0),
            axis: UnitVec3::new(Vec3::new(0.1, -0.2, 1.0)).unwrap(),
        })
        .collect();
    let tool = ball(1.0);
    let once: Vec<_> = lines.iter().map(|l| surface.drop_ball(&tool, l)).collect();
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..4)
            .map(|_| {
                scope.spawn(|| {
                    lines
                        .iter()
                        .rev()
                        .map(|l| surface.drop_ball(&tool, l))
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

#[test]
fn req_nfr_07_exact_ball_drop_refuses_what_it_cannot_answer() {
    let model = openchuck_testkit::brep::banded_cylinder(10.0, 30.0, &[10.0, 20.0]);
    let surface = BrepSurface::new(&model).unwrap().with_exact_drops(true);
    // A line that is not a number gives no answer, not a panic.
    let bad = ToolLine {
        origin: p3(f64::NAN, 0.0, 0.0),
        axis: UnitVec3::new(Vec3::new(0.0, 0.0, 1.0)).unwrap(),
    };
    assert!(surface.drop_ball(&ball(1.0), &bad).is_none());
    // A tool that is not a ball has no exact drop; the surface drops it on
    // the tessellation.
    let flat = ToolProfile {
        segments: vec![
            ProfileSegment::Line {
                to: ProfilePoint::new(1.0, 0.0),
            },
            ProfileSegment::Line {
                to: ProfilePoint::new(1.0, 4.0),
            },
        ],
    }
    .validate()
    .unwrap();
    assert!(openchuck_path::ball_radius(&flat).is_none());
    let line = radial_line(15.0, 30.0);
    assert!(surface.drop_ball(&flat, &line).is_none());
    let mesh_only = BrepSurface::new(&model).unwrap();
    assert_eq!(
        surface.drop_tool(&flat, &line),
        mesh_only.drop_tool(&flat, &line)
    );
}

#[test]
fn req_op_11_a_ball_grown_for_stock_to_leave_drops_exactly() {
    // Stock to leave grows the ball (OP-11): the grown tool is a ball of
    // the larger radius, and drops as one.
    let grown = openchuck_path::cutter::grow_profile(&ball(1.0), 0.3).unwrap();
    let r = openchuck_path::ball_radius(&grown).unwrap();
    assert!((r - 1.3).abs() < 1e-12);
    let model = openchuck_testkit::brep::banded_cylinder(10.0, 30.0, &[10.0, 20.0]);
    let surface = BrepSurface::new(&model).unwrap().with_exact_drops(true);
    let mesh_only = BrepSurface::new(&model).unwrap();
    for a in [0.0, 17.0, 95.0, 200.0] {
        let line = radial_line(15.0, a);
        let exact = surface.drop_tool(&grown, &line).unwrap();
        // A ball on a cylinder of radius 10 about the line's own axis comes
        // to rest with its tip on the cylinder: 10 from the axis, so 0.3
        // beyond it for the grown tip.
        assert!(
            (exact.offset - 10.0).abs() <= DROP_TOLERANCE,
            "{}",
            exact.offset
        );
        // The tessellation lies inside the cylinder, so its drop is lower.
        let mesh = mesh_only.drop_tool(&grown, &line).unwrap();
        assert!(mesh.offset <= exact.offset + 1e-9);
        assert_eq!(surface.drop_accuracy(), DROP_TOLERANCE);
        assert_eq!(mesh_only.drop_accuracy(), 0.0);
    }
}

#[test]
fn req_op_02_req_op_23_exact_contacts_are_judged_on_exact_face_sets() {
    // OI-833 (decision 0277): exact contacts lie on the exact faces, up to
    // 0.005 mm off the triangles of a 96-facet cylinder of radius 10, so a
    // region, avoid or check set must be the exact faces as well.
    use openchuck_path::{MeshSurface, RegionConstraint};
    use openchuck_types::brep::FaceId;
    let model = openchuck_testkit::brep::banded_cylinder(10.0, 30.0, &[10.0, 20.0]);
    let part = BrepSurface::new(&model).unwrap().with_exact_drops(true);
    let band = |i: u32| {
        BrepSurface::with_faces(&model, &[FaceId(i)])
            .unwrap()
            .with_exact_drops(true)
    };
    let (first, middle, last) = (band(0), band(1), band(2));
    let middle_mesh = MeshSurface::new(middle.tessellation()).unwrap();
    let tool = ball(1.0);
    let region = RegionConstraint {
        region: Some(&middle),
        ..RegionConstraint::default()
    };
    let triangles = RegionConstraint {
        region: Some(&middle_mesh),
        ..RegionConstraint::default()
    };
    let avoid = RegionConstraint {
        avoid: Some(&first),
        ..RegionConstraint::default()
    };
    let check = RegionConstraint {
        check: Some(&last),
        ..RegionConstraint::default()
    };
    let mut off_triangles = 0;
    for a in (0..36).map(|k| 10.0 * f64::from(k) + 1.3) {
        for (x, in_middle) in [(5.0, false), (15.0, true), (25.0, false)] {
            let line = radial_line(x, a);
            let c = part.drop_tool(&tool, &line).unwrap();
            assert_eq!(
                region.allows(c.contact, c.tip, &tool, &line),
                in_middle,
                "x {x} a {a}"
            );
            assert_eq!(
                avoid.allows(c.contact, c.tip, &tool, &line),
                x > 10.0,
                "x {x} a {a}"
            );
            // The ball touches the last band only where it sits on it.
            assert_eq!(
                check.allows(c.contact, c.tip, &tool, &line),
                x < 20.0,
                "x {x} a {a}"
            );
            if in_middle && !triangles.allows(c.contact, c.tip, &tool, &line) {
                off_triangles += 1;
            }
        }
        // A tool resting on the edge between two bands touches both: a
        // check on the next band stops it, a region of its own allows it.
        let line = radial_line(20.0, a);
        let c = part.drop_tool(&tool, &line).unwrap();
        assert!(!check.allows(c.contact, c.tip, &tool, &line), "a {a}");
        assert!(region.allows(c.contact, c.tip, &tool, &line), "a {a}");
    }
    // Most exact contacts lie off the triangles, which is why the sets are
    // exact faces when the drops are.
    assert!(off_triangles > 18, "{off_triangles}");
}

#[test]
fn req_op_12_a_ball_rests_on_the_point_of_a_cone() {
    // The apex of a cone is a point of its own (decision 0277): a ball over
    // the point of a sharp cone rests on it, not on a wall, since the walls
    // fall away faster than the ball curves.
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
    let model = BrepModel {
        faces: vec![cone],
        edges: vec![],
        vertices: vec![],
        tessellation: Default::default(),
    };
    let surface = BrepSurface::new(&model).unwrap();
    let r = 1.0;
    for dx in [0.0, 0.05, 0.2, 0.4] {
        let line = ToolLine {
            origin: p3(dx, 0.1 * dx, 10.0),
            axis: UnitVec3::new(Vec3::new(0.0, 0.0, 1.0)).unwrap(),
        };
        let got = surface.drop_ball(&ball(r), &line).unwrap();
        let want = lift(&line, r, Point3::ORIGIN).unwrap();
        assert!(
            (got.contact.offset - want).abs() < 1e-9,
            "dx {dx}: {} against {want}",
            got.contact.offset
        );
        assert!(
            got.contact.contact.distance(Point3::ORIGIN) < 1e-9,
            "dx {dx}"
        );
    }
}
