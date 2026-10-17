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

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use openchuck_types::brep::{BrepModel, EdgeId};
use openchuck_types::geometry::Point3;

use crate::brep::ExactFace;
use crate::curves::{CurveSource, EdgeChain};
use crate::error::{PathError, Result};
use crate::surface::{ParametricFace, SurfaceSource, Uv};

/// How close, in millimetres, two edge ends must be to be the same corner.
const WELD: f64 = 1e-5;

/// The most places along an edge where its faces' normals are compared.
const MAX_SAMPLES: usize = 32;

/// How close, in millimetres, the two sides of a parameter range must be to
/// be the same place (a seam).
const SEAM: f64 = 1e-4;

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

#[derive(Debug, Clone)]
struct EdgeData {
    id: EdgeId,
    polyline: Vec<Point3>,
    kind: Kind,
}

/// The real edges of a B-rep model, as chains (OP-08).
#[derive(Debug, Clone)]
pub struct BrepEdges {
    edges: Vec<EdgeData>,
    crease_degrees: f64,
    min_length: f64,
}

impl BrepEdges {
    /// The edges of every face of `model`, at the default crease angle
    /// ([`DEFAULT_CREASE_DEGREES`]) and with no length limit. Fails when a
    /// face is invalid.
    pub fn new(model: &BrepModel) -> Result<Self> {
        let faces = model
            .faces
            .iter()
            .map(ExactFace::new)
            .collect::<Result<Vec<_>>>()?;
        let mut edges = Vec::new();
        for edge in &model.edges {
            if edge.polyline.len() < 2 || polyline_length(&edge.polyline) <= WELD {
                continue;
            }
            let mut ids: Vec<u32> = edge.faces.iter().map(|f| f.0).collect();
            ids.sort_unstable();
            ids.dedup();
            let owners: Vec<&ExactFace> = ids
                .iter()
                .filter_map(|id| faces.iter().find(|f| f.number() == *id))
                .collect();
            let kind = match owners.as_slice() {
                [only] if ids.len() == 1 => {
                    if is_seam(only, &edge.polyline) {
                        continue;
                    }
                    Kind::Real
                }
                [a, b] if ids.len() == 2 => {
                    sharpest_turn(a, b, &edge.polyline).map_or(Kind::Real, Kind::Between)
                }
                _ => Kind::Real,
            };
            edges.push(EdgeData {
                id: edge.id,
                polyline: edge.polyline.clone(),
                kind,
            });
        }
        Ok(Self {
            edges,
            crease_degrees: DEFAULT_CREASE_DEGREES,
            min_length: 0.0,
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

    fn kept(&self) -> impl Iterator<Item = &EdgeData> {
        self.edges.iter().filter(|e| match e.kind {
            Kind::Real => true,
            Kind::Between(turn) => turn >= self.crease_degrees,
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

/// The sharpest turn, degrees, between the normals of `a` and `b` along the
/// polyline; `None` when no place gave both.
fn sharpest_turn(a: &ExactFace, b: &ExactFace, polyline: &[Point3]) -> Option<f64> {
    samples(polyline)
        .into_iter()
        .filter_map(|p| {
            let na = a.closest(p)?.point.normal.get();
            let nb = b.closest(p)?.point.normal.get();
            Some(na.dot(nb).clamp(-1.0, 1.0).acos().to_degrees())
        })
        .reduce(f64::max)
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
