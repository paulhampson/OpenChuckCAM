// SPDX-License-Identifier: AGPL-3.0-or-later
//! Ray-cast picking: which triangle of a mesh a ray hits first (UI-04; work
//! breakdown slice 10 item 1; architecture sections 5.4 and 5.8).
//!
//! A [`MeshPicker`] builds a bounding volume hierarchy once, when a model is
//! loaded, and answers each ray in logarithmic time. The viewport turns a
//! pointer position into a ray (`Camera::ray`) and asks the picker, so
//! picking is tested here without a window. Both sides of a triangle can be
//! hit, so a ray that starts inside a closed part picks the wall it meets.
//!
//! The answer is exact for the mesh as given: the hierarchy only decides which
//! triangles to test, and the distance comes from our own triangle test in
//! double precision, as in `openchuck-path` (decision record 0015). No
//! `parry3d-f64` type appears in the public API.
//!
//! The time budget is verification plan 4.3 (decision record 0036, OI-12): a
//! ray cast at most 1 ms at the 95th percentile and the hierarchy built in at
//! most 2 s, on a model of one million triangles.

use openchuck_types::geometry::{Aabb, Point3, Vec3};
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::{Error, Result};
use parry3d_f64::math::Vector;
use parry3d_f64::query::Ray;
use parry3d_f64::shape::TriMesh;

/// Where a ray first meets a mesh.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PickHit {
    /// Index of the triangle hit, as in the mesh (and so as in a selection
    /// set, whatever the placement; decision record 0032).
    pub triangle: u32,
    /// Distance from the ray's origin, in millimetres, along the unit
    /// direction.
    pub distance: f64,
    /// The point hit.
    pub point: Point3,
}

/// A mesh with a bounding volume hierarchy, for picking.
pub struct MeshPicker {
    bvh: TriMesh,
    mesh: TriangleMesh,
    bounds: Aabb,
}

impl std::fmt::Debug for MeshPicker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MeshPicker")
            .field("triangles", &self.mesh.triangles.len())
            .field("bounds", &self.bounds)
            .finish_non_exhaustive()
    }
}

/// Two pickers are equal when they were built from the same mesh (the
/// hierarchy follows from it).
impl PartialEq for MeshPicker {
    fn eq(&self, other: &Self) -> bool {
        self.mesh == other.mesh
    }
}

fn to_vector(p: Point3) -> Vector {
    Vector::new(p.x, p.y, p.z)
}

impl MeshPicker {
    /// Builds the hierarchy for `mesh`. Fails when the mesh has no triangles,
    /// an index out of range or a coordinate that is not finite.
    pub fn new(mesh: &TriangleMesh) -> Result<Self> {
        mesh.validate()?;
        if mesh.triangles.is_empty() {
            return Err(Error::invalid("mesh", "the mesh has no triangles"));
        }
        let vertices = mesh.vertices.iter().map(|p| to_vector(*p)).collect();
        let bvh = TriMesh::new(vertices, mesh.triangles.clone())
            .map_err(|e| Error::invalid("mesh", format!("cannot index the mesh: {e}")))?;
        let mut min = Point3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
        let mut max = Point3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
        for p in mesh
            .triangles
            .iter()
            .flatten()
            .filter_map(|&i| mesh.vertices.get(i as usize))
        {
            min = Point3::new(min.x.min(p.x), min.y.min(p.y), min.z.min(p.z));
            max = Point3::new(max.x.max(p.x), max.y.max(p.y), max.z.max(p.z));
        }
        Ok(Self {
            bvh,
            mesh: mesh.clone(),
            bounds: Aabb { min, max },
        })
    }

    /// Number of triangles.
    pub fn triangle_count(&self) -> usize {
        self.mesh.triangles.len()
    }

    /// The box holding every triangle.
    pub fn bounds(&self) -> Aabb {
        self.bounds
    }

    /// The unit normal of `triangle` (right-hand rule on its winding), or
    /// `None` for a triangle that is not in the mesh or has no area.
    pub fn normal(&self, triangle: u32) -> Option<Vec3> {
        let [a, b, c] = self.mesh.triangle(triangle as usize)?;
        let n = (b - a).cross(c - a);
        let length = n.length();
        (length.is_finite() && length > 1e-300).then(|| n * (1.0 / length))
    }

    /// The first triangle the ray from `origin` along `direction` hits, or
    /// `None` when it hits nothing. `direction` need not be a unit vector.
    /// Fails when the origin or direction is not finite, or the direction is
    /// zero. Of two triangles at the same distance (an edge or vertex hit)
    /// the answer is the same on every run.
    pub fn pick(&self, origin: Point3, direction: Vec3) -> Result<Option<PickHit>> {
        if !origin.is_finite() || !direction.is_finite() {
            return Err(Error::invalid(
                "ray",
                "the origin and direction must be finite",
            ));
        }
        let length = direction.length();
        if !(length.is_finite() && length > 1e-300) {
            return Err(Error::invalid("ray", "the direction must not be zero"));
        }
        let d = direction * (1.0 / length);
        let ray = Ray::new(to_vector(origin), Vector::new(d.x, d.y, d.z));
        let hit = self.bvh.bvh().cast_ray(&ray, f64::MAX, |i, best| {
            let tri = self.mesh.triangle(i as usize)?;
            ray_triangle(origin, d, &tri).filter(|t| *t <= best)
        });
        Ok(hit.map(|(triangle, distance)| PickHit {
            triangle,
            distance,
            point: origin + d * distance,
        }))
    }
}

/// Distance along unit `d` from `o` to triangle `t` (Möller and Trumbore),
/// both faces, or `None` when the ray misses it or points away.
fn ray_triangle(o: Point3, d: Vec3, t: &[Point3; 3]) -> Option<f64> {
    let [a, b, c] = *t;
    let e1 = b - a;
    let e2 = c - a;
    let p = d.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-300 {
        return None;
    }
    let inv = 1.0 / det;
    let s = o - a;
    let u = s.dot(p) * inv;
    if !(-1e-12..=1.0 + 1e-12).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let v = d.dot(q) * inv;
    if v < -1e-12 || u + v > 1.0 + 1e-12 {
        return None;
    }
    let dist = e2.dot(q) * inv;
    (dist >= 0.0).then_some(dist)
}

/// How far from a ray an edge may be and still be picked, in millimetres, at
/// a distance `t` along the ray: `base + per_distance * t` (slice 11 item 5;
/// decision record 0066). A front end makes it a number of pixels: for a
/// perspective view `base` is 0 and `per_distance` the angle the pixels span;
/// for an orthographic view `per_distance` is 0 and `base` the width the
/// pixels span.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PickTolerance {
    /// The reach at the ray's origin, millimetres.
    pub base: f64,
    /// How much the reach grows for each millimetre along the ray.
    pub per_distance: f64,
}

impl PickTolerance {
    /// The reach at distance `t` along the ray.
    pub fn at(&self, t: f64) -> f64 {
        self.base + self.per_distance * t
    }

    fn is_valid(&self) -> bool {
        self.base.is_finite()
            && self.per_distance.is_finite()
            && self.base >= 0.0
            && self.per_distance >= 0.0
            && (self.base > 0.0 || self.per_distance > 0.0)
    }
}

/// The edge a ray passes nearest, within the tolerance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EdgeHit {
    /// Index of the polyline, in the order the picker was given them.
    pub edge: usize,
    /// Index of the segment in the polyline: from point `segment` to point
    /// `segment + 1`.
    pub segment: usize,
    /// Distance along the unit ray to its point nearest the edge, millimetres.
    pub distance: f64,
    /// How far the ray passes from the edge there, millimetres.
    pub offset: f64,
    /// The point of the edge nearest the ray.
    pub point: Point3,
}

/// Polylines with their bounds, for picking edges near a ray (UI-04 for
/// B-rep edges and mesh edges; slice 11 item 5).
///
/// Each polyline is tested segment by segment, after a test against its box
/// grown by the tolerance; the drawn edges of a part number in the thousands,
/// not millions, so no hierarchy is built.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EdgePicker {
    polylines: Vec<Vec<Point3>>,
    boxes: Vec<Aabb>,
}

impl EdgePicker {
    /// Takes the polylines. Fails when a point is not finite; a polyline of
    /// fewer than two points can never be picked.
    pub fn new(polylines: Vec<Vec<Point3>>) -> Result<Self> {
        let mut boxes = Vec::with_capacity(polylines.len());
        for (i, line) in polylines.iter().enumerate() {
            let mut min = Point3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
            let mut max = Point3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
            for p in line {
                if !p.is_finite() {
                    return Err(Error::invalid(
                        format!("edge {i}"),
                        "every point must be finite",
                    ));
                }
                min = Point3::new(min.x.min(p.x), min.y.min(p.y), min.z.min(p.z));
                max = Point3::new(max.x.max(p.x), max.y.max(p.y), max.z.max(p.z));
            }
            boxes.push(Aabb { min, max });
        }
        Ok(Self { polylines, boxes })
    }

    /// Number of polylines.
    pub fn len(&self) -> usize {
        self.polylines.len()
    }

    /// Whether there are no polylines.
    pub fn is_empty(&self) -> bool {
        self.polylines.is_empty()
    }

    /// Polyline `edge`, as given.
    pub fn polyline(&self, edge: usize) -> Option<&[Point3]> {
        self.polylines.get(edge).map(Vec::as_slice)
    }

    /// The edge the ray from `origin` along `direction` passes nearest,
    /// measured against the tolerance there (the smallest share of the reach),
    /// considering only points of the ray no farther than `max_distance` (the
    /// surface hit, so an edge behind the part is not picked; infinity for
    /// none). Of two equally near, the one nearer the origin, then the lower
    /// index, so the answer is the same on every run. Fails when the ray or
    /// the tolerance is not valid.
    pub fn pick(
        &self,
        origin: Point3,
        direction: Vec3,
        tolerance: PickTolerance,
        max_distance: f64,
    ) -> Result<Option<EdgeHit>> {
        if !origin.is_finite() || !direction.is_finite() {
            return Err(Error::invalid(
                "ray",
                "the origin and direction must be finite",
            ));
        }
        let length = direction.length();
        if !(length.is_finite() && length > 1e-300) {
            return Err(Error::invalid("ray", "the direction must not be zero"));
        }
        if !tolerance.is_valid() || max_distance.is_nan() || max_distance < 0.0 {
            return Err(Error::invalid(
                "tolerance",
                "the reach must be finite, not negative and not zero, and the farthest \
                 distance not negative",
            ));
        }
        let d = direction * (1.0 / length);
        let mut best: Option<(f64, EdgeHit)> = None;
        for (edge, (line, b)) in self.polylines.iter().zip(&self.boxes).enumerate() {
            if line.len() < 2 {
                continue;
            }
            // The farthest the box is from the origin bounds the reach there.
            let far = [b.min.x, b.max.x]
                .iter()
                .flat_map(|&x| [b.min.y, b.max.y].map(move |y| (x, y)))
                .flat_map(|(x, y)| [b.min.z, b.max.z].map(move |z| Point3::new(x, y, z)))
                .map(|p| p.distance(origin))
                .fold(0.0, f64::max);
            let reach = tolerance.at(far.min(max_distance));
            if !ray_meets_box(origin, d, b, reach, max_distance) {
                continue;
            }
            for (segment, pair) in line.windows(2).enumerate() {
                let &[a, c] = pair else { continue };
                let (t, q) = ray_segment(origin, d, a, c);
                if t > max_distance {
                    continue;
                }
                let offset = (origin + d * t).distance(q);
                let allowed = tolerance.at(t);
                if offset > allowed {
                    continue;
                }
                let share = offset / allowed;
                let better = best.is_none_or(|(s, h)| {
                    share.total_cmp(&s).then(t.total_cmp(&h.distance)).is_lt()
                });
                if better {
                    best = Some((
                        share,
                        EdgeHit {
                            edge,
                            segment,
                            distance: t,
                            offset,
                            point: q,
                        },
                    ));
                }
            }
        }
        Ok(best.map(|(_, hit)| hit))
    }
}

/// Whether the ray from `o` along unit `d`, up to `max`, meets box `b` grown
/// by `grow` on every side (slab test).
fn ray_meets_box(o: Point3, d: Vec3, b: &Aabb, grow: f64, max: f64) -> bool {
    let mut near = 0.0_f64;
    let mut far = max;
    for (origin, dir, lo, hi) in [
        (o.x, d.x, b.min.x, b.max.x),
        (o.y, d.y, b.min.y, b.max.y),
        (o.z, d.z, b.min.z, b.max.z),
    ] {
        let (lo, hi) = (lo - grow, hi + grow);
        if dir.abs() < 1e-300 {
            if origin < lo || origin > hi {
                return false;
            }
            continue;
        }
        let (t0, t1) = ((lo - origin) / dir, (hi - origin) / dir);
        near = near.max(t0.min(t1));
        far = far.min(t0.max(t1));
        if near > far {
            return false;
        }
    }
    true
}

/// The point of the ray from `o` along unit `d` (`t >= 0`) and the point of
/// segment `a`–`c` that are nearest each other: `(t, point on the segment)`.
fn ray_segment(o: Point3, d: Vec3, a: Point3, c: Point3) -> (f64, Point3) {
    let u = c - a;
    let w = a - o;
    let uu = u.dot(u);
    let on_segment = |p: Point3| {
        if uu <= 1e-300 {
            return a;
        }
        let s = ((p - a).dot(u) / uu).clamp(0.0, 1.0);
        a + u * s
    };
    let ud = u.dot(d);
    let denominator = uu - ud * ud;
    // The segment's parameter where the lines are nearest, clamped; for
    // parallel lines any point will do, so take the segment's start.
    let s = if uu <= 1e-300 || denominator <= 1e-12 * uu {
        0.0
    } else {
        ((ud * w.dot(d) - w.dot(u)) / denominator).clamp(0.0, 1.0)
    };
    let q = a + u * s;
    let t = (q - o).dot(d);
    if t >= 0.0 {
        // With the ray's point fixed, the segment's nearest point may move.
        let q = on_segment(o + d * t);
        let t = (q - o).dot(d).max(0.0);
        (t, q)
    } else {
        (0.0, on_segment(o))
    }
}
