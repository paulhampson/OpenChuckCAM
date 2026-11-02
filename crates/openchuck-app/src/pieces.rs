// SPDX-License-Identifier: AGPL-3.0-or-later
//! Deviation colours that cover only the area each band applies to
//! (decision 0638; issue 488).
//!
//! A triangle is coloured by one value, so a large triangle holding more
//! than one band (a strip of the cut part across the edge of a bore, a
//! long triangle of the part half in reach) once took one colour over its
//! whole area. Such a triangle is cut into pieces instead: halved along its
//! longest edge, and each half again while its own test points still fall
//! in more than one band, down to about two simulation cells. A strip (four
//! or more times longer than it is wide, as the cut part's faced end is
//! made of) is cut across its length instead, so that a band edge across it
//! costs a few pieces rather than many along its whole length. The pieces
//! follow the band boundaries, not the area of the triangles that hold
//! them. Each piece is coloured by its parent's rule at its own centre.
//! The parents keep their numbers (selection sets, picking and outlines
//! read them); they are drawn as their pieces.

use openchuck_geom::select::TriangleSet;
use openchuck_types::geometry::Point3;
use openchuck_types::job::JobContext;
use openchuck_types::mesh::TriangleMesh;
use rayon::prelude::*;

use crate::AppError;
use crate::pathview::{BandPieces, DeviationBands, band_of, deviation_bands_with};

/// A value as the bands take it, and whether its material left is enclosed.
pub(crate) type Projected = (Option<(f64, bool)>, bool);

/// The rule a piece is coloured by: its parent, a point on it, and the
/// parent's corners.
pub(crate) type Rule<'a> = dyn Fn(usize, Point3, [Point3; 3]) -> Projected + Sync + 'a;

/// A parent and its pieces with their values.
type CutParent = (usize, Vec<([Point3; 3], Projected)>);

/// The bands' tolerance, the simulation cell and the most pieces.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Limits {
    /// The surface band, millimetres (one cell).
    pub(crate) tolerance: f64,
    /// The simulation cell, millimetres: no piece is cut finer than about
    /// two of them.
    pub(crate) pitch: f64,
    /// The most pieces ([`PIECE_BUDGET`]).
    pub(crate) budget: usize,
}

impl Limits {
    /// The limits for a simulation at `cell`, with the usual budget.
    pub(crate) fn for_cell(cell: f64) -> Self {
        Self {
            tolerance: cell,
            pitch: cell,
            budget: PIECE_BUDGET,
        }
    }
}

/// Whether a piece (its parent, its corners) holds more than one band, where
/// that is known exactly (the part, from its own samples).
pub(crate) type Mixed<'a> = dyn Fn(usize, [Point3; 3]) -> bool + Sync + 'a;

/// The most pieces made for one mesh: past it, the remaining parents are
/// drawn whole, as before, and the scale says so (NFR-04).
pub const PIECE_BUDGET: usize = 500_000;

/// Parents tested between two looks at the cancel flag.
const CHUNK: usize = 2048;

/// The deepest a piece is halved, a guard against a broken rule.
const MAX_DEPTH: u32 = 40;

/// The distance from `p` to the triangle `[a, b, c]`.
#[must_use]
pub fn distance_to_triangle(p: Point3, [a, b, c]: [Point3; 3]) -> f64 {
    // Ericson, Real-Time Collision Detection, 5.1.5.
    let (ab, ac, ap) = (b - a, c - a, p - a);
    let (d1, d2) = (ab.dot(ap), ac.dot(ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return p.distance(a);
    }
    let bp = p - b;
    let (d3, d4) = (ab.dot(bp), ac.dot(bp));
    if d3 >= 0.0 && d4 <= d3 {
        return p.distance(b);
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        return p.distance(a + ab * v);
    }
    let cp = p - c;
    let (d5, d6) = (ab.dot(cp), ac.dot(cp));
    if d6 >= 0.0 && d5 <= d6 {
        return p.distance(c);
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        return p.distance(a + ac * w);
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return p.distance(b + (c - b) * w);
    }
    let denom = 1.0 / (va + vb + vc);
    let (v, w) = (vb * denom, vc * denom);
    p.distance(a + ab * v + ac * w)
}

/// The pieces of `mesh`'s triangles that hold more than one band, by
/// `rule`, with `candidate` saying which triangles may (a cheap first
/// test). Whether a triangle or a piece holds more than one band is
/// `mixed`'s answer when given, else the rule's at seven test points. `pitch` is the simulation cell: no piece is
/// cut finer than about two of them, and a triangle no longer than that is
/// never cut. At most `budget` pieces: the parents in triangle order take
/// their pieces while they fit, and the rest are drawn whole. Each parent
/// is cut on its own, in fixed chunks, so the pieces are the same at any
/// thread count (NFR-06). `None` when no triangle needs it.
pub(crate) fn refine(
    mesh: &TriangleMesh,
    candidate: &(dyn Fn(usize) -> bool + Sync),
    rule: &Rule<'_>,
    mixed: Option<&Mixed<'_>>,
    limits: Limits,
    context: &JobContext<'_>,
) -> Result<Option<BandPieces>, AppError> {
    let Limits {
        tolerance,
        pitch,
        budget,
    } = limits;
    context.check_cancelled()?;
    let finest = 2.0 * pitch.max(1e-6);
    let chunks = mesh.triangles.len().div_ceil(CHUNK);
    let cut: Vec<Vec<CutParent>> = (0..chunks)
        .into_par_iter()
        .map(|chunk| {
            context.check_cancelled()?;
            let end = ((chunk + 1) * CHUNK).min(mesh.triangles.len());
            Ok((chunk * CHUNK..end)
                .filter(|t| candidate(*t))
                .filter_map(|t| {
                    let corners = mesh.triangle(t)?;
                    if longest(corners) <= finest {
                        return None;
                    }
                    let parent_rule = |p: Point3| rule(t, p, corners);
                    let one_band = |piece: [Point3; 3]| match mixed {
                        Some(mixed) => !mixed(t, piece),
                        None => uniform(piece, &parent_rule, tolerance),
                    };
                    if one_band(corners) {
                        return None;
                    }
                    let mut out = Vec::new();
                    cut_piece(
                        corners,
                        &parent_rule,
                        &one_band,
                        (finest, tolerance),
                        0,
                        &mut out,
                    );
                    Some((t, out))
                })
                .collect())
        })
        .collect::<Result<_, AppError>>()?;
    let mut parents = TriangleSet::default();
    let mut pieces = TriangleMesh::default();
    let mut parent = Vec::new();
    let mut values = Vec::new();
    let mut enclosed = Vec::new();
    let mut capped = false;
    for (t, out) in cut.into_iter().flatten() {
        if parent.len() + out.len() > budget {
            capped = true;
            continue;
        }
        parents.insert(u32::try_from(t).unwrap_or(u32::MAX));
        for (corners, (value, encl)) in out {
            let first = u32::try_from(pieces.vertices.len()).unwrap_or(u32::MAX);
            pieces.vertices.extend(corners);
            pieces
                .triangles
                .push([first, first.saturating_add(1), first.saturating_add(2)]);
            parent.push(u32::try_from(t).unwrap_or(u32::MAX));
            values.push(value);
            enclosed.push(encl);
        }
    }
    if parents.is_empty() {
        return Ok(None);
    }
    Ok(Some(BandPieces {
        parents,
        mesh: pieces,
        parent,
        bands: deviation_bands_with(&values, &enclosed, tolerance),
        capped,
    }))
}

/// The longest edge of a triangle.
fn longest([a, b, c]: [Point3; 3]) -> f64 {
    a.distance(b).max(b.distance(c)).max(c.distance(a))
}

/// The points a piece is tested at: its centre, and towards its corners
/// and the middles of its edges.
fn test_points([a, b, c]: [Point3; 3]) -> [Point3; 7] {
    let g = Point3::new(
        (a.x + b.x + c.x) / 3.0,
        (a.y + b.y + c.y) / 3.0,
        (a.z + b.z + c.z) / 3.0,
    );
    let mid =
        |p: Point3, q: Point3| Point3::new((p.x + q.x) / 2.0, (p.y + q.y) / 2.0, (p.z + q.z) / 2.0);
    let toward = |p: Point3| g + (p - g) * 0.95;
    [
        g,
        toward(a),
        toward(b),
        toward(c),
        toward(mid(a, b)),
        toward(mid(b, c)),
        toward(mid(c, a)),
    ]
}

/// Whether every test point of a piece falls in one band.
fn uniform(corners: [Point3; 3], rule: &dyn Fn(Point3) -> Projected, tolerance: f64) -> bool {
    let mut bands = test_points(corners).into_iter().map(|p| {
        let (value, enclosed) = rule(p);
        band_of(value, enclosed, tolerance)
    });
    let first = bands.next();
    bands.all(|b| Some(b) == first)
}

/// Cuts a piece in two along its longest edge while its test points
/// disagree and it is longer than `finest` (a strip is cut across its
/// length by [`cut_strip`]); each leaf takes the rule's value at its
/// centre. `tolerance` is the bands' surface tolerance.
fn cut_piece(
    corners: [Point3; 3],
    rule: &dyn Fn(Point3) -> Projected,
    one_band: &dyn Fn([Point3; 3]) -> bool,
    (finest, tolerance): (f64, f64),
    depth: u32,
    out: &mut Vec<([Point3; 3], Projected)>,
) {
    let [a, b, c] = corners;
    if depth >= MAX_DEPTH || longest(corners) <= finest || (depth > 0 && one_band(corners)) {
        let g = Point3::new(
            (a.x + b.x + c.x) / 3.0,
            (a.y + b.y + c.y) / 3.0,
            (a.z + b.z + c.z) / 3.0,
        );
        out.push((corners, rule(g)));
        return;
    }
    let [a, b, c] = longest_first(corners);
    let length = a.distance(b);
    if length >= ELONGATED * width([a, b, c]) {
        cut_strip([a, b, c], rule, one_band, (finest, tolerance), depth, out);
        return;
    }
    let m = Point3::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0, (a.z + b.z) / 2.0);
    cut_piece(
        [a, m, c],
        rule,
        one_band,
        (finest, tolerance),
        depth + 1,
        out,
    );
    cut_piece(
        [m, b, c],
        rule,
        one_band,
        (finest, tolerance),
        depth + 1,
        out,
    );
}

/// A triangle at least this many times longer than it is wide is a strip:
/// it is cut across its length, not halved along its longest edge.
const ELONGATED: f64 = 4.0;

/// The corners turned so that the first edge, a to b, is the longest,
/// keeping the winding.
fn longest_first([a, b, c]: [Point3; 3]) -> [Point3; 3] {
    if a.distance(b) >= b.distance(c) && a.distance(b) >= c.distance(a) {
        [a, b, c]
    } else if b.distance(c) >= c.distance(a) {
        [b, c, a]
    } else {
        [c, a, b]
    }
}

/// A triangle's width across its longest edge.
fn width([a, b, c]: [Point3; 3]) -> f64 {
    let length = longest([a, b, c]);
    if length > 0.0 {
        (b - a).cross(c - a).length() / length
    } else {
        0.0
    }
}

/// The part of the triangle `[a, b, c]` (a to b its longest edge) whose
/// distance along a to b is from `from` to `to`, as triangles in a fan.
fn slice([a, b, c]: [Point3; 3], from: f64, to: f64) -> Vec<[Point3; 3]> {
    let length = a.distance(b);
    let along = |p: Point3| (p - a).dot(b - a) / length;
    let mut polygon = vec![a, b, c];
    for (limit, keep_above) in [(from, true), (to, false)] {
        let inside = |p: Point3| {
            if keep_above {
                along(p) >= limit
            } else {
                along(p) <= limit
            }
        };
        let mut clipped = Vec::with_capacity(polygon.len() + 1);
        for (i, &p) in polygon.iter().enumerate() {
            let q = polygon.get((i + 1) % polygon.len()).copied().unwrap_or(p);
            if inside(p) {
                clipped.push(p);
            }
            if inside(p) != inside(q) {
                let (sp, sq) = (along(p), along(q));
                let t = (limit - sp) / (sq - sp);
                clipped.push(p + (q - p) * t);
            }
        }
        polygon = clipped;
    }
    let Some((&first, rest)) = polygon.split_first() else {
        return Vec::new();
    };
    rest.windows(2)
        .filter_map(|pair| match pair {
            [p, q] if (*p - first).cross(*q - first).length() > 0.0 => Some([first, *p, *q]),
            _ => None,
        })
        .collect()
}

/// Cuts a strip (a to b its longest edge) across its length. The length is
/// halved while a stretch's pieces hold more than one band and it is
/// longer than `finest` (or than the strip is wide); neighbouring stretches
/// of one band are joined. A stretch still holding more than one band is
/// cut on as a piece; each other piece takes the rule's value at its
/// centre. A band edge across the strip costs a few pieces, where halving
/// along the longest edge spreads them along the whole strip.
fn cut_strip(
    corners: [Point3; 3],
    rule: &dyn Fn(Point3) -> Projected,
    one_band: &dyn Fn([Point3; 3]) -> bool,
    (finest, tolerance): (f64, f64),
    depth: u32,
    out: &mut Vec<([Point3; 3], Projected)>,
) {
    let [a, b, _] = corners;
    let shortest = finest.max(width(corners));
    let centre = |[p, q, r]: [Point3; 3]| {
        Point3::new(
            (p.x + q.x + r.x) / 3.0,
            (p.y + q.y + r.y) / 3.0,
            (p.z + q.z + r.z) / 3.0,
        )
    };
    let band_at = |p: Point3| {
        let (value, enclosed) = rule(p);
        band_of(value, enclosed, tolerance)
    };
    // The stretches in order along the strip, each with its band when it
    // holds one only.
    let mut stretches: Vec<(f64, f64, Option<_>)> = Vec::new();
    let mut pending = vec![(0.0, a.distance(b), depth)];
    while let Some((from, to, level)) = pending.pop() {
        let pieces = slice(corners, from, to);
        let mut bands = pieces.iter().map(|p| band_at(centre(*p)));
        let first = bands.next();
        let one = first.is_some()
            && bands.all(|b| Some(b) == first)
            && pieces.iter().all(|p| one_band(*p));
        if one {
            stretches.push((from, to, first));
        } else if to - from <= shortest || level >= MAX_DEPTH {
            stretches.push((from, to, None));
        } else {
            let middle = (from + to) / 2.0;
            // The far half first, so the near half is taken next.
            pending.push((middle, to, level + 1));
            pending.push((from, middle, level + 1));
        }
    }
    let mut joined: Vec<(f64, f64, Option<_>)> = Vec::with_capacity(stretches.len());
    for stretch in stretches {
        match joined.last_mut() {
            Some(last) if last.2.is_some() && last.2 == stretch.2 => last.1 = stretch.1,
            _ => joined.push(stretch),
        }
    }
    for (from, to, band) in joined {
        for piece in slice(corners, from, to) {
            if band.is_some() {
                out.push((piece, rule(centre(piece))));
            } else {
                cut_piece(piece, rule, one_band, (finest, tolerance), depth + 1, out);
            }
        }
    }
}

/// `bands` with `pieces` attached: their parents left out of its bands.
pub(crate) fn with_pieces(mut bands: DeviationBands, pieces: Option<BandPieces>) -> DeviationBands {
    if let Some(pieces) = pieces {
        for (_, set) in &mut bands.bands {
            set.retain(|t| !pieces.parents.contains(t));
        }
        bands.bands.retain(|(_, set)| !set.is_empty());
        bands.pieces = Some(std::sync::Arc::new(pieces));
    }
    bands
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::float_cmp)]
    use super::*;
    use crate::pathview::DeviationBand;
    use openchuck_types::job::{CancelToken, NoProgress};

    const PITCH: f64 = 0.1;

    /// One right triangle with legs of `size` in the XY plane.
    fn triangle(size: f64) -> TriangleMesh {
        TriangleMesh {
            vertices: vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(size, 0.0, 0.0),
                Point3::new(0.0, size, 0.0),
            ],
            triangles: vec![[0, 1, 2]],
        }
    }

    /// On the surface left of `x`, more than 1 mm left from it on.
    fn split_at(x: f64) -> impl Fn(usize, Point3, [Point3; 3]) -> Projected + Sync {
        move |_, p, _| {
            if p.x < x {
                (Some((0.0, false)), false)
            } else {
                (Some((2.0, false)), false)
            }
        }
    }

    fn run(mesh: &TriangleMesh, rule: &Rule<'_>, budget: usize) -> Option<BandPieces> {
        let never = CancelToken::new();
        refine(
            mesh,
            &|_| true,
            rule,
            None,
            Limits {
                tolerance: PITCH,
                pitch: PITCH,
                budget,
            },
            &JobContext::new(&never, &NoProgress),
        )
        .unwrap()
    }

    fn area([a, b, c]: [Point3; 3]) -> f64 {
        (b - a).cross(c - a).length() / 2.0
    }

    fn band_area(pieces: &BandPieces, band: DeviationBand) -> f64 {
        pieces
            .bands
            .bands
            .iter()
            .filter(|(b, _)| *b == band)
            .flat_map(|(_, set)| set.iter())
            .map(|t| area(pieces.mesh.triangle(*t as usize).unwrap()))
            .sum()
    }

    #[test]
    fn req_sim_05_a_triangle_holding_two_classes_is_coloured_by_each_within_two_cells() {
        let mesh = triangle(10.0);
        let pieces = run(&mesh, &split_at(4.0), PIECE_BUDGET).expect("the triangle is cut");
        assert!(pieces.parents.contains(&0) && !pieces.capped);
        assert!(pieces.parent.iter().all(|p| *p == 0));
        // The true split: x >= 4 is a triangle with legs of 6.
        let thick = band_area(&pieces, DeviationBand::Thick);
        let on = band_area(&pieces, DeviationBand::OnTarget);
        assert!(
            (thick + on - 50.0).abs() < 1e-9,
            "the pieces cover the parent"
        );
        // The boundary is 6 mm long; it is placed to within two cells.
        assert!((thick - 18.0).abs() <= 6.0 * 2.0 * PITCH, "{thick}");
    }

    #[test]
    fn req_sim_05_a_triangle_of_one_class_is_drawn_whole() {
        let mesh = triangle(10.0);
        assert!(run(&mesh, &split_at(-1.0), PIECE_BUDGET).is_none());
        // Nor is a triangle smaller than two cells, whatever it holds.
        let small = triangle(0.1);
        assert!(run(&small, &split_at(0.05), PIECE_BUDGET).is_none());
    }

    #[test]
    fn req_sim_05_adaptive_pieces_follow_the_band_boundary() {
        // Twice the size: the boundary is twice as long and the area four
        // times; the pieces follow the boundary, so they grow by about two
        // (one more halving at the edge), not by four as even pieces would.
        let count = |size: f64| {
            run(&triangle(size), &split_at(0.4 * size), PIECE_BUDGET)
                .unwrap()
                .parent
                .len() as f64
        };
        let (small, large) = (count(10.0), count(20.0));
        assert!(large / small < 2.6, "{small} then {large} pieces");
        // And far fewer than even pieces two cells across.
        let even = 50.0 / (2.0 * PITCH * 2.0 * PITCH / 2.0);
        assert!(
            small < even / 5.0,
            "{small} pieces against {even} even ones"
        );
    }

    #[test]
    fn req_nfr_04_req_sim_05_a_long_strip_is_cut_across_its_length() {
        // The cut part's faced end is made of strips one column wide and as
        // long as the stock is across (decision 0638). Halving a strip along
        // its longest edge leaves a child as long as the strip, so the
        // pieces spread along it; cut across its length, a band edge costs
        // a few pieces and is still placed to within two cells.
        let (length, width) = (25.0, PITCH);
        let mesh = TriangleMesh {
            vertices: vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(length, 0.0, 0.0),
                Point3::new(length, width, 0.0),
                Point3::new(0.0, width, 0.0),
            ],
            triangles: vec![[0, 1, 2], [0, 2, 3]],
        };
        for edge in [3.37, 7.31, 12.5, 20.02] {
            let pieces = run(&mesh, &split_at(edge), PIECE_BUDGET).expect("the strip is cut");
            assert!(
                pieces.parent.len() <= 20,
                "{} pieces for one band edge across two triangles",
                pieces.parent.len()
            );
            let on = band_area(&pieces, DeviationBand::OnTarget);
            let thick = band_area(&pieces, DeviationBand::Thick);
            assert!(
                (on + thick - length * width).abs() < 1e-9,
                "the pieces cover the parents"
            );
            assert!((on - edge * width).abs() <= 2.0 * PITCH * width, "{on}");
        }
        // Two band edges, as on a strip across a bore: still a few pieces.
        let rule = |_: usize, p: Point3, _: [Point3; 3]| -> Projected {
            if (8.0..17.0).contains(&p.x) {
                (Some((2.0, false)), true)
            } else {
                (Some((0.0, false)), false)
            }
        };
        let pieces = run(&mesh, &rule, PIECE_BUDGET).unwrap();
        assert!(pieces.parent.len() <= 30, "{}", pieces.parent.len());
        let violet = band_area(&pieces, DeviationBand::Unreachable);
        assert!(
            (violet - 9.0 * width).abs() <= 4.0 * PITCH * width,
            "{violet}"
        );
    }

    #[test]
    fn req_nfr_06_the_split_colours_are_the_same_at_any_thread_count() {
        let mut mesh = TriangleMesh::default();
        for k in 0..40u32 {
            let x = f64::from(k) * 11.0;
            let first = k * 3;
            mesh.vertices.extend([
                Point3::new(x, 0.0, 0.0),
                Point3::new(x + 10.0, 0.0, 0.0),
                Point3::new(x, 10.0, 0.0),
            ]);
            mesh.triangles.push([first, first + 1, first + 2]);
        }
        let rule = |_: usize, p: Point3, _: [Point3; 3]| -> Projected {
            if (p.x % 11.0) + p.y < 7.0 {
                (Some((0.0, false)), false)
            } else {
                (Some((0.5, false)), true)
            }
        };
        let runs: Vec<_> = [1, 4]
            .iter()
            .map(|&threads| {
                rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build()
                    .unwrap()
                    .install(|| run(&mesh, &rule, PIECE_BUDGET))
            })
            .collect();
        assert!(runs[0].is_some());
        assert_eq!(runs[0], runs[1]);
    }

    #[test]
    fn req_nfr_04_the_added_triangles_stay_within_their_budget() {
        let mut mesh = triangle(10.0);
        mesh.vertices.extend([
            Point3::new(20.0, 0.0, 0.0),
            Point3::new(30.0, 0.0, 0.0),
            Point3::new(20.0, 10.0, 0.0),
        ]);
        mesh.triangles.push([3, 4, 5]);
        let rule = |_: usize, p: Point3, _: [Point3; 3]| -> Projected {
            if (p.x % 20.0) < 4.0 {
                (Some((0.0, false)), false)
            } else {
                (Some((2.0, false)), false)
            }
        };
        let whole = run(&mesh, &rule, PIECE_BUDGET).unwrap();
        let first = whole.parent.iter().filter(|p| **p == 0).count();
        // A budget that holds the first parent's pieces but not the second's.
        let capped = run(&mesh, &rule, first + 1).unwrap();
        assert!(capped.capped);
        assert!(capped.parent.len() <= first + 1);
        assert!(capped.parents.contains(&0) && !capped.parents.contains(&1));
    }

    #[test]
    fn req_sim_11_the_probe_on_a_split_triangle_reads_its_piece() {
        let mesh = triangle(10.0);
        let pieces = run(&mesh, &split_at(4.0), PIECE_BUDGET).unwrap();
        let bands = with_pieces(
            deviation_bands_with(&[Some((2.0, false))], &[false], PITCH),
            Some(pieces),
        );
        assert!(bands.bands.is_empty(), "the parent is drawn as its pieces");
        assert_eq!(bands.value_at(0, [1.0, 1.0, 0.0]), Some((0.0, false)));
        assert_eq!(
            bands.band_at(0, [1.0, 1.0, 0.0]),
            Some(DeviationBand::OnTarget)
        );
        assert_eq!(bands.value_at(0, [6.0, 1.0, 0.0]), Some((2.0, false)));
        assert_eq!(
            bands.band_at(0, [6.0, 1.0, 0.0]),
            Some(DeviationBand::Thick)
        );
    }

    #[test]
    fn req_ui_09_cutting_stops_when_cancelled() {
        let cancel = CancelToken::new();
        cancel.cancel();
        let e = refine(
            &triangle(10.0),
            &|_| true,
            &split_at(4.0),
            None,
            Limits::for_cell(PITCH),
            &JobContext::new(&cancel, &NoProgress),
        );
        assert!(e.is_err());
    }

    #[test]
    fn req_sim_05_the_distance_to_a_triangle_is_the_nearest_point_s() {
        let t = [
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
        ];
        assert_eq!(distance_to_triangle(Point3::new(0.2, 0.2, 3.0), t), 3.0);
        assert_eq!(distance_to_triangle(Point3::new(-1.0, 0.0, 0.0), t), 1.0);
        assert!(
            (distance_to_triangle(Point3::new(1.0, 1.0, 0.0), t) - 0.5f64.sqrt()).abs() < 1e-12
        );
    }

    #[test]
    fn req_sim_05_a_band_narrower_than_the_test_points_is_found_by_the_samples() {
        // A band 0.05 mm wide along one end of a long strip falls between
        // the test points; where the samples are known (the part), they say
        // which pieces hold more than one band.
        let strip = TriangleMesh {
            vertices: vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(20.0, 0.0, 0.0),
                Point3::new(0.0, 0.6, 0.0),
            ],
            triangles: vec![[0, 1, 2]],
        };
        let rule = split_at(0.05);
        let never = CancelToken::new();
        let context = JobContext::new(&never, &NoProgress);
        let blind = refine(
            &strip,
            &|_| true,
            &rule,
            None,
            Limits::for_cell(PITCH),
            &context,
        )
        .unwrap();
        assert!(blind.is_none(), "seven test points miss it");
        // The samples: 0.1 mm apart along the strip, the first in the band.
        let samples: Vec<Point3> = (0..200)
            .map(|i| Point3::new(0.025 + f64::from(i) * 0.1, 0.1, 0.0))
            .collect();
        let mixed = |_: usize, piece: [Point3; 3]| {
            let near: Vec<bool> = samples
                .iter()
                .filter(|p| distance_to_triangle(**p, piece) <= PITCH)
                .map(|p| p.x < 0.05)
                .collect();
            near.contains(&true) && near.contains(&false)
        };
        let pieces = refine(
            &strip,
            &|_| true,
            &rule,
            Some(&mixed),
            Limits::for_cell(PITCH),
            &context,
        )
        .unwrap()
        .expect("the samples find the band");
        assert!(
            pieces
                .bands
                .bands
                .iter()
                .any(|(b, _)| *b == DeviationBand::OnTarget)
        );
        assert!(pieces.parent.len() < 400, "{} pieces", pieces.parent.len());
    }
}
