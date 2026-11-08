// SPDX-License-Identifier: AGPL-3.0-or-later
//! Entry and exit moves (OP-11, OP-14; decision record 0026).
//!
//! A pass starts with an entry and, for the `lead` style, ends with an exit.
//! They are placed here, with the A-axis management, clearance, collision,
//! feed and timing rules of [`super::assemble`], so a strategy never writes
//! them; the operation's `entry` parameters choose the style.
//!
//! - **Plunge**: a rapid to the approach distance above the stock, then a
//!   straight plunge at the plunge feed (decision record 0018 item 9).
//! - **Lead**: down beside the cut, then a quarter-circle tangent to the cut
//!   at the cutting feed (`LeadIn`), and the mirror image after the last
//!   point (`LeadOut`).
//! - **Ramp**: a straight descent at the ramp angle along the line of the
//!   cut, ending at the first cut point (`LeadIn` at the plunge feed).
//! - **Helix**: a circle the tool centre follows downwards, ending at the
//!   first cut point and tangent to the cut (`LeadIn` at the plunge feed).
//!
//! A does not move during a ramp or helix, so the entry lies in the plane of
//! the pass's start angle and cannot add a wrap or reversal (OP-15). Every
//! entry is checked against the finished surface, with the stock to leave
//! (the tool may not come closer than the path tolerance allows) and, for the
//! descent to a ramp's start, against the stock model. An entry that does not
//! fit is refused with a message that names the style and the size it needs;
//! it never changes style on its own.

use std::f64::consts::{FRAC_PI_2, TAU};

use openchuck_types::ir::MoveKind;
use openchuck_types::tool::profile::RevolvedProfile;
use openchuck_types::units::Degrees;

use super::pass_shape::{self, Start};
use super::ring_ramp::{self, RingRamp};
use super::{Builder, EPS, View};
use crate::cutter::{self, ToolLine};
use crate::error::{PathError, Result};
use crate::helical::CHORD_FACTOR;
use crate::parallel;
use crate::params::{CommonParameters, EntryParameters, EntryStyle};
use crate::rotary::AxisPosition;
use crate::strategy::{CutPoint, StrategyInput};

/// The most segments one entry may take.
const MAX_SEGMENTS: usize = 100_000;

/// A horizontal direction in the fixed frame: (along, across).
type Dir = (f64, f64);

pub(super) struct Entry {
    style: EntryStyle,
    /// A single-point pass under a ramp is plunged (decision 0543 D7).
    single_point_plunge: bool,
    /// A ring or spiral ramp past 10 turns uses a steeper angle (decision
    /// 0543 section 4, point 3).
    steeper_near_axis: bool,
    /// The tangent of the steepest angle it may use.
    tan_max: f64,
    ramp_tan: f64,
    helix_radius: f64,
    lead_radius: f64,
    leave: f64,
    tolerance: f64,
    chord: f64,
    /// The profile grown by the stock to leave; `None` for a plain plunge.
    grown: Option<RevolvedProfile>,
    /// A height above all the stock (the retract height), from which the
    /// stock under a ramp's path is probed.
    probe_height: f64,
}

impl Entry {
    pub(super) fn new(
        input: &StrategyInput<'_>,
        common: &CommonParameters,
        e: &EntryParameters,
    ) -> Result<Self> {
        let radius = input.tool.cutting_radius();
        let leave = common.stock_to_leave.0;
        let grown = match e.style {
            EntryStyle::Plunge => None,
            _ => Some(cutter::grow_profile(input.tool, leave)?),
        };
        Ok(Self {
            style: e.style,
            single_point_plunge: e.allow_single_point_plunge,
            steeper_near_axis: e.steeper_near_axis,
            tan_max: e.ramp_angle_max.0.max(e.ramp_angle.0).to_radians().tan(),
            ramp_tan: e.ramp_angle.0.to_radians().tan(),
            helix_radius: e.helix_diameter * radius,
            lead_radius: if e.lead_radius.0 > 0.0 {
                e.lead_radius.0
            } else {
                radius
            },
            leave,
            tolerance: common.tolerance.get().0,
            chord: CHORD_FACTOR * common.tolerance.get().0,
            grown,
            probe_height: match common.retract {
                crate::params::Retract::Clearance => input.clearance.radius.0,
                crate::params::Retract::SafeZ => input.clearance.safe_z.0,
            },
        })
    }

    /// Moves from above the pass's first point to that point.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn enter(
        &self,
        b: &mut Builder<'_>,
        input: &StrategyInput<'_>,
        stock: View<'_>,
        common: &CommonParameters,
        pass: usize,
        points: &[CutPoint],
        rapids: &[usize],
    ) -> Result<()> {
        let Some(head) = points.first() else {
            return Ok(());
        };
        if let Some(plan) = self.ring_plan(b, input, stock, common, pass, points, rapids)? {
            return self.ring_ramp(b, input, common, pass, &plan);
        }
        match self.style {
            EntryStyle::Plunge => plunge(b, stock, common, head),
            EntryStyle::Lead => self.lead_in(b, input, stock, common, pass, head, points),
            EntryStyle::Ramp => self.ramp(b, input, stock, common, pass, head, points, rapids),
            EntryStyle::Helix => self.helix(b, input, stock, common, pass, head, points),
        }
    }

    /// Moves from the pass's last point; only the `lead` style has one.
    pub(super) fn exit(
        &self,
        b: &mut Builder<'_>,
        common: &CommonParameters,
        input: &StrategyInput<'_>,
        pass: usize,
        points: &[CutPoint],
    ) -> Result<()> {
        if self.style != EntryStyle::Lead {
            return Ok(());
        }
        let Some(last) = points.last() else {
            return Ok(());
        };
        let name = "lead-out";
        let end = b.rotary.to_axis(last.tip, last.a);
        // A pass of one cut point was entered by a plunge and has nothing to
        // lead out along (decision 0204).
        let Some(d) = direction_at_end(b, points) else {
            return Ok(());
        };
        let r = self.lead_radius;
        let offset = last.contact - last.tip;
        let mut refusal = None;
        for side in [1.0, -1.0] {
            let n = (-d.1 * side, d.0 * side);
            let centre = (end.along + r * n.0, end.across + r * n.1);
            let at = |phi: f64| AxisPosition {
                along: centre.0 - r * n.0 * phi.cos() + r * d.0 * phi.sin(),
                across: centre.1 - r * n.1 * phi.cos() + r * d.1 * phi.sin(),
                height: end.height,
            };
            let m = self.arc_segments(r);
            let positions: Vec<AxisPosition> = (1..=m)
                .map(|j| at(FRAC_PI_2 * j as f64 / m as f64))
                .collect();
            if let Some(g) = self.gouge(b, input, last.a, &positions) {
                refusal.get_or_insert(g);
                continue;
            }
            b.contact = last.contact;
            for pos in &positions {
                let tip = b.rotary.to_model(*pos, last.a);
                let pose = b.pose(tip, last.a);
                b.push(
                    MoveKind::LeadOut,
                    pose,
                    Some(tip + offset),
                    Some(common.feed),
                )?;
            }
            return Ok(());
        }
        Err(self.refuse(
            input,
            pass,
            name,
            &format!("an arc of radius {r:.3} mm"),
            refusal.unwrap_or_default(),
        ))
    }

    #[allow(clippy::too_many_arguments)]
    fn lead_in(
        &self,
        b: &mut Builder<'_>,
        input: &StrategyInput<'_>,
        stock: View<'_>,
        common: &CommonParameters,
        pass: usize,
        head: &CutPoint,
        points: &[CutPoint],
    ) -> Result<()> {
        let name = "lead-in";
        let goal = b.rotary.to_axis(head.tip, head.a);
        // A pass of one cut point (the part's tip just reaches this layer) has
        // no line to lead along: it is entered by a plunge (issue 199,
        // decision 0204).
        let Some(d) = direction_at_start(b, points) else {
            return plunge(b, stock, common, head);
        };
        let r = self.lead_radius;
        let offset = head.contact - head.tip;
        let mut refusal = None;
        for side in [1.0, -1.0] {
            let n = (-d.1 * side, d.0 * side);
            let centre = (goal.along + r * n.0, goal.across + r * n.1);
            let at = |phi: f64| AxisPosition {
                along: centre.0 - r * n.0 * phi.cos() - r * d.0 * phi.sin(),
                across: centre.1 - r * n.1 * phi.cos() - r * d.1 * phi.sin(),
                height: goal.height,
            };
            let m = self.arc_segments(r);
            let positions: Vec<AxisPosition> = (0..m)
                .map(|j| at(FRAC_PI_2 * (1.0 - j as f64 / m as f64)))
                .collect();
            if let Some(g) = self.gouge(b, input, head.a, &positions) {
                refusal.get_or_insert(g);
                continue;
            }
            // Down beside the cut, then round into it.
            let Some(start) = positions.first().copied() else {
                continue;
            };
            // The move across at the link's height is checked against
            // the stock along its whole line first (issue #642).
            super::clear_traverse(
                b,
                stock,
                start,
                head.a,
                common.approach_distance.0,
                self.probe_height,
            )?;
            let here = b.at();
            let above = AxisPosition {
                height: here.height,
                ..start
            };
            let tip = b.rotary.to_model(above, head.a);
            let pose = b.pose(tip, head.a);
            b.push(MoveKind::Rapid, pose, None, None)?;
            let stop = stop_height(b, stock, common, start.height);
            rapid_to(b, stop)?;
            let tip = b.rotary.to_model(start, head.a);
            let pose = b.pose(tip, head.a);
            b.push(MoveKind::Plunge, pose, None, Some(common.plunge_feed))?;
            b.contact = tip + offset;
            for pos in positions.iter().skip(1) {
                let tip = b.rotary.to_model(*pos, head.a);
                let pose = b.pose(tip, head.a);
                b.push(
                    MoveKind::LeadIn,
                    pose,
                    Some(tip + offset),
                    Some(common.feed),
                )?;
            }
            let pose = b.pose(head.tip, head.a);
            return b.push(
                MoveKind::LeadIn,
                pose,
                Some(head.contact),
                Some(common.feed),
            );
        }
        Err(self.refuse(
            input,
            pass,
            name,
            &format!("an arc of radius {r:.3} mm"),
            refusal.unwrap_or_default(),
        ))
    }

    /// A ramp that follows the pass's own toolpath (decision 0543): over a
    /// line or a partial ring, back and forth; a pass that starts as a ring
    /// or a spiral is ramped round the axis before it reaches here
    /// ([`Self::ring_plan`]), and comes here only when it starts above the
    /// stock, with nothing to descend into.
    #[allow(clippy::too_many_arguments)]
    fn ramp(
        &self,
        b: &mut Builder<'_>,
        input: &StrategyInput<'_>,
        stock: View<'_>,
        common: &CommonParameters,
        pass: usize,
        head: &CutPoint,
        points: &[CutPoint],
        rapids: &[usize],
    ) -> Result<()> {
        let shape = pass_shape::start(&b.rotary, points, rapids, self.tolerance);
        // A pass that turns A with its tip on the rotary axis does not move
        // relative to the part: like a single point, it has no path to
        // follow (decision 0543 D7).
        let on_axis = matches!(shape, Start::FullRing { .. } | Start::Spiral { .. }) && {
            let at = b.rotary.to_axis(head.tip, head.a);
            at.across.hypot(at.height) <= self.tolerance
        };
        match shape {
            // Reached only when there is nothing to descend into
            // (`ring_plan` plans the descent on the axis otherwise).
            _ if on_axis => plunge(b, stock, common, head),
            Start::SinglePoint => self.single_point(b, stock, common, pass, head),
            Start::FullRing { .. } | Start::Spiral { .. } => plunge(b, stock, common, head),
            Start::Line { end } => {
                // A line shorter than the minimum (the step before a ring,
                // say) is followed on along the pass, up to its first rapid,
                // so the ramp goes back and forth over the pass's own path
                // past it, A reversing during the entry only (decision 0543
                // D3).
                let path: Vec<(AxisPosition, Degrees)> = points
                    .iter()
                    .take(end + 1)
                    .map(|p| (b.rotary.to_axis(p.tip, p.a), p.a))
                    .collect();
                if travel_of(&path) < input.tool.cutting_radius() {
                    let last = (1..points.len())
                        .find(|k| rapids.contains(k))
                        .map_or(points.len().saturating_sub(1), |k| k - 1);
                    self.follow(b, input, stock, common, pass, points, last.max(end), true)
                } else {
                    self.follow(b, input, stock, common, pass, points, end, false)
                }
            }
            Start::PartialRing { end, .. } => {
                self.follow(b, input, stock, common, pass, points, end, true)
            }
        }
    }

    /// A pass with no path to follow: refused unless Allow single-point
    /// plunge is on, then plunged and named in a note (decision 0543 D7, the
    /// owner's answer).
    fn single_point(
        &self,
        b: &mut Builder<'_>,
        stock: View<'_>,
        common: &CommonParameters,
        pass: usize,
        head: &CutPoint,
    ) -> Result<()> {
        if !self.single_point_plunge {
            return Err(PathError::Strategy(format!(
                "Ramp entry: pass {} is a single point, so there is no path for the ramp to \
                 follow. Turn on Allow single-point plunge, or set Entry to Plunge or Helix.",
                pass + 1
            )));
        }
        b.single_point_plunges.push(pass);
        plunge(b, stock, common, head)
    }

    /// The back-and-forth ramp over the pass's own path up to `end`, from
    /// the approach distance above the highest stock under the stretch it
    /// uses (decision 0543 D3, D4, D5).
    #[allow(clippy::too_many_arguments)]
    fn follow(
        &self,
        b: &mut Builder<'_>,
        input: &StrategyInput<'_>,
        stock: View<'_>,
        common: &CommonParameters,
        pass: usize,
        points: &[CutPoint],
        end: usize,
        arc: bool,
    ) -> Result<()> {
        let path: Vec<(AxisPosition, Degrees)> = points
            .iter()
            .take(end + 1)
            .map(|p| (b.rotary.to_axis(p.tip, p.a), p.a))
            .collect();
        let Some(&(goal, a0)) = path.first() else {
            return Ok(());
        };
        let head = CutPoint {
            tip: b.rotary.to_model(goal, a0),
            a: a0,
            contact: b.rotary.to_model(goal, a0),
        };
        let low = path
            .iter()
            .map(|(p, _)| p.height)
            .fold(f64::INFINITY, f64::min);
        let total = travel_of(&path);
        let approach = common.approach_distance.0;
        // The stock over the first point: none means nothing to descend into.
        let here = stock_top_along(b, stock, &path, 0.0, self.probe_height);
        if here <= goal.height + EPS {
            return plunge(b, stock, common, &head);
        }
        // D5: the highest stock under the stretch the ramp uses, which
        // depends on how high it starts: a few rounds settle it.
        let mut top = here + approach;
        for _ in 0..8 {
            let reach = ((top - low) / self.ramp_tan).min(total);
            let next = stock_top_along(b, stock, &path, reach, self.probe_height) + approach;
            if next <= top + EPS {
                break;
            }
            top = next;
        }
        let minimum = input.tool.cutting_radius();
        let (moves, steep) = super::path_ramp::back_and_forth(&path, top, self.ramp_tan, minimum)
            .map_err(|refusal| {
            let what = if arc { "stretch" } else { "line" };
            PathError::Strategy(match refusal {
                super::path_ramp::Refusal::TooShort { length } => format!(
                    "Ramp entry: pass {} starts with a {what} of {length:.3} mm, shorter than the \
                     {minimum:.3} mm (the tool radius) a ramp needs to go back and forth over. \
                     Set Entry to Plunge or Helix.",
                    pass + 1
                ),
                super::path_ramp::Refusal::TooManyLegs { legs } => format!(
                    "Ramp entry: pass {} would need {legs} legs to ramp down its {what} of \
                     {total:.3} mm, more than the {} allowed. Raise Ramp angle, or set Entry to \
                     Plunge or Helix.",
                    pass + 1,
                    super::path_ramp::MAX_LEGS
                ),
            })
        })?;
        self.emit(b, input, common, pass, &moves, MoveKind::LeadIn)?;
        if steep {
            b.steep_ramps.push(pass);
        }
        if arc {
            let reversals = moves
                .windows(3)
                .filter(|w| match w {
                    [p, q, r] => (q.1.0 - p.1.0) * (r.1.0 - q.1.0) < 0.0,
                    _ => false,
                })
                .count();
            if reversals > 0 {
                b.notes.push(
                    crate::notice::Notice::information(format!(
                        "pass {} starts with too short a stretch to ramp along without turning \
                         the part, so its ramp goes back and forth over the pass's own path, \
                         reversing A {reversals} times during the entry",
                        pass + 1
                    ))
                    .about(crate::notice::topic::RAMP_ENTRY),
                );
            }
        }
        Ok(())
    }

    /// Emits the moves of a ramp that follows the toolpath: up to its start
    /// if the tool is lower, to its start's angle and position at the
    /// tool's height, down to the start, then along it. Every position is
    /// checked against the finished surface first: on the pass's own path
    /// this is defensive (decision 0543 section 3).
    fn emit(
        &self,
        b: &mut Builder<'_>,
        input: &StrategyInput<'_>,
        common: &CommonParameters,
        pass: usize,
        moves: &[(AxisPosition, Degrees)],
        kind: MoveKind,
    ) -> Result<()> {
        let Some(&(start, a)) = moves.first() else {
            return Ok(());
        };
        let found = moves
            .iter()
            .find_map(|(pos, a)| self.gouge(b, input, *a, std::slice::from_ref(pos)));
        if let Some(g) = found {
            return Err(PathError::Strategy(format!(
                "Ramp entry: pass {} ramps over its own path, yet the tool would cut {:.3} mm \
                 into the finished surface at X {:.3} mm. This should not happen: please report \
                 it as a defect, and set Entry to Plunge or Helix meanwhile.",
                pass + 1,
                g.depth,
                g.x
            )));
        }
        if start.height > b.at().height + EPS {
            b.retract(start.height)?;
        }
        let height = b.at().height;
        let over = b.rotary.to_model(AxisPosition { height, ..start }, a);
        let pose = b.pose(over, a);
        b.push(MoveKind::Rapid, pose, None, None)?;
        rapid_to(b, start.height)?;
        for (pos, a) in moves.iter().skip(1) {
            let tip = b.rotary.to_model(*pos, *a);
            let pose = b.pose(tip, *a);
            b.push(kind, pose, None, Some(common.plunge_feed))?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn helix(
        &self,
        b: &mut Builder<'_>,
        input: &StrategyInput<'_>,
        stock: View<'_>,
        common: &CommonParameters,
        pass: usize,
        head: &CutPoint,
        points: &[CutPoint],
    ) -> Result<()> {
        let name = "helix";
        let goal = b.rotary.to_axis(head.tip, head.a);
        let top = stop_height(b, stock, common, goal.height);
        let rise = top - goal.height;
        if rise <= EPS {
            return plunge(b, stock, common, head);
        }
        // Any tangent will do when the pass has no direction.
        let d = direction_at_start(b, points).unwrap_or((1.0, 0.0));
        let n = (-d.1, d.0);
        let rho = self.helix_radius;
        let per_turn = TAU * rho * self.ramp_tan;
        let turns = (rise / per_turn).ceil().max(1.0);
        let total = TAU * turns;
        let step = self.angle_step(rho);
        let count = (total / step).ceil().max(1.0);
        if count > MAX_SEGMENTS as f64 {
            return Err(PathError::Strategy(format!(
                "pass {}: the helix entry needs {count:.0} segments for {turns:.0} turns; use a \
                 larger ramp angle or a larger helix diameter",
                pass + 1
            )));
        }
        let count = count as usize;
        let centre = (goal.along + rho * n.0, goal.across + rho * n.1);
        let at = |j: usize| {
            let f = j as f64 / count as f64;
            let phi = total * (1.0 - f);
            AxisPosition {
                along: centre.0 - rho * n.0 * phi.cos() - rho * d.0 * phi.sin(),
                across: centre.1 - rho * n.1 * phi.cos() - rho * d.1 * phi.sin(),
                height: goal.height + rise * (1.0 - f),
            }
        };
        let positions: Vec<AxisPosition> = (1..count).map(at).collect();
        if let Some(g) = self.gouge(b, input, head.a, &positions) {
            return Err(self.refuse(
                input,
                pass,
                name,
                &format!(
                    "a circle {:.3} mm across and {turns:.0} turns to descend {rise:.3} mm at \
                     {:.2} degrees",
                    2.0 * rho,
                    self.ramp_tan.atan().to_degrees()
                ),
                g,
            ));
        }
        rapid_to(b, top)?;
        for pos in &positions {
            let tip = b.rotary.to_model(*pos, head.a);
            let pose = b.pose(tip, head.a);
            b.push(MoveKind::LeadIn, pose, None, Some(common.plunge_feed))?;
        }
        let pose = b.pose(head.tip, head.a);
        b.push(MoveKind::LeadIn, pose, None, Some(common.plunge_feed))
    }

    /// The ramp round the ring the pass starts with, when the entry is a
    /// ramp and the pass starts as a closed ring (decision 0543 section 2);
    /// `None` otherwise.
    #[allow(clippy::too_many_arguments)]
    fn ring_plan(
        &self,
        b: &Builder<'_>,
        input: &StrategyInput<'_>,
        stock: View<'_>,
        common: &CommonParameters,
        pass: usize,
        points: &[CutPoint],
        rapids: &[usize],
    ) -> Result<Option<RingRamp>> {
        if self.style != EntryStyle::Ramp {
            return Ok(None);
        }
        let shape = pass_shape::start(&b.rotary, points, rapids, self.tolerance);
        if let (Some(head), Start::FullRing { sign, .. } | Start::Spiral { sign }) =
            (points.first(), shape)
        {
            let at = b.rotary.to_axis(head.tip, head.a);
            if at.across.hypot(at.height) <= self.tolerance {
                // An end-face ring: the tip descends on the axis while A
                // turns (decision 0543 section 4, point 2, option (b)).
                let radius = input.tool.cutting_radius();
                let (planned, tan, at_max) = self.steepen(|tan| {
                    ring_ramp::plan_on_axis(
                        b,
                        stock,
                        common.approach_distance.0,
                        tan,
                        at,
                        head.a.0,
                        sign,
                        self.probe_height,
                        radius,
                        self.angle_step(radius).to_degrees(),
                    )
                });
                return match planned {
                    Ok(plan) => self.in_a_range(input, pass, plan).map(Some),
                    Err(ring_ramp::Refusal::NothingToDescend) => Ok(None),
                    Err(ring_ramp::Refusal::TooManyTurns { turns, .. }) => {
                        Err(PathError::Strategy(format!(
                            "Ramp entry: pass {} starts on the rotary axis, where the tool \
                             descends while the part turns; {}, measured at the tool's radius \
                             of {radius:.3} mm, that takes {turns:.1} turns, more than the {:.0} \
                             allowed. {}",
                            pass + 1,
                            angle_words(tan, at_max),
                            ring_ramp::MAX_TURNS,
                            fix_words(at_max)
                        )))
                    }
                };
            }
        }
        let (planned, tan, at_max) = match shape {
            Start::FullRing { end, sign } => self.steepen(|tan| {
                ring_ramp::plan(
                    b,
                    stock,
                    common.approach_distance.0,
                    tan,
                    points,
                    end,
                    sign,
                    self.probe_height,
                )
            }),
            Start::Spiral { sign } => {
                let Some(head) = points.first() else {
                    return Ok(None);
                };
                let at = b.rotary.to_axis(head.tip, head.a);
                let step = self.angle_step(at.across.hypot(at.height)).to_degrees();
                self.steepen(|tan| {
                    ring_ramp::plan_spiral(
                        b,
                        stock,
                        common.approach_distance.0,
                        tan,
                        at,
                        head.a.0,
                        sign,
                        self.probe_height,
                        step,
                        |a| self.touch_height(b, input, at, Degrees(a)),
                    )
                })
            }
            _ => return Ok(None),
        };
        let plan = match planned {
            Ok(plan) => plan,
            Err(ring_ramp::Refusal::NothingToDescend) => return Ok(None),
            Err(ring_ramp::Refusal::TooManyTurns { radius, turns }) => {
                return Err(PathError::Strategy(format!(
                    "Ramp entry: pass {} turns round the part with its tip {radius:.3} mm from \
                     the rotary axis, so {} a ramp round it would take {turns:.1} turns, more \
                     than the {:.0} allowed. {}",
                    pass + 1,
                    angle_words(tan, at_max),
                    ring_ramp::MAX_TURNS,
                    fix_words(at_max)
                )));
            }
        };
        self.in_a_range(input, pass, plan).map(Some)
    }

    /// Plans with the Ramp angle and, when that needs more than 10 turns and
    /// the steeper ramp near the axis is on, again at the angle that fits
    /// 10 turns, never steeper than the maximum (decision 0543 section 4,
    /// point 3). Gives the plan or refusal, the tangent of the angle used
    /// last, and whether that was the maximum.
    fn steepen(
        &self,
        mut attempt: impl FnMut(f64) -> std::result::Result<RingRamp, ring_ramp::Refusal>,
    ) -> (std::result::Result<RingRamp, ring_ramp::Refusal>, f64, bool) {
        let first = attempt(self.ramp_tan);
        let Err(ring_ramp::Refusal::TooManyTurns { mut turns, .. }) = first else {
            return (first, self.ramp_tan, false);
        };
        if !self.steeper_near_axis || self.tan_max <= self.ramp_tan {
            return (first, self.ramp_tan, false);
        }
        // Turns fall as the tangent rises: scale it to fit 10 turns, a few
        // times, as a steep fall of the pass may add a turn.
        let mut tan = self.ramp_tan;
        for _ in 0..4 {
            tan = (tan * turns / ring_ramp::MAX_TURNS * (1.0 + 1e-6)).min(self.tan_max);
            match attempt(tan) {
                Ok(mut plan) => {
                    plan.steepened = Some(tan.atan().to_degrees());
                    return (Ok(plan), tan, false);
                }
                Err(ring_ramp::Refusal::TooManyTurns { turns: t, .. }) => turns = t,
                other => return (other, tan, false),
            }
            if tan >= self.tan_max {
                break;
            }
        }
        let at_max = tan >= self.tan_max;
        (attempt(tan), tan, at_max)
    }

    /// Refuses `plan` when its start is outside a limited A range (decision
    /// 0543 D6).
    fn in_a_range(
        &self,
        input: &StrategyInput<'_>,
        pass: usize,
        plan: RingRamp,
    ) -> Result<RingRamp> {
        if let (openchuck_types::machine::ARange::Limited { min, max }, Some((_, a))) =
            (input.rotary.range, plan.start())
            && (a.0 < min.0 - EPS || a.0 > max.0 + EPS)
        {
            return Err(PathError::Strategy(format!(
                "Ramp entry: pass {} needs A from {:.3} degrees to ramp round the part, outside \
                 the machine's A range of {:.3} to {:.3} degrees. Widen A range in the setup's \
                 Machine settings, or set Entry to Plunge or Helix.",
                pass + 1,
                a.0,
                min.0,
                max.0
            )));
        }
        Ok(plan)
    }

    /// Where the entry of this pass starts, when it is not over the pass's
    /// first cut point at the pass's first A: the link goes there
    /// (decision 0543 section 2).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn start(
        &self,
        b: &Builder<'_>,
        input: &StrategyInput<'_>,
        stock: View<'_>,
        common: &CommonParameters,
        pass: usize,
        points: &[CutPoint],
        rapids: &[usize],
    ) -> Result<Option<CutPoint>> {
        Ok(self
            .ring_plan(b, input, stock, common, pass, points, rapids)?
            .and_then(|plan| plan.start())
            .map(|(pos, a)| {
                let tip = b.rotary.to_model(pos, a);
                CutPoint {
                    tip,
                    a,
                    contact: tip,
                }
            }))
    }

    /// Emits a planned ring or spiral ramp.
    fn ring_ramp(
        &self,
        b: &mut Builder<'_>,
        input: &StrategyInput<'_>,
        common: &CommonParameters,
        pass: usize,
        plan: &RingRamp,
    ) -> Result<()> {
        let kind = if plan.on_axis.is_some() {
            MoveKind::Plunge
        } else {
            MoveKind::LeadIn
        };
        self.emit(b, input, common, pass, &plan.moves, kind)?;
        if plan.follows_steep_drop {
            b.steep_ramps.push(pass);
        }
        if let Some(angle) = plan.steepened {
            b.steepened_ramps.push((pass, angle));
        }
        if let Some(turns) = plan.on_axis {
            b.notes.push(
                crate::notice::Notice::information(format!(
                    "pass {} starts on the rotary axis, where a ramp has no path to follow, so \
                     the tool descends there while the part turns {turns:.2} turns",
                    pass + 1
                ))
                .about(crate::notice::topic::RAMP_ENTRY),
            );
        }
        Ok(())
    }

    /// The height at which the tool, at `pos`'s X and Y and angle `a`,
    /// meets the finished surface with the stock to leave (negative
    /// infinity where it meets nothing).
    fn touch_height(
        &self,
        b: &Builder<'_>,
        input: &StrategyInput<'_>,
        pos: AxisPosition,
        a: Degrees,
    ) -> f64 {
        let Some(grown) = self.grown.as_ref() else {
            return f64::NEG_INFINITY;
        };
        let line = ToolLine {
            origin: b.rotary.to_model(
                AxisPosition {
                    height: self.probe_height,
                    ..pos
                },
                a,
            ),
            axis: b.rotary.tool_axis(a),
        };
        input
            .surface
            .drop_tool(grown, &line)
            .map_or(f64::NEG_INFINITY, |c| {
                self.probe_height + c.offset + self.leave
            })
    }

    /// Segments in a quarter circle of radius `r`.
    fn arc_segments(&self, r: f64) -> usize {
        ((FRAC_PI_2 / self.angle_step(r)).ceil() as usize).clamp(2, MAX_SEGMENTS)
    }

    /// The largest angle a chord of the path tolerance allows on radius `r`.
    fn angle_step(&self, r: f64) -> f64 {
        if self.chord >= r {
            FRAC_PI_2
        } else {
            (2.0 * (1.0 - self.chord / r).acos()).min(FRAC_PI_2)
        }
    }

    /// The worst gouge of the finished surface over `positions` (tool tip
    /// positions at angle `a`), if any exceeds the path tolerance:
    /// `(depth, index)`.
    fn gouge(
        &self,
        b: &Builder<'_>,
        input: &StrategyInput<'_>,
        a: Degrees,
        positions: &[AxisPosition],
    ) -> Option<Gouge> {
        let grown = self.grown.as_ref()?;
        let rotary = b.rotary;
        let axis = rotary.tool_axis(a);
        let (surface, leave) = (input.surface, self.leave);
        // Every position the tool will be sent to, in parallel and in order
        // (NFR-06); the depth of each below the finished surface.
        let depths: Vec<Option<f64>> = parallel::map_in_order(positions, |pos| {
            let line = ToolLine {
                origin: rotary.to_model(*pos, a),
                axis,
            };
            surface.drop_tool(grown, &line).map(|c| c.offset + leave)
        });
        let mut worst: Option<Gouge> = None;
        for (i, depth) in depths.into_iter().enumerate() {
            if let Some(d) = depth
                && d > self.tolerance
                && worst.is_none_or(|w| d > w.depth)
            {
                worst = Some(Gouge {
                    depth: d,
                    position: i,
                    x: positions
                        .get(i)
                        .map_or(f64::NAN, |p| rotary.to_model(*p, a).x),
                });
            }
        }
        worst
    }

    /// The refusal of an entry or exit that would cut into the finished
    /// surface. Where that is a held region (the tool is then over its
    /// keep-out), the message names it, as the user may not see why a ramp
    /// beside the part fails (SET-09; decision records 0114, 0161).
    fn refuse(
        &self,
        input: &StrategyInput<'_>,
        pass: usize,
        name: &str,
        size: &str,
        found: Gouge,
    ) -> PathError {
        let mid = crate::strategy::part_middle(input);
        if let Some(end) = crate::strategy::held_end(input.keep_out, mid, found.x, found.x) {
            let held = crate::strategy::held_names(&std::collections::BTreeSet::from([end]));
            return PathError::Strategy(format!(
                "pass {}: the {name} needs {size}, and it would run over {held} at X \
                 {:.3} mm, where the workholding grips the stock; use a plunge entry, or a size \
                 or angle that keeps it clear of the held region",
                pass + 1,
                found.x
            ));
        }
        PathError::Strategy(format!(
            "pass {}: the {name} needs {size}, and the tool would cut {:.3} mm into the finished \
             surface at position {} of its path (the tolerance is {:.3} mm); try another entry \
             style, a smaller size or a steeper angle",
            pass + 1,
            found.depth,
            found.position + 1,
            self.tolerance
        ))
    }
}

/// The deepest point an entry or exit would cut into the finished surface.
#[derive(Debug, Clone, Copy)]
struct Gouge {
    /// How far into the surface, in millimetres.
    depth: f64,
    /// Its index among the positions checked.
    position: usize,
    /// The model X of the tool's tip there (NaN when not known).
    x: f64,
}

impl Default for Gouge {
    fn default() -> Self {
        Self {
            depth: 0.0,
            position: 0,
            x: f64::NAN,
        }
    }
}

/// The height a rapid descent over the line the tool is on stops at: the
/// approach distance above where the stock begins on that line (or above the
/// goal, whichever is higher), and never above where the tool is.
fn stop_height(b: &Builder<'_>, stock: View<'_>, common: &CommonParameters, goal: f64) -> f64 {
    let here = b.at();
    let travel = (here.height - goal).max(0.0);
    let entry_height = super::footprint_entry(b, stock, here, b.pose.a, here.height, travel)
        .map(|d| here.height - d);
    let stop = goal.max(entry_height.unwrap_or(f64::NEG_INFINITY)) + common.approach_distance.0;
    stop.min(here.height)
}

/// A rapid straight down to `height`, if the tool is above it.
fn rapid_to(b: &mut Builder<'_>, height: f64) -> Result<()> {
    let here = b.at();
    if height < here.height - EPS {
        let tip = b.rotary.to_model(AxisPosition { height, ..here }, b.pose.a);
        let end = b.pose(tip, b.pose.a);
        b.push(MoveKind::Rapid, end, None, None)?;
    }
    Ok(())
}

/// Down to the approach distance above the stock, then a plunge at the plunge
/// feed.
fn plunge(
    b: &mut Builder<'_>,
    stock: View<'_>,
    common: &CommonParameters,
    target: &CutPoint,
) -> Result<()> {
    let goal = b.rotary.to_axis(target.tip, target.a);
    let stop = stop_height(b, stock, common, goal.height);
    rapid_to(b, stop)?;
    let end = b.pose(target.tip, target.a);
    b.push(MoveKind::Plunge, end, None, Some(common.plunge_feed))
}

/// The unit horizontal direction of the first move of the pass that has one.
fn direction_at_start(b: &Builder<'_>, points: &[CutPoint]) -> Option<Dir> {
    let first = points.first()?;
    let p = b.rotary.to_axis(first.tip, first.a);
    points.iter().skip(1).find_map(|q| {
        let q = b.rotary.to_axis(q.tip, q.a);
        unit((q.along - p.along, q.across - p.across))
    })
}

/// The unit horizontal direction of the last move of the pass that has one.
fn direction_at_end(b: &Builder<'_>, points: &[CutPoint]) -> Option<Dir> {
    let last = points.last()?;
    let p = b.rotary.to_axis(last.tip, last.a);
    points.iter().rev().skip(1).find_map(|q| {
        let q = b.rotary.to_axis(q.tip, q.a);
        unit((p.along - q.along, p.across - q.across))
    })
}

fn unit(v: Dir) -> Option<Dir> {
    let n = v.0.hypot(v.1);
    (n > 1e-6).then(|| (v.0 / n, v.1 / n))
}

/// The travel along `path`: its length across the machine plus, where A
/// turns, the arc length at the step's mean distance from the rotary axis.
fn travel_of(path: &[(AxisPosition, Degrees)]) -> f64 {
    path.windows(2)
        .map(|w| match w {
            [(q, b), (p, a)] => step_length(*q, *b, *p, *a),
            _ => 0.0,
        })
        .sum()
}

/// The travel of one step (see [`travel_of`]).
fn step_length(q: AxisPosition, b: Degrees, p: AxisPosition, a: Degrees) -> f64 {
    let radius = 0.5 * (p.across.hypot(p.height) + q.across.hypot(q.height));
    (p.along - q.along).hypot(p.across - q.across) + (a.0 - b.0).abs().to_radians() * radius
}

/// The highest stock under the tool's footprint along `path` from its first
/// point up to a travel of `reach` (decision 0543 D5), probed from `probe`
/// (above all the stock) at most half a tool radius apart; negative
/// infinity where there is none.
fn stock_top_along(
    b: &Builder<'_>,
    stock: View<'_>,
    path: &[(AxisPosition, Degrees)],
    reach: f64,
    probe: f64,
) -> f64 {
    let spacing = (0.5 * b.tool_radius).max(1e-3);
    let mut top = f64::NEG_INFINITY;
    let mut probe_at = |pos: AxisPosition, a: Degrees| {
        let over = AxisPosition {
            height: probe,
            ..pos
        };
        if let Some(d) = super::footprint_entry(b, stock, over, a, probe, 2.0 * probe.abs() + 1.0) {
            top = top.max(probe - d);
        }
    };
    let Some(&(first, a0)) = path.first() else {
        return top;
    };
    probe_at(first, a0);
    let mut travelled = 0.0;
    for w in path.windows(2) {
        let [(q, b0), (p, a)] = w else { continue };
        if travelled >= reach {
            break;
        }
        let length = step_length(*q, *b0, *p, *a);
        let n = ((length / spacing).ceil() as usize).clamp(1, 10_000);
        for k in 1..=n {
            let f = k as f64 / n as f64;
            if travelled + f * length > reach + spacing {
                break;
            }
            probe_at(
                AxisPosition {
                    along: q.along + f * (p.along - q.along),
                    across: q.across + f * (p.across - q.across),
                    height: q.height + f * (p.height - q.height),
                },
                Degrees(b0.0 + f * (a.0 - b0.0)),
            );
        }
        travelled += length;
    }
    top
}

/// The angle of a refusal of too many turns: the Ramp angle, or the
/// Maximum ramp angle when the steeper ramp near the axis reached it.
fn angle_words(tan: f64, at_max: bool) -> String {
    let degrees = tan.atan().to_degrees();
    if at_max {
        format!("even at the Maximum ramp angle of {degrees:.2} degrees")
    } else {
        format!("at {degrees:.2} degrees")
    }
}

/// What to change after a refusal of too many turns.
fn fix_words(at_max: bool) -> &'static str {
    if at_max {
        "Raise Maximum ramp angle, or set Entry to Plunge or Helix."
    } else {
        "Raise Ramp angle, or turn on Steeper ramp near the axis, or set Entry to Plunge or Helix."
    }
}
