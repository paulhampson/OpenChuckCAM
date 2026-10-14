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
//! What the viewport draws for a setup comes from [`scene`] as plain data
//! (decision record 0026).

#![warn(missing_docs)]

mod command;
mod error;
pub mod form;
mod jobs;
pub mod library;
pub mod model;
pub mod operation;
mod pipeline;
mod posting;
pub mod scene;
pub mod setup;
pub mod simulation;
pub mod tool_edit;
pub mod tools;

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};

use command::History;
use model::{LengthUnit, ModelInspection};
use openchuck_project::id::{ModelId, OperationId};
use openchuck_project::{
    ContentHash, Item, ModelCheck, ModelRef, Placement, Project, RelativePath,
};
use setup::{PartExtent, SetupReport};

pub use command::{Command, duplicate_operation};
pub use error::AppError;
pub use jobs::{JobHandle, JobId, JobManager, Threads};
/// The G-code reader and replay, re-exported so front ends need no engine
/// crate of their own (architecture section 5.8).
pub use openchuck_gcode as gcode;
pub use openchuck_path::{Registry, Strategy};
/// The project model that commands carry, re-exported so front ends need no
/// engine crate of their own (architecture section 5.8).
pub use openchuck_project as project;
pub use openchuck_sim as sim;
pub use openchuck_types::notice::VERIFY_GCODE_NOTICE;
pub use pipeline::Regeneration;
pub use posting::{GcodeComparison, GcodeReport, PostSelection};
pub use simulation::{OperationSimulation, SetupSimulation, SimulationReport};

// The application layer maps project data to every engine crate (architecture
// section 3). Most are still shells; naming them keeps the layering visible.
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
    /// The project as last opened, created or saved, for
    /// [`Application::is_modified`]. Behind a lock so `save` can stay `&self`.
    saved: std::sync::Mutex<Project>,
    jobs: JobManager,
    strategies: Registry,
    post: Option<PostSelection>,
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
            saved: std::sync::Mutex::new(project.clone()),
            document: Document {
                location: folder.to_path_buf(),
                project,
            },
            history: History::default(),
            jobs: JobManager::new(threads)?,
            strategies: Registry::builtin(),
            post: None,
            events_tx,
            events_rx,
        })
    }

    /// Saves the project to its folder. Only changed files are written
    /// (PRJ-01).
    pub fn save(&self) -> Result<(), AppError> {
        openchuck_project::file::save(&self.document.project, &self.document.location)?;
        if let Ok(mut saved) = self.saved.lock() {
            saved.clone_from(&self.document.project);
        }
        Ok(())
    }

    /// Whether the project differs from what was last opened, created or
    /// saved, so a front end can ask before discarding changes. Undoing back
    /// to the saved state makes it unmodified again.
    pub fn is_modified(&self) -> bool {
        self.saved
            .lock()
            .map_or(true, |saved| *saved != self.document.project)
    }

    /// One sentence per model file that has changed or is missing since it
    /// was added, naming the file, for a front end to show when a project
    /// opens (PRJ-02, OI-45). Empty when every file matches.
    pub fn model_file_warnings(&self) -> Vec<String> {
        self.document
            .project
            .models()
            .iter()
            .filter_map(|m| {
                let path = m.path.as_str();
                match m.check(&self.document.location) {
                    Ok(ModelCheck::Unchanged) => None,
                    Ok(ModelCheck::Missing) => Some(format!("'{path}' is missing")),
                    Ok(_) => Some(format!("'{path}' has changed since it was added")),
                    Err(e) => Some(format!("'{path}' cannot be checked: {e}")),
                }
            })
            .collect()
    }

    /// The project folder.
    pub fn location(&self) -> &Path {
        &self.document.location
    }

    /// Starts building the scene the viewport draws for `setup` (or the first
    /// setup, or every model when there is none) as a background job: the
    /// models are read and placed, which takes time on large meshes (UI-09,
    /// NFR-04; decision record 0002, "Findings for later slices").
    pub fn start_scene(
        &self,
        setup: Option<openchuck_project::id::SetupId>,
    ) -> JobHandle<scene::SceneView> {
        let document = self.document.clone();
        self.jobs
            .submit("build scene", self.events_tx.clone(), move |context| {
                scene::build_scene(&document.location, &document.project, setup, context)
            })
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
        self.add_model_with(file, units, &[])
    }

    /// [`Application::add_model`] with placement steps (IMP-05), as one
    /// undoable step: the import dialog adds a model as the user placed it
    /// (decision record 0026 item 13).
    pub fn add_model_with(
        &mut self,
        file: &Path,
        units: Option<LengthUnit>,
        transform: &[model::TransformStep],
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
        let steps = transform.to_vec();
        let inspection = self
            .jobs
            .submit("import model", self.events_tx.clone(), move |context| {
                model::inspect_bytes(&path, &bytes, units, &steps, context)
            })
            .wait()?;
        let id = ModelId::new();
        let mut reference = ModelRef::new(id, relative, hash);
        reference.units = units;
        reference.transform = transform.to_vec();
        self.execute(Command::Add {
            item: Item::Model(reference),
            placement: Placement::End,
        })?;
        Ok((id, inspection))
    }

    /// Starts reading, placing and checking a model file that is not yet in
    /// the project, for the import dialog: the inspection and a preview scene
    /// with the problems highlighted (OI-51).
    pub fn start_import_preview(
        &self,
        file: &Path,
        units: Option<LengthUnit>,
        transform: &[model::TransformStep],
    ) -> JobHandle<model::ImportPreview> {
        let path = file.to_path_buf();
        let steps = transform.to_vec();
        self.jobs
            .submit("check model", self.events_tx.clone(), move |context| {
                model::import_preview(&path, units, &steps, context)
            })
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

    /// The parameter form of an operation (UI-07).
    pub fn parameter_form(&self, operation: OperationId) -> Result<form::ParameterForm, AppError> {
        form::parameter_form(&self.document.project, &self.strategies, operation)
    }

    /// Sets an operation parameter, or clears it with `None`, after checking
    /// the value against the strategy's schema. An invalid value is an error
    /// naming the parameter, and the project is left unchanged (UI-07).
    pub fn set_parameter(
        &mut self,
        operation: OperationId,
        name: &str,
        value: Option<openchuck_project::ParameterValue>,
    ) -> Result<(), AppError> {
        form::check_parameter(
            &self.document.project,
            &self.strategies,
            operation,
            name,
            value.as_ref(),
        )?;
        self.execute(Command::SetParameter {
            operation,
            name: name.to_owned(),
            value,
        })
    }

    /// Sets one field of a project tool (UI-07), after checking the whole
    /// tool. An invalid value is an error naming the field, and the project
    /// is left unchanged.
    pub fn set_tool_field(
        &mut self,
        tool: openchuck_project::id::ToolId,
        name: &str,
        value: &openchuck_project::ParameterValue,
    ) -> Result<(), AppError> {
        let mut snapshot = self.tool_snapshot(tool)?;
        snapshot.tool = tool_edit::set_tool_field(&snapshot.tool, name, value)?;
        self.execute(Command::Replace(Item::Tool(snapshot)))
    }

    /// Gives a project tool a custom profile (TL-03), after the same checks
    /// as importing one from a file. An invalid profile is an error naming
    /// the segment, and the project is left unchanged.
    pub fn set_tool_profile(
        &mut self,
        tool: openchuck_project::id::ToolId,
        profile: tools::ToolProfile,
    ) -> Result<(), AppError> {
        let mut snapshot = self.tool_snapshot(tool)?;
        snapshot.tool = tool_edit::set_tool_profile(&snapshot.tool, profile)?;
        self.execute(Command::Replace(Item::Tool(snapshot)))
    }

    fn tool_snapshot(
        &self,
        tool: openchuck_project::id::ToolId,
    ) -> Result<openchuck_project::ToolSnapshot, AppError> {
        self.document
            .project
            .tools()
            .iter()
            .find(|t| t.id == tool)
            .cloned()
            .ok_or_else(|| {
                openchuck_project::ProjectError::NotFound {
                    kind: openchuck_project::id::ToolId::KIND,
                    id: tool.to_string(),
                }
                .into()
            })
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
        self.start_pipeline("regenerate", false)
    }

    /// Starts regenerating and simulating every operation as a background
    /// job: the result carries a simulation report (SIM-03, SIM-05 to SIM-07).
    pub fn start_simulate(&self) -> JobHandle<Regeneration> {
        self.start_pipeline("simulate", true)
    }

    fn start_pipeline(&self, name: &str, simulate: bool) -> JobHandle<Regeneration> {
        let document = self.document.clone();
        let strategies = self.strategies.clone();
        let post = self.post.clone();
        self.jobs
            .submit(name, self.events_tx.clone(), move |context| {
                pipeline::regenerate(
                    &document.location,
                    &document.project,
                    &strategies,
                    post.as_ref(),
                    simulate,
                    context,
                )
            })
    }

    /// Regenerates, simulates and waits for the result.
    pub fn simulate(&self) -> Result<Regeneration, AppError> {
        self.start_simulate().wait()
    }

    /// Chooses the post that turns toolpaths into G-code (GC-02, OI-88).
    /// Without one, regeneration produces toolpaths only.
    pub fn set_post(&mut self, post: Option<PostSelection>) {
        self.post = post;
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
