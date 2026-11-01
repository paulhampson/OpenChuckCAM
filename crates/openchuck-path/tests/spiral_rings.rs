// SPDX-License-Identifier: AGPL-3.0-or-later
//! Rings in place of the spiral for rest machining, continuous rotary
//! roughing and round Facing (issue #440; decision records 0390 and 0420;
//! OP-03, OP-09, OP-11, OP-12, OP-14, OP-15, OP-24, GC-07, NFR-06, NFR-07).
//!
//! With **Passes** set to **Rings**, each turn is one whole turn of A at a
//! fixed X, and the tool steps along the part at a fixed A to the next ring.
//! With the setting absent or **Spiral**, each strategy cuts exactly as
//! before.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

mod support;
use support::generate;

use std::collections::BTreeMap;

use openchuck_path::facing::parameter as facing;
use openchuck_path::helical::parameter::{HELIX, PASS_SHAPE, RINGS};
use openchuck_path::params::names;
use openchuck_path::rest::parameter as rest;
use openchuck_path::rotary::reversals;
use openchuck_path::roughing::parameter as roughing;
use openchuck_path::{
    AnalyticStock, Facing, GeneratedPath, MeshSurface, ParameterKind, ParameterValue,
    RestMachining, RotaryRoughing, Strategy,
};
use openchuck_testkit::gouge::{Outline, Pose, coverage, max_gouge};
use openchuck_testkit::parts::{Cone, Cylinder, RadialPart, revolve};
use openchuck_types::ir::{MoveKind, Segment, ToolPose};
use openchuck_types::tool::{Tool, ToolShape};
use openchuck_types::units::Degrees;
use support::{Fixture, ball, run, tool};

fn num(x: f64) -> ParameterValue {
    ParameterValue::Number(x)
}

fn text(s: &str) -> ParameterValue {
    ParameterValue::Text(s.to_owned())
}

fn with(
    base: &[(&str, f64)],
    extra: &[(&str, ParameterValue)],
) -> BTreeMap<String, ParameterValue> {
    let mut v: BTreeMap<String, ParameterValue> = base
        .iter()
        .map(|(k, x)| ((*k).to_owned(), num(*x)))
        .collect();
    for (k, x) in extra {
        v.insert((*k).to_owned(), x.clone());
    }
    v
}

fn ringed(mut v: BTreeMap<String, ParameterValue>) -> BTreeMap<String, ParameterValue> {
    v.insert(PASS_SHAPE.to_owned(), text(RINGS));
    v
}

fn path(
    f: &Fixture,
    strategy: &dyn Strategy,
    v: &BTreeMap<String, ParameterValue>,
) -> openchuck_path::Result<GeneratedPath> {
    run(|ctx| generate(strategy, &f.input(), v, ctx))
}

/// The cutting segments with the pose each starts from.
fn cuts(p: &GeneratedPath) -> Vec<(ToolPose, Segment)> {
    let mut prev = p.group.start;
    let mut out = Vec::new();
    for s in &p.group.segments {
        if s.kind == MoveKind::Cutting {
            out.push((prev, *s));
        }
        prev = s.end;
    }
    out
}

/// Every cutting move either turns A at constant X (a ring, the way
/// `sign` says) or moves along X at constant A (a step) with A a whole
/// number of turns from `start`, and X never goes back within a cut.
fn assert_rings_and_steps(p: &GeneratedPath, start: f64, sign: f64, what: &str) {
    let mut steps = 0;
    let mut turning = 0;
    for (from, s) in cuts(p) {
        let dx = s.end.tip.x - from.tip.x;
        let da = s.end.a.0 - from.a.0;
        if dx.abs() > 1e-9 {
            assert!(dx > 0.0, "{what}: X goes back ({dx}) at {:?}", s.end.tip);
            assert!(da.abs() < 1e-9, "{what}: a step turned A by {da}");
            let at = (s.end.a.0 - start).rem_euclid(360.0);
            assert!(
                !(1e-6..=360.0 - 1e-6).contains(&at),
                "{what}: a step at A {}",
                s.end.a.0
            );
            steps += 1;
        } else if da.abs() > 1e-9 {
            assert!(da * sign > 0.0, "{what}: A turns the set way ({da})");
            turning += 1;
        }
    }
    assert!(steps > 0 && turning > 0, "{what}: {steps} steps, {turning}");
}

/// The distinct X of the moves that turn A at constant X.
fn ring_xs(p: &GeneratedPath) -> Vec<f64> {
    let mut xs: Vec<f64> = cuts(p)
        .into_iter()
        .filter(|(from, s)| {
            (s.end.tip.x - from.tip.x).abs() < 1e-9 && (s.end.a.0 - from.a.0).abs() > 1e-9
        })
        .map(|(_, s)| s.end.tip.x)
        .collect();
    xs.sort_by(f64::total_cmp);
    xs.dedup_by(|b, a| (*b - *a).abs() < 1e-9);
    xs
}

fn fed(p: &GeneratedPath) -> Vec<Pose> {
    p.group
        .segments
        .iter()
        .filter(|s| matches!(s.kind, MoveKind::Cutting | MoveKind::Plunge))
        .map(|s| Pose {
            tip: s.end.tip,
            axis: s.end.axis.get(),
        })
        .collect()
}

fn a_path(p: &GeneratedPath) -> Vec<Degrees> {
    let c = cuts(p);
    c.first()
        .map(|(from, _)| from.a)
        .into_iter()
        .chain(c.iter().map(|(_, s)| s.end.a))
        .collect()
}

fn schema_has_passes(strategy: &dyn Strategy) {
    let schema = strategy.schema();
    let spec = schema
        .get(PASS_SHAPE)
        .unwrap_or_else(|| panic!("{}: no Passes", strategy.name()));
    assert_eq!(spec.label, "Passes");
    assert!(spec.description.contains("Spiral") && spec.description.contains("Rings"));
    match &spec.kind {
        ParameterKind::Choice { options, default } => {
            assert_eq!(options, &[HELIX.to_owned(), RINGS.to_owned()]);
            assert_eq!(default, HELIX, "{}: Spiral by default", strategy.name());
        }
        other => panic!("{other:?}"),
    }
}

fn refused_by_name(f: &Fixture, strategy: &dyn Strategy, v: &BTreeMap<String, ParameterValue>) {
    let mut v = v.clone();
    v.insert(PASS_SHAPE.to_owned(), text("zigzag"));
    let e = path(f, strategy, &v).unwrap_err().to_string();
    assert!(e.contains(PASS_SHAPE), "{}: {e}", strategy.name());
}

fn identical_at_any_thread_count(
    f: &Fixture,
    strategy: &dyn Strategy,
    v: &BTreeMap<String, ParameterValue>,
) {
    let runs = openchuck_testkit::determinism::check(|| {
        format!("{:?}", path(f, strategy, v).unwrap().group).into_bytes()
    })
    .unwrap();
    assert_eq!(runs.len(), 4);
}

// ---------------------------------------------------------------- rest

/// A shaft of radius 6 to X 12, a flank up to radius 12 at X 13.6, then
/// radius 12 to X 24: stock of radius 12 leaves material only on the small
/// shaft and the flank.
const SHAFT: [(f64, f64); 4] = [(0.0, 6.0), (12.0, 6.0), (13.6, 12.0), (24.0, 12.0)];

fn rest_values(extra: &[(&str, ParameterValue)]) -> BTreeMap<String, ParameterValue> {
    with(
        &[
            (names::STEPOVER, 1.0),
            (names::FEED, 400.0),
            (names::PLUNGE_FEED, 200.0),
        ],
        extra,
    )
}

fn shaft(stock_radius: f64) -> Fixture {
    let surface = MeshSurface::new(&revolve(&SHAFT, 180)).unwrap();
    let mut f = Fixture::new(surface, stock_radius, &ball(3.0));
    f.stock = AnalyticStock::Cylinder {
        radius: stock_radius,
        x_start: 0.0,
        x_end: 24.0,
    };
    f
}

#[test]
fn req_op_09_rest_machining_offers_rings_defaulting_to_spiral() {
    schema_has_passes(&RestMachining);
}

#[test]
fn req_op_09_rest_rings_turn_at_constant_x_and_step_at_constant_a() {
    let f = shaft(15.0);
    for (rotation, start, sign) in [("positive", 0.0, 1.0), ("negative", 40.0, -1.0)] {
        let v = ringed(rest_values(&[
            (rest::ROTATION, text(rotation)),
            (rest::START_ANGLE, num(start)),
        ]));
        let p = path(&f, &RestMachining, &v).unwrap();
        assert_rings_and_steps(&p, start, sign, rotation);
        // Rings evenly spaced no wider than the stepover over the part, with
        // a ring at each end; faces across the axis add their own.
        let xs = ring_xs(&p);
        assert!(xs.first().unwrap().abs() < 1e-9, "{xs:?}");
        assert!((xs.last().unwrap() - 24.0).abs() < 1e-9, "{xs:?}");
        for w in xs.windows(2) {
            assert!(w[1] - w[0] <= 1.0 + 1e-9, "{rotation}: gap {w:?}");
        }
        assert!(
            p.notes.iter().any(|n| n.message.contains("rings")),
            "{:?}",
            p.notes
        );
        assert_eq!(reversals(&a_path(&p), 1e-9), 0, "{rotation}: OP-15");
    }
}

#[test]
fn req_op_09_rest_rings_cut_only_where_material_is_left() {
    // Stock of radius 12: nothing is left on the large cylinder, so neither
    // the spiral nor the rings cut there.
    let f = shaft(12.0);
    let v = rest_values(&[]);
    let spiral = path(&f, &RestMachining, &v).unwrap();
    let rings = path(&f, &RestMachining, &ringed(v)).unwrap();
    let far = |p: &GeneratedPath| {
        cuts(p)
            .iter()
            .map(|(_, s)| s.end.tip.x)
            .fold(f64::NEG_INFINITY, f64::max)
    };
    assert!(far(&spiral) < 18.0, "{}", far(&spiral));
    assert!(far(&rings) <= far(&spiral) + 1.0, "{}", far(&rings));
    // On the clean cylinder, rings reach no further than the spiral does.
    let on_clean = |p: &GeneratedPath| {
        cuts(p)
            .iter()
            .filter(|(_, s)| s.end.tip.y.hypot(s.end.tip.z) > 11.9)
            .map(|(_, s)| s.end.tip.x)
            .fold(f64::NEG_INFINITY, f64::max)
    };
    assert!(
        on_clean(&rings) <= on_clean(&spiral) + 1e-3,
        "rings cut the clean cylinder"
    );
}

#[test]
fn req_op_09_req_gc_07_rest_spiral_is_unchanged_when_rings_are_not_chosen() {
    let f = shaft(12.0);
    let missing = path(&f, &RestMachining, &rest_values(&[])).unwrap();
    let spiral = path(
        &f,
        &RestMachining,
        &rest_values(&[(PASS_SHAPE, text(HELIX))]),
    )
    .unwrap();
    assert_eq!(
        format!("{:?}", missing.group),
        format!("{:?}", spiral.group)
    );
    assert_eq!(missing.notes, spiral.notes);
    assert!(ring_xs(&spiral).is_empty());
}

#[test]
fn req_op_09_req_op_12_rest_rings_leave_no_more_than_the_spiral_and_never_gouge() {
    let part = Cylinder {
        radius: 10.0,
        length: 20.0,
    };
    let surface = MeshSurface::new(&part.mesh(3, 1440)).unwrap();
    let f = Fixture::new(surface, 13.0, &ball(6.0));
    let v = rest_values(&[
        (names::STEPOVER, num(0.5)),
        (rest::MAX_ANGLE_STEP, num(1.0)),
    ]);
    let outline = Outline::new(&f.tool);
    let worst = |p: &GeneratedPath| {
        let poses = fed(p);
        let g = max_gouge(&outline, &part, &poses, 0.15);
        assert!(g.depth <= 0.002, "gouge {}", g.depth);
        coverage(&outline, &part, &poses, (7.0, 13.0), 0.2, 0.02)
    };
    let spiral = worst(&path(&f, &RestMachining, &v).unwrap());
    let rings = worst(&path(&f, &RestMachining, &ringed(v)).unwrap());
    assert_eq!(rings.uncovered, 0, "{rings:?}");
    assert!(rings.worst <= spiral.worst + 1e-3, "{rings:?} {spiral:?}");
}

#[test]
fn req_nfr_06_req_gc_07_rest_rings_are_identical_at_1_2_and_n_threads() {
    identical_at_any_thread_count(&shaft(12.0), &RestMachining, &ringed(rest_values(&[])));
}

#[test]
fn req_nfr_07_rest_refuses_an_unknown_pass_shape_by_name() {
    refused_by_name(&shaft(12.0), &RestMachining, &rest_values(&[]));
}

// ------------------------------------------------------------ roughing

fn rough_values(extra: &[(&str, ParameterValue)]) -> BTreeMap<String, ParameterValue> {
    with(
        &[
            (names::STEPOVER, 3.0),
            (names::FEED, 600.0),
            (names::PLUNGE_FEED, 200.0),
            (names::STOCK_TO_LEAVE, 0.5),
            (roughing::STEPDOWN, 2.0),
        ],
        extra,
    )
}

fn flat(d: f64) -> Tool {
    tool(ToolShape::FlatEnd, d)
}

fn rough_fixture<P: RadialPart + ?Sized>(part: &P, stock_radius: f64, t: &Tool) -> Fixture {
    let surface = MeshSurface::new(&part.mesh(40, 360)).unwrap();
    Fixture::new(surface, stock_radius, t)
}

fn cone() -> Cone {
    Cone {
        start_radius: 8.0,
        end_radius: 12.0,
        length: 30.0,
    }
}

/// The tip's height above the axis along the tool axis, which is the layer
/// level wherever the tool cuts above the part.
fn level(f: &Fixture, pose: &ToolPose) -> f64 {
    f.rotary.to_axis(pose.tip, pose.a).height
}

#[test]
fn req_op_03_continuous_roughing_offers_rings_only_for_its_spiral() {
    schema_has_passes(&RotaryRoughing);
    let shown = |extra: &[(&str, &str)]| {
        let v: BTreeMap<String, ParameterValue> = extra
            .iter()
            .map(|(k, x)| ((*k).to_owned(), text(x)))
            .collect();
        RotaryRoughing.schema_for(&v).get(PASS_SHAPE).is_some()
    };
    assert!(shown(&[]));
    assert!(shown(&[(roughing::DIRECTION, roughing::AROUND)]));
    assert!(!shown(&[(roughing::FORM, "indexed")]));
    assert!(!shown(&[(roughing::DIRECTION, roughing::ALONG)]));
    assert!(!shown(&[
        (roughing::FORM, "indexed"),
        (roughing::DIRECTION, roughing::AROUND)
    ]));
}

#[test]
fn req_op_03_req_op_11_roughing_rings_turn_at_constant_x_and_step_at_constant_a_in_layers() {
    let part = Cylinder {
        radius: 8.0,
        length: 20.0,
    };
    let f = rough_fixture(&part, 12.0, &flat(6.0));
    for start in [0.0, 30.0] {
        let v = ringed(rough_values(&[(roughing::START_ANGLE, num(start))]));
        let p = path(&f, &RotaryRoughing, &v).unwrap();
        assert_rings_and_steps(&p, start, 1.0, "roughing");
        // The rings are no wider apart than the stepover, from one end of
        // the range to the other.
        let xs = ring_xs(&p);
        for w in xs.windows(2) {
            assert!(w[1] - w[0] <= 3.0 + 1e-9, "gap {w:?}");
        }
        // Equal layers no deeper than the stepdown, as for the spiral: the
        // cuts above the part with its stock to leave (radius 8.5) are at
        // the same levels.
        let spiral = path(&f, &RotaryRoughing, &rough_values(&[])).unwrap();
        let levels = |p: &GeneratedPath| {
            let mut l: Vec<i64> = cuts(p)
                .iter()
                .map(|(_, s)| level(&f, &s.end))
                .filter(|h| *h > 8.5 + 1e-6)
                .map(|h| (h * 1e6).round() as i64)
                .collect();
            l.sort_unstable();
            l.dedup();
            l
        };
        assert_eq!(levels(&p), levels(&spiral));
        // A builds up turn by turn and never reverses within a cut (OP-15).
        for run in cut_runs(&p) {
            assert_eq!(reversals(&run, 1e-9), 0);
        }
        assert!(
            p.notes.iter().any(|n| n.message.contains("rings")),
            "{:?}",
            p.notes
        );
    }
}

/// The A of each run of cutting moves.
fn cut_runs(p: &GeneratedPath) -> Vec<Vec<Degrees>> {
    let mut runs: Vec<Vec<Degrees>> = Vec::new();
    let mut prev = p.group.start;
    let mut cutting = false;
    for s in &p.group.segments {
        let now = s.kind == MoveKind::Cutting;
        if now && !cutting {
            runs.push(vec![prev.a]);
        }
        if now && let Some(r) = runs.last_mut() {
            r.push(s.end.a);
        }
        cutting = now;
        prev = s.end;
    }
    runs
}

#[test]
fn req_op_03_req_gc_07_roughing_spiral_is_unchanged_when_rings_are_not_chosen() {
    let f = rough_fixture(&cone(), 15.0, &tool(ToolShape::BallEnd, 4.0));
    let missing = path(&f, &RotaryRoughing, &rough_values(&[])).unwrap();
    let spiral = path(
        &f,
        &RotaryRoughing,
        &rough_values(&[(PASS_SHAPE, text(HELIX))]),
    )
    .unwrap();
    assert_eq!(missing, spiral);
    // The other forms ignore the setting.
    for (form, direction) in [
        ("indexed", roughing::USUAL),
        ("continuous", roughing::ALONG),
    ] {
        let v = rough_values(&[
            (roughing::FORM, text(form)),
            (roughing::DIRECTION, text(direction)),
        ]);
        let a = path(&f, &RotaryRoughing, &v).unwrap();
        let b = path(&f, &RotaryRoughing, &ringed(v)).unwrap();
        assert_eq!(a, b, "{form} {direction}");
    }
}

#[test]
fn req_op_03_req_op_12_roughing_rings_leave_no_more_than_the_spiral_and_never_gouge() {
    for (name, part, t) in [
        (
            "cylinder",
            Box::new(Cylinder {
                radius: 10.0,
                length: 20.0,
            }) as Box<dyn RadialPart>,
            flat(6.0),
        ),
        ("cone", Box::new(cone()), tool(ToolShape::BallEnd, 4.0)),
    ] {
        let f = rough_fixture(part.as_ref(), 15.0, &t);
        let v = rough_values(&[(names::STEPOVER, num(1.5)), (roughing::STEPDOWN, num(3.0))]);
        let outline = Outline::new(&f.tool);
        let (x0, x1) = part.x_range();
        let mid = 0.5 * (x0 + x1);
        let worst = |p: &GeneratedPath| {
            let poses = fed(p);
            let g = max_gouge(&outline, part.as_ref(), &poses, 0.25);
            assert!(g.depth < 1e-3, "{name}: gouge {}", g.depth);
            coverage(
                &outline,
                part.as_ref(),
                &poses,
                (mid - 4.0, mid + 4.0),
                0.5,
                0.75,
            )
        };
        let spiral = worst(&path(&f, &RotaryRoughing, &v).unwrap());
        let rings = worst(&path(&f, &RotaryRoughing, &ringed(v)).unwrap());
        assert_eq!(rings.uncovered, 0, "{name}: {rings:?}");
        assert!(
            rings.worst <= spiral.worst + 0.02,
            "{name}: {rings:?} {spiral:?}"
        );
    }
}

#[test]
fn req_op_03_roughing_rings_are_one_cut_a_layer_where_nothing_splits_them() {
    let part = Cylinder {
        radius: 8.0,
        length: 20.0,
    };
    let f = rough_fixture(&part, 12.0, &flat(6.0));
    let p = path(&f, &RotaryRoughing, &ringed(rough_values(&[]))).unwrap();
    let spiral = path(&f, &RotaryRoughing, &rough_values(&[])).unwrap();
    // The top layers are above the part all along, so each is one cut, as
    // each of the spiral's is.
    assert_eq!(cut_runs(&p).len(), cut_runs(&spiral).len());
}

#[test]
fn req_nfr_06_req_gc_07_roughing_rings_are_identical_at_1_2_and_n_threads() {
    let f = rough_fixture(&cone(), 15.0, &tool(ToolShape::BallEnd, 4.0));
    identical_at_any_thread_count(
        &f,
        &RotaryRoughing,
        &ringed(rough_values(&[
            (names::STEPOVER, num(2.0)),
            (roughing::STEPDOWN, num(3.0)),
        ])),
    );
}

#[test]
fn req_nfr_07_roughing_refuses_an_unknown_pass_shape_by_name() {
    let f = rough_fixture(&cone(), 15.0, &flat(4.0));
    refused_by_name(&f, &RotaryRoughing, &rough_values(&[]));
}

// -------------------------------------------------------------- facing

fn facing_fixture() -> Fixture {
    let part = Cylinder {
        radius: 8.0,
        length: 20.0,
    };
    let surface = MeshSurface::new(&part.mesh(40, 360)).unwrap();
    let mut f = Fixture::new(surface, 12.0, &flat(4.0));
    f.stock = AnalyticStock::Cylinder {
        radius: 12.0,
        x_start: -6.0,
        x_end: 26.0,
    };
    f
}

fn round_values(extra: &[(&str, ParameterValue)]) -> BTreeMap<String, ParameterValue> {
    let mut v = with(
        &[
            (names::STEPOVER, 2.0),
            (names::FEED, 600.0),
            (names::PLUNGE_FEED, 200.0),
            (roughing::STEPDOWN, 2.0),
            (facing::SIZE, 20.0),
        ],
        extra,
    );
    v.insert(facing::SHAPE.to_owned(), text(facing::ROUND));
    v
}

#[test]
fn req_op_24_round_facing_offers_rings_and_only_for_round() {
    schema_has_passes(&Facing);
    let shown = |shape: &str| {
        let v: BTreeMap<String, ParameterValue> = [(facing::SHAPE.to_owned(), text(shape))]
            .into_iter()
            .collect();
        Facing.schema_for(&v).get(PASS_SHAPE).is_some()
    };
    assert!(shown(facing::ROUND));
    for shape in [facing::FLAT, facing::END, facing::BOX] {
        assert!(!shown(shape), "{shape}");
    }
}

#[test]
fn req_op_24_round_facing_in_rings_turns_the_stock_to_the_diameter() {
    let f = facing_fixture();
    let p = path(&f, &Facing, &ringed(round_values(&[]))).unwrap();
    assert_rings_and_steps(&p, 0.0, 1.0, "round facing");
    let lowest = cuts(&p)
        .iter()
        .map(|(_, s)| level(&f, &s.end))
        .fold(f64::INFINITY, f64::min);
    assert!((lowest - 10.0).abs() < 1e-6, "{lowest}");
    let xs = ring_xs(&p);
    for w in xs.windows(2) {
        assert!(w[1] - w[0] <= 2.0 + 1e-9, "gap {w:?}");
    }
    // Rings reach as far along as the spiral does.
    let spiral = path(&f, &Facing, &round_values(&[])).unwrap();
    let span = |p: &GeneratedPath| {
        cuts(p)
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), (_, s)| {
                (lo.min(s.end.tip.x), hi.max(s.end.tip.x))
            })
    };
    let (a, b) = (span(&p), span(&spiral));
    assert!(a.0 <= b.0 + 1e-9 && a.1 >= b.1 - 1e-9, "{a:?} {b:?}");
}

#[test]
fn req_op_24_req_gc_07_round_facing_spiral_is_unchanged_when_rings_are_not_chosen() {
    let f = facing_fixture();
    let missing = path(&f, &Facing, &round_values(&[])).unwrap();
    let spiral = path(&f, &Facing, &round_values(&[(PASS_SHAPE, text(HELIX))])).unwrap();
    assert_eq!(missing, spiral);
    // Flat Facing ignores the setting.
    let mut flat_v = round_values(&[]);
    flat_v.insert(facing::SHAPE.to_owned(), text(facing::FLAT));
    flat_v.insert(facing::SIZE.to_owned(), num(10.0));
    let a = path(&f, &Facing, &flat_v).unwrap();
    let b = path(&f, &Facing, &ringed(flat_v)).unwrap();
    assert_eq!(a, b);
}

#[test]
fn req_nfr_06_req_gc_07_round_facing_rings_are_identical_at_1_2_and_n_threads() {
    identical_at_any_thread_count(&facing_fixture(), &Facing, &ringed(round_values(&[])));
}

#[test]
fn req_nfr_07_round_facing_refuses_an_unknown_pass_shape_by_name() {
    refused_by_name(&facing_fixture(), &Facing, &round_values(&[]));
}

#[test]
fn req_nfr_07_roughing_rings_refuse_an_absurd_stepover_by_name() {
    let part = Cylinder {
        radius: 8.0,
        length: 20.0,
    };
    let f = rough_fixture(&part, 12.0, &flat(6.0));
    let v = ringed(rough_values(&[
        (names::STEPOVER, num(0.005)),
        (roughing::MAX_ANGLE_STEP, num(1.0)),
    ]));
    let e = path(&f, &RotaryRoughing, &v).unwrap_err().to_string();
    assert!(e.contains(names::STEPOVER) && e.contains("rings"), "{e}");
}
