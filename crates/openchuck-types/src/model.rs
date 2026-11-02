// SPDX-License-Identifier: AGPL-3.0-or-later
//! Model formats and kinds (IMP-01, IMP-02).
//!
//! A model is a mesh (STL) or a B-rep (STEP). The kind decides how it is
//! selected: a mesh by triangle, a B-rep by face and edge (architecture
//! section 5.6). The format is known from the file name's extension, which is
//! also how an importer is chosen, so a project can check its selections
//! without reading the model.

use serde::{Deserialize, Serialize};

use crate::brep::BrepModel;
use crate::mesh::TriangleMesh;

/// A model file format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum ModelFormat {
    /// STL, ASCII or binary (IMP-01).
    Stl,
    /// STEP (IMP-02, slice 11).
    Step,
}

/// Whether a model is a triangle mesh or a boundary representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelKind {
    /// Triangles only; selected by triangle.
    Mesh,
    /// Faces and edges; selected by face and edge.
    Brep,
}

impl ModelFormat {
    /// The format named by the extension of `file_name` (ignoring case):
    /// `.stl`, or `.step` and `.stp`.
    pub fn from_file_name(file_name: &str) -> Option<Self> {
        let (_, extension) = file_name.rsplit_once('.')?;
        match extension.to_ascii_lowercase().as_str() {
            "stl" => Some(Self::Stl),
            "step" | "stp" => Some(Self::Step),
            _ => None,
        }
    }

    /// The kind of model this format holds.
    pub const fn kind(self) -> ModelKind {
        match self {
            Self::Stl => ModelKind::Mesh,
            Self::Step => ModelKind::Brep,
        }
    }

    /// The format's name for messages.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Stl => "STL",
            Self::Step => "STEP",
        }
    }
}

/// Imported geometry in millimetres.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelGeometry {
    /// A triangle mesh. Triangle `i` is the `i`th facet of the file, so
    /// triangle indices are stable for the same file.
    Mesh(TriangleMesh),
    /// A B-rep with stable face and edge IDs.
    Brep(BrepModel),
}

impl ModelGeometry {
    /// The kind of model.
    pub const fn kind(&self) -> ModelKind {
        match self {
            Self::Mesh(_) => ModelKind::Mesh,
            Self::Brep(_) => ModelKind::Brep,
        }
    }
}
