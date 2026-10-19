// SPDX-License-Identifier: AGPL-3.0-or-later
//! The custom-profile editor with live preview (TL-03; work breakdown slice 8
//! item 5; decision record 0029 item 12). The profile being edited is held
//! here; every edit is checked by `openchuck-app` as the file import checks
//! it, and the preview turns red with the reason while it is invalid. Only
//! Apply changes the project, as one undoable step.

use eframe::egui::{self, Color32, Pos2, Sense, Stroke, pos2, vec2};
use openchuck_app::tool_edit::{
    check_profile, insert_segment, profile_outline, remove_segment, set_segment_arc,
};
use openchuck_app::tools::{ProfilePoint, ProfileSegment, Tool, ToolProfile};

use crate::theme::DARK;

/// What the user chose in the editor this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileAction {
    /// Keep editing.
    None,
    /// Apply the profile to the tool.
    Apply,
    /// Drop the edits.
    Cancel,
}

/// A change to the segment list, applied after drawing it.
#[derive(Debug, Clone, Copy)]
enum Change {
    Arc(usize, bool),
    Insert(Option<usize>),
    Remove(usize),
}

const VALID: Color32 = DARK.ok;
const INVALID: Color32 = DARK.error;

/// Draws the editor for `profile`, a draft of `tool`'s profile.
pub fn editor(ui: &mut egui::Ui, tool: &Tool, profile: &mut ToolProfile) -> ProfileAction {
    let verdict = check_profile(tool, profile);
    preview(ui, profile, verdict.is_ok());
    match &verdict {
        Ok(()) => {
            ui.colored_label(VALID, "Valid profile.");
        }
        Err(reason) => {
            ui.colored_label(INVALID, reason);
        }
    }
    ui.weak(
        "Segments run from the tip on the axis, outward and upward: neither the radius nor the \
         height may decrease.",
    );

    let mut change: Option<Change> = None;
    egui::Grid::new("profile segments")
        .num_columns(6)
        .show(ui, |ui| {
            ui.strong("#");
            ui.strong("Type");
            ui.strong("End r, h (mm)");
            ui.strong("Centre r, h (mm)");
            ui.label("");
            ui.end_row();
            let count = profile.segments.len();
            for i in 0..count {
                let Some(segment) = profile.segments.get_mut(i) else {
                    continue;
                };
                ui.label((i + 1).to_string());
                let mut arc = matches!(segment, ProfileSegment::Arc { .. });
                let was_arc = arc;
                egui::ComboBox::from_id_salt(("segment type", i))
                    .selected_text(if arc { "Arc" } else { "Line" })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut arc, false, "Line");
                        ui.selectable_value(&mut arc, true, "Arc");
                    });
                if arc != was_arc {
                    change = Some(Change::Arc(i, arc));
                }
                match segment {
                    ProfileSegment::Line { to } => {
                        ui.horizontal(|ui| point(ui, to));
                        ui.label("");
                    }
                    ProfileSegment::Arc { to, centre } => {
                        ui.horizontal(|ui| point(ui, to));
                        ui.horizontal(|ui| point(ui, centre));
                    }
                }
                ui.horizontal(|ui| {
                    if ui
                        .small_button("+")
                        .on_hover_text("Add a segment after this one")
                        .clicked()
                    {
                        change = Some(Change::Insert(Some(i)));
                    }
                    if ui.small_button("Delete").clicked() {
                        change = Some(Change::Remove(i));
                    }
                });
                ui.end_row();
            }
        });
    if profile.segments.is_empty() && ui.button("Add the first segment").clicked() {
        change = Some(Change::Insert(None));
    }
    match change {
        Some(Change::Arc(i, arc)) => set_segment_arc(profile, i, arc),
        Some(Change::Insert(after)) => insert_segment(profile, after),
        Some(Change::Remove(i)) => remove_segment(profile, i),
        None => {}
    }

    let mut action = ProfileAction::None;
    ui.horizontal(|ui| {
        if ui
            .add_enabled(verdict.is_ok(), egui::Button::new("Apply"))
            .on_disabled_hover_text("The profile must be valid first")
            .clicked()
        {
            action = ProfileAction::Apply;
        }
        if ui.button("Cancel").clicked() {
            action = ProfileAction::Cancel;
        }
    });
    action
}

/// Two drag boxes for a point's radius and height.
fn point(ui: &mut egui::Ui, p: &mut ProfilePoint) {
    ui.add(
        egui::DragValue::new(&mut p.radius.0)
            .speed(0.05)
            .max_decimals(4)
            .range(0.0..=f64::MAX),
    );
    ui.add(
        egui::DragValue::new(&mut p.height.0)
            .speed(0.05)
            .max_decimals(4)
            .range(0.0..=f64::MAX),
    );
}

/// The profile revolved into its silhouette: both sides of the axis, the tip
/// at the bottom, drawn to scale in a square.
fn preview(ui: &mut egui::Ui, profile: &ToolProfile, valid: bool) {
    let size = ui.available_width().clamp(120.0, 260.0);
    let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, DARK.field);
    let outline = profile_outline(profile);
    let reach = outline
        .iter()
        .fold(1.0f64, |m, p| m.max(p[0].abs()).max(p[1].abs()));
    let margin = 12.0;
    let scale = (f64::from(size) - 2.0 * f64::from(margin)) / (2.0 * reach).max(f64::MIN_POSITIVE);
    let axis_x = rect.center().x;
    let base = rect.bottom() - margin;
    let to_screen =
        |r: f64, h: f64| -> Pos2 { pos2(axis_x + (r * scale) as f32, base - (h * scale) as f32) };
    // The tool axis.
    painter.line_segment(
        [
            pos2(axis_x, rect.top() + 4.0),
            pos2(axis_x, rect.bottom() - 4.0),
        ],
        Stroke::new(1.0, Color32::from_gray(90)),
    );
    let colour = if valid { VALID } else { INVALID };
    for side in [1.0, -1.0] {
        let points: Vec<Pos2> = outline
            .iter()
            .map(|p| to_screen(side * p[0], p[1]))
            .collect();
        for pair in points.windows(2) {
            if let [a, b] = pair {
                painter.line_segment([*a, *b], Stroke::new(2.0, colour));
            }
        }
    }
    painter.text(
        rect.min + vec2(6.0, 4.0),
        egui::Align2::LEFT_TOP,
        format!(
            "{:.1} mm across",
            2.0 * outline.iter().fold(0.0f64, |m, p| m.max(p[0]))
        ),
        egui::FontId::proportional(11.0),
        Color32::from_gray(170),
    );
}
