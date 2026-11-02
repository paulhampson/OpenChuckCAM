// SPDX-License-Identifier: AGPL-3.0-or-later
//! Holes found in the faces of a B-rep model (OP-10; work breakdown slice 18
//! item 1; decision record 0051 items 1 and 2).
//!
//! A hole is a cylinder whose wall faces the axis (the material is outside
//! it), whole round, with the faces of its two ends known from the model's
//! edges. A cylinder the importer splits in two halves is one hole. An end
//! that a face covers (a flat or conical bottom) is closed; an end that no
//! face covers is open, either to the outside, to a larger bore or to a
//! further hole. A hole with one closed end is blind and is entered from the
//! open end. A hole open at both ends passes through, and is entered from
//! the end farther from the rotary axis, so a hole through a rod is drilled
//! from the near side and each wall of a tube from outside. A cylinder
//! closed at both ends is a cavity, not a hole.
//!
//! Everything is read from the model's plain data: the cylinder frame and
//! radius, the face's parameter box, its tessellation (for the facing of the
//! wall and for what covers the axis at an end) and the edges' faces and
//! polylines. Nothing here is a meshed guess for a mesh model: meshes have
//! no holes in this sense (slice 18, "Hole detection on meshes is not
//! planned").

use openchuck_types::brep::{BrepFace, BrepModel, FaceId, Surface};
use openchuck_types::geometry::{Point3, UnitVec3, Vec3};

/// How a hole ends.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum HoleEnd {
    /// A closed end: a flat, conical or other bottom.
    Blind {
        /// The depth of the deepest point of the bottom below the entry
        /// (the cone's apex, for a drilled point), at least the hole's
        /// length.
        tip_depth: f64,
    },
    /// Open at both ends.
    Through,
}

/// A round hole in a B-rep model, as a drill sees it.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Hole {
    /// The cylinder faces that make the wall, in model order. Selecting any
    /// of them selects the hole.
    pub faces: Vec<FaceId>,
    /// The centre of the opening the hole is entered from, on its axis.
    pub entry: Point3,
    /// The unit direction from the hole out of the material at the entry:
    /// the way the tool axis points (from tip to spindle) while drilling.
    pub direction: UnitVec3,
    /// The diameter, in millimetres.
    pub diameter: f64,
    /// The depth of the full diameter below the entry, in millimetres.
    pub length: f64,
    /// How the hole ends.
    pub end: HoleEnd,
}

/// Tolerance on lengths, in millimetres, when matching faces of one hole.
const TOL: f64 = 1e-6;

/// A hole-like cylinder face.
struct Wall<'a> {
    face: &'a BrepFace,
    origin: Point3,
    x: Vec3,
    y: Vec3,
    z: Vec3,
    radius: f64,
    /// The range of the face's axis coordinate, ascending.
    t0: f64,
    t1: f64,
}

/// The holes of `model`, ordered by their lowest face ID.
pub fn find_holes(model: &BrepModel) -> Vec<Hole> {
    let walls: Vec<Wall<'_>> = model.faces.iter().filter_map(wall).collect();
    let mut taken = vec![false; walls.len()];
    let mut holes = Vec::new();
    for (i, first) in walls.iter().enumerate() {
        if taken.get(i).copied().unwrap_or(true) {
            continue;
        }
        let mut group = vec![first];
        for (j, other) in walls.iter().enumerate().skip(i + 1) {
            if !taken.get(j).copied().unwrap_or(true) && same_hole(first, other) {
                group.push(other);
                if let Some(t) = taken.get_mut(j) {
                    *t = true;
                }
            }
        }
        if let Some(t) = taken.get_mut(i) {
            *t = true;
        }
        if covered_turn(&group) < std::f64::consts::TAU - 1e-6 {
            continue;
        }
        if let Some(hole) = hole_of(model, &group) {
            holes.push(hole);
        }
    }
    holes.sort_by_key(|h| h.faces.first().copied());
    holes
}

/// How much of a turn around the first wall's axis the group's faces cover,
/// counting overlaps once: each face's `u` interval is placed in the first
/// face's frame, merged, and summed.
fn covered_turn(group: &[&Wall<'_>]) -> f64 {
    use std::f64::consts::TAU;
    let Some(first) = group.first() else {
        return 0.0;
    };
    let hand = |w: &Wall<'_>| {
        if w.x.cross(w.y).dot(w.z) < 0.0 {
            -1.0
        } else {
            1.0
        }
    };
    let mut spans: Vec<(f64, f64)> = Vec::new();
    for w in group {
        let [u0, u1, _, _] = w.face.uv_bounds;
        let sign = w.z.dot(first.z).signum() * hand(w) * hand(first);
        let start = w.x.dot(first.y).atan2(w.x.dot(first.x));
        let (a, b) = if sign > 0.0 {
            (start + u0, start + u1)
        } else {
            (start - u1, start - u0)
        };
        let width = (b - a).min(TAU);
        let a = a.rem_euclid(TAU);
        if a + width > TAU {
            spans.push((a, TAU));
            spans.push((0.0, a + width - TAU));
        } else {
            spans.push((a, a + width));
        }
    }
    spans.sort_by(|p, q| p.0.total_cmp(&q.0));
    let mut total = 0.0;
    let mut end = f64::NEG_INFINITY;
    for (a, b) in spans {
        let a = a.max(end);
        if b > a {
            total += b - a;
        }
        end = end.max(b);
    }
    total
}

/// The cylinder `face` as the wall of a hole, if it is one.
fn wall(face: &BrepFace) -> Option<Wall<'_>> {
    let Surface::Cylinder { frame, radius } = &face.geometry else {
        return None;
    };
    if !(radius.is_finite() && *radius > 0.0) {
        return None;
    }
    // Triangles are wound counter-clockwise seen from outside the material,
    // so on the wall of a hole they face the axis.
    let mut facing = 0.0;
    let mesh = &face.tessellation;
    for t in &mesh.triangles {
        let [a, b, c] = t.map(|i| mesh.vertices.get(i as usize).copied());
        let (a, b, c) = (a?, b?, c?);
        let normal = (b - a).cross(c - a);
        let centre = Point3::new(
            (a.x + b.x + c.x) / 3.0,
            (a.y + b.y + c.y) / 3.0,
            (a.z + b.z + c.z) / 3.0,
        );
        let from_origin = centre - frame.origin;
        let radial = from_origin + frame.z * -from_origin.dot(frame.z);
        let length = radial.length();
        if length > TOL {
            facing += normal.dot(radial) / length;
        }
    }
    if facing >= 0.0 {
        return None;
    }
    let [_, _, v0, v1] = face.uv_bounds;
    Some(Wall {
        face,
        origin: frame.origin,
        x: frame.x,
        y: frame.y,
        z: frame.z,
        radius: *radius,
        t0: v0,
        t1: v1,
    })
}

/// True when `b` is the same bore as `a` (the other half of a split
/// cylinder): the same body, radius and axis line, over the same length.
fn same_hole(a: &Wall<'_>, b: &Wall<'_>) -> bool {
    if a.face.body != b.face.body || (a.radius - b.radius).abs() > TOL {
        return false;
    }
    let along = a.z.dot(b.z);
    if (along.abs() - 1.0).abs() > 1e-9 {
        return false;
    }
    // Where b's origin is on a's axis, and how far it is off the line.
    let d = b.origin - a.origin;
    let shift = d.dot(a.z);
    let off = d + a.z * -shift;
    if off.length() > TOL {
        return false;
    }
    let (b0, b1) = if along > 0.0 {
        (shift + b.t0, shift + b.t1)
    } else {
        (shift - b.t1, shift - b.t0)
    };
    (a.t0 - b0).abs() <= TOL && (a.t1 - b1).abs() <= TOL
}

fn hole_of(model: &BrepModel, group: &[&Wall<'_>]) -> Option<Hole> {
    let first = group.first()?;
    let (origin, z, radius) = (first.origin, first.z, first.radius);
    let (t0, t1) = (first.t0, first.t1);
    let ids: Vec<FaceId> = group.iter().map(|w| w.face.id).collect();
    // The faces that meet the wall at each end, by where their shared edge
    // lies along the axis.
    let mut at: [Vec<FaceId>; 2] = [Vec::new(), Vec::new()];
    let mid = 0.5 * (t0 + t1);
    for wall in group {
        for edge_id in &wall.face.edges {
            let Some(edge) = model.edges.iter().find(|e| e.id == *edge_id) else {
                continue;
            };
            if edge.polyline.is_empty() {
                continue;
            }
            // The coordinate along this group's axis, which may run the
            // other way from this face's own frame.
            let t = edge
                .polyline
                .iter()
                .map(|p| (*p - origin).dot(z))
                .sum::<f64>()
                / edge.polyline.len() as f64;
            let end = usize::from(t > mid);
            for other in &edge.faces {
                if !ids.contains(other)
                    && let Some(list) = at.get_mut(end)
                    && !list.contains(other)
                {
                    list.push(*other);
                }
            }
        }
    }
    let (basis_x, basis_y) = perpendicular(z);
    let covered = |end: usize| -> Option<f64> {
        // How far beyond this end the nearest face closes it: the first place
        // the axis pierces a neighbouring face past the end. `None` when no
        // neighbour is pierced there, so the end is open.
        let t_end = if end == 0 { t0 } else { t1 };
        let mut nearest: Option<f64> = None;
        for id in at.get(end)? {
            let Some(face) = model.faces.iter().find(|f| f.id == *id) else {
                continue;
            };
            for t in axis_piercings(face, origin, z, basis_x, basis_y) {
                let beyond = if end == 0 { t_end - t } else { t - t_end };
                if beyond >= -TOL {
                    nearest = Some(nearest.map_or(beyond, |n: f64| n.min(beyond)));
                }
            }
        }
        nearest
    };
    let (c0, c1) = (covered(0), covered(1));
    let length = t1 - t0;
    let at_end = |t: f64| origin + z * t;
    let (entry_end, end) = match (c0, c1) {
        (Some(_), Some(_)) => return None,
        (Some(deeper), None) => (
            1,
            HoleEnd::Blind {
                tip_depth: length + deeper.max(0.0),
            },
        ),
        (None, Some(deeper)) => (
            0,
            HoleEnd::Blind {
                tip_depth: length + deeper.max(0.0),
            },
        ),
        (None, None) => (farther_end(at_end(t0), at_end(t1), z), HoleEnd::Through),
    };
    let direction = if entry_end == 1 { z } else { -z };
    let entry = at_end(if entry_end == 1 { t1 } else { t0 });
    Some(Hole {
        faces: ids,
        entry,
        direction: UnitVec3::new(direction).ok()?,
        diameter: 2.0 * radius,
        length,
        end,
    })
}

/// Which end (0 at `a`, 1 at `b`) of a through hole to enter from: the one
/// farther from the rotary axis (the model X axis), and for a hole through
/// the axis, the one facing up, then towards +Y, so the choice never depends
/// on the order of the file.
fn farther_end(a: Point3, b: Point3, z: Vec3) -> usize {
    let (ra, rb) = (a.y.hypot(a.z), b.y.hypot(b.z));
    if (ra - rb).abs() > TOL {
        return usize::from(rb > ra);
    }
    // Same distance: enter from the end the axis direction points away from
    // when that direction has the larger Z (then Y).
    let up = if z.z.abs() > TOL { z.z } else { z.y };
    usize::from(up > 0.0)
}

/// Two unit vectors perpendicular to `z` and to each other.
fn perpendicular(z: Vec3) -> (Vec3, Vec3) {
    let helper = if z.x.abs() < 0.9 {
        Vec3::new(1.0, 0.0, 0.0)
    } else {
        Vec3::new(0.0, 1.0, 0.0)
    };
    let x = helper.cross(z);
    let x = x * (1.0 / x.length().max(f64::MIN_POSITIVE));
    let y = z.cross(x);
    (x, y)
}

/// The coordinates along the axis (through `origin` along `z`) at which the
/// axis pierces the face's triangles. The triangles are tested seen along the
/// axis, in the plane spanned by `bx` and `by`.
fn axis_piercings(face: &BrepFace, origin: Point3, z: Vec3, bx: Vec3, by: Vec3) -> Vec<f64> {
    let mesh = &face.tessellation;
    let flat = |p: Point3| {
        let d = p - origin;
        (d.dot(bx), d.dot(by))
    };
    let mut out = Vec::new();
    for t in &mesh.triangles {
        let [a, b, c] = t.map(|i| mesh.vertices.get(i as usize).copied());
        let (Some(a), Some(b), Some(c)) = (a, b, c) else {
            continue;
        };
        let (fa, fb, fc) = (flat(a), flat(b), flat(c));
        let side = |p: (f64, f64), q: (f64, f64)| p.0 * q.1 - p.1 * q.0;
        let d1 = side((fb.0 - fa.0, fb.1 - fa.1), (-fa.0, -fa.1));
        let d2 = side((fc.0 - fb.0, fc.1 - fb.1), (-fb.0, -fb.1));
        let d3 = side((fa.0 - fc.0, fa.1 - fc.1), (-fc.0, -fc.1));
        let slack = 1e-9;
        let inside = (d1 >= -slack && d2 >= -slack && d3 >= -slack)
            || (d1 <= slack && d2 <= slack && d3 <= slack);
        if !inside {
            continue;
        }
        let normal = (b - a).cross(c - a);
        let along = normal.dot(z);
        if along.abs() < 1e-12 * normal.length().max(f64::MIN_POSITIVE) {
            continue;
        }
        out.push((a - origin).dot(normal) / along);
    }
    out
}
