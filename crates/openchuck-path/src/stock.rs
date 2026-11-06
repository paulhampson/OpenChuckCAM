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

    /// The ends of the material along the rotary axis, model X (low, high),
    /// if known: where a path extended past the stock starts and ends
    /// (OP-24; decision record 0212 part B). `None` by default.
    fn x_extent(&self) -> Option<(f64, f64)> {
        None
    }

    /// True when this is the material earlier operations in the setup left
    /// (SIM-06), so what lies past the part is known; false for the setup's
    /// own stock definition, the default. A free end is faced on either; on
    /// the setup's own stock its note says the real blank must match it
    /// (decision record 0344, OI-1394).
    fn left_by_earlier_operations(&self) -> bool {
        false
    }

    /// The stock in a few words for a message, such as "a 35.000 x 35.000 mm
    /// box, its corners 24.749 mm from the rotary axis", if it has a simple
    /// shape (decision record 0509). `None` by default.
    fn describe(&self) -> Option<String> {
        None
    }

    /// The highest any material stands along `axis` above `centre`, within
    /// `radius` of the line through `centre` along `axis`, as
    /// `dot(p - centre, axis)` at its largest; negative infinity where there
    /// is no material there. Exact where it is given: the flute check of
    /// decision record 0509 then cannot miss material between readings.
    /// `None`, the default, when the model does not compute it; the check
    /// then holds the stock to a conservative bound from
    /// [`StockModel::max_radius`] (OI-2025).
    fn top_over_disc(&self, centre: Point3, axis: UnitVec3, radius: f64) -> Option<f64> {
        let _ = (centre, axis, radius);
        None
    }

    /// True when the material is the same at every angle of A: it is
    /// symmetric about the rotary axis, as round stock on the axis is. The
    /// flute check of decision record 0509 then adds nothing for the
    /// stock's turn between two readings, so stock flush with the flutes
    /// through a turn is measured exactly. False, the default, when the
    /// model does not say so.
    fn turns_unchanged(&self) -> bool {
        false
    }

    /// How far the material reaches from the rotary axis, slice by slice
    /// along X and sector by sector about the axis, for a model with no
    /// exact [`StockModel::top_over_disc`] (issue #547): the flute check of
    /// decision record 0509 then bounds the stock over the tool from the
    /// material within the tool's reach along X and towards the tool, not
    /// from the farthest material anywhere. Every material point must lie
    /// within the reach of its slice and sector. `None`, the default, when
    /// the model does not compute it; the check then keeps the bound from
    /// [`StockModel::max_radius`].
    fn reach_profile(&self) -> Option<ReachProfile> {
        None
    }
}

/// How far material reaches from the rotary axis (model X), by slice along
/// X and by sector of direction about it ([`StockModel::reach_profile`];
/// issue #547). Built from boxes of material, each recorded at its farthest
/// corner in every sector it spans, so it bounds the material from above:
/// a reach is never less than the material's, and may be more.
#[derive(Debug, Clone, PartialEq)]
pub struct ReachProfile {
    /// Model X where the first slice starts.
    x0: f64,
    /// The slices' length along X, mm.
    dx: f64,
    /// The number of slices.
    slices: usize,
    /// The farthest reach in each slice, mm; negative infinity where empty.
    slice_reach: Vec<f64>,
    /// The reach of slice `i`, sector `s` at `i * SECTORS + s`; negative
    /// infinity where empty.
    reach: Vec<f64>,
    /// The highest the material of slice `i` stands along any direction of
    /// band `k` (1 degree from `k` degrees), at `i * SECTORS + k`; negative
    /// infinity where the slice is empty. Worked out once, on the first
    /// reading after the last [`ReachProfile::add`], so a reading costs a
    /// look-up per slice and band, not a pass over every sector (the owner's
    /// project: about 710 s of checking with the pass over sectors).
    support: std::sync::OnceLock<Vec<f64>>,
}

impl ReachProfile {
    /// The number of sectors about the axis, each 1 degree.
    pub const SECTORS: usize = 360;

    /// An empty profile of `slices` slices of `dx` mm along X from `x0`;
    /// `None` when the sizes are not finite and positive.
    pub fn new(x0: f64, dx: f64, slices: usize) -> Option<Self> {
        (x0.is_finite() && dx.is_finite() && dx > 0.0 && slices > 0).then(|| Self {
            x0,
            dx,
            slices,
            slice_reach: vec![f64::NEG_INFINITY; slices],
            reach: vec![f64::NEG_INFINITY; slices * Self::SECTORS],
            support: std::sync::OnceLock::new(),
        })
    }

    /// Records material that may fill the box `y` by `z` (model Y and Z,
    /// low and high) in slice `slice`: its farthest corner from the axis,
    /// in every sector the box spans. A box that is not finite fills the
    /// slice at infinite reach, so it can only make a bound larger.
    pub fn add(&mut self, slice: usize, y: [f64; 2], z: [f64; 2]) {
        if slice >= self.slices {
            return;
        }
        // The bands are worked out again from the sectors.
        self.support = std::sync::OnceLock::new();
        let corners = [(y[0], z[0]), (y[0], z[1]), (y[1], z[0]), (y[1], z[1])];
        let far = corners
            .iter()
            .map(|&(a, b)| a.hypot(b))
            .fold(
                0.0_f64,
                |m, r| if r.is_nan() { f64::INFINITY } else { m.max(r) },
            );
        let finite = y.iter().chain(&z).all(|v| v.is_finite());
        let around = !finite || (y[0] <= 0.0 && 0.0 <= y[1] && z[0] <= 0.0 && 0.0 <= z[1]);
        let (from, to) = if around {
            (0.0, std::f64::consts::TAU)
        } else {
            // A box clear of the axis subtends less than half a turn: the
            // arc from its centre's direction to its corners'.
            let centre = (0.5 * (z[0] + z[1])).atan2(0.5 * (y[0] + y[1]));
            let (lo, hi) = corners
                .iter()
                .fold((0.0_f64, 0.0_f64), |(lo, hi), &(a, b)| {
                    let d = wrap(b.atan2(a) - centre);
                    (lo.min(d), hi.max(d))
                });
            (centre + lo, centre + hi)
        };
        let width = std::f64::consts::TAU / Self::SECTORS as f64;
        let first = (from / width).floor() as i64;
        let last = (to / width).floor() as i64;
        let count = (last - first + 1).clamp(1, Self::SECTORS as i64);
        let row = slice * Self::SECTORS;
        for k in 0..count {
            let s = (first + k).rem_euclid(Self::SECTORS as i64) as usize;
            if let Some(r) = self.reach.get_mut(row + s) {
                *r = r.max(far);
            }
        }
        if let Some(r) = self.slice_reach.get_mut(slice) {
            *r = r.max(far);
        }
    }

    /// The highest any material with model X in `x` (low, high) can stand
    /// along a direction about the axis, `dot(p, direction)`, for every
    /// direction within `spread` radians of `towards` (the angle from model
    /// +Y towards +Z): an upper bound, negative infinity where no material
    /// is in reach. Material past the profile's ends is not known: the
    /// profile must hold all the material.
    pub fn highest(&self, x: [f64; 2], towards: f64, spread: f64) -> f64 {
        if !(x[0].is_finite() && x[1].is_finite() && towards.is_finite() && spread.is_finite()) {
            return f64::INFINITY;
        }
        let first = ((x[0] - self.x0) / self.dx).floor().max(0.0);
        let last = ((x[1] - self.x0) / self.dx)
            .floor()
            .min(self.slices as f64 - 1.0);
        if last < first {
            return f64::NEG_INFINITY;
        }
        let support = self.support.get_or_init(|| self.bands());
        let width = std::f64::consts::TAU / Self::SECTORS as f64;
        let spread = spread.abs();
        // The bands the directions fall in, every one for a spread of half a
        // turn or more.
        let lo = ((towards - spread) / width).floor();
        let hi = ((towards + spread) / width).floor();
        let count = ((hi - lo) as usize + 1).min(Self::SECTORS);
        let lo = lo.rem_euclid(Self::SECTORS as f64) as usize;
        let mut best = f64::NEG_INFINITY;
        for i in (first as usize)..=(last as usize) {
            let all = self.slice_reach.get(i).copied().unwrap_or(f64::INFINITY);
            if all == f64::NEG_INFINITY {
                continue;
            }
            for b in 0..count {
                let k = (lo + b) % Self::SECTORS;
                let h = support
                    .get(i * Self::SECTORS + k)
                    .copied()
                    .unwrap_or(f64::INFINITY);
                best = best.max(h);
            }
        }
        best
    }

    /// For each slice and band of directions, the highest its material can
    /// stand along any direction in the band: for each sector, its reach
    /// times the largest cosine between the sector and the band, and at
    /// least the axis's height (material nearer the axis than its reach
    /// stands no higher than the reach's height where that is positive).
    /// The cosine depends only on how many sectors apart the two are.
    fn bands(&self) -> Vec<f64> {
        let n = Self::SECTORS;
        let width = std::f64::consts::TAU / n as f64;
        // The largest cosine of an angle between a sector `d` sectors on
        // from a band and that band: angles from (d - 1) to (d + 1) widths.
        let cosine: Vec<f64> = (0..n)
            .map(|d| {
                let lo = (d as f64 - 1.0) * width;
                let hi = (d as f64 + 1.0) * width;
                let t = std::f64::consts::TAU;
                if (lo / t).ceil() <= (hi / t).floor() {
                    1.0
                } else {
                    lo.cos().max(hi.cos())
                }
            })
            .collect();
        let mut out = vec![f64::NEG_INFINITY; self.slices * n];
        for i in 0..self.slices {
            if self.slice_reach.get(i).is_none_or(|r| *r == f64::NEG_INFINITY) {
                continue;
            }
            let row = self.reach.get(i * n..(i + 1) * n).unwrap_or_default();
            for (k, best) in out.iter_mut().skip(i * n).take(n).enumerate() {
                for (s, &r) in row.iter().enumerate() {
                    if r == f64::NEG_INFINITY {
                        continue;
                    }
                    let c = cosine.get((s + n - k) % n).copied().unwrap_or(1.0);
                    *best = best.max((r * c).max(0.0));
                }
            }
        }
        out
    }
}

/// `angle` wrapped to minus half a turn to half a turn, radians.
fn wrap(angle: f64) -> f64 {
    let t = std::f64::consts::TAU;
    let w = angle.rem_euclid(t);
    if w > 0.5 * t { w - t } else { w }
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

impl AnalyticStock {
    /// True when every size and place of the stock is a finite number.
    fn is_finite(&self) -> bool {
        match *self {
            Self::Cylinder {
                radius,
                x_start,
                x_end,
            } => radius.is_finite() && x_start.is_finite() && x_end.is_finite(),
            Self::Box(b) => b.min.is_finite() && b.max.is_finite(),
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

    fn x_extent(&self) -> Option<(f64, f64)> {
        match *self {
            Self::Cylinder { x_start, x_end, .. } => Some((x_start, x_end)),
            Self::Box(b) => Some((b.min.x, b.max.x)),
        }
    }

    fn top_over_disc(&self, centre: Point3, axis: UnitVec3, radius: f64) -> Option<f64> {
        let a = axis.get();
        // The tool axis of a 4-axis machine is across the rotary axis.
        // A centre, axis or radius that is not a finite number has no
        // answer (NFR-07): the caller then holds the stock to its bound.
        if !centre.is_finite() || !a.is_finite() || !radius.is_finite() || !self.is_finite() {
            return None;
        }
        if a.x.abs() > 1e-9 || radius < 0.0 {
            return None;
        }
        // Coordinates across X: `w` across the tool axis, `h` along it.
        let e = Vec3::new(0.0, -a.z, a.y);
        let (wc, hc) = (
            e.y * centre.y + e.z * centre.z,
            a.y * centre.y + a.z * centre.z,
        );
        let (x0, x1) = match *self {
            Self::Cylinder { x_start, x_end, .. } => (x_start, x_end),
            Self::Box(b) => (b.min.x, b.max.x),
        };
        // The nearest X of the material to the centre's; what is left of the
        // radius is the half width across the tool axis there.
        let dx = (x0 - centre.x).max(centre.x - x1).max(0.0);
        if dx > radius {
            return Some(f64::NEG_INFINITY);
        }
        // The half width, at most `radius`; where its square overflows,
        // `radius` itself, which is wider and so still contains the disc.
        let s = (radius * radius - dx * dx).sqrt();
        let s = if s.is_finite() { s } else { radius };
        let (lo, hi) = (wc - s, wc + s);
        // Coordinates too large to compare have no answer (NFR-07): `clamp`
        // below needs ordered bounds.
        if lo.is_nan() || hi.is_nan() || hc.is_nan() {
            return None;
        }
        let top = match *self {
            Self::Cylinder { radius: r, .. } => {
                // The section is a disc about the axis: highest at the `w`
                // nearest zero within the slab.
                let w = 0.0_f64.clamp(lo, hi);
                if w.abs() > r {
                    f64::NEG_INFINITY
                } else {
                    (r * r - w * w).sqrt()
                }
            }
            Self::Box(b) => {
                // The section is a rectangle; the highest point of it within
                // the slab is a corner inside it or where an edge crosses
                // one of its sides.
                let corners = [
                    (b.min.y, b.min.z),
                    (b.max.y, b.min.z),
                    (b.max.y, b.max.z),
                    (b.min.y, b.max.z),
                ]
                .map(|(y, z)| (e.y * y + e.z * z, a.y * y + a.z * z));
                let mut top = f64::NEG_INFINITY;
                let next = corners.iter().cycle().skip(1);
                for (&(w0, h0), &(w1, h1)) in corners.iter().zip(next) {
                    if (lo..=hi).contains(&w0) {
                        top = top.max(h0);
                    }
                    for side in [lo, hi] {
                        if (w0 - side) * (w1 - side) < 0.0 {
                            let t = (side - w0) / (w1 - w0);
                            top = top.max(h0 + t * (h1 - h0));
                        }
                    }
                }
                top
            }
        };
        let above = top - hc;
        (!above.is_nan()).then_some(above)
    }

    fn describe(&self) -> Option<String> {
        Some(match *self {
            Self::Cylinder { radius, .. } => {
                format!(
                    "a {:.3} mm diameter cylinder on the rotary axis",
                    2.0 * radius
                )
            }
            Self::Box(b) => format!(
                "a {:.3} x {:.3} mm box (Y {:.3} to {:.3}, Z {:.3} to {:.3}), its farthest \
                 corner {:.3} mm from the rotary axis",
                b.max.y - b.min.y,
                b.max.z - b.min.z,
                b.min.y,
                b.max.y,
                b.min.z,
                b.max.z,
                self.max_radius().unwrap_or(0.0)
            ),
        })
    }

    fn turns_unchanged(&self) -> bool {
        matches!(self, Self::Cylinder { .. })
    }

    fn max_radius(&self) -> Option<f64> {
        // A size that is not a finite number has no farthest radius
        // (NFR-07): `f64::max` would otherwise drop the NaN.
        if !self.is_finite() {
            return None;
        }
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
