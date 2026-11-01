// SPDX-License-Identifier: AGPL-3.0-or-later
//! True simultaneous 4-axis finishing: A varies continuously with X, Y and Z
//! to keep the tool near the surface normal (OP-05, OP-06; work breakdown
//! slice 13; decision record 0036).
//!
//! **Tool-axis policy.** The rotary axis is the model X axis, and a vertical
//! tool can only lie in the plane perpendicular to it at any A (requirements,
//! "Operations"). At a surface point with outward normal `n` the best axis
//! is `n`'s part in that plane, at the polar angle `phi_n` (decision record
//! 0015 item 3). The tool axis is given the polar angle
//! `phi = phi_n + lead * travel`, where `travel` is +1 or -1 by the way the
//! pass moves round the axis, and is then allowed to leave that target by at
//! most `max_tilt` so that A can be smoothed. The part of `n` along X (the
//! X-slope angle) cannot be followed by any A; it is reported in a note, not
//! hidden.
//!
//! **Drive curves.** A pattern gives tracks on the surface in (x, polar
//! angle) coordinates: one helix for `spiral` (the axial advance per turn is
//! the stepover, as for helical finishing), or one line along X per angular
//! station for `axial` (stepover as arc length at the part's largest radius).
//! A drive point is where a ray from outside the part towards the axis first
//! meets the surface, so the same code serves meshes and exact faces through
//! [`SurfaceSource`]. On a mesh the normal is the smoothed normal of
//! [`NORMAL_SPAN`] (OI-67); exact faces give their own.
//!
//! **Smooth A (OP-15).** Along each track the target angles are sampled
//! densely and replaced by the *taut string* through the band
//! `target +/- max_tilt`: the shortest path inside the band, which has the
//! fewest reversals any path in the band can have and is straight wherever
//! the band allows. With `max_tilt` 0 the tool follows the normal exactly
//! (the unconstrained version).
//!
//! **Tool-centre solving.** For a convex profile the tool is placed so that
//! its point with the surface's normal touches the drive point (the
//! normal-offset fast path of decision record 0012 item 4), then dropped
//! along its axis with the generic drop-cutter, which raises it clear of
//! anything else it would cut. A non-convex profile is dropped along the line
//! through the drive point. Either way every point is the drop's answer, so
//! no point gouges the surface.
//!
//! **Gouge protection between points (OP-12).** The machine moves in a
//! straight line in X, Y, Z and A between points. A move is halved until the
//! tool is within [`CHORD_FACTOR`] times the tolerance of the surface at the
//! middle of the move and at the middles of its two halves (the quarter
//! points; decision record 0133, as decision 0115 did for the shared fitter),
//! each measured by dropping the tool along its axis at that machine pose
//! (positive: the move would cut in; negative: it would leave material) and
//! taken along the surface normal. The quarter points of a move are the
//! middles of its halves, so their drops are handed down when the move is
//! split. A move that still cuts in by more than the tolerance at its middle
//! after [`MAX_DEPTH`] halvings is refused with a message naming it.

use openchuck_types::geometry::{Point3, UnitVec3, Vec3};
use openchuck_types::job::JobContext;
use openchuck_types::tool::profile::RevolvedProfile;
use openchuck_types::units::Degrees;

use crate::adaptive::{self, Counting, Family, Interval, Pose};
use crate::cutter::{self, Contact, ToolLine};
use crate::error::{PathError, Result};
use crate::faces;
use crate::parallel;
use crate::params::{ParameterKind, ParameterSchema, ParameterSpec, Parameters, Unit, names};
use crate::region::RegionConstraint;
use crate::rotary::{AxisPosition, TURN};
use crate::section;
use crate::strategy::{CutPoint, Pass, Strategy, StrategyInput, StrategyOutput};

pub use crate::helical::{CHORD_FACTOR, CONSTRAINED_SPACING};

mod hidden;

/// The registry name of [`SimultaneousFinishing`].
pub const NAME: &str = "simultaneous_finishing";

/// Parameter names specific to simultaneous finishing.
pub mod parameter {
    /// The drive pattern: `spiral` or `axial`.
    pub const PATTERN: &str = "pattern";
    /// Which way A turns as X increases along a spiral.
    pub const ROTATION: &str = "rotation";
    /// A at the start, as for helical finishing.
    pub const START_ANGLE: &str = "start_angle";
    /// The largest step round the axis between seeded points, degrees.
    pub const MAX_ANGLE_STEP: &str = "max_angle_step";
    /// Tilt of the tool axis from the normal towards the direction of
    /// travel round the axis, degrees.
    pub const LEAD_ANGLE: &str = "lead_angle";
    /// The most the tool axis may leave its target to smooth A, degrees.
    pub const MAX_TILT: &str = "max_tilt";
    /// The most the tool axis may lean from the surface normal on surfaces
    /// hidden from the drive (OP-20; decision record 0368), degrees.
    pub const MAX_HIDDEN_LEAN: &str = "max_hidden_lean";
    /// The Allowed excess with adaptive spacing on, mm (decision record
    /// 0239); the axial pattern only.
    pub const SCALLOP: &str = "scallop";
}

/// How far apart the drive samples are round the axis, in degrees, where
/// the A smoothing reads the normal.
pub const SAMPLE_ANGLE: f64 = 0.5;

/// How far apart the drive samples are along X, in millimetres.
pub const SAMPLE_LENGTH: f64 = 0.1;

/// How far apart the seeded tool positions of an axial pass are, in
/// millimetres along X, before path fitting adds points.
pub const AXIAL_SEED: f64 = 1.0;

/// The half-width, in millimetres on the surface, of the difference stencil
/// that gives the smoothed normal of a mesh (OI-67).
pub const NORMAL_SPAN: f64 = 0.5;

/// The lean of a flat-bottomed tool's axis from the normal, as the sine of
/// the angle (about 5.7 degrees), at and beyond which the tool is seeded
/// with its corner on the drive point; nearer the normal the seed moves
/// towards the tool's centre in step with the lean (OI-337).
const FLAT_BLEND: f64 = 0.1;

/// How many times a move may be halved.
pub const MAX_DEPTH: u32 = 12;

/// Most poses a long move is tested at (decision record 0363 item 2).
const MAX_TEST_POSES: usize = 256;

/// The least share of a cut in along the tool axis that the move test reads
/// as depth (decision record 0363 item 2).
const MIN_ALONG: f64 = 0.5;

/// How many times a move that still cuts in after [`MAX_DEPTH`] halvings
/// may be kept as two moves through the raised tool at its middle.
const RAISES: u32 = 12;

/// How many times the limit of an allowed region is bisected.
const EDGE_STEPS: u32 = 12;

/// Most drive samples one run may make (NFR-07).
const MAX_SAMPLES: f64 = 5.0e6;

/// Largest lead and largest tilt, degrees: their sum stays well short of a
/// right angle, so the tool never faces away from the surface.
const MAX_LEAD: f64 = 30.0;
const MAX_TILT: f64 = 30.0;

/// The largest lean from the normal on hidden surfaces, degrees: the tool's
/// side (OP-20; decision record 0368).
const MAX_HIDDEN_LEAN: f64 = 90.0;

/// Simultaneous 4-axis finishing (OP-05, OP-06).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SimultaneousFinishing;

fn degrees_spec(
    name: &str,
    label: &str,
    description: &str,
    min: Option<f64>,
    max: Option<f64>,
    default: f64,
) -> ParameterSpec {
    ParameterSpec {
        name: name.to_owned(),
        label: label.to_owned(),
        description: description.to_owned(),
        kind: ParameterKind::Number {
            unit: Unit::Degrees,
            min,
            min_exclusive: false,
            max,
            default: Some(default),
        },
    }
}

fn choice(name: &str, label: &str, description: &str, options: &[&str]) -> ParameterSpec {
    ParameterSpec {
        name: name.to_owned(),
        label: label.to_owned(),
        description: description.to_owned(),
        kind: ParameterKind::Choice {
            options: options.iter().map(|o| (*o).to_owned()).collect(),
            default: options.first().map(|o| (*o).to_owned()).unwrap_or_default(),
        },
    }
}

impl Strategy for SimultaneousFinishing {
    /// Exact contacts on a STEP part: measured within the limits of OI-835
    /// (decision 0277).
    fn exact_contact(&self) -> bool {
        true
    }

    fn name(&self) -> &str {
        NAME
    }

    fn label(&self) -> &str {
        "Simultaneous 4-axis finishing"
    }

    fn description(&self) -> &str {
        "Moves the tool while turning the part to approach the surface from different directions. Use it for curved shapes that need the cutting angle to change along a pass. Four-axis motion cannot follow every surface direction; check the resulting path and simulation."
    }

    fn schema(&self) -> ParameterSchema {
        let mut step = degrees_spec(
            parameter::MAX_ANGLE_STEP,
            "Largest angle step",
            "The most the path turns round the axis between two seeded points; points are \
             added where the surface needs them.",
            Some(0.0),
            Some(90.0),
            5.0,
        );
        if let ParameterKind::Number { min_exclusive, .. } = &mut step.kind {
            *min_exclusive = true;
        }
        ParameterSchema::with_common(
            vec![
            choice(
                parameter::PATTERN,
                "Pattern",
                "Spiral: one helix round the axis, the stepover apart per turn. Axial: passes \
                 along the axis, the stepover apart at the part's largest radius.",
                &["spiral", "axial"],
            ),
            choice(
                parameter::ROTATION,
                "Rotation",
                "Which way A turns as X increases along a spiral.",
                &["positive", "negative"],
            ),
            degrees_spec(
                parameter::START_ANGLE,
                "Start angle",
                "A at the start of the path, where the tool would face a round part.",
                None,
                None,
                0.0,
            ),
            step,
            degrees_spec(
                parameter::LEAD_ANGLE,
                "Lead angle",
                "Leans the tool from the surface normal towards the way the path travels round \
                 the axis, for example to keep a ball's centre, where it barely cuts, off the \
                 surface. A can only lean the tool round the axis, so this applies to the spiral \
                 pattern; axial passes travel along the axis and need 0. Gouging is still \
                 checked at any lean.",
                Some(-MAX_LEAD),
                Some(MAX_LEAD),
                0.0,
            ),
            degrees_spec(
                parameter::MAX_TILT,
                "Largest tilt",
                "How far the tool may lean away from the normal (plus the lead) so that A turns \
                 smoothly with fewer reversals. 0 follows the normal exactly; a larger value \
                 gives smoother A but cuts with more of the tool's side, which changes the finish \
                 for flat, bull-nose and V tools. The slope along the axis cannot be followed by \
                 any A and is reported separately.",
                Some(0.0),
                Some(MAX_TILT),
                10.0,
            ),
            degrees_spec(
                parameter::MAX_HIDDEN_LEAN,
                "Largest lean on hidden surfaces",
                "Surfaces a straight approach from outside cannot see, such as the walls of a \
                 slot or the side of a part facing another, are finished with their own passes \
                 after the main one. There the tool leans from the surface normal as little as \
                 the part lets it reach, up to this angle: 90 lets it cut a wall with its side, \
                 0 keeps it on the normal. Anything needing more is left uncut and listed in a \
                 warning.",
                Some(0.0),
                Some(MAX_HIDDEN_LEAN),
                90.0,
            ),
            ParameterSpec {
                name: parameter::SCALLOP.to_owned(),
                label: "Allowed excess".to_owned(),
                description: String::new(),
                kind: ParameterKind::Number {
                    unit: Unit::Millimetres,
                    min: Some(0.0),
                    min_exclusive: false,
                    max: None,
                    default: Some(adaptive::DEFAULT_ALLOWED_EXCESS),
                },
            },
        ]
        .into_iter()
        .chain(adaptive::specs())
        .collect())
    }

    fn schema_for(
        &self,
        values: &std::collections::BTreeMap<String, crate::params::ParameterValue>,
    ) -> ParameterSchema {
        // Adaptive spacing applies to the axial pattern only, and the
        // height is used only with it on (decision record 0239 item 2).
        let axial = matches!(
            values.get(parameter::PATTERN),
            Some(crate::params::ParameterValue::Text(p)) if p == "axial"
        );
        let mut schema = adaptive::schema_for(self.schema(), values, parameter::SCALLOP);
        if !axial || !adaptive::is_on(values) {
            schema.parameters.retain(|p| p.name != parameter::SCALLOP);
        }
        if !axial {
            schema.parameters.retain(|p| {
                p.name != adaptive::parameter::ADAPTIVE_SPACING
                    && p.name != adaptive::parameter::MIN_SPACING
            });
        }
        schema
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
        run(input, None, parameters, context).map(|o| o.passes)
    }

    fn passes_in_region(
        &self,
        input: &StrategyInput<'_>,
        region: &RegionConstraint<'_>,
        parameters: &Parameters,
        context: &JobContext<'_>,
    ) -> Result<Vec<Pass>> {
        run(input, Some(region), parameters, context).map(|o| o.passes)
    }

    fn passes_with_notes(
        &self,
        input: &StrategyInput<'_>,
        region: Option<&RegionConstraint<'_>>,
        parameters: &Parameters,
        context: &JobContext<'_>,
    ) -> Result<StrategyOutput> {
        run(input, region, parameters, context)
    }
}

/// A straight track on the surface in (x, polar angle) coordinates, run
/// from `t = 0` to `t = 1`.
#[derive(Debug, Clone, Copy)]
struct Track {
    x0: f64,
    x1: f64,
    /// Polar angles of the drive point, degrees, from +Y towards +Z.
    th0: f64,
    th1: f64,
    /// Seeded tool positions along the track.
    seeds: usize,
    /// Drive samples for the A smoothing.
    samples: usize,
    /// The zone of [`Plan::zones`] the track lies in, whose centre its drive
    /// rays aim at (OP-26).
    zone: usize,
    /// The contour of [`Plan::contours`] the track follows instead, on a
    /// surface hidden from the drive's rays (OP-20).
    contour: Option<usize>,
}

impl Track {
    fn at(&self, t: f64) -> (f64, f64) {
        (
            self.x0 + (self.x1 - self.x0) * t,
            self.th0 + (self.th1 - self.th0) * t,
        )
    }

    /// +1 or -1 as the track turns round the axis, 0 when it does not.
    fn travel(&self) -> f64 {
        let d = self.th1 - self.th0;
        if d.abs() < 1e-12 { 0.0 } else { d.signum() }
    }
}

/// A drive point: where the tool should touch, and the normal there.
#[derive(Debug, Clone, Copy)]
struct Drive {
    point: Point3,
    normal: UnitVec3,
    /// Polar angle of the normal's part perpendicular to X, degrees.
    phi: f64,
    /// Whether the answer came from an exact face.
    exact: bool,
}

/// The tool axis angle along a track: the taut string through the band, as
/// a piecewise linear function of `t` on each run of samples.
#[derive(Debug, Clone, Default)]
struct AxisPlan {
    /// Runs of `(t, phi)` vertices, in `t` order.
    runs: Vec<Vec<(f64, f64)>>,
}

impl AxisPlan {
    fn phi_at(&self, t: f64) -> Option<f64> {
        for run in &self.runs {
            let (Some(first), Some(last)) = (run.first(), run.last()) else {
                continue;
            };
            if t < first.0 - 1e-12 || t > last.0 + 1e-12 {
                continue;
            }
            let i = run.partition_point(|v| v.0 < t);
            let hi = run.get(i).copied().unwrap_or(*last);
            let lo = i
                .checked_sub(1)
                .and_then(|k| run.get(k))
                .copied()
                .unwrap_or(hi);
            if hi.0 - lo.0 <= f64::MIN_POSITIVE {
                return Some(hi.1);
            }
            let s = (t - lo.0) / (hi.0 - lo.0);
            return Some(lo.1 + s * (hi.1 - lo.1));
        }
        None
    }
}

/// A seeded or added tool position.
#[derive(Debug, Clone, Copy)]
struct Node {
    t: f64,
    a: Degrees,
    /// The drop with the (grown) tool.
    contact: Contact,
    /// The surface normal at the drive point.
    normal: UnitVec3,
    /// The normal's in-plane angle and the tool axis angle, degrees.
    phi_n: f64,
    phi: f64,
    /// A tool raised at the middle of a straight move (decision record 0363
    /// item 2), whose axis is the move's, not one chosen for a normal.
    raised: bool,
}

struct Plan<'a> {
    input: &'a StrategyInput<'a>,
    region: Option<&'a RegionConstraint<'a>>,
    constrained: bool,
    drop_profile: &'a RevolvedProfile,
    convex: bool,
    /// The tool's cutting radius, mm. A move whose tip travels further over
    /// the part is tested at a pose every quarter of it, so that a thin wall
    /// between the middle and quarter points still meets the tool at one of
    /// them; the first move that jumps further after the last halving goes
    /// through the raised tool (decision record 0363 item 2).
    reach: f64,
    /// Whether the tool has a flat bottom, whose normal is the axis over a
    /// width (a flat end or bull-nose), seeded by [`FLAT_BLEND`].
    flat_bottom: bool,
    leave: f64,
    tolerance: f64,
    chord: f64,
    /// Radius from which drive rays start.
    outside: f64,
    /// The X zones and the centres their drive rays aim at (OP-26).
    zones: &'a [section::Zone],
    sense: f64,
    /// The tool with its shank where wider than the cutting part, which the
    /// passes on hidden surfaces keep clear of the part (OP-20).
    body: Option<&'a RevolvedProfile>,
    /// The passes along surfaces hidden from the drive (OP-20).
    contours: Vec<hidden::Contour>,
}

/// What a run found worth telling the user.
#[derive(Debug, Clone, Default)]
struct Findings {
    slope: f64,
    slope_at: Option<Point3>,
    tilt: f64,
    /// The most material any kept move leaves beyond the tolerance, mm.
    under: f64,
    mesh: bool,
    /// The largest lean from the normal on a hidden surface, degrees, and
    /// where (OP-20).
    lean: f64,
    lean_at: Option<(f64, f64)>,
    /// Where a pass broke because the shank would meet the part.
    shank_breaks: Vec<crate::assemble::ShankBreak>,
}

/// The points a move is refined into, and the most material it leaves.
#[derive(Default)]
struct Refined {
    points: Vec<Option<Node>>,
    under: f64,
    /// Where the pass broke because the shank would meet the part.
    breaks: Vec<crate::assemble::ShankBreak>,
}

fn run(
    input: &StrategyInput<'_>,
    region: Option<&RegionConstraint<'_>>,
    parameters: &Parameters,
    context: &JobContext<'_>,
) -> Result<StrategyOutput> {
    let stepover = parameters.number(names::STEPOVER)?;
    let tolerance = parameters.number(names::TOLERANCE)?;
    let leave = parameters.number(names::STOCK_TO_LEAVE)?;
    let pattern = parameters.text(parameter::PATTERN)?;
    let rotation = if parameters.text(parameter::ROTATION)? == "negative" {
        -1.0
    } else {
        1.0
    };
    let start = parameters.number(parameter::START_ANGLE)?;
    let mut step = parameters.number(parameter::MAX_ANGLE_STEP)?;
    let lead = parameters.number(parameter::LEAD_ANGLE)?;
    let max_tilt = parameters.number(parameter::MAX_TILT)?;
    let max_lean = parameters.number(parameter::MAX_HIDDEN_LEAN)?;
    // Adaptive spacing of the axial pattern (decision record 0239).
    let adaptive = if pattern == "axial" {
        adaptive::Settings::read(parameters, parameter::SCALLOP)?
    } else {
        None
    };

    let bounds = input.surface.bounds();
    let (x0, x1) = outside_held(bounds.min.x, bounds.max.x, input.keep_out);
    let length = x1 - x0;
    if length.is_nan() || length <= 0.0 {
        return Err(PathError::Strategy(
            "the part has no length along the rotary axis to finish".to_owned(),
        ));
    }
    let radius = [bounds.min.y, bounds.max.y, bounds.min.z, bounds.max.z]
        .into_iter()
        .fold(0.0_f64, |r, v| r.max(v.abs()));
    if !(radius.is_finite() && radius > 0.0) {
        return Err(PathError::Strategy(
            "the part has no extent round the rotary axis to finish".to_owned(),
        ));
    }
    let sense = input.rotary.sense.sign();
    // The polar angle a round part faces at A = start (A = sense (90 - phi)).
    let th_start = 90.0 - sense * start;
    // A region constraint is seen only where the tool is dropped, so with
    // one the seeded points are no further apart than helical finishing's
    // `CONSTRAINED_SPACING` of path (OI-138).
    let constrained = region.is_some_and(|r| !r.is_empty());
    let mut axial_seed = AXIAL_SEED;
    if constrained {
        step = step.min((CONSTRAINED_SPACING / radius).to_degrees());
        axial_seed = axial_seed.min(CONSTRAINED_SPACING);
    }

    // Each face's turn: X, A at its start and the way A turns.
    let mut face_turns: Vec<(f64, f64, f64)> = Vec::new();
    let mut held: Vec<f64> = Vec::new();
    let tracks: Vec<Track> = match pattern {
        "axial" => {
            if lead != 0.0 {
                return Err(PathError::parameter(
                    parameter::LEAD_ANGLE,
                    "axial passes travel along the rotary axis, where A cannot lean the tool; \
                     set the lead angle to 0 or use the spiral pattern",
                ));
            }
            let stations = (TURN * radius.to_radians() / stepover).ceil().max(1.0);
            if stations * (length / SAMPLE_LENGTH) > MAX_SAMPLES {
                return Err(too_many(
                    stepover,
                    length,
                    stations * length / SAMPLE_LENGTH,
                ));
            }
            let samples = (length / SAMPLE_LENGTH).ceil().max(1.0) as usize;
            let seeds = (length / axial_seed).ceil().max(1.0) as usize;
            (0..stations as usize)
                .map(|k| {
                    let th = th_start - TURN * k as f64 / stations;
                    // With adaptive spacing every pass is fitted forwards
                    // and the order is set once the passes are added.
                    let (a, b) = if k % 2 == 0 || adaptive.is_some() {
                        (x0, x1)
                    } else {
                        (x1, x0)
                    };
                    Track {
                        x0: a,
                        x1: b,
                        th0: th,
                        th1: th,
                        seeds,
                        samples,
                        zone: 0,
                        contour: None,
                    }
                })
                .collect()
        }
        _ => {
            let total = TURN * length / stepover;
            let samples = (total / SAMPLE_ANGLE)
                .max(length / SAMPLE_LENGTH)
                .ceil()
                .max(1.0);
            if samples > MAX_SAMPLES {
                return Err(too_many(stepover, length, samples));
            }
            let seeds = (total / step).ceil().max(1.0) as usize;
            // A turns `rotation` ways as X increases; the drive angle the other
            // way by the sense (A = sense (90 - phi)).
            let spiral = Track {
                x0,
                x1,
                th0: th_start,
                th1: th_start - sense * rotation * total,
                seeds,
                samples: samples as usize,
                zone: 0,
                contour: None,
            };
            // A whole turn at each face across the axis, where the tool's
            // side touches it (decision record 0297), cut after the spiral
            // as its own pass, so the spiral and its smoothed A are as
            // before.
            let grown = cutter::grow_profile(input.tool, leave)?;
            // The faces are searched over the part's whole range, as for the
            // other helix strategies: a face beside a held band is found and
            // noted there, while the tracks stay outside the band (decision
            // record 0363 item 1).
            let found = faces::turns_to_the_ends(
                input,
                &grown,
                (bounds.min.x, bounds.max.x),
                tolerance,
                context,
            )?;
            held = found.held;
            let span = spiral.th1 - spiral.th0;
            face_turns = found
                .turns
                .iter()
                .map(|t| {
                    let th = spiral.th0 + span * (t.x - x0) / length;
                    (t.x, sense * (90.0 - th), rotation)
                })
                .collect();
            vec![spiral]
        }
    };

    // Where the section does not surround the axis, the drive rays aim at
    // its own centre (OP-26; decision record 0363 item 3). A part whose
    // every section surrounds the axis has one zone and is driven as before.
    let zones = section::zones(input.surface, x0, x1, radius, tolerance);
    let tracks: Vec<Track> = if zones.len() > 1 || zones.iter().any(|z| z.centres.is_some()) {
        tracks.iter().flat_map(|t| split(t, &zones)).collect()
    } else {
        tracks
    };

    let grown = cutter::grow_profile(input.tool, leave)?;
    let body = hidden::body(&grown, input.shank, leave);
    let mut plan = Plan {
        input,
        region,
        constrained,
        drop_profile: &grown,
        convex: grown.is_convex(),
        reach: input.tool.cutting_radius(),
        // Just off straight down, a round bottom's point is still at the
        // tip, a flat bottom's at its edge.
        flat_bottom: grown
            .point_for_normal(-std::f64::consts::FRAC_PI_2 + 1e-9)
            .is_some_and(|q| q.radius.0 > 1e-6),
        leave,
        tolerance,
        chord: CHORD_FACTOR * tolerance,
        outside: radius + 1.0,
        sense,
        zones: &zones,
        body: body.as_ref(),
        contours: Vec::new(),
    };

    let mut findings = Findings::default();
    let mut by_track = Vec::new();
    let count = tracks.len() as u64;
    for (k, track) in tracks.iter().enumerate() {
        context.check_cancelled()?;
        context.report(
            "simultaneous finishing: following the surface",
            k as u64,
            Some(count),
        );
        let axis = plan.axis_plan(track, lead, max_tilt, &mut findings);
        context.check_cancelled()?;
        by_track.push(plan.track_passes(track, &axis, lead, &mut findings, context)?);
    }
    let mut spaced = None;
    let mut passes: Vec<Pass> = match adaptive {
        Some(settings) => {
            let (passes, outcome) = adaptive_passes(
                &plan,
                &tracks,
                by_track,
                (settings, max_tilt, axial_seed, radius),
                &mut findings,
                context,
            )?;
            spaced = Some((settings, outcome));
            passes
        }
        None => by_track.into_iter().flatten().collect(),
    };
    // The faces' turns keep the tool across the rotary axis: the face's
    // normal lies along the axis, which no A can follow, and the tool's side
    // meets the face whatever the tilt would be.
    let turned: Vec<f64> = face_turns.iter().map(|t| t.0).collect();
    for turn in face_turns {
        context.check_cancelled()?;
        passes.extend(crate::helical::turn_passes(
            input,
            region,
            (&grown, leave, &[]),
            tolerance,
            turn,
            step,
            None,
            context,
        )?);
    }
    context.report(
        "simultaneous finishing: following the surface",
        count,
        Some(count),
    );

    // Surfaces the drive's rays do not meet first get passes of their own
    // where the tool reaches them, and the rest are reported (OP-20;
    // decision record 0368). A part with none gets nothing more.
    context.check_cancelled()?;
    let found = {
        let _timing = context.time_stage("hidden surfaces");
        plan.hidden(hidden::Settings {
            stepover,
            lead,
            max_tilt,
            max_lean,
            x0,
            x1,
        })
    };
    plan.contours = found.contours;
    let hidden_count = plan.contours.len() as u64;
    let mut hidden_x = (f64::INFINITY, f64::NEG_INFINITY);
    for k in 0..plan.contours.len() {
        context.check_cancelled()?;
        context.report(
            "simultaneous finishing: surfaces hidden from the drive",
            k as u64,
            Some(hidden_count),
        );
        let Some(c) = plan.contours.get(k) else {
            continue;
        };
        hidden_x = (hidden_x.0.min(c.x), hidden_x.1.max(c.x));
        let track = Track {
            x0: c.x,
            x1: c.x,
            th0: 0.0,
            th1: 0.0,
            seeds: (c.length() / hidden::CONTOUR_SEED).ceil().max(1.0) as usize,
            samples: c.samples.len(),
            zone: 0,
            contour: Some(k),
        };
        let axis = contour_axis_plan(c);
        passes.extend(plan.track_passes(&track, &axis, 0.0, &mut findings, context)?);
    }

    let mut notes = Vec::new();
    if let Some(at) = findings.slope_at
        && findings.slope > 0.05
    {
        notes.push(format!(
            "the surface slopes along the rotary axis, which A cannot follow: the tool axis is \
             up to {:.1} degrees from the surface normal for that reason (at X {:.3})",
            findings.slope, at.x
        ));
    }
    if max_tilt > 0.0 {
        let lead_text = if lead != 0.0 {
            format!(" plus the {lead} degree lead")
        } else {
            String::new()
        };
        notes.push(format!(
            "A is smoothed within {max_tilt} degrees of the normal{lead_text}; the largest tilt \
             used is {:.1} degrees from that",
            findings.tilt
        ));
    }
    if findings.under > 0.0 {
        notes.push(format!(
            "some moves leave up to {:.4} mm of material, more than the tolerance, because the \
             surface changes faster than the path can follow",
            findings.under
        ));
    }
    if findings.mesh {
        notes.push(
            "the tolerance is measured against the mesh, not the design surface it came from"
                .to_owned(),
        );
    }
    let off: Vec<String> = zones
        .iter()
        .filter(|z| z.centres.is_some())
        .map(|z| format!("X {:.3} to {:.3}", z.x0, z.x1))
        .collect();
    if !off.is_empty() {
        notes.push(format!(
            "the part's section does not surround the rotary axis at {}, so the path there goes \
             round the section's own centre: the tool moves across the axis as A turns, to stay \
             on the surface normal",
            off.join(", ")
        ));
    }
    notes.extend(faces::note(&turned));
    notes.extend(faces::held_note(&held));
    if let Some((x, a)) = findings.lean_at {
        let shank = if plan.body.is_some() {
            "the shank, wider than the cutting part, is kept clear of the part, as on every pass"
        } else {
            "the tool above its flutes is taken as no wider than its cutting part"
        };
        notes.push(format!(
            "surfaces hidden from the drive (such as the walls of a slot, an undercut, or sides \
             of the section facing each other) between X {:.3} and {:.3} are finished with \
             passes of their own after the main ones, the tool leaning from the surface normal \
             as little as the part lets it reach them: up to {:.1} degrees (at X {x:.3}, \
             A {a:.3}); {shank}; the tool holder is not checked",
            hidden_x.0, hidden_x.1, findings.lean
        ));
    }
    let shank_breaks = places(std::mem::take(&mut findings.shank_breaks));
    let mut warnings: Vec<crate::assemble::PathWarning> = unreached_warning(
        &found.unreached,
        2.0 * input.tool.cutting_radius(),
        max_lean,
    )
    .into_iter()
    .chain(shank_warning(&shank_breaks))
    .collect();
    if let Some((settings, outcome)) = &spaced {
        notes.push(outcome.note(settings));
        warnings.extend(outcome.warning(settings, &adaptive::Limits::OWNER, parameter::SCALLOP));
    }
    Ok(StrategyOutput {
        passes,
        notes,
        warnings,
        unreached: found.unreached,
        shank_breaks,
    })
}

/// The breaks for the shank as places: in X order, one for each run of
/// breaks within [`BREAK_NEAR`] mm of X and as many degrees of A of each
/// other (the halvings round one place find it several times).
fn places(mut breaks: Vec<crate::assemble::ShankBreak>) -> Vec<crate::assemble::ShankBreak> {
    breaks.sort_by(|a, b| {
        a.x.total_cmp(&b.x)
            .then(a.a.rem_euclid(TURN).total_cmp(&b.a.rem_euclid(TURN)))
    });
    let mut out: Vec<crate::assemble::ShankBreak> = Vec::new();
    for b in breaks {
        let close = out
            .iter()
            .rev()
            .take_while(|p| b.x - p.x <= BREAK_NEAR)
            .any(|p| {
                let da = (b.a - p.a).rem_euclid(TURN);
                da.min(TURN - da) <= BREAK_NEAR
            });
        if !close {
            out.push(b);
        }
    }
    out
}

/// How near two breaks for the shank are one place, mm along X and degrees
/// of A.
const BREAK_NEAR: f64 = 1.0;

/// The warning for the places a pass broke because the tool's shank would
/// meet the part (decision record 0368, the owner's answer on OI-1456).
fn shank_warning(breaks: &[crate::assemble::ShankBreak]) -> Option<crate::assemble::PathWarning> {
    const LISTED: usize = 5;
    if breaks.is_empty() {
        return None;
    }
    let listed: Vec<String> = breaks
        .iter()
        .take(LISTED)
        .map(|b| format!("X {:.3} near A {:.0}", b.x, b.a.rem_euclid(TURN)))
        .collect();
    let more = breaks.len().saturating_sub(LISTED);
    let more = if more > 0 {
        format!("; and {more} more")
    } else {
        String::new()
    };
    let plural = if breaks.len() == 1 { "" } else { "s" };
    Some(crate::assemble::PathWarning {
        message: format!(
            "simultaneous finishing breaks the path at {} place{plural} and links round them, \
             because the tool's shank is wider than its cutting part and would touch the part \
             there; the surface there is left uncut: {}{more}; check them in the simulation \
             before running the program",
            breaks.len(),
            listed.join("; ")
        ),
        parameter: None,
        requires_acknowledgement: false,
    })
}

/// The warning for the areas the tool cannot reach (OP-20; decision record
/// 0368): the largest five, with how many more, each with where it is, how
/// large and why. `None` when there are none.
fn unreached_warning(
    areas: &[crate::assemble::Unreached],
    diameter: f64,
    max_lean: f64,
) -> Option<crate::assemble::PathWarning> {
    use crate::assemble::UnreachedReason;
    const LISTED: usize = 5;
    if areas.is_empty() {
        return None;
    }
    let listed: Vec<String> = areas
        .iter()
        .take(LISTED)
        .map(|u| {
            let place = if u.a.1 - u.a.0 >= TURN - 1e-9 {
                "all round".to_owned()
            } else {
                format!("near A {:.0}", (0.5 * (u.a.0 + u.a.1)).rem_euclid(TURN))
            };
            let why = match u.reason {
                UnreachedReason::Blocked => "blocked: the part is in the way of every direction \
                                             the tool can take across the rotary axis, so it \
                                             would have to lean along it"
                    .to_owned(),
                UnreachedReason::TooNarrow => format!(
                    "too narrow for this {diameter:.3} mm tool, or deeper than its flutes or \
                     shank allow"
                ),
                UnreachedReason::NeedsLean { degrees } => format!(
                    "needs a lean of {degrees:.0} degrees from the surface normal, more than the \
                     largest lean on hidden surfaces ({max_lean} degrees)"
                ),
            };
            format!(
                "X {:.3} to {:.3} {place} ({:.1} mm2), {why}",
                u.x.0, u.x.1, u.area
            )
        })
        .collect();
    let more = areas.len().saturating_sub(LISTED);
    let more = if more > 0 {
        format!("; and {more} smaller")
    } else {
        String::new()
    };
    let plural = if areas.len() == 1 { "" } else { "s" };
    let lean = areas
        .iter()
        .any(|u| matches!(u.reason, UnreachedReason::NeedsLean { .. }));
    Some(crate::assemble::PathWarning {
        message: format!(
            "simultaneous finishing cannot reach {} hidden area{plural} of the part, which stay \
             uncut: {}{more}; check them in the simulation before running the program",
            areas.len(),
            listed.join("; ")
        ),
        parameter: lean.then(|| parameter::MAX_HIDDEN_LEAN.to_owned()),
        requires_acknowledgement: false,
    })
}

/// The tool-axis plan along a contour (OP-20): the reachable axis nearest
/// the target at each sample, smoothed inside the band that still reaches,
/// in runs between the samples the tool cannot reach.
fn contour_axis_plan(c: &hidden::Contour) -> AxisPlan {
    let mut plan = AxisPlan::default();
    let mut run: Vec<(f64, f64)> = Vec::new();
    let (mut lo, mut hi): (Vec<f64>, Vec<f64>) = (Vec::new(), Vec::new());
    let flush =
        |run: &mut Vec<(f64, f64)>, lo: &mut Vec<f64>, hi: &mut Vec<f64>, plan: &mut AxisPlan| {
            if !run.is_empty() {
                plan.runs.push(smooth_gates(
                    std::mem::take(run),
                    &std::mem::take(lo),
                    &std::mem::take(hi),
                ));
            }
        };
    for s in &c.samples {
        match s.gate {
            Some((best, l, h)) if !s.probe => {
                let shift = match run.last() {
                    Some(&(_, previous)) => near(best, previous) - best,
                    None => 0.0,
                };
                run.push((s.t, best + shift));
                lo.push(l + shift);
                hi.push(h + shift);
            }
            _ => flush(&mut run, &mut lo, &mut hi, &mut plan),
        }
    }
    flush(&mut run, &mut lo, &mut hi, &mut plan);
    plan
}

impl Findings {
    /// Takes in what another run of the plan found.
    fn merge(&mut self, other: &Self) {
        if other.slope > self.slope {
            self.slope = other.slope;
            self.slope_at = other.slope_at;
        }
        self.tilt = self.tilt.max(other.tilt);
        self.under = self.under.max(other.under);
        self.mesh |= other.mesh;
        if other.lean_at.is_some() && (self.lean_at.is_none() || other.lean > self.lean) {
            self.lean = other.lean;
            self.lean_at = other.lean_at;
        }
        self.shank_breaks.extend_from_slice(&other.shank_breaks);
    }
}

/// The axial pattern's passes as a family for adaptive spacing: across it,
/// the polar angle of the drive point in degrees; along it, model X.
struct Stations<'a> {
    plan: Plan<'a>,
    counters: &'a [Counting<'a>],
    max_tilt: f64,
    seed: f64,
    radius: f64,
    findings: std::sync::Mutex<Findings>,
    pose_step: f64,
    window: f64,
    station: f64,
    margin: f64,
}

impl Stations<'_> {
    /// Whether the part is driven in zones (OP-26).
    fn zoned(&self) -> bool {
        self.plan.zones.len() > 1 || self.plan.zones.iter().any(|z| z.centres.is_some())
    }

    /// The zone that holds `x`, for its drive rays.
    fn zone_of(&self, x: f64) -> usize {
        self.plan
            .zones
            .iter()
            .position(|z| x >= z.x0 && x <= z.x1)
            .unwrap_or(0)
    }
}

impl Family for Stations<'_> {
    fn pose(&self, c: f64, s: f64) -> (f64, Degrees) {
        (s, Degrees(self.plan.sense * (90.0 - c)))
    }

    fn poses(&self, _c: f64, pass: &Pass) -> Vec<Pose> {
        adaptive::machine_poses(&self.plan.input.rotary, pass, self.pose_step, |tip, _| {
            tip.x
        })
    }

    fn probe(&self, x: f64, a: Degrees) -> Option<(Point3, f64)> {
        // The drive point and the tool along its facet's normal, dropped as
        // the pattern drops it: one ray and one drop.
        let th = 90.0 - a.0 / self.plan.sense;
        let hit = self.plan.ray(self.zone_of(x), x, th)?;
        let phi = polar(hit.normal.get())?;
        let a = Degrees(self.plan.sense * (90.0 - phi));
        let axis = self.plan.input.rotary.tool_axis(a);
        let line = ToolLine {
            origin: self.plan.seed_tip(hit.point, hit.normal, axis),
            axis,
        };
        let contact = self
            .plan
            .input
            .surface
            .drop_tool(self.plan.drop_profile, &line)?;
        if let Some(region) = self.plan.region
            && !region.allows(contact.contact, contact.tip, self.plan.drop_profile, &line)
        {
            return None;
        }
        Some((contact.contact, self.plan.leave))
    }

    fn probe_cost(&self) -> u64 {
        2
    }

    fn fit(&self, c: f64, (from, to): (f64, f64), context: &JobContext<'_>) -> Result<Vec<Pass>> {
        let length = (to - from).abs();
        let whole = Track {
            x0: from,
            x1: to,
            th0: c,
            th1: c,
            seeds: (length / self.seed).ceil().max(1.0) as usize,
            samples: (length / SAMPLE_LENGTH).ceil().max(1.0) as usize,
            zone: 0,
            contour: None,
        };
        // Cut at the zones as the pattern's own tracks are (OP-26).
        let pieces = if self.zoned() {
            split(&whole, self.plan.zones)
        } else {
            vec![whole]
        };
        let mut findings = Findings::default();
        let mut passes = Vec::new();
        for track in &pieces {
            let axis = self
                .plan
                .axis_plan(track, 0.0, self.max_tilt, &mut findings);
            passes.extend(
                self.plan
                    .track_passes(track, &axis, 0.0, &mut findings, context)?,
            );
        }
        if let Ok(mut all) = self.findings.lock() {
            all.merge(&findings);
        }
        Ok(passes)
    }

    fn width(&self, dc: f64) -> f64 {
        self.radius * dc.to_radians().abs()
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
}

/// The axial pattern's passes, `by_track` along `tracks`, with passes added
/// between them where more than the Allowed excess is left (decision record
/// 0239), in order round the axis, every other one backwards. `plan` is the
/// operation's own.
fn adaptive_passes(
    plan: &Plan<'_>,
    tracks: &[Track],
    by_track: Vec<Vec<Pass>>,
    (settings, max_tilt, seed, radius): (adaptive::Settings, f64, f64, f64),
    findings: &mut Findings,
    context: &JobContext<'_>,
) -> Result<(Vec<Pass>, adaptive::Outcome)> {
    let input = plan.input;
    // The measure and the added passes look through a counter, so their
    // geometry queries are limited (decision record 0239 item 8).
    let counters = vec![Counting::new(input.surface)];
    let Some(first) = counters.first() else {
        return Err(PathError::Strategy("no surface to finish".to_owned()));
    };
    let counted = StrategyInput {
        surface: first,
        ..*input
    };
    let r = input.tool.cutting_radius();
    let reach = plan.drop_profile.cutting_radius();
    let station = AXIAL_SEED.min(0.5 * r).max(1e-3);
    let family = Stations {
        plan: Plan {
            input: &counted,
            region: plan.region,
            constrained: plan.constrained,
            drop_profile: plan.drop_profile,
            convex: plan.convex,
            reach: plan.reach,
            flat_bottom: plan.flat_bottom,
            leave: plan.leave,
            tolerance: plan.tolerance,
            chord: plan.chord,
            outside: plan.outside,
            zones: plan.zones,
            sense: plan.sense,
            body: plan.body,
            contours: Vec::new(),
        },
        counters: &counters,
        max_tilt,
        seed,
        radius,
        findings: std::sync::Mutex::new(Findings::default()),
        pose_step: adaptive::pose_step(r, settings.height),
        window: 2.0 * reach + station,
        station,
        margin: reach,
    };
    // One track a station: a station's pieces in each zone (OP-26) are
    // one pass of the family.
    let mut stations: Vec<(f64, Vec<Pass>)> = Vec::new();
    for (t, passes) in tracks.iter().zip(by_track) {
        match stations.last_mut() {
            Some((th, all)) if (*th - t.th0).abs() < 1e-12 => all.extend(passes),
            _ => stations.push((t.th0, passes)),
        }
    }
    let n = stations.len();
    let mut own: Vec<adaptive::Track> = stations
        .into_iter()
        .map(|(th, passes)| adaptive::Track::new(&family, th, passes))
        .collect();
    // The last station's neighbour is the first, a turn on (the stations
    // turn the drive angle downwards).
    if let Some(first) = own.first() {
        let (th, passes) = (first.c, first.passes.clone());
        own.push(adaptive::Track::new(&family, th - TURN, passes));
    }
    let points: u64 = own
        .iter()
        .take(n)
        .flat_map(|t| &t.passes)
        .map(|p| p.points.len() as u64 + adaptive::LINK_POINTS)
        .sum();
    let (lo, hi) = tracks
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), t| {
            (lo.min(t.x0).min(t.x1), hi.max(t.x0).max(t.x1))
        });
    let span = if lo <= hi { (lo, hi) } else { (0.0, 0.0) };
    let intervals: Vec<Interval> = (0..n)
        .map(|i| Interval {
            lo: i,
            hi: i + 1,
            spans: vec![span],
            level: 0,
        })
        .collect();
    let outcome = adaptive::refine(
        &family,
        own,
        intervals,
        input.tool,
        &settings,
        &adaptive::Limits::OWNER,
        points,
        context,
    )?;
    if let Ok(added) = family.findings.lock() {
        findings.merge(&added);
    }
    // In order of the drive angle, downwards, as the stations are cut.
    let mut order: Vec<&adaptive::Track> = outcome
        .tracks
        .iter()
        .enumerate()
        .filter(|&(i, _)| i != n)
        .map(|(_, t)| t)
        .collect();
    order.sort_by(|p, q| q.c.total_cmp(&p.c));
    let mut out = Vec::new();
    for (i, track) in order.into_iter().enumerate() {
        if i % 2 == 1 {
            out.extend(track.passes.iter().rev().cloned().map(|mut pass| {
                if pass.rapids.is_empty() {
                    pass.points.reverse();
                }
                pass
            }));
        } else {
            out.extend(track.passes.iter().cloned());
        }
    }
    Ok((out, outcome))
}

/// `track` cut at the zones' boundaries, in the track's own order, each
/// piece with its zone and its share of the seeds and samples (OP-26).
fn split(track: &Track, zones: &[section::Zone]) -> Vec<Track> {
    let dx = track.x1 - track.x0;
    if dx.abs() < 1e-12 {
        return vec![*track];
    }
    let mut pieces: Vec<(f64, f64, usize)> = zones
        .iter()
        .enumerate()
        .filter_map(|(k, z)| {
            let (ta, tb) = ((z.x0 - track.x0) / dx, (z.x1 - track.x0) / dx);
            let (lo, hi) = (ta.min(tb).max(0.0), ta.max(tb).min(1.0));
            (hi > lo).then_some((lo, hi, k))
        })
        .collect();
    pieces.sort_by(|a, b| a.0.total_cmp(&b.0));
    pieces
        .into_iter()
        .map(|(lo, hi, zone)| {
            let ((x0, th0), (x1, th1)) = (track.at(lo), track.at(hi));
            let share = |n: usize| ((n as f64 * (hi - lo)).ceil() as usize).max(1);
            Track {
                x0,
                x1,
                th0,
                th1,
                seeds: share(track.seeds),
                samples: share(track.samples),
                zone,
                contour: None,
            }
        })
        .collect()
}

/// The part's X range `x0` to `x1` less the held keep-out bands that cover
/// its ends (decision record 0363 item 1, OI-1372): the drop surface holds
/// the tabs from the held regions' inner edges, and a track there would ride
/// past the held stock's face, where the tool's tip may not go while it cuts
/// (decision record 0161). The faces' turns skip the bands the same way.
fn outside_held(mut x0: f64, mut x1: f64, keep_out: &[(f64, f64)]) -> (f64, f64) {
    loop {
        let (start, end) = (x0, x1);
        for &(a, b) in keep_out {
            if a <= x0 && x0 < b {
                x0 = b;
            }
            if a < x1 && x1 <= b {
                x1 = a;
            }
        }
        if (x0, x1) == (start, end) {
            return (x0, x1);
        }
    }
}

fn too_many(stepover: f64, length: f64, samples: f64) -> PathError {
    PathError::parameter(
        names::STEPOVER,
        format!(
            "{stepover} mm over {length:.1} mm of part needs {samples:.0} surface samples; \
             increase the stepover"
        ),
    )
}

/// The polar angle of `v`'s part perpendicular to X, degrees.
fn polar(v: Vec3) -> Option<f64> {
    (v.y.abs() > 1e-15 || v.z.abs() > 1e-15).then(|| v.z.atan2(v.y).to_degrees())
}

/// The equivalent of `angle` (degrees) nearest `previous`.
fn near(angle: f64, previous: f64) -> f64 {
    angle + TURN * ((previous - angle) / TURN).round()
}

impl Plan<'_> {
    /// The drive point at (`x`, polar angle `th`): the first surface point on
    /// the ray from outside the part towards the axis.
    fn drive(&self, track: &Track, x: f64, th: f64) -> Option<Drive> {
        let hit = self.ray(track.zone, x, th)?;
        let exact = hit.position.is_some();
        let normal = if exact {
            hit.normal
        } else {
            self.smoothed_normal(track.zone, x, th, hit.point, hit.normal)
        };
        let phi = polar(normal.get())?;
        Some(Drive {
            point: hit.point,
            normal,
            phi,
            exact,
        })
    }

    /// The drive point at `t` on `track`: on its contour where it follows
    /// one (OP-20), else where the drive's ray meets the surface.
    fn drive_at(&self, track: &Track, t: f64) -> Option<Drive> {
        let Some(k) = track.contour else {
            let (x, th) = track.at(t);
            return self.drive(track, x, th);
        };
        let (point, normal, exact) = self.contours.get(k)?.drive(self, t)?;
        Some(Drive {
            point,
            normal,
            phi: polar(normal.get()).unwrap_or(0.0),
            exact,
        })
    }

    /// The ray of the drive at (`x`, `th`) in `zone`: towards the axis, or
    /// towards the section's own centre where the zone has one (OP-26).
    fn ray(&self, zone: usize, x: f64, th: f64) -> Option<crate::surface::RayHit> {
        if let Some(z) = self.zones.get(zone)
            && z.centres.is_some()
        {
            let centre = z.centre_at(x);
            let outside = self.outside + centre.0.hypot(centre.1);
            return section::hit_towards(self.input.surface, x, centre, th, outside);
        }
        let (s, c) = th.to_radians().sin_cos();
        let origin = Point3::new(x, self.outside * c, self.outside * s);
        let direction = UnitVec3::new(Vec3::new(0.0, -c, -s)).ok()?;
        self.input.surface.cast_ray(origin, direction, self.outside)
    }

    /// The normal of the surface through four neighbouring drive points
    /// [`NORMAL_SPAN`] away, which averages out the facet normals of a mesh
    /// (OI-67). The facet normal where a neighbour is missing.
    fn smoothed_normal(
        &self,
        zone: usize,
        x: f64,
        th: f64,
        p: Point3,
        facet: UnitVec3,
    ) -> UnitVec3 {
        // A neighbour in another zone is on the far side of a face across
        // the axis, and its rays aim at another centre: as missing.
        if self.zones.len() > 1
            && let Some(z) = self.zones.get(zone)
            && (x - NORMAL_SPAN < z.x0 || x + NORMAL_SPAN > z.x1)
        {
            return facet;
        }
        let (cy, cz) = self.zones.get(zone).map_or((0.0, 0.0), |z| z.centre_at(x));
        let r = (p.y - cy).hypot(p.z - cz);
        if r <= NORMAL_SPAN {
            return facet;
        }
        let dth = (NORMAL_SPAN / r).to_degrees();
        let pt = |x: f64, th: f64| self.ray(zone, x, th).map(|h| h.point);
        let (Some(xp), Some(xm), Some(tp), Some(tm)) = (
            pt(x + NORMAL_SPAN, th),
            pt(x - NORMAL_SPAN, th),
            pt(x, th + dth),
            pt(x, th - dth),
        ) else {
            return facet;
        };
        let n = (xp - xm).cross(tp - tm);
        let n = if n.dot(facet.get()) < 0.0 { -n } else { n };
        match UnitVec3::new(n) {
            // A neighbour on another face of an edge: keep the facet normal.
            Ok(u) if u.get().dot(facet.get()) > 0.5 => u,
            _ => facet,
        }
    }

    /// The tool-axis plan for a track: dense drive samples, the target
    /// angles, and the taut string through the tilt band.
    fn axis_plan(
        &self,
        track: &Track,
        lead: f64,
        max_tilt: f64,
        findings: &mut Findings,
    ) -> AxisPlan {
        let n = track.samples;
        let ts: Vec<f64> = (0..=n).map(|i| i as f64 / n as f64).collect();
        let drives: Vec<Option<Drive>> = parallel::map_in_order(&ts, |t| {
            let (x, th) = track.at(*t);
            self.drive(track, x, th)
        });
        let offset = lead * track.travel();
        let mut plan = AxisPlan::default();
        let mut run: Vec<(f64, f64)> = Vec::new();
        let flush = |run: &mut Vec<(f64, f64)>, plan: &mut AxisPlan| {
            if !run.is_empty() {
                plan.runs.push(smooth_band(std::mem::take(run), max_tilt));
            }
        };
        for (t, d) in ts.iter().zip(&drives) {
            match d {
                Some(d) => {
                    findings.mesh |= !d.exact;
                    let target = d.phi + offset;
                    let target = match run.last() {
                        Some(&(_, previous)) => near(target, previous),
                        None => target,
                    };
                    run.push((*t, target));
                }
                None => flush(&mut run, &mut plan),
            }
        }
        flush(&mut run, &mut plan);
        plan
    }

    /// The tool at `t` on `track`, or `None` where there is no surface or
    /// the region forbids a cut.
    fn node(&self, track: &Track, axis: &AxisPlan, t: f64) -> Option<Node> {
        let drive = self.drive_at(track, t)?;
        // The plan's angle is continuous along the track; keep it so, and
        // take the normal's angle as its equivalent nearest it.
        let phi = axis.phi_at(t)?;
        let a = Degrees(self.sense * (90.0 - phi));
        let axis_dir = self.input.rotary.tool_axis(a);
        let line = ToolLine {
            origin: self.seed_tip(drive.point, drive.normal, axis_dir),
            axis: axis_dir,
        };
        // On a hidden surface the tool is placed touching its drive point,
        // and rests there or higher (OP-20).
        let contact = if track.contour.is_some() {
            self.input
                .surface
                .drop_tool_above(self.drop_profile, &line, 0.0)?
        } else {
            self.input.surface.drop_tool(self.drop_profile, &line)?
        };
        if let Some(region) = self.region
            && !region.allows(contact.contact, contact.tip, self.drop_profile, &line)
        {
            return None;
        }
        // The shank, where wider than the cutting part, must not meet the
        // part: the pass breaks there (decision record 0368, the owner's
        // answer on OI-1456).
        if track.contour.is_none() && self.body.is_some() && self.shank_clashes(&line, None) {
            return None;
        }
        // On a hidden surface the tool must rest on its drive point, not on
        // something in the way between the samples where its axis was
        // found to reach, and the shank, where wider than the cutting part,
        // must not meet the part (OP-20).
        if track.contour.is_some() {
            let d = drive.point - contact.tip;
            let along = d.dot(axis_dir.get());
            let across = (d - axis_dir.get() * along).length();
            if hidden::to_tool(self.drop_profile, across, along) > self.tolerance
                || self.shank_clashes(&line, Some(0.0))
            {
                return None;
            }
        }
        Some(Node {
            t,
            a,
            contact,
            normal: drive.normal,
            phi_n: near(drive.phi, phi),
            phi,
            raised: false,
        })
    }

    /// Where the tool at `t` on `track` is lost because its shank would meet
    /// the part: its X and A. `None` when there is no wider shank, or the
    /// tool is lost for another reason or not at all.
    fn shank_at(
        &self,
        track: &Track,
        axis: &AxisPlan,
        t: f64,
    ) -> Option<crate::assemble::ShankBreak> {
        self.body?;
        let drive = self.drive_at(track, t)?;
        let phi = axis.phi_at(t)?;
        let a = Degrees(self.sense * (90.0 - phi));
        let axis_dir = self.input.rotary.tool_axis(a);
        let line = ToolLine {
            origin: self.seed_tip(drive.point, drive.normal, axis_dir),
            axis: axis_dir,
        };
        let floor = track.contour.map(|_| 0.0);
        self.shank_clashes(&line, floor)
            .then_some(crate::assemble::ShankBreak {
                x: drive.point.x,
                a: a.0,
            })
    }

    /// Where the tip goes for the tool's point with outward normal `-n` to
    /// sit on `p` (convex profiles), or on the axis through `p` otherwise.
    fn seed_tip(&self, p: Point3, n: UnitVec3, axis: UnitVec3) -> Point3 {
        if !self.convex {
            return p;
        }
        let t = axis.get();
        let d = -n.get();
        let along = d.dot(t);
        let radial = d - t * along;
        let b = radial.length();
        let angle = along.atan2(b).min(0.0);
        let Some(q) = self.drop_profile.point_for_normal(angle) else {
            return p;
        };
        let e = if b > 1e-12 {
            radial * (1.0 / b)
        } else {
            Vec3::ZERO
        };
        // A flat bottom's contact is not unique near the normal: the corner
        // on the drive point flips sides as the axis crosses the normal, and
        // the planned axis is only within a few 1e-6 rad of it, so neighbouring
        // points jumped by the tool's diameter (decision record 0363 item 2,
        // OI-337). Such a tool's seed moves from its centre to its corner in
        // step with the lean, reaching the corner at `FLAT_BLEND`; the drop
        // then lowers it onto the surface. A round bottom is seeded as before.
        let radius = if self.flat_bottom {
            q.radius.0 * (b / FLAT_BLEND).min(1.0)
        } else {
            q.radius.0
        };
        p + (-(t * q.height.0 + e * radius))
    }

    /// The original tool's tip for a grown-tool drop.
    fn tip(&self, n: &Node) -> Point3 {
        let axis = self.input.rotary.tool_axis(n.a);
        n.contact.tip + axis.get() * self.leave
    }

    fn cut_point(&self, n: &Node) -> CutPoint {
        let tip = self.tip(n);
        let axis = self.input.rotary.tool_axis(n.a);
        let contact = if self.leave > 0.0 {
            cutter::cutter_contact(self.input.tool, tip, axis, n.contact.contact)
        } else {
            n.contact.contact
        };
        CutPoint {
            tip,
            a: n.a,
            contact,
        }
    }

    /// The error of the straight machine move from `a` to `b` at the node
    /// `at` between them: the tool is dropped along its axis at that machine
    /// pose, and the distance it moves is taken along the surface normal.
    /// Positive when the move would cut into the surface, negative when it
    /// would leave material; `None` when there is no surface under it. A is
    /// linear in the track parameter, so the straight move reaches the same
    /// fraction of every axis.
    ///
    /// On a hidden surface (OP-20) a cut in is the depth inside the tool,
    /// at the straight move's pose, of the point the drop came to rest on:
    /// there the axis is far from the normal and changes quickly at an
    /// edge, and the drive normal does not measure it.
    fn deviation(&self, track: &Track, a: &Node, b: &Node, at: &Node) -> Option<f64> {
        let (line, _) = self.straight_pose(a, b, at);
        self.input
            .surface
            .drop_tool(self.drop_profile, &line)
            .map(|c| {
                if track.contour.is_some() && c.offset > 0.0 {
                    let d = c.contact - line.origin;
                    let along = d.dot(line.axis.get());
                    let across = (d - line.axis.get() * along).length();
                    return hidden::to_tool(self.drop_profile, across, along).min(c.offset);
                }
                let along = line.axis.get().dot(at.normal.get()).max(0.0);
                // A cut in is never read at less than half its depth along
                // the axis: on a face steep along X (a blisk's blade) the drive
                // normal is nearly across the tool axis, and a drop of tenths
                // of a millimetre read as thousandths (decision record 0363
                // item 2). A wall the tool only slides on is a wall move.
                if c.offset > 0.0 {
                    c.offset * along.max(MIN_ALONG)
                } else {
                    c.offset * along
                }
            })
    }

    /// The tool's line and A on the straight machine move from `a` to `b`
    /// at the track parameter of `at`.
    fn straight_pose(&self, a: &Node, b: &Node, at: &Node) -> (ToolLine, Degrees) {
        let rotary = &self.input.rotary;
        let span = b.t - a.t;
        let frac = if span > 0.0 { (at.t - a.t) / span } else { 0.5 };
        let (pa, pb) = (
            rotary.to_axis(a.contact.tip, a.a),
            rotary.to_axis(b.contact.tip, b.a),
        );
        let lin = AxisPosition {
            along: pa.along + frac * (pb.along - pa.along),
            across: pa.across + frac * (pb.across - pa.across),
            height: pa.height + frac * (pb.height - pa.height),
        };
        let a_lin = Degrees(a.a.0 + frac * (crate::rotary::nearest_turn(b.a, a.a).0 - a.a.0));
        let line = ToolLine {
            origin: rotary.to_model(lin, a_lin),
            axis: rotary.tool_axis(a_lin),
        };
        (line, a_lin)
    }

    /// The tool at the middle of the straight move from `a` to `b` (at the
    /// track parameter of `at`), raised along its axis until it touches
    /// (decision record 0363 item 2): where the drive point passes from one
    /// surface to another in a concave corner, the seeded tool lands on one
    /// side or the other however close the two points are, and the straight
    /// move between them would dip into the corner. This pose touches both,
    /// so the move is kept as two moves through it.
    fn raised(&self, a: &Node, b: &Node, at: &Node) -> Option<Node> {
        let (line, a_lin) = self.straight_pose(a, b, at);
        let contact = self.input.surface.drop_tool(self.drop_profile, &line)?;
        Some(Node {
            t: at.t,
            a: a_lin,
            contact,
            phi: 90.0 - self.sense * a_lin.0,
            raised: true,
            ..*at
        })
    }

    /// How far the tool's tip travels over the part on the straight move
    /// from `a` to `b`: the larger of the straight distance between its ends
    /// and the arc A turns it through at its distance from the rotary axis.
    fn travel(&self, a: &Node, b: &Node) -> f64 {
        let turn = (crate::rotary::nearest_turn(b.a, a.a).0 - a.a.0)
            .abs()
            .to_radians();
        let (pa, pb) = (a.contact.tip, b.contact.tip);
        let r = pa.y.hypot(pa.z).max(pb.y.hypot(pb.z));
        (pb - pa).length().max(turn * r)
    }

    /// Whether a move whose tip travels further than [`reach`](Plan::reach)
    /// fails the move test at one of the poses a quarter of the reach apart
    /// along it (decision record 0363 item 2). A move that travels no
    /// further is tested at its middle and quarter points alone, as before,
    /// and so is a long move that holds at every pose: it is kept whole.
    fn long_move_cuts(&self, track: &Track, axis: &AxisPlan, a: &Node, b: &Node) -> bool {
        let travel = self.travel(a, b);
        if travel <= self.reach {
            return false;
        }
        let n = ((4.0 * travel / self.reach).ceil() as usize).clamp(4, MAX_TEST_POSES);
        (1..n).any(|k| {
            let t = a.t + (b.t - a.t) * k as f64 / n as f64;
            match self.node(track, axis, t) {
                Some(at) => self
                    .deviation(track, a, b, &at)
                    .is_none_or(|e| e.abs() > self.chord),
                None => self.gappy(track),
            }
        })
    }

    /// Whether the move from `a` to `b` is a wall move (decision record 0050
    /// item 2; decision record 0363 item 2, OI-337): its tip moves across the
    /// tool axis, and A turns the tool's edge, by less than the tolerance, so
    /// the machine moves the tool
    /// along its own axis, which sweeps only the tool's own body above the
    /// lower end and cuts nothing that end does not. A wall across the rotary
    /// axis (a shoulder, a held region's face, a tab meeting the part) makes
    /// the drop height jump there, which no halving removes; the move between
    /// is kept, as path assembly keeps it for the other strategies.
    fn wall_move(&self, a: &Node, b: &Node) -> bool {
        let rotary = &self.input.rotary;
        let mid = Degrees(0.5 * (a.a.0 + crate::rotary::nearest_turn(b.a, a.a).0));
        let axis = rotary.tool_axis(mid).get();
        let d = b.contact.tip - a.contact.tip;
        let turn = (crate::rotary::nearest_turn(b.a, a.a).0 - a.a.0)
            .abs()
            .to_radians();
        let r = a.contact.tip.y.hypot(a.contact.tip.z) + self.reach;
        (d - axis * d.dot(axis)).length() < self.tolerance && turn * r < self.tolerance
    }

    /// The node nearest `bad_t` where the tool may still cut, by bisection
    /// from `from`.
    fn edge(&self, track: &Track, axis: &AxisPlan, from: &Node, bad_t: f64) -> Option<Node> {
        let (mut good, mut bad) = (from.t, bad_t);
        let mut best = None;
        for _ in 0..EDGE_STEPS {
            let mid = 0.5 * (good + bad);
            match self.node(track, axis, mid) {
                Some(n) => {
                    good = mid;
                    best = Some(n);
                }
                None => bad = mid,
            }
        }
        best
    }

    /// Halves the move `a` to `b` until it is within the chord at its
    /// middle and at the middles of its halves; pushes the points after `a`,
    /// ending with `b`. `None` marks a gap where the region forbids cutting.
    fn refine(
        &self,
        track: &Track,
        axis: &AxisPlan,
        a: &Node,
        b: &Node,
        depth: u32,
        out: &mut Refined,
    ) -> Result<()> {
        self.refine_from(track, axis, a, b, depth, None, out)
    }

    /// [`refine`](Self::refine) with the drop at the middle already known
    /// (`Some(middle)`), as it is for each half of a move that was split: the
    /// quarter points of the move are the middles of its halves, so each
    /// drop is made once (decision record 0133).
    #[allow(clippy::too_many_arguments)]
    fn refine_from(
        &self,
        track: &Track,
        axis: &AxisPlan,
        a: &Node,
        b: &Node,
        depth: u32,
        middle: Option<Option<Node>>,
        out: &mut Refined,
    ) -> Result<()> {
        let t = 0.5 * (a.t + b.t);
        // On a hidden surface a test pose where the shank meets the part
        // fails too (OP-20).
        let clash = |q: &Node| {
            self.body.is_some() && self.shank_clashes(&self.straight_pose(a, b, q).0, None)
        };
        let middle = middle.unwrap_or_else(|| self.node(track, axis, t));
        // A middle lost because the shank would meet the part breaks the
        // pass, as on a hidden surface (decision record 0368).
        let shank_mid = if middle.is_none() {
            self.shank_at(track, axis, t)
        } else {
            None
        };
        match middle {
            Some(mid) => {
                let over = self.deviation(track, a, b, &mid);
                let error = over.unwrap_or(0.0);
                let jumps = self.travel(a, b) > self.reach;
                if depth < MAX_DEPTH {
                    // The straight move must hold at the quarter points too.
                    // A quarter with no material under it is no reason to
                    // split, unless a region constraint forbids cutting
                    // there: then the half finds the gap as it would at its
                    // own middle. A test pose of the straight move that meets
                    // no surface while the drive has one there says nothing
                    // about the move: between thin walls (the blades of a
                    // blisk) all three can miss while the tool sweeps through
                    // a wall between them, so the move is split (decision
                    // record 0363 item 2).
                    let quarter = |n: &Option<Node>| match n {
                        Some(q) => {
                            self.deviation(track, a, b, q)
                                .is_none_or(|e| e.abs() > self.chord)
                                || clash(q)
                        }
                        None => self.gappy(track),
                    };
                    let first = self.node(track, axis, 0.5 * (a.t + mid.t));
                    let second = self.node(track, axis, 0.5 * (mid.t + b.t));
                    if over.is_none_or(|e| e.abs() > self.chord)
                        || clash(&mid)
                        || self.long_move_cuts(track, axis, a, b)
                        || quarter(&first)
                        || quarter(&second)
                    {
                        self.refine_from(track, axis, a, &mid, depth + 1, Some(first), out)?;
                        return self.refine_from(
                            track,
                            axis,
                            &mid,
                            b,
                            depth + 1,
                            Some(second),
                            out,
                        );
                    }
                } else if clash(&mid) {
                    // The shank still meets the part after every halving:
                    // the pass breaks here, and the move is a link.
                    out.breaks.push(crate::assemble::ShankBreak {
                        x: mid.contact.contact.x,
                        a: mid.a.0,
                    });
                    out.points.push(None);
                } else if (error.abs() > self.chord || jumps) && !self.wall_move(a, b) {
                    // After every halving the tool still cuts in, or still
                    // jumps further than its radius between two points as
                    // good as equal on the track (where the drive passes
                    // from one surface to another): the move is kept only as
                    // moves through the raised tool at its middle.
                    if (error > self.tolerance || jumps)
                        && depth < MAX_DEPTH + RAISES
                        && let Some(raised) = self.raised(a, b, &mid)
                    {
                        self.refine_from(track, axis, a, &raised, depth + 1, None, out)?;
                        return self.refine_from(track, axis, &raised, b, depth + 1, None, out);
                    }
                    if error > self.tolerance && track.contour.is_some() {
                        // A pass on a hidden surface breaks where it cannot
                        // be kept within the tolerance (OP-20).
                        out.points.push(None);
                    } else if error > self.tolerance {
                        return Err(PathError::Strategy(format!(
                            "simultaneous finishing cannot keep the move to X {:.3}, A {:.3} \
                             within the tolerance: it would cut {error:.4} mm into the surface; \
                             the tool jumps between two surfaces there (a sharp edge, or a wall \
                             the drive meets), so a smaller largest tilt or angle step does not \
                             help: limit the operation to a region away from it, or finish that \
                             part with helical finishing",
                            b.contact.tip.x, b.a.0
                        )));
                    }
                    if -error > self.tolerance {
                        out.under = out.under.max(-error);
                    }
                }
            }
            None if (self.gappy(track) || shank_mid.is_some()) && depth < MAX_DEPTH => {
                out.breaks.extend(shank_mid);
                if let Some(left) = self.edge(track, axis, a, t) {
                    self.refine(track, axis, a, &left, depth + 1, out)?;
                }
                out.points.push(None);
                match self.edge(track, axis, b, t) {
                    Some(right) => {
                        out.points.push(Some(right));
                        self.refine(track, axis, &right, b, depth + 1, out)?;
                    }
                    None => out.points.push(Some(*b)),
                }
                return Ok(());
            }
            // A contour's tool that cannot reach its drive point at the
            // middle after every halving: the pass breaks there (OP-20).
            None if track.contour.is_some() || shank_mid.is_some() => {
                out.breaks.extend(shank_mid);
                out.points.push(None);
            }
            None => {}
        }
        out.points.push(Some(*b));
        Ok(())
    }

    /// Whether a node where the tool may not cut breaks the pass, rather
    /// than meaning there is no surface under it: with a region constraint
    /// (OI-138), and on a hidden surface, where it is a point the tool does
    /// not reach (OP-20).
    fn gappy(&self, track: &Track) -> bool {
        self.constrained || track.contour.is_some()
    }

    fn track_passes(
        &self,
        track: &Track,
        axis: &AxisPlan,
        lead: f64,
        findings: &mut Findings,
        context: &JobContext<'_>,
    ) -> Result<Vec<Pass>> {
        let n = track.seeds;
        let ts: Vec<f64> = (0..=n).map(|i| i as f64 / n as f64).collect();
        let nodes: Vec<Option<Node>> = parallel::map_in_order(&ts, |t| self.node(track, axis, *t));
        context.check_cancelled()?;
        // Where a seed is lost because the shank would meet the part, the
        // pass breaks there and the break is said (decision record 0368).
        let lost: Vec<Option<crate::assemble::ShankBreak>> = if self.body.is_some() {
            parallel::map_in_order(&ts.iter().zip(&nodes).collect::<Vec<_>>(), |(t, n)| {
                if n.is_none() {
                    self.shank_at(track, axis, **t)
                } else {
                    None
                }
            })
        } else {
            vec![None; ts.len()]
        };
        let breaks = |i: usize| self.gappy(track) || lost.get(i).is_some_and(Option::is_some);

        let mut pieces: Vec<Vec<Node>> = Vec::new();
        let mut current: Vec<Node> = Vec::new();
        for (i, node) in nodes.iter().enumerate() {
            match node {
                Some(n) => {
                    if current.is_empty()
                        && i.checked_sub(1).is_some_and(breaks)
                        && let Some(previous) = i.checked_sub(1).and_then(|p| ts.get(p))
                        && let Some(edge) = self.edge(track, axis, n, *previous)
                    {
                        current.push(edge);
                    }
                    current.push(*n);
                }
                None => {
                    findings.shank_breaks.extend(lost.get(i).copied().flatten());
                    if let Some(last) = current.last().copied() {
                        if breaks(i)
                            && let Some(next) = ts.get(i)
                            && let Some(edge) = self.edge(track, axis, &last, *next)
                        {
                            current.push(edge);
                        }
                        pieces.push(std::mem::take(&mut current));
                    }
                }
            }
        }
        if !current.is_empty() {
            pieces.push(current);
        }

        let _timing = context.time_stage("fitting");
        let mut passes = Vec::new();
        for piece in &pieces {
            context.check_cancelled()?;
            let intervals: Vec<(&Node, &Node)> = piece.iter().zip(piece.iter().skip(1)).collect();
            let refined: Vec<Result<Refined>> = parallel::map_in_order(&intervals, |(a, b)| {
                let mut out = Refined::default();
                self.refine(track, axis, a, b, 0, &mut out).map(|()| out)
            });
            let mut points = Vec::new();
            let mut breaks = Vec::new();
            let offset = lead * track.travel();
            let mut under = 0.0_f64;
            let mut push = |n: &Node, points: &mut Vec<CutPoint>| {
                if track.contour.is_some() {
                    self.note_hidden(n, findings);
                } else {
                    self.note(n, offset, findings);
                }
                points.push(self.cut_point(n));
            };
            if let Some(first) = piece.first() {
                push(first, &mut points);
            }
            for steps in refined {
                let steps = steps?;
                under = under.max(steps.under);
                breaks.extend(steps.breaks);
                for step in steps.points {
                    match step {
                        Some(n) => push(&n, &mut points),
                        None => {
                            if !points.is_empty() {
                                passes.push(Pass {
                                    helical_pass_start: false,
                                    points: std::mem::take(&mut points),
                                    stock: None,
                                    rapids: Vec::new(),
                                });
                            }
                        }
                    }
                }
            }
            findings.under = findings.under.max(under);
            findings.shank_breaks.extend(breaks);
            if !points.is_empty() {
                passes.push(Pass {
                    helical_pass_start: false,
                    points,
                    stock: None,
                    rapids: Vec::new(),
                });
            }
        }
        Ok(passes)
    }

    /// The lean from the normal of a tool on a hidden surface (OP-20).
    fn note_hidden(&self, n: &Node, findings: &mut Findings) {
        if n.raised {
            return;
        }
        let lean = (n.phi - n.phi_n).abs();
        if findings.lean_at.is_none() || lean > findings.lean {
            findings.lean = lean;
            findings.lean_at = Some((n.contact.contact.x, n.a.0));
        }
    }

    fn note(&self, n: &Node, offset: f64, findings: &mut Findings) {
        let slope = n.normal.get().x.abs().clamp(0.0, 1.0).asin().to_degrees();
        if slope > findings.slope {
            findings.slope = slope;
            findings.slope_at = Some(n.contact.contact);
        }
        if !n.raised {
            findings.tilt = findings.tilt.max((n.phi - n.phi_n - offset).abs());
        }
    }
}

/// The tool-axis angles for the samples `(t, target)` inside the band
/// `target +/- width` (decision record 0036 item 5), with the fewest
/// reversals any path in the band can have and, within that, as close to
/// the targets as possible. Exact targets for a band of width 0.
///
/// The taut string through the band, with free ends, fixes where the angle
/// must turn back: it has the fewest changes of direction of any path in
/// the band. Between those turns the angle is the monotone function nearest
/// the targets in the largest deviation (the mean of the running maximum
/// from one side and the running minimum from the other), which equals the
/// target wherever the target is already monotone. At a turn the taut
/// string lies on the band's edge, so the target there is the extreme of
/// the targets on both sides and the two fits meet on it.
fn smooth_band(samples: Vec<(f64, f64)>, width: f64) -> Vec<(f64, f64)> {
    if width <= 0.0 || samples.len() < 3 {
        return samples;
    }
    let lo: Vec<f64> = samples.iter().map(|s| s.1 - width).collect();
    let hi: Vec<f64> = samples.iter().map(|s| s.1 + width).collect();
    smooth_gates(samples, &lo, &hi)
}

/// [`smooth_band`] with a band of its own at each sample, `lo` to `hi`,
/// holding its target: the angles that reach a hidden surface (OP-20).
fn smooth_gates(samples: Vec<(f64, f64)>, lo: &[f64], hi: &[f64]) -> Vec<(f64, f64)> {
    if samples.len() < 3 || lo.iter().zip(hi).all(|(l, h)| h - l <= 0.0) {
        return samples;
    }
    let xs: Vec<f64> = samples.iter().map(|s| s.0).collect();
    let ys: Vec<f64> = samples.iter().map(|s| s.1).collect();
    let start = free_level(lo.iter().zip(hi), ys.first().copied().unwrap_or(0.0));
    let end = free_level(lo.iter().zip(hi).rev(), ys.last().copied().unwrap_or(0.0));
    let vertices = taut_vertices(&xs, lo, hi, start, end);

    // Direction of each piece of the string; a flat piece keeps the one
    // before it (or after it, at the start).
    let mut dirs: Vec<i8> = vertices
        .windows(2)
        .map(|w| match w {
            [(_, a), (_, b)] if b - a > 1e-12 => 1,
            [(_, a), (_, b)] if a - b > 1e-12 => -1,
            _ => 0,
        })
        .collect();
    let mut last = dirs.iter().copied().find(|d| *d != 0).unwrap_or(1);
    for d in &mut dirs {
        if *d == 0 {
            *d = last;
        }
        last = *d;
    }
    // Runs between the turns, as sample index ranges with their direction.
    let mut runs: Vec<(usize, usize, i8)> = Vec::new();
    let mut run_start = 0usize;
    for (k, d) in dirs.iter().enumerate() {
        let next = dirs.get(k + 1).copied();
        if next.is_none_or(|n| n != *d) {
            let at = vertices.get(k + 1).map_or(ys.len() - 1, |v| v.0);
            runs.push((run_start, at, *d));
            run_start = at;
        }
    }

    let mut out = samples;
    for (a, b, dir) in runs {
        let dir = f64::from(dir);
        let (Some(run), Some(lo_run), Some(hi_run)) = (ys.get(a..=b), lo.get(a..=b), hi.get(a..=b))
        else {
            continue;
        };
        // For a falling run, fit the negated targets as a rising one.
        let y: Vec<f64> = run.iter().map(|v| v * dir).collect();
        let mut from_start = Vec::with_capacity(y.len());
        let mut m = f64::NEG_INFINITY;
        for v in &y {
            m = m.max(*v);
            from_start.push(m);
        }
        let mut to_end = vec![0.0; y.len()];
        let mut m = f64::INFINITY;
        for (slot, v) in to_end.iter_mut().zip(&y).rev() {
            m = m.min(*v);
            *slot = m;
        }
        for (i, ((up, down), (l, h))) in from_start
            .iter()
            .zip(&to_end)
            .zip(lo_run.iter().zip(hi_run))
            .enumerate()
        {
            if let Some(slot) = out.get_mut(a + i) {
                slot.1 = (dir * 0.5 * (up + down)).clamp(*l, *h);
            }
        }
    }
    out
}

/// The level at which a taut string with a free end leaves that end: flat
/// across the gates from the end until they no longer overlap, then turning
/// on the limit that stops it. `fallback` (clamped) when every gate overlaps.
fn free_level<'a>(gates: impl Iterator<Item = (&'a f64, &'a f64)>, fallback: f64) -> f64 {
    let (mut low, mut high) = (f64::NEG_INFINITY, f64::INFINITY);
    for (l, h) in gates {
        if *l > high {
            // The band rises away: the string turns up under the upper limit.
            return high;
        }
        if *h < low {
            // The band falls away: the string turns down over the lower limit.
            return low;
        }
        low = low.max(*l);
        high = high.min(*h);
    }
    fallback.clamp(low.min(high), high.max(low))
}

/// The shortest path from `(x[0], start)` to `(x[n-1], end)` that passes
/// every gate `x[i]` between `lo[i]` and `hi[i]`, as its bend vertices with
/// its ends. `x` increases strictly. The string is straight wherever the
/// gates allow and bends only on a gate's limit: it has the fewest changes
/// of direction any path through the gates can have.
pub fn taut_string(x: &[f64], lo: &[f64], hi: &[f64], start: f64, end: f64) -> Vec<(f64, f64)> {
    taut_vertices(x, lo, hi, start, end)
        .into_iter()
        .filter_map(|(i, y)| x.get(i).map(|xi| (*xi, y)))
        .collect()
}

/// [`taut_string`] with each vertex given by its gate's index.
fn taut_vertices(x: &[f64], lo: &[f64], hi: &[f64], start: f64, end: f64) -> Vec<(usize, f64)> {
    let n = x.len().min(lo.len()).min(hi.len());
    let (Some(&x_last), true) = (n.checked_sub(1).and_then(|k| x.get(k)), n > 0) else {
        return Vec::new();
    };
    let mut out = vec![(0usize, start)];
    if n == 1 {
        return out;
    }
    let gate = |j: usize| -> (f64, f64, f64) {
        let xj = x.get(j).copied().unwrap_or(x_last);
        if j + 1 >= n {
            (xj, end, end)
        } else {
            (
                xj,
                lo.get(j).copied().unwrap_or(end),
                hi.get(j).copied().unwrap_or(end),
            )
        }
    };
    let (mut apex, mut ya) = (0usize, start);
    loop {
        let xa = x.get(apex).copied().unwrap_or(x_last);
        let (mut smax, mut kmax) = (f64::INFINITY, apex);
        let (mut smin, mut kmin) = (f64::NEG_INFINITY, apex);
        let mut bend: Option<(usize, f64)> = None;
        for j in apex + 1..n {
            let (xj, l, h) = gate(j);
            let dx = xj - xa;
            if dx <= 0.0 {
                continue;
            }
            let (sl, su) = ((l - ya) / dx, (h - ya) / dx);
            if sl > smax {
                // The string must climb more steeply than the upper limit at
                // `kmax` allows from here: it bends up under that limit.
                bend = Some((kmax, gate(kmax).2));
                break;
            }
            if su < smin {
                // It must fall more steeply: it bends down over the lower limit.
                bend = Some((kmin, gate(kmin).1));
                break;
            }
            if su < smax {
                smax = su;
                kmax = j;
            }
            if sl > smin {
                smin = sl;
                kmin = j;
            }
        }
        match bend {
            Some((k, y)) if k > apex => {
                out.push((k, y));
                apex = k;
                ya = y;
                if k + 1 >= n {
                    return out;
                }
            }
            _ => {
                out.push((n - 1, end));
                return out;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]
    use super::*;

    fn eval(v: &[(f64, f64)], x: f64) -> f64 {
        let i = v.partition_point(|p| p.0 < x).clamp(1, v.len() - 1);
        let (a, b) = (v[i - 1], v[i]);
        a.1 + (x - a.0) / (b.0 - a.0) * (b.1 - a.1)
    }

    #[test]
    fn req_set_09_the_tracks_start_and_end_outside_the_held_bands() {
        // A band over each end: the range starts and ends at the bands' inner
        // edges; a band inside the range or beyond it changes nothing.
        assert_eq!(
            outside_held(-8.0, 42.0, &[(-20.0, -5.0), (38.0, 50.0)]),
            (-5.0, 38.0)
        );
        assert_eq!(outside_held(0.0, 30.0, &[]), (0.0, 30.0));
        assert_eq!(
            outside_held(0.0, 30.0, &[(10.0, 12.0), (40.0, 50.0)]),
            (0.0, 30.0)
        );
        // Overlapping bands at one end are both passed.
        assert_eq!(
            outside_held(0.0, 30.0, &[(2.0, 4.0), (-1.0, 3.0)]),
            (4.0, 30.0)
        );
    }

    #[test]
    fn req_op_15_the_taut_string_is_straight_where_the_band_allows() {
        let x: Vec<f64> = (0..=10).map(f64::from).collect();
        let lo = vec![-1.0; 11];
        let hi = vec![1.0; 11];
        let s = taut_string(&x, &lo, &hi, 0.5, -0.5);
        assert_eq!(s, vec![(0.0, 0.5), (10.0, -0.5)]);
    }

    #[test]
    fn req_op_15_the_taut_string_stays_in_the_band_and_removes_jitter() {
        // A ramp with jitter of +/- 0.3 inside a band of +/- 0.5: the string
        // has no reversals and stays inside every gate.
        let x: Vec<f64> = (0..=200).map(|i| f64::from(i) * 0.1).collect();
        let target: Vec<f64> = x
            .iter()
            .enumerate()
            .map(|(i, x)| 2.0 * x + if i % 2 == 0 { 0.3 } else { -0.3 })
            .collect();
        let lo: Vec<f64> = target.iter().map(|t| t - 0.5).collect();
        let hi: Vec<f64> = target.iter().map(|t| t + 0.5).collect();
        let s = taut_string(&x, &lo, &hi, target[0], target[200]);
        for (i, xi) in x.iter().enumerate() {
            let y = eval(&s, *xi);
            assert!(y >= lo[i] - 1e-9 && y <= hi[i] + 1e-9, "{i}: {y}");
        }
        let ys: Vec<Degrees> = x.iter().map(|xi| Degrees(eval(&s, *xi))).collect();
        assert_eq!(crate::rotary::reversals(&ys, 0.0), 0);
        let raw: Vec<Degrees> = target.iter().map(|t| Degrees(*t)).collect();
        assert!(crate::rotary::reversals(&raw, 0.0) > 100);
    }

    #[test]
    fn req_op_06_smoothing_keeps_a_monotone_target_exactly() {
        // A straight ramp needs no turn: the angles are the targets, not the
        // shortest path across the band.
        let samples: Vec<(f64, f64)> = (0..=100)
            .map(|i| (f64::from(i) * 0.01, 3.0 * f64::from(i)))
            .collect();
        assert_eq!(smooth_band(samples.clone(), 10.0), samples);
    }

    #[test]
    fn req_op_06_req_op_15_smoothing_removes_small_reversals_with_least_deviation() {
        // A rising target with a dip of 4 inside a band of 3: no reversal is
        // left, every angle stays in the band, and away from the dip the
        // angles are the targets.
        let samples: Vec<(f64, f64)> = (0..=200)
            .map(|i| {
                let x = f64::from(i);
                let dip = if (90..110).contains(&i) { -4.0 } else { 0.0 };
                (x, 0.5 * x + dip)
            })
            .collect();
        let out = smooth_band(samples.clone(), 3.0);
        let ys: Vec<Degrees> = out.iter().map(|p| Degrees(p.1)).collect();
        assert_eq!(crate::rotary::reversals(&ys, 0.0), 0);
        let mut worst = 0.0_f64;
        for (o, s) in out.iter().zip(&samples) {
            worst = worst.max((o.1 - s.1).abs());
            assert!((o.1 - s.1).abs() <= 3.0 + 1e-9);
        }
        assert!(worst > 1.0 && worst <= 2.0 + 1e-9, "{worst}");
        assert_eq!(out[10], samples[10]);
        assert_eq!(out[190], samples[190]);
        // A real reversal (a hump of 20 against a band of 3) stays, turning
        // on the hump's own top.
        let hump: Vec<(f64, f64)> = (0..=200)
            .map(|i| {
                let x = f64::from(i);
                (x, 20.0 - (x - 100.0).abs() * 0.2)
            })
            .collect();
        let out = smooth_band(hump.clone(), 3.0);
        let ys: Vec<Degrees> = out.iter().map(|p| Degrees(p.1)).collect();
        assert_eq!(crate::rotary::reversals(&ys, 0.0), 1);
        assert!((out[100].1 - 20.0).abs() < 1e-9);
    }

    #[test]
    fn req_op_15_the_taut_string_bends_round_a_sine() {
        let x: Vec<f64> = (0..=400).map(|i| f64::from(i) * 0.05).collect();
        let target: Vec<f64> = x.iter().map(|x| 3.0 * x.sin()).collect();
        let lo: Vec<f64> = target.iter().map(|t| t - 0.4).collect();
        let hi: Vec<f64> = target.iter().map(|t| t + 0.4).collect();
        let s = taut_string(&x, &lo, &hi, target[0], target[400]);
        for (i, xi) in x.iter().enumerate() {
            let y = eval(&s, *xi);
            assert!(y >= lo[i] - 1e-9 && y <= hi[i] + 1e-9, "{i}: {y}");
        }
        let ys: Vec<Degrees> = x.iter().map(|xi| Degrees(eval(&s, *xi))).collect();
        // Over 0 to 20 the sine turns six times (at pi/2, 3 pi/2, ... 11 pi/2);
        // the band is too narrow to skip any, and the string adds none.
        assert_eq!(crate::rotary::reversals(&ys, 0.0), 6);
    }
}
