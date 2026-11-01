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
pub mod checkpoint_schedule;
pub mod checkpoints;
mod command;
mod connections;
mod error;
mod export;
pub mod facing;
pub mod form;
mod jobs;
pub mod library;
pub mod machine_edit;
pub mod machine_library;
pub mod machine_view;
pub mod materials;
pub mod model;
pub mod note_groups;
pub mod operation;
pub mod opflow;
pub mod output_names;
pub mod output_preview;
mod output_write;
pub mod overview;
pub mod pathview;
pub mod pick;
mod pipeline;
pub mod playback;
mod posting;
pub mod preset;
mod ramp_entry;
pub mod requirements;
pub mod scene;
pub mod selection;
pub mod settings;
pub mod setup;
pub mod setup_form;
pub mod setup_panes;
mod setup_sheet;
pub mod simulation;
pub mod step_sets;
mod stock_surface;
pub mod stockplay;
#[cfg(test)]
mod test_project;
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
    Check, CheckLevel, EXPORT_ACKNOWLEDGE_LABEL, EXPORT_NOTICE, EXPORT_PARTIAL_PATH_LABEL,
    EXPORT_RULE, EXPORT_STOCK_SKIPPED, EXPORT_STOCK_SKIPPED_LABEL, ExportChecks, PostSummary,
    builtin_post_summaries, partial_operations, partial_path_refusal, post_summary, run_is_current,
    simulation_out_of_date, write_gcode_files,
};
pub use jobs::{
    FINISHED_JOBS_KEPT, JobEntry, JobHandle, JobId, JobLog, JobManager, JobOutcome,
    RUN_PROGRESS_TOTAL, Threads, job_label,
};
pub use model::ModelStorage;
/// The G-code reader and replay, re-exported so front ends need no engine
/// crate of their own (architecture section 5.8).
pub use openchuck_gcode as gcode;
pub use openchuck_path as path;
pub use openchuck_path::StrategyGroup;
pub use openchuck_path::{Registry, Strategy};
/// The project model that commands carry, re-exported so front ends need no
/// engine crate of their own (architecture section 5.8).
pub use openchuck_project as project;
pub use openchuck_project::new_folder;
pub use openchuck_sim as sim;
pub use openchuck_types::notice::VERIFY_GCODE_NOTICE;
pub use opflow::{SetOptionsShown, StrategyChoice};
pub use output_names::{FileNameTemplates, OutputFiles, ProgramFile, ProgramStep};
pub use output_write::{OutputFailure, OutputPlan, OutputReport};
pub use pipeline::{GroupInfo, Regeneration, STAGE_GENERATE, STAGE_SIMULATE};
pub use posting::{GcodeComparison, GcodeReport, OutputGrouping, PlannedFile, PostSelection};
pub use selection::{
    CheckedSet, EditStart, RegionRequest, SelectionEdit, SelectionRole, SelectionSummary,
    SelectionUse, TriangleSet,
};
pub use setup_sheet::{
    Clock, FixedClock, SetupSheet, SheetBody, SheetSection, SystemClock, Traceability, iso8601_utc,
};
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
    recovered_unsaved: std::sync::atomic::AtomicBool,
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
    /// Where a setup sheet reads its generation time (decision 0110).
    clock: Box<dyn Clock>,
    /// The user's file name templates (issues 308 and 310); the defaults
    /// give the names written before templates existed.
    file_names: output_names::FileNameTemplates,
    /// Playback's checkpoints kept while simulating, within this many bytes
    /// ("Generate playback information with toolpath generation", decision
    /// record 0333); `None` keeps none, and playback simulates again.
    playback_with_generation: Option<usize>,
}

/// What a new project starts with, from the user's settings
/// ([`settings::SimulateSettings`]).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NewProject {
    /// Whether it skips the stock simulation (issue 214).
    pub skip_stock_simulation: bool,
}

/// The newest modification time and a fingerprint of the files `project`
/// was saved as in `folder`, read from disk, or `None` when any of them is
/// missing or unreadable (the project is not saved there). The fingerprint is
/// the hash of each file's path, length and bytes in path order.
fn saved_files_trace(folder: &Path, project: &Project) -> Option<(std::time::SystemTime, String)> {
    let files = openchuck_project::file::render(project).ok()?;
    let mut newest = std::time::UNIX_EPOCH;
    let mut bytes = Vec::new();
    for relative in files.keys() {
        let path = folder.join(relative);
        let content = std::fs::read(&path).ok()?;
        newest = newest.max(std::fs::metadata(&path).ok()?.modified().ok()?);
        bytes.extend_from_slice(relative.as_bytes());
        bytes.extend_from_slice(&(content.len() as u64).to_le_bytes());
        bytes.extend_from_slice(&content);
    }
    Some((
        newest,
        openchuck_project::ContentHash::of_bytes(&bytes).to_string(),
    ))
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
        Self::create_with(folder, name, threads, NewProject::default())
    }

    /// [`Application::create`], with the user's defaults for a new project
    /// (decision record 0168) saved in it from the start.
    pub fn create_with(
        folder: &Path,
        name: &str,
        threads: Threads,
        defaults: NewProject,
    ) -> Result<Self, AppError> {
        let mut project = openchuck_project::file::create(folder, name)?;
        if defaults.skip_stock_simulation {
            project.set_skip_stock_simulation(true);
            openchuck_project::file::save(&project, folder)?;
        }
        Self::with_project(folder, project, threads)
    }

    fn with_project(folder: &Path, project: Project, threads: Threads) -> Result<Self, AppError> {
        let (events_tx, events_rx) = channel();
        Ok(Self {
            saved: std::sync::Mutex::new(project.clone()),
            recovered_unsaved: std::sync::atomic::AtomicBool::new(false),
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
            clock: Box::new(SystemClock),
            file_names: output_names::FileNameTemplates::default(),
            playback_with_generation: None,
        })
    }

    /// Saves the project to its folder. Only changed files are written
    /// (PRJ-01).
    pub fn save(&self) -> Result<(), AppError> {
        openchuck_project::file::save(&self.document.project, &self.document.location)?;
        if let Ok(mut saved) = self.saved.lock() {
            saved.clone_from(&self.document.project);
        }
        self.recovered_unsaved
            .store(false, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    /// Whether the project differs from what was last opened, created or
    /// saved, so a front end can ask before discarding changes. Undoing back
    /// to the saved state makes it unmodified again.
    pub fn is_modified(&self) -> bool {
        self.recovered_unsaved
            .load(std::sync::atomic::Ordering::Relaxed)
            || self
                .saved
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
                let path = m.label();
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
        self.start_scene_with_facing(setup, None)
    }

    /// Scene plus actual Facing cutter sweep from a current regeneration.
    /// Stale results are never overlaid on edited draft geometry.
    pub fn start_scene_with_facing(
        &self,
        setup: Option<openchuck_project::id::SetupId>,
        run: Option<std::sync::Arc<Regeneration>>,
    ) -> JobHandle<scene::SceneView> {
        let document = self.document.clone();
        self.jobs
            .submit("build scene", self.events_tx.clone(), move |context| {
                let mut view =
                    scene::build_scene(&document.location, &document.project, setup, context)?;
                if let Some(run) = run.filter(|r| r.project.as_ref() == &document.project) {
                    facing::display::add_generated(&mut view, &run, context)?;
                }
                Ok(view)
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
    /// problems are reported in the returned inspection, not refused. A file
    /// outside the project folder is copied into its `models` folder first
    /// (decision record 0307); the file is referenced by its path relative to
    /// the project folder with its content hash (PRJ-02). Undoable like any
    /// command.
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
    /// (UI-09, NFR-04). A file outside the project folder is copied into it
    /// by the same job (decision record 0307 item 1), so the front end starts
    /// it when the user adds the model. Fails at once if the file cannot be
    /// found.
    pub fn start_prepare_model(
        &self,
        file: &Path,
        units: Option<LengthUnit>,
        transform: &[model::TransformStep],
    ) -> Result<JobHandle<model::PreparedModel>, AppError> {
        self.start_prepare_model_as(file, units, transform, model::ModelStorage::Copy)
    }

    /// [`Application::start_prepare_model`], keeping a file from outside the
    /// project folder as `storage` says: copied into `models/` before the
    /// job ends, from the bytes it read and hashed (so the copy has the
    /// recorded hash), or linked by its relative path. A failed copy fails
    /// the job and nothing is added.
    pub fn start_prepare_model_as(
        &self,
        file: &Path,
        units: Option<LengthUnit>,
        transform: &[model::TransformStep],
        storage: model::ModelStorage,
    ) -> Result<JobHandle<model::PreparedModel>, AppError> {
        // `None` when the file cannot be named from the project folder: on
        // another drive, or with a character a project path cannot hold
        // (such as `\` in a Unix file name). It can still be copied.
        let relative = model::relative_to(&self.document.location, file)?
            .and_then(|r| RelativePath::from_path(&r).ok());
        let root = self.document.location.clone();
        let path = file.to_path_buf();
        let steps = transform.to_vec();
        Ok(self
            .jobs
            .submit("import model", self.events_tx.clone(), move |context| {
                // The reader's stages report as one share that never starts
                // again (issue 201).
                let progress = jobs::RunProgress::new(context);
                progress.reach("import model", 0.0);
                let share = progress.share("import model", 0.0, 1.0);
                let within = openchuck_types::job::JobContext::new(context.cancel, &share);
                let bytes = model::read(&path)?;
                let hash = ContentHash::of_bytes(&bytes);
                let inspection = model::inspect_bytes(&path, &bytes, units, &steps, &within)?;
                share.finish();
                let name = model::original_name(&path);
                let stored = match (relative, storage) {
                    (Some(inside), _) if !inside.is_outside_project() => inside,
                    (_, model::ModelStorage::Copy) => {
                        context.check_cancelled()?;
                        let extension = path
                            .extension()
                            .map(|e| e.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        model::copy_into_project(&root, &bytes, &hash, &extension)?
                    }
                    (Some(link), model::ModelStorage::Link) => link,
                    (None, model::ModelStorage::Link) => {
                        return Err(AppError::ModelPath {
                            path: path.clone(),
                            reason: "it is on another drive, or its path has a character a \
                                     project path cannot hold; copy it into the project instead"
                                .to_owned(),
                        });
                    }
                };
                let mut reference = ModelRef::new(ModelId::new(), stored, hash);
                reference.name = (reference.path.file_name() != name).then_some(name);
                reference.units = units;
                reference.transform = steps;
                Ok(model::PreparedModel {
                    reference,
                    inspection,
                })
            }))
    }

    /// Adds a model prepared by [`Application::start_prepare_model`] as one
    /// undoable step. Undo removes the model but leaves a copied file, which
    /// adding the same file again reuses.
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

    /// [`Application::add_model`] with the file from outside the project
    /// folder kept as `storage` says (the command line's `--link`).
    pub fn add_model_as(
        &mut self,
        file: &Path,
        units: Option<LengthUnit>,
        storage: model::ModelStorage,
    ) -> Result<(ModelId, ModelInspection), AppError> {
        let prepared = self
            .start_prepare_model_as(file, units, &[], storage)?
            .wait()?;
        self.add_prepared_model(prepared)
    }

    /// Whether `file` is inside the project folder, so adding it uses it
    /// where it is; `false` for a file outside it, on another drive or not
    /// found (the import dialog's copy choice, decision record 0307 item 1).
    pub fn is_in_project_folder(&self, file: &Path) -> bool {
        model::relative_to(&self.document.location, file)
            .ok()
            .flatten()
            .and_then(|r| RelativePath::from_path(&r).ok())
            .is_some_and(|r| !r.is_outside_project())
    }

    /// The models whose files are outside the project folder (linked), in
    /// list order (decision record 0307 item 5).
    pub fn linked_models(&self) -> Vec<ModelId> {
        self.document
            .project
            .models()
            .iter()
            .filter(|m| m.path.is_outside_project())
            .map(|m| m.id)
            .collect()
    }

    /// One note for Messages per linked model, saying the folder is not
    /// self-contained and how to make it so (decision record 0307 item 8).
    pub fn linked_model_notes(&self) -> Vec<String> {
        self.document
            .project
            .models()
            .iter()
            .filter(|m| m.path.is_outside_project())
            .map(model::linked_note)
            .collect()
    }

    /// Starts copying the files of the linked models `ids` into the
    /// project's `models` folder as a background job ("Copy model into
    /// project", decision record 0307 items 5 and 10), so a large file does
    /// not hold up the window (UI-09). The job reads, checks and writes the
    /// files only; [`Application::finish_copying_models`] stores the new paths.
    /// A model that is not linked, is missing or has changed since it was
    /// added is refused with what to do.
    pub fn start_copying_models(&self, ids: &[ModelId]) -> JobHandle<Vec<model::ModelCopy>> {
        let models: Vec<Result<ModelRef, String>> = ids
            .iter()
            .map(|id| {
                self.document
                    .project
                    .models()
                    .iter()
                    .find(|m| m.id == *id)
                    .cloned()
                    .ok_or_else(|| AppError::UnknownModel(id.to_string()).to_string())
            })
            .collect();
        let root = self.document.location.clone();
        self.jobs
            .submit("copy model", self.events_tx.clone(), move |context| {
                let mut done = Vec::new();
                for model in models {
                    context.check_cancelled()?;
                    done.push(
                        model.and_then(|m| model::copy_model(&root, &m).map_err(|e| e.to_string())),
                    );
                }
                Ok(done)
            })
    }

    /// Stores the paths copied by [`Application::start_copying_models`], one
    /// undoable step for each model. Only the stored path and name change:
    /// the ID, hash, unit, placement and the selection sets made on it stay,
    /// so nothing needs generating again. A model changed in the project
    /// while it was being copied is not touched. Returns the models copied
    /// with their new paths, and a message for each one refused.
    pub fn finish_copying_models(
        &mut self,
        copies: Vec<model::ModelCopy>,
    ) -> (Vec<(ModelId, RelativePath)>, Vec<String>) {
        let mut copied = Vec::new();
        let mut refused = Vec::new();
        for copy in copies {
            let (before, after) = match copy {
                Ok(pair) => pair,
                Err(e) => {
                    refused.push(e);
                    continue;
                }
            };
            let now = self
                .document
                .project
                .models()
                .iter()
                .find(|m| m.id == before.id);
            if now != Some(&before) {
                refused.push(format!(
                    "'{}' changed in the project while it was being copied; nothing was changed",
                    before.display_name()
                ));
                continue;
            }
            let (id, path) = (after.id, after.path.clone());
            match self.execute(Command::Replace(Item::Model(after))) {
                Ok(()) => copied.push((id, path)),
                Err(e) => refused.push(e.to_string()),
            }
        }
        (copied, refused)
    }

    /// Copies the linked model `id` into the project folder and waits
    /// ([`Application::start_copying_models`] then
    /// [`Application::finish_copying_models`]), for the command line.
    pub fn copy_model_into_project(&mut self, id: ModelId) -> Result<RelativePath, AppError> {
        let copies = self.start_copying_models(&[id]).wait()?;
        let (copied, refused) = self.finish_copying_models(copies);
        match (copied.into_iter().next(), refused.into_iter().next()) {
            (Some((_, path)), _) => Ok(path),
            (None, Some(reason)) => Err(AppError::ModelCopy(reason)),
            (None, None) => Err(AppError::UnknownModel(id.to_string())),
        }
    }

    /// [`Application::copy_model_into_project`] for every linked model, and
    /// waits: the models copied with their new paths, and a message for each
    /// one refused.
    pub fn copy_linked_models(&mut self) -> (Vec<(ModelId, RelativePath)>, Vec<String>) {
        let ids = self.linked_models();
        match self.start_copying_models(&ids).wait() {
            Ok(copies) => self.finish_copying_models(copies),
            Err(e) => (Vec::new(), vec![e.to_string()]),
        }
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
            .map_or_else(|| self.find_model_by_name(text), Ok)
    }

    /// The one model whose recorded original name is `text`; two models with
    /// that name are refused, since it does not say which (decision record
    /// 0307 item 4).
    fn find_model_by_name(&self, text: &str) -> Result<&ModelRef, AppError> {
        let mut named = self
            .document
            .project
            .models()
            .iter()
            .filter(|m| m.name.as_deref() == Some(text));
        match (named.next(), named.next()) {
            (Some(m), None) => Ok(m),
            (Some(_), Some(_)) => Err(AppError::AmbiguousModel(text.to_owned())),
            _ => Err(AppError::UnknownModel(text.to_owned())),
        }
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
                        // Where this setup's offset puts the part (decision
                        // record 0201).
                        Some(Ok(extent)) => extent.map(|e| e.in_setup(&s.frame)),
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
                        rules: &s.clearance_rules,
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
            warnings.extend(
                openchuck_types::workholding::check_held_regions(
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

    /// Selects all four Box sides in one undoable command, preserving each
    /// stored offset/default state and every arbitrary row (OP-24, UI-07).
    pub fn select_all_facing_sides(&mut self, operation: OperationId) -> Result<(), AppError> {
        let op = self
            .document
            .project
            .operation(operation)
            .ok_or_else(|| AppError::UnknownOperation(operation.to_string()))?;
        if op.strategy != openchuck_path::facing::NAME
            || !matches!(op.parameters.get("shape"),Some(openchuck_project::ParameterValue::Text(mode)) if mode=="box")
        {
            return Err(AppError::Operation {
                name: op.name.clone(),
                reason: "All four requires Box sides facing".into(),
            });
        }
        let commands = ["positive_z", "positive_y", "negative_z", "negative_y"]
            .into_iter()
            .filter(|name| {
                op.parameters.get(*name) != Some(&openchuck_project::ParameterValue::Flag(true))
            })
            .map(|name| Command::SetParameter {
                operation,
                name: name.into(),
                value: Some(openchuck_project::ParameterValue::Flag(true)),
            })
            .collect::<Vec<_>>();
        if commands.is_empty() {
            return Ok(());
        }
        self.execute(Command::Batch(commands))
    }

    /// Resets a parameter to its default. A Facing plane returns to part plus
    /// allowance derivation as one undoable edit; clearing manual text with
    /// set_parameter(None) remains distinct (OP-24, UI-07).
    pub fn reset_parameter(&mut self, operation: OperationId, name: &str) -> Result<(), AppError> {
        let op = self
            .document
            .project
            .operation(operation)
            .ok_or_else(|| AppError::UnknownOperation(operation.to_string()))?;
        let side = name
            .strip_suffix("_offset")
            .filter(|side| ["positive_z", "positive_y", "negative_z", "negative_y"].contains(side));
        if op.strategy == openchuck_path::facing::NAME
            && matches!(op.parameters.get("shape"),Some(openchuck_project::ParameterValue::Text(shape)) if shape=="box")
            && let Some(side) = side
        {
            let derived = format!("{side}_from_part");
            let value = openchuck_project::ParameterValue::Flag(true);
            form::check_parameter(
                &self.document.project,
                &self.strategies,
                operation,
                name,
                None,
            )?;
            form::check_parameter(
                &self.document.project,
                &self.strategies,
                operation,
                &derived,
                Some(&value),
            )?;
            if !op.parameters.contains_key(name)
                && matches!(
                    op.parameters.get(&derived),
                    None | Some(openchuck_project::ParameterValue::Flag(true))
                )
            {
                return Ok(());
            }
            return self.execute(Command::Batch(vec![
                Command::SetParameter {
                    operation,
                    name: name.into(),
                    value: None,
                },
                Command::SetParameter {
                    operation,
                    name: derived,
                    value: Some(value),
                },
            ]));
        }
        self.set_parameter(operation, name, None)
    }

    /// Whether turning Adaptive spacing on for `operation` must first ask
    /// for the height: it is off and the operation holds a Scallop height of
    /// 0, which meant no scallop limit, not "no excess at all" (decision
    /// record 0239 item 5). An unknown operation needs nothing.
    pub fn adaptive_spacing_needs_height(&self, operation: OperationId) -> bool {
        use openchuck_path::adaptive::parameter::ADAPTIVE_SPACING;
        use openchuck_project::ParameterValue;
        let Some(op) = self.document.project.operation(operation) else {
            return false;
        };
        let has = self
            .strategies
            .get(&op.strategy)
            .is_ok_and(|s| s.schema().get(ADAPTIVE_SPACING).is_some());
        has && op.parameters.get(ADAPTIVE_SPACING) == Some(&ParameterValue::Flag(false))
            && matches!(op.parameters.get("scallop"), Some(ParameterValue::Number(h)) if *h == 0.0)
    }

    /// Turns Adaptive spacing on for `operation` with the Allowed excess
    /// `height`, both in one undoable step, after checking both (decision
    /// record 0239 item 5). Nothing changes when either is refused. The
    /// height stays the Scallop height if adaptive spacing is turned off
    /// again: one value serves both.
    pub fn enable_adaptive_spacing(
        &mut self,
        operation: OperationId,
        height: f64,
    ) -> Result<(), AppError> {
        use openchuck_path::adaptive::parameter::ADAPTIVE_SPACING;
        use openchuck_project::ParameterValue;
        let on = ParameterValue::Flag(true);
        let height = ParameterValue::Number(height);
        form::check_parameter(
            &self.document.project,
            &self.strategies,
            operation,
            ADAPTIVE_SPACING,
            Some(&on),
        )?;
        form::check_parameter(
            &self.document.project,
            &self.strategies,
            operation,
            "scallop",
            Some(&height),
        )?;
        self.execute(Command::Batch(vec![
            Command::SetParameter {
                operation,
                name: ADAPTIVE_SPACING.into(),
                value: Some(on),
            },
            Command::SetParameter {
                operation,
                name: "scallop".into(),
                value: Some(height),
            },
        ]))
    }

    /// Sets or clears one parameter after schema validation (UI-07).
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
        // A cutting value the user sets drops the operation's material
        // preset (issue 197; decision record 0230).
        if let Some(value) = &value
            && requirements::FROM_CUTTING_DATA.contains(&name)
            && let Some(op) = self.document.project.operation(operation)
        {
            let mut edited = op.clone();
            edited.parameters.insert(name.to_owned(), value.clone());
            preset::drop_preset(&self.document.project, &self.strategies, &mut edited);
            return self.execute(Command::Replace(Item::Operation(edited)));
        }
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

    /// What the running pipeline job is doing, in the status bar's words: the
    /// operation it is generating, or the operation whose simulation it is
    /// running again because the stock it leaves is not cached (decision
    /// record 0190). `None` while no operation is being worked on, including
    /// while operations are only taken from the cache. The job's progress is
    /// reported separately, on the event stream.
    pub fn pipeline_activity(&self) -> Option<String> {
        let (id, activity) = self.record.activity()?;
        let name = self.project().operation(id).map_or("", |o| o.name.as_str());
        Some(activity.text(name))
    }

    /// Starts regenerating and simulating every operation as a background
    /// job: the result carries a simulation report (SIM-03, SIM-05 to SIM-07).
    pub fn start_simulate(&self) -> JobHandle<Regeneration> {
        self.start_pipeline("simulate", true, None)
    }

    /// Starts generating every enabled operation's toolpath as a background
    /// job, with the stock simulation unless the project skips it (issue
    /// 214; decision record 0168): what the Generate commands do.
    pub fn start_generate(&self) -> JobHandle<Regeneration> {
        if self.simulates() {
            self.start_simulate()
        } else {
            self.start_regenerate()
        }
    }

    /// [`Application::start_generate`] up to and including `operation`
    /// ("generate selected"): the operations made are simulated unless the
    /// project skips the stock simulation.
    pub fn start_generate_until(&self, operation: OperationId) -> JobHandle<Regeneration> {
        let simulate = self.simulates();
        let name = if simulate { "simulate" } else { "regenerate" };
        self.start_pipeline(name, simulate, Some(operation))
    }

    /// Whether generating simulates the stock: unless the project skips it
    /// (decision record 0168).
    pub fn simulates(&self) -> bool {
        !self.document.project.skip_stock_simulation()
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
        let playback = self.playback_with_generation;
        self.jobs
            .submit(name, self.events_tx.clone(), move |context| {
                let result = pipeline::regenerate(
                    &document.location,
                    &document.project,
                    &strategies,
                    post.as_ref().map(|p| (p, grouping)),
                    simulate,
                    playback,
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

    /// Keeps playback's checkpoints while simulating, within `memory` bytes
    /// (the user's "Memory for playback checkpoints", capped by the front
    /// end), or none with `None`: "Generate playback information with
    /// toolpath generation" (decision record 0333). With them, playback of
    /// the run opens at once ([`Application::start_stock_playback`] uses
    /// them); without, it simulates again. The toolpaths, the G-code and the
    /// stock are the same either way. A change drops the stocks kept for
    /// reuse, so the next run simulates again and keeps them, or not.
    pub fn set_playback_with_generation(&mut self, memory: Option<usize>) {
        if self.playback_with_generation != memory {
            self.playback_with_generation = memory;
            self.cache.clear_stocks();
        }
    }

    /// The memory playback's checkpoints may hold when they are kept while
    /// simulating, or `None` when they are not.
    pub fn playback_with_generation(&self) -> Option<usize> {
        self.playback_with_generation
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

    /// The files `grouping` would write for `run` with `post`, with their
    /// names, setups and programmed times including selected-post slowdown,
    /// without writing anything (GC-05, GC-17).
    pub fn planned_files(
        &self,
        run: &Regeneration,
        post: &PostSelection,
        grouping: OutputGrouping,
    ) -> Result<Vec<PlannedFile>, AppError> {
        posting::plan_files(run, post, grouping)
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
    /// regeneration to include its original-IR programmed times. Use
    /// [`Self::write_setup_sheet_for_run`] for the selected post's adjusted
    /// times and same-run diagnostics.
    pub fn write_setup_sheet(
        &self,
        folder: &Path,
        toolpath: Option<&openchuck_types::ir::Toolpath>,
    ) -> Result<Vec<PathBuf>, AppError> {
        export::write_bytes(folder, &self.setup_sheet_files(toolpath)?)
    }

    /// Writes the same-run setup sheets, including post-adjusted durations
    /// and short-block diagnostics (GC-12, GC-17).
    pub fn write_setup_sheet_for_run(
        &self,
        folder: &Path,
        run: &Regeneration,
    ) -> Result<Vec<PathBuf>, AppError> {
        let [html, pdf] = self.output_files(run)?.setup_sheet;
        let sheet = self.setup_sheet_for_run(run);
        export::write_bytes(
            folder,
            &[(html, sheet.to_html().into_bytes()), (pdf, sheet.to_pdf())],
        )
    }

    /// The setup sheet's two file names, HTML then PDF, from the setup
    /// sheet template with the application's post (GC-12, issue 310). With
    /// the default template they are named after the project, as before.
    pub fn setup_sheet_file_names(&self) -> Result<[String; 2], AppError> {
        output_names::sheet_names(
            &self.file_names.setup_sheet,
            &self.job_facts(&self.document.project, self.post.as_ref()),
        )
        .map_err(AppError::FileNames)
    }

    /// Uses `templates` for the names of the files written from now on
    /// (issues 308 and 310). They are the user's, not the project's: the
    /// front end keeps them in the user's settings.
    pub fn set_file_name_templates(&mut self, templates: output_names::FileNameTemplates) {
        self.file_names = templates;
    }

    /// The file name templates in use.
    pub fn file_name_templates(&self) -> &output_names::FileNameTemplates {
        &self.file_names
    }

    /// What the templates' job-wide fields read for `project` and `post`.
    fn job_facts(&self, project: &Project, post: Option<&PostSelection>) -> output_names::JobFacts {
        let generated = setup_sheet::iso8601_utc(self.clock.now());
        output_names::JobFacts {
            project: project.name().to_owned(),
            machine: project
                .machine()
                .map_or_else(String::new, |m| m.name.clone()),
            post: match post {
                Some(PostSelection::Builtin(id)) => id.clone(),
                Some(PostSelection::Folder(path)) => path
                    .file_name()
                    .map_or_else(String::new, |n| n.to_string_lossy().into_owned()),
                None => String::new(),
            },
            date: generated.get(..10).unwrap_or(&generated).to_owned(),
        }
    }

    /// Every file a write of `run` makes, with the names it is written as:
    /// the program files in write order, then the setup sheet (issues 308 to
    /// 310). Two outputs whose templates give one name, letter case ignored,
    /// are told apart with a number (owner option B on OI-1072), reported
    /// in [`OutputFiles::renamed`]. Fails with the reason, before anything is
    /// written, when a template cannot name a file (NFR-07). The dialog's
    /// preview and the write both use this.
    pub fn output_files(&self, run: &Regeneration) -> Result<OutputFiles, AppError> {
        let job = self.job_facts(
            &run.project,
            run.posted_selection.as_ref().map(|(post, _)| post),
        );
        let mut programs = Vec::new();
        let mut groups = Vec::new();
        for (index, (file, contents)) in posting::program_files(run)?.into_iter().enumerate() {
            let extension = std::path::Path::new(&file.posted_name)
                .extension()
                .map_or_else(String::new, |e| e.to_string_lossy().into_owned());
            let stem = output_names::program_stem(
                &self.file_names.gcode,
                &job,
                &output_names::FileFacts {
                    index,
                    contents: &contents,
                    file: &file,
                },
            )
            .map_err(AppError::FileNames)?;
            groups.push((stem, vec![extension]));
            programs.push(file);
        }
        let sheet = output_names::sheet_stem(&self.file_names.setup_sheet, &job)
            .map_err(AppError::FileNames)?;
        groups.push((
            sheet,
            output_names::SHEET_EXTENSIONS
                .iter()
                .map(|e| (*e).to_owned())
                .collect(),
        ));
        let (mut named, renamed) = output_names::unique_names(&groups);
        let sheet_names = named.pop().unwrap_or_default();
        for (program, names) in programs.iter_mut().zip(named) {
            program.name = names.into_iter().next().unwrap_or_default();
        }
        let [html, pdf]: [String; 2] = sheet_names
            .try_into()
            .map_err(|_| AppError::FileNames("the setup sheet has no name".into()))?;
        Ok(OutputFiles {
            programs,
            setup_sheet: [html, pdf],
            renamed,
        })
    }

    fn setup_sheet_files(
        &self,
        toolpath: Option<&openchuck_types::ir::Toolpath>,
    ) -> Result<Vec<(String, Vec<u8>)>, AppError> {
        self.sheet_files(self.setup_sheet(toolpath))
    }

    fn sheet_files(&self, sheet: SetupSheet) -> Result<Vec<(String, Vec<u8>)>, AppError> {
        let [html, pdf] = self.setup_sheet_file_names()?;
        Ok(vec![
            (html, sheet.to_html().into_bytes()),
            (pdf, sheet.to_pdf()),
        ])
    }

    /// Preflights every full destination without creating anything. The
    /// returned plan freezes the same-run NC and setup-sheet bytes while
    /// existing paths are reviewed (GC-05/12/15, NFR-07).
    pub fn prepare_output(
        &self,
        folder: &Path,
        run: &std::sync::Arc<Regeneration>,
    ) -> Result<OutputPlan, AppError> {
        if run.gcode.is_empty() {
            return Err(AppError::Post(
                "There is no G-code to write. Generate the operations first.".to_owned(),
            ));
        }
        if *run.project != self.document.project {
            return Err(AppError::OutOfDate);
        }
        let (named, sources) = self.program_sources(run)?;
        let mut files: Vec<(String, Vec<u8>)> = Vec::new();
        for (program, source) in named.programs.iter().zip(sources) {
            let text = run.gcode.get(source).map(|(_, text)| text).ok_or_else(|| {
                AppError::Post(format!("no G-code for '{}'", program.posted_name))
            })?;
            files.push((program.name.clone(), text.as_bytes().to_vec()));
        }
        // The sheet lists the program files by these names and the hashes
        // of these exact bytes, so it is made from the same frozen files
        // (issue 309; decision 0189).
        let sheet = self.setup_sheet_for_files(run, &named, &files);
        let [html, pdf] = named.setup_sheet;
        files.push((html, sheet.to_html().into_bytes()));
        files.push((pdf, sheet.to_pdf()));
        let partial = export::partial_operations(&self.document.project, run);
        output_write::prepare(folder, std::sync::Arc::clone(run), files, partial)
    }

    /// The final names of the program files and, for each, the index of its
    /// text in the run: the one source of the write and the preview.
    fn program_sources(&self, run: &Regeneration) -> Result<(OutputFiles, Vec<usize>), AppError> {
        let named = self.output_files(run)?;
        let sources = named
            .programs
            .iter()
            .map(|program| {
                run.gcode
                    .iter()
                    .position(|(name, _)| *name == program.posted_name)
                    .ok_or_else(|| {
                        AppError::Post(format!("no G-code for '{}'", program.posted_name))
                    })
            })
            .collect::<Result<_, _>>()?;
        Ok((named, sources))
    }

    /// The program files exactly as [`Self::prepare_output`] would write
    /// them, for a read-only preview before anything is written (issue 312).
    /// Refused for a run the project has left behind, like the write.
    pub fn output_preview(
        &self,
        run: &std::sync::Arc<Regeneration>,
    ) -> Result<output_preview::OutputPreview, AppError> {
        if run.gcode.is_empty() {
            return Err(AppError::Post(
                "There is no G-code to preview. Generate the operations first.".to_owned(),
            ));
        }
        if *run.project != self.document.project {
            return Err(AppError::OutOfDate);
        }
        let (named, sources) = self.program_sources(run)?;
        output_preview::OutputPreview::new(
            run,
            sources
                .into_iter()
                .zip(named.programs.iter().map(|p| p.name.clone())),
        )
    }

    /// Revalidates the reviewed plan and writes in order, stopping at the
    /// first failure. `replace` confirms the plan's existing paths only.
    /// Results are retained in Messages; only full success completes Export.
    /// No atomic batch or machining verification is implied.
    pub fn write_output(&self, plan: &OutputPlan, replace: bool) -> OutputReport {
        let checks = self.export_checks(Some(&plan.run), Some(&plan.run), None);
        let report = if *plan.run.project != self.document.project {
            OutputReport::refused(
                plan.paths(),
                plan.paths().first().cloned().unwrap_or_default(),
                AppError::OutOfDate.to_string(),
            )
        } else if let Some(reason) = checks.first_blocker() {
            OutputReport::refused(
                plan.paths(),
                plan.paths().first().cloned().unwrap_or_default(),
                reason,
            )
        } else if !plan.partial_operations().is_empty() && !plan.partial_path_acknowledged() {
            OutputReport::refused(
                plan.paths(),
                plan.paths().first().cloned().unwrap_or_default(),
                export::partial_path_refusal(plan.partial_operations()),
            )
        } else {
            output_write::write(plan, replace)
        };
        if report.is_complete() {
            self.record_export(&plan.run);
        }
        self.record_output_report(report.clone());
        report
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

    /// The project's setup sheet with its traceability block (GC-12,
    /// decision 0110). Pass a regeneration's toolpath to include each
    /// operation's original-IR programmed time. This is a toolpath-only
    /// preview: it does not apply the selected post's Slow down mode. Use
    /// [`Self::setup_sheet_for_run`] for posted feeds and adjusted times.
    pub fn setup_sheet(&self, toolpath: Option<&openchuck_types::ir::Toolpath>) -> SetupSheet {
        // Each setup's part as the setup places it, for the tabs placed
        // automatically (decision record 0202); a part that cannot be read
        // leaves them where they were stored.
        let part = |setup: &openchuck_project::Setup| {
            let model = setup.part?;
            let extent = self.inspect_model(model).ok()?.extent?;
            Some(extent.in_setup(&setup.frame))
        };
        let options = self
            .post
            .as_ref()
            .map(PostSelection::load)
            .transpose()
            .map(|post| post.map(|post| post.options))
            .map_err(|error| error.to_string());
        SetupSheet::with_parts(&self.document.project, &self.strategies, toolpath, &part)
            .with_block_time(&self.document.project, toolpath, options)
            .with_program_files(&[])
            .with_traceability(&self.traceability())
    }

    /// The selected post's same-run setup sheet, including its adjusted
    /// programmed times and controller diagnostics (GC-12, GC-17), and the
    /// program files it is written with: their names from the templates,
    /// what each holds in order and the hash of each file's bytes (issue
    /// 309). A run with no program files gives a sheet that says so.
    pub fn setup_sheet_for_run(&self, run: &Regeneration) -> SetupSheet {
        match self.output_files(run) {
            Ok(named) => {
                let files: Vec<(String, Vec<u8>)> = named
                    .programs
                    .iter()
                    .map(|p| {
                        let bytes = run
                            .gcode
                            .iter()
                            .find(|(name, _)| *name == p.posted_name)
                            .map_or_else(Vec::new, |(_, text)| text.as_bytes().to_vec());
                        (p.name.clone(), bytes)
                    })
                    .collect();
                self.setup_sheet_for_files(run, &named, &files)
            }
            Err(error) => self
                .setup_sheet_without_files(run)
                .with_unnamed_program_files(&error.to_string())
                .with_traceability(&self.traceability()),
        }
    }

    /// The same-run sheet listing `named`'s program files with the hashes of
    /// `files`, the exact bytes that are written under those names.
    fn setup_sheet_for_files(
        &self,
        run: &Regeneration,
        named: &OutputFiles,
        files: &[(String, Vec<u8>)],
    ) -> SetupSheet {
        let listed: Vec<(&output_names::ProgramFile, &[u8])> = named
            .programs
            .iter()
            .map(|program| {
                let bytes = files
                    .iter()
                    .find(|(name, _)| *name == program.name)
                    .map_or(&[][..], |(_, bytes)| bytes.as_slice());
                (program, bytes)
            })
            .collect();
        self.setup_sheet_without_files(run)
            .with_program_files(&listed)
            .with_traceability(&self.traceability())
    }

    fn setup_sheet_without_files(&self, run: &Regeneration) -> SetupSheet {
        let cancel = openchuck_types::job::CancelToken::new();
        let context =
            openchuck_types::job::JobContext::new(&cancel, &openchuck_types::job::NoProgress);
        let part = |setup: &openchuck_project::Setup| {
            let model = setup.part?;
            let reference = run
                .project
                .models()
                .iter()
                .find(|reference| reference.id == model)?;
            let extent = model::inspect_model(&self.document.location, reference, &context)
                .ok()?
                .extent?;
            Some(extent.in_setup(&setup.frame))
        };
        let mut sheet = SetupSheet::with_parts(
            &run.project,
            &self.strategies,
            Some(run.posted_toolpath.as_ref().unwrap_or(&run.toolpath)),
            &part,
        )
        .with_short_blocks(&run.project, &run.short_blocks);
        if let Some((settings, sources)) = run.effective_short_blocks {
            sheet = sheet.with_short_block_configuration(settings, sources);
        }
        sheet
    }

    /// Replaces the clock a setup sheet reads its generation time from, so a
    /// test gets the same sheet every run (decision 0110 item 2).
    pub fn set_clock(&mut self, clock: Box<dyn Clock>) {
        self.clock = clock;
    }

    /// What ties a setup sheet to the files it came from (GC-12, decision
    /// 0110): the time now, when the project files were last written, this
    /// software's version, a fingerprint of those files and the version
    /// control line. Nothing is stored in the project, so the project files
    /// stay byte-stable (PRJ-01).
    ///
    /// The files read are the ones the project was last opened or saved with,
    /// so the fingerprint names the saved project even when the project has
    /// changed since; the saved line says so.
    pub fn traceability(&self) -> Traceability {
        let saved_project = self
            .saved
            .lock()
            .map_or_else(|e| e.into_inner().clone(), |saved| saved.clone());
        let (saved, fingerprint) = saved_files_trace(&self.document.location, &saved_project)
            .map_or_else(
                || {
                    (
                        Traceability::NOT_SAVED.to_owned(),
                        Traceability::NOT_SAVED.to_owned(),
                    )
                },
                |(time, hash)| {
                    let time = setup_sheet::iso8601_utc(time);
                    let saved = if self.is_modified() {
                        format!("{time} (changes made since are not saved)")
                    } else {
                        time
                    };
                    (saved, hash)
                },
            );
        Traceability {
            generated: setup_sheet::iso8601_utc(self.clock.now()),
            saved,
            version: env!("CARGO_PKG_VERSION").to_owned(),
            fingerprint,
            version_control: Traceability::NOT_TRACKED.to_owned(),
        }
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
