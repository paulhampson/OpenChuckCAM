// SPDX-License-Identifier: AGPL-3.0-or-later
//! The order of the passes (OP-12, OP-14, OP-15, SET-09, SIM-08, GC-07,
//! NFR-06; issue #469; decision record 0639): with "by region" the turns at
//! faces across the axis and the passes on hidden surfaces are cut beside
//! the passes of their region instead of after them all. The checks are on
//! the crankshaft (`stl/crankshaft.stl`), whose webs have faces across the
//! axis and surfaces hidden from the drive.
#![allow(
    clippy::unwrap_used,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation
)]

mod support;

use std::collections::BTreeMap;
use std::path::PathBuf;

use openchuck_path::params::names;
use openchuck_path::{
    GeneratedPath, HelicalFinishing, MeshSurface, ParameterValue, SimultaneousFinishing, Strategy,
    order,
};
use openchuck_types::ir::MoveKind;
use openchuck_types::mesh::TriangleMesh;
use support::{Fixture, ball, generate, read_binary_stl, run};

const DIAMETER: f64 = 3.175;

fn crankshaft() -> TriangleMesh {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/stl/crankshaft.stl");
    read_binary_stl(&path)
}

fn values(pass_order: &str) -> BTreeMap<String, ParameterValue> {
    [
        (names::STEPOVER, ParameterValue::Number(1.0)),
        (names::FEED, ParameterValue::Number(600.0)),
        (names::PLUNGE_FEED, ParameterValue::Number(150.0)),
        (names::TOLERANCE, ParameterValue::Number(0.02)),
        (
            order::PARAMETER,
            ParameterValue::Text(pass_order.to_owned()),
        ),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v))
    .collect()
}

/// The cuts: each run of fed moves that are not links, as its tips and A
/// (by whole turns, as the assembly places them), rounded, in order.
fn cuts(path: &GeneratedPath) -> Vec<Vec<[i64; 4]>> {
    let mut out = Vec::new();
    let mut open: Option<Vec<[i64; 4]>> = None;
    for s in &path.group.segments {
        if matches!(s.kind, MoveKind::Rapid | MoveKind::Linking) {
            out.extend(open.take());
            continue;
        }
        let r = |v: f64| (v * 1e4).round() as i64;
        let a = s.end.a.0.rem_euclid(360.0);
        let a = if a > 360.0 - 5e-5 { 0.0 } else { a };
        open.get_or_insert_with(Vec::new).push([
            r(s.end.tip.x),
            r(s.end.tip.y),
            r(s.end.tip.z),
            r(a),
        ]);
    }
    out.extend(open);
    out
}

fn duration(path: &GeneratedPath) -> f64 {
    path.group.segments.iter().map(|s| s.duration.0).sum()
}

/// Generates `strategy` on the crankshaft with the tool kept out of X below
/// 2 (a held region at the start, SET-09) in both orders.
fn both(strategy: &dyn Strategy) -> (GeneratedPath, GeneratedPath) {
    let f = Fixture::new(
        MeshSurface::new(&crankshaft()).unwrap(),
        18.0,
        &ball(DIAMETER),
    );
    let keep_out = [(-20.0, 2.0)];
    let input = openchuck_path::StrategyInput {
        keep_out: &keep_out,
        ..f.input()
    };
    let get = |o: &str| run(|ctx| generate(strategy, &input, &values(o), ctx)).unwrap();
    (get(order::AFTER_MAIN), get(order::BY_REGION))
}

/// Same cuts, each unchanged and in its own direction, whatever the order
/// (OP-12: the material left is the same; OP-15: no cut reversed), none of
/// them in the held region (SET-09).
fn assert_same_cuts(after: &GeneratedPath, by_region: &GeneratedPath) {
    let (mut a, mut b) = (cuts(after), cuts(by_region));
    assert!(a.len() > 4, "{}", a.len());
    // The order did change: the turns are not all after the main passes.
    assert_ne!(a, b);
    a.sort();
    b.sort();
    assert_eq!(a, b);
    for s in &by_region.group.segments {
        if s.kind.is_fed() {
            assert!(s.end.tip.x >= 2.0 - 1e-6, "{:?}", s.end.tip);
        }
    }
}

#[test]
fn req_op_12_req_op_15_req_set_09_simultaneous_cuts_are_the_same_in_either_order() {
    let (after, by_region) = both(&SimultaneousFinishing);
    assert_same_cuts(&after, &by_region);
    eprintln!(
        "simultaneous: {:.1} s after the main passes, {:.1} s by region",
        duration(&after),
        duration(&by_region)
    );
}

#[test]
fn req_op_12_req_op_15_req_set_09_helical_cuts_are_the_same_in_either_order() {
    let (after, by_region) = both(&HelicalFinishing);
    assert_same_cuts(&after, &by_region);
    eprintln!(
        "helical: {:.1} s after the main passes, {:.1} s by region",
        duration(&after),
        duration(&by_region)
    );
}

#[test]
fn req_gc_07_req_nfr_06_by_region_is_identical_at_1_2_and_n_threads() {
    let f = Fixture::new(
        MeshSurface::new(&crankshaft()).unwrap(),
        18.0,
        &ball(DIAMETER),
    );
    let v = values(order::BY_REGION);
    let runs = openchuck_testkit::determinism::check(|| {
        let path = run(|ctx| generate(&SimultaneousFinishing, &f.input(), &v, ctx)).unwrap();
        format!("{:?} {:?}", path.group, path.notes).into_bytes()
    })
    .unwrap();
    assert_eq!(runs.len(), 4);
}

/// How far the cuts' X centres travel from one cut to the next.
fn sweep(path: &GeneratedPath) -> f64 {
    let centres: Vec<f64> = cuts(path)
        .iter()
        .map(|c| {
            let (lo, hi) = c
                .iter()
                .fold((i64::MAX, i64::MIN), |(l, h), p| (l.min(p[0]), h.max(p[0])));
            0.5 * (lo + hi) as f64 * 1e-4
        })
        .collect();
    centres.windows(2).map(|w| (w[1] - w[0]).abs()).sum()
}

#[test]
fn req_op_14_req_sim_08_the_crankshaft_extra_passes_are_cut_beside_their_region() {
    // The owner's choice (decision record 0639): by region the tool goes
    // back along the part less, and the estimated time is at most one per
    // cent longer.
    for strategy in [
        &SimultaneousFinishing as &dyn Strategy,
        &HelicalFinishing as &dyn Strategy,
    ] {
        let (after, by_region) = both(strategy);
        let (s0, s1) = (sweep(&after), sweep(&by_region));
        let (t0, t1) = (duration(&after), duration(&by_region));
        eprintln!(
            "{}: X sweep {s0:.1} -> {s1:.1} mm, time {t0:.1} -> {t1:.1} s",
            strategy.name()
        );
        assert!(s1 < 0.8 * s0, "{s0} {s1}");
        assert!(t1 <= 1.01 * t0, "{t0} {t1}");
        // The user is told, and the old order says nothing new.
        let told = |p: &GeneratedPath| {
            p.notes
                .iter()
                .any(|n| n.message.contains("pass order by region"))
        };
        assert!(told(&by_region), "{:?}", by_region.notes);
        assert!(!told(&after));
    }
}
