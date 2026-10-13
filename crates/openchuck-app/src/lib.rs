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
//! Projects are folders of TOML files (decision records 0006 and 0008).
//! Models are added, inspected (mesh checks, IMP-04), given units (IMP-03)
//! and placed (IMP-05) through [`model`] (decision record 0010). The project's
//! machine profile and its setups' stock, positions and clearance are checked
//! through [`setup`] (decision record 0011). Tool snapshots, the tool
//! library and their checks are in [`tools`] (decision record 0014).
//! Operations are mapped to the pathing library's plain data and generated
//! in [`operation`] (decision record 0018); regeneration runs every operation
//! in machining order through the strategies in the application's registry.
//! The library's own strategy, helical finishing, is registered from the start;
//! a host or test can add more with [`Application::register_strategy`].

#![warn(missing_docs)]

mod command;
mod error;
mod jobs;
pub mod model;
pub mod operation;
mod pipeline;
pub mod setup;
pub mod tools;

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};

use command::History;
use model::{LengthUnit, ModelInspection};
use openchuck_project::id::ModelId;
use openchuck_project::{
    ContentHash, Item, ModelCheck, ModelRef, Placement, Project, RelativePath,
};
use setup::{PartExtent, SetupReport};

pub use command::Command;
pub use error::AppError;
pub use jobs::{JobHandle, JobId, JobManager, Threads};
pub use openchuck_path::{Registry, Strategy};
/// The project model that commands carry, re-exported so front ends need no
/// engine crate of their own (architecture section 5.8).
pub use openchuck_project as project;
pub use openchuck_types::notice::VERIFY_GCODE_NOTICE;
pub use pipeline::Regeneration;

// The application layer maps project data to every engine crate (architecture
// section 3). Most are still shells; naming them keeps the layering visible.
use openchuck_gcode as _;
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
    strategies: Registry,
    events_tx: Sender<Event>,
    events_rx: Receiver<Event>,
}

impl Application {
    /// Opens the project in `folder` with a worker pool of `threads`.
    ///
    /// A missing path, a path to a file and a damaged project are errors with
    /// a message for the user (NFR-07). A folder with no `project.toml` gives
    /// [`AppError::NoProject`], which a front end shows as a warning and
    /// answers by offering [`Application::create`] (decision record 0008
    /// item 8).
    pub fn open(folder: &Path, threads: Threads) -> Result<Self, AppError> {
        check_folder(folder)?;
        let project = match openchuck_project::file::load(folder) {
            Err(openchuck_project::ProjectError::NotAProject(path)) => {
                return Err(AppError::NoProject(path));
            }
            other => other?,
        };
        Self::with_project(folder, project, threads)
    }

    /// Creates a new, empty project named `name` in `folder` (created if
    /// needed) and opens it. Fails if the folder already holds a project.
    pub fn create(folder: &Path, name: &str, threads: Threads) -> Result<Self, AppError> {
        let project = openchuck_project::file::create(folder, name)?;
        Self::with_project(folder, project, threads)
    }

    fn with_project(folder: &Path, project: Project, threads: Threads) -> Result<Self, AppError> {
        let (events_tx, events_rx) = channel();
        Ok(Self {
            document: Document {
                location: folder.to_path_buf(),
                project,
            },
            history: History::default(),
            jobs: JobManager::new(threads)?,
            strategies: Registry::builtin(),
            events_tx,
            events_rx,
        })
    }

    /// Saves the project to its folder. Only changed files are written
    /// (PRJ-01).
    pub fn save(&self) -> Result<(), AppError> {
        Ok(openchuck_project::file::save(
            &self.document.project,
            &self.document.location,
        )?)
    }

    /// Checks every model file against its recorded content hash (PRJ-02).
    pub fn check_models(&self) -> Result<Vec<(ModelId, ModelCheck)>, AppError> {
        self.document
            .project
            .models()
            .iter()
            .map(|m| Ok((m.id, m.check(&self.document.location)?)))
            .collect()
    }

    /// Adds the model file at `file` to the project (IMP-01), read in
    /// `units`, or in millimetres when `None` (IMP-03). If the returned
    /// inspection has a [`unit_question`](ModelInspection::unit_question),
    /// the front end asks it and stores the answer with
    /// [`Command::SetModelUnits`]. The file is imported and
    /// checked first, so a file that cannot be read is refused; mesh
    /// problems are reported in the returned inspection, not refused. The
    /// file is referenced by its path relative to the project folder with its
    /// content hash (PRJ-02). Undoable like any command.
    pub fn add_model(
        &mut self,
        file: &Path,
        units: Option<LengthUnit>,
    ) -> Result<(ModelId, ModelInspection), AppError> {
        let relative = model::relative_to(&self.document.location, file)?.ok_or_else(|| {
            AppError::ModelPath {
                path: file.to_path_buf(),
                reason: "it is on another drive".to_owned(),
            }
        })?;
        let relative = RelativePath::from_path(&relative)?;
        let bytes = model::read(file)?;
        let hash = ContentHash::of_bytes(&bytes);
        let path = file.to_path_buf();
        let inspection = self
            .jobs
            .submit("import model", self.events_tx.clone(), move |context| {
                model::inspect_bytes(&path, &bytes, units, &[], context)
            })
            .wait()?;
        let id = ModelId::new();
        let mut reference = ModelRef::new(id, relative, hash);
        reference.units = units;
        self.execute(Command::Add {
            item: Item::Model(reference),
            placement: Placement::End,
        })?;
        Ok((id, inspection))
    }

    /// The model whose ID or path (as the project lists it) is `text`.
    pub fn find_model(&self, text: &str) -> Result<&ModelRef, AppError> {
        self.document
            .project
            .models()
            .iter()
            .find(|m| m.id.to_string() == text || m.path.as_str() == text)
            .ok_or_else(|| AppError::UnknownModel(text.to_owned()))
    }

    /// Starts importing, placing and checking a model of the project as a
    /// background job (IMP-04).
    pub fn start_inspect_model(&self, id: ModelId) -> Result<JobHandle<ModelInspection>, AppError> {
        let reference = self
            .document
            .project
            .models()
            .iter()
            .find(|m| m.id == id)
            .cloned()
            .ok_or_else(|| AppError::UnknownModel(id.to_string()))?;
        let root = self.document.location.clone();
        Ok(self
            .jobs
            .submit("inspect model", self.events_tx.clone(), move |context| {
                model::inspect_model(&root, &reference, context)
            }))
    }

    /// Inspects a model of the project and waits for the result.
    pub fn inspect_model(&self, id: ModelId) -> Result<ModelInspection, AppError> {
        self.start_inspect_model(id)?.wait()
    }

    /// Checks every setup against its stock, part and the project's machine
    /// (SET-05, SET-06). Each setup's part is imported and placed to measure
    /// it; a part that cannot be read is noted, not an error. Problems are
    /// warnings: nothing is refused.
    pub fn check_setups(&self) -> Result<Vec<SetupReport>, AppError> {
        let project = &self.document.project;
        let mut parts: Vec<(ModelId, Result<Option<PartExtent>, String>)> = Vec::new();
        let mut reports = Vec::new();
        for s in project.setups() {
            let mut notes = Vec::new();
            let part = match s.part {
                None => None,
                Some(id) => {
                    if !parts.iter().any(|(m, _)| *m == id) {
                        let extent = self
                            .inspect_model(id)
                            .map(|i| i.extent)
                            .map_err(|e| e.to_string());
                        parts.push((id, extent));
                    }
                    match parts.iter().find(|(m, _)| *m == id).map(|(_, e)| e) {
                        Some(Ok(extent)) => *extent,
                        Some(Err(e)) => {
                            notes.push(format!(
                                "the part cannot be read, so it is not checked: {e}"
                            ));
                            None
                        }
                        None => None,
                    }
                }
            };
            let (warnings, expected) = match project.machine() {
                Some(profile) => (
                    openchuck_types::setup::check_setup(&openchuck_types::setup::SetupCheck {
                        profile,
                        frame: &s.frame,
                        stock: s.stock.as_ref(),
                        clearance: s.clearance.as_ref(),
                        part: part.as_ref(),
                    }),
                    Some(setup::expected_work_offset(profile, &s.frame)),
                ),
                None => {
                    notes.push(
                        "the project has no machine profile, so the setup is not checked"
                            .to_owned(),
                    );
                    (Vec::new(), None)
                }
            };
            reports.push(SetupReport {
                setup: s.id,
                name: s.name.clone(),
                warnings,
                expected_work_offset: expected,
                part,
                notes,
            });
        }
        Ok(reports)
    }

    /// The setup whose ID or name is `text`.
    pub fn find_setup(&self, text: &str) -> Result<&openchuck_project::Setup, AppError> {
        self.document
            .project
            .setups()
            .iter()
            .find(|s| s.id.to_string() == text || s.name == text)
            .ok_or_else(|| AppError::UnknownSetup(text.to_owned()))
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
        let strategies = self.strategies.clone();
        self.jobs
            .submit("regenerate", self.events_tx.clone(), move |context| {
                pipeline::regenerate(&document.location, &document.project, &strategies, context)
            })
    }

    /// The strategies operations can use (OP-16).
    pub fn strategies(&self) -> &Registry {
        &self.strategies
    }

    /// Adds a strategy to the application's registry, for a host that brings
    /// its own (OP-16). Fails if one of that name is registered.
    pub fn register_strategy(
        &mut self,
        strategy: std::sync::Arc<dyn Strategy>,
    ) -> Result<(), AppError> {
        Ok(self.strategies.register(strategy)?)
    }

    /// Regenerates and waits for the result.
    pub fn regenerate(&self) -> Result<Regeneration, AppError> {
        self.start_regenerate().wait()
    }
}

/// Imports and checks the model file at `path` outside any project, on a
/// worker pool of `threads` (IMP-01, IMP-03, IMP-04). `units` overrides the
/// file's unit.
pub fn inspect_file(
    path: &Path,
    units: Option<LengthUnit>,
    threads: Threads,
) -> Result<ModelInspection, AppError> {
    let jobs = JobManager::new(threads)?;
    let (events, _ignored) = channel();
    let path = path.to_path_buf();
    jobs.submit("inspect model", events, move |context| {
        model::inspect_file(&path, units, &[], context)
    })
    .wait()
}

/// Fails unless `folder` is an existing folder.
fn check_folder(folder: &Path) -> Result<(), AppError> {
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
    if metadata.is_dir() {
        Ok(())
    } else {
        Err(AppError::NotAProjectFolder(folder.to_path_buf()))
    }
}
