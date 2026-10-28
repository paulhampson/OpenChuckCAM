// SPDX-License-Identifier: AGPL-3.0-or-later
//! Actual axial engagement, independent of candidate slice counts. Complete
//! projected flute footprints, including necessary overlap, recur at each
//! inward slice. A finish-only path cannot borrow missing outer removal.

use super::super::coverage::verify_end_coverage;
use super::super::end::EndFacingParameters;
use super::super::extents::FacingExtent;
use super::super::geometry::{CoverageRectangle, EndTarget, FacingEnd, LayerDistribution};
use super::super::plan::FacingSetup;
use super::refuse;
use crate::RotaryMapping;
use crate::error::Result;
use openchuck_types::ir::ToolpathGroup;
use openchuck_types::job::JobContext;
use std::collections::BTreeMap;

#[derive(Clone, Copy)]
struct Band {
    u0: f64,
    u1: f64,
    h0: f64,
    h1: f64,
}
type Bands = BTreeMap<u64, Vec<Band>>;
#[allow(clippy::too_many_arguments)]
pub(super) fn verify(
    setup: &FacingSetup<'_>,
    p: &EndFacingParameters,
    end: FacingEnd,
    target: EndTarget,
    coverage: CoverageRectangle,
    group: &ToolpathGroup,
    context: &JobContext<'_>,
) -> Result<()> {
    if !p.axial_stepdown.is_finite()
        || p.axial_stepdown <= 0.
        || p.axial_stepdown > setup.tool.diameter.0
        || !p.flute_step.is_finite()
        || p.flute_step <= 0.
        || p.flute_step > setup.tool.flute_length.0
    {
        return Err(refuse(
            "End axial/flute spacing exceeds the supported cutting limits",
        ));
    }
    let sign = if end == FacingEnd::Far { 1. } else { -1. };
    let bounds = FacingExtent::WholeStock.axial(setup.stock, setup.part, &[], 0.)?;
    let outer = if sign > 0. { bounds.1 } else { -bounds.0 };
    let levels = LayerDistribution::Equal.levels(outer, sign * target.x, p.axial_stepdown)?;
    if levels.is_empty() {
        return Err(refuse("End has no independently verified removal slice"));
    }
    let scale = std::iter::once(group.start)
        .chain(group.segments.iter().map(|s| s.end))
        .flat_map(|p| [p.tip.x.abs(), p.tip.y.abs(), p.tip.z.abs()])
        .fold(setup.tool.overall_length.0.max(1.), f64::max);
    let arithmetic = 1024. * f64::EPSILON * scale;
    if arithmetic > p.tolerance * 0.125 || p.axial_stepdown <= 4. * arithmetic {
        return Err(refuse(
            "End tolerance/stepdown is below the supported arithmetic resolution",
        ));
    }
    let r = setup.tool.diameter.0 * 0.5;
    let rotary = RotaryMapping::of_profile(setup.machine);
    let mut prefix = ToolpathGroup {
        start: group.start,
        tags: group.tags.clone(),
        segments: Vec::new(),
    };
    let mut level = 0usize;
    let mut previous_ceiling = outer;
    let mut slice_ceiling = f64::NEG_INFINITY;
    let mut previous_bands = Bands::new();
    let mut current_bands = Bands::new();
    let mut budget = 20_000_000usize;
    let mut from = group.start;
    for segment in &group.segments {
        context.check_cancelled()?;
        let to = segment.end;
        if segment.kind.is_fed() {
            let near = (sign * from.tip.x).min(sign * to.tip.x) - r;
            while levels
                .get(level)
                .is_some_and(|x| near < x - 0.25 * p.tolerance)
            {
                let plane = *levels
                    .get(level)
                    .ok_or_else(|| refuse("invalid End slice"))?;
                budget = budget
                    .checked_sub(prefix.segments.len().saturating_mul(16))
                    .ok_or_else(|| refuse("End slice verification work budget exceeded"))?;
                // This local proof plane is never a second displayed target.
                verify_end_coverage(
                    setup,
                    end,
                    EndTarget {
                        x: sign * plane,
                        ..target
                    },
                    coverage,
                    &prefix,
                    p.tolerance,
                    0.,
                    context,
                )?;
                if !slice_ceiling.is_finite() {
                    return Err(refuse("End has no actual preceding flute sweep"));
                }
                previous_ceiling = slice_ceiling;
                slice_ceiling = f64::NEG_INFINITY;
                previous_bands = std::mem::take(&mut current_bands);
                level += 1;
            }
            let plane = *levels
                .get(level)
                .ok_or_else(|| refuse("End cut goes behind the finished target"))?;
            if near <= plane + 0.25 * p.tolerance {
                if previous_ceiling - near > p.axial_stepdown + arithmetic {
                    return Err(refuse(
                        "an actual End cut exceeds the selected maximum axial stepdown",
                    ));
                }
                // Across-going flute sweeps clear translated copies of their
                // full projected bands, including cutter caps outside extent.
                if from.a == to.a && (from.tip.x - to.tip.x).abs() <= arithmetic {
                    let a = rotary.to_axis(from.tip, from.a);
                    let b = rotary.to_axis(to.tip, to.a);
                    if (a.across - b.across).abs() > 0. {
                        let band = Band {
                            u0: a.across.min(b.across),
                            u1: a.across.max(b.across),
                            h0: a.height.max(b.height),
                            h1: a.height.min(b.height) + setup.tool.flute_length.0,
                        };
                        let angle = from.a.0.rem_euclid(360.);
                        let key = if angle == 0. { 0 } else { angle.to_bits() };
                        if level > 0 {
                            let mut covered = false;
                            if let Some(prior) = previous_bands.get(&key) {
                                for old in prior {
                                    budget = budget.checked_sub(1).ok_or_else(|| {
                                        refuse("End overlap engagement work budget exceeded")
                                    })?;
                                    if old.u0 <= band.u0 + arithmetic
                                        && old.u1 >= band.u1 - arithmetic
                                        && old.h0 <= band.h0 + arithmetic
                                        && old.h1 >= band.h1 - arithmetic
                                    {
                                        covered = true;
                                        break;
                                    }
                                }
                            }
                            if !covered {
                                return Err(refuse(
                                    "deeper End flute overlap has no actual prior full-band removal at this view",
                                ));
                            }
                        }
                        current_bands.entry(key).or_default().push(band);
                        slice_ceiling = slice_ceiling.max(near);
                    }
                }
            }
        }
        prefix.segments.push(*segment);
        from = to;
    }
    if level + 1 != levels.len() {
        return Err(refuse("End actual slices do not reach the final target"));
    }
    Ok(())
}
