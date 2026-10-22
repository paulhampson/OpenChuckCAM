// SPDX-License-Identifier: AGPL-3.0-or-later
//! The strategy interface, parameter schemas, A-axis management, contact
//! feed, linking and determinism, run through a stub strategy that uses only
//! the public API (OP-01, OP-11, OP-13 to OP-16, NFR-06).
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing
)]

mod support;

use std::collections::BTreeMap;
use std::sync::Arc;

use openchuck_path::params::{common_parameters, names};
use openchuck_path::rotary::{
    AxisPosition, fit_range, nearest_turn, reversals, unwind_turns, unwrap_continuous,
};
use openchuck_path::{
    AnalyticStock, CommonParameters, MeshSurface, ParameterValue, PathError, Registry,
    RotaryMapping, StockModel, Strategy, generate,
};
use openchuck_testkit::gouge::{Outline, Pose, coverage, max_gouge};
use openchuck_testkit::oracle::{OracleMachine, model_tip};
use openchuck_testkit::parts::{Cylinder, HelicalGroove, RadialPart};
use openchuck_types::geometry::Point3;
use openchuck_types::ir::{MoveKind, ToolpathGroup};
use openchuck_types::kinematics::{RotaryFrame, machine_xyz};
use openchuck_types::machine::{ARange, ASense};
use openchuck_types::units::{Degrees, Millimetres};
use proptest::prelude::*;
use support::{Fixture, Rings, ball, ring_values, run};

fn cylinder_fixture() -> (Cylinder, Fixture) {
    let part = Cylinder {
        radius: 15.0,
        length: 40.0,
    };
    let surface = MeshSurface::new(&part.mesh(3, 1440)).unwrap();
    (part, Fixture::new(surface, 20.0, &ball(6.0)))
}

fn rings(fixture: &Fixture, values: &BTreeMap<String, ParameterValue>) -> ToolpathGroup {
    run(|ctx| generate(&Rings, &fixture.input(), values, ctx))
        .unwrap()
        .group
}

fn cut_poses(group: &ToolpathGroup) -> Vec<Pose> {
    group
        .segments
        .iter()
        .filter(|s| matches!(s.kind, MoveKind::Cutting | MoveKind::Plunge))
        .map(|s| Pose {
            tip: s.end.tip,
            axis: s.end.axis.get(),
        })
        .collect()
}

#[test]
fn req_op_16_stub_strategy_plugs_in_through_the_registry() {
    let mut registry = Registry::builtin();
    registry.register(Arc::new(Rings)).unwrap();
    let again = registry.register(Arc::new(Rings)).unwrap_err();
    assert!(again.to_string().contains("already registered"), "{again}");
    let found = registry.get("test_rings").unwrap();
    assert_eq!(found.label(), "Rings (test)");
    assert!(matches!(
        registry.get("no_such_strategy"),
        Err(PathError::UnknownStrategy(_))
    ));
    // The library's own strategy is there before anything is registered.
    assert_eq!(
        Registry::builtin()
            .get("helical_finishing")
            .unwrap()
            .label(),
        "Helical finishing"
    );
    // Its schema carries the common parameters first, then its own, each
    // with a unit and a default or a requirement, enough to draw a form.
    let schema = found.schema();
    let names: Vec<&str> = schema.parameters.iter().map(|p| p.name.as_str()).collect();
    let common: Vec<String> = common_parameters().into_iter().map(|p| p.name).collect();
    assert_eq!(
        &names[..common.len()],
        common.iter().map(String::as_str).collect::<Vec<_>>()
    );
    assert!(names.contains(&"angle_step"));
    // And it runs.
    let (_, fixture) = cylinder_fixture();
    let group = rings(&fixture, &ring_values(10.0, 12.0, 1.0, 0.0));
    assert!(group.segments.iter().any(|s| s.kind == MoveKind::Cutting));
}

#[test]
fn req_op_11_common_parameters_resolve_with_defaults_and_refuse_bad_values() {
    let schema = Rings.schema();
    let values = ring_values(0.0, 10.0, 0.5, 0.0);
    let resolved = schema.resolve(&values).unwrap();
    let common = CommonParameters::read(&resolved).unwrap();
    assert_eq!(common.tolerance.get(), Millimetres(0.01));
    assert_eq!(common.stock_to_leave, Millimetres(0.0));
    assert_eq!(common.stepover, Millimetres(0.5));
    assert_eq!(common.feed.0, 300.0);
    // A whole number reads as a number.
    let mut v = values.clone();
    v.insert(names::TOLERANCE.into(), ParameterValue::Integer(1));
    assert_eq!(
        schema
            .resolve(&v)
            .unwrap()
            .number(names::TOLERANCE)
            .unwrap(),
        1.0
    );
    let cases: Vec<(&str, ParameterValue, &str)> = vec![
        (
            names::TOLERANCE,
            ParameterValue::Number(0.0),
            "greater than 0",
        ),
        (
            names::STOCK_TO_LEAVE,
            ParameterValue::Number(-0.1),
            "at least 0",
        ),
        (
            names::FEED,
            ParameterValue::Text("fast".into()),
            "expected a number",
        ),
        (
            names::RETRACT,
            ParameterValue::Text("up".into()),
            "not one of",
        ),
        (
            "stepover_typo",
            ParameterValue::Number(1.0),
            "no parameter of that name",
        ),
        ("angle_step", ParameterValue::Number(f64::NAN), "finite"),
    ];
    for (name, value, expect) in cases {
        let mut v = values.clone();
        v.insert(name.into(), value);
        let e = schema.resolve(&v).unwrap_err().to_string();
        assert!(e.contains(name) && e.contains(expect), "{name}: {e}");
    }
    let mut v = values.clone();
    v.remove(names::FEED);
    let e = schema.resolve(&v).unwrap_err().to_string();
    assert!(e.contains("feed") && e.contains("required"), "{e}");
}

#[test]
fn req_op_15_angles_stay_continuous_and_fit_the_range() {
    assert_eq!(nearest_turn(Degrees(10.0), Degrees(700.0)), Degrees(730.0));
    assert_eq!(nearest_turn(Degrees(350.0), Degrees(0.0)), Degrees(-10.0));
    // Through the 0/360 seam the path keeps going.
    let raw: Vec<Degrees> = [350.0, 355.0, 0.0, 5.0, 10.0].map(Degrees).to_vec();
    let un = unwrap_continuous(&raw, Degrees(350.0));
    assert_eq!(
        un,
        [350.0, 355.0, 360.0, 365.0, 370.0].map(Degrees).to_vec()
    );
    assert_eq!(reversals(&un, 1e-9), 0);
    let back: Vec<Degrees> = [0.0, 10.0, 20.0, 15.0, 5.0, 8.0].map(Degrees).to_vec();
    assert_eq!(reversals(&back, 1e-9), 2);
    assert_eq!(reversals(&back, 6.0), 1);
    // A limited range shifts a pass by whole turns, preferring the turn
    // nearest the previous angle, and refuses a pass wider than the range.
    let range = ARange::Limited {
        min: Degrees(-90.0),
        max: Degrees(400.0),
    };
    let pass: Vec<Degrees> = [700.0, 720.0, 750.0].map(Degrees).to_vec();
    assert_eq!(fit_range(&pass, range, Degrees(0.0), "p").unwrap(), -720.0);
    assert_eq!(
        fit_range(&pass, range, Degrees(360.0), "p").unwrap(),
        -360.0
    );
    let wide: Vec<Degrees> = [0.0, 300.0, 600.0].map(Degrees).to_vec();
    let e = fit_range(&wide, range, Degrees(0.0), "pass 3")
        .unwrap_err()
        .to_string();
    assert!(e.contains("pass 3") && e.contains("range"), "{e}");
    let narrow = ARange::Limited {
        min: Degrees(10.0),
        max: Degrees(20.0),
    };
    assert!(fit_range(&[Degrees(0.0), Degrees(5.0)], narrow, Degrees(0.0), "p").is_err());
    // Unwinding returns A to (-180, 180].
    for (a, back) in [
        (725.0, 5.0),
        (-725.0, -5.0),
        (180.0, 180.0),
        (-180.0, 180.0),
        (540.0, 180.0),
    ] {
        assert!((a + unwind_turns(Degrees(a)) - back).abs() < 1e-9, "{a}");
    }
}

#[test]
fn req_op_15_generated_rings_keep_a_continuous_and_unwind_at_the_end() {
    let (_, fixture) = cylinder_fixture();
    let group = rings(&fixture, &ring_values(5.0, 8.0, 1.0, 0.0));
    let mut previous = group.start.a;
    let mut cut = Vec::new();
    for s in &group.segments {
        if s.kind == MoveKind::Cutting {
            assert!((s.end.a.0 - previous.0).abs() <= 180.0);
            cut.push(s.end.a);
        }
        assert!(s.end.a.0.is_finite() && s.end.tip.is_finite());
        previous = s.end.a;
    }
    // Four rings of a full turn each, all in the same direction.
    let span = cut.last().unwrap().0 - cut.first().unwrap().0;
    assert!(span > 4.0 * 360.0 - 10.0, "span {span}");
    assert_eq!(reversals(&cut, 1e-9), 0);
    let last = group.segments.last().unwrap();
    assert!(last.end.a.0 > -180.0 && last.end.a.0 <= 180.0);
}

#[test]
fn req_op_15_ir_poses_round_trip_through_the_independent_oracle() {
    let (_, fixture) = cylinder_fixture();
    for sense in [ASense::Positive, ASense::Negative] {
        let mut fixture = Fixture {
            ..fixture_clone(&fixture)
        };
        fixture.machine.a_sense = sense;
        fixture.rotary = RotaryMapping::of_profile(&fixture.machine);
        let group = rings(&fixture, &ring_values(5.0, 7.0, 1.0, 2.5));
        let frame = RotaryFrame::of_profile(&fixture.machine);
        let oracle = OracleMachine {
            origin_x: frame.origin.x,
            origin_y: frame.origin.y,
            origin_z: frame.origin.z,
            sense: sense.sign(),
        };
        for s in &group.segments {
            let m = machine_xyz(&frame, s.end.tip, s.end.a);
            let back = model_tip(&oracle, m.x, m.y, m.z, s.end.a.0);
            let tip = [s.end.tip.x, s.end.tip.y, s.end.tip.z];
            assert!(openchuck_testkit::oracle::distance(back, tip) < 1e-6);
            // The tool axis is machine +Z at that A.
            let up = fixture.rotary.tool_axis(s.end.a).get();
            assert!((up - s.end.axis.get()).length() < 1e-12);
        }
    }
}

fn fixture_clone(f: &Fixture) -> Fixture {
    let part = Cylinder {
        radius: 15.0,
        length: 40.0,
    };
    let mut copy = Fixture::new(
        MeshSurface::new(&part.mesh(3, 1440)).unwrap(),
        20.0,
        &ball(6.0),
    );
    copy.machine = f.machine.clone();
    copy
}

/// The point of the exact surface of `part` nearest `centre`, by a pattern
/// search over (x, angle) from the point straight below `centre`.
fn nearest_on<P: RadialPart>(part: &P, centre: Point3) -> Point3 {
    let at = |x: f64, t: f64| {
        let r = part.radius_at(x, t);
        Point3::new(x, r * t.cos(), r * t.sin())
    };
    let (mut x, mut t) = (centre.x, centre.z.atan2(centre.y));
    let mut best = at(x, t).distance(centre);
    let (mut sx, mut st) = (0.5, 0.05);
    while sx > 1e-10 {
        let mut moved = false;
        for (dx, dt) in [(sx, 0.0), (-sx, 0.0), (0.0, st), (0.0, -st)] {
            let d = at(x + dx, t + dt).distance(centre);
            if d < best {
                best = d;
                x += dx;
                t += dt;
                moved = true;
            }
        }
        if !moved {
            sx *= 0.5;
            st *= 0.5;
        }
    }
    at(x, t)
}

#[test]
fn req_op_13_contact_feed_is_correct_on_simultaneous_moves() {
    // A helical groove makes Z and A move together; the tool sits off the
    // axis so the contact is off the top.
    // The contact found on a mesh shifts with the facets' orientation
    // (decision record 0015), which shows in the contact length even where
    // the mesh's own error is tiny; this band of the part is meshed at 0.025
    // mm along X and 0.25 degrees around (decision record 0018, OI-77).
    let part = HelicalGroove {
        radius: 15.0,
        length: 6.0,
        pitch: 20.0,
        depth: 2.0,
    };
    let surface = MeshSurface::new(&part.mesh(241, 1440)).unwrap();
    let fixture = Fixture::new(surface, 20.0, &ball(6.0));
    let rho = 3.0;
    let mut errors = Vec::new();
    let mut values = ring_values(3.0, 3.0, 1.0, 3.0);
    values.insert("angle_step".into(), ParameterValue::Number(1.0));
    let group = rings(&fixture, &values);
    let feed = 300.0;
    let mut checked = 0;
    let mut worst = 0.0_f64;
    let mut previous = group.start;
    for s in &group.segments {
        if s.kind == MoveKind::Cutting {
            assert_eq!(s.contact_feed.unwrap().0, feed);
            // The IR's own figures: contact length over duration.
            let programmed = 60.0 * s.contact_length.0 / s.duration.0;
            assert!((programmed - feed).abs() < 1e-9 * feed);
            // The machine moves linearly in X, Y, Z and A between the two
            // ends. Follow it in 20 steps, find the real contact of the ball
            // on the exact surface (the surface point nearest the ball's
            // centre) and measure the path it traces in the move's time.
            let p = fixture.rotary.to_axis(previous.tip, previous.a);
            let q = fixture.rotary.to_axis(s.end.tip, s.end.a);
            let mut length = 0.0;
            let mut last: Option<Point3> = None;
            for k in 0..=20 {
                let t = f64::from(k) / 20.0;
                let at = AxisPosition {
                    along: p.along + t * (q.along - p.along),
                    across: p.across + t * (q.across - p.across),
                    height: p.height + t * (q.height - p.height),
                };
                let a = Degrees(previous.a.0 + t * (s.end.a.0 - previous.a.0));
                let tip = fixture.rotary.to_model(at, a);
                let centre = tip + fixture.rotary.tool_axis(a).get() * rho;
                let contact = nearest_on(&part, centre);
                if let Some(c) = last {
                    length += c.distance(contact);
                }
                last = Some(contact);
            }
            let effective = 60.0 * length / s.duration.0;
            worst = worst.max((effective - feed).abs() / feed);
            errors.push((effective - feed).abs() / feed);
            checked += 1;
        }
        previous = s.end;
    }
    errors.sort_by(f64::total_cmp);
    eprintln!(
        "effective feed error: median {:.4}, 95th percentile {:.4}, worst {:.4}",
        errors[errors.len() / 2],
        errors[errors.len() * 95 / 100],
        worst
    );
    assert!(checked > 300, "{checked}");
    assert!(
        worst <= 0.02,
        "worst effective feed error {:.2}%",
        100.0 * worst
    );
}

#[test]
fn req_op_13_machine_feed_limits_lengthen_moves_and_say_so() {
    let (_, mut fixture) = cylinder_fixture();
    fixture.machine.max_feeds.rotary.0 = 100.0;
    let out = run(|ctx| {
        generate(
            &Rings,
            &fixture.input(),
            &ring_values(5.0, 5.0, 1.0, 0.0),
            ctx,
        )
    })
    .unwrap();
    assert!(!out.notes.is_empty());
    assert!(
        out.notes[0].message.contains("feed limits"),
        "{:?}",
        out.notes[0]
    );
    for s in out
        .group
        .segments
        .iter()
        .filter(|s| s.kind == MoveKind::Cutting)
    {
        let da = (s.end.a.0 - 0.0).abs();
        assert!(da.is_finite());
        // 2 degrees at 100 deg/min takes at least 1.2 s.
        assert!(s.duration.0 >= 1.2 - 1e-9, "{}", s.duration.0);
    }
}

#[test]
fn req_op_14_links_stay_outside_the_clearance_and_never_cross_the_stock() {
    let (_, fixture) = cylinder_fixture();
    for retract in ["clearance", "safe_z"] {
        let mut values = ring_values(5.0, 10.0, 1.0, 2.0);
        values.insert(names::RETRACT.into(), ParameterValue::Text(retract.into()));
        let group = rings(&fixture, &values);
        let mut from = group.start;
        let mut links = 0;
        for s in &group.segments {
            if s.kind == MoveKind::Rapid {
                links += 1;
                // The machine moves in a straight line in X, Y, Z and A.
                let p = fixture.rotary.to_axis(from.tip, from.a);
                let q = fixture.rotary.to_axis(s.end.tip, s.end.a);
                for k in 0..=50 {
                    let t = f64::from(k) / 50.0;
                    let at = AxisPosition {
                        along: p.along + t * (q.along - p.along),
                        across: p.across + t * (q.across - p.across),
                        height: p.height + t * (q.height - p.height),
                    };
                    let a = Degrees(from.a.0 + t * (s.end.a.0 - from.a.0));
                    let tip = fixture.rotary.to_model(at, a);
                    assert!(
                        !fixture.stock.contains(tip),
                        "{retract}: rapid enters the stock at {tip:?}"
                    );
                    if (p.along - q.along).abs() > 1e-9 || (s.end.a.0 - from.a.0).abs() > 1e-9 {
                        // Moves along or around the axis stay outside the safe cylinder.
                        assert!(tip.y.hypot(tip.z) >= fixture.clearance.radius.0 - 1e-9);
                    }
                }
            }
            from = s.end;
        }
        assert!(links >= 10, "{retract}: {links}");
        // Each plunge starts the approach distance above the stock.
        let mut from = group.start;
        for s in &group.segments {
            if s.kind == MoveKind::Plunge {
                let start_height = fixture.rotary.to_axis(from.tip, from.a).height;
                let entry = (20.0f64.powi(2) - 2.0f64.powi(2)).sqrt();
                assert!(
                    (start_height - (entry + 1.0)).abs() < 1e-9,
                    "{start_height}"
                );
                assert_eq!(s.contact_feed.unwrap().0, 100.0);
            }
            from = s.end;
        }
    }
}

#[test]
fn req_op_14_stock_beyond_the_retract_height_is_refused() {
    let (_, mut fixture) = cylinder_fixture();
    fixture.stock = AnalyticStock::Cylinder {
        radius: 30.0,
        x_start: 0.0,
        x_end: 40.0,
    };
    let e = run(|ctx| {
        generate(
            &Rings,
            &fixture.input(),
            &ring_values(5.0, 6.0, 1.0, 0.0),
            ctx,
        )
    })
    .unwrap_err()
    .to_string();
    assert!(e.contains("clearance") && e.contains("OP-14"), "{e}");
}

#[test]
fn req_op_12_generated_rings_pass_the_gouge_and_coverage_checks() {
    let (part, fixture) = cylinder_fixture();
    let mut values = ring_values(10.0, 14.0, 0.5, 0.0);
    values.insert("angle_step".into(), ParameterValue::Number(1.0));
    let group = rings(&fixture, &values);
    let poses = cut_poses(&group);
    let outline = Outline::new(&fixture.tool);
    let g = max_gouge(&outline, &part, &poses, 0.1);
    assert!(g.depth <= 0.001, "gouge {}", g.depth);
    // Scallop of a 3 mm ball at 0.5 mm stepover is 0.0105 mm.
    let c = coverage(&outline, &part, &poses, (11.0, 13.0), 0.1, 0.02);
    assert_eq!(c.uncovered, 0, "{c:?}");
    // Seeded faults: a missing ring leaves material, a lowered pass gouges.
    let missing: Vec<Pose> = poses
        .iter()
        .filter(|p| (p.tip.x - 12.0).abs() > 1e-6)
        .copied()
        .collect();
    let c = coverage(&outline, &part, &missing, (11.0, 13.0), 0.1, 0.02);
    assert!(c.uncovered > 0, "{c:?}");
    let lowered: Vec<Pose> = poses
        .iter()
        .map(|p| Pose {
            tip: p.tip + p.axis * -0.03,
            axis: p.axis,
        })
        .collect();
    assert!(max_gouge(&outline, &part, &lowered, 0.1).depth > 0.02);
}

#[test]
fn req_op_01_regeneration_is_identical_at_1_2_and_n_threads() {
    let (_, fixture) = cylinder_fixture();
    let values = ring_values(5.0, 15.0, 1.0, 1.5);
    let runs = openchuck_testkit::determinism::check(|| {
        serde_json::to_vec(&rings(&fixture, &values)).unwrap()
    })
    .unwrap();
    assert_eq!(runs.len(), 4);
}

#[test]
fn req_op_01_cancellation_stops_generation() {
    use openchuck_types::job::{CancelToken, JobContext, NoProgress};
    let (_, fixture) = cylinder_fixture();
    let cancel = CancelToken::new();
    cancel.cancel();
    let ctx = JobContext::new(&cancel, &NoProgress);
    let e = generate(
        &Rings,
        &fixture.input(),
        &ring_values(5.0, 6.0, 1.0, 0.0),
        &ctx,
    )
    .unwrap_err();
    assert_eq!(e, PathError::Cancelled);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn req_op_15_no_nan_and_no_jump_over_half_a_turn(raw in prop::collection::vec(-1e5f64..1e5, 1..50), start in -1e4f64..1e4) {
        let angles: Vec<Degrees> = raw.into_iter().map(Degrees).collect();
        let un = unwrap_continuous(&angles, Degrees(start));
        let mut previous = start;
        for (a, u) in angles.iter().zip(&un) {
            prop_assert!(u.0.is_finite());
            prop_assert!((u.0 - previous).abs() <= 180.0 + 1e-6);
            let turns = (u.0 - a.0) / 360.0;
            prop_assert!((turns - turns.round()).abs() < 1e-6);
            previous = u.0;
        }
        let back = un.last().unwrap().0 + unwind_turns(*un.last().unwrap());
        prop_assert!(back > -180.0 - 1e-9 && back <= 180.0 + 1e-9);
    }

    #[test]
    fn req_nfr_07_any_parameter_values_give_a_result_or_an_error(
        tol in prop::num::f64::ANY, step in prop::num::f64::ANY, feed in prop::num::f64::ANY
    ) {
        let mut v = ring_values(0.0, 1.0, 0.5, 0.0);
        v.insert(names::TOLERANCE.into(), ParameterValue::Number(tol));
        v.insert("angle_step".into(), ParameterValue::Number(step));
        v.insert(names::FEED.into(), ParameterValue::Number(feed));
        let _ = Rings.schema().resolve(&v);
    }
}

#[test]
fn req_op_15_stock_model_answers_rays() {
    let s = AnalyticStock::Cylinder {
        radius: 10.0,
        x_start: 0.0,
        x_end: 20.0,
    };
    let down = openchuck_types::geometry::UnitVec3::new(openchuck_types::geometry::Vec3::new(
        0.0, 0.0, -1.0,
    ))
    .unwrap();
    assert_eq!(
        s.ray_entry(Point3::new(5.0, 0.0, 30.0), down, 100.0),
        Some(20.0)
    );
    assert_eq!(s.ray_entry(Point3::new(25.0, 0.0, 30.0), down, 100.0), None);
    assert_eq!(s.ray_entry(Point3::new(5.0, 0.0, 30.0), down, 10.0), None);
    assert_eq!(
        s.ray_entry(Point3::new(5.0, 0.0, 0.0), down, 10.0),
        Some(0.0)
    );
    assert_eq!(s.max_radius(), Some(10.0));
}
