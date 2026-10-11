// SPDX-License-Identifier: AGPL-3.0-or-later
//! The one shared forward-kinematics implementation (architecture section 5.1).
//!
//! The path library, the post and the G-code reader all use these functions,
//! so a mistake here would be consistent everywhere. An independent
//! closed-form oracle in `openchuck-testkit`, which shares no code with this
//! module, checks it (architecture section 9, work breakdown slice 0 item 11).
//!
//! # Frames (decision records 0004 item 5 and 0011)
//!
//! - The **model frame** (part frame) is fixed to the part. Its X axis is the
//!   rotary axis and its origin lies on that axis, whatever machine axis the
//!   rotary axis is parallel to. The neutral toolpath IR is written in this
//!   frame (decision record 0001, item 1).
//! - A **fixed frame** is fixed to the machine, with the tool axis along +Z:
//!   the **machine frame** (machine coordinates) or a **work frame** (the
//!   coordinates a program is written in, such as G54). A [`RotaryFrame`]
//!   says where the model frame sits in one of them at A = 0: the fixed
//!   coordinates of the model origin, and which machine axis the model X
//!   axis lies along. A rotation of A degrees turns the part by `sign * A`
//!   about the rotary axis, where `sign` comes from [`ASense`].
//!
//! With `R(t)` the right-handed rotation by `t` about the model X axis and `B`
//! the fixed rotation that takes model X onto the rotary axis's machine
//! direction (the identity for X; a quarter turn about Z for Y, so model X
//! becomes machine Y, model Y becomes machine -X and Z stays Z):
//!
//! ```text
//! fixed_xyz = origin + B * R(sign * A) * tip_model
//! tip_model = R(-sign * A) * B^-1 * (fixed_xyz - origin)
//! ```
//!
//! [`RotaryFrame::machine`] and [`RotaryFrame::work`] build the two frames of
//! a setup from the machine profile and the setup's position data.

use crate::geometry::{Point3, UnitVec3, Vec3};
use crate::machine::{ASense, MachineProfile, RotaryAxis};
use crate::setup::SetupFrame;
use crate::units::{Degrees, Millimetres};

/// A machine position: linear axes in fixed coordinates and the unwrapped A angle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MachinePosition {
    /// X, Y and Z in millimetres.
    pub xyz: Point3,
    /// A in degrees, unwrapped.
    pub a: Degrees,
}

/// Where the model frame sits in a fixed frame at A = 0, and how it turns.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RotaryFrame {
    /// The machine axis the rotary axis is parallel to.
    pub axis: RotaryAxis,
    /// Direction of positive A.
    pub sense: ASense,
    /// Fixed coordinates of the model origin at A = 0.
    pub origin: Point3,
}

impl RotaryFrame {
    /// The machine frame with the model origin at the profile's rotary
    /// reference point, as if the part offset were zero.
    pub fn of_profile(profile: &MachineProfile) -> Self {
        Self {
            axis: profile.rotary_axis,
            sense: profile.a_sense,
            origin: profile.rotary_origin,
        }
    }

    /// The machine frame of a setup: the model origin lies on the rotary
    /// axis, `part_offset` along it from the profile's rotary reference point.
    pub fn machine(profile: &MachineProfile, setup: &SetupFrame) -> Self {
        let mut frame = Self::of_profile(profile);
        frame.origin = frame.origin + axis_direction(frame.axis) * setup.part_offset.0;
        frame
    }

    /// The work frame of a setup: the coordinates its program is written in.
    /// The work zero lies on the rotary axis at model X = `work_offset.x`
    /// (decision record 0011, OI-40), so the model origin sits at
    /// `-work_offset.x` along the axis.
    pub fn work(profile: &MachineProfile, setup: &SetupFrame) -> Self {
        Self {
            axis: profile.rotary_axis,
            sense: profile.a_sense,
            origin: Point3::ORIGIN + axis_direction(profile.rotary_axis) * -setup.work_offset.x.0,
        }
    }

    /// The fixed coordinates of the model point on the rotary axis at model
    /// X = `x`. It does not move with A.
    pub fn on_axis(&self, x: Millimetres) -> Point3 {
        self.origin + axis_direction(self.axis) * x.0
    }
}

/// Something that defines a [`RotaryFrame`]: a frame itself, or a machine
/// profile (the frame with a zero part offset).
pub trait AsRotaryFrame {
    /// The frame.
    fn rotary_frame(&self) -> RotaryFrame;
}

impl AsRotaryFrame for RotaryFrame {
    fn rotary_frame(&self) -> RotaryFrame {
        *self
    }
}

impl AsRotaryFrame for MachineProfile {
    fn rotary_frame(&self) -> RotaryFrame {
        RotaryFrame::of_profile(self)
    }
}

/// The machine direction of the rotary axis (`B * X`).
pub fn axis_direction(axis: RotaryAxis) -> Vec3 {
    match axis {
        RotaryAxis::X => Vec3::new(1.0, 0.0, 0.0),
        RotaryAxis::Y => Vec3::new(0.0, 1.0, 0.0),
    }
}

/// `B`: takes model directions to machine directions at A = 0.
fn to_fixed(axis: RotaryAxis, v: Vec3) -> Vec3 {
    match axis {
        RotaryAxis::X => v,
        RotaryAxis::Y => Vec3::new(-v.y, v.x, v.z),
    }
}

/// `B^-1`.
fn to_model(axis: RotaryAxis, v: Vec3) -> Vec3 {
    match axis {
        RotaryAxis::X => v,
        RotaryAxis::Y => Vec3::new(v.y, -v.x, v.z),
    }
}

/// Rotates `v` by `angle` about the model X axis (right-handed).
fn rotate_about_x(v: Vec3, angle: Degrees) -> Vec3 {
    let (s, c) = angle.to_radians().sin_cos();
    Vec3::new(v.x, c * v.y - s * v.z, s * v.y + c * v.z)
}

/// Forward kinematics: the tool tip in the model frame for a position in the
/// frame's fixed coordinates.
pub fn tool_tip_in_model(frame: &impl AsRotaryFrame, position: &MachinePosition) -> Point3 {
    let frame = frame.rotary_frame();
    let relative = to_model(frame.axis, position.xyz - frame.origin);
    let angle = Degrees(-frame.sense.sign() * position.a.0);
    Point3::ORIGIN + rotate_about_x(relative, angle)
}

/// Inverse kinematics for a known A: the fixed XYZ that puts the tool tip at
/// `tip` (model frame) when the rotary axis is at `a`.
pub fn machine_xyz(frame: &impl AsRotaryFrame, tip: Point3, a: Degrees) -> Point3 {
    let frame = frame.rotary_frame();
    let angle = Degrees(frame.sense.sign() * a.0);
    frame.origin + to_fixed(frame.axis, rotate_about_x(tip.to_vec(), angle))
}

/// The tool axis (machine +Z) expressed in the model frame at rotary angle `a`.
pub fn tool_axis_in_model(frame: &impl AsRotaryFrame, a: Degrees) -> UnitVec3 {
    let frame = frame.rotary_frame();
    let angle = Degrees(-frame.sense.sign() * a.0);
    let v = rotate_about_x(to_model(frame.axis, UnitVec3::Z.get()), angle);
    // A rotation of a unit vector has length one up to rounding; fall back to
    // +Z only if `a` was not finite.
    UnitVec3::new(v).unwrap_or(UnitVec3::Z)
}

/// The machine direction a part direction faces at rotary angle `a`. For
/// example, the side of the part facing machine +Z at A = 0 faces
/// `facing_at(frame, B^-1 * Z, 90)` at A = 90. Used to state the A direction
/// in words (work breakdown slice 3 review checklist).
pub fn facing_at(frame: &impl AsRotaryFrame, model_direction: Vec3, a: Degrees) -> Vec3 {
    let frame = frame.rotary_frame();
    let angle = Degrees(frame.sense.sign() * a.0);
    to_fixed(frame.axis, rotate_about_x(model_direction, angle))
}

/// The model direction that faces machine +Z (up, toward the tool) at A = 0.
pub fn model_up(frame: &impl AsRotaryFrame) -> Vec3 {
    to_model(frame.rotary_frame().axis, UnitVec3::Z.get())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn req_nfr_05_kinematics_round_trip_in_double_precision() {
        let mut profile = MachineProfile::example_x_rotary();
        for axis in [RotaryAxis::X, RotaryAxis::Y] {
            profile.rotary_axis = axis;
            for sense in [ASense::Positive, ASense::Negative] {
                profile.a_sense = sense;
                for a in [-725.0, -90.0, -0.5, 0.0, 33.3, 359.999, 360.0, 1080.25] {
                    let tip = Point3::new(12.0, -7.5, 18.25);
                    let xyz = machine_xyz(&profile, tip, Degrees(a));
                    let back = tool_tip_in_model(&profile, &MachinePosition { xyz, a: Degrees(a) });
                    assert!(
                        back.distance(tip) < 1e-9,
                        "{axis:?} a = {a}, error {}",
                        back.distance(tip)
                    );
                }
            }
        }
    }

    #[test]
    fn tool_axis_is_vertical_at_zero_and_turns_with_a() {
        let profile = MachineProfile::example_x_rotary();
        let axis = tool_axis_in_model(&profile, Degrees(0.0)).get();
        assert!((axis - UnitVec3::Z.get()).length() < 1e-12);
        // Positive sense: the part turns +90 degrees about X, so seen from the
        // part the tool has turned -90 degrees: R(-90) * Z = +Y.
        let axis = tool_axis_in_model(&profile, Degrees(90.0)).get();
        assert!(
            (axis - Vec3::new(0.0, 1.0, 0.0)).length() < 1e-12,
            "{axis:?}"
        );
    }

    #[test]
    fn base_rotation_is_proper_and_inverts() {
        for axis in [RotaryAxis::X, RotaryAxis::Y] {
            let ex = to_fixed(axis, Vec3::new(1.0, 0.0, 0.0));
            let ey = to_fixed(axis, Vec3::new(0.0, 1.0, 0.0));
            let ez = to_fixed(axis, Vec3::new(0.0, 0.0, 1.0));
            assert!(
                (ex.cross(ey) - ez).length() < 1e-15,
                "{axis:?} is not right-handed"
            );
            assert!((ex - axis_direction(axis)).length() < 1e-15);
            let v = Vec3::new(1.5, -2.0, 3.25);
            assert!((to_model(axis, to_fixed(axis, v)) - v).length() < 1e-15);
        }
    }
}
