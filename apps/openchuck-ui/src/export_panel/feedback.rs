// SPDX-License-Identifier: AGPL-3.0-or-later
//! Export-owned in-main-window confirmation/result modals (UI-12/14/16).

use super::*;

fn primary(ui: &mut egui::Ui, label: &str) -> bool {
    ui.add(
        egui::Button::new(RichText::new(label).color(DARK.primary_text).strong())
            .fill(DARK.primary)
            .stroke(egui::Stroke::new(1.0, DARK.primary_border)),
    )
    .clicked()
}

/// A bounded modal with its own fixed action strip, leaving the shared
/// window/dock infrastructure untouched. Content reserves its scrollbar.
fn modal(
    ctx: &egui::Context,
    key: &str,
    title: &str,
    body: impl FnOnce(&mut egui::Ui),
    footer: impl FnOnce(&mut egui::Ui) -> Option<Act>,
) -> Option<Act> {
    let size = (ctx.content_rect().size() - egui::vec2(64.0, 64.0))
        .min(egui::vec2(760.0, 440.0))
        .max(egui::vec2(1.0, 1.0));
    let shown = egui::Modal::new(egui::Id::unique(key)).show(ctx, |ui| {
        ui.set_width(size.x);
        ui.set_height(size.y);
        ui.heading(title);
        let action = egui::Panel::bottom(egui::Id::unique((key, "actions")))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), footer)
                        .inner
                })
                .inner
            })
            .inner;
        crate::theme::solid_scroll_bar(ui);
        egui::ScrollArea::vertical()
            .id_salt((key, "body"))
            .min_scrolled_height(0.0)
            .auto_shrink([false, false])
            .show(ui, body);
        action
    });
    let escape = shown.is_top_modal
        && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
    shown.inner.or_else(|| {
        escape.then_some(if key == "export result" {
            Act::DismissResult
        } else {
            Act::CancelOverwrite
        })
    })
}

impl ExportPanel {
    /// Reopens a retained actual result, without making or writing files.
    pub(crate) fn show_report(&mut self, report: OutputReport) {
        self.overwrite = None;
        self.result = Some(report);
    }

    pub(super) fn feedback_frame(&self, ctx: &egui::Context, acts: &mut Vec<Act>) {
        if let Some(pending) = &self.overwrite
            && let Some(action) = modal(
                ctx,
                "export overwrite",
                "Replace existing files?",
                |ui| {
                    ui.label("These destinations already exist:");
                    for path in pending.plan.existing() {
                        ui.label(path.display().to_string());
                    }
                    ui.label("Replace the listed files? Cancel writes nothing.");
                    ui.label("Files are written in order. If a write fails, earlier completed writes remain.");
                },
                |ui| {
                    if primary(ui, "Replace files") {
                        return Some(Act::Replace);
                    }
                    ui.button("Cancel")
                        .clicked()
                        .then_some(Act::CancelOverwrite)
                },
            )
        {
            acts.push(action);
        }
        if let Some(report) = &self.result
            && let Some(action) = modal(
                ctx,
                "export result",
                "Export result",
                |ui| {
                    ui.strong(report.summary());
                    if !report.written.is_empty() {
                        ui.strong("Written");
                        for path in &report.written {
                            ui.label(path.display().to_string());
                        }
                    }
                    if let Some(failure) = &report.failed {
                        ui.colored_label(PROBLEM, format!("Failed: {}", failure.path.display()));
                        ui.label(&failure.reason);
                    }
                    if !report.not_attempted.is_empty() {
                        ui.strong("Not attempted");
                        for path in &report.not_attempted {
                            ui.label(path.display().to_string());
                        }
                    }
                    ui.separator();
                    ui.label(EXPORT_NOTICE);
                },
                |ui| {
                    if primary(ui, "Dismiss") {
                        return Some(Act::DismissResult);
                    }
                    if ui
                        .add_enabled(!report.written.is_empty(), egui::Button::new("Copy paths"))
                        .clicked()
                    {
                        ui.ctx().copy_text(
                            report
                                .written
                                .iter()
                                .map(|p| p.display().to_string())
                                .collect::<Vec<_>>()
                                .join("\n"),
                        );
                    }
                    None
                },
            )
        {
            acts.push(action);
        }
    }
}
