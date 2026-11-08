// SPDX-License-Identifier: AGPL-3.0-or-later
//! The lead on a pass that starts or ends by turning A (proposed decision
//! record 0544 items 1 to 5; issue #640).
//!
//! On a ring or a spiral the part turns under the tool, so a lead drawn in
//! the machine's X-Y plane (the line pass's lead, decision 0026) runs along
//! the rotary axis instead of along the cut. Here the lead lies in the plane
//! of the cut instead, at the pass's machine X and Y: in the unrolled frame
//! (the distance around the part at the tip's radius at the cut's end, and
//! the height) it is a quarter circle of the lead radius that meets the cut
//! tangentially, so the tool comes down onto the cut turning A and moving Z
//! together, and leaves it the same way, rising. A turns the way the pass
//! turns throughout, never reversing (OP-15).
//!
//! The moves above the surface carry no contact point; only the move that
//! meets it does (L3). Every position is checked against the finished
//! surface at its own A. A line pass, or a pass of one point, keeps the
//! horizontal lead of [`Entry::lead_in`] and [`Entry::exit`] unchanged.
//!
//! This module is a child of `entry` so it shares that module's checks.

use std::f64::consts::FRAC_PI_2;

use openchuck_types::ir::MoveKind;
use openchuck_types::units::Degrees;

use super::super::{Builder, EPS, View};
use super::pass_shape::{self, Start};
use super::{Entry, Gouge, rapid_to, stop_height};
use crate::cutter::ToolLine;
use crate::error::{PathError, Result};
use crate::parallel;
use crate::params::{CommonParameters, EntryStyle};
use crate::rotary::AxisPosition;
use crate::strategy::{CutPoint, StrategyInput};

/// How one end of a pass is shaped, for the lead.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum End {
    /// The pass ends here by turning A, the way `sign` says (+1 or -1) as
    /// the pass runs; `closed` when that end is a full ring or a spiral, so
    /// the stretch beside it is the pass's own cut.
    Turning { sign: f64, closed: bool },
    /// A line, or a single point: today's lead (decision 0026).
    Other,
}

fn of_start(shape: Start) -> End {
    match shape {
        Start::FullRing { sign, .. } | Start::Spiral { sign } => {
            End::Turning { sign, closed: true }
        }
        Start::PartialRing { sign, .. } => End::Turning {
            sign,
            closed: false,
        },
        Start::Line { .. } | Start::SinglePoint => End::Other,
    }
}

/// How the pass of `points` starts.
pub(super) fn first_end(b: &Builder<'_>, points: &[CutPoint], rapids: &[usize], tol: f64) -> End {
    of_start(pass_shape::start(&b.rotary, points, rapids, tol))
}

/// How the pass of `points` ends, with the sign of the way A turns as the
/// pass runs: the start of the pass read backwards.
pub(super) fn last_end(b: &Builder<'_>, points: &[CutPoint], rapids: &[usize], tol: f64) -> End {
    let n = points.len();
    let reversed: Vec<CutPoint> = points.iter().rev().copied().collect();
    // Point `r` is reached by a rapid from `r - 1`; read backwards, that
    // move reaches point `n - r` of the reversed pass.
    let mut back: Vec<usize> = rapids
        .iter()
        .filter(|&&r| r >= 1 && r < n)
        .map(|&r| n - r)
        .collect();
    back.sort_unstable();
    match of_start(pass_shape::start(&b.rotary, &reversed, &back, tol)) {
        End::Turning { sign, closed } => End::Turning {
            sign: -sign,
            closed,
        },
        End::Other => End::Other,
    }
}

/// A planned lead: the positions (machine frame) and angles of its moves in
/// the order the tool makes them. A lead-in's first is where the arc starts
/// and its last is just before the pass's first point; a lead-out's first is
/// just after the pass's last point and its last is where the arc ends.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct TurnLead {
    pub moves: Vec<(AxisPosition, Degrees)>,
}

impl Entry {
    /// Whether the tip at `end` is on the rotary axis, within the path
    /// tolerance. A lead has no plane of the cut there, so such an end is
    /// led like a single-point pass: a plunge with a note, and no lead-out
    /// (decision 0543 point 2, the owner's answer, relayed).
    fn on_axis(&self, b: &Builder<'_>, end: Option<&CutPoint>) -> bool {
        end.is_some_and(|p| {
            let at = b.rotary.to_axis(p.tip, p.a);
            at.across.hypot(at.height) <= self.tolerance
        })
    }

    /// Whether the pass starts by turning A with its tip on the rotary axis.
    pub(super) fn starts_on_axis(
        &self,
        b: &Builder<'_>,
        points: &[CutPoint],
        rapids: &[usize],
    ) -> bool {
        matches!(
            first_end(b, points, rapids, self.tolerance),
            End::Turning { .. }
        ) && self.on_axis(b, points.first())
    }

    /// Whether this pass's lead-in is the lead of a pass that turns A.
    pub(super) fn turning_lead_in(
        &self,
        b: &Builder<'_>,
        points: &[CutPoint],
        rapids: &[usize],
    ) -> Option<f64> {
        if self.style != EntryStyle::Lead {
            return None;
        }
        match first_end(b, points, rapids, self.tolerance) {
            End::Turning { sign, .. } if !self.on_axis(b, points.first()) => Some(sign),
            _ => None,
        }
    }

    /// The lead-in arc of a pass that starts by turning A the way `sign`
    /// says: from one lead radius back and one above, round to the first
    /// point, tangent to the cut there (0544 item 1).
    pub(super) fn arc_in(&self, b: &Builder<'_>, points: &[CutPoint], sign: f64) -> TurnLead {
        self.arc(b, points.iter(), sign, true)
    }

    /// The lead-out arc of a pass that ends by turning A the way `sign`
    /// says: from the last point, tangent to the cut, round to one lead
    /// radius on and one above.
    pub(super) fn arc_out(&self, b: &Builder<'_>, points: &[CutPoint], sign: f64) -> TurnLead {
        self.arc(b, points.iter().rev(), sign, false)
    }

    /// The arc at the end of the pass that `points` starts from (the pass
    /// forwards for a lead-in, backwards for a lead-out). `sign` is the way
    /// A turns as the pass runs.
    fn arc<'p>(
        &self,
        b: &Builder<'_>,
        mut points: impl Iterator<Item = &'p CutPoint>,
        sign: f64,
        entering: bool,
    ) -> TurnLead {
        let Some(end) = points.next() else {
            return TurnLead { moves: Vec::new() };
        };
        let p = b.rotary.to_axis(end.tip, end.a);
        // Callers leave out an end on the rotary axis (`on_axis`).
        let rho = p.across.hypot(p.height).max(EPS);
        // The unrolled frame: `s` the distance round the part at radius
        // `rho`, positive the way the pass runs; `h` the height.
        let unrolled = |q: &CutPoint| {
            let at = b.rotary.to_axis(q.tip, q.a);
            let s = sign * (q.a.0 - end.a.0).to_radians() * rho;
            (s, at.height - p.height)
        };
        // The cut's direction at the end, pointing along the pass (into the
        // cut for a lead-in, out of it for a lead-out).
        let along = points.map(unrolled).find_map(|(s, h)| {
            let n = s.hypot(h);
            (n > 1e-6).then(|| {
                let d = (s / n, h / n);
                if entering { d } else { (-d.0, -d.1) }
            })
        });
        // A cut that only moves the tip up or down has no way round: the
        // lead meets it as if it went round level.
        let d = along.filter(|d| d.0 > 1e-6).unwrap_or((1.0, 0.0));
        // The arc turns, towards the side away from the part, between the
        // cut's direction `theta` and straight down (a lead-in) or straight
        // up (a lead-out), so the tool meets or leaves the cut tangentially
        // and never moves back round the part: A turns the pass's way only
        // (OP-15). For a level cut it is a quarter circle one lead radius
        // back (or on) and one up.
        let theta = d.1.atan2(d.0);
        let (from, to) = if entering {
            (-FRAC_PI_2, theta)
        } else {
            (theta, FRAC_PI_2)
        };
        let r = self.lead_radius;
        let m = ((self.arc_segments(r) as f64) * (to - from) / FRAC_PI_2)
            .ceil()
            .max(2.0) as usize;
        // The centre is one lead radius from the end, across the cut on
        // the side away from the part; at tangent angle `alpha` the tool is
        // at `centre + r (sin alpha, -cos alpha)`.
        let centre = (-r * theta.sin(), r * theta.cos());
        let place = |alpha: f64| {
            let s = centre.0 + r * alpha.sin();
            let h = centre.1 - r * alpha.cos();
            (
                AxisPosition {
                    height: p.height + h,
                    ..p
                },
                Degrees(end.a.0 + sign * (s / rho).to_degrees()),
            )
        };
        let at = |j: usize| from + (to - from) * j as f64 / m as f64;
        let moves = if entering {
            (0..m).map(|j| place(at(j))).collect()
        } else {
            (1..=m).map(|j| place(at(j))).collect()
        };
        TurnLead { moves }
    }

    /// The deepest cut into the finished surface over `moves`, each at its
    /// own A, if any exceeds the path tolerance.
    fn gouge_turning(
        &self,
        b: &Builder<'_>,
        input: &StrategyInput<'_>,
        moves: &[(AxisPosition, Degrees)],
    ) -> Option<Gouge> {
        let grown = self.grown.as_ref()?;
        let rotary = b.rotary;
        let (surface, leave) = (input.surface, self.leave);
        let depths: Vec<Option<f64>> = parallel::map_in_order(moves, |(pos, a)| {
            let line = ToolLine {
                origin: rotary.to_model(*pos, *a),
                axis: rotary.tool_axis(*a),
            };
            surface.drop_tool(grown, &line).map(|c| c.offset + leave)
        });
        let mut worst: Option<Gouge> = None;
        for (i, depth) in depths.into_iter().enumerate() {
            if let Some(dp) = depth
                && dp > self.tolerance
                && worst.is_none_or(|w| dp > w.depth)
            {
                worst = Some(Gouge {
                    depth: dp,
                    position: i,
                    x: moves
                        .get(i)
                        .map_or(f64::NAN, |(p, a)| rotary.to_model(*p, *a).x),
                });
            }
        }
        worst
    }

    /// Checks a planned lead: the finished surface, the machine's A range
    /// and, on a partial ring in a pass that carries its stock, that the
    /// moves above the cut stay out of the stock (0544 items 1, 5 and 6).
    #[allow(clippy::too_many_arguments)]
    fn check_turning(
        &self,
        b: &Builder<'_>,
        input: &StrategyInput<'_>,
        stock: View<'_>,
        pass: usize,
        name: &str,
        lead: &TurnLead,
        closed: bool,
    ) -> Result<()> {
        let r = self.lead_radius;
        if let Some(g) = self.gouge_turning(b, input, &lead.moves) {
            return Err(self.refuse_turning(pass, name, g));
        }
        if let openchuck_types::machine::ARange::Limited { min, max } = input.rotary.range
            && let Some((_, a)) = lead
                .moves
                .iter()
                .find(|(_, a)| a.0 < min.0 - EPS || a.0 > max.0 + EPS)
        {
            return Err(PathError::Strategy(format!(
                "Lead entry: pass {}: the {name} turns A to {:.3} degrees on an arc of radius \
                 {r:.3} mm, outside the machine's A range of {:.3} to {:.3} degrees. Widen A \
                 range in the setup's Machine settings, make Lead radius smaller, or set Entry \
                 to Plunge or Helix.",
                pass + 1,
                a.0,
                min.0,
                max.0
            )));
        }
        if !closed && stock.wide {
            // A partial ring's stretch beside its end is not cut by the
            // pass, so in roughing the arc may only pass over stock that is
            // already gone.
            for (pos, a) in &lead.moves {
                let over = AxisPosition {
                    height: self.probe_height,
                    ..*pos
                };
                let travel = (self.probe_height - pos.height).max(0.0);
                if super::super::footprint_entry(b, stock, over, *a, self.probe_height, travel)
                    .is_some_and(|d| d < travel - EPS)
                {
                    return Err(PathError::Strategy(format!(
                        "Lead entry: pass {}: the {name} on an arc of radius {r:.3} mm would \
                         pass through stock the pass has not cleared, beside a cut that turns A \
                         only part of a turn. Set Entry to Plunge, Ramp or Helix.",
                        pass + 1
                    )));
                }
            }
        }
        Ok(())
    }

    /// The refusal of a lead on a pass that turns A that would cut the
    /// finished surface (decision 0536 form). Such a lead never leaves the
    /// pass's X, so it can reach a held region only where the pass itself
    /// does, which generation has already refused.
    fn refuse_turning(&self, pass: usize, name: &str, found: Gouge) -> PathError {
        let r = self.lead_radius;
        PathError::Strategy(format!(
            "Lead entry: pass {}: the {name} on an arc of radius {r:.3} mm would cut {:.3} mm \
             into the finished surface at position {} of its path, more than the tolerance of \
             {:.3} mm. Make Lead radius smaller, or set Entry to Plunge, Ramp or Helix.",
            pass + 1,
            found.depth,
            found.position + 1,
            self.tolerance
        ))
    }

    /// Where the lead-in of a pass that starts by turning A starts, for the
    /// link to arrive at (0544 item 5); `None` for other passes.
    pub(super) fn turning_lead_start(
        &self,
        b: &Builder<'_>,
        points: &[CutPoint],
        rapids: &[usize],
    ) -> Option<CutPoint> {
        let sign = self.turning_lead_in(b, points, rapids)?;
        let (pos, a) = self.arc_in(b, points, sign).moves.first().copied()?;
        let tip = b.rotary.to_model(pos, a);
        Some(CutPoint {
            tip,
            a,
            contact: tip,
        })
    }

    /// The lead-in of a pass that starts by turning A: over the arc's start
    /// at the height the tool is at, down beside the stock and by a plunge
    /// to the arc's start, then round the arc, turning A the pass's way, to
    /// the first point at the cutting feed (0544 items 1, 3 and 5).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn turning_lead_in_moves(
        &self,
        b: &mut Builder<'_>,
        input: &StrategyInput<'_>,
        stock: View<'_>,
        common: &CommonParameters,
        pass: usize,
        points: &[CutPoint],
        rapids: &[usize],
        sign: f64,
    ) -> Result<()> {
        let Some(head) = points.first() else {
            return Ok(());
        };
        let closed = matches!(
            first_end(b, points, rapids, self.tolerance),
            End::Turning { closed: true, .. }
        );
        let lead = self.arc_in(b, points, sign);
        self.check_turning(b, input, stock, pass, "lead-in", &lead, closed)?;
        let Some(&(start, a)) = lead.moves.first() else {
            return Ok(());
        };
        if start.height > b.at().height + EPS {
            b.retract(start.height)?;
        }
        let height = b.at().height;
        let over = b.rotary.to_model(AxisPosition { height, ..start }, a);
        let pose = b.pose(over, a);
        b.push(MoveKind::Rapid, pose, None, None)?;
        let stop = stop_height(b, stock, common, start.height);
        rapid_to(b, stop)?;
        let tip = b.rotary.to_model(start, a);
        let pose = b.pose(tip, a);
        b.push(MoveKind::Plunge, pose, None, Some(common.plunge_feed))?;
        for (pos, a) in lead.moves.iter().skip(1) {
            let tip = b.rotary.to_model(*pos, *a);
            let pose = b.pose(tip, *a);
            b.push(MoveKind::LeadIn, pose, None, Some(common.feed))?;
        }
        let pose = b.pose(head.tip, head.a);
        b.push(
            MoveKind::LeadIn,
            pose,
            Some(head.contact),
            Some(common.feed),
        )
    }

    /// The lead-out of a pass that ends by turning A, or `None` when the
    /// pass ends otherwise and keeps today's lead-out.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn turning_lead_out(
        &self,
        b: &mut Builder<'_>,
        input: &StrategyInput<'_>,
        stock: View<'_>,
        common: &CommonParameters,
        pass: usize,
        points: &[CutPoint],
        rapids: &[usize],
    ) -> Option<Result<()>> {
        let End::Turning { sign, closed } = last_end(b, points, rapids, self.tolerance) else {
            return None;
        };
        if self.on_axis(b, points.last()) {
            // A pass that also starts there was named by its entry's note,
            // which says it has no lead-out.
            if !self.starts_on_axis(b, points, rapids) {
                note_axis_end(b, pass);
            }
            return Some(Ok(()));
        }
        Some((|| {
            let lead = self.arc_out(b, points, sign);
            self.check_turning(b, input, stock, pass, "lead-out", &lead, closed)?;
            for (pos, a) in &lead.moves {
                let tip = b.rotary.to_model(*pos, *a);
                let pose = b.pose(tip, *a);
                b.push(MoveKind::LeadOut, pose, None, Some(common.feed))?;
            }
            Ok(())
        })())
    }
}

/// Notes that pass `pass` is a single point, entered by a plunge (0544
/// item 4): one note names every such pass.
pub(super) fn note_single_point(b: &mut Builder<'_>, pass: usize) {
    note_passes(
        b,
        pass,
        (
            "pass ",
            " is a single point, so it has no cut to lead into: it is entered by a straight \
             plunge and has no lead-out",
        ),
        (
            "passes ",
            " are single points, so they have no cut to lead into: each is entered by a \
             straight plunge and has no lead-out",
        ),
    );
}

/// Notes that pass `pass` starts by turning A with its tip on the rotary
/// axis, entered by a plunge (decision 0543 point 2, the owner's answer,
/// relayed).
pub(super) fn note_axis_start(b: &mut Builder<'_>, pass: usize) {
    note_passes(
        b,
        pass,
        (
            "pass ",
            " starts on the rotary axis, where a lead arc has no plane to follow, so it is \
             entered by a straight plunge and has no lead-out",
        ),
        (
            "passes ",
            " start on the rotary axis, where a lead arc has no plane to follow, so each is \
             entered by a straight plunge and has no lead-out",
        ),
    );
}

/// Notes that pass `pass` ends by turning A with its tip on the rotary
/// axis, where it has no lead-out.
fn note_axis_end(b: &mut Builder<'_>, pass: usize) {
    note_passes(
        b,
        pass,
        ("pass ", " ends on the rotary axis, so it has no lead-out"),
        (
            "passes ",
            " end on the rotary axis, so they have no lead-out",
        ),
    );
}

/// Adds pass `pass` to the one "Lead entry" note worded by `one` (for one
/// pass) and `many` (for several), each a text before and after the list.
fn note_passes(b: &mut Builder<'_>, pass: usize, one: (&str, &str), many: (&str, &str)) {
    use crate::notice::{Notice, topic};
    let listed = |text: &str| {
        [one, many].iter().find_map(|(head, tail)| {
            text.strip_prefix(head)
                .and_then(|rest| rest.strip_suffix(tail))
                .map(|list| list.split(", ").map(str::to_owned).collect::<Vec<_>>())
        })
    };
    let found = b
        .notes
        .iter()
        .position(|n| n.topic.as_deref() == Some(topic::LEAD_ENTRY) && listed(&n.text).is_some());
    let mut passes = found
        .and_then(|i| b.notes.get(i))
        .and_then(|n| listed(&n.text))
        .unwrap_or_default();
    passes.push((pass + 1).to_string());
    let (head, tail) = if passes.len() == 1 { one } else { many };
    let text = format!("{head}{}{tail}", passes.join(", "));
    match found.and_then(|i| b.notes.get_mut(i)) {
        Some(note) => note.text = text,
        None => b
            .notes
            .push(Notice::information(text).about(topic::LEAD_ENTRY)),
    }
}
