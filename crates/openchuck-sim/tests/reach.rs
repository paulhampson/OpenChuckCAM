// SPDX-License-Identifier: AGPL-3.0-or-later
//! Material left that no tool can reach (SIM-05, as amended by the owner on
//! 1 Nov 2026; issue 478).
//!
//! On a machine whose tool axis always lies across the rotary axis, a sample
//! whose empty side is closed off from the outside in its own section across
//! X cannot be reached by any tool, whatever its size or lean: a tool's
//! section by that plane is one strip from the point out past the stock.
//! The stand-in for the owner's carving is a tube along the rotary axis: its
//! bore is such a pocket in every section.

// Test code may unwrap, index and panic (NFR-07 applies to library code).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_cmp
)]

use std::f64::consts::TAU;

use openchuck_sim::{
    Deviation, DeviationOptions, DexelStock, ReachClass, ReachOptions, SampleOn, classify_reach,
    compare, sample_areas, triangle_values, triangle_worst,
};
use openchuck_types::geometry::{Aabb, Point3};
use openchuck_types::job::{CancelToken, JobContext, NoProgress};
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::stock::Stock;

const CELL: f64 = 0.1;

/// A closed tube along X from `x0` to `x1`, outer radius `outer`, bore
/// radius `inner`, as `n` facets round.
fn tube(outer: f64, inner: f64, x0: f64, x1: f64, n: usize) -> TriangleMesh {
    let ring = |x: f64, r: f64| {
        (0..n).map(move |i| {
            let t = TAU * i as f64 / n as f64;
            Point3::new(x, r * t.cos(), r * t.sin())
        })
    };
    let mut vertices: Vec<Point3> = Vec::new();
    vertices.extend(ring(x0, outer));
    vertices.extend(ring(x1, outer));
    vertices.extend(ring(x0, inner));
    vertices.extend(ring(x1, inner));
    let id = |k: usize| u32::try_from(k).unwrap();
    let (o0, o1, i0, i1) = (0, n, 2 * n, 3 * n);
    let mut triangles = Vec::new();
    for i in 0..n {
        let j = (i + 1) % n;
        // Outer side, facing out.
        triangles.push([id(o0 + i), id(o0 + j), id(o1 + i)]);
        triangles.push([id(o0 + j), id(o1 + j), id(o1 + i)]);
        // The bore, facing the axis.
        triangles.push([id(i0 + i), id(i1 + i), id(i0 + j)]);
        triangles.push([id(i0 + j), id(i1 + i), id(i1 + j)]);
        // The end at x0, facing -X; the end at x1, facing +X.
        triangles.push([id(o0 + i), id(i0 + i), id(i0 + j)]);
        triangles.push([id(o0 + i), id(i0 + j), id(o0 + j)]);
        triangles.push([id(o1 + i), id(o1 + j), id(i1 + j)]);
        triangles.push([id(o1 + i), id(i1 + j), id(i1 + i)]);
    }
    TriangleMesh {
        vertices,
        triangles,
    }
}

/// A tube cut open along its length by a slit `width` wide at A = 0.
fn slit_tube(outer: f64, inner: f64, x0: f64, x1: f64, width: f64, n: usize) -> TriangleMesh {
    let (po, pi) = ((width / 2.0 / outer).asin(), (width / 2.0 / inner).asin());
    let mut section = Vec::new();
    for k in 0..=n {
        let t = po + (TAU - 2.0 * po) * k as f64 / n as f64;
        section.push((outer * t.cos(), outer * t.sin()));
    }
    for k in 0..=n {
        let t = (TAU - pi) - (TAU - 2.0 * pi) * k as f64 / n as f64;
        section.push((inner * t.cos(), inner * t.sin()));
    }
    openchuck_testkit::parts::prism(&section, x0, x1, 4)
}

/// The signed volume of a closed mesh: positive when its normals face out.
fn volume(mesh: &TriangleMesh) -> f64 {
    (0..mesh.triangles.len())
        .map(|t| {
            let [a, b, c] = mesh.triangle(t).unwrap();
            let (a, b, c) = (a - Point3::ORIGIN, b - Point3::ORIGIN, c - Point3::ORIGIN);
            a.dot(b.cross(c)) / 6.0
        })
        .sum()
}

/// An uncut box of stock round the part, so every sample has material left.
fn box_stock(half: f64, x0: f64, x1: f64) -> DexelStock {
    let stock = Stock::Box {
        bounds: Aabb {
            min: Point3::new(x0, -half, -half),
            max: Point3::new(x1, half, half),
        },
    };
    DexelStock::from_stock(&stock, None, CELL).unwrap()
}

fn measured(part: &TriangleMesh, stock: &DexelStock) -> Deviation {
    compare(stock, part, &DeviationOptions::for_cell(CELL)).unwrap()
}

fn classify(part: &TriangleMesh, d: &Deviation, smallest_tool: f64) -> Vec<ReachClass> {
    let never = CancelToken::new();
    classify_reach(
        &[part],
        d,
        &ReachOptions::for_cell(CELL, smallest_tool),
        &JobContext::new(&never, &NoProgress),
    )
    .unwrap()
    .classes
}

/// Whether a sample faces the rotary axis, faces away from it, or faces
/// along it.
fn facing(p: &openchuck_sim::DeviationPoint) -> i8 {
    let r = p.point.y.hypot(p.point.z);
    let radial = (p.normal.y * p.point.y + p.normal.z * p.point.z) / r;
    if radial < -0.9 {
        -1
    } else if radial > 0.9 {
        1
    } else {
        0
    }
}

#[test]
fn req_sim_05_the_stand_in_tube_is_closed_and_faces_out() {
    let part = tube(6.0, 3.0, 0.0, 8.0, 48);
    part.validate().unwrap();
    assert!(volume(&part) > 0.0, "the tube's normals face out");
}

#[test]
fn req_sim_05_a_bore_along_the_rotary_axis_is_classed_enclosed() {
    let part = tube(6.0, 3.0, 0.0, 8.0, 48);
    let d = measured(&part, &box_stock(8.0, -2.0, 10.0));
    let classes = classify(&part, &d, 6.0);
    assert_eq!(classes.len(), d.points.len());
    let (mut bore, mut outside, mut ends) = (0, 0, 0);
    for (p, c) in d.points.iter().zip(&classes) {
        assert!(p.value > CELL, "the uncut stock leaves material everywhere");
        match facing(p) {
            // A sample on the rim of the bore's opening, at an end of the
            // tube, is touched by a tool beyond the end: not enclosed. Within
            // a slab of the ends the grid may say either.
            -1 if p.point.x == 0.0 || p.point.x == 8.0 => {
                assert_ne!(*c, ReachClass::Enclosed, "a rim sample at {:?}", p.point);
            }
            -1 if p.point.x < 2.0 * CELL || p.point.x > 8.0 - 2.0 * CELL => {}
            -1 => {
                bore += 1;
                assert_eq!(*c, ReachClass::Enclosed, "a bore sample at {:?}", p.point);
            }
            1 => {
                outside += 1;
                assert_eq!(*c, ReachClass::Open, "an outside sample at {:?}", p.point);
            }
            _ => {
                ends += 1;
                // An end face is reached along X, which this test does not
                // decide: never guessed (NFR-12).
                assert_eq!(*c, ReachClass::Undecided, "an end sample at {:?}", p.point);
            }
        }
    }
    assert!(bore > 1000 && outside > 1000 && ends > 1000);
}

#[test]
fn req_sim_05_a_bore_open_through_a_slit_wider_than_two_pitches_is_not_enclosed() {
    let part = slit_tube(6.0, 3.0, 0.0, 8.0, 1.0, 64);
    part.validate().unwrap();
    assert!(volume(&part) > 0.0);
    let d = measured(&part, &box_stock(8.0, -2.0, 10.0));
    let classes = classify(&part, &d, 0.5);
    let bore: Vec<_> = d
        .points
        .iter()
        .zip(&classes)
        .filter(|(p, _)| facing(p) == -1)
        .collect();
    assert!(bore.len() > 1000);
    assert!(
        bore.iter().all(|(_, c)| **c == ReachClass::Open),
        "a slit 1 mm wide opens the bore to the outside"
    );
}

#[test]
fn req_sim_05_req_nfr_12_a_tool_narrower_than_two_pitches_leaves_every_leftover_undecided() {
    let part = tube(6.0, 3.0, 0.0, 8.0, 48);
    let d = measured(&part, &box_stock(8.0, -2.0, 10.0));
    // A tool that could pass an opening the grid reads as closed.
    let classes = classify(&part, &d, 1.5 * CELL);
    assert!(classes.iter().all(|c| *c == ReachClass::Undecided));
}

#[test]
fn req_sim_05_material_within_the_tolerance_or_not_counted_is_not_classed() {
    let part = tube(6.0, 3.0, 0.0, 8.0, 48);
    let mut d = measured(&part, &box_stock(8.0, -2.0, 10.0));
    for (i, p) in d.points.iter_mut().enumerate() {
        match i % 3 {
            0 => p.value = CELL / 2.0,
            1 => p.on = SampleOn::Inside,
            _ => p.value = -1.0,
        }
    }
    let classes = classify(&part, &d, 6.0);
    assert!(classes.iter().all(|c| *c == ReachClass::NotLeft));
}

#[test]
fn req_nfr_07_a_tube_that_is_not_closed_never_classes_enclosed() {
    let mut part = tube(6.0, 3.0, 0.0, 8.0, 48);
    // Without its end at x1, rays along X cross it an odd number of times.
    part.triangles.retain(|t| {
        let x = |i: u32| part.vertices[i as usize].x;
        !(x(t[0]) == 8.0 && x(t[1]) == 8.0 && x(t[2]) == 8.0)
    });
    let d = measured(&part, &box_stock(8.0, -2.0, 10.0));
    let classes = classify(&part, &d, 6.0);
    assert!(classes.iter().all(|c| *c != ReachClass::Enclosed));
}

#[test]
fn req_nfr_06_reach_is_the_same_at_any_thread_count() {
    let part = tube(6.0, 3.0, 0.0, 8.0, 48);
    let d = measured(&part, &box_stock(8.0, -2.0, 10.0));
    let runs: Vec<Vec<ReachClass>> = [1, 3]
        .iter()
        .map(|&threads| {
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap()
                .install(|| classify(&part, &d, 6.0))
        })
        .collect();
    assert_eq!(runs[0], runs[1]);
}

#[test]
fn req_nfr_07_invalid_reach_options_are_refused() {
    let part = tube(6.0, 3.0, 0.0, 8.0, 12);
    let d = measured(&part, &box_stock(8.0, -2.0, 10.0));
    let never = CancelToken::new();
    let context = JobContext::new(&never, &NoProgress);
    for pitch in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        let mut options = ReachOptions::for_cell(CELL, 6.0);
        options.pitch = pitch;
        let e = classify_reach(&[&part], &d, &options, &context).unwrap_err();
        assert!(e.to_string().contains("pitch"), "{e}");
    }
    for bad in [-1.0, f64::NAN, f64::INFINITY] {
        let mut options = ReachOptions::for_cell(CELL, 6.0);
        options.left_above = bad;
        let e = classify_reach(&[&part], &d, &options, &context).unwrap_err();
        assert!(e.to_string().contains("left_above"), "{e}");
        let options = ReachOptions::for_cell(CELL, bad);
        let e = classify_reach(&[&part], &d, &options, &context).unwrap_err();
        assert!(e.to_string().contains("smallest_tool"), "{e}");
    }
    // Nothing to cast against: nothing is enclosed, and nothing fails.
    let reach = classify_reach(&[], &d, &ReachOptions::for_cell(CELL, 6.0), &context).unwrap();
    assert!(reach.classes.iter().all(|c| *c == ReachClass::Undecided));
}

#[test]
fn req_ui_09_reach_stops_when_cancelled() {
    let part = tube(6.0, 3.0, 0.0, 8.0, 12);
    let d = measured(&part, &box_stock(8.0, -2.0, 10.0));
    let cancel = CancelToken::new();
    cancel.cancel();
    let e = classify_reach(
        &[&part],
        &d,
        &ReachOptions::for_cell(CELL, 6.0),
        &JobContext::new(&cancel, &NoProgress),
    )
    .unwrap_err();
    assert!(matches!(e, openchuck_types::Error::Cancelled), "{e}");
}

#[test]
fn req_sim_05_sample_areas_add_up_to_the_surface() {
    let part = tube(6.0, 3.0, 0.0, 8.0, 48);
    let d = measured(&part, &box_stock(8.0, -2.0, 10.0));
    let areas = sample_areas(&d, &part, CELL).unwrap();
    assert_eq!(areas.len(), d.points.len());
    let surface: f64 = (0..part.triangles.len())
        .map(|t| {
            let [a, b, c] = part.triangle(t).unwrap();
            (b - a).cross(c - a).length() / 2.0
        })
        .sum();
    let total: f64 = areas.iter().sum();
    assert!(
        (total - surface).abs() < 1e-6 * surface,
        "{total} {surface}"
    );
    // A mesh the comparison was not made on is refused, not guessed:
    // longer (more samples than it holds) or with fewer triangles.
    let other = tube(6.0, 3.0, 0.0, 9.0, 48);
    assert!(sample_areas(&d, &other, CELL).is_err());
    let mut fewer = part.clone();
    fewer.triangles.truncate(part.triangles.len() / 2);
    assert!(sample_areas(&d, &fewer, CELL).is_err());
    // A degenerate triangle has no samples and stands for no area.
    let mut with_degenerate = part.clone();
    with_degenerate.triangles.push([0, 0, 0]);
    let d = measured(&with_degenerate, &box_stock(8.0, -2.0, 10.0));
    let total: f64 = sample_areas(&d, &with_degenerate, CELL)
        .unwrap()
        .iter()
        .sum();
    assert!(
        (total - surface).abs() < 1e-6 * surface,
        "{total} {surface}"
    );
}

#[test]
fn req_sim_05_each_triangle_names_the_sample_it_is_coloured_by() {
    let part = tube(6.0, 3.0, 0.0, 8.0, 48);
    let d = measured(&part, &box_stock(8.0, -2.0, 10.0));
    let worst = triangle_worst(&d, &part, CELL).unwrap();
    let values = triangle_values(&d, &part, CELL).unwrap();
    assert_eq!(worst.len(), values.len());
    for (w, v) in worst.iter().zip(&values) {
        let named = w.map(|i| (d.points[i].value, d.points[i].at_least));
        assert_eq!(named, *v);
    }
}
