// SPDX-License-Identifier: AGPL-3.0-or-later
//! Whole-stock, part and part-plus-tabs coverage (OP-24; decision0244).
//! The caller supplies tabs separately from held solids: extent membership
//! never changes either kind's protection during generation or simulation.

use openchuck_types::geometry::{Aabb, Point3, UnitVec3};
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::stock::Stock;

use super::geometry::{CoverageRectangle, FacingPlane, projection_range};
use crate::error::{PathError, Result};
use crate::solid::Solid;

/// Facing's explicit coverage choice; old/missing settings mean Whole stock.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FacingExtent {
    /// Entire initial stock projected into the facing plane.
    #[default]
    WholeStock,
    /// Entire placed part's projected bounding rectangle.
    Part,
    /// Projected bounds of the placed part and every retaining tab together.
    PartPlusTabs,
}

impl FacingExtent {
    /// Read the stable parameter value, shared by previews and planning.
    pub fn from_name(value: &str) -> Result<Self> {
        match value {
            "whole_stock" => Ok(Self::WholeStock),
            "part" => Ok(Self::Part),
            "part_plus_tabs" => Ok(Self::PartPlusTabs),
            _ => Err(PathError::parameter("extent", "unknown facing extent")),
        }
    }
    /// Outermost support of this extent's geometry in a facing normal.
    /// Whole-stock support starts the new side layers; part support derives
    /// target dimensions. This does not include a margin or stock allowance.
    pub fn support(
        self,
        plane: FacingPlane,
        stock: &Stock,
        part: &TriangleMesh,
        tabs: &[Solid],
    ) -> Result<f64> {
        self.project(stock, part, tabs, |mesh, solids| {
            plane.support(mesh_points(mesh), solids)
        })
    }

    /// Planar extent with margin on all four edges. This is not a silhouette
    /// or a target-plane section. The part must be the whole placed mesh, not
    /// selected faces. STEP hosts must establish their projection error first.
    pub fn planar(
        self,
        plane: FacingPlane,
        stock: &Stock,
        part: &TriangleMesh,
        tabs: &[Solid],
        margin: f64,
    ) -> Result<CoverageRectangle> {
        self.project(stock, part, tabs, |mesh, solids| {
            plane.bounds(mesh_points(mesh), solids, margin)
        })
    }

    /// Round Facing's X span; its circumference remains complete. Margin is
    /// axial only, not a radial change to the requested finished diameter.
    pub fn axial(
        self,
        stock: &Stock,
        part: &TriangleMesh,
        tabs: &[Solid],
        margin: f64,
    ) -> Result<(f64, f64)> {
        if !margin.is_finite() || margin < 0.0 {
            return Err(PathError::parameter(
                "axial margin",
                "must be finite and nonnegative",
            ));
        }
        self.project(stock, part, tabs, |mesh, solids| {
            let (lo, hi) = projection_range(UnitVec3::X, mesh_points(mesh), solids)?;
            let (lo, hi) = (lo - margin, hi + margin);
            if !lo.is_finite() || !hi.is_finite() {
                return Err(PathError::parameter(
                    "axial margin",
                    "expanded extent overflowed",
                ));
            }
            Ok((lo, hi))
        })
    }

    fn project<T>(
        self,
        stock: &Stock,
        part: &TriangleMesh,
        tabs: &[Solid],
        project: impl FnOnce(&TriangleMesh, &[Solid]) -> Result<T>,
    ) -> Result<T> {
        stock.validate()?;
        validate_part(part)?;
        match self {
            Self::Part => return project(part, &[]),
            Self::PartPlusTabs => return project(part, tabs),
            Self::WholeStock => {}
        }
        let solid = match stock {
            Stock::Box { bounds } => Solid::cuboid(*bounds)?,
            Stock::Cylinder {
                diameter,
                length,
                x_start,
            } => Solid::cylinder(
                Point3::new(x_start.0, 0.0, 0.0),
                UnitVec3::X,
                length.0,
                diameter.0 * 0.5,
            )?,
            Stock::OffsetBoundingBox { offset } => {
                let (x0, x1) = projection_range(UnitVec3::X, mesh_points(part), &[])?;
                let (y0, y1) = projection_range(UnitVec3::Y, mesh_points(part), &[])?;
                let (z0, z1) = projection_range(UnitVec3::Z, mesh_points(part), &[])?;
                Solid::cuboid(Aabb {
                    min: Point3::new(x0 - offset.0, y0 - offset.0, z0 - offset.0),
                    max: Point3::new(x1 + offset.0, y1 + offset.0, z1 + offset.0),
                })?
            }
            Stock::Imported { mesh } => {
                validate_part(mesh)?;
                return project(mesh, &[]);
            }
            _ => {
                return Err(PathError::parameter(
                    "facing extent",
                    "this stock type has no supported projection",
                ));
            }
        };
        project(&TriangleMesh::default(), &[solid])
    }
}

fn validate_part(mesh: &TriangleMesh) -> Result<()> {
    mesh.validate()?;
    if mesh.triangles.is_empty() {
        return Err(PathError::parameter(
            "facing geometry",
            "the whole placed mesh has no triangles",
        ));
    }
    Ok(())
}

fn mesh_points(mesh: &TriangleMesh) -> impl Iterator<Item = Point3> + '_ {
    // Only referenced vertices belong to the surface. Validation occurs
    // before iteration; checked lookup still avoids unchecked library access.
    mesh.triangles.iter().flatten().filter_map(|&i| {
        usize::try_from(i)
            .ok()
            .and_then(|i| mesh.vertices.get(i))
            .copied()
    })
}
