// SPDX-License-Identifier: AGPL-3.0-or-later
//! Analytic reference part: a star-shaped body about the A axis,
//! `r(x, th) = a0 + a1 sin(k1 x) + a2 cos(m th)`, with exact point and normal.
//! `th` is the angle from +Z toward +Y, the same as the approach angle `psi`.

use crate::Vector;

#[derive(Clone, Copy, Debug)]
pub struct Part {
    pub length: f64,
    pub a0: f64,
    pub a1: f64,
    pub k1: f64,
    pub a2: f64,
    pub m: f64,
}

impl Part {
    /// Axisymmetric vase: radius 14 mm swinging 3 mm along 80 mm.
    pub fn vase() -> Self {
        Part { length: 80.0, a0: 14.0, a1: 3.0, k1: std::f64::consts::TAU / 40.0, a2: 0.0, m: 3.0 }
    }
    /// The same vase with three 1.5 mm lobes, so the surface normal leaves the radial plane.
    pub fn lobed() -> Self {
        Part { a2: 1.5, ..Part::vase() }
    }
    pub fn r(&self, x: f64, th: f64) -> f64 {
        self.a0 + self.a1 * (self.k1 * x).sin() + self.a2 * (self.m * th).cos()
    }
    pub fn r_x(&self, x: f64) -> f64 {
        self.a1 * self.k1 * (self.k1 * x).cos()
    }
    pub fn r_th(&self, th: f64) -> f64 {
        -self.a2 * self.m * (self.m * th).sin()
    }
    pub fn point(&self, x: f64, th: f64) -> Vector {
        let r = self.r(x, th);
        Vector::new(x, r * th.sin(), r * th.cos())
    }
    /// Outward unit normal.
    pub fn normal(&self, x: f64, th: f64) -> Vector {
        let (s, c) = th.sin_cos();
        let r = self.r(x, th);
        let (rx, rt) = (self.r_x(x), self.r_th(th));
        let (a, b) = (rx * s, rx * c);
        let (p, q) = (rt * s + r * c, rt * c - r * s);
        Vector::new(a * q - b * p, -q, p).normalize()
    }
    pub fn r_min(&self) -> f64 {
        self.a0 - self.a1.abs() - self.a2.abs()
    }
    pub fn r_max(&self) -> f64 {
        self.a0 + self.a1.abs() + self.a2.abs()
    }
}
