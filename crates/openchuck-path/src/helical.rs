// SPDX-License-Identifier: AGPL-3.0-or-later
//! Helical finishing on the whole model: the first wrap-family strategy
//! (OP-04; work breakdown slice 5 item 5, completed in slice 12; decision
//! record 0018 item 12).
//!
//! A vertical tool over the rotary axis follows a helix about it: A turns
//! once every `stepover` mm of X (the pitch), so the passes are `stepover`
//! apart along the axis. In the development of the cylinder (x, s = R A) the
//! helix is a straight line, which is why the strategy needs no 2D offset of
//! its own; the region offsets of [`crate::unroll`] arrive with selections in
//! slice 12. At each point the tool is dropped onto the surface
//! ([`crate::SurfaceSource::drop_tool`]), so the cutter rides the surface
//! whatever its shape.
//!
//! **Path fitting.** The machine moves in a straight line in X, Y, Z and A
//! between two points, so the points are chosen by bisection: a move is split
//! until the tool, at the middle of the move and at the middles of its halves
//! on the straight machine path, is within [`CHORD_FACTOR`] times the
//! toolpath tolerance of where the tool would be if dropped there, measured
//! **along the tool's contact normal** (the factor settles OI-08, decision
//! record 0018 item 12; the normal, decision 0113 item 4; the quarter points,
//! decision 0115). The angle step never exceeds `max_angle_step`.
//!
//! **Stock to leave** drops a tool grown by the amount
//! ([`crate::cutter::grow_profile`]), so the surface is left `stock_to_leave`
//! mm proud everywhere, and the cutter contact is taken on the original tool.
//!
//! Where the tool is over no material (past the ends of a part whose
//! surface does not reach under it) the pass is split there and the shared
//! services link the pieces.

use openchuck_types::geometry::Point3;
use openchuck_types::job::JobContext;
use openchuck_types::tool::profile::RevolvedProfile;

use crate::cutter;
use crate::error::{PathError, Result};
use crate::extend::Extension;
use crate::faces;
use crate::params::{ParameterKind, ParameterSchema, ParameterSpec, Parameters, Unit, names};
use crate::region::RegionConstraint;
use crate::scallop;
use crate::strategy::{LeaveZone, Pass, Strategy, StrategyInput, StrategyOutput};
use crate::wrap_fit::{Fit, ZoneFit, fit_region, region_index, region_spacing, zone_fits};

pub use crate::wrap_fit::CHORD_FACTOR;

/// The furthest apart, in millimetres of path at the part's largest radius,
/// that the points of a helix are when a region constraint applies.
pub const CONSTRAINED_SPACING: f64 = 0.1;

/// Most points one strategy run may make; a larger job is refused rather
/// than run out of memory (NFR-07).
const MAX_SEEDS: f64 = 5.0e6;

/// Helical finishing of the whole model.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HelicalFinishing;

/// The registry name of [`HelicalFinishing`].
pub const NAME: &str = "helical_finishing";

/// Parameter names specific to helical finishing.
pub mod parameter {
    /// Which way A turns as X increases: `positive` or `negative`.
    pub const ROTATION: &str = "rotation";
    /// A at the start of the helix, degrees.
    pub const START_ANGLE: &str = "start_angle";
    /// The largest A step between points, degrees.
    pub const MAX_ANGLE_STEP: &str = "max_angle_step";
    /// The cusp height a ball-end or bull-nose tool may leave, mm; 0 is off.
    pub const SCALLOP: &str = "scallop";
}

impl Strategy for HelicalFinishing {
    /// Exact contacts on a STEP part: measured within the limits of OI-835
    /// (decision 0277).
    fn exact_contact(&self) -> bool {
        true
    }

    fn name(&self) -> &str {
        NAME
    }

    fn label(&self) -> &str {
        "Helical finishing"
    }

    fn description(&self) -> &str {
        "Follows the surface in a spiral around the part as the tool moves along its length. Use it for a general finish on round shapes. Selections and gaps can split the spiral into separate cuts, and the tool may not reach tight inside corners."
    }

    fn schema(&self) -> ParameterSchema {
        ParameterSchema::with_common(
            vec![
                ParameterSpec {
                    name: parameter::ROTATION.to_owned(),
                    label: "Rotation".to_owned(),
                    description: "Which way A turns as X increases along the helix.".to_owned(),
                    kind: ParameterKind::Choice {
                        options: vec!["positive".to_owned(), "negative".to_owned()],
                        default: "positive".to_owned(),
                    },
                },
                ParameterSpec {
                    name: parameter::START_ANGLE.to_owned(),
                    label: "Start angle".to_owned(),
                    description: "A at the lowest X of the part, where the helix starts."
                        .to_owned(),
                    kind: ParameterKind::Number {
                        unit: Unit::Degrees,
                        min: None,
                        min_exclusive: false,
                        max: None,
                        default: Some(0.0),
                    },
                },
                ParameterSpec {
                    name: parameter::MAX_ANGLE_STEP.to_owned(),
                    label: "Largest angle step".to_owned(),
                    description: "The most A may turn between two points of the path; points are \
                              added where the surface needs them."
                        .to_owned(),
                    kind: ParameterKind::Number {
                        unit: Unit::Degrees,
                        min: Some(0.0),
                        min_exclusive: true,
                        max: Some(90.0),
                        default: Some(5.0),
                    },
                },
                ParameterSpec {
                    name: parameter::SCALLOP.to_owned(),
                    label: "Scallop height".to_owned(),
                    description: "The largest cusp between turns, for ball-end and bull-nose \
                              tools. The pitch is the smaller of this and the stepover \
                              allows. 0 turns it off."
                        .to_owned(),
                    kind: ParameterKind::Number {
                        unit: Unit::Millimetres,
                        min: Some(0.0),
                        min_exclusive: false,
                        max: None,
                        default: Some(0.0),
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
        passes(input, region, zones, parameters, context).map(|(p, ..)| p)
    }

    fn passes(
        &self,
        input: &StrategyInput<'_>,
        parameters: &Parameters,
        context: &JobContext<'_>,
    ) -> Result<Vec<Pass>> {
        passes(input, None, &[], parameters, context).map(|(p, ..)| p)
    }

    fn passes_in_region(
        &self,
        input: &StrategyInput<'_>,
        region: &RegionConstraint<'_>,
        parameters: &Parameters,
        context: &JobContext<'_>,
    ) -> Result<Vec<Pass>> {
        passes(input, Some(region), &[], parameters, context).map(|(p, ..)| p)
    }

    fn passes_with_notes(
        &self,
        input: &StrategyInput<'_>,
        region: Option<&RegionConstraint<'_>>,
        parameters: &Parameters,
        context: &JobContext<'_>,
    ) -> Result<StrategyOutput> {
        let (passes, turns, held) = passes(input, region, &[], parameters, context)?;
        Ok(StrategyOutput {
            passes,
            notes: faces::note(&turns)
                .into_iter()
                .chain(faces::held_note(&held))
                .collect(),
            warnings: Vec::new(),
        })
    }
}

/// The passes of one whole turn of A at constant `x`, from A `start` turning
/// by `sign`, with the tool across the rotary axis dropped onto the surface
/// as helical finishing drops it (`grown` by `leave`), no more than `step`
/// degrees between seeds, each part of the surface left its own stock where
/// `zones` gives one, cut only where `wanted` (when given) takes the
/// contact: a face's turn (decision record 0297), for helical finishing, the
/// simultaneous spiral and rest machining.
#[allow(clippy::too_many_arguments)]
pub(crate) fn turn_passes(
    input: &StrategyInput<'_>,
    region: Option<&RegionConstraint<'_>>,
    (grown, leave, zones): (&RevolvedProfile, f64, &[ZoneFit<'_>]),
    tolerance: f64,
    (x, start, sign): (f64, f64, f64),
    step: f64,
    wanted: Option<&(dyn Fn(Point3) -> bool + Send + Sync)>,
    context: &JobContext<'_>,
) -> Result<Vec<Pass>> {
    let n = (360.0 / step).ceil().max(1.0) as usize;
    let fit = Fit {
        input,
        drop_profile: grown,
        leave,
        zones,
        x0: x,
        length: 0.0,
        start,
        sign,
        total_angle: 360.0,
        chord: CHORD_FACTOR * tolerance,
        region,
        constrained: region.is_some_and(|r| !r.is_empty()),
        candidates: None,
        spacing: region_spacing(tolerance, CONSTRAINED_SPACING),
        thin: false,
        max_dt: 1.0 / n as f64,
        wanted,
    };
    fit_region(&fit, n, "a turn at a face", context)
}

/// The passes of helical finishing, limited to `region` when given, with
/// the X of the turns it made at faces across the axis and of those a held
/// region kept it from.
fn passes(
    input: &StrategyInput<'_>,
    region: Option<&RegionConstraint<'_>>,
    zones: &[LeaveZone<'_>],
    parameters: &Parameters,
    context: &JobContext<'_>,
) -> Result<(Vec<Pass>, Vec<f64>, Vec<f64>)> {
    {
        let mut pitch = parameters.number(names::STEPOVER)?;
        let height = parameters.number(parameter::SCALLOP)?;
        // A tool with no closed form for a cusp is warned about
        // ([`Strategy::parameter_warnings`]) and the stepover alone is used.
        if height > 0.0
            && let Some(rho) = scallop::cusp_radius(input.tool)
        {
            pitch = pitch.min(scallop::straight_step(rho, height));
        }
        let tolerance = parameters.number(names::TOLERANCE)?;
        let leave = parameters.number(names::STOCK_TO_LEAVE)?;
        let sign = if parameters.text(parameter::ROTATION)? == "negative" {
            -1.0
        } else {
            1.0
        };
        let start = parameters.number(parameter::START_ANGLE)?;
        let mut step = parameters.number(parameter::MAX_ANGLE_STEP)?;
        let extension = Extension::read(input, parameters, "helical finishing")?;

        let bounds = input.surface.bounds();
        let (x0, x1) = (bounds.min.x, bounds.max.x);
        let length = x1 - x0;
        if length.is_nan() || length <= 0.0 {
            return Err(PathError::Strategy(
                "the part has no length along the rotary axis to finish".to_owned(),
            ));
        }
        // A region constraint can only be seen where the tool is dropped, so
        // its points are never further apart than `CONSTRAINED_SPACING` of
        // path: a feature of the selection narrower than that may be missed.
        let radius = [bounds.min.y, bounds.max.y, bounds.min.z, bounds.max.z]
            .into_iter()
            .fold(0.0_f64, |r, v| r.max(v.abs()));
        // With a selected region the pass is only fitted where it can reach
        // the region (decision record 0034 item 5); without one, or when the
        // region cannot be developed, points are never further apart than
        // `CONSTRAINED_SPACING` of path.
        let most_leave = zones.iter().fold(leave, |m, z| m.max(z.leave));
        let candidates = region_index(input, region, radius, most_leave, tolerance);
        if region.is_some_and(|r| !r.is_empty()) && candidates.is_none() && radius > 0.0 {
            step = step.min((CONSTRAINED_SPACING / radius).to_degrees());
        }
        let turns = length / pitch;
        let total_angle = 360.0 * turns;
        let seeds = (total_angle / step).ceil().max(1.0);
        if seeds > MAX_SEEDS {
            return Err(PathError::Parameter {
                name: names::STEPOVER.to_owned(),
                reason: format!(
                    "{pitch} mm over {length:.1} mm of part needs {seeds:.0} points at {step} \
                     degrees; increase the stepover or the largest angle step"
                ),
            });
        }
        let seeds = seeds as usize;
        let grown = cutter::grow_profile(input.tool, leave)?;
        let zone_fits = zone_fits(zones, input.tool)?;
        let fit = Fit {
            input,
            drop_profile: &grown,
            leave,
            zones: &zone_fits,
            x0,
            length,
            start,
            sign,
            total_angle,
            chord: CHORD_FACTOR * tolerance,
            region,
            constrained: region.is_some_and(|r| !r.is_empty()),
            candidates: candidates.as_ref(),
            spacing: region_spacing(tolerance, CONSTRAINED_SPACING),
            thin: false,
            max_dt: 1.0 / seeds as f64,
            wanted: None,
        };

        let passes = fit_region(&fit, seeds, "helical finishing", context)?;
        let mut passes = match extension {
            Some(e) => e.run_out(passes, input, &grown, leave, (x0, x1)),
            None => passes,
        };
        // One whole turn at each face across the axis, where the tool's side
        // touches it, cut after the helix as its own pass, so the helix is
        // as before (decision record 0297).
        let found = faces::turns_to_the_ends(input, &grown, (x0, x1), tolerance, context)?;
        let turns: Vec<f64> = found.turns.iter().map(|t| t.x).collect();
        for &x in &turns {
            context.check_cancelled()?;
            let at = start + sign * 360.0 * (x - x0) / pitch;
            passes.extend(turn_passes(
                input,
                region,
                (&grown, leave, &zone_fits),
                tolerance,
                (x, at, sign),
                step,
                None,
                context,
            )?);
        }
        Ok((passes, turns, found.held))
    }
}
