// SPDX-License-Identifier: AGPL-3.0-or-later
//! Controller command-rate management (GC-17; decision 0222).

use crate::{PostError, Result};
use openchuck_types::machine::ShortBlockMode;
use serde::{Deserialize, Serialize};

/// Effective controller throughput settings. Zero milliseconds disables the check.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ShortBlockSettings {
    /// Minimum average fed-block duration, milliseconds.
    pub min_block_ms: f64,
    /// Sliding window size, 1 to 4096 blocks. Short runs use their own length.
    pub block_window: usize,
    /// Report only or lengthen durations while retaining geometry.
    pub mode: ShortBlockMode,
}
impl Default for ShortBlockSettings {
    fn default() -> Self {
        Self {
            min_block_ms: 0.0,
            block_window: 1,
            mode: ShortBlockMode::Off,
        }
    }
}
impl ShortBlockSettings {
    /// Refuses nonfinite/negative durations and unbounded window allocations.
    pub fn validate(self) -> Result<()> {
        if !self.min_block_ms.is_finite() || self.min_block_ms < 0.0 {
            return Err(PostError::Setting {
                name: "min_block_ms".into(),
                reason: "must be finite and at least 0".into(),
            });
        }
        if !(1..=4096).contains(&self.block_window) {
            return Err(PostError::Setting {
                name: "block_window".into(),
                reason: "must be from 1 to 4096".into(),
            });
        }
        if !(self.min_block_ms * self.block_window as f64).is_finite() {
            return Err(PostError::Setting {
                name: "min_block_ms".into(),
                reason: "window duration is too large".into(),
            });
        }
        Ok(())
    }
    pub(crate) fn enabled(self) -> bool {
        self.min_block_ms > 0.0 && self.mode != ShortBlockMode::Off
    }
}
/// Where an effective command-rate setting came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SettingSource {
    /// The project's machine copy.
    Profile,
    /// An explicit post option.
    Post,
}
impl SettingSource {
    /// Human-readable source label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Profile => "machine profile",
            Self::Post => "post override",
        }
    }
}
/// Sources of the three effective settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShortBlockSources {
    /// Source of the minimum duration.
    pub min_block_ms: SettingSource,
    /// Source of the window size.
    pub block_window: SettingSource,
    /// Source of the selected action.
    pub mode: SettingSource,
}
impl Default for ShortBlockSources {
    fn default() -> Self {
        Self {
            min_block_ms: SettingSource::Profile,
            block_window: SettingSource::Profile,
            mode: SettingSource::Profile,
        }
    }
}
/// Original command demand and the predicted or applied mitigation, per group.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShortBlockReport {
    /// Stable IR operation tag, resolved to a display name by the host.
    pub operation: String,
    /// The effective limits and action.
    pub settings: ShortBlockSettings,
    /// Per-field provenance (profile versus post override).
    pub sources: ShortBlockSources,
    /// Individual fed blocks shorter than the target, including buffered bursts.
    pub short_moves: usize,
    /// Longest consecutive run of individual short fed blocks.
    pub longest_run: usize,
    /// Sliding windows below the target before mitigation.
    pub affected_windows: usize,
    /// Lowest original window average, milliseconds (zero for no fed moves).
    pub worst_window_ms: f64,
    /// Contact-path distance divided by time in the worst window, mm/min.
    pub effective_feed: f64,
    /// Additional seconds, predicted in Report and applied in Slow down.
    pub added_seconds: f64,
    /// Fed blocks whose feed would be or was lowered.
    pub slowed_moves: usize,
}

/// Raise the shortest durations first, sharing increases among equally short
/// blocks. Only increases are made, so overlapping windows already fixed stay
/// fixed. Ordering ties by input position makes output independent of threads.
pub(crate) fn lengthen(times: &mut [f64], settings: ShortBlockSettings) {
    if times.is_empty() {
        return;
    }
    let n = settings.block_window.min(times.len());
    let target = settings.min_block_ms / 1000.0 * n as f64;
    for start in 0..=times.len() - n {
        let Some(window) = times.get_mut(start..start + n) else {
            continue;
        };
        let mut deficit = target - window.iter().sum::<f64>();
        if deficit <= 0.0 {
            continue;
        }
        let mut order: Vec<(usize, f64)> = window.iter().copied().enumerate().collect();
        order.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
        for k in 1..=n {
            let Some((_, current)) = order.get(k - 1) else {
                continue;
            };
            let next = order.get(k).map(|(_, value)| *value);
            let cost = next.map_or(f64::INFINITY, |next| (next - current) * k as f64);
            if deficit <= cost {
                let level = (current + deficit / k as f64) * (1.0 + 8.0 * f64::EPSILON);
                for (i, _) in order.iter().take(k) {
                    if let Some(t) = window.get_mut(*i) {
                        *t = level;
                    }
                }
                break;
            }
            deficit -= cost;
        }
    }
}
