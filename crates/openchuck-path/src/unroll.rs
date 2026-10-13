// SPDX-License-Identifier: AGPL-3.0-or-later
//! Regions on the unrolled development and their offsets (OP-04; decision
//! record 0016 of spike SP-7, "2D offsets and booleans"; work breakdown slice
//! 5 item 5).
//!
//! A cylinder about the rotary axis is developed to a plane: `x` along the
//! axis and `s = R * angle` around it ([`Development`]). The map is an
//! isometry, so a distance in the plane is a distance on the surface, and a
//! straight segment in the plane is a straight (X, A) move. A [`Region`] is an
//! outer loop and holes of lines and arcs in that plane. Strategies of the
//! wrap family offset it pass by pass (helical and axial finishing of a
//! selected region in slice 12, curve following in slice 16).
//!
//! The rules SP-7 found, kept here so no strategy has to know them:
//!
//! 1. **Every pass is offset from the original region** by `k` times the
//!    step ([`ValidRegion::offset`], [`ValidRegion::passes`]); the API has no
//!    way to offset a previous result, because that failed silently on a hole
//!    grid and an annulus.
//! 2. **Touching input is refused** ([`Region::validate`]): fewer than two
//!    vertices, a value that is not finite, no area, a loop that crosses
//!    itself, a hole outside the boundary or inside another hole, and any two
//!    loops closer than [`OffsetOptions::min_gap`] (twice the offset
//!    library's position epsilon). The error names the loops and the gap.
//! 3. **Dense contours are simplified** to [`OffsetOptions::tolerance`]
//!    before offsetting (straight runs only; arcs are kept as they are).
//! 4. **Every result is checked**: each result vertex and arc midpoint must be
//!    at the offset distance from the region that was offset, within
//!    [`OffsetOptions::check_tolerance`], found through a spatial index. This
//!    catches a result whose points are off the offset curve. **It does not
//!    catch a result made of the right pieces of curve put together wrongly**
//!    (extra or missing loops): for very dense, noisy loops (vertices a few
//!    to a few tens of micrometres apart with noise of that size, which no
//!    mesh section has) the library does that without an error, and the check
//!    passes it (OI-82).
//! 5. **Arcs are kept through the 2D stage and cut into chords when converted
//!    to (X, A)** ([`chord_points`]), with a sagitta of at most the chord
//!    tolerance, because LinuxCNC's G2 and G3 lie in the XY, XZ or YZ plane
//!    and move other axes linearly, so they cannot carry an arc that holds A.
//!    Lines stay single moves.
//! 6. **A region that goes all the way round** is offset as three tiles
//!    ([`PeriodicBand`]); loops come back unwrapped (`s` continuous across the
//!    seam), as the IR keeps A unwrapped, so nothing is cut at the seam.
//!
//! `cavalier_contours` does the work and no type of it appears in this API.
//! The independent oracle and the corpus that judge it are in
//! `openchuck-testkit::region`.

use cavalier_contours::core::Control;
use cavalier_contours::core::math::Vector2;
use cavalier_contours::polyline::{
    PlineIntersect, PlineOrientation, PlineSelfIntersectOptions, PlineSource, PlineSourceMut,
    PlineVertex, Polyline, seg_closest_point, seg_midpoint,
};
use cavalier_contours::shape_algorithms::{Shape, ShapeOffsetOptions};
use cavalier_contours::static_aabb2d_index::StaticAABB2DIndex;
use openchuck_types::units::{Degrees, Millimetres};

use crate::error::{PathError, Result};
use crate::parallel;

/// Closest two loops may come, mm: twice the offset library's position
/// epsilon of 1e-5 (decision record 0016, item 2).
pub const MIN_LOOP_GAP: f64 = 2e-5;
/// Default simplification tolerance, mm: a fifth of the 0.005 mm path
/// fitting budget of verification plan 4.1.
pub const SIMPLIFY_TOLERANCE: f64 = 1e-3;
/// Default tolerance of the result check, mm: a tenth of the 0.01 mm toolpath
/// deviation.
pub const CHECK_TOLERANCE: f64 = 1e-3;
/// Chord error used to flatten arcs when measuring the gap between loops, mm;
/// small against [`MIN_LOOP_GAP`].
const GAP_FLATTEN: f64 = 1e-6;
/// Smallest area a loop may have, mm squared.
const MIN_AREA: f64 = 1e-9;
/// The offset library's position epsilon.
const POS_EPS: f64 = 1e-5;

/// A vertex of a closed loop in the development plane: the position and the
/// bulge of the segment to the next vertex (`tan(sweep / 4)`; zero for a
/// line, positive for an arc turning counter-clockwise).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vertex {
    /// Position along the rotary axis, mm.
    pub x: f64,
    /// Position around the cylinder, mm (`radius * angle`).
    pub s: f64,
    /// Bulge of the segment to the next vertex.
    pub bulge: f64,
}

impl Vertex {
    /// A vertex at (`x`, `s`) with `bulge`.
    pub const fn new(x: f64, s: f64, bulge: f64) -> Self {
        Self { x, s, bulge }
    }
}

/// A closed loop of lines and arcs.
pub type Loop = Vec<Vertex>;

/// The development of a cylinder about the rotary axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Development {
    radius: f64,
}

impl Development {
    /// The development of a cylinder of `radius`, which must be finite and
    /// greater than zero.
    pub fn new(radius: Millimetres) -> Result<Self> {
        if radius.0.is_finite() && radius.0 > 0.0 {
            Ok(Self { radius: radius.0 })
        } else {
            Err(PathError::Region(format!(
                "the cylinder radius {} must be finite and greater than zero",
                radius.0
            )))
        }
    }

    /// The radius.
    pub fn radius(&self) -> Millimetres {
        Millimetres(self.radius)
    }

    /// Distance once round the cylinder, `2 pi R`.
    pub fn period(&self) -> f64 {
        std::f64::consts::TAU * self.radius
    }

    /// (x, A) to the development plane.
    pub fn to_plane(&self, x: f64, a: Degrees) -> (f64, f64) {
        (x, self.radius * a.to_radians())
    }

    /// The development plane to (x, A).
    pub fn to_angle(&self, x: f64, s: f64) -> (f64, Degrees) {
        (x, Degrees::from_radians(s / self.radius))
    }
}

/// An outer boundary and holes, as given. [`Region::validate`] checks it and
/// fixes the orientation (outer counter-clockwise, holes clockwise).
#[derive(Debug, Clone, PartialEq)]
pub struct Region {
    /// The outer boundary.
    pub outer: Loop,
    /// The holes.
    pub holes: Vec<Loop>,
}

/// Tolerances of the checks and the simplification.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OffsetOptions {
    /// Straight runs are simplified to this, mm.
    pub tolerance: f64,
    /// Loops closer than this are refused, mm.
    pub min_gap: f64,
    /// A result vertex may be this far from the exact offset distance, mm.
    pub check_tolerance: f64,
}

impl Default for OffsetOptions {
    fn default() -> Self {
        Self {
            tolerance: SIMPLIFY_TOLERANCE,
            min_gap: MIN_LOOP_GAP,
            check_tolerance: CHECK_TOLERANCE,
        }
    }
}

impl OffsetOptions {
    fn check(&self) -> Result<()> {
        for (name, value) in [
            ("tolerance", self.tolerance),
            ("min_gap", self.min_gap),
            ("check_tolerance", self.check_tolerance),
        ] {
            if !(value.is_finite() && value > 0.0) {
                return Err(PathError::Region(format!(
                    "the {name} {value} must be finite and greater than zero"
                )));
            }
        }
        Ok(())
    }
}

fn pline(l: &[Vertex]) -> Polyline<f64> {
    let mut p = Polyline::new_closed();
    for v in l {
        p.add(v.x, v.s, v.bulge);
    }
    p
}

fn from_pline(p: &Polyline<f64>) -> Loop {
    p.iter_vertexes()
        .map(|v| Vertex::new(v.x, v.y, v.bulge))
        .collect()
}

/// The loop's name in messages: `None` is the outer boundary, `Some(i)` hole
/// number `i + 1`.
fn name(i: Option<usize>) -> String {
    match i {
        None => "the outer boundary".to_owned(),
        Some(i) => format!("hole {}", i + 1),
    }
}

fn loop_name(index: usize) -> String {
    name(index.checked_sub(1))
}

/// Runs `f`, turning a panic inside the offset library into an error
/// (NFR-07): no input may crash the product.
fn guarded<T>(what: &str, f: impl FnOnce() -> T) -> Result<T> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).map_err(|_| {
        PathError::Region(format!(
            "the offset library failed while {what}; the region is probably degenerate"
        ))
    })
}

impl Region {
    /// A region from an outer loop and holes.
    pub fn new(outer: Loop, holes: Vec<Loop>) -> Self {
        Self { outer, holes }
    }

    /// Checks the region (see the module documentation, rule 2), simplifies
    /// dense straight runs and fixes the orientation. The result is what can
    /// be offset.
    pub fn validate(&self, options: &OffsetOptions) -> Result<ValidRegion> {
        options.check()?;
        let mut loops: Vec<Polyline<f64>> = Vec::with_capacity(1 + self.holes.len());
        for (i, l) in std::iter::once(&self.outer)
            .chain(self.holes.iter())
            .enumerate()
        {
            loops.push(clean_loop(l, &loop_name(i), options)?);
        }
        // Orientation: the outer boundary counter-clockwise, holes clockwise.
        for (i, p) in loops.iter_mut().enumerate() {
            let ccw = p.orientation() == PlineOrientation::CounterClockwise;
            if ccw != (i == 0) {
                p.invert_direction_mut();
            }
        }
        for (i, p) in loops.iter().enumerate() {
            let mut crossed = false;
            guarded("checking a loop for self-intersection", || {
                p.visit_self_intersects_opt(
                    &mut |_: PlineIntersect<f64>| {
                        crossed = true;
                        Control::Break(())
                    },
                    &PlineSelfIntersectOptions::default(),
                );
            })?;
            if crossed {
                return Err(PathError::Region(format!(
                    "{} crosses itself",
                    loop_name(i)
                )));
            }
        }
        let Some((outer, holes)) = loops.split_first() else {
            return Err(PathError::Region("the region has no outer boundary".into()));
        };
        for (i, h) in holes.iter().enumerate() {
            let probe = h.at(0).pos();
            if outer.winding_number(probe) == 0 {
                return Err(PathError::Region(format!(
                    "{} is not inside the outer boundary",
                    name(Some(i))
                )));
            }
            for (j, other) in holes.iter().enumerate() {
                if i != j && other.winding_number(probe) != 0 {
                    return Err(PathError::Region(format!(
                        "{} is inside {}",
                        name(Some(i)),
                        name(Some(j))
                    )));
                }
            }
        }
        check_gaps(&loops, options.min_gap)?;
        let vertex_count = loops.iter().map(Polyline::vertex_count).sum();
        Ok(ValidRegion {
            loops,
            options: *options,
            vertex_count,
        })
    }
}

/// Removes repeated and redundant vertices, checks the loop is usable and
/// simplifies its straight runs to the tolerance.
fn clean_loop(l: &[Vertex], which: &str, options: &OffsetOptions) -> Result<Polyline<f64>> {
    if l.iter()
        .any(|v| !(v.x.is_finite() && v.s.is_finite() && v.bulge.is_finite()))
    {
        return Err(PathError::Region(format!(
            "{which} has a value that is not a finite number"
        )));
    }
    if l.len() < 2 {
        return Err(PathError::Region(format!(
            "{which} needs at least two vertices"
        )));
    }
    let raw = pline(l);
    let cleaned = guarded("removing repeated vertices", || {
        raw.remove_redundant(POS_EPS)
    })?
    .unwrap_or(raw);
    if cleaned.vertex_count() < 2 || cleaned.area().abs() < MIN_AREA {
        return Err(PathError::Region(format!("{which} encloses no area")));
    }
    let simple = simplify(&cleaned, options.tolerance);
    if simple.vertex_count() < 2 || simple.area().abs() < MIN_AREA {
        return Err(PathError::Region(format!(
            "{which} encloses no area once its straight runs are simplified to {} mm",
            options.tolerance
        )));
    }
    Ok(simple)
}

fn flag(flags: &[bool], i: usize) -> bool {
    flags.get(i).copied().unwrap_or(false)
}

fn set(flags: &mut [bool], i: usize) {
    if let Some(f) = flags.get_mut(i) {
        *f = true;
    }
}

/// Douglas-Peucker over the straight runs of a loop; vertices that start or
/// end an arc are kept.
fn simplify(p: &Polyline<f64>, tolerance: f64) -> Polyline<f64> {
    let vs: Vec<PlineVertex<f64>> = p.iter_vertexes().collect();
    let n = vs.len();
    if n < 4 {
        return p.clone();
    }
    let mut keep = vec![false; n];
    for (i, v) in vs.iter().enumerate() {
        if v.bulge != 0.0 {
            set(&mut keep, i);
            set(&mut keep, (i + 1) % n);
        }
    }
    if !keep.iter().any(|k| *k) {
        // A pure polygon: anchor at the first vertex and the one farthest from it.
        let first = vs.first().map_or(Vector2::zero(), PlineVertex::pos);
        let far = vs
            .iter()
            .enumerate()
            .max_by(|a, b| {
                (a.1.pos() - first)
                    .length_squared()
                    .total_cmp(&(b.1.pos() - first).length_squared())
            })
            .map_or(0, |(i, _)| i);
        set(&mut keep, 0);
        set(&mut keep, far);
    }
    let kept: Vec<usize> = (0..n).filter(|i| flag(&keep, *i)).collect();
    for (k, &a) in kept.iter().enumerate() {
        let Some(&b) = kept.get((k + 1) % kept.len()) else {
            continue;
        };
        if vs.get(a).is_none_or(|v| v.bulge != 0.0) {
            continue; // an arc starts here; nothing to simplify
        }
        let mut run = vec![a];
        let mut i = (a + 1) % n;
        while i != b {
            run.push(i);
            i = (i + 1) % n;
        }
        run.push(b);
        mark_douglas_peucker(&vs, &run, tolerance, &mut keep);
    }
    let mut out = Polyline::new_closed();
    for (i, v) in vs.iter().enumerate() {
        if flag(&keep, i) {
            out.add_vertex(*v);
        }
    }
    out
}

fn mark_douglas_peucker(vs: &[PlineVertex<f64>], run: &[usize], tol: f64, keep: &mut [bool]) {
    let (Some(&first), Some(&last)) = (run.first(), run.last()) else {
        return;
    };
    if run.len() < 3 {
        return;
    }
    let (Some(va), Some(vb)) = (vs.get(first), vs.get(last)) else {
        return;
    };
    let (a, b) = (va.pos(), vb.pos());
    let ab = b - a;
    let l2 = ab.length_squared();
    let mut worst = (0.0_f64, 0usize);
    for (k, &idx) in run.iter().enumerate().skip(1).take(run.len() - 2) {
        let Some(v) = vs.get(idx) else { continue };
        let p = v.pos();
        let d = if l2 <= f64::MIN_POSITIVE {
            (p - a).length()
        } else {
            let t = ((p - a).dot(ab) / l2).clamp(0.0, 1.0);
            (p - (a + Vector2::new(ab.x * t, ab.y * t))).length()
        };
        if d > worst.0 {
            worst = (d, k);
        }
    }
    if worst.0 > tol {
        let split = worst.1;
        if let Some(&idx) = run.get(split) {
            set(keep, idx);
        }
        if let (Some(head), Some(tail)) = (run.get(..=split), run.get(split..)) {
            mark_douglas_peucker(vs, head, tol, keep);
            mark_douglas_peucker(vs, tail, tol, keep);
        }
    }
}

// ---- gaps between loops -----------------------------------------------------

/// A loop flattened finely, with a spatial index of its segments.
struct Flat {
    pline: Polyline<f64>,
    index: StaticAABB2DIndex<f64>,
}

fn flatten(p: &Polyline<f64>) -> Result<Flat> {
    let pline = guarded("flattening a loop", || p.arcs_to_approx_lines(GAP_FLATTEN))?
        .unwrap_or_else(|| p.clone());
    let index = guarded("indexing a loop", || pline.create_aabb_index())?;
    Ok(Flat { pline, index })
}

fn boxes_near(a: &Polyline<f64>, b: &Polyline<f64>, gap: f64) -> bool {
    let (Some(ea), Some(eb)) = (a.extents(), b.extents()) else {
        return false;
    };
    ea.min_x - gap <= eb.max_x
        && eb.min_x - gap <= ea.max_x
        && ea.min_y - gap <= eb.max_y
        && eb.min_y - gap <= ea.max_y
}

/// The smallest distance from a vertex of `a` to the segments of `b`, if it
/// is below `limit`.
fn vertex_gap(a: &Flat, b: &Flat, limit: f64) -> Option<f64> {
    let n = b.pline.vertex_count();
    let mut best: Option<f64> = None;
    for v in a.pline.iter_vertexes() {
        let near = b
            .index
            .query(v.x - limit, v.y - limit, v.x + limit, v.y + limit);
        for seg in near {
            let (Some(s0), Some(s1)) = (
                b.pline.get(seg),
                b.pline.get(if seg + 1 >= n { 0 } else { seg + 1 }),
            ) else {
                continue;
            };
            let d = (v.pos() - seg_closest_point(s0, s1, v.pos(), POS_EPS)).length();
            if d < limit && best.is_none_or(|b| d < b) {
                best = Some(d);
            }
        }
    }
    best
}

/// Refuses loops that cross, touch or come closer than `min_gap`.
fn check_gaps(loops: &[Polyline<f64>], min_gap: f64) -> Result<()> {
    // Slack for the inscribed flattening: the true gap can be larger than the
    // polygons' by up to twice GAP_FLATTEN.
    let limit = min_gap + 2.0 * GAP_FLATTEN;
    for (i, a) in loops.iter().enumerate() {
        for (j, b) in loops.iter().enumerate().skip(i + 1) {
            if !boxes_near(a, b, limit) {
                continue;
            }
            let crossing = guarded("looking for loops that cross", || {
                let found = a.find_intersects(b);
                !found.basic_intersects.is_empty() || !found.overlapping_intersects.is_empty()
            })?;
            let gap = if crossing {
                Some(0.0)
            } else {
                let (fa, fb) = (flatten(a)?, flatten(b)?);
                match (vertex_gap(&fa, &fb, limit), vertex_gap(&fb, &fa, limit)) {
                    (Some(x), Some(y)) => Some(x.min(y)),
                    (x, y) => x.or(y),
                }
            };
            if let Some(gap) = gap {
                return Err(PathError::Region(format!(
                    "{} and {} {} ({gap:.2e} mm apart), closer than the {min_gap:.0e} mm the \
                     offset can be trusted at; move or merge them",
                    loop_name(i),
                    loop_name(j),
                    if crossing {
                        "cross or touch"
                    } else {
                        "come too close"
                    },
                )));
            }
        }
    }
    Ok(())
}

// ---- offsets ----------------------------------------------------------------

/// A checked region, ready to offset.
#[derive(Debug, Clone)]
pub struct ValidRegion {
    loops: Vec<Polyline<f64>>,
    options: OffsetOptions,
    vertex_count: usize,
}

/// The loops left after an offset: boundaries of material still to be cut
/// (`outers`, counter-clockwise) and of islands inside it (`holes`,
/// clockwise).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct OffsetLoops {
    /// Outer boundaries.
    pub outers: Vec<Loop>,
    /// Holes.
    pub holes: Vec<Loop>,
}

impl OffsetLoops {
    /// True when nothing is left.
    pub fn is_empty(&self) -> bool {
        self.outers.is_empty() && self.holes.is_empty()
    }

    /// Every loop, outers first.
    pub fn loops(&self) -> impl Iterator<Item = &Loop> {
        self.outers.iter().chain(self.holes.iter())
    }
}

impl ValidRegion {
    /// Vertices in the region once simplified, all loops together.
    pub fn vertex_count(&self) -> usize {
        self.vertex_count
    }

    /// The region as it will be offset: oriented and simplified.
    pub fn loops(&self) -> impl Iterator<Item = Loop> + '_ {
        self.loops.iter().map(from_pline)
    }

    /// The region offset by `distance` from its original boundary: positive
    /// shrinks it (inward), negative grows it. The result is checked
    /// (module documentation, rule 4); a result that fails the check is an
    /// error naming the worst vertex.
    pub fn offset(&self, distance: f64) -> Result<OffsetLoops> {
        if !distance.is_finite() {
            return Err(PathError::Region(format!(
                "the offset distance {distance} is not a finite number"
            )));
        }
        if distance == 0.0 {
            let (outers, holes) = self.loops.split_at(1);
            return Ok(OffsetLoops {
                outers: outers.iter().map(from_pline).collect(),
                holes: holes.iter().map(from_pline).collect(),
            });
        }
        let out = guarded("offsetting the region", || {
            let shape = Shape::from_plines(self.loops.iter().cloned());
            shape.parallel_offset(distance, &ShapeOffsetOptions::new())
        })?;
        let result = OffsetLoops {
            outers: out
                .ccw_plines
                .iter()
                .map(|p| from_pline(&p.polyline))
                .collect(),
            holes: out
                .cw_plines
                .iter()
                .map(|p| from_pline(&p.polyline))
                .collect(),
        };
        self.check_offset(&result, distance)?;
        Ok(result)
    }

    /// Passes `step`, `2 step`, and so on, each offset from the original
    /// region, until nothing is left or `max_passes` are made. Passes are
    /// computed in parallel on the caller's pool, in fixed chunks, and come
    /// back in order (NFR-06).
    pub fn passes(&self, step: f64, max_passes: usize) -> Result<Vec<OffsetLoops>> {
        if !(step.is_finite() && step > 0.0) {
            return Err(PathError::Region(format!(
                "the pass step {step} must be finite and greater than zero"
            )));
        }
        let mut out = Vec::new();
        let mut k = 1usize;
        while k <= max_passes {
            let ks: Vec<usize> = (k..=max_passes.min(k + parallel::CHUNK - 1)).collect();
            let results = parallel::map_in_order(&ks, |k| self.offset(step * *k as f64));
            for r in results {
                let loops = r?;
                if loops.is_empty() {
                    return Ok(out);
                }
                out.push(loops);
            }
            k += ks.len();
        }
        Ok(out)
    }

    /// Checks that `result` is the offset of this region by `distance`, as far
    /// as a cheap test can tell (rule 4): every vertex and arc midpoint of
    /// every loop must lie at `|distance|` from the region that was offset,
    /// within the check tolerance. [`ValidRegion::offset`] runs it on its own
    /// results; it is public so a host (and the tests) can run it on a result
    /// from anywhere.
    pub fn check_offset(&self, result: &OffsetLoops, distance: f64) -> Result<()> {
        let want = distance.abs();
        let tol = self.options.check_tolerance;
        let window = want + 2.0 * tol;
        let indexes = guarded("indexing the region", || {
            self.loops
                .iter()
                .map(|p| (p, p.create_aabb_index()))
                .collect::<Vec<_>>()
        })?;
        let nearest = |q: Vector2<f64>| -> f64 {
            let mut best = f64::INFINITY;
            for (p, index) in &indexes {
                let n = p.vertex_count();
                for seg in index.query(q.x - window, q.y - window, q.x + window, q.y + window) {
                    let (Some(s0), Some(s1)) =
                        (p.get(seg), p.get(if seg + 1 >= n { 0 } else { seg + 1 }))
                    else {
                        continue;
                    };
                    best = best.min((q - seg_closest_point(s0, s1, q, POS_EPS)).length());
                }
            }
            best
        };
        for (which, l) in result.loops().enumerate() {
            let p = pline(l);
            let n = p.vertex_count();
            for (i, v) in p.iter_vertexes().enumerate() {
                let next = p.at(if i + 1 >= n { 0 } else { i + 1 });
                let mut points = vec![v.pos()];
                if v.bulge != 0.0 {
                    points.push(seg_midpoint(v, next));
                }
                for q in points {
                    let d = nearest(q);
                    if !d.is_finite() || (d - want).abs() > tol {
                        return Err(PathError::Region(format!(
                            "the offset by {distance} mm returned a loop (number {}) with a point \
                             ({:.4}, {:.4}) at {} from the original boundary instead of {want} mm, \
                             so the result is not trusted; the region is probably too dense or too \
                             jagged (simplify it) or too thin for this distance",
                            which + 1,
                            q.x,
                            q.y,
                            if d.is_finite() {
                                format!("{d:.4} mm")
                            } else {
                                "more than the offset".to_owned()
                            },
                        )));
                    }
                }
            }
        }
        Ok(())
    }
}

// ---- back to (X, A) ---------------------------------------------------------

/// A point of a pass in (X, A).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnglePoint {
    /// Position along the rotary axis, mm.
    pub x: f64,
    /// Rotary angle, unwrapped.
    pub a: Degrees,
}

/// A loop as (X, A) points for linear moves: lines stay single moves
/// (their end vertices), and every arc is cut into chords with a sagitta of
/// at most `chord_tolerance` (module documentation, rule 5). The loop is
/// closed: the first point is not repeated.
pub fn chord_points(
    l: &[Vertex],
    development: Development,
    chord_tolerance: f64,
) -> Result<Vec<AnglePoint>> {
    if !(chord_tolerance.is_finite() && chord_tolerance > 0.0) {
        return Err(PathError::Region(format!(
            "the chord tolerance {chord_tolerance} must be finite and greater than zero"
        )));
    }
    let flat = guarded("cutting arcs into chords", || {
        pline(l).arcs_to_approx_lines(chord_tolerance)
    })?
    .ok_or_else(|| PathError::Region("a loop could not be cut into chords".into()))?;
    Ok(flat
        .iter_vertexes()
        .map(|v| {
            let (x, a) = development.to_angle(v.x, v.y);
            AnglePoint { x, a }
        })
        .collect())
}

// ---- a region that goes round ------------------------------------------------

/// A band of the cylinder, `x_min <= x <= x_max` all the way round, with
/// islands in it (module documentation, rule 6). The development is periodic
/// in `s` with the period `2 pi R`, and the offset library knows nothing of
/// periods, so the band is tiled three times (`s - P`, `s`, `s + P`),
/// offset as one region, and the loops that belong to the middle period are
/// returned.
#[derive(Debug, Clone, PartialEq)]
pub struct PeriodicBand {
    /// Lower end of the band along the axis.
    pub x_min: f64,
    /// Upper end of the band along the axis.
    pub x_max: f64,
    /// Islands (material that stays), each given once, in any period.
    pub islands: Vec<Loop>,
}

impl PeriodicBand {
    /// The band with every island grown by `distance` (negative shrinks
    /// them), as loops unwrapped across the seam: each returned loop's lowest
    /// `s` lies in `[-P/2, P/2)`, and one that straddles the seam simply
    /// continues past `P/2` (the IR keeps A unwrapped, so nothing is cut).
    /// Fails when islands grow into one ring that goes round the cylinder.
    pub fn offset_islands(
        &self,
        development: Development,
        distance: f64,
        options: &OffsetOptions,
    ) -> Result<Vec<Loop>> {
        let period = development.period();
        if !(self.x_min.is_finite() && self.x_max.is_finite() && self.x_min < self.x_max) {
            return Err(PathError::Region(
                "the band needs finite x_min below x_max".into(),
            ));
        }
        // Wide enough that no tiled island touches the artificial ends.
        let outer = vec![
            Vertex::new(self.x_min, -2.5 * period, 0.0),
            Vertex::new(self.x_max, -2.5 * period, 0.0),
            Vertex::new(self.x_max, 2.5 * period, 0.0),
            Vertex::new(self.x_min, 2.5 * period, 0.0),
        ];
        let mut holes = Vec::new();
        for island in &self.islands {
            for k in [-1.0, 0.0, 1.0] {
                holes.push(
                    island
                        .iter()
                        .map(|v| Vertex::new(v.x, v.s + k * period, v.bulge))
                        .collect::<Loop>(),
                );
            }
        }
        let valid = Region::new(outer, holes).validate(options)?;
        // The offset library shrinks the free region, which grows the islands.
        let result = valid.offset(distance)?;
        let mut kept = Vec::new();
        for l in result.holes {
            let (lo, hi) = l
                .iter()
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
                    (lo.min(v.s), hi.max(v.s))
                });
            if hi - lo >= period {
                return Err(PathError::Region(format!(
                    "the islands grown by {} mm join into a ring that goes all the way round the \
                     cylinder; offsets beyond this distance are not supported",
                    -distance
                )));
            }
            if (-0.5 * period..0.5 * period).contains(&lo) {
                kept.push(l);
            }
        }
        Ok(kept)
    }
}
