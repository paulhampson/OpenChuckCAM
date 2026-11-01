// SPDX-License-Identifier: AGPL-3.0-or-later
//! Links between passes of axial and indexed finishing: the option to keep
//! them low (OP-14, SET-06, SET-09, NFR-07, NFR-12, GC-07; issue #436;
//! decision 0510).
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing
)]

mod support;
use support::generate;

use std::collections::BTreeMap;

use openchuck_path::axial::parameter;
use openchuck_path::params::names;
use openchuck_path::{
    AxialFinishing, GeneratedPath, IndexedFinishing, MeshSurface, ParameterValue, Strategy,
    StrategyInput,
};
use openchuck_testkit::parts::{Cylinder, RadialPart};
use openchuck_types::ir::MoveKind;
use support::{Fixture, ball, run};

/// The part's radius; the fixture's stock is one millimetre more, its
/// clearance cylinder five more than the stock.
const PART: f64 = 15.0;

fn num(x: f64) -> ParameterValue {
    ParameterValue::Number(x)
}

fn text(s: &str) -> ParameterValue {
    ParameterValue::Text(s.to_owned())
}

fn values(extra: &[(&str, ParameterValue)]) -> BTreeMap<String, ParameterValue> {
    let mut v: BTreeMap<String, ParameterValue> = [
        (names::STEPOVER, 4.0),
        (names::FEED, 300.0),
        (names::PLUNGE_FEED, 100.0),
    ]
    .into_iter()
    .map(|(k, x)| (k.to_owned(), num(x)))
    .collect();
    for (k, x) in extra {
        v.insert((*k).to_owned(), x.clone());
    }
    v
}

fn fixture_with_stock(stock_radius: f64) -> Fixture {
    let part = Cylinder {
        radius: PART,
        length: 20.0,
    };
    let surface = MeshSurface::new(&part.mesh(3, 360)).unwrap();
    let mut f = Fixture::new(surface, stock_radius, &ball(6.0));
    // Fixed clearance, so the stock can be taken up to it.
    f.clearance.radius = openchuck_types::units::Millimetres(PART + 6.0);
    f.clearance.safe_z = openchuck_types::units::Millimetres(PART + 16.0);
    f
}

fn fixture() -> Fixture {
    fixture_with_stock(PART + 1.0)
}

fn axial_in(input: &StrategyInput<'_>, v: &BTreeMap<String, ParameterValue>) -> GeneratedPath {
    run(|ctx| generate(&AxialFinishing, input, v, ctx)).unwrap()
}

fn axial(f: &Fixture, v: &BTreeMap<String, ParameterValue>) -> GeneratedPath {
    axial_in(&f.input(), v)
}

/// The tip's distance from the rotary axis at the end of each move that is
/// neither a cut nor part of the first approach or the last retract: the
/// links between passes.
fn link_radii(path: &GeneratedPath) -> Vec<f64> {
    let segs = &path.group.segments;
    let first = segs
        .iter()
        .position(|s| s.kind == MoveKind::Cutting)
        .unwrap();
    let last = segs
        .iter()
        .rposition(|s| s.kind == MoveKind::Cutting)
        .unwrap();
    segs[first..=last]
        .iter()
        .filter(|s| s.kind != MoveKind::Cutting)
        .map(|s| s.end.tip.y.hypot(s.end.tip.z))
        .collect()
}

fn low() -> (&'static str, ParameterValue) {
    (parameter::LINK_HEIGHT, text(parameter::LOW))
}

#[test]
fn req_op_14_axial_links_default_to_retract_and_are_unchanged() {
    let f = fixture();
    let plain = axial(&f, &values(&[]));
    let explicit = axial(
        &f,
        &values(&[(parameter::LINK_HEIGHT, text(parameter::RETRACT))]),
    );
    assert_eq!(
        format!("{:?}", plain.group),
        format!("{:?}", explicit.group)
    );
    // Every link goes to the clearance cylinder.
    let top = link_radii(&plain).into_iter().fold(0.0_f64, f64::max);
    assert!((top - (PART + 6.0)).abs() < 1e-6, "{top}");
    assert!(
        AxialFinishing
            .schema()
            .get(parameter::LINK_HEIGHT)
            .is_some_and(|s| matches!(
                &s.kind,
                openchuck_path::ParameterKind::Choice { default, .. } if default == "retract"
            ))
    );
}

#[test]
fn req_op_14_axial_low_links_stay_below_the_clearance() {
    let f = fixture();
    let normal = axial(&f, &values(&[]));
    let lowered = axial(&f, &values(&[low()]));
    let top = link_radii(&lowered).into_iter().fold(0.0_f64, f64::max);
    assert!(top < PART + 6.0 - 1.0, "link at {top}");
    // The cuts are the same; only the moves between them differ.
    let cuts = |p: &GeneratedPath| {
        p.group
            .segments
            .iter()
            .filter(|s| s.kind == MoveKind::Cutting)
            .map(|s| format!("{:?}", s.end))
            .collect::<Vec<_>>()
    };
    assert_eq!(cuts(&normal), cuts(&lowered));
    assert!(lowered.group.segments.len() <= normal.group.segments.len());
}

#[test]
fn req_op_14_req_nfr_12_low_links_never_go_below_the_stock() {
    for stock in [PART + 0.5, PART + 1.0, PART + 3.0] {
        let f = fixture_with_stock(stock);
        let path = axial(&f, &values(&[low()]));
        for r in link_radii(&path) {
            // Moves between passes that stay near a cut may be at the
            // surface; any move that travels across A is above the stock.
            assert!(r >= PART - 1e-6, "stock {stock}: link at {r}");
        }
        // A travelling link (a rapid or linking move that turns A) holds the
        // tool the approach distance above the stock's top.
        let segs = &path.group.segments;
        let mut prev = path.group.start;
        for s in segs {
            if s.kind == MoveKind::Rapid && (s.end.a.0 - prev.a.0).abs() > 1e-9 {
                let r = s.end.tip.y.hypot(s.end.tip.z);
                assert!(r >= stock - 1e-6, "stock {stock}: turn at {r}");
            }
            prev = s.end;
        }
    }
}

#[test]
fn req_op_14_req_nfr_07_links_that_cannot_be_lowered_retract_and_are_said() {
    // The stock reaches within the approach distance of the clearance, so no
    // link can be lowered.
    let f = fixture_with_stock(PART + 5.5);
    let normal = axial(&f, &values(&[]));
    let lowered = axial(&f, &values(&[low()]));
    assert_eq!(
        format!("{:?}", normal.group),
        format!("{:?}", lowered.group)
    );
    assert!(
        lowered
            .notes
            .iter()
            .any(|n| n.message.contains("could not be lowered")),
        "{:?}",
        lowered.notes
    );
    assert!(!normal.notes.iter().any(|n| n.message.contains("lowered")));
    // A job that can lower every link says nothing.
    let all = axial(&fixture(), &values(&[low()]));
    assert!(
        !all.notes
            .iter()
            .any(|n| n.message.contains("could not be lowered"))
    );
}

/// The links between cuts as (X where the last cut ended, X where the next
/// begins, the highest the tool's tip went from the axis between them).
fn links(path: &GeneratedPath) -> Vec<(f64, f64, f64)> {
    let mut out = Vec::new();
    let mut last_cut: Option<f64> = None;
    let mut top = 0.0_f64;
    let mut prev = path.group.start;
    let mut moved = false;
    for s in &path.group.segments {
        if s.kind == MoveKind::Cutting {
            if moved && let Some(from) = last_cut {
                out.push((from, prev.tip.x, top));
            }
            last_cut = Some(s.end.tip.x);
            top = 0.0;
            moved = false;
        } else {
            top = top.max(s.end.tip.y.hypot(s.end.tip.z));
            moved = true;
        }
        prev = s.end;
    }
    out
}

#[test]
fn req_set_09_low_links_do_not_cross_a_held_region() {
    // A held interval inside the part splits every pass in two; the link
    // between the two pieces crosses it and must retract (SET-09), while the
    // links at the ends of the passes still stay low.
    let f = fixture();
    let keep = [(9.0, 11.0)];
    let mut input = f.input();
    input.keep_out = &keep;
    let path = axial_in(&input, &values(&[low()]));
    let clearance = PART + 6.0;
    let crossing = |from: f64, to: f64| from.min(to) < keep[0].1 && from.max(to) > keep[0].0;
    let all = links(&path);
    let (across, elsewhere): (Vec<_>, Vec<_>) =
        all.iter().partition(|&&(from, to, _)| crossing(from, to));
    assert!(
        !across.is_empty(),
        "no link crosses the held region: {all:?}"
    );
    for (from, to, top) in &across {
        assert!(
            (top - clearance).abs() < 1e-6,
            "link from x {from} to {to} crossing the held region went only to {top}"
        );
    }
    assert!(
        elsewhere.iter().any(|&(_, _, top)| top < clearance - 1.0),
        "no other link was lowered: {elsewhere:?}"
    );
    assert!(
        path.notes
            .iter()
            .any(|n| n.message.contains("could not be lowered")),
        "{:?}",
        path.notes
    );
}

#[test]
fn req_op_14_indexed_low_links_stay_below_the_clearance() {
    let f = fixture();
    let v = values(&[
        (
            openchuck_path::indexed::parameter::COUNT,
            ParameterValue::Integer(6),
        ),
        low(),
    ]);
    let path = run(|ctx| generate(&IndexedFinishing, &f.input(), &v, ctx)).unwrap();
    let top = link_radii(&path).into_iter().fold(0.0_f64, f64::max);
    assert!(top < PART + 6.0 - 1.0, "link at {top}");
    assert!(
        IndexedFinishing
            .schema()
            .get(parameter::LINK_HEIGHT)
            .is_some()
    );
}

#[test]
fn req_gc_07_low_links_are_identical_at_1_2_and_n_threads() {
    let f = fixture();
    let v = values(&[low()]);
    let runs =
        openchuck_testkit::determinism::check(|| format!("{:?}", axial(&f, &v).group).into_bytes())
            .unwrap();
    assert_eq!(runs.len(), 4);
}
