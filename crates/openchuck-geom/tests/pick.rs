// SPDX-License-Identifier: AGPL-3.0-or-later
//! Ray-cast picking on meshes with known answers, and its time budget (UI-04;
//! verification plan sections 4.3 and 5; decision record 0036, OI-12). No
//! window is needed.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_cmp
)]

use std::time::Instant;

use openchuck_geom::pick::MeshPicker;
use openchuck_testkit::parts::{Cylinder, RadialPart, Sphere, benchmark_model_1m};
use openchuck_types::geometry::{Point3, Vec3};
use openchuck_types::mesh::TriangleMesh;

/// A closed 10 mm cube; faces are triangle pairs: bottom 0-1, top 2-3,
/// front (-y) 4-5, back (+y) 6-7, right (+x) 8-9, left (-x) 10-11.
fn cube() -> TriangleMesh {
    let v = |x: f64, y: f64, z: f64| Point3::new(x * 10.0, y * 10.0, z * 10.0);
    TriangleMesh {
        vertices: vec![
            v(0.0, 0.0, 0.0),
            v(1.0, 0.0, 0.0),
            v(1.0, 1.0, 0.0),
            v(0.0, 1.0, 0.0),
            v(0.0, 0.0, 1.0),
            v(1.0, 0.0, 1.0),
            v(1.0, 1.0, 1.0),
            v(0.0, 1.0, 1.0),
        ],
        triangles: vec![
            [0, 2, 1],
            [0, 3, 2],
            [4, 5, 6],
            [4, 6, 7],
            [0, 1, 5],
            [0, 5, 4],
            [2, 3, 7],
            [2, 7, 6],
            [1, 2, 6],
            [1, 6, 5],
            [0, 4, 7],
            [0, 7, 3],
        ],
    }
}

fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::new(x, y, z)
}

fn v(x: f64, y: f64, z: f64) -> Vec3 {
    Vec3 { x, y, z }
}

#[test]
fn req_ui_04_a_ray_picks_the_face_it_meets_from_each_side() {
    let picker = MeshPicker::new(&cube()).unwrap();
    // Aimed at a point of each face that is clear of the diagonal.
    let cases = [
        (p(2.0, 3.0, 20.0), v(0.0, 0.0, -1.0), [2, 3], 10.0),
        (p(2.0, 3.0, -20.0), v(0.0, 0.0, 1.0), [0, 1], 20.0),
        (p(3.0, -20.0, 7.0), v(0.0, 1.0, 0.0), [4, 5], 20.0),
        (p(3.0, 30.0, 7.0), v(0.0, -1.0, 0.0), [6, 7], 20.0),
        (p(30.0, 3.0, 7.0), v(-1.0, 0.0, 0.0), [8, 9], 20.0),
        (p(-20.0, 3.0, 7.0), v(1.0, 0.0, 0.0), [10, 11], 10.0 + 10.0),
    ];
    for (origin, direction, faces, distance) in cases {
        let hit = picker.pick(origin, direction).unwrap().unwrap();
        assert!(faces.contains(&hit.triangle), "{origin:?} hit {hit:?}");
        assert!((hit.distance - distance).abs() < 1e-9, "{hit:?}");
        let along = origin + direction * hit.distance;
        assert!(hit.point.distance(along) < 1e-9);
    }
}

#[test]
fn req_ui_04_a_ray_that_misses_or_points_away_picks_nothing() {
    let picker = MeshPicker::new(&cube()).unwrap();
    assert_eq!(
        picker.pick(p(50.0, 50.0, 20.0), v(0.0, 0.0, -1.0)).unwrap(),
        None
    );
    // Pointing away from the part.
    assert_eq!(
        picker.pick(p(5.0, 5.0, 20.0), v(0.0, 0.0, 1.0)).unwrap(),
        None
    );
}

#[test]
fn req_ui_04_the_nearest_surface_wins_and_a_ray_from_inside_meets_the_wall() {
    let picker = MeshPicker::new(&cube()).unwrap();
    // From above the first hit is the top face, not the bottom behind it.
    let hit = picker.pick(p(2.0, 3.0, 50.0), v(0.0, 0.0, -1.0)).unwrap();
    assert!([2, 3].contains(&hit.unwrap().triangle));
    // From inside the cube, looking up, it is the top face again.
    let inside = picker
        .pick(p(2.0, 3.0, 5.0), v(0.0, 0.0, 1.0))
        .unwrap()
        .unwrap();
    assert!([2, 3].contains(&inside.triangle));
    assert!((inside.distance - 5.0).abs() < 1e-9);
}

#[test]
fn req_ui_04_the_direction_need_not_be_a_unit_vector() {
    let picker = MeshPicker::new(&cube()).unwrap();
    let unit = picker.pick(p(2.0, 3.0, 20.0), v(0.0, 0.0, -1.0)).unwrap();
    let long = picker.pick(p(2.0, 3.0, 20.0), v(0.0, 0.0, -250.0)).unwrap();
    assert_eq!(unit, long);
    assert!((unit.unwrap().distance - 10.0).abs() < 1e-9);
}

#[test]
fn req_ui_04_a_ray_through_an_edge_gives_the_same_triangle_every_run() {
    let picker = MeshPicker::new(&cube()).unwrap();
    // Straight down the diagonal of the top face: two triangles share the line.
    let first = picker.pick(p(4.0, 4.0, 30.0), v(0.0, 0.0, -1.0)).unwrap();
    for _ in 0..20 {
        let again = MeshPicker::new(&cube())
            .unwrap()
            .pick(p(4.0, 4.0, 30.0), v(0.0, 0.0, -1.0))
            .unwrap();
        assert_eq!(first, again);
    }
    assert!([2, 3].contains(&first.unwrap().triangle));
}

#[test]
fn req_ui_04_bad_rays_and_meshes_are_errors_not_crashes() {
    let picker = MeshPicker::new(&cube()).unwrap();
    assert!(picker.pick(p(0.0, 0.0, 0.0), v(0.0, 0.0, 0.0)).is_err());
    assert!(
        picker
            .pick(p(f64::NAN, 0.0, 0.0), v(0.0, 0.0, 1.0))
            .is_err()
    );
    assert!(
        picker
            .pick(p(0.0, 0.0, 0.0), v(f64::INFINITY, 0.0, 1.0))
            .is_err()
    );
    let empty = TriangleMesh {
        vertices: vec![p(0.0, 0.0, 0.0)],
        triangles: Vec::new(),
    };
    assert!(MeshPicker::new(&empty).is_err());
    let bad_index = TriangleMesh {
        vertices: vec![p(0.0, 0.0, 0.0), p(1.0, 0.0, 0.0), p(0.0, 1.0, 0.0)],
        triangles: vec![[0, 1, 7]],
    };
    assert!(MeshPicker::new(&bad_index).is_err());
}

#[test]
fn req_ui_04_hits_lie_on_the_analytic_surface() {
    // A cylinder of radius 12.5 about X, 40 around: the facets lie inside the
    // true surface by at most r (1 - cos(pi / 40)), and a ray from above meets
    // them there.
    let cylinder = Cylinder {
        radius: 12.5,
        length: 80.0,
    };
    let mesh = cylinder.mesh(81, 40);
    let picker = MeshPicker::new(&mesh).unwrap();
    let sagitta = 12.5 * (1.0 - (std::f64::consts::PI / 40.0).cos());
    for i in 1..79 {
        let x = f64::from(i);
        let hit = picker
            .pick(p(x + 0.123, 0.0, 50.0), v(0.0, 0.0, -1.0))
            .unwrap()
            .unwrap();
        let radius = hit.point.y.hypot(hit.point.z);
        assert!(
            radius <= 12.5 + 1e-9 && radius >= 12.5 - sagitta - 1e-9,
            "{hit:?}"
        );
        // The hit is on the triangle it names.
        let [a, b, c] = mesh.triangle(hit.triangle as usize).unwrap();
        let n = (b - a).cross(c - a);
        let off = (hit.point - a).dot(n) / n.length();
        assert!(off.abs() < 1e-9, "{off}");
    }
    let sphere = Sphere { radius: 20.0 }.mesh(90, 180);
    let picker = MeshPicker::new(&sphere).unwrap();
    let hit = picker
        .pick(p(0.1, 0.2, 100.0), v(0.0, 0.0, -1.0))
        .unwrap()
        .unwrap();
    assert!((hit.point.z - 20.0).abs() < 0.05, "{hit:?}");
}

#[test]
fn req_ui_04_the_hierarchy_agrees_with_testing_every_triangle() {
    let mesh = Sphere { radius: 20.0 }.mesh(24, 48);
    let picker = MeshPicker::new(&mesh).unwrap();
    // A fixed grid of directions at the centre and from outside.
    let mut checked = 0;
    for i in 0..30 {
        for j in 0..30 {
            let a = f64::from(i) * 0.21 + 0.01;
            let b = f64::from(j) * 0.2 + 0.013;
            let d = v(a.sin() * b.cos(), a.sin() * b.sin(), a.cos());
            for origin in [p(0.0, 0.0, 0.0), p(60.0, 5.0, -3.0)] {
                let want = brute_force(&mesh, origin, d);
                let got = picker.pick(origin, d).unwrap();
                match (want, got) {
                    (None, None) => {}
                    (Some((_, distance)), Some(hit)) => {
                        assert!((distance - hit.distance).abs() < 1e-9);
                        checked += 1;
                    }
                    other => panic!("{origin:?} {d:?}: {other:?}"),
                }
            }
        }
    }
    assert!(checked > 600, "{checked}");
}

fn brute_force(mesh: &TriangleMesh, o: Point3, d: Vec3) -> Option<(usize, f64)> {
    let mut best: Option<(usize, f64)> = None;
    for i in 0..mesh.triangles.len() {
        let [a, b, c] = mesh.triangle(i)?;
        let e1 = b - a;
        let e2 = c - a;
        let pv = d.cross(e2);
        let det = e1.dot(pv);
        if det.abs() < 1e-300 {
            continue;
        }
        let s = o - a;
        let u = s.dot(pv) / det;
        let q = s.cross(e1);
        let w = d.dot(q) / det;
        if !(-1e-12..=1.0 + 1e-12).contains(&u) || w < -1e-12 || u + w > 1.0 + 1e-12 {
            continue;
        }
        let t = e2.dot(q) / det;
        if t >= 0.0 && best.is_none_or(|(_, bt)| t < bt) {
            best = Some((i, t));
        }
    }
    best
}

/// The pick budget (verification plan 4.3, decision record 0036): on a model
/// of one million triangles, a ray cast takes at most 1 ms at the 95th
/// percentile and building the hierarchy at most 2 s. Hosted CI runners are
/// noisy, so the limits are five times larger there (`CI` set); the strict
/// figures are checked locally and on the reference machines.
#[test]
fn req_ui_04_picking_a_million_triangles_stays_within_its_budget() {
    let relax = if std::env::var_os("CI").is_some() {
        5.0
    } else {
        1.0
    };
    let mesh = benchmark_model_1m();
    assert!(mesh.triangles.len() >= 1_000_000);

    let started = Instant::now();
    let picker = MeshPicker::new(&mesh).unwrap();
    let build = started.elapsed().as_secs_f64();
    assert!(
        build <= 2.0 * relax,
        "building the hierarchy took {build:.3} s"
    );

    // 1,024 rays on a fixed grid, from outside the part towards its axis.
    let mut times = Vec::with_capacity(1024);
    let mut hits = 0;
    for i in 0..32 {
        for j in 0..32 {
            let angle = std::f64::consts::TAU * f64::from(i) / 32.0 + 0.0137;
            let x = 100.0 * (f64::from(j) + 0.37) / 32.0;
            let origin = p(x, 80.0 * angle.cos(), 80.0 * angle.sin());
            let direction = v(0.0, -angle.cos(), -angle.sin());
            let t = Instant::now();
            let hit = picker.pick(origin, direction).unwrap();
            times.push(t.elapsed().as_secs_f64());
            if let Some(hit) = hit {
                hits += 1;
                let radius = hit.point.y.hypot(hit.point.z);
                assert!((radius - 25.0).abs() < 0.01, "{hit:?}");
            }
        }
    }
    assert!(hits > 900, "{hits} of 1024 rays hit the part");
    times.sort_by(f64::total_cmp);
    let p95 = times[(times.len() * 95).div_ceil(100) - 1];
    eprintln!(
        "pick budget: build {build:.3} s, ray cast p95 {:.1} us over {} rays on {} triangles",
        p95 * 1e6,
        times.len(),
        mesh.triangles.len()
    );
    assert!(p95 <= 1e-3 * relax, "p95 ray cast {:.1} us", p95 * 1e6);
}
