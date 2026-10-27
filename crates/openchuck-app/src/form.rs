// SPDX-License-Identifier: AGPL-3.0-or-later
//! Parameter forms: an operation's parameters as a view model built from its
//! strategy's schema, with units, ranges, defaults and where each shown value
//! comes from (UI-07; work breakdown slice 8 item 5; decision record 0029
//! item 11).
//!
//! The front end draws one field per parameter and calls
//! [`crate::Application::set_parameter`], which checks the value against the
//! schema before it changes anything: an invalid value is refused with a
//! message naming the parameter, and the project is left as it was.

use std::collections::BTreeMap;

use openchuck_path::params::{ParameterKind, ParameterSchema, names};
use openchuck_path::{ParameterValue as PathValue, Registry};
use openchuck_project::id::OperationId;
use openchuck_project::{ParameterValue, Project};

use crate::AppError;
use crate::operation::parameter_values;

pub use openchuck_path::facing::selection::{FaceOffset, SideRequest, decode_rows, encode_rows};
pub use openchuck_types::units::Degrees;

/// What kind of value a field takes.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum FieldKind {
    /// A number in `unit` (a symbol such as "mm", empty for a plain number).
    Number {
        /// The unit symbol.
        unit: &'static str,
        /// Lowest value, if any.
        min: Option<f64>,
        /// True when `min` itself is not allowed.
        min_exclusive: bool,
        /// Highest value, if any.
        max: Option<f64>,
    },
    /// A whole number from `min` to `max`.
    Integer {
        /// Lowest value.
        min: i64,
        /// Highest value.
        max: i64,
    },
    /// On or off.
    Flag,
    /// Free text, such as a list of angles.
    Text,
    /// Ordered arbitrary A/optional plane-offset rows, stored as one text
    /// parameter and edited as a table rather than exposed JSON.
    IndexedFaces,
    /// One of a list of words.
    Choice {
        /// The words, in display order.
        options: Vec<String>,
    },
}

/// Where a field's shown value comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Source {
    /// The operation sets it.
    Operation,
    /// The tool's cutting data for the setup's stock material (TL-05).
    CuttingData,
    /// The strategy's default.
    Default,
    /// Nothing gives it, and it is required.
    Missing,
}

/// The section of the operation inspector a field is shown in (interface
/// specification section 8; decision record 0114 C10). The Geometry section,
/// which holds the selection roles, has no fields and is not listed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum FormSection {
    /// The feeds.
    ToolAndFeeds,
    /// The pattern of passes, their spacing and the tolerance.
    PatternAndTolerance,
    /// How the tool axis is set and how A turns.
    ToolAxisAndA,
    /// Entry, links and retract (decision record 0026).
    EntryLinksAndRetract,
    /// Limits that most operations leave as they are.
    Advanced,
}

impl FormSection {
    /// Every section, in the order the inspector shows them.
    pub const ALL: [Self; 5] = [
        Self::ToolAndFeeds,
        Self::PatternAndTolerance,
        Self::ToolAxisAndA,
        Self::EntryLinksAndRetract,
        Self::Advanced,
    ];

    /// The section's title, in sentence case.
    pub const fn title(self) -> &'static str {
        match self {
            Self::ToolAndFeeds => "Tool and feeds",
            Self::PatternAndTolerance => "Pattern and tolerance",
            Self::ToolAxisAndA => "Tool axis and A",
            Self::EntryLinksAndRetract => "Entry, links and retract",
            Self::Advanced => "Advanced",
        }
    }

    /// A short key for remembering whether the section is open.
    pub const fn key(self) -> &'static str {
        match self {
            Self::ToolAndFeeds => "tool",
            Self::PatternAndTolerance => "pattern",
            Self::ToolAxisAndA => "axis",
            Self::EntryLinksAndRetract => "links",
            Self::Advanced => "advanced",
        }
    }

    /// The section of the parameter `name`. A parameter the list does not
    /// name, such as one a strategy adds later, is shown with the pattern.
    pub fn of(name: &str) -> Self {
        match name {
            "feed" | "plunge_feed" => Self::ToolAndFeeds,
            "angles" | "count" | "index_angle" | "start_angle" | "rotation" | "lead_angle"
            | "max_tilt" | "axis_tolerance" | "max_angle_step" | "tip_end" | "tip_extension" => {
                Self::ToolAxisAndA
            }
            "retract"
            | "approach_distance"
            | "entry"
            | "ramp_back_and_forth"
            | "ramp_angle"
            | "helix_diameter"
            | "lead_radius"
            | "reentry_clearance"
            | "retract_height"
            | "chip_break_retract"
            | "peck_mode"
            | "peck_depth" => Self::EntryLinksAndRetract,
            "diameter_tolerance" | "crease_angle" | "min_length" | "min_remaining" | "min_cut"
            | "max_material_left" | "through_allowance" | "max_step" | "max_x_step"
            | "depth_reference" => Self::Advanced,
            _ => Self::PatternAndTolerance,
        }
    }
}

/// One parameter of the form.
#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    /// The name stored in the operation.
    pub name: String,
    /// The label to show.
    pub label: String,
    /// What it does, for a tooltip.
    pub description: String,
    /// Its kind, unit and range.
    pub kind: FieldKind,
    /// The value the operation sets, if any.
    pub set: Option<ParameterValue>,
    /// The value that applies: the operation's, the cutting data's or the
    /// default. `None` when it is missing.
    pub value: Option<ParameterValue>,
    /// Where `value` comes from.
    pub source: Source,
    /// The inspector section it is shown in.
    pub section: FormSection,
}

/// An operation's parameter form.
#[derive(Debug, Clone, PartialEq)]
pub struct ParameterForm {
    /// The operation.
    pub operation: OperationId,
    /// Its strategy's registry name.
    pub strategy: String,
    /// The fields, in the schema's display order.
    pub fields: Vec<Field>,
    /// Notes: overrides of the cutting data, or why required values are
    /// missing.
    pub notes: Vec<String>,
}

fn from_path(v: &PathValue) -> ParameterValue {
    match v {
        PathValue::Integer(i) => ParameterValue::Integer(*i),
        PathValue::Number(n) => ParameterValue::Number(*n),
        PathValue::Flag(b) => ParameterValue::Flag(*b),
        PathValue::Text(t) => ParameterValue::Text(t.clone()),
    }
}

fn to_path(v: &ParameterValue) -> PathValue {
    match v {
        ParameterValue::Integer(i) => PathValue::Integer(*i),
        ParameterValue::Number(n) => PathValue::Number(*n),
        ParameterValue::Flag(b) => PathValue::Flag(*b),
        ParameterValue::Text(t) => PathValue::Text(t.clone()),
    }
}

/// The form of `operation` in `project`, from its strategy in `registry`.
pub fn parameter_form(
    project: &Project,
    registry: &Registry,
    operation: OperationId,
) -> Result<ParameterForm, AppError> {
    let op = project
        .operation(operation)
        .ok_or_else(|| AppError::UnknownOperation(operation.to_string()))?;
    let schema = crate::operation::effective_schema(op, registry.get(&op.strategy)?);
    let (resolved, mut notes) = match parameter_values(project, op, registry) {
        Ok(r) => (r.values, r.notes),
        Err(e) => (BTreeMap::new(), vec![e.user_message()]),
    };
    // Said in the form, before the operation is run (OI-96, OI-238;
    // decision 0111 items 1 and 3).
    if let Some(reason) = crate::operation::brep_only(project, registry, op)
        .or_else(|| crate::operation::swarf_needs_faces(project, op))
    {
        notes.insert(0, format!("{}.", reason.replacen("swarf", "Swarf", 1)));
    }
    let reversing_inactive = matches!(
        op.parameters.get(names::RAMP_BACK_AND_FORTH),
        Some(ParameterValue::Flag(true))
    ) && !matches!(op.parameters.get(names::ENTRY),Some(ParameterValue::Text(style)) if style=="ramp");
    let from_data = [names::FEED, names::PLUNGE_FEED, names::STEPOVER];
    let fields = schema
        .parameters
        .iter()
        .map(|spec| {
            let set = op.parameters.get(&spec.name).cloned();
            let (kind, default) = match &spec.kind {
                ParameterKind::Number {
                    unit,
                    min,
                    min_exclusive,
                    max,
                    default,
                } => (
                    FieldKind::Number {
                        unit: unit.symbol(),
                        min: *min,
                        min_exclusive: *min_exclusive,
                        max: *max,
                    },
                    crate::operation::default_override(project, op, &spec.name)
                        .or(*default)
                        .map(ParameterValue::Number),
                ),
                ParameterKind::Integer { min, max, default } => (
                    FieldKind::Integer {
                        min: *min,
                        max: *max,
                    },
                    default.map(ParameterValue::Integer),
                ),
                ParameterKind::Flag { default } => {
                    (FieldKind::Flag, Some(ParameterValue::Flag(*default)))
                }
                ParameterKind::AngleList { default } => {
                    (FieldKind::Text, Some(ParameterValue::Text(default.clone())))
                }
                ParameterKind::IndexedFaces { default } => (
                    FieldKind::IndexedFaces,
                    Some(ParameterValue::Text(default.clone())),
                ),
                ParameterKind::Choice { options, default } => (
                    FieldKind::Choice {
                        options: options.clone(),
                    },
                    Some(ParameterValue::Text(default.clone())),
                ),
                // A kind added to the library later shows as text.
                _ => (
                    FieldKind::Choice {
                        options: Vec::new(),
                    },
                    None,
                ),
            };
            let data = (set.is_none() && from_data.contains(&spec.name.as_str()))
                .then(|| resolved.get(&spec.name).map(from_path))
                .flatten();
            let (value, source) = match (set.clone(), data, default) {
                (Some(v), _, _) => (Some(v), Source::Operation),
                (None, Some(v), _) => (Some(v), Source::CuttingData),
                (None, None, Some(v)) => (Some(v), Source::Default),
                (None, None, None) => (None, Source::Missing),
            };
            Field {
                section: FormSection::of(&spec.name),
                name: spec.name.clone(),
                label: if spec.name == names::RAMP_BACK_AND_FORTH && reversing_inactive {
                    format!("{} (inactive)", spec.label)
                } else {
                    facing_face_label(project, &op.strategy, &spec.name)
                        .unwrap_or_else(|| spec.label.clone())
                },
                description: spec.description.clone(),
                kind,
                set,
                value,
                source,
            }
        })
        .collect();
    Ok(ParameterForm {
        operation,
        strategy: op.strategy.clone(),
        fields,
        notes,
    })
}

// Canonical model faces use the same fixed order as 0244/SideSelection.
// Show the actual machine sense, leaving the face name alone if it is unknown.
fn facing_face_label(project: &Project, strategy: &str, name: &str) -> Option<String> {
    if strategy != openchuck_path::facing::NAME {
        return None;
    }
    let (face, angle) = match name {
        "positive_z" => ("+Z", 0.),
        "positive_y" => ("+Y", 90.),
        "negative_z" => ("−Z", 180.),
        "negative_y" => ("−Y", 270.),
        _ => return None,
    };
    let machine = project.machine()?;
    let a = if name == "positive_z" {
        0.
    } else {
        angle * machine.a_sense.sign()
    };
    Some(format!("{face} (A {a:.0}°)"))
}

/// Checks one value of `operation`'s strategy against its schema: the name
/// must exist, and the value must have the right kind and be in range.
/// `None` (clear the value) is always allowed: the cutting data or the
/// default then applies, and a value still missing is reported when the
/// operation is generated.
pub(crate) fn check_parameter(
    project: &Project,
    registry: &Registry,
    operation: OperationId,
    name: &str,
    value: Option<&ParameterValue>,
) -> Result<(), AppError> {
    let op = project
        .operation(operation)
        .ok_or_else(|| AppError::UnknownOperation(operation.to_string()))?;
    let strategy = registry.get(&op.strategy)?;
    let schema = strategy.schema();
    let mut proposed = crate::operation::stored_values(op);
    match value {
        Some(value) => {
            proposed.insert(name.to_owned(), to_path(value));
        }
        None => {
            proposed.remove(name);
        }
    }
    let active = strategy.schema_for(&proposed);
    // Visible mode-specific controls use their active range. Known inactive
    // values remain editable through the host API using their union spec.
    let spec = active
        .get(name)
        .or_else(|| schema.get(name))
        .ok_or_else(|| AppError::Operation {
            name: op.name.clone(),
            reason: format!("its strategy has no parameter '{name}'"),
        })?;
    let Some(value) = value else { return Ok(()) };
    let single = ParameterSchema {
        parameters: vec![spec.clone()],
    };
    let mut values = BTreeMap::new();
    values.insert(name.to_owned(), to_path(value));
    single.resolve(&values)?;
    Ok(())
}

#[cfg(test)]
mod section_tests {
    use super::FormSection;

    /// Each parameter goes in the inspector section of specification
    /// section 8; one the list does not name goes with the pattern.
    #[test]
    fn req_ui_07_parameters_are_placed_in_the_sections_of_section_8() {
        assert_eq!(FormSection::of("feed"), FormSection::ToolAndFeeds);
        assert_eq!(FormSection::of("max_tilt"), FormSection::ToolAxisAndA);
        assert_eq!(FormSection::of("entry"), FormSection::EntryLinksAndRetract);
        assert_eq!(FormSection::of("min_length"), FormSection::Advanced);
        assert_eq!(
            FormSection::of("stepover"),
            FormSection::PatternAndTolerance
        );
        assert_eq!(
            FormSection::of("a_new_one"),
            FormSection::PatternAndTolerance
        );
        let titles: Vec<&str> = FormSection::ALL.iter().map(|s| s.title()).collect();
        assert_eq!(
            titles,
            [
                "Tool and feeds",
                "Pattern and tolerance",
                "Tool axis and A",
                "Entry, links and retract",
                "Advanced"
            ]
        );
        let keys: Vec<&str> = FormSection::ALL.iter().map(|s| s.key()).collect();
        assert_eq!(keys, ["tool", "pattern", "axis", "links", "advanced"]);
    }
}
