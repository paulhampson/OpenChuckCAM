// SPDX-License-Identifier: AGPL-3.0-or-later
//! Adaptive pass spacing (OP-03, OP-04, OP-05, OP-24; issue #303; decision
//! record 0239).
//!
//! An operation's passes form a family: each pass sits at a position `c`
//! across the family (an angle for passes along the axis, an offset along X
//! for the wraps of a helix) and runs along a position `s` (X for a pass
//! along the axis, the angle turned for a helix). With **Adaptive spacing**
//! on, the stepover is the widest spacing: between two neighbouring passes
//! the material they leave is measured, and where it stands higher than the
//! **Allowed excess** above the operation's own target, a pass is added
//! halfway between them over that stretch only, then the two halves are
//! measured again, never closer than the **Smallest spacing**.
//!
//! **The measure.** At each station along the two passes, the tool (grown by
//! the stock to leave) is dropped at evenly spaced poses between them, onto
//! the surface the operation cuts, with its own drop, region and parts of
//! the surface with their own stock to leave. Each contact is a point of the
//! surface between the passes; the material left over it is its clearance to
//! the real tool at the nearest poses of either pass, less the stock to
//! leave there. So what the operation is meant to leave is never counted as
//! missed. The tool is the operation's own cutter model (decision record
//! 0012), its poses the machine's straight moves between the points of each
//! pass, densified so the tool's travel along a pass adds no more than a
//! tenth of the Allowed excess.
//!
//! **What is not proved.** The measure samples; it is not a bound on every
//! point. Between two neighbouring samples `p` and `q` the height is taken
//! as bounded by `(p + q + d) / 2`, where `d` is the largest change between
//! neighbouring samples there: the bound of a function whose slope is no
//! steeper than the samples show. Where the samples meet the
//! Allowed excess but that bound does not, the place is reported as
//! unverified and not refined further (decision record 0239 item 11). A
//! feature narrower than the samples can be missed. The tests grade the
//! result independently, with the test kit's tool outline and contact
//! checker (decision record 0255 is test-only).
//!
//! **Limits.** Each operation's adaptive work is bounded, across all its
//! layers: geometry queries (drops), points, added passes, levels of halving
//! and pending intervals ([`Limits`]). Work is done in one deterministic
//! order, left first, and counted as it is done, so where a limit is met
//! does not depend on the thread count or the machine's load (GC-07,
//! NFR-06). A candidate pass that would pass a limit is dropped whole. Where
//! a limit is met, the excess cannot be confirmed or the passes would be
//! closer than the Smallest spacing, the path made so far is kept and a
//! warning that must be acknowledged before export says where (decision
//! record 0239 item 10).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicU64, Ordering};

use openchuck_types::geometry::{Aabb, Point3, UnitVec3};
use openchuck_types::job::JobContext;
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::tool::profile::RevolvedProfile;
use openchuck_types::units::Degrees;

use crate::assemble::PathWarning;
use crate::cutter::{Contact, ToolLine, bound_at, clearance_at};
use crate::error::{PathError, Result};
use crate::holes::Hole;
use crate::params::{
    ParameterKind, ParameterSchema, ParameterSpec, ParameterValue, Parameters, Unit,
};
use crate::rotary::{AxisPosition, RotaryMapping};
use crate::strategy::Pass;
use crate::surface::{FaceRef, RayHit, SurfacePoint, SurfaceSource};

/// Parameter names.
pub mod parameter {
    /// Space the passes by the material they leave: on or off.
    pub const ADAPTIVE_SPACING: &str = "adaptive_spacing";
    /// The closest two passes may be, mm.
    pub const MIN_SPACING: &str = "min_spacing";
    /// The Allowed excess, mm, for a strategy with no Scallop height: the
    /// same name, so one value serves both where both exist (decision record
    /// 0239 item 4).
    pub const ALLOWED_EXCESS: &str = "scallop";
}

/// The strategies with adaptive spacing (decision record 0239): their
/// operations saved before it are migrated to it off (project schema 24).
pub const STRATEGIES: [&str; 4] = [
    "helical_finishing",
    "axial_finishing",
    "indexed_finishing",
    "simultaneous_finishing",
];

/// The strategies whose lines along the axis gained adaptive spacing in the
/// lane's second pull request (OI-1453): rotary roughing's two axial forms
/// and flat Facing. Their operations saved before it are migrated to it off
/// (project schema 25).
pub const LINE_STRATEGIES: [&str; 2] = ["rotary_roughing", "facing"];

/// The Allowed excess when the operation sets none and Adaptive spacing is
/// on, mm (decision record 0239 item 4).
pub const DEFAULT_ALLOWED_EXCESS: f64 = 0.01;

/// The Smallest spacing when the operation sets none, mm (decision record
/// 0239 item 7).
pub const DEFAULT_MIN_SPACING: f64 = 0.01;

/// The settings an adaptive strategy adds to its schema: the switch, on for
/// a new operation (decision record 0239 item 6), and the Smallest spacing.
pub(crate) fn specs() -> Vec<ParameterSpec> {
    vec![
        ParameterSpec {
            name: parameter::ADAPTIVE_SPACING.to_owned(),
            label: "Adaptive spacing".to_owned(),
            description: "Space the passes by the material they leave: the stepover is the \
                          widest spacing, and passes are added where more than the Allowed \
                          excess would be left between them."
                .to_owned(),
            kind: ParameterKind::Flag { default: true },
        },
        ParameterSpec {
            name: parameter::MIN_SPACING.to_owned(),
            label: "Smallest spacing".to_owned(),
            description: "With adaptive spacing on, passes are never added closer together \
                          than this. A stepover already below it is not enlarged."
                .to_owned(),
            kind: ParameterKind::Number {
                unit: Unit::Millimetres,
                min: Some(0.0),
                min_exclusive: true,
                max: None,
                default: Some(DEFAULT_MIN_SPACING),
            },
        },
    ]
}

/// The Allowed excess, for a strategy with no Scallop height (rotary
/// roughing and Facing): shown only with Adaptive spacing on.
pub(crate) fn allowed_excess_spec() -> ParameterSpec {
    ParameterSpec {
        name: parameter::ALLOWED_EXCESS.to_owned(),
        label: "Allowed excess".to_owned(),
        description: ALLOWED_EXCESS_DESCRIPTION.to_owned(),
        kind: ParameterKind::Number {
            unit: Unit::Millimetres,
            min: Some(0.0),
            min_exclusive: false,
            max: None,
            default: Some(DEFAULT_ALLOWED_EXCESS),
        },
    }
}

const ALLOWED_EXCESS_DESCRIPTION: &str = "With adaptive spacing on, the most material that may \
                                          stand between two passes above the finished surface \
                                          and its stock to leave. 0 asks for none at all, which \
                                          may leave places unverified or meet the limits.";

/// `schema` as it applies to `values`, for a strategy with no Scallop
/// height whose lines run along the axis only in some of its forms
/// (`applies`): elsewhere the switch and its settings are hidden, and with
/// the switch off only the switch is shown (OI-1453).
pub(crate) fn schema_for_lines(
    mut schema: ParameterSchema,
    values: &BTreeMap<String, ParameterValue>,
    applies: bool,
) -> ParameterSchema {
    let hidden: &[&str] = if !applies {
        &[
            parameter::ADAPTIVE_SPACING,
            parameter::MIN_SPACING,
            parameter::ALLOWED_EXCESS,
        ]
    } else if is_on(values) {
        &[]
    } else {
        &[parameter::MIN_SPACING, parameter::ALLOWED_EXCESS]
    };
    schema
        .parameters
        .retain(|p| !hidden.contains(&p.name.as_str()));
    schema
}

/// Whether `values` turn Adaptive spacing on: on unless set off, as for a
/// new operation.
pub(crate) fn is_on(values: &BTreeMap<String, ParameterValue>) -> bool {
    !matches!(
        values.get(parameter::ADAPTIVE_SPACING),
        Some(ParameterValue::Flag(false))
    )
}

/// `schema` as it applies to `values`: with Adaptive spacing on, the height
/// setting `height` is the Allowed excess, 0.01 mm when not set; with it off
/// it is the Scallop height as before, and the Smallest spacing is hidden
/// (decision record 0239 item 4). One value serves both.
pub(crate) fn schema_for(
    mut schema: ParameterSchema,
    values: &BTreeMap<String, ParameterValue>,
    height: &str,
) -> ParameterSchema {
    if is_on(values) {
        if let Some(spec) = schema.parameters.iter_mut().find(|p| p.name == height) {
            spec.label = "Allowed excess".to_owned();
            spec.description = ALLOWED_EXCESS_DESCRIPTION.to_owned();
            if let ParameterKind::Number { default, .. } = &mut spec.kind {
                *default = Some(DEFAULT_ALLOWED_EXCESS);
            }
        }
    } else {
        schema
            .parameters
            .retain(|p| p.name != parameter::MIN_SPACING);
    }
    schema
}

/// An operation's adaptive settings, when Adaptive spacing is on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Settings {
    /// The Allowed excess, mm.
    pub(crate) height: f64,
    /// The Smallest spacing, mm.
    pub(crate) min_spacing: f64,
}

impl Settings {
    /// Reads the settings, the Allowed excess from `height`; `None` with
    /// Adaptive spacing off. A height or spacing that is not a finite
    /// number, or not above zero for the spacing, refuses the operation
    /// (NFR-07).
    pub(crate) fn read(parameters: &Parameters, height: &str) -> Result<Option<Self>> {
        if !parameters
            .flag(parameter::ADAPTIVE_SPACING)
            .unwrap_or(false)
        {
            return Ok(None);
        }
        let h = parameters.number(height)?;
        if !(h.is_finite() && h >= 0.0) {
            return Err(PathError::parameter(
                height,
                "the allowed excess must be a finite number of millimetres, at least 0",
            ));
        }
        let min_spacing = parameters.number(parameter::MIN_SPACING)?;
        if !(min_spacing.is_finite() && min_spacing > 0.0) {
            return Err(PathError::parameter(
                parameter::MIN_SPACING,
                "the smallest spacing must be a finite number of millimetres above 0",
            ));
        }
        Ok(Some(Self {
            height: h,
            min_spacing,
        }))
    }
}

/// The bounds on one operation's adaptive work, counted across all its
/// layers (decision record 0239 item 8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Limits {
    /// Geometry queries: drops of the tool and rays, for measuring and
    /// fitting.
    pub(crate) queries: u64,
    /// Points of the operation's passes, its own and the added ones, with
    /// [`LINK_POINTS`] for each pass's entry and link.
    pub(crate) points: u64,
    /// Passes added.
    pub(crate) passes: u64,
    /// Times an interval between two passes is halved.
    pub(crate) levels: u32,
    /// Intervals waiting to be measured.
    pub(crate) intervals: usize,
}

impl Limits {
    /// The owner's limits (decision record 0239 item 8).
    pub(crate) const OWNER: Self = Self {
        queries: 1_000_000,
        points: 1_000_000,
        passes: 10_000,
        levels: 20,
        intervals: 65_536,
    };
}

/// Points charged for each pass's entry and link, which the shared services
/// make after the strategy: a bound on the plunge, ramp, retract and
/// clearance moves one pass adds (decision record 0239 item 8).
pub(crate) const LINK_POINTS: u64 = 16;

/// Why adaptive spacing could not confirm a place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Reason {
    /// The geometry query limit was met.
    Queries,
    /// The point limit was met.
    Points,
    /// The added pass limit was met.
    Passes,
    /// The interval was halved as often as allowed.
    Levels,
    /// Too many intervals were waiting.
    Intervals,
    /// Another pass would be closer than the Smallest spacing.
    Spacing,
    /// The samples met the height but no bound justified it.
    Unverified,
    /// The operation's tool cannot reach the place as the operation holds
    /// it (a vertical tool at one A, for indexed finishing).
    Reach,
}

impl Reason {
    fn words(self, limits: &Limits, settings: &Settings) -> String {
        match self {
            Self::Queries => format!("the limit of {} geometry queries was met", limits.queries),
            Self::Points => format!("the limit of {} points was met", limits.points),
            Self::Passes => format!("the limit of {} added passes was met", limits.passes),
            Self::Levels => format!(
                "the space between two passes was halved {} times, the most allowed",
                limits.levels
            ),
            Self::Intervals => format!(
                "the limit of {} places waiting to be measured was met",
                limits.intervals
            ),
            Self::Spacing => format!(
                "another pass would be closer than the smallest spacing of {:.3} mm",
                settings.min_spacing
            ),
            Self::Unverified => "the samples there are within it, but the measure could not \
                                 bound the height between them"
                .to_owned(),
            Self::Reach => "the tool, held vertical at one of the operation's angles, cannot \
                            reach there"
                .to_owned(),
        }
    }
}

/// A [`SurfaceSource`] that counts its drops and rays, the geometry queries
/// adaptive spacing is limited by. The count is the same at any thread count, since
/// the same drops are made.
#[derive(Debug)]
pub(crate) struct Counting<'a> {
    inner: &'a dyn SurfaceSource,
    count: AtomicU64,
}

impl<'a> Counting<'a> {
    pub(crate) fn new(inner: &'a dyn SurfaceSource) -> Self {
        Self {
            inner,
            count: AtomicU64::new(0),
        }
    }

    /// Drops and rays made so far.
    pub(crate) fn count(&self) -> u64 {
        self.count.load(Ordering::Relaxed)
    }
}

impl SurfaceSource for Counting<'_> {
    fn bounds(&self) -> Aabb {
        self.inner.bounds()
    }

    fn project(&self, point: Point3) -> Option<SurfacePoint> {
        self.inner.project(point)
    }

    fn distance(&self, point: Point3) -> Option<f64> {
        self.inner.distance(point)
    }

    fn within(&self, point: Point3, max_distance: f64) -> bool {
        self.inner.within(point, max_distance)
    }

    fn cast_ray(&self, origin: Point3, direction: UnitVec3, max_distance: f64) -> Option<RayHit> {
        self.count.fetch_add(1, Ordering::Relaxed);
        self.inner.cast_ray(origin, direction, max_distance)
    }

    fn tessellation(&self) -> &TriangleMesh {
        self.inner.tessellation()
    }

    fn drop_tool(&self, profile: &RevolvedProfile, line: &ToolLine) -> Option<Contact> {
        self.count.fetch_add(1, Ordering::Relaxed);
        self.inner.drop_tool(profile, line)
    }

    fn drop_accuracy(&self) -> f64 {
        self.inner.drop_accuracy()
    }

    fn drop_on_part(&self, profile: &RevolvedProfile, line: &ToolLine) -> Option<Contact> {
        self.count.fetch_add(1, Ordering::Relaxed);
        self.inner.drop_on_part(profile, line)
    }

    fn holes(&self) -> &[Hole] {
        self.inner.holes()
    }

    fn exact_faces(&self) -> Vec<FaceRef<'_>> {
        self.inner.exact_faces()
    }
}

/// One tool position of a pass: where it is along the pass, its tip and its
/// axis, in the model frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Pose {
    pub(crate) s: f64,
    pub(crate) tip: Point3,
    pub(crate) axis: UnitVec3,
}

/// The passes of a family at one position across it, and the tool's poses
/// along them, in order along the family.
#[derive(Debug, Clone)]
pub(crate) struct Track {
    /// The position across the family.
    pub(crate) c: f64,
    /// The passes, as fitted.
    pub(crate) passes: Vec<Pass>,
    /// The tool's poses along the passes, in order of `s`.
    poses: Vec<Pose>,
}

impl Track {
    /// The track of `passes` at `c` in `family`.
    pub(crate) fn new(family: &dyn Family, c: f64, passes: Vec<Pass>) -> Self {
        let mut poses: Vec<Pose> = passes
            .iter()
            .flat_map(|pass| family.poses(c, pass))
            .collect();
        poses.sort_by(|p, q| p.s.total_cmp(&q.s));
        Self { c, passes, poses }
    }

    /// The poses within `window` of `s`.
    fn near(&self, s: f64, window: f64) -> &[Pose] {
        let from = self.poses.partition_point(|p| p.s < s - window);
        let to = self.poses.partition_point(|p| p.s <= s + window);
        self.poses.get(from..to).unwrap_or(&[])
    }
}

/// The distance between the tool's poses along a pass, mm, for a tool of
/// cutting radius `r` and an Allowed excess of `height`: the travel between
/// poses leaves no more than a tenth of the Allowed excess (a ball's cusp
/// along a pass is about step^2 / 8r), at least 0.02 mm and at most half the
/// radius, and never above half the radius for a tool too small for 0.02 mm
/// (NFR-07).
pub(crate) fn pose_step(r: f64, height: f64) -> f64 {
    let most = 0.5 * r;
    (0.8 * r * height).sqrt().clamp(0.02_f64.min(most), most)
}

/// The poses of the tool along `pass`, each point with the machine's
/// straight move to the next divided so neighbouring poses are no more than
/// `step` mm apart: the moves are straight in the machine's X, Y, Z and A,
/// not in the model (decision records 0025 item 3, 0115). `s` gives each
/// pose's position along its family.
pub(crate) fn machine_poses(
    rotary: &RotaryMapping,
    pass: &Pass,
    step: f64,
    s: impl Fn(Point3, Degrees) -> f64,
) -> Vec<Pose> {
    let mut out = Vec::new();
    let pose = |tip: Point3, a: Degrees| Pose {
        s: s(tip, a),
        tip,
        axis: rotary.tool_axis(a),
    };
    for (i, point) in pass.points.iter().enumerate() {
        if let Some(next) = pass.points.get(i + 1) {
            let (p, q) = (
                rotary.to_axis(point.tip, point.a),
                rotary.to_axis(next.tip, next.a),
            );
            let length = ((q.along - p.along).powi(2)
                + (q.across - p.across).powi(2)
                + (q.height - p.height).powi(2))
            .sqrt()
            .max(point.tip.distance(next.tip));
            let n = if step > 0.0 && length.is_finite() {
                ((length / step).ceil() as usize).clamp(1, 10_000)
            } else {
                1
            };
            for k in 0..n {
                let t = k as f64 / n as f64;
                let a = Degrees(point.a.0 + (next.a.0 - point.a.0) * t);
                let at = AxisPosition {
                    along: p.along + (q.along - p.along) * t,
                    across: p.across + (q.across - p.across) * t,
                    height: p.height + (q.height - p.height) * t,
                };
                out.push(pose(rotary.to_model(at, a), a));
            }
        } else {
            out.push(pose(point.tip, point.a));
        }
    }
    out
}

/// A family of passes, as adaptive spacing sees it.
pub(crate) trait Family: Sync {
    /// The model X and A of the pass at `c` across the family, at `s` along
    /// it.
    fn pose(&self, c: f64, s: f64) -> (f64, Degrees);

    /// The tool's poses along `pass`, a pass of the family at `c`.
    fn poses(&self, c: f64, pass: &Pass) -> Vec<Pose>;

    /// The drop of the operation's tool at model X `x`, angle `a`, as the
    /// operation drops it (its grown tool, region and parts with their own
    /// stock to leave): the contact on the surface and the stock to leave
    /// there; `None` where it finds nothing it cuts.
    fn probe(&self, x: f64, a: Degrees) -> Option<(Point3, f64)>;

    /// The pass at `c` from `span.0` to `span.1` along the family, fitted as
    /// the operation fits its passes.
    fn fit(&self, c: f64, span: (f64, f64), context: &JobContext<'_>) -> Result<Vec<Pass>>;

    /// The distance, mm, between passes `dc` apart across the family, at
    /// its widest.
    fn width(&self, dc: f64) -> f64;

    /// How far along the family, in its units, a pose can touch the
    /// surface at a station.
    fn window(&self) -> f64;

    /// The distance between stations along the family, in its units.
    fn station_step(&self) -> f64;

    /// How far past a station where too much is left, along the family in
    /// its units, a pass added there runs: about the tool's radius, so the
    /// tool sweeps the whole place.
    fn margin(&self) -> f64;

    /// Geometry queries made so far.
    fn queries(&self) -> u64;

    /// The most geometry queries one [`probe`](Self::probe) makes.
    fn probe_cost(&self) -> u64 {
        1
    }

    /// Whether the operation can add a pass at `c`; where it cannot, a
    /// place that holds too much is reported, not refined.
    fn reachable(&self, _c: f64) -> bool {
        true
    }

    /// The model X and A to report for a place at `c` across the family, at
    /// `s` along it: [`pose`](Self::pose), unless the family's positions
    /// across it are not angles (offsets across the axis at one A).
    fn at(&self, c: f64, s: f64) -> (f64, Degrees) {
        self.pose(c, s)
    }
}

/// One measuring interval: the stretch between two tracks, by index, where
/// it is measured, and how often it has been halved.
#[derive(Debug, Clone)]
pub(crate) struct Interval {
    pub(crate) lo: usize,
    pub(crate) hi: usize,
    pub(crate) spans: Vec<(f64, f64)>,
    pub(crate) level: u32,
}

/// A place adaptive spacing could not confirm, and why.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Place {
    pub(crate) x: f64,
    pub(crate) a: f64,
    pub(crate) reason: Reason,
}

/// What adaptive spacing did.
#[derive(Debug, Clone)]
pub(crate) struct Outcome {
    /// Every track, the operation's own first, then the added ones in the
    /// order they were made.
    pub(crate) tracks: Vec<Track>,
    /// How many of `tracks` are the operation's own.
    pub(crate) own: usize,
    /// The smallest spacing between neighbouring passes added, mm.
    pub(crate) smallest: Option<f64>,
    /// The places it could not confirm.
    pub(crate) places: Vec<Place>,
    /// The geometry queries it made.
    pub(crate) queries: u64,
}

impl Outcome {
    /// Passes added.
    pub(crate) fn added(&self) -> usize {
        self.tracks
            .iter()
            .skip(self.own)
            .map(|t| t.passes.len())
            .sum()
    }

    /// The note: how many passes were added and the smallest spacing used
    /// (decision record 0239 item 12).
    pub(crate) fn note(&self, settings: &Settings) -> String {
        note(self.added(), self.smallest, self.queries, settings)
    }

    /// The warning, when a place could not be confirmed: the path made is
    /// kept, and must be acknowledged before export (decision record 0239
    /// item 10).
    pub(crate) fn warning(
        &self,
        settings: &Settings,
        limits: &Limits,
        height: &str,
    ) -> Option<PathWarning> {
        warning(&self.places, settings, limits, height)
    }
}

/// What several refinements of one operation did together: rotary
/// roughing refines each layer, and each A position of a layer, apart
/// (OI-1453), under one set of limits.
#[derive(Debug, Clone, Default)]
pub(crate) struct Tally {
    /// Passes added.
    pub(crate) added: u64,
    /// The smallest spacing between neighbouring passes added, mm.
    pub(crate) smallest: Option<f64>,
    /// The places not confirmed.
    pub(crate) places: Vec<Place>,
    /// The geometry queries made.
    pub(crate) queries: u64,
}

impl Tally {
    /// Adds `outcome` to the tally.
    pub(crate) fn absorb(&mut self, outcome: &Outcome) {
        self.added = self.added.saturating_add(outcome.added() as u64);
        self.smallest = match (self.smallest, outcome.smallest) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        self.places.extend(outcome.places.iter().copied());
        self.queries = self.queries.max(outcome.queries);
    }

    /// The note, as [`Outcome::note`].
    pub(crate) fn note(&self, settings: &Settings) -> String {
        note(
            usize::try_from(self.added).unwrap_or(usize::MAX),
            self.smallest,
            self.queries,
            settings,
        )
    }

    /// The warning, as [`Outcome::warning`].
    pub(crate) fn warning(
        &self,
        settings: &Settings,
        limits: &Limits,
        height: &str,
    ) -> Option<PathWarning> {
        warning(&self.places, settings, limits, height)
    }
}

/// The note for `added` passes, the closest `smallest` from a neighbour,
/// after `queries` geometry queries (decision record 0239 item 12).
fn note(added: usize, smallest: Option<f64>, queries: u64, settings: &Settings) -> String {
    match (added, smallest) {
        (0, _) | (_, None) => format!(
            "adaptive spacing: no passes were added; the material measured between the \
             passes is within the allowed excess of {:.3} mm",
            settings.height
        ),
        (n, Some(w)) => format!(
            "adaptive spacing: {n} pass{} added where more than the allowed excess of \
             {:.3} mm was left, the closest {w:.3} mm from its neighbour; measuring and \
             fitting them made {} geometry queries",
            if n == 1 { "" } else { "es" },
            settings.height,
            queries
        ),
    }
}

/// The warning for `places` not confirmed (decision record 0239 item 10).
fn warning(
    places: &[Place],
    settings: &Settings,
    limits: &Limits,
    height: &str,
) -> Option<PathWarning> {
    let first = places.first()?;
    let reasons: BTreeSet<Reason> = places.iter().map(|p| p.reason).collect();
    let why: Vec<String> = reasons.iter().map(|r| r.words(limits, settings)).collect();
    let count = places.len();
    Some(PathWarning {
        message: format!(
            "adaptive spacing could not confirm that the material left between the passes \
             is within the allowed excess of {:.3} mm at {count} place{}, the first near \
             X {:.3} A {:.1}: {}. The path made so far is kept; it is not complete. \
             Check the surface there before using the program",
            settings.height,
            if count == 1 { "" } else { "s" },
            first.x,
            first.a,
            why.join("; ")
        ),
        parameter: Some(height.to_owned()),
        requires_acknowledgement: true,
    })
}

/// The verdict of one station.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Station {
    s: f64,
    /// The highest material measured, mm above the target; `None` where no
    /// sample found surface the operation cuts.
    excess: Option<f64>,
    /// The height between the samples, bounded from their own slope, mm.
    bound: f64,
}

/// The bound of the height between neighbouring `values`: between `p` and
/// `q`, `(p + q + d) / 2`, with `d` the largest change between neighbours
/// at and next to them. Samples with nothing to cut are skipped.
fn sloped_bound(values: &[Option<f64>]) -> f64 {
    let change = |i: usize| match (values.get(i), values.get(i + 1)) {
        (Some(Some(p)), Some(Some(q))) => (p - q).abs(),
        _ => 0.0,
    };
    let mut bound = values.iter().flatten().copied().fold(0.0, f64::max);
    for i in 0..values.len().saturating_sub(1) {
        if let (Some(Some(p)), Some(Some(q))) = (values.get(i), values.get(i + 1)) {
            let d = [i.checked_sub(1), Some(i), Some(i + 1)]
                .into_iter()
                .flatten()
                .map(change)
                .fold(0.0, f64::max);
            bound = bound.max(0.5 * (p + q + d));
        }
    }
    bound
}

/// Samples across an interval at a station, before any are added.
const SAMPLES: usize = 8;

/// The most samples across an interval at a station.
const MOST_SAMPLES: usize = 64;

/// The excess taken where no pose of either pass reaches: the tool never
/// came near, so the material is the full height of what is there.
const UNREACHED: f64 = 1.0e3;

/// Adaptive spacing over `family`, from its own `tracks` and the intervals
/// between them to measure, for `tool` (the real tool), with `points`
/// already in the operation's passes.
#[allow(clippy::too_many_arguments)]
pub(crate) fn refine(
    family: &dyn Family,
    tracks: Vec<Track>,
    intervals: Vec<Interval>,
    tool: &RevolvedProfile,
    settings: &Settings,
    limits: &Limits,
    points: u64,
    context: &JobContext<'_>,
) -> Result<Outcome> {
    let own = tracks.len();
    let mut work = Work {
        family,
        tool,
        radius: tool.cutting_radius(),
        settings,
        limits,
        tracks,
        points,
        added: 0,
        smallest: None,
        places: Vec::new(),
        stopped: None,
    };
    // Left first: the stack's top is the interval to do next.
    let mut stack: Vec<Interval> = intervals.into_iter().rev().collect();
    let stage = "adaptive spacing: measuring between the passes";
    let mut done = 0_u64;
    context.report(stage, 0, None);
    while let Some(interval) = stack.pop() {
        context.check_cancelled()?;
        done += 1;
        context.report(stage, done, None);
        let children = work.interval(&interval, context)?;
        for child in children.into_iter().rev() {
            if stack.len() >= limits.intervals {
                work.give_up(&child, Reason::Intervals);
            } else {
                stack.push(child);
            }
        }
    }
    Ok(Outcome {
        queries: family.queries(),
        tracks: work.tracks,
        own,
        smallest: work.smallest,
        places: work.places,
    })
}

struct Work<'a> {
    family: &'a dyn Family,
    tool: &'a RevolvedProfile,
    radius: f64,
    settings: &'a Settings,
    limits: &'a Limits,
    tracks: Vec<Track>,
    points: u64,
    added: u64,
    smallest: Option<f64>,
    places: Vec<Place>,
    /// The limit met, after which no more is added.
    stopped: Option<Reason>,
}

impl Work<'_> {
    /// Measures `interval`, adds a pass halfway where the material left is
    /// too high, and gives the two halves to measure next.
    fn interval(&mut self, interval: &Interval, context: &JobContext<'_>) -> Result<Vec<Interval>> {
        if let Some(reason) = self.stopped {
            self.give_up(interval, reason);
            return Ok(Vec::new());
        }
        let (Some(lo), Some(hi)) = (self.tracks.get(interval.lo), self.tracks.get(interval.hi))
        else {
            return Ok(Vec::new());
        };
        let step = self.family.station_step();
        let mut at = Vec::new();
        for &(from, to) in &interval.spans {
            let n = (((to - from).abs() / step).ceil() as usize).clamp(1, 1_000_000);
            at.extend((0..=n).map(|k| from + (to - from) * k as f64 / n as f64));
        }
        // The most queries a station can make is charged before it is
        // measured, so where the limit is met does not depend on the order
        // the threads measure them in (decision record 0239 item 8). The
        // stations are measured in runs whose most fits what is left of the
        // limit, each charged with what the runs before it really made: a
        // station makes far fewer than its most, so charging the whole
        // interval's most at once met the limit with most of it unused
        // (issue #462).
        let per_station = ((MOST_SAMPLES - 1) as u64).saturating_mul(self.family.probe_cost());
        let mut stations = Vec::with_capacity(at.len());
        while stations.len() < at.len() {
            let left = self.limits.queries.saturating_sub(self.family.queries());
            let fits = usize::try_from(left / per_station.max(1)).unwrap_or(usize::MAX);
            let run = at.get(stations.len()..).unwrap_or(&[]);
            let Some(run) = run.get(..fits.min(run.len())).filter(|r| !r.is_empty()) else {
                self.stopped = Some(Reason::Queries);
                self.give_up(interval, Reason::Queries);
                return Ok(Vec::new());
            };
            context.check_cancelled()?;
            stations.extend(crate::parallel::map_each_in_order(run, |&s| {
                self.station(lo, hi, s)
            }));
        }
        let h = self.settings.height;
        let mut high = Vec::new();
        let mut unsure = Vec::new();
        let along: Vec<Option<f64>> = stations.iter().map(|st| st.excess).collect();
        for (i, st) in stations.iter().enumerate() {
            let Some(e) = st.excess else { continue };
            // The bound across the passes at this station, and along them to
            // its neighbours.
            let from = i.saturating_sub(1);
            let near = along.get(from..(i + 2).min(along.len())).unwrap_or(&[]);
            let between = st.bound.max(sloped_bound(near));
            if e > h + EPS {
                high.push(st.s);
            } else if between > h + EPS {
                unsure.push(st.s);
            }
        }
        let reach = step + self.family.margin();
        let spans_of = |at: &[f64]| merge(at, reach, &interval.spans);
        let c_mid = 0.5 * (lo.c + hi.c);
        let half = self.family.width((hi.c - lo.c) / 2.0);
        let high = spans_of(&high);
        // A place that cannot be confirmed within a stretch that gets a pass
        // is measured again in the halves.
        let inside = |s: f64| {
            high.iter()
                .any(|&(a, b)| s >= a.min(b) - EPS && s <= a.max(b) + EPS)
        };
        unsure.retain(|&s| !inside(s));
        for (from, to) in spans_of(&unsure) {
            self.place(c_mid, 0.5 * (from + to), Reason::Unverified);
        }
        if high.is_empty() {
            return Ok(Vec::new());
        }
        let reason = if interval.level >= self.limits.levels {
            Some(Reason::Levels)
        } else if half < self.settings.min_spacing {
            Some(Reason::Spacing)
        } else if !self.family.reachable(c_mid) {
            Some(Reason::Reach)
        } else {
            None
        };
        if let Some(reason) = reason {
            for &(from, to) in &high {
                self.place(c_mid, 0.5 * (from + to), reason);
            }
            return Ok(Vec::new());
        }
        // The candidate pass, fitted over each stretch, and kept whole or
        // not at all.
        let mut passes = Vec::new();
        for &span in &high {
            passes.extend(self.family.fit(c_mid, span, context)?);
        }
        let count = passes.len() as u64;
        let points: u64 = passes
            .iter()
            .map(|p| p.points.len() as u64 + LINK_POINTS)
            .sum();
        let over = if self.family.queries() > self.limits.queries {
            Some(Reason::Queries)
        } else if self.added.saturating_add(count) > self.limits.passes {
            Some(Reason::Passes)
        } else if self.points.saturating_add(points) > self.limits.points {
            Some(Reason::Points)
        } else {
            None
        };
        if let Some(reason) = over {
            self.stopped = Some(reason);
            for &(from, to) in &high {
                self.place(c_mid, 0.5 * (from + to), reason);
            }
            return Ok(Vec::new());
        }
        self.added += count;
        self.points += points;
        if count > 0 {
            self.smallest = Some(self.smallest.map_or(half, |w| w.min(half)));
        }
        let track = Track::new(self.family, c_mid, passes);
        let mid = self.tracks.len();
        self.tracks.push(track);
        let level = interval.level + 1;
        Ok(vec![
            Interval {
                lo: interval.lo,
                hi: mid,
                spans: high.clone(),
                level,
            },
            Interval {
                lo: mid,
                hi: interval.hi,
                spans: high,
                level,
            },
        ])
    }

    /// Records every stretch of `interval` as not confirmed, for `reason`.
    fn give_up(&mut self, interval: &Interval, reason: Reason) {
        let (Some(lo), Some(hi)) = (
            self.tracks.get(interval.lo).map(|t| t.c),
            self.tracks.get(interval.hi).map(|t| t.c),
        ) else {
            return;
        };
        for &(from, to) in &interval.spans {
            self.place(0.5 * (lo + hi), 0.5 * (from + to), reason);
        }
    }

    fn place(&mut self, c: f64, s: f64, reason: Reason) {
        let (x, a) = self.family.at(c, s);
        self.places.push(Place { x, a: a.0, reason });
    }

    /// The material left between `lo` and `hi` at `s`.
    fn station(&self, lo: &Track, hi: &Track, s: f64) -> Station {
        let window = self.family.window();
        let poses: Vec<Pose> = lo
            .near(s, window)
            .iter()
            .chain(hi.near(s, window))
            .copied()
            .collect();
        let sample = |t: f64| -> Option<f64> {
            let (x, a) = self.family.pose(lo.c + (hi.c - lo.c) * t, s);
            let (contact, leave) = self.family.probe(x, a)?;
            let mut best = UNREACHED;
            for pose in &poses {
                if bound_at(self.radius, pose.tip, pose.axis, contact) >= best {
                    continue;
                }
                best = best.min(clearance_at(self.tool, pose.tip, pose.axis, contact));
            }
            Some((best - leave).max(0.0))
        };
        // Evenly spaced samples, doubled while the bound between them is
        // above the Allowed excess and the samples are not, up to
        // `MOST_SAMPLES`.
        // Doubling keeps the samples it has: sample k of n is sample 2k of
        // 2n, the same position exactly, so a station makes at most
        // `MOST_SAMPLES - 1` probes, as it is charged (decision record 0239
        // item 8).
        let h = self.settings.height;
        let mut n = SAMPLES;
        let mut values: Vec<Option<f64>> = (1..n).map(|j| sample(j as f64 / n as f64)).collect();
        loop {
            let excess = values.iter().flatten().copied().reduce(f64::max);
            let bound = sloped_bound(&values);
            let settled = bound <= h + EPS || n >= MOST_SAMPLES;
            if settled || excess.is_none_or(|e| e > h + EPS) {
                return Station { s, excess, bound };
            }
            n *= 2;
            values = (1..n)
                .map(|j| {
                    if j % 2 == 0 {
                        values.get(j / 2 - 1).copied().flatten()
                    } else {
                        sample(j as f64 / n as f64)
                    }
                })
                .collect();
        }
    }
}

/// Heights within this of the Allowed excess meet it, mm.
const EPS: f64 = 1e-9;

/// The stretches around each of `at`, `reach` either side, within `spans`,
/// merged where they overlap.
fn merge(at: &[f64], reach: f64, spans: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let mut out: Vec<(f64, f64)> = Vec::new();
    for &(from, to) in spans {
        let (lo, hi) = (from.min(to), from.max(to));
        let forward = to >= from;
        let mut pieces: Vec<(f64, f64)> = Vec::new();
        for &s in at.iter().filter(|&&s| s >= lo - EPS && s <= hi + EPS) {
            let piece = ((s - reach).max(lo), (s + reach).min(hi));
            match pieces.last_mut() {
                Some(last) if piece.0 <= last.1 => last.1 = last.1.max(piece.1),
                _ => pieces.push(piece),
            }
        }
        out.extend(
            pieces
                .into_iter()
                .map(|(a, b)| if forward { (a, b) } else { (b, a) }),
        );
    }
    out
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]

    use super::*;
    use crate::strategy::CutPoint;
    use openchuck_types::geometry::Vec3;
    use openchuck_types::job::{CancelToken, NoProgress};
    use openchuck_types::tool::{Tool, ToolShape};
    use openchuck_types::units::Millimetres;

    /// Passes along X over the plane z = 0, each at y = `c`, with a ball
    /// tool standing vertically on the plane.
    struct Plane {
        queries: AtomicU64,
        length: f64,
        /// Instead of the plane, material of 0.009 mm over the target
        /// across the first part of each interval, up to this position
        /// across the family, and none past it: every sample is within an
        /// Allowed excess of 0.01 mm, but beside the step the bound between
        /// samples never is, so each station doubles its samples to the
        /// most.
        step: Option<f64>,
    }

    impl Plane {
        fn new() -> Self {
            Self {
                queries: AtomicU64::new(0),
                length: 10.0,
                step: None,
            }
        }

        fn pass(&self, c: f64, (from, to): (f64, f64)) -> Pass {
            let n = 20;
            Pass {
                helical_pass_start: false,
                points: (0..=n)
                    .map(|k| {
                        let x = from + (to - from) * k as f64 / n as f64;
                        let tip = Point3::new(x, c, 0.0);
                        CutPoint {
                            tip,
                            a: Degrees(0.0),
                            contact: tip,
                        }
                    })
                    .collect(),
                stock: None,
                rapids: Vec::new(),
            }
        }
    }

    impl Family for Plane {
        fn pose(&self, c: f64, s: f64) -> (f64, Degrees) {
            (s, Degrees(c))
        }

        fn poses(&self, _c: f64, pass: &Pass) -> Vec<Pose> {
            let axis = UnitVec3::new(Vec3::new(0.0, 0.0, 1.0)).unwrap();
            pass.points
                .iter()
                .map(|p| Pose {
                    s: p.tip.x,
                    tip: p.tip,
                    axis,
                })
                .collect()
        }

        fn probe(&self, x: f64, a: Degrees) -> Option<(Point3, f64)> {
            self.queries.fetch_add(1, Ordering::Relaxed);
            if let Some(edge) = self.step {
                // A contact far below every pose is measured as UNREACHED;
                // the stock to leave there sets the material over it.
                let left = if a.0 < edge { 0.009 } else { 0.0 };
                return Some((Point3::new(x, a.0, -5.0e3), UNREACHED - left));
            }
            Some((Point3::new(x, a.0, 0.0), 0.0))
        }

        fn fit(&self, c: f64, span: (f64, f64), _: &JobContext<'_>) -> Result<Vec<Pass>> {
            self.queries.fetch_add(21, Ordering::Relaxed);
            Ok(vec![self.pass(c, span)])
        }

        fn width(&self, dc: f64) -> f64 {
            dc.abs()
        }

        fn window(&self) -> f64 {
            2.5
        }

        fn station_step(&self) -> f64 {
            1.0
        }

        fn margin(&self) -> f64 {
            2.0
        }

        fn queries(&self) -> u64 {
            self.queries.load(Ordering::Relaxed)
        }
    }

    fn ball(diameter: f64) -> RevolvedProfile {
        Tool {
            shape: ToolShape::BallEnd,
            diameter: Millimetres(diameter),
            flute_length: Millimetres(10.0),
            overall_length: Millimetres(40.0),
            shank_diameter: Millimetres(diameter),
            flute_count: 2,
            material: None,
            centre_cutting: None,
        }
        .profile()
        .unwrap()
    }

    fn run(
        family: &Plane,
        spacing: f64,
        settings: Settings,
        limits: Limits,
    ) -> (Outcome, Vec<f64>) {
        let tool = ball(4.0);
        let tracks: Vec<Track> = [0.0, spacing]
            .iter()
            .map(|&c| Track::new(family, c, vec![family.pass(c, (0.0, family.length))]))
            .collect();
        let intervals = vec![Interval {
            lo: 0,
            hi: 1,
            spans: vec![(0.0, family.length)],
            level: 0,
        }];
        let cancel = CancelToken::new();
        let context = JobContext::new(&cancel, &NoProgress);
        let outcome = refine(
            family, tracks, intervals, &tool, &settings, &limits, 0, &context,
        )
        .unwrap();
        let mut cs: Vec<f64> = outcome.tracks.iter().map(|t| t.c).collect();
        cs.sort_by(f64::total_cmp);
        (outcome, cs)
    }

    /// The cusp a ball of radius `r` leaves between passes `w` apart on a
    /// plane.
    fn cusp(r: f64, w: f64) -> f64 {
        r - (r * r - 0.25 * w * w).sqrt()
    }

    const SETTINGS: Settings = Settings {
        height: 0.01,
        min_spacing: 0.01,
    };

    #[test]
    fn req_op_04_passes_are_added_until_the_cusp_is_within_the_allowed_excess() {
        let plane = Plane::new();
        let (outcome, cs) = run(&plane, 2.0, SETTINGS, Limits::OWNER);
        assert!(outcome.places.is_empty(), "{:?}", outcome.places);
        let widest = cs.windows(2).map(|w| w[1] - w[0]).fold(0.0, f64::max);
        assert!(cusp(2.0, widest) <= 0.01 + 1e-9, "widest {widest}");
        // Halving from 2 mm: 0.5 mm leaves 0.0157, 0.25 mm leaves 0.0039.
        assert!((widest - 0.25).abs() < 1e-12, "widest {widest}");
        assert_eq!(cs.len(), 9);
        assert_eq!(outcome.smallest, Some(0.25));
        assert_eq!(outcome.added(), 7);
    }

    #[test]
    fn req_op_04_passes_close_enough_already_get_none() {
        let plane = Plane::new();
        let (outcome, cs) = run(&plane, 0.25, SETTINGS, Limits::OWNER);
        assert_eq!(cs.len(), 2);
        assert_eq!(outcome.added(), 0);
        assert!(outcome.note(&SETTINGS).contains("no passes were added"));
        assert!(
            outcome
                .warning(&SETTINGS, &Limits::OWNER, "scallop")
                .is_none()
        );
    }

    #[test]
    fn req_op_04_never_closer_than_the_smallest_spacing_and_said() {
        let plane = Plane::new();
        let settings = Settings {
            height: 0.001,
            min_spacing: 0.3,
        };
        let (outcome, cs) = run(&plane, 2.0, settings, Limits::OWNER);
        let closest = cs.windows(2).map(|w| w[1] - w[0]).fold(f64::MAX, f64::min);
        assert!(closest >= 0.3, "closest {closest}");
        assert!(
            outcome.places.iter().all(|p| p.reason == Reason::Spacing),
            "{:?}",
            outcome.places
        );
        let warning = outcome
            .warning(&settings, &Limits::OWNER, "scallop")
            .unwrap();
        assert!(warning.message.contains("smallest spacing of 0.300 mm"));
        assert!(warning.message.contains("not complete"));
        assert_eq!(warning.parameter.as_deref(), Some("scallop"));
    }

    #[test]
    fn req_op_04_req_nfr_07_each_limit_stops_the_work_and_is_said() {
        let cases = [
            (
                Limits {
                    queries: 200,
                    ..Limits::OWNER
                },
                Reason::Queries,
            ),
            (
                Limits {
                    passes: 2,
                    ..Limits::OWNER
                },
                Reason::Passes,
            ),
            (
                Limits {
                    points: 60,
                    ..Limits::OWNER
                },
                Reason::Points,
            ),
            (
                Limits {
                    levels: 1,
                    ..Limits::OWNER
                },
                Reason::Levels,
            ),
            (
                Limits {
                    intervals: 1,
                    ..Limits::OWNER
                },
                Reason::Intervals,
            ),
        ];
        for (limits, reason) in cases {
            let plane = Plane::new();
            let (outcome, _) = run(&plane, 2.0, SETTINGS, limits);
            assert!(
                outcome.places.iter().any(|p| p.reason == reason),
                "{reason:?}: {:?}",
                outcome.places
            );
            assert!(outcome.added() as u64 <= limits.passes);
            let warning = outcome.warning(&SETTINGS, &limits, "scallop").unwrap();
            assert!(
                warning.message.contains(&reason.words(&limits, &SETTINGS)),
                "{}",
                warning.message
            );
        }
    }

    #[test]
    fn req_op_04_req_nfr_07_a_limit_the_work_fits_within_is_not_met() {
        // Issue #462: the measuring of an interval was refused whole when
        // the most its stations could make would pass the limit, though
        // they make far fewer. With the limit set to what the whole work
        // makes and the most of one more station, since each is charged
        // its most before it is measured (decision record 0239 item 8), the
        // work is done and nothing is reported.
        let (free, cs) = run(&Plane::new(), 2.0, SETTINGS, Limits::OWNER);
        assert!(free.places.is_empty(), "{:?}", free.places);
        let one = MOST_SAMPLES as u64 - 1;
        let limits = Limits {
            queries: free.queries + one,
            ..Limits::OWNER
        };
        let (tight, tight_cs) = run(&Plane::new(), 2.0, SETTINGS, limits);
        assert!(tight.places.is_empty(), "{:?}", tight.places);
        assert_eq!(tight_cs, cs);
        assert_eq!(tight.queries, free.queries);
        // Half of it, and the limit is met and said.
        let short = Limits {
            queries: free.queries / 2,
            ..Limits::OWNER
        };
        let (cut, _) = run(&Plane::new(), 2.0, SETTINGS, short);
        assert!(cut.places.iter().any(|p| p.reason == Reason::Queries));
    }

    #[test]
    fn req_op_04_req_nfr_07_measuring_makes_no_more_queries_than_it_charges() {
        // Each station is charged at most MOST_SAMPLES - 1 drops (decision
        // record 0239 item 8). Doubling the samples keeps the ones it has,
        // so a station that doubles to the most makes exactly that many,
        // and a limit of exactly that is not passed.
        let plane = Plane {
            length: 0.0,
            step: Some(0.45),
            ..Plane::new()
        };
        let stations = 2;
        let most = stations * (MOST_SAMPLES as u64 - 1);
        let limits = Limits {
            queries: most,
            ..Limits::OWNER
        };
        let (outcome, cs) = run(&plane, 1.0, SETTINGS, limits);
        assert_eq!(outcome.queries, most);
        assert_eq!(cs, vec![0.0, 1.0]);
        assert!(
            outcome
                .places
                .iter()
                .all(|p| p.reason == Reason::Unverified),
            "{:?}",
            outcome.places
        );
        assert!(!outcome.places.is_empty());
    }

    #[test]
    fn req_nfr_06_the_same_work_gives_the_same_passes() {
        let a = run(&Plane::new(), 2.0, SETTINGS, Limits::OWNER);
        let b = run(&Plane::new(), 2.0, SETTINGS, Limits::OWNER);
        assert_eq!(a.1, b.1);
        assert_eq!(a.0.queries, b.0.queries);
    }

    #[test]
    fn req_nfr_07_a_tiny_tool_gets_a_pose_step_within_its_radius() {
        // A tool with a cutting radius under 0.04 mm must not panic (NFR-07):
        // the step is never below 0.02 mm unless the radius is too small for
        // it, and never above half the radius.
        for r in [0.005, 0.01, 0.039, 0.04, 2.0] {
            for h in [0.0, 0.01, 0.5] {
                let step = pose_step(r, h);
                assert!(step > 0.0 && step <= 0.5 * r + 1e-12, "r {r} h {h}: {step}");
                assert!(
                    step >= (0.02_f64).min(0.5 * r) - 1e-12,
                    "r {r} h {h}: {step}"
                );
            }
        }
    }

    #[test]
    fn req_nfr_07_a_height_or_spacing_out_of_range_is_refused() {
        let schema = ParameterSchema {
            parameters: vec![ParameterSpec {
                name: "scallop".to_owned(),
                label: String::new(),
                description: String::new(),
                kind: ParameterKind::Number {
                    unit: Unit::Millimetres,
                    min: None,
                    min_exclusive: false,
                    max: None,
                    default: Some(0.0),
                },
            }]
            .into_iter()
            .chain(specs())
            .collect(),
        };
        let read = |height: f64, spacing: f64| {
            let values = BTreeMap::from([
                ("scallop".to_owned(), ParameterValue::Number(height)),
                (
                    parameter::MIN_SPACING.to_owned(),
                    ParameterValue::Number(spacing),
                ),
            ]);
            // The schema refuses what it can; the rest is refused when read.
            schema
                .resolve(&values)
                .and_then(|p| Settings::read(&p, "scallop"))
        };
        assert!(read(-0.1, 0.01).is_err());
        assert!(read(f64::NAN, 0.01).is_err());
        assert!(read(0.01, 0.0).is_err());
        assert!(read(0.01, f64::INFINITY).is_err());
        assert_eq!(
            read(0.0, 0.01).unwrap(),
            Some(Settings {
                height: 0.0,
                min_spacing: 0.01
            })
        );
    }

    #[test]
    fn req_op_04_the_bound_between_samples_follows_their_slope() {
        // A flat top is bounded by itself; a peak between samples is not
        // hidden by them.
        assert!((sloped_bound(&[Some(0.005), Some(0.005), Some(0.005)]) - 0.005).abs() < 1e-12);
        assert!(sloped_bound(&[Some(0.0), Some(0.01), Some(0.01), Some(0.0)]) > 0.01);
        assert!((sloped_bound(&[Some(0.002), None, Some(0.004)]) - 0.004).abs() < 1e-12);
    }

    #[test]
    fn req_op_04_merged_stretches_stay_within_the_spans_and_keep_their_direction() {
        assert_eq!(
            merge(&[1.0, 2.0, 8.0], 0.75, &[(0.0, 10.0)]),
            vec![(0.25, 2.75), (7.25, 8.75)]
        );
        assert_eq!(merge(&[0.5], 1.0, &[(5.0, 0.0)]), vec![(1.5, 0.0)]);
        assert!(merge(&[20.0], 1.0, &[(0.0, 10.0)]).is_empty());
    }
}
