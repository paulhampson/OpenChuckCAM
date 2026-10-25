// SPDX-License-Identifier: AGPL-3.0-or-later
//! The rotary kinematic mapping and A-axis management (OP-15; work breakdown
//! slice 5 item 3; decision record 0018 items 4 and 5).
//!
//! [`RotaryMapping`] takes the rotary axis as a parameter and never assumes
//! X: it maps a tool tip and an A angle in the model frame to coordinates
//! fixed to the machine, with the rotary axis through their origin, using the
//! one shared forward kinematics of `openchuck_types::kinematics`
//! (architecture section 5.1). Only the axis and its sense matter here; where
//! the axis sits on the machine is the post's business.
//!
//! The A-axis functions keep A continuous ([`nearest_turn`],
//! [`unwrap_continuous`]), fit a pass into the machine's range by whole turns
//! ([`fit_range`]), and count reversals ([`reversals`]). A is always kept
//! unwrapped in the IR (decision record 0001 item 1).

use openchuck_types::geometry::{Point3, UnitVec3, Vec3};
use openchuck_types::kinematics::{
    MachinePosition, RotaryFrame, machine_xyz, tool_axis_in_model, tool_tip_in_model,
};
use openchuck_types::machine::{ARange, ASense, MachineProfile, RotaryAxis};
use openchuck_types::units::{Degrees, Millimetres};

use crate::cutter::ToolLine;
use crate::error::{PathError, Result};

/// Degrees in one turn.
pub const TURN: f64 = 360.0;

/// The rotary axis as the pathing library sees it: which machine axis it is
/// parallel to, which way positive A turns the part, and the allowed travel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RotaryMapping {
    /// The machine axis the rotary axis is parallel to.
    pub axis: RotaryAxis,
    /// Direction of positive A.
    pub sense: ASense,
    /// Allowed A travel.
    pub range: ARange,
}

/// A position in coordinates fixed to the machine with the origin on the
/// rotary axis at model X = 0: `height` is along the tool axis (machine Z),
/// `along` is along the rotary axis and `across` is the third direction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AxisPosition {
    /// Along the rotary axis.
    pub along: f64,
    /// Across the rotary axis, perpendicular to the tool axis.
    pub across: f64,
    /// Height above the rotary axis, along the tool axis.
    pub height: f64,
}

impl RotaryMapping {
    /// The mapping of a machine profile.
    pub fn of_profile(profile: &MachineProfile) -> Self {
        Self {
            axis: profile.rotary_axis,
            sense: profile.a_sense,
            range: profile.a_range,
        }
    }

    fn frame(&self) -> RotaryFrame {
        RotaryFrame {
            axis: self.axis,
            sense: self.sense,
            origin: Point3::ORIGIN,
        }
    }

    /// The fixed-frame unit vectors along, across and up for this axis.
    fn basis(&self) -> (Vec3, Vec3, Vec3) {
        match self.axis {
            RotaryAxis::Y => (
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(-1.0, 0.0, 0.0),
                Vec3::new(0.0, 0.0, 1.0),
            ),
            _ => (
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(0.0, 0.0, 1.0),
            ),
        }
    }

    /// The tool axis (from tip to spindle) in the model frame at angle `a`.
    pub fn tool_axis(&self, a: Degrees) -> UnitVec3 {
        tool_axis_in_model(&self.frame(), a)
    }

    /// Where the model point `tip` is, fixed to the machine, at angle `a`.
    pub fn to_axis(&self, tip: Point3, a: Degrees) -> AxisPosition {
        let p = machine_xyz(&self.frame(), tip, a).to_vec();
        let (along, across, up) = self.basis();
        AxisPosition {
            along: p.dot(along),
            across: p.dot(across),
            height: p.dot(up),
        }
    }

    /// The model point at the fixed position `position` at angle `a`.
    pub fn to_model(&self, position: AxisPosition, a: Degrees) -> Point3 {
        let (along, across, up) = self.basis();
        let xyz = Point3::ORIGIN
            + along * position.along
            + across * position.across
            + up * position.height;
        tool_tip_in_model(&self.frame(), &MachinePosition { xyz, a })
    }

    /// The line a vertical tool moves along at model X = `x`, angle `a`, and
    /// `across` from the rotary axis (zero for a tool over the axis).
    pub fn tool_line(&self, x: Millimetres, a: Degrees, across: Millimetres) -> ToolLine {
        let origin = self.to_model(
            AxisPosition {
                along: x.0,
                across: across.0,
                height: 0.0,
            },
            a,
        );
        ToolLine {
            origin,
            axis: self.tool_axis(a),
        }
    }

    /// The A angle at which the model direction `normal` (perpendicular
    /// component only; its part along the rotary axis cannot be followed)
    /// faces the tool, as the tool axis policy of decision record 0015 item
    /// 3 asks. `None` when `normal` is along the rotary axis.
    pub fn a_facing(&self, normal: Vec3) -> Option<Degrees> {
        // The tool axis in the model frame at A is R(-sign A) * up; with the
        // in-plane normal at polar angle phi (from +Y towards +Z) and up at
        // polar angle 90, the axis lies at 90 - sign * A.
        // The model X axis is the rotary axis whatever machine axis it is
        // parallel to, so the in-plane part is always (y, z).
        let (y, z) = (normal.y, normal.z);
        if y.abs() < 1e-15 && z.abs() < 1e-15 {
            return None;
        }
        let phi = z.atan2(y).to_degrees();
        Some(Degrees(self.sense.sign() * (90.0 - phi)))
    }
}

/// The angle equal to `a` up to whole turns that is nearest `previous`, so a
/// move from `previous` never turns more than half a turn.
pub fn nearest_turn(a: Degrees, previous: Degrees) -> Degrees {
    let turns = ((previous.0 - a.0) / TURN).round();
    Degrees(a.0 + turns * TURN)
}

/// Makes a sequence of angles continuous: each is replaced by its equivalent
/// nearest the one before, starting from the equivalent of the first nearest
/// `start`.
pub fn unwrap_continuous(angles: &[Degrees], start: Degrees) -> Vec<Degrees> {
    let mut previous = start;
    angles
        .iter()
        .map(|a| {
            let next = nearest_turn(*a, previous);
            previous = next;
            next
        })
        .collect()
}

/// Whole turns to add to every angle of a continuous pass so that it lies in
/// `range` with its start as near `previous` as possible. Fails, naming the
/// pass, when the pass spans more than the range allows (OP-15): the axis
/// cannot unwind in the middle of a cut.
pub fn fit_range(pass: &[Degrees], range: ARange, previous: Degrees, name: &str) -> Result<f64> {
    let Some(first) = pass.first() else {
        return Ok(0.0);
    };
    let preferred = ((previous.0 - first.0) / TURN).round();
    let ARange::Limited { min, max } = range else {
        return Ok(preferred * TURN);
    };
    let lo = pass.iter().map(|a| a.0).fold(f64::INFINITY, f64::min);
    let hi = pass.iter().map(|a| a.0).fold(f64::NEG_INFINITY, f64::max);
    if hi - lo > max.0 - min.0 {
        return Err(PathError::RotaryLimit(format!(
            "{name} turns A through {:.3} degrees, more than the machine's range of {:.3} \
             ({} to {})",
            hi - lo,
            max.0 - min.0,
            min,
            max
        )));
    }
    // Turns k with min <= lo + k*TURN and hi + k*TURN <= max.
    let k_lo = ((min.0 - lo) / TURN).ceil();
    let k_hi = ((max.0 - hi) / TURN).floor();
    if k_lo > k_hi {
        return Err(PathError::RotaryLimit(format!(
            "{name} cannot be placed inside the machine's A range ({min} to {max}) by whole turns"
        )));
    }
    Ok(preferred.clamp(k_lo, k_hi) * TURN)
}

/// Number of times A changes direction along `angles`, ignoring steps
/// smaller than `dead_band` degrees (OP-15: reversals are minimised).
pub fn reversals(angles: &[Degrees], dead_band: f64) -> usize {
    let mut count = 0;
    let mut direction: i8 = 0;
    let mut anchor = match angles.first() {
        Some(a) => a.0,
        None => return 0,
    };
    for a in angles.iter().skip(1) {
        let step = a.0 - anchor;
        if step.abs() <= dead_band {
            continue;
        }
        let sign: i8 = if step > 0.0 { 1 } else { -1 };
        if direction != 0 && sign != direction {
            count += 1;
        }
        direction = sign;
        anchor = a.0;
    }
    count
}

/// Whole turns that bring `a` into `(-180, 180]`, as a legacy utility.
/// Shared assembly retains accumulated A at boundaries instead (decision 0238).
pub fn unwind_turns(a: Degrees) -> f64 {
    let wrapped = a.0 - TURN * ((a.0 + 180.0) / TURN).floor();
    // `wrapped` is in [-180, 180); keep +180 rather than -180.
    let wrapped = if wrapped <= -180.0 {
        wrapped + TURN
    } else {
        wrapped
    };
    wrapped - a.0
}
