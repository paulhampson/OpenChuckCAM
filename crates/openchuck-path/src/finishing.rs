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

use crate::params::{ParameterKind, ParameterSchema, ParameterSpec, ParameterValue, Unit};

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
