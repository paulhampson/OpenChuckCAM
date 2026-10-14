// SPDX-License-Identifier: AGPL-3.0-or-later
//! Where a straight line runs inside the tool: the one geometric query the
//! sweep and the collision sketch use (decision record 0022, SP-6).
//!
//! The tool solid is `{ r <= R, h(r) <= w <= top }` in the tool frame (axis
//! `+w`, tip at the origin), `h` being the profile's lower envelope and `top`
//! the flute length. The shank above the flutes is not modelled: cutting
//! deeper than the flutes is a collision question (COL-05), not a removal one
//! (OI-102).

use openchuck_types::tool::RevolvedProfile;
use openchuck_types::tool::profile::ProfilePiece;

/// At most this many separate spans are kept per line (non-convex profiles).
pub const MAX_SPANS: usize = 8;

/// The peak search stops once its bracket is this short, in millimetres.
const SEARCH_TOLERANCE: f64 = 1e-7;

/// Root brackets are refined to this width, in millimetres.
const ROOT_TOLERANCE: f64 = 1e-9;

/// Hard cap on search iterations, whatever the bracket.
const MAX_ITERATIONS: u32 = 80;

/// One piece of the lower envelope in a form that is cheap to evaluate.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Piece {
    /// `h = z0 + (r - r0) * slope` up to `r_end`.
    Line {
        r0: f64,
        z0: f64,
        slope: f64,
        r_end: f64,
    },
    /// A circular arc about (`cr`, `cz`) up to `r_end`; below the centre when
    /// `convex` (like a ball), above it otherwise.
    Arc {
        cr: f64,
        cz: f64,
        radius2: f64,
        convex: bool,
        r_end: f64,
    },
}

impl Piece {
    fn r_end(&self) -> f64 {
        match *self {
            Self::Line { r_end, .. } | Self::Arc { r_end, .. } => r_end,
        }
    }

    fn height_at(&self, r: f64) -> f64 {
        match *self {
            Self::Line { r0, z0, slope, .. } => z0 + (r - r0) * slope,
            Self::Arc {
                cr,
                cz,
                radius2,
                convex,
                ..
            } => {
                let dr = r - cr;
                let q = (radius2 - dr * dr).max(0.0).sqrt();
                if convex { cz - q } else { cz + q }
            }
        }
    }
}

/// A revolved tool profile prepared for the sweep (TL-03: every tool type,
/// standard or custom, arrives as a [`RevolvedProfile`]).
#[derive(Debug, Clone, PartialEq)]
pub struct ToolSolid {
    pieces: Vec<Piece>,
    radius: f64,
    top: f64,
    envelope_top: f64,
    convex: bool,
}

impl ToolSolid {
    /// The solid of `profile`, from the tip to the top of the flutes.
    pub fn new(profile: &RevolvedProfile) -> Self {
        let pieces = profile
            .envelope()
            .iter()
            .map(|e| match e.piece {
                ProfilePiece::Line { start, end } => Piece::Line {
                    r0: start.radius.0,
                    z0: start.height.0,
                    slope: (end.height.0 - start.height.0) / (end.radius.0 - start.radius.0),
                    r_end: e.r_end,
                },
                ProfilePiece::Arc {
                    centre,
                    radius,
                    convex,
                    ..
                } => Piece::Arc {
                    cr: centre.radius.0,
                    cz: centre.height.0,
                    radius2: radius * radius,
                    convex,
                    r_end: e.r_end,
                },
            })
            .collect();
        let mut solid = Self {
            pieces,
            radius: profile.cutting_radius(),
            top: profile.top(),
            envelope_top: 0.0,
            convex: profile.is_convex(),
        };
        solid.envelope_top = solid.height_at(solid.radius);
        solid
    }

    /// The cutting radius `R`.
    pub fn radius(&self) -> f64 {
        self.radius
    }

    /// The top of the flutes above the tip.
    pub fn top(&self) -> f64 {
        self.top
    }

    /// The lower envelope `h(r)` for `0 <= r <= R` (clamped).
    pub fn height_at(&self, r: f64) -> f64 {
        let r = r.clamp(0.0, self.radius);
        for p in &self.pieces {
            if r <= p.r_end() {
                return p.height_at(r);
            }
        }
        self.pieces.last().map_or(0.0, |p| p.height_at(p.r_end()))
    }

    /// True when a point in the tool frame (`r` from the axis, `w` above the
    /// tip) lies inside the tool solid.
    pub fn contains(&self, r: f64, w: f64) -> bool {
        r <= self.radius && w <= self.top && w >= self.height_at(r)
    }
}

/// The spans of a line inside the tool, in increasing parameter order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spans {
    n: usize,
    v: [(f64, f64); MAX_SPANS],
}

impl Spans {
    /// No spans.
    pub const EMPTY: Self = Self {
        n: 0,
        v: [(0.0, 0.0); MAX_SPANS],
    };

    /// The spans as a slice.
    pub fn as_slice(&self) -> &[(f64, f64)] {
        self.v.get(..self.n).unwrap_or_default()
    }

    /// True when the line misses the tool.
    pub fn is_empty(&self) -> bool {
        self.n == 0
    }

    fn push(&mut self, a: f64, b: f64) {
        if let Some(slot) = self.v.get_mut(self.n) {
            *slot = (a, b);
            self.n += 1;
        } else if let Some(last) = self.v.last_mut() {
            // Conservative: merge into the last span rather than lose cutting.
            last.1 = b;
        }
    }
}

/// The spans of the line `p(s) = o + s d` (tool frame, `d` a unit vector)
/// inside the tool, for `s` in `[s0, s1]`.
///
/// Convex profiles give one span, found exactly: `w - h(r(s))` is concave in
/// `s` (`w` is linear, `r` is convex in `s`, `h` is convex and non-decreasing),
/// so a golden-section search finds its peak and bisection finds the two
/// roots. Other profiles are marched at the smaller of `R / 8` and `march`
/// (the caller passes the simulation cell) and refined by bisection, so a
/// span missed there is shorter than one cell (OI-102).
pub fn cut_spans(
    tool: &ToolSolid,
    o: [f64; 3],
    d: [f64; 3],
    s0: f64,
    s1: f64,
    march: f64,
) -> Spans {
    let big_r = tool.radius;
    let top = tool.top;
    let (mut a, mut b) = (s0, s1);
    // Clip to the cylinder r <= R.
    let qa = d[0] * d[0] + d[1] * d[1];
    let qb = o[0] * d[0] + o[1] * d[1];
    let qc = o[0] * o[0] + o[1] * o[1] - big_r * big_r;
    if qa < 1e-18 {
        if qc > 0.0 {
            return Spans::EMPTY;
        }
    } else {
        let disc = qb * qb - qa * qc;
        if disc < 0.0 {
            return Spans::EMPTY;
        }
        let sq = disc.sqrt();
        a = a.max((-qb - sq) / qa);
        b = b.min((-qb + sq) / qa);
    }
    // Clip to the slab 0 <= w <= top.
    if d[2].abs() < 1e-15 {
        if o[2] < 0.0 || o[2] > top {
            return Spans::EMPTY;
        }
    } else {
        let t0 = -o[2] / d[2];
        let t1 = (top - o[2]) / d[2];
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
        w - tool.height_at(x.hypot(y))
    };
    let mut out = Spans::EMPTY;
    if qa < 1e-18 {
        // Parallel to the axis: r is constant, so the span is h(r)..top in w.
        let h = tool.height_at(o[0].hypot(o[1]));
        let (ta, tb) = if d[2].abs() < 1e-15 {
            (a, b)
        } else {
            let t0 = (h - o[2]) / d[2];
            let t1 = (top - o[2]) / d[2];
            (a.max(t0.min(t1)), b.min(t0.max(t1)))
        };
        if ta <= tb {
            out.push(ta, tb);
        }
        return out;
    }
    // At or above H = h(R) the solid is the plain cylinder, already clipped
    // (h never decreases, so h(r) <= H for every r <= R): only the part of
    // the line below H needs the envelope (OI-101).
    let hh = tool.envelope_top;
    let (ua, ub) = if d[2].abs() < 1e-15 {
        if o[2] >= hh {
            (a, b)
        } else {
            (f64::INFINITY, f64::NEG_INFINITY)
        }
    } else {
        let sh = (hh - o[2]) / d[2];
        if d[2] > 0.0 {
            (a.max(sh), b)
        } else {
            (a, b.min(sh))
        }
    };
    if ua <= ub {
        if tool.convex {
            // f is concave and f >= 0 on [ua, ub], so the span is one interval
            // containing it, found by one root on the low side.
            let lo = if ua > a && f(a) < 0.0 {
                root(&f, a, ua)
            } else {
                a.min(ua)
            };
            let hi = if ub < b && f(b) < 0.0 {
                root(&f, b, ub)
            } else {
                b.max(ub)
            };
            out.push(lo, hi);
            return out;
        }
        let step = (big_r / 8.0).min(march).max(1e-6);
        if ua > a {
            // Rising: the low part comes first and its last span ends at ua.
            march_spans(&f, a, ua, step, &mut out);
            let joined = out.n > 0 && out.as_slice().last().is_some_and(|l| l.1 >= ua);
            if joined {
                if let Some(last) = out.v.get_mut(out.n - 1) {
                    last.1 = ub;
                }
            } else {
                out.push(ua, ub);
            }
        } else if ub < b {
            // Falling: the high part comes first; the low part's first span
            // starts at ub.
            let mut low = Spans::EMPTY;
            march_spans(&f, ub, b, step, &mut low);
            let mut rest = low.as_slice();
            let mut end = ub;
            if let Some((first, tail)) = rest.split_first()
                && first.0 <= ub
            {
                end = first.1;
                rest = tail;
            }
            out.push(ua, end);
            for &(x, y) in rest {
                out.push(x, y);
            }
        } else {
            out.push(ua, ub);
        }
        return out;
    }
    if tool.convex {
        let peak = golden_max(&f, a, b);
        let (fa, fb, fp) = (f(a), f(b), f(peak));
        let (peak, fp) = if fa >= fp && fa >= fb {
            (a, fa)
        } else if fb >= fp {
            (b, fb)
        } else {
            (peak, fp)
        };
        if fp < 0.0 {
            return Spans::EMPTY;
        }
        let sa = if fa >= 0.0 { a } else { root(&f, a, peak) };
        let sb = if fb >= 0.0 { b } else { root(&f, b, peak) };
        out.push(sa, sb);
        return out;
    }
    let step = (big_r / 8.0).min(march).max(1e-6);
    march_spans(&f, a, b, step, &mut out);
    out
}

/// The spans of `f >= 0` on `[a, b]`, marched at about `step` and refined
/// by root finding; a span shorter than the step can be missed.
fn march_spans(f: &impl Fn(f64) -> f64, a: f64, b: f64, step: f64, out: &mut Spans) {
    let n = ((b - a) / step).ceil().clamp(1.0, 4096.0) as usize;
    let mut prev_s = a;
    let mut prev_in = f(a) >= 0.0;
    let mut start = prev_in.then_some(a);
    for k in 1..=n {
        let s = a + (b - a) * k as f64 / n as f64;
        let inside = f(s) >= 0.0;
        if inside != prev_in {
            if inside {
                start = Some(root(f, prev_s, s));
            } else if let Some(st) = start.take() {
                out.push(st, root(f, s, prev_s));
            }
        }
        prev_s = s;
        prev_in = inside;
    }
    if let Some(st) = start {
        out.push(st, b);
    }
}

/// The peak of a concave `f` on `[lo, hi]` by golden-section search.
fn golden_max(f: &impl Fn(f64) -> f64, mut lo: f64, mut hi: f64) -> f64 {
    const G: f64 = 0.618_033_988_749_894_9;
    let mut m1 = hi - G * (hi - lo);
    let mut m2 = lo + G * (hi - lo);
    let (mut f1, mut f2) = (f(m1), f(m2));
    let mut i = 0;
    while hi - lo > SEARCH_TOLERANCE && i < MAX_ITERATIONS {
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
        i += 1;
    }
    0.5 * (lo + hi)
}

/// The root of `f` between `out_s` (`f < 0`) and `in_s` (`f >= 0`) by the
/// Illinois variant of false position, falling back to bisection when a
/// step leaves the bracket; the returned parameter is inside (`f >= 0`).
fn root(f: &impl Fn(f64) -> f64, out_s: f64, in_s: f64) -> f64 {
    let (mut xo, mut xi) = (out_s, in_s);
    let (mut fo, mut fi) = (f(xo), f(xi));
    let mut side = 0i8;
    let mut i = 0;
    while (xi - xo).abs() > ROOT_TOLERANCE && i < MAX_ITERATIONS {
        let (lo, hi) = (xo.min(xi), xo.max(xi));
        let guess = (xo * fi - xi * fo) / (fi - fo);
        let x = if guess > lo && guess < hi {
            guess
        } else {
            0.5 * (xo + xi)
        };
        let fx = f(x);
        if fx >= 0.0 {
            xi = x;
            fi = fx;
            if side == 1 {
                fo *= 0.5;
            }
            side = 1;
        } else {
            xo = x;
            fo = fx;
            if side == -1 {
                fi *= 0.5;
            }
            side = -1;
        }
        i += 1;
    }
    xi
}

/// Removes `spans` (sorted, from one tool pose) from the sorted, disjoint
/// intervals `column`, writing what is left to `out`. Endpoints are copied,
/// never recomputed, so the result does not depend on the order poses are
/// applied in.
pub fn subtract(column: &[[f64; 2]], spans: &[(f64, f64)], out: &mut Vec<[f64; 2]>) {
    for &[lo, hi] in column {
        let mut cur = lo;
        let mut done = false;
        for &(a, b) in spans {
            if b <= cur || a >= hi {
                continue;
            }
            if a > cur {
                out.push([cur, a]);
            }
            if b >= hi {
                done = true;
                break;
            }
            cur = cur.max(b);
        }
        if !done && cur < hi {
            out.push([cur, hi]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openchuck_types::tool::{Tool, ToolShape};
    use openchuck_types::units::{Degrees, Millimetres};

    fn tool(shape: ToolShape) -> ToolSolid {
        let t = Tool {
            shape,
            diameter: Millimetres(6.0),
            flute_length: Millimetres(20.0),
            overall_length: Millimetres(50.0),
            shank_diameter: Millimetres(6.0),
            flute_count: 2,
            material: None,
        };
        ToolSolid::new(&t.profile().unwrap())
    }

    #[test]
    fn req_sim_03_vertical_line_through_a_ball_starts_on_the_sphere() {
        let ball = tool(ToolShape::BallEnd);
        // A line along the axis offset by 2 mm: enters at 3 - sqrt(9 - 4).
        let spans = cut_spans(
            &ball,
            [2.0, 0.0, -10.0],
            [0.0, 0.0, 1.0],
            -100.0,
            100.0,
            0.1,
        );
        let s = spans.as_slice();
        assert_eq!(s.len(), 1);
        assert!((s[0].0 - (10.0 + 3.0 - 5f64.sqrt())).abs() < 1e-6);
        assert!((s[0].1 - 30.0).abs() < 1e-9);
    }

    #[test]
    fn req_sim_03_horizontal_line_through_a_v_bit_matches_the_cone() {
        let v = tool(ToolShape::VBit {
            included_angle: Degrees(90.0),
            tip_diameter: Millimetres(0.0),
        });
        // At w = 2 the 90 degree V has radius 2: the line y = 0, w = 2 is
        // inside for |x| <= 2.
        let spans = cut_spans(&v, [-10.0, 0.0, 2.0], [1.0, 0.0, 0.0], -100.0, 100.0, 0.1);
        let s = spans.as_slice();
        assert_eq!(s.len(), 1);
        assert!((s[0].0 - 8.0).abs() < 1e-6, "{s:?}");
        assert!((s[0].1 - 12.0).abs() < 1e-6, "{s:?}");
    }

    #[test]
    fn req_sim_03_subtract_keeps_the_outside_pieces() {
        let mut out = Vec::new();
        subtract(
            &[[0.0, 10.0], [12.0, 20.0]],
            &[(2.0, 3.0), (9.0, 13.0)],
            &mut out,
        );
        assert_eq!(out, vec![[0.0, 2.0], [3.0, 9.0], [13.0, 20.0]]);
    }
}
