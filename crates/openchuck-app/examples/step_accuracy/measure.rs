// SPDX-License-Identifier: AGPL-3.0-or-later
//! The measuring functions of `step_accuracy` (decision record 0211), shared
//! with `tests/step_accuracy.rs`, which checks them on closed forms.
//!
//! Nothing here uses the drop-cutter or the path fitter: the tessellation is
//! judged against the exact faces by `ParametricFace::closest`. A tool pose
//! is judged by the bounded checker (`bounded.rs`, issue #346), and the
//! stations of a move are placed here.

use openchuck_path::surface::ParametricFace;
use openchuck_path::{ExactFace, RotaryMapping};
use openchuck_types::brep::{BrepFace, BrepModel, Frame, Surface, SurfaceKind};
use openchuck_types::geometry::{Point3, UnitVec3, Vec3};
use openchuck_types::ir::ToolPose;
use openchuck_types::units::Degrees;
use rayon::prelude::*;

/// The name of a face's surface kind, as the report prints it.
pub fn kind_name(kind: SurfaceKind) -> &'static str {
    match kind {
        SurfaceKind::Plane => "plane",
        SurfaceKind::Cylinder => "cylinder",
        SurfaceKind::Cone => "cone",
        SurfaceKind::Sphere => "sphere",
        SurfaceKind::Torus => "torus",
        SurfaceKind::BSpline => "b-spline",
        _ => "other",
    }
}

/// M1 for one kind of face: how far the tessellation lies from the exact
/// faces of that kind.
#[derive(Debug, Clone, PartialEq)]
pub struct KindError {
    /// The surface kind.
    pub kind: &'static str,
    /// Faces of that kind.
    pub faces: usize,
    /// Their triangles.
    pub triangles: usize,
    /// Triangles measured (every one, including large faces).
    pub measured: usize,
    /// Points measured.
    pub samples: usize,
    /// The largest distance from a sample to its face's surface, untrimmed
    /// (within its parameter box), mm: the tessellation's own error.
    pub max: f64,
    /// The largest distance from a sample to the trimmed face, mm: also
    /// counts where a sample near an edge lies outside the face's trim
    /// polylines, which are coarser than the tessellation there.
    pub trimmed: f64,
    /// The largest distance from a sample to the surface in closed form,
    /// from the face's plain data only (no evaluator of the path library),
    /// for the analytic kinds; `None` for a B-spline face.
    pub closed_form: Option<f64>,
    /// The 99th percentile of that distance, mm.
    pub p99: f64,
    /// The largest angle between a facet's normal and the exact normal at the
    /// closest point to its centre, in degrees (OI-76).
    pub normal_degrees: f64,
    /// The mean time of one `ExactFace::closest` on the trimmed face over
    /// the samples, in microseconds (one thread per face).
    pub closest_us: f64,
}

/// The distance from `p` to the whole (untrimmed) analytic surface
/// `surface`, in closed form; `None` for a B-spline surface.
pub fn closed_form_distance(surface: &Surface, p: Point3) -> Option<f64> {
    let local = |f: &Frame| {
        let w = p - f.origin;
        let h = w.dot(f.z);
        let rho = (w - f.z * h).length();
        (w, h, rho)
    };
    Some(match surface {
        Surface::Plane { frame } => (p - frame.origin).dot(frame.z).abs(),
        Surface::Cylinder { frame, radius } => (local(frame).2 - radius).abs(),
        Surface::Cone {
            frame,
            radius,
            semi_angle,
        } => {
            let (_, h, rho) = local(frame);
            ((rho - radius) * semi_angle.cos() - h * semi_angle.sin()).abs()
        }
        Surface::Sphere { frame, radius } => (local(frame).0.length() - radius).abs(),
        Surface::Torus {
            frame,
            major,
            minor,
        } => {
            let (_, h, rho) = local(frame);
            ((rho - major).hypot(h) - minor).abs()
        }
        _ => return None,
    })
}

/// Points of triangle `(a, b, c)`: its corners, the middles of its edges and
/// a barycentric grid of `n` steps a side (decision 0209 item 4.1, M1).
fn triangle_points(a: Point3, b: Point3, c: Point3, n: usize) -> Vec<Point3> {
    let mut out = vec![a, b, c, mid(a, b), mid(b, c), mid(c, a)];
    let n = n.max(1);
    for i in 0..=n {
        for j in 0..=(n - i) {
            let (u, v) = (i as f64 / n as f64, j as f64 / n as f64);
            out.push(a + (b - a) * u + (c - a) * v);
        }
    }
    out
}

fn mid(a: Point3, b: Point3) -> Point3 {
    a + (b - a) * 0.5
}

/// M1 for one face.
struct FaceError {
    kind: SurfaceKind,
    triangles: usize,
    measured: usize,
    /// Distances to the untrimmed surface, one per sample.
    distances: Vec<f64>,
    normal_degrees: f64,
    trimmed: f64,
    closed_form: Option<f64>,
    /// Time spent in `closest` on the trimmed face, and how many calls.
    closest_nanos: u128,
    closest_calls: u64,
}

/// M1: the distance from the tessellation of every face of `model` to the
/// face itself, by kind, sampled at the corners, the edge middles and a grid
/// of `grid` steps a side of each triangle. The face's own triangles are
/// measured against it, so a triangle is never judged against a neighbour.
pub fn tessellation_error(model: &BrepModel, grid: usize) -> Vec<KindError> {
    let per_face: Vec<FaceError> = model
        .faces
        .par_iter()
        .map(|face| face_error(face, grid))
        .collect();
    let mut kinds: Vec<&'static str> = per_face.iter().map(|f| kind_name(f.kind)).collect();
    kinds.sort_unstable();
    kinds.dedup();
    kinds
        .into_iter()
        .map(|kind| {
            let of_kind: Vec<&FaceError> = per_face
                .iter()
                .filter(|f| kind_name(f.kind) == kind)
                .collect();
            let mut all: Vec<f64> = of_kind
                .iter()
                .flat_map(|f| f.distances.iter().copied())
                .collect();
            all.sort_by(f64::total_cmp);
            let at = |q: f64| {
                all.get(((all.len().saturating_sub(1)) as f64 * q).round() as usize)
                    .copied()
                    .unwrap_or(0.0)
            };
            let calls: u64 = of_kind.iter().map(|f| f.closest_calls).sum();
            let nanos: u128 = of_kind.iter().map(|f| f.closest_nanos).sum();
            KindError {
                kind,
                faces: of_kind.len(),
                triangles: of_kind.iter().map(|f| f.triangles).sum(),
                measured: of_kind.iter().map(|f| f.measured).sum(),
                samples: all.len(),
                max: all.last().copied().unwrap_or(0.0),
                trimmed: of_kind.iter().map(|f| f.trimmed).fold(0.0, f64::max),
                closed_form: of_kind
                    .iter()
                    .filter_map(|f| f.closed_form)
                    .reduce(f64::max),
                p99: at(0.99),
                normal_degrees: of_kind.iter().map(|f| f.normal_degrees).fold(0.0, f64::max),
                closest_us: nanos as f64 / 1000.0 / calls.max(1) as f64,
            }
        })
        .collect()
}

fn face_error(face: &BrepFace, grid: usize) -> FaceError {
    let mut out = FaceError {
        kind: face.surface,
        triangles: face.tessellation.triangles.len(),
        measured: 0,
        distances: Vec::new(),
        normal_degrees: 0.0,
        trimmed: 0.0,
        closed_form: None,
        closest_nanos: 0,
        closest_calls: 0,
    };
    let untrimmed = BrepFace {
        trims: Vec::new(),
        ..face.clone()
    };
    let (Ok(exact), Ok(trimmed)) = (ExactFace::new(&untrimmed), ExactFace::new(face)) else {
        return out;
    };
    let mesh = &face.tessellation;
    // Every triangle is required by decision 0209. A fixed stride can miss
    // the only bad triangle and understate the face's measured maximum.
    out.measured = mesh.triangles.len();
    for t in &mesh.triangles {
        let [a, b, c] = t.map(|i| {
            mesh.vertices
                .get(i as usize)
                .copied()
                .unwrap_or(Point3::ORIGIN)
        });
        for p in triangle_points(a, b, c, grid) {
            if let Some(closest) = exact.closest(p) {
                out.distances.push(closest.distance);
            }
            let started = std::time::Instant::now();
            let answer = trimmed.closest(p);
            out.closest_nanos += started.elapsed().as_nanos();
            out.closest_calls += 1;
            if let Some(closest) = answer {
                out.trimmed = out.trimmed.max(closest.distance);
            }
            if let Some(d) = closed_form_distance(&face.geometry, p) {
                out.closed_form = Some(out.closed_form.unwrap_or(0.0).max(d));
            }
        }
        let n = (b - a).cross(c - a);
        let centre = Point3::new(
            (a.x + b.x + c.x) / 3.0,
            (a.y + b.y + c.y) / 3.0,
            (a.z + b.z + c.z) / 3.0,
        );
        if let (Some(unit), Some(closest)) = (UnitVec3::new(n).ok(), exact.closest(centre)) {
            let cos = unit.get().dot(closest.point.normal.get()).clamp(-1.0, 1.0);
            out.normal_degrees = out.normal_degrees.max(cos.acos().to_degrees());
        }
    }
    out
}

/// A tool position inside a move: where the machine's straight move puts
/// the tool, in the model frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Station {
    /// The tool tip.
    pub tip: Point3,
    /// The tool axis, tip to spindle.
    pub axis: Vec3,
}

impl Station {
    /// The tool at a pose.
    pub fn at(pose: &ToolPose) -> Self {
        Self {
            tip: pose.tip,
            axis: pose.axis.get(),
        }
    }

    /// The quarter, middle and three-quarter points of the move from `start`
    /// to `end`, linear in the machine's X, Y, Z and A as the controller
    /// runs it (decisions 0025 item 3, 0115 and 0133).
    pub fn along(rotary: &RotaryMapping, start: &ToolPose, end: &ToolPose) -> Vec<Self> {
        let (p, q) = (
            rotary.to_axis(start.tip, start.a),
            rotary.to_axis(end.tip, end.a),
        );
        [0.25, 0.5, 0.75]
            .into_iter()
            .map(|t| {
                let a = Degrees(start.a.0 + (end.a.0 - start.a.0) * t);
                let position = openchuck_path::rotary::AxisPosition {
                    along: p.along + (q.along - p.along) * t,
                    across: p.across + (q.across - p.across) * t,
                    height: p.height + (q.height - p.height) * t,
                };
                Self {
                    tip: rotary.to_model(position, a),
                    axis: rotary.tool_axis(a).get(),
                }
            })
            .collect()
    }
}
