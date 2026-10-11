// SPDX-License-Identifier: AGPL-3.0-or-later
//! Stand-ins for the SP-6 stock candidates, swept by the same profile.
//! SP-6 has not run, so these are the two simplest forms of its dexel and
//! rotary-grid candidates: enough to show the profile drives a sweep, and to
//! time the profile's share of the work. Moves are sampled into poses; each
//! cell takes every pose in order, so rows can run on any thread.

use crate::mesh::V3;
use crate::par;
use crate::profile::Profile;

/// Samples a straight move into poses no further apart than `step`.
pub fn sample_line(p0: V3, p1: V3, step: f64) -> Vec<V3> {
    let len = ((p1[0] - p0[0]).powi(2) + (p1[1] - p0[1]).powi(2) + (p1[2] - p0[2]).powi(2)).sqrt();
    let n = ((len / step).ceil() as usize).max(1);
    (0..=n)
        .map(|k| {
            let t = k as f64 / n as f64;
            [
                p0[0] + t * (p1[0] - p0[0]),
                p0[1] + t * (p1[1] - p0[1]),
                p0[2] + t * (p1[2] - p0[2]),
            ]
        })
        .collect()
}

/// Dexel (Z-map) stock: the top of material in each XY column, tool axis +Z.
#[derive(Debug, Clone, PartialEq)]
pub struct Heightfield {
    pub x0: f64,
    pub y0: f64,
    pub cell: f64,
    pub nx: usize,
    pub ny: usize,
    pub top: Vec<f64>,
}

impl Heightfield {
    pub fn block(x0: f64, y0: f64, cell: f64, nx: usize, ny: usize, z: f64) -> Self {
        Self {
            x0,
            y0,
            cell,
            nx,
            ny,
            top: vec![z; nx * ny],
        }
    }

    pub fn swept(&self, profile: &Profile, poses: &[V3], threads: usize) -> Heightfield {
        let r = profile.r_max();
        let rows = par::map_items(self.ny, threads, |j| {
            let y = self.y0 + j as f64 * self.cell;
            let mut row = self.top[j * self.nx..(j + 1) * self.nx].to_vec();
            for p in poses {
                if (y - p[1]).abs() > r {
                    continue;
                }
                let i0 = (((p[0] - r - self.x0) / self.cell).floor().max(0.0)) as usize;
                let i1 = ((((p[0] + r - self.x0) / self.cell).ceil()) as usize).min(self.nx - 1);
                for (i, top) in row.iter_mut().enumerate().take(i1 + 1).skip(i0) {
                    let x = self.x0 + i as f64 * self.cell;
                    if let Some(h) = profile.height_at((x - p[0]).hypot(y - p[1])) {
                        *top = top.min(p[2] + h);
                    }
                }
            }
            row
        });
        Heightfield {
            top: rows.concat(),
            ..self.clone()
        }
    }
}

/// A pose for the rotary grid: tool tip in the machine frame and the A angle (radians).
#[derive(Debug, Clone, Copy)]
pub struct RotaryPose {
    pub tip: V3,
    pub a: f64,
}

/// Rotary grid stock: material radius along each ray from the A axis, by
/// (x, part angle). Tool axis +Z in the machine frame; the part turns by A
/// (right-handed about +X), so a part ray at angle t points at t - A.
#[derive(Debug, Clone, PartialEq)]
pub struct RadialGrid {
    pub x0: f64,
    pub dx: f64,
    pub nx: usize,
    pub nt: usize,
    pub rho: Vec<f64>,
}

impl RadialGrid {
    pub fn cylinder(x0: f64, dx: f64, nx: usize, nt: usize, r: f64) -> Self {
        Self {
            x0,
            dx,
            nx,
            nt,
            rho: vec![r; nx * nt],
        }
    }

    pub fn theta(&self, j: usize) -> f64 {
        std::f64::consts::TAU * j as f64 / self.nt as f64
    }

    pub fn swept(&self, profile: &Profile, poses: &[RotaryPose], threads: usize) -> RadialGrid {
        let r = profile.r_max();
        let rows = par::map_items(self.nx, threads, |i| {
            let x = self.x0 + i as f64 * self.dx;
            let mut row = self.rho[i * self.nt..(i + 1) * self.nt].to_vec();
            for p in poses {
                let dx = x - p.tip[0];
                if dx.abs() > r {
                    continue;
                }
                for (j, rho) in row.iter_mut().enumerate() {
                    let phi = self.theta(j) - p.a;
                    if let Some(s) = cut_along_ray(profile, dx, phi, p.tip[1], p.tip[2], *rho) {
                        *rho = rho.min(s);
                    }
                }
            }
            row
        });
        RadialGrid {
            rho: rows.concat(),
            ..self.clone()
        }
    }
}

/// Smallest distance along the ray (0, sin phi, cos phi) from the A axis at
/// which the ray enters the tool (axial offset `dx` from the tip, tip at
/// (ty, tz) in the machine YZ plane), searched up to `limit`.
fn cut_along_ray(
    profile: &Profile,
    dx: f64,
    phi: f64,
    ty: f64,
    tz: f64,
    limit: f64,
) -> Option<f64> {
    let r = profile.r_max();
    let w2 = r * r - dx * dx;
    if w2 < 0.0 {
        return None;
    }
    let w = w2.sqrt();
    let (sp, cp) = phi.sin_cos();
    if cp <= 0.0 {
        return None;
    }
    let (mut lo, mut hi) = if sp.abs() < 1e-15 {
        if ty.abs() > w {
            return None;
        }
        (0.0, limit)
    } else {
        let a = (ty - w) / sp;
        let b = (ty + w) / sp;
        (a.min(b).max(0.0), a.max(b).min(limit))
    };
    // The tool never reaches below its tip (h >= 0), so the ray must first
    // rise to the tip height.
    lo = lo.max(tz / cp);
    if lo > hi {
        return None;
    }
    let g = |s: f64| {
        let rr = dx.hypot(s * sp - ty).min(r);
        s * cp - tz - profile.height_at(rr).unwrap_or(f64::MAX)
    };
    if g(lo) >= 0.0 {
        return Some(lo);
    }
    let n = 64;
    let step = (hi - lo) / n as f64;
    let mut prev = lo;
    let mut found = None;
    for k in 1..=n {
        let s = lo + step * k as f64;
        if g(s) >= 0.0 {
            found = Some(s);
            break;
        }
        prev = s;
    }
    hi = found?;
    lo = prev;
    for _ in 0..80 {
        let mid = 0.5 * (lo + hi);
        if mid <= lo || mid >= hi {
            break;
        }
        if g(mid) >= 0.0 { hi = mid } else { lo = mid }
    }
    Some(hi)
}
