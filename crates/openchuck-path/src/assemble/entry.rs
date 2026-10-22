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

/// The most positions checked along a ramp, which is one straight move: they
/// are at most a quarter of the tool radius apart until this many.
const MAX_RAMP_CHECKS: usize = 20_000;

/// A horizontal direction in the fixed frame: (along, across).
type Dir = (f64, f64);

pub(super) struct Entry {
    style: EntryStyle,
    ramp_tan: f64,
    helix_radius: f64,
    lead_radius: f64,
    leave: f64,
    tolerance: f64,
    chord: f64,
    /// The profile grown by the stock to leave; `None` for a plain plunge.
    grown: Option<RevolvedProfile>,
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
        })
    }

    /// Moves from above the pass's first point to that point.
    pub(super) fn enter(
        &self,
        b: &mut Builder<'_>,
        input: &StrategyInput<'_>,
        stock: View<'_>,
        common: &CommonParameters,
        pass: usize,
        points: &[CutPoint],
    ) -> Result<()> {
        let Some(head) = points.first() else {
            return Ok(());
        };
        match self.style {
            EntryStyle::Plunge => plunge(b, stock, common, head),
            EntryStyle::Lead => self.lead_in(b, input, stock, common, pass, head, points),
            EntryStyle::Ramp => self.ramp(b, input, stock, common, pass, head, points),
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
    ) -> Result<()> {
        let name = "ramp";
        let goal = b.rotary.to_axis(head.tip, head.a);
        let top = stop_height(b, stock, common, goal.height);
        let rise = top - goal.height;
        if rise <= EPS {
            return plunge(b, stock, common, head);
        }
        // As for the lead-in: a pass of one cut point has no line to ramp
        // along (issue 199, decision 0204).
        let Some(d) = direction_at_start(b, points) else {
            return plunge(b, stock, common, head);
        };
        let run = rise / self.ramp_tan;
        let start = AxisPosition {
            along: goal.along - d.0 * run,
            across: goal.across - d.1 * run,
            height: top,
        };
        let positions = lerp_positions(start, goal, run / (0.25 * input.tool.cutting_radius()));
        if let Some(g) = self.gouge(b, input, head.a, &positions) {
            return Err(self.refuse(
                input,
                pass,
                name,
                &format!(
                    "a run of {run:.3} mm to descend {rise:.3} mm at {:.2} degrees",
                    self.ramp_tan.atan().to_degrees()
                ),
                g,
            ));
        }
        // Across at the link height, down to the start of the ramp, then
        // along it.
        let here = b.at();
        let over = AxisPosition {
            height: here.height,
            ..start
        };
        let tip = b.rotary.to_model(over, head.a);
        let pose = b.pose(tip, head.a);
        b.push(MoveKind::Rapid, pose, None, None)?;
        let travel = here.height - top;
        if let Some(hit) = super::footprint_entry(b, stock, b.at(), head.a, here.height, travel)
            && hit < travel - 1e-6
        {
            return Err(PathError::Strategy(format!(
                "pass {}: the ramp starts {run:.3} mm back from the first cut point, where the \
                 stock is {:.3} mm higher than the {top:.3} mm the rapid stops at; use a \
                 steeper angle, another entry style or a larger approach distance",
                pass + 1,
                travel - hit
            )));
        }
        rapid_to(b, top)?;
        let pose = b.pose(head.tip, head.a);
        b.push(MoveKind::LeadIn, pose, None, Some(common.plunge_feed))
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

/// Positions evenly spaced from `from` to `to`, `count` steps (at least 2).
fn lerp_positions(from: AxisPosition, to: AxisPosition, count: f64) -> Vec<AxisPosition> {
    let n = count.ceil().clamp(2.0, MAX_RAMP_CHECKS as f64) as usize;
    (0..=n)
        .map(|i| {
            let t = i as f64 / n as f64;
            AxisPosition {
                along: from.along + t * (to.along - from.along),
                across: from.across + t * (to.across - from.across),
                height: from.height + t * (to.height - from.height),
            }
        })
        .collect()
}
