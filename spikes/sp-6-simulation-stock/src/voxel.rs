// SPDX-License-Identifier: AGPL-3.0-or-later
//! Candidate 3, voxels: one bit per cubic cell in the part frame, packed so
//! that each axial slab owns whole words (slabs can be cut on separate
//! threads). A cut takes the exact tool span along each (x, y) column, like
//! the dexels, and clears the cells whose centres fall inside it; this is the
//! fast form of a voxel cut (testing every cell in the tool's box is 10 to
//! 100 times slower and gives the same bits).

use crate::par;
use crate::profile::Profile;
use crate::stock::{FNV_START, Pose, StockModel, fnv, prepare};
use crate::toolray::cut_spans;

#[derive(Debug, Clone)]
pub struct VoxelStock {
    rs: f64,
    x0: f64,
    y0: f64,
    z0: f64,
    c: f64,
    nx: usize,
    ny: usize,
    nz: usize,
    sw: usize,
    bits: Vec<u64>,
}

fn clear_range(slab: &mut [u64], base: usize, k0: usize, k1: usize) {
    // Clears bits base+k0 ..= base+k1.
    let (a, b) = (base + k0, base + k1);
    let (wa, wb) = (a / 64, b / 64);
    for w in wa..=wb {
        let lo = if w == wa { a % 64 } else { 0 };
        let hi = if w == wb { b % 64 } else { 63 };
        let mask = if hi - lo == 63 {
            u64::MAX
        } else {
            ((1u64 << (hi - lo + 1)) - 1) << lo
        };
        slab[w] &= !mask;
    }
}

/// Lowest and highest set bit of the `nz` bits starting at `base`.
fn extent(slab: &[u64], base: usize, nz: usize) -> Option<(usize, usize)> {
    let mut lo = None;
    let mut hi = None;
    let (wa, wb) = (base / 64, (base + nz - 1) / 64);
    for w in wa..=wb {
        let mut v = slab[w];
        if w == wa {
            v &= u64::MAX << (base % 64);
        }
        if w == wb && (base + nz) % 64 != 0 {
            v &= (1u64 << ((base + nz) % 64)) - 1;
        }
        if v != 0 {
            if lo.is_none() {
                lo = Some(w * 64 + v.trailing_zeros() as usize - base);
            }
            hi = Some(w * 64 + 63 - v.leading_zeros() as usize - base);
        }
    }
    lo.zip(hi)
}

impl VoxelStock {
    pub fn new(rs: f64, x0: f64, len: f64, cell: f64) -> Self {
        let nx = (len / cell).round().max(1.0) as usize;
        let c = len / nx as f64;
        let ny = ((2.0 * rs / c).ceil() as usize).max(1);
        let nz = ny;
        let sw = (ny * nz).div_ceil(64);
        let y0 = -0.5 * ny as f64 * c;
        let z0 = y0;
        let mut bits = vec![0u64; nx * sw];
        for i in 0..nx {
            let slab = &mut bits[i * sw..(i + 1) * sw];
            for j in 0..ny {
                let y = y0 + (j as f64 + 0.5) * c;
                for k in 0..nz {
                    let z = z0 + (k as f64 + 0.5) * c;
                    if y * y + z * z <= rs * rs {
                        let b = j * nz + k;
                        slab[b / 64] |= 1 << (b % 64);
                    }
                }
            }
        }
        Self {
            rs,
            x0,
            y0,
            z0,
            c,
            nx,
            ny,
            nz,
            sw,
            bits,
        }
    }

    fn get(&self, i: usize, j: usize, k: usize) -> bool {
        let b = j * self.nz + k;
        self.bits[i * self.sw + b / 64] >> (b % 64) & 1 == 1
    }
}

impl StockModel for VoxelStock {
    fn name(&self) -> &'static str {
        "voxel"
    }
    fn cell(&self) -> f64 {
        self.c
    }

    fn apply(&mut self, tool: &Profile, poses: &[Pose], threads: usize) {
        let pcs = prepare(tool, poses, self.rs);
        let r = tool.r_max();
        let this = &*self;
        let slabs = par::map_items(this.nx, threads, |i| {
            let x = this.x0 + (i as f64 + 0.5) * this.c;
            let mut slab = this.bits[i * this.sw..(i + 1) * this.sw].to_vec();
            for pc in &pcs {
                let dxo = x - pc.tx;
                if dxo.abs() > r {
                    continue;
                }
                let j0 = (((pc.y_lo - this.y0) / this.c).floor().max(0.0)) as usize;
                let j1 = ((((pc.y_hi - this.y0) / this.c).ceil()) as usize).min(this.ny - 1);
                let s0 = pc.z_lo.max(this.z0);
                let s1 = pc.z_hi.min(this.z0 + this.nz as f64 * this.c);
                if s0 >= s1 {
                    continue;
                }
                for j in j0..=j1 {
                    let y = this.y0 + (j as f64 + 0.5) * this.c;
                    let Some((k_lo, k_hi)) = extent(&slab, j * this.nz, this.nz) else {
                        continue;
                    };
                    let (c0, c1) = (
                        s0.max(this.z0 + k_lo as f64 * this.c),
                        s1.min(this.z0 + (k_hi + 1) as f64 * this.c),
                    );
                    if c0 >= c1 {
                        continue;
                    }
                    let o = [dxo, y * pc.ca - pc.ty, y * pc.sa - pc.tz];
                    let d = [0.0, -pc.sa, pc.ca];
                    let spans = cut_spans(tool, o, d, c0, c1);
                    for &(a, b) in spans.as_slice() {
                        let ka = ((a - this.z0) / this.c - 0.5).ceil().max(0.0) as i64;
                        let kb = (((b - this.z0) / this.c - 0.5).floor() as i64)
                            .min(this.nz as i64 - 1);
                        if kb >= ka {
                            clear_range(&mut slab, j * this.nz, ka as usize, kb as usize);
                        }
                    }
                }
            }
            slab
        });
        self.bits = slabs.concat();
    }

    fn volume(&self) -> f64 {
        self.bits.iter().map(|w| w.count_ones() as f64).sum::<f64>() * self.c * self.c * self.c
    }

    fn memory_bytes(&self) -> usize {
        self.bits.capacity() * 8 + std::mem::size_of::<Self>()
    }

    fn contains(&self, p: [f64; 3]) -> bool {
        let fi = (p[0] - self.x0) / self.c;
        let fj = (p[1] - self.y0) / self.c;
        let fk = (p[2] - self.z0) / self.c;
        if fi < 0.0 || fj < 0.0 || fk < 0.0 {
            return false;
        }
        let (i, j, k) = (fi as usize, fj as usize, fk as usize);
        i < self.nx && j < self.ny && k < self.nz && self.get(i, j, k)
    }

    fn region_volume(&self, lo: [f64; 3], hi: [f64; 3]) -> f64 {
        let mut n = 0u64;
        for i in 0..self.nx {
            let x = self.x0 + (i as f64 + 0.5) * self.c;
            if x < lo[0] || x >= hi[0] {
                continue;
            }
            for j in 0..self.ny {
                let y = self.y0 + (j as f64 + 0.5) * self.c;
                if y < lo[1] || y >= hi[1] {
                    continue;
                }
                for k in 0..self.nz {
                    let z = self.z0 + (k as f64 + 0.5) * self.c;
                    if z >= lo[2] && z < hi[2] && self.get(i, j, k) {
                        n += 1;
                    }
                }
            }
        }
        n as f64 * self.c * self.c * self.c
    }

    fn state_hash(&self) -> u64 {
        let mut h = FNV_START;
        for w in &self.bits {
            fnv(&mut h, &w.to_le_bytes());
        }
        h
    }

    fn boxed_clone(&self) -> Box<dyn StockModel> {
        Box::new(self.clone())
    }
}
