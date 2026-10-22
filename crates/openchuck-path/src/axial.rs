// SPDX-License-Identifier: AGPL-3.0-or-later
//! Axial finishing: passes parallel to the rotary axis, stepping in A (OP-04;
//! work breakdown slice 12 item 2; decision record 0034 items 1 and 2).
//!
//! The tool is held at one A and dropped along X over the part, then A steps
//! to the next pass. The A positions are `360 / n` degrees apart, where `n` is
//! the smallest whole number of passes that keeps neighbouring passes no more
//! than `stepover` apart in arc length at the part's largest radius (or, with
//! a `scallop` height, no further apart than leaves that cusp), so the passes
//! close the circle exactly. Each pass is fitted by the same bisection as
//! helical finishing ([`crate::wrap_fit`]).

use openchuck_types::job::JobContext;
use openchuck_types::tool::profile::RevolvedProfile;
use openchuck_types::units::Degrees;

use crate::candidates::DevIndex;
use crate::cutter;
use crate::error::{PathError, Result};
use crate::extend::Extension;
use crate::helical::CONSTRAINED_SPACING;
use crate::params::{ParameterKind, ParameterSchema, ParameterSpec, Parameters, Unit, names};
use crate::region::RegionConstraint;
use crate::scallop;
use crate::strategy::{LeaveZone, Pass, Strategy, StrategyInput};
use crate::wrap_fit::{
    CHORD_FACTOR, Fit, ZoneFit, fit_region, region_index, region_spacing, zone_fits,
};

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

    fn description(&self) -> &str {
        "Cuts the surface in straight passes along the length of the part, turning the part a small step between passes. Use it where marks running along the part suit the job better than a spiral."
    }

    fn schema(&self) -> ParameterSchema {
        ParameterSchema::with_common(
            vec![
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
            ]
            .into_iter()
            .chain(crate::extend::specs(false))
            .collect(),
        )
    }

    fn honours_region(&self) -> bool {
        true
    }

    fn parameter_warnings(&self, tool: &RevolvedProfile, parameters: &Parameters) -> Vec<String> {
        parameters
            .number(parameter::SCALLOP)
            .ok()
            .and_then(|height| scallop::warning(tool, height))
            .into_iter()
            .collect()
    }

    fn honours_leave_zones(&self) -> bool {
        true
    }

    fn passes_in_zones(
        &self,
        input: &StrategyInput<'_>,
        region: Option<&RegionConstraint<'_>>,
        zones: &[LeaveZone<'_>],
        parameters: &Parameters,
        context: &JobContext<'_>,
    ) -> Result<Vec<Pass>> {
        passes(input, region, zones, parameters, context)
    }

    fn passes(
        &self,
        input: &StrategyInput<'_>,
        parameters: &Parameters,
        context: &JobContext<'_>,
    ) -> Result<Vec<Pass>> {
        passes(input, None, &[], parameters, context)
    }

    fn passes_in_region(
        &self,
        input: &StrategyInput<'_>,
        region: &RegionConstraint<'_>,
        parameters: &Parameters,
        context: &JobContext<'_>,
    ) -> Result<Vec<Pass>> {
        passes(input, Some(region), &[], parameters, context)
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

/// What axial and indexed finishing share: the part's extent, the grown tool
/// and the fitting limits, and the fit of one pass along X at one A.
pub(crate) struct AlongX<'a> {
    input: &'a StrategyInput<'a>,
    region: Option<&'a RegionConstraint<'a>>,
    grown: RevolvedProfile,
    zones: Vec<ZoneFit<'a>>,
    leave: f64,
    x0: f64,
    x1: f64,
    seeds: usize,
    chord: f64,
    constrained: bool,
    /// The selected region in the development, when it is known.
    candidates: Option<DevIndex>,
    /// Points inside the intervals that reach the region, mm of path.
    spacing: f64,
    /// The part's largest radius about the rotary axis.
    pub(crate) radius: f64,
    /// Alternate the direction of each pass.
    pub(crate) zigzag: bool,
    /// Past the stock's ends, when the operation asks (OP-24).
    extension: Option<Extension>,
}

impl<'a> AlongX<'a> {
    /// Reads the parts of `parameters` that every pass along X uses. The
    /// number of passes is checked with [`check_passes`](Self::check_passes).
    pub(crate) fn read(
        input: &'a StrategyInput<'a>,
        region: Option<&'a RegionConstraint<'a>>,
        zones: &[LeaveZone<'a>],
        parameters: &Parameters,
    ) -> Result<Self> {
        let tolerance = parameters.number(names::TOLERANCE)?;
        let leave = parameters.number(names::STOCK_TO_LEAVE)?;
        let mut x_step = parameters.number(parameter::MAX_X_STEP)?;
        let zigzag = parameters.text(parameter::DIRECTION)? == "zigzag";
        let label = if parameters.get(crate::indexed::parameter::ANGLES).is_some() {
            "indexed finishing"
        } else {
            "axial finishing"
        };
        let extension = Extension::read(input, parameters, label)?;

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
        let constrained = region.is_some_and(|r| !r.is_empty());
        // With a selected region the pass is only fitted where it can reach
        // the region (decision record 0034 item 5). Without one, or when the
        // region cannot be developed, a region constraint is only seen where
        // the tool is dropped (decision 0032 item 6), so points are never
        // further apart than `CONSTRAINED_SPACING`.
        let most_leave = zones.iter().fold(leave, |m, z| m.max(z.leave));
        let candidates = region_index(input, region, radius, most_leave, tolerance);
        if constrained && candidates.is_none() {
            x_step = x_step.min(CONSTRAINED_SPACING);
        }
        let seeds = (length / x_step).ceil().max(1.0);
        if seeds > MAX_SEEDS {
            return Err(PathError::Parameter {
                name: parameter::MAX_X_STEP.to_owned(),
                reason: format!("{seeds:.0} points per pass is too many; increase the step"),
            });
        }
        Ok(Self {
            input,
            region,
            grown: cutter::grow_profile(input.tool, leave)?,
            zones: zone_fits(zones, input.tool)?,
            leave,
            x0,
            x1,
            seeds: seeds as usize,
            chord: CHORD_FACTOR * tolerance,
            constrained,
            candidates,
            spacing: region_spacing(tolerance, CONSTRAINED_SPACING),
            radius,
            zigzag,
            extension,
        })
    }

    /// Refuses a job of `count` passes that would make too many points.
    pub(crate) fn check_passes(&self, count: usize) -> Result<()> {
        if self.candidates.is_none() && self.seeds as f64 * count as f64 > MAX_SEEDS {
            return Err(PathError::Parameter {
                name: parameter::MAX_X_STEP.to_owned(),
                reason: format!(
                    "{count} passes of {} points each is too many; increase the stepover \
                     or the largest step",
                    self.seeds
                ),
            });
        }
        Ok(())
    }

    /// The pass at `a`, running from the high end of X when `backwards`.
    pub(crate) fn pass(
        &self,
        a: Degrees,
        backwards: bool,
        context: &JobContext<'_>,
    ) -> Result<Vec<Pass>> {
        let length = self.x1 - self.x0;
        let fit = Fit {
            input: self.input,
            drop_profile: &self.grown,
            leave: self.leave,
            zones: &self.zones,
            x0: if backwards { self.x1 } else { self.x0 },
            length: if backwards { -length } else { length },
            start: a.0,
            sign: 1.0,
            total_angle: 0.0,
            chord: self.chord,
            region: self.region,
            constrained: self.constrained,
            candidates: self.candidates.as_ref(),
            spacing: self.spacing,
            thin: false,
            max_dt: 1.0 / self.seeds as f64,
            wanted: None,
        };
        let passes = fit_region(&fit, self.seeds, "axial finishing", context)?;
        Ok(match self.extension {
            Some(e) => e.run_out(
                passes,
                self.input,
                &self.grown,
                self.leave,
                (self.x0, self.x1),
            ),
            None => passes,
        })
    }
}

fn passes(
    input: &StrategyInput<'_>,
    region: Option<&RegionConstraint<'_>>,
    zones: &[LeaveZone<'_>],
    parameters: &Parameters,
    context: &JobContext<'_>,
) -> Result<Vec<Pass>> {
    let stepover = parameters.number(names::STEPOVER)?;
    let start = parameters.number(parameter::START_ANGLE)?;
    let height = parameters.number(parameter::SCALLOP)?;
    let along = AlongX::read(input, region, zones, parameters)?;
    // A tool with no closed form for a cusp is warned about
    // ([`Strategy::parameter_warnings`]) and the stepover alone is used.
    let scallop = if height > 0.0 {
        scallop::cusp_radius(input.tool).map(|rho| (rho, height))
    } else {
        None
    };
    let count = pass_count(along.radius, stepover, scallop)?;
    along.check_passes(count)?;
    let step = 360.0 / count as f64;

    let mut out = Vec::new();
    for i in 0..count {
        context.check_cancelled()?;
        let a = Degrees(start + step * i as f64);
        out.extend(along.pass(a, along.zigzag && i % 2 == 1, context)?);
    }
    Ok(out)
}
