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
//! # B-rep faces (spike SP-9, decision record 0017)
//!
//! Exact faces reach the library as plain data and are evaluated in pure Rust.
//! The face record is `openchuck_types::brep::BrepFace`; the evaluators
//! ([`crate::brep::ExactFace`], a [`ParametricFace`]) and the B-rep source
//! ([`crate::brep::BrepSurface`]) live in [`crate::brep`] (work breakdown
//! slice 11). A B-rep `SurfaceSource` holds its faces as [`ParametricFace`]s, answers
//! [`SurfaceSource::project`] and [`SurfaceSource::cast_ray`] by taking the
//! nearest point and the first trimmed hit over its faces (each answer
//! carries the face and its (u, v) in [`FacePosition`]), and offers its
//! tessellation through [`SurfaceSource::tessellation`] for display, mesh
//! checks and fallback. The five queries of decision record 0017 item 3 are
//! these: point and normal at (u, v) ([`ParametricFace::point_normal`]),
//! closest point ([`ParametricFace::closest`]), trimmed ray hits
//! ([`ParametricFace::ray_hits`]), the trim test
//! ([`ParametricFace::contains`]) and the tessellation. Tool-centre solving
//! on exact faces ([`SurfaceSource::drop_tool`]) is not settled by SP-9 and
//! stays open (OI-81); [`MeshSurface`] answers it on triangles.
//!
//! A host with its own geometry kernel can implement either trait directly
//! for exact evaluation.

use openchuck_types::geometry::{Aabb, Point3, UnitVec3, Vec3};
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::tool::profile::RevolvedProfile;
use parry3d_f64::math::Vector;
use parry3d_f64::partitioning::BvhNode;
use parry3d_f64::query::{PointQuery, Ray};
use parry3d_f64::shape::TriMesh;

use crate::cutter::{self, Contact, ToolLine};
use crate::error::{PathError, Result};
use crate::holes::Hole;

/// A position in a face's parameter space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Uv {
    /// First surface parameter.
    pub u: f64,
    /// Second surface parameter.
    pub v: f64,
}

/// Where on an exact surface an answer lies: the face, by its index in the
/// source, and the parameters on it. Meshes carry none.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FacePosition {
    /// The face's number: its `FaceId`, the deterministic number the import
    /// gave it (decision record 0003), not a position in a selection.
    pub face: u32,
    /// The parameters on that face.
    pub uv: Uv,
}

/// A point on the surface with the outward normal there.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfacePoint {
    /// The point, in the model frame.
    pub point: Point3,
    /// The outward unit normal at the point.
    pub normal: UnitVec3,
    /// The face and parameters, for exact surfaces; `None` for a mesh.
    pub position: Option<FacePosition>,
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
    /// The face and parameters, for exact surfaces; `None` for a mesh.
    pub position: Option<FacePosition>,
}

/// One exact face of a surface, as a strategy that reads the faces
/// themselves sees it (swarf reads their rulings, OP-07; decision record
/// 0061): the face's number and its evaluator.
#[derive(Debug, Clone, Copy)]
pub struct FaceRef<'a> {
    /// The face's number: its `FaceId`, as [`FacePosition::face`].
    pub number: u32,
    /// The face.
    pub face: &'a dyn ParametricFace,
}

/// The closest point of one face to a point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FaceClosest {
    /// The parameters of the closest point.
    pub uv: Uv,
    /// The closest point and its normal (the face's orientation applied).
    pub point: SurfacePoint,
    /// The distance to it.
    pub distance: f64,
}

/// One trimmed face of an exact surface (spike SP-9, decision record 0017
/// item 3). Everything is in the model frame; the normal has the face's
/// orientation applied, so it points out of the material.
pub trait ParametricFace: Send + Sync + std::fmt::Debug {
    /// The parameter box `(min, max)` the face's surface is defined on.
    fn uv_bounds(&self) -> (Uv, Uv);

    /// The point and unit normal at `uv`; `None` outside the parameter box
    /// or where the normal is undefined (a singular point).
    fn point_normal(&self, uv: Uv) -> Option<SurfacePoint>;

    /// The closest point of the trimmed face to `point`.
    fn closest(&self, point: Point3) -> Option<FaceClosest>;

    /// Every hit of the ray with the trimmed face no further than
    /// `max_distance`, nearest first.
    fn ray_hits(&self, origin: Point3, direction: UnitVec3, max_distance: f64) -> Vec<RayHit>;

    /// The trim test: whether `uv` lies on the face, inside its boundary
    /// loops.
    fn contains(&self, uv: Uv) -> bool;
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

    /// Whether `point` lies within `max_distance` of the surface. The same
    /// as comparing [`distance`](Self::distance), which a source may answer
    /// more cheaply by searching only within the limit.
    fn within(&self, point: Point3, max_distance: f64) -> bool {
        self.distance(point).is_some_and(|d| d <= max_distance)
    }

    /// The first surface point on the ray from `origin` along `direction`,
    /// no further than `max_distance`.
    fn cast_ray(&self, origin: Point3, direction: UnitVec3, max_distance: f64) -> Option<RayHit>;

    /// The surface as triangles: the whole surface for a mesh, the display
    /// and fallback form for exact faces.
    fn tessellation(&self) -> &TriangleMesh;

    /// Drops the tool described by `profile` along `line` from the spindle
    /// side, and returns where it first touches: the tip and the contact
    /// point. `None` when the tool would not touch the surface anywhere on
    /// the line.
    fn drop_tool(&self, profile: &RevolvedProfile, line: &ToolLine) -> Option<Contact>;

    /// How far, in millimetres, a [`drop_tool`](Self::drop_tool) answer may
    /// lie below the highest contact on this surface: 0 for a drop solved
    /// to rounding (a mesh, which is its own surface), the exact drop's
    /// tolerance for a B-rep source dropping onto its exact faces (decision
    /// 0277). Tests that compare two drops, or a contact with a surface,
    /// accept differences within it (OI-833).
    fn drop_accuracy(&self) -> f64 {
        0.0
    }

    /// [`drop_tool`](Self::drop_tool) onto the part alone, leaving out any
    /// held region or retaining tab joined to it (a [`crate::CompositeSurface`]);
    /// the same as `drop_tool` for a surface that is only the part. Tells a
    /// free end of the part from the end of a tab (decision record 0344).
    fn drop_on_part(&self, profile: &RevolvedProfile, line: &ToolLine) -> Option<Contact> {
        self.drop_tool(profile, line)
    }

    /// The round holes of the selected surfaces, found from their cylindrical
    /// faces (OP-10; decision record 0051). Only an exact (B-rep) surface has
    /// any: a mesh answers with none.
    fn holes(&self) -> &[Hole] {
        &[]
    }

    /// The exact faces of the selected surfaces, in face order, for a
    /// strategy that reads the faces themselves (swarf fits rulings on each,
    /// OP-07; decision record 0061). Only an exact (B-rep) surface has any:
    /// a mesh answers with none, and such a strategy refuses it.
    fn exact_faces(&self) -> Vec<FaceRef<'_>> {
        Vec::new()
    }
}

/// A triangle mesh with a bounding volume hierarchy, as a [`SurfaceSource`].
///
/// Normals are facet normals, taken outward from the triangle's winding
/// (counter-clockwise seen from outside, as the mesh checks of IMP-04
/// expect). The mesh's own error against the true surface is not known
/// here; it is the import's or the user's (decision record 0015 item 5).
pub struct MeshSurface {
    bvh: TriMesh,
    mesh: TriangleMesh,
    normals: Vec<Option<UnitVec3>>,
    bounds: Aabb,
    /// Two distances within this are equal up to rounding: see
    /// [`PROJECT_TIE`].
    tie: f64,
}

/// The fraction of the mesh's size within which two distances from a point
/// to two triangles count as equal (decision record 0113). A point on a mesh
/// edge or at a vertex is as close to every facet that meets there, up to
/// rounding; which facet a nearest-point search finds first then turns on
/// the platform's last bit (OI-319). [`SurfaceSource::project`] takes every
/// facet within this of the nearest and answers the same from either side.
/// One part in a thousand million of the mesh's diagonal is far above the
/// rounding of any coordinate and far below the written resolution.
pub const PROJECT_TIE: f64 = 1e-9;

impl std::fmt::Debug for MeshSurface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MeshSurface")
            .field("triangles", &self.mesh.triangles.len())
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
        let tie = PROJECT_TIE * (max - min).length().max(1.0);
        Ok(Self {
            bvh,
            mesh: mesh.clone(),
            normals,
            bounds: Aabb { min, max },
            tie,
        })
    }

    /// Number of triangles.
    pub fn triangle_count(&self) -> usize {
        self.mesh.triangles.len()
    }

    /// The triangles that could lift a tool dropped along `line` above
    /// `floor` (a tip height along the line that is certainly reached, or
    /// `None`): those whose bounding box comes within the tool's cutting
    /// radius of the line and whose best possible tip, the box's highest
    /// point less the lowest the profile gets at the box's nearest distance
    /// from the axis, is not below the floor. Conservative: it may return
    /// triangles that cannot lift the tool, never fewer. Index order, in
    /// `found`, which is emptied first.
    pub(crate) fn triangles_for_drop(
        &self,
        line: &ToolLine,
        profile: &RevolvedProfile,
        floor: Option<f64>,
        found: &mut Vec<u32>,
    ) {
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
            let nearest = (off.length() - half.length()).max(0.0);
            let Some(lowest) = profile.height_at(nearest) else {
                return false; // beyond the cutting radius
            };
            let reach = axis.x.abs() * half.x + axis.y.abs() * half.y + axis.z.abs() * half.z;
            let top = w.dot(axis) + reach;
            // Slightly below the floor is kept, so ties are decided as before.
            floor.is_none_or(|f| top - lowest >= f - 1e-9)
        };
        found.clear();
        found.extend(self.bvh.bvh().leaves(near));
        found.sort_unstable();
    }

    /// The highest point of the surface on the infinite line, as a position
    /// along the line from its origin; `None` when the line misses the surface.
    pub(crate) fn highest_on_line(&self, line: &ToolLine) -> Option<f64> {
        let b = self.bounds;
        let size = (b.max - b.min).length() + (line.origin - b.min).length() + 1.0;
        let from = line.origin + line.axis.get() * size;
        let down = UnitVec3::new(-line.axis.get()).ok()?;
        let hit = self.cast_ray(from, down, 2.0 * size)?;
        Some((hit.point - line.origin).dot(line.axis.get()))
    }

    /// Triangle `i` and its normal, if `i` is in range.
    pub(crate) fn triangle(&self, i: u32) -> Option<[Point3; 3]> {
        self.mesh.triangle(i as usize)
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
        let (_, (nearest, _)) = self.bvh.bvh().find_best(
            f64::MAX,
            |node: &BvhNode, _| node.aabb().distance_to_local_point(query, true),
            |i, _| {
                let tri = self.triangle(i)?;
                let q = closest_on_triangle(point, &tri);
                Some((q.distance(point), q))
            },
        )?;
        // Every triangle as near as the nearest, up to rounding: a point on
        // an edge or at a vertex is as close to each facet meeting there, and
        // which the search found first must not decide the answer (decision
        // record 0113, OI-319). The lowest-numbered gives the point, and the
        // normal is the mean of all their normals, the same from either side.
        let limit = nearest + self.tie;
        let mut tied: Vec<(u32, Point3)> = self
            .bvh
            .bvh()
            .leaves(|node: &BvhNode| node.aabb().distance_to_local_point(query, true) <= limit)
            .filter_map(|i| {
                let tri = self.triangle(i)?;
                let q = closest_on_triangle(point, &tri);
                (q.distance(point) <= limit).then_some((i, q))
            })
            .collect();
        tied.sort_unstable_by_key(|(i, _)| *i);
        let (first, closest) = *tied.first()?;
        let sum = tied.iter().fold(Vec3::ZERO, |acc, (i, q)| {
            acc + self.normal(*i, point - *q).get()
        });
        let normal = UnitVec3::new(sum).unwrap_or_else(|_| self.normal(first, point - closest));
        Some(SurfacePoint {
            point: closest,
            normal,
            position: None,
        })
    }

    fn cast_ray(&self, origin: Point3, direction: UnitVec3, max_distance: f64) -> Option<RayHit> {
        let d = direction.get();
        let ray = Ray::new(to_vector(origin), Vector::new(d.x, d.y, d.z));
        let (i, t) = self.bvh.bvh().cast_ray(&ray, max_distance, |i, best| {
            let tri = self.triangle(i)?;
            ray_triangle(origin, d, &tri).filter(|t| *t <= best)
        })?;
        Some(RayHit {
            distance: t,
            point: origin + d * t,
            normal: self.normal(i, -d),
            position: None,
        })
    }

    fn tessellation(&self) -> &TriangleMesh {
        &self.mesh
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
