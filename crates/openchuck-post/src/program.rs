// SPDX-License-Identifier: AGPL-3.0-or-later
//! The neutral machine program (architecture section 5.3, work breakdown
//! slice 6 item 1).
//!
//! The IR turned into machine terms: positions in the work frame with the
//! A angle, feed words per the post's feed rule, spindle, tool change and
//! comments. It names no dialect; a post definition renders it to text.

use serde::{Deserialize, Serialize};

use crate::feed::FeedWord;
use openchuck_types::ir::MoveKind;

/// A position in the work frame: linear axes in millimetres, A in degrees
/// (unwrapped).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Target {
    /// X, mm.
    pub x: f64,
    /// Y, mm.
    pub y: f64,
    /// Z, mm.
    pub z: f64,
    /// A, degrees.
    pub a: f64,
}

/// One step of the program.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "step", rename_all = "snake_case")]
pub enum Step {
    /// A comment line (the renderer makes it safe for its dialect).
    Comment {
        /// The text.
        text: String,
    },
    /// Select a tool.
    ToolChange {
        /// The tool number the program uses (GC-14).
        number: u32,
        /// The tool's name, for comments.
        name: String,
    },
    /// Start the spindle.
    SpindleOn {
        /// Speed, revolutions per minute.
        rpm: f64,
    },
    /// Rapid traverse to a position.
    Rapid {
        /// Where to.
        target: Target,
    },
    /// A move at feed.
    Feed {
        /// What the move is for (cut, link, plunge, lead-in, lead-out).
        kind: MoveKind,
        /// Where to.
        target: Target,
        /// The feed word under the post's feed rule.
        feed: FeedWord,
    },
    /// Safe operation boundary on a continuous axis. A supporting post may
    /// rebase the live rotary counter by whole turns here, without motion;
    /// other posts omit it. Never substitute a physical return (decision 0238).
    RotaryBoundary {
        /// The physical position in the unwrapped program. During rendering,
        /// `a` becomes the controller's counter after the non-moving reset.
        at: Target,
    },
}

/// The program for a whole toolpath, in machining order.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct MachineProgram {
    /// The steps.
    pub steps: Vec<Step>,
    /// Fed moves whose feed was lowered to the post's maximum.
    pub clamped_feeds: usize,
}
