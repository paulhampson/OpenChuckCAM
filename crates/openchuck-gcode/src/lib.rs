// SPDX-License-Identifier: AGPL-3.0-or-later
//! G-code reader and interpreter: parses G-code with A words back into motion for replay and checks.
//!
//! Architecture section 5.3 and 9; requirements GC-08, SIM-09.
//!
//! The in-house reader ([`read`], decision record 0005), the modal replay
//! ([`replay`]), the static checks ([`static_check`]), the comparison with the
//! toolpath IR ([`compare`]) and with a simulator's report
//! ([`compare_simulator`]).

#![warn(missing_docs)]

mod check;
mod compare;
mod interp;
mod reader;
mod sim;

pub use check::{CheckConfig, FeedLimits, Finding, FindingKind, static_check};
pub use compare::{CompareError, Comparison, TimingRule, Tolerances, compare};
pub use interp::{
    Axes, Dialect, Event, KNOWN_G, KNOWN_M, Motion, Move, Replay, ReplayError, Step, Z1_G, replay,
    replay_axis,
};
pub use reader::{Block, MAX_LINE_CHARS, Program, ReadError, Word, read};
pub use sim::{SimComparison, SimError, SimReport, SimRow, compare_simulator, parse_sim_report};
