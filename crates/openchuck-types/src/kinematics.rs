// SPDX-License-Identifier: AGPL-3.0-or-later
//! The one shared forward-kinematics implementation (architecture section 5.1).
//!
//! The path library, the post and the G-code reader all use these functions,
//! so a mistake here would be consistent everywhere. An independent
//! closed-form oracle in `openchuck-testkit`, which shares no code with this
//! module, checks it (architecture section 9, work breakdown slice 0 item 11).
//!
//! # Frames
//!
//! - The **model frame** is fixed to the part. Its X axis is the rotary axis
//!   and its origin lies on that axis. The neutral toolpath IR is written in
//!   this frame (decision record 0001, item 1).
//! - The **machine frame** is fixed to the machine. The tool axis is machine +Z.
//!   At A = 0 the model frame is the machine frame translated by
//!   [`MachineProfile::rotary_origin`]. A rotation of A degrees turns the part
//!   by `sign * A` about the rotary axis, where `sign` comes from
//!   [`MachineProfile::a_sense`].
//!
//! So for rotation about X, with `R(t)` the right-handed rotation by `t` about X:
//!
//! ```text
//! machine_xyz = rotary_origin + R(sign * A) * tip_model
//! tip_model   = R(-sign * A) * (machine_xyz - rotary_origin)
//! ```

use crate::geometry::{Point3, UnitVec3, Vec3};
use crate::machine::{MachineProfile, RotaryAxis};
use crate::units::Degrees;

/// A machine position: linear axes in machine coordinates and the unwrapped A angle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MachinePosition {
    /// Machine X, Y and Z in millimetres.
    pub xyz: Point3,
    /// A in degrees, unwrapped.
    pub a: Degrees,
}

/// Rotates `v` by `angle` about the profile's rotary axis (right-handed).
fn rotate(axis: RotaryAxis, v: Vec3, angle: Degrees) -> Vec3 {
    let (s, c) = angle.to_radians().sin_cos();
    match axis {
        RotaryAxis::X => Vec3::new(v.x, c * v.y - s * v.z, s * v.y + c * v.z),
    }
}

/// Forward kinematics: the tool tip in the model frame for a machine position.
pub fn tool_tip_in_model(profile: &MachineProfile, position: &MachinePosition) -> Point3 {
    let relative = position.xyz - profile.rotary_origin;
    let angle = Degrees(-profile.a_sense.sign() * position.a.0);
    Point3::ORIGIN + rotate(profile.rotary_axis, relative, angle)
}

/// Inverse kinematics for a known A: the machine XYZ that puts the tool tip at
/// `tip` (model frame) when the rotary axis is at `a`.
pub fn machine_xyz(profile: &MachineProfile, tip: Point3, a: Degrees) -> Point3 {
    let angle = Degrees(profile.a_sense.sign() * a.0);
    profile.rotary_origin + rotate(profile.rotary_axis, tip.to_vec(), angle)
}

/// The tool axis (machine +Z) expressed in the model frame at rotary angle `a`.
pub fn tool_axis_in_model(profile: &MachineProfile, a: Degrees) -> UnitVec3 {
    let angle = Degrees(-profile.a_sense.sign() * a.0);
    let v = rotate(profile.rotary_axis, UnitVec3::Z.get(), angle);
    // A rotation of a unit vector has length one up to rounding; fall back to
    // +Z only if `a` was not finite.
    UnitVec3::new(v).unwrap_or(UnitVec3::Z)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::machine::ASense;

    #[test]
    fn req_nfr_05_kinematics_round_trip_in_double_precision() {
        let mut profile = MachineProfile::example_x_rotary();
        for sense in [ASense::Positive, ASense::Negative] {
            profile.a_sense = sense;
            for a in [-725.0, -90.0, -0.5, 0.0, 33.3, 359.999, 360.0, 1080.25] {
                let tip = Point3::new(12.0, -7.5, 18.25);
                let xyz = machine_xyz(&profile, tip, Degrees(a));
                let back = tool_tip_in_model(&profile, &MachinePosition { xyz, a: Degrees(a) });
                assert!(
                    back.distance(tip) < 1e-9,
                    "a = {a}, error {}",
                    back.distance(tip)
                );
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
}
