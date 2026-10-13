// SPDX-License-Identifier: AGPL-3.0-or-later
//! Parametric surfaces `S(u, v)` behind one trait. The detector and the placer see only
//! `point`, so they work the same on analytic faces and on spline faces (SP-9 shape).
//! The part frame has the A axis along X.

use crate::vec3::{V3, v3};

pub trait Surface {
    fn name(&self) -> String;
    /// `(u0, u1, v0, v1)`.
    fn domain(&self) -> (f64, f64, f64, f64);
    fn point(&self, u: f64, v: f64) -> V3;
}

/// Unit normal `S_u x S_v` by central differences (the geometric side; callers pick the air
/// side with a sign). `None` at a degenerate point such as a cone apex.
pub fn normal(s: &dyn Surface, u: f64, v: f64) -> Option<V3> {
    let (u0, u1, v0, v1) = s.domain();
    let hu = 1e-6 * (u1 - u0);
    let hv = 1e-6 * (v1 - v0);
    let su = (s.point(u + hu, v) - s.point(u - hu, v)) * (0.5 / hu);
    let sv = (s.point(u, v + hv) - s.point(u, v - hv)) * (0.5 / hv);
    su.cross(sv).unit()
}

/// Cylinder with its axis along Y through `(x0, 0, 0)`, so every ruling is radial and
/// perpendicular to the A axis. Geometric normal points inward.
pub struct RadialCylinder {
    pub x0: f64,
    pub r: f64,
    pub u: (f64, f64),
    pub v: (f64, f64),
}
impl Surface for RadialCylinder {
    fn name(&self) -> String {
        format!("radial cylinder r={}", self.r)
    }
    fn domain(&self) -> (f64, f64, f64, f64) {
        (self.u.0, self.u.1, self.v.0, self.v.1)
    }
    fn point(&self, u: f64, v: f64) -> V3 {
        v3(self.x0 + self.r * u.cos(), v, self.r * u.sin())
    }
}

/// Cone with its axis along Y and apex `(x0, 0, 0)`, half angle `beta`; `v` is the distance
/// along the axis. Geometric normal points inward.
pub struct RadialCone {
    pub x0: f64,
    pub beta: f64,
    pub u: (f64, f64),
    pub v: (f64, f64),
}
impl Surface for RadialCone {
    fn name(&self) -> String {
        format!("radial cone beta={:.1} deg", self.beta.to_degrees())
    }
    fn domain(&self) -> (f64, f64, f64, f64) {
        (self.u.0, self.u.1, self.v.0, self.v.1)
    }
    fn point(&self, u: f64, v: f64) -> V3 {
        let (sb, cb) = self.beta.sin_cos();
        let rho = v * sb / cb;
        v3(self.x0 + rho * u.cos(), v, rho * u.sin())
    }
}

/// Helicoid `(p u, v cos u, v sin u)`: ruled (radial lines) but not developable.
pub struct Helicoid {
    pub pitch: f64,
    pub u: (f64, f64),
    pub v: (f64, f64),
}
impl Surface for Helicoid {
    fn name(&self) -> String {
        format!("helicoid p={} mm/rad", self.pitch)
    }
    fn domain(&self) -> (f64, f64, f64, f64) {
        (self.u.0, self.u.1, self.v.0, self.v.1)
    }
    fn point(&self, u: f64, v: f64) -> V3 {
        v3(self.pitch * u, v * u.cos(), v * u.sin())
    }
}

/// Cylinder coaxial with the A axis; rulings run along X.
pub struct CoaxialCylinder {
    pub r: f64,
    pub u: (f64, f64),
    pub v: (f64, f64),
}
impl Surface for CoaxialCylinder {
    fn name(&self) -> String {
        format!("coaxial cylinder r={}", self.r)
    }
    fn domain(&self) -> (f64, f64, f64, f64) {
        (self.u.0, self.u.1, self.v.0, self.v.1)
    }
    fn point(&self, u: f64, v: f64) -> V3 {
        v3(v, self.r * u.cos(), self.r * u.sin())
    }
}

pub struct Plane;
impl Surface for Plane {
    fn name(&self) -> String {
        "plane".into()
    }
    fn domain(&self) -> (f64, f64, f64, f64) {
        (0.0, 1.0, 0.0, 1.0)
    }
    fn point(&self, u: f64, v: f64) -> V3 {
        v3(10.0 * u, 20.0 + 15.0 * v, 5.0 + 3.0 * u)
    }
}

pub struct Sphere {
    pub r: f64,
}
impl Surface for Sphere {
    fn name(&self) -> String {
        format!("sphere r={}", self.r)
    }
    fn domain(&self) -> (f64, f64, f64, f64) {
        (0.3, 2.8, 0.3, 1.3)
    }
    fn point(&self, u: f64, v: f64) -> V3 {
        v3(
            self.r * v.sin() * u.cos(),
            self.r * v.cos() + 25.0,
            self.r * v.sin() * u.sin(),
        )
    }
}

pub struct Torus {
    pub major: f64,
    pub minor: f64,
}
impl Surface for Torus {
    fn name(&self) -> String {
        format!("torus R={} r={}", self.major, self.minor)
    }
    fn domain(&self) -> (f64, f64, f64, f64) {
        (0.2, 2.0, 0.2, 2.0)
    }
    fn point(&self, u: f64, v: f64) -> V3 {
        let ring = self.major + self.minor * v.cos();
        v3(ring * u.cos(), 25.0 + self.minor * v.sin(), ring * u.sin())
    }
}

/// Bicubic Bezier patch. `u` runs along the first index, `v` along the second.
pub struct Bezier {
    pub label: String,
    pub net: [[V3; 4]; 4],
}
fn bern(t: f64) -> [f64; 4] {
    let s = 1.0 - t;
    [s * s * s, 3.0 * t * s * s, 3.0 * t * t * s, t * t * t]
}
impl Surface for Bezier {
    fn name(&self) -> String {
        self.label.clone()
    }
    fn domain(&self) -> (f64, f64, f64, f64) {
        (0.0, 1.0, 0.0, 1.0)
    }
    fn point(&self, u: f64, v: f64) -> V3 {
        let (bu, bv) = (bern(u), bern(v));
        let mut p = V3::default();
        for i in 0..4 {
            for j in 0..4 {
                p = p + self.net[i][j] * (bu[i] * bv[j]);
            }
        }
        p
    }
}

/// A wall lofted between two curves along Y (rulings in the YZ plane, so cuttable with a
/// vertical tool after rotation), with the interior `v` control points pushed in Z by
/// `bump`. A bump of 0 makes it exactly ruled (a developable extrusion plus a small
/// twist); a bump makes the `v` lines bend by `0.75 * bump` at the middle.
pub fn near_ruled(bump: f64) -> Bezier {
    let zs = [0.0, 3.0, 5.0, 4.0];
    let mut net = [[V3::default(); 4]; 4];
    for i in 0..4 {
        let a = v3(4.0 * i as f64, 20.0, zs[i]);
        let b = v3(4.0 * i as f64, 35.0, zs[i] * 1.1);
        for j in 0..4 {
            let t = j as f64 / 3.0;
            let mut p = a + (b - a) * t;
            if j == 1 || j == 2 {
                p.z += bump;
            }
            net[i][j] = p;
        }
    }
    Bezier {
        label: format!("near-ruled spline bump={bump}"),
        net,
    }
}
