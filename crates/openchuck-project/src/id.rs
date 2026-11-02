// SPDX-License-Identifier: AGPL-3.0-or-later
//! Stable unique identifiers for project items (PRJ-05).
//!
//! Each kind of item has its own ID type, so an operation ID cannot be passed
//! where a setup ID is expected. An ID is a random 128-bit value created when
//! the user adds the item and kept for its lifetime. IDs are never made during
//! pathing, so the randomness does not affect repeatable output (NFR-06). The
//! text form is the hyphenated lower-case UUID, for example
//! `6f1c2e4a-0b7d-4c3e-9a51-2d8f0e6b7c19`.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::ProjectError;

macro_rules! define_id {
    ($(#[$doc:meta])* $name:ident, $kind:literal) => {
        $(#[$doc])*
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u128);

        impl $name {
            /// What this ID names, for messages.
            pub const KIND: &'static str = $kind;

            /// A new random ID.
            #[allow(clippy::new_without_default)]
            pub fn new() -> Self {
                Self(uuid::Uuid::new_v4().as_u128())
            }

            /// The ID with this value. For tests and for reading saved IDs.
            pub const fn from_u128(value: u128) -> Self {
                Self(value)
            }

            /// The ID's value.
            pub const fn as_u128(self) -> u128 {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&uuid::Uuid::from_u128(self.0).hyphenated(), f)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({self})", stringify!($name))
            }
        }

        impl FromStr for $name {
            type Err = ProjectError;

            fn from_str(text: &str) -> Result<Self, Self::Err> {
                parse_id(text, $kind).map(Self)
            }
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.collect_str(self)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let text = String::deserialize(deserializer)?;
                text.parse().map_err(serde::de::Error::custom)
            }
        }
    };
}

define_id!(
    /// Identifies a setup.
    SetupId,
    "setup"
);
define_id!(
    /// Identifies an operation.
    OperationId,
    "operation"
);
define_id!(
    /// Identifies a tool snapshot in a project.
    ToolId,
    "tool"
);
define_id!(
    /// Identifies a named selection set.
    SelectionSetId,
    "selection set"
);
define_id!(
    /// Identifies a model file referenced by the project.
    ModelId,
    "model"
);

/// Parses the hyphenated form only, so every saved ID has one spelling.
fn parse_id(text: &str, kind: &'static str) -> Result<u128, ProjectError> {
    let invalid = || ProjectError::InvalidId {
        kind,
        text: text.to_owned(),
    };
    let id = uuid::Uuid::try_parse(text).map_err(|_| invalid())?;
    if id.hyphenated().to_string() == text {
        Ok(id.as_u128())
    } else {
        Err(invalid())
    }
}
