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
//! [`check`] refuses a path whose deepest cut is past the flutes, saying
//! where, by how much and on which stock.
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
use openchuck_types::units::Degrees;

use crate::error::{PathError, Result};
use crate::rotary::AxisPosition;
use crate::stock::StockModel;
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

/// The most halvings of one step between readings (a step of 0.5 mm and 0.5
/// degree comes under [`REFINED`] in about 20).
const MOST_HALVINGS: u32 = 40;

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
) -> (f64, bool) {
    if let Some(top) = stock.top_over_disc(
        input.rotary.to_model(position, a),
        input.rotary.tool_axis(a),
        radius,
    ) {
        return (top.max(0.0), true);
    }
    match stock.max_radius() {
        Some(r) => ((r - position.height + BOUND_MARGIN).max(0.0), false),
        None => (f64::INFINITY, false),
    }
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
/// A. Over the step, the tip is within `L / 2` of the middle place (`L` the
/// step's length) and the stock turns at most half the step's angle about
/// the middle angle, which moves no material more than `R * angle / 2`
/// (`R` the stock's farthest radius). Material over the tool's disc at any
/// place of the step is therefore over a disc `d = L / 2 + R * angle / 2`
/// wider at the middle place, and stands at most `d` higher above that tip
/// than above its own: the stock's exact top over the wider disc, plus `d`,
/// bounds the whole step. Where that bound is past the flutes the step is
/// halved, reading its middle exactly, until the bound is within them or
/// `d` is below [`REFINED`], where the bound is taken as the depth. A stock
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
    halvings: u32,
) {
    // A reading past the flutes already refuses the path.
    if worst.is_some_and(|w| w.depth > limit + SLACK) {
        return;
    }
    let stock = input.stock;
    let Some(farthest) = stock.max_radius() else {
        return;
    };
    // The quick bound: nothing is further from the axis than `farthest`.
    if farthest - p.0.height.min(q.0.height) + BOUND_MARGIN <= limit {
        return;
    }
    let middle = lerp(p.0, q.0, 0.5);
    let a = Degrees(0.5 * (p.1.0 + q.1.0));
    let length = (q.0.along - p.0.along)
        .hypot(q.0.across - p.0.across)
        .hypot(q.0.height - p.0.height);
    let turn = (q.1.0 - p.1.0).abs().to_radians();
    let reach = 0.5 * length
        + if turn > 0.0 {
            0.5 * farthest * turn
        } else {
            0.0
        };
    let Some(top) = stock.top_over_disc(
        input.rotary.to_model(middle, a),
        input.rotary.tool_axis(a),
        radius + reach,
    ) else {
        return;
    };
    let bound = top + reach;
    if !bound.is_finite() || bound <= limit + SLACK {
        return;
    }
    if reach <= REFINED || halvings >= MOST_HALVINGS {
        keep(worst, input, middle, a, bound, true);
        return;
    }
    let (depth, exact) = depth_at(stock, input, middle, a, radius);
    keep(worst, input, middle, a, depth, exact);
    between(worst, input, radius, limit, p, (middle, a), halvings + 1);
    between(worst, input, radius, limit, (middle, a), q, halvings + 1);
}

/// The deepest cut `passes` make in `input`'s stock: how far the stock
/// stands above the tool's tip along the tool axis, over the tool's width,
/// at its worst; `None` when the passes never meet the stock, or the stock
/// can be shown to stand at most `limit` above every tip without reading it.
pub fn deepest(input: &StrategyInput<'_>, passes: &[Pass], limit: f64) -> Option<Deepest> {
    deepest_of(input, passes, limit, false)
}

/// [`deepest`] over passes whose angles are `emitted` by assembly (see
/// [`along_move`]) or given by a strategy.
fn deepest_of(
    input: &StrategyInput<'_>,
    passes: &[Pass],
    limit: f64,
    emitted: bool,
) -> Option<Deepest> {
    let stock = input.stock;
    let radius = body_radius(input);
    let bound = stock.max_radius();
    let found = crate::parallel::map_each_in_order(passes, |pass| {
        let mut worst: Option<Deepest> = None;
        let mut previous: Option<&CutPoint> = None;
        for (i, point) in pass.points.iter().enumerate() {
            // Rapid points of a drilled hole are not cuts; the move to them
            // is not read, nor the next from them.
            if pass.rapids.contains(&i) {
                previous = None;
                continue;
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
                    let (depth, exact) = depth_at(stock, input, position, a, radius);
                    keep(&mut worst, input, position, a, depth, exact);
                }
                // And the stock between the places read.
                for (&p, &q) in chain.iter().zip(chain.iter().skip(1)) {
                    between(&mut worst, input, radius, limit, p, q, 0);
                }
            }
            previous = Some(point);
        }
        worst
    });
    found
        .into_iter()
        .flatten()
        .filter(|d| d.depth > 0.0)
        .fold(None, |best: Option<Deepest>, d| {
            if best.is_none_or(|b| d.depth > b.depth) {
                Some(d)
            } else {
                best
            }
        })
}

/// How far the stock stands above the rotary axis over the tool's width
/// with the tool centred at model X `x`, at its worst over a turn of A: the
/// depth a turn down to the axis there cuts (a free end, decision records
/// 0344 and 0509). Read as [`deepest`] reads a move, every [`A_STEP`] and
/// between, so it is exact wherever it is past `limit` (the flutes) and may
/// be lower elsewhere.
pub(crate) fn deepest_on_axis(input: &StrategyInput<'_>, x: f64, limit: f64) -> f64 {
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
    deepest_of(input, &turn, limit, true).map_or(0.0, |d| d.depth)
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

/// Refuses `passes` when the stock stands above the tool's tip, along the
/// tool axis, further than the tool's flutes reach (decision record 0509):
/// past the flutes the shank would meet the stock. `parameter` names the
/// setting the user acts on (Finishing layers, where the strategy has
/// them); the message gives the depth, where, the flute length and the
/// stock.
pub fn check(input: &StrategyInput<'_>, passes: &[Pass], parameter: Option<&str>) -> Result<()> {
    let flutes = input.tool.top();
    refuse_past_flutes(input, deepest(input, passes, flutes), parameter)
}

/// [`check`] on the path as assembly wrote it: every fed move (cuts, plunges,
/// leads and fed links; rapids are not cuts), from the pose before it, with
/// the angles assembly emitted. The generation checks this, not the passes
/// a strategy gives: the way A turns between two points (at an exact half
/// turn, decided by the turns assembly carries from earlier points and
/// passes and by its fit to the A range) is then the way the tool turns,
/// not a prediction of it (CodeRabbit on #534).
pub fn check_path(
    input: &StrategyInput<'_>,
    path: &crate::assemble::GeneratedPath,
    parameter: Option<&str>,
) -> Result<()> {
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
    refuse_past_flutes(input, deepest_of(input, &runs, flutes, true), parameter)
}

/// The refusal of [`check`] and [`check_path`] when `worst` is past the
/// flutes.
fn refuse_past_flutes(
    input: &StrategyInput<'_>,
    worst: Option<Deepest>,
    parameter: Option<&str>,
) -> Result<()> {
    let flutes = input.tool.top();
    let Some(worst) = worst else {
        return Ok(());
    };
    if worst.depth <= flutes + SLACK {
        return Ok(());
    }
    let layers = if parameter.is_some() {
        ", or turn on Finishing layers"
    } else {
        ""
    };
    let reason = if !worst.depth.is_finite() {
        format!(
            "the stock's size is not known, so the tool's {flutes:.3} mm of flute cannot be \
             checked against how deep it stands above the tool (at X {:.3}, A {:.1}). Define \
             the setup's stock",
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
            " (a bound from how far the stock reaches from the rotary axis: that stock is not \
             measured exactly across the tool)"
        };
        format!(
            "the stock {how} {:.3} mm above the tool's tip along its axis{bound} (at X {:.3}, \
             A {:.1}), more than the tool's {flutes:.3} mm of flute, so the shank would meet \
             the stock; {}. Use a tool with longer flutes, or rough the part first in this \
             setup{layers}",
            worst.depth,
            worst.x,
            worst.a,
            stock_words(input.stock),
        )
    };
    Err(match parameter {
        Some(name) => PathError::parameter(name, reason),
        None => PathError::Strategy(reason),
    })
}
