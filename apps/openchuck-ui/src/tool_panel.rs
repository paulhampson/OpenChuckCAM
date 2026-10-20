// SPDX-License-Identifier: AGPL-3.0-or-later
//! The custom-profile editor with live preview (TL-03; work breakdown slice 8
//! item 5; decision record 0029 item 12; specification 12.1). The profile
//! being edited is held by the caller; every edit is checked by
//! `openchuck-app` as the file import checks it, and the preview turns red
//! with the reason while it is invalid. The selected segment and its end
//! points are highlighted. Only Apply changes the tool: one undoable step on
//! a project tool, a saved file for a library tool.

use eframe::egui::{self, Color32, Pos2, RichText, Sense, Stroke, pos2, vec2};
use openchuck_app::tool_edit::{
    arc_radius, check_profile, insert_arc, insert_segment, move_segment, profile_outline,
    remove_segment, set_segment_arc,
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
    /// Read the profile from a profile file.
    ImportFile,
    /// Write the profile to a profile file.
    ExportFile,
}

/// A change to the segment list, applied after drawing it.
#[derive(Debug, Clone, Copy)]
enum Change {
    Arc(usize, bool),
    Line,
    NewArc,
    Up,
    Down,
    Remove,
}

const VALID: Color32 = DARK.ok;
const INVALID: Color32 = DARK.error;

/// Draws the editor for `profile`, a draft of `tool`'s profile, with the
/// segment `selected` highlighted. `files` offers profile file import and
/// export.
pub fn editor(
    ui: &mut egui::Ui,
    tool: &Tool,
    profile: &mut ToolProfile,
    selected: &mut Option<usize>,
    files: bool,
) -> ProfileAction {
    let verdict = check_profile(tool, profile);
    let mut action = ProfileAction::None;
    let mut change: Option<Change> = None;
    let wide = ui.available_width() > 560.0;
    let mut body = |ui: &mut egui::Ui| {
        preview(ui, profile, verdict.is_ok(), *selected);
        ui.vertical(|ui| {
            heading(ui, "Profile segments");
            ui.horizontal_wrapped(|ui| {
                if ui
                    .small_button("+ Line")
                    .on_hover_text("Add a line after the selected segment")
                    .clicked()
                {
                    change = Some(Change::Line);
                }
                if ui
                    .small_button("+ Arc")
                    .on_hover_text("Add a quarter-turn arc after the selected segment")
                    .clicked()
                {
                    change = Some(Change::NewArc);
                }
                let some = selected.is_some_and(|i| i < profile.segments.len());
                if ui
                    .add_enabled(some, egui::Button::new("↑").small())
                    .on_hover_text("Move up")
                    .clicked()
                {
                    change = Some(Change::Up);
                }
                if ui
                    .add_enabled(some, egui::Button::new("↓").small())
                    .on_hover_text("Move down")
                    .clicked()
                {
                    change = Some(Change::Down);
                }
                if ui
                    .add_enabled(some, egui::Button::new("Delete").small())
                    .clicked()
                {
                    change = Some(Change::Remove);
                }
            });
            table(ui, profile, selected, &mut change);
            match &verdict {
                Ok(()) => {
                    ui.colored_label(
                        VALID,
                        "✓ Valid: starts on the axis, radius and height never decrease.",
                    );
                }
                Err(reason) => {
                    ui.colored_label(INVALID, reason);
                }
            }
            ui.label(
                RichText::new(
                    "Profiles use the same rules as an imported profile file. Undercut tools \
                     are not supported.",
                )
                .color(DARK.muted),
            );
            ui.horizontal(|ui| {
                if files {
                    if ui.small_button("Import profile file…").clicked() {
                        action = ProfileAction::ImportFile;
                    }
                    if ui
                        .add_enabled(
                            verdict.is_ok(),
                            egui::Button::new("Export profile…").small(),
                        )
                        .on_disabled_hover_text("The profile must be valid first")
                        .clicked()
                    {
                        action = ProfileAction::ExportFile;
                    }
                }
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
        });
    };
    if wide {
        ui.horizontal_top(|ui| body(ui));
    } else {
        body(ui);
    }
    let at = selected.filter(|i| *i < profile.segments.len());
    match change {
        Some(Change::Arc(i, arc)) => set_segment_arc(profile, i, arc),
        Some(Change::Line) => {
            insert_segment(profile, at.or(profile.segments.len().checked_sub(1)));
            *selected = Some(at.map_or(profile.segments.len() - 1, |i| i + 1));
        }
        Some(Change::NewArc) => {
            insert_arc(profile, at.or(profile.segments.len().checked_sub(1)));
            *selected = Some(at.map_or(profile.segments.len() - 1, |i| i + 1));
        }
        Some(Change::Up) => {
            if let Some(i) = at.and_then(|i| move_segment(profile, i, true)) {
                *selected = Some(i);
            }
        }
        Some(Change::Down) => {
            if let Some(i) = at.and_then(|i| move_segment(profile, i, false)) {
                *selected = Some(i);
            }
        }
        Some(Change::Remove) => {
            if let Some(i) = at {
                remove_segment(profile, i);
                *selected = i
                    .checked_sub(1)
                    .or((!profile.segments.is_empty()).then_some(0));
            }
        }
        None => {}
    }
    action
}

/// A section heading, in the 14.3 small caps style.
fn heading(ui: &mut egui::Ui, text: &str) {
    ui.label(
        RichText::new(text.to_uppercase())
            .small()
            .strong()
            .color(DARK.heading),
    );
}

/// The segment table: number, type, end (r, z), and the arc's radius; the
/// selected arc's centre can be edited under the table.
fn table(
    ui: &mut egui::Ui,
    profile: &mut ToolProfile,
    selected: &mut Option<usize>,
    change: &mut Option<Change>,
) {
    let radii: Vec<Option<f64>> = (0..profile.segments.len())
        .map(|i| arc_radius(profile, i))
        .collect();
    egui::Grid::new("profile segments")
        .num_columns(4)
        .striped(true)
        .show(ui, |ui| {
            ui.label(RichText::new("#").color(DARK.muted));
            ui.label(RichText::new("Type").color(DARK.muted));
            ui.label(RichText::new("End r, z (mm)").color(DARK.muted));
            ui.label(RichText::new("Arc").color(DARK.muted));
            ui.end_row();
            for (i, segment) in profile.segments.iter_mut().enumerate() {
                let chosen = *selected == Some(i);
                if ui
                    .selectable_label(chosen, (i + 1).to_string())
                    .on_hover_text("Select this segment")
                    .clicked()
                {
                    *selected = Some(i);
                }
                let mut arc = matches!(segment, ProfileSegment::Arc { .. });
                let was_arc = arc;
                egui::ComboBox::from_id_salt(("segment type", i))
                    .width(56.0)
                    .selected_text(if arc { "Arc" } else { "Line" })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut arc, false, "Line");
                        ui.selectable_value(&mut arc, true, "Arc");
                    });
                if arc != was_arc {
                    *change = Some(Change::Arc(i, arc));
                    *selected = Some(i);
                }
                let mut to = segment.end();
                let response = ui.horizontal(|ui| point(ui, &mut to)).inner;
                if response {
                    *selected = Some(i);
                    match segment {
                        ProfileSegment::Line { to: t } | ProfileSegment::Arc { to: t, .. } => {
                            *t = to;
                        }
                    }
                }
                match radii.get(i).copied().flatten() {
                    Some(r) => ui.monospace(format!("R {r:.3}")),
                    None => ui.label(RichText::new("–").color(DARK.muted)),
                };
                ui.end_row();
            }
        });
    if profile.segments.is_empty() {
        ui.label(RichText::new("No segments yet: add a line or an arc.").color(DARK.muted));
    }
    if let Some(ProfileSegment::Arc { centre, .. }) =
        selected.and_then(|i| profile.segments.get_mut(i))
    {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Arc centre r, z (mm)").color(DARK.muted));
            point(ui, centre);
        });
    }
}

/// Two drag boxes for a point's radius and height; true when either changed.
fn point(ui: &mut egui::Ui, p: &mut ProfilePoint) -> bool {
    let r = ui.add(
        egui::DragValue::new(&mut p.radius.0)
            .speed(0.05)
            .max_decimals(4)
            .range(0.0..=f64::MAX),
    );
    let h = ui.add(
        egui::DragValue::new(&mut p.height.0)
            .speed(0.05)
            .max_decimals(4)
            .range(0.0..=f64::MAX),
    );
    r.changed() || h.changed()
}

/// The profile's outline up to and including segment `index`, and where
/// that segment's part of it starts.
fn segment_points(profile: &ToolProfile, index: usize) -> Vec<[f64; 2]> {
    let before = ToolProfile {
        segments: profile.segments.iter().take(index).cloned().collect(),
    };
    let upto = ToolProfile {
        segments: profile.segments.iter().take(index + 1).cloned().collect(),
    };
    let start = profile_outline(&before).len().saturating_sub(1);
    profile_outline(&upto).split_off(start)
}

/// The profile revolved into its silhouette: both sides of the axis, the tip
/// at the bottom, drawn to scale in a square, with the selected segment and
/// its end points highlighted.
fn preview(ui: &mut egui::Ui, profile: &ToolProfile, valid: bool, selected: Option<usize>) {
    let size = ui.available_width().clamp(120.0, 220.0);
    let (rect, response) = ui.allocate_exact_size(vec2(size, size * 1.3), Sense::hover());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Other,
            true,
            "Profile preview: the revolved tool silhouette",
        )
    });
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, DARK.field);
    let outline = profile_outline(profile);
    let reach = outline
        .iter()
        .fold(1.0f64, |m, p| m.max(2.0 * p[0].abs()).max(p[1].abs()));
    let margin = 14.0;
    let scale = (f64::from(size) - 2.0 * f64::from(margin)) / reach.max(f64::MIN_POSITIVE);
    let axis_x = rect.center().x;
    let base = rect.bottom() - margin;
    let to_screen =
        |r: f64, h: f64| -> Pos2 { pos2(axis_x + (r * scale) as f32, base - (h * scale) as f32) };
    // The tool axis, dash-dot as the rotary axis is drawn (7.4).
    let mut y = rect.top() + 4.0;
    while y < rect.bottom() - 4.0 {
        let end = (y + 8.0).min(rect.bottom() - 4.0);
        painter.line_segment(
            [pos2(axis_x, y), pos2(axis_x, end)],
            Stroke::new(1.0, DARK.muted),
        );
        y += 12.0;
    }
    let colour = if valid { VALID } else { INVALID };
    for side in [1.0, -1.0] {
        let points: Vec<Pos2> = outline
            .iter()
            .map(|p| to_screen(side * p[0], p[1]))
            .collect();
        painter.add(egui::Shape::line(points, Stroke::new(2.0, colour)));
    }
    if let Some(i) = selected.filter(|i| *i < profile.segments.len()) {
        let points = segment_points(profile, i);
        let screen: Vec<Pos2> = points.iter().map(|p| to_screen(p[0], p[1])).collect();
        painter.add(egui::Shape::line(
            screen.clone(),
            Stroke::new(4.0, DARK.accent),
        ));
        for end in [screen.first(), screen.last()].into_iter().flatten() {
            painter.circle_filled(*end, 3.5, DARK.text);
        }
    }
    painter.text(
        rect.min + vec2(6.0, 4.0),
        egui::Align2::LEFT_TOP,
        format!(
            "Ø{:.3} mm",
            2.0 * outline.iter().fold(0.0f64, |m, p| m.max(p[0]))
        ),
        egui::FontId::monospace(11.0),
        DARK.muted,
    );
}
