// SPDX-License-Identifier: AGPL-3.0-or-later
//! Edge and curve following on mesh feature edges (OP-08, OP-16, NFR-06;
//! work breakdown slice 16; decision record 0047). The strategy against the
//! simulated stock is in `crates/openchuck-sim/tests/edge_following.rs`.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing
)]

mod support;

use std::collections::BTreeMap;

use openchuck_path::curves::feature_chains;
use openchuck_path::edge_following::parameter as p;
use openchuck_path::params::names;
use openchuck_path::{
    ChosenSegments, CurveSource, EdgeChain, EdgeFollowing, MeshFeatureEdges, MeshSurface,
    ParameterValue, Pass, PathError, Registry, Strategy, SurfaceSource, generate,
};
use openchuck_testkit::parts::revolve;
use openchuck_types::geometry::Point3;
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::tool::ToolShape;
use support::{Fixture, ball, run, tool};

const RADIUS: f64 = 10.0;
const LENGTH: f64 = 20.0;

fn num(x: f64) -> ParameterValue {
    ParameterValue::Number(x)
}

fn text(s: &str) -> ParameterValue {
    ParameterValue::Text(s.to_owned())
}

fn values(extra: &[(&str, ParameterValue)]) -> BTreeMap<String, ParameterValue> {
    let mut v: BTreeMap<String, ParameterValue> = [
        (names::STEPOVER, 0.5),
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

/// A closed cylinder about the rotary axis: two closed edge circles, one at
/// each end.
fn cylinder() -> Fixture {
    let mesh = revolve(&[(0.0, RADIUS), (LENGTH, RADIUS)], 180);
    Fixture::new(MeshSurface::new(&mesh).unwrap(), RADIUS + 1.0, &ball(1.0))
}

fn passes(f: &Fixture, strategy: &dyn Strategy, v: &BTreeMap<String, ParameterValue>) -> Vec<Pass> {
    let parameters = strategy.schema().resolve(v).unwrap();
    run(|ctx| {
        strategy
            .passes_with_notes(&f.input(), None, &parameters, ctx)
            .map(|o| o.passes)
    })
    .unwrap()
}

fn radius(p: Point3) -> f64 {
    p.y.hypot(p.z)
}

fn angle(p: Point3) -> f64 {
    p.z.atan2(p.y).to_degrees()
}

/// Half a cylinder's length of surface as a half pipe: radius 10 over the
/// first 90 degrees of its section and 12 over the rest, with a radial wall
/// between them. The edges of the open surface give open chains that wind
/// round the axis: an arc, a straight edge along it, and another arc.
fn stepped_half_pipe() -> TriangleMesh {
    let mut columns: Vec<(f64, f64)> = Vec::new();
    for i in 0..=30 {
        columns.push((3.0 * f64::from(i), 10.0));
    }
    for i in 30..=60 {
        columns.push((3.0 * f64::from(i), 12.0));
    }
    let rows = 4;
    let mut vertices = Vec::new();
    for r in 0..=rows {
        let x = LENGTH * f64::from(r) / f64::from(rows);
        for &(deg, radius) in &columns {
            let a = deg.to_radians();
            vertices.push(Point3::new(x, radius * a.cos(), radius * a.sin()));
        }
    }
    let width = columns.len() as u32;
    let mut triangles = Vec::new();
    for r in 0..rows as u32 {
        for c in 0..width - 1 {
            let (a, b) = (r * width + c, r * width + c + 1);
            let (d, e) = (a + width, b + width);
            triangles.push([a, b, e]);
            triangles.push([a, e, d]);
        }
    }
    TriangleMesh {
        vertices,
        triangles,
    }
}

fn half_pipe() -> Fixture {
    Fixture::new(
        MeshSurface::new(&stepped_half_pipe()).unwrap(),
        13.0,
        &ball(1.0),
    )
}

// ---- chains -------------------------------------------------------------

#[test]
fn req_op_08_the_ends_of_a_cylinder_are_two_closed_chains() {
    let mesh = revolve(&[(0.0, RADIUS), (LENGTH, RADIUS)], 180);
    let chains = feature_chains(&mesh, 30.0);
    assert_eq!(chains.len(), 2, "{chains:?}");
    for c in &chains {
        assert!(c.closed);
        assert_eq!(c.points.first(), c.points.last());
        assert_eq!(c.points.len(), 181);
        let circumference = std::f64::consts::TAU * RADIUS;
        assert!((c.length() - circumference).abs() < 0.2, "{}", c.length());
    }
}

#[test]
fn req_op_08_a_smooth_surface_has_no_feature_edge_and_the_crease_angle_decides() {
    let sphere_like = revolve(&[(0.0, 0.5), (5.0, 3.0), (10.0, 0.5)], 90);
    // Every join of the cone sides turns 20 degrees or more at the rim and
    // 90 degrees at the caps; nothing turns 100.
    assert!(feature_chains(&sphere_like, 100.0).is_empty());
    assert!(!feature_chains(&sphere_like, 30.0).is_empty());
}

#[test]
fn req_op_08_the_open_edges_of_a_surface_are_open_chains_split_where_three_meet() {
    let chains = feature_chains(&stepped_half_pipe(), 30.0);
    let open: Vec<&EdgeChain> = chains.iter().filter(|c| !c.closed).collect();
    assert!(open.len() >= 3, "{} chains", chains.len());
    // The arc and the straight edge at the low side of the section join at
    // their corner into one chain, which winds round the axis.
    assert!(open.iter().any(|c| {
        c.points
            .iter()
            .any(|q| (radius(*q) - 10.0).abs() < 1e-6 && q.x == 0.0)
            && c.points.iter().any(|q| q.x == LENGTH)
    }));
}

#[test]
fn req_op_08_short_chains_are_dropped_by_the_shortest_length() {
    let f = Fixture::new(
        MeshSurface::new(&revolve(
            &[(0.0, 6.0), (12.0, 6.0), (12.0, 12.0), (24.0, 12.0)],
            180,
        ))
        .unwrap(),
        13.0,
        &ball(1.0),
    );
    let all = MeshFeatureEdges::new(30.0).chains(&f.surface).unwrap();
    assert_eq!(all.len(), 4);
    let long = MeshFeatureEdges {
        crease_degrees: 30.0,
        min_length: 40.0,
    }
    .chains(&f.surface)
    .unwrap();
    assert_eq!(
        long.len(),
        2,
        "the two circles of radius 12 are 75 mm round"
    );
}

// ---- depth ---------------------------------------------------------------

#[test]
fn req_op_08_engraved_depth_is_constant_on_a_curved_surface() {
    let f = cylinder();
    let depth = 0.3;
    let tolerance = 0.01;
    let out = passes(
        &f,
        &EdgeFollowing::default(),
        &values(&[(p::DEPTH, num(depth))]),
    );
    assert_eq!(out.len(), 2, "one pass round each end");
    for pass in &out {
        assert!(pass.points.len() > 100);
        for c in &pass.points {
            let r = radius(c.tip);
            assert!(
                (r - (RADIUS - depth)).abs() <= tolerance,
                "tip at radius {r}, wanted {}",
                RADIUS - depth
            );
        }
        // The chain's own points are cut exactly.
        let exact = pass
            .points
            .iter()
            .filter(|c| (radius(c.tip) - (RADIUS - depth)).abs() < 1e-9)
            .count();
        assert!(exact >= 180, "{exact}");
        // A faces the point: the tool axis is the radial direction.
        for c in &pass.points {
            let axis = f.rotary.tool_axis(c.a).get();
            let tip = c.tip.to_vec();
            let out = Point3::new(0.0, tip.y, tip.z);
            let cross = (out.y * axis.z - out.z * axis.y).abs();
            assert!(cross < 1e-9, "the axis is not radial: {cross}");
        }
    }
}

#[test]
fn req_op_08_an_open_chain_winding_round_a_curved_surface_is_cut_at_constant_depth() {
    let f = half_pipe();
    let depth = 0.25;
    let out = passes(
        &f,
        &EdgeFollowing::default(),
        &values(&[(p::DEPTH, num(depth)), (p::MAX_STEP, num(0.4))]),
    );
    // Three or more open chains, including arcs that wind round the axis.
    assert!(out.len() >= 3);
    let mut turned = 0.0_f64;
    for pass in &out {
        // The wall between the two radii is a radial edge: the tool goes in
        // and out along its own axis there, so it has no one radius.
        let wall = pass
            .points
            .iter()
            .all(|c| c.tip.x == pass.points[0].tip.x && (angle(c.tip) - 90.0).abs() < 1e-6);
        if wall {
            let (lo, hi) = pass.points.iter().fold((f64::MAX, f64::MIN), |m, c| {
                (m.0.min(radius(c.tip)), m.1.max(radius(c.tip)))
            });
            assert!((lo - (10.0 - depth)).abs() < 1e-9 && (hi - (12.0 - depth)).abs() < 1e-9);
            continue;
        }
        let mut widest = 0.0_f64;
        for c in &pass.points {
            let r = radius(c.tip);
            let surface = if r < 11.0 { 10.0 } else { 12.0 };
            assert!(
                (r - (surface - depth)).abs() <= 0.01,
                "tip radius {r}, surface radius {surface}"
            );
            widest = widest.max(angle(c.tip));
        }
        turned = turned.max(widest);
    }
    assert!(turned > 150.0, "an arc reaches the far side: {turned}");
}

#[test]
fn req_op_08_a_curve_off_the_surface_is_projected_along_the_tool_axis() {
    // A helix wound round the cylinder 1 mm above it.
    #[derive(Debug)]
    struct Helix;
    impl CurveSource for Helix {
        fn chains(&self, _: &dyn SurfaceSource) -> openchuck_path::Result<Vec<EdgeChain>> {
            let points = (0..=360)
                .map(|i| {
                    let a = f64::from(i).to_radians();
                    Point3::new(
                        2.0 + 16.0 * f64::from(i) / 360.0,
                        11.0 * a.cos(),
                        11.0 * a.sin(),
                    )
                })
                .collect();
            Ok(vec![EdgeChain {
                points,
                closed: false,
            }])
        }
        fn lies_on_surface(&self) -> bool {
            false
        }
    }
    let f = cylinder();
    let strategy = EdgeFollowing::with_source(std::sync::Arc::new(Helix));
    let out = passes(&f, &strategy, &values(&[(p::DEPTH, num(0.4))]));
    assert_eq!(out.len(), 1);
    for c in &out[0].points {
        // The surface is a 2 degree faceted cylinder: its flats are up to
        // 0.0015 mm inside the circle.
        assert!(
            (radius(c.tip) - (RADIUS - 0.4)).abs() < 0.003,
            "{}",
            radius(c.tip)
        );
    }
}

#[test]
fn req_op_08_a_curve_that_misses_the_surface_is_refused_with_a_message() {
    #[derive(Debug)]
    struct Beyond;
    impl CurveSource for Beyond {
        fn chains(&self, _: &dyn SurfaceSource) -> openchuck_path::Result<Vec<EdgeChain>> {
            Ok(vec![EdgeChain {
                points: vec![Point3::new(50.0, 11.0, 0.0), Point3::new(55.0, 11.0, 0.0)],
                closed: false,
            }])
        }
        fn lies_on_surface(&self) -> bool {
            false
        }
    }
    let f = cylinder();
    let strategy = EdgeFollowing::with_source(std::sync::Arc::new(Beyond));
    let parameters = strategy.schema().resolve(&values(&[])).unwrap();
    let e = run(|ctx| strategy.passes(&f.input(), &parameters, ctx)).unwrap_err();
    assert!(
        e.to_string().contains("does not project onto the surface"),
        "{e}"
    );
}

// ---- width, levels, carving ---------------------------------------------

#[test]
fn req_op_08_a_v_bit_cuts_the_requested_width_by_going_to_the_depth_that_makes_it() {
    let v_bit = tool(
        ToolShape::VBit {
            included_angle: openchuck_types::units::Degrees(60.0),
            tip_diameter: openchuck_types::units::Millimetres(0.0),
        },
        3.0,
    );
    let f = Fixture::new(
        MeshSurface::new(&revolve(&[(0.0, RADIUS), (LENGTH, RADIUS)], 180)).unwrap(),
        RADIUS + 1.0,
        &v_bit,
    );
    let wanted = 1.0 / (30.0_f64.to_radians().tan()) / 2.0;
    let params = values(&[(p::LINE_WIDTH, num(1.0)), (p::DEPTH, num(5.0))]);
    let strategy = EdgeFollowing::default();
    let out = passes(&f, &strategy, &params);
    for c in &out[0].points {
        assert!((radius(c.tip) - (RADIUS - wanted)).abs() < 0.003);
    }
    let parameters = strategy.schema().resolve(&params).unwrap();
    let warnings = strategy.parameter_warnings(&f.tool, &parameters);
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("depth parameter is not used"))
    );
    // A deeper line is wider: halve the width and the depth halves.
    let narrow = passes(&f, &strategy, &values(&[(p::LINE_WIDTH, num(0.5))]));
    let r = radius(narrow[0].points[0].tip);
    assert!((r - (RADIUS - wanted / 2.0)).abs() < 0.003, "{r}");
    // The note says what the tool cuts.
    let notes = run(|ctx| strategy.passes_with_notes(&f.input(), None, &parameters, ctx))
        .unwrap()
        .notes;
    assert!(
        notes.iter().any(|n| n.contains("1.000 mm wide")),
        "{notes:?}"
    );
}

#[test]
fn req_op_08_a_width_the_tool_cannot_cut_is_refused() {
    let f = cylinder();
    let go = |extra: &[(&str, ParameterValue)]| {
        let strategy = EdgeFollowing::default();
        let parameters = strategy.schema().resolve(&values(extra)).unwrap();
        run(|ctx| strategy.passes(&f.input(), &parameters, ctx)).unwrap_err()
    };
    let e = go(&[(p::LINE_WIDTH, num(2.0))]);
    assert!(e.to_string().contains("wider than the tool"), "{e}");
    let e = go(&[(p::DEPTH, num(15.0))]);
    assert!(e.to_string().contains("flute length"), "{e}");
}

#[test]
fn req_op_08_depth_per_pass_makes_levels_ending_at_the_full_depth() {
    let f = cylinder();
    let out = passes(
        &f,
        &EdgeFollowing::default(),
        &values(&[(p::DEPTH, num(0.3)), (p::DEPTH_PER_PASS, num(0.1))]),
    );
    assert_eq!(out.len(), 6, "three levels on each of two chains");
    let level = |pass: &Pass| RADIUS - radius(pass.points[0].tip);
    // Each chain's levels are cut one after the other, shallowest first.
    for chain in out.chunks(3) {
        let depths: Vec<f64> = chain.iter().map(level).collect();
        for (d, want) in depths.iter().zip([0.1, 0.2, 0.3]) {
            assert!((d - want).abs() < 1e-9, "{depths:?}");
        }
    }
    // A step that does not divide the depth makes equal levels.
    let out = passes(
        &f,
        &EdgeFollowing::default(),
        &values(&[(p::DEPTH, num(0.3)), (p::DEPTH_PER_PASS, num(0.2))]),
    );
    assert_eq!(out.len(), 4);
    assert!((level(&out[0]) - 0.15).abs() < 1e-9);
    assert!((level(&out[1]) - 0.3).abs() < 1e-9);
}

#[test]
fn req_op_08_carving_cuts_lines_across_the_chain_to_the_carve_width() {
    let f = cylinder();
    // A 1 mm ball 0.2 mm deep cuts 0.8 mm wide, so a 3 mm band is lines out
    // to 1.1 mm either side, at most 0.5 mm apart: six.
    let out = passes(
        &f,
        &EdgeFollowing::default(),
        &values(&[
            (p::MODE, text("carve")),
            (p::DEPTH, num(0.2)),
            (p::CARVE_WIDTH, num(3.0)),
        ]),
    );
    assert_eq!(out.len(), 12, "six lines on each of two chains");
    let across: Vec<f64> = out
        .iter()
        .take(6)
        .map(|pass| pass.points[0].tip.x)
        .collect();
    let first = across[0];
    // The lines are 0.44 mm apart and symmetric about the chain's own x.
    let centre = 0.5 * (first + across[5]);
    assert!((across[5] - first - 2.2).abs() < 1e-9, "{across:?}");
    for pair in across.windows(2) {
        assert!((pair[1] - pair[0] - 0.44).abs() < 1e-9, "{across:?}");
    }
    assert!(
        centre.abs() < 1e-9 || (centre - LENGTH).abs() < 1e-9,
        "{centre}"
    );
    // A line beside the chain follows the surface: on the side with
    // cylinder under it the depth is the same.
    let inside = out
        .iter()
        .find(|pass| pass.points[0].tip.x > 0.5 && pass.points[0].tip.x < 1.2)
        .unwrap();
    for c in &inside.points {
        assert!((radius(c.tip) - (RADIUS - 0.2)).abs() < 0.003);
    }
    // A carve narrower than the line is refused.
    let strategy = EdgeFollowing::default();
    let parameters = strategy
        .schema()
        .resolve(&values(&[
            (p::MODE, text("carve")),
            (p::DEPTH, num(0.2)),
            (p::CARVE_WIDTH, num(0.5)),
        ]))
        .unwrap();
    let e = run(|ctx| strategy.passes(&f.input(), &parameters, ctx)).unwrap_err();
    assert!(e.to_string().contains("narrower than the"), "{e}");
    let parameters = strategy
        .schema()
        .resolve(&values(&[(p::MODE, text("carve"))]))
        .unwrap();
    let e = run(|ctx| strategy.passes(&f.input(), &parameters, ctx)).unwrap_err();
    assert!(e.to_string().contains("carve_width"), "{e}");
}

// ---- direction and start -------------------------------------------------

#[test]
fn req_op_08_chain_direction_and_start_point_are_controllable() {
    let f = cylinder();
    let strategy = EdgeFollowing::default();
    let first =
        |extra: &[(&str, ParameterValue)]| -> Vec<Pass> { passes(&f, &strategy, &values(extra)) };

    // Closed: start nearest an angle.
    for want in [0.0, 90.0, -90.0, 180.0] {
        let out = first(&[(p::START, text("angle")), (p::START_ANGLE, num(want))]);
        for pass in &out {
            let start = pass.points[0].tip;
            let diff = ((angle(start) - want + 540.0).rem_euclid(360.0) - 180.0).abs();
            assert!(diff < 1.0, "start at {} for {want}", angle(start));
            // A closed chain ends where it started.
            let end = pass.points[pass.points.len() - 1].tip;
            assert!(start.distance(end) < 1e-9);
        }
    }
    // Closed: lowest and highest X pick the chain end at that X; the ends of
    // the cylinder are two chains, so one starts at each.
    let xs = |out: &[Pass]| -> Vec<f64> { out.iter().map(|p| p.points[0].tip.x).collect() };
    // Reversed runs the other way round.
    let forward = first(&[]);
    let reversed = first(&[(p::START, text("reversed"))]);
    for (a, b) in forward.iter().zip(&reversed) {
        assert_eq!(a.points.len(), b.points.len());
        let mut back = b.points.clone();
        back.reverse();
        for (x, y) in a.points.iter().zip(&back) {
            assert!(x.tip.distance(y.tip) < 1e-9);
        }
    }
    assert_eq!(xs(&forward).len(), 2);

    // Open: low_x and high_x pick the end of a chain.
    let half = half_pipe();
    let open = |start: &str| -> Vec<Pass> {
        passes(&half, &strategy, &values(&[(p::START, text(start))]))
    };
    for (start, low) in [("low_x", true), ("high_x", false)] {
        for pass in open(start) {
            let (a, b) = (
                pass.points[0].tip.x,
                pass.points[pass.points.len() - 1].tip.x,
            );
            assert!(
                if low { a <= b + 1e-9 } else { a >= b - 1e-9 },
                "{start}: {a} {b}"
            );
        }
    }
    // Open: reversing swaps the ends, so the chain runs backwards.
    let a = open("first");
    let b = open("reversed");
    assert_eq!(a.len(), b.len());
    for pass in &a {
        let head = pass.points[0].tip;
        let tail = pass.points[pass.points.len() - 1].tip;
        assert!(
            b.iter().any(|q| {
                q.points[0].tip.distance(tail) < 1e-9
                    && q.points[q.points.len() - 1].tip.distance(head) < 1e-9
            }),
            "no reversed twin"
        );
    }
}

#[test]
fn req_op_08_chains_are_cut_from_the_lowest_x_nearest_first() {
    let f = cylinder();
    let out = passes(&f, &EdgeFollowing::default(), &values(&[]));
    let xs: Vec<f64> = out.iter().map(|p| p.points[0].tip.x).collect();
    assert!(
        xs[0].abs() < 1e-9 && (xs[1] - LENGTH).abs() < 1e-9,
        "{xs:?}"
    );
}

// ---- the registry, the schema and the generated path ---------------------

#[test]
fn req_op_16_edge_following_is_in_the_registry_with_uniform_parameters() {
    let registry = Registry::builtin();
    let strategy = registry.get("edge_following").unwrap();
    assert_eq!(strategy.name(), "edge_following");
    let schema = strategy.schema();
    for common in [
        names::TOLERANCE,
        names::STEPOVER,
        names::STOCK_TO_LEAVE,
        names::FEED,
        names::PLUNGE_FEED,
        names::RETRACT,
        names::ENTRY,
    ] {
        assert!(schema.get(common).is_some(), "{common}");
    }
    for own in [
        p::MODE,
        p::DEPTH,
        p::LINE_WIDTH,
        p::DEPTH_PER_PASS,
        p::CARVE_WIDTH,
        p::START,
        p::START_ANGLE,
        p::CREASE_ANGLE,
        p::MIN_LENGTH,
        p::MAX_STEP,
    ] {
        assert!(schema.get(own).is_some(), "{own}");
    }
    assert!(!strategy.honours_region());
    assert!(!strategy.needs_earlier_operations());
}

#[test]
fn req_op_08_the_generated_path_cuts_at_the_feed_with_no_rapid_cuts() {
    let f = cylinder();
    let path = run(|ctx| {
        generate(
            &EdgeFollowing::default(),
            &f.input(),
            &values(&[(p::DEPTH, num(0.3))]),
            ctx,
        )
    })
    .unwrap();
    let cutting = path
        .group
        .segments
        .iter()
        .filter(|s| s.kind == openchuck_types::ir::MoveKind::Cutting)
        .count();
    assert!(cutting > 300, "{cutting}");
    assert!(
        path.notes
            .iter()
            .any(|n| n.message.contains("2 chains (0 open, 2 closed)")),
        "{:?}",
        path.notes
    );
    // A on the whole path stays continuous: no jump of more than a few
    // degrees between neighbouring cutting poses.
    let mut last: Option<f64> = None;
    for s in &path.group.segments {
        if s.kind != openchuck_types::ir::MoveKind::Cutting {
            last = None;
            continue;
        }
        let a = s.end.a.0;
        if let Some(previous) = last {
            assert!((a - previous).abs() < 6.0, "A jumps from {previous} to {a}");
        }
        last = Some(a);
    }
}

#[test]
fn req_op_08_a_region_a_lead_entry_and_an_empty_part_are_refused_with_messages() {
    let f = cylinder();
    let e = run(|ctx| {
        generate(
            &EdgeFollowing::default(),
            &f.input(),
            &values(&[(names::ENTRY, text("ramp"))]),
            ctx,
        )
    })
    .unwrap_err();
    assert!(matches!(e, PathError::Parameter { .. }), "{e}");
    assert!(e.to_string().contains("plunge entry"), "{e}");
    // No edge turns this much.
    let e = run(|ctx| {
        generate(
            &EdgeFollowing::default(),
            &f.input(),
            &values(&[(p::CREASE_ANGLE, num(120.0))]),
            ctx,
        )
    })
    .unwrap_err();
    assert!(e.to_string().contains("no edge to follow"), "{e}");
    // Bad values.
    for (name, bad) in [
        (p::DEPTH, num(0.0)),
        (p::MAX_STEP, num(0.0)),
        (p::MODE, text("emboss")),
        (p::START, text("middle")),
        (p::CREASE_ANGLE, num(181.0)),
    ] {
        let e = run(|ctx| {
            generate(
                &EdgeFollowing::default(),
                &f.input(),
                &values(&[(name, bad)]),
                ctx,
            )
        })
        .unwrap_err();
        assert!(e.to_string().contains(name), "{name}: {e}");
    }
}

#[test]
fn req_op_08_a_chain_through_the_rotary_axis_is_refused() {
    #[derive(Debug)]
    struct Through;
    impl CurveSource for Through {
        fn chains(&self, _: &dyn SurfaceSource) -> openchuck_path::Result<Vec<EdgeChain>> {
            Ok(vec![EdgeChain {
                points: vec![Point3::new(5.0, 0.0, 0.0), Point3::new(6.0, 0.0, 0.0)],
                closed: false,
            }])
        }
    }
    let f = cylinder();
    let strategy = EdgeFollowing::with_source(std::sync::Arc::new(Through));
    let parameters = strategy.schema().resolve(&values(&[])).unwrap();
    let e = run(|ctx| strategy.passes(&f.input(), &parameters, ctx)).unwrap_err();
    assert!(e.to_string().contains("rotary axis"), "{e}");
}

#[test]
fn req_nfr_06_req_op_08_edge_following_is_identical_at_1_2_and_n_threads() {
    let f = half_pipe();
    let v = values(&[
        (p::MODE, text("carve")),
        (p::DEPTH, num(0.3)),
        (p::DEPTH_PER_PASS, num(0.15)),
        (p::CARVE_WIDTH, num(2.0)),
    ]);
    let runs = openchuck_testkit::determinism::check(|| {
        let path = run(|ctx| generate(&EdgeFollowing::default(), &f.input(), &v, ctx)).unwrap();
        format!("{:?}{:?}", path.group, path.notes).into_bytes()
    })
    .unwrap();
    assert_eq!(runs.len(), 4);
}

#[test]
fn req_op_08_a_custom_source_with_no_chains_does_not_blame_the_crease_angle() {
    #[derive(Debug)]
    struct Nothing;
    impl CurveSource for Nothing {
        fn chains(&self, _: &dyn SurfaceSource) -> openchuck_path::Result<Vec<EdgeChain>> {
            Ok(Vec::new())
        }
    }
    let f = cylinder();
    let strategy = EdgeFollowing::with_source(std::sync::Arc::new(Nothing));
    let parameters = strategy.schema().resolve(&values(&[])).unwrap();
    let e = run(|ctx| strategy.passes(&f.input(), &parameters, ctx))
        .unwrap_err()
        .to_string();
    assert!(e.contains("no edge to follow"), "{e}");
    assert!(!e.contains("crease_angle"), "{e}");
}

/// The segments of one end circle of `cylinder()`'s mesh (its first chain),
/// in an order that is not the chain's.
fn rim_segments(f: &Fixture) -> Vec<[Point3; 2]> {
    let chains = feature_chains(f.surface.tessellation(), 30.0);
    let rim = &chains[0];
    let mut segments: Vec<[Point3; 2]> = rim.points.windows(2).map(|w| [w[1], w[0]]).collect();
    segments.reverse();
    segments.rotate_left(17);
    segments
}

#[test]
fn req_op_08_chosen_mesh_edges_are_chained_and_cut_at_constant_depth() {
    let f = cylinder();
    let segments = rim_segments(&f);
    // The whole rim, given segment by segment out of order, is one closed chain.
    let rim = ChosenSegments::new(segments.clone()).unwrap();
    let chains = rim.chains(&f.surface).unwrap();
    assert_eq!(chains.len(), 1, "{chains:?}");
    assert!(chains[0].closed);
    assert_eq!(chains[0].points.len(), 181);

    // Part of it is one open chain, and only it is cut, at the depth asked for.
    let part: Vec<[Point3; 2]> = segments.iter().take(40).copied().collect();
    let chosen = EdgeFollowing::with_source(ChosenSegments::new(part).unwrap().shared());
    let depth = 0.3;
    let out = passes(&f, &chosen, &values(&[(p::DEPTH, num(depth))]));
    assert_eq!(out.len(), 1, "one pass along the chosen edges");
    for c in &out[0].points {
        let r = radius(c.tip);
        assert!((r - (RADIUS - depth)).abs() <= 0.01, "tip at radius {r}");
        assert!(c.tip.x.abs() < 1e-9 || (c.tip.x - LENGTH).abs() < 1e-9);
    }

    // The shortest chain drops it; a segment of no length or a point that is
    // not finite is refused.
    let short = ChosenSegments::new(segments.iter().take(2).copied().collect())
        .unwrap()
        .with_min_length(5.0);
    assert!(short.chains(&f.surface).unwrap().is_empty());
    let p0 = Point3::new(0.0, RADIUS, 0.0);
    assert!(ChosenSegments::new(vec![[p0, p0]]).is_err());
    assert!(ChosenSegments::new(vec![[p0, Point3::new(f64::NAN, 0.0, 0.0)]]).is_err());
}
