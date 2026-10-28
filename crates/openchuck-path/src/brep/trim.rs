// SPDX-License-Identifier: AGPL-3.0-or-later
//! The trim loops of one face, indexed for the trim test and the boundary
//! search (lane STEPALL-X1, issue #348; decision 0211 item 3.2).
//!
//! - The trim test is the even-odd rule over the loop segments in (u, v). A
//!   segment can only change the parity at `v` when `v` lies in its v range,
//!   so the segments are bucketed into strips of v: a query tests the
//!   segments of its strip, and gets the same answer the whole list gives,
//!   since the same crossing test runs on every segment that can cross.
//! - The boundary search finds the closest point of the loops mapped onto the
//!   surface. Each segment's image lies within a ball about the image of its
//!   middle, of radius half the segment's length in (u, v) weighted by bounds
//!   on the surface's speeds `|S_u|` and `|S_v|` over the segment. The
//!   distance to the centre less the radius is a lower bound on the
//!   distance to the segment's image. So is the distance to the chord
//!   between the images of its ends less an eighth of a bound on the
//!   image's second derivative (a curve strays no further from its chord).
//!   Segments whose bound is no nearer than the best answer so far are
//!   skipped without changing the answer.

use openchuck_types::geometry::Point3;

/// One loop segment, from `a` to `b` in (u, v), with the ball that holds
/// its image on the surface.
#[derive(Debug, Clone, Copy)]
pub(super) struct Segment {
    pub a: (f64, f64),
    pub b: (f64, f64),
    pub centre: openchuck_types::geometry::Point3,
    pub radius: f64,
    /// The images of the two ends.
    pub ends: (Point3, Point3),
    /// A bound on the image's second derivative along the segment (its
    /// parameter running from 0 to 1); infinite when there is none.
    pub bend: f64,
}

/// Whether the segment from `a` to `c` meets the box `[u0, u1, v0, v1]`
/// (touching counts): their boxes overlap and the box's corners are not all
/// strictly on one side of the segment's line.
fn meets(a: (f64, f64), c: (f64, f64), b: [f64; 4]) -> bool {
    let [u0, u1, v0, v1] = b;
    if a.0.max(c.0) < u0 || a.0.min(c.0) > u1 || a.1.max(c.1) < v0 || a.1.min(c.1) > v1 {
        return false;
    }
    let side = |u: f64, v: f64| (c.0 - a.0) * (v - a.1) - (c.1 - a.1) * (u - a.0);
    let s = [side(u0, v0), side(u1, v0), side(u0, v1), side(u1, v1)];
    !(s.iter().all(|&x| x > 0.0) || s.iter().all(|&x| x < 0.0))
}

/// The distance from `q` to the straight segment from `a` to `b`.
pub(super) fn chord_distance(q: Point3, a: Point3, b: Point3) -> f64 {
    let d = b - a;
    let l2 = d.dot(d);
    let t = if l2 > 0.0 {
        ((q - a).dot(d) / l2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    q.distance(a + d * t)
}

/// A lower bound on the distance from `q` to a curve piece: its image lies
/// within `radius` of `centre` and within `bend * w^2 / 8` of the chord
/// between its ends, `w` being the piece's length in its parameter.
pub(super) fn piece_lower(
    q: Point3,
    centre: Point3,
    radius: f64,
    ends: (Point3, Point3),
    bend_w2: f64,
) -> f64 {
    let ball = q.distance(centre) - radius;
    let chord = chord_distance(q, ends.0, ends.1) - bend_w2 / 8.0;
    // Rounding in the chord's distance: a relative hair.
    let chord = chord - 1e-12 * (1.0 + q.to_vec().length());
    ball.max(chord).max(0.0)
}

/// The segments of a face's loops, bucketed by v.
#[derive(Debug, Clone)]
pub(super) struct TrimIndex {
    /// Every segment, loop by loop, in loop order.
    pub segments: Vec<Segment>,
    /// The lowest v of any segment, and the strip height.
    v0: f64,
    dv: f64,
    v_max: f64,
    /// `starts[k]..starts[k + 1]` indexes `members` for strip `k`.
    starts: Vec<u32>,
    members: Vec<u32>,
}

impl TrimIndex {
    /// Indexes `segments` (already given their balls).
    pub fn new(segments: Vec<Segment>) -> Self {
        let lo = segments
            .iter()
            .map(|s| s.a.1.min(s.b.1))
            .fold(f64::INFINITY, f64::min);
        let hi = segments
            .iter()
            .map(|s| s.a.1.max(s.b.1))
            .fold(f64::NEG_INFINITY, f64::max);
        let n = segments.len();
        // About four segments a strip on a loop spread evenly in v.
        let strips = (n / 4).clamp(1, 4096);
        let (v0, v_max) = if lo.is_finite() && hi.is_finite() {
            (lo, hi)
        } else {
            (0.0, 0.0)
        };
        let dv = if v_max > v0 {
            (v_max - v0) / strips as f64
        } else {
            1.0
        };
        let strip_of = |v: f64| -> usize {
            let k = ((v - v0) / dv).floor();
            if k.is_nan() || k < 0.0 {
                0
            } else {
                (k as usize).min(strips - 1)
            }
        };
        // A strip lists every segment whose v range meets it, widened by a
        // strip each way so a rounded strip number cannot drop one.
        let mut lists: Vec<Vec<u32>> = vec![Vec::new(); strips];
        for (i, s) in segments.iter().enumerate() {
            let k0 = strip_of(s.a.1.min(s.b.1)).saturating_sub(1);
            let k1 = (strip_of(s.a.1.max(s.b.1)) + 1).min(strips - 1);
            for list in lists.iter_mut().take(k1 + 1).skip(k0) {
                list.push(u32::try_from(i).unwrap_or(u32::MAX));
            }
        }
        let mut starts = Vec::with_capacity(strips + 1);
        let mut members = Vec::new();
        starts.push(0);
        for l in lists {
            members.extend(l);
            starts.push(u32::try_from(members.len()).unwrap_or(u32::MAX));
        }
        Self {
            segments,
            v0,
            dv,
            v_max,
            starts,
            members,
        }
    }

    /// The even-odd trim test at (u, v): the same parity as testing every
    /// segment.
    pub fn inside(&self, u: f64, v: f64) -> bool {
        if self.segments.is_empty() || !(v >= self.v0 && v <= self.v_max) {
            // Outside the loops' v range no segment crosses.
            return false;
        }
        let strips = self.starts.len() - 1;
        let k = ((v - self.v0) / self.dv).floor();
        let k = if k < 0.0 {
            0
        } else {
            (k as usize).min(strips - 1)
        };
        let (Some(&s0), Some(&s1)) = (self.starts.get(k), self.starts.get(k + 1)) else {
            return false;
        };
        let mut odd = false;
        for &i in self.members.get(s0 as usize..s1 as usize).unwrap_or(&[]) {
            let Some(s) = self.segments.get(i as usize) else {
                continue;
            };
            let ((px, py), (x, y)) = (s.a, s.b);
            if (y > v) != (py > v) && u < (px - x) * (v - y) / (py - y) + x {
                odd = !odd;
            }
        }
        odd
    }

    /// Whether the (u, v) box `[u0, u1, v0, v1]` lies wholly inside the
    /// loops, wholly outside them, or may be crossed by one (`None`): no
    /// segment's box meets it, so its middle's side is the whole box's.
    pub fn box_side(&self, b: [f64; 4]) -> Option<bool> {
        let [u0, u1, v0, v1] = b;
        if self.segments.is_empty() {
            return Some(false);
        }
        if v1 >= self.v0 && v0 <= self.v_max {
            let strips = self.starts.len() - 1;
            let strip = |v: f64| -> usize {
                let k = ((v - self.v0) / self.dv).floor();
                if k.is_nan() || k < 0.0 {
                    0
                } else {
                    (k as usize).min(strips - 1)
                }
            };
            let (k0, k1) = (strip(v0), strip(v1));
            let (Some(&s0), Some(&s1)) = (self.starts.get(k0), self.starts.get(k1 + 1)) else {
                return None;
            };
            for &i in self.members.get(s0 as usize..s1 as usize).unwrap_or(&[]) {
                let Some(s) = self.segments.get(i as usize) else {
                    continue;
                };
                if meets(s.a, s.b, b) {
                    return None;
                }
            }
        }
        Some(self.inside(0.5 * (u0 + u1), 0.5 * (v0 + v1)))
    }

    /// The segments whose image may lie nearer `q` than `limit`, nearest
    /// bound first, with that bound: `(bound, segment)`. Ties go by
    /// segment order, so the order is deterministic.
    pub fn candidates(&self, q: Point3, limit: f64) -> Vec<(f64, usize)> {
        let mut out: Vec<(f64, usize)> = self
            .segments
            .iter()
            .enumerate()
            .filter_map(|(i, s)| {
                let lower = piece_lower(q, s.centre, s.radius, s.ends, s.bend);
                (lower <= limit).then_some((lower, i))
            })
            .collect();
        out.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        out
    }

    /// The nearest segment centre and its segment: the distance bounds the
    /// distance from `q` to the loops, since every centre is a point of
    /// their image. Ties go to the first segment.
    pub fn nearest_centre(&self, q: Point3) -> Option<(f64, usize)> {
        self.segments
            .iter()
            .enumerate()
            .map(|(i, s)| (q.distance(s.centre), i))
            .fold(None, |best: Option<(f64, usize)>, c| match best {
                Some(b) if b.0 <= c.0 => Some(b),
                _ => Some(c),
            })
    }
}
