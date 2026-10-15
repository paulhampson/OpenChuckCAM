// SPDX-License-Identifier: AGPL-3.0-or-later
//! The material left on a rotary part as a heightfield (decision record
//! 0033 item 2).
//!
//! Rotary roughing cuts from the outside in with the tool across the axis, so
//! the material at any angle about the axis and position along it is solid
//! from the rotary axis out to some radius, the **top** of the stock there.
//! [`HeightStock`] stores that radius on a grid of cells over (X, A). It is
//! built once from the setup's stock, cut as the roughing passes are planned
//! ([`HeightStock::cut`]) and copied for each layer, so the shared services
//! can check a pass's entry and its link from the pass before against what
//! the operation has already removed (OP-14), not only against the stock it
//! started with.
//!
//! The field is conservative where it matters for safety: a cut removes
//! material from a cell only when the whole cell is inside the tool, and
//! [`StockModel`] queries treat a cell as occupied when it or a neighbour is.
//! Material under an undercut in the setup's stock is taken as solid.

use openchuck_types::geometry::{Point3, UnitVec3};
use openchuck_types::tool::profile::RevolvedProfile;
use openchuck_types::units::Degrees;

use crate::error::{PathError, Result};
use crate::parallel;
use crate::rotary::{AxisPosition, RotaryMapping};
use crate::stock::StockModel;

/// The most cells one field may have; the cell size grows to fit.
pub const MAX_CELLS: usize = 400_000;

/// Remaining material as a radius over a grid of (X, A) cells.
#[derive(Debug, Clone)]
pub struct HeightStock {
    rotary: RotaryMapping,
    x0: f64,
    dx: f64,
    nx: usize,
    na: usize,
    /// +1 or -1: the sign of the across coordinate of a point turned by a
    /// positive angle difference.
    across_sign: f64,
    top: Vec<f32>,
    r_max: f64,
    /// The smallest radius cells are expected to matter at, which bounds how
    /// wide an angular window a footprint query visits.
    r_lo: f64,
}

impl HeightStock {
    /// The field for `base` over `x0..x1` with cells about `cell` mm across
    /// (larger when that would pass [`MAX_CELLS`]). `r_top` is the largest
    /// radius of any material, and `r_lo` the smallest radius a query will
    /// care about (the lowest level the roughing reaches).
    pub fn from_stock(
        base: &dyn StockModel,
        rotary: RotaryMapping,
        x_range: (f64, f64),
        r_top: f64,
        r_lo: f64,
        cell: f64,
    ) -> Result<Self> {
        let (x0, x1) = x_range;
        let length = x1 - x0;
        if !(length > 0.0 && r_top > 0.0 && cell > 0.0) || !length.is_finite() {
            return Err(PathError::Strategy(
                "the roughing field needs a positive length, radius and cell size".to_owned(),
            ));
        }
        let circumference = std::f64::consts::TAU * r_top;
        let mut cell = cell;
        while (length / cell).ceil() * (circumference / cell).ceil() > MAX_CELLS as f64 {
            cell *= 1.25;
        }
        let nx = (length / cell).ceil().max(1.0) as usize;
        let na = (circumference / cell).ceil().max(8.0) as usize;
        let dx = length / nx as f64;

        // Which way a positive angle difference moves a point across.
        let probe = rotary.to_axis(
            rotary.to_model(
                AxisPosition {
                    along: 0.0,
                    across: 0.0,
                    height: 1.0,
                },
                Degrees(10.0),
            ),
            Degrees(0.0),
        );
        let across_sign = if probe.across >= 0.0 { 1.0 } else { -1.0 };

        let cells: Vec<usize> = (0..nx * na).collect();
        let reach = r_top * 1.05 + 1.0;
        let da = 360.0 / na as f64;
        let top: Vec<f32> = parallel::map_in_order(&cells, |k| {
            let (ix, ia) = (k / na, k % na);
            let a = Degrees((ia as f64 + 0.5) * da);
            let x = x0 + (ix as f64 + 0.5) * dx;
            let origin = rotary.to_model(
                AxisPosition {
                    along: x,
                    across: 0.0,
                    height: reach,
                },
                a,
            );
            let Ok(down) = UnitVec3::new(-rotary.tool_axis(a).get()) else {
                return 0.0;
            };
            match base.ray_entry(origin, down, reach) {
                Some(d) => (reach - d).max(0.0) as f32,
                None => 0.0,
            }
        });
        let r_max = top.iter().fold(0.0_f32, |m, t| m.max(*t)) as f64;
        Ok(Self {
            rotary,
            x0,
            dx,
            nx,
            na,
            across_sign,
            top,
            r_max,
            r_lo: r_lo.max(1e-3),
        })
    }

    /// Cells along X and around the axis.
    pub fn cells(&self) -> (usize, usize) {
        (self.nx, self.na)
    }

    /// The top of the material at cell (`ix`, `ia`), zero where there is none.
    pub fn top_at(&self, ix: usize, ia: usize) -> f64 {
        self.top
            .get(ix * self.na + ia % self.na)
            .copied()
            .map_or(0.0, f64::from)
    }

    /// The cell size along X, mm.
    pub fn cell_x(&self) -> f64 {
        self.dx
    }

    fn da_degrees(&self) -> f64 {
        360.0 / self.na as f64
    }

    /// The cell a model point falls in, if it is over the field.
    fn cell_of(&self, p: Point3) -> Option<(usize, usize, f64)> {
        let r = p.y.hypot(p.z);
        let fx = (p.x - self.x0) / self.dx;
        if !(0.0..self.nx as f64).contains(&fx) {
            return None;
        }
        let phi = p.z.atan2(p.y).to_degrees();
        let a = (self.rotary.sense.sign() * (90.0 - phi)).rem_euclid(360.0);
        let ia = ((a / self.da_degrees()) as usize).min(self.na - 1);
        Some((fx as usize, ia, r))
    }

    /// The largest top among a cell and its four neighbours: the occupied
    /// radius for safety queries.
    fn dilated_top(&self, ix: usize, ia: usize) -> f64 {
        let mut t = self.top_at(ix, ia);
        t = t.max(self.top_at(ix, (ia + 1) % self.na));
        t = t.max(self.top_at(ix, (ia + self.na - 1) % self.na));
        if ix + 1 < self.nx {
            t = t.max(self.top_at(ix + 1, ia));
        }
        if ix > 0 {
            t = t.max(self.top_at(ix - 1, ia));
        }
        t
    }

    /// Calls `visit(cell index, top, height, lateral, distance)` for every
    /// cell with material whose top point lies within `rho` of the tool axis
    /// in the plane across the axis, for a vertical tool at model X `x`,
    /// angle `a` and `across` from the rotary axis. `height` is the top
    /// point's height along the tool axis, `distance` its distance from the
    /// tool axis.
    fn footprint(
        &self,
        x: f64,
        a: f64,
        across: f64,
        rho: f64,
        mut visit: impl FnMut(usize, f64, f64, f64, f64),
    ) {
        let ix_lo = (((x - rho - self.x0) / self.dx).floor().max(0.0)) as usize;
        let ix_hi = ((((x + rho - self.x0) / self.dx).floor()).max(-1.0) + 1.0).min(self.nx as f64)
            as usize;
        let reach = across.abs() + rho;
        let window = if reach >= self.r_lo {
            std::f64::consts::FRAC_PI_2
        } else {
            (reach / self.r_lo).asin()
        };
        let da = self.da_degrees();
        let half = (window.to_degrees() / da).ceil() as i64 + 1;
        let centre = (a / da).floor() as i64;
        for ix in ix_lo..ix_hi {
            let cx = self.x0 + (ix as f64 + 0.5) * self.dx;
            for step in -half..=half {
                let ia = (centre + step).rem_euclid(self.na as i64) as usize;
                let t = self.top_at(ix, ia);
                if t <= 0.0 {
                    continue;
                }
                let mut delta = (ia as f64 + 0.5) * da - a;
                delta = (delta + 180.0).rem_euclid(360.0) - 180.0;
                let d = delta.to_radians();
                let height = t * d.cos();
                if height <= 0.0 {
                    continue;
                }
                let lateral = self.across_sign * t * d.sin();
                let dist = (cx - x).hypot(lateral - across);
                if dist <= rho {
                    visit(ix * self.na + ia, t, height, lateral, dist);
                }
            }
        }
    }

    /// How far the highest material under a vertical tool stands above the
    /// tool's underside: with the tip at `tip` along the tool axis, at model
    /// X `x`, angle `a` (degrees) and `across` from the axis, counting only
    /// the cells wholly inside the tool. Positive when the tool would cut; the
    /// most negative number when no such material is under it.
    pub fn excess(&self, profile: &RevolvedProfile, x: f64, a: f64, across: f64, tip: f64) -> f64 {
        let radius = profile.cutting_radius();
        let da = self.da_degrees().to_radians();
        let dx = self.dx;
        let mut best = f64::NEG_INFINITY;
        self.footprint(x, a, across, radius, |_, top, height, _, dist| {
            // Only a cell the tool can clear counts, the same rule as `cut`:
            // a cell it only reaches in part would stay "to cut" for ever.
            if dist + 0.5 * dx.hypot(top * da) > radius {
                return;
            }
            let under = profile.height_at(dist).unwrap_or(0.0);
            best = best.max(height - (tip + under));
        });
        best
    }

    /// Removes the material a vertical tool cuts with its tip at `tip` along
    /// the tool axis, at model X `x`, angle `a` (degrees) and `across` from
    /// the axis. A cell loses material only where the whole cell is inside
    /// the cutting radius; `margin` is kept as extra material on top of the
    /// tool's underside, for the field's finite resolution.
    pub fn cut(
        &mut self,
        profile: &RevolvedProfile,
        x: f64,
        a: f64,
        across: f64,
        tip: f64,
        margin: f64,
    ) {
        let radius = profile.cutting_radius();
        let da = self.da_degrees().to_radians();
        let mut updates: Vec<(usize, f32)> = Vec::new();
        let across_sign = self.across_sign;
        let dx = self.dx;
        let x0 = self.x0;
        let na = self.na;
        self.footprint(x, a, across, radius, |k, t, height, lateral, dist| {
            // The cell's own size, so only cells wholly inside are cut.
            let half = 0.5 * dx.hypot(t * da);
            if dist + half > radius {
                return;
            }
            let ia = k % na;
            let cx = x0 + ((k / na) as f64 + 0.5) * dx;
            let mut delta = (ia as f64 + 0.5) * da.to_degrees() - a;
            delta = (delta + 180.0).rem_euclid(360.0) - 180.0;
            let cos = delta.to_radians().cos();
            if cos <= 1e-6 {
                return;
            }
            // Where the cell's radial line meets the tool's underside is
            // found at the cell's top and at the tip; the farther of the two
            // from the tool axis keeps the bound on the safe side.
            let low = tip / cos;
            let lat_low = across_sign * low * delta.to_radians().sin();
            let far = (cx - x).hypot(lat_low - across).max(dist).min(radius);
            let Some(under) = profile.height_at(far) else {
                return;
            };
            let bound = (tip + under + margin) / cos;
            let _ = (height, lateral);
            if bound < t {
                updates.push((k, bound as f32));
            }
        });
        for (k, bound) in updates {
            if let Some(t) = self.top.get_mut(k) {
                *t = t.min(bound);
            }
        }
    }

    /// The material's top radius after a cut, largest over the whole field.
    pub fn recompute_max(&mut self) {
        self.r_max = self.top.iter().fold(0.0_f32, |m, t| m.max(*t)) as f64;
    }
}

impl StockModel for HeightStock {
    fn contains(&self, point: Point3) -> bool {
        match self.cell_of(point) {
            Some((ix, ia, r)) => r > 0.0 && r <= self.dilated_top(ix, ia),
            None => false,
        }
    }

    fn ray_entry(&self, origin: Point3, direction: UnitVec3, max_distance: f64) -> Option<f64> {
        if self.contains(origin) {
            return Some(0.0);
        }
        let d = direction.get();
        // The part of the ray inside the cylinder about the rotary axis that
        // holds all the material, and inside the field's length.
        let (mut t0, mut t1) = (0.0_f64, max_distance);
        let a = d.y * d.y + d.z * d.z;
        let b = origin.y * d.y + origin.z * d.z;
        let c =
            origin.y * origin.y + origin.z * origin.z - (self.r_max * 1.05) * (self.r_max * 1.05);
        if a > f64::MIN_POSITIVE {
            let disc = b * b - a * c;
            if disc < 0.0 {
                return None;
            }
            let s = disc.sqrt();
            t0 = t0.max((-b - s) / a);
            t1 = t1.min((-b + s) / a);
        } else if c > 0.0 {
            return None;
        }
        let x_hi = self.x0 + self.nx as f64 * self.dx;
        if d.x.abs() > f64::MIN_POSITIVE {
            let (ta, tb) = ((self.x0 - origin.x) / d.x, (x_hi - origin.x) / d.x);
            t0 = t0.max(ta.min(tb));
            t1 = t1.min(ta.max(tb));
        } else if !(self.x0..=x_hi).contains(&origin.x) {
            return None;
        }
        if t0 > t1 {
            return None;
        }
        let arc = self.r_lo * self.da_degrees().to_radians();
        let step = 0.5 * self.dx.min(arc).max(1e-3);
        let at = |t: f64| origin + d * t;
        let mut previous = t0;
        let mut t = t0;
        loop {
            if self.contains(at(t)) {
                // Bisect between the last clear sample and this one.
                let (mut lo, mut hi) = (previous, t);
                for _ in 0..24 {
                    let mid = 0.5 * (lo + hi);
                    if self.contains(at(mid)) {
                        hi = mid;
                    } else {
                        lo = mid;
                    }
                }
                return Some(hi.max(0.0));
            }
            if t >= t1 {
                return None;
            }
            previous = t;
            t = (t + step).min(t1);
        }
    }

    fn max_radius(&self) -> Option<f64> {
        Some(self.r_max)
    }
}
