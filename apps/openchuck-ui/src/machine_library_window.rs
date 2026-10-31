// SPDX-License-Identifier: AGPL-3.0-or-later
//! The machine library window (SET-10, UI-13, NFR-07; specification 12.5;
//! lane MACHINES, decision record 0380; issue #335).
//!
//! Left, the machines: the project's copy ("This project"), the built-in
//! profiles (read-only, with Copy to edit) and the library's profiles, with
//! New machine, Duplicate and Delete. Centre, the form of every value in its
//! groups, each group with its provenance icon (UI-13) and "Mark
//! confirmed…". The footer has Import and Export, and Close.
//!
//! A library profile is edited as a draft: nothing is written until Save
//! (decision 0127's pattern). The project's copy is the project's own: each
//! change is one undoable command that keeps its source ID, and "Save to
//! library…" writes it onto its source or as a new profile. Built-in
//! profiles are never written. All the rules live in
//! `openchuck_app::machine_edit` and `machine_library`; this module only
//! draws and asks.

use std::path::{Path, PathBuf};

use eframe::egui::{self, RichText};
use openchuck_app::form::FieldKind;
use openchuck_app::machine_edit::{self, MachineField, confirm_value, set_machine_field};
use openchuck_app::machine_library::{
    self as lib, ImportChoice, ImportPlan, ImportStatus, MachineDraft,
};
use openchuck_app::machine_view::{MachineValue, tooltip};
use openchuck_app::project::ParameterValue;
use openchuck_app::setup::{AvailableMachine, MachineProfile, ProfileField, available_machines};
use openchuck_app::tools::ToolLibrary;
use openchuck_app::{Application, Command};

use crate::field_row::{self, FieldState, Outcome, TextRow};
use crate::floating::{WindowEvent, floating_unscrolled};
use crate::form_panel;
use crate::theme::DARK;

/// Which machine the form shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selected {
    /// The project's own copy.
    Project,
    /// A built-in profile, by its source text (`built-in makera-z1`).
    BuiltIn(String),
    /// A library profile file, or a draft not saved yet (`None`).
    Library(Option<PathBuf>),
}

/// Something the user did, carried out after the frame is drawn.
#[derive(Debug, Clone, PartialEq)]
enum Act {
    Choose(Selected),
    New,
    Duplicate,
    Set(&'static str, Option<ParameterValue>),
    AskConfirmValue(ProfileField),
    ConfirmValue,
    Save,
    SaveConfirmed,
    CancelEdits,
    AskDelete,
    Delete,
    Import(PathBuf),
    ApplyImport(ImportChoice),
    Export(PathBuf),
    AskSaveToLibrary,
    SaveCopyToSource,
    SaveCopyAsNew,
    Discard,
    KeepEditing,
    Dismiss,
    Close,
}

impl Act {
    /// Whether the act leaves the draft, so a draft with changes asks first.
    fn leaves(&self) -> bool {
        matches!(
            self,
            Self::Choose(_) | Self::New | Self::Duplicate | Self::Close
        )
    }
}

/// A question in front of the window.
#[derive(Debug, Clone, PartialEq)]
enum Ask {
    /// Leave a draft with changes, then do this.
    Discard(Box<Act>),
    /// Delete this library file: the question.
    Delete(PathBuf, String),
    /// What an import would do.
    Import(Box<ImportPlan>),
    /// Save a renamed draft whose tool set names the old name: the question
    /// and whether "Also rename the tool set" is ticked.
    SaveRename(String, bool),
    /// Mark a value confirmed: how it was confirmed, as typed.
    ConfirmValue(ProfileField, String),
    /// Save the project's copy to the library: its source file (if it is a
    /// library profile) and the values that would change there.
    SaveToLibrary(Option<PathBuf>, Vec<&'static str>),
}

/// The window's state.
#[derive(Debug, Default)]
pub struct MachineLibraryWindow {
    open: bool,
    /// The machine library folder; the default folder when `None`.
    folder: Option<PathBuf>,
    /// The tool library folder, for tool sets (TL-11); the default when
    /// `None`.
    tools_folder: Option<PathBuf>,
    available: Vec<AvailableMachine>,
    selected: Option<Selected>,
    draft: Option<MachineDraft>,
    fields: FieldState,
    ask: Option<Ask>,
    /// The last thing done, or why it was not (and whether it is a problem).
    message: Option<(String, bool)>,
}

impl MachineLibraryWindow {
    /// A closed window on the library in `folder` and the tool library in
    /// `tools_folder`, or the user's default folders when `None`.
    pub fn new(folder: Option<PathBuf>, tools_folder: Option<PathBuf>) -> Self {
        Self {
            folder,
            tools_folder,
            ..Self::default()
        }
    }

    /// Whether the window is open.
    pub fn is_open(&self) -> bool {
        self.open
    }

    fn folder(&self) -> Option<PathBuf> {
        self.folder
            .clone()
            .or_else(|| openchuck_app::library::default_folders().map(|f| f.machines))
    }

    fn tools(&self) -> Option<ToolLibrary> {
        let folder = self
            .tools_folder
            .clone()
            .or_else(|| openchuck_app::library::default_folders().map(|f| f.tools))?;
        ToolLibrary::open(&folder).ok()
    }

    /// Opens the window, reading the library again, on the project's copy
    /// when `project` is true. A draft with changes is kept.
    pub fn open(&mut self, project: bool) {
        if !self.open || !self.is_dirty() {
            self.reload();
            if project {
                self.choose(Selected::Project);
            }
        }
        self.open = true;
    }

    /// Reads the library and the built-in profiles again.
    pub fn reload(&mut self) {
        self.available = available_machines(self.folder().as_deref());
    }

    fn is_dirty(&self) -> bool {
        self.draft.as_ref().is_some_and(MachineDraft::is_dirty)
    }

    fn choose(&mut self, selected: Selected) {
        self.fields.clear();
        self.draft = match &selected {
            Selected::Library(Some(path)) => match MachineDraft::open(path) {
                Ok(d) => Some(d),
                Err(e) => {
                    self.message = Some((e.to_string(), true));
                    None
                }
            },
            _ => None,
        };
        self.selected = Some(selected);
    }

    fn forget(&mut self) {
        self.selected = None;
        self.draft = None;
        self.fields.clear();
    }

    /// The profile the form shows, and its ID.
    fn shown(&self, app: Option<&Application>) -> Option<(MachineProfile, Option<String>)> {
        match self.selected.as_ref()? {
            Selected::Project => {
                let project = app?.project();
                Some((
                    project.machine()?.clone(),
                    project.machine_id().map(str::to_owned),
                ))
            }
            Selected::BuiltIn(source) => self
                .available
                .iter()
                .find(|a| &a.source == source)
                .and_then(|a| Some((a.profile.as_ref().ok()?.clone(), a.id.clone()))),
            Selected::Library(_) => self
                .draft
                .as_ref()
                .map(|d| (d.profile.clone(), d.id.clone())),
        }
    }

    /// Draws the window when it is open and does what the user asked.
    /// Returns a line for the status bar.
    pub fn frame(
        &mut self,
        ctx: &egui::Context,
        mut app: Option<&mut Application>,
    ) -> Option<String> {
        if !self.open {
            return None;
        }
        if self.selected == Some(Selected::Project)
            && app
                .as_deref()
                .is_none_or(|a| a.project().machine().is_none())
        {
            self.forget();
        }
        let mut acts = Vec::new();
        let shown = self.shown(app.as_deref());
        let has_project = app
            .as_deref()
            .is_some_and(|a| a.project().machine().is_some());
        let (_, event) = floating_unscrolled(
            ctx,
            "machine library",
            "Machine library",
            [1000.0, 720.0],
            |ui| {
                self.draw(ui, shown.as_ref(), has_project, &mut acts);
            },
        );
        if event == WindowEvent::CloseRequested {
            acts.push(Act::Close);
        }
        let mut status = None;
        for act in acts {
            if let Some(s) = self.act(act, app.as_deref_mut()) {
                status = Some(s);
            }
        }
        status
    }

    fn draw(
        &mut self,
        ui: &mut egui::Ui,
        shown: Option<&(MachineProfile, Option<String>)>,
        has_project: bool,
        acts: &mut Vec<Act>,
    ) {
        egui::Panel::bottom("machine library footer")
            .resizable(false)
            .show(ui, |ui| self.footer(ui, shown, acts));
        egui::Panel::left("machine library list")
            .default_size(260.0)
            .resizable(true)
            .show(ui, |ui| self.list(ui, has_project, acts));
        egui::CentralPanel::default().show(ui, |ui| {
            crate::theme::solid_scroll_bar(ui);
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| match shown {
                    Some((profile, _)) => self.form(ui, profile, acts),
                    None => {
                        ui.label(
                            RichText::new(
                                "Choose a machine in the list, or make one with New machine.",
                            )
                            .color(DARK.muted),
                        );
                    }
                });
        });
        self.dialogs(ui, acts);
    }

    fn list(&self, ui: &mut egui::Ui, has_project: bool, acts: &mut Vec<Act>) {
        ui.horizontal(|ui| {
            if ui.button("New machine").clicked() {
                acts.push(Act::New);
            }
            let can_copy = !matches!(self.selected, None | Some(Selected::Library(None)));
            if ui
                .add_enabled(can_copy, egui::Button::new("Duplicate"))
                .clicked()
            {
                acts.push(Act::Duplicate);
            }
            let can_delete = matches!(self.selected, Some(Selected::Library(Some(_))));
            if ui
                .add_enabled(can_delete, egui::Button::new("Delete…"))
                .clicked()
            {
                acts.push(Act::AskDelete);
            }
        });
        ui.separator();
        let row = |ui: &mut egui::Ui, text: &str, selected: Selected, acts: &mut Vec<Act>| {
            let on = self.selected.as_ref() == Some(&selected);
            if ui.selectable_label(on, text).clicked() && !on {
                acts.push(Act::Choose(selected));
            }
        };
        heading(ui, "THIS PROJECT");
        if has_project {
            row(ui, "The project's machine", Selected::Project, acts);
        } else {
            ui.label(RichText::new("No machine is chosen.").color(DARK.muted));
        }
        heading(ui, "BUILT IN");
        for a in self
            .available
            .iter()
            .filter(|a| a.source.starts_with("built-in "))
        {
            let name = a
                .profile
                .as_ref()
                .map_or("(cannot be read)", |p| p.name.as_str());
            row(ui, name, Selected::BuiltIn(a.source.clone()), acts);
        }
        heading(ui, "LIBRARY");
        if let Some(Selected::Library(None)) = &self.selected
            && let Some(d) = &self.draft
        {
            let _ = ui.selectable_label(true, format!("{} (new)", d.profile.name));
        }
        let mut any = false;
        for a in self
            .available
            .iter()
            .filter(|a| !a.source.starts_with("built-in "))
        {
            any = true;
            let path = PathBuf::from(&a.source);
            match &a.profile {
                Ok(p) => row(ui, &p.name, Selected::Library(Some(path)), acts),
                Err(e) => {
                    let file = path
                        .file_name()
                        .map_or_else(|| a.source.clone(), |f| f.to_string_lossy().into_owned());
                    field_row::tip(
                        ui.label(
                            RichText::new(format!("{file}: cannot be read")).color(DARK.error),
                        ),
                        e,
                    );
                }
            }
        }
        if !any {
            ui.label(RichText::new("No machines in the library yet.").color(DARK.muted));
        }
        if let Some(folder) = self.folder() {
            ui.add_space(8.0);
            ui.label(
                RichText::new(format!("Library: {}", folder.display()))
                    .color(DARK.muted)
                    .small(),
            );
        }
    }

    fn footer(
        &self,
        ui: &mut egui::Ui,
        shown: Option<&(MachineProfile, Option<String>)>,
        acts: &mut Vec<Act>,
    ) {
        ui.add_space(6.0);
        if let Some((text, problem)) = &self.message {
            let colour = if *problem { DARK.error } else { DARK.muted };
            field_row::note_line(ui, text, colour);
        }
        ui.horizontal(|ui| {
            if ui.button("Import…").clicked()
                && let Some(file) = rfd::FileDialog::new()
                    .set_title("Import a machine profile")
                    .add_filter("Machine profile", &["toml"])
                    .pick_file()
            {
                acts.push(Act::Import(file));
            }
            let can_export =
                shown.is_some() && self.draft.as_ref().is_none_or(|d| d.blanks.is_empty());
            if ui
                .add_enabled(can_export, egui::Button::new("Export…"))
                .clicked()
                && let Some((profile, _)) = shown
                && let Some(file) = rfd::FileDialog::new()
                    .set_title("Export the machine profile")
                    .add_filter("Machine profile", &["toml"])
                    .set_file_name(format!(
                        "{}.toml",
                        openchuck_app::machine_library::file_for(Path::new(""), &profile.name)
                            .file_stem()
                            .map_or_else(String::new, |s| s.to_string_lossy().into_owned())
                    ))
                    .save_file()
            {
                acts.push(Act::Export(file));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Close").clicked() {
                    acts.push(Act::Close);
                }
            });
        });
    }

    fn form(&mut self, ui: &mut egui::Ui, profile: &MachineProfile, acts: &mut Vec<Act>) {
        let read_only = matches!(self.selected, Some(Selected::BuiltIn(_)));
        ui.horizontal(|ui| {
            heading(ui, &profile.name);
            match &self.selected {
                Some(Selected::Project) => {
                    ui.label(
                        RichText::new("The project's copy: each change can be undone")
                            .color(DARK.muted),
                    );
                    if ui.button("Save to library…").clicked() {
                        acts.push(Act::AskSaveToLibrary);
                    }
                }
                Some(Selected::BuiltIn(_)) => {
                    ui.label(RichText::new("Built in: it cannot be edited").color(DARK.muted));
                    if crate::shell::primary_button(ui, "Copy to edit", true).clicked() {
                        acts.push(Act::Duplicate);
                    }
                }
                Some(Selected::Library(_)) => {
                    if let Some(d) = &self.draft {
                        let dirty = d.is_dirty();
                        if crate::shell::primary_button(ui, "Save", dirty).clicked() {
                            acts.push(Act::Save);
                        }
                        if ui.add_enabled(dirty, egui::Button::new("Cancel")).clicked() {
                            acts.push(Act::CancelEdits);
                        }
                        let colour = if dirty { DARK.warning } else { DARK.muted };
                        ui.label(RichText::new(d.status().text()).color(colour));
                    }
                }
                None => {}
            }
        });
        let form = match &self.draft {
            Some(d) if matches!(self.selected, Some(Selected::Library(_))) => d.form(),
            _ => machine_edit::machine_form(profile),
        };
        let mut group = None;
        for field in &form {
            if group != Some(field.group) {
                group = Some(field.group);
                ui.add_space(6.0);
                heading(ui, &field.group.label().to_uppercase());
            }
            if field.applies {
                self.field(ui, profile, field, read_only, acts);
            }
            // A provenance entry shows once, after the last value it covers.
            if let Some(p) = field.provenance
                && last_of(&form, p) == Some(field.name)
            {
                provenance_row(ui, profile, p, read_only, acts);
            }
        }
    }

    fn field(
        &mut self,
        ui: &mut egui::Ui,
        profile: &MachineProfile,
        field: &MachineField,
        read_only: bool,
        acts: &mut Vec<Act>,
    ) {
        let name = field.name;
        let draft_blank = self.draft.as_ref().is_some_and(|d| d.blanks.contains(name))
            && matches!(self.selected, Some(Selected::Library(_)));
        match &field.kind {
            FieldKind::Choice { options } => {
                let labels: Vec<&str> = options.iter().map(|o| choice_label(o)).collect();
                let current = match &field.value {
                    Some(ParameterValue::Text(t)) => options.iter().position(|o| o == t),
                    _ => None,
                };
                if let Some(i) = field_row::choice_row(
                    ui,
                    name,
                    field.label,
                    field.description,
                    &labels,
                    current,
                    !read_only,
                ) && let Some(o) = options.get(i)
                {
                    acts.push(Act::Set(name, Some(ParameterValue::Text(o.clone()))));
                }
            }
            FieldKind::Flag => {
                let on = matches!(field.value, Some(ParameterValue::Flag(true)));
                if let Some(v) =
                    field_row::flag_row(ui, field.label, field.description, on, !read_only)
                {
                    acts.push(Act::Set(name, Some(ParameterValue::Flag(v))));
                }
            }
            kind => {
                let unit = match kind {
                    FieldKind::Number { unit, .. } => *unit,
                    _ => "",
                };
                let validate = |typed: &str| -> Result<(), String> {
                    let value = parse(kind, typed)?;
                    set_machine_field(profile, name, value.as_ref())
                        .map(drop)
                        .map_err(|e| e.message)
                };
                let row = TextRow {
                    key: name,
                    label: field.label,
                    description: field.description,
                    unit,
                    applied: field
                        .value
                        .as_ref()
                        .map_or_else(String::new, |v| form_panel::text(v, unit)),
                    enabled: !read_only,
                    number: !matches!(kind, FieldKind::Text),
                    needs: draft_blank.then_some("Fill in before saving"),
                    validate: Some(&validate),
                    ..TextRow::default()
                };
                if let Outcome::Commit(typed) = field_row::text_row(ui, &mut self.fields, &row) {
                    match parse(kind, &typed) {
                        Ok(value) => acts.push(Act::Set(name, value)),
                        Err(reason) => self.fields.refuse(name, &typed, reason),
                    }
                }
            }
        }
        if let Some(reason) = self.fields.error(name) {
            field_row::note_line(ui, reason, DARK.error);
        }
    }

    fn dialogs(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Act>) {
        let Some(ask) = &mut self.ask else {
            return;
        };
        egui::Modal::new(egui::Id::unique("machine library question")).show(ui.ctx(), |ui| {
            ui.set_max_width(460.0);
            match ask {
                Ask::Discard(_) => {
                    ui.label("Discard the changes to this machine?");
                    buttons(
                        ui,
                        acts,
                        ("Discard", Act::Discard),
                        ("Keep editing", Act::KeepEditing),
                    );
                }
                Ask::Delete(_, question) => {
                    ui.label(question.as_str());
                    buttons(ui, acts, ("Delete", Act::Delete), ("Cancel", Act::Dismiss));
                }
                Ask::Import(plan) => {
                    import_question(ui, plan, acts);
                }
                Ask::SaveRename(question, rename) => {
                    ui.label(question.as_str());
                    ui.checkbox(rename, "Also rename the tool set");
                    buttons(
                        ui,
                        acts,
                        ("Save", Act::SaveConfirmed),
                        ("Cancel", Act::Dismiss),
                    );
                }
                Ask::ConfirmValue(field, how) => {
                    ui.label(format!(
                        "Mark the {} confirmed on the machine. Say how it was confirmed, for \
                         example what was measured and when:",
                        openchuck_app::setup::profile_field_label(*field)
                    ));
                    ui.add(egui::TextEdit::multiline(how).desired_rows(3));
                    let ok = !how.trim().is_empty();
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(ok, egui::Button::new("Mark confirmed"))
                            .clicked()
                        {
                            acts.push(Act::ConfirmValue);
                        }
                        if ui.button("Cancel").clicked() {
                            acts.push(Act::Dismiss);
                        }
                    });
                }
                Ask::SaveToLibrary(source, changes) => match source {
                    Some(path) => {
                        let what = if changes.is_empty() {
                            "It is the same as the library profile it came from.".to_owned()
                        } else {
                            format!("It changes: {}.", changes.join(", "))
                        };
                        ui.label(format!(
                            "Write the project's copy over the library profile it came \
                                 from ({})? {what}",
                            path.display()
                        ));
                        ui.horizontal(|ui| {
                            if ui.button("Write over it").clicked() {
                                acts.push(Act::SaveCopyToSource);
                            }
                            if ui.button("Save as a new profile").clicked() {
                                acts.push(Act::SaveCopyAsNew);
                            }
                            if ui.button("Cancel").clicked() {
                                acts.push(Act::Dismiss);
                            }
                        });
                    }
                    None => {
                        ui.label(
                            "Save the project's copy as a new profile in the library? It \
                                 gets its own reference, and the project's copy then compares \
                                 with it.",
                        );
                        buttons(
                            ui,
                            acts,
                            ("Save as a new profile", Act::SaveCopyAsNew),
                            ("Cancel", Act::Dismiss),
                        );
                    }
                },
            }
        });
    }

    /// Carries out `act`. Returns a line for the status bar.
    fn act(&mut self, act: Act, app: Option<&mut Application>) -> Option<String> {
        if act.leaves() && self.is_dirty() && self.ask.is_none() {
            self.ask = Some(Ask::Discard(Box::new(act)));
            return None;
        }
        let folder = self.folder();
        let result: Result<Option<String>, String> = match act {
            Act::Choose(selected) => {
                self.choose(selected);
                Ok(None)
            }
            Act::New => {
                self.fields.clear();
                self.draft = Some(MachineDraft::new_machine());
                self.selected = Some(Selected::Library(None));
                Ok(None)
            }
            Act::Duplicate => {
                let shown = self.shown(app.as_deref());
                match (shown, &folder) {
                    (Some((profile, _)), Some(dir)) => {
                        self.fields.clear();
                        self.draft = Some(MachineDraft::copy_of(dir, &profile));
                        self.selected = Some(Selected::Library(None));
                        Ok(None)
                    }
                    _ => Err(no_folder()),
                }
            }
            Act::Set(name, value) => self.set(name, value, app),
            Act::AskConfirmValue(field) => {
                self.ask = Some(Ask::ConfirmValue(field, String::new()));
                Ok(None)
            }
            Act::ConfirmValue => match self.ask.take() {
                Some(Ask::ConfirmValue(field, how)) => self.confirm(field, &how, app),
                other => {
                    self.ask = other;
                    Ok(None)
                }
            },
            Act::Save => self.ask_save(),
            Act::SaveConfirmed => {
                let rename = matches!(self.ask, Some(Ask::SaveRename(_, true)));
                self.ask = None;
                self.write_draft(rename)
            }
            Act::CancelEdits => {
                match &self.selected {
                    Some(Selected::Library(Some(path))) => {
                        let path = path.clone();
                        self.choose(Selected::Library(Some(path)));
                    }
                    _ => self.forget(),
                }
                Ok(None)
            }
            Act::AskDelete => match &self.selected {
                Some(Selected::Library(Some(path))) => {
                    let name = self.draft.as_ref().map_or_else(String::new, |d| {
                        d.saved
                            .as_ref()
                            .map_or(d.profile.name.clone(), |s| s.name.clone())
                    });
                    let used = app.as_deref().is_some_and(|a| {
                        a.project().machine_id().is_some()
                            && a.project().machine_id()
                                == self.draft.as_ref().and_then(|d| d.id.as_deref())
                    });
                    self.ask = Some(Ask::Delete(path.clone(), lib::delete_message(&name, used)));
                    Ok(None)
                }
                _ => Ok(None),
            },
            Act::Delete => match (self.ask.take(), &folder) {
                (Some(Ask::Delete(path, _)), Some(dir)) => lib::delete_profile(dir, &path)
                    .map(|()| {
                        self.forget();
                        self.reload();
                        Some(format!(
                            "Deleted {} from the machine library.",
                            path.display()
                        ))
                    })
                    .map_err(|e| e.to_string()),
                _ => Ok(None),
            },
            Act::Import(file) => match &folder {
                Some(dir) => lib::plan_import(dir, &file)
                    .map(|plan| {
                        self.ask = Some(Ask::Import(Box::new(plan)));
                        None
                    })
                    .map_err(|e| e.to_string()),
                None => Err(no_folder()),
            },
            Act::ApplyImport(choice) => match (self.ask.take(), &folder) {
                (Some(Ask::Import(plan)), Some(dir)) => lib::apply_import(dir, &plan, choice)
                    .map(|written| {
                        self.reload();
                        Some(match written {
                            Some(path) => format!("Imported into {}.", path.display()),
                            None => "Nothing was imported; the library is unchanged.".to_owned(),
                        })
                    })
                    .map_err(|e| e.to_string()),
                _ => Ok(None),
            },
            Act::Export(file) => match self.shown(app.as_deref()) {
                Some((profile, id)) => lib::export_profile(&profile, id.as_deref(), &file)
                    .map(|()| Some(format!("Exported to {}.", file.display())))
                    .map_err(|e| e.to_string()),
                None => Ok(None),
            },
            Act::AskSaveToLibrary => {
                let id = app
                    .as_deref()
                    .and_then(|a| a.project().machine_id().map(str::to_owned));
                let copy = app.as_deref().and_then(|a| a.project().machine().cloned());
                let source = id.and_then(|id| {
                    self.available
                        .iter()
                        .find(|a| {
                            a.id.as_deref() == Some(id.as_str())
                                && !a.source.starts_with("built-in ")
                        })
                        .map(|a| PathBuf::from(&a.source))
                });
                let changes = match (&source, &copy) {
                    (Some(path), Some(copy)) => lib::copy_changes(path, copy).unwrap_or_default(),
                    _ => Vec::new(),
                };
                self.ask = Some(Ask::SaveToLibrary(source, changes));
                Ok(None)
            }
            Act::SaveCopyToSource => {
                let source = match self.ask.take() {
                    Some(Ask::SaveToLibrary(Some(path), _)) => Some(path),
                    _ => None,
                };
                let copy = app.as_deref().and_then(|a| a.project().machine().cloned());
                match (source, copy, &folder) {
                    (Some(path), Some(copy), Some(dir)) => {
                        lib::save_copy_to_source(dir, &path, &copy)
                            .map(|p| {
                                self.reload();
                                Some(format!("Saved the project's machine to {}.", p.display()))
                            })
                            .map_err(|e| e.to_string())
                    }
                    _ => Ok(None),
                }
            }
            Act::SaveCopyAsNew => {
                self.ask = None;
                match (app, &folder) {
                    (Some(app), Some(dir)) => match app.project().machine().cloned() {
                        Some(copy) => match lib::save_copy_as_new(dir, &copy) {
                            Ok((path, id)) => {
                                self.reload();
                                // The copy takes the saved profile, whose
                                // name may have gained " copy", and records
                                // it as its source (OI-1473).
                                let saved = MachineDraft::open(&path).map_or(copy, |d| d.profile);
                                app.execute(Command::SetMachineFrom {
                                    machine: Some(Box::new(saved)),
                                    id: Some(id),
                                })
                                .map(|()| {
                                    Some(format!(
                                        "Saved the project's machine as a new profile, {}.",
                                        path.display()
                                    ))
                                })
                                .map_err(|e| e.to_string())
                            }
                            Err(e) => Err(e.to_string()),
                        },
                        None => Ok(None),
                    },
                    _ => Err(no_folder()),
                }
            }
            Act::Discard => match self.ask.take() {
                Some(Ask::Discard(next)) => {
                    self.draft = None;
                    return self.act(*next, app);
                }
                _ => Ok(None),
            },
            Act::KeepEditing | Act::Dismiss => {
                self.ask = None;
                Ok(None)
            }
            Act::Close => {
                self.forget();
                self.ask = None;
                self.open = false;
                Ok(None)
            }
        };
        match result {
            Ok(status) => {
                if let Some(s) = &status {
                    self.message = Some((s.clone(), false));
                }
                status
            }
            Err(e) => {
                self.message = Some((e, true));
                None
            }
        }
    }

    fn set(
        &mut self,
        name: &'static str,
        value: Option<ParameterValue>,
        app: Option<&mut Application>,
    ) -> Result<Option<String>, String> {
        match &self.selected {
            Some(Selected::Library(_)) => {
                let Some(draft) = &mut self.draft else {
                    return Ok(None);
                };
                let result = draft.set(name, value.as_ref()).map_err(|e| e.message);
                self.fields.result(name, result.clone());
                result.map(|()| None)
            }
            Some(Selected::Project) => {
                let Some(app) = app else { return Ok(None) };
                let Some(copy) = app.project().machine().cloned() else {
                    return Ok(None);
                };
                let id = app.project().machine_id().map(str::to_owned);
                match set_machine_field(&copy, name, value.as_ref()) {
                    Ok(next) if next == copy => Ok(None),
                    Ok(next) => {
                        self.fields.result(name, Ok(()));
                        app.execute(Command::SetMachineFrom {
                            machine: Some(Box::new(next)),
                            id,
                        })
                        .map(|()| None)
                        .map_err(|e| e.to_string())
                    }
                    Err(e) => {
                        self.fields.result(name, Err(e.message.clone()));
                        Err(e.to_string())
                    }
                }
            }
            _ => Ok(None),
        }
    }

    fn confirm(
        &mut self,
        field: ProfileField,
        how: &str,
        app: Option<&mut Application>,
    ) -> Result<Option<String>, String> {
        match &self.selected {
            Some(Selected::Library(_)) => {
                let Some(draft) = &mut self.draft else {
                    return Ok(None);
                };
                draft.profile =
                    confirm_value(&draft.profile, field, how).map_err(|e| e.to_string())?;
                Ok(None)
            }
            Some(Selected::Project) => {
                let Some(app) = app else { return Ok(None) };
                let Some(copy) = app.project().machine().cloned() else {
                    return Ok(None);
                };
                let id = app.project().machine_id().map(str::to_owned);
                let next = confirm_value(&copy, field, how).map_err(|e| e.to_string())?;
                app.execute(Command::SetMachineFrom {
                    machine: Some(Box::new(next)),
                    id,
                })
                .map(|()| None)
                .map_err(|e| e.to_string())
            }
            _ => Ok(None),
        }
    }

    /// Save, asking first when a renamed machine's tool set would be left
    /// on the old name (OI-1474).
    fn ask_save(&mut self) -> Result<Option<String>, String> {
        let question = match (&self.draft, self.tools()) {
            (Some(d), Some(tools)) => lib::tool_set_rename_question(&tools, d),
            _ => None,
        };
        match question {
            Some(q) => {
                self.ask = Some(Ask::SaveRename(q, true));
                Ok(None)
            }
            None => self.write_draft(false),
        }
    }

    fn write_draft(&mut self, rename_set: bool) -> Result<Option<String>, String> {
        let folder = self.folder().ok_or_else(no_folder)?;
        let Some(draft) = &mut self.draft else {
            return Ok(None);
        };
        let old = draft.renamed_from().map(str::to_owned);
        let path = lib::save_draft(&folder, draft).map_err(|e| e.to_string())?;
        let new_name = draft.profile.name.clone();
        let mut status = format!("Saved {} in the machine library.", draft.profile.name);
        if rename_set
            && let Some(old) = old
            && let Some(mut tools) = self.tools()
        {
            match lib::rename_tool_set(&mut tools, &old, &new_name) {
                Ok(_) => status.push_str(" Its tool set was renamed too."),
                Err(e) => {
                    self.reload();
                    self.selected = Some(Selected::Library(Some(path)));
                    return Err(e.to_string());
                }
            }
        }
        self.reload();
        self.selected = Some(Selected::Library(Some(path)));
        Ok(Some(status))
    }
}

/// The name of the last form field that belongs to `p`.
fn last_of(form: &[MachineField], p: ProfileField) -> Option<&'static str> {
    form.iter()
        .rev()
        .find(|f| f.provenance == Some(p))
        .map(|f| f.name)
}

/// A group's provenance: the icon with its tooltip (UI-13), the state in
/// words, and "Mark confirmed…" where the user may confirm it.
fn provenance_row(
    ui: &mut egui::Ui,
    profile: &MachineProfile,
    field: ProfileField,
    read_only: bool,
    acts: &mut Vec<Act>,
) {
    if field == ProfileField::ShortBlockMode {
        return;
    }
    let confirmed = profile.is_confirmed(field);
    let value = MachineValue {
        field,
        label: openchuck_app::setup::profile_field_label(field),
        value: String::new(),
        confirmed,
        tooltip: tooltip(profile, field),
        collapsed: false,
    };
    ui.horizontal(|ui| {
        ui.add_space(field_row_label_indent());
        crate::machine_panel::icon(ui, &value);
        let words = if confirmed {
            "Confirmed on the machine"
        } else {
            "Not confirmed on the machine"
        };
        ui.label(RichText::new(words).color(DARK.muted).small());
        if !read_only && !confirmed && ui.small_button("Mark confirmed…").clicked() {
            acts.push(Act::AskConfirmValue(field));
        }
    });
}

fn field_row_label_indent() -> f32 {
    8.0
}

fn import_question(ui: &mut egui::Ui, plan: &ImportPlan, acts: &mut Vec<Act>) {
    let name = &plan.entry.profile.name;
    match &plan.status {
        ImportStatus::New => {
            ui.label(format!(
                "Import \"{name}\" into the machine library as a new machine?"
            ));
            buttons(
                ui,
                acts,
                ("Import", Act::ApplyImport(ImportChoice::KeepBoth)),
                ("Cancel", Act::Dismiss),
            );
        }
        ImportStatus::Same { .. } => {
            ui.label(format!(
                "\"{name}\" is already known, with the same values. Nothing needs importing."
            ));
            if ui.button("Close").clicked() {
                acts.push(Act::Dismiss);
            }
        }
        ImportStatus::Differs { path, fields } => {
            let whose = if path.is_some() {
                "in the library"
            } else {
                "built in"
            };
            ui.label(format!(
                "\"{name}\" is the same machine as one {whose}, but differs in: {}.",
                fields.join(", ")
            ));
            ui.horizontal(|ui| {
                if ui.button("Keep mine").clicked() {
                    acts.push(Act::ApplyImport(ImportChoice::KeepMine));
                }
                if path.is_some() && ui.button("Replace").clicked() {
                    acts.push(Act::ApplyImport(ImportChoice::Replace));
                }
                if ui.button("Keep both").clicked() {
                    acts.push(Act::ApplyImport(ImportChoice::KeepBoth));
                }
            });
        }
    }
}

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

fn heading(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).color(DARK.muted).small().strong());
}

fn no_folder() -> String {
    "The machine library folder could not be found: the configuration folder is not set.".to_owned()
}

/// The words shown for a choice stored as a file word.
fn choice_label(word: &str) -> &str {
    match word {
        "x" => "X",
        "y" => "Y (not supported yet)",
        "positive" => "Positive",
        "negative" => "Negative",
        "continuous" => "Continuous",
        "limited" => "Limited",
        "separate" => "Separate",
        "makera" => "Makera",
        "off" => "Off",
        "report" => "Report",
        "slow_down" => "Slow down",
        other => other,
    }
}

/// Reads typed text for a field; a sum such as `12.5 + 3` is worked out as
/// the other number boxes do (UI-26). Blank is `None`.
fn parse(kind: &FieldKind, typed: &str) -> Result<Option<ParameterValue>, String> {
    if matches!(kind, FieldKind::Text) {
        return Ok(Some(ParameterValue::Text(typed.to_owned())));
    }
    form_panel::parse(kind, typed)
}

#[cfg(test)]
impl MachineLibraryWindow {
    /// The machine shown.
    pub fn selected(&self) -> Option<&Selected> {
        self.selected.as_ref()
    }

    /// The draft, if a library profile is chosen.
    pub fn draft(&self) -> Option<&MachineDraft> {
        self.draft.as_ref()
    }

    /// Chooses a machine, as a click in the list does.
    pub fn click(&mut self, selected: Selected) {
        self.act(Act::Choose(selected), None);
    }

    /// New machine.
    pub fn new_machine(&mut self) {
        self.act(Act::New, None);
    }

    /// Duplicate, or Copy to edit on a built-in profile.
    pub fn duplicate(&mut self, app: Option<&mut Application>) {
        self.act(Act::Duplicate, app);
    }

    /// Sets a value as the form does.
    pub fn set_value(
        &mut self,
        app: Option<&mut Application>,
        name: &'static str,
        value: Option<ParameterValue>,
    ) -> Option<String> {
        self.act(Act::Set(name, value), app);
        self.message.as_ref().filter(|m| m.1).map(|m| m.0.clone())
    }

    /// "Mark confirmed…", typing `how`, then "Mark confirmed".
    pub fn mark_confirmed(
        &mut self,
        app: Option<&mut Application>,
        field: ProfileField,
        how: &str,
    ) {
        self.act(Act::AskConfirmValue(field), None);
        if let Some(Ask::ConfirmValue(_, text)) = &mut self.ask {
            *text = how.to_owned();
        }
        self.act(Act::ConfirmValue, app);
    }

    /// Save; returns the question asked first, if any.
    pub fn save(&mut self) -> Option<String> {
        self.act(Act::Save, None);
        match &self.ask {
            Some(Ask::SaveRename(q, _)) => Some(q.clone()),
            _ => None,
        }
    }

    /// Answers the rename question: save, renaming the tool set or not.
    pub fn answer_rename(&mut self, rename: bool) {
        if let Some(Ask::SaveRename(_, tick)) = &mut self.ask {
            *tick = rename;
        }
        self.act(Act::SaveConfirmed, None);
    }

    /// Delete, answering the question with `yes`; returns the question.
    pub fn delete(&mut self, app: Option<&Application>, yes: bool) -> Option<String> {
        let _ = app;
        self.act(Act::AskDelete, None);
        let question = match &self.ask {
            Some(Ask::Delete(_, q)) => Some(q.clone()),
            _ => None,
        };
        self.act(if yes { Act::Delete } else { Act::Dismiss }, None);
        question
    }

    /// Imports `file`, answering with `choice`; returns the plan's status.
    pub fn import(&mut self, file: &Path, choice: ImportChoice) -> Option<ImportStatus> {
        self.act(Act::Import(file.to_path_buf()), None);
        let status = match &self.ask {
            Some(Ask::Import(plan)) => Some(plan.status.clone()),
            _ => None,
        };
        self.act(Act::ApplyImport(choice), None);
        status
    }

    /// Exports the machine shown to `file`.
    pub fn export(&mut self, app: Option<&mut Application>, file: &Path) {
        self.act(Act::Export(file.to_path_buf()), app);
    }

    /// "Save to library…" on the project's copy, then the answer.
    pub fn save_copy(
        &mut self,
        app: Option<&mut Application>,
        onto_source: bool,
    ) -> Option<Vec<&'static str>> {
        let app = app?;
        self.act(Act::AskSaveToLibrary, Some(&mut *app));
        let changes = match &self.ask {
            Some(Ask::SaveToLibrary(Some(_), c)) => Some(c.clone()),
            _ => None,
        };
        let act = if onto_source {
            Act::SaveCopyToSource
        } else {
            Act::SaveCopyAsNew
        };
        self.act(act, Some(app));
        changes
    }

    /// Opens a question without answering it, so a test can draw it.
    pub fn start(&mut self, app: Option<&mut Application>, which: &str, arg: Option<&Path>) {
        let act = match which {
            "delete" => Act::AskDelete,
            "import" => Act::Import(arg.map(Path::to_path_buf).unwrap_or_default()),
            "confirm" => Act::AskConfirmValue(ProfileField::Travel),
            "save copy" => Act::AskSaveToLibrary,
            "save" => Act::Save,
            _ => return,
        };
        self.act(act, app);
    }

    /// Cancel: drops the draft's changes.
    pub fn cancel(&mut self) {
        self.act(Act::CancelEdits, None);
    }

    /// Whether the discard question is showing.
    pub fn asks_to_discard(&self) -> bool {
        matches!(self.ask, Some(Ask::Discard(_)))
    }

    /// Answers the discard question.
    pub fn answer_discard(&mut self, discard: bool) {
        self.act(
            if discard {
                Act::Discard
            } else {
                Act::KeepEditing
            },
            None,
        );
    }

    /// The last message and whether it is a problem.
    pub fn message(&self) -> Option<&(String, bool)> {
        self.message.as_ref()
    }

    /// The machines listed.
    pub fn listed(&self) -> &[AvailableMachine] {
        &self.available
    }
}
