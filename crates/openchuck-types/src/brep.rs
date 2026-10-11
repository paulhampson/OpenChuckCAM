// SPDX-License-Identifier: AGPL-3.0-or-later
//! Boundary-representation data handed from the STEP importer to the libraries (INT-02).
//!
//! Face and edge IDs are assigned deterministically by the importer, so the
//! same file always gives the same IDs (architecture section 5.4). The exact
//! surface description is deliberately minimal until spikes SP-1 (STEP kernel)
//! and SP-9 (exact surfaces) report; only the surface kind and a tessellation
//! are carried for now.

use serde::{Deserialize, Serialize};

use crate::geometry::Point3;
use crate::mesh::TriangleMesh;

/// A stable face identifier within one model file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FaceId(pub u32);

/// A stable edge identifier within one model file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EdgeId(pub u32);

/// The kind of surface underlying a face.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SurfaceKind {
    /// A plane.
    Plane,
    /// A circular cylinder.
    Cylinder,
    /// A circular cone.
    Cone,
    /// A sphere.
    Sphere,
    /// A torus.
    Torus,
    /// A B-spline or NURBS surface.
    BSpline,
    /// Any other surface type.
    Other,
}

/// One B-rep face.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrepFace {
    /// Stable identifier.
    pub id: FaceId,
    /// Underlying surface kind.
    pub surface: SurfaceKind,
    /// Bounding edges.
    pub edges: Vec<EdgeId>,
    /// Tessellation at the model's tolerance (IMP-06).
    pub tessellation: TriangleMesh,
}

/// One B-rep edge.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrepEdge {
    /// Stable identifier.
    pub id: EdgeId,
    /// Faces that share this edge.
    pub faces: Vec<FaceId>,
    /// The edge sampled as a polyline at the model's tolerance.
    pub polyline: Vec<Point3>,
}

/// A B-rep model: faces and edges with stable IDs.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrepModel {
    /// Faces, ordered by ID.
    pub faces: Vec<BrepFace>,
    /// Edges, ordered by ID.
    pub edges: Vec<BrepEdge>,
}
