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

/// Which entries of a wrapped row of `widths` start a new row in `available` width, so that
/// an entry (a mark with its word) is never split from its word at a row's end (OI-530).
/// `gap` is the space between entries.
fn row_starts(widths: &[f32], available: f32, gap: f32) -> Vec<bool> {
    let mut used = 0.0;
    widths
        .iter()
        .map(|w| {
            let wraps = used > 0.0 && used + w > available;
            used = if wraps { *w } else { used + w } + gap;
            wraps
        })
        .collect()
}

/// The legend of the marks, pinned under the tree (specification section 6). Each mark stays
/// with its word when the row wraps.
pub fn legend(ui: &mut egui::Ui) {
    let font = egui::TextStyle::Small.resolve(ui.style());
    let widths: Vec<f32> = ItemState::ALL
        .iter()
        .map(|state| {
            let word = ui
                .painter()
                .layout_no_wrap(state.name().to_owned(), font.clone(), DARK.muted)
                .size()
                .x;
            SIZE + 4.0 + 4.0 + word + 6.0
        })
        .collect();
    let starts = row_starts(&widths, ui.available_width(), 4.0);
    // Rows of whole entries: a row never wraps inside an entry, and the word
    // of an entry does not wrap either (issue 212, F-56).
    let mut rows: Vec<Vec<ItemState>> = Vec::new();
    for (state, wraps) in ItemState::ALL.into_iter().zip(starts) {
        match rows.last_mut() {
            Some(row) if !wraps => row.push(state),
            _ => rows.push(vec![state]),
        }
    }
    for row in rows {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            for state in row {
                mark(ui, state, state.name());
                ui.add(
                    egui::Label::new(egui::RichText::new(state.name()).small().color(DARK.muted))
                        .extend(),
                );
                ui.add_space(6.0);
            }
        });
    }
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

/// The colour a state's mark is drawn in.
pub fn colour_of(state: ItemState) -> Color32 {
    match state {
        ItemState::UpToDate => DARK.ok,
        ItemState::Stale | ItemState::Warning => DARK.warning,
        ItemState::Running => DARK.accent,
        ItemState::Error => DARK.error,
        ItemState::Off => DARK.muted,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn req_ui_11_the_legend_wraps_between_entries_never_inside_one() {
        // OI-530: an entry is a mark and its word; the row breaks before an entry that does
        // not fit whole, so a word never lands on the next row apart from its mark.
        let widths = [60.0, 50.0, 70.0, 40.0, 60.0, 30.0];
        let starts = row_starts(&widths, 200.0, 4.0);
        assert_eq!(starts, [false, false, false, true, false, false]);
        // Every row holds what fits.
        let mut used = 0.0;
        for (w, wraps) in widths.iter().zip(&starts) {
            if *wraps {
                used = 0.0;
            }
            used += w + 4.0;
            assert!(used - 4.0 <= 200.0, "{used}");
        }
        // A single entry wider than the row stays on its own row, never skipped.
        assert_eq!(row_starts(&[300.0, 20.0], 200.0, 4.0), [false, true]);
        assert!(row_starts(&[], 200.0, 4.0).is_empty());
    }

    #[test]
    fn req_ui_11_the_legend_at_the_tree_width_keeps_each_word_inside_the_panel() {
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::pos2(0.0, 0.0),
                    egui::vec2(240.0, 200.0),
                )),
                ..Default::default()
            },
            legend,
        );
        output.textures_delta.clear();
        let mut words = Vec::new();
        for clipped in &output.shapes {
            if let egui::Shape::Text(t) = &clipped.shape {
                words.push((t.galley.text().to_owned(), t.pos.x + t.galley.size().x));
            }
        }
        for state in ItemState::ALL {
            let (_, right) = words
                .iter()
                .find(|(t, _)| t == state.name())
                .unwrap_or_else(|| panic!("{} is drawn", state.name()));
            assert!(*right <= 240.0, "{} ends at {right}", state.name());
        }
    }

    /// Issue 212 (F-56): at every width the tree can have (200 to 400, less
    /// its margins) each mark is drawn on the same row as its word and just
    /// before it, never at the end of one row with the word on the next.
    #[test]
    fn req_ui_11_each_legend_mark_stays_on_the_row_of_its_word() {
        fn leaves<'a>(shape: &'a egui::Shape, out: &mut Vec<&'a egui::Shape>) {
            match shape {
                egui::Shape::Vec(v) => v.iter().for_each(|s| leaves(s, out)),
                other => out.push(other),
            }
        }
        for width in (184..=400).step_by(8) {
            let ctx = egui::Context::default();
            crate::theme::install_style(&ctx, crate::theme::Theme::Dark);
            let input = || egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::pos2(0.0, 0.0),
                    egui::vec2(width as f32, 300.0),
                )),
                ..Default::default()
            };
            // The fonts are installed for the next frame.
            ctx.run_ui(input(), legend).textures_delta.clear();
            let mut output = ctx.run_ui(input(), legend);
            output.textures_delta.clear();
            let mut all = Vec::new();
            for clipped in &output.shapes {
                leaves(&clipped.shape, &mut all);
            }
            for state in ItemState::ALL {
                let at = all
                    .iter()
                    .position(
                        |s| matches!(s, egui::Shape::Text(t) if t.galley.text() == state.name()),
                    )
                    .unwrap_or_else(|| panic!("{} is drawn at {width}", state.name()));
                let egui::Shape::Text(word) = all[at] else {
                    unreachable!()
                };
                let word = egui::Rect::from_min_size(word.pos, word.galley.size());
                assert!(
                    word.height() < 20.0,
                    "{} wraps inside its own entry at {width}: {word:?}",
                    state.name()
                );
                // The mark is the shape drawn just before its word.
                let mark = all[at - 1].visual_bounding_rect();
                assert!(
                    (mark.center().y - word.center().y).abs() < 6.0,
                    "{} mark at {mark:?} and word at {word:?}, width {width}",
                    state.name()
                );
                assert!(
                    mark.right() <= word.left() + 1.0,
                    "{} mark {mark:?} word {word:?} at {width}",
                    state.name()
                );
            }
        }
    }
}
