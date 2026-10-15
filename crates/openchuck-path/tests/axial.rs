// SPDX-License-Identifier: AGPL-3.0-or-later
//! Axial finishing (OP-04, with OP-11, OP-13, OP-15; work breakdown slice 12
//! item 2; decision record 0033).
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing
)]

mod support;

use std::collections::BTreeMap;

use openchuck_path::axial::parameter;
use openchuck_path::params::names;
use openchuck_path::{
    AxialFinishing, GeneratedPath, MeshSurface, ParameterValue, Strategy, generate,
};
use openchuck_testkit::gouge::{Outline, Pose, coverage, max_gouge};
use openchuck_testkit::parts::{Cone, Cylinder, RadialPart};
use openchuck_types::ir::{MoveKind, ToolPose};
use openchuck_types::tool::ToolShape;
use support::{Fixture, ball, run, tool};

fn num(x: f64) -> ParameterValue {
    ParameterValue::Number(x)
}

fn values(stepover: f64, extra: &[(&str, ParameterValue)]) -> BTreeMap<String, ParameterValue> {
    let mut v: BTreeMap<String, ParameterValue> = [
        (names::STEPOVER, stepover),
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

fn fixture<P: RadialPart + ?Sized>(
    part: &P,
    rings: u32,
    around: u32,
    t: &openchuck_types::tool::Tool,
) -> Fixture {
    let surface = MeshSurface::new(&part.mesh(rings, around)).unwrap();
    let b = surface.bounds_for_test();
    Fixture::new(surface, b + 1.0, t)
}

trait Radius {
    fn bounds_for_test(&self) -> f64;
}

impl Radius for MeshSurface {
    fn bounds_for_test(&self) -> f64 {
        use openchuck_path::SurfaceSource;
        let b = self.bounds();
        b.max
            .y
            .abs()
            .max(b.max.z.abs())
            .max(b.min.y.abs())
            .max(b.min.z.abs())
    }
}

fn axial(f: &Fixture, v: &BTreeMap<String, ParameterValue>) -> GeneratedPath {
    run(|ctx| generate(&AxialFinishing, &f.input(), v, ctx)).unwrap()
}

/// The cutting segments with the pose each starts from.
fn cuts(path: &GeneratedPath) -> Vec<(ToolPose, openchuck_types::ir::Segment)> {
    let mut prev = path.group.start;
    let mut out = Vec::new();
    for s in &path.group.segments {
        if s.kind == MoveKind::Cutting {
            out.push((prev, *s));
        }
        prev = s.end;
    }
    out
}

/// The cuts grouped by pass: a pass is a run of cuts at one A.
fn passes(path: &GeneratedPath) -> Vec<Vec<(ToolPose, openchuck_types::ir::Segment)>> {
    let mut out: Vec<Vec<(ToolPose, openchuck_types::ir::Segment)>> = Vec::new();
    for c in cuts(path) {
        match out.last_mut() {
            Some(pass) if (pass[0].1.end.a.0 - c.1.end.a.0).abs() < 1e-9 => pass.push(c),
            _ => out.push(vec![c]),
        }
    }
    out
}

#[test]
fn req_op_04_axial_passes_close_the_circle_and_follow_a_cylinder() {
    let part = Cylinder {
        radius: 15.0,
        length: 40.0,
    };
    let f = fixture(&part, 3, 1440, &ball(6.0));
    for (stepover, start) in [(1.0, 0.0), (2.5, 30.0)] {
        let path = axial(
            &f,
            &values(stepover, &[(parameter::START_ANGLE, num(start))]),
        );
        let passes = passes(&path);
        // Whole number of passes, never wider than the stepover at R = 15.
        let n = (std::f64::consts::TAU * 15.0 / stepover).ceil() as usize;
        assert_eq!(passes.len(), n, "stepover {stepover}");
        let step = 360.0 / n as f64;
        for (i, pass) in passes.iter().enumerate() {
            let a = pass[0].1.end.a.0;
            let want = start + step * i as f64;
            assert!(
                (a - want).abs() < 1e-6
                    || ((a - want).rem_euclid(360.0)).min((want - a).rem_euclid(360.0)) < 1e-6,
                "pass {i}: {a} vs {want}"
            );
            // The ball rides the cylinder: the tip is R from the axis.
            for (_, s) in pass {
                let r = s.end.tip.y.hypot(s.end.tip.z);
                assert!((r - 15.0).abs() < 2e-3, "{r}");
            }
        }
        // Neighbouring passes are within the stepover at the surface.
        assert!(step.to_radians() * 15.0 <= stepover + 1e-9);
    }
}

#[test]
fn req_op_04_axial_zigzag_alternates_and_climb_does_not() {
    let part = Cylinder {
        radius: 15.0,
        length: 20.0,
    };
    let f = fixture(&part, 3, 720, &ball(6.0));
    let direction = |name: &str| {
        let v = values(
            4.0,
            &[(parameter::DIRECTION, ParameterValue::Text(name.to_owned()))],
        );
        passes(&axial(&f, &v))
            .iter()
            .map(|p| {
                let (first, last) = (p.first().unwrap().0.tip.x, p.last().unwrap().1.end.tip.x);
                (first, last)
            })
            .collect::<Vec<_>>()
    };
    let zig = direction("zigzag");
    assert!(zig.len() > 4);
    for (i, (from, to)) in zig.iter().enumerate() {
        assert!((to - from).abs() > 19.9, "full length");
        assert_eq!(to > from, i % 2 == 0, "pass {i} direction");
    }
    for (from, to) in direction("climb") {
        assert!(to - from > 19.9);
    }
}

#[test]
fn req_op_04_req_op_11_scallop_sets_the_pass_count_for_ball_and_warns_for_flat() {
    let part = Cylinder {
        radius: 15.0,
        length: 10.0,
    };
    let f = fixture(&part, 3, 720, &ball(6.0));
    // Cusp of a 3 mm ball arc at 15 mm: solve the exact angle independently.
    let (rho, r, h) = (3.0_f64, 15.0_f64, 0.01_f64);
    let big = r + rho;
    let top = r + h;
    let phi = ((big * big + top * top - rho * rho) / (2.0 * big * top)).acos();
    let n = (std::f64::consts::TAU / (2.0 * phi)).ceil() as usize;
    let path = axial(&f, &values(50.0, &[(parameter::SCALLOP, num(h))]));
    assert_eq!(passes(&path).len(), n);
    // The smaller of stepover and scallop wins.
    let path = axial(&f, &values(0.5, &[(parameter::SCALLOP, num(h))]));
    assert_eq!(
        passes(&path).len(),
        (std::f64::consts::TAU * r / 0.5).ceil().max(n as f64) as usize
    );
    // A flat end has no closed form: the scallop is warned about and ignored,
    // and the stepover alone sets the passes (not an error).
    let flat = tool(ToolShape::FlatEnd, 6.0);
    let g = fixture(&part, 3, 720, &flat);
    let with_scallop = values(1.0, &[(parameter::SCALLOP, num(h))]);
    let path = axial(&g, &with_scallop);
    assert_eq!(
        passes(&path).len(),
        (std::f64::consts::TAU * r / 1.0).ceil() as usize
    );
    let schema = AxialFinishing.schema();
    let resolved = schema.resolve(&with_scallop).unwrap();
    let warnings = AxialFinishing.parameter_warnings(&g.tool, &resolved);
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(
        warnings[0].contains("scallop") && warnings[0].contains("ignored"),
        "{warnings:?}"
    );
    // No warning for a ball, or when no scallop is asked for.
    let ball_resolved = schema.resolve(&with_scallop).unwrap();
    assert!(
        AxialFinishing
            .parameter_warnings(&f.tool, &ball_resolved)
            .is_empty()
    );
    let none = schema.resolve(&values(1.0, &[])).unwrap();
    assert!(AxialFinishing.parameter_warnings(&g.tool, &none).is_empty());
}

#[test]
fn req_op_04_axial_leaves_no_gouge_and_no_uncovered_surface_on_a_cone() {
    let part = Cone {
        start_radius: 10.0,
        end_radius: 16.0,
        length: 30.0,
    };
    let f = fixture(&part, 61, 720, &ball(6.0));
    let path = axial(&f, &values(0.5, &[(parameter::MAX_X_STEP, num(0.25))]));
    let all: Vec<Pose> = path
        .group
        .segments
        .iter()
        .filter(|s| matches!(s.kind, MoveKind::Cutting | MoveKind::Plunge))
        .map(|s| Pose {
            tip: s.end.tip,
            axis: s.end.axis.get(),
        })
        .collect();
    let outline = Outline::new(&f.tool);
    let (x0, x1) = part.x_range();
    let mid = 0.5 * (x0 + x1);
    let near: Vec<Pose> = all
        .iter()
        .filter(|p| (p.tip.x - mid).abs() < 4.0)
        .copied()
        .step_by(3)
        .collect();
    let g = max_gouge(&outline, &part, &near, 0.15);
    assert!(g.depth <= 0.003, "gouge {}", g.depth);
    let c = coverage(&outline, &part, &all, (mid - 3.0, mid + 3.0), 0.2, 0.03);
    assert_eq!(c.uncovered, 0, "{c:?}");
}

#[test]
fn req_op_11_lead_entry_works_on_all_three_finishing_strategies() {
    use openchuck_path::indexed::parameter as indexed_parameter;
    use openchuck_path::{HelicalFinishing, IndexedFinishing, Strategy};
    let part = Cylinder {
        radius: 15.0,
        length: 20.0,
    };
    let f = fixture(&part, 41, 720, &ball(6.0));
    let outline = Outline::new(&f.tool);
    let lead = [(names::ENTRY, ParameterValue::Text("lead".into()))];
    type Case<'a> = (&'a dyn Strategy, Vec<(&'a str, ParameterValue)>);
    let strategies: Vec<Case<'_>> = vec![
        (&AxialFinishing, vec![(names::STEPOVER, num(3.0))]),
        (
            &IndexedFinishing,
            vec![(indexed_parameter::COUNT, ParameterValue::Integer(4))],
        ),
        (&HelicalFinishing, vec![(names::STEPOVER, num(5.0))]),
    ];
    for (strategy, extra) in strategies {
        let mut v = values(3.0, &lead);
        for (k, x) in extra {
            v.insert(k.to_owned(), x);
        }
        let path = run(|ctx| generate(strategy, &f.input(), &v, ctx))
            .unwrap_or_else(|e| panic!("{}: {e}", strategy.name()));
        let kinds: Vec<MoveKind> = path.group.segments.iter().map(|s| s.kind).collect();
        let lead_in = kinds.iter().filter(|k| **k == MoveKind::LeadIn).count();
        let lead_out = kinds.iter().filter(|k| **k == MoveKind::LeadOut).count();
        assert!(
            lead_in >= 1 && lead_out >= 1,
            "{}: {lead_in} {lead_out}",
            strategy.name()
        );
        // No move of the whole path cuts into the part.
        let poses: Vec<Pose> = path
            .group
            .segments
            .iter()
            .filter(|s| !matches!(s.kind, MoveKind::Rapid | MoveKind::Linking))
            .map(|s| Pose {
                tip: s.end.tip,
                axis: s.end.axis.get(),
            })
            .collect();
        let g = max_gouge(&outline, &part, &poses, 0.15);
        assert!(g.depth <= 0.003, "{}: gouge {}", strategy.name(), g.depth);
    }
}

mod property {
    use super::*;
    use openchuck_path::HelicalFinishing;
    use proptest::prelude::*;

    /// Distance in the development (X, R times the angle in radians) from
    /// (`x`, `theta` degrees) to the nearest cutting point of `path`, with R the
    /// part's largest radius.
    fn nearest(path: &GeneratedPath, r: f64, x: f64, theta: f64) -> f64 {
        path.group
            .segments
            .iter()
            .filter(|s| s.kind == MoveKind::Cutting)
            .map(|s| {
                let da = (s.end.a.0 - theta).rem_euclid(360.0);
                let da = da.min(360.0 - da).to_radians() * r;
                (s.end.tip.x - x).hypot(da)
            })
            .fold(f64::INFINITY, f64::min)
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(12))]

        /// OP-04: every point of the surface lies within the stepover of an
        /// axial or helical path, on cones of any slope.
        #[test]
        fn req_op_04_every_surface_point_is_within_the_stepover_of_the_path(
            r0 in 8.0..14.0f64,
            r1 in 8.0..14.0f64,
            stepover in 0.8..3.0f64,
            start in -180.0..180.0f64,
            probes in proptest::collection::vec((0.0..1.0f64, 0.0..360.0f64), 12),
        ) {
            let length = 20.0;
            let part = Cone { start_radius: r0, end_radius: r1, length };
            let f = fixture(&part, 21, 360, &ball(4.0));
            let r = r0.max(r1);
            let axial_path = axial(
                &f,
                &values(stepover, &[
                    (parameter::START_ANGLE, num(start)),
                    (parameter::MAX_X_STEP, num(0.5)),
                ]),
            );
            let mut hv = values(stepover, &[]);
            hv.insert(
                openchuck_path::helical::parameter::START_ANGLE.to_owned(),
                num(start),
            );
            let helical_path =
                run(|ctx| generate(&HelicalFinishing, &f.input(), &hv, ctx)).unwrap();
            for (u, theta) in probes {
                let x = u * length;
                for (what, path) in [("axial", &axial_path), ("helical", &helical_path)] {
                    let d = nearest(path, r, x, theta);
                    prop_assert!(d <= stepover, "{what}: ({x}, {theta}) is {d} from the path");
                }
            }
        }
    }
}
