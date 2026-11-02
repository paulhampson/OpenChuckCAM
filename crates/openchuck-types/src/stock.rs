// SPDX-License-Identifier: AGPL-3.0-or-later
//! Stock definitions (SET-03, SET-04). Coordinates are in the model frame,
//! whose X axis is the rotary axis.

use serde::{Deserialize, Serialize};

use crate::error::{Result, require_non_negative, require_positive};
use crate::geometry::Aabb;
use crate::mesh::TriangleMesh;
use crate::units::Millimetres;

/// The material before machining.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
#[non_exhaustive]
pub enum Stock {
    /// A cylinder coaxial with the rotary axis, from `x_start` to `x_start + length`.
    Cylinder {
        /// Diameter.
        diameter: Millimetres,
        /// Length along X.
        length: Millimetres,
        /// Start position along X.
        x_start: Millimetres,
    },
    /// A box in the model frame.
    Box {
        /// The box.
        bounds: Aabb,
    },
    /// The model's bounding box grown by `offset` on every side.
    OffsetBoundingBox {
        /// Offset added on every side.
        offset: Millimetres,
    },
    /// An imported solid.
    Imported {
        /// The solid as a closed mesh.
        mesh: TriangleMesh,
    },
}

impl Stock {
    /// Fails if a dimension is not finite or out of range.
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::Cylinder {
                diameter,
                length,
                x_start,
            } => {
                require_positive("stock.diameter", diameter.0)?;
                require_positive("stock.length", length.0)?;
                crate::error::require_finite("stock.x_start", x_start.0)
            }
            Self::Box { bounds } => bounds.validate("stock.bounds"),
            Self::OffsetBoundingBox { offset } => require_non_negative("stock.offset", offset.0),
            Self::Imported { mesh } => mesh.validate(),
        }
    }
}
