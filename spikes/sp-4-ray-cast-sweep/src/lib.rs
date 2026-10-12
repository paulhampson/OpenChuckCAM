// SPDX-License-Identifier: AGPL-3.0-or-later
//! SP-4 spike: continuous 4-axis with parry3d-f64 ray casts swept around the A axis.
//!
//! Throwaway code (work breakdown, Spikes). Frames: the part frame has the A axis along +X.
//! A tool approach direction at angle `psi` is `u(psi) = (0, sin psi, cos psi)`, pointing from
//! the part toward the tool; the machine has a fixed vertical tool, and with `P_machine = Rx(A) P_part` the tool
//! is vertical exactly when `A = psi`. The sign is the spike's own and is not the Z1 sign (OI-03).

pub mod exact;
pub mod mesh;
pub mod par;
pub mod part;
pub mod sweep;
pub mod tool;

pub use parry3d_f64::math::Vector;

/// Unit approach direction (tool to part is `-u`) for angle `psi`.
pub fn u_dir(psi: f64) -> Vector {
    Vector::new(0.0, psi.sin(), psi.cos())
}

/// Second lateral axis, perpendicular to X and to `u(psi)`.
pub fn e2_dir(psi: f64) -> Vector {
    Vector::new(0.0, psi.cos(), -psi.sin())
}

/// Machine Y and Z of a part-frame point at rotary angle `a` (rotation about +X).
pub fn to_machine(p: Vector, a: f64) -> Vector {
    let (s, c) = a.sin_cos();
    Vector::new(p.x, p.y * c - p.z * s, p.y * s + p.z * c)
}
