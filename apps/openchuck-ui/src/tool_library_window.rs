// SPDX-License-Identifier: AGPL-3.0-or-later
//! The tool library window (specification 12.1; TL-01 to TL-07, TL-11,
//! TL-12; decision records 0014, 0081 and 0127; board 10).
//!
//! Left, the tool list: a search, the tools grouped by type with an "in
//! project" chip, and New tool, Duplicate and Delete. Centre, the tool form
//! (name, type, sizes, material) and, for a custom profile, the profile
//! editor. Right, the cutting data by stock material (TL-05) and the
//! machine tool sets (TL-11). The footer has Import library and Export
//! library (TL-01), Close, and "Add to project".
//!
//! Every library edit is written to the tool's file at once by
//! `openchuck-app`; the default-tool ledger is never touched, so an edited
//! default stays edited and a deleted one stays deleted (TL-12). Adding to
//! the project is one undoable command.

use std::collections::BTreeMap;
use std::path::PathBuf;

use eframe::egui::{self, RichText};
use openchuck_app::Application;
use openchuck_app::project::ParameterValue;
use openchuck_app::project::id::{OperationId, ToolId};
use openchuck_app::tool_edit::{self, starting_profile};
use openchuck_app::tool_library::{
    self as library, ConflictChoice, CuttingRow, DeleteImpact, ImportPreview, ToolType,
};
use openchuck_app::tools::{
    CuttingData, FeedRate, LibraryTool, Millimetres, SpindleSpeed, ToolLibrary, ToolProfile,
    ToolShape,
};

use crate::field_row::{self, Outcome, TextRow};
use crate::floating::{WindowEvent, floating_unscrolled};
use crate::form_panel::{self, FormState};
use crate::theme::DARK;
use crate::tool_panel::{self, ProfileAction};

/// Why the window was opened, which sets its primary action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    /// From the Tools menu: browse and edit; "Add to project" is offered.
    Browse,
    /// To add a tool to the project: "Add to project" is the primary action.
    Add,
    /// To choose the tool of this operation: "Add to project" also gives
    /// the operation the tool.
    Operation(OperationId),
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

    /// The typed values, or the first that is not a number.
    fn data(&self) -> Result<CuttingData, String> {
        let mut numbers = [0.0; 5];
        for ((n, text), (label, ..)) in numbers.iter_mut().zip(&self.values).zip(CUTTING) {
            *n = text
                .trim()
                .parse::<f64>()
                .map_err(|_| format!("{label}: type a number."))?;
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
#[derive(Debug, Clone)]
enum Act {
    Close,
    Choose(ToolId),
    New(ToolType),
    Duplicate,
    AskDelete,
    Delete,
    CancelDelete,
    Name(String),
    Type(ToolType),
    Material(String),
    Field(String, Option<ParameterValue>),
    EditProfile,
    Profile(ProfileAction),
    SaveCutting,
    RemoveCutting(String),
    SetNumber(String, String),
    ImportLibrary,
    ApplyImport,
    CancelImport,
    ExportLibrary,
    AddToProject,
}

/// What the window reports to the shell.
#[derive(Debug, Default)]
pub struct WindowOutcome {
    /// A line for the status bar.
    pub status: Option<String>,
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
    chosen: Option<ToolId>,
    fields: FormState,
    draft: Option<(ToolId, ToolProfile)>,
    segment: Option<usize>,
    cutting: Option<CuttingDraft>,
    delete: Option<(ToolId, DeleteImpact)>,
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
            fields: FormState::default(),
            draft: None,
            segment: None,
            cutting: None,
            delete: None,
            import: None,
            message: None,
        }
    }

    /// Whether the window is open.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Opens the window for `purpose`, reading the library again from its
    /// folder, with `tool` chosen when given.
    pub fn open(&mut self, purpose: Purpose, tool: Option<ToolId>) {
        self.open = true;
        self.purpose = purpose;
        self.reload();
        if tool.is_some() {
            self.choose(tool);
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

    /// The library for a change made outside the window (Save to library),
    /// read once when first asked.
    pub fn library_mut(&mut self) -> Option<&mut ToolLibrary> {
        self.library();
        self.library.as_mut()
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
        if self
            .chosen
            .is_some_and(|id| self.library.as_ref().is_none_or(|l| l.get(id).is_none()))
        {
            self.chosen = None;
        }
    }

    fn choose(&mut self, tool: Option<ToolId>) {
        if self.chosen != tool {
            self.chosen = tool;
            self.fields.clear();
            self.draft = None;
            self.segment = None;
            self.cutting = None;
        }
    }

    fn chosen_tool(&self) -> Option<&LibraryTool> {
        self.library.as_ref()?.get(self.chosen?)
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
        let mut acts = Vec::new();
        let project = app.as_deref().map(Application::project);
        let title = match self.purpose {
            Purpose::Operation(_) => "Tool library: choose the operation's tool",
            _ => "Tool library",
        };
        let (_, event) = floating_unscrolled(ctx, "tool library", title, [1200.0, 760.0], |ui| {
            self.draw(ui, project, &mut acts);
        });
        if event == WindowEvent::CloseRequested {
            acts.push(Act::Close);
        }
        for act in acts {
            if let Some(status) = self.act(act, app.as_deref_mut()) {
                outcome.status = Some(status);
            }
        }
        outcome
    }

    #[allow(clippy::too_many_lines)]
    fn draw(
        &mut self,
        ui: &mut egui::Ui,
        project: Option<&openchuck_app::project::Project>,
        acts: &mut Vec<Act>,
    ) {
        egui::Panel::bottom("tool library footer")
            .resizable(false)
            .show(ui, |ui| self.footer(ui, project, acts));
        let Some(library) = &self.library else {
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
                list(ui, library, project, &mut self.search, self.chosen, acts)
            });
        let tool = self.chosen_tool().cloned();
        egui::Panel::right("tool library data")
            .default_size(300.0)
            .resizable(true)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| match &tool {
                    Some(tool) => self.data(ui, tool, acts),
                    None => {
                        ui.label(RichText::new("Choose a tool.").color(DARK.muted));
                    }
                });
            });
        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| match &tool {
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
    fn footer(
        &self,
        ui: &mut egui::Ui,
        project: Option<&openchuck_app::project::Project>,
        acts: &mut Vec<Act>,
    ) {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let readable = self.library.is_some();
            if ui
                .add_enabled(readable, egui::Button::new("Import library…"))
                .on_hover_text("Copy tools from another library folder or a tool file")
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
                ui.label(RichText::new(text).color(colour));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let in_project = project.is_some_and(|p| {
                    self.chosen
                        .is_some_and(|id| p.tools().iter().any(|t| t.id == id))
                });
                let (label, enabled, reason) = match (project, self.chosen, self.purpose) {
                    (None, ..) => ("Add to project", false, "Open a project first"),
                    (_, None, _) => ("Add to project", false, "Choose a tool first"),
                    (_, _, Purpose::Operation(_)) if in_project => {
                        ("Use for the operation", true, "")
                    }
                    _ if in_project => {
                        ("Add to project", false, "The project has this tool already")
                    }
                    _ => ("Add to project", true, ""),
                };
                let primary = self.purpose != Purpose::Browse;
                let button = if primary {
                    crate::shell::primary_button(ui, label, enabled)
                } else {
                    ui.add_enabled(enabled, egui::Button::new(label))
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

    /// The tool form: name, type, sizes and material, then the profile
    /// editor for a custom profile (12.1, centre).
    fn tool_form(&mut self, ui: &mut egui::Ui, tool: &LibraryTool, acts: &mut Vec<Act>) {
        heading(ui, "Tool");
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
        let current = Some(
            tool.tool
                .material
                .as_ref()
                .and_then(|m| materials.iter().position(|x| x == m))
                .unwrap_or(0),
        );
        if let Some(i) = field_row::choice_row(
            ui,
            "tool material",
            "Material",
            "What the tool is made of (TL-04). Not the stock material.",
            &labels,
            current,
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
        match (&mut self.draft, &tool.tool.shape) {
            (Some((id, draft)), _) if *id == tool.id => {
                heading(ui, "Profile");
                let action = tool_panel::editor(ui, &tool.tool, draft, &mut self.segment, true);
                if action != ProfileAction::None {
                    acts.push(Act::Profile(action));
                }
            }
            (_, ToolShape::Custom { .. }) => {
                heading(ui, "Profile");
                if ui.button("Edit the profile…").clicked() {
                    acts.push(Act::EditProfile);
                }
            }
            _ => {
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

    /// The right column: cutting data by stock material (TL-05) and the
    /// machine tool sets (TL-11).
    fn data(&mut self, ui: &mut egui::Ui, tool: &LibraryTool, acts: &mut Vec<Act>) {
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
        heading(ui, "Machine tool sets");
        let Some(library) = &self.library else { return };
        let sets = library::tool_set_numbers(library, tool.id);
        if sets.is_empty() {
            ui.label(RichText::new("This library has no machine tool sets.").color(DARK.muted));
        }
        for (machine, number) in sets {
            let key = format!("set:{machine}");
            let row = TextRow {
                key: &key,
                label: &machine,
                description: "The number this tool gets when it is added to a project for this \
                              machine. Empty: not in the set.",
                range: Some("1 or more, not another tool's number in this set"),
                applied: number.map(|n| n.to_string()).unwrap_or_default(),
                hint: Some("not in the set".to_owned()),
                enabled: true,
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

    /// The delete confirmation and the import review.
    fn dialogs(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Act>) {
        if let Some((_, impact)) = &self.delete {
            egui::Modal::new(egui::Id::new("delete library tool")).show(ui.ctx(), |ui| {
                ui.heading(format!("Delete \"{}\" from the library?", impact.name));
                ui.label("Its file is deleted. This cannot be undone.");
                if impact.in_project {
                    ui.label("This project keeps its own copy of the tool, and its operations still use it.");
                }
                for (machine, number) in &impact.tool_sets {
                    ui.label(format!("It is T{number} in the {machine} tool set: it is taken out of the set."));
                }
                ui.label(
                    RichText::new("A default tool that is deleted is not installed again.")
                        .color(DARK.muted),
                );
                ui.horizontal(|ui| {
                    if ui.button("Delete").clicked() {
                        acts.push(Act::Delete);
                    }
                    if ui.button("Cancel").clicked() {
                        acts.push(Act::CancelDelete);
                    }
                });
            });
        }
        if let Some((preview, choices)) = &mut self.import {
            egui::Modal::new(egui::Id::new("import library")).show(ui.ctx(), |ui| {
                ui.heading("Import tools");
                ui.label(format!(
                    "From {}: {} new, {} the same as yours, {} that differ from yours.",
                    preview.from.display(),
                    preview.added.len(),
                    preview.unchanged.len(),
                    preview.conflicts.len()
                ));
                if !preview.conflicts.is_empty() {
                    ui.label("For each tool that differs, choose what to keep:");
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
                                    ui.radio_value(
                                        choice,
                                        ConflictChoice::Replace,
                                        "Replace with the import",
                                    );
                                    ui.radio_value(choice, ConflictChoice::KeepBoth, "Keep both");
                                });
                            }
                        });
                }
                ui.horizontal(|ui| {
                    if ui.button("Import").clicked() {
                        acts.push(Act::ApplyImport);
                    }
                    if ui.button("Cancel").clicked() {
                        acts.push(Act::CancelImport);
                    }
                });
            });
        }
    }

    /// "Add to project" for the chosen tool, as the footer's button does.
    #[cfg(test)]
    pub fn add_chosen(&mut self, app: Option<&mut Application>) -> Option<String> {
        self.act(Act::AddToProject, app)
    }

    /// Does `act`; returns a status line for the shell, when there is one.
    #[allow(clippy::too_many_lines)]
    fn act(&mut self, act: Act, app: Option<&mut Application>) -> Option<String> {
        let chosen = self.chosen;
        let mut status = None;
        let Some(library) = &mut self.library else {
            if matches!(act, Act::Close) {
                self.open = false;
            }
            return None;
        };
        let result: Result<Option<String>, String> = match act {
            Act::Close => {
                self.open = false;
                self.delete = None;
                self.import = None;
                Ok(None)
            }
            Act::Choose(id) => {
                self.choose(Some(id));
                Ok(None)
            }
            Act::New(kind) => library::new_tool(library, kind)
                .map(|id| {
                    self.search.clear();
                    self.choose(Some(id));
                    Some(format!(
                        "Made a new {} in the library.",
                        kind.label().to_lowercase()
                    ))
                })
                .map_err(|e| e.to_string()),
            Act::Duplicate => match chosen {
                Some(id) => library::duplicate_tool(library, id)
                    .map(|copy| {
                        self.choose(Some(copy));
                        Some("Duplicated the tool in the library.".to_owned())
                    })
                    .map_err(|e| e.to_string()),
                None => Ok(None),
            },
            Act::AskDelete => {
                let project = app.as_deref().map(Application::project);
                match chosen.map(|id| (id, library::delete_impact(library, project, id))) {
                    Some((id, Ok(impact))) => {
                        self.delete = Some((id, impact));
                        Ok(None)
                    }
                    Some((_, Err(e))) => Err(e.to_string()),
                    None => Ok(None),
                }
            }
            Act::Delete => match self.delete.take() {
                Some((id, impact)) => library::delete_tool(library, id)
                    .map(|()| {
                        self.chosen = None;
                        Some(format!("Deleted \"{}\" from the library.", impact.name))
                    })
                    .map_err(|e| e.to_string()),
                None => Ok(None),
            },
            Act::CancelDelete => {
                self.delete = None;
                Ok(None)
            }
            Act::Name(typed) => match chosen {
                Some(id) => {
                    let r = library::set_name(library, id, &typed)
                        .map_err(|e| form_panel::refusal(&e.to_string()));
                    self.fields.result("name", r);
                    Ok(None)
                }
                None => Ok(None),
            },
            Act::Type(kind) => match chosen {
                Some(id) => {
                    let r = library::set_type(library, id, kind)
                        .map_err(|e| form_panel::refusal(&e.to_string()));
                    self.fields.result("type", r);
                    self.draft = None;
                    Ok(None)
                }
                None => Ok(None),
            },
            Act::Material(material) => match chosen {
                Some(id) => library::set_material(library, id, &material)
                    .map(|()| None)
                    .map_err(|e| e.to_string()),
                None => Ok(None),
            },
            Act::Field(name, value) => {
                if let Some(id) = chosen {
                    let r = match &value {
                        Some(v) => library::set_field(library, id, &name, v)
                            .map_err(|e| form_panel::refusal(&e.to_string())),
                        None => Err("A tool size needs a value. Not applied.".to_owned()),
                    };
                    self.fields.result(&name, r);
                }
                Ok(None)
            }
            Act::EditProfile => {
                if let Some(tool) = chosen.and_then(|id| library.get(id))
                    && let Some(p) = starting_profile(&tool.tool)
                {
                    self.draft = Some((tool.id, p));
                    self.segment = None;
                }
                Ok(None)
            }
            Act::Profile(action) => self.profile_action(action),
            Act::SaveCutting => self.save_cutting(),
            Act::RemoveCutting(material) => match chosen {
                Some(id) => library::remove_cutting_data(library, id, &material)
                    .map(|()| {
                        self.cutting = None;
                        Some(format!("Removed the cutting data for {material}."))
                    })
                    .map_err(|e| e.to_string()),
                None => Ok(None),
            },
            Act::SetNumber(machine, typed) => {
                if let Some(id) = chosen {
                    let key = format!("set:{machine}");
                    let typed = typed.trim().trim_start_matches(['T', 't']);
                    let number = if typed.is_empty() {
                        Ok(None)
                    } else {
                        typed.parse::<u32>().map(Some).map_err(|_| {
                            "Type a whole tool number, or leave it empty. Not applied.".to_owned()
                        })
                    };
                    let r = number.and_then(|n| {
                        library::set_tool_set_number(library, &machine, id, n)
                            .map_err(|e| form_panel::refusal(&e.to_string()))
                    });
                    self.fields.result(&key, r);
                }
                Ok(None)
            }
            Act::ImportLibrary => {
                let picked = rfd::FileDialog::new()
                    .set_title("Import tools from a library folder")
                    .pick_folder();
                match picked {
                    Some(folder) => library::preview_import(library, &folder)
                        .map(|preview| {
                            self.import = Some((preview, BTreeMap::new()));
                            None
                        })
                        .map_err(|e| e.to_string()),
                    None => Ok(None),
                }
            }
            Act::ApplyImport => match self.import.take() {
                Some((preview, choices)) => library::apply_import(library, &preview, |id| {
                    choices
                        .get(&id)
                        .copied()
                        .unwrap_or(ConflictChoice::KeepMine)
                })
                .map(|o| {
                    Some(format!(
                        "Imported: {} added, {} replaced, {} kept.",
                        o.added.len(),
                        o.replaced.len(),
                        o.kept.len()
                    ))
                })
                .map_err(|e| e.to_string()),
                None => Ok(None),
            },
            Act::CancelImport => {
                self.import = None;
                Ok(None)
            }
            Act::ExportLibrary => {
                let picked = rfd::FileDialog::new()
                    .set_title("Export the tool library into a folder")
                    .pick_folder();
                match picked {
                    Some(folder) => library::export_tools(library, &[], &folder)
                        .map(|files| {
                            Some(format!(
                                "Exported {} tools to {}.",
                                files.len(),
                                folder.display()
                            ))
                        })
                        .map_err(|e| e.to_string()),
                    None => Ok(None),
                }
            }
            Act::AddToProject => match (app, chosen) {
                (Some(app), Some(id)) => add_to_project(app, library, id, self.purpose),
                _ => Ok(None),
            },
        };
        match result {
            Ok(Some(text)) => {
                status = Some(text.clone());
                self.message = Some((text, false));
            }
            Ok(None) => {}
            Err(e) => self.message = Some((e, true)),
        }
        status
    }

    fn profile_action(&mut self, action: ProfileAction) -> Result<Option<String>, String> {
        let Some(library) = &mut self.library else {
            return Ok(None);
        };
        match action {
            ProfileAction::None => Ok(None),
            ProfileAction::Cancel => {
                self.draft = None;
                self.segment = None;
                Ok(None)
            }
            ProfileAction::Apply => match self.draft.clone() {
                Some((id, profile)) => library::set_profile(library, id, profile)
                    .map(|()| {
                        self.draft = None;
                        Some("Saved the profile to the library.".to_owned())
                    })
                    .map_err(|e| e.to_string()),
                None => Ok(None),
            },
            ProfileAction::ImportFile => {
                let picked = rfd::FileDialog::new()
                    .set_title("Import a profile file")
                    .add_filter("Profile file", &["toml", "txt"])
                    .pick_file();
                match (picked, &mut self.draft) {
                    (Some(file), Some((_, draft))) => openchuck_app::tools::load_profile(&file)
                        .map(|p| {
                            *draft = p;
                            self.segment = None;
                            Some(format!(
                                "Read the profile from {}. Apply to keep it.",
                                file.display()
                            ))
                        })
                        .map_err(|e| e.to_string()),
                    _ => Ok(None),
                }
            }
            ProfileAction::ExportFile => {
                let picked = rfd::FileDialog::new()
                    .set_title("Export the profile")
                    .add_filter("Profile file", &["toml"])
                    .save_file();
                let tool = self
                    .chosen
                    .and_then(|id| library.get(id))
                    .map(|t| t.tool.clone());
                match (picked, tool, &self.draft) {
                    (Some(file), Some(tool), Some((_, draft))) => {
                        library::export_profile(&tool, draft, &file)
                            .map(|()| Some(format!("Wrote the profile to {}.", file.display())))
                            .map_err(|e| e.to_string())
                    }
                    _ => Ok(None),
                }
            }
        }
    }

    fn save_cutting(&mut self) -> Result<Option<String>, String> {
        let (Some(library), Some(id), Some(draft)) =
            (&mut self.library, self.chosen, &mut self.cutting)
        else {
            return Ok(None);
        };
        let material = draft.material.trim().to_owned();
        if material.is_empty() {
            draft.problem = Some("Name the stock material first.".to_owned());
            return Ok(None);
        }
        let saved = draft.data().and_then(|data| {
            if let Some(old) = draft.editing.as_deref().filter(|old| *old != material) {
                // Renamed: the old entry goes once the new one is accepted.
                library::set_cutting_data(library, id, &material, data)
                    .and_then(|()| library::remove_cutting_data(library, id, old))
                    .map_err(|e| e.to_string())
            } else {
                library::set_cutting_data(library, id, &material, data).map_err(|e| e.to_string())
            }
        });
        match saved {
            Ok(()) => {
                self.cutting = None;
                Ok(Some(format!("Saved the cutting data for {material}.")))
            }
            Err(e) => {
                draft.problem = Some(form_panel::refusal(&e));
                Ok(None)
            }
        }
    }
}

/// Adds library tool `id` to the project as one undoable command, and for an
/// operation's choice gives the operation the tool (TL-07, TL-11).
fn add_to_project(
    app: &mut Application,
    library: &ToolLibrary,
    id: ToolId,
    purpose: Purpose,
) -> Result<Option<String>, String> {
    let have = app.project().tools().iter().any(|t| t.id == id);
    let mut commands = Vec::new();
    let mut text = String::new();
    if !have {
        let (command, controller, source) =
            library::add_to_project(app.project(), library, id).map_err(|e| e.to_string())?;
        commands.push(command);
        let how = match source {
            openchuck_app::tools::NumberSource::ToolSet => "from the machine's tool set".to_owned(),
            openchuck_app::tools::NumberSource::ToolSetTaken { wanted } => format!(
                "the next free number; the machine's tool set gives T{wanted}, which another tool here uses"
            ),
            openchuck_app::tools::NumberSource::NextFree => "the next free number".to_owned(),
        };
        text = format!("Added to the project as T{} ({how}).", controller.number);
    }
    if let Purpose::Operation(operation) = purpose {
        // The tool must be in the project before the operation can name it,
        // so the commands run as one batch.
        let mut preview = app.project().clone();
        for c in &commands {
            if let openchuck_app::Command::Add { item, placement } = c {
                preview
                    .insert(item.clone(), *placement)
                    .map_err(|e| e.to_string())?;
            }
        }
        commands
            .push(library::set_operation_tool(&preview, operation, id).map_err(|e| e.to_string())?);
        text = format!("{text} The operation now uses it.")
            .trim()
            .to_owned();
    }
    let command = match commands.len() {
        0 => return Ok(None),
        1 => commands.remove(0),
        _ => openchuck_app::Command::Batch(commands),
    };
    app.execute(command).map_err(|e| e.to_string())?;
    Ok(Some(text))
}

/// The tool list (12.1, left): search, groups, and New tool, Duplicate and
/// Delete under it.
fn list(
    ui: &mut egui::Ui,
    library: &ToolLibrary,
    project: Option<&openchuck_app::project::Project>,
    search: &mut String,
    chosen: Option<ToolId>,
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
                ui.menu_button("New tool ▾", |ui| {
                    for kind in ToolType::ALL {
                        if ui.button(kind.label()).clicked() {
                            acts.push(Act::New(kind));
                            ui.close();
                        }
                    }
                });
                if ui
                    .add_enabled(chosen.is_some(), egui::Button::new("Duplicate"))
                    .clicked()
                {
                    acts.push(Act::Duplicate);
                }
                if ui
                    .add_enabled(chosen.is_some(), egui::Button::new("Delete"))
                    .on_hover_text("Delete the tool from the library; a project keeps its copy")
                    .clicked()
                {
                    acts.push(Act::AskDelete);
                }
            });
            ui.add_space(6.0);
        });
    egui::ScrollArea::vertical().show(ui, |ui| {
        let groups = library::tool_list(library, project, search);
        if groups.is_empty() {
            ui.label(RichText::new("No tools match.").color(DARK.muted));
        }
        for (group, rows) in groups {
            ui.add_space(6.0);
            ui.label(
                RichText::new(group.title().to_uppercase())
                    .small()
                    .strong()
                    .color(DARK.muted),
            );
            for row in rows {
                let response = ui
                    .horizontal(|ui| {
                        let r = ui.selectable_label(chosen == Some(row.id), &row.name);
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
                    .inner
                    .on_hover_text(&row.summary);
                if response.clicked() {
                    acts.push(Act::Choose(row.id));
                }
            }
        }
        let problems = library.problems();
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
            if ui.button("Save").clicked() {
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
