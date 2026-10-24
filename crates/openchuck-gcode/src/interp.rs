// SPDX-License-Identifier: AGPL-3.0-or-later
//! Modal replay of a read program: the positions reached, the feed in force
//! and the events around them (work breakdown slice 6 item 7).
//!
//! It knows the codes our posts emit and refuses any other by name; a post
//! that adds a code adds it here and to the subset fixture in the same change
//! (decision record 0005 item 4). Arcs are read and recorded but not replayed
//! as motion (OI-68).

use crate::reader::{Block, Program, Word};

/// A G-code dialect. Codes and meanings differ in a few places.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    /// LinuxCNC.
    Linuxcnc,
    /// The Makera Z1 firmware: also `G92.4 A0 S0`, which drops whole turns
    /// of A with no motion (decision record 0007).
    MakeraZ1,
}

/// How a move travels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motion {
    /// `G0`.
    Rapid,
    /// `G1`.
    Linear,
    /// `G2`.
    ArcCw,
    /// `G3`.
    ArcCcw,
}

/// Position of the four axes: X, Y, Z in the program's units, A in degrees.
pub type Axes = [f64; 4];

/// A move to a reached position.
#[derive(Debug, Clone, PartialEq)]
pub struct Move {
    /// One-based source line.
    pub line: usize,
    /// The `N` word of the block, which LinuxCNC reports in place of the line
    /// number when a program has them.
    pub sequence: Option<usize>,
    /// How it travels.
    pub motion: Motion,
    /// Where it started (work coordinates).
    pub from: Axes,
    /// Where it ended (work coordinates).
    pub to: Axes,
    /// The feed word in force, if one has been set.
    pub feed: Option<f64>,
    /// An `F` word was written in this block.
    pub feed_in_block: bool,
    /// Inverse-time mode (G93) was in force.
    pub inverse_time: bool,
    /// The move was in machine coordinates (`G53`): `from` and `to` are
    /// machine coordinates and are not tracked into later moves.
    pub machine: bool,
    /// The tool number selected, if any.
    pub tool: Option<u32>,
    /// The spindle was on.
    pub spindle_on: bool,
    /// Millimetres (G21) were in force.
    pub metric: bool,
}

/// Something that happens between moves.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// `T<n> M6` with the position the tool changes at.
    ToolChange {
        /// Source line.
        line: usize,
        /// The tool number.
        tool: u32,
        /// Where the axes stood, or `None` before the first move.
        at: Option<Axes>,
    },
    /// Spindle started (`M3` or `M4`) at a speed.
    SpindleOn {
        /// Source line.
        line: usize,
        /// Speed, rpm, as last set by `S`.
        rpm: f64,
    },
    /// Spindle stopped (`M5`).
    SpindleOff {
        /// Source line.
        line: usize,
    },
    /// `G92.4 A0 S0`: whole turns removed from A with no motion.
    DropTurns {
        /// Source line.
        line: usize,
        /// A before.
        from: f64,
        /// A after.
        to: f64,
    },
    /// `M2` or `M30`.
    ProgramEnd {
        /// Source line.
        line: usize,
    },
}

/// One thing the replay found, in program order.
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    /// A move.
    Move(Move),
    /// An event.
    Event(Event),
}

/// The result of a replay.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Replay {
    /// Moves and events in order.
    pub steps: Vec<Step>,
    /// A work offset (`G54` to `G59`) was selected before the first move.
    pub work_offset_before_motion: bool,
    /// Absolute distance mode (G90) was in force at the first move.
    pub absolute_at_first_move: bool,
    /// Units were set (G20 or G21) before the first move.
    pub units_before_motion: bool,
}

impl Replay {
    /// The moves only.
    pub fn moves(&self) -> impl Iterator<Item = &Move> {
        self.steps.iter().filter_map(|s| match s {
            Step::Move(m) => Some(m),
            Step::Event(_) => None,
        })
    }
}

/// A replay error naming the place.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("line {line}, column {col}: {message}")]
pub struct ReplayError {
    /// One-based line.
    pub line: usize,
    /// One-based column.
    pub col: usize,
    /// What is wrong.
    pub message: String,
}

/// G codes the posts emit and the reader subset covers.
pub const KNOWN_G: &[&str] = &[
    "0", "1", "2", "3", "4", "17", "18", "19", "20", "21", "28", "40", "43", "49", "53", "54",
    "55", "56", "57", "58", "59", "61", "61.1", "64", "90", "90.1", "91", "91.1", "93", "94",
];
/// The Z1-only G code.
pub const Z1_G: &str = "92.4";
/// M codes the posts emit.
pub const KNOWN_M: &[&str] = &["0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "30"];

fn code_key(word: &Word) -> String {
    // From the source text, so G01, G1 and G1.0 agree and G61.1 stays distinct.
    let value = word.text.parse::<f64>().unwrap_or(word.value);
    if (value - value.round()).abs() < 1e-12 {
        format!("{}", value.round() as i64)
    } else {
        format!("{value}")
    }
}

#[derive(Clone)]
struct State {
    pos: Axes,
    motion: Motion,
    absolute: bool,
    inverse: bool,
    metric: bool,
    feed: Option<f64>,
    tool: Option<u32>,
    pending_tool: Option<u32>,
    rpm: f64,
    spindle_on: bool,
}

fn err(word: &Word, message: impl Into<String>) -> ReplayError {
    ReplayError {
        line: word.line,
        col: word.col,
        message: message.into(),
    }
}

/// Replays a program whose rotary axis is written with the letter `A`.
pub fn replay(program: &Program, dialect: Dialect) -> Result<Replay, ReplayError> {
    replay_axis(program, dialect, 'A')
}

/// Replays a program whose rotary axis is written with `a_letter` (`A`, `B`
/// or `C`; a post chooses it, GC-04). The other two letters are then not in the
/// subset and are refused by name.
pub fn replay_axis(
    program: &Program,
    dialect: Dialect,
    a_letter: char,
) -> Result<Replay, ReplayError> {
    let mut s = State {
        pos: [0.0; 4],
        motion: Motion::Rapid,
        absolute: true,
        inverse: false,
        metric: true,
        feed: None,
        tool: None,
        pending_tool: None,
        rpm: 0.0,
        spindle_on: false,
    };
    let context = Context { dialect, a_letter };
    let mut out = Replay::default();
    let mut work_offset = false;
    let mut units = false;
    let mut moved = false;
    for block in &program.blocks {
        if block.block_delete {
            continue;
        }
        replay_block(
            block,
            &context,
            &mut s,
            &mut out,
            &mut work_offset,
            &mut units,
            &mut moved,
        )?;
    }
    Ok(out)
}

/// What a replay was asked for, which does not change from block to block.
#[derive(Clone, Copy)]
struct Context {
    dialect: Dialect,
    a_letter: char,
}

fn replay_block(
    block: &Block,
    context: &Context,
    s: &mut State,
    out: &mut Replay,
    work_offset: &mut bool,
    units: &mut bool,
    moved: &mut bool,
) -> Result<(), ReplayError> {
    let Context { dialect, a_letter } = *context;
    let mut target: [Option<f64>; 4] = [None; 4];
    let mut skip_motion = false;
    let mut machine = false;
    let mut drop_turns = false;
    let mut m_events: Vec<(&Word, String)> = Vec::new();
    let mut has_m6 = false;
    let mut s_word: Option<f64> = None;
    let mut feed_in_block = false;
    for w in &block.words {
        match w.letter {
            'G' => {
                let key = code_key(w);
                if key == Z1_G && dialect == Dialect::MakeraZ1 {
                    drop_turns = true;
                    continue;
                }
                if !KNOWN_G.contains(&key.as_str()) {
                    return Err(err(w, format!("G{key} is not in the post subset")));
                }
                match key.as_str() {
                    "0" => s.motion = Motion::Rapid,
                    "1" => s.motion = Motion::Linear,
                    "2" => s.motion = Motion::ArcCw,
                    "3" => s.motion = Motion::ArcCcw,
                    "4" | "28" => skip_motion = true,
                    "53" => machine = true,
                    "54" | "55" | "56" | "57" | "58" | "59" => {
                        if !*moved {
                            *work_offset = true;
                        }
                    }
                    "90" => s.absolute = true,
                    "91" => s.absolute = false,
                    "93" => s.inverse = true,
                    "94" => s.inverse = false,
                    "20" => {
                        s.metric = false;
                        *units = !*moved;
                    }
                    "21" => {
                        s.metric = true;
                        *units = !*moved;
                    }
                    _ => {}
                }
            }
            'M' => {
                let key = code_key(w);
                if !KNOWN_M.contains(&key.as_str()) {
                    return Err(err(w, format!("M{key} is not in the post subset")));
                }
                m_events.push((w, key.clone()));
                if key == "6" {
                    has_m6 = true;
                }
            }
            'X' => target[0] = Some(w.value),
            'Y' => target[1] = Some(w.value),
            'Z' => target[2] = Some(w.value),
            letter if letter == a_letter => target[3] = Some(w.value),
            'F' => {
                s.feed = Some(w.value);
                feed_in_block = true;
            }
            'S' => s_word = Some(w.value),
            'T' => {
                if w.value < 0.0 || w.value.fract() != 0.0 || w.value > f64::from(u32::MAX) {
                    return Err(err(w, "a tool number must be a whole number"));
                }
                s.pending_tool = Some(w.value as u32);
            }
            'N' | 'H' | 'P' | 'Q' | 'I' | 'J' | 'K' | 'R' | 'D' | 'L' => {}
            other => return Err(err(w, format!("word `{other}` is not in the post subset"))),
        }
    }
    if let Some(rpm) = s_word
        && !drop_turns
    {
        s.rpm = rpm;
    }
    let first = block.words.first();
    if drop_turns {
        let w = first.ok_or_else(|| ReplayError {
            line: block.line,
            col: 1,
            message: "empty block".into(),
        })?;
        let a_ok = target[3].is_some_and(|v| v == 0.0);
        let s_ok = s_word.is_some_and(|v| v == 0.0);
        if !(a_ok && s_ok) || target[..3].iter().any(Option::is_some) {
            return Err(err(
                w,
                "G92.4 is only read as `G92.4 A0 S0`, which drops whole turns of A",
            ));
        }
        let from = s.pos[3];
        // Z1 firmware 1.1.2 uses fabs(wa) > 360 before fmodf. Exactly
        // +/-360 remains unchanged; treating it as zero misreads the next
        // absolute A move by a full turn (decision 0238).
        if from.abs() > 360.0 {
            s.pos[3] = from % 360.0;
        }
        out.steps.push(Step::Event(Event::DropTurns {
            line: block.line,
            from,
            to: s.pos[3],
        }));
        return Ok(());
    }
    for (w, key) in m_events {
        match key.as_str() {
            "3" | "4" => {
                s.spindle_on = true;
                out.steps.push(Step::Event(Event::SpindleOn {
                    line: w.line,
                    rpm: s.rpm,
                }));
            }
            "5" => {
                s.spindle_on = false;
                out.steps
                    .push(Step::Event(Event::SpindleOff { line: w.line }));
            }
            "2" | "30" => out
                .steps
                .push(Step::Event(Event::ProgramEnd { line: w.line })),
            _ => {}
        }
    }
    if has_m6 {
        let tool = s.pending_tool.or(s.tool);
        if let Some(tool) = tool {
            s.tool = Some(tool);
            out.steps.push(Step::Event(Event::ToolChange {
                line: block.line,
                tool,
                at: (*moved).then_some(s.pos),
            }));
        }
    }
    if skip_motion || target.iter().all(Option::is_none) {
        return Ok(());
    }
    if !*moved {
        out.work_offset_before_motion = *work_offset;
        out.units_before_motion = *units;
        out.absolute_at_first_move = s.absolute;
        *moved = true;
    }
    let from = s.pos;
    let mut to = from;
    for (i, t) in target.iter().enumerate() {
        if let (Some(v), Some(slot)) = (t, to.get_mut(i)) {
            let base = from.get(i).copied().unwrap_or(0.0);
            *slot = if s.absolute || machine { *v } else { base + v };
        }
    }
    let motion = if machine { Motion::Rapid } else { s.motion };
    if !machine {
        s.pos = to;
    }
    out.steps.push(Step::Move(Move {
        line: first.map_or(block.line, |w| w.line),
        sequence: block
            .words
            .iter()
            .find(|w| w.letter == 'N' && w.value >= 0.0 && w.value.fract() == 0.0)
            .map(|w| w.value as usize),
        motion,
        from,
        to,
        feed: s.feed,
        feed_in_block,
        inverse_time: s.inverse,
        machine,
        tool: s.tool,
        spindle_on: s.spindle_on,
        metric: s.metric,
    }));
    Ok(())
}
