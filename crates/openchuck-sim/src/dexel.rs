// SPDX-License-Identifier: AGPL-3.0-or-later
//! The dexel stock (decision record 0022 item 1): material intervals along
//! model Z on a grid of (x, y) columns, in the model (part) frame. The part
//! turns, so the tool's axis tilts in this frame; a cut is the exact tool
//! span along each column's line (any tool, any tilt), subtracted from that
//! column's intervals. Holds walls, undercuts and islands.

use openchuck_path::StockModel;
use openchuck_types::geometry::{Aabb, Point3, UnitVec3, Vec3};
use openchuck_types::job::CancelToken;
use openchuck_types::stock::Stock;
use openchuck_types::{Error, Result};
use rayon::prelude::*;

use crate::span::{ToolSolid, cut_spans, subtract};

/// Poses between looks at the cancel flag while cutting.
const WATCH_EVERY: usize = 16;

/// One tool position for the sweep, in the model frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pose {
    /// Tool tip.
    pub tip: Point3,
    /// Unit tool axis, from the tip towards the spindle.
    pub axis: Vec3,
}

/// A pose prepared for the sweep: an orthonormal tool frame and a bounding
/// box, in the model frame, of the part of the tool that can meet material.
/// Plain arrays, so the hot loop needs no calls into other crates.
#[derive(Debug, Clone, Copy)]
struct Prepared {
    tip: [f64; 3],
    u: [f64; 3],
    v: [f64; 3],
    w: [f64; 3],
    lo: [f64; 3],
    hi: [f64; 3],
}

impl Prepared {
    /// `reach` is the largest distance of any material from the rotary (X)
    /// axis: the tool beyond it cuts nothing, so a long flute pointing away
    /// from the part does not widen the box.
    fn new(pose: &Pose, tool: &ToolSolid, reach: f64) -> Option<Self> {
        let w = [pose.axis.x, pose.axis.y, pose.axis.z];
        let tip = [pose.tip.x, pose.tip.y, pose.tip.z];
        let helper = if w[0].abs() < 0.9 {
            [1.0, 0.0, 0.0]
        } else {
            [0.0, 1.0, 0.0]
        };
        let u = unit(cross(helper, w));
        let v = cross(w, u);
        let r = tool.radius();
        // The axis segment tip + t w, 0 <= t <= top, within reach + r of the
        // X axis: |yz(tip) + t yz(w)|^2 <= (reach + r)^2.
        let (mut t0, mut t1) = (0.0, tool.top());
        let limit = reach + r;
        let qa = w[1] * w[1] + w[2] * w[2];
        let qb = tip[1] * w[1] + tip[2] * w[2];
        let qc = tip[1] * tip[1] + tip[2] * tip[2] - limit * limit;
        if qa < 1e-18 {
            if qc > 0.0 {
                return None;
            }
        } else {
            let disc = qb * qb - qa * qc;
            if disc < 0.0 {
                return None;
            }
            let sq = disc.sqrt();
            t0 = f64::max(t0, (-qb - sq) / qa);
            t1 = f64::min(t1, (-qb + sq) / qa);
        }
        if t0 > t1 {
            return None;
        }
        let a = [tip[0] + t0 * w[0], tip[1] + t0 * w[1], tip[2] + t0 * w[2]];
        let b = [tip[0] + t1 * w[0], tip[1] + t1 * w[1], tip[2] + t1 * w[2]];
        // A cylinder of radius r about that segment bounds the tool there.
        let ext = |c: f64| r * (1.0 - c * c).max(0.0).sqrt();
        let e = [ext(w[0]), ext(w[1]), ext(w[2])];
        let lo = [
            a[0].min(b[0]) - e[0],
            a[1].min(b[1]) - e[1],
            a[2].min(b[2]) - e[2],
        ];
        let hi = [
            a[0].max(b[0]) + e[0],
            a[1].max(b[1]) + e[1],
            a[2].max(b[2]) + e[2],
        ];
        Some(Self {
            tip,
            u,
            v,
            w,
            lo,
            hi,
        })
    }

    /// The column line (x, y, s) in the tool frame: origin and unit direction.
    fn line(&self, x: f64, y: f64) -> ([f64; 3], [f64; 3]) {
        let rel = [x - self.tip[0], y - self.tip[1], -self.tip[2]];
        let dot = |a: [f64; 3]| rel[0] * a[0] + rel[1] * a[1] + rel[2] * a[2];
        (
            [dot(self.u), dot(self.v), dot(self.w)],
            [self.u[2], self.v[2], self.w[2]],
        )
    }
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn unit(v: [f64; 3]) -> [f64; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l > 0.0 {
        [v[0] / l, v[1] / l, v[2] / l]
    } else {
        v
    }
}

/// Material intervals along model Z on an (x, y) grid of columns.
///
/// Column `(i, j)` stands at the centre of cell `i` along X and `j` along Y;
/// its intervals are sorted and disjoint. The cell sizes `cx` and `cy` are
/// the requested resolution, shrunk slightly so a whole number of cells
/// spans the stock.
#[derive(Debug, Clone, PartialEq)]
pub struct DexelStock {
    x0: f64,
    y0: f64,
    cx: f64,
    cy: f64,
    nx: usize,
    ny: usize,
    cols: Vec<Vec<[f64; 2]>>,
    reach: f64,
}

/// The largest grid a stock may have, in columns (a guard against a cell far
/// too fine for the stock; 0.025 mm over a 100 x 100 mm box is 16 million).
pub const MAX_COLUMNS: usize = 64_000_000;

impl DexelStock {
    /// The stock of a setup at resolution `cell`. `part_bounds` resolves
    /// bounding-box stock; imported stock (SET-04, P2) is refused.
    pub fn from_stock(stock: &Stock, part_bounds: Option<Aabb>, cell: f64) -> Result<Self> {
        stock.validate()?;
        match stock {
            Stock::Cylinder {
                diameter,
                length,
                x_start,
            } => {
                let r = 0.5 * diameter.0;
                Self::grid((x_start.0, x_start.0 + length.0), (-r, r), cell, |_, y| {
                    if y.abs() < r {
                        let z = (r * r - y * y).sqrt();
                        vec![[-z, z]]
                    } else {
                        Vec::new()
                    }
                })
            }
            Stock::Box { bounds } => Self::of_box(*bounds, cell),
            Stock::OffsetBoundingBox { offset } => {
                let b = part_bounds.ok_or_else(|| {
                    Error::invalid(
                        "stock",
                        "bounding-box stock needs the part, and the setup has none",
                    )
                })?;
                let d = Vec3::new(offset.0, offset.0, offset.0);
                Self::of_box(
                    Aabb {
                        min: b.min + -d,
                        max: b.max + d,
                    },
                    cell,
                )
            }
            _ => Err(Error::invalid(
                "stock",
                "imported stock is not supported yet (SET-04)",
            )),
        }
    }

    fn of_box(b: Aabb, cell: f64) -> Result<Self> {
        b.validate("stock.bounds")?;
        Self::grid((b.min.x, b.max.x), (b.min.y, b.max.y), cell, |_, _| {
            vec![[b.min.z, b.max.z]]
        })
    }

    fn grid(
        (xa, xb): (f64, f64),
        (ya, yb): (f64, f64),
        cell: f64,
        column: impl Fn(f64, f64) -> Vec<[f64; 2]>,
    ) -> Result<Self> {
        if !(cell.is_finite() && cell > 0.0) {
            return Err(Error::invalid(
                "simulation.cell",
                format!("{cell} must be a finite length greater than zero"),
            ));
        }
        let count = |len: f64| ((len / cell).ceil().max(1.0)) as usize;
        let (nx, ny) = (count(xb - xa), count(yb - ya));
        if nx.saturating_mul(ny) > MAX_COLUMNS {
            return Err(Error::invalid(
                "simulation.cell",
                format!(
                    "a {cell} mm cell needs {nx} x {ny} columns for this stock, more than the \
                     {MAX_COLUMNS} supported; choose a coarser resolution"
                ),
            ));
        }
        let (cx, cy) = ((xb - xa) / nx as f64, (yb - ya) / ny as f64);
        let (cx, cy) = (
            if cx > 0.0 { cx } else { cell },
            if cy > 0.0 { cy } else { cell },
        );
        let mut cols = Vec::with_capacity(nx * ny);
        for i in 0..nx {
            let x = xa + (i as f64 + 0.5) * cx;
            for j in 0..ny {
                cols.push(column(x, ya + (j as f64 + 0.5) * cy));
            }
        }
        let mut stock = Self {
            x0: xa,
            y0: ya,
            cx,
            cy,
            nx,
            ny,
            cols,
            reach: 0.0,
        };
        stock.reach = stock.max_radius_of_material() + cx.hypot(cy);
        Ok(stock)
    }

    /// A bound on the distance of any material from the rotary (X) axis,
    /// set when the stock is made (material only ever shrinks): the farthest
    /// column centre with material, plus a cell.
    pub fn reach(&self) -> f64 {
        self.reach
    }

    /// Cell size along X and Y.
    pub fn cell(&self) -> (f64, f64) {
        (self.cx, self.cy)
    }

    /// Number of columns along X and Y.
    pub fn columns(&self) -> (usize, usize) {
        (self.nx, self.ny)
    }

    /// The (x, y) of column `(i, j)`'s centre.
    pub fn column_centre(&self, i: usize, j: usize) -> (f64, f64) {
        (
            self.x0 + (i as f64 + 0.5) * self.cx,
            self.y0 + (j as f64 + 0.5) * self.cy,
        )
    }

    /// The material intervals of column `(i, j)`, sorted, low to high.
    pub fn column(&self, i: usize, j: usize) -> &[[f64; 2]] {
        if i >= self.nx || j >= self.ny {
            return &[];
        }
        self.cols.get(i * self.ny + j).map_or(&[], |c| c.as_slice())
    }

    /// The column whose cell holds (x, y), if any.
    pub fn column_at(&self, x: f64, y: f64) -> Option<(usize, usize)> {
        let fi = (x - self.x0) / self.cx;
        let fj = (y - self.y0) / self.cy;
        if !(fi >= 0.0 && fj >= 0.0 && fi < self.nx as f64 && fj < self.ny as f64) {
            return None;
        }
        Some((fi as usize, fj as usize))
    }

    /// The grid's extent in X and Y, and the material's in Z (empty stock
    /// gives an empty Z range at zero).
    pub fn bounds(&self) -> Aabb {
        let (mut zlo, mut zhi) = (f64::INFINITY, f64::NEG_INFINITY);
        for c in &self.cols {
            if let (Some(first), Some(last)) = (c.first(), c.last()) {
                zlo = zlo.min(first[0]);
                zhi = zhi.max(last[1]);
            }
        }
        if zlo > zhi {
            (zlo, zhi) = (0.0, 0.0);
        }
        Aabb {
            min: Point3::new(self.x0, self.y0, zlo),
            max: Point3::new(
                self.x0 + self.nx as f64 * self.cx,
                self.y0 + self.ny as f64 * self.cy,
                zhi,
            ),
        }
    }

    /// The material volume in cubic millimetres, added in column order.
    pub fn volume(&self) -> f64 {
        self.cols.iter().map(|c| column_length(c)).sum::<f64>() * self.cx * self.cy
    }

    /// Material volume inside the box `region`, counting each column by its
    /// centre.
    pub fn region_volume(&self, region: &Aabb) -> f64 {
        let mut v = 0.0;
        for i in 0..self.nx {
            for j in 0..self.ny {
                let (x, y) = self.column_centre(i, j);
                if x < region.min.x || x >= region.max.x || y < region.min.y || y >= region.max.y {
                    continue;
                }
                for iv in self.column(i, j) {
                    v += (iv[1].min(region.max.z) - iv[0].max(region.min.z)).max(0.0);
                }
            }
        }
        v * self.cx * self.cy
    }

    /// Releases the capacity the columns hold beyond their material, so the
    /// stock holds, and [`DexelStock::memory_bytes`] reports, the same as a
    /// copy of it, however it was cut. The material is unchanged.
    pub fn shrink_to_fit(&mut self) {
        for column in &mut self.cols {
            column.shrink_to_fit();
        }
        self.cols.shrink_to_fit();
    }

    /// Bytes held by the stock, an estimate from the vectors' capacities.
    pub fn memory_bytes(&self) -> usize {
        self.cols
            .iter()
            .map(|c| c.capacity() * std::mem::size_of::<[f64; 2]>())
            .sum::<usize>()
            + self.cols.capacity() * std::mem::size_of::<Vec<[f64; 2]>>()
            + std::mem::size_of::<Self>()
    }

    /// A 64-bit FNV-1a hash of the stored state, for determinism checks
    /// (NFR-06): equal stocks give equal hashes on any thread count.
    pub fn state_hash(&self) -> u64 {
        let mut h = FNV_START;
        for x in [self.x0, self.y0, self.cx, self.cy] {
            fnv(&mut h, &x.to_bits().to_le_bytes());
        }
        for c in &self.cols {
            fnv(&mut h, &(c.len() as u64).to_le_bytes());
            for iv in c {
                fnv(&mut h, &iv[0].to_bits().to_le_bytes());
                fnv(&mut h, &iv[1].to_bits().to_le_bytes());
            }
        }
        h
    }

    /// Cuts the stock with `tool` at every pose, in parallel by rows of
    /// columns on the caller's rayon pool. Each column is cut by the poses in
    /// order, alone, so the result does not depend on the thread count, and
    /// because span endpoints depend only on the pose and the column, not on
    /// the order. Returns the volume removed.
    pub fn cut(&mut self, tool: &ToolSolid, poses: &[Pose]) -> f64 {
        self.cut_watching(tool, poses, None)
    }

    /// [`DexelStock::cut`] that gives up soon after `cancel` is cancelled and
    /// returns [`Error::Cancelled`]. A batch of poses can take over a second
    /// to cut, so the flag is watched while cutting, not only between
    /// batches (UI-09). After an error the stock is partly cut and must be
    /// dropped.
    pub fn cut_cancellable(
        &mut self,
        tool: &ToolSolid,
        poses: &[Pose],
        cancel: &CancelToken,
    ) -> Result<f64> {
        let removed = self.cut_watching(tool, poses, Some(cancel));
        cancel.check()?;
        Ok(removed)
    }

    fn cut_watching(
        &mut self,
        tool: &ToolSolid,
        poses: &[Pose],
        cancel: Option<&CancelToken>,
    ) -> f64 {
        if poses.is_empty() {
            return 0.0;
        }
        let reach = self.reach();
        let prepared: Vec<Prepared> = poses
            .iter()
            .filter_map(|p| Prepared::new(p, tool, reach))
            .collect();
        let (x0, y0, cx, cy, ny) = (self.x0, self.y0, self.cx, self.cy, self.ny);
        let march = cx.min(cy);
        let removed: Vec<f64> = self
            .cols
            .par_chunks_mut(ny.max(1))
            .enumerate()
            .map(|(i, row)| {
                let x = x0 + (i as f64 + 0.5) * cx;
                let mut removed = 0.0;
                let mut scratch: Vec<[f64; 2]> = Vec::new();
                for (n, p) in prepared.iter().enumerate() {
                    // A relaxed load every few poses: cheap beside the cut.
                    if n % WATCH_EVERY == 0 && cancel.is_some_and(CancelToken::is_cancelled) {
                        break;
                    }
                    if x < p.lo[0] || x > p.hi[0] {
                        continue;
                    }
                    let j0 = ((p.lo[1] - y0) / cy - 0.5).ceil().max(0.0);
                    let j1 = ((p.hi[1] - y0) / cy - 0.5).floor();
                    if j1 < j0 {
                        continue;
                    }
                    let (j0, j1) = (j0 as usize, (j1 as usize).min(ny.saturating_sub(1)));
                    for (j, col) in row.iter_mut().enumerate().take(j1 + 1).skip(j0) {
                        let (Some(first), Some(last)) = (col.first(), col.last()) else {
                            continue;
                        };
                        if first[0] > p.hi[2] || last[1] < p.lo[2] {
                            continue;
                        }
                        let y = y0 + (j as f64 + 0.5) * cy;
                        // The column line (x, y, s) in the tool frame.
                        let (o, d) = p.line(x, y);
                        // Only where the column still holds material: the
                        // result depends on the column's state, but each
                        // column takes the poses in the same order on any
                        // thread count, so it stays deterministic (NFR-06).
                        let (s0, s1) = (p.lo[2].max(first[0]), p.hi[2].min(last[1]));
                        let spans = cut_spans(tool, o, d, s0, s1, march);
                        if spans.is_empty() {
                            continue;
                        }
                        let before = column_length(col);
                        scratch.clear();
                        subtract(col, spans.as_slice(), &mut scratch);
                        if scratch.as_slice() != col.as_slice() {
                            removed += before - column_length(&scratch);
                            col.clone_from(&scratch);
                        }
                    }
                }
                removed
            })
            .collect();
        removed.iter().sum::<f64>() * cx * cy
    }

    /// True when the tool at `pose` would touch material: the cut test
    /// without the subtraction (the COL-05 sketch in decision record 0022
    /// item 5).
    pub fn touches(&self, tool: &ToolSolid, pose: &Pose) -> bool {
        let Some(p) = Prepared::new(pose, tool, self.reach()) else {
            return false;
        };
        let march = self.cx.min(self.cy);
        for i in 0..self.nx {
            let (x, _) = self.column_centre(i, 0);
            if x < p.lo[0] || x > p.hi[0] {
                continue;
            }
            for j in 0..self.ny {
                let (_, y) = self.column_centre(i, j);
                if y < p.lo[1] || y > p.hi[1] {
                    continue;
                }
                let col = self.column(i, j);
                if col.is_empty() {
                    continue;
                }
                let (o, d) = p.line(x, y);
                let spans = cut_spans(tool, o, d, p.lo[2], p.hi[2], march);
                for &(a, b) in spans.as_slice() {
                    if col.iter().any(|iv| iv[0] < b && iv[1] > a) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// True when the model-frame point lies in material.
    pub fn contains_point(&self, p: Point3) -> bool {
        self.column_at(p.x, p.y).is_some_and(|(i, j)| {
            self.column(i, j)
                .iter()
                .any(|iv| p.z >= iv[0] && p.z <= iv[1])
        })
    }

    /// The distance from `p` to the nearest material (`material` true) or to
    /// the nearest empty space (false), each column taken as a prism one
    /// cell across; outside the grid is empty. Searched ring by ring around
    /// `p`'s column, no further than `max`; `None` when nothing is that close.
    pub fn distance_to(&self, p: Point3, material: bool, max: f64) -> Option<f64> {
        let (gx1, gy1) = (
            self.x0 + self.nx as f64 * self.cx,
            self.y0 + self.ny as f64 * self.cy,
        );
        let mut best = f64::INFINITY;
        if !material {
            let inside_grid = p.x >= self.x0 && p.x <= gx1 && p.y >= self.y0 && p.y <= gy1;
            best = if inside_grid {
                (p.x - self.x0)
                    .min(gx1 - p.x)
                    .min(p.y - self.y0)
                    .min(gy1 - p.y)
            } else {
                0.0
            };
        }
        let c = self.cx.min(self.cy);
        let ic = ((p.x - self.x0) / self.cx).floor() as i64;
        let jc = ((p.y - self.y0) / self.cy).floor() as i64;
        let rings = (max / c).ceil() as i64 + 1;
        let (nx, ny) = (self.nx as i64, self.ny as i64);
        let visit = |i: i64, j: i64, best: &mut f64| {
            if i < 0 || j < 0 || i >= nx || j >= ny {
                return;
            }
            let (xa, ya) = (self.x0 + i as f64 * self.cx, self.y0 + j as f64 * self.cy);
            let dx = (xa - p.x).max(p.x - (xa + self.cx)).max(0.0);
            let dy = (ya - p.y).max(p.y - (ya + self.cy)).max(0.0);
            let dl2 = dx * dx + dy * dy;
            if dl2 >= *best * *best {
                return;
            }
            if let Some(dz) = column_gap(self.column(i as usize, j as usize), p.z, material) {
                *best = best.min((dl2 + dz * dz).sqrt());
            }
        };
        for k in 0..=rings {
            let bound = (k - 1).max(0) as f64 * c;
            if bound > max || (k > 0 && bound >= best) {
                break;
            }
            if k == 0 {
                visit(ic, jc, &mut best);
                continue;
            }
            for i in (ic - k)..=(ic + k) {
                visit(i, jc - k, &mut best);
                visit(i, jc + k, &mut best);
            }
            for j in (jc - k + 1)..(jc + k) {
                visit(ic - k, j, &mut best);
                visit(ic + k, j, &mut best);
            }
        }
        (best <= max).then_some(best)
    }

    /// The largest distance from the rotary axis of any column centre with
    /// material.
    pub fn max_radius_of_material(&self) -> f64 {
        let mut best: f64 = 0.0;
        for i in 0..self.nx {
            for j in 0..self.ny {
                let (_, y) = self.column_centre(i, j);
                for iv in self.column(i, j) {
                    let z = iv[0].abs().max(iv[1].abs());
                    best = best.max(y.hypot(z));
                }
            }
        }
        best
    }

    /// Distance along the ray `origin + t direction` to the first material,
    /// no further than `max_distance`. Exact along Z; otherwise the ray is
    /// marched at a quarter cell and the crossing bisected.
    pub fn first_material(
        &self,
        origin: Point3,
        direction: Vec3,
        max_distance: f64,
    ) -> Option<f64> {
        if self.contains_point(origin) {
            return Some(0.0);
        }
        if direction.x.abs() < 1e-15 && direction.y.abs() < 1e-15 {
            let (i, j) = self.column_at(origin.x, origin.y)?;
            let dz = direction.z;
            let mut best: Option<f64> = None;
            for iv in self.column(i, j) {
                let t = if dz > 0.0 {
                    (iv[0] - origin.z) / dz
                } else {
                    (iv[1] - origin.z) / dz
                };
                if t >= 0.0 && t <= max_distance {
                    best = Some(best.map_or(t, |b: f64| b.min(t)));
                }
            }
            return best;
        }
        let b = self.bounds();
        let (t_in, t_out) = slab(origin, direction, &b)?;
        let start = t_in.max(0.0);
        let end = t_out.min(max_distance);
        if start > end {
            return None;
        }
        let at = |t: f64| origin + direction * t;
        let step = 0.25 * self.cx.min(self.cy);
        let mut prev = start;
        if self.contains_point(at(start)) {
            return Some(start);
        }
        let n = ((end - start) / step).ceil().max(1.0) as usize;
        for k in 1..=n {
            let t = (start + k as f64 * step).min(end);
            if self.contains_point(at(t)) {
                let (mut out_t, mut in_t) = (prev, t);
                for _ in 0..48 {
                    let m = 0.5 * (out_t + in_t);
                    if self.contains_point(at(m)) {
                        in_t = m;
                    } else {
                        out_t = m;
                    }
                }
                return Some(in_t);
            }
            prev = t;
        }
        None
    }
}

/// The distance along a column from height `z` to its nearest material
/// (`material` true; `None` for an empty column) or to its nearest gap.
fn column_gap(col: &[[f64; 2]], z: f64, material: bool) -> Option<f64> {
    if material {
        col.iter()
            .map(|iv| (iv[0] - z).max(z - iv[1]).max(0.0))
            .reduce(f64::min)
    } else {
        Some(
            col.iter()
                .find(|iv| z >= iv[0] && z <= iv[1])
                .map_or(0.0, |iv| (z - iv[0]).min(iv[1] - z)),
        )
    }
}

fn column_length(c: &[[f64; 2]]) -> f64 {
    c.iter().map(|iv| iv[1] - iv[0]).sum()
}

/// The ray's parameter range inside the box, if it meets it.
fn slab(o: Point3, d: Vec3, b: &Aabb) -> Option<(f64, f64)> {
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
    (t0 <= t1).then_some((t0, t1))
}

const FNV_START: u64 = 0xcbf2_9ce4_8422_2325;

fn fnv(h: &mut u64, bytes: &[u8]) {
    for b in bytes {
        *h ^= u64::from(*b);
        *h = h.wrapping_mul(0x0100_0000_01b3);
    }
}

/// The dexel stock as the pathing library sees it (architecture section 5.2):
/// later operations generate against the material actually left (SIM-06).
impl StockModel for DexelStock {
    fn contains(&self, point: Point3) -> bool {
        self.contains_point(point)
    }

    fn ray_entry(&self, origin: Point3, direction: UnitVec3, max_distance: f64) -> Option<f64> {
        self.first_material(origin, direction.get(), max_distance)
    }

    fn max_radius(&self) -> Option<f64> {
        Some(self.max_radius_of_material())
    }

    /// The X range of the columns that hold material, to their cell edges
    /// (OP-24; decision record 0212 part B).
    fn x_extent(&self) -> Option<(f64, f64)> {
        let with_material = |i: usize| (0..self.ny).any(|j| !self.column(i, j).is_empty());
        let first = (0..self.nx).find(|&i| with_material(i))?;
        let last = (0..self.nx).rev().find(|&i| with_material(i))?;
        Some((
            self.x0 + first as f64 * self.cx,
            self.x0 + (last + 1) as f64 * self.cx,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openchuck_types::units::Millimetres;

    #[test]
    fn req_op_24_the_stock_ends_are_where_its_material_is() {
        use openchuck_path::StockModel;
        let stock = Stock::Cylinder {
            diameter: Millimetres(20.0),
            length: Millimetres(30.0),
            x_start: Millimetres(-5.0),
        };
        let dexel = DexelStock::from_stock(&stock, None, 0.5).unwrap();
        let (lo, hi) = dexel.x_extent().unwrap();
        assert!(
            (lo + 5.0).abs() <= 0.5 && (hi - 25.0).abs() <= 0.5,
            "{lo} {hi}"
        );
    }

    #[test]
    fn req_sim_03_a_new_cylinder_holds_its_volume_within_the_resolution() {
        let stock = Stock::Cylinder {
            diameter: Millimetres(20.0),
            length: Millimetres(30.0),
            x_start: Millimetres(0.0),
        };
        let d = DexelStock::from_stock(&stock, None, 0.1).unwrap();
        let exact = std::f64::consts::PI * 100.0 * 30.0;
        // Columns are counted by their centre: the error is a strip of about
        // one cell around the rim.
        assert!((d.volume() - exact).abs() < 2.0 * std::f64::consts::PI * 10.0 * 30.0 * 0.1);
        assert!(d.contains_point(Point3::new(5.0, 0.0, 9.9)));
        assert!(!d.contains_point(Point3::new(5.0, 0.0, 10.1)));
        let down = UnitVec3::new(Vec3::new(0.0, 0.0, -1.0)).unwrap();
        let t = d
            .ray_entry(Point3::new(5.0, 0.05, 20.0), down, 50.0)
            .unwrap();
        assert!((t - 10.0).abs() < 0.01, "{t}");
        assert!(d.max_radius().unwrap() <= 10.0 + 1e-9);
    }

    #[test]
    fn req_sim_03_slanted_ray_finds_the_box() {
        let stock = Stock::Box {
            bounds: Aabb {
                min: Point3::new(0.0, 0.0, 0.0),
                max: Point3::new(10.0, 10.0, 10.0),
            },
        };
        let d = DexelStock::from_stock(&stock, None, 0.1).unwrap();
        let s = std::f64::consts::FRAC_1_SQRT_2;
        let t = d
            .first_material(Point3::new(-5.0, 5.0, 15.0), Vec3::new(s, 0.0, -s), 100.0)
            .unwrap();
        // Enters the top at z = 10, 5 mm down: t = 5 sqrt 2.
        assert!((t - 5.0 * 2f64.sqrt()).abs() < 0.05, "{t}");
    }
}
