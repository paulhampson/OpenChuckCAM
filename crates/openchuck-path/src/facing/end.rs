// SPDX-License-Identifier: AGPL-3.0-or-later
//! Candidate flank-end geometry (OP-24;0252), before shared motion assembly.
//! Like the box candidate planner, this does not clip protections or certify
//! tool clearance/completion. A host must never publish these passes directly.

mod routing;

use openchuck_types::geometry::Point3;
use openchuck_types::job::JobContext;
use openchuck_types::tool::ToolShape;
use openchuck_types::units::Degrees;

use super::extents::FacingExtent;
use super::geometry::{CoverageRectangle, EndTarget, FacingEnd, FacingPlane, LayerDistribution};
use super::plan::FacingSetup;
use crate::error::{PathError, Result};
use crate::rotary::{AxisPosition, RotaryMapping};
use crate::strategy::{CutPoint, Pass};

/// Explicit end settings; the side layer-distribution option never changes
/// the accepted equal axial slice construction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EndFacingParameters {
    /// Stock/part/part-and-tabs projected YZ coverage.
    pub extent: FacingExtent,
    /// Margin at all four projected edges.
    pub margin: f64,
    /// Maximum inward axial slice, limited by the cutting diameter: the
    /// operation's Stepover, a width across the tool (decision record 0547).
    pub axial_stepdown: f64,
    /// Maximum spacing between flute bands; no greater than flute length:
    /// the operation's Stepdown, a depth along the tool (decision record 0547).
    pub flute_step: f64,
    /// Initial displayed A for end-facing views.
    pub start_angle: Degrees,
    /// Maximum angular spacing between tab-facing views.
    pub max_angle_step: Degrees,
    /// Existing operation allowance, applied outward once.
    pub allowance: f64,
    /// Total operation geometric tolerance.
    pub tolerance: f64,
    /// The operation's approach distance: how far the complete tool stays
    /// from the stock's end face where each band starts and ends, so a rapid
    /// beside the face keeps the clearance a rapid above the stock keeps
    /// (OP-14; issue #499). Never less than a quarter of the tolerance.
    pub approach: f64,
}

impl EndFacingParameters {
    /// Translate resolved End-mode settings into the candidate planner's inputs.
    pub fn read(parameters: &crate::params::Parameters) -> Result<Self> {
        use super::parameter;
        use crate::params::names;
        use crate::roughing::parameter as roughing;
        if parameters.text(parameter::SHAPE)? != parameter::END {
            return Err(refuse("expected End settings"));
        }
        Ok(Self {
            extent: FacingExtent::from_name(parameters.text(parameter::EXTENT)?)?,
            margin: parameters.number(parameter::MARGIN)?,
            // The slice along X is across the tool and the band along it,
            // as a tool's cutting data defines Stepover and Stepdown (TL-05;
            // decision record 0547 item 3).
            axial_stepdown: parameters.number(names::STEPOVER)?,
            flute_step: parameters.number(roughing::STEPDOWN)?,
            start_angle: Degrees(parameters.number(roughing::START_ANGLE)?),
            max_angle_step: Degrees(parameters.number(roughing::MAX_ANGLE_STEP)?),
            allowance: parameters.number(names::STOCK_TO_LEAVE)?,
            tolerance: parameters.number(names::TOLERANCE)?,
            approach: parameters.number(names::APPROACH_DISTANCE)?,
        })
    }
}

/// Unassembled candidates: in particular, a tab crossing this rectangle has
/// not been declared reachable or safe by a successful candidate construction.
#[derive(Debug, Clone, PartialEq)]
pub struct EndPassPlan {
    /// Automatically selected opposite end.
    pub end: FacingEnd,
    /// Conservative placed-source target and displayed length.
    pub target: EndTarget,
    /// Requested YZ region, distinct from actual finite-cutter overlap.
    pub coverage: CoverageRectangle,
    /// Equal slice planes in model X, ending at the exact derived target.
    pub axial_planes: Vec<f64>,
    /// Required flute bands (lowZ,highZ), top first. The actual flute can
    /// overlap a band already removed; these are coverage bands, not a tool envelope.
    pub bands: Vec<EndBand>,
    /// Upper bound on the nominal radial remainder at a supported cylinder
    /// tab, separate from source/written-motion error; None without tab routing.
    pub tab_residual_bound: Option<f64>,
    /// Candidate straight, fixed-A passes, axial slice then ordered view/band.
    pub passes: Vec<Pass>,
}

/// One fixed-A finite flute band, expressed in shared machine-axis coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EndBand {
    /// Displayed A, before shared assembly fits equivalent whole turns.
    pub angle: Degrees,
    /// Required low/high heights along the tool axis, top bands first per view.
    pub heights: (f64, f64),
    /// Across-axis sweep covering that clipped band of the requested rectangle.
    pub across: (f64, f64),
}

/// Construct the base straight-flank sweeps. Protection-aware routing,
/// indexed tab views and full-tool access are separate planning/validation
/// stages. None is inferred from candidate success or from warnings.
pub fn plan_end(
    setup: &FacingSetup<'_>,
    parameters: &EndFacingParameters,
    context: &JobContext<'_>,
) -> Result<EndPassPlan> {
    context.check_cancelled()?;
    let end = FacingEnd::opposite_held(setup.held[0].is_some(), setup.held[1].is_some())?;
    setup.part.validate()?;
    setup.stock.validate()?;
    setup.tool.validate()?;
    setup.machine.validate()?;
    for (lo, hi) in setup.held.iter().flatten() {
        if !lo.is_finite() || !hi.is_finite() || hi <= lo {
            return Err(refuse("held intervals must be finite with positive length"));
        }
    }
    if !matches!(setup.tool.shape, ToolShape::FlatEnd)
        || setup.tool.shank_diameter.0 > setup.tool.diameter.0
    {
        return Err(refuse(
            "this straight-flank construction requires a flat end mill with no larger shank",
        ));
    }
    if !parameters.axial_stepdown.is_finite()
        || parameters.axial_stepdown <= 0.
        || parameters.axial_stepdown > setup.tool.diameter.0
    {
        return Err(refuse(
            "the Stepover (slice thickness along X) must be above zero and no greater than the cutting diameter",
        ));
    }
    if !parameters.flute_step.is_finite()
        || parameters.flute_step <= 0.
        || parameters.flute_step > setup.tool.flute_length.0
    {
        return Err(refuse(
            "the Stepdown (band along the tool) must be above zero and no greater than the flute length",
        ));
    }
    if !parameters.start_angle.0.is_finite()
        || !parameters.max_angle_step.0.is_finite()
        || parameters.max_angle_step.0 <= 0.
    {
        return Err(refuse(
            "end view angles must be finite with positive maximum spacing",
        ));
    }
    if !parameters.allowance.is_finite()
        || parameters.allowance < 0.
        || !parameters.tolerance.is_finite()
        || parameters.tolerance <= 0.
    {
        return Err(refuse("invalid stock allowance or tolerance"));
    }
    if !parameters.approach.is_finite() || parameters.approach < 0. {
        return Err(refuse(
            "the approach distance must be finite and not negative",
        ));
    }
    let source_error = setup
        .projection_error
        .filter(|v| v.is_finite() && *v >= 0.)
        .ok_or_else(|| {
            refuse("the whole placed source has no established projection-error bound")
        })?;
    if source_error > 0.25 * parameters.tolerance {
        return Err(refuse(
            "the source projection bound exceeds its geometric error budget",
        ));
    }
    let part_x = FacingExtent::Part.axial(setup.stock, setup.part, &[], 0.)?;
    let stock_x = FacingExtent::WholeStock.axial(setup.stock, setup.part, &[], 0.)?;
    let target = end.target(part_x, parameters.allowance)?;
    let sign = if end == FacingEnd::Far { 1. } else { -1. };
    // Keep the displayed target and length model-derived. A source bound
    // reserves cutting accuracy inside tolerance, not another allowance.
    let cutting_x = target.x + sign * source_error;
    // The stock ends at (or inside) the part's end: nothing is past it to face
    // (issue #471, the owner's note N2), said in the user's words.
    let (part_end, stock_end) = match end {
        FacingEnd::Far => (part_x.1, stock_x.1),
        FacingEnd::Start => (part_x.0, stock_x.0),
    };
    let flush = part_end.is_finite()
        && stock_end.is_finite()
        && sign * (stock_end - part_end) <= parameters.tolerance;
    let nothing_past = || {
        refuse(&format!(
            "the stock ends at the part's end at X {part_end:.3}: there is no stock past this \
             end to face. If the real blank is longer, set the stock's length to match it"
        ))
    };
    if !cutting_x.is_finite() || cutting_x < stock_x.0 || cutting_x > stock_x.1 {
        if flush {
            return Err(nothing_past());
        }
        return Err(refuse("the target end plane is outside supplied stock"));
    }
    let coverage = parameters.extent.planar(
        FacingPlane::end(end),
        setup.stock,
        setup.part,
        setup.tabs,
        parameters.margin,
    )?;
    let [y0, z0] = coverage.min();
    let [y1, z1] = coverage.max();
    if y1 <= y0 || z1 <= z0 {
        return Err(refuse("the requested end has no positive-area extent"));
    }
    let (sign, outer) = match end {
        FacingEnd::Start => (-1., -stock_x.0),
        FacingEnd::Far => (1., stock_x.1),
    };
    if outer - sign * cutting_x > parameters.tolerance {
        reject_known_frustum_obstruction(
            setup,
            end,
            cutting_x,
            coverage,
            parameters.tolerance,
            context,
        )?;
    }
    let axial_planes = LayerDistribution::Equal
        .levels(outer, sign * cutting_x, parameters.axial_stepdown)?
        .into_iter()
        .map(|h| sign * h)
        .collect::<Vec<_>>();
    // With no stock to leave the target is the flush stock end itself, so no
    // layer is cut (the review of #491): refused the same way, not returned
    // as an empty plan. Stock past the end by any depth still gets its layer.
    if axial_planes.is_empty() && flush {
        return Err(nothing_past());
    }
    let (bands, tab_residual_bound) =
        routing::bands(setup, parameters, coverage, end, cutting_x, context)?;
    let count = axial_planes
        .len()
        .checked_mul(bands.len())
        .filter(|count| *count <= 1_000_000)
        .ok_or_else(|| refuse("end facing pass budget exceeded"))?;
    let mut passes = Vec::with_capacity(count);
    let radius = 0.5 * setup.tool.diameter.0;
    let outside = setup.protected.iter().fold(outer, |outside, solid| {
        let bounds = solid.bounds();
        outside.max(if sign > 0. {
            bounds.max.x
        } else {
            -bounds.min.x
        })
    });
    // Axial approach/departure are part of each material-removal slice.
    // Shared entry/retract starts at these air points, keeping its selected
    // style and semantics. This is not a stock-edge cutting extension. The
    // complete tool stays the approach distance clear of the end face (and
    // of any protection beyond it), so the shared rapid descent beside the
    // face keeps the same allowance as one stopping above the stock (OP-14;
    // issue #499). It was a quarter of the tolerance, 2.5 um by default.
    let gap = parameters.approach.max(0.25 * parameters.tolerance);
    let air_x = sign * (outside + radius + gap);
    if !air_x.is_finite() || sign * air_x - radius <= outside {
        return Err(refuse(
            "cannot establish complete-tool axial air clearance at these coordinates",
        ));
    }
    let rotary = RotaryMapping::of_profile(setup.machine);
    // Where there is air beside a band's end, its sweep starts or ends there
    // with the whole tool the approach distance clear of the stock, and the
    // flank enters the face from its edge; where there is none it keeps the
    // move along X into the face from air beyond it (decision record 0547
    // item 1).
    let beside = bands
        .iter()
        .map(|band| routing::air_beside(setup, end, cutting_x, band, 0.125 * parameters.tolerance))
        .collect::<Result<Vec<_>>>()?;
    let clear = radius + gap;
    let paired = bands.iter().zip(beside).collect::<Vec<_>>();
    let views = paired
        .chunk_by(|a, b| a.0.angle == b.0.angle)
        .collect::<Vec<_>>();
    for (slice, &x) in axial_planes.iter().enumerate() {
        // Reverse VIEW order on alternate slices, keeping top-first bands
        // within each view. Repeated physical views then retain the same
        // controller counter phase instead of accumulating whole turns.
        let ordered = if slice % 2 == 0 {
            views.to_vec()
        } else {
            views.iter().rev().copied().collect::<Vec<_>>()
        };
        for &(band, (low_air, high_air)) in ordered.into_iter().flatten() {
            context.check_cancelled()?;
            let (lo, hi) = band.heights;
            let centre_x = x + sign * radius;
            if !centre_x.is_finite() {
                return Err(refuse("flank compensation overflowed"));
            }
            let mut route = Vec::with_capacity(4);
            if low_air {
                route.push((centre_x, band.across.0 - clear));
            } else {
                route.push((air_x, band.across.0));
                route.push((centre_x, band.across.0));
            }
            if high_air {
                route.push((centre_x, band.across.1 + clear));
            } else {
                route.push((centre_x, band.across.1));
                route.push((air_x, band.across.1));
            }
            let points = route
                .into_iter()
                .map(|(along, across)| CutPoint {
                    tip: rotary.to_model(
                        AxisPosition {
                            along,
                            across,
                            height: lo,
                        },
                        band.angle,
                    ),
                    a: band.angle,
                    contact: rotary.to_model(
                        AxisPosition {
                            along: along - sign * radius,
                            across,
                            height: lo + 0.5 * (hi - lo),
                        },
                        band.angle,
                    ),
                })
                .collect::<Vec<_>>();
            passes.push(Pass {
                helical_pass_start: false,
                low_link: false,
                points,
                stock: None,
                rapids: vec![],
            });
        }
    }
    Ok(EndPassPlan {
        end,
        target,
        coverage,
        axial_planes,
        bands,
        tab_residual_bound,
        passes,
    })
}

/// A positive obstruction witness, never a general clearance certificate.
/// For any A, a flute touching (x,y,z) on its tangent flank also occupies
/// (x+q*d,y,z) for every0<d<diameter, at the same flute height. An adjacent
/// outward-widening frustum therefore blocks a radial band of width
/// slope*min(diameter,remaining tab length), independently of indexing.
fn reject_known_frustum_obstruction(
    setup: &FacingSetup<'_>,
    end: FacingEnd,
    x: f64,
    coverage: CoverageRectangle,
    tolerance: f64,
    context: &JobContext<'_>,
) -> Result<()> {
    let q = match end {
        FacingEnd::Start => -1.,
        FacingEnd::Far => 1.,
    };
    let [y0, z0] = coverage.min();
    let [y1, z1] = coverage.max();
    for tab in setup.tabs {
        context.check_cancelled()?;
        let axis = tab.axis().get();
        // The obstruction theorem uses exactly axial sections; an arbitrary
        // nearly axial solid needs its own bound, not an angular tolerance.
        let exactly_axial = axis == openchuck_types::geometry::UnitVec3::X.get()
            || axis == -openchuck_types::geometry::UnitVec3::X.get();
        if tab.as_cuboid().is_some() || !exactly_axial {
            continue;
        }
        let base = tab.base();
        let top = tab.top();
        let (lo, hi, r0, r1) = if base.x < top.x {
            (base.x, top.x, tab.base_radius(), tab.top_radius())
        } else {
            (top.x, base.x, tab.top_radius(), tab.base_radius())
        };
        if x < lo || x > hi {
            continue;
        }
        let slope = (r1 - r0) / (hi - lo);
        let growth = q * slope;
        if growth <= 0. {
            continue;
        }
        let remaining = if q > 0. { hi - x } else { x - lo };
        let diameter = setup.tool.diameter.0;
        let distance = diameter.min(remaining);
        let band = growth * distance;
        let near_radius = r0 + slope * (x - lo);
        if !band.is_finite() || !near_radius.is_finite() {
            return Err(refuse("tab access bounds overflowed"));
        }
        if band <= tolerance {
            continue;
        }
        let closest = (base.y.clamp(y0, y1), base.z.clamp(z0, z1));
        let radial = |(y, z): (f64, f64)| (y - base.y).hypot(z - base.z);
        let farthest = [(y0, z0), (y1, z0), (y1, z1), (y0, z1)]
            .into_iter()
            .max_by(|a, b| radial(*a).total_cmp(&radial(*b)));
        let Some(farthest) = farthest else {
            continue;
        };
        let lower = near_radius.max(radial(closest));
        let upper = (near_radius + band).min(radial(farthest));
        if upper - lower <= tolerance {
            continue;
        }
        let radius = lower + 0.5 * (upper - lower);
        let delta = (farthest.0 - closest.0, farthest.1 - closest.1);
        if !delta.0.is_finite() || !delta.1.is_finite() {
            return Err(refuse("tab access bounds overflowed"));
        }
        // Bounded search finds a POSITIVE witness inside the rectangle and
        // obstructed annulus. No absence of sampled collisions implies safety.
        let point = |t: f64| (closest.0 + t * delta.0, closest.1 + t * delta.1);
        let (mut a, mut b) = (0., 1.);
        for _ in 0..64 {
            let mid = 0.5 * (a + b);
            if radial(point(mid)) < radius {
                a = mid;
            } else {
                b = mid;
            }
        }
        let (y, z) = point(0.5 * (a + b));
        let requested = Point3::new(x, y, z);
        if setup
            .protected
            .iter()
            .any(|solid| solid.contains(requested))
        {
            continue;
        }
        let witness = Point3::new(x + q * distance * (1. - 1e-6), y, z);
        if tab.contains(witness) {
            return Err(refuse(&format!(
                "an outward-widening tab has an inaccessible {band:.3} mm radial band intersecting the requested face; this straight flank cannot finish it while preserving the tab"
            )));
        }
    }
    Ok(())
}

fn refuse(reason: &str) -> PathError {
    PathError::Strategy(format!("End facing: {reason}"))
}
