// SPDX-License-Identifier: AGPL-3.0-or-later
//! The flute length against the depth of the cut (TL-04, SET-03, OP-12,
//! NFR-07, NFR-12; issue #530; decision record 0509).
//!
//! A pass that cuts all the stock above it in one go (any strategy that does
//! not cut in layers) needs flutes at least as long as the stock stands
//! above the tool's tip along the tool axis, over the tool's whole width:
//! past the flutes the shank, not a cutting edge, would meet the stock. The
//! depth is taken from the stock itself, along the tool axis, at every cut
//! point and between them, so the worst case of box stock (a corner
//! uppermost over the rotary axis, its half diagonal from the axis), stock
//! not centred on the axis or turned about it, and tips below the axis are
//! all measured as they are. [`deepest`] is the one measure every check uses;
//! [`check`] warns of a path whose deepest cut is past the flutes, saying
//! where, by how much and on which stock. It is a warning, not a refusal
//! (the owner's decision of 6 Nov 2026, decision record 0522, amending
//! 0509 item 3): the measure is conservative, so the path is kept and the
//! user, who verifies every program (NFR-12), decides.
//!
//! The depth is exact where the stock model gives it
//! ([`StockModel::top_over_disc`]: the setup's own box or round stock, in
//! closed form). Any other stock model, such as the stock earlier
//! operations left, gets a conservative bound instead, never a sampled read:
//! no material is further from the axis than [`StockModel::max_radius`], so
//! at a tip `h` above the axis the stock stands at most `max_radius - h`
//! above it. A stock with neither is refused (OI-2025).
//!
//! The same bound comes first everywhere: only where it is past the flutes
//! is the exact answer asked for.

use openchuck_types::geometry::Point3;
use openchuck_types::job::{CancelToken, JobContext};
use openchuck_types::units::Degrees;

use crate::assemble::PathWarning;
use crate::error::Result;
use crate::rotary::AxisPosition;
use crate::stock::{ReachProfile, StockModel};
use crate::strategy::{CutPoint, Pass, StrategyInput};

/// How near half a turn, degrees, a step between two cut points must be for
/// both ways round to be read: there whole turns carried by assembly decide
/// the way, so neither can be assumed. Only an exact tie is ambiguous: any
/// other step has one nearest turn whatever whole turns are added, and
/// `rotary::nearest_turn` rounds the same quotient assembly rounds, so
/// rounding (a few ulp of 360) is all the width needs. Generation itself
/// checks the assembled path ([`check_path`]), where no tie arises.
const HALF_TURN_SLACK: f64 = 1e-9;

/// The largest step in A, degrees, between the places a move is read at.
const A_STEP: f64 = 0.5;

/// The largest step along a move, mm, between the places it is read at.
const LENGTH_STEP: f64 = 0.5;

/// How far past the flutes, mm, the stock may stand before a path is
/// refused: rounding only.
const SLACK: f64 = 1e-6;

/// How far, mm, the tool may move against the stock between two places that
/// are both read before the stock between them counts as read: a step is
/// halved until its bound ([`between`]) is within the flutes or its reach is
/// this small. Below this the bound is taken as the depth.
const REFINED: f64 = 1e-6;

/// The most readings of the stock over one step between two places read
/// (CodeRabbit on #534): a top of stock that stands within [`REFINED`] of
/// the flutes along a cut would otherwise be halved to that reach
/// everywhere, about a million readings a step. Past the budget the step's
/// bound is kept as a bound, so the cut is refused, never accepted unread;
/// 256 readings take a step of 0.5 mm and 0.5 degree to about 0.002 mm, so
/// only stock that close to the flutes is refused this way.
const STEP_BUDGET: u32 = 256;

/// Added to the bound from a stock model's farthest radius, mm: that radius
/// may be measured at cell centres (the simulation's columns), so the bound
/// is not trusted closer.
const BOUND_MARGIN: f64 = 1.0;

/// The deepest cut a set of passes makes: how far the stock stands above
/// the tool's tip along the tool axis, mm, and where (model X and A).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Deepest {
    /// The stock above the tip, mm.
    pub depth: f64,
    /// Model X of the tip.
    pub x: f64,
    /// A, degrees, in 0 to 360.
    pub a: f64,
    /// True when the depth is the stock model's exact answer; false when it
    /// is the conservative bound from the stock's farthest radius.
    pub exact: bool,
}

/// The radius of the tool's body above its tip: the cutting radius, or the
/// shank's where it is wider.
fn body_radius(input: &StrategyInput<'_>) -> f64 {
    input
        .tool
        .cutting_radius()
        .max(input.shank.map_or(0.0, |s| s.radius))
}

/// How far the stock stands above a tip at `position` (fixed to the machine)
/// at angle `a`, along the tool axis, over a disc of `radius` about the tool
/// axis, zero where there is none, and whether that is exact.
///
/// Exact where the stock model gives [`StockModel::top_over_disc`] (the
/// setup's own box or round stock). Any other stock model is never read by
/// sampling, which could miss material between readings (CodeRabbit on #534):
/// it gets the conservative bound `max_radius - height` plus
/// [`BOUND_MARGIN`], since no material is further from the axis than its
/// farthest radius. A stock model with no farthest radius has no bound:
/// infinity, which [`check`] refuses.
pub(crate) fn depth_at(
    stock: &dyn StockModel,
    input: &StrategyInput<'_>,
    position: AxisPosition,
    a: Degrees,
    radius: f64,
    profile: Option<&ReachProfile>,
) -> (f64, bool) {
    if let Some(top) = stock.top_over_disc(
        input.rotary.to_model(position, a),
        input.rotary.tool_axis(a),
        radius,
    ) {
        // An answer that is not a number is no answer (NFR-07).
        if top.is_nan() {
            return (f64::INFINITY, false);
        }
        return (top.max(0.0), true);
    }
    let farthest = match stock.max_radius() {
        Some(r) if r.is_finite() && position.height.is_finite() => {
            (r - position.height + BOUND_MARGIN).max(0.0)
        }
        _ => f64::INFINITY,
    };
    let near = in_reach(input, profile, (position, a), (position, a), radius);
    (near.map_or(farthest, |n| n.min(farthest)), false)
}

/// The bound on the stock over the tool from the material within its reach
/// (issue #547), for a stock model with a [`ReachProfile`] and no exact
/// answer: over the step from `p` to `q` (the same place for one reading),
/// no material over the tool's disc lies further along X than `radius` from
/// the tips, and none stands higher along the tool axis than the profile's
/// highest over that X range, for every direction the axis takes over the
/// step's turn. Less the lower tip's height, plus [`BOUND_MARGIN`], that
/// bounds how far the stock stands above any tip of the step. `None` where
/// there is no profile, or the tool axis is not square to the rotary axis
/// (the X range would not hold).
fn in_reach(
    input: &StrategyInput<'_>,
    profile: Option<&ReachProfile>,
    p: (AxisPosition, Degrees),
    q: (AxisPosition, Degrees),
    radius: f64,
) -> Option<f64> {
    let profile = profile?;
    let middle = Degrees(0.5 * (p.1.0 + q.1.0));
    let axis = input.rotary.tool_axis(middle).get();
    let square = |a: Degrees| input.rotary.tool_axis(a).get().x.abs() <= 1e-9;
    if !(square(p.1) && square(q.1)) {
        return None;
    }
    let towards = axis.z.atan2(axis.y);
    let spread = 0.5 * (q.1.0 - p.1.0).abs().to_radians();
    let xp = input.rotary.to_model(p.0, p.1).x;
    let xq = input.rotary.to_model(q.0, q.1).x;
    let highest = profile.highest([xp.min(xq) - radius, xp.max(xq) + radius], towards, spread);
    if highest == f64::NEG_INFINITY {
        return Some(0.0);
    }
    let low = p.0.height.min(q.0.height);
    let bound = highest - low + BOUND_MARGIN;
    // A bound that is not a number is no bound (NFR-07).
    Some(if bound.is_nan() {
        f64::INFINITY
    } else {
        bound.max(0.0)
    })
}

/// The places along the move from `from` to `to` the stock is read at,
/// one chain for each way round A may turn: the start, the end, and enough
/// between that A and the tip step at most [`A_STEP`] and [`LENGTH_STEP`];
/// each fixed to the machine, with its A. With no `from`, the end alone.
/// With `emitted`, the angles are the ones assembly wrote (unwrapped,
/// carried across passes and fitted to the A range), so the move turns by
/// exactly their difference.
fn along_move(
    input: &StrategyInput<'_>,
    from: Option<&CutPoint>,
    to: &CutPoint,
    emitted: bool,
) -> Vec<Vec<(AxisPosition, Degrees)>> {
    let b = input.rotary.to_axis(to.tip, to.a);
    let Some(from) = from else {
        return vec![vec![(b, to.a)]];
    };
    let a0 = input.rotary.to_axis(from.tip, from.a);
    let length = (b.along - a0.along)
        .hypot(b.across - a0.across)
        .hypot(b.height - a0.height);
    // The way assembly turns A: the nearest equivalent of the next angle
    // (`rotary::nearest_turn`), so a pass given across the 0/360 seam turns
    // a little, not nearly a whole turn. Whole turns carried from earlier
    // points and passes, or fitted to the A range, never change which way a
    // step turns, except at an exact half turn, where they decide it. There
    // both ways round are read, so the way assembly takes is always among
    // them (CodeRabbit on #534).
    let delta = if emitted {
        to.a.0 - from.a.0
    } else {
        crate::rotary::nearest_turn(to.a, from.a).0 - from.a.0
    };
    let ways: &[f64] = if !emitted && (delta.abs() - 180.0).abs() <= HALF_TURN_SLACK {
        &[1.0, -1.0]
    } else {
        &[1.0]
    };
    let turn = delta.abs();
    let steps = ((turn / A_STEP).max(length / LENGTH_STEP).ceil() as usize).clamp(1, 10_000);
    ways.iter()
        .map(|&way| {
            (0..=steps)
                .map(|i| {
                    let t = i as f64 / steps as f64;
                    (lerp(a0, b, t), Degrees(from.a.0 + t * way * delta))
                })
                .collect()
        })
        .collect()
}

/// The place `t` of the way from `p` to `q`.
fn lerp(p: AxisPosition, q: AxisPosition, t: f64) -> AxisPosition {
    let at = |u: f64, v: f64| u + t * (v - u);
    AxisPosition {
        along: at(p.along, q.along),
        across: at(p.across, q.across),
        height: at(p.height, q.height),
    }
}

/// A reading at `position` and `a` of `depth`, kept in `worst` if deeper.
fn keep(
    worst: &mut Option<Deepest>,
    input: &StrategyInput<'_>,
    position: AxisPosition,
    a: Degrees,
    depth: f64,
    exact: bool,
) {
    // A depth that is not a number is no bound: it refuses as stock of
    // unknown size, never drops out of the comparison below.
    let depth = if depth.is_nan() { f64::INFINITY } else { depth };
    if worst.is_none_or(|w| depth > w.depth) {
        *worst = Some(Deepest {
            depth,
            exact,
            x: input.rotary.to_model(position, a).x,
            a: a.0.rem_euclid(360.0),
        });
    }
}

/// The stock between two places of a move that are both read, `p` and `q`
/// (the owner's Codex review of #534): exact readings at the places alone
/// say nothing of material the tool's body sweeps between them, such as a
/// thin slab that comes within its radius only half way. Kept in `worst`
/// when it stands past `limit`.
///
/// In the machine's frame the tool axis is fixed and the stock turns with
/// A. Over the step, the tip is within `s / 2` across the tool axis and
/// `h / 2` along it of the middle place (`s` and `h` the step's travel
/// across and along the axis), and the stock turns at most half the step's
/// angle about the middle angle, which moves no material more than
/// `t = R * angle / 2` (`R` the stock's farthest radius; none where the
/// stock is the same at every A, [`StockModel::turns_unchanged`], as round
/// stock is). Material over the
/// tool's disc at any place of the step is therefore over a disc
/// `s / 2 + t` wider at the middle place, and stands at most `h / 2 + t`
/// higher above that tip than above its own: the stock's exact top over the
/// wider disc, plus `h / 2 + t`, bounds the whole step, and a straight cut
/// at one height with A still under a flat top is bounded exactly, with no
/// halving. The step's reach is `d = s / 2 + h / 2 + t`. Where that bound is past the flutes the step is
/// halved, reading its middle exactly, until the bound is within them or
/// `d` is below [`REFINED`], where the bound is taken as the depth, or the
/// step's [`STEP_BUDGET`] of readings is spent, where it is kept as a bound
/// (not exact), so the path is refused rather than accepted unread. A stock
/// model with no exact answer is held to the bound from its farthest radius
/// at the places themselves, which bounds the step already: that bound is
/// lowest at the higher tip and does not turn with A.
#[allow(clippy::too_many_arguments)]
fn between(
    worst: &mut Option<Deepest>,
    input: &StrategyInput<'_>,
    radius: f64,
    limit: f64,
    p: (AxisPosition, Degrees),
    q: (AxisPosition, Degrees),
    budget: &mut u32,
    profile: Option<&ReachProfile>,
) {
    // A reading past the flutes already refuses the path.
    if worst.is_some_and(|w| w.depth > limit + SLACK) {
        return;
    }
    let stock = input.stock;
    let farthest = stock.max_radius();
    // The quick bound: nothing is further from the axis than `farthest`.
    if farthest.is_some_and(|r| r - p.0.height.min(q.0.height) + BOUND_MARGIN <= limit) {
        return;
    }
    let middle = lerp(p.0, q.0, 0.5);
    let a = Degrees(0.5 * (p.1.0 + q.1.0));
    // How far the tip moves from the middle across and along the tool axis,
    // and how far the stock's turn moves any material.
    let sideways = 0.5 * (q.0.along - p.0.along).hypot(q.0.across - p.0.across);
    let up = 0.5 * (q.0.height - p.0.height).abs();
    let turn = (q.1.0 - p.1.0).abs().to_radians();
    let turned = if turn > 0.0 && !stock.turns_unchanged() {
        match farthest {
            Some(r) => 0.5 * r * turn,
            // How far a turn moves the stock is not known (the owner's
            // Codex review of #534): the step cannot be bounded, so it is
            // refused as a stock of unknown size, never accepted unread.
            None => {
                keep(worst, input, middle, a, f64::INFINITY, false);
                return;
            }
        }
    } else {
        0.0
    };
    let reach = sideways + up + turned;
    *budget = budget.saturating_sub(1);
    let Some(top) = stock.top_over_disc(
        input.rotary.to_model(middle, a),
        input.rotary.tool_axis(a),
        radius + sideways + turned,
    ) else {
        // No exact answer over the wider disc: the step is held to the bound
        // from the farthest radius at its lower tip, as the places are
        // (equal to theirs where the model gives no exact answer at all).
        let low = p.0.height.min(q.0.height);
        let bound = farthest.map_or(f64::INFINITY, |r| r - low + BOUND_MARGIN);
        // Or, tighter, from the material within the tool's reach over the
        // step (issue #547).
        let near = in_reach(input, profile, p, q, radius);
        keep(
            worst,
            input,
            middle,
            a,
            near.map_or(bound, |n| n.min(bound)),
            false,
        );
        return;
    };
    let bound = top + up + turned;
    // No material over the wider disc: nothing to read.
    if bound == f64::NEG_INFINITY || bound <= limit + SLACK {
        return;
    }
    // An answer that is not a number, or no bound, refuses (NFR-07).
    if !bound.is_finite() {
        keep(worst, input, middle, a, f64::INFINITY, false);
        return;
    }
    if reach <= REFINED {
        keep(worst, input, middle, a, bound, true);
        return;
    }
    if *budget == 0 {
        keep(worst, input, middle, a, bound, false);
        return;
    }
    *budget -= 1;
    let (depth, exact) = depth_at(stock, input, middle, a, radius, profile);
    keep(worst, input, middle, a, depth, exact);
    between(worst, input, radius, limit, p, (middle, a), budget, profile);
    between(worst, input, radius, limit, (middle, a), q, budget, profile);
}

/// The deepest cut `passes` make in `input`'s stock: how far the stock
/// stands above the tool's tip along the tool axis, over the tool's width,
/// at its worst; `None` when the passes never meet the stock, or the stock
/// can be shown to stand at most `limit` above every tip without reading it.
pub fn deepest(input: &StrategyInput<'_>, passes: &[Pass], limit: f64) -> Option<Deepest> {
    // With no token to cancel, reading cannot fail.
    deepest_of(input, passes, limit, false, None).unwrap_or_default()
}

/// [`deepest`] over passes whose angles are `emitted` by assembly (see
/// [`along_move`]) or given by a strategy.
fn deepest_of(
    input: &StrategyInput<'_>,
    passes: &[Pass],
    limit: f64,
    emitted: bool,
    cancel: Option<&CancelToken>,
) -> Result<Option<Deepest>> {
    let stock = input.stock;
    let radius = body_radius(input);
    let bound = stock.max_radius();
    // Built once for the whole path (issue #547).
    let profile = stock.reach_profile();
    let profile = profile.as_ref();
    let found = crate::parallel::map_each_in_order(passes, |pass| -> Result<Option<Deepest>> {
        let mut worst: Option<Deepest> = None;
        let mut previous: Option<&CutPoint> = None;
        for (i, point) in pass.points.iter().enumerate() {
            // A place that is not a number cannot be read (NFR-07): refused,
            // and said to be the path's, not the stock's.
            if !point.tip.is_finite() || !point.a.0.is_finite() {
                return Ok(Some(Deepest {
                    depth: f64::INFINITY,
                    exact: false,
                    x: f64::NAN,
                    a: f64::NAN,
                }));
            }
            // The move to a rapid point (of a drilled hole) is not a cut and
            // is not read; the fed move from it is, from where the rapid left
            // the tool, as `check_path` reads a fed move after a rapid.
            if pass.rapids.contains(&i) {
                previous = Some(point);
                continue;
            }
            // Cancel is polled at each move (UI-09; architecture section
            // 5.1; the owner's Codex review of #534): a move's work is
            // bounded by its readings and their budget.
            if let Some(cancel) = cancel {
                cancel.check()?;
            }
            for chain in along_move(input, previous, point, emitted) {
                // The start of a move was read as the end of the one before.
                let skip = usize::from(previous.is_some());
                for &(position, a) in chain.iter().skip(skip) {
                    // Nothing is further from the axis than the bound, so the
                    // stock stands at most this far above the tip. The
                    // height is signed: a tip below the axis (a cut past the
                    // middle of the stock) adds its depth below it, so it is
                    // skipped less.
                    if bound.is_some_and(|r| r - position.height + BOUND_MARGIN <= limit) {
                        continue;
                    }
                    let (depth, exact) = depth_at(stock, input, position, a, radius, profile);
                    keep(&mut worst, input, position, a, depth, exact);
                }
                // And the stock between the places read.
                for (&p, &q) in chain.iter().zip(chain.iter().skip(1)) {
                    let mut budget = STEP_BUDGET;
                    between(&mut worst, input, radius, limit, p, q, &mut budget, profile);
                }
            }
            previous = Some(point);
        }
        Ok(worst)
    });
    let found = found.into_iter().collect::<Result<Vec<_>>>()?;
    Ok(found.into_iter().flatten().filter(|d| d.depth > 0.0).fold(
        None,
        |best: Option<Deepest>, d| {
            if best.is_none_or(|b| d.depth > b.depth) {
                Some(d)
            } else {
                best
            }
        },
    ))
}

/// How far the stock stands above the rotary axis over the tool's width
/// with the tool centred at model X `x`, at its worst over a turn of A: the
/// depth a turn down to the axis there cuts (a free end, decision records
/// 0344 and 0509). Read as [`deepest`] reads a move, every [`A_STEP`] and
/// between, so it is exact wherever it is past `limit` (the flutes) and may
/// be lower elsewhere; and whether it is exact (false where it is a bound,
/// as for stock earlier operations left).
pub(crate) fn deepest_on_axis(input: &StrategyInput<'_>, x: f64, limit: f64) -> (f64, bool) {
    let points = (0..=4)
        .map(|k| {
            let a = Degrees(90.0 * f64::from(k));
            let along = input.rotary.to_axis(Point3::new(x, 0.0, 0.0), a).along;
            let tip = input.rotary.to_model(
                AxisPosition {
                    along,
                    across: 0.0,
                    height: 0.0,
                },
                a,
            );
            CutPoint {
                tip,
                a,
                contact: tip,
            }
        })
        .collect();
    let turn = [Pass {
        points,
        ..Pass::default()
    }];
    // One turn, its work bounded by its readings and their budget, needs no
    // token to cancel; with none, reading cannot fail.
    deepest_of(input, &turn, limit, true, None)
        .unwrap_or_default()
        .map_or((0.0, true), |d| (d.depth, d.exact))
}

/// What the stock is, for a message: its own words when it gives them
/// ([`StockModel::describe`]), else how far it reaches from the axis.
pub(crate) fn stock_words(stock: &dyn StockModel) -> String {
    if stock.left_by_earlier_operations() {
        return "the stock is what the earlier operations in this setup left".to_owned();
    }
    match (stock.describe(), stock.max_radius()) {
        (Some(words), _) => format!("the stock is {words}"),
        (None, Some(r)) => format!("the stock reaches {r:.3} mm from the rotary axis"),
        (None, None) => "the stock's size is not known".to_owned(),
    }
}

/// The words every warning of this check starts with, so a host can tell it
/// apart and name the operation and the tool in it (decision record 0522).
pub const WARNING_LEAD: &str = "Flute length: ";

/// A warning when the stock stands above the tool's tip in `passes`, along
/// the tool axis, further than the tool's flutes reach (decision records
/// 0509 and 0522): past the flutes the shank may meet the stock. `None`
/// when it does not. `parameter` names the setting the user acts on
/// (Finishing layers, where the strategy has them); the message gives the
/// depth, whether it is exact or a bound, where, the flute length and the
/// stock.
pub fn check(
    input: &StrategyInput<'_>,
    passes: &[Pass],
    parameter: Option<&str>,
) -> Option<PathWarning> {
    let flutes = input.tool.top();
    warn_past_flutes(input, deepest(input, passes, flutes), parameter)
}

/// [`check`] on the path as assembly wrote it: every fed move (cuts, plunges,
/// leads and fed links; rapids are not cuts), from the pose before it, with
/// the angles assembly emitted. The generation checks this, not the passes
/// a strategy gives: the way A turns between two points (at an exact half
/// turn, decided by the turns assembly carries from earlier points and
/// passes and by its fit to the A range) is then the way the tool turns,
/// not a prediction of it (CodeRabbit on #534).
///
/// Cancel is polled at each move (UI-09): a long path is read in bounded
/// work between polls, and a cancelled job returns
/// [`crate::PathError::Cancelled`], the only error.
pub fn check_path(
    input: &StrategyInput<'_>,
    path: &crate::assemble::GeneratedPath,
    parameter: Option<&str>,
    context: &JobContext<'_>,
) -> Result<Option<PathWarning>> {
    let flutes = input.tool.top();
    let point = |pose: &openchuck_types::ir::ToolPose| CutPoint {
        tip: pose.tip,
        a: pose.a,
        contact: pose.tip,
    };
    // Runs of fed moves, each from the pose it starts at.
    let mut runs: Vec<Pass> = Vec::new();
    let mut run = vec![point(&path.group.start)];
    for segment in &path.group.segments {
        if segment.kind.is_fed() {
            run.push(point(&segment.end));
        } else {
            if run.len() > 1 {
                runs.push(Pass {
                    points: std::mem::take(&mut run),
                    ..Pass::default()
                });
            }
            run = vec![point(&segment.end)];
        }
    }
    if run.len() > 1 {
        runs.push(Pass {
            points: run,
            ..Pass::default()
        });
    }
    let worst = deepest_of(input, &runs, flutes, true, Some(context.cancel))?;
    Ok(warn_past_flutes(input, worst, parameter))
}

/// The warning of [`check`] and [`check_path`] when `worst` is past the
/// flutes. It says what was found and what the user can do; it never says
/// the program is safe (NFR-12).
fn warn_past_flutes(
    input: &StrategyInput<'_>,
    worst: Option<Deepest>,
    parameter: Option<&str>,
) -> Option<PathWarning> {
    let flutes = input.tool.top();
    let worst = worst?;
    if worst.depth <= flutes + SLACK {
        return None;
    }
    let layers = if parameter.is_some() {
        ", or turn on Finishing layers"
    } else {
        ""
    };
    let reason = if !worst.x.is_finite() || !worst.a.is_finite() {
        format!(
            "the path has a tool position that is not a finite number, so the tool's \
             {flutes:.3} mm of flute could not be checked against the stock there. Regenerate \
             the operation, and report it if it happens again"
        )
    } else if !worst.depth.is_finite() {
        format!(
            "the stock's size is not known, so the tool's {flutes:.3} mm of flute could not \
             be checked against how deep it stands above the tool (at X {:.3}, A {:.1}). \
             Define the setup's stock",
            worst.x, worst.a,
        )
    } else {
        let how = if worst.exact {
            "stands up to"
        } else {
            "may stand up to"
        };
        let bound = if worst.exact {
            ""
        } else {
            " (a bound, from how far the stock reaches from the rotary axis or across a step \
             between readings: that stock is not measured exactly across the tool)"
        };
        format!(
            "the stock {how} {:.3} mm above the tool's tip along its axis{bound} (at X {:.3}, \
             A {:.1}), more than the tool's {flutes:.3} mm of flute, so the shank may meet \
             the stock; {}. The path was made: check it before running it. Use a tool with \
             longer flutes, or rough the part first in this setup{layers}",
            worst.depth,
            worst.x,
            worst.a,
            stock_words(input.stock),
        )
    };
    Some(PathWarning {
        message: format!("{WARNING_LEAD}{reason}"),
        parameter: parameter.map(str::to_owned),
        requires_acknowledgement: false,
    })
}
