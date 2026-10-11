// SPDX-License-Identifier: AGPL-3.0-or-later
//! The OpenChuckCAM application layer (architecture section 5.7).
//!
//! One API for every front end: [`Command`]s change the project, [`Query`]s
//! return view data, and an event stream reports job progress and results.
//! The UI and the CLI use exactly this API and never call an engine crate
//! directly. Long work runs on the [`JobManager`]'s worker pool, never on the
//! caller's thread (UI-09, NFR-04).
//!
//! Every project edit is a [`Command`] and can be undone and redone (UI-08).
//! The project file format waits for spike SP-8 (decision record 0005), so an
//! existing folder still opens as an empty project, and regeneration runs the
//! pipeline over no operations and produces an empty toolpath.

#![warn(missing_docs)]

mod command;
mod error;
mod jobs;
mod pipeline;

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};

use command::History;
use openchuck_project::Project;

pub use command::Command;
pub use error::AppError;
pub use jobs::{JobHandle, JobId, JobManager, Threads};
/// The project model that commands carry, re-exported so front ends need no
/// engine crate of their own (architecture section 5.8).
pub use openchuck_project as project;
pub use openchuck_types::notice::VERIFY_GCODE_NOTICE;
pub use pipeline::Regeneration;

// The application layer maps project data to every engine crate (architecture
// section 3). Most are still shells; naming them keeps the layering visible.
use openchuck_gcode as _;
use openchuck_geom as _;
use openchuck_import as _;
use openchuck_path as _;
use openchuck_post as _;
use openchuck_sim as _;

/// A request for view data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Query {
    /// Counts of the project's contents.
    Summary,
}

/// The answer to a [`Query`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum QueryResult {
    /// Answer to [`Query::Summary`].
    Summary(ProjectSummary),
}

/// Counts of a project's contents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectSummary {
    /// The project folder.
    pub location: PathBuf,
    /// Number of setups.
    pub setups: usize,
    /// Number of operations.
    pub operations: usize,
    /// Number of models.
    pub models: usize,
}

/// Something that happened, reported on the event stream.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Event {
    /// A job started.
    JobStarted {
        /// The job.
        job: JobId,
        /// What it does.
        name: String,
    },
    /// A job reported progress.
    JobProgress {
        /// The job.
        job: JobId,
        /// What it reported.
        progress: openchuck_types::job::Progress,
    },
    /// A job finished, failed or was cancelled.
    JobFinished {
        /// The job.
        job: JobId,
        /// `None` on success, or the error message.
        error: Option<String>,
    },
}

/// The open project and where it lives.
#[derive(Debug, Clone, PartialEq)]
struct Document {
    location: PathBuf,
    project: Project,
}

/// An open project and the services that work on it.
#[derive(Debug)]
pub struct Application {
    document: Document,
    history: History,
    jobs: JobManager,
    events_tx: Sender<Event>,
    events_rx: Receiver<Event>,
}

impl Application {
    /// Opens the project in `folder` with a worker pool of `threads`.
    ///
    /// Until the project file format exists (SP-8, decision record 0005) an
    /// existing folder opens as an empty project named after the folder. A
    /// missing path or a path to a file is an error.
    pub fn open(folder: &Path, threads: Threads) -> Result<Self, AppError> {
        let metadata = std::fs::metadata(folder).map_err(|source| {
            if source.kind() == std::io::ErrorKind::NotFound {
                AppError::ProjectNotFound(folder.to_path_buf())
            } else {
                AppError::Io {
                    path: folder.to_path_buf(),
                    source,
                }
            }
        })?;
        if !metadata.is_dir() {
            return Err(AppError::NotAProjectFolder(folder.to_path_buf()));
        }
        let (events_tx, events_rx) = channel();
        let name = folder
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        Ok(Self {
            document: Document {
                location: folder.to_path_buf(),
                project: Project::new(name),
            },
            history: History::default(),
            jobs: JobManager::new(threads)?,
            events_tx,
            events_rx,
        })
    }

    /// Applies a command and records it for undo. On error the project is
    /// unchanged and nothing is recorded.
    pub fn execute(&mut self, command: Command) -> Result<(), AppError> {
        self.history.execute(&mut self.document.project, command)
    }

    /// Undoes the latest command. Returns false when there is nothing to undo.
    pub fn undo(&mut self) -> Result<bool, AppError> {
        self.history.undo(&mut self.document.project)
    }

    /// Redoes the latest undone command. Returns false when there is nothing to redo.
    pub fn redo(&mut self) -> Result<bool, AppError> {
        self.history.redo(&mut self.document.project)
    }

    /// Whether there is a command to undo.
    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    /// Whether there is an undone command to redo.
    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    /// The project document, read-only. Change it with [`Application::execute`].
    pub fn project(&self) -> &Project {
        &self.document.project
    }

    /// Answers a query.
    pub fn query(&self, query: Query) -> QueryResult {
        match query {
            Query::Summary => QueryResult::Summary(ProjectSummary {
                location: self.document.location.clone(),
                setups: self.document.project.setups().len(),
                operations: self.document.project.operations().len(),
                models: self.document.project.models().len(),
            }),
        }
    }

    /// Events reported since the last call, oldest first.
    pub fn drain_events(&self) -> Vec<Event> {
        self.events_rx.try_iter().collect()
    }

    /// Starts regenerating every operation's toolpath and G-code as a background job.
    pub fn start_regenerate(&self) -> JobHandle<Regeneration> {
        let document = self.document.clone();
        self.jobs
            .submit("regenerate", self.events_tx.clone(), move |context| {
                pipeline::regenerate(&document.location, context)
            })
    }

    /// Regenerates and waits for the result.
    pub fn regenerate(&self) -> Result<Regeneration, AppError> {
        self.start_regenerate().wait()
    }
}
