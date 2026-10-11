// SPDX-License-Identifier: AGPL-3.0-or-later
//! Indexed triangle meshes.

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::geometry::Point3;

/// An indexed triangle mesh in millimetres.
///
/// Validity checks beyond indexing and finiteness (holes, non-manifold edges,
/// flipped normals, IMP-04) live in `openchuck-geom`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TriangleMesh {
    /// Vertex positions.
    pub vertices: Vec<Point3>,
    /// Triangles as counter-clockwise vertex indices (outward normal by the right-hand rule).
    pub triangles: Vec<[u32; 3]>,
}

impl TriangleMesh {
    /// Fails if a vertex is not finite or a triangle refers to a missing vertex.
    pub fn validate(&self) -> Result<()> {
        for (i, v) in self.vertices.iter().enumerate() {
            v.validate(&format!("vertices[{i}]"))?;
        }
        let count = self.vertices.len();
        for (i, tri) in self.triangles.iter().enumerate() {
            for &index in tri {
                if usize::try_from(index).map_or(true, |index| index >= count) {
                    return Err(Error::invalid(
                        format!("triangles[{i}]"),
                        format!("vertex index {index} is out of range (mesh has {count} vertices)"),
                    ));
                }
            }
        }
        Ok(())
    }

    /// The corners of triangle `index`, or `None` if it or a vertex is missing.
    pub fn triangle(&self, index: usize) -> Option<[Point3; 3]> {
        let [a, b, c] = *self.triangles.get(index)?;
        let get = |i: u32| self.vertices.get(usize::try_from(i).ok()?).copied();
        Some([get(a)?, get(b)?, get(c)?])
    }
}
