// SPDX-License-Identifier: AGPL-3.0-or-later
//! The stock at any point of the playback, from the simulator's checkpoints
//! (SIM-04, SIM-05; work breakdown slice 9 items 5 and 6; decision record
//! 0040).
//!
//! The pipeline's simulation keeps no checkpoints, because each is a copy of
//! the stock (decision 0025 item 7). Playback needs them, so it runs the
//! simulation again with checkpoints at the start, after each operation and
//! every so many moves ([`start_stock_playback`]), and then
//! [`StockPlayback::stock_at`] replays only from the nearest one to the point
//! asked for. Both run as jobs.
//!
//! [`start_stock_playback`]: crate::Application::start_stock_playback

use std::sync::Arc;

use openchuck_project::id::{ModelId, OperationId, SetupId};
use openchuck_sim::{DeviationOptions, DexelStock, Simulator, stock_mesh, triangle_values};
use openchuck_types::job::JobContext;
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::tool::RevolvedProfile;

use crate::pathview::{DeviationBands, deviation_bands};
use crate::playback::{Cursor, Timeline};
use crate::{AppError, Application, JobHandle, Regeneration};

/// The most memory the checkpoints of one setup may hold, in bytes. Each is a
/// copy of the stock; a project that would need more is refused with a hint
/// to coarsen the simulation cell.
pub const MAX_CHECKPOINT_BYTES: usize = 1_536 * 1024 * 1024;

/// Checkpoints kept within an operation besides the one after it.
const CHECKPOINTS_WITHIN: usize = 8;

/// The fewest moves between two checkpoints inside an operation.
const MIN_CHECKPOINT_SPACING: usize = 1_000;

struct SetupPlay {
    id: SetupId,
    simulator: Simulator,
    /// The groups run on this setup's stock, in order, counted across all
    /// groups of the timeline.
    groups: Vec<usize>,
}

/// The simulation of every setup with its checkpoints, to give the stock at a
/// point of the playback.
pub struct StockPlayback {
    timeline: Arc<Timeline>,
    setups: Vec<SetupPlay>,
}

impl std::fmt::Debug for StockPlayback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StockPlayback")
            .field("setups", &self.setups.len())
            .finish_non_exhaustive()
    }
}

impl StockPlayback {
    /// The timeline this plays.
    pub fn timeline(&self) -> &Arc<Timeline> {
        &self.timeline
    }

    /// The stock at `cursor`: after the whole moves done, not part-way into
    /// the next (the dexel stock changes at samples a cell apart, and the
    /// part of a move already cut is shown by the tool drawn at the cursor).
    pub fn stock_at(
        &self,
        cursor: Cursor,
        context: &JobContext<'_>,
    ) -> Result<DexelStock, AppError> {
        let timeline = &self.timeline;
        let path = &timeline.run().toolpath;
        let (group, moves) = match timeline.locate(cursor.done) {
            Some(place) => place,
            None => {
                let last = path
                    .groups
                    .iter()
                    .rposition(|g| !g.segments.is_empty())
                    .unwrap_or(0);
                (last, path.groups.get(last).map_or(0, |g| g.segments.len()))
            }
        };
        let setup = self
            .setups
            .iter()
            .find(|s| s.groups.contains(&group))
            .ok_or_else(|| AppError::Playback("no simulation covers this move".to_owned()))?;
        let local = setup
            .groups
            .iter()
            .position(|g| *g == group)
            .ok_or_else(|| AppError::Playback("no simulation covers this move".to_owned()))?;
        let infos = &timeline.run().groups;
        let groups: Vec<(&openchuck_types::ir::ToolpathGroup, &RevolvedProfile)> = setup
            .groups
            .iter()
            .filter_map(|g| Some((path.groups.get(*g)?, &infos.get(*g)?.tool)))
            .collect();
        Ok(setup.simulator.stock_at(local, moves, &groups, context)?)
    }

    /// The stock at `cursor` as a closed mesh for the viewport.
    pub fn mesh_at(
        &self,
        cursor: Cursor,
        context: &JobContext<'_>,
    ) -> Result<TriangleMesh, AppError> {
        Ok(stock_mesh(&self.stock_at(cursor, context)?))
    }

    /// The setups simulated, in the order their first operation runs.
    pub fn setups(&self) -> Vec<SetupId> {
        self.setups.iter().map(|s| s.id).collect()
    }

    /// How many checkpoints are kept in all.
    pub fn checkpoints(&self) -> usize {
        self.setups
            .iter()
            .map(|s| s.simulator.checkpoints().len())
            .sum()
    }
}

impl Application {
    /// Simulates `timeline`'s toolpath again with checkpoints, as a job, so
    /// that the stock at any move can be shown (SIM-04). The pipeline's own
    /// simulation keeps none (decision 0025 item 7). Fails, naming the cell
    /// to coarsen, when the checkpoints would need more than
    /// [`MAX_CHECKPOINT_BYTES`] for a setup.
    pub fn start_stock_playback(&self, timeline: Arc<Timeline>) -> JobHandle<StockPlayback> {
        let project = self.document.project.clone();
        self.jobs
            .submit("prepare playback", self.events_tx.clone(), move |context| {
                let run = Arc::clone(timeline.run());
                let settings = project.simulation();
                let machine = project
                    .machine()
                    .ok_or_else(|| AppError::Playback("the project has no machine".to_owned()))?;
                // The groups of each setup, in order of first use.
                let mut plan: Vec<(SetupId, Vec<usize>)> = Vec::new();
                for (g, info) in run.groups.iter().enumerate() {
                    match plan.iter_mut().find(|(s, _)| *s == info.setup) {
                        Some((_, groups)) => groups.push(g),
                        None => plan.push((info.setup, vec![g])),
                    }
                }
                let total: u64 = run.groups.len() as u64;
                let mut done = 0u64;
                context.report("prepare playback", 0, Some(total));
                let mut setups = Vec::new();
                for (id, groups) in plan {
                    let first = groups
                        .first()
                        .and_then(|g| run.groups.get(*g))
                        .ok_or_else(|| AppError::Playback("a group has no record".to_owned()))?;
                    let operation = project.operation(first.operation).ok_or_else(|| {
                        AppError::Playback(
                            "the project changed since the toolpath was made; regenerate it"
                                .to_owned(),
                        )
                    })?;
                    let (_, stock_definition, _) =
                        crate::pipeline::setup_parts(&project, operation)?;
                    let stock = DexelStock::from_stock(
                        stock_definition,
                        Some(first.part_bounds),
                        settings.cell.0,
                    )?;
                    let moves: usize = groups
                        .iter()
                        .filter_map(|g| run.toolpath.groups.get(*g))
                        .map(|g| g.segments.len())
                        .sum();
                    let every = (moves / CHECKPOINTS_WITHIN).max(MIN_CHECKPOINT_SPACING);
                    let copies = groups.len() + 2 + moves / every;
                    let bytes = copies.saturating_mul(stock.memory_bytes());
                    if bytes > MAX_CHECKPOINT_BYTES {
                        return Err(AppError::Playback(format!(
                            "stock playback of this setup would keep {copies} copies of a stock \
                             of {:.0} MB, over the {:.0} MB allowed; make the simulation cell \
                             coarser (now {} mm) or play the toolpath without the stock",
                            stock.memory_bytes() as f64 / 1e6,
                            MAX_CHECKPOINT_BYTES as f64 / 1e6,
                            settings.cell.0
                        )));
                    }
                    let mut simulator = Simulator::new(stock, machine, Some(every));
                    for g in &groups {
                        context.check_cancelled()?;
                        let (Some(path), Some(info)) =
                            (run.toolpath.groups.get(*g), run.groups.get(*g))
                        else {
                            continue;
                        };
                        simulator.run_group(path, &info.tool, context)?;
                        done += 1;
                        context.report("prepare playback", done, Some(total));
                    }
                    setups.push(SetupPlay {
                        id,
                        simulator,
                        groups,
                    });
                }
                Ok(StockPlayback { timeline, setups })
            })
    }

    /// Builds the stock at `cursor` as a mesh, as a job (replays from the
    /// nearest checkpoint).
    pub fn start_stock_at(
        &self,
        playback: &Arc<StockPlayback>,
        cursor: Cursor,
    ) -> JobHandle<TriangleMesh> {
        let playback = Arc::clone(playback);
        self.jobs
            .submit("stock at move", self.events_tx.clone(), move |context| {
                playback.mesh_at(cursor, context)
            })
    }

    /// Sorts the triangles of the part of `setup` into bands by the deviation
    /// the simulation found, as a job (SIM-05). `run` must have been made
    /// with a simulation.
    pub fn start_deviation_bands(
        &self,
        run: Arc<Regeneration>,
        setup: SetupId,
    ) -> JobHandle<(ModelId, DeviationBands)> {
        let document = self.document.clone();
        self.jobs.submit(
            "colour by deviation",
            self.events_tx.clone(),
            move |context| {
                let report = run.simulation.as_ref().ok_or_else(|| {
                    AppError::Playback("the toolpath was generated without a simulation".to_owned())
                })?;
                let result = report
                    .setups
                    .iter()
                    .find(|s| s.id == setup)
                    .ok_or_else(|| {
                        AppError::Playback("the simulation did not cover this setup".to_owned())
                    })?;
                let deviation = result.deviation.as_ref().ok_or_else(|| {
                    AppError::Playback("this setup has no part to compare with".to_owned())
                })?;
                let model = document
                    .project
                    .setup(setup)
                    .and_then(|s| s.part)
                    .and_then(|id| document.project.models().iter().find(|m| m.id == id))
                    .ok_or_else(|| AppError::Playback("the setup has no part".to_owned()))?;
                let mesh = crate::model::placed_mesh(&document.location, model, context)?;
                let options = DeviationOptions::for_cell(report.settings.cell.0);
                let values = triangle_values(deviation, &mesh, options.spacing)?;
                Ok((model.id, deviation_bands(&values, report.settings.cell.0)))
            },
        )
    }
}

/// The operation a group came from, for a legend; kept here so a front end
/// need not reach into the run.
pub fn group_operation(run: &Regeneration, group: usize) -> Option<OperationId> {
    run.groups.get(group).map(|g| g.operation)
}
