// SPDX-License-Identifier: AGPL-3.0-or-later
//! The per-operation Exact STEP surface contact (decision 0500; issue #351;
//! requirement OP-27, IMP-02, OP-12, GC-07, NFR-06, NFR-07): which strategies offer
//! it, its default for each, which tools it applies to, and that a surface
//! asked for exact drops drops every tool that has one on the exact faces.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing
)]

use std::collections::BTreeMap;

use openchuck_path::contact::{self, ExactContact, parameter::EXACT_CONTACT};
use openchuck_path::params::names;
use openchuck_path::{
    BrepSurface, ParameterKind, ParameterValue, Registry, SurfaceSource, ToolLine,
};
use openchuck_types::geometry::{Point3, UnitVec3, Vec3};
use openchuck_types::tool::profile::{ProfilePoint, ProfileSegment, RevolvedProfile, ToolProfile};

fn profile(segments: Vec<ProfileSegment>) -> RevolvedProfile {
    ToolProfile { segments }.validate().unwrap()
}

fn pt(r: f64, z: f64) -> ProfilePoint {
    ProfilePoint::new(r, z)
}

fn ball(r: f64) -> RevolvedProfile {
    profile(vec![
        ProfileSegment::Arc {
            to: pt(r, r),
            centre: pt(0.0, r),
        },
        ProfileSegment::Line { to: pt(r, 4.0 * r) },
    ])
}

fn flat(r: f64) -> RevolvedProfile {
    profile(vec![
        ProfileSegment::Line { to: pt(r, 0.0) },
        ProfileSegment::Line { to: pt(r, 4.0 * r) },
    ])
}

/// The owner's axial finishing tool: 3.175 mm, a 0.25 mm tip, 5 degrees.
fn tapered() -> RevolvedProfile {
    let (rt, a, r) = (0.25_f64, 5.0_f64.to_radians(), 1.5875);
    let (tr, tz) = (rt * a.cos(), rt * (1.0 - a.sin()));
    profile(vec![
        ProfileSegment::Arc {
            to: pt(tr, tz),
            centre: pt(0.0, rt),
        },
        ProfileSegment::Line {
            to: pt(r, tz + (r - tr) / a.tan()),
        },
    ])
}

fn bull_nose() -> RevolvedProfile {
    profile(vec![
        ProfileSegment::Line { to: pt(0.7, 0.0) },
        ProfileSegment::Arc {
            to: pt(1.0, 0.3),
            centre: pt(0.7, 0.3),
        },
        ProfileSegment::Line { to: pt(1.0, 4.0) },
    ])
}

const BALL: [&str; 4] = [
    "helical_finishing",
    "indexed_finishing",
    "rest_machining",
    "simultaneous_finishing",
];
const OUTLINE: [&str; 2] = ["axial_finishing", "pencil_finishing"];

#[test]
fn req_op_27_exact_contact_is_offered_by_the_strategies_that_drop_on_faces() {
    // Decision 0500: helical, indexed, rest and simultaneous finishing take
    // the exact ball drop (decision 0277), on by default; axial finishing and
    // pencil clean-up take the outline drop (decision 0338), off by default.
    // Every other strategy has no such setting and keeps the tessellation.
    let registry = Registry::builtin();
    for strategy in registry.iter() {
        let name = strategy.name();
        let schema = strategy.schema();
        let spec = schema.get(EXACT_CONTACT);
        let expected = if BALL.contains(&name) {
            Some((ExactContact::Ball, true))
        } else if OUTLINE.contains(&name) {
            Some((ExactContact::Outline, false))
        } else {
            None
        };
        assert_eq!(
            strategy.exact_contact().map(|k| (k, k.default_on())),
            expected,
            "{name}"
        );
        assert_eq!(
            contact::BALL_STRATEGIES.contains(&name),
            BALL.contains(&name),
            "{name}"
        );
        assert_eq!(
            contact::OUTLINE_STRATEGIES.contains(&name),
            OUTLINE.contains(&name),
            "{name}"
        );
        match expected {
            None => assert!(spec.is_none(), "{name} offers the setting"),
            Some((_, on)) => {
                let spec = spec.unwrap_or_else(|| panic!("{name} has no setting"));
                assert_eq!(spec.label, "Exact STEP surface contact", "{name}");
                assert_eq!(spec.kind, ParameterKind::Flag { default: on }, "{name}");
                // Plain words: slower, more accurate, the program is still
                // the user's to check (NFR-12); no promised figure.
                let d = spec.description.to_lowercase();
                for word in ["slower", "accurate", "tessellation", "check the program"] {
                    assert!(d.contains(word), "{name}: '{word}' in {d}");
                }
                // Next to Tolerance, in the same section of the form.
                let order: Vec<&str> = schema.parameters.iter().map(|p| p.name.as_str()).collect();
                let at = order.iter().position(|n| *n == names::TOLERANCE).unwrap();
                assert_eq!(order.get(at + 1), Some(&EXACT_CONTACT), "{name}: {order:?}");
                // The active schema keeps it whatever the other values.
                assert!(
                    strategy
                        .schema_for(&BTreeMap::new())
                        .get(EXACT_CONTACT)
                        .is_some(),
                    "{name}"
                );
            }
        }
    }
}

#[test]
fn req_op_27_exact_contact_applies_to_the_tools_each_kind_supports() {
    // The ball drop of decision 0277 for a ball only; the outline drop of
    // decision 0338 for every tool with an exact drop (a ball, a tapered
    // ball, a flat end). A bull nose has none (OI-1350): it stays on the
    // tessellation whatever the setting.
    assert!(ExactContact::Ball.supports(&ball(1.0)));
    assert!(!ExactContact::Ball.supports(&tapered()));
    assert!(!ExactContact::Ball.supports(&flat(1.0)));
    for p in [ball(1.0), tapered(), flat(1.0)] {
        assert!(ExactContact::Outline.supports(&p));
    }
    assert!(!ExactContact::Ball.supports(&bull_nose()));
    assert!(!ExactContact::Outline.supports(&bull_nose()));
}

#[test]
fn req_op_27_req_gc_07_exact_contact_is_read_from_the_values_or_the_default() {
    let registry = Registry::builtin();
    let helical = registry.get("helical_finishing").unwrap();
    let pencil = registry.get("pencil_finishing").unwrap();
    let roughing = registry.get("rotary_roughing").unwrap();
    let none = BTreeMap::new();
    let set = |on: bool| BTreeMap::from([(EXACT_CONTACT.to_owned(), ParameterValue::Flag(on))]);
    // Unset: the strategy's default, so an operation without the value
    // machines as it did before the setting existed.
    assert!(contact::is_on(helical, &none));
    assert!(!contact::is_on(pencil, &none));
    assert!(!contact::is_on(helical, &set(false)));
    assert!(contact::is_on(pencil, &set(true)));
    // A strategy without the setting never takes exact contacts, whatever
    // is stored.
    assert!(!contact::is_on(roughing, &set(true)));
    // The drop is exact only for a tool the strategy's kind supports.
    assert!(contact::exact_drops(helical, &none, &ball(1.0)));
    assert!(!contact::exact_drops(helical, &none, &tapered()));
    assert!(!contact::exact_drops(helical, &set(false), &ball(1.0)));
    assert!(!contact::exact_drops(pencil, &none, &flat(1.0)));
    assert!(contact::exact_drops(pencil, &set(true), &flat(1.0)));
    assert!(contact::exact_drops(pencil, &set(true), &tapered()));
    assert!(!contact::exact_drops(pencil, &set(true), &bull_nose()));
    assert!(!contact::exact_drops(roughing, &set(true), &ball(1.0)));
}

#[test]
fn req_op_27_exact_drops_drop_every_tool_with_an_exact_drop_on_the_faces() {
    // With exact drops asked for, `drop_tool` answers with the exact drop
    // for every tool that has one (decision 0500 adopts decision 0338's
    // outline drop behind the setting); a tool without one, and a surface
    // not asked, drop on the tessellation as before.
    let model = openchuck_testkit::brep::banded_cylinder(10.0, 30.0, &[10.0, 20.0]);
    let exact = BrepSurface::new(&model).unwrap().with_exact_drops(true);
    let mesh = BrepSurface::new(&model).unwrap();
    let mut differed = 0;
    for k in 0..24 {
        let a = f64::from(k) * 15.0_f64.to_radians() + 0.13;
        let line = ToolLine {
            origin: Point3::new(5.0 + 0.9 * f64::from(k), 0.0, 0.0),
            axis: UnitVec3::new(Vec3::new(0.0, a.sin(), a.cos())).unwrap(),
        };
        for p in [ball(1.0), tapered(), flat(1.0)] {
            let want = exact.drop_exact(&p, &line).map(|d| d.contact);
            assert!(want.is_some());
            assert_eq!(exact.drop_tool(&p, &line), want);
            if mesh.drop_tool(&p, &line) != want {
                differed += 1;
            }
        }
        assert_eq!(
            exact.drop_tool(&bull_nose(), &line),
            mesh.drop_tool(&bull_nose(), &line)
        );
    }
    // The tessellation's answers are not the exact ones: the test sees the
    // difference it is about.
    assert!(differed > 0);
}
