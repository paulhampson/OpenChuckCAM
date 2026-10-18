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

/// Which moving frame an item is drawn in. The front end sets one transform
/// per slot for each frame (`Frame::transforms`), so an item that moves does
/// not have to be uploaded again: the tool follows the playback position, and
/// in the machine view the workpiece turns with A.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Slot {
    /// Fixed in the view.
    #[default]
    Fixed,
    /// Fixed to the workpiece, which turns with A in the machine view.
    Part,
    /// Fixed to the tool, which follows the playback position.
    Tool,
}

impl Slot {
    /// Number of slots.
    pub const COUNT: usize = 3;

    /// The slot's index into `Frame::transforms`.
    pub const fn index(self) -> usize {
        match self {
            Self::Fixed => 0,
            Self::Part => 1,
            Self::Tool => 2,
        }
    }
}

/// How a line is drawn along its length (interface specification 7.4): the
/// dashes are measured in logical pixels on screen at the camera's target,
/// so they keep their size as the view zooms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LineStyle {
    /// Unbroken: cut, link and entry moves, edges, outlines.
    #[default]
    Solid,
    /// Dashes 5 pixels long with 4-pixel gaps: the stock outline, the
    /// clearance cylinder and safe Z.
    Dashed,
    /// A 14-pixel dash, a gap, a 2-pixel dot and a gap of 4 pixels each: the
    /// rotary axis.
    DashDot,
    /// Dots 2 pixels long, 4 pixels apart: rapid moves.
    Dotted,
}

impl LineStyle {
    /// The pattern in logical pixels: on, off, on, off. A solid line is all
    /// zero.
    pub const fn pattern(self) -> [f32; 4] {
        match self {
            Self::Solid => [0.0; 4],
            Self::Dashed => [5.0, 4.0, 0.0, 0.0],
            Self::DashDot => [14.0, 4.0, 2.0, 4.0],
            Self::Dotted => [2.0, 4.0, 0.0, 0.0],
        }
    }
}

/// A hatch drawn over a mesh's fill in screen space: lines 1 pixel wide,
/// 6 logical pixels apart whatever the zoom (interface specification 7.4 and
/// 7.5).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hatch {
    /// The colour of the hatch lines.
    pub colour: Colour,
    /// Lines in both diagonal directions (a held region) rather than one
    /// (avoid and check surfaces, the travel limit, a value not measured).
    pub cross: bool,
}

/// The outline drawn round a mesh: its silhouette as seen from the eye, its
/// creases (edges where the faces meet at more than 30 degrees) and its
/// open boundary, as a wide line of the given width (interface
/// specification 7.5: role outlines 1.5 pixels, hovered 2, selected 3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Outline {
    /// Colour.
    pub colour: Colour,
    /// Width in logical pixels.
    pub width: f32,
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
    /// The moving frame it is drawn in.
    pub slot: Slot,
    /// Drawn after the opaque items, blended with what is behind it and
    /// without hiding it, such as the remaining stock around the part.
    pub translucent: bool,
    /// A screen-space hatch over the fill.
    pub hatch: Option<Hatch>,
    /// An outline round the silhouette, creases and boundary.
    pub outline: Option<Outline>,
    /// Only the outline is drawn, with no fill and no feature edges: held
    /// regions and tabs while picking and on Simulate, and highlights.
    pub outline_only: bool,
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
    /// The moving frame it is drawn in.
    pub slot: Slot,
    /// The line's width in logical pixels at 100% scale. Up to 1 is a
    /// hairline of one device pixel; a wider line is drawn as a quad whose
    /// width does not change with the zoom, so a state such as a hovered or
    /// selected edge can differ by width as well as colour (specification
    /// section 16; OI-245).
    pub width: f32,
    /// Solid, dashed, dash-dot or dotted.
    pub style: LineStyle,
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
            slot: Slot::Fixed,
            translucent: false,
            hatch: None,
            outline: None,
            outline_only: false,
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
            slot: Slot::Fixed,
            width: 0.0,
            style: LineStyle::Solid,
        });
    }

    /// Adds a line through `points` in order, `width` logical pixels wide
    /// (see [`LineItem::width`]).
    pub fn add_wide_polyline(
        &mut self,
        points: &[Point3],
        colour: Colour,
        on_top: bool,
        width: f32,
    ) {
        self.add_polyline(points, colour, on_top);
        if let Some(line) = self.lines.last_mut() {
            line.width = width;
        }
    }

    /// Adds a line through `points` in `style`, `width` logical pixels wide
    /// (up to 1 is a hairline).
    pub fn add_styled_polyline(
        &mut self,
        points: &[Point3],
        colour: Colour,
        on_top: bool,
        width: f32,
        style: LineStyle,
    ) {
        self.add_wide_polyline(points, colour, on_top, width);
        if let Some(line) = self.lines.last_mut() {
            line.style = style;
        }
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

/// The points of `segments` (two per segment) each with its distance along
/// the line in millimetres, counted from the first point, for the dash
/// pattern. Segments that follow on from one another continue the count, so
/// a dashed polyline keeps its rhythm round a curve.
pub(crate) fn line_vertices(segments: &[[f32; 3]]) -> Vec<[f32; 4]> {
    let mut out = Vec::with_capacity(segments.len());
    let mut along = 0.0_f32;
    for pair in segments.chunks_exact(2) {
        if let [a, b] = pair {
            let length = distance(*a, *b);
            out.push([a[0], a[1], a[2], along]);
            out.push([b[0], b[1], b[2], along + length]);
            along += length;
        }
    }
    out
}

fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// An edge of a mesh's outline: its two ends and the normals of the two
/// faces that meet there. The vertex shader draws it only when one face
/// looks toward the eye and the other away (the silhouette); an edge with
/// both normals zero (a crease, or the open boundary) is always drawn.
pub(crate) type OutlineEdge = [[f32; 3]; 4];

/// The cosine of the angle between two faces above which their shared edge
/// is not a crease: 30 degrees, the feature-edge angle of decision 0029
/// item 4.
const CREASE_COS: f32 = 0.866_025_4;

/// The outline edges of a triangle mesh. Vertices at the same place are
/// joined first (a solid's seam and caps repeat them), so the faces either
/// side of a seam are found. Edges shared by two faces at less than 30
/// degrees are silhouette candidates; creases, boundary edges and edges of
/// more than two faces are always drawn; the rest are left out. The order
/// is fixed by the vertex order, so the upload is the same every time.
pub(crate) fn outline_edges(positions: &[[f32; 3]], indices: &[u32]) -> Vec<OutlineEdge> {
    use std::collections::BTreeMap;
    // A tenth of a micrometre joins coincident points without joining
    // distinct ones on any part this program machines.
    #[allow(clippy::cast_possible_truncation)]
    let key = |p: [f32; 3]| p.map(|v| (f64::from(v) * 1.0e4).round() as i64);
    let mut first: BTreeMap<[i64; 3], u32> = BTreeMap::new();
    let welded: Vec<u32> = (0u32..)
        .zip(positions)
        .map(|(i, p)| *first.entry(key(*p)).or_insert(i))
        .collect();
    let mut faces: BTreeMap<(u32, u32), Vec<[f32; 3]>> = BTreeMap::new();
    for t in indices.chunks_exact(3) {
        let Some(ids) = t
            .iter()
            .map(|&i| welded.get(i as usize).copied())
            .collect::<Option<Vec<u32>>>()
        else {
            continue;
        };
        let (Some(&a), Some(&b), Some(&c)) = (ids.first(), ids.get(1), ids.get(2)) else {
            continue;
        };
        let point = |i: u32| positions.get(i as usize).copied().unwrap_or_default();
        let (pa, pb, pc) = (point(a), point(b), point(c));
        let u = [pb[0] - pa[0], pb[1] - pa[1], pb[2] - pa[2]];
        let v = [pc[0] - pa[0], pc[1] - pa[1], pc[2] - pa[2]];
        let n = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        let length = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        if length <= f32::EPSILON || a == b || b == c || c == a {
            continue;
        }
        let n = n.map(|x| x / length);
        for (p, q) in [(a, b), (b, c), (c, a)] {
            faces.entry((p.min(q), p.max(q))).or_default().push(n);
        }
    }
    let point = |i: u32| positions.get(i as usize).copied().unwrap_or_default();
    faces
        .into_iter()
        .map(|((a, b), normals)| {
            let always = [[0.0; 3]; 2];
            let pair = match normals.as_slice() {
                [n1, n2] => {
                    let cos = n1[0] * n2[0] + n1[1] * n2[1] + n1[2] * n2[2];
                    if cos < CREASE_COS { always } else { [*n1, *n2] }
                }
                _ => always,
            };
            [point(a), point(b), pair[0], pair[1]]
        })
        .collect()
}

#[allow(clippy::cast_possible_truncation)]
fn to_f32(p: Point3) -> [f32; 3] {
    [p.x as f32, p.y as f32, p.z as f32]
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing, clippy::float_cmp)]

    use super::*;

    #[test]
    fn req_ui_03_a_line_counts_its_length_on_across_its_segments() {
        // Three segments of a polyline, 3, 4 and 5 long: the count carries on.
        let segments = [
            [0.0, 0.0, 0.0],
            [3.0, 0.0, 0.0],
            [3.0, 0.0, 0.0],
            [3.0, 4.0, 0.0],
            [3.0, 4.0, 0.0],
            [0.0, 0.0, 0.0],
        ];
        let along: Vec<f32> = line_vertices(&segments).iter().map(|v| v[3]).collect();
        assert_eq!(along, [0.0, 3.0, 3.0, 7.0, 7.0, 12.0]);
        // An odd last point has no partner and is left out.
        assert_eq!(line_vertices(&segments[..3]).len(), 2);
    }

    #[test]
    fn req_ui_03_line_styles_have_the_specified_patterns() {
        assert_eq!(LineStyle::default(), LineStyle::Solid);
        assert_eq!(LineStyle::Solid.pattern(), [0.0; 4]);
        // Stock dashed, rotary axis dash-dot, rapids dotted (7.4).
        assert_eq!(LineStyle::Dashed.pattern(), [5.0, 4.0, 0.0, 0.0]);
        assert_eq!(LineStyle::DashDot.pattern(), [14.0, 4.0, 2.0, 4.0]);
        assert_eq!(LineStyle::Dotted.pattern(), [2.0, 4.0, 0.0, 0.0]);
    }

    /// A unit cube with its eight corners repeated per face, as a solid's
    /// caps repeat the rim: 24 vertices, 12 triangles.
    fn split_cube() -> (Vec<[f32; 3]>, Vec<u32>) {
        let corner = |i: u32| [(i & 1) as f32, ((i >> 1) & 1) as f32, ((i >> 2) & 1) as f32];
        let faces: [[u32; 4]; 6] = [
            [0, 2, 3, 1],
            [4, 5, 7, 6],
            [0, 1, 5, 4],
            [2, 6, 7, 3],
            [1, 3, 7, 5],
            [0, 4, 6, 2],
        ];
        let mut positions = Vec::new();
        let mut indices = Vec::new();
        for face in faces {
            let base = positions.len() as u32;
            positions.extend(face.iter().map(|&i| corner(i)));
            indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        (positions, indices)
    }

    #[test]
    fn req_ui_25_outline_edges_join_repeated_vertices_and_keep_the_creases() {
        let (positions, indices) = split_cube();
        let edges = outline_edges(&positions, &indices);
        // Twelve cube edges, joined across the repeated vertices, plus one
        // diagonal on each face.
        assert_eq!(edges.len(), 18);
        let creases = edges
            .iter()
            .filter(|e| e[2] == [0.0; 3] && e[3] == [0.0; 3])
            .count();
        assert_eq!(creases, 12);
        // The diagonals lie in a flat face: silhouette candidates with both
        // normals the same.
        assert!(
            edges
                .iter()
                .filter(|e| e[2] != [0.0; 3])
                .all(|e| e[2] == e[3])
        );
        // The same mesh gives the same edges in the same order.
        assert_eq!(edges, outline_edges(&positions, &indices));
    }

    #[test]
    fn req_ui_25_req_nfr_07_outline_edges_skip_bad_triangles_and_keep_the_boundary() {
        // One triangle: its three sides are its open boundary.
        let positions = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let edges = outline_edges(&positions, &[0, 1, 2]);
        assert_eq!(edges.len(), 3);
        // A missing vertex, a repeated vertex and a zero-area triangle give
        // nothing, without a panic.
        assert!(outline_edges(&positions, &[0, 1, 9]).is_empty());
        assert!(outline_edges(&positions, &[0, 0, 1]).is_empty());
        let line = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]];
        assert!(outline_edges(&line, &[0, 1, 2]).is_empty());
        assert!(outline_edges(&[], &[]).is_empty());
    }

    #[test]
    fn req_ui_03_add_styled_polyline_sets_the_style_and_width() {
        let mut scene = Scene::default();
        let points = [Point3::new(0.0, 0.0, 0.0), Point3::new(1.0, 0.0, 0.0)];
        scene.add_styled_polyline(&points, [1.0; 4], false, 1.5, LineStyle::Dashed);
        assert_eq!(scene.lines[0].style, LineStyle::Dashed);
        assert_eq!(scene.lines[0].width, 1.5);
        scene.add_polyline(&points, [1.0; 4], false);
        assert_eq!(scene.lines[1].style, LineStyle::Solid);
    }
}
