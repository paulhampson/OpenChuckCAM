// SPDX-License-Identifier: AGPL-3.0-or-later
//! The machine profile as data (SET-01, SET-06, architecture section 5.1).
//!
//! Rotary conventions are profile data and are never hard-coded in the engine
//! or the post (decision record 0001, item 2). Each value can record where it
//! came from and whether it is confirmed on the machine ([`Provenance`]); the
//! Makera Z1 profile in `openchuck-project` marks every value (decision record
//! 0011). [`MachineProfile::example_x_rotary`] is an example for tests and
//! documentation, not a real machine.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result, require_finite, require_positive};
use crate::geometry::{Aabb, Point3};
use crate::units::{Degrees, FeedRate, Millimetres, RotaryFeedRate, SpindleSpeed};

/// The machine axis the rotary axis is parallel to.
///
/// The first release machines about X only. Y (SET-02, P2) exists so the
/// shared kinematics can be tested with it, proving X is a parameter and never
/// assumed; [`MachineProfile::validate`] refuses it until SET-02 is built
/// (decision record 0011).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum RotaryAxis {
    /// The rotary axis is parallel to machine X.
    X,
    /// The rotary axis is parallel to machine Y (SET-02, not available yet).
    Y,
}

impl RotaryAxis {
    /// The axis letter of the machine axis it is parallel to.
    pub const fn name(self) -> &'static str {
        match self {
            Self::X => "X",
            Self::Y => "Y",
        }
    }
}

/// Which way the part turns for a positive A value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ASense {
    /// A positive A turns the part counter-clockwise about the positive rotary
    /// axis direction, seen in the machine frame (right-hand rule).
    Positive,
    /// A positive A turns the part clockwise about the positive rotary axis direction.
    Negative,
}

impl ASense {
    /// +1 for [`ASense::Positive`], -1 for [`ASense::Negative`].
    pub const fn sign(self) -> f64 {
        match self {
            Self::Positive => 1.0,
            Self::Negative => -1.0,
        }
    }

    /// The opposite sense.
    pub const fn flipped(self) -> Self {
        match self {
            Self::Positive => Self::Negative,
            Self::Negative => Self::Positive,
        }
    }
}

/// The allowed travel of the rotary axis.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ARange {
    /// Unlimited continuous rotation.
    Continuous,
    /// Travel limited to `min..=max` (unwrapped degrees).
    Limited {
        /// Lowest A.
        min: Degrees,
        /// Highest A.
        max: Degrees,
    },
}

/// Highest feed rates the machine accepts.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaxFeeds {
    /// The X and Y axes.
    pub linear: FeedRate,
    /// The Z axis, which many machines limit below X and Y (OI-59).
    pub z: FeedRate,
    /// Rotary axis.
    pub rotary: RotaryFeedRate,
    /// Rapid traverse on X and Y.
    pub rapid_linear: FeedRate,
    /// Rapid traverse on Z.
    pub rapid_z: FeedRate,
    /// Rapid traverse on the rotary axis.
    pub rapid_rotary: RotaryFeedRate,
    /// How the controller counts A in the feed word of a move that also moves
    /// X, Y or Z, which the linear (or Z) limit then applies to (decision
    /// record 0076). Written only when not [`RotaryInFeed::Separate`].
    #[serde(default, skip_serializing_if = "RotaryInFeed::is_separate")]
    pub rotary_in_feed: RotaryInFeed,
}

/// How a controller counts the rotary axis in the feed word of a move that
/// also moves the linear axes (decision record 0076). The post's feed rule
/// turns a move's time into the F word (decision record 0007 item 3); this
/// says which length the machine's feed limit applies to, so path assembly
/// can time every fed move for it while the IR stays the same for every
/// post.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum RotaryInFeed {
    /// The feed word is the speed of the XYZ line alone; A has only its own
    /// limit (LinuxCNC).
    #[default]
    Separate,
    /// The Makera Z1 firmware (decision record 0007): the feed word is the
    /// speed of `max(L, dA * P / 360)`, with `L` the XYZ line, `dA` the
    /// change of A in degrees and `P = 2 pi r + 30` mm, `r` the end point's
    /// distance from the rotary axis, counted as 1 mm when less.
    Makera,
}

impl RotaryInFeed {
    /// Whether this is [`RotaryInFeed::Separate`], the default.
    pub fn is_separate(&self) -> bool {
        *self == Self::Separate
    }
}

/// Geometry the tool retracts to between cuts (SET-06, OP-14).
///
/// Both values are measured from the rotary axis, so they hold at any A: a
/// safe cylinder of `radius` about the axis that retracts and rapid links
/// stay outside of, and a safe Z, the height above the axis that the tool
/// goes to before a large rotation, a tool change and at the start and end of
/// a program. The safe Z is never below the cylinder (decision record 0011).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Clearance {
    /// Radius of the safe cylinder about the rotary axis.
    pub radius: Millimetres,
    /// Safe height above the rotary axis.
    pub safe_z: Millimetres,
}

impl Clearance {
    /// Fails unless both values are positive and the safe Z is not below the
    /// safe cylinder.
    pub fn validate(&self, field: &str) -> Result<()> {
        require_positive(&format!("{field}.radius"), self.radius.0)?;
        require_positive(&format!("{field}.safe_z"), self.safe_z.0)?;
        if self.safe_z.0 < self.radius.0 {
            return Err(Error::invalid(
                format!("{field}.safe_z"),
                format!(
                    "the safe Z ({}) must not be below the safe cylinder (radius {})",
                    self.safe_z, self.radius
                ),
            ));
        }
        Ok(())
    }
}

/// A field of the machine profile, for recording where its value came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileField {
    /// [`MachineProfile::rotary_axis`].
    RotaryAxis,
    /// [`MachineProfile::a_sense`].
    ASense,
    /// [`MachineProfile::rotary_origin`].
    RotaryOrigin,
    /// [`MachineProfile::a_range`].
    ARange,
    /// [`MachineProfile::travel`].
    Travel,
    /// [`MachineProfile::max_feeds`].
    MaxFeeds,
    /// [`MachineProfile::clearance`].
    Clearance,
    /// [`MachineProfile::min_spindle_rpm`] and
    /// [`MachineProfile::max_spindle_rpm`].
    SpindleRange,
}

impl ProfileField {
    /// Every field, in order.
    pub const ALL: [Self; 8] = [
        Self::RotaryAxis,
        Self::ASense,
        Self::RotaryOrigin,
        Self::ARange,
        Self::Travel,
        Self::MaxFeeds,
        Self::Clearance,
        Self::SpindleRange,
    ];

    /// The field's name in the profile file.
    pub const fn name(self) -> &'static str {
        match self {
            Self::RotaryAxis => "rotary_axis",
            Self::ASense => "a_sense",
            Self::RotaryOrigin => "rotary_origin",
            Self::ARange => "a_range",
            Self::Travel => "travel",
            Self::MaxFeeds => "max_feeds",
            Self::Clearance => "clearance",
            Self::SpindleRange => "spindle_range",
        }
    }
}

impl std::fmt::Display for ProfileField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// Whether a profile value has been checked on the machine itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confirmation {
    /// Checked on the machine.
    Confirmed,
    /// Taken from documents, other software or source code, or a
    /// placeholder; not yet checked on the machine.
    Unconfirmed,
}

/// Where a profile value came from (work breakdown slice 3 item 2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provenance {
    /// Checked on the machine or not.
    pub state: Confirmation,
    /// The source, in words.
    pub source: String,
}

/// The tool number limit profiles get when they do not say: 1000. The Z1's
/// real limit is not known (the number range is effectively unlimited; the
/// probe uses a high number), and the owner chose 1000 as reasonable for now
/// (13 Oct 2026, OI-62).
pub const DEFAULT_MAX_TOOL_NUMBER: u32 = 1000;

const fn default_max_tool_number() -> u32 {
    DEFAULT_MAX_TOOL_NUMBER
}

/// Which end of a machine's spindle speed range a speed is outside, with
/// that end (decision record 0231).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpindleLimit {
    /// Faster than the machine's highest speed.
    Above(SpindleSpeed),
    /// Slower than the machine's lowest speed.
    Below(SpindleSpeed),
}

/// A machine profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MachineProfile {
    /// Display name.
    pub name: String,
    /// Axis the rotary axis is parallel to.
    pub rotary_axis: RotaryAxis,
    /// Direction of positive A.
    pub a_sense: ASense,
    /// Machine coordinates of the rotary reference point: a point on the
    /// rotary axis, such as the centre of the chuck face. Across the axis it
    /// is the rotary axis offset (decision record 0001, item 2); along the
    /// axis it is where a setup's part offset is measured from (SET-05,
    /// decision record 0011).
    pub rotary_origin: Point3,
    /// Allowed A travel.
    pub a_range: ARange,
    /// Travel limits of the linear axes, in machine coordinates.
    pub travel: Aabb,
    /// Highest feed rates.
    pub max_feeds: MaxFeeds,
    /// The default clearance geometry; a setup may set its own (SET-06).
    pub clearance: Clearance,
    /// The highest tool number the controller accepts (GC-14, OI-62). The
    /// lowest is 1. Not tied to a measured value: see
    /// [`DEFAULT_MAX_TOOL_NUMBER`].
    #[serde(default = "default_max_tool_number")]
    pub max_tool_number: u32,
    /// The lowest spindle speed the machine runs at, when the profile gives
    /// one. A limit, never a default (decision records 0196 and 0231).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_spindle_rpm: Option<SpindleSpeed>,
    /// The highest spindle speed the machine runs at, when the profile gives
    /// one. A limit, never a default (decision records 0196 and 0231).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_spindle_rpm: Option<SpindleSpeed>,
    /// Where each value came from, and whether it is confirmed on the
    /// machine. A field not listed is treated as unconfirmed.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub provenance: BTreeMap<ProfileField, Provenance>,
}

impl MachineProfile {
    /// Fails if any value is not finite or out of range, or the rotary axis
    /// is one this release does not support.
    pub fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty() {
            return Err(Error::invalid("machine.name", "the name must not be empty"));
        }
        if self.max_tool_number < 1 {
            return Err(Error::invalid(
                "machine.max_tool_number",
                "the highest tool number must be at least 1",
            ));
        }
        if self.rotary_axis != RotaryAxis::X {
            return Err(Error::invalid(
                "machine.rotary_axis",
                format!(
                    "a rotary axis about {} is not supported yet (SET-02); use x",
                    self.rotary_axis.name()
                ),
            ));
        }
        self.rotary_origin.validate("machine.rotary_origin")?;
        if let ARange::Limited { min, max } = self.a_range {
            require_finite("machine.a_range.min", min.0)?;
            require_finite("machine.a_range.max", max.0)?;
            if min.0 >= max.0 {
                return Err(Error::invalid(
                    "machine.a_range",
                    "min must be less than max",
                ));
            }
        }
        self.travel.validate("machine.travel")?;
        require_positive("machine.max_feeds.linear", self.max_feeds.linear.0)?;
        require_positive("machine.max_feeds.z", self.max_feeds.z.0)?;
        require_positive("machine.max_feeds.rotary", self.max_feeds.rotary.0)?;
        require_positive(
            "machine.max_feeds.rapid_linear",
            self.max_feeds.rapid_linear.0,
        )?;
        require_positive("machine.max_feeds.rapid_z", self.max_feeds.rapid_z.0)?;
        require_positive(
            "machine.max_feeds.rapid_rotary",
            self.max_feeds.rapid_rotary.0,
        )?;
        self.clearance.validate("machine.clearance")?;
        for (field, value) in [
            ("machine.min_spindle_rpm", self.min_spindle_rpm),
            ("machine.max_spindle_rpm", self.max_spindle_rpm),
        ] {
            if let Some(v) = value {
                require_positive(field, v.0)?;
            }
        }
        if let (Some(min), Some(max)) = (self.min_spindle_rpm, self.max_spindle_rpm)
            && min.0 >= max.0
        {
            return Err(Error::invalid(
                "machine.min_spindle_rpm",
                format!("the lowest spindle speed ({min}) must be below the highest ({max})"),
            ));
        }
        for (field, provenance) in &self.provenance {
            if provenance.source.trim().is_empty() {
                return Err(Error::invalid(
                    format!("machine.provenance.{field}.source"),
                    "say where the value came from",
                ));
            }
        }
        Ok(())
    }

    /// Whether the value of `field` is confirmed on the machine.
    pub fn is_confirmed(&self, field: ProfileField) -> bool {
        self.provenance
            .get(&field)
            .is_some_and(|p| p.state == Confirmation::Confirmed)
    }

    /// The fields not confirmed on the machine, in order. The spindle range
    /// is listed only when the profile gives one.
    pub fn unconfirmed_fields(&self) -> Vec<ProfileField> {
        let spindle = self.min_spindle_rpm.is_some() || self.max_spindle_rpm.is_some();
        ProfileField::ALL
            .into_iter()
            .filter(|f| *f != ProfileField::SpindleRange || spindle)
            .filter(|f| !self.is_confirmed(*f))
            .collect()
    }

    /// Whether `speed` is outside the profile's spindle speed range, and
    /// which end; `None` inside it or when the profile gives none.
    pub fn spindle_limit(&self, speed: SpindleSpeed) -> Option<SpindleLimit> {
        if let Some(max) = self.max_spindle_rpm
            && speed.0 > max.0
        {
            return Some(SpindleLimit::Above(max));
        }
        if let Some(min) = self.min_spindle_rpm
            && speed.0 < min.0
        {
            return Some(SpindleLimit::Below(min));
        }
        None
    }

    /// An example profile with the rotary axis along X, offset from the
    /// machine origin. For tests and documentation only: its values describe
    /// no real machine.
    pub fn example_x_rotary() -> Self {
        Self {
            name: "Example X rotary".to_owned(),
            rotary_axis: RotaryAxis::X,
            a_sense: ASense::Positive,
            rotary_origin: Point3::new(0.0, 12.5, -40.0),
            a_range: ARange::Continuous,
            travel: Aabb {
                min: Point3::new(-200.0, -100.0, -100.0),
                max: Point3::new(200.0, 100.0, 50.0),
            },
            max_feeds: MaxFeeds {
                linear: FeedRate(3000.0),
                z: FeedRate(2000.0),
                rotary: RotaryFeedRate(7200.0),
                rapid_linear: FeedRate(6000.0),
                rapid_z: FeedRate(4000.0),
                rapid_rotary: RotaryFeedRate(14400.0),
                rotary_in_feed: RotaryInFeed::Separate,
            },
            max_tool_number: DEFAULT_MAX_TOOL_NUMBER,
            min_spindle_rpm: None,
            max_spindle_rpm: None,
            clearance: Clearance {
                radius: Millimetres(40.0),
                safe_z: Millimetres(50.0),
            },
            provenance: BTreeMap::new(),
        }
    }
}
