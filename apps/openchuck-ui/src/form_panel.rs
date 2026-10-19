// SPDX-License-Identifier: AGPL-3.0-or-later
//! The parameter form of the chosen operation (UI-07; decision record 0029
//! item 11; interface specification section 8). The form comes from
//! `openchuck-app` as a view model; this module only draws it, with the
//! shared field rows ([`crate::field_row`]) in the sections of section 8, and
//! hands edits back. A refused value stays in its field with the reason under
//! it, and the project is unchanged.

use eframe::egui::{self, RichText};
use openchuck_app::form::{Field, FieldKind, FormSection, ParameterForm, Source};
use openchuck_app::project::ParameterValue;
use openchuck_app::settings::UserSettings;

use crate::field_row::{self, Outcome, TextRow};
use crate::theme::DARK;

/// Typed text and refused values in a form.
pub type FormState = field_row::FieldState;

/// An edit the user made to one field.
#[derive(Debug, Clone, PartialEq)]
pub struct Edit {
    /// The parameter's name.
    pub name: String,
    /// The new value, or `None` to clear it (the default or the cutting
    /// data then applies).
    pub value: Option<ParameterValue>,
}

/// Text for a value in a field, with the fixed places of its unit (14.3).
pub fn text(value: &ParameterValue, unit: &str) -> String {
    match value {
        ParameterValue::Number(v) => field_row::number(*v, field_row::places(unit)),
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
            .ok_or_else(|| "Not a number.".to_owned()),
        FieldKind::Integer { .. } => typed
            .parse::<i64>()
            .map(|v| Some(ParameterValue::Integer(v)))
            .map_err(|_| "Not a whole number.".to_owned()),
        _ => Ok(Some(ParameterValue::Text(typed.to_owned()))),
    }
}

/// The allowed range in words, for the field's tooltip.
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

/// Where the value that applies comes from, for the hint (section 8).
fn source_text(source: Source) -> &'static str {
    match source {
        Source::CuttingData => "from the tool's cutting data",
        Source::Default => "default",
        Source::Missing => "required",
        _ => "",
    }
}

fn unit_of(kind: &FieldKind) -> &'static str {
    match kind {
        FieldKind::Number { unit, .. } => unit,
        _ => "",
    }
}

/// The hint an empty field shows: the value that applies and its source.
fn hint(field: &Field) -> Option<String> {
    let source = source_text(field.source);
    match (&field.value, field.set.is_some()) {
        (_, true) => None,
        (Some(v), false) => Some(format!("{} \u{b7} {source}", text(v, unit_of(&field.kind)))),
        (None, false) => (!source.is_empty()).then(|| source.to_owned()),
    }
}

/// What an operation's form gave back this frame.
#[derive(Debug, Default)]
pub struct FormOutcome {
    /// The edit the user committed, if any.
    pub edit: Option<Edit>,
    /// A section was opened or closed: the settings need writing.
    pub settings_changed: bool,
}

/// The sections open by default, the first time an operation is shown.
const DEFAULT_OPEN: [&str; 3] = ["tool", "pattern", "axis"];

/// Draws an operation's form in its sections, each collapsible and its open
/// state remembered in `settings` for every operation (section 8).
pub fn show(
    ui: &mut egui::Ui,
    form: &ParameterForm,
    state: &mut FormState,
    settings: &mut UserSettings,
) -> FormOutcome {
    let mut outcome = FormOutcome::default();
    for section in FormSection::ALL {
        let fields: Vec<&Field> = form
            .fields
            .iter()
            .filter(|f| f.section == section)
            .collect();
        if fields.is_empty() {
            continue;
        }
        let key = section.key();
        let open = settings.section_open("operation", key, DEFAULT_OPEN.contains(&key));
        let section = field_row::section(
            ui,
            egui::Id::new(("operation section", key)),
            section.title(),
            open,
            false,
            |_| {},
            |ui| {
                let mut edit = None;
                for field in fields {
                    if let Some(e) = row(ui, field, true, state) {
                        edit = Some(e);
                    }
                }
                edit
            },
        );
        if let Some(Some(edit)) = section.body {
            outcome.edit = Some(edit);
        }
        let now_open = section.open;
        if now_open != open {
            settings.set_section_open("operation", key, now_open, &DEFAULT_OPEN);
            outcome.settings_changed = true;
        }
    }
    for note in &form.notes {
        ui.label(RichText::new(note).color(DARK.muted));
    }
    outcome
}

/// Draws a list of fields with no sections (a tool's dimensions), with
/// `notes` under them. With `reset`, a value that is set can be cleared so
/// the default applies.
pub fn show_fields(
    ui: &mut egui::Ui,
    fields: &[Field],
    notes: &[String],
    reset: bool,
    state: &mut FormState,
) -> Option<Edit> {
    let mut edit = None;
    for field in fields {
        if let Some(e) = row(ui, field, reset, state) {
            edit = Some(e);
        }
    }
    for note in notes {
        ui.label(RichText::new(note).color(DARK.muted));
    }
    edit
}

fn row(ui: &mut egui::Ui, field: &Field, reset: bool, state: &mut FormState) -> Option<Edit> {
    let name = || field.name.clone();
    match &field.kind {
        FieldKind::Flag => {
            let on = matches!(field.value, Some(ParameterValue::Flag(true)));
            field_row::flag_row(ui, &field.label, &field.description, on, true).map(|on| Edit {
                name: name(),
                value: Some(ParameterValue::Flag(on)),
            })
        }
        FieldKind::Choice { options } if !options.is_empty() => {
            let current = match &field.value {
                Some(ParameterValue::Text(t)) => options.iter().position(|o| o == t),
                _ => None,
            };
            let labels: Vec<&str> = options.iter().map(String::as_str).collect();
            field_row::choice_row(
                ui,
                &field.name,
                &field.label,
                &field.description,
                &labels,
                current,
                true,
            )
            .and_then(|i| options.get(i))
            .map(|o| Edit {
                name: name(),
                value: Some(ParameterValue::Text(o.clone())),
            })
        }
        FieldKind::Number { .. } | FieldKind::Integer { .. } | FieldKind::Text => {
            let unit = unit_of(&field.kind);
            let range = range(&field.kind);
            let row = TextRow {
                key: &field.name,
                label: &field.label,
                description: &field.description,
                unit,
                range: range.as_deref(),
                applied: field
                    .set
                    .as_ref()
                    .map(|v| text(v, unit))
                    .unwrap_or_default(),
                hint: hint(field),
                problem: None,
                reset: reset && field.set.is_some(),
                enabled: true,
                focus: false,
            };
            match field_row::text_row(ui, state, &row) {
                Outcome::None => None,
                Outcome::Reset => Some(Edit {
                    name: name(),
                    value: None,
                }),
                Outcome::Commit(typed) => match parse(&field.kind, &typed) {
                    Ok(value) => Some(Edit {
                        name: name(),
                        value,
                    }),
                    Err(e) => {
                        state.refuse(&field.name, &typed, format!("{e} Not applied."));
                        None
                    }
                },
            }
        }
        _ => {
            field_row::read_only_row(
                ui,
                &field.label,
                &field.description,
                &field
                    .value
                    .as_ref()
                    .map(|v| text(v, ""))
                    .unwrap_or_default(),
            );
            None
        }
    }
}

/// The reason under a refused field: the application's message as a
/// sentence, ending "Not applied." (section 8).
pub fn refusal(message: &str) -> String {
    let message = message.trim().trim_end_matches('.');
    let mut chars = message.chars();
    let first = chars.next().map(|c| c.to_uppercase().collect::<String>());
    format!(
        "{}{}. Not applied.",
        first.unwrap_or_default(),
        chars.as_str()
    )
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
        assert_eq!(parse(&number, "abc").unwrap_err(), "Not a number.");
        assert!(parse(&number, "inf").is_err());
        let integer = FieldKind::Integer { min: 0, max: 9 };
        assert_eq!(
            parse(&integer, "3").unwrap(),
            Some(ParameterValue::Integer(3))
        );
        assert!(parse(&integer, "3.5").is_err());
        // The text of a value reads back as the same value.
        for v in [0.1, 0.0005, 12.5] {
            assert_eq!(
                parse(&number, &text(&ParameterValue::Number(v), "mm")).unwrap(),
                Some(ParameterValue::Number(v))
            );
        }
    }

    #[test]
    fn req_ui_07_an_empty_field_hints_the_value_that_applies_and_its_source() {
        let field = Field {
            name: "feed".into(),
            label: "Cutting feed".into(),
            description: String::new(),
            kind: FieldKind::Number {
                unit: "mm/min",
                min: Some(0.0),
                min_exclusive: true,
                max: None,
            },
            set: None,
            value: Some(ParameterValue::Number(1200.0)),
            source: Source::CuttingData,
            section: FormSection::ToolAndFeeds,
        };
        assert_eq!(
            hint(&field).unwrap(),
            "1200 \u{b7} from the tool's cutting data"
        );
        assert_eq!(range(&field.kind).unwrap(), "above 0 mm/min");
        let set = Field {
            set: Some(ParameterValue::Number(900.0)),
            ..field
        };
        assert_eq!(hint(&set), None);
    }

    #[test]
    fn req_ui_07_a_refusal_is_a_sentence_ending_not_applied() {
        assert_eq!(
            refusal("must not exceed the tool diameter (3.0 mm)"),
            "Must not exceed the tool diameter (3.0 mm). Not applied."
        );
    }
}
