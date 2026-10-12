// SPDX-License-Identifier: AGPL-3.0-or-later
//! Checks against the independent oracle (exact distance to the original boundary, membership
//! by flattening) plus a hash for determinism.

use crate::corpus::Case;
use crate::geom::*;
use crate::ops::*;
use std::panic::{catch_unwind, AssertUnwindSafe};

/// Largest allowed distance between a result boundary point and the exact offset curve, mm.
/// One tenth of the 0.01 mm toolpath deviation budget (verification plan 4.1).
pub const DEV_LIMIT: f64 = 1e-3;
/// Limit for results built from flattened arcs (i_overlay): the 0.005 mm tessellation budget
/// of the verification plan, section 4.1.
pub const DEV_LIMIT_FLAT: f64 = 5e-3;
/// Points closer than this to an expected boundary are not used for membership checks.
pub const MARGIN: f64 = 2e-3;
/// Chord tolerance used for membership tests (the oracle's own flattening).
pub const ARC_TOL: f64 = 1e-4;
/// Chord tolerance used to flatten arcs before i_overlay sees them, and for its round joins.
/// 1e-4 was tried first and is far too slow (see the report).
pub const IO_ARC_TOL: f64 = 1e-3;
pub const IO_JOIN_TOL: f64 = 5e-4;

#[derive(Clone, Debug, Default)]
pub struct Verdict {
    pub loops: usize,
    pub arcs: usize,
    pub lines: usize,
    pub vertices: usize,
    pub arc_length_share: f64,
    pub max_dev: f64,
    pub mismatches: usize,
    pub samples: usize,
    pub nonfinite: bool,
    pub panicked: bool,
}

impl Verdict {
    pub fn ok(&self, limit: f64) -> bool {
        !self.panicked && !self.nonfinite && self.max_dev <= limit && self.mismatches == 0
    }
}

/// FNV-1a over the bit patterns of every coordinate and bulge, in order.
pub fn hash_loops(loops: &[Loop]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut mix = |x: u64| {
        for b in x.to_le_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x100_0000_01b3);
        }
    };
    mix(loops.len() as u64);
    for l in loops {
        mix(l.len() as u64);
        for v in l {
            for c in v {
                mix(c.to_bits());
            }
        }
    }
    h
}

fn seg_len(a: [f64; 3], b: [f64; 3]) -> f64 {
    let d = (b[0] - a[0]).hypot(b[1] - a[1]);
    if a[2] == 0.0 {
        d
    } else {
        let (_, r, _, sw) = arc_params([a[0], a[1]], [b[0], b[1]], a[2]);
        r * sw.abs()
    }
}

/// Sample points along a loop: flattened vertices and midpoints of every flattened segment.
pub fn boundary_samples(l: &Loop) -> Vec<Pt> {
    // 1e-5 chord error, so the oracle's own resolution is a tenth of the library tolerance
    let f = flatten(l, 1e-5);
    let n = f.len();
    let mut out = Vec::with_capacity(2 * n);
    for i in 0..n {
        let (a, b) = (f[i], f[(i + 1) % n]);
        out.push(a);
        out.push([0.5 * (a[0] + b[0]), 0.5 * (a[1] + b[1])]);
    }
    out
}

fn signed_dist(base: &Region, mem: &Membership, p: Pt) -> f64 {
    let d = dist_region(p, base);
    if mem.contains(p) { d } else { -d }
}

/// Check `result` as the inward offset by `d` of `base` (`d < 0` grows the region).
pub fn check_offset(base: &Region, result: &[Loop], d: f64, rng: &mut Rng, n_random: usize, limit: f64) -> Verdict {
    let margin = 2.0 * limit;
    let mut v = Verdict { loops: result.len(), ..Default::default() };
    let mut arc_len = 0.0;
    let mut tot_len = 0.0;
    for l in result {
        v.vertices += l.len();
        for i in 0..l.len() {
            let (a, b) = (l[i], l[(i + 1) % l.len()]);
            if !(a[0].is_finite() && a[1].is_finite() && a[2].is_finite()) {
                v.nonfinite = true;
            }
            let sl = seg_len(a, b);
            tot_len += sl;
            if a[2] == 0.0 {
                v.lines += 1;
            } else {
                v.arcs += 1;
                arc_len += sl;
            }
        }
    }
    v.arc_length_share = if tot_len > 0.0 { arc_len / tot_len } else { 0.0 };
    if v.nonfinite {
        return v;
    }
    let base_mem = region_membership(base, ARC_TOL);
    // Boundary points must lie on the exact offset curve.
    for l in result {
        for p in boundary_samples(l) {
            let dev = (signed_dist(base, &base_mem, p) - d).abs();
            v.max_dev = v.max_dev.max(dev);
        }
    }
    // Coverage: random points agree with the exact offset region away from its boundary.
    let res_mem = Membership::new(result.iter(), ARC_TOL);
    let b = base.bounds();
    let pad = d.abs() + 1.0;
    for _ in 0..n_random {
        let p = [rng.range(b[0] - pad, b[2] + pad), rng.range(b[1] - pad, b[3] + pad)];
        let sd = signed_dist(base, &base_mem, p);
        if (sd - d).abs() < margin {
            continue;
        }
        v.samples += 1;
        if (sd >= d) != res_mem.contains(p) {
            v.mismatches += 1;
        }
    }
    v
}

#[derive(Clone, Copy, Debug)]
pub enum Engine {
    CavDirect,
    CavSuccessive,
    IoDirect,
}

impl Engine {
    pub fn name(self) -> &'static str {
        match self {
            Engine::CavDirect => "cavalier direct",
            Engine::CavSuccessive => "cavalier successive",
            Engine::IoDirect => "i_overlay direct",
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct SweepReport {
    pub case: String,
    pub engine: &'static str,
    pub passes: usize,
    pub failed_passes: usize,
    pub first_failure: Option<(usize, Verdict)>,
    pub max_dev: f64,
    pub mismatches: usize,
    pub arcs: usize,
    pub lines: usize,
    pub vertices: usize,
    pub arc_share_first: f64,
    pub hash: u64,
    pub area_not_monotone: usize,
    pub micros: u128,
    /// Time inside the library calls only (not the oracle).
    pub lib_micros: u128,
}

pub const MAX_PASSES: usize = 400;

fn run_engine(engine: Engine, case: &Case, prev: &[Loop], k: usize, sign: f64) -> Vec<Loop> {
    let step = case.step * sign;
    match engine {
        Engine::CavDirect => cav_offset(&case.region_loops(), step * k as f64),
        Engine::CavSuccessive => cav_offset(prev, step),
        Engine::IoDirect => io_offset(&case.region_loops(), step * k as f64, IO_ARC_TOL, IO_JOIN_TOL),
    }
}

impl Case {
    pub fn region_loops(&self) -> Vec<Loop> {
        self.region.loops().cloned().collect()
    }
}

/// Offset the case in passes of `case.step` until nothing is left, checking every pass.
pub fn sweep(case: &Case, engine: Engine, seed: u64, n_random: usize) -> SweepReport {
    sweep_signed(case, engine, seed, n_random, 1.0, MAX_PASSES)
}

/// Outward passes (profile contouring around an island): `sign` = -1.0, a few passes.
pub fn sweep_outward(case: &Case, engine: Engine, seed: u64, n_random: usize) -> SweepReport {
    sweep_signed(case, engine, seed, n_random, -1.0, 10)
}

pub fn sweep_signed(case: &Case, engine: Engine, seed: u64, n_random: usize, sign: f64, max_passes: usize) -> SweepReport {
    let mut rep = SweepReport { case: case.name.clone(), engine: engine.name(), ..Default::default() };
    let limit = match engine { Engine::IoDirect => DEV_LIMIT_FLAT, _ => DEV_LIMIT };
    let mut rng = Rng(seed);
    let mut prev: Vec<Loop> = case.region_loops();
    let mut prev_area = case.region.area();
    let mut hashes = Vec::new();
    let t0 = std::time::Instant::now();
    for k in 1..=max_passes {
        let d = sign * case.step * k as f64;
        let tl = std::time::Instant::now();
        let res = catch_unwind(AssertUnwindSafe(|| run_engine(engine, case, &prev, k, sign)));
        rep.lib_micros += tl.elapsed().as_micros();
        let (loops, panicked) = match res {
            Ok(l) => (l, false),
            Err(_) => (Vec::new(), true),
        };
        let mut v = if panicked {
            Verdict { panicked: true, ..Default::default() }
        } else {
            check_offset(&case.region, &loops, d, &mut rng, n_random, limit)
        };
        v.panicked = panicked;
        rep.passes += 1;
        rep.max_dev = rep.max_dev.max(v.max_dev);
        rep.mismatches += v.mismatches;
        rep.arcs += v.arcs;
        rep.lines += v.lines;
        rep.vertices += v.vertices;
        if k == 1 {
            rep.arc_share_first = v.arc_length_share;
        }
        if !v.ok(limit) {
            rep.failed_passes += 1;
            if rep.first_failure.is_none() {
                rep.first_failure = Some((k, v.clone()));
            }
        }
        let area: f64 = loops.iter().map(signed_area).sum();
        if sign > 0.0 && !loops.is_empty() && area > prev_area + 1e-6 {
            rep.area_not_monotone += 1;
        }
        prev_area = area;
        hashes.push(hash_loops(&loops));
        if loops.is_empty() || panicked {
            break;
        }
        prev = loops;
    }
    rep.micros = t0.elapsed().as_micros();
    rep.hash = hashes.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, x| (h ^ x).wrapping_mul(0x100_0000_01b3));
    rep
}

// ---- booleans --------------------------------------------------------------------------

#[derive(Debug, Default, Clone)]
pub struct BoolReport {
    pub label: String,
    pub engine: &'static str,
    pub ops: usize,
    pub failed: usize,
    pub panicked: usize,
    pub mismatches: usize,
    pub samples: usize,
    pub first_failure: Option<String>,
    pub hash: u64,
}

fn expected(op: Op, a: bool, b: bool) -> bool {
    match op {
        Op::Union => a || b,
        Op::Intersect => a && b,
        Op::Difference => a && !b,
        Op::Xor => a != b,
    }
}

/// Compare a boolean result with the exact expectation at random points away from both inputs'
/// boundaries.
fn check_bool(a: &Region, b: &Region, op: Op, result: &[Loop], rng: &mut Rng, n: usize) -> (usize, usize) {
    let (ma, mb) = (region_membership(a, ARC_TOL), region_membership(b, ARC_TOL));
    let mr = Membership::new(result.iter(), ARC_TOL);
    let (ba, bb) = (a.bounds(), b.bounds());
    let lo = [ba[0].min(bb[0]) - 1.0, ba[1].min(bb[1]) - 1.0];
    let hi = [ba[2].max(bb[2]) + 1.0, ba[3].max(bb[3]) + 1.0];
    let (mut mism, mut samples) = (0, 0);
    for _ in 0..n {
        let p = [rng.range(lo[0], hi[0]), rng.range(lo[1], hi[1])];
        if dist_region(p, a) < MARGIN || dist_region(p, b) < MARGIN {
            continue;
        }
        samples += 1;
        if expected(op, ma.contains(p), mb.contains(p)) != mr.contains(p) {
            mism += 1;
        }
    }
    (mism, samples)
}

/// Booleans of `a` against `b` with i_overlay, all four operations.
pub fn bool_io(label: &str, a: &Region, b: &Region, seed: u64, n: usize) -> BoolReport {
    let mut rep = BoolReport { label: label.into(), engine: "i_overlay", ..Default::default() };
    let mut rng = Rng(seed);
    let mut hs = 0xcbf2_9ce4_8422_2325u64;
    for op in OPS {
        rep.ops += 1;
        match catch_unwind(AssertUnwindSafe(|| io_boolean(a, b, op, ARC_TOL))) {
            Err(_) => {
                rep.panicked += 1;
                rep.failed += 1;
                rep.first_failure.get_or_insert(format!("{op:?}: panic"));
            }
            Ok(r) => {
                let (m, s) = check_bool(a, b, op, &r, &mut rng, n);
                rep.mismatches += m;
                rep.samples += s;
                hs = (hs ^ hash_loops(&r)).wrapping_mul(0x100_0000_01b3);
                if m > 0 {
                    rep.failed += 1;
                    rep.first_failure.get_or_insert(format!("{op:?}: {m} of {s} points wrong"));
                }
            }
        }
    }
    rep.hash = hs;
    rep
}

/// Booleans of two single closed loops with cavalier_contours (arcs kept).
pub fn bool_cav(label: &str, a: &Loop, b: &Loop, seed: u64, n: usize) -> BoolReport {
    let mut rep = BoolReport { label: label.into(), engine: "cavalier", ..Default::default() };
    let mut rng = Rng(seed);
    let (ra, rb) = (Region::new(a.clone(), vec![]), Region::new(b.clone(), vec![]));
    let mut hs = 0xcbf2_9ce4_8422_2325u64;
    for op in OPS {
        rep.ops += 1;
        match catch_unwind(AssertUnwindSafe(|| cav_boolean(&ra.outer, &rb.outer, op))) {
            Err(_) => {
                rep.panicked += 1;
                rep.failed += 1;
                rep.first_failure.get_or_insert(format!("{op:?}: panic"));
            }
            Ok(r) => {
                let (m, s) = check_bool(&ra, &rb, op, &r, &mut rng, n);
                rep.mismatches += m;
                rep.samples += s;
                hs = (hs ^ hash_loops(&r)).wrapping_mul(0x100_0000_01b3);
                if m > 0 {
                    rep.failed += 1;
                    rep.first_failure.get_or_insert(format!("{op:?}: {m} of {s} points wrong"));
                }
            }
        }
    }
    rep.hash = hs;
    rep
}

// ---- input validation (NFR-07) -----------------------------------------------------------

/// Reasons a region is refused before any library sees it.
#[derive(Debug, Clone, PartialEq)]
pub enum Refusal {
    Empty,
    TooFewVertices,
    NotFinite,
    /// Two loops come closer than `gap` mm (touching or overlapping loops).
    LoopsTouch { gap: f64 },
    /// A loop crosses itself.
    SelfIntersecting,
    /// A hole is not inside the outer boundary.
    HoleOutside,
}

/// Minimum distance from the boundary of `a` to the boundary of `b`, sampled densely.
fn loop_gap(a: &Loop, b: &Loop) -> f64 {
    let f = flatten(a, 1e-3);
    let n = f.len();
    let mut best = f64::MAX;
    for i in 0..n {
        let (p, q) = (f[i], f[(i + 1) % n]);
        let len = (q[0] - p[0]).hypot(q[1] - p[1]);
        let k = ((len / 0.02).ceil() as usize).max(1);
        for j in 0..k {
            let t = j as f64 / k as f64;
            best = best.min(dist_loop([p[0] + t * (q[0] - p[0]), p[1] + t * (q[1] - p[1])], b));
        }
    }
    best
}

/// The checks the product would run before offsetting. `min_gap` is how close two loops may
/// come (cavalier_contours' position epsilon is 1e-5; use a small multiple of it).
pub fn validate_region(r: &Region, min_gap: f64) -> Result<(), Refusal> {
    if r.outer.is_empty() {
        return Err(Refusal::Empty);
    }
    for l in r.loops() {
        if l.len() < 2 {
            return Err(Refusal::TooFewVertices);
        }
        if l.iter().any(|v| !(v[0].is_finite() && v[1].is_finite() && v[2].is_finite())) {
            return Err(Refusal::NotFinite);
        }
        if !crate::corpus::is_simple(l) {
            return Err(Refusal::SelfIntersecting);
        }
    }
    let om = Membership::new([&r.outer], ARC_TOL);
    for h in &r.holes {
        let g = loop_gap(h, &r.outer);
        if g < min_gap {
            return Err(Refusal::LoopsTouch { gap: g });
        }
        let f = flatten(h, 1e-3);
        if !om.contains(f[0]) {
            return Err(Refusal::HoleOutside);
        }
    }
    for i in 0..r.holes.len() {
        for j in i + 1..r.holes.len() {
            let g = loop_gap(&r.holes[i], &r.holes[j]);
            if g < min_gap {
                return Err(Refusal::LoopsTouch { gap: g });
            }
        }
    }
    Ok(())
}

/// For tests of the checker itself.
pub fn check_bool_pub(a: &Region, b: &Region, op: Op, result: &[Loop], rng: &mut Rng) -> (usize, usize) {
    check_bool(a, b, op, result, rng, 3000)
}
