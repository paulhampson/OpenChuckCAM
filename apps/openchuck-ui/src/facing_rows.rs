// SPDX-License-Identifier: AGPL-3.0-or-later
//! Ordered indexed-face controls (OP-24, UI-07/26; decision0252).
//! Persistence is one validated parent parameter edit, never exposed JSON.

use eframe::egui::{self, RichText};
use openchuck_app::form::{Degrees, FaceOffset, Field, SideRequest, decode_rows, encode_rows};
use openchuck_app::project::ParameterValue;

use crate::field_row::{self, FieldState, Outcome, TextRow};
use crate::form_panel::Edit;
use crate::pending_edits::{self, Ticket};
use crate::theme::DARK;

#[derive(Debug, Clone, Copy)]
enum Action {
    Add,
    Remove(usize),
    Up(usize),
    Down(usize),
    Sort,
    Reset,
}

pub fn show(ui: &mut egui::Ui, field: &Field, state: &mut FieldState) -> Option<Edit> {
    let key = ui.id().with(("indexed facing actions", &field.name));
    let mut pending = ui
        .ctx()
        .data_mut(|d| d.get_temp::<Option<(Ticket, Action)>>(key))
        .flatten();
    let mut action = pending_edits::take_local(ui.ctx(), &mut pending);
    let data = match &field.value {
        Some(ParameterValue::Text(s)) => s.as_str(),
        _ => "[]",
    };
    let decoded = decode_rows(data);
    let valid = decoded.is_ok();
    ui.label(RichText::new(&field.label).strong())
        .on_hover_text(&field.description);
    if let Err(error) = &decoded {
        ui.colored_label(DARK.error, format!("Cannot read these faces: {error}"));
    }
    if let Some(error) = state.error(&field.name) {
        ui.colored_label(DARK.error, error);
    }
    let mut rows = decoded.unwrap_or_default();
    let mut changed = false;
    let mut requested = None;
    if valid {
        let count = rows.len();
        for (index, face) in rows.iter_mut().enumerate() {
            ui.group(|ui| {
                ui.label(format!("Face {}", index + 1));
                let angle_key = format!("{}/row/{index}/angle", field.name);
                if let Some(Some(value)) = number(
                    ui,
                    state,
                    &angle_key,
                    "A angle",
                    "°",
                    Some(face.angle.0),
                    false,
                ) {
                    face.angle = Degrees(value);
                    changed = true;
                }
                let offset_key = format!("{}/row/{index}/offset", field.name);
                let offset = match face.offset {
                    FaceOffset::FromPart => None,
                    FaceOffset::Explicit(value) => Some(value),
                };
                if let Some(value) =
                    number(ui, state, &offset_key, "Plane offset", "mm", offset, true)
                {
                    face.offset = value.map_or(FaceOffset::FromPart, FaceOffset::Explicit);
                    changed = true;
                }
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(index > 0, egui::Button::new("Move up"))
                        .clicked()
                    {
                        requested = Some(Action::Up(index));
                    }
                    if ui
                        .add_enabled(index + 1 < count, egui::Button::new("Move down"))
                        .clicked()
                    {
                        requested = Some(Action::Down(index));
                    }
                    if ui.button("Remove face").clicked() {
                        requested = Some(Action::Remove(index));
                    }
                });
            });
        }
        ui.horizontal(|ui| {
            if ui
                .add_enabled(rows.len() < 4096, egui::Button::new("Add face"))
                .clicked()
            {
                requested = Some(Action::Add);
            }
            if ui
                .add_enabled(rows.len() > 1, egui::Button::new("Sort by A angle"))
                .clicked()
            {
                requested = Some(Action::Sort);
            }
        });
        ui.label(RichText::new("Blank offset follows the part plus stock to leave. Standard faces are cut first; these faces follow in the order shown.").color(DARK.muted));
    }
    if field.set.is_some() && ui.button("Reset arbitrary faces").clicked() {
        requested = Some(Action::Reset);
    }
    if pending.is_none()
        && let Some(requested) = requested
    {
        action = pending_edits::defer_local(ui.ctx(), &mut pending, requested);
    }
    ui.ctx().data_mut(|d| d.insert_temp(key, pending));
    if let Some(action) = action {
        match action {
            Action::Add => rows.push(SideRequest {
                angle: Degrees(0.),
                offset: FaceOffset::FromPart,
            }),
            Action::Remove(index) if index < rows.len() => {
                rows.remove(index);
            }
            Action::Up(index) if index > 0 && index < rows.len() => rows.swap(index, index - 1),
            Action::Down(index) if index + 1 < rows.len() => rows.swap(index, index + 1),
            Action::Sort => rows.sort_by(|a, b| a.angle.0.total_cmp(&b.angle.0)),
            Action::Reset => {
                state.clear_prefix(&format!("{}/row/", field.name));
                return Some(Edit {
                    name: field.name.clone(),
                    value: None,
                    reset_to_default: true,
                });
            }
            _ => return None,
        }
        state.clear_prefix(&format!("{}/row/", field.name));
        changed = true;
    }
    if !changed {
        return None;
    }
    match encode_rows(&rows) {
        Ok(text) => Some(Edit {
            name: field.name.clone(),
            value: Some(ParameterValue::Text(text)),
            reset_to_default: false,
        }),
        Err(error) => {
            state.result(&field.name, Err(error.to_string()));
            None
        }
    }
}

/// Existing arithmetic/focus/refusal handling is shared with scalar forms.
fn number(
    ui: &mut egui::Ui,
    state: &mut FieldState,
    key: &str,
    label: &str,
    unit: &str,
    value: Option<f64>,
    optional: bool,
) -> Option<Option<f64>> {
    let read = |text: &str| -> Result<Option<f64>, String> {
        if text.trim().is_empty() && optional {
            return Ok(None);
        }
        text.trim()
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite())
            .map(Some)
            .ok_or_else(|| "Enter a finite number. Not applied.".into())
    };
    let validate = |text: &str| read(text).map(|_| ());
    let row = TextRow {
        key,
        label,
        unit,
        description: if optional {
            "Target plane offset from the rotary axis, allowance included once. Clear to follow the placed part plus stock to leave."
        } else {
            "Machine A angle in degrees. This face is cut with A fixed."
        },
        applied: value
            .map(|v| field_row::number(v, field_row::places(unit)))
            .unwrap_or_default(),
        hint: optional.then(|| "Part plus allowance".into()),
        enabled: true,
        number: true,
        reset: optional && value.is_some(),
        validate: Some(&validate),
        ..TextRow::default()
    };
    let result = match field_row::text_row(ui, state, &row) {
        Outcome::None => return None,
        Outcome::Reset => Ok(None),
        Outcome::Commit(text) => read(&text),
    };
    match result {
        Ok(value) => {
            state.result(key, Ok(()));
            Some(value)
        }
        Err(reason) => {
            state.result(key, Err(reason));
            None
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::panic,
        clippy::indexing_slicing,
        clippy::float_cmp
    )]
    use super::*;
    use egui::{Event, Key, Modifiers, PointerButton};
    use openchuck_app::form::{FieldKind, FormSection, Source};

    struct Rig {
        ctx: egui::Context,
        state: FieldState,
        field: Field,
        shapes: Vec<egui::epaint::ClippedShape>,
        edits: usize,
    }
    impl Rig {
        fn new() -> Self {
            let data = r#"[{"angle":130,"offset":9},{"angle":40,"offset":null}]"#;
            let mut rig = Self {
                ctx: egui::Context::default(),
                state: FieldState::default(),
                shapes: vec![],
                edits: 0,
                field: Field {
                    name: "arbitrary_faces".into(),
                    label: "Arbitrary A faces".into(),
                    description: String::new(),
                    kind: FieldKind::IndexedFaces,
                    set: Some(ParameterValue::Text(data.into())),
                    value: Some(ParameterValue::Text(data.into())),
                    source: Source::Operation,
                    section: FormSection::PatternAndTolerance,
                    problem: None,
                },
            };
            rig.frame(vec![]);
            rig
        }
        fn frame(&mut self, events: Vec<Event>) {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(700., 900.),
                )),
                events,
                ..Default::default()
            };
            let mut edit = None;
            let mut output = self.ctx.run_ui(input, |ui| {
                pending_edits::begin(ui.ctx());
                edit = show(ui, &self.field, &mut self.state);
            });
            output.textures_delta.clear();
            self.shapes = output.shapes;
            if let Some(edit) = edit {
                self.field.value = edit.value.clone();
                self.field.set = edit.value;
                self.state.result(&edit.name, Ok(()));
                self.edits += 1;
            }
        }
        fn rows(&self) -> Vec<SideRequest> {
            let Some(ParameterValue::Text(text)) = &self.field.value else {
                panic!("text rows expected")
            };
            decode_rows(text).unwrap()
        }
        fn centre(&self, text: &str, nth: usize) -> egui::Pos2 {
            self.shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Text(t) if t.galley.text().starts_with(text) => {
                        Some(t.pos + t.galley.rect.center().to_vec2())
                    }
                    _ => None,
                })
                .nth(nth)
                .unwrap_or_else(|| panic!("{text} #{nth} not shown"))
        }
        fn click(&mut self, text: &str, nth: usize) {
            let at = self.centre(text, nth);
            self.frame(vec![Event::PointerMoved(at)]);
            for pressed in [true, false] {
                self.frame(vec![Event::PointerButton {
                    pos: at,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                }]);
            }
        }
        fn type_first_angle(&mut self, text: &str) {
            self.click("130", 0);
            let modifiers = Modifiers {
                ctrl: true,
                command: true,
                ..Modifiers::NONE
            };
            self.frame(vec![
                Event::Key {
                    key: Key::A,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers,
                },
                Event::Text(text.into()),
            ]);
        }
    }

    #[test]
    fn req_op_24_req_ui_07_move_sort_add_remove_keep_offsets_attached() {
        let mut rig = Rig::new();
        assert_eq!(
            rig.rows().iter().map(|r| r.angle.0).collect::<Vec<_>>(),
            vec![130., 40.]
        );
        rig.click("Move down", 0);
        assert_eq!(
            rig.rows(),
            vec![
                SideRequest {
                    angle: Degrees(40.),
                    offset: FaceOffset::FromPart
                },
                SideRequest {
                    angle: Degrees(130.),
                    offset: FaceOffset::Explicit(9.)
                }
            ]
        );
        assert_eq!(rig.edits, 1);
        rig.frame(vec![]);
        rig.click("Move up", 1);
        assert_eq!(rig.rows()[0].angle.0, 130.);
        rig.frame(vec![]);
        rig.click("Sort by A angle", 0);
        assert_eq!(rig.rows()[0].angle.0, 40.);
        assert_eq!(rig.rows()[1].offset, FaceOffset::Explicit(9.));
        rig.frame(vec![]);
        rig.click("Add face", 0);
        assert_eq!(rig.rows().len(), 3);
        assert_eq!(rig.rows()[2].offset, FaceOffset::FromPart);
        rig.frame(vec![]);
        rig.click("Remove face", 0);
        assert_eq!(
            rig.rows().iter().map(|r| r.angle.0).collect::<Vec<_>>(),
            vec![130., 0.]
        );
        assert_eq!(rig.edits, 5);
    }

    #[test]
    fn req_ui_07_req_ui_26_invalid_angle_blocks_sort_until_shared_guard_resolves() {
        let mut rig = Rig::new();
        rig.type_first_angle("bad");
        rig.click("Sort by A angle", 0);
        assert!(pending_edits::local_requested(&rig.ctx));
        assert_eq!(rig.edits, 0);
        assert_eq!(rig.state.typed("arbitrary_faces/row/0/angle"), Some("bad"));
        pending_edits::resolve_local(&rig.ctx, false);
        rig.frame(vec![]);
        assert_eq!(rig.rows()[0].angle.0, 130.);
        assert_eq!(rig.state.typed("arbitrary_faces/row/0/angle"), Some("bad"));
        rig.click("Sort by A angle", 0);
        pending_edits::discard(&rig.ctx);
        pending_edits::resolve_local(&rig.ctx, true);
        rig.frame(vec![]);
        assert_eq!(rig.rows()[0].angle.0, 40.);
        assert_eq!(rig.edits, 1);
        assert!(rig.state.typed("arbitrary_faces/row/0/angle").is_none());
    }

    #[test]
    fn req_ui_26_req_op_24_keep_applies_arithmetic_before_reordering_faces() {
        let mut rig = Rig::new();
        rig.type_first_angle("20*3");
        rig.click("Sort by A angle", 0);
        assert!(pending_edits::local_requested(&rig.ctx));
        assert_eq!(rig.edits, 0);
        let entries = pending_edits::entries(&rig.ctx);
        assert_eq!(entries.len(), 1);
        pending_edits::set_mode(&rig.ctx, pending_edits::Mode::Keep(entries[0].id));
        rig.frame(vec![]);
        pending_edits::set_mode(&rig.ctx, pending_edits::Mode::Normal);
        pending_edits::resolve_local(&rig.ctx, true);
        rig.frame(vec![]);
        assert_eq!(
            rig.rows(),
            vec![
                SideRequest {
                    angle: Degrees(40.),
                    offset: FaceOffset::FromPart
                },
                SideRequest {
                    angle: Degrees(60.),
                    offset: FaceOffset::Explicit(9.)
                }
            ]
        );
        assert_eq!(rig.edits, 2);
    }
}
