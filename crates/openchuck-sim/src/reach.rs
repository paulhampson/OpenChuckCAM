// SPDX-License-Identifier: AGPL-3.0-or-later
//! Material left that no tool can reach (SIM-05, as amended on 1 Nov 2026;
//! issue 478).
//!
//! On this machine every tool axis lies in a plane across the rotary axis
//! (X). A tool whose radius never narrows from its tip upward (the monotone
//! rule of `openchuck_types::tool::profile`, with a shank no narrower than
//! the cutter) is cut by the plane `x = p.x` in one strip that runs from the
//! point it touches out past the stock. So where the empty space beside a
//! sample is closed off from the outside in the sample's own section across
//! X, no such tool can reach it, whatever its size or lean: the sample is
//! *enclosed* (the bore of a tube along the rotary axis, for example).
//!
//! **How.** A grid across X at the pitch (the simulation cell) is cast with
//! one ray along X per grid point through each closed body (the part, and
//! each held region and tab), keeping where it crosses. X is cut into slabs;
//! a grid point is solid in a slab only where one body fills the whole slab
//! there. The empty grid points are flood-filled from the grid's border; a
//! sample whose empty side lands on an empty point the fill never reached is
//! enclosed, one the fill reached is open. Every slab is worked on its own,
//! so the result is the same at any thread count (NFR-06).
//!
//! **Limits, never guessed (NFR-12).** An opening narrower than about two
//! pitches reads as closed, so when a tool of the setup is that small,
//! nothing is called enclosed. A sample facing along X (an end face, which a
//! tool reaches from the end), one whose empty side crosses a wall within
//! the step looked along, or one beside a column whose ray crosses a body an
//! odd number of times (a body that is not closed, treated as empty) is
//! *undecided* or open, never enclosed. Open is a fact about the grid, not a
//! claim that a tool fits: whether one does is not decided here.

use openchuck_types::job::JobContext;
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::{Error, Result};
use rayon::prelude::*;

use crate::deviation::{Deviation, SampleOn};

/// The most grid points along Y or Z: a coarser pitch is used for a part
/// wider than this many cells, so the grid stays within a million points.
const MAX_GRID: f64 = 1000.0;

/// The most slabs along X: wider slabs are used for a longer part. A wider
/// slab only makes fewer points solid, so fewer samples enclosed.
const MAX_SLABS: f64 = 500.0;

/// How much wider than a slab, as a share of it, a body must fill to be
/// solid there.
const SLAB_MARGIN: f64 = 1e-6;

/// Grid-pitch fractions a sample's empty side is looked at, along its
/// normal across X.
const STEPS: [f64; 5] = [0.5, 0.75, 1.0, 1.25, 1.5];

/// A sample whose normal across X is shorter than this faces along X.
const ACROSS: f64 = 0.5;

/// How to classify the material left.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReachOptions {
    /// The grid's pitch, in millimetres: the simulation cell.
    pub pitch: f64,
    /// Material left beyond this is classified, in millimetres: the
    /// tolerance (one cell).
    pub left_above: f64,
    /// The smallest cutting diameter of the setup's tools, in millimetres.
    /// Below two pitches, an opening the grid reads as closed might pass it,
    /// so nothing is called enclosed.
    pub smallest_tool: f64,
}

impl ReachOptions {
    /// The options for a simulation at `cell` with tools no smaller than
    /// `smallest_tool` across.
    #[must_use]
    pub fn for_cell(cell: f64, smallest_tool: f64) -> Self {
        Self {
            pitch: cell,
            left_above: cell,
            smallest_tool,
        }
    }
}

/// What a sample's material is, as far as reaching it goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReachClass {
    /// No material is left there beyond the tolerance, or the sample does
    /// not count on the part (decision 0194).
    NotLeft,
    /// Material is left, closed off from the outside across X: no tool on
    /// this machine reaches it.
    Enclosed,
    /// Material is left, open to the outside across X at the grid's pitch.
    /// Whether a tool fits there is not decided.
    Open,
    /// Material is left and the test cannot tell (see the module's limits).
    Undecided,
}

/// Each sample's class, in the order of [`Deviation::points`].
#[derive(Debug, Clone, PartialEq)]
pub struct Reach {
    /// One per sample.
    pub classes: Vec<ReachClass>,
    /// The grid pitch used, in millimetres: the option's, or coarser for a
    /// part wider than a thousand of them.
    pub pitch: f64,
}

/// The crossings along X of one body's rays, one ray per grid point.
struct Rays {
    /// Where each ray's crossings start in `xs`; one more than the rays.
    start: Vec<usize>,
    /// The crossings, sorted along each ray.
    xs: Vec<f64>,
}

impl Rays {
    /// Whether the body fills `[a, b]` along the ray of grid point `k`. A ray
    /// crossing an odd number of times is of a body that is not closed, and
    /// fills nothing.
    fn fills(&self, k: usize, a: f64, b: f64) -> bool {
        let (Some(&s), Some(&e)) = (self.start.get(k), self.start.get(k + 1)) else {
            return false;
        };
        let Some(xs) = self.xs.get(s..e) else {
            return false;
        };
        xs.len() % 2 == 0
            && xs
                .chunks_exact(2)
                .any(|c| matches!(c, [lo, hi] if *lo <= a && *hi >= b))
    }
}

/// The grid across X.
struct Grid {
    y0: f64,
    z0: f64,
    pitch: f64,
    ny: usize,
    nz: usize,
}

impl Grid {
    /// The grid point holding `(y, z)`, if inside.
    fn at(&self, y: f64, z: f64) -> Option<usize> {
        let i = ((y - self.y0) / self.pitch).floor();
        let j = ((z - self.z0) / self.pitch).floor();
        if i < 0.0 || j < 0.0 || i >= self.ny as f64 || j >= self.nz as f64 {
            return None;
        }
        Some(i as usize * self.nz + j as usize)
    }

    /// The point a grid point's ray runs through: its centre, moved by a
    /// fixed fraction of the pitch so that rays miss the edges of a regular
    /// mesh.
    fn centre(&self, k: usize) -> (f64, f64) {
        let (i, j) = (k / self.nz, k % self.nz);
        (
            self.y0 + (i as f64 + 0.5 + 1.37e-6) * self.pitch,
            self.z0 + (j as f64 + 0.5 + 2.71e-6) * self.pitch,
        )
    }
}

/// Classifies the material left at each sample of `deviation` as enclosed,
/// open or undecided (see the module), against `bodies`: the closed meshes
/// whose union is what stays (the part, and each held region and tab).
///
/// Fails on options that are not finite and positive, and with
/// [`Error::Cancelled`] soon after `context` is cancelled.
pub fn classify_reach(
    bodies: &[&TriangleMesh],
    deviation: &Deviation,
    options: &ReachOptions,
    context: &JobContext<'_>,
) -> Result<Reach> {
    if !(options.pitch.is_finite() && options.pitch > 0.0) {
        return Err(Error::invalid(
            "reach.pitch",
            format!(
                "{} must be a finite length greater than zero",
                options.pitch
            ),
        ));
    }
    for (name, v) in [
        ("left_above", options.left_above),
        ("smallest_tool", options.smallest_tool),
    ] {
        if !(v.is_finite() && v >= 0.0) {
            return Err(Error::invalid(
                format!("reach.{name}"),
                format!("{v} must be a finite length of zero or more"),
            ));
        }
    }
    context.check_cancelled()?;
    let left = |p: &crate::DeviationPoint| {
        p.on == SampleOn::Part && p.counts() && p.value > options.left_above
    };
    let mut classes: Vec<ReachClass> = deviation
        .points
        .iter()
        .map(|p| {
            if left(p) {
                ReachClass::Undecided
            } else {
                ReachClass::NotLeft
            }
        })
        .collect();
    // The bodies' extent across X.
    let mut lo = [f64::INFINITY; 2];
    let mut hi = [f64::NEG_INFINITY; 2];
    for v in bodies.iter().flat_map(|b| b.vertices.iter()) {
        for (k, c) in [v.y, v.z].into_iter().enumerate() {
            if let (Some(l), Some(h)) = (lo.get_mut(k), hi.get_mut(k)) {
                *l = l.min(c);
                *h = h.max(c);
            }
        }
    }
    let [ylo, zlo] = lo;
    let [yhi, zhi] = hi;
    let pitch = if ylo.is_finite() && zlo.is_finite() && yhi.is_finite() && zhi.is_finite() {
        options.pitch.max((yhi - ylo).max(zhi - zlo) / MAX_GRID)
    } else {
        // Nothing to cast against: nothing is enclosed.
        return Ok(Reach {
            classes,
            pitch: options.pitch,
        });
    };
    if options.smallest_tool < 2.0 * pitch || !classes.contains(&ReachClass::Undecided) {
        return Ok(Reach { classes, pitch });
    }
    // Two empty grid points round the bodies, so the border is outside.
    let grid = Grid {
        y0: ylo - 2.0 * pitch,
        z0: zlo - 2.0 * pitch,
        pitch,
        ny: ((yhi - ylo) / pitch).ceil() as usize + 4,
        nz: ((zhi - zlo) / pitch).ceil() as usize + 4,
    };
    let rays: Vec<Rays> = bodies
        .iter()
        .map(|b| cast(b, &grid, context))
        .collect::<Result<_>>()?;
    // The slabs along X over the samples left.
    let leftover: Vec<usize> = (0..classes.len())
        .filter(|i| classes.get(*i) == Some(&ReachClass::Undecided))
        .collect();
    let (x0, x1) = leftover
        .iter()
        .filter_map(|i| deviation.points.get(*i))
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), p| {
            (a.min(p.point.x), b.max(p.point.x))
        });
    let width = pitch.max((x1 - x0) / MAX_SLABS);
    let slab_of = |x: f64| ((x - x0) / width).floor().max(0.0) as usize;
    let mut by_slab: Vec<(usize, usize)> = leftover
        .iter()
        .filter_map(|i| deviation.points.get(*i).map(|p| (slab_of(p.point.x), *i)))
        .collect();
    by_slab.sort_unstable();
    let mut groups: Vec<&[(usize, usize)]> = Vec::new();
    let mut rest = by_slab.as_slice();
    while let Some(&(slab, _)) = rest.first() {
        let n = rest.iter().take_while(|(s, _)| *s == slab).count();
        let (group, tail) = rest.split_at(n);
        groups.push(group);
        rest = tail;
    }
    let decided: Vec<Vec<(usize, ReachClass)>> = groups
        .par_iter()
        .map(|group| {
            context.check_cancelled()?;
            let slab = group.first().map_or(0, |(s, _)| *s);
            // A hair wider than the slab, so a sample on its boundary (the
            // rim of an opening at the part's end) is enclosed only if the
            // slabs on both sides are.
            let a = x0 + slab as f64 * width - SLAB_MARGIN * width;
            let b = x0 + (slab + 1) as f64 * width + SLAB_MARGIN * width;
            let status = fill(&grid, &rays, a, b);
            Ok(group
                .iter()
                .map(|(_, i)| {
                    let class = deviation
                        .points
                        .get(*i)
                        .map_or(ReachClass::Undecided, |p| decide(&grid, &status, p));
                    (*i, class)
                })
                .collect())
        })
        .collect::<Result<_>>()?;
    for (i, class) in decided.into_iter().flatten() {
        if let Some(slot) = classes.get_mut(i) {
            *slot = class;
        }
    }
    Ok(Reach { classes, pitch })
}

/// A grid point's state in one slab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Cell {
    Solid,
    /// Empty and reached from the border.
    Outside,
    /// Empty and closed off from the border.
    Pocket,
}

/// The rays of `body` along X through every grid point.
fn cast(body: &TriangleMesh, grid: &Grid, context: &JobContext<'_>) -> Result<Rays> {
    context.check_cancelled()?;
    // Triangles by the buckets of 16 x 16 grid points their extent across X
    // touches.
    const BUCKET: usize = 16;
    let (by, bz) = (grid.ny.div_ceil(BUCKET), grid.nz.div_ceil(BUCKET));
    let mut buckets: Vec<Vec<u32>> = vec![Vec::new(); by * bz];
    let bucket = |v: f64, origin: f64, n: usize| {
        (((v - origin) / (grid.pitch * BUCKET as f64))
            .floor()
            .max(0.0) as usize)
            .min(n - 1)
    };
    for t in 0..body.triangles.len() {
        let Some([p, q, r]) = body.triangle(t) else {
            continue;
        };
        let (ya, yb) = (p.y.min(q.y).min(r.y), p.y.max(q.y).max(r.y));
        let (za, zb) = (p.z.min(q.z).min(r.z), p.z.max(q.z).max(r.z));
        for i in bucket(ya, grid.y0, by)..=bucket(yb, grid.y0, by) {
            for j in bucket(za, grid.z0, bz)..=bucket(zb, grid.z0, bz) {
                if let Some(list) = buckets.get_mut(i * bz + j) {
                    list.push(u32::try_from(t).unwrap_or(u32::MAX));
                }
            }
        }
    }
    let per: Vec<Vec<f64>> = (0..grid.ny * grid.nz)
        .into_par_iter()
        .with_min_len(4096)
        .map(|k| {
            let (y, z) = grid.centre(k);
            let list = buckets
                .get(bucket(y, grid.y0, by) * bz + bucket(z, grid.z0, bz))
                .map_or(&[][..], Vec::as_slice);
            let mut xs: Vec<f64> = list
                .iter()
                .filter_map(|t| body.triangle(*t as usize))
                .filter_map(|[p, q, r]| {
                    let d = (q.y - p.y) * (r.z - p.z) - (r.y - p.y) * (q.z - p.z);
                    if d.abs() < 1e-18 {
                        return None;
                    }
                    let u = ((y - p.y) * (r.z - p.z) - (r.y - p.y) * (z - p.z)) / d;
                    let v = ((q.y - p.y) * (z - p.z) - (y - p.y) * (q.z - p.z)) / d;
                    (u >= 0.0 && v >= 0.0 && u + v <= 1.0)
                        .then_some(p.x + u * (q.x - p.x) + v * (r.x - p.x))
                })
                .collect();
            xs.sort_by(f64::total_cmp);
            xs
        })
        .collect();
    context.check_cancelled()?;
    let mut start = Vec::with_capacity(per.len() + 1);
    let mut xs = Vec::with_capacity(per.iter().map(Vec::len).sum());
    for ray in per {
        start.push(xs.len());
        xs.extend(ray);
    }
    start.push(xs.len());
    Ok(Rays { start, xs })
}

/// Each grid point's state in the slab `[a, b]`: solid where a body fills
/// the whole slab, then the empty points flood-filled from the border,
/// four-connected.
fn fill(grid: &Grid, rays: &[Rays], a: f64, b: f64) -> Vec<Cell> {
    let n = grid.ny * grid.nz;
    let mut cells: Vec<Cell> = (0..n)
        .map(|k| {
            if rays.iter().any(|r| r.fills(k, a, b)) {
                Cell::Solid
            } else {
                Cell::Pocket
            }
        })
        .collect();
    let mut stack = Vec::new();
    let visit = |k: usize, cells: &mut Vec<Cell>, stack: &mut Vec<usize>| {
        if let Some(c) = cells.get_mut(k)
            && *c == Cell::Pocket
        {
            *c = Cell::Outside;
            stack.push(k);
        }
    };
    for i in 0..grid.ny {
        for j in 0..grid.nz {
            if i == 0 || j == 0 || i + 1 == grid.ny || j + 1 == grid.nz {
                visit(i * grid.nz + j, &mut cells, &mut stack);
            }
        }
    }
    while let Some(k) = stack.pop() {
        let (i, j) = (k / grid.nz, k % grid.nz);
        if i > 0 {
            visit(k - grid.nz, &mut cells, &mut stack);
        }
        if i + 1 < grid.ny {
            visit(k + grid.nz, &mut cells, &mut stack);
        }
        if j > 0 {
            visit(k - 1, &mut cells, &mut stack);
        }
        if j + 1 < grid.nz {
            visit(k + 1, &mut cells, &mut stack);
        }
    }
    cells
}

/// A sample's class from the grid points on its empty side across X: past
/// any solid point it starts in, every point looked at must be empty, and
/// all of them closed off (enclosed) or all reached (open).
fn decide(grid: &Grid, cells: &[Cell], p: &crate::DeviationPoint) -> ReachClass {
    let across = p.normal.y.hypot(p.normal.z);
    if across.is_nan() || across < ACROSS {
        return ReachClass::Undecided;
    }
    let (dy, dz) = (p.normal.y / across, p.normal.z / across);
    let states: Vec<Cell> = STEPS
        .iter()
        .map(|s| {
            let (y, z) = (
                p.point.y + dy * s * grid.pitch,
                p.point.z + dz * s * grid.pitch,
            );
            grid.at(y, z)
                .and_then(|k| cells.get(k).copied())
                .unwrap_or(Cell::Outside)
        })
        .skip_while(|c| *c == Cell::Solid)
        .collect();
    if states.is_empty() || states.contains(&Cell::Solid) {
        ReachClass::Undecided
    } else if states.iter().all(|c| *c == Cell::Pocket) {
        ReachClass::Enclosed
    } else if states.iter().all(|c| *c == Cell::Outside) {
        ReachClass::Open
    } else {
        ReachClass::Undecided
    }
}
