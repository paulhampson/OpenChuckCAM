// SPDX-License-Identifier: AGPL-3.0-or-later
//! The measuring functions of `step_accuracy` (decision record 0211), shared
//! with `tests/step_accuracy.rs`, which checks them on closed forms.
//!
//! Nothing here uses the drop-cutter or the path fitter: the tessellation is
//! judged against the exact faces by `ParametricFace::closest`, and a tool
//! pose against points sampled on a surface by the independent checker's
//! outline (`openchuck_testkit::gouge::Outline`), as decision 0056's
//! *Verification* does.

use std::collections::HashMap;

use openchuck_path::surface::{ParametricFace, Uv};
use openchuck_path::{ExactFace, RotaryMapping};
use openchuck_testkit::gouge::Outline;
use openchuck_types::brep::{BrepFace, BrepModel, Frame, Surface, SurfaceKind};
use openchuck_types::geometry::{Point3, UnitVec3, Vec3};
use openchuck_types::ir::ToolPose;
use openchuck_types::mesh::TriangleMesh;
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
    /// Triangles measured (every one, up to [`MAX_TRIANGLES`] a face).
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

/// Points covering triangle `(a, b, c)` about `spacing` apart: rows
/// parallel to its longest side, `spacing` apart across it, each with points
/// `spacing` apart along it, so a long sliver gets as few points as its area
/// needs.
fn spaced_points(a: Point3, b: Point3, c: Point3, spacing: f64) -> Vec<Point3> {
    // The longest side as the base (p, q), the apex r opposite it.
    let sides = [(a, b, c), (b, c, a), (c, a, b)];
    let (p, q, r) = sides
        .into_iter()
        .max_by(|x, y| (x.1 - x.0).length().total_cmp(&(y.1 - y.0).length()))
        .unwrap_or((a, b, c));
    let base = q - p;
    let length = base.length();
    let height = if length > 0.0 {
        base.cross(r - p).length() / length
    } else {
        0.0
    };
    let rows = ((height / spacing).ceil() as usize).max(1);
    let mut out = vec![a, b, c];
    for k in 0..=rows {
        let t = k as f64 / rows as f64;
        let (from, to) = (p + (r - p) * t, q + (r - q) * t);
        let n = (((1.0 - t) * length / spacing).ceil() as usize).max(1);
        for i in 0..=n {
            out.push(from + (to - from) * (i as f64 / n as f64));
        }
    }
    out
}

fn mid(a: Point3, b: Point3) -> Point3 {
    a + (b - a) * 0.5
}

/// The most triangles of one face that M1 measures.
pub const MAX_TRIANGLES: usize = 20_000;

/// M1: the distance from the tessellation of every face of `model` to the
/// face itself, by kind, sampled at the corners, the edge middles and a grid
/// of `grid` steps a side of each triangle. The face's own triangles are
/// measured against it, so a triangle is never judged against a neighbour.
pub fn tessellation_error(model: &BrepModel, grid: usize) -> Vec<KindError> {
    // Per face: (kind, triangles, distances, worst normal angle, worst
    // distance to the trimmed face, worst closed-form distance).
    let per_face: Vec<(SurfaceKind, usize, Vec<f64>, f64, f64, Option<f64>, usize)> = model
        .faces
        .par_iter()
        .map(|face| {
            let untrimmed = openchuck_types::brep::BrepFace {
                trims: Vec::new(),
                ..face.clone()
            };
            let (Ok(exact), Ok(trimmed)) = (ExactFace::new(&untrimmed), ExactFace::new(face))
            else {
                return (face.surface, 0, Vec::new(), 0.0, 0.0, None, 0);
            };
            let mesh = &face.tessellation;
            let mut distances = Vec::new();
            let mut normal: f64 = 0.0;
            let mut trim: f64 = 0.0;
            let mut closed: Option<f64> = None;
            // A face of many triangles is measured on every `stride`th of
            // them, a fixed choice, so the run stays within minutes.
            let stride = mesh.triangles.len().div_ceil(MAX_TRIANGLES).max(1);
            for t in mesh.triangles.iter().step_by(stride) {
                let [a, b, c] = t.map(|i| {
                    mesh.vertices
                        .get(i as usize)
                        .copied()
                        .unwrap_or(Point3::ORIGIN)
                });
                for p in triangle_points(a, b, c, grid) {
                    if let Some(closest) = exact.closest(p) {
                        distances.push(closest.distance);
                    }
                    if let Some(closest) = trimmed.closest(p) {
                        trim = trim.max(closest.distance);
                    }
                    if let Some(d) = closed_form_distance(&face.geometry, p) {
                        closed = Some(closed.unwrap_or(0.0).max(d));
                    }
                }
                let n = (b - a).cross(c - a);
                let centre = Point3::new(
                    (a.x + b.x + c.x) / 3.0,
                    (a.y + b.y + c.y) / 3.0,
                    (a.z + b.z + c.z) / 3.0,
                );
                if let (Some(unit), Some(closest)) = (UnitVec3::new(n).ok(), exact.closest(centre))
                {
                    let cos = unit.get().dot(closest.point.normal.get()).clamp(-1.0, 1.0);
                    normal = normal.max(cos.acos().to_degrees());
                }
            }
            let measured = mesh.triangles.len().div_ceil(stride);
            (
                face.surface,
                mesh.triangles.len(),
                distances,
                normal,
                trim,
                closed,
                measured,
            )
        })
        .collect();
    let mut kinds: Vec<&'static str> = per_face.iter().map(|f| kind_name(f.0)).collect();
    kinds.sort_unstable();
    kinds.dedup();
    kinds
        .into_iter()
        .map(|kind| {
            let of_kind: Vec<_> = per_face.iter().filter(|f| kind_name(f.0) == kind).collect();
            let mut all: Vec<f64> = of_kind.iter().flat_map(|f| f.2.iter().copied()).collect();
            all.sort_by(f64::total_cmp);
            let at = |q: f64| {
                all.get(((all.len().saturating_sub(1)) as f64 * q).round() as usize)
                    .copied()
                    .unwrap_or(0.0)
            };
            KindError {
                kind,
                faces: of_kind.len(),
                triangles: of_kind.iter().map(|f| f.1).sum(),
                measured: of_kind.iter().map(|f| f.6).sum(),
                samples: all.len(),
                max: all.last().copied().unwrap_or(0.0),
                trimmed: of_kind.iter().map(|f| f.4).fold(0.0, f64::max),
                closed_form: of_kind.iter().filter_map(|f| f.5).reduce(f64::max),
                p99: at(0.99),
                normal_degrees: of_kind.iter().map(|f| f.3).fold(0.0, f64::max),
            }
        })
        .collect()
}

/// Points on a surface, bucketed in cubes for the search round a tool.
#[derive(Debug, Default)]
pub struct Cloud {
    cell: f64,
    cells: HashMap<[i64; 3], Vec<Point3>>,
    count: usize,
}

/// Side of the cubes the points are bucketed in, mm.
const CELL: f64 = 0.5;

impl Cloud {
    fn key(&self, p: Point3) -> [i64; 3] {
        [
            (p.x / self.cell).floor() as i64,
            (p.y / self.cell).floor() as i64,
            (p.z / self.cell).floor() as i64,
        ]
    }

    fn from_points(points: impl IntoIterator<Item = Point3>) -> Self {
        let mut cloud = Self {
            cell: CELL,
            ..Self::default()
        };
        for p in points {
            let key = cloud.key(p);
            cloud.cells.entry(key).or_default().push(p);
            cloud.count += 1;
        }
        cloud
    }

    /// Points measured.
    pub fn len(&self) -> usize {
        self.count
    }

    /// Points on the exact faces of `model`, inside their trims, about
    /// `spacing` mm apart along each parameter direction, and along each
    /// face's trim loops, so the samples reach the face's edges (a grid alone
    /// stops up to a step short of them).
    pub fn of_faces(model: &BrepModel, spacing: f64) -> Self {
        let per_face: Vec<Vec<Point3>> = model
            .faces
            .par_iter()
            .map(|face| {
                ExactFace::new(face)
                    .map(|exact| {
                        let mut points = face_points(&exact, spacing);
                        points.extend(boundary_points(face, &exact, spacing));
                        points
                    })
                    .unwrap_or_default()
            })
            .collect();
        Self::from_points(per_face.into_iter().flatten())
    }

    /// Points on the triangles of `mesh`, about `spacing` mm apart.
    pub fn of_mesh(mesh: &TriangleMesh, spacing: f64) -> Self {
        let per_triangle: Vec<Vec<Point3>> = mesh
            .triangles
            .par_iter()
            .map(|t| {
                let [a, b, c] = t.map(|i| {
                    mesh.vertices
                        .get(i as usize)
                        .copied()
                        .unwrap_or(Point3::ORIGIN)
                });
                spaced_points(a, b, c, spacing)
            })
            .collect();
        Self::from_points(per_triangle.into_iter().flatten())
    }

    /// The smallest signed clearance between the tool of `outline`, its tip
    /// at `tip` and its axis along `axis` (tip to spindle), and the points:
    /// negative by the deepest a point lies inside the tool. Only the tool up
    /// to `height` above its tip is checked, and only points within `margin`
    /// of it are seen, so the answer is at most `margin`.
    pub fn clearance(
        &self,
        outline: &Outline,
        tip: Point3,
        axis: Vec3,
        height: f64,
        margin: f64,
    ) -> f64 {
        let reach = outline.radius() + margin;
        let top = tip + axis * height;
        let lo = Point3::new(
            tip.x.min(top.x) - reach,
            tip.y.min(top.y) - reach,
            tip.z.min(top.z) - reach,
        );
        let hi = Point3::new(
            tip.x.max(top.x) + reach,
            tip.y.max(top.y) + reach,
            tip.z.max(top.z) + reach,
        );
        let (a, b) = (self.key(lo), self.key(hi));
        // A cube is looked at when its centre is within reach of the axis
        // segment, widened by the cube's half diagonal.
        let half = self.cell * 0.5 * 3.0_f64.sqrt();
        let mut best = margin;
        for x in a[0]..=b[0] {
            for y in a[1]..=b[1] {
                for z in a[2]..=b[2] {
                    let centre = Point3::new(
                        (x as f64 + 0.5) * self.cell,
                        (y as f64 + 0.5) * self.cell,
                        (z as f64 + 0.5) * self.cell,
                    );
                    let w = centre - tip;
                    let along = w.dot(axis).clamp(-margin, height + margin);
                    if (w - axis * along).length() > reach + half {
                        continue;
                    }
                    let Some(points) = self.cells.get(&[x, y, z]) else {
                        continue;
                    };
                    for p in points {
                        let w = *p - tip;
                        let z = w.dot(axis);
                        if z > height + margin {
                            continue;
                        }
                        let r = (w - axis * z).length();
                        best = best.min(outline.clearance(r, z));
                    }
                }
            }
        }
        best
    }
}

/// Points of one face, a grid in (u, v) as fine as `spacing` along the
/// longest iso-line in each direction, kept where the trims hold them.
fn face_points(face: &ExactFace, spacing: f64) -> Vec<Point3> {
    let (lo, hi) = face.uv_bounds();
    let at = |s: f64, t: f64| {
        face.point_normal(Uv {
            u: lo.u + (hi.u - lo.u) * s,
            v: lo.v + (hi.v - lo.v) * t,
        })
        .map(|p| p.point)
    };
    // The longest iso-line each way, from a coarse grid.
    const COARSE: usize = 16;
    let mut long = [0.0_f64; 2];
    for k in 0..=COARSE {
        let fixed = k as f64 / COARSE as f64;
        for (d, line) in long.iter_mut().enumerate() {
            let mut length = 0.0;
            let mut last: Option<Point3> = None;
            for i in 0..=COARSE {
                let free = i as f64 / COARSE as f64;
                let p = if d == 0 {
                    at(free, fixed)
                } else {
                    at(fixed, free)
                };
                if let (Some(a), Some(b)) = (last, p) {
                    length += (b - a).length();
                }
                last = p.or(last);
            }
            *line = line.max(length);
        }
    }
    let nu = ((long[0] / spacing).ceil() as usize).clamp(1, 40_000);
    let nv = ((long[1] / spacing).ceil() as usize).clamp(1, 40_000);
    (0..=nu)
        .flat_map(|i| (0..=nv).map(move |j| (i, j)))
        .filter_map(|(i, j)| {
            let uv = Uv {
                u: lo.u + (hi.u - lo.u) * i as f64 / nu as f64,
                v: lo.v + (hi.v - lo.v) * j as f64 / nv as f64,
            };
            if face.contains(uv) {
                face.point_normal(uv).map(|p| p.point)
            } else {
                None
            }
        })
        .collect()
}

/// Points on the exact surface along the trim loops of `face`, about
/// `spacing` mm apart: each loop's segments in (u, v) are divided until
/// their ends are that close in space.
fn boundary_points(face: &BrepFace, exact: &ExactFace, spacing: f64) -> Vec<Point3> {
    let at = |u: f64, v: f64| exact.point_normal(Uv { u, v }).map(|p| p.point);
    let mut out = Vec::new();
    for l in &face.trims {
        let n = l.0.len();
        for k in 0..n {
            let (Some(a), Some(b)) = (l.0.get(k), l.0.get((k + 1) % n)) else {
                continue;
            };
            let (Some(pa), Some(pb)) = (at(a.u, a.v), at(b.u, b.v)) else {
                continue;
            };
            let steps = (((pb - pa).length() / spacing).ceil() as usize).clamp(1, 10_000);
            for i in 0..steps {
                let t = i as f64 / steps as f64;
                if let Some(p) = at(a.u + (b.u - a.u) * t, a.v + (b.v - a.v) * t) {
                    out.push(p);
                }
            }
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
