// SPDX-License-Identifier: AGPL-3.0-or-later
//! GC-10: compare bounded fixtures with the independent replay of the original source.
use std::ops::RangeInclusive;

use crate::{
    Dialect, Motion, Move, Program, SimBoundary, SimComparison, SimError, SimReport, Step,
    compare_simulator, read, replay,
};

fn invalid(message: impl Into<String>) -> SimError {
    SimError::Report(message.into())
}

fn subset(program: &Program) -> Result<(), SimError> {
    let mut ended = false;
    for block in &program.blocks {
        if ended && !block.words.is_empty() {
            return Err(invalid("source code after program end"));
        }
        for word in &block.words {
            if word.letter == 'G'
                && ![0.0, 1.0, 17.0, 21.0, 54.0, 61.0, 90.0, 94.0].contains(&word.value)
            {
                return Err(invalid(format!(
                    "source range uses unsupported G{}",
                    word.text
                )));
            }
            if word.letter == 'M' {
                if ![2.0, 3.0, 5.0, 6.0, 30.0].contains(&word.value) {
                    return Err(invalid(format!(
                        "source range uses unsupported M{}",
                        word.text
                    )));
                }
                if matches!(word.value, 2.0 | 30.0) {
                    if block.words.len() != 1 {
                        return Err(invalid("source end must be on its own line"));
                    }
                    ended = true;
                }
            }
        }
    }
    Ok(())
}

/// Use the independent interpreter, including a nonmoving marker, to expose every
/// modal field immediately after `line`. End commands are checked before their reset.
fn state_at(program: &Program, line: usize) -> Result<(Move, f64), SimError> {
    let mut prefix = program.clone();
    prefix.blocks.retain(|b| {
        b.line <= line
            && !b
                .words
                .iter()
                .any(|w| w.letter == 'M' && matches!(w.value, 2.0 | 30.0))
    });
    let before = replay(&prefix, Dialect::Linuxcnc).map_err(|e| invalid(e.to_string()))?;
    let position = before.moves().last().map_or([0.0; 4], |m| m.to);
    let rpm = prefix
        .blocks
        .iter()
        .flat_map(|b| &b.words)
        .rev()
        .find(|w| w.letter == 'S')
        .map_or(0.0, |w| w.value);
    let mut marker = String::new();
    for (axis, value) in "XYZA".chars().zip(position) {
        marker.push_str(&format!("{axis}{value} "));
    }
    let parsed = read(&marker).map_err(|e| invalid(format!("source boundary marker: {e:?}")))?;
    prefix.blocks.extend(parsed.blocks);
    let replayed = replay(&prefix, Dialect::Linuxcnc).map_err(|e| invalid(e.to_string()))?;
    let state = replayed
        .moves()
        .last()
        .cloned()
        .ok_or_else(|| invalid("missing source boundary state"))?;
    Ok((state, rpm))
}

fn check_boundary(
    program: &Program,
    line: usize,
    actual: &SimBoundary,
    label: &str,
) -> Result<(), SimError> {
    let (expected, rpm) = state_at(program, line)?;
    let bad = |field: &str| {
        invalid(format!(
            "source boundary {label} (after line {line}): {field} differs from original program"
        ))
    };
    for values in [actual.position, actual.actual_position] {
        if values
            .iter()
            .zip(expected.to)
            .any(|(a, e)| !a.is_finite() || (*a - e).abs() > 0.001)
        {
            return Err(bad("position"));
        }
    }
    let motion = match expected.motion {
        Motion::Rapid => 0,
        Motion::Linear => 10,
        _ => return Err(bad("motion")),
    };
    for code in [motion, 170, 210, 540, 610, 900, 940] {
        if !actual.gcodes.contains(&code) {
            return Err(bad("G modes"));
        }
    }
    if actual.tool != expected.tool.unwrap_or(0)
        || actual.spindle_on != expected.spindle_on
        || actual.direction != i32::from(expected.spindle_on)
    {
        return Err(bad("tool/spindle"));
    }
    if !actual.rpm.is_finite() || (actual.rpm - rpm).abs() > 1e-6 {
        return Err(bad("S"));
    }
    if !actual.feed.is_finite()
        || expected
            .feed
            .is_some_and(|f| (actual.feed - f).abs() > 1e-6)
    {
        return Err(bad("F"));
    }
    if actual
        .g5x_offset
        .iter()
        .chain(&actual.g92_offset)
        .any(|v| !v.is_finite() || v.abs() > 1e-9)
        || !actual.rotation_xy.is_finite()
        || actual.rotation_xy.abs() > 1e-9
    {
        return Err(bad("coordinate offsets/rotation"));
    }
    Ok(())
}

/// Compare a bounded fixture with a line range of the **original** program.
/// The whole original prefix is replayed independently to check both actual
/// boundaries; a self-consistent but incorrectly restored fixture cannot pass.
/// Source lines are inclusive. This verification-fixture API supports only the
/// absolute metric G0/G1 subset used by first_cut, with zero work offsets.
/// At least one original move and exactly two checked boundaries are required.
pub fn compare_simulator_range(
    program: &Program,
    report: &SimReport,
    lines: RangeInclusive<usize>,
    tolerance: f64,
) -> Result<SimComparison, SimError> {
    if !tolerance.is_finite() || tolerance < 0.0 {
        return Err(invalid(
            "source range tolerance must be finite and nonnegative",
        ));
    }
    let (first, last) = (*lines.start(), *lines.end());
    if first == 0 || last < first || last > program.blocks.last().map_or(0, |b| b.line) {
        return Err(invalid("invalid original source range"));
    }
    subset(program)?;
    if !report.errors.is_empty() {
        return Err(SimError::Faults(report.errors.join("; ")));
    }
    if report.boundaries.len() != 2 {
        return Err(invalid("source range requires two actual boundary checks"));
    }
    let start = report
        .boundaries
        .first()
        .ok_or_else(|| invalid("missing start boundary"))?;
    let end = report
        .boundaries
        .last()
        .ok_or_else(|| invalid("missing end boundary"))?;
    check_boundary(program, first - 1, start, "start")?;
    check_boundary(program, last, end, "end")?;
    let mut original = replay(program, Dialect::Linuxcnc).map_err(|e| invalid(e.to_string()))?;
    original
        .steps
        .retain(|s| matches!(s, Step::Move(m) if lines.contains(&m.line)));
    if original.moves().next().is_none() {
        return Err(invalid("source range contains no original moves"));
    }
    compare_simulator(&original, report, tolerance)
}
