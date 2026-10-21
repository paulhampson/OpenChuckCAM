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
//! Triangle selections, named sets and limiting an operation to them are in
//! [`selection`] (UI-04, UI-05, PRJ-06, OP-02). The library's own strategy, helical finishing, is registered from the start;
//! a host or test can add more with [`Application::register_strategy`].
//! What the viewport draws for a setup comes from [`scene`] as plain data
//! (decision record 0029).

#![warn(missing_docs)]

mod acknowledgement;
mod action_log;
mod cache;
mod command;
mod error;
mod export;
pub mod form;
mod jobs;
pub mod library;
pub mod machine_view;
pub mod model;
pub mod operation;
pub mod opflow;
pub mod overview;
pub mod pathview;
pub mod pick;
mod pipeline;
pub mod playback;
mod posting;
pub mod scene;
pub mod selection;
pub mod settings;
pub mod setup;
pub mod setup_form;
pub mod setup_panes;
mod setup_sheet;
pub mod simulation;
pub mod stockplay;
pub mod tool_edit;
pub mod tool_library;
pub mod tools;
pub mod workholding;
pub mod workholding_view;

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};

use command::History;
use model::{LengthUnit, ModelInspection};
use openchuck_project::id::{ModelId, OperationId};
use openchuck_project::{
    ContentHash, Item, ModelCheck, ModelRef, Placement, Project, RelativePath,
};
use setup::{PartExtent, SetupReport};

pub use acknowledgement::{Acknowledgements, Lookup as AcknowledgementLookup};
pub use cache::DEFAULT_CACHE_VERSIONS;
pub use command::{
    Command, NO_MODEL_REASON, ONE_SETUP_REASON, add_setup_blocked, can_add_setup,
    duplicate_operation, new_setup,
};
pub use error::AppError;
pub use export::{
    Check, CheckLevel, EXPORT_ACKNOWLEDGE_LABEL, EXPORT_NOTICE, EXPORT_RULE, ExportChecks,
    PostSummary, builtin_post_summaries, post_summary, write_gcode_files,
};
pub use jobs::{
    FINISHED_JOBS_KEPT, JobEntry, JobHandle, JobId, JobLog, JobManager, JobOutcome, Threads,
    job_label,
};
/// The G-code reader and replay, re-exported so front ends need no engine
/// crate of their own (architecture section 5.8).
pub use openchuck_gcode as gcode;
pub use openchuck_path as path;
pub use openchuck_path::{Registry, Strategy};
/// The project model that commands carry, re-exported so front ends need no
/// engine crate of their own (architecture section 5.8).
pub use openchuck_project as project;
pub use openchuck_project::new_folder;
pub use openchuck_sim as sim;
pub use openchuck_types::notice::VERIFY_GCODE_NOTICE;
pub use opflow::StrategyChoice;
pub use pipeline::{GroupInfo, Regeneration};
pub use posting::{GcodeComparison, GcodeReport, OutputGrouping, PlannedFile, PostSelection};
pub use selection::{
    CheckedSet, EditStart, RegionRequest, SelectionEdit, SelectionRole, SelectionSummary,
    SelectionUse, TriangleSet,
};
pub use setup_sheet::{SetupSheet, SheetBody, SheetSection};
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
        /// Whether it stopped because it was cancelled on request.
        cancelled: bool,
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
    cache: cache::PipelineCache,
    strategies: Registry,
    post: Option<PostSelection>,
    grouping: OutputGrouping,
    events_tx: Sender<Event>,
    events_rx: Receiver<Event>,
    /// Changes with every command, undo and redo ([`Application::revision`]).
    revision: u64,
    /// What generation has made, for the item states (decision record 0117).
    record: std::sync::Arc<overview::GenerationRecord>,
    /// The lines for the user-action log not yet drained (NFR-18).
    actions: Vec<String>,
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
            cache: cache::PipelineCache::default(),
            strategies: Registry::builtin(),
            post: None,
            grouping: OutputGrouping::default(),
            events_tx,
            events_rx,
            revision: 0,
            record: std::sync::Arc::default(),
            actions: Vec::new(),
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

    /// The default retaining tabs of `setup` (OP-21, decision 0035 item 4 and
    /// the owner's answers to OI-158), as explicit tabs to add to its
    /// workholding: one in the head zone and one in the tail zone, computed
    /// from the part's extent. The setup is not changed; the front end shows
    /// them and replaces the setup, which can be undone (decision 0045 item
    /// 3). Reads and places the part, so it runs as a job.
    pub fn start_default_tabs(
        &self,
        setup: openchuck_project::id::SetupId,
    ) -> JobHandle<Vec<openchuck_types::workholding::Tab>> {
        let document = self.document.clone();
        self.jobs
            .submit("default tabs", self.events_tx.clone(), move |context| {
                workholding::default_tabs_of(&document.location, &document.project, setup, context)
            })
    }

    /// What the workholding section needs from the placed part of `setup`:
    /// its extent and the default tab of each zone for the stored held regions
    /// (interface specification 11.7; decision record 0114, OI-334 c, d and
    /// e). Reads and places the part, so it runs as a job. Fails when the
    /// setup has no part.
    pub fn start_part_extent(
        &self,
        setup: openchuck_project::id::SetupId,
    ) -> JobHandle<workholding::PartFacts> {
        let document = self.document.clone();
        self.jobs
            .submit("part extent", self.events_tx.clone(), move |context| {
                workholding::part_facts_of(&document.location, &document.project, setup, context)
            })
    }

    /// What is wrong with `draft` as the workholding of `setup`, from the
    /// part and the tools, which the setup's own validation cannot see: tabs
    /// outside the head and tail zones (OI-180) and tabs narrower than a tool
    /// of the setup's operations (OI-159). A front end runs it before it
    /// stores an edit; generation refuses the same things. Empty when there
    /// is nothing wrong or no part to check against. Reads and places the
    /// part, so it runs as a job.
    pub fn start_check_workholding(
        &self,
        setup: openchuck_project::id::SetupId,
        draft: &openchuck_types::workholding::Workholding,
    ) -> JobHandle<Vec<workholding::WorkholdingFinding>> {
        let document = self.document.clone();
        let draft = draft.clone();
        self.jobs.submit(
            "check workholding",
            self.events_tx.clone(),
            move |context| {
                let mut edited = document
                    .project
                    .setup(setup)
                    .ok_or_else(|| AppError::UnknownSetup(setup.to_string()))?
                    .clone();
                edited.workholding = draft;
                workholding::findings_of(&document.location, &document.project, &edited, context)
            },
        )
    }

    /// As [`Application::start_check_workholding`], for `s` as it would be
    /// stored, which need not be in the project yet (a setup being added from
    /// the command line), waiting for the result. For a front end that may
    /// block, such as the command line; an interface starts the job instead.
    pub fn check_setup_workholding(
        &self,
        s: &openchuck_project::Setup,
    ) -> Result<Vec<String>, AppError> {
        let document = self.document.clone();
        let s = s.clone();
        self.jobs
            .submit(
                "check workholding",
                self.events_tx.clone(),
                move |context| {
                    workholding::findings_of(&document.location, &document.project, &s, context)
                },
            )
            .wait()
            .map(|found| found.into_iter().map(|f| f.text).collect())
    }

    /// Notes about the selections of `operation` that reach a held region or a
    /// tab of its setup (SET-09, OP-21; UI-03): those are never machined, so
    /// the selection cuts less than it names there. Reads and places the part,
    /// so it runs as a job. Empty when the setup holds nothing.
    pub fn start_workholding_warnings(
        &self,
        operation: openchuck_project::id::OperationId,
    ) -> JobHandle<Vec<String>> {
        let document = self.document.clone();
        self.jobs.submit(
            "workholding warnings",
            self.events_tx.clone(),
            move |context| {
                workholding::reach_warnings_of(
                    &document.location,
                    &document.project,
                    operation,
                    context,
                )
            },
        )
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
    /// (decision record 0029 item 13). It waits for the file to be read and
    /// checked; a front end that must stay responsive uses
    /// [`Application::start_prepare_model`] and
    /// [`Application::add_prepared_model`] instead.
    pub fn add_model_with(
        &mut self,
        file: &Path,
        units: Option<LengthUnit>,
        transform: &[model::TransformStep],
    ) -> Result<(ModelId, ModelInspection), AppError> {
        let prepared = self.start_prepare_model(file, units, transform)?.wait()?;
        self.add_prepared_model(prepared)
    }

    /// Starts reading, hashing, placing and checking a model file as a
    /// background job, ready to add with [`Application::add_prepared_model`]
    /// (UI-09, NFR-04). Fails at once if the file cannot be referenced from
    /// the project folder.
    pub fn start_prepare_model(
        &self,
        file: &Path,
        units: Option<LengthUnit>,
        transform: &[model::TransformStep],
    ) -> Result<JobHandle<model::PreparedModel>, AppError> {
        let relative = model::relative_to(&self.document.location, file)?.ok_or_else(|| {
            AppError::ModelPath {
                path: file.to_path_buf(),
                reason: "it is on another drive".to_owned(),
            }
        })?;
        let relative = RelativePath::from_path(&relative)?;
        let path = file.to_path_buf();
        let steps = transform.to_vec();
        Ok(self
            .jobs
            .submit("import model", self.events_tx.clone(), move |context| {
                let bytes = model::read(&path)?;
                let hash = ContentHash::of_bytes(&bytes);
                let inspection = model::inspect_bytes(&path, &bytes, units, &steps, context)?;
                let mut reference = ModelRef::new(ModelId::new(), relative, hash);
                reference.units = units;
                reference.transform = steps;
                Ok(model::PreparedModel {
                    reference,
                    inspection,
                })
            }))
    }

    /// Adds a model prepared by [`Application::start_prepare_model`] as one
    /// undoable step.
    pub fn add_prepared_model(
        &mut self,
        prepared: model::PreparedModel,
    ) -> Result<(ModelId, ModelInspection), AppError> {
        let id = prepared.reference.id;
        self.execute(Command::Add {
            item: Item::Model(prepared.reference),
            placement: Placement::End,
        })?;
        Ok((id, prepared.inspection))
    }

    /// Starts reading, hashing, placing and checking `file` to replace the
    /// file of the project's model `id`, with the model's unit and placement
    /// (the Model inspector's "Replace file…", interface specification 11.2;
    /// decision record 0114 C14). [`Application::replace_model`] stores it.
    /// Selection sets made on the old file then say so, as they do when a
    /// file changes on disk.
    pub fn start_replace_model(
        &self,
        id: ModelId,
        file: &Path,
    ) -> Result<JobHandle<model::PreparedModel>, AppError> {
        let reference = self
            .document
            .project
            .models()
            .iter()
            .find(|m| m.id == id)
            .ok_or_else(|| AppError::UnknownModel(id.to_string()))?;
        self.start_prepare_model(file, reference.units, &reference.transform)
    }

    /// Stores a file prepared by [`Application::start_replace_model`] as the
    /// file of model `id`, keeping its ID, unit and placement, as one undoable
    /// step.
    pub fn replace_model(
        &mut self,
        id: ModelId,
        prepared: model::PreparedModel,
    ) -> Result<ModelInspection, AppError> {
        let old = self
            .document
            .project
            .models()
            .iter()
            .find(|m| m.id == id)
            .cloned()
            .ok_or_else(|| AppError::UnknownModel(id.to_string()))?;
        let mut reference = prepared.reference;
        reference.id = id;
        reference.units = old.units;
        reference.transform = old.transform;
        self.execute(Command::Replace(Item::Model(reference)))?;
        Ok(prepared.inspection)
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
            let mut warnings = warnings;
            warnings.extend(
                openchuck_types::workholding::check_tab_zones(
                    &s.workholding,
                    s.stock.as_ref(),
                    part.as_ref(),
                )
                .into_iter()
                .map(|kind| setup::SetupWarning {
                    kind,
                    unconfirmed: Vec::new(),
                }),
            );
            // A tab narrower than a tool that operations of the setup use is
            // refused when they are generated (OI-159); say so here, when the
            // tab or the tool is chosen.
            notes.extend(workholding::narrow_tab_notes(project, s));
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
        let text = action_log::describe(&command, &self.document.project);
        self.history.execute(&mut self.document.project, command)?;
        self.revision += 1;
        self.note_action(format!("command: {text}"));
        Ok(())
    }

    /// Undoes the latest command. Returns false when there is nothing to undo.
    pub fn undo(&mut self) -> Result<bool, AppError> {
        let text = self
            .history
            .next_undo()
            .map(|c| action_log::describe(c, &self.document.project));
        let undone = self.history.undo(&mut self.document.project)?;
        self.revision += u64::from(undone);
        if let (true, Some(text)) = (undone, text) {
            self.note_action(format!("undo: {text}"));
        }
        Ok(undone)
    }

    /// Redoes the latest undone command. Returns false when there is nothing to redo.
    pub fn redo(&mut self) -> Result<bool, AppError> {
        let text = self
            .history
            .next_redo()
            .map(|c| action_log::describe(c, &self.document.project));
        let redone = self.history.redo(&mut self.document.project)?;
        self.revision += u64::from(redone);
        if let (true, Some(text)) = (redone, text) {
            self.note_action(format!("redo: {text}"));
        }
        Ok(redone)
    }

    /// Keeps a line for the user-action log (NFR-18), the oldest dropped
    /// past a bound.
    fn note_action(&mut self, line: String) {
        if self.actions.len() >= action_log::KEPT {
            self.actions.remove(0);
        }
        self.actions.push(line);
    }

    /// The lines for the user-action log since the last call, oldest first:
    /// one per command, undo and redo, naming the action and the kind and
    /// name of the item, and parameter names, never values (NFR-18). The
    /// front end writes them only when the user's switch is on.
    pub fn drain_actions(&mut self) -> Vec<String> {
        std::mem::take(&mut self.actions)
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
        self.start_pipeline("regenerate", false, None)
    }

    /// Starts generating the enabled operations up to and including
    /// `operation`, in machining order, as a background job; the ones after
    /// it are left as they are (stale if they were). Operations before it are
    /// reused from the cache where nothing they read changed. This is
    /// "generate selected" (G, specification section 16; decision record
    /// 0117, OI-351). A disabled or unknown operation is refused.
    pub fn start_regenerate_until(&self, operation: OperationId) -> JobHandle<Regeneration> {
        self.start_pipeline("regenerate", false, Some(operation))
    }

    /// [`Application::start_regenerate_until`], waiting for the result.
    pub fn regenerate_until(&self, operation: OperationId) -> Result<Regeneration, AppError> {
        self.start_regenerate_until(operation).wait()
    }

    /// Starts regenerating and simulating every operation as a background
    /// job: the result carries a simulation report (SIM-03, SIM-05 to SIM-07).
    pub fn start_simulate(&self) -> JobHandle<Regeneration> {
        self.start_pipeline("simulate", true, None)
    }

    fn start_pipeline(
        &self,
        name: &str,
        simulate: bool,
        until: Option<OperationId>,
    ) -> JobHandle<Regeneration> {
        let document = self.document.clone();
        let strategies = self.strategies.clone();
        let post = self.post.clone();
        let grouping = self.grouping;
        let cache = self.cache.clone();
        let record = std::sync::Arc::clone(&self.record);
        self.jobs
            .submit(name, self.events_tx.clone(), move |context| {
                let result = pipeline::regenerate(
                    &document.location,
                    &document.project,
                    &strategies,
                    post.as_ref().map(|p| (p, grouping)),
                    simulate,
                    &cache,
                    &record,
                    until,
                    context,
                );
                match &result {
                    Ok(run) => record.succeeded(&document.project, simulate, run),
                    Err(e) if e.is_cancelled() => record.cancelled(),
                    Err(e) => record.failed(&document.project, e.to_string()),
                }
                result
            })
    }

    /// Regenerates, simulates and waits for the result.
    pub fn simulate(&self) -> Result<Regeneration, AppError> {
        self.start_simulate().wait()
    }

    /// How many generated results are kept for each operation, so that
    /// going back to an earlier state (an undo, a parameter toggled back)
    /// reuses a result instead of regenerating. At least the latest one is
    /// useful; 0 turns the cache off. The default is
    /// [`DEFAULT_CACHE_VERSIONS`].
    pub fn cache_versions(&self) -> usize {
        self.cache.versions()
    }

    /// Sets how many results are kept for each operation (see
    /// [`Application::cache_versions`]). Results beyond the new number are
    /// dropped at once, oldest first. Each is a whole toolpath, so a higher
    /// number costs memory on long paths.
    pub fn set_cache_versions(&self, count: usize) {
        self.cache.set_versions(count);
    }

    /// Chooses the post that turns toolpaths into G-code (GC-02, OI-88).
    /// Without one, regeneration produces toolpaths only.
    pub fn set_post(&mut self, post: Option<PostSelection>) {
        self.post = post;
    }

    /// Chooses how the G-code is split into files (GC-05). The default is one
    /// file per setup.
    pub fn set_output_grouping(&mut self, grouping: OutputGrouping) {
        self.grouping = grouping;
    }

    /// How the G-code is split into files.
    pub fn output_grouping(&self) -> OutputGrouping {
        self.grouping
    }

    /// The files `grouping` would write for `toolpath` with `post`, with their
    /// names, setups and programmed times, before anything is posted (GC-05).
    pub fn planned_files(
        &self,
        toolpath: &openchuck_types::ir::Toolpath,
        post: &PostSelection,
        grouping: OutputGrouping,
    ) -> Result<Vec<PlannedFile>, AppError> {
        posting::plan_files(&self.document.project, toolpath, post, grouping)
    }

    /// Starts regenerating with `post` and `grouping` for this one run, leaving
    /// the application's own post and grouping as they were, so a later
    /// generation does not post G-code unasked. Cached operations are reused,
    /// so only posting and its checks cost time (GC-02, GC-05).
    pub fn start_regenerate_with_post(
        &mut self,
        post: PostSelection,
        grouping: OutputGrouping,
    ) -> JobHandle<Regeneration> {
        let (before_post, before_grouping) = (self.post.replace(post), self.grouping);
        self.grouping = grouping;
        let handle = self.start_pipeline("post", false, None);
        self.post = before_post;
        self.grouping = before_grouping;
        handle
    }

    /// Writes the setup sheet as HTML and PDF into `folder`, named after the
    /// project, and returns the two paths (GC-12). Pass the toolpath of a
    /// regeneration to include the programmed times.
    pub fn write_setup_sheet(
        &self,
        folder: &Path,
        toolpath: Option<&openchuck_types::ir::Toolpath>,
    ) -> Result<Vec<PathBuf>, AppError> {
        let sheet = self.setup_sheet(toolpath);
        let stem = format!(
            "{}-setup-sheet",
            posting::slug(self.document.project.name(), "project")
        );
        export::write_bytes(
            folder,
            &[
                (format!("{stem}.html"), sheet.to_html().into_bytes()),
                (format!("{stem}.pdf"), sheet.to_pdf()),
            ],
        )
    }

    /// The checks before writing G-code (GC-15). `run` is the latest
    /// regeneration, `posted` a run made with the chosen post (its files'
    /// checks count) and `last_error` the message of a later failed one.
    pub fn export_checks(
        &self,
        run: Option<&Regeneration>,
        posted: Option<&Regeneration>,
        last_error: Option<&str>,
    ) -> ExportChecks {
        export::export_checks(&self.document.project, run, posted, last_error)
    }

    /// The post in use, if one is chosen.
    pub fn post(&self) -> Option<&PostSelection> {
        self.post.as_ref()
    }

    /// The project's setup sheet (GC-12). Pass a regeneration's toolpath to
    /// include each operation's programmed time.
    pub fn setup_sheet(&self, toolpath: Option<&openchuck_types::ir::Toolpath>) -> SetupSheet {
        SetupSheet::new(&self.document.project, &self.strategies, toolpath)
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
        self.strategies.register(strategy)?;
        // A new strategy may replace what a cached result was made with.
        self.cache.clear();
        Ok(())
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
