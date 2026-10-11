// SPDX-License-Identifier: AGPL-3.0-or-later
//! Analytic test parts on the A axis (X): cylinder, cone and sphere. Each gives
//! its exact top surface for the gouge checker and a tessellation whose
//! vertices lie on the surface (so the mesh is inscribed: facets sit inside the
//! part by at most the sagitta).

use std::f64::consts::PI;

use crate::mesh::{Mesh, V3};

#[derive(Debug, Clone, Copy)]
pub enum Part {
    /// Radius `r`, from `x0` to `x1` along X.
    Cylinder { r: f64, x0: f64, x1: f64 },
    /// Radius `r0 + x * tan(half_angle)`, from `x0` to `x1`.
    Cone {
        r0: f64,
        half_angle_deg: f64,
        x0: f64,
        x1: f64,
    },
    /// Radius `r`, centred on the origin.
    Sphere { r: f64 },
}

impl Part {
    /// Largest radius about the X axis (cylinder, cone) or the origin (sphere).
    pub fn max_radius(&self) -> f64 {
        match *self {
            Part::Cylinder { r, .. } | Part::Sphere { r } => r,
            Part::Cone {
                r0,
                half_angle_deg,
                x0,
                x1,
            } => {
                let t = half_angle_deg.to_radians().tan();
                (r0 + x0 * t).max(r0 + x1 * t)
            }
        }
    }

    /// Exact top surface height at (x, y), or `None` off the part.
    pub fn top(&self, x: f64, y: f64) -> Option<f64> {
        let rad = match *self {
            Part::Cylinder { r, x0, x1 } => ((x0..=x1).contains(&x)).then_some(r)?,
            Part::Cone {
                r0,
                half_angle_deg,
                x0,
                x1,
            } => ((x0..=x1).contains(&x)).then_some(r0 + x * half_angle_deg.to_radians().tan())?,
            Part::Sphere { r } => {
                let q = r * r - x * x;
                if q < 0.0 {
                    return None;
                }
                q.sqrt()
            }
        };
        let q = rad * rad - y * y;
        (q >= 0.0).then(|| q.sqrt())
    }

    /// Exact outward unit normal at a point on the surface.
    pub fn normal(&self, p: V3) -> V3 {
        match *self {
            Part::Cylinder { .. } => {
                let l = p[1].hypot(p[2]);
                [0.0, p[1] / l, p[2] / l]
            }
            Part::Cone { half_angle_deg, .. } => {
                let b = half_angle_deg.to_radians();
                let l = p[1].hypot(p[2]);
                // Radial component cos(b), axial component -sin(b).
                [-b.sin(), b.cos() * p[1] / l, b.cos() * p[2] / l]
            }
            Part::Sphere { .. } => {
                let l = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
                [p[0] / l, p[1] / l, p[2] / l]
            }
        }
    }

    /// Tessellates with `n` divisions around and a step of about `dx` along X.
    /// Sagitta is `r * (1 - cos(pi / n))` for the largest radius.
    pub fn mesh(&self, n: usize, dx: f64) -> Mesh {
        let mut m = Mesh::default();
        match *self {
            Part::Cylinder { x0, x1, .. } | Part::Cone { x0, x1, .. } => {
                let tan = match *self {
                    Part::Cone { half_angle_deg, .. } => half_angle_deg.to_radians().tan(),
                    _ => 0.0,
                };
                let base = match *self {
                    Part::Cylinder { r, .. } => r,
                    Part::Cone { r0, .. } => r0,
                    Part::Sphere { .. } => unreachable!(),
                };
                let nx = (((x1 - x0) / dx).ceil() as usize).max(1);
                for i in 0..=nx {
                    let x = x0 + (x1 - x0) * i as f64 / nx as f64;
                    let rad = base + x * tan;
                    for j in 0..n {
                        let t = 2.0 * PI * j as f64 / n as f64;
                        m.verts.push([x, rad * t.sin(), rad * t.cos()]);
                    }
                }
                for i in 0..nx {
                    for j in 0..n {
                        let a = (i * n + j) as u32;
                        let b = (i * n + (j + 1) % n) as u32;
                        let c = a + n as u32;
                        let d = b + n as u32;
                        m.tris.push([a, b, d]);
                        m.tris.push([a, d, c]);
                    }
                }
            }
            Part::Sphere { r } => {
                // Latitude rings from pole to pole, n around, n/2 from pole to pole.
                let nl = n / 2;
                for i in 0..=nl {
                    let lat = PI * i as f64 / nl as f64;
                    let (s, c) = lat.sin_cos();
                    for j in 0..n {
                        let t = 2.0 * PI * j as f64 / n as f64;
                        m.verts.push([r * c, r * s * t.sin(), r * s * t.cos()]);
                    }
                }
                for i in 0..nl {
                    for j in 0..n {
                        let a = (i * n + j) as u32;
                        let b = (i * n + (j + 1) % n) as u32;
                        let c = a + n as u32;
                        let d = b + n as u32;
                        m.tris.push([a, b, d]);
                        m.tris.push([a, d, c]);
                    }
                }
            }
        }
        // Drop degenerate triangles at the sphere's poles.
        let verts = &m.verts;
        m.tris.retain(|t| {
            let [a, b, c] = t.map(|k| verts[k as usize]);
            let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
            let n = [
                u[1] * v[2] - u[2] * v[1],
                u[2] * v[0] - u[0] * v[2],
                u[0] * v[1] - u[1] * v[0],
            ];
            (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt() > 1e-15
        });
        m
    }
}

/// Divisions around a circle of radius `r` for a sagitta of at most `chord`.
pub fn divisions_for(r: f64, chord: f64) -> usize {
    (PI / (1.0 - chord / r).acos()).ceil() as usize
}
