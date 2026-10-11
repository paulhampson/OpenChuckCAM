// SPDX-License-Identifier: AGPL-3.0-or-later
//! Mesh validity checks and the report the user sees (IMP-04).
//!
//! The checks find, without changing the mesh:
//!
//! - **Holes:** edges used by one triangle only. Joined boundary edges form
//!   one hole each. Vertices are joined only when their coordinates are
//!   identical in the file, so a gap between near-coincident vertices shows
//!   as a hole.
//! - **Non-manifold edges:** edges used by more than two triangles.
//! - **Flipped normals:** triangles wound against their neighbours. Within
//!   each piece of the mesh joined by two-triangle edges, a closed piece is
//!   expected to wind anticlockwise seen from outside (positive enclosed
//!   volume), so a closed piece that is wholly inside out has every triangle
//!   flipped; an open piece takes its majority winding as correct. Separately,
//!   a stated facet normal (as STL files carry) that points against the
//!   triangle's winding is reported as a normal mismatch.
//! - **Degenerate triangles:** a triangle that uses one vertex twice, or whose
//!   area is zero to within double precision relative to its longest side
//!   (`|ab x ac| <= 1e-12 * longest side squared`).
//!
//! Every list is sorted, so the report is the same on every run (NFR-06).

use std::collections::BTreeMap;

use openchuck_types::Result;
use openchuck_types::geometry::{Point3, Vec3};
use openchuck_types::mesh::TriangleMesh;

use crate::edges::EdgeMap;

/// Relative area below which a triangle counts as degenerate.
pub const DEGENERATE_RELATIVE_AREA: f64 = 1e-12;

/// One hole: a connected run of boundary edges.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hole {
    /// The boundary edges as `[low, high]` vertex indices, ascending.
    pub edges: Vec<[u32; 2]>,
}

/// What the mesh checks found (IMP-04).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MeshReport {
    /// Number of triangles checked.
    pub triangles: usize,
    /// Number of vertices.
    pub vertices: usize,
    /// Number of distinct edges.
    pub edges: usize,
    /// Pieces of the mesh joined through two-triangle edges.
    pub shells: usize,
    /// Holes, ordered by their first edge.
    pub holes: Vec<Hole>,
    /// Edges used by more than two triangles, as `[low, high]`, ascending.
    pub non_manifold_edges: Vec<[u32; 2]>,
    /// Triangles wound against the rest of their piece, ascending.
    pub flipped_triangles: Vec<u32>,
    /// Closed pieces that are mostly inside out: more than half their
    /// triangles are in `flipped_triangles`.
    pub inside_out_shells: usize,
    /// Triangles whose stated normal points against their winding, ascending.
    pub normal_mismatches: Vec<u32>,
    /// Degenerate triangles, ascending.
    pub degenerate_triangles: Vec<u32>,
    /// The enclosed volume in cubic millimetres when the mesh is closed and
    /// has no non-manifold edges, counted with the corrected winding.
    pub volume: Option<f64>,
}

impl MeshReport {
    /// True when no check found a problem.
    pub fn is_clean(&self) -> bool {
        self.holes.is_empty()
            && self.non_manifold_edges.is_empty()
            && self.flipped_triangles.is_empty()
            && self.normal_mismatches.is_empty()
            && self.degenerate_triangles.is_empty()
    }

    /// Number of boundary edges over all holes.
    pub fn boundary_edges(&self) -> usize {
        self.holes.iter().map(|h| h.edges.len()).sum()
    }

    /// One line per problem found, for the user; empty when clean.
    pub fn problems(&self) -> Vec<String> {
        let mut lines = Vec::new();
        if !self.holes.is_empty() {
            lines.push(format!(
                "{} hole(s) bounded by {} open edge(s): the surface is not closed",
                self.holes.len(),
                self.boundary_edges()
            ));
        }
        if !self.non_manifold_edges.is_empty() {
            lines.push(format!(
                "{} non-manifold edge(s): shared by more than two triangles",
                self.non_manifold_edges.len()
            ));
        }
        if self.inside_out_shells > 0 {
            lines.push(format!(
                "{} closed piece(s) inside out: normals point inwards",
                self.inside_out_shells
            ));
        }
        if !self.flipped_triangles.is_empty() {
            lines.push(format!(
                "{} flipped triangle(s): wound against their neighbours, so their normals point the wrong way",
                self.flipped_triangles.len()
            ));
        }
        if !self.normal_mismatches.is_empty() {
            lines.push(format!(
                "{} triangle(s) whose stated normal points against their vertex order",
                self.normal_mismatches.len()
            ));
        }
        if !self.degenerate_triangles.is_empty() {
            lines.push(format!(
                "{} degenerate triangle(s): zero area or a repeated vertex",
                self.degenerate_triangles.len()
            ));
        }
        lines
    }
}

/// Checks `mesh` (IMP-04). `stated_normals`, when given, holds one normal per
/// triangle as the file states it; a zero or non-finite stated normal is
/// ignored. Fails only if the mesh refers to a missing vertex.
pub fn check_mesh(mesh: &TriangleMesh, stated_normals: Option<&[Vec3]>) -> Result<MeshReport> {
    let map = EdgeMap::new(mesh)?;
    let corners = |t: usize| mesh.triangle(t);

    let mut report = MeshReport {
        triangles: mesh.triangles.len(),
        vertices: mesh.vertices.len(),
        edges: map.len(),
        ..MeshReport::default()
    };

    // Degenerate triangles and normal mismatches, triangle by triangle.
    for (t, indices) in mesh.triangles.iter().enumerate() {
        let index = u32::try_from(t).unwrap_or(u32::MAX);
        let Some([a, b, c]) = corners(t) else {
            continue;
        };
        let [i, j, k] = *indices;
        let n = (b - a).cross(c - a);
        let longest = (b - a).length().max((c - b).length()).max((a - c).length());
        if i == j || j == k || k == i || n.length() <= DEGENERATE_RELATIVE_AREA * longest * longest
        {
            report.degenerate_triangles.push(index);
            continue;
        }
        if let Some(stated) = stated_normals.and_then(|normals| normals.get(t))
            && stated.is_finite()
            && stated.length() > 0.0
            && n.dot(*stated) < 0.0
        {
            report.normal_mismatches.push(index);
        }
    }

    // Boundary and non-manifold edges.
    let mut boundary = Vec::new();
    for (e, &edge) in map.edges().iter().enumerate() {
        match map.uses(e).len() {
            1 => boundary.push(edge),
            2 => {}
            _ => report.non_manifold_edges.push(edge),
        }
    }
    report.holes = group_holes(&boundary, mesh.vertices.len());

    orient(mesh, &map, &mut report);
    Ok(report)
}

/// Groups boundary edges that share vertices into holes.
fn group_holes(boundary: &[[u32; 2]], vertex_count: usize) -> Vec<Hole> {
    let mut sets = DisjointSets::new(vertex_count);
    for &[a, b] in boundary {
        sets.union(a as usize, b as usize);
    }
    // Holes keyed by their root, in order of their first (lowest) edge.
    let mut by_root: BTreeMap<usize, usize> = BTreeMap::new();
    let mut holes: Vec<Hole> = Vec::new();
    for &edge in boundary {
        let root = sets.find(edge[0] as usize);
        let next = holes.len();
        let index = *by_root.entry(root).or_insert(next);
        match holes.get_mut(index) {
            Some(hole) => hole.edges.push(edge),
            None => holes.push(Hole { edges: vec![edge] }),
        }
    }
    holes
}

/// Finds flipped triangles and inside-out pieces, and the enclosed volume.
fn orient(mesh: &TriangleMesh, map: &EdgeMap, report: &mut MeshReport) {
    let count = mesh.triangles.len();
    // Winding relative to the piece's first triangle: true when opposite.
    let mut parity = vec![false; count];
    let mut seen = vec![false; count];
    let mut all_closed = report.non_manifold_edges.is_empty();
    let mut total_volume = 0.0;
    let mut flipped = Vec::new();
    let mut stack = Vec::new();

    for seed in 0..count {
        if seen.get(seed) != Some(&false) {
            continue;
        }
        report.shells += 1;
        let mut members = vec![seed];
        let mut closed = true;
        if let Some(s) = seen.get_mut(seed) {
            *s = true;
        }
        stack.push(seed);
        while let Some(t) = stack.pop() {
            let here = parity.get(t).copied().unwrap_or(false);
            for edge in map.sides(t).into_iter().flatten() {
                let uses = map.uses(edge as usize);
                if uses.len() != 2 {
                    closed = false;
                    continue;
                }
                let Some(own) = uses.iter().find(|u| u.triangle as usize == t) else {
                    continue;
                };
                let Some(other) = uses.iter().find(|u| u.triangle as usize != t) else {
                    // The triangle uses this edge twice; it cannot be oriented by it.
                    closed = false;
                    continue;
                };
                let n = other.triangle as usize;
                if seen.get(n) == Some(&false) {
                    if let Some(s) = seen.get_mut(n) {
                        *s = true;
                    }
                    if let Some(p) = parity.get_mut(n) {
                        // Agreeing triangles use a shared edge in opposite directions.
                        *p = here ^ (own.forward == other.forward);
                    }
                    members.push(n);
                    stack.push(n);
                }
            }
        }
        members.sort_unstable();

        // Enclosed volume with every triangle turned to the seed's winding.
        let reference = mesh.triangle(seed).map_or(Point3::ORIGIN, |[a, _, _]| a);
        let volume: f64 = members
            .iter()
            .filter_map(|&t| {
                let [a, b, c] = mesh.triangle(t)?;
                let v = (a - reference).dot((b - reference).cross(c - reference)) / 6.0;
                Some(if parity.get(t) == Some(&true) { -v } else { v })
            })
            .sum();
        let opposite = members
            .iter()
            .filter(|&&t| parity.get(t) == Some(&true))
            .count();
        // The parity that is wrong: for a closed piece, the one whose winding
        // makes the enclosed volume negative; otherwise the minority (on a
        // tie the seed's winding is kept).
        let wrong = if closed && volume != 0.0 {
            volume > 0.0
        } else {
            opposite * 2 <= members.len()
        };
        let wrong_count = if wrong {
            opposite
        } else {
            members.len() - opposite
        };
        if closed && volume != 0.0 {
            total_volume += volume.abs();
            if wrong_count * 2 > members.len() {
                report.inside_out_shells += 1;
            }
        } else {
            all_closed = false;
        }
        flipped.extend(
            members
                .iter()
                .filter(|&&t| parity.get(t).copied().unwrap_or(false) == wrong)
                .filter_map(|&t| u32::try_from(t).ok()),
        );
    }
    flipped.sort_unstable();
    report.flipped_triangles = flipped;
    report.volume = (all_closed && count > 0).then_some(total_volume);
}

/// Union-find over vertex indices, for grouping boundary edges.
struct DisjointSets {
    parent: Vec<usize>,
}

impl DisjointSets {
    fn new(len: usize) -> Self {
        Self {
            parent: (0..len).collect(),
        }
    }

    fn find(&mut self, mut x: usize) -> usize {
        let mut root = x;
        while let Some(&p) = self.parent.get(root) {
            if p == root {
                break;
            }
            root = p;
        }
        // Path compression.
        while let Some(&p) = self.parent.get(x) {
            if p == root {
                break;
            }
            if let Some(slot) = self.parent.get_mut(x) {
                *slot = root;
            }
            x = p;
        }
        root
    }

    fn union(&mut self, a: usize, b: usize) {
        let (ra, rb) = (self.find(a), self.find(b));
        // The lower index becomes the root, so grouping is deterministic.
        let (low, high) = (ra.min(rb), ra.max(rb));
        if let Some(slot) = self.parent.get_mut(high) {
            *slot = low;
        }
    }
}
