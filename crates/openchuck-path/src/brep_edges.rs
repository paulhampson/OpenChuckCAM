// SPDX-License-Identifier: AGPL-3.0-or-later
//! The edges of a B-rep model as a [`CurveSource`] for edge and curve
//! following (OP-08; work breakdown slice 16 item 1; decision record 0053).
//!
//! The B-rep knows which curves are edges: each [`BrepEdge`] carries its
//! polyline (the kernel's tessellation of the edge at the model's tolerance)
//! and the faces it joins. [`BrepEdges`] keeps the edges that are real edges
//! of the part, joins them end to end where exactly two meet, and gives the
//! chains in a deterministic order (NFR-06). Which edges are real is the
//! rule of decision 0053 item 2:
//!
//! - an edge between two faces is real when the faces' normals differ by at
//!   least the crease angle somewhere along it, so the tangent-continuous
//!   boundary of a fillet is not (by default);
//! - a seam, where one face meets itself (the line along a cylinder where
//!   its parameter wraps round), is never real;
//! - an edge of one face that is not a seam (the boundary of an open
//!   surface), an edge of three or more faces, and an edge whose normals
//!   cannot be evaluated are always real;
//! - an edge with no length (a degenerate edge, a pole) is never real.
//!
//! [`BrepEdges::concave`] keeps only the inside corners, for pencil
//! clean-up (OP-09; decision record 0226): the edges between two faces where
//! the part's outside turns inward, so a tool can rest touching both faces.
//! Which way an edge turns is read from the exact faces, not from triangles,
//! at the same places the turn is measured. At each place the direction
//! into each face, square to the edge in the face's tangent plane, is found
//! by stepping both ways in the face's (u, v) and keeping the side its trim
//! test puts on the face; the edge
//! is an inside corner there when the direction into each face points out of
//! the other face (along its outward normal). An edge is an inside corner
//! when it is one at every place where its faces turn by more than
//! [`SIDE_DEGREES`] and the side could be told; an edge that is an inside
//! corner along part of its length and an outside corner along the rest, or
//! whose side cannot be told anywhere, is not kept, and
//! [`BrepEdges::not_inside_throughout`] counts it.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use openchuck_types::brep::{BrepEdge, BrepModel, EdgeId};
use openchuck_types::geometry::{Point3, Vec3};

use crate::brep::ExactFace;
use crate::curves::{CurveSource, EdgeChain};
use crate::error::{PathError, Result};
use crate::parallel;
use crate::surface::{FaceClosest, ParametricFace, SurfaceSource, Uv};

/// How close, in millimetres, two edge ends must be to be the same corner.
const WELD: f64 = 1e-5;

/// The most places along an edge where its faces' normals are compared.
const MAX_SAMPLES: usize = 32;

/// How close, in millimetres, the two sides of a parameter range must be to
/// be the same place (a seam).
const SEAM: f64 = 1e-4;

/// How far, in millimetres, from an edge the faces are probed to find which
/// side of it each face is on: well beyond the model's tessellation tolerance
/// (0.01 mm), which the edge's polyline is accurate to, and small against a
/// face's width.
const PROBE: f64 = 0.05;

/// The smallest turn between the faces, degrees, at which [`BrepEdges::concave`]
/// asks which way an edge turns: below it the faces are tangent and the side
/// is not defined.
pub const SIDE_DEGREES: f64 = 0.5;

/// The default crease angle, degrees: edges whose faces are tangent to within
/// this are not edges (decision 0053 item 2).
pub const DEFAULT_CREASE_DEGREES: f64 = 1.0;

/// What an edge is, from the faces it joins.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Kind {
    /// Always an edge: a boundary, a non-manifold edge, or one whose normals
    /// could not be compared.
    Real,
    /// An edge between two faces, which turn by this many degrees at their
    /// sharpest along it.
    Between(f64),
}

/// Which way an edge between two faces turns, seen from outside the part.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Turn {
    /// An inside corner wherever the side could be told.
    Inside,
    /// An outside corner wherever the side could be told.
    Outside,
    /// An inside corner in places and an outside corner in others.
    Both,
    /// The side could not be told anywhere along it.
    Unknown,
}

#[derive(Debug, Clone)]
struct EdgeData {
    id: EdgeId,
    polyline: Vec<Point3>,
    kind: Kind,
    /// Which way the edge turns; `None` when it was not asked.
    turn: Option<Turn>,
}

/// The real edges of a B-rep model, as chains (OP-08).
#[derive(Debug, Clone)]
pub struct BrepEdges {
    edges: Vec<EdgeData>,
    crease_degrees: f64,
    min_length: f64,
    /// Whether only the inside corners are kept ([`Self::concave`]).
    inside_only: bool,
}

impl BrepEdges {
    /// The edges of every face of `model`, at the default crease angle
    /// ([`DEFAULT_CREASE_DEGREES`]) and with no length limit. Fails when a
    /// face is invalid.
    pub fn new(model: &BrepModel) -> Result<Self> {
        Self::build(model, false)
    }

    /// Only the inside corners of `model`: the edges between two faces that
    /// turn inward, at the default crease angle and with no length limit,
    /// for pencil clean-up (OP-09; decision record 0226). Boundaries, seams,
    /// edges of three or more faces, outside corners and edges that turn
    /// both ways are not kept. Fails when a face is invalid.
    pub fn concave(model: &BrepModel) -> Result<Self> {
        Self::build(model, true)
    }

    fn build(model: &BrepModel, inside_only: bool) -> Result<Self> {
        let faces = model
            .faces
            .iter()
            .map(ExactFace::new)
            .collect::<Result<Vec<_>>>()?;
        // Each edge is examined alone, in parallel, and kept in model order
        // (NFR-06).
        let edges =
            parallel::map_each_in_order(&model.edges, |edge| examine(edge, &faces, inside_only))
                .into_iter()
                .flatten()
                .collect();
        Ok(Self {
            edges,
            crease_degrees: DEFAULT_CREASE_DEGREES,
            min_length: 0.0,
            inside_only,
        })
    }

    /// Only the edges named in `ids` (the user's edge selection, the
    /// existing `Edges` target). Edges that are seams or degenerate are
    /// skipped as always. Fails when an ID is not in `model`, or when none
    /// of the chosen edges is left.
    pub fn with_edges(model: &BrepModel, ids: &[EdgeId]) -> Result<Self> {
        for id in ids {
            if !model.edges.iter().any(|e| e.id == *id) {
                return Err(PathError::Surface(format!(
                    "edge {} is not in the model",
                    id.0
                )));
            }
        }
        let mut all = Self::new(model)?;
        all.edges.retain(|e| ids.contains(&e.id));
        if all.edges.is_empty() {
            return Err(PathError::Surface(
                "none of the chosen edges can be followed: they are seams or have no length"
                    .to_owned(),
            ));
        }
        Ok(all)
    }

    /// The smallest turn between the two faces of an edge, degrees, that
    /// makes it an edge. Zero keeps every edge between two faces, tangent
    /// ones too.
    #[must_use]
    pub fn with_crease(mut self, degrees: f64) -> Self {
        self.crease_degrees = degrees;
        self
    }

    /// Chains shorter than this are dropped, millimetres.
    #[must_use]
    pub fn with_min_length(mut self, millimetres: f64) -> Self {
        self.min_length = millimetres;
        self
    }

    /// The source as a shared value, for [`crate::EdgeFollowing::with_source`].
    pub fn shared(self) -> Arc<dyn CurveSource> {
        Arc::new(self)
    }

    /// The IDs of the edges that are kept at the current crease angle, in ID
    /// order.
    pub fn edge_ids(&self) -> Vec<EdgeId> {
        self.kept().map(|e| e.id).collect()
    }

    /// The kept edges at the current crease angle, in ID order, each with
    /// the sharpest turn between its two faces in degrees, or `None` for an
    /// edge that is always real (a boundary, an edge of three or more faces,
    /// or one whose normals could not be compared). The viewport draws these
    /// and selects faces across them (slice 11 item 5; decision record 0066).
    pub fn edge_turns(&self) -> Vec<(EdgeId, Option<f64>)> {
        self.kept()
            .map(|e| {
                let turn = match e.kind {
                    Kind::Real => None,
                    Kind::Between(turn) => Some(turn),
                };
                (e.id, turn)
            })
            .collect()
    }

    /// How many edges between two faces are not kept by [`Self::concave`]
    /// though they may be inside corners: those that turn by at least the
    /// crease angle but are an inside corner along only part of their length
    /// or whose side could not be told, and those whose faces could not be
    /// compared at all (their turn is not known). Zero for [`Self::new`].
    pub fn not_inside_throughout(&self) -> usize {
        self.edges
            .iter()
            .filter(|e| match e.kind {
                Kind::Between(turn) => {
                    turn >= self.crease_degrees
                        && matches!(e.turn, Some(Turn::Both | Turn::Unknown))
                }
                Kind::Real => e.turn == Some(Turn::Unknown),
            })
            .count()
    }

    fn kept(&self) -> impl Iterator<Item = &EdgeData> {
        self.edges.iter().filter(|e| match e.kind {
            Kind::Real => !self.inside_only,
            Kind::Between(turn) => {
                turn >= self.crease_degrees && (!self.inside_only || e.turn == Some(Turn::Inside))
            }
        })
    }
}

impl CurveSource for BrepEdges {
    fn chains(&self, _surface: &dyn SurfaceSource) -> Result<Vec<EdgeChain>> {
        if !self.crease_degrees.is_finite() || !self.min_length.is_finite() {
            return Err(PathError::Surface(
                "the crease angle and the shortest chain must be finite numbers".to_owned(),
            ));
        }
        let kept: Vec<&EdgeData> = self.kept().collect();
        let mut chains = chain_edges(&kept);
        chains.retain(|c| c.length() >= self.min_length);
        Ok(chains)
    }

    /// The edges are on the part's faces to the model's tessellation
    /// tolerance, as the mesh's own edges are; they are not projected.
    fn lies_on_surface(&self) -> bool {
        true
    }
}

/// What `edge` is, from the faces it joins, or `None` when it is no edge
/// at all (degenerate, or a seam). With `sides`, which way an edge between
/// two faces turns as well.
fn examine(edge: &BrepEdge, faces: &[ExactFace], sides: bool) -> Option<EdgeData> {
    if edge.polyline.len() < 2 || polyline_length(&edge.polyline) <= WELD {
        return None;
    }
    let mut ids: Vec<u32> = edge.faces.iter().map(|f| f.0).collect();
    ids.sort_unstable();
    ids.dedup();
    let owners: Vec<&ExactFace> = ids
        .iter()
        .filter_map(|id| faces.iter().find(|f| f.number() == *id))
        .collect();
    let (kind, turn) = match owners.as_slice() {
        [only] if ids.len() == 1 => {
            if is_seam(only, &edge.polyline) {
                return None;
            }
            (Kind::Real, None)
        }
        [a, b] if ids.len() == 2 => {
            let (sharpest, turn) = between(a, b, &edge.polyline, sides);
            // With `sides`, the turn is kept for every edge of two faces,
            // one whose faces could not be compared too (then unknown), so
            // it is counted as left out.
            let kind = sharpest.map_or(Kind::Real, Kind::Between);
            (kind, sides.then_some(turn))
        }
        _ => (Kind::Real, None),
    };
    Some(EdgeData {
        id: edge.id,
        polyline: edge.polyline.clone(),
        kind,
        turn,
    })
}

fn polyline_length(points: &[Point3]) -> f64 {
    points
        .iter()
        .zip(points.iter().skip(1))
        .map(|(a, b)| a.distance(*b))
        .sum()
}

/// The places along the polyline where normals are compared: the midpoints of
/// up to [`MAX_SAMPLES`] of its segments, spread evenly.
fn samples(polyline: &[Point3]) -> Vec<Point3> {
    let segments = polyline.len().saturating_sub(1);
    let n = segments.min(MAX_SAMPLES);
    (0..n)
        .filter_map(|k| {
            let s = k * segments / n.max(1);
            let (a, b) = (polyline.get(s)?, polyline.get(s + 1)?);
            Some(Point3::new(
                (a.x + b.x) / 2.0,
                (a.y + b.y) / 2.0,
                (a.z + b.z) / 2.0,
            ))
        })
        .collect()
}

/// The places along the polyline where the side is asked, as [`samples`],
/// each with the direction of its segment.
fn samples_along(polyline: &[Point3]) -> Vec<(Point3, Vec3)> {
    let segments = polyline.len().saturating_sub(1);
    let n = segments.min(MAX_SAMPLES);
    (0..n)
        .filter_map(|k| {
            let s = k * segments / n.max(1);
            let (a, b) = (polyline.get(s)?, polyline.get(s + 1)?);
            Some((
                Point3::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0, (a.z + b.z) / 2.0),
                *b - *a,
            ))
        })
        .collect()
}

/// The direction from the closest point `at` of `face` into the face,
/// square to the edge's direction `along` in the face's tangent plane: of
/// the two ways, the one whose step of [`PROBE`] stays on the face by its
/// trim test. The step is taken in the face's (u, v), from its derivatives
/// there, so no closest point is searched for. `None` when both ways or
/// neither stay on it, or the parametrisation is singular there.
fn into_face(face: &ExactFace, at: &FaceClosest, along: Vec3) -> Option<Vec3> {
    let across = at.point.normal.get().cross(along);
    let size = across.length();
    if size <= 1e-12 {
        return None;
    }
    let d = across * (1.0 / size);
    let (lo, hi) = face.uv_bounds();
    let Uv { u, v } = at.uv;
    let centre = face.point_normal(at.uv)?.point;
    let point = |du: f64, dv: f64| {
        face.point_normal(Uv {
            u: u + du,
            v: v + dv,
        })
        .map(|p| p.point)
    };
    let slope = |plus: Option<Point3>, minus: Option<Point3>, step: f64| match (plus, minus) {
        (Some(p), Some(m)) => Some((p - m) * (0.5 / step)),
        (Some(p), None) => Some((p - centre) * (1.0 / step)),
        (None, Some(m)) => Some((centre - m) * (1.0 / step)),
        (None, None) => None,
    };
    let (hu, hv) = (
        1e-6 * (hi.u - lo.u).abs().max(1e-6),
        1e-6 * (hi.v - lo.v).abs().max(1e-6),
    );
    let su = slope(point(hu, 0.0), point(-hu, 0.0), hu)?;
    let sv = slope(point(0.0, hv), point(0.0, -hv), hv)?;
    // The step in (u, v) whose image is `d`, by least squares.
    let (g11, g12, g22) = (su.dot(su), su.dot(sv), sv.dot(sv));
    let det = g11 * g22 - g12 * g12;
    if det <= 1e-12 * g11 * g22 {
        return None;
    }
    let (du, dv) = (
        (g22 * su.dot(d) - g12 * sv.dot(d)) / det,
        (g11 * sv.dot(d) - g12 * su.dot(d)) / det,
    );
    let image = (su * du + sv * dv).length();
    if image <= 1e-12 {
        return None;
    }
    let k = PROBE / image;
    let on = |sign: f64| {
        face.contains(Uv {
            u: u + sign * k * du,
            v: v + sign * k * dv,
        })
    };
    match (on(1.0), on(-1.0)) {
        (true, false) => Some(d),
        (false, true) => Some(-d),
        _ => None,
    }
}

/// The sharpest turn, degrees, between the normals of `a` and `b` along the
/// polyline (`None` when no place gave both), and with `sides` which way the
/// edge turns (decision record 0226): at a place where the faces turn by
/// more than [`SIDE_DEGREES`], an inside corner when the way into each face
/// points out of the other, along its outward normal.
fn between(a: &ExactFace, b: &ExactFace, polyline: &[Point3], sides: bool) -> (Option<f64>, Turn) {
    let cos_side = SIDE_DEGREES.to_radians().cos();
    let mut sharpest: Option<f64> = None;
    let mut turns = Vec::new();
    for (p, along) in samples_along(polyline) {
        let (Some(ca), Some(cb)) = (a.closest(p), b.closest(p)) else {
            continue;
        };
        let (na, nb) = (ca.point.normal.get(), cb.point.normal.get());
        let cos = na.dot(nb).clamp(-1.0, 1.0);
        let turn = cos.acos().to_degrees();
        sharpest = Some(sharpest.map_or(turn, |s: f64| s.max(turn)));
        if !sides || cos > cos_side {
            continue;
        }
        let side = into_face(a, &ca, along)
            .zip(into_face(b, &cb, along))
            .and_then(|(into_a, into_b)| {
                let (sa, sb) = (nb.dot(into_a), na.dot(into_b));
                if sa > 0.0 && sb > 0.0 {
                    Some(Turn::Inside)
                } else if sa < 0.0 && sb < 0.0 {
                    Some(Turn::Outside)
                } else {
                    None
                }
            });
        turns.push(side);
    }
    (sharpest, combine(turns))
}

/// One edge's turn from the turns at its places: the one they all agree on,
/// [`Turn::Both`] when they differ, [`Turn::Unknown`] when none was told.
fn combine(sides: impl IntoIterator<Item = Option<Turn>>) -> Turn {
    sides
        .into_iter()
        .flatten()
        .fold(Turn::Unknown, |all, here| match all {
            Turn::Unknown => here,
            _ if all == here => all,
            _ => Turn::Both,
        })
}

/// Whether the edge is a seam of `face`: at every sample the point is on the
/// edge of the face's parameter box, and the opposite side of the box is the
/// same point.
fn is_seam(face: &ExactFace, polyline: &[Point3]) -> bool {
    let (lo, hi) = face.uv_bounds();
    let points = samples(polyline);
    !points.is_empty()
        && points.into_iter().all(|p| {
            let Some(c) = face.closest(p) else {
                return false;
            };
            let (u, v) = (c.uv.u, c.uv.v);
            let near = |x: f64, edge: f64, span: f64| (x - edge).abs() <= 1e-6 * (1.0 + span);
            let opposite = |x: f64, lo: f64, hi: f64| {
                if near(x, lo, hi - lo) {
                    Some(hi)
                } else if near(x, hi, hi - lo) {
                    Some(lo)
                } else {
                    None
                }
            };
            let across_u = opposite(u, lo.u, hi.u).map(|u2| Uv { u: u2, v });
            let across_v = opposite(v, lo.v, hi.v).map(|v2| Uv { u, v: v2 });
            [across_u, across_v].into_iter().flatten().any(|uv| {
                face.point_normal(uv)
                    .is_some_and(|q| q.point.distance(c.point.point) <= SEAM)
            })
        })
}

/// Sets the last point to the first, so a closed chain repeats it exactly.
fn close(points: &mut [Point3]) {
    if let Some(f) = points.first().copied()
        && let Some(l) = points.last_mut()
    {
        *l = f;
    }
}

fn weld(p: Point3) -> [i64; 3] {
    [p.x, p.y, p.z].map(|v| (v / WELD).round() as i64)
}

/// Joins edges end to end where exactly two of them meet. Open chains come
/// first, found by scanning the edges in ID order and starting at the end
/// that is not an ordinary join; then closed loops, by the lowest edge ID.
/// A closed edge is a closed chain of its own. A closed chain repeats its
/// first point last.
fn chain_edges(edges: &[&EdgeData]) -> Vec<EdgeChain> {
    let mut chains = Vec::new();
    // Ends of the open edges, by corner.
    let mut at: BTreeMap<[i64; 3], Vec<(usize, bool)>> = BTreeMap::new();
    let mut open: Vec<usize> = Vec::new();
    for (i, e) in edges.iter().enumerate() {
        let (Some(first), Some(last)) = (e.polyline.first(), e.polyline.last()) else {
            continue;
        };
        if weld(*first) == weld(*last) {
            let mut points = e.polyline.clone();
            close(&mut points);
            chains.push(EdgeChain {
                points,
                closed: true,
            });
            continue;
        }
        at.entry(weld(*first)).or_default().push((i, false));
        at.entry(weld(*last)).or_default().push((i, true));
        open.push(i);
    }
    let degree = |key: &[i64; 3]| at.get(key).map_or(0, Vec::len);

    let mut used: BTreeSet<usize> = BTreeSet::new();
    // Walks from the corner `start` along edge `first`, which is entered at
    // its start (`reversed` false) or its end.
    let walk = |first: usize, reversed: bool, used: &mut BTreeSet<usize>| -> EdgeChain {
        let mut points: Vec<Point3> = Vec::new();
        let (mut edge, mut backwards) = (first, reversed);
        let start_key;
        {
            let Some(e) = edges.get(first) else {
                return EdgeChain {
                    points,
                    closed: false,
                };
            };
            start_key = if reversed {
                e.polyline.last().map(|p| weld(*p))
            } else {
                e.polyline.first().map(|p| weld(*p))
            };
        }
        loop {
            used.insert(edge);
            let Some(e) = edges.get(edge) else { break };
            let line: Vec<Point3> = if backwards {
                e.polyline.iter().rev().copied().collect()
            } else {
                e.polyline.clone()
            };
            points.extend(line.iter().skip(usize::from(!points.is_empty())).copied());
            let Some(end) = line.last().map(|p| weld(*p)) else {
                break;
            };
            if Some(end) == start_key {
                close(&mut points);
                return EdgeChain {
                    points,
                    closed: true,
                };
            }
            if degree(&end) != 2 {
                break;
            }
            let next = at
                .get(&end)
                .and_then(|n| n.iter().find(|(i, _)| *i != edge && !used.contains(i)));
            match next {
                Some(&(i, at_end)) => {
                    edge = i;
                    backwards = at_end;
                }
                None => break,
            }
        }
        EdgeChain {
            points,
            closed: false,
        }
    };

    for &i in &open {
        if used.contains(&i) {
            continue;
        }
        let Some(e) = edges.get(i) else { continue };
        let (Some(first), Some(last)) = (e.polyline.first(), e.polyline.last()) else {
            continue;
        };
        // An end that is not an ordinary join starts an open chain.
        if degree(&weld(*first)) != 2 {
            chains.push(walk(i, false, &mut used));
        } else if degree(&weld(*last)) != 2 {
            chains.push(walk(i, true, &mut used));
        }
    }
    // What is left are loops of ordinary joins.
    for &i in &open {
        if !used.contains(&i) {
            chains.push(walk(i, false, &mut used));
        }
    }
    chains
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn req_op_09_an_edge_that_turns_both_ways_or_cannot_be_told_is_no_inside_corner() {
        use Turn::{Both, Inside, Outside, Unknown};
        assert_eq!(combine([Some(Inside), None, Some(Inside)]), Inside);
        assert_eq!(combine([Some(Outside), Some(Outside)]), Outside);
        assert_eq!(combine([Some(Inside), Some(Outside), Some(Inside)]), Both);
        assert_eq!(combine([None, None]), Unknown);
        assert_eq!(combine(std::iter::empty()), Unknown);
    }
}
