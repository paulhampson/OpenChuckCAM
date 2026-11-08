// SPDX-License-Identifier: AGPL-3.0-or-later
//! How a pass starts, told from its points (proposed decision record 0543
//! section 2; issues #639 and #640).
//!
//! An entry that follows the toolpath needs to know whether the pass starts
//! as a line (the tool moves sideways in the machine at a fixed A), as a
//! full ring (A turns a whole turn at a fixed machine X and Y and the tip
//! comes back to where it started), as a spiral (A turns while the tip
//! drifts along the rotary axis), as a partial ring (A turns at a fixed X
//! and Y and stops before a whole turn), or is a single point. The pass
//! carries no flag for this: strategies build rings, spirals and lines as
//! points, and [`super::assemble`] has made A continuous before the entry
//! is placed, so the shape is read from the points in the machine's fixed
//! frame ([`RotaryMapping::to_axis`]).

use crate::rotary::{AxisPosition, RotaryMapping};
use crate::strategy::CutPoint;

/// A change of A smaller than this, in degrees, is no turn.
const A_EPS: f64 = 1e-9;

/// How a pass starts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Start {
    /// One cut point, or points that do not move the tool: nothing to
    /// follow.
    SinglePoint,
    /// The tool moves sideways at a fixed A. `end` is the index of the last
    /// point of the leading run that a ramp may follow: it stops at the
    /// first rapid inside the pass and where the pass starts to turn A.
    Line { end: usize },
    /// A turns a whole turn at a fixed machine X and Y and the tip comes
    /// back to its first position; `end` is the index of the point that
    /// closes the turn. `sign` is the way A turns (+1 or -1).
    FullRing { end: usize, sign: f64 },
    /// A turns at a fixed machine X and Y and stops before a whole turn;
    /// `end` is the index of the last point of that arc.
    PartialRing { end: usize, sign: f64 },
    /// A turns while the tip drifts along the rotary axis.
    Spiral { sign: f64 },
}

/// How the pass of `points` starts. `rapids` are the indices of its points
/// reached by a rapid (`Pass::rapids`), and `tolerance` the path tolerance:
/// positions closer than it are the same position.
pub(super) fn start(
    rotary: &RotaryMapping,
    points: &[CutPoint],
    rapids: &[usize],
    tolerance: f64,
) -> Start {
    let at: Vec<AxisPosition> = points.iter().map(|p| rotary.to_axis(p.tip, p.a)).collect();
    let (Some(first), Some(p0)) = (points.first(), at.first()) else {
        return Start::SinglePoint;
    };
    let sideways = |q: &AxisPosition| (q.along - p0.along).hypot(q.across - p0.across);
    let same =
        |q: &AxisPosition| sideways(q) <= tolerance && (q.height - p0.height).abs() <= tolerance;
    // The first point that moves the tool, sideways, in height or in A.
    let Some(k) = (1..points.len()).find(|&i| {
        at.get(i).is_some_and(|q| !same(q))
            || points
                .get(i)
                .is_some_and(|p| (p.a.0 - first.a.0).abs() > A_EPS)
    }) else {
        return Start::SinglePoint;
    };
    let turned = points.get(k).map_or(0.0, |p| p.a.0 - first.a.0);
    if turned.abs() <= A_EPS {
        // A line: it runs until a rapid or until A starts to turn.
        let mut end = k;
        while end + 1 < points.len()
            && !rapids.contains(&(end + 1))
            && points
                .get(end + 1)
                .zip(points.get(end))
                .is_some_and(|(n, p)| (n.a.0 - p.a.0).abs() <= A_EPS)
        {
            end += 1;
        }
        return Start::Line { end };
    }
    let sign = turned.signum();
    // Follow the turn while A keeps turning the same way at a fixed X and
    // Y, until it has turned a whole turn.
    let mut end = 0;
    for i in 1..points.len() {
        let (Some(p), Some(prev), Some(q)) = (points.get(i), points.get(i - 1), at.get(i)) else {
            break;
        };
        if rapids.contains(&i) || (p.a.0 - prev.a.0) * sign < -A_EPS {
            break;
        }
        if sideways(q) > tolerance {
            // The tip has left the start X and Y while A turns: a spiral,
            // unless it only stepped along the axis after an arc at a fixed
            // place (a ring cut short, then a step).
            if (p.a.0 - prev.a.0).abs() > A_EPS {
                return Start::Spiral { sign };
            }
            break;
        }
        end = i;
        if (p.a.0 - first.a.0).abs() >= 360.0 - A_EPS {
            if same(q) {
                return Start::FullRing { end, sign };
            }
            break;
        }
    }
    Start::PartialRing { end, sign }
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

    fn point(along: f64, height: f64, a: f64) -> CutPoint {
        let tip = rotary().to_model(
            AxisPosition {
                along,
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

    /// A ring at `along`, starting at `a0`, `steps` points a whole turn the
    /// way `sign` says, at a height that varies with the angle (a part that
    /// is not round) and comes back to where it started.
    fn ring(along: f64, a0: f64, sign: f64, steps: usize) -> Vec<CutPoint> {
        (0..=steps)
            .map(|j| {
                let t = j as f64 / steps as f64;
                let height = 12.0 + (t * std::f64::consts::TAU).sin();
                point(along, height, a0 + sign * 360.0 * t)
            })
            .collect()
    }

    #[test]
    fn req_op_11_a_pass_of_one_point_has_nothing_to_follow() {
        let r = rotary();
        assert_eq!(
            start(&r, &[point(0.0, 10.0, 0.0)], &[], 0.01),
            Start::SinglePoint
        );
        let repeated = [point(0.0, 10.0, 0.0), point(0.0, 10.0, 0.0)];
        assert_eq!(start(&r, &repeated, &[], 0.01), Start::SinglePoint);
        assert_eq!(start(&r, &[], &[], 0.01), Start::SinglePoint);
    }

    #[test]
    fn req_op_11_a_line_runs_to_a_rapid_or_to_where_a_starts_to_turn() {
        let r = rotary();
        let mut line: Vec<CutPoint> = (0..5).map(|i| point(f64::from(i), 10.0, 30.0)).collect();
        assert_eq!(start(&r, &line, &[], 0.01), Start::Line { end: 4 });
        assert_eq!(start(&r, &line, &[3], 0.01), Start::Line { end: 2 });
        line.push(point(4.0, 10.0, 40.0));
        assert_eq!(start(&r, &line, &[], 0.01), Start::Line { end: 4 });
    }

    #[test]
    fn req_op_11_a_ring_is_a_whole_turn_back_to_its_first_position_either_way() {
        let r = rotary();
        for sign in [1.0, -1.0] {
            let mut pass = ring(5.0, 20.0, sign, 72);
            assert_eq!(
                start(&r, &pass, &[], 0.01),
                Start::FullRing { end: 72, sign }
            );
            // The step to the next ring does not change how the pass starts.
            pass.push(point(7.0, 12.0, 20.0 + sign * 360.0));
            assert_eq!(
                start(&r, &pass, &[], 0.01),
                Start::FullRing { end: 72, sign }
            );
        }
    }

    #[test]
    fn req_op_11_an_arc_that_stops_before_a_whole_turn_is_a_partial_ring() {
        let r = rotary();
        let arc: Vec<CutPoint> = ring(5.0, 0.0, 1.0, 72).into_iter().take(30).collect();
        assert_eq!(
            start(&r, &arc, &[], 0.01),
            Start::PartialRing { end: 29, sign: 1.0 }
        );
        // Stopped by a rapid inside the pass.
        let whole = ring(5.0, 0.0, 1.0, 72);
        assert_eq!(
            start(&r, &whole, &[10], 0.01),
            Start::PartialRing { end: 9, sign: 1.0 }
        );
        // Stopped by A turning back.
        let mut back: Vec<CutPoint> = ring(5.0, 0.0, -1.0, 72).into_iter().take(10).collect();
        back.push(point(5.0, 12.0, -40.0));
        assert_eq!(
            start(&r, &back, &[], 0.01),
            Start::PartialRing { end: 9, sign: -1.0 }
        );
        // Stopped by a step along the axis.
        let mut stepped: Vec<CutPoint> = ring(5.0, 0.0, 1.0, 72).into_iter().take(10).collect();
        let last_a = stepped.last().unwrap().a.0;
        stepped.push(point(8.0, 12.0, last_a));
        assert_eq!(
            start(&r, &stepped, &[], 0.01),
            Start::PartialRing { end: 9, sign: 1.0 }
        );
    }

    #[test]
    fn req_op_11_a_turn_that_drifts_along_the_axis_is_a_spiral() {
        let r = rotary();
        // 2 mm per turn, 5 degrees a step.
        let spiral: Vec<CutPoint> = (0..200)
            .map(|j| {
                let a = 5.0 * f64::from(j);
                point(2.0 * a / 360.0, 12.0, -a)
            })
            .collect();
        assert_eq!(start(&r, &spiral, &[], 0.01), Start::Spiral { sign: -1.0 });
    }

    #[test]
    fn req_op_11_a_whole_turn_that_does_not_come_back_is_not_a_ring() {
        let r = rotary();
        // A whole turn at a fixed X whose height has dropped by a layer:
        // not closed, so the ramp cannot use its last stretch.
        let open: Vec<CutPoint> = (0..=72)
            .map(|j| point(5.0, 12.0 - f64::from(j) / 72.0, 5.0 * f64::from(j)))
            .collect();
        assert_eq!(
            start(&r, &open, &[], 0.01),
            Start::PartialRing { end: 72, sign: 1.0 }
        );
    }
}
