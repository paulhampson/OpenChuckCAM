// SPDX-License-Identifier: AGPL-3.0-or-later
//! SP-7 tests, named after the requirements they bear on: OP-04 (helical and wrap strategies
//! use the unrolled development), OP-03/OP-08 (same family), NFR-05 (double precision),
//! NFR-06 (determinism), NFR-07 (invalid input gives an error, never a crash).

use sp7_2d_offsets::check::*;
use sp7_2d_offsets::corpus::{self, Case};
use sp7_2d_offsets::dev::{loop_to_angles, Cyl};
use sp7_2d_offsets::geom::*;
use sp7_2d_offsets::ops::*;
use sp7_2d_offsets::seam::*;

const N: usize = 400;

fn good_hand() -> Vec<Case> {
    corpus::hand_made().into_iter().filter(|c| !c.degenerate).collect()
}

fn assert_sweeps_ok(cases: &[Case], engine: Engine) {
    let mut bad = Vec::new();
    for c in cases {
        let r = sweep(c, engine, 7, N);
        if r.failed_passes > 0 || r.area_not_monotone > 0 {
            bad.push(format!("{} [{}]: {} of {} passes failed, dev {:.2e}, first {:?}", c.name, r.engine, r.failed_passes, r.passes, r.max_dev, r.first_failure.as_ref().map(|(k, v)| (k, v.max_dev, v.mismatches, v.panicked))));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

#[test]
fn req_op_04_cavalier_offsets_hand_corpus() {
    assert_sweeps_ok(&good_hand(), Engine::CavDirect);
}

#[test]
fn req_op_04_cavalier_offsets_random_corpus() {
    let (cases, _) = corpus::random_cases(0x5eed_0007, 120);
    assert_sweeps_ok(&cases, Engine::CavDirect);
}

#[test]
fn req_op_04_iooverlay_offsets_hand_corpus() {
    assert_sweeps_ok(&good_hand(), Engine::IoDirect);
}

#[test]
fn req_op_04_iooverlay_offsets_random_corpus() {
    let (cases, _) = corpus::random_cases(0x5eed_0007, 120);
    assert_sweeps_ok(&cases, Engine::IoDirect);
}

/// Finding: cavalier_contours offsetting the previous pass's result (instead of the original
/// every time) goes wrong when loops merge or a ring closes exactly. The product must offset
/// the original region by k * step. This test pins both halves of that finding.
#[test]
fn req_op_04_successive_offsets_unsafe_direct_safe() {
    let cases = good_hand();
    let pick: Vec<&Case> = cases.iter().filter(|c| c.name == "hole_grid_6x6_gap4" || c.name == "annulus_r20_r8").collect();
    assert_eq!(pick.len(), 2);
    let mut successive_failed = 0;
    for c in pick {
        assert_eq!(sweep(c, Engine::CavDirect, 3, N).failed_passes, 0, "{}", c.name);
        // 60 passes is enough to see the failure without the 400-pass tail
        if sweep(c, Engine::CavSuccessive, 3, 100).failed_passes > 0 {
            successive_failed += 1;
        }
    }
    assert!(successive_failed >= 1, "successive offsetting no longer fails: re-read the SP-7 finding");
}

/// Touching or overlapping loops: cavalier_contours returns a wrong region silently, so the
/// product must refuse such input before offsetting.
#[test]
fn req_nfr_07_touching_loops_refused_before_offset() {
    for c in corpus::hand_made().iter().filter(|c| c.degenerate) {
        let v = validate_region(&c.region, 2e-5);
        assert!(matches!(v, Err(Refusal::LoopsTouch { .. })), "{}: {:?}", c.name, v);
    }
    // a gap of 1e-4 is above the library's epsilon and works (see the corpus)
    for c in good_hand().iter().filter(|c| c.name.starts_with("hole_near_edge")) {
        assert_eq!(validate_region(&c.region, 2e-5), Ok(()), "{}", c.name);
    }
}

#[test]
fn req_nfr_07_invalid_input_is_refused_and_does_not_hang() {
    let sq = rect(0.0, 0.0, 10.0, 10.0);
    let cases: Vec<(&str, Region)> = vec![
        ("empty outer", Region { outer: vec![], holes: vec![] }),
        ("one vertex", Region { outer: vec![[1.0, 1.0, 0.0]], holes: vec![] }),
        ("NaN coordinate", Region { outer: vec![[0.0, 0.0, 0.0], [f64::NAN, 1.0, 0.0], [1.0, 1.0, 0.0]], holes: vec![] }),
        ("infinite bulge", Region { outer: vec![[0.0, 0.0, f64::INFINITY], [4.0, 0.0, 0.0], [4.0, 4.0, 0.0]], holes: vec![] }),
        ("bow tie", Region { outer: polygon(&[[0.0, 0.0], [10.0, 10.0], [10.0, 0.0], [0.0, 10.0]]), holes: vec![] }),
        ("hole outside", Region { outer: sq.clone(), holes: vec![circle(30.0, 30.0, 2.0)] }),
        ("hole crossing", Region { outer: sq.clone(), holes: vec![circle(10.0, 5.0, 2.0)] }),
        ("hole identical to outer", Region { outer: sq.clone(), holes: vec![reverse(&sq)] }),
    ];
    for (name, r) in cases {
        let v = validate_region(&r, 2e-5);
        assert!(v.is_err(), "{name}: not refused");
        // Raw calls: whatever happens, count it (panic is caught) but it must return.
        let loops: Vec<Loop> = r.loops().cloned().collect();
        let _ = std::panic::catch_unwind(|| cav_offset(&loops, 0.5));
        let _ = std::panic::catch_unwind(|| io_offset(&loops, 0.5, IO_ARC_TOL, IO_JOIN_TOL));
    }
}

#[test]
fn req_op_04_boolean_cavalier_arcs_kept() {
    let mut fails = Vec::new();
    let mut total = 0;
    for (i, c) in good_hand().iter().enumerate() {
        let a = &c.region.outer;
        let w = c.region.bounds();
        let shifts = [(0.0, 0.0), (1e-6, 0.0), (1e-3, 1e-3), (0.37 * (w[2] - w[0]), 0.21 * (w[3] - w[1]))];
        for (j, (dx, dy)) in shifts.iter().enumerate() {
            let b: Loop = a.iter().map(|v| [v[0] + dx, v[1] + dy, v[2]]).collect();
            let r = bool_cav(&format!("{} shift#{}", c.name, j), a, &b, 100 + i as u64, 600);
            total += r.ops;
            if r.failed > 0 {
                fails.push(format!("{}: {:?}", r.label, r.first_failure));
            }
        }
        // the pass loops: previous pass minus the next, for the first outer loops
        let p1 = cav_offset(&c.region_loops(), c.step);
        let p2 = cav_offset(&c.region_loops(), 2.0 * c.step);
        if let (Some(l1), Some(l2)) = (p1.first(), p2.first()) {
            let r = bool_cav(&format!("{} pass1 vs pass2", c.name), l1, l2, 500 + i as u64, 600);
            total += r.ops;
            if r.failed > 0 {
                fails.push(format!("{}: {:?}", r.label, r.first_failure));
            }
        }
    }
    assert!(fails.is_empty(), "{} of {} failed:\n{}", fails.len(), total, fails.join("\n"));
}

#[test]
fn req_op_04_boolean_iooverlay_regions() {
    let mut fails = Vec::new();
    for (i, c) in good_hand().iter().enumerate() {
        let w = c.region.bounds();
        let shifts = [(0.0, 0.0), (1e-6, 0.0), (1e-3, 1e-3), (0.37 * (w[2] - w[0]), 0.21 * (w[3] - w[1]))];
        for (j, (dx, dy)) in shifts.iter().enumerate() {
            let b = c.region.translated(*dx, *dy);
            let r = bool_io(&format!("{} shift#{}", c.name, j), &c.region, &b, 200 + i as u64, 600);
            if r.failed > 0 {
                fails.push(format!("{}: {:?}", r.label, r.first_failure));
            }
        }
    }
    assert!(fails.is_empty(), "{}", fails.join("\n"));
}

#[test]
fn req_nfr_06_same_result_every_run_and_at_any_thread_count() {
    let cases = good_hand();
    let run = |e: Engine, c: &Case| sweep(c, e, 11, 50).hash;
    for e in [Engine::CavDirect, Engine::IoDirect] {
        let one: Vec<u64> = cases.iter().map(|c| run(e, c)).collect();
        let again: Vec<u64> = cases.iter().map(|c| run(e, c)).collect();
        assert_eq!(one, again, "{} differs between runs", e.name());
        for threads in [2usize, 4] {
            let mut out = vec![0u64; cases.len()];
            std::thread::scope(|s| {
                let chunk = cases.len().div_ceil(threads);
                for (ci, (cs, os)) in cases.chunks(chunk).zip(out.chunks_mut(chunk)).enumerate() {
                    let _ = ci;
                    s.spawn(move || {
                        for (c, o) in cs.iter().zip(os.iter_mut()) {
                            *o = run(e, c);
                        }
                    });
                }
            });
            assert_eq!(one, out, "{} differs at {} threads", e.name(), threads);
        }
    }
}

#[test]
fn req_op_04_seam_regions_tiled() {
    let mut bad = Vec::new();
    for r in [12.5, 25.0, 50.0] {
        let cyl = Cyl { radius: r };
        let p = cyl.circumference();
        let islands = [
            (Island::Disc(4.0), 20.0, 0.5 * p - 1.0),
            (Island::Square(3.0), 40.0, -0.5 * p + 2.0),
            (Island::Star(5.0), 60.0, 0.5 * p),
            (Island::Slot(12.0, 2.0), 30.0, 0.0),
            (Island::Disc(2.0), 10.0, 0.25 * p),
        ];
        for d in [0.3, 1.0, 2.5] {
            for io in [false, true] {
                let rep = check_seam(cyl, 80.0, &islands, d, io, 5);
                let limit = if io { DEV_LIMIT_FLAT } else { DEV_LIMIT };
                if rep.panicked || rep.mismatches > 0 || rep.max_dev > limit {
                    bad.push(format!("R {r} d {d} io {io}: {rep:?}"));
                }
            }
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

#[test]
fn req_nfr_05_back_to_angles_within_tolerance() {
    let tol = 0.005;
    for r in [12.5, 25.0, 50.0] {
        let cyl = Cyl { radius: r };
        for c in good_hand() {
            for k in [1usize, 3] {
                for l in cav_offset(&c.region_loops(), c.step * k as f64) {
                    let (angles, counts) = loop_to_angles(&l, cyl, tol);
                    assert_eq!(counts.lines + counts.arcs, l.len());
                    // every converted vertex maps back onto the development loop
                    for a in &angles {
                        let p = cyl.to_dev(a[0], a[1]);
                        assert!(dist_loop(p, &l) <= tol * 1.01, "{} R {}: vertex off by {}", c.name, r, dist_loop(p, &l));
                        let q = cyl.to_angle(p);
                        assert!((q[1] - a[1]).abs() < 1e-9 * (1.0 + a[1].abs()));
                    }
                    // and so does every chord midpoint: the sagitta is within tolerance
                    for i in 0..angles.len() {
                        let (u, v) = (cyl.to_dev(angles[i][0], angles[i][1]), cyl.to_dev(angles[(i + 1) % angles.len()][0], angles[(i + 1) % angles.len()][1]));
                        let m = [0.5 * (u[0] + v[0]), 0.5 * (u[1] + v[1])];
                        assert!(dist_loop(m, &l) <= tol * 1.01, "{} R {}: chord midpoint off by {}", c.name, r, dist_loop(m, &l));
                    }
                    // an all-line loop converts with no new vertices
                    if counts.arcs == 0 {
                        assert_eq!(angles.len(), l.len());
                    }
                }
            }
        }
    }
}

// ---- the oracle itself --------------------------------------------------------------------

#[test]
fn req_nfr_05_oracle_matches_closed_forms() {
    let c = circle(1.0, 2.0, 5.0);
    assert!((signed_area(&c) - std::f64::consts::PI * 25.0).abs() < 1e-9);
    assert!((dist_loop([1.0, 2.0], &c) - 5.0).abs() < 1e-12);
    assert!((dist_loop([9.0, 2.0], &c) - 3.0).abs() < 1e-12);
    assert!((dist_loop([1.0, 2.0 - 4.0], &c) - 1.0).abs() < 1e-12);
    let rr = rounded_rect(0.0, 0.0, 20.0, 10.0, 3.0);
    let area = 200.0 - (4.0 - std::f64::consts::PI) * 9.0;
    assert!((signed_area(&rr) - area).abs() < 1e-9);
    // the point nearest a rounded corner is on the arc, not on an end of the chord
    let corner = 3.0 - 3.0 / 2f64.sqrt();
    assert!((dist_loop([corner, corner], &rr) - 0.0).abs() < 1e-12);
    assert!((dist_loop([0.0, 0.0], &rr) - (3.0 * 2f64.sqrt() - 3.0)).abs() < 1e-12);
    assert!((signed_area(&reverse(&rr)) + area).abs() < 1e-9);
}

/// The checks must be able to fail: a wrong distance and a missing loop are both caught.
#[test]
fn req_nfr_05_checks_have_teeth() {
    let c = &good_hand()[0]; // bore
    let mut rng = Rng(1);
    let right = cav_offset(&c.region_loops(), 2.0);
    assert!(check_offset(&c.region, &right, 2.0, &mut rng, 2000, DEV_LIMIT).ok(DEV_LIMIT));
    // the right loops judged against the wrong distance
    assert!(!check_offset(&c.region, &right, 2.01, &mut rng, 2000, DEV_LIMIT).ok(DEV_LIMIT));
    // a missing loop (drop the hole's offset)
    let missing: Vec<Loop> = right.iter().take(1).cloned().collect();
    assert!(!check_offset(&c.region, &missing, 2.0, &mut rng, 4000, DEV_LIMIT).ok(DEV_LIMIT));
    // a loop shifted by 0.01 mm
    let shifted: Vec<Loop> = right.iter().map(|l| l.iter().map(|v| [v[0] + 0.01, v[1], v[2]]).collect()).collect();
    assert!(!check_offset(&c.region, &shifted, 2.0, &mut rng, 2000, DEV_LIMIT).ok(DEV_LIMIT));
    // boolean: a result for the wrong operation
    let (a, b) = (Region::new(rect(0.0, 0.0, 10.0, 10.0), vec![]), Region::new(rect(5.0, 5.0, 15.0, 15.0), vec![]));
    let r = cav_boolean(&a.outer, &b.outer, Op::Union);
    let mut rng = Rng(2);
    assert_eq!(check_bool_pub(&a, &b, Op::Union, &r, &mut rng).0, 0);
    assert!(check_bool_pub(&a, &b, Op::Intersect, &r, &mut rng).0 > 0);
}

#[test]
fn req_op_04_boolean_random_pairs() {
    let (cases, _) = corpus::random_cases(0x5eed_0008, 60);
    let mut rng = Rng(77);
    let mut fails = Vec::new();
    for (i, c) in cases.iter().enumerate() {
        let w = c.region.bounds();
        let (cx, cy) = (0.5 * (w[0] + w[2]), 0.5 * (w[1] + w[3]));
        let b = c
            .region
            .rotated(cx, cy, rng.range(0.0, std::f64::consts::TAU))
            .translated(rng.range(-0.4, 0.4) * (w[2] - w[0]), rng.range(-0.4, 0.4) * (w[3] - w[1]));
        let r = bool_io(&c.name, &c.region, &b, i as u64, 600);
        if r.failed > 0 {
            fails.push(format!("i_overlay {}: {:?}", c.name, r.first_failure));
        }
        let r = bool_cav(&c.name, &c.region.outer, &b.outer, i as u64, 600);
        if r.failed > 0 {
            fails.push(format!("cavalier {}: {:?}", c.name, r.first_failure));
        }
    }
    assert!(fails.is_empty(), "{}", fails.join("\n"));
}

/// Outward offsets (profile passes around a part): the region grows, holes shrink and close.
#[test]
fn req_op_04_outward_offsets() {
    let (rand, _) = corpus::random_cases(0x5eed_0009, 40);
    let mut cases: Vec<Case> = good_hand().into_iter().filter(|c| c.name != "hole_grid_6x6_gap4").collect();
    cases.extend(rand);
    let mut bad = Vec::new();
    for c in &cases {
        for e in [Engine::CavDirect, Engine::IoDirect] {
            let r = sweep_outward(c, e, 3, 100);
            if r.failed_passes > 0 {
                bad.push(format!("{} [{}]: {} of {} passes failed, dev {:.2e}, first {:?}", c.name, r.engine, r.failed_passes, r.passes, r.max_dev, r.first_failure.as_ref().map(|(k, v)| (k, v.max_dev, v.mismatches, v.panicked))));
            }
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// Finding: when an outward offset closes a hole, i_overlay's outline leaves a zero-area
/// contour (here a 3-vertex one) where the hole was. Left in, it is a bogus loop 6.9 mm from
/// where the offset boundary is. The product drops contours below MIN_LOOP_AREA.
#[test]
fn req_op_04_iooverlay_closing_hole_leaves_zero_area_loop() {
    let (rand, _) = corpus::random_cases(0x5eed_0009, 40);
    let c = rand.iter().find(|c| c.name == "rand_holes_2").unwrap();
    let d = -8.0 * c.step;
    let raw = io_offset_raw(&c.region_loops(), d, IO_ARC_TOL, IO_JOIN_TOL);
    assert!(count_artefacts(&raw) > 0, "i_overlay no longer leaves artefacts: re-read the SP-7 finding");
    let v = check_offset(&c.region, &raw, d, &mut Rng(1), 300, DEV_LIMIT_FLAT);
    assert!(!v.ok(DEV_LIMIT_FLAT));
    let clean = io_offset(&c.region_loops(), d, IO_ARC_TOL, IO_JOIN_TOL);
    assert!(check_offset(&c.region, &clean, d, &mut Rng(1), 300, DEV_LIMIT_FLAT).ok(DEV_LIMIT_FLAT));
}
