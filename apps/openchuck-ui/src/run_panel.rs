// SPDX-License-Identifier: AGPL-3.0-or-later
//! Adding operations and generating their toolpaths: the flow from a new
//! operation to a toolpath on screen (UI-06, UI-09, NFR-04; work breakdown
//! slice 9 item 7; decision record 0040 item 7).
//!
//! Generating runs as a job on the application's pool, so the window stays
//! live; the panel shows its progress and a Cancel button, then the results:
//! which operations were made and which came from the cache (editing
//! operation N regenerates N and the ones after it only), notes, the G-code
//! checks and, after a simulation, the material removed and the worst
//! remaining and overcut values (SIM-05).

use std::sync::Arc;
use std::time::Instant;

use eframe::egui::{self, Color32, RichText};
use openchuck_app::project::Project;
use openchuck_app::project::id::{OperationId, SelectionSetId, SetupId, ToolId};
use openchuck_app::{Application, JobHandle, JobId, JobLog, Regeneration, StrategyChoice};

use crate::theme::DARK;

/// How often the window is drawn again while a job runs, for its progress.
const RUNNING_REPAINT: std::time::Duration = std::time::Duration::from_millis(100);

const PROBLEM: Color32 = DARK.error;

/// What a group of results says when pointed at (issue 255).
const GROUP_HINT: &str = "The same note for consecutive moves, with values within 1 % of each \
     other. Open it to see each move.";
const NOTE: Color32 = DARK.warning;

/// How serious a line of the results is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Information.
    Info,
    /// Something the user should look at.
    Warning,
    /// Something wrong.
    Problem,
}

/// One line of the results window.
#[derive(Debug, Clone, PartialEq)]
pub struct ResultLine {
    /// How serious it is.
    pub severity: Severity,
    /// The sentence.
    pub text: String,
    /// The notes a grouped line holds, one per move, shown when it is
    /// opened (issue 255); empty for a line of its own.
    pub moves: Vec<String>,
}

/// The lines of the results window for `run`, in order: what was generated
/// and what was reused, the notes, the G-code checks, and the simulation
/// (SIM-05).
pub fn result_lines(run: &Regeneration, project: &Project) -> Vec<ResultLine> {
    let name = |id: OperationId| {
        project
            .operation(id)
            .map_or_else(|| "(removed operation)".to_owned(), |o| o.name.clone())
    };
    let line = |severity, text: String| ResultLine {
        severity,
        text,
        moves: Vec::new(),
    };
    let mut out = Vec::new();
    let names = |ids: &[OperationId]| {
        ids.iter()
            .map(|id| name(*id))
            .collect::<Vec<_>>()
            .join(", ")
    };
    if !run.generated.is_empty() {
        out.push(line(
            Severity::Info,
            format!("Generated: {}", names(&run.generated)),
        ));
    }
    if !run.reused.is_empty() {
        out.push(line(
            Severity::Info,
            format!(
                "Reused (nothing they depend on changed): {}",
                names(&run.reused)
            ),
        ));
    }
    // Repeated notes about consecutive moves are one line each (issue 255;
    // decision record 0218).
    for group in openchuck_app::note_groups::group_notes(&run.notes) {
        out.push(ResultLine {
            severity: Severity::Warning,
            text: group.text,
            moves: if group.notes.len() > 1 {
                group.notes
            } else {
                Vec::new()
            },
        });
    }
    for report in &run.reports {
        match &report.comparison {
            Ok(c) => out.push(line(
                Severity::Info,
                format!(
                    "{}: {} moves; replay agrees with the toolpath to {:.4} mm and {:.4}°",
                    report.file, report.moves, c.max_position_error, c.max_a_error
                ),
            )),
            Err(e) => out.push(line(
                Severity::Problem,
                format!("{}: the replay check failed: {e}", report.file),
            )),
        }
        for finding in &report.findings {
            out.push(line(
                Severity::Problem,
                format!("{}: {finding}", report.file),
            ));
        }
    }
    if let Some(simulation) = &run.simulation {
        for setup in &simulation.setups {
            out.push(line(
                Severity::Info,
                format!(
                    "Setup '{}': stock {:.0} mm³ before, {:.0} mm³ after ({:.0} mm³ removed)",
                    setup.setup,
                    setup.initial_volume,
                    setup.final_volume,
                    setup.initial_volume - setup.final_volume
                ),
            ));
            for op in &setup.operations {
                out.push(line(
                    Severity::Info,
                    format!(
                        "  {}: {:.0} mm³ removed in {} moves",
                        op.operation, op.removed_volume, op.moves
                    ),
                ));
                for cut in &op.rapid_cuts {
                    out.push(line(
                        Severity::Problem,
                        format!(
                            "  {}: move {} is a rapid that removes {:.3} mm³",
                            op.operation, cut.segment, cut.volume
                        ),
                    ));
                }
            }
            if let Some(d) = &setup.deviation {
                if let Some(p) = d.worst_remaining {
                    out.push(line(
                        Severity::Info,
                        format!(
                            "  Most material left: {:.3} mm at X {:.2}, Y {:.2}, Z {:.2}",
                            p.value, p.point.x, p.point.y, p.point.z
                        ),
                    ));
                }
                match d.worst_overcut {
                    Some(p) if d.overcut_count > 0 => out.push(line(
                        Severity::Problem,
                        format!(
                            "  Overcut: {:.3} mm deepest, at X {:.2}, Y {:.2}, Z {:.2}; {} samples beyond {:.2} mm",
                            -p.value, p.point.x, p.point.y, p.point.z, d.overcut_count, d.overcut_limit
                        ),
                    )),
                    Some(p) => out.push(line(
                        Severity::Info,
                        format!(
                            "  Deepest cut below the surface: {:.3} mm, within the simulation's error",
                            -p.value
                        ),
                    )),
                    None => out.push(line(Severity::Info, "  No overcut".to_owned())),
                }
            }
        }
    }
    out
}

/// The New operation dialog's material preset (issue 197; decision record
/// 0230): the setup's stock material, or one of the tool's other materials,
/// with a warning when it is not the setup's and a message when the tool has
/// no cutting data for it. A choice the tool does not have goes back to the
/// setup's material.
fn preset_row(
    ui: &mut egui::Ui,
    project: &openchuck_app::project::Project,
    setup: SetupId,
    tool: Option<ToolId>,
    material: &mut Option<String>,
) {
    use openchuck_app::preset::{PresetState, preset_for};
    let mut preset = preset_for(project, setup, tool, material.as_deref());
    if material.is_some() && preset.state != PresetState::Applied {
        *material = None;
        preset = preset_for(project, setup, tool, None);
    }
    if preset.state == PresetState::NoTool {
        return;
    }
    ui.horizontal(|ui| {
        ui.label("Material preset").on_hover_text(PRESET_HELP);
        let shown = preset
            .choices
            .iter()
            .find(|c| c.material == *material)
            .map_or("", |c| c.label.as_str());
        egui::ComboBox::from_id_salt("add op preset")
            .selected_text(shown)
            .show_ui(ui, |ui| {
                for c in &preset.choices {
                    ui.selectable_value(material, c.material.clone(), &c.label);
                }
            });
    });
    if let Some(why) = &preset.differs {
        ui.horizontal_wrapped(|ui| {
            crate::shell::mark(ui, openchuck_app::overview::ItemState::Warning, why);
            ui.label(why);
        });
    }
    if let Some(message) = &preset.message {
        ui.horizontal_wrapped(|ui| {
            ui.weak(message);
        });
    }
}

/// What the material preset is, for its tooltip.
pub(crate) const PRESET_HELP: &str = "The tool's cutting data for one stock material: the \
    spindle speed, feed per tooth, plunge feed, stepover and stepdown. It fills the \
    operation's cutting values; changing one of them drops the preset. Check the values \
    against your tool, material and machine before you run the program.";

struct RunningJob {
    handle: JobHandle<Regeneration>,
    simulate: bool,
    started: Instant,
}

/// The form for a new operation.
struct AddOperation {
    name: String,
    strategy: String,
    setup: Option<SetupId>,
    tool: Option<ToolId>,
    /// The material whose preset the operation takes; `None` for the
    /// setup's stock material (issue 197; decision record 0230).
    material: Option<String>,
    /// The selection set the new operation takes in Cut on, with its name.
    cut_on: Option<(SelectionSetId, String)>,
}

/// Generating, its results, and the add-operation dialog.
#[derive(Default)]
pub struct RunPanel {
    job: Option<RunningJob>,
    progress: Option<String>,
    last: Option<Arc<Regeneration>>,
    last_error: Option<String>,
    last_seconds: f64,
    show_results: bool,
    adding: Option<AddOperation>,
    /// The New operation dialog asked for the tool library (decision record
    /// 0127 item 6).
    wants_library: bool,
}

impl RunPanel {
    /// Gives the operation being made in the New operation dialog the
    /// project tool `tool`, chosen in the tool library window.
    pub fn set_adding_tool(&mut self, tool: ToolId) {
        if let Some(form) = &mut self.adding {
            form.tool = Some(tool);
        }
    }

    /// Whether the New operation dialog asked for the tool library since
    /// the last call.
    pub fn take_library_request(&mut self) -> bool {
        std::mem::take(&mut self.wants_library)
    }

    /// Forgets the results of a project that was closed.
    pub fn clear(&mut self) {
        if let Some(job) = self.job.take() {
            job.handle.cancel();
        }
        *self = Self::default();
    }

    /// Whether a job is running.
    pub fn is_running(&self) -> bool {
        self.job.is_some()
    }

    /// Starts generating, with the stock simulation unless the project skips
    /// it (issue 214), or always when `simulate` ("Generate and simulate").
    /// Does nothing while a job runs.
    pub fn start(&mut self, app: &Application, simulate: bool) {
        if self.job.is_some() {
            return;
        }
        let handle = if simulate {
            app.start_simulate()
        } else {
            app.start_generate()
        };
        self.begin(handle, simulate || app.simulates());
    }

    /// Starts generating up to and including `operation` (G, "generate
    /// selected"; decision record 0117, OI-351). Does nothing while a job
    /// runs.
    pub fn start_until(&mut self, app: &Application, operation: OperationId) {
        if self.job.is_none() {
            self.begin(app.start_generate_until(operation), app.simulates());
        }
    }

    fn begin(&mut self, handle: JobHandle<Regeneration>, simulate: bool) {
        self.last_error = None;
        self.progress = None;
        self.job = Some(RunningJob {
            handle,
            simulate,
            started: Instant::now(),
        });
    }

    /// Asks the running job to stop. It stops at its next check and leaves no
    /// partial result (UI-09).
    pub fn cancel(&self) {
        if let Some(job) = &self.job {
            job.handle.cancel();
        }
    }

    /// The latest finished run, with its simulation if one was made.
    pub fn last(&self) -> Option<&Arc<Regeneration>> {
        self.last.as_ref()
    }

    /// The message of the latest run if it failed.
    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    /// Finishes the job when it has. Returns a new run, with the job that
    /// made it. Progress is read from the shell's job log.
    pub fn poll(
        &mut self,
        jobs: &JobLog,
        ctx: &egui::Context,
    ) -> Option<(Arc<Regeneration>, JobId)> {
        let job = self.job.as_ref()?;
        let id = job.handle.id();
        // One share of the whole run, as a percentage that never starts
        // again at a new stage (issue 201).
        if let Some(percent) = jobs.get(id).and_then(openchuck_app::JobEntry::percent) {
            self.progress = Some(format!("{percent} %"));
        }
        match job.handle.try_result() {
            None => {
                // The progress text and the end of the job need no more than
                // this; a repaint every frame kept the graphics busy for the
                // whole of a long generation (issue 252).
                ctx.request_repaint_after(RUNNING_REPAINT);
                None
            }
            Some(result) => {
                let finished = self.job.take()?;
                self.last_seconds = finished.started.elapsed().as_secs_f64();
                self.progress = None;
                match result {
                    Ok(run) => {
                        let run = Arc::new(run);
                        self.last = Some(Arc::clone(&run));
                        Some((run, id))
                    }
                    Err(e) if e.is_cancelled() => {
                        self.last_error =
                            Some("Cancelled. Nothing was kept from that run.".to_owned());
                        None
                    }
                    Err(e) => {
                        self.last_error = Some(e.to_string());
                        self.show_results = true;
                        None
                    }
                }
            }
        }
    }

    /// What the footer says while a job runs: "Generating… 42 %", then the
    /// operation being worked on, or `None` when nothing runs.
    pub fn running_text(&self, app: Option<&Application>) -> Option<String> {
        let job = self.job.as_ref()?;
        let what = if job.simulate {
            "Simulating"
        } else {
            "Generating"
        };
        let mut text = match &self.progress {
            Some(p) => format!("{what}… {p}"),
            None => format!("{what}…"),
        };
        // The operation being worked on, and whether its simulation is being
        // run again because its stock was not cached (decision record 0190).
        if let Some(activity) = app.and_then(Application::pipeline_activity) {
            text.push_str(": ");
            text.push_str(&activity);
        }
        Some(text)
    }

    /// Whether the results window or the add-operation dialog is open.
    pub fn has_window(&self) -> bool {
        self.show_results || self.adding.is_some()
    }

    /// Whether the add-operation dialog is open.
    #[cfg(test)]
    #[must_use]
    pub fn is_adding(&self) -> bool {
        self.adding.is_some()
    }

    /// Opens the results window on the latest run.
    pub fn show_results(&mut self) {
        self.show_results = true;
    }

    /// Opens the add-operation dialog (11.4): the setup and tool of the
    /// operation `from` (the selected one), or else the last tool used in
    /// machining order, and the selection set `set` for Cut on.
    pub fn begin_add(
        &mut self,
        app: &Application,
        from: Option<OperationId>,
        set: Option<SelectionSetId>,
    ) {
        let project = app.project();
        let from = from.and_then(|id| project.operation(id));
        let setup = from
            .map(|o| o.setup)
            .or_else(|| project.setups().first().map(|s| s.id));
        // A strategy the setup's part cannot take is listed but not chosen
        // (decision record 0174).
        let choices: Vec<StrategyChoice> = app
            .strategy_choices_for(setup)
            .into_iter()
            .filter(|c| c.unavailable.is_none())
            .collect();
        let tool = from.and_then(|o| o.tool).or_else(|| {
            project
                .operations()
                .iter()
                .rev()
                .find_map(|o| o.tool)
                .or_else(|| project.tools().first().map(|t| t.id))
        });
        let cut_on = set.and_then(|id| {
            project
                .selection_sets()
                .iter()
                .find(|s| s.id == id)
                .map(|s| (id, s.name.clone()))
        });
        let strategy = choices
            .iter()
            .find(|c| cut_on.is_none() || c.takes_selections)
            .or(choices.first())
            .map(|c| c.name.clone())
            .unwrap_or_default();
        // The selected operation's preset comes along with its tool.
        let material = from
            .filter(|o| o.tool == tool)
            .and_then(|o| o.material.clone());
        self.adding = Some(AddOperation {
            name: format!("Operation {}", project.operations().len() + 1),
            strategy,
            setup,
            tool,
            material,
            cut_on,
        });
    }

    /// Draws the add-operation dialog. Returns the new operation when one was
    /// added, and an error message to show.
    pub fn add_dialog(
        &mut self,
        ctx: &egui::Context,
        app: &mut Application,
    ) -> (Option<OperationId>, Option<String>) {
        let Some(form) = &mut self.adding else {
            return (None, None);
        };
        let choices: Vec<StrategyChoice> = app.strategy_choices_for(form.setup);
        let setups: Vec<(SetupId, String)> = app
            .project()
            .setups()
            .iter()
            .map(|s| (s.id, s.name.clone()))
            .collect();
        let tools: Vec<(ToolId, String)> = app
            .project()
            .tools()
            .iter()
            .map(|t| (t.id, t.name.clone()))
            .collect();
        let mut close = false;
        let mut add = false;
        let mut wants_library = false;
        egui::Window::new("Add operation")
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Name");
                    ui.text_edit_singleline(&mut form.name);
                });
                // One list: roughing first, finishing next, the specialist
                // strategies last, each with a short description under its
                // name (issue 195; decision record 0204).
                ui.label("Strategy");
                egui::ScrollArea::vertical()
                    .id_salt("add op strategy list")
                    .max_height(300.0)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        let mut group = None;
                        for c in &choices {
                            if group != Some(c.group) {
                                group = Some(c.group);
                                ui.add_space(2.0);
                                ui.label(
                                    egui::RichText::new(group_heading(c.group))
                                        .small()
                                        .strong(),
                                );
                            }
                            // Disabled with the reason, not hidden: a
                            // strategy that needs a STEP part on a mesh part
                            // (decision record 0174).
                            if let Some(why) = &c.unavailable {
                                ui.add_enabled(
                                    false,
                                    egui::Button::selectable(
                                        false,
                                        format!("{} \u{b7} needs a STEP model", c.label),
                                    ),
                                )
                                .on_disabled_hover_text(why);
                            } else {
                                let label = if c.takes_selections {
                                    c.label.clone()
                                } else {
                                    format!("{} \u{b7} ignores selections", c.label)
                                };
                                ui.selectable_value(&mut form.strategy, c.name.clone(), label);
                            }
                            ui.horizontal_wrapped(|ui| {
                                ui.add_space(18.0);
                                ui.weak(&c.description);
                            });
                        }
                    });
                ui.horizontal(|ui| {
                    ui.label("Setup");
                    let shown = setups
                        .iter()
                        .find(|(id, _)| Some(*id) == form.setup)
                        .map_or("none: add a setup first", |(_, n)| n.as_str());
                    egui::ComboBox::from_id_salt("add op setup")
                        .selected_text(shown)
                        .show_ui(ui, |ui| {
                            for (id, name) in &setups {
                                ui.selectable_value(&mut form.setup, Some(*id), name);
                            }
                        });
                });
                ui.horizontal(|ui| {
                    ui.label("Tool");
                    let shown = tools
                        .iter()
                        .find(|(id, _)| Some(*id) == form.tool)
                        .map_or("none yet", |(_, n)| n.as_str());
                    egui::ComboBox::from_id_salt("add op tool")
                        .selected_text(shown)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut form.tool, None, "none yet");
                            for (id, name) in &tools {
                                ui.selectable_value(&mut form.tool, Some(*id), name);
                            }
                        });
                    if ui
                        .button("From the library…")
                        .on_hover_text("Choose a tool in the tool library and add it to the project")
                        .clicked()
                    {
                        wants_library = true;
                    }
                });
                if let Some(setup) = form.setup {
                    preset_row(ui, app.project(), setup, form.tool, &mut form.material);
                }
                let takes = choices
                    .iter()
                    .find(|c| c.name == form.strategy)
                    .is_some_and(|c| c.takes_selections);
                if let Some((_, set)) = &form.cut_on {
                    if takes {
                        ui.label(format!("Cut on: {set}"));
                    } else {
                        ui.weak(format!(
                            "This strategy ignores selections, so it does not take '{set}'."
                        ));
                    }
                }
                // The setup chosen after the strategy may not take it.
                let unavailable = choices
                    .iter()
                    .find(|c| c.name == form.strategy)
                    .and_then(|c| c.unavailable.clone());
                if let Some(why) = &unavailable {
                    ui.horizontal_wrapped(|ui| {
                        crate::shell::mark(
                            ui,
                            openchuck_app::overview::ItemState::Warning,
                            why,
                        );
                        ui.label(format!("{why}."));
                    });
                }
                ui.weak("Everything else starts at the strategy's defaults; set them in the operation's panel.");
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            form.setup.is_some() && unavailable.is_none(),
                            egui::Button::new("Add"),
                        )
                        .clicked()
                    {
                        add = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                });
            });
        let mut added = None;
        let mut error = None;
        if add && let Some(setup) = form.setup {
            let takes = choices
                .iter()
                .find(|c| c.name == form.strategy)
                .is_some_and(|c| c.takes_selections);
            let cut_on = form.cut_on.as_ref().filter(|_| takes).map(|(id, _)| *id);
            match app.add_operation_on(
                &form.name,
                &form.strategy,
                setup,
                form.tool,
                cut_on,
                form.material.as_deref(),
            ) {
                Ok(id) => {
                    added = Some(id);
                    close = true;
                }
                Err(e) => error = Some(e.to_string()),
            }
        }
        if close {
            self.adding = None;
        }
        self.wants_library |= wants_library;
        (added, error)
    }

    /// Draws the results window.
    pub fn results_window(&mut self, ctx: &egui::Context, app: &Application) {
        if !self.show_results {
            return;
        }
        let mut open = true;
        egui::Window::new("Results")
            .open(&mut open)
            .default_width(520.0)
            .show(ctx, |ui| {
                if let Some(error) = &self.last_error {
                    ui.colored_label(PROBLEM, error);
                    ui.separator();
                }
                match &self.last {
                    None => {
                        ui.label("Nothing has been generated yet.");
                    }
                    Some(run) => {
                        if *run.project != *app.project() {
                            ui.colored_label(
                                NOTE,
                                "The project has changed since this run. Generate again.",
                            );
                        }
                        ui.label(format!(
                            "{} operation(s), {} moves, {:.1} s to generate",
                            run.toolpath.groups.len(),
                            run.toolpath
                                .groups
                                .iter()
                                .map(|g| g.segments.len())
                                .sum::<usize>(),
                            self.last_seconds
                        ));
                        egui::ScrollArea::vertical()
                            .max_height(360.0)
                            .show(ui, |ui| {
                                for (index, line) in
                                    result_lines(run, app.project()).into_iter().enumerate()
                                {
                                    let colour = |text: RichText| match line.severity {
                                        Severity::Info => text,
                                        Severity::Warning => text.color(NOTE),
                                        Severity::Problem => text.color(PROBLEM),
                                    };
                                    if line.moves.is_empty() {
                                        ui.label(colour(RichText::new(&line.text)));
                                        continue;
                                    }
                                    // A group opens on its moves, one line
                                    // each; it starts closed (issue 255).
                                    egui::CollapsingHeader::new(colour(RichText::new(&line.text)))
                                        .id_salt(("result group", index))
                                        .default_open(false)
                                        .show(ui, |ui| {
                                            for note in &line.moves {
                                                ui.label(colour(RichText::new(note)));
                                            }
                                        })
                                        .header_response
                                        .on_hover_text(GROUP_HINT);
                                }
                            });
                        ui.separator();
                        ui.weak(openchuck_app::VERIFY_GCODE_NOTICE);
                    }
                }
            });
        self.show_results = open;
    }
}

/// The heading over a group of the strategy list.
fn group_heading(group: openchuck_app::StrategyGroup) -> &'static str {
    use openchuck_app::StrategyGroup;
    match group {
        StrategyGroup::Roughing => "Roughing",
        StrategyGroup::Finishing => "Finishing",
        StrategyGroup::Specialist => "Specialist",
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn req_nfr_12_repeated_notes_are_one_line_that_opens_on_each_move() {
        // Issue 255: the results group repeated notes about consecutive moves
        // and keep each move's note for when the line is opened.
        let notes: Vec<String> = (2..=4)
            .map(|m| {
                format!(
                    "operation 'Roughing': move {m}: the machine's feed limits slow this move: \
                     the contact feed is 972.9 mm/min, not 1000.0"
                )
            })
            .collect();
        let run = Regeneration {
            notes: notes.clone(),
            ..Regeneration::default()
        };
        let lines = result_lines(&run, &Project::new("notes"));
        assert_eq!(lines.len(), 1);
        assert!(lines[0].text.contains("moves 2 to 4 (3 moves)"));
        assert_eq!(lines[0].moves, notes);
        assert_eq!(lines[0].severity, Severity::Warning);
    }

    #[test]
    fn req_ui_09_an_empty_run_has_no_lines_and_a_new_panel_is_not_stale_for_nothing() {
        let run = Regeneration::default();
        let project = Project::new("empty");
        assert!(result_lines(&run, &project).is_empty());
        let panel = RunPanel::default();
        assert!(!panel.is_running());
        assert!(panel.last.is_none());
    }

    /// While a job runs the window is drawn again at a steady pace for its
    /// progress, not at every frame the display can show (issue 252: a
    /// generation of 20 minutes kept the graphics busy the whole time).
    #[test]
    fn req_ui_09_req_nfr_04_a_running_job_redraws_at_a_steady_pace() {
        use std::time::{Duration, Instant};

        use openchuck_app::JobLog;

        use crate::testing::cube_project;

        let (dir, app, _, _) = cube_project("run-pace");
        let ctx = egui::Context::default();
        let pass = |f: &mut dyn FnMut(&egui::Context)| {
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| f(ui.ctx()));
            // No renderer here takes the font texture.
            output.textures_delta.clear();
            output
                .viewport_output
                .get(&egui::ViewportId::ROOT)
                .map(|v| v.repaint_delay)
        };
        // egui's own first passes ask for repaints of their own; with
        // nothing running it then waits for input.
        let mut warm = 0;
        while pass(&mut |_| {}) != Some(Duration::MAX) {
            warm += 1;
            assert!(warm < 10);
        }
        let mut panel = RunPanel::default();
        panel.start(&app, false);
        let started = Instant::now();
        let mut running_passes = 0;
        while panel.is_running() {
            let delay = pass(&mut |ctx| {
                panel.poll(&JobLog::default(), ctx);
            });
            if panel.is_running() {
                running_passes += 1;
                // egui takes its own frame time off the delay.
                let delay = delay.unwrap();
                assert!(
                    delay > Duration::from_millis(50) && delay <= RUNNING_REPAINT,
                    "{delay:?}"
                );
            }
            assert!(started.elapsed() < Duration::from_secs(120));
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(running_passes > 0);
        drop(app);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// A cancelled run must not make an older run look current: what the
    /// application records changes only when a run succeeds (UI-11).
    #[test]
    fn req_ui_09_req_ui_11_a_cancelled_run_leaves_an_older_run_stale() {
        use std::time::{Duration, Instant};

        use openchuck_app::JobLog;
        use openchuck_app::overview::{ItemRef, ItemState};

        use crate::testing::cube_project;

        let (dir, mut app, op, _) = cube_project("run-stale");
        let ctx = egui::Context::default();
        let mut panel = RunPanel::default();
        let finish = |panel: &mut RunPanel| {
            let started = Instant::now();
            while panel.is_running() {
                panel.poll(&JobLog::default(), &ctx);
                assert!(started.elapsed() < Duration::from_secs(120));
                std::thread::sleep(Duration::from_millis(5));
            }
        };
        let state = |app: &Application| {
            app.overview(None, &JobLog::default())
                .status(ItemRef::Operation(op))
                .map(|s| s.state)
        };
        panel.start(&app, false);
        finish(&mut panel);
        assert_eq!(state(&app), Some(ItemState::UpToDate));
        // An edit makes the run out of date.
        app.set_parameter(
            op,
            "stepover",
            Some(openchuck_app::project::ParameterValue::Number(0.7)),
        )
        .unwrap();
        assert_eq!(state(&app), Some(ItemState::Stale));
        // A run that is started and cancelled does not make it current.
        panel.start(&app, true);
        panel.cancel();
        finish(&mut panel);
        assert_eq!(state(&app), Some(ItemState::Stale));
        // A run that succeeds does.
        panel.start(&app, false);
        finish(&mut panel);
        assert_eq!(state(&app), Some(ItemState::UpToDate));
        let _ = std::fs::remove_dir_all(dir);
    }

    /// A new operation starts with the last tool used and takes the
    /// selection set shown (interface specification 11.4; decision record
    /// 0114 C16).
    #[test]
    fn req_ui_06_a_new_operation_takes_the_last_tool_and_the_selection() {
        use openchuck_app::project::SelectionTarget;
        let (dir, mut app, op, _) = crate::testing::cube_project("run-new-op");
        let operation = app.project().operation(op).unwrap().clone();
        let model = app.project().models()[0].id;
        let set = app
            .save_selection("Top", model, SelectionTarget::Triangles([0, 1].into()))
            .unwrap();
        let mut panel = RunPanel::default();
        panel.begin_add(&app, None, Some(set));
        let form = panel.adding.as_ref().unwrap();
        assert_eq!(form.tool, operation.tool);
        assert_eq!(form.setup, Some(operation.setup));
        assert_eq!(form.cut_on.as_ref().map(|(id, _)| *id), Some(set));
        let takes = app
            .strategy_choices()
            .into_iter()
            .find(|c| c.name == form.strategy)
            .unwrap()
            .takes_selections;
        assert!(takes, "{}", form.strategy);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
