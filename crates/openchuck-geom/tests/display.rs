// SPDX-License-Identifier: AGPL-3.0-or-later
//! Feature edges for the viewport's edge display (UI-02; decision record 0026
//! item 4).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_cmp
)]

use openchuck_geom::display::{DEFAULT_CREASE_DEGREES, feature_edges};
use openchuck_testkit::parts::standard_parts;
use openchuck_types::geometry::Point3;
use openchuck_types::mesh::TriangleMesh;

/// A closed 10 mm cube with outward triangles.
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

#[test]
fn req_ui_02_cube_feature_edges_are_its_twelve_edges() {
    // The six face diagonals are flat and left out.
    let edges = feature_edges(&cube(), DEFAULT_CREASE_DEGREES).unwrap();
    assert_eq!(edges.len(), 12);
    for [a, b] in edges {
        let (p, q) = (cube().vertices[a as usize], cube().vertices[b as usize]);
        let differing = [p.x != q.x, p.y != q.y, p.z != q.z]
            .iter()
            .filter(|d| **d)
            .count();
        assert_eq!(differing, 1, "edge {a}-{b} is a diagonal");
    }
}

#[test]
fn req_ui_02_open_edges_are_feature_edges() {
    let single = TriangleMesh {
        vertices: vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
        ],
        triangles: vec![[0, 1, 2]],
    };
    assert_eq!(feature_edges(&single, 179.0).unwrap().len(), 3);
}

#[test]
fn req_ui_02_smooth_parts_show_few_edges() {
    // A finely tessellated smooth part shows its silhouette-defining creases
    // (the cap rims), not its tessellation.
    for part in standard_parts() {
        let mesh = part.mesh(40, 120);
        let all = openchuck_geom::edges::EdgeMap::new(&mesh).unwrap().len();
        let features = feature_edges(&mesh, DEFAULT_CREASE_DEGREES).unwrap();
        assert!(
            features.len() * 10 < all,
            "{} of {all} edges are feature edges",
            features.len()
        );
    }
}

#[test]
fn req_ui_02_crease_angle_limits() {
    // At 0 degrees every edge between non-coplanar triangles counts; at 180
    // only open and non-manifold edges do.
    let mesh = cube();
    assert_eq!(feature_edges(&mesh, 0.0).unwrap().len(), 12);
    assert!(feature_edges(&mesh, 180.0).unwrap().is_empty());
    assert!(feature_edges(&mesh, f64::NAN).is_err());
}

#[test]
fn req_nfr_06_feature_edges_are_deterministic() {
    let mesh = standard_parts()[0].mesh(30, 90);
    let first = feature_edges(&mesh, DEFAULT_CREASE_DEGREES).unwrap();
    for _ in 0..3 {
        assert_eq!(feature_edges(&mesh, DEFAULT_CREASE_DEGREES).unwrap(), first);
    }
}
