// SPDX-License-Identifier: AGPL-3.0-or-later
//! The dock under the view (specification section 9): Messages, Selection
//! sets and Jobs on every step, Setup checks on Setup and Playback on
//! Simulate. Messages collect what the application finds, by severity, each
//! naming its source and offering the action that fixes it (UI-12).

use std::time::Duration;

use eframe::egui::{self, RichText};
use openchuck_app::JobOutcome;
use openchuck_app::overview::{Severity, Step};

use super::{Dialog, Selection, Shell, marks, palette};

/// A tab of the dock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DockTab {
    Messages,
    SelectionSets,
    Jobs,
    SetupChecks,
    Playback,
}

impl DockTab {
    fn name(self) -> &'static str {
        match self {
            Self::Messages => "Messages",
            Self::SelectionSets => "Selection sets",
            Self::Jobs => "Jobs",
            Self::SetupChecks => "Setup checks",
            Self::Playback => "Playback",
        }
    }
}

/// "1 min 05 s", "4.2 s": a job's time.
fn duration(d: Duration) -> String {
    let s = d.as_secs_f64();
    if s < 60.0 {
        format!("{s:.1} s")
    } else {
        let whole = d.as_secs();
        format!("{} min {:02} s", whole / 60, whole % 60)
    }
}

impl Shell {
    /// The tabs this step offers.
    fn tabs(&self) -> Vec<DockTab> {
        let mut tabs = vec![DockTab::Messages, DockTab::SelectionSets, DockTab::Jobs];
        match self.step {
            Step::Setup => tabs.insert(0, DockTab::SetupChecks),
            Step::Simulate => tabs.push(DockTab::Playback),
            _ => {}
        }
        tabs
    }

    /// The tab strip, with the error and warning counts on Messages.
    pub(super) fn dock_tabs(&mut self, ui: &mut egui::Ui) {
        let tabs = self.tabs();
        if !tabs.contains(&self.dock_tab) {
            self.dock_tab = DockTab::Messages;
        }
        for tab in tabs {
            let mut text = tab.name().to_owned();
            match tab {
                DockTab::Messages => {
                    let n = self.overview.count(Severity::Error)
                        + self.overview.count(Severity::Warning);
                    if n > 0 {
                        text.push_str(&format!(" ({n})"));
                    }
                }
                DockTab::Jobs => {
                    let n = self.jobs.running().len();
                    if n > 0 {
                        text.push_str(&format!(" ({n})"));
                    }
                }
                DockTab::SetupChecks => {
                    let n = self
                        .overview
                        .setup_checks
                        .iter()
                        .filter(|c| !c.passed && Some(c.setup) == self.setup)
                        .count();
                    if n > 0 {
                        text.push_str(&format!(" ({n})"));
                    }
                }
                _ => {}
            }
            if ui.selectable_label(self.dock_tab == tab, text).clicked() {
                self.dock_tab = tab;
                if self.settings.dock.collapsed {
                    self.toggle_region(super::frame::Region::Dock);
                }
            }
        }
    }

    /// The dock's contents.
    pub(super) fn dock(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            self.dock_tabs(ui);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .small_button("﹀")
                    .on_hover_text("Collapse the dock (Ctrl 3)")
                    .clicked()
                {
                    self.toggle_region(super::frame::Region::Dock);
                }
            });
        });
        ui.separator();
        match self.dock_tab {
            DockTab::Messages => self.messages_tab(ui),
            DockTab::SelectionSets => self.sets_tab(ui),
            DockTab::Jobs => self.jobs_tab(ui),
            DockTab::SetupChecks => self.checks_tab(ui),
            DockTab::Playback => self.playback_tab(ui),
        }
    }

    /// Messages: filter chips with counts, then one row per message
    /// (specification section 9).
    fn messages_tab(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            for (on, severity) in self.message_filter.iter_mut().zip(Severity::ALL) {
                let count = self.overview.count(severity);
                let text = RichText::new(format!("{} {count}", severity.name())).color(if *on {
                    marks::colour(severity)
                } else {
                    palette::MUTED
                });
                if ui
                    .selectable_label(*on, text)
                    .on_hover_text(format!("Show or hide {}", severity.name().to_lowercase()))
                    .clicked()
                {
                    *on = !*on;
                }
            }
        });
        let mut act = None;
        let mut show = None;
        egui::ScrollArea::vertical()
            .id_salt("messages")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if self.app.is_some() && self.overview.messages.is_empty() {
                    ui.label(RichText::new("No messages.").color(palette::MUTED));
                }
                for (index, m) in self.overview.messages.iter().enumerate() {
                    let shown = match m.severity {
                        Severity::Error => self.message_filter[0],
                        Severity::Warning => self.message_filter[1],
                        Severity::Info => self.message_filter[2],
                    };
                    if !shown {
                        continue;
                    }
                    let highlighted = self.highlight == Some(index);
                    let frame = if highlighted {
                        egui::Frame::new()
                            .fill(palette::SELECTED)
                            .inner_margin(egui::Margin::symmetric(4, 2))
                    } else {
                        egui::Frame::new().inner_margin(egui::Margin::symmetric(4, 2))
                    };
                    let response = frame
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                marks::severity(ui, m.severity);
                                ui.add(egui::Label::new(&m.text).wrap());
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if let Some(action) = m.action
                                            && ui.link(action.label()).clicked()
                                        {
                                            act = Some(action);
                                        }
                                        if let Some(source) = m.source
                                            && ui
                                                .link(&m.source_name)
                                                .on_hover_text("Select it")
                                                .clicked()
                                        {
                                            show = Some(source);
                                        }
                                    },
                                );
                            });
                        })
                        .response;
                    if highlighted {
                        response.scroll_to_me(Some(egui::Align::Center));
                    }
                }
            });
        if let Some(item) = show {
            self.show_item(item);
        }
        if let Some(action) = act {
            self.message_action(action);
        }
    }

    /// Selection sets: the table, the save and edit controls (section 12.2;
    /// its rework is lane X6's).
    fn sets_tab(&mut self, ui: &mut egui::Ui) {
        let Some(app) = &mut self.app else {
            ui.label(RichText::new("No project open.").color(palette::MUTED));
            return;
        };
        let summaries = app.selection_summaries(&[]);
        if let Some(Selection::Set(id)) = self.selected
            && !summaries.iter().any(|s| s.id == id)
        {
            self.selected = Some(Selection::Picking);
        }
        let error = egui::ScrollArea::vertical()
            .id_salt("selection sets")
            .show(ui, |ui| {
                self.selection
                    .show(ui, app, &summaries, self.viewport.view())
            })
            .inner;
        if let Some(error) = error {
            self.dialog = Some(Dialog::Error(error));
        }
    }

    /// Jobs: running jobs with progress, time and Cancel; finished jobs with
    /// their outcome (UI-09; specification section 9).
    fn jobs_tab(&mut self, ui: &mut egui::Ui) {
        let mut cancel = None;
        egui::ScrollArea::vertical()
            .id_salt("jobs")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if self.jobs.running().is_empty() && self.jobs.finished().next().is_none() {
                    ui.label(RichText::new("No jobs yet.").color(palette::MUTED));
                }
                for entry in self.jobs.running() {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(&entry.label);
                        if let Some(f) = entry.fraction() {
                            ui.add(
                                egui::ProgressBar::new(f)
                                    .desired_width(120.0)
                                    .show_percentage(),
                            );
                        }
                        ui.label(
                            RichText::new(duration(entry.started.elapsed())).color(palette::MUTED),
                        );
                        if ui.small_button("Cancel").clicked() {
                            cancel = Some(entry.id);
                        }
                    });
                }
                for entry in self.jobs.finished() {
                    ui.horizontal(|ui| {
                        match &entry.outcome {
                            Some(JobOutcome::Failed(e)) => {
                                marks::severity(ui, Severity::Error);
                                ui.label(format!("{}: failed. {e}", entry.label));
                            }
                            Some(JobOutcome::Cancelled) => {
                                ui.add_space(marks::SIZE + 6.0);
                                ui.label(format!("{}: cancelled", entry.label));
                            }
                            Some(JobOutcome::Done(summary)) => {
                                marks::tick(ui);
                                match summary {
                                    Some(s) => ui.label(format!("{}: {s}", entry.label)),
                                    None => ui.label(format!("{}: done", entry.label)),
                                };
                            }
                            None => {}
                        }
                        if let Some(took) = entry.took {
                            ui.label(RichText::new(duration(took)).color(palette::MUTED));
                        }
                    });
                }
            });
        if let (Some(id), Some(app)) = (cancel, &self.app) {
            app.cancel_job(id);
        }
    }

    /// Setup checks: one row per check of the chosen setup, a tick or an
    /// amber triangle (SET-06; specification section 9).
    fn checks_tab(&mut self, ui: &mut egui::Ui) {
        let mut act = None;
        egui::ScrollArea::vertical()
            .id_salt("setup checks")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let rows: Vec<_> = self
                    .overview
                    .setup_checks
                    .iter()
                    .filter(|c| Some(c.setup) == self.setup)
                    .collect();
                if rows.is_empty() {
                    let text = if self.overview.checks_current {
                        "There is no setup to check."
                    } else {
                        "Checking…"
                    };
                    ui.label(RichText::new(text).color(palette::MUTED));
                }
                for check in rows {
                    ui.horizontal(|ui| {
                        if check.passed {
                            marks::tick(ui);
                        } else {
                            marks::severity(ui, Severity::Warning);
                        }
                        ui.add(egui::Label::new(&check.text).wrap());
                        if let Some(action) = check.action {
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.link(action.label()).clicked() {
                                        act = Some(action);
                                    }
                                },
                            );
                        }
                    });
                }
            });
        if let Some(action) = act {
            self.message_action(action);
        }
    }

    /// Playback: the transport and timeline (SIM-04; its rework is lane
    /// X6's).
    fn playback_tab(&mut self, ui: &mut egui::Ui) {
        match &self.app {
            Some(app) if self.play.has_run() => {
                egui::ScrollArea::vertical()
                    .id_salt("playback")
                    .show(ui, |ui| self.play.show(ui, app));
            }
            _ => {
                ui.label(
                    RichText::new("Run the simulation to play the cut.").color(palette::MUTED),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn req_ui_09_job_times_read_plainly() {
        assert_eq!(duration(Duration::from_millis(4200)), "4.2 s");
        assert_eq!(duration(Duration::from_secs(65)), "1 min 05 s");
    }
}
