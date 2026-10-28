// SPDX-License-Identifier: AGPL-3.0-or-later
//! Independent End slab coverage by actual finite flute sweeps. This is a
//! nominal geometric proof, separate from tool clearance and post precision.

use openchuck_types::geometry::UnitVec3;
use openchuck_types::ir::ToolpathGroup;
use openchuck_types::job::JobContext;
use openchuck_types::tool::ToolShape;
use openchuck_types::units::Degrees;

use super::super::extents::FacingExtent;
use super::super::geometry::{CoverageRectangle, EndTarget, FacingEnd};
use super::super::plan::FacingSetup;
use super::refuse;
use crate::error::Result;
use crate::rotary::RotaryMapping;

/// Prove actual fixed-A fed flute sweeps cover the entire requested End slab,
/// from a bounded outward tolerance plane to original stock's unheld end.
/// Missing outer layers, internal flute gaps and missing final cuts refuse.
/// Protected axial solids are excluded only where they actually exist; any
/// residual around them must fit a separately reserved geometric budget.
/// `margin` erodes guaranteed removal for caller-supplied motion uncertainty.
/// This does not establish that the caller's written-motion bound is valid,
/// or certify protected/body/shank/rapid/machine clearance.
#[allow(clippy::too_many_arguments)]
pub fn verify_end_coverage(
    setup: &FacingSetup<'_>,
    end: FacingEnd,
    target: EndTarget,
    coverage: CoverageRectangle,
    group: &ToolpathGroup,
    tolerance: f64,
    margin: f64,
    context: &JobContext<'_>,
) -> Result<()> {
    context.check_cancelled()?;
    setup.tool.validate()?;
    setup.stock.validate()?;
    if !matches!(setup.tool.shape, ToolShape::FlatEnd)
        || !target.x.is_finite()
        || !tolerance.is_finite()
        || tolerance <= 0.
        || !margin.is_finite()
        || margin < 0.
        || margin > 0.125 * tolerance
    {
        return Err(refuse(
            "unsupported end tool or invalid coverage/error budget",
        ));
    }
    if group.segments.len() > 1_000_000 {
        return Err(refuse("end coverage move budget exceeded"));
    }
    let sign = if end == FacingEnd::Far { 1. } else { -1. };
    let x = FacingExtent::WholeStock.axial(setup.stock, setup.part, &[], 0.)?;
    let outer = if sign > 0. { x.1 } else { -x.0 };
    let low = sign * target.x + 0.25 * tolerance;
    let tab_budget = 0.5 * tolerance - 2. * margin;
    let radius = 0.5 * setup.tool.diameter.0;
    let flute = setup.tool.flute_length.0;
    let rotary = RotaryMapping::of_profile(setup.machine);
    let [y0, z0] = coverage.min();
    let [y1, z1] = coverage.max();
    let initial = vec![[y0, z0], [y1, z0], [y1, z1], [y0, z1]];
    if y1 <= y0 || z1 <= z0 {
        return Err(refuse("end coverage must have positive area"));
    }
    let mut sweeps = Vec::new();
    let mut boundaries = vec![low, outer];
    let mut previous = group.start;
    if !previous.tip.is_finite() || !previous.a.0.is_finite() {
        return Err(refuse("end coverage start is not finite"));
    }
    for segment in &group.segments {
        context.check_cancelled()?;
        segment.validate("end coverage")?;
        let to = segment.end;
        if segment.kind.is_fed() {
            let near = (sign * previous.tip.x).min(sign * to.tip.x) - radius;
            if near < sign * target.x - 0.25 * tolerance + margin {
                return Err(refuse("an end cut goes behind the finished target plane"));
            }
            if previous.a == to.a {
                let a = rotary.to_axis(previous.tip, previous.a);
                let b = rotary.to_axis(to.tip, to.a);
                let s = Sweep {
                    angle: to.a,
                    x: [previous.tip.x, to.tip.x],
                    u: [a.across.min(b.across), a.across.max(b.across)],
                    h: [
                        a.height.max(b.height) + margin,
                        a.height.min(b.height) + flute - margin,
                    ],
                };
                if s.u[0] < s.u[1] && s.h[0] < s.h[1] {
                    let common_lo = (previous.tip.x.max(to.tip.x) - radius + margin).max(x.0);
                    let common_hi = (previous.tip.x.min(to.tip.x) + radius - margin).min(x.1);
                    let (lo, hi) = if sign > 0. {
                        (common_lo, common_hi)
                    } else {
                        (-common_hi, -common_lo)
                    };
                    if hi > low && lo < outer && hi > lo {
                        boundaries.extend([lo.max(low), hi.min(outer)]);
                        sweeps.push(s);
                    }
                }
            }
        }
        previous = to;
    }
    if low >= outer {
        return Ok(());
    }
    if sweeps.len() > 100_000 {
        return Err(refuse("end coverage sweep budget exceeded"));
    }
    boundaries.sort_by(f64::total_cmp);
    boundaries.dedup();
    let scale = [y0, y1, z0, z1, x.0, x.1, flute, radius]
        .into_iter()
        .map(f64::abs)
        .fold(1., f64::max);
    if !scale.is_finite() || scale * f64::EPSILON * 1024. > 0.001 * tolerance {
        return Err(refuse(
            "end coverage coordinates have no adequate arithmetic precision",
        ));
    }
    let mut budget = 20_000_000usize;
    for pair in boundaries.windows(2) {
        context.check_cancelled()?;
        let &[lo, hi] = pair else {
            return Err(refuse("invalid end coverage slab"));
        };
        let physical = [sign * lo, sign * hi];
        let mut views = std::collections::BTreeMap::<u64, (Degrees, Vec<[f64; 4]>)>::new();
        for sweep in &sweeps {
            spend(&mut budget, 1)?;
            let dx = physical
                .into_iter()
                .flat_map(|x| sweep.x.map(|c| (x - c).abs()))
                .fold(0., f64::max)
                + margin;
            if dx > radius {
                continue;
            }
            let width = ((radius - dx) * (radius + dx)).max(0.).sqrt() - margin;
            if width < 0. {
                continue;
            }
            let rect = [
                sweep.u[0] - width,
                sweep.u[1] + width,
                sweep.h[0],
                sweep.h[1],
            ];
            let angle = sweep.angle.0.rem_euclid(360.);
            let key = if angle == 0. { 0 } else { angle.to_bits() };
            let (_, rectangles) = views
                .entry(key)
                .or_insert_with(|| (sweep.angle, Vec::new()));
            spend(&mut budget, rectangles.len().saturating_add(1))?;
            let contains = |a: &[f64; 4], b: &[f64; 4]| {
                a[0] <= b[0] && a[1] >= b[1] && a[2] <= b[2] && a[3] >= b[3]
            };
            // Exact set dominance, not geometric sampling or a tolerance:
            // repeated axial slices can guarantee nested rectangles at the
            // SAME physical view. Their union equals the retained larger one.
            if rectangles.iter().any(|old| contains(old, &rect)) {
                continue;
            }
            rectangles.retain(|old| !contains(&rect, old));
            rectangles.push(rect);
        }
        let mut remaining = vec![initial.clone()];
        for (angle, rect) in views
            .into_values()
            .flat_map(|(a, rectangles)| rectangles.into_iter().map(move |r| (a, r)))
        {
            spend(&mut budget, remaining.len().saturating_add(1))?;
            let across = rotary
                .to_model(
                    crate::rotary::AxisPosition {
                        along: 0.,
                        across: 1.,
                        height: 0.,
                    },
                    angle,
                )
                .to_vec();
            let up = rotary.tool_axis(angle).get();
            let mut next = Vec::new();
            for polygon in remaining {
                context.check_cancelled()?;
                let mut inside = polygon;
                for (axis, bound, lower) in [
                    (0, rect[0], true),
                    (0, rect[1], false),
                    (1, rect[2], true),
                    (1, rect[3], false),
                ] {
                    let normal = if axis == 0 { across } else { up };
                    let n = [normal.y, normal.z];
                    let outside = clip(&inside, n, bound, !lower);
                    if area(&outside) {
                        next.push(outside);
                    }
                    inside = clip(&inside, n, bound, lower);
                    if !area(&inside) {
                        break;
                    }
                }
                if next.len() > 100_000 {
                    return Err(refuse("end coverage polygon budget exceeded"));
                }
            }
            remaining = next;
            if remaining.is_empty() {
                break;
            }
        }
        for polygon in remaining {
            context.check_cancelled()?;
            spend(
                &mut budget,
                setup
                    .protected
                    .len()
                    .saturating_add(setup.tabs.len())
                    .saturating_mul(polygon.len())
                    .saturating_add(1),
            )?;
            let protected = setup.protected.iter().chain(setup.tabs).any(|solid| {
                let axis = solid.axis();
                if solid.as_cuboid().is_some()
                    || !(axis == UnitVec3::X || axis.get() == -UnitVec3::X.get())
                {
                    return false;
                }
                let base = solid.base();
                let top = solid.top();
                let xmin = physical[0].min(physical[1]);
                let xmax = physical[0].max(physical[1]);
                if xmin < base.x.min(top.x) || xmax > base.x.max(top.x) {
                    return false;
                }
                let r = |x| {
                    solid.base_radius()
                        + (solid.top_radius() - solid.base_radius())
                            * ((x - base.x) / (top.x - base.x))
                };
                let radius = r(xmin).min(r(xmax)) + tab_budget;
                polygon
                    .iter()
                    .all(|p| (p[0] - base.y).hypot(p[1] - base.z) <= radius)
            });
            if !protected {
                return Err(refuse(
                    "the actual end sweeps leave an unfinished required region in an axial layer",
                ));
            }
        }
    }
    Ok(())
}

struct Sweep {
    angle: Degrees,
    x: [f64; 2],
    u: [f64; 2],
    h: [f64; 2],
}
type Polygon = Vec<[f64; 2]>;
fn clip(polygon: &[[f64; 2]], normal: [f64; 2], bound: f64, above: bool) -> Polygon {
    let Some(mut previous) = polygon.last().copied() else {
        return vec![];
    };
    let distance = |p: [f64; 2]| p[0] * normal[0] + p[1] * normal[1] - bound;
    let inside = |d: f64| if above { d >= 0. } else { d <= 0. };
    let mut out = Vec::new();
    for &p in polygon {
        let a = distance(previous);
        let b = distance(p);
        if inside(a) != inside(b) {
            let t = a / (a - b);
            out.push([
                previous[0] + t * (p[0] - previous[0]),
                previous[1] + t * (p[1] - previous[1]),
            ]);
        }
        if inside(b) {
            out.push(p);
        }
        previous = p;
    }
    out
}
fn area(p: &[[f64; 2]]) -> bool {
    let Some(a) = p.first() else {
        return false;
    };
    p.windows(2).any(|pair| {
        let [b, c] = pair else {
            return false;
        };
        ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])).abs() > 0.
    })
}
fn spend(budget: &mut usize, n: usize) -> Result<()> {
    *budget = budget
        .checked_sub(n)
        .ok_or_else(|| refuse("end coverage work budget exceeded"))?;
    Ok(())
}
