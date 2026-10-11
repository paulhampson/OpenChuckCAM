// SPDX-License-Identifier: AGPL-3.0-or-later
//! Model placement as plain data (IMP-05).
//!
//! A model is placed in the model frame by an ordered list of
//! [`TransformStep`]s, applied first to last to the imported geometry. The
//! model frame's X axis is the rotary axis and its origin lies on it (decision
//! record 0004 item 5), so orienting a part to the rotary axis is a rotation
//! that brings the part's own axis onto X followed by
//! [`TransformStep::CentreOnRotaryAxis`].
//!
//! The steps are stored in the project (one line per value, so an edit is a
//! small diff) and evaluated by `openchuck-geom`. Rotations, mirrors and
//! scales act about the model origin.

use serde::{Deserialize, Serialize};

use crate::error::{Result, require_finite, require_positive};
use crate::units::Degrees;

/// A coordinate axis of the model frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Axis {
    /// X, the rotary axis.
    X,
    /// Y.
    Y,
    /// Z.
    Z,
}

impl Axis {
    /// The axis's lower-case letter.
    pub const fn letter(self) -> &'static str {
        match self {
            Self::X => "x",
            Self::Y => "y",
            Self::Z => "z",
        }
    }

    /// Reads `x`, `y` or `z`, ignoring case.
    pub fn parse(text: &str) -> Option<Self> {
        match text.trim().to_ascii_lowercase().as_str() {
            "x" => Some(Self::X),
            "y" => Some(Self::Y),
            "z" => Some(Self::Z),
            _ => None,
        }
    }
}

impl std::fmt::Display for Axis {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.letter())
    }
}

/// One step of a model's placement.
///
/// Written in a project file as a table with a `kind`, for example
/// `kind = "rotate"`, `axis = "z"`, `angle = 90.0`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TransformStep {
    /// Moves the model by a displacement in millimetres.
    Translate {
        /// X displacement.
        x: f64,
        /// Y displacement.
        y: f64,
        /// Z displacement.
        z: f64,
    },
    /// Rotates the model about an axis through the origin, right-handed: a
    /// positive angle turns anticlockwise when looking from the positive end of
    /// the axis towards the origin.
    Rotate {
        /// The axis.
        axis: Axis,
        /// The angle.
        angle: Degrees,
    },
    /// Scales the model uniformly about the origin.
    Scale {
        /// The factor, greater than zero. Use [`TransformStep::Mirror`] to reflect.
        factor: f64,
    },
    /// Reflects the model in the plane through the origin normal to `axis`,
    /// negating that coordinate. Triangle winding is reversed with it, so
    /// normals stay outward.
    Mirror {
        /// The coordinate that is negated.
        axis: Axis,
    },
    /// Moves the model across the rotary axis so the centre of its bounding
    /// box lies on it (Y and Z zero). Its position along the axis is kept.
    CentreOnRotaryAxis,
}

impl TransformStep {
    /// Fails unless every value is finite and a scale factor is above zero.
    pub fn validate(&self, field: &str) -> Result<()> {
        match *self {
            Self::Translate { x, y, z } => {
                require_finite(&format!("{field}.x"), x)?;
                require_finite(&format!("{field}.y"), y)?;
                require_finite(&format!("{field}.z"), z)
            }
            Self::Rotate { angle, .. } => require_finite(&format!("{field}.angle"), angle.0),
            Self::Scale { factor } => require_positive(&format!("{field}.factor"), factor),
            Self::Mirror { .. } | Self::CentreOnRotaryAxis => Ok(()),
        }
    }

    /// Validates every step of a placement, naming the step that fails.
    pub fn validate_all(steps: &[Self], field: &str) -> Result<()> {
        steps
            .iter()
            .enumerate()
            .try_for_each(|(i, step)| step.validate(&format!("{field}[{i}]")))
    }
}

impl std::fmt::Display for TransformStep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Translate { x, y, z } => write!(f, "translate by ({x}, {y}, {z}) mm"),
            Self::Rotate { axis, angle } => write!(f, "rotate {} deg about {axis}", angle.0),
            Self::Scale { factor } => write!(f, "scale by {factor}"),
            Self::Mirror { axis } => write!(f, "mirror {axis}"),
            Self::CentreOnRotaryAxis => f.write_str("centre on the rotary axis"),
        }
    }
}
