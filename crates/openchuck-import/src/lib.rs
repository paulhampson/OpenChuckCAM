// SPDX-License-Identifier: AGPL-3.0-or-later
//! Model and curve readers behind a `ModelImporter` trait: STL, STEP (the C++ kernel is wrapped here and nowhere else).
//!
//! Architecture section 5.4; requirements IMP-01 to IMP-03, IMP-06, IMP-08, NFR-13.
//!
//! Every importer turns a file's bytes into [`ImportedModel`]: geometry in
//! millimetres with stable numbering, the units used, and notes for the user.
//! This is the unit boundary (IMP-03): coordinates are read in the user's
//! unit for the model, or in millimetres by default, and nothing past this
//! crate sees any other unit. A unit the file names, or a part whose size
//! is implausible in millimetres, raises a [`UnitQuestion`] for the user to
//! answer; nothing is converted without that answer (decision record 0010).
//!
//! Damaged input gives an [`ImportError`] that says what is wrong, never a
//! panic (NFR-07). STL is read in process (slice 2). STEP is read by the
//! OpenCASCADE wrapper in a child process (slice 11, decision record 0003):
//! see [`step`].
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
//! // Read in millimetres; the header's unit is a question, not a conversion.
//! let model = import_bytes("tri.stl", text.as_bytes(), &ImportOptions::default(), &context)?;
//! assert_eq!(model.units.applied, LengthUnit::Millimetre);
//! assert_eq!(model.unit_question.unwrap().suggestions, [LengthUnit::Inch]);
//! // The user confirms inches.
//! let options = ImportOptions { units: Some(LengthUnit::Inch), ..ImportOptions::default() };
//! let model = import_bytes("tri.stl", text.as_bytes(), &options, &context)?;
//! let ModelGeometry::Mesh(mesh) = &model.geometry else { unreachable!() };
//! assert_eq!(mesh.vertices[1].x, 25.4);
//! # Ok::<(), openchuck_import::ImportError>(())
//! ```

#![warn(missing_docs)]

pub mod step;
pub mod stl;

#[cfg(feature = "kernel")]
pub mod kernel;

use std::path::Path;

use openchuck_types::brep::TessellationSettings;
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
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ImportOptions {
    /// The unit the file is in (IMP-03). `None` reads an STL in millimetres
    /// and raises a [`UnitQuestion`] when the file suggests otherwise; a STEP
    /// file states its own unit, which `None` uses.
    pub units: Option<LengthUnit>,
    /// How finely a STEP model is tessellated (IMP-06); `None` uses
    /// [`TessellationSettings::default`]. Ignored for STL, whose triangles
    /// are fixed by the file.
    pub tessellation: Option<TessellationSettings>,
}

/// Where the unit used for an import came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitSource {
    /// The user chose it.
    User,
    /// Nobody chose: millimetres, the default.
    Default,
    /// The file states its own unit (STEP, IMP-03) and nobody overrode it.
    File,
}

/// Smallest bounding-box size, in millimetres, that a part read in
/// millimetres is taken to be at: below it the file may be in inches,
/// centimetres or metres.
pub const SMALL_PART_MM: f64 = 2.0;

/// Largest bounding-box size, in millimetres, that a part read in
/// millimetres is taken to be at: above it the file may be in micrometres.
pub const LARGE_PART_MM: f64 = 2000.0;

/// The units of an import (IMP-03). Coordinates are converted from
/// `applied` to millimetres; nothing past the importer sees another unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Units {
    /// A unit the file names (an STL header's `units=in`), if any. It is a
    /// hint only: it is never applied without the user's confirmation.
    pub file: Option<LengthUnit>,
    /// The unit the coordinates were read in and converted from.
    pub applied: LengthUnit,
    /// Where `applied` came from.
    pub source: UnitSource,
}

impl Units {
    /// The user's unit if they chose one, else millimetres.
    pub fn choose(file: Option<LengthUnit>, user: Option<LengthUnit>) -> Self {
        Self {
            file,
            applied: user.unwrap_or(LengthUnit::Millimetre),
            source: if user.is_some() {
                UnitSource::User
            } else {
                UnitSource::Default
            },
        }
    }

    /// Which unit was used and why, for the user: `in, set by the user`.
    pub fn describe(&self) -> String {
        let applied = self.applied;
        match (self.source, self.file) {
            (UnitSource::User, Some(file)) if file != applied => {
                format!("{applied}, set by the user (the file names {file})")
            }
            (UnitSource::User, _) => format!("{applied}, set by the user"),
            (UnitSource::Default, _) => format!("{applied}, the default"),
            (UnitSource::File, _) => format!("{applied}, from the file"),
        }
    }
}

/// Why the unit of an import needs the user's confirmation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UnitConcern {
    /// The file names a unit other than millimetres.
    FileNames(LengthUnit),
    /// Read in millimetres, the part's largest bounding-box size is below
    /// [`SMALL_PART_MM`].
    TooSmall {
        /// The largest size, in millimetres.
        size: f64,
    },
    /// Read in millimetres, the part's largest bounding-box size is above
    /// [`LARGE_PART_MM`].
    TooLarge {
        /// The largest size, in millimetres.
        size: f64,
    },
}

/// A question for the user about the unit of a file read in millimetres by
/// default (IMP-03). A front end shows it and stores the answer as the
/// model's unit; nothing is converted until the user answers. Raised only
/// when the user has not chosen a unit.
#[derive(Debug, Clone, PartialEq)]
pub struct UnitQuestion {
    /// Why it is asked.
    pub concern: UnitConcern,
    /// Likely units, most likely first. Millimetres stays a valid answer.
    pub suggestions: Vec<LengthUnit>,
}

impl UnitQuestion {
    /// The question to ask about a file read in millimetres whose part is
    /// `size` millimetres across at its largest and which names `file`, or
    /// `None` when nothing is in doubt. A unit the file names comes first.
    pub fn ask(file: Option<LengthUnit>, size: f64) -> Option<Self> {
        if let Some(unit) = file.filter(|u| *u != LengthUnit::Millimetre) {
            return Some(Self {
                concern: UnitConcern::FileNames(unit),
                suggestions: vec![unit],
            });
        }
        if size < SMALL_PART_MM {
            // Below a fifth of a millimetre, metres are likelier than inches
            // (OrcaSlicer draws the same line, by volume).
            let suggestions = if size < SMALL_PART_MM / 10.0 {
                vec![LengthUnit::Metre, LengthUnit::Inch, LengthUnit::Centimetre]
            } else {
                vec![LengthUnit::Inch, LengthUnit::Centimetre, LengthUnit::Metre]
            };
            Some(Self {
                concern: UnitConcern::TooSmall { size },
                suggestions,
            })
        } else if size > LARGE_PART_MM {
            Some(Self {
                concern: UnitConcern::TooLarge { size },
                suggestions: vec![LengthUnit::Micrometre],
            })
        } else {
            None
        }
    }

    /// The question in words, for the user.
    pub fn message(&self) -> String {
        let choices: Vec<&str> = self.suggestions.iter().map(|u| u.short_name()).collect();
        let choices = choices.join(" or ");
        match self.concern {
            UnitConcern::FileNames(unit) => format!(
                "the file says its unit is {unit}, but it was read in mm; convert it from {unit}?"
            ),
            UnitConcern::TooSmall { size } => format!(
                "read in mm the part is only {size:.3} mm across, so it may be in {choices}; choose its unit"
            ),
            UnitConcern::TooLarge { size } => format!(
                "read in mm the part is {size:.0} mm across, so it may be in {choices}; choose its unit"
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
    /// A question for the user about the unit, when the file suggests the
    /// default was wrong.
    pub unit_question: Option<UnitQuestion>,
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
    #[error("'{0}' is not a model format OpenChuckCAM reads: use an .stl, .step or .stp file")]
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
    /// The STEP reader process failed, timed out, was not found or crashed on
    /// the file. A crash means the file is damaged or uses something the
    /// kernel cannot read; OpenChuckCAM itself is unaffected (NFR-07).
    #[error("STEP import failed: {0}")]
    Worker(String),
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
        ModelFormat::Step => Some(&step::STEP_IMPORTER),
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
