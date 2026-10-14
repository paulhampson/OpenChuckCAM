// SPDX-License-Identifier: AGPL-3.0-or-later
//! The application shell: menus, the project tree, new, open and save with
//! unsaved-change handling, undo and redo, and the viewport (UI-01, UI-06,
//! UI-08; work breakdown slice 8 items 3, 4 and 6; decision record 0026).
//!
//! Every project change goes through `openchuck-app`; the shell holds no CAM
//! logic. Folders are typed in for now: a native file dialog needs a crate
//! outside the crate selection (OI-117).

use std::path::PathBuf;

use eframe::egui::{self, Key, KeyboardShortcut, Modifiers};
use openchuck_app::project::ItemId;
use openchuck_app::project::id::{OperationId, SetupId};
use openchuck_app::scene::SceneView;
use openchuck_app::{AppError, Application, Command, JobHandle, Threads};
use openchuck_viewport::{DisplayMode, Projection, StandardView};

use crate::bench;
use crate::viewport::Viewport;

const SAVE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::S);
const UNDO: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Z);
const REDO: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::Z);
const REDO_ALT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Y);

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
        if let Ok(checks) = app.check_models() {
            let changed = checks
                .iter()
                .filter(|(_, c)| *c != openchuck_app::project::ModelCheck::Unchanged)
                .count();
            if changed > 0 {
                self.status = format!(
                    "{changed} model file(s) have changed or are missing since they were added"
                );
            }
        }
        self.setup = app.project().setups().first().map(|s| s.id);
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
                    Ok(view) => self.viewport.set_scene(view, fit),
                    Err(AppError::Engine(e)) if e.to_string().contains("cancel") => {}
                    Err(e) => self.status = format!("The view could not be built: {e}"),
                }
            }
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
                Ok(true) => self.rebuild_scene(false),
                Ok(false) => {}
                Err(e) => self.dialog = Some(Dialog::Error(e.to_string())),
            }
        }
    }

    fn redo(&mut self) {
        if let Some(app) = &mut self.app {
            match app.redo() {
                Ok(true) => self.rebuild_scene(false),
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
        let (save, undo, redo) = ctx.input_mut(|i| {
            (
                i.consume_shortcut(&SAVE),
                i.consume_shortcut(&UNDO),
                i.consume_shortcut(&REDO) || i.consume_shortcut(&REDO_ALT),
            )
        });
        if save {
            self.save();
        }
        if undo {
            self.undo();
        }
        if redo {
            self.redo();
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
        ui.heading(project.name());
        let mut chosen = None;
        let mut command: Option<TreeAction> = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::CollapsingHeader::new(format!("Models ({})", project.models().len()))
                .default_open(true)
                .show(ui, |ui| {
                    for m in project.models() {
                        ui.label(m.path.as_str());
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
            egui::CollapsingHeader::new(format!("Operations ({})", project.operations().len()))
                .default_open(true)
                .show(ui, |ui| {
                    let count = project.operations().len();
                    for (index, o) in project.operations().iter().enumerate() {
                        if let Some(c) = operation_row(ui, index, count, o) {
                            command = Some(c);
                        }
                    }
                });
            egui::CollapsingHeader::new(format!("Tools ({})", project.tools().len()))
                .default_open(false)
                .show(ui, |ui| {
                    for t in project.tools() {
                        ui.label(&t.name);
                    }
                });
        });
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
            TreeAction::Command(c) => Ok(*c),
            TreeAction::Duplicate(id) => openchuck_app::duplicate_operation(app.project(), id),
        };
        if let Err(e) = command.and_then(|c| app.execute(c)) {
            self.dialog = Some(Dialog::Error(e.to_string()));
        }
    }

    fn status_bar(&self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            if self.scene_job.is_some() {
                ui.spinner();
                ui.label("Building the view…");
            }
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
                ui.text_edit_singleline(&mut folder);
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

/// What a row of the project tree asks for.
#[derive(Debug, Clone, PartialEq)]
enum TreeAction {
    /// A command to execute.
    Command(Box<Command>),
    /// Duplicate this operation (the command is built from the project).
    Duplicate(OperationId),
}

/// One operation in the tree: enable box, name, and move up, move down and
/// duplicate buttons (UI-06). A disabled operation's name is greyed.
fn operation_row(
    ui: &mut egui::Ui,
    index: usize,
    count: usize,
    o: &openchuck_app::project::Operation,
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
        ui.label(if o.enabled {
            name
        } else {
            name.weak().strikethrough()
        });
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
        let ctx = ui.ctx().clone();
        self.guard_close(&ctx);
        self.poll_scene(&ctx);
        self.bench_frame(&ctx);
        self.shortcuts(&ctx);
        egui::Panel::top("menu").show(ui, |ui| self.menu(ui));
        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui));
        egui::Panel::left("tree")
            .resizable(true)
            .default_size(240.0)
            .show(ui, |ui| self.tree(ui));
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| self.viewport.show(ui));
        self.dialogs(&ctx);
    }
}
