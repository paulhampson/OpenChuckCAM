// SPDX-License-Identifier: AGPL-3.0-or-later
//! Gate 3: the LinuxCNC simulator's report compared with the replay of the
//! same program (verification plan section 6; work breakdown slice 6 items 9
//! and 11).
//!
//! The simulator driver (`tools/linuxcnc/sim_driver.py`) runs a program in
//! LinuxCNC's headless XYZA simulator and writes, for each program line that
//! moved, the position it ended at, plus any errors LinuxCNC reported and the
//! final position. Here that report is checked against our own replay of the
//! file: every move must reach its position, nothing may be reported (a limit
//! fault is an error), and the program must end where the replay says.
//!
//! trivkins treats A as a plain number, so this confirms the dialect, the
//! reached values and the absence of limit faults, **not** the A direction
//! (decision record 0001): the oracle and the air cut do that.

use serde::Deserialize;

use crate::interp::Replay;

/// One program line that moved in the simulator.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SimRow {
    /// The program line (one-based).
    pub line: usize,
    /// Seconds the line moved for.
    #[serde(default)]
    pub seconds: f64,
    /// Where it first was seen: X, Y, Z, A.
    #[serde(default)]
    pub first: Option<[f64; 4]>,
    /// The last position sampled during the line: X, Y, Z, A. A line that
    /// finishes between two samples ends up to one sampling step short of its
    /// target, so the next row's first position is what shows it was passed.
    pub end: [f64; 4],
}

/// The driver's report.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SimReport {
    /// Lines that moved, in order.
    pub rows: Vec<SimRow>,
    /// Errors LinuxCNC reported, limit faults included.
    #[serde(default)]
    pub errors: Vec<String>,
    /// Position when the program ended.
    pub final_position: [f64; 4],
    /// Every distinct position sampled during the run, in order. When it is
    /// there, the comparison follows it instead of the per-line rows, because
    /// LinuxCNC's reported line number lags the motion.
    #[serde(default)]
    pub path: Vec<[f64; 4]>,
}

/// Why the simulator run does not match.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SimError {
    /// The report could not be read.
    #[error("simulator report: {0}")]
    Report(String),
    /// LinuxCNC reported errors.
    #[error("LinuxCNC reported: {0}")]
    Faults(String),
    /// A move did not reach its position.
    #[error("{0}")]
    Mismatch(String),
}

/// What a matching run showed.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SimComparison {
    /// Moves checked.
    pub moves: usize,
    /// Largest distance between a reached and a replayed position (mm or degrees).
    pub max_error: f64,
    /// Seconds from the report's rows, summed.
    pub seconds: f64,
}

/// How far the final position may be from the replay's: the machine is at
/// rest then, so sampling plays no part.
pub const FINAL_TOLERANCE: f64 = 1e-3;

/// Reads the driver's JSON report.
pub fn parse_sim_report(text: &str) -> Result<SimReport, SimError> {
    serde_json::from_str(text).map_err(|e| SimError::Report(e.to_string()))
}

/// Compares a simulator report with the replay of the same program. `tolerance`
/// allows for the sampling step of the driver (about 1 ms of motion) and for
/// the program line that LinuxCNC reports lagging the motion by a sample or
/// two; the final position is held to [`FINAL_TOLERANCE`].
pub fn compare_simulator(
    replay: &Replay,
    report: &SimReport,
    tolerance: f64,
) -> Result<SimComparison, SimError> {
    if !report.errors.is_empty() {
        return Err(SimError::Faults(report.errors.join("; ")));
    }
    if !report.path.is_empty() {
        return compare_path(replay, report, tolerance);
    }
    let mut out = SimComparison::default();
    let mut last: Option<[f64; 4]> = None;
    // The window a line's target must lie in, per axis: from the last sample
    // of the line to the first sample of the next (or the final position).
    let window_end = |index: usize| -> [f64; 4] {
        report
            .rows
            .get(index + 1)
            .and_then(|next| next.first)
            .unwrap_or(report.final_position)
    };
    for mv in replay.moves() {
        if mv.machine {
            continue;
        }
        last = Some(mv.to);
        let moved = mv
            .from
            .iter()
            .zip(&mv.to)
            .any(|(a, b)| (a - b).abs() > tolerance);
        let key = mv.sequence.unwrap_or(mv.line);
        let found = report.rows.iter().position(|r| r.line == key);
        let row = found.and_then(|i| report.rows.get(i));
        match (row, moved) {
            (Some(row), _) => {
                let beyond = found.map_or(row.end, window_end);
                let error = (0..4)
                    .map(|i| {
                        let a = row.end.get(i).copied().unwrap_or(0.0);
                        let b = beyond.get(i).copied().unwrap_or(a);
                        let t = mv.to.get(i).copied().unwrap_or(0.0);
                        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
                        if t < lo {
                            lo - t
                        } else if t > hi {
                            t - hi
                        } else {
                            0.0
                        }
                    })
                    .fold(0.0, f64::max);
                if error > tolerance {
                    return Err(SimError::Mismatch(format!(
                        "line {}: the simulator ended at {:?}, the program says {:?}",
                        mv.line, row.end, mv.to
                    )));
                }
                out.max_error = out.max_error.max(error);
                out.seconds += row.seconds;
                out.moves += 1;
            }
            (None, true) => {
                return Err(SimError::Mismatch(format!(
                    "line {}: the simulator did not move for this line",
                    mv.line
                )));
            }
            (None, false) => {}
        }
    }
    if let Some(last) = last {
        let error = report
            .final_position
            .iter()
            .zip(&last)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f64::max);
        if error > FINAL_TOLERANCE {
            return Err(SimError::Mismatch(format!(
                "the program ended at {:?} but the simulator stopped at {:?}",
                last, report.final_position
            )));
        }
        out.max_error = out.max_error.max(error);
    }
    Ok(out)
}

/// Follows the sampled path: each target must be visited, in program order,
/// to within `tolerance` on every axis. With exact stop the machine slows to
/// a halt at every node, so the samples are dense there.
fn compare_path(
    replay: &Replay,
    report: &SimReport,
    tolerance: f64,
) -> Result<SimComparison, SimError> {
    let mut out = SimComparison::default();
    let mut index = 0usize;
    let mut last: Option<[f64; 4]> = None;
    for mv in replay.moves() {
        if mv.machine {
            continue;
        }
        last = Some(mv.to);
        let moved = mv
            .from
            .iter()
            .zip(&mv.to)
            .any(|(a, b)| (a - b).abs() > tolerance);
        if !moved {
            continue;
        }
        // The first sample, from `index` on, that is within tolerance of the
        // target on every axis, or the first pair of samples that the target
        // lies between (a pause in sampling must not hide a node the machine
        // passed through).
        let near = |p: &[f64; 4]| {
            p.iter()
                .zip(&mv.to)
                .all(|(a, b)| (a - b).abs() <= tolerance)
        };
        let mut hit = None;
        let mut k = index;
        while let Some(p) = report.path.get(k) {
            if near(p) {
                hit = Some(k);
                break;
            }
            if let Some(q) = report.path.get(k + 1)
                && passes_between(p, q, &mv.to, tolerance)
            {
                hit = Some(k);
                break;
            }
            k += 1;
        }
        match hit {
            Some(first) => {
                index = first;
                // Of the run of samples within tolerance, the nearest one is
                // where the machine stopped.
                let distance = |p: &[f64; 4]| {
                    p.iter()
                        .zip(&mv.to)
                        .map(|(a, b)| (a - b).abs())
                        .fold(0.0, f64::max)
                };
                let mut best = (
                    index,
                    report.path.get(index).map_or(f64::INFINITY, distance),
                );
                let mut k = index;
                while let Some(p) = report.path.get(k) {
                    let d = distance(p);
                    if d > tolerance && k > index + 1 {
                        break;
                    }
                    if d < best.1 {
                        best = (k, d);
                    }
                    k += 1;
                }
                index = best.0;
                out.max_error = out.max_error.max(best.1.min(tolerance));
                out.moves += 1;
            }
            None => {
                return Err(SimError::Mismatch(format!(
                    "line {}: the simulator never reached {:?} after the previous move",
                    mv.line, mv.to
                )));
            }
        }
    }
    out.seconds = report.rows.iter().map(|r| r.seconds).sum();
    if let Some(last) = last {
        let error = report
            .final_position
            .iter()
            .zip(&last)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f64::max);
        if error > FINAL_TOLERANCE {
            return Err(SimError::Mismatch(format!(
                "the program ended at {last:?} but the simulator stopped at {:?}",
                report.final_position
            )));
        }
    }
    Ok(out)
}

/// True if some point on the straight line from `p` to `q` is within `tolerance`
/// of `target` on every axis.
fn passes_between(p: &[f64; 4], q: &[f64; 4], target: &[f64; 4], tolerance: f64) -> bool {
    let (mut lo, mut hi) = (0.0_f64, 1.0_f64);
    for i in 0..4 {
        let (a, b, t) = (
            p.get(i).copied().unwrap_or(0.0),
            q.get(i).copied().unwrap_or(0.0),
            target.get(i).copied().unwrap_or(0.0),
        );
        let d = b - a;
        if d.abs() < 1e-12 {
            if (a - t).abs() > tolerance {
                return false;
            }
            continue;
        }
        let (t1, t2) = ((t - tolerance - a) / d, (t + tolerance - a) / d);
        lo = lo.max(t1.min(t2));
        hi = hi.min(t1.max(t2));
        if lo > hi {
            return false;
        }
    }
    true
}
