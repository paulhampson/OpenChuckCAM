// SPDX-License-Identifier: AGPL-3.0-or-later
//! The edge map of a triangle mesh: which triangles use each edge, and in
//! which direction (architecture section 5.4).
//!
//! An edge is an unordered pair of vertex indices. Edges are numbered in
//! ascending order of `(low vertex, high vertex)`, so the numbering depends
//! only on the mesh and is the same on every run (NFR-06). A corner pair that
//! names the same vertex twice (a collapsed triangle) is not an edge.

use openchuck_types::mesh::TriangleMesh;
use openchuck_types::{Error, Result};

/// One triangle's use of an edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EdgeUse {
    /// The triangle's index in the mesh.
    pub triangle: u32,
    /// True when the triangle's winding runs from the edge's low vertex to its
    /// high vertex. Two triangles that agree on orientation use a shared edge
    /// in opposite directions.
    pub forward: bool,
}

/// Every edge of a mesh with the triangles that use it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EdgeMap {
    /// Edges as `[low, high]` vertex indices, ascending.
    edges: Vec<[u32; 2]>,
    /// Uses of edge `e` are `uses[starts[e]..starts[e + 1]]`, by triangle index.
    starts: Vec<usize>,
    uses: Vec<EdgeUse>,
    /// The edge of each triangle side (corner `k` to corner `k + 1`), or
    /// `None` for a side whose corners are the same vertex.
    sides: Vec<[Option<u32>; 3]>,
}

impl EdgeMap {
    /// Builds the edge map. Fails if a triangle refers to a missing vertex or
    /// the mesh has more than `u32::MAX` triangles.
    pub fn new(mesh: &TriangleMesh) -> Result<Self> {
        mesh.validate()?;
        let triangle_count = u32::try_from(mesh.triangles.len()).map_err(|_| {
            Error::invalid("triangles", "a mesh may hold at most 4294967295 triangles")
        })?;
        // (low, high, triangle, side, forward), sorted so equal edges are adjacent.
        let mut records: Vec<(u32, u32, u32, usize, bool)> =
            Vec::with_capacity(mesh.triangles.len() * 3);
        for (triangle, corners) in (0..triangle_count).zip(&mesh.triangles) {
            for side in 0..3 {
                let (Some(&a), Some(&b)) = (corners.get(side), corners.get((side + 1) % 3)) else {
                    continue;
                };
                if a != b {
                    records.push((a.min(b), a.max(b), triangle, side, a < b));
                }
            }
        }
        records.sort_unstable();

        let mut map = Self {
            edges: Vec::new(),
            starts: Vec::new(),
            uses: Vec::with_capacity(records.len()),
            sides: vec![[None; 3]; mesh.triangles.len()],
        };
        for (low, high, triangle, side, forward) in records {
            if map.edges.last() != Some(&[low, high]) {
                map.starts.push(map.uses.len());
                map.edges.push([low, high]);
            }
            let edge = u32::try_from(map.edges.len() - 1)
                .map_err(|_| Error::invalid("triangles", "the mesh has too many edges"))?;
            map.uses.push(EdgeUse { triangle, forward });
            if let Some(slot) = map
                .sides
                .get_mut(triangle as usize)
                .and_then(|s| s.get_mut(side))
            {
                *slot = Some(edge);
            }
        }
        map.starts.push(map.uses.len());
        Ok(map)
    }

    /// Number of edges.
    pub fn len(&self) -> usize {
        self.edges.len()
    }

    /// True when the mesh has no edges.
    pub fn is_empty(&self) -> bool {
        self.edges.is_empty()
    }

    /// Every edge as `[low, high]` vertex indices, in edge-number order.
    pub fn edges(&self) -> &[[u32; 2]] {
        &self.edges
    }

    /// The vertices of edge `edge`.
    pub fn edge(&self, edge: usize) -> Option<[u32; 2]> {
        self.edges.get(edge).copied()
    }

    /// The triangles that use edge `edge`, by triangle index.
    pub fn uses(&self, edge: usize) -> &[EdgeUse] {
        match (self.starts.get(edge), self.starts.get(edge + 1)) {
            (Some(&start), Some(&end)) => self.uses.get(start..end).unwrap_or_default(),
            _ => &[],
        }
    }

    /// The edges of triangle `triangle`'s three sides, corner `k` to corner
    /// `k + 1`; `None` for a side that joins a vertex to itself.
    pub fn sides(&self, triangle: usize) -> [Option<u32>; 3] {
        self.sides.get(triangle).copied().unwrap_or([None; 3])
    }
}
