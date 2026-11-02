// SPDX-License-Identifier: AGPL-3.0-or-later
//! Walls in the target (OP-13, OP-14; decision record 0050, closing OI-209).
//!
//! Where the tool meets a wall (a step in the part, a tab narrower than the
//! part's end face, a held region wider than the part), the tool's drop
//! height jumps: the tool rides at one height up to the wall and at another
//! past it. Path fitting bisects the move across the jump down to its
//! smallest step, so a pass reaches the wall through a cascade of ever
//! shorter moves (micro-moves, down to thousandths of a degree) and crosses
//! it in one move almost wholly along the tool axis (the wall step).
//!
//! The shared services treat a wall in three ways:
//!
//! 1. **Micro-moves are merged.** A run of micro-moves next to a wall step,
//!    on either side, is merged into as few moves as keep every dropped point
//!    within the path-fitting chord limit of the straight machine move that
//!    replaces it ([`merge_at_walls`]); on a flat or gently curved surface
//!    that is one move. The wall step itself is kept.
//! 2. **Contact smoothing stops at a wall step.** The contact jumps across the
//!    wall; averaging it with the points on the other side would charge part
//!    of the wall to the moves beside it, which then crawl at a fraction of a
//!    millimetre a minute ([`wall_breaks`]).
//! 3. **A move that hardly moves X or Y is timed against Z's limit**, since
//!    the post may write it with no X or Y at all (in `push`).

use openchuck_types::geometry::Point3;

use crate::rotary::{AxisPosition, RotaryMapping};
use crate::strategy::CutPoint;

/// A step's motion of the tool tip over the part, split into the part along
/// the tool axis and the part across it, in millimetres.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Motion {
    across: f64,
    along_axis: f64,
}

fn motion(rotary: &RotaryMapping, from: &CutPoint, to: &CutPoint) -> Motion {
    let d = to.tip - from.tip;
    let axis = rotary.tool_axis(to.a).get();
    let along_axis = d.dot(axis);
    let across = (d - axis * along_axis).length();
    Motion {
        across,
        along_axis: along_axis.abs(),
    }
}

/// What a step between two cut points is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    /// Moves the tool across the part by less than the tolerance and along
    /// its axis by more: the tool climbs or drops a wall.
    Wall,
    /// Moves the tool by less than the tolerance across the part and along
    /// its axis.
    Micro,
    /// Anything else.
    Normal,
}

fn classify(rotary: &RotaryMapping, from: &CutPoint, to: &CutPoint, tolerance: f64) -> Step {
    kind(motion(rotary, from, to), tolerance)
}

fn kind(m: Motion, tolerance: f64) -> Step {
    if m.across >= tolerance {
        Step::Normal
    } else if m.along_axis > tolerance {
        Step::Wall
    } else {
        Step::Micro
    }
}

/// The indices `i` of `points` where the step from `i - 1` to `i` is a wall
/// step, in increasing order.
pub(crate) fn wall_breaks(
    rotary: &RotaryMapping,
    points: &[CutPoint],
    tolerance: f64,
) -> Vec<usize> {
    points
        .windows(2)
        .enumerate()
        .filter_map(|(i, w)| match w {
            [a, b] if classify(rotary, a, b, tolerance) == Step::Wall => Some(i + 1),
            _ => None,
        })
        .collect()
}

/// How far, in millimetres, the tool on the straight machine move from `a`
/// to `b` passes from `mid`: the tip of the move at the parameter nearest
/// `mid` in the machine's axes (A counted as arc at the larger height), in
/// the model frame. A parameter that is not the nearest overstates the
/// distance, so the measure errs towards keeping a point.
fn deviation(rotary: &RotaryMapping, a: &CutPoint, b: &CutPoint, mid: &CutPoint) -> f64 {
    let pa = rotary.to_axis(a.tip, a.a);
    let pb = rotary.to_axis(b.tip, b.a);
    let pm = rotary.to_axis(mid.tip, mid.a);
    let radius = pa.height.abs().max(pb.height.abs()).max(pm.height.abs());
    let arc = |deg: f64| deg.to_radians() * radius;
    let d = [
        pb.along - pa.along,
        pb.across - pa.across,
        pb.height - pa.height,
        arc(b.a.0 - a.a.0),
    ];
    let m = [
        pm.along - pa.along,
        pm.across - pa.across,
        pm.height - pa.height,
        arc(mid.a.0 - a.a.0),
    ];
    let dd: f64 = d.iter().map(|v| v * v).sum();
    let s = if dd > 0.0 {
        (d.iter().zip(&m).map(|(x, y)| x * y).sum::<f64>() / dd).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let lerp = |x: f64, y: f64| x + s * (y - x);
    let at = AxisPosition {
        along: lerp(pa.along, pb.along),
        across: lerp(pa.across, pb.across),
        height: lerp(pa.height, pb.height),
    };
    let a_at = openchuck_types::units::Degrees(lerp(a.a.0, b.a.0));
    let tip: Point3 = rotary.to_model(at, a_at);
    tip.distance(mid.tip)
}

/// The fewest of `points` (a run of micro-moves and the points at its ends)
/// that keep every dropped point within `chord` of the straight machine move
/// between the kept points either side of it. The ends are always kept.
fn thin(rotary: &RotaryMapping, points: &[CutPoint], chord: f64) -> Vec<CutPoint> {
    let Some(first) = points.first() else {
        return Vec::new();
    };
    let last = points.len() - 1;
    let mut kept = vec![*first];
    let mut start = 0;
    while start < last {
        // Extend the move from `start` while every point between stays
        // within the chord; the next point always can be reached.
        let mut end = start + 1;
        for candidate in start + 2..=last {
            let (Some(a), Some(b), Some(between)) = (
                points.get(start),
                points.get(candidate),
                points.get(start + 1..candidate),
            ) else {
                break;
            };
            if between
                .iter()
                .all(|mid| deviation(rotary, a, b, mid) <= chord)
            {
                end = candidate;
            } else {
                break;
            }
        }
        if let Some(p) = points.get(end) {
            kept.push(*p);
        }
        start = end;
    }
    kept
}

/// A step of the bisection cascade into a wall is at most this fraction of
/// the step beyond it, away from the wall (bisection halves it).
const CASCADE: f64 = 0.75;

/// `points` with the micro-moves next to each wall step merged (see the
/// module documentation). The steps merged are those of the bisection
/// cascade into the wall: walking away from the wall step, every step that
/// moves less than `tolerance` (a micro-move), or at most [`CASCADE`] times
/// the step beyond it, until the first that is neither. So the merged moves
/// together are never longer than the step path fitting started from. The
/// points of each such run are thinned to the fewest that keep every dropped
/// point within `chord` of the move that replaces it. `tolerance` is the
/// toolpath tolerance, which also tells a wall step from a micro-move. A pass
/// with no wall step is returned unchanged.
pub(crate) fn merge_at_walls(
    rotary: &RotaryMapping,
    points: &[CutPoint],
    tolerance: f64,
    chord: f64,
) -> Vec<CutPoint> {
    let motions: Vec<Motion> = points
        .windows(2)
        .map(|w| match w {
            [a, b] => motion(rotary, a, b),
            _ => Motion {
                across: 0.0,
                along_axis: 0.0,
            },
        })
        .collect();
    let steps: Vec<Step> = motions.iter().map(|m| kind(*m, tolerance)).collect();
    if !steps.contains(&Step::Wall) {
        return points.to_vec();
    }
    // Whether step `j` belongs to the cascade, given the step `beyond` it.
    let cascade = |j: usize, beyond: Option<usize>| -> bool {
        match steps.get(j) {
            Some(Step::Micro) => true,
            Some(Step::Normal) => beyond.is_some_and(|k| {
                steps.get(k) != Some(&Step::Wall)
                    && matches!(
                        (motions.get(j), motions.get(k)),
                        (Some(m), Some(b)) if m.across <= CASCADE * b.across
                    )
            }),
            _ => false,
        }
    };
    let mut merge = vec![false; steps.len()];
    for (i, step) in steps.iter().enumerate() {
        if *step != Step::Wall {
            continue;
        }
        for j in (0..i).rev() {
            if !cascade(j, j.checked_sub(1)) {
                break;
            }
            if let Some(m) = merge.get_mut(j) {
                *m = true;
            }
        }
        for j in i + 1..steps.len() {
            if !cascade(j, Some(j + 1).filter(|k| *k < steps.len())) {
                break;
            }
            if let Some(m) = merge.get_mut(j) {
                *m = true;
            }
        }
    }
    // Step `j` runs from point `j` to point `j + 1`. Each run of marked steps
    // from `s` to `e` covers points `s..=e + 1`, whose ends are kept.
    let mut out = Vec::with_capacity(points.len());
    if let Some(first) = points.first() {
        out.push(*first);
    }
    let mut j = 0;
    while j < steps.len() {
        if merge.get(j) == Some(&true) {
            let s = j;
            while merge.get(j) == Some(&true) {
                j += 1;
            }
            if let Some(run) = points.get(s..=j) {
                // The run's first point is already in `out`.
                out.extend(thin(rotary, run, chord).into_iter().skip(1));
            }
        } else {
            if let Some(p) = points.get(j + 1) {
                out.push(*p);
            }
            j += 1;
        }
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use openchuck_types::machine::MachineProfile;
    use openchuck_types::units::Degrees;

    fn rotary() -> RotaryMapping {
        RotaryMapping::of_profile(&MachineProfile::example_x_rotary())
    }

    /// A tool over the axis at `x`, `height` above it, at angle `a`.
    fn point(r: &RotaryMapping, x: f64, height: f64, a: f64) -> CutPoint {
        let tip = r.to_model(
            AxisPosition {
                along: x,
                across: 0.0,
                height,
            },
            Degrees(a),
        );
        CutPoint {
            tip,
            a: Degrees(a),
            contact: tip,
        }
    }

    /// The bisection cascade into a climb: steps halving towards A 10, the
    /// last at 0.005 degrees, then the climb from 5 to 8.5 mm.
    fn climb(r: &RotaryMapping) -> Vec<CutPoint> {
        let mut out = vec![point(r, 0.0, 5.0, 0.0), point(r, 0.0, 5.0, 5.0)];
        let mut a = 5.0;
        let mut step = 2.5;
        while step > 0.004 {
            a += step;
            out.push(point(r, 0.0, 5.0, a));
            step /= 2.0;
        }
        out.push(point(r, 0.0, 8.5, 10.0));
        out.push(point(r, 0.0, 8.5, 15.0));
        out
    }

    #[test]
    fn req_op_13_micro_moves_before_a_climb_merge_into_one_move() {
        let r = rotary();
        let points = climb(&r);
        assert!(points.len() > 12);
        let merged = merge_at_walls(&r, &points, 0.01, 0.004);
        // The normal moves stay, the micro-moves become one move to the
        // foot of the wall, and the climb stays.
        let a: Vec<f64> = merged.iter().map(|p| p.a.0).collect();
        let foot = points[points.len() - 3].a.0;
        assert_eq!(a.len(), 5, "{a:?}");
        assert_eq!(a[..2], [0.0, 5.0]);
        assert!(
            (a[2] - foot).abs() < 1e-12 && a[3..] == [10.0, 15.0],
            "{a:?}"
        );
        assert_eq!(wall_breaks(&r, &merged, 0.01), vec![3]);
    }

    #[test]
    fn req_op_13_a_pass_without_a_wall_is_unchanged() {
        let r = rotary();
        let mut points = climb(&r);
        // Without the climb, the cascade is just short moves on a surface.
        points.truncate(points.len() - 2);
        assert_eq!(merge_at_walls(&r, &points, 0.01, 0.004), points);
        assert!(wall_breaks(&r, &points, 0.01).is_empty());
    }

    #[test]
    fn req_op_12_a_merge_keeps_every_dropped_point_within_the_chord() {
        let r = rotary();
        // Micro-moves down a curved corner before a drop: a point the chord
        // cannot drop is kept.
        let mut points = vec![point(&r, 0.0, 9.0, 0.0)];
        for k in 1..=8 {
            let a = 5.0 + 0.001 * f64::from(k);
            let dip = if k == 4 { 0.009 } else { 0.0 };
            points.push(point(&r, 0.0, 9.0 - dip, a));
        }
        points.push(point(&r, 0.0, 5.0, 5.0085));
        let merged = merge_at_walls(&r, &points, 0.01, 0.004);
        assert!(merged.iter().any(|p| (p.a.0 - 5.004).abs() < 1e-12));
        for p in &points {
            let near = merged
                .windows(2)
                .any(|w| deviation(&r, &w[0], &w[1], p) <= 0.004);
            assert!(near, "{p:?}");
        }
    }
}
