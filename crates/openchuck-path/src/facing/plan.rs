// SPDX-License-Identifier: AGPL-3.0-or-later
//! Facing-local pass planning from explicit whole setup geometry (0252).
//! Returned passes are candidates BEFORE shared assembly. They must not be
//! published as completed machining until actual full-tool motion, retained
//! stock, protected solids and achieved coverage are independently validated.

use openchuck_types::geometry::Point3;
use openchuck_types::job::JobContext;
use openchuck_types::machine::MachineProfile;
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::stock::Stock;
use openchuck_types::tool::{Tool, ToolShape};
use std::ops::Range;

use super::extents::FacingExtent;
use super::geometry::{CoverageRectangle, LayerDistribution};
use super::selection::{ResolvedSide, SideSelection};
use crate::error::{PathError, Result};
use crate::rotary::RotaryMapping;
use crate::solid::Solid;
use crate::strategy::{CutPoint, Pass};

/// Whole placed setup data, separate from the composite machining target.
/// Hosts must not substitute selected surfaces or warn-only geometry here.
#[derive(Debug)]
pub struct FacingSetup<'a> {
    /// Whole placed source mesh, with all referenced facets.
    pub part: &'a TriangleMesh,
    /// Explicit initial stock definition, not the chosen facing extent.
    pub stock: &'a Stock,
    /// Actual configured cutting/flute/shank dimensions.
    pub tool: &'a Tool,
    /// Actual machine frame, sense and limits.
    pub machine: &'a MachineProfile,
    /// Analytic retaining tabs only, for Part plus tabs coverage.
    pub tabs: &'a [Solid],
    /// All analytic held/tab protections; never changed by an extent choice.
    pub protected: &'a [Solid],
    /// Actual gripped model-X intervals at start and far end, before tool growth.
    pub held: [Option<(f64, f64)>; 2],
    /// Conservative source projection error; exact STL facets use Some(0).
    /// None means no bound, not zero error. Exact STEP hosts supply a proved bound.
    pub projection_error: Option<f64>,
}

/// Choices validated by new box-side planning, independent of legacy modes.
#[derive(Debug, Clone, PartialEq)]
pub struct BoxFacingParameters {
    /// Canonical selections and arbitrary faces in user order.
    pub selection: SideSelection,
    /// Explicit requested coverage geometry.
    pub extent: FacingExtent,
    /// Planar extent margin, millimetres.
    pub margin: f64,
    /// Full increments/remainder or equal increments.
    pub layers: LayerDistribution,
    /// Maximum inward layer depth, millimetres.
    pub stepdown: f64,
    /// Maximum raster spacing, millimetres, no larger than flat cutting diameter.
    pub stepover: f64,
    /// Existing operation stock allowance, applied once.
    pub allowance: f64,
    /// Operation's total geometric tolerance, millimetres.
    pub tolerance: f64,
}

impl BoxFacingParameters {
    /// Translate active Box settings without adding an allowance to overrides.
    pub fn read(parameters: &crate::params::Parameters) -> Result<Self> {
        use super::parameter;
        use crate::params::names;
        if parameters.text(parameter::SHAPE)? != parameter::BOX {
            return Err(refuse("expected Box sides settings"));
        }
        let layers = match parameters.text(parameter::LAYERS)? {
            "full" => LayerDistribution::Full,
            "equal" => LayerDistribution::Equal,
            _ => return Err(refuse("unknown layer distribution")),
        };
        Ok(Self {
            selection: SideSelection::read(parameters)?,
            extent: FacingExtent::from_name(parameters.text(parameter::EXTENT)?)?,
            margin: parameters.number(parameter::MARGIN)?,
            layers,
            stepdown: parameters.number(crate::roughing::parameter::STEPDOWN)?,
            stepover: parameters.number(names::STEPOVER)?,
            allowance: parameters.number(names::STOCK_TO_LEAVE)?,
            tolerance: parameters.number(names::TOLERANCE)?,
        })
    }
}

/// One requested side and its nominal layer/pass description for previews.
#[derive(Debug, Clone, PartialEq)]
pub struct PlannedSide {
    /// Resolved plane target and shared coordinate convention.
    pub target: ResolvedSide,
    /// Requested rectangle; full-tool overlap is separate.
    pub coverage: CoverageRectangle,
    /// Cutting layer offsets. An established source-error bound can reserve
    /// an outward fraction of tolerance; it never changes the nominal target.
    pub levels: Vec<f64>,
    /// This face's candidate passes in the enclosing plan.
    pub passes: Range<usize>,
}

/// Candidate cut passes and shared description. This is not a GeneratedPath
/// or a safety/completion certificate; assembly and validation remain required.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FacingPassPlan {
    /// Requested faces in their effective cutting order.
    pub faces: Vec<PlannedSide>,
    /// Candidate points, preserving fixed A on each side.
    pub passes: Vec<Pass>,
}

/// Build bounded, cancellable box-side rasters. The host must subsequently
/// assemble entries/links, check every complete-tool sweep and certify actual
/// completion before producing exportable output. Protections are not clipped
/// away here; a planner success is intentionally not face-completion success.
pub fn plan_box_sides(
    setup: &FacingSetup<'_>,
    parameters: &BoxFacingParameters,
    context: &JobContext<'_>,
) -> Result<FacingPassPlan> {
    context.check_cancelled()?;
    setup.part.validate()?;
    setup.stock.validate()?;
    setup.machine.validate()?;
    setup.tool.validate()?;
    if !matches!(
        setup.stock,
        Stock::Box { .. } | Stock::OffsetBoundingBox { .. }
    ) {
        return Err(refuse("Box sides requires box/block stock"));
    }
    if !matches!(setup.tool.shape, ToolShape::FlatEnd) {
        return Err(refuse(
            "this box-side construction requires a flat end mill",
        ));
    }
    if setup.tool.shank_diameter.0 > setup.tool.diameter.0 {
        return Err(refuse(
            "the larger shank has no supported complete-tool bound for this construction",
        ));
    }
    if !parameters.tolerance.is_finite() || parameters.tolerance <= 0.0 {
        return Err(refuse("tolerance must be finite and above zero"));
    }
    if !parameters.allowance.is_finite() || parameters.allowance < 0.0 {
        return Err(refuse("stock to leave must be finite and nonnegative"));
    }
    let source_error = setup
        .projection_error
        .filter(|e| e.is_finite() && *e >= 0.0)
        .ok_or_else(|| {
            refuse("the whole placed source has no established projection-error bound")
        })?;
    if source_error > 0.25 * parameters.tolerance {
        return Err(refuse(
            "the source projection bound exceeds its geometric error budget",
        ));
    }
    if !parameters.stepdown.is_finite()
        || parameters.stepdown <= 0.0
        || parameters.stepdown > setup.tool.flute_length.0
    {
        return Err(refuse(
            "stepdown must be above zero and no greater than flute length",
        ));
    }
    if !parameters.stepover.is_finite()
        || parameters.stepover <= 0.0
        || parameters.stepover > setup.tool.diameter.0
    {
        return Err(refuse(
            "stepover must be above zero and no greater than flat cutting diameter",
        ));
    }
    for (lo, hi) in setup.held.iter().flatten() {
        if !lo.is_finite() || !hi.is_finite() || hi <= lo {
            return Err(refuse("held intervals must be finite with positive length"));
        }
    }
    // Iterate referenced facets only: a spare vertex is not part geometry.
    let vertices: Vec<Point3> = setup
        .part
        .triangles
        .iter()
        .flatten()
        .filter_map(|&i| {
            usize::try_from(i)
                .ok()
                .and_then(|i| setup.part.vertices.get(i))
                .copied()
        })
        .collect();
    let rotary = RotaryMapping::of_profile(setup.machine);
    let targets = parameters
        .selection
        .resolve(rotary, &vertices, parameters.allowance)?;
    let mut plan = FacingPassPlan::default();
    for target in targets {
        context.check_cancelled()?;
        let coverage = parameters.extent.planar(
            target.plane,
            setup.stock,
            setup.part,
            setup.tabs,
            parameters.margin,
        )?;
        let outer = FacingExtent::WholeStock.support(target.plane, setup.stock, setup.part, &[])?;
        let cutting_offset = target.offset + source_error;
        if !cutting_offset.is_finite() || cutting_offset > outer {
            return Err(refuse("the target plane lies outside supplied stock"));
        }
        let levels = parameters
            .layers
            .levels(outer, cutting_offset, parameters.stepdown)?;
        let [x0, y0] = coverage.min();
        let [x1, y1] = coverage.max();
        if x1 <= x0 || y1 <= y0 {
            return Err(refuse("the requested side has no positive-area extent"));
        }
        // Across-going lines allow the eventual protected-end validator to
        // assess axial edge footprints independently of full-length passes.
        let rows = ((x1 - x0) / parameters.stepover).ceil().max(1.0);
        if !rows.is_finite() || rows > 100_000.0 {
            return Err(refuse("raster row budget exceeded"));
        }
        let rows = rows as usize;
        let count = (rows + 1)
            .checked_mul(levels.len())
            .and_then(|n| n.checked_add(plan.passes.len()))
            .filter(|n| *n <= 1_000_000)
            .ok_or_else(|| refuse("facing pass budget exceeded"))?;
        plan.passes.reserve(count - plan.passes.len());
        let start = plan.passes.len();
        for &level in &levels {
            for row in 0..=rows {
                context.check_cancelled()?;
                let x = if row == rows {
                    x1
                } else {
                    x0 + (x1 - x0) * row as f64 / rows as f64
                };
                let points = [
                    target.plane.point(x, y0, level)?,
                    target.plane.point(x, y1, level)?,
                ]
                .map(|tip| CutPoint {
                    tip,
                    a: target.angle,
                    contact: tip,
                })
                .to_vec();
                plan.passes.push(Pass {
                    helical_pass_start: false,
                    low_link: false,
                    points,
                    stock: None,
                    rapids: Vec::new(),
                });
            }
        }
        plan.faces.push(PlannedSide {
            target,
            coverage,
            levels,
            passes: start..plan.passes.len(),
        });
    }
    Ok(plan)
}

fn refuse(reason: &str) -> PathError {
    PathError::Strategy(format!("Box facing: {reason}"))
}
