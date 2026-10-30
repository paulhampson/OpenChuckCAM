// SPDX-License-Identifier: AGPL-3.0-or-later
//! Faces across the rotary axis (OP-03, OP-04, OP-05; issue 384; decision
//! record 0297).
//!
//! A helix advances along the axis by its pitch every turn, so on a face
//! perpendicular to the axis the side of the tool reaches the face only where
//! a turn happens to pass the X at which the side touches it. Everywhere else
//! the nearest turn is up to one pitch short, and the face keeps a step of
//! material up to one pitch thick. The helix strategies therefore add one
//! whole turn of A at constant X at each such face, at the X where the side
//! of the tool just touches it, as rotary roughing already does beside a held
//! region (decision record 0161 item 6).
//!
//! **Finding the faces.** The tool the strategy drops (grown by its stock to
//! leave) is dropped along lines of X at angles all round the axis, no
//! further apart than its diameter at the part's largest radius, with X
//! samples no further apart than half its radius (and 0.5 mm). Where the
//! side of the tool meets a face across the axis, the drop's height jumps:
//! one side of the jump the tool rides the lower surface with its side on
//! the face, the other it rides the top of the face. A jump between two
//! neighbouring samples is narrowed by bisection to [`WIDTH`]; one still
//! larger than the toolpath tolerance there is a face, and the turn goes on
//! the low side of it, a tenth of the tolerance clear ([`CLEAR`]), so the
//! tool's side meets the face. A slope or a gentle curve is passed over
//! without bisecting: its change over an interval is nearly that of both
//! intervals either side. The line at A 0 is searched first; on the other
//! lines, a face found there whose jump still holds is taken with two drops
//! instead of a bisection (a face square to the axis is at the same X all
//! round), and anything else is bisected afresh. The height on top of the face is kept for
//! each line of A, so a strategy can tell where round the axis the face is. A slope, however
//! steep, rises by less than the tolerance over [`WIDTH`] and is left to the
//! helix; a tool whose side is not straight up (a tapered tool) meets no
//! jump and gets no turns. Only the X of the turns is found: the turn itself
//! drops the tool at every point, so it never cuts the part, and a face that
//! covers only part of the circumference (a crank web) is cut where it is.
//!
//! The angles and X samples are fixed by the part's size and the tool, not
//! the thread count, and the turns are sorted, so the result is the same
//! whatever the threads (GC-07, NFR-06). A part with no such face gives no
//! turns, and its path is made exactly as before.

use openchuck_types::job::JobContext;
use openchuck_types::tool::profile::RevolvedProfile;
use openchuck_types::units::{Degrees, Millimetres};

use crate::error::{PathError, Result};
use crate::parallel;
use crate::strategy::StrategyInput;

/// How narrow, in mm of X, a jump in the drop height is made by bisection
/// before it is judged. A slope steeper than tolerance / `WIDTH` (10,000 for
/// the default tolerance, within 0.006 degrees of square to the axis) counts
/// as a face.
const WIDTH: f64 = 1.0e-6;

/// How far, as a fraction of the toolpath tolerance, a turn stands clear of
/// its face on the low side: the most it adds to what the face keeps.
const CLEAR: f64 = 0.1;

/// The most X samples on one line, and the most angles; a larger part is
/// sampled more coarsely rather than run out of time (NFR-07).
const MAX_SAMPLES: f64 = 100_000.0;
const MAX_ANGLES: f64 = 720.0;
const MIN_ANGLES: f64 = 36.0;

/// The most turns one run may add (NFR-07).
const MAX_TURNS: usize = 10_000;

/// A turn at a face across the axis.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Turn {
    /// The X of the turn: the low side of the face, where the side of the
    /// tool touches it.
    pub(crate) x: f64,
    /// How high the tool's tip stands on top of the face, along the tool
    /// axis, on each line of A the faces were found on (`360 k / n` degrees
    /// for `n` lines); negative infinity on a line where the face is not.
    tops: Vec<f64>,
}

impl Turn {
    /// How high the face reaches at A `a` (degrees): the higher of the two
    /// lines either side of it, or `None` where neither meets the face.
    pub(crate) fn top_at(&self, a: f64) -> Option<f64> {
        let n = self.tops.len();
        if n == 0 {
            return None;
        }
        let k = ((a.rem_euclid(360.0) / 360.0) * n as f64).floor() as usize % n;
        let top = self.tops.get(k).copied().unwrap_or(f64::NEG_INFINITY).max(
            self.tops
                .get((k + 1) % n)
                .copied()
                .unwrap_or(f64::NEG_INFINITY),
        );
        top.is_finite().then_some(top)
    }
}

/// The turns a helix from `range.0` to `range.1` adds at the faces across the
/// axis that `profile` meets, in increasing X. `tolerance` is the toolpath
/// tolerance. X inside a `keep_out`
/// interval (a held region widened by the tool) gets no turn.
pub(crate) fn turns(
    input: &StrategyInput<'_>,
    profile: &RevolvedProfile,
    range: (f64, f64),
    tolerance: f64,
    context: &JobContext<'_>,
) -> Result<Vec<Turn>> {
    let (x0, x1) = range;
    let radius = profile.cutting_radius();
    if !(x1 > x0 && radius > 0.0 && tolerance > 0.0) {
        return Ok(Vec::new());
    }
    let bounds = input.surface.bounds();
    let reach = [bounds.min.y, bounds.max.y, bounds.min.z, bounds.max.z]
        .into_iter()
        .fold(0.0_f64, |r, v| r.max(v.abs()));
    if !reach.is_finite() || reach <= 0.0 {
        return Ok(Vec::new());
    }
    // X samples no further apart than half the tool's radius (and 0.5 mm):
    // a thin web shows both its faces in different intervals (the tool rides
    // its top for its width plus a diameter), and a ball's jump at a face is
    // not lost beside its roll over the face's edge.
    let spacing = (0.5 * radius).min(0.5);
    let samples = ((x1 - x0) / spacing).ceil().clamp(1.0, MAX_SAMPLES) as usize;
    // Angles no further apart than the tool's diameter at the part's largest
    // radius: a face narrower round the axis than that may be missed.
    let angles = (std::f64::consts::TAU * reach / (2.0 * radius))
        .ceil()
        .clamp(MIN_ANGLES, MAX_ANGLES) as usize;
    let height = |x: f64, a: f64| -> Option<f64> {
        let line = input
            .rotary
            .tool_line(Millimetres(x), Degrees(a), Millimetres(0.0));
        let contact = input.surface.drop_tool(profile, &line)?;
        Some(input.rotary.to_axis(contact.tip, Degrees(a)).height)
    };
    let at = |i: usize| x0 + (x1 - x0) * i as f64 / samples as f64;
    // The faces on one line of A, given the brackets of faces already found
    // on another: where one lies inside a jump and still holds the jump, it
    // is taken without bisecting again.
    let line = |k: usize, known: &[Found]| -> Vec<Found> {
        let a = 360.0 * k as f64 / angles as f64;
        let heights: Vec<Option<f64>> = (0..=samples).map(|i| height(at(i), a)).collect();
        // The change of height over each interval, where both ends have one.
        let change = |i: usize| match (heights.get(i), heights.get(i + 1)) {
            (Some(Some(lo)), Some(Some(hi))) => Some(hi - lo),
            _ => None,
        };
        let mut out = Vec::new();
        for (i, pair) in heights.windows(2).enumerate() {
            let (Some(Some(lo)), Some(Some(hi))) = (pair.first(), pair.get(1)) else {
                continue;
            };
            let d = hi - lo;
            if d.abs() <= tolerance {
                continue;
            }
            // On a slope or a gentle curve the change is nearly that of the
            // intervals either side; a face makes it stand out from at least
            // one (a ball's roll over the face's edge may match the other).
            let smooth = |j: Option<usize>| {
                j.and_then(change)
                    .is_some_and(|e| (d - e).abs() <= tolerance)
            };
            if smooth(i.checked_sub(1)) && smooth(Some(i + 1)) {
                continue;
            }
            let (xa, xb) = (at(i), at(i + 1));
            let reused = known
                .iter()
                .filter(|f| f.up == (d > 0.0) && xa <= f.bracket.0 && f.bracket.1 <= xb)
                .find_map(|f| {
                    let (l, h) = (height(f.bracket.0, a)?, height(f.bracket.1, a)?);
                    let holds = (h - l).abs() > tolerance && (h > l) == f.up;
                    holds.then_some(Found {
                        top: l.max(h),
                        ..*f
                    })
                });
            if let Some(face) =
                reused.or_else(|| narrow((xa, *lo), (xb, *hi), tolerance, |x| height(x, a)))
            {
                out.push(face);
            }
        }
        out
    };
    context.report("finding faces across the axis", 0, Some(angles as u64));
    let first = line(0, &[]);
    let rest: Vec<usize> = (1..angles).collect();
    let found: Vec<Vec<Found>> = std::iter::once(first.clone())
        .chain(parallel::map_in_order(&rest, |k| line(*k, &first)))
        .collect();
    context.check_cancelled()?;
    let mut ups: Vec<(Found, usize)> = Vec::new();
    let mut downs: Vec<(Found, usize)> = Vec::new();
    for (k, line) in found.into_iter().enumerate() {
        for f in line {
            if f.up {
                ups.push((f, k));
            } else {
                downs.push((f, k));
            }
        }
    }
    // One turn per face: the samples of a face at different angles agree to
    // within the bisection, so those within the tolerance are one face. The
    // turn takes the one furthest onto the low side, so the tool's side is on
    // the face, not on its top, at every angle.
    let mut turns = cluster(ups, tolerance, f64::min, angles);
    turns.extend(cluster(downs, tolerance, f64::max, angles));
    turns.sort_by(|a, b| a.x.total_cmp(&b.x));
    turns.dedup_by(|b, a| {
        let same = b.x - a.x <= tolerance;
        if same {
            for (t, u) in a.tops.iter_mut().zip(&b.tops) {
                *t = t.max(*u);
            }
        }
        same
    });
    turns.retain(|t| {
        (x0..=x1).contains(&t.x) && !input.keep_out.iter().any(|&(a, b)| (a..=b).contains(&t.x))
    });
    if turns.len() > MAX_TURNS {
        return Err(PathError::Strategy(format!(
            "the part has {} faces across the rotary axis; the most this strategy turns at is \
             {MAX_TURNS}",
            turns.len()
        )));
    }
    Ok(turns)
}

/// The note an operation carries when it turned at faces across the axis:
/// how many and where, so the user knows the path differs from a plain helix
/// there (decision record 0297). `None` with no turns.
pub(crate) fn note(turns: &[f64]) -> Option<String> {
    let (last, rest) = turns.split_last()?;
    let at = |x: &f64| format!("{x:.3}");
    let xs = if rest.is_empty() {
        at(last)
    } else {
        format!(
            "{} and {}",
            rest.iter().map(at).collect::<Vec<_>>().join(", "),
            at(last)
        )
    };
    let (count, each) = if rest.is_empty() {
        ("a face".to_owned(), "it")
    } else {
        (format!("{} faces", turns.len()), "each")
    };
    Some(format!(
        "the part has {count} across the rotary axis: the tool turns once round the axis \
         without moving along it at X {xs}, where its side touches {each}, so the face keeps \
         only the stock to leave"
    ))
}

/// A face found on one line of A: the X of its low side, where the side of
/// the tool touches it, the height on top of it, whether the height rises
/// with X, and the X either side of its jump.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Found {
    x: f64,
    top: f64,
    up: bool,
    /// The X either side of the jump after bisection.
    bracket: (f64, f64),
}

/// The face in a jump of the drop height from `a` to `b` (X, height),
/// narrowed by bisection to [`WIDTH`]. `None` where the jump is a slope or a
/// curve (its middle is within the tolerance of the straight line between
/// its ends, or its change falls within the tolerance) or the tool leaves the
/// part inside it.
fn narrow(
    a: (f64, f64),
    b: (f64, f64),
    tolerance: f64,
    height: impl Fn(f64) -> Option<f64>,
) -> Option<Found> {
    let (mut a, mut b) = (a, b);
    while b.0 - a.0 > WIDTH {
        // A slope's change halves with the interval and soon falls within
        // the tolerance; a face's does not.
        if (b.1 - a.1).abs() <= tolerance {
            return None;
        }
        let m = 0.5 * (a.0 + b.0);
        let h = height(m)?;
        // A face puts the middle on one side of its jump, about half the
        // jump from the straight line between the ends; a slope or a gentle
        // curve keeps it on the line, within the tolerance.
        if (h - 0.5 * (a.1 + b.1)).abs() <= tolerance {
            return None;
        }
        // Keep the half with the larger change: the jump is in it.
        if (h - a.1).abs() >= (b.1 - h).abs() {
            b = (m, h);
        } else {
            a = (m, h);
        }
    }
    let up = b.1 > a.1;
    let top = a.1.max(b.1);
    // A tenth of the tolerance further onto the low side, so the tool's side
    // is clear of the face rather than tangent to it, which a drop may read
    // either way and lift the tool onto the top of the face.
    let clear = CLEAR * tolerance;
    let x = if up { a.0 - clear } else { b.0 + clear };
    ((b.1 - a.1).abs() > tolerance).then_some(Found {
        x,
        top,
        up,
        bracket: (a.0, b.0),
    })
}

/// `found` (each with the line of A it was found on) sorted by X and gathered
/// into runs no more than `gap` apart between neighbours: a turn each, at the
/// X `pick` chooses of its members, with the highest top on each of the
/// `lines`.
fn cluster(
    mut found: Vec<(Found, usize)>,
    gap: f64,
    pick: fn(f64, f64) -> f64,
    lines: usize,
) -> Vec<Turn> {
    found.sort_by(|a, b| a.0.x.total_cmp(&b.0.x));
    let mut out: Vec<Turn> = Vec::new();
    let mut last = f64::NEG_INFINITY;
    for (f, k) in found {
        if out.last().is_none() || f.x - last > gap {
            out.push(Turn {
                x: f.x,
                tops: vec![f64::NEG_INFINITY; lines],
            });
        }
        if let Some(turn) = out.last_mut() {
            turn.x = pick(turn.x, f.x);
            if let Some(t) = turn.tops.get_mut(k) {
                *t = t.max(f.top);
            }
        }
        last = f.x;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn req_op_04_a_step_is_narrowed_to_its_low_side() {
        // A step up at X 2 of 3 mm: the low side is just below 2, a tenth of
        // the tolerance clear of it.
        let clear = 0.1 * 0.01;
        let step = |x: f64| Some(if x < 2.0 { 1.0 } else { 4.0 });
        let f = narrow((1.5, 1.0), (2.5, 4.0), 0.01, step).unwrap();
        assert!(f.up);
        assert!(f.x < 2.0 - clear && 2.0 - clear - f.x <= WIDTH, "{f:?}");
        assert!((f.top - 4.0).abs() < 1e-12, "{f:?}");
        // A step down: the low side is just above it.
        let down = |x: f64| Some(if x < 2.0 { 4.0 } else { 1.0 });
        let f = narrow((1.5, 4.0), (2.5, 1.0), 0.01, down).unwrap();
        assert!(!f.up);
        assert!(f.x >= 2.0 + clear && f.x - 2.0 - clear <= WIDTH, "{f:?}");
    }

    #[test]
    fn req_op_04_the_note_says_where_the_tool_turns() {
        assert_eq!(note(&[]), None);
        assert_eq!(
            note(&[8.4115]).unwrap(),
            "the part has a face across the rotary axis: the tool turns once round the axis \
             without moving along it at X 8.412, where its side touches it, so the face keeps \
             only the stock to leave"
        );
        assert_eq!(
            note(&[1.0, 2.0, 3.5]).unwrap(),
            "the part has 3 faces across the rotary axis: the tool turns once round the axis \
             without moving along it at X 1.000, 2.000 and 3.500, where its side touches each, \
             so the face keeps only the stock to leave"
        );
    }

    #[test]
    fn req_op_04_a_steep_slope_is_not_a_face() {
        // 89 degrees from the axis: 57 mm of height per mm of X.
        let slope = |x: f64| Some(57.0 * x);
        assert_eq!(narrow((0.0, 0.0), (0.1, 5.7), 0.01, slope), None);
        // A curve that rises straight up at one end, as a ball rolling over
        // an edge does: a quarter of a circle of 2 mm. It is continuous, so
        // it is no face.
        let curve = |x: f64| Some((4.0 - (2.0 - x) * (2.0 - x)).max(0.0).sqrt());
        let end = curve(0.5).unwrap();
        assert_eq!(narrow((0.0, 0.0), (0.5, end), 0.01, curve), None);
        // Leaving the part inside the jump gives no face.
        let off = |x: f64| (x < 0.05).then_some(0.0);
        assert_eq!(narrow((0.0, 0.0), (0.1, 3.0), 0.01, off), None);
    }

    #[test]
    fn req_op_04_samples_of_one_face_make_one_turn() {
        let at = |x: f64, top: f64, k: usize| {
            (
                Found {
                    x,
                    top,
                    up: true,
                    bracket: (x, x),
                },
                k,
            )
        };
        let found = vec![
            at(3.0, 1.0, 0),
            at(1.0, 2.0, 1),
            at(1.000_001, 5.0, 2),
            at(1.000_002, 3.0, 2),
            at(3.000_000_5, 4.0, 3),
        ];
        let low = cluster(found.clone(), 0.01, f64::min, 4);
        assert_eq!(low.iter().map(|t| t.x).collect::<Vec<_>>(), vec![1.0, 3.0]);
        let high = cluster(found, 0.01, f64::max, 4);
        assert_eq!(
            high.iter().map(|t| t.x).collect::<Vec<_>>(),
            vec![1.000_002, 3.000_000_5]
        );
        // The first face is on lines 1 and 2 (A 90 and 180), the highest top
        // on each; the second on lines 0 and 3.
        let face = &low[0];
        assert_eq!(face.top_at(90.0), Some(5.0));
        assert_eq!(face.top_at(45.0), Some(2.0));
        assert_eq!(face.top_at(200.0), Some(5.0));
        assert_eq!(face.top_at(300.0), None);
        assert_eq!(low[1].top_at(300.0), Some(4.0));
    }
}
