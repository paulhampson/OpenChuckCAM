// SPDX-License-Identifier: AGPL-3.0-or-later
//! Points, vectors, unit vectors and boxes in double precision, in millimetres.
//!
//! These are our own plain types, so no maths crate appears in a public API
//! (architecture section 2).

use serde::{Deserialize, Serialize};
use std::ops::{Add, Mul, Neg, Sub};

use crate::error::{Error, Result, require_finite};

/// A point in millimetres.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Point3 {
    /// X coordinate.
    pub x: f64,
    /// Y coordinate.
    pub y: f64,
    /// Z coordinate.
    pub z: f64,
}

/// A displacement or direction in millimetres.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Vec3 {
    /// X component.
    pub x: f64,
    /// Y component.
    pub y: f64,
    /// Z component.
    pub z: f64,
}

impl Point3 {
    /// The origin.
    pub const ORIGIN: Self = Self::new(0.0, 0.0, 0.0);

    /// A point from its coordinates.
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    /// The displacement from the origin to this point.
    pub const fn to_vec(self) -> Vec3 {
        Vec3::new(self.x, self.y, self.z)
    }

    /// Euclidean distance to `other`.
    pub fn distance(self, other: Self) -> f64 {
        (self - other).length()
    }

    /// True when every coordinate is finite.
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }

    /// Fails unless every coordinate is finite.
    pub fn validate(self, field: &str) -> Result<()> {
        require_finite(&format!("{field}.x"), self.x)?;
        require_finite(&format!("{field}.y"), self.y)?;
        require_finite(&format!("{field}.z"), self.z)
    }
}

impl Vec3 {
    /// The zero vector.
    pub const ZERO: Self = Self::new(0.0, 0.0, 0.0);

    /// A vector from its components.
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    /// Dot product.
    pub fn dot(self, other: Self) -> f64 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    /// Cross product.
    pub fn cross(self, other: Self) -> Self {
        Self::new(
            self.y * other.z - self.z * other.y,
            self.z * other.x - self.x * other.z,
            self.x * other.y - self.y * other.x,
        )
    }

    /// Euclidean length.
    pub fn length(self) -> f64 {
        self.dot(self).sqrt()
    }

    /// True when every component is finite.
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }
}

impl Sub for Point3 {
    type Output = Vec3;
    fn sub(self, rhs: Self) -> Vec3 {
        Vec3::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
    }
}

impl Add<Vec3> for Point3 {
    type Output = Point3;
    fn add(self, rhs: Vec3) -> Point3 {
        Point3::new(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
    }
}

impl Add for Vec3 {
    type Output = Vec3;
    fn add(self, rhs: Self) -> Vec3 {
        Vec3::new(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
    }
}

impl Sub for Vec3 {
    type Output = Vec3;
    fn sub(self, rhs: Self) -> Vec3 {
        Vec3::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
    }
}

impl Neg for Vec3 {
    type Output = Vec3;
    fn neg(self) -> Vec3 {
        Vec3::new(-self.x, -self.y, -self.z)
    }
}

impl Mul<f64> for Vec3 {
    type Output = Vec3;
    fn mul(self, rhs: f64) -> Vec3 {
        Vec3::new(self.x * rhs, self.y * rhs, self.z * rhs)
    }
}

/// A direction of length one.
///
/// [`UnitVec3::new`] normalises its input. Deserialising is strict: a stored
/// vector must already have length one (within 1e-9), so a corrupt file is an
/// error rather than a silently different direction.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "Vec3", into = "Vec3")]
pub struct UnitVec3(Vec3);

impl UnitVec3 {
    /// The +X direction.
    pub const X: Self = Self(Vec3::new(1.0, 0.0, 0.0));
    /// The +Y direction.
    pub const Y: Self = Self(Vec3::new(0.0, 1.0, 0.0));
    /// The +Z direction.
    pub const Z: Self = Self(Vec3::new(0.0, 0.0, 1.0));

    /// Allowed deviation of a stored unit vector's length from one.
    pub const LENGTH_TOLERANCE: f64 = 1e-9;

    /// Normalises `v`. Fails if `v` is zero or not finite.
    pub fn new(v: Vec3) -> Result<Self> {
        let length = v.length();
        if !length.is_finite() || length <= f64::MIN_POSITIVE {
            return Err(Error::invalid(
                "direction",
                "a direction must be finite and non-zero",
            ));
        }
        Ok(Self(v * (1.0 / length)))
    }

    /// The direction as a vector.
    pub const fn get(self) -> Vec3 {
        self.0
    }
}

impl TryFrom<Vec3> for UnitVec3 {
    type Error = Error;
    fn try_from(v: Vec3) -> Result<Self> {
        let length = v.length();
        if v.is_finite() && (length - 1.0).abs() <= Self::LENGTH_TOLERANCE {
            Ok(Self(v))
        } else {
            Err(Error::invalid(
                "direction",
                format!("expected a unit vector, found length {length}"),
            ))
        }
    }
}

impl From<UnitVec3> for Vec3 {
    fn from(v: UnitVec3) -> Self {
        v.0
    }
}

/// An axis-aligned box.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Aabb {
    /// Lowest corner.
    pub min: Point3,
    /// Highest corner.
    pub max: Point3,
}

impl Aabb {
    /// Fails unless the corners are finite and `min` is not above `max` on any axis.
    pub fn validate(&self, field: &str) -> Result<()> {
        self.min.validate(&format!("{field}.min"))?;
        self.max.validate(&format!("{field}.max"))?;
        if self.min.x > self.max.x || self.min.y > self.max.y || self.min.z > self.max.z {
            return Err(Error::invalid(field, "min must not exceed max on any axis"));
        }
        Ok(())
    }

    /// True when `p` lies inside or on the box.
    pub fn contains(&self, p: Point3) -> bool {
        (self.min.x..=self.max.x).contains(&p.x)
            && (self.min.y..=self.max.y).contains(&p.y)
            && (self.min.z..=self.max.z).contains(&p.z)
    }
}
