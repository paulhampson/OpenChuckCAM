// SPDX-License-Identifier: AGPL-3.0-or-later
//! The operations under test, behind plain `Loop` data so nothing else touches the crates.

use crate::geom::{flatten, from_pline, to_pline, Loop, Region};
use cavalier_contours::polyline::{BooleanOp, PlineSource, PlineSourceMut, Polyline};
use cavalier_contours::shape_algorithms::{Shape, ShapeOffsetOptions};
use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::float::single::SingleFloatOverlay;
use i_overlay::mesh::float::outline::offset::OutlineOffset;
use i_overlay::mesh::float::style::{LineJoin, OutlineStyle};

/// Inward offset of a region by `d` (negative grows it) with cavalier_contours' shape offset.
/// Arcs are kept as arcs. Returns every result loop (outer boundaries and holes).
pub fn cav_offset(loops: &[Loop], d: f64) -> Vec<Loop> {
    let shape = Shape::from_plines(loops.iter().map(to_pline));
    let out = shape.parallel_offset(d, &ShapeOffsetOptions::new());
    out.ccw_plines.iter().chain(out.cw_plines.iter()).map(|p| from_pline(&p.polyline)).collect()
}

/// The same offset one loop at a time (no shape-level interaction between loops), for the
/// finding that holes must be offset together with the outer boundary.
pub fn cav_offset_each_loop(loops: &[Loop], d: f64) -> Vec<Loop> {
    let mut out = Vec::new();
    for l in loops {
        for p in to_pline(l).parallel_offset(d) {
            out.push(from_pline(&p));
        }
    }
    out
}

/// Contours with less area than this (mm squared) are artefacts, not material.
pub const MIN_LOOP_AREA: f64 = 1e-6;

/// Inward offset by `d` with i_overlay's outline offset, exactly as it returns. Arcs must be
/// flattened first (`arc_tol`), and the result is polylines only. Round joins get the step
/// (segment length over radius) that keeps their chord error at `join_tol`.
pub fn io_offset_raw(loops: &[Loop], d: f64, arc_tol: f64, join_tol: f64) -> Vec<Loop> {
    let round_step = (8.0 * join_tol / d.abs().max(0.05)).sqrt();
    let shape: Vec<Vec<[f64; 2]>> = loops.iter().map(|l| flatten(l, arc_tol)).collect();
    let style = OutlineStyle::new(-d).line_join(LineJoin::Round(round_step));
    let shapes = shape.outline(&style);
    shapes
        .into_iter()
        .flat_map(|s| s.into_iter())
        .map(|c| c.into_iter().map(|p| [p[0], p[1], 0.0]).collect::<Loop>())
        .collect()
}

/// [`io_offset_raw`] without the zero-area contours i_overlay leaves behind when a hole closes.
pub fn io_offset(loops: &[Loop], d: f64, arc_tol: f64, join_tol: f64) -> Vec<Loop> {
    let mut out = io_offset_raw(loops, d, arc_tol, join_tol);
    out.retain(|l| crate::geom::signed_area(l).abs() >= MIN_LOOP_AREA);
    out
}

/// How many contours of the raw result are artefacts.
pub fn count_artefacts(raw: &[Loop]) -> usize {
    raw.iter().filter(|l| crate::geom::signed_area(l).abs() < MIN_LOOP_AREA).count()
}

#[derive(Clone, Copy, Debug)]
pub enum Op {
    Union,
    Intersect,
    Difference,
    Xor,
}

pub const OPS: [Op; 4] = [Op::Union, Op::Intersect, Op::Difference, Op::Xor];

/// Boolean of two single closed loops with cavalier_contours (arcs kept). Returns the result
/// loops (positive and negative together; they are nested for even-odd membership).
pub fn cav_boolean(a: &Loop, b: &Loop, op: Op) -> Vec<Loop> {
    let (pa, pb): (Polyline<f64>, Polyline<f64>) = (to_pline(a), to_pline(b));
    let o = match op {
        Op::Union => BooleanOp::Or,
        Op::Intersect => BooleanOp::And,
        Op::Difference => BooleanOp::Not,
        Op::Xor => BooleanOp::Xor,
    };
    let res = pa.boolean(&pb, o);
    res.pos_plines.iter().chain(res.neg_plines.iter()).map(|p| from_pline(&p.pline)).collect()
}

/// Boolean of two regions (outer and holes each) with i_overlay on arcs flattened at `arc_tol`.
pub fn io_boolean(a: &Region, b: &Region, op: Op, arc_tol: f64) -> Vec<Loop> {
    let fa: Vec<Vec<[f64; 2]>> = a.loops().map(|l| flatten(l, arc_tol)).collect();
    let fb: Vec<Vec<[f64; 2]>> = b.loops().map(|l| flatten(l, arc_tol)).collect();
    let rule = match op {
        Op::Union => OverlayRule::Union,
        Op::Intersect => OverlayRule::Intersect,
        Op::Difference => OverlayRule::Difference,
        Op::Xor => OverlayRule::Xor,
    };
    fa.overlay(&fb, rule, FillRule::EvenOdd)
        .into_iter()
        .flat_map(|s| s.into_iter())
        .map(|c| c.into_iter().map(|p| [p[0], p[1], 0.0]).collect::<Loop>())
        .collect()
}

/// Build a one-vertex-list polyline helper used by the corpus builders.
pub fn closed(v: &[[f64; 3]]) -> Polyline<f64> {
    let mut p = Polyline::new_closed();
    for q in v {
        p.add(q[0], q[1], q[2]);
    }
    p
}
