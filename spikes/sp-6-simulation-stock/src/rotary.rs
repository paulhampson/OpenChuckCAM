// SPDX-License-Identifier: AGPL-3.0-or-later
//! Candidate 1, the rotary grid: for each axial cell `x` and part angle
//! `theta`, the material radius along the ray from the A axis. A cut finds
//! where the ray enters the tool and shortens the radius to it. Stock is
//! star-shaped about the axis by construction (a cavity cut from outside
//! always removes the outer material above it, so nothing is lost for
//! turning-style and wrap work; a feature that leaves material outside a cut
//! on the same ray, such as a floating island, cannot be held).

use std::f64::consts::{PI, TAU};

use crate::par;
use crate::profile::Profile;
use crate::stock::{FNV_START, Pose, PoseC, StockModel, fnv, prepare};
use crate::toolray::cut_spans;

#[derive(Debug, Clone)]
pub struct RotaryGrid {
    rs: f64,
    x0: f64,
    dx: f64,
    nx: usize,
    nt: usize,
    dt: f64,
    rho: Vec<f64>,
}

impl RotaryGrid {
    /// A cylinder of radius `rs` from `x0` for `len`, axial cell `cell` and
    /// an angular cell whose arc at the stock radius is no longer than `cell`.
    pub fn new(rs: f64, x0: f64, len: f64, cell: f64) -> Self {
        let nx = (len / cell).round().max(1.0) as usize;
        let nt = ((TAU * rs / cell).ceil() as usize).max(8);
        Self {
            rs,
            x0,
            dx: len / nx as f64,
            nx,
            nt,
            dt: TAU / nt as f64,
            rho: vec![rs; nx * nt],
        }
    }

    fn x_of(&self, i: usize) -> f64 {
        self.x0 + (i as f64 + 0.5) * self.dx
    }
}

/// Angular window (radians, machine frame) of rays from the origin that can
/// meet the rectangle `y in [y0, y1], z in [z0, z1]`; `None` means every ray.
fn window(y0: f64, y1: f64, z0: f64, z1: f64) -> Option<(f64, f64)> {
    if y0 <= 0.0 && 0.0 <= y1 && z0 <= 0.0 && 0.0 <= z1 {
        return None;
    }
    let c = (0.5 * (z0 + z1)).atan2(0.5 * (y0 + y1));
    let (mut dmin, mut dmax) = (f64::MAX, f64::MIN);
    for (y, z) in [(y0, z0), (y0, z1), (y1, z0), (y1, z1)] {
        let mut d = z.atan2(y) - c;
        if d > PI {
            d -= TAU;
        }
        if d < -PI {
            d += TAU;
        }
        dmin = dmin.min(d);
        dmax = dmax.max(d);
    }
    Some((c + dmin - 1e-9, c + dmax + 1e-9))
}

impl StockModel for RotaryGrid {
    fn name(&self) -> &'static str {
        "rotary grid"
    }
    fn cell(&self) -> f64 {
        self.dx
    }

    fn apply(&mut self, tool: &Profile, poses: &[Pose], threads: usize) {
        let pcs: Vec<PoseC> = prepare(tool, poses, self.rs);
        let r = tool.r_max();
        let ze = tool.z_end();
        let this = &*self;
        let rows = par::map_items(this.nx, threads, |i| {
            let x = this.x_of(i);
            let mut row = this.rho[i * this.nt..(i + 1) * this.nt].to_vec();
            for pc in &pcs {
                let dxo = x - pc.tx;
                if dxo.abs() > r {
                    continue;
                }
                let w2 = (r * r - dxo * dxo).sqrt();
                let a = pc.sa.atan2(pc.ca);
                let nt = this.nt as i64;
                let (j_lo, j_hi) = match window(pc.ty - w2, pc.ty + w2, pc.tz, (pc.tz + ze).min(this.rs)) {
                    Some((p0, p1)) => {
                        let j0 = ((p0 - a) / this.dt - 0.5).ceil() as i64;
                        let j1 = ((p1 - a) / this.dt - 0.5).floor() as i64;
                        if j1 - j0 + 1 >= nt { (0, nt - 1) } else { (j0, j1) }
                    }
                    None => (0, nt - 1),
                };
                for jj in j_lo..=j_hi {
                    let j = jj.rem_euclid(this.nt as i64) as usize;
                    let rho = row[j];
                    if rho <= 0.0 {
                        continue;
                    }
                    let psi = (j as f64 + 0.5) * this.dt + a;
                    let (sp, cp) = psi.sin_cos();
                    if rho * sp < pc.tz {
                        continue;
                    }
                    let spans = cut_spans(tool, [dxo, -pc.ty, -pc.tz], [0.0, cp, sp], 0.0, rho);
                    if spans.n > 0 {
                        row[j] = spans.v[0].0.min(rho);
                    }
                }
            }
            row
        });
        self.rho = rows.concat();
    }

    fn volume(&self) -> f64 {
        self.rho.iter().map(|r| 0.5 * r * r).sum::<f64>() * self.dt * self.dx
    }

    fn memory_bytes(&self) -> usize {
        self.rho.capacity() * 8 + std::mem::size_of::<Self>()
    }

    fn contains(&self, p: [f64; 3]) -> bool {
        let fi = (p[0] - self.x0) / self.dx;
        if fi < 0.0 || fi >= self.nx as f64 {
            return false;
        }
        let i = fi as usize;
        let r = p[1].hypot(p[2]);
        let th = p[2].atan2(p[1]).rem_euclid(TAU);
        let j = ((th / self.dt) as usize).min(self.nt - 1);
        let rho = self.rho[i * self.nt + j];
        rho > 0.0 && r <= rho
    }

    fn region_volume(&self, lo: [f64; 3], hi: [f64; 3]) -> f64 {
        let full = lo[1] <= -self.rs && hi[1] >= self.rs && lo[2] <= -self.rs && hi[2] >= self.rs;
        let mut v = 0.0;
        for i in 0..self.nx {
            let x = self.x_of(i);
            if x < lo[0] || x >= hi[0] {
                continue;
            }
            for j in 0..self.nt {
                let rho = self.rho[i * self.nt + j];
                if full {
                    v += 0.5 * rho * rho * self.dt * self.dx;
                    continue;
                }
                let (st, ct) = ((j as f64 + 0.5) * self.dt).sin_cos();
                let n = ((rho / (self.dx * 0.5)).ceil() as usize).max(1);
                let ds = rho / n as f64;
                for k in 0..n {
                    let s = (k as f64 + 0.5) * ds;
                    let (y, z) = (s * ct, s * st);
                    if y >= lo[1] && y <= hi[1] && z >= lo[2] && z <= hi[2] {
                        v += s * ds * self.dt * self.dx;
                    }
                }
            }
        }
        v
    }

    fn state_hash(&self) -> u64 {
        let mut h = FNV_START;
        for r in &self.rho {
            fnv(&mut h, &r.to_bits().to_le_bytes());
        }
        h
    }

    fn boxed_clone(&self) -> Box<dyn StockModel> {
        Box::new(self.clone())
    }
}
