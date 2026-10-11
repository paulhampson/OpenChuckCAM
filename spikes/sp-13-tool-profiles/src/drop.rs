// SPDX-License-Identifier: AGPL-3.0-or-later
//! Drop-cutter tool-centre solving against a triangle mesh, generic over the
//! profile: the tool axis is +Z (the part is rotated by A first), and the tip
//! height is the highest of the vertex, facet and edge tests. Every test asks
//! the profile only for the lower envelope `h(r)` and its pieces.
//!
//! Facet test: along the facet's uphill direction maximise `G s - h(s)`; a
//! line piece peaks at an end, a convex arc where `h'(s) = G`, a concave arc
//! at an end. Edge test: split the edge where its horizontal distance from
//! the axis crosses a piece boundary; on each part `z(t) - h(d(t))` is concave
//! for lines and convex arcs (golden-section search is exact to rounding) and
//! is sampled then refined for concave arcs.

use crate::mesh::{V3, XyIndex};
use crate::profile::{Piece, Profile, RPiece};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Drop {
    /// Tip height.
    pub tip_z: f64,
    /// Contact point on the part.
    pub contact: V3,
}

fn better(best: &mut Option<Drop>, tip_z: f64, contact: V3) {
    if best.is_none_or(|b| tip_z > b.tip_z) {
        *best = Some(Drop { tip_z, contact });
    }
}

/// Drops the tool with its axis through (x, y) onto the mesh.
pub fn drop_mesh(
    profile: &Profile,
    index: &XyIndex,
    x: f64,
    y: f64,
    scratch: &mut Vec<u32>,
) -> Option<Drop> {
    let r = profile.r_max();
    index.query(x, y, r, scratch);
    // The profile never dips below the tip (h >= 0), so a triangle can lift
    // the tip at most to its highest vertex. Visit triangles from the highest
    // down and stop once none can beat the best so far.
    let mut order: Vec<(f64, u32)> = scratch
        .iter()
        .map(|&t| {
            (
                index
                    .mesh
                    .tri(t as usize)
                    .iter()
                    .map(|v| v[2])
                    .fold(f64::MIN, f64::max),
                t,
            )
        })
        .collect();
    order.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    let mut best: Option<Drop> = None;
    for &(zmax, t) in &order {
        if best.is_some_and(|b| zmax <= b.tip_z) {
            break;
        }
        let tri = index.mesh.tri(t as usize);
        for v in &tri {
            vertex(profile, x, y, *v, &mut best);
        }
        for k in 0..3 {
            edge(profile, x, y, tri[k], tri[(k + 1) % 3], &mut best);
        }
        facet(profile, x, y, &tri, &mut best);
    }
    best
}

fn vertex(profile: &Profile, x: f64, y: f64, v: V3, best: &mut Option<Drop>) {
    let d = (v[0] - x).hypot(v[1] - y);
    if let Some(h) = profile.height_at(d) {
        better(best, v[2] - h, v);
    }
}

fn facet(profile: &Profile, x: f64, y: f64, tri: &[V3; 3], best: &mut Option<Drop>) {
    let [a, b, c] = *tri;
    let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let mut n = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if len == 0.0 {
        return;
    }
    n = [n[0] / len, n[1] / len, n[2] / len];
    if n[2] < 0.0 {
        n = [-n[0], -n[1], -n[2]];
    }
    if n[2] < 1e-12 {
        return; // Vertical facet: its edges carry the contact.
    }
    let plane = |px: f64, py: f64| a[2] - (n[0] * (px - a[0]) + n[1] * (py - a[1])) / n[2];
    let gx = -n[0] / n[2];
    let gy = -n[1] / n[2];
    let g = gx.hypot(gy);
    // Best radius s along the uphill direction.
    let mut best_s = 0.0;
    let mut best_f = f64::MIN;
    let mut consider = |s: f64, h: f64| {
        let f = g * s - h;
        if f > best_f {
            best_f = f;
            best_s = s;
        }
    };
    if g < 1e-15 {
        consider(0.0, 0.0);
    } else {
        for rp in profile.rpieces() {
            consider(rp.r0, rp.piece.eval(rp.r0));
            consider(rp.r1, rp.piece.eval(rp.r1));
            if let Piece::Arc {
                c,
                rho,
                convex: true,
                ..
            } = rp.piece
            {
                let s = c.r + g * rho / (1.0 + g * g).sqrt();
                if s > rp.r0 && s < rp.r1 {
                    consider(s, rp.piece.eval(s));
                }
            }
        }
    }
    let (ux, uy) = if g < 1e-15 {
        (0.0, 0.0)
    } else {
        (gx / g, gy / g)
    };
    let px = x + best_s * ux;
    let py = y + best_s * uy;
    if inside_xy(tri, px, py) {
        let pz = plane(px, py);
        if let Some(h) = profile.height_at(best_s) {
            better(best, pz - h, [px, py, pz]);
        }
    }
}

fn inside_xy(tri: &[V3; 3], px: f64, py: f64) -> bool {
    let cross = |a: V3, b: V3| (b[0] - a[0]) * (py - a[1]) - (b[1] - a[1]) * (px - a[0]);
    let d0 = cross(tri[0], tri[1]);
    let d1 = cross(tri[1], tri[2]);
    let d2 = cross(tri[2], tri[0]);
    let eps = 1e-12;
    (d0 >= -eps && d1 >= -eps && d2 >= -eps) || (d0 <= eps && d1 <= eps && d2 <= eps)
}

fn edge(profile: &Profile, x: f64, y: f64, p0: V3, p1: V3, best: &mut Option<Drop>) {
    let ex = p1[0] - p0[0];
    let ey = p1[1] - p0[1];
    let l2 = ex * ex + ey * ey;
    if l2 < 1e-24 {
        return; // Vertical edge: its top vertex carries the contact.
    }
    let wx = p0[0] - x;
    let wy = p0[1] - y;
    let bq = wx * ex + wy * ey;
    let cq = wx * wx + wy * wy;
    // d(t)^2 = l2 t^2 + 2 bq t + cq
    let roots = |r: f64| -> Option<(f64, f64)> {
        let disc = bq * bq - l2 * (cq - r * r);
        if disc < 0.0 {
            return None;
        }
        let s = disc.sqrt();
        Some(((-bq - s) / l2, (-bq + s) / l2))
    };
    let r_max = profile.r_max();
    let Some((ta, tb)) = roots(r_max) else { return };
    let ta = ta.max(0.0);
    let tb = tb.min(1.0);
    if ta > tb {
        return;
    }
    let mut ts: Vec<f64> = Vec::with_capacity(8);
    ts.extend([ta, tb]);
    let tmin = -bq / l2;
    if tmin > ta && tmin < tb {
        ts.push(tmin);
    }
    for rp in profile.rpieces() {
        for rk in [rp.r0, rp.r1] {
            if rk > 0.0 && rk < r_max {
                if let Some((t0, t1)) = roots(rk) {
                    for t in [t0, t1] {
                        if t > ta && t < tb {
                            ts.push(t);
                        }
                    }
                }
            }
        }
    }
    ts.sort_by(f64::total_cmp);
    ts.dedup();
    let point = |t: f64| [p0[0] + t * ex, p0[1] + t * ey, p0[2] + t * (p1[2] - p0[2])];
    // Direct evaluation: the expanded quadratic loses about 1e-8 mm to
    // cancellation near the tool axis, which shows at a sharp V's apex.
    let dist = |t: f64| (wx + t * ex).hypot(wy + t * ey);
    for w in ts.windows(2) {
        let (t0, t1) = (w[0], w[1]);
        let mid = dist(0.5 * (t0 + t1));
        let Some(rp) = piece_at(profile.rpieces(), mid) else {
            continue;
        };
        let f = |t: f64| {
            let d = dist(t).clamp(rp.r0, rp.r1);
            point(t)[2] - rp.piece.eval(d)
        };
        let t = if rp.piece.convex_in_r() {
            golden_max(&f, t0, t1)
        } else {
            sampled_max(&f, t0, t1, 32)
        };
        for tt in [t0, t, t1] {
            let p = point(tt);
            better(best, f(tt), p);
        }
    }
}

fn piece_at(rpieces: &[RPiece], d: f64) -> Option<&RPiece> {
    rpieces.iter().find(|rp| d >= rp.r0 && d <= rp.r1)
}

/// Golden-section maximum of a unimodal function on [a, b].
pub fn golden_max(f: &impl Fn(f64) -> f64, mut a: f64, mut b: f64) -> f64 {
    let k = (5f64.sqrt() - 1.0) / 2.0;
    let mut c = b - k * (b - a);
    let mut d = a + k * (b - a);
    let (mut fc, mut fd) = (f(c), f(d));
    for _ in 0..100 {
        if (b - a).abs() <= 1e-15 * (1.0 + a.abs()) {
            break;
        }
        if fc < fd {
            a = c;
            c = d;
            fc = fd;
            d = a + k * (b - a);
            fd = f(d);
        } else {
            b = d;
            d = c;
            fd = fc;
            c = b - k * (b - a);
            fc = f(c);
        }
    }
    0.5 * (a + b)
}

/// Samples `n` points, then refines about the best one.
fn sampled_max(f: &impl Fn(f64) -> f64, a: f64, b: f64, n: usize) -> f64 {
    let step = (b - a) / n as f64;
    let mut best_i = 0;
    let mut best_f = f64::MIN;
    for i in 0..=n {
        let v = f(a + step * i as f64);
        if v > best_f {
            best_f = v;
            best_i = i;
        }
    }
    let lo = a + step * best_i.saturating_sub(1) as f64;
    let hi = (a + step * (best_i + 1) as f64).min(b);
    golden_max(f, lo, hi)
}
