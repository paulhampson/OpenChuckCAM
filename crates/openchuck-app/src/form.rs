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
    let schema = registry.get(&op.strategy)?.schema();
    let (resolved, mut notes) = match parameter_values(project, op, registry) {
        Ok(r) => (r.values, r.notes),
        Err(e) => (BTreeMap::new(), vec![e.to_string()]),
    };
    // Said in the form, before the operation is run (OI-96, OI-238;
    // decision 0111 items 1 and 3).
    if let Some(reason) = crate::operation::brep_only(project, op)
        .or_else(|| crate::operation::swarf_needs_faces(project, op))
    {
        notes.insert(0, format!("{}.", reason.replacen("swarf", "Swarf", 1)));
    }
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
                name: spec.name.clone(),
                label: spec.label.clone(),
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
    let schema = registry.get(&op.strategy)?.schema();
    let spec = schema.get(name).ok_or_else(|| AppError::Operation {
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
