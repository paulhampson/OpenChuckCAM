// SPDX-License-Identifier: AGPL-3.0-or-later
//! Analytic solids for workholding and retaining tabs (requirements SET-09
//! and OP-21; work breakdown slice 21b; decision record 0035 item 2).
//!
//! A [`Solid`] is a capped cylinder or frustum on any axis, or a box with
//! faces parallel to the model's planes: the held region of a setup (a
//! cylinder about the rotary axis on round stock, the stock's own box section
//! on box stock; decision record 0160) and each tab. It answers the
//! queries of [`SurfaceSource`](crate::SurfaceSource) in closed form, so a
//! [`CompositeSurface`](crate::CompositeSurface) can join it to the model's
//! surface:
//!
//! - **closest point**: the problem reduces to the solid's half-plane section
//!   (axial position `s`, radius `rho`), where the boundary is three line
//!   segments, so the answer is exact;
//! - **ray**: the two cap discs and the lateral cone, which is a quadratic
//!   in the ray parameter (a cylinder is the case of equal radii);
//! - **tool drop**: the tool tip rests at the maximum of `z - h(r)` over the
//!   solid (`z` along the tool axis, `r` the distance from it, `h` the
//!   profile's lower envelope). That maximum is on the boundary, which is
//!   ruled: for a section plane at angle `theta` it is three segments (the
//!   bottom cap's radius, the lateral generator, the top cap's radius). On a
//!   segment `z - h(r)` is concave for a convex profile, so a golden-section
//!   search is exact to rounding. Across `theta` the best of the segments is
//!   quasi-concave for a convex profile, so it is sampled on the arc of
//!   angles the tool can reach and refined about the best sample. A concave
//!   custom profile is sampled more finely and is accurate to the sampling
//!   (the mesh drop-cutter splits at every profile piece instead).
//!
//! A box answers the same queries exactly: the closest point by clamping to
//! it, the ray by the slab method, and the tool drop by the mesh
//! drop-cutter's vertex, edge and facet tests on its twelve triangles.
//!
//! Everything is double precision, millimetres and the model frame, and
//! nothing depends on threads or randomness, so results are identical at any
//! thread count (NFR-06). Bad input is an error, never a panic (NFR-07).

use std::f64::consts::{PI, TAU};

use openchuck_types::geometry::{Aabb, Point3, UnitVec3, Vec3};
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::tool::profile::RevolvedProfile;

use crate::cutter::{Contact, ToolLine};
use crate::error::{PathError, Result};
use crate::surface::{RayHit, SurfacePoint};

/// A capped cylinder or frustum (the solid between two parallel discs), or a
/// box with faces parallel to the model's planes.
///
/// The base disc is centred on `base`, the top disc lies `length` along
/// `axis`; the radius changes linearly between them (equal radii give a
/// cylinder). All dimensions are positive: a point or a flat disc is not a
/// solid (OP-21 validation). A box ([`Solid::cuboid`]) reads as a solid along
/// X: its base is the centre of its low-X face, its length its X extent, and
/// both radii the radius of the circle about its X centre line through the
/// corners of its section.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Solid {
    base: Point3,
    axis: UnitVec3,
    e1: Vec3,
    e2: Vec3,
    length: f64,
    base_radius: f64,
    top_radius: f64,
    /// The box, for a solid made by [`Solid::cuboid`].
    cuboid: Option<Aabb>,
}

fn positive(name: &str, value: f64) -> Result<()> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        Err(PathError::Surface(format!(
            "the {name} {value} must be a finite number above zero"
        )))
    }
}

impl Solid {
    /// A cylinder of `radius` and `length` from the disc centred on `base`,
    /// along `axis`.
    pub fn cylinder(base: Point3, axis: UnitVec3, length: f64, radius: f64) -> Result<Self> {
        Self::frustum(base, axis, length, radius, radius)
    }

    /// A frustum from the disc centred on `base` (radius `base_radius`) to a
    /// disc `length` along `axis` (radius `top_radius`).
    pub fn frustum(
        base: Point3,
        axis: UnitVec3,
        length: f64,
        base_radius: f64,
        top_radius: f64,
    ) -> Result<Self> {
        if !base.is_finite() {
            return Err(PathError::Surface(
                "the solid's base point must be finite".to_owned(),
            ));
        }
        positive("length", length)?;
        positive("base radius", base_radius)?;
        positive("top radius", top_radius)?;
        let a = axis.get();
        // The global axis least aligned with `a` seeds the frame, so that
        // `e1 x e2 = a`.
        let seed = if a.x.abs() <= a.y.abs() && a.x.abs() <= a.z.abs() {
            Vec3::new(1.0, 0.0, 0.0)
        } else if a.y.abs() <= a.z.abs() {
            Vec3::new(0.0, 1.0, 0.0)
        } else {
            Vec3::new(0.0, 0.0, 1.0)
        };
        let e1 = UnitVec3::new(seed - a * seed.dot(a))
            .map_err(|_| PathError::Surface("the solid's axis is degenerate".to_owned()))?
            .get();
        let e2 = a.cross(e1);
        Ok(Self {
            base,
            axis,
            e1,
            e2,
            length,
            base_radius,
            top_radius,
            cuboid: None,
        })
    }

    /// A box with faces parallel to the model's planes, from `bounds.min` to
    /// `bounds.max`: the held region of box stock (SET-09; decision record
    /// 0160). Every side must be finite and above zero.
    pub fn cuboid(bounds: Aabb) -> Result<Self> {
        if !bounds.min.is_finite() || !bounds.max.is_finite() {
            return Err(PathError::Surface(
                "the box's corners must be finite".to_owned(),
            ));
        }
        let size = bounds.max - bounds.min;
        positive("box's length along X", size.x)?;
        positive("box's width along Y", size.y)?;
        positive("box's height along Z", size.z)?;
        let (cy, cz) = (
            (bounds.min.y + bounds.max.y) / 2.0,
            (bounds.min.z + bounds.max.z) / 2.0,
        );
        let corner = (size.y / 2.0).hypot(size.z / 2.0);
        let mut solid = Self::cylinder(
            Point3::new(bounds.min.x, cy, cz),
            UnitVec3::X,
            size.x,
            corner,
        )?;
        solid.cuboid = Some(bounds);
        Ok(solid)
    }

    /// The box, for a solid made by [`Solid::cuboid`]; `None` for a cylinder
    /// or frustum.
    pub fn as_cuboid(&self) -> Option<Aabb> {
        self.cuboid
    }

    /// The centre of the base disc.
    pub fn base(&self) -> Point3 {
        self.base
    }

    /// The axis, from the base disc to the top disc.
    pub fn axis(&self) -> UnitVec3 {
        self.axis
    }

    /// The distance between the discs.
    pub fn length(&self) -> f64 {
        self.length
    }

    /// The radius of the base disc.
    pub fn base_radius(&self) -> f64 {
        self.base_radius
    }

    /// The radius of the top disc.
    pub fn top_radius(&self) -> f64 {
        self.top_radius
    }

    /// The centre of the top disc.
    pub fn top(&self) -> Point3 {
        self.base + self.axis.get() * self.length
    }

    /// The exact volume, `pi/3 L (R0^2 + R0 R1 + R1^2)`.
    pub fn volume(&self) -> f64 {
        if let Some(b) = self.cuboid {
            let d = b.max - b.min;
            return d.x * d.y * d.z;
        }
        let (r0, r1) = (self.base_radius, self.top_radius);
        PI / 3.0 * self.length * (r0 * r0 + r0 * r1 + r1 * r1)
    }

    /// The exact area of the two discs and the side.
    pub fn surface_area(&self) -> f64 {
        if let Some(b) = self.cuboid {
            let d = b.max - b.min;
            return 2.0 * (d.x * d.y + d.y * d.z + d.z * d.x);
        }
        let (r0, r1) = (self.base_radius, self.top_radius);
        let slant = self.length.hypot(r1 - r0);
        PI * (r0 * r0 + r1 * r1 + (r0 + r1) * slant)
    }

    /// True when `point` lies inside the solid or on its surface.
    pub fn contains(&self, point: Point3) -> bool {
        if let Some(b) = self.cuboid {
            return (b.min.x..=b.max.x).contains(&point.x)
                && (b.min.y..=b.max.y).contains(&point.y)
                && (b.min.z..=b.max.z).contains(&point.z);
        }
        let (s, w) = self.split(point);
        (0.0..=self.length).contains(&s) && w.length() <= self.radius_at(s)
    }

    /// A box that holds the whole solid.
    pub fn bounds(&self) -> Aabb {
        if let Some(b) = self.cuboid {
            return b;
        }
        let a = self.axis.get();
        let top = self.top();
        // A disc of radius R about a unit axis spans R sqrt(1 - a_i^2) along
        // coordinate i.
        let span = |r: f64, k: f64| r * (1.0 - k * k).max(0.0).sqrt();
        let lo = |b: f64, t: f64, k: f64| {
            (b - span(self.base_radius, k)).min(t - span(self.top_radius, k))
        };
        let hi = |b: f64, t: f64, k: f64| {
            (b + span(self.base_radius, k)).max(t + span(self.top_radius, k))
        };
        Aabb {
            min: Point3::new(
                lo(self.base.x, top.x, a.x),
                lo(self.base.y, top.y, a.y),
                lo(self.base.z, top.z, a.z),
            ),
            max: Point3::new(
                hi(self.base.x, top.x, a.x),
                hi(self.base.y, top.y, a.y),
                hi(self.base.z, top.z, a.z),
            ),
        }
    }

    /// The solid as triangles with `segments` around, counter-clockwise seen
    /// from outside. Used for display and fallback; the queries do not use it.
    pub fn mesh(&self, segments: u32) -> TriangleMesh {
        if let Some(b) = self.cuboid {
            return box_mesh(b);
        }
        let n = segments.max(3);
        let top = self.top();
        let mut vertices = vec![self.base, top];
        for i in 0..n {
            let d = self.direction(TAU * f64::from(i) / f64::from(n));
            vertices.push(self.base + d * self.base_radius);
        }
        for i in 0..n {
            let d = self.direction(TAU * f64::from(i) / f64::from(n));
            vertices.push(top + d * self.top_radius);
        }
        let bottom = |i: u32| 2 + i % n;
        let upper = |i: u32| 2 + n + i % n;
        let mut triangles = Vec::new();
        for i in 0..n {
            triangles.push([0, bottom(i + 1), bottom(i)]);
            triangles.push([1, upper(i), upper(i + 1)]);
            triangles.push([bottom(i), bottom(i + 1), upper(i + 1)]);
            triangles.push([bottom(i), upper(i + 1), upper(i)]);
        }
        TriangleMesh {
            vertices,
            triangles,
        }
    }

    /// The closest point of the surface to `point`, with the outward normal.
    /// Exact; for a point inside, the closest point of the boundary.
    pub fn project(&self, point: Point3) -> Option<SurfacePoint> {
        if !point.is_finite() {
            return None;
        }
        if let Some(b) = self.cuboid {
            return Some(box_project(b, point));
        }
        let (s, w) = self.split(point);
        let rho = w.length();
        let dir = if rho > 1e-300 {
            w * (1.0 / rho)
        } else {
            self.e1
        };
        let (l, r0, r1) = (self.length, self.base_radius, self.top_radius);
        let slant = l.hypot(r1 - r0);
        // The boundary in the (s, rho) half-plane, with each piece's outward
        // normal as (n_s, n_rho).
        let pieces = [
            ((0.0, 0.0), (0.0, r0), (-1.0, 0.0)),
            ((0.0, r0), (l, r1), ((r0 - r1) / slant, l / slant)),
            ((l, r1), (l, 0.0), (1.0, 0.0)),
        ];
        let mut best: Option<Nearest> = None;
        for (a, b, n) in pieces {
            let q = closest_2d((s, rho), a, b);
            let d2 = (q.0 - s).powi(2) + (q.1 - rho).powi(2);
            if best.is_none_or(|b| d2 < b.d2) {
                best = Some(Nearest { d2, q, n });
            }
        }
        let Nearest { q, n, .. } = best?;
        let a = self.axis.get();
        Some(SurfacePoint {
            point: self.base + a * q.0 + dir * q.1,
            normal: UnitVec3::new(a * n.0 + dir * n.1).unwrap_or(self.axis),
            position: None,
        })
    }

    /// The first point of the surface on the ray, no further than
    /// `max_distance`. A ray that starts inside meets the surface where it
    /// leaves.
    pub fn cast_ray(
        &self,
        origin: Point3,
        direction: UnitVec3,
        max_distance: f64,
    ) -> Option<RayHit> {
        if !origin.is_finite() {
            return None;
        }
        if let Some(b) = self.cuboid {
            return box_ray(b, origin, direction, max_distance);
        }
        let a = self.axis.get();
        let d = direction.get();
        let (o_s, o_w) = self.split(origin);
        let d_s = d.dot(a);
        let d_w = d - a * d_s;
        let (l, r0, r1) = (self.length, self.base_radius, self.top_radius);
        let reach = max_distance.min(f64::MAX);
        let mut best: Option<(f64, Vec3)> = None;
        let mut offer = |t: f64, normal: Vec3| {
            if (0.0..=reach).contains(&t) && best.is_none_or(|(bt, _)| t < bt) {
                best = Some((t, normal));
            }
        };
        // The discs.
        for (plane, radius, n_s) in [(0.0, r0, -1.0), (l, r1, 1.0)] {
            if d_s.abs() > 1e-14 {
                let t = (plane - o_s) / d_s;
                if (o_w + d_w * t).length() <= radius * (1.0 + 1e-12) {
                    offer(t, a * n_s);
                }
            }
        }
        // The side: |o_w + d_w t|^2 = (rho_o + k d_s t)^2.
        let k = (r1 - r0) / l;
        let rho_o = r0 + k * o_s;
        let qa = d_w.dot(d_w) - (k * d_s).powi(2);
        let qb = o_w.dot(d_w) - rho_o * k * d_s;
        let qc = o_w.dot(o_w) - rho_o * rho_o;
        let slant = l.hypot(r1 - r0);
        let mut roots = [None, None];
        if qa.abs() < 1e-14 {
            if qb.abs() > 1e-300 {
                roots[0] = Some(-qc / (2.0 * qb));
            }
        } else {
            let disc = qb * qb - qa * qc;
            if disc >= 0.0 {
                // The stable form: no cancellation between -qb and the root.
                let q = -(qb + qb.signum() * disc.sqrt());
                roots[0] = Some(q / qa);
                if q.abs() > 1e-300 {
                    roots[1] = Some(qc / q);
                }
            }
        }
        for t in roots.into_iter().flatten() {
            let s = o_s + d_s * t;
            if (0.0..=l).contains(&s) {
                let w = o_w + d_w * t;
                let rho = w.length();
                if rho > 1e-300 {
                    let dir = w * (1.0 / rho);
                    offer(t, a * ((r0 - r1) / slant) + dir * (l / slant));
                }
            }
        }
        let (t, normal) = best?;
        Some(RayHit {
            distance: t,
            point: origin + d * t,
            normal: UnitVec3::new(normal).unwrap_or(self.axis),
            position: None,
        })
    }

    /// Drops the tool described by `profile` along `line` from the spindle
    /// side and returns where it first touches the solid; `None` when the
    /// tool would not touch it anywhere on the line.
    pub fn drop_tool(&self, profile: &RevolvedProfile, line: &ToolLine) -> Option<Contact> {
        if let Some(b) = self.cuboid {
            // A box out of the tool's reach gives nothing: every candidate of
            // the triangle tests lies within the cutting radius of the line.
            // Saying so at once gives the same answer (issue #382).
            if box_line_gap(b, line) > profile.cutting_radius() + BOX_REACH_SLACK {
                return None;
            }
            let mesh = box_mesh(b);
            let triangles: Vec<[Point3; 3]> = mesh
                .triangles
                .iter()
                .filter_map(|t| {
                    let p = |i: u32| mesh.vertices.get(usize::try_from(i).ok()?).copied();
                    Some([p(t[0])?, p(t[1])?, p(t[2])?])
                })
                .collect();
            return crate::cutter::drop_on_triangles(&triangles, profile, line);
        }
        let tool = ToolFrame {
            origin: line.origin,
            axis: line.axis.get(),
        };
        let reach = profile.cutting_radius();
        // Quick rejection: the solid lies inside a capsule about its axis.
        let near = tool.segment_distance(self.base, self.top());
        if near - self.base_radius.max(self.top_radius) > reach {
            return None;
        }
        let convex = profile.is_convex();
        let section = |theta: f64| {
            let d = self.direction(theta);
            let (bottom, top) = (
                self.base + d * self.base_radius,
                self.top() + d * self.top_radius,
            );
            [(self.base, bottom), (bottom, top), (top, self.top())]
        };
        let distance = |theta: f64| {
            section(theta)
                .iter()
                .map(|(p, q)| tool.segment_distance(*p, *q))
                .fold(f64::INFINITY, f64::min)
        };
        let best_at = |theta: f64| -> Option<(f64, Point3)> {
            let mut best: Option<(f64, Point3)> = None;
            for (p, q) in section(theta) {
                if let Some(c) = segment_best(&tool, profile, convex, p, q)
                    && best.is_none_or(|b| c.0 > b.0)
                {
                    best = Some(c);
                }
            }
            best
        };
        let samples = if convex { 16 } else { 96 };
        let mut best: Option<(f64, Point3)> = None;
        for span in reachable_arcs(&distance, reach) {
            if let Some(c) = best_on_arc(&best_at, span, samples)
                && best.is_none_or(|b| c.0 > b.0)
            {
                best = Some(c);
            }
        }
        best.map(|(offset, contact)| Contact {
            tip: line.at(offset),
            contact,
            offset,
        })
    }

    /// The furthest any point of the solid lies along `line`'s axis, as a
    /// position from its origin: a box's highest corner, or the highest point
    /// of a cylinder's or frustum's two rims (a linear function peaks on the
    /// rim of a disc, and a frustum is the hull of its two discs). The
    /// profile never dips below its tip (`h(r) >= 0`), so no tip
    /// [`drop_tool`](Self::drop_tool) gives lies further along the line than
    /// this, up to rounding.
    pub(crate) fn highest_along(&self, line: &ToolLine) -> f64 {
        let u = line.axis.get();
        let along = |p: Point3| (p - line.origin).dot(u);
        if let Some(b) = self.cuboid {
            return (0..8u8)
                .map(|i| {
                    along(Point3::new(
                        if i & 1 == 0 { b.min.x } else { b.max.x },
                        if i & 2 == 0 { b.min.y } else { b.max.y },
                        if i & 4 == 0 { b.min.z } else { b.max.z },
                    ))
                })
                .fold(f64::NEG_INFINITY, f64::max);
        }
        let tilt = self.axis.get().dot(u);
        let across = (1.0 - tilt * tilt).max(0.0).sqrt();
        (along(self.base) + self.base_radius * across)
            .max(along(self.top()) + self.top_radius * across)
    }

    fn split(&self, p: Point3) -> (f64, Vec3) {
        let v = p - self.base;
        let a = self.axis.get();
        let s = v.dot(a);
        (s, v - a * s)
    }

    fn radius_at(&self, s: f64) -> f64 {
        self.base_radius + (self.top_radius - self.base_radius) * (s / self.length)
    }

    /// The unit direction at polar angle `theta` about the axis.
    fn direction(&self, theta: f64) -> Vec3 {
        self.e1 * theta.cos() + self.e2 * theta.sin()
    }
}

/// Rounding allowed for in the reach test of a box, mm: far above the
/// rounding of the distances involved.
const BOX_REACH_SLACK: f64 = 1e-7;

/// A distance no greater than the box `b`'s from `line`. The distance from a
/// point to the line is at least its offset along any unit direction across
/// the line, and over a box that offset is linear, so its least size is
/// known exactly; the directions taken are each model axis with its part
/// along the line removed.
fn box_line_gap(b: Aabb, line: &ToolLine) -> f64 {
    let u = line.axis.get();
    let centre = Point3::new(
        0.5 * (b.min.x + b.max.x),
        0.5 * (b.min.y + b.max.y),
        0.5 * (b.min.z + b.max.z),
    );
    let half = Vec3::new(
        0.5 * (b.max.x - b.min.x),
        0.5 * (b.max.y - b.min.y),
        0.5 * (b.max.z - b.min.z),
    );
    let w = centre - line.origin;
    [
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
        Vec3::new(0.0, 0.0, 1.0),
    ]
    .into_iter()
    .filter_map(|e| UnitVec3::new(e - u * e.dot(u)).ok())
    .map(|across| {
        let a = across.get();
        let spread = half.x * a.x.abs() + half.y * a.y.abs() + half.z * a.z.abs();
        w.dot(a).abs() - spread
    })
    .fold(0.0, f64::max)
}

/// The eight corners of `b`, indexed by bit 0 = high X, bit 1 = high Y and
/// bit 2 = high Z, as twelve triangles counter-clockwise seen from outside.
fn box_mesh(b: Aabb) -> TriangleMesh {
    let vertices = (0..8u32)
        .map(|i| {
            Point3::new(
                if i & 1 == 0 { b.min.x } else { b.max.x },
                if i & 2 == 0 { b.min.y } else { b.max.y },
                if i & 4 == 0 { b.min.z } else { b.max.z },
            )
        })
        .collect();
    // Each face as a quad, counter-clockwise seen from outside.
    let quads: [[u32; 4]; 6] = [
        [0, 4, 6, 2], // -X
        [1, 3, 7, 5], // +X
        [0, 1, 5, 4], // -Y
        [2, 6, 7, 3], // +Y
        [0, 2, 3, 1], // -Z
        [4, 5, 7, 6], // +Z
    ];
    let triangles = quads
        .iter()
        .flat_map(|[a, b, c, d]| [[*a, *b, *c], [*a, *c, *d]])
        .collect();
    TriangleMesh {
        vertices,
        triangles,
    }
}

/// The closest point of the box's surface to `p`, with the outward normal:
/// for a point outside, the point clamped to the box (the normal from it to
/// `p`); for a point inside or on it, the nearest face (ties to the first in
/// the order -X, +X, -Y, +Y, -Z, +Z).
fn box_project(b: Aabb, p: Point3) -> SurfacePoint {
    let q = Point3::new(
        p.x.clamp(b.min.x, b.max.x),
        p.y.clamp(b.min.y, b.max.y),
        p.z.clamp(b.min.z, b.max.z),
    );
    if let Ok(normal) = UnitVec3::new(p - q) {
        return SurfacePoint {
            point: q,
            normal,
            position: None,
        };
    }
    let faces = [
        (p.x - b.min.x, Vec3::new(-1.0, 0.0, 0.0)),
        (b.max.x - p.x, Vec3::new(1.0, 0.0, 0.0)),
        (p.y - b.min.y, Vec3::new(0.0, -1.0, 0.0)),
        (b.max.y - p.y, Vec3::new(0.0, 1.0, 0.0)),
        (p.z - b.min.z, Vec3::new(0.0, 0.0, -1.0)),
        (b.max.z - p.z, Vec3::new(0.0, 0.0, 1.0)),
    ];
    let (mut depth, mut n) = faces[0];
    for (d, normal) in faces.into_iter().skip(1) {
        if d < depth {
            depth = d;
            n = normal;
        }
    }
    SurfacePoint {
        point: p + n * depth,
        normal: UnitVec3::new(n).unwrap_or(UnitVec3::X),
        position: None,
    }
}

/// The first point of the box's surface on the ray (the slab method), no
/// further than `max_distance`; from inside, where the ray leaves.
fn box_ray(b: Aabb, origin: Point3, direction: UnitVec3, max_distance: f64) -> Option<RayHit> {
    let d = direction.get();
    let axes = [
        (origin.x, d.x, b.min.x, b.max.x, Vec3::new(1.0, 0.0, 0.0)),
        (origin.y, d.y, b.min.y, b.max.y, Vec3::new(0.0, 1.0, 0.0)),
        (origin.z, d.z, b.min.z, b.max.z, Vec3::new(0.0, 0.0, 1.0)),
    ];
    // The entry and exit parameters with the normal of the face crossed.
    let mut enter = (f64::NEG_INFINITY, Vec3::new(0.0, 0.0, 0.0));
    let mut leave = (f64::INFINITY, Vec3::new(0.0, 0.0, 0.0));
    for (o, v, lo, hi, e) in axes {
        if v.abs() < 1e-300 {
            if o < lo || o > hi {
                return None;
            }
            continue;
        }
        let (t0, t1) = ((lo - o) / v, (hi - o) / v);
        // Entering through the low face when moving up, the high one when
        // moving down; leaving through the other.
        let (near, far, n_near, n_far) = if v > 0.0 {
            (t0, t1, e * -1.0, e)
        } else {
            (t1, t0, e, e * -1.0)
        };
        if near > enter.0 {
            enter = (near, n_near);
        }
        if far < leave.0 {
            leave = (far, n_far);
        }
    }
    if enter.0 > leave.0 {
        return None;
    }
    let (t, normal) = if enter.0 >= 0.0 { enter } else { leave };
    if !(0.0..=max_distance.min(f64::MAX)).contains(&t) {
        return None;
    }
    Some(RayHit {
        distance: t,
        point: origin + d * t,
        normal: UnitVec3::new(normal).unwrap_or(UnitVec3::X),
        position: None,
    })
}

/// The nearest piece of the section's boundary so far: the squared distance,
/// the point and the piece's outward normal, all as (s, rho).
#[derive(Clone, Copy)]
struct Nearest {
    d2: f64,
    q: (f64, f64),
    n: (f64, f64),
}

/// The closest point to `p` on the segment `a`-`b` in the plane.
fn closest_2d(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    let (abx, aby) = (b.0 - a.0, b.1 - a.1);
    let l2 = abx * abx + aby * aby;
    if l2 <= f64::MIN_POSITIVE {
        return a;
    }
    let t = (((p.0 - a.0) * abx + (p.1 - a.1) * aby) / l2).clamp(0.0, 1.0);
    (a.0 + abx * t, a.1 + aby * t)
}

/// The tool's line: height along it and distance from it.
struct ToolFrame {
    origin: Point3,
    axis: Vec3,
}

impl ToolFrame {
    /// The height along the axis and the part of the offset across it.
    fn split(&self, p: Point3) -> (f64, Vec3) {
        let v = p - self.origin;
        let z = v.dot(self.axis);
        (z, v - self.axis * z)
    }

    /// The smallest distance from the line to the segment `p`-`q`.
    fn segment_distance(&self, p: Point3, q: Point3) -> f64 {
        let (_, m0) = self.split(p);
        let (_, m1) = self.split(q);
        let m1 = m1 - m0;
        let a = m1.dot(m1);
        let t = if a <= 1e-24 {
            0.0
        } else {
            (-m0.dot(m1) / a).clamp(0.0, 1.0)
        };
        (m0 + m1 * t).length()
    }
}

/// The highest tip height a tool can have on the segment `p`-`q`, with the
/// contact point: the maximum of `z - h(r)` where the segment is within the
/// cutting radius of the tool axis.
fn segment_best(
    tool: &ToolFrame,
    profile: &RevolvedProfile,
    convex: bool,
    p: Point3,
    q: Point3,
) -> Option<(f64, Point3)> {
    let reach = profile.cutting_radius();
    let (z0, m0) = tool.split(p);
    let (z1, m1) = tool.split(q);
    let m1 = m1 - m0;
    let dz = z1 - z0;
    // Where the distance from the axis is within the cutting radius: a
    // quadratic in t, used only to bound the search.
    let a = m1.dot(m1);
    let b = m0.dot(m1);
    let c = m0.dot(m0) - reach * reach;
    let (ta, tb) = if a <= 1e-24 {
        if c > 0.0 {
            return None;
        }
        (0.0, 1.0)
    } else {
        let disc = b * b - a * c;
        if disc < 0.0 {
            return None;
        }
        let s = disc.sqrt();
        (((-b - s) / a).max(0.0), ((-b + s) / a).min(1.0))
    };
    if ta > tb {
        return None;
    }
    let f = |t: f64| -> f64 {
        // Evaluated directly, never from the expanded quadratic (decision
        // record 0012, findings).
        let r = (m0 + m1 * t).length().min(reach);
        profile
            .height_at(r)
            .map_or(f64::NEG_INFINITY, |h| z0 + dz * t - h)
    };
    let t = if convex {
        golden(&f, ta, tb, 1e-13)
    } else {
        sampled(&f, ta, tb, 48, 1e-13)
    };
    let (mut best_t, mut best_f) = (ta, f(ta));
    for candidate in [t, tb] {
        let v = f(candidate);
        if v > best_f {
            best_t = candidate;
            best_f = v;
        }
    }
    best_f.is_finite().then(|| (best_f, p + (q - p) * best_t))
}

/// An arc of angles about the axis, `from` to `to` (radians, may exceed one
/// turn), where the section can come within the tool's reach.
#[derive(Debug, Clone, Copy)]
struct Arc {
    from: f64,
    to: f64,
    full: bool,
}

/// The arcs of `theta` where `distance(theta)` (the section's distance from
/// the tool line) is within `reach`. A full turn when every sample is, else
/// the runs found, with their ends located by bisection, and a window too
/// narrow to hold a sample, found by minimising the distance about each
/// sampled local minimum.
fn reachable_arcs(distance: &impl Fn(f64) -> f64, reach: f64) -> Vec<Arc> {
    const N: usize = 72;
    let step = TAU / N as f64;
    let at = |k: usize| k as f64 * step;
    let ds: Vec<f64> = (0..N).map(|k| distance(at(k))).collect();
    let ok = |k: usize| ds.get(k % N).is_some_and(|d| *d <= reach);
    if (0..N).all(ok) {
        return vec![Arc {
            from: 0.0,
            to: TAU,
            full: true,
        }];
    }
    // The boundary between an unreachable angle `bad` and a reachable `good`.
    let boundary = |mut bad: f64, mut good: f64| {
        for _ in 0..60 {
            let mid = 0.5 * (bad + good);
            if distance(mid) <= reach {
                good = mid;
            } else {
                bad = mid;
            }
        }
        good
    };
    let mut arcs = Vec::new();
    for k in 0..N {
        if ok(k) && !ok(k + N - 1) {
            let mut e = k;
            while ok(e + 1) {
                e += 1;
            }
            arcs.push(Arc {
                from: boundary(at(k) - step, at(k)),
                to: boundary(at(e) + step, at(e)),
                full: false,
            });
        }
    }
    // A window too narrow to hold a sample lies about a local minimum of the
    // distance: a line through the solid has one where it enters and one where
    // it leaves, and a small tool on a large solid sees each as a window
    // narrower than the step. Each sampled local minimum that is not already
    // in an arc is refined.
    let before = |k: usize| ds.get((k + N - 1) % N).copied().unwrap_or(f64::INFINITY);
    let after = |k: usize| ds.get((k + 1) % N).copied().unwrap_or(f64::INFINITY);
    for (k, d) in ds.iter().enumerate() {
        if ok(k) || *d > before(k) || *d > after(k) {
            continue;
        }
        let (lo, hi) = (at(k) - step, at(k) + step);
        let neg = |theta: f64| -distance(theta);
        let theta = golden(&neg, lo, hi, 1e-13);
        if distance(theta) <= reach {
            arcs.push(Arc {
                from: boundary(lo, theta),
                to: boundary(hi, theta),
                full: false,
            });
        }
    }
    arcs
}

/// The best of `best_at` over `arc`: sampled, then refined about the best
/// sample (the best of the segments is quasi-concave in the angle for a
/// convex tool).
fn best_on_arc(
    best_at: &impl Fn(f64) -> Option<(f64, Point3)>,
    arc: Arc,
    samples: u32,
) -> Option<(f64, Point3)> {
    let n = if arc.full { samples * 3 } else { samples };
    let width = arc.to - arc.from;
    let points = if arc.full { n } else { n + 1 };
    let theta = |i: u32| arc.from + width * f64::from(i) / f64::from(n);
    let values: Vec<Option<(f64, Point3)>> = (0..points).map(|i| best_at(theta(i))).collect();
    let mut best: Option<(f64, Point3)> = None;
    let mut best_i = 0;
    for (i, v) in values.iter().enumerate() {
        if let Some(c) = v
            && best.is_none_or(|b| c.0 > b.0)
        {
            best = Some(*c);
            best_i = i as u32;
        }
    }
    best?;
    let lo = theta(best_i) - width / f64::from(n);
    let hi = theta(best_i) + width / f64::from(n);
    let (lo, hi) = if arc.full {
        (lo, hi)
    } else {
        (lo.max(arc.from), hi.min(arc.to))
    };
    let f = |t: f64| best_at(t).map_or(f64::NEG_INFINITY, |c| c.0);
    let refined = golden(&f, lo, hi, 1e-12);
    for t in [refined, lo, hi] {
        if let Some(c) = best_at(t)
            && best.is_none_or(|b| c.0 > b.0)
        {
            best = Some(c);
        }
    }
    best
}

/// Golden-section maximum of a unimodal function on `[a, b]`, to `tol`.
fn golden(f: &impl Fn(f64) -> f64, mut a: f64, mut b: f64, tol: f64) -> f64 {
    let k = (5f64.sqrt() - 1.0) / 2.0;
    let mut c = b - k * (b - a);
    let mut d = a + k * (b - a);
    let (mut fc, mut fd) = (f(c), f(d));
    for _ in 0..100 {
        if (b - a).abs() <= tol * (1.0 + a.abs()) {
            break;
        }
        if fc < fd {
            a = c;
            c = d;
            fc = fd;
            d = a + k * (b - a);
            fd = f(d);
        } else {
            b = d;
            d = c;
            fd = fc;
            c = b - k * (b - a);
            fc = f(c);
        }
    }
    0.5 * (a + b)
}

/// Samples `n` intervals, then refines about the best sample.
fn sampled(f: &impl Fn(f64) -> f64, a: f64, b: f64, n: u32, tol: f64) -> f64 {
    let step = (b - a) / f64::from(n);
    let mut best_i = 0;
    let mut best_f = f64::NEG_INFINITY;
    for i in 0..=n {
        let v = f(a + step * f64::from(i));
        if v > best_f {
            best_f = v;
            best_i = i;
        }
    }
    let lo = a + step * f64::from(best_i.saturating_sub(1));
    let hi = (a + step * f64::from(best_i + 1)).min(b);
    golden(f, lo, hi, tol)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use openchuck_types::tool::{Tool, ToolShape};
    use openchuck_types::units::{Degrees, Millimetres};

    /// A deterministic stream in `[0, 1)`.
    struct Lcg(u64);

    impl Lcg {
        fn next(&mut self) -> f64 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (self.0 >> 11) as f64 / (1_u64 << 53) as f64
        }
    }

    fn v_bit() -> RevolvedProfile {
        Tool {
            shape: ToolShape::VBit {
                included_angle: Degrees(60.0),
                tip_diameter: Millimetres(0.1),
            },
            diameter: Millimetres(3.175),
            flute_length: Millimetres(12.0),
            overall_length: Millimetres(50.0),
            shank_diameter: Millimetres(3.175),
            flute_count: 1,
            material: None,
            centre_cutting: None,
        }
        .profile()
        .unwrap()
    }

    fn random_line(rng: &mut Lcg) -> ToolLine {
        let origin = Point3::new(
            (rng.next() - 0.5) * 40.0,
            (rng.next() - 0.5) * 40.0,
            (rng.next() - 0.5) * 40.0,
        );
        // Mostly across X, as a rotary machine's tool is, sometimes not.
        let x = if rng.next() < 0.5 {
            0.0
        } else {
            rng.next() - 0.5
        };
        let axis = UnitVec3::new(Vec3::new(x, rng.next() - 0.5, rng.next() - 0.5)).unwrap();
        ToolLine { origin, axis }
    }

    fn distance_to_line(p: Point3, line: &ToolLine) -> f64 {
        let w = p - line.origin;
        let u = line.axis.get();
        (w - u * w.dot(u)).length()
    }

    /// The reach test of a box is never more than the box's real distance
    /// from the line, so a box it rules out gives no contact, exactly as
    /// dropping onto its faces would say (issue #382; GC-07).
    #[test]
    fn req_gc_07_a_box_out_of_reach_gives_the_same_answer_as_its_faces() {
        let b = Aabb {
            min: Point3::new(-10.0, -12.0, -8.0),
            max: Point3::new(-4.0, 12.0, 8.0),
        };
        let mesh = box_mesh(b);
        let triangles: Vec<[Point3; 3]> = mesh
            .triangles
            .iter()
            .map(|t| t.map(|i| mesh.vertices[i as usize]))
            .collect();
        let solid = Solid::cuboid(b).unwrap();
        let profile = v_bit();
        let mut rng = Lcg(5);
        let mut ruled_out = 0;
        for _ in 0..3000 {
            let line = random_line(&mut rng);
            let gap = box_line_gap(b, &line);
            for i in 0..=8 {
                for j in 0..=8 {
                    for k in 0..=8 {
                        let f = |lo: f64, hi: f64, n: u32| lo + (hi - lo) * f64::from(n) / 8.0;
                        let p = Point3::new(
                            f(b.min.x, b.max.x, i),
                            f(b.min.y, b.max.y, j),
                            f(b.min.z, b.max.z, k),
                        );
                        assert!(gap <= distance_to_line(p, &line) + 1e-9, "{line:?}");
                    }
                }
            }
            let every = crate::cutter::drop_on_triangles(&triangles, &profile, &line);
            let quick = solid.drop_tool(&profile, &line);
            assert_eq!(
                quick.map(|c| c.offset.to_bits()),
                every.map(|c| c.offset.to_bits()),
                "{line:?}"
            );
            ruled_out += usize::from(gap > profile.cutting_radius() + BOX_REACH_SLACK);
        }
        assert!(ruled_out > 500, "too few lines out of reach: {ruled_out}");
    }

    /// No drop onto a solid lies further along the line than the solid's
    /// highest point along it, and no point of the solid does either.
    #[test]
    fn req_gc_07_no_contact_with_a_solid_is_above_its_highest_point() {
        let solids = [
            Solid::cylinder(Point3::new(-4.0, 0.0, 0.0), UnitVec3::X, 4.0, 5.0).unwrap(),
            Solid::frustum(
                Point3::new(1.0, 2.0, -1.0),
                UnitVec3::new(Vec3::new(0.3, 1.0, 0.2)).unwrap(),
                6.0,
                3.0,
                1.0,
            )
            .unwrap(),
            Solid::cuboid(Aabb {
                min: Point3::new(-10.0, -12.0, -8.0),
                max: Point3::new(-4.0, 12.0, 8.0),
            })
            .unwrap(),
        ];
        let profile = v_bit();
        let mut rng = Lcg(9);
        let mut touched = 0;
        for solid in solids {
            let points = solid.mesh(64).vertices;
            for _ in 0..1500 {
                let mut line = random_line(&mut rng);
                line.origin = Point3::new(
                    0.3 * line.origin.x,
                    0.3 * line.origin.y,
                    0.3 * line.origin.z,
                );
                let highest = solid.highest_along(&line);
                for p in &points {
                    assert!((*p - line.origin).dot(line.axis.get()) <= highest + 1e-9);
                }
                if let Some(c) = solid.drop_tool(&profile, &line) {
                    assert!(c.offset <= highest + 1e-9, "{line:?}");
                    touched += 1;
                }
            }
        }
        assert!(touched > 500, "too few drops touched: {touched}");
    }
}
