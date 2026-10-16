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
