// SPDX-License-Identifier: AGPL-3.0-or-later
//! The machine profile as data (SET-01, SET-06, architecture section 5.1).
//!
//! Rotary conventions are profile data and are never hard-coded in the engine
//! or the post (decision record 0001, item 2). Values for the Makera Z1 are
//! not confirmed until spike SP-12 and slice 3; [`MachineProfile::example_x_rotary`]
//! is an example for tests and documentation, not a real machine.

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result, require_finite, require_positive};
use crate::geometry::{Aabb, Point3};
use crate::units::{Degrees, FeedRate, Millimetres, RotaryFeedRate};

/// The machine axis the rotary axis is parallel to.
///
/// Only X exists in the first release; Y follows under SET-02 as a profile
/// change. Strategy maths takes this as a parameter and never assumes X.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum RotaryAxis {
    /// The rotary axis is parallel to machine X.
    X,
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
    /// Linear axes.
    pub linear: FeedRate,
    /// Rotary axis.
    pub rotary: RotaryFeedRate,
    /// Rapid traverse on the linear axes.
    pub rapid_linear: FeedRate,
    /// Rapid traverse on the rotary axis.
    pub rapid_rotary: RotaryFeedRate,
}

/// Geometry the tool retracts to between cuts (OP-14).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
#[non_exhaustive]
pub enum Clearance {
    /// A cylinder about the rotary axis.
    Cylinder {
        /// Radius from the rotary axis.
        radius: Millimetres,
    },
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
    /// Machine coordinates of the model origin at A = 0. The model origin lies
    /// on the rotary axis, so for rotation about X the Y and Z of this point
    /// are the rotary axis offset (decision record 0001, item 2).
    pub rotary_origin: Point3,
    /// Allowed A travel.
    pub a_range: ARange,
    /// Travel limits of the linear axes, in machine coordinates.
    pub travel: Aabb,
    /// Highest feed rates.
    pub max_feeds: MaxFeeds,
    /// Retract geometry.
    pub clearance: Clearance,
}

impl MachineProfile {
    /// Fails if any value is not finite or out of range.
    pub fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty() {
            return Err(Error::invalid("machine.name", "the name must not be empty"));
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
        require_positive("machine.max_feeds.rotary", self.max_feeds.rotary.0)?;
        require_positive(
            "machine.max_feeds.rapid_linear",
            self.max_feeds.rapid_linear.0,
        )?;
        require_positive(
            "machine.max_feeds.rapid_rotary",
            self.max_feeds.rapid_rotary.0,
        )?;
        match self.clearance {
            Clearance::Cylinder { radius } => {
                require_positive("machine.clearance.radius", radius.0)
            }
        }
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
                rotary: RotaryFeedRate(7200.0),
                rapid_linear: FeedRate(6000.0),
                rapid_rotary: RotaryFeedRate(14400.0),
            },
            clearance: Clearance::Cylinder {
                radius: Millimetres(40.0),
            },
        }
    }
}
