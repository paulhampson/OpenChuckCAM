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

use openchuck_app::requirements::{Requirement, RequirementField};

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
    /// An explicit Reset action, distinct from committing empty manual text.
    pub reset_to_default: bool,
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
    pub edits: Vec<Edit>,
    /// A section was opened or closed: the settings need writing.
    pub settings_changed: bool,
    /// Select the four standard Box faces as one application command.
    pub all_four: bool,
}

/// The sentence under a setting the operation's path warns about (decision
/// record 0175): the warning itself, with its numbers, is in Messages.
pub const WARNED_SETTING: &str = "The last path was made with a warning about this setting (see \
     Messages): change it to accept or remove the warning.";

/// The settings of an operation to mark, and the one to bring into view.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Marks {
    /// The settings its current path's warnings point at (decision record
    /// 0175), by parameter name.
    pub warned: Vec<String>,
    /// The setting to scroll to this frame, its section opened: the "Show
    /// setting" link of a message.
    pub jump: Option<String>,
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
    needs: &[&Requirement],
    jump: Option<&RequirementField>,
    marks: &Marks,
) -> FormOutcome {
    let mut outcome = FormOutcome::default();
    if form.strategy == "facing"
        && form
            .fields
            .iter()
            .any(|f| matches!(f.kind, FieldKind::IndexedFaces))
    {
        let key = ui.id().with(("Facing All four", form.operation));
        let mut pending = ui
            .ctx()
            .data_mut(|d| d.get_temp::<Option<(crate::pending_edits::Ticket, ())>>(key))
            .flatten();
        outcome.all_four = crate::pending_edits::take_local(ui.ctx(), &mut pending).is_some();
        if pending.is_none() && ui.button("All four sides").clicked() {
            outcome.all_four |=
                crate::pending_edits::defer_local(ui.ctx(), &mut pending, ()).is_some();
        }
        ui.ctx().data_mut(|d| d.insert_temp(key, pending));
    }
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
        // The jump to the first missing value, or to a setting a path's
        // warning points at, opens the section it is in.
        let forced = fields.iter().any(|f| {
            jump == Some(&RequirementField::Parameter(f.name.clone()))
                || marks.jump.as_deref() == Some(f.name.as_str())
        });
        let open = settings.section_open("operation", key, DEFAULT_OPEN.contains(&key)) || forced;
        let section = field_row::section(
            ui,
            egui::Id::new(("operation section", key)),
            section.title(),
            open,
            forced,
            |_| {},
            |ui| {
                let mut edits = Vec::new();
                for field in fields {
                    let field_key = RequirementField::Parameter(field.name.clone());
                    let need = needs
                        .iter()
                        .find(|r| r.field == field_key)
                        .map(|r| r.text.as_str());
                    let warned = marks.warned.contains(&field.name).then_some(WARNED_SETTING);
                    let scroll = jump == Some(&field_key)
                        || marks.jump.as_deref() == Some(field.name.as_str());
                    let edit = if matches!(field.kind, FieldKind::IndexedFaces) {
                        ui.push_id(form.operation, |ui| {
                            row(ui, field, true, state, need, warned, scroll, None)
                        })
                        .inner
                    } else {
                        row(ui, field, true, state, need, warned, scroll, None)
                    };
                    if let Some(e) = edit {
                        edits.push(e);
                    }
                }
                edits
            },
        );
        if let Some(edits) = section.body {
            outcome.edits.extend(edits);
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
    preflight: impl Fn(&Field) -> crate::pending_edits::Preflight,
) -> Vec<Edit> {
    let mut edits = Vec::new();
    for field in fields {
        if let Some(e) = row(
            ui,
            field,
            reset,
            state,
            None,
            None,
            false,
            Some(preflight(field)),
        ) {
            edits.push(e);
        }
    }
    for note in notes {
        ui.label(RichText::new(note).color(DARK.muted));
    }
    edits
}

#[allow(clippy::too_many_arguments)]
fn row(
    ui: &mut egui::Ui,
    field: &Field,
    reset: bool,
    state: &mut FormState,
    need: Option<&str>,
    warned: Option<&str>,
    scroll: bool,
    preflight: Option<crate::pending_edits::Preflight>,
) -> Option<Edit> {
    let name = || field.name.clone();
    match &field.kind {
        FieldKind::IndexedFaces => crate::facing_rows::show(ui, field, state),
        FieldKind::Flag => {
            let on = matches!(field.value, Some(ParameterValue::Flag(true)));
            field_row::flag_row(ui, &field.label, &field.description, on, true).map(|on| Edit {
                name: name(),
                value: Some(ParameterValue::Flag(on)),
                reset_to_default: false,
            })
        }
        FieldKind::Choice { options } if !options.is_empty() => {
            let current = match &field.value {
                Some(ParameterValue::Text(t)) => options.iter().position(|o| o == t),
                _ => None,
            };
            let labels: Vec<&str> = options
                .iter()
                .map(|value| choice_label(&field.name, value))
                .collect();
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
                reset_to_default: false,
            })
        }
        FieldKind::Number { .. } | FieldKind::Integer { .. } | FieldKind::Text => {
            let unit = unit_of(&field.kind);
            let range = range(&field.kind);
            let validate = |typed: &str| {
                let value = parse(&field.kind, typed)?;
                match (&field.kind, value) {
                    (
                        FieldKind::Number {
                            min,
                            min_exclusive,
                            max,
                            ..
                        },
                        Some(ParameterValue::Number(v)),
                    ) => {
                        if min.is_some_and(|m| if *min_exclusive { v <= m } else { v < m })
                            || max.is_some_and(|m| v > m)
                        {
                            return Err(format!(
                                "{}: {}. Not applied.",
                                field.label,
                                range.as_deref().unwrap_or("outside the allowed range")
                            ));
                        }
                    }
                    (FieldKind::Integer { min, max }, Some(ParameterValue::Integer(v)))
                        if v < *min || v > *max =>
                    {
                        return Err(format!(
                            "{} must be from {min} to {max}. Not applied.",
                            field.label
                        ));
                    }
                    _ => {}
                }
                Ok(())
            };
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
                needs: need,
                warned,
                scroll_to: scroll,
                number: !matches!(field.kind, FieldKind::Text),
                validate: Some(&validate),
                preflight,
            };
            match field_row::text_row(ui, state, &row) {
                Outcome::None => None,
                Outcome::Reset => Some(Edit {
                    name: name(),
                    value: None,
                    reset_to_default: true,
                }),
                Outcome::Commit(typed) => match parse(&field.kind, &typed) {
                    Ok(value) => Some(Edit {
                        name: name(),
                        value,
                        reset_to_default: false,
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
                field
                    .value
                    .as_ref()
                    .map(|v| text(v, ""))
                    .unwrap_or_default(),
            );
            None
        }
    }
}

fn choice_label<'a>(name: &str, value: &'a str) -> &'a str {
    match (name, value) {
        ("shape", "flat") => "Flat along stock",
        ("shape", "round") => "Round",
        ("shape", "end") => "Unheld end",
        ("shape", "box") => "Box sides",
        ("extent", "whole_stock") => "Whole stock",
        ("extent", "part") => "Part only",
        ("extent", "part_plus_tabs") => "Part plus tabs",
        ("layer_distribution", "full") => "Full stepdown, shorter final pass",
        ("layer_distribution", "equal") => "Equal stepdowns",
        _ => value,
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

    fn texts(shapes: &[egui::epaint::ClippedShape]) -> Vec<String> {
        shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::Shape::Text(t) => Some(t.galley.text().to_owned()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn req_op_24_req_ui_07_arbitrary_faces_show_editable_rows_and_explicit_order_controls() {
        let ctx = egui::Context::default();
        let data = r#"[{"angle":130,"offset":9},{"angle":40,"offset":null}]"#;
        let field = Field {
            name: "arbitrary_faces".into(),
            label: "Arbitrary A faces".into(),
            description: String::new(),
            kind: FieldKind::IndexedFaces,
            set: Some(ParameterValue::Text(data.into())),
            value: Some(ParameterValue::Text(data.into())),
            source: Source::Operation,
            section: FormSection::PatternAndTolerance,
        };
        let mut state = FormState::default();
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(700., 900.),
                )),
                ..Default::default()
            },
            |ui| {
                row(ui, &field, true, &mut state, None, None, false, None);
            },
        );
        output.textures_delta.clear();
        let shown = texts(&output.shapes);
        for label in [
            "A angle",
            "Plane offset",
            "Move up",
            "Move down",
            "Sort by A angle",
            "Add face",
        ] {
            assert!(
                shown.iter().any(|s| s == label),
                "Missing {label}: {shown:?}"
            );
        }
        assert!(!shown.iter().any(|s| s.contains("\"angle\"")), "{shown:?}");
    }

    #[test]
    fn req_op_07_req_nfr_12_the_setting_a_warning_names_is_marked_and_the_jump_opens_it() {
        // Decision 0175 (issue #226): the tolerance a swarf overcut warning
        // points at is marked with the warning shape and a sentence, not
        // colour alone, and "Show setting" opens its section and brings it
        // into view even when the user closed the section.
        let form = ParameterForm {
            operation: openchuck_app::project::id::OperationId::new(),
            strategy: "swarf".into(),
            fields: vec![Field {
                name: "tolerance".into(),
                label: "Tolerance".into(),
                description: String::new(),
                kind: FieldKind::Number {
                    unit: "mm",
                    min: Some(0.0),
                    min_exclusive: true,
                    max: None,
                },
                set: None,
                value: Some(ParameterValue::Number(0.01)),
                source: Source::Default,
                section: FormSection::PatternAndTolerance,
            }],
            notes: Vec::new(),
        };
        let ctx = egui::Context::default();
        let mut settings = UserSettings::default();
        settings.set_section_open("operation", "pattern", false, &DEFAULT_OPEN);
        let run = |settings: &mut UserSettings, marks: &Marks| {
            let mut state = FormState::default();
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(500.0, 400.0),
                )),
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |ui| {
                show(ui, &form, &mut state, settings, &[], None, marks);
            });
            output.textures_delta.clear();
            texts(&output.shapes)
        };
        let warned = Marks {
            warned: vec!["tolerance".into()],
            jump: None,
        };
        // Closed by the user, and no jump: the mark waits in the section.
        assert!(
            !run(&mut settings, &warned)
                .iter()
                .any(|t| t == WARNED_SETTING)
        );
        // The jump opens the section and shows the mark under the field.
        let jump = Marks {
            jump: Some("tolerance".into()),
            ..warned.clone()
        };
        run(&mut settings, &jump);
        // The section opens over the frames of its animation.
        for _ in 0..30 {
            run(&mut settings, &warned);
        }
        let shown = run(&mut settings, &warned);
        assert!(shown.iter().any(|t| t == WARNED_SETTING), "{shown:?}");
        assert!(WARNED_SETTING.contains("warning"));
        // No warning: no mark.
        settings.set_section_open("operation", "pattern", true, &DEFAULT_OPEN);
        let shown = run(&mut settings, &Marks::default());
        assert!(!shown.iter().any(|t| t == WARNED_SETTING), "{shown:?}");
        assert!(shown.iter().any(|t| t == "Tolerance"), "{shown:?}");
    }
}
