// SPDX-License-Identifier: AGPL-3.0-or-later
//! Where a straight line is inside the tool: the one geometric query all three
//! stock candidates use. The tool solid is `{ r <= R, h(r) <= w <= z_end }`
//! in the tool frame (axis +Z, tip at the origin), `h` being the profile's
//! lower envelope. Flutes end at `z_end`; the shank above is not modelled
//! (cutting deeper than the flute is a collision question, COL-05, not a
//! removal one).

use crate::profile::Profile;

/// At most this many separate spans are kept per line (non-convex profiles).
pub const MAX_SPANS: usize = 8;

/// The spans of a line inside the tool, in increasing parameter order.
#[derive(Debug, Clone, Copy)]
pub struct Spans {
    pub n: usize,
    pub v: [(f64, f64); MAX_SPANS],
}

impl Spans {
    pub const EMPTY: Spans = Spans {
        n: 0,
        v: [(0.0, 0.0); MAX_SPANS],
    };
    pub fn as_slice(&self) -> &[(f64, f64)] {
        &self.v[..self.n]
    }
    fn push(&mut self, a: f64, b: f64) {
        if self.n < MAX_SPANS {
            self.v[self.n] = (a, b);
            self.n += 1;
        } else {
            // Conservative: merge into the last span rather than lose cutting.
            self.v[MAX_SPANS - 1].1 = b;
        }
    }
}

/// Inside-the-tool spans of the line `p(s) = o + s d` (tool frame, `d` a unit
/// vector), for `s` in `[s0, s1]`.
///
/// Convex profiles give one span, found exactly: `w - h(r(s))` is concave in
/// `s` (w is linear, r is convex in s, h is convex and non-decreasing), so a
/// ternary search finds its peak and bisection finds the two roots. Other
/// profiles are marched at R/8 and refined by bisection; a span shorter than
/// the march step can be missed there, which is the stated limit for
/// non-convex custom profiles.
pub fn cut_spans(profile: &Profile, o: [f64; 3], d: [f64; 3], s0: f64, s1: f64) -> Spans {
    let big_r = profile.r_max();
    let ze = profile.z_end();
    let (mut a, mut b) = (s0, s1);
    let qa = d[0] * d[0] + d[1] * d[1];
    let qb = 2.0 * (o[0] * d[0] + o[1] * d[1]);
    let qc = o[0] * o[0] + o[1] * o[1] - big_r * big_r;
    if qa < 1e-18 {
        if qc > 0.0 {
            return Spans::EMPTY;
        }
    } else {
        let disc = qb * qb - 4.0 * qa * qc;
        if disc < 0.0 {
            return Spans::EMPTY;
        }
        let sq = disc.sqrt();
        a = a.max((-qb - sq) / (2.0 * qa));
        b = b.min((-qb + sq) / (2.0 * qa));
    }
    if d[2].abs() < 1e-15 {
        if o[2] < 0.0 || o[2] > ze {
            return Spans::EMPTY;
        }
    } else {
        let t0 = -o[2] / d[2];
        let t1 = (ze - o[2]) / d[2];
        a = a.max(t0.min(t1));
        b = b.min(t0.max(t1));
    }
    if a > b {
        return Spans::EMPTY;
    }
    let f = |s: f64| -> f64 {
        let x = o[0] + s * d[0];
        let y = o[1] + s * d[1];
        let w = o[2] + s * d[2];
        let r = x.hypot(y).min(big_r);
        w - profile.height_at(r).unwrap_or(f64::INFINITY)
    };
    let mut out = Spans::EMPTY;
    if profile.is_convex() {
        // Golden-section search for the peak of the concave f.
        const G: f64 = 0.618_033_988_749_894_9;
        let (mut lo, mut hi) = (a, b);
        let mut m1 = hi - G * (hi - lo);
        let mut m2 = lo + G * (hi - lo);
        let (mut f1, mut f2) = (f(m1), f(m2));
        for _ in 0..42 {
            if f1 < f2 {
                lo = m1;
                m1 = m2;
                f1 = f2;
                m2 = lo + G * (hi - lo);
                f2 = f(m2);
            } else {
                hi = m2;
                m2 = m1;
                f2 = f1;
                m1 = hi - G * (hi - lo);
                f1 = f(m1);
            }
        }
        let peak = 0.5 * (lo + hi);
        let peak = if f(a) >= f(peak) && f(a) >= f(b) {
            a
        } else if f(b) >= f(peak) {
            b
        } else {
            peak
        };
        if f(peak) < 0.0 {
            return Spans::EMPTY;
        }
        let sa = if f(a) >= 0.0 {
            a
        } else {
            bisect(&f, a, peak)
        };
        let sb = if f(b) >= 0.0 {
            b
        } else {
            bisect(&f, b, peak)
        };
        out.push(sa, sb);
        return out;
    }
    let n = (((b - a) / (big_r / 8.0)).ceil() as usize).clamp(1, 256);
    let mut prev_s = a;
    let mut prev_in = f(a) >= 0.0;
    let mut start = if prev_in { Some(a) } else { None };
    for k in 1..=n {
        let s = a + (b - a) * k as f64 / n as f64;
        let inside = f(s) >= 0.0;
        if inside != prev_in {
            let edge = if inside {
                bisect(&f, prev_s, s)
            } else {
                bisect(&f, s, prev_s)
            };
            if inside {
                start = Some(edge);
            } else if let Some(st) = start.take() {
                out.push(st, edge);
            }
        }
        prev_s = s;
        prev_in = inside;
    }
    if let Some(st) = start {
        out.push(st, b);
    }
    out
}

/// Root of `f` between `out_s` (f < 0) and `in_s` (f >= 0).
fn bisect(f: &impl Fn(f64) -> f64, out_s: f64, in_s: f64) -> f64 {
    let (mut out_s, mut in_s) = (out_s, in_s);
    for _ in 0..45 {
        let m = 0.5 * (out_s + in_s);
        if f(m) >= 0.0 {
            in_s = m;
        } else {
            out_s = m;
        }
    }
    in_s
}

/// Removes `spans` (sorted, from a tool) from the sorted disjoint intervals
/// `[lo, hi]`, appending the remainder to `out`.
pub fn subtract(lo: f64, hi: f64, spans: &[(f64, f64)], out: &mut Vec<[f64; 2]>) {
    let mut cur = lo;
    for &(a, b) in spans {
        if b <= cur || a >= hi {
            continue;
        }
        if a > cur {
            out.push([cur, a.min(hi)]);
        }
        cur = cur.max(b);
        if cur >= hi {
            return;
        }
    }
    if cur < hi {
        out.push([cur, hi]);
    }
}
