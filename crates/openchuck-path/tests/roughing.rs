// SPDX-License-Identifier: AGPL-3.0-or-later
//! Rotary roughing (OP-03, OP-11, OP-14; work breakdown slice 14; decision
//! record 0033).
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing
)]

mod support;

use std::collections::BTreeMap;

use openchuck_path::params::names;
use openchuck_path::roughing::{NAME, parameter};
use openchuck_path::{GeneratedPath, MeshSurface, ParameterValue, RotaryRoughing, generate};
use openchuck_testkit::gouge::{Outline, Pose, max_gouge, min_clearance};
use openchuck_testkit::parts::{Cone, Cylinder, RadialPart};
use openchuck_types::ir::MoveKind;
use openchuck_types::tool::{Tool, ToolShape};
use support::{Fixture, run, tool};

fn num(x: f64) -> ParameterValue {
    ParameterValue::Number(x)
}

fn text(s: &str) -> ParameterValue {
    ParameterValue::Text(s.to_owned())
}

fn values(extra: &[(&str, ParameterValue)]) -> BTreeMap<String, ParameterValue> {
    let mut v: BTreeMap<String, ParameterValue> = [
        (names::STEPOVER, 3.0),
        (names::FEED, 600.0),
        (names::PLUNGE_FEED, 200.0),
        (names::STOCK_TO_LEAVE, 0.5),
        (parameter::STEPDOWN, 2.0),
    ]
    .into_iter()
    .map(|(k, x)| (k.to_owned(), num(x)))
    .collect();
    for (k, x) in extra {
        v.insert((*k).to_owned(), x.clone());
    }
    v
}

fn fixture<P: RadialPart + ?Sized>(part: &P, stock_radius: f64, t: &Tool) -> Fixture {
    let surface = MeshSurface::new(&part.mesh(40, 360)).unwrap();
    Fixture::new(surface, stock_radius, t)
}

fn rough(f: &Fixture, v: &BTreeMap<String, ParameterValue>) -> GeneratedPath {
    run(|ctx| generate(&RotaryRoughing, &f.input(), v, ctx)).unwrap()
}

fn flat(d: f64) -> Tool {
    tool(ToolShape::FlatEnd, d)
}

fn poses(path: &GeneratedPath) -> Vec<Pose> {
    path.group
        .segments
        .iter()
        .filter(|s| {
            matches!(
                s.kind,
                MoveKind::Cutting | MoveKind::Plunge | MoveKind::LeadIn
            )
        })
        .map(|s| Pose {
            tip: s.end.tip,
            axis: s.end.axis.get(),
        })
        .collect()
}

fn cone() -> Cone {
    Cone {
        start_radius: 8.0,
        end_radius: 12.0,
        length: 30.0,
    }
}

#[test]
fn req_op_11_req_op_16_the_roughing_schema_lists_every_parameter_with_unit_range_and_default() {
    use openchuck_path::{ParameterKind, Strategy, Unit};
    let schema = RotaryRoughing.schema();
    assert_eq!(RotaryRoughing.name(), NAME);
    for name in [
        names::TOLERANCE,
        names::STEPOVER,
        names::STOCK_TO_LEAVE,
        names::FEED,
        names::PLUNGE_FEED,
        names::RETRACT,
        names::APPROACH_DISTANCE,
        names::ENTRY,
        names::RAMP_ANGLE,
        names::HELIX_DIAMETER,
        names::LEAD_RADIUS,
        parameter::FORM,
        parameter::STEPDOWN,
        parameter::INDEX_ANGLE,
        parameter::START_ANGLE,
        parameter::MAX_ANGLE_STEP,
        parameter::MIN_CUT,
    ] {
        let spec = schema.get(name).unwrap_or_else(|| panic!("no {name}"));
        assert!(
            !spec.label.is_empty() && !spec.description.is_empty(),
            "{name}"
        );
    }
    let Some(ParameterKind::Number {
        unit,
        min,
        min_exclusive,
        default,
        ..
    }) = schema.get(parameter::STEPDOWN).map(|p| p.kind.clone())
    else {
        panic!("stepdown is a number");
    };
    assert_eq!(unit, Unit::Millimetres);
    assert_eq!((min, min_exclusive, default), (Some(0.0), true, None));
    // The registry offers it beside helical finishing.
    assert!(openchuck_path::Registry::builtin().get(NAME).is_ok());
}

#[test]
fn req_op_11_a_bad_stepdown_or_form_is_refused_by_name() {
    let f = fixture(
        &Cylinder {
            radius: 10.0,
            length: 20.0,
        },
        15.0,
        &flat(6.0),
    );
    for (name, value) in [
        (parameter::STEPDOWN, num(0.0)),
        (parameter::STEPDOWN, num(-1.0)),
        (parameter::FORM, text("spiral")),
        (parameter::INDEX_ANGLE, num(0.0)),
        (parameter::MAX_ANGLE_STEP, num(120.0)),
    ] {
        let err = run(|ctx| generate(&RotaryRoughing, &f.input(), &values(&[(name, value)]), ctx))
            .unwrap_err()
            .to_string();
        assert!(err.contains(name), "{name}: {err}");
    }
    // The stepdown has no default: it comes from the tool's cutting data.
    let mut v = values(&[]);
    v.remove(parameter::STEPDOWN);
    let err = run(|ctx| generate(&RotaryRoughing, &f.input(), &v, ctx))
        .unwrap_err()
        .to_string();
    assert!(err.contains("stepdown"), "{err}");
}

/// The distinct heights (from the rotary axis, along the tool axis) the tool
/// cuts at on a cylinder part, highest first.
fn levels(f: &Fixture, path: &GeneratedPath) -> Vec<f64> {
    let mut heights: Vec<f64> = path
        .group
        .segments
        .iter()
        .filter(|s| s.kind == MoveKind::Cutting)
        .map(|s| (f.rotary.to_axis(s.end.tip, s.end.a).height * 100.0).round() / 100.0)
        .collect();
    heights.sort_by(|a, b| b.total_cmp(a));
    heights.dedup();
    heights
}

#[test]
fn req_op_11_layers_are_equal_and_no_deeper_than_the_stepdown() {
    let f = fixture(
        &Cylinder {
            radius: 10.0,
            length: 20.0,
        },
        15.0,
        &flat(6.0),
    );
    for stepdown in [1.0, 2.0, 4.0] {
        let path = run(|ctx| {
            generate(
                &RotaryRoughing,
                &f.input(),
                &values(&[(parameter::STEPDOWN, num(stepdown))]),
                ctx,
            )
        })
        .unwrap();
        // The flat layers above the part's floor (10.5, with the stock to
        // leave); below it the tool follows the part.
        let heights: Vec<f64> = levels(&f, &path)
            .into_iter()
            .filter(|h| *h >= 10.49)
            .collect();
        assert!(heights.len() >= 2, "{heights:?}");
        // From the stock's radius to the floor (the part with its stock to
        // leave, less the little the flat tool's rounded offset corner
        // lets it dip near the ends) in equal steps.
        let steps: Vec<f64> = heights.windows(2).map(|w| w[0] - w[1]).collect();
        for step in &steps {
            assert!(*step <= stepdown + 0.011, "{stepdown}: {steps:?}");
        }
        let layers = (15.0_f64 - 10.03) / stepdown;
        assert!(
            layers.ceil() as usize + 1 >= heights.len(),
            "{stepdown}: {heights:?}"
        );
    }
}

#[test]
fn req_op_12_roughing_never_gouges_the_part_with_its_stock_to_leave() {
    for (name, part, tool_) in [
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
        let f = fixture(part.as_ref(), 15.0, &tool_);
        for form in ["continuous", "indexed"] {
            let v = values(&[
                (parameter::FORM, text(form)),
                (names::STEPOVER, num(1.5)),
                (parameter::STEPDOWN, num(3.0)),
            ]);
            let path = run(|ctx| generate(&RotaryRoughing, &f.input(), &v, ctx)).unwrap();
            let outline = Outline::new(&f.tool);
            let g = max_gouge(&outline, part.as_ref(), &poses(&path), 0.25);
            assert!(g.depth < 1e-3, "{name} {form}: {}", g.depth);
            for p in poses(&path).iter().step_by(37) {
                let clearance = min_clearance(&outline, part.as_ref(), p, 0.25);
                assert!(
                    clearance > 0.5 - 0.02,
                    "{name} {form}: clearance {clearance}"
                );
            }
        }
    }
}

#[test]
fn req_nfr_06_roughing_is_identical_at_1_2_and_n_threads() {
    let f = fixture(&cone(), 15.0, &tool(ToolShape::BallEnd, 4.0));
    let v = values(&[(names::STEPOVER, num(2.0)), (parameter::STEPDOWN, num(3.0))]);
    let mut results = Vec::new();
    for threads in [1, 2, 7] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        results.push(pool.install(|| rough(&f, &v)));
    }
    assert!(results[0].group.segments.len() > 100);
    assert_eq!(results[0], results[1]);
    assert_eq!(results[0], results[2]);
}

#[test]
fn req_op_11_a_plunge_is_refused_for_a_tool_that_cannot_plunge_and_ramp_and_helix_are_not() {
    use openchuck_path::params::{EntryParameters, EntryStyle};
    let plunge = EntryParameters::PLUNGE;
    assert!(plunge.check_tool(true).is_ok());
    let err = plunge.check_tool(false).unwrap_err().to_string();
    assert!(
        err.contains("entry") && err.contains("cannot cut straight down"),
        "{err}"
    );
    for style in [EntryStyle::Lead, EntryStyle::Ramp, EntryStyle::Helix] {
        let e = EntryParameters { style, ..plunge };
        assert!(e.check_tool(false).is_ok(), "{style:?}");
    }
}

#[test]
fn req_op_03_the_part_must_be_under_the_tool_and_the_stock_above_it() {
    // Stock no larger than the part with its stock to leave: nothing to rough.
    let f = fixture(
        &Cylinder {
            radius: 10.0,
            length: 20.0,
        },
        10.2,
        &flat(6.0),
    );
    let err = run(|ctx| generate(&RotaryRoughing, &f.input(), &values(&[]), ctx))
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("no material") || err.contains("no cut"),
        "{err}"
    );
}
