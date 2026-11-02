// SPDX-License-Identifier: AGPL-3.0-or-later
//! Analytic non-cutting shank clearance from original stock minus guaranteed
//! removal prisms supplied by preceding ACTUAL fixed-A fed moves. Sampled
//! stock fields and planned passes are deliberately not inputs to this proof.

use openchuck_types::ir::ToolpathGroup;
use openchuck_types::job::JobContext;
use openchuck_types::tool::ToolShape;
use openchuck_types::units::Degrees;

use super::{refuse, verify_machine_motion};
use crate::error::Result;
use crate::facing::extents::FacingExtent;
use crate::facing::geometry::FacingPlane;
use crate::facing::plan::FacingSetup;
use crate::rotary::RotaryMapping;

type FluteSweep = (Degrees, [[f64; 2]; 2], (f64, f64));

/// Prove the continuous non-cutting shank avoids remaining initial stock.
/// Each fixed-A fed move can add a guaranteed rectangular subset of its flute
/// sweep AFTER its shank was checked. No rapid supplies removal evidence; no
/// proposed, raised-away or missing pass is counted. Removal at other A views
/// is conservatively ignored. `margin` inflates the shank and erodes removal.
/// This is a nominal IR proof; the caller supplies a proved written-motion
/// error margin and separately proves full-tool protection/rapid/completion.
pub fn verify_shank_clearance(
    setup: &FacingSetup<'_>,
    group: &ToolpathGroup,
    margin: f64,
    context: &JobContext<'_>,
) -> Result<()> {
    verify_machine_motion(group, setup.machine, context)?;
    setup.tool.validate()?;
    setup.stock.validate()?;
    if !matches!(setup.tool.shape, ToolShape::FlatEnd) {
        return Err(refuse("shank proof requires a supported flat tool"));
    }
    if !margin.is_finite() || margin < 0. {
        return Err(refuse("shank error margin must be finite and nonnegative"));
    }
    if setup.tool.flute_length == setup.tool.overall_length {
        return Ok(());
    }
    let rotary = RotaryMapping::of_profile(setup.machine);
    let r = setup.tool.shank_diameter.0 * 0.5 + margin;
    let cut_r = setup.tool.diameter.0 * 0.5;
    let flute = setup.tool.flute_length.0;
    let length = setup.tool.overall_length.0;
    let mut previous = group.start;
    let mut removed: Vec<(Degrees, Prism)> = Vec::new();
    let mut capsules: Vec<FluteSweep> = Vec::new();
    let mut budget = 10_000_000usize;
    let moves = std::iter::once((group.start, false))
        .chain(group.segments.iter().map(|s| (s.end, s.kind.is_fed())));
    for (index, (end, fed)) in moves.enumerate() {
        context.check_cancelled()?;
        let projection_work = match setup.stock {
            openchuck_types::stock::Stock::Imported { mesh } => {
                mesh.triangles.len().saturating_mul(12)
            }
            openchuck_types::stock::Stock::OffsetBoundingBox { .. } => {
                setup.part.vertices.len().saturating_mul(4)
            }
            _ => 1,
        };
        spend(&mut budget, projection_work)?;
        let plane = FacingPlane::side(rotary, end.a)?;
        let region = FacingExtent::WholeStock.planar(plane, setup.stock, setup.part, &[], 0.)?;
        let stock = Prism {
            low: [
                region.min()[0],
                region.min()[1],
                -FacingExtent::WholeStock.support(
                    plane.opposite(),
                    setup.stock,
                    setup.part,
                    &[],
                )?,
            ],
            high: [
                region.max()[0],
                region.max()[1],
                FacingExtent::WholeStock.support(plane, setup.stock, setup.part, &[])?,
            ],
        };
        let from = rotary.to_axis(previous.tip, previous.a);
        let to = rotary.to_axis(end.tip, end.a);
        let shank = Prism {
            low: [
                previous.tip.x.min(end.tip.x) - r,
                from.across.min(to.across) - r,
                from.height.min(to.height) + flute - margin,
            ],
            high: [
                previous.tip.x.max(end.tip.x) + r,
                from.across.max(to.across) + r,
                from.height.max(to.height) + length + margin,
            ],
        };
        stock.finite()?;
        shank.finite()?;
        if previous.a != end.a {
            // Throughout rotary motion, model X is affine. A global stock
            // radius also bounds every orientation, unlike the end-A box.
            let radius = super::enclosing_radius(setup, context)?;
            if !(shank.high[0] <= stock.low[0]
                || shank.low[0] >= stock.high[0]
                || shank.low[2] >= radius)
            {
                return Err(refuse(
                    "rotary shank move has no continuous remaining-stock separation",
                ));
            }
        } else if let Some(query) = shank.intersection(stock) {
            spend(&mut budget, removed.len())?;
            let prisms = removed
                .iter()
                .filter(|(a, _)| *a == end.a)
                .filter_map(|(_, p)| p.intersection(query))
                .collect::<Vec<_>>();
            let mut known = covered(query, &prisms, &mut budget, context)?;
            if !known && cut_r - margin >= r {
                spend(&mut budget, capsules.len())?;
                let current = [[from.along, from.across], [to.along, to.across]];
                // An actual prior flute sweeps a capsule in XY times its
                // common height interval, INCLUDING circular end caps. If
                // both current centreline endpoints lie inside the eroded
                // prior capsule, convexity contains the entire shank sweep.
                // This complements the inscribed-prism proof; no rectangular
                // corner outside a circular cutter is assumed removed.
                known = capsules.iter().any(|(a, line, h)| {
                    *a == end.a
                        && h.0 <= query.low[2]
                        && h.1 >= query.high[2]
                        && current
                            .into_iter()
                            .all(|p| distance(p, *line) <= cut_r - margin - r)
                });
            }
            if !known {
                return Err(refuse(&format!(
                    "move {} shank reaches remaining stock without actual prior removal",
                    index + 1
                )));
            }
        }
        if fed && previous.a == end.a {
            if capsules.len() >= 100_000 {
                return Err(refuse("shank capsule budget exceeded"));
            }
            capsules.push((
                end.a,
                [[from.along, from.across], [to.along, to.across]],
                (
                    from.height.max(to.height) + margin,
                    from.height.min(to.height) + flute - margin,
                ),
            ));
            // For every U strictly between the endpoint U values there is a
            // centre on this controller-linear segment. The common X diameter
            // interval and common flute-height interval give an inscribed
            // removed prism, even for a diagonal/raised segment. End caps add
            // no assumed removal beyond that U span.
            let prism = Prism {
                low: [
                    previous.tip.x.max(end.tip.x) - cut_r + margin,
                    from.across.min(to.across) + margin,
                    from.height.max(to.height) + margin,
                ],
                high: [
                    previous.tip.x.min(end.tip.x) + cut_r - margin,
                    from.across.max(to.across) - margin,
                    from.height.min(to.height) + flute - margin,
                ],
            };
            prism.finite()?;
            if prism.positive() {
                if removed.len() >= 100_000 {
                    return Err(refuse("shank proof removal budget exceeded"));
                }
                removed.push((end.a, prism));
            }
        }
        previous = end;
    }
    Ok(())
}

fn distance(p: [f64; 2], line: [[f64; 2]; 2]) -> f64 {
    let [a, b] = line;
    let d = [b[0] - a[0], b[1] - a[1]];
    let length = d[0] * d[0] + d[1] * d[1];
    let t = if length > 0. {
        ((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / length
    } else {
        0.
    }
    .clamp(0., 1.);
    (p[0] - a[0] - t * d[0]).hypot(p[1] - a[1] - t * d[1])
}

#[derive(Clone, Copy)]
struct Prism {
    low: [f64; 3],
    high: [f64; 3],
}
impl Prism {
    fn finite(self) -> Result<()> {
        if self.low.into_iter().chain(self.high).all(f64::is_finite) {
            Ok(())
        } else {
            Err(refuse("shank proof has no finite geometry bound"))
        }
    }
    fn positive(self) -> bool {
        self.low.into_iter().zip(self.high).all(|(a, b)| a < b)
    }
    fn intersection(self, other: Self) -> Option<Self> {
        let [x, u, h] = self.low;
        let [a, b, c] = other.low;
        let [xx, uu, hh] = self.high;
        let [aa, bb, cc] = other.high;
        let p = Self {
            low: [x.max(a), u.max(b), h.max(c)],
            high: [xx.min(aa), uu.min(bb), hh.min(cc)],
        };
        p.positive().then_some(p)
    }
}

// Exact rectangular union inclusion by slabs in X, then U, then merged height
// intervals. No cell-centre samples or stock-resolution tolerance are used.
fn covered(
    query: Prism,
    prisms: &[Prism],
    budget: &mut usize,
    context: &JobContext<'_>,
) -> Result<bool> {
    let mut xs = vec![query.low[0], query.high[0]];
    for p in prisms {
        xs.extend([p.low[0], p.high[0]]);
    }
    xs.sort_by(f64::total_cmp);
    xs.dedup();
    for x in xs.windows(2) {
        let &[x0, x1] = x else {
            return Err(refuse("invalid shank proof slab"));
        };
        context.check_cancelled()?;
        spend(budget, prisms.len())?;
        let ps = prisms
            .iter()
            .filter(|p| p.low[0] <= x0 && p.high[0] >= x1)
            .collect::<Vec<_>>();
        let mut us = vec![query.low[1], query.high[1]];
        for p in &ps {
            us.extend([p.low[1], p.high[1]]);
        }
        us.sort_by(f64::total_cmp);
        us.dedup();
        for u in us.windows(2) {
            let &[u0, u1] = u else {
                return Err(refuse("invalid shank proof slab"));
            };
            context.check_cancelled()?;
            spend(budget, ps.len())?;
            let mut hs = ps
                .iter()
                .filter(|p| p.low[1] <= u0 && p.high[1] >= u1)
                .map(|p| (p.low[2], p.high[2]))
                .collect::<Vec<_>>();
            hs.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut top = query.low[2];
            for (low, high) in hs {
                if low > top {
                    return Ok(false);
                }
                top = top.max(high);
                if top >= query.high[2] {
                    break;
                }
            }
            if top < query.high[2] {
                return Ok(false);
            }
        }
    }
    Ok(true)
}
fn spend(budget: &mut usize, n: usize) -> Result<()> {
    *budget = budget
        .checked_sub(n)
        .ok_or_else(|| refuse("shank proof union budget exceeded"))?;
    Ok(())
}
