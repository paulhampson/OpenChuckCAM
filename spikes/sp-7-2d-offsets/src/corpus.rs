// SPDX-License-Identifier: AGPL-3.0-or-later
//! The corpus of awkward regions, in the unrolled plane (mm). Hand-made cases first, then
//! seeded random ones, so every run sees the same shapes.

use crate::geom::*;
use std::f64::consts::{PI, TAU};

pub struct Case {
    pub name: String,
    pub region: Region,
    /// Pass spacing for the pocketing sweep (mm).
    pub step: f64,
    /// The case is deliberately degenerate (touching, invalid): a clear result or an error is
    /// acceptable, a panic or a wrong region is not.
    pub degenerate: bool,
}

fn case(name: &str, region: Region, step: f64) -> Case {
    Case { name: name.into(), region, step, degenerate: false }
}
fn degenerate(name: &str, region: Region, step: f64) -> Case {
    Case { name: name.into(), region, step, degenerate: true }
}

pub fn hand_made() -> Vec<Case> {
    let mut v = Vec::new();
    // 1. A cylinder (R 25, circumference 157 mm) patch with a bore.
    v.push(case("bore", Region::new(rect(0.0, 0.0, 60.0, 120.0), vec![circle(30.0, 60.0, 8.0)]), 0.5));
    // 2. Many holes close together: offset islands merge as the passes grow.
    let mut holes = Vec::new();
    for i in 0..6 {
        for j in 0..6 {
            holes.push(circle(10.0 + 14.0 * i as f64, 10.0 + 14.0 * j as f64, 5.0));
        }
    }
    v.push(case("hole_grid_6x6_gap4", Region::new(rect(0.0, 0.0, 90.0, 90.0), holes), 0.4));
    // 3. Rounded rectangle (arcs on the boundary).
    v.push(case("rounded_rect_r6", Region::new(rounded_rect(0.0, 0.0, 80.0, 50.0, 6.0), vec![]), 0.5));
    v.push(case(
        "rounded_rect_with_slot_hole",
        Region::new(rounded_rect(0.0, 0.0, 80.0, 50.0, 6.0), vec![slot(20.0, 60.0, 25.0, 4.0)]),
        0.5,
    ));
    // 4. A thin slot pocket: the offset vanishes at half the width.
    v.push(case("slot_w6", Region::new(slot(0.0, 40.0, 0.0, 3.0), vec![]), 0.25));
    // 5. Dumbbell: two blocks and a narrow neck (the neck closes first and splits the region).
    v.push(case(
        "dumbbell_neck4",
        Region::new(
            polygon(&[
                [0.0, 0.0], [30.0, 0.0], [30.0, 13.0], [50.0, 13.0], [50.0, 0.0], [80.0, 0.0],
                [80.0, 30.0], [50.0, 30.0], [50.0, 17.0], [30.0, 17.0], [30.0, 30.0], [0.0, 30.0],
            ]),
            vec![],
        ),
        0.5,
    ));
    // 6. Deep concave stars: reflex corners grow arcs in an offset, sharp tips vanish.
    v.push(case("star5_deep", Region::new(star(0.0, 0.0, 5, 30.0, 6.0, 0.3), vec![]), 0.3));
    v.push(case("star12", Region::new(star(0.0, 0.0, 12, 30.0, 18.0, 0.0), vec![circle(0.0, 0.0, 6.0)]), 0.3));
    // 7. Hole nearly touching the boundary.
    for (g, nm) in [(1e-1, "1e-1"), (1e-2, "1e-2"), (1e-3, "1e-3"), (1e-4, "1e-4")] {
        v.push(case(
            &format!("hole_near_edge_gap_{nm}"),
            Region::new(rect(0.0, 0.0, 60.0, 60.0), vec![circle(30.0, 10.0 + g, 10.0)]),
            0.5,
        ));
    }
    v.push(degenerate(
        "hole_tangent_to_edge_gap_0",
        Region::new(rect(0.0, 0.0, 60.0, 60.0), vec![circle(30.0, 10.0, 10.0)]),
        0.5,
    ));
    v.push(degenerate(
        "hole_tangent_to_edge_gap_1e-7",
        Region::new(rect(0.0, 0.0, 60.0, 60.0), vec![circle(30.0, 10.0 + 1e-7, 10.0)]),
        0.5,
    ));
    // 8. Two holes nearly touching each other.
    v.push(case(
        "two_holes_gap_1e-3",
        Region::new(rect(0.0, 0.0, 60.0, 40.0), vec![circle(20.0, 20.0, 8.0), circle(36.001, 20.0, 8.0)]),
        0.5,
    ));
    // 9. Sliver narrower than any tool.
    v.push(case("sliver_w0.02", Region::new(rect(0.0, 0.0, 50.0, 0.02), vec![]), 0.005));
    // 10. Sharp spike (1 degree).
    v.push(case(
        "spike_1deg",
        Region::new(polygon(&[[0.0, 0.0], [40.0, 0.35], [40.0, 0.0], [20.0, -15.0], [-10.0, -5.0], [-10.0, 15.0]]), vec![]),
        0.3,
    ));
    // 11. Redundant vertices: duplicates, collinear points, zero-length arcs, tiny bulges.
    v.push(case(
        "redundant_vertices",
        Region::new(
            vec![
                [0.0, 0.0, 0.0], [0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [20.0, 0.0, 1e-12], [40.0, 0.0, 0.0],
                [40.0, 10.0, 0.0], [40.0, 30.0, 1e-9], [40.0, 30.0, 0.0], [0.0, 30.0, 0.0], [0.0, 15.0, 0.0],
            ],
            vec![],
        ),
        0.5,
    ));
    // 12. Far from the origin: s = 2000 mm is about 12.7 turns of an R 25 cylinder.
    v.push(case(
        "far_from_origin_s2000",
        Region::new(rounded_rect(0.0, 2000.0, 60.0, 2060.0, 8.0), vec![circle(30.0, 2030.0, 9.0)]),
        0.5,
    ));
    // 13. Tiny hole (smaller than the pass step).
    v.push(case("tiny_hole_r0.3", Region::new(rect(0.0, 0.0, 30.0, 30.0), vec![circle(15.0, 15.0, 0.3)]), 0.5));
    // 14. Whole circle with arcs only.
    v.push(case("disc_r20", Region::new(circle(0.0, 0.0, 20.0), vec![]), 0.5));
    v.push(case("annulus_r20_r8", Region::new(circle(0.0, 0.0, 20.0), vec![circle(0.0, 0.0, 8.0)]), 0.5));
    // 15. Off-centre hole in a disc, nearly concentric: offsets collapse the thin side first.
    v.push(case("disc_off_centre_hole", Region::new(circle(0.0, 0.0, 20.0), vec![circle(1.2, 0.0, 17.0)]), 0.2));
    v
}

/// True if no two non-adjacent edges of the flattened loop cross (own check for the random
/// cases, independent of the libraries).
pub fn is_simple(l: &Loop) -> bool {
    let p = flatten(l, 0.01);
    let n = p.len();
    fn cross(a: Pt, b: Pt, c: Pt) -> f64 {
        (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
    }
    for i in 0..n {
        for j in i + 2..n {
            if i == 0 && j == n - 1 {
                continue;
            }
            let (a, b, c, d) = (p[i], p[(i + 1) % n], p[j], p[(j + 1) % n]);
            let (d1, d2, d3, d4) = (cross(a, b, c), cross(a, b, d), cross(c, d, a), cross(c, d, b));
            if d1 * d2 < 0.0 && d3 * d4 < 0.0 {
                return false;
            }
        }
    }
    true
}

/// Seeded random cases: star-shaped polygons (some with arcs on their edges) and rectangles
/// with random non-overlapping holes. Candidates that fail the own validity check are dropped,
/// and the number dropped is returned so the report can say so.
pub fn random_cases(seed: u64, count: usize) -> (Vec<Case>, usize) {
    let mut rng = Rng(seed);
    let mut out = Vec::new();
    let mut dropped = 0;
    let mut i = 0;
    while out.len() < count {
        i += 1;
        let kind = i % 3;
        if kind == 0 || kind == 1 {
            let n = 5 + (rng.next_u64() % 9) as usize;
            let arcs = kind == 1;
            let mut v: Loop = Vec::new();
            for k in 0..n {
                let a = TAU * (k as f64 + rng.range(-0.3, 0.3)) / n as f64;
                let r = rng.range(8.0, 30.0);
                let bulge = if arcs { rng.range(-0.25, 0.25) } else { 0.0 };
                v.push([r * a.cos(), r * a.sin(), bulge]);
            }
            if !is_simple(&v) {
                dropped += 1;
                continue;
            }
            let mut holes = Vec::new();
            if rng.unit() < 0.5 {
                let r = rng.range(1.0, 4.0);
                let h = circle(rng.range(-2.0, 2.0), rng.range(-2.0, 2.0), r);
                if dist_loop([0.0, 0.0], &v) > r + 3.0 {
                    holes.push(h);
                }
            }
            let nm = if arcs { "rand_arc_poly" } else { "rand_poly" };
            out.push(case(&format!("{nm}_{i}"), Region::new(v, holes), rng.range(0.2, 0.8)));
        } else {
            let (w, h) = (rng.range(30.0, 80.0), rng.range(30.0, 80.0));
            let mut holes: Vec<Loop> = Vec::new();
            let mut centres: Vec<(f64, f64, f64)> = Vec::new();
            for _ in 0..(2 + rng.next_u64() % 10) {
                let r = rng.range(1.5, 7.0);
                let (cx, cy) = (rng.range(r + 1.0, w - r - 1.0), rng.range(r + 1.0, h - r - 1.0));
                if centres.iter().all(|&(x, y, q)| (x - cx).hypot(y - cy) > q + r + 0.5) {
                    centres.push((cx, cy, r));
                    holes.push(circle(cx, cy, r));
                }
            }
            out.push(case(&format!("rand_holes_{i}"), Region::new(rect(0.0, 0.0, w, h), holes), rng.range(0.3, 1.0)));
        }
    }
    let _ = PI;
    (out, dropped)
}
