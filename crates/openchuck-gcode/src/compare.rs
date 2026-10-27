// SPDX-License-Identifier: AGPL-3.0-or-later
//! Gate 2: replay compared with the IR (verification plan section 6; work
//! breakdown slice 6 items 7 and 11).
//!
//! Every position the G-code reaches is mapped back to a tool tip in the model
//! frame by the shared kinematics and compared with the IR's pose; A is
//! compared modulo whole turns (an unwinding command may shift it); the A
//! travel and the cut time under the controller's feed rule are compared with
//! the IR's. The feed timing here is written separately from the post's feed
//! rules, so a mistake in one is caught by the other (architecture section 9).

use openchuck_types::geometry::Point3;
use openchuck_types::ir::Toolpath;
use openchuck_types::kinematics::{
    AsRotaryFrame, MachinePosition, axis_direction, machine_xyz, tool_tip_in_model,
};
use openchuck_types::units::Degrees;

use crate::interp::{Event, Move, Replay, Step};

/// How the controller turns an F word into time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimingRule {
    /// LinuxCNC G94.
    Linuxcnc,
    /// Makera Z1 G94 (decision record 0007).
    Makera,
    /// Inverse time (G93).
    InverseTime,
}

/// What the comparison allows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tolerances {
    /// Largest tip error, mm.
    pub position_mm: f64,
    /// Largest A error modulo whole turns, degrees.
    pub a_deg: f64,
    /// Largest relative error of a move's time.
    pub time_relative: f64,
    /// Decimal places of the written X, Y and Z.
    pub pos_decimals: u32,
    /// Decimal places of the written A.
    pub a_decimals: u32,
}

impl Tolerances {
    /// Rounding only: half a unit in the last written decimal for positions
    /// and A, doubled for the combined effect of three axes (verification plan
    /// 4.1: 0.0005 mm and 0.001 degrees at three decimals).
    pub fn for_decimals(pos: u32, a: u32) -> Self {
        let half = |d: u32| 0.5 * 10f64.powi(-i32::try_from(d).unwrap_or(9));
        Self {
            position_mm: half(pos) * 3f64.sqrt() * 1.001,
            a_deg: half(a) * 2.0,
            time_relative: 0.01,
            pos_decimals: pos,
            a_decimals: a,
        }
    }
}

/// What the comparison found.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Comparison {
    /// Poses compared.
    pub poses: usize,
    /// Largest tip error, mm.
    pub max_position_error: f64,
    /// Largest A error modulo whole turns, degrees.
    pub max_a_error: f64,
    /// A travel of the IR, not counting whole-turn unwinds, degrees.
    pub ir_a_travel: f64,
    /// A travel of the G-code, not counting whole-turn unwinds, degrees.
    pub gcode_a_travel: f64,
    /// Sum of the IR's fed move durations, seconds.
    pub ir_cut_seconds: f64,
    /// The same under the controller's feed rule from the G-code, seconds.
    pub gcode_cut_seconds: f64,
    /// Largest relative error of a fed move's time.
    pub max_time_error: f64,
    /// The source line of the move with the largest time error.
    pub worst_time_line: usize,
}

/// Why a comparison could not be made or failed.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum CompareError {
    /// The G-code has fewer or more moves than the IR needs.
    #[error("{0}")]
    Structure(String),
    /// A tolerance was exceeded.
    #[error("{0}")]
    Tolerance(String),
}

fn same(a: &[f64; 4], b: &[f64; 4]) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-9)
}

fn whole_turns(delta: f64) -> bool {
    delta.abs() > 1e-9 && ((delta / 360.0) - (delta / 360.0).round()).abs() * 360.0 < 1e-9
}

fn gcode_seconds(m: &Move, rule: TimingRule, axis: [f64; 3]) -> Option<f64> {
    let f = m.feed?;
    if f <= 0.0 {
        return None;
    }
    if rule == TimingRule::InverseTime {
        return Some(60.0 / f);
    }
    let length = ((m.to[0] - m.from[0]).powi(2)
        + (m.to[1] - m.from[1]).powi(2)
        + (m.to[2] - m.from[2]).powi(2))
    .sqrt();
    let da = (m.to[3] - m.from[3]).abs();
    let amount = match rule {
        TimingRule::Linuxcnc => {
            if length > 1e-9 {
                length
            } else {
                da
            }
        }
        _ => {
            let along = m.to[0] * axis[0] + m.to[1] * axis[1] + m.to[2] * axis[2];
            let r = ((m.to[0] - along * axis[0]).powi(2)
                + (m.to[1] - along * axis[1]).powi(2)
                + (m.to[2] - along * axis[2]).powi(2))
            .sqrt();
            // The firmware counts a radius of 1 mm or less as 1 mm (decision 0007).
            let p = 2.0 * std::f64::consts::PI * r.max(1.0) + 30.0;
            if length > 1e-9 {
                length.max(da * p / 360.0)
            } else {
                da * p.min(360.0) / 360.0
            }
        }
    };
    Some(amount * 60.0 / f)
}

/// Compares a replay with the IR it was posted from.
pub fn compare(
    replay: &Replay,
    toolpath: &Toolpath,
    frame: &impl AsRotaryFrame,
    rule: TimingRule,
    tolerances: Tolerances,
) -> Result<Comparison, CompareError> {
    compare_with_moves(replay, toolpath, frame, rule, tolerances, &mut |_| Ok(()))
}

/// One replayed move matched to a source operation. Observer values are
/// provisional until the ENTIRE comparison succeeds. A dropped sub-resolution
/// IR segment has no reached move; counter resets consume no segment.
#[derive(Debug, Clone, Copy)]
pub struct ComparedMove<'a> {
    /// Zero-based group index in the supplied toolpath.
    pub group: usize,
    /// Source segment, or None for a move to an operation's starting pose.
    pub segment: Option<usize>,
    /// Actual replayed work-coordinate words. A uses the replay's sign.
    pub movement: &'a Move,
    /// Add to the written A counter to recover the physical unwrapped A.
    pub coordinate_shift: f64,
    /// Programmed fed-move duration under the requested timing rule.
    /// None for a rapid. This is not a measured controller duration.
    pub seconds: Option<f64>,
}

/// Compare with the same structural/tolerance checks as [`compare`], exposing
/// reached moves for independent operation-specific checks. Observation cannot
/// waive a later comparison failure. An observer error aborts the comparison;
/// callers must discard all provisional output if any check fails.
pub fn compare_with_moves(
    replay: &Replay,
    toolpath: &Toolpath,
    frame: &impl AsRotaryFrame,
    rule: TimingRule,
    tolerances: Tolerances,
    observe: &mut impl FnMut(ComparedMove<'_>) -> Result<(), CompareError>,
) -> Result<Comparison, CompareError> {
    let frame = frame.rotary_frame();
    let axis = axis_direction(frame.axis);
    let axis = [axis.x, axis.y, axis.z];
    // Positions as the post writes them: the controller moves between these,
    // and a pose that rounds onto the one before it writes no line.
    let written = |v: f64, decimals: u32| {
        let places = usize::try_from(decimals).unwrap_or(9).min(9);
        let text = format!("{v:.places$}");
        text.parse::<f64>()
            .map_or(v, |x| if x == 0.0 { 0.0 } else { x })
    };
    let machine_of = |tip: Point3, a: f64| {
        let p = machine_xyz(&frame, tip, Degrees(a));
        [
            written(p.x, tolerances.pos_decimals),
            written(p.y, tolerances.pos_decimals),
            written(p.z, tolerances.pos_decimals),
            written(a, tolerances.a_decimals),
        ]
    };

    // Every IR segment is physical motion. Non-moving controller resets
    // may occur only after an operation boundary, consuming no IR pose.
    struct Expected {
        tip: Point3,
        a: f64,
        boundary_after: bool,
        fed: Option<f64>,
        group: usize,
        segment: Option<usize>,
        /// The program's first move, from wherever the controller is to the
        /// first group's start pose: the IR counts no A travel for it, so
        /// neither does the G-code. Later group starts count on both sides.
        start: bool,
    }
    let mut expected: Vec<Expected> = Vec::new();
    let mut ir_travel = 0.0;
    for (g, group) in toolpath.groups.iter().enumerate() {
        let mut prev = machine_of(group.start.tip, group.start.a.0);
        let before = expected.last().map(|e: &Expected| machine_of(e.tip, e.a));
        if g == 0 || !before.is_some_and(|b| same(&b, &prev)) {
            // The post writes the move to a group's start as a plain rapid, so
            // the A it turns between two groups is travel in the G-code and
            // counts here too (a file with more than one operation).
            if let Some(b) = before {
                ir_travel += (prev[3] - b[3]).abs();
            }
            expected.push(Expected {
                tip: group.start.tip,
                a: group.start.a.0,
                boundary_after: false,
                fed: None,
                group: g,
                segment: None,
                start: before.is_none(),
            });
        }
        for (index, segment) in group.segments.iter().enumerate() {
            let to = machine_of(segment.end.tip, segment.end.a.0);
            let moved = !same(&prev, &to);
            if moved {
                ir_travel += (to[3] - prev[3]).abs();
                let fed = segment.kind.is_fed().then_some(segment.duration.0);
                expected.push(Expected {
                    tip: segment.end.tip,
                    a: segment.end.a.0,
                    boundary_after: false,
                    fed,
                    group: g,
                    segment: Some(index),
                    start: false,
                });
            }
            prev = to;
        }
        if let Some(last) = expected.last_mut() {
            last.boundary_after = true;
        }
    }

    let mut out = Comparison {
        ir_a_travel: ir_travel,
        ..Comparison::default()
    };
    let mut steps = replay.steps.iter();
    let mut coordinate_shift = 0.0;
    let mut boundary = None;
    for e in &expected {
        let Some(mv) = next_move(
            &mut steps,
            &mut coordinate_shift,
            boundary,
            tolerances.a_deg,
        )?
        else {
            return Err(CompareError::Structure(format!(
                "the G-code ends before the IR does: group {} needs a move to tip ({:.3}, {:.3}, {:.3}) A {:.3}",
                e.group, e.tip.x, e.tip.y, e.tip.z, e.a
            )));
        };
        let tip = tool_tip_in_model(
            &frame,
            &MachinePosition {
                xyz: Point3::new(mv.to[0], mv.to[1], mv.to[2]),
                a: Degrees(mv.to[3]),
            },
        );
        let position_error = tip.distance(e.tip);
        let a_error = (mv.to[3] + coordinate_shift - e.a).abs();
        out.max_position_error = out.max_position_error.max(position_error);
        out.max_a_error = out.max_a_error.max(a_error);
        out.poses += 1;
        if !e.start {
            out.gcode_a_travel += (mv.to[3] - mv.from[3]).abs();
        }
        if let (Some(ir_seconds), Some(seconds)) = (e.fed, gcode_seconds(mv, rule, axis)) {
            out.ir_cut_seconds += ir_seconds;
            out.gcode_cut_seconds += seconds;
            if ir_seconds > 0.0 {
                let error = ((seconds - ir_seconds) / ir_seconds).abs();
                if error > out.max_time_error {
                    out.max_time_error = error;
                    out.worst_time_line = mv.line;
                }
            }
        } else if e.fed.is_some() {
            return Err(CompareError::Structure(format!(
                "line {}: a cutting move has no usable feed",
                mv.line
            )));
        }
        observe(ComparedMove {
            group: e.group,
            segment: e.segment,
            movement: mv,
            coordinate_shift,
            seconds: e.fed.and_then(|_| gcode_seconds(mv, rule, axis)),
        })?;
        boundary = e.boundary_after.then_some(e.a);
    }
    if next_move(
        &mut steps,
        &mut coordinate_shift,
        boundary,
        tolerances.a_deg,
    )?
    .is_some()
    {
        return Err(CompareError::Structure(
            "the G-code has moves beyond the end of the IR".into(),
        ));
    }
    if out.max_position_error > tolerances.position_mm {
        return Err(CompareError::Tolerance(format!(
            "tip error {:.6} mm is above {:.6} mm",
            out.max_position_error, tolerances.position_mm
        )));
    }
    if out.max_a_error > tolerances.a_deg {
        return Err(CompareError::Tolerance(format!(
            "A error {:.6} degrees is above {:.6}",
            out.max_a_error, tolerances.a_deg
        )));
    }
    let travel_allowed = tolerances.a_deg * (out.poses as f64).max(1.0);
    if (out.ir_a_travel - out.gcode_a_travel).abs() > travel_allowed {
        return Err(CompareError::Tolerance(format!(
            "A travel {:.3} degrees differs from the IR's {:.3}",
            out.gcode_a_travel, out.ir_a_travel
        )));
    }
    if out.max_time_error > tolerances.time_relative {
        return Err(CompareError::Tolerance(format!(
            "line {}: the move's time is {:.2} % off the IR's (limit {:.2} %)",
            out.worst_time_line,
            out.max_time_error * 100.0,
            tolerances.time_relative * 100.0
        )));
    }
    Ok(out)
}

/// Counter resets are bookkeeping at a reached operation boundary. Track
/// their offset explicitly so even a whole-turn error in a later command
/// cannot be hidden by comparing only equivalent orientations.
fn next_move<'a>(
    steps: &mut impl Iterator<Item = &'a Step>,
    shift: &mut f64,
    boundary: Option<f64>,
    tolerance: f64,
) -> Result<Option<&'a Move>, CompareError> {
    for step in steps.by_ref() {
        match step {
            Step::Move(m) if !m.machine => return Ok(Some(m)),
            Step::Event(Event::DropTurns { line, from, to }) => {
                let delta = from - to;
                let expected_to = if from.abs() > 360.0 {
                    from % 360.0
                } else {
                    *from
                };
                if !from.is_finite()
                    || !to.is_finite()
                    || !boundary.is_some_and(|a| (from + *shift - a).abs() <= tolerance)
                    || (delta.abs() > tolerance && !whole_turns(delta))
                    || (to - expected_to).abs() > tolerance
                {
                    return Err(CompareError::Structure(format!(
                        "line {line}: rotary counter reset is not at the reached operation boundary or does not match the controller"
                    )));
                }
                *shift += delta;
            }
            _ => {}
        }
    }
    Ok(None)
}
