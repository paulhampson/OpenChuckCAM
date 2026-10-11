// SPDX-License-Identifier: AGPL-3.0-or-later
//! Model and curve readers behind a `ModelImporter` trait: STL, STEP (the C++ kernel is wrapped here and nowhere else).
//!
//! Architecture section 5.4; requirements IMP-01 to IMP-03, IMP-06, IMP-08, NFR-13.
//!
//! Every importer turns a file's bytes into [`ImportedModel`]: geometry in
//! millimetres with stable numbering, the units used, and notes for the user.
//! This is the unit boundary (IMP-03): a file's unit is read where the file
//! states one, a user override wins over it, millimetres are assumed
//! otherwise, and nothing past this crate sees any other unit.
//!
//! Damaged input gives an [`ImportError`] that says what is wrong, never a
//! panic (NFR-07). STL is read now (slice 2); STEP arrives with the
//! OpenCASCADE wrapper in slice 11 (decision record 0003).
//!
//! ```
//! use openchuck_import::{ImportOptions, import_bytes};
//! use openchuck_types::job::{CancelToken, JobContext, NoProgress};
//! use openchuck_types::model::ModelGeometry;
//! use openchuck_types::units::LengthUnit;
//!
//! let text = "solid tri units=in
//!   facet normal 0 0 1
//!     outer loop
//!       vertex 0 0 0
//!       vertex 1 0 0
//!       vertex 0 1 0
//!     endloop
//!   endfacet
//! endsolid tri
//! ";
//! let (cancel, progress) = (CancelToken::new(), NoProgress);
//! let context = JobContext::new(&cancel, &progress);
//! let model = import_bytes("tri.stl", text.as_bytes(), &ImportOptions::default(), &context)?;
//! assert_eq!(model.units.applied, LengthUnit::Inch);
//! let ModelGeometry::Mesh(mesh) = &model.geometry else { unreachable!() };
//! assert_eq!(mesh.vertices[1].x, 25.4);
//! # Ok::<(), openchuck_import::ImportError>(())
//! ```

#![warn(missing_docs)]

pub mod stl;

use std::path::Path;

use openchuck_types::geometry::Vec3;
use openchuck_types::job::JobContext;
use openchuck_types::model::{ModelFormat, ModelGeometry};
use openchuck_types::units::LengthUnit;

// Mesh work that the STEP adapter will share (tessellation checks) lives in
// openchuck-geom (architecture section 3).
use openchuck_geom as _;

/// What every STL import tells the user (work breakdown slice 2 item 7;
/// architecture section 5.4; verification plan section 5).
pub const STL_TESSELLATION_NOTICE: &str = "STL holds triangles only: the tessellation was fixed when \
the file was exported, and gouge tolerance on this part is measured against this mesh, not the \
original surface.";

/// Choices the user makes for an import.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ImportOptions {
    /// The unit the file is in, overriding any unit the file states (IMP-03).
    pub units: Option<LengthUnit>,
}

/// Where the unit used for an import came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitSource {
    /// The file states it.
    File,
    /// The user chose it.
    Override,
    /// Neither: millimetres were assumed.
    Assumed,
}

/// The units of an import (IMP-03).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Units {
    /// The unit the file states, if any.
    pub file: Option<LengthUnit>,
    /// The unit the coordinates were read in and converted from.
    pub applied: LengthUnit,
    /// Where `applied` came from.
    pub source: UnitSource,
}

impl Units {
    /// Picks the unit: the override, else the file's, else millimetres.
    pub fn choose(file: Option<LengthUnit>, user: Option<LengthUnit>) -> Self {
        let (applied, source) = match (user, file) {
            (Some(unit), _) => (unit, UnitSource::Override),
            (None, Some(unit)) => (unit, UnitSource::File),
            (None, None) => (LengthUnit::Millimetre, UnitSource::Assumed),
        };
        Self {
            file,
            applied,
            source,
        }
    }

    /// Which unit was used and why, for the user: `in, as the file states`.
    pub fn describe(&self) -> String {
        let applied = self.applied;
        match (self.source, self.file) {
            (UnitSource::File, _) => format!("{applied}, as the file states"),
            (UnitSource::Override, Some(file)) if file != applied => {
                format!("{applied}, set by the user (the file states {file})")
            }
            (UnitSource::Override, _) => format!("{applied}, set by the user"),
            (UnitSource::Assumed, _) => format!(
                "{applied}, assumed because the file states none; set the model's units if this is wrong"
            ),
        }
    }
}

/// A model read from a file, in millimetres.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportedModel {
    /// The file format.
    pub format: ModelFormat,
    /// The geometry.
    pub geometry: ModelGeometry,
    /// The units used.
    pub units: Units,
    /// For a mesh, the normal each triangle states in the file (zero where
    /// none is stated), for the normal check in `openchuck-geom`.
    pub stated_normals: Option<Vec<Vec3>>,
    /// Notes for the user, such as [`STL_TESSELLATION_NOTICE`].
    pub notes: Vec<String>,
}

/// An import failure, worded for the user (NFR-07).
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ImportError {
    /// The file name's extension names no format we read.
    #[error(
        "'{0}' is not a model format OpenChuckCAM reads: use an .stl file (STEP arrives in a later release)"
    )]
    UnknownFormat(String),
    /// The format is known, but this build cannot read it yet.
    #[error("{0} import is not available yet")]
    NotSupported(&'static str),
    /// The file could not be read.
    #[error("cannot read '{}': {source}", path.display())]
    Io {
        /// The file.
        path: std::path::PathBuf,
        /// The underlying error.
        source: std::io::Error,
    },
    /// The file's contents are not a valid model.
    #[error("not a valid {format} file: {reason}")]
    Invalid {
        /// The format expected.
        format: &'static str,
        /// What is wrong.
        reason: String,
    },
    /// The import was cancelled.
    #[error("the import was cancelled")]
    Cancelled,
}

/// Reads one model format.
pub trait ModelImporter: Send + Sync {
    /// The format this importer reads.
    fn format(&self) -> ModelFormat;

    /// Reads a whole file's bytes. Checks `context` for cancellation and
    /// reports progress on long files.
    fn import(
        &self,
        bytes: &[u8],
        options: &ImportOptions,
        context: &JobContext<'_>,
    ) -> Result<ImportedModel, ImportError>;
}

/// The importer for `format`, if this build has one.
pub fn importer_for(format: ModelFormat) -> Option<&'static dyn ModelImporter> {
    match format {
        ModelFormat::Stl => Some(&stl::StlImporter),
        _ => None,
    }
}

/// Imports `bytes`, choosing the importer by `file_name`'s extension.
pub fn import_bytes(
    file_name: &str,
    bytes: &[u8],
    options: &ImportOptions,
    context: &JobContext<'_>,
) -> Result<ImportedModel, ImportError> {
    let format = ModelFormat::from_file_name(file_name)
        .ok_or_else(|| ImportError::UnknownFormat(file_name.to_owned()))?;
    let importer = importer_for(format).ok_or(ImportError::NotSupported(format.name()))?;
    importer.import(bytes, options, context)
}

/// Reads and imports the file at `path`.
pub fn import_file(
    path: &Path,
    options: &ImportOptions,
    context: &JobContext<'_>,
) -> Result<ImportedModel, ImportError> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    // Check the format before reading a possibly large file.
    let format = ModelFormat::from_file_name(&name)
        .ok_or_else(|| ImportError::UnknownFormat(name.clone()))?;
    importer_for(format).ok_or(ImportError::NotSupported(format.name()))?;
    let bytes = std::fs::read(path).map_err(|source| ImportError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    import_bytes(&name, &bytes, options, context)
}
