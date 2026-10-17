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
use std::sync::Arc;

use openchuck_path::params::{EntryParameters, ParameterKind, names};
use openchuck_path::roughing::parameter::STEPDOWN;
use openchuck_path::{
    AcceptAll, AnalyticStock, BrepEdges, BrepSurface, CompositeSurface, CurveSource, EdgeFollowing,
    MeshSurface, ParameterValue, Registry, RotaryMapping, StockModel, Strategy, StrategyInput,
    SurfaceSource, drill, edge_following, generate, generate_in_region, generate_in_zones,
};
use openchuck_project::id::OperationId;
use openchuck_project::{ContentHash, ModelRef, Operation, Project, SelectionTarget};
use openchuck_types::brep::{BrepModel, EdgeId, FaceId};
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
            (STEPDOWN, data.stepdown.0),
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
    let missing: Vec<&str> = [names::FEED, names::PLUNGE_FEED, names::STEPOVER, STEPDOWN]
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
    generate_with_part(root, project, id, registry, stock, None, None, context)
}

/// How edge following builds the B-rep edges of a STEP part, beyond the
/// operation's parameters (decision 0057).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EdgeOptions {
    /// Keep the edges between tangent faces (the boundaries of fillets, the
    /// line where two halves of a cylinder meet): the B-rep source is built
    /// with a crease angle of zero, whatever the operation's crease angle. A
    /// mesh part has no such choice.
    pub keep_tangent: bool,
}

/// The edge options of `operation` as the project holds them: the chosen
/// `Edges` set's own setting if it has one, else the project's default, else
/// off (decision 0057). Neither is stored yet: the per-selection setting and
/// the project default are lane O's project schema 12 work (OI-232), so this
/// answers off until lane O wires it here.
pub fn edge_options(project: &Project, operation: &Operation) -> EdgeOptions {
    EdgeOptions {
        keep_tangent: keep_tangent_edges(project, operation),
    }
}

/// Whether edge following on `operation` keeps tangent edges: the seam lane
/// O wires to the stored per-selection setting and project default (decision
/// 0057, OI-232). Off until then, and always off on a mesh part.
fn keep_tangent_edges(project: &Project, operation: &Operation) -> bool {
    let _ = (project, operation);
    false
}

/// As [`generate_operation_on`], with the edge options given rather than
/// read from the project (decision 0057). Fails when `edges` keeps tangent
/// edges on an operation whose part is a mesh, which has no such choice.
pub fn generate_operation_with(
    root: &Path,
    project: &Project,
    id: OperationId,
    registry: &Registry,
    stock: Option<&dyn StockModel>,
    edges: EdgeOptions,
    context: &JobContext<'_>,
) -> Result<OperationPath, AppError> {
    generate_with_part(
        root,
        project,
        id,
        registry,
        stock,
        None,
        Some(edges),
        context,
    )
}

/// As [`generate_operation_on`], with the setup's part already placed when
/// `placed` is given (the tests hand in a B-rep built without the kernel),
/// and the edge options when `edges` is given (otherwise the project's).
#[allow(clippy::too_many_arguments)]
pub(crate) fn generate_with_part(
    root: &Path,
    project: &Project,
    id: OperationId,
    registry: &Registry,
    stock: Option<&dyn StockModel>,
    placed: Option<crate::model::PlacedPart>,
    edges: Option<EdgeOptions>,
    context: &JobContext<'_>,
) -> Result<OperationPath, AppError> {
    let carried = stock;
    let operation = project
        .operation(id)
        .ok_or_else(|| AppError::UnknownOperation(id.to_string()))?;
    let edges = edges.unwrap_or_else(|| edge_options(project, operation));
    let strategy = registry.get(&operation.strategy)?;
    let limited = operation.selection_refs().next().is_some();
    // A STEP part's selections are faces and edges, which choose the holes
    // and the edges below (decision 0056); a mesh part's are triangles.
    if limited && !on_step_part(project, operation) && !strategy.honours_region() {
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

    let placed = match placed {
        Some(placed) => placed,
        None => crate::model::placed_part(root, model, context)?,
    };
    if edges.keep_tangent && placed.brep.is_none() {
        return Err(missing(
            operation,
            "keeping tangent edges is a choice for the B-rep edges of a STEP part; its part is \
             a mesh, whose edges are chosen by the crease angle",
        ));
    }
    let (mesh, file_hash) = (&placed.mesh, placed.hash);
    context.check_cancelled()?;
    // A STEP part is machined on its exact faces (IMP-02; decision 0056).
    let brep_choice = match &placed.brep {
        Some(brep) => Some(brep_selection(project, operation, model, file_hash, brep)?),
        None => None,
    };
    let model_surface: Arc<dyn SurfaceSource> = match (&placed.brep, &brep_choice) {
        (Some(brep), Some(choice)) if !choice.faces.is_empty() => {
            Arc::new(BrepSurface::with_faces(brep, &choice.faces)?)
        }
        (Some(brep), _) => Arc::new(BrepSurface::new(brep)?),
        (None, _) => Arc::new(MeshSurface::new(mesh)?),
    };
    let part_bounds = model_surface.bounds();
    let profile = snapshot.tool.profile()?;
    // The held regions and tabs join the model's surface as the target the
    // strategies see (SET-09, OP-21; decision 0035 item 1). A setup without
    // them uses the model's own surface, so its G-code is unchanged.
    let mut reach_notes = Vec::new();
    let composite = if setup.workholding.is_empty() {
        None
    } else {
        let part = openchuck_geom::part_extent(mesh)
            .ok_or_else(|| missing(operation, "its setup's part has no extent"))?;
        crate::workholding::check_for_tool(
            &setup.workholding,
            setup.stock.as_ref(),
            &part,
            2.0 * profile.cutting_radius(),
            &snapshot.name,
        )
        .map_err(|e| missing(operation, &e.to_string()))?;
        let labelled =
            crate::workholding::labelled_solids(&setup.workholding, setup.stock.as_ref(), &part)
                .map_err(|e| missing(operation, &e.to_string()))?;
        if placed.brep.is_none() {
            reach_notes =
                crate::workholding::selection_reach_notes(project, operation, mesh, &labelled);
        }
        Some(CompositeSurface::new(
            model_surface.clone(),
            labelled.into_iter().map(|(_, s)| s).collect(),
        ))
    };
    let surface: &dyn SurfaceSource = match &composite {
        Some(c) => c,
        None => model_surface.as_ref(),
    };
    let resolved_region = match placed.brep {
        Some(_) => None,
        None => crate::selection::resolve(project, operation, model, file_hash, mesh)?,
    };
    let constraint = resolved_region.as_ref().map(|r| r.constraint());
    let analytic = AnalyticStock::from_stock(stock, Some(part_bounds))?;
    let stock: &dyn StockModel = carried.unwrap_or(&analytic);
    let tags = PathTags {
        operation: operation.id.to_string(),
        setup: setup.id.to_string(),
        tool: snapshot.id.to_string(),
    };
    let input = StrategyInput {
        surface,
        stock,
        collision: &AcceptAll,
        tool: &profile,
        rotary: RotaryMapping::of_profile(machine),
        clearance: setup.clearance.unwrap_or(machine.clearance),
        max_feeds: machine.max_feeds,
        tags: &tags,
    };
    let resolved = parameter_values(project, operation, registry)?;
    // The values as the strategy's schema reads them (a whole number given for
    // a number is a number), resolved once for everything below.
    let parameters = strategy.schema().resolve(&resolved.values)?;
    // A straight plunge needs a tool that can cut straight down (OI-117).
    EntryParameters::read(&parameters)?.check_tool(snapshot.tool.can_plunge())?;
    let operation_leave = parameters.number(names::STOCK_TO_LEAVE)?;
    let zones = resolved_region
        .as_ref()
        .map_or_else(Vec::new, |r| r.zones(operation_leave));
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
    // Edge following on a STEP part follows the B-rep's own edges, with the
    // operation's crease angle and shortest chain (OP-08; decision 0053 item 7,
    // decision 0056).
    let brep_edges = match (&placed.brep, &brep_choice) {
        (Some(brep), Some(choice)) if operation.strategy == edge_following::NAME => Some(
            brep_curves(operation, brep, &choice.edges, &parameters, edges)?,
        ),
        _ => None,
    };
    let registry_strategy = strategy;
    let with_edges;
    let strategy: &dyn Strategy = match brep_edges {
        Some(source) => {
            with_edges = EdgeFollowing::with_source(source);
            &with_edges
        }
        None => strategy,
    };
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
    notes.extend(reach_notes);
    // The registry's strategy: a B-rep edge source uses the crease angle and
    // the shortest chain, so the warning that they are not used does not apply.
    notes.extend(registry_strategy.parameter_warnings(&profile, &parameters));
    // Rest machining and pencil clean-up work on what earlier operations
    // left (OP-09, SIM-06); against the setup's own stock the whole part has
    // material left, which is rarely what was meant.
    if carried.is_none() && strategy.needs_earlier_operations() {
        notes.push(format!(
            "{} cleans up material that earlier operations left, and no earlier operation of \
             this setup has cut: the setup's own stock is taken, so all of the part counts as \
             having material left",
            strategy.label()
        ));
    }
    notes.extend(
        path.notes
            .iter()
            .map(|n| format!("move {}: {}", n.segment, n.message)),
    );
    Ok(OperationPath {
        group: path.group,
        notes,
        tool: profile,
        part_bounds,
    })
}

/// True when the part of `operation`'s setup is a STEP model, by its file
/// name, without reading it.
pub(crate) fn on_step_part(project: &Project, operation: &Operation) -> bool {
    project
        .setup(operation.setup)
        .and_then(|s| s.part)
        .and_then(|id| project.models().iter().find(|m| m.id == id))
        .is_some_and(|m| {
            openchuck_types::model::ModelFormat::from_file_name(m.path.as_str())
                .is_some_and(|f| f.kind() == openchuck_types::model::ModelKind::Brep)
        })
}

/// The default of parameter `name` of `operation` where the application's
/// differs from the strategy's schema: edge following on a STEP part takes
/// the B-rep crease angle (decision 0056).
pub(crate) fn default_override(
    project: &Project,
    operation: &Operation,
    name: &str,
) -> Option<f64> {
    (operation.strategy == edge_following::NAME
        && name == edge_following::parameter::CREASE_ANGLE
        && on_step_part(project, operation))
    .then_some(openchuck_path::brep_edges::DEFAULT_CREASE_DEGREES)
}

/// What an operation on a STEP part selects: the faces whose holes radial
/// drilling drills, or the edges edge following follows (OP-02, OP-08,
/// OP-10). Empty lists mean the whole part.
struct BrepChoice {
    faces: Vec<FaceId>,
    edges: Vec<EdgeId>,
}

/// Reads the selections of `operation` on its STEP part `part` (decision
/// 0056). Only face sets for radial drilling and edge sets for edge
/// following are honoured so far; anything else is an error that says so,
/// never a silent drop (OP-02).
fn brep_selection(
    project: &Project,
    operation: &Operation,
    part: &ModelRef,
    hash: ContentHash,
    brep: &BrepModel,
) -> Result<BrepChoice, AppError> {
    let fail = |reason: String| missing(operation, &reason);
    if operation.boundary.is_some() || !operation.avoid.is_empty() || !operation.check.is_empty() {
        return Err(fail(
            "boundaries, avoid and check surfaces are not built for STEP models yet; remove them \
             from the operation"
                .to_owned(),
        ));
    }
    let mut choice = BrepChoice {
        faces: Vec::new(),
        edges: Vec::new(),
    };
    for id in &operation.selections {
        let set = project
            .selection_sets()
            .iter()
            .find(|s| s.id == *id)
            .ok_or_else(|| AppError::UnknownSelection(id.to_string()))?;
        let bad = |reason: String| {
            fail(format!(
                "selection set '{}' cannot be used: {reason}",
                set.name
            ))
        };
        if set.model != part.id {
            return Err(bad(
                "it was made on a different model from the setup's part".to_owned(),
            ));
        }
        if set.model_hash != hash {
            return Err(bad(format!(
                "it was made on a different version of '{}' (the file has changed); select \
                 again on the current model",
                part.path.as_str()
            )));
        }
        match &set.target {
            SelectionTarget::Faces(faces) if faces.is_empty() => {
                return Err(bad(
                    "it selects no faces; select the hole walls to drill, or take the set out \
                     of the operation to drill every hole"
                        .to_owned(),
                ));
            }
            SelectionTarget::Edges(edges) if edges.is_empty() => {
                return Err(bad(
                    "it selects no edges; select the edges to follow, or take the set out of \
                     the operation to follow every edge"
                        .to_owned(),
                ));
            }
            SelectionTarget::Faces(faces) => {
                if let Some(face) = faces
                    .iter()
                    .find(|f| !brep.faces.iter().any(|g| g.id == **f))
                {
                    return Err(bad(format!("face {} is not in the model", face.0)));
                }
                choice.faces.extend(faces.iter().copied());
            }
            SelectionTarget::Edges(edges) => {
                if let Some(edge) = edges
                    .iter()
                    .find(|e| !brep.edges.iter().any(|g| g.id == **e))
                {
                    return Err(bad(format!("edge {} is not in the model", edge.0)));
                }
                choice.edges.extend(edges.iter().copied());
            }
            // The project refuses a triangle set on a STEP model.
            SelectionTarget::Triangles(_) => {
                return Err(bad(
                    "it selects triangles, and a STEP model is selected by face and edge"
                        .to_owned(),
                ));
            }
        }
    }
    choice.faces.sort_unstable();
    choice.faces.dedup();
    choice.edges.sort_unstable();
    choice.edges.dedup();
    if !choice.faces.is_empty() && operation.strategy != drill::NAME {
        return Err(fail(format!(
            "the strategy '{}' cannot yet be limited to faces of a STEP model (OP-02); only \
             radial drilling uses selected faces, to choose its holes",
            operation.strategy
        )));
    }
    if !choice.edges.is_empty() && operation.strategy != edge_following::NAME {
        return Err(fail(format!(
            "the strategy '{}' does not use selected edges; only edge following does",
            operation.strategy
        )));
    }
    Ok(choice)
}

/// The edges edge following follows on a STEP part: the chosen edges, or
/// every real edge, at the operation's crease angle (the B-rep default of
/// [`openchuck_path::brep_edges::DEFAULT_CREASE_DEGREES`] when the operation
/// does not set one) and its shortest chain (decision 0056); at a crease
/// angle of zero when `options` keeps tangent edges (decision 0057).
fn brep_curves(
    operation: &Operation,
    brep: &BrepModel,
    edges: &[EdgeId],
    parameters: &openchuck_path::params::Parameters,
    options: EdgeOptions,
) -> Result<Arc<dyn CurveSource>, AppError> {
    use openchuck_path::edge_following::parameter::{CREASE_ANGLE, MIN_LENGTH};
    let source = if edges.is_empty() {
        BrepEdges::new(brep)?
    } else {
        BrepEdges::with_edges(brep, edges)?
    };
    let crease = if options.keep_tangent {
        0.0
    } else if operation.parameters.contains_key(CREASE_ANGLE) {
        parameters.number(CREASE_ANGLE)?
    } else {
        openchuck_path::brep_edges::DEFAULT_CREASE_DEGREES
    };
    Ok(source
        .with_crease(crease)
        .with_min_length(parameters.number(MIN_LENGTH)?)
        .shared())
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    //! The B-rep path through the pipeline without the STEP kernel: a
    //! closed-form drilled blank (`openchuck_testkit::brep`) is handed in as
    //! the placed part of a project whose setup names a STEP file (decision
    //! 0056). The kernel tests in `tests/step_pipeline.rs` read the same path
    //! from a real STEP file.

    use std::collections::{BTreeMap, BTreeSet};

    use openchuck_project::id::{ModelId, SelectionSetId, SetupId, ToolId};
    use openchuck_project::{
        Item, OrderKey, ParameterValue as Value, Placement, RelativePath, SelectionSet, Setup,
        ToolSnapshot,
    };
    use openchuck_testkit::brep::{Built, HoleSpec, drilled_blank};
    use openchuck_types::geometry::Point3;
    use openchuck_types::job::{CancelToken, NoProgress};
    use openchuck_types::stock::Stock;
    use openchuck_types::tool::{ControllerMapping, Tool, ToolShape};
    use openchuck_types::transform::TransformStep;
    use openchuck_types::units::{Degrees, Millimetres};

    use super::*;
    use crate::model::PlacedPart;

    const RADIUS: f64 = 12.0;
    const LENGTH: f64 = 60.0;
    const DIAMETER: f64 = 4.0;
    /// The placement: the blank moved 5 mm along the rotary axis.
    const SHIFT: f64 = 5.0;

    struct Fixture {
        project: Project,
        operation: OperationId,
        model: ModelId,
        hash: ContentHash,
        brep: BrepModel,
        built: Vec<Built>,
    }

    fn polar(phi: f64) -> [f64; 3] {
        [0.0, phi.to_radians().cos(), phi.to_radians().sin()]
    }

    fn fixture(strategy: &str, parameters: &[(&str, Value)]) -> Fixture {
        fixture_with(
            strategy,
            parameters,
            &[
                HoleSpec::blind(15.0, polar(90.0), DIAMETER, 6.0),
                HoleSpec::through(30.0, polar(0.0), DIAMETER),
                HoleSpec::blind(45.0, polar(225.0), DIAMETER, 4.0),
            ],
            "blank.step",
        )
    }

    fn fixture_with(
        strategy: &str,
        parameters: &[(&str, Value)],
        holes: &[HoleSpec],
        file: &str,
    ) -> Fixture {
        let (brep, built) = drilled_blank(RADIUS, LENGTH, holes);
        let hash = ContentHash::of_bytes(b"blank.step");
        let mut project = Project::new("blank");
        project
            .set_machine(Some(crate::setup::builtin("makera-z1").unwrap().unwrap()))
            .unwrap();
        let model = ModelId::new();
        let mut reference = ModelRef::new(model, RelativePath::new(file).unwrap(), hash);
        reference.transform = vec![TransformStep::Translate {
            x: SHIFT,
            y: 0.0,
            z: 0.0,
        }];
        let setup_id = SetupId::new();
        let mut setup = Setup::new(setup_id, "Setup 1");
        setup.part = Some(model);
        setup.stock = Some(Stock::Cylinder {
            diameter: Millimetres(2.0 * RADIUS + 1.0),
            length: Millimetres(LENGTH),
            x_start: Millimetres(SHIFT),
        });
        let tool = ToolId::new();
        let snapshot = ToolSnapshot {
            id: tool,
            order: OrderKey::default(),
            name: "4 mm drill".into(),
            tool: Tool {
                shape: ToolShape::Drill {
                    point_angle: Degrees(118.0),
                },
                diameter: Millimetres(DIAMETER),
                flute_length: Millimetres(30.0),
                overall_length: Millimetres(60.0),
                shank_diameter: Millimetres(DIAMETER),
                flute_count: 2,
                material: None,
                centre_cutting: Some(true),
            },
            controller: ControllerMapping {
                number: 1,
                length_offset: None,
                diameter_offset: None,
            },
            cutting_data: BTreeMap::new(),
        };
        let operation = OperationId::new();
        let mut values: BTreeMap<String, Value> = [
            (names::FEED, Value::Number(200.0)),
            (names::PLUNGE_FEED, Value::Number(100.0)),
            (names::STEPOVER, Value::Number(1.0)),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v))
        .collect();
        for (k, v) in parameters {
            values.insert((*k).to_owned(), v.clone());
        }
        for item in [
            Item::Model(reference),
            Item::Setup(setup),
            Item::Tool(snapshot),
            Item::Operation(Operation {
                enabled: true,
                id: operation,
                order: OrderKey::default(),
                name: "Op".into(),
                setup: setup_id,
                strategy: strategy.into(),
                tool: Some(tool),
                selections: Vec::new(),
                boundary: None,
                avoid: Vec::new(),
                check: Vec::new(),
                parameters: values,
            }),
        ] {
            project.insert(item, Placement::End).unwrap();
        }
        Fixture {
            project,
            operation,
            model,
            hash,
            brep,
            built,
        }
    }

    #[derive(Clone, Copy, Debug)]
    enum Role {
        Machine,
        Boundary,
        Avoid,
        Check,
    }

    impl Fixture {
        /// Adds a selection set on the part and puts it in the operation.
        fn select(&mut self, target: SelectionTarget, hash: ContentHash) {
            self.select_as(target, hash, self.model, Role::Machine);
        }

        /// Adds a selection set on `model` and puts it in `role` of the
        /// operation.
        fn select_as(
            &mut self,
            target: SelectionTarget,
            hash: ContentHash,
            model: ModelId,
            role: Role,
        ) {
            let id = SelectionSetId::new();
            self.project
                .insert(
                    Item::SelectionSet(SelectionSet {
                        id,
                        order: OrderKey::default(),
                        name: "chosen".into(),
                        model,
                        model_hash: hash,
                        target,
                        descriptors: Vec::new(),
                        stock_to_leave: None,
                        colour: None,
                    }),
                    Placement::End,
                )
                .unwrap();
            let mut operation = self.project.operation(self.operation).unwrap().clone();
            match role {
                Role::Machine => operation.selections.push(id),
                Role::Boundary => operation.boundary = Some(id),
                Role::Avoid => operation.avoid.push(id),
                Role::Check => operation.check.push(id),
            }
            self.project.replace(Item::Operation(operation)).unwrap();
        }

        fn run(&self) -> Result<OperationPath, AppError> {
            let placed = PlacedPart::of_brep(
                &self.brep,
                &self
                    .project
                    .models()
                    .iter()
                    .find(|m| m.id == self.model)
                    .unwrap()
                    .transform,
                self.hash,
            )
            .unwrap();
            let cancel = CancelToken::new();
            let context = JobContext::new(&cancel, &NoProgress);
            generate_with_part(
                Path::new("."),
                &self.project,
                self.operation,
                &Registry::builtin(),
                None,
                Some(placed),
                None,
                &context,
            )
        }
    }

    /// The height of the drill's 118 degree point.
    fn point_height() -> f64 {
        0.5 * DIAMETER / 59.0_f64.to_radians().tan()
    }

    /// How deep below the placed opening of `hole` the tool tip goes on the
    /// hole's own axis line, or `None` when no move reaches the line.
    fn depth_at(path: &OperationPath, hole: &Built) -> Option<f64> {
        let entry = Point3::new(hole.entry.x + SHIFT, hole.entry.y, hole.entry.z);
        path.group
            .segments
            .iter()
            .filter(|s| s.kind.is_fed())
            .filter_map(|s| {
                let off = s.end.tip - entry;
                let along = off.dot(hole.direction);
                let across = (off - hole.direction * along).length();
                (across < 1e-6).then_some(-along)
            })
            .reduce(f64::max)
    }

    #[test]
    fn req_op_10_a_step_part_drills_every_hole_through_the_pipeline() {
        let f = fixture(drill::NAME, &[]);
        let path = f.run().unwrap();
        for (i, hole) in f.built.iter().enumerate() {
            let through = hole.length > 2.0 * RADIUS - 1e-6;
            let want = hole.length + point_height() + if through { 0.5 } else { 0.0 };
            let got = depth_at(&path, hole).unwrap_or_else(|| panic!("hole {i} is not drilled"));
            assert!((got - want).abs() < 1e-6, "hole {i}: {got} against {want}");
        }
    }

    #[test]
    fn req_op_10_req_op_02_selected_faces_choose_the_holes_on_a_step_part() {
        let mut f = fixture(drill::NAME, &[]);
        let walls: BTreeSet<FaceId> = f.built[1].walls.iter().copied().collect();
        f.select(SelectionTarget::Faces(walls), f.hash);
        let path = f.run().unwrap();
        assert!(depth_at(&path, &f.built[1]).is_some());
        assert!(depth_at(&path, &f.built[0]).is_none());
        assert!(depth_at(&path, &f.built[2]).is_none());
    }

    #[test]
    fn req_op_08_a_step_part_follows_its_b_rep_edges_with_the_operations_values() {
        let engrave = [("depth", Value::Number(0.2))];
        let f = fixture(edge_following::NAME, &engrave);
        // The form shows the B-rep crease angle as the default.
        let form =
            crate::form::parameter_form(&f.project, &Registry::builtin(), f.operation).unwrap();
        let crease = form
            .fields
            .iter()
            .find(|x| x.name == edge_following::parameter::CREASE_ANGLE)
            .unwrap();
        assert_eq!(crease.value, Some(Value::Number(1.0)));
        assert_eq!(crease.source, crate::form::Source::Default);
        let path = f.run().unwrap();
        assert!(
            path.group
                .segments
                .iter()
                .any(|s| s.kind == openchuck_types::ir::MoveKind::Cutting)
        );
        // No warning that the crease angle and shortest chain are unused.
        assert!(
            !path
                .notes
                .iter()
                .any(|n| n.contains("not used") && n.contains("crease")),
            "{:?}",
            path.notes
        );
        // The shortest chain is the operation's: longer than every rim, so
        // nothing is left to follow.
        let mut long = engrave.to_vec();
        long.push(("min_length", Value::Number(1000.0)));
        let err = fixture(edge_following::NAME, &long).run().unwrap_err();
        assert!(err.to_string().contains("no edge to follow"), "{err}");
        // A chosen edge is the only one followed.
        let mut f = fixture(edge_following::NAME, &engrave);
        let edge = f.brep.edges.iter().find(|e| e.faces.len() == 2).unwrap().id;
        f.select(SelectionTarget::Edges([edge].into_iter().collect()), f.hash);
        let one = f.run().unwrap();
        assert!(one.group.segments.len() < path.group.segments.len());
    }

    #[test]
    fn req_op_02_req_nfr_07_selections_a_step_part_cannot_use_are_refused() {
        let faces = || SelectionTarget::Faces([FaceId(1)].into_iter().collect());
        let edges = || SelectionTarget::Edges([EdgeId(0)].into_iter().collect());
        let registry = Registry::builtin();
        let mut cases: Vec<(String, SelectionTarget, Role, bool, &str)> = Vec::new();
        for choice in registry.iter() {
            let strategy = choice.name().to_owned();
            if strategy != drill::NAME {
                cases.push((
                    strategy.clone(),
                    faces(),
                    Role::Machine,
                    true,
                    "limited to faces",
                ));
            }
            if strategy != edge_following::NAME {
                cases.push((
                    strategy.clone(),
                    edges(),
                    Role::Machine,
                    true,
                    "only edge following",
                ));
            }
            // Boundaries, avoid and check surfaces are not built for STEP parts
            // (OI-233), whatever the strategy.
            for role in [Role::Boundary, Role::Avoid, Role::Check] {
                cases.push((
                    strategy.clone(),
                    faces(),
                    role,
                    true,
                    "not built for STEP models",
                ));
            }
        }
        let drill = || drill::NAME.to_owned();
        let edge = || edge_following::NAME.to_owned();
        cases.extend([
            (
                drill(),
                SelectionTarget::Faces([FaceId(9999)].into_iter().collect()),
                Role::Machine,
                true,
                "face 9999 is not in the model",
            ),
            (
                edge(),
                SelectionTarget::Edges([EdgeId(9999)].into_iter().collect()),
                Role::Machine,
                true,
                "edge 9999 is not in the model",
            ),
            (drill(), faces(), Role::Machine, false, "different version"),
            (
                drill(),
                SelectionTarget::Faces(BTreeSet::new()),
                Role::Machine,
                true,
                "selects no faces",
            ),
            (
                edge(),
                SelectionTarget::Edges(BTreeSet::new()),
                Role::Machine,
                true,
                "selects no edges",
            ),
        ]);
        for (strategy, target, role, same_hash, message) in cases {
            let mut f = fixture(&strategy, &[("depth", Value::Number(0.2))]);
            let hash = if same_hash {
                f.hash
            } else {
                ContentHash::of_bytes(b"other")
            };
            f.select_as(target, hash, f.model, role);
            let err = f.run().unwrap_err().to_string();
            assert!(
                err.contains(message) && err.contains("'Op'"),
                "{strategy} {role:?}: {err}"
            );
        }
    }

    #[test]
    fn req_op_02_req_nfr_07_a_set_made_on_another_model_is_refused() {
        let mut f = fixture(drill::NAME, &[]);
        let other = ModelId::new();
        f.project
            .insert(
                Item::Model(ModelRef::new(
                    other,
                    RelativePath::new("other.step").unwrap(),
                    f.hash,
                )),
                Placement::End,
            )
            .unwrap();
        f.select_as(
            SelectionTarget::Faces([FaceId(1)].into_iter().collect()),
            f.hash,
            other,
            Role::Machine,
        );
        let err = f.run().unwrap_err().to_string();
        assert!(err.contains("different model"), "{err}");
    }

    #[test]
    fn req_op_10_req_nfr_07_faces_with_no_hole_and_a_part_with_no_holes_are_clear_errors() {
        // The blank's side holds no hole wall: nothing to drill.
        let mut f = fixture(drill::NAME, &[]);
        f.select(
            SelectionTarget::Faces([FaceId(0)].into_iter().collect()),
            f.hash,
        );
        let err = f.run().unwrap_err().to_string();
        assert!(err.contains("no hole to drill"), "{err}");
        // A part with no holes at all.
        let f = fixture_with(drill::NAME, &[], &[], "blank.step");
        let err = f.run().unwrap_err().to_string();
        assert!(err.contains("no hole to drill"), "{err}");
    }

    #[test]
    fn req_op_08_the_crease_angle_default_is_the_b_reps_on_a_step_part_and_the_meshs_on_a_mesh() {
        let crease = |f: &Fixture| {
            crate::form::parameter_form(&f.project, &Registry::builtin(), f.operation)
                .unwrap()
                .fields
                .into_iter()
                .find(|x| x.name == edge_following::parameter::CREASE_ANGLE)
                .unwrap()
        };
        let engrave = [("depth", Value::Number(0.2))];
        // A STEP part: 1 degree, shown as the default.
        let step = fixture(edge_following::NAME, &engrave);
        let field = crease(&step);
        assert_eq!(field.value, Some(Value::Number(1.0)));
        assert_eq!(field.source, crate::form::Source::Default);
        // A mesh part keeps the schema's 30 degrees.
        let mesh = fixture_with(edge_following::NAME, &engrave, &[], "blank.stl");
        let field = crease(&mesh);
        assert_eq!(field.value, Some(Value::Number(30.0)));
        // Other strategies and parameters are not touched.
        let op = mesh.project.operation(mesh.operation).unwrap();
        assert!(default_override(&step.project, op, names::FEED).is_none());
        // The operation's value wins: every edge of the blank turns 90
        // degrees, so 120 leaves none to follow, where the default follows
        // them all.
        let mut over = engrave.to_vec();
        over.push((
            edge_following::parameter::CREASE_ANGLE,
            Value::Number(120.0),
        ));
        let set = fixture(edge_following::NAME, &over);
        assert_eq!(crease(&set).source, crate::form::Source::Operation);
        let err = set.run().unwrap_err().to_string();
        assert!(err.contains("no edge to follow"), "{err}");
        assert!(step.run().is_ok());
    }

    #[test]
    fn req_nfr_07_a_step_part_that_cannot_be_placed_or_evaluated_is_an_error() {
        let (brep, _) = drilled_blank(RADIUS, LENGTH, &[]);
        let hash = ContentHash::of_bytes(b"blank.step");
        // An invalid placement step.
        let err = PlacedPart::of_brep(&brep, &[TransformStep::Scale { factor: 0.0 }], hash)
            .unwrap_err()
            .to_string();
        assert!(err.contains("factor"), "{err}");
        // A face whose frame has no axes cannot be evaluated: the operation
        // fails with a message, not a panic.
        let mut f = fixture(drill::NAME, &[]);
        let face = &mut f.brep.faces[0];
        let zero = openchuck_types::geometry::Vec3::new(0.0, 0.0, 0.0);
        match &mut face.geometry {
            openchuck_types::brep::Surface::Cylinder { frame, .. }
            | openchuck_types::brep::Surface::Plane { frame } => {
                frame.x = zero;
                frame.y = zero;
                frame.z = zero;
            }
            other => panic!("unexpected side face {other:?}"),
        }
        assert!(f.run().is_err());
    }

    #[test]
    fn req_op_08_req_nfr_07_keeping_tangent_edges_is_refused_on_a_mesh_part() {
        let f = fixture_with(
            edge_following::NAME,
            &[("depth", Value::Number(0.2))],
            &[HoleSpec::through(30.0, polar(0.0), DIAMETER)],
            "blank.stl",
        );
        let run = |keep_tangent: bool| {
            // The blank's triangles as a mesh part: no B-rep.
            let mut placed = PlacedPart::of_brep(&f.brep, &[], f.hash).unwrap();
            placed.brep = None;
            let cancel = CancelToken::new();
            let context = JobContext::new(&cancel, &NoProgress);
            generate_with_part(
                Path::new("."),
                &f.project,
                f.operation,
                &Registry::builtin(),
                None,
                Some(placed),
                Some(EdgeOptions { keep_tangent }),
                &context,
            )
        };
        let err = run(true).unwrap_err().to_string();
        assert!(err.contains("'Op'") && err.contains("mesh"), "{err}");
        // Off, the mesh's own feature edges are followed as before.
        assert!(run(false).is_ok());
        // The project answers off for every operation (lane O stores the
        // setting, OI-232).
        let op = f.project.operation(f.operation).unwrap();
        assert_eq!(edge_options(&f.project, op), EdgeOptions::default());
    }
}
