// SPDX-License-Identifier: AGPL-3.0-or-later
//! Facing: the stock cut down to a flat face at chosen A positions, or round
//! to a diameter, over its whole length, with the option to carry the passes
//! past its ends and sides (OP-24; decision record 0212 part B item 4).
//!
//! Facing is built on rotary roughing's layers and remaining-stock field
//! ([`crate::roughing`]): the layers run from the top of the stock down to
//! the face, a node is cut only where material stands above the tool, and
//! where the part with its stock to leave rises above the face the tool
//! follows it (OP-12). Unlike roughing it covers the whole stock rather than
//! the part's length, and it takes no selections.

use openchuck_types::job::JobContext;

use crate::error::Result;
use crate::params::{ParameterKind, ParameterSchema, ParameterSpec, Parameters, Unit};
use crate::roughing::parameter as roughing;
use crate::strategy::{Pass, Strategy, StrategyGroup, StrategyInput};

/// The registry name of [`Facing`].
pub const NAME: &str = "facing";

/// Parameter names specific to facing.
pub mod parameter {
    /// [`FLAT`] or [`ROUND`].
    pub const SHAPE: &str = "shape";
    /// Flat: the face's distance from the rotary axis; round: the diameter.
    pub const SIZE: &str = "size";
    /// Flat: the A positions to face at, degrees.
    pub const ANGLES: &str = "angles";
    /// A flat face at each listed A.
    pub const FLAT: &str = "flat";
    /// The stock turned down to a diameter.
    pub const ROUND: &str = "round";
}

/// Facing of the stock (OP-24).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Facing;

impl Strategy for Facing {
    fn name(&self) -> &str {
        NAME
    }

    fn label(&self) -> &str {
        "Facing"
    }

    fn description(&self) -> &str {
        "Cuts the stock down to a flat face, or round to a diameter, along its whole length, and can carry on past the ends of the stock so no thin skin is left at its edges. Use it to square up stock before roughing."
    }

    fn group(&self) -> StrategyGroup {
        StrategyGroup::Roughing
    }

    fn schema(&self) -> ParameterSchema {
        let own = crate::roughing::RotaryRoughing.schema();
        let borrowed = [
            roughing::STEPDOWN,
            roughing::MAX_ANGLE_STEP,
            roughing::START_ANGLE,
            roughing::MIN_CUT,
        ]
        .into_iter()
        .filter_map(|name| own.get(name).cloned());
        let mut parameters = vec![
            ParameterSpec {
                name: parameter::SHAPE.to_owned(),
                label: "Shape".to_owned(),
                description: "Flat: a flat face at each angle you list, the part held still. \
                              Round: the stock turned down to a cylinder as the part turns."
                    .to_owned(),
                kind: ParameterKind::Choice {
                    options: vec![parameter::FLAT.to_owned(), parameter::ROUND.to_owned()],
                    default: parameter::FLAT.to_owned(),
                },
            },
            ParameterSpec {
                name: parameter::SIZE.to_owned(),
                label: "Finished size".to_owned(),
                description: "Flat: how far the face is from the rotary axis. Round: the \
                              diameter to turn the stock down to."
                    .to_owned(),
                kind: ParameterKind::Number {
                    unit: Unit::Millimetres,
                    min: Some(0.0),
                    min_exclusive: true,
                    max: None,
                    default: None,
                },
            },
            ParameterSpec {
                name: parameter::ANGLES.to_owned(),
                label: "Angles".to_owned(),
                description: "Flat: the A positions to face at, in degrees, separated by \
                              commas, in the order to cut them."
                    .to_owned(),
                kind: ParameterKind::AngleList {
                    default: "0".to_owned(),
                },
            },
        ];
        parameters.extend(borrowed);
        parameters.extend(crate::extend::specs(true));
        ParameterSchema::with_common(parameters)
    }

    fn passes(
        &self,
        input: &StrategyInput<'_>,
        parameters: &Parameters,
        context: &JobContext<'_>,
    ) -> Result<Vec<Pass>> {
        let round = parameters.text(parameter::SHAPE)? == parameter::ROUND;
        let size = parameters.number(parameter::SIZE)?;
        let level = if round { 0.5 * size } else { size };
        crate::roughing::face(input, parameters, round, level, context)
    }
}
