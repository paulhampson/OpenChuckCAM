// SPDX-License-Identifier: AGPL-3.0-or-later
//! Edge and curve following: engraving or carving along edge chains projected
//! onto the surface (OP-08; work breakdown slice 16; decision record 0047).
//!
//! **Chains.** A [`CurveSource`] gives open or closed polylines in the model
//! frame. The registered strategy takes the feature edges of the part's
//! tessellation ([`MeshFeatureEdges`]); a host with chains of its own
//! (B-rep edges, a drawing) builds [`EdgeFollowing::with_source`], or calls
//! [`follow_chains`] directly.
//!
//! **Projection and depth.** The tool axis is the radial direction through
//! the cut point (the rotary axis is the model X axis), so A faces the point
//! and follows the curve as it winds round the part, one simultaneous 4-axis
//! move. The tool tip is `depth` below the surface point *along that axis*:
//! the engraved depth is radial, which on a curved surface is constant by
//! construction and does not depend on how the surface slopes. A chain that
//! is not on the surface (`lies_on_surface` false) is first projected onto it
//! along the same radial line.
//!
//! **Line width follows depth.** A tool cuts a groove whose half width at a
//! depth is the radius at which its profile is that high ([`RevolvedProfile::height_at`]).
//! With `line_width` set, the depth is the one at which the tool cuts that
//! width at the surface: for a V-bit `width / 2 / tan(half angle)`.
//!
//! **Engrave and carve.** Engraving makes one line along the chain. Carving
//! cuts a band `carve_width` wide at the surface: lines offset across the
//! chain, `stepover` apart at most, each following the surface at its own
//! offset. Either mode cuts in levels `depth_per_pass` deep.
//!
//! **Direction and start.** `start` sets which end of an open chain, or which
//! point of a closed one, the cut starts at, and for open chains which way it
//! runs. Chains are cut in a short order from the lowest X.

use std::sync::Arc;

use openchuck_types::geometry::{Point3, UnitVec3, Vec3};
use openchuck_types::job::JobContext;
use openchuck_types::tool::profile::RevolvedProfile;
use openchuck_types::units::Degrees;

use crate::curves::{CurveSource, EdgeChain, MeshFeatureEdges};
use crate::cutter;
use crate::error::{PathError, Result};
use crate::parallel;
use crate::params::{
    EntryParameters, EntryStyle, ParameterKind, ParameterSchema, ParameterSpec, Parameters, Unit,
    names,
};
use crate::strategy::{CutPoint, Pass, Strategy, StrategyInput, StrategyOutput};

/// The registry name of [`EdgeFollowing`].
pub const NAME: &str = "edge_following";

/// Parameter names specific to edge following.
pub mod parameter {
    /// `engrave` (one line) or `carve` (a band).
    pub const MODE: &str = "mode";
    /// The depth below the surface, millimetres.
    pub const DEPTH: &str = "depth";
    /// The width of the line at the surface; sets the depth. 0 is off.
    pub const LINE_WIDTH: &str = "line_width";
    /// The depth of one level; 0 cuts all of the depth at once.
    pub const DEPTH_PER_PASS: &str = "depth_per_pass";
    /// The width of the band a carve cuts at the surface.
    pub const CARVE_WIDTH: &str = "carve_width";
    /// Where each chain starts: `first`, `reversed`, `low_x`, `high_x` or
    /// `angle`.
    pub const START: &str = "start";
    /// The angle `start = angle` starts nearest to, degrees about the rotary
    /// axis from model +Y towards +Z.
    pub const START_ANGLE: &str = "start_angle";
    /// The smallest turn that makes a mesh edge a feature edge, degrees.
    pub const CREASE_ANGLE: &str = "crease_angle";
    /// Chains shorter than this are not cut, millimetres.
    pub const MIN_LENGTH: &str = "min_length";
    /// The furthest apart, along the chain, that points are.
    pub const MAX_STEP: &str = "max_step";
}

/// Most points one run may make (NFR-07).
const MAX_POINTS: f64 = 2.0e6;

/// Most levels or offset lines one run may make.
const MAX_LINES: usize = 1000;

/// The closest to the rotary axis a point may be: A cannot face it.
const MIN_RADIUS: f64 = 1e-3;

/// The widest angle between two points of a chain, radians, whatever the
/// tolerance allows.
const MAX_ANGLE: f64 = 5.0 * std::f64::consts::PI / 180.0;

/// Edge and curve following (OP-08).
#[derive(Debug, Clone, Default)]
pub struct EdgeFollowing {
    source: Option<Arc<dyn CurveSource>>,
}

impl EdgeFollowing {
    /// The strategy cutting the chains of `source` in place of the part's
    /// feature edges. Its `crease_angle` and `min_length` parameters then do
    /// nothing.
    pub fn with_source(source: Arc<dyn CurveSource>) -> Self {
        Self {
            source: Some(source),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Start {
    First,
    Reversed,
    LowX,
    HighX,
    Angle,
}

/// The limits of a number parameter.
struct Range {
    min: Option<f64>,
    min_exclusive: bool,
    max: Option<f64>,
}

fn number(
    name: &str,
    label: &str,
    description: &str,
    unit: Unit,
    range: Range,
    default: f64,
) -> ParameterSpec {
    let Range {
        min,
        min_exclusive,
        max,
    } = range;
    ParameterSpec {
        name: name.to_owned(),
        label: label.to_owned(),
        description: description.to_owned(),
        kind: ParameterKind::Number {
            unit,
            min,
            min_exclusive,
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
            default: options
                .first()
                .map_or_else(String::new, |o| (*o).to_owned()),
        },
    }
}

impl Strategy for EdgeFollowing {
    fn name(&self) -> &str {
        NAME
    }

    fn label(&self) -> &str {
        "Edge and curve following"
    }

    fn schema(&self) -> ParameterSchema {
        use parameter::*;
        ParameterSchema::with_common(vec![
            choice(
                MODE,
                "Mode",
                "Engrave cuts one line along each chain; carve cuts a band of the carve width.",
                &["engrave", "carve"],
            ),
            number(
                DEPTH,
                "Depth",
                "How far below the surface the tool tip goes, measured along the tool axis.",
                Unit::Millimetres,
                Range {
                    min: Some(0.0),
                    min_exclusive: true,
                    max: None,
                },
                0.2,
            ),
            number(
                LINE_WIDTH,
                "Line width",
                "The width of the line at the surface. When above zero the depth is the one at \
                 which the tool cuts this width (a V-bit: the width follows the depth).",
                Unit::Millimetres,
                Range {
                    min: Some(0.0),
                    min_exclusive: false,
                    max: None,
                },
                0.0,
            ),
            number(
                DEPTH_PER_PASS,
                "Depth per pass",
                "The depth of each level. Zero cuts the whole depth at once.",
                Unit::Millimetres,
                Range {
                    min: Some(0.0),
                    min_exclusive: false,
                    max: None,
                },
                0.0,
            ),
            number(
                CARVE_WIDTH,
                "Carve width",
                "Carve only: the width of the band cut at the surface, centred on the chain.",
                Unit::Millimetres,
                Range {
                    min: Some(0.0),
                    min_exclusive: false,
                    max: None,
                },
                0.0,
            ),
            choice(
                START,
                "Start",
                "Where each chain starts: as the chain is stored, reversed, at the lowest or \
                 highest X, or nearest the start angle.",
                &["first", "reversed", "low_x", "high_x", "angle"],
            ),
            number(
                START_ANGLE,
                "Start angle",
                "The angle about the rotary axis, from +Y towards +Z, that the start is nearest \
                 to when the start is 'angle'.",
                Unit::Degrees,
                Range {
                    min: Some(-360.0),
                    min_exclusive: false,
                    max: Some(360.0),
                },
                0.0,
            ),
            number(
                CREASE_ANGLE,
                "Crease angle",
                "The smallest turn between two faces that makes a mesh edge one to follow.",
                Unit::Degrees,
                Range {
                    min: Some(0.0),
                    min_exclusive: true,
                    max: Some(180.0),
                },
                30.0,
            ),
            number(
                MIN_LENGTH,
                "Shortest chain",
                "Chains shorter than this are not cut.",
                Unit::Millimetres,
                Range {
                    min: Some(0.0),
                    min_exclusive: false,
                    max: None,
                },
                0.0,
            ),
            number(
                MAX_STEP,
                "Largest step",
                "The furthest apart, along the chain, that points of the path are.",
                Unit::Millimetres,
                Range {
                    min: Some(0.0),
                    min_exclusive: true,
                    max: None,
                },
                0.5,
            ),
        ])
    }

    fn parameter_warnings(&self, _tool: &RevolvedProfile, parameters: &Parameters) -> Vec<String> {
        let mut out = Vec::new();
        let carve = parameters.text(parameter::MODE).is_ok_and(|m| m == "carve");
        let positive = |name: &str| parameters.number(name).is_ok_and(|v| v > 0.0);
        if positive(parameter::LINE_WIDTH) {
            out.push(
                "edge following: the depth is set by the line width, so the depth parameter is \
                 not used"
                    .to_owned(),
            );
        }
        if !carve && positive(parameter::CARVE_WIDTH) {
            out.push("edge following: the carve width is used only in carve mode".to_owned());
        }
        if positive(names::STOCK_TO_LEAVE) {
            out.push(
                "edge following cuts to a depth below the surface, so stock to leave is not used"
                    .to_owned(),
            );
        }
        if self.source.is_some()
            && (positive(parameter::MIN_LENGTH)
                || parameters
                    .number(parameter::CREASE_ANGLE)
                    .is_ok_and(|v| (v - 30.0).abs() > 1e-9))
        {
            out.push(
                "edge following: the chains come from the operation's own edges, so the crease \
                 angle and the shortest chain are not used"
                    .to_owned(),
            );
        }
        out
    }

    fn passes(
        &self,
        input: &StrategyInput<'_>,
        parameters: &Parameters,
        context: &JobContext<'_>,
    ) -> Result<Vec<Pass>> {
        Ok(self
            .passes_with_notes(input, None, parameters, context)?
            .passes)
    }

    fn passes_with_notes(
        &self,
        input: &StrategyInput<'_>,
        region: Option<&crate::region::RegionConstraint<'_>>,
        parameters: &Parameters,
        context: &JobContext<'_>,
    ) -> Result<StrategyOutput> {
        if region.is_some() {
            return Err(PathError::Strategy(format!(
                "the strategy '{NAME}' cannot be limited to a region; select the edges to follow"
            )));
        }
        let mesh_edges;
        let source: &dyn CurveSource = match &self.source {
            Some(source) => source.as_ref(),
            None => {
                mesh_edges = MeshFeatureEdges {
                    crease_degrees: parameters.number(parameter::CREASE_ANGLE)?,
                    min_length: parameters.number(parameter::MIN_LENGTH)?,
                };
                &mesh_edges
            }
        };
        let chains = source.chains(input.surface)?;
        if chains.is_empty() {
            if self.source.is_some() {
                return Err(PathError::Strategy(
                    "edge following has no edge to follow: the operation's own edges gave no chain"
                        .to_owned(),
                ));
            }
            return Err(PathError::Strategy(format!(
                "edge following has no edge to follow: the part has no edge turning {} degrees \
                 or more and at least {} mm long; lower {} or {}",
                parameters.number(parameter::CREASE_ANGLE).unwrap_or(0.0),
                parameters.number(parameter::MIN_LENGTH).unwrap_or(0.0),
                parameter::CREASE_ANGLE,
                parameter::MIN_LENGTH,
            )));
        }
        follow_chains(
            input,
            &chains,
            source.lies_on_surface(),
            parameters,
            context,
        )
    }
}

/// What the parameters make of the tool and the depth.
struct Plan {
    carve: bool,
    /// The depths of the levels, shallowest first, the last the full depth.
    levels: Vec<f64>,
    /// The offsets across the chain, millimetres, lowest first.
    offsets: Vec<f64>,
    start: Start,
    start_angle: f64,
    max_step: f64,
    tolerance: f64,
    /// The line cut at the full depth, for the note.
    width_at_depth: f64,
}

/// The half width of the groove the tool cuts with its tip `depth` below the
/// surface: the largest radius at which the tool profile is no higher.
fn half_width(tool: &RevolvedProfile, depth: f64) -> f64 {
    let radius = tool.cutting_radius();
    let reaches = |r: f64| tool.height_at(r).is_some_and(|h| h <= depth + 1e-12);
    if reaches(radius) {
        return radius;
    }
    let (mut lo, mut hi) = (0.0, radius);
    for _ in 0..60 {
        let mid = 0.5 * (lo + hi);
        if reaches(mid) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    lo
}

fn plan(tool: &RevolvedProfile, p: &Parameters) -> Result<Plan> {
    let carve = p.text(parameter::MODE)? == "carve";
    let line_width = p.number(parameter::LINE_WIDTH)?;
    let depth = if line_width > 0.0 {
        let half = 0.5 * line_width;
        let diameter = 2.0 * tool.cutting_radius();
        let depth = match tool.height_at(half) {
            Some(depth) => depth,
            None => {
                return Err(PathError::parameter(
                    parameter::LINE_WIDTH,
                    format!(
                        "{line_width} mm is wider than the tool's {diameter:.3} mm cutting \
                         diameter"
                    ),
                ));
            }
        };
        if depth < 1e-6 {
            return Err(PathError::parameter(
                parameter::LINE_WIDTH,
                format!(
                    "{line_width} mm is not wider than this tool's tip, which cuts that width \
                     with no depth; ask for a wider line or use a tool with a finer point"
                ),
            ));
        }
        depth
    } else {
        p.number(parameter::DEPTH)?
    };
    if depth > tool.top() + 1e-9 {
        return Err(PathError::parameter(
            parameter::DEPTH,
            format!(
                "a depth of {depth:.3} mm is more than the tool's {:.3} mm flute length",
                tool.top()
            ),
        ));
    }
    let per_pass = p.number(parameter::DEPTH_PER_PASS)?;
    let count = if per_pass > 0.0 {
        (depth / per_pass - 1e-9).ceil().max(1.0)
    } else {
        1.0
    };
    if count > MAX_LINES as f64 {
        return Err(PathError::parameter(
            parameter::DEPTH_PER_PASS,
            format!("{count:.0} levels are needed; increase the depth per pass"),
        ));
    }
    let count = count as usize;
    let levels: Vec<f64> = (1..=count)
        .map(|k| depth * k as f64 / count as f64)
        .collect();

    let width_at_depth = 2.0 * half_width(tool, depth);
    let offsets = if carve {
        let width = p.number(parameter::CARVE_WIDTH)?;
        if width <= 0.0 {
            return Err(PathError::parameter(
                parameter::CARVE_WIDTH,
                "a carve needs a width greater than zero",
            ));
        }
        let reach = 0.5 * (width - width_at_depth);
        if reach < -1e-9 {
            return Err(PathError::parameter(
                parameter::CARVE_WIDTH,
                format!(
                    "{width} mm is narrower than the {width_at_depth:.3} mm line this tool cuts \
                     at a depth of {depth:.3} mm; widen the carve or use a thinner tool"
                ),
            ));
        }
        let stepover = p.number(names::STEPOVER)?;
        let lines = if reach > 1e-9 {
            (2.0 * reach / stepover).ceil() + 1.0
        } else {
            1.0
        };
        if lines > MAX_LINES as f64 {
            return Err(PathError::parameter(
                names::STEPOVER,
                format!("{lines:.0} lines across the carve are needed; increase the stepover"),
            ));
        }
        let lines = lines as usize;
        if lines == 1 {
            vec![0.0]
        } else {
            (0..lines)
                .map(|i| -reach + 2.0 * reach * i as f64 / (lines - 1) as f64)
                .collect()
        }
    } else {
        vec![0.0]
    };
    let start = match p.text(parameter::START)? {
        "reversed" => Start::Reversed,
        "low_x" => Start::LowX,
        "high_x" => Start::HighX,
        "angle" => Start::Angle,
        _ => Start::First,
    };
    Ok(Plan {
        carve,
        levels,
        offsets,
        start,
        start_angle: p.number(parameter::START_ANGLE)?,
        max_step: p.number(parameter::MAX_STEP)?,
        tolerance: p.number(names::TOLERANCE)?,
        width_at_depth,
    })
}

/// The polar angle of a point about the rotary axis, degrees from +Y towards
/// +Z.
fn polar(p: Point3) -> f64 {
    p.z.atan2(p.y).to_degrees()
}

fn angle_distance(a: f64, b: f64) -> f64 {
    ((a - b + 180.0).rem_euclid(360.0) - 180.0).abs()
}

/// How many pieces the segment from `a` to `b` is cut into: none further
/// apart than `max_step`, and none turning about the rotary axis by more than
/// the tolerance allows (a chord of the circle at the point's radius may be no
/// more than `tolerance` below its arc, so the radial depth holds between
/// points).
fn pieces(a: Point3, b: Point3, max_step: f64, tolerance: f64) -> usize {
    let radius = a.y.hypot(a.z).max(b.y.hypot(b.z));
    let limit = if tolerance < radius {
        (2.0 * (1.0 - tolerance / radius).acos()).min(MAX_ANGLE)
    } else {
        MAX_ANGLE
    };
    let turn = angle_distance(polar(a), polar(b)).to_radians();
    (a.distance(b) / max_step)
        .ceil()
        .max((turn / limit.max(1e-6)).ceil())
        .max(1.0) as usize
}

/// `chain` resampled by [`pieces`]. The chain's own points are kept.
fn resample(chain: &EdgeChain, max_step: f64, tolerance: f64) -> Vec<Point3> {
    let mut points = Vec::new();
    for (a, b) in chain.points.iter().zip(chain.points.iter().skip(1)) {
        let n = pieces(*a, *b, max_step, tolerance);
        for i in 0..n {
            let f = i as f64 / n as f64;
            points.push(*a + (*b - *a) * f);
        }
    }
    if let Some(last) = chain.points.last() {
        points.push(*last);
    }
    points
}

/// How many points `chain` resamples to, without making them.
fn sample_count(chain: &EdgeChain, max_step: f64, tolerance: f64) -> f64 {
    chain
        .points
        .iter()
        .zip(chain.points.iter().skip(1))
        .map(|(a, b)| pieces(*a, *b, max_step, tolerance) as f64)
        .sum::<f64>()
        + 1.0
}

/// Puts the resampled points of a chain in cutting order (see the module
/// documentation, "Direction and start").
fn orient(mut points: Vec<Point3>, closed: bool, start: Start, angle: f64) -> Vec<Point3> {
    let Some(first) = points.first().copied() else {
        return points;
    };
    if closed && points.len() > 2 {
        // Closed: the last point is the first. Rotate to the wanted start.
        points.pop();
        let key = |p: &Point3| match start {
            Start::LowX => p.x,
            Start::HighX => -p.x,
            Start::Angle => angle_distance(polar(*p), angle),
            _ => 0.0,
        };
        let wanted = match start {
            Start::LowX | Start::HighX | Start::Angle => points
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| key(a).total_cmp(&key(b)))
                .map_or(0, |(i, _)| i),
            _ => 0,
        };
        points.rotate_left(wanted);
        if let Some(head) = points.first().copied() {
            points.push(head);
        }
        if start == Start::Reversed {
            points.reverse();
        }
        return points;
    }
    let Some(last) = points.last().copied() else {
        return points;
    };
    let flip = match start {
        Start::First => false,
        Start::Reversed => true,
        Start::LowX => last.x < first.x - 1e-9,
        Start::HighX => last.x > first.x + 1e-9,
        Start::Angle => {
            angle_distance(polar(last), angle) < angle_distance(polar(first), angle) - 1e-9
        }
    };
    if flip {
        points.reverse();
    }
    points
}

/// One point of a chain, with the tool axis there.
struct Site {
    point: Point3,
    a: Degrees,
    axis: UnitVec3,
    /// The direction across the chain, perpendicular to the tool axis; zero
    /// for a chain that runs along the tool axis.
    lateral: Vec3,
}

/// The direction opposite `axis`.
fn inward(axis: UnitVec3) -> UnitVec3 {
    UnitVec3::new(-axis.get()).unwrap_or(axis)
}

/// The point where a line from `from` along `direction` first meets the
/// surface, or `None` when it misses.
fn drop_along(
    input: &StrategyInput<'_>,
    from: Point3,
    direction: UnitVec3,
    distance: f64,
) -> Option<Point3> {
    input
        .surface
        .cast_ray(from, direction, distance)
        .map(|hit| hit.point)
}

/// The sites of `points` (a chain already in cutting order, on the surface or
/// to be projected onto it).
fn sites(input: &StrategyInput<'_>, points: &[Point3], on_surface: bool) -> Result<Vec<Site>> {
    let b = input.surface.bounds();
    let reach = b
        .min
        .y
        .abs()
        .max(b.max.y.abs())
        .hypot(b.min.z.abs().max(b.max.z.abs()))
        + 1.0;
    let placed: Vec<Result<(Point3, Degrees, UnitVec3)>> = parallel::map_in_order(points, |&p| {
        let radial = Vec3::new(0.0, p.y, p.z);
        if radial.length() < MIN_RADIUS {
            return Err(PathError::Strategy(format!(
                "a chain passes within {MIN_RADIUS} mm of the rotary axis at ({:.3}, {:.3}, \
                     {:.3}), where A cannot face it; edge following cannot cut it",
                p.x, p.y, p.z
            )));
        }
        let a = input.rotary.a_facing(radial).ok_or_else(|| {
            PathError::Strategy("a chain point lies on the rotary axis".to_owned())
        })?;
        let axis = input.rotary.tool_axis(a);
        let point = if on_surface {
            p
        } else {
            // Along the tool axis from outside the part.
            let height = radial.length();
            let from = p + axis.get() * (reach - height);
            drop_along(input, from, inward(axis), 2.0 * reach).ok_or_else(|| {
                PathError::Strategy(format!(
                    "a point of a curve at ({:.3}, {:.3}, {:.3}) does not project onto the \
                         surface along the tool axis",
                    p.x, p.y, p.z
                ))
            })?
        };
        Ok((point, a, axis))
    });
    let mut out = Vec::with_capacity(placed.len());
    for item in placed {
        out.push(item?);
    }
    let count = out.len();
    let tangent = |i: usize| -> Option<Vec3> {
        let before = out.get(i.saturating_sub(1))?.0;
        let after = out.get((i + 1).min(count.saturating_sub(1)))?.0;
        Some(after - before)
    };
    let mut result: Vec<Site> = Vec::with_capacity(count);
    let mut last_lateral: Option<Vec3> = None;
    let mut pending: Vec<usize> = Vec::new();
    for (i, (point, a, axis)) in out.iter().enumerate() {
        let across = tangent(i)
            .map(|t| axis.get().cross(t))
            .filter(|c| c.length() > 1e-9)
            .map(|c| c * (1.0 / c.length()));
        let lateral = match across.or(last_lateral) {
            Some(l) => l,
            None => {
                pending.push(i);
                Vec3::ZERO
            }
        };
        if across.is_some() {
            last_lateral = across;
        }
        result.push(Site {
            point: *point,
            a: *a,
            axis: *axis,
            lateral,
        });
    }
    // A chain along the tool axis has no side; it can be engraved but not
    // carved (see `follow_chains`).
    if let (false, Some(l)) = (pending.is_empty(), last_lateral) {
        for i in pending {
            if let Some(site) = result.get_mut(i) {
                site.lateral = l;
            }
        }
    }
    Ok(result)
}

/// The cut points of one chain: for every offset across it, the surface
/// points at that offset (where the tool tip is measured from).
fn offset_surface(
    input: &StrategyInput<'_>,
    sites: &[Site],
    offset: f64,
    reach: f64,
) -> Vec<Point3> {
    if offset == 0.0 {
        return sites.iter().map(|s| s.point).collect();
    }
    parallel::map_in_order(sites, |s| {
        let beside = s.point + s.lateral * offset;
        let from = beside + s.axis.get() * reach;
        drop_along(input, from, inward(s.axis), 2.0 * reach).unwrap_or(beside)
    })
}

/// Cuts `chains` as `parameters` say (see the module documentation). Public
/// so that a host with its own chains, or a strategy that wraps this one, can
/// use it without a [`CurveSource`]. `on_surface` says whether the chains lie
/// on the surface already; if not they are projected along the tool axis.
pub fn follow_chains(
    input: &StrategyInput<'_>,
    chains: &[EdgeChain],
    on_surface: bool,
    parameters: &Parameters,
    context: &JobContext<'_>,
) -> Result<StrategyOutput> {
    let entry = EntryParameters::read(parameters)?;
    if entry.style != EntryStyle::Plunge {
        return Err(PathError::parameter(
            names::ENTRY,
            "edge following cuts below the surface, where a lead, ramp or helix entry cannot be \
             checked against it; use a plunge entry",
        ));
    }
    let plan = plan(input.tool, parameters)?;

    let total: f64 = chains
        .iter()
        .map(|c| sample_count(c, plan.max_step, plan.tolerance))
        .sum::<f64>()
        * (plan.levels.len() * plan.offsets.len()) as f64;
    if total > MAX_POINTS {
        return Err(PathError::parameter(
            parameter::MAX_STEP,
            format!(
                "{total:.0} points are needed along the chains; increase the largest step, the \
                 depth per pass or the stepover"
            ),
        ));
    }

    let b = input.surface.bounds();
    let part_radius = b
        .min
        .y
        .abs()
        .max(b.max.y.abs())
        .hypot(b.min.z.abs().max(b.max.z.abs()));
    let reach = part_radius + plan.offsets.iter().fold(0.0_f64, |m, o| m.max(o.abs())) + 1.0;

    // The chains in cutting order, each with its offsets and levels.
    let mut cuts: Vec<ChainCut> = Vec::new();
    let mut sideless = 0;
    for (n, chain) in chains.iter().enumerate() {
        context.check_cancelled()?;
        context.report(
            "edge following: placing the tool on the chains",
            n as u64,
            Some(chains.len() as u64),
        );
        let points = resample(chain, plan.max_step, plan.tolerance);
        let points = orient(points, chain.closed, plan.start, plan.start_angle);
        if points.len() < 2 {
            continue;
        }
        let along = sites(input, &points, on_surface)?;
        if plan.carve && along.iter().all(|s| s.lateral.length() < 0.5) {
            sideless += 1;
            continue;
        }
        let lines: Vec<Vec<Point3>> = plan
            .offsets
            .iter()
            .map(|&o| offset_surface(input, &along, o, reach))
            .collect();
        cuts.push(ChainCut {
            closed: chain.closed,
            sites: along,
            lines,
        });
    }
    if cuts.is_empty() {
        return Err(PathError::Strategy(if sideless > 0 {
            "every chain runs along the tool axis, so none has a side to carve to; engrave it"
                .to_owned()
        } else {
            "edge following found no chain with two points to cut".to_owned()
        }));
    }
    let cuts = order(cuts);

    let mut passes: Vec<Pass> = Vec::new();
    for cut in &cuts {
        for &depth in &plan.levels {
            for line in &cut.lines {
                let points: Vec<CutPoint> = cut
                    .sites
                    .iter()
                    .zip(line)
                    .map(|(site, surface)| {
                        let tip = *surface + (-site.axis.get()) * depth;
                        CutPoint {
                            tip,
                            a: site.a,
                            contact: cutter::cutter_contact(input.tool, tip, site.axis, *surface),
                        }
                    })
                    .collect();
                passes.push(Pass {
                    points,
                    stock: None,
                    rapids: Vec::new(),
                });
            }
        }
    }
    context.check_cancelled()?;

    let closed = cuts.iter().filter(|c| c.closed).count();
    let full = plan.levels.last().copied().unwrap_or(0.0);
    let mut notes = vec![format!(
        "edge following: {} chains ({} open, {} closed), {} passes, {} mm deep in {} level{}; \
         the tool cuts a line {:.3} mm wide at that depth",
        cuts.len(),
        cuts.len() - closed,
        closed,
        passes.len(),
        trim(full),
        plan.levels.len(),
        if plan.levels.len() == 1 { "" } else { "s" },
        plan.width_at_depth,
    )];
    if sideless > 0 {
        notes.push(format!(
            "edge following: {sideless} chain{} run along the tool axis and cannot be carved, so \
             {} not cut",
            if sideless == 1 { "" } else { "s" },
            if sideless == 1 { "it is" } else { "they are" },
        ));
    }
    if plan.carve {
        notes.push(format!(
            "edge following: carving {} lines across each chain, the outer ones {:.3} mm either \
             side of it",
            plan.offsets.len(),
            plan.offsets.last().copied().unwrap_or(0.0)
        ));
    }
    Ok(StrategyOutput {
        passes,
        notes,
        warnings: Vec::new(),
    })
}

fn trim(x: f64) -> String {
    format!("{x:.3}")
}

/// A chain placed: the tool's sites along it and the surface points of each
/// line across it.
struct ChainCut {
    closed: bool,
    sites: Vec<Site>,
    lines: Vec<Vec<Point3>>,
}

/// The chains in a short order: the first has the lowest start (X, then Y,
/// then Z), each next is the one whose start is nearest the end of the one
/// before. Ties go to the earlier chain, so the order is deterministic.
fn order(mut cuts: Vec<ChainCut>) -> Vec<ChainCut> {
    let start = |c: &ChainCut| c.sites.first().map_or(Point3::ORIGIN, |s| s.point);
    let end = |c: &ChainCut| c.sites.last().map_or(Point3::ORIGIN, |s| s.point);
    let mut out = Vec::with_capacity(cuts.len());
    let first = cuts
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            let (p, q) = (start(a), start(b));
            p.x.total_cmp(&q.x)
                .then(p.y.total_cmp(&q.y))
                .then(p.z.total_cmp(&q.z))
        })
        .map(|(i, _)| i);
    let Some(first) = first else {
        return cuts;
    };
    let mut current = cuts.remove(first);
    loop {
        let here = end(&current);
        out.push(current);
        let nearest = cuts
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| start(a).distance(here).total_cmp(&start(b).distance(here)))
            .map(|(i, _)| i);
        match nearest {
            Some(i) => current = cuts.remove(i),
            None => break,
        }
    }
    out
}
