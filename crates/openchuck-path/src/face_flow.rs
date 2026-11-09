// SPDX-License-Identifier: AGPL-3.0-or-later
//! Face-flow finishing (OP-28; decision record 0525; lane FACEFLOW).
//!
//! On a STEP (B-rep) part the user selects faces, and each face is cut with
//! passes that run parallel to a **guide**, a chain of its boundary edges,
//! and step across the face from it to the opposite boundary: a blisk's
//! blade floor is cut along the shape of the floor, not the machine's axes.
//!
//! # The passes
//!
//! - **Loops and sides.** A face's outer boundary is its longest loop of
//!   edges, joined end to end at their vertices. The loop is cut into sides
//!   where it turns by more than [`CORNER_ANGLE`]; a loop with fewer than
//!   two such corners is cut at its two points furthest apart.
//! - **The guide** is the longest side, measured along the edges' exact
//!   curves, ties going to the side with the lowest edge ID; or the edges
//!   the user chose, which must be joined end to end on the face's boundary
//!   (decision 0525, owner's decisions D2 and D3).
//! - **The opposite boundary** is what is left of the loop without the guide
//!   and the two sides next to it; with only two sides left it is the point
//!   where they meet (a three-sided face), and with one it is that side.
//! - **The blend.** Guide and opposite are both taken by arc-length fraction
//!   `s` and put on the face's (u, v); the pass at weight `w` is the face's
//!   point at `(1 - w) uv_guide(s) + w uv_opposite(s)` (owner's point 2), so
//!   every point of every pass lies on the exact face.
//! - **The spacing.** Weights are chosen so that, at every `s`, the next
//!   pass is no further than the stepover along the face and, with a
//!   scallop height and a ball or bull-nose, leaves no higher cusp; the
//!   gaps are then evened out. The last pass is the opposite boundary.
//!
//! # The tool
//!
//! The tool axis follows the face's normal in the plane across the rotary
//! axis, leaned by the lead angle towards the travel round the axis, and
//! smoothed along the pass within the tilt limit, as simultaneous finishing
//! does (OP-05, OP-06, OP-15). The tool is placed touching the pass point
//! and dropped along its axis onto the **whole part**, so faces beside the
//! selection (a blade wall beside a floor) lift it where they are in the
//! way: such a stretch is left out and reported as unreached, with where
//! and why (OP-12, NFR-07). Every straight move between two points is
//! checked at its middle and quarter points and split where it would leave
//! the face by more than the tolerance (decision records 0115 and 0133).
//!
//! # Direction and order
//!
//! Passes are cut climb, conventional or meander (decision 0525 D1, a
//! strategy-local choice until OP-25's shared one exists). Several faces are
//! cut one after another, each with its own guide, starting from the face
//! with the longest guide and then along the faces' shared edges (D2).

use crate::notice::{Level, Notice, topic};
use std::collections::{BTreeMap, BTreeSet};
use std::f64::consts::TAU;
use std::sync::Arc;

use openchuck_types::brep::{BrepFace, BrepModel, EdgeId, FaceId, Surface};
use openchuck_types::geometry::{Point3, UnitVec3, Vec3};
use openchuck_types::job::JobContext;
use openchuck_types::tool::profile::RevolvedProfile;
use openchuck_types::units::Degrees;

use crate::assemble::{MAX_UNREACHED_POINTS, Unreached, UnreachedReason};
use crate::brep::{BrepSurface, ExactFace};
use crate::cutter::{self, ToolLine};
use crate::error::{PathError, Result};
use crate::params::{ParameterKind, ParameterSchema, ParameterSpec, Parameters, Unit, names};
use crate::region::RegionConstraint;
use crate::rotary::{RotaryMapping, nearest_turn};
use crate::scallop;
use crate::strategy::{CutPoint, Pass, Strategy, StrategyInput, StrategyOutput};
use crate::surface::{ParametricFace, SurfacePoint, SurfaceSource, Uv};

/// The registry name.
pub const NAME: &str = "face_flow";

/// Parameter names specific to face-flow finishing.
pub mod parameter {
    /// The scallop (cusp) height between passes, mm; zero for none.
    pub const SCALLOP: &str = "scallop";
    /// Lean of the tool axis from the normal towards the travel round the
    /// rotary axis, degrees.
    pub const LEAD_ANGLE: &str = "lead_angle";
    /// The most the tool axis may leave its target to smooth A, degrees.
    pub const MAX_TILT: &str = "max_tilt";
    /// The cutting direction: [`MEANDER`], [`CLIMB`] or [`CONVENTIONAL`].
    pub const DIRECTION: &str = "direction";
    /// Passes alternate direction, with no return move between them.
    pub const MEANDER: &str = "meander";
    /// Every pass climb cut (with `M3`, the uncut material on the tool's
    /// right looking along the feed from the spindle; decision 0213).
    pub const CLIMB: &str = "climb";
    /// Every pass conventional cut.
    pub const CONVENTIONAL: &str = "conventional";
}

/// The turn between two edges, in degrees, above which their junction is a
/// corner of the face's boundary.
pub const CORNER_ANGLE: f64 = 30.0;

/// Largest lead and tilt, degrees, as simultaneous finishing's.
const LEAD_LIMIT: f64 = 30.0;
const TILT_LIMIT: f64 = 30.0;

/// Two edge ends closer than this, in millimetres, are joined when the
/// edges carry no vertices.
const JOIN: f64 = 1e-3;

/// The spacing of the samples along the guide, in millimetres, before a
/// pass is refined to the tolerance.
const SAMPLE_SPACING: f64 = 0.25;

/// Most samples along the guide (NFR-07).
const MAX_GUIDE_SAMPLES: usize = 8192;

/// Samples across the face for the pass spacing, and their spacing in
/// millimetres.
const ACROSS_SPACING: f64 = 0.02;
const MIN_ACROSS: usize = 64;
const MAX_ACROSS: usize = 4096;

/// Places along the guide at which the pass spacing is measured.
const SPACING_PLACES: usize = 49;

/// Most passes on one face (NFR-07).
const MAX_PASSES: usize = 20_000;

/// How many times a pass's chord or a move is halved.
const MAX_DEPTH: u32 = 8;

/// The step in guide fraction over which the direction of travel is read.
const TANGENT_STEP: f64 = 1e-6;

/// Fractions of a straight move at which it is checked (decision records
/// 0115 and 0133).
const MOVE_CHECKS: [f64; 3] = [0.25, 0.5, 0.75];

/// The selection a host gives the strategy: the model, the faces to cut and
/// the user's guide edges (empty for the default guides).
#[derive(Debug)]
struct Selection {
    model: Arc<BrepModel>,
    faces: Vec<FaceId>,
    guide: Vec<EdgeId>,
}

/// Face-flow finishing (OP-28). The registry's instance has no model; a host
/// builds one with [`FaceFlow::on_brep`] for each operation on a STEP part.
#[derive(Debug, Default, Clone)]
pub struct FaceFlow {
    selection: Option<Arc<Selection>>,
}

impl FaceFlow {
    /// The strategy for `faces` of `model`, with the guide made of `guide`
    /// where it bounds a face (the longest side of each face otherwise).
    pub fn on_brep(model: Arc<BrepModel>, faces: Vec<FaceId>, guide: Vec<EdgeId>) -> Self {
        Self {
            selection: Some(Arc::new(Selection {
                model,
                faces,
                guide,
            })),
        }
    }
}

// ---------------------------------------------------------------------------
// The strategy.

fn number(
    name: &str,
    label: &str,
    description: &str,
    unit: Unit,
    range: (f64, f64),
    default: f64,
) -> ParameterSpec {
    ParameterSpec {
        name: name.to_owned(),
        label: label.to_owned(),
        description: description.to_owned(),
        kind: ParameterKind::Number {
            unit,
            min: Some(range.0),
            min_exclusive: false,
            max: range.1.is_finite().then_some(range.1),
            default: Some(default),
        },
    }
}

impl Strategy for FaceFlow {
    fn name(&self) -> &str {
        NAME
    }

    fn label(&self) -> &str {
        "Face flow finishing"
    }

    fn description(&self) -> &str {
        "Finishes selected faces of a STEP model with passes that run along one edge of each \
         face, the guide, and step across the face to the opposite edge, following the face's \
         shape rather than the machine's axes: for a blade floor of a blisk, for example. The \
         guide is the longest side of the face unless you choose its edges. Passes blend \
         between the guide and the opposite side where they differ in length. Where a \
         neighbouring face, such as a blade wall, is in the way of the tool, that stretch is \
         left uncut and reported. The cutting direction is chosen here until a shared setting \
         for every strategy exists. Needs a STEP model. Check the toolpath and simulation \
         before running the program."
    }

    fn needs_brep_part(&self) -> Option<&str> {
        Some("it follows the exact faces and edges of a STEP model, and a mesh has none")
    }

    fn schema(&self) -> ParameterSchema {
        use parameter::*;
        let mut schema = ParameterSchema::with_common(vec![
            number(
                SCALLOP,
                "Scallop height",
                "The highest ridge left between neighbouring passes, for a ball-end or \
                 bull-nose tool: passes are put closer than the stepover where it needs them. \
                 Zero leaves the stepover alone to set the passes.",
                Unit::Millimetres,
                (0.0, f64::INFINITY),
                0.0,
            ),
            number(
                LEAD_ANGLE,
                "Lead angle",
                "How far the tool axis leans from the surface normal towards the travel round \
                 the rotary axis. Passes that run along the rotary axis cannot lean.",
                Unit::Degrees,
                (-LEAD_LIMIT, LEAD_LIMIT),
                0.0,
            ),
            number(
                MAX_TILT,
                "Maximum tilt",
                "How far the tool axis may leave its leaned direction to smooth the turning \
                 of the part along a pass.",
                Unit::Degrees,
                (0.0, TILT_LIMIT),
                5.0,
            ),
            ParameterSpec {
                name: DIRECTION.to_owned(),
                label: "Cutting direction".to_owned(),
                description: "Meander alternates the direction of the passes; climb and \
                              conventional cut every pass the same way relative to the uncut \
                              material and the spindle. This choice belongs to face-flow \
                              finishing until a shared cutting-direction setting exists."
                    .to_owned(),
                kind: ParameterKind::Choice {
                    options: [MEANDER, CLIMB, CONVENTIONAL]
                        .iter()
                        .map(|o| (*o).to_owned())
                        .collect(),
                    default: MEANDER.to_owned(),
                },
            },
        ]);
        for spec in &mut schema.parameters {
            if spec.name == names::STEPOVER {
                spec.description = "The largest distance between neighbouring passes, \
                                    measured along the face."
                    .to_owned();
            }
        }
        schema
    }

    fn parameter_warnings(&self, tool: &RevolvedProfile, parameters: &Parameters) -> Vec<String> {
        parameters
            .number(parameter::SCALLOP)
            .ok()
            .and_then(|height| scallop::warning(tool, height))
            .into_iter()
            .collect()
    }

    fn honours_region(&self) -> bool {
        true
    }

    fn passes_with_notes(
        &self,
        input: &StrategyInput<'_>,
        region: Option<&RegionConstraint<'_>>,
        parameters: &Parameters,
        context: &JobContext<'_>,
    ) -> Result<StrategyOutput> {
        let Some(selection) = &self.selection else {
            return Err(PathError::refused(
                crate::topic::FACE_FLOW,
                "face-flow finishing needs the exact faces of a STEP model and the faces to \
                 cut: add the part as a STEP file and select the faces"
                    .to_owned(),
            ));
        };
        let default = RegionConstraint::default();
        face_flow(
            selection,
            input,
            region.unwrap_or(&default),
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
        Ok(self
            .passes_with_notes(input, None, parameters, context)?
            .passes)
    }
}

// ---------------------------------------------------------------------------
// Settings.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    Meander,
    Climb,
    Conventional,
}

#[derive(Debug, Clone, Copy)]
struct Settings {
    tolerance: f64,
    stepover: f64,
    leave: f64,
    /// The cusp height and the radius of the tool's cutting arc, when a
    /// scallop height applies.
    scallop: Option<(f64, f64)>,
    lead: f64,
    tilt: f64,
    direction: Direction,
}

impl Settings {
    fn read(parameters: &Parameters, tool: &RevolvedProfile) -> Result<Self> {
        use parameter::*;
        // The schema has already refused values that are not finite or are
        // out of range (a stepover of zero or less among them).
        let tolerance = parameters.number(names::TOLERANCE)?;
        let stepover = parameters.number(names::STEPOVER)?;
        let leave = parameters.number(names::STOCK_TO_LEAVE)?;
        let height = parameters.number(SCALLOP)?;
        let scallop = (height > 0.0)
            .then(|| scallop::cusp_radius(tool).map(|rho| (height, rho)))
            .flatten();
        let lead = parameters.number(LEAD_ANGLE)?;
        let tilt = parameters.number(MAX_TILT)?;
        let direction = match parameters.text(DIRECTION)? {
            CLIMB => Direction::Climb,
            CONVENTIONAL => Direction::Conventional,
            _ => Direction::Meander,
        };
        Ok(Self {
            tolerance,
            stepover,
            leave,
            scallop,
            lead,
            tilt,
            direction,
        })
    }
}

// ---------------------------------------------------------------------------
// Curves by arc length.

/// A polyline taken by arc-length fraction.
#[derive(Debug, Clone)]
struct Curve {
    points: Vec<Point3>,
    /// Cumulative length at each point.
    cum: Vec<f64>,
}

impl Curve {
    fn new(points: Vec<Point3>) -> Self {
        let mut cum = Vec::with_capacity(points.len());
        let mut total = 0.0;
        let mut last: Option<Point3> = None;
        for p in &points {
            if let Some(q) = last {
                total += p.distance(q);
            }
            cum.push(total);
            last = Some(*p);
        }
        Self { points, cum }
    }

    fn length(&self) -> f64 {
        self.cum.last().copied().unwrap_or(0.0)
    }

    /// The point at fraction `s` of the length.
    fn at(&self, s: f64) -> Option<Point3> {
        let total = self.length();
        let first = *self.points.first()?;
        if total <= 0.0 {
            return Some(first);
        }
        let target = s.clamp(0.0, 1.0) * total;
        let i = self.cum.partition_point(|c| *c < target);
        if i == 0 {
            return Some(first);
        }
        let (a, b) = (*self.points.get(i - 1)?, *self.points.get(i)?);
        let (ca, cb) = (*self.cum.get(i - 1)?, *self.cum.get(i)?);
        let f = if cb > ca {
            (target - ca) / (cb - ca)
        } else {
            0.0
        };
        Some(a + (b - a) * f)
    }

    fn reversed(&self) -> Self {
        Self::new(self.points.iter().rev().copied().collect())
    }
}

// ---------------------------------------------------------------------------
// Loops, sides, the guide and the opposite boundary.

/// One edge of a loop, in the loop's direction.
#[derive(Debug, Clone)]
struct Entry {
    edge: EdgeId,
    points: Vec<Point3>,
}

/// An end of an edge: its vertex, when it has one, and its point.
#[derive(Debug, Clone, Copy)]
struct End {
    vertex: Option<u32>,
    point: Point3,
}

fn same(a: End, b: End) -> bool {
    match (a.vertex, b.vertex) {
        (Some(x), Some(y)) => x == y,
        _ => a.point.distance(b.point) < JOIN,
    }
}

/// The points of an edge from its first vertex to its last: its exact curve
/// sampled when it has one (decision 0227), its polyline otherwise.
fn edge_points(model: &BrepModel, edge: EdgeId) -> Option<(Vec<Point3>, End, End)> {
    let e = model.edges.get(edge.0 as usize)?;
    let points = e
        .curve
        .as_ref()
        .and_then(crate::edge_curve::sample)
        .filter(|p| p.len() >= 2)
        .unwrap_or_else(|| e.polyline.clone());
    let (first, last) = (*points.first()?, *points.last()?);
    let (va, vb) = match e.vertices {
        Some([a, b]) => (Some(a.0), Some(b.0)),
        None => (None, None),
    };
    Some((
        points,
        End {
            vertex: va,
            point: first,
        },
        End {
            vertex: vb,
            point: last,
        },
    ))
}

/// The face's boundary loops, each joined end to end, the lowest edge first
/// and each loop starting at its lowest edge.
fn loops(model: &BrepModel, face: &BrepFace) -> Result<Vec<Vec<Entry>>> {
    let ids: BTreeSet<EdgeId> = face.edges.iter().copied().collect();
    let mut pieces = Vec::new();
    for id in ids {
        let (points, a, b) = edge_points(model, id).ok_or_else(|| {
            PathError::refused(
                crate::topic::FACE_FLOW,
                format!(
                    "edge {} of face {} has no points to follow",
                    id.0, face.id.0
                ),
            )
        })?;
        pieces.push((id, points, a, b));
    }
    let mut used = vec![false; pieces.len()];
    let mut out = Vec::new();
    while let Some(first) = used.iter().position(|u| !*u) {
        let Some((id, points, a, b)) = pieces.get(first).cloned() else {
            break;
        };
        if let Some(u) = used.get_mut(first) {
            *u = true;
        }
        let start = a;
        let mut end = b;
        let mut lp = vec![Entry { edge: id, points }];
        while !same(end, start) {
            let next = pieces.iter().enumerate().find_map(|(j, (_, _, pa, pb))| {
                if used.get(j).copied().unwrap_or(true) {
                    None
                } else if same(*pa, end) {
                    Some((j, false))
                } else if same(*pb, end) {
                    Some((j, true))
                } else {
                    None
                }
            });
            let Some((j, reversed)) = next else {
                return Err(PathError::refused(
                    crate::topic::FACE_FLOW,
                    format!(
                        "the boundary of face {} does not close: its edges are not joined end to \
                     end",
                        face.id.0
                    ),
                ));
            };
            if let Some(u) = used.get_mut(j) {
                *u = true;
            }
            let Some((jid, jpoints, ja, jb)) = pieces.get(j).cloned() else {
                break;
            };
            if reversed {
                end = ja;
                lp.push(Entry {
                    edge: jid,
                    points: jpoints.into_iter().rev().collect(),
                });
            } else {
                end = jb;
                lp.push(Entry {
                    edge: jid,
                    points: jpoints,
                });
            }
        }
        out.push(lp);
    }
    Ok(out)
}

fn entries_length(entries: &[Entry]) -> f64 {
    entries
        .iter()
        .map(|e| Curve::new(e.points.clone()).length())
        .sum()
}

/// The unit direction along `points` at its start (`true`) or end.
fn tangent(points: &[Point3], at_start: bool) -> Option<Vec3> {
    let pick = |a: Point3, b: Point3| {
        let d = b - a;
        let l = d.length();
        (l > 1e-12).then(|| d * (1.0 / l))
    };
    if at_start {
        let a = *points.first()?;
        points.iter().skip(1).find_map(|b| pick(a, *b))
    } else {
        let b = *points.last()?;
        points.iter().rev().skip(1).find_map(|a| pick(*a, b))
    }
}

/// Whether the loop turns by more than [`CORNER_ANGLE`] from `a` to `b`.
fn corner(a: &Entry, b: &Entry) -> bool {
    match (tangent(&a.points, false), tangent(&b.points, true)) {
        (Some(x), Some(y)) => x.dot(y).clamp(-1.0, 1.0).acos().to_degrees() > CORNER_ANGLE,
        _ => true,
    }
}

/// What a field of passes is built between.
#[derive(Debug, Clone)]
struct Boundaries {
    guide: Curve,
    guide_edges: Vec<EdgeId>,
    opposite: Opposite,
}

#[derive(Debug, Clone)]
enum Opposite {
    Side(Curve),
    Point(Point3),
}

impl Opposite {
    fn at(&self, s: f64) -> Option<Point3> {
        match self {
            Self::Side(c) => c.at(s),
            Self::Point(p) => Some(*p),
        }
    }
}

fn concat(entries: &[Entry]) -> Vec<Point3> {
    let mut out: Vec<Point3> = Vec::new();
    for e in entries {
        for p in &e.points {
            if out.last().is_none_or(|q| q.distance(*p) > 1e-12) {
                out.push(*p);
            }
        }
    }
    out
}

/// The sides of `entries` (in order, open), split at corners.
fn sides_of(entries: &[Entry]) -> Vec<Vec<Entry>> {
    let mut sides: Vec<Vec<Entry>> = Vec::new();
    let mut current: Vec<Entry> = Vec::new();
    for e in entries {
        if let Some(last) = current.last()
            && corner(last, e)
        {
            sides.push(std::mem::take(&mut current));
        }
        current.push(e.clone());
    }
    if !current.is_empty() {
        sides.push(current);
    }
    sides
}

/// Orient the opposite boundary so it starts near the guide's start.
fn oriented(guide: &Curve, opposite: Curve) -> Curve {
    let (Some(g0), Some(g1), Some(o0), Some(o1)) = (
        guide.points.first(),
        guide.points.last(),
        opposite.points.first(),
        opposite.points.last(),
    ) else {
        return opposite;
    };
    let keep = g0.distance(*o0) + g1.distance(*o1);
    let flip = g0.distance(*o1) + g1.distance(*o0);
    if flip < keep {
        opposite.reversed()
    } else {
        opposite
    }
}

/// The guide and opposite boundary of `face`, the guide made of `chosen`
/// where any of them bound it.
fn boundaries(model: &BrepModel, face: &BrepFace, chosen: &BTreeSet<EdgeId>) -> Result<Boundaries> {
    let mut all = loops(model, face)?;
    if all.is_empty() {
        return Err(PathError::refused(
            crate::topic::FACE_FLOW,
            format!("face {} has no boundary edges to follow", face.id.0),
        ));
    }
    // The outer loop is the longest; the others are holes, cut round by
    // the face's trim test.
    let mut best = 0;
    let mut best_len = f64::NEG_INFINITY;
    for (i, l) in all.iter().enumerate() {
        let len = entries_length(l);
        if len > best_len * (1.0 + 1e-9) {
            best = i;
            best_len = len;
        }
    }
    let outer = all.swap_remove(best);
    let n = outer.len();
    let mine: Vec<usize> = outer
        .iter()
        .enumerate()
        .filter(|(_, e)| chosen.contains(&e.edge))
        .map(|(i, _)| i)
        .collect();
    let corners: Vec<usize> = (0..n)
        .filter(|&i| match (outer.get(i), outer.get((i + 1) % n)) {
            (Some(a), Some(b)) => n > 1 && corner(a, b),
            _ => false,
        })
        .collect();
    if mine.is_empty() && corners.len() < 2 {
        return Ok(halves(&outer));
    }
    // The guide's entries as a range of the loop, from `g0` for `len`.
    let (g0, len) = if mine.is_empty() {
        // Rotate so the loop starts after a corner, then take its sides.
        let start = corners.first().map_or(0, |c| (c + 1) % n);
        let rotated: Vec<Entry> = (0..n)
            .filter_map(|k| outer.get((start + k) % n).cloned())
            .collect();
        let sides = sides_of(&rotated);
        let mut pick: Option<(usize, f64, EdgeId, usize)> = None;
        let mut offset = 0;
        for side in &sides {
            let length = entries_length(side);
            let lowest = side
                .iter()
                .map(|e| e.edge)
                .min()
                .unwrap_or(EdgeId(u32::MAX));
            let better = match pick {
                None => true,
                Some((_, l, id, _)) => {
                    length > l * (1.0 + 1e-6) || (length >= l * (1.0 - 1e-6) && lowest < id)
                }
            };
            if better {
                pick = Some((offset, length, lowest, side.len()));
            }
            offset += side.len();
        }
        let (at, _, _, count) = pick.unwrap_or((0, 0.0, EdgeId(0), 1));
        ((start + at) % n, count)
    } else {
        contiguous(&mine, n).ok_or_else(|| {
            PathError::refused(
                crate::topic::FACE_FLOW,
                format!(
                    "the guide edges {} of face {} are not joined end to end into one chain",
                    list(mine.iter().filter_map(|i| outer.get(*i)).map(|e| e.edge.0)),
                    face.id.0
                ),
            )
        })?
    };
    if len >= n {
        return Err(PathError::refused(
            crate::topic::FACE_FLOW,
            format!(
                "the guide of face {} is its whole boundary: choose fewer edges, so there is an \
             opposite side to step towards",
                face.id.0
            ),
        ));
    }
    let guide_entries: Vec<Entry> = (0..len)
        .filter_map(|k| outer.get((g0 + k) % n).cloned())
        .collect();
    let rest: Vec<Entry> = (len..n)
        .filter_map(|k| outer.get((g0 + k) % n).cloned())
        .collect();
    let guide = Curve::new(concat(&guide_entries));
    let sides = sides_of(&rest);
    let opposite = match sides.len() {
        0 | 1 => Opposite::Side(oriented(&guide, Curve::new(concat(&rest)))),
        2 => {
            let apex = sides
                .first()
                .and_then(|s| s.last())
                .and_then(|e| e.points.last())
                .copied()
                .ok_or_else(|| {
                    PathError::refused(
                        crate::topic::FACE_FLOW,
                        format!("face {} has an empty side", face.id.0),
                    )
                })?;
            Opposite::Point(apex)
        }
        k => {
            let middle: Vec<Entry> = sides
                .get(1..k - 1)
                .unwrap_or_default()
                .iter()
                .flatten()
                .cloned()
                .collect();
            Opposite::Side(oriented(&guide, Curve::new(concat(&middle))))
        }
    };
    Ok(Boundaries {
        guide,
        guide_edges: guide_entries.iter().map(|e| e.edge).collect(),
        opposite,
    })
}

/// The start and count of `indices` as one run of the cyclic range `0..n`,
/// or `None` when they are not one run.
fn contiguous(indices: &[usize], n: usize) -> Option<(usize, usize)> {
    let set: BTreeSet<usize> = indices.iter().copied().collect();
    let len = set.len();
    if len == 0 {
        return None;
    }
    // The run starts at an index whose predecessor is not in the set.
    let starts: Vec<usize> = set
        .iter()
        .copied()
        .filter(|i| len == n || !set.contains(&((i + n - 1) % n)))
        .collect();
    match starts.as_slice() {
        [start] => Some((*start, len)),
        _ if len == n => Some((0, n)),
        _ => None,
    }
}

/// A smooth loop cut at its two points furthest apart: the longer half is
/// the guide, the other the opposite side.
fn halves(outer: &[Entry]) -> Boundaries {
    let mut points: Vec<(Point3, EdgeId)> = Vec::new();
    for e in outer {
        for p in &e.points {
            if points.last().is_none_or(|(q, _)| q.distance(*p) > 1e-12) {
                points.push((*p, e.edge));
            }
        }
    }
    if points.len() > 1
        && let (Some(a), Some(b)) = (points.first(), points.last())
        && a.0.distance(b.0) < JOIN
    {
        points.pop();
    }
    let step = (points.len() / 400).max(1);
    let candidates: Vec<usize> = (0..points.len()).step_by(step).collect();
    let mut far = (0, 0, f64::NEG_INFINITY);
    for (k, &i) in candidates.iter().enumerate() {
        for &j in candidates.iter().skip(k + 1) {
            if let (Some(a), Some(b)) = (points.get(i), points.get(j)) {
                let d = a.0.distance(b.0);
                if d > far.2 {
                    far = (i, j, d);
                }
            }
        }
    }
    let (i, j, _) = far;
    let n = points.len();
    let first: Vec<(Point3, EdgeId)> = (i..=j).filter_map(|k| points.get(k).copied()).collect();
    let second: Vec<(Point3, EdgeId)> = (j..=i + n)
        .filter_map(|k| points.get(k % n.max(1)).copied())
        .collect();
    let curve = |half: &[(Point3, EdgeId)]| Curve::new(half.iter().map(|p| p.0).collect());
    let (a, b) = (curve(&first), curve(&second));
    let (guide, opposite, edges) = if a.length() >= b.length() {
        (a, b, &first)
    } else {
        (b, a, &second)
    };
    let guide_edges: BTreeSet<EdgeId> = edges.iter().map(|p| p.1).collect();
    let opposite = oriented(&guide, opposite);
    Boundaries {
        guide,
        guide_edges: guide_edges.into_iter().collect(),
        opposite: Opposite::Side(opposite),
    }
}

fn list(ids: impl Iterator<Item = u32>) -> String {
    ids.map(|i| i.to_string()).collect::<Vec<_>>().join(", ")
}

// ---------------------------------------------------------------------------
// The field of passes on one face.

/// The face's parameter period in u and in v, for surfaces that close round
/// an axis.
fn periods(surface: &Surface) -> (Option<f64>, Option<f64>) {
    match surface {
        Surface::Cylinder { .. } | Surface::Cone { .. } | Surface::Sphere { .. } => {
            (Some(TAU), None)
        }
        Surface::Torus { .. } => (Some(TAU), Some(TAU)),
        _ => (None, None),
    }
}

fn unwrap_to(x: f64, near: f64, period: Option<f64>) -> f64 {
    match period {
        Some(p) => x + ((near - x) / p).round() * p,
        None => x,
    }
}

/// One face's passes in its own terms: the face, its boundaries in (u, v)
/// by guide fraction, and the evaluated grid used for the spacing.
struct Field<'a> {
    id: FaceId,
    face: &'a ExactFace,
    boundaries: Boundaries,
    /// (u, v) of the guide and of the opposite boundary at `s = j / m`.
    guide_uv: Vec<Uv>,
    opposite_uv: Vec<Uv>,
}

impl Field<'_> {
    fn m(&self) -> usize {
        self.guide_uv.len().saturating_sub(1)
    }

    /// The (u, v) at guide fraction `s` and weight `w`.
    fn uv(&self, s: f64, w: f64) -> Option<Uv> {
        let m = self.m();
        let x = s.clamp(0.0, 1.0) * m as f64;
        let j = (x.floor() as usize).min(m.saturating_sub(1));
        let f = x - j as f64;
        let lerp = |v: &[Uv]| -> Option<Uv> {
            let (a, b) = (v.get(j)?, v.get((j + 1).min(m))?);
            Some(Uv {
                u: a.u + (b.u - a.u) * f,
                v: a.v + (b.v - a.v) * f,
            })
        };
        let (g, o) = (lerp(&self.guide_uv)?, lerp(&self.opposite_uv)?);
        Some(Uv {
            u: g.u + (o.u - g.u) * w,
            v: g.v + (o.v - g.v) * w,
        })
    }

    /// The face's point and normal at (s, w); on the face's boundary by
    /// construction where `s` or `w` is 0 or 1, otherwise only inside its
    /// trim.
    fn at(&self, s: f64, w: f64) -> Option<SurfacePoint> {
        let uv = self.uv(s, w)?;
        let edge = s <= 0.0 || s >= 1.0 || w <= 0.0 || w >= 1.0;
        if !edge && !self.face.contains(uv) {
            return None;
        }
        self.face.point_normal(uv)
    }
}

/// Puts the boundary samples on the face.
fn field<'a>(
    model: &BrepModel,
    face_id: FaceId,
    face: &'a ExactFace,
    boundaries: Boundaries,
) -> Result<Field<'a>> {
    let brep = model.faces.get(face_id.0 as usize).ok_or_else(|| {
        PathError::refused(
            crate::topic::FACE_FLOW,
            format!("the model has no face {}", face_id.0),
        )
    })?;
    let (pu, pv) = periods(&brep.geometry);
    let length = boundaries.guide.length().max(match &boundaries.opposite {
        Opposite::Side(c) => c.length(),
        Opposite::Point(_) => 0.0,
    });
    let m = ((length / SAMPLE_SPACING).ceil() as usize).clamp(16, MAX_GUIDE_SAMPLES);
    let place = |p: Point3, seed: Option<Uv>| -> Result<Uv> {
        let found = match seed {
            Some(seed) => face.closest_from(p, seed, f64::INFINITY),
            None => face.closest(p),
        };
        found.map(|c| c.uv).ok_or_else(|| {
            PathError::refused(
                crate::topic::FACE_FLOW,
                format!(
                    "the boundary of face {} could not be put on its surface",
                    face_id.0
                ),
            )
        })
    };
    let mut guide_uv: Vec<Uv> = Vec::with_capacity(m + 1);
    let mut opposite_uv: Vec<Uv> = Vec::with_capacity(m + 1);
    for j in 0..=m {
        let s = j as f64 / m as f64;
        let g = boundaries.guide.at(s).ok_or_else(|| {
            PathError::refused(
                crate::topic::FACE_FLOW,
                format!("the guide of face {} is empty", face_id.0),
            )
        })?;
        let o = boundaries.opposite.at(s).ok_or_else(|| {
            PathError::refused(
                crate::topic::FACE_FLOW,
                format!("the opposite side of face {} is empty", face_id.0),
            )
        })?;
        let mut gu = place(g, guide_uv.last().copied())?;
        let mut ou = match (&boundaries.opposite, opposite_uv.last()) {
            (Opposite::Point(_), Some(prev)) => *prev,
            (_, prev) => place(o, prev.copied())?,
        };
        // Keep each boundary continuous across a seam, and the opposite
        // boundary on the same turn as the guide.
        if let Some(prev) = guide_uv.last() {
            gu.u = unwrap_to(gu.u, prev.u, pu);
            gu.v = unwrap_to(gu.v, prev.v, pv);
        }
        if let Some(prev) = opposite_uv.last() {
            ou.u = unwrap_to(ou.u, prev.u, pu);
            ou.v = unwrap_to(ou.v, prev.v, pv);
        } else {
            ou.u = unwrap_to(ou.u, gu.u, pu);
            ou.v = unwrap_to(ou.v, gu.v, pv);
        }
        guide_uv.push(gu);
        opposite_uv.push(ou);
    }
    Ok(Field {
        id: face_id,
        face,
        boundaries,
        guide_uv,
        opposite_uv,
    })
}

/// The face sampled across, at a few places along the guide, for the pass
/// spacing.
struct Across {
    /// At each place: the points and normals at `w = i / n`, and the length
    /// along the face from `w = 0`.
    places: Vec<Vec<Option<(Point3, Vec3, f64)>>>,
    n: usize,
}

impl Across {
    fn new(field: &Field<'_>) -> Self {
        let mut width: f64 = 0.0;
        for k in 0..SPACING_PLACES {
            let s = k as f64 / (SPACING_PLACES - 1) as f64;
            if let (Some(a), Some(b)) = (field.at(s, 0.0), field.at(s, 1.0)) {
                width = width.max(a.point.distance(b.point));
            }
        }
        let n = ((width / ACROSS_SPACING).ceil() as usize).clamp(MIN_ACROSS, MAX_ACROSS);
        let places = (0..SPACING_PLACES)
            .map(|k| {
                let s = k as f64 / (SPACING_PLACES - 1) as f64;
                let mut along = 0.0;
                let mut last: Option<Point3> = None;
                (0..=n)
                    .map(|i| {
                        let w = i as f64 / n as f64;
                        let sp = field.at(s, w)?;
                        if let Some(q) = last {
                            along += sp.point.distance(q);
                        }
                        last = Some(sp.point);
                        Some((sp.point, sp.normal.get(), along))
                    })
                    .collect()
            })
            .collect();
        Self { places, n }
    }

    /// The point, normal and length along at place `k` and weight `w`,
    /// interpolated.
    fn at(&self, k: usize, w: f64) -> Option<(Point3, Vec3, f64)> {
        let row = self.places.get(k)?;
        let x = w.clamp(0.0, 1.0) * self.n as f64;
        let i = (x.floor() as usize).min(self.n.saturating_sub(1));
        let f = x - i as f64;
        let (a, b) = ((*row.get(i)?)?, (*row.get(i + 1)?)?);
        let n = a.1 + (b.1 - a.1) * f;
        let len = n.length();
        Some((
            a.0 + (b.0 - a.0) * f,
            if len > 0.0 { n * (1.0 / len) } else { a.1 },
            a.2 + (b.2 - a.2) * f,
        ))
    }

    /// How far the step from `wa` to `wb` uses the allowance, at its worst
    /// place: 1 is the stepover or the scallop height exactly.
    fn ratio(&self, wa: f64, wb: f64, settings: &Settings) -> f64 {
        let mut worst: f64 = 0.0;
        for k in 0..self.places.len() {
            let (Some(a), Some(b)) = (self.at(k, wa), self.at(k, wb)) else {
                continue;
            };
            worst = worst.max((b.2 - a.2).abs() / settings.stepover);
            if let Some((height, rho)) = settings.scallop
                && let Some(m) = self.at(k, 0.5 * (wa + wb))
            {
                let (ca, cb) = (a.0 + a.1 * rho, b.0 + b.1 * rho);
                let d = ca.distance(cb);
                let cusp = if d >= 2.0 * rho {
                    f64::INFINITY
                } else {
                    let down = -(a.1 + b.1);
                    let dl = down.length();
                    if dl <= 1e-12 {
                        0.0
                    } else {
                        let mid = ca + (cb - ca) * 0.5;
                        let q = mid + down * ((rho * rho - 0.25 * d * d).sqrt() / dl);
                        (q - m.0).dot(m.1)
                    }
                };
                worst = worst.max(cusp / height);
            }
        }
        worst
    }

    /// The width of the face across at guide fraction `s`, along the face.
    fn width(&self, s: f64) -> f64 {
        let k = (s.clamp(0.0, 1.0) * (self.places.len().saturating_sub(1)) as f64).round() as usize;
        self.at(k, 1.0).map_or(f64::INFINITY, |p| p.2)
    }
}

/// The weights of the passes with each step using at most `limit` of the
/// allowance; `None` when there are more than [`MAX_PASSES`].
fn greedy(across: &Across, settings: &Settings, limit: f64) -> Result<Option<Vec<f64>>> {
    let mut weights = vec![0.0];
    let mut w = 0.0;
    while w < 1.0 {
        if weights.len() > MAX_PASSES {
            return Ok(None);
        }
        if across.ratio(w, 1.0, settings) <= limit {
            weights.push(1.0);
            break;
        }
        let (mut lo, mut hi) = (w, 1.0);
        for _ in 0..40 {
            let mid = 0.5 * (lo + hi);
            if across.ratio(w, mid, settings) <= limit {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        if lo - w < 1e-12 {
            return Err(PathError::parameter(
                names::STEPOVER,
                "the stepover (or scallop height) is too small for this face to be cut",
            ));
        }
        weights.push(lo);
        w = lo;
    }
    Ok(Some(weights))
}

/// The weights of the passes, as few as the allowance needs and evened out.
fn weights(across: &Across, settings: &Settings, face: FaceId) -> Result<Vec<f64>> {
    let too_many = || {
        PathError::refused(
            crate::topic::FACE_FLOW,
            format!(
                "face {} would need more than {MAX_PASSES} passes: raise the stepover",
                face.0
            ),
        )
        .with_setting(crate::params::names::STEPOVER)
    };
    // At least the widest place's width over the stepover: refuse at once
    // what cannot be cut in the limit, rather than search for it.
    let widest = (0..across.places.len())
        .filter_map(|k| across.at(k, 1.0).map(|p| p.2))
        .fold(0.0, f64::max);
    if widest / settings.stepover > MAX_PASSES as f64 {
        return Err(too_many());
    }
    let first = greedy(across, settings, 1.0)?.ok_or_else(too_many)?;
    let gaps = first.len() - 1;
    if gaps < 2 {
        return Ok(first);
    }
    // The smallest share of the allowance that still needs no more passes:
    // the gaps come out nearly equal.
    let (mut lo, mut hi) = (0.9 * (gaps - 1) as f64 / gaps as f64, 1.0);
    let mut best = first;
    for _ in 0..14 {
        let mid = 0.5 * (lo + hi);
        match greedy(across, settings, mid)? {
            Some(w) if w.len() - 1 <= gaps => {
                hi = mid;
                best = w;
            }
            _ => lo = mid,
        }
    }
    Ok(best)
}

// ---------------------------------------------------------------------------
// The tool along a pass.

/// One placed point of a pass.
#[derive(Debug, Clone, Copy)]
struct Pose {
    s: f64,
    point: CutPoint,
}

/// What became of one sample of a pass.
#[derive(Debug, Clone, Copy)]
enum Sample {
    Cut(Pose),
    /// The tool cannot reach the point without gouging the part elsewhere.
    Blocked {
        point: Point3,
        a: f64,
        s: f64,
    },
    /// The region (avoid or check set) forbids a cut here, or the face has
    /// no point here.
    Skipped,
}

/// What the tool needs that does not change along a pass.
struct Tooling<'a> {
    input: &'a StrategyInput<'a>,
    region: &'a RegionConstraint<'a>,
    part: &'a BrepSurface,
    profile: &'a RevolvedProfile,
    settings: Settings,
    sense: f64,
}

impl Tooling<'_> {
    fn axis(&self, phi: f64) -> (Degrees, UnitVec3) {
        let a = Degrees(self.sense * (90.0 - phi));
        (a, self.input.rotary.tool_axis(a))
    }

    /// The tip for the tool's point with outward normal `-n` to sit on `p`.
    fn seed(&self, p: Point3, n: Vec3, axis: UnitVec3) -> Point3 {
        let t = axis.get();
        let d = -n;
        let along = d.dot(t);
        let radial = d - t * along;
        let b = radial.length();
        let angle = along.atan2(b).min(0.0);
        let Some(q) = self.profile.point_for_normal(angle) else {
            return p;
        };
        let e = if b > 1e-12 {
            radial * (1.0 / b)
        } else {
            Vec3::ZERO
        };
        p + (-(t * q.height.0 + e * q.radius.0))
    }

    /// The tool touching `p` (normal `n`) at angle `phi`.
    fn place(&self, s: f64, p: Point3, n: Vec3, phi: f64) -> Sample {
        let (a, axis) = self.axis(phi);
        let seed = self.seed(p, n, axis);
        let line = ToolLine { origin: seed, axis };
        let Some(contact) = self.part.drop_tool(self.profile, &line) else {
            return Sample::Skipped;
        };
        let lift = (contact.tip - seed).dot(axis.get());
        if lift > self.settings.tolerance {
            return Sample::Blocked {
                point: p,
                a: a.0,
                s,
            };
        }
        if !self
            .region
            .allows(contact.contact, contact.tip, self.profile, &line)
        {
            return Sample::Skipped;
        }
        Sample::Cut(Pose {
            s,
            point: self.cut_point(contact.tip, contact.contact, a, axis),
        })
    }

    fn cut_point(
        &self,
        grown_tip: Point3,
        contact: Point3,
        a: Degrees,
        axis: UnitVec3,
    ) -> CutPoint {
        let leave = self.settings.leave;
        let tip = grown_tip + axis.get() * leave;
        let contact = if leave > 0.0 {
            cutter::cutter_contact(self.input.tool, tip, axis, contact)
        } else {
            contact
        };
        CutPoint { tip, a, contact }
    }

    /// How far the straight machine move from `a` to `b` leaves the part at
    /// its middle and quarter points, along the tool axis: positive into it.
    fn move_error(&self, a: &CutPoint, b: &CutPoint) -> f64 {
        let rotary = self.input.rotary;
        let pa = rotary.to_axis(a.tip, a.a);
        let pb = rotary.to_axis(b.tip, b.a);
        let bb = nearest_turn(b.a, a.a);
        let mut worst: f64 = 0.0;
        for f in MOVE_CHECKS {
            let at = Degrees(a.a.0 + (bb.0 - a.a.0) * f);
            let pos = crate::rotary::AxisPosition {
                along: pa.along + (pb.along - pa.along) * f,
                across: pa.across + (pb.across - pa.across) * f,
                height: pa.height + (pb.height - pa.height) * f,
            };
            let tip = rotary.to_model(pos, at);
            let axis = rotary.tool_axis(at);
            let grown = tip + axis.get() * -self.settings.leave;
            let line = ToolLine {
                origin: grown,
                axis,
            };
            if let Some(c) = self.part.drop_tool(self.profile, &line) {
                let e = (c.tip - grown).dot(axis.get());
                if e.abs() > worst.abs() {
                    worst = e;
                }
            }
        }
        worst
    }
}

/// The polar angle, degrees, of the normal's part across the rotary axis;
/// `None` along it.
fn polar(n: Vec3) -> Option<f64> {
    (n.y.abs() > 1e-12 || n.z.abs() > 1e-12).then(|| n.z.atan2(n.y).to_degrees())
}

/// The angle equal to `x` by whole turns nearest `near`.
fn near(x: f64, near: f64) -> f64 {
    x + ((near - x) / 360.0).round() * 360.0
}

/// One pass's samples, in guide order.
fn pass_samples(field: &Field<'_>, tooling: &Tooling<'_>, w: f64) -> Vec<Sample> {
    // The base samples: the face's points along the pass, refined where the
    // chord leaves the face by more than the tolerance.
    let m = field.m();
    let mut base: Vec<(f64, Option<SurfacePoint>)> = Vec::new();
    for j in 0..=m {
        let s = j as f64 / m as f64;
        let sp = field.at(s, w);
        if let (Some((s0, Some(a))), Some(b)) = (base.last().copied(), sp) {
            refine_chord(
                field,
                w,
                (s0, a),
                (s, b),
                tooling.settings.tolerance,
                0,
                &mut base,
            );
        }
        base.push((s, sp));
    }
    // Travel round the axis, for the lead, and the axis plan.
    let points: Vec<Option<Point3>> = base.iter().map(|(_, sp)| sp.map(|p| p.point)).collect();
    let mut phi_n: Vec<Option<f64>> = Vec::with_capacity(base.len());
    let mut last: Option<f64> = None;
    for (_, sp) in &base {
        let phi = sp.and_then(|p| polar(p.normal.get())).map(|x| match last {
            Some(prev) => near(x, prev),
            None => x,
        });
        if phi.is_some() {
            last = phi;
        }
        phi_n.push(phi);
    }
    // Where the normal is along the rotary axis the tool cannot face it: it
    // takes the angle of the nearest point that it can face, or, on a pass
    // with none, comes from outside the point (the drop then says whether
    // it reaches).
    let known: Vec<(usize, f64)> = phi_n
        .iter()
        .enumerate()
        .filter_map(|(i, p)| p.map(|p| (i, p)))
        .collect();
    for (i, slot) in phi_n.iter_mut().enumerate() {
        if slot.is_some() {
            continue;
        }
        let nearest = known
            .iter()
            .min_by_key(|(k, _)| k.abs_diff(i))
            .map(|(_, p)| *p);
        let outside = base
            .get(i)
            .and_then(|(_, sp)| sp.as_ref())
            .and_then(|p| polar(p.point.to_vec()));
        *slot = nearest.or(outside);
    }
    let settings = tooling.settings;
    let mut target: Vec<Option<f64>> = Vec::with_capacity(base.len());
    for (i, phi) in phi_n.iter().enumerate() {
        let Some(phi) = *phi else {
            target.push(None);
            continue;
        };
        // The direction of travel, from the face itself a hair either side.
        let s = base.get(i).map_or(0.0, |b| b.0);
        let travel = match (
            field.at((s - TANGENT_STEP).max(0.0), w),
            field.at((s + TANGENT_STEP).min(1.0), w),
        ) {
            (Some(a), Some(b)) => b.point - a.point,
            _ => Vec3::ZERO,
        };
        let len = travel.length();
        let r = phi.to_radians();
        let round = Vec3::new(0.0, -r.sin(), r.cos());
        let share = if len > 0.0 {
            travel.dot(round) / len
        } else {
            0.0
        };
        target.push(Some(phi + settings.lead * share));
    }
    // Smooth A within the tilt band, along the length of each stretch of
    // the pass that has points (OP-15; as simultaneous finishing).
    let phi = smooth(&base, &points, &target, settings.tilt);
    let mut out = Vec::with_capacity(base.len());
    for (i, (s, sp)) in base.iter().enumerate() {
        match (sp, phi.get(i).copied().flatten()) {
            (Some(p), Some(phi)) => out.push(tooling.place(*s, p.point, p.normal.get(), phi)),
            (Some(p), None) => out.push(Sample::Blocked {
                point: p.point,
                a: 0.0,
                s: *s,
            }),
            _ => out.push(Sample::Skipped),
        }
    }
    refine_moves(field, tooling, w, &base, &phi, out)
}

fn refine_chord(
    field: &Field<'_>,
    w: f64,
    a: (f64, SurfacePoint),
    b: (f64, SurfacePoint),
    tolerance: f64,
    depth: u32,
    out: &mut Vec<(f64, Option<SurfacePoint>)>,
) {
    if depth >= MAX_DEPTH {
        return;
    }
    let s = 0.5 * (a.0 + b.0);
    let Some(m) = field.at(s, w) else {
        return;
    };
    let chord = a.1.point + (b.1.point - a.1.point) * 0.5;
    if chord.distance(m.point) <= tolerance {
        return;
    }
    refine_chord(field, w, a, (s, m), tolerance, depth + 1, out);
    out.push((s, Some(m)));
    refine_chord(field, w, (s, m), b, tolerance, depth + 1, out);
}

/// The smoothed angle at each base sample.
fn smooth(
    base: &[(f64, Option<SurfacePoint>)],
    points: &[Option<Point3>],
    target: &[Option<f64>],
    tilt: f64,
) -> Vec<Option<f64>> {
    let mut out: Vec<Option<f64>> = vec![None; base.len()];
    let mut i = 0;
    while i < base.len() {
        if target.get(i).copied().flatten().is_none() {
            i += 1;
            continue;
        }
        let start = i;
        while i < base.len() && target.get(i).copied().flatten().is_some() {
            i += 1;
        }
        // The stretch `start..i`: x by length along it.
        let mut x = Vec::with_capacity(i - start);
        let mut lo = Vec::with_capacity(i - start);
        let mut hi = Vec::with_capacity(i - start);
        let mut idx = Vec::with_capacity(i - start);
        let mut along = 0.0;
        let mut last: Option<Point3> = None;
        for k in start..i {
            let (Some(p), Some(t)) = (
                points.get(k).copied().flatten(),
                target.get(k).copied().flatten(),
            ) else {
                continue;
            };
            if let Some(q) = last {
                along += p.distance(q);
            }
            if x.last().is_some_and(|l: &f64| along <= *l) {
                last = Some(p);
                continue;
            }
            last = Some(p);
            x.push(along);
            lo.push(t - tilt);
            hi.push(t + tilt);
            idx.push(k);
        }
        let (Some(first), Some(end)) = (
            idx.first().and_then(|k| target.get(*k).copied().flatten()),
            idx.last().and_then(|k| target.get(*k).copied().flatten()),
        ) else {
            continue;
        };
        if x.len() < 2 {
            for k in &idx {
                if let Some(slot) = out.get_mut(*k) {
                    *slot = target.get(*k).copied().flatten();
                }
            }
            continue;
        }
        let string = crate::simultaneous::taut_string(&x, &lo, &hi, first, end);
        for (n, k) in idx.iter().enumerate() {
            let Some(xi) = x.get(n) else { continue };
            let v = interpolate(&string, *xi);
            let (Some(l), Some(h)) = (lo.get(n), hi.get(n)) else {
                continue;
            };
            if let Some(slot) = out.get_mut(*k) {
                *slot = v.map(|v| v.clamp(*l, *h));
            }
        }
        // Samples dropped as coincident take their neighbour's angle.
        for k in start..i {
            if out.get(k).copied().flatten().is_none() {
                let v = (start..k).rev().find_map(|j| out.get(j).copied().flatten());
                if let Some(slot) = out.get_mut(k) {
                    *slot = v.or_else(|| target.get(k).copied().flatten());
                }
            }
        }
    }
    out
}

fn interpolate(vertices: &[(f64, f64)], x: f64) -> Option<f64> {
    let i = vertices.partition_point(|v| v.0 < x);
    match (
        i.checked_sub(1).and_then(|k| vertices.get(k)),
        vertices.get(i),
    ) {
        (Some(a), Some(b)) if b.0 > a.0 => Some(a.1 + (b.1 - a.1) * (x - a.0) / (b.0 - a.0)),
        (_, Some(b)) => Some(b.1),
        (Some(a), None) => Some(a.1),
        _ => None,
    }
}

/// The smoothed angle at guide fraction `s`, from the base samples.
fn phi_at(base: &[(f64, Option<SurfacePoint>)], phi: &[Option<f64>], s: f64) -> Option<f64> {
    let i = base.partition_point(|b| b.0 < s);
    let a = i
        .checked_sub(1)
        .and_then(|k| Some((base.get(k)?.0, phi.get(k).copied().flatten()?)));
    let b = base
        .get(i)
        .and_then(|x| Some((x.0, phi.get(i).copied().flatten()?)));
    match (a, b) {
        (Some(a), Some(b)) if b.0 > a.0 => {
            let bb = near(b.1, a.1);
            Some(a.1 + (bb - a.1) * (s - a.0) / (b.0 - a.0))
        }
        (Some(a), _) => Some(a.1),
        (_, Some(b)) => Some(b.1),
        _ => None,
    }
}

/// Splits each move that leaves the part by more than the tolerance at its
/// middle or quarter points, by placing the tool between its ends; a move
/// that still cuts in after [`MAX_DEPTH`] halvings is broken.
fn refine_moves(
    field: &Field<'_>,
    tooling: &Tooling<'_>,
    w: f64,
    base: &[(f64, Option<SurfacePoint>)],
    phi: &[Option<f64>],
    samples: Vec<Sample>,
) -> Vec<Sample> {
    let mut out: Vec<Sample> = Vec::with_capacity(samples.len());
    for sample in samples {
        if let (Some(Sample::Cut(a)), Sample::Cut(b)) = (out.last().copied(), sample) {
            split_move(field, tooling, w, base, phi, a, b, 0, &mut out);
        }
        out.push(sample);
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn split_move(
    field: &Field<'_>,
    tooling: &Tooling<'_>,
    w: f64,
    base: &[(f64, Option<SurfacePoint>)],
    phi: &[Option<f64>],
    a: Pose,
    b: Pose,
    depth: u32,
    out: &mut Vec<Sample>,
) {
    let error = tooling.move_error(&a.point, &b.point);
    if error.abs() <= tooling.settings.tolerance {
        return;
    }
    if depth >= MAX_DEPTH {
        if error > tooling.settings.tolerance {
            // Still cutting in: break the pass here rather than gouge.
            out.push(Sample::Skipped);
        }
        return;
    }
    let s = 0.5 * (a.s + b.s);
    let (Some(sp), Some(angle)) = (field.at(s, w), phi_at(base, phi, s)) else {
        out.push(Sample::Skipped);
        return;
    };
    let middle = tooling.place(s, sp.point, sp.normal.get(), angle);
    match middle {
        Sample::Cut(m) => {
            split_move(field, tooling, w, base, phi, a, m, depth + 1, out);
            out.push(middle);
            split_move(field, tooling, w, base, phi, m, b, depth + 1, out);
        }
        other => out.push(other),
    }
}

// ---------------------------------------------------------------------------
// The whole operation.

/// A stretch of a pass the tool cannot reach.
#[derive(Debug, Clone)]
struct Stretch {
    face: FaceId,
    pass: usize,
    s: (f64, f64),
    points: Vec<Point3>,
    a: Vec<f64>,
    reason: UnreachedReason,
    /// Its area: its length times the width it stands for across.
    area: f64,
}

/// One face's cut: its passes in guide order, each as its runs, and what
/// cannot be reached.
struct FaceCut {
    id: FaceId,
    passes: Vec<Vec<Vec<CutPoint>>>,
    stretches: Vec<Stretch>,
    guide_edges: Vec<EdgeId>,
    guide_length: f64,
    point_opposite: bool,
    dropped_short: usize,
    spacing: f64,
}

#[allow(clippy::too_many_lines)]
fn face_flow(
    selection: &Selection,
    input: &StrategyInput<'_>,
    region: &RegionConstraint<'_>,
    parameters: &Parameters,
    context: &JobContext<'_>,
) -> Result<StrategyOutput> {
    let model = selection.model.as_ref();
    let settings = Settings::read(parameters, input.tool)?;
    let faces: BTreeSet<FaceId> = selection.faces.iter().copied().collect();
    if faces.is_empty() {
        return Err(PathError::refused(
            crate::topic::FACE_FLOW,
            "face-flow finishing needs at least one face to cut: select the faces".to_owned(),
        ));
    }
    if let Some(bad) = faces.iter().find(|f| f.0 as usize >= model.faces.len()) {
        return Err(PathError::refused(
            crate::topic::FACE_FLOW,
            format!("the model has no face {}", bad.0),
        ));
    }
    let guide: BTreeSet<EdgeId> = selection.guide.iter().copied().collect();
    if let Some(bad) = guide.iter().find(|e| e.0 as usize >= model.edges.len()) {
        return Err(PathError::refused(
            crate::topic::FACE_FLOW,
            format!("the model has no edge {} to use as the guide", bad.0),
        ));
    }
    if let Some(bad) = guide.iter().find(|e| {
        !faces.iter().any(|f| {
            model
                .faces
                .get(f.0 as usize)
                .is_some_and(|face| face.edges.contains(e))
        })
    }) {
        return Err(PathError::refused(
            crate::topic::FACE_FLOW,
            format!(
                "guide edge {} is not on the boundary of the selected faces: choose edges of the \
             faces to cut",
                bad.0
            ),
        ));
    }
    if !input.tool.is_convex() {
        return Err(PathError::refused(
            crate::topic::FACE_FLOW,
            "face-flow finishing needs a tool whose profile is convex".to_owned(),
        ));
    }
    let part = BrepSurface::new(model)?.with_exact_drops(true);
    let profile = cutter::grow_profile(input.tool, settings.leave)?;
    let tooling = Tooling {
        input,
        region,
        part: &part,
        profile: &profile,
        settings,
        sense: input.rotary.sense.sign(),
    };
    // The faces' evaluators, in ID order.
    let exact: Vec<(FaceId, ExactFace)> = faces
        .iter()
        .map(|id| {
            let face = model.faces.get(id.0 as usize).ok_or_else(|| {
                PathError::refused(
                    crate::topic::FACE_FLOW,
                    format!("the model has no face {}", id.0),
                )
            })?;
            Ok((*id, ExactFace::new(face)?))
        })
        .collect::<Result<_>>()?;
    let mut fields = Vec::with_capacity(exact.len());
    for (id, face) in &exact {
        context.check_cancelled()?;
        let brep = model.faces.get(id.0 as usize).ok_or_else(|| {
            PathError::refused(
                crate::topic::FACE_FLOW,
                format!("the model has no face {}", id.0),
            )
        })?;
        let b = boundaries(model, brep, &guide)?;
        fields.push(field(model, *id, face, b)?);
    }
    let order = face_order(model, &fields);
    let total = fields.len();
    let mut cuts: Vec<FaceCut> = Vec::with_capacity(total);
    for (done, index) in order.iter().enumerate() {
        context.check_cancelled()?;
        context.report("face-flow finishing", done as u64, Some(total as u64));
        let Some(f) = fields.get(*index) else {
            continue;
        };
        cuts.push(cut_face(f, &tooling, &profile, context)?);
    }
    context.report("face-flow finishing", total as u64, Some(total as u64));
    assemble(cuts, &tooling, input, context)
}

/// The order the faces are cut in (decision 0525 D2): the face with the
/// longest guide first (the lowest ID on a tie), then a face sharing an
/// edge with the face just cut, then with any face cut, lowest ID first.
fn face_order(model: &BrepModel, fields: &[Field<'_>]) -> Vec<usize> {
    let edges_of = |f: &Field<'_>| -> BTreeSet<EdgeId> {
        model
            .faces
            .get(f.id.0 as usize)
            .map(|face| face.edges.iter().copied().collect())
            .unwrap_or_default()
    };
    let edge_sets: Vec<BTreeSet<EdgeId>> = fields.iter().map(edges_of).collect();
    let shares = |i: usize, j: usize| match (edge_sets.get(i), edge_sets.get(j)) {
        (Some(a), Some(b)) => a.intersection(b).next().is_some(),
        _ => false,
    };
    let start = |left: &BTreeSet<usize>| -> Option<usize> {
        let mut best: Option<(usize, f64)> = None;
        for &i in left {
            let len = fields.get(i).map_or(0.0, |f| f.boundaries.guide.length());
            if best.is_none_or(|(_, l)| len > l * (1.0 + 1e-6)) {
                best = Some((i, len));
            }
        }
        best.map(|b| b.0)
    };
    let mut left: BTreeSet<usize> = (0..fields.len()).collect();
    let mut order: Vec<usize> = Vec::with_capacity(fields.len());
    while !left.is_empty() {
        let next = order
            .last()
            .and_then(|&last| left.iter().copied().find(|&i| shares(last, i)))
            .or_else(|| {
                left.iter()
                    .copied()
                    .find(|&i| order.iter().any(|&o| shares(o, i)))
            })
            .or_else(|| start(&left));
        let Some(next) = next else { break };
        left.remove(&next);
        order.push(next);
    }
    order
}

fn cut_face(
    f: &Field<'_>,
    tooling: &Tooling<'_>,
    profile: &RevolvedProfile,
    context: &JobContext<'_>,
) -> Result<FaceCut> {
    let across = Across::new(f);
    let weights = weights(&across, &tooling.settings, f.id)?;
    let diameter = 2.0 * profile.cutting_radius();
    let mut passes: Vec<Vec<Vec<CutPoint>>> = Vec::with_capacity(weights.len());
    let mut stretches: Vec<Stretch> = Vec::new();
    let mut dropped_short = 0;
    let point_opposite = matches!(f.boundaries.opposite, Opposite::Point(_));
    let spacing = weights
        .windows(2)
        .filter_map(|w| match w {
            [a, b] => Some(
                (0..across.places.len())
                    .filter_map(|k| Some((across.at(k, *b)?.2 - across.at(k, *a)?.2).abs()))
                    .fold(0.0, f64::max),
            ),
            _ => None,
        })
        .fold(0.0, f64::max);
    for (k, &w) in weights.iter().enumerate() {
        context.check_cancelled()?;
        let samples = pass_samples(f, tooling, w);
        // A pass towards a point opposite shorter than the tool is left out.
        if point_opposite && w > 0.0 {
            let pts: Vec<Point3> = [0.0, 1.0]
                .iter()
                .filter_map(|s| f.at(*s, w))
                .map(|p| p.point)
                .collect();
            if let [a, b] = pts.as_slice()
                && a.distance(*b) < diameter
            {
                dropped_short += 1;
                continue;
            }
        }
        // The width each pass stands for across the face.
        let gap = |i: Option<usize>| i.and_then(|i| weights.get(i)).copied();
        let below = gap(k.checked_sub(1)).map_or(0.0, |x| w - x);
        let above = gap(Some(k + 1)).map_or(0.0, |x| x - w);
        let share = 0.5 * (below + above);
        let mut runs: Vec<Vec<CutPoint>> = Vec::new();
        let mut run: Vec<CutPoint> = Vec::new();
        let mut blocked: Vec<(Point3, f64, f64)> = Vec::new();
        let flush_blocked = |blocked: &mut Vec<(Point3, f64, f64)>,
                             stretches: &mut Vec<Stretch>| {
            if blocked.is_empty() {
                return;
            }
            let pts: Vec<Point3> = blocked.iter().map(|b| b.0).collect();
            let length = Curve::new(pts.clone()).length();
            let (s0, s1) = (
                blocked.first().map_or(0.0, |b| b.2),
                blocked.last().map_or(0.0, |b| b.2),
            );
            let mid = 0.5 * (s0 + s1);
            let width = across.width(mid);
            // Its width across: the share of the face's width it stands for.
            let reason = if width < diameter {
                UnreachedReason::TooNarrow
            } else {
                UnreachedReason::Blocked
            };
            stretches.push(Stretch {
                face: f.id,
                pass: k,
                s: (s0, s1),
                points: pts,
                a: blocked.iter().map(|b| b.1).collect(),
                reason,
                area: length.max(SAMPLE_SPACING) * share * width,
            });
            blocked.clear();
        };
        for sample in samples {
            match sample {
                Sample::Cut(pose) => {
                    flush_blocked(&mut blocked, &mut stretches);
                    run.push(pose.point);
                }
                Sample::Blocked { point, a, s } => {
                    if run.len() >= 2 {
                        runs.push(std::mem::take(&mut run));
                    } else {
                        run.clear();
                    }
                    blocked.push((point, a, s));
                }
                Sample::Skipped => {
                    flush_blocked(&mut blocked, &mut stretches);
                    if run.len() >= 2 {
                        runs.push(std::mem::take(&mut run));
                    } else {
                        run.clear();
                    }
                }
            }
        }
        flush_blocked(&mut blocked, &mut stretches);
        if run.len() >= 2 {
            runs.push(run);
        }
        passes.push(runs);
    }
    Ok(FaceCut {
        id: f.id,
        passes,
        stretches,
        guide_edges: f.boundaries.guide_edges.clone(),
        guide_length: f.boundaries.guide.length(),
        point_opposite,
        dropped_short,
        spacing,
    })
}

/// Whether the uncut material, towards `next`, is on the tool's right
/// looking along `run`'s feed from the spindle.
fn material_on_right(run: &[CutPoint], next: Vec3, rotary: &RotaryMapping) -> Option<bool> {
    let i = run.len() / 2;
    let (a, b) = (run.get(i.checked_sub(1)?)?, run.get(i)?);
    let feed = b.contact - a.contact;
    let right = feed.cross(rotary.tool_axis(b.a).get());
    let side = next.dot(right);
    (side.abs() > 1e-12).then_some(side > 0.0)
}

fn centre(runs: &[Vec<CutPoint>]) -> Option<Point3> {
    let pts: Vec<Point3> = runs.iter().flatten().map(|c| c.contact).collect();
    let n = pts.len();
    (n > 0).then(|| {
        let s = pts.iter().fold(Vec3::ZERO, |s, p| s + p.to_vec());
        Point3::ORIGIN + s * (1.0 / n as f64)
    })
}

fn reverse(runs: &mut [Vec<CutPoint>]) {
    runs.reverse();
    for r in runs.iter_mut() {
        r.reverse();
    }
}

fn assemble(
    cuts: Vec<FaceCut>,
    tooling: &Tooling<'_>,
    input: &StrategyInput<'_>,
    context: &JobContext<'_>,
) -> Result<StrategyOutput> {
    let settings = tooling.settings;
    let mut out_passes: Vec<Pass> = Vec::new();
    let mut notes: Vec<Notice> = Vec::new();
    let mut stretches: Vec<Stretch> = Vec::new();
    let mut face_notes: Vec<String> = Vec::new();
    for mut cut in cuts {
        let geometric = cut.passes.len();
        // Directions (decision 0525 D1).
        let centres: Vec<Option<Point3>> = cut.passes.iter().map(|p| centre(p)).collect();
        let mut forward = true;
        for k in 0..cut.passes.len() {
            let here = centres.get(k).copied().flatten();
            let towards = centres
                .get(k + 1..)
                .and_then(|rest| rest.iter().find_map(|c| *c))
                .zip(here)
                .map(|(n, h)| n - h)
                .or_else(|| {
                    let before = centres
                        .get(..k)
                        .and_then(|b| b.iter().rev().find_map(|c| *c));
                    before.zip(here).map(|(b, h)| h - b)
                });
            let Some(runs) = cut.passes.get_mut(k) else {
                continue;
            };
            if runs.is_empty() {
                continue;
            }
            match settings.direction {
                Direction::Meander => {
                    if !forward {
                        reverse(runs);
                    }
                    forward = !forward;
                }
                Direction::Climb | Direction::Conventional => {
                    let want = settings.direction == Direction::Climb;
                    let longest = runs
                        .iter()
                        .max_by_key(|r| r.len())
                        .cloned()
                        .unwrap_or_default();
                    if let Some(next) = towards
                        && let Some(right) = material_on_right(&longest, next, &input.rotary)
                        && right != want
                    {
                        reverse(runs);
                    }
                }
            }
        }
        let edges = list(cut.guide_edges.iter().map(|e| e.0));
        let plural = if cut.guide_edges.len() == 1 { "" } else { "s" };
        let towards = if cut.point_opposite {
            "the opposite point"
        } else {
            "the opposite side"
        };
        let mut note = format!(
            "face {} is cut from its guide, edge{plural} {edges} ({:.1} mm long), towards {towards} \
             in {} up to {:.3} mm apart along the face",
            cut.id.0,
            cut.guide_length,
            crate::counted(geometric, "pass", "passes"),
            cut.spacing
        );
        if cut.dropped_short > 0 {
            note.push_str(&format!(
                "; {} nearest the point {} left out, being shorter than the tool's \
                 diameter",
                crate::counted(cut.dropped_short, "pass", "passes"),
                if cut.dropped_short == 1 { "is" } else { "are" }
            ));
        }
        face_notes.push(note);
        for runs in cut.passes {
            for points in runs {
                out_passes.push(Pass {
                    low_link: !out_passes.is_empty(),
                    points,
                    ..Pass::default()
                });
            }
        }
        stretches.append(&mut cut.stretches);
    }
    let unreached = areas(&stretches, context)?;
    let diameter = 2.0 * input.tool.cutting_radius();
    if out_passes.is_empty() {
        let why = if unreached.is_empty() {
            "nothing on them is left to cut after the avoid and check sets".to_owned()
        } else {
            unreached_list(&unreached, diameter)
        };
        return Err(PathError::refused(
            crate::topic::FACE_FLOW,
            format!("face-flow finishing cannot reach any of the selected faces: {why}"),
        ));
    }
    notes.push(
        Notice::information(format!(
            "cuts {} on {}",
            crate::counted(out_passes.len(), "pass", "passes"),
            crate::counted(face_notes.len(), "face", "faces")
        ))
        .about(topic::FACE_FLOW),
    );
    notes.extend(
        face_notes
            .into_iter()
            .map(|note| Notice::information(note).about(topic::FACE_FLOW)),
    );
    let mut warnings = Vec::new();
    if !unreached.is_empty() {
        let plural = if unreached.len() == 1 { "" } else { "s" };
        warnings.push(Notice {
            level: Level::Warning,
            topic: Some(topic::UNREACHED_AREAS.to_owned()),
            segment: None,
            text: format!(
                "face-flow finishing cannot reach {} area{plural} of the selected faces, which \
                 stay uncut: {}; check them in the simulation before running the program",
                unreached.len(),
                unreached_list(&unreached, diameter)
            ),
            setting: None,
            acknowledgement: None,
        });
    }
    Ok(StrategyOutput {
        passes: out_passes,
        notices: crate::strategy::notices(warnings, notes),
        unreached,
        shank_breaks: Vec::new(),
    })
}

/// The unreachable stretches joined into areas: stretches of neighbouring
/// passes of one face whose spans along the guide overlap, of one reason.
fn areas(stretches: &[Stretch], context: &JobContext<'_>) -> Result<Vec<Unreached>> {
    // Union-find over the stretches.
    let n = stretches.len();
    let mut parent: Vec<usize> = (0..n).collect();
    fn root(parent: &mut [usize], mut i: usize) -> usize {
        while let Some(&p) = parent.get(i) {
            if p == i {
                break;
            }
            let gp = parent.get(p).copied().unwrap_or(p);
            if let Some(slot) = parent.get_mut(i) {
                *slot = gp;
            }
            i = p;
        }
        i
    }
    // Only stretches of one face on neighbouring passes can join: compare
    // each pass's stretches with the next pass's, not every pair.
    let mut by_pass: BTreeMap<(FaceId, usize), Vec<usize>> = BTreeMap::new();
    for (i, s) in stretches.iter().enumerate() {
        by_pass.entry((s.face, s.pass)).or_default().push(i);
    }
    for ((face, pass), here) in &by_pass {
        context.check_cancelled()?;
        let Some(next) = by_pass.get(&(*face, pass + 1)) else {
            continue;
        };
        for &i in here {
            for &j in next {
                let (Some(a), Some(b)) = (stretches.get(i), stretches.get(j)) else {
                    continue;
                };
                let overlap = a.s.0 <= b.s.1 + 1e-9 && b.s.0 <= a.s.1 + 1e-9;
                if overlap && a.reason == b.reason {
                    let (ri, rj) = (root(&mut parent, i), root(&mut parent, j));
                    if ri != rj
                        && let Some(slot) = parent.get_mut(ri.max(rj))
                    {
                        *slot = ri.min(rj);
                    }
                }
            }
        }
    }
    let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for i in 0..n {
        let r = root(&mut parent, i);
        groups.entry(r).or_default().push(i);
    }
    let mut out: Vec<Unreached> = groups
        .values()
        .filter_map(|members| {
            let parts: Vec<&Stretch> = members.iter().filter_map(|i| stretches.get(*i)).collect();
            let first = parts.first()?;
            let points: Vec<Point3> = parts
                .iter()
                .flat_map(|s| s.points.iter().copied())
                .collect();
            let a: Vec<f64> = parts.iter().flat_map(|s| s.a.iter().copied()).collect();
            let x = points
                .iter()
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| {
                    (lo.min(p.x), hi.max(p.x))
                });
            let a0 = a.first().copied().unwrap_or(0.0);
            let a = a
                .iter()
                .map(|v| near(*v, a0))
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
                    (lo.min(v), hi.max(v))
                });
            let step = points.len().div_ceil(MAX_UNREACHED_POINTS).max(1);
            let points: Vec<Point3> = points
                .into_iter()
                .step_by(step)
                .take(MAX_UNREACHED_POINTS)
                .collect();
            Some(Unreached {
                x,
                a,
                area: parts.iter().map(|s| s.area).sum(),
                reason: first.reason,
                points,
            })
        })
        .collect();
    out.sort_by(|a, b| b.area.total_cmp(&a.area).then(a.x.0.total_cmp(&b.x.0)));
    Ok(out)
}

/// The areas for a message: the largest five, each with where, how large
/// and why, then how many more.
fn unreached_list(areas: &[Unreached], diameter: f64) -> String {
    const LISTED: usize = 5;
    let listed: Vec<String> = areas
        .iter()
        .take(LISTED)
        .map(|u| {
            let why = match u.reason {
                UnreachedReason::TooNarrow => {
                    format!("the face is too narrow for this {diameter:.3} mm tool there")
                }
                _ => "a neighbouring face is in the way of the tool".to_owned(),
            };
            format!(
                "X {:.3} to {:.3} near A {:.0} ({:.1} mm2), {why}",
                u.x.0,
                u.x.1,
                (0.5 * (u.a.0 + u.a.1)).rem_euclid(360.0),
                u.area
            )
        })
        .collect();
    let more = areas.len().saturating_sub(LISTED);
    let more = if more > 0 {
        format!("; and {more} smaller")
    } else {
        String::new()
    };
    format!("{}{more}", listed.join("; "))
}
