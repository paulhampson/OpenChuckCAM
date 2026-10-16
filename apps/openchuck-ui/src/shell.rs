// SPDX-License-Identifier: AGPL-3.0-or-later
//! The application shell: menus, the project tree, new, open and save with
//! unsaved-change handling, undo and redo, and the viewport (UI-01, UI-06,
//! UI-08; work breakdown slice 8 items 3, 4 and 6; decision record 0029).
//!
//! Every project change goes through `openchuck-app`; the shell holds no CAM
//! logic. Paths can be typed, or picked with the platform's own dialog
//! through `rfd` (OI-127); the typed path stays when no dialog is available.

use std::path::PathBuf;

use eframe::egui::{self, Key, KeyboardShortcut, Modifiers};
use openchuck_app::project::ItemId;
use openchuck_app::project::id::{ModelId, OperationId, SelectionSetId, SetupId, ToolId};
use openchuck_app::scene::SceneView;
use openchuck_app::tool_edit;
use openchuck_app::tools::{self, ToolProfile};
use openchuck_app::{AppError, Application, Command, JobHandle, Threads};
use openchuck_viewport::{DisplayMode, Projection, StandardView};

use crate::bench;
use crate::form_panel::{self, FormState};
use crate::import_panel::{ImportAction, ImportSession};
use crate::machine_panel::MachinePanel;
use crate::model_panel::ModelPanel;
use crate::playback_panel::PlaybackPanel;
use crate::run_panel::RunPanel;
use crate::selection_panel::{self, SelectionPanel};
use crate::tool_panel;
use crate::viewport::Viewport;

/// The starting text and control size. egui's own default reads small on
/// large desktop displays (owner's visual check, 14 Oct 2026).
pub const DEFAULT_ZOOM: f32 = 1.2;

const SAVE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::S);
const UNDO: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Z);
const REDO: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::Z);
const REDO_ALT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Y);
const GENERATE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::NONE, Key::F5);
const SIMULATE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::SHIFT, Key::F5);

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
    /// Something failed; the message says what.
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
    /// The operation or tool shown in the properties panel.
    selected: Option<Selection>,
    /// A custom profile being edited, for this tool (TL-03).
    profile_draft: Option<(ToolId, ToolProfile)>,
    /// A model being imported; the viewport shows its preview meanwhile.
    import: Option<ImportSession>,
    /// The chosen model's panel, with its running check.
    model_panel: Option<ModelPanel>,
    /// The machine panel, with the profiles found when it was opened.
    machine_panel: Option<MachinePanel>,
    /// Typed text and refused values in the parameter form.
    form: FormState,
    /// Picking triangles and the selection sets (slice 10).
    selection: SelectionPanel,
    /// Adding operations and generating (slice 9 item 7).
    run: RunPanel,
    /// The toolpath display and playback (slice 9).
    play: PlaybackPanel,
    /// What `model_file_warnings` said when the project opened, for the
    /// selection warnings; reading every model file is not done per frame.
    file_warnings: Vec<String>,
}

impl Shell {
    /// A shell with no project open. `viewport_available` is false when
    /// eframe runs without wgpu.
    pub fn new(viewport_available: bool, folder: Option<PathBuf>) -> Self {
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
            profile_draft: None,
            import: None,
            model_panel: None,
            machine_panel: None,
            form: FormState::default(),
            selection: SelectionPanel::default(),
            run: RunPanel::default(),
            play: PlaybackPanel::default(),
            file_warnings: Vec::new(),
        };
        if let Some(folder) = folder {
            shell.open(folder);
        }
        shell
    }

    /// A shell running the frame-time benchmark on `app`'s scene.
    pub fn benchmark(app: Application, run: bench::Run) -> Self {
        let mut shell = Self::new(true, None);
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
            Err(e) => self.dialog = Some(Dialog::Error(e.to_string())),
        }
    }

    fn create(&mut self, folder: PathBuf, name: &str) {
        match Application::create(&folder, name, Threads::All) {
            Ok(app) => self.set_app(app),
            Err(e) => self.dialog = Some(Dialog::Error(e.to_string())),
        }
    }

    fn set_app(&mut self, app: Application) {
        self.status = format!("Opened {}", app.location().display());
        // Model files changed or missing since they were added (PRJ-02, OI-45).
        let warnings = app.model_file_warnings();
        self.file_warnings.clone_from(&warnings);
        self.selection.clear();
        self.run.clear();
        self.play.clear();
        if !warnings.is_empty() {
            self.dialog = Some(Dialog::Error(format!(
                "Some model files do not match the project:\n{}\n\nThe toolpaths will use the files \
                 as they are now.",
                warnings.join("\n")
            )));
        }
        self.setup = app.project().setups().first().map(|s| s.id);
        self.selected = None;
        self.profile_draft = None;
        self.import = None;
        self.form.clear();
        self.app = Some(app);
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
                    }
                    Err(e) if e.is_cancelled() => {}
                    Err(e) => self.status = format!("The view could not be built: {e}"),
                }
            }
        }
    }

    /// Starts generating, and simulating when asked (UI-09).
    fn generate(&mut self, simulate: bool) {
        if let Some(app) = &self.app {
            self.run.start(app, simulate);
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
                self.quitting = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
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
        // Shift+F5 first: it would otherwise be taken for F5.
        let (simulate, generate) =
            ctx.input_mut(|i| (i.consume_shortcut(&SIMULATE), i.consume_shortcut(&GENERATE)));
        if simulate {
            self.generate(true);
        } else if generate {
            self.generate(false);
        }
    }

    fn menu(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("New project…").clicked() {
                    self.request(Next::New, &ctx);
                }
                if ui.button("Open project…").clicked() {
                    self.request(Next::Open, &ctx);
                }
                if ui
                    .add_enabled(
                        self.app.is_some() && self.import.is_none(),
                        egui::Button::new("Import model…"),
                    )
                    .clicked()
                {
                    self.dialog = Some(Dialog::ModelFile {
                        path: String::new(),
                    });
                }
                let save = egui::Button::new("Save").shortcut_text(ctx.format_shortcut(&SAVE));
                if ui.add_enabled(self.app.is_some(), save).clicked() {
                    self.save();
                }
                ui.separator();
                if ui.button("Quit").clicked() {
                    self.request(Next::Quit, &ctx);
                }
            });
            ui.menu_button("Edit", |ui| {
                let (can_undo, can_redo) = self
                    .app
                    .as_ref()
                    .map_or((false, false), |a| (a.can_undo(), a.can_redo()));
                let undo = egui::Button::new("Undo").shortcut_text(ctx.format_shortcut(&UNDO));
                if ui.add_enabled(can_undo, undo).clicked() {
                    self.undo();
                }
                let redo = egui::Button::new("Redo").shortcut_text(ctx.format_shortcut(&REDO));
                if ui.add_enabled(can_redo, redo).clicked() {
                    self.redo();
                }
            });
            ui.menu_button("Operations", |ui| {
                let open = self.app.is_some();
                let idle = !self.run.is_running();
                if ui
                    .add_enabled(open, egui::Button::new("Add operation…"))
                    .clicked()
                {
                    if let Some(app) = &self.app {
                        self.run.begin_add(app);
                    }
                    ui.close();
                }
                ui.separator();
                let generate =
                    egui::Button::new("Generate").shortcut_text(ctx.format_shortcut(&GENERATE));
                if ui.add_enabled(open && idle, generate).clicked() {
                    self.generate(false);
                    ui.close();
                }
                let simulate = egui::Button::new("Generate and simulate")
                    .shortcut_text(ctx.format_shortcut(&SIMULATE));
                if ui.add_enabled(open && idle, simulate).clicked() {
                    self.generate(true);
                    ui.close();
                }
                if ui.add_enabled(!idle, egui::Button::new("Cancel")).clicked() {
                    self.run.cancel();
                    ui.close();
                }
                ui.separator();
                if ui
                    .add_enabled(open, egui::Button::new("Results…"))
                    .clicked()
                {
                    self.run.show_results();
                    ui.close();
                }
                ui.checkbox(&mut self.play.open, "Show playback");
            });
            ui.menu_button("View", |ui| {
                for view in StandardView::ALL {
                    if ui.button(view.name()).clicked() {
                        self.viewport.set_view(view);
                    }
                }
                if ui.button("Fit all").clicked() {
                    self.viewport.fit();
                }
                ui.separator();
                let mut projection = self.viewport.camera.projection;
                ui.radio_value(&mut projection, Projection::Perspective, "Perspective");
                ui.radio_value(&mut projection, Projection::Orthographic, "Orthographic");
                self.viewport.set_projection(projection);
                ui.separator();
                for mode in DisplayMode::ALL {
                    ui.radio_value(&mut self.viewport.mode, mode, mode.name());
                }
                ui.separator();
                // Text and controls scale together (egui's zoom; Ctrl and +
                // or - also work).
                let zoom = ctx.zoom_factor();
                ui.label(format!("Text size: {:.0} %", zoom * 100.0));
                ui.horizontal(|ui| {
                    if ui.button("Larger").clicked() {
                        ctx.set_zoom_factor((zoom * 1.1).min(3.0));
                    }
                    if ui.button("Smaller").clicked() {
                        ctx.set_zoom_factor((zoom / 1.1).max(0.5));
                    }
                    if ui.button("Reset").clicked() {
                        ctx.set_zoom_factor(DEFAULT_ZOOM);
                    }
                });
            });
        });
    }

    /// The project tree (UI-06): models, setups, operations and tools.
    /// Choosing a setup shows it in the viewport. Operations can be enabled,
    /// disabled, moved and duplicated, each through an undoable command.
    fn tree(&mut self, ui: &mut egui::Ui) {
        let Some(app) = &self.app else {
            ui.label("No project open. Use File, Open project or New project.");
            return;
        };
        let project = app.project();
        let mut chosen = None;
        let mut add_operation = false;
        let mut command: Option<TreeAction> = None;
        ui.heading(project.name());
        let machine = project.machine().map_or_else(
            || "Machine: none".to_owned(),
            |m| format!("Machine: {}", m.name),
        );
        if ui
            .selectable_label(self.selected == Some(Selection::Machine), machine)
            .clicked()
        {
            command = Some(TreeAction::SelectMachine);
        }
        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::CollapsingHeader::new(format!("Models ({})", project.models().len()))
                .default_open(true)
                .show(ui, |ui| {
                    for m in project.models() {
                        let selected = self.selected == Some(Selection::Model(m.id));
                        if ui.selectable_label(selected, m.path.as_str()).clicked() {
                            command = Some(TreeAction::SelectModel(m.id));
                        }
                    }
                });
            egui::CollapsingHeader::new(format!("Setups ({})", project.setups().len()))
                .default_open(true)
                .show(ui, |ui| {
                    for s in project.setups() {
                        if ui
                            .selectable_label(self.setup == Some(s.id), &s.name)
                            .clicked()
                        {
                            chosen = Some(s.id);
                        }
                    }
                });
            let summaries = app.selection_summaries(&self.file_warnings);
            egui::CollapsingHeader::new(format!("Selections ({})", summaries.len()))
                .default_open(true)
                .show(ui, |ui| {
                    let on_part = self.selected == Some(Selection::Picking);
                    if ui
                        .selectable_label(on_part, "Select on the part…")
                        .on_hover_text("Pick triangles in the view and save them as sets")
                        .clicked()
                    {
                        command = Some(TreeAction::SelectPicking);
                    }
                    for s in &summaries {
                        let selected = self.selected == Some(Selection::Set(s.id));
                        let text = egui::RichText::new(format!("{} ({})", s.name, s.count));
                        let text = if s.problem.is_some() {
                            text.color(egui::Color32::from_rgb(230, 80, 80))
                        } else {
                            text
                        };
                        let row = ui.selectable_label(selected, text);
                        if let Some(problem) = &s.problem {
                            row.clone().on_hover_text(format!("⚠ {problem}"));
                        }
                        if row.clicked() {
                            command = Some(TreeAction::SelectSet(s.id));
                        }
                    }
                });
            egui::CollapsingHeader::new(format!("Operations ({})", project.operations().len()))
                .default_open(true)
                .show(ui, |ui| {
                    if ui.small_button("Add operation…").clicked() {
                        add_operation = true;
                    }
                    let count = project.operations().len();
                    for (index, o) in project.operations().iter().enumerate() {
                        let warning = summaries
                            .iter()
                            .find(|s| {
                                s.problem.is_some() && s.uses.iter().any(|u| u.operation == o.id)
                            })
                            .map(|s| {
                                format!("{}: {}", s.name, s.problem.clone().unwrap_or_default())
                            });
                        if let Some(c) = operation_row(
                            ui,
                            index,
                            count,
                            o,
                            self.selected == Some(Selection::Operation(o.id)),
                            warning.as_deref(),
                            self.run.badge(app, o.id),
                        ) {
                            command = Some(c);
                        }
                    }
                });
            egui::CollapsingHeader::new(format!("Tools ({})", project.tools().len()))
                .default_open(false)
                .show(ui, |ui| {
                    for t in project.tools() {
                        let selected = self.selected == Some(Selection::Tool(t.id));
                        if ui.selectable_label(selected, &t.name).clicked() {
                            command = Some(TreeAction::SelectTool(t.id));
                        }
                    }
                });
        });
        if add_operation {
            self.run.begin_add(app);
        }
        if let Some(id) = chosen
            && self.setup != Some(id)
        {
            self.setup = Some(id);
            self.rebuild_scene(true);
        }
        if let Some(action) = command {
            self.tree_action(action);
        }
    }

    /// Runs a project tree action through a command, so it can be undone.
    fn tree_action(&mut self, action: TreeAction) {
        let Some(app) = &mut self.app else { return };
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
            TreeAction::SelectPicking => {
                self.select(Selection::Picking);
                return;
            }
            TreeAction::SelectSet(id) => {
                self.selection.show_set(Some(id));
                self.select(Selection::Set(id));
                return;
            }
            TreeAction::SelectMachine => {
                if self.selected != Some(Selection::Machine) {
                    self.machine_panel = Some(MachinePanel::default());
                }
                self.select(Selection::Machine);
                return;
            }
            TreeAction::Command(c) => Ok(*c),
            TreeAction::Duplicate(id) => openchuck_app::duplicate_operation(app.project(), id),
        };
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

    /// The chosen operation's parameters or tool's dimensions (UI-07).
    fn properties(&mut self, ui: &mut egui::Ui) {
        match self.selected {
            Some(Selection::Operation(id)) => self.operation_properties(ui, id),
            Some(Selection::Tool(id)) => self.tool_properties(ui, id),
            Some(Selection::Model(id)) => self.model_properties(ui, id),
            Some(Selection::Machine) => self.machine_properties(ui),
            Some(Selection::Picking | Selection::Set(_)) => self.selection_properties(ui),
            None => {}
        }
    }

    /// Picking triangles and the selection sets (UI-04, UI-05).
    fn selection_properties(&mut self, ui: &mut egui::Ui) {
        let Some(app) = &mut self.app else { return };
        let summaries = app.selection_summaries(&self.file_warnings);
        if let Some(Selection::Set(id)) = self.selected
            && !summaries.iter().any(|s| s.id == id)
        {
            self.selected = Some(Selection::Picking);
        }
        let error = egui::ScrollArea::vertical()
            .show(ui, |ui| {
                self.selection
                    .show(ui, app, &summaries, self.viewport.view())
            })
            .inner;
        if let Some(error) = error {
            self.dialog = Some(Dialog::Error(error));
        }
    }

    /// The project's machine and its library source (SET-01, OI-54).
    fn machine_properties(&mut self, ui: &mut egui::Ui) {
        let Some(app) = &mut self.app else { return };
        let panel = self.machine_panel.get_or_insert_with(MachinePanel::default);
        let copy = app.project().machine().cloned();
        let copy_id = app.project().machine_id().map(str::to_owned);
        let command = egui::ScrollArea::vertical()
            .show(ui, |ui| panel.show(ui, copy.as_ref(), copy_id.as_deref()))
            .inner;
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
        let summaries = app.selection_summaries(&self.file_warnings);
        let roles = selection_panel::operation_roles(ui, app, id, &summaries);
        ui.separator();
        let edit = match app.parameter_form(id) {
            Ok(form) => {
                ui.weak(format!("Strategy: {}", form.strategy));
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
                .map_err(|e| e.to_string());
            self.form.result(&edit.name, result);
        }
        if let Some(error) = roles {
            self.dialog = Some(Dialog::Error(error));
        }
        if delete {
            match app.delete_operation(id) {
                Ok(()) => self.selected = None,
                Err(e) => self.dialog = Some(Dialog::Error(e.to_string())),
            }
        }
    }

    /// Works the generating job, the add-operation dialog and the playback
    /// (slice 9).
    fn run_frame(&mut self, ctx: &egui::Context) {
        let mut error = None;
        let mut added = None;
        if let Some(app) = &mut self.app {
            if let Some(run) = self.run.poll(app, ctx) {
                self.play.set_run(app, run);
            }
            let (new, problem) = self.run.add_dialog(ctx, app);
            added = new;
            error = problem;
            self.run.results_window(ctx, app);
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
        if self.selection.frame(app, input) {
            ctx.request_repaint();
        }
        if let Some(overlay) = self.selection.overlay(app, self.viewport.view()) {
            self.viewport.set_overlay(overlay);
            ctx.request_repaint();
        }
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

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            if self.scene_job.is_some() {
                ui.spinner();
                ui.label("Building the view…");
            }
            self.run.status(ui);
            if self.app.as_ref().is_some_and(Application::is_modified) {
                ui.label("Unsaved changes.");
            }
            ui.label(&self.status);
        });
        let view = self.viewport.view();
        if let Some(direction) = &view.a_direction {
            ui.label(format!("A direction: {direction}."));
        }
        for note in &view.notes {
            ui.label(note);
        }
        ui.small(openchuck_app::VERIFY_GCODE_NOTICE);
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
            Dialog::Error(message) => {
                ui.label(message);
                if ui.button("OK").clicked() {
                    close = true;
                }
            }
        });
        if close {
            self.dialog = None;
        }
        if let Some(action) = action {
            action(self);
        }
    }

    /// Stops the window closing while there are unsaved changes, and asks.
    fn guard_close(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.viewport().close_requested())
            && !self.quitting
            && self.app.as_ref().is_some_and(Application::is_modified)
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.dialog = Some(Dialog::Unsaved(Next::Quit));
        }
    }
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
    /// Show this operation's parameters.
    Select(OperationId),
    /// Show this tool's dimensions and profile.
    SelectTool(ToolId),
    /// Show this model's units, placement and checks.
    SelectModel(ModelId),
    /// Show the project's machine.
    SelectMachine,
    /// Pick triangles on the part.
    SelectPicking,
    /// Show this selection set.
    SelectSet(SelectionSetId),
}

/// What the properties panel shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Selection {
    Operation(OperationId),
    Tool(ToolId),
    Model(ModelId),
    Machine,
    /// Picking on the part, and the selection sets (slice 10).
    Picking,
    /// A selection set, picking on the part as well.
    Set(SelectionSetId),
}

/// One operation in the tree: enable box, name, and move up, move down and
/// duplicate buttons (UI-06). A disabled operation's name is greyed.
fn operation_row(
    ui: &mut egui::Ui,
    index: usize,
    count: usize,
    o: &openchuck_app::project::Operation,
    selected: bool,
    warning: Option<&str>,
    badge: Option<&str>,
) -> Option<TreeAction> {
    let mut action = None;
    ui.horizontal(|ui| {
        let mut enabled = o.enabled;
        if ui
            .checkbox(&mut enabled, "")
            .on_hover_text("Include in regeneration, simulation and posting")
            .changed()
        {
            action = Some(TreeAction::Command(Box::new(
                Command::SetOperationEnabled {
                    operation: o.id,
                    enabled,
                },
            )));
        }
        let name = egui::RichText::new(&o.name);
        let name = if o.enabled {
            name
        } else {
            name.weak().strikethrough()
        };
        if ui.selectable_label(selected, name).clicked() {
            action = Some(TreeAction::Select(o.id));
        }
        if let Some(badge) = badge {
            ui.weak(format!("({badge})"));
        }
        if let Some(warning) = warning {
            ui.colored_label(egui::Color32::from_rgb(230, 80, 80), "⚠")
                .on_hover_text(format!("A selection set cannot be used: {warning}"));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let id = ItemId::Operation(o.id);
            if ui.small_button("Duplicate").clicked() {
                action = Some(TreeAction::Duplicate(o.id));
            }
            if ui
                .add_enabled(index + 1 < count, egui::Button::new("↓").small())
                .on_hover_text("Move later")
                .clicked()
            {
                action = Some(TreeAction::Command(Box::new(Command::Move {
                    id,
                    index: index + 1,
                })));
            }
            if ui
                .add_enabled(index > 0, egui::Button::new("↑").small())
                .on_hover_text("Move earlier")
                .clicked()
            {
                action = Some(TreeAction::Command(Box::new(Command::Move {
                    id,
                    index: index - 1,
                })));
            }
        });
    });
    action
}

impl eframe::App for Shell {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.draw(ui);
    }
}

impl Shell {
    /// Draws one frame: the panels, the viewport, the dialogs and the work
    /// that keeps the jobs moving. The whole of the app's `ui`, apart from
    /// the eframe frame it does not use, so a test can run it.
    fn draw(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.guard_close(&ctx);
        self.poll_scene(&ctx);
        self.bench_frame(&ctx);
        self.shortcuts(&ctx);
        egui::Panel::top("menu").show(ui, |ui| self.menu(ui));
        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui));
        if self.play.open && self.play.has_run() {
            egui::Panel::bottom("playback")
                .resizable(false)
                .show(ui, |ui| {
                    if let Some(app) = &self.app {
                        self.play.show(ui, app);
                    }
                });
        }
        egui::Panel::left("tree")
            .resizable(true)
            .default_size(240.0)
            .show(ui, |ui| self.tree(ui));
        if self.selected.is_some() {
            egui::Panel::right("properties")
                .resizable(true)
                .default_size(320.0)
                .show(ui, |ui| self.properties(ui));
        }
        self.viewport.set_picking(matches!(
            self.selected,
            Some(Selection::Picking | Selection::Set(_))
        ));
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| self.viewport.show(ui));
        self.pick_frame(&ctx);
        self.run_frame(&ctx);
        self.import_frame(&ctx);
        self.dialogs(&ctx);
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
        let mut shell = Shell::new(false, Some(dir.clone()));
        let deadline = Instant::now() + Duration::from_secs(120);
        let wait = |shell: &mut Shell, done: &dyn Fn(&Shell) -> bool| {
            while !done(shell) {
                assert!(Instant::now() < deadline, "timed out");
                frame(shell, &ctx);
                std::thread::sleep(Duration::from_millis(5));
            }
        };
        // The scene is built, then the project tree and status bar draw.
        wait(&mut shell, &|s| s.scene_job.is_none());
        assert!(shell.viewport.view().picking.picker().is_some());
        // Generate and simulate (Shift+F5 does the same): the run is a job.
        shell.generate(true);
        assert!(shell.run.is_running());
        frame(&mut shell, &ctx);
        wait(&mut shell, &|s| {
            !s.run.is_running()
                && s.viewport
                    .layer(openchuck_viewport::Layer::Paths)
                    .is_some_and(|p| !p.lines.is_empty())
        });
        let app = shell.app.as_ref().unwrap();
        assert_eq!(shell.run.badge(app, op), Some("generated"));
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
        frame(&mut shell, &ctx);
        let app = shell.app.as_ref().unwrap();
        assert_eq!(shell.run.badge(app, op), Some("out of date"));
        shell.generate(false);
        wait(&mut shell, &|s| !s.run.is_running());
        let app = shell.app.as_ref().unwrap();
        assert_eq!(shell.run.badge(app, op), Some("generated"));
        // Cancelling a run leaves the old results alone.
        shell.generate(true);
        shell.run.cancel();
        wait(&mut shell, &|s| !s.run.is_running());
        let _ = std::fs::remove_dir_all(dir);
    }
}
