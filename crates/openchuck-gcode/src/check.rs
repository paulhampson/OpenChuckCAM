// SPDX-License-Identifier: AGPL-3.0-or-later
//! Gate 1: static checks on replayed G-code (verification plan section 6;
//! work breakdown slice 6 item 11; GC-08).
//!
//! Syntax is the reader's. These checks work on the replay: units, distance
//! mode and work offset before the first move, a feed on every cutting move,
//! the spindle on while cutting, axis limits, a safe tool change and a program
//! end. Limits are in the coordinates the program is written in.

use crate::interp::{Axes, Event, Motion, Replay, Step};

/// The highest F allowed for each kind of move, in units per minute.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FeedLimits {
    /// A move that changes X or Y (mm/min).
    pub linear: f64,
    /// A move that changes only Z, or Z and A (mm/min).
    pub z: f64,
    /// A move that changes only A (degrees/min).
    pub rotary: f64,
}

impl FeedLimits {
    /// The limit that applies to a move, by the axes it changes.
    fn for_move(&self, m: &crate::interp::Move) -> f64 {
        let moved = |i: usize| {
            (m.to.get(i).copied().unwrap_or(0.0) - m.from.get(i).copied().unwrap_or(0.0)).abs()
                > 1e-9
        };
        if moved(0) || moved(1) {
            self.linear
        } else if moved(2) {
            self.z
        } else {
            self.rotary
        }
    }
}

/// Limits and rules a program is checked against.
#[derive(Debug, Clone, PartialEq)]
pub struct CheckConfig {
    /// Allowed range of X, Y and Z in program coordinates (min, max).
    pub travel: [(f64, f64); 3],
    /// Allowed range of A, or `None` for continuous.
    pub a_range: Option<(f64, f64)>,
    /// The highest F allowed, in units per minute (G94 moves only).
    pub max_feed: FeedLimits,
    /// A tool change happens at or above this Z.
    pub safe_z: f64,
    /// The spindle must be off at a tool change.
    pub spindle_off_at_tool_change: bool,
    /// Whether to require a work offset before the first move.
    pub require_work_offset: bool,
    /// Whether the program may use inverse-time feed (G93).
    pub allow_inverse_time: bool,
}

/// What a finding is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindingKind {
    /// Units are not set to millimetres before the first move.
    Units,
    /// Absolute mode was not in force at the first move.
    DistanceMode,
    /// No work offset before the first move.
    WorkOffset,
    /// A fed move has no feed word (or one that is not positive).
    MissingFeed,
    /// A feed is above the limit.
    FeedTooHigh,
    /// A position is outside an axis limit.
    AxisLimit,
    /// A fed move with the spindle off.
    SpindleOff,
    /// A tool change away from the safe Z, or with the spindle on.
    UnsafeToolChange,
    /// Inverse-time feed where it is not allowed.
    InverseTime,
    /// An arc, which the replay does not follow.
    Arc,
    /// The program has no end code.
    NoEnd,
}

/// One problem, with the line it is on.
#[derive(Debug, Clone, PartialEq)]
pub struct Finding {
    /// What it is about.
    pub kind: FindingKind,
    /// One-based source line (0 for the program as a whole).
    pub line: usize,
    /// A message for the user.
    pub message: String,
}

fn outside(range: (f64, f64), value: f64) -> bool {
    value < range.0 - 1e-9 || value > range.1 + 1e-9
}

fn axes_label(i: usize) -> &'static str {
    ["X", "Y", "Z", "A"].get(i).copied().unwrap_or("?")
}

fn check_axes(findings: &mut Vec<Finding>, line: usize, to: &Axes, config: &CheckConfig) {
    for (i, range) in config.travel.iter().enumerate() {
        let v = to.get(i).copied().unwrap_or(0.0);
        if outside(*range, v) {
            findings.push(Finding {
                kind: FindingKind::AxisLimit,
                line,
                message: format!(
                    "{} {v} is outside {} to {}",
                    axes_label(i),
                    range.0,
                    range.1
                ),
            });
        }
    }
    if let Some(range) = config.a_range {
        let a = to.get(3).copied().unwrap_or(0.0);
        if outside(range, a) {
            findings.push(Finding {
                kind: FindingKind::AxisLimit,
                line,
                message: format!("A {a} is outside {} to {}", range.0, range.1),
            });
        }
    }
}

/// Runs the static checks. An empty result means the program passed.
pub fn static_check(replay: &Replay, config: &CheckConfig) -> Vec<Finding> {
    let mut findings = Vec::new();
    let mut ended = false;
    let mut first_move = true;
    for step in &replay.steps {
        match step {
            Step::Move(m) => {
                if first_move {
                    first_move = false;
                    if !replay.units_before_motion || !m.metric {
                        findings.push(Finding {
                            kind: FindingKind::Units,
                            line: m.line,
                            message: "units are not set to millimetres (G21) before the first move"
                                .into(),
                        });
                    }
                    if !replay.absolute_at_first_move {
                        findings.push(Finding {
                            kind: FindingKind::DistanceMode,
                            line: m.line,
                            message: "absolute mode (G90) is not in force at the first move".into(),
                        });
                    }
                    if config.require_work_offset && !replay.work_offset_before_motion {
                        findings.push(Finding {
                            kind: FindingKind::WorkOffset,
                            line: m.line,
                            message:
                                "no work offset (G54 to G59) is selected before the first move"
                                    .into(),
                        });
                    }
                } else if !m.metric {
                    findings.push(Finding {
                        kind: FindingKind::Units,
                        line: m.line,
                        message: "the move is in inches (G20)".into(),
                    });
                }
                if matches!(m.motion, Motion::ArcCw | Motion::ArcCcw) {
                    findings.push(Finding {
                        kind: FindingKind::Arc,
                        line: m.line,
                        message: "an arc move is not followed by the replay".into(),
                    });
                    continue;
                }
                if m.machine {
                    continue;
                }
                check_axes(&mut findings, m.line, &m.to, config);
                if m.motion == Motion::Linear {
                    let feed_ok = m.feed.is_some_and(|f| f.is_finite() && f > 0.0);
                    if !feed_ok || (m.inverse_time && !m.feed_in_block) {
                        findings.push(Finding {
                            kind: FindingKind::MissingFeed,
                            line: m.line,
                            message: "a cutting move has no feed".into(),
                        });
                    }
                    if m.inverse_time && !config.allow_inverse_time {
                        findings.push(Finding {
                            kind: FindingKind::InverseTime,
                            line: m.line,
                            message: "inverse-time feed (G93) is not allowed for this machine"
                                .into(),
                        });
                    }
                    let limit = config.max_feed.for_move(m);
                    if !m.inverse_time && m.feed.is_some_and(|f| f > limit) {
                        findings.push(Finding {
                            kind: FindingKind::FeedTooHigh,
                            line: m.line,
                            message: format!(
                                "F{} is above the limit {}",
                                m.feed.unwrap_or(0.0),
                                limit
                            ),
                        });
                    }
                    if !m.spindle_on {
                        findings.push(Finding {
                            kind: FindingKind::SpindleOff,
                            line: m.line,
                            message: "a cutting move with the spindle off".into(),
                        });
                    }
                }
            }
            Step::Event(Event::ToolChange {
                line, at: Some(at), ..
            }) => {
                let z = at.get(2).copied().unwrap_or(f64::NEG_INFINITY);
                if z < config.safe_z - 1e-9 {
                    findings.push(Finding {
                        kind: FindingKind::UnsafeToolChange,
                        line: *line,
                        message: format!(
                            "tool change at Z {z} is below the safe Z {}",
                            config.safe_z
                        ),
                    });
                }
            }
            Step::Event(Event::ProgramEnd { .. }) => ended = true,
            Step::Event(_) => {}
        }
    }
    if config.spindle_off_at_tool_change {
        let mut spindle = false;
        for step in &replay.steps {
            match step {
                Step::Event(Event::SpindleOn { .. }) => spindle = true,
                Step::Event(Event::SpindleOff { .. }) => spindle = false,
                Step::Event(Event::ToolChange { line, .. }) if spindle => findings.push(Finding {
                    kind: FindingKind::UnsafeToolChange,
                    line: *line,
                    message: "tool change with the spindle still on".into(),
                }),
                _ => {}
            }
        }
    }
    if !ended {
        findings.push(Finding {
            kind: FindingKind::NoEnd,
            line: 0,
            message: "the program has no M2 or M30".into(),
        });
    }
    findings
}
