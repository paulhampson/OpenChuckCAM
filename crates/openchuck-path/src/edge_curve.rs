// SPDX-License-Identifier: AGPL-3.0-or-later
//! Evaluating and sampling the exact curve under a B-rep edge (OP-08;
//! decision record 0227, OI-229).
//!
//! The kernel tessellates each edge at the model's tolerance, so a chord of
//! a curved edge can be 0.01 mm off the edge and a coarser model is further
//! off. [`sample`] walks the exact curve instead, with chords no more than
//! [`SAG`] below it, so edge following's engraving depth follows the real
//! edge. The result depends only on the curve, so it is the same at any
//! thread count (GC-07, NFR-06).

use openchuck_types::brep::{BSplineCurve, EdgeCurve, Frame};
use openchuck_types::geometry::Point3;

/// The most a chord of a sampled curve may lie from the curve, in
/// millimetres: a twentieth of the default tessellation tolerance, so it is
/// far inside any cut tolerance a user can ask for.
pub const SAG: f64 = 5.0e-4;

/// The pieces a curved edge is first cut into before bisecting, so a curve
/// whose midpoint happens to lie on its chord (an S bend) is still found.
const FIRST_PIECES: usize = 16;

/// The deepest bisection of one piece, so an invalid curve cannot run away.
const MAX_DEPTH: u32 = 14;

/// The most points a sampled edge may have.
const MAX_POINTS: usize = 1 << 20;

fn on_frame(f: &Frame, a: f64, b: f64) -> Point3 {
    Point3::new(
        f.origin.x + a * f.x.x + b * f.y.x,
        f.origin.y + a * f.x.y + b * f.y.y,
        f.origin.z + a * f.x.z + b * f.y.z,
    )
}

/// The point of the curve at parameter `t`; `None` when it is not finite or
/// the curve cannot be evaluated.
pub fn point(curve: &EdgeCurve, t: f64) -> Option<Point3> {
    let p = match curve {
        EdgeCurve::Line {
            origin, direction, ..
        } => Point3::new(
            origin.x + t * direction.x,
            origin.y + t * direction.y,
            origin.z + t * direction.z,
        ),
        EdgeCurve::Circle { frame, radius, .. } => {
            on_frame(frame, radius * t.cos(), radius * t.sin())
        }
        EdgeCurve::Ellipse {
            frame,
            major,
            minor,
            ..
        } => on_frame(frame, major * t.cos(), minor * t.sin()),
        EdgeCurve::BSpline(c) => spline_point(c, t)?,
        _ => return None,
    };
    p.is_finite().then_some(p)
}

/// The edge's parameter range, from its first point to its last.
fn range(curve: &EdgeCurve) -> Option<(f64, f64)> {
    match curve {
        EdgeCurve::Line { first, last, .. }
        | EdgeCurve::Circle { first, last, .. }
        | EdgeCurve::Ellipse { first, last, .. } => Some((*first, *last)),
        EdgeCurve::BSpline(c) => Some((c.first, c.last)),
        _ => None,
    }
}

/// de Boor's algorithm in homogeneous coordinates.
fn spline_point(c: &BSplineCurve, t: f64) -> Option<Point3> {
    let (n, d) = (c.poles.len(), c.degree as usize);
    if d == 0 || n <= d || c.knots.len() != n + d + 1 || c.weights.len() != n {
        return None;
    }
    let (lo, hi) = (*c.knots.get(d)?, *c.knots.get(n)?);
    let t = t.clamp(lo, hi);
    // The span k with knots[k] <= t < knots[k + 1], the last one at the end.
    let mut k = d;
    while k + 1 < n && *c.knots.get(k + 1)? <= t {
        k += 1;
    }
    let mut work: Vec<[f64; 4]> = Vec::with_capacity(d + 1);
    for j in 0..=d {
        let i = k + j - d;
        let (p, w) = (c.poles.get(i)?, *c.weights.get(i)?);
        work.push([p.x * w, p.y * w, p.z * w, w]);
    }
    for r in 1..=d {
        for j in (r..=d).rev() {
            let i = k + j - d;
            let left = *c.knots.get(i)?;
            let right = *c.knots.get(i + d + 1 - r)?;
            let span = right - left;
            let alpha = if span > 0.0 { (t - left) / span } else { 0.0 };
            let (prev, cur) = (*work.get(j - 1)?, *work.get(j)?);
            let mut mixed = [0.0; 4];
            for (m, (p, c)) in mixed.iter_mut().zip(prev.iter().zip(&cur)) {
                *m = (1.0 - alpha) * p + alpha * c;
            }
            *work.get_mut(j)? = mixed;
        }
    }
    let [x, y, z, w] = *work.get(d)?;
    (w > 0.0).then(|| Point3::new(x / w, y / w, z / w))
}

/// How far `m` is from the segment `a` to `b`.
fn sag(a: Point3, b: Point3, m: Point3) -> f64 {
    let (ab, am) = (
        [b.x - a.x, b.y - a.y, b.z - a.z],
        [m.x - a.x, m.y - a.y, m.z - a.z],
    );
    let len2 = ab.iter().map(|v| v * v).sum::<f64>();
    let s = if len2 > 0.0 {
        (ab.iter().zip(&am).map(|(p, q)| p * q).sum::<f64>() / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let foot = Point3::new(a.x + s * ab[0], a.y + s * ab[1], a.z + s * ab[2]);
    foot.distance(m)
}

/// Appends the points after `a` up to and including `b` (parameters `ta`,
/// `tb`), bisecting while the chord is more than [`SAG`] from the curve at
/// its quarter, middle or three-quarter point. `None` when the curve fails
/// to evaluate, the point limit is passed, or the deepest bisection still
/// leaves a chord further than [`SAG`] (the caller keeps the polyline
/// rather than treat such samples as exact).
fn refine(
    curve: &EdgeCurve,
    (ta, a): (f64, Point3),
    (tb, b): (f64, Point3),
    depth: u32,
    out: &mut Vec<Point3>,
) -> Option<()> {
    let tm = 0.5 * (ta + tb);
    let m = point(curve, tm)?;
    let mut worst = sag(a, b, m);
    for f in [0.25, 0.75] {
        worst = worst.max(sag(a, b, point(curve, ta + f * (tb - ta))?));
    }
    if worst > SAG {
        if depth >= MAX_DEPTH {
            return None;
        }
        refine(curve, (ta, a), (tm, m), depth + 1, out)?;
        refine(curve, (tm, m), (tb, b), depth + 1, out)?;
    } else {
        out.push(b);
    }
    (out.len() <= MAX_POINTS).then_some(())
}

/// The edge as a polyline of points on the exact curve, from its first
/// point to its last, with every chord within [`SAG`] of the curve. A line
/// is its two ends. `None` when the curve cannot be evaluated (the caller
/// keeps the kernel's polyline).
pub fn sample(curve: &EdgeCurve) -> Option<Vec<Point3>> {
    let (first, last) = range(curve)?;
    let (a, b) = (point(curve, first)?, point(curve, last)?);
    if matches!(curve, EdgeCurve::Line { .. }) {
        return Some(vec![a, b]);
    }
    let pieces = match curve {
        EdgeCurve::BSpline(c) => FIRST_PIECES.max(c.poles.len() * 2),
        _ => FIRST_PIECES,
    };
    // The seeds the edge is cut at before bisecting: even steps, and for a
    // spline its interior knots too, so a feature in a short span between
    // two steps cannot hide behind a chord whose midpoint lies on it. They
    // run from `first` to `last` whichever is larger, and end at `last`.
    let mut seeds: Vec<f64> = (1..pieces)
        .map(|i| first + (last - first) * i as f64 / pieces as f64)
        .collect();
    if let EdgeCurve::BSpline(c) = curve {
        let (low, high) = (first.min(last), first.max(last));
        seeds.extend(c.knots.iter().copied().filter(|k| *k > low && *k < high));
    }
    seeds.sort_by(|x, y| {
        let order = x.total_cmp(y);
        if first <= last {
            order
        } else {
            order.reverse()
        }
    });
    seeds.dedup();
    seeds.push(last);
    let mut out = vec![a];
    let mut previous = (first, a);
    for t in seeds {
        let here = if t.total_cmp(&last).is_eq() {
            b
        } else {
            point(curve, t)?
        };
        refine(curve, previous, (t, here), 0, &mut out)?;
        previous = (t, here);
    }
    Some(out)
}
