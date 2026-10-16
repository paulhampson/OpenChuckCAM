// SPDX-License-Identifier: AGPL-3.0-or-later
//! The simulator: runs toolpath groups against a dexel stock in machining
//! order, so each operation cuts the stock the ones before it left (SIM-06),
//! keeps checkpoints to jump to any operation or move, and reports per
//! group. Cancellable, with progress, and deterministic at any thread count
//! (slice 7 item 8; NFR-04, NFR-06).

use openchuck_types::geometry::Point3;
use openchuck_types::ir::{PathTags, ToolpathGroup};
use openchuck_types::job::JobContext;
use openchuck_types::kinematics::{AsRotaryFrame, RotaryFrame};
use openchuck_types::tool::RevolvedProfile;
use openchuck_types::{Error, Result};

use crate::dexel::{DexelStock, Pose};
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
    /// The stock at that point.
    pub stock: DexelStock,
}

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
            stock: stock.clone(),
        }];
        Self {
            frame: frame.rotary_frame(),
            stock,
            spacing: cx.min(cy),
            reach,
            checkpoint_every: checkpoint_every.filter(|&n| n > 0),
            keep_checkpoints: true,
            checkpoints,
            groups_done: 0,
        }
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
            self.checkpoints.push(Checkpoint {
                group: index + 1,
                moves: 0,
                stock: self.stock.clone(),
            });
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
        let target = (group, moves);
        let cp = self
            .checkpoints
            .iter()
            .rev()
            .find(|c| (c.group, c.moves) <= target)
            .ok_or_else(|| {
                Error::invalid("checkpoint", "no checkpoint before the requested move")
            })?;
        let mut replay = Self {
            frame: self.frame,
            stock: cp.stock.clone(),
            spacing: self.spacing,
            reach: self.reach,
            checkpoint_every: None,
            keep_checkpoints: false,
            checkpoints: Vec::new(),
            groups_done: cp.group,
        };
        let mut g = cp.group;
        let mut skip = cp.moves;
        while g <= group {
            let (path, profile) = groups.get(g).ok_or_else(|| {
                Error::invalid("group", format!("group {g} has not been simulated"))
            })?;
            let end = if g == group {
                moves.min(path.segments.len())
            } else {
                path.segments.len()
            };
            replay.run_range(path, profile, skip, end, false, context)?;
            skip = 0;
            g += 1;
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
            let done = i + 1;
            let at_checkpoint =
                keep && done < end && self.checkpoint_every.is_some_and(|n| done % n == 0);
            if batch.len() >= BATCH_POSES || at_checkpoint {
                self.flush(&tool, &mut batch, &mut result, context, total)?;
            }
            if at_checkpoint {
                self.checkpoints.push(Checkpoint {
                    group: self.groups_done,
                    moves: done,
                    stock: self.stock.clone(),
                });
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
