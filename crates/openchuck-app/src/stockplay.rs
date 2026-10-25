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
pub use crate::stock_surface::StockSurface;
use crate::stock_surface::SurfaceSamples;
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
    model: Option<ModelId>,
    samples: Option<SurfaceSamples>,
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

    /// The stock at `cursor`, including the swept part of the current move.
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
        let stock = setup.simulator.stock_at(local, moves, &groups, context)?;
        let Some(segment) = path.groups.get(group).and_then(|g| g.segments.get(moves)) else {
            return Ok(stock);
        };
        if cursor.fraction <= 0.0 {
            return Ok(stock);
        }
        let pose = timeline.sample(cursor)?;
        let original = &path.groups[group];
        let start = moves
            .checked_sub(1)
            .and_then(|i| original.segments.get(i))
            .map_or(original.start, |s| s.end);
        let mut partial = *segment;
        partial.end = openchuck_types::ir::ToolPose {
            tip: pose.tip,
            axis: openchuck_types::geometry::UnitVec3::new(pose.axis)?,
            a: openchuck_types::units::Degrees(pose.a),
        };
        let fraction = cursor.fraction.clamp(0.0, 1.0);
        partial.duration.0 *= fraction;
        partial.contact_length.0 *= fraction;
        let partial = openchuck_types::ir::ToolpathGroup {
            tags: original.tags.clone(),
            start,
            segments: vec![partial],
        };
        let frame = timeline
            .work_frame(group)
            .ok_or_else(|| AppError::Playback("no frame covers this move".to_owned()))?;
        let mut simulator = Simulator::without_checkpoints(stock, &frame);
        simulator.run_group(&partial, &infos[group].tool, context)?;
        Ok(simulator.into_stock())
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
        self.jobs
            .submit("prepare playback", self.events_tx.clone(), move |context| {
                let run = Arc::clone(timeline.run());
                // The run's own project, not the current one: the stock, the
                // cell size and the machine must be those the run was made with.
                let project = Arc::clone(&run.project);
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
                // Every setup's simulator is kept, so the limit is on the total.
                let mut kept_bytes = 0usize;
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
                    kept_bytes = add_checkpoint_bytes(
                        kept_bytes,
                        copies,
                        stock.memory_bytes(),
                        settings.cell.0,
                    )?;
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
                        model: project.setup(id).and_then(|s| s.part),
                        samples: run
                            .simulation
                            .as_ref()
                            .and_then(|r| r.setups.iter().find(|s| s.id == id))
                            .and_then(|s| s.deviation.as_ref())
                            .map(|d| SurfaceSamples::new(&d.points, settings.cell.0)),
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

    /// Actual playback stock with display-only final-result colours and a
    /// matching CPU picker, prepared together off the interface thread.
    pub fn start_stock_surface(
        &self,
        playback: &Arc<StockPlayback>,
        cursor: Cursor,
    ) -> JobHandle<StockSurface> {
        let playback = Arc::clone(playback);
        self.jobs
            .submit("stock at move", self.events_tx.clone(), move |context| {
                let mesh = playback.mesh_at(cursor, context)?;
                let group = playback
                    .timeline
                    .locate(cursor.done)
                    .map(|(g, _)| g)
                    .or_else(|| {
                        playback
                            .timeline
                            .run()
                            .toolpath
                            .groups
                            .iter()
                            .rposition(|g| !g.segments.is_empty())
                    })
                    .unwrap_or(0);
                let setup = playback
                    .setups
                    .iter()
                    .find(|s| s.groups.contains(&group))
                    .ok_or_else(|| {
                        AppError::Playback("no simulation covers this move".to_owned())
                    })?;
                StockSurface::new(mesh, setup.id, setup.model, setup.samples.as_ref(), context)
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
                // The part as the run was made with it (see `Regeneration::project`).
                let model = run
                    .project
                    .setup(setup)
                    .and_then(|s| s.part)
                    .and_then(|id| run.project.models().iter().find(|m| m.id == id))
                    .ok_or_else(|| AppError::Playback("the setup has no part".to_owned()))?;
                let mesh = match run.project.setup(setup) {
                    Some(setup) => {
                        crate::model::setup_mesh(&document.location, model, setup, context)?
                    }
                    None => crate::model::placed_mesh(&document.location, model, context)?,
                };
                let model_triangles = mesh.triangles.len();
                // The deviation was measured against the part with the held
                // regions and tabs; the colours are of the part's triangles,
                // which come first.
                let target = match run.project.setup(setup) {
                    Some(setup) => {
                        crate::workholding::setup_deviation_target(setup, mesh.clone())?.mesh
                    }
                    None => mesh.clone(),
                };
                let options = DeviationOptions::for_cell(report.settings.cell.0);
                let mut values = triangle_values(deviation, &target, options.spacing)?;
                values.truncate(model_triangles);
                Ok((model.id, deviation_bands(&values, report.settings.cell.0)))
            },
        )
    }
}

/// Adds the checkpoints of one more setup (`copies` copies of a stock of
/// `stock_bytes`) to the `kept` bytes of the setups before it, and refuses
/// when the total passes [`MAX_CHECKPOINT_BYTES`]: all the setups' simulators
/// stay in memory together.
fn add_checkpoint_bytes(
    kept: usize,
    copies: usize,
    stock_bytes: usize,
    cell: f64,
) -> Result<usize, AppError> {
    let total = kept.saturating_add(copies.saturating_mul(stock_bytes));
    if total > MAX_CHECKPOINT_BYTES {
        return Err(AppError::Playback(format!(
            "stock playback would keep at least {:.0} MB of checkpoints across the setups \
             ({copies} copies of a stock of {:.0} MB in the setup being added), over the {:.0} MB \
             allowed; make the simulation cell coarser (now {cell} mm) or play the toolpath \
             without the stock",
            total as f64 / 1e6,
            stock_bytes as f64 / 1e6,
            MAX_CHECKPOINT_BYTES as f64 / 1e6,
        )));
    }
    Ok(total)
}

/// The operation a group came from, for a legend; kept here so a front end
/// need not reach into the run.
pub fn group_operation(run: &Regeneration, group: usize) -> Option<OperationId> {
    run.groups.get(group).map(|g| g.operation)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn req_nfr_04_the_checkpoint_limit_counts_every_setup_together() {
        let half = MAX_CHECKPOINT_BYTES / 2 + 1;
        // Each setup alone is within the limit ...
        let first = add_checkpoint_bytes(0, 1, half, 0.1).unwrap();
        assert_eq!(first, half);
        // ... but the second one takes the total over it.
        let error = add_checkpoint_bytes(first, 1, half, 0.1).unwrap_err();
        assert!(error.to_string().contains("across the setups"), "{error}");
        // A total exactly at the limit is allowed, and huge counts do not wrap.
        assert!(add_checkpoint_bytes(0, 1, MAX_CHECKPOINT_BYTES, 0.1).is_ok());
        assert!(add_checkpoint_bytes(0, usize::MAX, 2, 0.1).is_err());
    }
}
