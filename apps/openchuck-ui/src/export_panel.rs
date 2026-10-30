// SPDX-License-Identifier: AGPL-3.0-or-later
//! Post and export: the dialog that writes G-code (GC-02, GC-04, GC-05,
//! GC-12, GC-14 to GC-16; specification section 11.6; decision record 0071).
//!
//! The left column chooses the post, the file grouping and the output folder,
//! settles the tool numbers and lists the checks before writing. The right
//! column lists the files to write with a preview; the setup sheet is
//! written with the G-code, from the same run (decision 0189). The amber safety notice is
//! always shown, and the tick box under it is asked once per project on each
//! computer (the user's settings file, never the project). "Write G-code" is
//! disabled until the checks pass and the box is ticked, and says which is
//! missing. The dialog only draws and wires: the checks, the plan, the tool
//! numbering and the writing are in `openchuck-app`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use eframe::egui::{self, Color32, RichText};
use openchuck_app::project::Project;
use openchuck_app::project::id::ToolId;
use openchuck_app::tools::{ToolNumbering, tool_number_plan, tool_numbering_command};
use openchuck_app::{
    AcknowledgementLookup, Acknowledgements, Application, CheckLevel, EXPORT_ACKNOWLEDGE_LABEL,
    EXPORT_NOTICE, EXPORT_RULE, EXPORT_STOCK_SKIPPED, EXPORT_STOCK_SKIPPED_LABEL, ExportChecks,
    JobHandle, OutputGrouping, OutputPlan, OutputReport, PlannedFile, PostSelection, PostSummary,
    Regeneration, builtin_post_summaries, post_summary,
};

use crate::floating::{WindowEvent, floating_unscrolled};
use crate::theme::DARK;

#[path = "export_panel/feedback.rs"]
mod feedback;

#[path = "export_panel/file_names.rs"]
mod file_names;

const PROBLEM: Color32 = DARK.error;
const WARNING: Color32 = DARK.warning;
/// The safety notice is the warning box of the tokens (14.1).
const NOTICE_FILL: Color32 = DARK.warning_box.bg;
/// How many lines of a file the preview shows.
const PREVIEW_LINES: usize = 14;

#[cfg(test)]
#[path = "export_panel/layout_tests.rs"]
mod layout_tests;

#[cfg(test)]
#[path = "export_panel/write_feedback_tests.rs"]
mod write_feedback_tests;

/// How the tool numbers are settled (specification 11.6 item 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NumberChoice {
    /// The numbers as the project has them.
    Keep,
    /// One, two, three and so on in order of first use.
    InOrder,
    /// Numbers typed for each tool.
    ByHand,
}

/// What a G-code run was made from; a new key means the files are made again.
#[derive(Debug, Clone)]
struct Key {
    post: PostSelection,
    grouping: OutputGrouping,
    project: Project,
}

/// Two keys are the same when they give the same files: what only the view
/// uses (the hidden toolpaths) and the stock simulation choice do not count
/// (decision records 0168 and 0169; GC-07).
impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        self.post == other.post
            && self.grouping == other.grouping
            && self.project.same_output(&other.project)
    }
}

/// The files made for a key.
struct Files {
    key: Key,
    run: Arc<Regeneration>,
    plan: Vec<PlannedFile>,
}

/// Something the user asked for this frame, done after drawing.
#[derive(Debug, Clone, PartialEq)]
enum Act {
    Close,
    ChooseFolder,
    ApplyNumbers(ToolNumbering),
    Write,
    Replace,
    CancelOverwrite,
    DismissResult,
}

struct PendingOutput {
    plan: OutputPlan,
    key: Key,
    folder: String,
    names: openchuck_app::FileNameTemplates,
}

/// What the dialog hands back to the shell.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Outcome {
    /// A retained result was added, so the shell refreshes Messages.
    pub reported: bool,
    /// A sentence for the status bar.
    pub status: Option<String>,
    /// A problem to show in an error dialog.
    pub error: Option<String>,
}

/// The export dialog's state.
pub struct ExportPanel {
    overwrite: Option<PendingOutput>,
    result: Option<OutputReport>,
    deferred: Option<(crate::pending_edits::Ticket, Act)>,
    open: bool,
    posts: Vec<PostSummary>,
    /// Index into `posts`; `posts.len()` means the post folder in `custom`.
    post_index: usize,
    custom: String,
    grouping: OutputGrouping,
    folder: String,
    number_choice: NumberChoice,
    by_hand: Vec<(ToolId, String)>,
    job: Option<(JobHandle<Regeneration>, Key)>,
    /// The key the last job was started for, so a failed one is not retried
    /// every frame.
    attempted: Option<Key>,
    files: Option<Files>,
    files_error: Option<String>,
    /// What the chosen post says, read when the choice changes and not on
    /// every frame (a post folder is read from disk).
    summary: Option<(PostSelection, Result<PostSummary, String>)>,
    preview: usize,
    ack: Option<Acknowledgements>,
    ack_lookup: AcknowledgementLookup,
    /// Whether this computer has the acknowledgement that the project skips
    /// the stock simulation, and the box for it (OI-602; decision record
    /// 0168).
    skip_lookup: AcknowledgementLookup,
    skip_ticked: bool,
    ticked: bool,
    /// The user's file name templates (issues 308 and 310).
    file_names: openchuck_app::FileNameTemplates,
    /// The names they give the current files.
    named: Option<file_names::Named>,
}

impl Default for ExportPanel {
    fn default() -> Self {
        Self::new(Acknowledgements::default_location())
    }
}

impl ExportPanel {
    /// A closed dialog that keeps acknowledgements in `ack` (the user's
    /// settings file; `None` when the configuration folder cannot be found,
    /// in which case the box is asked every time).
    pub fn new(ack: Option<Acknowledgements>) -> Self {
        Self {
            overwrite: None,
            result: None,
            deferred: None,
            open: false,
            posts: builtin_post_summaries(),
            post_index: 0,
            custom: String::new(),
            grouping: OutputGrouping::default(),
            folder: String::new(),
            number_choice: NumberChoice::Keep,
            by_hand: Vec::new(),
            job: None,
            attempted: None,
            files: None,
            files_error: None,
            summary: None,
            preview: 0,
            ack,
            skip_lookup: AcknowledgementLookup {
                acknowledged: false,
                problem: None,
            },
            skip_ticked: false,
            ack_lookup: AcknowledgementLookup {
                acknowledged: false,
                problem: None,
            },
            ticked: false,
            file_names: openchuck_app::FileNameTemplates::default(),
            named: None,
        }
    }

    /// Whether the dialog is open.
    pub fn is_open(&self) -> bool {
        self.open || self.overwrite.is_some() || self.result.is_some()
    }

    /// Forgets everything about a project that was closed.
    pub fn clear(&mut self) {
        if let Some((job, _)) = self.job.take() {
            job.cancel();
        }
        let names = std::mem::take(&mut self.file_names);
        *self = Self::new(self.ack.take());
        self.file_names = names;
    }

    /// Opens the dialog for `app`'s project (Ctrl E).
    pub fn open(&mut self, app: &Application) {
        self.open = true;
        self.attempted = None;
        self.files = None;
        self.files_error = None;
        self.summary = None;
        self.ticked = false;
        self.skip_ticked = false;
        self.refresh_acknowledgement(app);
        self.read_numbers(app);
    }

    fn refresh_acknowledgement(&mut self, app: &Application) {
        self.ack_lookup = self.ack.as_ref().map_or_else(
            || AcknowledgementLookup {
                acknowledged: false,
                problem: Some(
                    "This computer's settings folder was not found, so exports ask each time."
                        .to_owned(),
                ),
            },
            |a| a.lookup(app.location()),
        );
        self.skip_lookup = self.ack.as_ref().map_or(
            AcknowledgementLookup {
                acknowledged: false,
                problem: None,
            },
            |a| a.lookup_stock_skipped(app.location()),
        );
    }

    /// Whether the project skips the stock simulation, or the files were made
    /// while it did: the skip does not change the files, so they stay current
    /// when it is unticked, and still hold a cut that was not simulated.
    fn skips(&self, app: &Application) -> bool {
        app.project().skip_stock_simulation()
            || self
                .files_current(app)
                .is_some_and(|f| f.key.project.skip_stock_simulation())
    }

    /// Whether writing waits for the acknowledgement that the project skips
    /// the stock simulation (OI-602).
    fn skip_unacknowledged(&self, app: &Application) -> bool {
        self.skips(app) && !(self.skip_lookup.acknowledged || self.skip_ticked)
    }

    fn read_numbers(&mut self, app: &Application) {
        self.by_hand = tool_number_plan(app.project())
            .into_iter()
            .map(|r| (r.tool, r.number.to_string()))
            .collect();
    }

    /// What the chosen post says, loaded again only when the choice changes.
    fn summary(&mut self) -> Option<&Result<PostSummary, String>> {
        let selection = self.selection()?;
        if self.summary.as_ref().map(|(s, _)| s) != Some(&selection) {
            let loaded = post_summary(&selection).map_err(|e| e.to_string());
            self.summary = Some((selection, loaded));
        }
        self.summary.as_ref().map(|(_, r)| r)
    }

    /// The post chosen, or why it cannot be read.
    fn selection(&self) -> Option<PostSelection> {
        match self.posts.get(self.post_index) {
            Some(summary) => Some(summary.selection.clone()),
            None if self.custom.trim().is_empty() => None,
            None => Some(PostSelection::Folder(PathBuf::from(self.custom.trim()))),
        }
    }

    fn key(&self, app: &Application) -> Option<Key> {
        Some(Key {
            post: self.selection()?,
            grouping: self.grouping,
            project: app.project().clone(),
        })
    }

    fn files_current(&self, app: &Application) -> Option<&Files> {
        let files = self.files.as_ref()?;
        (files.key.project.same_output(app.project())
            && Some(&files.key.post) == self.selection().as_ref()
            && files.key.grouping == self.grouping)
            .then_some(files)
    }

    /// The numbers typed for a by-hand renumbering, or why they are not numbers.
    fn typed_numbers(&self) -> Result<Vec<(ToolId, u32)>, String> {
        self.by_hand
            .iter()
            .map(|(id, text)| {
                text.trim()
                    .parse::<u32>()
                    .map(|n| (*id, n))
                    .map_err(|_| format!("'{}' is not a tool number.", text.trim()))
            })
            .collect()
    }

    fn numbering(&self) -> Result<ToolNumbering, String> {
        Ok(match self.number_choice {
            NumberChoice::Keep => ToolNumbering::Keep,
            NumberChoice::InOrder => ToolNumbering::InOrderOfUse { first: 1 },
            NumberChoice::ByHand => ToolNumbering::Manual(self.typed_numbers()?),
        })
    }

    /// What is wrong with the tool numbers as they stand in the project, if
    /// anything (a number used twice, or outside the machine's range). A
    /// choice not yet applied is not looked at: it is checked when applied.
    fn numbers_problem(app: &Application) -> Option<String> {
        let max = app.project().machine().map(|m| m.max_tool_number);
        tool_numbering_command(app.project(), &ToolNumbering::Keep, max)
            .err()
            .map(|e| e.to_string())
    }

    /// Why "Write G-code" is disabled, if it is: the first thing missing.
    fn write_blocker(
        &mut self,
        app: &Application,
        checks: &ExportChecks,
        acknowledged: bool,
    ) -> Option<String> {
        if self.job.is_some() {
            return Some("The files are being made. Wait for them.".to_owned());
        }
        if let Some(blocker) = checks.first_blocker() {
            return Some(blocker.to_owned());
        }
        if let Some(problem) = Self::numbers_problem(app) {
            return Some(problem);
        }
        if self.selection().is_none() {
            return Some("Choose a post folder.".to_owned());
        }
        if let Some(error) = &self.files_error {
            return Some(error.clone());
        }
        match self.files_current(app) {
            None => return Some("The files are not made yet.".to_owned()),
            Some(files) if files.run.gcode.is_empty() => {
                return Some("The toolpath is empty, so there is no G-code to write.".to_owned());
            }
            Some(_) => {}
        }
        // Nothing is written until every file has a name of its own
        // (issues 308 to 310).
        if let Some(problem) = self.names_problem(app) {
            return Some(problem);
        }
        if self.folder.trim().is_empty() {
            return Some("Choose an output folder.".to_owned());
        }
        if !acknowledged {
            return Some(
                "Tick the box to say you will verify the G-code before running it.".to_owned(),
            );
        }
        if self.skip_unacknowledged(app) {
            return Some(
                "Tick the box to say you know the cut has not been simulated for this project."
                    .to_owned(),
            );
        }
        None
    }

    /// Reads the running job, and starts one when the choices or the project
    /// have changed and nothing blocks making the files.
    fn drive(
        &mut self,
        ctx: &egui::Context,
        app: &mut Application,
        run: Option<&Arc<Regeneration>>,
        run_error: Option<&str>,
    ) {
        if let Some((job, key)) = &self.job {
            match job.try_result() {
                None => {
                    ctx.request_repaint();
                    return;
                }
                Some(result) => {
                    let key = key.clone();
                    self.job = None;
                    match result.and_then(|run| {
                        let plan = app.planned_files(&run, &key.post, key.grouping)?;
                        Ok((Arc::new(run), plan))
                    }) {
                        Ok((run, plan)) => {
                            self.files_error = None;
                            self.preview = self.preview.min(plan.len().saturating_sub(1));
                            self.files = Some(Files { key, run, plan });
                        }
                        Err(e) => {
                            self.files = None;
                            self.files_error = Some(e.to_string());
                        }
                    }
                }
            }
        }
        // The custom post path is a dialog draft. Do not start a posting
        // job from it until the field has been confirmed.
        if crate::pending_edits::entries(ctx)
            .iter()
            .any(|e| e.label == "Post folder")
        {
            return;
        }
        let Some(key) = self.key(app) else { return };
        if self.files_current(app).is_some() || self.attempted.as_ref() == Some(&key) {
            return;
        }
        // Nothing is made while a blocker says the toolpaths are missing, out
        // of date or failed: the dialog says so instead.
        let checks = app.export_checks(run.map(AsRef::as_ref), None, run_error);
        if !checks.can_write() {
            return;
        }
        self.attempted = Some(key.clone());
        let handle = app.start_regenerate_with_post(key.post.clone(), key.grouping);
        self.job = Some((handle, key));
        ctx.request_repaint();
    }

    /// Draws the dialog when it is open and does what the user asked.
    /// `run` is the latest generation (with its simulation, if one was made)
    /// and `run_error` the message of a later failed one.
    pub fn frame(
        &mut self,
        ctx: &egui::Context,
        app: &mut Application,
        run: Option<&Arc<Regeneration>>,
        run_error: Option<&str>,
    ) -> Outcome {
        let mut outcome = Outcome::default();
        if !self.is_open() {
            return outcome;
        }
        if self.open {
            self.drive(ctx, app, run, run_error);
        }
        // The names previewed are the names written: both come from the
        // application with these templates.
        app.set_file_name_templates(self.file_names.clone());
        let posted = self.files_current(app).map(|f| Arc::clone(&f.run));
        let checks = app.export_checks(run.map(AsRef::as_ref), posted.as_deref(), run_error);
        let acknowledged = self.ack_lookup.acknowledged || self.ticked;
        let blocker = self.write_blocker(app, &checks, acknowledged);
        let mut acts = Vec::new();
        if let Some(action) = crate::pending_edits::take_local(ctx, &mut self.deferred) {
            acts.push(action);
        }
        if self.open {
            let (_, event) = floating_unscrolled(
                ctx,
                "export dialog",
                "Post and export",
                [980.0, 700.0],
                |ui| {
                    self.draw(ui, app, &checks, blocker.as_deref(), &mut acts);
                },
            );
            if event == WindowEvent::CloseRequested {
                acts.push(Act::Close);
            }
        }
        self.feedback_frame(ctx, &mut acts);
        for act in acts {
            let Some(act) = crate::pending_edits::defer_local(ctx, &mut self.deferred, act) else {
                continue;
            };
            match act {
                Act::DismissResult => self.result = None,
                Act::CancelOverwrite => self.overwrite = None,
                Act::Replace => {
                    if let Some(pending) = self.overwrite.take() {
                        if blocker.is_none()
                            && self.key(app).as_ref() == Some(&pending.key)
                            && self.folder == pending.folder
                            && self.file_names == pending.names
                        {
                            outcome = self.finish_write(app, &pending.plan, true);
                        } else {
                            let reason = blocker.clone().unwrap_or_else(|| {
                                "The export choices changed. Review the files before writing."
                                    .to_owned()
                            });
                            outcome = self.refused(app, pending.plan.paths(), reason);
                        }
                    }
                }
                Act::Close => self.open = false,
                Act::ChooseFolder => {
                    let mut dialog = rfd::FileDialog::new().set_title("Output folder");
                    if let Some(start) = existing_dir(&self.folder) {
                        dialog = dialog.set_directory(start);
                    }
                    if let Some(picked) = dialog.pick_folder() {
                        self.folder = picked.display().to_string();
                    }
                }
                Act::ApplyNumbers(_) => {
                    let numbering = match self.numbering() {
                        Ok(numbering) => numbering,
                        Err(error) => {
                            outcome.error = Some(error);
                            continue;
                        }
                    };
                    let max = app.project().machine().map(|m| m.max_tool_number);
                    match tool_numbering_command(app.project(), &numbering, max) {
                        Ok(Some(command)) => match app.execute(command) {
                            Ok(()) => {
                                outcome.status = Some(
                                    "Tool numbers changed. Undo with Ctrl Z; save the project to keep them."
                                        .to_owned(),
                                );
                                self.number_choice = NumberChoice::Keep;
                                self.read_numbers(app);
                            }
                            Err(e) => outcome.error = Some(e.to_string()),
                        },
                        Ok(None) => {
                            outcome.status = Some("The tool numbers stay as they are.".to_owned());
                            self.number_choice = NumberChoice::Keep;
                        }
                        Err(e) => outcome.error = Some(e.to_string()),
                    }
                }
                Act::Write => {
                    // The button was disabled for a blocker; check again so a
                    // stale click cannot write.
                    if blocker.is_none()
                        && self.result.is_none()
                        && self.overwrite.is_none()
                        && let Some(files) = posted.as_ref()
                    {
                        outcome = self.write(app, files);
                    }
                }
            }
        }
        outcome
    }

    /// Writes the G-code and the setup sheet into the output folder, and
    /// keeps the acknowledgement on this computer after the first export of a
    /// project.
    fn write(&mut self, app: &Application, files: &Arc<Regeneration>) -> Outcome {
        let folder = PathBuf::from(self.folder.trim());
        match app.prepare_output(&folder, files) {
            Ok(plan) if !plan.existing().is_empty() => {
                if let Some(key) = self.key(app) {
                    self.overwrite = Some(PendingOutput {
                        plan,
                        key,
                        folder: self.folder.clone(),
                        names: self.file_names.clone(),
                    });
                }
                Outcome::default()
            }
            Ok(plan) => self.finish_write(app, &plan, false),
            Err(error) => {
                let folder = std::path::absolute(&folder).unwrap_or(folder);
                // The names the templates give, when they give any: a
                // naming problem lists no destination it did not settle.
                let paths: Vec<_> = app.output_files(files).map_or_else(
                    |_| Vec::new(),
                    |named| {
                        named
                            .programs
                            .iter()
                            .map(|p| &p.name)
                            .chain(&named.setup_sheet)
                            .map(|name| folder.join(name))
                            .collect()
                    },
                );
                let failed = match &error {
                    openchuck_app::AppError::Io { path, .. } => path.clone(),
                    _ => paths.first().cloned().unwrap_or(folder),
                };
                let report = OutputReport::refused(&paths, failed, error.to_string());
                app.record_output_report(report.clone());
                self.show_report(report.clone());
                Outcome {
                    reported: true,
                    status: Some(report.summary()),
                    error: None,
                }
            }
        }
    }

    fn refused(&mut self, app: &Application, paths: &[PathBuf], reason: String) -> Outcome {
        let path = paths.first().cloned().unwrap_or_default();
        let report = OutputReport::refused(paths, path, reason);
        app.record_output_report(report.clone());
        self.show_report(report.clone());
        Outcome {
            reported: true,
            status: Some(report.summary()),
            error: None,
        }
    }

    fn finish_write(&mut self, app: &Application, plan: &OutputPlan, replace: bool) -> Outcome {
        let mut outcome = Outcome::default();
        // The sheet is written from the same run as the G-code (decision
        // 0189), and the export recorded: the step bar says "G-code written"
        // until something changes (UI-10, decision record 0282 item 5).
        let report = app.write_output(plan, replace);
        // The acknowledgement is kept once a file really was written,
        // including partial success (accepted decision 0071).
        if !report.written.is_empty() {
            if !self.ack_lookup.acknowledged
                && let Some(ack) = &self.ack
            {
                if let Err(e) = ack.acknowledge(app.location()) {
                    outcome.error = Some(format!(
                        "The files were written, but the acknowledgement could not be kept on this computer: {e}"
                    ));
                }
                self.refresh_acknowledgement(app);
            }
            // The skip's acknowledgement is kept the same way (OI-602).
            if self.skips(app)
                && !self.skip_lookup.acknowledged
                && let Some(ack) = &self.ack
            {
                if let Err(e) = ack.acknowledge_stock_skipped(app.location()) {
                    outcome.error = Some(format!(
                        "The files were written, but the acknowledgement could not be kept on this computer: {e}"
                    ));
                }
                self.refresh_acknowledgement(app);
            }
        }
        outcome.reported = true;
        outcome.status = Some(report.summary());
        self.show_report(report);
        outcome
    }

    fn draw(
        &mut self,
        ui: &mut egui::Ui,
        app: &Application,
        checks: &ExportChecks,
        blocker: Option<&str>,
        acts: &mut Vec<Act>,
    ) {
        egui::Panel::bottom("export footer").show(ui, |ui| {
            ui.add_space(4.0);
            // Its own wrapped line: beside the buttons it ran out of the dialog and over
            // the primary button (issue 222).
            if let Some(reason) = blocker {
                ui.add(egui::Label::new(RichText::new(reason).weak()).wrap());
            }
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // The primary styling shows the action that can be taken now; a
                    // blocked one is drawn as an ordinary disabled button (OI-349).
                    let button = if blocker.is_none() {
                        egui::Button::new(
                            RichText::new("Write G-code")
                                .color(DARK.primary_text)
                                .strong(),
                        )
                        .fill(DARK.primary)
                        .stroke(egui::Stroke::new(1.0, DARK.primary_border))
                    } else {
                        egui::Button::new("Write G-code")
                    };
                    let write = ui.add_enabled(blocker.is_none(), button);
                    let write = match blocker {
                        Some(reason) => write.on_disabled_hover_text(reason),
                        None => write,
                    };
                    if write.clicked() {
                        acts.push(Act::Write);
                    }
                    if ui.button("Cancel").clicked() {
                        acts.push(Act::Close);
                    }
                });
            });
            ui.add_space(4.0);
        });
        // Keep the body below the fixed actions, and leave the scrollbar
        // its own strip (UI-16, issue 311). Future preview content belongs
        // inside this body, without changing the footer or its scroll bounds.
        crate::theme::solid_scroll_bar(ui);
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.columns(2, |columns| {
                let [left, right] = columns else { return };
                self.left(left, app, checks, acts);
                let current = self.files_current(app).is_some();
                self.right(right, app, current);
            });
            ui.add_space(8.0);
            self.notice(ui);
            self.skip_notice(ui, app);
        });
    }

    fn left(
        &mut self,
        ui: &mut egui::Ui,
        app: &Application,
        checks: &ExportChecks,
        acts: &mut Vec<Act>,
    ) {
        ui.heading("Post-processor");
        egui::ComboBox::from_label("Post")
            .selected_text(
                self.posts
                    .get(self.post_index)
                    .map_or("A post folder", |p| p.name.as_str()),
            )
            .show_ui(ui, |ui| {
                for (i, post) in self.posts.iter().enumerate() {
                    ui.selectable_value(&mut self.post_index, i, &post.name);
                }
                ui.selectable_value(&mut self.post_index, self.posts.len(), "A post folder");
            });
        if self.post_index >= self.posts.len() {
            crate::pending_edits::text_edit(ui, &mut self.custom, "Post folder")
                .on_hover_text("The folder that holds the post's files.");
        }
        match self.summary().cloned() {
            Some(Ok(post)) => {
                egui::Grid::new("export post facts")
                    .num_columns(2)
                    .show(ui, |ui| {
                        for (label, value) in [
                            ("Units", post.units.as_str()),
                            ("Rotary axis", post.a_letter.as_str()),
                            ("Wrap", post.a_wrap.as_str()),
                            ("Feed mode", post.feed_mode.as_str()),
                        ] {
                            ui.label(label);
                            // A long value wraps inside its column instead of running under
                            // the Files pane (UI-16; OI-532).
                            ui.add(egui::Label::new(value).wrap());
                            ui.end_row();
                        }
                    });
                ui.weak("The post sets these. Change them in the post's files.");
            }
            Some(Err(e)) => {
                ui.colored_label(PROBLEM, e);
            }
            None => {
                ui.colored_label(WARNING, "Choose the folder of a post.");
            }
        }
        ui.add_space(4.0);
        let setup_runs = setup_runs(app);
        ui.label("Files");
        for grouping in OutputGrouping::ALL {
            let (label, hint) = match grouping {
                OutputGrouping::Project => (
                    "One file for the project",
                    "Every operation in one program.",
                ),
                OutputGrouping::Setup => (
                    "One file per setup",
                    "The default: one program for each setup.",
                ),
                OutputGrouping::Operation => {
                    ("One file per operation", "A program for each operation.")
                }
                OutputGrouping::Tool => (
                    "One file per tool",
                    "A program for each run of operations that use one tool.",
                ),
            };
            let blocked = grouping == OutputGrouping::Project && setup_runs > 1;
            let response = ui.add_enabled(
                !blocked,
                egui::RadioButton::new(self.grouping == grouping, label),
            );
            let response = if blocked {
                response.on_disabled_hover_text(
                    "Each setup has its own work offset, so a project with more than one setup cannot go in one file.",
                )
            } else {
                response.on_hover_text(hint)
            };
            if response.clicked() {
                self.grouping = grouping;
            }
        }
        ui.horizontal(|ui| {
            ui.label("Output folder");
            crate::pending_edits::text_edit(ui, &mut self.folder, "Output folder");
            if ui.button("Choose…").clicked() {
                acts.push(Act::ChooseFolder);
            }
        });
        self.file_names_section(ui, app);

        ui.separator();
        self.numbers(ui, app, acts);
        ui.separator();
        self.checks(ui, checks);
    }

    fn numbers(&mut self, ui: &mut egui::Ui, app: &Application, acts: &mut Vec<Act>) {
        ui.heading("Tool numbers");
        let plan = tool_number_plan(app.project());
        match app.project().machine() {
            Some(machine) => ui.label(format!(
                "The machine accepts tool numbers 1 to {}.",
                machine.max_tool_number
            )),
            None => ui.colored_label(WARNING, "The project has no machine."),
        };
        if plan.is_empty() {
            ui.label("No enabled operation uses a tool, so the program has no tool numbers.");
            return;
        }
        egui::Grid::new("export tool numbers")
            .num_columns(3)
            .striped(true)
            .show(ui, |ui| {
                ui.strong("Tool");
                ui.strong("Used in");
                ui.strong("Program number");
                ui.end_row();
                for row in &plan {
                    ui.label(&row.name);
                    ui.label(used_in(app.project(), row.tool));
                    match self.number_choice {
                        NumberChoice::ByHand => {
                            if let Some((_, text)) =
                                self.by_hand.iter_mut().find(|(id, _)| *id == row.tool)
                            {
                                let before = text.clone();
                                let response = ui.add(
                                    crate::theme::single_line(ui, &mut *text).desired_width(48.0),
                                );
                                crate::pending_edits::draft_text(
                                    ui,
                                    &response,
                                    "Tool number",
                                    &before,
                                    text,
                                    |text| {
                                        text.trim()
                                            .parse::<u32>()
                                            .map(|_| text.to_owned())
                                            .map_err(|_| "Enter a whole tool number.".to_owned())
                                    },
                                );
                            }
                        }
                        NumberChoice::InOrder => {
                            let n = plan
                                .iter()
                                .position(|r| r.tool == row.tool)
                                .map_or(0, |i| i + 1);
                            ui.label(format!("T{n}"));
                        }
                        NumberChoice::Keep => {
                            ui.label(format!("T{}", row.number));
                        }
                    }
                    ui.end_row();
                }
            });
        ui.horizontal_wrapped(|ui| {
            for (choice, label) in [
                (NumberChoice::Keep, "Keep as saved"),
                (NumberChoice::InOrder, "Renumber in order of use"),
                (NumberChoice::ByHand, "By hand"),
            ] {
                if ui
                    .selectable_value(&mut self.number_choice, choice, label)
                    .clicked()
                {
                    self.read_numbers(app);
                }
            }
        });
        let max = app.project().machine().map(|m| m.max_tool_number);
        let pending = self.numbering().map_err(|e| e.clone()).and_then(|n| {
            tool_numbering_command(app.project(), &n, max).map_err(|e| e.to_string())
        });
        match &pending {
            Err(reason) => {
                ui.colored_label(PROBLEM, reason);
            }
            Ok(Some(_)) => {
                ui.weak("These numbers are not in the project yet.");
            }
            Ok(None) => {}
        }
        if let Some(problem) = Self::numbers_problem(app) {
            ui.colored_label(PROBLEM, problem);
        }
        ui.horizontal(|ui| {
            let label = match self.number_choice {
                NumberChoice::Keep => "Confirm numbers",
                _ => "Use these numbers",
            };
            if ui
                .add_enabled(pending.is_ok(), egui::Button::new(label))
                .on_hover_text("The numbers are saved in the project, so the next export gives the same G-code. Undo with Ctrl Z.")
                .clicked()
                && let Ok(numbering) = self.numbering()
            {
                acts.push(Act::ApplyNumbers(numbering));
            }
        });
    }

    fn checks(&self, ui: &mut egui::Ui, checks: &ExportChecks) {
        ui.heading("Checks before writing");
        if checks.checks.is_empty() {
            ui.label("Nothing to report.");
        }
        for check in &checks.checks {
            let (mark, colour) = match check.level {
                CheckLevel::Blocking => ("Blocks writing:", PROBLEM),
                CheckLevel::Warning => ("Warning:", WARNING),
            };
            ui.horizontal_wrapped(|ui| {
                ui.colored_label(colour, mark);
                ui.label(&check.text);
            });
        }
        if !checks.not_posted.is_empty() {
            ui.label(format!(
                "Not written (disabled): {}.",
                checks.not_posted.join(", ")
            ));
        }
        ui.weak(EXPORT_RULE);
    }

    /// The files column. `current` says the files were made from the project and
    /// choices as they stand; old files are not listed or previewed, since
    /// they are not what "Write G-code" would write.
    fn right(&mut self, ui: &mut egui::Ui, app: &Application, current: bool) {
        ui.heading("Files to write");
        if self.job.is_some() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Making the files…");
            });
        }
        if let Some(error) = &self.files_error {
            ui.colored_label(PROBLEM, error);
        }
        // The names the templates give, the ones written (issues 308, 310).
        let named = if current {
            self.named(app).cloned()
        } else {
            None
        };
        let Some(files) = self.files.as_ref().filter(|_| current) else {
            if self.job.is_none() && self.files_error.is_none() {
                ui.weak("The files appear here once every operation is generated and up to date.");
            }
            return;
        };
        let mut chosen = None;
        egui::Grid::new("export files")
            .num_columns(4)
            .striped(true)
            .show(ui, |ui| {
                ui.strong("File");
                ui.strong("Setup");
                ui.strong("Lines");
                ui.strong("Programmed time");
                ui.end_row();
                for (i, planned) in files.plan.iter().enumerate() {
                    let lines = files
                        .run
                        .gcode
                        .iter()
                        .find(|(name, _)| *name == planned.name)
                        .map_or(0, |(_, text)| text.lines().count());
                    let shown = final_name(named.as_ref(), &planned.name);
                    if ui.selectable_label(self.preview == i, shown).clicked() {
                        chosen = Some(i);
                    }
                    ui.label(&planned.setup);
                    ui.label(lines.to_string());
                    ui.label(duration(planned.programmed_seconds));
                    ui.end_row();
                }
            });
        if let Some((settings, sources)) = files.run.effective_short_blocks {
            ui.label(format!(
                "Short-block settings: {} ({}), {:.3} ms ({}), {} {} ({}).{}",
                settings.mode.label(),
                sources.mode.as_str(),
                settings.min_block_ms,
                sources.min_block_ms.as_str(),
                settings.block_window,
                if settings.block_window == 1 {
                    "block"
                } else {
                    "blocks"
                },
                sources.block_window.as_str(),
                if settings.min_block_ms == 0.0
                    || settings.mode == openchuck_app::setup::ShortBlockMode::Off
                {
                    " Check off; no slowdown applied."
                } else {
                    ""
                }
            ));
        }
        for report in &files.run.short_blocks {
            let operation = files
                .run
                .project
                .operations()
                .iter()
                .find(|o| o.id.to_string() == report.operation)
                .map_or(report.operation.as_str(), |o| o.name.as_str());
            let mode = match report.settings.mode {
                openchuck_app::setup::ShortBlockMode::Off => "Off",
                openchuck_app::setup::ShortBlockMode::Report => "Report",
                openchuck_app::setup::ShortBlockMode::SlowDown => "Slow down",
            };
            ui.label(format!("Short blocks — {operation}: {mode}, {:.3} ms over {} {} (limit: {}, window: {}, mode: {}). {} affected windows before slowdown; worst average {:.3} ms; {} moves below the limit, longest run {}. Effective feed {:.3} mm/min; {:.3} s {}.",
                report.settings.min_block_ms, report.settings.block_window, if report.settings.block_window == 1 { "block" } else { "blocks" }, report.sources.min_block_ms.as_str(), report.sources.block_window.as_str(), report.sources.mode.as_str(),
                report.affected_windows, report.worst_window_ms, report.short_moves, report.longest_run, report.effective_feed, report.added_seconds,
                if report.settings.mode == openchuck_app::setup::ShortBlockMode::SlowDown { "added" } else { "would be added by Slow down" }));
        }
        match &named {
            Some(Ok(named)) => {
                let [html, pdf] = &named.setup_sheet;
                ui.label(format!(
                    "The setup sheet is written with the G-code, from the same run: {html} and {pdf}. It lists each program file with its SHA-256."
                ));
                if let Some(warning) = named.warning() {
                    ui.add(egui::Label::new(RichText::new(warning).color(WARNING)).wrap());
                }
            }
            Some(Err(problem)) => {
                ui.add(egui::Label::new(RichText::new(problem).color(PROBLEM)).wrap());
            }
            None => {}
        }
        ui.weak("Programmed time is the sum of the moves at their programmed feeds. It is not an estimate of the machine's time.");
        ui.add_space(4.0);
        let shown_index = chosen.unwrap_or(self.preview);
        if let Some(planned) = files.plan.get(shown_index) {
            ui.strong(format!(
                "Start of {}",
                final_name(named.as_ref(), &planned.name)
            ));
            if let Some((_, text)) = files.run.gcode.iter().find(|(n, _)| *n == planned.name) {
                let shown: Vec<&str> = text.lines().take(PREVIEW_LINES).collect();
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    for line in &shown {
                        ui.label(RichText::new(*line).monospace());
                    }
                });
            }
        }
        if let Some(i) = chosen {
            self.preview = i;
        }
    }

    /// While the project skips the stock simulation, the warning and its tick
    /// box in the safety band, asked once per project on each computer like
    /// the export's (OI-602, the owner's option C; decision record 0168).
    fn skip_notice(&mut self, ui: &mut egui::Ui, app: &Application) {
        if !self.skips(app) {
            return;
        }
        ui.add_space(6.0);
        ui.label(RichText::new(EXPORT_STOCK_SKIPPED).color(DARK.warning_box.text));
        if self.skip_lookup.acknowledged {
            ui.add_enabled(
                false,
                egui::Checkbox::new(&mut true.clone(), EXPORT_STOCK_SKIPPED_LABEL),
            );
            ui.label("Acknowledged for this project on this computer.");
        } else {
            ui.checkbox(&mut self.skip_ticked, EXPORT_STOCK_SKIPPED_LABEL);
        }
    }

    fn notice(&mut self, ui: &mut egui::Ui) {
        egui::Frame::new()
            .fill(NOTICE_FILL)
            .stroke(egui::Stroke::new(1.0, DARK.warning_box.border))
            .inner_margin(egui::Margin::same(10))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new(EXPORT_NOTICE).color(DARK.warning_box.text));
                ui.add_space(4.0);
                if self.ack_lookup.acknowledged {
                    ui.add_enabled(
                        false,
                        egui::Checkbox::new(&mut true.clone(), EXPORT_ACKNOWLEDGE_LABEL),
                    );
                    ui.label("Acknowledged for this project on this computer.");
                } else {
                    ui.checkbox(&mut self.ticked, EXPORT_ACKNOWLEDGE_LABEL);
                    ui.label(
                        "This is asked once for each project on each computer. Opening the project on another computer asks again.",
                    );
                }
                if let Some(problem) = &self.ack_lookup.problem {
                    ui.colored_label(WARNING, problem);
                }
            });
    }
}

/// The name the templates give the file posted as `posted`; a dash while the
/// templates cannot name the files (the problem is shown beside the list).
fn final_name<'a>(
    named: Option<&'a Result<openchuck_app::OutputFiles, String>>,
    posted: &'a str,
) -> &'a str {
    match named {
        Some(Ok(named)) => named
            .programs
            .iter()
            .find(|p| p.posted_name == posted)
            .map_or(posted, |p| p.name.as_str()),
        Some(Err(_)) => "—",
        None => posted,
    }
}

/// How many runs of consecutive operations in one setup the enabled
/// operations make; more than one means one file for the project is refused.
fn setup_runs(app: &Application) -> usize {
    let mut runs = 0;
    let mut last = None;
    for operation in app.project().operations().iter().filter(|o| o.enabled) {
        if last != Some(operation.setup) {
            runs += 1;
            last = Some(operation.setup);
        }
    }
    runs
}

/// The names of the enabled operations that use `tool`.
fn used_in(project: &Project, tool: ToolId) -> String {
    project
        .operations()
        .iter()
        .filter(|o| o.enabled && o.tool == Some(tool))
        .map(|o| o.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Seconds as "3 min 07 s", or "42 s".
fn duration(seconds: f64) -> String {
    let total = seconds.max(0.0).round() as u64;
    if total >= 60 {
        format!("{} min {:02} s", total / 60, total % 60)
    } else {
        format!("{total} s")
    }
}

/// The folder `text` names, or its parent, if it exists.
fn existing_dir(text: &str) -> Option<PathBuf> {
    let path = Path::new(text.trim());
    if text.trim().is_empty() {
        return None;
    }
    if path.is_dir() {
        Some(path.to_path_buf())
    } else {
        path.parent().filter(|p| p.is_dir()).map(Path::to_path_buf)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::time::{Duration, Instant};

    use super::*;
    use crate::testing::cube_project;

    fn frame(
        panel: &mut ExportPanel,
        ctx: &egui::Context,
        app: &mut Application,
        run: Option<&Arc<Regeneration>>,
    ) -> Outcome {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(1400.0, 900.0),
            )),
            ..Default::default()
        };
        let mut outcome = Outcome::default();
        let mut output = ctx.run_ui(input, |ui| {
            outcome = panel.frame(ui.ctx(), app, run, None);
        });
        output.textures_delta.clear();
        outcome
    }

    fn settle(
        panel: &mut ExportPanel,
        ctx: &egui::Context,
        app: &mut Application,
        run: Option<&Arc<Regeneration>>,
    ) {
        let deadline = Instant::now() + Duration::from_secs(120);
        loop {
            frame(panel, ctx, app, run);
            if panel.job.is_none() && (panel.files.is_some() || panel.files_error.is_some()) {
                return;
            }
            assert!(Instant::now() < deadline, "timed out");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn settings(name: &str) -> (PathBuf, Acknowledgements) {
        let dir = std::env::temp_dir().join(format!("openchuck-ui-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        (dir.clone(), Acknowledgements::in_folder(&dir))
    }

    /// Whether one frame of the dialog's body paints a rectangle in the primary fill.
    fn paints_primary(blocker: Option<&str>) -> bool {
        let (dir, app, _, _) = cube_project("export-primary");
        let mut panel = ExportPanel::new(None);
        let checks = app.export_checks(None, None, None);
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(1400.0, 900.0),
            )),
            ..Default::default()
        };
        let mut acts = Vec::new();
        let mut output = ctx.run_ui(input, |ui| {
            panel.draw(ui, &app, &checks, blocker, &mut acts);
        });
        output.textures_delta.clear();
        let _ = std::fs::remove_dir_all(dir);
        output.shapes.iter().any(|clipped| {
            matches!(&clipped.shape, egui::epaint::Shape::Rect(r) if r.fill == DARK.primary)
        })
    }

    #[test]
    fn req_ui_16_the_posts_long_wrap_line_stays_in_its_column() {
        // OI-532: "whole turns dropped without motion" ran under the Files pane.
        let (dir, mut app, _, _) = cube_project("export-wrap");
        let mut panel = ExportPanel::new(None);
        panel.open(&app);
        let checks = app.export_checks(None, None, None);
        let ctx = egui::Context::default();
        let mut paint = || {
            let mut acts = Vec::new();
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::pos2(0.0, 0.0),
                        egui::vec2(800.0, 900.0),
                    )),
                    ..Default::default()
                },
                |ui| panel.draw(ui, &app, &checks, None, &mut acts),
            );
            output.textures_delta.clear();
            let mut texts = Vec::new();
            for clipped in &output.shapes {
                if let egui::epaint::Shape::Text(t) = &clipped.shape {
                    texts.push((t.galley.text().to_owned(), t.pos.x, t.galley.size().x));
                }
            }
            texts
        };
        paint();
        let texts = paint();
        let files_x = texts
            .iter()
            .find(|(t, _, _)| t == "Files to write")
            .map(|(_, x, _)| *x)
            .expect("the right column is drawn");
        let wrap = texts
            .iter()
            .find(|(t, _, _)| t.contains("whole turns"))
            .expect("the Wrap value is drawn");
        assert!(wrap.1 + wrap.2 <= files_x, "{wrap:?} against {files_x}");
        app.set_post(None);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn req_nfr_15_write_gcode_has_the_primary_styling_only_when_it_can_be_pressed() {
        // OI-349: a blocked button is an ordinary disabled button, not a dimmed primary.
        assert!(paints_primary(None));
        assert!(!paints_primary(Some("Choose an output folder first.")));
    }

    #[test]
    fn req_gc_15_nothing_can_be_written_before_the_operations_are_generated() {
        let (dir, mut app, _, _) = cube_project("export-none");
        let (settings_dir, ack) = settings("export-none-settings");
        let mut panel = ExportPanel::new(Some(ack));
        panel.open(&app);
        let ctx = egui::Context::default();
        frame(&mut panel, &ctx, &mut app, None);
        assert!(panel.job.is_none(), "no files are made from nothing");
        let checks = app.export_checks(None, None, None);
        let reason = panel.write_blocker(&app, &checks, true).unwrap();
        assert!(reason.contains("Generate"), "{reason}");
        let _ = std::fs::remove_dir_all(dir);
        let _ = std::fs::remove_dir_all(settings_dir);
    }

    #[test]
    fn req_gc_05_req_gc_14_req_gc_15_req_gc_16_the_dialog_writes_files_once_every_check_passes() {
        let (dir, mut app, op, _) = cube_project("export-flow");
        app.set_post(None);
        let (settings_dir, ack) = settings("export-flow-settings");
        let out = dir.join("out");
        let mut panel = ExportPanel::new(Some(ack));
        let ctx = egui::Context::default();
        let run = Arc::new(app.regenerate().unwrap());
        panel.open(&app);
        settle(&mut panel, &ctx, &mut app, Some(&run));
        assert!(
            app.post().is_none(),
            "the dialog leaves the application's post alone"
        );
        let files = panel.files.as_ref().unwrap();
        assert_eq!(files.plan.len(), 1);
        assert_eq!(files.run.gcode.len(), 1);
        // The folder, then the box, are what is still missing.
        let checks = app.export_checks(Some(&run), panel.files.as_ref().map(|f| &*f.run), None);
        assert!(checks.can_write(), "{checks:?}");
        let reason = panel.write_blocker(&app, &checks, false).unwrap();
        assert!(reason.contains("output folder"), "{reason}");
        panel.folder = out.display().to_string();
        let reason = panel.write_blocker(&app, &checks, false).unwrap();
        assert!(reason.contains("Tick the box"), "{reason}");
        assert_eq!(panel.write_blocker(&app, &checks, true), None);
        // Writing keeps the acknowledgement on this computer, not in the project.
        panel.ticked = true;
        let posted = panel
            .files_current(&app)
            .map(|f| Arc::clone(&f.run))
            .unwrap();
        let outcome = panel.write(&app, &posted);
        assert_eq!(outcome.error, None);
        assert_eq!(outcome.status.as_deref(), Some("Wrote 3 files."));
        assert_eq!(panel.result.as_ref().unwrap().written.len(), 3);
        assert!(out.join(&posted.gcode[0].0).exists());
        // The setup sheet is written with the G-code (decision 0189).
        for name in app.setup_sheet_file_names().unwrap() {
            assert!(out.join(&name).exists(), "{name}");
        }
        assert!(
            panel.ack_lookup.acknowledged,
            "acknowledged after the first export"
        );
        let again = ExportPanel::new(Some(Acknowledgements::in_folder(&settings_dir)));
        let mut again = again;
        again.open(&app);
        assert!(
            again.ack_lookup.acknowledged,
            "not asked again on this computer"
        );
        let project_text: String = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| std::fs::read_to_string(e.unwrap().path()).ok())
            .collect();
        assert!(
            !project_text.contains("acknowledged"),
            "never in the project"
        );
        let mut fresh = ExportPanel::new(Some(Acknowledgements::in_folder(
            &settings_dir.join("other"),
        )));
        fresh.open(&app);
        assert!(
            !fresh.ack_lookup.acknowledged,
            "another computer asks again"
        );
        // An edit after generating makes the operation out of date: blocked.
        app.set_parameter(
            op,
            "stepover",
            Some(openchuck_app::project::ParameterValue::Number(0.7)),
        )
        .unwrap();
        let checks = app.export_checks(Some(&run), None, None);
        let reason = panel.write_blocker(&app, &checks, true).unwrap();
        assert!(reason.contains("out of date"), "{reason}");
        assert!(
            panel.files_current(&app).is_none(),
            "old files are not listed or previewed once the project changed"
        );
        let _ = std::fs::remove_dir_all(dir);
        let _ = std::fs::remove_dir_all(settings_dir);
    }

    #[test]
    fn req_nfr_12_req_gc_16_a_skipped_stock_simulation_is_acknowledged_before_writing() {
        // OI-602, the owner's option C (decision record 0168): while the
        // project skips the stock simulation, writing waits for its own box,
        // asked once per project on each computer like the export's.
        let (dir, mut app, _, _) = cube_project("export-skip");
        app.set_post(None);
        app.execute(openchuck_app::Command::SetSkipStockSimulation(true))
            .unwrap();
        let (settings_dir, ack) = settings("export-skip-settings");
        let mut panel = ExportPanel::new(Some(ack));
        let ctx = egui::Context::default();
        let run = Arc::new(app.regenerate().unwrap());
        panel.open(&app);
        settle(&mut panel, &ctx, &mut app, Some(&run));
        panel.folder = dir.join("out").display().to_string();
        let checks = app.export_checks(Some(&run), panel.files.as_ref().map(|f| &*f.run), None);
        assert!(checks.can_write(), "a warning, not a blocking check");
        assert!(
            checks
                .checks
                .iter()
                .any(|c| c.text == openchuck_app::EXPORT_STOCK_SKIPPED)
        );
        let reason = panel.write_blocker(&app, &checks, true).unwrap();
        assert!(reason.contains("not been simulated"), "{reason}");
        panel.skip_ticked = true;
        assert_eq!(panel.write_blocker(&app, &checks, true), None);
        panel.ticked = true;
        let posted = panel
            .files_current(&app)
            .map(|f| Arc::clone(&f.run))
            .unwrap();
        assert_eq!(panel.write(&app, &posted).error, None);
        // Kept on this computer: not asked again.
        let mut again = ExportPanel::new(Some(Acknowledgements::in_folder(&settings_dir)));
        again.open(&app);
        assert!(again.skip_lookup.acknowledged);
        assert!(again.ack_lookup.acknowledged);
        assert!(!again.skip_unacknowledged(&app));
        // A project that does not skip asks nothing more.
        app.execute(openchuck_app::Command::SetSkipStockSimulation(false))
            .unwrap();
        let mut fresh = ExportPanel::new(Some(Acknowledgements::in_folder(
            &settings_dir.join("other"),
        )));
        fresh.open(&app);
        assert!(!fresh.skip_unacknowledged(&app));
        let _ = std::fs::remove_dir_all(dir);
        let _ = std::fs::remove_dir_all(settings_dir);
    }

    #[test]
    fn req_nfr_12_files_made_while_skipping_still_ask_after_the_skip_is_unticked() {
        // OI-602 (decision record 0168): the skip does not change the files,
        // so files made while it was ticked stay current when it is unticked;
        // they still hold a cut that was not simulated, so writing them still
        // waits for the box.
        let (dir, mut app, _, _) = cube_project("export-skip-unticked");
        app.set_post(None);
        app.execute(openchuck_app::Command::SetSkipStockSimulation(true))
            .unwrap();
        let (settings_dir, ack) = settings("export-skip-unticked-settings");
        let mut panel = ExportPanel::new(Some(ack));
        let ctx = egui::Context::default();
        let run = Arc::new(app.regenerate().unwrap());
        panel.open(&app);
        settle(&mut panel, &ctx, &mut app, Some(&run));
        panel.folder = dir.join("out").display().to_string();
        app.execute(openchuck_app::Command::SetSkipStockSimulation(false))
            .unwrap();
        assert!(
            panel.files_current(&app).is_some(),
            "the files stay current"
        );
        assert!(panel.skip_unacknowledged(&app));
        let checks = app.export_checks(Some(&run), panel.files.as_ref().map(|f| &*f.run), None);
        assert!(
            checks
                .checks
                .iter()
                .any(|c| c.text == openchuck_app::EXPORT_STOCK_SKIPPED)
        );
        let reason = panel.write_blocker(&app, &checks, true).unwrap();
        assert!(reason.contains("not been simulated"), "{reason}");
        let _ = std::fs::remove_dir_all(dir);
        let _ = std::fs::remove_dir_all(settings_dir);
    }

    #[test]
    fn req_gc_05_one_file_for_the_project_is_offered_only_with_one_setup_run() {
        let (dir, app, _, _) = cube_project("export-runs");
        assert_eq!(setup_runs(&app), 1);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn req_gc_14_the_tool_number_dialog_takes_typed_numbers_and_refuses_text() {
        let (dir, mut app, _, _) = cube_project("export-numbers");
        let (settings_dir, ack) = settings("export-numbers-settings");
        let mut panel = ExportPanel::new(Some(ack));
        panel.open(&app);
        assert_eq!(panel.by_hand.len(), 1);
        panel.number_choice = NumberChoice::ByHand;
        panel.by_hand[0].1 = "x".to_owned();
        assert!(panel.numbering().unwrap_err().contains("not a tool number"));
        panel.by_hand[0].1 = "9".to_owned();
        let numbering = panel.numbering().unwrap();
        let max = app.project().machine().map(|m| m.max_tool_number);
        let command = tool_numbering_command(app.project(), &numbering, max)
            .unwrap()
            .unwrap();
        app.execute(command).unwrap();
        assert_eq!(tool_number_plan(app.project())[0].number, 9);
        assert!(app.can_undo());
        assert_eq!(ExportPanel::numbers_problem(&app), None);
        let _ = std::fs::remove_dir_all(dir);
        let _ = std::fs::remove_dir_all(settings_dir);
    }

    #[test]
    fn req_gc_04_the_post_folder_is_read_when_the_choice_changes_not_every_frame() {
        let (settings_dir, ack) = settings("export-summary-settings");
        let copy = settings_dir.join("post-copy");
        std::fs::create_dir_all(&copy).unwrap();
        let posts = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../crates/openchuck-post/posts/linuxcnc");
        for name in ["post.toml", "main.post"] {
            std::fs::copy(posts.join(name), copy.join(name)).unwrap();
        }
        let mut panel = ExportPanel::new(Some(ack));
        panel.post_index = panel.posts.len();
        panel.custom = copy.display().to_string();
        assert!(panel.summary().unwrap().is_ok());
        // With the folder gone the cached answer stands until the choice changes.
        std::fs::remove_dir_all(&copy).unwrap();
        assert!(panel.summary().unwrap().is_ok(), "read again from disk");
        panel.custom.push('x');
        assert!(panel.summary().unwrap().is_err(), "a new choice is read");
        let _ = std::fs::remove_dir_all(settings_dir);
    }

    #[test]
    fn req_gc_16_the_notice_is_fixed_and_never_says_safe() {
        assert!(EXPORT_NOTICE.starts_with("Verify this G-code before you run it."));
        assert!(EXPORT_NOTICE.contains("run an air cut first"));
        for text in [EXPORT_NOTICE, EXPORT_ACKNOWLEDGE_LABEL, EXPORT_RULE] {
            let lower = text.to_lowercase();
            for banned in ["safe", "ready to run", "machine-ready", "verified"] {
                assert!(!lower.contains(banned), "{banned} in {text}");
            }
        }
        assert_eq!(duration(42.4), "42 s");
        assert_eq!(duration(187.0), "3 min 07 s");
    }
}
