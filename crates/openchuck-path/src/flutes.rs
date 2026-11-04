// SPDX-License-Identifier: AGPL-3.0-or-later
//! The flute length against the depth of the cut (TL-04, SET-03, OP-12,
//! NFR-07, NFR-12; issue #530; decision record 0508).
//!
//! A pass that cuts all the stock above it in one go (any strategy that does
//! not cut in layers) needs flutes at least as long as the stock stands
//! above the tool's tip along the tool axis, over the tool's whole width:
//! past the flutes the shank, not a cutting edge, would meet the stock. The
//! depth is read from the stock itself, along the tool axis, at every cut
//! point and between them, so the worst case of box stock (a corner
//! uppermost over the rotary axis, its half diagonal from the axis), stock
//! not centred on the axis and stock left by earlier operations are all
//! measured as they are. [`deepest`] is the one measure every check uses;
//! [`check`] refuses a path whose deepest cut is past the flutes, saying
//! where, by how much and on which stock.
//!
//! A quick bound comes first: no material is further from the rotary axis
//! than [`StockModel::max_radius`], so at a tip `h` above the axis the stock
//! stands at most `max_radius - h` above it, whatever the stock's shape or
//! where it sits. Only where that bound is past the flutes is the stock read.

use openchuck_types::geometry::{Point3, UnitVec3};
use openchuck_types::units::Degrees;

use crate::error::{PathError, Result};
use crate::rotary::AxisPosition;
use crate::stock::StockModel;
use crate::strategy::{CutPoint, Pass, StrategyInput};

/// The largest step in A, degrees, between the places a move is read at.
const A_STEP: f64 = 0.5;

/// The largest step along a move, mm, between the places it is read at.
const LENGTH_STEP: f64 = 0.5;

/// How far past the flutes, mm, the stock may stand before a path is
/// refused: rounding only.
const SLACK: f64 = 1e-6;

/// How far inside the flutes, mm, the quick bound must be before a place is
/// left unread: a stock model's farthest radius may be measured at cell
/// centres (the simulation's columns), so the bound is not trusted closer.
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
/// axis; zero where there is none. Read on rings of the disc, then refined
/// about the highest reading.
pub(crate) fn depth_at(
    stock: &dyn StockModel,
    input: &StrategyInput<'_>,
    position: AxisPosition,
    a: Degrees,
    radius: f64,
) -> f64 {
    let axis = input.rotary.tool_axis(a).get();
    let Ok(down) = UnitVec3::new(axis * -1.0) else {
        return 0.0;
    };
    let reach = stock.max_radius().unwrap_or(0.0).max(0.0) + position.height.abs() + 1.0;
    let read = |along: f64, across: f64| -> f64 {
        let tip = input.rotary.to_model(
            AxisPosition {
                along: position.along + along,
                across: position.across + across,
                height: position.height,
            },
            a,
        );
        let from: Point3 = tip + axis * reach;
        stock
            .ray_entry(from, down, 2.0 * reach)
            .map_or(0.0, |d| (reach - d).max(0.0))
    };
    const AROUND: usize = 16;
    let mut best = (read(0.0, 0.0), 0.0, 0.0);
    for ring in [0.5, 1.0] {
        for k in 0..AROUND {
            let t = std::f64::consts::TAU * k as f64 / AROUND as f64;
            let (u, v) = (ring * radius * t.cos(), ring * radius * t.sin());
            let d = read(u, v);
            if d > best.0 {
                best = (d, u, v);
            }
        }
    }
    // A pattern search about the highest reading, kept in the disc: the top
    // of a box over the disc peaks at an edge between the readings.
    let mut step = 0.25 * radius;
    for _ in 0..12 {
        let mut moved = false;
        for (du, dv) in [(step, 0.0), (-step, 0.0), (0.0, step), (0.0, -step)] {
            let (u, v) = (best.1 + du, best.2 + dv);
            if u.hypot(v) > radius {
                continue;
            }
            let d = read(u, v);
            if d > best.0 {
                best = (d, u, v);
                moved = true;
            }
        }
        if !moved {
            step *= 0.5;
        }
    }
    best.0
}

/// The places along the move from `from` to `to` the stock is read at: the
/// end, and enough between that A and the tip step at most [`A_STEP`] and
/// [`LENGTH_STEP`]; each fixed to the machine, with its A.
fn along_move(
    input: &StrategyInput<'_>,
    from: Option<&CutPoint>,
    to: &CutPoint,
) -> Vec<(AxisPosition, Degrees)> {
    let b = input.rotary.to_axis(to.tip, to.a);
    let Some(from) = from else {
        return vec![(b, to.a)];
    };
    let a0 = input.rotary.to_axis(from.tip, from.a);
    let length = (b.along - a0.along)
        .hypot(b.across - a0.across)
        .hypot(b.height - a0.height);
    let turn = (to.a.0 - from.a.0).abs();
    let steps = ((turn / A_STEP).max(length / LENGTH_STEP).ceil() as usize).clamp(1, 10_000);
    (1..=steps)
        .map(|i| {
            let t = i as f64 / steps as f64;
            let lerp = |p: f64, q: f64| p + t * (q - p);
            (
                AxisPosition {
                    along: lerp(a0.along, b.along),
                    across: lerp(a0.across, b.across),
                    height: lerp(a0.height, b.height),
                },
                Degrees(lerp(from.a.0, to.a.0)),
            )
        })
        .collect()
}

/// The deepest cut `passes` make in `input`'s stock: how far the stock
/// stands above the tool's tip along the tool axis, over the tool's width,
/// at its worst; `None` when the passes never meet the stock, or the stock
/// can be shown to stand at most `limit` above every tip without reading it.
pub fn deepest(input: &StrategyInput<'_>, passes: &[Pass], limit: f64) -> Option<Deepest> {
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
            for (position, a) in along_move(input, previous, point) {
                // Nothing is further from the axis than the bound, so the
                // stock stands at most this far above the tip. The height is
                // signed: a tip below the axis (a cut past the middle of the
                // stock) adds its depth below it, so it is skipped less.
                if bound.is_some_and(|r| r - position.height + BOUND_MARGIN <= limit) {
                    continue;
                }
                let depth = depth_at(stock, input, position, a, radius);
                if worst.is_none_or(|w| depth > w.depth) {
                    worst = Some(Deepest {
                        depth,
                        x: input.rotary.to_model(position, a).x,
                        a: a.0.rem_euclid(360.0),
                    });
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
/// with the tool centred at model X `x`, at its worst over a turn of A read
/// every [`A_STEP`]: the depth a turn down to the axis there cuts (a free
/// end, decision records 0344 and 0508).
pub(crate) fn deepest_on_axis(input: &StrategyInput<'_>, x: f64) -> f64 {
    let radius = body_radius(input);
    let steps = (360.0 / A_STEP).round() as usize;
    (0..steps)
        .map(|k| {
            let a = Degrees(A_STEP * k as f64);
            let position = AxisPosition {
                along: input.rotary.to_axis(Point3::new(x, 0.0, 0.0), a).along,
                across: 0.0,
                height: 0.0,
            };
            depth_at(input.stock, input, position, a, radius)
        })
        .fold(0.0, f64::max)
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
/// tool axis, further than the tool's flutes reach (decision record 0508):
/// past the flutes the shank would meet the stock. `parameter` names the
/// setting the user acts on (Finishing layers, where the strategy has
/// them); the message gives the depth, where, the flute length and the
/// stock.
pub fn check(input: &StrategyInput<'_>, passes: &[Pass], parameter: Option<&str>) -> Result<()> {
    let flutes = input.tool.top();
    let Some(worst) = deepest(input, passes, flutes) else {
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
    let reason = format!(
        "the stock stands up to {:.3} mm above the tool's tip along its axis (at X {:.3}, A \
         {:.1}), more than the tool's {flutes:.3} mm of flute, so the shank would meet the \
         stock; {}. Use a tool with longer flutes, or rough the part first in this setup{layers}",
        worst.depth,
        worst.x,
        worst.a,
        stock_words(input.stock),
    );
    Err(match parameter {
        Some(name) => PathError::parameter(name, reason),
        None => PathError::Strategy(reason),
    })
}
