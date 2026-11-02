// SPDX-License-Identifier: AGPL-3.0-or-later
//! Extending the passes past the stock (OP-24; decision record 0212 part B).
//!
//! A strategy that honours the option carries its passes past the chosen
//! ends of the stock (and, for Facing on a flat, its sides) until the near
//! side of the tool is [`parameter::EXTEND_DISTANCE`] past the stock's edge,
//! so the tool leaves the stock completely and leaves no skin or witness line
//! there. The option is off by default, so a path without it is unchanged.
//!
//! **Refused, not cut short (NFR-07, NFR-12).** An end held by the
//! workholding (SET-09) cannot be extended past: generation says so, naming
//! the end, and writes no path. A distance below zero or above the tool's
//! diameter is refused by name. The stock's ends come from the stock model
//! ([`StockModel::x_extent`]); a model that gives none is refused. The host
//! checks the extended path against the machine's travel.
//!
//! The default distance (decision record 0212 part B item 2) depends on the
//! tool, so a parameter schema cannot hold it: the schema's default is zero,
//! and the application fills [`default_distance`] for an operation that has
//! no value of its own.

use openchuck_types::units::{Degrees, Millimetres};

use crate::error::{PathError, Result};
use crate::params::{ParameterKind, ParameterSpec, Parameters, Unit};
use crate::rotary::AxisPosition;
use crate::strategy::{CutPoint, Pass, StrategyInput};
use openchuck_types::tool::profile::RevolvedProfile;

/// Parameter names and values of the option.
pub mod parameter {
    /// Which ends of the stock the passes run past: [`NO`], [`BOTH_ENDS`],
    /// [`START_END`] or [`FAR_END`].
    pub const EXTEND: &str = "extend";
    /// Also past the sides of the stock (Facing on a flat only).
    pub const EXTEND_SIDES: &str = "extend_sides";
    /// How far past the stock's edge the near side of the tool goes, mm.
    pub const EXTEND_DISTANCE: &str = "extend_distance";
    /// Not past the stock: the default, as before the option.
    pub const NO: &str = "no";
    /// Past both ends of the stock.
    pub const BOTH_ENDS: &str = "both ends";
    /// Past the start of the stock (its low X end).
    pub const START_END: &str = "start end";
    /// Past the far end of the stock (its high X end).
    pub const FAR_END: &str = "far end";
}

/// The parameters of the option, with the sides flag when `sides`.
pub fn specs(sides: bool) -> Vec<ParameterSpec> {
    let mut out = vec![
        ParameterSpec {
            name: parameter::EXTEND.to_owned(),
            label: "Extend past the stock".to_owned(),
            description: "Carries the passes past the chosen ends of the stock, so the tool \
                          leaves the stock completely and leaves no thin skin or line at its \
                          edge. The start end is the low X end of the stock, the far end the \
                          high X end. An end held by the chuck cannot be extended past."
                .to_owned(),
            kind: ParameterKind::Choice {
                options: vec![
                    parameter::NO.to_owned(),
                    parameter::BOTH_ENDS.to_owned(),
                    parameter::START_END.to_owned(),
                    parameter::FAR_END.to_owned(),
                ],
                default: parameter::NO.to_owned(),
            },
        },
        ParameterSpec {
            name: parameter::EXTEND_DISTANCE.to_owned(),
            label: "Distance past the stock".to_owned(),
            description: "How far past the edge of the stock the near side of the tool goes, \
                          from zero up to the tool's diameter. Starts at a tenth of the tool's \
                          diameter."
                .to_owned(),
            kind: ParameterKind::Number {
                unit: Unit::Millimetres,
                min: Some(0.0),
                min_exclusive: false,
                max: None,
                default: Some(0.0),
            },
        },
    ];
    if sides {
        out.insert(
            1,
            ParameterSpec {
                name: parameter::EXTEND_SIDES.to_owned(),
                label: "Also past the sides".to_owned(),
                description: "Carries the passes past the sides of the stock as well as its ends."
                    .to_owned(),
                kind: ParameterKind::Flag { default: false },
            },
        );
    }
    out
}

/// The default distance past the stock for a tool of `diameter` mm: a tenth
/// of the diameter, rounded to 0.05 mm, at least 0.1 mm (decision record
/// 0212 part B item 2; 0.3 mm for a 3.175 mm tool, 0.6 mm for 6 mm).
pub fn default_distance(diameter: f64) -> f64 {
    let rounded = (0.1 * diameter / 0.05).round() * 0.05;
    // Rounded to a whole number of hundredths, so 0.30000000000000004 is 0.3.
    ((rounded * 100.0).round() / 100.0).max(0.1)
}

/// The option as one operation asks for it, with where the passes end.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Extension {
    /// Where the tool's tip goes to past the start of the stock, model X.
    pub start: Option<f64>,
    /// Where the tool's tip goes to past the far end of the stock, model X.
    pub end: Option<f64>,
    /// Past the sides too.
    pub sides: bool,
    /// The distance from the stock's edge to the tool's near side.
    pub distance: f64,
    /// The tool's cutting radius.
    pub radius: f64,
    /// The stock's ends, model X.
    pub stock: (f64, f64),
}

impl Extension {
    /// The option of an operation, or `None` when it is off. Refuses a
    /// distance above the tool's diameter, a stock with no known ends, and an
    /// end held by the workholding.
    pub(crate) fn read(
        input: &StrategyInput<'_>,
        parameters: &Parameters,
        label: &str,
    ) -> Result<Option<Self>> {
        let (start, end) = match parameters.text(parameter::EXTEND)? {
            parameter::BOTH_ENDS => (true, true),
            parameter::START_END => (true, false),
            parameter::FAR_END => (false, true),
            _ => return Ok(None),
        };
        let radius = input.tool.cutting_radius();
        let distance = parameters.number(parameter::EXTEND_DISTANCE)?;
        if !distance.is_finite() || distance > 2.0 * radius + 1e-9 {
            return Err(PathError::Parameter {
                name: parameter::EXTEND_DISTANCE.to_owned(),
                reason: format!(
                    "{distance} mm is more than the tool's diameter ({} mm); the most is the \
                     diameter",
                    2.0 * radius
                ),
            });
        }
        let sides = matches!(
            parameters.get(parameter::EXTEND_SIDES),
            Some(crate::params::ParameterValue::Flag(true))
        );
        let (x0, x1) = input.stock.x_extent().ok_or_else(|| {
            PathError::Strategy(format!(
                "{label} cannot extend past the stock: the stock model gives no ends"
            ))
        })?;
        let mid = crate::strategy::part_middle(input);
        for (wanted, which, held) in [
            (
                start,
                "start",
                input.keep_out.iter().any(|&(_, b)| b <= mid),
            ),
            (end, "end", input.keep_out.iter().any(|&(a, _)| a >= mid)),
        ] {
            if wanted && held {
                return Err(PathError::Parameter {
                    name: parameter::EXTEND.to_owned(),
                    reason: format!(
                        "the {which} of the stock is held by the workholding, so {label} cannot \
                         extend past it (SET-09); extend past the other end only, or not at all"
                    ),
                });
            }
        }
        let reach = radius + distance;
        Ok(Some(Self {
            start: start.then_some(x0 - reach),
            end: end.then_some(x1 + reach),
            sides,
            distance,
            radius,
            stock: (x0, x1),
        }))
    }

    /// `range` widened to reach the extended ends.
    pub(crate) fn widen(&self, range: (f64, f64)) -> (f64, f64) {
        (
            self.start.map_or(range.0, |s| range.0.min(s)),
            self.end.map_or(range.1, |e| range.1.max(e)),
        )
    }

    /// `x` moved onto the stock's end where it lies past an extended end:
    /// the position whose material decides whether the tool cuts there.
    pub(crate) fn onto_stock(&self, x: f64) -> f64 {
        if self.start.is_some() && x < self.stock.0 {
            self.stock.0
        } else if self.end.is_some() && x > self.stock.1 {
            self.stock.1
        } else {
            x
        }
    }

    /// Finishing passes carried on past the extended ends of the stock
    /// (decision record 0212 part B item 3): a pass that starts or ends at an
    /// end of the part (`part`, model X) runs on along X, at the height of
    /// its end point along the tool axis and its A, until the tool's tip
    /// reaches the extended end. The tool is dropped on the target at each
    /// point of the run-out (`grown` is the tool grown by the stock to
    /// leave), and rises over anything there, a tab or the part, rather than
    /// cutting it.
    pub(crate) fn run_out(
        &self,
        passes: Vec<Pass>,
        input: &StrategyInput<'_>,
        grown: &RevolvedProfile,
        leave: f64,
        part: (f64, f64),
    ) -> Vec<Pass> {
        let slack = 1e-3;
        passes
            .into_iter()
            .map(|mut pass| {
                let ends = (pass.points.first().copied(), pass.points.last().copied());
                let (Some(first), Some(last)) = ends else {
                    return pass;
                };
                let towards = |p: CutPoint| -> Option<f64> {
                    let x = input.rotary.to_axis(p.tip, p.a).along;
                    match (self.start, self.end) {
                        (Some(s), _) if x <= part.0 + slack && s < x => Some(s),
                        (_, Some(e)) if x >= part.1 - slack && e > x => Some(e),
                        _ => None,
                    }
                };
                let before = towards(first).map(|to| self.line(first, to, input, grown, leave));
                let after = towards(last).map(|to| self.line(last, to, input, grown, leave));
                if before.is_none() && after.is_none() {
                    return pass;
                }
                let shift = before.as_ref().map_or(0, Vec::len);
                let mut points = before.map_or_else(Vec::new, |mut b| {
                    b.reverse();
                    b
                });
                points.append(&mut pass.points);
                if let Some(mut a) = after {
                    points.append(&mut a);
                }
                pass.points = points;
                pass.rapids = pass.rapids.iter().map(|i| i + shift).collect();
                pass
            })
            .collect()
    }

    /// The points from `from` (not included) along X to the tip at `to`, at
    /// `from`'s height and A, raised over the target where it stands higher.
    fn line(
        &self,
        from: CutPoint,
        to: f64,
        input: &StrategyInput<'_>,
        grown: &RevolvedProfile,
        leave: f64,
    ) -> Vec<CutPoint> {
        let rotary = input.rotary;
        let at = rotary.to_axis(from.tip, from.a);
        let gap = to - at.along;
        let spacing = self.radius.clamp(0.1, 1.0);
        let n = (gap.abs() / spacing).ceil().max(1.0) as usize;
        (1..=n)
            .map(|k| {
                let along = at.along + gap * k as f64 / n as f64;
                let line = rotary.tool_line(Millimetres(along), from.a, Millimetres(at.across));
                let floor = input
                    .surface
                    .drop_tool(grown, &line)
                    .map(|c| rotary.to_axis(c.tip, from.a).height + leave);
                let height = floor.map_or(at.height, |f| f.max(at.height));
                let tip = rotary.to_model(
                    AxisPosition {
                        along,
                        across: at.across,
                        height,
                    },
                    Degrees(from.a.0),
                );
                CutPoint {
                    tip,
                    a: from.a,
                    contact: tip,
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::default_distance;

    #[test]
    fn req_op_24_the_default_distance_is_a_tenth_of_the_tool_diameter() {
        // Decision record 0212 part B item 2: rounded to 0.05 mm, at least
        // 0.1 mm.
        for (diameter, distance) in [
            (3.175, 0.3),
            (4.0, 0.4),
            (6.0, 0.6),
            (1.0, 0.1),
            (0.5, 0.1),
            (12.7, 1.25),
        ] {
            assert!(
                (default_distance(diameter) - distance).abs() < 1e-12,
                "{diameter}: {}",
                default_distance(diameter)
            );
        }
    }
}
