// SPDX-License-Identifier: AGPL-3.0-or-later
//! Independent nominal machine travel/feed checks for Facing (OP-24).
//! These do not certify tool/stock/protection clearance or written precision.

use crate::error::{PathError, Result};
use crate::rotary::RotaryMapping;

mod protection;
pub use protection::verify_protected_clearance;
mod stock;
pub use stock::verify_shank_clearance;

/// Conservative all-angle radial bound of stock, placed part and protections.
/// A coaxial cylinder uses its analytic radius, rather than the larger radius
/// of its enclosing square. Other geometry retains conservative projected boxes.
pub fn enclosing_radius(
    setup: &super::plan::FacingSetup<'_>,
    context: &JobContext<'_>,
) -> Result<f64> {
    use super::extents::FacingExtent;
    use super::geometry::{FacingEnd, FacingPlane};
    let error = setup
        .projection_error
        .filter(|e| e.is_finite() && *e >= 0.)
        .ok_or_else(|| refuse("radial clearance requires a whole-source projection bound"))?;
    setup.stock.validate()?;
    let radius = |lo: [f64; 2], hi: [f64; 2]| {
        lo[0]
            .abs()
            .max(hi[0].abs())
            .hypot(lo[1].abs().max(hi[1].abs()))
    };
    let plane = FacingPlane::end(FacingEnd::Far);
    let stock = match setup.stock {
        openchuck_types::stock::Stock::Cylinder { diameter, .. } => diameter.0 * 0.5,
        _ => {
            let b = FacingExtent::WholeStock.planar(plane, setup.stock, setup.part, &[], 0.)?;
            radius(b.min(), b.max())
        }
    };
    let part = FacingExtent::Part.planar(plane, setup.stock, setup.part, &[], error)?;
    let mut result = stock.max(radius(part.min(), part.max()));
    for solid in setup.protected.iter().chain(setup.tabs) {
        context.check_cancelled()?;
        let b = solid.bounds();
        result = result.max(radius([b.min.y, b.min.z], [b.max.y, b.max.z]));
    }
    if !result.is_finite() {
        return Err(refuse("setup has no finite radial bound"));
    }
    Ok(result)
}

/// Prove each rapid's COMPLETE supported flat-tool body outside original stock,
/// the whole placed part and all protections for the entire controller-linear
/// XYZ/A leg. Conservative axial or global-height separation is required;
/// unsupported separation refuses. This does not certify fed moves or completion.
/// `margin` is an explicit additional position/envelope error budget in mm.
pub fn verify_rapid_clearance(
    setup: &super::plan::FacingSetup<'_>,
    group: &ToolpathGroup,
    margin: f64,
    context: &JobContext<'_>,
) -> Result<()> {
    use super::extents::FacingExtent;
    use super::geometry::{FacingEnd, FacingPlane};
    use openchuck_types::tool::ToolShape;
    verify_machine_motion(group, setup.machine, context)?;
    setup.tool.validate()?;
    if !matches!(setup.tool.shape, ToolShape::FlatEnd) {
        return Err(refuse(
            "rapid envelope has no supported complete-tool bound",
        ));
    }
    if !margin.is_finite() || margin < 0. {
        return Err(refuse(
            "rapid clearance margin must be finite and nonnegative",
        ));
    }
    let error = setup
        .projection_error
        .filter(|error| error.is_finite() && *error >= 0.)
        .ok_or_else(|| refuse("rapid clearance requires a whole-source projection bound"))?;
    let (mut lo, mut hi) = FacingExtent::WholeStock.axial(setup.stock, setup.part, &[], 0.)?;
    let part = FacingExtent::Part.axial(setup.stock, setup.part, &[], 0.)?;
    lo = lo.min(part.0 - error);
    hi = hi.max(part.1 + error);
    let plane = FacingPlane::end(FacingEnd::Far);
    let stock = FacingExtent::WholeStock.planar(plane, setup.stock, setup.part, &[], 0.)?;
    let whole_radius = enclosing_radius(setup, context)?;
    for protected in setup.protected.iter().chain(setup.tabs) {
        context.check_cancelled()?;
        let bounds = protected.bounds();
        lo = lo.min(bounds.min.x);
        hi = hi.max(bounds.max.x);
    }
    if !lo.is_finite() || !hi.is_finite() || !whole_radius.is_finite() {
        return Err(refuse("rapid geometry has no finite whole-setup bound"));
    }
    let radius = 0.5 * setup.tool.diameter.0.max(setup.tool.shank_diameter.0);
    let rotary = RotaryMapping::of_profile(setup.machine);
    let mut previous = group.start;
    let mut fixed_support: Option<(openchuck_types::units::Degrees, f64, f64)> = None;
    let mut budget = 20_000_000usize;
    for (index, segment) in group.segments.iter().enumerate() {
        context.check_cancelled()?;
        if !segment.kind.is_fed() {
            // Model X is affine throughout a controller-linear move even as
            // A changes. A vertical complete tool has constant radial X reach.
            let low = previous.tip.x.min(segment.end.tip.x) - radius;
            let high = previous.tip.x.max(segment.end.tip.x) + radius;
            let height = rotary
                .to_axis(previous.tip, previous.a)
                .height
                .min(rotary.to_axis(segment.end.tip, segment.end.a).height);
            // Fixed-machine Z is affine too. Every point of this supported
            // tool is at/above its tip, while the entire rotating setup stays
            // within whole_radius. No endpoint-only collision inference.
            let mut clear =
                low > hi + margin || high < lo - margin || height > whole_radius + margin;
            if !clear && previous.a == segment.end.a {
                // At fixed A, machine-relative height is affine and every
                // point of this tool is at/above its tip. The exact projected
                // stock top is tighter than the all-angle rotation cylinder;
                // it remains a continuous bound, not an endpoint sample.
                let a = segment.end.a;
                if fixed_support.is_none_or(|cached| cached.0 != a) {
                    let work = setup
                        .part
                        .triangles
                        .len()
                        .saturating_mul(3)
                        .saturating_add(setup.protected.len())
                        .saturating_add(setup.tabs.len())
                        .saturating_add(match setup.stock {
                            openchuck_types::stock::Stock::Imported { mesh } => {
                                mesh.triangles.len().saturating_mul(3)
                            }
                            openchuck_types::stock::Stock::OffsetBoundingBox { .. } => {
                                setup.part.triangles.len().saturating_mul(3)
                            }
                            _ => 8,
                        });
                    budget = budget
                        .checked_sub(work)
                        .ok_or_else(|| refuse("rapid projection work budget exceeded"))?;
                    let plane = FacingPlane::side(rotary, a)?;
                    let stock_support =
                        FacingExtent::WholeStock.support(plane, setup.stock, setup.part, &[])?;
                    let mut support =
                        FacingExtent::Part.support(plane, setup.stock, setup.part, &[])? + error;
                    for solid in setup.protected.iter().chain(setup.tabs) {
                        support = support
                            .max(plane.support(std::iter::empty(), std::slice::from_ref(solid))?);
                    }
                    fixed_support = Some((a, support.max(stock_support), support));
                }
                let support = fixed_support
                    .ok_or_else(|| refuse("rapid support is unavailable"))?
                    .1;
                let scale = whole_radius
                    .max(lo.abs())
                    .max(hi.abs())
                    .max(radius)
                    .max(height.abs())
                    .max(support.abs())
                    .max(1.);
                let roundoff = 128. * f64::EPSILON * scale * a.0.abs().to_radians().max(1.);
                clear = height > support + margin + roundoff;
                if !clear
                    && height
                        > fixed_support
                            .ok_or_else(|| refuse("rapid support is unavailable"))?
                            .2
                            + margin
                            + roundoff
                {
                    // A local stock corner can be lower than global support.
                    // Separate the entire swept tool bounding rectangle from
                    // the original WORLD YZ stock box. These are two additional
                    // continuous separating axes;
                    // no sampled-stock/clear-endpoint inference is involved.
                    let plane = FacingPlane::side(rotary, a)?;
                    let from = rotary.to_axis(previous.tip, a);
                    let to = rotary.to_axis(segment.end.tip, a);
                    let across = (
                        from.across.min(to.across) - radius - margin - roundoff,
                        from.across.max(to.across) + radius + margin + roundoff,
                    );
                    let heights = (
                        height - margin - roundoff,
                        from.height.max(to.height)
                            + setup.tool.overall_length.0
                            + margin
                            + roundoff,
                    );
                    let corners = [
                        plane.point(0., across.0, heights.0)?,
                        plane.point(0., across.0, heights.1)?,
                        plane.point(0., across.1, heights.0)?,
                        plane.point(0., across.1, heights.1)?,
                    ];
                    let ylo = corners.iter().map(|p| p.y).fold(f64::INFINITY, f64::min);
                    let yhi = corners
                        .iter()
                        .map(|p| p.y)
                        .fold(f64::NEG_INFINITY, f64::max);
                    let zlo = corners.iter().map(|p| p.z).fold(f64::INFINITY, f64::min);
                    let zhi = corners
                        .iter()
                        .map(|p| p.z)
                        .fold(f64::NEG_INFINITY, f64::max);
                    clear = ylo > stock.max()[0]
                        || yhi < stock.min()[0]
                        || zlo > stock.max()[1]
                        || zhi < stock.min()[1];
                }
            }
            if !clear {
                return Err(refuse(&format!(
                    "rapid move {} has no continuous complete-tool separation from stock, the whole part and protected geometry",
                    index + 1
                )));
            }
        }
        previous = segment.end;
    }
    Ok(())
}
use openchuck_types::ir::{ToolPose, ToolpathGroup};
use openchuck_types::job::JobContext;
use openchuck_types::kinematics::{RotaryFrame, machine_xyz};
use openchuck_types::machine::{ARange, MachineProfile, RotaryInFeed};

/// Check every move, including group start, entries/links/rapids and rotary
/// motion. Controller-linear XYZ and A attain axis extrema at endpoints;
/// tool-envelope clearance requires a separate continuous-motion proof.
pub fn verify_machine_motion(
    group: &ToolpathGroup,
    machine: &MachineProfile,
    context: &JobContext<'_>,
) -> Result<()> {
    context.check_cancelled()?;
    machine.validate()?;
    if group.segments.len() > 1_000_000 {
        return Err(refuse("motion verification budget exceeded"));
    }
    let frame = RotaryFrame::of_profile(machine);
    let rotary = RotaryMapping::of_profile(machine);
    let check_pose = |pose: ToolPose| -> Result<()> {
        if !pose.tip.is_finite() || !pose.a.0.is_finite() {
            return Err(refuse("a motion pose is not finite"));
        }
        if let ARange::Limited { min, max } = machine.a_range
            && (pose.a.0 < min.0 || pose.a.0 > max.0)
        {
            return Err(refuse("a motion pose exceeds A travel"));
        }
        if (pose.axis.get() - rotary.tool_axis(pose.a).get()).length() > 1e-9 {
            return Err(refuse(
                "the tool axis does not match the fixed spindle at this A",
            ));
        }
        let xyz = machine_xyz(&frame, pose.tip, pose.a);
        if !xyz.is_finite() || !machine.travel.contains(xyz) {
            return Err(refuse("a motion pose exceeds machine linear travel"));
        }
        Ok(())
    };
    check_pose(group.start)?;
    let mut previous = group.start;
    for (index, segment) in group.segments.iter().enumerate() {
        context.check_cancelled()?;
        segment.validate("facing motion")?;
        check_pose(segment.end)?;
        let from = machine_xyz(&frame, previous.tip, previous.a);
        let to = machine_xyz(&frame, segment.end.tip, segment.end.a);
        let delta = to - from;
        let (dx, dy, dz) = (delta.x.abs(), delta.y.abs(), delta.z.abs());
        let da = (segment.end.a.0 - previous.a.0).abs();
        let limits = machine.max_feeds;
        let (linear, z, angular) = if segment.kind.is_fed() {
            (limits.linear.0, limits.z.0, limits.rotary.0)
        } else {
            (
                limits.rapid_linear.0,
                limits.rapid_z.0,
                limits.rapid_rotary.0,
            )
        };
        let mut required = 60. * (dx / linear).max(dy / linear).max(dz / z).max(da / angular);
        let line = dx.hypot(dy).hypot(dz);
        if line > 1e-9 {
            let counted = if segment.kind.is_fed() {
                match limits.rotary_in_feed {
                    RotaryInFeed::Separate => line,
                    RotaryInFeed::Makera => line.max(
                        da * (std::f64::consts::TAU
                            * segment.end.tip.y.hypot(segment.end.tip.z).max(1.)
                            + 30.)
                            / 360.,
                    ),
                    _ => return Err(refuse("this rotary feed rule has no verification bound")),
                }
            } else {
                line
            };
            let line_limit = if dx > 1e-9 || dy > 1e-9 { linear } else { z };
            required = required.max(60. * counted / line_limit);
        }
        if !required.is_finite() || segment.duration.0 + 1e-9 + 1e-10 * required < required {
            return Err(refuse(&format!(
                "move {} is timed at {:.6} s but needs at least {required:.6} s for the machine's axis and coupled feed limits",
                index + 1,
                segment.duration.0
            )));
        }
        previous = segment.end;
    }
    Ok(())
}

fn refuse(reason: &str) -> PathError {
    PathError::Strategy(format!("Facing motion: {reason}"))
}
