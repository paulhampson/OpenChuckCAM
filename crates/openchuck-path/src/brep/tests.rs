// SPDX-License-Identifier: AGPL-3.0-or-later
//! The bounds the exact queries rely on (lane STEPALL-X1, issue #348;
//! requirement IMP-02, NFR-05): each is checked against the surface it
//! bounds, evaluated independently, on rational patches with uneven knots.
#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::cast_precision_loss
)]

use openchuck_types::brep::BSplinePatch;
use openchuck_types::geometry::Point3;

use super::{Nurbs, hull};

/// A deterministic sequence in [0, 1).
fn lcg(seed: &mut u64) -> f64 {
    *seed = seed
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    (*seed >> 11) as f64 / (1u64 << 53) as f64
}

/// A clamped knot vector of `n` poles and degree `p` with uneven interior
/// knots, one of them doubled when `p` allows.
fn knots(n: usize, p: usize, seed: &mut u64) -> Vec<f64> {
    let inner = n - p - 1;
    let mut k: Vec<f64> = (0..inner).map(|_| 0.05 + 0.9 * lcg(seed)).collect();
    k.sort_by(f64::total_cmp);
    if p >= 2 && inner >= 2 {
        k[1] = k[0];
    }
    let mut out = vec![0.0; p + 1];
    out.extend(k);
    out.extend(vec![1.0; p + 1]);
    out
}

fn patch(seed: &mut u64) -> BSplinePatch {
    let pu = 1 + (lcg(seed) * 4.0) as usize;
    let pv = 1 + (lcg(seed) * 4.0) as usize;
    let nu = pu + 1 + (lcg(seed) * 4.0) as usize;
    let nv = pv + 1 + (lcg(seed) * 4.0) as usize;
    let mut poles = Vec::new();
    let mut weights = Vec::new();
    for i in 0..nu {
        for j in 0..nv {
            poles.push(Point3::new(
                10.0 * i as f64 + 3.0 * lcg(seed),
                10.0 * j as f64 + 3.0 * lcg(seed),
                8.0 * lcg(seed),
            ));
            weights.push(0.3 + 2.0 * lcg(seed));
        }
    }
    BSplinePatch {
        degree_u: u32::try_from(pu).unwrap(),
        degree_v: u32::try_from(pv).unwrap(),
        knots_u: knots(nu, pu, seed),
        knots_v: knots(nv, pv, seed),
        poles_u: u32::try_from(nu).unwrap(),
        poles_v: u32::try_from(nv).unwrap(),
        poles,
        weights,
    }
}

#[test]
fn req_imp_02_derivative_bounds_hold_over_the_patch() {
    let mut seed = 3;
    for _ in 0..40 {
        let p = patch(&mut seed);
        let n = Nurbs::new(0, &p).unwrap();
        let b = n.bounds;
        let h = 1e-6;
        let (mut su, mut sv, mut suu, mut suv, mut svv) =
            (0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64);
        for i in 0..=40 {
            for j in 0..=40 {
                let (u, v) = (
                    h + (1.0 - 2.0 * h) * i as f64 / 40.0,
                    h + (1.0 - 2.0 * h) * j as f64 / 40.0,
                );
                let (_, du, dv) = n.d1(u, v).unwrap();
                let (_, du1, dv1) = n.d1(u + h, v).unwrap();
                let (_, du0, dv0) = n.d1(u - h, v).unwrap();
                let (_, du2, _) = n.d1(u, v + h).unwrap();
                let (_, du3, _) = n.d1(u, v - h).unwrap();
                let (_, _, dv2) = n.d1(u, v + h).unwrap();
                let (_, _, dv3) = n.d1(u, v - h).unwrap();
                su = su.max(du.length());
                sv = sv.max(dv.length());
                suu = suu.max(((du1 - du0) * (0.5 / h)).length());
                suv = suv
                    .max(((du2 - du3) * (0.5 / h)).length())
                    .max(((dv1 - dv0) * (0.5 / h)).length());
                svv = svv.max(((dv2 - dv3) * (0.5 / h)).length());
            }
        }
        // Differences across a knot can exceed the one-sided derivatives by
        // their rounding: allow a hair.
        let ok = |got: f64, bound: f64| got <= bound * (1.0 + 1e-6) + 1e-6;
        assert!(ok(su, b.u), "|S_u| {su} over {}", b.u);
        assert!(ok(sv, b.v), "|S_v| {sv} over {}", b.v);
        assert!(ok(suu, b.uu), "|S_uu| {suu} over {}", b.uu);
        assert!(ok(suv, b.uv), "|S_uv| {suv} over {}", b.uv);
        assert!(ok(svv, b.vv), "|S_vv| {svv} over {}", b.vv);
    }
}

#[test]
fn req_imp_02_hull_cells_tile_the_box_and_hold_the_surface() {
    let mut seed = 17;
    for _ in 0..30 {
        let p = patch(&mut seed);
        let n = Nurbs::new(0, &p).unwrap();
        let clip = [
            0.1 + 0.2 * lcg(&mut seed),
            0.7 + 0.3 * lcg(&mut seed),
            0.0,
            0.6 + 0.4 * lcg(&mut seed),
        ];
        let tree = hull::HullTree::new(
            (n.pu, n.pv),
            (n.nu, n.nv),
            (&n.uk, &n.vk),
            &n.poles,
            &n.w,
            clip,
            5.0,
            6,
        )
        .unwrap();
        let mut area = 0.0;
        for node in tree.nodes.iter().filter(|x| x.net.is_some()) {
            let [u0, u1, v0, v1] = node.uv;
            assert!(u0 >= clip[0] - 1e-15 && u1 <= clip[1] + 1e-15);
            assert!(v0 >= clip[2] - 1e-15 && v1 <= clip[3] + 1e-15);
            area += (u1 - u0) * (v1 - v0);
            let net = &tree.nets[node.net.unwrap() as usize];
            for i in 0..=6 {
                for j in 0..=6 {
                    let (u, v) = (
                        u0 + (u1 - u0) * i as f64 / 6.0,
                        v0 + (v1 - v0) * j as f64 / 6.0,
                    );
                    let (s, _, _) = n.d1(u, v).unwrap();
                    // On the cell: its bounds and its parents' put it at no
                    // distance.
                    assert!(
                        node.bound.lower(s) <= 1e-9,
                        "a cell's bound misses its surface"
                    );
                    assert!(net.bound().lower(s) <= 1e-9);
                }
            }
            // The cell's own middle is the surface's.
            let (s, _, _) = n.d1(0.5 * (u0 + u1), 0.5 * (v0 + v1)).unwrap();
            assert!(net.centre().distance(s) < 1e-9);
        }
        let want = (clip[1] - clip[0]) * (clip[3] - clip[2]);
        assert!((area - want).abs() < 1e-12, "{area} against {want}");
        // An inner node's box holds its children's.
        for node in &tree.nodes {
            if let Some((a, b)) = node.children {
                for c in [a, b] {
                    let (outer, inner) = (node.bound.aabb, tree.nodes[c as usize].bound.aabb);
                    assert!(
                        outer.min.x <= inner.min.x
                            && outer.min.y <= inner.min.y
                            && outer.min.z <= inner.min.z
                    );
                    assert!(
                        outer.max.x >= inner.max.x
                            && outer.max.y >= inner.max.y
                            && outer.max.z >= inner.max.z
                    );
                }
            }
        }
    }
}
