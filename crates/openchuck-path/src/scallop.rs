// SPDX-License-Identifier: AGPL-3.0-or-later
//! Stepover from a scallop (cusp) height (OP-04, OP-11; decision record 0033
//! item 1).
//!
//! The closed forms need a tool whose contact is on one arc of radius `rho`: a
//! ball end, or the corner of a bull-nose. Any other profile is refused,
//! because a cusp height has no closed form for it.

use openchuck_types::tool::profile::{ProfilePiece, RevolvedProfile};

use crate::error::{PathError, Result};

/// The radius of the arc that cuts, for a ball end or a bull-nose; fails for
/// any other profile.
pub(crate) fn cusp_radius(profile: &RevolvedProfile) -> Result<f64> {
    let refuse = || PathError::Parameter {
        name: "scallop".to_owned(),
        reason: "a scallop height is only defined for ball-end and bull-nose tools; \
                     give a stepover instead"
            .to_owned(),
    };
    let mut radius: Option<f64> = None;
    for piece in profile.pieces() {
        match *piece {
            ProfilePiece::Arc {
                radius: r, convex, ..
            } => {
                if !convex || radius.is_some_and(|known| (known - r).abs() > 1e-6) {
                    return Err(refuse());
                }
                radius = Some(r);
            }
            ProfilePiece::Line { start, end } => {
                // Only a flat bottom or a straight side can follow or precede
                // the arc.
                let (dr, dz) = (
                    (end.radius.0 - start.radius.0).abs(),
                    (end.height.0 - start.height.0).abs(),
                );
                if dr > 1e-9 && dz > 1e-9 {
                    return Err(refuse());
                }
            }
        }
    }
    radius.filter(|r| *r > 0.0).ok_or_else(refuse)
}

/// The angle, in radians, between neighbouring passes round a convex surface
/// of radius `surface` that leaves a cusp of height `height` with a tool arc
/// of radius `rho` (the tool centres lie on a circle of radius
/// `surface + rho`).
pub(crate) fn circumferential_step(rho: f64, surface: f64, height: f64) -> f64 {
    let h = height.min(rho);
    let (big, top) = (surface + rho, surface + h);
    let cos = ((big * big + top * top - rho * rho) / (2.0 * big * top)).clamp(-1.0, 1.0);
    2.0 * cos.acos()
}

/// The distance between neighbouring passes along a straight line of the
/// surface that leaves a cusp of height `height` with a tool arc of radius
/// `rho`.
pub(crate) fn straight_step(rho: f64, height: f64) -> f64 {
    let h = height.min(rho);
    2.0 * (2.0 * rho * h - h * h).max(0.0).sqrt()
}
