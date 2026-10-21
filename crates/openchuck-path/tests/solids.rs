// SPDX-License-Identifier: AGPL-3.0-or-later
//! Analytic solids and the composite target (SET-09 held region, OP-21
//! retaining tabs; work breakdown slice 21b; decision record 0035).
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing
)]

mod support;

use std::collections::BTreeMap;
use std::sync::Arc;

use openchuck_path::params::names;
use openchuck_path::{
    AcceptAll, AnalyticStock, CompositeSurface, HelicalFinishing, MeshSurface, ParameterValue,
    RotaryMapping, Solid, StrategyInput, SurfaceSource, ToolLine, generate,
};
use openchuck_testkit::gouge::{Outline, Pose, max_gouge};
use openchuck_testkit::parts::{Cone, Cylinder, RadialPart};
use openchuck_types::geometry::{Point3, UnitVec3, Vec3};
use openchuck_types::ir::MoveKind;
use openchuck_types::machine::Clearance;
use openchuck_types::tool::{RevolvedProfile, ToolShape};
use openchuck_types::units::Millimetres;
use support::{ball, machine, run, tags, tool};

fn dir(x: f64, y: f64, z: f64) -> UnitVec3 {
    UnitVec3::new(Vec3::new(x, y, z)).unwrap()
}

/// A deterministic stream in `[0, 1)`; the tests need no external randomness.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }

    fn between(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next()
    }
}

/// The solids the tests use: about the rotary axis (a held region, a neck),
/// a frustum, a tab standing out across the axis, and one on a tilted axis.
fn solids() -> Vec<Solid> {
    vec![
        Solid::cylinder(Point3::new(0.0, 0.0, 0.0), dir(1.0, 0.0, 0.0), 30.0, 10.0).unwrap(),
        Solid::frustum(
            Point3::new(-5.0, 0.0, 0.0),
            dir(1.0, 0.0, 0.0),
            25.0,
            12.0,
            5.0,
        )
        .unwrap(),
        Solid::cylinder(Point3::new(8.0, 0.0, 0.0), dir(0.0, 1.0, 0.0), 14.0, 3.0).unwrap(),
        Solid::frustum(
            Point3::new(2.0, 1.0, -3.0),
            dir(0.3, 0.5, 0.8),
            18.0,
            2.5,
            7.0,
        )
        .unwrap(),
    ]
}

fn mesh_volume(m: &openchuck_types::mesh::TriangleMesh) -> f64 {
    m.triangles
        .iter()
        .map(|t| {
            let [a, b, c] = t.map(|i| m.vertices[i as usize].to_vec());
            a.dot(b.cross(c)) / 6.0
        })
        .sum()
}

#[test]
fn req_op_21_a_solid_has_its_analytic_volume_area_bounds_and_outward_mesh() {
    let c = Solid::cylinder(Point3::new(1.0, 2.0, 3.0), dir(1.0, 0.0, 0.0), 10.0, 4.0).unwrap();
    assert!((c.volume() - std::f64::consts::PI * 16.0 * 10.0).abs() < 1e-9);
    assert!(
        (c.surface_area()
            - (2.0 * std::f64::consts::PI * 16.0 + 2.0 * std::f64::consts::PI * 4.0 * 10.0))
            .abs()
            < 1e-9
    );
    let b = c.bounds();
    assert_eq!((b.min.x, b.max.x), (1.0, 11.0));
    assert!((b.min.y + 2.0).abs() < 1e-12 && (b.max.y - 6.0).abs() < 1e-12);
    assert!((b.min.z + 1.0).abs() < 1e-12 && (b.max.z - 7.0).abs() < 1e-12);
    let f = Solid::frustum(Point3::ORIGIN, dir(0.0, 0.0, 1.0), 6.0, 5.0, 2.0).unwrap();
    let want = std::f64::consts::PI / 3.0 * 6.0 * (25.0 + 10.0 + 4.0);
    assert!((f.volume() - want).abs() < 1e-9);
    let slant = 6.0f64.hypot(3.0);
    let area = std::f64::consts::PI * (25.0 + 4.0 + 7.0 * slant);
    assert!((f.surface_area() - area).abs() < 1e-9);
    assert!(f.contains(Point3::new(4.9, 0.0, 0.0)));
    assert!(!f.contains(Point3::new(4.9, 0.0, 6.0)));
    assert!(f.contains(Point3::new(0.0, 0.0, 6.0)));
    assert!(!f.contains(Point3::new(0.0, 0.0, 6.1)));
    // The mesh is closed and faces outward: its signed volume is positive and
    // close to the exact one (a polygon of 96 sides loses 0.1 per cent).
    for s in solids() {
        let v = mesh_volume(&s.mesh(96));
        assert!(
            v > 0.0 && (1.0 - v / s.volume()).abs() < 2e-3,
            "{v} {}",
            s.volume()
        );
    }
}

#[test]
fn req_op_21_an_impossible_solid_is_refused_with_a_message() {
    let origin = Point3::ORIGIN;
    let x = dir(1.0, 0.0, 0.0);
    for (what, result) in [
        ("length", Solid::cylinder(origin, x, 0.0, 3.0)),
        ("base radius", Solid::frustum(origin, x, 5.0, -1.0, 3.0)),
        ("top radius", Solid::frustum(origin, x, 5.0, 3.0, 0.0)),
        ("length", Solid::cylinder(origin, x, f64::NAN, 3.0)),
        (
            "base radius",
            Solid::cylinder(origin, x, 5.0, f64::INFINITY),
        ),
        (
            "base point",
            Solid::cylinder(Point3::new(f64::NAN, 0.0, 0.0), x, 5.0, 3.0),
        ),
    ] {
        let err = result.unwrap_err().to_string();
        assert!(err.contains(what), "{what}: {err}");
    }
}

#[test]
fn req_op_21_the_closest_point_and_normal_are_exact() {
    let c = Solid::cylinder(Point3::ORIGIN, dir(1.0, 0.0, 0.0), 10.0, 5.0).unwrap();
    let on_side = c.project(Point3::new(5.0, 8.0, 0.0)).unwrap();
    assert_eq!(on_side.point, Point3::new(5.0, 5.0, 0.0));
    assert!((on_side.normal.get().y - 1.0).abs() < 1e-15);
    let on_base = c.project(Point3::new(-2.0, 3.0, 0.0)).unwrap();
    assert_eq!(on_base.point, Point3::new(0.0, 3.0, 0.0));
    assert!((on_base.normal.get().x + 1.0).abs() < 1e-15);
    let rim = c.project(Point3::new(-2.0, 8.0, 0.0)).unwrap();
    assert!(rim.point.distance(Point3::new(0.0, 5.0, 0.0)) < 1e-15);
    // On the axis outside the top disc, and a point inside (the boundary).
    let above = c.project(Point3::new(14.0, 0.0, 0.0)).unwrap();
    assert_eq!(above.point, Point3::new(10.0, 0.0, 0.0));
    let inside = c.project(Point3::new(5.0, 4.0, 0.0)).unwrap();
    assert_eq!(inside.point, Point3::new(5.0, 5.0, 0.0));
    // A frustum: the side's normal tilts away from the wider end.
    let f = Solid::frustum(Point3::ORIGIN, dir(1.0, 0.0, 0.0), 10.0, 5.0, 2.0).unwrap();
    let p = f.project(Point3::new(5.0, 9.0, 0.0)).unwrap();
    let slant = 10.0f64.hypot(3.0);
    let want = Vec3::new(3.0 / slant, 10.0 / slant, 0.0);
    assert!((p.normal.get() - want).length() < 1e-14);
    // The surface point is the foot: the vector to the query is along the normal.
    let v = Point3::new(5.0, 9.0, 0.0) - p.point;
    assert!((v - p.normal.get() * v.length()).length() < 1e-12);
    assert!(
        c.project(Point3::new(f64::NAN, 0.0, 0.0)).is_none(),
        "a bad point gives nothing, not a panic"
    );
}

#[test]
fn req_op_21_the_closest_point_is_the_nearest_of_a_dense_sampling() {
    let mut rng = Lcg(7);
    for s in solids() {
        // Points across every triangle of an inscribed mesh, not just its
        // corners: the discs have few vertices.
        let mesh = s.mesh(180);
        let mut samples = Vec::new();
        for t in &mesh.triangles {
            let [a, b, c] = t.map(|i| mesh.vertices[i as usize]);
            let longest = a.distance(b).max(b.distance(c)).max(c.distance(a));
            let n = (longest / 0.8).ceil().max(1.0) as u32;
            for i in 0..=n {
                for j in 0..=(n - i) {
                    let (u, v) = (f64::from(i) / f64::from(n), f64::from(j) / f64::from(n));
                    samples.push(a + (b - a) * u + (c - a) * v);
                }
            }
        }
        for _ in 0..24 {
            let b = s.bounds();
            let q = Point3::new(
                rng.between(b.min.x - 8.0, b.max.x + 8.0),
                rng.between(b.min.y - 8.0, b.max.y + 8.0),
                rng.between(b.min.z - 8.0, b.max.z + 8.0),
            );
            let got = s.project(q).unwrap();
            let d = got.point.distance(q);
            // The samples lie on the surface or just inside it, so none is
            // closer than the exact answer, and the nearest is close to it.
            let sampled = samples
                .iter()
                .map(|v| v.distance(q))
                .fold(f64::INFINITY, f64::min);
            assert!(d <= sampled + 1e-9, "{d} {sampled}");
            assert!(sampled - d < 0.4, "{d} {sampled}");
        }
    }
}

#[test]
fn req_op_21_a_ray_meets_the_surface_where_the_mesh_does() {
    let mut rng = Lcg(11);
    for s in solids() {
        let m = MeshSurface::new(&s.mesh(1440)).unwrap();
        let b = s.bounds();
        let (mut hits, mut max_diff) = (0, 0.0f64);
        for _ in 0..150 {
            let o = Point3::new(
                rng.between(b.min.x - 15.0, b.max.x + 15.0),
                rng.between(b.min.y - 15.0, b.max.y + 15.0),
                rng.between(b.min.z - 15.0, b.max.z + 15.0),
            );
            let target = Point3::new(
                rng.between(b.min.x, b.max.x),
                rng.between(b.min.y, b.max.y),
                rng.between(b.min.z, b.max.z),
            );
            let Ok(d) = UnitVec3::new(target - o) else {
                continue;
            };
            let want = m.cast_ray(o, d, 1e6);
            let got = s.cast_ray(o, d, 1e6);
            // Grazing rays may meet only one of the two, so those are skipped.
            if let (Some(g), Some(w)) = (got, want) {
                hits += 1;
                max_diff = max_diff.max((g.distance - w.distance).abs());
                assert!((g.point - (o + d.get() * g.distance)).length() < 1e-9);
                assert!((g.normal.get().length() - 1.0).abs() < 1e-12);
            }
        }
        assert!(hits > 20, "{hits} rays hit");
        assert!(max_diff < 5e-3, "ray distances differ by {max_diff}");
    }
}

#[test]
fn req_op_21_a_ray_hits_exact_distances_from_outside_and_inside() {
    let c = Solid::cylinder(Point3::ORIGIN, dir(1.0, 0.0, 0.0), 10.0, 5.0).unwrap();
    // Down onto the side, along the axis onto a disc, and sideways past it.
    let down = c
        .cast_ray(Point3::new(5.0, 0.0, 20.0), dir(0.0, 0.0, -1.0), 100.0)
        .unwrap();
    assert!((down.distance - 15.0).abs() < 1e-12);
    assert!((down.normal.get().z - 1.0).abs() < 1e-12);
    let along = c
        .cast_ray(Point3::new(-4.0, 1.0, 1.0), dir(1.0, 0.0, 0.0), 100.0)
        .unwrap();
    assert!((along.distance - 4.0).abs() < 1e-12);
    assert!((along.normal.get().x + 1.0).abs() < 1e-12);
    assert!(
        c.cast_ray(Point3::new(5.0, 0.0, 20.0), dir(0.0, 0.0, -1.0), 10.0)
            .is_none(),
        "no further than the limit"
    );
    assert!(
        c.cast_ray(Point3::new(5.0, 6.0, 20.0), dir(0.0, 0.0, -1.0), 100.0)
            .is_none(),
        "a ray that passes beside it"
    );
    // From inside it leaves through the side, and through the top disc.
    let out = c
        .cast_ray(Point3::new(5.0, 0.0, 0.0), dir(0.0, 1.0, 0.0), 100.0)
        .unwrap();
    assert!((out.distance - 5.0).abs() < 1e-12);
    let up = c
        .cast_ray(Point3::new(5.0, 0.0, 0.0), dir(1.0, 0.0, 0.0), 100.0)
        .unwrap();
    assert!((up.distance - 5.0).abs() < 1e-12);
    // A frustum: the slant meets a ray at the interpolated radius.
    let f = Solid::frustum(Point3::ORIGIN, dir(1.0, 0.0, 0.0), 10.0, 6.0, 2.0).unwrap();
    let h = f
        .cast_ray(Point3::new(5.0, 0.0, 20.0), dir(0.0, 0.0, -1.0), 100.0)
        .unwrap();
    assert!((h.point.z - 4.0).abs() < 1e-12);
}

fn profile(shape: ToolShape, diameter: f64) -> RevolvedProfile {
    tool(shape, diameter).profile().unwrap()
}

#[test]
fn req_op_21_a_tool_drops_on_a_cylinder_at_the_closed_form_height() {
    // A cylinder of radius 10 about X, the tool above it with its axis along Z.
    let r = 10.0;
    let c = Solid::cylinder(Point3::ORIGIN, dir(1.0, 0.0, 0.0), 30.0, r).unwrap();
    let ball = profile(ToolShape::BallEnd, 6.0);
    let flat = profile(ToolShape::FlatEnd, 6.0);
    for y in [0.0, 1.0, 2.5, 4.0, 7.0] {
        let line = ToolLine {
            origin: Point3::new(15.0, y, 0.0),
            axis: UnitVec3::Z,
        };
        // A ball of radius 3 rests with its centre at distance r + 3 from the
        // axis; its tip is 3 below the centre.
        let c_ball = c.drop_tool(&ball, &line).unwrap();
        let want_ball = ((r + 3.0).powi(2) - y * y).sqrt() - 3.0;
        assert!(
            (c_ball.offset - want_ball).abs() < 1e-9,
            "ball at y {y}: {} against {want_ball}",
            c_ball.offset
        );
        assert!((c_ball.tip.z - want_ball).abs() < 1e-9);
        // A flat end of radius 3 rests on the highest point under its disc,
        // the one with the smallest |y|.
        let c_flat = c.drop_tool(&flat, &line).unwrap();
        let nearest = (y.abs() - 3.0).max(0.0);
        let want_flat = (r * r - nearest * nearest).sqrt();
        assert!(
            (c_flat.offset - want_flat).abs() < 1e-9,
            "flat at y {y}: {} against {want_flat}",
            c_flat.offset
        );
    }
    // Over the end disc a flat end rests on the flat top of a cylinder along Z.
    let post = Solid::cylinder(Point3::ORIGIN, UnitVec3::Z, 7.0, 5.0).unwrap();
    let line = ToolLine {
        origin: Point3::new(6.0, 0.0, 0.0),
        axis: UnitVec3::Z,
    };
    let landed = post.drop_tool(&flat, &line).unwrap();
    assert!((landed.offset - 7.0).abs() < 1e-9, "{}", landed.offset);
    // Out of reach on either side gives nothing.
    let far = ToolLine {
        origin: Point3::new(15.0, 14.0, 0.0),
        axis: UnitVec3::Z,
    };
    assert!(c.drop_tool(&ball, &far).is_none());
}

#[test]
fn req_op_21_the_tool_drop_matches_a_fine_mesh_of_the_solid() {
    let tools = [
        profile(ToolShape::BallEnd, 6.0),
        profile(ToolShape::FlatEnd, 5.0),
        profile(
            ToolShape::BullNose {
                corner_radius: Millimetres(1.0),
            },
            6.0,
        ),
    ];
    let mut rng = Lcg(23);
    let (mut checked, mut worst) = (0, 0.0f64);
    for s in solids() {
        // Inscribed, so it lies inside the solid: the mesh drop is never
        // higher than the exact one.
        let mesh = MeshSurface::new(&s.mesh(1440)).unwrap();
        let b = s.bounds();
        for profile in &tools {
            for _ in 0..40 {
                let axis = match checked % 3 {
                    0 => UnitVec3::Z,
                    1 => {
                        // The 4-axis case: perpendicular to the rotary axis.
                        let a = rng.between(0.0, std::f64::consts::TAU);
                        dir(0.0, a.sin(), a.cos())
                    }
                    _ => dir(
                        rng.between(-0.4, 0.4),
                        rng.between(-0.4, 0.4),
                        rng.between(0.8, 1.0),
                    ),
                };
                let origin = Point3::new(
                    rng.between(b.min.x - 3.0, b.max.x + 3.0),
                    rng.between(b.min.y - 3.0, b.max.y + 3.0),
                    rng.between(b.min.z - 3.0, b.max.z + 3.0),
                );
                let line = ToolLine { origin, axis };
                let want = mesh.drop_tool(profile, &line);
                let got = s.drop_tool(profile, &line);
                checked += 1;
                match (got, want) {
                    (Some(g), Some(w)) => {
                        let diff = g.offset - w.offset;
                        assert!(
                            diff > -1e-9,
                            "exact {} below the mesh {}",
                            g.offset,
                            w.offset
                        );
                        worst = worst.max(diff);
                        assert!(
                            diff < 2e-3,
                            "exact {} against mesh {} for {line:?}",
                            g.offset,
                            w.offset
                        );
                        assert!(s.bounds().contains(Point3::new(
                            g.contact.x.clamp(b.min.x, b.max.x),
                            g.contact.y.clamp(b.min.y, b.max.y),
                            g.contact.z.clamp(b.min.z, b.max.z)
                        )));
                        // The contact is on the surface of the solid.
                        let on = s.project(g.contact).unwrap().point.distance(g.contact);
                        assert!(on < 1e-6, "contact {on} from the surface");
                    }
                    (Some(_), None) | (None, None) => {}
                    (None, Some(w)) => panic!("the mesh was reached ({}), the solid not", w.offset),
                }
            }
        }
    }
    assert!(checked > 400 && worst < 2e-3, "{checked} {worst}");
}

fn model() -> Arc<dyn SurfaceSource> {
    let part = Cone {
        start_radius: 10.0,
        end_radius: 16.0,
        length: 30.0,
    };
    Arc::new(MeshSurface::new(&part.mesh(61, 720)).unwrap())
}

#[test]
fn req_set_09_the_composite_answers_as_the_union_of_its_parts() {
    let m = model();
    let held =
        Solid::cylinder(Point3::new(30.0, 0.0, 0.0), dir(1.0, 0.0, 0.0), 10.0, 18.0).unwrap();
    let tab = Solid::frustum(
        Point3::new(-6.0, 0.0, 0.0),
        dir(1.0, 0.0, 0.0),
        6.0,
        5.0,
        8.0,
    )
    .unwrap();
    let composite = CompositeSurface::new(Arc::clone(&m), vec![held, tab]);
    assert_eq!(composite.solids().len(), 2);
    let b = composite.bounds();
    assert!(b.min.x <= -6.0 && b.max.x >= 40.0 && b.max.y >= 18.0);
    // The tessellation is the model's plus each solid's.
    let tri = composite.tessellation();
    assert_eq!(
        tri.triangles.len(),
        m.tessellation().triangles.len() + 2 * held.mesh(96).triangles.len()
    );
    assert!(tri.validate().is_ok());
    let ball = profile(ToolShape::BallEnd, 4.0);
    for x in [-5.0, -1.0, 8.0, 29.0, 33.0, 39.0, 41.0] {
        let line = ToolLine {
            origin: Point3::new(x, 0.0, 0.0),
            axis: UnitVec3::Z,
        };
        let parts = [
            m.drop_tool(&ball, &line),
            held.drop_tool(&ball, &line),
            tab.drop_tool(&ball, &line),
        ];
        let best = parts
            .into_iter()
            .flatten()
            .max_by(|a, b| a.offset.total_cmp(&b.offset));
        let got = composite.drop_tool(&ball, &line);
        assert_eq!(got, best, "drop at x {x}");
        // Over the held region the tool rides on it, not on the cone below.
        if (31.0..=39.0).contains(&x) {
            assert!((got.unwrap().offset - 18.0).abs() < 1e-6, "{got:?}");
        }
    }
    // Nearest point and first ray hit across the parts.
    let q = Point3::new(35.0, 30.0, 0.0);
    let p = composite.project(q).unwrap();
    assert!((p.point.distance(q) - 12.0).abs() < 1e-9);
    assert!((composite.distance(q).unwrap() - 12.0).abs() < 1e-9);
    let hit = composite
        .cast_ray(Point3::new(35.0, 0.0, 30.0), dir(0.0, 0.0, -1.0), 100.0)
        .unwrap();
    assert!((hit.distance - 12.0).abs() < 1e-9);
    let model_hit = composite
        .cast_ray(Point3::new(10.0, 0.0, 30.0), dir(0.0, 0.0, -1.0), 100.0)
        .unwrap();
    let want = m
        .cast_ray(Point3::new(10.0, 0.0, 30.0), dir(0.0, 0.0, -1.0), 100.0)
        .unwrap();
    assert_eq!(model_hit, want);
}

#[test]
fn req_op_21_a_composite_without_solids_is_the_model() {
    let m = model();
    let composite = CompositeSurface::new(Arc::clone(&m), Vec::new());
    assert_eq!(composite.bounds(), m.bounds());
    assert_eq!(composite.tessellation(), m.tessellation());
    let t = profile(ToolShape::BallEnd, 6.0);
    for x in [1.0, 15.0, 29.0] {
        let line = ToolLine {
            origin: Point3::new(x, 1.0, 0.0),
            axis: dir(0.0, 0.2, 1.0),
        };
        assert_eq!(composite.drop_tool(&t, &line), m.drop_tool(&t, &line));
    }
    let q = Point3::new(12.0, 4.0, 25.0);
    assert_eq!(composite.project(q), m.project(q));
}

/// The oracle part with each solid about the rotary axis added to it: the
/// exact surface of the finished part and what is held, for the independent
/// checker.
struct WithSolids<'a> {
    part: &'a Cone,
    solids: &'a [Solid],
}

impl RadialPart for WithSolids<'_> {
    fn name(&self) -> &'static str {
        "cone with held region and tab"
    }

    fn x_range(&self) -> (f64, f64) {
        let (mut lo, mut hi) = self.part.x_range();
        for s in self.solids {
            let b = s.bounds();
            lo = lo.min(b.min.x);
            hi = hi.max(b.max.x);
        }
        (lo, hi)
    }

    fn radius_at(&self, x: f64, angle: f64) -> f64 {
        let (x0, x1) = self.part.x_range();
        let mut r = if (x0..=x1).contains(&x) {
            self.part.radius_at(x, angle)
        } else {
            0.0
        };
        for s in self.solids {
            // Each solid here lies about the rotary axis, along +X.
            let s0 = s.base().x;
            if (s0..=s0 + s.length()).contains(&x) {
                let k = (x - s0) / s.length();
                r = r.max(s.base_radius() + (s.top_radius() - s.base_radius()) * k);
            }
        }
        r
    }
}

#[test]
fn req_op_21_helical_finishing_on_the_composite_never_gouges_the_held_region_or_the_tab() {
    let part = Cone {
        start_radius: 10.0,
        end_radius: 16.0,
        length: 30.0,
    };
    let held =
        Solid::cylinder(Point3::new(30.0, 0.0, 0.0), dir(1.0, 0.0, 0.0), 10.0, 18.0).unwrap();
    let tab = Solid::frustum(
        Point3::new(-6.0, 0.0, 0.0),
        dir(1.0, 0.0, 0.0),
        6.0,
        5.0,
        8.0,
    )
    .unwrap();
    let solids = [held, tab];
    let composite = CompositeSurface::new(model(), solids.to_vec());
    let t = ball(4.0);
    let tool_profile = t.profile().unwrap();
    let machine = machine();
    let stock = AnalyticStock::Cylinder {
        radius: 20.0,
        x_start: -8.0,
        x_end: 42.0,
    };
    let tag = tags();
    let input = StrategyInput {
        surface: &composite,
        stock: &stock,
        collision: &AcceptAll,
        tool: &tool_profile,
        rotary: RotaryMapping::of_profile(&machine),
        clearance: Clearance {
            radius: Millimetres(25.0),
            safe_z: Millimetres(35.0),
        },
        max_feeds: machine.max_feeds,
        tags: &tag,
        keep_out: &[],
    };
    let values: BTreeMap<String, ParameterValue> = [
        (names::STEPOVER, 2.0),
        (names::FEED, 300.0),
        (names::PLUNGE_FEED, 100.0),
    ]
    .into_iter()
    .map(|(k, x)| (k.to_owned(), ParameterValue::Number(x)))
    .collect();
    let path = run(|ctx| generate(&HelicalFinishing, &input, &values, ctx)).unwrap();
    let poses: Vec<Pose> = path
        .group
        .segments
        .iter()
        .filter(|s| !matches!(s.kind, MoveKind::Rapid | MoveKind::Linking))
        .map(|s| Pose {
            tip: s.end.tip,
            axis: s.end.axis.get(),
        })
        .collect();
    assert!(poses.len() > 500, "{} poses", poses.len());
    // The path reaches over the held region and the tab, riding on them.
    let top = |lo: f64, hi: f64| {
        poses
            .iter()
            .filter(|p| (lo..hi).contains(&p.tip.x))
            .map(|p| p.tip.y.hypot(p.tip.z))
            .fold(0.0, f64::max)
    };
    assert!(top(31.0, 39.0) > 17.0, "held region {}", top(31.0, 39.0));
    assert!(top(-5.0, -1.0) > 5.0, "tab {}", top(-5.0, -1.0));
    let exact = WithSolids {
        part: &part,
        solids: &solids,
    };
    let outline = Outline::new(&tool_profile);
    let near: Vec<Pose> = poses.iter().copied().step_by(2).collect();
    let g = max_gouge(&outline, &exact, &near, 0.15);
    assert!(g.depth <= 0.003, "gouge {} at {:?}", g.depth, g.at);
    // And the check can fail: a path made for the cone alone cuts the held
    // region it knows nothing of.
    let bare = Cylinder {
        radius: 15.0,
        length: 20.0,
    };
    let sunk = Pose {
        tip: Point3::new(10.0, 0.0, 10.0),
        axis: Vec3::new(0.0, 0.0, 1.0),
    };
    assert!(max_gouge(&outline, &bare, &[sunk], 0.15).depth > 1.0);
}

#[test]
fn req_nfr_06_the_composite_drop_is_identical_at_1_2_and_n_threads() {
    use rayon::prelude::*;
    let composite = CompositeSurface::new(model(), solids());
    let t = profile(ToolShape::BallEnd, 5.0);
    let lines: Vec<ToolLine> = (0..400)
        .map(|i| {
            let a = f64::from(i) * 0.37;
            ToolLine {
                origin: Point3::new(-5.0 + f64::from(i % 50), 0.0, 0.0),
                axis: dir(0.0, a.sin(), a.cos()),
            }
        })
        .collect();
    let mut results = Vec::new();
    for threads in [1, 2, 7] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        let drops: Vec<_> = pool.install(|| {
            lines
                .par_iter()
                .map(|l| composite.drop_tool(&t, l))
                .collect()
        });
        results.push(drops);
    }
    assert!(results[0].iter().flatten().count() > 100);
    assert_eq!(results[0], results[1]);
    assert_eq!(results[0], results[2]);
}

#[test]
fn req_op_21_a_small_tool_through_a_large_solid_finds_the_top_among_several_narrow_windows() {
    // A tool of 1 mm on a cylinder of radius 20: the angles where a section
    // comes within its reach are about a degree wide, narrower than the
    // sampling step, and a line through the cylinder has two of them (where it
    // enters and where it leaves). The tool rests on the upper one.
    let c = Solid::cylinder(Point3::ORIGIN, dir(1.0, 0.0, 0.0), 30.0, 20.0).unwrap();
    let mesh = MeshSurface::new(&c.mesh(2880)).unwrap();
    let p = profile(ToolShape::BallEnd, 1.0);
    for tilt in [0.5f64, 1.0, 2.0, 2.2, 3.0, 4.0] {
        for y0 in [0.0, 0.1, 0.25, -0.2] {
            let t = tilt.to_radians();
            let line = ToolLine {
                origin: Point3::new(15.0, y0, 0.0),
                axis: dir(0.0, t.sin(), t.cos()),
            };
            let want = mesh.drop_tool(&p, &line).unwrap();
            let got = c
                .drop_tool(&p, &line)
                .unwrap_or_else(|| panic!("tilt {tilt} y0 {y0}: the solid was not reached"));
            let diff = got.offset - want.offset;
            assert!(
                (-1e-9..2e-3).contains(&diff),
                "tilt {tilt} y0 {y0}: {} against the mesh {}",
                got.offset,
                want.offset
            );
        }
    }
}

/// The held region of box stock: the stock's own box section over the
/// gripped length (decision record 0160).
fn held_box() -> Solid {
    Solid::cuboid(openchuck_types::geometry::Aabb {
        min: Point3::new(-12.0, -12.0, -9.0),
        max: Point3::new(-8.0, 12.0, 9.0),
    })
    .unwrap()
}

#[test]
fn req_set_09_a_box_solid_has_its_volume_area_bounds_and_outward_mesh() {
    let b = held_box();
    assert_eq!(b.volume(), 4.0 * 24.0 * 18.0);
    assert_eq!(
        b.surface_area(),
        2.0 * (4.0 * 24.0 + 24.0 * 18.0 + 18.0 * 4.0)
    );
    let bounds = b.bounds();
    assert_eq!(bounds.min, Point3::new(-12.0, -12.0, -9.0));
    assert_eq!(bounds.max, Point3::new(-8.0, 12.0, 9.0));
    assert_eq!(b.as_cuboid(), Some(bounds));
    assert_eq!(solids()[0].as_cuboid(), None);
    // Read as a solid along X through the corners of its section.
    assert_eq!((b.base().x, b.top().x, b.length()), (-12.0, -8.0, 4.0));
    assert_eq!(b.base_radius(), 12.0f64.hypot(9.0));
    assert!(b.contains(Point3::new(-10.0, 11.9, -8.9)));
    assert!(b.contains(Point3::new(-8.0, 12.0, 9.0)));
    assert!(!b.contains(Point3::new(-10.0, 12.1, 0.0)));
    assert!(!b.contains(Point3::new(-7.9, 0.0, 0.0)));
    let mesh = b.mesh(96);
    assert_eq!((mesh.vertices.len(), mesh.triangles.len()), (8, 12));
    assert!((mesh_volume(&mesh) - b.volume()).abs() < 1e-9);
}

#[test]
fn req_set_09_an_impossible_box_is_refused_with_a_message() {
    let flat = Solid::cuboid(openchuck_types::geometry::Aabb {
        min: Point3::new(0.0, 0.0, 0.0),
        max: Point3::new(4.0, 0.0, 2.0),
    });
    let message = flat.unwrap_err().to_string();
    assert!(message.contains("width along Y"), "{message}");
    let endless = Solid::cuboid(openchuck_types::geometry::Aabb {
        min: Point3::new(0.0, 0.0, 0.0),
        max: Point3::new(f64::INFINITY, 1.0, 1.0),
    });
    assert!(endless.is_err());
}

#[test]
fn req_set_09_a_box_answers_as_its_own_mesh_does() {
    let b = held_box();
    let mesh = MeshSurface::new(&b.mesh(0)).unwrap();
    // Exact cases: down onto the top face, along X onto the end face, from
    // inside out through a side, and closest points outside, inside and past
    // an edge.
    let down = b
        .cast_ray(Point3::new(-10.0, 0.0, 20.0), dir(0.0, 0.0, -1.0), 100.0)
        .unwrap();
    assert_eq!((down.distance, down.normal.get().z), (11.0, 1.0));
    let along = b
        .cast_ray(Point3::new(-30.0, 1.0, 1.0), dir(1.0, 0.0, 0.0), 100.0)
        .unwrap();
    assert_eq!((along.distance, along.normal.get().x), (18.0, -1.0));
    let out = b
        .cast_ray(Point3::new(-10.0, 0.0, 0.0), dir(0.0, 1.0, 0.0), 100.0)
        .unwrap();
    assert_eq!((out.distance, out.normal.get().y), (12.0, 1.0));
    assert!(
        b.cast_ray(Point3::new(-10.0, 0.0, 20.0), dir(0.0, 0.0, -1.0), 5.0)
            .is_none()
    );
    assert!(
        b.cast_ray(Point3::new(-10.0, 30.0, 20.0), dir(0.0, 0.0, -1.0), 100.0)
            .is_none()
    );
    let inside = b.project(Point3::new(-11.0, 0.0, 0.0)).unwrap();
    assert_eq!(inside.point, Point3::new(-12.0, 0.0, 0.0));
    assert_eq!(inside.normal.get().x, -1.0);
    let past = b.project(Point3::new(-5.0, 15.0, 0.0)).unwrap();
    assert_eq!(past.point, Point3::new(-8.0, 12.0, 0.0));
    assert!((past.normal.get().x - 0.5f64.sqrt()).abs() < 1e-12);
    // At random, the box agrees with the mesh of its own twelve triangles.
    let tools = [
        profile(ToolShape::BallEnd, 6.0),
        profile(ToolShape::FlatEnd, 5.0),
    ];
    let mut rng = Lcg(41);
    let bb = b.bounds();
    let (mut drops, mut rays) = (0, 0);
    for k in 0..300 {
        let o = Point3::new(
            rng.between(bb.min.x - 10.0, bb.max.x + 10.0),
            rng.between(bb.min.y - 10.0, bb.max.y + 10.0),
            rng.between(bb.min.z - 10.0, bb.max.z + 10.0),
        );
        let p = b.project(o).unwrap();
        let q = mesh.project(o).unwrap();
        assert!(
            (p.point.distance(o) - q.point.distance(o)).abs() < 1e-9,
            "{o:?}"
        );
        let target = Point3::new(
            rng.between(bb.min.x, bb.max.x),
            rng.between(bb.min.y, bb.max.y),
            rng.between(bb.min.z, bb.max.z),
        );
        if let Ok(d) = UnitVec3::new(target - o)
            && let (Some(g), Some(w)) = (b.cast_ray(o, d, 1e6), mesh.cast_ray(o, d, 1e6))
        {
            assert!((g.distance - w.distance).abs() < 1e-9, "{o:?} {d:?}");
            rays += 1;
        }
        let a = rng.between(0.0, std::f64::consts::TAU);
        let line = ToolLine {
            origin: o,
            axis: dir(0.0, a.sin(), a.cos()),
        };
        let tool = &tools[k % 2];
        match (b.drop_tool(tool, &line), mesh.drop_tool(tool, &line)) {
            (Some(g), Some(w)) => {
                assert!((g.offset - w.offset).abs() < 1e-9, "{line:?}");
                drops += 1;
            }
            (None, None) => {}
            (g, w) => panic!("box {g:?} against mesh {w:?} for {line:?}"),
        }
    }
    assert!(drops > 50 && rays > 50, "{drops} drops, {rays} rays");
}
