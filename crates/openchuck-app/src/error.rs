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
    /// The project has its one setup; more are SET-07 (P2).
    #[error("more than one setup per project is not supported yet (SET-07)")]
    OneSetupOnly,
    /// A setup needs a part, so it cannot be added before a model is imported.
    #[error("import a model first: a setup needs a part")]
    NoModelForSetup,
    /// No operation has this ID.
    #[error("the project has no operation '{0}'")]
    UnknownOperation(String),
    /// No selection set has this ID.
    #[error("the project has no selection set '{0}'")]
    UnknownSelection(String),
    /// Playback cannot be set up as asked (SIM-02, SIM-04).
    #[error("playback: {0}")]
    Playback(String),
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

impl AppError {
    /// Whether the job was cancelled on request (UI-09), as opposed to
    /// failing. Decided by the kind of error, never by its words: an
    /// operation's name appears in messages and is the user's to choose.
    pub fn is_cancelled(&self) -> bool {
        matches!(
            self,
            Self::Engine(openchuck_types::Error::Cancelled)
                | Self::Path(openchuck_path::PathError::Cancelled)
                | Self::Import(openchuck_import::ImportError::Cancelled)
        )
    }

    /// The error in plain words for a panel (specification section 15): a
    /// refused parameter value says what is wrong with it, without the
    /// parameter's name in the strategy's schema, which the panel shows
    /// beside the field already.
    pub fn user_message(&self) -> String {
        match self {
            Self::Path(openchuck_path::PathError::Parameter { reason, .. }) => {
                let mut chars = reason.chars();
                let mut out: String = chars
                    .next()
                    .map(|c| c.to_uppercase().chain(chars).collect())
                    .unwrap_or_default();
                if !out.ends_with(['.', '!', '?']) {
                    out.push('.');
                }
                out
            }
            other => other.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Specification section 15: no schema field names in what a panel reads.
    #[test]
    fn req_nfr_15_a_refused_parameter_reads_as_a_sentence() {
        let error = AppError::from(openchuck_path::PathError::parameter(
            "stepover_mm",
            "must be above 0 mm",
        ));
        assert_eq!(error.user_message(), "Must be above 0 mm.");
        let cancelled = AppError::Path(openchuck_path::PathError::Cancelled);
        assert_eq!(cancelled.user_message(), cancelled.to_string());
    }
}
