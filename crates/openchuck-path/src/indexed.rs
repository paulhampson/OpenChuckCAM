// SPDX-License-Identifier: AGPL-3.0-or-later
//! Indexed finishing: passes along the rotary axis at fixed A positions, the
//! 3+1 positional form (OP-04; work breakdown slice 12 item 3; decision record
//! 0033 item 3).
//!
//! One pass along X is cut at each requested A, in the order given. The angle
//! goes to the IR as given (the shared services only add whole turns to keep A
//! continuous), and the move from one angle to the next is a positioning move
//! through the clearance geometry, never a feed move across the stock
//! (OP-14). The angles are a list (`angles`), or `count` equally spaced
//! positions from `start_angle` when the list is empty.

use openchuck_types::job::JobContext;
use openchuck_types::units::Degrees;

use crate::axial::{AlongX, parameter as axial_parameter};
use crate::error::{PathError, Result};
use crate::params::{
    MAX_ANGLES, ParameterKind, ParameterSchema, ParameterSpec, Parameters, Unit, names,
};
use crate::region::RegionConstraint;
use crate::strategy::{LeaveZone, Pass, Strategy, StrategyInput};

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
    fn name(&self) -> &str {
        NAME
    }

    fn label(&self) -> &str {
        "Indexed finishing"
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
        for name in [axial_parameter::DIRECTION, axial_parameter::MAX_X_STEP] {
            if let Some(spec) = own.get(name) {
                schema.parameters.push(spec.clone());
            }
        }
        schema
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
        passes(input, region, zones, parameters, context)
    }

    fn passes(
        &self,
        input: &StrategyInput<'_>,
        parameters: &Parameters,
        context: &JobContext<'_>,
    ) -> Result<Vec<Pass>> {
        passes(input, None, &[], parameters, context)
    }

    fn passes_in_region(
        &self,
        input: &StrategyInput<'_>,
        region: &RegionConstraint<'_>,
        parameters: &Parameters,
        context: &JobContext<'_>,
    ) -> Result<Vec<Pass>> {
        passes(input, Some(region), &[], parameters, context)
    }
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
) -> Result<Vec<Pass>> {
    let angles = positions(parameters)?;
    let along = AlongX::read(input, region, zones, parameters)?;
    along.check_passes(angles.len())?;
    let mut out = Vec::new();
    for (i, a) in angles.iter().enumerate() {
        context.check_cancelled()?;
        out.extend(along.pass(Degrees(*a), along.zigzag && i % 2 == 1, context)?);
    }
    Ok(out)
}
