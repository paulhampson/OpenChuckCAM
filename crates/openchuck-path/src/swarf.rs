// SPDX-License-Identifier: AGPL-3.0-or-later
//! Swarf (flank) machining of ruled B-rep faces (OP-07; work breakdown
//! slice 17; spike SP-11, decision record 0021; decision record 0061).
//!
//! The side of the tool is laid along the straight lines (the *rulings*) of
//! a ruled face, and the tool is moved along the face with the flank in
//! contact. Only an exact face has rulings to read, so the strategy works on
//! the faces a [`SurfaceSource`](crate::SurfaceSource) reports through
//! [`exact_faces`](crate::SurfaceSource::exact_faces) (a B-rep model, or a
//! selection of its faces); a mesh is refused (decision 0021, OI-96).
//!
//! 1. **Detection** ([`detect_rulings`]). For each family of iso-lines of
//!    the face a straight line is fitted to the samples of every line and the
//!    worst distance of a sample from its line is the family's *residual*. A
//!    family whose residual is within the operation's tolerance is ruled.
//!    When both are, the family whose rulings lie closer to perpendicular to
//!    the rotary axis is used (a plane is ruled both ways, and only one way
//!    may be cut on four axes), then `v`. A face ruled in neither family is
//!    refused with the residual and the limit (decision 0021 item 2). The
//!    angle the surface normal turns along a ruling (the *twist*) is
//!    measured too: it is zero on a developable face and is the early
//!    warning for a face that is ruled but cannot be cut with a straight
//!    flank (decision 0021 item 3).
//! 2. **The flank** ([`Flank`]) is the straight part of the tool's profile
//!    that reaches the cutting radius: the side of a flat or ball end, the
//!    taper of a tapered ball, the V of a V-bit, as one cone with a half
//!    angle (decision 0021 item 5). The contact sits on it, above a ball or
//!    corner radius, and a ruling longer than the flank is refused (OI-97).
//! 3. **Placement** (decision 0021 item 4). Along each ruling with direction
//!    `d` (towards the spindle) and outward normal `n` at its middle, the
//!    tool axis a five-axis machine would use is `cos(a) d + sin(a) n` for a
//!    flank half angle `a`, and the axis passes `r` from the contact, `r`
//!    the flank radius at the contact height. Four axes can only hold the
//!    axis perpendicular to the rotary axis, so that axis is projected into
//!    the plane perpendicular to it and A is the angle at which the
//!    projection is the machine's vertical ([`RotaryMapping::a_facing`]);
//!    the angle between the two is the pose's *tilt*. Rulings along the
//!    rotary axis have no such projection and are refused (OI-94). The end
//!    of the ruling the tool tip sits at is chosen once per face (the end
//!    nearer the rotary axis, by default) so A never jumps by half a turn,
//!    and the tip sits `tip_extension` beyond that end.
//! 4. **Pose spacing** (decision 0021 item 6). The machine moves straight
//!    in X, Y, Z and A between poses. A move is halved until the tool at its
//!    middle keeps the ruling there within [`CHORD_FACTOR`] times the
//!    tolerance of its flank, as the other continuous strategies do, and
//!    until A turns no more than [`MAX_ANGLE_STEP`] in it.
//! 5. **The gate** (decision 0021 item 3; OP-12). After placement the face
//!    is sampled across and along its rulings and each sample is measured
//!    against the union of the tool at every pose and at points of the
//!    machine's motion between poses. A face the tool cuts into by more
//!    than the tolerance (beyond the stock to leave) is still cut, with a
//!    warning naming the depth and the place, the twist and the tilt, the
//!    tool taper that would remove the tilt where one exists, and the
//!    tolerance to raise to accept it (decision record 0175, amending
//!    decision record 0061 item 4). Material left beyond the
//!    allowance is silent within the tolerance, a note up to
//!    `max_material_left`, and beyond that a refusal naming the amount,
//!    where, and what to change (decision record 0061 item 7). The limit
//!    is set per operation; zero takes [`DEFAULT_MAX_MATERIAL_LEFT`]
//!    ([`Settings::material_left_limit`]).
//!
//! Everything here is deterministic (NFR-06): the only parallel work is the
//! gate's sampling, through [`crate::parallel`].

use std::f64::consts::{FRAC_PI_2, PI};

use openchuck_types::geometry::{Point3, UnitVec3, Vec3};
use openchuck_types::job::JobContext;
use openchuck_types::tool::profile::{ProfilePiece, RevolvedProfile};
use openchuck_types::units::Degrees;

use crate::assemble::PathWarning;
use crate::error::{PathError, Result};
use crate::parallel;
use crate::params::{ParameterKind, ParameterSchema, ParameterSpec, Parameters, Unit, names};
use crate::region::{BoundaryLimits, Protection, RegionConstraint, Window};
use crate::rotary::{AxisPosition, RotaryMapping, nearest_turn};
use crate::strategy::{CutPoint, Pass, Strategy, StrategyGroup, StrategyInput, StrategyOutput};
use crate::surface::{FaceRef, ParametricFace, SurfaceSource, Uv};

pub use crate::wrap_fit::CHORD_FACTOR;

/// The registry name of [`Swarf`].
pub const NAME: &str = "swarf";

/// Parameter names specific to swarf.
pub mod parameter {
    /// How far the tool tip reaches beyond the end of the ruling it sits at.
    pub const TIP_EXTENSION: &str = "tip_extension";
    /// The most material the flank may leave beyond the stock to leave.
    pub const MAX_MATERIAL_LEFT: &str = "max_material_left";
    /// Which end of the rulings the tool tip sits at.
    pub const TIP_END: &str = "tip_end";
    /// Which way along the face the tool moves.
    pub const DIRECTION: &str = "direction";
}

/// The choices of `tip_end`.
const AUTO: &str = "auto";
const REVERSED: &str = "reversed";
/// The choices of `direction`.
const FORWARD: &str = "forward";
const REVERSE: &str = "reverse";

/// Iso-lines sampled per family by [`detect_rulings`].
pub const DETECTION_LINES: usize = 17;
/// Samples per iso-line in [`detect_rulings`].
pub const DETECTION_SAMPLES: usize = 33;
/// Samples along an iso-line when its extent on the trimmed face is found.
const EXTENT_SAMPLES: usize = 33;
/// Samples along a ruling in the mid-move check.
const CHECK_SAMPLES: usize = 17;
/// Samples along a ruling in the gate.
const GATE_SAMPLES: usize = 25;
/// Samples along a ruling that find where it leaves a boundary.
const BOUNDARY_SAMPLES: usize = 33;
/// Samples across each parameter of an avoid or check face.
const GUARD_GRID: usize = 33;
/// Bisections that find where the tool comes too close to an avoid or check
/// surface.
const GUARD_STEPS: u32 = 16;
/// Points of the machine's motion between two poses the gate looks at.
const GATE_STEPS: usize = 4;
/// Samples of the directrix that find where the face has rulings.
const SCAN: usize = 64;
/// Bisections that refine an extent or a run end.
const EDGE_STEPS: u32 = 30;
/// Power iterations of the line fit.
const FIT_ITERATIONS: usize = 60;
/// How many times a move may be halved.
pub const MAX_DEPTH: u32 = 12;
/// The most A turns in one move, in degrees: a move that is exact in the
/// machine's kinematics (a helicoid swept by A) is still cut in steps a
/// machine and a simulation can follow, as the other continuous strategies
/// do.
pub const MAX_ANGLE_STEP: f64 = 5.0;
/// Most poses one face may take (NFR-07).
const MAX_POSES: usize = 20_000;
/// Needed tapers closer than this, in degrees, are one taper.
const SAME_TAPER: f64 = 0.05;
/// The `max_material_left` a zero setting takes, in millimetres, on every
/// pass: ten times the default tolerance, a step a finish cut shows and the
/// simulation's finest setting resolves (verification plan 4.1, 0.02 mm).
/// One limit, configured per operation, is the owner's choice over a rule by
/// the pass's role (decision record 0061 item 7, revised).
pub const DEFAULT_MAX_MATERIAL_LEFT: f64 = 0.1;
/// Below this tilt, in degrees, the four axes hold the ideal axis.
const NO_TILT: f64 = 1e-3;
/// Two points closer than this, in millimetres, are the same.
const SAME_POINT: f64 = 1e-7;

/// Swarf machining of the selected B-rep faces (OP-07).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Swarf;

// ---------------------------------------------------------------------------
// Detection.

/// Which parameter runs along the rulings of a face.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    /// The rulings are the lines of constant `v`, running in `u`.
    AlongU,
    /// The rulings are the lines of constant `u`, running in `v`.
    AlongV,
}

impl Family {
    /// The parameter name, `u` or `v`.
    pub const fn name(self) -> &'static str {
        match self {
            Self::AlongU => "u",
            Self::AlongV => "v",
        }
    }

    /// The parameters of the point at `along` on the iso-line at `fixed`.
    fn uv(self, fixed: f64, along: f64) -> Uv {
        match self {
            Self::AlongU => Uv { u: along, v: fixed },
            Self::AlongV => Uv { u: fixed, v: along },
        }
    }

    /// The parameter ranges `(fixed, along)` of the family on `face`.
    fn ranges(self, face: &dyn ParametricFace) -> ((f64, f64), (f64, f64)) {
        let (lo, hi) = face.uv_bounds();
        match self {
            Self::AlongU => ((lo.v, hi.v), (lo.u, hi.u)),
            Self::AlongV => ((lo.u, hi.u), (lo.v, hi.v)),
        }
    }
}

/// How well one family of iso-lines fits straight lines.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FamilyFit {
    /// The family.
    pub family: Family,
    /// The worst distance, in millimetres, of a sample from the line fitted
    /// to its iso-line.
    pub residual: f64,
    /// The largest angle, in degrees, the normal turns between the middle of
    /// an iso-line and either end: zero on a developable face.
    pub twist: f64,
    /// The largest component along the rotary axis of a unit fitted line
    /// (1 for a line along the axis, 0 for one perpendicular to it).
    pub along_axis: f64,
}

/// The result of [`detect_rulings`].
#[derive(Debug, Clone, PartialEq)]
pub struct Detection {
    /// The fit of the lines of constant `v`.
    pub u: FamilyFit,
    /// The fit of the lines of constant `u`.
    pub v: FamilyFit,
    /// The family used, when one fits within the tolerance.
    pub chosen: Option<Family>,
    /// The tolerance the fit was judged against, in millimetres.
    pub tolerance: f64,
}

impl Detection {
    /// The fit of `family`.
    pub fn fit(&self, family: Family) -> FamilyFit {
        match family {
            Family::AlongU => self.u,
            Family::AlongV => self.v,
        }
    }

    /// The better fit: the smaller residual, `v` on a tie.
    pub fn best(&self) -> FamilyFit {
        if self.u.residual < self.v.residual {
            self.u
        } else {
            self.v
        }
    }

    /// Why the face is refused, when no family fits (slice 17 item 4).
    pub fn refusal(&self) -> Option<String> {
        if self.chosen.is_some() {
            return None;
        }
        let b = self.best();
        Some(format!(
            "it is not a ruled surface within the tolerance: the best straight-line fit of its \
             iso-lines misses by {:.4} mm (limit {:.4} mm)",
            b.residual, self.tolerance
        ))
    }
}

/// The best-fit line through `points`: the centroid and a unit direction
/// (the principal axis of the scatter, by a fixed number of power iterations
/// started from the end-to-end chord, so the answer is deterministic and the
/// direction runs from the first point towards the last).
fn fit_line(points: &[Point3]) -> Option<(Point3, Vec3)> {
    let (first, last) = (points.first()?, points.last()?);
    let n = points.len() as f64;
    let mut c = Vec3::ZERO;
    for p in points {
        c = c + p.to_vec();
    }
    let centroid = Point3::ORIGIN + c * (1.0 / n);
    let mut m = [[0.0f64; 3]; 3];
    for p in points {
        let d = *p - centroid;
        let a = [d.x, d.y, d.z];
        for (row, ai) in m.iter_mut().zip(a) {
            for (cell, aj) in row.iter_mut().zip(a) {
                *cell += ai * aj;
            }
        }
    }
    // The seed: the end-to-end chord, or, for a closed line (a circle comes
    // back to its start), the chord to the middle sample, or else X.
    let middle = points.get(points.len() / 2)?;
    let chord = UnitVec3::new(*last - *first)
        .or_else(|_| UnitVec3::new(*middle - *first))
        .map_or(Vec3::new(1.0, 0.0, 0.0), UnitVec3::get);
    let mut d = chord;
    for _ in 0..FIT_ITERATIONS {
        let w = Vec3::new(
            m[0][0] * d.x + m[0][1] * d.y + m[0][2] * d.z,
            m[1][0] * d.x + m[1][1] * d.y + m[1][2] * d.z,
            m[2][0] * d.x + m[2][1] * d.y + m[2][2] * d.z,
        );
        match UnitVec3::new(w) {
            Ok(unit) => d = unit.get(),
            Err(_) => break,
        }
    }
    if d.dot(chord) < 0.0 {
        d = -d;
    }
    Some((centroid, d))
}

fn distance_to_line(p: Point3, c: Point3, d: Vec3) -> f64 {
    let w = p - c;
    (w - d * w.dot(d)).length()
}

/// The angle between two unit vectors, in degrees, well conditioned near
/// zero (an `acos` of a dot product is not).
fn angle_between(a: Vec3, b: Vec3) -> f64 {
    a.cross(b).length().atan2(a.dot(b)).to_degrees()
}

/// The fit of one family on `face`.
fn fit_family(face: &dyn ParametricFace, family: Family) -> FamilyFit {
    let ((f0, f1), (t0, t1)) = family.ranges(face);
    let mut residual = 0.0f64;
    let mut twist = 0.0f64;
    let mut along_axis = 0.0f64;
    for i in 0..DETECTION_LINES {
        let fixed = f0 + (f1 - f0) * (i as f64 + 0.5) / DETECTION_LINES as f64;
        let samples: Vec<_> = (0..DETECTION_SAMPLES)
            .filter_map(|k| {
                let along = t0 + (t1 - t0) * k as f64 / (DETECTION_SAMPLES - 1) as f64;
                face.point_normal(family.uv(fixed, along))
            })
            .collect();
        if samples.len() < 3 {
            continue;
        }
        let points: Vec<Point3> = samples.iter().map(|s| s.point).collect();
        let Some((c, d)) = fit_line(&points) else {
            continue;
        };
        for p in &points {
            residual = residual.max(distance_to_line(*p, c, d));
        }
        along_axis = along_axis.max(d.x.abs());
        let mid = samples.len() / 2;
        if let (Some(a), Some(m), Some(b)) = (samples.first(), samples.get(mid), samples.last()) {
            twist = twist
                .max(angle_between(a.normal.get(), m.normal.get()))
                .max(angle_between(b.normal.get(), m.normal.get()));
        }
    }
    FamilyFit {
        family,
        residual,
        twist,
        along_axis,
    }
}

/// Fits rulings on `face` (decision record 0021 item 2): the family whose
/// iso-lines are straight within `tolerance` millimetres, preferring, when
/// both are, the one whose rulings lie closer to perpendicular to the rotary
/// axis (the model X axis), then `v`.
pub fn detect_rulings(face: &dyn ParametricFace, tolerance: f64) -> Detection {
    let u = fit_family(face, Family::AlongU);
    let v = fit_family(face, Family::AlongV);
    let fits = |f: &FamilyFit| f.residual <= tolerance;
    let chosen = match (fits(&u), fits(&v)) {
        (true, true) => Some(if u.along_axis < v.along_axis - 1e-9 {
            Family::AlongU
        } else {
            Family::AlongV
        }),
        (true, false) => Some(Family::AlongU),
        (false, true) => Some(Family::AlongV),
        (false, false) => None,
    };
    Detection {
        u,
        v,
        chosen,
        tolerance,
    }
}

// ---------------------------------------------------------------------------
// The flank.

/// The straight part of a tool's profile the swarf contact sits on: one cone
/// about the tool axis (decision record 0021 item 5), from the height where
/// a ball or corner radius ends to the end of the profile.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Flank {
    /// The half angle of the cone, in radians: zero for a straight side.
    pub half_angle: f64,
    /// The radius where the flank starts.
    pub r_start: f64,
    /// The height above the tip where the flank starts.
    pub z_start: f64,
    /// The radius where the flank ends (the cutting radius).
    pub r_end: f64,
    /// The height above the tip where the flank ends.
    pub z_end: f64,
}

impl Flank {
    /// The flank of `profile`: the first straight piece that reaches the
    /// cutting radius and rises along the tool, or the straight side above
    /// the piece that reaches it. Fails for a profile with neither (a ball
    /// end whose flute length is its radius, a plain flat).
    pub fn of(profile: &RevolvedProfile) -> Result<Self> {
        let r = profile.cutting_radius();
        let rises = |start: openchuck_types::tool::ProfilePoint,
                     end: openchuck_types::tool::ProfilePoint| {
            end.height.0 - start.height.0 > 1e-9
        };
        let mut reached = false;
        for piece in profile.pieces() {
            let (start, end) = (piece.start(), piece.end());
            let reaches = end.radius.0 >= r - 1e-9;
            if let ProfilePiece::Line { .. } = piece
                && (reaches || reached)
                && rises(start, end)
            {
                let (dr, dz) = (end.radius.0 - start.radius.0, end.height.0 - start.height.0);
                return Ok(Self {
                    half_angle: dr.atan2(dz),
                    r_start: start.radius.0,
                    z_start: start.height.0,
                    r_end: end.radius.0,
                    z_end: end.height.0,
                });
            }
            reached |= reaches;
        }
        Err(PathError::Strategy(
            "the tool has no straight flank to cut with: its profile has no straight piece \
             rising to the cutting radius, so swarf cannot use it"
                .to_owned(),
        ))
    }

    /// The flank's length along the tool axis.
    pub fn length(&self) -> f64 {
        self.z_end - self.z_start
    }

    /// The flank radius at height `h` above the tip.
    pub fn radius_at(&self, h: f64) -> f64 {
        self.r_start + (h - self.z_start) * self.half_angle.tan()
    }
}

// ---------------------------------------------------------------------------
// Signed distance to the tool body.

/// The signed distance from a point at distance `rho` from the tool axis and
/// height `h` above the tip to the tool's surface: negative inside the tool.
/// The body is the revolved profile, continued upward at the cutting radius
/// (the shank and holder are not known here; OI-237).
fn clearance(profile: &RevolvedProfile, rho: f64, h: f64) -> f64 {
    let r = profile.cutting_radius();
    let top = profile.top();
    let inside = rho <= r + 1e-12 && profile.height_at(rho.min(r)).is_some_and(|z| h >= z);
    let mut d = f64::INFINITY;
    for piece in profile.pieces() {
        d = d.min(distance_to_piece(piece, rho, h));
    }
    // The side above the profile's end.
    let dz = (top - h).max(0.0);
    d = d.min(((rho - r).powi(2) + dz * dz).sqrt());
    if inside { -d } else { d }
}

fn distance_to_piece(piece: &ProfilePiece, rho: f64, h: f64) -> f64 {
    match *piece {
        ProfilePiece::Line { start, end } => {
            let (ar, az) = (start.radius.0, start.height.0);
            let (dr, dz) = (end.radius.0 - ar, end.height.0 - az);
            let l2 = dr * dr + dz * dz;
            let t = if l2 > 0.0 {
                (((rho - ar) * dr + (h - az) * dz) / l2).clamp(0.0, 1.0)
            } else {
                0.0
            };
            (rho - ar - t * dr).hypot(h - az - t * dz)
        }
        ProfilePiece::Arc {
            start,
            end,
            centre,
            radius,
            ..
        } => {
            let (cr, cz) = (centre.radius.0, centre.height.0);
            let angle = |r: f64, z: f64| {
                let t = (z - cz).atan2(r - cr);
                if t < -FRAC_PI_2 { t + 2.0 * PI } else { t }
            };
            let (ta, tb) = (
                angle(start.radius.0, start.height.0),
                angle(end.radius.0, end.height.0),
            );
            let tq = angle(rho, h);
            let ends = (rho - start.radius.0)
                .hypot(h - start.height.0)
                .min((rho - end.radius.0).hypot(h - end.height.0));
            if tq >= ta.min(tb) && tq <= ta.max(tb) {
                (radius - (rho - cr).hypot(h - cz)).abs().min(ends)
            } else {
                ends
            }
        }
    }
}

/// `clearance` for a model point against the tool at `tip` along `axis`.
fn clearance_at(profile: &RevolvedProfile, tip: Point3, axis: UnitVec3, p: Point3) -> f64 {
    let w = p - tip;
    let h = w.dot(axis.get());
    let rho = (w - axis.get() * h).length();
    clearance(profile, rho, h)
}

// ---------------------------------------------------------------------------
// Rulings and poses.

/// One ruling of the face: the straight iso-line at `t`, between the ends
/// of its extent on the trimmed face.
#[derive(Debug, Clone, Copy)]
struct Ruling {
    t: f64,
    /// Parameter range of the extent along the ruling.
    s: (f64, f64),
    lo: Point3,
    hi: Point3,
    /// The outward normal at the middle of the extent.
    normal: UnitVec3,
    /// The angle the normal turns between the middle and either end.
    twist: f64,
}

impl Ruling {
    fn length(&self) -> f64 {
        (self.hi - self.lo).length()
    }

    fn middle(&self) -> Point3 {
        self.lo + (self.hi - self.lo) * 0.5
    }
}

/// Where the tool sits for one ruling.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pose {
    /// The parameter of the ruling along the face.
    pub t: f64,
    /// The tool tip, in the model frame.
    pub tip: Point3,
    /// The tool axis used, perpendicular to the rotary axis, in the model
    /// frame.
    pub axis: UnitVec3,
    /// The rotary angle, continuous along the pass.
    pub a: Degrees,
    /// The middle of the engaged ruling, on the model surface: the cutter
    /// contact point the feed applies to (OP-13).
    pub contact: Point3,
    /// The angle, in degrees, between the axis a five-axis machine would use
    /// and `axis`.
    pub tilt: f64,
    /// The flank half angle, in degrees, that would make the tilt zero, when
    /// one would.
    pub needed_taper: Option<f64>,
    /// The engaged length of the ruling, in millimetres.
    pub ruling_length: f64,
}

/// What a face's rulings need from the tool and the operation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Settings {
    /// The toolpath tolerance, in millimetres (NFR-05).
    pub tolerance: f64,
    /// The stock to leave on the face, in millimetres (OP-11).
    pub stock_to_leave: f64,
    /// How far the tip reaches beyond the end of the ruling it sits at.
    pub tip_extension: f64,
    /// The most material the flank may leave beyond the stock to leave
    /// before the face is refused; less, but more than the tolerance, is a
    /// note. Zero takes [`DEFAULT_MAX_MATERIAL_LEFT`]
    /// ([`Settings::material_left_limit`]).
    pub max_material_left: f64,
    /// Put the tip at the other end of the rulings than the rule chooses.
    pub reverse_tip_end: bool,
    /// Cut the face in decreasing parameter.
    pub reverse_direction: bool,
}

impl Settings {
    /// The limit on material left that applies (decision record 0061 item
    /// 7): `max_material_left` when set, else [`DEFAULT_MAX_MATERIAL_LEFT`],
    /// the same on every pass whatever its stock to leave.
    pub fn material_left_limit(&self) -> f64 {
        if self.max_material_left > 0.0 {
            self.max_material_left
        } else {
            DEFAULT_MAX_MATERIAL_LEFT
        }
    }
}

/// The swarf toolpath of one face, with what the gate measured.
#[derive(Debug, Clone, PartialEq)]
pub struct FacePath {
    /// The face's number.
    pub number: u32,
    /// The fit of the rulings.
    pub detection: Detection,
    /// The family the rulings are.
    pub family: Family,
    /// The runs of poses, each cut without lifting the tool. A closed face
    /// gives one run whose last pose repeats its first.
    pub runs: Vec<Vec<Pose>>,
    /// Whether the single run closes on itself.
    pub closed: bool,
    /// The deepest the tool cuts beyond the stock to leave, in millimetres.
    pub gouge: f64,
    /// Where it does.
    pub gouge_at: Point3,
    /// The most material left beyond the stock to leave, in millimetres.
    pub left: f64,
    /// Where it is.
    pub left_at: Point3,
    /// The largest twist of the normal along a ruling, in degrees.
    pub twist: f64,
    /// The largest tilt of any pose, in degrees.
    pub max_tilt: f64,
    /// One flank half angle, in degrees, that would remove the tilt on every
    /// pose, when there is one and the tilt is not already zero.
    pub suggested_taper: Option<f64>,
    /// What the boundary and the avoid and check surfaces left uncut.
    pub limits: Limits,
}

/// What the limits of an operation did to one face (decision record 0176),
/// so its note can say which limit cut the pass short.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Limits {
    /// When a boundary applies, which of its limits are on: the pass (limit
    /// A) and the tip (limit B).
    pub boundary: Option<(bool, bool)>,
    /// The length along the face, in millimetres, whose rulings the
    /// boundary leaves nothing of, or (with the pass limited) leave it.
    pub outside_boundary: f64,
    /// The most the boundary raised the tool tip along a ruling, in
    /// millimetres.
    pub tip_raised: f64,
    /// The length along the face left uncut because the tool would cut into
    /// an avoid surface there, in millimetres.
    pub avoid_uncut: f64,
    /// The length along the face left uncut because the tool would touch a
    /// check surface there, in millimetres.
    pub check_uncut: f64,
    /// Each watched surface of the region (only measured, decision record
    /// 0176) that the tool breaks on this face, in the region's order.
    pub watched: Vec<WatchedBreak>,
}

/// What the tool does to a surface of this kind that it breaks, for notes
/// and warnings.
fn breaks(protection: Protection) -> &'static str {
    match protection {
        Protection::Avoid => "cuts into the avoid set",
        Protection::Check => "comes closer than the stock to leave to the check set",
    }
}

/// A watched avoid or check surface the tool breaks on a face (decision
/// record 0176): the path is kept, and the operation warns.
#[derive(Debug, Clone, PartialEq)]
pub struct WatchedBreak {
    /// The surface's place in the region's watched surfaces.
    pub index: usize,
    /// The surface's name, as the region gives it.
    pub name: String,
    /// Whether it is an avoid or a check surface.
    pub protection: Protection,
    /// The length along the face, in millimetres, over which the tool cuts
    /// into it (avoid) or comes closer than the stock to leave (check).
    pub length: f64,
}

impl Limits {
    /// The sentences of a face's note that say what each limit did.
    fn describe(&self) -> String {
        let mut out = String::new();
        if let Some((pass, tip)) = self.boundary {
            if pass {
                if self.outside_boundary > SAME_POINT {
                    out.push_str(&format!(
                        " The boundary limits the pass: about {:.3} mm along the face is not \
                         cut because its rulings leave the boundary there.",
                        self.outside_boundary
                    ));
                } else {
                    out.push_str(" The boundary limits the pass, and every ruling lies inside it.");
                }
            } else if self.outside_boundary > SAME_POINT {
                out.push_str(&format!(
                    " About {:.3} mm along the face is not cut because no part of its rulings \
                     lies inside the boundary.",
                    self.outside_boundary
                ));
            }
            if tip {
                if self.tip_raised > SAME_POINT {
                    out.push_str(&format!(
                        " The boundary limits the cut at the tool tip: the tip is raised by up \
                         to {:.3} mm along the rulings, on top of the tip extension, so the \
                         bottom of the flank stops at the boundary.",
                        self.tip_raised
                    ));
                }
                out.push_str(
                    " The far end of the rulings is not limited by the boundary: the side of \
                     the tool cuts each ruling up to that end.",
                );
            }
        }
        if self.avoid_uncut > SAME_POINT {
            out.push_str(&format!(
                " About {:.3} mm along the face is left uncut where the tool would cut into an \
                 avoid surface; the pass stops there and starts again after it.",
                self.avoid_uncut
            ));
        }
        if self.check_uncut > SAME_POINT {
            out.push_str(&format!(
                " About {:.3} mm along the face is left uncut where the tool would touch a \
                 check surface; the pass stops there and starts again after it.",
                self.check_uncut
            ));
        }
        for w in &self.watched {
            out.push_str(&format!(
                " Along about {:.3} mm of the face the tool {} '{}', which only warns, so \
                 the path is kept there.",
                w.length,
                breaks(w.protection),
                w.name
            ));
        }
        out
    }
}

impl FacePath {
    /// Every pose, in cutting order.
    pub fn poses(&self) -> impl Iterator<Item = &Pose> {
        self.runs.iter().flatten()
    }

    fn describe_tilt(&self) -> String {
        if self.max_tilt <= NO_TILT {
            return String::new();
        }
        match self.suggested_taper {
            Some(a) => format!(
                " The four-axis tilt reaches {:.2} degrees; a tool with a flank half angle of \
                 {a:.2} degrees would remove it.",
                self.max_tilt
            ),
            None => format!(
                " The four-axis tilt reaches {:.2} degrees, and no single tool taper removes it.",
                self.max_tilt
            ),
        }
    }

    fn describe_twist(&self) -> String {
        if self.twist > 0.01 {
            format!(
                " The face is ruled but not developable: it is made of straight lines, but its \
                 surface turns {:.2} degrees along each of them, so the straight side of the tool \
                 cannot lie flat on it everywhere; a smaller tool radius deviates less.",
                self.twist
            )
        } else {
            String::new()
        }
    }

    /// The warning for a face the tool cuts into, beyond the stock to leave,
    /// by more than `tolerance` (OP-12; decision record 0175, amending
    /// decision record 0061 item 4): the path is still made, and the
    /// warning names the depth and the place, why, and what the user can
    /// change. `None` within the tolerance.
    pub fn overcut_warning(&self, tolerance: f64) -> Option<String> {
        if self.gouge <= tolerance {
            return None;
        }
        Some(format!(
            "Swarf on face {} cuts into the finished surface by {:.4} mm at ({:.3}, {:.3}, \
             {:.3}), more than this operation's tolerance of {tolerance} mm.{}{} To accept this \
             cut, raise the Tolerance of this operation to at least {:.4} mm (in the operation \
             inspector's parameters, where the field is marked); to cut less into the face, use \
             a tool with a smaller tool radius or the taper named here, or machine this face \
             with another strategy. The program is made with this cut in it: check the program \
             and the simulation before running it.",
            self.number,
            self.gouge,
            self.gouge_at.x,
            self.gouge_at.y,
            self.gouge_at.z,
            self.describe_twist(),
            self.describe_tilt(),
            self.gouge,
        ))
    }

    /// Why the face is refused after placement (decision record 0061 item
    /// 7): the flank would leave more than `max_material_left` beyond the
    /// stock to leave. A cut into the surface deeper than the tolerance is
    /// not refused but warned of ([`overcut_warning`](Self::overcut_warning);
    /// decision record 0175).
    pub fn refusal(&self, max_material_left: f64) -> Option<String> {
        if self.left > max_material_left {
            let change = if self.max_tilt > NO_TILT || self.twist > 0.01 {
                String::new()
            } else {
                " A tighter tolerance puts the poses closer together, and a larger \
                 max_material_left accepts the step."
                    .to_owned()
            };
            return Some(format!(
                "swarf on face {} would leave {:.4} mm beyond the stock to leave at ({:.3}, \
                 {:.3}, {:.3}), more than the max_material_left of {max_material_left} \
                 mm.{}{}{change}",
                self.number,
                self.left,
                self.left_at.x,
                self.left_at.y,
                self.left_at.z,
                self.describe_twist(),
                self.describe_tilt()
            ));
        }
        None
    }

    /// What to tell the user about the face (decision record 0036 item 9):
    /// material left beyond the stock to leave is mentioned only when it is
    /// more than `tolerance` (and, for a face that is not refused, no more
    /// than `max_material_left`).
    pub fn note(&self, tolerance: f64, max_material_left: f64) -> String {
        let fit = self.detection.fit(self.family);
        let left = if self.left > tolerance {
            format!(
                " It leaves up to {:.4} mm beyond the stock to leave at ({:.3}, {:.3}, {:.3}), \
                 within the max_material_left of {max_material_left} mm.",
                self.left, self.left_at.x, self.left_at.y, self.left_at.z
            )
        } else {
            String::new()
        };
        format!(
            "swarf on face {}: rulings along {} (the straight-line fit misses by {:.4} mm, limit \
             {tolerance} mm), the normal twists {:.2} degrees along a ruling, {} poses, the tool \
             cuts at most {:.4} mm beyond the stock to leave.{}{left}{}",
            self.number,
            self.family.name(),
            fit.residual,
            self.twist,
            self.poses().count(),
            self.gouge,
            self.describe_tilt(),
            self.limits.describe()
        )
    }
}

/// The lower end of the parameter interval of a bisection between a sample
/// that is on the face (`inside`) and one that is not (`outside`).
fn refine(mut inside: f64, mut outside: f64, present: impl Fn(f64) -> bool) -> f64 {
    for _ in 0..EDGE_STEPS {
        let mid = 0.5 * (inside + outside);
        if present(mid) {
            inside = mid;
        } else {
            outside = mid;
        }
    }
    inside
}

/// The ruling at `t`, or `None` where the iso-line has no extent on the
/// trimmed face or its normal is undefined.
fn ruling_at(face: &dyn ParametricFace, family: Family, t: f64) -> Option<Ruling> {
    let (_, (s0, s1)) = family.ranges(face);
    let on_face = |s: f64| {
        let uv = family.uv(t, s);
        face.contains(uv) && face.point_normal(uv).is_some()
    };
    let at = |k: usize| s0 + (s1 - s0) * k as f64 / (EXTENT_SAMPLES - 1) as f64;
    let present: Vec<bool> = (0..EXTENT_SAMPLES).map(|k| on_face(at(k))).collect();
    let first = present.iter().position(|p| *p)?;
    let last = present.iter().rposition(|p| *p)?;
    let lo = if first == 0 {
        s0
    } else {
        refine(at(first), at(first - 1), on_face)
    };
    let hi = if last + 1 == EXTENT_SAMPLES {
        s1
    } else {
        refine(at(last), at(last + 1), on_face)
    };
    ruling_over(face, family, t, lo, hi)
}

/// The ruling at `t` between the parameters `lo` and `hi` along it, or
/// `None` when that is no length or its normal is undefined.
fn ruling_over(
    face: &dyn ParametricFace,
    family: Family,
    t: f64,
    lo: f64,
    hi: f64,
) -> Option<Ruling> {
    if hi - lo <= 0.0 {
        return None;
    }
    let mid = 0.5 * (lo + hi);
    let p_lo = face.point_normal(family.uv(t, lo))?;
    let p_hi = face.point_normal(family.uv(t, hi))?;
    let p_mid = face.point_normal(family.uv(t, mid))?;
    if p_hi.point.distance(p_lo.point) < SAME_POINT {
        return None;
    }
    let twist = angle_between(p_lo.normal.get(), p_mid.normal.get())
        .max(angle_between(p_hi.normal.get(), p_mid.normal.get()));
    Some(Ruling {
        t,
        s: (lo, hi),
        lo: p_lo.point,
        hi: p_hi.point,
        normal: p_mid.normal,
        twist,
    })
}

/// How a containment boundary limits the rulings of a face (decision record
/// 0176). `pass` (limit A) keeps only the rulings that lie inside it, whole,
/// so the pass stops or splits where they leave it; `tip` (limit B) moves
/// the tip end of each ruling to where it enters the boundary, raising the
/// tool. With both, a ruling is shortened at the tip end and then kept only
/// when the rest lies inside. The far end of a ruling cannot be limited: the
/// side of the tool covers the whole ruling up to it.
#[derive(Debug, Clone, Copy)]
struct Limiter {
    boundary: Option<Window>,
    pass: bool,
    tip: bool,
    /// Which end the tip sits at: the low end of the ruling's parameter for
    /// +1, the high end for -1.
    sign: f64,
}

impl Limiter {
    /// The boundary, when it limits anything.
    fn window(&self) -> Option<Window> {
        self.boundary.filter(|_| self.pass || self.tip)
    }

    /// The ruling at `t` as the boundary leaves it, or `None` where the
    /// boundary leaves nothing of it to cut.
    fn ruling(&self, face: &dyn ParametricFace, family: Family, t: f64) -> Option<Ruling> {
        let ruling = ruling_at(face, family, t)?;
        let Some(window) = self.window() else {
            return Some(ruling);
        };
        let inside = |s: f64| {
            face.point_normal(family.uv(t, s))
                .is_some_and(|p| window.contains(p.point))
        };
        // From the tip end towards the far end.
        let (mut tip_s, far_s) = if self.sign > 0.0 {
            (ruling.s.0, ruling.s.1)
        } else {
            (ruling.s.1, ruling.s.0)
        };
        let at =
            |tip: f64, k: usize| tip + (far_s - tip) * k as f64 / (BOUNDARY_SAMPLES - 1) as f64;
        if self.tip {
            let first = (0..BOUNDARY_SAMPLES).position(|k| inside(at(tip_s, k)))?;
            if let Some(before) = first.checked_sub(1) {
                tip_s = refine(at(tip_s, first), at(tip_s, before), inside);
            }
        }
        if self.pass && !(0..BOUNDARY_SAMPLES).all(|k| inside(at(tip_s, k))) {
            return None;
        }
        if !self.tip {
            return Some(ruling);
        }
        let (lo, hi) = if self.sign > 0.0 {
            (tip_s, far_s)
        } else {
            (far_s, tip_s)
        };
        ruling_over(face, family, t, lo, hi)
    }
}

/// Points along the extent of `ruling`, `count` of them, on the model surface.
fn along_ruling(
    face: &dyn ParametricFace,
    family: Family,
    ruling: &Ruling,
    count: usize,
) -> Vec<Point3> {
    (0..count)
        .filter_map(|k| {
            let s = ruling.s.0 + (ruling.s.1 - ruling.s.0) * k as f64 / (count - 1) as f64;
            face.point_normal(family.uv(ruling.t, s)).map(|p| p.point)
        })
        .collect()
}

/// The ideal and four-axis placement of the tool on `ruling`, with the tip
/// at the ruling's low end for `sign` +1, its high end for -1.
struct Placement {
    axis: UnitVec3,
    tilt: f64,
    needed_taper: Option<f64>,
    tip: Point3,
    length: f64,
}

fn place(
    ruling: &Ruling,
    sign: f64,
    flank: &Flank,
    settings: &Settings,
    rotary: RotaryMapping,
) -> Result<Placement> {
    let n = ruling.normal.get();
    let chord = (ruling.hi - ruling.lo) * sign;
    // The ruling direction towards the spindle, made exactly perpendicular
    // to the normal (a near-ruled face is not exactly straight).
    let d = UnitVec3::new(chord - n * chord.dot(n))
        .map_err(|_| {
            PathError::Strategy(format!(
                "a ruling at parameter {:.4} runs along its own normal",
                ruling.t
            ))
        })?
        .get();
    let (sa, ca) = flank.half_angle.sin_cos();
    let ideal = d * ca + n * sa;
    let axis = UnitVec3::new(Vec3::new(0.0, ideal.y, ideal.z)).map_err(|_| {
        PathError::Strategy(
            "its rulings run along the rotary axis, so no rotation of the part brings the flank \
             to them and no tool taper helps (decision record 0021, OI-94)"
                .to_owned(),
        )
    })?;
    let tilt = angle_between(ideal, axis.get());
    // The half angle that would put the ideal axis perpendicular to the
    // rotary axis: cos(a) d.x + sin(a) n.x = 0.
    let needed = (-d.x).atan2(n.x);
    let needed = needed - PI * ((needed + FRAC_PI_2) / PI).floor();
    let needed_taper =
        (tilt > NO_TILT && needed > 1e-6 && needed < FRAC_PI_2 - 1e-6).then(|| needed.to_degrees());
    let length = ruling.length();
    let h_tip_end = flank.z_start + settings.tip_extension;
    let h_mid = h_tip_end + 0.5 * length * ca;
    let tip_end = if sign > 0.0 { ruling.lo } else { ruling.hi };
    let middle = tip_end + d * (0.5 * length) + n * settings.stock_to_leave;
    let centre = middle + (n * ca - d * sa) * flank.radius_at(h_mid);
    let tip = centre + axis.get() * -h_mid;
    debug_assert!(
        rotary.a_facing(axis.get()).is_some(),
        "the axis is perpendicular to the rotary axis"
    );
    Ok(Placement {
        axis,
        tilt,
        needed_taper,
        tip,
        length,
    })
}

fn pose_of(
    ruling: &Ruling,
    sign: f64,
    flank: &Flank,
    settings: &Settings,
    rotary: RotaryMapping,
) -> Result<Pose> {
    let p = place(ruling, sign, flank, settings, rotary)?;
    let a = rotary.a_facing(p.axis.get()).unwrap_or(Degrees(0.0));
    Ok(Pose {
        t: ruling.t,
        tip: p.tip,
        axis: p.axis,
        a,
        contact: ruling.middle(),
        tilt: p.tilt,
        needed_taper: p.needed_taper,
        ruling_length: p.length,
    })
}

/// Which end of the rulings the tip sits at, decided once for the face at
/// its middle ruling: the smaller four-axis tilt; on a tie the end nearer
/// the rotary axis, then the end with the smaller Y, then the smaller Z.
fn choose_sign(
    ruling: &Ruling,
    flank: &Flank,
    settings: &Settings,
    rotary: RotaryMapping,
) -> Result<f64> {
    let tilt = |sign: f64| place(ruling, sign, flank, settings, rotary).map(|p| p.tilt);
    let (plus, minus) = (tilt(1.0), tilt(-1.0));
    let sign = match (plus, minus) {
        (Ok(p), Ok(m)) if (p - m).abs() > 1e-9 => {
            if p < m {
                1.0
            } else {
                -1.0
            }
        }
        (Ok(_), Err(_)) => 1.0,
        (Err(_), Ok(_)) => -1.0,
        (Err(e), Err(_)) => return Err(e),
        _ => {
            let d = ruling.hi - ruling.lo;
            let m = ruling.middle();
            let radial = Vec3::new(0.0, m.y, m.z);
            let outward = d.dot(radial);
            if outward.abs() > 1e-9 * (1.0 + radial.length()) {
                outward.signum()
            } else if d.y.abs() > 1e-9 {
                d.y.signum()
            } else if d.z.abs() > 1e-9 {
                d.z.signum()
            } else {
                1.0
            }
        }
    };
    Ok(if settings.reverse_tip_end {
        -sign
    } else {
        sign
    })
}

/// The tool at the middle of the machine's straight move from `a` to `b`
/// (X, Y, Z and A move linearly), at `fraction` of the way.
fn between(a: &Pose, b: &Pose, fraction: f64, rotary: RotaryMapping) -> (Point3, UnitVec3) {
    let pa = rotary.to_axis(a.tip, a.a);
    let pb = rotary.to_axis(b.tip, b.a);
    let lerp = |x: f64, y: f64| x + (y - x) * fraction;
    let angle = Degrees(lerp(a.a.0, b.a.0));
    let tip = rotary.to_model(
        AxisPosition {
            along: lerp(pa.along, pb.along),
            across: lerp(pa.across, pb.across),
            height: lerp(pa.height, pb.height),
        },
        angle,
    );
    (tip, rotary.tool_axis(angle))
}

/// The worst deviation of the tool at `tip` and `axis` from the ruling's
/// samples: the largest distance any of them lies from the tool's surface,
/// beyond the stock to leave, inside or out.
fn ruling_deviation(
    profile: &RevolvedProfile,
    tip: Point3,
    axis: UnitVec3,
    samples: &[Point3],
    leave: f64,
) -> f64 {
    samples
        .iter()
        .map(|p| (clearance_at(profile, tip, axis, *p) - leave).abs())
        .fold(0.0, f64::max)
}

/// The engaged length is checked against the flank (OI-97).
fn check_length(flank: &Flank, settings: &Settings, pose: &Pose, number: u32) -> Result<()> {
    let needed = settings.tip_extension + pose.ruling_length * flank.half_angle.cos();
    if needed > flank.length() + 1e-9 {
        return Err(PathError::Strategy(format!(
            "swarf on face {number} needs {needed:.3} mm of flank (a ruling of {:.3} mm plus the \
             tip extension of {} mm) but the tool's flank is {:.3} mm long, from {:.3} to {:.3} \
             mm above the tip",
            pose.ruling_length,
            settings.tip_extension,
            flank.length(),
            flank.z_start,
            flank.z_end
        )));
    }
    Ok(())
}

/// The directrix intervals, in parameter, over which `has` finds rulings,
/// each refined to where it stops finding them.
fn runs_of(t0: f64, t1: f64, has: impl Fn(f64) -> bool) -> Vec<(f64, f64)> {
    let at = |k: usize| t0 + (t1 - t0) * k as f64 / SCAN as f64;
    let present: Vec<bool> = (0..=SCAN).map(|k| has(at(k))).collect();
    let is_present = |k: usize| present.get(k).copied().unwrap_or(false);
    let mut runs = Vec::new();
    let mut k = 0;
    while k <= SCAN {
        if !is_present(k) {
            k += 1;
            continue;
        }
        let start = k;
        while is_present(k + 1) {
            k += 1;
        }
        let end = k;
        let lo = if start == 0 {
            t0
        } else {
            refine(at(start), at(start - 1), &has)
        };
        let hi = if end == SCAN {
            t1
        } else {
            refine(at(end), at(end + 1), &has)
        };
        if hi > lo {
            runs.push((lo, hi));
        }
        k += 1;
    }
    runs
}

/// The length along the face of the directrix `runs`, measured between the
/// middles of the face's own rulings.
fn directrix_length(face: &dyn ParametricFace, family: Family, runs: &[(f64, f64)]) -> f64 {
    runs.iter()
        .map(|(lo, hi)| {
            let middles: Vec<Point3> = (0..=SCAN)
                .filter_map(|k| {
                    ruling_at(face, family, lo + (hi - lo) * k as f64 / SCAN as f64)
                        .map(|r| r.middle())
                })
                .collect();
            middles
                .iter()
                .zip(middles.iter().skip(1))
                .map(|(a, b)| a.distance(*b))
                .sum::<f64>()
        })
        .sum()
}

/// Which kind of surface a [`Guard`] keeps the tool from.
/// An avoid or check surface as the tool must keep clear of it (decision
/// record 0176): sampled points, and the least clearance the tool keeps from
/// each, beyond which the tool position is not used.
struct Guard {
    kind: Protection,
    points: Vec<Point3>,
    least: f64,
}

impl Guard {
    /// The guard of `surface`: an avoid surface may not be cut into deeper
    /// than the tolerance; a check surface keeps the stock to leave, less
    /// the tolerance, clear of the tool. The points are the exact faces
    /// sampled no further apart than an eighth of the tool's cutting radius
    /// (and at least [`GUARD_GRID`] each way), or a mesh's vertices.
    fn new(
        kind: Protection,
        surface: &dyn SurfaceSource,
        settings: &Settings,
        radius: f64,
    ) -> Self {
        let least = match kind {
            Protection::Avoid => -settings.tolerance,
            Protection::Check => settings.stock_to_leave.max(0.0) - settings.tolerance,
        };
        let faces = surface.exact_faces();
        let points = if faces.is_empty() {
            surface.tessellation().vertices.clone()
        } else {
            let spacing = (radius / 8.0).max(0.05);
            faces
                .iter()
                .flat_map(|f| face_samples(f.face, spacing))
                .collect()
        };
        Self {
            kind,
            points,
            least,
        }
    }

    /// Whether the tool at `tip` along `axis`, cutting near `contact` on a
    /// ruling of `length`, comes closer to the surface than it may.
    fn breaks(
        &self,
        tool: &RevolvedProfile,
        (tip, axis, contact, length): (Point3, UnitVec3, Point3, f64),
        reach: f64,
    ) -> bool {
        self.points.iter().any(|p| {
            p.distance(contact) <= 0.5 * length + reach
                && clearance_at(tool, tip, axis, *p) < self.least
        })
    }
}

/// Points of `face`, on a grid of its parameters no coarser than `spacing`
/// on the face (measured along its middle iso-lines) and no finer than 400
/// each way.
fn face_samples(face: &dyn ParametricFace, spacing: f64) -> Vec<Point3> {
    let (lo, hi) = face.uv_bounds();
    let uv = |a: f64, b: f64| Uv {
        u: lo.u + (hi.u - lo.u) * a,
        v: lo.v + (hi.v - lo.v) * b,
    };
    let length = |line: &dyn Fn(f64) -> Uv| {
        let points: Vec<Point3> = (0..=16)
            .filter_map(|k| face.point_normal(line(k as f64 / 16.0)).map(|p| p.point))
            .collect();
        points
            .iter()
            .zip(points.iter().skip(1))
            .map(|(a, b)| a.distance(*b))
            .sum::<f64>()
    };
    let count = |l: f64| ((l / spacing).ceil() as usize + 1).clamp(GUARD_GRID, 400);
    let nu = count(length(&|a| uv(a, 0.5)));
    let nv = count(length(&|b| uv(0.5, b)));
    let mut points = Vec::new();
    for i in 0..nu {
        for j in 0..nv {
            let at = uv(i as f64 / (nu - 1) as f64, j as f64 / (nv - 1) as f64);
            if face.contains(at)
                && let Some(p) = face.point_normal(at)
            {
                points.push(p.point);
            }
        }
    }
    points
}

/// Plans the swarf of one face: detection, placement, spacing, the limits
/// of `region` and the gate. Fails when the face is not ruled, its rulings
/// cannot be reached on four axes, a ruling is longer than the flank, the
/// face has no rulings on it, or the limits leave nothing of it to cut; a
/// face the tool would cut into is returned with its measured gouge (see
/// [`FacePath::overcut_warning`]).
///
/// `region` limits the cut (OP-02; decision record 0176): its boundary as
/// its [`BoundaryLimits`] say, and its avoid and check surfaces by leaving
/// out every tool position that would come too close to them, so the pass
/// stops or splits there. Its watched surfaces are only measured: the
/// lengths along which the tool breaks them are in
/// [`Limits::watched`]. A selected region is not used: swarf's faces are
/// its selection.
pub fn face_path(
    face: FaceRef<'_>,
    tool: &RevolvedProfile,
    rotary: RotaryMapping,
    settings: &Settings,
    region: &RegionConstraint<'_>,
    context: &JobContext<'_>,
) -> Result<FacePath> {
    let number = face.number;
    let face = face.face;
    let fail =
        |what: String| PathError::Strategy(format!("swarf cannot cut face {number}: {what}"));
    let flank = Flank::of(tool)?;
    let detection = detect_rulings(face, settings.tolerance);
    if let Some(why) = detection.refusal() {
        return Err(fail(why));
    }
    let family = detection.chosen.unwrap_or(Family::AlongV);
    let ((t0, t1), (s0, s1)) = family.ranges(face);
    let whole = runs_of(t0, t1, |t| ruling_at(face, family, t).is_some());
    if whole.is_empty() {
        return Err(fail(
            "none of its rulings lies on the trimmed face".to_owned(),
        ));
    }
    // A face whose directrix comes back to its start is cut as one closed
    // run.
    let closed = whole.len() == 1
        && whole.first().is_some_and(|r| r.0 <= t0 && r.1 >= t1)
        && [s0, 0.5 * (s0 + s1), s1].iter().all(|s| {
            match (
                face.point_normal(family.uv(t0, *s)),
                face.point_normal(family.uv(t1, *s)),
            ) {
                (Some(a), Some(b)) => a.point.distance(b.point) < SAME_POINT,
                _ => false,
            }
        });
    // The tip end is chosen on the whole face, before any limit, so a
    // boundary never turns the tool over.
    let middle_t = whole
        .iter()
        .map(|r| 0.5 * (r.0 + r.1))
        .fold(0.0, |acc, t| acc + t)
        / whole.len() as f64;
    let middle = ruling_at(face, family, middle_t)
        .or_else(|| {
            whole
                .first()
                .and_then(|r| ruling_at(face, family, 0.5 * (r.0 + r.1)))
        })
        .ok_or_else(|| fail("its middle ruling has no extent".to_owned()))?;
    let sign = choose_sign(&middle, &flank, settings, rotary).map_err(|e| match e {
        PathError::Strategy(why) => fail(why),
        other => other,
    })?;
    let limiter = Limiter {
        boundary: region.boundary,
        pass: region.boundary_limits.pass,
        tip: region.boundary_limits.tip,
        sign,
    };
    let rule = |t: f64| limiter.ruling(face, family, t);
    let runs = if limiter.window().is_some() {
        runs_of(t0, t1, |t| rule(t).is_some())
    } else {
        whole.clone()
    };
    if runs.is_empty() {
        return Err(fail(
            "none of its rulings lies inside the boundary set, so the boundary leaves nothing \
             of it to cut"
                .to_owned(),
        ));
    }
    let outside_boundary = if limiter.window().is_some() {
        (directrix_length(face, family, &whole) - directrix_length(face, family, &runs)).max(0.0)
    } else {
        0.0
    };
    let closed = closed && outside_boundary <= SAME_POINT && runs.len() == whole.len();
    let pose_at = |t: f64| -> Result<Option<Pose>> {
        let Some(r) = rule(t) else {
            return Ok(None);
        };
        let pose = pose_of(&r, sign, &flank, settings, rotary).map_err(|e| match e {
            PathError::Strategy(why) => fail(why),
            other => other,
        })?;
        check_length(&flank, settings, &pose, number)?;
        Ok(Some(pose))
    };
    let chord = CHORD_FACTOR * settings.tolerance;
    let leave = settings.stock_to_leave;
    let reach = tool.cutting_radius() + settings.tip_extension.abs() + leave.abs() + 1.0;
    let radius = tool.cutting_radius();
    let guards: Vec<Guard> = [
        (Protection::Avoid, region.avoid),
        (Protection::Check, region.check),
    ]
    .into_iter()
    .filter_map(|(kind, surface)| surface.map(|s| Guard::new(kind, s, settings, radius)))
    .collect();
    let watched: Vec<Guard> = region
        .watch
        .iter()
        .map(|w| Guard::new(w.protection, w.surface, settings, radius))
        .collect();
    let mut broken = vec![0.0f64; watched.len()];
    let mut uncut = [0.0f64; 2];
    let mut out_runs = Vec::new();
    let mut total = 0usize;
    for (lo, hi) in &runs {
        context.check_cancelled()?;
        let initial = if closed { 8 } else { 4 };
        let mut poses: Vec<(Pose, u32)> = Vec::new();
        for k in 0..=initial {
            let t = lo + (hi - lo) * k as f64 / initial as f64;
            if let Some(mut p) = pose_at(t)? {
                if let Some((prev, _)) = poses.last() {
                    p.a = nearest_turn(p.a, prev.a);
                }
                poses.push((p, 0));
            }
        }
        if poses.len() < 2 {
            continue;
        }
        // Split each move until the tool at its middle keeps the ruling
        // there within the chord limit.
        let mut i = 0;
        while let (Some(&(a, depth)), Some(&(b, _))) = (poses.get(i), poses.get(i + 1)) {
            let t = 0.5 * (a.t + b.t);
            let Some(mut p) = pose_at(t)? else {
                i += 1;
                continue;
            };
            // The move is split when the tool half way along it leaves the
            // ruling there by more than the chord limit beyond what the
            // pose at that ruling leaves anyway (a tilted flank never lies
            // on its ruling, and more poses do not help that), or when A
            // turns too far in one move.
            let split = depth < MAX_DEPTH
                && ((b.a.0 - a.a.0).abs() > MAX_ANGLE_STEP
                    || match rule(t) {
                        Some(r) => {
                            let samples = along_ruling(face, family, &r, CHECK_SAMPLES);
                            let (tip, axis) = between(&a, &b, 0.5, rotary);
                            let moving = ruling_deviation(tool, tip, axis, &samples, leave);
                            let posed = ruling_deviation(tool, p.tip, p.axis, &samples, leave);
                            moving > posed + chord
                        }
                        None => false,
                    });
            if split {
                p.a = nearest_turn(p.a, a.a);
                // Both halves are one level deeper: the move from `a` and
                // the move from the new pose.
                poses.insert(i + 1, (p, depth + 1));
                if let Some(first) = poses.get_mut(i) {
                    first.1 = depth + 1;
                }
                total += 1;
                if total > MAX_POSES {
                    return Err(fail(format!(
                        "it needs more than {MAX_POSES} poses at a tolerance of {} mm",
                        settings.tolerance
                    )));
                }
                if total.is_multiple_of(256) {
                    context.check_cancelled()?;
                }
            } else {
                i += 1;
            }
        }
        let run: Vec<Pose> = poses.into_iter().map(|(p, _)| p).collect();
        let pieces = if guards.is_empty() {
            vec![run]
        } else {
            context.check_cancelled()?;
            guarded(&run, &guards, tool, rotary, reach, &mut uncut, &pose_at)?
        };
        // A watched surface is measured along what is cut, the length the
        // guard would have left uncut, and the run is kept as it is.
        for (guard, length) in watched.iter().zip(&mut broken) {
            for piece in &pieces {
                context.check_cancelled()?;
                let mut along = [0.0f64; 2];
                guarded(
                    piece,
                    std::slice::from_ref(guard),
                    tool,
                    rotary,
                    reach,
                    &mut along,
                    &pose_at,
                )?;
                *length += along[0] + along[1];
            }
        }
        for mut run in pieces {
            if settings.reverse_direction {
                run.reverse();
                let mut previous: Option<Degrees> = None;
                for p in &mut run {
                    if let Some(prev) = previous {
                        p.a = nearest_turn(p.a, prev);
                    }
                    previous = Some(p.a);
                }
            }
            out_runs.push(run);
        }
    }
    if out_runs.is_empty() {
        let blocked: Vec<&str> = guards
            .iter()
            .map(|g| match g.kind {
                Protection::Avoid => "cut into the avoid surfaces",
                Protection::Check => "touch the check surfaces",
            })
            .collect();
        return Err(fail(if blocked.is_empty() {
            "no run of rulings has two poses".to_owned()
        } else {
            format!(
                "the tool would {} wherever it cuts it, so nothing of it is left to cut; take \
                 the face out of the face set, or the surface out of the set",
                blocked.join(" or ")
            )
        }));
    }
    let closed = closed && out_runs.len() == 1 && uncut.iter().all(|u| *u <= SAME_POINT);
    context.check_cancelled()?;

    // The gate: every sample of the face against the union of the tool
    // along the whole motion (decision record 0021 item 3).
    let mut positions: Vec<(Point3, UnitVec3, Point3, f64)> = Vec::new();
    for run in &out_runs {
        for (i, p) in run.iter().enumerate() {
            positions.push((p.tip, p.axis, p.contact, p.ruling_length));
            if let Some(q) = run.get(i + 1) {
                positions.extend(motion(p, q, rotary));
            }
        }
    }
    let mut samples: Vec<Point3> = Vec::new();
    for run in &out_runs {
        for (i, p) in run.iter().enumerate() {
            let mut ts = vec![p.t];
            if let Some(q) = run.get(i + 1) {
                ts.push(0.5 * (p.t + q.t));
            }
            for t in ts {
                if let Some(r) = rule(t) {
                    samples.extend(along_ruling(face, family, &r, GATE_SAMPLES));
                }
            }
        }
    }
    let deviations: Vec<f64> = parallel::map_in_order(&samples, |p| {
        let mut best = f64::INFINITY;
        for (tip, axis, contact, length) in &positions {
            if p.distance(*contact) > 0.5 * length + reach {
                continue;
            }
            best = best.min(clearance_at(tool, *tip, *axis, *p) - leave);
        }
        best
    });
    let mut gouge = 0.0f64;
    let mut gouge_at = Point3::ORIGIN;
    let mut left = 0.0f64;
    let mut left_at = Point3::ORIGIN;
    for (p, d) in samples.iter().zip(&deviations) {
        if !d.is_finite() {
            continue;
        }
        if *d < -gouge {
            gouge = -d;
            gouge_at = *p;
        }
        if *d > left {
            left = *d;
            left_at = *p;
        }
    }
    let twist = out_runs
        .iter()
        .flatten()
        .filter_map(|p| rule(p.t).map(|r| r.twist))
        .fold(detection.fit(family).twist, f64::max);
    // How far the boundary raised the tip: from the tip end of the whole
    // ruling to the tip end of the limited one.
    let tip_end = |r: &Ruling| if sign > 0.0 { r.lo } else { r.hi };
    let tip_raised = if limiter.tip && limiter.window().is_some() {
        out_runs
            .iter()
            .flatten()
            .filter_map(|p| {
                Some(tip_end(&ruling_at(face, family, p.t)?).distance(tip_end(&rule(p.t)?)))
            })
            .fold(0.0, f64::max)
    } else {
        0.0
    };
    let max_tilt = out_runs
        .iter()
        .flatten()
        .map(|p| p.tilt)
        .fold(0.0, f64::max);
    let tapers: Vec<f64> = out_runs
        .iter()
        .flatten()
        .filter_map(|p| p.needed_taper)
        .collect();
    let suggested_taper = if max_tilt > NO_TILT
        && !tapers.is_empty()
        && tapers.len()
            == out_runs
                .iter()
                .flatten()
                .filter(|p| p.tilt > NO_TILT)
                .count()
    {
        let lo = tapers.iter().copied().fold(f64::INFINITY, f64::min);
        let hi = tapers.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        (hi - lo <= SAME_TAPER).then_some(0.5 * (lo + hi))
    } else {
        None
    };
    let [avoid_uncut, check_uncut] = uncut;
    Ok(FacePath {
        number,
        detection,
        family,
        runs: out_runs,
        closed,
        gouge,
        gouge_at,
        left,
        left_at,
        twist,
        max_tilt,
        suggested_taper,
        limits: Limits {
            boundary: limiter.window().map(|_| (limiter.pass, limiter.tip)),
            outside_boundary,
            tip_raised,
            avoid_uncut,
            check_uncut,
            watched: region
                .watch
                .iter()
                .zip(broken)
                .enumerate()
                .filter(|(_, (_, length))| *length > SAME_POINT)
                .map(|(index, (w, length))| WatchedBreak {
                    index,
                    name: w.name.to_owned(),
                    protection: w.protection,
                    length,
                })
                .collect(),
        },
    })
}

/// The tool at the points of the machine's motion from `p` to `q` that the
/// gate and the guards look at, as (tip, axis, contact, ruling length).
fn motion(p: &Pose, q: &Pose, rotary: RotaryMapping) -> Vec<(Point3, UnitVec3, Point3, f64)> {
    (1..GATE_STEPS)
        .map(|k| {
            let f = k as f64 / GATE_STEPS as f64;
            let (tip, axis) = between(p, q, f, rotary);
            let contact = p.contact + (q.contact - p.contact) * f;
            (tip, axis, contact, p.ruling_length.max(q.ruling_length))
        })
        .collect()
}

/// The pieces of `run` the guards leave (decision record 0176): a pose at
/// which the tool comes too close to an avoid or check surface is left out,
/// and so is a move along which it does, so the pass stops there and starts
/// again after it with a retract. Pieces of fewer than two poses are
/// dropped. The length along the face left uncut is added to `uncut`, the
/// avoid surfaces' first, by the first guard broken.
fn guarded(
    run: &[Pose],
    guards: &[Guard],
    tool: &RevolvedProfile,
    rotary: RotaryMapping,
    reach: f64,
    uncut: &mut [f64; 2],
    pose_at: &dyn Fn(f64) -> Result<Option<Pose>>,
) -> Result<Vec<Vec<Pose>>> {
    let first_broken = |position: (Point3, UnitVec3, Point3, f64)| {
        guards
            .iter()
            .find(|g| g.breaks(tool, position, reach))
            .map(|g| g.kind)
    };
    let clear = |p: &Pose| first_broken((p.tip, p.axis, p.contact, p.ruling_length)).is_none();
    // Where the tool goes from clear to not, the last clear pose is found by
    // bisection, so the pass stops close to the surface, not at the poses
    // the spacing happened to place.
    let mut run = run.to_vec();
    let mut i = 0;
    while let (Some(&a), Some(&b)) = (run.get(i), run.get(i + 1)) {
        let (a_clear, b_clear) = (clear(&a), clear(&b));
        if a_clear == b_clear {
            i += 1;
            continue;
        }
        let (mut good, mut bad) = if a_clear { (a.t, b.t) } else { (b.t, a.t) };
        let mut found: Option<Pose> = None;
        for _ in 0..GUARD_STEPS {
            let t = 0.5 * (good + bad);
            match pose_at(t)? {
                Some(p) if clear(&p) => {
                    good = t;
                    found = Some(p);
                }
                _ => bad = t,
            }
        }
        match found {
            Some(mut p) => {
                let (next, neighbour) = if a_clear { (i + 1, a) } else { (i + 1, b) };
                p.a = nearest_turn(p.a, neighbour.a);
                run.insert(next, p);
                i += 2;
            }
            None => i += 1,
        }
    }
    let run = run.as_slice();
    let at_pose: Vec<Option<Protection>> = parallel::map_in_order(run, |p| {
        first_broken((p.tip, p.axis, p.contact, p.ruling_length))
    });
    let moves: Vec<(&Pose, &Pose)> = run.iter().zip(run.iter().skip(1)).collect();
    let on_move: Vec<Option<Protection>> = parallel::map_in_order(&moves, |(p, q)| {
        motion(p, q, rotary).into_iter().find_map(first_broken)
    });
    // The pieces as indices into `run`.
    let mut pieces: Vec<Vec<usize>> = Vec::new();
    let mut current: Vec<usize> = Vec::new();
    for i in 0..run.len() {
        if at_pose.get(i).copied().flatten().is_some() {
            pieces.push(std::mem::take(&mut current));
            continue;
        }
        current.push(i);
        if on_move.get(i).copied().flatten().is_some() {
            pieces.push(std::mem::take(&mut current));
        }
    }
    pieces.push(current);
    pieces.retain(|p| p.len() >= 2);
    // Every move not cut counts as uncut, against the guard that stopped it.
    let mut cut = vec![false; moves.len()];
    for piece in &pieces {
        for i in piece.iter().take(piece.len() - 1) {
            if let Some(c) = cut.get_mut(*i) {
                *c = true;
            }
        }
    }
    let fallback = guards.first().map(|g| g.kind);
    for (i, (p, q)) in moves.iter().enumerate() {
        if cut.get(i).copied().unwrap_or(true) {
            continue;
        }
        let kind = on_move
            .get(i)
            .copied()
            .flatten()
            .or(at_pose.get(i).copied().flatten())
            .or(at_pose.get(i + 1).copied().flatten())
            .or(fallback);
        let length = p.contact.distance(q.contact);
        match kind {
            Some(Protection::Check) => uncut[1] += length,
            _ => uncut[0] += length,
        }
    }
    Ok(pieces
        .into_iter()
        .map(|piece| {
            piece
                .into_iter()
                .filter_map(|i| run.get(i).copied())
                .collect()
        })
        .collect())
}

// ---------------------------------------------------------------------------
// The strategy.

fn number_spec(
    name: &str,
    label: &str,
    description: &str,
    unit: Unit,
    default: f64,
) -> ParameterSpec {
    ParameterSpec {
        name: name.to_owned(),
        label: label.to_owned(),
        description: description.to_owned(),
        kind: ParameterKind::Number {
            unit,
            min: Some(0.0),
            min_exclusive: false,
            max: None,
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
            default: options.first().copied().unwrap_or_default().to_owned(),
        },
    }
}

impl Strategy for Swarf {
    fn name(&self) -> &str {
        NAME
    }

    fn label(&self) -> &str {
        "Swarf (flank) machining"
    }

    fn description(&self) -> &str {
        "Cuts with the side of the tool laid along straight lines in a curved wall of the part, so one pass finishes a tall surface. It needs a STEP model, not a mesh, and walls that are made of straight lines."
    }

    fn group(&self) -> StrategyGroup {
        StrategyGroup::Specialist
    }

    fn needs_brep_part(&self) -> Option<&str> {
        Some("it reads the rulings of the exact faces of a STEP model, and a mesh has none")
    }

    fn schema(&self) -> ParameterSchema {
        use parameter::*;
        let mut schema = ParameterSchema::with_common(vec![
            number_spec(
                TIP_EXTENSION,
                "Tip extension",
                "How far the tool tip reaches beyond the end of the ruling it sits at, along \
                 the tool axis. Zero puts the start of the flank at that end.",
                Unit::Millimetres,
                0.0,
            ),
            number_spec(
                MAX_MATERIAL_LEFT,
                "Most material left",
                "The most material the flank may leave beyond the stock to leave anywhere on \
                 the face, between poses or where four axes cannot hold the flank on the \
                 rulings. More than the tolerance is reported; more than this refuses the face. \
                 Zero takes 0.1 mm.",
                Unit::Millimetres,
                0.0,
            ),
            choice(
                TIP_END,
                "Tip end",
                "Which end of the rulings the tool tip sits at: auto chooses the end with the \
                 smaller four-axis tilt, then the end nearer the rotary axis; reversed takes the \
                 other end.",
                &[AUTO, REVERSED],
            ),
            choice(
                DIRECTION,
                "Direction",
                "Which way along the face the tool moves: forward follows the face's parameter, \
                 reverse goes the other way.",
                &[FORWARD, REVERSE],
            ),
        ]);
        for spec in &mut schema.parameters {
            match spec.name.as_str() {
                names::STEPOVER => {
                    spec.description =
                        "Not used: each face is cut in one pass along its rulings.".to_owned();
                    if let ParameterKind::Number { default, .. } = &mut spec.kind {
                        *default = Some(1.0);
                    }
                }
                names::TOLERANCE => {
                    spec.description = "The toolpath tolerance: a face is ruled when its \
                                        iso-lines are straight within it, the poses keep the \
                                        flank within it, and a cut into the face deeper than \
                                        it is warned of: raise it to accept such a cut."
                        .to_owned();
                }
                _ => {}
            }
        }
        schema
    }

    fn honours_region(&self) -> bool {
        true
    }

    fn honours_restriction_options(&self) -> bool {
        true
    }

    fn passes_in_region(
        &self,
        input: &StrategyInput<'_>,
        region: &RegionConstraint<'_>,
        parameters: &Parameters,
        context: &JobContext<'_>,
    ) -> Result<Vec<Pass>> {
        Ok(swarf(input, region, parameters, context)?.passes)
    }

    fn passes_with_notes(
        &self,
        input: &StrategyInput<'_>,
        region: Option<&RegionConstraint<'_>>,
        parameters: &Parameters,
        context: &JobContext<'_>,
    ) -> Result<StrategyOutput> {
        swarf(
            input,
            region.unwrap_or(&RegionConstraint::default()),
            parameters,
            context,
        )
    }

    fn passes(
        &self,
        input: &StrategyInput<'_>,
        parameters: &Parameters,
        context: &JobContext<'_>,
    ) -> Result<Vec<Pass>> {
        Ok(swarf(input, &RegionConstraint::default(), parameters, context)?.passes)
    }
}

fn swarf(
    input: &StrategyInput<'_>,
    region: &RegionConstraint<'_>,
    parameters: &Parameters,
    context: &JobContext<'_>,
) -> Result<StrategyOutput> {
    if region.region.is_some() {
        return Err(PathError::Strategy(
            "swarf cuts the selected faces, not a region: put the walls to cut in its face set, \
             and use a boundary, avoid or check set to limit it"
                .to_owned(),
        ));
    }
    let faces = input.surface.exact_faces();
    if faces.is_empty() {
        return Err(PathError::Strategy(
            "swarf needs the exact faces of a STEP model to read their rulings, and the selection \
             has none: a mesh has no rulings to follow (decision record 0021)"
                .to_owned(),
        ));
    }
    let settings = Settings {
        tolerance: parameters.number(names::TOLERANCE)?,
        stock_to_leave: parameters.number(names::STOCK_TO_LEAVE)?,
        tip_extension: parameters.number(parameter::TIP_EXTENSION)?,
        max_material_left: parameters.number(parameter::MAX_MATERIAL_LEFT)?,
        reverse_tip_end: parameters.text(parameter::TIP_END)? == REVERSED,
        reverse_direction: parameters.text(parameter::DIRECTION)? == REVERSE,
    };
    let mut passes = Vec::new();
    let mut notes = Vec::new();
    let mut warnings = Vec::new();
    // A boundary with both its limits off limits nothing: the path is made
    // as if there were none, and the user is told (decision record 0176).
    if region.boundary.is_some()
        && region.boundary_limits
            == (BoundaryLimits {
                pass: false,
                tip: false,
            })
    {
        warnings.push(PathWarning {
            message: "The boundary of this swarf operation limits nothing, because both its \
                      limits, the pass and the tip, are off: the faces are cut as if there were \
                      no boundary."
                .to_owned(),
            parameter: None,
        });
    }
    // Each watched surface the tool breaks, over every face, in the
    // region's order.
    let mut watched: Vec<(f64, Vec<u32>)> = vec![(0.0, Vec::new()); region.watch.len()];
    let total = faces.len() as u64;
    for (i, face) in faces.into_iter().enumerate() {
        context.report("swarf", i as u64, Some(total));
        let path = face_path(face, input.tool, input.rotary, &settings, region, context)?;
        let limit = settings.material_left_limit();
        if let Some(why) = path.refusal(limit) {
            return Err(PathError::Strategy(why));
        }
        if let Some(message) = path.overcut_warning(settings.tolerance) {
            warnings.push(PathWarning {
                message,
                parameter: Some(names::TOLERANCE.to_owned()),
            });
        }
        for broken in &path.limits.watched {
            if let Some((length, faces)) = watched.get_mut(broken.index) {
                *length += broken.length;
                faces.push(path.number);
            }
        }
        notes.push(path.note(settings.tolerance, limit));
        for run in &path.runs {
            passes.push(Pass {
                points: run
                    .iter()
                    .map(|p| CutPoint {
                        tip: p.tip,
                        a: p.a,
                        contact: p.contact,
                    })
                    .collect(),
                stock: None,
                rapids: Vec::new(),
            });
        }
    }
    for (w, (length, faces)) in region.watch.iter().zip(watched) {
        if faces.is_empty() {
            continue;
        }
        let faces: Vec<String> = faces.iter().map(u32::to_string).collect();
        warnings.push(PathWarning {
            message: format!(
                "The tool {} '{}' along about {length:.3} mm of face {}. The set only warns, \
                 so the path is kept: set it to cut the pass short, or check the cut in the \
                 simulation before using the path.",
                breaks(w.protection),
                w.name,
                faces.join(", ")
            ),
            parameter: None,
        });
    }
    context.report("swarf", total, Some(total));
    Ok(StrategyOutput {
        passes,
        notes,
        warnings,
    })
}
