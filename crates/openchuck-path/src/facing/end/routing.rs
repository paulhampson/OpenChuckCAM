// SPDX-License-Identifier: AGPL-3.0-or-later
//! Nominal tangent-view routing around one axial cylindrical tab (0252).
//! This bounds planar remainder and tab separation of the candidate flute
//! bands; entries, links, written precision and whole setup proof remain separate.

use super::super::geometry::{CoverageRectangle, FacingEnd, LayerDistribution};
use super::super::plan::FacingSetup;
use super::{EndBand, EndFacingParameters, refuse};
use crate::error::Result;
use crate::rotary::RotaryMapping;
use openchuck_types::geometry::{Point3, UnitVec3};
use openchuck_types::job::JobContext;
use openchuck_types::units::Degrees;

/// The flute bands that face the end coverage at `target`, top first in
/// each view, and the tab remainder bound when a tab is routed round.
/// Views are the start angle alone, two opposite views where the flutes
/// cannot reach the whole end from one (decision 0504), or the tangent
/// views round one cylindrical tab (decision 0252). Bands reach the
/// written-motion allowance past the coverage edges and overlap by as
/// much (decision 0501). An end the flutes cannot reach is refused.
pub(super) fn bands(
    setup: &FacingSetup<'_>,
    parameters: &EndFacingParameters,
    coverage: CoverageRectangle,
    end: FacingEnd,
    target: f64,
    context: &JobContext<'_>,
) -> Result<(Vec<EndBand>, Option<f64>)> {
    let [y0, z0] = coverage.min();
    let [y1, z1] = coverage.max();
    let corners = [
        Point3::new(target, y0, z0),
        Point3::new(target, y1, z0),
        Point3::new(target, y1, z1),
        Point3::new(target, y0, z1),
    ];
    let rotary = RotaryMapping::of_profile(setup.machine);
    // Tabs wholly behind the target do not need an unheld-end tangent view.
    // Keep EVERY protected solid in setup.protected for whole-motion proof.
    let tabs = setup
        .tabs
        .iter()
        .filter(|tab| match end {
            FacingEnd::Far => tab.bounds().max.x >= target,
            FacingEnd::Start => tab.bounds().min.x <= target,
        })
        .collect::<Vec<_>>();
    let (views, tab, residual) = match tabs.as_slice() {
        [] => (1, None, None),
        [tab] => {
            let axial = tab.axis() == UnitVec3::X || tab.axis().get() == -UnitVec3::X.get();
            if tab.as_cuboid().is_some()
                || !axial
                || target < tab.base().x.min(tab.top().x)
                || target > tab.base().x.max(tab.top().x)
            {
                return Err(refuse(
                    "complete end routing for this tab placement is not established",
                ));
            }
            let radius = tab.base_radius()
                + (tab.top_radius() - tab.base_radius())
                    * ((target - tab.base().x) / (tab.top().x - tab.base().x));
            let centre = tab.base();
            if corners
                .iter()
                .all(|p| (p.y - centre.y).hypot(p.z - centre.z) <= radius)
            {
                return Ok((vec![], None));
            }
            if (tab.base_radius() - tab.top_radius()).abs() > 0. {
                return Err(refuse(
                    "complete routing around a non-cylindrical tab is not established for this requested face",
                ));
            }
            // Reserve one quarter for tab clearance and one eighth for the
            // analytic polygon remainder. The rest of the radial budget stays
            // available for actual written/assembly errors, checked separately.
            let clearance = 0.25 * parameters.tolerance;
            let outer = radius + clearance;
            let polygon_budget = 0.125 * parameters.tolerance;
            let geometric = (std::f64::consts::PI / (outer / (outer + polygon_budget)).acos())
                .ceil()
                .max(3.);
            let angular = (360. / parameters.max_angle_step.0).ceil().max(3.);
            let count = geometric.max(angular);
            if !count.is_finite() || count > 4096. {
                return Err(refuse("tab-facing view budget exceeded at this tolerance"));
            }
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            // Finite3..4096 above.
            let count = count as usize;
            let residual = outer / (std::f64::consts::PI / count as f64).cos() - radius;
            if !residual.is_finite() || residual > 0.5 * parameters.tolerance * (1. + 1e-9) {
                return Err(refuse(
                    "the tab-facing remainder exceeds its geometric budget",
                ));
            }
            (count, Some((centre, outer)), Some(residual))
        }
        _ => {
            return Err(refuse(
                "complete end routing around multiple tabs is not established",
            ));
        }
    };
    // The flutes limit how deep one view may reach (the owner's decision F1,
    // issue #499): at most the flute length below the stock's top at that
    // view. An end taller than that is faced from A and A + 180 degrees, each
    // view down past the axis by the written-motion allowance, and their
    // union certified by the completion proof as for tab views.
    let written = 0.125 * parameters.tolerance;
    let flute = setup.tool.flute_length.0;
    let top_at = |angle: Degrees| -> Result<f64> {
        super::super::extents::FacingExtent::WholeStock.support(
            super::super::geometry::FacingPlane::side(rotary, angle)?,
            setup.stock,
            setup.part,
            &[],
        )
    };
    let heights_at = |angle: Degrees| {
        let h = corners.map(|p| rotary.to_axis(p, angle).height);
        (
            h.iter().copied().fold(f64::INFINITY, f64::min),
            h.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        )
    };
    let (views, split) = if tab.is_none() {
        let (low, _) = heights_at(parameters.start_angle);
        if top_at(parameters.start_angle)? - (low - written) <= flute {
            (views, None)
        } else {
            (2, Some(0.))
        }
    } else {
        (views, None)
    };
    let mut bands = vec![];
    let reach = corners.iter().map(|p| p.y.hypot(p.z)).fold(0., f64::max)
        + setup.tool.overall_length.0
        + setup.tool.diameter.0;
    if !reach.is_finite() {
        return Err(refuse("end-view precision bound overflowed"));
    }
    for view in 0..views {
        context.check_cancelled()?;
        let offset = rotary.sense.sign() * 360. * view as f64 / views as f64;
        let angle = Degrees(parameters.start_angle.0 + offset);
        if !angle.0.is_finite() {
            return Err(refuse("end view angle overflowed"));
        }
        let reduced = Degrees(parameters.start_angle.0.rem_euclid(360.) + offset);
        let numeric_error = ((angle.0 - parameters.start_angle.0 - offset)
            .to_radians()
            .abs()
            * reach)
            .max(
                (rotary.tool_axis(angle).get() - rotary.tool_axis(reduced).get()).length() * reach,
            );
        if !numeric_error.is_finite() || numeric_error > 0.01 * parameters.tolerance {
            return Err(refuse(
                "end view angles have no geometric precision bound at these magnitudes",
            ));
        }
        let polygon = corners
            .map(|p| {
                let axis = rotary.to_axis(p, angle);
                [axis.across, axis.height]
            })
            .to_vec();
        let low = polygon.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min);
        let high = polygon
            .iter()
            .map(|p| p[1])
            .fold(f64::NEG_INFINITY, f64::max);
        // The bands reach past the requested edges by one eighth of the
        // tolerance, the written-motion share of the budget (0252), and
        // overlap each other by at least as much, so the coordinates the
        // controller reads back (rounded words, single-precision floats)
        // still cover the face. Without it the Baluster's End facing could
        // not be exported (issue #499). A band the tab limits keeps its
        // separation and is not carried further.
        let mut low = low - written;
        let high = high + written;
        if let Some((centre, radius)) = tab {
            low = low.max(rotary.to_axis(centre, angle).height + radius);
        }
        if let Some(split) = split {
            low = low.max(split - written);
        }
        let top = top_at(angle)?;
        if !top.is_finite() {
            return Err(refuse("end view stock top overflowed"));
        }
        if low < high && top - low > flute {
            let from = if tab.is_some() {
                format!("from each of the {views} views around the tab")
            } else {
                format!(
                    "even facing it from A {:.3} and A {:.3}",
                    parameters.start_angle.0,
                    parameters.start_angle.0 + rotary.sense.sign() * 180.
                )
            };
            return Err(too_deep(target, angle, top - low, flute, &from));
        }
        if !low.is_finite() || !high.is_finite() {
            return Err(refuse("end view projection overflowed"));
        }
        if low >= high {
            continue;
        }
        let step = parameters.flute_step.min(flute - 2. * written);
        if !step.is_finite() || step <= 0. {
            return Err(refuse(
                "the flutes are too short to overlap the bands by the written-motion allowance",
            ));
        }
        let mut upper = high;
        for lower in LayerDistribution::Full.levels(high, low, step)? {
            context.check_cancelled()?;
            let strip = clip(&clip(&polygon, lower, true), upper, false);
            let min = strip.iter().map(|p| p[0]).fold(f64::INFINITY, f64::min);
            let max = strip.iter().map(|p| p[0]).fold(f64::NEG_INFINITY, f64::max);
            if min.is_finite() && max.is_finite() && max > min {
                bands.push(EndBand {
                    angle,
                    heights: (lower, upper),
                    across: (min, max),
                });
                if bands.len() > 100_000 {
                    return Err(refuse("end flute-band budget exceeded"));
                }
            }
            upper = lower;
        }
    }
    Ok((bands, residual))
}

/// The refusal for an end deeper than the flutes reach (decision 0344 item 7
/// style: the place, the depth, the flutes and each remedy).
fn too_deep(
    target: f64,
    angle: Degrees,
    depth: f64,
    flute: f64,
    from: &str,
) -> crate::error::PathError {
    crate::error::PathError::Strategy(format!(
        "End facing: the end at X {target:.3} needs the tool {depth:.3} mm below the top of the \
         stock at A {:.3}, more than the tool's {flute:.3} mm of flute, {from}, so the tool cannot \
         face it. Use a tool with longer flutes, or face the stock down with Box facing first",
        angle.0
    ))
}

/// Intersect a convex polygon with one tool-height halfplane. Each output
/// endpoint is either a requested rectangle corner or its analytic edge cut.
fn clip(polygon: &[[f64; 2]], height: f64, above: bool) -> Vec<[f64; 2]> {
    let Some(mut previous) = polygon.last().copied() else {
        return vec![];
    };
    let inside = |point: [f64; 2]| {
        if above {
            point[1] >= height
        } else {
            point[1] <= height
        }
    };
    let mut result = vec![];
    for &point in polygon {
        let (was, is) = (inside(previous), inside(point));
        if was != is {
            let t = (height - previous[1]) / (point[1] - previous[1]);
            result.push([previous[0] + t * (point[0] - previous[0]), height]);
        }
        if is {
            result.push(point);
        }
        previous = point;
    }
    result
}
