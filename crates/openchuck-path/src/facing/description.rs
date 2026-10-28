// SPDX-License-Identifier: AGPL-3.0-or-later
//! One nominal coverage/target description for viewport and Setup (0252).
//! Geometry describes the supplied placed mesh; source accuracy stays explicit.
//! It is not a sweep, clearance or completion certificate.

use openchuck_types::geometry::Point3;
use openchuck_types::units::Degrees;

use super::extents::FacingExtent;
use super::geometry::{CoverageRectangle, EndTarget, FacingEnd, FacingPlane};
use super::parameter;
use super::plan::FacingSetup;
use super::selection::{ResolvedSide, SideSelection};
use crate::error::{PathError, Result};
use crate::params::{Parameters, names};
use crate::rotary::RotaryMapping;

/// An indexed plane with its requested coverage, before cutter overlap.
#[derive(Debug, Clone, PartialEq)]
pub struct FaceDescription {
    /// Nominal target plane (legacy Flat can retain protected protrusions).
    pub target: ResolvedSide,
    /// Requested rectangle, already including the user-entered margin.
    pub coverage: CoverageRectangle,
}
impl FaceDescription {
    /// Four corners in winding order for both host diagrams. No guessed
    /// silhouette, projected AABB or tool-centre offset enters these points.
    pub fn corners(&self) -> Result<[Point3; 4]> {
        let [u0, v0] = self.coverage.min();
        let [u1, v1] = self.coverage.max();
        let point = |u, v| self.target.plane.point(u, v, self.target.offset);
        Ok([
            point(u0, v0)?,
            point(u1, v0)?,
            point(u1, v1)?,
            point(u0, v1)?,
        ])
    }
}

/// Nominal finished setting and requested coverage. Protected geometry and
/// intentional leftovers remain distinct from this surface in every mode.
#[derive(Debug, Clone, PartialEq)]
pub enum FacingGeometry {
    /// Legacy Flat nominal settings; the existing composite target may rise
    /// above these planes and must remain visible in completion reporting.
    Flat {
        /// In legacy angle-list order.
        faces: Vec<FaceDescription>,
    },
    /// Four selected box faces followed by ordered arbitrary A faces.
    Box {
        /// In effective cut order.
        faces: Vec<FaceDescription>,
    },
    /// End selected from actual workholding, never an inferred tip orientation.
    End {
        /// Which unheld end.
        end: FacingEnd,
        /// Model-derived X and length, excluding tabs and held stock.
        target: EndTarget,
        /// YZ rectangle at the target X.
        coverage: CoverageRectangle,
    },
    /// Legacy round target; coverage has full circumference in this X span.
    Round {
        /// Existing finished-diameter setting; margin never changes it.
        diameter: f64,
        /// Axial coverage including margin at both ends.
        x_extent: (f64, f64),
    },
}

/// Coverage choices and source-bound status accompanying nominal geometry.
#[derive(Debug, Clone, PartialEq)]
pub struct FacingDescription {
    /// Requested geometry; cutter sweep is a separate generated result.
    pub geometry: FacingGeometry,
    /// The selected meaning of the bounding geometry.
    pub extent: FacingExtent,
    /// User-entered margin, separate from extension and source error.
    pub margin: f64,
    /// Bound from the placed mesh to the original source; None is unknown.
    /// Hosts must not present an unknown STEP projection as certified exact.
    pub source_projection_error: Option<f64>,
}

/// Describe the stored settings using whole placed geometry. This neither
/// changes targets nor grants permission to cut tabs in Part plus tabs.
pub fn describe(setup: &FacingSetup<'_>, parameters: &Parameters) -> Result<FacingDescription> {
    setup.part.validate()?;
    setup.stock.validate()?;
    setup.machine.validate()?;
    if setup
        .projection_error
        .is_some_and(|e| !e.is_finite() || e < 0.)
    {
        return Err(PathError::parameter(
            "projection error",
            "must be finite and nonnegative, or explicitly unknown",
        ));
    }
    let extent = FacingExtent::from_name(parameters.text(parameter::EXTENT)?)?;
    let margin = parameters.number(parameter::MARGIN)?;
    let rotary = RotaryMapping::of_profile(setup.machine);
    let coverage = |plane| extent.planar(plane, setup.stock, setup.part, setup.tabs, margin);
    let face = |target: ResolvedSide| -> Result<FaceDescription> {
        Ok(FaceDescription {
            coverage: coverage(target.plane)?,
            target,
        })
    };
    let geometry = match parameters.text(parameter::SHAPE)? {
        parameter::ROUND => FacingGeometry::Round {
            diameter: parameters.number(parameter::SIZE)?,
            x_extent: extent.axial(setup.stock, setup.part, setup.tabs, margin)?,
        },
        parameter::END => {
            let end = FacingEnd::opposite_held(setup.held[0].is_some(), setup.held[1].is_some())?;
            let part_x = FacingExtent::Part.axial(setup.stock, setup.part, &[], 0.)?;
            let target = end.target(part_x, parameters.number(names::STOCK_TO_LEAVE)?)?;
            FacingGeometry::End {
                end,
                target,
                coverage: coverage(FacingPlane::end(end))?,
            }
        }
        parameter::FLAT => {
            let offset = parameters.number(parameter::SIZE)?;
            let faces = parameters
                .angles(parameter::ANGLES)?
                .into_iter()
                .map(|angle| {
                    let angle = Degrees(angle);
                    face(ResolvedSide {
                        angle,
                        offset,
                        plane: FacingPlane::side(rotary, angle)?,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            FacingGeometry::Flat { faces }
        }
        parameter::BOX => {
            let selection = SideSelection::read(parameters)?;
            let vertices = setup
                .part
                .triangles
                .iter()
                .flatten()
                .filter_map(|i| {
                    usize::try_from(*i)
                        .ok()
                        .and_then(|i| setup.part.vertices.get(i))
                        .copied()
                })
                .collect::<Vec<_>>();
            let faces = selection
                .resolve(rotary, &vertices, parameters.number(names::STOCK_TO_LEAVE)?)?
                .into_iter()
                .map(face)
                .collect::<Result<Vec<_>>>()?;
            FacingGeometry::Box { faces }
        }
        _ => {
            return Err(PathError::parameter(
                parameter::SHAPE,
                "unknown facing mode",
            ));
        }
    };
    Ok(FacingDescription {
        geometry,
        extent,
        margin,
        source_projection_error: setup.projection_error,
    })
}
