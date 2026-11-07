// SPDX-License-Identifier: AGPL-3.0-or-later
//! Face-flow finishing (OP-28; decision record 0525; lane FACEFLOW).
//!
//! The parts are built by hand as B-rep models with their edges and
//! vertices, so no STEP kernel is involved; the STEP blisk is the
//! application's reference job. Each check is made against the closed form
//! of the part (the plane, the bilinear patch, the cylinder), never with the
//! pathing library's own evaluators.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::too_many_lines
)]

mod support;

use std::collections::BTreeMap;
use std::f64::consts::{FRAC_PI_3, PI};
use std::sync::Arc;

use openchuck_path::face_flow::{self, FaceFlow, parameter as p};
use openchuck_path::params::names;
use openchuck_path::{
    AnalyticStock, CutPoint, Pass, RegionConstraint, RotaryMapping, Strategy, StrategyInput,
    StrategyOutput, UnreachedReason,
};
use openchuck_types::brep::{
    BSplinePatch, BrepEdge, BrepFace, BrepModel, BrepVertex, EdgeCurve, EdgeId, FaceId, Frame,
    Surface, TrimLoop, UvPoint, VertexId,
};
use openchuck_types::geometry::{Point3, Vec3};
use openchuck_types::machine::Clearance;
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::tool::Tool;
use openchuck_types::units::Millimetres;
use support::{ball, machine, run, tags};

// ---- values -------------------------------------------------------------------

fn num(x: f64) -> openchuck_path::ParameterValue {
    openchuck_path::ParameterValue::Number(x)
}

fn text(s: &str) -> openchuck_path::ParameterValue {
    openchuck_path::ParameterValue::Text(s.to_owned())
}

fn values(
    stepover: f64,
    extra: &[(&str, openchuck_path::ParameterValue)],
) -> BTreeMap<String, openchuck_path::ParameterValue> {
    let mut v: BTreeMap<String, openchuck_path::ParameterValue> = [
        (names::STEPOVER, stepover),
        (names::FEED, 300.0),
        (names::PLUNGE_FEED, 100.0),
        (names::TOLERANCE, 0.005),
    ]
    .into_iter()
    .map(|(k, x)| (k.to_owned(), num(x)))
    .collect();
    for (k, x) in extra {
        v.insert((*k).to_owned(), x.clone());
    }
    v
}

// ---- a hand-built B-rep with edges and vertices ----------------------------------

fn p3(x: f64, y: f64, z: f64) -> Point3 {
    Point3::new(x, y, z)
}

/// A model under construction: faces with their edges, edges with their
/// vertices and curves.
#[derive(Default)]
struct Builder {
    model: BrepModel,
}

impl Builder {
    fn vertex(&mut self, p: Point3) -> VertexId {
        if let Some(v) = self
            .model
            .vertices
            .iter()
            .find(|v| v.point.distance(p) < 1e-9)
        {
            return v.id;
        }
        let id = VertexId(self.model.vertices.len() as u32);
        self.model.vertices.push(BrepVertex { id, point: p });
        id
    }

    /// The edge along `curve` from t 0 to 1, sampled finely, between the
    /// faces `faces`; a straight edge gets its exact line.
    fn edge(&mut self, faces: &[u32], curve: &dyn Fn(f64) -> Point3, straight: bool) -> EdgeId {
        let n = if straight { 1 } else { 256 };
        let polyline: Vec<Point3> = (0..=n)
            .map(|i| curve(f64::from(i) / f64::from(n)))
            .collect();
        let (a, b) = (polyline[0], polyline[polyline.len() - 1]);
        let id = EdgeId(self.model.edges.len() as u32);
        let vertices = Some([self.vertex(a), self.vertex(b)]);
        let exact = straight.then(|| {
            let d = b - a;
            let len = d.length();
            EdgeCurve::Line {
                origin: a,
                direction: d * (1.0 / len),
                first: 0.0,
                last: len,
            }
        });
        self.model.edges.push(BrepEdge {
            id,
            faces: faces.iter().map(|f| FaceId(*f)).collect(),
            vertices,
            polyline,
            curve: exact,
        });
        id
    }

    /// Adds face `id` (its position in the model) over `uv`, with the trim
    /// loop `trim` in (u, v) when it is not the whole box, bounded by
    /// `edges`, tessellated from `eval` where `inside`.
    #[allow(clippy::too_many_arguments)]
    fn face(
        &mut self,
        geometry: Surface,
        uv: [f64; 4],
        trim: Option<Vec<(f64, f64)>>,
        edges: Vec<EdgeId>,
        eval: &dyn Fn(f64, f64) -> Point3,
        inside: &dyn Fn(f64, f64) -> bool,
        reversed: bool,
    ) -> FaceId {
        let id = FaceId(self.model.faces.len() as u32);
        let (nu, nv) = (64usize, 64usize);
        let mut tess = TriangleMesh::default();
        for i in 0..=nu {
            for j in 0..=nv {
                let u = uv[0] + (uv[1] - uv[0]) * i as f64 / nu as f64;
                let v = uv[2] + (uv[3] - uv[2]) * j as f64 / nv as f64;
                tess.vertices.push(eval(u, v));
            }
        }
        let at = |i: usize, j: usize| u32::try_from(i * (nv + 1) + j).unwrap();
        for i in 0..nu {
            for j in 0..nv {
                let u = uv[0] + (uv[1] - uv[0]) * (i as f64 + 0.5) / nu as f64;
                let v = uv[2] + (uv[3] - uv[2]) * (j as f64 + 0.5) / nv as f64;
                if !inside(u, v) {
                    continue;
                }
                let (t1, t2) = (
                    [at(i, j), at(i + 1, j), at(i + 1, j + 1)],
                    [at(i, j), at(i + 1, j + 1), at(i, j + 1)],
                );
                if reversed {
                    tess.triangles.push([t1[0], t1[2], t1[1]]);
                    tess.triangles.push([t2[0], t2[2], t2[1]]);
                } else {
                    tess.triangles.push(t1);
                    tess.triangles.push(t2);
                }
            }
        }
        self.model.faces.push(BrepFace {
            id,
            surface: geometry.kind(),
            body: 0,
            edges,
            geometry,
            reversed,
            uv_bounds: uv,
            trims: trim
                .map(|t| {
                    vec![TrimLoop(
                        t.into_iter().map(|(u, v)| UvPoint { u, v }).collect(),
                    )]
                })
                .unwrap_or_default(),
            tessellation: tess,
        });
        id
    }
}

fn frame(o: Point3, x: Vec3, y: Vec3) -> Frame {
    Frame {
        origin: o,
        x,
        y,
        z: x.cross(y),
    }
}

/// A flat polygon at height `z`, facing +Z, with `corners` in (x, y)
/// counter-clockwise seen from above. The plane's (u, v) are X and Y. Its
/// edges are the polygon's sides in order; a side given in `shared` is
/// that existing edge, which this face then also bounds.
fn flat(b: &mut Builder, z: f64, corners: &[(f64, f64)], shared: &[Option<u32>]) -> FaceId {
    let id = b.model.faces.len() as u32;
    let mut edges = Vec::new();
    for (i, &(x0, y0)) in corners.iter().enumerate() {
        let (x1, y1) = corners[(i + 1) % corners.len()];
        if let Some(e) = shared.get(i).copied().flatten() {
            b.model.edges[e as usize].faces.push(FaceId(id));
            edges.push(EdgeId(e));
            continue;
        }
        edges.push(b.edge(
            &[id],
            &|t| p3(x0 + (x1 - x0) * t, y0 + (y1 - y0) * t, z),
            true,
        ));
    }
    let xs = corners.iter().map(|c| c.0);
    let ys = corners.iter().map(|c| c.1);
    let uv = [
        xs.clone().fold(f64::INFINITY, f64::min),
        xs.fold(f64::NEG_INFINITY, f64::max),
        ys.clone().fold(f64::INFINITY, f64::min),
        ys.fold(f64::NEG_INFINITY, f64::max),
    ];
    let polygon: Vec<(f64, f64)> = corners.to_vec();
    let inside = move |u: f64, v: f64| inside_polygon(&polygon, u, v);
    b.face(
        Surface::Plane {
            frame: frame(
                p3(0.0, 0.0, z),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
            ),
        },
        uv,
        Some(corners.to_vec()),
        edges,
        &|u, v| p3(u, v, z),
        &inside,
        false,
    )
}

fn inside_polygon(polygon: &[(f64, f64)], u: f64, v: f64) -> bool {
    let mut inside = false;
    for (i, &(x0, y0)) in polygon.iter().enumerate() {
        let (x1, y1) = polygon[(i + 1) % polygon.len()];
        if (y0 > v) != (y1 > v) && u < x0 + (v - y0) / (y1 - y0) * (x1 - x0) {
            inside = !inside;
        }
    }
    inside
}

/// A wall: the rectangle from `a` to `b` along the floor and `height` up
/// in +Z, facing `facing`, sharing its bottom edge `floor_edge` with the
/// floor. Its own top and end edges are added.
fn wall(
    b: &mut Builder,
    a: Point3,
    c: Point3,
    height: f64,
    facing: Vec3,
    floor_edge: EdgeId,
) -> FaceId {
    let id = b.model.faces.len() as u32;
    b.model.edges[floor_edge.0 as usize].faces.push(FaceId(id));
    let up = Vec3::new(0.0, 0.0, 1.0);
    let len = (c - a).length();
    let along = (c - a) * (1.0 / len);
    // u along the floor, v up: the normal is along x cross up, which must
    // be `facing`; otherwise run u the other way.
    let (origin, x) = if along.cross(up).dot(facing) > 0.0 {
        (a, along)
    } else {
        (c, -along)
    };
    let top = b.edge(&[id], &|t| origin + x * (len * t) + up * height, true);
    let e0 = b.edge(&[id], &|t| a + up * (height * t), true);
    let e1 = b.edge(&[id], &|t| c + up * (height * t), true);
    b.face(
        Surface::Plane {
            frame: frame(origin, x, up),
        },
        [0.0, len, 0.0, height],
        None,
        vec![floor_edge, top, e0, e1],
        &|u, v| origin + x * u + up * v,
        &|_, _| true,
        false,
    )
}

/// A twisted ruled face: the bilinear patch through four corners over X
/// from 0 to `length` and Y from -`half` to `half`, with heights `z` at
/// (0, -half), (length, -half), (0, half), (length, half). Every line of
/// constant X and of constant Y on it is straight. Returns the model and
/// the closed form height over (x, y).
fn twisted(length: f64, half: f64, z: [f64; 4]) -> (BrepModel, impl Fn(f64, f64) -> f64) {
    let height = move |x: f64, y: f64| {
        let (s, t) = (x / length, (y + half) / (2.0 * half));
        z[0] * (1.0 - s) * (1.0 - t) + z[1] * s * (1.0 - t) + z[2] * (1.0 - s) * t + z[3] * s * t
    };
    let point = move |u: f64, v: f64| {
        let (x, y) = (u * length, -half + 2.0 * half * v);
        p3(x, y, height(x, y))
    };
    let patch = BSplinePatch {
        degree_u: 1,
        degree_v: 1,
        knots_u: vec![0.0, 0.0, 1.0, 1.0],
        knots_v: vec![0.0, 0.0, 1.0, 1.0],
        poles_u: 2,
        poles_v: 2,
        // v fastest: (u0 v0), (u0 v1), (u1 v0), (u1 v1).
        poles: vec![
            point(0.0, 0.0),
            point(0.0, 1.0),
            point(1.0, 0.0),
            point(1.0, 1.0),
        ],
        weights: vec![1.0; 4],
    };
    let mut b = Builder::default();
    let edges = vec![
        b.edge(&[0], &|t| point(t, 0.0), true),
        b.edge(&[0], &|t| point(1.0, t), true),
        b.edge(&[0], &|t| point(1.0 - t, 1.0), true),
        b.edge(&[0], &|t| point(0.0, 1.0 - t), true),
    ];
    b.face(
        Surface::BSpline(patch),
        [0.0, 1.0, 0.0, 1.0],
        None,
        edges,
        &point,
        &|_, _| true,
        false,
    );
    (b.model, height)
}

/// A patch of the cylinder of `radius` about the model X axis, round from
/// angle `from` to `to` (radians from +Y towards +Z) and along X from 0 to
/// `length`, facing away from the axis. Edges: the line at `from` (0), the
/// arc at X `length` (1), the line at `to` (2), the arc at X 0 (3).
fn cylinder_patch(radius: f64, from: f64, to: f64, length: f64) -> BrepModel {
    offset_cylinder(0.0, radius, from, to, length)
}

/// As [`cylinder_patch`], about the line parallel to X through (0, 0,
/// `centre`).
fn offset_cylinder(centre: f64, radius: f64, from: f64, to: f64, length: f64) -> BrepModel {
    let point = move |u: f64, v: f64| p3(v, radius * u.cos(), centre + radius * u.sin());
    let mut b = Builder::default();
    let edges = vec![
        b.edge(&[0], &|t| point(from, length * t), true),
        b.edge(&[0], &|t| point(from + (to - from) * t, length), false),
        b.edge(&[0], &|t| point(to, length * (1.0 - t)), true),
        b.edge(&[0], &|t| point(to - (to - from) * t, 0.0), false),
    ];
    b.face(
        Surface::Cylinder {
            frame: Frame {
                origin: p3(0.0, 0.0, centre),
                x: Vec3::new(0.0, 1.0, 0.0),
                y: Vec3::new(0.0, 0.0, 1.0),
                z: Vec3::new(1.0, 0.0, 0.0),
            },
            radius,
        },
        [from, to, 0.0, length],
        None,
        edges,
        &point,
        &|_, _| true,
        false,
    );
    b.model
}

/// A floor at Z 10 from X 0 to 30 and Y -`half` to `half`, between two
/// walls 6 mm high along its long sides. Face 0 is the floor; its edges are
/// 0 (Y -half), 1 (the end at X 30), 2 (Y half) and 3 (the end at X 0).
fn channel(half: f64) -> BrepModel {
    let mut b = Builder::default();
    flat(
        &mut b,
        10.0,
        &[(0.0, -half), (30.0, -half), (30.0, half), (0.0, half)],
        &[],
    );
    wall(
        &mut b,
        p3(0.0, -half, 10.0),
        p3(30.0, -half, 10.0),
        6.0,
        Vec3::new(0.0, 1.0, 0.0),
        EdgeId(0),
    );
    wall(
        &mut b,
        p3(30.0, half, 10.0),
        p3(0.0, half, 10.0),
        6.0,
        Vec3::new(0.0, -1.0, 0.0),
        EdgeId(2),
    );
    b.model
}

// ---- running the strategy ---------------------------------------------------------

struct Rig {
    model: Arc<BrepModel>,
    surface: openchuck_path::BrepSurface,
    stock: AnalyticStock,
    tool: openchuck_types::tool::profile::RevolvedProfile,
    rotary: RotaryMapping,
    tags: openchuck_types::ir::PathTags,
    machine: openchuck_types::machine::MachineProfile,
}

impl Rig {
    fn new(model: BrepModel, tool: &Tool) -> Self {
        model.validate().unwrap();
        let machine = machine();
        let surface = openchuck_path::BrepSurface::new(&model).unwrap();
        Self {
            model: Arc::new(model),
            surface,
            stock: AnalyticStock::Cylinder {
                radius: 25.0,
                x_start: -5.0,
                x_end: 50.0,
            },
            tool: tool.profile().unwrap(),
            rotary: RotaryMapping::of_profile(&machine),
            tags: tags(),
            machine,
        }
    }

    fn input(&self) -> StrategyInput<'_> {
        StrategyInput {
            surface: &self.surface,
            stock: &self.stock,
            collision: &openchuck_path::AcceptAll,
            tool: &self.tool,
            rotary: self.rotary,
            clearance: Clearance {
                radius: Millimetres(30.0),
                safe_z: Millimetres(40.0),
            },
            max_feeds: self.machine.max_feeds,
            tags: &self.tags,
            entry_safety: None,
            facing: None,
            keep_out: &[],
            shank: None,
        }
    }

    fn strategy(&self, faces: &[u32], guide: &[u32]) -> FaceFlow {
        FaceFlow::on_brep(
            Arc::clone(&self.model),
            faces.iter().map(|f| FaceId(*f)).collect(),
            guide.iter().map(|e| EdgeId(*e)).collect(),
        )
    }

    fn output(
        &self,
        strategy: &FaceFlow,
        v: &BTreeMap<String, openchuck_path::ParameterValue>,
    ) -> openchuck_path::Result<StrategyOutput> {
        let parameters = strategy.resolve_parameters(v)?;
        run(|ctx| strategy.passes_with_notes(&self.input(), None, &parameters, ctx))
    }

    fn passes(
        &self,
        faces: &[u32],
        guide: &[u32],
        v: &BTreeMap<String, openchuck_path::ParameterValue>,
    ) -> Vec<Pass> {
        self.output(&self.strategy(faces, guide), v).unwrap().passes
    }

    /// The tool's axis at a cut point.
    fn axis(&self, c: &CutPoint) -> Vec3 {
        self.rotary.tool_axis(c.a).get()
    }
}

fn contacts(pass: &Pass) -> Vec<Point3> {
    pass.points.iter().map(|c| c.contact).collect()
}

/// The distance from `q` to the polyline `line`.
fn to_polyline(q: Point3, line: &[Point3]) -> f64 {
    line.windows(2)
        .map(|w| {
            let (a, b) = (w[0], w[1]);
            let d = b - a;
            let t = ((q - a).dot(d) / d.dot(d)).clamp(0.0, 1.0);
            q.distance(a + d * t)
        })
        .fold(f64::INFINITY, f64::min)
}

/// The middle of a pass, by its contact points.
fn middle(pass: &Pass) -> Point3 {
    let c = contacts(pass);
    let n = c.len() as f64;
    let s = c.iter().fold(Vec3::ZERO, |s, p| s + p.to_vec());
    Point3::ORIGIN + s * (1.0 / n)
}

// ---- OP-28 point 1 and 2: passes on the face, parallel to the guide, at the stepover ----

#[test]
fn req_op_28_on_a_twisted_ruled_face_the_passes_lie_on_the_face_parallel_to_the_guide_at_the_stepover()
 {
    let (model, height) = twisted(30.0, 6.0, [10.0, 12.0, 11.0, 10.0]);
    let rig = Rig::new(model, &ball(6.0));
    let stepover = 0.8;
    let passes = rig.passes(&[0], &[], &values(stepover, &[]));
    assert!(passes.len() >= 15, "{} passes", passes.len());
    // Every contact is on the face, by its closed form.
    for c in passes.iter().flat_map(|p| &p.points) {
        let q = c.contact;
        assert!(
            q.x > -1e-6 && q.x < 30.0 + 1e-6 && q.y.abs() < 6.0 + 1e-6,
            "{q:?}"
        );
        assert!(
            (q.z - height(q.x, q.y)).abs() < 0.005,
            "{q:?} is off the face"
        );
    }
    // The default guide is the longest side: the edge at Y -6 (it rises by
    // 2 mm over its length; the one at Y 6 falls by 1 mm), so the first
    // pass runs along it and the last along the opposite side. On a twisted
    // face the ball's contact can sit a little off the pass's own point
    // where it rests at the edge, so within a few hundredths.
    let first = contacts(&passes[0]);
    let last = contacts(passes.last().unwrap());
    assert!(first.iter().all(|q| (q.y + 6.0).abs() < 0.05), "{first:?}");
    assert!(last.iter().all(|q| (q.y - 6.0).abs() < 0.05), "{last:?}");
    assert!(first.iter().any(|q| q.x < 1e-3) && first.iter().any(|q| q.x > 30.0 - 1e-3));
    // Neighbouring passes are no further apart than the stepover, measured
    // on the face, and not much closer either: parallel to the guide.
    for w in passes.windows(2) {
        let earlier = contacts(&w[0]);
        for q in contacts(&w[1]) {
            let d = to_polyline(q, &earlier);
            assert!(d <= stepover * 1.001 + 0.005, "a gap of {d} mm");
        }
    }
    for w in passes[..passes.len() - 2].windows(2) {
        let earlier = contacts(&w[0]);
        let d = contacts(&w[1])
            .iter()
            .map(|q| to_polyline(*q, &earlier))
            .fold(f64::INFINITY, f64::min);
        assert!(d > 0.6 * stepover, "passes crowd to {d} mm");
    }
}

#[test]
fn req_op_28_the_notes_name_the_guide_and_the_passes() {
    let (model, _) = twisted(30.0, 6.0, [10.0, 12.0, 11.0, 10.0]);
    let rig = Rig::new(model, &ball(6.0));
    let out = rig
        .output(&rig.strategy(&[0], &[]), &values(1.0, &[]))
        .unwrap();
    let notes = out.notes().join("\n");
    assert!(notes.contains("guide"), "{notes}");
    assert!(notes.contains("edge 0"), "{notes}");
    assert!(
        notes.contains(&format!("{} passes", out.passes.len())),
        "{notes}"
    );
}

// ---- the stepover on a cylinder, as a distance and as a scallop height ----------------

/// The cusp height between two ball passes round a cylinder of `radius`,
/// at angles `t1` and `t2`, for a ball of radius `rho`: the two balls'
/// centres are on the circle of `radius + rho`, and the cusp is where their
/// circles meet nearest the axis.
fn cusp(radius: f64, rho: f64, t1: f64, t2: f64) -> f64 {
    let big = radius + rho;
    let half = 0.5 * (t2 - t1).abs();
    let c = half.cos();
    let r = big * c - (big * big * c * c - (big * big - rho * rho)).sqrt();
    r - radius
}

fn angle(q: Point3) -> f64 {
    q.z.atan2(q.y)
}

#[test]
fn req_op_28_on_a_cylinder_the_scallop_height_is_met_between_passes() {
    let radius = 10.0;
    let rig = Rig::new(
        cylinder_patch(radius, FRAC_PI_3, 2.0 * FRAC_PI_3, 30.0),
        &ball(6.0),
    );
    let h = 0.01;
    let passes = rig.passes(&[0], &[], &values(5.0, &[(p::SCALLOP, num(h))]));
    // The passes run along X (the guide is the longer straight side) and
    // step round the axis.
    let angles: Vec<f64> = passes.iter().map(|p| angle(middle(p))).collect();
    for p in &passes {
        let c = contacts(p);
        let a0 = angle(c[0]);
        assert!(
            c.iter().all(|q| (angle(*q) - a0).abs() < 1e-4),
            "a pass turns round the axis"
        );
        assert!(
            c.iter()
                .all(|q| ((q.y * q.y + q.z * q.z).sqrt() - radius).abs() < 0.005)
        );
    }
    let mut highest: f64 = 0.0;
    for w in angles.windows(2) {
        highest = highest.max(cusp(radius, 3.0, w[0], w[1]));
    }
    assert!(highest <= h + 1e-4, "a cusp of {highest} mm");
    // Not needlessly close: the first steps leave at least half the height.
    assert!(cusp(radius, 3.0, angles[0], angles[1]) > 0.5 * h);
}

#[test]
fn req_op_28_on_a_cylinder_the_stepover_is_measured_on_the_face() {
    let radius = 10.0;
    let rig = Rig::new(
        cylinder_patch(radius, FRAC_PI_3, 2.0 * FRAC_PI_3, 30.0),
        &ball(6.0),
    );
    let stepover = 1.0;
    let passes = rig.passes(&[0], &[], &values(stepover, &[]));
    let angles: Vec<f64> = passes.iter().map(|p| angle(middle(p))).collect();
    let span = radius * FRAC_PI_3;
    assert_eq!(passes.len(), (span / stepover).ceil() as usize + 1);
    for w in angles.windows(2) {
        let arc = radius * (w[1] - w[0]).abs();
        assert!(arc <= stepover * 1.0005, "{arc} mm apart on the face");
        assert!(arc > 0.5 * stepover);
    }
}

#[test]
fn req_op_28_a_scallop_height_for_a_flat_end_is_ignored_with_a_warning() {
    let rig = Rig::new(
        cylinder_patch(10.0, FRAC_PI_3, 2.0 * FRAC_PI_3, 30.0),
        &support::tool(openchuck_types::tool::ToolShape::FlatEnd, 6.0),
    );
    let strategy = rig.strategy(&[0], &[]);
    let v = values(1.0, &[(p::SCALLOP, num(0.01))]);
    let parameters = strategy.resolve_parameters(&v).unwrap();
    let warnings = strategy.parameter_warnings(&rig.tool, &parameters);
    assert!(
        warnings.iter().any(|w| w.contains("scallop")),
        "{warnings:?}"
    );
    let with = rig.passes(&[0], &[], &v);
    let without = rig.passes(&[0], &[], &values(1.0, &[]));
    assert_eq!(with, without);
}

// ---- point 1: the blend between boundaries of different length -----------------------

/// A trapezoid at Z 10: the guide side from (0, -5) to (30, -5) and the
/// opposite side from (25, 5) to (5, 5). Edges 0 (bottom), 1, 2 (top), 3.
fn trapezoid() -> BrepModel {
    let mut b = Builder::default();
    flat(
        &mut b,
        10.0,
        &[(0.0, -5.0), (30.0, -5.0), (25.0, 5.0), (5.0, 5.0)],
        &[],
    );
    b.model
}

#[test]
fn req_op_28_passes_blend_from_the_guide_to_a_shorter_opposite_side() {
    let rig = Rig::new(trapezoid(), &ball(6.0));
    let stepover = 0.7;
    let passes = rig.passes(&[0], &[], &values(stepover, &[]));
    let first = contacts(&passes[0]);
    let last = contacts(passes.last().unwrap());
    // The first pass is the guide, the last the opposite side, end to end.
    assert!(first.iter().all(|q| (q.y + 5.0).abs() < 1e-6));
    assert!(last.iter().all(|q| (q.y - 5.0).abs() < 1e-6));
    let span = |c: &[Point3]| {
        (
            c.iter().map(|q| q.x).fold(f64::INFINITY, f64::min),
            c.iter().map(|q| q.x).fold(f64::NEG_INFINITY, f64::max),
        )
    };
    let (a, b) = span(&first);
    assert!(a.abs() < 1e-6 && (b - 30.0).abs() < 1e-6, "{a} {b}");
    let (a, b) = span(&last);
    assert!((a - 5.0).abs() < 1e-6 && (b - 25.0).abs() < 1e-6, "{a} {b}");
    // Each pass is on the face, and the passes in between shorten evenly:
    // each runs from the left side to the right side.
    for p in &passes {
        let c = contacts(p);
        for q in &c {
            assert!((q.z - 10.0).abs() < 1e-6);
            let left = 0.5 * (q.y + 5.0);
            assert!(
                q.x > left - 1e-6 && q.x < 30.0 - left + 1e-6,
                "{q:?} is off the face"
            );
        }
        let (a, b) = span(&c);
        let left = 0.5 * (c[0].y + 5.0);
        assert!(
            (a - left).abs() < 1e-3 && (b - (30.0 - left)).abs() < 1e-3,
            "{a} {b}"
        );
    }
    // No gap on the face wider than the stepover.
    for w in passes.windows(2) {
        let earlier = contacts(&w[0]);
        for q in contacts(&w[1]) {
            assert!(to_polyline(q, &earlier) <= stepover * 1.001);
        }
    }
}

#[test]
fn req_op_28_a_three_sided_face_closes_on_its_apex() {
    let mut b = Builder::default();
    flat(&mut b, 10.0, &[(0.0, -5.0), (30.0, -5.0), (15.0, 5.0)], &[]);
    let rig = Rig::new(b.model, &ball(6.0));
    let out = rig
        .output(&rig.strategy(&[0], &[]), &values(0.7, &[]))
        .unwrap();
    let passes = &out.passes;
    assert!(
        contacts(&passes[0])
            .iter()
            .all(|q| (q.y + 5.0).abs() < 1e-6)
    );
    let triangle = [(0.0, -5.0), (30.0, -5.0), (15.0, 5.0)];
    for q in passes.iter().flat_map(contacts) {
        let inside = inside_polygon(&triangle, q.x, q.y)
            || to_polyline(
                q,
                &[
                    p3(0.0, -5.0, 10.0),
                    p3(30.0, -5.0, 10.0),
                    p3(15.0, 5.0, 10.0),
                    p3(0.0, -5.0, 10.0),
                ],
            ) < 1e-6;
        assert!(inside, "{q:?} is off the face");
    }
    // The passes close on the apex, ending where they would be shorter than
    // the tool's diameter, and a note says so.
    let shortest = passes
        .iter()
        .map(|p| {
            let c = contacts(p);
            c[0].distance(c[c.len() - 1])
        })
        .fold(f64::INFINITY, f64::min);
    assert!(shortest >= 6.0 - 1e-6, "{shortest}");
    let top = passes
        .iter()
        .flat_map(contacts)
        .map(|q| q.y)
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(
        top > 5.0 - 10.0 * 6.0 / 30.0 - 0.7 - 1e-6,
        "the passes stop at Y {top}"
    );
    assert!(out.notes().join(" ").contains("point"), "{:?}", out.notes());
}

// ---- point 1: the guide, by default and chosen ---------------------------------------

#[test]
fn req_op_28_of_two_equal_sides_the_guide_is_the_one_with_the_lowest_edge() {
    // A rectangle: the sides along X are equal; edge 0 is at Y -5.
    let mut b = Builder::default();
    flat(
        &mut b,
        10.0,
        &[(0.0, -5.0), (20.0, -5.0), (20.0, 5.0), (0.0, 5.0)],
        &[],
    );
    let rig = Rig::new(b.model, &ball(6.0));
    let passes = rig.passes(&[0], &[], &values(1.0, &[]));
    assert!(
        contacts(&passes[0])
            .iter()
            .all(|q| (q.y + 5.0).abs() < 1e-6)
    );
}

#[test]
fn req_op_28_the_user_can_choose_the_guide() {
    let rig = Rig::new(trapezoid(), &ball(6.0));
    // The top side (edge 2) as the guide: the passes start there.
    let passes = rig.passes(&[0], &[2], &values(0.7, &[]));
    assert!(
        contacts(&passes[0])
            .iter()
            .all(|q| (q.y - 5.0).abs() < 1e-6)
    );
    assert!(
        contacts(passes.last().unwrap())
            .iter()
            .all(|q| (q.y + 5.0).abs() < 1e-6)
    );
    // A slanted side (edge 1) as the guide: the passes run along it.
    let passes = rig.passes(&[0], &[1], &values(0.7, &[]));
    let first = contacts(&passes[0]);
    let side = [p3(30.0, -5.0, 10.0), p3(25.0, 5.0, 10.0)];
    assert!(first.iter().all(|q| to_polyline(*q, &side) < 1e-6));
}

#[test]
fn req_op_28_a_guide_that_is_not_a_boundary_chain_of_the_faces_is_refused() {
    let rig = Rig::new(channel(4.0), &ball(6.0));
    // Edge 5 is a wall's top edge, not on the floor.
    let err = rig
        .output(&rig.strategy(&[0], &[5]), &values(1.0, &[]))
        .unwrap_err()
        .to_string();
    assert!(err.contains("edge 5") && err.contains("guide"), "{err}");
    // Edges 0 and 2 are the floor's two long sides, not joined end to end.
    let err = rig
        .output(&rig.strategy(&[0], &[0, 2]), &values(1.0, &[]))
        .unwrap_err()
        .to_string();
    assert!(err.contains("guide") && err.contains("joined"), "{err}");
    // An edge the model does not have.
    let err = rig
        .output(&rig.strategy(&[0], &[99]), &values(1.0, &[]))
        .unwrap_err()
        .to_string();
    assert!(err.contains("edge 99"), "{err}");
}

// ---- point 4: protection of neighbouring faces, and what cannot be reached --------------

/// The ball's centre at a cut point.
fn centre_of(rig: &Rig, c: &CutPoint, r: f64) -> Point3 {
    c.tip + rig.axis(c) * r
}

/// The straight machine move from `a` to `b`, at fraction `f`: the tip
/// and A, interpolated in machine coordinates (decision records 0115 and
/// 0133).
fn between(rig: &Rig, a: &CutPoint, b: &CutPoint, f: f64) -> CutPoint {
    let pa = rig.rotary.to_axis(a.tip, a.a);
    let pb = rig.rotary.to_axis(b.tip, b.a);
    let bb = openchuck_path::rotary::nearest_turn(b.a, a.a);
    let at = openchuck_types::units::Degrees(a.a.0 + (bb.0 - a.a.0) * f);
    let pos = openchuck_path::rotary::AxisPosition {
        along: pa.along + (pb.along - pa.along) * f,
        across: pa.across + (pb.across - pa.across) * f,
        height: pa.height + (pb.height - pa.height) * f,
    };
    let tip = rig.rotary.to_model(pos, at);
    CutPoint {
        tip,
        a: at,
        contact: tip,
    }
}

#[test]
fn req_op_28_the_walls_beside_a_floor_are_never_gouged_and_what_cannot_be_reached_is_reported() {
    let half = 4.0;
    let rig = Rig::new(channel(half), &ball(6.0));
    let out = rig
        .output(&rig.strategy(&[0], &[]), &values(0.5, &[]))
        .unwrap();
    assert!(!out.passes.is_empty());
    // Independent of the library: the ball's centre stays at least its
    // radius from the floor and from both walls, at every point and at the
    // middle and quarter points of every move.
    let tol = 0.005;
    let check = |c: &CutPoint| {
        let o = centre_of(&rig, c, 3.0);
        assert!(o.z - 10.0 >= 3.0 - tol, "into the floor at {o:?}");
        if o.z < 16.0 + 3.0 {
            assert!(o.y.abs() <= half - 3.0 + tol, "into a wall at {o:?}");
        }
    };
    for pass in &out.passes {
        for c in &pass.points {
            check(c);
        }
        for w in pass.points.windows(2) {
            for f in [0.25, 0.5, 0.75] {
                check(&between(&rig, &w[0], &w[1], f));
            }
        }
    }
    // The floor within a tool radius of each wall cannot be reached: it is
    // reported, not cut.
    assert!(
        out.passes
            .iter()
            .flat_map(contacts)
            .all(|q| q.y.abs() <= half - 3.0 + tol)
    );
    assert!(!out.unreached.is_empty());
    for u in &out.unreached {
        assert_eq!(u.reason, UnreachedReason::Blocked);
        assert!(u.area > 0.0 && !u.points.is_empty());
        assert!(u.points.iter().all(|q| q.y.abs() > half - 3.0 - 0.5 - tol));
    }
    let area: f64 = out.unreached.iter().map(|u| u.area).sum();
    let expected = 30.0 * 2.0 * 3.0;
    assert!(
        (area - expected).abs() < 0.2 * expected,
        "{area} mm² reported"
    );
    assert!(
        out.warnings()
            .into_iter()
            .any(|w| w.text.contains("cannot reach")),
        "{:?}",
        out.warnings()
    );
}

#[test]
fn req_op_28_a_floor_narrower_than_the_tool_is_reported_as_too_narrow() {
    let rig = Rig::new(channel(2.5), &ball(6.0));
    let err = rig
        .output(&rig.strategy(&[0], &[]), &values(0.5, &[]))
        .unwrap_err()
        .to_string();
    assert!(err.contains("too narrow"), "{err}");
}

#[test]
fn req_op_28_an_avoid_set_is_not_cut() {
    // Two flat faces side by side, both selected, the second also an avoid
    // set (OP-02): its surface is left as it is.
    let mut b = Builder::default();
    flat(
        &mut b,
        10.0,
        &[(0.0, -5.0), (15.0, -5.0), (15.0, 5.0), (0.0, 5.0)],
        &[],
    );
    flat(
        &mut b,
        10.0,
        &[(15.0, -5.0), (30.0, -5.0), (30.0, 5.0), (15.0, 5.0)],
        &[None, None, None, Some(1)],
    );
    let rig = Rig::new(b.model, &ball(6.0));
    let avoid = openchuck_path::BrepSurface::with_faces(&rig.model, &[FaceId(1)]).unwrap();
    let region = RegionConstraint {
        avoid: Some(&avoid),
        ..RegionConstraint::default()
    };
    let strategy = rig.strategy(&[0, 1], &[]);
    let parameters = strategy.resolve_parameters(&values(0.5, &[])).unwrap();
    let out = run(|ctx| strategy.passes_with_notes(&rig.input(), Some(&region), &parameters, ctx))
        .unwrap();
    let all: Vec<Point3> = out.passes.iter().flat_map(contacts).collect();
    assert!(all.iter().all(|q| q.x < 15.0), "a contact on the avoid set");
    assert!(all.iter().any(|q| q.x > 10.0) && all.iter().any(|q| q.x < 1.0));
    // Left out on purpose, not unreachable.
    assert!(out.unreached.is_empty());
    // The strategy says it honours regions, so a host gives it its sets
    // (OP-02), and its plain passes are those it notes (OP-16).
    assert!(strategy.honours_region());
    let plain = run(|ctx| strategy.passes(&rig.input(), &parameters, ctx)).unwrap();
    let noted = run(|ctx| strategy.passes_with_notes(&rig.input(), None, &parameters, ctx))
        .unwrap()
        .passes;
    assert_eq!(plain, noted);
}

/// A flat face at Z 10 bounded by `edges`, each a curve from t 0 to 1, in
/// order round the face counter-clockwise seen from above.
fn curved_flat(edges: &[&dyn Fn(f64) -> Point3]) -> BrepModel {
    let mut b = Builder::default();
    let ids: Vec<EdgeId> = edges.iter().map(|c| b.edge(&[0], *c, false)).collect();
    let outline: Vec<(f64, f64)> = edges
        .iter()
        .flat_map(|c| (0..256).map(move |i| c(f64::from(i) / 256.0)))
        .map(|p| (p.x, p.y))
        .collect();
    let xs = outline.iter().map(|c| c.0);
    let ys = outline.iter().map(|c| c.1);
    let uv = [
        xs.clone().fold(f64::INFINITY, f64::min),
        xs.fold(f64::NEG_INFINITY, f64::max),
        ys.clone().fold(f64::INFINITY, f64::min),
        ys.fold(f64::NEG_INFINITY, f64::max),
    ];
    let inside = {
        let outline = outline.clone();
        move |u: f64, v: f64| inside_polygon(&outline, u, v)
    };
    b.face(
        Surface::Plane {
            frame: frame(
                p3(0.0, 0.0, 10.0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
            ),
        },
        uv,
        Some(outline),
        ids,
        &|u, v| p3(u, v, 10.0),
        &inside,
        false,
    );
    b.model
}

#[test]
fn req_op_28_a_round_face_is_cut_between_its_two_halves() {
    // One closed edge, a circle of radius 5 about (15, 0): no corners, so
    // it is cut at its two points furthest apart.
    let circle = |t: f64| {
        let a = std::f64::consts::TAU * t;
        p3(15.0 + 5.0 * a.cos(), 5.0 * a.sin(), 10.0)
    };
    let rig = Rig::new(curved_flat(&[&circle]), &ball(6.0));
    let out = rig
        .output(&rig.strategy(&[0], &[]), &values(0.8, &[]))
        .unwrap();
    let r = |q: &Point3| (q.x - 15.0).hypot(q.y);
    let all: Vec<Point3> = out.passes.iter().flat_map(contacts).collect();
    assert!(
        all.iter().all(|q| r(q) <= 5.0 + 1e-3),
        "a contact outside the face"
    );
    let first = contacts(&out.passes[0]);
    let last = contacts(out.passes.last().unwrap());
    assert!(first.iter().all(|q| (r(q) - 5.0).abs() < 1e-3));
    assert!(last.iter().all(|q| (r(q) - 5.0).abs() < 1e-3));
    // The two halves are opposite each other.
    let (a, b) = (middle(&out.passes[0]), middle(out.passes.last().unwrap()));
    assert!((a - p3(15.0, 0.0, 10.0)).dot(b - p3(15.0, 0.0, 10.0)) < 0.0);
    assert!(out.passes.len() >= 10);
}

#[test]
fn req_op_28_a_two_sided_face_is_cut_from_its_longer_side_to_the_other() {
    // Two arcs from (0, 0) to (20, 0), meeting at corners: the lower one
    // bulges 4 mm and is the longer, the upper one 2 mm.
    let arc = |sag: f64| {
        move |t: f64| {
            let x = 20.0 * t;
            // A circle through both ends with the sagitta `sag`.
            let radius = (100.0 + sag * sag) / (2.0 * sag);
            let y = ((radius * radius - (x - 10.0) * (x - 10.0)).sqrt() - (radius - sag)).max(0.0);
            p3(x, y, 10.0)
        }
    };
    let up = arc(2.0);
    let lower = arc(4.0);
    let down = move |t: f64| {
        let p = lower(t);
        p3(p.x, -p.y, 10.0)
    };
    let back = move |t: f64| up(1.0 - t);
    let rig = Rig::new(curved_flat(&[&down, &back]), &ball(6.0));
    let passes = rig.passes(&[0], &[], &values(0.5, &[]));
    let on = |q: &Point3, sag: f64, sign: f64| {
        let radius = (100.0 + sag * sag) / (2.0 * sag);
        let centre = p3(10.0, -sign * (radius - sag), 10.0);
        (q.distance(centre) - radius).abs() < 1e-3
    };
    assert!(contacts(&passes[0]).iter().all(|q| on(q, 4.0, -1.0)));
    assert!(
        contacts(passes.last().unwrap())
            .iter()
            .all(|q| on(q, 2.0, 1.0))
    );
}

#[test]
fn req_op_28_moves_round_a_small_face_off_the_axis_stay_within_the_tolerance() {
    // A narrow strip of a cylinder of radius 1.2 whose axis is 8 mm from
    // the rotary axis: the guide is its arc, so each pass goes round it and
    // the part turns under the tool. A straight machine move between two
    // points then leaves the face, and must be split to the tolerance
    // (decision records 0115 and 0133).
    let (centre, radius, r) = (8.0, 1.2, 1.0);
    let rig = Rig::new(
        offset_cylinder(centre, radius, PI / 6.0, 5.0 * PI / 6.0, 2.0),
        &ball(2.0 * r),
    );
    // A fine tolerance: the face's points along a pass must be closer than
    // the guide's samples for its chords to stay on it.
    let tol = 0.002;
    let passes = rig.passes(&[0], &[], &values(0.3, &[(names::TOLERANCE, num(tol))]));
    let longest = passes.iter().map(|p| p.points.len()).max().unwrap();
    assert!(longest > 40, "only {longest} points round the face");
    assert!(!passes.is_empty());
    let from_axis = |c: &CutPoint| {
        let o = centre_of(&rig, c, r);
        o.y.hypot(o.z - centre)
    };
    for pass in &passes {
        for c in &pass.points {
            let d = from_axis(c);
            assert!(d >= radius + r - tol - 1e-6, "into the face at {c:?}: {d}");
        }
        for w in pass.points.windows(2) {
            for f in [0.25, 0.5, 0.75] {
                let d = from_axis(&between(&rig, &w[0], &w[1], f));
                assert!(
                    d >= radius + r - tol - 1e-6,
                    "a move cuts into the face: {d}"
                );
                assert!(d <= radius + r + 2.0 * tol, "a move leaves {d} mm");
            }
        }
    }
}

#[test]
fn req_op_28_stock_to_leave_is_left_on_the_face() {
    let rig = Rig::new(trapezoid(), &ball(6.0));
    let passes = rig.passes(
        &[0],
        &[],
        &values(1.0, &[(names::STOCK_TO_LEAVE, num(0.2))]),
    );
    for c in passes.iter().flat_map(|p| &p.points) {
        // Within the exact drop's accuracy at the face's edge.
        assert!((c.tip.z - 10.2).abs() < 1e-4, "{c:?}");
    }
}

// ---- point 3: the tool axis ---------------------------------------------------------

/// The difference between two angles in degrees, in (-180, 180].
fn turn(a: f64, b: f64) -> f64 {
    (a - b + 540.0).rem_euclid(360.0) - 180.0
}

#[test]
fn req_op_28_the_tool_axis_follows_the_face_within_the_tilt_limit() {
    // Half a cylinder round the axis, 10 long: the guide is the half-circle
    // arc at X 0, so the passes run round and A follows the face.
    let rig = Rig::new(cylinder_patch(10.0, 0.0, PI, 10.0), &ball(6.0));
    // The axis leans by the lead towards the travel round the axis, and the
    // smoothing may move it by up to the tilt from there, as simultaneous
    // finishing's (OP-06).
    for (lead, tilt) in [(0.0, 5.0), (8.0, 10.0), (8.0, 0.0)] {
        let passes = rig.passes(
            &[0],
            &[],
            &values(1.0, &[(p::LEAD_ANGLE, num(lead)), (p::MAX_TILT, num(tilt))]),
        );
        for pass in &passes {
            let c = contacts(pass);
            // A pass goes round at one X.
            assert!(c.iter().all(|q| (q.x - c[0].x).abs() < 1e-6));
            for point in &pass.points {
                let q = point.contact;
                let normal = Vec3::new(0.0, q.y, q.z);
                let facing = rig.rotary.a_facing(normal).unwrap();
                let off = turn(point.a.0, facing.0).abs();
                assert!(off <= lead + tilt + 1e-6, "{off} degrees from the normal");
                if tilt == 0.0 {
                    assert!((off - lead).abs() < 1e-6, "{off} degrees, not the lead");
                }
            }
            let a: Vec<f64> = pass.points.iter().map(|c| c.a.0).collect();
            let spread = a.iter().fold(f64::NEG_INFINITY, |m, x| m.max(*x))
                - a.iter().fold(f64::INFINITY, |m, x| m.min(*x));
            assert!(spread > 150.0, "A turns only {spread} degrees along a pass");
        }
    }
}

// ---- point 5: direction, and several faces ----------------------------------------

/// The side the next pass is on, seen from the spindle along the feed:
/// positive on the tool's right.
fn sides(rig: &Rig, passes: &[Pass]) -> Vec<f64> {
    passes
        .windows(2)
        .map(|w| {
            let pass = &w[0];
            let i = pass.points.len() / 2;
            let (a, b) = (pass.points[i - 1], pass.points[i]);
            let feed = b.contact - a.contact;
            let right = feed.cross(rig.axis(&b));
            (middle(&w[1]) - middle(pass)).dot(right).signum()
        })
        .collect()
}

#[test]
fn req_op_28_climb_conventional_and_meander_direct_the_passes() {
    let rig = Rig::new(trapezoid(), &ball(6.0));
    let go = |d: &str| rig.passes(&[0], &[], &values(1.0, &[(p::DIRECTION, text(d))]));
    // Climb with M3: the uncut material, where the next pass goes, is on
    // the tool's right looking along the feed from the spindle (decision
    // record 0213); conventional on its left.
    assert!(sides(&rig, &go(p::CLIMB)).iter().all(|s| *s > 0.0));
    assert!(sides(&rig, &go(p::CONVENTIONAL)).iter().all(|s| *s < 0.0));
    // Meander, the default, alternates.
    let meander = go(p::MEANDER);
    let s = sides(&rig, &meander);
    assert!(s.windows(2).all(|w| w[0] == -w[1]), "{s:?}");
    assert_eq!(meander, rig.passes(&[0], &[], &values(1.0, &[])));
}

#[test]
fn req_op_28_several_faces_are_cut_one_after_another_along_their_shared_edges() {
    // Three flat faces in a row along X, at Z 10: face 0 from X 0 to 20,
    // face 1 from 20 to 45 (the longest guide) and face 2 from 45 to 55.
    let mut b = Builder::default();
    flat(
        &mut b,
        10.0,
        &[(0.0, -5.0), (20.0, -5.0), (20.0, 5.0), (0.0, 5.0)],
        &[],
    );
    // Face 1's side at X 20 is face 0's edge 1; face 2's side at X 45 is
    // face 1's edge 5.
    flat(
        &mut b,
        10.0,
        &[(20.0, -5.0), (45.0, -5.0), (45.0, 5.0), (20.0, 5.0)],
        &[None, None, None, Some(1)],
    );
    flat(
        &mut b,
        10.0,
        &[(45.0, -5.0), (55.0, -5.0), (55.0, 5.0), (45.0, 5.0)],
        &[None, None, None, Some(5)],
    );
    let rig = Rig::new(b.model, &ball(6.0));
    let passes = rig.passes(&[0, 1, 2], &[], &values(1.0, &[]));
    // Which face each pass is on, by its middle.
    let face = |p: &Pass| {
        let x = middle(p).x;
        if x < 20.0 {
            0
        } else if x < 45.0 {
            1
        } else {
            2
        }
    };
    let order: Vec<u32> = passes.iter().map(face).collect();
    let mut runs = order.clone();
    runs.dedup();
    // Face 1 first (its guide is the longest), then a face that shares an
    // edge with it, lowest ID first; each face once.
    assert_eq!(runs, vec![1, 0, 2], "{order:?}");
    // Within a face no pass waits for a full retract.
    for w in passes.windows(2) {
        if face(&w[0]) == face(&w[1]) {
            assert!(w[1].low_link);
        }
    }
}

// ---- point 6 and NFR-07: what is refused ------------------------------------------------

#[test]
fn req_op_28_the_strategy_needs_exact_faces() {
    let flow = FaceFlow::default();
    assert!(flow.needs_brep_part().is_some());
    assert_eq!(flow.name(), face_flow::NAME);
    // What it does, in plain words, never implying the output is safe to
    // run unchecked (OP-16, NFR-12; decision 0237).
    assert!(!flow.label().is_empty());
    assert!(flow.description().contains("STEP model"));
    assert!(
        flow.description()
            .contains("Check the toolpath and simulation")
    );
    // The registry's instance has no model: generating with it is refused
    // in plain words, never a crash.
    let rig = Rig::new(trapezoid(), &ball(6.0));
    let err = rig
        .output(&flow, &values(1.0, &[]))
        .unwrap_err()
        .to_string();
    assert!(err.contains("STEP"), "{err}");
}

#[test]
fn req_op_28_nfr_07_bad_input_is_refused_with_a_message() {
    let rig = Rig::new(trapezoid(), &ball(6.0));
    let err = rig
        .output(&rig.strategy(&[], &[]), &values(1.0, &[]))
        .unwrap_err()
        .to_string();
    assert!(err.contains("face"), "{err}");
    let err = rig
        .output(&rig.strategy(&[7], &[]), &values(1.0, &[]))
        .unwrap_err()
        .to_string();
    assert!(err.contains("face 7"), "{err}");
    // Too fine a stepover for the face.
    let err = rig
        .output(&rig.strategy(&[0], &[]), &values(1e-4, &[]))
        .unwrap_err()
        .to_string();
    assert!(err.contains("passes"), "{err}");
    // The whole boundary as the guide leaves nothing to step towards.
    let err = rig
        .output(&rig.strategy(&[0], &[0, 1, 2, 3]), &values(1.0, &[]))
        .unwrap_err()
        .to_string();
    assert!(err.contains("whole boundary"), "{err}");
    // A face whose edges do not close, and one with no edges at all.
    let mut open = trapezoid();
    open.faces[0].edges.pop();
    let broken = Rig::new(open, &ball(6.0));
    let err = broken
        .output(&broken.strategy(&[0], &[]), &values(1.0, &[]))
        .unwrap_err()
        .to_string();
    assert!(err.contains("does not close"), "{err}");
    let bare = Rig::new(
        openchuck_testkit::ruled::floor_plane(10.0, 0.0, 30.0, -5.0, 5.0),
        &ball(6.0),
    );
    let err = bare
        .output(&bare.strategy(&[0], &[]), &values(1.0, &[]))
        .unwrap_err()
        .to_string();
    assert!(err.contains("no boundary edges"), "{err}");
    let strategy = rig.strategy(&[0], &[]);
    for (name, value) in [
        (p::LEAD_ANGLE, num(40.0)),
        (p::LEAD_ANGLE, num(-40.0)),
        (p::MAX_TILT, num(-1.0)),
        (p::MAX_TILT, num(45.0)),
        (p::SCALLOP, num(-0.1)),
        (p::DIRECTION, text("sideways")),
        (names::STEPOVER, num(0.0)),
        (names::STEPOVER, num(f64::NAN)),
    ] {
        let err = rig.output(&strategy, &values(1.0, &[(name, value.clone())]));
        assert!(err.is_err(), "{name} = {value:?} was accepted");
    }
}

// ---- point 7: determinism --------------------------------------------------------------

#[test]
fn req_op_28_nfr_06_the_path_is_identical_at_1_2_and_n_threads() {
    let rig = Rig::new(channel(4.0), &ball(6.0));
    let strategy = rig.strategy(&[0], &[]);
    let v = values(0.5, &[(p::LEAD_ANGLE, num(3.0))]);
    let runs = openchuck_testkit::determinism::check(|| {
        let path = run(|ctx| openchuck_path::generate(&strategy, &rig.input(), &v, ctx)).unwrap();
        format!("{:?} {:?} {:?}", path.group, path.notices, path.unreached).into_bytes()
    })
    .unwrap();
    assert_eq!(runs.len(), 4);
}
