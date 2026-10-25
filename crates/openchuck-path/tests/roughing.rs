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

/// Issue #292: a continuous axis needs no terminal whole-turn return.
#[test]
fn req_op_03_req_op_15_continuous_roughing_retract_keeps_accumulated_a() {
    let part = Cylinder {
        radius: 8.0,
        length: 6.0,
    };
    let f = fixture(&part, 10.0, &flat(3.0));
    let path = rough(&f, &values(&[]));
    let last_cut = path
        .group
        .segments
        .iter()
        .rev()
        .find(|s| s.kind == MoveKind::Cutting)
        .unwrap();
    let end = path.group.segments.last().unwrap();
    assert!(last_cut.end.a.0.abs() > 360.0, "case must accumulate turns");
    assert_eq!(
        end.end.a, last_cut.end.a,
        "safe retract must not discard accumulated turns on a continuous axis"
    );
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
        parameter::DIRECTION,
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

/// The cut direction values (issue 246; decision record 0212 part A).
const USUAL: &str = "usual for the form";
const ALONG: &str = "along the length of the part";
const AROUND: &str = "around the part";

/// The cutting moves of each pass: runs of consecutive cutting segments.
fn cutting_runs(path: &GeneratedPath) -> Vec<Vec<openchuck_types::ir::Segment>> {
    let mut runs = Vec::new();
    let mut run = Vec::new();
    for s in &path.group.segments {
        if s.kind == MoveKind::Cutting {
            run.push(*s);
        } else if !run.is_empty() {
            runs.push(std::mem::take(&mut run));
        }
    }
    if !run.is_empty() {
        runs.push(run);
    }
    runs
}

#[test]
fn req_op_03_req_gc_07_the_cut_direction_defaults_to_the_forms_own_direction() {
    use openchuck_path::{ParameterKind, Strategy};
    let schema = RotaryRoughing.schema();
    let spec = schema.get(parameter::DIRECTION).unwrap();
    assert!(!spec.label.is_empty() && !spec.description.is_empty());
    let ParameterKind::Choice { options, default } = &spec.kind else {
        panic!("the cut direction is a choice");
    };
    assert_eq!(options, &[USUAL, ALONG, AROUND]);
    assert_eq!(default, USUAL);
    // An operation saved before the option (no value) gives the same path as
    // the usual direction for its form, and that is the form's own: around
    // (the helix) for continuous, along for indexed.
    let f = fixture(&cone(), 15.0, &tool(ToolShape::BallEnd, 4.0));
    for (form, own) in [("continuous", AROUND), ("indexed", ALONG)] {
        let base = values(&[
            (parameter::FORM, text(form)),
            (parameter::STEPDOWN, num(3.0)),
        ]);
        let before = rough(&f, &base);
        for direction in [USUAL, own] {
            let mut v = base.clone();
            v.insert(parameter::DIRECTION.to_owned(), text(direction));
            assert_eq!(rough(&f, &v), before, "{form} {direction}");
        }
    }
    let err = run(|ctx| {
        generate(
            &RotaryRoughing,
            &f.input(),
            &values(&[(parameter::DIRECTION, text("diagonal"))]),
            ctx,
        )
    })
    .unwrap_err()
    .to_string();
    assert!(err.contains(parameter::DIRECTION), "{err}");
}

#[test]
fn req_op_03_continuous_roughing_along_the_length_cuts_lines_along_x_and_turns_a_between_them() {
    let part = Cylinder {
        radius: 10.0,
        length: 20.0,
    };
    let f = fixture(&part, 15.0, &flat(6.0));
    let v = values(&[
        (parameter::DIRECTION, text(ALONG)),
        (names::STEPOVER, num(3.0)),
        (parameter::STEPDOWN, num(2.0)),
    ]);
    let path = rough(&f, &v);
    let runs = cutting_runs(&path);
    assert!(runs.len() > 10, "{}", runs.len());
    let mut angles = Vec::new();
    for r in &runs {
        // A is held while the tool cuts, and X runs one way.
        let a = r[0].end.a.0;
        assert!(r.iter().all(|s| (s.end.a.0 - a).abs() < 1e-9), "A moved");
        let xs: Vec<f64> = r.iter().map(|s| s.end.tip.x).collect();
        let rising = xs.windows(2).all(|w| w[1] >= w[0] - 1e-9);
        let falling = xs.windows(2).all(|w| w[1] <= w[0] + 1e-9);
        assert!(rising || falling, "X turned back within a pass");
        angles.push(a.rem_euclid(360.0));
    }
    // Neighbouring lines are no more than the stepover apart at the stock's
    // radius: 2 pi 15 / 3 makes 32 lines, 11.25 degrees apart.
    angles.sort_by(f64::total_cmp);
    angles.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
    assert_eq!(angles.len(), 32, "{angles:?}");
    for w in angles.windows(2) {
        assert!((w[1] - w[0] - 11.25).abs() < 1e-6, "{angles:?}");
    }
}

#[test]
fn req_op_03_req_op_15_indexed_roughing_around_the_part_cuts_across_at_fixed_a_stepping_along_x() {
    let part = Cylinder {
        radius: 10.0,
        length: 20.0,
    };
    let f = fixture(&part, 15.0, &flat(6.0));
    let v = values(&[
        (parameter::FORM, text("indexed")),
        (parameter::DIRECTION, text(AROUND)),
        (parameter::INDEX_ANGLE, num(90.0)),
        (names::STEPOVER, num(3.0)),
        (parameter::STEPDOWN, num(2.0)),
    ]);
    let path = rough(&f, &v);
    let runs = cutting_runs(&path);
    assert!(runs.len() > 10, "{}", runs.len());
    let mut xs = Vec::new();
    for r in &runs {
        // A is one of the index positions and does not move while cutting
        // (OP-15); X is held, so the pass runs across the sector.
        let a = r[0].end.a.0;
        assert!(
            (a.rem_euclid(90.0)).min(90.0 - a.rem_euclid(90.0)) < 1e-9,
            "{a}"
        );
        assert!(r.iter().all(|s| (s.end.a.0 - a).abs() < 1e-9), "A moved");
        let x = r[0].end.tip.x;
        assert!(r.iter().all(|s| (s.end.tip.x - x).abs() < 1e-9), "X moved");
        xs.push(x);
    }
    // The lines step along X by no more than the stepover.
    xs.sort_by(f64::total_cmp);
    xs.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
    assert!(xs.len() > 5, "{xs:?}");
    for w in xs.windows(2) {
        assert!(w[1] - w[0] <= 3.0 + 1e-9, "{xs:?}");
    }
}

#[test]
fn req_op_12_req_nfr_06_both_new_directions_never_gouge_and_are_identical_at_any_thread_count() {
    let f = fixture(&cone(), 15.0, &tool(ToolShape::BallEnd, 4.0));
    for (form, direction) in [("continuous", ALONG), ("indexed", AROUND)] {
        let v = values(&[
            (parameter::FORM, text(form)),
            (parameter::DIRECTION, text(direction)),
            (names::STEPOVER, num(1.5)),
            (parameter::STEPDOWN, num(3.0)),
        ]);
        let mut results = Vec::new();
        for threads in [1, 3] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            results.push(pool.install(|| rough(&f, &v)));
        }
        assert_eq!(results[0], results[1], "{form} {direction}");
        let path = &results[0];
        let outline = Outline::new(&f.tool);
        let g = max_gouge(&outline, &cone(), &poses(path), 0.25);
        assert!(g.depth < 1e-3, "{form} {direction}: {}", g.depth);
        for p in poses(path).iter().step_by(37) {
            let clearance = min_clearance(&outline, &cone(), p, 0.25);
            assert!(clearance > 0.5 - 0.02, "{form} {direction}: {clearance}");
        }
    }
}

#[test]
fn req_nfr_07_too_fine_a_stepover_in_a_new_direction_is_refused_by_name() {
    // A stepover of 2 microns asks for millions of points a layer in either
    // new direction: refused with the parameter to change, before any work.
    let f = fixture(
        &Cylinder {
            radius: 10.0,
            length: 20.0,
        },
        15.0,
        &flat(6.0),
    );
    for (form, direction, name) in [
        ("continuous", ALONG, names::STEPOVER),
        ("indexed", AROUND, parameter::INDEX_ANGLE),
    ] {
        let v = values(&[
            (parameter::FORM, text(form)),
            (parameter::DIRECTION, text(direction)),
            (names::STEPOVER, num(0.002)),
        ]);
        let err = run(|ctx| generate(&RotaryRoughing, &f.input(), &v, ctx))
            .unwrap_err()
            .to_string();
        assert!(
            err.contains(name) && err.contains("too many"),
            "{form}: {err}"
        );
    }
}
