// SPDX-License-Identifier: AGPL-3.0-or-later
//! The flute length against the deepest cut (TL-04, SET-03, OP-12, NFR-07,
//! NFR-12; issue #530; decision record 0509).
//!
//! The measure is checked against analytic answers: a tool tip at height
//! `h` above the rotary axis, on a pass that turns a full revolution, has
//! the stock's farthest point from the axis (along the tool axis) above it
//! when that point is uppermost: for box stock the corner, its half
//! diagonal from the axis, not the inscribed radius. Each strategy that
//! cuts all the stock above it at once is then refused with short flutes.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

mod support;

use std::collections::BTreeMap;

use openchuck_path::flutes::{Deepest, check, deepest};
use openchuck_path::params::names;
use openchuck_path::rotary::AxisPosition;
use openchuck_path::strategy::{CutPoint, Pass};
use openchuck_path::{AnalyticStock, MeshSurface, ParameterValue, StockModel};
use openchuck_testkit::parts::{RadialPart, revolve};
use openchuck_types::geometry::{Aabb, Point3, UnitVec3, Vec3};
use openchuck_types::tool::{Tool, ToolShape};
use openchuck_types::units::{Degrees, Millimetres};
use support::{Fixture, run, tool};

/// The part: a round bar of radius 5 from X 0 to 20 on the axis.
const BAR: f64 = 5.0;

fn bar() -> MeshSurface {
    MeshSurface::new(&revolve(&[(0.0, BAR), (20.0, BAR)], 96)).unwrap()
}

/// A flat end mill of 3.175 mm with `flutes` mm of flute.
fn flat(flutes: f64) -> Tool {
    let mut t = tool(ToolShape::FlatEnd, 3.175);
    t.flute_length = Millimetres(flutes);
    t.overall_length = Millimetres(flutes + 20.0);
    t
}

/// Box stock across Y and Z, over X -2 to 22.
fn boxed(y: (f64, f64), z: (f64, f64)) -> AnalyticStock {
    AnalyticStock::Box(Aabb {
        min: Point3::new(-2.0, y.0, z.0),
        max: Point3::new(22.0, y.1, z.1),
    })
}

/// A fixture on the bar with `stock` and a flat tool of `flutes` mm.
fn fixture(stock: AnalyticStock, flutes: f64) -> Fixture {
    let mut f = Fixture::new(bar(), 40.0, &flat(flutes));
    f.stock = stock;
    f
}

/// One turn of A at model X 10 with the tip `height` above the axis, every
/// 10 degrees (the measure reads between the points).
fn turn(f: &Fixture, height: f64) -> Vec<Pass> {
    let points = (0..=36)
        .map(|k| {
            let a = Degrees(10.0 * f64::from(k));
            let tip = f.rotary.to_model(
                AxisPosition {
                    along: 10.0,
                    across: 0.0,
                    height,
                },
                a,
            );
            CutPoint {
                tip,
                a,
                contact: tip,
            }
        })
        .collect();
    vec![Pass {
        points,
        ..Pass::default()
    }]
}

fn deepest_of(f: &Fixture, passes: &[Pass]) -> Deepest {
    deepest(&f.input(), passes, 0.0).unwrap()
}

/// The angle of the tool axis at A, degrees from +Y towards +Z, in 0 to 360.
fn axis_angle(f: &Fixture, a: f64) -> f64 {
    let v = f.rotary.tool_axis(Degrees(a)).get();
    v.z.atan2(v.y).to_degrees().rem_euclid(360.0)
}

#[test]
fn req_tl_04_req_set_03_a_box_corner_above_the_axis_is_the_deepest_cut() {
    // 35 x 35 mm box stock (the owner's case): the corner is 24.749 mm from
    // the axis, the inscribed radius 17.5; the tip at the bar's radius.
    let f = fixture(boxed((-17.5, 17.5), (-17.5, 17.5)), 30.0);
    let corner = 17.5_f64.hypot(17.5);
    let d = deepest_of(&f, &turn(&f, BAR));
    eprintln!("35 x 35 box: deepest {:.4} mm at A {:.2}", d.depth, d.a);
    assert!((d.depth - (corner - BAR)).abs() < 1e-3, "{d:?}");
    // At a corner: the tool axis at 45 degrees plus a quarter turn.
    let at = axis_angle(&f, d.a);
    assert!(
        ((at - 45.0).rem_euclid(90.0)).min((45.0 - at).rem_euclid(90.0)) < 0.5,
        "{at}"
    );
}

#[test]
fn req_tl_04_req_nfr_07_a_box_corner_needs_flutes_past_the_half_diagonal() {
    // The boundary either side of 19.749 mm; the inscribed depth (12.5 mm)
    // is not enough.
    let depth = 17.5_f64.hypot(17.5) - BAR;
    for (flutes, refused) in [(12.6, true), (depth - 0.01, true), (depth + 0.01, false)] {
        let f = fixture(boxed((-17.5, 17.5), (-17.5, 17.5)), flutes);
        let r = check(&f.input(), &turn(&f, BAR), None);
        assert_eq!(r.is_err(), refused, "flutes {flutes}: {r:?}");
    }
}

#[test]
fn req_tl_04_req_set_03_a_rectangular_box_is_measured_at_its_corner() {
    // 30 x 50 mm: the corner hypot(15, 25) = 29.155 mm from the axis.
    let f = fixture(boxed((-15.0, 15.0), (-25.0, 25.0)), 30.0);
    let d = deepest_of(&f, &turn(&f, BAR));
    assert!(
        (d.depth - (15.0_f64.hypot(25.0) - BAR)).abs() < 1e-3,
        "{d:?}"
    );
    let at = axis_angle(&f, d.a);
    let corner = 25.0_f64.atan2(15.0).to_degrees();
    let near = [corner, 180.0 - corner, 180.0 + corner, 360.0 - corner]
        .iter()
        .map(|c| (at - c).abs())
        .fold(f64::INFINITY, f64::min);
    assert!(near < 0.5, "axis at {at}, corner at {corner}");
}

#[test]
fn req_tl_04_req_set_03_stock_off_the_axis_is_measured_at_its_farthest_corner() {
    // Bounding box plus offset stock need not be centred on the axis: Y -10
    // to 25, Z -17.5 to 17.5; the farthest corner (25, 17.5).
    let f = fixture(boxed((-10.0, 25.0), (-17.5, 17.5)), 30.0);
    let d = deepest_of(&f, &turn(&f, BAR));
    assert!(
        (d.depth - (25.0_f64.hypot(17.5) - BAR)).abs() < 1e-3,
        "{d:?}"
    );
}

#[test]
fn req_tl_04_a_tip_below_the_axis_adds_its_depth() {
    // A tip 2.5 mm below the axis (the side of an off-axis pin facing it,
    // as on the crankshaft) has the corner 24.749 + 2.5 mm above it.
    let f = fixture(boxed((-17.5, 17.5), (-17.5, 17.5)), 30.0);
    let d = deepest_of(&f, &turn(&f, -2.5));
    assert!(
        (d.depth - (17.5_f64.hypot(17.5) + 2.5)).abs() < 1e-3,
        "{d:?}"
    );
}

/// Square stock turned about X by `turned` degrees: a box in its own frame.
#[derive(Debug)]
struct Turned {
    inner: AnalyticStock,
    turned: f64,
}

impl Turned {
    fn to_box(&self, v: Vec3) -> Vec3 {
        let (s, c) = (-self.turned.to_radians()).sin_cos();
        Vec3::new(v.x, c * v.y - s * v.z, s * v.y + c * v.z)
    }
}

impl StockModel for Turned {
    fn contains(&self, p: Point3) -> bool {
        self.inner
            .contains(Point3::ORIGIN + self.to_box(p - Point3::ORIGIN))
    }

    fn ray_entry(&self, origin: Point3, direction: UnitVec3, max_distance: f64) -> Option<f64> {
        let o = Point3::ORIGIN + self.to_box(origin - Point3::ORIGIN);
        let d = UnitVec3::new(self.to_box(direction.get())).ok()?;
        self.inner.ray_entry(o, d, max_distance)
    }

    fn max_radius(&self) -> Option<f64> {
        self.inner.max_radius()
    }

    fn top_over_disc(&self, centre: Point3, axis: UnitVec3, radius: f64) -> Option<f64> {
        let c = Point3::ORIGIN + self.to_box(centre - Point3::ORIGIN);
        let a = UnitVec3::new(self.to_box(axis.get())).ok()?;
        self.inner.top_over_disc(c, a, radius)
    }
}

#[test]
fn req_tl_04_stock_turned_about_the_axis_is_measured_as_it_lies() {
    let f = fixture(boxed((-17.5, 17.5), (-17.5, 17.5)), 30.0);
    let stock = Turned {
        inner: f.stock,
        turned: 30.0,
    };
    let input = openchuck_path::StrategyInput {
        stock: &stock,
        ..f.input()
    };
    let d = deepest(&input, &turn(&f, BAR), 0.0).unwrap();
    assert!(
        (d.depth - (17.5_f64.hypot(17.5) - BAR)).abs() < 1e-3,
        "{d:?}"
    );
    // The corners are now at 75, 165, 255 and 345 degrees.
    let at = axis_angle(&f, d.a);
    let off = (at - 75.0).rem_euclid(90.0);
    assert!(off.min(90.0 - off) < 0.5, "{at}");
}

#[test]
fn req_tl_04_round_stock_is_measured_at_its_radius() {
    // A 35 mm cylinder: 17.5 - 5 = 12.5 mm, the same at every A.
    let round = AnalyticStock::Cylinder {
        radius: 17.5,
        x_start: -2.0,
        x_end: 22.0,
    };
    let f = fixture(round, 30.0);
    let d = deepest_of(&f, &turn(&f, BAR));
    assert!((d.depth - 12.5).abs() < 1e-3, "{d:?}");
    for (flutes, refused) in [(12.49, true), (12.51, false)] {
        let f = fixture(round, flutes);
        assert_eq!(
            check(&f.input(), &turn(&f, BAR), None).is_err(),
            refused,
            "{flutes}"
        );
    }
}

/// Stock an earlier operation left: a round of radius 5.5 over the bar.
#[derive(Debug)]
struct Roughed(AnalyticStock);

impl StockModel for Roughed {
    fn contains(&self, p: Point3) -> bool {
        self.0.contains(p)
    }
    fn ray_entry(&self, o: Point3, d: UnitVec3, m: f64) -> Option<f64> {
        self.0.ray_entry(o, d, m)
    }
    fn max_radius(&self) -> Option<f64> {
        self.0.max_radius()
    }
    fn top_over_disc(&self, centre: Point3, axis: UnitVec3, radius: f64) -> Option<f64> {
        self.0.top_over_disc(centre, axis, radius)
    }
    fn left_by_earlier_operations(&self) -> bool {
        true
    }
}

#[test]
fn req_tl_04_stock_left_by_earlier_operations_is_measured_not_the_blank() {
    let f = fixture(boxed((-17.5, 17.5), (-17.5, 17.5)), 1.0);
    let roughed = Roughed(AnalyticStock::Cylinder {
        radius: BAR + 0.5,
        x_start: -2.0,
        x_end: 22.0,
    });
    let input = openchuck_path::StrategyInput {
        stock: &roughed,
        ..f.input()
    };
    check(&input, &turn(&f, BAR), None).unwrap();
    let thin = fixture(boxed((-17.5, 17.5), (-17.5, 17.5)), 0.4);
    let input = openchuck_path::StrategyInput {
        stock: &roughed,
        ..thin.input()
    };
    let e = check(&input, &turn(&thin, BAR), None)
        .unwrap_err()
        .to_string();
    assert!(
        e.contains("what the earlier operations in this setup left"),
        "{e}"
    );
}

#[test]
fn req_nfr_07_req_nfr_12_the_refusal_gives_the_numbers_in_plain_words() {
    let f = fixture(boxed((-17.5, 17.5), (-17.5, 17.5)), 25.0);
    let e = check(&f.input(), &turn(&f, -2.5), Some("finishing_layers"))
        .unwrap_err()
        .to_string();
    eprintln!("{e}");
    for words in [
        "parameter finishing_layers",
        "27.249 mm above the tool's tip",
        "25.000 mm of flute",
        "35.000 x 35.000 mm box",
        "24.749 mm from the rotary axis",
        "longer flutes",
        "turn on Finishing layers",
    ] {
        assert!(e.contains(words), "{words:?} in {e}");
    }
    // Nothing says the program is safe or checked (NFR-12).
    for words in ["safe", "verified", "guarantee"] {
        assert!(!e.to_lowercase().contains(words), "{words:?} in {e}");
    }
}

fn values(extra: &[(&str, ParameterValue)]) -> BTreeMap<String, ParameterValue> {
    [
        (names::STEPOVER, ParameterValue::Number(1.0)),
        (names::FEED, ParameterValue::Number(600.0)),
        (names::PLUNGE_FEED, ParameterValue::Number(150.0)),
        (names::TOLERANCE, ParameterValue::Number(0.05)),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v))
    .chain(extra.iter().map(|(k, v)| ((*k).to_owned(), v.clone())))
    .collect()
}

/// Generates `strategy` on the bar in 35 x 35 box stock with `flutes`.
fn on_box(
    strategy: &dyn openchuck_path::Strategy,
    flutes: f64,
    extra: &[(&str, ParameterValue)],
) -> openchuck_path::Result<openchuck_path::GeneratedPath> {
    let f = fixture(boxed((-17.5, 17.5), (-17.5, 17.5)), flutes);
    run(|ctx| support::generate(strategy, &f.input(), &values(extra), ctx))
}

/// The depth the bar in 35 x 35 box stock needs, mm: the corner less the
/// bar's radius.
fn box_depth() -> f64 {
    17.5_f64.hypot(17.5) - BAR
}

fn refused_then_accepted(
    strategy: &dyn openchuck_path::Strategy,
    extra: &[(&str, ParameterValue)],
    parameter: Option<&str>,
) {
    refused_and_accepted_at(
        strategy,
        extra,
        parameter,
        (box_depth() - 0.05, box_depth() + 0.05),
    );
}

/// Refused with `flutes.0` of flute, accepted with `flutes.1`.
fn refused_and_accepted_at(
    strategy: &dyn openchuck_path::Strategy,
    extra: &[(&str, ParameterValue)],
    parameter: Option<&str>,
    flutes: (f64, f64),
) {
    let e = on_box(strategy, flutes.0, extra).unwrap_err();
    let text = e.to_string();
    assert!(text.contains("mm of flute"), "{}: {text}", strategy.name());
    assert!(text.contains("35.000 x 35.000 mm box"), "{text}");
    if let Some(p) = parameter {
        assert!(text.starts_with(&format!("parameter {p}:")), "{text}");
    }
    on_box(strategy, flutes.1, extra).unwrap_or_else(|e| panic!("{}: {e}", strategy.name()));
}

#[test]
fn req_tl_04_req_nfr_07_simultaneous_finishing_is_refused_with_short_flutes() {
    refused_then_accepted(&openchuck_path::SimultaneousFinishing, &[], None);
}

#[test]
fn req_tl_04_req_nfr_07_axial_finishing_is_refused_with_short_flutes() {
    refused_then_accepted(
        &openchuck_path::AxialFinishing,
        &[("face_free_ends", ParameterValue::Flag(false))],
        Some("finishing_layers"),
    );
}

#[test]
fn req_tl_04_req_nfr_07_helical_finishing_is_refused_with_short_flutes() {
    refused_then_accepted(
        &openchuck_path::HelicalFinishing,
        &[("face_free_ends", ParameterValue::Flag(false))],
        Some("finishing_layers"),
    );
}

#[test]
fn req_tl_04_req_nfr_07_finishing_layers_hold_each_layer_to_the_flutes_instead() {
    // With layers on, the Stepdown is what the flutes must reach (0345).
    let layers = [
        ("face_free_ends", ParameterValue::Flag(false)),
        ("finishing_layers", ParameterValue::Flag(true)),
        ("stepdown", ParameterValue::Number(2.0)),
    ];
    on_box(&openchuck_path::AxialFinishing, 5.0, &layers).unwrap();
}

#[test]
fn req_tl_04_req_nfr_07_indexed_finishing_is_refused_with_short_flutes() {
    // A held at faces of the box, not its corners: the tool meets the flat
    // top of the box (17.5 mm from the axis), more than the inscribed depth
    // of 12.5 mm above the bar.
    refused_and_accepted_at(
        &openchuck_path::IndexedFinishing,
        &[],
        Some("finishing_layers"),
        (12.4, 40.0),
    );
}

#[test]
fn req_tl_04_req_nfr_07_rest_machining_on_the_setups_own_stock_is_refused_with_short_flutes() {
    refused_then_accepted(
        &openchuck_path::RestMachining,
        &[("face_free_ends", ParameterValue::Flag(false))],
        None,
    );
}

#[test]
fn req_tl_04_req_nfr_07_pencil_finishing_is_refused_with_short_flutes() {
    // A stepped shaft, radius 5 then 8: one inside corner at X 10; the
    // corner of the 35 x 35 box stands 24.749 - 5 above the tip there.
    let stepped = || {
        MeshSurface::new(&revolve(
            &[(0.0, BAR), (10.0, BAR), (10.0, 8.0), (20.0, 8.0)],
            96,
        ))
        .unwrap()
    };
    let pencil = openchuck_path::PencilFinishing::default();
    let at = |flutes: f64| {
        let mut f = fixture(boxed((-17.5, 17.5), (-17.5, 17.5)), flutes);
        f.surface = stepped();
        run(|ctx| support::generate(&pencil, &f.input(), &values(&[]), ctx))
    };
    let e = at(box_depth() - 0.5).unwrap_err().to_string();
    assert!(e.contains("mm of flute"), "{e}");
    at(box_depth() + 0.5).unwrap();
}

#[test]
fn req_nfr_07_a_roughing_stepdown_past_the_flutes_is_refused() {
    // OI-153: a layer deeper than the flutes would cut with the shank.
    let rough = openchuck_path::RotaryRoughing;
    let e = on_box(&rough, 3.0, &[("stepdown", ParameterValue::Number(3.5))])
        .unwrap_err()
        .to_string();
    assert!(e.starts_with("parameter stepdown:"), "{e}");
    assert!(e.contains("3.000 mm of flute"), "{e}");
    // The layers start at the corner, so a Stepdown within the flutes is
    // all the check roughing needs.
    on_box(&rough, 3.0, &[("stepdown", ParameterValue::Number(3.0))]).unwrap();
}

#[test]
fn req_op_08_req_tl_04_req_nfr_07_edge_following_counts_the_stock_above_the_surface() {
    // The bar's end edges engraved 0.3 mm deep in 35 x 35 box stock: the
    // corner stands 24.749 - 4.7 mm above the tip, not the 0.3 mm the
    // depth alone asks (the depth check of OP-08 measured from the part).
    let edges = openchuck_path::EdgeFollowing::default();
    let depth = [(
        openchuck_path::edge_following::parameter::DEPTH,
        ParameterValue::Number(0.3),
    )];
    let need = 17.5_f64.hypot(17.5) - (BAR - 0.3);
    refused_and_accepted_at(&edges, &depth, None, (need - 0.05, need + 0.05));
}

// Cuts below the middle of the stock (the owner's question on #530): a tip
// below the rotary axis has the stock's far side above it, the farthest
// point's radius plus the depth below the axis.

#[test]
fn req_tl_04_a_cut_below_the_midpoint_adds_its_depth_below_the_axis_in_both_senses_of_a() {
    let round = AnalyticStock::Cylinder {
        radius: 17.5,
        x_start: -2.0,
        x_end: 22.0,
    };
    let square = boxed((-17.5, 17.5), (-17.5, 17.5));
    for sense in [
        openchuck_types::machine::ASense::Positive,
        openchuck_types::machine::ASense::Negative,
    ] {
        for (stock, top) in [(round, 17.5), (square, 17.5_f64.hypot(17.5))] {
            for below in [5.0, 10.0, 15.0] {
                let want = top + below;
                let mut f = fixture(stock, 50.0);
                f.rotary.sense = sense;
                let d = deepest_of(&f, &turn(&f, -below));
                assert!(
                    (d.depth - want).abs() < 1e-3,
                    "{sense:?} {stock:?} {below} below: {d:?}"
                );
                for (flutes, refused) in [(want - 0.01, true), (want + 0.01, false)] {
                    let mut f = fixture(stock, flutes);
                    f.rotary.sense = sense;
                    assert_eq!(
                        check(&f.input(), &turn(&f, -below), None).is_err(),
                        refused,
                        "{sense:?} {stock:?} {below} below, flutes {flutes}"
                    );
                }
            }
        }
    }
}

#[test]
fn req_tl_04_the_quick_bound_never_skips_a_tip_below_the_axis() {
    // The bound is the farthest radius less the signed tip height: below
    // the axis it grows, so the place is read. Flutes just short of the
    // real depth are refused at every depth below the axis.
    for below in [0.5, 2.5, 7.5, 12.5] {
        let want = 17.5_f64.hypot(17.5) + below;
        let f = fixture(boxed((-17.5, 17.5), (-17.5, 17.5)), want - 0.02);
        assert!(
            check(&f.input(), &turn(&f, -below), None).is_err(),
            "{below}"
        );
    }
}

/// A pin of radius 3 whose centre is 13 mm from the axis, X 0 to 20: its
/// side facing the axis is 10 mm from it, so with the pin under the axis
/// the tool's tip is 10 mm below it.
fn pin() -> MeshSurface {
    let mut mesh = openchuck_testkit::parts::Cylinder {
        radius: 3.0,
        length: 20.0,
    }
    .mesh(41, 180);
    for v in &mut mesh.vertices {
        *v = Point3::new(v.x, v.y, v.z - 13.0);
    }
    MeshSurface::new(&mesh).unwrap()
}

/// The depth a refusal names: "stands up to D mm".
fn named_depth(message: &str) -> f64 {
    let rest = message.split("stands up to ").nth(1).unwrap();
    rest.split(' ').next().unwrap().parse().unwrap()
}

/// `strategy` on the pin in 35 x 35 box stock: refused with 24 mm of flute,
/// naming a depth past the box's farthest corner (24.749 mm), which only a
/// tip below the axis has; accepted with 36 mm, past the bound 24.749 + 10.
fn below_the_axis(strategy: &dyn openchuck_path::Strategy, extra: &[(&str, ParameterValue)]) {
    let on = |flutes: f64| {
        let mut f = fixture(boxed((-17.5, 17.5), (-17.5, 17.5)), flutes);
        f.surface = pin();
        run(|ctx| support::generate(strategy, &f.input(), &values(extra), ctx))
    };
    let e = on(24.0).unwrap_err().to_string();
    let depth = named_depth(&e);
    eprintln!("{}: {e}", strategy.name());
    assert!(
        depth > 17.5_f64.hypot(17.5) + 1.0,
        "{}: {e}",
        strategy.name()
    );
    assert!(depth <= 17.5_f64.hypot(17.5) + 10.0 + 1e-6, "{e}");
    on(36.0).unwrap_or_else(|e| panic!("{}: {e}", strategy.name()));
}

#[test]
fn req_tl_04_simultaneous_finishing_below_the_midpoint_is_measured() {
    below_the_axis(&openchuck_path::SimultaneousFinishing, &[]);
}

#[test]
fn req_tl_04_axial_finishing_below_the_midpoint_is_measured() {
    below_the_axis(
        &openchuck_path::AxialFinishing,
        &[("face_free_ends", ParameterValue::Flag(false))],
    );
}

#[test]
fn req_tl_04_helical_finishing_below_the_midpoint_is_measured() {
    below_the_axis(
        &openchuck_path::HelicalFinishing,
        &[("face_free_ends", ParameterValue::Flag(false))],
    );
}

#[test]
fn req_tl_04_indexed_finishing_below_the_midpoint_is_measured() {
    below_the_axis(&openchuck_path::IndexedFinishing, &[]);
}

#[test]
fn req_tl_04_rest_machining_below_the_midpoint_is_measured() {
    below_the_axis(
        &openchuck_path::RestMachining,
        &[("face_free_ends", ParameterValue::Flag(false))],
    );
}

fn crankshaft() -> MeshSurface {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/stl/crankshaft.stl");
    MeshSurface::new(&support::read_binary_stl(&path)).unwrap()
}

/// The owner's case on the public CrankShaft model: simultaneous finishing
/// with a 3.175 mm flat tool, in 35 x 35 mm box stock over the part, and
/// the same box 2.5 mm off the axis in Z.
fn crank(flutes: f64, z: (f64, f64)) -> openchuck_path::Result<openchuck_path::GeneratedPath> {
    let mut f = Fixture::new(crankshaft(), 40.0, &flat(flutes));
    f.stock = AnalyticStock::Box(Aabb {
        min: Point3::new(-1.0, -17.5, z.0),
        max: Point3::new(41.0, 17.5, z.1),
    });
    let v = values(&[
        (names::STEPOVER, ParameterValue::Number(0.45)),
        (names::TOLERANCE, ParameterValue::Number(0.01)),
    ]);
    run(|ctx| support::generate(&openchuck_path::SimultaneousFinishing, &f.input(), &v, ctx))
}

#[test]
fn req_tl_04_req_nfr_07_the_crankshaft_in_35_by_35_box_stock_is_refused_with_25_mm_of_flute() {
    // The pins' sides facing the axis are 2.5 mm from it: with a pin under
    // the axis the tip is 2.5 mm below it (bound 24.749 + 2.502).
    let e = crank(25.0, (-17.5, 17.5)).unwrap_err().to_string();
    eprintln!("centred: {e}");
    assert!(e.contains("25.000 mm of flute"), "{e}");
    assert!(e.contains("35.000 x 35.000 mm box"), "{e}");
    assert!(named_depth(&e) > 25.0, "{e}");
    crank(28.5, (-17.5, 17.5)).unwrap();
    // The same box 2.5 mm off the axis in Z, as bounding-box stock can be:
    // its farthest corner is hypot(17.5, 20) = 26.575 mm from the axis, and
    // the cut approaches 30 mm (the owner's report).
    let e = crank(25.0, (-15.0, 20.0)).unwrap_err().to_string();
    eprintln!("off the axis: {e}");
    assert!(e.contains("26.575 mm from the rotary axis"), "{e}");
    assert!(named_depth(&e) > 26.0, "{e}");
}

#[test]
fn req_tl_04_req_nfr_07_pencil_finishing_on_the_crankshaft_is_refused_at_the_depth_it_measures() {
    // Pencil's creases on the CrankShaft model in 35 x 35 box stock: its
    // passes do not go below the axis there (the deepest is 13.5 mm, by the
    // creases it finds), so this is the refusal boundary at the depth the
    // shared measure reads, either side by 0.05 mm.
    let pencil = openchuck_path::PencilFinishing::default();
    let on = |flutes: f64| {
        let mut f = Fixture::new(crankshaft(), 40.0, &flat(flutes));
        f.stock = AnalyticStock::Box(Aabb {
            min: Point3::new(-1.0, -17.5, -17.5),
            max: Point3::new(41.0, 17.5, 17.5),
        });
        run(|ctx| support::generate(&pencil, &f.input(), &values(&[]), ctx))
    };
    let e = on(1.0).unwrap_err().to_string();
    eprintln!("pencil: {e}");
    let depth = named_depth(&e);
    assert!(on(depth - 0.05).is_err());
    on(depth + 0.05).unwrap();
}

/// A pass of points at X 10, the tip `height` above the axis, at the given A
/// values (degrees, as a strategy gives them).
fn at_angles(f: &Fixture, height: f64, angles: &[f64]) -> Vec<Pass> {
    let points = angles
        .iter()
        .map(|&a| {
            let a = Degrees(a);
            let tip = f.rotary.to_model(
                AxisPosition {
                    along: 10.0,
                    across: 0.0,
                    height,
                },
                a,
            );
            CutPoint {
                tip,
                a,
                contact: tip,
            }
        })
        .collect();
    vec![Pass {
        points,
        ..Pass::default()
    }]
}

#[test]
fn req_tl_04_req_nfr_07_a_narrow_strip_of_stock_between_readings_is_not_missed() {
    // CodeRabbit on #534: a strip of stock 0.1 mm wide, 0.95 mm across the
    // tool axis from it (between the rings of readings at 0.79 and 1.59 mm on
    // a 3.175 mm tool, where no reading of a ring lies across 0.90 to 1.00
    // mm, and out of reach of the search about an empty centre) and 15 mm
    // above the tip, is read exactly for box stock
    // (`StockModel::top_over_disc`, decision record 0509).
    let probe = fixture(boxed((-17.5, 17.5), (-17.5, 17.5)), 30.0);
    let a = Degrees(30.0);
    let axis = probe.rotary.tool_axis(a).get();
    let across = Vec3::new(0.0, -axis.z, axis.y);
    let tip = probe.rotary.to_model(
        AxisPosition {
            along: 10.0,
            across: 0.0,
            height: BAR,
        },
        a,
    );
    let centre = tip + axis * 15.0 + across * 0.95;
    let strip = AnalyticStock::Box(Aabb {
        min: Point3::new(-2.0, centre.y - 0.05, centre.z - 0.05),
        max: Point3::new(22.0, centre.y + 0.05, centre.z + 0.05),
    });
    let f = fixture(strip, 30.0);
    let d = deepest_of(&f, &at_angles(&f, BAR, &[30.0, 30.0]));
    // The strip's highest corner along the axis: 15 mm and a little more,
    // as the square is turned to the axis.
    assert!(d.depth > 15.0 && d.depth < 15.08, "{d:?}");
    let f = fixture(strip, 14.9);
    assert!(check(&f.input(), &at_angles(&f, BAR, &[30.0, 30.0]), None).is_err());
}

#[test]
fn req_tl_04_a_pass_across_the_0_360_seam_turns_the_short_way() {
    // CodeRabbit on #534: A given as 350 then 10 turns 20 degrees, as
    // assembly turns it, not 340 the other way round (which would read the
    // box's corner at 45 degrees and refuse a valid path).
    let f = fixture(boxed((-17.5, 17.5), (-17.5, 17.5)), 30.0);
    let raw = deepest_of(&f, &at_angles(&f, BAR, &[340.0, 350.0, 0.0, 10.0, 20.0]));
    let unwrapped = deepest_of(
        &f,
        &at_angles(&f, BAR, &[340.0, 350.0, 360.0, 370.0, 380.0]),
    );
    assert!(
        (raw.depth - unwrapped.depth).abs() < 1e-9,
        "{raw:?} {unwrapped:?}"
    );
    // Within 20 degrees of a face, short of the corner's 19.749 mm.
    assert!(
        raw.depth < 17.5 / 20.0_f64.to_radians().cos() - BAR + 1.0,
        "{raw:?}"
    );
}

/// Stock with no exact answer over the tool's disc: the round of radius
/// `0.5` mm past the bar, as a simulation's cells might hold it, known only
/// by rays and its farthest radius, or by nothing at all.
#[derive(Debug)]
struct Unmeasured {
    inner: AnalyticStock,
    radius_known: bool,
}

impl StockModel for Unmeasured {
    fn contains(&self, p: Point3) -> bool {
        self.inner.contains(p)
    }
    fn ray_entry(&self, o: Point3, d: UnitVec3, m: f64) -> Option<f64> {
        self.inner.ray_entry(o, d, m)
    }
    fn max_radius(&self) -> Option<f64> {
        self.radius_known.then(|| self.inner.max_radius()).flatten()
    }
}

#[test]
fn req_tl_04_req_nfr_07_stock_with_no_exact_answer_is_held_to_a_conservative_bound() {
    // Decision record 0509 (CodeRabbit on #534): a stock model that does not
    // give `top_over_disc` is never read by sampling. Its bound is its
    // farthest radius less the tip's height plus 1 mm: 5.5 - 5 + 1 = 1.5 mm
    // over the bar, past the 0.5 mm the round really stands.
    let round = AnalyticStock::Cylinder {
        radius: BAR + 0.5,
        x_start: -2.0,
        x_end: 22.0,
    };
    let bounded = Unmeasured {
        inner: round,
        radius_known: true,
    };
    let at = |flutes: f64, stock: &dyn StockModel| {
        let f = fixture(boxed((-17.5, 17.5), (-17.5, 17.5)), flutes);
        let input = openchuck_path::StrategyInput { stock, ..f.input() };
        check(&input, &turn(&f, BAR), None).map_err(|e| e.to_string())
    };
    // Exact for the same round: 0.5 mm.
    assert!(at(0.6, &round).is_ok());
    // Bounded: refused at 1.4 mm, though the round stands 0.5 mm; the words
    // say it is a bound.
    let e = at(1.4, &bounded).unwrap_err();
    assert!(e.contains("may stand up to 1.500 mm"), "{e}");
    assert!(e.contains("not measured exactly"), "{e}");
    assert!(at(1.6, &bounded).is_ok());
    // No farthest radius either: refused, whatever the flutes.
    let blind = Unmeasured {
        inner: round,
        radius_known: false,
    };
    let e = at(100.0, &blind).unwrap_err();
    assert!(e.contains("the stock's size is not known"), "{e}");
}

#[test]
fn req_tl_04_req_nfr_07_a_narrow_round_of_stock_between_readings_is_not_missed() {
    // The same for round stock: a rod of radius 0.06 mm along X, 0.7 mm
    // across the tool axis and 15 mm above the tip at A 0, would sit
    // between rings of readings; `top_over_disc` reads it exactly. A
    // cylinder of stock is on the rotary axis, so the rod is the stock's
    // whole section seen from a tip 15.06 mm below its top and 0.7 mm aside.
    let f = fixture(
        AnalyticStock::Cylinder {
            radius: 0.06,
            x_start: -2.0,
            x_end: 22.0,
        },
        30.0,
    );
    let a = Degrees(0.0);
    let axis = f.rotary.tool_axis(a).get();
    let across = Vec3::new(0.0, -axis.z, axis.y);
    // The tip 15 mm below the rod and 0.7 mm aside of it.
    let tip = Point3::ORIGIN + axis * -15.0 + across * 0.7;
    let tip = f.rotary.to_axis(Point3::new(10.0, tip.y, tip.z), a);
    let pass = vec![Pass {
        points: vec![
            CutPoint {
                tip: f.rotary.to_model(tip, a),
                a,
                contact: f.rotary.to_model(tip, a),
            };
            2
        ],
        ..Pass::default()
    }];
    let d = deepest(&f.input(), &pass, 0.0).unwrap();
    assert!(d.exact && (d.depth - 15.06).abs() < 1e-6, "{d:?}");
    let short = fixture(
        AnalyticStock::Cylinder {
            radius: 0.06,
            x_start: -2.0,
            x_end: 22.0,
        },
        15.0,
    );
    assert!(check(&short.input(), &pass, None).is_err());
}

#[test]
fn req_tl_04_an_exact_half_turn_reads_both_ways_round() {
    // CodeRabbit on #534 (second and third reviews): at an exact half turn
    // the way assembly turns A depends on whole turns it carries from earlier
    // points and passes (it unwraps from 0 and carries the angle across
    // passes) and on its fit to the A range, so the check reads both ways
    // round and takes the deeper. Each case below has box stock off the axis
    // whose deeper half is the one assembly cuts, the other half 8.4 mm
    // shallower, so reading only the raw step's nearest turn would accept.
    use openchuck_path::rotary::{nearest_turn, unwrap_continuous};
    let read = |f: &Fixture, passes: &[&[f64]]| {
        let all: Vec<Pass> = passes
            .iter()
            .flat_map(|angles| at_angles(f, BAR, angles))
            .collect();
        deepest(&f.input(), &all, 0.0).unwrap().depth
    };

    // The first pass of a path given as 350 then 170: assembly unwraps it
    // from 0 to -10 then -190, down through 260; the raw step's nearest turn
    // goes up through 80.
    let cut = unwrap_continuous(&[Degrees(350.0), Degrees(170.0)], Degrees(0.0));
    assert_eq!((cut[0].0, cut[1].0), (-10.0, -190.0));
    assert_eq!(nearest_turn(Degrees(170.0), Degrees(350.0)).0, 530.0);
    let f = fixture(boxed((-25.0, 10.0), (-25.0, 10.0)), 30.0);
    let cut_way = read(&f, &[&[350.0, 260.0, 170.0]]);
    let other_way = read(&f, &[&[350.0, 80.0, 170.0]]);
    assert!(cut_way > other_way + 8.0, "{cut_way} {other_way}");
    assert!((read(&f, &[&[350.0, 170.0]]) - cut_way).abs() < 1e-9);

    // A later pass whose carried angle differs from its first raw angle by a
    // whole turn: after a pass ending at 360, a pass given as 10 then 190 is
    // cut from 370 up to 550, while from 10 alone the nearest turn of 190
    // goes down to -170.
    let carried = unwrap_continuous(&[Degrees(10.0), Degrees(190.0)], Degrees(360.0));
    assert_eq!((carried[0].0, carried[1].0), (370.0, 550.0));
    assert_eq!(nearest_turn(Degrees(190.0), Degrees(10.0)).0, -170.0);
    let f = fixture(boxed((-10.0, 25.0), (-10.0, 25.0)), 30.0);
    let cut_way = read(&f, &[&[10.0, 100.0, 190.0]]);
    let other_way = read(&f, &[&[10.0, -80.0, -170.0]]);
    assert!(cut_way > other_way + 8.0, "{cut_way} {other_way}");
    // The first pass stays clear of the deepest corner, so the second pass's
    // half turn decides the answer.
    let first: &[f64] = &[0.0, 0.0];
    assert!(read(&f, &[first]) < cut_way);
    let both = read(&f, &[first, &[10.0, 190.0]]);
    assert!((both - cut_way).abs() < 1e-9, "{both} {cut_way}");
}

/// A strategy for the half-turn tests: it gives the passes it holds, tips
/// on the bar at X 10, at the angles given (as a strategy gives them, not
/// unwrapped), and leaves the rest to the shared generation and assembly.
#[derive(Debug)]
struct Given(Vec<Vec<f64>>);

impl openchuck_path::Strategy for Given {
    fn name(&self) -> &str {
        "test_given"
    }
    fn label(&self) -> &str {
        "Given passes (test)"
    }
    fn description(&self) -> &str {
        "A strategy for tests."
    }
    fn schema(&self) -> openchuck_path::ParameterSchema {
        openchuck_path::ParameterSchema::with_common(Vec::new())
    }
    fn passes(
        &self,
        input: &openchuck_path::StrategyInput<'_>,
        _p: &openchuck_path::Parameters,
        _context: &openchuck_types::job::JobContext<'_>,
    ) -> openchuck_path::Result<Vec<Pass>> {
        Ok(self
            .0
            .iter()
            .map(|angles| Pass {
                points: angles
                    .iter()
                    .map(|&a| {
                        let a = Degrees(a);
                        let tip = input.rotary.to_model(
                            AxisPosition {
                                along: 10.0,
                                across: 0.0,
                                height: BAR,
                            },
                            a,
                        );
                        CutPoint {
                            tip,
                            a,
                            contact: tip,
                        }
                    })
                    .collect(),
                ..Pass::default()
            })
            .collect())
    }
}

#[test]
fn req_tl_04_req_nfr_07_the_generation_checks_the_turn_assembly_writes() {
    // CodeRabbit on #534 (third review): the generation checks the path as
    // assembly wrote it, so at an exact half turn the arc read is the arc
    // cut, whatever the whole turns assembly carries. A first pass given as
    // 350 then 170 is cut down through 260; with box stock Y and Z -25 to 10
    // that half stands 30.355 mm above the tip and the other 21.926 mm, so
    // 25 mm of flute is refused and 31 mm is accepted.
    let on = |flutes: f64, passes: Vec<Vec<f64>>, y: (f64, f64), z: (f64, f64)| {
        let f = fixture(boxed(y, z), flutes);
        run(|ctx| support::generate(&Given(passes), &f.input(), &values(&[]), ctx)).map(|p| p.group)
    };
    let first = vec![vec![350.0, 170.0]];
    let neg = (-25.0, 10.0);
    let e = on(25.0, first.clone(), neg, neg).unwrap_err().to_string();
    assert!(e.contains("30.355 mm"), "{e}");
    let cut = on(31.0, first, neg, neg).unwrap();
    // The path written turns down: its fed moves go from -10 to -190.
    let fed: Vec<f64> = cut
        .segments
        .iter()
        .filter(|s| s.kind == openchuck_types::ir::MoveKind::Cutting)
        .map(|s| s.end.a.0)
        .collect();
    assert!(fed.iter().any(|a| (*a + 190.0).abs() < 1e-9), "{fed:?}");
    // A later pass given as 10 then 190 after one ending at 360 is cut up
    // from 370 to 550; with box stock Y and Z -10 to 25 that half is the
    // deeper.
    let pos = (-10.0, 25.0);
    let carried = vec![vec![0.0, 90.0, 180.0, 270.0, 360.0], vec![10.0, 190.0]];
    let e = on(25.0, carried.clone(), pos, pos).unwrap_err().to_string();
    assert!(e.contains("30.355 mm"), "{e}");
    on(31.0, carried, pos, pos).unwrap();
    // The owner's Codex review of #534: a first pass given as 270 then 90,
    // with box stock Y -10 to 25 and Z -25 to 10, is unwrapped by assembly
    // to -90 then -270 and reaches 30.355 mm (at -225); the raw step's
    // nearest turn, 270 to 450, reads 21.926 mm only.
    let codex = (vec![vec![270.0, 90.0]], (-10.0, 25.0), (-25.0, 10.0));
    let e = on(25.0, codex.0.clone(), codex.1, codex.2)
        .unwrap_err()
        .to_string();
    assert!(e.contains("30.355 mm"), "{e}");
    on(31.0, codex.0.clone(), codex.1, codex.2).unwrap();
    // And the same half turn after a pass that wraps: after 0 to 360, the
    // pass is carried to 630 and turns up to 810 (90 to 270 on the stock, through 180).
    let after_wrap = vec![vec![0.0, 90.0, 180.0, 270.0, 360.0], vec![270.0, 90.0]];
    let raw = on(
        25.0,
        vec![vec![0.0, 0.0], vec![270.0, 450.0]],
        codex.1,
        codex.2,
    );
    let carried = on(25.0, after_wrap, codex.1, codex.2);
    assert_eq!(raw.is_err(), carried.is_err(), "{raw:?} {carried:?}");
}

#[test]
fn req_tl_04_req_nfr_07_stock_between_two_readings_of_a_move_is_not_missed() {
    // The owner's Codex review of #534: a 3 mm flat tool with 1 mm of flute
    // moves 0.5 mm along X at A 0, its tip from X 0 to 0.5. A slab of stock
    // 0.1 mm long in X (0.2 to 0.3), 1.49 to 1.50 mm across the tool axis
    // and 15 mm tall is out of the tool's 1.5 mm radius at both ends of the
    // move (1.487 mm reach at 0.2 mm off in X) but inside it at X 0.25, 15
    // mm above the tip: the move is refused, not read at its ends only.
    let mut t = tool(ToolShape::FlatEnd, 3.0);
    t.flute_length = Millimetres(1.0);
    t.overall_length = Millimetres(20.0);
    let probe = Fixture::new(bar(), 40.0, &t);
    let a = Degrees(0.0);
    let axis = probe.rotary.tool_axis(a).get();
    let across = Vec3::new(0.0, -axis.z, axis.y);
    let tip = |along: f64| {
        probe.rotary.to_model(
            AxisPosition {
                along,
                across: 0.0,
                height: 0.0,
            },
            a,
        )
    };
    let (start, end) = (tip(0.0), tip(0.5));
    assert!((end - start - Vec3::new(0.5, 0.0, 0.0)).length() < 1e-12);
    let corner = |x: f64, w: f64, h: f64| start + Vec3::new(x, 0.0, 0.0) + across * w + axis * h;
    let (p, q) = (corner(0.2, 1.49, 0.0), corner(0.3, 1.50, 15.0));
    let slab = AnalyticStock::Box(Aabb {
        min: Point3::new(p.x.min(q.x), p.y.min(q.y), p.z.min(q.z)),
        max: Point3::new(p.x.max(q.x), p.y.max(q.y), p.z.max(q.z)),
    });
    let mut f = Fixture::new(bar(), 40.0, &t);
    f.stock = slab;
    let pass = |points: &[Point3]| {
        vec![Pass {
            points: points
                .iter()
                .map(|&tip| CutPoint {
                    tip,
                    a,
                    contact: tip,
                })
                .collect(),
            ..Pass::default()
        }]
    };
    // Neither end alone meets it.
    assert!(check(&f.input(), &pass(&[start]), None).is_ok());
    assert!(check(&f.input(), &pass(&[end]), None).is_ok());
    let e = check(&f.input(), &pass(&[start, end]), None)
        .unwrap_err()
        .to_string();
    assert!(e.contains("1.000 mm of flute"), "{e}");
    assert!(named_depth(&e) >= 15.0 - 1e-6, "{e}");
    // With 15.1 mm of flute the same move is accepted: the bound between
    // readings is refined, not taken as the depth.
    t.flute_length = Millimetres(15.1);
    let mut f = Fixture::new(bar(), 40.0, &t);
    f.stock = slab;
    check(&f.input(), &pass(&[start, end]), None).unwrap();
}

/// An assembled path at A 0, heights above the rotary axis: a rapid to 30
/// mm at X 10, then `kind` down to `deep` and back up to 30, then a plunge
/// to `cut` and a cut along X at that height to X 14.
fn written(
    f: &Fixture,
    kind: openchuck_types::ir::MoveKind,
    deep: f64,
    cut: f64,
) -> openchuck_path::GeneratedPath {
    use openchuck_types::ir::{MoveKind, PathTags, Segment, ToolPose, ToolpathGroup};
    let a = Degrees(0.0);
    let pose = |x: f64, height: f64| ToolPose {
        tip: f.rotary.to_model(
            AxisPosition {
                along: x,
                across: 0.0,
                height,
            },
            a,
        ),
        axis: f.rotary.tool_axis(a),
        a,
    };
    let segment = |kind: MoveKind, end: ToolPose| Segment {
        kind,
        end,
        contact_feed: kind
            .is_fed()
            .then_some(openchuck_types::units::FeedRate(300.0)),
        duration: openchuck_types::units::Seconds(1.0),
        contact_length: Millimetres(1.0),
    };
    openchuck_path::GeneratedPath {
        group: ToolpathGroup {
            rotary_reset_after: Vec::new(),
            tags: PathTags {
                operation: "op".to_owned(),
                setup: "setup".to_owned(),
                tool: "tool".to_owned(),
            },
            start: pose(10.0, 40.0),
            // Down to `deep` and back up by `kind`, then a plunge to the
            // cut, the cut, and away.
            segments: vec![
                segment(MoveKind::Rapid, pose(10.0, 30.0)),
                segment(kind, pose(10.0, deep)),
                segment(kind, pose(10.0, 30.0)),
                segment(MoveKind::Plunge, pose(10.0, cut)),
                segment(MoveKind::Cutting, pose(14.0, cut)),
                segment(MoveKind::Rapid, pose(14.0, 40.0)),
            ],
        },
        notes: Vec::new(),
        warnings: Vec::new(),
        unreached: Vec::new(),
        shank_breaks: Vec::new(),
    }
}

#[test]
fn req_tl_04_req_nfr_07_fed_moves_deeper_than_the_cut_are_refused_and_rapids_are_not_read() {
    // Decision record 0509 (the coordinator's conditions on #534): the
    // generation reads every fed move of the path as assembly wrote it, so
    // a plunge, a lead or a fed link that goes deeper than the cut is held
    // to the flutes too, with the numbers; a rapid is not a cut and is not
    // read. Round stock of radius 17.5: the cut at 15 over the axis stands
    // 2.5 mm under the stock's top, the deep move at 5 over it 12.5 mm.
    use openchuck_path::flutes::check_path;
    use openchuck_types::ir::MoveKind;
    let round = AnalyticStock::Cylinder {
        radius: 17.5,
        x_start: -2.0,
        x_end: 22.0,
    };
    let f = fixture(round, 10.0);
    for kind in [
        MoveKind::Plunge,
        MoveKind::LeadIn,
        MoveKind::LeadOut,
        MoveKind::Linking,
        MoveKind::Cutting,
    ] {
        let e = run(|ctx| check_path(&f.input(), &written(&f, kind, 5.0, 15.0), None, ctx))
            .unwrap_err()
            .to_string();
        assert!(e.contains("stands up to 12.500 mm"), "{kind:?}: {e}");
        assert!(e.contains("10.000 mm of flute"), "{kind:?}: {e}");
    }
    // The same depth reached at a rapid is not read: only the 2.5 mm cut.
    run(|ctx| {
        check_path(
            &f.input(),
            &written(&f, MoveKind::Rapid, 5.0, 15.0),
            None,
            ctx,
        )
    })
    .unwrap();
    // And the cut alone is within 10 mm of flute; the deep plunge is past it
    // with 12 mm and within it with 13 mm.
    let deep = written(&f, MoveKind::Plunge, 5.0, 15.0);
    let f12 = fixture(round, 12.0);
    assert!(run(|ctx| check_path(&f12.input(), &deep, None, ctx)).is_err());
    let f13 = fixture(round, 13.0);
    run(|ctx| check_path(&f13.input(), &deep, None, ctx)).unwrap();
}

#[test]
fn req_tl_04_req_nfr_07_stock_flush_with_the_flutes_is_settled_in_bounded_work() {
    // CodeRabbit on #534 (review of 6580f9b): stock whose top stands exactly
    // as far above the tip as the flutes reach keeps each step's bound
    // between readings just past the flutes, so halving alone would read a
    // step about a million times. Decision record 0509: the bound grows
    // along the tool axis only by the tip's own rise and the stock's turn,
    // so a straight cut at one height under a flat top is settled with no
    // halving and accepted; and a step has a budget of readings, past which
    // its bound is kept and the cut refused as a bound, never accepted
    // unread. Each case takes well under a second.
    let timed = |f: &Fixture, passes: &[Pass]| {
        let started = std::time::Instant::now();
        let result = check(&f.input(), passes, None);
        let took = started.elapsed().as_secs_f64();
        assert!(took < 1.0, "{took} s");
        result
    };

    // A straight cut 10 mm along X at A 0 under a flat face 10 mm above the
    // tip, with 10 mm of flute: flush, accepted.
    let probe = fixture(boxed((-20.0, 20.0), (-20.0, 20.0)), 10.0);
    let a = Degrees(0.0);
    let axis = probe.rotary.tool_axis(a).get();
    let tip = |along: f64| {
        probe.rotary.to_model(
            AxisPosition {
                along,
                across: 0.0,
                height: 2.0,
            },
            a,
        )
    };
    let top = tip(0.0) + axis * 10.0;
    let hi = if axis.z > 0.5 {
        Point3::new(22.0, 20.0, top.z)
    } else {
        assert!(axis.y > 0.5, "{axis:?}");
        Point3::new(22.0, top.y, 20.0)
    };
    let mut f = fixture(boxed((-20.0, 20.0), (-20.0, 20.0)), 10.0);
    f.stock = AnalyticStock::Box(Aabb {
        min: Point3::new(-2.0, -20.0, -20.0),
        max: hi,
    });
    let straight = vec![Pass {
        points: (0..=20)
            .map(|k| {
                let tip = tip(0.5 * f64::from(k));
                CutPoint {
                    tip,
                    a,
                    contact: tip,
                }
            })
            .collect(),
        ..Pass::default()
    }];
    timed(&f, &straight).unwrap();

    // A turn on round stock of radius 17.5 with the tip 5 above the axis:
    // the stock stands 12.5 mm above it all round. Round stock on the axis
    // is the same at every A (`StockModel::turns_unchanged`), so a turn
    // adds nothing to a step's bound: with 12.5 mm of flute, flush, it is
    // settled with no halving and accepted, as a free end turned flush
    // with the flutes on round stock is.
    let round = AnalyticStock::Cylinder {
        radius: 17.5,
        x_start: -2.0,
        x_end: 22.0,
    };
    let f = fixture(round, 12.5);
    timed(&f, &turn(&f, BAR)).unwrap();

    // The same stock as a model that does not say a turn leaves it
    // unchanged: the turning steps' bounds stay past the flutes, so it is
    // refused as a bound within the budget; with 12.51 mm, accepted.
    let bounded = |flutes: f64| {
        let f = fixture(round, flutes);
        let stock = Turned {
            inner: f.stock,
            turned: 0.0,
        };
        let started = std::time::Instant::now();
        let input = openchuck_path::StrategyInput {
            stock: &stock,
            ..f.input()
        };
        let result = check(&input, &turn(&f, BAR), None).map_err(|e| e.to_string());
        let took = started.elapsed().as_secs_f64();
        assert!(took < 1.0, "{took} s");
        result
    };
    let e = bounded(12.5).unwrap_err();
    assert!(e.contains("may stand up to 12.50"), "{e}");
    assert!(e.contains("not measured exactly"), "{e}");
    bounded(12.51).unwrap();
}

/// A stock model that gives its exact top but not its farthest radius, as
/// the `StockModel` contract allows (the owner's Codex review of 6580f9b).
#[derive(Debug)]
struct NoRadius(AnalyticStock);

impl StockModel for NoRadius {
    fn contains(&self, p: Point3) -> bool {
        self.0.contains(p)
    }

    fn ray_entry(&self, origin: Point3, direction: UnitVec3, max_distance: f64) -> Option<f64> {
        self.0.ray_entry(origin, direction, max_distance)
    }

    fn max_radius(&self) -> Option<f64> {
        None
    }

    fn top_over_disc(&self, centre: Point3, axis: UnitVec3, radius: f64) -> Option<f64> {
        self.0.top_over_disc(centre, axis, radius)
    }
}

/// One pass through `points` at `a`.
fn through(points: &[Point3], a: Degrees) -> Vec<Pass> {
    vec![Pass {
        points: points
            .iter()
            .map(|&tip| CutPoint {
                tip,
                a,
                contact: tip,
            })
            .collect(),
        ..Pass::default()
    }]
}

#[test]
fn req_tl_04_req_nfr_07_stock_of_unknown_reach_between_readings_is_not_missed() {
    // The owner's Codex review of 6580f9b: a stock model that gives its
    // exact top but no farthest radius skipped the stock between readings.
    // Codex's slab (as in the test above: 3 mm tool, 1 mm of flute, a move
    // 0.5 mm along X at A 0, a slab 15 mm tall between its ends) is read
    // between the ends with no radius: a move at one A needs none.
    let mut t = tool(ToolShape::FlatEnd, 3.0);
    t.flute_length = Millimetres(1.0);
    t.overall_length = Millimetres(20.0);
    let f = Fixture::new(bar(), 40.0, &t);
    let a = Degrees(0.0);
    let axis = f.rotary.tool_axis(a).get();
    let across = Vec3::new(0.0, -axis.z, axis.y);
    let at = |along: f64| {
        f.rotary.to_model(
            AxisPosition {
                along,
                across: 0.0,
                height: 0.0,
            },
            a,
        )
    };
    let (start, end) = (at(0.0), at(0.5));
    let corner = |x: f64, w: f64, h: f64| start + Vec3::new(x, 0.0, 0.0) + across * w + axis * h;
    let (p, q) = (corner(0.2, 1.49, 0.0), corner(0.3, 1.50, 15.0));
    let stock = NoRadius(AnalyticStock::Box(Aabb {
        min: Point3::new(p.x.min(q.x), p.y.min(q.y), p.z.min(q.z)),
        max: Point3::new(p.x.max(q.x), p.y.max(q.y), p.z.max(q.z)),
    }));
    let input = openchuck_path::StrategyInput {
        stock: &stock,
        ..f.input()
    };
    assert!(check(&input, &through(&[start], a), None).is_ok());
    assert!(check(&input, &through(&[end], a), None).is_ok());
    let e = check(&input, &through(&[start, end], a), None)
        .unwrap_err()
        .to_string();
    assert!(e.contains("1.000 mm of flute"), "{e}");
    assert!(named_depth(&e) >= 15.0 - 1e-6, "{e}");

    // A turn moves the stock by how far it reaches from the axis, which is
    // not known: the step cannot be bounded, so it is refused as stock of
    // unknown size, never accepted unread.
    let f = fixture(boxed((-10.0, 10.0), (-10.0, 10.0)), 30.0);
    let stock = NoRadius(f.stock);
    let input = openchuck_path::StrategyInput {
        stock: &stock,
        ..f.input()
    };
    let e = check(&input, &turn(&f, BAR), None).unwrap_err().to_string();
    assert!(e.contains("the stock's size is not known"), "{e}");
}

#[test]
fn req_tl_04_req_nfr_07_a_cut_exactly_at_the_flutes_is_read_in_bounded_work() {
    // The owner's Codex review of 6580f9b: round stock of radius 17.5 over X
    // -2 to 102, a 3 mm flat tool with 10 mm of flute, and a cut at A 0 with
    // the tip 7.5 mm above the axis from X 0 to 100: the stock stands
    // exactly 10 mm above the tip all along. On 6580f9b each 0.5 mm step was
    // halved to about 2^18 readings. A move along the tool's side under the
    // top of round stock adds nothing to the step's bound, so it is settled
    // with no halving: accepted, in well under a second. With 9.99 mm of
    // flute it is refused at 10 mm.
    let round = AnalyticStock::Cylinder {
        radius: 17.5,
        x_start: -2.0,
        x_end: 102.0,
    };
    let a = Degrees(0.0);
    let cut = |flutes: f64| {
        let mut t = tool(ToolShape::FlatEnd, 3.0);
        t.flute_length = Millimetres(flutes);
        t.overall_length = Millimetres(flutes + 20.0);
        let mut f = Fixture::new(bar(), 40.0, &t);
        f.stock = round;
        let at = |along: f64| {
            f.rotary.to_model(
                AxisPosition {
                    along,
                    across: 0.0,
                    height: 7.5,
                },
                a,
            )
        };
        let passes = through(&[at(0.0), at(100.0)], a);
        let started = std::time::Instant::now();
        let result = check(&f.input(), &passes, None).map_err(|e| e.to_string());
        let took = started.elapsed().as_secs_f64();
        assert!(took < 1.0, "{took} s");
        result
    };
    cut(10.0).unwrap();
    let e = cut(9.99).unwrap_err();
    assert!((named_depth(&e) - 10.0).abs() < 1e-6, "{e}");
}

#[test]
fn req_ui_09_req_tl_04_a_cancelled_flute_check_stops() {
    // UI-09 and architecture section 5.1 (the owner's Codex review of
    // 6580f9b): the check polls Cancel as it reads the path, so a cancelled
    // job stops with the cancel error, not a refusal or an acceptance.
    use openchuck_path::PathError;
    use openchuck_path::flutes::check_path;
    use openchuck_types::ir::MoveKind;
    use openchuck_types::job::{CancelToken, JobContext, NoProgress};
    let f = fixture(boxed((-10.0, 10.0), (-10.0, 10.0)), 30.0);
    let path = written(&f, MoveKind::Cutting, 5.0, 15.0);
    let cancel = CancelToken::new();
    let context = JobContext::new(&cancel, &NoProgress);
    check_path(&f.input(), &path, None, &context).unwrap();
    cancel.cancel();
    let e = check_path(&f.input(), &path, None, &context).unwrap_err();
    assert!(matches!(e, PathError::Cancelled), "{e:?}");
}

/// A stock model whose answers are not numbers: its top over the disc, or
/// (with no top) its farthest radius.
#[derive(Debug)]
struct NotANumber {
    top: bool,
}

impl StockModel for NotANumber {
    fn contains(&self, _: Point3) -> bool {
        true
    }

    fn ray_entry(&self, _: Point3, _: UnitVec3, _: f64) -> Option<f64> {
        Some(0.0)
    }

    fn max_radius(&self) -> Option<f64> {
        if self.top { Some(20.0) } else { Some(f64::NAN) }
    }

    fn top_over_disc(&self, _: Point3, _: UnitVec3, _: f64) -> Option<f64> {
        self.top.then_some(f64::NAN)
    }
}

#[test]
fn req_nfr_07_req_tl_04_a_place_or_answer_that_is_not_a_number_is_refused() {
    // NFR-07 (the coordinator's review of #534): the check never accepts
    // what it cannot read. A tool position or angle that is not a finite
    // number is refused as the path's; a stock answer that is not a number
    // is refused as stock of unknown size.
    let f = fixture(boxed((-10.0, 10.0), (-10.0, 10.0)), 30.0);
    let at = |along: f64| {
        f.rotary.to_model(
            AxisPosition {
                along,
                across: 0.0,
                height: BAR,
            },
            Degrees(0.0),
        )
    };
    for (tip, a) in [
        (Point3::new(f64::NAN, 0.0, BAR), Degrees(0.0)),
        (Point3::new(5.0, f64::INFINITY, BAR), Degrees(0.0)),
        (at(5.0), Degrees(f64::NAN)),
    ] {
        let passes = vec![Pass {
            points: [at(0.0), tip]
                .iter()
                .zip([Degrees(0.0), a])
                .map(|(&tip, a)| CutPoint {
                    tip,
                    a,
                    contact: tip,
                })
                .collect(),
            ..Pass::default()
        }];
        let e = check(&f.input(), &passes, None).unwrap_err().to_string();
        assert!(e.contains("not a finite number"), "{e}");
    }
    // With 10 mm of flute, so the farthest radius (20 mm) does not settle
    // it first.
    let f = fixture(boxed((-10.0, 10.0), (-10.0, 10.0)), 10.0);
    for top in [true, false] {
        let stock = NotANumber { top };
        let input = openchuck_path::StrategyInput {
            stock: &stock,
            ..f.input()
        };
        let e = check(&input, &through(&[at(0.0), at(5.0)], Degrees(0.0)), None)
            .unwrap_err()
            .to_string();
        assert!(e.contains("the stock's size is not known"), "{top}: {e}");
    }
}

#[test]
fn req_tl_04_a_move_of_no_length_is_read_where_it_stands() {
    // A move that neither travels nor turns reads the stock where it
    // stands, as a single place does: 35 x 35 box stock with its corner up
    // at A 45, the tip 5 mm above the axis, 19.749 mm.
    let f = fixture(boxed((-17.5, 17.5), (-17.5, 17.5)), 30.0);
    let a = Degrees(45.0);
    let tip = f.rotary.to_model(
        AxisPosition {
            along: 10.0,
            across: 0.0,
            height: BAR,
        },
        a,
    );
    let one = deepest_of(&f, &through(&[tip], a));
    let still = deepest_of(&f, &through(&[tip, tip, tip], a));
    assert!(
        (one.depth - 17.5_f64.hypot(17.5) + BAR).abs() < 1e-9,
        "{one:?}"
    );
    assert_eq!(one.depth.to_bits(), still.depth.to_bits(), "{still:?}");
}

#[test]
fn req_tl_04_req_nfr_07_a_move_of_many_turns_is_read_all_round() {
    // A fed move that turns A many times at one place (as assembly may
    // write it, its angles unwrapped) is read all round, even past the
    // 10,000 readings a move is split into: on 35 x 35 box stock with the
    // tip 5 mm above the axis, 10 and 30 turns are both refused at the
    // corner (19.749 mm) with 19.7 mm of flute and accepted with 19.8 mm.
    use openchuck_path::flutes::check_path;
    use openchuck_types::ir::{MoveKind, PathTags, Segment, ToolPose, ToolpathGroup};
    let many = |f: &Fixture, turns: f64| {
        let pose = |a: Degrees| ToolPose {
            tip: f.rotary.to_model(
                AxisPosition {
                    along: 10.0,
                    across: 0.0,
                    height: BAR,
                },
                a,
            ),
            axis: f.rotary.tool_axis(a),
            a,
        };
        openchuck_path::GeneratedPath {
            group: ToolpathGroup {
                rotary_reset_after: Vec::new(),
                tags: PathTags {
                    operation: "op".to_owned(),
                    setup: "setup".to_owned(),
                    tool: "tool".to_owned(),
                },
                start: pose(Degrees(0.0)),
                segments: vec![Segment {
                    kind: MoveKind::Cutting,
                    end: pose(Degrees(360.0 * turns)),
                    contact_feed: Some(openchuck_types::units::FeedRate(300.0)),
                    duration: openchuck_types::units::Seconds(1.0),
                    contact_length: Millimetres(1.0),
                }],
            },
            notes: Vec::new(),
            warnings: Vec::new(),
            unreached: Vec::new(),
            shank_breaks: Vec::new(),
        }
    };
    let square = boxed((-17.5, 17.5), (-17.5, 17.5));
    for turns in [10.0, 30.0] {
        let f = fixture(square, 19.7);
        let e = run(|ctx| check_path(&f.input(), &many(&f, turns), None, ctx))
            .unwrap_err()
            .to_string();
        assert!(
            (named_depth(&e) - (17.5_f64.hypot(17.5) - BAR)).abs() < 1e-3,
            "{turns}: {e}"
        );
        let f = fixture(square, 19.8);
        run(|ctx| check_path(&f.input(), &many(&f, turns), None, ctx)).unwrap();
    }
}

#[test]
fn req_tl_04_req_nfr_07_a_shank_wider_than_the_flutes_is_read_over_its_width() {
    // The tool's body above the tip is as wide as its shank where that is
    // wider than the cutting part: a 3 mm tool with 5 mm of flute and a
    // 6 mm shank moves along X at A 0 beside a strip of stock 2.0 to 2.1 mm
    // across the tool axis and 20 mm tall. The cutting part (1.5 mm) misses
    // it; the shank (3 mm) is over it, so the move is refused at 20 mm.
    let mut t = tool(ToolShape::FlatEnd, 3.0);
    t.flute_length = Millimetres(5.0);
    t.overall_length = Millimetres(40.0);
    let mut f = Fixture::new(bar(), 40.0, &t);
    let a = Degrees(0.0);
    let axis = f.rotary.tool_axis(a).get();
    let across = Vec3::new(0.0, -axis.z, axis.y);
    let at = |along: f64| {
        f.rotary.to_model(
            AxisPosition {
                along,
                across: 0.0,
                height: 0.0,
            },
            a,
        )
    };
    let (start, end) = (at(0.0), at(5.0));
    let (p, q) = (start + across * 2.0, end + across * 2.1 + axis * 20.0);
    f.stock = AnalyticStock::Box(Aabb {
        min: Point3::new(p.x.min(q.x), p.y.min(q.y), p.z.min(q.z)),
        max: Point3::new(p.x.max(q.x), p.y.max(q.y), p.z.max(q.z)),
    });
    let passes = through(&[start, end], a);
    check(&f.input(), &passes, None).unwrap();
    let input = openchuck_path::StrategyInput {
        shank: Some(openchuck_path::strategy::Shank {
            radius: 3.0,
            height: 5.0,
        }),
        ..f.input()
    };
    let e = check(&input, &passes, None).unwrap_err().to_string();
    assert!((named_depth(&e) - 20.0).abs() < 1e-6, "{e}");
}
