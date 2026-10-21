// SPDX-License-Identifier: AGPL-3.0-or-later
//! The tool library window (specification 12.1; TL-01 to TL-07, TL-11,
//! TL-12; decision records 0014, 0081 and 0127; board 10).
//!
//! Left, the tool list: a search, the project's own tools ("This project")
//! and the library's tools grouped by type with an "in project" chip, and
//! New tool, Duplicate and Delete. Centre, the tool form (name, type, sizes,
//! material) and, for a custom profile, the profile editor. Right, the
//! cutting data by stock material (TL-05) and, for a library tool, the
//! machine tool sets (TL-11), or, for a project tool, its number and how it
//! stands against the library (TL-06, TL-07). The footer has Import library
//! and Export library (TL-01), Close and "Add to project".
//!
//! A library tool is edited as a draft: nothing is written until Save, and
//! Cancel or leaving it with "Discard" drops the changes (decision record
//! 0127 item 4). A project tool is the project's own copy: each change is
//! one undoable command and the library is untouched (item 1). The
//! default-tool ledger is never touched, so an edited default stays edited
//! and a deleted one stays deleted (TL-12).

use std::collections::BTreeMap;
use std::path::PathBuf;

use eframe::egui::{self, RichText};
use openchuck_app::project::id::{OperationId, ToolId};
use openchuck_app::project::{ParameterValue, Project};
use openchuck_app::tool_edit::{self, starting_profile};
use openchuck_app::tool_library::{
    self as library, ConflictChoice, CopyState, CuttingRow, DeleteImpact, ImportPreview, ToolType,
};
use openchuck_app::tools::{
    CuttingData, FeedRate, LibraryTool, Millimetres, SpindleSpeed, ToolLibrary, ToolProfile,
    ToolShape,
};
use openchuck_app::{AppError, Application, Command};

use crate::field_row::{self, Outcome, TextRow};
use crate::floating::{WindowEvent, floating_unscrolled};
use crate::form_panel::{self, FormState};
use crate::theme::DARK;
use crate::tool_panel::{self, ProfileAction};

/// Why the window was opened, which sets its primary action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    /// Without a project: browse and edit the library.
    Browse,
    /// To add a tool to the project: "Add to project" is the primary action.
    Add,
    /// To choose a tool for the operation being made in the New operation
    /// dialog: the tool added is handed back to the dialog.
    NewOperation,
    /// To choose the tool of this operation: "Add to project" also gives the
    /// operation the tool, in the same undoable step.
    Operation(OperationId),
}

/// Which tool the form shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chosen {
    /// A tool of the library, edited as a draft.
    Library(ToolId),
    /// The project's own copy of a tool, edited by commands.
    Project(ToolId),
}

/// Tool materials offered in the Material list; any other value a file holds
/// is offered too.
const MATERIALS: [&str; 3] = ["carbide", "HSS", "cobalt HSS"];

/// The labels, units and ranges of the cutting data fields, in table order.
const CUTTING: [(&str, &str, &str); 5] = [
    ("Spindle speed", "rpm", "above 0"),
    ("Feed per tooth", "mm", "above 0"),
    ("Plunge feed", "mm/min", "above 0"),
    ("Stepover", "mm", "above 0, at most the diameter"),
    ("Stepdown", "mm", "above 0, at most the flute length"),
];

/// A library tool being edited: nothing is written until Save.
#[derive(Debug, Clone, PartialEq)]
struct Draft {
    tool: LibraryTool,
    /// Its number in each machine tool set (TL-11).
    numbers: Vec<(String, Option<u32>)>,
}

/// A cutting data entry being typed: the material and the five values.
#[derive(Debug, Clone, Default, PartialEq)]
struct CuttingDraft {
    /// The material the entry was opened for; `None` for a new one.
    editing: Option<String>,
    material: String,
    values: [String; 5],
    problem: Option<String>,
}

impl CuttingDraft {
    fn of(row: &CuttingRow) -> Self {
        let d = &row.data;
        Self {
            editing: Some(row.material.clone()),
            material: row.material.clone(),
            values: [
                d.spindle_speed.0.to_string(),
                d.feed_per_tooth.0.to_string(),
                d.plunge_feed.0.to_string(),
                d.stepover.0.to_string(),
                d.stepdown.0.to_string(),
            ],
            problem: None,
        }
    }

    /// The typed values, or which one is not a number.
    fn data(&self) -> Result<CuttingData, String> {
        let mut numbers = [0.0; 5];
        for ((n, text), (label, ..)) in numbers.iter_mut().zip(&self.values).zip(CUTTING) {
            *n = crate::arith::number(text, None)
                .map_err(|reason| format!("{label}: {reason} Not applied."))?;
        }
        let [rpm, fpt, plunge, over, down] = numbers;
        Ok(CuttingData {
            spindle_speed: SpindleSpeed(rpm),
            feed_per_tooth: Millimetres(fpt),
            plunge_feed: FeedRate(plunge),
            stepover: Millimetres(over),
            stepdown: Millimetres(down),
        })
    }
}

/// Something the user asked for, done after drawing.
#[derive(Debug, Clone, PartialEq)]
enum Act {
    Close,
    Choose(Chosen),
    New(ToolType),
    Duplicate,
    AskDelete,
    Delete,
    Save,
    CancelEdits,
    Discard,
    KeepEditing,
    Name(String),
    Type(ToolType),
    Material(String),
    Field(String, Option<ParameterValue>),
    EditProfile,
    Profile(ProfileAction),
    SaveCutting,
    RemoveCutting(String),
    SetNumber(String, String),
    ProjectNumber(String),
    UpdateFromLibrary,
    AskSaveToLibrary,
    SaveToLibrary,
    RemoveFromProject,
    ImportLibrary,
    ApplyImport,
    CancelImport,
    ExportLibrary,
    AddToProject,
    CancelDialog,
}

impl Act {
    /// Whether the act leaves the tool being edited, so a draft with changes
    /// asks first.
    fn leaves(&self) -> bool {
        matches!(
            self,
            Self::Close | Self::Choose(_) | Self::New(_) | Self::Duplicate | Self::ImportLibrary
        )
    }
}

/// What the window reports to the shell.
#[derive(Debug, Default)]
pub struct WindowOutcome {
    /// A line for the status bar.
    pub status: Option<String>,
    /// A tool added to the project for the New operation dialog.
    pub for_new_operation: Option<ToolId>,
}

/// A confirmation in front of the window.
#[derive(Debug, Clone, PartialEq)]
enum Confirm {
    /// Delete this library tool.
    Delete(ToolId, DeleteImpact),
    /// Write the project's copy over the library tool: the fields that
    /// change, or `None` for a tool new to the library.
    SaveToLibrary(ToolId, Option<Vec<String>>),
    /// Leave a draft with changes, then do this.
    Discard(Box<Act>),
}

/// The window's state.
#[derive(Debug)]
pub struct ToolLibraryWindow {
    open: bool,
    purpose: Purpose,
    /// The library folder; the default folder when `None`.
    folder: Option<PathBuf>,
    library: Option<ToolLibrary>,
    /// Why the library could not be read.
    unreadable: Option<String>,
    search: String,
    chosen: Option<Chosen>,
    /// The library tool being edited (with `Chosen::Library`).
    draft: Option<Draft>,
    fields: FormState,
    profile: Option<ToolProfile>,
    segment: Option<usize>,
    cutting: Option<CuttingDraft>,
    confirm: Option<Confirm>,
    import: Option<(ImportPreview, BTreeMap<ToolId, ConflictChoice>)>,
    /// The last thing done, or why it was not.
    message: Option<(String, bool)>,
}

impl Default for ToolLibraryWindow {
    fn default() -> Self {
        Self::new(None)
    }
}

impl ToolLibraryWindow {
    /// A closed window on the library in `folder`, or the user's default
    /// tool library folder when `None`.
    pub fn new(folder: Option<PathBuf>) -> Self {
        Self {
            open: false,
            purpose: Purpose::Browse,
            folder,
            library: None,
            unreadable: None,
            search: String::new(),
            chosen: None,
            draft: None,
            fields: FormState::default(),
            profile: None,
            segment: None,
            cutting: None,
            confirm: None,
            import: None,
            message: None,
        }
    }

    /// Whether the window is open.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Opens the window for `purpose`, reading the library again from its
    /// folder, with `chosen` shown when given. A draft with changes is kept.
    pub fn open(&mut self, purpose: Purpose, chosen: Option<Chosen>) {
        self.purpose = purpose;
        if !self.open || !self.is_dirty() {
            self.reload();
        }
        self.open = true;
        if let Some(chosen) = chosen
            && !self.is_dirty()
        {
            self.choose(chosen);
        }
    }

    /// The library, read once when first asked (the operation inspector
    /// compares project tools with it, TL-07).
    pub fn library(&mut self) -> Option<&ToolLibrary> {
        if self.library.is_none() && self.unreadable.is_none() {
            self.reload();
        }
        self.library.as_ref()
    }

    /// Reads the library from its folder again.
    pub fn reload(&mut self) {
        let folder = self
            .folder
            .clone()
            .or_else(|| openchuck_app::library::default_folders().map(|f| f.tools));
        match folder.map(|f| ToolLibrary::open(&f)) {
            Some(Ok(library)) => {
                self.library = Some(library);
                self.unreadable = None;
            }
            Some(Err(e)) => {
                self.library = None;
                self.unreadable = Some(format!("The tool library could not be read: {e}"));
            }
            None => {
                self.library = None;
                self.unreadable = Some(
                    "The tool library folder could not be found: the configuration folder is \
                     not set."
                        .to_owned(),
                );
            }
        }
        if let Some(Chosen::Library(id)) = self.chosen {
            match self.library.as_ref().and_then(|l| l.get(id)) {
                Some(_) => self.choose(Chosen::Library(id)),
                None => self.forget(),
            }
        }
    }

    /// Whether the library draft has changes that are not saved.
    fn is_dirty(&self) -> bool {
        let (Some(draft), Some(lib)) = (&self.draft, &self.library) else {
            return false;
        };
        library::is_dirty(lib, &draft.tool)
            || draft.numbers != library::tool_set_numbers(lib, draft.tool.id)
    }

    fn forget(&mut self) {
        self.chosen = None;
        self.draft = None;
        self.clear_editing();
    }

    fn clear_editing(&mut self) {
        self.fields.clear();
        self.profile = None;
        self.segment = None;
        self.cutting = None;
    }

    fn choose(&mut self, chosen: Chosen) {
        self.clear_editing();
        self.chosen = Some(chosen);
        self.draft = match (chosen, &self.library) {
            (Chosen::Library(id), Some(lib)) => lib.get(id).map(|t| Draft {
                tool: t.clone(),
                numbers: library::tool_set_numbers(lib, id),
            }),
            _ => None,
        };
    }

    /// The tool the form shows: the draft, or the project's copy.
    fn shown(&self, project: Option<&Project>) -> Option<LibraryTool> {
        match self.chosen? {
            Chosen::Library(_) => self.draft.as_ref().map(|d| d.tool.clone()),
            Chosen::Project(id) => library::project_tool_as_edit(project?, id).ok(),
        }
    }

    /// Draws the window when it is open and does what the user asked.
    pub fn frame(
        &mut self,
        ctx: &egui::Context,
        mut app: Option<&mut Application>,
    ) -> WindowOutcome {
        let mut outcome = WindowOutcome::default();
        if !self.open {
            return outcome;
        }
        // A project tool that was removed (an undo, say) is no longer shown.
        if let Some(Chosen::Project(id)) = self.chosen
            && app
                .as_deref()
                .is_none_or(|a| !a.project().tools().iter().any(|t| t.id == id))
        {
            self.forget();
        }
        let mut acts = Vec::new();
        let project = app.as_deref().map(Application::project);
        let title = match self.purpose {
            Purpose::Operation(_) => "Tool library: choose the operation's tool",
            Purpose::NewOperation => "Tool library: choose the new operation's tool",
            _ => "Tool library",
        };
        let (_, event) = floating_unscrolled(ctx, "tool library", title, [1200.0, 760.0], |ui| {
            self.draw(ui, project, &mut acts);
        });
        if event == WindowEvent::CloseRequested {
            acts.push(Act::Close);
        }
        for act in acts {
            let (status, added) = self.act(act, app.as_deref_mut());
            if status.is_some() {
                outcome.status = status;
            }
            if added.is_some() {
                outcome.for_new_operation = added;
            }
        }
        outcome
    }

    fn draw(&mut self, ui: &mut egui::Ui, project: Option<&Project>, acts: &mut Vec<Act>) {
        // Esc closes the window (it asks first when a draft would be lost) unless a field
        // has the keyboard, where Esc belongs to the field (specification section 16).
        if !ui.ctx().egui_wants_keyboard_input()
            && ui.ctx().input(|i| i.key_pressed(egui::Key::Escape))
        {
            acts.push(Act::Close);
        }
        egui::Panel::bottom("tool library footer")
            .resizable(false)
            .show(ui, |ui| self.footer(ui, project, acts));
        let Some(lib) = &self.library else {
            ui.label(
                RichText::new(self.unreadable.as_deref().unwrap_or("No tool library."))
                    .color(DARK.error),
            );
            return;
        };
        egui::Panel::left("tool library list")
            .default_size(270.0)
            .resizable(true)
            .show(ui, |ui| {
                list(ui, lib, project, &mut self.search, self.chosen, acts)
            });
        let tool = self.shown(project);
        egui::Panel::right("tool library data")
            .default_size(300.0)
            .resizable(true)
            .show(ui, |ui| {
                crate::theme::solid_scroll_bar(ui);
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| match &tool {
                        Some(tool) => self.data(ui, tool, project, acts),
                        None => {
                            ui.label(RichText::new("Choose a tool.").color(DARK.muted));
                        }
                    });
            });
        egui::CentralPanel::default().show(ui, |ui| {
            crate::theme::solid_scroll_bar(ui);
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| match &tool {
                    Some(tool) => self.tool_form(ui, tool, acts),
                    None => {
                        ui.label(
                            RichText::new("Choose a tool in the list, or make one with New tool.")
                                .color(DARK.muted),
                        );
                    }
                });
        });
        self.dialogs(ui, acts);
    }

    /// The footer: Import library and Export library on the left; Close and
    /// "Add to project" on the right (12.1).
    fn footer(&self, ui: &mut egui::Ui, project: Option<&Project>, acts: &mut Vec<Act>) {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let readable = self.library.is_some();
            if ui
                .add_enabled(readable, egui::Button::new("Import library…"))
                .on_hover_text("Copy tools from another library folder")
                .clicked()
            {
                acts.push(Act::ImportLibrary);
            }
            if ui
                .add_enabled(readable, egui::Button::new("Export library…"))
                .on_hover_text(
                    "Write every tool of this library into a folder another user can import",
                )
                .clicked()
            {
                acts.push(Act::ExportLibrary);
            }
            if let Some((text, error)) = &self.message {
                let colour = if *error { DARK.error } else { DARK.muted };
                ui.add(egui::Label::new(RichText::new(text).color(colour)).truncate());
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let (label, enabled, reason) = self.add_state(project);
                let button = if self.purpose == Purpose::Browse {
                    ui.add_enabled(enabled, egui::Button::new(label))
                } else {
                    crate::shell::primary_button(ui, label, enabled)
                };
                if button.on_disabled_hover_text(reason).clicked() {
                    acts.push(Act::AddToProject);
                }
                if ui.button("Close").clicked() {
                    acts.push(Act::Close);
                }
            });
        });
        ui.add_space(8.0);
    }

    /// The label of the primary action, whether it can be used, and why not.
    fn add_state(&self, project: Option<&Project>) -> (&'static str, bool, &'static str) {
        let for_operation = matches!(self.purpose, Purpose::Operation(_) | Purpose::NewOperation);
        let Some(project) = project else {
            return ("Add to project", false, "Open a project first");
        };
        match self.chosen {
            None => ("Add to project", false, "Choose a tool first"),
            Some(Chosen::Project(_)) if for_operation => ("Use for the operation", true, ""),
            Some(Chosen::Project(_)) => ("Add to project", false, "It is in the project"),
            Some(Chosen::Library(_)) if self.is_dirty() => {
                ("Add to project", false, "Save the tool first")
            }
            Some(Chosen::Library(id)) => {
                let have = project.tools().iter().any(|t| t.id == id);
                match (have, for_operation) {
                    (true, true) => ("Use for the operation", true, ""),
                    (true, false) => (
                        "Add to project",
                        false,
                        "The project has this tool already: it is in This project",
                    ),
                    (false, _) => ("Add to project", true, ""),
                }
            }
        }
    }

    /// The tool form: name, type, sizes and material, then the profile
    /// editor for a custom profile (12.1, centre).
    fn tool_form(&mut self, ui: &mut egui::Ui, tool: &LibraryTool, acts: &mut Vec<Act>) {
        ui.horizontal(|ui| {
            heading(ui, "Tool");
            match self.chosen {
                Some(Chosen::Project(_)) => {
                    ui.label(
                        RichText::new("The project's copy: each change can be undone")
                            .color(DARK.muted),
                    );
                }
                Some(Chosen::Library(id)) => {
                    let dirty = self.is_dirty();
                    let new = self.library.as_ref().is_some_and(|l| l.get(id).is_none());
                    if crate::shell::primary_button(ui, "Save", dirty).clicked() {
                        acts.push(Act::Save);
                    }
                    if ui.add_enabled(dirty, egui::Button::new("Cancel")).clicked() {
                        acts.push(Act::CancelEdits);
                    }
                    let note = match (new, dirty) {
                        (true, _) => "New: not saved yet",
                        (false, true) => "Changed: not saved yet",
                        (false, false) => "Saved in the library",
                    };
                    let colour = if dirty { DARK.warning } else { DARK.muted };
                    ui.label(RichText::new(note).color(colour));
                }
                None => {}
            }
        });
        let name = TextRow {
            key: "name",
            label: "Name",
            description: "The tool's name in lists and on the setup sheet.",
            applied: tool.name.clone(),
            enabled: true,
            ..TextRow::default()
        };
        if let Outcome::Commit(typed) = field_row::text_row(ui, &mut self.fields, &name) {
            acts.push(Act::Name(typed));
        }
        let kind = ToolType::of(&tool.tool.shape);
        let labels: Vec<&str> = ToolType::ALL.iter().map(|t| t.label()).collect();
        let current = ToolType::ALL.iter().position(|t| *t == kind);
        if let Some(i) = field_row::choice_row(
            ui,
            "tool type",
            "Type",
            "The tool's shape (TL-02, TL-03). The sizes are kept.",
            &labels,
            current,
            true,
        ) && let Some(t) = ToolType::ALL.get(i)
        {
            acts.push(Act::Type(*t));
        }
        if let Some(reason) = self.fields.error("type") {
            field_row::note_line(ui, reason, DARK.error);
        }
        if kind == ToolType::Custom {
            field_row::read_only_row(
                ui,
                "Diameter",
                "Where the profile ends, doubled.",
                RichText::new(format!("{:.3} mm  from profile", tool.tool.diameter.0)).monospace(),
            );
        }
        let fields = tool_edit::tool_form(&tool.tool);
        if let Some(edit) = form_panel::show_fields(ui, &fields, &[], false, &mut self.fields) {
            acts.push(Act::Field(edit.name, edit.value));
        }
        let mut materials: Vec<String> = MATERIALS.iter().map(|m| (*m).to_owned()).collect();
        if let Some(m) = &tool.tool.material
            && !materials.iter().any(|x| x == m)
        {
            materials.push(m.clone());
        }
        materials.insert(0, "Not given".to_owned());
        let labels: Vec<&str> = materials.iter().map(String::as_str).collect();
        let current = tool
            .tool
            .material
            .as_ref()
            .and_then(|m| materials.iter().position(|x| x == m))
            .unwrap_or(0);
        if let Some(i) = field_row::choice_row(
            ui,
            "tool material",
            "Material",
            "What the tool is made of (TL-04). Not the stock material.",
            &labels,
            Some(current),
            true,
        ) {
            let material = if i == 0 {
                String::new()
            } else {
                materials.get(i).cloned().unwrap_or_default()
            };
            acts.push(Act::Material(material));
        }
        ui.add_space(8.0);
        ui.separator();
        match (&mut self.profile, &tool.tool.shape) {
            (Some(draft), _) => {
                heading(ui, "Profile");
                let action = tool_panel::editor(ui, &tool.tool, draft, &mut self.segment, true);
                if action != ProfileAction::None {
                    acts.push(Act::Profile(action));
                }
            }
            (None, ToolShape::Custom { .. }) => {
                heading(ui, "Profile");
                if ui.button("Edit the profile…").clicked() {
                    acts.push(Act::EditProfile);
                }
            }
            (None, _) => {
                ui.label(
                    RichText::new(
                        "A standard type: its shape follows its sizes. Choose \"Custom profile\" \
                         as the type to draw one.",
                    )
                    .color(DARK.muted),
                );
            }
        }
    }

    /// The right column: cutting data by stock material (TL-05), then the
    /// machine tool sets (TL-11) of a library tool or the project's number
    /// and copy state of a project tool (TL-06, TL-07).
    fn data(
        &mut self,
        ui: &mut egui::Ui,
        tool: &LibraryTool,
        project: Option<&Project>,
        acts: &mut Vec<Act>,
    ) {
        heading(ui, "Cutting data by material");
        let rows = library::cutting_rows(tool);
        egui::Grid::new("cutting data")
            .num_columns(4)
            .striped(true)
            .show(ui, |ui| {
                for h in ["Material", "rpm", "mm/t", "Plunge"] {
                    ui.label(RichText::new(h).small().color(DARK.muted));
                }
                ui.end_row();
                for row in &rows {
                    let open = self
                        .cutting
                        .as_ref()
                        .is_some_and(|d| d.editing.as_deref() == Some(row.material.as_str()));
                    if ui
                        .selectable_label(open, &row.material)
                        .on_hover_text(format!(
                            "Stepover {} mm, stepdown {} mm; cutting feed {:.0} mm/min. \
                             Click to edit.",
                            row.data.stepover.0, row.data.stepdown.0, row.feed
                        ))
                        .clicked()
                    {
                        self.cutting = if open {
                            None
                        } else {
                            Some(CuttingDraft::of(row))
                        };
                    }
                    ui.monospace(format!("{:.0}", row.data.spindle_speed.0));
                    ui.monospace(format!("{:.3}", row.data.feed_per_tooth.0));
                    ui.monospace(format!("{:.0}", row.data.plunge_feed.0));
                    ui.end_row();
                }
            });
        if rows.is_empty() {
            ui.label(RichText::new("No cutting data yet.").color(DARK.muted));
        }
        if self.cutting.is_none() && ui.small_button("Add material").clicked() {
            self.cutting = Some(CuttingDraft::default());
        }
        if let Some(draft) = &mut self.cutting {
            cutting_editor(ui, draft, acts);
        }
        ui.label(
            RichText::new(
                "Operations cutting a stock of this material take these values unless they set \
                 their own. Stepover and stepdown are in each material's detail.",
            )
            .color(DARK.muted),
        );
        ui.add_space(10.0);
        match self.chosen {
            Some(Chosen::Project(id)) => self.project_data(ui, id, project, acts),
            _ => self.tool_sets(ui, acts),
        }
    }

    /// A library tool's number in each machine tool set (TL-11).
    fn tool_sets(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Act>) {
        heading(ui, "Machine tool sets");
        let Some(draft) = &self.draft else { return };
        if draft.numbers.is_empty() {
            ui.label(RichText::new("This library has no machine tool sets.").color(DARK.muted));
        }
        for (machine, number) in &draft.numbers {
            let key = format!("set:{machine}");
            let row = TextRow {
                key: &key,
                label: machine,
                description: "The number this tool gets when it is added to a project for this \
                              machine. Empty: not in the set.",
                range: Some("1 or more, not another tool's number in this set"),
                applied: number.map(|n| n.to_string()).unwrap_or_default(),
                hint: Some("not in the set".to_owned()),
                enabled: true,
                number: true,
                ..TextRow::default()
            };
            if let Outcome::Commit(typed) = field_row::text_row(ui, &mut self.fields, &row) {
                acts.push(Act::SetNumber(machine.clone(), typed));
            }
        }
        ui.label(
            RichText::new(
                "The number suggested when this tool is added to a project for that machine. The \
                 project keeps its own number.",
            )
            .color(DARK.muted),
        );
    }

    /// A project tool's number, its state against the library, and the
    /// operations that use it (TL-06, TL-07; decision record 0127 items 1,
    /// 3 and 6).
    fn project_data(
        &mut self,
        ui: &mut egui::Ui,
        id: ToolId,
        project: Option<&Project>,
        acts: &mut Vec<Act>,
    ) {
        heading(ui, "In this project");
        let Some(row) = project.and_then(|p| {
            library::project_rows(p, self.library.as_ref(), "")
                .into_iter()
                .find(|r| r.id == id)
        }) else {
            return;
        };
        let number = TextRow {
            key: "project number",
            label: "Tool number",
            description: "The project's number for this tool (TL-06), used by every operation \
                          that uses it.",
            range: Some("1 or more, not another project tool's number"),
            applied: row.number.to_string(),
            enabled: true,
            number: true,
            ..TextRow::default()
        };
        if let Outcome::Commit(typed) = field_row::text_row(ui, &mut self.fields, &number) {
            acts.push(Act::ProjectNumber(typed));
        }
        ui.label(RichText::new(row.copy.text()).color(copy_colour(&row.copy)));
        ui.horizontal_wrapped(|ui| {
            let differs = matches!(row.copy, CopyState::Differs(_));
            if ui
                .add_enabled(differs, egui::Button::new("Update from library"))
                .on_hover_text(
                    "Replace the project's copy with the library's tool, keeping the tool \
                     number (one undoable step)",
                )
                .clicked()
            {
                acts.push(Act::UpdateFromLibrary);
            }
            let saveable = matches!(row.copy, CopyState::Differs(_) | CopyState::NotInLibrary);
            if ui
                .add_enabled(saveable, egui::Button::new("Save to library"))
                .on_hover_text("Write the project's copy into the tool library")
                .clicked()
            {
                acts.push(Act::AskSaveToLibrary);
            }
        });
        if row.used_by.is_empty() {
            ui.label(RichText::new("No operation uses it.").color(DARK.muted));
        } else {
            ui.label(
                RichText::new(format!("Used by {}.", row.used_by.join(", "))).color(DARK.muted),
            );
        }
        let unused = row.used_by.is_empty();
        if ui
            .add_enabled(unused, egui::Button::new("Remove from project"))
            .on_disabled_hover_text("Give the operations that use it another tool first")
            .clicked()
        {
            acts.push(Act::RemoveFromProject);
        }
    }

    /// The confirmations and the import review.
    fn dialogs(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Act>) {
        match &self.confirm {
            Some(Confirm::Delete(_, impact)) => {
                modal(ui, "delete library tool", |ui| {
                    ui.heading(format!("Delete \"{}\" from the library?", impact.name));
                    ui.label("Its file is deleted. This cannot be undone.");
                    if impact.in_project {
                        ui.label(
                            "This project keeps its own copy of the tool, and its operations \
                             still use it; they then show \"Not in the library\".",
                        );
                    }
                    for (machine, number) in &impact.tool_sets {
                        ui.label(format!(
                            "It is T{number} in the {machine} tool set: it is taken out of the set."
                        ));
                    }
                    ui.label(
                        RichText::new("A default tool that is deleted is not installed again.")
                            .color(DARK.muted),
                    );
                    buttons(
                        ui,
                        acts,
                        ("Delete", Act::Delete),
                        ("Cancel", Act::CancelDialog),
                    );
                });
            }
            Some(Confirm::SaveToLibrary(_, fields)) => {
                modal(ui, "save to library", |ui| {
                    ui.heading("Save the project's tool to the library?");
                    match fields {
                        None => {
                            ui.label("The library has no such tool: it is added as a new one.");
                        }
                        Some(fields) => {
                            ui.label(format!(
                                "The library's tool changes: {}. Other projects that use it \
                                 keep their own copies.",
                                fields.join(", ")
                            ));
                        }
                    }
                    ui.label(
                        RichText::new("The project's tool number stays with the project.")
                            .color(DARK.muted),
                    );
                    buttons(
                        ui,
                        acts,
                        ("Save to library", Act::SaveToLibrary),
                        ("Cancel", Act::CancelDialog),
                    );
                });
            }
            Some(Confirm::Discard(_)) => {
                let name = self
                    .draft
                    .as_ref()
                    .map_or_else(String::new, |d| d.tool.name.clone());
                modal(ui, "discard changes", |ui| {
                    ui.heading(format!("Discard the changes to \"{name}\"?"));
                    ui.label("They are not saved in the library.");
                    buttons(
                        ui,
                        acts,
                        ("Discard", Act::Discard),
                        ("Keep editing", Act::KeepEditing),
                    );
                });
            }
            None => {}
        }
        if let Some((preview, choices)) = &mut self.import {
            modal(ui, "import library", |ui| {
                ui.heading("Import tools");
                ui.label(format!(
                    "From {}: {} new, {} the same as yours, {} that differ from yours.",
                    preview.from.display(),
                    preview.added.len(),
                    preview.unchanged.len(),
                    preview.conflicts.len()
                ));
                if !preview.conflicts.is_empty() {
                    ui.horizontal(|ui| {
                        ui.label("For every tool that differs:");
                        for (label, choice) in [
                            ("Keep mine", ConflictChoice::KeepMine),
                            ("Replace", ConflictChoice::Replace),
                            ("Keep both", ConflictChoice::KeepBoth),
                        ] {
                            if ui.small_button(label).clicked() {
                                for (_, c) in &preview.conflicts {
                                    choices.insert(c.id, choice);
                                }
                            }
                        }
                    });
                    egui::ScrollArea::vertical()
                        .max_height(300.0)
                        .show(ui, |ui| {
                            for (_, conflict) in &preview.conflicts {
                                let words: Vec<String> = conflict
                                    .fields
                                    .iter()
                                    .map(|f| library::field_words(f))
                                    .collect();
                                ui.label(format!(
                                    "\"{}\": differs in {}",
                                    conflict.name,
                                    words.join(", ")
                                ));
                                let choice = choices
                                    .entry(conflict.id)
                                    .or_insert(ConflictChoice::KeepMine);
                                ui.horizontal(|ui| {
                                    ui.radio_value(choice, ConflictChoice::KeepMine, "Keep mine");
                                    ui.radio_value(choice, ConflictChoice::Replace, "Replace");
                                    ui.radio_value(choice, ConflictChoice::KeepBoth, "Keep both");
                                });
                            }
                        });
                }
                buttons(
                    ui,
                    acts,
                    ("Import", Act::ApplyImport),
                    ("Cancel", Act::CancelImport),
                );
            });
        }
    }

    /// Does `act`; returns a status line for the shell and a tool added for
    /// the New operation dialog.
    fn act(&mut self, act: Act, app: Option<&mut Application>) -> (Option<String>, Option<ToolId>) {
        if let Act::Choose(chosen) = &act
            && Some(*chosen) == self.chosen
        {
            // The tool already shown: nothing to leave.
            return (None, None);
        }
        if act.leaves() && self.is_dirty() {
            self.confirm = Some(Confirm::Discard(Box::new(act)));
            return (None, None);
        }
        let mut added = None;
        let result = self.run(act, app, &mut added);
        let status = match result {
            Ok(Some(text)) => {
                self.message = Some((text.clone(), false));
                Some(text)
            }
            Ok(None) => None,
            Err(e) => {
                self.message = Some((e, true));
                None
            }
        };
        (status, added)
    }

    #[allow(clippy::too_many_lines)]
    fn run(
        &mut self,
        act: Act,
        app: Option<&mut Application>,
        added: &mut Option<ToolId>,
    ) -> Result<Option<String>, String> {
        let text = |e: AppError| e.to_string();
        match act {
            Act::Close => {
                self.open = false;
                self.confirm = None;
                self.import = None;
                self.draft = None;
                if matches!(self.chosen, Some(Chosen::Library(_))) {
                    self.chosen = None;
                }
                Ok(None)
            }
            Act::Choose(chosen) => {
                self.choose(chosen);
                Ok(None)
            }
            Act::New(kind) => {
                let lib = self.library.as_ref().ok_or("No tool library.")?;
                let tool = library::new_draft(lib, kind).map_err(text)?;
                let numbers = library::tool_set_numbers(lib, tool.id);
                self.clear_editing();
                self.search.clear();
                self.chosen = Some(Chosen::Library(tool.id));
                self.draft = Some(Draft { tool, numbers });
                Ok(Some(format!(
                    "New {}: set it up, then Save.",
                    kind.label().to_lowercase()
                )))
            }
            Act::Duplicate => {
                let lib = self.library.as_ref().ok_or("No tool library.")?;
                let tool = match (self.chosen, app.as_deref()) {
                    (Some(Chosen::Library(id)), _) => {
                        library::duplicate_draft(lib, id).map_err(text)?
                    }
                    (Some(Chosen::Project(id)), Some(app)) => {
                        // A project tool duplicates into the library too.
                        let mut copy =
                            library::project_tool_as_edit(app.project(), id).map_err(text)?;
                        copy.id = ToolId::new();
                        copy.name = format!("{} copy", copy.name);
                        copy
                    }
                    _ => return Ok(None),
                };
                let numbers = library::tool_set_numbers(lib, tool.id);
                self.clear_editing();
                self.chosen = Some(Chosen::Library(tool.id));
                self.draft = Some(Draft { tool, numbers });
                Ok(Some(
                    "Duplicated as a new library tool: Save to keep it.".into(),
                ))
            }
            Act::AskDelete => {
                let Some(Chosen::Library(id)) = self.chosen else {
                    return Ok(None);
                };
                let lib = self.library.as_ref().ok_or("No tool library.")?;
                if lib.get(id).is_none() {
                    // A new tool not saved yet: deleting is dropping it.
                    self.forget();
                    return Ok(Some("The new tool was not saved.".into()));
                }
                let project = app.as_deref().map(Application::project);
                let impact = library::delete_impact(lib, project, id).map_err(text)?;
                self.confirm = Some(Confirm::Delete(id, impact));
                Ok(None)
            }
            Act::Delete => {
                let Some(Confirm::Delete(id, impact)) = self.confirm.take() else {
                    return Ok(None);
                };
                let lib = self.library.as_mut().ok_or("No tool library.")?;
                library::delete_tool(lib, id).map_err(text)?;
                self.forget();
                Ok(Some(format!(
                    "Deleted \"{}\" from the library.",
                    impact.name
                )))
            }
            Act::Save => {
                let (Some(lib), Some(draft)) = (&mut self.library, &self.draft) else {
                    return Ok(None);
                };
                library::save_draft(lib, &draft.tool, &draft.numbers).map_err(text)?;
                let name = draft.tool.name.clone();
                self.fields.clear();
                Ok(Some(format!("Saved \"{name}\" in the library.")))
            }
            Act::CancelEdits => {
                match self.chosen {
                    Some(Chosen::Library(id))
                        if self.library.as_ref().is_some_and(|l| l.get(id).is_some()) =>
                    {
                        self.choose(Chosen::Library(id));
                    }
                    _ => self.forget(),
                }
                Ok(None)
            }
            Act::Discard => {
                let Some(Confirm::Discard(next)) = self.confirm.take() else {
                    return Ok(None);
                };
                self.draft = None;
                self.chosen = None;
                self.clear_editing();
                self.run(*next, app, added)
            }
            Act::KeepEditing | Act::CancelDialog => {
                self.confirm = None;
                Ok(None)
            }
            Act::Name(typed) => {
                let r = self.edit(app, |t| library::edit_name(t, &typed));
                self.fields.result("name", r);
                Ok(None)
            }
            Act::Type(kind) => {
                let r = self.edit(app, |t| library::edit_type(t, kind));
                self.fields.result("type", r);
                self.profile = None;
                Ok(None)
            }
            Act::Material(material) => {
                self.edit(app, |t| library::edit_material(t, &material))?;
                Ok(None)
            }
            Act::Field(name, value) => {
                let r = match &value {
                    Some(v) => self.edit(app, |t| library::edit_field(t, &name, v)),
                    None => Err("A tool size needs a value. Not applied.".to_owned()),
                };
                self.fields.result(&name, r);
                Ok(None)
            }
            Act::EditProfile => {
                let project = app.as_deref().map(Application::project);
                if let Some(tool) = self.shown(project) {
                    self.profile = starting_profile(&tool.tool);
                    self.segment = None;
                }
                Ok(None)
            }
            Act::Profile(action) => self.profile_action(action, app),
            Act::SaveCutting => {
                let Some(draft) = self.cutting.clone() else {
                    return Ok(None);
                };
                let saved = draft.data().and_then(|data| {
                    self.edit(app, |t| {
                        library::edit_cutting_data(
                            t,
                            &draft.material,
                            data,
                            draft.editing.as_deref(),
                        )
                    })
                });
                match saved {
                    Ok(()) => self.cutting = None,
                    Err(e) => {
                        if let Some(c) = &mut self.cutting {
                            c.problem = Some(e);
                        }
                    }
                }
                Ok(None)
            }
            Act::RemoveCutting(material) => {
                self.edit(app, |t| {
                    library::remove_cutting_data(t, &material);
                    Ok(())
                })?;
                self.cutting = None;
                Ok(None)
            }
            Act::SetNumber(machine, typed) => {
                let key = format!("set:{machine}");
                let r = parse_number(&typed, true).and_then(|n| {
                    let (Some(lib), Some(draft)) = (&self.library, &mut self.draft) else {
                        return Ok(());
                    };
                    library::check_tool_set_number(lib, &machine, draft.tool.id, n)
                        .map_err(|e| form_panel::refusal(&e.to_string()))?;
                    if let Some(entry) = draft.numbers.iter_mut().find(|(m, _)| *m == machine) {
                        entry.1 = n;
                    }
                    Ok(())
                });
                self.fields.result(&key, r);
                Ok(None)
            }
            Act::ProjectNumber(typed) => {
                let (Some(Chosen::Project(id)), Some(app)) = (self.chosen, app) else {
                    return Ok(None);
                };
                let r = parse_number(&typed, false).and_then(|n| {
                    library::set_project_tool_number(app.project(), id, n.unwrap_or(0))
                        .and_then(|c| app.execute(c))
                        .map_err(|e| form_panel::refusal(&e.to_string()))
                });
                self.fields.result("project number", r);
                Ok(None)
            }
            Act::UpdateFromLibrary => {
                let (Some(Chosen::Project(id)), Some(app), Some(lib)) =
                    (self.chosen, app, &self.library)
                else {
                    return Ok(None);
                };
                match library::update_from_library(app.project(), lib, id).map_err(text)? {
                    Some(c) => {
                        app.execute(c).map_err(text)?;
                        self.clear_editing();
                        Ok(Some("Updated the project's tool from the library.".into()))
                    }
                    None => Ok(None),
                }
            }
            Act::AskSaveToLibrary => {
                let (Some(Chosen::Project(id)), Some(app), Some(lib)) =
                    (self.chosen, app.as_deref(), &self.library)
                else {
                    return Ok(None);
                };
                let fields =
                    library::save_to_library_changes(app.project(), lib, id).map_err(text)?;
                self.confirm = Some(Confirm::SaveToLibrary(id, fields));
                Ok(None)
            }
            Act::SaveToLibrary => {
                let Some(Confirm::SaveToLibrary(id, _)) = self.confirm.take() else {
                    return Ok(None);
                };
                let (Some(app), Some(lib)) = (app.as_deref(), &mut self.library) else {
                    return Ok(None);
                };
                library::save_to_library(app.project(), lib, id).map_err(text)?;
                Ok(Some("Saved the project's tool to the library.".into()))
            }
            Act::RemoveFromProject => {
                let (Some(Chosen::Project(id)), Some(app)) = (self.chosen, app) else {
                    return Ok(None);
                };
                let command = library::remove_from_project(app.project(), id).map_err(text)?;
                app.execute(command).map_err(text)?;
                self.forget();
                Ok(Some(
                    "Removed the tool from the project. Undo with Ctrl Z.".into(),
                ))
            }
            Act::ImportLibrary => {
                let picked = rfd::FileDialog::new()
                    .set_title("Import tools from a library folder")
                    .pick_folder();
                let (Some(folder), Some(lib)) = (picked, &self.library) else {
                    return Ok(None);
                };
                let preview = library::preview_import(lib, &folder).map_err(text)?;
                self.import = Some((preview, BTreeMap::new()));
                Ok(None)
            }
            Act::ApplyImport => {
                let (Some((preview, choices)), Some(lib)) = (self.import.take(), &mut self.library)
                else {
                    return Ok(None);
                };
                let o = library::apply_import(lib, &preview, |id| {
                    choices
                        .get(&id)
                        .copied()
                        .unwrap_or(ConflictChoice::KeepMine)
                })
                .map_err(text)?;
                if let Some(Chosen::Library(id)) = self.chosen {
                    self.choose(Chosen::Library(id));
                }
                Ok(Some(format!(
                    "Imported: {} added, {} replaced, {} kept.",
                    o.added.len(),
                    o.replaced.len(),
                    o.kept.len()
                )))
            }
            Act::CancelImport => {
                self.import = None;
                Ok(None)
            }
            Act::ExportLibrary => {
                let picked = rfd::FileDialog::new()
                    .set_title("Export the tool library into a folder")
                    .pick_folder();
                let (Some(folder), Some(lib)) = (picked, &self.library) else {
                    return Ok(None);
                };
                let files = library::export_tools(lib, &[], &folder).map_err(text)?;
                Ok(Some(format!(
                    "Exported {} tools to {}.",
                    files.len(),
                    folder.display()
                )))
            }
            Act::AddToProject => {
                let (Some(app), Some(chosen)) = (app, self.chosen) else {
                    return Ok(None);
                };
                let id = match chosen {
                    Chosen::Library(id) | Chosen::Project(id) => id,
                };
                let lib = self.library.as_ref().ok_or("No tool library.")?;
                let status = add_to_project(app, lib, id, self.purpose)?;
                if self.purpose == Purpose::NewOperation {
                    *added = Some(id);
                }
                if self.purpose != Purpose::Add && self.purpose != Purpose::Browse {
                    self.open = false;
                } else {
                    self.choose(Chosen::Project(id));
                }
                Ok(status)
            }
        }
    }

    /// Applies `edit` to the tool shown: to the library draft, or to the
    /// project's copy as one undoable command (decision record 0127 item 1).
    /// An error is the refusal, as a sentence ending "Not applied.".
    fn edit(
        &mut self,
        app: Option<&mut Application>,
        edit: impl FnOnce(&mut LibraryTool) -> Result<(), AppError>,
    ) -> Result<(), String> {
        let refusal = |e: AppError| form_panel::refusal(&e.to_string());
        match self.chosen {
            Some(Chosen::Library(_)) => match &mut self.draft {
                Some(draft) => edit(&mut draft.tool).map_err(refusal),
                None => Ok(()),
            },
            Some(Chosen::Project(id)) => {
                let Some(app) = app else { return Ok(()) };
                let mut tool = library::project_tool_as_edit(app.project(), id).map_err(refusal)?;
                edit(&mut tool).map_err(refusal)?;
                match library::snapshot_command(app.project(), &tool).map_err(refusal)? {
                    Some(c) => app.execute(c).map_err(refusal),
                    None => Ok(()),
                }
            }
            None => Ok(()),
        }
    }

    fn profile_action(
        &mut self,
        action: ProfileAction,
        app: Option<&mut Application>,
    ) -> Result<Option<String>, String> {
        match action {
            ProfileAction::None => Ok(None),
            ProfileAction::Cancel => {
                self.profile = None;
                self.segment = None;
                Ok(None)
            }
            ProfileAction::Apply => {
                let Some(profile) = self.profile.clone() else {
                    return Ok(None);
                };
                self.edit(app, |t| library::edit_profile(t, profile))?;
                self.profile = None;
                self.segment = None;
                Ok(None)
            }
            ProfileAction::ImportFile => {
                let picked = rfd::FileDialog::new()
                    .set_title("Import a profile file")
                    .add_filter("Profile file", &["toml"])
                    .pick_file();
                let (Some(file), Some(profile)) = (picked, &mut self.profile) else {
                    return Ok(None);
                };
                *profile = openchuck_app::tools::load_profile(&file).map_err(|e| e.to_string())?;
                self.segment = None;
                Ok(Some(format!(
                    "Read the profile from {}. Apply to use it.",
                    file.display()
                )))
            }
            ProfileAction::ExportFile => {
                let picked = rfd::FileDialog::new()
                    .set_title("Export the profile")
                    .add_filter("Profile file", &["toml"])
                    .save_file();
                let project = app.as_deref().map(Application::project);
                let (Some(file), Some(tool), Some(profile)) =
                    (picked, self.shown(project), &self.profile)
                else {
                    return Ok(None);
                };
                library::export_profile(&tool.tool, profile, &file).map_err(|e| e.to_string())?;
                Ok(Some(format!("Wrote the profile to {}.", file.display())))
            }
        }
    }
}

/// A typed tool number: a whole number, with an optional T before it; empty
/// is `None` when `empty` allows it.
fn parse_number(typed: &str, empty: bool) -> Result<Option<u32>, String> {
    let typed = typed.trim().trim_start_matches(['T', 't']);
    if typed.is_empty() && empty {
        return Ok(None);
    }
    typed
        .parse::<u32>()
        .map(Some)
        .map_err(|_| "Type a whole tool number. Not applied.".to_owned())
}

/// Adds library tool `id` to the project as one undoable command, and for an
/// operation's choice gives the operation the tool in the same step (TL-07,
/// TL-11; decision record 0127 item 6).
fn add_to_project(
    app: &mut Application,
    lib: &ToolLibrary,
    id: ToolId,
    purpose: Purpose,
) -> Result<Option<String>, String> {
    let have = app.project().tools().iter().any(|t| t.id == id);
    let mut commands = Vec::new();
    let mut text = String::new();
    let mut preview = app.project().clone();
    if !have {
        let (command, controller, source) =
            library::add_to_project(app.project(), lib, id).map_err(|e| e.to_string())?;
        if let Command::Add { item, placement } = &command {
            preview
                .insert(item.clone(), *placement)
                .map_err(|e| e.to_string())?;
        }
        commands.push(command);
        let how = match source {
            openchuck_app::tools::NumberSource::ToolSet => "from the machine's tool set".to_owned(),
            openchuck_app::tools::NumberSource::ToolSetTaken { wanted } => format!(
                "the next free number; the machine's tool set gives T{wanted}, which another \
                 tool here uses"
            ),
            openchuck_app::tools::NumberSource::NextFree => "the next free number".to_owned(),
        };
        text = format!("Added to the project as T{} ({how}).", controller.number);
    }
    if let Purpose::Operation(operation) = purpose {
        commands
            .push(library::set_operation_tool(&preview, operation, id).map_err(|e| e.to_string())?);
        text = format!("{text} The operation now uses it.")
            .trim()
            .to_owned();
    }
    let command = match commands.len() {
        0 => return Ok(None),
        1 => commands.remove(0),
        _ => Command::Batch(commands),
    };
    app.execute(command).map_err(|e| e.to_string())?;
    Ok(Some(text))
}

/// The colour a copy state is written in.
pub fn copy_colour(copy: &CopyState) -> egui::Color32 {
    match copy {
        CopyState::Same => DARK.ok,
        CopyState::Differs(_) => DARK.warning,
        CopyState::NotInLibrary | CopyState::Unknown => DARK.muted,
    }
}

/// The tool list (12.1, left): search, This project, the library's groups,
/// and New tool, Duplicate and Delete under it.
fn list(
    ui: &mut egui::Ui,
    lib: &ToolLibrary,
    project: Option<&Project>,
    search: &mut String,
    chosen: Option<Chosen>,
    acts: &mut Vec<Act>,
) {
    ui.add_space(6.0);
    ui.add(
        egui::TextEdit::singleline(search)
            .hint_text("Search tools")
            .desired_width(f32::INFINITY),
    );
    egui::Panel::bottom("tool list buttons")
        .resizable(false)
        .show(ui, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.menu_button("New tool \u{23f7}", |ui| {
                    for kind in ToolType::ALL {
                        if ui.button(kind.label()).clicked() {
                            acts.push(Act::New(kind));
                            ui.close();
                        }
                    }
                });
                if ui
                    .add_enabled(chosen.is_some(), egui::Button::new("Duplicate"))
                    .on_hover_text("Copy the tool as a new library tool")
                    .clicked()
                {
                    acts.push(Act::Duplicate);
                }
                let library_tool = matches!(chosen, Some(Chosen::Library(_)));
                if ui
                    .add_enabled(library_tool, egui::Button::new("Delete"))
                    .on_hover_text("Delete the tool from the library; a project keeps its copy")
                    .on_disabled_hover_text(
                        "Choose a library tool; a project tool is removed from the project",
                    )
                    .clicked()
                {
                    acts.push(Act::AskDelete);
                }
            });
            ui.add_space(6.0);
        });
    crate::theme::solid_scroll_bar(ui);
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if let Some(project) = project {
                let rows = library::project_rows(project, Some(lib), search);
                group_title(ui, "This project");
                if rows.is_empty() {
                    ui.label(RichText::new("No tools yet.").color(DARK.muted));
                }
                for row in rows {
                    let selected = chosen == Some(Chosen::Project(row.id));
                    let response = ui
                        .horizontal(|ui| {
                            let r = ui.selectable_label(selected, &row.name);
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    crate::shell::chip(ui, &format!("T{}", row.number));
                                },
                            );
                            r
                        })
                        .inner;
                    let used = if row.used_by.is_empty() {
                        "No operation uses it.".to_owned()
                    } else {
                        format!("Used by {}.", row.used_by.join(", "))
                    };
                    if response
                        .on_hover_text(format!("{}\n{}\n{used}", row.summary, row.copy.text()))
                        .clicked()
                    {
                        acts.push(Act::Choose(Chosen::Project(row.id)));
                    }
                }
            }
            let groups = library::tool_list(lib, project, search);
            if groups.is_empty() {
                ui.add_space(6.0);
                ui.label(RichText::new("No library tools match.").color(DARK.muted));
            }
            for (group, rows) in groups {
                group_title(ui, group.title());
                for row in rows {
                    let selected = chosen == Some(Chosen::Library(row.id));
                    let response = ui
                        .horizontal(|ui| {
                            let r = ui.selectable_label(selected, &row.name);
                            if row.in_project {
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        crate::shell::chip(ui, "in project");
                                    },
                                );
                            }
                            r
                        })
                        .inner;
                    if response.on_hover_text(&row.summary).clicked() {
                        acts.push(Act::Choose(Chosen::Library(row.id)));
                    }
                }
            }
            let problems = lib.problems();
            if !problems.is_empty() {
                ui.add_space(8.0);
                ui.label(
                    RichText::new(format!("{} files could not be read:", problems.len()))
                        .color(DARK.warning),
                );
                for p in problems {
                    ui.label(RichText::new(p).small().color(DARK.muted));
                }
            }
        });
}

/// The editor of one material's cutting data.
fn cutting_editor(ui: &mut egui::Ui, draft: &mut CuttingDraft, acts: &mut Vec<Act>) {
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label("Stock material");
            ui.text_edit_singleline(&mut draft.material);
        });
        egui::Grid::new("cutting draft")
            .num_columns(3)
            .show(ui, |ui| {
                for ((label, unit, range), text) in CUTTING.iter().zip(&mut draft.values) {
                    ui.label(*label);
                    ui.add(
                        egui::TextEdit::singleline(text)
                            .desired_width(80.0)
                            .font(egui::TextStyle::Monospace),
                    )
                    .on_hover_text(format!("Allowed: {range}."));
                    ui.label(RichText::new(*unit).color(DARK.muted));
                    ui.end_row();
                }
            });
        if let Some(problem) = &draft.problem {
            ui.label(RichText::new(problem).color(DARK.error));
        }
        ui.horizontal(|ui| {
            if ui.button("Apply").clicked() {
                acts.push(Act::SaveCutting);
            }
            if let Some(material) = &draft.editing
                && ui.button("Remove").clicked()
            {
                acts.push(Act::RemoveCutting(material.clone()));
            }
        });
    });
}

/// A modal in front of the window, drawn with `contents`.
fn modal(ui: &egui::Ui, id: &str, contents: impl FnOnce(&mut egui::Ui)) {
    egui::Modal::new(egui::Id::new(("tool library", id))).show(ui.ctx(), contents);
}

/// A modal's two buttons.
fn buttons(ui: &mut egui::Ui, acts: &mut Vec<Act>, yes: (&str, Act), no: (&str, Act)) {
    ui.horizontal(|ui| {
        if ui.button(yes.0).clicked() {
            acts.push(yes.1);
        }
        if ui.button(no.0).clicked() {
            acts.push(no.1);
        }
    });
}

/// A list group's heading.
fn group_title(ui: &mut egui::Ui, title: &str) {
    ui.add_space(6.0);
    ui.label(
        RichText::new(title.to_uppercase())
            .small()
            .strong()
            .color(DARK.muted),
    );
}

/// A section heading (14.3).
fn heading(ui: &mut egui::Ui, text: &str) {
    ui.add_space(4.0);
    ui.label(
        RichText::new(text.to_uppercase())
            .small()
            .strong()
            .color(DARK.heading),
    );
    ui.add_space(2.0);
}

impl ToolLibraryWindow {
    /// Opens the window on the project's tool `id` with the "Save to
    /// library" confirmation showing (the operation inspector's button).
    pub fn ask_save_to_library(&mut self, app: Option<&mut Application>, id: ToolId) {
        self.open(Purpose::Add, Some(Chosen::Project(id)));
        if self.chosen == Some(Chosen::Project(id)) {
            self.act(Act::AskSaveToLibrary, app);
        }
    }
}

#[cfg(test)]
impl ToolLibraryWindow {
    /// Does what the footer's primary button does, for the tool chosen.
    pub fn add_chosen(
        &mut self,
        app: Option<&mut Application>,
    ) -> (Option<String>, Option<ToolId>) {
        self.act(Act::AddToProject, app)
    }

    /// The tool chosen.
    pub fn chosen(&self) -> Option<Chosen> {
        self.chosen
    }

    /// Makes a new library draft of `kind`.
    pub fn new_tool(&mut self, kind: ToolType) {
        self.act(Act::New(kind), None);
    }

    /// Renames the tool shown, as the Name field does.
    pub fn rename(&mut self, app: Option<&mut Application>, name: &str) {
        self.act(Act::Name(name.to_owned()), app);
    }

    /// Chooses another tool, as a click in the list does.
    pub fn click(&mut self, chosen: Chosen) {
        self.act(Act::Choose(chosen), None);
    }

    /// Whether the discard prompt is showing.
    pub fn asks_to_discard(&self) -> bool {
        matches!(self.confirm, Some(Confirm::Discard(_)))
    }

    /// Answers the discard prompt.
    pub fn answer_discard(&mut self, discard: bool) {
        let act = if discard {
            Act::Discard
        } else {
            Act::KeepEditing
        };
        self.act(act, None);
    }

    /// Saves the draft.
    pub fn save(&mut self) {
        self.act(Act::Save, None);
    }

    /// The library as read.
    pub fn read(&self) -> Option<&ToolLibrary> {
        self.library.as_ref()
    }
}
