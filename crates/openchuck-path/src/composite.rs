// SPDX-License-Identifier: AGPL-3.0-or-later
//! The model's surface joined to analytic solids (requirements SET-09 and
//! OP-21; work breakdown slice 21b; decision record 0035 items 1 and 2).
//!
//! The held region of a setup and each retaining tab are part of the target:
//! a strategy that treats the target as the finished part then leaves them
//! alone, with stock to leave, gouge protection and the simulation's target
//! all unchanged. [`CompositeSurface`] is the [`SurfaceSource`] that does
//! this. It holds the model's own source and a list of [`Solid`]s and answers
//! every query as the union of its parts: a tool drop returns the highest
//! contact, a distance or projection the nearest point, a ray the first hit.
//! The port and [`StrategyInput`](crate::StrategyInput) do not change.
//!
//! Ties go to the model, then to the solids in the order given, so the answer
//! does not depend on thread count (NFR-06).

use std::sync::Arc;

use openchuck_types::geometry::{Aabb, Point3, UnitVec3};
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::tool::profile::RevolvedProfile;

use crate::cutter::{Contact, ToolLine};
use crate::holes::Hole;
use crate::solid::Solid;
use crate::surface::{FaceRef, RayHit, SurfacePoint, SurfaceSource};

/// Segments around a solid in the composite's tessellation.
const TESSELLATION_SEGMENTS: u32 = 96;

/// A model surface with analytic solids joined to it, as a
/// [`SurfaceSource`].
#[derive(Debug)]
pub struct CompositeSurface {
    model: Arc<dyn SurfaceSource>,
    solids: Vec<Solid>,
    mesh: TriangleMesh,
    bounds: Aabb,
}

impl CompositeSurface {
    /// Joins `solids` to `model`. With no solids it answers as the model does.
    pub fn new(model: Arc<dyn SurfaceSource>, solids: Vec<Solid>) -> Self {
        let mut mesh = model.tessellation().clone();
        let mut bounds = model.bounds();
        for solid in &solids {
            let offset = u32::try_from(mesh.vertices.len()).unwrap_or(u32::MAX);
            let part = solid.mesh(TESSELLATION_SEGMENTS);
            mesh.vertices.extend(part.vertices);
            mesh.triangles.extend(
                part.triangles
                    .iter()
                    .map(|t| t.map(|i| i.saturating_add(offset))),
            );
            let b = solid.bounds();
            bounds = Aabb {
                min: Point3::new(
                    bounds.min.x.min(b.min.x),
                    bounds.min.y.min(b.min.y),
                    bounds.min.z.min(b.min.z),
                ),
                max: Point3::new(
                    bounds.max.x.max(b.max.x),
                    bounds.max.y.max(b.max.y),
                    bounds.max.z.max(b.max.z),
                ),
            };
        }
        Self {
            model,
            solids,
            mesh,
            bounds,
        }
    }

    /// The model's own surface.
    pub fn model(&self) -> &Arc<dyn SurfaceSource> {
        &self.model
    }

    /// The solids joined to it, in the order given.
    pub fn solids(&self) -> &[Solid] {
        &self.solids
    }
}

impl SurfaceSource for CompositeSurface {
    fn bounds(&self) -> Aabb {
        self.bounds
    }

    fn project(&self, point: Point3) -> Option<SurfacePoint> {
        let mut best = self.model.project(point);
        for solid in &self.solids {
            if let Some(c) = solid.project(point)
                && best.is_none_or(|b| c.point.distance(point) < b.point.distance(point))
            {
                best = Some(c);
            }
        }
        best
    }

    fn cast_ray(&self, origin: Point3, direction: UnitVec3, max_distance: f64) -> Option<RayHit> {
        let mut best = self.model.cast_ray(origin, direction, max_distance);
        for solid in &self.solids {
            if let Some(h) = solid.cast_ray(origin, direction, max_distance)
                && best.is_none_or(|b| h.distance < b.distance)
            {
                best = Some(h);
            }
        }
        best
    }

    fn tessellation(&self) -> &TriangleMesh {
        &self.mesh
    }

    fn holes(&self) -> &[Hole] {
        self.model.holes()
    }

    fn exact_faces(&self) -> Vec<FaceRef<'_>> {
        self.model.exact_faces()
    }

    fn drop_tool(&self, profile: &RevolvedProfile, line: &ToolLine) -> Option<Contact> {
        let mut best = self.model.drop_tool(profile, line);
        for solid in &self.solids {
            if let Some(c) = solid.drop_tool(profile, line)
                && best.is_none_or(|b| c.offset > b.offset)
            {
                best = Some(c);
            }
        }
        best
    }
}
