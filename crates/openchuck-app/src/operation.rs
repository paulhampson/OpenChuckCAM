// SPDX-License-Identifier: AGPL-3.0-or-later
//! Operations: mapping project objects to the pathing library's plain data
//! and generating their toolpaths (OP-01, OP-11; work breakdown slice 5
//! items 8 and 10; decision record 0016 items 7 and 10).
//!
//! The library never sees the project file (INT-01): this module reads the
//! operation's setup, machine, tool snapshot and part, and hands
//! `openchuck-path` a mesh, stock, tool profile, rotary mapping, clearance
//! and parameter values. An operation's feed, plunge feed and stepover,
//! when it does not set them, come from its tool's cutting data for the
//! setup's stock material (TL-05).

use std::collections::BTreeMap;
use std::path::Path;

use openchuck_path::params::{ParameterKind, names};
use openchuck_path::{
    AcceptAll, AnalyticStock, GeneratedPath, MeshSurface, ParameterValue, Registry, RotaryMapping,
    StrategyInput, SurfaceSource, generate,
};
use openchuck_project::id::OperationId;
use openchuck_project::{Operation, Project};
use openchuck_types::ir::PathTags;
use openchuck_types::job::JobContext;

use crate::AppError;

fn missing(operation: &Operation, what: &str) -> AppError {
    AppError::Operation {
        name: operation.name.clone(),
        reason: what.to_owned(),
    }
}

/// The operation's parameter values in the library's form, with the feed,
/// plunge feed and stepover filled from the tool's cutting data for
/// `material` where the operation leaves them out and the strategy has them.
pub fn parameter_values(
    project: &Project,
    operation: &Operation,
    registry: &Registry,
) -> Result<BTreeMap<String, ParameterValue>, AppError> {
    let strategy = registry.get(&operation.strategy)?;
    let schema = strategy.schema();
    let mut values: BTreeMap<String, ParameterValue> = operation
        .parameters
        .iter()
        .map(|(k, v)| {
            let v = match v {
                openchuck_project::ParameterValue::Integer(i) => ParameterValue::Integer(*i),
                openchuck_project::ParameterValue::Number(n) => ParameterValue::Number(*n),
                openchuck_project::ParameterValue::Flag(b) => ParameterValue::Flag(*b),
                openchuck_project::ParameterValue::Text(t) => ParameterValue::Text(t.clone()),
            };
            (k.clone(), v)
        })
        .collect();
    let material = project
        .setup(operation.setup)
        .and_then(|s| s.material.as_deref());
    let snapshot = operation
        .tool
        .and_then(|id| project.tools().iter().find(|t| t.id == id));
    if let (Some(material), Some(snapshot)) = (material, snapshot)
        && let Some(data) = snapshot.cutting_data.get(material)
    {
        let defaults = [
            (names::FEED, data.cutting_feed(&snapshot.tool).0),
            (names::PLUNGE_FEED, data.plunge_feed.0),
            (names::STEPOVER, data.stepover.0),
        ];
        for (name, value) in defaults {
            let numeric = matches!(
                schema.get(name).map(|p| &p.kind),
                Some(ParameterKind::Number { .. })
            );
            if numeric && !values.contains_key(name) {
                values.insert(name.to_owned(), ParameterValue::Number(value));
            }
        }
    }
    Ok(values)
}

/// Generates the toolpath of operation `id` of the project in `root`.
///
/// Fails with a message naming the operation when a piece is missing (the
/// machine profile, the setup's part or stock, the tool), when the strategy
/// is unknown, or when the operation is limited to selections, which
/// arrives with OP-02 (slice 14) and is refused rather than ignored.
pub fn generate_operation(
    root: &Path,
    project: &Project,
    id: OperationId,
    registry: &Registry,
    context: &JobContext<'_>,
) -> Result<GeneratedPath, AppError> {
    let operation = project
        .operation(id)
        .ok_or_else(|| AppError::UnknownOperation(id.to_string()))?;
    let strategy = registry.get(&operation.strategy)?;
    if !operation.selections.is_empty() {
        return Err(missing(
            operation,
            "limiting an operation to selections is not built yet (OP-02, slice 14); \
             remove its selections to machine the whole part",
        ));
    }
    let machine = project
        .machine()
        .ok_or_else(|| missing(operation, "the project has no machine profile"))?;
    let setup = project
        .setup(operation.setup)
        .ok_or_else(|| missing(operation, "its setup is missing"))?;
    let tool_id = operation
        .tool
        .ok_or_else(|| missing(operation, "it has no tool"))?;
    let snapshot = project
        .tools()
        .iter()
        .find(|t| t.id == tool_id)
        .ok_or_else(|| missing(operation, "its tool is missing"))?;
    let part_id = setup
        .part
        .ok_or_else(|| missing(operation, "its setup has no part"))?;
    let model = project
        .models()
        .iter()
        .find(|m| m.id == part_id)
        .ok_or_else(|| missing(operation, "its setup's part is missing"))?;
    let stock = setup
        .stock
        .as_ref()
        .ok_or_else(|| missing(operation, "its setup has no stock"))?;

    let mesh = crate::model::placed_mesh(root, model, context)?;
    context.check_cancelled()?;
    let surface = MeshSurface::new(&mesh)?;
    let stock = AnalyticStock::from_stock(stock, Some(surface.bounds()))?;
    let profile = snapshot.tool.profile()?;
    let tags = PathTags {
        operation: operation.id.to_string(),
        setup: setup.id.to_string(),
        tool: snapshot.id.to_string(),
    };
    let input = StrategyInput {
        surface: &surface,
        stock: &stock,
        collision: &AcceptAll,
        tool: &profile,
        rotary: RotaryMapping::of_profile(machine),
        clearance: setup.clearance.unwrap_or(machine.clearance),
        max_feeds: machine.max_feeds,
        tags: &tags,
    };
    let values = parameter_values(project, operation, registry)?;
    Ok(generate(strategy, &input, &values, context)?)
}
