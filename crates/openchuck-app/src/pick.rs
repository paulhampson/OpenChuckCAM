// SPDX-License-Identifier: AGPL-3.0-or-later
//! Picking in the scene: which triangle of which model is under a ray, and
//! what a pick selects (UI-04; work breakdown slice 10 item 1; decision
//! record 0036).
//!
//! The scene job builds a [`ScenePicker`] with the part meshes it places, so
//! the hierarchy is built off the interface thread (NFR-04) and costs
//! nothing at the pointer. The viewport turns the pointer into a ray with
//! `Camera::ray` and asks [`ScenePicker::pick`]; it only draws the highlight,
//! which is [`crate::scene::triangles_mesh`] of the triangles chosen. What a
//! click selects is a [`PickMode`], which maps the pick to a
//! [`RegionRequest`] for [`Application::select_region`].
//!
//! [`Application::select_region`]: crate::Application::select_region

use std::sync::Arc;

use openchuck_geom::pick::MeshPicker;
use openchuck_project::id::ModelId;
use openchuck_types::geometry::{Point3, Vec3};

use crate::scene::{SceneMesh, SceneRole};
use crate::{AppError, RegionRequest};

/// A triangle under a ray.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScenePick {
    /// Index of the mesh in the scene's `meshes`.
    pub mesh: usize,
    /// The model the mesh shows, when it shows one.
    pub model: Option<ModelId>,
    /// Index of the triangle: facet `triangle` of the model file, whatever
    /// the placement (decision record 0032).
    pub triangle: u32,
    /// Distance from the ray's origin, millimetres.
    pub distance: f64,
    /// The point hit, in the model frame.
    pub point: Point3,
    /// The triangle's unit normal in the model frame; `None` when it has no
    /// area.
    pub normal: Option<Vec3>,
}

struct PartPicker {
    mesh: usize,
    model: Option<ModelId>,
    picker: MeshPicker,
}

/// The hierarchies of the scene's part meshes.
pub struct ScenePicker {
    parts: Vec<PartPicker>,
}

impl PartialEq for ScenePicker {
    fn eq(&self, other: &Self) -> bool {
        self.parts.len() == other.parts.len()
            && self
                .parts
                .iter()
                .zip(&other.parts)
                .all(|(a, b)| a.mesh == b.mesh && a.model == b.model && a.picker == b.picker)
    }
}

impl std::fmt::Debug for ScenePicker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScenePicker")
            .field("parts", &self.parts.len())
            .finish()
    }
}

impl ScenePicker {
    /// Builds the hierarchy of every part mesh in `meshes`. A part with no
    /// triangles is left out.
    pub(crate) fn build(meshes: &[SceneMesh]) -> Result<Self, AppError> {
        let mut parts = Vec::new();
        for (index, item) in meshes.iter().enumerate() {
            if item.role == SceneRole::Part && !item.mesh.triangles.is_empty() {
                parts.push(PartPicker {
                    mesh: index,
                    model: item.model,
                    picker: MeshPicker::new(&item.mesh)?,
                });
            }
        }
        Ok(Self { parts })
    }

    /// Whether nothing can be picked.
    pub fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }

    /// The nearest triangle the ray from `origin` along `direction` meets in
    /// any part, or `None`. Fails for a ray that is not finite or has no
    /// direction.
    pub fn pick(
        &self,
        origin: [f64; 3],
        direction: [f64; 3],
    ) -> Result<Option<ScenePick>, AppError> {
        let o = Point3::new(origin[0], origin[1], origin[2]);
        let d = Vec3::new(direction[0], direction[1], direction[2]);
        let mut best: Option<ScenePick> = None;
        for part in &self.parts {
            if let Some(hit) = part.picker.pick(o, d)?
                && best.is_none_or(|b| hit.distance < b.distance)
            {
                best = Some(ScenePick {
                    mesh: part.mesh,
                    model: part.model,
                    triangle: hit.triangle,
                    distance: hit.distance,
                    point: hit.point,
                    normal: part.picker.normal(hit.triangle),
                });
            }
        }
        Ok(best)
    }
}

/// A scene's picker, shared between the scene view and the interface. Two
/// handles are equal when they are the same picker or pickers built from the
/// same meshes, so two builds of one scene compare equal (NFR-06).
#[derive(Clone, Default)]
pub struct Picking(Option<Arc<ScenePicker>>);

impl Picking {
    pub(crate) fn new(picker: ScenePicker) -> Self {
        Self(Some(Arc::new(picker)))
    }

    /// The picker, when the scene has anything to pick.
    pub fn picker(&self) -> Option<&ScenePicker> {
        self.0.as_deref().filter(|p| !p.is_empty())
    }
}

impl std::fmt::Debug for Picking {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            Some(p) => p.fmt(f),
            None => f.write_str("Picking(none)"),
        }
    }
}

impl PartialEq for Picking {
    fn eq(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (None, None) => true,
            (Some(a), Some(b)) => Arc::ptr_eq(a, b) || a == b,
            _ => false,
        }
    }
}

/// What a click selects (UI-04).
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum PickMode {
    /// The triangle clicked, alone.
    Triangle,
    /// The whole connected shell the triangle is in.
    Connected,
    /// The smooth patch around the triangle: the connected triangles reached
    /// without turning more than `max_edge_angle` degrees at an edge.
    SmoothPatch {
        /// The sharpest edge crossed, degrees, 0 to 180.
        max_edge_angle: f64,
    },
    /// Every triangle facing within `max_angle` degrees of the clicked
    /// triangle's direction, wherever it is on the part.
    SameDirection {
        /// The widest angle from the direction, degrees, 0 to 180.
        max_angle: f64,
    },
}

impl PickMode {
    /// The request that selects what `pick` stands for, to give
    /// [`Application::select_region`](crate::Application::select_region) on
    /// the pick's model; `None` when the pick selects the triangle alone (the
    /// mode is [`Triangle`](Self::Triangle), or the mode needs the
    /// triangle's direction and it has none).
    pub fn request(&self, pick: &ScenePick) -> Option<RegionRequest> {
        match *self {
            Self::Triangle => None,
            Self::Connected => Some(RegionRequest::FloodFill {
                seeds: vec![pick.triangle],
            }),
            Self::SmoothPatch { max_edge_angle } => Some(RegionRequest::FloodFillByAngle {
                seeds: vec![pick.triangle],
                max_edge_angle,
            }),
            Self::SameDirection { max_angle } => {
                pick.normal.map(|direction| RegionRequest::ByDirection {
                    direction,
                    max_angle,
                })
            }
        }
    }
}
