// SPDX-License-Identifier: AGPL-3.0-or-later
//! Indexed finishing: passes along the rotary axis at fixed A positions, the
//! 3+1 positional form (OP-04; work breakdown slice 12 item 3; decision record
//! 0034 item 3).
//!
//! One pass along X is cut at each requested A, in the order given. The angle
//! goes to the IR as given (the shared services only add whole turns to keep A
//! continuous), and the move from one angle to the next is a positioning move
//! through the clearance geometry, never a feed move across the stock
//! (OP-14). The angles are a list (`angles`), or `count` equally spaced
//! positions from `start_angle` when the list is empty.

use openchuck_types::job::JobContext;

use openchuck_types::geometry::Point3;
use openchuck_types::units::Degrees;

use crate::adaptive::{self, Counting, Family, Interval, Pose, Track};
use crate::axial::{AlongX, parameter as axial_parameter};
use crate::error::{PathError, Result};
use crate::params::{
    MAX_ANGLES, ParameterKind, ParameterSchema, ParameterSpec, Parameters, Unit, names,
};
use crate::region::RegionConstraint;
use crate::rotary::{AxisPosition, RotaryMapping};
use crate::strategy::{LeaveZone, Pass, Strategy, StrategyInput, StrategyOutput};

/// Indexed finishing of the whole model, or of a region.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct IndexedFinishing;

/// The registry name of [`IndexedFinishing`].
pub const NAME: &str = "indexed_finishing";

/// Parameter names specific to indexed finishing.
pub mod parameter {
    /// The A positions, degrees, separated by commas.
    pub const ANGLES: &str = "angles";
    /// How many equally spaced positions, when `angles` is empty.
    pub const COUNT: &str = "count";
    /// A of the first equally spaced position, degrees.
    pub const START_ANGLE: &str = "start_angle";
}

impl Strategy for IndexedFinishing {
    /// Exact contacts on a STEP part: measured within the limits of OI-835
    /// (decision 0277).
    fn exact_contact(&self) -> bool {
        true
    }

    fn name(&self) -> &str {
        NAME
    }

    fn label(&self) -> &str {
        "Indexed finishing"
    }

    fn description(&self) -> &str {
        "Turns the part to each chosen angle, holds it still and follows the surface along its length. Use it for features reached from a few set directions. It does not turn the part during a cutting pass."
    }

    fn schema(&self) -> ParameterSchema {
        let mut schema = ParameterSchema::with_common(vec![
            ParameterSpec {
                name: parameter::ANGLES.to_owned(),
                label: "Angles".to_owned(),
                description: "The A positions to cut at, in degrees, in the order to cut \
                              them, separated by commas. Empty to use the count instead."
                    .to_owned(),
                kind: ParameterKind::AngleList {
                    default: String::new(),
                },
            },
            ParameterSpec {
                name: parameter::COUNT.to_owned(),
                label: "Count".to_owned(),
                description: "How many equally spaced positions to cut at when no angles \
                              are listed."
                    .to_owned(),
                kind: ParameterKind::Integer {
                    min: 1,
                    max: MAX_ANGLES as i64,
                    default: Some(4),
                },
            },
            ParameterSpec {
                name: parameter::START_ANGLE.to_owned(),
                label: "Start angle".to_owned(),
                description: "A of the first equally spaced position.".to_owned(),
                kind: ParameterKind::Number {
                    unit: Unit::Degrees,
                    min: None,
                    min_exclusive: false,
                    max: None,
                    default: Some(0.0),
                },
            },
        ]);
        // The positions, not a stepover, set where the passes are; the common
        // stepover is not used, so it gets a default.
        if let Some(spec) = schema
            .parameters
            .iter_mut()
            .find(|p| p.name == names::STEPOVER)
        {
            spec.description = "Not used: the angles set where the passes are.".to_owned();
            if let ParameterKind::Number { default, .. } = &mut spec.kind {
                *default = Some(1.0);
            }
        }
        let own = super::axial::AxialFinishing.schema();
        for name in [
            axial_parameter::DIRECTION,
            axial_parameter::LINK_HEIGHT,
            axial_parameter::MAX_X_STEP,
            crate::extend::parameter::EXTEND,
            crate::extend::parameter::EXTEND_DISTANCE,
            crate::finishing::parameter::FINISH_ALLOWANCE,
            crate::finishing::parameter::FINISHING_LAYERS,
            crate::finishing::parameter::STEPDOWN,
            axial_parameter::SCALLOP,
            adaptive::parameter::ADAPTIVE_SPACING,
            adaptive::parameter::MIN_SPACING,
        ] {
            if let Some(spec) = own.get(name) {
                schema.parameters.push(spec.clone());
            }
        }
        schema
    }

    fn schema_for(
        &self,
        values: &std::collections::BTreeMap<String, crate::params::ParameterValue>,
    ) -> ParameterSchema {
        let mut schema = adaptive::schema_for(
            crate::finishing::schema_for(self.schema(), values),
            values,
            axial_parameter::SCALLOP,
        );
        // The height is the Allowed excess only: indexed finishing has no
        // stepover for a scallop to set (decision record 0239).
        if !adaptive::is_on(values) {
            schema
                .parameters
                .retain(|p| p.name != axial_parameter::SCALLOP);
        }
        schema
    }

    fn honours_region(&self) -> bool {
        true
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
        output(passes(input, region, zones, parameters, context)?)
    }

    fn passes(
        &self,
        input: &StrategyInput<'_>,
        parameters: &Parameters,
        context: &JobContext<'_>,
    ) -> Result<Vec<Pass>> {
        passes(input, None, &[], parameters, context).map(|m| m.passes)
    }

    fn passes_in_region(
        &self,
        input: &StrategyInput<'_>,
        region: &RegionConstraint<'_>,
        parameters: &Parameters,
        context: &JobContext<'_>,
    ) -> Result<Vec<Pass>> {
        passes(input, Some(region), &[], parameters, context).map(|m| m.passes)
    }

    fn passes_with_notes(
        &self,
        input: &StrategyInput<'_>,
        region: Option<&RegionConstraint<'_>>,
        parameters: &Parameters,
        context: &JobContext<'_>,
    ) -> Result<StrategyOutput> {
        output(passes(input, region, &[], parameters, context)?)
    }
}

/// The passes made, with their notes.
fn output(made: Made) -> Result<StrategyOutput> {
    let (x0, x1) = made.range;
    let mut warnings = Vec::new();
    let mut spaced = None;
    if let Some((settings, outcome)) = &made.spaced {
        spaced = Some(outcome.note(settings));
        warnings.extend(outcome.warning(
            settings,
            &adaptive::Limits::OWNER,
            axial_parameter::SCALLOP,
        ));
    }
    Ok(StrategyOutput {
        notes: beyond_note(&made.passes, x0, x1)
            .into_iter()
            .chain(spaced)
            .chain(made.layered)
            .collect(),
        passes: made.passes,
        warnings,
        unreached: Vec::new(),
        shank_breaks: Vec::new(),
    })
}

/// The passes of indexed finishing, the target's extent along X, the note
/// on its finishing layers and what adaptive spacing did.
struct Made {
    passes: Vec<Pass>,
    range: (f64, f64),
    layered: Option<String>,
    spaced: Option<(adaptive::Settings, adaptive::Outcome)>,
}

/// The note an indexed operation carries when its passes cut past the ends
/// of the part (`x0` to `x1`), where the tool's side faces an end face the
/// target stops short of (decision record 0343). `None` when they did not.
fn beyond_note(passes: &[Pass], x0: f64, x1: f64) -> Option<String> {
    let points = || passes.iter().flat_map(|p| p.points.iter());
    let start = points().any(|c| c.tip.x < x0);
    let end = points().any(|c| c.tip.x > x1);
    let ends = match (start, end) {
        (true, true) => "past both ends of the part",
        (true, false) => "past the start of the part",
        (false, true) => "past the far end of the part",
        (false, false) => return None,
    };
    Some(format!(
        "the passes run on {ends} by up to the tool's radius, so its side faces each end face \
         where the target stops short of it, along each position only: between the positions \
         the end face keeps what earlier operations left"
    ))
}

/// The positions for `parameters`, in the order to cut.
fn positions(parameters: &Parameters) -> Result<Vec<f64>> {
    let listed = parameters.angles(parameter::ANGLES)?;
    if !listed.is_empty() {
        return Ok(listed);
    }
    let count = parameters.integer(parameter::COUNT)?;
    let start = parameters.number(parameter::START_ANGLE)?;
    if count < 1 {
        return Err(PathError::parameter(
            parameter::COUNT,
            "at least one position",
        ));
    }
    let step = 360.0 / count as f64;
    Ok((0..count).map(|i| start + step * i as f64).collect())
}

fn passes(
    input: &StrategyInput<'_>,
    region: Option<&RegionConstraint<'_>>,
    zones: &[LeaveZone<'_>],
    parameters: &Parameters,
    context: &JobContext<'_>,
) -> Result<Made> {
    let angles = positions(parameters)?;
    let mut along = AlongX::read(input, region, zones, parameters)?;
    // Each pass runs on past the ends of the target by the tool's radius
    // (grown by the stock to leave), less a tenth of the tolerance so its
    // first point still stands on the target, so where the target stands
    // short of the tool's radius past an end face (a short tab) the tool goes
    // down beside the face onto it and the fit finds the face between that
    // point and the next. Past a free end the tool only rides the end's edge
    // (decision record 0343). A stays fixed at each position.
    let tolerance = parameters.number(names::TOLERANCE)?;
    along.beyond = along.grown_radius() - 0.1 * tolerance;
    along.check_passes(angles.len())?;
    let adaptive = adaptive::Settings::read(parameters, axial_parameter::SCALLOP)?;
    let (passes, spaced) = match adaptive {
        None => (
            along.passes_at(&angles, "indexed finishing", context)?,
            None,
        ),
        Some(settings) => {
            let groups = along.passes_by_angle(&angles, "indexed finishing", context)?;
            let (passes, outcome) = adaptive_passes(
                input,
                (region, zones, parameters),
                &along,
                (&angles, groups),
                settings,
                context,
            )?;
            (passes, Some((settings, outcome)))
        }
    };
    // Finishing layers above them, when on; each layer keeps the pass's A
    // (decision record 0345).
    let (passes, layered) = crate::finishing::layers(input, passes, parameters)?;
    Ok(Made {
        passes,
        range: along.range(),
        layered,
        spaced,
    })
}

/// How far from a position, in degrees round the part, a vertical tool held
/// at that position's A is taken to reach: past it the surface faces too
/// far from the tool (decision record 0239, the owner's choice on indexed
/// finishing).
const REACH: f64 = 75.0;

/// Indexed finishing's positions as a family for adaptive spacing: across
/// it, the angle round the part in degrees, the positions sorted by angle;
/// along it, model X. A pass at an angle between two positions is cut at the
/// nearer position's A, offset across the axis towards the other, so A is
/// never one the user did not list (decision record 0239 item 3).
struct Positions<'a> {
    along: &'a AlongX<'a>,
    counters: &'a [Counting<'a>],
    rotary: RotaryMapping,
    /// The positions in angle order, each in [first, first + 360), then the
    /// first again a turn on.
    sorted: Vec<f64>,
    /// For each gap `k`, the side across the axis (+1 or -1) a tool at
    /// `sorted[k]` stands towards `sorted[k + 1]`, and the side a tool at
    /// `sorted[k + 1]` stands towards `sorted[k]`.
    sides: Vec<(f64, f64)>,
    pose_step: f64,
    window: f64,
    station: f64,
    margin: f64,
}

impl Positions<'_> {
    /// Where a pass at angle `c` is cut: the index of the position in
    /// `sorted` whose A it takes, the offset across the axis, mm, and the
    /// angle from that position, degrees.
    fn locate(&self, c: f64) -> Option<(usize, f64, f64)> {
        let k = self
            .sorted
            .windows(2)
            .position(|w| matches!(w, [p, q] if c >= *p && c <= *q))?;
        let (p, q) = (*self.sorted.get(k)?, *self.sorted.get(k + 1)?);
        let (to_next, to_previous) = *self.sides.get(k)?;
        // A vertical tool's centre stands the tool's radius off the part, so
        // the offset that puts its contact `phi` round from the position is
        // taken on the circle through its centre (exact for a round part).
        let r = self.along.radius + self.along.grown_radius();
        let (phi, g) = (c - p, q - p);
        if phi <= 0.5 * g {
            Some((k, to_next * r * phi.to_radians().sin(), phi))
        } else {
            let at = if k + 1 == self.sorted.len() - 1 {
                0
            } else {
                k + 1
            };
            Some((at, to_previous * r * (g - phi).to_radians().sin(), g - phi))
        }
    }

    fn a_of(&self, index: usize) -> Degrees {
        Degrees(self.sorted.get(index).copied().unwrap_or(0.0))
    }
}

impl Family for Positions<'_> {
    fn pose(&self, c: f64, s: f64) -> (f64, Degrees) {
        (s, Degrees(c))
    }

    fn poses(&self, _c: f64, pass: &Pass) -> Vec<Pose> {
        adaptive::machine_poses(&self.rotary, pass, self.pose_step, |tip, _| tip.x)
    }

    fn probe(&self, x: f64, c: Degrees) -> Option<(Point3, f64)> {
        let (at, across, _) = self.locate(c.0)?;
        self.along.probe_across(x, self.a_of(at), across)
    }

    fn fit(&self, c: f64, span: (f64, f64), context: &JobContext<'_>) -> Result<Vec<Pass>> {
        let Some((at, across, _)) = self.locate(c) else {
            return Ok(Vec::new());
        };
        self.along
            .pass_over_across(self.a_of(at), across, span, context)
    }

    fn width(&self, dc: f64) -> f64 {
        self.along.radius * dc.to_radians().abs()
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

    fn reachable(&self, c: f64) -> bool {
        self.locate(c).is_some_and(|(_, _, off)| off <= REACH)
    }
}

/// The side across the rotary axis (+1 or -1) a vertical tool at A `from`
/// stands on to face the part towards the angle a tool at A `toward` faces,
/// at radius `r`.
fn side(rotary: &RotaryMapping, from: f64, toward: f64, r: f64) -> f64 {
    let at = |a: f64, across: f64| {
        rotary.to_model(
            AxisPosition {
                along: 0.0,
                across,
                height: r,
            },
            Degrees(a),
        )
    };
    let polar = |p: Point3| p.z.atan2(p.y);
    let turn = |from: f64, to: f64| {
        (to - from + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI
    };
    let base = polar(at(from, 0.0));
    let moved = turn(base, polar(at(from, 0.01 * r)));
    let wanted = turn(base, polar(at(toward, 0.0)));
    if moved * wanted >= 0.0 { 1.0 } else { -1.0 }
}

/// The passes at `angles` (in the order listed, each `groups` entry the
/// passes at one), with passes added beside the positions where more than
/// the Allowed excess is left between neighbouring positions (decision
/// record 0239): each added pass follows the passes of the position whose A
/// it keeps.
fn adaptive_passes(
    input: &StrategyInput<'_>,
    (region, zones, parameters): (Option<&RegionConstraint<'_>>, &[LeaveZone<'_>], &Parameters),
    along: &AlongX<'_>,
    (angles, groups): (&[f64], Vec<Vec<Pass>>),
    settings: adaptive::Settings,
    context: &JobContext<'_>,
) -> Result<(Vec<Pass>, adaptive::Outcome)> {
    // The positions in angle order, each listed angle once.
    let first = angles.first().copied().unwrap_or(0.0);
    let mut order: Vec<(f64, usize)> = Vec::new();
    for (i, &a) in angles.iter().enumerate() {
        let norm = first + (a - first).rem_euclid(360.0);
        if !order.iter().any(|&(p, _)| (p - norm).abs() < 1e-9) {
            order.push((norm, i));
        }
    }
    order.sort_by(|p, q| p.0.total_cmp(&q.0));
    let mut sorted: Vec<f64> = order.iter().map(|&(p, _)| p).collect();
    if let Some(&p) = sorted.first() {
        sorted.push(p + 360.0);
    }
    let r = along.radius;
    let sides: Vec<(f64, f64)> = sorted
        .windows(2)
        .map(|w| match w {
            [p, q] => (
                side(&input.rotary, *p, *q, r),
                side(&input.rotary, *q, *p, r),
            ),
            _ => (1.0, -1.0),
        })
        .collect();
    // The measure and the added passes drop through counters, so their
    // geometry queries are limited (decision record 0239 item 8).
    let counters: Vec<Counting<'_>> = std::iter::once(input.surface)
        .chain(zones.iter().map(|z| z.surface))
        .map(Counting::new)
        .collect();
    let Some((head, rest)) = counters.split_first() else {
        return Err(PathError::Strategy("no surface to finish".to_owned()));
    };
    let counted = StrategyInput {
        surface: head,
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
    let measure = AlongX::read(&counted, region, &counted_zones, parameters)?;
    let tool_r = input.tool.cutting_radius();
    let family = Positions {
        along: &measure,
        counters: &counters,
        rotary: input.rotary,
        sorted: sorted.clone(),
        sides,
        pose_step: (0.8 * tool_r * settings.height)
            .sqrt()
            .clamp(0.02, 0.5 * tool_r),
        window: 2.0 * along.grown_radius() + along.x_step(),
        station: along.x_step().min(0.5 * tool_r).max(1e-3),
        margin: along.grown_radius(),
    };
    let n = order.len();
    let mut tracks: Vec<Track> = order
        .iter()
        .map(|&(p, i)| Track::new(&family, p, groups.get(i).cloned().unwrap_or_default()))
        .collect();
    if let (Some(&p), Some(t)) = (sorted.last(), tracks.first()) {
        let passes = t.passes.clone();
        tracks.push(Track::new(&family, p, passes));
    }
    let points: u64 = groups
        .iter()
        .flatten()
        .map(|p| p.points.len() as u64 + adaptive::LINK_POINTS)
        .sum();
    let (x0, x1) = along.range();
    let intervals: Vec<Interval> = (0..n)
        .filter(|&k| matches!((sorted.get(k), sorted.get(k + 1)), (Some(p), Some(q)) if q > p))
        .map(|k| Interval {
            lo: k,
            hi: k + 1,
            spans: vec![(x0, x1)],
            level: 0,
        })
        .collect();
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
    // The added passes by the position whose A they keep, in angle order.
    let mut added: Vec<Vec<(f64, Pass)>> = vec![Vec::new(); n];
    for track in outcome.tracks.iter().skip(outcome.own) {
        if let Some((at, ..)) = family.locate(track.c)
            && let Some(list) = added.get_mut(at)
        {
            list.extend(track.passes.iter().cloned().map(|p| (track.c, p)));
        }
    }
    let mut out = Vec::new();
    for (i, group) in groups.into_iter().enumerate() {
        out.extend(group);
        if let Some(k) = order.iter().position(|&(_, listed)| listed == i)
            && let Some(list) = added.get_mut(k)
        {
            list.sort_by(|p, q| p.0.total_cmp(&q.0));
            out.extend(list.drain(..).map(|(_, p)| p));
        }
    }
    Ok((out, outcome))
}
