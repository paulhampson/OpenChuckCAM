// SPDX-License-Identifier: AGPL-3.0-or-later
//! Conservative flat-tip raster coverage from actual fixed-A sweeps (OP-24).
//! This checks planar completion only. A host must independently establish
//! full-body clearance, permitted removal, machine limits and written-motion
//! error bounds. It must supply the flat cutting radius, not a ball/shank radius.

use openchuck_types::ir::ToolPose;
use openchuck_types::job::JobContext;

use super::geometry::{CoverageRectangle, FacingPlane};
use crate::error::{PathError, Result};

mod end;
pub use end::verify_end_coverage;

/// One actual machine-linear, fixed-A fed sweep, in model coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlatSweep {
    /// Starting pose after any clipping, raising or assembly transformation.
    pub from: ToolPose,
    /// Ending pose; physical A must stay fixed throughout this sweep.
    pub to: ToolPose,
}

/// Prove a rectangle covered by flat cutter sweeps spanning its u range.
/// Incomplete/unsupported paths refuse; success is not a clearance certificate.
/// Raised roughing layers do not count towards final coverage. No lateral gap
/// is hidden by the normal finishing tolerance.
#[allow(clippy::too_many_arguments)]
pub fn verify_flat_raster(
    rectangle: CoverageRectangle,
    plane: FacingPlane,
    target: f64,
    sweeps: &[FlatSweep],
    radius: f64,
    tolerance: f64,
    context: &JobContext<'_>,
) -> Result<()> {
    context.check_cancelled()?;
    if !target.is_finite()
        || !radius.is_finite()
        || radius <= 0.0
        || !tolerance.is_finite()
        || tolerance <= 0.0
    {
        return Err(refuse("invalid target, flat cutting radius or tolerance"));
    }
    if sweeps.len() > 1_000_000 {
        return Err(refuse("coverage work limit exceeded"));
    }
    let [u0, v0] = rectangle.min();
    let [u1, v1] = rectangle.max();
    let mut strips = Vec::new();
    for s in sweeps {
        context.check_cancelled()?;
        if !s.from.a.0.is_finite() || !s.to.a.0.is_finite() || (s.from.a.0 - s.to.a.0).abs() > 0.0 {
            return Err(refuse("a flat raster sweep must keep physical A fixed"));
        }
        let normal = plane.normal();
        let axis_error = (s.from.axis.get() - normal)
            .length()
            .max((s.to.axis.get() - normal).length());
        if !axis_error.is_finite() || axis_error >= 1.0 {
            return Err(refuse(
                "the cutting disc has no supported facing projection",
            ));
        }
        let [x0, y0, z0] = plane.coordinates(s.from.tip)?;
        let [x1, y1, z1] = plane.coordinates(s.to.tip)?;
        let normal_error = radius * axis_error;
        if z0.min(z1) - normal_error < target - tolerance {
            return Err(refuse("a supplied sweep overcuts the requested plane"));
        }
        if z0.max(z1) + normal_error > target + tolerance {
            continue;
        }
        // Orthogonal projection of a disc contains a concentric disc of
        // radius r*cos(theta). For axis distance e<1, 1-e <= cos(theta),
        // so this smaller radius is conservative without guessing zero tilt.
        let projected = radius * (1.0 - axis_error);
        let end_gap = (x0.min(x1) - u0).max(u1 - x0.max(x1)).max(0.);
        if end_gap >= projected {
            continue;
        }
        // At every requested u, clamp to the centreline's u span. Its v is
        // between endpoint v values; even at an end cap the remaining disc
        // half-width is at least sqrt(projected^2 - end_gap^2). This proves
        // complete corners without equating a centreline to a cutter edge.
        let r = ((projected - end_gap) * (projected + end_gap)).sqrt();
        let lo = y0.max(y1) - r;
        let hi = y0.min(y1) + r;
        if lo <= hi {
            strips.push((lo, hi));
        }
    }
    strips.sort_by(|a, b| a.0.total_cmp(&b.0));
    context.check_cancelled()?;
    let mut reached = v0;
    for (lo, hi) in strips {
        context.check_cancelled()?;
        if hi < reached {
            continue;
        }
        if lo > reached {
            return Err(refuse(
                "an unfinished strip remains inside the requested extent",
            ));
        }
        reached = reached.max(hi);
        if reached >= v1 {
            return Ok(());
        }
    }
    Err(refuse(
        "the requested face is not completely covered by final-depth sweeps",
    ))
}

fn refuse(reason: &str) -> PathError {
    PathError::Strategy(format!("Facing completion: {reason}"))
}
