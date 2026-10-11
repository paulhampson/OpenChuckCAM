// SPDX-License-Identifier: AGPL-3.0-or-later
//! Unit newtypes in double precision (NFR-05, architecture section 8).
//!
//! Lengths are millimetres and angles are degrees internally. Inch display is a
//! UI setting and never reaches this crate.

use serde::{Deserialize, Serialize};
use std::ops::{Add, Neg, Sub};

use crate::error::{Result, require_positive};

macro_rules! unit {
    ($(#[$doc:meta])* $name:ident, $suffix:literal) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub f64);

        impl $name {
            /// The raw value.
            pub const fn value(self) -> f64 {
                self.0
            }

            /// True when the value is neither infinite nor NaN.
            pub fn is_finite(self) -> bool {
                self.0.is_finite()
            }
        }

        impl Add for $name {
            type Output = Self;
            fn add(self, rhs: Self) -> Self {
                Self(self.0 + rhs.0)
            }
        }

        impl Sub for $name {
            type Output = Self;
            fn sub(self, rhs: Self) -> Self {
                Self(self.0 - rhs.0)
            }
        }

        impl Neg for $name {
            type Output = Self;
            fn neg(self) -> Self {
                Self(-self.0)
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{} {}", self.0, $suffix)
            }
        }
    };
}

unit!(
    /// A length in millimetres.
    Millimetres,
    "mm"
);
unit!(
    /// An angle in degrees. Rotary A angles are kept unwrapped (not reduced to 0..360).
    Degrees,
    "deg"
);
unit!(
    /// A linear feed rate in millimetres per minute.
    FeedRate,
    "mm/min"
);
unit!(
    /// A rotary feed rate in degrees per minute.
    RotaryFeedRate,
    "deg/min"
);
unit!(
    /// A duration in seconds.
    Seconds,
    "s"
);

impl Degrees {
    /// The angle in radians.
    pub fn to_radians(self) -> f64 {
        self.0.to_radians()
    }

    /// Builds an angle from radians.
    pub fn from_radians(radians: f64) -> Self {
        Self(radians.to_degrees())
    }
}

/// A toolpath tolerance: how far a path may deviate from the ideal surface.
///
/// The default is 0.01 mm (NFR-05); the user can change it, and it must be
/// finite and greater than zero.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "Millimetres", into = "Millimetres")]
pub struct Tolerance(Millimetres);

impl Tolerance {
    /// The default toolpath tolerance, 0.01 mm (NFR-05).
    pub const DEFAULT: Self = Self(Millimetres(0.01));

    /// A tolerance of `value`, which must be finite and greater than zero.
    pub fn new(value: Millimetres) -> Result<Self> {
        require_positive("tolerance", value.0)?;
        Ok(Self(value))
    }

    /// The tolerance as a length.
    pub const fn get(self) -> Millimetres {
        self.0
    }
}

impl Default for Tolerance {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl TryFrom<Millimetres> for Tolerance {
    type Error = crate::Error;
    fn try_from(value: Millimetres) -> Result<Self> {
        Self::new(value)
    }
}

impl From<Tolerance> for Millimetres {
    fn from(value: Tolerance) -> Self {
        value.0
    }
}
