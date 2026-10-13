// SPDX-License-Identifier: AGPL-3.0-or-later
//! The `StockModel` and `CollisionOracle` ports (architecture section 5.2;
//! work breakdown slice 5 item 2).
//!
//! `openchuck-sim` implements both later (slice 7). The library ships an
//! analytic [`AnalyticStock`] for tests and small hosts, and [`AcceptAll`],
//! which accepts every move and relies on the clearance geometry
//! (verification plan section 5, the openchuck-path row).

use openchuck_types::geometry::{Aabb, Point3, UnitVec3, Vec3};
use openchuck_types::ir::{MoveKind, ToolPose};
use openchuck_types::stock::Stock;

use crate::error::{PathError, Result};

/// The material still to be cut, as the strategies and the linking see it.
/// Model frame.
pub trait StockModel: Send + Sync + std::fmt::Debug {
    /// True when `point` is inside the material.
    fn contains(&self, point: Point3) -> bool;

    /// The distance along the ray from `origin` (unit `direction`) at which
    /// it first enters material, no further than `max_distance`; zero when
    /// `origin` is already inside, `None` when the ray stays clear.
    fn ray_entry(&self, origin: Point3, direction: UnitVec3, max_distance: f64) -> Option<f64>;

    /// The largest distance of any material from the rotary axis, if known.
    fn max_radius(&self) -> Option<f64>;
}

/// Stock as a solid of simple shape: a cylinder on the rotary axis or a box
/// (SET-03). Bounding-box stock is a box once the part's bounds are known.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AnalyticStock {
    /// A cylinder about the model X axis from `x_start` to `x_end`.
    Cylinder {
        /// Radius.
        radius: f64,
        /// Lower end along X.
        x_start: f64,
        /// Upper end along X.
        x_end: f64,
    },
    /// A box in the model frame.
    Box(Aabb),
}

impl AnalyticStock {
    /// The stock of a setup. `part_bounds` resolves bounding-box stock;
    /// imported stock (SET-04, P2) is refused.
    pub fn from_stock(stock: &Stock, part_bounds: Option<Aabb>) -> Result<Self> {
        stock.validate()?;
        match stock {
            Stock::Cylinder {
                diameter,
                length,
                x_start,
            } => Ok(Self::Cylinder {
                radius: 0.5 * diameter.0,
                x_start: x_start.0,
                x_end: x_start.0 + length.0,
            }),
            Stock::Box { bounds } => Ok(Self::Box(*bounds)),
            Stock::OffsetBoundingBox { offset } => {
                let b = part_bounds.ok_or_else(|| {
                    PathError::Strategy(
                        "bounding-box stock needs the part, and the setup has none".to_owned(),
                    )
                })?;
                let d = Vec3::new(offset.0, offset.0, offset.0);
                Ok(Self::Box(Aabb {
                    min: b.min + -d,
                    max: b.max + d,
                }))
            }
            _ => Err(PathError::Strategy(
                "imported stock is not supported yet (SET-04)".to_owned(),
            )),
        }
    }
}

impl StockModel for AnalyticStock {
    fn contains(&self, p: Point3) -> bool {
        match *self {
            Self::Cylinder {
                radius,
                x_start,
                x_end,
            } => (x_start..=x_end).contains(&p.x) && p.y * p.y + p.z * p.z <= radius * radius,
            Self::Box(b) => b.contains(p),
        }
    }

    fn ray_entry(&self, o: Point3, d: UnitVec3, max_distance: f64) -> Option<f64> {
        let d = d.get();
        let (t0, t1) = match *self {
            Self::Box(b) => slab_interval(o, d, b)?,
            Self::Cylinder {
                radius,
                x_start,
                x_end,
            } => {
                // Inside the infinite cylinder: |(o + t d)_yz|^2 <= r^2.
                let a = d.y * d.y + d.z * d.z;
                let b = o.y * d.y + o.z * d.z;
                let c = o.y * o.y + o.z * o.z - radius * radius;
                let (c0, c1) = if a <= f64::MIN_POSITIVE {
                    if c > 0.0 {
                        return None;
                    }
                    (f64::NEG_INFINITY, f64::INFINITY)
                } else {
                    let disc = b * b - a * c;
                    if disc < 0.0 {
                        return None;
                    }
                    let s = disc.sqrt();
                    ((-b - s) / a, (-b + s) / a)
                };
                let (x0, x1) = if d.x.abs() <= f64::MIN_POSITIVE {
                    if !(x_start..=x_end).contains(&o.x) {
                        return None;
                    }
                    (f64::NEG_INFINITY, f64::INFINITY)
                } else {
                    let ta = (x_start - o.x) / d.x;
                    let tb = (x_end - o.x) / d.x;
                    (ta.min(tb), ta.max(tb))
                };
                (c0.max(x0), c1.min(x1))
            }
        };
        if t0 > t1 || t1 < 0.0 {
            return None;
        }
        let entry = t0.max(0.0);
        (entry <= max_distance).then_some(entry)
    }

    fn max_radius(&self) -> Option<f64> {
        Some(match *self {
            Self::Cylinder { radius, .. } => radius,
            Self::Box(b) => {
                let y = b.min.y.abs().max(b.max.y.abs());
                let z = b.min.z.abs().max(b.max.z.abs());
                (y * y + z * z).sqrt()
            }
        })
    }
}

fn slab_interval(o: Point3, d: Vec3, b: Aabb) -> Option<(f64, f64)> {
    let mut t0 = f64::NEG_INFINITY;
    let mut t1 = f64::INFINITY;
    for (o, d, lo, hi) in [
        (o.x, d.x, b.min.x, b.max.x),
        (o.y, d.y, b.min.y, b.max.y),
        (o.z, d.z, b.min.z, b.max.z),
    ] {
        if d.abs() <= f64::MIN_POSITIVE {
            if o < lo || o > hi {
                return None;
            }
        } else {
            let ta = (lo - o) / d;
            let tb = (hi - o) / d;
            t0 = t0.max(ta.min(tb));
            t1 = t1.min(ta.max(tb));
        }
    }
    Some((t0, t1))
}

/// Answers whether a move is free of collisions (OP-14, COL-08).
pub trait CollisionOracle: Send + Sync + std::fmt::Debug {
    /// True when the tool may move from `from` to `to` as a move of `kind`.
    fn allows(&self, from: &ToolPose, to: &ToolPose, kind: MoveKind) -> bool;
}

/// Accepts every move; the clearance geometry keeps links safe.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AcceptAll;

impl CollisionOracle for AcceptAll {
    fn allows(&self, _from: &ToolPose, _to: &ToolPose, _kind: MoveKind) -> bool {
        true
    }
}
