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

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use eframe::egui::{self, Key, KeyboardShortcut, Modifiers};
use openchuck_app::overview::{
    Checks, ItemRef, Message, MessageAction, Overview, PrimaryKind, SetupPart, Severity, Step,
};
use openchuck_app::project::Item;
use openchuck_app::project::ItemId;
use openchuck_app::project::id::{ModelId, OperationId, SelectionSetId, SetupId, ToolId};
use openchuck_app::scene::SceneView;
use openchuck_app::settings::{SettingsFile, UserSettings};
use openchuck_app::tool_edit;
use openchuck_app::tools::{self, ToolProfile};
use openchuck_app::{AppError, Application, Command, JobHandle, JobLog, Threads};

use crate::bench;
use crate::export_panel::ExportPanel;
use crate::form_panel::{self, FormState};
use crate::import_panel::{ImportAction, ImportSession};
use crate::machine_panel::MachinePanel;
use crate::model_panel::ModelPanel;
use crate::playback_panel::PlaybackPanel;
use crate::run_panel::RunPanel;
use crate::selection_panel::{self, SelectionPanel};
use crate::theme::DARK;
use crate::tool_panel;
use crate::viewport::Viewport;
use crate::workholding_panel::WorkholdingPanel;

mod dock;
mod frame;
mod marks;
mod tree;

use dock::DockTab;

/// The starting text and control size. egui's own default reads small on
/// large desktop displays (owner's visual check, 14 Oct 2026; OI-166).
pub const DEFAULT_ZOOM: f32 = openchuck_app::settings::DEFAULT_SCALE;

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
/// Alt 1 to Alt 5 open the steps (specification section 4).
const STEP_KEYS: [Key; 5] = [Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5];
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
    /// The folder has no project: offer to create one there (decision record
    /// 0008 item 8).
    OfferCreate { folder: PathBuf },
    /// Ask for a model file to import, typed or picked (OI-127).
    ModelFile { path: String },
    /// The project has unsaved changes.
    Unsaved(Next),
    /// Rename an item (F2).
    Rename { item: ItemRef, name: String },
    /// The keyboard map (Help).
    Keys,
    /// About the program (Help).
    About,
    /// A command the user gave failed outright; the message says what
    /// (specification section 9: the only pop-up errors).
    Error(String),
}

/// The shell's state.
pub struct Shell {
    app: Option<Application>,
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
    /// A custom profile being edited, for this tool (TL-03).
    profile_draft: Option<(ToolId, ToolProfile)>,
    /// A model being imported; the viewport shows its preview meanwhile.
    import: Option<ImportSession>,
    /// The chosen model's panel, with its running check.
    model_panel: Option<ModelPanel>,
    /// The machine panel, with the profiles found when it was opened.
    machine_panel: Option<MachinePanel>,
    /// The chosen setup's workholding and tabs (SET-09, OP-21).
    workholding_panel: Option<WorkholdingPanel>,
    /// Typed text and refused values in the parameter form.
    form: FormState,
    /// Picking triangles and the selection sets (slice 10).
    selection: SelectionPanel,
    /// Adding operations and generating (slice 9 item 7).
    run: RunPanel,
    /// Post and export (slice 19 item 4).
    export: ExportPanel,
    /// A text field had the keyboard at the end of the last frame.
    text_had_focus: bool,
    /// The toolpath display and playback (slice 9).
    play: PlaybackPanel,
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
            app: None,
            viewport: Viewport::new(viewport_available),
            dialog: None,
            scene_job: None,
            setup: None,
            status: String::new(),
            quitting: false,
            bench: None,
            selected: None,
            setup_section: None,
            profile_draft: None,
            import: None,
            model_panel: None,
            machine_panel: None,
            workholding_panel: None,
            form: FormState::default(),
            selection: SelectionPanel::default(),
            run: RunPanel::default(),
            export: ExportPanel::default(),
            text_had_focus: false,
            play: PlaybackPanel::default(),
            step: Step::Model,
            jobs: JobLog::default(),
            checks: None,
            checks_job: None,
            overview: Overview::no_project(),
            overview_dirty: true,
            settings: loaded,
            settings_file: settings,
            settings_changed: None,
            settings_problem: problem,
            dock_tab: DockTab::Messages,
            message_filter: [true; 3],
            highlight: None,
            auto_collapsed: frame::AutoCollapse::default(),
            reset: None,
        };
        if let Some(folder) = folder {
            shell.open(folder);
        }
        shell
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
        match Application::create(&folder, name, Threads::All) {
            Ok(app) => self.set_app(app),
            Err(e) => {
                self.dialog = Some(Dialog::Error(format!(
                    "The project could not be created: {e}"
                )));
            }
        }
    }

    fn set_app(&mut self, app: Application) {
        self.status = format!("Opened {}", app.location().display());
        // Changed or missing model files are messages now (PRJ-02, OI-45;
        // specification section 9), from the project checks.
        self.selection.clear();
        self.run.clear();
        self.export.clear();
        self.play.clear();
        self.jobs.clear();
        self.checks = None;
        if let Some(job) = self.checks_job.take() {
            job.cancel();
        }
        self.setup = app.project().setups().first().map(|s| s.id);
        self.selected = None;
        self.setup_section = None;
        self.profile_draft = None;
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
    fn rebuild_scene(&mut self, fit: bool) {
        if let Some(app) = &self.app {
            if let Some((old, _)) = self.scene_job.take() {
                old.cancel();
            }
            let setup = self
                .setup
                .filter(|id| app.project().setups().iter().any(|s| s.id == *id));
            self.scene_job = Some((app.start_scene(setup), fit));
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
        if self.overview_dirty || self.overview.revision != revision {
            let mut overview = app.overview(self.checks.as_deref(), &self.jobs);
            for note in &self.viewport.view().notes {
                overview.push(Message::new(Severity::Info, note.clone(), None, None));
            }
            if let Some(problem) = &self.settings_problem {
                overview.push(Message::new(Severity::Warning, problem.clone(), None, None));
            }
            self.overview = overview;
            self.overview_dirty = false;
            self.highlight = None;
        }
    }

    /// Generates the selected operation, up to and including it, when an
    /// operation is selected (G, F5); otherwise every stale operation
    /// (specification section 16; decision record 0117, OI-351). A turned-off
    /// operation goes to the run too, which refuses it and says why (OI-352).
    fn generate_selected(&mut self) {
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
        if let Some(app) = &self.app {
            self.run.start(app, simulate);
            self.overview_dirty = true;
        }
    }

    fn save(&mut self) -> bool {
        let Some(app) = &self.app else { return true };
        match app.save() {
            Ok(()) => {
                self.status = format!("Saved {}", app.location().display());
                true
            }
            Err(e) => {
                self.dialog = Some(Dialog::Error(format!("The project was not saved: {e}")));
                false
            }
        }
    }

    fn undo(&mut self) {
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
        if self.app.as_ref().is_some_and(Application::is_modified) {
            self.dialog = Some(Dialog::Unsaved(next));
        } else {
            self.proceed(next, ctx);
        }
    }

    fn proceed(&mut self, next: Next, ctx: &egui::Context) {
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
        }
    }

    /// Opens the import dialog (Ctrl I, the Model "+", the workflow bar).
    fn import_model(&mut self) {
        if self.app.is_some() && self.import.is_none() {
            self.dialog = Some(Dialog::ModelFile {
                path: String::new(),
            });
        }
    }

    /// Opens the new-operation dialog on the Operations step (N, the
    /// Operations "+", the workflow bar).
    fn new_operation(&mut self) {
        if let Some(app) = &self.app
            && self.overview.step(Step::Operations).is_available()
        {
            self.step = Step::Operations;
            self.run.begin_add(app);
        }
    }

    /// Creates the project's one setup as a single undoable command, selects
    /// it and opens the Setup step (the Setups "+", the workflow bar;
    /// decision record 0144). With a setup already there it does nothing.
    fn add_setup(&mut self) {
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
        if self.app.is_some() {
            if self.overview.step(Step::Operations).is_available() {
                self.step = Step::Operations;
            }
            self.selection.show_set(None);
            self.dock_tab = DockTab::SelectionSets;
            self.settings.dock.collapsed = false;
            self.select(Selection::Picking);
        }
    }

    /// Opens `step` if it is available (UI-10).
    fn open_step(&mut self, step: Step) {
        if self.overview.step(step).is_available() {
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
        let action = self.overview.primary(self.step);
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
            PrimaryKind::GenerateStale(_) => self.generate(false),
            PrimaryKind::RunSimulation | PrimaryKind::GenerateAndSimulate(_) => {
                self.generate(true);
            }
            PrimaryKind::OpenExport => self.open_export(),
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        if self.dialog.is_some() {
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
        // Alt 1 to 5: the steps. Ctrl 1 to 3: collapse the panels.
        for (index, key) in STEP_KEYS.into_iter().enumerate() {
            if ctx.input_mut(|i| i.consume_key(Modifiers::ALT, key))
                && let Some(step) = Step::ALL.get(index)
            {
                self.open_step(*step);
            }
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
        if let Some(Selection::Operation(id)) = self.selected {
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
            Selection::Tool(id) => Some(ItemRef::Tool(id)),
            Selection::Model(id) => Some(ItemRef::Model(id)),
            Selection::Picking => None,
        }
    }

    /// Opens the rename dialog for `item`, if it has a name of its own.
    fn begin_rename(&mut self, item: ItemRef) {
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

    /// Renames `item` as one undoable command.
    fn rename(&mut self, item: ItemRef, name: &str) {
        let Some(app) = &mut self.app else { return };
        let name = name.trim();
        if name.is_empty() {
            self.dialog = Some(Dialog::Error("A name cannot be empty.".to_owned()));
            return;
        }
        let project = app.project();
        let replaced = match item {
            ItemRef::Operation(id) => project.operation(id).cloned().map(|mut o| {
                o.name = name.to_owned();
                Item::Operation(o)
            }),
            ItemRef::Setup(id) => project.setup(id).cloned().map(|mut s| {
                s.name = name.to_owned();
                Item::Setup(s)
            }),
            ItemRef::Tool(id) => {
                project
                    .tools()
                    .iter()
                    .find(|t| t.id == id)
                    .cloned()
                    .map(|mut t| {
                        t.name = name.to_owned();
                        Item::Tool(t)
                    })
            }
            ItemRef::SelectionSet(id) => project
                .selection_sets()
                .iter()
                .find(|s| s.id == id)
                .cloned()
                .map(|mut s| {
                    s.name = name.to_owned();
                    Item::SelectionSet(s)
                }),
            ItemRef::Model(_) | ItemRef::SetupPart(..) => None,
        };
        if let Some(item) = replaced
            && let Err(e) = app.execute(Command::Replace(item))
        {
            self.dialog = Some(Dialog::Error(format!("The name was not changed: {e}")));
        }
    }

    fn open_export(&mut self) {
        if let Some(app) = &self.app {
            self.export.open(app);
        }
    }

    /// Carries out a message's action link (specification section 9).
    fn message_action(&mut self, action: MessageAction) {
        match action {
            MessageAction::Show(item) => self.show_item(item),
            MessageAction::Reselect(set) => {
                self.start_picking();
                self.selection.show_set(Some(set));
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
        }
    }

    /// Selects `item` in the tree and the inspector (a message's source).
    fn show_item(&mut self, item: ItemRef) {
        match item {
            ItemRef::Model(id) => self.select(Selection::Model(id)),
            ItemRef::Setup(id) => self.choose_setup(id, None),
            ItemRef::SetupPart(id, part) => self.choose_setup(id, Some(part)),
            ItemRef::SelectionSet(id) => {
                self.selection.show_set(Some(id));
                self.select(Selection::Set(id));
            }
            ItemRef::Tool(id) => self.select(Selection::Tool(id)),
            ItemRef::Operation(id) => self.select(Selection::Operation(id)),
        }
        self.settings.inspector.collapsed = false;
    }

    /// Chooses `setup` (the view shows it) and opens its inspector at
    /// `section`.
    fn choose_setup(&mut self, setup: SetupId, section: Option<SetupPart>) {
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
        let command = match action {
            TreeAction::Select(id) => {
                self.select(Selection::Operation(id));
                return;
            }
            TreeAction::SelectTool(id) => {
                self.select(Selection::Tool(id));
                return;
            }
            TreeAction::SelectModel(id) => {
                self.select(Selection::Model(id));
                return;
            }
            TreeAction::SelectSet(id) => {
                self.selection.show_set(Some(id));
                self.dock_tab = DockTab::SelectionSets;
                self.select(Selection::Set(id));
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
                    // The tool library window is lane X9's (OI-343);
                    // tools are added from the command line until then.
                    tree::Group::Setups => self.add_setup(),
                    tree::Group::Tools => {}
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
        if self.selected != Some(selection) {
            self.selected = Some(selection);
            self.form.clear();
        }
    }

    /// The inspector: the chosen item (specification section 8; its header,
    /// banner and sections are lane X4's).
    fn properties(&mut self, ui: &mut egui::Ui) {
        match self.selected {
            Some(Selection::Operation(id)) => self.operation_properties(ui, id),
            Some(Selection::Tool(id)) => self.tool_properties(ui, id),
            Some(Selection::Model(id)) => self.model_properties(ui, id),
            Some(Selection::Setup(id)) => self.setup_properties(ui, id),
            Some(Selection::Set(id)) => self.set_properties(ui, id),
            Some(Selection::Picking) => {
                ui.heading("Selecting on the part");
                ui.label(
                    "Click the part to select. The Selection sets tab of the dock saves and \
                     edits sets; Esc stops picking.",
                );
            }
            None => {
                ui.weak("Choose an item in the project panel to see it here.");
            }
        }
    }

    /// A selection set: its size, its problem and where it is used.
    fn set_properties(&mut self, ui: &mut egui::Ui, id: SelectionSetId) {
        let Some(app) = &self.app else { return };
        let summaries = app.selection_summaries(&[]);
        let Some(s) = summaries.iter().find(|s| s.id == id) else {
            self.selected = Some(Selection::Picking);
            return;
        };
        ui.heading(&s.name);
        frame::status_chip(ui, self.overview.status(ItemRef::SelectionSet(id)));
        ui.label(format!("{} {}", s.count, s.kind));
        if let Some(path) = &s.model_path {
            ui.weak(format!("On {path}"));
        }
        if let Some(problem) = &s.problem {
            ui.colored_label(DARK.error, format!("Cannot be used: {problem}."));
        }
        if s.uses.is_empty() {
            ui.weak("No operation uses it.");
        } else {
            for u in &s.uses {
                ui.label(format!("Used by {}", u.name));
            }
        }
    }

    /// The setup inspector: its five sections (specification section 11.3).
    /// The Machine and Workholding sections are the existing panels; the
    /// fields of Stock, Part position and Work offset and clearance are lane
    /// X4's (decision record 0114 C15), so they show the setup's values.
    fn setup_properties(&mut self, ui: &mut egui::Ui, id: SetupId) {
        let Some(app) = &mut self.app else { return };
        let Some(setup) = app.project().setup(id).cloned() else {
            self.selected = None;
            return;
        };
        ui.heading(&setup.name);
        frame::status_chip(ui, self.overview.status(ItemRef::Setup(id)));
        let focus = self.setup_section.take();
        let mut command = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            for part in SetupPart::ALL {
                let row = openchuck_app::overview::setup_row(app.project(), &setup, part);
                let key = section_key(part);
                let open = self.settings.section_open("setup", key, true);
                let mut header = egui::CollapsingHeader::new(
                    egui::RichText::new(section_title(part).to_uppercase())
                        .small()
                        .strong()
                        .color(DARK.heading),
                )
                .id_salt(("setup section", key))
                .default_open(open);
                if focus == Some(part) {
                    header = header.open(Some(true));
                }
                let response = header.show(ui, |ui| match part {
                    SetupPart::Machine => {
                        let panel = self.machine_panel.get_or_insert_with(MachinePanel::default);
                        let copy = app.project().machine().cloned();
                        let copy_id = app.project().machine_id().map(str::to_owned);
                        if let Some(c) = panel.show(ui, copy.as_ref(), copy_id.as_deref()) {
                            command = Some(c);
                        }
                    }
                    SetupPart::Workholding => {
                        let panel = self
                            .workholding_panel
                            .get_or_insert_with(|| WorkholdingPanel::new(&setup));
                        if let Some(c) = panel.show(ui, app, &setup) {
                            command = Some(c);
                        }
                    }
                    _ => {
                        ui.label(&row.label);
                        if let Some(chip) = &row.chip {
                            ui.label(chip);
                        }
                    }
                });
                if focus == Some(part) {
                    response
                        .header_response
                        .scroll_to_me(Some(egui::Align::TOP));
                }
                let now_open = response.body_returned.is_some();
                if now_open != open && focus != Some(part) {
                    self.settings
                        .set_section_open("setup", key, now_open, &SECTION_KEYS);
                    self.settings_changed.get_or_insert_with(Instant::now);
                }
            }
        });
        if let Some(command) = command {
            match app.execute(command) {
                Ok(()) => self.rebuild_scene(false),
                Err(e) => self.dialog = Some(Dialog::Error(e.to_string())),
            }
        }
    }

    /// A model's units, placement and checks (IMP-03, IMP-04, IMP-05).
    fn model_properties(&mut self, ui: &mut egui::Ui, id: ModelId) {
        let Some(app) = &mut self.app else { return };
        let Some(reference) = app.project().models().iter().find(|m| m.id == id).cloned() else {
            self.selected = None;
            return;
        };
        frame::status_chip(ui, self.overview.status(ItemRef::Model(id)));
        let panel = match &mut self.model_panel {
            Some(p) if p.model() == id => p,
            _ => self.model_panel.insert(ModelPanel::new(id)),
        };
        let command = egui::ScrollArea::vertical()
            .show(ui, |ui| panel.show(ui, app, &reference))
            .inner;
        if let Some(command) = command {
            match app.execute(command) {
                Ok(()) => self.rebuild_scene(false),
                Err(e) => self.dialog = Some(Dialog::Error(e.to_string())),
            }
        }
    }

    /// A tool's dimensions and its profile editor (UI-07, TL-03).
    fn tool_properties(&mut self, ui: &mut egui::Ui, id: ToolId) {
        let Some(app) = &mut self.app else { return };
        let Some(snapshot) = app.project().tools().iter().find(|t| t.id == id).cloned() else {
            self.selected = None;
            return;
        };
        ui.heading(&snapshot.name);
        ui.weak(tools::describe(&snapshot.tool));
        ui.separator();
        let fields = tool_edit::tool_form(&snapshot.tool);
        let edit = egui::ScrollArea::vertical()
            .show(ui, |ui| {
                let edit =
                    form_panel::show_fields(ui, "tool form", &fields, &[], false, &mut self.form);
                ui.separator();
                ui.strong("Profile");
                let mut action = tool_panel::ProfileAction::None;
                match &mut self.profile_draft {
                    Some((tool, draft)) if *tool == id => {
                        action = tool_panel::editor(ui, &snapshot.tool, draft);
                    }
                    _ => {
                        let label =
                            if matches!(snapshot.tool.shape, tools::ToolShape::Custom { .. }) {
                                "Edit the custom profile…"
                            } else {
                                "Make a custom profile from this shape…"
                            };
                        if ui.button(label).clicked()
                            && let Some(p) = tool_edit::starting_profile(&snapshot.tool)
                        {
                            self.profile_draft = Some((id, p));
                        }
                    }
                }
                (edit, action)
            })
            .inner;
        let (edit, action) = edit;
        if let Some(edit) = edit {
            // A tool dimension cannot be cleared: say so rather than leave the
            // box looking empty while the tool keeps its value.
            let result = match &edit.value {
                Some(value) => app
                    .set_tool_field(id, &edit.name, value)
                    .map_err(|e| e.to_string()),
                None => Err("a tool dimension needs a value".to_owned()),
            };
            self.form.result(&edit.name, result);
        }
        match action {
            tool_panel::ProfileAction::Apply => {
                if let Some((_, draft)) = self.profile_draft.take()
                    && let Err(e) = app.set_tool_profile(id, draft.clone())
                {
                    self.profile_draft = Some((id, draft));
                    self.dialog = Some(Dialog::Error(e.to_string()));
                }
            }
            tool_panel::ProfileAction::Cancel => self.profile_draft = None,
            tool_panel::ProfileAction::None => {}
        }
    }

    /// An operation's parameter form (UI-07).
    fn operation_properties(&mut self, ui: &mut egui::Ui, id: OperationId) {
        let mut delete = false;
        let Some(app) = &mut self.app else { return };
        let Some(operation) = app.project().operation(id) else {
            self.selected = None;
            return;
        };
        let setup_of = operation.setup;
        ui.horizontal(|ui| {
            ui.heading(&operation.name);
            if ui
                .small_button("Delete operation")
                .on_hover_text("Remove this operation (it can be undone)")
                .clicked()
            {
                delete = true;
            }
        });
        frame::status_chip(ui, self.overview.status(ItemRef::Operation(id)));
        let summaries = app.selection_summaries(&[]);
        let roles = selection_panel::operation_roles(ui, app, id, &summaries);
        // Selections that reach a held region or a tab: from the project
        // checks, also listed in Messages (11.7).
        let mut edit_workholding = false;
        let reach: Vec<String> = self
            .overview
            .messages
            .iter()
            .filter(|m| {
                m.source == Some(ItemRef::Operation(id))
                    && matches!(m.action, Some(MessageAction::EditWorkholding(_)))
            })
            .map(|m| m.text.clone())
            .collect();
        for note in reach {
            ui.horizontal_wrapped(|ui| {
                ui.colored_label(DARK.warning, format!("\u{26a0} {note}"));
                if ui.small_button("Edit workholding").clicked() {
                    edit_workholding = true;
                }
            });
        }
        ui.separator();
        let edit = match app.parameter_form(id) {
            Ok(form) => {
                ui.weak(format!("Strategy: {}", app.strategy_label(&form.strategy)));
                ui.separator();
                egui::ScrollArea::vertical()
                    .show(ui, |ui| form_panel::show(ui, &form, &mut self.form))
                    .inner
            }
            Err(e) => {
                ui.label(e.to_string());
                None
            }
        };
        if let Some(edit) = edit {
            let result = app
                .set_parameter(id, &edit.name, edit.value)
                .map_err(|e| e.user_message());
            self.form.result(&edit.name, result);
        }
        if let Some(error) = roles {
            self.dialog = Some(Dialog::Error(error));
        }
        if delete {
            self.tree_action(TreeAction::Delete(id));
        } else if edit_workholding {
            // An unapplied draft of this setup stays (11.7); the view shows
            // the setup the panel edits.
            self.choose_setup(setup_of, Some(SetupPart::Workholding));
        }
    }

    /// Works the generating job, the add-operation dialog and the playback
    /// (slice 9).
    fn run_frame(&mut self, ctx: &egui::Context) {
        let mut error = None;
        let mut added = None;
        if let Some(app) = &mut self.app {
            if let Some((run, job)) = self.run.poll(&self.jobs, ctx) {
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
            if let Some(status) = outcome.status {
                self.status = status;
            }
            error = error.or(outcome.error);
            let dt = f64::from(ctx.input(|i| i.stable_dt)).min(0.25);
            if self.play.frame(app, &mut self.viewport, dt) {
                ctx.request_repaint();
            }
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
        let Some(app) = &self.app else { return };
        if self.selection.frame(app, self.viewport.view(), input) {
            ctx.request_repaint();
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
        // The selected operation's avoid and check sets are drawn hatched (7.4).
        self.selection.show_roles_of(
            app,
            match self.selected {
                Some(Selection::Operation(id)) => Some(id),
                _ => None,
            },
        );
        if let Some(overlay) = self.selection.overlay(app, self.viewport.view()) {
            self.viewport.set_overlay(overlay);
            ctx.request_repaint();
        }
        if let Some(app) = &mut self.app {
            // A save or an edit that ended, whether or not its tab is shown.
            if let Some(e) = self.selection.poll(app) {
                self.dialog = Some(Dialog::Error(e));
            }
            // The warning before a click of the other kind resets the
            // selection.
            if let Some(e) = self.selection.reset_dialog(ctx, app) {
                self.dialog = Some(Dialog::Error(e));
            }
        }
    }

    /// Whether a dialog or a floating window (export, results, add operation)
    /// is in front of the view.
    fn window_open(&self) -> bool {
        self.dialog.is_some() || self.export.is_open() || self.run.has_window()
    }

    /// Runs the import dialog (OI-51): polls its check, shows the preview in
    /// the viewport, and adds the model or closes.
    fn import_frame(&mut self, ctx: &egui::Context) {
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

    fn dialogs(&mut self, ctx: &egui::Context) {
        let Some(dialog) = self.dialog.clone() else {
            return;
        };
        let mut close = false;
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
                ui.label("Project folder:");
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut folder);
                    if ui.button("Browse…").clicked()
                        && let Some(picked) = pick_folder(&folder)
                    {
                        folder = picked;
                    }
                });
                if create {
                    ui.label("Project name:");
                    ui.text_edit_singleline(&mut name);
                }
                ui.horizontal(|ui| {
                    let ok = ui
                        .add_enabled(!folder.trim().is_empty(), egui::Button::new("OK"))
                        .clicked();
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                    if ok {
                        let path = PathBuf::from(folder.trim());
                        let name = name.trim().to_owned();
                        close = true;
                        action = Some(if create {
                            Box::new(move |s: &mut Self| s.create(path, &name))
                        } else {
                            Box::new(move |s: &mut Self| s.open(path))
                        });
                    }
                });
                if !close {
                    self.dialog = Some(Dialog::Folder {
                        create,
                        folder,
                        name,
                    });
                }
            }
            Dialog::ModelFile { mut path } => {
                ui.heading("Import model");
                ui.label("Model file (STL), inside or beside the project folder:");
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut path);
                    if ui.button("Browse…").clicked()
                        && let Some(picked) = pick_model(&path, self.app.as_ref())
                    {
                        path = picked;
                    }
                });
                ui.horizontal(|ui| {
                    let ok = ui
                        .add_enabled(!path.trim().is_empty(), egui::Button::new("Check"))
                        .clicked();
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                    if ok {
                        close = true;
                        let file = PathBuf::from(path.trim());
                        action = Some(Box::new(move |s: &mut Self| {
                            if let Some(app) = &s.app {
                                s.import = Some(ImportSession::new(app, file));
                            }
                        }));
                    }
                });
                if !close {
                    self.dialog = Some(Dialog::ModelFile { path });
                }
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
                let field = ui.text_edit_singleline(&mut name);
                let enter = field.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                // The field has the keyboard when the dialog opens.
                if !enter && ui.memory(|m| m.focused().is_none()) {
                    field.request_focus();
                }
                ui.horizontal(|ui| {
                    if ui.button("Rename").clicked() || enter {
                        close = true;
                        let name = name.clone();
                        action = Some(Box::new(move |s: &mut Self| s.rename(item, &name)));
                    }
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                });
                if !close {
                    self.dialog = Some(Dialog::Rename { item, name });
                }
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
                ui.heading("OpenChuckCAM");
                ui.label(format!("Version {}", env!("CARGO_PKG_VERSION")));
                ui.label("Free software under the GNU Affero General Public License, version 3 or later.");
                ui.label(openchuck_app::VERIFY_GCODE_NOTICE);
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
        if close {
            self.dialog = None;
            // A field of the closed dialog must not keep the keyboard.
            ctx.memory_mut(egui::Memory::stop_text_input);
        }
        if let Some(action) = action {
            action(self);
        }
    }

    /// Stops the window closing while there are unsaved changes, and asks.
    fn guard_close(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.viewport().close_requested()) {
            if !self.quitting && self.app.as_ref().is_some_and(Application::is_modified) {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.dialog = Some(Dialog::Unsaved(Next::Quit));
            } else {
                self.save_settings();
            }
        }
    }
}

/// The settings key of a setup inspector section.
fn section_key(part: SetupPart) -> &'static str {
    match part {
        SetupPart::Machine => "machine",
        SetupPart::Stock => "stock",
        SetupPart::PartPosition => "part position",
        SetupPart::WorkOffset => "work offset",
        SetupPart::Workholding => "workholding",
    }
}

/// The setup sections open by default: all of them.
const SECTION_KEYS: [&str; 5] = [
    "machine",
    "part position",
    "stock",
    "work offset",
    "workholding",
];

/// A setup inspector section's title (specification section 11.3).
fn section_title(part: SetupPart) -> &'static str {
    match part {
        SetupPart::Machine => "Machine",
        SetupPart::Workholding => "Workholding and tabs",
        other => other.name(),
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
fn pick_folder(current: &str) -> Option<String> {
    let mut dialog = rfd::FileDialog::new().set_title("Project folder");
    if let Some(start) = existing_dir(current) {
        dialog = dialog.set_directory(start);
    }
    dialog.pick_folder().map(|p| p.display().to_string())
}

/// Asks the platform's file dialog for a model file, starting near `current`
/// or in the project folder (OI-127).
fn pick_model(current: &str, app: Option<&Application>) -> Option<String> {
    let mut dialog = rfd::FileDialog::new()
        .set_title("Import model")
        .add_filter("STL model", &["stl", "STL"]);
    let start = existing_dir(current).or_else(|| app.map(|a| a.location().to_path_buf()));
    if let Some(start) = start {
        dialog = dialog.set_directory(start);
    }
    dialog.pick_file().map(|p| p.display().to_string())
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
    /// Show this operation's parameters.
    Select(OperationId),
    /// Show this tool's dimensions and profile.
    SelectTool(ToolId),
    /// Show this model's units, placement and checks.
    SelectModel(ModelId),
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
    Tool(ToolId),
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
    /// Draws one frame: the regions, the viewport, the dialogs and the work
    /// that keeps the jobs moving. The whole of the app's `ui`, apart from
    /// the eframe frame it does not use, so a test can run it.
    fn draw(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.guard_close(&ctx);
        self.poll_scene(&ctx);
        self.overview_frame(&ctx);
        self.bench_frame(&ctx);
        self.shortcuts(&ctx);
        self.layout(ui);
        self.pick_frame(&ctx);
        self.run_frame(&ctx);
        self.import_frame(&ctx);
        self.dialogs(&ctx);
        self.settings_frame(&ctx);
        self.text_had_focus = ctx.egui_wants_keyboard_input();
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

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
        let deadline = Instant::now() + Duration::from_secs(120);
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
        shell.run.begin_add(shell.app.as_ref().unwrap());
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
mod frame_tests;
