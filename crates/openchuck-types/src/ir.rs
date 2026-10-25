// SPDX-License-Identifier: AGPL-3.0-or-later
//! The neutral toolpath IR (GC-01, architecture section 5.1).
//!
//! An ordered list of segments grouped by operation, setup and tool. Each
//! segment carries, in the **model frame** (decision record 0001, item 1):
//!
//! - the tool tip and the unit tool axis at the end of the move,
//! - the explicit, **unwrapped** A angle in degrees,
//! - the move kind,
//! - the cutter-contact feed and the computed segment duration,
//! - the contact path length,
//!
//! and, through its group, tags back to the operation, setup and tool. The IR
//! names no controller, axis letter or dialect: machine XYZ is derived only in
//! the post, from the machine profile. Every segment is physical motion,
//! including a whole-turn rapid. Non-moving controller counter resets are
//! post actions and consume no segment or duration (decision 0241).
//!
//! The post chooses the feed mode per move from the contact feed and duration
//! (OP-13, GC-04).

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result, require_finite, require_non_negative, require_positive};
use crate::geometry::{Point3, UnitVec3};
use crate::units::{Degrees, FeedRate, Millimetres, Seconds};

/// What a move is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MoveKind {
    /// Rapid traverse, not cutting.
    Rapid,
    /// A linking move between passes at feed.
    Linking,
    /// A cutting move.
    Cutting,
    /// A plunge into the material.
    Plunge,
    /// A lead-in to a cut.
    LeadIn,
    /// A lead-out from a cut.
    LeadOut,
}

impl MoveKind {
    /// True for the kinds that move at a programmed feed (all but rapids).
    pub const fn is_fed(self) -> bool {
        !matches!(self, Self::Rapid)
    }
}

/// Tool tip, tool axis and rotary angle, all in the model frame.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolPose {
    /// Tool tip position in the model frame.
    pub tip: Point3,
    /// Unit tool axis in the model frame, pointing from the tip towards the spindle.
    pub axis: UnitVec3,
    /// Rotary angle, unwrapped (it may pass 360 or go below 0).
    pub a: Degrees,
}

/// One move, from the end of the previous segment (or the group start) to `end`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Segment {
    /// What the move is for.
    pub kind: MoveKind,
    /// The pose at the end of the move.
    pub end: ToolPose,
    /// Feed at the cutter contact point. Required for fed moves, absent for rapids.
    pub contact_feed: Option<FeedRate>,
    /// The computed time the move takes.
    pub duration: Seconds,
    /// Length of the path traced by the cutter contact point.
    pub contact_length: Millimetres,
}

/// Opaque identifiers tying a group back to the project. The libraries do not
/// interpret them; the application maps them to its stable IDs.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PathTags {
    /// The operation that produced the moves.
    pub operation: String,
    /// The setup the operation belongs to.
    pub setup: String,
    /// The tool that cuts them.
    pub tool: String,
}

/// Consecutive moves of one operation, setup and tool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolpathGroup {
    /// Where the moves came from.
    pub tags: PathTags,
    /// The pose before the first move.
    pub start: ToolPose,
    /// The moves, in order.
    pub segments: Vec<Segment>,
}

/// A complete toolpath: groups in machining order.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Toolpath {
    /// Groups in machining order.
    pub groups: Vec<ToolpathGroup>,
}

impl ToolPose {
    fn validate(&self, field: &str) -> Result<()> {
        self.tip.validate(&format!("{field}.tip"))?;
        require_finite(&format!("{field}.a"), self.a.0)
    }
}

impl Segment {
    /// Fails if a value is not finite or out of range, or the feed does not
    /// match the move kind. `field` prefixes the error path.
    pub fn validate(&self, field: &str) -> Result<()> {
        self.end.validate(&format!("{field}.end"))?;
        require_non_negative(&format!("{field}.duration"), self.duration.0)?;
        require_non_negative(&format!("{field}.contact_length"), self.contact_length.0)?;
        match (self.kind.is_fed(), self.contact_feed) {
            (true, Some(feed)) => require_positive(&format!("{field}.contact_feed"), feed.0),
            (true, None) => Err(Error::invalid(
                format!("{field}.contact_feed"),
                "a fed move needs a contact feed",
            )),
            (false, Some(_)) => Err(Error::invalid(
                format!("{field}.contact_feed"),
                "a rapid move must not carry a contact feed",
            )),
            (false, None) => Ok(()),
        }
    }
}

impl Toolpath {
    /// Checks every group and segment, and fails on the first problem found.
    pub fn validate(&self) -> Result<()> {
        for (g, group) in self.groups.iter().enumerate() {
            group.start.validate(&format!("groups[{g}].start"))?;
            for (s, segment) in group.segments.iter().enumerate() {
                segment.validate(&format!("groups[{g}].segments[{s}]"))?;
            }
        }
        Ok(())
    }

    /// Every segment in machining order, with its tags.
    pub fn segments(&self) -> impl Iterator<Item = (&PathTags, &Segment)> {
        self.groups
            .iter()
            .flat_map(|group| group.segments.iter().map(move |s| (&group.tags, s)))
    }

    /// Sum of segment durations, added in machining order.
    pub fn total_duration(&self) -> Seconds {
        Seconds(self.segments().map(|(_, s)| s.duration.0).sum())
    }
}
