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
use openchuck_types::job::{JobContext, Progress, ProgressSink};

use openchuck_project::id::OperationId;

use crate::AppError;
use crate::simulation::{SetupState, SimulationReport};

/// The progress stage of making the toolpaths, one step per operation.
pub const STAGE_GENERATE: &str = "regenerate";

/// The progress stage of the stock simulation's comparison with the part,
/// one step per setup, after the toolpaths (issue 214; decision record
/// 0168). The material removal itself runs with each operation, inside
/// [`STAGE_GENERATE`]. One percentage across the stages is lane BUG-PROG's
/// (issue 201).
pub const STAGE_SIMULATE: &str = "simulate the stock";

/// Where the progress of one step of a run goes: an operation's (the
/// strategy's stages and the stock simulation's moves) or a setup's
/// comparison with the part (reading the model, comparing), each with its
/// own count. Shown as they came, they replaced the run's count, so the text
/// jumped between totals in the middle of a run (issue 248). Each is passed
/// on as the run's count instead, at the step still being worked on, so the
/// run is seen to be alive and its count never changes total or goes back
/// within a stage. One percentage across the stages, with the step's own
/// share in it, is lane BUG-PROG's (issue 201): it is made here, from the
/// report this drops.
struct OperationProgress<'a> {
    run: &'a JobContext<'a>,
    /// The run's stage: [`STAGE_GENERATE`] or [`STAGE_SIMULATE`].
    stage: &'static str,
    /// Steps of the stage done before this one.
    done: u64,
    /// Steps in the stage.
    total: u64,
}

impl ProgressSink for OperationProgress<'_> {
    fn report(&self, _own: Progress) {
        self.run.report(self.stage, self.done, Some(self.total));
    }
}

/// The setup, stock and machine an operation's simulation needs.
pub(crate) fn setup_parts<'a>(
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

/// What the application knows about one group of the toolpath that the
/// neutral IR does not carry (the IR's tags are names, which are not unique):
/// the operation and setup it came from and the tool as the simulation
/// sweeps it. One per group, in the same order as `Regeneration::toolpath`.
#[derive(Debug, Clone, PartialEq)]
pub struct GroupInfo {
    /// The operation that made the group.
    pub operation: OperationId,
    /// The setup it belongs to.
    pub setup: openchuck_project::id::SetupId,
    /// The tool's revolved profile.
    pub tool: openchuck_types::tool::RevolvedProfile,
    /// The bounds of the setup's placed part (bounding-box stock needs them
    /// when the stock is built again for playback).
    pub part_bounds: openchuck_types::geometry::Aabb,
    /// The operation's warnings: its path was made but does not meet a limit
    /// it sets (decision record 0175). They make the operation's state a
    /// warning and are listed in Messages and the results.
    pub warnings: Vec<openchuck_path::PathWarning>,
}

/// The result of regenerating a project.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Regeneration {
    /// The toolpath of every operation, in machining order.
    pub toolpath: Toolpath,
    /// Where each group of `toolpath` came from, in the same order
    /// (playback and the display of toolpaths by operation, SIM-01, SIM-04).
    pub groups: Vec<GroupInfo>,
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
    /// Operations whose moves were run through their setup's stock
    /// simulation in this run. An operation's stock is simulated when a later
    /// operation of its setup is made on it (SIM-06) or a simulation is asked
    /// for, unless the cache holds the stock it leaves (decision record 0190).
    pub simulated: Vec<OperationId>,
    /// The operation the run stopped after, when it was asked to generate up
    /// to and including one operation ("generate selected", specification
    /// section 16; decision record 0117, OI-351). The enabled operations after
    /// it are not in this run, and stay stale.
    pub stopped_after: Option<OperationId>,
    /// The project the run was made from. Stock playback and the deviation
    /// colours read the stock, the cell size and the part from here, so they
    /// match the run even if the project has been edited since.
    pub project: Arc<Project>,
}

/// Regenerates every enabled operation of `project` (in the folder
/// `location`) in machining order. The first operation that cannot be
/// generated stops the run with its error: nothing is left out silently
/// (verification policy 8). A disabled operation is skipped as if it were
/// absent: no toolpath, no stock removal, no G-code (UI-06; decision record
/// 0029 item 10). `record` follows which operation is being made, for the
/// item states (decision record 0117).
#[allow(clippy::too_many_arguments)]
pub(crate) fn regenerate(
    location: &Path,
    project: &Project,
    registry: &Registry,
    post: Option<(
        &crate::posting::PostSelection,
        crate::posting::OutputGrouping,
    )>,
    simulate: bool,
    cache: &PipelineCache,
    record: &crate::overview::GenerationRecord,
    until: Option<OperationId>,
    context: &JobContext<'_>,
) -> Result<Regeneration, AppError> {
    context.check_cancelled()?;
    let mut operations: Vec<&openchuck_project::Operation> =
        project.operations().iter().filter(|o| o.enabled).collect();
    // Up to and including `until`: the operations before it are made (or
    // reused) because it cuts the stock they leave; the ones after are not.
    if let Some(last) = until {
        let operation = project
            .operation(last)
            .ok_or_else(|| AppError::UnknownOperation(last.to_string()))?;
        if !operation.enabled {
            return Err(AppError::Operation {
                name: operation.name.clone(),
                reason: "it is turned off, so it is not generated; turn it on first".to_owned(),
            });
        }
        if let Some(end) = operations.iter().position(|o| o.id == last) {
            operations.truncate(end + 1);
        }
    }
    let total = operations.len() as u64;
    context.report(STAGE_GENERATE, 0, Some(total));
    let settings = project.simulation();
    let mut regeneration = Regeneration {
        project: Arc::new(project.clone()),
        stopped_after: until,
        ..Regeneration::default()
    };
    // Each setup's simulation so far. It is shared with the cache, which keeps
    // it as the stock an operation leaves, and copied only when the run
    // simulates on from a state the cache holds (decision record 0190).
    let mut setups: Vec<Arc<SetupState>> = Vec::new();
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
        // A failure from here to the next operation is this operation's.
        record.working_on(Some(operation.id));
        let own = OperationProgress {
            run: context,
            stage: STAGE_GENERATE,
            done: i as u64,
            total,
        };
        let within = JobContext::new(context.cancel, &own);
        // The key chains from the operations before, which fixes the stock
        // they leave (SIM-06), so a hit is valid on carried stock too.
        let key = if caching {
            Some(crate::cache::operation_key(
                project, operation, previous, &files,
            )?)
        } else {
            None
        };
        let hit = key.and_then(|k| cache.get(operation.id, k));
        let cached_path = hit.is_some();
        let (path, cached_stock) = if let Some(hit) = hit {
            regeneration.reused.push(operation.id);
            (hit.path, hit.stock)
        } else {
            // Only an operation that is made, or simulated, shows as running.
            record.doing(crate::overview::PipelineActivity::Generating);
            let carried = setups
                .iter()
                .find(|s| s.id == operation.setup)
                .map(|s| s.stock() as &dyn StockModel);
            let made = Arc::new(crate::operation::generate_operation_on(
                location,
                project,
                operation.id,
                registry,
                carried,
                &within,
            )?);
            if let Some(key) = key
                && files.unchanged(location, project, operation)
            {
                cache.put(operation.id, key, Arc::clone(&made), generation);
            }
            regeneration.generated.push(operation.id);
            (made, None)
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
            let mut index = setups.iter().position(|s| s.id == operation.setup);
            if let Some(stock) = cached_stock {
                // The stock this operation leaves is cached under its key,
                // which fixes everything simulated before it in the setup.
                match index.and_then(|at| setups.get_mut(at)) {
                    Some(slot) => *slot = stock,
                    None => {
                        setups.push(stock);
                        index = Some(setups.len() - 1);
                    }
                }
            } else {
                if cached_path {
                    // The path is reused, but the stock it leaves must be
                    // worked out again; the status bar says so.
                    record.doing(crate::overview::PipelineActivity::Resimulating);
                }
                let at = match index {
                    Some(at) => at,
                    None => {
                        let (setup, stock, machine) = setup_parts(project, operation)?;
                        setups.push(Arc::new(SetupState::new(
                            setup.id,
                            &setup.name,
                            stock,
                            path.part_bounds,
                            machine,
                            settings,
                        )?));
                        setups.len() - 1
                    }
                };
                index = Some(at);
                if let Some(state) = setups.get_mut(at) {
                    Arc::make_mut(state).run(
                        operation.id,
                        &operation.name,
                        &path.group,
                        &path.tool,
                        &within,
                    )?;
                    regeneration.simulated.push(operation.id);
                    if let Some(key) = key {
                        cache.put_stock(operation.id, key, Arc::clone(state), generation);
                    }
                }
            }
            let cuts = index
                .and_then(|at| setups.get(at))
                .map(|s| s.last_rapid_cuts())
                .unwrap_or_default();
            for cut in cuts {
                regeneration.notes.push(format!(
                    "operation '{}': move {} is a rapid that removes {:.3} mm3 of material",
                    operation.name, cut.segment, cut.volume
                ));
            }
        }
        regeneration.toolpath.groups.push(path.group.clone());
        regeneration.groups.push(GroupInfo {
            operation: operation.id,
            setup: operation.setup,
            tool: path.tool.clone(),
            part_bounds: path.part_bounds,
            warnings: path.warnings.clone(),
        });
        context.report(STAGE_GENERATE, i as u64 + 1, Some(total));
    }
    record.working_on(None);
    if simulate {
        let mut report = SimulationReport {
            settings,
            setups: Vec::new(),
        };
        let count = setups.len() as u64;
        for (done, state) in (0_u64..).zip(setups) {
            context.report(STAGE_SIMULATE, done, Some(count));
            context.check_cancelled()?;
            let own = OperationProgress {
                run: context,
                stage: STAGE_SIMULATE,
                done,
                total: count,
            };
            let within = JobContext::new(context.cancel, &own);
            let mut state = Arc::unwrap_or_clone(state);
            state.rename(project);
            let setup = project.setup(state.id);
            let part = setup
                .and_then(|s| s.part)
                .and_then(|id| project.models().iter().find(|m| m.id == id));
            // The held regions and tabs are part of what the stock is
            // compared with (SET-09, OP-21).
            let mesh = match (part, setup) {
                (Some(model), Some(setup)) => Some(crate::workholding::setup_deviation_target(
                    setup,
                    crate::model::setup_mesh(location, model, setup, &within)?,
                )?),
                _ => None,
            };
            report
                .setups
                .push(state.finish(mesh.as_ref(), settings, &within)?);
        }
        context.report(STAGE_SIMULATE, count, Some(count));
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
    if let Some((selection, grouping)) = post {
        context.check_cancelled()?;
        let (files, reports, notes) =
            crate::posting::post_toolpath(project, &regeneration.toolpath, selection, grouping)?;
        regeneration.gcode = files;
        regeneration.reports = reports;
        regeneration.notes.extend(notes);
    }
    Ok(regeneration)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]

    use std::sync::Mutex;

    use openchuck_path::{CutPoint, ParameterSchema, Parameters, Pass, Strategy, StrategyInput};
    use openchuck_types::job::CancelToken;
    use openchuck_types::units::{Degrees, Millimetres};

    use super::*;
    use crate::overview::{GenerationRecord, PipelineActivity};
    use crate::test_project::TestProject;
    use crate::{Application, Command};

    /// One ring of cuts at model X = 0 (a test strategy, OP-16).
    #[derive(Debug)]
    struct OneRing;

    impl Strategy for OneRing {
        fn name(&self) -> &str {
            "one_ring"
        }
        fn label(&self) -> &str {
            "One ring"
        }

        fn description(&self) -> &str {
            "A strategy for tests."
        }
        fn schema(&self) -> ParameterSchema {
            ParameterSchema::with_common(Vec::new())
        }
        fn passes(
            &self,
            input: &StrategyInput<'_>,
            _: &Parameters,
            context: &JobContext<'_>,
        ) -> openchuck_path::Result<Vec<Pass>> {
            // Its own stages with their own counts, as the strategies report
            // them (issue 248).
            context.report("one ring: dropping the tool", 0, Some(37));
            context.report("one ring: fitting the path", 0, Some(1));
            let points = (0..=36)
                .filter_map(|i| {
                    let a = Degrees(10.0 * f64::from(i));
                    let line = input
                        .rotary
                        .tool_line(Millimetres(0.0), a, Millimetres(0.0));
                    let c = input.surface.drop_tool(input.tool, &line)?;
                    Some(CutPoint {
                        tip: c.tip,
                        a,
                        contact: c.contact,
                    })
                })
                .collect();
            Ok(vec![Pass {
                points,
                stock: None,
                rapids: Vec::new(),
            }])
        }
    }

    /// Notes what the record says the run is doing each time the run
    /// reports progress while it makes the toolpaths.
    struct Spy {
        record: Arc<GenerationRecord>,
        seen: Mutex<Vec<(OperationId, PipelineActivity)>>,
    }

    impl ProgressSink for Spy {
        fn report(&self, progress: Progress) {
            if progress.stage == STAGE_GENERATE
                && let Some(activity) = self.record.activity()
            {
                self.seen.lock().unwrap().push(activity);
            }
        }
    }

    /// Runs the pipeline with a simulation, as the Simulate job does, and
    /// returns the run with what the record said during the simulation.
    fn simulate(app: &Application) -> (Regeneration, Vec<(OperationId, PipelineActivity)>) {
        let spy = Spy {
            record: Arc::clone(&app.record),
            seen: Mutex::new(Vec::new()),
        };
        let cancel = CancelToken::new();
        let context = JobContext::new(&cancel, &spy);
        let run = regenerate(
            app.location(),
            app.project(),
            &app.strategies,
            None,
            true,
            &app.cache,
            &app.record,
            None,
            &context,
        )
        .unwrap();
        (run, spy.seen.into_inner().unwrap())
    }

    #[test]
    fn req_nfr_04_req_ui_09_a_replayed_simulation_says_so_and_one_stock_is_kept_per_operation() {
        // Decision record 0190, the owner's choice B: one stock is kept per
        // operation, the latest. Going back to an earlier state reuses the
        // paths, but their stock is worked out again, and the status bar
        // says so; a reused operation is never said to be generating.
        let mut p = TestProject::new("replay");
        p.app.register_strategy(Arc::new(OneRing)).unwrap();
        let ring = p.add_operation("Ring", "one_ring");
        let next = p.add_operation("Ring again", "one_ring");
        let ids = [ring, next];

        let (first, seen) = simulate(&p.app);
        assert_eq!(first.generated, ids);
        assert_eq!(first.simulated, ids);
        assert!(seen.contains(&(ring, PipelineActivity::Generating)));
        assert_eq!(p.app.cache.stocks_kept(), 2);

        // Nothing changed: no simulation runs, so nothing is said.
        let (again, seen) = simulate(&p.app);
        assert_eq!(again.reused, ids);
        assert!(again.simulated.is_empty());
        assert!(seen.is_empty(), "{seen:?}");

        p.app
            .execute(Command::SetParameter {
                operation: ring,
                name: openchuck_path::params::names::STOCK_TO_LEAVE.into(),
                value: Some(openchuck_project::ParameterValue::Number(0.3)),
            })
            .unwrap();
        let (edited, _) = simulate(&p.app);
        assert_eq!(edited.generated, ids);
        // Each operation keeps one stock, the latest, though it keeps three
        // paths.
        assert_eq!(p.app.cache.stocks_kept(), 2);

        p.app.undo().unwrap();
        let (back, seen) = simulate(&p.app);
        assert!(back.generated.is_empty());
        assert_eq!(back.reused, ids);
        assert_eq!(back.simulated, ids);
        assert!(
            seen.contains(&(ring, PipelineActivity::Resimulating)),
            "{seen:?}"
        );
        assert!(
            !seen.iter().any(|(_, a)| *a == PipelineActivity::Generating),
            "{seen:?}"
        );
        assert_eq!(p.app.cache.stocks_kept(), 2);
        assert_eq!(
            PipelineActivity::Resimulating.text("Ring"),
            "Re-running the simulation for 'Ring'"
        );
        // Between runs, nothing is being done.
        assert_eq!(p.app.pipeline_activity(), None);
    }

    /// Keeps every progress report.
    #[derive(Default)]
    struct Reports(Mutex<Vec<Progress>>);

    impl ProgressSink for Reports {
        fn report(&self, progress: Progress) {
            self.0.lock().unwrap().push(progress);
        }
    }

    #[test]
    fn req_ui_09_the_progress_counts_operations_with_one_total_that_only_goes_up() {
        // Issue 248: the strategies and the stock simulation report their
        // own stages, each with its own count, and those reports replaced
        // the run's count of operations, so the text shown jumped between
        // totals ("0 of 1", "0 of 10") in the middle of a run. The run shows
        // one count, of operations, then one of setups for the comparison
        // with the part.
        let mut p = TestProject::new("progress");
        p.app.register_strategy(Arc::new(OneRing)).unwrap();
        p.add_operation("Ring", "one_ring");
        p.add_operation("Ring again", "one_ring");
        let reports = Reports::default();
        let cancel = CancelToken::new();
        let context = JobContext::new(&cancel, &reports);
        regenerate(
            p.app.location(),
            p.app.project(),
            &p.app.strategies,
            None,
            true,
            &p.app.cache,
            &p.app.record,
            None,
            &context,
        )
        .unwrap();
        let reports = reports.0.into_inner().unwrap();
        let generate: Vec<&Progress> = reports
            .iter()
            .take_while(|r| r.stage != STAGE_SIMULATE)
            .collect();
        assert!(generate.len() > 3, "{reports:?}");
        let mut done = 0;
        for r in &generate {
            assert_eq!(r.stage, STAGE_GENERATE, "{reports:?}");
            assert_eq!(r.total, Some(2), "{reports:?}");
            assert!(r.done >= done, "{reports:?}");
            done = r.done;
        }
        assert_eq!(done, 2);
        let mut done = 0;
        for r in reports.iter().skip(generate.len()) {
            assert_eq!(r.stage, STAGE_SIMULATE, "{reports:?}");
            assert_eq!(r.total, Some(1), "{reports:?}");
            assert!(r.done >= done, "{reports:?}");
            done = r.done;
        }
        assert_eq!(done, 1);
    }
}
