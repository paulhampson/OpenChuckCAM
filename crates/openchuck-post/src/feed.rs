// SPDX-License-Identifier: AGPL-3.0-or-later
//! Per-controller feed rules (OP-13, GC-04; decision records 0007 item 3 and
//! 0019 item 5).
//!
//! The IR keeps the cutter-contact feed and each move's duration. A rule turns
//! those into the F word a controller needs so that the move takes the IR's
//! time on that controller. The rules are a small closed set selected by name
//! in the post definition and computed here, in Rust, so a template cannot
//! silently change a safety-relevant formula.
//!
//! Lengths are in millimetres, angles in degrees, durations in seconds.
//! Feeds are in units per minute, so `F = amount * 60 / duration`.

use serde::{Deserialize, Serialize};

/// What a controller's feed word means for a move.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", content = "value", rename_all = "snake_case")]
pub enum FeedWord {
    /// Units per minute (G94): mm/min, or degrees/min for an A-only move.
    PerMinute(f64),
    /// Inverse time (G93): the reciprocal of the move time in minutes.
    InverseTime(f64),
}

/// The named per-controller rule that turns duration into a feed word.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedRule {
    /// LinuxCNC (G94): F applies to the XYZ length of a move that has any, and
    /// to the A travel (degrees per minute) of an A-only move.
    Linuxcnc,
    /// Makera Z1 firmware (G94; decision 0007): a move with XYZ and A takes
    /// `max(L, dA * P / 360) / F`, an A-only move `dA * min(P, 360) / 360 / F`,
    /// with `P = 2 pi r + 30 mm` and `r` the end point's distance from the
    /// rotary axis. Derived from the firmware source; unconfirmed on a Z1
    /// until the air cut (OI-37).
    Makera,
    /// Inverse time (G93): F is `60 / duration`, for controllers that have it.
    InverseTime,
    /// The IR's contact feed, unchanged. Wrong on rotary moves (SP-3 measured
    /// 37 % too fast); kept as the baseline the tests compare against.
    Contact,
}

/// The measured facts of one move that a rule needs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MoveMetrics {
    /// Length of the straight XYZ motion in the work frame, mm.
    pub length: f64,
    /// Absolute change of A, degrees.
    pub delta_a: f64,
    /// Distance of the end point from the rotary axis, mm.
    pub end_radius: f64,
    /// The IR's contact feed, mm/min.
    pub contact_feed: f64,
    /// The IR's duration for the move, seconds. Greater than zero.
    pub duration: f64,
}

/// Motion shorter than this (mm or degrees) counts as none.
pub const NO_MOTION: f64 = 1e-9;

impl FeedRule {
    /// The rule's name as written in a post definition.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Linuxcnc => "linuxcnc",
            Self::Makera => "makera",
            Self::InverseTime => "inverse_time",
            Self::Contact => "contact",
        }
    }

    /// Parses a name from a post definition.
    pub fn parse(text: &str) -> Option<Self> {
        [
            Self::Linuxcnc,
            Self::Makera,
            Self::InverseTime,
            Self::Contact,
        ]
        .into_iter()
        .find(|rule| rule.name() == text)
    }

    /// The feed word for a move, or `None` when the move has no motion.
    pub fn feed(self, m: &MoveMetrics) -> Option<FeedWord> {
        let moves_xyz = m.length > NO_MOTION;
        let moves_a = m.delta_a > NO_MOTION;
        if !moves_xyz && !moves_a {
            return None;
        }
        let per_minute = |amount: f64| FeedWord::PerMinute(amount * 60.0 / m.duration);
        Some(match self {
            Self::Contact => FeedWord::PerMinute(m.contact_feed),
            Self::InverseTime => FeedWord::InverseTime(60.0 / m.duration),
            Self::Linuxcnc => per_minute(if moves_xyz { m.length } else { m.delta_a }),
            Self::Makera => {
                // The firmware counts a radius of 1 mm or less as 1 mm (decision 0007).
                let p = 2.0 * std::f64::consts::PI * m.end_radius.max(1.0) + 30.0;
                per_minute(if moves_xyz {
                    m.length.max(m.delta_a * p / 360.0)
                } else {
                    m.delta_a * p.min(360.0) / 360.0
                })
            }
        })
    }
}
