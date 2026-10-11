// SPDX-License-Identifier: AGPL-3.0-or-later
//! Mesh checks on meshes with known, seeded defects (IMP-04; verification
//! plan section 5, "Meshes with known defects").

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_cmp
)]

use openchuck_geom::check::{MeshReport, check_mesh};
use openchuck_geom::edges::EdgeMap;
use openchuck_testkit::parts::standard_parts;
use openchuck_types::geometry::{Point3, Vec3};
use openchuck_types::mesh::TriangleMesh;
use proptest::prelude::*;

/// A closed 10 mm cube with outward (anticlockwise from outside) triangles.
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
            [0, 3, 2], // bottom, -z
            [4, 5, 6],
            [4, 6, 7], // top, +z
            [0, 1, 5],
            [0, 5, 4], // front, -y
            [2, 3, 7],
            [2, 7, 6], // back, +y
            [1, 2, 6],
            [1, 6, 5], // right, +x
            [0, 4, 7],
            [0, 7, 3], // left, -x
        ],
    }
}

fn check(mesh: &TriangleMesh) -> MeshReport {
    check_mesh(mesh, None).unwrap()
}

#[test]
fn req_imp_04_clean_meshes_give_no_report() {
    let report = check(&cube());
    assert!(report.is_clean(), "{report:?}");
    assert!(report.problems().is_empty());
    assert_eq!(
        (report.triangles, report.vertices, report.edges),
        (12, 8, 18)
    );
    assert_eq!(report.shells, 1);
    assert!((report.volume.unwrap() - 1000.0).abs() < 1e-9);

    for part in standard_parts() {
        let mesh = part.mesh(40, 48);
        let report = check(&mesh);
        assert!(
            report.is_clean(),
            "{}: {:?}",
            part.name(),
            report.problems()
        );
        assert!(report.volume.unwrap() > 0.0, "{}", part.name());
    }
}

#[test]
fn req_imp_04_holes_are_counted() {
    // One triangle removed: one hole of three open edges.
    let mut mesh = cube();
    mesh.triangles.remove(2);
    let report = check(&mesh);
    assert_eq!(report.holes.len(), 1);
    assert_eq!(report.boundary_edges(), 3);
    assert_eq!(report.volume, None);
    assert!(report.flipped_triangles.is_empty());

    // A whole face removed: still one hole, four open edges.
    let mut mesh = cube();
    mesh.triangles.drain(2..4);
    let report = check(&mesh);
    assert_eq!(report.holes.len(), 1);
    assert_eq!(report.boundary_edges(), 4);

    // Top and bottom removed: two separate holes.
    let mut mesh = cube();
    mesh.triangles.drain(0..4);
    let report = check(&mesh);
    assert_eq!(report.holes.len(), 2);
    assert_eq!(report.boundary_edges(), 8);
    assert_eq!(report.problems().len(), 1, "{:?}", report.problems());

    // A gap: two vertices at the same place are not joined, so the seam is open.
    let mut mesh = cube();
    mesh.vertices.push(mesh.vertices[6]);
    let duplicate = (mesh.vertices.len() - 1) as u32;
    mesh.triangles[2] = [4, 5, duplicate];
    let report = check(&mesh);
    assert!(!report.holes.is_empty());
}

#[test]
fn req_imp_04_non_manifold_edges_are_counted() {
    // A fin on the edge 0-1 makes that edge shared by three triangles; the
    // fin's other two edges are open.
    let mut mesh = cube();
    mesh.vertices.push(Point3::new(5.0, -10.0, -10.0));
    mesh.triangles.push([0, 1, 8]);
    let report = check(&mesh);
    assert_eq!(report.non_manifold_edges, vec![[0, 1]]);
    assert_eq!(report.holes.len(), 1);
    assert_eq!(report.boundary_edges(), 2);
    assert_eq!(report.volume, None);
}

#[test]
fn req_imp_04_flipped_normals_are_counted() {
    // One triangle wound the wrong way.
    let mut mesh = cube();
    mesh.triangles[5] = [0, 4, 5];
    let report = check(&mesh);
    assert_eq!(report.flipped_triangles, vec![5]);
    assert_eq!(report.inside_out_shells, 0);
    assert!(report.holes.is_empty());

    // The seed triangle itself flipped: still only that one is reported.
    let mut mesh = cube();
    mesh.triangles[0] = [0, 1, 2];
    assert_eq!(check(&mesh).flipped_triangles, vec![0]);

    // The whole part inside out: every triangle flipped, one inside-out shell.
    let mut mesh = cube();
    for t in &mut mesh.triangles {
        t.swap(1, 2);
    }
    let report = check(&mesh);
    assert_eq!(report.flipped_triangles, (0..12).collect::<Vec<_>>());
    assert_eq!(report.inside_out_shells, 1);
    assert!(report.problems().iter().any(|p| p.contains("inside out")));
}

#[test]
fn req_imp_04_stated_normals_against_the_winding_are_counted() {
    let mesh = cube();
    let mut normals: Vec<Vec3> = (0..12)
        .map(|t| {
            let [a, b, c] = mesh.triangle(t).unwrap();
            (b - a).cross(c - a)
        })
        .collect();
    assert!(check_mesh(&mesh, Some(&normals)).unwrap().is_clean());
    normals[3] = -normals[3];
    normals[7] = Vec3::ZERO; // not stated: ignored
    normals[8] = Vec3::new(f64::NAN, 0.0, 0.0); // unusable: ignored
    let report = check_mesh(&mesh, Some(&normals)).unwrap();
    assert_eq!(report.normal_mismatches, vec![3]);
    assert!(report.flipped_triangles.is_empty());
}

#[test]
fn req_imp_04_degenerate_triangles_are_counted() {
    let mut mesh = cube();
    // Collinear corners on the edge 0-1, and a repeated vertex.
    mesh.vertices.push(Point3::new(5.0, 0.0, 0.0));
    mesh.triangles.push([0, 8, 1]);
    mesh.triangles.push([2, 2, 6]);
    let report = check(&mesh);
    assert_eq!(report.degenerate_triangles, vec![12, 13]);

    // A very thin but real sliver is not degenerate.
    let sliver = TriangleMesh {
        vertices: vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(100.0, 0.0, 0.0),
            Point3::new(50.0, 1e-6, 0.0),
        ],
        triangles: vec![[0, 1, 2]],
    };
    assert!(check(&sliver).degenerate_triangles.is_empty());
}

#[test]
fn req_imp_04_every_defect_in_one_mesh() {
    let mut mesh = cube();
    mesh.triangles.remove(11); // hole (left face, second triangle)
    mesh.triangles[4] = [0, 5, 1]; // flipped
    mesh.vertices.push(Point3::new(5.0, 5.0, 20.0));
    mesh.triangles.push([4, 5, 8]); // fin on 4-5 (top and front already use it)
    mesh.vertices.push(Point3::new(30.0, 30.0, 30.0));
    mesh.triangles.push([9, 9, 9]); // degenerate, and adds no edge
    let report = check(&mesh);
    assert_eq!(report.non_manifold_edges, vec![[4, 5]]);
    assert_eq!(report.degenerate_triangles, vec![12]);
    assert_eq!(report.flipped_triangles, vec![4]);
    assert_eq!(report.holes.len(), 2, "{:?}", report.holes);
    assert_eq!(report.problems().len(), 4, "{:?}", report.problems());
}

#[test]
fn req_imp_04_nfr_07_bad_indices_are_an_error() {
    let mut mesh = cube();
    mesh.triangles.push([0, 1, 99]);
    let err = check_mesh(&mesh, None).unwrap_err();
    assert!(err.to_string().contains("out of range"), "{err}");
    assert!(EdgeMap::new(&mesh).is_err());
    // An empty mesh is clean and has no volume.
    let report = check(&TriangleMesh::default());
    assert!(report.is_clean());
    assert_eq!(report.volume, None);
}

#[test]
fn edge_map_numbers_edges_by_vertex_pair() {
    let map = EdgeMap::new(&cube()).unwrap();
    assert_eq!(map.len(), 18);
    let edges = map.edges();
    assert!(edges.windows(2).all(|w| w[0] < w[1]));
    assert!(edges.iter().all(|[a, b]| a < b));
    for e in 0..map.len() {
        let uses = map.uses(e);
        assert_eq!(uses.len(), 2);
        assert_ne!(uses[0].forward, uses[1].forward);
    }
    // Every triangle side maps to the edge joining its corners.
    let mesh = cube();
    for (t, corners) in mesh.triangles.iter().enumerate() {
        for (k, edge) in map.sides(t).into_iter().enumerate() {
            let [a, b] = map.edge(edge.unwrap() as usize).unwrap();
            let (c, d) = (corners[k], corners[(k + 1) % 3]);
            assert_eq!([a, b], [c.min(d), c.max(d)]);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Removing any set of triangles and reversing any set never crashes, and
    /// the flipped triangles of a closed mesh are exactly the reversed ones
    /// when fewer than half are reversed (NFR-07, IMP-04).
    #[test]
    fn req_imp_04_reversed_triangles_are_found(reversed in proptest::collection::btree_set(0usize..12, 0..6)) {
        let mut mesh = cube();
        for &t in &reversed {
            mesh.triangles[t].swap(1, 2);
        }
        let report = check(&mesh);
        let expected: Vec<u32> = reversed.iter().map(|&t| t as u32).collect();
        prop_assert_eq!(report.flipped_triangles, expected);
        prop_assert!(report.holes.is_empty());
    }

    #[test]
    fn req_nfr_07_any_triangle_soup_is_checked_without_panic(
        vertices in proptest::collection::vec((-5.0f64..5.0, -5.0f64..5.0, -5.0f64..5.0), 1..12),
        triangles in proptest::collection::vec((0u32..12, 0u32..12, 0u32..12), 0..30),
    ) {
        let n = vertices.len() as u32;
        let mesh = TriangleMesh {
            vertices: vertices.into_iter().map(|(x, y, z)| Point3::new(x, y, z)).collect(),
            triangles: triangles.into_iter().map(|(a, b, c)| [a % n, b % n, c % n]).collect(),
        };
        let first = check(&mesh);
        prop_assert_eq!(&first, &check(&mesh));
        prop_assert!(first.flipped_triangles.windows(2).all(|w| w[0] < w[1]));
    }
}
