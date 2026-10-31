// SPDX-License-Identifier: AGPL-3.0-or-later
//! Editing a machine profile: the form of the machine library window and of
//! the project's copy (SET-01, UI-13, NFR-07; lane MACHINES, decision record
//! 0380, proposed).
//!
//! [`machine_form`] lists every value of a [`MachineProfile`] as a
//! [`MachineField`], in the groups the window shows. [`set_machine_field`]
//! sets one value, checks it on its own and against the rest of the profile,
//! and fails with a [`FieldProblem`] in plain words that names the field by
//! its label. A value that changes is marked "not confirmed on the machine"
//! with [`USER_CHANGED`] as its source (UI-13); setting a value to what it
//! already is keeps its provenance. Nothing here touches a file or the
//! project: the library window and the application commands do that.

use openchuck_project::ParameterValue;
use openchuck_types::machine::{
    ARange, ASense, Confirmation, MachineProfile, ProfileField, Provenance, RotaryAxis,
    RotaryInFeed, ShortBlockMode,
};
use openchuck_types::units::{Degrees, FeedRate, Millimetres, RotaryFeedRate, SpindleSpeed};

use crate::form::FieldKind;

/// The source given to a value the user changes in the editor (UI-13).
pub const USER_CHANGED: &str = "Changed by the user; check it on the machine";

/// The highest tool number the form accepts.
pub const HIGHEST_TOOL_NUMBER: i64 = 1_000_000;

/// Field names of the machine form: the value's path in the profile file,
/// under `[machine]`.
pub mod fields {
    /// The profile's name.
    pub const NAME: &str = "name";
    /// The axis the chuck turns about.
    pub const ROTARY_AXIS: &str = "rotary_axis";
    /// Which way positive A turns the part.
    pub const A_SENSE: &str = "a_sense";
    /// Continuous or limited A travel.
    pub const A_RANGE: &str = "a_range";
    /// Lowest A of a limited range.
    pub const A_MIN: &str = "a_range.min";
    /// Highest A of a limited range.
    pub const A_MAX: &str = "a_range.max";
    /// Rotary reference point, machine X.
    pub const ORIGIN_X: &str = "rotary_origin.x";
    /// Rotary reference point, machine Y.
    pub const ORIGIN_Y: &str = "rotary_origin.y";
    /// Rotary reference point, machine Z (the axis height).
    pub const ORIGIN_Z: &str = "rotary_origin.z";
    /// Lowest machine X.
    pub const TRAVEL_MIN_X: &str = "travel.min.x";
    /// Lowest machine Y.
    pub const TRAVEL_MIN_Y: &str = "travel.min.y";
    /// Lowest machine Z.
    pub const TRAVEL_MIN_Z: &str = "travel.min.z";
    /// Highest machine X.
    pub const TRAVEL_MAX_X: &str = "travel.max.x";
    /// Highest machine Y.
    pub const TRAVEL_MAX_Y: &str = "travel.max.y";
    /// Highest machine Z.
    pub const TRAVEL_MAX_Z: &str = "travel.max.z";
    /// Highest X and Y feed.
    pub const FEED_LINEAR: &str = "max_feeds.linear";
    /// Highest Z feed.
    pub const FEED_Z: &str = "max_feeds.z";
    /// Highest A feed.
    pub const FEED_ROTARY: &str = "max_feeds.rotary";
    /// Highest X and Y rapid.
    pub const RAPID_LINEAR: &str = "max_feeds.rapid_linear";
    /// Highest Z rapid.
    pub const RAPID_Z: &str = "max_feeds.rapid_z";
    /// Highest A rapid.
    pub const RAPID_ROTARY: &str = "max_feeds.rapid_rotary";
    /// How the controller counts A in the feed word.
    pub const ROTARY_IN_FEED: &str = "max_feeds.rotary_in_feed";
    /// Safe cylinder radius.
    pub const CLEARANCE_RADIUS: &str = "clearance.radius";
    /// Safe Z above the axis.
    pub const SAFE_Z: &str = "clearance.safe_z";
    /// Highest tool number.
    pub const MAX_TOOL_NUMBER: &str = "max_tool_number";
    /// Lowest spindle speed.
    pub const MIN_SPINDLE_RPM: &str = "min_spindle_rpm";
    /// Highest spindle speed.
    pub const MAX_SPINDLE_RPM: &str = "max_spindle_rpm";
    /// Minimum average block time (GC-17).
    pub const MIN_BLOCK_MS: &str = "min_block_ms";
    /// Block window (GC-17).
    pub const BLOCK_WINDOW: &str = "block_window";
    /// Short-move handling (GC-17).
    pub const SHORT_BLOCK_MODE: &str = "short_block_mode";
    /// Whether the controller measures each tool's length.
    pub const AUTOMATIC_TOOL_LENGTH: &str = "automatic_tool_length";
    /// Spindle at home to the rotary axis.
    pub const SPINDLE_TO_AXIS: &str = "spindle_to_axis";
    /// Recommended minimum safe Z.
    pub const RECOMMENDED_MIN_SAFE_Z: &str = "recommended_min_safe_z";
}

/// The groups of the machine form, in the order shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MachineGroup {
    /// The profile's name.
    Profile,
    /// Rotary axis, A direction, A range and the rotary reference point.
    RotaryAxis,
    /// Linear travel limits.
    Travel,
    /// Maximum feeds and rapids.
    Feeds,
    /// The default clearance.
    Clearance,
    /// Tool numbers and spindle speeds.
    ToolsAndSpindle,
    /// Short moves (GC-17).
    ShortMoves,
    /// Spindle height (decision 0317).
    SpindleHeight,
}

impl MachineGroup {
    /// The group's heading.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Profile => "Profile",
            Self::RotaryAxis => "Rotary axis",
            Self::Travel => "Travel",
            Self::Feeds => "Maximum feeds",
            Self::Clearance => "Clearance",
            Self::ToolsAndSpindle => "Tools and spindle",
            Self::ShortMoves => "Short moves",
            Self::SpindleHeight => "Spindle height",
        }
    }
}

/// One value of the machine form.
#[derive(Debug, Clone, PartialEq)]
pub struct MachineField {
    /// The field's name ([`fields`]).
    pub name: &'static str,
    /// The label, in sentence case.
    pub label: &'static str,
    /// What the value means, for the tooltip.
    pub description: &'static str,
    /// The group it is shown in.
    pub group: MachineGroup,
    /// What kind of value it takes.
    pub kind: FieldKind,
    /// The current value; `None` for an optional value left blank.
    pub value: Option<ParameterValue>,
    /// True when the value may be left blank.
    pub optional: bool,
    /// The provenance entry the value belongs to (UI-13); `None` for the
    /// name and the highest tool number, which have none.
    pub provenance: Option<ProfileField>,
    /// False when the field does not apply now (the A limits of a
    /// continuous axis). It is still listed, so the form keeps its shape.
    pub applies: bool,
}

/// Why a value was refused, in plain words, naming the field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldProblem {
    /// The field's name ([`fields`]); empty when the problem is the whole
    /// profile's.
    pub name: String,
    /// The field's label.
    pub label: String,
    /// What is wrong and what is allowed, as a sentence.
    pub message: String,
}

impl std::fmt::Display for FieldProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.label.is_empty() {
            f.write_str(&self.message)
        } else {
            write!(f, "{}: {}", self.label, self.message)
        }
    }
}

impl std::error::Error for FieldProblem {}

struct Spec {
    name: &'static str,
    label: &'static str,
    description: &'static str,
    group: MachineGroup,
    provenance: Option<ProfileField>,
}

const fn spec(
    name: &'static str,
    label: &'static str,
    description: &'static str,
    group: MachineGroup,
    provenance: Option<ProfileField>,
) -> Spec {
    Spec {
        name,
        label,
        description,
        group,
        provenance,
    }
}

use MachineGroup as G;
use ProfileField as P;
use fields as f;

const SPECS: &[Spec] = &[
    spec(
        f::NAME,
        "Name",
        "The name the machine is listed under.",
        G::Profile,
        None,
    ),
    spec(
        f::ROTARY_AXIS,
        "Rotary axis",
        "The machine axis the chuck turns about. Only X is supported (a Y axis is SET-02).",
        G::RotaryAxis,
        Some(P::RotaryAxis),
    ),
    spec(
        f::A_SENSE,
        "A+ direction",
        "Positive turns the part right-handed about the positive rotary axis.",
        G::RotaryAxis,
        Some(P::ASense),
    ),
    spec(
        f::A_RANGE,
        "A range",
        "Continuous when the A axis turns without limit, else limited to a range.",
        G::RotaryAxis,
        Some(P::ARange),
    ),
    spec(
        f::A_MIN,
        "Lowest A",
        "The lowest A of a limited range, in degrees.",
        G::RotaryAxis,
        Some(P::ARange),
    ),
    spec(
        f::A_MAX,
        "Highest A",
        "The highest A of a limited range, in degrees.",
        G::RotaryAxis,
        Some(P::ARange),
    ),
    spec(
        f::ORIGIN_X,
        "Rotary reference X",
        "Machine X of the rotary reference point, a point on the axis such as the chuck face centre.",
        G::RotaryAxis,
        Some(P::RotaryOrigin),
    ),
    spec(
        f::ORIGIN_Y,
        "Rotary reference Y",
        "Machine Y of the rotary axis.",
        G::RotaryAxis,
        Some(P::RotaryOrigin),
    ),
    spec(
        f::ORIGIN_Z,
        "Axis height (machine Z)",
        "Machine Z of the rotary axis.",
        G::RotaryAxis,
        Some(P::RotaryOrigin),
    ),
    spec(
        f::TRAVEL_MIN_X,
        "Lowest X",
        "The lowest machine X the machine reaches.",
        G::Travel,
        Some(P::Travel),
    ),
    spec(
        f::TRAVEL_MAX_X,
        "Highest X",
        "The highest machine X the machine reaches.",
        G::Travel,
        Some(P::Travel),
    ),
    spec(
        f::TRAVEL_MIN_Y,
        "Lowest Y",
        "The lowest machine Y the machine reaches.",
        G::Travel,
        Some(P::Travel),
    ),
    spec(
        f::TRAVEL_MAX_Y,
        "Highest Y",
        "The highest machine Y the machine reaches.",
        G::Travel,
        Some(P::Travel),
    ),
    spec(
        f::TRAVEL_MIN_Z,
        "Lowest Z",
        "The lowest machine Z the machine reaches.",
        G::Travel,
        Some(P::Travel),
    ),
    spec(
        f::TRAVEL_MAX_Z,
        "Highest Z",
        "The highest machine Z the machine reaches.",
        G::Travel,
        Some(P::Travel),
    ),
    spec(
        f::FEED_LINEAR,
        "X and Y feed",
        "The highest X and Y feed.",
        G::Feeds,
        Some(P::MaxFeeds),
    ),
    spec(
        f::FEED_Z,
        "Z feed",
        "The highest Z feed.",
        G::Feeds,
        Some(P::MaxFeeds),
    ),
    spec(
        f::FEED_ROTARY,
        "A feed",
        "The highest A feed.",
        G::Feeds,
        Some(P::MaxFeeds),
    ),
    spec(
        f::RAPID_LINEAR,
        "X and Y rapid",
        "The highest X and Y rapid.",
        G::Feeds,
        Some(P::MaxFeeds),
    ),
    spec(
        f::RAPID_Z,
        "Z rapid",
        "The highest Z rapid.",
        G::Feeds,
        Some(P::MaxFeeds),
    ),
    spec(
        f::RAPID_ROTARY,
        "A rapid",
        "The highest A rapid.",
        G::Feeds,
        Some(P::MaxFeeds),
    ),
    spec(
        f::ROTARY_IN_FEED,
        "A in the feed word",
        "Separate: the feed word is the XYZ speed alone, and A has its own limit (LinuxCNC). \
         Makera: the controller counts A with X, Y and Z in the feed word (decision 0007). \
         The post chosen at export has its own feed rule; keep the two consistent (OI-254).",
        G::Feeds,
        Some(P::MaxFeeds),
    ),
    spec(
        f::CLEARANCE_RADIUS,
        "Safe cylinder radius",
        "The default radius about the axis that retracts and rapid links stay outside. \
         A setup can set its own.",
        G::Clearance,
        Some(P::Clearance),
    ),
    spec(
        f::SAFE_Z,
        "Safe Z",
        "The default height above the axis for large rotations, tool changes and the start \
         and end of a program. It must not be below the safe cylinder. A setup can set its own.",
        G::Clearance,
        Some(P::Clearance),
    ),
    spec(
        f::MAX_TOOL_NUMBER,
        "Highest tool number",
        "The highest tool number the controller accepts.",
        G::ToolsAndSpindle,
        None,
    ),
    spec(
        f::MIN_SPINDLE_RPM,
        "Lowest spindle speed",
        "The lowest spindle speed; blank when there is no lower limit.",
        G::ToolsAndSpindle,
        Some(P::SpindleRange),
    ),
    spec(
        f::MAX_SPINDLE_RPM,
        "Highest spindle speed",
        "The highest spindle speed; blank when it is not known.",
        G::ToolsAndSpindle,
        Some(P::SpindleRange),
    ),
    spec(
        f::MIN_BLOCK_MS,
        "Minimum block time",
        "The shortest average time per block the controller keeps up with. 0 turns the check \
         off. A larger time trades speed for smoothness (GC-17).",
        G::ShortMoves,
        Some(P::MinBlockMs),
    ),
    spec(
        f::BLOCK_WINDOW,
        "Block window",
        "How many blocks the average is taken over, from 1 to 4096 (GC-17).",
        G::ShortMoves,
        Some(P::BlockWindow),
    ),
    spec(
        f::SHORT_BLOCK_MODE,
        "Short moves",
        "Off, Report, or Slow down, which lengthens move times only where needed and keeps \
         the programmed geometry (GC-17).",
        G::ShortMoves,
        Some(P::ShortBlockMode),
    ),
    spec(
        f::AUTOMATIC_TOOL_LENGTH,
        "Tool length measurement",
        "On when the controller measures each tool, so a longer tool raises the spindle \
         (decision 0317).",
        G::SpindleHeight,
        Some(P::ToolLengthMeasurement),
    ),
    spec(
        f::SPINDLE_TO_AXIS,
        "Spindle to axis",
        "The spindle at home down to the rotary axis; blank when it is not known (decision 0318).",
        G::SpindleHeight,
        Some(P::SpindleToAxis),
    ),
    spec(
        f::RECOMMENDED_MIN_SAFE_Z,
        "Recommended minimum safe Z",
        "Advice to check, not a limit; blank for none.",
        G::SpindleHeight,
        Some(P::RecommendedMinSafeZ),
    ),
];

fn spec_of(name: &str) -> Option<&'static Spec> {
    SPECS.iter().find(|s| s.name == name)
}

fn number_kind(unit: &'static str, min: Option<f64>, min_exclusive: bool) -> FieldKind {
    FieldKind::Number {
        unit,
        min,
        min_exclusive,
        max: None,
    }
}

fn choice(options: &[&str]) -> FieldKind {
    FieldKind::Choice {
        options: options.iter().map(|&o| o.to_owned()).collect(),
    }
}

fn kind_of(name: &str) -> FieldKind {
    use fields::*;
    match name {
        NAME => FieldKind::Text,
        ROTARY_AXIS => choice(&["x", "y"]),
        A_SENSE => choice(&["positive", "negative"]),
        A_RANGE => choice(&["continuous", "limited"]),
        A_MIN | A_MAX => number_kind("deg", None, false),
        ORIGIN_X | ORIGIN_Y | ORIGIN_Z | TRAVEL_MIN_X | TRAVEL_MIN_Y | TRAVEL_MIN_Z
        | TRAVEL_MAX_X | TRAVEL_MAX_Y | TRAVEL_MAX_Z => number_kind("mm", None, false),
        FEED_LINEAR | FEED_Z | RAPID_LINEAR | RAPID_Z => number_kind("mm/min", Some(0.0), true),
        FEED_ROTARY | RAPID_ROTARY => number_kind("deg/min", Some(0.0), true),
        ROTARY_IN_FEED => choice(&["separate", "makera"]),
        CLEARANCE_RADIUS | SAFE_Z | SPINDLE_TO_AXIS | RECOMMENDED_MIN_SAFE_Z => {
            number_kind("mm", Some(0.0), true)
        }
        MAX_TOOL_NUMBER => FieldKind::Integer {
            min: 1,
            max: crate::machine_edit::HIGHEST_TOOL_NUMBER,
        },
        MIN_SPINDLE_RPM | MAX_SPINDLE_RPM => number_kind("rpm", Some(0.0), true),
        MIN_BLOCK_MS => number_kind("ms", Some(0.0), false),
        BLOCK_WINDOW => FieldKind::Integer { min: 1, max: 4096 },
        SHORT_BLOCK_MODE => choice(&["off", "report", "slow_down"]),
        _ => FieldKind::Flag,
    }
}

fn value_of(profile: &MachineProfile, name: &str) -> Option<ParameterValue> {
    use ParameterValue::{Flag, Integer, Number, Text};
    use fields::*;
    let (a_min, a_max) = match profile.a_range {
        ARange::Continuous => (None, None),
        ARange::Limited { min, max } => (Some(min.0), Some(max.0)),
    };
    let f = &profile.max_feeds;
    Some(match name {
        NAME => Text(profile.name.clone()),
        ROTARY_AXIS => Text(
            match profile.rotary_axis {
                RotaryAxis::X => "x",
                _ => "y",
            }
            .into(),
        ),
        A_SENSE => Text(
            match profile.a_sense {
                ASense::Positive => "positive",
                ASense::Negative => "negative",
            }
            .into(),
        ),
        A_RANGE => Text(
            match profile.a_range {
                ARange::Continuous => "continuous",
                ARange::Limited { .. } => "limited",
            }
            .into(),
        ),
        A_MIN => Number(a_min?),
        A_MAX => Number(a_max?),
        ORIGIN_X => Number(profile.rotary_origin.x),
        ORIGIN_Y => Number(profile.rotary_origin.y),
        ORIGIN_Z => Number(profile.rotary_origin.z),
        TRAVEL_MIN_X => Number(profile.travel.min.x),
        TRAVEL_MIN_Y => Number(profile.travel.min.y),
        TRAVEL_MIN_Z => Number(profile.travel.min.z),
        TRAVEL_MAX_X => Number(profile.travel.max.x),
        TRAVEL_MAX_Y => Number(profile.travel.max.y),
        TRAVEL_MAX_Z => Number(profile.travel.max.z),
        FEED_LINEAR => Number(f.linear.0),
        FEED_Z => Number(f.z.0),
        FEED_ROTARY => Number(f.rotary.0),
        RAPID_LINEAR => Number(f.rapid_linear.0),
        RAPID_Z => Number(f.rapid_z.0),
        RAPID_ROTARY => Number(f.rapid_rotary.0),
        ROTARY_IN_FEED => Text(
            match f.rotary_in_feed {
                RotaryInFeed::Makera => "makera",
                _ => "separate",
            }
            .into(),
        ),
        CLEARANCE_RADIUS => Number(profile.clearance.radius.0),
        SAFE_Z => Number(profile.clearance.safe_z.0),
        MAX_TOOL_NUMBER => Integer(i64::from(profile.max_tool_number)),
        MIN_SPINDLE_RPM => Number(profile.min_spindle_rpm?.0),
        MAX_SPINDLE_RPM => Number(profile.max_spindle_rpm?.0),
        MIN_BLOCK_MS => Number(profile.min_block_ms),
        BLOCK_WINDOW => Integer(i64::try_from(profile.block_window).unwrap_or(i64::MAX)),
        SHORT_BLOCK_MODE => Text(
            match profile.short_block_mode {
                ShortBlockMode::Off => "off",
                ShortBlockMode::Report => "report",
                ShortBlockMode::SlowDown => "slow_down",
            }
            .into(),
        ),
        AUTOMATIC_TOOL_LENGTH => Flag(profile.automatic_tool_length),
        SPINDLE_TO_AXIS => Number(profile.spindle_to_axis?.0),
        RECOMMENDED_MIN_SAFE_Z => Number(profile.recommended_min_safe_z?.0),
        _ => return None,
    })
}

/// Every value of `profile` as a form field, in the order and groups the
/// editor shows them.
pub fn machine_form(profile: &MachineProfile) -> Vec<MachineField> {
    use fields::*;
    let limited = matches!(profile.a_range, ARange::Limited { .. });
    SPECS
        .iter()
        .map(|s| MachineField {
            name: s.name,
            label: s.label,
            description: s.description,
            group: s.group,
            kind: kind_of(s.name),
            value: value_of(profile, s.name),
            optional: matches!(
                s.name,
                MIN_SPINDLE_RPM | MAX_SPINDLE_RPM | SPINDLE_TO_AXIS | RECOMMENDED_MIN_SAFE_Z
            ),
            provenance: s.provenance,
            applies: limited || !matches!(s.name, A_MIN | A_MAX),
        })
        .collect()
}

fn problem(name: &str, message: impl Into<String>) -> FieldProblem {
    FieldProblem {
        name: name.to_owned(),
        label: spec_of(name).map_or_else(String::new, |s| s.label.to_owned()),
        message: message.into(),
    }
}

/// Formats a number for a message: no trailing zeros, at most 4 decimals.
fn plain(v: f64) -> String {
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.into() }
}

/// `profile` with the field `name` set to `value` (`None` leaves an
/// optional value blank), after checking the value and the whole profile.
/// A value that changes is marked not confirmed, with [`USER_CHANGED`] as
/// its source; setting the value it already has changes nothing.
pub fn set_machine_field(
    profile: &MachineProfile,
    name: &str,
    value: Option<&ParameterValue>,
) -> Result<MachineProfile, FieldProblem> {
    use fields::*;
    let Some(spec) = spec_of(name) else {
        return Err(FieldProblem {
            name: name.to_owned(),
            label: String::new(),
            message: format!("A machine profile has no value called \"{name}\"."),
        });
    };
    let unit = match kind_of(name) {
        FieldKind::Number { unit, .. } => unit,
        _ => "",
    };
    let wrong = |message: String| problem(name, message);
    let with_unit = |v: f64| {
        if unit.is_empty() {
            plain(v)
        } else {
            format!("{} {unit}", plain(v))
        }
    };
    // The value as a number, if it is one.
    let number = || -> Result<Option<f64>, FieldProblem> {
        let v = match value {
            None => return Ok(None),
            Some(ParameterValue::Number(v)) => *v,
            #[allow(clippy::cast_precision_loss)]
            Some(ParameterValue::Integer(i)) => *i as f64,
            Some(ParameterValue::Text(t)) if t.trim().is_empty() => return Ok(None),
            Some(_) => return Err(wrong("Enter a number.".into())),
        };
        if v.is_finite() {
            Ok(Some(v))
        } else {
            Err(wrong("Enter a number.".into()))
        }
    };
    let required = || -> Result<f64, FieldProblem> {
        number()?.ok_or_else(|| wrong("Enter a value; it cannot be left blank.".into()))
    };
    let positive = || -> Result<f64, FieldProblem> {
        let v = required()?;
        if v > 0.0 {
            Ok(v)
        } else {
            Err(wrong(format!(
                "Must be more than 0 {unit}; {} was entered.",
                with_unit(v)
            )))
        }
    };
    let optional_positive = || -> Result<Option<f64>, FieldProblem> {
        match number()? {
            None => Ok(None),
            Some(v) if v > 0.0 => Ok(Some(v)),
            Some(v) => Err(wrong(format!(
                "Must be more than 0 {unit}, or blank; {} was entered.",
                with_unit(v)
            ))),
        }
    };
    let whole = |min: i64, max: i64| -> Result<i64, FieldProblem> {
        let n = match value {
            Some(ParameterValue::Integer(i)) => *i,
            Some(ParameterValue::Number(v)) if v.is_finite() && v.fract() == 0.0 => {
                #[allow(clippy::cast_possible_truncation)]
                let n = *v as i64;
                n
            }
            None => return Err(wrong("Enter a value; it cannot be left blank.".into())),
            _ => return Err(wrong("Enter a whole number.".into())),
        };
        if (min..=max).contains(&n) {
            Ok(n)
        } else {
            Err(wrong(format!(
                "Must be a whole number from {min} to {max}; {n} was entered."
            )))
        }
    };
    let word = |options: &[&str]| -> Result<String, FieldProblem> {
        match value {
            Some(ParameterValue::Text(t)) if options.contains(&t.as_str()) => Ok(t.clone()),
            _ => Err(wrong(format!("Choose one of: {}.", options.join(", ")))),
        }
    };

    let mut p = profile.clone();
    match name {
        NAME => {
            let Some(ParameterValue::Text(t)) = value else {
                return Err(wrong("Enter a name.".into()));
            };
            let t = t.trim();
            if t.is_empty() {
                return Err(wrong("Enter a name; it cannot be left blank.".into()));
            }
            p.name = t.to_owned();
        }
        ROTARY_AXIS => {
            if word(&["x", "y"])? == "y" {
                return Err(wrong(
                    "A rotary axis about Y is not supported yet (SET-02); use X.".into(),
                ));
            }
            p.rotary_axis = RotaryAxis::X;
        }
        A_SENSE => {
            p.a_sense = if word(&["positive", "negative"])? == "positive" {
                ASense::Positive
            } else {
                ASense::Negative
            };
        }
        A_RANGE => {
            p.a_range = if word(&["continuous", "limited"])? == "continuous" {
                ARange::Continuous
            } else {
                match p.a_range {
                    // A limited range starts at one turn each way, for the
                    // user to set; it is marked not confirmed like any edit.
                    ARange::Continuous => ARange::Limited {
                        min: Degrees(-360.0),
                        max: Degrees(360.0),
                    },
                    limited @ ARange::Limited { .. } => limited,
                }
            };
        }
        A_MIN | A_MAX => {
            let v = required()?;
            let ARange::Limited { min, max } = &mut p.a_range else {
                return Err(wrong(
                    "The A axis is continuous; choose a limited range first.".into(),
                ));
            };
            if name == A_MIN {
                *min = Degrees(v);
            } else {
                *max = Degrees(v);
            }
        }
        ORIGIN_X => p.rotary_origin.x = required()?,
        ORIGIN_Y => p.rotary_origin.y = required()?,
        ORIGIN_Z => p.rotary_origin.z = required()?,
        TRAVEL_MIN_X => p.travel.min.x = required()?,
        TRAVEL_MIN_Y => p.travel.min.y = required()?,
        TRAVEL_MIN_Z => p.travel.min.z = required()?,
        TRAVEL_MAX_X => p.travel.max.x = required()?,
        TRAVEL_MAX_Y => p.travel.max.y = required()?,
        TRAVEL_MAX_Z => p.travel.max.z = required()?,
        FEED_LINEAR => p.max_feeds.linear = FeedRate(positive()?),
        FEED_Z => p.max_feeds.z = FeedRate(positive()?),
        FEED_ROTARY => p.max_feeds.rotary = RotaryFeedRate(positive()?),
        RAPID_LINEAR => p.max_feeds.rapid_linear = FeedRate(positive()?),
        RAPID_Z => p.max_feeds.rapid_z = FeedRate(positive()?),
        RAPID_ROTARY => p.max_feeds.rapid_rotary = RotaryFeedRate(positive()?),
        ROTARY_IN_FEED => {
            p.max_feeds.rotary_in_feed = if word(&["separate", "makera"])? == "makera" {
                RotaryInFeed::Makera
            } else {
                RotaryInFeed::Separate
            };
        }
        CLEARANCE_RADIUS => p.clearance.radius = Millimetres(positive()?),
        SAFE_Z => p.clearance.safe_z = Millimetres(positive()?),
        MAX_TOOL_NUMBER => {
            p.max_tool_number = u32::try_from(whole(1, crate::machine_edit::HIGHEST_TOOL_NUMBER)?)
                .unwrap_or(u32::MAX);
        }
        MIN_SPINDLE_RPM => p.min_spindle_rpm = optional_positive()?.map(SpindleSpeed),
        MAX_SPINDLE_RPM => p.max_spindle_rpm = optional_positive()?.map(SpindleSpeed),
        MIN_BLOCK_MS => {
            let v = required()?;
            if v < 0.0 {
                return Err(wrong(format!(
                    "Must be 0 ms or more (0 turns the check off); {} was entered.",
                    with_unit(v)
                )));
            }
            p.min_block_ms = v;
        }
        BLOCK_WINDOW => {
            p.block_window = usize::try_from(whole(1, 4096)?).unwrap_or(1);
        }
        SHORT_BLOCK_MODE => {
            p.short_block_mode = match word(&["off", "report", "slow_down"])?.as_str() {
                "off" => ShortBlockMode::Off,
                "report" => ShortBlockMode::Report,
                _ => ShortBlockMode::SlowDown,
            };
        }
        AUTOMATIC_TOOL_LENGTH => {
            let Some(ParameterValue::Flag(on)) = value else {
                return Err(wrong("Choose on or off.".into()));
            };
            p.automatic_tool_length = *on;
        }
        SPINDLE_TO_AXIS => p.spindle_to_axis = optional_positive()?.map(Millimetres),
        RECOMMENDED_MIN_SAFE_Z => {
            p.recommended_min_safe_z = optional_positive()?.map(Millimetres);
        }
        _ => unreachable!("every spec has an arm"),
    }
    if p == *profile {
        return Ok(p);
    }
    if let Some(field) = spec.provenance {
        p.provenance.insert(
            field,
            Provenance {
                state: Confirmation::Unconfirmed,
                source: USER_CHANGED.to_owned(),
            },
        );
    }
    check_machine(&p).map_err(|mut e| {
        // A rule between two values is reported on the field being edited;
        // its message names both values.
        if !e.name.is_empty() {
            e.name = name.to_owned();
            e.label = spec.label.to_owned();
        }
        e
    })?;
    Ok(p)
}

/// Checks the whole profile, giving the first problem in plain words with
/// the field it belongs to: the rules between values first (the A range,
/// the travel, the safe Z against the cylinder, the spindle speeds), then
/// [`MachineProfile::validate`] for everything else.
pub fn check_machine(profile: &MachineProfile) -> Result<(), FieldProblem> {
    use fields::*;
    if let ARange::Limited { min, max } = profile.a_range
        && min.0 >= max.0
    {
        return Err(problem(
            A_MAX,
            format!(
                "The lowest A ({} deg) must be below the highest A ({} deg).",
                plain(min.0),
                plain(max.0)
            ),
        ));
    }
    let t = &profile.travel;
    for (axis, lo, hi, name) in [
        ("X", t.min.x, t.max.x, TRAVEL_MAX_X),
        ("Y", t.min.y, t.max.y, TRAVEL_MAX_Y),
        ("Z", t.min.z, t.max.z, TRAVEL_MAX_Z),
    ] {
        if lo.is_finite() && hi.is_finite() && lo > hi {
            return Err(problem(
                name,
                format!(
                    "The lowest {axis} ({} mm) must not be above the highest {axis} ({} mm).",
                    plain(lo),
                    plain(hi)
                ),
            ));
        }
    }
    let c = &profile.clearance;
    if c.radius.0 > 0.0 && c.safe_z.0 > 0.0 && c.safe_z.0 < c.radius.0 {
        return Err(problem(
            SAFE_Z,
            format!(
                "The safe Z ({} mm) must not be below the safe cylinder radius ({} mm).",
                plain(c.safe_z.0),
                plain(c.radius.0)
            ),
        ));
    }
    if let (Some(lo), Some(hi)) = (profile.min_spindle_rpm, profile.max_spindle_rpm)
        && lo.0 >= hi.0
    {
        return Err(problem(
            MIN_SPINDLE_RPM,
            format!(
                "The lowest spindle speed ({} rpm) must be below the highest ({} rpm).",
                plain(lo.0),
                plain(hi.0)
            ),
        ));
    }
    profile.validate().map_err(|e| match e {
        openchuck_types::Error::InvalidValue { field, reason } => {
            let path = field.strip_prefix("machine.").unwrap_or(&field);
            // A provenance entry belongs to the first field it covers.
            let provenance = path
                .strip_prefix("provenance.")
                .and_then(|rest| rest.split('.').next());
            let name = SPECS
                .iter()
                .find(|s| match provenance {
                    Some(p) => s.provenance.is_some_and(|f| f.name() == p),
                    None => s.name == path || path.starts_with(&format!("{}.", s.name)),
                })
                .or_else(|| SPECS.iter().find(|s| s.name.starts_with(path)))
                .map_or("", |s| s.name);
            let mut p = problem(name, format!("{}.", upper_first(&reason)));
            if name.is_empty() {
                p.message = format!("{path}: {reason}.");
            }
            p
        }
        other => problem("", other.to_string()),
    })
}

fn upper_first(s: &str) -> String {
    let mut c = s.chars();
    c.next().map_or_else(String::new, |f| {
        f.to_uppercase().collect::<String>() + c.as_str()
    })
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::fields::*;
    use super::*;
    use openchuck_project::machines::builtin;
    use proptest::prelude::*;

    fn z1() -> MachineProfile {
        builtin("makera-z1").expect("built in").expect("reads")
    }

    fn num(v: f64) -> ParameterValue {
        ParameterValue::Number(v)
    }

    fn text(t: &str) -> ParameterValue {
        ParameterValue::Text(t.to_owned())
    }

    #[test]
    fn req_set_01_req_ui_13_the_form_lists_every_value_with_its_provenance() {
        let p = z1();
        let form = machine_form(&p);
        // Every profile field that has provenance is in the form.
        for field in ProfileField::ALL {
            assert!(
                form.iter().any(|f| f.provenance == Some(field)),
                "{field} is not in the form"
            );
        }
        let get = |n: &str| form.iter().find(|f| f.name == n).expect(n);
        assert_eq!(get(NAME).value, Some(text("Makera Z1")));
        assert_eq!(get(ORIGIN_Z).value, Some(num(-48.6)));
        assert_eq!(get(SAFE_Z).value, Some(num(40.0)));
        assert_eq!(get(SPINDLE_TO_AXIS).value, Some(num(73.1)));
        assert_eq!(get(RECOMMENDED_MIN_SAFE_Z).value, Some(num(30.0)));
        assert_eq!(
            get(MAX_TOOL_NUMBER).value,
            Some(ParameterValue::Integer(1000))
        );
        assert_eq!(get(ROTARY_IN_FEED).value, Some(text("makera")));
        assert_eq!(get(MIN_SPINDLE_RPM).value, None);
        assert!(get(MIN_SPINDLE_RPM).optional);
        // A continuous axis lists its limits but they do not apply.
        assert!(!get(A_MIN).applies && get(A_MIN).value.is_none());
        assert!(get(A_RANGE).applies);
        // Groups come in the order shown.
        let groups: Vec<_> = form.iter().map(|f| f.group).collect();
        let mut sorted = groups.clone();
        sorted.sort();
        assert_eq!(groups, sorted);
        // Every field's current value sets back without a change.
        for f in form.iter().filter(|f| f.applies) {
            let again = set_machine_field(&p, f.name, f.value.as_ref())
                .unwrap_or_else(|e| panic!("{}: {e}", f.name));
            assert_eq!(again, p, "{} changed the profile", f.name);
        }
    }

    #[test]
    fn req_ui_13_an_edited_value_is_marked_not_confirmed_and_others_keep_theirs() {
        let p = z1();
        assert!(p.is_confirmed(ProfileField::MaxFeeds));
        let q = set_machine_field(&p, FEED_Z, Some(&num(500.0))).expect("valid");
        assert_eq!(q.max_feeds.z.0, 500.0);
        assert!(!q.is_confirmed(ProfileField::MaxFeeds));
        assert_eq!(q.provenance[&ProfileField::MaxFeeds].source, USER_CHANGED);
        // Every other value keeps its provenance, including the owner's
        // axis height (decision 0317).
        for field in ProfileField::ALL {
            if field != ProfileField::MaxFeeds {
                assert_eq!(
                    p.provenance.get(&field),
                    q.provenance.get(&field),
                    "{field}"
                );
            }
        }
        // The same value again keeps the provenance it had.
        let same = set_machine_field(&p, FEED_Z, Some(&num(600.0))).expect("valid");
        assert_eq!(same, p);
        // A name change has no provenance to touch.
        let named = set_machine_field(&p, NAME, Some(&text("  My Z1 "))).expect("valid");
        assert_eq!(named.name, "My Z1");
        assert_eq!(named.provenance, p.provenance);
    }

    #[test]
    fn req_set_01_every_kind_of_value_can_be_set() {
        let p = z1();
        let set = |p: &MachineProfile, n: &str, v: Option<ParameterValue>| {
            set_machine_field(p, n, v.as_ref()).unwrap_or_else(|e| panic!("{n}: {e}"))
        };
        let q = set(&p, A_SENSE, Some(text("negative")));
        assert_eq!(q.a_sense, ASense::Negative);
        let q = set(&p, A_RANGE, Some(text("limited")));
        assert_eq!(
            q.a_range,
            ARange::Limited {
                min: Degrees(-360.0),
                max: Degrees(360.0)
            }
        );
        let q = set(&q, A_MAX, Some(num(720.0)));
        let q = set(&q, A_MIN, Some(num(-10.0)));
        assert_eq!(
            q.a_range,
            ARange::Limited {
                min: Degrees(-10.0),
                max: Degrees(720.0)
            }
        );
        // Choosing limited again keeps the limits; continuous drops them.
        assert_eq!(set(&q, A_RANGE, Some(text("limited"))), q);
        assert_eq!(
            set(&q, A_RANGE, Some(text("continuous"))).a_range,
            ARange::Continuous
        );
        let q = set(&p, ORIGIN_X, Some(num(-150.0)));
        assert_eq!(q.rotary_origin.x, -150.0);
        let q = set(&p, TRAVEL_MIN_Z, Some(ParameterValue::Integer(-90)));
        assert_eq!(q.travel.min.z, -90.0);
        let q = set(&p, ROTARY_IN_FEED, Some(text("separate")));
        assert_eq!(q.max_feeds.rotary_in_feed, RotaryInFeed::Separate);
        let q = set(&p, MAX_TOOL_NUMBER, Some(ParameterValue::Integer(24)));
        assert_eq!(q.max_tool_number, 24);
        assert_eq!(q.provenance, p.provenance);
        let q = set(&p, MIN_SPINDLE_RPM, Some(num(3000.0)));
        assert_eq!(q.min_spindle_rpm, Some(SpindleSpeed(3000.0)));
        let q = set(&p, MAX_SPINDLE_RPM, None);
        assert_eq!(q.max_spindle_rpm, None);
        let q = set(&p, SPINDLE_TO_AXIS, Some(text("")));
        assert_eq!(q.spindle_to_axis, None);
        let q = set(&p, SHORT_BLOCK_MODE, Some(text("slow_down")));
        assert_eq!(q.short_block_mode, ShortBlockMode::SlowDown);
        let q = set(&p, BLOCK_WINDOW, Some(ParameterValue::Integer(8)));
        assert_eq!(q.block_window, 8);
        let q = set(&p, MIN_BLOCK_MS, Some(num(0.0)));
        assert_eq!(q.min_block_ms, 0.0);
        let q = set(&p, AUTOMATIC_TOOL_LENGTH, Some(ParameterValue::Flag(false)));
        assert!(!q.automatic_tool_length);
        let q = set(&p, SAFE_Z, Some(num(55.0)));
        assert_eq!(q.clearance.safe_z.0, 55.0);
    }

    fn refused(p: &MachineProfile, n: &str, v: Option<ParameterValue>) -> FieldProblem {
        match set_machine_field(p, n, v.as_ref()) {
            Ok(_) => panic!("{n} = {v:?} was accepted"),
            Err(e) => e,
        }
    }

    #[test]
    fn req_nfr_07_bad_values_are_refused_in_plain_words_naming_the_field() {
        let p = z1();
        let cases: &[(&str, Option<ParameterValue>, &str, &str)] = &[
            (
                NAME,
                Some(text("  ")),
                "Name",
                "Enter a name; it cannot be left blank.",
            ),
            (
                FEED_Z,
                Some(num(0.0)),
                "Z feed",
                "Must be more than 0 mm/min; 0 mm/min was entered.",
            ),
            (
                FEED_ROTARY,
                Some(num(-5.0)),
                "A feed",
                "Must be more than 0 deg/min; -5 deg/min was entered.",
            ),
            (
                FEED_LINEAR,
                Some(num(f64::NAN)),
                "X and Y feed",
                "Enter a number.",
            ),
            (
                FEED_LINEAR,
                Some(text("fast")),
                "X and Y feed",
                "Enter a number.",
            ),
            (
                ORIGIN_Z,
                None,
                "Axis height (machine Z)",
                "Enter a value; it cannot be left blank.",
            ),
            (
                ROTARY_AXIS,
                Some(text("y")),
                "Rotary axis",
                "A rotary axis about Y is not supported yet (SET-02); use X.",
            ),
            (
                A_SENSE,
                Some(text("left")),
                "A+ direction",
                "Choose one of: positive, negative.",
            ),
            (
                A_MIN,
                Some(num(0.0)),
                "Lowest A",
                "The A axis is continuous; choose a limited range first.",
            ),
            (
                SAFE_Z,
                Some(num(30.0)),
                "Safe Z",
                "The safe Z (30 mm) must not be below the safe cylinder radius (40 mm).",
            ),
            (
                CLEARANCE_RADIUS,
                Some(num(45.0)),
                "Safe cylinder radius",
                "The safe Z (40 mm) must not be below the safe cylinder radius (45 mm).",
            ),
            (
                TRAVEL_MAX_Z,
                Some(num(-120.0)),
                "Highest Z",
                "The lowest Z (-100 mm) must not be above the highest Z (-120 mm).",
            ),
            (
                TRAVEL_MIN_X,
                Some(num(10.0)),
                "Lowest X",
                "The lowest X (10 mm) must not be above the highest X (0 mm).",
            ),
            (
                MAX_TOOL_NUMBER,
                Some(ParameterValue::Integer(0)),
                "Highest tool number",
                "Must be a whole number from 1 to 1000000; 0 was entered.",
            ),
            (
                MAX_TOOL_NUMBER,
                Some(num(2.5)),
                "Highest tool number",
                "Enter a whole number.",
            ),
            (
                BLOCK_WINDOW,
                Some(ParameterValue::Integer(5000)),
                "Block window",
                "Must be a whole number from 1 to 4096; 5000 was entered.",
            ),
            (
                MIN_BLOCK_MS,
                Some(num(-1.0)),
                "Minimum block time",
                "Must be 0 ms or more (0 turns the check off); -1 ms was entered.",
            ),
            (
                MIN_SPINDLE_RPM,
                Some(num(14000.0)),
                "Lowest spindle speed",
                "The lowest spindle speed (14000 rpm) must be below the highest (13000 rpm).",
            ),
            (
                SPINDLE_TO_AXIS,
                Some(num(0.0)),
                "Spindle to axis",
                "Must be more than 0 mm, or blank; 0 mm was entered.",
            ),
            (
                AUTOMATIC_TOOL_LENGTH,
                Some(num(1.0)),
                "Tool length measurement",
                "Choose on or off.",
            ),
        ];
        for (name, value, label, message) in cases {
            let e = refused(&p, name, value.clone());
            assert_eq!(e.name, *name, "{name}");
            assert_eq!(e.label, *label, "{name}");
            assert_eq!(e.message, *message, "{name}");
            assert_eq!(e.to_string(), format!("{label}: {message}"));
        }
        let e = refused(&p, "spindle_colour", Some(text("red")));
        assert_eq!(
            e.to_string(),
            "A machine profile has no value called \"spindle_colour\"."
        );
        // A limited range with its ends the wrong way round.
        let q = set_machine_field(&p, A_RANGE, Some(&text("limited"))).expect("valid");
        let e = refused(&q, A_MIN, Some(num(400.0)));
        assert_eq!(
            e.to_string(),
            "Lowest A: The lowest A (400 deg) must be below the highest A (360 deg)."
        );
    }

    #[test]
    fn req_nfr_07_a_whole_profile_check_names_the_field_in_plain_words() {
        let mut p = z1();
        assert_eq!(check_machine(&p), Ok(()));
        p.provenance
            .get_mut(&ProfileField::Travel)
            .expect("set")
            .source = " ".into();
        let e = check_machine(&p).expect_err("empty source");
        assert_eq!(e.name, TRAVEL_MIN_X);
        assert_eq!(e.message, "Say where the value came from.");
        let mut p = z1();
        p.rotary_origin.y = f64::INFINITY;
        let e = check_machine(&p).expect_err("not finite");
        assert_eq!(e.name, ORIGIN_Y);
        assert_eq!(e.label, "Rotary reference Y");
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(512))]

        /// NFR-07: no name or value panics, and whatever is accepted is a
        /// valid profile.
        #[test]
        fn req_nfr_07_setting_any_value_never_panics(
            which in 0usize..40,
            kind in 0u8..5,
            n in proptest::num::f64::ANY,
            i in any::<i64>(),
            t in ".{0,12}",
        ) {
            let p = z1();
            let names: Vec<&str> = SPECS.iter().map(|s| s.name).chain(["bogus", ""]).collect();
            let name = names[which % names.len()];
            let value = match kind {
                0 => None,
                1 => Some(ParameterValue::Number(n)),
                2 => Some(ParameterValue::Integer(i)),
                3 => Some(ParameterValue::Flag(i % 2 == 0)),
                _ => Some(ParameterValue::Text(t)),
            };
            if let Ok(q) = set_machine_field(&p, name, value.as_ref()) {
                prop_assert!(q.validate().is_ok());
                prop_assert_eq!(check_machine(&q), Ok(()));
            }
        }
    }
}
