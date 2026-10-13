//! The "shared machine model": what openchuck-types would own (architecture section 3).
//! Rotation about X only. Written with 2x2 rotation matrices on the (Y, Z) plane.
//!
//! Convention under test (SP-12 will confirm for the Z1):
//!   machine_yz = axis_yz + R(sigma * A) * model_yz
//! where R(alpha) is a right-handed rotation about +X, and sigma = `a_sign`.
//! sigma = +1: positive A turns the workpiece counter-clockwise looking from +X to the origin.

#[derive(Clone, Copy, Debug)]
pub struct Machine {
    pub a_sign: f64,
    /// Machine (Y, Z) of the rotary axis, in the work offset.
    pub axis_yz: [f64; 2],
}

impl Machine {
    pub const REFERENCE: Machine = Machine {
        a_sign: 1.0,
        axis_yz: [2.0, -30.0],
    };

    fn rot(alpha_deg: f64, v: [f64; 2]) -> [f64; 2] {
        let (s, c) = alpha_deg.to_radians().sin_cos();
        [c * v[0] - s * v[1], s * v[0] + c * v[1]]
    }

    /// Model-frame tip to machine XYZ for a given A (degrees).
    pub fn to_machine(&self, tip_model: [f64; 3], a_deg: f64) -> [f64; 3] {
        let r = Self::rot(self.a_sign * a_deg, [tip_model[1], tip_model[2]]);
        [tip_model[0], self.axis_yz[0] + r[0], self.axis_yz[1] + r[1]]
    }

    /// Machine XYZA back to the model-frame tip (the forward kinematics the replay uses).
    pub fn to_model(&self, xyz: [f64; 3], a_deg: f64) -> [f64; 3] {
        let v = [xyz[1] - self.axis_yz[0], xyz[2] - self.axis_yz[1]];
        let r = Self::rot(-self.a_sign * a_deg, v);
        [xyz[0], r[0], r[1]]
    }

    /// Direction vectors (tool axis) rotate with A but are not translated.
    pub fn axis_to_machine(&self, axis_model: [f64; 3], a_deg: f64) -> [f64; 3] {
        let r = Self::rot(self.a_sign * a_deg, [axis_model[1], axis_model[2]]);
        [axis_model[0], r[0], r[1]]
    }
}
