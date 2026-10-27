// SPDX-License-Identifier: AGPL-3.0-or-later
//! Independent nominal validation of assembled Facing operations. Connected
//! and posted paths still need their own validation before output is enabled.

mod end_layers;
mod legacy;
mod overlap;
pub(super) use legacy::verify as legacy;

use super::description::{FacingGeometry, describe};
use super::motion::{verify_protected_clearance, verify_rapid_clearance, verify_shank_clearance};
use super::plan::FacingSetup;
use crate::error::{PathError, Result};
use crate::params::{Parameters, names};
use crate::{RotaryMapping, StrategyInput};
use openchuck_types::ir::ToolpathGroup;
use openchuck_types::job::JobContext;

pub(super) fn end(
    setup: &FacingSetup<'_>,
    parameters: &Parameters,
    group: &ToolpathGroup,
    context: &JobContext<'_>,
) -> Result<()> {
    let description = describe(setup, parameters)?;
    let FacingGeometry::End {
        end,
        target,
        coverage,
    } = description.geometry
    else {
        return Err(refuse("expected End geometry"));
    };
    let tolerance = parameters.number(names::TOLERANCE)?;
    // Zero here means nominal IR, not a claim that posted/controller error is
    // zero. The application separately checks actual posted/controller motion.
    verify_rapid_clearance(setup, group, 0., context)?;
    verify_protected_clearance(setup, group, tolerance, 0., context)?;
    verify_shank_clearance(setup, group, 0., context)?;
    end_layers::verify(
        setup,
        &super::end::EndFacingParameters::read(parameters)?,
        end,
        target,
        coverage,
        group,
        context,
    )?;
    super::coverage::verify_end_coverage(
        setup, end, target, coverage, group, tolerance, 0., context,
    )?;
    context.check_cancelled()?;
    Ok(())
}
fn refuse(reason: &str) -> PathError {
    PathError::Strategy(format!("Facing final validation: {reason}"))
}

pub(super) fn input<'a>(
    input: &StrategyInput<'a>,
    context: &JobContext<'_>,
) -> Result<&'a super::plan::FacingSetup<'a>> {
    context.check_cancelled()?;
    let setup = input
        .facing
        .ok_or_else(|| refuse("whole placed setup geometry is required"))?;
    validate_setup(setup, context)?;
    if &setup.tool.profile()? != input.tool
        || RotaryMapping::of_profile(setup.machine) != input.rotary
    {
        return Err(refuse(
            "the actual tool profile or rotary frame differs from the whole setup used for verification",
        ));
    }
    Ok(setup)
}

pub(super) fn validate_setup(setup: &FacingSetup<'_>, context: &JobContext<'_>) -> Result<()> {
    context.check_cancelled()?;
    let source_error = setup
        .projection_error
        .filter(|e| e.is_finite() && *e >= 0.)
        .ok_or_else(|| refuse("the whole source has no established projection bound"))?;
    if source_error > 0. {
        return Err(refuse(
            "nominal Facing completion currently requires exact placed triangles; nonzero source projection uncertainty is not certified",
        ));
    }
    for (lo, hi) in setup.held.iter().flatten() {
        if !lo.is_finite() || !hi.is_finite() || hi <= lo {
            return Err(refuse("held intervals must be finite with positive length"));
        }
    }
    Ok(())
}

pub(super) fn box_sides(
    setup: &FacingSetup<'_>,
    parameters: &Parameters,
    group: &ToolpathGroup,
    context: &JobContext<'_>,
) -> Result<()> {
    use super::coverage::{FlatSweep, verify_flat_raster};
    use super::extents::FacingExtent;
    use super::plan::BoxFacingParameters;
    let settings = BoxFacingParameters::read(parameters)?;
    // Revalidate supported construction/tool/engagement settings, but derive
    // completion from the actual assembled sweeps below, never pass counts.
    if !matches!(
        setup.stock,
        openchuck_types::stock::Stock::Box { .. }
            | openchuck_types::stock::Stock::OffsetBoundingBox { .. }
    ) || !matches!(setup.tool.shape, openchuck_types::tool::ToolShape::FlatEnd)
        || setup.tool.shank_diameter.0 > setup.tool.diameter.0
        || settings.stepdown > setup.tool.flute_length.0
        || settings.stepover > setup.tool.diameter.0
    {
        return Err(refuse("unsupported Box stock/tool/engagement settings"));
    }
    let FacingGeometry::Box { faces } = describe(setup, parameters)?.geometry else {
        return Err(refuse("expected Box geometry"));
    };
    if faces.len().saturating_mul(group.segments.len()) > 20_000_000 {
        return Err(refuse("Box completion work budget exceeded"));
    }
    verify_box_removal_domain(setup, &faces, group, settings.tolerance, context)?;
    verify_rapid_clearance(setup, group, 0., context)?;
    verify_protected_clearance(setup, group, settings.tolerance, 0., context)?;
    verify_shank_clearance(setup, group, 0., context)?;
    let rotary = RotaryMapping::of_profile(setup.machine);
    let mut engagement_work = 20_000_000usize;
    for face in faces {
        context.check_cancelled()?;
        let outer =
            FacingExtent::WholeStock.support(face.target.plane, setup.stock, setup.part, &[])?;
        let levels = settings
            .layers
            .levels(outer, face.target.offset, settings.stepdown)?;
        if levels.is_empty() {
            return Err(refuse(
                "an already satisfied Box plane has no verified removal evidence",
            ));
        }
        let mut level = 0usize;
        let mut sweeps: Vec<FlatSweep> = Vec::new();
        let mut previous_ceiling = outer;
        let mut layer_ceiling = f64::NEG_INFINITY;
        let mut physical_view = None;
        let mut previous_footprints = overlap::Footprints::default();
        let mut current_footprints = overlap::Footprints::default();
        // Only arithmetic roundoff belongs in an engagement comparison.
        // The finishing tolerance must not enlarge a selected maximum depth.
        let depth_roundoff =
            512. * f64::EPSILON * outer.abs().max(face.target.offset.abs()).max(1.);
        if depth_roundoff > settings.tolerance * 0.125 || settings.stepdown <= 4. * depth_roundoff {
            return Err(refuse(
                "Box tolerance/stepdown is below the supported arithmetic resolution",
            ));
        }
        let coordinates_roundoff = 512.
            * f64::EPSILON
            * face
                .coverage
                .min()
                .into_iter()
                .chain(face.coverage.max())
                .map(f64::abs)
                .fold(outer.abs().max(1.), f64::max);
        let check = |level: usize, sweeps: &[FlatSweep]| -> Result<()> {
            let target = *levels
                .get(level)
                .ok_or_else(|| refuse("invalid Box layer index"))?;
            verify_flat_raster(
                face.coverage.transposed(),
                face.target.plane.transposed(),
                target,
                sweeps,
                0.5 * setup.tool.diameter.0,
                settings.tolerance,
                context,
            )
        };
        let mut from = group.start;
        for segment in &group.segments {
            context.check_cancelled()?;
            let to = segment.end;
            if segment.kind.is_fed()
                && from.a == to.a
                && (from.axis.get() - face.target.plane.normal()).length()
                    <= (settings.tolerance / (2. * setup.tool.diameter.0)).min(0.1)
            {
                if let Some(a) = physical_view {
                    if a != from.a {
                        return Err(refuse(
                            "selected Box views cannot be distinguished at this motion precision",
                        ));
                    }
                } else {
                    physical_view = Some(from.a);
                    previous_ceiling = FacingExtent::WholeStock.support(
                        super::geometry::FacingPlane::side(rotary, from.a)?,
                        setup.stock,
                        setup.part,
                        &[],
                    )?;
                }
                let actual0 = rotary.to_axis(from.tip, from.a);
                let actual1 = rotary.to_axis(to.tip, to.a);
                let h0 = face.target.plane.coordinates(from.tip)?[2];
                let h1 = face.target.plane.coordinates(to.tip)?[2];
                if h0.min(h1) < face.target.offset - settings.tolerance {
                    return Err(refuse("a Box move goes below its requested finished plane"));
                }
                // A deeper entry is permitted only AFTER actual sweeps cover
                // the preceding layer. Missing or reordered outer layers do
                // not borrow evidence from a later final-plane raster.
                while levels
                    .get(level)
                    .is_some_and(|h| h0.min(h1) < h - settings.tolerance)
                {
                    check(level, &sweeps)?;
                    previous_ceiling = layer_ceiling;
                    layer_ceiling = f64::NEG_INFINITY;
                    previous_footprints = std::mem::take(&mut current_footprints).ordered();
                    level += 1;
                    sweeps.clear();
                }
                if let Some(h) = levels.get(level)
                    && h0.max(h1) <= h + settings.tolerance
                {
                    if previous_ceiling - actual0.height.min(actual1.height)
                        > settings.stepdown + depth_roundoff
                    {
                        return Err(refuse(
                            "an actual Box cut exceeds the selected maximum stepdown",
                        ));
                    }
                    let footprint = overlap::Footprint::of(from, to, rotary, coordinates_roundoff)?;
                    if level > 0
                        && !previous_footprints.covers(
                            footprint,
                            coordinates_roundoff,
                            &mut engagement_work,
                        )?
                    {
                        return Err(refuse(
                            "deeper Box cutter overlap has no actual prior full-footprint removal; selected extent coverage alone does not establish engagement",
                        ));
                    }
                    current_footprints.insert(footprint);
                    layer_ceiling = layer_ceiling.max(actual0.height.max(actual1.height));
                    sweeps.push(FlatSweep { from, to });
                }
            }
            from = to;
        }
        check(level, &sweeps)?;
        if level + 1 != levels.len() {
            return Err(refuse("Box layers do not reach the final target"));
        }
    }
    context.check_cancelled()?;
    Ok(())
}

// Coverage grants only the cutter overlap of centres inside its rectangle.
// A fed entry/link outside that domain must independently prove complete-tool
// air separation; its high endpoint must not hide unrequested stock removal.
fn verify_box_removal_domain(
    setup: &FacingSetup<'_>,
    faces: &[super::description::FaceDescription],
    group: &ToolpathGroup,
    tolerance: f64,
    context: &JobContext<'_>,
) -> Result<()> {
    let mut from = group.start;
    for segment in &group.segments {
        context.check_cancelled()?;
        let to = segment.end;
        if segment.kind.is_fed() {
            let covered = from.a == to.a
                && faces.iter().any(|face| {
                    if (from.axis.get() - face.target.plane.normal()).length()
                        > (tolerance / (2. * setup.tool.diameter.0)).min(0.1)
                    {
                        return false;
                    }
                    let inside = |p| {
                        face.target.plane.coordinates(p).is_ok_and(|q| {
                            q.into_iter()
                                .take(2)
                                .zip(face.coverage.min())
                                .zip(face.coverage.max())
                                .all(|((value, low), high)| {
                                    value >= low - tolerance * 0.125
                                        && value <= high + tolerance * 0.125
                                })
                        })
                    };
                    inside(from.tip) && inside(to.tip)
                });
            if !covered {
                let mut air = ToolpathGroup {
                    tags: group.tags.clone(),
                    start: from,
                    segments: Vec::with_capacity(1),
                };
                let mut leg = *segment;
                leg.kind = openchuck_types::ir::MoveKind::Rapid;
                leg.contact_feed = None;
                air.segments.push(leg);
                verify_rapid_clearance(setup, &air, 0., context).map_err(|error| {
                    if matches!(error, PathError::Cancelled) { error } else { refuse(
                        "entry/link would remove stock outside selected Facing coverage and its necessary cutter overlap",
                    ) }
                })?;
            }
        }
        from = to;
    }
    Ok(())
}
