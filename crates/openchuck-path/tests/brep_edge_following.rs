// SPDX-License-Identifier: AGPL-3.0-or-later
//! Edge and curve following on B-rep edges (OP-08, NFR-06; work breakdown
//! slice 16 item 1; decision record 0053). The B-rep is built by hand, so no
//! kernel is involved; the STEP corpus part is in
//! `crates/openchuck-app/tests/brep_edges_step.rs` (`--features kernel-tests`).
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::cast_precision_loss
)]

mod support;

use std::collections::BTreeMap;
use std::f64::consts::TAU;

use openchuck_path::edge_following::parameter as p;
use openchuck_path::params::names;
use openchuck_path::{
    AcceptAll, AnalyticStock, BrepEdges, BrepSurface, CurveSource, EdgeFollowing, ParameterValue,
    Pass, RotaryMapping, Strategy, StrategyInput, SurfaceSource, generate,
};
use openchuck_types::brep::{
    BrepEdge, BrepFace, BrepModel, EdgeCurve, EdgeId, FaceId, Frame, Surface, TrimLoop, UvPoint,
};
use openchuck_types::geometry::{Point3, Vec3};
use openchuck_types::ir::PathTags;
use openchuck_types::machine::{Clearance, MachineProfile};
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::tool::profile::RevolvedProfile;
use support::{ball, run};

const R: f64 = 10.0;
const LENGTH: f64 = 20.0;

fn num(x: f64) -> ParameterValue {
    ParameterValue::Number(x)
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

// ---- a hand-built B-rep: a cylinder about the rotary (X) axis ---------------

fn p3(x: f64, y: f64, z: f64) -> Point3 {
    Point3::new(x, y, z)
}

/// A frame whose z axis is `z` and whose x axis is `x`; y is z cross x.
fn frame(o: Point3, x: Vec3, z: Vec3) -> Frame {
    Frame {
        origin: o,
        x,
        y: z.cross(x),
        z,
    }
}

fn face(
    id: u32,
    geometry: Surface,
    uv: [f64; 4],
    trims: Vec<Vec<(f64, f64)>>,
    tessellation: TriangleMesh,
) -> BrepFace {
    BrepFace {
        id: FaceId(id),
        surface: geometry.kind(),
        body: 0,
        edges: vec![],
        geometry,
        reversed: false,
        uv_bounds: uv,
        trims: trims
            .into_iter()
            .map(|l| TrimLoop(l.into_iter().map(|(u, v)| UvPoint { u, v }).collect()))
            .collect(),
        tessellation,
    }
}

/// The side of the cylinder from `x0` to `x1`: u runs round the axis from +Y
/// towards +Z, v along X.
fn side(id: u32, x0: f64, x1: f64, u1: f64) -> BrepFace {
    let o = p3(x0, 0.0, 0.0);
    let (nu, nv) = (192usize, 1usize);
    let mut tess = TriangleMesh::default();
    for i in 0..=nu {
        for j in 0..=nv {
            let u = u1 * i as f64 / nu as f64;
            tess.vertices
                .push(p3(x0 + (x1 - x0) * j as f64, R * u.cos(), R * u.sin()));
        }
    }
    let at = |i: usize, j: usize| u32::try_from(i * (nv + 1) + j).unwrap();
    for i in 0..nu {
        for j in 0..nv {
            // Counter-clockwise seen from outside.
            tess.triangles
                .push([at(i, j), at(i + 1, j + 1), at(i + 1, j)]);
            tess.triangles
                .push([at(i, j), at(i, j + 1), at(i + 1, j + 1)]);
        }
    }
    face(
        id,
        Surface::Cylinder {
            frame: frame(o, Vec3::new(0.0, 1.0, 0.0), Vec3::new(1.0, 0.0, 0.0)),
            radius: R,
        },
        [0.0, u1, 0.0, x1 - x0],
        vec![],
        tess,
    )
}

/// A disc of radius `R` at `x`, facing +X or -X.
fn cap(id: u32, x: f64, up: bool) -> BrepFace {
    let z = if up { 1.0 } else { -1.0 };
    let circle: Vec<(f64, f64)> = (0..96)
        .map(|i| {
            let a = TAU * f64::from(i) / 96.0;
            (R * a.cos(), R * a.sin())
        })
        .collect();
    let mut tess = TriangleMesh::default();
    tess.vertices.push(p3(x, 0.0, 0.0));
    for &(u, v) in circle.iter().step_by(2) {
        tess.vertices.push(p3(x, u, z * v));
    }
    for i in 0..48u32 {
        let (a, b) = (1 + i, 1 + (i + 1) % 48);
        tess.triangles.push(if up { [0, a, b] } else { [0, b, a] });
    }
    face(
        id,
        Surface::Plane {
            frame: frame(
                p3(x, 0.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(z, 0.0, 0.0),
            ),
        },
        [-R, R, -R, R],
        vec![circle],
        tess,
    )
}

fn arc(x: f64, from: f64, to: f64, steps: usize) -> Vec<Point3> {
    (0..=steps)
        .map(|i| {
            let a = from + (to - from) * i as f64 / steps as f64;
            p3(x, R * a.cos(), R * a.sin())
        })
        .collect()
}

fn edge(id: u32, faces: &[u32], polyline: Vec<Point3>) -> BrepEdge {
    BrepEdge {
        id: EdgeId(id),
        faces: faces.iter().map(|f| FaceId(*f)).collect(),
        vertices: None,
        curve: None,
        polyline,
    }
}

/// The capped cylinder: side (face 0), a cap at each end (faces 1 and 2), the
/// two rims (edges 0 and 1) and the seam along the side (edge 2).
fn capped_cylinder() -> BrepModel {
    BrepModel {
        faces: vec![
            side(0, 0.0, LENGTH, TAU),
            cap(1, 0.0, false),
            cap(2, LENGTH, true),
        ],
        edges: vec![
            edge(0, &[0, 1], arc(0.0, 0.0, TAU, 192)),
            edge(1, &[0, 2], arc(LENGTH, 0.0, TAU, 192)),
            edge(2, &[0], vec![p3(0.0, R, 0.0), p3(LENGTH, R, 0.0)]),
        ],
        ..BrepModel::default()
    }
}

/// Two halves of the side meeting tangentially at X = 10, closed by two caps.
fn split_cylinder() -> BrepModel {
    BrepModel {
        faces: vec![
            side(0, 0.0, 10.0, TAU),
            side(3, 10.0, LENGTH, TAU),
            cap(1, 0.0, false),
            cap(2, LENGTH, true),
        ],
        edges: vec![
            edge(0, &[0, 1], arc(0.0, 0.0, TAU, 192)),
            edge(1, &[3, 2], arc(LENGTH, 0.0, TAU, 192)),
            edge(2, &[0, 3], arc(10.0, 0.0, TAU, 192)),
            edge(3, &[0], vec![p3(0.0, R, 0.0), p3(10.0, R, 0.0)]),
            edge(4, &[3], vec![p3(10.0, R, 0.0), p3(LENGTH, R, 0.0)]),
        ],
        ..BrepModel::default()
    }
}

/// A line along the side that is not a seam: a split line a quarter turn round,
/// from X = 2 to X = 18, which is one face's own curve.
fn line_on_side(id: u32) -> BrepEdge {
    let a = TAU / 4.0;
    edge(
        id,
        &[0],
        vec![
            p3(2.0, R * a.cos(), R * a.sin()),
            p3(18.0, R * a.cos(), R * a.sin()),
        ],
    )
}

/// A helix on the side, one and a half turns from X = 2 to X = 18.
fn helix(id: u32) -> BrepEdge {
    let n = 1080;
    edge(
        id,
        &[0],
        (0..=n)
            .map(|i| {
                let t = f64::from(i) / f64::from(n);
                let a = 1.5 * TAU * t;
                p3(2.0 + 16.0 * t, R * a.cos(), R * a.sin())
            })
            .collect(),
    )
}

fn chains_of(model: &BrepModel, source: &BrepEdges) -> Vec<openchuck_path::EdgeChain> {
    let surface = BrepSurface::new(model).unwrap();
    source.chains(&surface).unwrap()
}

// ---- chains ------------------------------------------------------------------

#[test]
fn req_op_08_brep_the_rims_of_a_cylinder_are_two_closed_chains_and_the_seam_is_no_edge() {
    let model = capped_cylinder();
    let source = BrepEdges::new(&model).unwrap();
    assert_eq!(source.edge_ids(), vec![EdgeId(0), EdgeId(1)], "no seam");
    let chains = chains_of(&model, &source);
    assert_eq!(chains.len(), 2);
    for (chain, x) in chains.iter().zip([0.0, LENGTH]) {
        assert!(chain.closed);
        assert_eq!(chain.points.first(), chain.points.last());
        assert!(chain.points.iter().all(|q| (q.x - x).abs() < 1e-12));
    }
    // The seam is no edge even when every tangent edge is kept.
    let all = BrepEdges::new(&model).unwrap().with_crease(0.0);
    assert_eq!(all.edge_ids(), vec![EdgeId(0), EdgeId(1)]);
}

#[test]
fn req_op_08_brep_a_tangent_edge_is_not_an_edge_unless_the_crease_angle_is_zero() {
    let model = split_cylinder();
    let source = BrepEdges::new(&model).unwrap();
    // The join at X = 10 is tangent (0 degrees); the rims turn 90; no seams.
    assert_eq!(source.edge_ids(), vec![EdgeId(0), EdgeId(1)]);
    let all = BrepEdges::new(&model).unwrap().with_crease(0.0);
    assert_eq!(all.edge_ids(), vec![EdgeId(0), EdgeId(1), EdgeId(2)]);
    assert_eq!(chains_of(&model, &all).len(), 3);
    // A crease angle above the rims' 90 degrees leaves nothing.
    let none = BrepEdges::new(&model).unwrap().with_crease(95.0);
    assert!(none.edge_ids().is_empty());
}

#[test]
fn req_op_08_brep_edges_join_into_open_and_closed_chains_and_stop_where_three_meet() {
    let a = p3(0.0, 0.0, 0.0);
    let b = p3(5.0, 0.0, 0.0);
    let c = p3(5.0, 5.0, 0.0);
    let d = p3(0.0, 5.0, 0.0);
    let e = p3(10.0, 0.0, 0.0);
    let f = p3(5.0, -5.0, 0.0);
    let model = BrepModel {
        // Edges given out of order and some reversed: a-b, c-b (reversed),
        // c-d, then a spur b-e, b-f meeting at b with a-b and c-b (degree 4),
        // and a separate loop of two half circles.
        edges: vec![
            edge(0, &[], vec![a, b]),
            edge(1, &[], vec![c, b]),
            edge(2, &[], vec![c, d]),
            edge(3, &[], vec![b, e]),
            edge(4, &[], vec![b, f]),
            edge(5, &[], arc(30.0, 0.0, std::f64::consts::PI, 8)),
            edge(6, &[], arc(30.0, std::f64::consts::PI, TAU, 8)),
        ],
        ..BrepModel::default()
    };
    let source = BrepEdges::new(&model).unwrap();
    let surface = BrepSurface::new(&capped_cylinder()).unwrap();
    let chains = source.chains(&surface).unwrap();
    // b has four edges, so a-b, b-e and b-f are single-edge chains. c has two
    // edges (c-b, c-d), so they join into d-c-b.
    let open: Vec<_> = chains.iter().filter(|c| !c.closed).collect();
    let closed: Vec<_> = chains.iter().filter(|c| c.closed).collect();
    assert_eq!(closed.len(), 1, "the two half circles close");
    assert_eq!(closed[0].points.first(), closed[0].points.last());
    assert_eq!(open.len(), 4);
    let joined = open.iter().find(|c| c.points.len() == 3).unwrap();
    // d-c-b or b-c-d, joined through c and not through b.
    assert!(joined.points.contains(&c) && joined.points.contains(&d) && joined.points.contains(&b));
    assert_eq!(joined.points[1], c);
    // Deterministic: the same call gives the same chains.
    assert_eq!(chains, source.chains(&surface).unwrap());
}

#[test]
fn req_op_08_brep_short_chains_are_dropped_by_the_shortest_length() {
    let model = BrepModel {
        edges: vec![
            edge(0, &[], vec![p3(0.0, 0.0, 0.0), p3(1.0, 0.0, 0.0)]),
            edge(1, &[], vec![p3(0.0, 5.0, 0.0), p3(8.0, 5.0, 0.0)]),
        ],
        ..BrepModel::default()
    };
    let surface = BrepSurface::new(&capped_cylinder()).unwrap();
    let all = BrepEdges::new(&model).unwrap();
    assert_eq!(all.chains(&surface).unwrap().len(), 2);
    let long = BrepEdges::new(&model).unwrap().with_min_length(2.0);
    let chains = long.chains(&surface).unwrap();
    assert_eq!(chains.len(), 1);
    assert!((chains[0].length() - 8.0).abs() < 1e-12);
}

#[test]
fn req_op_08_brep_chosen_edges_are_the_only_ones_followed_and_a_bad_id_is_refused() {
    let model = split_cylinder();
    let source = BrepEdges::with_edges(&model, &[EdgeId(1)]).unwrap();
    assert_eq!(source.edge_ids(), vec![EdgeId(1)]);
    let e = BrepEdges::with_edges(&model, &[EdgeId(99)])
        .unwrap_err()
        .to_string();
    assert!(e.contains("edge 99"), "{e}");
    // A seam alone is nothing to follow.
    let e = BrepEdges::with_edges(&model, &[EdgeId(3)])
        .unwrap_err()
        .to_string();
    assert!(e.contains("seams"), "{e}");
    let bad = BrepEdges::new(&model).unwrap().with_crease(f64::NAN);
    let surface = BrepSurface::new(&model).unwrap();
    assert!(bad.chains(&surface).is_err());
}

// ---- the strategy on the exact surface ----------------------------------------

struct Fixture {
    surface: BrepSurface,
    stock: AnalyticStock,
    tool: RevolvedProfile,
    rotary: RotaryMapping,
    clearance: Clearance,
    tags: PathTags,
    machine: MachineProfile,
}

impl Fixture {
    fn new(model: &BrepModel) -> Self {
        let machine = MachineProfile::example_x_rotary();
        let surface = BrepSurface::new(model).unwrap();
        let b = surface.bounds();
        Self {
            surface,
            stock: AnalyticStock::Cylinder {
                radius: R + 1.0,
                x_start: b.min.x - 1.0,
                x_end: b.max.x + 1.0,
            },
            tool: ball(1.0).profile().unwrap(),
            rotary: RotaryMapping::of_profile(&machine),
            clearance: Clearance {
                radius: openchuck_types::units::Millimetres(R + 6.0),
                safe_z: openchuck_types::units::Millimetres(R + 16.0),
            },
            tags: support::tags(),
            machine,
        }
    }

    fn input(&self) -> StrategyInput<'_> {
        StrategyInput {
            surface: &self.surface,
            stock: &self.stock,
            collision: &AcceptAll,
            tool: &self.tool,
            rotary: self.rotary,
            clearance: self.clearance,
            max_feeds: self.machine.max_feeds,
            tags: &self.tags,
            keep_out: &[],
        }
    }
}

fn passes(
    f: &Fixture,
    strategy: &EdgeFollowing,
    v: &BTreeMap<String, ParameterValue>,
) -> Vec<Pass> {
    let parameters = strategy.schema().resolve(v).unwrap();
    run(|ctx| {
        strategy
            .passes_with_notes(&f.input(), None, &parameters, ctx)
            .map(|o| o.passes)
    })
    .unwrap()
}

fn radius(q: Point3) -> f64 {
    q.y.hypot(q.z)
}

#[test]
fn req_op_08_brep_engraved_depth_is_constant_on_a_curved_surface_closed_chains() {
    let model = capped_cylinder();
    let f = Fixture::new(&model);
    let depth = 0.3;
    let strategy = EdgeFollowing::with_source(BrepEdges::new(&model).unwrap().shared());
    let out = passes(&f, &strategy, &values(&[(p::DEPTH, num(depth))]));
    assert_eq!(out.len(), 2, "one pass round each rim");
    for pass in &out {
        assert!(pass.points.len() > 100);
        for c in &pass.points {
            assert!(
                (radius(c.tip) - (R - depth)).abs() <= 0.01,
                "tip at radius {}",
                radius(c.tip)
            );
        }
    }
}

#[test]
fn req_op_08_brep_engraved_depth_is_constant_along_an_open_helix_on_a_curved_surface() {
    let mut model = capped_cylinder();
    model.edges.push(helix(3));
    let f = Fixture::new(&model);
    let depth = 0.25;
    let source = BrepEdges::with_edges(&model, &[EdgeId(3)]).unwrap();
    let chains = chains_of(&model, &source);
    assert_eq!(chains.len(), 1);
    assert!(!chains[0].closed);
    let strategy = EdgeFollowing::with_source(source.shared());
    let out = passes(
        &f,
        &strategy,
        &values(&[(p::DEPTH, num(depth)), (p::MAX_STEP, num(0.4))]),
    );
    assert_eq!(out.len(), 1);
    let pass = &out[0];
    let mut turned = 0.0_f64;
    for (c, d) in pass.points.iter().zip(pass.points.iter().skip(1)) {
        assert!(
            (radius(c.tip) - (R - depth)).abs() <= 0.01,
            "tip at radius {}",
            radius(c.tip)
        );
        turned += (d.a.0 - c.a.0).abs();
    }
    assert!(
        turned > 500.0,
        "the helix winds round the axis: {turned} degrees of A"
    );
    // The tip is on the helix: its axial position runs from X = 2 to 18.
    let (lo, hi) = pass.points.iter().fold((f64::MAX, f64::MIN), |m, c| {
        (m.0.min(c.tip.x), m.1.max(c.tip.x))
    });
    assert!(
        (lo - 2.0).abs() < 1e-6 && (hi - 18.0).abs() < 1e-6,
        "{lo} {hi}"
    );
}

#[test]
fn req_op_08_brep_an_edge_that_is_not_a_seam_on_a_curved_face_is_followed() {
    let mut model = capped_cylinder();
    model.edges.push(line_on_side(3));
    let source = BrepEdges::new(&model).unwrap();
    assert_eq!(source.edge_ids(), vec![EdgeId(0), EdgeId(1), EdgeId(3)]);
}

#[test]
fn req_op_08_brep_a_part_with_no_real_edge_says_so() {
    let model = capped_cylinder();
    let f = Fixture::new(&model);
    let strategy =
        EdgeFollowing::with_source(BrepEdges::new(&model).unwrap().with_crease(95.0).shared());
    let parameters = strategy.schema().resolve(&values(&[])).unwrap();
    let e = run(|ctx| strategy.passes(&f.input(), &parameters, ctx))
        .unwrap_err()
        .to_string();
    assert!(e.contains("no edge to follow"), "{e}");
}

#[test]
fn req_nfr_06_req_op_08_brep_edge_following_is_identical_at_1_2_and_n_threads() {
    let mut model = capped_cylinder();
    model.edges.push(helix(3));
    let f = Fixture::new(&model);
    let strategy = EdgeFollowing::with_source(BrepEdges::new(&model).unwrap().shared());
    let v = values(&[(p::DEPTH, num(0.3)), (p::DEPTH_PER_PASS, num(0.15))]);
    let runs = openchuck_testkit::determinism::check(|| {
        let path = run(|ctx| generate(&strategy, &f.input(), &v, ctx)).unwrap();
        format!("{:?}{:?}", path.group, path.notes).into_bytes()
    })
    .unwrap();
    assert_eq!(runs.len(), 4);
}

// ---- the exact edge curve (decision 0227, OI-229) -----------------------------

/// A circle edge about the X axis at `x`, the exact curve of `arc`.
fn rim_curve(x: f64) -> EdgeCurve {
    EdgeCurve::Circle {
        frame: frame(
            p3(x, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
        ),
        radius: R,
        first: 0.0,
        last: TAU,
    }
}

/// The capped cylinder whose rims the kernel tessellated into only `pieces`
/// chords (a coarse model), with or without the rims' exact curve.
fn coarse_cylinder(pieces: usize, exact: bool) -> BrepModel {
    let mut model = capped_cylinder();
    for (id, x) in [(0_usize, 0.0), (1, LENGTH)] {
        model.edges[id].polyline = arc(x, 0.0, TAU, pieces);
        model.edges[id].curve = exact.then(|| rim_curve(x));
    }
    model
}

/// A closed chain repeats its first point last, so only that differs.
fn assert_polyline_kept(chain: &openchuck_path::EdgeChain, polyline: &[Point3]) {
    let n = polyline.len();
    assert_eq!(chain.points.len(), n);
    assert_eq!(chain.points[..n - 1], polyline[..n - 1]);
    assert_eq!(chain.points[n - 1], polyline[0]);
}

fn widest_gap(points: &[Point3]) -> f64 {
    points
        .iter()
        .map(|q| (radius(*q) - R).abs())
        .fold(0.0, f64::max)
}

#[test]
fn req_op_08_brep_a_chain_on_an_edge_with_an_exact_curve_lies_on_the_curve() {
    let model = coarse_cylinder(24, true);
    let chains = chains_of(&model, &BrepEdges::new(&model).unwrap());
    assert_eq!(chains.len(), 2);
    for chain in &chains {
        assert!(chain.closed);
        assert!(
            widest_gap(&chain.points) < 1e-9,
            "every point is on the rim"
        );
        // Chords sag no more than the sampling limit below the circle.
        let sag = chain
            .points
            .iter()
            .zip(chain.points.iter().skip(1))
            .map(|(a, b)| R - radius(p3((a.x + b.x) / 2.0, (a.y + b.y) / 2.0, (a.z + b.z) / 2.0)))
            .fold(0.0, f64::max);
        assert!(sag <= openchuck_path::edge_curve::SAG + 1e-12, "sag {sag}");
    }
}

#[test]
fn req_op_08_brep_an_edge_without_an_exact_curve_keeps_the_kernels_polyline() {
    let model = coarse_cylinder(24, false);
    let chains = chains_of(&model, &BrepEdges::new(&model).unwrap());
    assert_polyline_kept(&chains[0], &model.edges[0].polyline);
    // The chords of the coarse polyline cut inside the rim by up to 0.0856.
    let sag = R * (1.0 - (TAU / 48.0).cos());
    assert!(sag > 0.08);
}

#[test]
fn req_op_08_brep_a_curve_that_does_not_meet_its_edge_is_ignored() {
    let mut model = coarse_cylinder(24, true);
    // The curve now ends a millimetre away from the polyline's ends.
    model.edges[0].curve = Some(EdgeCurve::Circle {
        frame: frame(
            p3(0.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
        ),
        radius: R,
        first: 0.1,
        last: TAU,
    });
    let chains = chains_of(&model, &BrepEdges::new(&model).unwrap());
    assert_polyline_kept(&chains[0], &model.edges[0].polyline);
}

#[test]
fn req_op_08_brep_engraved_depth_is_exact_on_a_coarsely_tessellated_curved_edge() {
    let depth = 0.3;
    let tips = |exact: bool| -> f64 {
        let model = coarse_cylinder(24, exact);
        let f = Fixture::new(&model);
        let strategy = EdgeFollowing::with_source(BrepEdges::new(&model).unwrap().shared());
        let out = passes(&f, &strategy, &values(&[(p::DEPTH, num(depth))]));
        assert_eq!(out.len(), 2);
        out.iter()
            .flat_map(|pass| pass.points.iter())
            .map(|c| (radius(c.tip) - (R - depth)).abs())
            .fold(0.0, f64::max)
    };
    let with_curve = tips(true);
    let without = tips(false);
    assert!(
        with_curve <= 0.002,
        "depth error with the exact curve {with_curve}"
    );
    assert!(
        without > 0.05,
        "the coarse polyline's chords should show as depth error: {without}"
    );
}

#[test]
fn req_nfr_06_req_op_08_brep_exact_edge_following_is_identical_at_1_2_and_n_threads() {
    let model = coarse_cylinder(24, true);
    let f = Fixture::new(&model);
    let strategy = EdgeFollowing::with_source(BrepEdges::new(&model).unwrap().shared());
    let v = values(&[(p::DEPTH, num(0.3))]);
    let runs = openchuck_testkit::determinism::check(|| {
        let path = run(|ctx| generate(&strategy, &f.input(), &v, ctx)).unwrap();
        format!("{:?}{:?}", path.group, path.notes).into_bytes()
    })
    .unwrap();
    assert_eq!(runs.len(), 4);
}
