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
    /// Where it ended: X, Y, Z, A.
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

/// Reads the driver's JSON report.
pub fn parse_sim_report(text: &str) -> Result<SimReport, SimError> {
    serde_json::from_str(text).map_err(|e| SimError::Report(e.to_string()))
}

/// Compares a simulator report with the replay of the same program. `tolerance`
/// allows for the sampling step of the driver (about 1 ms of motion).
pub fn compare_simulator(
    replay: &Replay,
    report: &SimReport,
    tolerance: f64,
) -> Result<SimComparison, SimError> {
    if !report.errors.is_empty() {
        return Err(SimError::Faults(report.errors.join("; ")));
    }
    let mut out = SimComparison::default();
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
        let key = mv.sequence.unwrap_or(mv.line);
        let row = report.rows.iter().find(|r| r.line == key);
        match (row, moved) {
            (Some(row), _) => {
                let error = row
                    .end
                    .iter()
                    .zip(&mv.to)
                    .map(|(a, b)| (a - b).abs())
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
        if error > tolerance {
            return Err(SimError::Mismatch(format!(
                "the program ended at {:?} but the simulator stopped at {:?}",
                last, report.final_position
            )));
        }
        out.max_error = out.max_error.max(error);
    }
    Ok(out)
}
