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
use std::collections::BTreeMap;

use openchuck_types::job::JobContext;
use openchuck_types::tool::profile::RevolvedProfile;
use openchuck_types::units::Degrees;

use crate::adaptive::{self, Counting, Family, Interval, Pose, Track};
use crate::assemble::PathWarning;
use crate::cutter;
use crate::error::{PathError, Result};
use crate::extend::Extension;
use crate::faces;
use crate::params::{ParameterKind, ParameterSchema, ParameterSpec, Parameters, Unit, names};
use crate::region::RegionConstraint;
use crate::rotary::RotaryMapping;
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
            .chain(crate::finishing::specs(true, true))
            .chain(adaptive::specs())
            .collect(),
        )
    }

    fn schema_for(
        &self,
        values: &BTreeMap<String, crate::params::ParameterValue>,
    ) -> ParameterSchema {
        adaptive::schema_for(
            crate::finishing::schema_for(self.schema(), values),
            values,
            parameter::SCALLOP,
        )
    }

    fn honours_region(&self) -> bool {
        true
    }

    fn parameter_warnings(&self, tool: &RevolvedProfile, parameters: &Parameters) -> Vec<String> {
        // With adaptive spacing on the height is measured with the tool
        // itself, so no closed form is needed (decision record 0239).
        if parameters
            .flag(adaptive::parameter::ADAPTIVE_SPACING)
            .unwrap_or(false)
        {
            return Vec::new();
        }
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
    ) -> Result<StrategyOutput> {
        let (passes, notes, warnings) = passes(input, region, zones, parameters, context)?;
        Ok(StrategyOutput {
            passes,
            notes,
            warnings,
            unreached: Vec::new(),
            shank_breaks: Vec::new(),
        })
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
        let (passes, notes, warnings) = passes(input, region, &[], parameters, context)?;
        Ok(StrategyOutput {
            passes,
            notes,
            warnings,
            unreached: Vec::new(),
            shank_breaks: Vec::new(),
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
        across: 0.0,
    };
    fit_region(&fit, n, "a turn at a face", context)
}

/// The passes of helical finishing, limited to `region` when given, with
/// its notes and warnings: the faces across the axis it turned at, those a
/// held region kept it from, and its free ends.
fn passes(
    input: &StrategyInput<'_>,
    region: Option<&RegionConstraint<'_>>,
    zones: &[LeaveZone<'_>],
    parameters: &Parameters,
    context: &JobContext<'_>,
) -> Result<(Vec<Pass>, Vec<String>, Vec<PathWarning>)> {
    {
        let mut pitch = parameters.number(names::STEPOVER)?;
        let height = parameters.number(parameter::SCALLOP)?;
        let adaptive = adaptive::Settings::read(parameters, parameter::SCALLOP)?;
        // A tool with no closed form for a cusp is warned about
        // ([`Strategy::parameter_warnings`]) and the stepover alone is used.
        // With adaptive spacing on, the height is the Allowed excess and the
        // stepover is the widest pitch (decision record 0239).
        if height > 0.0
            && adaptive.is_none()
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
            across: 0.0,
        };

        let passes = fit_region(&fit, seeds, "helical finishing", context)?;
        // Passes added between the wraps where more than the Allowed excess
        // is left (decision record 0239), cut after the helix.
        let spaced = match adaptive {
            Some(settings) => Some(adaptive_passes(
                input,
                (region, zones, candidates.as_ref()),
                &passes,
                Helix {
                    x0,
                    pitch,
                    total: total_angle,
                    start,
                    sign,
                    step,
                    tolerance,
                    leave,
                },
                settings,
                context,
            )?),
            None => None,
        };
        let mut passes = match extension {
            Some(e) => e.run_out(passes, input, &grown, leave, (x0, x1)),
            None => passes,
        };
        let mut spaced_note = None;
        let mut spaced_warning = None;
        if let Some((added, outcome, settings)) = spaced {
            passes.extend(added);
            spaced_note = Some(outcome.note(&settings));
            spaced_warning =
                outcome.warning(&settings, &adaptive::Limits::OWNER, parameter::SCALLOP);
        }
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
        // Finishing layers above the passes made so far, when on (decision
        // record 0345).
        let (mut passes, layered) = crate::finishing::layers(input, passes, parameters)?;
        // The free ends, in layers down beside them (decision record 0344).
        let free = crate::finishing::free_ends(
            input,
            &grown,
            &found.free,
            parameters,
            (start, sign, step),
        )?;
        passes.extend(free.passes);
        let notes = faces::note(&turns)
            .into_iter()
            .chain(faces::held_note(&found.held))
            .chain(spaced_note)
            .chain(layered)
            .chain(free.notes)
            .collect();
        let mut warnings = free.warnings;
        warnings.extend(spaced_warning);
        Ok((passes, notes, warnings))
    }
}

/// The helix of helical finishing, as adaptive spacing needs it.
#[derive(Debug, Clone, Copy)]
struct Helix {
    /// X where the helix starts, mm.
    x0: f64,
    /// X per turn, mm.
    pitch: f64,
    /// Degrees turned over the whole helix.
    total: f64,
    /// A at the start, degrees.
    start: f64,
    /// Which way A turns as X increases.
    sign: f64,
    /// The largest A step between seeds, degrees.
    step: f64,
    tolerance: f64,
    leave: f64,
}

/// Helical finishing's passes as a family for adaptive spacing: across it,
/// the offset along X from the helix, mm (the next wrap is one pitch on);
/// along it, the degrees turned from the helix's start.
struct Wraps<'a> {
    fit: Fit<'a>,
    helix: Helix,
    counters: &'a [Counting<'a>],
    rotary: RotaryMapping,
    pose_step: f64,
    window: f64,
    station: f64,
    margin: f64,
}

impl Family for Wraps<'_> {
    fn pose(&self, c: f64, s: f64) -> (f64, Degrees) {
        let h = &self.helix;
        (
            h.x0 + c + h.pitch * s / 360.0,
            Degrees(h.start + h.sign * s),
        )
    }

    fn poses(&self, c: f64, pass: &Pass) -> Vec<Pose> {
        let h = self.helix;
        // The helix itself, seen one pitch on, is the next wrap.
        let shift = if c >= h.pitch - 1e-9 { 360.0 } else { 0.0 };
        adaptive::machine_poses(&self.rotary, pass, self.pose_step, |_, a| {
            h.sign * (a.0 - h.start) - shift
        })
    }

    fn probe(&self, x: f64, a: Degrees) -> Option<(Point3, f64)> {
        let fit = Fit {
            x0: x,
            length: 0.0,
            start: a.0,
            total_angle: 0.0,
            max_dt: 1.0,
            ..self.fit
        };
        fit.node(0.0).map(|n| (n.contact.contact, n.leave))
    }

    fn fit(&self, c: f64, (from, to): (f64, f64), context: &JobContext<'_>) -> Result<Vec<Pass>> {
        let h = &self.helix;
        let turned = (to - from).abs();
        let seeds = ((turned / h.step).ceil() as usize).max(1);
        let fit = Fit {
            x0: h.x0 + c + h.pitch * from / 360.0,
            length: h.pitch * (to - from) / 360.0,
            start: h.start + h.sign * from,
            total_angle: to - from,
            max_dt: 1.0 / seeds as f64,
            ..self.fit
        };
        fit_region(&fit, seeds, "helical finishing", context)
    }

    fn width(&self, dc: f64) -> f64 {
        dc.abs()
    }

    fn window(&self) -> f64 {
        self.window
    }

    fn station_step(&self) -> f64 {
        self.station
    }

    fn margin(&self) -> f64 {
        self.margin
    }

    fn queries(&self) -> u64 {
        self.counters.iter().map(Counting::count).sum()
    }

    fn probe_cost(&self) -> u64 {
        self.counters.len() as u64
    }
}

/// The passes adaptive spacing adds between the wraps of the helix `passes`
/// (decision record 0239), in order along X, with what it did. Below the
/// first wrap and above the last only one wrap passes, so nothing is
/// measured there; those stretches are as before.
fn adaptive_passes(
    input: &StrategyInput<'_>,
    (region, zones, candidates): (
        Option<&RegionConstraint<'_>>,
        &[LeaveZone<'_>],
        Option<&crate::candidates::DevIndex>,
    ),
    passes: &[Pass],
    helix: Helix,
    settings: adaptive::Settings,
    context: &JobContext<'_>,
) -> Result<(Vec<Pass>, adaptive::Outcome, adaptive::Settings)> {
    // The measure and the added passes drop through counters, so their
    // geometry queries are limited (decision record 0239 item 8).
    let counters: Vec<Counting<'_>> = std::iter::once(input.surface)
        .chain(zones.iter().map(|z| z.surface))
        .map(Counting::new)
        .collect();
    let Some((first, rest)) = counters.split_first() else {
        return Err(PathError::Strategy("no surface to finish".to_owned()));
    };
    let counted = StrategyInput {
        surface: first,
        ..*input
    };
    let counted_zones: Vec<LeaveZone<'_>> = zones
        .iter()
        .zip(rest)
        .map(|(z, c)| LeaveZone {
            surface: c,
            leave: z.leave,
        })
        .collect();
    let grown = cutter::grow_profile(input.tool, helix.leave)?;
    let zone_fits = zone_fits(&counted_zones, input.tool)?;
    let fit = Fit {
        input: &counted,
        drop_profile: &grown,
        leave: helix.leave,
        zones: &zone_fits,
        x0: helix.x0,
        length: 0.0,
        start: helix.start,
        sign: helix.sign,
        total_angle: 0.0,
        chord: CHORD_FACTOR * helix.tolerance,
        region,
        constrained: region.is_some_and(|r| !r.is_empty()),
        candidates,
        spacing: region_spacing(helix.tolerance, CONSTRAINED_SPACING),
        thin: false,
        max_dt: 1.0,
        wanted: None,
        across: 0.0,
    };
    let r = input.tool.cutting_radius();
    let reach = grown.cutting_radius();
    // The angles below are in degrees turned: a millimetre of arc is the
    // fewest degrees at the part's largest radius, and the most near the
    // axis, at the tool's tips' smallest radius.
    let radii = passes
        .iter()
        .flat_map(|p| &p.points)
        .map(|c| c.tip.y.hypot(c.tip.z));
    let (least, most) = radii.fold((f64::INFINITY, 0.0_f64), |(l, m), v| (l.min(v), m.max(v)));
    let near = (least - reach).max(reach).max(1e-3);
    let far = most.max(near);
    let station_mm = (0.5 * r).min(1.0);
    let family = Wraps {
        fit,
        helix,
        counters: &counters,
        rotary: input.rotary,
        pose_step: (0.8 * r * settings.height).sqrt().clamp(0.02, 0.5 * r),
        window: ((2.0 * reach + station_mm) / near).to_degrees().min(180.0),
        station: (station_mm / far).to_degrees().min(helix.step),
        margin: ((reach + station_mm) / near).to_degrees().min(180.0),
    };
    let tracks = vec![
        Track::new(&family, 0.0, passes.to_vec()),
        Track::new(&family, helix.pitch, passes.to_vec()),
    ];
    let points: u64 = passes
        .iter()
        .map(|p| p.points.len() as u64 + adaptive::LINK_POINTS)
        .sum();
    let intervals = if helix.total > 360.0 {
        vec![Interval {
            lo: 0,
            hi: 1,
            spans: vec![(0.0, helix.total - 360.0)],
            level: 0,
        }]
    } else {
        Vec::new()
    };
    let outcome = adaptive::refine(
        &family,
        tracks,
        intervals,
        input.tool,
        &settings,
        &adaptive::Limits::OWNER,
        points,
        context,
    )?;
    let mut added: Vec<Pass> = outcome
        .tracks
        .iter()
        .skip(outcome.own)
        .flat_map(|t| t.passes.iter().cloned())
        .collect();
    let first_x = |p: &Pass| p.points.first().map_or(f64::INFINITY, |c| c.tip.x);
    added.sort_by(|p, q| first_x(p).total_cmp(&first_x(q)));
    Ok((added, outcome, settings))
}
