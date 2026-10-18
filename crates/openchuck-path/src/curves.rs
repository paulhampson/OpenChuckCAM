// SPDX-License-Identifier: AGPL-3.0-or-later
//! Curve sources for edge and curve following (OP-08; work breakdown slice
//! 16 item 1; decision record 0047 item 1).
//!
//! A [`CurveSource`] gives the strategy [`EdgeChain`]s: polylines in the model
//! frame, open or closed. The strategy knows nothing about where they come
//! from, so the mesh feature edges of this module, the B-rep edges of slice
//! 16's second lane and curves imported from a drawing (IMP-08) are three
//! sources of the same strategy.
//!
//! [`MeshFeatureEdges`] finds the edges of the part's tessellation where the
//! surface turns by at least a crease angle, convex or concave, together with
//! the edges that do not join exactly two triangles (the boundary of an open
//! surface), and chains them at vertices where exactly two of them meet.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use openchuck_types::geometry::{Point3, UnitVec3};

use crate::error::{PathError, Result};
use crate::surface::SurfaceSource;

/// How close, in millimetres, two vertices must be to be the same corner.
const WELD: f64 = 1e-5;

/// A chain of edges as a polyline in the model frame.
#[derive(Debug, Clone, PartialEq)]
pub struct EdgeChain {
    /// The points, in order. A closed chain repeats its first point last.
    pub points: Vec<Point3>,
    /// Whether the chain closes on itself.
    pub closed: bool,
}

impl EdgeChain {
    /// The length of the polyline in millimetres.
    pub fn length(&self) -> f64 {
        self.points
            .iter()
            .zip(self.points.iter().skip(1))
            .map(|(a, b)| a.distance(*b))
            .sum()
    }
}

/// Where the chains an edge-following operation cuts come from (decision
/// record 0047 item 1).
pub trait CurveSource: Send + Sync + std::fmt::Debug {
    /// The chains, in a deterministic order (NFR-06), for a part whose
    /// surface is `surface`.
    fn chains(&self, surface: &dyn SurfaceSource) -> Result<Vec<EdgeChain>>;

    /// Whether the chains lie on the part's surface already, as the edges of
    /// its own tessellation do. A source that does not (a curve drawn in a
    /// plane, IMP-08) is projected onto the surface along the tool axis by the
    /// strategy.
    fn lies_on_surface(&self) -> bool {
        true
    }
}

/// The feature edges of the part's tessellation, as chains (OP-08).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeshFeatureEdges {
    /// The smallest turn between two faces that makes an edge a feature
    /// edge, degrees.
    pub crease_degrees: f64,
    /// Chains shorter than this are dropped, millimetres.
    pub min_length: f64,
}

impl MeshFeatureEdges {
    /// Feature edges at `crease_degrees`, with no length limit.
    pub fn new(crease_degrees: f64) -> Self {
        Self {
            crease_degrees,
            min_length: 0.0,
        }
    }

    /// The source as a shared value, for [`crate::EdgeFollowing::with_source`].
    pub fn shared(self) -> Arc<dyn CurveSource> {
        Arc::new(self)
    }
}

impl CurveSource for MeshFeatureEdges {
    fn chains(&self, surface: &dyn SurfaceSource) -> Result<Vec<EdgeChain>> {
        if !self.crease_degrees.is_finite() || !self.min_length.is_finite() {
            return Err(PathError::Surface(
                "the crease angle and the shortest chain must be finite numbers".to_owned(),
            ));
        }
        let mut chains = feature_chains(surface.tessellation(), self.crease_degrees);
        chains.retain(|c| c.length() >= self.min_length);
        Ok(chains)
    }
}

/// Edges chosen one by one, as segments in the model frame (a user's choice
/// of a mesh's triangle edges, OI-204; decision record 0066), chained at
/// corners where exactly two of them meet. No crease angle applies: the user
/// chose them.
#[derive(Debug, Clone, PartialEq)]
pub struct ChosenSegments {
    segments: Vec<[Point3; 2]>,
    /// Chains shorter than this are dropped, millimetres.
    pub min_length: f64,
}

impl ChosenSegments {
    /// The segments, with no length limit. Ends closer than a
    /// hundred-thousandth of a millimetre are one corner, and a segment whose
    /// ends are one corner is skipped. Fails when a point is not finite or no
    /// segment has length.
    pub fn new(segments: Vec<[Point3; 2]>) -> Result<Self> {
        if segments.iter().flatten().any(|p| !p.is_finite()) {
            return Err(PathError::Surface(
                "every point of a chosen edge must be finite".to_owned(),
            ));
        }
        if segments.iter().all(|[a, b]| weld_key(*a) == weld_key(*b)) {
            return Err(PathError::Surface(
                "none of the chosen edges has length".to_owned(),
            ));
        }
        Ok(Self {
            segments,
            min_length: 0.0,
        })
    }

    /// Chains shorter than `millimetres` are dropped.
    #[must_use]
    pub fn with_min_length(mut self, millimetres: f64) -> Self {
        self.min_length = millimetres;
        self
    }

    /// The source as a shared value, for [`crate::EdgeFollowing::with_source`].
    pub fn shared(self) -> Arc<dyn CurveSource> {
        Arc::new(self)
    }
}

impl CurveSource for ChosenSegments {
    fn chains(&self, _surface: &dyn SurfaceSource) -> Result<Vec<EdgeChain>> {
        if !self.min_length.is_finite() {
            return Err(PathError::Surface(
                "the shortest chain must be a finite number".to_owned(),
            ));
        }
        let mut ids: BTreeMap<[i64; 3], u32> = BTreeMap::new();
        let mut welded: Vec<Point3> = Vec::new();
        let mut id = |p: Point3| {
            *ids.entry(weld_key(p)).or_insert_with(|| {
                welded.push(p);
                (welded.len() - 1) as u32
            })
        };
        let mut pairs: Vec<(u32, u32)> = Vec::new();
        let mut seen: BTreeSet<(u32, u32)> = BTreeSet::new();
        for [a, b] in &self.segments {
            let (i, j) = (id(*a), id(*b));
            let pair = (i.min(j), i.max(j));
            if i != j && seen.insert(pair) {
                pairs.push(pair);
            }
        }
        let mut chains: Vec<EdgeChain> = chain_edges(&pairs)
            .into_iter()
            .map(|(chain, closed)| EdgeChain {
                points: chain
                    .iter()
                    .filter_map(|v| welded.get(*v as usize).copied())
                    .collect(),
                closed,
            })
            .collect();
        chains.retain(|c| c.length() >= self.min_length);
        Ok(chains)
    }
}

fn weld_key(p: Point3) -> [i64; 3] {
    [p.x, p.y, p.z].map(|v| (v / WELD).round() as i64)
}

/// The feature edges of `mesh` that turn at least `min_angle` degrees (or
/// that do not join exactly two triangles), chained into polylines in a
/// deterministic order: by the welded vertex each chain starts at. Vertices
/// closer than a hundred-thousandth of a millimetre are one. Triangles with no
/// area are ignored.
pub fn feature_chains(
    mesh: &openchuck_types::mesh::TriangleMesh,
    min_angle: f64,
) -> Vec<EdgeChain> {
    let mut ids: BTreeMap<[i64; 3], u32> = BTreeMap::new();
    let mut welded: Vec<Point3> = Vec::new();
    let mut map: Vec<u32> = Vec::with_capacity(mesh.vertices.len());
    for &p in &mesh.vertices {
        let key = [p.x, p.y, p.z].map(|v| (v / WELD).round() as i64);
        let id = *ids.entry(key).or_insert_with(|| {
            welded.push(p);
            (welded.len() - 1) as u32
        });
        map.push(id);
    }

    let mut normals: Vec<Option<UnitVec3>> = Vec::with_capacity(mesh.triangles.len());
    let mut edges: BTreeMap<(u32, u32), Vec<usize>> = BTreeMap::new();
    for (t, tri) in mesh.triangles.iter().enumerate() {
        let corners = tri.map(|i| map.get(i as usize).copied());
        let [Some(i), Some(j), Some(k)] = corners else {
            normals.push(None);
            continue;
        };
        let points = [i, j, k].map(|v| welded.get(v as usize).copied());
        let [Some(a), Some(b), Some(c)] = points else {
            normals.push(None);
            continue;
        };
        let normal = (i != j && j != k && i != k)
            .then(|| UnitVec3::new((b - a).cross(c - a)).ok())
            .flatten();
        normals.push(normal);
        if normal.is_none() {
            continue;
        }
        for (p, q) in [(i, j), (j, k), (k, i)] {
            edges.entry((p.min(q), p.max(q))).or_default().push(t);
        }
    }

    let cos_min = min_angle.clamp(0.0, 180.0).to_radians().cos();
    let mut feature: Vec<(u32, u32)> = Vec::new();
    for (&(p, q), owners) in &edges {
        let is_feature = match owners.as_slice() {
            [ta, tb] => {
                let na = normals.get(*ta).copied().flatten();
                let nb = normals.get(*tb).copied().flatten();
                match (na, nb) {
                    (Some(na), Some(nb)) => na.get().dot(nb.get()) < cos_min,
                    _ => true,
                }
            }
            _ => true,
        };
        if is_feature {
            feature.push((p, q));
        }
    }

    chain_edges(&feature)
        .into_iter()
        .map(|(chain, closed)| EdgeChain {
            points: chain
                .iter()
                .filter_map(|v| welded.get(*v as usize).copied())
                .collect(),
            closed,
        })
        .collect()
}

/// Chains edges, given as vertex pairs, at vertices where exactly two of them
/// meet. Each chain is a list of vertices and whether it closes (its first
/// vertex is then repeated last). Open chains come first, in vertex order,
/// then closed loops, in the order of their lowest edge.
fn chain_edges(edges: &[(u32, u32)]) -> Vec<(Vec<u32>, bool)> {
    let mut next: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for &(p, q) in edges {
        next.entry(p).or_default().push(q);
        next.entry(q).or_default().push(p);
    }
    let mut used: BTreeSet<(u32, u32)> = BTreeSet::new();
    let key = |p: u32, q: u32| (p.min(q), p.max(q));
    let mut chains = Vec::new();
    let walk = |start: u32, first: u32, used: &mut BTreeSet<(u32, u32)>| -> (Vec<u32>, bool) {
        let mut chain = vec![start];
        let (mut previous, mut here) = (start, first);
        used.insert(key(start, first));
        loop {
            chain.push(here);
            if here == start {
                return (chain, true);
            }
            let onward = next.get(&here).and_then(|n| {
                (n.len() == 2)
                    .then(|| n.iter().copied().find(|v| *v != previous))
                    .flatten()
            });
            match onward {
                Some(v) if used.insert(key(here, v)) => {
                    previous = here;
                    here = v;
                }
                _ => return (chain, false),
            }
        }
    };
    // Open chains begin at a vertex that is not an ordinary part of a chain.
    for (&v, neighbours) in &next {
        if neighbours.len() == 2 {
            continue;
        }
        for &w in neighbours {
            if used.contains(&key(v, w)) {
                continue;
            }
            chains.push(walk(v, w, &mut used));
        }
    }
    // What is left are closed loops.
    for &(p, q) in edges {
        if used.contains(&key(p, q)) {
            continue;
        }
        chains.push(walk(p, q, &mut used));
    }
    chains
}
