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

pub mod coverage;
pub mod description;
pub mod end;
pub mod extents;
pub mod geometry;
pub mod motion;
pub mod plan;
pub mod selection;
mod validation;

use openchuck_types::job::JobContext;

use crate::error::Result;
use crate::params::{
    ParameterKind, ParameterSchema, ParameterSpec, ParameterValue, Parameters, Unit,
};
use crate::roughing::parameter as roughing;
use crate::strategy::{Pass, Strategy, StrategyGroup, StrategyInput};
use std::collections::BTreeMap;

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
    /// Face the end opposite the sole held end.
    pub const END: &str = "end";
    /// Selected indexed box sides and ordered arbitrary A faces.
    pub const BOX: &str = "box";
    /// Whole stock, part or part-and-tabs coverage.
    pub const EXTENT: &str = "extent";
    /// Nonnegative extent margin, separate from stock-edge extension.
    pub const MARGIN: &str = "extent_margin";
    /// Full steps with remainder, or equal depth layers, for new sides only.
    pub const LAYERS: &str = "layer_distribution";
    /// Ordered angle/offset rows, stored as validated JSON text.
    pub const ARBITRARY: &str = "arbitrary_faces";
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
        "Reduces the stock along its length to flat faces at chosen rotary angles, or to a round diameter. Use it to prepare stock before roughing. The part and workholding can limit the cut."
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
                              Round: the stock turned down to a cylinder as the part turns. \
                              End: the end opposite the sole held end. \
                              Box: selected standard sides and ordered arbitrary A faces."
                    .to_owned(),
                kind: ParameterKind::Choice {
                    options: vec![
                        parameter::FLAT.to_owned(),
                        parameter::ROUND.to_owned(),
                        parameter::END.to_owned(),
                        parameter::BOX.to_owned(),
                    ],
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
        parameters.extend(new_specs());
        parameters.extend(borrowed);
        parameters.extend(crate::extend::specs(true));
        ParameterSchema::with_common(parameters)
    }

    fn schema_for(&self, values: &BTreeMap<String, ParameterValue>) -> ParameterSchema {
        let shape = match values.get(parameter::SHAPE) {
            Some(ParameterValue::Text(s)) => s.as_str(),
            _ => parameter::FLAT,
        };
        let new = matches!(shape, parameter::END | parameter::BOX);
        let mut schema = self.schema();
        schema.parameters.retain(|spec| {
            let name = spec.name.as_str();
            if new
                && matches!(
                    name,
                    parameter::SIZE | parameter::ANGLES | roughing::MIN_CUT
                )
            {
                return false;
            }
            if shape == parameter::BOX
                && matches!(name, roughing::MAX_ANGLE_STEP | roughing::START_ANGLE)
            {
                return false;
            }
            if name == parameter::LAYERS || name == parameter::ARBITRARY {
                return shape == parameter::BOX;
            }
            for side in ["positive_z", "positive_y", "negative_z", "negative_y"] {
                if name == side {
                    return shape == parameter::BOX;
                }
                let selected = matches!(values.get(side), Some(ParameterValue::Flag(true)));
                if name == format!("{side}_from_part") {
                    return shape == parameter::BOX && selected;
                }
                if name == format!("{side}_offset") {
                    return shape == parameter::BOX
                        && selected
                        && matches!(
                            values.get(&format!("{side}_from_part")),
                            Some(ParameterValue::Flag(false))
                        );
                }
            }
            true
        });
        for spec in &mut schema.parameters {
            if spec.name.ends_with("_offset")
                && let ParameterKind::Number { default, .. } = &mut spec.kind
            {
                *default = None;
            }
        }
        if shape == parameter::END {
            for spec in &mut schema.parameters {
                if spec.name == roughing::STEPDOWN {
                    spec.label = "Axial stepdown".into();
                    spec.description="Maximum axial slice thickness, distributed equally to the exact end target; bounded by the tool's diameter and declared limits.".into();
                }
                if spec.name == crate::params::names::STEPOVER {
                    spec.label = "Flute band spacing".into();
                    spec.description="Maximum spacing along the flute direction between end-facing bands. Must not exceed the active flute length; axial slice depth is separate.".into();
                }
            }
        }
        if shape == parameter::ROUND {
            for spec in &mut schema.parameters {
                if spec.name == parameter::MARGIN {
                    spec.label = "Axial margin".into();
                }
            }
        }
        schema
    }

    fn passes(
        &self,
        input: &StrategyInput<'_>,
        parameters: &Parameters,
        context: &JobContext<'_>,
    ) -> Result<Vec<Pass>> {
        if matches!(
            parameters.text(parameter::SHAPE)?,
            parameter::END | parameter::BOX
        ) {
            let setup = validation::input(input, context)?;
            if parameters.text(crate::extend::parameter::EXTEND)? != crate::extend::parameter::NO
                || parameters.flag(crate::extend::parameter::EXTEND_SIDES)?
            {
                return Err(crate::error::PathError::Strategy("Stock-edge extension for this new Facing construction is not yet supported; no distance was reinterpreted. Disable extension or use legacy Flat/Round.".into()));
            }
            return if parameters.text(parameter::SHAPE)? == parameter::END {
                Ok(
                    end::plan_end(setup, &end::EndFacingParameters::read(parameters)?, context)?
                        .passes,
                )
            } else {
                Ok(plan::plan_box_sides(
                    setup,
                    &plan::BoxFacingParameters::read(parameters)?,
                    context,
                )?
                .passes)
            };
        }
        let round = parameters.text(parameter::SHAPE)? == parameter::ROUND;
        let size = parameters.number(parameter::SIZE)?;
        let level = if round { 0.5 * size } else { size };
        let scope = if restricted(parameters)? {
            let setup = input.facing.ok_or_else(|| {
                crate::error::PathError::Strategy(
                    "Facing requires whole-setup geometry for this mode".into(),
                )
            })?;
            if setup.projection_error.is_none() {
                return Err(crate::error::PathError::Strategy(
                    "Facing extent requires an established whole-source projection bound".into(),
                ));
            }
            let description = description::describe(setup, parameters)?;
            Some(match description.geometry {
                description::FacingGeometry::Flat { faces } => crate::roughing::FacingScope::Flat(
                    faces.into_iter().map(|f| f.coverage).collect(),
                ),
                description::FacingGeometry::Round { x_extent, .. } => {
                    crate::roughing::FacingScope::Round(x_extent)
                }
                _ => {
                    return Err(crate::error::PathError::Strategy(
                        "Legacy Facing coverage does not match the selected mode".into(),
                    ));
                }
            })
        } else {
            None
        };
        crate::roughing::face(input, parameters, round, level, scope.as_ref(), context)
    }

    fn validate_path(
        &self,
        input: &StrategyInput<'_>,
        parameters: &Parameters,
        path: &crate::GeneratedPath,
        context: &JobContext<'_>,
    ) -> Result<()> {
        if !needs_verification(parameters)? {
            return Ok(());
        }
        let setup = validation::input(input, context)?;
        self.validate_group(setup, parameters, &path.group, context)
    }
}

impl Facing {
    /// Recheck actual assembled or connected nominal motion using the SAME
    /// whole setup and resolved settings retained with generation. This is
    /// independent of candidate pass counts. It does not certify post words,
    /// controller rounding or a different tool/geometry supplied by a host.
    /// Legacy Whole-stock/zero-margin paths retain their existing checks.
    pub fn validate_group(
        &self,
        setup: &plan::FacingSetup<'_>,
        parameters: &Parameters,
        group: &openchuck_types::ir::ToolpathGroup,
        context: &JobContext<'_>,
    ) -> Result<()> {
        if !needs_verification(parameters)? {
            return Ok(());
        }
        validation::validate_setup(setup, context)?;
        crate::params::EntryParameters::read(parameters)?.check_tool(setup.tool.can_plunge())?;
        match parameters.text(parameter::SHAPE)? {
            parameter::END => validation::end(setup, parameters, group, context),
            parameter::BOX => validation::box_sides(setup, parameters, group, context),
            _ => validation::legacy(setup, parameters, group, context),
        }
    }
}

fn needs_verification(parameters: &Parameters) -> Result<bool> {
    Ok(matches!(
        parameters.text(parameter::SHAPE)?,
        parameter::END | parameter::BOX
    ) || restricted(parameters)?)
}

fn restricted(parameters: &Parameters) -> Result<bool> {
    Ok(parameters.text(parameter::EXTENT)? != "whole_stock"
        || parameters.number(parameter::MARGIN)? != 0.)
}

fn new_specs() -> Vec<ParameterSpec> {
    let choice = |name: &str, label: &str, description: &str, options: &[&str], default: &str| {
        ParameterSpec {
            name: name.into(),
            label: label.into(),
            description: description.into(),
            kind: ParameterKind::Choice {
                options: options.iter().map(|s| (*s).into()).collect(),
                default: default.into(),
            },
        }
    };
    let mut specs=vec![
        choice(parameter::EXTENT,"Coverage extent","Part is the entire placed part's projected bounding outline; Part plus tabs includes their bounds while preserving every tab.",&["whole_stock","part","part_plus_tabs"],"whole_stock"),
        ParameterSpec {name:parameter::MARGIN.into(),label:"Extent margin".into(),description:"Extra coverage beyond each projected edge; zero by default. Round uses an axial margin at both ends. Independent of stock-edge extension.".into(),kind:ParameterKind::Number {unit:Unit::Millimetres,min:Some(0.0),min_exclusive:false,max:None,default:Some(0.0)}},
        choice(parameter::LAYERS,"Layer distribution","Full stepdown uses the selected depth with a shorter final pass. Equal stepdowns distributes the depth equally without exceeding the selected maximum.",&["full","equal"],"full"),
        ParameterSpec {name:parameter::ARBITRARY.into(),label:"Arbitrary A faces".into(),description:"Additional indexed A angles and per-face offsets in user order, after selected standard faces. Move rows or explicitly sort by displayed A.".into(),kind:ParameterKind::IndexedFaces {default:"[]".into()}},
    ];
    for (side, label) in [
        ("positive_z", "+Z"),
        ("positive_y", "+Y"),
        ("negative_z", "−Z"),
        ("negative_y", "−Y"),
    ] {
        specs.push(ParameterSpec {
            name: side.into(),
            label: format!("Face {label}"),
            description: format!(
                "Select the box's {label} face; its machine A follows the configured rotary sense."
            ),
            kind: ParameterKind::Flag { default: false },
        });
        specs.push(ParameterSpec {name:format!("{side}_from_part"),label:format!("{label}: part plus allowance"),description:"Derive the target from the whole placed part plus existing stock to leave. Clear to enter an explicit offset.".into(),kind:ParameterKind::Flag {default:true}});
        specs.push(ParameterSpec {name:format!("{side}_offset"),label:format!("{label} plane offset"),description:"Target plane position from the rotary axis, with allowance already included. It is not the cutter-centre position.".into(),kind:ParameterKind::Number {unit:Unit::Millimetres,min:None,min_exclusive:false,max:None,default:Some(0.0)}});
    }
    specs
}
