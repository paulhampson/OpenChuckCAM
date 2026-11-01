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

use std::sync::{Arc, Mutex, PoisonError};

use openchuck_project::Project;
use openchuck_project::id::{ModelId, OperationId, SetupId};
use openchuck_sim::{
    Checkpoint, DeviationOptions, DexelStock, Simulator, stock_mesh, triangle_values,
};
use openchuck_types::job::{JobContext, Progress, ProgressSink};
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::tool::RevolvedProfile;

use crate::pathview::{DeviationBands, deviation_bands};
use crate::playback::{Cursor, Timeline};
pub use crate::stock_surface::StockSurface;
use crate::stock_surface::SurfaceSamples;
use crate::{AppError, Application, JobHandle, Regeneration};

/// The memory playback's checkpoints may hold by default, in bytes: the
/// user's setting "Memory for playback checkpoints" changes it (decision
/// record 0322). A project whose setups could not keep the checkpoints they
/// must is refused with a hint to raise it or to coarsen the simulation cell.
pub const DEFAULT_PLAYBACK_MEMORY: usize = 1_536 * 1024 * 1024;

/// The fewest moves between two checkpoints inside an operation.
pub(crate) const MIN_CHECKPOINT_SPACING: usize = 1_000;

struct SetupPlay {
    id: SetupId,
    /// Its checkpoints are the run's own (decision record 0333).
    from_run: bool,
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
    /// The stock after the whole moves of the last cursor asked for, with
    /// the index of its setup: a checkpoint for the next request, so that
    /// playing forward replays only the moves since the last frame.
    kept: Mutex<Option<Arc<(usize, Checkpoint)>>>,
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
        let index = self
            .setups
            .iter()
            .position(|s| s.groups.contains(&group))
            .ok_or_else(|| AppError::Playback("no simulation covers this move".to_owned()))?;
        let setup = self
            .setups
            .get(index)
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
        // Replaying from any checkpoint gives the same stock (decision 0025
        // item 4), so the last request's stock is used when it is nearer.
        let kept = self
            .kept
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
            .filter(|k| k.0 == index);
        let stock = setup.simulator.stock_at_from(
            kept.as_deref().map(|k| &k.1),
            local,
            moves,
            &groups,
            context,
        )?;
        drop(kept);
        *self.kept.lock().unwrap_or_else(PoisonError::into_inner) = Some(Arc::new((
            index,
            Checkpoint {
                group: local,
                moves,
                stock: Arc::new(stock.pack()),
            },
        )));
        let Some(original) = path.groups.get(group) else {
            return Ok(stock);
        };
        let Some(segment) = original.segments.get(moves) else {
            return Ok(stock);
        };
        if cursor.fraction <= 0.0 {
            return Ok(stock);
        }
        let pose = timeline.sample(cursor)?;
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
            rotary_reset_after: Vec::new(),
            tags: original.tags.clone(),
            start,
            segments: vec![partial],
        };
        let frame = timeline
            .work_frame(group)
            .ok_or_else(|| AppError::Playback("no frame covers this move".to_owned()))?;
        let mut simulator = Simulator::without_checkpoints(stock, &frame);
        let info = infos
            .get(group)
            .ok_or_else(|| AppError::Playback("no tool covers this move".to_owned()))?;
        simulator.run_group(&partial, &info.tool, context)?;
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

    /// The memory the checkpoints hold in all, in bytes.
    pub fn checkpoint_bytes(&self) -> usize {
        self.setups
            .iter()
            .map(|s| s.simulator.checkpoint_bytes())
            .sum()
    }

    /// True when every setup's checkpoints are those the run kept while it
    /// simulated ("Generate playback information with toolpath generation",
    /// decision record 0333), so nothing was simulated again.
    pub fn from_the_run(&self) -> bool {
        !self.setups.is_empty() && self.setups.iter().all(|s| s.from_run)
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
    /// simulation keeps none (decision 0025 item 7). The checkpoints and the
    /// copies a replay holds never pass `memory` bytes (the user's setting;
    /// [`DEFAULT_PLAYBACK_MEMORY`] by default): they are spaced evenly to
    /// fill it, never closer than 1,000 moves, and one that would pass it is
    /// not kept (decision record 0322). Fails, naming the setting and the
    /// cell, when the copies at the start and after each operation would not
    /// fit.
    pub fn start_stock_playback(
        &self,
        timeline: Arc<Timeline>,
        memory: usize,
    ) -> JobHandle<StockPlayback> {
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
                // Each setup's stock, and the copies that must be kept: the
                // start and after each operation, packed as checkpoints are,
                // and the replay's (the stock kept from the last request,
                // packed, and the one being replayed, unpacked).
                let mut stocks = Vec::new();
                let mut kept_bytes = 0usize;
                let (mut packed, mut working) = (0usize, 0usize);
                for (_, groups) in &plan {
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
                    let bytes = stock.pack().memory_bytes();
                    packed = packed.max(bytes);
                    working = working.max(stock.memory_bytes());
                    kept_bytes = add_checkpoint_bytes(
                        kept_bytes,
                        groups.len() + 1,
                        bytes,
                        settings.cell.0,
                        memory,
                    )?;
                    stocks.push(stock);
                }
                let replay = packed.saturating_add(working);
                let kept_bytes =
                    add_checkpoint_bytes(kept_bytes, 1, replay, settings.cell.0, memory)?;
                let moves: usize = run.toolpath.groups.iter().map(|g| g.segments.len()).sum();
                // The copies inside share what is left after those kept.
                let every = checkpoint_spacing(memory.saturating_sub(kept_bytes), packed, 0, moves);
                // Progress is the moves simulated, of every group's.
                let total = moves as u64;
                let mut done = 0u64;
                context.report("prepare playback", 0, Some(total));
                let mut setups = Vec::new();
                // Every setup's simulator is kept, so the budget is shared:
                // each setup may use what the ones before it left.
                let mut held = replay;
                for ((id, groups), stock) in plan.into_iter().zip(stocks) {
                    // The run's own checkpoints, when it kept them while
                    // simulating every operation of this setup (decision
                    // record 0333): nothing is simulated again.
                    let reused = run
                        .simulation
                        .as_ref()
                        .and_then(|r| r.setups.iter().find(|s| s.id == id))
                        .filter(|s| s.operations.len() == groups.len())
                        .and_then(|s| {
                            Simulator::from_checkpoints(
                                machine,
                                s.checkpoints.clone(),
                                groups.len(),
                            )
                        });
                    let from_run = reused.is_some();
                    let mut simulator = match reused {
                        Some(simulator) => {
                            done += groups
                                .iter()
                                .filter_map(|g| run.toolpath.groups.get(*g))
                                .map(|g| g.segments.len() as u64)
                                .sum::<u64>();
                            context.report("prepare playback", done, Some(total));
                            simulator
                        }
                        None => Simulator::new(stock, machine, every)
                            .with_checkpoint_budget(memory.saturating_sub(held)),
                    };
                    for g in groups.iter().filter(|_| !from_run) {
                        context.check_cancelled()?;
                        let (Some(path), Some(info)) =
                            (run.toolpath.groups.get(*g), run.groups.get(*g))
                        else {
                            continue;
                        };
                        let within = Within {
                            sink: context.progress,
                            before: done,
                            total,
                        };
                        simulator.run_group(
                            path,
                            &info.tool,
                            &JobContext::new(context.cancel, &within),
                        )?;
                        done += path.segments.len() as u64;
                        context.report("prepare playback", done, Some(total));
                    }
                    held = held.saturating_add(simulator.checkpoint_bytes());
                    setups.push(SetupPlay {
                        id,
                        from_run,
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
                Ok(StockPlayback {
                    timeline,
                    setups,
                    kept: Mutex::new(None),
                })
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
        self.stock_surface_job(playback, cursor, true)
    }

    /// The same surface without its picker, for a frame shown while playback
    /// plays: it is ready sooner, and pointing at it finds nothing (the
    /// owner's decision D5 on issue #382; decision record 0322).
    pub fn start_playing_stock_surface(
        &self,
        playback: &Arc<StockPlayback>,
        cursor: Cursor,
    ) -> JobHandle<StockSurface> {
        self.stock_surface_job(playback, cursor, false)
    }

    fn stock_surface_job(
        &self,
        playback: &Arc<StockPlayback>,
        cursor: Cursor,
        picker: bool,
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
                StockSurface::new(
                    mesh,
                    setup.id,
                    setup.model,
                    setup.samples.as_ref(),
                    picker,
                    context,
                )
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
                let setup_ref = run
                    .project
                    .setup(setup)
                    .ok_or_else(|| AppError::Playback("the setup has no part".to_owned()))?;
                let model = setup_ref
                    .part
                    .and_then(|id| run.project.models().iter().find(|m| m.id == id))
                    .ok_or_else(|| AppError::Playback("the setup has no part".to_owned()))?;
                let part = crate::model::setup_part(&document.location, model, setup_ref, context)?;
                // The file is read now, not as the run read it: colours made
                // for another file's triangles would be wrong, so a file
                // edited behind the application is refused (decision 0520).
                if part.hash != model.hash {
                    return Err(AppError::PartChanged(model.label()));
                }
                let mesh = part.mesh;
                let model_triangles = mesh.triangles.len();
                // The deviation was measured against the part with the held
                // regions and tabs; the colours are of the part's triangles,
                // which come first.
                let target =
                    crate::workholding::setup_deviation_target(setup_ref, mesh.clone())?.mesh;
                let options = DeviationOptions::for_cell(report.settings.cell.0);
                let mut values = triangle_values(deviation, &target, options.spacing)?;
                values.truncate(model_triangles);
                Ok((model.id, deviation_bands(&values, report.settings.cell.0)))
            },
        )
    }
}

/// Whether the part of `setup` is no longer the one `run` was made with
/// (decision 0520). The deviation colours the simulation measured belong to
/// that part's triangles, so a part that changed must not carry them.
///
/// The part has changed when the setup is gone from either project, when
/// the setup names another part or places it differently on the machine
/// (its frame), or when the part's model differs in its content hash, its
/// unit or its placement steps. The setup's other settings (stock, material,
/// clearance, holding) change the toolpath, not the part: the run is then out
/// of date and keeps its colours (owner's answer to question 2, case 1).
/// Where the file is stored and the name it is shown by are labels. The model
/// file on disk is checked by [`Application::start_deviation_bands`], which
/// reads it.
#[must_use]
pub fn part_changed_since(project: &Project, run: &Regeneration, setup: SetupId) -> bool {
    let (Some(then), Some(now)) = (run.project.setup(setup), project.setup(setup)) else {
        return true;
    };
    if then.part != now.part || then.frame != now.frame {
        return true;
    }
    let Some(id) = then.part else {
        return true;
    };
    let (Some(then), Some(now)) = (
        run.project.models().iter().find(|m| m.id == id),
        project.models().iter().find(|m| m.id == id),
    ) else {
        return true;
    };
    then.hash != now.hash || then.units != now.units || then.transform != now.transform
}

/// The progress of one group of "prepare playback", reported as moves of
/// every group's: `before` moves were simulated before it, of `total`.
struct Within<'a> {
    sink: &'a dyn ProgressSink,
    before: u64,
    total: u64,
}

impl ProgressSink for Within<'_> {
    fn report(&self, progress: Progress) {
        self.sink.report(Progress {
            stage: "prepare playback".to_owned(),
            done: self.before.saturating_add(progress.done).min(self.total),
            total: Some(self.total),
        });
    }

    fn wants_timings(&self) -> bool {
        self.sink.wants_timings()
    }

    fn timing(&self, stage: &str, elapsed: std::time::Duration) {
        self.sink.timing(stage, elapsed);
    }
}

/// Adds `copies` copies of a stock of `stock_bytes` (the checkpoints a setup
/// must keep, or the replay's) to the `kept` bytes counted before, and
/// refuses when the total passes `memory`: all the setups' simulators stay
/// in memory together.
fn add_checkpoint_bytes(
    kept: usize,
    copies: usize,
    stock_bytes: usize,
    cell: f64,
    memory: usize,
) -> Result<usize, AppError> {
    let total = kept.saturating_add(copies.saturating_mul(stock_bytes));
    if total > memory {
        return Err(AppError::Playback(format!(
            "stock playback would keep at least {:.0} MB of copies of the stock across the \
             setups (a stock of {:.0} MB), over the {:.0} MB of \"Memory for playback \
             checkpoints\" in Settings; raise that, make the simulation cell coarser (now \
             {cell} mm) or play the toolpath without the stock",
            total as f64 / 1e6,
            stock_bytes as f64 / 1e6,
            memory as f64 / 1e6,
        )));
    }
    Ok(total)
}

/// The moves between two checkpoints inside the operations, so that copies
/// of a stock of `stock_bytes` fill `memory` less the `kept` copies that must
/// be held anyway, over `moves` moves in all; never closer than
/// [`MIN_CHECKPOINT_SPACING`]. `None` when no copy fits.
fn checkpoint_spacing(
    memory: usize,
    stock_bytes: usize,
    kept: usize,
    moves: usize,
) -> Option<usize> {
    if stock_bytes == 0 {
        return Some(MIN_CHECKPOINT_SPACING);
    }
    let copies = memory.checked_sub(kept.saturating_mul(stock_bytes))? / stock_bytes;
    if copies == 0 {
        return None;
    }
    Some(moves.div_ceil(copies + 1).max(MIN_CHECKPOINT_SPACING))
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

    const MIB: usize = 1024 * 1024;

    #[test]
    fn req_nfr_04_the_checkpoint_limit_counts_every_setup_together() {
        let budget = 1_536 * MIB;
        let half = budget / 2 + 1;
        // Each setup alone is within the limit ...
        let first = add_checkpoint_bytes(0, 1, half, 0.1, budget).unwrap();
        assert_eq!(first, half);
        // ... but the second one takes the total over it, and the refusal
        // names the setting and the cell.
        let error = add_checkpoint_bytes(first, 1, half, 0.1, budget).unwrap_err();
        let text = error.to_string();
        assert!(text.contains("across the setups"), "{text}");
        assert!(text.contains("Memory for playback checkpoints"), "{text}");
        assert!(text.contains("0.1 mm"), "{text}");
        // A total exactly at the limit is allowed, and huge counts do not wrap.
        assert!(add_checkpoint_bytes(0, 1, budget, 0.1, budget).is_ok());
        assert!(add_checkpoint_bytes(0, usize::MAX, 2, 0.1, budget).is_err());
    }

    #[test]
    fn req_sim_04_req_nfr_07_checkpoints_are_spaced_to_fill_the_memory_budget() {
        // The owner's carving (CARVEPERF's measurement): 290,548 moves, a
        // stock of 7 MB, 3 copies a setup must keep and 2 for the replay.
        let (moves, stock, kept) = (290_548, 7_000_000, 5);
        let at = |mb: usize| checkpoint_spacing(mb * MIB, stock, kept, moves).unwrap();
        // About 225 copies in 1,536 MB, so about 1,300 moves apart; about 70
        // in 512 MB, about 4,000 apart; the floor of 1,000 moves in 8 GB.
        assert!((1_200..=1_400).contains(&at(1_536)), "{}", at(1_536));
        assert!((3_700..=4_300).contains(&at(512)), "{}", at(512));
        assert_eq!(at(8_192), MIN_CHECKPOINT_SPACING);
        // More memory never spaces them further apart.
        let mut last = usize::MAX;
        for mb in (256..=8_192).step_by(256) {
            assert!(at(mb) <= last, "{mb} MB");
            last = at(mb);
        }
        // The copies inside fit what is left after those that must be kept.
        let every = at(1_536);
        assert!((moves / every + kept) * stock <= 1_536 * MIB);
        // A short job keeps one every 1,000 moves, as before.
        assert_eq!(
            checkpoint_spacing(1_536 * MIB, stock, kept, 2_000),
            Some(MIN_CHECKPOINT_SPACING)
        );
        // No room for any inside: only those that must be kept.
        assert_eq!(checkpoint_spacing(kept * stock, stock, kept, moves), None);
        assert_eq!(checkpoint_spacing(0, stock, kept, moves), None);
        assert_eq!(
            checkpoint_spacing(1_536 * MIB, 0, kept, moves),
            Some(MIN_CHECKPOINT_SPACING)
        );
    }
}
