// SPDX-License-Identifier: AGPL-3.0-or-later
//! Regions that go all the way round the cylinder. The development is periodic in s with
//! period P = 2 pi R, and neither library knows about periods, so the spike tiles the region
//! three times (s - P, s, s + P), offsets the tiled region, and judges the middle period
//! against the exact periodic distance.

use crate::check::*;
use crate::dev::Cyl;
use crate::geom::*;
use crate::ops::*;

#[derive(Clone, Copy, Debug)]
pub enum Island {
    Disc(f64),
    Square(f64),
    Star(f64),
    Slot(f64, f64),
}

fn island_loop(kind: Island, x: f64, s: f64) -> Loop {
    match kind {
        Island::Disc(r) => circle(x, s, r),
        Island::Square(h) => rect(x - h, s - h, x + h, s + h),
        Island::Star(r) => star(x, s, 5, r, 0.4 * r, 0.2),
        Island::Slot(l, w) => slot(x - l / 2.0, x + l / 2.0, s, w),
    }
}

/// Build the three-tile region: a band 0 <= x <= length, every s, with islands at (x, s) and
/// their copies one period either way.
pub fn tiled_band(cyl: Cyl, length: f64, islands: &[(Island, f64, f64)]) -> Region {
    let p = cyl.circumference();
    // wide enough that no tiled island touches the artificial ends
    let outer = rect(0.0, -2.5 * p, length, 2.5 * p);
    let mut holes = Vec::new();
    for &(kind, x, s) in islands {
        for k in -1..=1 {
            holes.push(island_loop(kind, x, s + k as f64 * p));
        }
    }
    Region::new(outer, holes)
}

#[derive(Debug, Default, Clone)]
pub struct SeamReport {
    pub samples: usize,
    pub mismatches: usize,
    pub max_dev: f64,
    pub loops: usize,
    pub panicked: bool,
}

/// Offset the tiled band by `d` with the engine and judge the window |s| <= P/2 (plus a
/// little beyond, to catch loops that straddle the seam) against the periodic oracle.
pub fn check_seam(cyl: Cyl, length: f64, islands: &[(Island, f64, f64)], d: f64, io: bool, seed: u64) -> SeamReport {
    let p = cyl.circumference();
    let base = tiled_band(cyl, length, islands);
    assert_eq!(validate_region(&base, 2e-5), Ok(()), "the tiled test input must itself be valid");
    let loops: Vec<Loop> = base.loops().cloned().collect();
    let res = std::panic::catch_unwind(|| if io { io_offset(&loops, d, IO_ARC_TOL, IO_JOIN_TOL) } else { cav_offset(&loops, d) });
    let Ok(res) = res else { return SeamReport { panicked: true, ..Default::default() } };
    let mut rep = SeamReport { loops: res.len(), ..Default::default() };
    let limit = if io { DEV_LIMIT_FLAT } else { DEV_LIMIT };
    let margin = 2.0 * limit;
    let base_mem = region_membership(&base, ARC_TOL);
    let res_mem = Membership::new(res.iter(), ARC_TOL);
    let in_window = |s: f64| s.abs() <= 0.5 * p + 2.0;
    // result boundary points inside the window lie on the exact offset curve
    for l in &res {
        let f = flatten(l, 1e-4);
        for q in f {
            if in_window(q[1]) {
                let dd = dist_region(q, &base);
                let sd = if base_mem.contains(q) { dd } else { -dd };
                rep.max_dev = rep.max_dev.max((sd - d).abs());
            }
        }
    }
    let mut rng = Rng(seed);
    for _ in 0..4000 {
        let q = [rng.range(-1.0, length + 1.0), rng.range(-0.5 * p - 2.0, 0.5 * p + 2.0)];
        let dd = dist_region(q, &base);
        let sd = if base_mem.contains(q) { dd } else { -dd };
        if (sd - d).abs() < margin {
            continue;
        }
        rep.samples += 1;
        if (sd >= d) != res_mem.contains(q) {
            rep.mismatches += 1;
        }
    }
    rep
}
