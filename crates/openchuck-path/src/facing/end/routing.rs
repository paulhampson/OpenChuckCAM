// SPDX-License-Identifier: AGPL-3.0-or-later
//! Nominal tangent-view routing around one axial cylindrical tab (0244).
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
        let mut low = polygon.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min);
        let high = polygon
            .iter()
            .map(|p| p[1])
            .fold(f64::NEG_INFINITY, f64::max);
        if let Some((centre, radius)) = tab {
            low = low.max(rotary.to_axis(centre, angle).height + radius);
        }
        if !low.is_finite() || !high.is_finite() {
            return Err(refuse("end view projection overflowed"));
        }
        if low >= high {
            continue;
        }
        let mut upper = high;
        for lower in LayerDistribution::Full.levels(high, low, parameters.flute_step)? {
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
