// SPDX-License-Identifier: AGPL-3.0-or-later
//! Cutting tools (TL-02 to TL-06; decision records 0012 and 0014).
//!
//! Every tool shape is described by one revolved profile of line and arc
//! segments ([`profile`]), which path solving, the gouge checker and the
//! simulation sweep share (architecture section 5.1). [`Tool::profile`] maps
//! each type to it by one rule: the cutting end (flat, ball, corner, taper, V
//! or point) runs from the tip to the cutting radius, then a vertical side
//! runs to the flute length. A cutting end taller than the flute length is
//! rejected, so the diameter, the angles and the flute length never silently
//! disagree. A custom profile is the cutting end; it must end at the tool
//! radius, and the same vertical side is added up to the flute length.
//!
//! A tool also carries the controller values the post writes
//! ([`ControllerMapping`], TL-06) and its cutting data per stock material
//! ([`CuttingData`], TL-05), held beside the geometry by the library and the
//! project's tool snapshots.

pub mod profile;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result, require_finite, require_non_negative, require_positive};
use crate::units::{Degrees, FeedRate, Millimetres, SpindleSpeed};

pub use profile::{ProfilePoint, ProfileSegment, RevolvedProfile, ToolProfile};

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
    /// Tapered ball end mill: a ball of `tip_radius`, then a taper tangent to
    /// it out to the tool diameter.
    TaperedBall {
        /// Radius of the ball at the tip.
        tip_radius: Millimetres,
        /// Half angle of the taper, from the tool axis.
        taper_half_angle: Degrees,
    },
    /// V-bit engraver.
    VBit {
        /// Included angle of the V.
        included_angle: Degrees,
        /// Flat diameter at the tip (zero for a sharp point).
        tip_diameter: Millimetres,
    },
    /// Chamfer mill. The same geometry as a V-bit; the type is kept for naming.
    Chamfer {
        /// Included angle.
        included_angle: Degrees,
        /// Flat diameter at the tip.
        tip_diameter: Millimetres,
    },
    /// Twist drill: a sharp V of its point angle.
    Drill {
        /// Included point angle.
        point_angle: Degrees,
    },
    /// A custom revolved profile (TL-03): the cutting end, from the tip to
    /// the tool radius.
    Custom {
        /// The profile.
        profile: ToolProfile,
    },
}

impl ToolShape {
    /// A short name for the type, as the file and the CLI write it.
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::FlatEnd => "flat_end",
            Self::BallEnd => "ball_end",
            Self::BullNose { .. } => "bull_nose",
            Self::TaperedBall { .. } => "tapered_ball",
            Self::VBit { .. } => "v_bit",
            Self::Chamfer { .. } => "chamfer",
            Self::Drill { .. } => "drill",
            Self::Custom { .. } => "custom",
        }
    }
}

/// A cutting tool's geometry and physical parameters (TL-02 to TL-04).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tool {
    /// Shape of the cutting end, with its corner radius, taper or tip angle.
    pub shape: ToolShape,
    /// Cutting diameter.
    pub diameter: Millimetres,
    /// Length of the fluted (cutting) part from the tip.
    pub flute_length: Millimetres,
    /// Overall length of the tool, tip to shank end.
    pub overall_length: Millimetres,
    /// Shank diameter.
    pub shank_diameter: Millimetres,
    /// Number of flutes.
    pub flute_count: u32,
    /// The tool's own material, for example "carbide" or "HSS". Not the stock
    /// material, which keys the cutting data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material: Option<String>,
    /// Whether the cutting end can cut straight down (a centre-cutting end
    /// mill, a drill) or must enter by ramping or a helix (OI-117, decision
    /// 0033). `None` means "not stated": [`Tool::can_plunge`] then follows
    /// the shape.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub centre_cutting: Option<bool>,
}

impl Tool {
    /// True when the tool may enter the material by a straight plunge: the
    /// stated `centre_cutting`, or when it is not stated, the shape's usual
    /// behaviour. End mills, ball ends, drills, V-bits and chamfer tools are
    /// taken as able to plunge; a custom profile is not, because nothing says
    /// what its centre does. A user who knows better states it.
    #[must_use]
    pub fn can_plunge(&self) -> bool {
        self.centre_cutting
            .unwrap_or(!matches!(self.shape, ToolShape::Custom { .. }))
    }

    /// Fails if a dimension is not finite, not positive, inconsistent, or the
    /// shape cannot be built (see [`Tool::profile`]).
    pub fn validate(&self) -> Result<()> {
        self.profile().map(|_| ())
    }

    /// The tool's full revolved outline, from the tip to the flute length,
    /// after checking every dimension. This is the form path solving, the
    /// gouge checker and the simulation sweep read (slice 4 item 8).
    pub fn profile(&self) -> Result<RevolvedProfile> {
        require_positive("tool.diameter", self.diameter.0)?;
        require_positive("tool.flute_length", self.flute_length.0)?;
        require_positive("tool.overall_length", self.overall_length.0)?;
        require_positive("tool.shank_diameter", self.shank_diameter.0)?;
        if self.flute_length.0 > self.overall_length.0 {
            return Err(Error::invalid(
                "tool.flute_length",
                format!(
                    "the flute length {} must not exceed the overall length {}",
                    self.flute_length, self.overall_length
                ),
            ));
        }
        if self.flute_count == 0 {
            return Err(Error::invalid(
                "tool.flute_count",
                "a tool needs at least one flute",
            ));
        }
        if let Some(material) = &self.material {
            require_name("tool.material", material)?;
        }
        let (segments, field) = self.cutting_end()?;
        let r = self.diameter.0 / 2.0;
        let flute = self.flute_length.0;
        let top = segments.last().map_or(0.0, |s| s.end().height.0);
        if top > flute + profile::LENGTH_EPSILON {
            return Err(Error::invalid(
                field,
                format!(
                    "the cutting end reaches the tool radius at {top:.4} mm, above the flute length {flute} mm"
                ),
            ));
        }
        let mut segments = segments;
        if flute - top > profile::LENGTH_EPSILON {
            segments.push(ProfileSegment::Line {
                to: ProfilePoint::new(r, flute),
            });
        }
        ToolProfile { segments }.validate()
    }

    /// The cutting end's segments, from the tip to the tool radius, and the
    /// field to blame when it is too tall.
    fn cutting_end(&self) -> Result<(Vec<ProfileSegment>, &'static str)> {
        let r = self.diameter.0 / 2.0;
        let line = |r: f64, z: f64| ProfileSegment::Line {
            to: ProfilePoint::new(r, z),
        };
        match &self.shape {
            ToolShape::FlatEnd => Ok((vec![line(r, 0.0)], "tool.flute_length")),
            ToolShape::BallEnd => Ok((
                vec![ProfileSegment::Arc {
                    to: ProfilePoint::new(r, r),
                    centre: ProfilePoint::new(0.0, r),
                }],
                "tool.diameter",
            )),
            ToolShape::BullNose { corner_radius } => {
                let c = corner_radius.0;
                require_positive("tool.corner_radius", c)?;
                if c > r {
                    return Err(Error::invalid(
                        "tool.corner_radius",
                        format!(
                            "the corner radius {corner_radius} must not exceed the tool radius {}",
                            Millimetres(r)
                        ),
                    ));
                }
                let mut segments = Vec::new();
                if r - c > profile::LENGTH_EPSILON {
                    segments.push(line(r - c, 0.0));
                }
                segments.push(ProfileSegment::Arc {
                    to: ProfilePoint::new(r, c),
                    centre: ProfilePoint::new(r - c, c),
                });
                Ok((segments, "tool.corner_radius"))
            }
            ToolShape::TaperedBall {
                tip_radius,
                taper_half_angle,
            } => {
                let rt = tip_radius.0;
                require_positive("tool.tip_radius", rt)?;
                let a = require_angle("tool.taper_half_angle", *taper_half_angle, 0.0, 90.0)?;
                // Where the taper leaves the ball: the ball's normal there is at -a.
                let (tr, tz) = (rt * a.cos(), rt * (1.0 - a.sin()));
                if tr >= r - profile::LENGTH_EPSILON {
                    return Err(Error::invalid(
                        "tool.tip_radius",
                        format!(
                            "the tip radius {tip_radius} leaves no taper inside the tool radius {}",
                            Millimetres(r)
                        ),
                    ));
                }
                let top = tz + (r - tr) / a.tan();
                Ok((
                    vec![
                        ProfileSegment::Arc {
                            to: ProfilePoint::new(tr, tz),
                            centre: ProfilePoint::new(0.0, rt),
                        },
                        line(r, top),
                    ],
                    "tool.taper_half_angle",
                ))
            }
            ToolShape::VBit {
                included_angle,
                tip_diameter,
            }
            | ToolShape::Chamfer {
                included_angle,
                tip_diameter,
            } => v_end(r, "tool.included_angle", *included_angle, *tip_diameter),
            ToolShape::Drill { point_angle } => {
                v_end(r, "tool.point_angle", *point_angle, Millimetres(0.0))
            }
            ToolShape::Custom { profile } => {
                let end = profile.end();
                // Validate first, so a broken profile is reported by segment.
                profile.validate()?;
                if (end.radius.0 - r).abs() > profile::ARC_TOLERANCE {
                    return Err(Error::invalid(
                        "tool.profile",
                        format!(
                            "the profile ends at radius {}, but the tool radius is {}; \
                             a custom profile runs from the tip to the cutting radius",
                            end.radius,
                            Millimetres(r)
                        ),
                    ));
                }
                Ok((profile.segments.clone(), "tool.profile"))
            }
        }
    }
}

/// A V-shaped end: a flat of `tip_diameter` (zero for a sharp point), then
/// flanks at half the included angle from the axis out to radius `r`.
fn v_end(
    r: f64,
    field: &'static str,
    included: Degrees,
    tip_diameter: Millimetres,
) -> Result<(Vec<ProfileSegment>, &'static str)> {
    let half = require_angle(field, included, 0.0, 180.0)? / 2.0;
    require_non_negative("tool.tip_diameter", tip_diameter.0)?;
    let rt = tip_diameter.0 / 2.0;
    if rt >= r {
        return Err(Error::invalid(
            "tool.tip_diameter",
            format!(
                "the tip diameter {tip_diameter} must be smaller than the tool diameter {}",
                Millimetres(2.0 * r)
            ),
        ));
    }
    let mut segments = Vec::new();
    if rt > profile::LENGTH_EPSILON {
        segments.push(ProfileSegment::Line {
            to: ProfilePoint::new(rt, 0.0),
        });
    }
    segments.push(ProfileSegment::Line {
        to: ProfilePoint::new(r, (r - rt) / half.tan()),
    });
    Ok((segments, field))
}

/// Controller values for a tool, written by the post (TL-06).
///
/// The offsets default to the tool number, the usual convention (`T3 H3 D3`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ControllerMapping {
    /// Tool number (`T`).
    pub number: u32,
    /// Tool length offset register (`H`), when it differs from the tool number.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub length_offset: Option<u32>,
    /// Diameter (cutter compensation) offset register (`D`), when it differs
    /// from the tool number.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diameter_offset: Option<u32>,
}

impl ControllerMapping {
    /// The mapping for tool `number` with both offsets equal to it.
    pub const fn numbered(number: u32) -> Self {
        Self {
            number,
            length_offset: None,
            diameter_offset: None,
        }
    }

    /// The length offset register to use.
    pub fn length_offset(&self) -> u32 {
        self.length_offset.unwrap_or(self.number)
    }

    /// The diameter offset register to use.
    pub fn diameter_offset(&self) -> u32 {
        self.diameter_offset.unwrap_or(self.number)
    }

    /// Fails unless the number and offsets are at least 1 (0 means "no tool"
    /// or "cancel" on common controllers).
    pub fn validate(&self) -> Result<()> {
        for (field, value) in [
            ("controller.number", Some(self.number)),
            ("controller.length_offset", self.length_offset),
            ("controller.diameter_offset", self.diameter_offset),
        ] {
            if value == Some(0) {
                return Err(Error::invalid(
                    field,
                    "must be at least 1 (0 means no tool or cancel on most controllers)",
                ));
            }
        }
        Ok(())
    }
}

/// Cutting data for one tool in one stock material (TL-05).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CuttingData {
    /// Spindle speed.
    pub spindle_speed: SpindleSpeed,
    /// Feed per tooth (chip load).
    pub feed_per_tooth: Millimetres,
    /// Feed for plunging along the tool axis.
    pub plunge_feed: FeedRate,
    /// Default stepover between passes, across the tool.
    pub stepover: Millimetres,
    /// Default stepdown between passes, along the tool axis.
    pub stepdown: Millimetres,
}

impl CuttingData {
    /// The cutting feed: spindle speed times flute count times feed per tooth.
    pub fn cutting_feed(&self, tool: &Tool) -> FeedRate {
        FeedRate(self.spindle_speed.0 * f64::from(tool.flute_count) * self.feed_per_tooth.0)
    }

    /// Fails if a value is not finite and positive, the stepover exceeds the
    /// tool diameter, or the stepdown exceeds the flute length. `field` names
    /// the entry in messages, for example `cutting_data.oak`.
    pub fn validate(&self, tool: &Tool, field: &str) -> Result<()> {
        require_positive(&format!("{field}.spindle_speed"), self.spindle_speed.0)?;
        require_positive(&format!("{field}.feed_per_tooth"), self.feed_per_tooth.0)?;
        require_positive(&format!("{field}.plunge_feed"), self.plunge_feed.0)?;
        require_positive(&format!("{field}.stepover"), self.stepover.0)?;
        require_positive(&format!("{field}.stepdown"), self.stepdown.0)?;
        if self.stepover.0 > tool.diameter.0 {
            return Err(Error::invalid(
                format!("{field}.stepover"),
                format!(
                    "the stepover {} exceeds the tool diameter {} and would leave material between passes",
                    self.stepover, tool.diameter
                ),
            ));
        }
        if self.stepdown.0 > tool.flute_length.0 {
            return Err(Error::invalid(
                format!("{field}.stepdown"),
                format!(
                    "the stepdown {} exceeds the flute length {}",
                    self.stepdown, tool.flute_length
                ),
            ));
        }
        Ok(())
    }
}

/// Cutting data by stock material name (the name a setup gives its stock,
/// matched ignoring case and repeated spaces; see [`find_cutting_data`]).
pub type CuttingTable = BTreeMap<String, CuttingData>;

/// The form of a material name that names are compared in: surrounding
/// white space removed, runs of white space made one space, and lower case.
/// "Softwood", "softwood" and " SOFTWOOD " share a key; "Soft wood" does not.
pub fn material_key(name: &str) -> String {
    name.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// The cutting data of `table` for the stock material `material`, with the
/// name the table holds it under. An exact name wins; otherwise the names are
/// compared by [`material_key`], first in name order.
pub fn find_cutting_data<'a>(
    table: &'a CuttingTable,
    material: &str,
) -> Option<(&'a str, &'a CuttingData)> {
    if let Some((name, data)) = table.get_key_value(material) {
        return Some((name, data));
    }
    let key = material_key(material);
    table
        .iter()
        .find(|(name, _)| material_key(name) == key)
        .map(|(name, data)| (name.as_str(), data))
}

/// Checks a whole tool entry as the library and the project hold it: the
/// geometry, the controller mapping and every cutting data entry. Material
/// names must be non-empty with no surrounding spaces.
pub fn validate_entry(
    tool: &Tool,
    controller: &ControllerMapping,
    cutting_data: &CuttingTable,
) -> Result<()> {
    tool.validate()?;
    controller.validate()?;
    for (material, data) in cutting_data {
        require_name("cutting_data", material)?;
        data.validate(tool, &format!("cutting_data.{material}"))?;
    }
    let mut seen: BTreeMap<String, &str> = BTreeMap::new();
    for material in cutting_data.keys() {
        if let Some(first) = seen.insert(material_key(material), material) {
            return Err(Error::invalid(
                "cutting_data",
                format!(
                    "the materials {first:?} and {material:?} differ only by case or spacing, so a stock material would match both"
                ),
            ));
        }
    }
    Ok(())
}

/// Fails unless `name` is non-empty and has no surrounding white space.
fn require_name(field: &str, name: &str) -> Result<()> {
    if name.trim().is_empty() {
        Err(Error::invalid(field, "the name must not be empty"))
    } else if name.trim() != name {
        Err(Error::invalid(
            field,
            format!("the name {name:?} must not start or end with spaces"),
        ))
    } else {
        Ok(())
    }
}

/// Fails unless `low < angle < high`; returns the angle in radians.
fn require_angle(field: &str, angle: Degrees, low: f64, high: f64) -> Result<f64> {
    require_finite(field, angle.0)?;
    if angle.0 > low && angle.0 < high {
        Ok(angle.to_radians())
    } else {
        Err(Error::invalid(
            field,
            format!("{angle} must lie strictly between {low} and {high} degrees"),
        ))
    }
}
