// SPDX-License-Identifier: AGPL-3.0-or-later
//! The single departure policy. Callers retain their semantic intent; no
//! pointer event or old button eligibility is replayed (decision 0233).

use eframe::egui::{self, Id};

use super::{Action, Shell};
use crate::pending_edits::{self as edits, Mode};

#[derive(Default)]
pub(super) struct Guard {
    action: Option<Action>,
    focus: Option<Id>,
    resolving: bool,
    dispatching: bool,
    pick: Option<super::pending_pick::Intent>,
}

impl Guard {
    pub(super) fn active(&self) -> bool {
        self.action.is_some()
    }
}

impl Shell {
    pub(super) fn resume_pick(&mut self, intent: super::pending_pick::Intent, ctx: &egui::Context) {
        if self.scene_job.is_some() {
            self.edit_guard.pick = Some(intent);
            ctx.request_repaint();
            return;
        }
        if let Some(input) = self.app.as_ref().and_then(|app| {
            intent.resolve(
                app,
                self.viewport.view(),
                self.workholding_panel
                    .as_ref()
                    .map(|panel| (panel.setup(), panel.workholding())),
            )
        }) {
            self.pick_intent(input, ctx);
        } else {
            self.status = "The selected geometry changed; select it again.".to_owned();
        }
    }

    pub(super) fn finish_pick(&mut self, ctx: &egui::Context) {
        if self.scene_job.is_none()
            && let Some(intent) = self.edit_guard.pick.take()
        {
            self.resume_pick(intent, ctx);
        }
    }

    /// Every form-leaving action asks this policy, before changing UI or
    /// starting work. The closure owns IDs and meaning, never coordinates.
    pub(super) fn defer(&mut self, action: impl FnOnce(&mut Self) + 'static) -> bool {
        if self.edit_guard.dispatching {
            return false;
        }
        let Some(ctx) = &self.edit_context else {
            return false;
        };
        let entries = edits::entries(ctx);
        if entries.is_empty() {
            return false;
        }
        if self.edit_guard.action.is_none() {
            self.edit_guard.focus = ctx
                .memory(|m| m.focused())
                .filter(|id| entries.iter().any(|e| e.id == *id))
                .or_else(|| entries.first().map(|e| e.id));
            self.edit_guard.action = Some(Box::new(action));
            edits::set_mode(ctx, Mode::Frozen);
        }
        true
    }

    pub(super) fn pending_dialog(&mut self, ctx: &egui::Context) {
        if self.edit_guard.action.is_none() && edits::local_requested(ctx) {
            self.defer(|_| {});
        }
        if self.edit_guard.action.is_none() {
            return;
        }
        if self.edit_guard.resolving {
            egui::Modal::new(Id::new("unconfirmed changes")).show(ctx, |ui| {
                ui.spinner();
                ui.label("Confirming changes…");
            });
            ctx.request_repaint();
            return;
        }
        let entries = edits::entries(ctx);
        let mut answer = None;
        egui::Modal::new(Id::new("unconfirmed changes")).show(ctx, |ui| {
            ui.heading("Unconfirmed changes");
            ui.label("You have changed these fields but not confirmed them:");
            for entry in &entries {
                ui.label(&entry.label);
            }
            ui.label("Keep the changes or discard them?");
            ui.horizontal(|ui| {
                if ui.button("Keep").clicked() {
                    answer = Some(0);
                }
                if ui.button("Discard").clicked() {
                    answer = Some(1);
                }
                if ui.button("Cancel").clicked() {
                    answer = Some(2);
                }
            });
        });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            answer = Some(2);
        }
        match answer {
            Some(0) => {
                // Preflight the entire form before a widget can return a
                // Commit. In particular an invalid earlier field must stop
                // a valid active field from applying partially.
                if let Err((id, reason)) = edits::validate(&entries) {
                    edits::refuse(ctx, id, reason);
                    edits::focus(ctx, id);
                    self.edit_guard.action = None;
                    edits::resolve_local(ctx, false);
                    edits::set_mode(ctx, Mode::Normal);
                } else {
                    self.edit_guard.resolving = true;
                    if let Some(entry) = entries.first() {
                        edits::set_mode(ctx, Mode::Keep(entry.id));
                    }
                }
            }
            Some(1) => {
                edits::discard(ctx);
                self.edit_guard.resolving = true;
                // Frozen until the next frame has restored all drawn
                // buffers; hidden ones keep a discard token for next draw.
            }
            Some(2) => {
                if let Some(id) = self.edit_guard.focus {
                    edits::focus(ctx, id);
                }
                self.edit_guard.action = None;
                edits::resolve_local(ctx, false);
                edits::set_mode(ctx, Mode::Normal);
            }
            _ => {}
        }
        ctx.request_repaint();
    }

    /// Called after fields and their existing command handlers have drawn.
    pub(super) fn finish_pending(&mut self, ctx: &egui::Context) {
        if !self.edit_guard.resolving {
            return;
        }
        let entries = edits::entries(ctx);
        if let Some(entry) = entries.first() {
            if let Err(reason) = &entry.checked {
                edits::refuse(ctx, entry.id, reason.clone());
                edits::focus(ctx, entry.id);
                self.edit_guard.action = None;
                edits::resolve_local(ctx, false);
                self.edit_guard.resolving = false;
                edits::set_mode(ctx, Mode::Normal);
            } else {
                edits::set_mode(ctx, Mode::Keep(entry.id));
                ctx.request_repaint();
            }
            return;
        }
        self.edit_guard.resolving = false;
        edits::set_mode(ctx, Mode::Normal);
        edits::resolve_local(ctx, true);
        if let Some(action) = self.edit_guard.action.take() {
            // A commit changes both eligibility and the primary button's
            // meaning. Refresh from the committed project, then dispatch.
            self.overview_frame(ctx);
            self.edit_guard.dispatching = true;
            action(self);
            self.edit_guard.dispatching = false;
        }
    }
}
