// SPDX-License-Identifier: AGPL-3.0-or-later
//! The operation pipeline (architecture section 6). Each operation's
//! toolpath is generated in machining order (OP-01, slice 5); operations
//! after the first in a setup are generated against the stock the earlier
//! ones left, as the simulation carries it (SIM-06, slice 7); a simulation
//! report follows when asked for, and posting when a post is chosen (slice 6).

use std::path::Path;
use std::sync::Arc;

use openchuck_path::{Registry, StockModel};
use openchuck_project::Project;
use openchuck_types::ir::Toolpath;
use openchuck_types::job::JobContext;

use openchuck_project::id::OperationId;

use crate::AppError;
use crate::simulation::{SetupState, SimulationReport};

/// The setup, stock and machine an operation's simulation needs.
fn setup_parts<'a>(
    project: &'a Project,
    operation: &openchuck_project::Operation,
) -> Result<
    (
        &'a openchuck_project::Setup,
        &'a openchuck_types::stock::Stock,
        &'a openchuck_types::machine::MachineProfile,
    ),
    AppError,
> {
    let fail = |what: &str| AppError::Operation {
        name: operation.name.clone(),
        reason: what.to_owned(),
    };
    let setup = project
        .setup(operation.setup)
        .ok_or_else(|| fail("its setup is missing"))?;
    let stock = setup
        .stock
        .as_ref()
        .ok_or_else(|| fail("its setup has no stock"))?;
    let machine = project
        .machine()
        .ok_or_else(|| fail("the project has no machine profile"))?;
    Ok((setup, stock, machine))
}
use crate::cache::PipelineCache;

/// The result of regenerating a project.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Regeneration {
    /// The toolpath of every operation, in machining order.
    pub toolpath: Toolpath,
    /// Generated G-code files as (file name, contents), in output order.
    pub gcode: Vec<(String, String)>,
    /// Notes about the toolpaths, each naming its operation, in order.
    pub notes: Vec<String>,
    /// What the G-code checks found for each file in `gcode` (GC-08, GC-11).
    pub reports: Vec<crate::posting::GcodeReport>,
    /// The material removal simulation, when one was asked for (slice 7).
    pub simulation: Option<crate::simulation::SimulationReport>,
    /// Operations whose toolpath was generated in this run.
    pub generated: Vec<OperationId>,
    /// Operations whose toolpath came from the cache because nothing it
    /// depends on, including the operations before it, changed.
    pub reused: Vec<OperationId>,
}

/// Regenerates every enabled operation of `project` (in the folder
/// `location`) in machining order. The first operation that cannot be
/// generated stops the run with its error: nothing is left out silently
/// (verification policy 8). A disabled operation is skipped as if it were
/// absent: no toolpath, no stock removal, no G-code (UI-06; decision record
/// 0029 item 10).
pub(crate) fn regenerate(
    location: &Path,
    project: &Project,
    registry: &Registry,
    post: Option<&crate::posting::PostSelection>,
    simulate: bool,
    cache: &PipelineCache,
    context: &JobContext<'_>,
) -> Result<Regeneration, AppError> {
    context.check_cancelled()?;
    let operations: Vec<&openchuck_project::Operation> =
        project.operations().iter().filter(|o| o.enabled).collect();
    let total = operations.len() as u64;
    context.report("regenerate", 0, Some(total));
    let settings = project.simulation();
    let mut regeneration = Regeneration::default();
    let mut setups: Vec<SetupState> = Vec::new();
    // Disabled operations keep their cached results, so enabling one again
    // can reuse them; only removed operations are dropped.
    let ids: Vec<OperationId> = project.operations().iter().map(|o| o.id).collect();
    cache.retain_only(&ids);
    // With caching off, none of the hashing and keying is done.
    let caching = cache.versions() != 0;
    let generation = cache.generation();
    let files = if caching {
        crate::cache::ModelFiles::read(location, project)
    } else {
        crate::cache::ModelFiles::default()
    };
    let mut previous = None;
    for (i, operation) in operations.iter().enumerate() {
        context.check_cancelled()?;
        let carried = setups
            .iter()
            .find(|s| s.id == operation.setup)
            .map(|s| s.stock() as &dyn StockModel);
        // The key chains from the operations before, which fixes the stock
        // they leave (SIM-06), so a hit is valid on carried stock too.
        let key = if caching {
            Some(crate::cache::operation_key(
                project, operation, previous, &files,
            )?)
        } else {
            None
        };
        let path = if let Some(hit) = key.and_then(|k| cache.get(operation.id, k)) {
            regeneration.reused.push(operation.id);
            hit
        } else {
            let made = Arc::new(crate::operation::generate_operation_on(
                location,
                project,
                operation.id,
                registry,
                carried,
                context,
            )?);
            if let Some(key) = key
                && files.unchanged(location, project, operation)
            {
                cache.put(operation.id, key, Arc::clone(&made), generation);
            }
            regeneration.generated.push(operation.id);
            made
        };
        previous = key;
        for note in &path.notes {
            regeneration
                .notes
                .push(format!("operation '{}': {note}", operation.name));
        }
        let followed = operations
            .get(i + 1..)
            .unwrap_or_default()
            .iter()
            .any(|o| o.setup == operation.setup);
        if simulate || followed {
            let index = match setups.iter().position(|s| s.id == operation.setup) {
                Some(index) => index,
                None => {
                    let (setup, stock, machine) = setup_parts(project, operation)?;
                    setups.push(SetupState::new(
                        setup.id,
                        &setup.name,
                        stock,
                        path.part_bounds,
                        machine,
                        settings,
                    )?);
                    setups.len() - 1
                }
            };
            if let Some(state) = setups.get_mut(index) {
                let result = state.run(&operation.name, &path.group, &path.tool, context)?;
                for cut in result.rapid_cuts {
                    regeneration.notes.push(format!(
                        "operation '{}': move {} is a rapid that removes {:.3} mm3 of material",
                        operation.name, cut.segment, cut.volume
                    ));
                }
            }
        }
        regeneration.toolpath.groups.push(path.group.clone());
        context.report("regenerate", i as u64 + 1, Some(total));
    }
    if simulate {
        let mut report = SimulationReport {
            settings,
            setups: Vec::new(),
        };
        for state in setups {
            context.check_cancelled()?;
            let part = project
                .setup(state.id)
                .and_then(|s| s.part)
                .and_then(|id| project.models().iter().find(|m| m.id == id));
            let mesh = match part {
                Some(model) => Some(crate::model::placed_mesh(location, model, context)?),
                None => None,
            };
            report.setups.push(state.finish(mesh.as_ref(), settings)?);
        }
        regeneration.simulation = Some(report);
    }
    // Every operation was keyed and made from the part files as they were at
    // the start. A file that changed since would leave results from two
    // snapshots side by side, so the run is refused instead.
    if caching && !files.parts_unchanged(location, project) {
        return Err(AppError::Cache(
            "a model file changed while regenerating; regenerate again".to_owned(),
        ));
    }
    regeneration.toolpath.validate()?;
    if let Some(selection) = post {
        context.check_cancelled()?;
        let (files, reports, notes) =
            crate::posting::post_toolpath(project, &regeneration.toolpath, selection)?;
        regeneration.gcode = files;
        regeneration.reports = reports;
        regeneration.notes.extend(notes);
    }
    Ok(regeneration)
}
