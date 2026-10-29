// SPDX-License-Identifier: AGPL-3.0-or-later
//! Bounds on the first and second derivatives of a rational B-spline patch
//! over its whole domain (lane STEPALL-X1, issue #348; decision 0211 item
//! 3.2), for the boundary search's bounds on a trim segment's image.
//!
//! In homogeneous form `H = (w P, w)` the patch is `A / W` with `A` and `W`
//! polynomial B-splines. Differentiating `W S = A`:
//!
//! - `S_u = (A_u - W_u S) / W`,
//! - `S_uu = (A_uu - W_uu S - 2 W_u S_u) / W`,
//! - `S_uv = (A_uv - W_uv S - W_u S_v - W_v S_u) / W`.
//!
//! `(A_u, W_u)` is a B-spline with the derivative control points `D_i = p
//! (H_(i+1) - H_i) / (t_(i+p+1) - t_(i+1))` (Piegl and Tiller, eq. 3.8), whose
//! basis functions are non-negative and sum to one; the same holds for the
//! second differences. So `|A_u - W_u S|` is at most the largest `|D_xyz - D_w
//! S|`, and with `S` in the control hull (positive weights), within `R` of the
//! hull's centre `c`, that is at most `|D_xyz - D_w c| + |D_w| R`. `W` is at
//! least the smallest weight.

/// A homogeneous point `(w x, w y, w z, w)`.
type Hom = [f64; 4];

fn sub(a: Hom, b: Hom) -> Hom {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2], a[3] - b[3]]
}

fn scale(a: Hom, k: f64) -> Hom {
    [a[0] * k, a[1] * k, a[2] * k, a[3] * k]
}

/// The derivative control net along the first index of a `rows x cols` net
/// (row major) of degree `p` with knots `k`: `rows - 1` rows. Pieces of zero
/// length, whose basis function vanishes, give zero points.
fn derive(net: &[Hom], rows: usize, cols: usize, p: usize, k: &[f64]) -> Vec<Hom> {
    let mut out = Vec::with_capacity(rows.saturating_sub(1) * cols);
    for i in 0..rows.saturating_sub(1) {
        let den = k.get(i + p + 1).copied().unwrap_or(0.0) - k.get(i + 1).copied().unwrap_or(0.0);
        for j in 0..cols {
            let (Some(&a), Some(&b)) = (net.get((i + 1) * cols + j), net.get(i * cols + j)) else {
                out.push([0.0; 4]);
                continue;
            };
            out.push(if den > 0.0 {
                scale(sub(a, b), p as f64 / den)
            } else {
                [0.0; 4]
            });
        }
    }
    out
}

/// The net transposed (`rows x cols` to `cols x rows`).
fn transpose(net: &[Hom], rows: usize, cols: usize) -> Vec<Hom> {
    let mut out = Vec::with_capacity(net.len());
    for j in 0..cols {
        for i in 0..rows {
            out.push(net.get(i * cols + j).copied().unwrap_or([0.0; 4]));
        }
    }
    out
}

/// `(max |D_xyz - D_w c| + |D_w| r, max |D_w|)` over a derivative net.
fn reach(net: &[Hom], c: [f64; 3], r: f64) -> (f64, f64) {
    net.iter().fold((0.0_f64, 0.0_f64), |(k, o), d| {
        let v = [d[0] - d[3] * c[0], d[1] - d[3] * c[1], d[2] - d[3] * c[2]];
        let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        (k.max(len + d[3].abs() * r), o.max(d[3].abs()))
    })
}

/// Bounds over the patch on `|S_u|`, `|S_v|`, `|S_uu|`, `|S_uv|` and
/// `|S_vv|`.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Derivatives {
    pub u: f64,
    pub v: f64,
    pub uu: f64,
    pub uv: f64,
    pub vv: f64,
}

/// The bounds for a patch of degrees `(pu, pv)` with `nu x nv` homogeneous
/// control points `h` (u major) and knots `(uk, vk)`.
pub(super) fn derivatives(
    (pu, pv): (usize, usize),
    (nu, nv): (usize, usize),
    (uk, vk): (&[f64], &[f64]),
    h: &[Hom],
) -> Derivatives {
    let pts: Vec<[f64; 3]> = h
        .iter()
        .map(|x| [x[0] / x[3], x[1] / x[3], x[2] / x[3]])
        .collect();
    let lo = |a: usize| {
        pts.iter()
            .filter_map(|p| p.get(a).copied())
            .fold(f64::INFINITY, f64::min)
    };
    let hi = |a: usize| {
        pts.iter()
            .filter_map(|p| p.get(a).copied())
            .fold(f64::NEG_INFINITY, f64::max)
    };
    let c = [
        0.5 * (lo(0) + hi(0)),
        0.5 * (lo(1) + hi(1)),
        0.5 * (lo(2) + hi(2)),
    ];
    let r =
        0.5 * ((hi(0) - lo(0)).powi(2) + (hi(1) - lo(1)).powi(2) + (hi(2) - lo(2)).powi(2)).sqrt();
    let w_min = h.iter().map(|x| x[3]).fold(f64::INFINITY, f64::min);
    // Along u: rows are u. Along v: transpose so rows are v.
    let du = derive(h, nu, nv, pu, uk);
    let ht = transpose(h, nu, nv);
    let dv = derive(&ht, nv, nu, pv, vk);
    // Second differences: the derivative nets have one degree less and the
    // knot vectors without their first and last knots.
    let (uk1, vk1) = (
        uk.get(1..uk.len().saturating_sub(1)).unwrap_or(&[]),
        vk.get(1..vk.len().saturating_sub(1)).unwrap_or(&[]),
    );
    let duu = if pu >= 2 {
        derive(&du, nu - 1, nv, pu - 1, uk1)
    } else {
        Vec::new()
    };
    let dvv = if pv >= 2 {
        derive(&dv, nv - 1, nu, pv - 1, vk1)
    } else {
        Vec::new()
    };
    // The mixed net: the v derivative of the u derivative.
    let du_t = transpose(&du, nu - 1, nv);
    let duv = derive(&du_t, nv, nu - 1, pv, vk);
    let (ku, ou) = reach(&du, c, r);
    let (kv, ov) = reach(&dv, c, r);
    let (kuu, _) = reach(&duu, c, r);
    let (kvv, _) = reach(&dvv, c, r);
    let (kuv, _) = reach(&duv, c, r);
    let margin = 1.0 + 1e-9;
    let (bu, bv) = (ku / w_min, kv / w_min);
    Derivatives {
        u: bu * margin,
        v: bv * margin,
        uu: (kuu + 2.0 * ou * bu) / w_min * margin,
        vv: (kvv + 2.0 * ov * bv) / w_min * margin,
        uv: (kuv + ou * bv + ov * bu) / w_min * margin,
    }
}
