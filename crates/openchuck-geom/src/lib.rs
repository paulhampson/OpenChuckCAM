// SPDX-License-Identifier: AGPL-3.0-or-later
//! Mesh utilities: adjacency, validity checks, transforms, selection algorithms and ray-cast picking.
//!
//! Architecture section 5.4; requirements IMP-04, IMP-05, UI-04.
//!
//! - [`edges`]: the edge map (which triangles use each edge, and how).
//! - [`check`]: holes, non-manifold edges, flipped normals and degenerate
//!   triangles, reported for the user (IMP-04).
//! - [`transform`]: translate, rotate, scale and mirror with the winding kept
//!   outward, and orientation to the rotary axis (IMP-05).
//!
//! Selection and picking arrive in slice 10. Everything here is format
//! independent and single-threaded, so results are the same on every run
//! (NFR-06).
//!
//! ```
//! use openchuck_geom::check::check_mesh;
//! use openchuck_geom::transform::place;
//! use openchuck_types::geometry::Point3;
//! use openchuck_types::mesh::TriangleMesh;
//! use openchuck_types::transform::{Axis, TransformStep};
//!
//! // A closed tetrahedron, wound anticlockwise seen from outside.
//! let mesh = TriangleMesh {
//!     vertices: vec![
//!         Point3::new(0.0, 0.0, 0.0),
//!         Point3::new(1.0, 0.0, 0.0),
//!         Point3::new(0.0, 1.0, 0.0),
//!         Point3::new(0.0, 0.0, 1.0),
//!     ],
//!     triangles: vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]],
//! };
//! assert!(check_mesh(&mesh, None)?.is_clean());
//!
//! // Mirroring keeps the normals outward.
//! let mirrored = place(&mesh, &[TransformStep::Mirror { axis: Axis::X }])?;
//! assert!(check_mesh(&mirrored, None)?.is_clean());
//! # Ok::<(), openchuck_types::Error>(())
//! ```

#![warn(missing_docs)]

pub mod check;
pub mod edges;
pub mod transform;

use openchuck_types::geometry::Aabb;
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::setup::PartExtent;
use openchuck_types::units::Millimetres;

/// Size figures of a mesh for a model summary.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeshSummary {
    /// Number of vertices.
    pub vertices: usize,
    /// Number of triangles.
    pub triangles: usize,
    /// Bounding box of the vertices that triangles use, if any.
    pub bounds: Option<Aabb>,
    /// Total triangle area in square millimetres.
    pub area: f64,
}

/// Counts, bounding box and surface area of `mesh`.
pub fn summary(mesh: &TriangleMesh) -> MeshSummary {
    let area = (0..mesh.triangles.len())
        .filter_map(|t| mesh.triangle(t))
        .map(|[a, b, c]| (b - a).cross(c - a).length() / 2.0)
        .sum();
    MeshSummary {
        vertices: mesh.vertices.len(),
        triangles: mesh.triangles.len(),
        bounds: transform::bounds(mesh, &transform::Affine3::IDENTITY),
        area,
    }
}

/// The extent of a placed mesh in the model frame: its bounding box and its
/// largest distance from the rotary axis (model X), over the vertices that
/// triangles use. `None` for a mesh with no triangles. Setups check their
/// stock and clearance against it (SET-06).
pub fn part_extent(mesh: &TriangleMesh) -> Option<PartExtent> {
    let bounds = transform::bounds(mesh, &transform::Affine3::IDENTITY)?;
    let radius = (0..mesh.triangles.len())
        .filter_map(|t| mesh.triangle(t))
        .flatten()
        .map(|p| p.y.hypot(p.z))
        .fold(0.0, f64::max);
    Some(PartExtent {
        bounds,
        radius: Millimetres(radius),
    })
}
