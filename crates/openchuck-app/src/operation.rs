// SPDX-License-Identifier: AGPL-3.0-or-later
//! Operations: mapping project objects to the pathing library's plain data
//! and generating their toolpaths (OP-01, OP-11; work breakdown slice 5
//! items 8 and 10; decision record 0018 items 7 and 10).
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
    AcceptAll, AnalyticStock, MeshSurface, ParameterValue, Registry, RotaryMapping, StockModel,
    StrategyInput, SurfaceSource, generate, generate_in_region, generate_in_zones,
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

/// An operation's parameter values in the library's form, and what the
/// application did to get them.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedValues {
    /// The values, including any filled from the tool's cutting data.
    pub values: BTreeMap<String, ParameterValue>,
    /// Notes for the user, in parameter order: where the operation's own value
    /// overrides the material's cutting data (decision record 0018 item 7).
    pub notes: Vec<String>,
}

/// The operation's parameter values in the library's form, with the feed,
/// plunge feed and stepover filled from the tool's cutting data for the
/// setup's stock material where the operation leaves them out and the
/// strategy has them (TL-05).
///
/// A value the operation sets wins over the cutting data, and a note says so
/// when the two differ. When a value is required, the operation does not set
/// it and the cutting data cannot give it, the error names the material and
/// the tool, so the user knows whether to set the value or to add cutting
/// data for the material (names match exactly; OI-61).
pub fn parameter_values(
    project: &Project,
    operation: &Operation,
    registry: &Registry,
) -> Result<ResolvedValues, AppError> {
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
    let data = match (material, snapshot) {
        (Some(material), Some(snapshot)) => snapshot
            .cutting_data
            .get(material)
            .map(|data| (material, snapshot, data)),
        _ => None,
    };
    let mut notes = Vec::new();
    if let Some((material, snapshot, data)) = data {
        let from_data = [
            (names::FEED, data.cutting_feed(&snapshot.tool).0),
            (names::PLUNGE_FEED, data.plunge_feed.0),
            (names::STEPOVER, data.stepover.0),
        ];
        for (name, value) in from_data {
            let numeric = matches!(
                schema.get(name).map(|p| &p.kind),
                Some(ParameterKind::Number { .. })
            );
            if !numeric {
                continue;
            }
            match values.get(name) {
                None => {
                    values.insert(name.to_owned(), ParameterValue::Number(value));
                }
                Some(own) => {
                    let own = match own {
                        ParameterValue::Number(n) => Some(*n),
                        #[allow(clippy::cast_precision_loss)]
                        ParameterValue::Integer(i) => Some(*i as f64),
                        _ => None,
                    };
                    if let Some(own) = own
                        && (own - value).abs() > 1e-9 * value.abs().max(1.0)
                    {
                        notes.push(format!(
                            "{name} {own} set on the operation overrides {value} from the cutting \
                             data of '{}' for '{material}'",
                            snapshot.name
                        ));
                    }
                }
            }
        }
    }
    // A required value that neither the operation nor the cutting data gives.
    let missing: Vec<&str> = [names::FEED, names::PLUNGE_FEED, names::STEPOVER]
        .into_iter()
        .filter(|name| {
            !values.contains_key(*name)
                && matches!(
                    schema.get(name).map(|p| &p.kind),
                    Some(ParameterKind::Number { default: None, .. })
                )
        })
        .collect();
    if !missing.is_empty() {
        let why = match (material, snapshot) {
            (None, _) => "its setup has no stock material, so no cutting data applies".to_owned(),
            (Some(material), Some(snapshot)) => format!(
                "tool '{}' has no cutting data for the material '{material}' \
                 (names must match exactly)",
                snapshot.name
            ),
            (Some(_), None) => "it has no tool, so no cutting data applies".to_owned(),
        };
        return Err(AppError::Operation {
            name: operation.name.clone(),
            reason: format!(
                "{} {} required: set {} on the operation, or fix the cause: {why}",
                missing.join(", "),
                if missing.len() == 1 { "is" } else { "are" },
                if missing.len() == 1 { "it" } else { "them" },
            ),
        });
    }
    Ok(ResolvedValues { values, notes })
}

/// An operation's toolpath group and notes about it for the user.
#[derive(Debug, Clone, PartialEq)]
pub struct OperationPath {
    /// The moves.
    pub group: openchuck_types::ir::ToolpathGroup,
    /// Notes: where the operation's values override the material's cutting
    /// data, then moves the machine's feed limits slow down.
    pub notes: Vec<String>,
    /// The tool's revolved profile, as the simulation sweeps it.
    pub tool: openchuck_types::tool::RevolvedProfile,
    /// The bounds of the setup's placed part (for bounding-box stock).
    pub part_bounds: openchuck_types::geometry::Aabb,
}

/// Generates the toolpath of operation `id` of the project in `root`.
///
/// Fails with a message naming the operation when a piece is missing (the
/// machine profile, the setup's part or stock, the tool), when the strategy
/// is unknown, a selection set it uses cannot be resolved (a changed model, a
/// missing triangle, an empty set), or its strategy cannot be limited to
/// selections; each is an error that says which, never a silent drop (OP-02).
pub fn generate_operation(
    root: &Path,
    project: &Project,
    id: OperationId,
    registry: &Registry,
    context: &JobContext<'_>,
) -> Result<OperationPath, AppError> {
    generate_operation_on(root, project, id, registry, None, context)
}

/// As [`generate_operation`], against `stock` when given: the material the
/// operations before it in the setup left, as the simulation carries it
/// (SIM-06). Without it the setup's stock definition is used.
pub fn generate_operation_on(
    root: &Path,
    project: &Project,
    id: OperationId,
    registry: &Registry,
    stock: Option<&dyn StockModel>,
    context: &JobContext<'_>,
) -> Result<OperationPath, AppError> {
    let carried = stock;
    let operation = project
        .operation(id)
        .ok_or_else(|| AppError::UnknownOperation(id.to_string()))?;
    let strategy = registry.get(&operation.strategy)?;
    let limited = operation.selection_refs().next().is_some();
    if limited && !strategy.honours_region() {
        return Err(missing(
            operation,
            &format!(
                "the strategy '{}' cannot yet be limited to selections (OP-02); remove \
                 the operation's selections to machine the whole part",
                operation.strategy
            ),
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

    let (mesh, file_hash) = crate::model::placed_mesh_hashed(root, model, context)?;
    context.check_cancelled()?;
    let surface = MeshSurface::new(&mesh)?;
    let resolved_region = crate::selection::resolve(project, operation, model, file_hash, &mesh)?;
    let constraint = resolved_region.as_ref().map(|r| r.constraint());
    let analytic = AnalyticStock::from_stock(stock, Some(surface.bounds()))?;
    let stock: &dyn StockModel = carried.unwrap_or(&analytic);
    let profile = snapshot.tool.profile()?;
    let tags = PathTags {
        operation: operation.id.to_string(),
        setup: setup.id.to_string(),
        tool: snapshot.id.to_string(),
    };
    let input = StrategyInput {
        surface: &surface,
        stock,
        collision: &AcceptAll,
        tool: &profile,
        rotary: RotaryMapping::of_profile(machine),
        clearance: setup.clearance.unwrap_or(machine.clearance),
        max_feeds: machine.max_feeds,
        tags: &tags,
    };
    let resolved = parameter_values(project, operation, registry)?;
    let zones = resolved_region.as_ref().map_or_else(Vec::new, |r| {
        let leave = match resolved.values.get(names::STOCK_TO_LEAVE) {
            Some(openchuck_path::ParameterValue::Number(v)) => *v,
            _ => 0.0,
        };
        r.zones(leave)
    });
    if !zones.is_empty() && !strategy.honours_leave_zones() {
        return Err(missing(
            operation,
            &format!(
                "the strategy '{}' cannot leave different stock on different selections \
                 (OP-11); clear the stock to leave of the selection sets",
                operation.strategy
            ),
        ));
    }
    let path = if zones.is_empty() {
        match &constraint {
            Some(region) => {
                generate_in_region(strategy, &input, region, &resolved.values, context)?
            }
            None => generate(strategy, &input, &resolved.values, context)?,
        }
    } else {
        generate_in_zones(
            strategy,
            &input,
            constraint.as_ref(),
            &zones,
            &resolved.values,
            context,
        )?
    };
    // Notes about the values come first, then the notes about moves.
    let mut notes = resolved.notes;
    notes.extend(
        path.notes
            .iter()
            .map(|n| format!("move {}: {}", n.segment, n.message)),
    );
    Ok(OperationPath {
        group: path.group,
        notes,
        tool: profile,
        part_bounds: surface.bounds(),
    })
}
