// SPDX-License-Identifier: AGPL-3.0-or-later
//! The OpenChuckCAM application layer (architecture section 5.7).
//!
//! One API for every front end: [`Command`]s change the project, [`Query`]s
//! return view data, and an event stream reports job progress and results.
//! The UI and the CLI use exactly this API and never call an engine crate
//! directly. Long work runs on the [`JobManager`]'s worker pool, never on the
//! caller's thread (UI-09, NFR-04).
//!
//! Work breakdown slice 0 builds the skeleton: the project document is empty
//! until slice 1 defines the project format, so regeneration runs the pipeline
//! over no operations and produces an empty toolpath.

#![warn(missing_docs)]

mod error;
mod jobs;
mod pipeline;

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};

pub use error::AppError;
pub use jobs::{JobHandle, JobId, JobManager, Threads};
pub use openchuck_types::notice::VERIFY_GCODE_NOTICE;
pub use pipeline::Regeneration;

// The application layer maps project data to every engine crate (architecture
// section 3). Most are still shells; naming them keeps the layering visible.
use openchuck_gcode as _;
use openchuck_geom as _;
use openchuck_import as _;
use openchuck_path as _;
use openchuck_post as _;
use openchuck_project as _;
use openchuck_sim as _;

/// A change to the project. Every command will be undoable (UI-08, slice 1).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Command {}

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

/// The in-memory project document. Empty until slice 1 defines its contents.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Document {
    location: PathBuf,
}

/// An open project and the services that work on it.
#[derive(Debug)]
pub struct Application {
    document: Document,
    jobs: JobManager,
    events_tx: Sender<Event>,
    events_rx: Receiver<Event>,
}

impl Application {
    /// Opens the project in `folder` with a worker pool of `threads`.
    ///
    /// Until slice 1 a project is a folder, and an existing folder opens as an
    /// empty project. A missing path or a path to a file is an error.
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
        Ok(Self {
            document: Document {
                location: folder.to_path_buf(),
            },
            jobs: JobManager::new(threads)?,
            events_tx,
            events_rx,
        })
    }

    /// Applies a command.
    pub fn execute(&mut self, command: Command) -> Result<(), AppError> {
        match command {}
    }

    /// Answers a query.
    pub fn query(&self, query: Query) -> QueryResult {
        match query {
            Query::Summary => QueryResult::Summary(ProjectSummary {
                location: self.document.location.clone(),
                setups: 0,
                operations: 0,
                models: 0,
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
