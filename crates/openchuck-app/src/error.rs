// SPDX-License-Identifier: AGPL-3.0-or-later
use std::path::PathBuf;

/// An error from the application layer, worded for the user (NFR-07).
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum AppError {
    /// The project folder does not exist.
    #[error("project folder '{}' does not exist", .0.display())]
    ProjectNotFound(PathBuf),
    /// The path exists but is not a folder.
    #[error("'{}' is not a project folder", .0.display())]
    NotAProjectFolder(PathBuf),
    /// The folder exists but holds no project (no `project.toml`). Front ends
    /// offer to create one with [`Application::create`](crate::Application::create).
    #[error("'{}' has no project.toml, so it is not a project yet", .0.display())]
    NoProject(PathBuf),
    /// A file could not be read or written.
    #[error("cannot access '{}': {source}", path.display())]
    Io {
        /// The path involved.
        path: PathBuf,
        /// The underlying error.
        source: std::io::Error,
    },
    /// The worker pool could not be started.
    #[error("cannot start the worker pool: {0}")]
    WorkerPool(String),
    /// A project edit or check failed.
    #[error(transparent)]
    Project(#[from] openchuck_project::ProjectError),
    /// A model file could not be imported.
    #[error(transparent)]
    Import(#[from] openchuck_import::ImportError),
    /// A model file cannot be referenced from the project folder.
    #[error("'{}' cannot be referenced from the project folder: {reason}", path.display())]
    ModelPath {
        /// The model file.
        path: PathBuf,
        /// Why not.
        reason: String,
    },
    /// No model has this ID or path.
    #[error("the project has no model '{0}' (give its ID or its path as the project lists it)")]
    UnknownModel(String),
    /// No setup has this ID or name.
    #[error("the project has no setup '{0}' (give its ID or its name)")]
    UnknownSetup(String),
    /// No operation has this ID.
    #[error("the project has no operation '{0}'")]
    UnknownOperation(String),
    /// No selection set has this ID.
    #[error("the project has no selection set '{0}'")]
    UnknownSelection(String),
    /// A selection cannot be made or changed as asked (UI-04, UI-05).
    #[error("selection '{name}': {reason}")]
    Selection {
        /// The set's name.
        name: String,
        /// Why not.
        reason: String,
    },
    /// An operation cannot be generated as it stands.
    #[error("operation '{name}' cannot be generated: {reason}")]
    Operation {
        /// The operation's name.
        name: String,
        /// Why not.
        reason: String,
    },
    /// The tool numbers cannot be used as asked (GC-14).
    #[error("tool numbers: {0}")]
    ToolNumbers(String),
    /// The toolpath could not be posted to G-code.
    #[error("cannot post the G-code: {0}")]
    Post(String),
    /// The pathing library reported an error.
    #[error(transparent)]
    Path(#[from] openchuck_path::PathError),
    /// An engine library reported an error.
    #[error(transparent)]
    Engine(#[from] openchuck_types::Error),
    /// The pipeline cache could not key an operation's inputs.
    #[error("pipeline cache: {0}")]
    Cache(String),
    /// The job hit an internal error and stopped. This is a defect; please report it.
    #[error("the job '{0}' stopped on an internal error; please report this as a defect")]
    JobPanicked(String),
    /// The job stopped without a result.
    #[error("the job '{0}' stopped without a result")]
    JobLost(String),
}
