// SPDX-License-Identifier: AGPL-3.0-or-later
//! A ramp around a ring, before its first point (proposed decision record
//! 0543 section 2; issue #639).
//!
//! A ring is a closed loop: the stretch before its first point is the
//! ring's own last stretch, which the ring cuts again at full depth when it
//! closes. The ramp follows that stretch, raised along the tool axis, from
//! the approach distance above the highest stock under the ring, turning A
//! the way the pass turns, and arrives at the first cut point at full
//! depth. It never leaves the ring's machine X and Y, so it cannot reach a
//! held region, a tab or the next ring that the ring itself does not.
//!
//! The descent is limited by the ramp angle on the tip's real path: the arc
//! length the tip travels relative to the part, at its own distance from
//! the rotary axis at each step (decision 0543 D4). A ring that reaches the
//! axis (an end face) still has length elsewhere on it. Where
//! the pass itself drops more steeply than that just before its first
//! point, the ramp follows the pass's own drop there (D4, the owner's
//! answer pending); everywhere else it stays on or above the angle.

use openchuck_types::units::Degrees;

use super::{Builder, EPS, View};
use crate::rotary::AxisPosition;
use crate::strategy::CutPoint;

/// The most turns a ring ramp may make (decision 0543 D4).
pub(super) const MAX_TURNS: f64 = 10.0;

/// A planned ring ramp: the positions (machine frame) and angles of its
/// moves, the first being where the descent starts and the last the pass's
/// first cut point.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct RingRamp {
    pub moves: Vec<(AxisPosition, Degrees)>,
    /// True when, just before the first point, the ramp follows a drop of
    /// the pass steeper than the ramp angle.
    pub follows_steep_drop: bool,
    /// The turns of A of a descent on the rotary axis (decision 0543
    /// section 4, point 2): `Some` when the tip stays on the axis and the
    /// moves are plunges while A turns.
    pub on_axis: Option<f64>,
    /// The angle used, degrees, when the steeper ramp near the axis made it
    /// steeper than the Ramp angle (decision 0543 section 4, point 3).
    pub steepened: Option<f64>,
}

impl RingRamp {
    /// Where the descent starts.
    pub fn start(&self) -> Option<(AxisPosition, Degrees)> {
        self.moves.first().copied()
    }
}

/// Why a ring cannot be ramped round.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Refusal {
    /// More than [`MAX_TURNS`] turns; `radius` is the tip's mean distance
    /// from the rotary axis round the ring.
    TooManyTurns { radius: f64, turns: f64 },
    /// The pass starts above the stock: nothing to ramp down into.
    NothingToDescend,
}

/// Plans the ramp round the ring `points[..=end]` (a closed ring that turns
/// A the way `sign` says), against `stock`, from the approach distance
/// above the highest stock under the ring. `probe_height` is a height above
/// all the stock (the retract height), from which it is probed.
#[allow(clippy::too_many_arguments)]
pub(super) fn plan(
    b: &Builder<'_>,
    stock: View<'_>,
    approach: f64,
    tan: f64,
    points: &[CutPoint],
    end: usize,
    sign: f64,
    probe_height: f64,
) -> Result<RingRamp, Refusal> {
    let ring: Vec<(AxisPosition, f64)> = points
        .iter()
        .take(end + 1)
        .map(|p| (b.rotary.to_axis(p.tip, p.a), p.a.0))
        .collect();
    let (Some(&(head, a0)), Some(&(_, a_end))) = (ring.first(), ring.last()) else {
        return Err(Refusal::NothingToDescend);
    };
    // The highest stock under the tool anywhere round the ring.
    let mut stock_top = f64::NEG_INFINITY;
    let mut base_top = f64::NEG_INFINITY;
    let mut base_low = f64::INFINITY;
    // The tip's mean distance from the axis round the ring, by angle.
    let mut swept = 0.0;
    let mut turned = 0.0;
    for &(pos, a) in &ring {
        let over = AxisPosition {
            height: probe_height,
            ..pos
        };
        let travel = (probe_height - pos.height).max(0.0);
        if let Some(d) = super::footprint_entry(b, stock, over, Degrees(a), probe_height, travel) {
            stock_top = stock_top.max(probe_height - d);
        }
        base_top = base_top.max(pos.height);
        base_low = base_low.min(pos.height);
    }
    for w in ring.windows(2) {
        if let [(p, a), (q, b)] = w {
            let da = (b - a).abs();
            swept += da * 0.5 * (p.across.hypot(p.height) + q.across.hypot(q.height));
            turned += da;
        }
    }
    let radius = if turned > 0.0 { swept / turned } else { 0.0 };
    if stock_top <= head.height + EPS {
        return Err(Refusal::NothingToDescend);
    }
    let top = stock_top.max(base_top) + approach;
    // Arc length needed at the mean radius, as an angle; the loop below adds
    // turns until the ramp, measured on its real path, starts at `top`.
    let run = (top - base_low) / tan;
    let mut delta = (run / radius.max(EPS)).to_degrees();
    let whole = (a_end - a0).abs();
    // Offsets back from the first point (degrees) of each ring point, from
    // 0 at the closing point down the ring to `whole` at the first point.
    let back: Vec<f64> = ring.iter().map(|&(_, a)| (a_end - a).abs()).collect();
    // Where the pass rises just before its first point the ramp needs more
    // than `delta` to start at `top`: a turn more each time.
    loop {
        let turns = delta / 360.0;
        if !delta.is_finite() || turns > MAX_TURNS {
            return Err(Refusal::TooManyTurns { radius, turns });
        }
        let planned = round(&ring, &back, whole, head, a0, sign, delta, top, tan);
        if planned
            .moves
            .first()
            .is_some_and(|(p, _)| p.height >= top - 1e-9)
        {
            return Ok(planned);
        }
        delta += whole;
    }
}

/// The ramp `delta` degrees round the ring.
#[allow(clippy::too_many_arguments)]
fn round(
    ring: &[(AxisPosition, f64)],
    back: &[f64],
    whole: f64,
    head: AxisPosition,
    a0: f64,
    sign: f64,
    delta: f64,
    top: f64,
    tan: f64,
) -> RingRamp {
    // Where the ring is at an offset `phi` back from its first point.
    let at = |phi: f64| -> AxisPosition {
        let phi = phi.rem_euclid(whole);
        // The ring's points by offset: back[k] falls as k rises.
        let k = back
            .iter()
            .position(|&o| o <= phi + 1e-12)
            .unwrap_or(back.len().saturating_sub(1));
        let (Some(&(p1, _)), Some(&o1)) = (ring.get(k), back.get(k)) else {
            return head;
        };
        let Some(j) = k.checked_sub(1) else {
            return p1;
        };
        let (Some(&(p0, _)), Some(&o0)) = (ring.get(j), back.get(j)) else {
            return p1;
        };
        let f = if o0 - o1 > 1e-12 {
            (phi - o1) / (o0 - o1)
        } else {
            0.0
        };
        AxisPosition {
            along: p1.along + f * (p0.along - p1.along),
            across: p1.across + f * (p0.across - p1.across),
            height: p1.height + f * (p0.height - p1.height),
        }
    };
    // Sample offsets: the ring's own points in every turn the ramp covers,
    // and the start, from the start down to 0.
    let mut offsets = vec![delta];
    let mut lap = (delta / whole).floor();
    while lap >= 0.0 {
        for &o in back {
            let phi = lap * whole + o;
            if phi < delta - 1e-9 && phi > 1e-12 {
                offsets.push(phi);
            }
        }
        lap -= 1.0;
    }
    offsets.push(0.0);
    offsets.sort_by(|x, y| y.total_cmp(x));
    offsets.dedup_by(|x, y| (*x - *y).abs() < 1e-9);
    let base: Vec<AxisPosition> = offsets.iter().map(|&phi| at(phi)).collect();
    let travel = real_travel(&offsets, &base);
    let heights: Vec<f64> = base.iter().map(|p| p.height).collect();
    let (h, steep) = super::path_ramp::heights(&travel, &heights, top, tan);
    let mut moves: Vec<(AxisPosition, Degrees)> = base
        .iter()
        .zip(&offsets)
        .zip(h)
        .map(|((&pos, &phi), height)| (AxisPosition { height, ..pos }, Degrees(a0 - sign * phi)))
        .collect();
    if let Some(last) = moves.last_mut() {
        *last = (head, Degrees(a0));
    }
    RingRamp {
        moves,
        follows_steep_drop: steep,
        on_axis: None,
        steepened: None,
    }
}

/// Plans the ramp of a spiral (decision 0543 D2 (a)): round the rotary axis
/// at the spiral's start X and Y, before its first point, turning A the way
/// `sign` says, from the approach distance above the highest stock there to
/// the first cut point `head` at `a0`. `base` gives the height the ramp may
/// not go under at an angle: the higher of the first point's height and
/// where the tool meets the finished surface there. `step` is the largest A
/// step between positions, degrees. Unlike a ring's, this stretch is not the
/// pass's own path, so the gouge check on every position stays the proof.
#[allow(clippy::too_many_arguments)]
pub(super) fn plan_spiral(
    b: &Builder<'_>,
    stock: View<'_>,
    approach: f64,
    tan: f64,
    head: AxisPosition,
    a0: f64,
    sign: f64,
    probe_height: f64,
    step: f64,
    base: impl Fn(f64) -> f64,
) -> Result<RingRamp, Refusal> {
    // The stock round the axis at this X, probed every `step` degrees of a
    // whole turn (the ramp covers at most whole turns of the same place).
    let count = ((360.0 / step.max(1e-3)).ceil() as usize).clamp(4, 100_000);
    let mut stock_top = f64::NEG_INFINITY;
    let mut low = head.height;
    for k in 0..count {
        let a = a0 - sign * 360.0 * k as f64 / count as f64;
        let over = AxisPosition {
            height: probe_height,
            ..head
        };
        let travel = (probe_height - head.height).max(0.0);
        if let Some(d) = super::footprint_entry(b, stock, over, Degrees(a), probe_height, travel) {
            stock_top = stock_top.max(probe_height - d);
        }
        low = low.min(base(a));
    }
    let radius = head.across.hypot(head.height);
    if stock_top <= head.height + EPS {
        return Err(Refusal::NothingToDescend);
    }
    let top = stock_top + approach;
    let mut delta = ((top - low) / tan / radius.max(EPS)).to_degrees();
    loop {
        let turns = delta / 360.0;
        if !delta.is_finite() || turns > MAX_TURNS {
            return Err(Refusal::TooManyTurns { radius, turns });
        }
        let n = ((delta / step.max(1e-3)).ceil() as usize).max(1);
        let offsets: Vec<f64> = (0..=n)
            .map(|k| delta * (1.0 - k as f64 / n as f64))
            .collect();
        let bases: Vec<f64> = offsets
            .iter()
            .map(|&phi| {
                if phi <= 0.0 {
                    head.height
                } else {
                    base(a0 - sign * phi).max(head.height)
                }
            })
            .collect();
        let positions: Vec<AxisPosition> = bases
            .iter()
            .map(|&height| AxisPosition { height, ..head })
            .collect();
        let travel = real_travel(&offsets, &positions);
        let (h, steep) = super::path_ramp::heights(&travel, &bases, top, tan);
        if h.first().is_some_and(|&h0| h0 >= top - 1e-9) {
            let mut moves: Vec<(AxisPosition, Degrees)> = offsets
                .iter()
                .zip(h)
                .map(|(&phi, height)| (AxisPosition { height, ..head }, Degrees(a0 - sign * phi)))
                .collect();
            if let Some(last) = moves.last_mut() {
                *last = (head, Degrees(a0));
            }
            return Ok(RingRamp {
                moves,
                follows_steep_drop: steep,
                on_axis: None,
                steepened: None,
            });
        }
        delta += 360.0;
    }
}

/// The tip's travel relative to the part from the first of `offsets`
/// (degrees back from the first cut point, falling) at each sample, on the
/// path through `positions`: at each step, the arc at the step's mean
/// distance from the rotary axis and the change of position across the
/// machine (decision 0543 D4).
fn real_travel(offsets: &[f64], positions: &[AxisPosition]) -> Vec<f64> {
    let mut out = Vec::with_capacity(offsets.len());
    let mut total = 0.0;
    let mut previous: Option<(f64, AxisPosition)> = None;
    for (&phi, &p) in offsets.iter().zip(positions) {
        if let Some((o, q)) = previous {
            let r = 0.5 * (p.across.hypot(p.height) + q.across.hypot(q.height));
            total +=
                (o - phi).abs().to_radians() * r + (p.along - q.along).hypot(p.across - q.across);
        }
        out.push(total);
        previous = Some((phi, p));
    }
    out
}

/// Plans the descent of a pass that starts on the rotary axis (an end-face
/// ring; decision 0543 section 4, point 2, the owner's option (b)): the tip
/// moves straight down from the approach distance above the stock to the
/// first cut point `head`, at `a0`, while A turns the way `sign` says, as
/// many turns as a ramp at `tan` would take round a ring at the tool's
/// cutting radius `tool_radius`. `step` is the largest A step, degrees.
#[allow(clippy::too_many_arguments)]
pub(super) fn plan_on_axis(
    b: &Builder<'_>,
    stock: View<'_>,
    approach: f64,
    tan: f64,
    head: AxisPosition,
    a0: f64,
    sign: f64,
    probe_height: f64,
    tool_radius: f64,
    step: f64,
) -> Result<RingRamp, Refusal> {
    let over = AxisPosition {
        height: probe_height,
        ..head
    };
    let travel = (probe_height - head.height).max(0.0);
    let mut stock_top = f64::NEG_INFINITY;
    for k in 0..36 {
        let a = Degrees(a0 - sign * 10.0 * f64::from(k));
        if let Some(d) = super::footprint_entry(b, stock, over, a, probe_height, travel) {
            stock_top = stock_top.max(probe_height - d);
        }
    }
    if stock_top <= head.height + EPS {
        return Err(Refusal::NothingToDescend);
    }
    let rise = stock_top + approach - head.height;
    let turns = rise / (tan * std::f64::consts::TAU * tool_radius.max(EPS));
    if !turns.is_finite() || turns > MAX_TURNS {
        return Err(Refusal::TooManyTurns {
            radius: tool_radius,
            turns,
        });
    }
    let delta = 360.0 * turns;
    let n = ((delta / step.max(1e-3)).ceil() as usize).max(1);
    let moves = (0..=n)
        .map(|k| {
            let f = k as f64 / n as f64;
            (
                AxisPosition {
                    height: head.height + rise * (1.0 - f),
                    ..head
                },
                Degrees(a0 - sign * delta * (1.0 - f)),
            )
        })
        .collect();
    Ok(RingRamp {
        moves,
        follows_steep_drop: false,
        on_axis: Some(turns),
        steepened: None,
    })
}
