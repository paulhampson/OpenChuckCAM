// SPDX-License-Identifier: AGPL-3.0-only
// In-house tensor-product NURBS surface: evaluation with first derivatives, Newton closest point
// and Newton ray intersection from a seed grid. About 200 lines; the point is to see what writing
// our own costs and how it compares with the kernel and with curvo.
use crate::vec3::*;

#[derive(Clone)]
pub struct Nurbs {
    pub pu: usize, pub pv: usize,
    pub uk: Vec<f64>, pub vk: Vec<f64>,   // flat knot vectors
    pub nu: usize, pub nv: usize,
    pub poles: Vec<V3>,                   // u major: poles[iu * nv + iv]
    pub w: Vec<f64>,
    pub flip: bool,
}

fn span(k: &[f64], deg: usize, n: usize, u: f64) -> usize {
    // k has n + deg + 1 entries; valid spans are deg..n-1
    if u >= k[n] { return n - 1; }
    if u <= k[deg] { return deg; }
    let (mut lo, mut hi) = (deg, n);
    while hi - lo > 1 { let mid = (lo + hi) / 2; if u < k[mid] { hi = mid } else { lo = mid } }
    lo
}

/// Non-zero basis functions of degree `deg` at `u` (Piegl and Tiller A2.2): N_{span-deg..span}.
fn basis_values(k: &[f64], deg: usize, sp: usize, u: f64) -> Vec<f64> {
    let mut n = vec![0.0; deg + 1];
    let mut left = vec![0.0; deg + 1]; let mut right = vec![0.0; deg + 1];
    n[0] = 1.0;
    for j in 1..=deg {
        left[j] = u - k[sp + 1 - j]; right[j] = k[sp + j] - u;
        let mut saved = 0.0;
        for r in 0..j {
            let t = n[r] / (right[r + 1] + left[j - r]);
            n[r] = saved + right[r + 1] * t;
            saved = left[j - r] * t;
        }
        n[j] = saved;
    }
    n
}

/// Basis functions and first derivatives: N'_{i,p} = p (N_{i,p-1} / (u_{i+p} - u_i) - N_{i+1,p-1} / (u_{i+p+1} - u_{i+1})).
fn basis(k: &[f64], deg: usize, sp: usize, u: f64) -> (Vec<f64>, Vec<f64>) {
    let n0 = basis_values(k, deg, sp, u);
    let mut d1 = vec![0.0; deg + 1];
    if deg > 0 {
        let lb = basis_values(k, deg - 1, sp, u); // indices span-(deg-1) ..= span
        for r in 0..=deg {
            let i = sp - deg + r;
            let mut d = 0.0;
            if r >= 1 { let den = k[i + deg] - k[i]; if den > 0.0 { d += lb[r - 1] / den; } }
            if r < deg { let den = k[i + deg + 1] - k[i + 1]; if den > 0.0 { d -= lb[r] / den; } }
            d1[r] = d * deg as f64;
        }
    }
    (n0, d1)
}

impl Nurbs {
    pub fn u_domain(&self) -> (f64, f64) { (self.uk[self.pu], self.uk[self.nu]) }
    pub fn v_domain(&self) -> (f64, f64) { (self.vk[self.pv], self.vk[self.nv]) }

    pub fn d1(&self, u: f64, v: f64) -> (V3, V3, V3) {
        let su = span(&self.uk, self.pu, self.nu, u); let sv = span(&self.vk, self.pv, self.nv, v);
        let (bu, dbu) = basis(&self.uk, self.pu, su, u); let (bv, dbv) = basis(&self.vk, self.pv, sv, v);
        // homogeneous sums: A = sum N w P, W = sum N w and their derivatives
        let (mut a, mut au, mut av) = ([0.0; 3], [0.0; 3], [0.0; 3]);
        let (mut w, mut wu, mut wv) = (0.0, 0.0, 0.0);
        for i in 0..=self.pu { for j in 0..=self.pv {
            let idx = (su - self.pu + i) * self.nv + (sv - self.pv + j);
            let (p, wt) = (self.poles[idx], self.w[idx]);
            let (n, nu_, nv_) = (bu[i] * bv[j] * wt, dbu[i] * bv[j] * wt, bu[i] * dbv[j] * wt);
            a = add(a, mul(p, n)); au = add(au, mul(p, nu_)); av = add(av, mul(p, nv_));
            w += n; wu += nu_; wv += nv_;
        } }
        let s = mul(a, 1.0 / w);
        let su_ = mul(sub(au, mul(s, wu)), 1.0 / w);
        let sv_ = mul(sub(av, mul(s, wv)), 1.0 / w);
        (s, su_, sv_)
    }

    pub fn eval(&self, u: f64, v: f64) -> (V3, V3) {
        let (p, du, dv) = self.d1(u, v);
        let mut n = norm(cross(du, dv));
        if self.flip { n = mul(n, -1.0); }
        (p, n)
    }

    fn clamp(&self, u: f64, v: f64) -> (f64, f64) {
        let (a, b) = self.u_domain(); let (c, d) = self.v_domain();
        (u.clamp(a, b), v.clamp(c, d))
    }

    /// Closest point on the untrimmed patch: Gauss-Newton on the stationarity conditions from a seed grid.
    pub fn project(&self, q: V3) -> (f64, f64, V3) {
        let (u0, u1) = self.u_domain(); let (v0, v1) = self.v_domain();
        let g = 8;
        let mut best = (0.0, 0.0, f64::MAX);
        for i in 0..=g { for j in 0..=g {
            let (u, v) = (u0 + (u1 - u0) * i as f64 / g as f64, v0 + (v1 - v0) * j as f64 / g as f64);
            let d = dist(self.d1(u, v).0, q);
            if d < best.2 { best = (u, v, d); }
        } }
        let (mut u, mut v) = (best.0, best.1);
        for _ in 0..40 {
            let (p, su, sv) = self.d1(u, v);
            let r = sub(p, q);
            // Gauss-Newton: minimise |S - q|^2 with the Hessian dropped second derivatives.
            let (a, b, c) = (dot(su, su), dot(su, sv), dot(sv, sv));
            let (f, g2) = (dot(r, su), dot(r, sv));
            let det = a * c - b * b;
            if det.abs() < 1e-18 { break; }
            let du = (c * f - b * g2) / det; let dv = (a * g2 - b * f) / det;
            let (nu, nv) = self.clamp(u - du, v - dv);
            let done = (nu - u).abs() + (nv - v).abs() < 1e-14;
            u = nu; v = nv;
            if done { break; }
        }
        (u, v, self.d1(u, v).0)
    }

    /// First hit of the ray within t in [0, tmax] on the untrimmed patch: Newton on S(u,v) = o + t d from a seed grid.
    pub fn ray(&self, o: V3, d: V3, tmax: f64) -> Vec<(f64, f64, f64)> {
        let (u0, u1) = self.u_domain(); let (v0, v1) = self.v_domain();
        let g = 6;
        let mut hits: Vec<(f64, f64, f64)> = Vec::new();
        for i in 0..=g { for j in 0..=g {
            let (mut u, mut v) = (u0 + (u1 - u0) * i as f64 / g as f64, v0 + (v1 - v0) * j as f64 / g as f64);
            let mut t = dot(sub(self.d1(u, v).0, o), d);
            let mut ok = false;
            for _ in 0..40 {
                let (p, su, sv) = self.d1(u, v);
                let r = sub(sub(p, o), mul(d, t));
                if len(r) < 1e-12 { ok = true; break; }
                // Solve [su sv -d] [du dv dt] = -r (3x3 by Cramer).
                let m = [su, sv, mul(d, -1.0)];
                let det = dot(m[0], cross(m[1], m[2]));
                if det.abs() < 1e-16 { break; }
                let rhs = mul(r, -1.0);
                let du = dot(rhs, cross(m[1], m[2])) / det;
                let dv = dot(m[0], cross(rhs, m[2])) / det;
                let dt = dot(m[0], cross(m[1], rhs)) / det;
                let (nu, nv) = self.clamp(u + du, v + dv);
                u = nu; v = nv; t += dt;
            }
            if ok && t >= 0.0 && t <= tmax && !hits.iter().any(|h| (h.0 - t).abs() < 1e-7) { hits.push((t, u, v)); }
        } }
        hits.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        hits
    }
}
