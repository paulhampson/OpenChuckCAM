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
use openchuck_geom::transform::{placement, transform_mesh, transform_normals};
use openchuck_import::{ImportOptions, import_bytes};
use openchuck_project::{ContentHash, ModelRef};
use openchuck_types::job::JobContext;
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
) -> Result<(ModelInspection, openchuck_types::mesh::TriangleMesh), AppError> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let model = import_bytes(&name, bytes, &ImportOptions { units }, context)?;
    let mesh = match &model.geometry {
        ModelGeometry::Mesh(mesh) => mesh,
        ModelGeometry::Brep(_) => return Err(ImportError::NotSupported("STEP").into()),
    };
    context.check_cancelled()?;
    context.report("check mesh", 0, None);
    let map = placement(mesh, transform)?;
    let placed = transform_mesh(mesh, &map);
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

/// Imports a project's model and places it in the model frame, for the
/// pathing library (work breakdown slice 5 item 10). Only meshes are read
/// so far: STEP waits for slice 11.
pub(crate) fn placed_mesh(
    project_root: &Path,
    model: &ModelRef,
    context: &JobContext<'_>,
) -> Result<openchuck_types::mesh::TriangleMesh, AppError> {
    let path = model.path.resolve(project_root);
    let bytes = read(&path)?;
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let imported = import_bytes(
        &name,
        &bytes,
        &ImportOptions { units: model.units },
        context,
    )?;
    let mesh = match &imported.geometry {
        ModelGeometry::Mesh(mesh) => mesh,
        ModelGeometry::Brep(_) => return Err(ImportError::NotSupported("STEP").into()),
    };
    let map = placement(mesh, &model.transform)?;
    Ok(transform_mesh(mesh, &map))
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
