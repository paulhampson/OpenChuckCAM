// SPDX-License-Identifier: AGPL-3.0-or-later
//! 2D offsets on the unrolled development against the independent oracle
//! (OP-04, NFR-05 to NFR-07; decision record 0016 of spike SP-7 and decision
//! record 0018 item 11).
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing
)]

use openchuck_path::unroll::{
    Development, Loop, OffsetLoops, OffsetOptions, PeriodicBand, Region, Vertex, chord_points,
};
use openchuck_testkit::region as oracle;
use openchuck_types::units::Millimetres;
use proptest::prelude::*;

fn to_lib(l: &oracle::Loop) -> Loop {
    l.iter().map(|v| Vertex::new(v[0], v[1], v[2])).collect()
}

fn from_lib(l: &Loop) -> oracle::Loop {
    l.iter().map(|v| [v.x, v.s, v.bulge]).collect()
}

fn lib_region(r: &oracle::Region) -> Region {
    Region::new(to_lib(&r.outer), r.holes.iter().map(to_lib).collect())
}

fn all_loops(o: &OffsetLoops) -> Vec<oracle::Loop> {
    o.loops().map(from_lib).collect()
}

/// Sweeps `case` inward in direct passes until nothing is left (at most
/// `max`), judging every pass against the oracle. Returns the number of passes
/// checked.
fn sweep(case: &oracle::Case, max: usize, n_random: usize) -> usize {
    let valid = lib_region(&case.region)
        .validate(&OffsetOptions::default())
        .unwrap_or_else(|e| panic!("{}: {e}", case.name));
    let mut rng = oracle::Rng(0x5eed_0007);
    let mut checked = 0;
    for k in 1..=max {
        let d = case.step * k as f64;
        let result = valid
            .offset(d)
            .unwrap_or_else(|e| panic!("{} pass {k}: {e}", case.name));
        if result.is_empty() {
            break;
        }
        let verdict = oracle::check_offset(
            &case.region,
            &all_loops(&result),
            d,
            &mut rng,
            n_random,
            oracle::DEV_LIMIT,
        );
        assert!(
            verdict.ok(oracle::DEV_LIMIT),
            "{} pass {k} (d = {d}): {verdict:?}",
            case.name
        );
        checked += 1;
    }
    checked
}

#[test]
fn req_op_04_every_hand_made_case_offsets_correctly_from_the_original() {
    let mut total = 0;
    for case in oracle::hand_made().iter().filter(|c| !c.degenerate) {
        // Far from the origin and sliver cases need many passes to vanish.
        let max = if case.step < 0.01 { 6 } else { 14 };
        let passes = sweep(case, max, 400);
        assert!(passes > 0, "{}: no pass was checked", case.name);
        total += passes;
    }
    assert!(total > 150, "{total} passes");
}

#[test]
fn req_op_04_random_regions_offset_correctly_from_the_original() {
    let mut total = 0;
    for case in oracle::random_cases(0x5eed_0007, 36) {
        total += sweep(&case, 10, 300);
    }
    assert!(total > 100, "{total} passes");
}

#[test]
fn req_op_04_the_hole_grid_that_defeats_successive_offsetting_passes_from_the_original() {
    // Step 0.4 on the 6 x 6 grid fails in cavalier when each pass is offset
    // from the previous one (decision 0016, finding 1). Here every pass is
    // offset from the original, and passes() makes the same loops.
    let case = oracle::hand_made()
        .into_iter()
        .find(|c| c.name == "hole_grid_6x6_gap4")
        .unwrap();
    let valid = lib_region(&case.region)
        .validate(&OffsetOptions::default())
        .unwrap();
    let passes = valid.passes(case.step, 14).unwrap();
    assert!(passes.len() >= 10, "{} passes", passes.len());
    let mut rng = oracle::Rng(7);
    for (i, pass) in passes.iter().enumerate() {
        let d = case.step * (i + 1) as f64;
        assert_eq!(pass, &valid.offset(d).unwrap());
        let v = oracle::check_offset(
            &case.region,
            &all_loops(pass),
            d,
            &mut rng,
            600,
            oracle::DEV_LIMIT,
        );
        assert!(v.ok(oracle::DEV_LIMIT), "pass {}: {v:?}", i + 1);
    }
    // Where the islands touch (a pass at d = 2.0, between two holes 4 mm
    // apart) the exact tangent is also right.
    let tangent = valid.offset(2.0).unwrap();
    assert!(
        oracle::check_offset(
            &case.region,
            &all_loops(&tangent),
            2.0,
            &mut rng,
            600,
            oracle::DEV_LIMIT
        )
        .ok(oracle::DEV_LIMIT)
    );
}

#[test]
fn req_nfr_07_touching_and_invalid_regions_are_refused_with_a_message() {
    let opts = OffsetOptions::default();
    for case in oracle::hand_made().iter().filter(|c| c.degenerate) {
        let e = lib_region(&case.region)
            .validate(&opts)
            .expect_err(&case.name)
            .to_string();
        for part in ["hole 1", "the outer boundary", "mm"] {
            assert!(e.contains(part), "{}: {e}", case.name);
        }
    }
    // A gap above the limit is accepted, one below it is refused.
    let at = |gap: f64| {
        lib_region(&oracle::Region::new(
            oracle::rect(0.0, 0.0, 60.0, 60.0),
            vec![oracle::circle(30.0, 10.0 + gap, 10.0)],
        ))
        .validate(&opts)
    };
    assert!(at(1e-4).is_ok());
    assert!(at(1e-5).is_err());

    let square = oracle::rect(0.0, 0.0, 10.0, 10.0);
    let cases: Vec<(&str, Region, &str)> = vec![
        (
            "bow tie",
            lib_region(&oracle::Region::new(
                oracle::polygon(&[[0.0, 0.0], [10.0, 10.0], [14.0, 0.0], [0.0, 8.0]]),
                vec![],
            )),
            "crosses itself",
        ),
        (
            "hole outside",
            lib_region(&oracle::Region::new(
                square.clone(),
                vec![oracle::circle(30.0, 5.0, 1.0)],
            )),
            "not inside the outer boundary",
        ),
        (
            "hole in a hole",
            lib_region(&oracle::Region::new(
                oracle::rect(0.0, 0.0, 60.0, 60.0),
                vec![
                    oracle::circle(30.0, 30.0, 10.0),
                    oracle::circle(30.0, 30.0, 4.0),
                ],
            )),
            "is inside hole",
        ),
        (
            "not finite",
            Region::new(
                vec![
                    Vertex::new(0.0, 0.0, 0.0),
                    Vertex::new(f64::NAN, 1.0, 0.0),
                    Vertex::new(1.0, 1.0, 0.0),
                ],
                vec![],
            ),
            "not a finite number",
        ),
        (
            "one vertex",
            Region::new(vec![Vertex::new(0.0, 0.0, 0.0)], vec![]),
            "at least two vertices",
        ),
        (
            "no area",
            Region::new(
                vec![
                    Vertex::new(0.0, 0.0, 0.0),
                    Vertex::new(5.0, 0.0, 0.0),
                    Vertex::new(10.0, 0.0, 0.0),
                ],
                vec![],
            ),
            "encloses no area",
        ),
        (
            "empty",
            Region::new(Vec::new(), vec![]),
            "at least two vertices",
        ),
    ];
    for (label, region, expect) in cases {
        let e = region.validate(&opts).expect_err(label).to_string();
        assert!(e.contains(expect), "{label}: {e}");
    }
    for bad in [
        OffsetOptions {
            tolerance: 0.0,
            ..opts
        },
        OffsetOptions {
            min_gap: f64::NAN,
            ..opts
        },
        OffsetOptions {
            check_tolerance: -1.0,
            ..opts
        },
    ] {
        assert!(
            lib_region(&oracle::Region::new(square.clone(), vec![]))
                .validate(&bad)
                .is_err()
        );
    }
    let valid = lib_region(&oracle::Region::new(square, vec![]))
        .validate(&opts)
        .unwrap();
    assert!(valid.offset(f64::NAN).is_err());
    assert!(valid.passes(0.0, 5).is_err());
    assert!(Development::new(Millimetres(0.0)).is_err());
}

#[test]
fn req_nfr_07_a_jagged_dense_loop_does_not_crash() {
    // 50,000 vertices about 5 um apart, each 0.2 mm off a circle (decision
    // 0016, finding 5). All this test claims is NFR-07: the call returns, with
    // a result or an error, and a panic inside the library does not escape.
    //
    // It does NOT claim the result is right. In a debug build the library
    // trips an internal `debug_assert` and the panic is caught (an error); in a
    // release build, which is the product, the library returns loops that are
    // wrong (8 outer and 20 hole loops instead of 1, half of the judged points
    // on the wrong side) and the result check passes them, because each vertex
    // lies on a true piece of the offset curve (OI-82).
    let n = 50_000;
    let radius = 50_000.0 * 0.005 / std::f64::consts::TAU;
    let mut rng = oracle::Rng(0x5eed_0007);
    let jagged: Loop = (0..n)
        .map(|i| {
            let t = std::f64::consts::TAU * f64::from(i) / f64::from(n);
            let r = radius + rng.range(-0.2, 0.2);
            Vertex::new(r * t.cos(), r * t.sin(), 0.0)
        })
        .collect();
    let valid = Region::new(jagged, vec![])
        .validate(&OffsetOptions::default())
        .unwrap();
    if let Err(e) = valid.offset(1.0) {
        assert!(!e.to_string().is_empty());
    }
}

#[test]
fn req_nfr_07_dense_gentle_loops_are_simplified_within_tolerance() {
    // A 20,000-vertex polygon on a circle of 20 mm (chord error under 1e-4)
    // is thinned to the simplification tolerance and still offsets right.
    let n = 20_000;
    let circle: Loop = (0..n)
        .map(|i| {
            let t = std::f64::consts::TAU * f64::from(i) / f64::from(n);
            Vertex::new(20.0 * t.cos(), 20.0 * t.sin(), 0.0)
        })
        .collect();
    let opts = OffsetOptions::default();
    let valid = Region::new(circle.clone(), vec![]).validate(&opts).unwrap();
    assert!(
        valid.vertex_count() < 1000,
        "{} vertices kept of {n}",
        valid.vertex_count()
    );
    // Every kept vertex is one of the originals, and every original is
    // within the tolerance of the simplified boundary.
    let kept: Vec<_> = valid.loops().next().unwrap();
    let simple = from_lib(&kept);
    for v in &circle {
        let d = oracle::dist_loop([v.x, v.s], &simple);
        assert!(d <= opts.tolerance + 1e-9, "{d}");
    }
    let result = valid.offset(2.0).unwrap();
    let region = oracle::Region::new(from_lib(&circle), vec![]);
    let v = oracle::check_offset(
        &region,
        &all_loops(&result),
        2.0,
        &mut oracle::Rng(3),
        500,
        oracle::DEV_LIMIT + opts.tolerance,
    );
    assert!(v.ok(oracle::DEV_LIMIT + opts.tolerance), "{v:?}");
}

#[test]
fn req_op_04_arcs_are_cut_into_chords_within_tolerance_and_lines_stay_single() {
    let dev = Development::new(Millimetres(25.0)).unwrap();
    let region = oracle::Region::new(oracle::rounded_rect(0.0, 0.0, 60.0, 90.0, 8.0), vec![]);
    let valid = lib_region(&region)
        .validate(&OffsetOptions::default())
        .unwrap();
    let pass = valid.offset(1.5).unwrap();
    let tol = 0.005;
    for l in pass.loops() {
        let points = chord_points(l, dev, tol).unwrap();
        // Back to the plane: s = R * A. Every point is on the exact loop,
        // and every chord midpoint is within the sagitta tolerance of it.
        let exact = from_lib(l);
        let plane: Vec<[f64; 2]> = points
            .iter()
            .map(|p| [p.x, dev.radius().0 * p.a.to_radians()])
            .collect();
        for q in &plane {
            assert!(oracle::dist_loop(*q, &exact) < 1e-6);
        }
        let n = plane.len();
        for i in 0..n {
            let (a, b) = (plane[i], plane[(i + 1) % n]);
            let mid = [0.5 * (a[0] + b[0]), 0.5 * (a[1] + b[1])];
            assert!(oracle::dist_loop(mid, &exact) <= tol + 1e-9);
        }
        // Four lines stay four lines; each corner arc becomes several chords.
        let arcs = l.iter().filter(|v| v.bulge != 0.0).count();
        assert_eq!(arcs, 4);
        assert!(n > l.len() + 4 * 3, "{n} points from {} vertices", l.len());
    }
    // A plain rectangle stays four moves.
    let rect = lib_region(&oracle::Region::new(
        oracle::rect(0.0, 0.0, 10.0, 20.0),
        vec![],
    ))
    .validate(&OffsetOptions::default())
    .unwrap()
    .offset(1.0)
    .unwrap();
    assert_eq!(chord_points(&rect.outers[0], dev, tol).unwrap().len(), 4);
    // A region far round the cylinder keeps A unwrapped: s = 2000 mm is
    // 12.7 turns of R 25 mm.
    let far = lib_region(&oracle::Region::new(
        oracle::rect(0.0, 2000.0, 10.0, 2020.0),
        vec![],
    ))
    .validate(&OffsetOptions::default())
    .unwrap();
    let points = chord_points(&far.loops().next().unwrap(), dev, tol).unwrap();
    assert!(points.iter().all(|p| p.a.0 > 4500.0));
    assert!(chord_points(&rect.outers[0], dev, 0.0).is_err());
}

/// Distance and side in the periodic development: the islands repeat every
/// `period` in `s`.
fn periodic_signed_distance(islands: &[oracle::Loop], period: f64, p: [f64; 2]) -> f64 {
    let mut best = f64::INFINITY;
    let mut inside = false;
    for island in islands {
        for k in -2..=2 {
            let shifted: oracle::Loop = island
                .iter()
                .map(|v| [v[0], v[1] + f64::from(k) * period, v[2]])
                .collect();
            best = best.min(oracle::dist_loop(p, &shifted));
            if oracle::Membership::new([&shifted], oracle::ARC_TOL).contains(p) {
                inside = true;
            }
        }
    }
    // Positive in free space, negative inside an island: the sign convention
    // of the band (islands are material that stays).
    if inside { -best } else { best }
}

#[test]
fn req_op_04_islands_across_the_seam_offset_correctly_in_the_periodic_development() {
    let dev = Development::new(Millimetres(12.5)).unwrap();
    let period = dev.period();
    let islands = vec![
        oracle::circle(20.0, 0.5 * period - 1.0, 4.0),
        oracle::rect(40.0, 0.5 * period + 2.0, 46.0, 0.5 * period + 8.0),
        oracle::star(30.0, -0.5 * period + 3.0, 5, 6.0, 2.4, 0.2),
        oracle::slot(10.0, 22.0, 0.1 * period, 1.5),
    ];
    let band = PeriodicBand {
        x_min: 0.0,
        x_max: 60.0,
        islands: islands.iter().map(to_lib).collect(),
    };
    let mut rng = oracle::Rng(0x5eed_0009);
    for d in [0.3, 0.8, 1.5, 2.5] {
        // Islands grown by d: the free region shrinks by d.
        let loops = band
            .offset_islands(dev, d, &OffsetOptions::default())
            .unwrap();
        assert!(!loops.is_empty());
        // Each loop's lowest s is in the middle period, and one at the seam
        // continues past it.
        let mut straddles = 0;
        for l in &loops {
            let lo = l.iter().map(|v| v.s).fold(f64::INFINITY, f64::min);
            let hi = l.iter().map(|v| v.s).fold(f64::NEG_INFINITY, f64::max);
            assert!((-0.5 * period..0.5 * period).contains(&lo));
            if hi > 0.5 * period {
                straddles += 1;
            }
            // Boundary points lie at distance d from the islands.
            for p in oracle::boundary_samples(&from_lib(l)) {
                let sd = periodic_signed_distance(&islands, period, p);
                assert!((sd - d).abs() < 2e-3, "d {d}: {sd} at {p:?}");
            }
        }
        assert!(straddles >= 1, "d {d}: no loop straddles the seam");
        // Random points away from the offset curve agree with the loops,
        // read with the periodic repeat.
        let copies: Vec<oracle::Loop> = loops
            .iter()
            .flat_map(|l| {
                (-1..=1).map(move |k| {
                    l.iter()
                        .map(|v| [v.x, v.s + f64::from(k) * period, v.bulge])
                        .collect::<oracle::Loop>()
                })
            })
            .collect();
        let membership = oracle::Membership::new(copies.iter(), oracle::ARC_TOL);
        let mut judged = 0;
        for _ in 0..2500 {
            let p = [rng.range(1.0, 59.0), rng.range(-0.5 * period, 0.5 * period)];
            let sd = periodic_signed_distance(&islands, period, p);
            if (sd - d).abs() < 3e-3 {
                continue;
            }
            judged += 1;
            // Inside a grown island when the signed distance is below d.
            assert_eq!(sd < d, membership.contains(p), "d {d}: {sd} at {p:?}");
        }
        assert!(judged > 1500);
    }
}

#[test]
fn req_op_04_islands_that_grow_into_a_ring_are_refused() {
    let dev = Development::new(Millimetres(12.5)).unwrap();
    let period = dev.period();
    let n = 6;
    let islands: Vec<Loop> = (0..n)
        .map(|i| {
            to_lib(&oracle::circle(
                20.0,
                f64::from(i) * period / f64::from(n),
                4.0,
            ))
        })
        .collect();
    let band = PeriodicBand {
        x_min: 0.0,
        x_max: 40.0,
        islands,
    };
    // Separate at first.
    assert!(
        band.offset_islands(dev, 1.0, &OffsetOptions::default())
            .is_ok()
    );
    // Grown by 3 mm the discs (4 + 3 = 7 radius, 13.1 mm apart) overlap and
    // close into a ring.
    let e = band
        .offset_islands(dev, 3.0, &OffsetOptions::default())
        .unwrap_err()
        .to_string();
    assert!(e.contains("all the way round"), "{e}");
}

#[test]
fn req_nfr_06_passes_are_identical_at_1_2_and_n_threads() {
    let case = oracle::hand_made()
        .into_iter()
        .find(|c| c.name == "rounded_rect_with_slot_hole")
        .unwrap();
    let valid = lib_region(&case.region)
        .validate(&OffsetOptions::default())
        .unwrap();
    let runs = openchuck_testkit::determinism::check(|| {
        format!("{:?}", valid.passes(case.step, 60).unwrap()).into_bytes()
    })
    .unwrap();
    assert_eq!(runs.len(), 4);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(40))]

    #[test]
    fn req_nfr_07_any_polygon_is_refused_or_offset_correctly(
        radii in prop::collection::vec(2.0f64..30.0, 3..14),
        bulges in prop::collection::vec(-0.4f64..0.4, 3..14),
        d in 0.1f64..3.0,
    ) {
        let n = radii.len().min(bulges.len());
        let outer: oracle::Loop = (0..n)
            .map(|i| {
                let t = std::f64::consts::TAU * i as f64 / n as f64;
                [radii[i] * t.cos(), radii[i] * t.sin(), bulges[i]]
            })
            .collect();
        let region = oracle::Region::new(outer, vec![]);
        match lib_region(&region).validate(&OffsetOptions::default()) {
            Err(e) => prop_assert!(!e.to_string().is_empty()),
            Ok(valid) => match valid.offset(d) {
                Err(e) => prop_assert!(!e.to_string().is_empty()),
                Ok(result) => {
                    let v = oracle::check_offset(
                        &region, &all_loops(&result), d, &mut oracle::Rng(5), 200, oracle::DEV_LIMIT);
                    prop_assert!(v.ok(oracle::DEV_LIMIT), "{v:?}");
                }
            },
        }
    }
}

#[test]
fn req_nfr_07_the_result_check_has_teeth() {
    // A right result passes; a loop shifted by half a millimetre, a loop
    // missing its inner part and a point that is not finite are refused with
    // a message that says the result is not trusted.
    let case = oracle::hand_made()
        .into_iter()
        .find(|c| c.name == "rounded_rect_with_slot_hole")
        .unwrap();
    let valid = lib_region(&case.region)
        .validate(&OffsetOptions::default())
        .unwrap();
    let good = valid.offset(1.0).unwrap();
    valid.check_offset(&good, 1.0).unwrap();
    let shifted = OffsetLoops {
        outers: good
            .outers
            .iter()
            .map(|l| {
                l.iter()
                    .map(|v| Vertex::new(v.x + 0.5, v.s, v.bulge))
                    .collect()
            })
            .collect(),
        holes: good.holes.clone(),
    };
    let wrong_distance = valid.check_offset(&good, 1.5);
    for bad in [wrong_distance, valid.check_offset(&shifted, 1.0)] {
        let e = bad.unwrap_err().to_string();
        assert!(e.contains("not trusted"), "{e}");
    }
    let mut broken = good.clone();
    broken.holes[0][0].x = f64::NAN;
    assert!(valid.check_offset(&broken, 1.0).is_err());
}
