// SPDX-License-Identifier: AGPL-3.0-or-later
//! Analytic test parts (NFR-08; work breakdown slice 0 item 10).

// Test code may unwrap, index and panic (NFR-07 applies to library code).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_cmp
)]

use std::collections::BTreeMap;

use openchuck_testkit::parts::{RadialPart, RuledHyperboloid, benchmark_model_1m, standard_parts};

/// Every edge is shared by exactly two triangles, in opposite directions.
fn assert_closed(mesh: &openchuck_types::mesh::TriangleMesh, name: &str) {
    let mut edges: BTreeMap<(u32, u32), i32> = BTreeMap::new();
    for t in &mesh.triangles {
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            assert_ne!(a, b, "{name}: degenerate triangle {t:?}");
            *edges.entry((a.min(b), a.max(b))).or_default() += if a < b { 1 } else { -1 };
        }
    }
    for (edge, balance) in edges {
        assert_eq!(
            balance, 0,
            "{name}: edge {edge:?} is open or inconsistently oriented"
        );
    }
}

#[test]
fn req_nfr_08_parts_lie_on_their_analytic_surface() {
    for part in standard_parts() {
        let mesh = part.mesh(41, 64);
        mesh.validate().unwrap();
        let (x0, x1) = part.x_range();
        for v in &mesh.vertices {
            if v.y == 0.0 && v.z == 0.0 {
                continue; // cap centres and poles
            }
            let angle = v.z.atan2(v.y);
            let r = v.y.hypot(v.z);
            assert!(v.x >= x0 - 1e-12 && v.x <= x1 + 1e-12);
            let exact = part.radius_at(v.x, angle);
            assert!(
                (r - exact).abs() < 1e-9,
                "{}: r {r} vs {exact}",
                part.name()
            );
        }
        assert_closed(&mesh, part.name());
        // Outward normals give a positive signed volume.
        let volume: f64 = (0..mesh.triangles.len())
            .map(|i| {
                let [p, q, r] = mesh.triangle(i).unwrap();
                p.to_vec().dot(q.to_vec().cross(r.to_vec())) / 6.0
            })
            .sum();
        assert!(volume > 0.0, "{}: normals point inwards", part.name());
    }
}

#[test]
fn req_nfr_08_ruled_surface_contains_its_rulings() {
    let part = RuledHyperboloid {
        end_radius: 15.0,
        length: 40.0,
        twist: 1.0,
    };
    for k in 0..12 {
        let (p, q) = part.ruling(f64::from(k) * 0.5);
        for i in 0..=10 {
            let t = f64::from(i) / 10.0;
            let x = p.x + (q.x - p.x) * t;
            let y = p.y + (q.y - p.y) * t;
            let z = p.z + (q.z - p.z) * t;
            let exact = part.radius_at(x, z.atan2(y));
            assert!((y.hypot(z) - exact).abs() < 1e-9);
        }
    }
}

#[test]
fn req_nfr_03_benchmark_model_has_about_one_million_triangles() {
    let mesh = benchmark_model_1m();
    assert_eq!(mesh.triangles.len(), 1_002_000);
    mesh.validate().unwrap();
}
