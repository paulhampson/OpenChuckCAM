// SPDX-License-Identifier: AGPL-3.0-or-later
//! Axial finishing (OP-04, with OP-11, OP-13, OP-15; work breakdown slice 12
//! item 2; decision record 0034).
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
use openchuck_path::{AxialFinishing, GeneratedPath, MeshSurface, ParameterValue, Strategy};
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
    // Adaptive spacing off: the Scallop height as before (decision record
    // 0239).
    let with_scallop = values(
        1.0,
        &[
            (parameter::SCALLOP, num(h)),
            ("adaptive_spacing", ParameterValue::Flag(false)),
        ],
    );
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
        // No rapid puts the ball into the stock (a cylinder of radius 16 from
        // X = -1 to 21): the lead arcs start outside the part, where a line
        // through the tool tip alone can miss the stock end beside the ball.
        let mut prev = path.group.start;
        for s in &path.group.segments {
            if s.kind == MoveKind::Rapid {
                for i in 0..=20 {
                    let t = f64::from(i) / 20.0;
                    // The machine moves in a straight line in X, Y, Z and A.
                    let (p0, p1) = (
                        f.rotary.to_axis(prev.tip, prev.a),
                        f.rotary.to_axis(s.end.tip, s.end.a),
                    );
                    let at = openchuck_path::rotary::AxisPosition {
                        along: p0.along + t * (p1.along - p0.along),
                        across: p0.across + t * (p1.across - p0.across),
                        height: p0.height + t * (p1.height - p0.height),
                    };
                    let a = openchuck_types::units::Degrees(prev.a.0 + t * (s.end.a.0 - prev.a.0));
                    let tip = f.rotary.to_model(at, a);
                    let c = tip + f.rotary.tool_axis(a).get() * 3.0;
                    let dx = (-1.0 - c.x).max(0.0).max(c.x - 21.0);
                    let dr = (c.y.hypot(c.z) - 16.0).max(0.0);
                    assert!(
                        // The cutter is judged by 32 lines round it, so it may
                        // reach material by up to 3 (1 - cos(pi / 32)) = 0.015 mm.
                        dx.hypot(dr) > 3.0 - 0.02,
                        "{}: a rapid puts the ball {:.3} mm into the stock at {c:?}",
                        strategy.name(),
                        3.0 - dx.hypot(dr)
                    );
                }
            }
            prev = s.end;
        }
    }
}

mod property {
    use super::*;
    use openchuck_path::HelicalFinishing;
    use proptest::prelude::*;

    /// Distance in the development (X, R times the angle in radians) from
    /// (`x`, `theta` degrees) to the nearest point of the cutting moves of
    /// `path`, with R the part's largest radius. The cutter travels along each
    /// move, so a move counts along its whole length, not only at its ends;
    /// the turn is unwrapped so a point is found across the 360 degree seam.
    pub(super) fn nearest(path: &GeneratedPath, r: f64, x: f64, theta: f64) -> f64 {
        let mut from = path.group.start;
        let mut best = f64::INFINITY;
        for s in &path.group.segments {
            let (a, b) = (from, s.end);
            from = s.end;
            if s.kind != MoveKind::Cutting {
                continue;
            }
            let (a0, a1) = (a.a.0, b.a.0);
            let first = ((a0.min(a1) - theta) / 360.0).floor() as i64 - 1;
            let last = ((a0.max(a1) - theta) / 360.0).ceil() as i64 + 1;
            let (p0, p1) = (
                [a.tip.x, r * a0.to_radians()],
                [b.tip.x, r * a1.to_radians()],
            );
            for k in first..=last {
                let probe = [x, r * (theta + 360.0 * k as f64).to_radians()];
                best = best.min(distance_to_segment(probe, p0, p1));
            }
        }
        best
    }

    /// Distance from `p` to the segment `a` to `b`, in a plane.
    fn distance_to_segment(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let length2 = dx * dx + dy * dy;
        let t = if length2 > 0.0 {
            (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / length2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        (p[0] - (a[0] + t * dx)).hypot(p[1] - (a[1] + t * dy))
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

/// OP-04: the ends of a part are within the stepover of an axial or helical
/// path. The last turn of a helix stops at the end of the part, so the ends are
/// where the gap is widest (just under one pitch); they are checked on a fixed
/// set of parts and every degree of A, not left to the random probes of the
/// property test, which met them only by chance (a failure in CI at
/// `r0 = 8`, `r1 = 13.9095`, `stepover = 0.97188`, `start = -169.4`, probe at
/// `x = 19.934`, 50.8 degrees, was this and not a flaw of either path).
#[test]
fn req_op_04_the_ends_of_a_part_are_within_the_stepover_of_the_path() {
    use openchuck_path::HelicalFinishing;
    // (r0, r1, stepover, start angle): the reported case first, then steep
    // and shallow cones, the widest and the narrowest stepover, and starts
    // either side of the seam.
    let cases = [
        (
            8.0,
            13.909_514_619_978_863,
            0.971_875_737_810_104_7,
            -169.401_701_445_534_7,
        ),
        (14.0, 8.0, 0.8, 0.0),
        (8.0, 14.0, 3.0, 179.0),
        (11.0, 11.0, 1.7, -180.0),
        (9.3, 12.1, 2.2, 90.0),
    ];
    let length = 20.0;
    for (r0, r1, stepover, start) in cases {
        let part = Cone {
            start_radius: r0,
            end_radius: r1,
            length,
        };
        let f = fixture(&part, 21, 360, &ball(4.0));
        let r = r0.max(r1);
        let axial_path = axial(
            &f,
            &values(
                stepover,
                &[
                    (parameter::START_ANGLE, num(start)),
                    (parameter::MAX_X_STEP, num(0.5)),
                ],
            ),
        );
        let mut hv = values(stepover, &[]);
        hv.insert(
            openchuck_path::helical::parameter::START_ANGLE.to_owned(),
            num(start),
        );
        let helical_path = run(|ctx| generate(&HelicalFinishing, &f.input(), &hv, ctx)).unwrap();
        for (what, path) in [("axial", &axial_path), ("helical", &helical_path)] {
            let mut worst = (0.0f64, 0.0f64, 0.0f64);
            for x in [
                0.0,
                0.01,
                0.066,
                0.25,
                length - 0.25,
                length - 0.066,
                length - 0.01,
                length,
            ] {
                for degree in 0..360 {
                    let theta = f64::from(degree) + 0.5;
                    let d = property::nearest(path, r, x, theta);
                    if d > worst.0 {
                        worst = (d, x, theta);
                    }
                }
            }
            assert!(
                worst.0 <= stepover * (1.0 + 1e-9),
                "{what} r0 {r0} r1 {r1} stepover {stepover} start {start}: ({}, {}) is {} from the path",
                worst.1,
                worst.2,
                worst.0
            );
        }
    }
}

/// OP-14: no rapid puts the cutter into the stock, with a lead arc that starts
/// beyond the end of the part (and so beside the end of the stock), on a
/// part much smaller than its stock. The line through the tool tip alone
/// misses the stock end there, so the descent was stopped too low.
#[test]
fn req_op_14_req_op_11_lead_entry_beside_the_stock_end_never_rapids_into_stock() {
    let part = Cone {
        start_radius: 10.0,
        end_radius: 5.0,
        length: 30.0,
    };
    let surface = MeshSurface::new(&part.mesh(121, 144)).unwrap();
    // Stock: radius 11, from X = -1 to 31 (what `Fixture` makes).
    let f = Fixture::new(surface, 11.0, &ball(3.0));
    let v = values(
        2.0,
        &[
            (names::ENTRY, ParameterValue::Text("lead".into())),
            (parameter::MAX_X_STEP, num(3.0)),
        ],
    );
    let path = axial(&f, &v);
    let mut prev = path.group.start;
    let mut worst = 0.0_f64;
    let mut rapids = 0;
    for s in &path.group.segments {
        if s.kind == MoveKind::Rapid {
            rapids += 1;
            for i in 0..=20 {
                let t = f64::from(i) / 20.0;
                // The machine moves in a straight line in X, Y, Z and A.
                let (p0, p1) = (
                    f.rotary.to_axis(prev.tip, prev.a),
                    f.rotary.to_axis(s.end.tip, s.end.a),
                );
                let at = openchuck_path::rotary::AxisPosition {
                    along: p0.along + t * (p1.along - p0.along),
                    across: p0.across + t * (p1.across - p0.across),
                    height: p0.height + t * (p1.height - p0.height),
                };
                let a = openchuck_types::units::Degrees(prev.a.0 + t * (s.end.a.0 - prev.a.0));
                let c = f.rotary.to_model(at, a) + f.rotary.tool_axis(a).get() * 1.5;
                let dx = (-1.0 - c.x).max(0.0).max(c.x - 31.0);
                let dr = (c.y.hypot(c.z) - 11.0).max(0.0);
                worst = worst.max(1.5 - dx.hypot(dr));
            }
        }
        prev = s.end;
    }
    assert!(rapids > 10);
    // The cutter is judged by 32 lines round it, so it may reach material by
    // up to 1.5 (1 - cos(pi / 32)) = 0.007 mm.
    assert!(
        worst <= 0.02,
        "a rapid puts the ball {worst:.3} mm into the stock"
    );
}

/// The surface of a fixture, noting which of the pool's threads dropped the
/// tool on it.
#[derive(Debug)]
struct Watched<'a> {
    surface: &'a MeshSurface,
    threads: std::sync::Mutex<std::collections::BTreeSet<Option<usize>>>,
}

impl openchuck_path::SurfaceSource for Watched<'_> {
    fn bounds(&self) -> openchuck_types::geometry::Aabb {
        self.surface.bounds()
    }

    fn project(
        &self,
        point: openchuck_types::geometry::Point3,
    ) -> Option<openchuck_path::surface::SurfacePoint> {
        self.surface.project(point)
    }

    fn cast_ray(
        &self,
        origin: openchuck_types::geometry::Point3,
        direction: openchuck_types::geometry::UnitVec3,
        max_distance: f64,
    ) -> Option<openchuck_path::surface::RayHit> {
        self.surface.cast_ray(origin, direction, max_distance)
    }

    fn tessellation(&self) -> &openchuck_types::mesh::TriangleMesh {
        self.surface.tessellation()
    }

    fn drop_tool(
        &self,
        profile: &openchuck_types::tool::RevolvedProfile,
        line: &openchuck_path::cutter::ToolLine,
    ) -> Option<openchuck_path::cutter::Contact> {
        self.threads
            .lock()
            .unwrap()
            .insert(rayon::current_thread_index());
        self.surface.drop_tool(profile, line)
    }
}

#[test]
fn req_nfr_04_axial_finishing_fits_its_passes_on_more_than_one_thread() {
    // Issue #252: a pass of fewer than 64 points was fitted on one thread,
    // and the passes one after another, so the whole job ran on one core.
    let part = Cylinder {
        radius: 10.0,
        length: 20.0,
    };
    let f = fixture(&part, 3, 360, &ball(3.0));
    let watched = Watched {
        surface: &f.surface,
        threads: std::sync::Mutex::default(),
    };
    let input = openchuck_path::StrategyInput {
        surface: &watched,
        ..f.input()
    };
    let v = values(0.5, &[]);
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(4)
        .build()
        .unwrap();
    pool.install(|| run(|ctx| generate(&AxialFinishing, &input, &v, ctx)).unwrap());
    let threads = watched.threads.lock().unwrap().len();
    assert!(threads > 1, "the tool was dropped on {threads} thread");
}

#[test]
fn req_nfr_06_axial_finishing_is_identical_at_1_2_and_n_threads() {
    let part = Cone {
        start_radius: 12.0,
        end_radius: 6.0,
        length: 20.0,
    };
    let f = fixture(&part, 21, 360, &ball(3.0));
    for direction in ["zigzag", "climb"] {
        let v = values(
            1.0,
            &[(parameter::DIRECTION, ParameterValue::Text(direction.into()))],
        );
        let runs = openchuck_testkit::determinism::check(|| {
            format!("{:?}", axial(&f, &v).group).into_bytes()
        })
        .unwrap();
        assert_eq!(runs.len(), 4);
    }
}

/// The progress reports of a job, in the order they were sent.
#[derive(Default)]
struct Reports(std::sync::Mutex<Vec<openchuck_types::job::Progress>>);

impl openchuck_types::job::ProgressSink for Reports {
    fn report(&self, progress: openchuck_types::job::Progress) {
        self.0.lock().unwrap().push(progress);
    }
}

#[test]
fn req_nfr_04_req_ui_09_req_nfr_06_quiet_passes_keep_timings_without_changing_progress_or_paths() {
    use openchuck_path::IndexedFinishing;
    use openchuck_types::job::{CancelToken, JobContext, Progress, ProgressSink};
    use std::sync::Mutex;
    use std::time::Duration;

    #[derive(Default)]
    struct TimedReports {
        enabled: bool,
        progress: Reports,
        stages: Mutex<Vec<String>>,
    }
    impl ProgressSink for TimedReports {
        fn report(&self, progress: Progress) {
            self.progress.report(progress);
        }
        fn wants_timings(&self) -> bool {
            self.enabled
        }
        fn timing(&self, stage: &str, _: Duration) {
            self.stages.lock().unwrap().push(stage.to_owned());
        }
    }

    let part = Cylinder {
        radius: 5.0,
        length: 2.0,
    };
    let f = fixture(&part, 3, 90, &ball(2.0));
    let v = values(2.0, &[]);
    for strategy in [&AxialFinishing as &dyn Strategy, &IndexedFinishing] {
        for threads in [1, 4] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            pool.install(|| {
                let cancel = CancelToken::new();
                let off = TimedReports::default();
                let on = TimedReports {
                    enabled: true,
                    ..TimedReports::default()
                };
                let baseline =
                    generate(strategy, &f.input(), &v, &JobContext::new(&cancel, &off)).unwrap();
                let timed =
                    generate(strategy, &f.input(), &v, &JobContext::new(&cancel, &on)).unwrap();
                assert_eq!(baseline, timed);
                assert_eq!(
                    off.progress.0.into_inner().unwrap(),
                    on.progress.0.into_inner().unwrap()
                );
                assert!(off.stages.into_inner().unwrap().is_empty());
                let stages = on.stages.into_inner().unwrap();
                assert!(
                    stages.iter().any(|s| s == "fitting"),
                    "{} at {threads} threads: {stages:?}",
                    strategy.name()
                );
                assert!(stages.iter().any(|s| s == "assembly"));
            });
        }
    }
}

#[test]
fn req_ui_09_axial_finishing_counts_its_passes_with_one_total_that_only_goes_up() {
    // Issue #248: each pass reported its own counts (its points, then its
    // pieces), so the progress shown jumped between totals as the passes
    // ran. The passes are counted instead, with one total.
    let part = Cylinder {
        radius: 10.0,
        length: 20.0,
    };
    let f = fixture(&part, 3, 360, &ball(3.0));
    let v = values(2.0, &[]);
    let reports = Reports::default();
    let cancel = openchuck_types::job::CancelToken::new();
    let context = openchuck_types::job::JobContext::new(&cancel, &reports);
    generate(&AxialFinishing, &f.input(), &v, &context).unwrap();
    let reports = reports.0.into_inner().unwrap();
    let strategy: Vec<_> = reports
        .iter()
        .filter(|p| p.stage.starts_with("axial finishing"))
        .collect();
    assert!(!strategy.is_empty());
    let total = strategy[0].total;
    // 2 mm round a radius of 10: 32 passes.
    assert_eq!(total, Some(32));
    let mut done = 0;
    for p in &strategy {
        assert_eq!(p.total, total, "{} changed the total", p.stage);
        assert!(p.done >= done, "{} went back from {done}", p.stage);
        done = p.done;
    }
    assert_eq!(done, 32);
}

/// A surface that cancels the job from inside the pass at its `at`th tool
/// axis: an axial pass keeps one axis, so on one thread this is the `at`th
/// pass, which then fails at its next check.
#[derive(Debug)]
struct CancelInPass<'a> {
    surface: &'a MeshSurface,
    at: usize,
    cancel: openchuck_types::job::CancelToken,
    axes: std::sync::Mutex<Vec<[u64; 3]>>,
}

impl openchuck_path::SurfaceSource for CancelInPass<'_> {
    fn bounds(&self) -> openchuck_types::geometry::Aabb {
        self.surface.bounds()
    }
    fn project(
        &self,
        point: openchuck_types::geometry::Point3,
    ) -> Option<openchuck_path::SurfacePoint> {
        self.surface.project(point)
    }
    fn cast_ray(
        &self,
        origin: openchuck_types::geometry::Point3,
        direction: openchuck_types::geometry::UnitVec3,
        max_distance: f64,
    ) -> Option<openchuck_path::RayHit> {
        self.surface.cast_ray(origin, direction, max_distance)
    }
    fn tessellation(&self) -> &openchuck_types::mesh::TriangleMesh {
        self.surface.tessellation()
    }
    fn drop_tool(
        &self,
        profile: &openchuck_types::tool::RevolvedProfile,
        line: &openchuck_path::ToolLine,
    ) -> Option<openchuck_path::Contact> {
        let a = line.axis.get();
        let key = [a.x.to_bits(), a.y.to_bits(), a.z.to_bits()];
        let mut axes = self.axes.lock().unwrap();
        if !axes.contains(&key) {
            axes.push(key);
            if axes.len() == self.at {
                self.cancel.cancel();
            }
        }
        drop(axes);
        self.surface.drop_tool(profile, line)
    }
}

#[test]
fn req_ui_09_a_pass_that_fails_is_not_counted_as_done() {
    // OI-843: a pass that ended with an error was counted, so the progress
    // could read "32 of 32" just before the run ended with that error. The
    // last pass is cancelled from inside, on one thread so the passes run
    // in order: 31 passes are done, and the count says so.
    use openchuck_path::SurfaceSource;
    let part = Cylinder {
        radius: 10.0,
        length: 20.0,
    };
    let f = fixture(&part, 3, 360, &ball(3.0));
    let v = values(2.0, &[]);
    let cancel = openchuck_types::job::CancelToken::new();
    let surface = CancelInPass {
        surface: &f.surface,
        at: 32,
        cancel: cancel.clone(),
        axes: std::sync::Mutex::default(),
    };
    assert!(surface.bounds().max.x > surface.bounds().min.x);
    let mut input = f.input();
    input.surface = &surface;
    let reports = Reports::default();
    let context = openchuck_types::job::JobContext::new(&cancel, &reports);
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(1)
        .build()
        .unwrap();
    let result = pool.install(|| generate(&AxialFinishing, &input, &v, &context));
    assert!(result.is_err());
    let done = reports
        .0
        .into_inner()
        .unwrap()
        .iter()
        .filter(|p| p.stage.ends_with("fitting the passes"))
        .map(|p| p.done)
        .max();
    assert_eq!(done, Some(31));
}
