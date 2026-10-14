// SPDX-License-Identifier: AGPL-3.0-or-later
//! Display data derived from a mesh: the feature edges the viewport draws in
//! "shaded with edges" mode (UI-02; decision record 0029 item 4).
//!
//! A feature edge is an edge where the surface turns sharply, plus the edges
//! that do not join exactly two triangles (boundaries and non-manifold
//! edges). Drawing every triangle edge roughly halves the frame rate and
//! hides the shape under the tessellation (decision record 0002, "Findings
//! for later slices"), so the viewport draws these instead.

use openchuck_types::Result;
use openchuck_types::geometry::Vec3;
use openchuck_types::mesh::TriangleMesh;

use crate::edges::EdgeMap;

/// The default crease angle in degrees: an edge whose two triangles' normals
/// differ by more than this is a feature edge.
pub const DEFAULT_CREASE_DEGREES: f64 = 30.0;

/// The feature edges of `mesh` as `[low, high]` vertex indices, in the edge
/// map's order, so the result is the same on every run (NFR-06).
///
/// An edge is a feature edge when it is used by one triangle or by more than
/// two, or when its two triangles' normals differ by more than
/// `crease_degrees`. A triangle with no area has no normal, so an edge it
/// shares is a feature edge. Fails if the mesh is invalid or the angle is not
/// finite.
pub fn feature_edges(mesh: &TriangleMesh, crease_degrees: f64) -> Result<Vec<[u32; 2]>> {
    if !crease_degrees.is_finite() {
        return Err(openchuck_types::Error::invalid(
            "crease_degrees",
            "the crease angle must be a finite number of degrees",
        ));
    }
    let map = EdgeMap::new(mesh)?;
    let cos_limit = crease_degrees.clamp(0.0, 180.0).to_radians().cos();
    let normals: Vec<Option<Vec3>> = mesh
        .triangles
        .iter()
        .map(|t| triangle_normal(mesh, *t))
        .collect();
    let normal = |triangle: u32| normals.get(triangle as usize).copied().flatten();
    let mut edges = Vec::new();
    for (e, &edge) in map.edges().iter().enumerate() {
        let feature = match map.uses(e) {
            [a, b] => match (normal(a.triangle), normal(b.triangle)) {
                (Some(na), Some(nb)) => na.dot(nb) < cos_limit,
                _ => true,
            },
            _ => true,
        };
        if feature {
            edges.push(edge);
        }
    }
    Ok(edges)
}

/// The unit normal of a triangle by the right-hand rule, or `None` when it has
/// no area.
fn triangle_normal(mesh: &TriangleMesh, [a, b, c]: [u32; 3]) -> Option<Vec3> {
    let p = |i: u32| mesh.vertices.get(i as usize).copied();
    let (a, b, c) = (p(a)?, p(b)?, p(c)?);
    let n = (b - a).cross(c - a);
    let length = n.length();
    (length > 0.0 && length.is_finite()).then(|| n * (1.0 / length))
}
