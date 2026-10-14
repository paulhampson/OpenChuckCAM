// SPDX-License-Identifier: AGPL-3.0-or-later
//! Exact B-rep faces as a [`SurfaceSource`] (requirement IMP-02; spike SP-9,
//! decision record 0017 items 2 and 3; work breakdown slice 11).
//!
//! An [`ExactFace`] is built from an [`openchuck_types::brep::BrepFace`] and
//! implements [`ParametricFace`] in pure Rust: closed forms for the plane,
//! cylinder, cone, sphere and torus, and a clamped rational B-spline
//! evaluator for everything else. [`BrepSurface`] holds the faces of a
//! [`BrepModel`] (all of them, or a selection) and implements
//! [`SurfaceSource`]. No kernel type is involved.
//!
//! # Conventions
//!
//! - Analytic faces use the parametrisation in the `openchuck_types::brep`
//!   module documentation. The frame's `y` is used exactly as given, never
//!   recomputed as `z x x`, so mirrored (left-handed) frames work: the
//!   surface normal is `du x dv`, which points inward for a mirrored frame,
//!   and is flipped when the face is `reversed`.
//! - The trim test is even-odd over the trim loops in (u, v). Periodic
//!   parameters (u of every revolved surface, v of the torus) are wrapped
//!   into the face's parameter box, and the loops are also tested shifted by
//!   one period, so a face that straddles the seam is classified correctly.
//!   A face with no loops is its whole parameter box.
//! - The closest point of a trimmed face is the closed-form or iterated
//!   closest point of the surface when that lies on the face, otherwise the
//!   closest point on the trim loops (the polylines, mapped to the surface:
//!   as accurate as the polylines are, which is the import's tolerance).
//! - B-spline closest points seed from the parameter grid, from its local
//!   minima and from the nearest tessellation vertex, run a damped
//!   (Levenberg-Marquardt) Gauss-Newton clamped to the parameter box, and
//!   finish with a derivative-free pattern search. The damping and the
//!   search deal with singular parametrisations such as the poles of a
//!   sphere (OI-72: undamped Gauss-Newton stalled there, worst 0.48 mm).
//! - Face bounding boxes are conservative: the control-point hull for
//!   B-splines, a sampled box widened by 2 % of its diagonal for analytic
//!   faces. They only prune; answers do not depend on them.
//! - Everything is deterministic and single-threaded (NFR-06).
//!
//! # Tool drops
//!
//! Tool-centre solving on exact faces is not settled by SP-9 (open item
//! OI-81). [`BrepSurface::drop_tool`](SurfaceSource::drop_tool) therefore
//! drops the tool onto the joined tessellation through [`MeshSurface`]; its
//! error is the tessellation's.

use std::f64::consts::{PI, TAU};

use openchuck_types::brep::{BSplinePatch, BrepFace, BrepModel, FaceId, Frame, Surface};
use openchuck_types::geometry::{Aabb, Point3, UnitVec3, Vec3};
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::tool::profile::RevolvedProfile;

use crate::cutter::{Contact, ToolLine};
use crate::error::{PathError, Result};
use crate::surface::{
    FaceClosest, FacePosition, MeshSurface, ParametricFace, RayHit, SurfacePoint, SurfaceSource, Uv,
};

fn bad(face: u32, what: impl std::fmt::Display) -> PathError {
    PathError::Surface(format!("face {face}: {what}"))
}

fn dist2(a: Point3, b: Point3) -> f64 {
    let d = a - b;
    d.dot(d)
}

fn at(s: &[f64], i: usize) -> f64 {
    s.get(i).copied().unwrap_or(f64::NAN)
}

fn put(s: &mut [f64], i: usize, v: f64) {
    if let Some(x) = s.get_mut(i) {
        *x = v;
    }
}

fn same(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-12 * (1.0 + a.abs())
}

fn wrap(x: f64, lo: f64, period: f64) -> f64 {
    lo + (x - lo).rem_euclid(period)
}

// ---------------------------------------------------------------------------
// Real roots of a polynomial and a stable quadratic.

fn quad_roots(a: f64, b: f64, c: f64, out: &mut Vec<f64>) {
    if a.abs() < 1e-14 {
        if b.abs() > 1e-14 {
            out.push(-c / b);
        }
        return;
    }
    let disc = b * b - 4.0 * a * c;
    if disc < 0.0 {
        return;
    }
    let s = disc.sqrt();
    let q = -0.5 * (b + if b < 0.0 { -s } else { s });
    out.push(q / a);
    if q.abs() > 0.0 {
        out.push(c / q);
    }
}

fn poly_eval(c: &[f64], x: f64) -> (f64, f64) {
    let mut v = 0.0;
    let mut mag = 0.0;
    for k in c.iter().rev() {
        v = v * x + k;
        mag = mag * x.abs() + k.abs();
    }
    (v, mag)
}

/// Real roots of `c[0] + c[1] x + ...` (ascending), by isolating them between
/// the roots of the derivative. Double roots (grazing contacts) are kept.
fn real_roots(c: &[f64]) -> Vec<f64> {
    let top = c.iter().fold(0.0_f64, |m, k| m.max(k.abs()));
    let mut n = c.len();
    while n > 0 && at(c, n - 1).abs() <= 1e-14 * top {
        n -= 1;
    }
    let c = c.get(..n).unwrap_or(&[]);
    match n {
        0 | 1 => Vec::new(),
        2 => vec![-at(c, 0) / at(c, 1)],
        3 => {
            let mut out = Vec::new();
            quad_roots(at(c, 2), at(c, 1), at(c, 0), &mut out);
            out.sort_by(f64::total_cmp);
            out
        }
        _ => {
            let deriv: Vec<f64> = c
                .iter()
                .enumerate()
                .skip(1)
                .map(|(i, k)| k * i as f64)
                .collect();
            let crit = real_roots(&deriv);
            let lead = at(c, n - 1);
            let bound = 1.0
                + c.iter()
                    .take(n - 1)
                    .fold(0.0_f64, |m, k| m.max((k / lead).abs()));
            let mut pts = vec![-bound];
            pts.extend(crit.iter().copied().filter(|x| x.abs() < bound));
            pts.push(bound);
            let mut out = Vec::new();
            for (i, w) in pts.windows(2).enumerate() {
                let (a, b) = (at(w, 0), at(w, 1));
                let (fa, _) = poly_eval(c, a);
                let (fb, _) = poly_eval(c, b);
                if i > 0 {
                    let (f, mag) = poly_eval(c, a);
                    if f.abs() <= 1e-13 * mag && f.abs() > 0.0 {
                        out.push(a); // a grazing (double) root at a critical point
                    }
                }
                if fa.abs() <= 0.0 {
                    out.push(a);
                } else if fa * fb < 0.0 {
                    let (mut lo, mut hi, mut flo) = (a, b, fa);
                    for _ in 0..200 {
                        let mid = 0.5 * (lo + hi);
                        if mid <= lo || mid >= hi {
                            break;
                        }
                        let (fm, _) = poly_eval(c, mid);
                        if fm * flo <= 0.0 {
                            hi = mid;
                        } else {
                            lo = mid;
                            flo = fm;
                        }
                    }
                    out.push(0.5 * (lo + hi));
                }
            }
            out.sort_by(f64::total_cmp);
            out.dedup_by(|a, b| (*a - *b).abs() <= 1e-12 * (1.0 + b.abs()));
            out
        }
    }
}

// ---------------------------------------------------------------------------
// Small dense solves.

fn solve2(a: [[f64; 2]; 2], b: [f64; 2]) -> Option<[f64; 2]> {
    let [[a00, a01], [a10, a11]] = a;
    let det = a00 * a11 - a01 * a10;
    if !det.is_finite() || det.abs() <= 1e-300 {
        return None;
    }
    let [b0, b1] = b;
    Some([(b0 * a11 - a01 * b1) / det, (a00 * b1 - a10 * b0) / det])
}

fn solve3(a: [[f64; 3]; 3], b: [f64; 3]) -> Option<[f64; 3]> {
    let [[a, b_, c], [d, e, f], [g, h, i]] = a;
    let [b0, b1, b2] = b;
    let det = a * (e * i - f * h) - b_ * (d * i - f * g) + c * (d * h - e * g);
    if !det.is_finite() || det.abs() <= 1e-300 {
        return None;
    }
    let x = b0 * (e * i - f * h) - b_ * (b1 * i - f * b2) + c * (b1 * h - e * b2);
    let y = a * (b1 * i - f * b2) - b0 * (d * i - f * g) + c * (d * b2 - b1 * g);
    let z = a * (e * b2 - b1 * h) - b_ * (d * b2 - b1 * g) + b0 * (d * h - e * g);
    Some([x / det, y / det, z / det])
}

// ---------------------------------------------------------------------------
// Analytic surfaces.

#[derive(Debug, Clone, Copy)]
struct Axes {
    o: Point3,
    x: Vec3,
    y: Vec3,
    z: Vec3,
    /// +1 for a right-handed frame, -1 for a mirrored one.
    hand: f64,
}

impl Axes {
    fn new(face: u32, f: &Frame) -> Result<Self> {
        let ok = |v: Vec3| v.is_finite() && (v.length() - 1.0).abs() < 1e-6;
        if !f.origin.is_finite() || !ok(f.x) || !ok(f.y) || !ok(f.z) {
            return Err(bad(face, "the frame must be finite with unit axes"));
        }
        if f.x.dot(f.y).abs() > 1e-6 || f.x.dot(f.z).abs() > 1e-6 || f.y.dot(f.z).abs() > 1e-6 {
            return Err(bad(face, "the frame axes must be orthogonal"));
        }
        let hand = if f.x.cross(f.y).dot(f.z) >= 0.0 {
            1.0
        } else {
            -1.0
        };
        Ok(Self {
            o: f.origin,
            x: f.x,
            y: f.y,
            z: f.z,
            hand,
        })
    }

    fn radial(&self, u: f64) -> Vec3 {
        self.x * u.cos() + self.y * u.sin()
    }

    fn dradial(&self, u: f64) -> Vec3 {
        self.x * (-u.sin()) + self.y * u.cos()
    }

    fn local(&self, p: Point3) -> (f64, f64, f64) {
        let d = p - self.o;
        (d.dot(self.x), d.dot(self.y), d.dot(self.z))
    }
}

#[derive(Debug, Clone, Copy)]
enum Quad {
    Plane,
    Cylinder { r: f64 },
    Cone { r: f64, a: f64 },
    Sphere { r: f64 },
    Torus { major: f64, minor: f64 },
}

#[derive(Debug, Clone, Copy)]
struct Analytic {
    ax: Axes,
    quad: Quad,
}

impl Analytic {
    fn revolved(&self) -> bool {
        !matches!(self.quad, Quad::Plane)
    }

    fn periods(&self) -> (Option<f64>, Option<f64>) {
        match self.quad {
            Quad::Plane => (None, None),
            Quad::Torus { .. } => (Some(TAU), Some(TAU)),
            _ => (Some(TAU), None),
        }
    }

    fn d1(&self, u: f64, v: f64) -> (Point3, Vec3, Vec3) {
        let ax = &self.ax;
        let (rad, drad) = (ax.radial(u), ax.dradial(u));
        match self.quad {
            Quad::Plane => (ax.o + ax.x * u + ax.y * v, ax.x, ax.y),
            Quad::Cylinder { r } => (ax.o + rad * r + ax.z * v, drad * r, ax.z),
            Quad::Cone { r, a } => {
                let (s, c) = a.sin_cos();
                let w = r + v * s;
                (
                    ax.o + rad * w + ax.z * (v * c),
                    drad * w,
                    rad * s + ax.z * c,
                )
            }
            Quad::Sphere { r } => {
                let (s, c) = v.sin_cos();
                (
                    ax.o + rad * (r * c) + ax.z * (r * s),
                    drad * (r * c),
                    rad * (-r * s) + ax.z * (r * c),
                )
            }
            Quad::Torus { major, minor } => {
                let (s, c) = v.sin_cos();
                let w = major + minor * c;
                (
                    ax.o + rad * w + ax.z * (minor * s),
                    drad * w,
                    rad * (-minor * s) + ax.z * (minor * c),
                )
            }
        }
    }

    /// `du x dv` as a unit vector in closed form, defined at the sphere's
    /// poles; `None` at a cone's apex.
    fn normal(&self, u: f64, v: f64) -> Option<Vec3> {
        let ax = &self.ax;
        let n = match self.quad {
            Quad::Plane => ax.z,
            Quad::Cylinder { .. } => ax.radial(u),
            Quad::Cone { r, a } => {
                let (s, c) = a.sin_cos();
                let w = r + v * s;
                if w.abs() <= 1e-12 * (1.0 + r.abs()) {
                    return None;
                }
                (ax.radial(u) * c - ax.z * s) * w.signum()
            }
            Quad::Sphere { .. } => {
                let (s, c) = v.sin_cos();
                let n = ax.radial(u) * c + ax.z * s;
                if c < -1e-9 { -n } else { n }
            }
            Quad::Torus { major, minor } => {
                let (s, c) = v.sin_cos();
                let w = major + minor * c;
                (ax.radial(u) * c + ax.z * s) * if w < 0.0 { -1.0 } else { 1.0 }
            }
        };
        Some(n * self.ax.hand)
    }

    /// (u, v) of a point known to lie on the surface.
    fn uv_of(&self, p: Point3) -> (f64, f64) {
        let (lx, ly, lz) = self.ax.local(p);
        let u = ly.atan2(lx);
        let rho = lx.hypot(ly);
        match self.quad {
            Quad::Plane => (lx, ly),
            Quad::Cylinder { .. } => (u, lz),
            Quad::Cone { r, a } => {
                let (s, c) = a.sin_cos();
                let v = lz / c;
                if r + v * s < 0.0 { (u + PI, v) } else { (u, v) }
            }
            Quad::Sphere { .. } => (u, lz.atan2(rho)),
            Quad::Torus { major, .. } => (u, lz.atan2(rho - major)),
        }
    }

    /// Closest-point candidates of the whole surface, as (u, v); and whether
    /// the point is on the axis (u arbitrary).
    fn project(&self, p: Point3, scale: f64) -> (Vec<(f64, f64)>, bool) {
        let (lx, ly, lz) = self.ax.local(p);
        let u = ly.atan2(lx);
        let rho = lx.hypot(ly);
        let on_axis = self.revolved() && rho <= 1e-10 * (1.0 + scale);
        let c = match self.quad {
            Quad::Plane => vec![(lx, ly)],
            Quad::Cylinder { .. } => vec![(u, lz)],
            Quad::Cone { r, a } => {
                let (s, c) = a.sin_cos();
                vec![
                    (u, (rho - r) * s + lz * c),
                    (u + PI, -(rho + r) * s + lz * c),
                ]
            }
            Quad::Sphere { .. } => vec![(u, lz.atan2(rho))],
            Quad::Torus { major, .. } => vec![(u, lz.atan2(rho - major))],
        };
        (c, on_axis)
    }

    /// Parameters t of every intersection of the ray with the (untrimmed)
    /// surface, unsorted and unfiltered.
    fn ray_ts(&self, o: Point3, d: Vec3) -> Vec<f64> {
        let (px, py, pz) = self.ax.local(o);
        let dx = d.dot(self.ax.x);
        let dy = d.dot(self.ax.y);
        let dz = d.dot(self.ax.z);
        let mut ts = Vec::new();
        match self.quad {
            Quad::Plane => {
                if dz.abs() > 1e-14 {
                    ts.push(-pz / dz);
                }
            }
            Quad::Cylinder { r } => quad_roots(
                dx * dx + dy * dy,
                2.0 * (px * dx + py * dy),
                px * px + py * py - r * r,
                &mut ts,
            ),
            Quad::Cone { r, a } => {
                let k = a.tan();
                let c0 = r + k * pz;
                quad_roots(
                    dx * dx + dy * dy - k * k * dz * dz,
                    2.0 * (px * dx + py * dy - c0 * k * dz),
                    px * px + py * py - c0 * c0,
                    &mut ts,
                );
            }
            Quad::Sphere { r } => quad_roots(
                1.0,
                2.0 * (px * dx + py * dy + pz * dz),
                px * px + py * py + pz * pz - r * r,
                &mut ts,
            ),
            Quad::Torus { major, minor } => {
                // (|x|^2 + R^2 - r^2)^2 = 4 R^2 (x^2 + y^2), a quartic in t.
                let a2 = dx * dx + dy * dy + dz * dz;
                let a1 = 2.0 * (px * dx + py * dy + pz * dz);
                let a0 = px * px + py * py + pz * pz + major * major - minor * minor;
                let k = 4.0 * major * major;
                let c = [
                    a0 * a0 - k * (px * px + py * py),
                    2.0 * a1 * a0 - k * 2.0 * (px * dx + py * dy),
                    a1 * a1 + 2.0 * a2 * a0 - k * (dx * dx + dy * dy),
                    2.0 * a2 * a1,
                    a2 * a2,
                ];
                ts = real_roots(&c);
            }
        }
        ts
    }
}

// ---------------------------------------------------------------------------
// Rational B-splines.

#[derive(Debug, Clone)]
struct Nurbs {
    pu: usize,
    pv: usize,
    nu: usize,
    nv: usize,
    uk: Vec<f64>,
    vk: Vec<f64>,
    poles: Vec<Point3>,
    w: Vec<f64>,
    /// Typical lengths of the first derivatives, for the degeneracy tests.
    scale_u: f64,
    scale_v: f64,
}

fn span(k: &[f64], deg: usize, n: usize, u: f64) -> usize {
    if u >= at(k, n) {
        return n - 1;
    }
    if u <= at(k, deg) {
        return deg;
    }
    let (mut lo, mut hi) = (deg, n);
    while hi - lo > 1 {
        let mid = (lo + hi) / 2;
        if u < at(k, mid) {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    lo
}

/// The non-zero basis functions at `u` (Piegl and Tiller A2.2).
fn basis_values(k: &[f64], deg: usize, sp: usize, u: f64) -> Vec<f64> {
    let mut n = vec![0.0; deg + 1];
    let mut left = vec![0.0; deg + 1];
    let mut right = vec![0.0; deg + 1];
    put(&mut n, 0, 1.0);
    for j in 1..=deg {
        put(&mut left, j, u - at(k, sp + 1 - j));
        put(&mut right, j, at(k, sp + j) - u);
        let mut saved = 0.0;
        for r in 0..j {
            let t = at(&n, r) / (at(&right, r + 1) + at(&left, j - r));
            put(&mut n, r, saved + at(&right, r + 1) * t);
            saved = at(&left, j - r) * t;
        }
        put(&mut n, j, saved);
    }
    n
}

fn basis(k: &[f64], deg: usize, sp: usize, u: f64) -> (Vec<f64>, Vec<f64>) {
    let n0 = basis_values(k, deg, sp, u);
    let mut d1 = vec![0.0; deg + 1];
    if deg > 0 {
        let lb = basis_values(k, deg - 1, sp, u);
        for r in 0..=deg {
            let i = sp + r - deg;
            let mut d = 0.0;
            if r >= 1 {
                let den = at(k, i + deg) - at(k, i);
                if den > 0.0 {
                    d += at(&lb, r - 1) / den;
                }
            }
            if r < deg {
                let den = at(k, i + deg + 1) - at(k, i + 1);
                if den > 0.0 {
                    d -= at(&lb, r) / den;
                }
            }
            put(&mut d1, r, d * deg as f64);
        }
    }
    (n0, d1)
}

impl Nurbs {
    fn new(face: u32, p: &BSplinePatch) -> Result<Self> {
        p.validate().map_err(|e| bad(face, e))?;
        let (pu, pv) = (p.degree_u as usize, p.degree_v as usize);
        let (nu, nv) = (p.poles_u as usize, p.poles_v as usize);
        if pu > 24 || pv > 24 {
            return Err(bad(face, "b-spline degree above 24 is not supported"));
        }
        let clamped = |k: &[f64], deg: usize, n: usize| {
            k.iter().take(deg + 1).all(|x| same(*x, at(k, 0)))
                && (n..n + deg + 1).all(|i| same(at(k, i), at(k, n)))
                && at(k, n) > at(k, deg)
        };
        if !clamped(&p.knots_u, pu, nu) || !clamped(&p.knots_v, pv, nv) {
            return Err(bad(
                face,
                "the knot vectors must be clamped with a non-empty domain",
            ));
        }
        let mut n = Self {
            pu,
            pv,
            nu,
            nv,
            uk: p.knots_u.clone(),
            vk: p.knots_v.clone(),
            poles: p.poles.clone(),
            w: p.weights.clone(),
            scale_u: 1.0,
            scale_v: 1.0,
        };
        let hull = Self::hull(&n.poles);
        let diag = (hull.max - hull.min).length().max(1e-12);
        let (u0, u1) = n.u_domain();
        let (v0, v1) = n.v_domain();
        n.scale_u = diag / (u1 - u0);
        n.scale_v = diag / (v1 - v0);
        Ok(n)
    }

    fn hull(points: &[Point3]) -> Aabb {
        let mut min = Point3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
        let mut max = Point3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
        for p in points {
            min = Point3::new(min.x.min(p.x), min.y.min(p.y), min.z.min(p.z));
            max = Point3::new(max.x.max(p.x), max.y.max(p.y), max.z.max(p.z));
        }
        Aabb { min, max }
    }

    fn u_domain(&self) -> (f64, f64) {
        (at(&self.uk, self.pu), at(&self.uk, self.nu))
    }

    fn v_domain(&self) -> (f64, f64) {
        (at(&self.vk, self.pv), at(&self.vk, self.nv))
    }

    /// Point and first partial derivatives (parameters clamped to the domain).
    fn d1(&self, u: f64, v: f64) -> Option<(Point3, Vec3, Vec3)> {
        let (u0, u1) = self.u_domain();
        let (v0, v1) = self.v_domain();
        let (u, v) = (u.clamp(u0, u1), v.clamp(v0, v1));
        if !u.is_finite() || !v.is_finite() {
            return None;
        }
        let su = span(&self.uk, self.pu, self.nu, u);
        let sv = span(&self.vk, self.pv, self.nv, v);
        let (bu, dbu) = basis(&self.uk, self.pu, su, u);
        let (bv, dbv) = basis(&self.vk, self.pv, sv, v);
        let (mut a, mut au, mut av) = (Vec3::ZERO, Vec3::ZERO, Vec3::ZERO);
        let (mut w, mut wu, mut wv) = (0.0, 0.0, 0.0);
        let (bu0, bv0) = (su.checked_sub(self.pu)?, sv.checked_sub(self.pv)?);
        for i in 0..=self.pu {
            for j in 0..=self.pv {
                let idx = (bu0 + i) * self.nv + (bv0 + j);
                let p = self.poles.get(idx)?.to_vec();
                let wt = *self.w.get(idx)?;
                let n = at(&bu, i) * at(&bv, j) * wt;
                let nu_ = at(&dbu, i) * at(&bv, j) * wt;
                let nv_ = at(&bu, i) * at(&dbv, j) * wt;
                a = a + p * n;
                au = au + p * nu_;
                av = av + p * nv_;
                w += n;
                wu += nu_;
                wv += nv_;
            }
        }
        if !(w.is_finite() && w > 0.0) {
            return None;
        }
        let s = a * (1.0 / w);
        let d_u = (au - s * wu) * (1.0 / w);
        let d_v = (av - s * wv) * (1.0 / w);
        let p = Point3::new(s.x, s.y, s.z);
        (p.is_finite() && d_u.is_finite() && d_v.is_finite()).then_some((p, d_u, d_v))
    }

    /// The unit normal `du x dv`, or `None` where the parametrisation is
    /// degenerate (a zero-length derivative or parallel derivatives).
    fn normal(&self, u: f64, v: f64) -> Option<Vec3> {
        let (_, du, dv) = self.d1(u, v)?;
        let c = du.cross(dv);
        let (lu, lv, lc) = (du.length(), dv.length(), c.length());
        if lu <= 1e-9 * self.scale_u || lv <= 1e-9 * self.scale_v || lc <= 1e-9 * lu * lv {
            return None;
        }
        Some(c * (1.0 / lc))
    }
}

/// A cached grid of surface points over the effective parameter box.
#[derive(Debug, Clone)]
struct Grid {
    nu: usize,
    nv: usize,
    pts: Vec<Point3>,
    /// The largest distance to a grid neighbour, per point.
    reach: Vec<f64>,
    singular: Vec<bool>,
}

// ---------------------------------------------------------------------------
// The face.

#[derive(Debug, Clone)]
enum Geometry {
    Quad(Analytic),
    Nurbs(Box<Nurbs>, Grid),
}

/// One exact trimmed face, evaluated in pure Rust (IMP-02).
#[derive(Debug, Clone)]
pub struct ExactFace {
    number: u32,
    geometry: Geometry,
    reversed: bool,
    /// The parameter box `[u_min, u_max, v_min, v_max]`.
    bounds: [f64; 4],
    /// The box the iterations may use (the box within the B-spline's domain).
    search: [f64; 4],
    loops: Vec<Vec<(f64, f64)>>,
    period_u: Option<f64>,
    period_v: Option<f64>,
    vertices: Vec<Point3>,
    bbox: Aabb,
    scale: f64,
}

const TOL: f64 = 1e-12;

impl ExactFace {
    /// Builds the evaluator for `face`. Fails, with a message naming the face,
    /// when its frame, sizes, B-spline data, parameter box, trim loops or
    /// tessellation are invalid (NFR-07).
    pub fn new(face: &BrepFace) -> Result<Self> {
        let n = face.id.0;
        let b = face.uv_bounds;
        if b.iter().any(|x| !x.is_finite()) || b[0] >= b[1] || b[2] >= b[3] {
            return Err(bad(n, "the parameter box must be finite and non-empty"));
        }
        face.tessellation.validate().map_err(|e| bad(n, e))?;
        let mut loops = Vec::new();
        for l in &face.trims {
            if l.0.iter().any(|p| !p.u.is_finite() || !p.v.is_finite()) {
                return Err(bad(n, "a trim loop has a coordinate that is not finite"));
            }
            if l.0.len() < 3 {
                return Err(bad(n, "a trim loop needs at least three points"));
            }
            loops.push(l.0.iter().map(|p| (p.u, p.v)).collect::<Vec<_>>());
        }
        let positive = |x: f64, what: &str| {
            if x.is_finite() && x > 0.0 {
                Ok(x)
            } else {
                Err(bad(n, format!("the {what} must be positive and finite")))
            }
        };
        let quad = |frame: &Frame, quad: Quad| -> Result<Analytic> {
            Ok(Analytic {
                ax: Axes::new(n, frame)?,
                quad,
            })
        };
        let analytic = match &face.geometry {
            Surface::Plane { frame } => Some(quad(frame, Quad::Plane)?),
            Surface::Cylinder { frame, radius } => Some(quad(
                frame,
                Quad::Cylinder {
                    r: positive(*radius, "radius")?,
                },
            )?),
            Surface::Cone {
                frame,
                radius,
                semi_angle,
            } => {
                if !radius.is_finite() || !semi_angle.is_finite() || semi_angle.abs() >= 1.5 {
                    return Err(bad(
                        n,
                        "the cone needs a finite radius and |semi-angle| < 1.5",
                    ));
                }
                Some(quad(
                    frame,
                    Quad::Cone {
                        r: *radius,
                        a: *semi_angle,
                    },
                )?)
            }
            Surface::Sphere { frame, radius } => Some(quad(
                frame,
                Quad::Sphere {
                    r: positive(*radius, "radius")?,
                },
            )?),
            Surface::Torus {
                frame,
                major,
                minor,
            } => Some(quad(
                frame,
                Quad::Torus {
                    major: positive(*major, "major radius")?,
                    minor: positive(*minor, "minor radius")?,
                },
            )?),
            Surface::BSpline(_) => None,
            _ => return Err(bad(n, "unsupported surface kind")),
        };
        let (geometry, period_u, period_v, search) = match (analytic, &face.geometry) {
            (Some(a), _) => {
                let (pu, pv) = a.periods();
                (Geometry::Quad(a), pu, pv, b)
            }
            (None, Surface::BSpline(p)) => {
                let nurbs = Nurbs::new(n, p)?;
                let (u0, u1) = nurbs.u_domain();
                let (v0, v1) = nurbs.v_domain();
                let s = [b[0].max(u0), b[1].min(u1), b[2].max(v0), b[3].min(v1)];
                if s[0] >= s[1] || s[2] >= s[3] {
                    return Err(bad(n, "the parameter box lies outside the patch's domain"));
                }
                let grid = Self::build_grid(&nurbs, s)?;
                (Geometry::Nurbs(Box::new(nurbs), grid), None, None, s)
            }
            _ => return Err(bad(n, "unsupported surface")),
        };
        let mut out = Self {
            number: n,
            geometry,
            reversed: face.reversed,
            bounds: b,
            search,
            loops,
            period_u,
            period_v,
            vertices: face.tessellation.vertices.clone(),
            bbox: Aabb {
                min: Point3::ORIGIN,
                max: Point3::ORIGIN,
            },
            scale: 1.0,
        };
        out.bbox = out.compute_bbox(&face.tessellation)?;
        out.scale = (out.bbox.max - out.bbox.min).length().max(1e-12);
        Ok(out)
    }

    fn build_grid(n: &Nurbs, s: [f64; 4]) -> Result<Grid> {
        let gu = (4 * n.nu).clamp(16, 48) + 1;
        let gv = (4 * n.nv).clamp(16, 48) + 1;
        let mut pts = Vec::with_capacity(gu * gv);
        let mut singular = Vec::with_capacity(gu * gv);
        for i in 0..gu {
            for j in 0..gv {
                let u = s[0] + (s[1] - s[0]) * i as f64 / (gu - 1) as f64;
                let v = s[2] + (s[3] - s[2]) * j as f64 / (gv - 1) as f64;
                let (p, du, dv) = n
                    .d1(u, v)
                    .ok_or_else(|| PathError::Surface("b-spline cannot be evaluated".to_owned()))?;
                pts.push(p);
                singular.push(du.length() <= 1e-6 * n.scale_u || dv.length() <= 1e-6 * n.scale_v);
            }
        }
        let mut reach = vec![0.0; pts.len()];
        for i in 0..gu {
            for j in 0..gv {
                let here = pts.get(i * gv + j).copied().unwrap_or(Point3::ORIGIN);
                let mut m = 0.0_f64;
                for (a, b) in [(1, 0), (0, 1)] {
                    for (si, sj) in [(i + a, j + b), (i.wrapping_sub(a), j.wrapping_sub(b))] {
                        if si < gu && sj < gv {
                            let q = pts.get(si * gv + sj).copied().unwrap_or(here);
                            m = m.max(here.distance(q));
                        }
                    }
                }
                put(&mut reach, i * gv + j, m);
            }
        }
        Ok(Grid {
            nu: gu,
            nv: gv,
            pts,
            reach,
            singular,
        })
    }

    fn compute_bbox(&self, tess: &TriangleMesh) -> Result<Aabb> {
        let mut pts: Vec<Point3> = tess.vertices.clone();
        match &self.geometry {
            Geometry::Nurbs(n, _) => pts.extend(n.poles.iter().copied()),
            Geometry::Quad(a) => {
                let b = self.bounds;
                let g = 32;
                for i in 0..=g {
                    for j in 0..=g {
                        let u = b[0] + (b[1] - b[0]) * i as f64 / g as f64;
                        let v = b[2] + (b[3] - b[2]) * j as f64 / g as f64;
                        pts.push(a.d1(u, v).0);
                    }
                }
            }
        }
        if pts.is_empty() || pts.iter().any(|p| !p.is_finite()) {
            return Err(bad(self.number, "the face has no finite extent"));
        }
        let h = Nurbs::hull(&pts);
        let margin = match self.geometry {
            Geometry::Nurbs(..) => 1e-9,
            Geometry::Quad(_) => 0.02 * (h.max - h.min).length() + 1e-9,
        };
        let m = Vec3::new(margin, margin, margin);
        Ok(Aabb {
            min: Point3::new(h.min.x - m.x, h.min.y - m.y, h.min.z - m.z),
            max: Point3::new(h.max.x + m.x, h.max.y + m.y, h.max.z + m.z),
        })
    }

    /// The face number given by the importer (`BrepFace::id`).
    pub fn number(&self) -> u32 {
        self.number
    }

    /// A box that holds the whole face (it may be larger than the face).
    pub fn bounding_box(&self) -> Aabb {
        self.bbox
    }

    fn d1(&self, u: f64, v: f64) -> Option<(Point3, Vec3, Vec3)> {
        match &self.geometry {
            Geometry::Quad(a) => Some(a.d1(u, v)),
            Geometry::Nurbs(n, _) => n.d1(u, v),
        }
    }

    fn eval(&self, u: f64, v: f64) -> Option<Point3> {
        match &self.geometry {
            Geometry::Quad(a) => Some(a.d1(u, v).0),
            Geometry::Nurbs(n, _) => n.d1(u, v).map(|d| d.0),
        }
    }

    /// The outward unit normal at (u, v), with a fallback direction for a
    /// degenerate point.
    fn normal(&self, u: f64, v: f64) -> Option<Vec3> {
        let n = match &self.geometry {
            Geometry::Quad(a) => a.normal(u, v),
            Geometry::Nurbs(n, _) => n.normal(u, v).or_else(|| {
                // At a pole: the limit, taken a hair towards the box centre.
                let (cu, cv) = (
                    0.5 * (self.search[0] + self.search[1]),
                    0.5 * (self.search[2] + self.search[3]),
                );
                [1e-7, 1e-5, 1e-3]
                    .into_iter()
                    .find_map(|k| n.normal(u + (cu - u) * k, v + (cv - v) * k))
            }),
        }?;
        Some(if self.reversed { -n } else { n })
    }

    fn surface_point(&self, u: f64, v: f64, fallback: Vec3) -> Option<SurfacePoint> {
        let point = self.eval(u, v)?;
        let normal = self
            .normal(u, v)
            .and_then(|n| UnitVec3::new(n).ok())
            .or_else(|| UnitVec3::new(fallback).ok())
            .unwrap_or(UnitVec3::Z);
        Some(SurfacePoint {
            point,
            normal,
            position: Some(FacePosition {
                face: self.number,
                uv: Uv { u, v },
            }),
        })
    }

    fn wrapped(&self, uv: Uv) -> (f64, f64) {
        let u = self
            .period_u
            .map_or(uv.u, |p| wrap(uv.u, self.bounds[0], p));
        let v = self
            .period_v
            .map_or(uv.v, |p| wrap(uv.v, self.bounds[2], p));
        (u, v)
    }

    fn in_box(&self, u: f64, v: f64) -> bool {
        let b = self.bounds;
        let (tu, tv) = (TOL * (1.0 + b[1] - b[0]), TOL * (1.0 + b[3] - b[2]));
        u >= b[0] - tu && u <= b[1] + tu && v >= b[2] - tv && v <= b[3] + tv
    }

    fn inside_one(&self, u: f64, v: f64) -> bool {
        if self.loops.is_empty() {
            return self.in_box(u, v);
        }
        let mut odd = false;
        for l in &self.loops {
            let mut prev = l.last().copied().unwrap_or((u, v));
            for &(x, y) in l {
                let (px, py) = prev;
                if (y > v) != (py > v) && u < (px - x) * (v - y) / (py - y) + x {
                    odd = !odd;
                }
                prev = (x, y);
            }
        }
        odd
    }

    /// The loops used for boundary searches: the trims, or the box.
    fn boundary_loops(&self) -> Vec<Vec<(f64, f64)>> {
        if self.loops.is_empty() {
            let b = self.bounds;
            vec![vec![(b[0], b[2]), (b[1], b[2]), (b[1], b[3]), (b[0], b[3])]]
        } else {
            self.loops.clone()
        }
    }

    fn clamp_search(&self, u: f64, v: f64) -> (f64, f64) {
        let s = self.search;
        (u.clamp(s[0], s[1]), v.clamp(s[2], s[3]))
    }

    /// Distance squared from `q` to the surface at (u, v); infinite when it
    /// cannot be evaluated.
    fn d2_at(&self, q: Point3, u: f64, v: f64) -> f64 {
        self.eval(u, v).map_or(f64::INFINITY, |p| dist2(p, q))
    }

    // ---- closest point ----------------------------------------------------

    /// The closest point on the trim loops.
    fn boundary_closest(&self, q: Point3) -> Option<(f64, f64, f64)> {
        let mut best: Option<(f64, f64, f64)> = None;
        for l in self.boundary_loops() {
            let mut prev = l.last().copied()?;
            for &(x, y) in &l {
                let (px, py) = prev;
                prev = (x, y);
                let f = |t: f64| {
                    let (u, v) = self.clamp_search_if_nurbs(px + t * (x - px), py + t * (y - py));
                    (self.d2_at(q, u, v), u, v)
                };
                let n = 8;
                let mut k_best = (f64::INFINITY, 0usize);
                for k in 0..=n {
                    let d = f(k as f64 / n as f64).0;
                    if d < k_best.0 {
                        k_best = (d, k);
                    }
                }
                let mut lo = (k_best.1.saturating_sub(1)) as f64 / n as f64;
                let mut hi = ((k_best.1 + 1).min(n)) as f64 / n as f64;
                for _ in 0..60 {
                    let m1 = lo + (hi - lo) / 3.0;
                    let m2 = hi - (hi - lo) / 3.0;
                    if f(m1).0 < f(m2).0 {
                        hi = m2;
                    } else {
                        lo = m1;
                    }
                }
                let mut t = 0.5 * (lo + hi);
                // Newton on the stationarity condition (S - q) . dS/dt = 0 with
                // a differenced slope: the search above compares distances and
                // is only good to their square root.
                let g = |t: f64| -> Option<f64> {
                    let (uu, vv) = (px + t * (x - px), py + t * (y - py));
                    let (sp, su, sv) = self.d1(uu, vv)?;
                    Some((sp - q).dot(su * (x - px) + sv * (y - py)))
                };
                for _ in 0..12 {
                    let h = 1e-6;
                    let (t1, t2) = ((t - h).max(0.0), (t + h).min(1.0));
                    let (Some(g0), Some(g1), Some(g2)) = (g(t), g(t1), g(t2)) else {
                        break;
                    };
                    let slope = (g2 - g1) / (t2 - t1);
                    if slope.is_nan() || slope <= 0.0 {
                        break;
                    }
                    let nt = (t - g0 / slope).clamp(0.0, 1.0);
                    if f(nt).0 <= f(t).0 * (1.0 + 1e-13) {
                        t = nt;
                    } else {
                        break;
                    }
                }
                let (d, u, v) = f(t);
                if d.is_finite() && best.is_none_or(|b| d < b.0) {
                    best = Some((d, u, v));
                }
            }
        }
        best.map(|(d, u, v)| (d.sqrt(), u, v))
    }

    fn clamp_search_if_nurbs(&self, u: f64, v: f64) -> (f64, f64) {
        match self.geometry {
            Geometry::Nurbs(..) => self.clamp_search(u, v),
            Geometry::Quad(_) => (u, v),
        }
    }

    fn quad_candidates(&self, a: &Analytic, q: Point3) -> Vec<(f64, f64, f64)> {
        let (cands, on_axis) = a.project(q, self.scale);
        let mut out = Vec::new();
        for (u, v) in cands {
            let (u, v) = self.wrapped(Uv { u, v });
            if on_axis {
                let width = (self.bounds[1] - self.bounds[0]).min(self.period_u.unwrap_or(TAU));
                for k in 0..64 {
                    let uu = self.bounds[0] + width * k as f64 / 63.0;
                    out.push((self.d2_at(q, uu, v).sqrt(), uu, v));
                }
            } else {
                out.push((self.d2_at(q, u, v).sqrt(), u, v));
            }
        }
        out
    }

    /// `r . S_uu`, `r . S_uv` and `r . S_vv` by central differences of the
    /// first derivatives over a small step (one-sided at the box edge).
    fn curvature_terms(&self, n: &Nurbs, u: f64, v: f64, r: Vec3) -> (f64, f64, f64) {
        let s = self.search;
        let (hu, hv) = (1e-6 * (s[1] - s[0]), 1e-6 * (s[3] - s[2]));
        let (up, um) = ((u + hu).min(s[1]), (u - hu).max(s[0]));
        let (vp, vm) = ((v + hv).min(s[3]), (v - hv).max(s[2]));
        let (du, dv) = (up - um, vp - vm);
        let (Some(a), Some(b), Some(c), Some(d)) =
            (n.d1(up, v), n.d1(um, v), n.d1(u, vp), n.d1(u, vm))
        else {
            return (0.0, 0.0, 0.0);
        };
        let ok = du > 0.0 && dv > 0.0;
        if !ok {
            return (0.0, 0.0, 0.0);
        }
        let suu = (a.1 - b.1) * (1.0 / du);
        let suv = (c.1 - d.1) * (1.0 / dv);
        let svv = (c.2 - d.2) * (1.0 / dv);
        (r.dot(suu), r.dot(suv), r.dot(svv))
    }

    /// Damped Newton on `|S(u, v) - q|^2`, clamped to the search box.
    fn lm_closest(&self, n: &Nurbs, q: Point3, mut u: f64, mut v: f64) -> (f64, f64, f64) {
        let mut lambda = 1e-3;
        let mut f = self.d2_at(q, u, v);
        for _ in 0..80 {
            let Some((p, su, sv)) = n.d1(u, v) else { break };
            let r = p - q;
            let (a, b, c) = (su.dot(su), su.dot(sv), sv.dot(sv));
            let (g1, g2) = (r.dot(su), r.dot(sv));
            // The curvature term r . S_ij, from differences of the first
            // derivatives. Without it the iteration converges only linearly
            // (or not at all) from far away on a convex surface.
            let (huu, huv, hvv) = self.curvature_terms(n, u, v, r);
            let (a, b, c) = (a + huu, b + huv, c + hvv);
            let mu = 1e-12 * (a.abs() + c.abs()) + 1e-300;
            let mut accepted = false;
            for _ in 0..20 {
                // A parameter held on the box edge by its gradient is fixed;
                // the other is solved alone (a projected Newton step).
                let sb = self.search;
                let fix_u = (u <= sb[0] && g1 > 0.0) || (u >= sb[1] && g1 < 0.0);
                let fix_v = (v <= sb[2] && g2 > 0.0) || (v >= sb[3] && g2 < 0.0);
                let (haa, hcc) = (a + lambda * (a.abs() + mu), c + lambda * (c.abs() + mu));
                let step = match (fix_u, fix_v) {
                    (true, true) => Some([0.0, 0.0]),
                    (true, false) => (hcc > 0.0).then(|| [0.0, -g2 / hcc]),
                    (false, true) => (haa > 0.0).then(|| [-g1 / haa, 0.0]),
                    (false, false) => solve2([[haa, b], [b, hcc]], [-g1, -g2]),
                };
                let Some([du, dv]) = step else {
                    lambda *= 4.0;
                    continue;
                };
                let (nu, nv) = self.clamp_search(u + du, v + dv);
                let fn_ = self.d2_at(q, nu, nv);
                // Equal to rounding counts: near the minimum the distance is
                // flat, and the step is what pins the position down.
                if fn_ <= f * (1.0 + 1e-13) {
                    let moved = (nu - u).abs() + (nv - v).abs();
                    (u, v, f) = (nu, nv, fn_);
                    lambda = (lambda * 0.2).max(1e-12);
                    accepted = moved > 1e-14;
                    break;
                }
                lambda *= 4.0;
            }
            if !accepted {
                break;
            }
        }
        (u, v, f.sqrt())
    }

    /// A derivative-free pattern search from (u, v), shrinking from one grid
    /// cell; it moves off the stalls a gradient method has at singular points.
    fn polish(&self, q: Point3, mut u: f64, mut v: f64) -> (f64, f64, f64) {
        let s = self.search;
        let (mut hu, mut hv) = ((s[1] - s[0]) / 16.0, (s[3] - s[2]) / 16.0);
        let mut f = self.d2_at(q, u, v);
        for _ in 0..40 {
            let mut best = (f, u, v);
            for (du, dv) in [
                (1.0, 0.0),
                (-1.0, 0.0),
                (0.0, 1.0),
                (0.0, -1.0),
                (1.0, 1.0),
                (1.0, -1.0),
                (-1.0, 1.0),
                (-1.0, -1.0),
            ] {
                let (nu, nv) = self.clamp_search(u + du * hu, v + dv * hv);
                let d = self.d2_at(q, nu, nv);
                if d < best.0 {
                    best = (d, nu, nv);
                }
            }
            if best.0 < f {
                (f, u, v) = best;
            } else {
                hu *= 0.5;
                hv *= 0.5;
            }
        }
        (u, v, f.sqrt())
    }

    fn nurbs_candidates(&self, n: &Nurbs, g: &Grid, q: Point3) -> Vec<(f64, f64, f64)> {
        let s = self.search;
        let uv_of = |idx: usize| {
            let (i, j) = (idx / g.nv, idx % g.nv);
            (
                s[0] + (s[1] - s[0]) * i as f64 / (g.nu - 1) as f64,
                s[2] + (s[3] - s[2]) * j as f64 / (g.nv - 1) as f64,
            )
        };
        let d_of = |idx: usize| g.pts.get(idx).map_or(f64::INFINITY, |p| dist2(*p, q));
        let mut ranked: Vec<(f64, usize)> = (0..g.pts.len()).map(|i| (d_of(i), i)).collect();
        ranked.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let mut seeds: Vec<usize> = Vec::new();
        // The nearest regular grid points.
        for &(_, i) in &ranked {
            if seeds.len() >= 2 {
                break;
            }
            if !g.singular.get(i).copied().unwrap_or(false) {
                seeds.push(i);
            }
        }
        // The grid's local minima, nearest first.
        let mut minima = 0;
        for &(d, i) in &ranked {
            if minima >= 4 {
                break;
            }
            let (gi, gj) = (i / g.nv, i % g.nv);
            let mut is_min = true;
            for (di, dj) in [
                (-1i64, -1i64),
                (-1, 0),
                (-1, 1),
                (0, -1),
                (0, 1),
                (1, -1),
                (1, 0),
                (1, 1),
            ] {
                let (ni, nj) = (gi as i64 + di, gj as i64 + dj);
                if ni < 0 || nj < 0 || ni as usize >= g.nu || nj as usize >= g.nv {
                    continue;
                }
                let k = ni as usize * g.nv + nj as usize;
                let dk = d_of(k);
                if dk < d || (dk <= d && k < i) {
                    is_min = false;
                    break;
                }
            }
            if is_min {
                minima += 1;
                if !seeds.contains(&i) {
                    seeds.push(i);
                }
            }
        }
        // The grid point nearest the nearest tessellation vertex.
        let near_vertex = self
            .vertices
            .iter()
            .min_by(|a, b| dist2(**a, q).total_cmp(&dist2(**b, q)));
        if let Some(tv) = near_vertex
            && let Some(&(_, i)) = ranked.iter().min_by(|a, b| {
                let da = g.pts.get(a.1).map_or(f64::INFINITY, |p| dist2(*p, *tv));
                let db = g.pts.get(b.1).map_or(f64::INFINITY, |p| dist2(*p, *tv));
                da.total_cmp(&db).then(a.1.cmp(&b.1))
            })
            && !seeds.contains(&i)
        {
            seeds.push(i);
        }
        let mut found: Vec<(f64, f64, f64)> = seeds
            .iter()
            .map(|&i| {
                let (u, v) = uv_of(i);
                let (u, v, d) = self.lm_closest(n, q, u, v);
                (d, u, v)
            })
            .collect();
        found.sort_by(|a, b| a.0.total_cmp(&b.0));
        found
            .into_iter()
            .enumerate()
            .map(|(k, (d, u, v))| {
                if k < 2 {
                    let (pu, pv, pd) = self.polish(q, u, v);
                    // Only a real gain beats the Gauss-Newton answer, whose position is
                    // better pinned down than the search's.
                    if pd < d * (1.0 - 1e-9) {
                        (pd, pu, pv)
                    } else {
                        (d, u, v)
                    }
                } else {
                    (d, u, v)
                }
            })
            .collect()
    }

    // ---- rays -------------------------------------------------------------

    fn make_hit(&self, t: f64, u: f64, v: f64, p: Point3, dir: Vec3) -> Option<RayHit> {
        let sp = self.surface_point(u, v, -dir)?;
        Some(RayHit {
            distance: t,
            point: p,
            normal: sp.normal,
            position: sp.position,
        })
    }

    fn nurbs_ray(
        &self,
        n: &Nurbs,
        g: &Grid,
        o: Point3,
        d: Vec3,
        max: f64,
    ) -> Vec<(f64, f64, f64, Point3)> {
        let s = self.search;
        let tol = 1e-10 + 1e-12 * self.scale;
        let mut hits: Vec<(f64, f64, f64, Point3)> = Vec::new();
        for (idx, p) in g.pts.iter().enumerate() {
            let reach = g.reach.get(idx).copied().unwrap_or(0.0);
            let w = *p - o;
            let t0 = w.dot(d);
            let off = (w - d * t0).length();
            if off > 1.25 * reach + 1e-9 || t0 < -reach - 1e-9 || t0 > max + reach + 1e-9 {
                continue;
            }
            let (i, j) = (idx / g.nv, idx % g.nv);
            let u = s[0] + (s[1] - s[0]) * i as f64 / (g.nu - 1) as f64;
            let v = s[2] + (s[3] - s[2]) * j as f64 / (g.nv - 1) as f64;
            let Some((u, v, _)) = self.lm_ray(n, o, d, (u, v, t0), tol) else {
                continue;
            };
            let Some((p, _, _)) = n.d1(u, v) else {
                continue;
            };
            let t = (p - o).dot(d);
            if !(-1e-9..=max + 1e-9).contains(&t) {
                continue;
            }
            let t = t.max(0.0);
            if hits
                .iter()
                .any(|h| (h.0 - t).abs() < 1e-7 && h.3.distance(p) < 1e-7)
            {
                continue;
            }
            hits.push((t, u, v, p));
        }
        hits
    }

    /// Damped Newton on `S(u, v) - o - t d = 0`, clamped to the search box.
    fn lm_ray(
        &self,
        n: &Nurbs,
        o: Point3,
        d: Vec3,
        start: (f64, f64, f64),
        tol: f64,
    ) -> Option<(f64, f64, f64)> {
        let (mut u, mut v, mut t) = start;
        let resid = |u: f64, v: f64, t: f64| n.d1(u, v).map(|(p, su, sv)| (p - o - d * t, su, sv));
        let (mut r, mut su, mut sv) = resid(u, v, t)?;
        let mut f = r.length();
        let mut lambda = 1e-4;
        for _ in 0..120 {
            if f <= tol {
                return Some((u, v, t));
            }
            let cols = [su, sv, -d];
            let mut jtj = [[0.0; 3]; 3];
            let mut g = [0.0; 3];
            for (i, ci) in cols.iter().enumerate() {
                for (j, cj) in cols.iter().enumerate() {
                    if let Some(x) = jtj.get_mut(i).and_then(|row| row.get_mut(j)) {
                        *x = ci.dot(*cj);
                    }
                }
                put(&mut g, i, ci.dot(r));
            }
            let diag_max = jtj[0][0].max(jtj[1][1]).max(jtj[2][2]);
            let mu = 1e-12 * diag_max + 1e-300;
            let mut accepted = false;
            for _ in 0..20 {
                let mut m = jtj;
                for k in 0..3 {
                    if let Some(x) = m.get_mut(k).and_then(|row| row.get_mut(k)) {
                        *x += lambda * (*x + mu);
                    }
                }
                let Some([du, dv, dt]) = solve3(m, [-g[0], -g[1], -g[2]]) else {
                    lambda *= 4.0;
                    continue;
                };
                let (nu, nv) = self.clamp_search(u + du, v + dv);
                let nt = t + dt;
                if let Some((nr, nsu, nsv)) = resid(nu, nv, nt) {
                    let nf = nr.length();
                    if nf < f {
                        (u, v, t, r, su, sv, f) = (nu, nv, nt, nr, nsu, nsv, nf);
                        lambda = (lambda * 0.2).max(1e-12);
                        accepted = true;
                        break;
                    }
                }
                lambda *= 4.0;
            }
            if !accepted {
                break;
            }
        }
        (f <= tol).then_some((u, v, t))
    }
}

impl ParametricFace for ExactFace {
    fn uv_bounds(&self) -> (Uv, Uv) {
        (
            Uv {
                u: self.bounds[0],
                v: self.bounds[2],
            },
            Uv {
                u: self.bounds[1],
                v: self.bounds[3],
            },
        )
    }

    fn point_normal(&self, uv: Uv) -> Option<SurfacePoint> {
        if !uv.u.is_finite() || !uv.v.is_finite() {
            return None;
        }
        let (u, v) = self.wrapped(uv);
        if !self.in_box(u, v) {
            return None;
        }
        let sp = self.surface_point(u, v, Vec3::ZERO)?;
        // Without a normal there is no answer, not a made-up one.
        self.normal(u, v)?;
        Some(sp)
    }

    fn closest(&self, point: Point3) -> Option<FaceClosest> {
        if !point.is_finite() {
            return None;
        }
        let mut cands = match &self.geometry {
            Geometry::Quad(a) => self.quad_candidates(a, point),
            Geometry::Nurbs(n, g) => self.nurbs_candidates(n, g, point),
        };
        cands.retain(|c| c.0.is_finite());
        cands.sort_by(|a, b| a.0.total_cmp(&b.0));
        let contains = |c: &&(f64, f64, f64)| self.contains(Uv { u: c.1, v: c.2 });
        let best = match cands.first() {
            Some(first) if contains(&first) => Some(*first),
            _ => {
                let inside = cands.iter().find(contains).copied();
                let edge = self.boundary_closest(point);
                match (inside, edge) {
                    (Some(a), Some(b)) => Some(if a.0 <= b.0 { a } else { b }),
                    (a, b) => a.or(b),
                }
            }
        }?;
        let (d, u, v) = best;
        let eval = self.eval(u, v)?;
        let sp = self.surface_point(u, v, point - eval)?;
        Some(FaceClosest {
            uv: Uv { u, v },
            distance: d,
            point: sp,
        })
    }

    fn ray_hits(&self, origin: Point3, direction: UnitVec3, max_distance: f64) -> Vec<RayHit> {
        let d = direction.get();
        if !origin.is_finite() || max_distance.is_nan() || max_distance < 0.0 {
            return Vec::new();
        }
        let mut out = Vec::new();
        match &self.geometry {
            Geometry::Quad(a) => {
                let mut ts = a.ray_ts(origin, d);
                ts.retain(|t| t.is_finite() && *t >= 0.0 && *t <= max_distance);
                ts.sort_by(f64::total_cmp);
                for t in ts {
                    let p = origin + d * t;
                    let (u, v) = a.uv_of(p);
                    let (u, v) = self.wrapped(Uv { u, v });
                    if self.contains(Uv { u, v })
                        && let Some(h) = self.make_hit(t, u, v, p, d)
                    {
                        out.push(h);
                    }
                }
            }
            Geometry::Nurbs(n, g) => {
                let mut hits = self.nurbs_ray(n, g, origin, d, max_distance);
                hits.sort_by(|a, b| a.0.total_cmp(&b.0));
                for (t, u, v, p) in hits {
                    if self.contains(Uv { u, v })
                        && let Some(h) = self.make_hit(t, u, v, p, d)
                    {
                        out.push(h);
                    }
                }
            }
        }
        out
    }

    fn contains(&self, uv: Uv) -> bool {
        if !uv.u.is_finite() || !uv.v.is_finite() {
            return false;
        }
        let (u, v) = self.wrapped(uv);
        let us = match self.period_u {
            Some(p) => vec![u, u - p, u + p],
            None => vec![u],
        };
        let vs = match self.period_v {
            Some(p) => vec![v, v - p, v + p],
            None => vec![v],
        };
        us.iter()
            .any(|&x| vs.iter().any(|&y| self.inside_one(x, y)))
    }
}

// ---------------------------------------------------------------------------
// The source.

/// The faces of a B-rep model as a [`SurfaceSource`] (IMP-02).
///
/// [`SurfaceSource::project`] is the nearest point over the faces and
/// [`SurfaceSource::cast_ray`] the first trimmed hit; answers carry the
/// face's number (`BrepFace::id`) and (u, v) in a [`FacePosition`]. Faces are
/// visited nearest bounding box first with ties by face order, so answers are
/// deterministic. The tessellation is the faces' tessellations joined, and
/// tool drops fall back to it (OI-81 stays open).
#[derive(Debug)]
pub struct BrepSurface {
    faces: Vec<ExactFace>,
    mesh: TriangleMesh,
    face_of_triangle: Vec<FaceId>,
    fallback: Option<MeshSurface>,
    bounds: Aabb,
}

impl BrepSurface {
    /// All faces of `model`. Fails when there are none or one is invalid.
    pub fn new(model: &BrepModel) -> Result<Self> {
        let faces = model
            .faces
            .iter()
            .map(ExactFace::new)
            .collect::<Result<Vec<_>>>()?;
        let (mesh, face_of_triangle) = model.joined_tessellation();
        Self::assemble(faces, mesh, face_of_triangle)
    }

    /// Only the faces named in `ids` (the user's face selection). Fails when
    /// an ID is not in the model or the selection is empty.
    pub fn with_faces(model: &BrepModel, ids: &[FaceId]) -> Result<Self> {
        for id in ids {
            if !model.faces.iter().any(|f| f.id == *id) {
                return Err(PathError::Surface(format!(
                    "face {} is not in the model",
                    id.0
                )));
            }
        }
        let chosen: Vec<&BrepFace> = model.faces.iter().filter(|f| ids.contains(&f.id)).collect();
        let faces = chosen
            .iter()
            .map(|f| ExactFace::new(f))
            .collect::<Result<Vec<_>>>()?;
        let mut mesh = TriangleMesh::default();
        let mut face_of_triangle = Vec::new();
        for f in chosen {
            let base = u32::try_from(mesh.vertices.len()).unwrap_or(u32::MAX);
            mesh.vertices.extend_from_slice(&f.tessellation.vertices);
            for t in &f.tessellation.triangles {
                mesh.triangles.push(t.map(|i| i.saturating_add(base)));
                face_of_triangle.push(f.id);
            }
        }
        Self::assemble(faces, mesh, face_of_triangle)
    }

    fn assemble(
        faces: Vec<ExactFace>,
        mesh: TriangleMesh,
        face_of_triangle: Vec<FaceId>,
    ) -> Result<Self> {
        let mut it = faces.iter().map(ExactFace::bounding_box);
        let first = it
            .next()
            .ok_or_else(|| PathError::Surface("the model has no faces".to_owned()))?;
        let bounds = it.fold(first, |a, b| Aabb {
            min: Point3::new(
                a.min.x.min(b.min.x),
                a.min.y.min(b.min.y),
                a.min.z.min(b.min.z),
            ),
            max: Point3::new(
                a.max.x.max(b.max.x),
                a.max.y.max(b.max.y),
                a.max.z.max(b.max.z),
            ),
        });
        let fallback = if mesh.triangles.is_empty() {
            None
        } else {
            Some(MeshSurface::new(&mesh)?)
        };
        Ok(Self {
            faces,
            mesh,
            face_of_triangle,
            fallback,
            bounds,
        })
    }

    /// The faces, in model order.
    pub fn faces(&self) -> &[ExactFace] {
        &self.faces
    }

    /// The face of each triangle of [`SurfaceSource::tessellation`].
    pub fn face_of_triangle(&self) -> &[FaceId] {
        &self.face_of_triangle
    }
}

fn box_distance(b: &Aabb, p: Point3) -> f64 {
    let dx = (b.min.x - p.x).max(p.x - b.max.x).max(0.0);
    let dy = (b.min.y - p.y).max(p.y - b.max.y).max(0.0);
    let dz = (b.min.z - p.z).max(p.z - b.max.z).max(0.0);
    (dx * dx + dy * dy + dz * dz).sqrt()
}

/// Entry distance of the ray into the box, `None` on a miss.
fn box_entry(b: &Aabb, o: Point3, d: Vec3, max: f64) -> Option<f64> {
    let (mut t0, mut t1) = (0.0_f64, max);
    for (lo, hi, oo, dd) in [
        (b.min.x, b.max.x, o.x, d.x),
        (b.min.y, b.max.y, o.y, d.y),
        (b.min.z, b.max.z, o.z, d.z),
    ] {
        if dd.abs() < 1e-300 {
            if oo < lo || oo > hi {
                return None;
            }
        } else {
            let (a, c) = ((lo - oo) / dd, (hi - oo) / dd);
            t0 = t0.max(a.min(c));
            t1 = t1.min(a.max(c));
        }
    }
    (t0 <= t1).then_some(t0)
}

impl SurfaceSource for BrepSurface {
    fn bounds(&self) -> Aabb {
        self.bounds
    }

    fn project(&self, point: Point3) -> Option<SurfacePoint> {
        if !point.is_finite() {
            return None;
        }
        let mut order: Vec<(f64, usize)> = self
            .faces
            .iter()
            .enumerate()
            .map(|(i, f)| (box_distance(&f.bounding_box(), point), i))
            .collect();
        order.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let mut best: Option<FaceClosest> = None;
        for (lower, i) in order {
            if best.as_ref().is_some_and(|b| lower > b.distance) {
                break;
            }
            if let Some(c) = self.faces.get(i).and_then(|f| f.closest(point))
                && best.as_ref().is_none_or(|b| c.distance < b.distance)
            {
                best = Some(c);
            }
        }
        best.map(|c| c.point)
    }

    fn cast_ray(&self, origin: Point3, direction: UnitVec3, max_distance: f64) -> Option<RayHit> {
        let d = direction.get();
        let mut order: Vec<(f64, usize)> = self
            .faces
            .iter()
            .enumerate()
            .filter_map(|(i, f)| {
                box_entry(&f.bounding_box(), origin, d, max_distance).map(|t| (t, i))
            })
            .collect();
        order.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let mut best: Option<RayHit> = None;
        for (entry, i) in order {
            if best.as_ref().is_some_and(|b| entry > b.distance) {
                break;
            }
            let hit = self.faces.get(i).and_then(|f| {
                f.ray_hits(origin, direction, max_distance)
                    .into_iter()
                    .next()
            });
            if let Some(h) = hit
                && best.as_ref().is_none_or(|b| h.distance < b.distance)
            {
                best = Some(h);
            }
        }
        best
    }

    fn tessellation(&self) -> &TriangleMesh {
        &self.mesh
    }

    fn drop_tool(&self, profile: &RevolvedProfile, line: &ToolLine) -> Option<Contact> {
        // No exact solver yet (OI-81): drop onto the tessellation.
        self.fallback.as_ref()?.drop_tool(profile, line)
    }
}
