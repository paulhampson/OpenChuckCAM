// SPDX-License-Identifier: AGPL-3.0-or-later
//! The values an operation or a setup needs before it can be generated, as
//! one list (lane BUG-STALE part 2; decision 0173; requirements UI-07,
//! UI-10, UI-11, NFR-07; issues 210, 224 and 140).
//!
//! The same list drives the marks in the inspectors, the colour of the step
//! bar, and whether Post and export is offered. It comes from the project and
//! from each strategy's registered parameter schema (a number with no default
//! is required), not from a list in the interface, so a new strategy supplies
//! its own. Generation checks the same values and still stops with its own
//! message (NFR-07); the list never replaces that check.

use openchuck_path::params::{ParameterKind, ParameterSchema};
use openchuck_project::id::{OperationId, SetupId};
use openchuck_project::{Operation, Project};
use openchuck_types::units::SpindleSpeed;

use crate::Application;

/// Which value is missing.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RequirementField {
    /// The project has no machine profile.
    Machine,
    /// The setup has no stock.
    Stock,
    /// The setup has no part (model) to machine.
    Part,
    /// The operation has no tool.
    Tool,
    /// The operation has no spindle speed: it gave none and the tool has no
    /// cutting data for the setup's material (decision 0196).
    SpindleSpeed,
    /// A parameter of the operation's strategy that is required and that
    /// neither the operation nor the tool's cutting data gives (by name).
    Parameter(String),
}

/// One missing value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Requirement {
    /// Which value.
    pub field: RequirementField,
    /// The field's name as the interface labels it.
    pub label: String,
    /// A short plain sentence for under the field ("Choose a tool").
    pub text: String,
}

impl Requirement {
    fn new(field: RequirementField, label: &str, text: &str) -> Self {
        Self {
            field,
            label: label.to_owned(),
            text: text.to_owned(),
        }
    }
}

/// The cutting data of the operation's tool for its setup's stock material,
/// with the tool and the material, when there is one.
pub(crate) fn cutting_data<'a>(
    project: &'a Project,
    operation: &Operation,
) -> Option<(
    &'a str,
    &'a openchuck_project::ToolSnapshot,
    &'a openchuck_types::tool::CuttingData,
)> {
    let material = project
        .setup(operation.setup)
        .and_then(|s| s.material.as_deref())?;
    let snapshot = operation
        .tool
        .and_then(|id| project.tools().iter().find(|t| t.id == id))?;
    let (_, data) = openchuck_types::tool::find_cutting_data(&snapshot.cutting_data, material)?;
    Some((material, snapshot, data))
}

/// Where an operation's spindle speed comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SpindleSource {
    /// The operation's own value.
    Operation,
    /// The tool's cutting data for the setup's material.
    ToolData,
}

/// The spindle speed of `operation` and where it comes from: its own value,
/// else the tool's cutting data for the setup's material, else none. There
/// is no machine default (decision 0196).
pub(crate) fn spindle_speed(
    project: &Project,
    operation: &Operation,
) -> Option<(SpindleSpeed, SpindleSource)> {
    if let Some(speed) = operation.spindle_speed {
        return Some((speed, SpindleSource::Operation));
    }
    cutting_data(project, operation).map(|(_, _, d)| (d.spindle_speed, SpindleSource::ToolData))
}

/// Why `operation` has no spindle speed and what to do about it, in plain
/// words; names the operation, the tool and the material.
pub(crate) fn missing_spindle_message(project: &Project, operation: &Operation) -> String {
    let tool = operation
        .tool
        .and_then(|id| project.tools().iter().find(|t| t.id == id))
        .map(|t| t.name.as_str());
    let material = project
        .setup(operation.setup)
        .and_then(|s| s.material.as_deref());
    let fix = match (tool, material) {
        (Some(tool), Some(material)) => format!(
            "Enter a spindle speed for this operation, or add cutting data for \"{material}\" to the tool \"{tool}\" in the tool library"
        ),
        (Some(tool), None) => format!(
            "Enter a spindle speed for this operation, or choose the stock material in the setup and add cutting data for it to the tool \"{tool}\" in the tool library"
        ),
        (None, _) => "Choose a tool, then enter a spindle speed for this operation".to_owned(),
    };
    format!("The operation \"{}\" has no spindle speed. {fix}.", operation.name)
}

/// The names of the parameters the cutting data can give.
pub(crate) const FROM_CUTTING_DATA: [&str; 4] = [
    openchuck_path::params::names::FEED,
    openchuck_path::params::names::PLUNGE_FEED,
    openchuck_path::params::names::STEPOVER,
    openchuck_path::roughing::parameter::STEPDOWN,
];

/// The required parameters of `schema` that `operation` leaves out and the
/// cutting data does not give, in the schema's order. Generation uses this
/// list for its "is required" error, so the two cannot disagree.
pub(crate) fn missing_parameters(
    project: &Project,
    operation: &Operation,
    schema: &ParameterSchema,
) -> Vec<(String, String)> {
    let given = cutting_data(project, operation).is_some();
    schema
        .parameters
        .iter()
        .filter(|spec| {
            matches!(spec.kind, ParameterKind::Number { default: None, .. })
                && !operation.parameters.contains_key(&spec.name)
                && !(given && FROM_CUTTING_DATA.contains(&spec.name.as_str()))
                && crate::operation::default_override(project, operation, &spec.name).is_none()
        })
        .map(|spec| (spec.name.clone(), spec.label.clone()))
        .collect()
}

/// What `setup` needs: a machine for the project, and a stock and a part.
pub fn setup_requirements(project: &Project, setup: SetupId) -> Vec<Requirement> {
    let Some(setup) = project.setup(setup) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if project.machine().is_none() {
        out.push(Requirement::new(
            RequirementField::Machine,
            "Machine",
            "Choose a machine",
        ));
    }
    if setup.stock.is_none() {
        out.push(Requirement::new(
            RequirementField::Stock,
            "Stock",
            "Set the stock",
        ));
    }
    if setup.part.is_none() {
        out.push(Requirement::new(
            RequirementField::Part,
            "Part",
            "Choose the part",
        ));
    }
    out
}

impl Application {
    /// What `operation` needs before it can be generated: a tool, and each
    /// required parameter of its strategy that nothing gives. Empty when the
    /// operation is complete or is not in the project.
    pub fn operation_requirements(&self, operation: OperationId) -> Vec<Requirement> {
        operation_requirements(&self.document.project, &self.strategies, operation)
    }

    /// What the setup needs (see [`setup_requirements`]).
    pub fn setup_requirements(&self, setup: SetupId) -> Vec<Requirement> {
        setup_requirements(&self.document.project, setup)
    }
}

pub(crate) fn operation_requirements(
    project: &Project,
    registry: &openchuck_path::Registry,
    operation: OperationId,
) -> Vec<Requirement> {
    let Some(op) = project.operation(operation) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let no_tool = !op
        .tool
        .is_some_and(|id| project.tools().iter().any(|t| t.id == id));
    if no_tool {
        out.push(Requirement::new(
            RequirementField::Tool,
            "Tool",
            "Choose a tool",
        ));
    }
    if !no_tool && spindle_speed(project, op).is_none() {
        out.push(Requirement::new(
            RequirementField::SpindleSpeed,
            "Spindle speed",
            "Enter the spindle speed",
        ));
    }
    // An unknown strategy is reported when the operation is generated.
    if let Ok(strategy) = registry.get(&op.strategy) {
        // In the order the inspector shows them, so "Show first" goes to the
        // field nearest the top (a stable sort keeps the schema's order
        // within a section).
        let mut parameters = missing_parameters(project, op, &strategy.schema());
        parameters.sort_by_key(|(name, _)| crate::form::FormSection::of(name));
        for (name, label) in parameters {
            // The feeds and the stepover come from the tool's cutting data:
            // until a tool is chosen the tool is what is missing, and one line
            // keeps a fresh operation calm (issue 224).
            if no_tool && FROM_CUTTING_DATA.contains(&name.as_str()) {
                continue;
            }
            out.push(Requirement {
                text: format!("Enter {}", label.to_lowercase()),
                field: RequirementField::Parameter(name),
                label,
            });
        }
    }
    out
}
