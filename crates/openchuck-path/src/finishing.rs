// SPDX-License-Identifier: AGPL-3.0-or-later
//! Settings the finishing operations share for their layers and their free
//! ends (decision records 0344 and 0345; OP-04, OP-09, TL-05, NFR-12).
//!
//! **Stepdown** is the depth of each layer along the tool axis. It has the
//! same name as roughing's, so the tool's cutting data for the operation's
//! material fills it when the operation sets none (TL-05, decision record
//! 0230). Nothing else gives it a value: there is no guessed depth of cut
//! (NFR-12). With **Finishing layers** on it is required; with them off it
//! is used only to face a free end, and zero means none was given.
//!
//! **Face free ends** turns at an end face of the part with nothing beyond
//! it to stand the tool on. It is on for a new operation; an operation saved
//! before it is migrated to off (project schema 23), so it machines as
//! before.

use std::collections::BTreeMap;

use openchuck_types::geometry::{Point3, UnitVec3, Vec3};
use openchuck_types::tool::profile::RevolvedProfile;
use openchuck_types::units::Degrees;

use crate::assemble::PathWarning;
use crate::error::{PathError, Result};
use crate::faces::FreeEnd;
use crate::params::{
    ParameterKind, ParameterSchema, ParameterSpec, ParameterValue, Parameters, Unit,
};
use crate::rotary::AxisPosition;
use crate::strategy::{CutPoint, Pass, StrategyInput};

/// Parameter names.
pub mod parameter {
    /// Turn at the part's free ends: on or off.
    pub const FACE_FREE_ENDS: &str = "face_free_ends";
    /// Cut in layers along the tool axis down to the target: on or off.
    pub const FINISHING_LAYERS: &str = "finishing_layers";
    /// The depth of each layer along the tool axis, mm; the name roughing
    /// uses, so the tool's cutting data fills it.
    pub use crate::roughing::parameter::STEPDOWN;
}

/// The Stepdown when it is required: a positive number with no default.
fn required_stepdown() -> ParameterKind {
    ParameterKind::Number {
        unit: Unit::Millimetres,
        min: Some(0.0),
        min_exclusive: true,
        max: None,
        default: None,
    }
}

/// The settings for a finishing strategy's schema: Face free ends when
/// `free_ends`, Finishing layers when `layers`, and Stepdown when either.
pub(crate) fn specs(free_ends: bool, layers: bool) -> Vec<ParameterSpec> {
    let mut out = Vec::new();
    if free_ends {
        out.push(ParameterSpec {
            name: parameter::FACE_FREE_ENDS.to_owned(),
            label: "Face free ends".to_owned(),
            description: "Turn at each end face of the part that has nothing beyond it, \
                          stepping down beside the face by the Stepdown, so the face keeps \
                          only the stock to leave."
                .to_owned(),
            kind: ParameterKind::Flag { default: true },
        });
    }
    if layers {
        out.push(ParameterSpec {
            name: parameter::FINISHING_LAYERS.to_owned(),
            label: "Finishing layers".to_owned(),
            description: "Cut in layers the Stepdown apart along the tool axis, the last on \
                          the finished surface with its stock to leave."
                .to_owned(),
            kind: ParameterKind::Flag { default: false },
        });
    }
    if free_ends || layers {
        out.push(ParameterSpec {
            name: parameter::STEPDOWN.to_owned(),
            label: "Stepdown".to_owned(),
            description: "The depth of each layer along the tool axis. Taken from the tool's \
                          cutting data when not set here. Required with finishing layers on; \
                          otherwise used only to face a free end."
                .to_owned(),
            kind: ParameterKind::Number {
                unit: Unit::Millimetres,
                min: Some(0.0),
                min_exclusive: false,
                max: None,
                default: Some(0.0),
            },
        });
    }
    out
}

/// `schema` as it applies to `values`: with Finishing layers on, the
/// Stepdown is required (decision record 0344, C1).
pub(crate) fn schema_for(
    mut schema: ParameterSchema,
    values: &BTreeMap<String, ParameterValue>,
) -> ParameterSchema {
    let layers = matches!(
        values.get(parameter::FINISHING_LAYERS),
        Some(ParameterValue::Flag(true))
    );
    if layers
        && let Some(spec) = schema
            .parameters
            .iter_mut()
            .find(|p| p.name == parameter::STEPDOWN)
    {
        spec.kind = required_stepdown();
    }
    schema
}

/// The angles round the axis the stock is read at for a free end.
const AROUND: usize = 72;

/// What facing the free ends gives an operation: the turns, its notes and
/// its warnings.
#[derive(Debug, Default)]
pub(crate) struct FreeEnds {
    pub(crate) passes: Vec<Pass>,
    pub(crate) notes: Vec<String>,
    pub(crate) warnings: Vec<PathWarning>,
}

/// The turns at the part's free ends `ends` (decision record 0344): with
/// Face free ends on, a Stepdown and stock that earlier operations left,
/// the tool (`grown` by the stock to leave) goes down beside each free end
/// face in layers of the Stepdown to the rotary axis, one whole turn of A at
/// constant X each, starting at A `start` turning by `sign`, no more than
/// `step` degrees between points. Without a Stepdown, or on stock no earlier
/// operation left, the face is not faced and a warning says why (C1, C2 and
/// OI-1394). A face deeper than the flutes refuses the operation (C3). Where
/// the stock carries on past the ring the tool cuts, a warning says the
/// stock there can be cut free (the owner's choice on OI-1390).
pub(crate) fn free_ends(
    input: &StrategyInput<'_>,
    grown: &RevolvedProfile,
    ends: &[FreeEnd],
    parameters: &Parameters,
    (start, sign, step): (f64, f64, f64),
) -> Result<FreeEnds> {
    let mut out = FreeEnds::default();
    if !parameters.flag(parameter::FACE_FREE_ENDS).unwrap_or(false) {
        return Ok(out);
    }
    let stock = input.stock;
    // Only an end the stock carries on past has anything to face: the end
    // of the stock itself is not a free end.
    let extent = stock.x_extent();
    let tolerance = 1e-3;
    let ends: Vec<&FreeEnd> = ends
        .iter()
        .filter(|e| match extent {
            Some((_, hi)) if e.far => hi > e.face + tolerance,
            Some((lo, _)) => lo < e.face - tolerance,
            None => true,
        })
        .collect();
    let stepdown = parameters.number(parameter::STEPDOWN).unwrap_or(0.0);
    let radius = grown.cutting_radius();
    let width = 2.0 * input.tool.cutting_radius();
    for end in ends {
        let face = end.face;
        // Zero (none given) or NaN: no Stepdown.
        if stepdown.is_nan() || stepdown <= 0.0 {
            out.warnings.push(PathWarning {
                message: format!(
                    "the end face at X {face:.3} has nothing beyond it and is not faced: it \
                     keeps what earlier operations left. Stepdown: Required. Enter the depth \
                     of each layer, or add cutting data for this tool and material in the \
                     tool library."
                ),
                parameter: Some(parameter::STEPDOWN.to_owned()),
            });
            continue;
        }
        if !stock.left_by_earlier_operations() {
            out.warnings.push(PathWarning {
                message: format!(
                    "the end face at X {face:.3} has nothing beyond it and is not faced: no \
                     earlier operation in this setup shows what stock is past it, so the \
                     tool would cut there blind. Add the roughing first, or turn off Face \
                     free ends."
                ),
                parameter: Some(parameter::FACE_FREE_ENDS.to_owned()),
            });
            continue;
        }
        // The highest material the turn meets: the stock read from outside
        // along the tool axis at each angle, across the ring.
        let reach = stock.max_radius().unwrap_or(0.0) + 1.0;
        let top = (0..AROUND)
            .flat_map(|k| {
                let a = Degrees(360.0 * k as f64 / AROUND as f64);
                [-0.9, 0.0, 0.9].map(move |u| (a, end.x + u * radius))
            })
            .filter_map(|(a, x)| {
                let at = |h: f64| {
                    input.rotary.to_model(
                        AxisPosition {
                            along: x,
                            across: 0.0,
                            height: h,
                        },
                        a,
                    )
                };
                let down = UnitVec3::new(at(0.0) - at(1.0)).ok()?;
                let d = stock.ray_entry(at(reach), down, reach)?;
                Some(reach - d)
            })
            .fold(f64::NEG_INFINITY, f64::max);
        if !top.is_finite() || top <= 0.0 {
            continue;
        }
        if top > input.tool.top() {
            return Err(PathError::Strategy(format!(
                "the end face at X {face:.3} has stock {top:.3} mm deep beside it, more than \
                 the tool's {:.3} mm of flute; use a tool with longer flutes, turn off Face \
                 free ends, or face it with Facing",
                input.tool.top()
            )));
        }
        // Layers at whole Stepdowns above the axis, the first the highest
        // below the material's top, the last on the axis.
        let layers = ((top / stepdown).ceil() as usize).max(1);
        let points = ((360.0 / step).ceil() as usize).max(4);
        let toward = if end.far { -1.0 } else { 1.0 };
        let side = input.tool.cutting_radius();
        for m in (0..layers).rev() {
            let h = m as f64 * stepdown;
            let pass: Vec<CutPoint> = (0..=points)
                .map(|i| {
                    let a = Degrees(start + sign * 360.0 * i as f64 / points as f64);
                    let place = |along: f64| {
                        input.rotary.to_model(
                            AxisPosition {
                                along,
                                across: 0.0,
                                height: h,
                            },
                            a,
                        )
                    };
                    CutPoint {
                        tip: place(end.x),
                        a,
                        contact: place(end.x + toward * side),
                    }
                })
                .collect();
            out.passes.push(Pass {
                helical_pass_start: false,
                points: pass,
                stock: None,
                rapids: Vec::new(),
            });
        }
        let (from, to) = (end.x - radius, end.x + radius);
        out.notes.push(format!(
            "the end face at X {face:.3} has nothing beyond it: the tool goes down beside it \
             in {layers} layers of {stepdown} mm to the rotary axis, turning once round the \
             axis in each"
        ));
        // Stock past the ring that no held region past it grips: a cheap test
        // of whether there is any, not of whether it is joined to the rest
        // (OI-1393).
        let gripped = input
            .keep_out
            .iter()
            .any(|&(a, b)| if end.far { a > end.x } else { b < end.x });
        if !gripped && let Some((lo, hi)) = extent {
            let beyond = if end.far {
                (end.x + radius + 0.1, hi - 1e-3)
            } else {
                (lo + 1e-3, end.x - radius - 0.1)
            };
            let found = beyond.1 > beyond.0
                && (0..=4).any(|i| {
                    let x = beyond.0 + (beyond.1 - beyond.0) * i as f64 / 4.0;
                    (0..16).any(|k| {
                        let t = std::f64::consts::TAU * k as f64 / 16.0;
                        [0.25, 0.5, 0.75].into_iter().any(|f| {
                            let r = f * top;
                            stock.contains(Point3::ORIGIN + Vec3::new(x, r * t.cos(), r * t.sin()))
                        })
                    })
                });
            if found {
                out.warnings.push(PathWarning {
                    message: format!(
                        "the end face at X {face:.3} is faced down to the rotary axis: the tool \
                         cuts a ring {width:.3} mm wide from X {:.3} to {:.3} down to the axis, \
                         and the stock past the ring is not held by anything in this setup, so \
                         it can be cut free there and thrown while the rotary axis turns. \
                         Before running this program, check the stock and the workholding on \
                         the machine",
                        from.min(to),
                        from.max(to)
                    ),
                    parameter: Some(parameter::FACE_FREE_ENDS.to_owned()),
                });
            }
        }
    }
    Ok(out)
}
