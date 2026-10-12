// SPDX-License-Identifier: AGPL-3.0-only
// Brute-force triangle-mesh queries: the tessellation baseline. (SP-4 uses a parry3d-f64 BVH for speed;
// accuracy of the queries is the same, and accuracy is what SP-9 compares.)
use crate::ffi::Mesh;
use crate::vec3::*;

/// Closest point on triangle abc to p (Ericson, Real-Time Collision Detection 5.1.5).
fn closest_on_tri(p: V3, a: V3, b: V3, c: V3) -> V3 {
    let (ab, ac, ap) = (sub(b, a), sub(c, a), sub(p, a));
    let (d1, d2) = (dot(ab, ap), dot(ac, ap));
    if d1 <= 0.0 && d2 <= 0.0 { return a; }
    let bp = sub(p, b); let (d3, d4) = (dot(ab, bp), dot(ac, bp));
    if d3 >= 0.0 && d4 <= d3 { return b; }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 { return add(a, mul(ab, d1 / (d1 - d3))); }
    let cp = sub(p, c); let (d5, d6) = (dot(ab, cp), dot(ac, cp));
    if d6 >= 0.0 && d5 <= d6 { return c; }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 { return add(a, mul(ac, d2 / (d2 - d6))); }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        return add(b, mul(sub(c, b), (d4 - d3) / ((d4 - d3) + (d5 - d6))));
    }
    let denom = 1.0 / (va + vb + vc);
    add(a, add(mul(ab, vb * denom), mul(ac, vc * denom)))
}

impl Mesh {
    /// (point, facet normal, distance) of the closest point on the mesh.
    pub fn closest(&self, p: V3) -> Option<(V3, V3, f64)> {
        let mut best: Option<(V3, V3, f64)> = None;
        for t in &self.tris {
            let q = closest_on_tri(p, t[0], t[1], t[2]);
            let d = dist(p, q);
            if best.map_or(true, |b| d < b.2) {
                best = Some((q, norm(cross(sub(t[1], t[0]), sub(t[2], t[0]))), d));
            }
        }
        best
    }

    /// Ray hits (t, facet normal) for t in [0, tmax], sorted by t (Moller-Trumbore).
    pub fn ray(&self, o: V3, d: V3, tmax: f64) -> Vec<(f64, V3)> {
        let mut hits = Vec::new();
        for t in &self.tris {
            let (e1, e2) = (sub(t[1], t[0]), sub(t[2], t[0]));
            let pv = cross(d, e2); let det = dot(e1, pv);
            if det.abs() < 1e-18 { continue; }
            let inv = 1.0 / det; let tv = sub(o, t[0]);
            let u = dot(tv, pv) * inv; if !(-1e-12..=1.0 + 1e-12).contains(&u) { continue; }
            let qv = cross(tv, e1); let v = dot(d, qv) * inv; if v < -1e-12 || u + v > 1.0 + 1e-12 { continue; }
            let s = dot(e2, qv) * inv;
            if s >= 0.0 && s <= tmax { hits.push((s, norm(cross(e1, e2)))); }
        }
        hits.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        hits
    }
}
