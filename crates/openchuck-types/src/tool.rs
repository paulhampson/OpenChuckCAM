// SPDX-License-Identifier: AGPL-3.0-or-later
//! Cutting tools (TL-02, TL-03).
//!
//! Every tool shape is described by one revolved profile of line and arc
//! segments, which path solving, the gouge checker and the simulation sweep
//! will share (architecture section 5.1). How the standard shapes map to that
//! profile is settled by spike SP-13 and slice 4; this module holds the data.

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result, require_finite, require_positive};
use crate::units::{Degrees, Millimetres};

/// A point on a tool profile: distance from the tool axis and height above the tip.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilePoint {
    /// Distance from the tool axis.
    pub radius: Millimetres,
    /// Height above the tool tip.
    pub height: Millimetres,
}

/// One segment of a revolved tool profile, running from the previous point.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProfileSegment {
    /// A straight line to `to`.
    Line {
        /// End point.
        to: ProfilePoint,
    },
    /// A circular arc to `to` about `centre`.
    Arc {
        /// End point.
        to: ProfilePoint,
        /// Arc centre.
        centre: ProfilePoint,
    },
}

/// A tool outline revolved about the tool axis, starting on the axis at the tip.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolProfile {
    /// Segments from the tip outwards and upwards.
    pub segments: Vec<ProfileSegment>,
}

/// The shape of the cutting end.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
#[non_exhaustive]
pub enum ToolShape {
    /// Flat end mill.
    FlatEnd,
    /// Ball end mill.
    BallEnd,
    /// Bull-nose end mill.
    BullNose {
        /// Corner radius.
        corner_radius: Millimetres,
    },
    /// Tapered ball end mill.
    TaperedBall {
        /// Radius of the ball at the tip.
        tip_radius: Millimetres,
        /// Half angle of the taper.
        taper_half_angle: Degrees,
    },
    /// V-bit engraver.
    VBit {
        /// Included angle of the V.
        included_angle: Degrees,
        /// Flat diameter at the tip (zero for a sharp point).
        tip_diameter: Millimetres,
    },
    /// Chamfer mill.
    Chamfer {
        /// Included angle.
        included_angle: Degrees,
        /// Flat diameter at the tip.
        tip_diameter: Millimetres,
    },
    /// Twist drill.
    Drill {
        /// Included point angle.
        point_angle: Degrees,
    },
    /// A custom revolved profile (TL-03).
    Custom {
        /// The profile.
        profile: ToolProfile,
    },
}

/// A cutting tool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tool {
    /// Shape of the cutting end.
    pub shape: ToolShape,
    /// Cutting diameter.
    pub diameter: Millimetres,
    /// Length of the fluted part.
    pub flute_length: Millimetres,
    /// Shank diameter.
    pub shank_diameter: Millimetres,
    /// Length from the holder face to the tip.
    pub stickout: Millimetres,
}

impl Tool {
    /// Fails if a dimension is not finite, not positive, or inconsistent.
    pub fn validate(&self) -> Result<()> {
        require_positive("tool.diameter", self.diameter.0)?;
        require_positive("tool.flute_length", self.flute_length.0)?;
        require_positive("tool.shank_diameter", self.shank_diameter.0)?;
        require_positive("tool.stickout", self.stickout.0)?;
        if self.flute_length.0 > self.stickout.0 {
            return Err(Error::invalid(
                "tool.flute_length",
                "the flute length must not exceed the stickout",
            ));
        }
        let radius = self.diameter.0 / 2.0;
        match &self.shape {
            ToolShape::FlatEnd | ToolShape::BallEnd => Ok(()),
            ToolShape::BullNose { corner_radius } => {
                require_positive("tool.corner_radius", corner_radius.0)?;
                if corner_radius.0 > radius {
                    return Err(Error::invalid(
                        "tool.corner_radius",
                        "the corner radius must not exceed the tool radius",
                    ));
                }
                Ok(())
            }
            ToolShape::TaperedBall {
                tip_radius,
                taper_half_angle,
            } => {
                require_positive("tool.tip_radius", tip_radius.0)?;
                require_angle("tool.taper_half_angle", *taper_half_angle, 0.0, 90.0)
            }
            ToolShape::VBit {
                included_angle,
                tip_diameter,
            }
            | ToolShape::Chamfer {
                included_angle,
                tip_diameter,
            } => {
                require_angle("tool.included_angle", *included_angle, 0.0, 180.0)?;
                crate::error::require_non_negative("tool.tip_diameter", tip_diameter.0)
            }
            ToolShape::Drill { point_angle } => {
                require_angle("tool.point_angle", *point_angle, 0.0, 180.0)
            }
            ToolShape::Custom { profile } => {
                if profile.segments.is_empty() {
                    return Err(Error::invalid(
                        "tool.profile",
                        "a custom profile needs at least one segment",
                    ));
                }
                for (i, segment) in profile.segments.iter().enumerate() {
                    let (to, centre) = match segment {
                        ProfileSegment::Line { to } => (to, None),
                        ProfileSegment::Arc { to, centre } => (to, Some(centre)),
                    };
                    for (name, p) in [("to", Some(to)), ("centre", centre)] {
                        if let Some(p) = p {
                            let field = format!("tool.profile.segments[{i}].{name}");
                            require_finite(&field, p.radius.0)?;
                            require_finite(&field, p.height.0)?;
                        }
                    }
                }
                Ok(())
            }
        }
    }
}

/// Fails unless `low < angle < high`.
fn require_angle(field: &str, angle: Degrees, low: f64, high: f64) -> Result<()> {
    require_finite(field, angle.0)?;
    if angle.0 > low && angle.0 < high {
        Ok(())
    } else {
        Err(Error::invalid(
            field,
            format!("{angle} must lie strictly between {low} and {high} degrees"),
        ))
    }
}
