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
use openchuck_app::project::{Item, Setup};
use openchuck_app::selection::{SelectionRole, SelectionSummary};
use openchuck_app::setup_form::{self, SetupRow, StockShape};
use openchuck_app::tool_edit;
use openchuck_app::tools;

use super::{Dialog, Selection, Shell, TreeAction, frame, marks};
use crate::field_row::{self, Outcome, TextRow};
use crate::form_panel;
use crate::machine_panel::MachinePanel;
use crate::model_panel::ModelPanel;
use crate::theme::DARK;
use crate::tool_panel;
use crate::workholding_panel::WorkholdingPanel;

/// The key of the header's name in the form state.
const NAME: &str = "\u{0}name";

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
        let Some(app) = &mut self.app else { return };
        if self.results.show(ui, app, run.as_ref()) {
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
        let focus = self.setup_section.take();
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
                            egui::Frame::new()
                                .stroke(Stroke::new(1.0, DARK.line))
                                .corner_radius(3.0)
                                .inner_margin(egui::Margin::symmetric(5, 0))
                                .show(ui, |ui| {
                                    ui.label(RichText::new(chip).small().color(*colour))
                                });
                        }
                    });
                },
                |ui| match part {
                    SetupPart::Machine => {
                        let panel = self.machine_panel.get_or_insert_with(MachinePanel::default);
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
                        if let Some(e) =
                            stock_section(ui, &mut self.form, &setup, copy.as_ref(), part_extent)
                        {
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

    /// A tool's dimensions and its profile editor (UI-07, TL-03).
    fn tool_properties(&mut self, ui: &mut egui::Ui, id: ToolId) {
        let Some(app) = &self.app else { return };
        let Some(snapshot) = app.project().tools().iter().find(|t| t.id == id).cloned() else {
            self.selected = None;
            return;
        };
        self.header(ui, ItemRef::Tool(id), &snapshot.name, true);
        ui.label(RichText::new(tools::describe(&snapshot.tool)).color(DARK.muted));
        ui.separator();
        let fields = tool_edit::tool_form(&snapshot.tool);
        let edit = form_panel::show_fields(ui, &fields, &[], false, &mut self.form);
        ui.separator();
        ui.strong("Profile");
        let mut action = tool_panel::ProfileAction::None;
        match &mut self.profile_draft {
            Some((tool, draft)) if *tool == id => {
                action = tool_panel::editor(ui, &snapshot.tool, draft);
            }
            _ => {
                let label = if matches!(snapshot.tool.shape, tools::ToolShape::Custom { .. }) {
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
        let Some(app) = &mut self.app else { return };
        if let Some(edit) = edit {
            // A tool dimension cannot be cleared: say so rather than leave the
            // box looking empty while the tool keeps its value.
            let result = match &edit.value {
                Some(value) => app
                    .set_tool_field(id, &edit.name, value)
                    .map_err(|e| form_panel::refusal(&e.to_string())),
                None => Err("A tool dimension needs a value. Not applied.".to_owned()),
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
            |ui| geometry(ui, &operation, &summaries, &reach),
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

        let Some(app) = &mut self.app else { return };
        let edit = match app.parameter_form(id) {
            Ok(form) => {
                let outcome = form_panel::show(ui, &form, &mut self.form, &mut self.settings);
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
        if let Some(edit) = edit {
            let result = app
                .set_parameter(id, &edit.name, edit.value)
                .map_err(|e| form_panel::refusal(&e.user_message()));
            self.form.result(&edit.name, result);
        }
        if let Some((role, set, add)) = role_edit
            && let Err(e) = app.set_operation_selection(id, role, set, add)
        {
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

/// A role change the Geometry section asks for: the role, the set, and
/// whether to add it.
type RoleEdit = (SelectionRole, SelectionSetId, bool);

/// The Geometry section (section 8): Cut on, Contain within, Avoid and Check,
/// the roles of decision record 0032 item 5, each with its sets, a list to
/// add from and a button to remove. A set with a problem has a red border.
/// Under the role that holds a set reaching a held region or a tab, the
/// warning of 11.7 with "Show in view" and "Edit workholding".
fn geometry(
    ui: &mut egui::Ui,
    operation: &openchuck_app::project::Operation,
    summaries: &[SelectionSummary],
    reach: &[String],
) -> (Option<RoleEdit>, Option<ReachAction>) {
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
                        format!("{name} \u{b7} {} {} not found \u{2715}", s.count, s.kind)
                    }
                    _ => format!("{name} \u{2715}"),
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
                    edit = Some((role, *id, false));
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
                                edit = Some((role, s.id, true));
                            }
                        }
                    });
            }
        });
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
