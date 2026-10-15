// SPDX-License-Identifier: AGPL-3.0-or-later
//! Self-describing parameter schemas (OP-16, UI-07) and the common operation
//! parameters (OP-11 core; decision record 0018 item 7).
//!
//! A strategy publishes a [`ParameterSchema`]: each parameter's name, label,
//! unit, range, default and choices. The parameter panel is generated from it
//! (UI-07), and [`ParameterSchema::resolve`] checks a set of values against it,
//! fills in defaults and refuses unknown names, so an operation never runs
//! with a value its strategy does not understand.

use std::collections::BTreeMap;

use openchuck_types::units::{Degrees, FeedRate, Millimetres, Tolerance};

use crate::error::{PathError, Result};

/// A parameter value, as an operation stores it.
#[derive(Debug, Clone, PartialEq)]
pub enum ParameterValue {
    /// A whole number.
    Integer(i64),
    /// A number in the unit the schema gives.
    Number(f64),
    /// On or off.
    Flag(bool),
    /// Text, including a choice from a list.
    Text(String),
}

/// The unit a number is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Unit {
    /// Millimetres.
    Millimetres,
    /// Degrees.
    Degrees,
    /// Millimetres per minute.
    MillimetresPerMinute,
    /// A plain number.
    None,
}

impl Unit {
    /// The unit's symbol for display, empty for a plain number.
    pub const fn symbol(self) -> &'static str {
        match self {
            Self::Millimetres => "mm",
            Self::Degrees => "deg",
            Self::MillimetresPerMinute => "mm/min",
            Self::None => "",
        }
    }
}

/// What kind of value a parameter takes, with its range and default.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum ParameterKind {
    /// A number in `unit`. `min` is exclusive when `min_exclusive` is set.
    /// No default means the value is required.
    Number {
        /// The unit.
        unit: Unit,
        /// Lowest value, if any.
        min: Option<f64>,
        /// True when `min` itself is not allowed.
        min_exclusive: bool,
        /// Highest value, if any (inclusive).
        max: Option<f64>,
        /// The default, if any.
        default: Option<f64>,
    },
    /// A whole number in `min..=max`.
    Integer {
        /// Lowest value.
        min: i64,
        /// Highest value.
        max: i64,
        /// The default, if any.
        default: Option<i64>,
    },
    /// On or off.
    Flag {
        /// The default.
        default: bool,
    },
    /// One of a fixed list of words.
    Choice {
        /// The allowed words, in display order.
        options: Vec<String>,
        /// The default, one of `options`.
        default: String,
    },
    /// A list of angles in degrees, typed as numbers separated by commas,
    /// semicolons or spaces. May be empty when `default` is.
    AngleList {
        /// The default text, possibly empty.
        default: String,
    },
}

/// One parameter of a strategy.
#[derive(Debug, Clone, PartialEq)]
pub struct ParameterSpec {
    /// The name stored in the operation (`stepover`).
    pub name: String,
    /// The label shown to the user (`Stepover`).
    pub label: String,
    /// What it does, in a sentence.
    pub description: String,
    /// Its kind, range and default.
    pub kind: ParameterKind,
}

/// Every parameter of a strategy, in display order.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ParameterSchema {
    /// The parameters.
    pub parameters: Vec<ParameterSpec>,
}

/// Values checked against a schema, with defaults filled in.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Parameters(BTreeMap<String, ParameterValue>);

impl ParameterSchema {
    /// The schema with the common parameters first, then `own`.
    pub fn with_common(own: Vec<ParameterSpec>) -> Self {
        let mut parameters = common_parameters();
        parameters.extend(own);
        Self { parameters }
    }

    /// The parameter called `name`.
    pub fn get(&self, name: &str) -> Option<&ParameterSpec> {
        self.parameters.iter().find(|p| p.name == name)
    }

    /// Checks `values` against the schema. Fails, naming the parameter, on an
    /// unknown name, a missing required value, a value of the wrong kind, a
    /// value out of range or a choice not in the list.
    pub fn resolve(&self, values: &BTreeMap<String, ParameterValue>) -> Result<Parameters> {
        if let Some(unknown) = values.keys().find(|k| self.get(k).is_none()) {
            return Err(PathError::parameter(
                unknown.clone(),
                "this strategy has no parameter of that name",
            ));
        }
        let mut out = BTreeMap::new();
        for spec in &self.parameters {
            let value = resolve_one(spec, values.get(&spec.name))?;
            out.insert(spec.name.clone(), value);
        }
        Ok(Parameters(out))
    }
}

fn resolve_one(spec: &ParameterSpec, value: Option<&ParameterValue>) -> Result<ParameterValue> {
    let name = spec.name.as_str();
    let wrong = |expected: &str| PathError::parameter(name, format!("expected {expected}"));
    match &spec.kind {
        ParameterKind::Number {
            unit,
            min,
            min_exclusive,
            max,
            default,
        } => {
            let v = match value {
                None => default.ok_or_else(|| PathError::parameter(name, "a value is required"))?,
                Some(ParameterValue::Number(v)) => *v,
                // A whole number is written without a decimal point (decision 0008 item 6).
                #[allow(clippy::cast_precision_loss)]
                Some(ParameterValue::Integer(i)) => *i as f64,
                Some(_) => return Err(wrong("a number")),
            };
            if !v.is_finite() {
                return Err(PathError::parameter(
                    name,
                    "the value is not a finite number",
                ));
            }
            let unit = unit.symbol();
            if let Some(lo) = *min
                && (v < lo || (*min_exclusive && v <= lo))
            {
                let relation = if *min_exclusive {
                    "greater than"
                } else {
                    "at least"
                };
                return Err(PathError::parameter(
                    name,
                    format!("{v} {unit} must be {relation} {lo} {unit}"),
                ));
            }
            if let Some(hi) = *max
                && v > hi
            {
                return Err(PathError::parameter(
                    name,
                    format!("{v} {unit} must be at most {hi} {unit}"),
                ));
            }
            Ok(ParameterValue::Number(v))
        }
        ParameterKind::Integer { min, max, default } => {
            let v = match value {
                None => default.ok_or_else(|| PathError::parameter(name, "a value is required"))?,
                Some(ParameterValue::Integer(i)) => *i,
                Some(_) => return Err(wrong("a whole number")),
            };
            if v < *min || v > *max {
                return Err(PathError::parameter(
                    name,
                    format!("{v} must be from {min} to {max}"),
                ));
            }
            Ok(ParameterValue::Integer(v))
        }
        ParameterKind::Flag { default } => match value {
            None => Ok(ParameterValue::Flag(*default)),
            Some(ParameterValue::Flag(b)) => Ok(ParameterValue::Flag(*b)),
            Some(_) => Err(wrong("true or false")),
        },
        ParameterKind::AngleList { default } => {
            let v = match value {
                None => default.clone(),
                Some(ParameterValue::Text(t)) => t.clone(),
                Some(_) => return Err(wrong("a list of angles in degrees")),
            };
            parse_angles(&v).map_err(|reason| PathError::parameter(name, reason))?;
            Ok(ParameterValue::Text(v))
        }
        ParameterKind::Choice { options, default } => {
            let v = match value {
                None => default.clone(),
                Some(ParameterValue::Text(t)) => t.clone(),
                Some(_) => return Err(wrong(&format!("one of {}", options.join(", ")))),
            };
            if options.contains(&v) {
                Ok(ParameterValue::Text(v))
            } else {
                Err(PathError::parameter(
                    name,
                    format!("\"{v}\" is not one of {}", options.join(", ")),
                ))
            }
        }
    }
}

/// The most angles one list may hold.
pub const MAX_ANGLES: usize = 720;

/// Reads a list of angles in degrees: numbers separated by commas,
/// semicolons or spaces. Empty text is an empty list.
pub fn parse_angles(text: &str) -> std::result::Result<Vec<f64>, String> {
    let mut out = Vec::new();
    for word in text
        .split(|c: char| c == ',' || c == ';' || c.is_whitespace())
        .filter(|w| !w.is_empty())
    {
        let v = word
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite())
            .ok_or_else(|| format!("'{word}' is not an angle in degrees"))?;
        out.push(v);
    }
    if out.len() > MAX_ANGLES {
        return Err(format!(
            "{} angles is more than the {MAX_ANGLES} allowed",
            out.len()
        ));
    }
    Ok(out)
}

impl Parameters {
    /// The list of angles called `name`, in degrees.
    pub fn angles(&self, name: &str) -> Result<Vec<f64>> {
        parse_angles(self.text(name)?).map_err(|reason| PathError::parameter(name, reason))
    }

    /// The value called `name`.
    pub fn get(&self, name: &str) -> Option<&ParameterValue> {
        self.0.get(name)
    }

    /// The number called `name`.
    pub fn number(&self, name: &str) -> Result<f64> {
        match self.0.get(name) {
            Some(ParameterValue::Number(v)) => Ok(*v),
            _ => Err(PathError::parameter(name, "expected a number")),
        }
    }

    /// The whole number called `name`.
    pub fn integer(&self, name: &str) -> Result<i64> {
        match self.0.get(name) {
            Some(ParameterValue::Integer(v)) => Ok(*v),
            _ => Err(PathError::parameter(name, "expected a whole number")),
        }
    }

    /// The flag called `name`.
    pub fn flag(&self, name: &str) -> Result<bool> {
        match self.0.get(name) {
            Some(ParameterValue::Flag(v)) => Ok(*v),
            _ => Err(PathError::parameter(name, "expected true or false")),
        }
    }

    /// The text called `name`.
    pub fn text(&self, name: &str) -> Result<&str> {
        match self.0.get(name) {
            Some(ParameterValue::Text(v)) => Ok(v),
            _ => Err(PathError::parameter(name, "expected text")),
        }
    }

    /// Every value, in name order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &ParameterValue)> {
        self.0.iter().map(|(k, v)| (k.as_str(), v))
    }
}

/// Names of the common parameters.
pub mod names {
    /// Toolpath tolerance.
    pub const TOLERANCE: &str = "tolerance";
    /// Distance between passes.
    pub const STEPOVER: &str = "stepover";
    /// Material left on the surface.
    pub const STOCK_TO_LEAVE: &str = "stock_to_leave";
    /// Cutting feed at the contact point.
    pub const FEED: &str = "feed";
    /// Plunge feed.
    pub const PLUNGE_FEED: &str = "plunge_feed";
    /// Where links retract to.
    pub const RETRACT: &str = "retract";
    /// How far above the stock a rapid approach stops.
    pub const APPROACH_DISTANCE: &str = "approach_distance";
    /// The entry style (decision record 0026).
    pub const ENTRY: &str = "entry";
    /// The descent angle of a ramp or helix entry.
    pub const RAMP_ANGLE: &str = "ramp_angle";
    /// The helix entry's circle, as a fraction of the tool diameter.
    pub const HELIX_DIAMETER: &str = "helix_diameter";
    /// The radius of a lead-in or lead-out arc; zero means the tool radius.
    pub const LEAD_RADIUS: &str = "lead_radius";
}

fn number(
    name: &str,
    label: &str,
    description: &str,
    unit: Unit,
    min: f64,
    min_exclusive: bool,
    default: Option<f64>,
) -> ParameterSpec {
    ParameterSpec {
        name: name.to_owned(),
        label: label.to_owned(),
        description: description.to_owned(),
        kind: ParameterKind::Number {
            unit,
            min: Some(min),
            min_exclusive,
            max: None,
            default,
        },
    }
}

/// The parameters every strategy has (OP-11 core). Feed, plunge feed and
/// stepover have no default here: the application fills them from the
/// tool's cutting data for the setup's material (TL-05) when the operation
/// does not set them.
pub fn common_parameters() -> Vec<ParameterSpec> {
    use names::*;
    vec![
        number(
            TOLERANCE,
            "Tolerance",
            "How far the toolpath may deviate from the ideal surface.",
            Unit::Millimetres,
            0.0,
            true,
            Some(Tolerance::DEFAULT.get().0),
        ),
        number(
            STEPOVER,
            "Stepover",
            "Distance between neighbouring passes, measured as the strategy states.",
            Unit::Millimetres,
            0.0,
            true,
            None,
        ),
        number(
            STOCK_TO_LEAVE,
            "Stock to leave",
            "Material left on the selected surfaces for a later operation.",
            Unit::Millimetres,
            0.0,
            false,
            Some(0.0),
        ),
        number(
            FEED,
            "Cutting feed",
            "Feed at the cutter contact point while cutting.",
            Unit::MillimetresPerMinute,
            0.0,
            true,
            None,
        ),
        number(
            PLUNGE_FEED,
            "Plunge feed",
            "Feed of the tool tip moving into the material.",
            Unit::MillimetresPerMinute,
            0.0,
            true,
            None,
        ),
        ParameterSpec {
            name: RETRACT.to_owned(),
            label: "Retract".to_owned(),
            description: "Links between passes retract to the safe cylinder, or to the safe Z."
                .to_owned(),
            kind: ParameterKind::Choice {
                options: vec!["clearance".to_owned(), "safe_z".to_owned()],
                default: "clearance".to_owned(),
            },
        },
        number(
            APPROACH_DISTANCE,
            "Approach distance",
            "A rapid approach stops this far above the stock; the rest is a plunge at the plunge feed.",
            Unit::Millimetres,
            0.0,
            false,
            Some(1.0),
        ),
        ParameterSpec {
            name: ENTRY.to_owned(),
            label: "Entry".to_owned(),
            description: "How the tool enters the cut: a straight plunge, a tangent arc (lead), \
                          a straight ramp, or a descending helix (decision record 0026)."
                .to_owned(),
            kind: ParameterKind::Choice {
                options: EntryStyle::ALL
                    .iter()
                    .map(|s| s.name().to_owned())
                    .collect(),
                default: EntryStyle::Plunge.name().to_owned(),
            },
        },
        ParameterSpec {
            name: RAMP_ANGLE.to_owned(),
            label: "Ramp angle".to_owned(),
            description: "Angle of descent to the horizontal for ramp and helix entry.".to_owned(),
            kind: ParameterKind::Number {
                unit: Unit::Degrees,
                min: Some(0.0),
                min_exclusive: true,
                max: Some(89.0),
                default: Some(DEFAULT_RAMP_ANGLE),
            },
        },
        number(
            HELIX_DIAMETER,
            "Helix diameter",
            "Diameter of the circle the tool centre follows in helix entry, as a fraction of the \
             tool diameter.",
            Unit::None,
            0.0,
            true,
            Some(DEFAULT_HELIX_DIAMETER),
        ),
        number(
            LEAD_RADIUS,
            "Lead radius",
            "Radius of the lead-in and lead-out arcs; 0 means the tool radius.",
            Unit::Millimetres,
            0.0,
            false,
            Some(0.0),
        ),
    ]
}

/// The default ramp and helix descent angle, in degrees (decision record
/// 0026 item 6: a starting value, not verified on a machine).
pub const DEFAULT_RAMP_ANGLE: f64 = 3.0;

/// The default helix circle, as a fraction of the tool diameter (decision
/// record 0026 item 6: a starting value, not verified on a machine).
pub const DEFAULT_HELIX_DIAMETER: f64 = 0.75;

/// How the tool enters the first cut point of a pass (OP-11; decision record
/// 0026).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryStyle {
    /// A rapid to the approach distance above the stock, then a straight
    /// plunge at the plunge feed.
    Plunge,
    /// Down beside the cut, then a quarter-circle arc tangent to the cut,
    /// with a matching arc out at the end of the pass.
    Lead,
    /// A straight descent at the ramp angle along the line of the cut.
    Ramp,
    /// A circle the tool centre follows downwards, leaving tangent to the
    /// cut.
    Helix,
}

impl EntryStyle {
    /// Every style, in the order the form offers them.
    pub const ALL: [Self; 4] = [Self::Plunge, Self::Lead, Self::Ramp, Self::Helix];

    /// The name an operation stores.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Plunge => "plunge",
            Self::Lead => "lead",
            Self::Ramp => "ramp",
            Self::Helix => "helix",
        }
    }

    fn parse(name: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|s| s.name() == name)
            .unwrap_or(Self::Plunge)
    }
}

/// The entry parameters, read from resolved values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EntryParameters {
    /// The style.
    pub style: EntryStyle,
    /// Descent angle of a ramp or helix.
    pub ramp_angle: Degrees,
    /// The helix circle as a fraction of the tool diameter.
    pub helix_diameter: f64,
    /// Radius of the lead arcs, or zero for the tool radius.
    pub lead_radius: Millimetres,
}

impl EntryParameters {
    /// A straight plunge, the entry before decision record 0026.
    pub const PLUNGE: Self = Self {
        style: EntryStyle::Plunge,
        ramp_angle: Degrees(DEFAULT_RAMP_ANGLE),
        helix_diameter: DEFAULT_HELIX_DIAMETER,
        lead_radius: Millimetres(0.0),
    };

    /// Reads the entry parameters from values resolved against a schema that
    /// includes the common parameters.
    pub fn read(p: &Parameters) -> Result<Self> {
        use names::*;
        Ok(Self {
            style: EntryStyle::parse(p.text(ENTRY)?),
            ramp_angle: Degrees(p.number(RAMP_ANGLE)?),
            helix_diameter: p.number(HELIX_DIAMETER)?,
            lead_radius: Millimetres(p.number(LEAD_RADIUS)?),
        })
    }
}

/// Where links between passes go (OP-14).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Retract {
    /// The top of the safe cylinder: the height of its radius above the axis.
    Clearance,
    /// The safe Z.
    SafeZ,
}

/// The common parameters, read from resolved values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CommonParameters {
    /// Toolpath tolerance.
    pub tolerance: Tolerance,
    /// Distance between passes.
    pub stepover: Millimetres,
    /// Material left on the surface.
    pub stock_to_leave: Millimetres,
    /// Cutting feed at the contact point.
    pub feed: FeedRate,
    /// Plunge feed.
    pub plunge_feed: FeedRate,
    /// Where links retract to.
    pub retract: Retract,
    /// How far above the stock a rapid approach stops.
    pub approach_distance: Millimetres,
}

impl CommonParameters {
    /// Reads the common parameters from values resolved against a schema
    /// that includes them.
    pub fn read(p: &Parameters) -> Result<Self> {
        use names::*;
        Ok(Self {
            tolerance: Tolerance::new(Millimetres(p.number(TOLERANCE)?))?,
            stepover: Millimetres(p.number(STEPOVER)?),
            stock_to_leave: Millimetres(p.number(STOCK_TO_LEAVE)?),
            feed: FeedRate(p.number(FEED)?),
            plunge_feed: FeedRate(p.number(PLUNGE_FEED)?),
            retract: match p.text(RETRACT)? {
                "safe_z" => Retract::SafeZ,
                _ => Retract::Clearance,
            },
            approach_distance: Millimetres(p.number(APPROACH_DISTANCE)?),
        })
    }
}
