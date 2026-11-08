// SPDX-License-Identifier: AGPL-3.0-or-later
//! A cutting move inside a roughing pass goes down along the tool axis no
//! faster than the plunge feed (OP-11, OP-03; decision record 0548, issue
//! #643). Each check reads the assembled path only: the tip's descent along
//! the tool axis and the move's time, not the assembler's own helpers.
//!
//! The G-code these paths give is the user's to verify before it is run on a
//! machine (NFR-12).
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing
)]

mod support;

use std::collections::BTreeMap;

use openchuck_path::params::names;
use openchuck_path::roughing::parameter;
use openchuck_path::{
    GeneratedPath, HelicalFinishing, MeshSurface, ParameterValue, RotaryRoughing, Strategy,
};
use openchuck_testkit::parts::RadialPart;
use openchuck_types::ir::{MoveKind, ToolpathGroup};
use openchuck_types::tool::ToolShape;
use support::{Fixture, generate, run, tool};

const FEED: f64 = 1000.0;
const PLUNGE: f64 = 150.0;

/// A cylinder of radius 6 with a steep raised band of radius 10 from X 10 to
/// X 14: roughing comes down off the band to each layer below its top.
struct RaisedBand;

impl RadialPart for RaisedBand {
    fn name(&self) -> &'static str {
        "raised band"
    }
    fn x_range(&self) -> (f64, f64) {
        (0.0, 24.0)
    }
    fn radius_at(&self, x: f64, _angle: f64) -> f64 {
        if (10.0..=14.0).contains(&x) {
            10.0
        } else {
            6.0
        }
    }
}

/// A cylinder of radius 6 with one raised lobe of radius 10 over a quarter of
/// the turn: a pass round the part comes down off the lobe.
struct RaisedLobe;

impl RadialPart for RaisedLobe {
    fn name(&self) -> &'static str {
        "raised lobe"
    }
    fn x_range(&self) -> (f64, f64) {
        (0.0, 16.0)
    }
    fn radius_at(&self, _x: f64, angle: f64) -> f64 {
        let a = angle.rem_euclid(std::f64::consts::TAU);
        if a < std::f64::consts::FRAC_PI_2 {
            10.0
        } else {
            6.0
        }
    }
}

fn values(extra: &[(&str, ParameterValue)]) -> BTreeMap<String, ParameterValue> {
    let mut v: BTreeMap<String, ParameterValue> = [
        (names::STEPOVER, 2.0),
        (names::FEED, FEED),
        (names::PLUNGE_FEED, PLUNGE),
        (names::STOCK_TO_LEAVE, 0.3),
        (parameter::STEPDOWN, 1.5),
    ]
    .into_iter()
    .map(|(k, x)| (k.to_owned(), ParameterValue::Number(x)))
    .collect();
    for (k, x) in extra {
        v.insert((*k).to_owned(), x.clone());
    }
    v
}

fn fixture(part: &dyn RadialPart) -> Fixture {
    let surface = MeshSurface::new(&part.mesh(120, 360)).unwrap();
    Fixture::new(surface, 11.0, &tool(ToolShape::FlatEnd, 3.0))
}

fn path(
    strategy: &dyn Strategy,
    part: &dyn RadialPart,
    v: &BTreeMap<String, ParameterValue>,
) -> GeneratedPath {
    let f = fixture(part);
    run(|ctx| generate(strategy, &f.input(), v, ctx)).unwrap()
}

/// One cutting move: how far its tip goes down along the tool axis, and the
/// speed of that descent in mm/min. The tool axis of the X-rotary fixture
/// passes through the rotary axis, so the tip's height above it is the tip's
/// component along the tool axis.
struct Descent {
    down: f64,
    rate: f64,
    feed: f64,
}

fn descents(group: &ToolpathGroup) -> Vec<Descent> {
    let mut from = group.start;
    let mut out = Vec::new();
    for s in &group.segments {
        let height = |p: &openchuck_types::ir::ToolPose| {
            let a = p.axis.get();
            p.tip.x * a.x + p.tip.y * a.y + p.tip.z * a.z
        };
        if s.kind == MoveKind::Cutting {
            let down = height(&from) - height(&s.end);
            out.push(Descent {
                down,
                rate: 60.0 * down / s.duration.0,
                feed: s.contact_feed.unwrap().0,
            });
        }
        from = s.end;
    }
    out
}

/// The tip goes down along the tool axis at no more than the plunge feed in
/// every cutting move of a roughing pass, off a raised band (steps along X)
/// and off a raised lobe (steps round A).
#[test]
fn req_op_11_req_op_03_roughing_drop_off_a_raised_floor_goes_down_no_faster_than_the_plunge_feed() {
    for part in [&RaisedBand as &dyn RadialPart, &RaisedLobe] {
        let p = path(&RotaryRoughing, part, &values(&[]));
        let all = descents(&p.group);
        let steep = all.iter().filter(|d| d.down > 0.5).count();
        assert!(
            steep > 0,
            "{}: the case must come down off the raised floor",
            part.name()
        );
        let worst = all.iter().map(|d| d.rate).fold(0.0, f64::max);
        assert!(
            worst <= PLUNGE * (1.0 + 1e-9),
            "{}: a cutting move goes down at {worst:.1} mm/min, above the plunge feed {PLUNGE}",
            part.name()
        );
    }
}

/// Only a move that would go down faster than the plunge feed changes, and
/// then only as much as needed: every other cutting move keeps the cutting
/// feed, and a slowed one takes exactly its descent plus one written unit
/// (0.001 mm, the most that rounding its two heights to three decimals can
/// add) at the plunge feed.
#[test]
fn req_op_11_shallow_descent_and_level_moves_keep_the_cutting_feed() {
    let p = path(&RotaryRoughing, &RaisedBand, &values(&[]));
    let all = descents(&p.group);
    let mut slowed = 0;
    for d in &all {
        if d.feed == FEED {
            continue;
        }
        slowed += 1;
        assert!(d.feed < FEED, "a feed is raised: {}", d.feed);
        let expected = PLUNGE * d.down / (d.down + 0.001);
        assert!(
            (d.rate - expected).abs() <= 1e-6 * PLUNGE,
            "a slowed move goes down at {:.4} mm/min, not {expected:.4}",
            d.rate
        );
    }
    assert!(slowed > 0, "the case must slow at least one move");
    assert!(
        slowed < all.len() / 2,
        "{slowed} of {} cutting moves were slowed",
        all.len()
    );
}

/// Finishing keeps its feeds (the owner's answer to question 1 of decision
/// record 0548; OI-2134): the same raised band finished keeps the cutting
/// feed on every cutting move, though some go down faster than the plunge.
#[test]
fn req_op_11_finishing_descent_is_unchanged() {
    let p = path(&HelicalFinishing, &RaisedBand, &values(&[]));
    let all = descents(&p.group);
    assert!(
        all.iter().all(|d| d.feed == FEED),
        "finishing changed a feed"
    );
    assert!(
        all.iter().any(|d| d.rate > PLUNGE),
        "the case must have a finishing descent faster than the plunge feed"
    );
}

/// Only roughing and Facing's Flat and Round modes take the rule.
#[test]
fn req_op_11_only_roughing_strategies_limit_their_descents() {
    use openchuck_path::{Facing, Parameters};
    let resolve = |s: &dyn Strategy, v: &BTreeMap<String, ParameterValue>| -> Parameters {
        s.resolve_parameters(v).unwrap()
    };
    let rough = values(&[]);
    assert!(RotaryRoughing.limits_descent_feed(&resolve(&RotaryRoughing, &rough)));
    assert!(!HelicalFinishing.limits_descent_feed(&resolve(&HelicalFinishing, &rough)));
    for (shape, limited) in [
        ("flat", true),
        ("round", true),
        ("end", false),
        ("box", false),
    ] {
        let v = values(&[
            (
                openchuck_path::facing::parameter::SHAPE,
                ParameterValue::Text(shape.to_owned()),
            ),
            (
                openchuck_path::facing::parameter::SIZE,
                ParameterValue::Number(10.0),
            ),
        ]);
        let p = Facing.resolve_parameters(&v).unwrap();
        assert_eq!(Facing.limits_descent_feed(&p), limited, "{shape}");
    }
}

/// The same path at 1, 2 and 4 threads (GC-07, NFR-06).
#[test]
fn req_gc_07_req_nfr_06_descent_feed_is_deterministic_at_1_2_n_threads() {
    let f = fixture(&RaisedBand);
    let v = values(&[]);
    let paths: Vec<GeneratedPath> = [1, 2, 4]
        .into_iter()
        .map(|n| {
            rayon::ThreadPoolBuilder::new()
                .num_threads(n)
                .build()
                .unwrap()
                .install(|| run(|ctx| generate(&RotaryRoughing, &f.input(), &v, ctx)).unwrap())
        })
        .collect();
    for p in &paths[1..] {
        assert_eq!(p.group, paths[0].group);
    }
}
