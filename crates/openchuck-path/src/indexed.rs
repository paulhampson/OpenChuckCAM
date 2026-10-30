// SPDX-License-Identifier: AGPL-3.0-or-later
//! Indexed finishing: passes along the rotary axis at fixed A positions, the
//! 3+1 positional form (OP-04; work breakdown slice 12 item 3; decision record
//! 0034 item 3).
//!
//! One pass along X is cut at each requested A, in the order given. The angle
//! goes to the IR as given (the shared services only add whole turns to keep A
//! continuous), and the move from one angle to the next is a positioning move
//! through the clearance geometry, never a feed move across the stock
//! (OP-14). The angles are a list (`angles`), or `count` equally spaced
//! positions from `start_angle` when the list is empty.

use openchuck_types::job::JobContext;

use crate::axial::{AlongX, parameter as axial_parameter};
use crate::error::{PathError, Result};
use crate::params::{
    MAX_ANGLES, ParameterKind, ParameterSchema, ParameterSpec, Parameters, Unit, names,
};
use crate::region::RegionConstraint;
use crate::strategy::{LeaveZone, Pass, Strategy, StrategyInput, StrategyOutput};

/// Indexed finishing of the whole model, or of a region.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct IndexedFinishing;

/// The registry name of [`IndexedFinishing`].
pub const NAME: &str = "indexed_finishing";

/// Parameter names specific to indexed finishing.
pub mod parameter {
    /// The A positions, degrees, separated by commas.
    pub const ANGLES: &str = "angles";
    /// How many equally spaced positions, when `angles` is empty.
    pub const COUNT: &str = "count";
    /// A of the first equally spaced position, degrees.
    pub const START_ANGLE: &str = "start_angle";
}

impl Strategy for IndexedFinishing {
    /// Exact contacts on a STEP part: measured within the limits of OI-835
    /// (decision 0277).
    fn exact_contact(&self) -> bool {
        true
    }

    fn name(&self) -> &str {
        NAME
    }

    fn label(&self) -> &str {
        "Indexed finishing"
    }

    fn description(&self) -> &str {
        "Turns the part to each chosen angle, holds it still and follows the surface along its length. Use it for features reached from a few set directions. It does not turn the part during a cutting pass."
    }

    fn schema(&self) -> ParameterSchema {
        let mut schema = ParameterSchema::with_common(vec![
            ParameterSpec {
                name: parameter::ANGLES.to_owned(),
                label: "Angles".to_owned(),
                description: "The A positions to cut at, in degrees, in the order to cut \
                              them, separated by commas. Empty to use the count instead."
                    .to_owned(),
                kind: ParameterKind::AngleList {
                    default: String::new(),
                },
            },
            ParameterSpec {
                name: parameter::COUNT.to_owned(),
                label: "Count".to_owned(),
                description: "How many equally spaced positions to cut at when no angles \
                              are listed."
                    .to_owned(),
                kind: ParameterKind::Integer {
                    min: 1,
                    max: MAX_ANGLES as i64,
                    default: Some(4),
                },
            },
            ParameterSpec {
                name: parameter::START_ANGLE.to_owned(),
                label: "Start angle".to_owned(),
                description: "A of the first equally spaced position.".to_owned(),
                kind: ParameterKind::Number {
                    unit: Unit::Degrees,
                    min: None,
                    min_exclusive: false,
                    max: None,
                    default: Some(0.0),
                },
            },
        ]);
        // The positions, not a stepover, set where the passes are; the common
        // stepover is not used, so it gets a default.
        if let Some(spec) = schema
            .parameters
            .iter_mut()
            .find(|p| p.name == names::STEPOVER)
        {
            spec.description = "Not used: the angles set where the passes are.".to_owned();
            if let ParameterKind::Number { default, .. } = &mut spec.kind {
                *default = Some(1.0);
            }
        }
        let own = super::axial::AxialFinishing.schema();
        for name in [
            axial_parameter::DIRECTION,
            axial_parameter::MAX_X_STEP,
            crate::extend::parameter::EXTEND,
            crate::extend::parameter::EXTEND_DISTANCE,
            crate::finishing::parameter::FINISH_ALLOWANCE,
            crate::finishing::parameter::FINISHING_LAYERS,
            crate::finishing::parameter::STEPDOWN,
        ] {
            if let Some(spec) = own.get(name) {
                schema.parameters.push(spec.clone());
            }
        }
        schema
    }

    fn schema_for(
        &self,
        values: &std::collections::BTreeMap<String, crate::params::ParameterValue>,
    ) -> ParameterSchema {
        crate::finishing::schema_for(self.schema(), values)
    }

    fn honours_region(&self) -> bool {
        true
    }

    fn honours_leave_zones(&self) -> bool {
        true
    }

    fn passes_in_zones(
        &self,
        input: &StrategyInput<'_>,
        region: Option<&RegionConstraint<'_>>,
        zones: &[LeaveZone<'_>],
        parameters: &Parameters,
        context: &JobContext<'_>,
    ) -> Result<Vec<Pass>> {
        passes(input, region, zones, parameters, context).map(|(p, ..)| p)
    }

    fn passes(
        &self,
        input: &StrategyInput<'_>,
        parameters: &Parameters,
        context: &JobContext<'_>,
    ) -> Result<Vec<Pass>> {
        passes(input, None, &[], parameters, context).map(|(p, ..)| p)
    }

    fn passes_in_region(
        &self,
        input: &StrategyInput<'_>,
        region: &RegionConstraint<'_>,
        parameters: &Parameters,
        context: &JobContext<'_>,
    ) -> Result<Vec<Pass>> {
        passes(input, Some(region), &[], parameters, context).map(|(p, ..)| p)
    }

    fn passes_with_notes(
        &self,
        input: &StrategyInput<'_>,
        region: Option<&RegionConstraint<'_>>,
        parameters: &Parameters,
        context: &JobContext<'_>,
    ) -> Result<StrategyOutput> {
        let (passes, (x0, x1), layered) = passes(input, region, &[], parameters, context)?;
        Ok(StrategyOutput {
            notes: beyond_note(&passes, x0, x1)
                .into_iter()
                .chain(layered)
                .collect(),
            passes,
            warnings: Vec::new(),
        })
    }
}

/// The note an indexed operation carries when its passes cut past the ends
/// of the part (`x0` to `x1`), where the tool's side faces an end face the
/// target stops short of (decision record 0343). `None` when they did not.
fn beyond_note(passes: &[Pass], x0: f64, x1: f64) -> Option<String> {
    let points = || passes.iter().flat_map(|p| p.points.iter());
    let start = points().any(|c| c.tip.x < x0);
    let end = points().any(|c| c.tip.x > x1);
    let ends = match (start, end) {
        (true, true) => "past both ends of the part",
        (true, false) => "past the start of the part",
        (false, true) => "past the far end of the part",
        (false, false) => return None,
    };
    Some(format!(
        "the passes run on {ends} by up to the tool's radius, so its side faces each end face \
         where the target stops short of it, along each position only: between the positions \
         the end face keeps what earlier operations left"
    ))
}

/// The positions for `parameters`, in the order to cut.
fn positions(parameters: &Parameters) -> Result<Vec<f64>> {
    let listed = parameters.angles(parameter::ANGLES)?;
    if !listed.is_empty() {
        return Ok(listed);
    }
    let count = parameters.integer(parameter::COUNT)?;
    let start = parameters.number(parameter::START_ANGLE)?;
    if count < 1 {
        return Err(PathError::parameter(
            parameter::COUNT,
            "at least one position",
        ));
    }
    let step = 360.0 / count as f64;
    Ok((0..count).map(|i| start + step * i as f64).collect())
}

fn passes(
    input: &StrategyInput<'_>,
    region: Option<&RegionConstraint<'_>>,
    zones: &[LeaveZone<'_>],
    parameters: &Parameters,
    context: &JobContext<'_>,
) -> Result<(Vec<Pass>, (f64, f64), Option<String>)> {
    let angles = positions(parameters)?;
    let mut along = AlongX::read(input, region, zones, parameters)?;
    // Each pass runs on past the ends of the target by the tool's radius
    // (grown by the stock to leave), less a tenth of the tolerance so its
    // first point still stands on the target, so where the target stands
    // short of the tool's radius past an end face (a short tab) the tool goes
    // down beside the face onto it and the fit finds the face between that
    // point and the next. Past a free end the tool only rides the end's edge
    // (decision record 0343). A stays fixed at each position.
    let tolerance = parameters.number(names::TOLERANCE)?;
    along.beyond = along.grown_radius() - 0.1 * tolerance;
    along.check_passes(angles.len())?;
    let passes = along.passes_at(&angles, "indexed finishing", context)?;
    // Finishing layers above them, when on; each layer keeps the pass's A
    // (decision record 0345).
    let (passes, layered) = crate::finishing::layers(input, passes, parameters)?;
    Ok((passes, along.range(), layered))
}
