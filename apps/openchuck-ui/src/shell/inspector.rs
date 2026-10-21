// SPDX-License-Identifier: AGPL-3.0-or-later
//! The inspector: the selected item's header, error banner and sections
//! (UI-07, UI-11, UI-13, UI-25; interface specification sections 8, 11.2,
//! 11.3, 11.4 and 11.7; slice 22 lane X4, decision records 0114 C10, C14 to
//! C16 and 0123).
//!
//! The order is section 8's: the header (the name in an editable field, and
//! a status chip in the words of the status mark), the error banner when the
//! item is in error, then the sections, each collapsible with its open state
//! remembered per item kind. Every value comes from a view model in
//! `openchuck-app`; every change is one undoable command.

use eframe::egui::{self, RichText, Stroke};
use openchuck_app::Command;
use openchuck_app::overview::{ItemRef, ItemState, MessageAction, SetupPart, Severity, Step};
use openchuck_app::project::id::{ModelId, OperationId, SelectionSetId, SetupId, ToolId};
use openchuck_app::project::{BoundaryOptions, Item, OnBreak, Setup};
use openchuck_app::requirements::RequirementField;
use openchuck_app::selection::{SelectionRole, SelectionSummary};
use openchuck_app::setup_form::{self, SetupRow, StockShape};
use openchuck_app::tool_library;

use super::{Dialog, Selection, Shell, TreeAction, frame, marks};
use crate::field_row::{self, Outcome, TextRow};
use crate::form_panel;
use crate::machine_panel::MachinePanel;
use crate::model_panel::ModelPanel;
use crate::theme::DARK;
use crate::workholding_panel::WorkholdingPanel;

/// The key of the header's name in the form state.
const NAME: &str = "\u{0}name";

/// The key of the Tool section's tool number in the form state.
const TOOL_NUMBER: &str = "\u{0}tool number";

/// What the user asked of an operation's Tool section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ToolAct {
    /// Use another project tool.
    Use(ToolId),
    /// Choose a tool in the tool library window.
    Library,
    /// Set the project's tool number to the typed text.
    Number(ToolId, String),
    /// Replace the project's copy with the library tool (TL-07).
    Update(ToolId),
    /// Write the project's copy into the library, after the window's
    /// confirmation naming the fields.
    SaveToLibrary(ToolId),
    /// Edit the project's copy in the tool library window.
    Edit(ToolId),
}

/// The settings key of a setup inspector section.
pub(super) fn section_key(part: SetupPart) -> &'static str {
    match part {
        SetupPart::Machine => "machine",
        SetupPart::Stock => "stock",
        SetupPart::PartPosition => "part position",
        SetupPart::WorkOffset => "work offset",
        SetupPart::Workholding => "workholding",
    }
}

/// The setup sections open by default: all of them.
pub(super) const SECTION_KEYS: [&str; 5] = [
    "machine",
    "part position",
    "stock",
    "work offset",
    "workholding",
];

/// A setup inspector section's title (specification section 11.3).
pub(super) fn section_title(part: SetupPart) -> &'static str {
    match part {
        SetupPart::Machine => "Machine",
        SetupPart::Workholding => "Workholding and tabs",
        other => other.name(),
    }
}

/// What the error banner offers (section 8; board 3b).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BannerAction {
    /// Pick the set again on the part.
    Reselect(SelectionSetId),
    /// Show the set as it was made, in the view.
    ShowMissing(SelectionSetId),
}

/// The error banner (section 8): what cannot be done, why, and the actions
/// that fix it, in the error box's colours. Returns the action clicked.
fn banner<A: Copy>(
    ui: &mut egui::Ui,
    title: &str,
    cause: &str,
    actions: &[(&str, A)],
) -> Option<A> {
    let boxed = DARK.error_box;
    let mut clicked = None;
    egui::Frame::new()
        .fill(boxed.bg)
        .stroke(Stroke::new(1.0, boxed.border))
        .corner_radius(crate::theme::RADIUS)
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                marks::paint(
                    ui.painter(),
                    ui.cursor().min + egui::vec2(6.0, 8.0),
                    ItemState::Error,
                    marks::SIZE,
                );
                ui.add_space(16.0);
                ui.label(RichText::new(title).strong().color(boxed.text));
            });
            ui.label(RichText::new(cause).color(boxed.text));
            ui.horizontal_wrapped(|ui| {
                for (i, (label, action)) in actions.iter().enumerate() {
                    let button = if i == 0 {
                        frame::primary_button(ui, label, true)
                    } else {
                        ui.button(*label)
                    };
                    if button.clicked() {
                        clicked = Some(*action);
                    }
                }
            });
        });
    clicked
}

/// A status's cause without the state's name before it, as a sentence.
fn cause_text(cause: &str, state: ItemState) -> String {
    cause
        .strip_prefix(state.name())
        .and_then(|c| c.strip_prefix(": "))
        .unwrap_or(cause)
        .to_owned()
}

impl Shell {
    /// The inspector: the chosen item (specification section 8).
    pub(super) fn properties(&mut self, ui: &mut egui::Ui) {
        if self.step == Step::Simulate {
            self.results_inspector(ui);
            return;
        }
        match self.selected {
            Some(Selection::Operation(id)) => self.operation_properties(ui, id),
            Some(Selection::Model(id)) => self.model_properties(ui, id),
            Some(Selection::Setup(id)) => self.setup_properties(ui, id),
            Some(Selection::Set(id)) => self.set_properties(ui, id),
            Some(Selection::Picking) => {
                ui.heading("Selecting on the part");
                ui.label(
                    "Click the part to select. The Selection sets tab of the dock saves and \
                     edits sets; Esc stops picking.",
                );
                if let Some(app) = &mut self.app
                    && let Some(error) = self.selection.options(ui, app)
                {
                    self.dialog = Some(Dialog::Error(error));
                }
            }
            None => {
                ui.weak("Choose an item in the project panel to see it here.");
            }
        }
    }

    /// The results inspector of the Simulate step (specification 11.5).
    fn results_inspector(&mut self, ui: &mut egui::Ui) {
        let run = self.run.last().cloned();
        // An out-of-date result says so first, with the way to simulate
        // again, and its numbers are dimmed (issue 260).
        if self.out_of_date_banner(ui) {
            self.primary();
        }
        let stale = self
            .out_of_date
            .reason()
            .map(|_| super::out_of_date::STALE_OPACITY);
        let Some(app) = &mut self.app else { return };
        if self.results.show(ui, app, run.as_ref(), stale) {
            self.play
                .set_colour_by(self.results.colour_by, &mut self.viewport);
        }
    }

    /// The header: the name in an editable field (when the item has a name of
    /// its own) and the status chip (section 8).
    fn header(&mut self, ui: &mut egui::Ui, item: ItemRef, name: &str, editable: bool) {
        if editable {
            if let Outcome::Commit(typed) = field_row::name_field(ui, &mut self.form, NAME, name) {
                let result = self.rename_checked(item, &typed);
                self.form.result(NAME, result);
            }
            field_row::name_problem(ui, &self.form, NAME);
        } else {
            ui.add(egui::Label::new(RichText::new(name).heading()).truncate());
        }
        frame::status_chip(ui, self.overview.status(item));
    }

    /// A selection set: its size, its problem and where it is used.
    fn set_properties(&mut self, ui: &mut egui::Ui, id: SelectionSetId) {
        let Some(app) = &self.app else { return };
        let summaries = app.selection_summaries(&[]);
        let Some(s) = summaries.into_iter().find(|s| s.id == id) else {
            self.selected = Some(Selection::Picking);
            return;
        };
        self.header(ui, ItemRef::SelectionSet(id), &s.name, true);
        ui.label(format!("{} {}", s.count, s.kind));
        if let Some(path) = &s.model_path {
            ui.label(RichText::new(format!("On {path}")).color(DARK.muted));
        }
        if let Some(problem) = &s.problem
            && let Some(BannerAction::Reselect(set)) = banner(
                ui,
                "This set can't be used",
                &form_panel::refusal(problem).replace(" Not applied.", ""),
                &[("Reselect in the view", BannerAction::Reselect(id))],
            )
        {
            self.message_action(MessageAction::Reselect(set));
        }
        if s.uses.is_empty() {
            ui.label(RichText::new("No operation uses it.").color(DARK.muted));
        } else {
            for u in &s.uses {
                ui.label(format!("Used by {}", u.name));
            }
        }
        // A set that is selected also turns picking on, so the angles and
        // options stay in reach here (decision 0153).
        if let Some(app) = &mut self.app
            && let Some(error) = self.selection.options(ui, app)
        {
            self.dialog = Some(Dialog::Error(error));
        }
    }

    /// The setup inspector: its five sections (specification section 11.3),
    /// each with its row's status mark in the header.
    fn setup_properties(&mut self, ui: &mut egui::Ui, id: SetupId) {
        let Some(app) = &self.app else { return };
        let Some(setup) = app.project().setup(id).cloned() else {
            self.selected = None;
            return;
        };
        self.header(ui, ItemRef::Setup(id), &setup.name, true);
        // What the setup still needs (issues 210 and 224): a count with a jump
        // to the first one, and the words under the section that holds it.
        let needs = self
            .app
            .as_ref()
            .map(|a| a.setup_requirements(id))
            .unwrap_or_default();
        let section_of = |field: &RequirementField| match field {
            RequirementField::Machine => SetupPart::Machine,
            RequirementField::Part => SetupPart::PartPosition,
            _ => SetupPart::Stock,
        };
        let mut jump_to = None;
        if let Some(first) = needs.first() {
            ui.horizontal(|ui| {
                marks::mark(ui, ItemState::Warning, "Required values are missing");
                let text = match needs.len() {
                    1 => "1 value is missing".to_owned(),
                    n => format!("{n} values are missing"),
                };
                ui.label(RichText::new(text).color(DARK.warning));
                if ui
                    .small_button("Show first")
                    .on_hover_text(format!("Go to: {}", first.label))
                    .clicked()
                {
                    jump_to = Some(section_of(&first.field));
                }
            });
        }
        let focus = self.setup_section.take().or(jump_to);
        let mut edited: Option<(Setup, Option<String>)> = None;
        let mut command = None;
        for part in SetupPart::ALL {
            let key = section_key(part);
            let open = self.settings.section_open("setup", key, true);
            let status = self.overview.status(ItemRef::SetupPart(id, part)).cloned();
            let Some(app) = &self.app else { return };
            let copy = app.project().machine().cloned();
            let chips: Vec<(String, egui::Color32)> = match part {
                SetupPart::Machine => copy
                    .as_ref()
                    .map(|_| ("copy stored in project".to_owned(), DARK.muted))
                    .into_iter()
                    .collect(),
                SetupPart::Workholding => {
                    let row = openchuck_app::overview::setup_row(app.project(), &setup, part);
                    let mut chips: Vec<_> = row.chip.map(|c| (c, DARK.muted)).into_iter().collect();
                    if chips.is_empty() {
                        chips.push(("None".to_owned(), DARK.muted));
                    }
                    if self
                        .workholding_panel
                        .as_ref()
                        .is_some_and(|p| p.setup() == id && p.has_unapplied_changes())
                    {
                        chips.push(("Not applied".to_owned(), DARK.warning));
                    }
                    chips
                }
                _ => Vec::new(),
            };
            let section = field_row::section(
                ui,
                egui::Id::new(("setup section", key)),
                section_title(part),
                open,
                focus == Some(part),
                |ui| {
                    if let Some(s) = &status
                        && s.state != ItemState::UpToDate
                    {
                        marks::mark(ui, s.state, &s.cause);
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        for (chip, colour) in chips.iter().rev() {
                            field_row::chip(ui, chip, *colour);
                        }
                    });
                },
                |ui| {
                    if let Some(need) = needs.iter().find(|r| section_of(&r.field) == part) {
                        field_row::warning_line(ui, &need.text);
                    }
                    match part {
                        SetupPart::Machine => {
                            let panel =
                                self.machine_panel.get_or_insert_with(MachinePanel::default);
                            let copy_id = app.project().machine_id().map(str::to_owned);
                            if let Some(c) = panel.show(ui, copy.as_ref(), copy_id.as_deref()) {
                                command = Some(c);
                            }
                        }
                        SetupPart::Stock => {
                            let part_extent = self
                                .workholding_panel
                                .as_ref()
                                .and_then(|p| p.part_extent());
                            if let Some(e) = stock_section(
                                ui,
                                &mut self.form,
                                &setup,
                                copy.as_ref(),
                                part_extent,
                            ) {
                                edited = Some(e);
                            }
                        }
                        SetupPart::PartPosition => {
                            if let Some(e) = rows(
                                ui,
                                &mut self.form,
                                &setup,
                                copy.as_ref(),
                                &setup_form::part_position_rows(&setup),
                            ) {
                                edited = Some(e);
                            }
                            field_row::read_only_row(
                                ui,
                                "Rotate about X",
                                "The model's placement steps turn it about the rotary axis.",
                                "Set by the model's placement steps",
                            );
                        }
                        SetupPart::WorkOffset => {
                            if let Some(e) =
                                clearance_section(ui, &mut self.form, &setup, copy.as_ref())
                            {
                                edited = Some(e);
                            }
                        }
                        SetupPart::Workholding => {
                            let context = workholding_context(&self.overview, app.project(), id);
                            let panel = self
                                .workholding_panel
                                .get_or_insert_with(|| WorkholdingPanel::new(&setup));
                            if let Some(c) = panel.show(ui, app, &setup, &context) {
                                command = Some(c);
                            }
                        }
                    }
                },
            );
            if focus == Some(part) {
                section.header.scroll_to_me(Some(egui::Align::TOP));
            }
            if section.open != open && focus != Some(part) {
                self.settings
                    .set_section_open("setup", key, section.open, &SECTION_KEYS);
                self.settings_touched();
            }
        }
        if let Some((setup, key)) = edited {
            command = Some(Command::Replace(Item::Setup(setup)));
            self.pending_field = key;
        }
        if let Some(command) = command {
            let key = self.pending_field.take();
            let Some(app) = &mut self.app else { return };
            match app.execute(command) {
                Ok(()) => {
                    if let Some(key) = key {
                        self.form.result(&key, Ok(()));
                    }
                    self.rebuild_scene(false);
                }
                Err(e) => match key {
                    Some(key) => self
                        .form
                        .result(&key, Err(form_panel::refusal(&e.to_string()))),
                    None => self.dialog = Some(Dialog::Error(e.to_string())),
                },
            }
        }
    }

    /// A model's file, units, placement and checks (IMP-03, IMP-04, IMP-05;
    /// section 11.2).
    fn model_properties(&mut self, ui: &mut egui::Ui, id: ModelId) {
        let Some(app) = &self.app else { return };
        let Some(reference) = app.project().models().iter().find(|m| m.id == id).cloned() else {
            self.selected = None;
            return;
        };
        let name = reference
            .path
            .as_str()
            .rsplit('/')
            .next()
            .unwrap_or(reference.path.as_str())
            .to_owned();
        self.header(ui, ItemRef::Model(id), &name, false);
        let Some(app) = &mut self.app else { return };
        let panel = match &mut self.model_panel {
            Some(p) if p.model() == id => p,
            _ => self.model_panel.insert(ModelPanel::new(id)),
        };
        let outcome = panel.show(ui, app, &reference, &mut self.settings);
        if outcome.settings_changed {
            self.settings_touched();
        }
        if outcome.choose_replacement
            && let Some(file) = super::pick_model(reference.path.as_str(), self.app.as_ref())
            && let Some(app) = &self.app
            && let Some(panel) = &mut self.model_panel
        {
            panel.replace_with(
                app.start_replace_model(id, std::path::Path::new(&file))
                    .map_err(|e| format!("The file was not replaced: {e}")),
            );
        }
        let Some(app) = &mut self.app else { return };
        if let Some(prepared) = outcome.replaced
            && let Err(e) = app.replace_model(id, prepared)
        {
            self.dialog = Some(Dialog::Error(format!("The file was not replaced: {e}")));
        }
        if let Some(command) = outcome.command {
            match app.execute(command) {
                Ok(()) => self.rebuild_scene(false),
                Err(e) => self.dialog = Some(Dialog::Error(e.to_string())),
            }
        } else if self
            .app
            .as_ref()
            .is_some_and(|a| a.revision() != self.overview.revision)
        {
            self.rebuild_scene(false);
        }
    }

    /// An operation's Tool section: its tool, the project's tool number and
    /// how the project's copy stands against the library (TL-06, TL-07,
    /// TL-11; decision record 0127). Tools are not in the tree, so this is
    /// where an operation's tool is seen and changed.
    fn tool_section(
        &mut self,
        ui: &mut egui::Ui,
        operation: &openchuck_app::project::Operation,
    ) -> Option<ToolAct> {
        let jump_to_tool =
            self.requirement_jump == Some(openchuck_app::requirements::RequirementField::Tool);
        let app = self.app.as_ref()?;
        let project = app.project();
        let library = self.tool_library.library();
        let shown = tool_library::operation_tool(project, library, operation.id);
        let others: Vec<(ToolId, String)> = project
            .tools()
            .iter()
            .map(|t| (t.id, format!("T{}  {}", t.controller.number, t.name)))
            .collect();
        let open = self
            .settings
            .section_open("operation", "tool section", true);
        let form = &mut self.form;
        let section = field_row::section(
            ui,
            egui::Id::new(("operation section", "tool section")),
            "Tool",
            open,
            false,
            |_| {},
            |ui| {
                let mut act = None;
                match &shown {
                    None => {
                        if jump_to_tool {
                            ui.scroll_to_cursor(Some(egui::Align::Center));
                        }
                        field_row::warning_line(ui, "Choose a tool");
                    }
                    Some(tool) => {
                        ui.label(RichText::new(&tool.name).strong());
                        ui.label(RichText::new(&tool.description).color(DARK.muted));
                        let row = TextRow {
                            key: TOOL_NUMBER,
                            label: "Tool number",
                            description: "The project's number for this tool, used by every \
                                          operation that uses it (TL-06). The post writes it \
                                          as T, with the H and D offsets.",
                            range: Some("1 or more, not another project tool's number"),
                            applied: tool.controller.number.to_string(),
                            enabled: true,
                            ..TextRow::default()
                        };
                        if let Outcome::Commit(typed) = field_row::text_row(ui, form, &row) {
                            act = Some(ToolAct::Number(tool.id, typed));
                        }
                        if let Some((machine, number)) = &tool.tool_set_number {
                            field_row::note_line(
                                ui,
                                &format!("The {machine} tool set gives T{number}."),
                                DARK.muted,
                            );
                        }
                        let colour = match tool.copy {
                            tool_library::CopyState::Same => DARK.ok,
                            tool_library::CopyState::Differs(_) => DARK.warning,
                            _ => DARK.muted,
                        };
                        ui.label(RichText::new(tool.copy.text()).color(colour));
                        ui.horizontal_wrapped(|ui| {
                            let differs = matches!(tool.copy, tool_library::CopyState::Differs(_));
                            if ui
                                .add_enabled(differs, egui::Button::new("Update from library"))
                                .on_hover_text(
                                    "Replace the project's copy with the library's tool, \
                                     keeping the tool number (one undoable step)",
                                )
                                .clicked()
                            {
                                act = Some(ToolAct::Update(tool.id));
                            }
                            let saveable = matches!(
                                tool.copy,
                                tool_library::CopyState::Differs(_)
                                    | tool_library::CopyState::NotInLibrary
                            );
                            if ui
                                .button("Edit tool…")
                                .on_hover_text(
                                    "Edit the project's copy of the tool in the tool library window",
                                )
                                .clicked()
                            {
                                act = Some(ToolAct::Edit(tool.id));
                            }
                            if ui
                                .add_enabled(saveable, egui::Button::new("Save to library…"))
                                .on_hover_text("Write the project's copy into the tool library")
                                .clicked()
                            {
                                act = Some(ToolAct::SaveToLibrary(tool.id));
                            }
                        });
                    }
                }
                ui.horizontal(|ui| {
                    let current = shown.as_ref().map(|t| t.id);
                    let text = others
                        .iter()
                        .find(|(id, _)| Some(*id) == current)
                        .map_or("Choose…", |(_, n)| n.as_str());
                    egui::ComboBox::from_id_salt(("operation tool", operation.id))
                        .selected_text(text)
                        .show_ui(ui, |ui| {
                            for (id, name) in &others {
                                if ui.selectable_label(Some(*id) == current, name).clicked()
                                    && Some(*id) != current
                                {
                                    act = Some(ToolAct::Use(*id));
                                }
                            }
                        });
                    if ui
                        .button("From the library…")
                        .on_hover_text("Choose a tool in the tool library and use it here")
                        .clicked()
                    {
                        act = Some(ToolAct::Library);
                    }
                });
                act
            },
        );
        if section.open != open {
            self.settings.set_section_open(
                "operation",
                "tool section",
                section.open,
                &["tool section"],
            );
            self.settings_touched();
        }
        section.body.flatten()
    }

    /// Does what the Tool section asked.
    pub(super) fn tool_act(&mut self, operation: OperationId, act: ToolAct) {
        if act == ToolAct::Library {
            let current = self
                .app
                .as_ref()
                .and_then(|a| a.project().operation(operation))
                .and_then(|o| o.tool);
            self.open_tool_library(
                super::Purpose::Operation(operation),
                current.map(super::Chosen::Project),
            );
            return;
        }
        let library = self.tool_library.library().cloned();
        let Some(app) = &mut self.app else { return };
        let project = app.project();
        let command = match act {
            ToolAct::Library => return,
            ToolAct::Use(tool) => tool_library::set_operation_tool(project, operation, tool),
            ToolAct::Number(tool, typed) => {
                let typed = typed.trim().trim_start_matches(['T', 't']);
                let result = match typed.parse::<u32>() {
                    Ok(n) => tool_library::set_project_tool_number(project, tool, n)
                        .and_then(|c| app.execute(c))
                        .map_err(|e| form_panel::refusal(&e.to_string())),
                    Err(_) => Err("Type a whole tool number. Not applied.".to_owned()),
                };
                self.form.result(TOOL_NUMBER, result);
                return;
            }
            ToolAct::Update(tool) => match &library {
                Some(lib) => match tool_library::update_from_library(project, lib, tool) {
                    Ok(Some(c)) => Ok(c),
                    Ok(None) => return,
                    Err(e) => Err(e),
                },
                None => return,
            },
            ToolAct::SaveToLibrary(tool) => {
                self.tool_library.ask_save_to_library(Some(app), tool);
                return;
            }
            ToolAct::Edit(tool) => {
                self.tool_library
                    .open(super::Purpose::Add, Some(super::Chosen::Project(tool)));
                return;
            }
        };
        if let Err(e) = command.and_then(|c| app.execute(c)) {
            self.dialog = Some(Dialog::Error(e.to_string()));
        }
    }

    /// An operation's header, error banner, Geometry section and parameter
    /// sections (UI-07; sections 8 and 11.4; board 3b).
    fn operation_properties(&mut self, ui: &mut egui::Ui, id: OperationId) {
        let Some(app) = &self.app else { return };
        let Some(operation) = app.project().operation(id).cloned() else {
            self.selected = None;
            return;
        };
        let summaries = app.selection_summaries(&[]);
        let strategy = app.strategy_label(&operation.strategy);
        let set_options = app.strategy_takes_set_options(&operation.strategy);
        let mut delete = false;
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .small_button("Delete")
                    .on_hover_text("Remove this operation (it can be undone)")
                    .clicked()
                {
                    delete = true;
                }
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    self.header_inline(ui, ItemRef::Operation(id), &operation.name);
                });
            });
        });
        field_row::name_problem(ui, &self.form, NAME);
        frame::status_chip(ui, self.overview.status(ItemRef::Operation(id)));
        ui.label(RichText::new(strategy).color(DARK.muted));

        // What is missing, as soon as the operation exists (issue 224): a
        // count and a jump to the first one, in the same words as under the
        // fields and in the step bar.
        let needs: Vec<openchuck_app::requirements::Requirement> = self
            .overview
            .missing_for(ItemRef::Operation(id))
            .into_iter()
            .cloned()
            .collect();
        if let Some(first) = needs.first() {
            ui.horizontal(|ui| {
                marks::mark(ui, ItemState::Warning, "Required values are missing");
                let text = match needs.len() {
                    1 => "1 value is missing".to_owned(),
                    n => format!("{n} values are missing"),
                };
                ui.label(RichText::new(text).color(DARK.warning));
                if ui
                    .small_button("Show first")
                    .on_hover_text(format!("Go to: {}", first.label))
                    .clicked()
                {
                    self.requirement_jump = Some(first.field.clone());
                }
            });
        }

        // The error banner (board 3b).
        let mut banner_action = None;
        if let Some(status) = self
            .overview
            .status(ItemRef::Operation(id))
            .filter(|s| s.state == ItemState::Error)
        {
            let broken: Option<&SelectionSummary> = summaries.iter().find(|s| {
                s.problem.is_some()
                    && (operation.selections.contains(&s.id)
                        || operation.boundary == Some(s.id)
                        || operation.avoid.contains(&s.id)
                        || operation.check.contains(&s.id))
            });
            let mut actions = Vec::new();
            if let Some(set) = broken {
                actions.push(("Reselect in the view", BannerAction::Reselect(set.id)));
                actions.push(("Show missing", BannerAction::ShowMissing(set.id)));
            }
            banner_action = banner(
                ui,
                "This operation can't be generated",
                &cause_text(&status.cause, ItemState::Error),
                &actions,
            );
            ui.label(
                RichText::new(
                    "Export is blocked while this operation is in error. Turning it off lets \
                     the others export.",
                )
                .color(DARK.muted),
            );
        }

        // The Geometry section: the roles, with the selection warnings under
        // the role that holds the set (11.7).
        let reach: Vec<String> = self
            .overview
            .messages
            .iter()
            .filter(|m| {
                m.source == Some(ItemRef::Operation(id))
                    && m.severity == Severity::Warning
                    && matches!(m.action, Some(MessageAction::EditWorkholding(_)))
            })
            .map(|m| m.text.clone())
            .collect();
        let open = self.settings.section_open("operation", "geometry", true);
        let mut role_edit = None;
        let mut reach_action = None;
        let section = field_row::section(
            ui,
            egui::Id::new(("operation section", "geometry")),
            "Geometry",
            open,
            false,
            |_| {},
            |ui| geometry(ui, &operation, &summaries, &reach, set_options),
        );
        if let Some((edit, action)) = section.body {
            role_edit = edit;
            reach_action = action;
        }
        if section.open != open {
            self.settings
                .set_section_open("operation", "geometry", section.open, &["geometry"]);
            self.settings_touched();
        }

        // The Tool section (TL-06, TL-07, TL-11; decision record 0127).
        if let Some(act) = self.tool_section(ui, &operation) {
            self.tool_act(id, act);
        }

        let Some(app) = &mut self.app else { return };
        // The settings the operation's path warns about are marked, and a
        // "Show setting" link brings the first into view once (decision
        // record 0175).
        let marks = form_panel::Marks {
            warned: app.operation_warning_settings(id),
            jump: self
                .setting_jump
                .take_if(|(op, _)| *op == id)
                .map(|(_, name)| name),
        };
        let edit = match app.parameter_form(id) {
            Ok(form) => {
                let need_refs: Vec<&openchuck_app::requirements::Requirement> =
                    needs.iter().collect();
                let outcome = form_panel::show(
                    ui,
                    &form,
                    &mut self.form,
                    &mut self.settings,
                    &need_refs,
                    self.requirement_jump.as_ref(),
                    &marks,
                );
                if outcome.settings_changed {
                    self.settings_changed = Some(std::time::Instant::now());
                }
                outcome.edit
            }
            Err(e) => {
                ui.label(RichText::new(e.to_string()).color(DARK.error));
                None
            }
        };
        // The jump has been drawn this frame.
        self.requirement_jump = None;
        if let Some(edit) = edit {
            let result = app
                .set_parameter(id, &edit.name, edit.value)
                .map_err(|e| form_panel::refusal(&e.user_message()));
            self.form.result(&edit.name, result);
        }
        let result = match role_edit {
            Some(GeometryEdit::Role(role, set, add)) => {
                app.set_operation_selection(id, role, set, add)
            }
            Some(GeometryEdit::Boundary(options)) => app.set_boundary_options(id, options),
            Some(GeometryEdit::OnBreak(role, set, on_break)) => {
                app.set_on_break(id, role, set, on_break)
            }
            None => Ok(()),
        };
        if let Err(e) = result {
            self.dialog = Some(Dialog::Error(e.to_string()));
        }
        match banner_action {
            Some(BannerAction::Reselect(set)) => self.message_action(MessageAction::Reselect(set)),
            Some(BannerAction::ShowMissing(set)) => {
                self.selection.show_set(Some(set));
                self.dock_tab = super::dock::DockTab::SelectionSets;
                self.viewport.fit();
            }
            None => {}
        }
        match reach_action {
            Some(ReachAction::Show(set, item)) => {
                // Selects the tab or held region (7.5) and shows the set.
                self.selection.show_set(Some(set));
                self.choose_setup(operation.setup, Some(SetupPart::Workholding));
                if let Some(item) = item {
                    if let Some(panel) = &mut self.workholding_panel {
                        panel.open_item(item);
                    }
                    self.viewport.set_selected_solid(Some(item));
                }
            }
            Some(ReachAction::EditWorkholding) => {
                // An unapplied draft of this setup stays (11.7); the view
                // shows the setup the panel edits.
                self.choose_setup(operation.setup, Some(SetupPart::Workholding));
            }
            None => {}
        }
        if delete {
            self.tree_action(TreeAction::Delete(id));
        }
    }

    /// The header's name field inside a row of its own (an operation's, with
    /// Delete beside it).
    fn header_inline(&mut self, ui: &mut egui::Ui, item: ItemRef, name: &str) {
        if let Outcome::Commit(typed) = field_row::name_field(ui, &mut self.form, NAME, name) {
            let result = self.rename_checked(item, &typed);
            self.form.result(NAME, result);
        }
    }
}

/// What the workholding section shows of the rest of the project (11.7): the
/// stored workholding's error, and the selection warnings of the setup's
/// operations, each with the operation's number and name.
fn workholding_context(
    overview: &openchuck_app::overview::Overview,
    project: &openchuck_app::project::Project,
    setup: SetupId,
) -> crate::workholding_panel::Context {
    let stored_error = overview
        .status(ItemRef::SetupPart(setup, SetupPart::Workholding))
        .filter(|s| s.state == ItemState::Error)
        .map(|s| cause_text(&s.cause, ItemState::Error));
    let reach = overview
        .messages
        .iter()
        .filter(|m| m.action == Some(MessageAction::EditWorkholding(setup)))
        .filter_map(|m| match m.source {
            Some(ItemRef::Operation(op)) => project.operation(op).map(|o| {
                (
                    openchuck_app::workholding::operation_label(project, o),
                    m.text.clone(),
                )
            }),
            _ => None,
        })
        .collect();
    crate::workholding_panel::Context {
        stored_error,
        reach,
    }
}

/// What a selection warning's link asks for (11.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReachAction {
    /// Show the set, and select the held region or tab it reaches.
    Show(SelectionSetId, Option<crate::workholding_panel::HeldItem>),
    /// Open the setup's workholding section.
    EditWorkholding,
}

/// The sets of `role` in `operation`.
fn members(
    operation: &openchuck_app::project::Operation,
    role: SelectionRole,
) -> Vec<SelectionSetId> {
    match role {
        SelectionRole::Machine => operation.selections.clone(),
        SelectionRole::Boundary => operation.boundary.into_iter().collect(),
        SelectionRole::Avoid => operation.avoid.clone(),
        SelectionRole::Check => operation.check.clone(),
        _ => Vec::new(),
    }
}

/// A change the Geometry section asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GeometryEdit {
    /// Put a set in a role (`true`) or take it out.
    Role(SelectionRole, SelectionSetId, bool),
    /// New options for the boundary set (decision record 0176).
    Boundary(BoundaryOptions),
    /// What happens where the tool would break an avoid or check set.
    OnBreak(SelectionRole, SelectionSetId, OnBreak),
}

/// The choices of an avoid or check set's option, as the Geometry section
/// names them (decision record 0176 item 3).
const ON_BREAK: [(OnBreak, &str, &str); 3] = [
    (
        OnBreak::CutShort,
        "Cut short",
        "Where the tool would break this set, the pass stops short of it and starts again \
         after it.",
    ),
    (
        OnBreak::Warn,
        "Warn only",
        "The path is kept where the tool would break this set, with a warning saying how far \
         along the face it does.",
    ),
    (
        OnBreak::Ignore,
        "Ignore",
        "The set limits nothing: the tool is not kept clear of it. The operation warns that it \
         limits nothing.",
    ),
];

/// The options of the sets in `role` (decision record 0176 item 3), under
/// its row: the boundary's two limits, or each avoid or check set's choice.
/// A set whose options leave it limiting nothing says so.
fn set_options(
    ui: &mut egui::Ui,
    operation: &openchuck_app::project::Operation,
    role: SelectionRole,
    sets: &[SelectionSetId],
    summaries: &[SelectionSummary],
) -> Option<GeometryEdit> {
    let mut edit = None;
    let indent = crate::theme::width::LABEL_COLUMN + ui.spacing().item_spacing.x;
    let options = &operation.restriction_options;
    let name = |id: &SelectionSetId| {
        summaries
            .iter()
            .find(|s| s.id == *id)
            .map_or_else(|| "(missing set)".to_owned(), |s| s.name.clone())
    };
    match role {
        SelectionRole::Boundary => {
            let id = sets.first()?;
            let mut boundary = options.boundary;
            ui.horizontal_wrapped(|ui| {
                ui.add_space(indent);
                let pass = ui
                    .checkbox(&mut boundary.limits_pass, "Limits the pass")
                    .on_hover_text(
                        "Cut only the rulings that lie inside the boundary, whole, so the pass \
                         stops, or splits with a retract, where the rulings leave it.",
                    );
                let tip = ui
                    .checkbox(&mut boundary.limits_tip, "Limits the tip")
                    .on_hover_text(
                        "Raise the tool tip along each ruling so the bottom of the flank stops \
                         where the ruling enters the boundary. The far end of a ruling cannot \
                         be limited.",
                    );
                if pass.changed() || tip.changed() {
                    edit = Some(GeometryEdit::Boundary(boundary));
                }
            });
            if options.boundary.limits_nothing() {
                limits_nothing(ui, indent, &name(id));
            }
        }
        SelectionRole::Avoid | SelectionRole::Check => {
            for id in sets {
                let current = if role == SelectionRole::Avoid {
                    options.avoid(*id)
                } else {
                    options.check(*id)
                }
                .on_break;
                let set = name(id);
                ui.horizontal_wrapped(|ui| {
                    ui.add_space(indent);
                    ui.label(RichText::new(&set).color(DARK.muted));
                    for (choice, label, tip) in ON_BREAK {
                        if ui
                            .selectable_label(current == choice, label)
                            .on_hover_text(tip)
                            .clicked()
                            && current != choice
                        {
                            edit = Some(GeometryEdit::OnBreak(role, *id, choice));
                        }
                    }
                });
                if current == OnBreak::Ignore {
                    limits_nothing(ui, indent, &set);
                }
            }
        }
        _ => {}
    }
    edit
}

/// The warning under a set whose options leave it limiting nothing.
fn limits_nothing(ui: &mut egui::Ui, indent: f32, set: &str) {
    ui.horizontal_wrapped(|ui| {
        ui.add_space(indent);
        marks::mark(ui, ItemState::Warning, "Warning");
        ui.label(RichText::new(format!("The set '{set}' limits nothing.")).color(DARK.warning));
    });
}

/// The Geometry section (section 8): Cut on, Contain within, Avoid and Check,
/// the roles of decision record 0032 item 5, each with its sets, a list to
/// add from and a button to remove. A set with a problem has a red border.
/// With `with_options`, each set's options under its role (decision record
/// 0176 item 3). Under the role that holds a set reaching a held region or a
/// tab, the warning of 11.7 with "Show in view" and "Edit workholding".
fn geometry(
    ui: &mut egui::Ui,
    operation: &openchuck_app::project::Operation,
    summaries: &[SelectionSummary],
    reach: &[String],
    with_options: bool,
) -> (Option<GeometryEdit>, Option<ReachAction>) {
    let mut edit = None;
    let mut action = None;
    if summaries.is_empty() {
        ui.label(
            RichText::new("No selection sets yet. Make some with the Selection sets \"+\".")
                .color(DARK.muted),
        );
        return (None, None);
    }
    ui.label(
        RichText::new("With nothing in Cut on, the operation cuts the whole part.")
            .color(DARK.muted),
    );
    for role in SelectionRole::ALL {
        let sets = members(operation, role);
        ui.horizontal_wrapped(|ui| {
            let (rect, _) = ui.allocate_exact_size(
                egui::vec2(
                    crate::theme::width::LABEL_COLUMN,
                    ui.spacing().interact_size.y,
                ),
                egui::Sense::hover(),
            );
            let label = ui.put(
                rect,
                egui::Label::new(RichText::new(role.name()).color(DARK.muted)),
            );
            label.on_hover_text(role.description());
            for id in &sets {
                let summary = summaries.iter().find(|s| s.id == *id);
                let name = summary.map_or("(missing set)", |s| s.name.as_str());
                let problem = summary.and_then(|s| s.problem.clone());
                let text = match (&problem, summary) {
                    (Some(_), Some(s)) => {
                        format!("{name} \u{b7} {} {} not found \u{d7}", s.count, s.kind)
                    }
                    _ => format!("{name} \u{d7}"),
                };
                let mut button =
                    egui::Button::new(RichText::new(text).color(if problem.is_some() {
                        DARK.error
                    } else {
                        DARK.text
                    }))
                    .small();
                if problem.is_some() {
                    button = button.stroke(Stroke::new(1.0, DARK.error));
                }
                let response = ui.add(button);
                let tip = problem.map_or_else(
                    || "Take it out of this role".to_owned(),
                    |p| {
                        format!(
                            "{}\nClick to take it out of this role.",
                            form_panel::refusal(&p).replace(" Not applied.", "")
                        )
                    },
                );
                if field_row::tip(response, &tip).clicked() {
                    edit = Some(GeometryEdit::Role(role, *id, false));
                }
            }
            let free: Vec<&SelectionSummary> =
                summaries.iter().filter(|s| !sets.contains(&s.id)).collect();
            if !free.is_empty() {
                egui::ComboBox::from_id_salt(("role", role.name()))
                    .selected_text("add…")
                    .show_ui(ui, |ui| {
                        for s in free {
                            if ui.selectable_label(false, &s.name).clicked() {
                                edit = Some(GeometryEdit::Role(role, s.id, true));
                            }
                        }
                    });
            }
        });
        // The sets' options, for a strategy that takes them (decision
        // record 0176).
        if with_options && let Some(e) = set_options(ui, operation, role, &sets, summaries) {
            edit = Some(e);
        }
        // The selection warning under the role that holds the set (11.7).
        for id in &sets {
            let Some(s) = summaries.iter().find(|s| s.id == *id) else {
                continue;
            };
            let quoted = format!("'{}'", s.name);
            for note in reach.iter().filter(|n| n.contains(&quoted)) {
                ui.horizontal_wrapped(|ui| {
                    ui.add_space(crate::theme::width::LABEL_COLUMN + ui.spacing().item_spacing.x);
                    marks::mark(ui, ItemState::Warning, "Warning");
                    ui.label(RichText::new(note).color(DARK.warning));
                    if ui.link("Show in view").clicked() {
                        action = Some(ReachAction::Show(
                            *id,
                            crate::workholding_panel::reached_item(note),
                        ));
                    }
                    if ui.link("Edit workholding").clicked() {
                        action = Some(ReachAction::EditWorkholding);
                    }
                });
            }
        }
    }
    (edit, action)
}

/// The key of a setup row in the form state.
fn setup_key(row: &SetupRow) -> String {
    format!("\u{0}setup {:?}", row.field)
}

/// Draws setup rows. Returns the edited setup and the key of the row it came
/// from, when a value was committed and passed its checks; a refused value
/// stays in its field with the reason (section 8).
fn rows(
    ui: &mut egui::Ui,
    state: &mut field_row::FieldState,
    setup: &Setup,
    machine: Option<&openchuck_app::setup::MachineProfile>,
    rows: &[SetupRow],
) -> Option<(Setup, Option<String>)> {
    let mut edited = None;
    for row in rows {
        let key = setup_key(row);
        let outcome = field_row::text_row(
            ui,
            state,
            &TextRow {
                key: &key,
                label: row.label,
                description: row.description,
                unit: row.unit,
                range: row.range,
                applied: row.value.clone(),
                hint: row.hint.clone(),
                problem: None,
                reset: false,
                enabled: true,
                focus: false,
                warned: None,
                needs: None,
                scroll_to: false,
            },
        );
        if let Outcome::Commit(typed) = outcome {
            match setup_form::apply(setup, machine, row.field, &typed) {
                Ok(s) => edited = Some((s, Some(key))),
                Err(reason) => state.refuse(&key, &typed, format!("{reason} Not applied.")),
            }
        }
    }
    edited
}

/// The Stock section (SET-03; 11.3): the shape as a segmented control, its
/// sizes, and the material.
fn stock_section(
    ui: &mut egui::Ui,
    state: &mut field_row::FieldState,
    setup: &Setup,
    machine: Option<&openchuck_app::setup::MachineProfile>,
    part: Option<openchuck_app::setup::PartExtent>,
) -> Option<(Setup, Option<String>)> {
    let shapes: Vec<&str> = StockShape::ALL.iter().map(|s| s.name()).collect();
    let current = StockShape::of(setup.stock.as_ref())
        .and_then(|s| StockShape::ALL.iter().position(|a| *a == s));
    let imported = setup.stock.is_some() && current.is_none();
    if imported {
        field_row::read_only_row(ui, "Shape", "", "Imported");
    } else if setup.stock.is_none() {
        ui.label(RichText::new("No stock yet: choose its shape.").color(DARK.warning));
    }
    let mut edited = None;
    if !imported
        && let Some(i) = field_row::choice_row(
            ui,
            "stock shape",
            "Shape",
            "A cylinder on the rotary axis, a box, or the model's bounding box plus an offset.",
            &shapes,
            current,
            true,
        )
        && let Some(shape) = StockShape::ALL.get(i)
    {
        edited = Some((
            setup_form::with_stock_shape(setup, *shape, part.as_ref()),
            None,
        ));
    }
    rows(ui, state, setup, machine, &setup_form::stock_rows(setup)).or(edited)
}

/// The Work offset and clearance section (SET-05, SET-06; 11.3): the
/// register, the zero position (read-only, decision 0011 item 1), the zero's
/// X, the safe radius and the safe Z.
fn clearance_section(
    ui: &mut egui::Ui,
    state: &mut field_row::FieldState,
    setup: &Setup,
    machine: Option<&openchuck_app::setup::MachineProfile>,
) -> Option<(Setup, Option<String>)> {
    let (names, current) = setup_form::registers(setup);
    let mut edited = field_row::choice_row(
        ui,
        "work offset",
        "Work offset",
        "The controller's work offset register the program uses.",
        &names,
        Some(current),
        true,
    )
    .map(|i| (setup_form::with_register(setup, i), None));
    field_row::read_only_row(
        ui,
        "Zero position",
        "The work zero lies on the rotary axis.",
        "On the A axis (Y0 Z0)",
    );
    if let Some(e) = rows(
        ui,
        state,
        setup,
        machine,
        &setup_form::clearance_rows(setup, machine),
    ) {
        edited = Some(e);
    }
    edited
}

impl Shell {
    /// Renames `item` as one undoable command, or says why not.
    pub(super) fn rename_checked(&mut self, item: ItemRef, name: &str) -> Result<(), String> {
        let Some(app) = &mut self.app else {
            return Ok(());
        };
        let name = name.trim();
        if name.is_empty() {
            return Err("A name cannot be empty. Not applied.".to_owned());
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
        match replaced {
            Some(item) => app
                .execute(Command::Replace(item))
                .map_err(|e| form_panel::refusal(&e.to_string())),
            // A model is named by its file, and a setup's part by the setup.
            None => Ok(()),
        }
    }
}
