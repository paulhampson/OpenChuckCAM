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

use std::sync::Mutex;
use std::time::Duration;

use std::collections::BTreeMap;

use openchuck_types::job::{JobContext, Progress, ProgressSink};
use openchuck_types::tool::profile::RevolvedProfile;
use openchuck_types::units::Degrees;

use crate::assemble::PathWarning;
use crate::candidates::DevIndex;
use crate::cutter;
use crate::error::{PathError, Result};
use crate::extend::Extension;
use crate::faces;
use crate::helical::CONSTRAINED_SPACING;
use crate::parallel;
use crate::params::{ParameterKind, ParameterSchema, ParameterSpec, Parameters, Unit, names};
use crate::region::RegionConstraint;
use crate::scallop;
use crate::strategy::{LeaveZone, Pass, Strategy, StrategyInput, StrategyOutput};
use crate::wrap_fit::{
    CHORD_FACTOR, Fit, ZoneFit, fit_region, region_index, region_spacing, zone_fits,
};

/// Axial finishing of the whole model, or of a region.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AxialFinishing;

/// Keep one outer progress count while forwarding the host's optional timings.
struct QuietPassProgress<'a>(&'a dyn ProgressSink);

impl ProgressSink for QuietPassProgress<'_> {
    fn report(&self, _: Progress) {}

    fn wants_timings(&self) -> bool {
        self.0.wants_timings()
    }

    fn timing(&self, stage: &str, elapsed: Duration) {
        self.0.timing(stage, elapsed);
    }
}

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
        "Follows the surface in passes along the length of the part, turning the part a small step between passes. Use it where marks running along the part suit the job better than a spiral."
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
            .chain(crate::finishing::specs(true, true))
            .collect(),
        )
    }

    fn schema_for(
        &self,
        values: &BTreeMap<String, crate::params::ParameterValue>,
    ) -> ParameterSchema {
        crate::finishing::schema_for(self.schema(), values)
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
    /// How far, mm, each pass runs on past the ends of the target, so the
    /// tool can go down beside an end face (indexed finishing; decision
    /// record 0343). Zero for axial finishing, which turns at the end faces
    /// instead.
    pub(crate) beyond: f64,
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
            beyond: 0.0,
        })
    }

    /// The target's extent along X, mm.
    pub(crate) fn range(&self) -> (f64, f64) {
        (self.x0, self.x1)
    }

    /// The cutting radius of the tool grown by the stock to leave, mm.
    pub(crate) fn grown_radius(&self) -> f64 {
        self.grown.cutting_radius()
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
        let (x0, x1) = (self.x0, self.x1);
        let passes = if self.beyond > 0.0 {
            // The pass over the target is fitted as before, so its points do
            // not move; the stretches past each end are fitted on their own
            // and joined to it where they meet (decision record 0343).
            let h = (x1 - x0) / self.seeds as f64;
            let seeds = ((self.beyond / h).ceil() as usize).max(1);
            let (lo, hi) = (x0 - self.beyond, x1 + self.beyond);
            let pieces = if backwards {
                [(hi, x1, seeds), (x1, x0, self.seeds), (x0, lo, seeds)]
            } else {
                [(lo, x0, seeds), (x0, x1, self.seeds), (x1, hi, seeds)]
            };
            let mut out: Vec<Pass> = Vec::new();
            for (from, to, n) in pieces {
                for pass in self.fit_span(a, (from, to), n, context)? {
                    join(&mut out, pass);
                }
            }
            out
        } else if backwards {
            self.fit_span(a, (x1, x0), self.seeds, context)?
        } else {
            self.fit_span(a, (x0, x1), self.seeds, context)?
        };
        let (lo, hi) = (x0 - self.beyond, x1 + self.beyond);
        Ok(match self.extension {
            Some(e) => e.run_out(passes, self.input, &self.grown, self.leave, (lo, hi)),
            None => passes,
        })
    }

    /// The fit of one stretch along X at `a`, from `span.0` to `span.1`,
    /// with `seeds` seeds.
    fn fit_span(
        &self,
        a: Degrees,
        (from, to): (f64, f64),
        seeds: usize,
        context: &JobContext<'_>,
    ) -> Result<Vec<Pass>> {
        let fit = Fit {
            input: self.input,
            drop_profile: &self.grown,
            leave: self.leave,
            zones: &self.zones,
            x0: from,
            length: to - from,
            start: a.0,
            sign: 1.0,
            total_angle: 0.0,
            chord: self.chord,
            region: self.region,
            constrained: self.constrained,
            candidates: self.candidates.as_ref(),
            spacing: self.spacing,
            thin: false,
            max_dt: 1.0 / seeds as f64,
            wanted: None,
        };
        fit_region(&fit, seeds, "axial finishing", context)
    }

    /// The passes at each of `angles` in order, every other one backwards
    /// when zig-zagging. The passes are fitted in parallel, one to a thread
    /// (issue #252): a pass has too few points to spread over the threads by
    /// itself. Each pass is fitted alone and they are joined in order, so the
    /// path never depends on the thread count (NFR-06). `label` names the
    /// strategy in the one progress count it reports, of passes.
    pub(crate) fn passes_at(
        &self,
        angles: &[f64],
        label: &str,
        context: &JobContext<'_>,
    ) -> Result<Vec<Pass>> {
        let stage = format!("{label}: fitting the passes");
        let total = angles.len() as u64;
        context.report(&stage, 0, Some(total));
        // The passes report nothing of their own: their counts, from many
        // threads at once, would replace each other in the progress shown.
        let diagnostics = QuietPassProgress(context.progress);
        let quiet = JobContext::new(context.cancel, &diagnostics);
        let done = Mutex::new(0_u64);
        let indexed: Vec<(usize, f64)> = angles.iter().copied().enumerate().collect();
        let fitted = parallel::map_each_in_order(&indexed, |&(i, a)| {
            quiet.check_cancelled()?;
            let pass = self.pass(Degrees(a), self.zigzag && i % 2 == 1, &quiet);
            // Counted under the lock, so the count shown only goes up; a
            // pass that fails is not counted as done (OI-843).
            if pass.is_ok()
                && let Ok(mut done) = done.lock()
            {
                *done += 1;
                context.report(&stage, *done, Some(total));
            }
            pass
        });
        let mut out = Vec::new();
        // The first error in pass order, so a failure is the same at any
        // thread count.
        for pass in fitted {
            out.extend(pass?);
        }
        Ok(out)
    }
}

/// Adds `pass` to `out`, joined to the last pass where it starts at the
/// point that pass ends at.
fn join(out: &mut Vec<Pass>, pass: Pass) {
    if let Some(last) = out.last_mut()
        && let (Some(end), Some(start)) = (last.points.last(), pass.points.first())
        && end.tip.distance(start.tip) <= 1e-9
        && last.rapids.is_empty()
        && pass.rapids.is_empty()
    {
        last.points.extend(pass.points.into_iter().skip(1));
        return;
    }
    out.push(pass);
}

/// The largest A step, degrees, between the seeds of a turn at an end face.
const TURN_STEP: f64 = 5.0;

/// The passes of axial finishing, with its notes and warnings: the end
/// faces past the ends of the target it turned at, those a held region kept
/// it from, and its free ends.
fn passes(
    input: &StrategyInput<'_>,
    region: Option<&RegionConstraint<'_>>,
    zones: &[LeaveZone<'_>],
    parameters: &Parameters,
    context: &JobContext<'_>,
) -> Result<(Vec<Pass>, Vec<String>, Vec<PathWarning>)> {
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
    let angles: Vec<f64> = (0..count).map(|i| start + step * i as f64).collect();
    let mut passes = along.passes_at(&angles, "axial finishing", context)?;
    // A pass along X reaches over the target's own length only, so an end
    // face whose turn lies past the target's end (beside a tab shorter than
    // the tool's radius) is never met by the tool's side: one whole turn of
    // A at each such face, as the helix strategies make (decision records
    // 0297 and 0343). The faces inside the target's length are left to the
    // passes, as before.
    let tolerance = parameters.number(names::TOLERANCE)?;
    let found = faces::turns_to_the_ends(
        input,
        &along.grown,
        (along.x0, along.x1),
        tolerance,
        context,
    )?;
    let beyond = |x: f64| x < along.x0 || x > along.x1;
    let turns: Vec<f64> = found
        .turns
        .iter()
        .map(|t| t.x)
        .filter(|&x| beyond(x))
        .collect();
    let held: Vec<f64> = found.held.iter().copied().filter(|&x| beyond(x)).collect();
    for &x in &turns {
        context.check_cancelled()?;
        passes.extend(crate::helical::turn_passes(
            input,
            region,
            (&along.grown, along.leave, &along.zones),
            tolerance,
            (x, start, 1.0),
            TURN_STEP,
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
        &along.grown,
        &found.free,
        parameters,
        (start, 1.0, TURN_STEP),
    )?;
    passes.extend(free.passes);
    let notes = faces::note(&turns)
        .into_iter()
        .chain(faces::held_note(&held))
        .chain(layered)
        .chain(free.notes)
        .collect();
    Ok((passes, notes, free.warnings))
}
