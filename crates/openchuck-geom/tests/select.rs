// SPDX-License-Identifier: AGPL-3.0-or-later
//! Triangle selection on meshes with known answers (UI-04; verification plan
//! section 5, "Geometry and selection").

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_cmp
)]

use openchuck_geom::edges::EdgeMap;
use openchuck_geom::select::{
    TriangleSet, flood_fill, flood_fill_by_angle, select_by_direction, triangle_normals,
};
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

/// Two separate unit squares in the z = 0 plane, 2 triangles each.
fn two_plates() -> TriangleMesh {
    let p = |x: f64, y: f64| Point3::new(x, y, 0.0);
    TriangleMesh {
        vertices: vec![
            p(0.0, 0.0),
            p(1.0, 0.0),
            p(1.0, 1.0),
            p(0.0, 1.0),
            p(5.0, 0.0),
            p(6.0, 0.0),
            p(6.0, 1.0),
            p(5.0, 1.0),
        ],
        triangles: vec![[0, 1, 2], [0, 2, 3], [4, 5, 6], [4, 6, 7]],
    }
}

fn set(items: &[u32]) -> TriangleSet {
    items.iter().copied().collect()
}

fn vec3(x: f64, y: f64, z: f64) -> Vec3 {
    Vec3 { x, y, z }
}

#[test]
fn req_ui_04_flood_fill_covers_a_whole_shell_and_only_that_shell() {
    let cube = cube();
    let edges = EdgeMap::new(&cube).unwrap();
    assert_eq!(flood_fill(&cube, &edges, &[7]).unwrap().len(), 12);

    let plates = two_plates();
    let edges = EdgeMap::new(&plates).unwrap();
    assert_eq!(flood_fill(&plates, &edges, &[0]).unwrap(), set(&[0, 1]));
    assert_eq!(flood_fill(&plates, &edges, &[3]).unwrap(), set(&[2, 3]));
    // Several seeds join their shells.
    assert_eq!(flood_fill(&plates, &edges, &[1, 2]).unwrap().len(), 4);
}

#[test]
fn req_ui_04_flood_fill_by_angle_stops_at_a_sharp_edge() {
    let cube = cube();
    let edges = EdgeMap::new(&cube).unwrap();
    // The cube's edges are 90 degrees, so one face is its two triangles.
    let faces = [
        (0, [0, 1]),
        (3, [2, 3]),
        (4, [4, 5]),
        (7, [6, 7]),
        (8, [8, 9]),
        (11, [10, 11]),
    ];
    for (seed, face) in faces {
        assert_eq!(
            flood_fill_by_angle(&cube, &edges, &[seed], 30.0).unwrap(),
            set(&face),
            "seed {seed}"
        );
    }
    // At exactly the edge angle the step is taken; below it, it is not.
    let count = |angle| {
        flood_fill_by_angle(&cube, &edges, &[0], angle)
            .unwrap()
            .len()
    };
    assert_eq!(count(90.0), 12);
    assert_eq!(count(89.9), 2);
    assert_eq!(count(180.0), 12);
}

#[test]
fn req_ui_04_flood_fill_by_angle_follows_a_smooth_ramp() {
    // A strip of five quads along X, bent 10 degrees at each joint except
    // the last, which bends 80.
    let mut vertices = vec![Point3::new(0.0, 0.0, 0.0), Point3::new(0.0, 1.0, 0.0)];
    let mut triangles = Vec::new();
    let (mut x, mut z, mut angle) = (0.0_f64, 0.0_f64, 0.0_f64);
    for k in 0..5_u32 {
        angle += if k == 4 { 80.0 } else { 10.0 };
        x += angle.to_radians().cos();
        z += angle.to_radians().sin();
        vertices.push(Point3::new(x, 0.0, z));
        vertices.push(Point3::new(x, 1.0, z));
        let a = 2 * k;
        triangles.push([a, a + 2, a + 3]);
        triangles.push([a, a + 3, a + 1]);
    }
    let mesh = TriangleMesh {
        vertices,
        triangles,
    };
    let edges = EdgeMap::new(&mesh).unwrap();
    // Joints of 10 degrees are crossed at a 15 degree limit, the 80 is not.
    let smooth = flood_fill_by_angle(&mesh, &edges, &[0], 15.0).unwrap();
    assert_eq!(smooth, (0..8).collect::<TriangleSet>());
    let all = flood_fill_by_angle(&mesh, &edges, &[0], 80.0).unwrap();
    assert_eq!(all.len(), 10);
}

#[test]
fn req_ui_04_select_by_direction_picks_the_faces_within_the_cone() {
    let cube = cube();
    let up = vec3(0.0, 0.0, 1.0);
    assert_eq!(select_by_direction(&cube, up, 10.0).unwrap(), set(&[2, 3]));
    assert_eq!(
        select_by_direction(&cube, vec3(1.0, 0.0, 0.0), 0.0).unwrap(),
        set(&[8, 9])
    );
    // 90 degrees takes every face but the one pointing away; 180 takes all.
    assert_eq!(select_by_direction(&cube, up, 90.0).unwrap().len(), 10);
    assert_eq!(select_by_direction(&cube, up, 180.0).unwrap().len(), 12);
    // The direction need not be a unit vector.
    assert_eq!(
        select_by_direction(&cube, vec3(0.0, 0.0, 40.0), 10.0).unwrap(),
        set(&[2, 3])
    );
}

#[test]
fn req_ui_04_degenerate_triangles_are_never_crossed() {
    let mut mesh = two_plates();
    // A sliver on the diagonal of plate one.
    mesh.triangles.push([0, 2, 2]);
    let edges = EdgeMap::new(&mesh).unwrap();
    assert!(triangle_normals(&mesh)[4].is_none());
    let up = vec3(0.0, 0.0, 1.0);
    assert_eq!(select_by_direction(&mesh, up, 180.0).unwrap().len(), 4);
    // Seeded on the sliver, an angle-limited fill stays on it.
    assert_eq!(
        flood_fill_by_angle(&mesh, &edges, &[4], 10.0).unwrap(),
        set(&[4])
    );
}

#[test]
fn req_ui_04_req_nfr_07_bad_input_is_an_error() {
    let cube = cube();
    let edges = EdgeMap::new(&cube).unwrap();
    assert!(flood_fill(&cube, &edges, &[12]).is_err());
    for angle in [-1.0, f64::NAN, 181.0] {
        assert!(flood_fill_by_angle(&cube, &edges, &[0], angle).is_err());
    }
    assert!(select_by_direction(&cube, vec3(0.0, 0.0, 0.0), 10.0).is_err());
    assert!(select_by_direction(&cube, vec3(f64::NAN, 0.0, 1.0), 10.0).is_err());
    assert!(select_by_direction(&cube, vec3(0.0, 0.0, 1.0), 200.0).is_err());
    // No seeds select nothing.
    assert!(flood_fill(&cube, &edges, &[]).unwrap().is_empty());
}

#[test]
fn req_ui_04_a_submesh_keeps_the_selected_triangles_in_order() {
    use openchuck_geom::select::submesh;
    let cube = cube();
    let top = submesh(&cube, &set(&[3, 2])).unwrap();
    assert_eq!(top.triangles, vec![cube.triangles[2], cube.triangles[3]]);
    assert_eq!(top.vertices, cube.vertices);
    assert!(submesh(&cube, &set(&[12])).is_err());
    assert!(
        submesh(&cube, &TriangleSet::new())
            .unwrap()
            .triangles
            .is_empty()
    );
}
