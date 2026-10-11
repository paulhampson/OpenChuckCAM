// SPDX-License-Identifier: AGPL-3.0-or-later
//! The independent kinematic oracle for rotation about X (architecture section 9).
//!
//! The path library, the post and the G-code reader all share
//! `openchuck_types::kinematics`, so a replay through them could agree with a
//! wrong machine model. This module maps a machine position back to the tool
//! tip in model space from first principles, in polar form, and deliberately
//! shares no code with the shared model: it reads only plain numbers.
//!
//! Model: the part turns about a line parallel to machine X through the point
//! (`origin_y`, `origin_z`). A point of the part at polar angle `phi` (measured
//! from +Y towards +Z) and radius `r` about that line appears in the machine
//! at polar angle `phi + sense * A`. Machine X equals model X plus `origin_x`.
//! So, given the machine tip position, the model tip has radius `r` and polar
//! angle `theta - sense * A`, where `theta` is the tip's machine polar angle.
//!
//! Decision record 0001 confirmed this design in spike SP-3.

/// The oracle's own description of an X-rotary machine.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OracleMachine {
    /// Machine X of the model origin.
    pub origin_x: f64,
    /// Machine Y of the rotary axis.
    pub origin_y: f64,
    /// Machine Z of the rotary axis.
    pub origin_z: f64,
    /// +1.0 if a positive A turns the part counter-clockwise about +X, -1.0 if clockwise.
    pub sense: f64,
}

/// The tool tip in model coordinates `[x, y, z]` for machine position `(x, y, z, a_degrees)`.
pub fn model_tip(machine: &OracleMachine, x: f64, y: f64, z: f64, a_degrees: f64) -> [f64; 3] {
    let dy = y - machine.origin_y;
    let dz = z - machine.origin_z;
    let radius = dy.hypot(dz);
    let theta = dz.atan2(dy);
    let phi = theta - machine.sense * a_degrees * std::f64::consts::PI / 180.0;
    [x - machine.origin_x, radius * phi.cos(), radius * phi.sin()]
}

/// Distance in millimetres between two points given as arrays.
pub fn distance(p: [f64; 3], q: [f64; 3]) -> f64 {
    let [px, py, pz] = p;
    let [qx, qy, qz] = q;
    ((px - qx).powi(2) + (py - qy).powi(2) + (pz - qz).powi(2)).sqrt()
}
