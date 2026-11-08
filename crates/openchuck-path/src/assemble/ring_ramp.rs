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
//! The descent is limited by the ramp angle on the tip's real path, the arc
//! length at the smallest tip radius on the ring (decision 0543 D4). Where
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
    /// More than [`MAX_TURNS`] turns at the ring's smallest tip radius.
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
    let mut radius = f64::INFINITY;
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
        radius = radius.min(pos.across.hypot(pos.height));
    }
    if stock_top <= head.height + EPS {
        return Err(Refusal::NothingToDescend);
    }
    let top = stock_top.max(base_top) + approach;
    // Arc length needed at the smallest radius, as an angle.
    let run = (top - base_low) / tan;
    let delta = (run / radius.max(EPS)).to_degrees();
    let turns = delta / 360.0;
    if !delta.is_finite() || turns > MAX_TURNS {
        return Err(Refusal::TooManyTurns { radius, turns });
    }
    let whole = (a_end - a0).abs();
    // Offsets back from the first point (degrees) of each ring point, from
    // 0 at the closing point down the ring to `whole` at the first point.
    let back: Vec<f64> = ring.iter().map(|&(_, a)| (a_end - a).abs()).collect();
    // Where the ring is at an offset `phi` back from its first point.
    let at = |phi: f64| -> AxisPosition {
        let phi = phi.rem_euclid(whole);
        // The ring's points by offset: back[k] falls as k rises.
        let k = back
            .iter()
            .position(|&o| o <= phi + 1e-12)
            .unwrap_or(back.len() - 1);
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
        for &o in back.iter() {
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
    // Heights: no faster a fall than the angle from the start (forward),
    // and able to reach the first point (backward); the lower of the two.
    let slope = tan * radius;
    // The samples: offset back from the first point (degrees) and the
    // ring's position there.
    let samples: Vec<(f64, AxisPosition)> = offsets.iter().copied().zip(base).collect();
    let mut forward = Vec::with_capacity(samples.len());
    let mut last: Option<(f64, f64)> = None;
    for &(phi, pos) in &samples {
        let h = match last {
            None => top,
            Some((o, h)) => pos.height.max(h - slope * (o - phi).to_radians()),
        };
        forward.push(h);
        last = Some((phi, h));
    }
    let mut backward = Vec::with_capacity(samples.len());
    let mut last: Option<(f64, f64)> = None;
    for &(phi, pos) in samples.iter().rev() {
        let h = match last {
            None => head.height,
            Some((o, h)) => pos.height.max(h + slope * (phi - o).to_radians()),
        };
        backward.push(h);
        last = Some((phi, h));
    }
    backward.reverse();
    let mut steep = false;
    let mut moves = Vec::with_capacity(samples.len());
    let mut previous: Option<(f64, f64)> = None;
    for ((&(phi, pos), f), b) in samples.iter().zip(&forward).zip(&backward) {
        let height = f.min(*b).min(top);
        if let Some((o, h)) = previous
            && h - height > slope * (o - phi).to_radians() + 1e-9
        {
            steep = true;
        }
        previous = Some((phi, height));
        moves.push((AxisPosition { height, ..pos }, Degrees(a0 - sign * phi)));
    }
    if let Some(last) = moves.last_mut() {
        *last = (head, Degrees(a0));
    }
    Ok(RingRamp {
        moves,
        follows_steep_drop: steep,
    })
}
