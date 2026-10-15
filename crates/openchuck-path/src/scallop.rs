// SPDX-License-Identifier: AGPL-3.0-or-later
//! Stepover from a scallop (cusp) height (OP-04, OP-11; decision record 0034
//! item 1).
//!
//! The closed forms need a tool whose contact is on one arc of radius `rho`: a
//! ball end, or the corner of a bull-nose. Any other profile is refused,
//! because a cusp height has no closed form for it.

use openchuck_types::tool::profile::{ProfilePiece, RevolvedProfile};

/// The radius of the arc that cuts, for a ball end or a bull-nose; `None`
/// for any other profile, which has no closed form for a cusp height.
pub(crate) fn cusp_radius(profile: &RevolvedProfile) -> Option<f64> {
    let mut radius: Option<f64> = None;
    for piece in profile.pieces() {
        match *piece {
            ProfilePiece::Arc {
                radius: r, convex, ..
            } => {
                if !convex || radius.is_some_and(|known| (known - r).abs() > 1e-6) {
                    return None;
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
                    return None;
                }
            }
        }
    }
    radius.filter(|r| *r > 0.0)
}

/// What to tell the user when `height` is set and `profile` cannot use it:
/// the scallop is ignored and the stepover alone sets the passes. `None`
/// when there is nothing to say.
pub(crate) fn warning(profile: &RevolvedProfile, height: f64) -> Option<String> {
    (height > 0.0 && cusp_radius(profile).is_none()).then(|| {
        format!(
            "the scallop height of {height} mm is ignored: a cusp height can only be worked out \
             for ball-end and bull-nose tools, so the stepover alone sets the passes"
        )
    })
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
