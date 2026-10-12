// SPDX-License-Identifier: AGPL-3.0-only
// In-house analytic surfaces with OpenCASCADE's parametrisation, so (u, v) agree with the kernel.
// Everything here is closed form; nothing calls the kernel.
use crate::vec3::*;
use std::f64::consts::PI;

#[derive(Clone, Copy, Debug)]
pub enum Kind { Plane, Cylinder, Cone { semi: f64 }, Sphere, Torus { minor: f64 } }

/// Frame (origin, z axis, x direction) plus the size parameter: cylinder/sphere radius, cone reference
/// radius, torus major radius. `flip` is the face orientation (reversed faces have inward-pointing du x dv).
#[derive(Clone, Copy, Debug)]
pub struct Analytic { pub kind: Kind, pub o: V3, pub z: V3, pub x: V3, pub r: f64, pub flip: bool }

impl Analytic {
    fn y(&self) -> V3 { cross(self.z, self.x) }
    fn radial(&self, u: f64) -> V3 { add(mul(self.x, u.cos()), mul(self.y(), u.sin())) }
    fn dradial(&self, u: f64) -> V3 { add(mul(self.x, -u.sin()), mul(self.y(), u.cos())) }

    /// Point and partial derivatives at (u, v).
    pub fn d1(&self, u: f64, v: f64) -> (V3, V3, V3) {
        let (o, z) = (self.o, self.z);
        match self.kind {
            Kind::Plane => (add(add(o, mul(self.x, u)), mul(self.y(), v)), self.x, self.y()),
            Kind::Cylinder => (add(add(o, mul(self.radial(u), self.r)), mul(z, v)),
                mul(self.dradial(u), self.r), z),
            Kind::Cone { semi } => {
                let rad = self.r + v * semi.sin();
                (add(add(o, mul(self.radial(u), rad)), mul(z, v * semi.cos())),
                 mul(self.dradial(u), rad),
                 add(mul(self.radial(u), semi.sin()), mul(z, semi.cos())))
            }
            Kind::Sphere => {
                let (c, s) = (v.cos(), v.sin());
                (add(add(o, mul(self.radial(u), self.r * c)), mul(z, self.r * s)),
                 mul(self.dradial(u), self.r * c),
                 add(mul(self.radial(u), -self.r * s), mul(z, self.r * c)))
            }
            Kind::Torus { minor } => {
                let (c, s) = (v.cos(), v.sin());
                let rad = self.r + minor * c;
                (add(add(o, mul(self.radial(u), rad)), mul(z, minor * s)),
                 mul(self.dradial(u), rad),
                 add(mul(self.radial(u), -minor * s), mul(z, minor * c)))
            }
        }
    }

    pub fn eval(&self, u: f64, v: f64) -> (V3, V3) {
        let (p, du, dv) = self.d1(u, v);
        let mut n = norm(cross(du, dv));
        if self.flip { n = mul(n, -1.0); }
        (p, n)
    }

    /// Closest point on the untrimmed surface: (u, v, point). u is in [0, 2 pi) for revolved surfaces.
    pub fn project(&self, q: V3) -> (f64, f64, V3) {
        let d = sub(q, self.o);
        let (lx, ly, lz) = (dot(d, self.x), dot(d, self.y()), dot(d, self.z));
        let u = ly.atan2(lx).rem_euclid(2.0 * PI);
        let rho = lx.hypot(ly);
        match self.kind {
            Kind::Plane => { let (u, v) = (lx, ly); (u, v, self.d1(u, v).0) }
            Kind::Cylinder => (u, lz, self.d1(u, lz).0),
            Kind::Cone { semi } => {
                // Meridian line through (r0, 0) with direction (sin a, cos a): project (rho, lz) onto it.
                let (s, c) = (semi.sin(), semi.cos());
                let v = (rho - self.r) * s + lz * c;
                (u, v, self.d1(u, v).0)
            }
            Kind::Sphere => {
                let v = lz.atan2(rho);
                (u, v, self.d1(u, v).0)
            }
            Kind::Torus { minor } => {
                // Nearest point on the tube circle in the meridian plane through q.
                let (dr, dz) = (rho - self.r, lz);
                let v = dz.atan2(dr).rem_euclid(2.0 * PI);
                let _ = minor;
                (u, v, self.d1(u, v).0)
            }
        }
    }

    /// All ray hits for t in [0, tmax], as (t, u, v), sorted by t. Quadrics in closed form; the torus by
    /// scanning the implicit function and bisecting.
    pub fn ray(&self, o: V3, d: V3, tmax: f64) -> Vec<(f64, f64, f64)> {
        let mut ts: Vec<f64> = Vec::new();
        let rel = sub(o, self.o);
        let (px, py, pz) = (dot(rel, self.x), dot(rel, self.y()), dot(rel, self.z));
        let (dx, dy, dz) = (dot(d, self.x), dot(d, self.y()), dot(d, self.z));
        match self.kind {
            Kind::Plane => { if dz.abs() > 1e-14 { ts.push(-pz / dz); } }
            Kind::Cylinder => quad(dx * dx + dy * dy, 2.0 * (px * dx + py * dy), px * px + py * py - self.r * self.r, &mut ts),
            Kind::Cone { semi } => {
                // rho = r0 + z tan(a)  (v measured along the generator; z = v cos a)
                let k = semi.tan();
                let (a0, a1) = (self.r, k);
                // (px+t dx)^2 + (py+t dy)^2 = (a0 + a1 (pz + t dz))^2
                let c0 = a0 + a1 * pz;
                quad(dx * dx + dy * dy - a1 * a1 * dz * dz, 2.0 * (px * dx + py * dy - c0 * a1 * dz), px * px + py * py - c0 * c0, &mut ts);
            }
            Kind::Sphere => quad(1.0, 2.0 * (px * dx + py * dy + pz * dz), px * px + py * py + pz * pz - self.r * self.r, &mut ts),
            Kind::Torus { minor } => {
                let f = |t: f64| {
                    let (x, y, z) = (px + t * dx, py + t * dy, pz + t * dz);
                    (x.hypot(y) - self.r).powi(2) + z * z - minor * minor
                };
                let step = minor / 8.0;
                let n = (tmax / step).ceil() as usize + 1;
                let mut t0 = 0.0; let mut f0 = f(0.0);
                for k in 1..=n {
                    let t1 = (k as f64 * step).min(tmax);
                    let f1 = f(t1);
                    if f0 == 0.0 { ts.push(t0); }
                    else if f0 * f1 < 0.0 {
                        let (mut a, mut b) = (t0, t1);
                        for _ in 0..80 { let m = 0.5 * (a + b); if f(a) * f(m) <= 0.0 { b = m } else { a = m } }
                        ts.push(0.5 * (a + b));
                    }
                    t0 = t1; f0 = f1;
                }
            }
        }
        if let Kind::Cone { semi } = self.kind { let k = semi.tan(); ts.retain(|t| self.r + k * (pz + t * dz) >= 0.0); }
        ts.retain(|t| *t >= 0.0 && *t <= tmax);
        ts.sort_by(|a, b| a.partial_cmp(b).unwrap());
        ts.into_iter().map(|t| { let (u, v, _) = self.project(add(o, mul(d, t))); (t, u, v) }).collect()
    }
}

fn quad(a: f64, b: f64, c: f64, out: &mut Vec<f64>) {
    if a.abs() < 1e-14 { if b.abs() > 1e-14 { out.push(-c / b); } return; }
    let disc = b * b - 4.0 * a * c;
    if disc < 0.0 { return; }
    let s = disc.sqrt();
    // Stable form.
    let q = -0.5 * (b + b.signum().max(0.0).mul_add(2.0, -1.0) * s);
    out.push(q / a);
    if q != 0.0 { out.push(c / q); }
}
