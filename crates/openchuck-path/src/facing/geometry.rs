// SPDX-License-Identifier: AGPL-3.0-or-later
//! Facing's coverage and target geometry (OP-24; accepted decision 0244).
//!
//! Bounds project the supplied placed geometry, not its world-axis bounding
//! box. Analytic tab projections preserve their shapes. These calculations
//! describe coverage; they do not authorise removal or certify tool clearance.

use openchuck_types::geometry::{Point3, UnitVec3, Vec3};
use openchuck_types::units::Degrees;

use crate::error::{PathError, Result};
use crate::rotary::{AxisPosition, RotaryMapping};
use crate::solid::Solid;

/// The end opposite the sole defined held end (model X coordinates).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FacingEnd {
    /// The low-X end.
    Start,
    /// The high-X end.
    Far,
}

/// The target end plane and length to the opposite model end, in millimetres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EndTarget {
    /// Target model X, with the existing allowance applied outward once.
    pub x: f64,
    /// Model length plus the single end allowance; excludes tabs/held stock.
    pub length: f64,
}

impl FacingEnd {
    /// Resolve after workholding changes; never guess when neither end is held.
    pub fn opposite_held(start: bool, far: bool) -> Result<Self> {
        match (start, far) {
            (true, false) => Ok(Self::Far),
            (false, true) => Ok(Self::Start),
            (false, false) => Err(PathError::Strategy(
                "Define which end is held before facing the opposite end.".to_owned(),
            )),
            (true, true) => Err(PathError::Strategy(
                "Cannot face an unheld end: both ends are held.".to_owned(),
            )),
        }
    }

    /// Compute the model-derived target, independently of protruding tabs.
    pub fn target(self, part_x: (f64, f64), allowance: f64) -> Result<EndTarget> {
        let (lo, hi) = part_x;
        if !lo.is_finite() || !hi.is_finite() || hi < lo {
            return Err(invalid("part bounds", "must be finite and ordered"));
        }
        nonnegative("stock to leave", allowance)?;
        let x = match self {
            Self::Start => lo - allowance,
            Self::Far => hi + allowance,
        };
        let length = hi - lo + allowance;
        if !x.is_finite() || !length.is_finite() {
            return Err(invalid(
                "target end",
                "coordinate or finished length overflowed",
            ));
        }
        Ok(EndTarget { x, length })
    }
}

/// Distribution of depths for the new indexed box-side mode only.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LayerDistribution {
    /// Full selected increments, then an exact, potentially shorter final pass.
    #[default]
    Full,
    /// Equal increments, none larger than the selected maximum.
    Equal,
}

impl LayerDistribution {
    /// Inward plane offsets from stock support to target, excluding stock top.
    /// A satisfied target gives no layers; an outward/unbounded target refuses.
    /// This function does not change the legacy roughing layer arithmetic.
    pub fn levels(self, outer: f64, target: f64, maximum: f64) -> Result<Vec<f64>> {
        if !outer.is_finite() || !target.is_finite() || target > outer {
            return Err(invalid(
                "facing layers",
                "target must be finite and at or below stock support",
            ));
        }
        if !maximum.is_finite() || maximum <= 0.0 {
            return Err(invalid("stepdown", "must be finite and above zero"));
        }
        let depth = outer - target;
        let count = (depth / maximum).ceil();
        if !depth.is_finite() || !count.is_finite() || count > 100_000.0 {
            return Err(invalid(
                "stepdown",
                "would require more than 100000 facing layers",
            ));
        }
        // The range check precedes the float-to-integer conversion/allocation.
        let count = count as usize;
        let mut levels = Vec::with_capacity(count);
        let step = match self {
            Self::Full => maximum,
            Self::Equal if count > 0 => depth / count as f64,
            Self::Equal => return Ok(levels),
        };
        let mut previous = outer;
        for k in 1..=count {
            let level = if k == count {
                target
            } else {
                (outer - k as f64 * step).max(target)
            };
            if !level.is_finite() || level >= previous {
                return Err(invalid(
                    "stepdown",
                    "cannot make progress at these coordinates",
                ));
            }
            levels.push(level);
            previous = level;
            if level <= target {
                break;
            }
        }
        Ok(levels)
    }
}

/// The rectangle enclosing a projection in a facing plane, in millimetres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoverageRectangle {
    min: [f64; 2],
    max: [f64; 2],
}

impl CoverageRectangle {
    /// Exchange the two in-plane directions without changing the region.
    pub fn transposed(self) -> Self {
        let [u0, v0] = self.min;
        let [u1, v1] = self.max;
        Self {
            min: [v0, u0],
            max: [v1, u1],
        }
    }

    /// Lower coordinates in the plane's two displayed directions.
    pub fn min(self) -> [f64; 2] {
        self.min
    }

    /// Upper coordinates in the plane's two displayed directions.
    pub fn max(self) -> [f64; 2] {
        self.max
    }
}

/// Model-frame directions used by both path coverage and host diagrams.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FacingPlane {
    u: UnitVec3,
    v: UnitVec3,
    normal: Vec3,
}

impl FacingPlane {
    /// Reverse only the outward normal, retaining both projection directions.
    /// This obtains the opposite support without adding/rounding an A angle.
    pub fn opposite(self) -> Self {
        Self {
            normal: -self.normal,
            ..self
        }
    }

    /// Exchange the two raster directions, retaining the same outward target.
    pub fn transposed(self) -> Self {
        Self {
            u: self.v,
            v: self.u,
            normal: self.normal,
        }
    }

    /// Project a model point into the plane's u, v and outward-normal
    /// coordinates. The normal coordinate is the target plane's offset.
    pub fn coordinates(self, point: Point3) -> Result<[f64; 3]> {
        let p = point.to_vec();
        let result = [p.dot(self.u.get()), p.dot(self.v.get()), p.dot(self.normal)];
        if result.iter().any(|v| !v.is_finite()) {
            return Err(invalid("facing geometry", "point projection is not finite"));
        }
        Ok(result)
    }

    /// Outward model-frame unit direction, for actual-motion checks.
    pub fn normal(self) -> Vec3 {
        self.normal
    }

    /// A model point from its two plane coordinates and outward plane offset.
    pub fn point(self, u: f64, v: f64, offset: f64) -> Result<Point3> {
        let point = Point3::ORIGIN + self.u.get() * u + self.v.get() * v + self.normal * offset;
        if !point.is_finite() {
            return Err(invalid("facing geometry", "planned point is not finite"));
        }
        Ok(point)
    }

    /// Y-Z end projection. Low/high X changes the outward target normal only.
    pub fn end(end: FacingEnd) -> Self {
        Self {
            u: UnitVec3::Y,
            v: UnitVec3::Z,
            normal: match end {
                FacingEnd::Start => -UnitVec3::X.get(),
                FacingEnd::Far => UnitVec3::X.get(),
            },
        }
    }

    /// X-transverse indexed-side projection using shared machine kinematics.
    pub fn side(rotary: RotaryMapping, a: Degrees) -> Result<Self> {
        if !a.0.is_finite() {
            return Err(invalid("facing angle", "must be finite"));
        }
        let along = rotary.to_model(
            AxisPosition {
                along: 1.0,
                across: 0.0,
                height: 0.0,
            },
            a,
        );
        let across = rotary.to_model(
            AxisPosition {
                along: 0.0,
                across: 1.0,
                height: 0.0,
            },
            a,
        );
        Ok(Self {
            u: UnitVec3::new(along.to_vec())?,
            v: UnitVec3::new(across.to_vec())?,
            normal: rotary.tool_axis(a).get(),
        })
    }

    /// Project the entire supplied geometry and optional tab union, then add
    /// margin. For a mesh, pass its placed vertices; never substitute its AABB.
    /// Protected solids not included here must still be checked during motion.
    pub fn bounds(
        self,
        points: impl IntoIterator<Item = Point3>,
        tabs: &[Solid],
        margin: f64,
    ) -> Result<CoverageRectangle> {
        nonnegative("facing margin", margin)?;
        let mut u = Range::default();
        let mut v = Range::default();
        for p in points {
            if !p.is_finite() {
                return Err(invalid(
                    "facing geometry",
                    "every placed vertex must be finite",
                ));
            }
            u.include(p.to_vec().dot(self.u.get()))?;
            v.include(p.to_vec().dot(self.v.get()))?;
        }
        for tab in tabs {
            u.solid(tab, self.u.get())?;
            v.solid(tab, self.v.get())?;
        }
        let (u0, u1) = u.finish()?;
        let (v0, v1) = v.finish()?;
        let result = CoverageRectangle {
            min: [u0 - margin, v0 - margin],
            max: [u1 + margin, v1 + margin],
        };
        if result
            .min
            .into_iter()
            .chain(result.max)
            .any(|v| !v.is_finite())
        {
            return Err(invalid("facing margin", "expanded projection overflowed"));
        }
        Ok(result)
    }

    /// Outermost part/tab support along the target normal. Call with the part
    /// alone when deriving a finished dimension; tabs never extend that value.
    pub fn support(self, points: impl IntoIterator<Item = Point3>, tabs: &[Solid]) -> Result<f64> {
        let mut range = Range::default();
        for p in points {
            if !p.is_finite() {
                return Err(invalid(
                    "facing geometry",
                    "every placed vertex must be finite",
                ));
            }
            range.include(p.to_vec().dot(self.normal))?;
        }
        for tab in tabs {
            range.solid(tab, self.normal)?;
        }
        Ok(range.finish()?.1)
    }
}

#[derive(Debug)]
struct Range {
    lo: f64,
    hi: f64,
}

impl Default for Range {
    fn default() -> Self {
        Self {
            lo: f64::INFINITY,
            hi: f64::NEG_INFINITY,
        }
    }
}

impl Range {
    fn include(&mut self, x: f64) -> Result<()> {
        if !x.is_finite() {
            return Err(invalid("facing geometry", "projection overflowed"));
        }
        self.lo = self.lo.min(x);
        self.hi = self.hi.max(x);
        Ok(())
    }

    fn solid(&mut self, solid: &Solid, direction: Vec3) -> Result<()> {
        if let Some(b) = solid.as_cuboid() {
            let mut lo = 0.0;
            let mut hi = 0.0;
            for (d, a, z) in [
                (direction.x, b.min.x, b.max.x),
                (direction.y, b.min.y, b.max.y),
                (direction.z, b.min.z, b.max.z),
            ] {
                lo += (d * a).min(d * z);
                hi += (d * a).max(d * z);
            }
            self.include(lo)?;
            return self.include(hi);
        }
        // A frustum is the convex hull of its cap discs; a linear functional
        // attains its extremes on those discs. Cross length avoids cancellation
        // when the projection direction is almost parallel to the solid axis.
        let radial = direction.cross(solid.axis().get()).length();
        for (centre, radius) in [
            (solid.base(), solid.base_radius()),
            (solid.top(), solid.top_radius()),
        ] {
            let mid = centre.to_vec().dot(direction);
            self.include(mid - radius * radial)?;
            self.include(mid + radius * radial)?;
        }
        Ok(())
    }

    fn finish(self) -> Result<(f64, f64)> {
        if self.lo > self.hi {
            return Err(invalid("facing geometry", "cannot project empty geometry"));
        }
        Ok((self.lo, self.hi))
    }
}

fn invalid(field: &str, reason: &str) -> PathError {
    PathError::parameter(field, reason)
}

fn nonnegative(field: &str, value: f64) -> Result<()> {
    if value.is_finite() && value >= 0.0 {
        Ok(())
    } else {
        Err(invalid(field, "must be finite and nonnegative"))
    }
}

pub(super) fn projection_range(
    direction: UnitVec3,
    points: impl IntoIterator<Item = Point3>,
    solids: &[Solid],
) -> Result<(f64, f64)> {
    let mut range = Range::default();
    for p in points {
        if !p.is_finite() {
            return Err(invalid(
                "facing geometry",
                "every placed vertex must be finite",
            ));
        }
        range.include(p.to_vec().dot(direction.get()))?;
    }
    for solid in solids {
        range.solid(solid, direction.get())?;
    }
    range.finish()
}
