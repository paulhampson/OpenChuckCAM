// SPDX-License-Identifier: AGPL-3.0-or-later
//! Axial finishing: passes parallel to the rotary axis, stepping in A (OP-04;
//! work breakdown slice 12 item 2; decision record 0033 items 1 and 2).
//!
//! The tool is held at one A and dropped along X over the part, then A steps
//! to the next pass. The A positions are `360 / n` degrees apart, where `n` is
//! the smallest whole number of passes that keeps neighbouring passes no more
//! than `stepover` apart in arc length at the part's largest radius (or, with
//! a `scallop` height, no further apart than leaves that cusp), so the passes
//! close the circle exactly. Each pass is fitted by the same bisection as
//! helical finishing ([`crate::wrap_fit`]).

use openchuck_types::job::JobContext;
use openchuck_types::units::Degrees;

use crate::cutter;
use crate::error::{PathError, Result};
use crate::helical::CONSTRAINED_SPACING;
use crate::params::{ParameterKind, ParameterSchema, ParameterSpec, Parameters, Unit, names};
use crate::region::RegionConstraint;
use crate::scallop;
use crate::strategy::{Pass, Strategy, StrategyInput};
use crate::wrap_fit::{CHORD_FACTOR, Fit, fit_curve};

/// Axial finishing of the whole model, or of a region.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AxialFinishing;

/// The registry name of [`AxialFinishing`].
pub const NAME: &str = "axial_finishing";

/// Parameter names specific to axial finishing.
pub mod parameter {
    /// `zigzag` (alternate the direction of X) or `climb` (always the same way).
    pub const DIRECTION: &str = "direction";
    /// A of the first pass, degrees.
    pub const START_ANGLE: &str = "start_angle";
    /// The cusp height a ball-end or bull-nose tool may leave, mm; 0 is off.
    pub const SCALLOP: &str = "scallop";
    /// The furthest apart, in X, points of a pass are before refinement, mm.
    pub const MAX_X_STEP: &str = "max_x_step";
}

/// Most points one strategy run may make (NFR-07).
const MAX_SEEDS: f64 = 5.0e6;

impl Strategy for AxialFinishing {
    fn name(&self) -> &str {
        NAME
    }

    fn label(&self) -> &str {
        "Axial finishing"
    }

    fn schema(&self) -> ParameterSchema {
        ParameterSchema::with_common(vec![
            ParameterSpec {
                name: parameter::DIRECTION.to_owned(),
                label: "Direction".to_owned(),
                description: "Zig-zag alternates the direction of each pass along X; climb \
                              cuts every pass the same way."
                    .to_owned(),
                kind: ParameterKind::Choice {
                    options: vec!["zigzag".to_owned(), "climb".to_owned()],
                    default: "zigzag".to_owned(),
                },
            },
            ParameterSpec {
                name: parameter::START_ANGLE.to_owned(),
                label: "Start angle".to_owned(),
                description: "A of the first pass.".to_owned(),
                kind: ParameterKind::Number {
                    unit: Unit::Degrees,
                    min: None,
                    min_exclusive: false,
                    max: None,
                    default: Some(0.0),
                },
            },
            ParameterSpec {
                name: parameter::SCALLOP.to_owned(),
                label: "Scallop height".to_owned(),
                description: "The largest cusp between passes, for ball-end and bull-nose \
                              tools. The passes are as close as the smaller of this and the \
                              stepover needs. 0 turns it off."
                    .to_owned(),
                kind: ParameterKind::Number {
                    unit: Unit::Millimetres,
                    min: Some(0.0),
                    min_exclusive: false,
                    max: None,
                    default: Some(0.0),
                },
            },
            ParameterSpec {
                name: parameter::MAX_X_STEP.to_owned(),
                label: "Largest step along X".to_owned(),
                description: "The most X may advance between two points of a pass; points \
                              are added where the surface needs them."
                    .to_owned(),
                kind: ParameterKind::Number {
                    unit: Unit::Millimetres,
                    min: Some(0.0),
                    min_exclusive: true,
                    max: None,
                    default: Some(1.0),
                },
            },
        ])
    }

    fn honours_region(&self) -> bool {
        true
    }

    fn passes(
        &self,
        input: &StrategyInput<'_>,
        parameters: &Parameters,
        context: &JobContext<'_>,
    ) -> Result<Vec<Pass>> {
        passes(input, None, parameters, context)
    }

    fn passes_in_region(
        &self,
        input: &StrategyInput<'_>,
        region: &RegionConstraint<'_>,
        parameters: &Parameters,
        context: &JobContext<'_>,
    ) -> Result<Vec<Pass>> {
        passes(input, Some(region), parameters, context)
    }
}

/// The number of passes round the circle for `radius`, `stepover` and the
/// optional `scallop` height with tool arc `rho`.
fn pass_count(radius: f64, stepover: f64, scallop: Option<(f64, f64)>) -> Result<usize> {
    let mut step = stepover / radius;
    if let Some((rho, height)) = scallop {
        step = step.min(scallop::circumferential_step(rho, radius, height));
    }
    let n = (std::f64::consts::TAU / step).ceil();
    if !n.is_finite() || n > MAX_SEEDS {
        return Err(PathError::Parameter {
            name: names::STEPOVER.to_owned(),
            reason: "the stepover needs too many passes; increase it".to_owned(),
        });
    }
    Ok((n as usize).max(1))
}

fn passes(
    input: &StrategyInput<'_>,
    region: Option<&RegionConstraint<'_>>,
    parameters: &Parameters,
    context: &JobContext<'_>,
) -> Result<Vec<Pass>> {
    let stepover = parameters.number(names::STEPOVER)?;
    let tolerance = parameters.number(names::TOLERANCE)?;
    let leave = parameters.number(names::STOCK_TO_LEAVE)?;
    let start = parameters.number(parameter::START_ANGLE)?;
    let height = parameters.number(parameter::SCALLOP)?;
    let mut x_step = parameters.number(parameter::MAX_X_STEP)?;
    let zigzag = parameters.text(parameter::DIRECTION)? == "zigzag";

    let bounds = input.surface.bounds();
    let (x0, x1) = (bounds.min.x, bounds.max.x);
    let length = x1 - x0;
    if length.is_nan() || length <= 0.0 {
        return Err(PathError::Strategy(
            "the part has no length along the rotary axis to finish".to_owned(),
        ));
    }
    let radius = [bounds.min.y, bounds.max.y, bounds.min.z, bounds.max.z]
        .into_iter()
        .fold(0.0_f64, |r, v| r.max(v.abs()));
    if radius.is_nan() || radius <= 0.0 {
        return Err(PathError::Strategy(
            "the part has no radius about the rotary axis to finish".to_owned(),
        ));
    }
    let scallop = if height > 0.0 {
        Some((scallop::cusp_radius(input.tool)?, height))
    } else {
        None
    };
    let constrained = region.is_some_and(|r| !r.is_empty());
    if constrained {
        // A region constraint is only seen where the tool is dropped
        // (decision 0032 item 6; replaced by region offsets later in slice 12).
        x_step = x_step.min(CONSTRAINED_SPACING);
    }
    let count = pass_count(radius, stepover, scallop)?;
    let seeds = (length / x_step).ceil().max(1.0);
    if seeds * count as f64 > MAX_SEEDS {
        return Err(PathError::Parameter {
            name: parameter::MAX_X_STEP.to_owned(),
            reason: format!(
                "{count} passes of {seeds:.0} points each is too many; increase the stepover \
                 or the largest step"
            ),
        });
    }
    let seeds = seeds as usize;
    let grown = cutter::grow_profile(input.tool, leave)?;
    let step = 360.0 / count as f64;

    let mut out = Vec::new();
    for i in 0..count {
        context.check_cancelled()?;
        let a = Degrees(start + step * i as f64);
        let backwards = zigzag && i % 2 == 1;
        let fit = Fit {
            input,
            drop_profile: &grown,
            leave,
            x0: if backwards { x1 } else { x0 },
            length: if backwards { -length } else { length },
            start: a.0,
            sign: 1.0,
            total_angle: 0.0,
            chord: CHORD_FACTOR * tolerance,
            region,
            constrained,
        };
        out.extend(fit_curve(&fit, seeds, "axial finishing", context)?);
    }
    Ok(out)
}
