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
use openchuck_types::job::{CancelToken, JobContext, NoProgress};
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

/// What a sample of the target lies on (decision record 0194).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SampleOn {
    /// The part's own surface: always counted.
    #[default]
    Part,
    /// An exposed face of a held region or a retaining tab (SET-09, OP-21):
    /// only an overcut there is counted (a cut into the workholding); the
    /// material left beside it is where the tools keep clear of it.
    Workholding,
    /// A face inside the finished part (the part's end face under a tab, a
    /// held region's face under a tab): not a surface of it, never counted.
    Inside,
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
    /// What the sample lies on, which decides whether it is counted.
    pub on: SampleOn,
}

impl DeviationPoint {
    /// Whether the sample counts in the results: the worst values, the
    /// overcut places, the histogram and the share within tolerance, and the
    /// colours of its triangle. A sample inside the finished part never
    /// does, and one on a held region or tab only where it is overcut
    /// (decision record 0194).
    #[must_use]
    pub fn counts(&self) -> bool {
        match self.on {
            SampleOn::Part => true,
            SampleOn::Workholding => self.value < 0.0,
            SampleOn::Inside => false,
        }
    }
}

/// The result of a comparison, as plain data for reports and the viewport.
#[derive(Debug, Clone, PartialEq)]
pub struct Deviation {
    /// Every sample, in triangle order, counted or not
    /// ([`DeviationPoint::counts`]); the figures below are of the counted.
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
    let never = CancelToken::new();
    compare_cancellable(
        stock,
        target,
        options,
        &JobContext::new(&never, &NoProgress),
    )
}

/// [`compare`] that stops with [`Error::Cancelled`] soon after `context` is
/// cancelled: the comparison is the longest stretch of a simulation run, and
/// a cancelled run must stop within the stated time (UI-09).
pub fn compare_cancellable(
    stock: &DexelStock,
    target: &TriangleMesh,
    options: &DeviationOptions,
    context: &JobContext<'_>,
) -> Result<Deviation> {
    compare_classified(stock, target, options, &|_, _, _| SampleOn::Part, context)
}

/// What a sample lies on, from its triangle's index in the target, its
/// position and the triangle's outward unit normal.
pub type Classify<'a> = dyn Fn(usize, Point3, Vec3) -> SampleOn + Sync + 'a;

/// [`compare_cancellable`] with each sample's [`SampleOn`] from `classify`:
/// a target made of the part and its held regions and tabs is sampled whole,
/// in triangle order, and only the samples that count make the figures
/// (decision record 0194).
pub fn compare_classified(
    stock: &DexelStock,
    target: &TriangleMesh,
    options: &DeviationOptions,
    classify: &Classify<'_>,
    context: &JobContext<'_>,
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
    context.check_cancelled()?;
    let mut points = surface_samples(target, options.spacing, classify, context)?;
    context.check_cancelled()?;
    // Measured in place, in fixed chunks, so each sample is held once.
    points.par_chunks_mut(CHUNK).try_for_each(|chunk| {
        context.check_cancelled()?;
        for d in chunk {
            *d = measure(stock, d.point, d.normal, d.on, options);
        }
        Ok::<(), Error>(())
    })?;
    let mut worst_remaining: Option<DeviationPoint> = None;
    let mut worst_overcut: Option<DeviationPoint> = None;
    for p in points.iter().filter(|p| p.counts()) {
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
        .filter(|p| p.counts() && -p.value > options.overcut_limit)
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
        let counted = || self.points.iter().filter(|p| p.counts());
        let mut out = Leftover {
            leave,
            tolerance,
            samples: counted().count(),
            mean: 0.0,
            least: f64::INFINITY,
            most: f64::NEG_INFINITY,
            below: 0,
            above: 0,
            worst_above: None,
        };
        let mut sum = 0.0;
        for p in counted() {
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

/// The deviation on each triangle of `mesh`: of the samples taken on it that
/// count ([`DeviationPoint::counts`]), the one furthest from the surface,
/// with the sign of [`DeviationPoint::value`] (positive for material left,
/// negative for overcut), and whether that sample's value is only a bound.
/// `None` for a triangle that has no such sample (a degenerate one, or one
/// wholly inside the finished part). `deviation` must be the comparison of `mesh` made with
/// samples no further apart than `spacing` ([`DeviationOptions::spacing`]),
/// since the samples are in triangle order and a triangle's share follows
/// from its shape. Fails if the counts do not add up.
pub fn triangle_values(
    deviation: &Deviation,
    mesh: &TriangleMesh,
    spacing: f64,
) -> Result<Vec<Option<(f64, bool)>>> {
    let mut out = Vec::with_capacity(mesh.triangles.len());
    let mut next = 0usize;
    for t in 0..mesh.triangles.len() {
        let count = triangle_sample_count(mesh, t, spacing, usize::MAX);
        if count == 0 {
            out.push(None);
            continue;
        }
        let samples = deviation.points.get(next..next + count).ok_or_else(|| {
            Error::invalid(
                "deviation",
                "it does not hold the samples of this mesh at this spacing",
            )
        })?;
        next += count;
        let worst =
            samples
                .iter()
                .filter(|p| p.counts())
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

/// The unit normal and corners of triangle `t`; `None` for a triangle that
/// is degenerate or refers to a missing vertex.
fn triangle_frame(mesh: &TriangleMesh, t: usize) -> Option<(Vec3, [Point3; 3])> {
    let [a, b, c] = mesh.triangle(t)?;
    let cross = (b - a).cross(c - a);
    let area2 = cross.length();
    if area2.is_nan() || area2 <= 1e-18 {
        return None;
    }
    Some((cross * (1.0 / area2), [a, b, c]))
}

/// A triangle's longest edge over its height beyond which it is sampled on a
/// lattice rather than subdivided n x n (the faces of a cube, at 2, are not).
const THIN: f64 = 4.0;

/// Where a triangle's samples go.
///
/// A well-shaped triangle is subdivided n x n so that no sub-triangle is
/// longer than the spacing, and sampled at the centroids (decision record
/// 0025 item 6). That gives n² samples whatever the area, so a long thin
/// triangle (an axial strip of a turned part, a blade of a blisk) once had
/// the square of its length in cells: 74 million samples for the owner's
/// blisk at 0.1 mm (OI-317). A triangle whose longest edge is more than
/// [`THIN`] times its height is sampled instead on a square lattice of pitch
/// `spacing` in its plane, plus points along each edge no further apart than
/// the spacing: a point further than `spacing / √2` from every edge has a
/// lattice point within that distance, and one nearer an edge has an edge
/// point within `spacing √3 / 2`, so every point is within the spacing of a
/// sample. The count follows the area plus the perimeter in cells.
enum Plan {
    /// The n x n subdivision from corner `a` along `ab` and `ac`.
    Grid {
        a: Point3,
        ab: Vec3,
        ac: Vec3,
        n: usize,
    },
    /// The lattice in the frame `o`, `u` (along the longest edge), `v`, with
    /// the corners in that frame, and the edges.
    Lattice {
        o: Point3,
        u: Vec3,
        v: Vec3,
        flat: [[f64; 2]; 3],
        corners: [Point3; 3],
        pitch: f64,
    },
}

impl Plan {
    fn new([a, b, c]: [Point3; 3], spacing: f64) -> Self {
        // The longest edge, the first of equals, as (start, end, length).
        let (o, q, longest) = cycle([a, b, c])
            .map(|(p, q)| (p, q, (q - p).length()))
            .into_iter()
            .fold((a, b, f64::NEG_INFINITY), |best, e| {
                if e.2 > best.2 { e } else { best }
            });
        let height = (b - a).cross(c - a).length() / longest;
        if longest <= THIN * height {
            return Self::Grid {
                a,
                ab: b - a,
                ac: c - a,
                n: ((longest / spacing).ceil() as usize).max(1),
            };
        }
        let u = (q - o) * (1.0 / longest);
        let w = (b - a).cross(c - a);
        let w = w * (1.0 / w.length());
        let v = w.cross(u);
        let flat = [a, b, c].map(|p| [(p - o).dot(u), (p - o).dot(v)]);
        Self::Lattice {
            o,
            u,
            v,
            flat,
            corners: [a, b, c],
            pitch: spacing,
        }
    }

    /// The lattice rows: their count, and for row `j` its height and the
    /// range of lattice columns inside the triangle.
    fn rows(&[f0, f1, f2]: &[[f64; 2]; 3], pitch: f64) -> (usize, f64, f64) {
        let vmin = f0[1].min(f1[1]).min(f2[1]);
        let vmax = f0[1].max(f1[1]).max(f2[1]);
        let umin = f0[0].min(f1[0]).min(f2[0]);
        let rows = ((vmax - vmin) / pitch - 0.5).ceil().max(0.0) as usize;
        (rows, vmin, umin)
    }

    fn row(
        flat: &[[f64; 2]; 3],
        pitch: f64,
        vmin: f64,
        umin: f64,
        j: usize,
    ) -> (f64, usize, usize) {
        let y = vmin + (j as f64 + 0.5) * pitch;
        let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
        for (p, q) in cycle(*flat) {
            let dy = q[1] - p[1];
            if (p[1] - y) * (q[1] - y) <= 0.0 && dy.abs() > 0.0 {
                let x = p[0] + (y - p[1]) / dy * (q[0] - p[0]);
                lo = lo.min(x);
                hi = hi.max(x);
            }
        }
        // Columns i with umin + (i + 0.5) pitch in [lo, hi].
        let i0 = ((lo - umin) / pitch - 0.5).ceil().max(0.0);
        let i1 = ((hi - umin) / pitch - 0.5).floor();
        if i1 >= i0 {
            (y, i0 as usize, i1 as usize + 1)
        } else {
            (y, 0, 0)
        }
    }

    /// Points along each edge, `ceil(length / pitch)` to an edge.
    fn edge_points(corners: &[Point3; 3], pitch: f64) -> [usize; 3] {
        cycle(*corners).map(|(p, q)| (((q - p).length() / pitch).ceil() as usize).max(1))
    }

    /// How many samples, counting no further than just past `limit`.
    fn count(&self, limit: usize) -> usize {
        match self {
            Self::Grid { n, .. } => n.saturating_mul(*n),
            Self::Lattice {
                flat,
                corners,
                pitch,
                ..
            } => {
                let mut count = Self::edge_points(corners, *pitch)
                    .iter()
                    .fold(0usize, |s, k| s.saturating_add(*k));
                let (rows, vmin, umin) = Self::rows(flat, *pitch);
                for j in 0..rows {
                    if count > limit {
                        break;
                    }
                    let (_, i0, i1) = Self::row(flat, *pitch, vmin, umin, j);
                    count = count.saturating_add(i1 - i0);
                }
                count
            }
        }
    }

    /// Gives each sample to `visit`, in a fixed order.
    fn samples(&self, mut visit: impl FnMut(Point3)) {
        match *self {
            Self::Grid { a, ab, ac, n } => {
                let step = 1.0 / n as f64;
                for i in 0..n {
                    for j in 0..n - i {
                        // Upright sub-triangle (i, j), (i+1, j), (i, j+1).
                        let (u, v) = ((i as f64 + 1.0 / 3.0) * step, (j as f64 + 1.0 / 3.0) * step);
                        visit(a + ab * u + ac * v);
                        // Inverted sub-triangle (i+1, j), (i, j+1), (i+1, j+1).
                        if i + j + 1 < n {
                            let (u, v) =
                                ((i as f64 + 2.0 / 3.0) * step, (j as f64 + 2.0 / 3.0) * step);
                            visit(a + ab * u + ac * v);
                        }
                    }
                }
            }
            Self::Lattice {
                o,
                u,
                v,
                ref flat,
                ref corners,
                pitch,
            } => {
                let (rows, vmin, umin) = Self::rows(flat, pitch);
                for j in 0..rows {
                    let (y, i0, i1) = Self::row(flat, pitch, vmin, umin, j);
                    for i in i0..i1 {
                        let x = umin + (i as f64 + 0.5) * pitch;
                        visit(o + u * x + v * y);
                    }
                }
                let counts = Self::edge_points(corners, pitch);
                for ((p, q), n) in cycle(*corners).into_iter().zip(counts) {
                    for m in 0..n {
                        visit(p + (q - p) * ((m as f64 + 0.5) / n as f64));
                    }
                }
            }
        }
    }
}

/// The edges of a triangle, in order: (a, b), (b, c), (c, a).
fn cycle<T: Copy>([a, b, c]: [T; 3]) -> [(T, T); 3] {
    [(a, b), (b, c), (c, a)]
}

/// How many samples triangle `t` has at `spacing` (zero for a degenerate
/// one), counting no further than just past `limit`.
fn triangle_sample_count(mesh: &TriangleMesh, t: usize, spacing: f64, limit: usize) -> usize {
    triangle_frame(mesh, t).map_or(0, |(_, corners)| Plan::new(corners, spacing).count(limit))
}

/// The most surface samples a comparison may take, a guard against a cell
/// far too fine for the part (each sample is held once, as its 64-byte
/// result; the stock's grid has the same guard, [`MAX_COLUMNS`]). The
/// owner's blisk, 10,550 mm² of surface, has 2.3 million at 0.1 mm.
///
/// [`MAX_COLUMNS`]: crate::MAX_COLUMNS
pub const MAX_DEVIATION_SAMPLES: usize = 64_000_000;

/// Points on each triangle no further apart than `spacing`, with the
/// triangle's unit normal, as its [`Plan`] places them, in triangle order,
/// to be measured. Degenerate triangles give none. Fails, before taking any,
/// when there would be more than [`MAX_DEVIATION_SAMPLES`].
fn surface_samples(
    mesh: &TriangleMesh,
    spacing: f64,
    classify: &Classify<'_>,
    context: &JobContext<'_>,
) -> Result<Vec<DeviationPoint>> {
    let mut counts = Vec::with_capacity(mesh.triangles.len());
    let mut total = 0usize;
    for t in 0..mesh.triangles.len() {
        let left = MAX_DEVIATION_SAMPLES.saturating_sub(total);
        let count = triangle_sample_count(mesh, t, spacing, left);
        total = total.saturating_add(count);
        if total > MAX_DEVIATION_SAMPLES {
            return Err(Error::invalid(
                "simulation.cell",
                format!(
                    "comparing the stock with the part at {spacing} mm needs more than the \
                     {MAX_DEVIATION_SAMPLES} surface samples supported; choose a coarser \
                     resolution"
                ),
            ));
        }
        counts.push(count);
    }
    let blank = DeviationPoint {
        point: Point3::new(0.0, 0.0, 0.0),
        normal: Vec3::new(0.0, 0.0, 0.0),
        value: 0.0,
        at_least: false,
        on: SampleOn::Part,
    };
    let mut points = vec![blank; total];
    // Each triangle fills its own share, so the order is the triangles'.
    let mut shares = Vec::with_capacity(counts.len());
    let mut rest = points.as_mut_slice();
    for (t, &count) in counts.iter().enumerate() {
        let (share, tail) = rest.split_at_mut(count);
        rest = tail;
        if count > 0 {
            shares.push((t, share));
        }
    }
    shares.into_par_iter().try_for_each(|(t, share)| {
        context.check_cancelled()?;
        if let Some((normal, corners)) = triangle_frame(mesh, t) {
            let mut slots = share.iter_mut();
            Plan::new(corners, spacing).samples(|point| {
                if let Some(slot) = slots.next() {
                    slot.point = point;
                    slot.normal = normal;
                    slot.on = classify(t, point, normal);
                }
            });
        }
        Ok::<(), Error>(())
    })?;
    Ok(points)
}

fn measure(
    stock: &DexelStock,
    p: Point3,
    n: Vec3,
    on: SampleOn,
    options: &DeviationOptions,
) -> DeviationPoint {
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
        on,
    }
}
