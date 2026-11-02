// SPDX-License-Identifier: AGPL-3.0-or-later
//! The sampled M2 check that decision 0211 found unqualified, kept only so
//! the regression of issue #346 can show what it misses: points on a
//! surface read against the testkit's tool outline. Its nominal spacing is
//! no bound: on decision 0211's flat-cutter plane it reads -0.00333 mm where
//! the exact clearance is -0.01 mm. M2 uses `bounded.rs`.

use std::collections::BTreeMap;

use openchuck_path::ExactFace;
use openchuck_path::surface::{ParametricFace, Uv};
use openchuck_testkit::gouge::Outline;
use openchuck_types::brep::{BrepFace, BrepModel};
use openchuck_types::geometry::{Point3, Vec3};
use openchuck_types::mesh::TriangleMesh;
use rayon::prelude::*;

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

/// Points on a surface, bucketed in cubes for the search round a tool.
#[derive(Debug, Default)]
pub struct Cloud {
    cell: f64,
    cells: BTreeMap<[i64; 3], Vec<Point3>>,
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
