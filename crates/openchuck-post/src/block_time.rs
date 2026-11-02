// SPDX-License-Identifier: AGPL-3.0-or-later
//! GC-17 compatibility Report: a deterministic analysis of original IR durations.
//!
//! This API retains complete-window diagnostic semantics. The production post
//! also returns `ShortBlockReport`, measured from emitted feeds, and uses that
//! controller-time model for optional Slow down. These metrics need not match.
use crate::{PostError, Result};
use openchuck_types::{
    ir::{MoveKind, Toolpath},
    machine::ShortBlockMode,
};

/// Profile settings, before optional post overrides.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BlockTimeSettings {
    /// Minimum window mean in milliseconds; zero disables the check.
    pub min_block_ms: f64,
    /// Number of consecutive fed blocks in a complete averaging window.
    pub block_window: u32,
    /// Off or Report; no feed mutation is implemented here.
    pub mode: ShortBlockMode,
}
impl Default for BlockTimeSettings {
    fn default() -> Self {
        Self {
            min_block_ms: 0.0,
            block_window: 1,
            mode: ShortBlockMode::Off,
        }
    }
}
impl BlockTimeSettings {
    /// Refuse non-finite or negative times and an empty window.
    pub fn validate(self) -> Result<()> {
        if !self.min_block_ms.is_finite() || self.min_block_ms < 0.0 || self.block_window == 0 {
            return Err(PostError::Setting {
                name: "minimum block time/window".into(),
                reason: "time must be finite and nonnegative; window must be at least 1".into(),
            });
        }
        Ok(())
    }
}
/// One operation's report, computed without modifying its path or program.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockTimeReport {
    /// IR operation tag.
    pub operation: String,
    /// Effective settings after post overrides.
    pub settings: BlockTimeSettings,
    /// Unique fed blocks covered by a window whose mean is under the limit.
    pub affected_blocks: usize,
    /// Longest contiguous run of affected fed blocks; rapid moves break a run.
    pub longest_run: usize,
    /// Number of complete windows examined.
    pub windows: usize,
    /// Lowest complete-window mean, milliseconds; zero if there is no complete window.
    pub worst_mean_ms: f64,
    /// Contact distance per minute in the longest affected run; zero without an affected run.
    pub effective_feed: f64,
    /// Estimated added seconds if deficient windows were lengthened, shortest blocks first.
    pub seconds_added: f64,
}
impl BlockTimeReport {
    /// Plain wording for the setup sheet and application warning.
    pub fn summary(&self, source: &str) -> String {
        let mean = if self.windows == 0 {
            "no complete window".to_owned()
        } else {
            format!(
                "worst mean {:.3} ms, effective feed in longest run {:.3} mm/min",
                self.worst_mean_ms, self.effective_feed
            )
        };
        format!(
            "minimum block time {:.3} ms ({source}), window {}, Report: {} affected blocks, longest run {}; {mean}; estimated slow-down adds {:.3} s. G-code unchanged; verify before running.",
            self.settings.min_block_ms,
            self.settings.block_window,
            self.affected_blocks,
            self.longest_run,
            self.seconds_added
        )
    }
}
/// Analyse complete N-block windows, independently for each operation and fed run.
/// Rapids and operation boundaries reset the window. An incomplete window is
/// explicitly reported as unexamined, rather than treated as a per-block check.
/// Durations are the IR times consumed by the post's feed rule (decision 0222).
pub fn block_time_reports(
    path: &Toolpath,
    settings: BlockTimeSettings,
) -> Result<Vec<BlockTimeReport>> {
    settings.validate()?;
    if settings.min_block_ms == 0.0 || settings.mode == ShortBlockMode::Off {
        return Ok(Vec::new());
    }
    path.validate()?;
    let mut reports = Vec::new();
    for group in &path.groups {
        let mut report = BlockTimeReport {
            operation: group.tags.operation.clone(),
            settings,
            affected_blocks: 0,
            longest_run: 0,
            windows: 0,
            worst_mean_ms: 0.0,
            effective_feed: 0.0,
            seconds_added: 0.0,
        };
        let mut run = Vec::new();
        for segment in &group.segments {
            if segment.kind == MoveKind::Rapid {
                analyse_run(&run, &mut report);
                run.clear();
            } else {
                run.push((segment.duration.0, segment.contact_length.0));
            }
        }
        analyse_run(&run, &mut report);
        reports.push(report);
    }
    Ok(reports)
}
fn analyse_run(run: &[(f64, f64)], report: &mut BlockTimeReport) {
    let n = report.settings.block_window as usize;
    if run.len() < n {
        return;
    }
    let limit = report.settings.min_block_ms / 1000.0;
    let mut affected = vec![false; run.len()];
    let mut estimated: Vec<f64> = run.iter().map(|b| b.0).collect();
    for (start, window) in run.windows(n).enumerate() {
        let seconds: f64 = window.iter().map(|b| b.0).sum();
        let mean_ms = seconds * 1000.0 / n as f64;
        if report.windows == 0 || mean_ms < report.worst_mean_ms {
            report.worst_mean_ms = mean_ms;
        }
        report.windows += 1;
        if mean_ms < report.settings.min_block_ms
            && let Some(flags) = affected.get_mut(start..start + n)
        {
            flags.fill(true);
        }
        // Estimate only. No path, duration, feed word or G-code is changed.
        if let Some(times) = estimated.get_mut(start..start + n) {
            let mut deficit = (limit * n as f64 - times.iter().sum::<f64>()).max(0.0);
            if deficit <= 0.0 {
                continue;
            }
            let mut order: Vec<usize> = (0..n).collect();
            order.sort_by(|a, b| {
                times
                    .get(*a)
                    .copied()
                    .unwrap_or(0.0)
                    .total_cmp(&times.get(*b).copied().unwrap_or(0.0))
            });
            for index in order {
                if deficit <= 0.0 {
                    break;
                }
                if let Some(time) = times.get_mut(index) {
                    let add = (limit - *time).max(0.0).min(deficit);
                    *time += add;
                    deficit -= add;
                    report.seconds_added += add;
                }
            }
        }
    }
    let mut length = 0;
    let mut seconds = 0.0;
    let mut contact = 0.0;
    for (index, flag) in affected.into_iter().enumerate() {
        if flag {
            report.affected_blocks += 1;
            length += 1;
            if let Some(block) = run.get(index) {
                seconds += block.0;
                contact += block.1;
            }
            if length > report.longest_run {
                report.longest_run = length;
                report.effective_feed = if seconds > 0.0 {
                    contact * 60.0 / seconds
                } else {
                    0.0
                };
            }
        } else {
            length = 0;
            seconds = 0.0;
            contact = 0.0;
        }
    }
}
