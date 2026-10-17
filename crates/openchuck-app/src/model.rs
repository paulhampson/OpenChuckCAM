// SPDX-License-Identifier: AGPL-3.0-or-later
//! Models: adding a model file, inspecting it, its units and its placement
//! (IMP-01, IMP-03, IMP-04, IMP-05; work breakdown slice 2).
//!
//! Inspecting a model imports it (`openchuck-import`), places it in the model
//! frame with its transform steps and checks the placed mesh
//! (`openchuck-geom`). The types a front end needs are re-exported here, so
//! it needs no engine crate of its own (architecture section 5.8).

use std::path::{Component, Path, PathBuf};

use openchuck_geom::check::check_mesh;
use openchuck_geom::transform::{
    brep_placement, placement, transform_brep, transform_mesh, transform_normals,
};
use openchuck_import::{ImportOptions, ImportedModel, import_bytes};
use openchuck_project::{ContentHash, ModelRef};
use openchuck_types::brep::BrepModel;
use openchuck_types::job::JobContext;
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::model::ModelGeometry;

use crate::AppError;

pub use openchuck_geom::MeshSummary;
pub use openchuck_geom::check::{Hole, MeshReport};
pub use openchuck_geom::transform::{AxisDirection, orient_to_rotary_axis};
pub use openchuck_import::{
    ImportError, LARGE_PART_MM, SMALL_PART_MM, STL_TESSELLATION_NOTICE, UnitConcern, UnitQuestion,
    UnitSource, Units,
};
pub use openchuck_types::model::{ModelFormat, ModelKind};
pub use openchuck_types::setup::PartExtent;
pub use openchuck_types::transform::{Axis, TransformStep};
pub use openchuck_types::units::{Degrees, LengthUnit};

/// What inspecting a model found: its size in the model frame, the mesh
/// checks (IMP-04), the units used (IMP-03) and notes for the user.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelInspection {
    /// The model file.
    pub path: PathBuf,
    /// Its format.
    pub format: ModelFormat,
    /// The units it was read in.
    pub units: Units,
    /// A question for the user about the unit, raised when no unit is set
    /// for the model and the file names another unit or the part's size is
    /// implausible in millimetres (IMP-03). A front end asks it and stores
    /// the answer with [`Command::SetModelUnits`](crate::Command::SetModelUnits).
    pub unit_question: Option<UnitQuestion>,
    /// The placement applied (IMP-05).
    pub transform: Vec<TransformStep>,
    /// Counts, bounds and area after placement, in millimetres.
    pub summary: MeshSummary,
    /// Bounds and largest radius about the rotary axis after placement, for
    /// the setup checks (SET-06); `None` for a mesh with no triangles.
    pub extent: Option<PartExtent>,
    /// The mesh checks.
    pub report: MeshReport,
    /// Notes from the import and the application, such as the STL
    /// tessellation statement.
    pub notes: Vec<String>,
}

/// Reads `bytes` as the model file `path`, places it and checks it.
pub(crate) fn inspect_bytes(
    path: &Path,
    bytes: &[u8],
    units: Option<LengthUnit>,
    transform: &[TransformStep],
    context: &JobContext<'_>,
) -> Result<ModelInspection, AppError> {
    inspect_placed(path, bytes, units, transform, context).map(|(inspection, _)| inspection)
}

/// [`inspect_bytes`], also returning the placed mesh.
pub(crate) fn inspect_placed(
    path: &Path,
    bytes: &[u8],
    units: Option<LengthUnit>,
    transform: &[TransformStep],
    context: &JobContext<'_>,
) -> Result<(ModelInspection, TriangleMesh), AppError> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let model = import_bytes(
        &name,
        bytes,
        &ImportOptions {
            units,
            ..ImportOptions::default()
        },
        context,
    )?;
    context.check_cancelled()?;
    context.report("check mesh", 0, None);
    let (placed, map, _) = place(&model, transform)?;
    let normals = model
        .stated_normals
        .as_deref()
        .map(|normals| transform_normals(normals, &map));
    let report = check_mesh(&placed, normals.as_deref())?;
    let inspection = ModelInspection {
        path: path.to_path_buf(),
        format: model.format,
        units: model.units,
        unit_question: model.unit_question,
        transform: transform.to_vec(),
        summary: openchuck_geom::summary(&placed),
        extent: openchuck_geom::part_extent(&placed),
        report,
        notes: model.notes,
    };
    Ok((inspection, placed))
}

/// A model file read, hashed and checked, ready to be added to the project
/// with [`crate::Application::add_prepared_model`].
#[derive(Debug, Clone, PartialEq)]
pub struct PreparedModel {
    pub(crate) reference: ModelRef,
    pub(crate) inspection: ModelInspection,
}

impl PreparedModel {
    /// What the checks found.
    pub fn inspection(&self) -> &ModelInspection {
        &self.inspection
    }
}

/// A model file checked before it is added: the inspection, and the scene
/// of the placed model with the triangles and edges the checks list
/// highlighted (OI-51; decision record 0029 item 13).
#[derive(Debug, Clone, PartialEq)]
pub struct ImportPreview {
    /// What the checks found.
    pub inspection: ModelInspection,
    /// The placed model, its problems and the rotary axis.
    pub scene: crate::scene::SceneView,
}

/// Reads, places and checks the model file `path`, and builds its preview.
pub(crate) fn import_preview(
    path: &Path,
    units: Option<LengthUnit>,
    transform: &[TransformStep],
    context: &JobContext<'_>,
) -> Result<ImportPreview, AppError> {
    let bytes = read(path)?;
    let (inspection, placed) = inspect_placed(path, &bytes, units, transform, context)?;
    let scene = crate::scene::model_scene(placed, &inspection.report)?;
    Ok(ImportPreview { inspection, scene })
}

/// Imports and checks the model file at `path` without a project, read in
/// `units` (or the file's own) and placed by `transform`.
pub fn inspect_file(
    path: &Path,
    units: Option<LengthUnit>,
    transform: &[TransformStep],
    context: &JobContext<'_>,
) -> Result<ModelInspection, AppError> {
    let bytes = read(path)?;
    inspect_bytes(path, &bytes, units, transform, context)
}

/// Inspects a project's model: its units override and transform apply, and
/// a file that has changed since it was added is noted (PRJ-02).
pub(crate) fn inspect_model(
    project_root: &Path,
    model: &ModelRef,
    context: &JobContext<'_>,
) -> Result<ModelInspection, AppError> {
    let path = model.path.resolve(project_root);
    let bytes = read(&path)?;
    let mut inspection = inspect_bytes(&path, &bytes, model.units, &model.transform, context)?;
    if ContentHash::of_bytes(&bytes) != model.hash {
        inspection.notes.push(format!(
            "'{}' has changed since it was added to the project (PRJ-02)",
            model.path.as_str()
        ));
    }
    Ok(inspection)
}

/// An imported model placed in the model frame (IMP-05): its triangles, and
/// for a STEP model its B-rep placed by the same map. A STEP model's mesh is
/// its faces' tessellation joined in face order, which is what the viewport
/// draws, the simulation compares with and the mesh strategies cut.
fn place(
    model: &ImportedModel,
    transform: &[TransformStep],
) -> Result<
    (
        TriangleMesh,
        openchuck_geom::transform::Affine3,
        Option<BrepModel>,
    ),
    AppError,
> {
    Ok(match &model.geometry {
        ModelGeometry::Mesh(mesh) => {
            let map = placement(mesh, transform)?;
            (transform_mesh(mesh, &map), map, None)
        }
        ModelGeometry::Brep(brep) => {
            let (placed, map) = PlacedPart::place_brep(brep, transform)?;
            (placed.joined_tessellation().0, map, Some(placed))
        }
    })
}

/// A project's part as the pipeline reads it.
#[derive(Debug, Clone)]
pub(crate) struct PlacedPart {
    /// The placed triangles: the mesh, or a STEP model's joined tessellation.
    pub mesh: TriangleMesh,
    /// The placed B-rep of a STEP model.
    pub brep: Option<BrepModel>,
    /// The hash of the file as read now, which selection sets are checked
    /// against (PRJ-06).
    pub hash: ContentHash,
}

impl PlacedPart {
    fn place_brep(
        brep: &BrepModel,
        transform: &[TransformStep],
    ) -> Result<(BrepModel, openchuck_geom::transform::Affine3), AppError> {
        let map = brep_placement(brep, transform)?;
        Ok((transform_brep(brep, &map), map))
    }

    /// The part of a B-rep model read from a file with hash `hash`, placed by
    /// `transform`.
    #[cfg(test)]
    pub(crate) fn of_brep(
        brep: &BrepModel,
        transform: &[TransformStep],
        hash: ContentHash,
    ) -> Result<Self, AppError> {
        let (placed, _) = Self::place_brep(brep, transform)?;
        Ok(Self {
            mesh: placed.joined_tessellation().0,
            brep: Some(placed),
            hash,
        })
    }
}

/// Imports a project's model and places it in the model frame, for the
/// pathing library (work breakdown slice 5 item 10), keeping a STEP model's
/// B-rep (decision 0056).
pub(crate) fn placed_part(
    project_root: &Path,
    model: &ModelRef,
    context: &JobContext<'_>,
) -> Result<PlacedPart, AppError> {
    let path = model.path.resolve(project_root);
    let bytes = read(&path)?;
    let hash = ContentHash::of_bytes(&bytes);
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let imported = import_bytes(
        &name,
        &bytes,
        &ImportOptions {
            units: model.units,
            ..ImportOptions::default()
        },
        context,
    )?;
    let (mesh, _, brep) = place(&imported, &model.transform)?;
    Ok(PlacedPart { mesh, brep, hash })
}

/// The placed triangles of a project's model: a mesh, or a STEP model's
/// joined tessellation, for drawing, the workholding checks and the
/// simulation.
pub(crate) fn placed_mesh(
    project_root: &Path,
    model: &ModelRef,
    context: &JobContext<'_>,
) -> Result<TriangleMesh, AppError> {
    placed_part(project_root, model, context).map(|part| part.mesh)
}

/// The placed mesh of a mesh model with the hash of the file as read now,
/// for triangle selections. Triangle `i` of the result is facet `i` of the
/// file: placement keeps the order. A STEP model is refused: it is selected
/// by face and edge, never by triangle (architecture section 5.5).
pub(crate) fn placed_mesh_hashed(
    project_root: &Path,
    model: &ModelRef,
    context: &JobContext<'_>,
) -> Result<(TriangleMesh, ContentHash), AppError> {
    let part = placed_part(project_root, model, context)?;
    if part.brep.is_some() {
        return Err(AppError::Selection {
            name: model.path.as_str().to_owned(),
            reason: "a STEP model is selected by face and edge, not by triangle".to_owned(),
        });
    }
    Ok((part.mesh, part.hash))
}

pub(crate) fn read(path: &Path) -> Result<Vec<u8>, AppError> {
    std::fs::read(path).map_err(|source| AppError::Io {
        path: path.to_path_buf(),
        source,
    })
}

/// `target` relative to the folder `base`, with `..` where needed. Both must
/// exist. `None` when they share no root (another drive on Windows).
pub(crate) fn relative_to(base: &Path, target: &Path) -> Result<Option<PathBuf>, AppError> {
    let canonical = |path: &Path| {
        path.canonicalize().map_err(|source| AppError::Io {
            path: path.to_path_buf(),
            source,
        })
    };
    let (base, target) = (canonical(base)?, canonical(target)?);
    let base: Vec<Component<'_>> = base.components().collect();
    let target: Vec<Component<'_>> = target.components().collect();
    let common = base.iter().zip(&target).take_while(|(a, b)| a == b).count();
    if common == 0 {
        return Ok(None);
    }
    let mut relative = PathBuf::new();
    for _ in common..base.len() {
        relative.push("..");
    }
    for part in target.iter().skip(common) {
        relative.push(part.as_os_str());
    }
    Ok(Some(relative))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use openchuck_geom::transform::Affine3;
    use openchuck_path::ExactFace;
    use openchuck_path::surface::{ParametricFace, Uv};
    use openchuck_testkit::brep::{HoleSpec, drilled_blank};
    use openchuck_types::geometry::Vec3;
    use openchuck_types::units::Degrees;

    use super::*;

    #[test]
    fn req_imp_05_a_placed_b_rep_is_evaluated_where_the_placement_puts_it() {
        let mut point = HoleSpec::blind(20.0, [0.0, 0.6, 0.8], 4.0, 5.0);
        point.point = true;
        let (brep, _) = drilled_blank(
            10.0,
            40.0,
            &[point, HoleSpec::through(30.0, [0.0, 1.0, 0.0], 3.0)],
        );
        let steps = [
            TransformStep::Rotate {
                axis: Axis::Z,
                angle: Degrees(30.0),
            },
            TransformStep::Mirror { axis: Axis::Y },
            TransformStep::Scale { factor: 2.5 },
            TransformStep::Translate {
                x: 3.0,
                y: -1.0,
                z: 7.0,
            },
        ];
        let map = brep_placement(&brep, &steps).unwrap();
        let placed = transform_brep(&brep, &map);
        let s = map.determinant().abs().cbrt();
        assert!(map.determinant() < 0.0 && (s - 2.5).abs() < 1e-12);
        let direction = |m: &Affine3, v: Vec3| m.apply_vec(v) * (1.0 / s);
        let mut checked = 0;
        for (before, after) in brep.faces.iter().zip(&placed.faces) {
            assert_eq!(before.id, after.id);
            let (a, b) = (
                ExactFace::new(before).unwrap(),
                ExactFace::new(after).unwrap(),
            );
            let ((a0, a1), (b0, b1)) = (a.uv_bounds(), b.uv_bounds());
            for (fu, fv) in [(0.5, 0.5), (0.2, 0.7), (0.9, 0.1)] {
                let at = |lo: Uv, hi: Uv| Uv {
                    u: lo.u + fu * (hi.u - lo.u),
                    v: lo.v + fv * (hi.v - lo.v),
                };
                let (Some(p), Some(q)) = (a.point_normal(at(a0, a1)), b.point_normal(at(b0, b1)))
                else {
                    continue;
                };
                let want = map.apply(p.point);
                assert!(
                    (q.point - want).length() < 1e-9,
                    "face {}: {:?} against {want:?}",
                    after.id.0,
                    q.point
                );
                // Still outward: the normal turns with the part, mirror and all.
                let n = direction(&map, p.normal.get());
                assert!(
                    (q.normal.get() - n).length() < 1e-9,
                    "face {}: normal {:?} against {n:?}",
                    after.id.0,
                    q.normal
                );
                checked += 1;
            }
        }
        assert!(checked > 10, "{checked}");
        for (before, after) in brep.edges.iter().zip(&placed.edges) {
            assert_eq!(before.id, after.id);
            assert!(
                before
                    .polyline
                    .iter()
                    .zip(&after.polyline)
                    .all(|(p, q)| (map.apply(*p) - *q).length() < 1e-9)
            );
        }
    }
}
