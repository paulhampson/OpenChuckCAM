// SPDX-License-Identifier: AGPL-3.0-or-later
//! Editing a project's tools: the tool form and the custom-profile editor
//! (UI-07, TL-03; work breakdown slice 8 item 5; decision record 0029
//! item 12).
//!
//! The form lists a tool's dimensions as [`crate::form::Field`]s, the same
//! view model as an operation's parameters, with the fields its shape needs.
//! The profile editor works on a [`ToolProfile`] the front end holds while
//! the user edits it: [`profile_outline`] draws it even while it is invalid,
//! and [`check_profile`] gives the same verdict as importing it from a file,
//! because both end in [`ToolProfile::validate`] through [`Tool::validate`].
//! Changes reach the project only through [`crate::Application`] commands,
//! which refuse an invalid tool and leave the project unchanged.

use openchuck_project::ParameterValue;
use openchuck_types::tool::profile::ProfilePiece;
use openchuck_types::tool::{ProfilePoint, ProfileSegment, Tool, ToolProfile, ToolShape};
use openchuck_types::units::{Degrees, Millimetres};

use crate::AppError;
use crate::form::{Field, FieldKind, Source};
use crate::tools::with_custom_profile;

/// Field names of the tool form.
pub mod fields {
    /// Tool diameter.
    pub const DIAMETER: &str = "diameter";
    /// Flute (cutting) length.
    pub const FLUTE_LENGTH: &str = "flute_length";
    /// Overall length.
    pub const OVERALL_LENGTH: &str = "overall_length";
    /// Shank diameter.
    pub const SHANK_DIAMETER: &str = "shank_diameter";
    /// Number of flutes.
    pub const FLUTE_COUNT: &str = "flute_count";
    /// Bull-nose corner radius.
    pub const CORNER_RADIUS: &str = "corner_radius";
    /// Tapered ball tip radius.
    pub const TIP_RADIUS: &str = "tip_radius";
    /// Tapered ball half angle.
    pub const TAPER_HALF_ANGLE: &str = "taper_half_angle";
    /// V-bit or chamfer included angle.
    pub const INCLUDED_ANGLE: &str = "included_angle";
    /// V-bit or chamfer flat at the tip.
    pub const TIP_DIAMETER: &str = "tip_diameter";
    /// Drill point angle.
    pub const POINT_ANGLE: &str = "point_angle";
}

fn mm(name: &str, label: &str, description: &str, value: f64) -> Field {
    number(name, label, description, "mm", Some(0.0), true, None, value)
}

fn degrees(name: &str, label: &str, description: &str, max: f64, value: f64) -> Field {
    number(
        name,
        label,
        description,
        "deg",
        Some(0.0),
        true,
        Some(max),
        value,
    )
}

#[allow(clippy::too_many_arguments)]
fn number(
    name: &str,
    label: &str,
    description: &str,
    unit: &'static str,
    min: Option<f64>,
    min_exclusive: bool,
    max: Option<f64>,
    value: f64,
) -> Field {
    Field {
        name: name.to_owned(),
        label: label.to_owned(),
        description: description.to_owned(),
        kind: FieldKind::Number {
            unit,
            min,
            min_exclusive,
            max,
        },
        set: Some(ParameterValue::Number(value)),
        value: Some(ParameterValue::Number(value)),
        source: Source::Operation,
    }
}

/// The tool's form fields: its dimensions, then what its shape needs. A
/// custom-profile tool has no diameter field: its diameter is where the
/// profile ends.
pub fn tool_form(tool: &Tool) -> Vec<Field> {
    use fields::*;
    let mut out = Vec::new();
    if !matches!(tool.shape, ToolShape::Custom { .. }) {
        out.push(mm(
            DIAMETER,
            "Diameter",
            "Cutting diameter.",
            tool.diameter.0,
        ));
    }
    out.push(mm(
        FLUTE_LENGTH,
        "Flute length",
        "Length of the cutting part, from the tip.",
        tool.flute_length.0,
    ));
    out.push(mm(
        OVERALL_LENGTH,
        "Overall length",
        "Length from the tip to the end of the shank.",
        tool.overall_length.0,
    ));
    out.push(mm(
        SHANK_DIAMETER,
        "Shank diameter",
        "Diameter of the shank.",
        tool.shank_diameter.0,
    ));
    out.push(Field {
        name: FLUTE_COUNT.to_owned(),
        label: "Flutes".to_owned(),
        description: "Number of flutes, for the feed per tooth.".to_owned(),
        kind: FieldKind::Integer { min: 1, max: 99 },
        set: Some(ParameterValue::Integer(i64::from(tool.flute_count))),
        value: Some(ParameterValue::Integer(i64::from(tool.flute_count))),
        source: Source::Operation,
    });
    match &tool.shape {
        ToolShape::BullNose { corner_radius } => out.push(mm(
            CORNER_RADIUS,
            "Corner radius",
            "Radius of the rounded corner, at most the tool radius.",
            corner_radius.0,
        )),
        ToolShape::TaperedBall {
            tip_radius,
            taper_half_angle,
        } => {
            out.push(mm(
                TIP_RADIUS,
                "Tip radius",
                "Radius of the ball at the tip.",
                tip_radius.0,
            ));
            out.push(degrees(
                TAPER_HALF_ANGLE,
                "Taper half angle",
                "Angle of the taper from the tool axis.",
                90.0,
                taper_half_angle.0,
            ));
        }
        ToolShape::VBit {
            included_angle,
            tip_diameter,
        }
        | ToolShape::Chamfer {
            included_angle,
            tip_diameter,
        } => {
            out.push(degrees(
                INCLUDED_ANGLE,
                "Included angle",
                "Angle between the two cutting edges.",
                180.0,
                included_angle.0,
            ));
            let mut tip = mm(
                TIP_DIAMETER,
                "Tip diameter",
                "Flat at the tip; zero for a sharp point.",
                tip_diameter.0,
            );
            tip.kind = FieldKind::Number {
                unit: "mm",
                min: Some(0.0),
                min_exclusive: false,
                max: None,
            };
            out.push(tip);
        }
        ToolShape::Drill { point_angle } => out.push(degrees(
            POINT_ANGLE,
            "Point angle",
            "Included angle of the drill point.",
            180.0,
            point_angle.0,
        )),
        _ => {}
    }
    out
}

/// The tool with field `name` set to `value`, after checking the whole tool
/// (TL-02, TL-03). Fails naming the field.
pub fn set_tool_field(tool: &Tool, name: &str, value: &ParameterValue) -> Result<Tool, AppError> {
    use fields::*;
    let wrong = |what: &str| {
        AppError::Engine(openchuck_types::Error::invalid(
            format!("tool.{name}"),
            what.to_owned(),
        ))
    };
    let number = match value {
        ParameterValue::Number(v) => Some(*v),
        #[allow(clippy::cast_precision_loss)]
        ParameterValue::Integer(i) => Some(*i as f64),
        _ => None,
    };
    let mut t = tool.clone();
    if name == FLUTE_COUNT {
        let ParameterValue::Integer(n) = value else {
            return Err(wrong("expected a whole number"));
        };
        t.flute_count = u32::try_from(*n).map_err(|_| wrong("must be from 1 to 99"))?;
        if !(1..=99).contains(&t.flute_count) {
            return Err(wrong("must be from 1 to 99"));
        }
    } else {
        let v = number.ok_or_else(|| wrong("expected a number"))?;
        if !v.is_finite() {
            return Err(wrong("the value is not a finite number"));
        }
        let shape = &mut t.shape;
        match (name, shape) {
            (DIAMETER, s) if !matches!(s, ToolShape::Custom { .. }) => t.diameter = Millimetres(v),
            (FLUTE_LENGTH, _) => t.flute_length = Millimetres(v),
            (OVERALL_LENGTH, _) => t.overall_length = Millimetres(v),
            (SHANK_DIAMETER, _) => t.shank_diameter = Millimetres(v),
            (CORNER_RADIUS, ToolShape::BullNose { corner_radius }) => {
                *corner_radius = Millimetres(v)
            }
            (TIP_RADIUS, ToolShape::TaperedBall { tip_radius, .. }) => *tip_radius = Millimetres(v),
            (
                TAPER_HALF_ANGLE,
                ToolShape::TaperedBall {
                    taper_half_angle, ..
                },
            ) => {
                if v <= 0.0 || v >= 90.0 {
                    return Err(wrong("must be above 0 and below 90 degrees"));
                }
                *taper_half_angle = Degrees(v);
            }
            (
                INCLUDED_ANGLE,
                ToolShape::VBit { included_angle, .. } | ToolShape::Chamfer { included_angle, .. },
            ) => {
                if v <= 0.0 || v >= 180.0 {
                    return Err(wrong("must be above 0 and below 180 degrees"));
                }
                *included_angle = Degrees(v);
            }
            (
                TIP_DIAMETER,
                ToolShape::VBit { tip_diameter, .. } | ToolShape::Chamfer { tip_diameter, .. },
            ) => *tip_diameter = Millimetres(v),
            (POINT_ANGLE, ToolShape::Drill { point_angle }) => {
                if v <= 0.0 || v >= 180.0 {
                    return Err(wrong("must be above 0 and below 180 degrees"));
                }
                *point_angle = Degrees(v);
            }
            _ => return Err(wrong("this tool has no such field")),
        }
    }
    t.validate()?;
    Ok(t)
}

/// The tool with its shape replaced by the custom `profile`, after the same
/// checks as importing the profile from a file (TL-03; decision record 0012
/// item 2). Fails naming the segment.
pub fn set_tool_profile(tool: &Tool, profile: ToolProfile) -> Result<Tool, AppError> {
    let t = with_custom_profile(tool, profile);
    t.validate()?;
    Ok(t)
}

/// Whether `profile` would make a valid tool from `tool`: `Ok`, or the reason
/// naming the segment. For the editor's live preview.
pub fn check_profile(tool: &Tool, profile: &ToolProfile) -> Result<(), String> {
    with_custom_profile(tool, profile.clone())
        .validate()
        .map_err(|e| e.to_string())
}

/// A starting profile for the editor: the tool's own outline as segments
/// (a custom tool's profile as it is), so converting a standard tool to a
/// custom one keeps its shape. `None` when the tool is invalid.
pub fn starting_profile(tool: &Tool) -> Option<ToolProfile> {
    if let ToolShape::Custom { profile } = &tool.shape {
        return Some(profile.clone());
    }
    let revolved = tool.profile().ok()?;
    let segments = revolved
        .pieces()
        .iter()
        .map(|piece| match *piece {
            ProfilePiece::Line { end, .. } => ProfileSegment::Line { to: end },
            ProfilePiece::Arc { end, centre, .. } => ProfileSegment::Arc { to: end, centre },
        })
        .collect();
    Some(ToolProfile { segments })
}

/// Segments used to draw a quarter turn of arc.
const ARC_STEPS: usize = 16;

/// The profile as points in (radius, height) order, from the tip, with arcs
/// drawn as short lines, for a preview. It is drawn whether or not the
/// profile is valid: an arc whose ends are not on one circle is drawn about
/// its given centre at the start's distance, and the short way round.
pub fn profile_outline(profile: &ToolProfile) -> Vec<[f64; 2]> {
    let mut points = vec![[0.0, 0.0]];
    let mut at = ProfilePoint::TIP;
    for segment in &profile.segments {
        match *segment {
            ProfileSegment::Line { to } => points.push([to.radius.0, to.height.0]),
            ProfileSegment::Arc { to, centre } => {
                let (cr, ch) = (centre.radius.0, centre.height.0);
                let a0 = (at.height.0 - ch).atan2(at.radius.0 - cr);
                let a1 = (to.height.0 - ch).atan2(to.radius.0 - cr);
                let mut sweep = a1 - a0;
                if sweep > std::f64::consts::PI {
                    sweep -= std::f64::consts::TAU;
                } else if sweep < -std::f64::consts::PI {
                    sweep += std::f64::consts::TAU;
                }
                let radius = at.distance(centre);
                let steps = ((sweep.abs() / std::f64::consts::FRAC_PI_2) * ARC_STEPS as f64)
                    .ceil()
                    .max(1.0) as usize;
                for i in 1..steps {
                    let a = a0 + sweep * i as f64 / steps as f64;
                    points.push([cr + radius * a.cos(), ch + radius * a.sin()]);
                }
                points.push([to.radius.0, to.height.0]);
            }
        }
        at = segment.end();
    }
    points
}

/// Where segment `index` starts: the tip for the first, else the previous
/// segment's end.
pub fn segment_start(profile: &ToolProfile, index: usize) -> ProfilePoint {
    index
        .checked_sub(1)
        .and_then(|i| profile.segments.get(i))
        .map_or(ProfilePoint::TIP, ProfileSegment::end)
}

/// Inserts a line after segment `after` (`None`: at the start), ending
/// 1 mm above where it starts, so the user can then move its end. The
/// profile may be invalid until they do; the preview shows it.
pub fn insert_segment(profile: &mut ToolProfile, after: Option<usize>) {
    let index = after.map_or(0, |i| (i + 1).min(profile.segments.len()));
    let start = segment_start(profile, index);
    let to = ProfilePoint::new(start.radius.0, start.height.0 + 1.0);
    profile.segments.insert(index, ProfileSegment::Line { to });
}

/// Removes segment `index`; nothing happens when there is no such segment.
pub fn remove_segment(profile: &mut ToolProfile, index: usize) {
    if index < profile.segments.len() {
        profile.segments.remove(index);
    }
}

/// Turns segment `index` into a convex quarter-turn arc between the same
/// ends (a ball-like corner), or into a straight line. An arc whose ends
/// coincide cannot be made, and the segment is left as it was.
pub fn set_segment_arc(profile: &mut ToolProfile, index: usize, arc: bool) {
    let start = segment_start(profile, index);
    let Some(segment) = profile.segments.get_mut(index) else {
        return;
    };
    let to = segment.end();
    if !arc {
        *segment = ProfileSegment::Line { to };
        return;
    }
    let radius = start.distance(to) / std::f64::consts::SQRT_2;
    if let Ok(centre) = openchuck_types::tool::profile::arc_centre(start, to, radius, true) {
        *segment = ProfileSegment::Arc { to, centre };
    }
}
