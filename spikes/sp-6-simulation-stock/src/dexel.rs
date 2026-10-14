// SPDX-License-Identifier: AGPL-3.0-or-later
//! Candidate 2, dexels: material intervals along part-frame Z in a grid of
//! (x, y) columns. The part turns, so the tool's axis tilts in this frame; a
//! cut is the exact tool span along each column's line (any tool, any tilt),
//! subtracted from that column's intervals. Holds undercuts and islands.


use crate::par;
use crate::profile::Profile;
use crate::stock::{FNV_START, Pose, StockModel, fnv, prepare};
use crate::toolray::{cut_spans, subtract};

#[derive(Debug, Clone)]
pub struct DexelStock {
    rs: f64,
    x0: f64,
    y0: f64,
    c: f64,
    nx: usize,
    ny: usize,
    cols: Vec<Vec<[f64; 2]>>,
}

impl DexelStock {
    pub fn new(rs: f64, x0: f64, len: f64, cell: f64) -> Self {
        let nx = (len / cell).round().max(1.0) as usize;
        let c = len / nx as f64;
        let ny = ((2.0 * rs / c).ceil() as usize).max(1);
        let y0 = -0.5 * ny as f64 * c;
        let mut cols = Vec::with_capacity(nx * ny);
        for _ in 0..nx {
            for j in 0..ny {
                let y = y0 + (j as f64 + 0.5) * c;
                cols.push(if y.abs() < rs {
                    let z = (rs * rs - y * y).sqrt();
                    vec![[-z, z]]
                } else {
                    Vec::new()
                });
            }
        }
        Self {
            rs,
            x0,
            y0,
            c,
            nx,
            ny,
            cols,
        }
    }
}

impl StockModel for DexelStock {
    fn name(&self) -> &'static str {
        "dexel"
    }
    fn cell(&self) -> f64 {
        self.c
    }

    fn apply(&mut self, tool: &Profile, poses: &[Pose], threads: usize) {
        let pcs = prepare(tool, poses, self.rs);
        let r = tool.r_max();
        let this = &*self;
        let rows = par::map_items(this.nx, threads, |i| {
            let x = this.x0 + (i as f64 + 0.5) * this.c;
            let mut row: Vec<Vec<[f64; 2]>> = this.cols[i * this.ny..(i + 1) * this.ny].to_vec();
            for pc in &pcs {
                let dxo = x - pc.tx;
                if dxo.abs() > r {
                    continue;
                }
                let j0 = (((pc.y_lo - this.y0) / this.c).floor().max(0.0)) as usize;
                let j1 = ((((pc.y_hi - this.y0) / this.c).ceil()) as usize).min(this.ny - 1);
                for j in j0..=j1 {
                    if row[j].is_empty() {
                        continue;
                    }
                    let y = this.y0 + (j as f64 + 0.5) * this.c;
                    let o = [dxo, y * pc.ca - pc.ty, y * pc.sa - pc.tz];
                    let d = [0.0, -pc.sa, pc.ca];
                    let mut next: Option<Vec<[f64; 2]>> = None;
                    for (n, iv) in row[j].iter().enumerate() {
                        let s0 = iv[0].max(pc.z_lo);
                        let s1 = iv[1].min(pc.z_hi);
                        if s0 >= s1 {
                            if let Some(v) = next.as_mut() {
                                v.push(*iv);
                            }
                            continue;
                        }
                        let spans = cut_spans(tool, o, d, s0, s1);
                        if spans.n == 0 {
                            if let Some(v) = next.as_mut() {
                                v.push(*iv);
                            }
                            continue;
                        }
                        let v = next.get_or_insert_with(|| row[j][..n].to_vec());
                        subtract(iv[0], iv[1], spans.as_slice(), v);
                    }
                    if let Some(v) = next {
                        row[j] = v;
                    }
                }
            }
            row
        });
        self.cols = rows.into_iter().flatten().collect();
    }

    fn volume(&self) -> f64 {
        self.cols
            .iter()
            .flat_map(|c| c.iter().map(|iv| iv[1] - iv[0]))
            .sum::<f64>()
            * self.c
            * self.c
    }

    fn memory_bytes(&self) -> usize {
        self.cols
            .iter()
            .map(|c| c.capacity() * 16 + std::mem::size_of::<Vec<[f64; 2]>>())
            .sum::<usize>()
            + std::mem::size_of::<Self>()
    }

    fn contains(&self, p: [f64; 3]) -> bool {
        let fi = (p[0] - self.x0) / self.c;
        let fj = (p[1] - self.y0) / self.c;
        if fi < 0.0 || fj < 0.0 || fi >= self.nx as f64 || fj >= self.ny as f64 {
            return false;
        }
        self.cols[fi as usize * self.ny + fj as usize]
            .iter()
            .any(|iv| p[2] >= iv[0] && p[2] <= iv[1])
    }

    fn region_volume(&self, lo: [f64; 3], hi: [f64; 3]) -> f64 {
        let mut v = 0.0;
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
                for iv in &self.cols[i * self.ny + j] {
                    v += (iv[1].min(hi[2]) - iv[0].max(lo[2])).max(0.0);
                }
            }
        }
        v * self.c * self.c
    }

    fn state_hash(&self) -> u64 {
        let mut h = FNV_START;
        for c in &self.cols {
            fnv(&mut h, &(c.len() as u64).to_le_bytes());
            for iv in c {
                fnv(&mut h, &iv[0].to_bits().to_le_bytes());
                fnv(&mut h, &iv[1].to_bits().to_le_bytes());
            }
        }
        h
    }

    fn boxed_clone(&self) -> Box<dyn StockModel> {
        Box::new(self.clone())
    }
}

