// SPDX-License-Identifier: AGPL-3.0-or-later
//! Bounded first-cut verification fixtures (GC-10, NFR-06, OI-991).

use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};

/// Conservative simulator-time budget, including fixture positioning/startup.
pub(super) const SECONDS: f64 = 300.0;

/// Only the fixed modes used by first_cut are supported. Unknown modes fail closed.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub(super) struct Boundary {
    pub position: [f64; 4],
    pub motion: u8,
    pub feed: Option<f64>,
    pub tool: u32,
    pub spindle_on: bool,
    pub rpm: f64,
}

#[derive(Debug)]
pub(super) struct Chunk {
    pub source: String,
    pub text: String,
    pub first_line: usize,
    pub last_line: usize,
    pub estimated_seconds: f64,
    pub boundaries: [Boundary; 2],
}

#[derive(Serialize, Deserialize)]
pub(super) struct Manifest {
    pub original_program: std::path::PathBuf,
    pub source_program: String,
    pub first_line: usize,
    pub last_line: usize,
    pub original_lines: String,
    pub estimated_seconds: f64,
    pub states: [Boundary; 2],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<u64>,
}

/// Read only this verification fixture's numeric, absolute G0/G1 subset.
fn words(raw: &str) -> Result<Vec<(u8, f64)>> {
    let mut code = String::new();
    let mut comment = false;
    for c in raw.chars() {
        match c {
            ';' if !comment => break,
            '(' if !comment => comment = true,
            ')' if comment => comment = false,
            '(' | ')' => bail!("unbalanced/nested comment in replay fixture"),
            _ if !comment => code.push(c),
            _ => {}
        }
    }
    ensure!(!comment, "unclosed comment in replay fixture");
    let mut rest = code.trim();
    let mut out = Vec::new();
    while !rest.is_empty() {
        let letter = rest
            .as_bytes()
            .first()
            .copied()
            .unwrap_or(0)
            .to_ascii_uppercase();
        ensure!(
            letter.is_ascii_alphabetic(),
            "unsupported replay syntax: {rest}"
        );
        rest = rest.get(1..).unwrap_or_default().trim_start();
        let n = rest
            .bytes()
            .take_while(|b| b.is_ascii_digit() || matches!(b, b'.' | b'-' | b'+'))
            .count();
        let number: f64 = rest
            .get(..n)
            .unwrap_or_default()
            .parse()
            .context("invalid replay number")?;
        ensure!(number.is_finite(), "non-finite replay number");
        out.push((letter, number));
        rest = rest.get(n..).unwrap_or_default().trim_start();
    }
    Ok(out)
}

impl Boundary {
    /// Apply one original line and estimate its simulator duration, never a controller limit.
    fn advance(&mut self, raw: &str) -> Result<(f64, bool)> {
        let fields = words(raw)?;
        let terminal = fields
            .iter()
            .any(|(k, v)| *k == b'M' && matches!(*v, 2.0 | 30.0));
        if terminal {
            ensure!(fields.len() == 1, "program end must be on its own line");
            return Ok((0.0, true));
        }
        let before = self.position;
        let mut selected_tool = None;
        for (key, value) in &fields {
            match *key {
                b'G' => match *value {
                    0.0 => self.motion = 0,
                    1.0 => self.motion = 1,
                    17.0 | 21.0 | 54.0 | 61.0 | 90.0 | 94.0 => {}
                    _ => bail!("unsupported chunk mode G{value}; do not guess its state"),
                },
                b'F' => {
                    ensure!(*value > 0.0, "feed must be positive");
                    self.feed = Some(*value);
                }
                b'S' => {
                    ensure!(*value >= 0.0, "spindle speed must be nonnegative");
                    self.rpm = *value;
                }
                b'T' => {
                    ensure!(
                        *value >= 0.0 && *value <= f64::from(u32::MAX) && value.fract() == 0.0,
                        "invalid tool number"
                    );
                    selected_tool = Some(*value as u32);
                }
                b'X' | b'Y' | b'Z' | b'A' | b'M' => {}
                _ => bail!("unsupported chunk word {}{value}", char::from(*key)),
            }
        }
        let change = fields.contains(&(b'M', 6.0));
        ensure!(
            change == selected_tool.is_some(),
            "T and M6 must be on the same line"
        );
        for (key, value) in &fields {
            if *key == b'M' {
                match *value {
                    3.0 => {
                        ensure!(self.rpm > 0.0, "M3 needs a positive S value");
                        self.spindle_on = true;
                    }
                    5.0 => self.spindle_on = false,
                    6.0 => self.tool = selected_tool.unwrap_or(0),
                    _ => bail!("unsupported chunk M{value}"),
                }
            }
            if let Some(index) = b"XYZA".iter().position(|k| k == key)
                && let Some(axis) = self.position.get_mut(index)
            {
                *axis = *value;
            }
        }
        let delta: Vec<f64> = before
            .iter()
            .zip(self.position)
            .map(|(a, b)| (b - a).abs())
            .collect();
        let linear = delta.iter().take(3).map(|d| d * d).sum::<f64>().sqrt();
        let angular = delta.get(3).copied().unwrap_or(0.0);
        if linear == 0.0 && angular == 0.0 {
            return Ok((0.0, false));
        }
        // checks.ini: 66.667 mm/s linear, 360 degrees/s A. 0.05 s bounds
        // acceleration/deceleration overhead per stopped block in that simulator.
        let velocity_floor = (linear / 66.667).max(angular / 360.0);
        let nominal = if self.motion == 1 {
            let feed = self.feed.context("fed motion without F")?;
            60.0 * if linear > 0.0 { linear } else { angular } / feed
        } else {
            0.0
        };
        let seconds = nominal.max(velocity_floor) + 0.05;
        ensure!(seconds.is_finite(), "unbounded replay duration");
        Ok((seconds, false))
    }

    fn prelude(&self) -> String {
        let mut text = "G90 G21 G17 G61 G54 G94\nM5\n".to_owned();
        if self.tool > 0 {
            text.push_str(&format!("T{} M6\n", self.tool));
        }
        text.push_str(&format!(
            "S{}{}\n",
            self.rpm,
            if self.spindle_on { " M3" } else { "" }
        ));
        text.push_str("G0");
        for (axis, value) in "XYZA".chars().zip(self.position) {
            text.push_str(&format!(" {axis}{value}"));
        }
        text.push('\n');
        if let Some(feed) = self.feed {
            text.push_str(&format!("F{feed}\n"));
        }
        text.push_str(&format!(
            "G{}\nM0 (replay boundary)\nM66 E0 L0\n",
            self.motion
        ));
        text
    }
}

fn finish(
    source: String,
    first_line: usize,
    last_line: usize,
    estimated_seconds: f64,
    start: Boundary,
    end: Boundary,
) -> Result<Chunk> {
    let mut text = start.prelude();
    let mut terminal = false;
    for raw in source.split_inclusive('\n') {
        if words(raw)?
            .iter()
            .any(|(k, v)| *k == b'M' && matches!(*v, 2.0 | 30.0))
        {
            text.push_str("M0 (replay boundary)\nM66 E0 L0\n");
            terminal = true;
        }
        text.push_str(raw);
    }
    if !terminal {
        if !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str("M0 (replay boundary)\nM66 E0 L0\nM5\nM30\n");
    }
    Ok(Chunk {
        source,
        text,
        first_line,
        last_line,
        estimated_seconds,
        boundaries: [start, end],
    })
}

/// Initialize simulated joints at the source boundary, with no positioning motion.
/// Travel/velocity/acceleration limits and the interpreter configuration stay intact.
pub(super) fn start_config(config: &str, state: &Boundary) -> Result<String> {
    let mut joint = None;
    let mut seen = [false; 4];
    let mut out = String::new();
    for raw in config.split_inclusive('\n') {
        let line = raw.trim();
        if line.starts_with('[') {
            joint = line
                .strip_prefix("[JOINT_")
                .and_then(|s| s.strip_suffix(']'))
                .and_then(|s| s.parse::<usize>().ok());
        }
        let key = line.split_once('=').map(|(k, _)| k.trim());
        if let Some(index) = joint
            && let Some(value) = state.position.get(index)
        {
            if key == Some("HOME_OFFSET") {
                continue;
            }
            if key == Some("HOME") {
                out.push_str(&format!("HOME = {value}\nHOME_OFFSET = {value}\n"));
                if let Some(done) = seen.get_mut(index) {
                    *done = true;
                }
                continue;
            }
        }
        out.push_str(raw);
    }
    ensure!(seen.iter().all(|v| *v), "missing joint HOME configuration");
    Ok(out)
}

/// Preserve contiguous original lines exactly once. Only simulation wrappers are added.
pub(super) fn plan(source: &str, budget: f64) -> Result<Vec<Chunk>> {
    ensure!(
        budget.is_finite() && budget > 0.0 && budget <= 600.0,
        "chunk budget must be in (0, 600]"
    );
    let mut parts = Vec::new();
    let mut state = Boundary::default();
    let mut start = state.clone();
    // Simulator joints are homed at the recorded start; no added positioning trip.
    let mut elapsed = 45.05;
    let mut body = String::new();
    let mut first = 1;
    let mut last = 0;
    let mut ended = false;
    for (i, raw) in source.split_inclusive('\n').enumerate() {
        ensure!(!ended || words(raw)?.is_empty(), "code after program end");
        let before = state.clone();
        let (seconds, terminal) = state
            .advance(raw)
            .with_context(|| format!("source line {}", i + 1))?;
        if elapsed + seconds > budget {
            ensure!(!body.is_empty(), "a single move exceeds the chunk budget");
            parts.push(finish(
                std::mem::take(&mut body),
                first,
                i,
                elapsed,
                start,
                before.clone(),
            )?);
            first = i + 1;
            start = before;
            elapsed = 45.05;
        }
        ensure!(
            elapsed + seconds <= budget,
            "source line {}: single move takes {seconds:.3} s plus {:.3} s startup, over budget {budget}",
            i + 1,
            elapsed
        );
        elapsed += seconds;
        body.push_str(raw);
        ended |= terminal;
        last = i + 1;
    }
    ensure!(ended, "missing program end");
    if !body.is_empty() {
        parts.push(finish(body, first, last, elapsed, start, state)?);
    }
    Ok(parts)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp)] // Exact preservation, not a numerical approximation.
    use super::*;

    const FIRST_CUT: &str =
        include_str!("../../../testdata/golden/first_cut/first_cut_linuxcnc.ngc");

    #[test]
    fn req_gc_10_req_nfr_06_chunks_preserve_every_original_line_and_boundary_state() {
        let parts = plan(FIRST_CUT, SECONDS).unwrap();
        let repeated = plan(FIRST_CUT, SECONDS).unwrap();
        assert_eq!(
            parts.iter().map(|p| &p.text).collect::<Vec<_>>(),
            repeated.iter().map(|p| &p.text).collect::<Vec<_>>()
        );
        assert!(parts.len() > 1);
        assert_eq!(
            parts.iter().map(|p| p.source.as_str()).collect::<String>(),
            FIRST_CUT
        );
        for part in &parts {
            assert!(part.estimated_seconds <= SECONDS);
            assert_eq!(part.boundaries.len(), 2);
            assert_eq!(part.text.matches("M0 (replay boundary)").count(), 2);
            assert_eq!(
                part.text
                    .matches("M0 (replay boundary)\nM66 E0 L0\n")
                    .count(),
                2
            );
        }
        for adjacent in parts.windows(2) {
            assert_eq!(adjacent[0].boundaries[1], adjacent[1].boundaries[0]);
            assert_eq!(adjacent[0].last_line + 1, adjacent[1].first_line);
        }
    }

    #[test]
    fn req_gc_10_chunks_restore_implicit_motion_feed_tool_and_spindle() {
        let source = "G90 G21 G17 G61 G54\nT2 M6\nS9000 M3\nG0 X1 Y2 Z3 A0\nG1 X11 F60\nX21\nX31\nX41\nM5\nM30\n";
        let parts = plan(source, 60.0).unwrap();
        let next = &parts[1];
        assert_eq!(next.boundaries[0].position, [11.0, 2.0, 3.0, 0.0]);
        assert_eq!(next.boundaries[0].feed, Some(60.0));
        assert_eq!(next.boundaries[0].tool, 2);
        assert!(next.boundaries[0].spindle_on);
        assert_eq!(next.boundaries[0].rpm, 9000.0);
        assert!(next.text.contains("T2 M6\nS9000 M3\n"));
        assert!(
            next.text
                .contains("F60\nG1\nM0 (replay boundary)\nM66 E0 L0\nX21\n")
        );
    }

    #[test]
    fn req_gc_10_req_nfr_07_chunks_refuse_unmodelled_modes_and_unbounded_moves() {
        for bad in [
            "G91",
            "G20",
            "G2 X1 I1",
            "G92.4 A0 S0",
            "M0",
            "G1 X1 F0",
            "G1 X1 F-1",
            "Q1",
            "#1=2",
        ] {
            let source = format!("G90 G21 G17 G61 G54\n{bad}\nM30\n");
            assert!(plan(&source, 240.0).is_err(), "accepted {bad}");
        }
        assert!(plan("G1 X100 F1\nM30\n", 240.0).is_err());
        assert!(plan("M30\nG0 X1\n", 240.0).is_err());
        assert!(plan("G0 X1\n", 240.0).is_err());
        for budget in [0.0, f64::NAN, 601.0] {
            assert!(plan(FIRST_CUT, budget).is_err());
        }
    }

    #[test]
    fn req_gc_10_chunk_homing_restores_coordinates_without_changing_limits() {
        let config = include_str!("../../../tools/linuxcnc/checks.ini");
        let state = Boundary {
            position: [70.0, 0.0, 5.0, 200000.0],
            ..Boundary::default()
        };
        let configured = start_config(config, &state).unwrap();
        assert!(configured.contains("HOME = 200000\nHOME_OFFSET = 200000"));
        assert!(configured.contains("HOME = 70\nHOME_OFFSET = 70"));
        assert_eq!(
            config
                .lines()
                .filter(|l| l.contains("LIMIT")
                    || l.contains("VELOCITY")
                    || l.contains("ACCELERATION"))
                .collect::<Vec<_>>(),
            configured
                .lines()
                .filter(|l| l.contains("LIMIT")
                    || l.contains("VELOCITY")
                    || l.contains("ACCELERATION"))
                .collect::<Vec<_>>()
        );
    }
}
