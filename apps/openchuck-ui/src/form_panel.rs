// SPDX-License-Identifier: AGPL-3.0-or-later
//! The parameter form of the chosen operation (UI-07; decision record 0026
//! item 11). The form comes from `openchuck-app` as a view model; this
//! module only draws it and hands edits back. A rejected value stays in its
//! field with the reason under it, and the project is unchanged.

use std::collections::BTreeMap;

use eframe::egui::{self, Color32, RichText};
use openchuck_app::form::{Field, FieldKind, ParameterForm, Source};
use openchuck_app::project::ParameterValue;

/// An edit the user made to one field.
#[derive(Debug, Clone, PartialEq)]
pub struct Edit {
    /// The parameter's name.
    pub name: String,
    /// The new value, or `None` to clear it (the default or the cutting
    /// data then applies).
    pub value: Option<ParameterValue>,
}

/// Text being typed into fields, and rejected values with their reasons.
#[derive(Debug, Default)]
pub struct FormState {
    text: BTreeMap<String, String>,
    errors: BTreeMap<String, String>,
}

impl FormState {
    /// Forgets typed text and errors, for another operation or after undo.
    pub fn clear(&mut self) {
        self.text.clear();
        self.errors.clear();
    }

    /// Records the outcome of applying an edit to `name`.
    pub fn result(&mut self, name: &str, result: Result<(), String>) {
        match result {
            Ok(()) => {
                self.text.remove(name);
                self.errors.remove(name);
            }
            Err(e) => {
                self.errors.insert(name.to_owned(), e);
            }
        }
    }

    /// The reason a field's value was refused, if it was.
    pub fn error(&self, name: &str) -> Option<&str> {
        self.errors.get(name).map(String::as_str)
    }
}

/// Text for a value in a field.
pub fn text(value: &ParameterValue) -> String {
    match value {
        ParameterValue::Number(v) => format!("{v}"),
        ParameterValue::Integer(i) => i.to_string(),
        ParameterValue::Flag(b) => b.to_string(),
        ParameterValue::Text(t) => t.clone(),
    }
}

/// Reads typed text as a value of `kind`. Empty text clears the value.
pub fn parse(kind: &FieldKind, typed: &str) -> Result<Option<ParameterValue>, String> {
    let typed = typed.trim();
    if typed.is_empty() {
        return Ok(None);
    }
    match kind {
        FieldKind::Number { .. } => typed
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite())
            .map(|v| Some(ParameterValue::Number(v)))
            .ok_or_else(|| format!("'{typed}' is not a number")),
        FieldKind::Integer { .. } => typed
            .parse::<i64>()
            .map(|v| Some(ParameterValue::Integer(v)))
            .map_err(|_| format!("'{typed}' is not a whole number")),
        _ => Ok(Some(ParameterValue::Text(typed.to_owned()))),
    }
}

/// The allowed range in words, for a tooltip.
fn range(kind: &FieldKind) -> Option<String> {
    match kind {
        FieldKind::Number {
            unit,
            min,
            min_exclusive,
            max,
        } => {
            let lo = min.map(|m| {
                let relation = if *min_exclusive { "above" } else { "at least" };
                format!("{relation} {m} {unit}")
            });
            let hi = max.map(|m| format!("at most {m} {unit}"));
            match (lo, hi) {
                (Some(l), Some(h)) => Some(format!("{l} and {h}")),
                (l, h) => l.or(h),
            }
        }
        FieldKind::Integer { min, max } => Some(format!("from {min} to {max}")),
        _ => None,
    }
}

fn source_text(source: Source) -> &'static str {
    match source {
        Source::Operation => "",
        Source::CuttingData => "from the tool's cutting data",
        Source::Default => "default",
        Source::Missing => "required",
        _ => "",
    }
}

/// Draws the form. Returns the edit the user committed this frame, if any.
pub fn show(ui: &mut egui::Ui, form: &ParameterForm, state: &mut FormState) -> Option<Edit> {
    let mut edit = None;
    egui::Grid::new("parameter form")
        .num_columns(3)
        .striped(true)
        .show(ui, |ui| {
            for field in &form.fields {
                if let Some(e) = row(ui, field, state) {
                    edit = Some(e);
                }
                ui.end_row();
                if let Some(error) = state.error(&field.name) {
                    ui.label("");
                    ui.label(RichText::new(error).color(Color32::from_rgb(230, 80, 80)));
                    ui.end_row();
                }
            }
        });
    for note in &form.notes {
        ui.small(note);
    }
    edit
}

fn row(ui: &mut egui::Ui, field: &Field, state: &mut FormState) -> Option<Edit> {
    let mut edit = None;
    let mut tip = field.description.clone();
    if let Some(r) = range(&field.kind) {
        tip.push_str(&format!(" Allowed: {r}."));
    }
    ui.label(&field.label).on_hover_text(&tip);
    ui.horizontal(|ui| match &field.kind {
        FieldKind::Number { unit, .. } => {
            edit = text_field(ui, field, state, unit);
        }
        FieldKind::Integer { .. } => {
            edit = text_field(ui, field, state, "");
        }
        FieldKind::Flag => {
            let mut on = matches!(field.value, Some(ParameterValue::Flag(true)));
            if ui.checkbox(&mut on, "").on_hover_text(&tip).changed() {
                edit = Some(Edit {
                    name: field.name.clone(),
                    value: Some(ParameterValue::Flag(on)),
                });
            }
        }
        FieldKind::Choice { options } => {
            let current = field.value.as_ref().map(text).unwrap_or_default();
            egui::ComboBox::from_id_salt(&field.name)
                .selected_text(&current)
                .show_ui(ui, |ui| {
                    for option in options {
                        if ui.selectable_label(*option == current, option).clicked()
                            && *option != current
                        {
                            edit = Some(Edit {
                                name: field.name.clone(),
                                value: Some(ParameterValue::Text(option.clone())),
                            });
                        }
                    }
                });
        }
        _ => {
            ui.label(field.value.as_ref().map(text).unwrap_or_default());
        }
    });
    ui.horizontal(|ui| {
        let note = source_text(field.source);
        if field.source == Source::Missing {
            ui.label(RichText::new(note).color(Color32::from_rgb(230, 80, 80)));
        } else if !note.is_empty() {
            ui.weak(note);
        }
        if field.set.is_some()
            && ui
                .small_button("Reset")
                .on_hover_text("Clear the value: the default or the tool's cutting data applies")
                .clicked()
        {
            edit = Some(Edit {
                name: field.name.clone(),
                value: None,
            });
        }
    });
    edit
}

/// A text box for a number. The value is applied when the box loses focus
/// (or Enter is pressed); until then the typed text is kept.
fn text_field(ui: &mut egui::Ui, field: &Field, state: &mut FormState, unit: &str) -> Option<Edit> {
    let shown = field.set.as_ref().map(text).unwrap_or_default();
    let mut typed = state.text.get(&field.name).cloned().unwrap_or(shown);
    let hint = field.value.as_ref().map(text).unwrap_or_default();
    let response = ui.add(
        egui::TextEdit::singleline(&mut typed)
            .hint_text(hint)
            .desired_width(90.0),
    );
    if !unit.is_empty() {
        ui.label(unit);
    }
    if response.changed() {
        state.text.insert(field.name.clone(), typed.clone());
    }
    if response.lost_focus() && state.text.contains_key(&field.name) {
        return match parse(&field.kind, &typed) {
            Ok(value) => Some(Edit {
                name: field.name.clone(),
                value,
            }),
            Err(e) => {
                state.errors.insert(field.name.clone(), e);
                None
            }
        };
    }
    None
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn req_ui_07_typed_text_is_read_by_kind() {
        let number = FieldKind::Number {
            unit: "mm",
            min: None,
            min_exclusive: false,
            max: None,
        };
        assert_eq!(
            parse(&number, " 0.25 ").unwrap(),
            Some(ParameterValue::Number(0.25))
        );
        assert_eq!(parse(&number, "").unwrap(), None);
        assert!(parse(&number, "abc").is_err());
        assert!(parse(&number, "inf").is_err());
        let integer = FieldKind::Integer { min: 0, max: 9 };
        assert_eq!(
            parse(&integer, "3").unwrap(),
            Some(ParameterValue::Integer(3))
        );
        assert!(parse(&integer, "3.5").is_err());
        // The text of a value reads back as the same value.
        assert_eq!(
            parse(&number, &text(&ParameterValue::Number(0.1))).unwrap(),
            Some(ParameterValue::Number(0.1))
        );
    }

    #[test]
    fn req_ui_07_refused_values_keep_their_reason_until_accepted() {
        let mut state = FormState::default();
        state.text.insert("stepover".into(), "-1".into());
        state.result("stepover", Err("must be above 0 mm".into()));
        assert_eq!(state.error("stepover"), Some("must be above 0 mm"));
        state.result("stepover", Ok(()));
        assert_eq!(state.error("stepover"), None);
        assert!(state.text.is_empty());
    }
}
