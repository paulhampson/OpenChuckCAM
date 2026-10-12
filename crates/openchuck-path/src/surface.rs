// SPDX-License-Identifier: AGPL-3.0-or-later
//! The `SurfaceSource` port and the library's mesh implementation
//! (architecture section 5.2; decision records 0012 and 0015).
//!
//! A [`SurfaceSource`] answers the geometric questions the strategies ask of
//! the selected surfaces, all in the model frame: the closest point with its
//! normal, the first hit of a ray, the distance from a point, and where a tool
//! dropped along a line first touches (tool-centre solving, decision record
//! 0012 item 4). [`MeshSurface`] implements it for triangle meshes on a
//! `parry3d-f64` bounding volume hierarchy (decision record 0015 item 1); no
//! parry type appears in this crate's public API.
//!
//! A host with its own geometry kernel can implement the trait directly for
//! exact evaluation. How B-rep faces implement it inside the library is
//! settled by SP-9, which has not run (decision record 0016 item 3).

use openchuck_types::geometry::{Aabb, Point3, UnitVec3, Vec3};
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::tool::profile::RevolvedProfile;
use parry3d_f64::math::Vector;
use parry3d_f64::partitioning::BvhNode;
use parry3d_f64::query::{PointQuery, Ray};
use parry3d_f64::shape::TriMesh;

use crate::cutter::{self, Contact, ToolLine};
use crate::error::{PathError, Result};

/// A point on the surface with the outward normal there.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfacePoint {
    /// The point, in the model frame.
    pub point: Point3,
    /// The outward unit normal at the point.
    pub normal: UnitVec3,
}

/// Where a ray first meets the surface.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RayHit {
    /// Distance from the ray's origin along its unit direction.
    pub distance: f64,
    /// The hit point.
    pub point: Point3,
    /// The outward unit normal there.
    pub normal: UnitVec3,
}

/// The selected surfaces of a part, as the strategies see them.
///
/// Every query is in the model frame, whose X axis is the rotary axis.
/// Implementations must be deterministic: the same query gives the same
/// answer on any thread (NFR-06).
pub trait SurfaceSource: Send + Sync + std::fmt::Debug {
    /// A box that holds every point of the surface.
    fn bounds(&self) -> Aabb;

    /// The closest surface point to `point`, with its normal; `None` for an
    /// empty surface.
    fn project(&self, point: Point3) -> Option<SurfacePoint>;

    /// The distance from `point` to the surface.
    fn distance(&self, point: Point3) -> Option<f64> {
        self.project(point).map(|p| p.point.distance(point))
    }

    /// The first surface point on the ray from `origin` along `direction`,
    /// no further than `max_distance`.
    fn cast_ray(&self, origin: Point3, direction: UnitVec3, max_distance: f64) -> Option<RayHit>;

    /// Drops the tool described by `profile` along `line` from the spindle
    /// side, and returns where it first touches: the tip and the contact
    /// point. `None` when the tool would not touch the surface anywhere on
    /// the line.
    fn drop_tool(&self, profile: &RevolvedProfile, line: &ToolLine) -> Option<Contact>;
}

/// A triangle mesh with a bounding volume hierarchy, as a [`SurfaceSource`].
///
/// Normals are facet normals, taken outward from the triangle's winding
/// (counter-clockwise seen from outside, as the mesh checks of IMP-04
/// expect). The mesh's own error against the true surface is not known
/// here; it is the import's or the user's (decision record 0015 item 5).
pub struct MeshSurface {
    bvh: TriMesh,
    triangles: Vec<[Point3; 3]>,
    normals: Vec<Option<UnitVec3>>,
    bounds: Aabb,
}

impl std::fmt::Debug for MeshSurface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MeshSurface")
            .field("triangles", &self.triangles.len())
            .field("bounds", &self.bounds)
            .finish_non_exhaustive()
    }
}

fn to_vector(p: Point3) -> Vector {
    Vector::new(p.x, p.y, p.z)
}

impl MeshSurface {
    /// Builds the hierarchy for `mesh`. Fails when the mesh has no triangles,
    /// an index out of range or a coordinate that is not finite.
    pub fn new(mesh: &TriangleMesh) -> Result<Self> {
        mesh.validate()?;
        if mesh.triangles.is_empty() {
            return Err(PathError::Surface("the mesh has no triangles".to_owned()));
        }
        let mut triangles = Vec::with_capacity(mesh.triangles.len());
        for i in 0..mesh.triangles.len() {
            let t = mesh.triangle(i).ok_or_else(|| {
                PathError::Surface(format!("triangle {i} refers to a missing vertex"))
            })?;
            triangles.push(t);
        }
        let normals = triangles
            .iter()
            .map(|[a, b, c]| UnitVec3::new((*b - *a).cross(*c - *a)).ok())
            .collect();
        let vertices = mesh.vertices.iter().map(|p| to_vector(*p)).collect();
        let bvh = TriMesh::new(vertices, mesh.triangles.clone())
            .map_err(|e| PathError::Surface(format!("cannot index the mesh: {e}")))?;
        let mut min = Point3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
        let mut max = Point3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
        for p in triangles.iter().flatten() {
            min = Point3::new(min.x.min(p.x), min.y.min(p.y), min.z.min(p.z));
            max = Point3::new(max.x.max(p.x), max.y.max(p.y), max.z.max(p.z));
        }
        Ok(Self {
            bvh,
            triangles,
            normals,
            bounds: Aabb { min, max },
        })
    }

    /// Number of triangles.
    pub fn triangle_count(&self) -> usize {
        self.triangles.len()
    }

    /// The triangles whose bounding boxes come within `radius` of the
    /// infinite line, in index order. Conservative: it may return triangles
    /// that are further away, never fewer.
    pub(crate) fn triangles_near_line(&self, line: &ToolLine, radius: f64) -> Vec<u32> {
        let origin = line.origin;
        let axis = line.axis.get();
        let near = |node: &BvhNode| {
            let (lo, hi) = (node.mins(), node.maxs());
            let centre = Point3::new(
                0.5 * (lo.x + hi.x),
                0.5 * (lo.y + hi.y),
                0.5 * (lo.z + hi.z),
            );
            let half = Vec3::new(
                0.5 * (hi.x - lo.x),
                0.5 * (hi.y - lo.y),
                0.5 * (hi.z - lo.z),
            );
            let w = centre - origin;
            let off = w - axis * w.dot(axis);
            off.length() <= radius + half.length()
        };
        let mut found: Vec<u32> = self.bvh.bvh().leaves(near).collect();
        found.sort_unstable();
        found
    }

    /// Triangle `i` and its normal, if `i` is in range.
    pub(crate) fn triangle(&self, i: u32) -> Option<&[Point3; 3]> {
        self.triangles.get(i as usize)
    }

    fn normal(&self, i: u32, fallback: Vec3) -> UnitVec3 {
        self.normals
            .get(i as usize)
            .copied()
            .flatten()
            .or_else(|| UnitVec3::new(fallback).ok())
            .unwrap_or(UnitVec3::Z)
    }
}

impl SurfaceSource for MeshSurface {
    fn bounds(&self) -> Aabb {
        self.bounds
    }

    fn project(&self, point: Point3) -> Option<SurfacePoint> {
        let query = to_vector(point);
        let (i, (_, closest)) = self.bvh.bvh().find_best(
            f64::MAX,
            |node: &BvhNode, _| node.aabb().distance_to_local_point(query, true),
            |i, _| {
                let tri = self.triangles.get(i as usize)?;
                let q = closest_on_triangle(point, tri);
                Some((q.distance(point), q))
            },
        )?;
        Some(SurfacePoint {
            point: closest,
            normal: self.normal(i, point - closest),
        })
    }

    fn cast_ray(&self, origin: Point3, direction: UnitVec3, max_distance: f64) -> Option<RayHit> {
        let d = direction.get();
        let ray = Ray::new(to_vector(origin), Vector::new(d.x, d.y, d.z));
        let (i, t) = self.bvh.bvh().cast_ray(&ray, max_distance, |i, best| {
            let tri = self.triangles.get(i as usize)?;
            ray_triangle(origin, d, tri).filter(|t| *t <= best)
        })?;
        Some(RayHit {
            distance: t,
            point: origin + d * t,
            normal: self.normal(i, -d),
        })
    }

    fn drop_tool(&self, profile: &RevolvedProfile, line: &ToolLine) -> Option<Contact> {
        cutter::drop_on_mesh(self, profile, line)
    }
}

/// The closest point of triangle `t` to `p` (Ericson, Real-Time Collision
/// Detection, 5.1.5), evaluated directly in double precision.
pub(crate) fn closest_on_triangle(p: Point3, t: &[Point3; 3]) -> Point3 {
    let [a, b, c] = *t;
    let ab = b - a;
    let ac = c - a;
    let ap = p - a;
    let d1 = ab.dot(ap);
    let d2 = ac.dot(ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = p - b;
    let d3 = ab.dot(bp);
    let d4 = ac.dot(bp);
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        return a + ab * v;
    }
    let cp = p - c;
    let d5 = ab.dot(cp);
    let d6 = ac.dot(cp);
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        return a + ac * w;
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return b + (c - b) * w;
    }
    let denom = va + vb + vc;
    if denom.abs() <= f64::MIN_POSITIVE {
        // Degenerate triangle: the closest of its three edges.
        let candidates = [
            closest_on_segment(p, a, b),
            closest_on_segment(p, b, c),
            closest_on_segment(p, c, a),
        ];
        return candidates
            .into_iter()
            .min_by(|x, y| x.distance(p).total_cmp(&y.distance(p)))
            .unwrap_or(a);
    }
    let v = vb / denom;
    let w = vc / denom;
    a + ab * v + ac * w
}

fn closest_on_segment(p: Point3, a: Point3, b: Point3) -> Point3 {
    let ab = b - a;
    let l2 = ab.dot(ab);
    if l2 <= f64::MIN_POSITIVE {
        return a;
    }
    a + ab * ((p - a).dot(ab) / l2).clamp(0.0, 1.0)
}

/// Möller and Trumbore's ray and triangle test: the distance along the unit
/// direction `d`, or `None`. Both sides of the triangle are hit.
pub(crate) fn ray_triangle(o: Point3, d: Vec3, t: &[Point3; 3]) -> Option<f64> {
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
