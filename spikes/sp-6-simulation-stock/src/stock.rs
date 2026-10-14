// SPDX-License-Identifier: AGPL-3.0-or-later
//! The `StockModel` interface the three candidates sit behind, in the part
//! frame: X along the A axis, the stock a cylinder of radius `rs` on it. The
//! part turns about +X by the pose's A (right-handed); the tool is vertical
//! (+Z) at `tip` in the machine frame. Moves are sampled into poses and the
//! stock is cut at each pose. A swept volume between two poses is not built,
//! so poses must be closer than the resolution (stated in RESULT.md).

use crate::profile::Profile;

/// One tool pose: tip in the machine frame and the part angle A (radians).
#[derive(Debug, Clone, Copy)]
pub struct Pose {
    pub tip: [f64; 3],
    pub a: f64,
}

/// A pose with its trigonometry and the tool's bounding box in the part frame.
#[derive(Debug, Clone, Copy)]
pub struct PoseC {
    pub tx: f64,
    pub ty: f64,
    pub tz: f64,
    pub sa: f64,
    pub ca: f64,
    pub x_lo: f64,
    pub x_hi: f64,
    pub y_lo: f64,
    pub y_hi: f64,
    pub z_lo: f64,
    pub z_hi: f64,
}

/// `rs` is the stock radius: the tool box is clipped to the stock's cylinder
/// so poses are not culled by flute length the stock can never reach.
pub fn prepare(tool: &Profile, poses: &[Pose], rs: f64) -> Vec<PoseC> {
    let r = tool.r_max();
    let ze = tool.z_end();
    poses
        .iter()
        .map(|p| {
            let (sa, ca) = p.a.sin_cos();
            let (mut ylo, mut yhi, mut zlo, mut zhi) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
            for ym in [(p.tip[1] - r).max(-rs), (p.tip[1] + r).min(rs)] {
                for zm in [p.tip[2].max(-rs), (p.tip[2] + ze).min(rs)] {
                    let y = ym * ca + zm * sa;
                    let z = -ym * sa + zm * ca;
                    ylo = ylo.min(y);
                    yhi = yhi.max(y);
                    zlo = zlo.min(z);
                    zhi = zhi.max(z);
                }
            }
            PoseC {
                tx: p.tip[0],
                ty: p.tip[1],
                tz: p.tip[2],
                sa,
                ca,
                x_lo: p.tip[0] - r,
                x_hi: p.tip[0] + r,
                y_lo: ylo,
                y_hi: yhi,
                z_lo: zlo,
                z_hi: zhi,
            }
        })
        .collect()
}

pub trait StockModel: Send + Sync {
    fn name(&self) -> &'static str;
    /// Nominal resolution in mm.
    fn cell(&self) -> f64;
    /// Cuts the stock with `tool` at every pose. Results do not depend on `threads`.
    fn apply(&mut self, tool: &Profile, poses: &[Pose], threads: usize);
    /// Material volume in mm^3.
    fn volume(&self) -> f64;
    /// Bytes held by the stock.
    fn memory_bytes(&self) -> usize;
    /// Is the part-frame point inside the material?
    fn contains(&self, p: [f64; 3]) -> bool;
    /// Material volume inside the part-frame box `lo..hi`.
    fn region_volume(&self, lo: [f64; 3], hi: [f64; 3]) -> f64;
    /// FNV-1a hash of the stored state, for the thread-count checks.
    fn state_hash(&self) -> u64;
    fn boxed_clone(&self) -> Box<dyn StockModel>;

    /// Distance along the part-frame ray `origin + t dir` (unit `dir`) to the
    /// first material, up to `tmax`. Marches at a quarter cell and bisects,
    /// so the answer is exact to the stored geometry.
    fn ray_first_hit(&self, origin: [f64; 3], dir: [f64; 3], tmax: f64) -> Option<f64> {
        let at = |t: f64| {
            [
                origin[0] + t * dir[0],
                origin[1] + t * dir[1],
                origin[2] + t * dir[2],
            ]
        };
        let step = self.cell() / 4.0;
        let n = (tmax / step).ceil() as usize;
        let mut prev = 0.0;
        if self.contains(at(0.0)) {
            return Some(0.0);
        }
        for k in 1..=n {
            let t = (k as f64 * step).min(tmax);
            if self.contains(at(t)) {
                let (mut out_t, mut in_t) = (prev, t);
                for _ in 0..50 {
                    let m = 0.5 * (out_t + in_t);
                    if self.contains(at(m)) {
                        in_t = m;
                    } else {
                        out_t = m;
                    }
                }
                return Some(in_t);
            }
            prev = t;
        }
        None
    }
}

pub fn fnv(h: &mut u64, bytes: &[u8]) {
    for b in bytes {
        *h ^= *b as u64;
        *h = h.wrapping_mul(0x100_0000_01b3);
    }
}

pub const FNV_START: u64 = 0xcbf2_9ce4_8422_2325;
