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
use openchuck_app::project::id::{OperationId, SetupId, ToolId};
use openchuck_app::{Application, JobHandle, JobId, JobLog, Regeneration, StrategyChoice};

const PROBLEM: Color32 = Color32::from_rgb(230, 80, 80);
const NOTE: Color32 = Color32::from_rgb(230, 180, 60);

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
    let line = |severity, text: String| ResultLine { severity, text };
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
    for note in &run.notes {
        out.push(line(Severity::Warning, note.clone()));
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
}

impl RunPanel {
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

    /// Starts generating, and simulating when `simulate`. Does nothing while
    /// a job runs.
    pub fn start(&mut self, app: &Application, simulate: bool) {
        if self.job.is_some() {
            return;
        }
        self.last_error = None;
        self.progress = None;
        let handle = if simulate {
            app.start_simulate()
        } else {
            app.start_regenerate()
        };
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
        if let Some(entry) = jobs.get(id)
            && let Some(progress) = &entry.progress
        {
            self.progress = Some(match progress.total {
                Some(total) => format!("{} of {total}", progress.done),
                None => progress.stage.clone(),
            });
        }
        match job.handle.try_result() {
            None => {
                ctx.request_repaint();
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
                        self.show_results = true;
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

    /// What the footer says while a job runs: "Generating… 2 of 4", or
    /// `None` when nothing runs.
    pub fn running_text(&self) -> Option<String> {
        let job = self.job.as_ref()?;
        let what = if job.simulate {
            "Simulating"
        } else {
            "Generating"
        };
        Some(match &self.progress {
            Some(p) => format!("{what}… {p}"),
            None => format!("{what}…"),
        })
    }

    /// Whether the results window or the add-operation dialog is open.
    pub fn has_window(&self) -> bool {
        self.show_results || self.adding.is_some()
    }

    /// Opens the results window on the latest run.
    pub fn show_results(&mut self) {
        self.show_results = true;
    }

    /// Opens the add-operation dialog with the first setup, tool and
    /// strategy chosen.
    pub fn begin_add(&mut self, app: &Application) {
        let choices = app.strategy_choices();
        self.adding = Some(AddOperation {
            name: format!("Operation {}", app.project().operations().len() + 1),
            strategy: choices.first().map(|c| c.name.clone()).unwrap_or_default(),
            setup: app.project().setups().first().map(|s| s.id),
            tool: app.project().tools().first().map(|t| t.id),
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
        let choices: Vec<StrategyChoice> = app.strategy_choices();
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
        egui::Window::new("Add operation")
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Name");
                    ui.text_edit_singleline(&mut form.name);
                });
                ui.horizontal(|ui| {
                    ui.label("Strategy");
                    let shown = choices
                        .iter()
                        .find(|c| c.name == form.strategy)
                        .map_or("choose…", |c| c.label.as_str());
                    egui::ComboBox::from_id_salt("add op strategy")
                        .selected_text(shown)
                        .show_ui(ui, |ui| {
                            for c in &choices {
                                ui.selectable_value(&mut form.strategy, c.name.clone(), &c.label);
                            }
                        });
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
                });
                ui.weak("Everything else starts at the strategy's defaults; set them in the operation's panel.");
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(form.setup.is_some(), egui::Button::new("Add"))
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
            match app.add_operation(&form.name, &form.strategy, setup, form.tool) {
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
                                for line in result_lines(run, app.project()) {
                                    let text = RichText::new(&line.text);
                                    ui.label(match line.severity {
                                        Severity::Info => text,
                                        Severity::Warning => text.color(NOTE),
                                        Severity::Problem => text.color(PROBLEM),
                                    });
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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn req_ui_09_an_empty_run_has_no_lines_and_a_new_panel_is_not_stale_for_nothing() {
        let run = Regeneration::default();
        let project = Project::new("empty");
        assert!(result_lines(&run, &project).is_empty());
        let panel = RunPanel::default();
        assert!(!panel.is_running());
        assert!(panel.last.is_none());
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
}
