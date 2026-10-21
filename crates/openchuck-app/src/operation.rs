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
use openchuck_path::roughing::{self, parameter::STEPDOWN};
use openchuck_path::{
    AcceptAll, AnalyticStock, BoundaryLimits, BrepEdges, BrepSurface, ChosenSegments,
    CompositeSurface, CurveSource, EdgeFollowing, MeshSurface, ParameterValue, PathWarning,
    Protection, RegionConstraint, Registry, RotaryMapping, StockModel, Strategy, StrategyInput,
    SurfaceSource, WatchedSurface, Window, drill, edge_following, generate, generate_in_region,
    generate_in_zones, swarf,
};
use openchuck_project::id::{OperationId, SelectionSetId};
use openchuck_project::{
    BoundaryOptions, ContentHash, ModelRef, OnBreak, Operation, Project, SelectionTarget,
};
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
    // A required value that neither the operation nor the cutting data gives:
    // the same list the interface marks (`requirements`), narrowed to the
    // values whose cause is the cutting data.
    let missing: Vec<String> = crate::requirements::missing_parameters(project, operation, &schema)
        .into_iter()
        .map(|(name, _)| name)
        .filter(|name| crate::requirements::FROM_CUTTING_DATA.contains(&name.as_str()))
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
    /// What the user must see before using the path: it was made, but does
    /// not meet a limit the operation sets, such as swarf cutting into the
    /// finished surface deeper than the tolerance (NFR-12; decision record
    /// 0175). Each is in `notes` too.
    pub warnings: Vec<openchuck_path::PathWarning>,
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

/// The edge options of `operation` as the project holds them (decision 0057,
/// decision record 0066 item 9).
pub fn edge_options(project: &Project, operation: &Operation) -> EdgeOptions {
    EdgeOptions {
        keep_tangent: keep_tangent_edges(project, operation),
    }
}

/// Whether edge following on `operation` keeps tangent edges: on a STEP part,
/// when one of the `Edges` sets it follows keeps them, by its own choice or
/// else the project's default (`Project::keeps_tangent_edges`). An operation
/// that follows every edge has no set to say, so it keeps none, and its cache
/// key, which holds each edge set's resolved choice, stays whole. Always off
/// on a mesh part, whose edges are chosen by the crease angle.
fn keep_tangent_edges(project: &Project, operation: &Operation) -> bool {
    on_step_part(project, operation)
        && operation
            .selections
            .iter()
            .filter_map(|id| project.selection_sets().iter().find(|s| s.id == *id))
            .filter(|s| matches!(s.target, SelectionTarget::Edges(_)))
            .any(|s| project.keeps_tangent_edges(s))
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
    // Swarf reads rulings from exact faces and drilling finds holes in them
    // (decision 0021, OI-96; decision 0174): said before any selection on a
    // mesh is judged.
    if let Some(reason) = brep_only(project, registry, operation) {
        return Err(missing(operation, &reason));
    }
    let limited = operation.selection_refs().next().is_some();
    // A STEP part's selections are faces and edges, which choose the holes
    // and the edges below (decision 0056); a mesh part's are triangles, or
    // its chosen edges, which edge following follows rather than honouring a
    // region (OI-204; decision record 0066).
    if limited
        && !on_step_part(project, operation)
        && !strategy.honours_region()
        && !crate::selection::selects_mesh_edges(project, operation)
    {
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

    // The part as this setup places it: the offset moves it against the
    // stock, workholding and work zero (SET-05; decision record 0201).
    let placed = match placed {
        Some(placed) => placed,
        None => crate::model::placed_part(root, model, context)?,
    }
    .in_setup(&setup.frame)?;
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
    // The whole placed part, not the selected faces: bounding-box stock and
    // playback need the part's extent.
    let part_bounds = openchuck_geom::summary(mesh)
        .bounds
        .ok_or_else(|| missing(operation, "its setup's part has no extent"))?;
    let profile = snapshot.tool.profile()?;
    // The held regions and tabs join the model's surface as the target the
    // strategies see (SET-09, OP-21; decision 0035 item 1). A setup without
    // them uses the model's own surface, so its G-code is unchanged.
    let mut reach_notes = Vec::new();
    // No part of the tool reaches over a gripped length while it cuts
    // (SET-09; decision 0161): each held region widened by the tool's largest
    // radius, its shank included.
    let keep_out = keep_out_of(setup, &part_bounds, &profile, &snapshot.tool);
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
    // A mesh part's chosen edges (OI-204) are followed, not a region.
    let mesh_edges = match placed.brep {
        Some(_) => None,
        None => crate::selection::mesh_edge_segments(project, operation, model, file_hash, mesh)?,
    };
    let resolved_region = match (&placed.brep, &mesh_edges) {
        (None, None) => crate::selection::resolve(project, operation, model, file_hash, mesh)?,
        _ => None,
    };
    // Swarf on a STEP part is limited by face sets (decision 0176).
    let brep_limits = match (&placed.brep, &brep_choice) {
        (Some(brep), Some(choice)) => BrepLimits::of(brep, choice)?,
        _ => None,
    };
    let watch = brep_limits
        .as_ref()
        .map(BrepLimits::watch)
        .unwrap_or_default();
    let constraint = match (&resolved_region, &brep_limits) {
        (Some(r), _) => Some(r.constraint()),
        (None, Some(l)) => Some(l.constraint(&watch)),
        (None, None) => None,
    };
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
        keep_out: &keep_out,
    };
    let resolved = parameter_values(project, operation, registry)?;
    // The values as the strategy's schema reads them (a whole number given for
    // a number is a number), resolved once for everything below.
    let parameters = strategy.schema().resolve(&resolved.values)?;
    if operation.strategy == swarf::NAME && placed.brep.is_some() {
        swarf_compatibility(
            operation,
            model_surface.as_ref(),
            parameters.number(names::TOLERANCE)?,
        )?;
    }
    // A straight plunge needs a tool that can cut straight down (OI-117).
    EntryParameters::read(&parameters)?.check_tool(snapshot.tool.can_plunge())?;
    let operation_leave = parameters.number(names::STOCK_TO_LEAVE)?;
    let mut zones = resolved_region
        .as_ref()
        .map_or_else(Vec::new, |r| r.zones(operation_leave));
    let mut values = resolved.values;
    let mut leave_notes = Vec::new();
    if !zones.is_empty() && !strategy.honours_leave_zones() {
        if operation.strategy != roughing::NAME {
            return Err(missing(
                operation,
                &format!(
                    "the strategy '{}' cannot leave different stock on different selections; \
                     clear the stock to leave of the selection sets",
                    strategy.label()
                ),
            ));
        }
        // Rotary roughing leaves the largest amount wherever it cuts, so it
        // never cuts below what any selection asks for (decision 0058).
        zones.clear();
        if let Some((leave, set)) = largest_selection_leave(project, operation)
            && leave > operation_leave
        {
            values.insert(
                names::STOCK_TO_LEAVE.to_owned(),
                ParameterValue::Number(leave),
            );
            leave_notes.push(format!(
                "stock to leave: roughing leaves {leave} mm wherever it cuts, the amount of \
                 the selection set '{set}' (the largest of its selections), in place of the \
                 operation's {operation_leave} mm; roughing leaves one amount everywhere it \
                 cuts, and a finishing operation cuts each selection to its own"
            ));
        }
    }
    // Edge following on a STEP part follows the B-rep's own edges, with the
    // operation's crease angle and shortest chain (OP-08; decision 0053 item 7,
    // decision 0056); on a mesh part's chosen edges it follows only them (OP-08,
    // OI-204; decision record 0066).
    let brep_edges = match (&placed.brep, &brep_choice) {
        (Some(brep), Some(choice)) if operation.strategy == edge_following::NAME => Some(
            brep_curves(operation, brep, &choice.edges, &parameters, edges)?,
        ),
        _ => None,
    };
    let registry_strategy = strategy;
    let with_edges;
    let strategy: &dyn Strategy = match (brep_edges, mesh_edges) {
        (Some(source), _) => {
            with_edges = EdgeFollowing::with_source(source);
            &with_edges
        }
        (None, Some(segments)) => {
            with_edges = EdgeFollowing::with_source(
                ChosenSegments::new(segments)?
                    .with_min_length(parameters.number(edge_following::parameter::MIN_LENGTH)?)
                    .shared(),
            );
            &with_edges
        }
        (None, None) => strategy,
    };
    let path = if zones.is_empty() {
        match &constraint {
            Some(region) => generate_in_region(strategy, &input, region, &values, context)?,
            None => generate(strategy, &input, &values, context)?,
        }
    } else {
        generate_in_zones(
            strategy,
            &input,
            constraint.as_ref(),
            &zones,
            &values,
            context,
        )?
    };
    // A set the operation's options leave limiting nothing is a warning,
    // said first in the notes as well (decision 0176), as the library does
    // for its own warnings.
    let mut warnings = brep_limits
        .as_ref()
        .map(|l| l.warnings.clone())
        .unwrap_or_default();
    // Notes about the values come first, then the notes about moves.
    let mut notes: Vec<String> = warnings.iter().map(|w| w.message.clone()).collect();
    notes.extend(resolved.notes);
    notes.extend(leave_notes);
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
        warnings: {
            warnings.extend(path.warnings);
            warnings
        },
    })
}

/// The largest stock to leave of the selection sets `operation` selects, and
/// the name of the first set that holds it. `None` when no set has one.
fn largest_selection_leave<'a>(
    project: &'a Project,
    operation: &Operation,
) -> Option<(f64, &'a str)> {
    operation
        .selections
        .iter()
        .filter_map(|id| project.selection_sets().iter().find(|s| s.id == *id))
        .filter_map(|s| s.stock_to_leave.map(|leave| (leave, s.name.as_str())))
        .fold(None, |best, (leave, name)| match best {
            Some((top, _)) if top >= leave => best,
            _ => Some((leave, name)),
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

/// The name of the part of `operation`'s setup when it is a mesh (an STL),
/// read from its file name; `None` for a STEP part or a setup with no part.
fn mesh_part_of_setup(project: &Project, setup: openchuck_project::id::SetupId) -> Option<&str> {
    let part = project
        .setup(setup)
        .and_then(|s| s.part)
        .and_then(|id| project.models().iter().find(|m| m.id == id))?;
    let brep = openchuck_types::model::ModelFormat::from_file_name(part.path.as_str())
        .is_some_and(|f| f.kind() == openchuck_types::model::ModelKind::Brep);
    (!brep).then_some(part.path.as_str())
}

/// Why `strategy` cannot be used in `setup`: the strategy needs a B-rep
/// (STEP) part by its registration ([`Strategy::needs_brep_part`]) and the
/// setup's part is a mesh (decision 0174, amending decision 0111 item 3;
/// decision 0021, OI-96). In plain words for the strategy list. `None` when
/// it can be used, or the setup has no part yet.
///
/// [`Strategy::needs_brep_part`]: openchuck_path::Strategy::needs_brep_part
pub(crate) fn unavailable_on_setup(
    project: &Project,
    strategy: &dyn openchuck_path::Strategy,
    setup: openchuck_project::id::SetupId,
) -> Option<String> {
    let why = strategy.needs_brep_part()?;
    let part = mesh_part_of_setup(project, setup)?;
    Some(format!(
        "Needs a STEP model: {why}. This setup's part '{part}' is a mesh (STL); add the part as \
         a STEP file to use it"
    ))
}

/// Why `operation` cannot run on its setup's part: its strategy needs a
/// B-rep (STEP) part by its registration and the part is a mesh (decision
/// 0021, OI-96; decision 0174). The operation could be added before the
/// rule, or its part replaced since, so the form and generation still say
/// so (NFR-07). `None` for a strategy that works on a mesh, a STEP part, a
/// setup with no part (reported as missing where the part is read), or a
/// strategy that is not registered (reported where it is looked up).
pub(crate) fn brep_only(
    project: &Project,
    registry: &Registry,
    operation: &Operation,
) -> Option<String> {
    let strategy = registry.get(&operation.strategy).ok()?;
    let why = strategy.needs_brep_part()?;
    let part = mesh_part_of_setup(project, operation.setup)?;
    Some(format!(
        "{} is B-rep only ({why}): the setup's part '{part}' is a mesh; add the part as a STEP \
         file",
        strategy.label()
    ))
}

/// Why swarf on a STEP part with no face set is refused (decision 0111
/// item 1).
const SWARF_NEEDS_FACES: &str = "swarf needs a face set: select the ruled walls to cut with the \
     side of the tool, save them as a set and put it in the operation";

/// Why `operation` cannot run yet when its strategy is swarf on a STEP part
/// and none of its selection sets is a face set (decision 0111 item 1), for
/// the parameter form. Whether the faces suit swarf is judged when it is
/// generated ([`swarf_compatibility`]).
pub(crate) fn swarf_needs_faces(project: &Project, operation: &Operation) -> Option<String> {
    if operation.strategy != swarf::NAME || !on_step_part(project, operation) {
        return None;
    }
    let has_faces = operation.selections.iter().any(|id| {
        project
            .selection_sets()
            .iter()
            .any(|s| s.id == *id && matches!(&s.target, SelectionTarget::Faces(f) if !f.is_empty()))
    });
    (!has_faces).then(|| SWARF_NEEDS_FACES.to_owned())
}

/// Checks, before swarf runs, that every face of `surface` (the operation's
/// face set) is one swarf can cut: ruled within `tolerance` in a family whose
/// rulings do not run along the rotary axis (decision 0061 items 2 and 4,
/// decision 0111 item 2). Fails naming every face that is not, and why.
fn swarf_compatibility(
    operation: &Operation,
    surface: &dyn SurfaceSource,
    tolerance: f64,
) -> Result<(), AppError> {
    let problems: Vec<String> = surface
        .exact_faces()
        .into_iter()
        .filter_map(|face| {
            let detection = swarf::detect_rulings(face.face, tolerance);
            let why = match detection.chosen {
                None => detection.refusal(),
                // Within a hundredth of a degree of the axis: no rotation
                // of the part brings the flank to such a ruling (OI-94).
                Some(family) if detection.fit(family).along_axis > AXIAL_RULING => Some(
                    "its rulings run along the rotary axis, so no rotation of the part brings \
                     the side of the tool to them"
                        .to_owned(),
                ),
                Some(_) => None,
            };
            why.map(|why| format!("face {}: {why}", face.number))
        })
        .collect();
    if problems.is_empty() {
        return Ok(());
    }
    Err(missing(
        operation,
        &format!(
            "its face set has faces swarf cannot cut; take them out of the set ({})",
            problems.join("; ")
        ),
    ))
}

/// The component along the rotary axis above which a unit ruling is taken
/// to run along it: the cosine of 0.01 degrees.
const AXIAL_RULING: f64 = 0.999_999_984_769_129;

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

/// The boundary, avoid and check faces of swarf on a STEP part as the
/// pathing library takes them (OP-02; decision 0176): the boundary as the
/// window of its faces, the avoid and check sets that cut the pass short as
/// their exact faces, and those that only warn as watched surfaces, each
/// named. A set that limits nothing is left out, with a warning naming it.
struct BrepLimits {
    boundary: Option<Window>,
    boundary_limits: BoundaryLimits,
    avoid: Option<BrepSurface>,
    check: Option<BrepSurface>,
    watched: Vec<(String, Protection, BrepSurface)>,
    warnings: Vec<PathWarning>,
}

impl BrepLimits {
    /// `None` when `choice` has no boundary, avoid or check sets.
    fn of(brep: &BrepModel, choice: &BrepChoice) -> Result<Option<Self>, AppError> {
        if choice.boundary.is_none() && choice.avoid.is_empty() && choice.check.is_empty() {
            return Ok(None);
        }
        let surface = |faces: &[FaceId]| {
            (!faces.is_empty())
                .then(|| BrepSurface::with_faces(brep, faces))
                .transpose()
        };
        let mut warnings = Vec::new();
        let mut boundary = None;
        let mut boundary_limits = BoundaryLimits::default();
        if let Some(set) = &choice.boundary {
            if set.boundary.limits_nothing() {
                warnings.push(limits_nothing(
                    &format!("boundary set '{}'", set.name),
                    "both Limits the pass and Limits the tip are off: the faces are cut as if \
                     there were no boundary",
                    "Turn one of them on",
                ));
            } else {
                boundary = surface(&set.faces)?
                    .map(|b| Window::of_mesh(b.tessellation()))
                    .transpose()?;
                boundary_limits = BoundaryLimits {
                    pass: set.boundary.limits_pass,
                    tip: set.boundary.limits_tip,
                };
            }
        }
        let mut cut_short = [Vec::new(), Vec::new()];
        let mut watched = Vec::new();
        for (sets, protection, role, short) in [
            (&choice.avoid, Protection::Avoid, "avoid", 0),
            (&choice.check, Protection::Check, "check", 1),
        ] {
            for set in sets {
                match set.on_break {
                    OnBreak::CutShort => {
                        if let Some(faces) = cut_short.get_mut(short) {
                            faces.extend_from_slice(&set.faces);
                        }
                    }
                    OnBreak::Warn => {
                        if let Some(s) = surface(&set.faces)? {
                            watched.push((set.name.clone(), protection, s));
                        }
                    }
                    OnBreak::Ignore => warnings.push(limits_nothing(
                        &format!("{role} set '{}'", set.name),
                        "it is set to Ignore: the tool is not kept clear of it",
                        "Set it to Cut short or Warn only",
                    )),
                }
            }
        }
        for faces in &mut cut_short {
            faces.sort_unstable();
            faces.dedup();
        }
        let [avoid, check] = cut_short;
        Ok(Some(Self {
            boundary,
            boundary_limits,
            avoid: surface(&avoid)?,
            check: surface(&check)?,
            watched,
            warnings,
        }))
    }

    /// The watched surfaces, for [`constraint`](Self::constraint).
    fn watch(&self) -> Vec<WatchedSurface<'_>> {
        self.watched
            .iter()
            .map(|(name, protection, surface)| WatchedSurface {
                name,
                protection: *protection,
                surface,
            })
            .collect()
    }

    fn constraint<'a>(&'a self, watch: &'a [WatchedSurface<'a>]) -> RegionConstraint<'a> {
        RegionConstraint {
            region: None,
            boundary: self.boundary,
            avoid: self.avoid.as_ref().map(|s| s as _),
            check: self.check.as_ref().map(|s| s as _),
            boundary_limits: self.boundary_limits,
            watch,
        }
    }
}

/// The warning for a set whose options leave it limiting nothing (decision
/// 0176): the path is made as if it were not there.
fn limits_nothing(set: &str, why: &str, fix: &str) -> PathWarning {
    PathWarning {
        message: format!(
            "The {set} limits nothing, because {why}. {fix} beside the set in the operation \
             inspector's Geometry section, or take the set out of the operation."
        ),
        parameter: None,
    }
}

/// A boundary, avoid or check set of swarf on a STEP part, with its
/// options (decision 0176).
struct LimitSet {
    name: String,
    faces: Vec<FaceId>,
    boundary: BoundaryOptions,
    on_break: OnBreak,
}

/// What an operation on a STEP part selects: the faces whose holes radial
/// drilling drills or whose walls swarf cuts, or the edges edge following
/// follows (OP-02, OP-07, OP-08, OP-10). Empty lists mean the whole part.
struct BrepChoice {
    faces: Vec<FaceId>,
    edges: Vec<EdgeId>,
    /// The boundary, avoid and check sets, for swarf, each with its faces
    /// and options (decision 0176).
    boundary: Option<LimitSet>,
    avoid: Vec<LimitSet>,
    check: Vec<LimitSet>,
}

/// Reads the selections of `operation` on its STEP part `part` (decision
/// 0056). Only face sets for radial drilling and swarf (decision 0061 item 1)
/// and edge sets for edge following are honoured so far; anything else is an
/// error that says so, never a silent drop (OP-02).
fn brep_selection(
    project: &Project,
    operation: &Operation,
    part: &ModelRef,
    hash: ContentHash,
    brep: &BrepModel,
) -> Result<BrepChoice, AppError> {
    let fail = |reason: String| missing(operation, &reason);
    let limited =
        operation.boundary.is_some() || !operation.avoid.is_empty() || !operation.check.is_empty();
    if limited && operation.strategy != swarf::NAME {
        return Err(fail(
            "boundaries, avoid and check surfaces on a STEP model are built for swarf only so \
             far; remove them from the operation"
                .to_owned(),
        ));
    }
    let options = &operation.restriction_options;
    let limit = |id: SelectionSetId, on_break: OnBreak| -> Result<LimitSet, AppError> {
        Ok(LimitSet {
            name: project
                .selection_sets()
                .iter()
                .find(|s| s.id == id)
                .map(|s| s.name.clone())
                .unwrap_or_default(),
            faces: limit_faces(
                project,
                operation,
                part,
                hash,
                brep,
                &mut std::iter::once(id),
            )?,
            boundary: options.boundary,
            on_break,
        })
    };
    let mut choice = BrepChoice {
        faces: Vec::new(),
        edges: Vec::new(),
        boundary: operation
            .boundary
            .map(|id| limit(id, OnBreak::default()))
            .transpose()?,
        avoid: operation
            .avoid
            .iter()
            .map(|id| limit(*id, options.avoid(*id).on_break))
            .collect::<Result<_, _>>()?,
        check: operation
            .check
            .iter()
            .map(|id| limit(*id, options.check(*id).on_break))
            .collect::<Result<_, _>>()?,
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
        if set.stock_to_leave.is_some() {
            return Err(bad(
                "a stock to leave per selection set is not built for STEP parts yet; \
                 clear it on the set and set the operation's stock to leave"
                    .to_owned(),
            ));
        }
        match &set.target {
            SelectionTarget::Faces(faces) if faces.is_empty() => {
                return Err(bad(if operation.strategy == swarf::NAME {
                    "it selects no faces; select the ruled walls to cut with the side of the \
                     tool"
                        .to_owned()
                } else {
                    "it selects no faces; select the hole walls to drill, or take the set out \
                     of the operation to drill every hole"
                        .to_owned()
                }));
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
            // The project refuses triangle and mesh edge sets on a STEP model.
            SelectionTarget::Triangles(_) => {
                return Err(bad(
                    "it selects triangles, and a STEP model is selected by face and edge"
                        .to_owned(),
                ));
            }
            SelectionTarget::MeshEdges(_) => {
                return Err(bad(
                    "it selects edges of a mesh's triangles, and a STEP model is selected by its \
                     own faces and edges"
                        .to_owned(),
                ));
            }
        }
    }
    choice.faces.sort_unstable();
    choice.faces.dedup();
    choice.edges.sort_unstable();
    choice.edges.dedup();
    if !choice.faces.is_empty()
        && operation.strategy != drill::NAME
        && operation.strategy != swarf::NAME
    {
        return Err(fail(format!(
            "the strategy '{}' cannot yet be limited to faces of a STEP model (OP-02); only \
             radial drilling, to choose its holes, and swarf, to choose the walls it cuts, use \
             selected faces",
            operation.strategy
        )));
    }
    if !choice.edges.is_empty() && operation.strategy != edge_following::NAME {
        return Err(fail(format!(
            "the strategy '{}' does not use selected edges; only edge following does",
            operation.strategy
        )));
    }
    // Swarf cuts only the walls the user chose, never every face of the part
    // (the owner's change to decision 0061 item 1, decision 0111 item 1).
    if operation.strategy == swarf::NAME && choice.faces.is_empty() {
        return Err(fail(SWARF_NEEDS_FACES.to_owned()));
    }
    // A wall that is cut cannot also be kept clear of (decision 0176).
    for (ids, role) in [(&operation.avoid, "avoid"), (&operation.check, "check")] {
        for id in ids {
            let Some(set) = project.selection_sets().iter().find(|s| s.id == *id) else {
                continue;
            };
            if let SelectionTarget::Faces(faces) = &set.target
                && let Some(face) = faces.iter().find(|f| choice.faces.contains(f))
            {
                return Err(fail(format!(
                    "face {} is in the face set it cuts and in the {role} set '{}'; take it out \
                     of one of them",
                    face.0, set.name
                )));
            }
        }
    }
    Ok(choice)
}

/// The faces of the boundary, avoid or check sets `ids` of `operation` on
/// its STEP part (decision 0176): each must be a face set made on the part
/// as it is now. Empty when there are none.
fn limit_faces(
    project: &Project,
    operation: &Operation,
    part: &ModelRef,
    hash: ContentHash,
    brep: &BrepModel,
    ids: &mut dyn Iterator<Item = SelectionSetId>,
) -> Result<Vec<FaceId>, AppError> {
    let mut all = Vec::new();
    for id in ids {
        let set = project
            .selection_sets()
            .iter()
            .find(|s| s.id == id)
            .ok_or_else(|| AppError::UnknownSelection(id.to_string()))?;
        let bad = |reason: String| {
            missing(
                operation,
                &format!("selection set '{}' cannot be used: {reason}", set.name),
            )
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
        let SelectionTarget::Faces(faces) = &set.target else {
            return Err(bad(format!(
                "it selects {}, and a boundary, avoid or check set of a STEP model selects faces",
                set.target.kind_name()
            )));
        };
        if faces.is_empty() {
            return Err(bad("it selects no faces".to_owned()));
        }
        if let Some(face) = faces
            .iter()
            .find(|f| !brep.faces.iter().any(|g| g.id == **f))
        {
            return Err(bad(format!("face {} is not in the model", face.0)));
        }
        all.extend(faces.iter().copied());
    }
    all.sort_unstable();
    all.dedup();
    Ok(all)
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

/// The model X intervals the tool's tip keeps out of while it cuts: each held
/// region of `setup` on its stock, widened on both sides by the largest radius
/// of `tool` (its cutting profile or its shank), as the tool is always across
/// the rotary axis, and by [`HELD_CLEARANCE`] (SET-09; decision 0161). Empty with no held region, or
/// when the stock's extent is not known (the held regions are then refused
/// when their solids are built).
fn keep_out_of(
    setup: &openchuck_project::Setup,
    part_bounds: &openchuck_types::geometry::Aabb,
    profile: &openchuck_types::tool::profile::RevolvedProfile,
    tool: &openchuck_types::tool::Tool,
) -> Vec<(f64, f64)> {
    let part = openchuck_types::setup::PartExtent {
        bounds: *part_bounds,
        radius: openchuck_types::units::Millimetres(0.0),
    };
    let Some(bounds) = setup.stock.as_ref().and_then(|s| s.bounds(Some(&part))) else {
        return Vec::new();
    };
    let reach = profile
        .pieces()
        .iter()
        .map(|p| p.end().radius.0)
        .fold(profile.cutting_radius(), f64::max)
        .max(tool.shank_diameter.0 / 2.0);
    let (start, end) = setup
        .workholding
        .held_intervals((bounds.min.x, bounds.max.x));
    [start, end]
        .into_iter()
        .flatten()
        .map(|(lo, hi)| (lo - reach - HELD_CLEARANCE, hi + reach + HELD_CLEARANCE))
        .collect()
}

/// How far, beyond its largest radius, the tool keeps from a held region
/// along the axis, in millimetres (decision 0161): the jaw face is not a
/// surface to cut against, and the simulation's cells would read a tool
/// flush with it as touching the held stock.
pub const HELD_CLEARANCE: f64 = 0.5;

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
        Item, OrderKey, ParameterValue as Value, Placement, RelativePath, RestrictionOptions,
        SelectionSet, SetOptions, Setup, ToolSnapshot,
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
        let drill = Tool {
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
        };
        fixture_on(strategy, parameters, brep, built, file, drill, RADIUS)
    }

    /// A project on the STEP file `file` whose placed part is `brep`, in a
    /// cylinder of stock `radius` + 0.5 mm, cut by `tool`.
    fn fixture_on(
        strategy: &str,
        parameters: &[(&str, Value)],
        brep: BrepModel,
        built: Vec<Built>,
        file: &str,
        tool: Tool,
        radius: f64,
    ) -> Fixture {
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
            diameter: Millimetres(2.0 * radius + 1.0),
            length: Millimetres(LENGTH),
            x_start: Millimetres(SHIFT),
        });
        let snapshot = ToolSnapshot {
            id: ToolId::new(),
            order: OrderKey::default(),
            name: "the tool".into(),
            tool,
            controller: ControllerMapping {
                number: 1,
                length_offset: None,
                diameter_offset: None,
            },
            cutting_data: BTreeMap::new(),
        };
        let tool_id = snapshot.id;
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
                tool: Some(tool_id),
                selections: Vec::new(),
                boundary: None,
                avoid: Vec::new(),
                check: Vec::new(),
                restriction_options: Default::default(),
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

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
        /// operation. Returns its ID.
        fn select_as(
            &mut self,
            target: SelectionTarget,
            hash: ContentHash,
            model: ModelId,
            role: Role,
        ) -> SelectionSetId {
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
                        keep_tangent_edges: None,
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
            id
        }

        /// Changes the options of the operation's boundary, avoid and check
        /// sets (decision 0176).
        fn set_options(&mut self, change: impl FnOnce(&mut RestrictionOptions)) {
            let mut operation = self.project.operation(self.operation).unwrap().clone();
            change(&mut operation.restriction_options);
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
            if strategy != drill::NAME && strategy != swarf::NAME {
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
            // Boundaries, avoid and check surfaces on a STEP part are built
            // for swarf only (decision 0176; OI-233 for the others).
            for role in [Role::Boundary, Role::Avoid, Role::Check] {
                if strategy == swarf::NAME {
                    cases.extend([
                        (strategy.clone(), edges(), role, true, "selects faces"),
                        (
                            strategy.clone(),
                            SelectionTarget::Faces([FaceId(9999)].into_iter().collect()),
                            role,
                            true,
                            "face 9999 is not in the model",
                        ),
                        (
                            strategy.clone(),
                            SelectionTarget::Faces(BTreeSet::new()),
                            role,
                            true,
                            "selects no faces",
                        ),
                        (strategy.clone(), faces(), role, false, "different version"),
                    ]);
                } else {
                    cases.push((
                        strategy.clone(),
                        faces(),
                        role,
                        true,
                        "built for swarf only",
                    ));
                }
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
    fn req_op_08_an_edge_sets_tangent_choice_or_the_projects_default_reaches_edge_following() {
        let mut f = fixture(edge_following::NAME, &[("depth", Value::Number(0.2))]);
        let edges: BTreeSet<EdgeId> = f.brep.edges.iter().take(2).map(|e| e.id).collect();
        f.select(SelectionTarget::Edges(edges), f.hash);
        let keep = |project: &Project| {
            let op = project.operation(f.operation).unwrap();
            edge_options(project, op).keep_tangent
        };
        let set_choice = |project: &mut Project, choice: Option<bool>| {
            let mut set = project.selection_sets()[0].clone();
            set.keep_tangent_edges = choice;
            project.replace(Item::SelectionSet(set)).unwrap();
        };
        // The set does not say and the project's default is off.
        assert!(!keep(&f.project));
        // The project's default applies to a set that does not say.
        f.project.set_keep_tangent_edges(true);
        assert!(keep(&f.project));
        // The set's own choice wins either way (decision record 0066 item 9).
        set_choice(&mut f.project, Some(false));
        assert!(!keep(&f.project));
        f.project.set_keep_tangent_edges(false);
        set_choice(&mut f.project, Some(true));
        assert!(keep(&f.project));
        // And the generated path follows the B-rep with the tangent edges
        // kept, as handing the option in directly does.
        let read = f.run().unwrap();
        let given = {
            let cancel = CancelToken::new();
            let context = JobContext::new(&cancel, &NoProgress);
            generate_with_part(
                Path::new("."),
                &f.project,
                f.operation,
                &Registry::builtin(),
                None,
                Some(
                    PlacedPart::of_brep(&f.brep, &f.project.models()[0].transform, f.hash).unwrap(),
                ),
                Some(EdgeOptions { keep_tangent: true }),
                &context,
            )
            .unwrap()
        };
        assert_eq!(read.group, given.group);
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
        // An operation with no edge set keeps no tangent edges, whatever
        // the project's default (decision record 0066 item 9).
        let op = f.project.operation(f.operation).unwrap();
        assert_eq!(edge_options(&f.project, op), EdgeOptions::default());
        let mut project = f.project.clone();
        project.set_keep_tangent_edges(true);
        assert_eq!(edge_options(&project, op), EdgeOptions::default());
    }

    #[test]
    fn req_op_11_req_nfr_07_a_stock_to_leave_on_a_step_selection_is_refused() {
        let mut f = fixture(drill::NAME, &[]);
        let walls: BTreeSet<FaceId> = f.built[1].walls.iter().copied().collect();
        f.select(SelectionTarget::Faces(walls), f.hash);
        let mut set = f.project.selection_sets()[0].clone();
        set.stock_to_leave = Some(0.2);
        f.project.replace(Item::SelectionSet(set)).unwrap();
        let err = f.run().unwrap_err().to_string();
        assert!(err.contains("'Op'") && !err.contains("OP-11"), "{err}");
    }

    #[test]
    fn req_op_10_the_part_bounds_are_the_whole_parts_when_faces_are_selected() {
        let whole = fixture(drill::NAME, &[]).run().unwrap().part_bounds;
        let mut f = fixture(drill::NAME, &[]);
        let walls: BTreeSet<FaceId> = f.built[1].walls.iter().copied().collect();
        f.select(SelectionTarget::Faces(walls), f.hash);
        let one = f.run().unwrap().part_bounds;
        assert_eq!(one, whole);
        // The blank, moved by the placement.
        assert!((whole.min.x - SHIFT).abs() < 1e-9 && (whole.max.x - SHIFT - LENGTH).abs() < 1e-9);
    }

    /// The swarf fixture: a round boss wall (face 0, a cylinder of
    /// `BOSS_RADIUS` about an axis parallel to Z through X `BOSS_X`, from
    /// `FOOT` to `TOP` above the rotary axis), a band of a sphere (face 1,
    /// not ruled) and a cylinder coaxial with the rotary axis (face 2, ruled
    /// along the axis), as one STEP part, cut by a 6 mm flat end mill.
    const BOSS_X: f64 = 30.0;
    const BOSS_RADIUS: f64 = 8.0;
    const FOOT: f64 = 10.0;
    const TOP: f64 = 18.0;
    const FLAT: f64 = 6.0;

    fn swarf_fixture() -> Fixture {
        use openchuck_testkit::ruled::{boss_cylinder, coaxial_cylinder, sphere_band};
        let mut brep = boss_cylinder(BOSS_X, BOSS_RADIUS, FOOT, TOP);
        for (id, mut other) in [
            (1, sphere_band(50.0, 5.0, -0.5, 0.5)),
            (2, coaxial_cylinder(6.0, 0.0, 20.0)),
        ] {
            for face in &mut other.faces {
                face.id = FaceId(id);
            }
            brep.faces.extend(other.faces);
        }
        let flat = Tool {
            shape: ToolShape::FlatEnd,
            diameter: Millimetres(FLAT),
            flute_length: Millimetres(15.0),
            overall_length: Millimetres(60.0),
            shank_diameter: Millimetres(FLAT),
            flute_count: 2,
            material: None,
            centre_cutting: None,
        };
        fixture_on(swarf::NAME, &[], brep, Vec::new(), "boss.step", flat, 20.0)
    }

    #[test]
    fn req_op_07_req_nfr_12_a_swarf_overcut_above_the_tolerance_is_made_with_a_warning() {
        // Decision 0175 (issue #226): a straight tool on a drafted wall cuts
        // into it beyond the tolerance. The path is made, with a warning that
        // names the operation's tolerance, which reaches the operation's
        // path (and so the operation's state and Messages) and its notes.
        use openchuck_testkit::ruled::draft_cone;
        let brep = draft_cone(BOSS_X, 10.0, 6f64.to_radians(), FOOT, 16.0);
        let flat = Tool {
            shape: ToolShape::FlatEnd,
            diameter: Millimetres(FLAT),
            flute_length: Millimetres(15.0),
            overall_length: Millimetres(60.0),
            shank_diameter: Millimetres(FLAT),
            flute_count: 2,
            material: None,
            centre_cutting: None,
        };
        let mut f = fixture_on(
            swarf::NAME,
            &[(swarf::parameter::MAX_MATERIAL_LEFT, Value::Number(1.0))],
            brep,
            Vec::new(),
            "cone.step",
            flat,
            20.0,
        );
        f.select(SelectionTarget::Faces([FaceId(0)].into()), f.hash);
        let path = f.run().unwrap();
        assert_eq!(path.warnings.len(), 1, "{:?}", path.warnings);
        let warning = &path.warnings[0];
        assert_eq!(warning.parameter.as_deref(), Some(names::TOLERANCE));
        assert!(
            warning
                .message
                .contains("cuts into the finished surface by")
                && warning.message.contains("tolerance of 0.01 mm"),
            "{}",
            warning.message
        );
        assert!(
            path.notes.iter().any(|n| n.contains(&warning.message)),
            "{:?}",
            path.notes
        );
    }

    #[test]
    fn req_op_07_req_op_02_selected_faces_choose_the_swarf_walls_on_a_step_part() {
        // Swarf needs a face set (decision 0111 item 1): with none, the form
        // says so and the operation is refused before anything is cut.
        let f = swarf_fixture();
        let form =
            crate::form::parameter_form(&f.project, &Registry::builtin(), f.operation).unwrap();
        assert!(
            form.notes
                .first()
                .is_some_and(|n| n.contains("needs a face set")),
            "{:?}",
            form.notes
        );
        let err = f.run().unwrap_err().to_string();
        assert!(
            err.contains("needs a face set") && err.contains("ruled walls") && err.contains("'Op'"),
            "{err}"
        );
        // A set of the boss wall alone is cut, on its flank: the tip at the
        // foot, one tool radius outside the wall (decision 0061 items 3 to 5).
        let mut f = swarf_fixture();
        f.select(SelectionTarget::Faces([FaceId(0)].into()), f.hash);
        let form =
            crate::form::parameter_form(&f.project, &Registry::builtin(), f.operation).unwrap();
        assert!(
            !form.notes.iter().any(|n| n.contains("face set")),
            "{:?}",
            form.notes
        );
        let path = f.run().unwrap();
        let cuts: Vec<_> = path
            .group
            .segments
            .iter()
            .filter(|s| s.kind == openchuck_types::ir::MoveKind::Cutting)
            .collect();
        assert!(cuts.len() > 8, "{} cutting moves", cuts.len());
        for s in &cuts {
            let tip = s.end.tip;
            let r = (tip.x - BOSS_X - SHIFT).hypot(tip.y);
            assert!(
                (r - BOSS_RADIUS - FLAT / 2.0).abs() < 0.02,
                "tip at {r} from the boss axis"
            );
            assert!((tip.z - FOOT).abs() < 0.02, "tip at Z {}", tip.z);
        }
        assert!(
            path.notes.iter().any(|n| n.contains("face 0")),
            "{:?}",
            path.notes
        );
        // A set must suit swarf (decision 0111 item 2): every face swarf
        // cannot cut is named before it runs, the sphere (not ruled) and the
        // coaxial cylinder (ruled along the rotary axis) together.
        let mut f = swarf_fixture();
        f.select(
            SelectionTarget::Faces([FaceId(0), FaceId(1), FaceId(2)].into()),
            f.hash,
        );
        let err = f.run().unwrap_err().to_string();
        assert!(
            err.contains("'Op'")
                && err.contains("take them out of the set")
                && err.contains("face 1: it is not a ruled surface")
                && err.contains("face 2: its rulings run along the rotary axis")
                && !err.contains("face 0"),
            "{err}"
        );
        // An empty set is refused with swarf's own advice.
        let mut f = swarf_fixture();
        f.select(SelectionTarget::Faces(BTreeSet::new()), f.hash);
        let err = f.run().unwrap_err().to_string();
        assert!(
            err.contains("selects no faces") && err.contains("walls to cut"),
            "{err}"
        );
    }

    /// The fixture of the swarf limits (decision 0176): a flat wall (face
    /// 0, `y = 8`, X from 22 to 38, from `FOOT` to `TOP`, facing +Y), the
    /// end wall across it at X = 38 (face 1, facing -X), and a strip of the
    /// wall from X 22 to 30 as a face of its own (face 2) to bound it.
    fn swarf_limits_fixture(parameters: &[(&str, Value)]) -> Fixture {
        use openchuck_testkit::ruled::{cross_plane, wall_plane};
        let mut brep = wall_plane(8.0, 22.0, 38.0, FOOT, TOP);
        for (id, mut other) in [
            (1, cross_plane(38.0, 8.0, 20.0, FOOT, TOP)),
            (2, wall_plane(8.0, 22.0, 30.0, FOOT, TOP)),
        ] {
            for face in &mut other.faces {
                face.id = FaceId(id);
            }
            brep.faces.extend(other.faces);
        }
        let flat = Tool {
            shape: ToolShape::FlatEnd,
            diameter: Millimetres(FLAT),
            flute_length: Millimetres(15.0),
            overall_length: Millimetres(60.0),
            shank_diameter: Millimetres(FLAT),
            flute_count: 2,
            material: None,
            centre_cutting: None,
        };
        let mut f = fixture_on(
            swarf::NAME,
            parameters,
            brep,
            Vec::new(),
            "wall.step",
            flat,
            20.0,
        );
        f.select(SelectionTarget::Faces([FaceId(0)].into()), f.hash);
        f
    }

    /// The largest X of the tool tip on the cutting moves of `path`, in the
    /// part's frame.
    fn furthest_cut(path: &OperationPath) -> f64 {
        path.group
            .segments
            .iter()
            .filter(|s| s.kind == openchuck_types::ir::MoveKind::Cutting)
            .map(|s| s.end.tip.x - SHIFT)
            .fold(f64::MIN, f64::max)
    }

    #[test]
    fn req_op_02_req_op_07_swarf_on_a_step_part_honours_boundary_avoid_and_check_face_sets() {
        // Without limits the whole wall is cut.
        let plain = swarf_limits_fixture(&[]).run().unwrap();
        assert!((furthest_cut(&plain) - 38.0).abs() < 0.02);
        // The end wall as an avoid set: the pass stops a tool radius short
        // of it, and the note says why (issue #227).
        for role in [Role::Avoid, Role::Check] {
            let mut f = swarf_limits_fixture(&[]);
            f.select_as(
                SelectionTarget::Faces([FaceId(1)].into()),
                f.hash,
                f.model,
                role,
            );
            let path = f.run().unwrap();
            let reach = furthest_cut(&path);
            assert!((reach - 35.0).abs() < 0.02, "{role:?}: {reach}");
            let said = if role == Role::Avoid {
                "cut into an avoid surface"
            } else {
                "touch a check surface"
            };
            assert!(
                path.notes.iter().any(|n| n.contains(said)),
                "{:?}",
                path.notes
            );
        }
        // The strip as the boundary: the pass stops at X = 30.
        let mut f = swarf_limits_fixture(&[]);
        f.select_as(
            SelectionTarget::Faces([FaceId(2)].into()),
            f.hash,
            f.model,
            Role::Boundary,
        );
        let path = f.run().unwrap();
        assert!(
            (furthest_cut(&path) - 30.0).abs() < 0.02,
            "{}",
            furthest_cut(&path)
        );
        assert!(
            path.notes
                .iter()
                .any(|n| n.contains("The boundary limits the pass")),
            "{:?}",
            path.notes
        );
        // With both of the boundary set's limits off, it limits nothing and
        // the operation says so as a warning naming the set (decision 0176
        // item 3, the owner's per-set options).
        let mut f = swarf_limits_fixture(&[]);
        f.select_as(
            SelectionTarget::Faces([FaceId(2)].into()),
            f.hash,
            f.model,
            Role::Boundary,
        );
        f.set_options(|o| {
            o.boundary = BoundaryOptions {
                limits_pass: false,
                limits_tip: false,
            };
        });
        let path = f.run().unwrap();
        assert!((furthest_cut(&path) - 38.0).abs() < 0.02);
        assert_eq!(path.warnings.len(), 1, "{:?}", path.warnings);
        let warning = &path.warnings[0];
        assert!(
            warning
                .message
                .starts_with("The boundary set 'chosen' limits nothing"),
            "{}",
            warning.message
        );
        assert_eq!(warning.parameter, None);
        assert_eq!(path.notes.first(), Some(&warning.message));
        // Limit A alone stops the pass at the strip as before.
        let mut f = swarf_limits_fixture(&[]);
        f.select_as(
            SelectionTarget::Faces([FaceId(2)].into()),
            f.hash,
            f.model,
            Role::Boundary,
        );
        f.set_options(|o| o.boundary.limits_tip = false);
        let path = f.run().unwrap();
        assert!((furthest_cut(&path) - 30.0).abs() < 0.02);
        assert!(
            !path
                .notes
                .iter()
                .any(|n| n.contains("far end of the rulings")),
            "{:?}",
            path.notes
        );
        // An avoid or check set that only warns keeps the whole cut and
        // warns naming the set; one set to Ignore limits nothing, and says
        // so.
        for (role, on_break, said) in [
            (
                Role::Avoid,
                OnBreak::Warn,
                "cuts into the avoid set 'chosen'",
            ),
            (Role::Check, OnBreak::Warn, "to the check set 'chosen'"),
            (
                Role::Avoid,
                OnBreak::Ignore,
                "The avoid set 'chosen' limits nothing",
            ),
            (
                Role::Check,
                OnBreak::Ignore,
                "The check set 'chosen' limits nothing",
            ),
        ] {
            let mut f = swarf_limits_fixture(&[]);
            let id = f.select_as(
                SelectionTarget::Faces([FaceId(1)].into()),
                f.hash,
                f.model,
                role,
            );
            f.set_options(|o| {
                let options = SetOptions { on_break };
                if role == Role::Avoid {
                    o.set_avoid(id, options);
                } else {
                    o.set_check(id, options);
                }
            });
            let path = f.run().unwrap();
            assert!(
                (furthest_cut(&path) - 38.0).abs() < 0.02,
                "{role:?} {on_break:?}: {}",
                furthest_cut(&path)
            );
            assert_eq!(path.warnings.len(), 1, "{:?}", path.warnings);
            assert!(
                path.warnings[0].message.contains(said),
                "{}",
                path.warnings[0].message
            );
        }
        // A wall that is cut and kept clear of is refused, naming it.
        let mut f = swarf_limits_fixture(&[]);
        f.select_as(
            SelectionTarget::Faces([FaceId(0)].into()),
            f.hash,
            f.model,
            Role::Check,
        );
        let err = f.run().unwrap_err().to_string();
        assert!(
            err.contains("'Op'")
                && err.contains("face 0 is in the face set it cuts and in the check set 'chosen'"),
            "{err}"
        );
    }
}
