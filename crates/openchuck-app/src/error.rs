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
    /// An engine library reported an error.
    #[error(transparent)]
    Engine(#[from] openchuck_types::Error),
    /// The job hit an internal error and stopped. This is a defect; please report it.
    #[error("the job '{0}' stopped on an internal error; please report this as a defect")]
    JobPanicked(String),
    /// The job stopped without a result.
    #[error("the job '{0}' stopped without a result")]
    JobLost(String),
}
