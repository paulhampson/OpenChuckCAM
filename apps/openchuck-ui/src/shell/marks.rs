// SPDX-License-Identifier: AGPL-3.0-or-later
//! The status marks of the project tree and their legend (UI-11;
//! specification section 6): each state has a shape as well as a colour, so
//! no state is shown by colour alone (NFR-15).

use std::f32::consts::{FRAC_PI_2, TAU};

use eframe::egui::{self, Color32, Pos2, Stroke, epaint};
use openchuck_app::overview::{ItemState, Severity};

use crate::theme::DARK;

/// The side of a mark's square, in points.
pub const SIZE: f32 = 10.0;

/// Draws the mark of `state` centred on `centre`, at `size` across.
pub fn paint(painter: &egui::Painter, centre: Pos2, state: ItemState, size: f32) {
    let r = size / 2.0;
    match state {
        ItemState::UpToDate => {
            painter.circle_filled(centre, r, DARK.ok);
        }
        ItemState::Stale => {
            let d = r * 0.9;
            painter.add(egui::Shape::convex_polygon(
                vec![
                    centre + egui::vec2(0.0, -d),
                    centre + egui::vec2(d, 0.0),
                    centre + egui::vec2(0.0, d),
                    centre + egui::vec2(-d, 0.0),
                ],
                DARK.warning,
                Stroke::NONE,
            ));
        }
        ItemState::Running => {
            // A ring with a gap.
            let points: Vec<Pos2> = (0..=20)
                .map(|i| {
                    #[allow(clippy::cast_precision_loss)]
                    let a = -FRAC_PI_2 + (i as f32 / 20.0) * TAU * 0.75;
                    centre + r * 0.8 * egui::vec2(a.cos(), a.sin())
                })
                .collect();
            painter.add(egui::Shape::line(points, Stroke::new(2.0, DARK.accent)));
        }
        ItemState::Warning => {
            painter.add(egui::Shape::closed_line(
                triangle(centre, r),
                Stroke::new(1.4, DARK.warning),
            ));
        }
        ItemState::Error => {
            painter.add(egui::Shape::convex_polygon(
                triangle(centre, r),
                DARK.error,
                Stroke::NONE,
            ));
        }
        ItemState::Off => {
            painter.circle_stroke(centre, r * 0.8, Stroke::new(1.5, DARK.muted));
        }
    }
}

fn triangle(centre: Pos2, r: f32) -> Vec<Pos2> {
    vec![
        centre + egui::vec2(0.0, -r),
        centre + egui::vec2(r, r * 0.8),
        centre + egui::vec2(-r, r * 0.8),
    ]
}

/// A mark as a widget of its own size, with the cause on hover and on
/// keyboard focus (specification section 16).
pub fn mark(ui: &mut egui::Ui, state: ItemState, cause: &str) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(SIZE + 4.0, SIZE + 4.0), egui::Sense::hover());
    paint(ui.painter(), rect.center(), state, SIZE);
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, cause.to_owned()));
    response.on_hover_text(cause)
}

/// The severity icon of a message: a red triangle, an amber triangle
/// outline, or a blue "i" (specification section 9).
pub fn severity(ui: &mut egui::Ui, severity: Severity) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(SIZE + 6.0, SIZE + 6.0), egui::Sense::hover());
    let painter = ui.painter();
    let c = rect.center();
    match severity {
        Severity::Error => paint(painter, c, ItemState::Error, SIZE + 2.0),
        Severity::Warning => paint(painter, c, ItemState::Warning, SIZE + 2.0),
        Severity::Info => {
            painter.circle_stroke(c, SIZE / 2.0 + 1.0, Stroke::new(1.4, DARK.accent));
            painter.text(
                c,
                egui::Align2::CENTER_CENTER,
                "i",
                egui::FontId::proportional(9.0),
                DARK.accent,
            );
        }
    }
    response.on_hover_text(severity.name())
}

/// A green tick, for a passed check.
pub fn tick(ui: &mut egui::Ui) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(SIZE + 6.0, SIZE + 6.0), egui::Sense::hover());
    let c = rect.center();
    ui.painter().add(egui::Shape::line(
        vec![
            c + egui::vec2(-4.5, 0.5),
            c + egui::vec2(-1.5, 3.5),
            c + egui::vec2(4.5, -3.5),
        ],
        Stroke::new(1.6, DARK.ok),
    ));
    response
}

/// The legend of the marks, pinned under the tree (specification section 6).
pub fn legend(ui: &mut egui::Ui) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        for state in ItemState::ALL {
            mark(ui, state, state.name());
            ui.label(egui::RichText::new(state.name()).small().color(DARK.muted));
            ui.add_space(6.0);
        }
    });
}

/// A thin progress bar under a running row (specification section 6).
pub fn progress_bar(ui: &egui::Ui, row: egui::Rect, fraction: f32) {
    let bar = egui::Rect::from_min_max(
        egui::pos2(row.left(), row.bottom() - 3.0),
        egui::pos2(
            row.left() + row.width() * fraction.clamp(0.0, 1.0),
            row.bottom(),
        ),
    );
    ui.painter()
        .add(epaint::RectShape::filled(bar, 0.0, DARK.accent));
}

/// The colour that goes with a severity, for chips and counts.
pub fn colour(severity: Severity) -> Color32 {
    match severity {
        Severity::Error => DARK.error,
        Severity::Warning => DARK.warning,
        Severity::Info => DARK.accent,
    }
}
