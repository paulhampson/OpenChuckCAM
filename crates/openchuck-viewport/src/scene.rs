// SPDX-License-Identifier: AGPL-3.0-or-later
//! What the renderer draws, as plain data ready for the GPU: meshes with
//! their feature edges, and lines. The front end builds it from the
//! application's scene view model; this crate never reads a project.

use openchuck_types::geometry::Point3;
use openchuck_types::mesh::TriangleMesh;

/// An RGBA colour, each channel from 0 to 1.
pub type Colour = [f32; 4];

/// How meshes are drawn (UI-02).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum DisplayMode {
    /// Shaded surfaces only.
    Shaded,
    /// Shaded surfaces with feature edges over them.
    #[default]
    ShadedWithEdges,
    /// Every triangle edge, no surfaces.
    Wireframe,
}

impl DisplayMode {
    /// Every mode, in menu order.
    pub const ALL: [Self; 3] = [Self::Shaded, Self::ShadedWithEdges, Self::Wireframe];

    /// The mode's name for menus.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Shaded => "Shaded",
            Self::ShadedWithEdges => "Shaded with edges",
            Self::Wireframe => "Wireframe",
        }
    }
}

/// A triangle mesh to draw.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MeshItem {
    /// Vertex positions in the model frame.
    pub positions: Vec<[f32; 3]>,
    /// Three vertex indices per triangle.
    pub indices: Vec<u32>,
    /// Two vertex indices per feature edge.
    pub feature_edges: Vec<u32>,
    /// Surface colour.
    pub colour: Colour,
    /// Colour of the edges.
    pub edge_colour: Colour,
}

/// Lines to draw.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LineItem {
    /// Two points per line segment, in the model frame.
    pub segments: Vec<[f32; 3]>,
    /// Colour.
    pub colour: Colour,
    /// Drawn over everything, for guides such as the rotary axis and the work
    /// coordinate system that would otherwise hide inside the part.
    pub on_top: bool,
}

/// Everything the renderer draws.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Scene {
    /// Meshes.
    pub meshes: Vec<MeshItem>,
    /// Lines.
    pub lines: Vec<LineItem>,
}

impl Scene {
    /// Adds a mesh with its feature edges (vertex index pairs). Triangles or
    /// edges that name a missing vertex are left out.
    pub fn add_mesh(
        &mut self,
        mesh: &TriangleMesh,
        feature_edges: &[[u32; 2]],
        colour: Colour,
        edge_colour: Colour,
    ) {
        let count = mesh.vertices.len();
        let valid = |i: u32| (i as usize) < count;
        self.meshes.push(MeshItem {
            positions: mesh.vertices.iter().map(|p| to_f32(*p)).collect(),
            indices: mesh
                .triangles
                .iter()
                .filter(|t| t.iter().all(|&i| valid(i)))
                .flatten()
                .copied()
                .collect(),
            feature_edges: feature_edges
                .iter()
                .filter(|e| e.iter().all(|&i| valid(i)))
                .flatten()
                .copied()
                .collect(),
            colour,
            edge_colour,
        });
    }

    /// Adds a line through `points` in order.
    pub fn add_polyline(&mut self, points: &[Point3], colour: Colour, on_top: bool) {
        let segments = points
            .windows(2)
            .flat_map(|w| w.iter().map(|p| to_f32(*p)))
            .collect();
        self.lines.push(LineItem {
            segments,
            colour,
            on_top,
        });
    }

    /// The box holding every point of the scene, or `None` when it is empty.
    pub fn bounds(&self) -> Option<([f64; 3], [f64; 3])> {
        let points = self
            .meshes
            .iter()
            .flat_map(|m| &m.positions)
            .chain(self.lines.iter().flat_map(|l| &l.segments));
        let mut bounds: Option<([f64; 3], [f64; 3])> = None;
        for p in points {
            let p = p.map(f64::from);
            bounds = Some(match bounds {
                None => (p, p),
                Some((lo, hi)) => (
                    [lo[0].min(p[0]), lo[1].min(p[1]), lo[2].min(p[2])],
                    [hi[0].max(p[0]), hi[1].max(p[1]), hi[2].max(p[2])],
                ),
            });
        }
        bounds
    }

    /// Number of triangles in the scene.
    pub fn triangle_count(&self) -> usize {
        self.meshes.iter().map(|m| m.indices.len() / 3).sum()
    }
}

/// Every triangle side as a line, two indices per side, for the wireframe
/// mode. A side shared by two triangles appears twice; drawing it twice
/// costs less than finding the pairs on a large mesh.
pub(crate) fn wireframe_indices(indices: &[u32]) -> Vec<u32> {
    indices
        .chunks_exact(3)
        .flat_map(|t| match *t {
            [a, b, c] => [a, b, b, c, c, a],
            _ => [0; 6],
        })
        .collect()
}

#[allow(clippy::cast_possible_truncation)]
fn to_f32(p: Point3) -> [f32; 3] {
    [p.x as f32, p.y as f32, p.z as f32]
}
