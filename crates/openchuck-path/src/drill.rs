// SPDX-License-Identifier: AGPL-3.0-or-later
//! Radial drilling at indexed A positions (OP-10; work breakdown slice 18;
//! decision record 0051).
//!
//! The holes are the round holes the surface reports
//! ([`SurfaceSource::holes`](crate::SurfaceSource::holes), found from
//! cylindrical faces of a B-rep model by [`crate::holes`]), so the user
//! selects a hole by selecting a face of its wall, or drills every hole of
//! the model. Each hole is one pass along its own axis:
//!
//! 1. **Axis to A.** The tool axis is fixed on the machine's Z, so the hole's
//!    axis must be radial (perpendicular to the rotary axis). A is the angle
//!    at which the direction out of the hole faces the tool
//!    ([`RotaryMapping::a_facing`]); the tool is held over the hole's own
//!    axis line, so a hole whose axis does not meet the rotary axis is drilled
//!    with the tool offset across it. A hole tilted away from the radial
//!    plane by more than `axis_tolerance` cannot be drilled and is left out
//!    with a note.
//! 2. **Size.** A hole is drilled by a tool of its diameter, within
//!    `diameter_tolerance`; others are left out with a note (a larger value
//!    drills them as a pilot).
//! 3. **Depth.** A through hole is drilled to its length plus the length of
//!    the tool's point plus `through_allowance`, so the full diameter breaks
//!    through. A blind hole reaches its full diameter at the model's depth
//!    (`depth_reference` `full_diameter`) or puts the tool tip at the deepest
//!    point of the hole (`tip`).
//! 4. **Moves, expanded.** The tool rapids to the retract height above the
//!    top of the hole (the stock's surface on the axis, if higher than the
//!    opening), then feeds to the first peck. With a `peck_depth` the depth is
//!    divided into equal pecks no deeper than it. After each peck the tool
//!    either retracts to the retract height and rapids back to
//!    `reentry_clearance` above the last depth (`full_retract`, chips
//!    cleared) or retracts `chip_break_retract` and feeds on
//!    (`chip_break`). Canned cycles are a post option and not output here.
//! 5. **Order.** Holes at the same A are cut together, along X, alternating
//!    direction from one angle to the next. The angles are visited in one
//!    sweep that starts after the largest gap between them, so A turns no
//!    further than the whole turn less that gap (within the machine's A range
//!    when it is limited, from its lowest end).

use openchuck_types::geometry::{Point3, UnitVec3};
use openchuck_types::job::JobContext;
use openchuck_types::machine::ARange;
use openchuck_types::tool::profile::RevolvedProfile;
use openchuck_types::units::Degrees;

use crate::error::{PathError, Result};
use crate::holes::{Hole, HoleEnd};
use crate::params::{
    EntryStyle, ParameterKind, ParameterSchema, ParameterSpec, Parameters, Unit, names,
};
use crate::rotary::TURN;
use crate::strategy::{CutPoint, Pass, Strategy, StrategyGroup, StrategyInput, StrategyOutput};

/// Radial drilling of the holes of the model.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RadialDrilling;

/// The registry name of [`RadialDrilling`].
pub const NAME: &str = "radial_drilling";

/// Parameter names specific to radial drilling.
pub mod parameter {
    /// The deepest one peck goes; zero drills each hole in one move.
    pub const PECK_DEPTH: &str = "peck_depth";
    /// What the tool does between pecks.
    pub const PECK_MODE: &str = "peck_mode";
    /// How far the tool backs out between pecks in `chip_break` mode.
    pub const CHIP_BREAK_RETRACT: &str = "chip_break_retract";
    /// How far above the top of the hole the tool retracts to.
    pub const RETRACT_HEIGHT: &str = "retract_height";
    /// How far above the last depth a rapid return stops, in `full_retract`.
    pub const REENTRY_CLEARANCE: &str = "reentry_clearance";
    /// How far past the end of a through hole the full diameter goes.
    pub const THROUGH_ALLOWANCE: &str = "through_allowance";
    /// What the depth of a blind hole means for the tool.
    pub const DEPTH_REFERENCE: &str = "depth_reference";
    /// How far a hole's diameter may differ from the tool's.
    pub const DIAMETER_TOLERANCE: &str = "diameter_tolerance";
    /// How far a hole's axis may lie from the radial plane, in degrees.
    pub const AXIS_TOLERANCE: &str = "axis_tolerance";
}

/// The choices of `peck_mode`.
const FULL_RETRACT: &str = "full_retract";
const CHIP_BREAK: &str = "chip_break";
/// The choices of `depth_reference`.
const FULL_DIAMETER: &str = "full_diameter";
const TIP: &str = "tip";

/// Angles closer than this, in degrees, are the same A position.
const SAME_ANGLE: f64 = 1e-6;

fn number(
    name: &str,
    label: &str,
    description: &str,
    unit: Unit,
    max: Option<f64>,
    default: f64,
) -> ParameterSpec {
    ParameterSpec {
        name: name.to_owned(),
        label: label.to_owned(),
        description: description.to_owned(),
        kind: ParameterKind::Number {
            unit,
            min: Some(0.0),
            min_exclusive: false,
            max,
            default: Some(default),
        },
    }
}

fn choice(name: &str, label: &str, description: &str, options: &[&str]) -> ParameterSpec {
    ParameterSpec {
        name: name.to_owned(),
        label: label.to_owned(),
        description: description.to_owned(),
        kind: ParameterKind::Choice {
            options: options.iter().map(|o| (*o).to_owned()).collect(),
            default: options.first().copied().unwrap_or_default().to_owned(),
        },
    }
}

impl Strategy for RadialDrilling {
    fn name(&self) -> &str {
        NAME
    }

    fn label(&self) -> &str {
        "Radial drilling"
    }

    fn description(&self) -> &str {
        "Drills holes straight into the part from the side, turning the part so each hole faces the tool. It needs a STEP model so it can find the round holes."
    }

    fn group(&self) -> StrategyGroup {
        StrategyGroup::Specialist
    }

    fn needs_brep_part(&self) -> Option<&str> {
        Some("it finds round holes in the cylindrical faces of a STEP model, and a mesh has none")
    }

    fn schema(&self) -> ParameterSchema {
        use parameter::*;
        let mut schema = ParameterSchema::with_common(vec![
            number(
                PECK_DEPTH,
                "Peck depth",
                "The deepest one peck goes, from the top of the hole. Zero drills each hole in \
                 one move.",
                Unit::Millimetres,
                None,
                0.0,
            ),
            choice(
                PECK_MODE,
                "Peck mode",
                "Between pecks the tool retracts to the retract height and rapids back \
                 (full_retract), or backs out a short way and feeds on (chip_break).",
                &[FULL_RETRACT, CHIP_BREAK],
            ),
            number(
                CHIP_BREAK_RETRACT,
                "Chip-break retract",
                "How far the tool backs out between pecks in chip_break mode.",
                Unit::Millimetres,
                None,
                0.5,
            ),
            number(
                RETRACT_HEIGHT,
                "Retract height",
                "How far above the top of the hole (the opening, or the stock's surface on its \
                 axis if higher) the tool retracts to and starts feeding from.",
                Unit::Millimetres,
                None,
                2.0,
            ),
            number(
                REENTRY_CLEARANCE,
                "Re-entry clearance",
                "In full_retract mode a rapid return into the hole stops this far above the \
                 depth of the last peck, and the tool feeds the rest.",
                Unit::Millimetres,
                None,
                0.5,
            ),
            number(
                THROUGH_ALLOWANCE,
                "Through allowance",
                "How far beyond the end of a through hole the tool's full diameter goes, on top \
                 of the length of its point.",
                Unit::Millimetres,
                None,
                0.5,
            ),
            choice(
                DEPTH_REFERENCE,
                "Blind hole depth",
                "A blind hole is drilled until the tool's full diameter reaches the hole's depth \
                 (full_diameter), or until the tool tip reaches the deepest point of the hole \
                 (tip).",
                &[FULL_DIAMETER, TIP],
            ),
            number(
                DIAMETER_TOLERANCE,
                "Diameter tolerance",
                "A hole is drilled only if its diameter is within this of the tool's. Raise it \
                 to drill larger holes as a pilot.",
                Unit::Millimetres,
                None,
                0.1,
            ),
            number(
                AXIS_TOLERANCE,
                "Axis tolerance",
                "A hole whose axis is tilted from the radial plane by more than this cannot be \
                 drilled on four axes and is left out.",
                Unit::Degrees,
                Some(45.0),
                0.5,
            ),
        ]);
        for spec in &mut schema.parameters {
            match spec.name.as_str() {
                names::STEPOVER => {
                    spec.description = "Not used: the holes set where the passes are.".to_owned();
                    if let ParameterKind::Number { default, .. } = &mut spec.kind {
                        *default = Some(1.0);
                    }
                }
                names::STOCK_TO_LEAVE => {
                    spec.description = "Not used: a drilled hole is cut to its size.".to_owned();
                }
                names::ENTRY => {
                    spec.description =
                        "A drill enters straight down; this has no other choice.".to_owned();
                    spec.kind = ParameterKind::Choice {
                        options: vec![EntryStyle::Plunge.name().to_owned()],
                        default: EntryStyle::Plunge.name().to_owned(),
                    };
                }
                names::RAMP_ANGLE | names::HELIX_DIAMETER | names::LEAD_RADIUS => {
                    spec.description = "Not used: a drill enters straight down.".to_owned();
                }
                _ => {}
            }
        }
        schema
    }

    fn passes_with_notes(
        &self,
        input: &StrategyInput<'_>,
        region: Option<&crate::region::RegionConstraint<'_>>,
        parameters: &Parameters,
        context: &JobContext<'_>,
    ) -> Result<StrategyOutput> {
        if region.is_some() {
            return Err(PathError::Strategy(
                "radial drilling takes its holes from the selected faces, not from a region"
                    .to_owned(),
            ));
        }
        drill(input, parameters, context)
    }

    fn passes(
        &self,
        input: &StrategyInput<'_>,
        parameters: &Parameters,
        context: &JobContext<'_>,
    ) -> Result<Vec<Pass>> {
        Ok(drill(input, parameters, context)?.passes)
    }
}

/// The height of the tool's point: from the tip up to where the full
/// diameter starts.
fn point_height(tool: &RevolvedProfile) -> f64 {
    tool.height_at(tool.cutting_radius()).unwrap_or(0.0)
}

/// A hole that will be drilled, placed on the A circle.
struct Site<'a> {
    hole: &'a Hole,
    /// Its number in the surface's list, from 1, for messages.
    number: usize,
    /// A in `[0, 360)`.
    a: f64,
}

fn describe(site: &Site<'_>) -> String {
    format!(
        "hole {} (face {}, diameter {:.3} mm)",
        site.number,
        site.hole.faces.first().map_or(0, |f| f.0),
        site.hole.diameter
    )
}

fn drill(
    input: &StrategyInput<'_>,
    parameters: &Parameters,
    context: &JobContext<'_>,
) -> Result<StrategyOutput> {
    let holes = input.surface.holes();
    if holes.is_empty() {
        return Err(PathError::Strategy(
            "there is no hole to drill: radial drilling finds round holes in the cylindrical \
             faces of a STEP model, and the selection has none (a mesh has no holes to find)"
                .to_owned(),
        ));
    }
    let tool_diameter = 2.0 * input.tool.cutting_radius();
    let diameter_tolerance = parameters.number(parameter::DIAMETER_TOLERANCE)?;
    let axis_tolerance = parameters.number(parameter::AXIS_TOLERANCE)?;
    let mut notes = Vec::new();
    let mut sites = Vec::new();
    for (i, hole) in holes.iter().enumerate() {
        let mut site = Site {
            hole,
            number: i + 1,
            a: 0.0,
        };
        let d = hole.direction.get();
        let tilt = d.x.clamp(-1.0, 1.0).abs().asin().to_degrees();
        if tilt > axis_tolerance {
            notes.push(format!(
                "{} is left out: its axis is {tilt:.2} degrees from the radial plane, more than \
                 the axis tolerance of {axis_tolerance} degrees, and a four-axis machine cannot \
                 tilt the tool",
                describe(&site)
            ));
            continue;
        }
        if (hole.diameter - tool_diameter).abs() > diameter_tolerance {
            notes.push(format!(
                "{} is left out: the tool is {tool_diameter:.3} mm, more than the diameter \
                 tolerance of {diameter_tolerance} mm from it",
                describe(&site)
            ));
            continue;
        }
        let Some(a) = input.rotary.a_facing(d) else {
            continue;
        };
        site.a = a.0.rem_euclid(TURN);
        sites.push(site);
    }
    if sites.is_empty() {
        return Err(PathError::Strategy(format!(
            "no hole can be drilled with this tool: {}",
            notes.join("; ")
        )));
    }
    context.check_cancelled()?;

    let order = sweep(&sites, input.rotary.range)?;
    let mut passes = Vec::new();
    for (group, (a, members)) in order.iter().enumerate() {
        let mut members: Vec<&Site<'_>> = members.to_vec();
        members.sort_by(|p, q| {
            p.hole
                .entry
                .x
                .total_cmp(&q.hole.entry.x)
                .then(p.number.cmp(&q.number))
        });
        if group % 2 == 1 {
            members.reverse();
        }
        for site in members {
            context.check_cancelled()?;
            passes.push(hole_pass(input, parameters, site, Degrees(*a))?);
        }
    }
    Ok(StrategyOutput {
        passes,
        notes,
        warnings: Vec::new(),
    })
}

/// The A positions in the order to visit them, each with the holes there.
/// Unwrapped, so the angles rise through the sweep.
fn sweep<'a, 's>(sites: &'s [Site<'a>], range: ARange) -> Result<Vec<(f64, Vec<&'s Site<'a>>)>> {
    let mut sorted: Vec<&Site<'a>> = sites.iter().collect();
    sorted.sort_by(|p, q| p.a.total_cmp(&q.a).then(p.number.cmp(&q.number)));
    let mut groups: Vec<(f64, Vec<&Site<'a>>)> = Vec::new();
    for site in sorted {
        match groups.last_mut() {
            Some((a, members)) if site.a - *a <= SAME_ANGLE => members.push(site),
            _ => groups.push((site.a, vec![site])),
        }
    }
    // An angle just under a whole turn is the same position as one just over
    // zero.
    if groups.len() > 1
        && let (Some(first), Some(last)) = (groups.first().map(|g| g.0), groups.last().map(|g| g.0))
        && first + TURN - last <= SAME_ANGLE
        && let Some((_, extra)) = groups.pop()
        && let Some((_, members)) = groups.first_mut()
    {
        members.extend(extra);
    }
    if let ARange::Limited { min, max } = range {
        for (a, members) in &mut groups {
            let shifted = *a + TURN * ((min.0 - *a) / TURN).ceil();
            if shifted > max.0 + 1e-9 {
                let which = members
                    .iter()
                    .map(|s| describe(s))
                    .collect::<Vec<_>>()
                    .join(", ");
                return Err(PathError::RotaryLimit(format!(
                    "{which} needs A = {:.3} degrees, which the machine's range of {min} to {max} \
                     does not reach",
                    *a
                )));
            }
            *a = shifted;
        }
        groups.sort_by(|p, q| p.0.total_cmp(&q.0));
        return Ok(groups);
    }
    // Start after the largest gap, so the sweep leaves that gap uncrossed.
    let n = groups.len();
    let mut start = 0;
    let mut widest = f64::NEG_INFINITY;
    for i in 0..n {
        let here = groups.get(i).map_or(0.0, |g| g.0);
        let next = groups.get((i + 1) % n).map_or(0.0, |g| g.0);
        let gap = if i + 1 == n {
            next + TURN - here
        } else {
            next - here
        };
        if gap > widest + SAME_ANGLE {
            widest = gap;
            start = (i + 1) % n;
        }
    }
    groups.rotate_left(start);
    let mut previous = f64::NEG_INFINITY;
    for (a, _) in &mut groups {
        while *a < previous {
            *a += TURN;
        }
        previous = *a;
    }
    Ok(groups)
}

/// The pass that drills `site`, with A at `a`.
fn hole_pass(
    input: &StrategyInput<'_>,
    parameters: &Parameters,
    site: &Site<'_>,
    a: Degrees,
) -> Result<Pass> {
    let hole = site.hole;
    let peck_depth = parameters.number(parameter::PECK_DEPTH)?;
    let break_out = parameters.number(parameter::CHIP_BREAK_RETRACT)?;
    let clearance = parameters.number(parameter::RETRACT_HEIGHT)?;
    let reentry = parameters.number(parameter::REENTRY_CLEARANCE)?;
    let allowance = parameters.number(parameter::THROUGH_ALLOWANCE)?;
    let chip_break = parameters.text(parameter::PECK_MODE)? == CHIP_BREAK;
    let by_tip = parameters.text(parameter::DEPTH_REFERENCE)? == TIP;

    // The tool axis, from the tip towards the spindle, is the way out of the
    // hole; the hole's own axis line carries the tool.
    let out = input.rotary.tool_axis(a);
    let point = point_height(input.tool);
    let tip_depth = match hole.end {
        HoleEnd::Through => hole.length + point + allowance,
        HoleEnd::Blind { tip_depth } if by_tip => tip_depth,
        HoleEnd::Blind { .. } => hole.length + point,
    };
    let top = stock_top(input, hole.entry, out).max(0.0);
    let total = top + tip_depth;
    if !(total.is_finite() && total > 0.0) {
        return Err(PathError::Strategy(format!(
            "{} has no depth to drill",
            describe(site)
        )));
    }
    let at = |depth: f64| -> CutPoint {
        let tip = hole.entry + out.get() * (top - depth);
        CutPoint {
            tip,
            a,
            contact: tip,
        }
    };

    let pecks = if peck_depth > 0.0 {
        (total / peck_depth).ceil().max(1.0)
    } else {
        1.0
    };
    if pecks > 100_000.0 {
        return Err(PathError::parameter(
            parameter::PECK_DEPTH,
            format!(
                "{peck_depth} mm would make {pecks} pecks in {}",
                describe(site)
            ),
        ));
    }
    let pecks = pecks as usize;
    let mut points = vec![at(-clearance)];
    let mut rapids = Vec::new();
    let depth_of = |k: usize| total * k as f64 / pecks as f64;
    for k in 1..=pecks {
        if k > 1 {
            let last = depth_of(k - 1);
            if chip_break {
                rapids.push(points.len());
                points.push(at((last - break_out).max(-clearance)));
            } else {
                rapids.push(points.len());
                points.push(at(-clearance));
                rapids.push(points.len());
                points.push(at((last - reentry).max(-clearance)));
            }
        }
        points.push(at(depth_of(k)));
    }
    Ok(Pass {
        points,
        stock: None,
        rapids,
    })
}

/// How far above `entry`, along `out`, the stock reaches on the hole's axis,
/// or zero when it does not.
fn stock_top(input: &StrategyInput<'_>, entry: Point3, out: UnitVec3) -> f64 {
    let reach = input
        .stock
        .max_radius()
        .unwrap_or(input.clearance.radius.0)
        .max(input.clearance.radius.0);
    let far = entry.to_vec().length() + reach + 1.0;
    let origin = entry + out.get() * far;
    let Ok(back) = UnitVec3::new(-out.get()) else {
        return 0.0;
    };
    match input.stock.ray_entry(origin, back, 2.0 * far) {
        Some(t) if t > 1e-9 => (far - t).max(0.0),
        _ => 0.0,
    }
}
