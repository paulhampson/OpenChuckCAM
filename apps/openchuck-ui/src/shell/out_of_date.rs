// SPDX-License-Identifier: AGPL-3.0-or-later
//! A Simulate result that no longer matches the project (issue 260; NFR-12,
//! UI-11; decision record 0192). While the latest run is out of date, the
//! viewport carries the single actionable banner (issue 297), with
//! the stale mark as its shape, the reason in plain words and the Simulate
//! step's guarded primary action as its button, so a stale result never looks
//! current and the user can simulate again from where they are. The test is
//! the one the export check uses ([`openchuck_app::simulation_out_of_date`]).

use std::sync::Arc;

use eframe::egui::{self, RichText, Stroke};
use openchuck_app::Regeneration;
use openchuck_app::overview::{ItemState, PrimaryKind, Step};

use super::{Shell, frame, marks};
use crate::theme::DARK;

/// The banner's title.
pub(super) const TITLE: &str = "Simulation out of date";

/// How opaque the numbers and controls of an out-of-date result are drawn:
/// dimmed, so they are not read as current, but still legible to compare.
pub(super) const STALE_OPACITY: f32 = 0.55;

/// What the out-of-date test last found: for the project revision and the
/// run it looked at, the reason, if any. Worked out again only when either
/// changes, not every frame.
#[derive(Default)]
pub(super) struct OutOfDate {
    seen: Option<(u64, Option<Arc<Regeneration>>)>,
    reason: Option<String>,
}

impl OutOfDate {
    /// Why the latest run no longer matches the project, if it does not.
    pub(super) fn reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

impl Shell {
    /// Brings the out-of-date test up to date with the project and the
    /// latest run.
    pub(super) fn follow_out_of_date(&mut self) {
        let Some(app) = &self.app else {
            self.out_of_date = OutOfDate::default();
            return;
        };
        let run = self.run.last();
        let fresh = self
            .out_of_date
            .seen
            .as_ref()
            .is_some_and(|(revision, seen)| {
                *revision == app.revision()
                    && match (seen, run) {
                        (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                        (None, None) => true,
                        _ => false,
                    }
            });
        if fresh {
            return;
        }
        self.out_of_date = OutOfDate {
            seen: Some((app.revision(), run.cloned())),
            reason: run.and_then(|r| openchuck_app::simulation_out_of_date(app.project(), r)),
        };
    }

    /// The banner, where the result is shown. Returns true when its button,
    /// the Simulate step's primary action, was clicked: the caller runs it
    /// with [`Shell::primary`]. Draws nothing while the result is current.
    pub(super) fn out_of_date_banner(&self, ui: &mut egui::Ui) -> bool {
        self.banner().is_some_and(|banner| banner.show(ui))
    }

    /// What the viewport banner says while the result is out of date.
    pub(super) fn banner(&self) -> Option<Banner> {
        let reason = self.out_of_date.reason()?.to_owned();
        let primary = self.overview.primary(Step::Simulate);
        let runs = matches!(
            primary.kind,
            PrimaryKind::RunSimulation
                | PrimaryKind::GenerateAndSimulate(_)
                | PrimaryKind::GenerateStale(_)
                | PrimaryKind::Generate
        );
        // Skipping the stock simulation, the button only generates the
        // toolpaths (decision 0168), and the banner says so.
        let toolpaths_only = matches!(
            primary.kind,
            PrimaryKind::GenerateStale(_) | PrimaryKind::Generate
        );
        Some(Banner {
            reason,
            label: if toolpaths_only {
                primary.label
            } else {
                "Run simulation again".to_owned()
            },
            blocked: primary.blocked,
            runs,
            toolpaths_only,
            running: self.run.is_running(),
        })
    }

    /// The banner over the top of the 3D view on Simulate, on the result it
    /// is about (the owner's answer on issue 260).
    pub(super) fn view_banner(&mut self, ctx: &egui::Context) {
        if self.step != Step::Simulate || self.out_of_date.reason().is_none() {
            return;
        }
        let rect = self.viewport.last_rect;
        if !rect.is_positive() {
            return;
        }
        let again = egui::Area::new(egui::Id::unique("out of date banner"))
            .order(egui::Order::Middle)
            .pivot(egui::Align2::CENTER_TOP)
            .fixed_pos(rect.center_top() + egui::vec2(0.0, 12.0))
            .show(ctx, |ui| {
                ui.set_max_width(rect.width().min(420.0) - 16.0);
                self.out_of_date_banner(ui)
            })
            .inner;
        if again {
            self.primary();
        }
    }
}

/// The out-of-date banner's contents (see [`Shell::banner`]).
pub(super) struct Banner {
    reason: String,
    label: String,
    blocked: Option<String>,
    runs: bool,
    toolpaths_only: bool,
    running: bool,
}

impl Banner {
    /// Draws the banner. Returns true when its button was clicked.
    pub(super) fn show(&self, ui: &mut egui::Ui) -> bool {
        let Banner {
            reason,
            label,
            blocked,
            runs,
            toolpaths_only,
            running,
        } = self;
        let (runs, toolpaths_only, running) = (*runs, *toolpaths_only, *running);
        let boxed = DARK.warning_box;
        let mut clicked = false;
        egui::Frame::new()
            .fill(boxed.bg)
            .stroke(Stroke::new(1.0, boxed.border))
            .corner_radius(crate::theme::RADIUS)
            .inner_margin(egui::Margin::same(8))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    marks::mark(ui, ItemState::Stale, reason);
                    ui.label(RichText::new(TITLE).strong().color(boxed.text));
                });
                let next = match (running, runs, toolpaths_only) {
                    (true, _, true) => " Generating the toolpaths again…",
                    (true, _, false) => " Simulating again…",
                    (false, true, true) => " Generate the toolpaths again to see the effect.",
                    (false, true, false) => " Simulate again to see the effect.",
                    (false, false, _) => " Turn on an operation to simulate again.",
                };
                ui.add(
                    egui::Label::new(RichText::new(format!("{reason}{next}")).color(boxed.text))
                        .wrap(),
                );
                if runs {
                    let enabled = blocked.is_none() && !running;
                    clicked = frame::primary_button(ui, label, enabled)
                        .on_disabled_hover_text(blocked.as_deref().unwrap_or("Running"))
                        .clicked();
                }
            });
        clicked
    }
}
