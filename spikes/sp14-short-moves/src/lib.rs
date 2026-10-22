// SPDX-License-Identifier: AGPL-3.0-or-later
//! SP-14: block-time measurement of posted G-code, and a joint-space merge
//! with a part-frame tolerance. Throwaway spike code.

use openchuck_gcode::{Dialect, Motion, Step, read, replay};

/// Which feed rule times a G1 block (decision 0007).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Rule {
    Makera,
    Linuxcnc,
}

/// Axis rates for timing rapids: the Z1 firmware maxima (decision 0013),
/// mm/min for X, Y, Z and deg/min for A.
pub const RAPID: [f64; 4] = [1200.0, 1200.0, 600.0, 3600.0];

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
pub enum Class {
    Rapid,
    /// G1 with no change on any axis (as written).
    Zero,
    AOnly,
    Xyz,
    Mixed,
}

#[derive(Clone, Debug)]
pub struct Blk {
    pub line: usize,
    pub class: Class,
    pub from: [f64; 4],
    pub to: [f64; 4],
    /// Seconds.
    pub t: f64,
    pub feed: f64,
}

pub fn amount(rule: Rule, from: [f64; 4], to: [f64; 4]) -> f64 {
    let l =
        ((to[0] - from[0]).powi(2) + (to[1] - from[1]).powi(2) + (to[2] - from[2]).powi(2)).sqrt();
    let da = (to[3] - from[3]).abs();
    match rule {
        Rule::Linuxcnc => {
            if l > 1e-9 {
                l
            } else {
                da
            }
        }
        Rule::Makera => {
            let r = (to[1].powi(2) + to[2].powi(2)).sqrt();
            let p = 2.0 * std::f64::consts::PI * r.max(1.0) + 30.0;
            if l > 1e-9 {
                l.max(da * p / 360.0)
            } else {
                da * p.min(360.0) / 360.0
            }
        }
    }
}

pub fn classify(from: [f64; 4], to: [f64; 4]) -> Class {
    let xyz = (0..3).any(|i| (to[i] - from[i]).abs() > 1e-9);
    let a = (to[3] - from[3]).abs() > 1e-9;
    match (xyz, a) {
        (false, false) => Class::Zero,
        (false, true) => Class::AOnly,
        (true, false) => Class::Xyz,
        (true, true) => Class::Mixed,
    }
}

pub fn blocks(text: &str, rule: Rule) -> Vec<Blk> {
    let dialect = match rule {
        Rule::Makera => Dialect::MakeraZ1,
        Rule::Linuxcnc => Dialect::Linuxcnc,
    };
    let program = read(text).expect("readable G-code");
    let rep = replay(&program, dialect).expect("replayable G-code");
    let mut out = Vec::new();
    for s in &rep.steps {
        let Step::Move(m) = s else { continue };
        if m.machine {
            continue;
        }
        if m.motion == Motion::Rapid {
            let t = (0..4)
                .map(|i| (m.to[i] - m.from[i]).abs() / RAPID[i] * 60.0)
                .fold(0.0, f64::max);
            out.push(Blk {
                line: m.line,
                class: Class::Rapid,
                from: m.from,
                to: m.to,
                t,
                feed: 0.0,
            });
            continue;
        }
        let f = m.feed.unwrap_or(0.0);
        let class = classify(m.from, m.to);
        let t = if f > 0.0 {
            amount(rule, m.from, m.to) * 60.0 / f
        } else {
            0.0
        };
        out.push(Blk {
            line: m.line,
            class,
            from: m.from,
            to: m.to,
            t,
            feed: f,
        });
    }
    out
}

/// The tool tip in the part frame: undo the A rotation (right-handed about +X).
pub fn tip(p: [f64; 4]) -> [f64; 3] {
    let a = -p[3].to_radians();
    let (s, c) = a.sin_cos();
    [p[0], p[1] * c - p[2] * s, p[1] * s + p[2] * c]
}

fn lerp(a: [f64; 4], b: [f64; 4], t: f64) -> [f64; 4] {
    [0, 1, 2, 3].map(|i| a[i] + (b[i] - a[i]) * t)
}

fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// Points along a joint-space segment mapped to the part frame.
fn samples(a: [f64; 4], b: [f64; 4], n: usize) -> Vec<[f64; 3]> {
    (0..=n)
        .map(|k| tip(lerp(a, b, k as f64 / n as f64)))
        .collect()
}

/// Distance from a point to a sampled part-frame curve (polyline through samples).
fn to_curve(p: [f64; 3], c: &[[f64; 3]]) -> f64 {
    let mut best = f64::MAX;
    for w in c.windows(2) {
        let (a, b) = (w[0], w[1]);
        let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let ap = [p[0] - a[0], p[1] - a[1], p[2] - a[2]];
        let l2 = ab[0] * ab[0] + ab[1] * ab[1] + ab[2] * ab[2];
        let t = if l2 > 0.0 {
            ((ap[0] * ab[0] + ap[1] * ab[1] + ap[2] * ab[2]) / l2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        best = best.min(dist(
            p,
            [a[0] + ab[0] * t, a[1] + ab[1] * t, a[2] + ab[2] * t],
        ));
    }
    best
}

/// Two-way sampled deviation between the merged joint-space line and the
/// original blocks, in the part frame, mm.
pub fn deviation(run: &[Blk]) -> f64 {
    let (s, e) = (run[0].from, run[run.len() - 1].to);
    let merged = samples(s, e, 8 * run.len().max(4));
    let mut orig = Vec::new();
    for b in run {
        let pts = samples(b.from, b.to, 8);
        if orig.is_empty() {
            orig.extend(pts)
        } else {
            orig.extend(pts.into_iter().skip(1))
        }
    }
    let one = orig
        .iter()
        .map(|p| to_curve(*p, &merged))
        .fold(0.0, f64::max);
    let two = merged
        .iter()
        .map(|p| to_curve(*p, &orig))
        .fold(0.0, f64::max);
    one.max(two)
}

/// What a merge pass does.
#[derive(Clone, Copy, Debug)]
pub struct MergeOpts {
    /// Part-frame tolerance, mm. 0 drops zero-length blocks only.
    pub tol: f64,
    /// Only merge blocks shorter than this, seconds (0: merge any block).
    pub below: f64,
    /// Longest merged block, blocks.
    pub max_run: usize,
    /// Largest ratio of programmed feeds within one merged block.
    pub feed_ratio: f64,
}

/// Greedy merge of consecutive G1 blocks. The merged block keeps the summed
/// time (its F is set so it lasts as long as the blocks it replaces).
pub fn merge(blocks: &[Blk], o: MergeOpts) -> Vec<Blk> {
    let mut out: Vec<Blk> = Vec::new();
    let mut i = 0;
    while i < blocks.len() {
        let b = &blocks[i];
        if b.class == Class::Zero {
            // A zero-length block takes no time and has no motion; drop it.
            i += 1;
            continue;
        }
        if b.class == Class::Rapid || o.tol <= 0.0 || (o.below > 0.0 && b.t >= o.below) {
            out.push(b.clone());
            i += 1;
            continue;
        }
        let mut j = i + 1;
        let (mut fmin, mut fmax) = (b.feed, b.feed);
        while j < blocks.len() && j - i < o.max_run {
            let n = &blocks[j];
            if n.class == Class::Rapid
                || n.class == Class::Zero
                || (o.below > 0.0 && n.t >= o.below)
            {
                break;
            }
            let (lo, hi) = (fmin.min(n.feed), fmax.max(n.feed));
            if hi / lo > o.feed_ratio || deviation(&blocks[i..=j]) > o.tol {
                break;
            }
            fmin = lo;
            fmax = hi;
            j += 1;
        }
        let run = &blocks[i..j];
        let t: f64 = run.iter().map(|r| r.t).sum();
        let (from, to) = (run[0].from, run[run.len() - 1].to);
        out.push(Blk {
            line: run[0].line,
            class: classify(from, to),
            from,
            to,
            t,
            feed: 0.0,
        });
        i = j;
    }
    out
}

/// Part-frame length of a block (tip path), mm, from 8 samples.
pub fn path_len(b: &Blk) -> f64 {
    let p = samples(b.from, b.to, 8);
    p.windows(2).map(|w| dist(w[0], w[1])).sum()
}

pub const LIMITS_MS: [f64; 4] = [1.0, 2.0, 5.0, 10.0];

#[derive(Debug, Default, Clone)]
pub struct Stats {
    pub fed: usize,
    pub rapids: usize,
    pub zero: usize,
    /// Fed blocks under each limit.
    pub under: [usize; 4],
    /// Longest run of consecutive fed blocks under each limit.
    pub run: [usize; 4],
    /// Effective feed in that longest run as a fraction of programmed,
    /// if each block took at least the limit.
    pub run_eff: [f64; 4],
    /// Seconds added to the job if each fed block took at least the limit.
    pub added_s: [f64; 4],
    pub fed_s: f64,
    pub p01_ms: f64,
    pub p05_ms: f64,
    pub p50_ms: f64,
    pub min_ms: f64,
    /// Shortest average block time over any 32 consecutive fed blocks, ms
    /// (the Z1 planner queue depth).
    pub win32_ms: f64,
    /// Lowest speed cap over all 32-block windows from stopping within the
    /// queued length, v = sqrt(2 a L), as a fraction of the window's
    /// programmed speed (a = 150 mm/s^2, the Z1 config), and that speed.
    pub qcap_frac: f64,
    pub qcap_mm_min: f64,
}

pub fn stats(blocks: &[Blk]) -> Stats {
    let mut s = Stats::default();
    let mut times: Vec<f64> = Vec::new();
    for b in blocks {
        match b.class {
            Class::Rapid => s.rapids += 1,
            Class::Zero => s.zero += 1,
            _ => {
                s.fed += 1;
                times.push(b.t * 1000.0);
                s.fed_s += b.t;
            }
        }
    }
    for (k, lim) in LIMITS_MS.iter().enumerate() {
        let lim_s = lim / 1000.0;
        let mut cur: Vec<f64> = Vec::new();
        let mut flush = |cur: &mut Vec<f64>, s: &mut Stats| {
            if cur.len() > s.run[k] {
                s.run[k] = cur.len();
                let sum: f64 = cur.iter().sum();
                s.run_eff[k] = sum / (cur.len() as f64 * lim_s);
            }
            cur.clear();
        };
        for b in blocks {
            if b.class == Class::Rapid {
                flush(&mut cur, &mut s);
                continue;
            }
            if b.class == Class::Zero {
                // A zero-length block still takes a planner slot; it keeps a run going.
                continue;
            }
            if b.t < lim_s {
                s.under[k] += 1;
                s.added_s[k] += lim_s - b.t;
                cur.push(b.t);
            } else {
                flush(&mut cur, &mut s);
            }
        }
        flush(&mut cur, &mut s);
    }
    let fed: Vec<&Blk> = blocks
        .iter()
        .filter(|b| !matches!(b.class, Class::Rapid | Class::Zero))
        .collect();
    s.win32_ms = f64::NAN;
    s.qcap_frac = f64::NAN;
    if fed.len() >= 32 {
        let mut best = f64::MAX;
        let mut cap = f64::MAX;
        let mut cap_v = 0.0;
        for w in fed.windows(32) {
            let t: f64 = w.iter().map(|b| b.t).sum();
            best = best.min(t / 32.0);
            let l: f64 = w.iter().map(|b| path_len(b)).sum();
            if t > 0.0 && l > 0.0 {
                let v = l / t; // programmed mm/s along the part
                let vmax = (2.0 * 150.0 * l).sqrt();
                if vmax / v < cap {
                    cap = vmax / v;
                    cap_v = vmax * 60.0;
                }
            }
        }
        s.win32_ms = best * 1000.0;
        s.qcap_frac = cap;
        s.qcap_mm_min = cap_v;
    }
    times.sort_by(|a, b| a.total_cmp(b));
    let pct = |p: f64| {
        if times.is_empty() {
            f64::NAN
        } else {
            times[((times.len() - 1) as f64 * p) as usize]
        }
    };
    s.p01_ms = pct(0.01);
    s.p05_ms = pct(0.05);
    s.p50_ms = pct(0.5);
    s.min_ms = pct(0.0);
    s
}

pub fn header() -> String {
    "fed | zero | <1ms | <2ms | <5ms | <10ms | run<1 | run<2 | run<5 (eff) | run<10 (eff) | min ms | p1 ms | p5 ms | p50 ms | fed s | +s@5ms | +s@10ms | win32 ms | qcap (mm/min)".into()
}

pub fn row(s: &Stats) -> String {
    format!(
        "{} | {} | {} | {} | {} | {} | {} | {} | {} ({:.0}%) | {} ({:.0}%) | {:.2} | {:.2} | {:.2} | {:.1} | {:.0} | {:.1} | {:.1} | {:.2} | {:.2} ({:.0})",
        s.fed,
        s.zero,
        s.under[0],
        s.under[1],
        s.under[2],
        s.under[3],
        s.run[0],
        s.run[1],
        s.run[2],
        s.run_eff[2] * 100.0,
        s.run[3],
        s.run_eff[3] * 100.0,
        s.min_ms,
        s.p01_ms,
        s.p05_ms,
        s.p50_ms,
        s.fed_s,
        s.added_s[2],
        s.added_s[3],
        s.win32_ms,
        s.qcap_frac,
        s.qcap_mm_min
    )
}
