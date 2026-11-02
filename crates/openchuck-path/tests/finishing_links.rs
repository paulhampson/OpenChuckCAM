// SPDX-License-Identifier: AGPL-3.0-or-later
//! Links between passes of helical finishing, rest machining and
//! simultaneous finishing: the option to keep them low, as axial and indexed
//! finishing have it (OP-14, SET-06, SET-09, SIM-08, NFR-07, NFR-12, GC-07,
//! NFR-06; issue #469; OI-1741; decisions 0496 and 0640). The checks are on
//! the crankshaft (`stl/crankshaft.stl`), where the helix crosses the
//! off-axis pins in pieces and the face turns and hidden passes add links.
#![allow(clippy::unwrap_used, clippy::float_cmp, clippy::indexing_slicing)]

mod support;

use std::collections::BTreeMap;
use std::path::PathBuf;

use openchuck_path::axial::parameter;
use openchuck_path::params::names;
use openchuck_path::{
    GeneratedPath, HelicalFinishing, MeshSurface, ParameterKind, ParameterValue, RestMachining,
    SimultaneousFinishing, Strategy, StrategyInput,
};
use openchuck_types::ir::MoveKind;
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::units::Millimetres;
use support::{Fixture, ball, generate, read_binary_stl, run};

const DIAMETER: f64 = 3.175;
/// The stock's radius: the crankshaft reaches 17.5 mm from the axis.
const STOCK: f64 = 18.5;
/// The clearance cylinder's radius.
const CLEARANCE: f64 = 30.0;

fn crankshaft() -> TriangleMesh {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/stl/crankshaft.stl");
    read_binary_stl(&path)
}

fn fixture() -> Fixture {
    let mut f = Fixture::new(
        MeshSurface::new(&crankshaft()).unwrap(),
        STOCK,
        &ball(DIAMETER),
    );
    f.clearance.radius = Millimetres(CLEARANCE);
    f.clearance.safe_z = Millimetres(CLEARANCE + 10.0);
    f
}

fn values(link: Option<&str>) -> BTreeMap<String, ParameterValue> {
    let mut v: BTreeMap<String, ParameterValue> = [
        (names::STEPOVER, 1.0),
        (names::FEED, 600.0),
        (names::PLUNGE_FEED, 150.0),
        (names::TOLERANCE, 0.02),
    ]
    .into_iter()
    .map(|(k, x)| (k.to_owned(), ParameterValue::Number(x)))
    .collect();
    if let Some(link) = link {
        v.insert(
            parameter::LINK_HEIGHT.to_owned(),
            ParameterValue::Text(link.to_owned()),
        );
    }
    v
}

fn strategies() -> [&'static dyn Strategy; 3] {
    [&HelicalFinishing, &RestMachining, &SimultaneousFinishing]
}

fn path(strategy: &dyn Strategy, input: &StrategyInput<'_>, link: Option<&str>) -> GeneratedPath {
    run(|ctx| generate(strategy, input, &values(link), ctx)).unwrap()
}

/// The cutting moves' end poses, in order.
fn cuts(p: &GeneratedPath) -> Vec<String> {
    p.group
        .segments
        .iter()
        .filter(|s| s.kind == MoveKind::Cutting)
        .map(|s| format!("{:?}", s.end))
        .collect()
}

fn duration(p: &GeneratedPath) -> f64 {
    p.group.segments.iter().map(|s| s.duration.0).sum()
}

/// The highest the tip goes from the axis on each move that turns A while
/// not cutting, between the first cut and the last.
fn travelling(p: &GeneratedPath) -> Vec<f64> {
    let segs = &p.group.segments;
    let first = segs
        .iter()
        .position(|s| s.kind == MoveKind::Cutting)
        .unwrap();
    let last = segs
        .iter()
        .rposition(|s| s.kind == MoveKind::Cutting)
        .unwrap();
    let mut prev = segs[first].end;
    let mut out = Vec::new();
    for s in &segs[first + 1..=last] {
        if s.kind == MoveKind::Rapid && (s.end.a.0 - prev.a.0).abs() > 1e-9 {
            out.push(s.end.tip.y.hypot(s.end.tip.z));
        }
        prev = s.end;
    }
    out
}

#[test]
fn req_op_14_links_default_to_retract_and_are_unchanged() {
    let f = fixture();
    for strategy in strategies() {
        let spec = strategy
            .schema()
            .get(parameter::LINK_HEIGHT)
            .cloned()
            .unwrap();
        assert!(
            matches!(&spec.kind, ParameterKind::Choice { default, .. } if default == parameter::RETRACT),
            "{}",
            strategy.name()
        );
        // The limit of the check is in the words the user reads (NFR-12).
        assert!(spec.description.contains("verify"), "{}", spec.description);
        let plain = path(strategy, &f.input(), None);
        let explicit = path(strategy, &f.input(), Some(parameter::RETRACT));
        assert_eq!(
            format!("{:?} {:?}", plain.group, plain.notes),
            format!("{:?} {:?}", explicit.group, explicit.notes),
            "{}",
            strategy.name()
        );
    }
}

#[test]
fn req_op_14_req_sim_08_req_nfr_12_low_links_stay_below_the_clearance_above_the_stock() {
    let f = fixture();
    for strategy in strategies() {
        let high = path(strategy, &f.input(), None);
        let low = path(strategy, &f.input(), Some(parameter::LOW));
        // The cuts are the same; only the moves between them differ.
        assert_eq!(cuts(&high), cuts(&low), "{}", strategy.name());
        let (t0, t1) = (duration(&high), duration(&low));
        let tops = travelling(&low);
        let top = tops.iter().copied().fold(0.0_f64, f64::max);
        eprintln!(
            "{}: {} travelling links, highest {top:.2} mm; time {t0:.1} -> {t1:.1} s",
            strategy.name(),
            tops.len()
        );
        assert!(!tops.is_empty(), "{}", strategy.name());
        // Every link that turns A is above the stock and, where it could be
        // lowered, below the clearance.
        for r in &tops {
            assert!(*r >= STOCK - 1e-6, "{}: link at {r}", strategy.name());
        }
        assert!(
            tops.iter().any(|r| *r < CLEARANCE - 1.0),
            "{}: {tops:?}",
            strategy.name()
        );
        assert!(t1 < t0, "{}: {t0} {t1}", strategy.name());
    }
}

#[test]
fn req_set_09_low_links_do_not_reach_into_a_held_region() {
    // A held interval across the middle web: no fed move enters it, and
    // the operation still keeps its links that can stay low.
    let f = fixture();
    let keep = [(11.0, 14.0)];
    let mut input = f.input();
    input.keep_out = &keep;
    for strategy in strategies() {
        let low = path(strategy, &input, Some(parameter::LOW));
        for s in &low.group.segments {
            if s.kind.is_fed() {
                let x = s.end.tip.x;
                assert!(
                    !(x > keep[0].0 && x < keep[0].1),
                    "{}: {x}",
                    strategy.name()
                );
            }
        }
        assert!(
            travelling(&low).iter().any(|r| *r < CLEARANCE - 1.0),
            "{}",
            strategy.name()
        );
    }
}

#[test]
fn req_gc_07_req_nfr_06_low_links_are_identical_at_1_2_and_n_threads() {
    let f = fixture();
    let v = values(Some(parameter::LOW));
    let runs = openchuck_testkit::determinism::check(|| {
        let p = run(|ctx| generate(&SimultaneousFinishing, &f.input(), &v, ctx)).unwrap();
        format!("{:?} {:?}", p.group, p.notes).into_bytes()
    })
    .unwrap();
    assert_eq!(runs.len(), 4);
}
