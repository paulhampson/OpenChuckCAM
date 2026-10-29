// SPDX-License-Identifier: AGPL-3.0-or-later
//! The application shell: the window's regions, the workflow steps and the
//! footer's primary action, the project tree with its marks, the dock, new,
//! open and save with unsaved-change handling, undo and redo, and the
//! viewport (UI-01, UI-06, UI-08 to UI-12, UI-15, UI-16; work breakdown slice
//! 8 items 3, 4, 6, 8, 11 and 12; decision records 0029 and 0117;
//! specification sections 3 to 6, 9, 10, 11.1 and 13).
//!
//! Every project change goes through `openchuck-app`; the shell holds no CAM
//! logic. What it shows about the state of the job (steps, marks, messages,
//! setup checks, the primary action) is the application's
//! [`Overview`](openchuck_app::overview::Overview). Paths can be typed, or
//! picked with the platform's own dialog through `rfd` (OI-127); the typed
//! path stays when no dialog is available.
//!
//! The regions are drawn by the submodules: [`frame`] (title bar, workflow
//! bar, footer, empty state, panel layout and the per-user settings),
//! [`tree`] (the project panel) and [`dock`] (Messages, Selection sets, Jobs,
//! Setup checks and Playback).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use crate::theme::DARK;
use eframe::egui::{self, Key, KeyboardShortcut, Modifiers};
use openchuck_app::new_folder;
use openchuck_app::overview::{
    Checks, ItemRef, Message, MessageAction, Overview, PrimaryKind, SetupPart, Severity, Step,
};
use openchuck_app::project::ItemId;
use openchuck_app::project::id::{ModelId, OperationId, SelectionSetId, SetupId};
use openchuck_app::scene::SceneView;
use openchuck_app::settings::{SettingsFile, UserSettings};
use openchuck_app::{AppError, Application, Command, JobHandle, JobLog, Threads};

use crate::about;
use crate::bench;
use crate::export_panel::ExportPanel;
use crate::form_panel::FormState;
use crate::import_panel::{ImportAction, ImportSession};
use crate::machine_panel::MachinePanel;
use crate::model_panel::ModelPanel;
use crate::playback_panel::PlaybackPanel;
use crate::results_panel::ResultsPanel;
use crate::run_panel::RunPanel;
use crate::selection_panel::SelectionPanel;
use crate::tool_library_window::{Chosen, Purpose, ToolLibraryWindow};
use crate::viewport::Viewport;
use crate::workholding_panel::WorkholdingPanel;

mod actions;
mod checkpoints;
mod dock;
mod frame;
mod inspector;
pub(crate) mod marks;
mod out_of_date;
mod pending;
mod pending_pick;
mod tree;

#[cfg(test)]
mod checkpoint_tests;
#[cfg(test)]
mod pending_edit_tests;

use dock::DockTab;
pub(crate) use frame::{chip, primary_button};
pub(crate) use marks::{mark, tick};

const SAVE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::S);
const UNDO: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Z);
const REDO: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::Z);
const REDO_ALT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Y);
const NEW: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::N);
const OPEN: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::O);
const IMPORT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::I);
const EXPORT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::E);
const GENERATE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::NONE, Key::G);
const GENERATE_F5: KeyboardShortcut = KeyboardShortcut::new(Modifiers::NONE, Key::F5);
const GENERATE_STALE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::SHIFT, Key::G);
const GENERATE_STALE_F5: KeyboardShortcut = KeyboardShortcut::new(Modifiers::SHIFT, Key::F5);
const NEW_OPERATION: KeyboardShortcut = KeyboardShortcut::new(Modifiers::NONE, Key::N);
const DUPLICATE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::D);
const DELETE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::NONE, Key::Delete);
const RENAME: KeyboardShortcut = KeyboardShortcut::new(Modifiers::NONE, Key::F2);
const EARLIER: KeyboardShortcut = KeyboardShortcut::new(Modifiers::ALT, Key::ArrowUp);
const LATER: KeyboardShortcut = KeyboardShortcut::new(Modifiers::ALT, Key::ArrowDown);
/// Alt 1 to Alt 4 open the steps (specification section 4).
const STEP_KEYS: [Key; 4] = [Key::Num1, Key::Num2, Key::Num3, Key::Num4];
/// Alt 5 opens Post and export as Ctrl E does, for users of the fifth step
/// there was before (decision record 0282 item 4).
const EXPORT_ALT: Key = Key::Num5;
/// Ctrl 1, 2 and 3 collapse the project panel, the inspector and the dock.
const PANEL_KEYS: [Key; 3] = [Key::Num1, Key::Num2, Key::Num3];

/// Work a dialog hands back to the shell once it closes.
type Action = Box<dyn FnOnce(&mut Shell)>;

/// What to do once unsaved changes are dealt with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Next {
    New,
    Open,
    Quit,
    Recovery,
}

/// A dialog in front of the window.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Dialog {
    /// Ask for a folder (and a name, for a new project).
    Folder {
        create: bool,
        folder: String,
        name: String,
    },
    /// The folder a new project would get already exists and holds none of a
    /// project's files: ask whether to use it or change the name (#219,
    /// decision record 0165). Nothing in it is changed or deleted.
    FolderExists {
        parent: String,
        name: String,
        folder: PathBuf,
    },
    /// The folder has no project: offer to create one there (decision record
    /// 0008 item 8).
    OfferCreate { folder: PathBuf },
    /// The project has unsaved changes.
    Unsaved(Next),
    /// Rename an item (F2).
    Rename { item: ItemRef, name: String },
    /// The keyboard map (Help).
    Keys,
    /// About the program (Help).
    About,
    /// The settings the program keeps for the user on this computer: what is
    /// recorded in the log (NFR-18).
    Settings,
    /// A command the user gave failed outright; the message says what
    /// (specification section 9: the only pop-up errors).
    Error(String),
}

/// The shell's state.
pub struct Shell {
    recovery: checkpoints::State,
    recovery_project: Option<(Application, Vec<String>, bool)>,
    edit_context: Option<egui::Context>,
    edit_guard: pending::Guard,
    app: Option<Application>,
    /// The window title last sent to the system (#218).
    sent_title: String,
    /// The setting the inspector brings into view on its next frame: a
    /// message's "Show setting" link (decision record 0175).
    setting_jump: Option<(OperationId, String)>,
    viewport: Viewport,
    dialog: Option<Dialog>,
    scene_job: Option<(JobHandle<SceneView>, bool)>,
    setup: Option<SetupId>,
    status: String,
    quitting: bool,
    /// A frame-time benchmark run (`--bench`).
    bench: Option<bench::Run>,
    /// The item shown in the inspector.
    selected: Option<Selection>,
    /// The section of the setup inspector a setup's child row chose.
    setup_section: Option<SetupPart>,
    /// The model's child row chosen in the tree, for its highlight
    /// (issue #181).
    model_section: Option<&'static str>,
    /// The missing value the "Show first" jump is taking the user to, until
    /// the inspector has scrolled to it.
    requirement_jump: Option<openchuck_app::requirements::RequirementField>,
    /// The tool library window (specification 12.1).
    tool_library: ToolLibraryWindow,
    /// A model being imported; the viewport shows its preview meanwhile.
    import: Option<ImportSession>,
    /// The chosen model's panel, with its running check.
    model_panel: Option<ModelPanel>,
    /// The machine panel, with the profiles found when it was opened.
    machine_panel: Option<MachinePanel>,
    /// The chosen setup's workholding and tabs (SET-09, OP-21).
    workholding_panel: Option<WorkholdingPanel>,
    /// Typed text and refused values in the inspector's fields.
    form: FormState,
    /// The setup row whose committed value the pending command applies.
    pending_field: Option<String>,
    /// Picking triangles and the selection sets (slice 10).
    selection: SelectionPanel,
    /// Adding operations and generating (slice 9 item 7).
    run: RunPanel,
    /// Post and export (slice 19 item 4).
    export: ExportPanel,
    /// The export dialog was opened from Simulate's "Post and export…", so
    /// the keyboard goes back to it when the dialog closes (UI-14; decision
    /// record 0282 item 8).
    export_from_footer: bool,
    /// A text field had the keyboard at the end of the last frame.
    text_had_focus: bool,
    /// The toolpath display and playback (slice 9).
    play: PlaybackPanel,
    /// The Simulate camera to restore while its run remains available.
    simulate_camera: Option<(Arc<openchuck_app::Regeneration>, openchuck_viewport::Camera)>,
    /// The results inspector of the Simulate step (SIM-05, SIM-07).
    results: ResultsPanel,
    /// The workflow step open (UI-10).
    step: Step,
    /// The jobs seen on the event stream (the Jobs tab).
    jobs: JobLog,
    /// The latest project checks, and the job making the next.
    checks: Option<Arc<Checks>>,
    checks_job: Option<JobHandle<Checks>>,
    /// The state of the job as shown, and whether it needs working out
    /// again.
    overview: Overview,
    overview_dirty: bool,
    /// The per-user settings (UI-15) and where they are kept.
    settings: UserSettings,
    settings_file: Option<SettingsFile>,
    /// When the settings changed and were not yet written.
    settings_changed: Option<Instant>,
    /// The dock was drawn at a size other than the one asked for (clamped by a small window
    /// that has since grown): the next frame asks for the saved size again (OI-359).
    dock_stale: bool,
    /// A problem reading the settings, shown as a message.
    settings_problem: Option<String>,
    /// The dock's tab, and the message severities shown.
    dock_tab: DockTab,
    message_filter: [bool; 3],
    /// The message the disabled primary action points at.
    highlight: Option<usize>,
    /// Panels collapsed by the small-window rule, not by the user.
    auto_collapsed: frame::AutoCollapse,
    /// A panel whose splitter was double-clicked: its size resets.
    reset: Option<frame::Region>,
    /// What the user-action log last saw (NFR-18).
    recorder: actions::Recorder,
    /// Whether the latest run still matches the project (issue 260).
    out_of_date: out_of_date::OutOfDate,
    /// The selection panel's count of sets shown, and the selection the set
    /// shown last was shown for (issue 261).
    shown_for: (u64, Option<Selection>),
}

impl Shell {
    /// A shell with no project open. `viewport_available` is false when
    /// eframe runs without wgpu. The per-user settings are read from the
    /// configuration folder.
    pub fn new(viewport_available: bool, folder: Option<PathBuf>) -> Self {
        Self::with_settings(viewport_available, folder, SettingsFile::default_location())
    }

    /// A shell whose settings live in `settings` (none: defaults, never
    /// written).
    pub fn with_settings(
        viewport_available: bool,
        folder: Option<PathBuf>,
        settings: Option<SettingsFile>,
    ) -> Self {
        let (loaded, problem) = match &settings {
            Some(file) => {
                let loaded = file.load();
                (loaded.settings, loaded.problem)
            }
            None => (UserSettings::default(), None),
        };
        let mut shell = Self {
            recovery: checkpoints::State::new(
                settings
                    .as_ref()
                    .and_then(|s| s.file().parent())
                    .map(Path::to_owned),
            ),
            recovery_project: None,
            edit_context: None,
            edit_guard: pending::Guard::default(),
            app: None,
            sent_title: String::new(),
            setting_jump: None,
            viewport: Viewport::new(viewport_available),
            dialog: None,
            scene_job: None,
            setup: None,
            status: String::new(),
            quitting: false,
            bench: None,
            selected: None,
            setup_section: None,
            model_section: None,
            requirement_jump: None,
            tool_library: ToolLibraryWindow::default(),
            import: None,
            model_panel: None,
            machine_panel: None,
            workholding_panel: None,
            form: FormState::default(),
            pending_field: None,
            selection: SelectionPanel::default(),
            run: RunPanel::default(),
            results: ResultsPanel::default(),
            export: ExportPanel::default(),
            export_from_footer: false,
            text_had_focus: false,
            play: PlaybackPanel::default(),
            simulate_camera: None,
            step: Step::Model,
            jobs: JobLog::default(),
            checks: None,
            checks_job: None,
            overview: Overview::no_project(),
            overview_dirty: true,
            settings: loaded,
            settings_file: settings,
            settings_changed: None,
            dock_stale: false,
            settings_problem: problem,
            dock_tab: DockTab::Messages,
            message_filter: [true; 3],
            highlight: None,
            auto_collapsed: frame::AutoCollapse::default(),
            reset: None,
            recorder: actions::Recorder::default(),
            out_of_date: out_of_date::OutOfDate::default(),
            shown_for: (0, None),
        };
        crate::logging::set_recording(shell.settings.record_actions);
        shell.apply_simulate_settings();
        if let Some(folder) = folder {
            shell.open(folder);
        }
        shell
    }

    /// Gives the playback panel and the view the user's Simulate choices
    /// (decision records 0168 and 0169).
    fn apply_simulate_settings(&mut self) {
        let simulate = self.settings.simulate;
        self.viewport.set_cut_part_solid(simulate.cut_part_solid);
        self.play.set_turn_part_with_a(simulate.turn_part_with_a);
        self.play
            .set_hide_outside_window(simulate.hide_outside_window);
        self.viewport
            .set_simulate_show(crate::viewport::SimulateShow {
                model: simulate.show_model,
                cut_part: simulate.show_cut_part,
                toolpath: simulate.show_toolpath,
            });
    }

    /// Keeps the Simulate choices the user made in the settings, so they are
    /// remembered between runs (issues 203, 204, 205).
    fn keep_simulate_settings(&mut self) {
        let show = self.viewport.simulate_show();
        let now = openchuck_app::settings::SimulateSettings {
            cut_part_solid: self.viewport.cut_part_solid(),
            turn_part_with_a: self.play.turns_part_with_a(),
            show_model: show.model,
            show_cut_part: show.cut_part,
            show_toolpath: show.toolpath,
            hide_outside_window: self.play.hides_outside_window(),
            ..self.settings.simulate
        };
        if now != self.settings.simulate {
            self.settings.simulate = now;
            self.settings_touched();
        }
    }

    /// The interface scale the settings keep, for the window's first frame.
    pub fn scale(&self) -> f32 {
        self.settings.scale
    }

    /// A shell running the frame-time benchmark on `app`'s scene.
    pub fn benchmark(app: Application, run: bench::Run) -> Self {
        let mut shell = Self::with_settings(true, None, None);
        shell.viewport.mode = run.mode();
        shell.bench = Some(run);
        shell.set_app(app);
        shell
    }

    /// Advances a benchmark run by one frame.
    fn bench_frame(&mut self, ctx: &egui::Context) {
        let Some(run) = &mut self.bench else { return };
        let (triangles, feature_edges) = self.viewport.counts();
        let state = bench::SceneState {
            ready: self.viewport.is_drawn(),
            camera: &self.viewport.camera,
            triangles,
            feature_edges,
            pixels: self.viewport.pixels(),
        };
        let (step, camera) = run.frame(state);
        if let Some(camera) = camera {
            self.viewport.camera = camera;
        }
        match step {
            bench::Step::Continue => ctx.request_repaint(),
            bench::Step::Finish => {
                self.quitting = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    fn open(&mut self, folder: PathBuf) {
        match Application::open(&folder, Threads::All) {
            Ok(app) => self.set_app(app),
            Err(AppError::NoProject(_)) => self.dialog = Some(Dialog::OfferCreate { folder }),
            Err(e) => {
                self.dialog = Some(Dialog::Error(format!(
                    "The project could not be opened: {e}"
                )));
            }
        }
    }

    fn create(&mut self, folder: PathBuf, name: &str) {
        let defaults = openchuck_app::NewProject {
            skip_stock_simulation: self.settings.simulate.skip_stock_simulation,
        };
        match Application::create_with(&folder, name, Threads::All, defaults) {
            Ok(app) => self.set_app(app),
            Err(e) => {
                self.dialog = Some(Dialog::Error(format!(
                    "The project could not be created: {e}"
                )));
            }
        }
    }

    fn set_app(&mut self, app: Application) {
        self.recovery.attach(
            app.location(),
            self.app.as_ref().map_or(0, Application::revision),
            self.settings.checkpoint_minutes,
        );
        if let Some(ctx) = &self.edit_context {
            crate::pending_edits::clear(ctx);
        }
        self.status = format!("Opened {}", app.location().display());
        // Changed or missing model files are messages now (PRJ-02, OI-45;
        // specification section 9), from the project checks.
        self.selection.clear();
        self.run.clear();
        self.export.clear();
        self.play.clear();
        self.simulate_camera = None;
        self.jobs.clear();
        self.checks = None;
        if let Some(job) = self.checks_job.take() {
            job.cancel();
        }
        self.setup = app.project().setups().first().map(|s| s.id);
        self.selected = None;
        self.setup_section = None;
        // A workholding draft, and its "not applied" note, belong to the
        // project that was open; they must not carry over (lane 21d, #119).
        self.workholding_panel = None;
        self.machine_panel = None;
        self.import = None;
        self.form.clear();
        self.highlight = None;
        if self.bench.is_none() {
            self.settings.opened(app.location());
            self.settings_touched();
        }
        self.step = if app.project().models().is_empty() {
            Step::Model
        } else if app.project().operations().is_empty() {
            Step::Setup
        } else {
            Step::Operations
        };
        self.app = Some(app);
        self.overview_dirty = true;
        self.rebuild_scene(true);
    }

    /// Starts building the scene again after the project or the chosen setup changed.
    fn rebuild_scene(&mut self, mut fit: bool) {
        if let Some(app) = &self.app {
            if let Some((old, pending_fit)) = self.scene_job.take() {
                old.cancel();
                fit |= pending_fit;
            }
            let setup = self
                .setup
                .filter(|id| app.project().setups().iter().any(|s| s.id == *id));
            self.scene_job = Some((
                app.start_scene_with_facing(setup, self.run.last().cloned()),
                fit,
            ));
        }
    }

    fn poll_scene(&mut self, ctx: &egui::Context) {
        let Some((job, fit)) = &self.scene_job else {
            return;
        };
        match job.try_result() {
            None => ctx.request_repaint(),
            Some(result) => {
                let fit = *fit;
                self.scene_job = None;
                match result {
                    // While importing, the viewport shows the import preview.
                    Ok(_) if self.import.is_some() => {}
                    Ok(view) => {
                        self.viewport.set_scene(view, fit);
                        self.selection.scene_changed();
                        self.overview_dirty = true;
                    }
                    // A failure is a message, from the job log.
                    Err(_) => self.overview_dirty = true,
                }
            }
        }
    }

    /// Reads the events into the job log, keeps the project checks going and
    /// works the overview out again when anything it reads changed.
    fn overview_frame(&mut self, ctx: &egui::Context) {
        let Some(app) = &self.app else {
            if self.overview_dirty {
                self.overview = Overview::no_project();
                if let Some(problem) = &self.recovery.problem {
                    self.overview.push(Message::new(
                        Severity::Warning,
                        problem.clone(),
                        None,
                        None,
                    ));
                }
                self.overview_dirty = false;
            }
            return;
        };
        let events = app.drain_events();
        if !events.is_empty() {
            // Only the jobs the log lists change an item's state or a
            // message; the quick ones (the stock at a playback move, a region
            // pick) run most frames and must not rebuild the overview.
            let listed = events.iter().any(|e| match e {
                openchuck_app::Event::JobStarted { name, .. } => {
                    openchuck_app::job_label(name).is_some()
                }
                openchuck_app::Event::JobProgress { job, .. }
                | openchuck_app::Event::JobFinished { job, .. } => self.jobs.get(*job).is_some(),
                _ => true,
            });
            self.recorder.record_jobs(&events);
            self.jobs.update(events);
            self.overview_dirty |= listed;
        }
        if let Some(job) = &self.checks_job
            && let Some(result) = job.try_result()
        {
            self.checks_job = None;
            if let Ok(checks) = result {
                self.checks = Some(Arc::new(checks));
            }
            self.overview_dirty = true;
        }
        let revision = app.revision();
        let behind = self
            .checks
            .as_ref()
            .is_none_or(|c| c.revision() != revision);
        if behind && self.checks_job.is_none() {
            self.checks_job = Some(app.start_checks(self.checks.clone()));
        }
        if self.checks_job.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        let geometry_changed = self.overview.revision != revision;
        if self.overview_dirty || geometry_changed {
            let mut overview = app.overview(self.checks.as_deref(), &self.jobs);
            for note in &self.viewport.view().notes {
                overview.push(Message::new(Severity::Info, note.clone(), None, None));
            }
            if let Some(problem) = &self.settings_problem {
                overview.push(Message::new(Severity::Warning, problem.clone(), None, None));
            }
            if let Some(problem) = &self.recovery.problem {
                overview.push(Message::new(Severity::Warning, problem.clone(), None, None));
            }
            self.overview = overview;
            self.overview_dirty = false;
            self.highlight = None;
        }
        if geometry_changed {
            self.rebuild_scene(false);
        }
    }

    /// Generates the selected operation, up to and including it, when an
    /// operation is selected (G, F5); otherwise every stale operation
    /// (specification section 16; decision record 0117, OI-351). A turned-off
    /// operation goes to the run too, which refuses it and says why (OI-352).
    fn generate_selected(&mut self) {
        if self.defer(Self::generate_selected) {
            return;
        }
        let Some(app) = &self.app else { return };
        match self.selected {
            Some(Selection::Operation(id)) => {
                self.run.start_until(app, id);
                self.overview_dirty = true;
            }
            _ => self.generate(false),
        }
    }

    /// Starts generating, and simulating when asked (UI-09).
    fn generate(&mut self, simulate: bool) {
        if self.defer(move |s| s.generate(simulate)) {
            return;
        }
        if let Some(app) = &self.app {
            self.run.start(app, simulate);
            self.overview_dirty = true;
        }
    }

    fn save(&mut self) -> bool {
        if self.defer(|s| {
            s.save();
        }) {
            return false;
        }
        let Some(app) = &self.app else { return true };
        match app.save() {
            Ok(()) => {
                self.recovery.saved(app.revision());
                self.status = format!("Saved {}", app.location().display());
                true
            }
            Err(e) => {
                self.dialog = Some(Dialog::Error(format!("The project was not saved: {e}")));
                false
            }
        }
    }

    /// Turns the recording of the user's actions in the log on or off, and
    /// keeps the choice in the settings (NFR-18).
    fn set_record_actions(&mut self, on: bool) {
        self.settings.record_actions = on;
        crate::logging::set_recording(on);
        self.settings_touched();
    }

    fn undo(&mut self) {
        if self.defer(Self::undo) {
            return;
        }
        if let Some(app) = &mut self.app {
            match app.undo() {
                Ok(true) => {
                    self.form.clear();
                    self.rebuild_scene(false);
                }
                Ok(false) => {}
                Err(e) => self.dialog = Some(Dialog::Error(e.to_string())),
            }
        }
    }

    fn redo(&mut self) {
        if self.defer(Self::redo) {
            return;
        }
        if let Some(app) = &mut self.app {
            match app.redo() {
                Ok(true) => {
                    self.form.clear();
                    self.rebuild_scene(false);
                }
                Ok(false) => {}
                Err(e) => self.dialog = Some(Dialog::Error(e.to_string())),
            }
        }
    }

    /// Runs `next`, first asking about unsaved changes.
    fn request(&mut self, next: Next, ctx: &egui::Context) {
        let deferred_ctx = ctx.clone();
        if self.defer(move |s| s.request(next, &deferred_ctx)) {
            return;
        }
        if self.app.as_ref().is_some_and(Application::is_modified) {
            self.dialog = Some(Dialog::Unsaved(next));
        } else {
            self.proceed(next, ctx);
        }
    }

    fn proceed(&mut self, next: Next, ctx: &egui::Context) {
        if next == Next::Quit
            && self
                .recovery
                .close(next, self.app.as_ref().map_or(0, Application::revision))
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.request_repaint();
            return;
        }
        match next {
            Next::New | Next::Open => {
                self.dialog = Some(Dialog::Folder {
                    create: next == Next::New,
                    folder: String::new(),
                    name: "Untitled".to_owned(),
                });
            }
            Next::Quit => {
                self.save_settings();
                self.quitting = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Next::Recovery => {
                if let Some((app, warnings, recovered)) = self.recovery_project.take() {
                    self.set_app(app);
                    self.recovery.finish_install();
                    if recovered {
                        self.status =
                            "Recovered checkpoint. Save explicitly to keep these changes.".into();
                    }
                    if !warnings.is_empty() {
                        self.recovery.problem = Some(warnings.join("\n"));
                        self.overview_dirty = true;
                    }
                }
            }
        }
    }

    /// Opens the import dialog (Ctrl I, the Model "+", the workflow bar).
    fn import_model(&mut self) {
        if self.defer(Self::import_model) {
            return;
        }
        if self.app.is_some() && self.import.is_none() {
            self.import = Some(ImportSession::blank());
        }
    }

    /// Opens the tool library window (specification 12.1) for `purpose`,
    /// with `tool` chosen when given.
    pub(super) fn open_tool_library(&mut self, purpose: Purpose, tool: Option<Chosen>) {
        if self.defer(move |s| s.open_tool_library(purpose, tool)) {
            return;
        }
        self.tool_library.open(purpose, tool);
    }

    /// Opens the new-operation dialog on the Operations step (N, the
    /// Operations "+", the workflow bar).
    fn new_operation(&mut self) {
        if self.defer(Self::new_operation) {
            return;
        }
        if let Some(app) = &self.app
            && self.overview.step(Step::Operations).is_available()
        {
            self.step = Step::Operations;
            let from = match self.selected {
                Some(Selection::Operation(id)) => Some(id),
                _ => None,
            };
            let set = match self.selected {
                Some(Selection::Set(id)) => Some(id),
                _ => self.selection.shown_set(),
            };
            self.run.begin_add(app, from, set);
        }
    }

    /// Creates the project's one setup as a single undoable command, selects
    /// it and opens the Setup step (the Setups "+", the workflow bar;
    /// decision record 0144). With a setup already there, or no model yet, it
    /// does nothing.
    fn add_setup(&mut self) {
        if self.defer(Self::add_setup) {
            return;
        }
        let Some(app) = &mut self.app else { return };
        if !openchuck_app::can_add_setup(app.project()) {
            return;
        }
        let result = openchuck_app::new_setup(app.project()).and_then(|c| app.execute(c));
        if let Err(e) = result {
            self.dialog = Some(Dialog::Error(format!("The setup was not added: {e}")));
            return;
        }
        let Some(id) = app.project().setups().first().map(|s| s.id) else {
            return;
        };
        self.open_step(Step::Setup);
        self.choose_setup(id, None);
    }

    /// Starts picking on the part with an empty working selection (the
    /// Selection sets "+", decision record 0037 item 7).
    fn start_picking(&mut self) {
        if self.defer(Self::start_picking) {
            return;
        }
        if self.app.is_some() {
            if self.overview.step(Step::Operations).is_available() {
                self.step = Step::Operations;
            }
            self.selection.show_set(None);
            // New selection is in the inspector (12.2; decision 0215).
            self.settings.inspector.collapsed = false;
            self.select(Selection::Picking);
        }
    }

    /// Opens `step` if it is available (UI-10).
    fn open_step(&mut self, step: Step) {
        if step != self.step && self.defer(move |s| s.open_step(step)) {
            return;
        }
        if self.overview.step(step).is_available() {
            if self.step == Step::Simulate && step != Step::Simulate {
                self.play.pause();
                self.simulate_camera = self
                    .run
                    .last()
                    .filter(|_| self.play.has_run())
                    .map(|run| (Arc::clone(run), self.viewport.camera));
            } else if step == Step::Simulate
                && self.play.has_run()
                && let Some((run, camera)) = &self.simulate_camera
                && self
                    .run
                    .last()
                    .is_some_and(|current| Arc::ptr_eq(current, run))
            {
                self.viewport.camera = *camera;
            }
            // Another step ends selection mode, and its highlight with it
            // (issue 261).
            if step != self.step
                && matches!(self.selected, Some(Selection::Picking | Selection::Set(_)))
            {
                self.selected = None;
            }
            self.step = step;
            if step == Step::Simulate {
                self.play.open = true;
                self.dock_tab = DockTab::Playback;
            } else if self.dock_tab == DockTab::Playback {
                self.dock_tab = DockTab::Messages;
            }
            if step == Step::Setup {
                self.dock_tab = DockTab::SetupChecks;
            } else if self.dock_tab == DockTab::SetupChecks {
                self.dock_tab = DockTab::Messages;
            }
        }
    }

    /// Does what the footer's primary button says (specification section 5).
    fn primary(&mut self) {
        if self.defer(Self::primary) {
            return;
        }
        self.act(self.overview.primary(self.step));
    }

    /// Simulate's simulation action, in its own place on the footer
    /// (decision record 0282 item 2).
    fn simulation(&mut self) {
        if self.defer(Self::simulation) {
            return;
        }
        self.act(self.overview.simulation_action());
    }

    /// Simulate's "Post and export…" (decision record 0282 items 2 and 3):
    /// the old fifth step's button, with its rules.
    fn post_and_export(&mut self) {
        if self.defer(Self::post_and_export) {
            return;
        }
        self.act(self.overview.export_action());
    }

    /// Does `action`, a footer action, unless it is disabled or a job runs:
    /// then shows the message that disables it, if any.
    fn act(&mut self, action: openchuck_app::overview::PrimaryAction) {
        if action.blocked.is_some() || self.run.is_running() {
            self.highlight = action.message;
            if action.message.is_some() {
                self.dock_tab = DockTab::Messages;
                self.settings.dock.collapsed = false;
            }
            return;
        }
        match action.kind {
            PrimaryKind::ImportModel => self.import_model(),
            PrimaryKind::Continue(step) => self.open_step(step),
            PrimaryKind::GenerateStale(_) | PrimaryKind::Generate => self.generate(false),
            PrimaryKind::RunSimulation | PrimaryKind::GenerateAndSimulate(_) => {
                self.generate(true);
            }
            PrimaryKind::OpenExport => self.open_export(),
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        // Global shortcuts run before widgets. Leave input to the modal
        // shown last frame, including export and tool-library confirmations.
        if self.dialog.is_some()
            || self.edit_guard.active()
            || ctx.memory(|memory| memory.top_modal_layer().is_some())
        {
            return;
        }
        let (save, undo, redo) = ctx.input_mut(edit_shortcuts);
        if save {
            self.save();
        }
        if undo {
            self.undo();
        }
        if redo {
            self.redo();
        }
        let (new, open, import, export) = ctx.input_mut(|i| {
            (
                i.consume_shortcut(&NEW),
                i.consume_shortcut(&OPEN),
                i.consume_shortcut(&IMPORT),
                i.consume_shortcut(&EXPORT),
            )
        });
        if new {
            self.request(Next::New, ctx);
        }
        if open {
            self.request(Next::Open, ctx);
        }
        if import {
            self.import_model();
        }
        if export {
            self.open_export();
        }
        // Alt 1 to 4: the steps; Alt 5: Post and export. Ctrl 1 to 3:
        // collapse the panels.
        for (index, key) in STEP_KEYS.into_iter().enumerate() {
            if ctx.input_mut(|i| i.consume_key(Modifiers::ALT, key))
                && let Some(step) = Step::ALL.get(index)
            {
                self.open_step(*step);
            }
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::ALT, EXPORT_ALT)) {
            self.open_export();
        }
        for (key, region) in PANEL_KEYS.into_iter().zip(frame::Region::ALL) {
            if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, key)) {
                self.toggle_region(region);
            }
        }
        // Keys that would type a letter are left to a text field with the
        // keyboard (a modal dialog returned above). A floating window such as
        // the results, which opens after each run, does not take them.
        if self.text_had_focus {
            return;
        }
        let (stale, generate, new_operation) = ctx.input_mut(|i| {
            // Shift first: Shift G would otherwise be taken for G.
            let stale =
                i.consume_shortcut(&GENERATE_STALE) || i.consume_shortcut(&GENERATE_STALE_F5);
            let generate = i.consume_shortcut(&GENERATE) || i.consume_shortcut(&GENERATE_F5);
            (stale, generate, i.consume_shortcut(&NEW_OPERATION))
        });
        if stale {
            self.generate(false);
        } else if generate {
            self.generate_selected();
        }
        if new_operation {
            self.new_operation();
        }
        let (duplicate, delete, rename, earlier, later) = ctx.input_mut(|i| {
            (
                i.consume_shortcut(&DUPLICATE),
                i.consume_shortcut(&DELETE),
                i.consume_shortcut(&RENAME),
                i.consume_shortcut(&EARLIER),
                i.consume_shortcut(&LATER),
            )
        });
        // The tool library window is a floating window, so these keys would
        // otherwise act on the selected operation behind it.
        if let Some(Selection::Operation(id)) = self.selected
            && !self.tool_library.is_open()
        {
            let action = if duplicate {
                Some(TreeAction::Duplicate(id))
            } else if delete {
                Some(TreeAction::Delete(id))
            } else if earlier {
                self.move_action(id, -1)
            } else if later {
                self.move_action(id, 1)
            } else {
                None
            };
            if let Some(action) = action {
                self.tree_action(action);
            }
        }
        if delete
            && let Some(Selection::Set(id)) = self.selected
            && !self.tool_library.is_open()
        {
            self.tree_action(TreeAction::DeleteSet(id));
        }
        if rename && let Some(item) = self.selected_item() {
            self.begin_rename(item);
        }
        // Esc cancels the running job when nothing is being picked
        // (specification section 16); picking takes its own Esc.
        let picking = matches!(self.selected, Some(Selection::Picking | Selection::Set(_)));
        if !picking
            && self.run.is_running()
            && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape))
        {
            self.run.cancel();
        }
    }

    /// The command that moves operation `id` by `by` places, if it can move.
    fn move_action(&self, id: OperationId, by: isize) -> Option<TreeAction> {
        let ops = self.app.as_ref()?.project().operations();
        let index = ops.iter().position(|o| o.id == id)?;
        let to = index.checked_add_signed(by).filter(|t| *t < ops.len())?;
        Some(TreeAction::Command(Box::new(Command::Move {
            id: ItemId::Operation(id),
            index: to,
        })))
    }

    /// The tree item the inspector shows, if it can be renamed or deleted.
    fn selected_item(&self) -> Option<ItemRef> {
        match self.selected? {
            Selection::Operation(id) => Some(ItemRef::Operation(id)),
            Selection::Setup(id) => Some(ItemRef::Setup(id)),
            Selection::Set(id) => Some(ItemRef::SelectionSet(id)),
            Selection::Model(id) => Some(ItemRef::Model(id)),
            Selection::Picking => None,
        }
    }

    /// Opens the rename dialog for `item`, if it has a name of its own.
    fn begin_rename(&mut self, item: ItemRef) {
        if self.defer(move |s| s.begin_rename(item)) {
            return;
        }
        let Some(app) = &self.app else { return };
        let project = app.project();
        let name = match item {
            ItemRef::Operation(id) => project.operation(id).map(|o| o.name.clone()),
            ItemRef::Setup(id) => project.setup(id).map(|s| s.name.clone()),
            ItemRef::Tool(id) => project
                .tools()
                .iter()
                .find(|t| t.id == id)
                .map(|t| t.name.clone()),
            ItemRef::SelectionSet(id) => project
                .selection_sets()
                .iter()
                .find(|s| s.id == id)
                .map(|s| s.name.clone()),
            // A model is named by its file.
            ItemRef::Model(_) | ItemRef::SetupPart(..) => None,
        };
        if let Some(name) = name {
            self.dialog = Some(Dialog::Rename { item, name });
        }
    }

    /// Renames `item` as one undoable command (the F2 dialog).
    fn rename(&mut self, item: ItemRef, name: &str) {
        if let Err(e) = self.rename_checked(item, name) {
            self.dialog = Some(Dialog::Error(format!("The name was not changed: {e}")));
        }
    }

    /// Why Post and export is unavailable, if it is (decision 0148): the
    /// first thing the project lacks.
    pub(crate) fn post_blocker(&self) -> Option<&'static str> {
        if self.app.is_none() {
            return Some("Open or start a project first");
        }
        self.overview.post_blocker()
    }

    fn open_export(&mut self) {
        if self.defer(Self::open_export) {
            return;
        }
        if self.post_blocker().is_some() {
            return;
        }
        if let Some(app) = &self.app {
            // The user's file name templates, kept across projects (issues
            // 308 and 310).
            self.export.set_file_names(self.settings.file_names.clone());
            self.export.open(app);
        }
    }

    fn open_dialog(&mut self, dialog: Dialog) {
        let deferred = dialog.clone();
        if self.defer(move |s| s.open_dialog(deferred)) {
            return;
        }
        self.dialog = Some(dialog);
    }

    fn show_results(&mut self) {
        if self.defer(Self::show_results) {
            return;
        }
        self.run.show_results();
    }

    /// Carries out a message's action link (specification section 9).
    fn message_action(&mut self, action: MessageAction) {
        if self.defer(move |s| s.message_action(action)) {
            return;
        }
        match action {
            MessageAction::ReviewExport(index) => {
                if let Some(report) = self.app.as_ref().and_then(|app| app.output_report(index)) {
                    self.export.show_report(report);
                }
            }
            MessageAction::Show(item) => self.show_item(item),
            MessageAction::Reselect(set) => {
                // Waits, as the Pick again button does, while a change is
                // checked (decision 0215 item 6; OI-874).
                if self.selection.is_writing() {
                    return;
                }
                self.start_picking();
                self.selection.pick_again(set);
                self.select(Selection::Set(set));
            }
            MessageAction::ReviewSetup(setup) => {
                self.open_step(Step::Setup);
                self.choose_setup(setup, None);
            }
            MessageAction::EditWorkholding(setup) => {
                self.choose_setup(setup, Some(SetupPart::Workholding));
            }
            MessageAction::Generate => self.generate(false),
            MessageAction::ShowSetting(op) => {
                self.show_item(ItemRef::Operation(op));
                self.setting_jump = self
                    .app
                    .as_ref()
                    .and_then(|app| app.operation_warning_settings(op).into_iter().next())
                    .map(|name| (op, name));
            }
        }
    }

    /// Takes the user to the first problem: the step it belongs to, the item
    /// in the tree and, for a missing value, the field (issue 210).
    fn go_to_first_problem(&mut self) {
        if self.defer(Self::go_to_first_problem) {
            return;
        }
        use openchuck_app::requirements::RequirementField;
        let Some(item) = self.overview.first_problem() else {
            return;
        };
        let field = self
            .overview
            .missing
            .iter()
            .find(|m| m.item == item)
            .map(|m| m.requirement.field.clone());
        self.open_step(item.step());
        match (item, &field) {
            (ItemRef::Setup(id), Some(field)) => {
                let part = match field {
                    RequirementField::Machine => SetupPart::Machine,
                    RequirementField::Part => SetupPart::PartPosition,
                    _ => SetupPart::Stock,
                };
                self.choose_setup(id, Some(part));
                self.settings.inspector.collapsed = false;
            }
            _ => {
                self.show_item(item);
                self.requirement_jump = field;
            }
        }
    }

    /// Selects `item` in the tree and the inspector (a message's source).
    fn show_item(&mut self, item: ItemRef) {
        if self.defer(move |s| s.show_item(item)) {
            return;
        }
        match item {
            ItemRef::Model(id) => self.select(Selection::Model(id)),
            ItemRef::Setup(id) => self.choose_setup(id, None),
            ItemRef::SetupPart(id, part) => self.choose_setup(id, Some(part)),
            ItemRef::SelectionSet(id) => {
                self.selection.show_set(Some(id));
                self.select(Selection::Set(id));
            }
            // Tools are not in the tree: a tool opens in the tool library
            // window (decision record 0127).
            ItemRef::Tool(id) => self.open_tool_library(Purpose::Add, Some(Chosen::Project(id))),
            ItemRef::Operation(id) => self.select(Selection::Operation(id)),
        }
        self.settings.inspector.collapsed = false;
    }

    /// Chooses `setup` (the view shows it) and opens its inspector at
    /// `section`.
    fn choose_setup(&mut self, setup: SetupId, section: Option<SetupPart>) {
        if self.defer(move |s| s.choose_setup(setup, section)) {
            return;
        }
        if self
            .workholding_panel
            .as_ref()
            .is_none_or(|p| p.setup() != setup)
        {
            self.workholding_panel = None;
        }
        if section == Some(SetupPart::Machine) && self.machine_panel.is_none() {
            self.machine_panel = Some(MachinePanel::default());
        }
        if self.setup != Some(setup) {
            self.setup = Some(setup);
            self.rebuild_scene(true);
        }
        self.setup_section = section;
        self.select(Selection::Setup(setup));
    }

    /// Runs a project tree action through a command, so it can be undone.
    fn tree_action(&mut self, action: TreeAction) {
        let deferred = action.clone();
        if self.defer(move |s| s.tree_action(deferred)) {
            return;
        }
        let command = match action {
            TreeAction::Select(id) => {
                self.select(Selection::Operation(id));
                if self.step == Step::Simulate {
                    self.open_step(Step::Operations);
                    self.reveal_operation_inspector();
                }
                return;
            }
            TreeAction::SelectModel(id, section) => {
                self.select(Selection::Model(id));
                self.model_section = section;
                if let Some(key) = section {
                    self.model_panel
                        .get_or_insert_with(|| ModelPanel::new(id))
                        .open_section(key);
                    if let Some(panel) = &mut self.model_panel
                        && panel.model() != id
                    {
                        *panel = ModelPanel::new(id);
                        panel.open_section(key);
                    }
                }
                return;
            }
            TreeAction::SelectSet(id) => {
                self.selection.show_set(Some(id));
                self.select(Selection::Set(id));
                return;
            }
            TreeAction::DeleteSet(id) => {
                self.delete_set(id);
                return;
            }
            TreeAction::SelectSetup(id, section) => {
                self.choose_setup(id, section);
                return;
            }
            TreeAction::Rename(item) => {
                self.begin_rename(item);
                return;
            }
            TreeAction::ShowInView(item) => {
                self.show_item(item);
                self.viewport.fit();
                return;
            }
            TreeAction::Generate(id) => {
                self.select(Selection::Operation(id));
                self.generate_selected();
                return;
            }
            TreeAction::Add(group) => {
                match group {
                    tree::Group::Model => self.import_model(),
                    tree::Group::SelectionSets => self.start_picking(),
                    tree::Group::Operations => self.new_operation(),
                    tree::Group::Setups => self.add_setup(),
                }
                return;
            }
            TreeAction::Command(c) => Ok(*c),
            TreeAction::Duplicate(id) => self
                .app
                .as_ref()
                .map_or(Err(AppError::UnknownOperation(id.to_string())), |app| {
                    openchuck_app::duplicate_operation(app.project(), id)
                }),
            TreeAction::Delete(id) => {
                if let Some(app) = &mut self.app {
                    match app.delete_operation(id) {
                        Ok(()) => {
                            if self.selected == Some(Selection::Operation(id)) {
                                self.selected = None;
                            }
                        }
                        Err(e) => {
                            self.dialog =
                                Some(Dialog::Error(format!("The operation was not deleted: {e}")));
                        }
                    }
                }
                return;
            }
        };
        let Some(app) = &mut self.app else { return };
        if let Err(e) = command.and_then(|c| app.execute(c)) {
            self.dialog = Some(Dialog::Error(e.to_string()));
        }
    }

    fn select(&mut self, selection: Selection) {
        if self.selected != Some(selection) && self.defer(move |s| s.select(selection)) {
            return;
        }
        // A model's Placement or Mesh check row is chosen only by its own
        // tree row, which sets it again after this.
        self.model_section = None;
        if self.selected != Some(selection) {
            self.selected = Some(selection);
            self.form.clear();
        }
    }

    /// Works the generating job, the add-operation dialog and the playback
    /// (slice 9).
    fn run_frame(&mut self, ctx: &egui::Context) {
        let mut error = None;
        let mut regenerated = false;
        let mut added = None;
        let windowed = self.window_open();
        if let Some(app) = &mut self.app {
            if let Some((run, job)) = self.run.poll(&self.jobs, ctx) {
                regenerated = true;
                self.simulate_camera = None;
                self.jobs.set_summary(job, run_summary(&run));
                self.play.set_run(app, run);
                self.overview_dirty = true;
            }
            let (new, problem) = self.run.add_dialog(ctx, app);
            added = new;
            error = problem;
            self.run.results_window(ctx, app);
            let outcome = self
                .export
                .frame(ctx, app, self.run.last(), self.run.last_error());
            if outcome.reported {
                self.overview_dirty = true;
            }
            // A confirmed template change is the user's setting from now on.
            if let Some(names) = self.export.file_names_to_keep(ctx)
                && *names != self.settings.file_names
            {
                self.settings.file_names = names.clone();
                self.settings_changed = Some(Instant::now());
            }
            if let Some(status) = outcome.status {
                self.status = status;
            }
            error = error.or(outcome.error);
            if self.step == Step::Simulate && !windowed {
                self.play.handle_keys(ctx);
            }
            self.play.forget_removed(app);
            if let Some(last) = self.run.last() {
                self.play.restore(app, last);
            }
            // The playback window is not remembered: leaving Simulate lifts
            // it (issue 205).
            if self.step != Step::Simulate && self.play.window().is_some() {
                self.play.reset_window();
            }
            let dt = f64::from(ctx.input(|i| i.stable_dt)).min(0.25);
            if self.play.frame(app, &mut self.viewport, dt) {
                ctx.request_repaint();
            }
        }
        if regenerated {
            self.rebuild_scene(false);
        }
        // The tool library window works with or without a project (12.1). Its
        // folded groups and size filter are kept in the settings (UI-15).
        if !self.tool_library.view_synced() {
            self.tool_library
                .set_view(self.settings.tool_library.clone());
        }
        let library = self.tool_library.frame(ctx, self.app.as_mut());
        if self.tool_library.view_synced()
            && *self.tool_library.view() != self.settings.tool_library
        {
            self.settings.tool_library = self.tool_library.view().clone();
            self.settings_touched();
        }
        if let Some(status) = library.status {
            self.status = status;
        }
        if let Some(tool) = library.for_new_operation {
            self.run.set_adding_tool(tool);
        }
        if self.run.take_library_request() {
            self.open_tool_library(Purpose::NewOperation, None);
        }
        if let Some(id) = added {
            self.select(Selection::Operation(id));
        }
        if let Some(message) = error {
            self.dialog = Some(Dialog::Error(message));
        }
    }

    /// Hands the pointer's picks to the selection panel and draws its
    /// highlights (slice 10 item 1).
    fn pick_frame(&mut self, ctx: &egui::Context) {
        let input = self.viewport.take_input();
        self.pick_intent(input, ctx);
        if self.app.is_none() {
            return;
        }
        // Q, W, E and R choose the click; Esc leaves picking (specification
        // 15). Not while a dialog or a floating window is open: its keys are
        // its own, and Esc must not leave picking behind it.
        if !self.window_open()
            && matches!(self.selected, Some(Selection::Picking | Selection::Set(_)))
            && self.selection.keys(ctx, self.text_had_focus)
        {
            self.selected = None;
        }
        if let Some(app) = &mut self.app {
            // Keep keyboard handling first, including Esc during Pick again.
            // A check can finish after the inspector polled it during layout:
            // store it before deriving the shown set, legend and overlay
            // (UI-05, issue 298).
            if let Some(e) = self.selection.poll(app) {
                self.dialog = Some(Dialog::Error(e));
            }
            if let Some(e) = self.selection.reset_dialog(ctx, app) {
                self.dialog = Some(Dialog::Error(e));
            }
        }
        self.follow_selection_panel();
        let Some(app) = &self.app else { return };
        // The highlights follow the selection state (issue 261): the working
        // selection is drawn only in selection mode, and a set shown for one
        // item goes when another is selected, however the selection changed
        // (a click, Esc, a deletion or another step).
        let shows = self.selection.show_count();
        if shows != self.shown_for.0 {
            // A set was shown since: it is shown for the item selected now.
            self.shown_for = (shows, self.selected);
        } else if self.selected != self.shown_for.1 {
            let keeps = matches!(
                self.selected,
                Some(Selection::Set(id)) if self.selection.shown_set() == Some(id)
            );
            if !keeps {
                self.selection.show_set(None);
            }
            self.shown_for = (self.selection.show_count(), self.selected);
        }
        self.selection.set_picking(matches!(
            self.selected,
            Some(Selection::Picking | Selection::Set(_))
        ));
        // The selected operation's avoid and check sets are drawn hatched (7.4).
        self.selection.show_roles_of(
            app,
            match self.selected {
                Some(Selection::Operation(id)) => Some(id),
                _ => None,
            },
        );
        // The set drawn in its own colour joins the legends (decision 0216).
        self.viewport.set_legend_set(self.selection.shown_look(app));
        if let Some(overlay) = self.selection.overlay(app, self.viewport.view()) {
            self.viewport.set_overlay(overlay);
            ctx.request_repaint();
        }
    }

    /// A viewport pick retains model/face/edge identity, never a pointer
    /// coordinate. It shares the same departure guard as tree selection.
    fn pick_intent(&mut self, input: crate::viewport::PickInput, ctx: &egui::Context) {
        if input.click.is_some() || input.edge_click.is_some() || input.solid_click.is_some() {
            let deferred_ctx = ctx.clone();
            let intent = self
                .app
                .as_ref()
                .and_then(|app| pending_pick::Intent::capture(app, self.viewport.view(), input));
            if self.defer(move |s| {
                if let Some(intent) = intent {
                    s.resume_pick(intent, &deferred_ctx);
                }
            }) {
                return;
            }
        }
        if let Some(item) = input.solid_click {
            self.select_solid(item);
        }
        let Some(app) = &self.app else { return };
        if self.selection.frame(app, self.viewport.view(), input) {
            ctx.request_repaint();
        }
    }

    /// Selects what the selection panel chose or left (decision 0215): a new
    /// set once it is saved, or New selection after a plain click left the
    /// chosen set.
    fn follow_selection_panel(&mut self) {
        if let Some(chosen) = self.selection.take_chosen_change() {
            self.select(chosen.map_or(Selection::Picking, Selection::Set));
            self.shown_for = (self.selection.show_count(), self.selected);
        }
    }

    /// Deletes selection set `id`, refused with the operations that use it
    /// (12.2; decision 0215 item 2).
    fn delete_set(&mut self, id: SelectionSetId) {
        let Some(app) = &mut self.app else { return };
        let Some(summary) = app
            .selection_summaries(&[])
            .into_iter()
            .find(|s| s.id == id)
        else {
            return;
        };
        if let Some(note) = crate::selection_panel::delete_refusal(&summary) {
            self.dialog = Some(Dialog::Error(note));
            return;
        }
        match app.delete_selection(id) {
            Ok(()) => {
                if self.selected == Some(Selection::Set(id)) {
                    self.selection.show_set(None);
                    self.selected = Some(Selection::Picking);
                }
            }
            Err(e) => {
                self.dialog = Some(Dialog::Error(format!("The set was not deleted: {e}")));
            }
        }
    }

    /// Selects the held region or tab `item` of the setup shown: its setup
    /// in the inspector at the workholding section, its row open with the
    /// first field focused, and its solid outlined (7.5).
    fn select_solid(&mut self, item: openchuck_app::scene::WorkholdingItem) {
        if self.defer(move |s| s.select_solid(item)) {
            return;
        }
        let Some(setup) = self.setup else { return };
        self.choose_setup(setup, Some(SetupPart::Workholding));
        if let Some(panel) = &mut self.workholding_panel {
            panel.open_item(item);
        }
        self.viewport.set_selected_solid(Some(item));
    }

    /// Gives the view what the workholding section shows (7.5, 11.7; OI-334
    /// k, OI-355): the draft while it has unapplied changes, whatever is
    /// selected, and, while its setup is selected, the default tabs offered,
    /// the failing parts and the row with keyboard focus. The selected solid
    /// stays while its setup is selected, and Esc clears it, on every step,
    /// the Setup panes included (OI-354).
    fn workholding_view_frame(&mut self, ctx: &egui::Context) {
        let shown = self.setup;
        let setup_selected =
            matches!(self.selected, Some(Selection::Setup(id)) if Some(id) == shown);
        let mut focused = None;
        let draft = match (&self.app, &mut self.workholding_panel) {
            (Some(app), Some(panel)) if Some(panel.setup()) == shown => {
                match app.project().setup(panel.setup()) {
                    Some(setup) if setup_selected || panel.has_unapplied_changes() => {
                        if setup_selected {
                            focused = panel.focused();
                        }
                        let setup = setup.clone();
                        panel.viewport_draft(&setup)
                    }
                    _ => None,
                }
            }
            _ => None,
        };
        self.viewport.set_workholding_draft(draft);
        self.viewport.set_focused_solid(focused);
        let esc = !self.text_had_focus && ctx.input(|i| i.key_pressed(Key::Escape));
        if self.viewport.selected_solid().is_some() && (!setup_selected || esc) {
            self.viewport.set_selected_solid(None);
        }
    }

    /// Whether a dialog or a floating window (export, results, add operation)
    /// is in front of the view.
    fn window_open(&self) -> bool {
        self.dialog.is_some()
            || self.export.is_open()
            || self.run.has_window()
            || self.import.is_some()
            || self.tool_library.is_open()
    }

    /// Runs the import dialog (OI-51): polls its check, shows the preview in
    /// the viewport, and adds the model or closes.
    fn import_frame(&mut self, ctx: &egui::Context) {
        // A dropped file opens the import window on it (specification 11.2).
        let dropped = ctx.input(|i| {
            i.raw
                .dropped_files
                .first()
                .map(|f| f.path().to_path_buf())
                .filter(|p| !p.as_os_str().is_empty())
        });
        if let Some(file) = dropped {
            self.dropped_model(file);
        }
        let (Some(app), Some(session)) = (&self.app, &mut self.import) else {
            return;
        };
        if session.poll() {
            ctx.request_repaint();
        }
        if session.fresh {
            session.fresh = false;
            if let Some(preview) = &session.preview {
                self.viewport.set_scene(preview.scene.clone(), true);
            }
        }
        session.overlays(ctx, self.viewport.last_rect);
        let action = session.window(ctx, app);
        match action {
            ImportAction::None => {}
            ImportAction::Cancel => {
                self.import = None;
                self.rebuild_scene(true);
            }
            // The file is read and checked as a job; the model is added when
            // it ends, below, so the window never waits on it.
            ImportAction::Add => {
                if let (Some(app), Some(session)) = (&self.app, &mut self.import)
                    && !session.is_adding()
                {
                    session.start_adding(app);
                }
            }
        }
        let (Some(app), Some(session)) = (&mut self.app, &mut self.import) else {
            return;
        };
        if session.is_adding() {
            ctx.request_repaint();
        }
        match session.poll_adding() {
            None => {}
            Some(Ok(prepared)) => match app.add_prepared_model(prepared) {
                Ok(_) => {
                    self.status = format!("Added {}", session.path.display());
                    self.import = None;
                    self.rebuild_scene(true);
                }
                Err(e) => session.error = Some(e.to_string()),
            },
            Some(Err(e)) => session.error = Some(e.to_string()),
        }
    }

    fn dropped_model(&mut self, file: PathBuf) {
        let deferred = file.clone();
        if self.defer(move |s| s.dropped_model(deferred)) {
            return;
        }
        match (&self.app, &mut self.import) {
            (Some(app), None) => self.import = Some(ImportSession::new(app, file)),
            (Some(app), Some(session)) if !session.is_adding() => session.set_path(app, file),
            (Some(_), Some(_)) => {}
            (None, _) => {
                self.status = "Open or create a project before importing a model.".to_owned()
            }
        }
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        let Some(dialog) = self.dialog.clone() else {
            return;
        };
        let new_project = matches!(dialog, Dialog::Folder { create: true, .. });
        let mut close = false;
        let mut form_submit = false;
        let mut form_browse = false;
        let mut action: Option<Action> = None;
        egui::Modal::new(egui::Id::new("openchuck dialog")).show(ctx, |ui| match dialog {
            Dialog::Folder {
                create,
                mut folder,
                mut name,
            } => {
                ui.heading(if create {
                    "New project"
                } else {
                    "Open project"
                });
                ui.label(if create {
                    "Parent folder:"
                } else {
                    "Project folder:"
                });
                ui.horizontal(|ui| {
                    crate::pending_edits::text_edit(ui, &mut folder, "Project folder");
                    form_browse = ui.button("Browse…").clicked();
                });
                // What a new project would get: the cleaned folder name, and
                // whether it can be made there (#219).
                let mut found = None;
                // A new project needs a name: a blank one would be turned
                // into a fallback folder name without the user choosing it
                // (OI-694).
                if create {
                    ui.label("Project name:");
                    crate::pending_edits::name_edit(ui, &mut name, "Project name");
                    if name.trim().is_empty() {
                        ui.colored_label(DARK.error, "Give the project a name.");
                    } else if !folder.trim().is_empty() {
                        found = Some(new_folder::inspect(Path::new(folder.trim()), name.trim()));
                    }
                    if let Some(found) = &found {
                        ui.label(format!("Folder to create: {}", found.name));
                        if let Some(problem) = new_folder_problem(found) {
                            ui.colored_label(DARK.error, problem);
                        }
                    }
                }
                let blocked = (create && name.trim().is_empty())
                    || found
                        .as_ref()
                        .is_some_and(|f| new_folder_problem(f).is_some());
                ui.horizontal(|ui| {
                    let ok = ui
                        .add_enabled(
                            !folder.trim().is_empty() && !blocked,
                            egui::Button::new("OK"),
                        )
                        .clicked();
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                    if ok { form_submit = true; }
                });
                self.dialog = Some(Dialog::Folder { create, folder, name });
            }
            Dialog::FolderExists {
                parent,
                name,
                folder,
            } => {
                ui.heading("Folder already exists");
                ui.label(format!(
                    "A folder named \"{}\" already exists in {parent}.",
                    folder
                        .file_name()
                        .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
                ));
                ui.label(
                    "If you use it, the project's files are added to it. Nothing in it is changed or deleted.",
                );
                ui.horizontal(|ui| {
                    if ui.button("Use the existing folder").clicked() {
                        close = true;
                        let project_name = name.clone();
                        action = Some(Box::new(move |s: &mut Self| {
                            s.create(folder, &project_name);
                        }));
                    }
                    if ui.button("Change the name").clicked() {
                        close = true;
                        action = Some(Box::new(move |s: &mut Self| {
                            s.dialog = Some(Dialog::Folder {
                                create: true,
                                folder: parent,
                                name,
                            });
                        }));
                    }
                });
            }
            Dialog::OfferCreate { folder } => {
                ui.label(format!(
                    "{} holds no project. Create a new project there?",
                    folder.display()
                ));
                ui.horizontal(|ui| {
                    if ui.button("Create").clicked() {
                        close = true;
                        let name = folder.file_name().map_or_else(
                            || "Untitled".to_owned(),
                            |n| n.to_string_lossy().into_owned(),
                        );
                        action = Some(Box::new(move |s: &mut Self| s.create(folder, &name)));
                    }
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                });
            }
            Dialog::Unsaved(next) => {
                ui.label("The project has unsaved changes.");
                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() {
                        close = true;
                        let ctx = ui.ctx().clone();
                        action = Some(Box::new(move |s: &mut Self| {
                            if s.save() {
                                s.proceed(next, &ctx);
                            }
                        }));
                    }
                    if ui.button("Discard changes").clicked() {
                        close = true;
                        let ctx = ui.ctx().clone();
                        action = Some(Box::new(move |s: &mut Self| s.proceed(next, &ctx)));
                    }
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                });
            }
            Dialog::Rename { item, mut name } => {
                ui.heading("Rename");
                let field = crate::pending_edits::name_edit(ui, &mut name, "Name");
                let enter = field.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                // The field has the keyboard when the dialog opens.
                if !enter && ui.memory(|m| m.focused().is_none()) {
                    field.request_focus();
                }
                ui.horizontal(|ui| {
                    if ui.button("Rename").clicked() || enter {
                        form_submit = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                });
                self.dialog = Some(Dialog::Rename { item, name });
            }
            Dialog::Keys => {
                ui.heading("Keyboard shortcuts");
                egui::Grid::new("keys").striped(true).show(ui, |ui| {
                    for (keys, what) in frame::KEY_MAP {
                        ui.monospace(*keys);
                        ui.label(*what);
                        ui.end_row();
                    }
                });
                if ui.button("Close").clicked() {
                    close = true;
                }
            }
            Dialog::About => {
                let build = openchuck_buildinfo::current_text();
                let project = self.project_facts().map(|(_, _, path)| path);
                if about::show(ui, build, about::licence_text(), project.as_deref()).close {
                    close = true;
                }
            }
            Dialog::Settings => {
                ui.heading("Settings");
                let mut minutes=self.settings.checkpoint_minutes;
                ui.horizontal(|ui| {
                    ui.label("Recovery checkpoint interval (minutes)");
                    if ui.add(egui::DragValue::new(&mut minutes).range(1..=15)).changed() {
                        self.settings.checkpoint_minutes=minutes;
                        self.settings_touched();
                    }
                });
                ui.label("Keeps three periodic checkpoints and one interim checkpoint after 15 seconds without a confirmed change. Recovery never saves over your project automatically.");
                let mut skip = self.settings.simulate.skip_stock_simulation;
                if ui
                    .checkbox(&mut skip, "Skip the stock simulation in new projects")
                    .on_hover_text(
                        "New projects then make only their toolpaths when generating. \
                         Each project keeps its own choice, in Simulate",
                    )
                    .changed()
                {
                    action = Some(Box::new(move |s: &mut Self| {
                        s.settings.simulate.skip_stock_simulation = skip;
                        s.settings_touched();
                    }));
                }
                let mut record = crate::logging::recording();
                if ui
                    .checkbox(&mut record, "Record my actions in the log")
                    .changed()
                {
                    action = Some(Box::new(move |s: &mut Self| s.set_record_actions(record)));
                }
                ui.label(
                    "Adds the menu commands, buttons, dialogs and edits you use to the log, \
                     with the names of the items and parameters but not their values, \
                     so a bug report can show the steps taken. The log stays on this computer.",
                );
                match crate::logging::log_path() {
                    Some(path) => ui.label(format!("The log is {}", path.display())),
                    None => ui.label("There is no log file in this run."),
                };
                if ui.button("Close").clicked() {
                    close = true;
                }
            }
            Dialog::Error(message) => {
                ui.label(message);
                if ui.button("OK").clicked() {
                    close = true;
                }
            }
        });
        if form_browse {
            self.browse_form_dialog();
            return;
        }
        if form_submit {
            if new_project {
                // OK explicitly accepts this dialog's values; its own
                // validation has already decided whether OK is enabled.
                crate::pending_edits::clear(ctx);
            }
            if !self.defer(Self::accept_form_dialog) {
                self.accept_form_dialog();
            }
            return;
        }
        if close {
            if new_project {
                // Cancel explicitly abandons this dialog's input.
                crate::pending_edits::clear(ctx);
            }
            let deferred_ctx = ctx.clone();
            if !crate::pending_edits::entries(ctx).is_empty() {
                self.defer(move |s| {
                    s.dialog = None;
                    deferred_ctx.memory_mut(egui::Memory::stop_text_input);
                    if let Some(action) = action {
                        action(s);
                    }
                });
                return;
            }
            self.dialog = None;
            // A field of the closed dialog must not keep the keyboard.
            ctx.memory_mut(egui::Memory::stop_text_input);
        }
        if let Some(action) = action {
            action(self);
        }
    }

    /// Re-reads an accepted dialog's restored or confirmed draft, so a
    /// deferred action never captures a value later discarded by the user.
    fn accept_form_dialog(&mut self) {
        match self.dialog.take() {
            Some(Dialog::Folder {
                create,
                folder,
                name,
            }) => {
                if folder.trim().is_empty() || (create && name.trim().is_empty()) {
                    self.dialog = Some(Dialog::Folder {
                        create,
                        folder,
                        name,
                    });
                    return;
                }
                if create {
                    let found = new_folder::inspect(Path::new(folder.trim()), name.trim());
                    if let Some(problem) = new_folder_problem(&found) {
                        self.dialog = Some(Dialog::Error(problem));
                    } else if found.state == new_folder::FolderState::ExistsUsable {
                        self.dialog = Some(Dialog::FolderExists {
                            parent: folder.trim().to_owned(),
                            name: name.trim().to_owned(),
                            folder: found.path,
                        });
                    } else {
                        self.create(found.path, name.trim());
                    }
                } else {
                    self.open(PathBuf::from(folder.trim()));
                }
            }
            Some(Dialog::Rename { item, name }) => self.rename(item, &name),
            other => self.dialog = other,
        }
    }

    fn browse_form_dialog(&mut self) {
        if self.defer(Self::browse_form_dialog) {
            return;
        }
        if let Some(Dialog::Folder { folder, .. }) = &mut self.dialog
            && let Some(picked) = pick_folder(folder)
        {
            *folder = picked;
        }
    }

    /// Stops the window closing while there are unsaved changes, and asks.
    fn guard_close(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.viewport().close_requested()) {
            if !self.quitting {
                let deferred_ctx = ctx.clone();
                if self.defer(move |s| s.request(Next::Quit, &deferred_ctx)) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                    return;
                }
            }
            if !self.quitting && self.app.as_ref().is_some_and(Application::is_modified) {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.dialog = Some(Dialog::Unsaved(Next::Quit));
            } else {
                if !self.quitting
                    && self.recovery.close(
                        Next::Quit,
                        self.app.as_ref().map_or(0, Application::revision),
                    )
                {
                    ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                    ctx.request_repaint();
                } else {
                    self.save_settings();
                }
            }
        }
    }
}

/// "Generated 2, reused 1": a run's outcome for the Jobs tab (decision
/// record 0030 item 6).
fn run_summary(run: &openchuck_app::Regeneration) -> String {
    let mut out = format!("Generated {}", run.generated.len());
    if !run.reused.is_empty() {
        out.push_str(&format!(", reused {}", run.reused.len()));
    }
    if run.simulation.is_some() {
        out.push_str(", simulated");
    }
    // Only when an enabled operation follows the one it stopped after: G on
    // the last operation leaves nothing stale.
    if let Some(last) = run.stopped_after {
        let ops = run.project.operations();
        let at = ops.iter().position(|o| o.id == last);
        if let Some(i) = at
            && ops.iter().skip(i + 1).any(|o| o.enabled)
        {
            out.push_str(&format!(
                "; stopped after operation {}, so later operations stay stale",
                i + 1
            ));
        }
    }
    out
}

/// Which of save, undo and redo were pressed this frame, consuming them.
/// Redo is checked first: egui matches a shortcut while an extra Shift is
/// held, so Ctrl+Shift+Z would otherwise be taken by Undo (Ctrl+Z).
fn edit_shortcuts(i: &mut egui::InputState) -> (bool, bool, bool) {
    let save = i.consume_shortcut(&SAVE);
    let redo = i.consume_shortcut(&REDO) || i.consume_shortcut(&REDO_ALT);
    let undo = !redo && i.consume_shortcut(&UNDO);
    (save, undo, redo)
}

/// Asks the platform's folder dialog for a folder, starting near `current`
/// (OI-127). `None` when the user cancels or no dialog is available, in
/// which case the typed path stays.
/// Why a new project cannot be made in the folder `found` names, in plain
/// words, or `None` when it can (or the user is first asked, for a folder
/// that exists and is usable).
fn new_folder_problem(found: &new_folder::NewFolder) -> Option<String> {
    use new_folder::FolderState;
    match &found.state {
        FolderState::NoParent => Some("The parent folder does not exist.".to_owned()),
        FolderState::Free | FolderState::ExistsUsable => None,
        FolderState::ExistsAsFile => Some(format!(
            "A file named \"{}\" is already there. Choose another name.",
            found.name
        )),
        FolderState::ExistsWithProject(names) => Some(format!(
            "The folder \"{}\" already holds an OpenChuckCAM project or its files ({}). \
Nothing is overwritten. Choose another name.",
            found.name,
            names.join(", ")
        )),
    }
}

fn pick_folder(current: &str) -> Option<String> {
    #[cfg(test)]
    if stub_picker(current) {
        return None;
    }
    let mut dialog = rfd::FileDialog::new().set_title("Project folder");
    if let Some(start) = existing_dir(current) {
        dialog = dialog.set_directory(start);
    }
    dialog.pick_folder().map(|p| p.display().to_string())
}

/// Asks the platform's file dialog for a model file, starting near `current`
/// or in the project folder (OI-127).
/// The name of the model file filter and the extensions it offers (IMP-01,
/// IMP-02): STL and STEP, in either case.
pub(crate) const MODEL_FILTER: (&str, &[&str]) = (
    "3D models (STL, STEP)",
    &["stl", "STL", "step", "STEP", "stp", "STP"],
);

pub(crate) fn pick_model(current: &str, app: Option<&Application>) -> Option<String> {
    #[cfg(test)]
    if stub_picker(current) {
        return None;
    }
    let mut dialog = rfd::FileDialog::new()
        .set_title("Import model")
        .add_filter(MODEL_FILTER.0, MODEL_FILTER.1);
    let start = existing_dir(current).or_else(|| app.map(|a| a.location().to_path_buf()));
    if let Some(start) = start {
        dialog = dialog.set_directory(start);
    }
    dialog.pick_file().map(|p| p.display().to_string())
}

#[cfg(test)]
thread_local! {
    static PICKER_INPUTS: std::cell::RefCell<Option<Vec<String>>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
fn stub_picker(current: &str) -> bool {
    PICKER_INPUTS.with(|inputs| {
        let mut inputs = inputs.borrow_mut();
        if let Some(inputs) = inputs.as_mut() {
            inputs.push(current.to_owned());
            true
        } else {
            false
        }
    })
}

/// The folder `text` names, or its parent, if it exists.
fn existing_dir(text: &str) -> Option<PathBuf> {
    let path = PathBuf::from(text.trim());
    if text.trim().is_empty() {
        return None;
    }
    if path.is_dir() {
        Some(path)
    } else {
        path.parent().filter(|p| p.is_dir()).map(PathBuf::from)
    }
}

/// What a row of the project tree asks for.
#[derive(Debug, Clone, PartialEq)]
enum TreeAction {
    /// A command to execute.
    Command(Box<Command>),
    /// Duplicate this operation (the command is built from the project).
    Duplicate(OperationId),
    /// Delete this operation.
    Delete(OperationId),
    /// Delete this selection set; refused while an operation uses it
    /// (12.2; decision 0215 item 2).
    DeleteSet(SelectionSetId),
    /// Show this operation's parameters.
    Select(OperationId),
    /// Show this model's units, placement and checks, at a section (the
    /// model panel's section key; issue #181).
    SelectModel(ModelId, Option<&'static str>),
    /// Show this selection set.
    SelectSet(SelectionSetId),
    /// Show this setup, at a section.
    SelectSetup(SetupId, Option<SetupPart>),
    /// Rename an item.
    Rename(ItemRef),
    /// Select an item and fit the view.
    ShowInView(ItemRef),
    /// Generate up to and including this operation.
    Generate(OperationId),
    /// The "+" of a group header.
    Add(tree::Group),
}

/// What the inspector shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Selection {
    Operation(OperationId),
    Model(ModelId),
    /// A setup, its machine and its workholding and tabs.
    Setup(SetupId),
    /// Picking on the part, and the selection sets (slice 10).
    Picking,
    /// A selection set, picking on the part as well.
    Set(SelectionSetId),
}

impl eframe::App for Shell {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.draw(ui);
        crate::graphics::reached(crate::graphics::Stage::Drawing);
    }
}

impl Shell {
    /// The project's name, whether it has unsaved changes and its folder.
    fn project_facts(&self) -> Option<(String, bool, String)> {
        self.app.as_ref().map(|a| {
            (
                a.project().name().to_owned(),
                a.is_modified(),
                a.location().display().to_string(),
            )
        })
    }

    /// The window title for the project now open (#218).
    fn window_title(&self) -> String {
        let facts = self.project_facts();
        crate::window_title::title(facts.as_ref().map(|(n, m, p)| (n.as_str(), *m, p.as_str())))
    }

    /// Sends the window title to the system when it has changed.
    fn update_window_title(&mut self, ctx: &egui::Context) {
        let title = self.window_title();
        if title != self.sent_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.sent_title = title;
        }
    }

    /// Draws one frame: the regions, the viewport, the dialogs and the work
    /// that keeps the jobs moving. The whole of the app's `ui`, apart from
    /// the eframe frame it does not use, so a test can run it.
    fn draw(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.edit_context = Some(ctx.clone());
        if self.recovery.is_closing() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.checkpoint_frame(&ctx);
            return;
        }
        if let Some(target) = crate::pending_edits::begin(&ctx) {
            let deferred_ctx = ctx.clone();
            self.defer(move |_| deferred_ctx.memory_mut(|m| m.request_focus(target)));
        }
        self.guard_close(&ctx);
        self.update_window_title(&ctx);
        self.poll_scene(&ctx);
        self.finish_pick(&ctx);
        self.overview_frame(&ctx);
        // Before anything is drawn, so a result is never drawn as current
        // in the frame after the change that made it out of date.
        self.follow_out_of_date();
        self.bench_frame(&ctx);
        if !self.recovery.blocks_input() {
            self.shortcuts(&ctx);
        }
        self.layout(ui);
        self.workholding_view_frame(&ctx);
        self.pick_frame(&ctx);
        self.run_frame(&ctx);
        self.import_frame(&ctx);
        self.dialogs(&ctx);
        self.finish_pending(&ctx);
        self.pending_dialog(&ctx);
        self.keep_simulate_settings();
        self.settings_frame(&ctx);
        if self.recovery_project.is_some()
            && self.dialog.is_none()
            && !self.edit_guard.active()
            && let Some((app, _, _)) = self.recovery_project.take()
        {
            self.recovery.cancel_install(app.location());
        }
        self.checkpoint_frame(&ctx);
        self.text_had_focus = ctx.egui_wants_keyboard_input();
        self.record_actions_frame(&ctx);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn req_imp_02_the_file_chooser_offers_step_and_stl() {
        let (name, extensions) = MODEL_FILTER;
        assert_eq!(name, "3D models (STL, STEP)");
        for wanted in ["stl", "STL", "step", "STEP", "stp", "STP"] {
            assert!(extensions.contains(&wanted), "{wanted} is not offered");
        }
    }

    /// The shortcuts pressed in one frame with `key` and `modifiers` down.
    fn press(key: Key, modifiers: Modifiers) -> (bool, bool, bool) {
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            events: vec![egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers,
            }],
            ..Default::default()
        };
        let mut pressed = (false, false, false);
        let mut output = ctx.run_ui(input, |ui| {
            pressed = ui.ctx().input_mut(edit_shortcuts);
        });
        output.textures_delta.clear();
        pressed
    }

    #[test]
    fn req_ui_08_ctrl_shift_z_redoes_and_ctrl_z_undoes() {
        assert_eq!(press(Key::Z, Modifiers::COMMAND), (false, true, false));
        assert_eq!(
            press(Key::Z, Modifiers::COMMAND | Modifiers::SHIFT),
            (false, false, true)
        );
        assert_eq!(press(Key::Y, Modifiers::COMMAND), (false, false, true));
        assert_eq!(press(Key::S, Modifiers::COMMAND), (true, false, false));
    }

    #[test]
    fn req_ui_09_a_run_up_to_the_last_operation_does_not_say_later_ones_stay_stale() {
        use std::sync::Arc;

        use crate::testing::cube_project;

        let (dir, mut app, op, _) = cube_project("run-summary");
        let mut run = openchuck_app::Regeneration {
            generated: vec![op],
            stopped_after: Some(op),
            project: Arc::new(app.project().clone()),
            ..Default::default()
        };
        // The only operation: nothing after it stays stale.
        assert_eq!(run_summary(&run), "Generated 1");
        // With an enabled operation after it, the summary says so.
        let copy = openchuck_app::duplicate_operation(app.project(), op).unwrap();
        app.execute(copy).unwrap();
        run.project = Arc::new(app.project().clone());
        assert_eq!(
            run_summary(&run),
            "Generated 1; stopped after operation 1, so later operations stay stale"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn req_ui_01_file_dialogs_start_in_an_existing_folder() {
        let dir = std::env::temp_dir();
        let text = dir.display().to_string();
        assert_eq!(existing_dir(&text), Some(dir.clone()));
        // A file that does not exist yet starts in its folder.
        let file = dir.join("not-there-yet.stl").display().to_string();
        assert_eq!(existing_dir(&file), Some(dir));
        assert_eq!(existing_dir("   "), None);
        assert_eq!(existing_dir("/no/such/folder/at/all"), None);
    }

    /// Runs one frame of the whole shell without a window.
    fn frame(shell: &mut Shell, ctx: &egui::Context) {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(1400.0, 900.0),
            )),
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| shell.draw(ui));
        output.textures_delta.clear();
    }

    /// Slice 9 review checklist, "the operation add, edit, generate and
    /// regenerate flow in the UI": every panel is drawn through a run, with
    /// no window and no GPU, and the run's results reach the viewport.
    #[test]
    fn req_ui_06_req_ui_09_the_shell_generates_shows_and_plays_a_run() {
        use std::time::{Duration, Instant};

        use crate::testing::cube_project;

        let (dir, app, op, _) = cube_project("shell-flow");
        app.save().unwrap();
        drop(app);
        let ctx = egui::Context::default();
        let mut shell = Shell::with_settings(false, Some(dir.clone()), None);
        // Every generation simulates the stock now (issue 214), and Simulate
        // prepares the cut part: about 8 s uninstrumented, several times that
        // in the instrumented coverage run, so the bound leaves room for it.
        let deadline = Instant::now() + Duration::from_secs(300);
        let wait = |shell: &mut Shell, done: &dyn Fn(&Shell) -> bool| {
            while !done(shell) {
                assert!(Instant::now() < deadline, "timed out");
                frame(shell, &ctx);
                std::thread::sleep(Duration::from_millis(5));
            }
        };
        // The scene is built, then the project tree and the footer draw.
        wait(&mut shell, &|s| s.scene_job.is_none());
        assert!(shell.viewport.view().picking.picker().is_some());
        // Generate and simulate: the run is a job.
        shell.generate(true);
        assert!(shell.run.is_running());
        frame(&mut shell, &ctx);
        wait(&mut shell, &|s| {
            !s.run.is_running()
                && s.viewport
                    .layer(openchuck_viewport::Layer::Paths)
                    .is_some_and(|p| !p.lines.is_empty())
        });
        wait(&mut shell, &|s| s.overview.stale.is_empty());
        let item = ItemRef::Operation(op);
        assert_eq!(
            shell.overview.status(item).map(|s| s.state),
            Some(openchuck_app::overview::ItemState::UpToDate)
        );
        let paths = shell
            .viewport
            .layer(openchuck_viewport::Layer::Paths)
            .unwrap();
        assert!(!paths.lines.is_empty());
        assert!(shell.play.open);
        // Every panel draws: the operation's, the selections', the results,
        // the add-operation dialog and the playback controls.
        shell.selected = Some(Selection::Operation(op));
        shell.run.show_results();
        shell.run.begin_add(shell.app.as_ref().unwrap(), None, None);
        frame(&mut shell, &ctx);
        shell.selected = Some(Selection::Picking);
        shell.open_step(Step::Simulate);
        frame(&mut shell, &ctx);
        // Editing makes the run out of date; generating again reuses nothing
        // that changed.
        let app = shell.app.as_mut().unwrap();
        app.set_parameter(
            op,
            "stepover",
            Some(openchuck_app::project::ParameterValue::Number(0.6)),
        )
        .unwrap();
        wait(&mut shell, &|s| !s.overview.stale.is_empty());
        assert_eq!(
            shell.overview.primary(Step::Operations).label,
            "Generate stale (1)"
        );
        shell.primary_on(Step::Operations);
        wait(&mut shell, &|s| {
            !s.run.is_running() && s.overview.stale.is_empty()
        });
        // The run's outcome is in the Jobs tab.
        assert!(shell.jobs.finished().any(|e| matches!(
            &e.outcome,
            Some(openchuck_app::JobOutcome::Done(Some(s))) if s.starts_with("Generated")
        )));
        // Cancelling a run leaves the old results alone.
        shell.generate(true);
        shell.run.cancel();
        wait(&mut shell, &|s| !s.run.is_running());
        let _ = std::fs::remove_dir_all(dir);
    }

    impl Shell {
        /// Presses the footer's primary button with `step` open.
        fn primary_on(&mut self, step: Step) {
            self.step = step;
            self.primary();
        }
    }
}

#[cfg(test)]
mod keys_tests;

#[cfg(test)]
mod four_step_tests;
#[cfg(test)]
mod frame_tests;
#[cfg(test)]
mod strategy_picker_tests;

#[cfg(test)]
mod numbox_tests;

#[cfg(test)]
mod new_project_tests;

#[cfg(test)]
mod simulate_tests;

#[cfg(test)]
mod smoke_tests;

#[cfg(test)]
mod tool_library_tests;

#[cfg(test)]
mod tooltip_tests;

#[cfg(test)]
mod import_layout_tests;
