//! Independent kinematic oracle for rotation about X (verification plan, finding 16).
//!
//! Deliberately shares NO code with `machine.rs`: it works in polar form (radius and angle
//! about the rotary axis) using atan2, rather than rotation matrices, and it states the
//! convention from first principles instead of calling the shared model.
//!
//! First principles: with sigma = +1, increasing A turns the workpiece counter-clockwise when
//! viewed from +X looking at the origin (right-handed about +X). A material point at model
//! polar angle psi (measured from +Y towards +Z about the axis) therefore sits at machine polar
//! angle psi + sigma*A about the rotary axis, at the same radius and the same X.

pub struct Oracle {
    pub sigma: f64,
    pub axis_y: f64,
    pub axis_z: f64,
}

impl Oracle {
    /// Where is the tool tip in the workpiece (model) frame when the machine reads X Y Z A?
    pub fn tool_tip_in_model(&self, x: f64, y: f64, z: f64, a_deg: f64) -> [f64; 3] {
        let dy = y - self.axis_y;
        let dz = z - self.axis_z;
        let rho = dy.hypot(dz);
        let machine_angle = dz.atan2(dy);
        let model_angle = machine_angle - self.sigma * a_deg.to_radians();
        [x, rho * model_angle.cos(), rho * model_angle.sin()]
    }

    /// Closed form for the one case SP-3 uses: the contact point on a cylinder of radius `r`
    /// (about +X) at model angle `theta_deg` and axial position `x` is brought under the tool
    /// (machine +Z above the axis) when sigma*A = 90 - theta (mod 360).
    /// Returns the expected machine (X, Y, Z) for the tool tip and A modulo 360.
    pub fn expected_top_contact(&self, x: f64, theta_deg: f64, r: f64) -> ([f64; 3], f64) {
        let a = (self.sigma * (90.0 - theta_deg)).rem_euclid(360.0);
        ([x, self.axis_y, self.axis_z + r], a)
    }
}

/// Angular difference modulo 360, in [-180, 180).
pub fn ang_diff(a: f64, b: f64) -> f64 {
    (a - b + 180.0).rem_euclid(360.0) - 180.0
}
