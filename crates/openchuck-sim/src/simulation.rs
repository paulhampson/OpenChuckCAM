// SPDX-License-Identifier: AGPL-3.0-or-later
//! The simulator: runs toolpath groups against a dexel stock in machining
//! order, so each operation cuts the stock the ones before it left (SIM-06),
//! keeps checkpoints to jump to any operation or move, and reports per
//! group. Cancellable, with progress, and deterministic at any thread count
//! (slice 7 item 8; NFR-04, NFR-06).

use std::sync::Arc;

use openchuck_types::geometry::Point3;
use openchuck_types::ir::{PathTags, ToolpathGroup};
use openchuck_types::job::{JobContext, Progress, ProgressSink};
use openchuck_types::kinematics::{AsRotaryFrame, RotaryFrame};
use openchuck_types::tool::RevolvedProfile;
use openchuck_types::{Error, Result};

use crate::dexel::{DexelStock, PackedStock, Pose};
use crate::sample::{Sampling, must_not_cut, sample_move};
use crate::span::ToolSolid;

/// Poses cut in one batch between progress reports. Fixed, so batching never
/// changes the result (it would not anyway: span endpoints depend only on the
/// pose and the column). The cancel flag is also watched inside the cut
/// ([`DexelStock::cut_cancellable`]), as one batch can take over a second.
pub const BATCH_POSES: usize = 32_768;

/// Removal below this, in cubic millimetres, is rounding, not a cut.
const VOLUME_EPSILON: f64 = 1e-9;

/// A rapid move that removed material: a fault in the toolpath.
#[derive(Debug, Clone, PartialEq)]
pub struct RapidCut {
    /// Index of the move in its group.
    pub segment: usize,
    /// Volume removed, in cubic millimetres.
    pub volume: f64,
    /// Where the move ends, in the model frame.
    pub end: Point3,
}

/// What simulating one group did.
#[derive(Debug, Clone, PartialEq)]
pub struct GroupResult {
    /// The group's operation, setup and tool.
    pub tags: PathTags,
    /// Moves simulated.
    pub moves: usize,
    /// Poses the moves were sampled into.
    pub poses: usize,
    /// Volume removed, in cubic millimetres.
    pub removed_volume: f64,
    /// Material volume left afterwards.
    pub volume_after: f64,
    /// Rapid moves that removed material.
    pub rapid_cuts: Vec<RapidCut>,
}

/// The stock after a number of moves of a group.
#[derive(Debug, Clone, PartialEq)]
pub struct Checkpoint {
    /// The group, counted from the first simulated.
    pub group: usize,
    /// Moves of that group done.
    pub moves: usize,
    /// The stock at that point, packed ([`DexelStock::pack`]); shared, so a
    /// copy of the simulator or of its checkpoints holds the same bytes
    /// (decision record 0333).
    pub stock: Arc<PackedStock>,
}

/// Moves between two progress reports within a batch of poses.
const PROGRESS_EVERY: usize = 256;

/// Runs toolpath groups against a stock.
#[derive(Debug, Clone)]
pub struct Simulator {
    frame: RotaryFrame,
    stock: DexelStock,
    spacing: f64,
    reach: f64,
    checkpoint_every: Option<usize>,
    keep_checkpoints: bool,
    checkpoints: Vec<Checkpoint>,
    /// The most memory the checkpoints may hold, if limited.
    checkpoint_budget: Option<usize>,
    /// The memory the checkpoints hold.
    checkpoint_bytes: usize,
    groups_done: usize,
}

impl Simulator {
    /// A simulator for `stock`, on the machine kinematics of `frame` (the
    /// controller interpolates in machine coordinates). Moves are sampled no
    /// further apart than the stock's cell. A checkpoint is kept at the start,
    /// after each group, and, with `checkpoint_every`, every that many moves
    /// within a group (each holds a copy of the stock).
    pub fn new(
        stock: DexelStock,
        frame: &impl AsRotaryFrame,
        checkpoint_every: Option<usize>,
    ) -> Self {
        let (cx, cy) = stock.cell();
        let reach = stock.reach();
        let checkpoints = vec![Checkpoint {
            group: 0,
            moves: 0,
            stock: Arc::new(stock.pack()),
        }];
        let checkpoint_bytes = checkpoints.iter().map(|c| c.stock.memory_bytes()).sum();
        Self {
            frame: frame.rotary_frame(),
            stock,
            spacing: cx.min(cy),
            reach,
            checkpoint_every: checkpoint_every.filter(|&n| n > 0),
            keep_checkpoints: true,
            checkpoints,
            checkpoint_budget: None,
            checkpoint_bytes,
            groups_done: 0,
        }
    }

    /// Limits the memory the checkpoints hold to `bytes`, each copy counted
    /// at its own size (a stock grows as it is cut). A checkpoint that would
    /// pass the limit is not kept; the one at the start always is. Fewer
    /// checkpoints never change a stock, only how far a jump replays.
    #[must_use]
    pub fn with_checkpoint_budget(mut self, bytes: usize) -> Self {
        self.checkpoint_budget = Some(bytes);
        self
    }

    /// As [`Simulator::with_checkpoint_budget`], on a simulator already in
    /// use: a host that shares one budget among several simulators gives
    /// each what the others have left (`None` for no limit).
    pub fn set_checkpoint_budget(&mut self, bytes: Option<usize>) {
        self.checkpoint_budget = bytes;
    }

    /// The memory the checkpoints hold.
    pub fn checkpoint_bytes(&self) -> usize {
        self.checkpoint_bytes
    }

    /// Keeps a checkpoint of the stock now, after `moves` moves of group
    /// `group`, unless it would pass the budget.
    fn keep_checkpoint(&mut self, group: usize, moves: usize) {
        let stock = self.stock.pack();
        let bytes = self.checkpoint_bytes.saturating_add(stock.memory_bytes());
        if self.checkpoint_budget.is_some_and(|budget| bytes > budget) {
            return;
        }
        self.checkpoint_bytes = bytes;
        self.checkpoints.push(Checkpoint {
            group,
            moves,
            stock: Arc::new(stock),
        });
    }

    /// Keeps a checkpoint every `every` moves within the groups run from now
    /// on (`None` for none inside them), as [`Simulator::new`]'s
    /// `checkpoint_every`: a host that learns each group's length only as it
    /// comes spaces each one to its share of the budget (decision record
    /// 0333). Does nothing on a simulator without checkpoints.
    pub fn set_checkpoint_every(&mut self, every: Option<usize>) {
        self.checkpoint_every = every.filter(|&n| n > 0);
    }

    /// A simulator that replays from `checkpoints`, kept by another one over
    /// `groups_done` groups on the machine kinematics of `frame` (playback
    /// from the pipeline's own simulation, decision record 0333), so that
    /// [`Simulator::stock_at`] gives the stock at any move of those groups.
    /// `None` unless the first checkpoint is at the start, where every
    /// replay can begin. Its stock is the last checkpoint's; it is meant to
    /// replay, not to run more groups.
    pub fn from_checkpoints(
        frame: &impl AsRotaryFrame,
        checkpoints: Vec<Checkpoint>,
        groups_done: usize,
    ) -> Option<Self> {
        let first = checkpoints.first()?;
        if (first.group, first.moves) != (0, 0) {
            return None;
        }
        let stock = checkpoints.last()?.stock.unpack();
        let (cx, cy) = stock.cell();
        let reach = stock.reach();
        let checkpoint_bytes = checkpoints.iter().map(|c| c.stock.memory_bytes()).sum();
        Some(Self {
            frame: frame.rotary_frame(),
            stock,
            spacing: cx.min(cy),
            reach,
            checkpoint_every: None,
            keep_checkpoints: false,
            checkpoints,
            checkpoint_budget: None,
            checkpoint_bytes,
            groups_done,
        })
    }

    /// A simulator that keeps no checkpoints, for a host that only needs the
    /// stock at the end (each checkpoint holds a copy of the stock).
    /// [`Simulator::stock_at`] then fails.
    pub fn without_checkpoints(stock: DexelStock, frame: &impl AsRotaryFrame) -> Self {
        let (cx, cy) = stock.cell();
        let reach = stock.reach();
        Self {
            frame: frame.rotary_frame(),
            stock,
            spacing: cx.min(cy),
            reach,
            checkpoint_every: None,
            keep_checkpoints: false,
            checkpoints: Vec::new(),
            checkpoint_budget: None,
            checkpoint_bytes: 0,
            groups_done: 0,
        }
    }

    /// The stock, giving up the simulator.
    pub fn into_stock(self) -> DexelStock {
        self.stock
    }

    /// The stock now.
    pub fn stock(&self) -> &DexelStock {
        &self.stock
    }

    /// The checkpoints kept, in order.
    pub fn checkpoints(&self) -> &[Checkpoint] {
        &self.checkpoints
    }

    /// Groups simulated so far.
    pub fn groups_done(&self) -> usize {
        self.groups_done
    }

    /// Simulates `group`, cut by the tool of `profile`, on the current stock.
    pub fn run_group(
        &mut self,
        group: &ToolpathGroup,
        profile: &RevolvedProfile,
        context: &JobContext<'_>,
    ) -> Result<GroupResult> {
        let index = self.groups_done;
        let keep = self.keep_checkpoints;
        let result = self.run_range(group, profile, 0, group.segments.len(), keep, context)?;
        self.groups_done += 1;
        if keep {
            self.keep_checkpoint(index + 1, 0);
        }
        Ok(result)
    }

    /// The stock after `moves` moves of group `group` (counted from the first
    /// simulated), replayed from the nearest checkpoint before it. `groups`
    /// are the groups simulated so far, with their tools, in order.
    pub fn stock_at(
        &self,
        group: usize,
        moves: usize,
        groups: &[(&ToolpathGroup, &RevolvedProfile)],
        context: &JobContext<'_>,
    ) -> Result<DexelStock> {
        self.stock_at_from(None, group, moves, groups, context)
    }

    /// As [`Simulator::stock_at`], also replaying from `kept`, a checkpoint
    /// the caller holds (a stock this simulator gave for an earlier point,
    /// such as the last one a playback showed) when it is the nearest one
    /// before the point. Replaying from any checkpoint gives the same stock
    /// as running up to the point. The stock at a checkpoint, or at the end
    /// of a group (the checkpoint after it), is a copy of it.
    pub fn stock_at_from(
        &self,
        kept: Option<&Checkpoint>,
        group: usize,
        moves: usize,
        groups: &[(&ToolpathGroup, &RevolvedProfile)],
        context: &JobContext<'_>,
    ) -> Result<DexelStock> {
        // The end of a group is the start of the next one.
        let ended = groups
            .get(group)
            .is_some_and(|(path, _)| moves >= path.segments.len());
        let target = if ended && self.groups_done > group {
            (group + 1, 0)
        } else {
            (group, moves)
        };
        let cp = self
            .checkpoints
            .iter()
            .rev()
            .find(|c| (c.group, c.moves) <= target)
            .into_iter()
            .chain(kept.filter(|c| (c.group, c.moves) <= target))
            .max_by_key(|c| (c.group, c.moves))
            .ok_or_else(|| {
                Error::invalid("checkpoint", "no checkpoint before the requested move")
            })?;
        if (cp.group, cp.moves) == target {
            return Ok(cp.stock.unpack());
        }
        let mut replay = Self {
            frame: self.frame,
            stock: cp.stock.unpack(),
            spacing: self.spacing,
            reach: self.reach,
            checkpoint_every: None,
            keep_checkpoints: false,
            checkpoints: Vec::new(),
            checkpoint_budget: None,
            checkpoint_bytes: 0,
            groups_done: cp.group,
        };
        // The ranges to replay, first to last, so that progress is one total.
        let mut ranges = Vec::new();
        let mut skip = cp.moves;
        for g in cp.group..=group {
            let (path, profile) = groups.get(g).ok_or_else(|| {
                Error::invalid("group", format!("group {g} has not been simulated"))
            })?;
            let end = if g == group {
                moves.min(path.segments.len())
            } else {
                path.segments.len()
            };
            ranges.push((*path, *profile, skip, end));
            skip = 0;
        }
        let total = ranges
            .iter()
            .map(|(_, _, first, end)| end.saturating_sub(*first) as u64)
            .sum();
        let mut before = 0u64;
        for (path, profile, first, end) in ranges {
            let within = Within {
                sink: context.progress,
                before,
                total,
            };
            let part = JobContext::new(context.cancel, &within);
            replay.run_range(path, profile, first, end, false, &part)?;
            before += end.saturating_sub(first) as u64;
        }
        Ok(replay.stock)
    }

    /// Simulates moves `first..end` of `group`, starting from the pose
    /// before move `first`.
    fn run_range(
        &mut self,
        group: &ToolpathGroup,
        profile: &RevolvedProfile,
        first: usize,
        end: usize,
        keep: bool,
        context: &JobContext<'_>,
    ) -> Result<GroupResult> {
        group.start.tip.validate("group.start.tip")?;
        if !group.start.a.0.is_finite() {
            return Err(Error::invalid("group.start.a", "A is not a finite number"));
        }
        let tool = ToolSolid::new(profile);
        let sampling = Sampling {
            spacing: self.spacing,
            reach: self.reach,
            tool_extent: tool.radius().hypot(tool.top()),
        };
        let total = end.saturating_sub(first) as u64;
        let mut result = GroupResult {
            tags: group.tags.clone(),
            moves: 0,
            poses: 0,
            removed_volume: 0.0,
            volume_after: 0.0,
            rapid_cuts: Vec::new(),
        };
        let mut prev = first
            .checked_sub(1)
            .and_then(|i| group.segments.get(i))
            .map_or(group.start, |s| s.end);
        let mut batch: Vec<Pose> = Vec::new();
        for (i, segment) in group.segments.iter().enumerate().take(end).skip(first) {
            segment.validate(&format!("segments[{i}]"))?;
            if must_not_cut(segment.kind) {
                self.flush(&tool, &mut batch, &mut result, context, total)?;
                sample_move(&self.frame, &prev, &segment.end, &sampling, &mut batch);
                let removed = self.cut_batch(&tool, &mut batch, &mut result, context)?;
                if removed > VOLUME_EPSILON {
                    result.rapid_cuts.push(RapidCut {
                        segment: i,
                        volume: removed,
                        end: segment.end.tip,
                    });
                }
            } else {
                sample_move(&self.frame, &prev, &segment.end, &sampling, &mut batch);
            }
            result.moves += 1;
            prev = segment.end;
            // Progress between batches too, for a bar that fills smoothly
            // (decision record 0322); the stock is cut only at a flush.
            if result.moves.is_multiple_of(PROGRESS_EVERY) {
                context.report("simulate", result.moves as u64, Some(total));
            }
            let done = i + 1;
            let at_checkpoint =
                keep && done < end && self.checkpoint_every.is_some_and(|n| done % n == 0);
            if batch.len() >= BATCH_POSES || at_checkpoint {
                self.flush(&tool, &mut batch, &mut result, context, total)?;
            }
            if at_checkpoint {
                self.keep_checkpoint(self.groups_done, done);
            }
        }
        self.flush(&tool, &mut batch, &mut result, context, total)?;
        result.volume_after = self.stock.volume();
        Ok(result)
    }

    fn cut_batch(
        &mut self,
        tool: &ToolSolid,
        batch: &mut Vec<Pose>,
        result: &mut GroupResult,
        context: &JobContext<'_>,
    ) -> Result<f64> {
        let removed = self.stock.cut_cancellable(tool, batch, context.cancel)?;
        result.poses += batch.len();
        result.removed_volume += removed;
        batch.clear();
        Ok(removed)
    }

    fn flush(
        &mut self,
        tool: &ToolSolid,
        batch: &mut Vec<Pose>,
        result: &mut GroupResult,
        context: &JobContext<'_>,
        total: u64,
    ) -> Result<()> {
        context.check_cancelled()?;
        if !batch.is_empty() {
            self.cut_batch(tool, batch, result, context)?;
        }
        context.report("simulate", result.moves as u64, Some(total));
        Ok(())
    }
}

/// The progress of one range of a longer replay, reported as a share of the
/// whole: `before` moves were replayed before it, of `total`.
struct Within<'a> {
    sink: &'a dyn ProgressSink,
    before: u64,
    total: u64,
}

impl ProgressSink for Within<'_> {
    fn report(&self, progress: Progress) {
        self.sink.report(Progress {
            stage: progress.stage,
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
