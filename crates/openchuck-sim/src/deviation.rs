// SPDX-License-Identifier: AGPL-3.0-or-later
//! The simulated stock against the target model (SIM-05).
//!
//! The target's surface is sampled at the simulation spacing. At each sample
//! the deviation is the closest distance to the stock's boundary, each
//! column taken as a prism one cell across: when the point is in material,
//! the distance to the nearest empty space (remaining, positive); when it is
//! not, the distance to the nearest material (overcut, negative). The closest
//! distance, not the distance along the normal: at an edge of the part the
//! normal can run along the cut surface, where the dexels' half-cell
//! quantisation across the column would read as a deep overcut. Values are
//! exact to the stored stock, whose own error is up to one cell (verification
//! plan section 4.1), so an overcut counts as one only beyond the cell.

use openchuck_types::geometry::{Point3, Vec3};
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::{Error, Result};
use rayon::prelude::*;

use crate::dexel::DexelStock;

/// Samples per parallel chunk; fixed so results never depend on the thread count.
const CHUNK: usize = 256;

/// How to measure.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeviationOptions {
    /// Largest distance between surface samples, in millimetres.
    pub spacing: f64,
    /// How far to look, in millimetres. A deviation this large or more is
    /// reported as this value, marked as at least.
    pub max_distance: f64,
    /// An overcut deeper than this is reported as one, in millimetres.
    pub overcut_limit: f64,
    /// How many separate overcut places to list.
    pub max_places: usize,
    /// Places closer than this are one place, in millimetres.
    pub place_separation: f64,
}

impl DeviationOptions {
    /// The defaults for a stock simulated at `cell`: samples one cell apart,
    /// a 3 mm search, overcuts beyond one cell, ten places 2 mm apart.
    pub fn for_cell(cell: f64) -> Self {
        Self {
            spacing: cell,
            max_distance: 3.0,
            overcut_limit: cell,
            max_places: 10,
            place_separation: 2.0,
        }
    }
}

/// One surface sample and its deviation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeviationPoint {
    /// The point on the target's surface.
    pub point: Point3,
    /// The outward unit normal there.
    pub normal: Vec3,
    /// Material left above the surface (positive) or cut below it (negative).
    pub value: f64,
    /// True when the search reached `max_distance`: the value is a bound.
    pub at_least: bool,
}

/// The result of a comparison, as plain data for reports and the viewport.
#[derive(Debug, Clone, PartialEq)]
pub struct Deviation {
    /// Every sample, in triangle order.
    pub points: Vec<DeviationPoint>,
    /// The sample with the most material left, if any is left.
    pub worst_remaining: Option<DeviationPoint>,
    /// The deepest overcut, if any point is overcut at all.
    pub worst_overcut: Option<DeviationPoint>,
    /// Samples overcut beyond the limit.
    pub overcut_count: usize,
    /// The deepest overcut of each separate place beyond the limit, deepest first.
    pub overcut_places: Vec<DeviationPoint>,
    /// The limit applied.
    pub overcut_limit: f64,
}

/// Compares `stock` with the closed surface `target`.
pub fn compare(
    stock: &DexelStock,
    target: &TriangleMesh,
    options: &DeviationOptions,
) -> Result<Deviation> {
    target.validate()?;
    for (name, v) in [
        ("spacing", options.spacing),
        ("max_distance", options.max_distance),
    ] {
        if !(v.is_finite() && v > 0.0) {
            return Err(Error::invalid(
                format!("deviation.{name}"),
                format!("{v} must be a finite length greater than zero"),
            ));
        }
    }
    let samples = surface_samples(target, options.spacing);
    let points: Vec<DeviationPoint> = samples
        .par_chunks(CHUNK)
        .map(|chunk| {
            chunk
                .iter()
                .map(|&(p, n)| measure(stock, p, n, options))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>()
        .into_iter()
        .flatten()
        .collect();
    let mut worst_remaining: Option<DeviationPoint> = None;
    let mut worst_overcut: Option<DeviationPoint> = None;
    for p in &points {
        if p.value > 0.0 && worst_remaining.is_none_or(|w| p.value > w.value) {
            worst_remaining = Some(*p);
        }
        if p.value < 0.0 && worst_overcut.is_none_or(|w| p.value < w.value) {
            worst_overcut = Some(*p);
        }
    }
    let mut over: Vec<DeviationPoint> = points
        .iter()
        .copied()
        .filter(|p| -p.value > options.overcut_limit)
        .collect();
    let overcut_count = over.len();
    // Deepest first; ties in surface order, so the list is deterministic.
    over.sort_by(|a, b| a.value.total_cmp(&b.value));
    let mut places: Vec<DeviationPoint> = Vec::new();
    for p in over {
        if places.len() >= options.max_places {
            break;
        }
        if places
            .iter()
            .all(|q| q.point.distance(p.point) >= options.place_separation)
        {
            places.push(p);
        }
    }
    Ok(Deviation {
        points,
        worst_remaining,
        worst_overcut,
        overcut_count,
        overcut_places: places,
        overcut_limit: options.overcut_limit,
    })
}

/// The material left on the part measured against a stock to leave: how much
/// of the surface is at it, above it and below it (work breakdown slice 14
/// item 7; decision record 0033).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Leftover {
    /// The stock to leave the operation asked for.
    pub leave: f64,
    /// How far from it a sample may be and still count as at it.
    pub tolerance: f64,
    /// Samples measured.
    pub samples: usize,
    /// The mean material left.
    pub mean: f64,
    /// The least material left (negative where the stock is cut below the
    /// surface).
    pub least: f64,
    /// The most material left.
    pub most: f64,
    /// Samples with less than `leave - tolerance` left: cut into the stock to
    /// leave.
    pub below: usize,
    /// Samples with more than `leave + tolerance` left: not roughed down to it
    /// (a corner the tool cannot reach, or too few passes).
    pub above: usize,
    /// The sample with the most material beyond `leave + tolerance`.
    pub worst_above: Option<DeviationPoint>,
}

impl Leftover {
    /// The share of samples at the stock to leave, 0 to 1 (1 when there are
    /// none).
    #[must_use]
    pub fn at_leave(&self) -> f64 {
        if self.samples == 0 {
            1.0
        } else {
            (self.samples - self.below - self.above) as f64 / self.samples as f64
        }
    }
}

impl Deviation {
    /// The material left against `leave`, within `tolerance` (all in mm).
    /// A sample whose search reached its limit counts as above.
    #[must_use]
    pub fn leftover(&self, leave: f64, tolerance: f64) -> Leftover {
        let mut out = Leftover {
            leave,
            tolerance,
            samples: self.points.len(),
            mean: 0.0,
            least: f64::INFINITY,
            most: f64::NEG_INFINITY,
            below: 0,
            above: 0,
            worst_above: None,
        };
        let mut sum = 0.0;
        for p in &self.points {
            sum += p.value;
            out.least = out.least.min(p.value);
            out.most = out.most.max(p.value);
            if p.value < leave - tolerance {
                out.below += 1;
            } else if p.value > leave + tolerance || p.at_least {
                out.above += 1;
                if out.worst_above.is_none_or(|w| p.value > w.value) {
                    out.worst_above = Some(*p);
                }
            }
        }
        if out.samples > 0 {
            out.mean = sum / out.samples as f64;
        } else {
            out.least = 0.0;
            out.most = 0.0;
        }
        out
    }
}

/// The deviation on each triangle of `mesh`: of the samples taken on it, the
/// one furthest from the surface, with the sign of [`DeviationPoint::value`]
/// (positive for material left, negative for overcut), and whether that
/// sample's value is only a bound. `None` for a triangle that has no samples
/// (a degenerate one). `deviation` must be the comparison of `mesh` made with
/// samples no further apart than `spacing` ([`DeviationOptions::spacing`]),
/// since the samples are in triangle order and a triangle's share follows
/// from its size. Fails if the counts do not add up.
pub fn triangle_values(
    deviation: &Deviation,
    mesh: &TriangleMesh,
    spacing: f64,
) -> Result<Vec<Option<(f64, bool)>>> {
    let mut out = Vec::with_capacity(mesh.triangles.len());
    let mut next = 0usize;
    for t in 0..mesh.triangles.len() {
        let Some((_, n, _)) = triangle_subdivision(mesh, t, spacing) else {
            out.push(None);
            continue;
        };
        let count = n * n;
        let samples = deviation.points.get(next..next + count).ok_or_else(|| {
            Error::invalid(
                "deviation",
                "it does not hold the samples of this mesh at this spacing",
            )
        })?;
        next += count;
        let worst = samples
            .iter()
            .fold(None, |best: Option<&DeviationPoint>, p| {
                if best.is_none_or(|b| p.value.abs() > b.value.abs()) {
                    Some(p)
                } else {
                    best
                }
            });
        out.push(worst.map(|p| (p.value, p.at_least)));
    }
    if next != deviation.points.len() {
        return Err(Error::invalid(
            "deviation",
            "it holds more samples than this mesh has at this spacing",
        ));
    }
    Ok(out)
}

/// The unit normal and the subdivision count `n` of triangle `t` (samples are
/// the centroids of an n x n subdivision), with its corners; `None` for a
/// triangle that is degenerate or refers to a missing vertex.
fn triangle_subdivision(
    mesh: &TriangleMesh,
    t: usize,
    spacing: f64,
) -> Option<(Vec3, usize, [Point3; 3])> {
    let [a, b, c] = mesh.triangle(t)?;
    let (ab, ac) = (b - a, c - a);
    let cross = ab.cross(ac);
    let area2 = cross.length();
    if area2.is_nan() || area2 <= 1e-18 {
        return None;
    }
    let normal = cross * (1.0 / area2);
    let longest = ab.length().max(ac.length()).max((c - b).length());
    let n = ((longest / spacing).ceil() as usize).clamp(1, 4096);
    Some((normal, n, [a, b, c]))
}

/// Points on each triangle no further apart than `spacing`: the centroids of
/// an n x n subdivision, with the triangle's unit normal. Degenerate
/// triangles give none.
fn surface_samples(mesh: &TriangleMesh, spacing: f64) -> Vec<(Point3, Vec3)> {
    let mut out = Vec::new();
    for t in 0..mesh.triangles.len() {
        let Some((normal, n, [a, b, c])) = triangle_subdivision(mesh, t, spacing) else {
            continue;
        };
        let (ab, ac) = (b - a, c - a);
        let step = 1.0 / n as f64;
        for i in 0..n {
            for j in 0..n - i {
                // Upright sub-triangle (i, j), (i+1, j), (i, j+1).
                let (u, v) = ((i as f64 + 1.0 / 3.0) * step, (j as f64 + 1.0 / 3.0) * step);
                out.push((a + ab * u + ac * v, normal));
                // Inverted sub-triangle (i+1, j), (i, j+1), (i+1, j+1).
                if i + j + 1 < n {
                    let (u, v) = ((i as f64 + 2.0 / 3.0) * step, (j as f64 + 2.0 / 3.0) * step);
                    out.push((a + ab * u + ac * v, normal));
                }
            }
        }
    }
    out
}

fn measure(stock: &DexelStock, p: Point3, n: Vec3, options: &DeviationOptions) -> DeviationPoint {
    let inside = stock.contains_point(p);
    let max = options.max_distance;
    let (distance, at_least) = stock
        .distance_to(p, !inside, max)
        .map_or((max, true), |d| (d, false));
    DeviationPoint {
        point: p,
        normal: n,
        value: if inside { distance } else { -distance },
        at_least,
    }
}
