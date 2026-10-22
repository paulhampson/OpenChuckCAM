// SPDX-License-Identifier: AGPL-3.0-or-later
//! The dock under the view (specification section 9): Messages, Selection
//! sets and Jobs on every step, Setup checks on Setup and Playback on
//! Simulate. Messages collect what the application finds, by severity, each
//! naming its source and offering the action that fixes it (UI-12).

use std::time::Duration;

use eframe::egui::{self, RichText};
use openchuck_app::JobOutcome;
use openchuck_app::overview::{Severity, Step};

use super::{Command, Shell, marks};
use crate::theme::DARK;

/// A tab of the dock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DockTab {
    Messages,
    Jobs,
    SetupChecks,
    Playback,
}

impl DockTab {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Messages => "Messages",
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
    pub(super) fn tabs(&self) -> Vec<DockTab> {
        let mut tabs = vec![DockTab::Messages, DockTab::Jobs];
        match self.step {
            Step::Setup => tabs.insert(0, DockTab::SetupChecks),
            Step::Simulate => tabs.push(DockTab::Playback),
            _ => {}
        }
        tabs
    }

    /// Opens the Jobs tab, opening the dock if it was collapsed (the job in
    /// the status bar, issue 201).
    pub(super) fn show_jobs(&mut self) {
        self.dock_tab = DockTab::Jobs;
        if self.collapsed(super::frame::Region::Dock) {
            self.toggle_region(super::frame::Region::Dock);
        }
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
        // The panel's frame is drawn as tall as what is in it: a tab with
        // little in it would leave the rest of the dock undrawn (the black
        // area of issue 259) and its size would change from frame to frame
        // (the playback box moving, issue 215). The contents take the whole
        // panel.
        ui.set_min_size(ui.available_size());
        ui.horizontal(|ui| {
            self.dock_tabs(ui);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .small_button("\u{23f7}")
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
                    DARK.muted
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
                    ui.label(RichText::new("No messages.").color(DARK.muted));
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
                            .fill(DARK.selected)
                            .inner_margin(egui::Margin::symmetric(4, 2))
                    } else {
                        egui::Frame::new().inner_margin(egui::Margin::symmetric(4, 2))
                    };
                    let source_name = m.source.map(|_| m.source_name.as_str());
                    let response = frame
                        .show(ui, |ui| {
                            let clicked = row_with_links(
                                ui,
                                |ui| {
                                    marks::severity(ui, m.severity);
                                },
                                &m.text,
                                [source_name, m.action.map(|a| a.label())],
                            );
                            match clicked {
                                Some(0) => show = m.source,
                                Some(_) => act = m.action,
                                None => {}
                            }
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

    /// Jobs: running jobs with progress, time and Cancel; finished jobs with
    /// their outcome (UI-09; specification section 9).
    fn jobs_tab(&mut self, ui: &mut egui::Ui) {
        let mut cancel = None;
        // The operation the pipeline works on, as the status bar says it
        // (decision 0190 item 5).
        let activity = self
            .app
            .as_ref()
            .and_then(openchuck_app::Application::pipeline_activity);
        egui::ScrollArea::vertical()
            .id_salt("jobs")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if self.jobs.running().is_empty() && self.jobs.finished().next().is_none() {
                    ui.label(RichText::new("No jobs yet.").color(DARK.muted));
                }
                for entry in self.jobs.running() {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        match &activity {
                            Some(a) if matches!(entry.name.as_str(), "regenerate" | "simulate") => {
                                ui.label(format!("{}: {a}", entry.label))
                            }
                            _ => ui.label(&entry.label),
                        };
                        if let Some(f) = entry.fraction() {
                            ui.add(
                                egui::ProgressBar::new(f)
                                    .desired_width(120.0)
                                    .show_percentage(),
                            );
                        }
                        ui.label(
                            RichText::new(duration(entry.started.elapsed())).color(DARK.muted),
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
                            ui.label(RichText::new(duration(took)).color(DARK.muted));
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
                    ui.label(RichText::new(text).color(DARK.muted));
                }
                for check in rows {
                    let mark = |ui: &mut egui::Ui| {
                        if check.passed {
                            marks::tick(ui);
                        } else {
                            marks::severity(ui, Severity::Warning);
                        }
                    };
                    let label = check.action.map(|a| a.label());
                    if row_with_links(ui, mark, &check.text, [label, None]) == Some(0) {
                        act = check.action;
                    }
                }
            });
        if let Some(action) = act {
            self.message_action(action);
        }
    }

    /// Playback: the transport and timeline (SIM-04; its rework is lane
    /// X6's).
    fn playback_tab(&mut self, ui: &mut egui::Ui) {
        // The project's stock simulation choice (issue 214; decision record
        // 0168) is in the tab with or without a run, as one undoable step.
        // A result out of date says so here, with the way to simulate again,
        // above the controls it still has or in place of the ones it lost
        // (issues 259 and 260).
        let stale = self.out_of_date.reason().is_some();
        let again = self.app.is_some() && self.out_of_date_banner(ui);
        let skip = match &self.app {
            Some(app) if self.play.has_run() => {
                // The transport and the timeline stay in a strip that does
                // not scroll; the rest scrolls under it, with a bar in a strip
                // of its own so it never lies over a control (issue 262).
                ui.scope(|ui| {
                    if stale {
                        ui.multiply_opacity(super::out_of_date::STALE_OPACITY);
                    }
                    let skip = self.play.show_strip(ui, app);
                    crate::theme::solid_scroll_bar(ui);
                    egui::ScrollArea::vertical()
                        .id_salt("playback")
                        .auto_shrink([false, true])
                        // However little room the strip and the banner
                        // leave, the rest scrolls in it: the dock does not
                        // grow past its height (issue 262).
                        .min_scrolled_height(0.0)
                        .show(ui, |ui| self.play.show_rest(ui, app));
                    skip
                })
                .inner
            }
            Some(app) => {
                let skip = crate::playback_panel::waiting_row(ui, app);
                if !stale {
                    ui.label(
                        RichText::new("Run the simulation to play the cut.").color(DARK.muted),
                    );
                }
                skip
            }
            None => {
                ui.label(RichText::new("Run the simulation to play the cut.").color(DARK.muted));
                None
            }
        };
        if again {
            self.primary();
        }
        if let Some(skip) = skip {
            self.tree_action(super::TreeAction::Command(Box::new(
                Command::SetSkipStockSimulation(skip),
            )));
        }
    }
}

/// One row of a dock list: a mark, a sentence that wraps, and up to two links after it. The
/// sentence gets the width the links leave, so a link never lies over the wrapped text
/// (UI-16; OI-525). Returns the index of the link clicked.
fn row_with_links(
    ui: &mut egui::Ui,
    mark: impl FnOnce(&mut egui::Ui),
    text: &str,
    links: [Option<&str>; 2],
) -> Option<usize> {
    let font = egui::TextStyle::Body.resolve(ui.style());
    let spacing = ui.spacing().item_spacing.x;
    let links_width: f32 = links
        .iter()
        .flatten()
        .map(|l| {
            ui.painter()
                .layout_no_wrap((*l).to_owned(), font.clone(), DARK.muted)
                .size()
                .x
                + spacing
        })
        .sum();
    let mut clicked = None;
    ui.horizontal(|ui| {
        mark(ui);
        let width = (ui.available_width() - links_width - spacing).max(60.0);
        ui.allocate_ui_with_layout(
            egui::vec2(width, 0.0),
            egui::Layout::left_to_right(egui::Align::Min),
            |ui| {
                ui.set_max_width(width);
                ui.add(egui::Label::new(text).wrap());
            },
        );
        for (index, link) in links.iter().enumerate() {
            if let Some(link) = link
                && ui.link(*link).clicked()
            {
                clicked = Some(index);
            }
        }
    });
    clicked
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
