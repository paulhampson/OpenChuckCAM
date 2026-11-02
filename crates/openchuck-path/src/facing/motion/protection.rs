// SPDX-License-Identifier: AGPL-3.0-or-later
//! Conservative continuous full-body separation, independent of pass planning.
//! This proves part/protection clearance, not stock engagement or completion.

use openchuck_types::geometry::Point3;
use openchuck_types::ir::{ToolPose, ToolpathGroup};
use openchuck_types::job::JobContext;
use openchuck_types::tool::ToolShape;

use super::{refuse, verify_machine_motion};
use crate::error::Result;
use crate::facing::extents::FacingExtent;
use crate::facing::geometry::FacingPlane;
use crate::facing::plan::FacingSetup;
use crate::rotary::RotaryMapping;
use crate::solid::Solid;

/// Verify the COMPLETE supported flat-tool body against the whole placed part
/// and every tab/held solid throughout every move, rapid or fed. Model removal
/// within `model_tolerance` is allowed by OP-12; protections never receive that
/// allowance. `margin` adds the caller's nonnegative motion/envelope error.
/// Unsupported separation refuses. This does not certify cutting engagement,
/// retained stock, achieved coverage, or any post's written precision.
pub fn verify_protected_clearance(
    setup: &FacingSetup<'_>,
    group: &ToolpathGroup,
    model_tolerance: f64,
    margin: f64,
    context: &JobContext<'_>,
) -> Result<()> {
    verify_machine_motion(group, setup.machine, context)?;
    setup.tool.validate()?;
    setup.part.validate()?;
    if !matches!(setup.tool.shape, ToolShape::FlatEnd) {
        return Err(refuse("protection proof requires a supported flat tool"));
    }
    if !model_tolerance.is_finite() || model_tolerance < 0. || !margin.is_finite() || margin < 0. {
        return Err(refuse(
            "protection tolerance and margin must be finite and nonnegative",
        ));
    }
    let source_error = setup
        .projection_error
        .filter(|e| e.is_finite() && *e >= 0.)
        .ok_or_else(|| refuse("protection proof requires a whole-source projection bound"))?;
    let part_x = FacingExtent::Part.axial(setup.stock, setup.part, &[], 0.)?;
    let vertices = setup
        .part
        .triangles
        .iter()
        .flatten()
        .filter_map(|i| {
            usize::try_from(*i)
                .ok()
                .and_then(|i| setup.part.vertices.get(i))
        })
        .copied()
        .collect::<Vec<_>>();
    let part_radius = vertices.iter().map(|p| p.y.hypot(p.z)).fold(0., f64::max);
    let mut geometry_scale = vertices
        .iter()
        .flat_map(|p| [p.x.abs(), p.y.abs(), p.z.abs()])
        .fold(1., f64::max);
    for solid in setup.protected.iter().chain(setup.tabs) {
        context.check_cancelled()?;
        let bounds = solid.bounds();
        if !bounds.min.is_finite() || !bounds.max.is_finite() {
            return Err(refuse("protection geometry has no finite bound"));
        }
        geometry_scale = [bounds.min, bounds.max]
            .into_iter()
            .flat_map(|p| [p.x.abs(), p.y.abs(), p.z.abs()])
            .fold(geometry_scale, f64::max);
    }
    let radius = 0.5 * setup.tool.diameter.0.max(setup.tool.shank_diameter.0);
    let length = setup.tool.overall_length.0;
    let rotary = RotaryMapping::of_profile(setup.machine);
    let mut previous = group.start;
    // A sequence with no segments still has a physical starting pose.
    let ends = std::iter::once(group.start).chain(group.segments.iter().map(|s| s.end));
    let mut budget = 20_000_000usize;
    let mut projection: Option<(openchuck_types::units::Degrees, f64, (f64, f64))> = None;
    for (index, end) in ends.enumerate() {
        context.check_cancelled()?;
        let work = setup.protected.len().saturating_add(setup.tabs.len());
        budget = budget
            .checked_sub(work)
            .ok_or_else(|| refuse("protection proof geometry budget exceeded"))?;
        let scale = [previous.tip, end.tip]
            .into_iter()
            .flat_map(|p| [p.x.abs(), p.y.abs(), p.z.abs()])
            .fold(radius.max(length).max(geometry_scale), f64::max);
        let phase_scale = previous.a.0.abs().max(end.a.0.abs()).to_radians().max(1.);
        let roundoff = 128. * f64::EPSILON * scale * phase_scale;
        if !roundoff.is_finite() {
            return Err(refuse("protection geometry has no finite arithmetic bound"));
        }
        let body = Body::new(previous, end, radius, length, rotary);
        if ![
            body.x.0,
            body.x.1,
            body.across.0,
            body.across.1,
            body.height.0,
            body.height.1,
        ]
        .into_iter()
        .all(f64::is_finite)
        {
            return Err(refuse("complete-tool envelope has no finite bound"));
        }
        let part_gap = source_error + margin + roundoff - model_tolerance;
        let mut part_clear =
            body.separated_x(part_x, part_gap) || body.height.0 >= part_radius + part_gap;
        if !part_clear && previous.a == end.a {
            // Axial/global separation needs no facet scan. At fixed A the
            // immutable whole-source projection can be shared by all moves
            // of that view, rather than spent again for every raster segment.
            if projection.as_ref().is_none_or(|p| p.0 != end.a) {
                budget = budget
                    .checked_sub(vertices.len().saturating_mul(2))
                    .ok_or_else(|| refuse("protection proof geometry budget exceeded"))?;
                let plane = FacingPlane::side(rotary, end.a)?;
                let support = plane.support(vertices.iter().copied(), &[])?;
                let bound = plane.bounds(vertices.iter().copied(), &[], 0.)?;
                projection = Some((end.a, support, (bound.min()[1], bound.max()[1])));
            }
            let (_, support, (low, high)) =
                projection.ok_or_else(|| refuse("whole-source projection is unavailable"))?;
            part_clear = body.height.0 >= support + part_gap;
            if !part_clear {
                part_clear = separated(body.across, (low, high), part_gap);
            }
        }
        if !part_clear {
            return Err(refuse(&format!(
                "move {index} has no continuous complete-tool separation from the whole part"
            )));
        }
        for solid in setup.protected.iter().chain(setup.tabs) {
            context.check_cancelled()?;
            if !body.clear_solid(*solid, previous.a == end.a, rotary, end, margin + roundoff)? {
                return Err(refuse(&format!(
                    "move {index} has no continuous complete-tool separation from protected geometry"
                )));
            }
        }
        previous = end;
    }
    Ok(())
}

struct Body {
    x: (f64, f64),
    across: (f64, f64),
    height: (f64, f64),
}
impl Body {
    fn new(from: ToolPose, to: ToolPose, radius: f64, length: f64, rotary: RotaryMapping) -> Self {
        let a = rotary.to_axis(from.tip, from.a);
        let b = rotary.to_axis(to.tip, to.a);
        // In controller-linear XYZ/A, model X and machine-relative height
        // are affine, even during A motion. Across bounds refer to machine
        // coordinates during rotary motion and are used ONLY at fixed A.
        Self {
            x: (
                from.tip.x.min(to.tip.x) - radius,
                from.tip.x.max(to.tip.x) + radius,
            ),
            across: (
                a.across.min(b.across) - radius,
                a.across.max(b.across) + radius,
            ),
            height: (a.height.min(b.height), a.height.max(b.height) + length),
        }
    }
    fn separated_x(&self, x: (f64, f64), gap: f64) -> bool {
        separated(self.x, x, gap)
    }
    #[allow(clippy::float_cmp)] // Exactly axial is required by the disc proof.
    fn clear_solid(
        &self,
        solid: Solid,
        fixed: bool,
        rotary: RotaryMapping,
        end: ToolPose,
        gap: f64,
    ) -> Result<bool> {
        let bounds = solid.bounds();
        if self.separated_x((bounds.min.x, bounds.max.x), gap) {
            return Ok(true);
        }
        let radial = bounds
            .min
            .y
            .abs()
            .max(bounds.max.y.abs())
            .hypot(bounds.min.z.abs().max(bounds.max.z.abs()));
        if self.height.0 >= radial + gap {
            return Ok(true);
        }
        if !fixed {
            return Ok(false);
        }
        // For an exactly axial cylinder/frustum, bound the radius over the
        // actual overlapping axial slab, not over the tab's unrelated end.
        // Its disc and the full swept-body UH rectangle must be disjoint.
        let axis = solid.axis().get();
        if solid.as_cuboid().is_none() && axis.x.abs() == 1. && axis.y == 0. && axis.z == 0. {
            let s0 = ((self.x.0 - gap - solid.base().x) * axis.x)
                .min((self.x.1 + gap - solid.base().x) * axis.x)
                .max(0.);
            let s1 = ((self.x.0 - gap - solid.base().x) * axis.x)
                .max((self.x.1 + gap - solid.base().x) * axis.x)
                .min(solid.length());
            if s0 > s1 {
                return Ok(true);
            }
            let radius_at = |s| {
                solid.base_radius()
                    + (solid.top_radius() - solid.base_radius()) * (s / solid.length())
            };
            let r = radius_at(s0).max(radius_at(s1));
            let c = rotary.to_axis(solid.base(), end.a);
            let distance = distance_to_interval(c.across, self.across)
                .hypot(distance_to_interval(c.height, self.height));
            return Ok(distance >= r + gap);
        }
        // Convex projected bounds provide a conservative separating plane for
        // boxes and other tab orientations. Failure is refusal, not collision
        // sampling or an inference from clear endpoints.
        let plane = FacingPlane::side(rotary, end.a)?;
        let rect = plane.bounds(std::iter::empty::<Point3>(), &[solid], 0.)?;
        let support = plane.support(std::iter::empty::<Point3>(), &[solid])?;
        Ok(separated(self.across, (rect.min()[1], rect.max()[1]), gap)
            || self.height.0 >= support + gap)
    }
}
fn separated(a: (f64, f64), b: (f64, f64), gap: f64) -> bool {
    a.1 <= b.0 - gap || a.0 >= b.1 + gap
}
fn distance_to_interval(x: f64, range: (f64, f64)) -> f64 {
    (range.0 - x).max(x - range.1).max(0.)
}
