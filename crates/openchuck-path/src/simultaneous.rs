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
//! tool, at the middle of the move, is within [`CHORD_FACTOR`] times the
//! tolerance of the surface, measured by dropping the tool along its axis at
//! that machine pose (positive: the move would cut in; negative: it would
//! leave material) and taken along the surface normal. A move that still
//! cuts in by more than the tolerance after [`MAX_DEPTH`] halvings is
//! refused with a message naming it.

use openchuck_types::geometry::{Point3, UnitVec3, Vec3};
use openchuck_types::job::JobContext;
use openchuck_types::tool::profile::RevolvedProfile;
use openchuck_types::units::Degrees;

use crate::cutter::{self, Contact, ToolLine};
use crate::error::{PathError, Result};
use crate::parallel;
use crate::params::{ParameterKind, ParameterSchema, ParameterSpec, Parameters, Unit, names};
use crate::region::RegionConstraint;
use crate::rotary::{AxisPosition, TURN};
use crate::strategy::{CutPoint, Pass, Strategy, StrategyInput, StrategyOutput};

pub use crate::helical::{CHORD_FACTOR, CONSTRAINED_SPACING};

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

/// How many times a move may be halved.
pub const MAX_DEPTH: u32 = 12;

/// How many times the limit of an allowed region is bisected.
const EDGE_STEPS: u32 = 12;

/// Most drive samples one run may make (NFR-07).
const MAX_SAMPLES: f64 = 5.0e6;

/// Largest lead and largest tilt, degrees: their sum stays well short of a
/// right angle, so the tool never faces away from the surface.
const MAX_LEAD: f64 = 30.0;
const MAX_TILT: f64 = 30.0;

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
    fn name(&self) -> &str {
        NAME
    }

    fn label(&self) -> &str {
        "Simultaneous 4-axis finishing"
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
        ParameterSchema::with_common(vec![
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
}

struct Plan<'a> {
    input: &'a StrategyInput<'a>,
    region: Option<&'a RegionConstraint<'a>>,
    constrained: bool,
    drop_profile: &'a RevolvedProfile,
    convex: bool,
    leave: f64,
    tolerance: f64,
    chord: f64,
    /// Radius from which drive rays start.
    outside: f64,
    sense: f64,
}

/// What a run found worth telling the user.
#[derive(Debug, Clone, Copy, Default)]
struct Findings {
    slope: f64,
    slope_at: Option<Point3>,
    tilt: f64,
    /// The most material any kept move leaves beyond the tolerance, mm.
    under: f64,
    mesh: bool,
}

/// The points a move is refined into, and the most material it leaves.
#[derive(Default)]
struct Refined {
    points: Vec<Option<Node>>,
    under: f64,
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
                    let (a, b) = if k % 2 == 0 { (x0, x1) } else { (x1, x0) };
                    Track {
                        x0: a,
                        x1: b,
                        th0: th,
                        th1: th,
                        seeds,
                        samples,
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
            vec![Track {
                x0,
                x1,
                th0: th_start,
                th1: th_start - sense * rotation * total,
                seeds,
                samples: samples as usize,
            }]
        }
    };

    let grown = cutter::grow_profile(input.tool, leave)?;
    let plan = Plan {
        input,
        region,
        constrained,
        drop_profile: &grown,
        convex: grown.is_convex(),
        leave,
        tolerance,
        chord: CHORD_FACTOR * tolerance,
        outside: radius + 1.0,
        sense,
    };

    let mut findings = Findings::default();
    let mut passes = Vec::new();
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
        passes.extend(plan.track_passes(track, &axis, lead, &mut findings, context)?);
    }
    context.report(
        "simultaneous finishing: following the surface",
        count,
        Some(count),
    );

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
    Ok(StrategyOutput { passes, notes })
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
    fn drive(&self, x: f64, th: f64) -> Option<Drive> {
        let hit = self.ray(x, th)?;
        let exact = hit.position.is_some();
        let normal = if exact {
            hit.normal
        } else {
            self.smoothed_normal(x, th, hit.point, hit.normal)
        };
        let phi = polar(normal.get())?;
        Some(Drive {
            point: hit.point,
            normal,
            phi,
            exact,
        })
    }

    fn ray(&self, x: f64, th: f64) -> Option<crate::surface::RayHit> {
        let (s, c) = th.to_radians().sin_cos();
        let origin = Point3::new(x, self.outside * c, self.outside * s);
        let direction = UnitVec3::new(Vec3::new(0.0, -c, -s)).ok()?;
        self.input.surface.cast_ray(origin, direction, self.outside)
    }

    /// The normal of the surface through four neighbouring drive points
    /// [`NORMAL_SPAN`] away, which averages out the facet normals of a mesh
    /// (OI-67). The facet normal where a neighbour is missing.
    fn smoothed_normal(&self, x: f64, th: f64, p: Point3, facet: UnitVec3) -> UnitVec3 {
        let r = p.y.hypot(p.z);
        if r <= NORMAL_SPAN {
            return facet;
        }
        let dth = (NORMAL_SPAN / r).to_degrees();
        let pt = |x: f64, th: f64| self.ray(x, th).map(|h| h.point);
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
            self.drive(x, th)
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
        let (x, th) = track.at(t);
        let drive = self.drive(x, th)?;
        // The plan's angle is continuous along the track; keep it so, and
        // take the normal's angle as its equivalent nearest it.
        let phi = axis.phi_at(t)?;
        let a = Degrees(self.sense * (90.0 - phi));
        let axis_dir = self.input.rotary.tool_axis(a);
        let line = ToolLine {
            origin: self.seed_tip(drive.point, drive.normal, axis_dir),
            axis: axis_dir,
        };
        let contact = self.input.surface.drop_tool(self.drop_profile, &line)?;
        if let Some(region) = self.region
            && !region.allows(contact.contact, contact.tip, self.drop_profile, &line)
        {
            return None;
        }
        Some(Node {
            t,
            a,
            contact,
            normal: drive.normal,
            phi_n: near(drive.phi, phi),
            phi,
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
        p + (-(t * q.height.0 + e * q.radius.0))
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

    /// The error of the straight machine move from `a` to `b` at its
    /// middle: the tool is dropped along its axis at that machine pose, and
    /// the distance it moves is taken along the surface normal. Positive
    /// when the move would cut into the surface, negative when it would
    /// leave material; zero when there is no surface under it.
    fn deviation(&self, a: &Node, b: &Node, normal: UnitVec3) -> f64 {
        let rotary = &self.input.rotary;
        let (pa, pb) = (
            rotary.to_axis(a.contact.tip, a.a),
            rotary.to_axis(b.contact.tip, b.a),
        );
        let mid = AxisPosition {
            along: 0.5 * (pa.along + pb.along),
            across: 0.5 * (pa.across + pb.across),
            height: 0.5 * (pa.height + pb.height),
        };
        let a_mid = Degrees(0.5 * (a.a.0 + crate::rotary::nearest_turn(b.a, a.a).0));
        let line = ToolLine {
            origin: rotary.to_model(mid, a_mid),
            axis: rotary.tool_axis(a_mid),
        };
        match self.input.surface.drop_tool(self.drop_profile, &line) {
            Some(c) => c.offset * line.axis.get().dot(normal.get()).max(0.0),
            None => 0.0,
        }
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
    /// middle; pushes the points after `a`, ending with `b`. `None` marks a
    /// gap where the region forbids cutting.
    fn refine(
        &self,
        track: &Track,
        axis: &AxisPlan,
        a: &Node,
        b: &Node,
        depth: u32,
        out: &mut Refined,
    ) -> Result<()> {
        let t = 0.5 * (a.t + b.t);
        match self.node(track, axis, t) {
            Some(mid) => {
                let error = self.deviation(a, b, mid.normal);
                if error.abs() > self.chord {
                    if depth < MAX_DEPTH {
                        self.refine(track, axis, a, &mid, depth + 1, out)?;
                        return self.refine(track, axis, &mid, b, depth + 1, out);
                    }
                    if error > self.tolerance {
                        return Err(PathError::Strategy(format!(
                            "simultaneous finishing cannot keep the move to X {:.3}, A {:.3} \
                             within the tolerance: it would cut {error:.4} mm into the surface; \
                             reduce the largest tilt or the largest angle step",
                            b.contact.tip.x, b.a.0
                        )));
                    }
                    if -error > self.tolerance {
                        out.under = out.under.max(-error);
                    }
                }
            }
            None if self.constrained && depth < MAX_DEPTH => {
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
            None => {}
        }
        out.points.push(Some(*b));
        Ok(())
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

        let mut pieces: Vec<Vec<Node>> = Vec::new();
        let mut current: Vec<Node> = Vec::new();
        for (i, node) in nodes.iter().enumerate() {
            match node {
                Some(n) => {
                    if current.is_empty()
                        && self.constrained
                        && let Some(previous) = i.checked_sub(1).and_then(|p| ts.get(p))
                        && let Some(edge) = self.edge(track, axis, n, *previous)
                    {
                        current.push(edge);
                    }
                    current.push(*n);
                }
                None => {
                    if let Some(last) = current.last().copied() {
                        if self.constrained
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

        let mut passes = Vec::new();
        for piece in &pieces {
            context.check_cancelled()?;
            let intervals: Vec<(&Node, &Node)> = piece.iter().zip(piece.iter().skip(1)).collect();
            let refined: Vec<Result<Refined>> = parallel::map_in_order(&intervals, |(a, b)| {
                let mut out = Refined::default();
                self.refine(track, axis, a, b, 0, &mut out).map(|()| out)
            });
            let mut points = Vec::new();
            let offset = lead * track.travel();
            let mut under = 0.0_f64;
            let mut push = |n: &Node, points: &mut Vec<CutPoint>| {
                self.note(n, offset, findings);
                points.push(self.cut_point(n));
            };
            if let Some(first) = piece.first() {
                push(first, &mut points);
            }
            for steps in refined {
                let steps = steps?;
                under = under.max(steps.under);
                for step in steps.points {
                    match step {
                        Some(n) => push(&n, &mut points),
                        None => {
                            if !points.is_empty() {
                                passes.push(Pass {
                                    points: std::mem::take(&mut points),
                                    stock: None,
                                });
                            }
                        }
                    }
                }
            }
            findings.under = findings.under.max(under);
            if !points.is_empty() {
                passes.push(Pass {
                    points,
                    stock: None,
                });
            }
        }
        Ok(passes)
    }

    fn note(&self, n: &Node, offset: f64, findings: &mut Findings) {
        let slope = n.normal.get().x.abs().clamp(0.0, 1.0).asin().to_degrees();
        if slope > findings.slope {
            findings.slope = slope;
            findings.slope_at = Some(n.contact.contact);
        }
        findings.tilt = findings.tilt.max((n.phi - n.phi_n - offset).abs());
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
    let xs: Vec<f64> = samples.iter().map(|s| s.0).collect();
    let ys: Vec<f64> = samples.iter().map(|s| s.1).collect();
    let lo: Vec<f64> = ys.iter().map(|y| y - width).collect();
    let hi: Vec<f64> = ys.iter().map(|y| y + width).collect();
    let start = free_level(lo.iter().zip(&hi), ys.first().copied().unwrap_or(0.0));
    let end = free_level(lo.iter().zip(&hi).rev(), ys.last().copied().unwrap_or(0.0));
    let vertices = taut_vertices(&xs, &lo, &hi, start, end);

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
