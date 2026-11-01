// SPDX-License-Identifier: AGPL-3.0-or-later
//! The per-operation Exact STEP surface contact (decision record 0500; issue
//! #351; IMP-02, OP-12): the user's choice, for an operation whose strategy
//! drops the tool on the part, between the import's tessellation and the
//! exact faces of a STEP part, trading generation time for accuracy.
//!
//! A strategy that offers it names its kind ([`Strategy::exact_contact`]) and
//! adds the setting to its schema next to the tolerance ([`with_switch`]).
//! The host reads it ([`exact_drops`]) and builds the operation's surfaces
//! with [`BrepSurface::with_exact_drops`](crate::BrepSurface::with_exact_drops);
//! the strategy itself does not read it. The setting has no effect on a mesh
//! part.
//!
//! - [`ExactContact::Ball`]: the exact ball drop of decision 0277, measured
//!   within both generation limits of OI-835 for helical, indexed, rest and
//!   simultaneous finishing, so on by default.
//! - [`ExactContact::Outline`]: every tool with an exact drop, the outline
//!   drop of decision 0338 included, measured far over those limits for axial
//!   finishing and pencil clean-up (OI-1351), so off by default. Turning it on
//!   does not declare the limits met: OI-835 still governs any default.

use std::collections::BTreeMap;

use openchuck_types::tool::profile::RevolvedProfile;

use crate::params::{ParameterKind, ParameterSchema, ParameterSpec, ParameterValue, names};
use crate::strategy::Strategy;

/// The parameter's stored name.
pub mod parameter {
    /// Drop the tool on a STEP part's exact faces: on or off.
    pub const EXACT_CONTACT: &str = "exact_contact";
}

/// The strategies whose setting is of kind [`ExactContact::Ball`], on by
/// default: their operations saved before the setting are migrated to it on
/// (project schema 27), as they already machined.
pub const BALL_STRATEGIES: [&str; 4] = [
    "helical_finishing",
    "indexed_finishing",
    "rest_machining",
    "simultaneous_finishing",
];

/// The strategies whose setting is of kind [`ExactContact::Outline`], off by
/// default: their operations saved before the setting are migrated to it off
/// (project schema 27), as they already machined.
pub const OUTLINE_STRATEGIES: [&str; 2] = ["axial_finishing", "pencil_finishing"];

/// Which tools a strategy's Exact STEP surface contact applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExactContact {
    /// A ball end (also grown for stock to leave): the exact ball drop of
    /// decision 0277, on by default.
    Ball,
    /// Every tool with an exact drop ([`has_exact_drop`](crate::has_exact_drop)):
    /// a ball, a tapered ball, a flat end at zero stock to leave (decision
    /// 0338), off by default.
    Outline,
}

impl ExactContact {
    /// Whether the setting applies to a tool of `profile`: a tool it does not
    /// apply to drops on the tessellation whatever the setting.
    pub fn supports(self, profile: &RevolvedProfile) -> bool {
        match self {
            Self::Ball => crate::ball_radius(profile).is_some(),
            Self::Outline => crate::has_exact_drop(profile),
        }
    }

    /// The setting for a new operation: on where the exact drop is measured
    /// within the limits of OI-835 (the ball, decision 0277), off where it is
    /// not (the outline, decision 0338).
    pub const fn default_on(self) -> bool {
        matches!(self, Self::Ball)
    }

    fn description(self) -> &'static str {
        match self {
            Self::Ball => {
                "On a STEP part, drop a ball-end tool on the part's exact surfaces instead of \
                 its tessellation. More accurate and can be slower. Measured on one test part \
                 (decision 0277): the deepest cut into the surfaces fell from about 0.03-0.04 mm \
                 to about 0.004 mm, and the whole job took about the same time; other parts may \
                 take longer. Off drops on the tessellation. No effect on a mesh part. Check the \
                 program before running it."
            }
            Self::Outline => {
                "On a STEP part, drop the tool (a ball, tapered ball or flat end) on the part's \
                 exact surfaces instead of its tessellation. More accurate and much slower. \
                 Measured once on one test part (decision 0338): pencil clean-up took about 31 \
                 times as long and cut at most 0.0066 mm into the surfaces instead of 0.034 mm; \
                 axial finishing about 7 times as long, 0.0041 mm instead of 0.041 mm. Times \
                 differ by part. No effect on a mesh part. Check the program before running it."
            }
        }
    }

    fn spec(self) -> ParameterSpec {
        ParameterSpec {
            name: parameter::EXACT_CONTACT.to_owned(),
            label: "Exact STEP surface contact".to_owned(),
            description: self.description().to_owned(),
            kind: ParameterKind::Flag {
                default: self.default_on(),
            },
        }
    }
}

/// `schema` with the Exact STEP surface contact of `kind` placed right after
/// the tolerance (decision record 0500), or last when it has no tolerance.
pub fn with_switch(kind: ExactContact, mut schema: ParameterSchema) -> ParameterSchema {
    let at = schema
        .parameters
        .iter()
        .position(|p| p.name == names::TOLERANCE)
        .map_or(schema.parameters.len(), |i| i + 1);
    schema.parameters.insert(at, kind.spec());
    schema
}

/// Whether `values` turn the Exact STEP surface contact on for `strategy`:
/// the stored flag, or the strategy's default when none is stored. Always
/// false for a strategy without the setting.
pub fn is_on(strategy: &dyn Strategy, values: &BTreeMap<String, ParameterValue>) -> bool {
    let Some(kind) = strategy.exact_contact() else {
        return false;
    };
    match values.get(parameter::EXACT_CONTACT) {
        Some(ParameterValue::Flag(on)) => *on,
        _ => kind.default_on(),
    }
}

/// Whether a host drops a tool of `profile` on a STEP part's exact faces for
/// an operation of `strategy` with `values`: the setting is on and applies to
/// the tool.
pub fn exact_drops(
    strategy: &dyn Strategy,
    values: &BTreeMap<String, ParameterValue>,
    profile: &RevolvedProfile,
) -> bool {
    is_on(strategy, values)
        && strategy
            .exact_contact()
            .is_some_and(|kind| kind.supports(profile))
}
