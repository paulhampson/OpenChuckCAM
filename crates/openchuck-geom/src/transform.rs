// SPDX-License-Identifier: AGPL-3.0-or-later
//! Model placement: transforms and orientation to the rotary axis (IMP-05).
//!
//! A placement is the ordered list of
//! [`TransformStep`](openchuck_types::transform::TransformStep)s stored in the
//! project. [`placement`] turns it into one affine map for a given mesh (the
//! centring step needs the mesh's bounding box at that point), and
//! [`transform_mesh`] applies a map, reversing the winding of every triangle
//! when the map mirrors, so normals stay outward.
//!
//! Rotations by a whole number of quarter turns use exact sines and cosines,
//! so a part turned by 90 degrees has exact coordinates.

use openchuck_types::Result;
use openchuck_types::brep::{
    BSplineCurve, BSplinePatch, BrepEdge, BrepFace, BrepModel, BrepVertex, EdgeCurve, Frame,
    Surface, TrimLoop, UvPoint,
};
use openchuck_types::geometry::{Aabb, Point3, Vec3};
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::transform::{Axis, TransformStep};

/// An affine map `p -> m p + t` in double precision.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Affine3 {
    /// The linear part, by rows.
    pub m: [[f64; 3]; 3],
    /// The translation.
    pub t: [f64; 3],
}

impl Default for Affine3 {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Affine3 {
    /// The identity map.
    pub const IDENTITY: Self = Self {
        m: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        t: [0.0; 3],
    };

    /// A translation by `v`.
    pub fn translation(v: Vec3) -> Self {
        Self {
            t: [v.x, v.y, v.z],
            ..Self::IDENTITY
        }
    }

    /// A right-handed rotation by `degrees` about `axis` through the origin.
    pub fn rotation(axis: Axis, degrees: f64) -> Self {
        let (s, c) = sin_cos_degrees(degrees);
        let m = match axis {
            Axis::X => [[1.0, 0.0, 0.0], [0.0, c, -s], [0.0, s, c]],
            Axis::Y => [[c, 0.0, s], [0.0, 1.0, 0.0], [-s, 0.0, c]],
            Axis::Z => [[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]],
        };
        Self { m, t: [0.0; 3] }
    }

    /// A uniform scale by `factor` about the origin.
    pub fn scale(factor: f64) -> Self {
        Self {
            m: [[factor, 0.0, 0.0], [0.0, factor, 0.0], [0.0, 0.0, factor]],
            t: [0.0; 3],
        }
    }

    /// A reflection that negates the `axis` coordinate.
    pub fn mirror(axis: Axis) -> Self {
        let mut map = Self::IDENTITY;
        match axis {
            Axis::X => map.m[0][0] = -1.0,
            Axis::Y => map.m[1][1] = -1.0,
            Axis::Z => map.m[2][2] = -1.0,
        }
        map
    }

    /// This map followed by `next`.
    pub fn then(&self, next: &Self) -> Self {
        let a = &next.m;
        let b = &self.m;
        let mut m = [[0.0; 3]; 3];
        for (i, row) in m.iter_mut().enumerate() {
            for (j, value) in row.iter_mut().enumerate() {
                *value = dot3(row_of(a, i), column_of(b, j));
            }
        }
        let t = next.apply_vec(Vec3::new(self.t[0], self.t[1], self.t[2]));
        Self {
            m,
            t: [t.x + next.t[0], t.y + next.t[1], t.z + next.t[2]],
        }
    }

    /// The image of point `p`.
    pub fn apply(&self, p: Point3) -> Point3 {
        let v = self.apply_vec(p.to_vec());
        Point3::new(v.x + self.t[0], v.y + self.t[1], v.z + self.t[2])
    }

    /// The image of direction `v` (the linear part only).
    pub fn apply_vec(&self, v: Vec3) -> Vec3 {
        let p = [v.x, v.y, v.z];
        Vec3::new(dot3(self.m[0], p), dot3(self.m[1], p), dot3(self.m[2], p))
    }

    /// The determinant of the linear part: negative for a map that mirrors.
    pub fn determinant(&self) -> f64 {
        let [r0, r1, r2] = self.m;
        let cross = Vec3::new(r1[0], r1[1], r1[2]).cross(Vec3::new(r2[0], r2[1], r2[2]));
        r0[0] * cross.x + r0[1] * cross.y + r0[2] * cross.z
    }
}

fn dot3(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn row_of(m: &[[f64; 3]; 3], i: usize) -> [f64; 3] {
    m.get(i).copied().unwrap_or([0.0; 3])
}

fn column_of(m: &[[f64; 3]; 3], j: usize) -> [f64; 3] {
    let at = |i: usize| m.get(i).and_then(|row| row.get(j)).copied().unwrap_or(0.0);
    [at(0), at(1), at(2)]
}

/// Sine and cosine of an angle in degrees, exact for whole quarter turns.
fn sin_cos_degrees(degrees: f64) -> (f64, f64) {
    let turns = degrees / 90.0;
    if turns.fract() == 0.0 && turns.abs() < 1e15 {
        // Whole quarter turns: exact values, and never a negative zero.
        return match turns.rem_euclid(4.0) as u8 {
            0 => (0.0, 1.0),
            1 => (1.0, 0.0),
            2 => (0.0, -1.0),
            _ => (-1.0, 0.0),
        };
    }
    degrees.to_radians().sin_cos()
}

/// The axis-aligned bounding box of the mesh's vertices under `map`, or
/// `None` for a mesh with no triangles. Only vertices that a triangle uses
/// count.
pub fn bounds(mesh: &TriangleMesh, map: &Affine3) -> Option<Aabb> {
    let mut used = vec![false; mesh.vertices.len()];
    for &index in mesh.triangles.iter().flatten() {
        if let Some(u) = used.get_mut(index as usize) {
            *u = true;
        }
    }
    let mut points = mesh
        .vertices
        .iter()
        .zip(&used)
        .filter(|(_, used)| **used)
        .map(|(p, _)| map.apply(*p));
    let first = points.next()?;
    Some(points.fold(
        Aabb {
            min: first,
            max: first,
        },
        |b, p| Aabb {
            min: Point3::new(b.min.x.min(p.x), b.min.y.min(p.y), b.min.z.min(p.z)),
            max: Point3::new(b.max.x.max(p.x), b.max.y.max(p.y), b.max.z.max(p.z)),
        },
    ))
}

/// The map of a placement for `mesh`: `steps` applied first to last. Fails if
/// a step holds an invalid value.
pub fn placement(mesh: &TriangleMesh, steps: &[TransformStep]) -> Result<Affine3> {
    TransformStep::validate_all(steps, "transform")?;
    let mut map = Affine3::IDENTITY;
    for step in steps {
        let next = match *step {
            TransformStep::Translate { x, y, z } => Affine3::translation(Vec3::new(x, y, z)),
            TransformStep::Rotate { axis, angle } => Affine3::rotation(axis, angle.0),
            TransformStep::Scale { factor } => Affine3::scale(factor),
            TransformStep::Mirror { axis } => Affine3::mirror(axis),
            TransformStep::CentreOnRotaryAxis => match bounds(mesh, &map) {
                Some(b) => Affine3::translation(Vec3::new(
                    0.0,
                    -(b.min.y + b.max.y) / 2.0,
                    -(b.min.z + b.max.z) / 2.0,
                )),
                None => Affine3::IDENTITY,
            },
        };
        map = map.then(&next);
    }
    Ok(map)
}

/// The mesh mapped by `map`. When the map mirrors, each triangle's winding is
/// reversed (its second and third corners swap), so a closed mesh keeps its
/// normals outward. Triangle and vertex indices keep their meaning.
pub fn transform_mesh(mesh: &TriangleMesh, map: &Affine3) -> TriangleMesh {
    let mirrored = map.determinant() < 0.0;
    TriangleMesh {
        vertices: mesh.vertices.iter().map(|&p| map.apply(p)).collect(),
        triangles: mesh
            .triangles
            .iter()
            .map(|&[a, b, c]| if mirrored { [a, c, b] } else { [a, b, c] })
            .collect(),
    }
}

/// Per-triangle normals (such as those an STL file states) mapped by `map`,
/// to go with [`transform_mesh`]. Placement maps are rotations, mirrors,
/// uniform scales and moves, which keep angles, so the linear part keeps a
/// normal perpendicular to its triangle and on the same side.
pub fn transform_normals(normals: &[Vec3], map: &Affine3) -> Vec<Vec3> {
    normals.iter().map(|&n| map.apply_vec(n)).collect()
}

/// The B-rep model mapped by `map` (IMP-05 for STEP models), with every face,
/// edge and vertex keeping its ID.
///
/// Placement maps are rotations, mirrors, uniform scales and moves, so the
/// linear part is a scale `s` times an orthogonal map. Each analytic frame's
/// origin is mapped and its axes are mapped and divided by `s`, so they stay
/// orthonormal; radii are multiplied by `s`, and the parameters that are
/// lengths (u and v of a plane, v of a cylinder and a cone) are multiplied by
/// `s` in the UV bounds and the trim loops, so a (u, v) names the same place
/// on the part as before. B-spline poles are mapped and their parameters kept.
/// When the map mirrors, a frame becomes left-handed, which turns the
/// surface's `du x dv` inward, so the face's `reversed` flag is flipped to keep
/// its normal outward, and each tessellation triangle's winding is reversed
/// as in [`transform_mesh`]. Fails when a face's surface is a kind this
/// function does not know and the map moves the model, since that surface
/// could not be moved with its face.
pub fn transform_brep(model: &BrepModel, map: &Affine3) -> Result<BrepModel> {
    let det = map.determinant();
    let mirrored = det < 0.0;
    let s = det.abs().cbrt();
    let axis = |v: Vec3| map.apply_vec(v) * (1.0 / s);
    let frame = |f: &Frame| Frame {
        origin: map.apply(f.origin),
        x: axis(f.x),
        y: axis(f.y),
        z: axis(f.z),
    };
    let faces = model
        .faces
        .iter()
        .map(|face| -> Result<BrepFace> {
            // Which parameters are lengths, and so scale with the model.
            let (geometry, scale_u, scale_v) = match &face.geometry {
                Surface::Plane { frame: f } => (Surface::Plane { frame: frame(f) }, s, s),
                Surface::Cylinder { frame: f, radius } => (
                    Surface::Cylinder {
                        frame: frame(f),
                        radius: radius * s,
                    },
                    1.0,
                    s,
                ),
                Surface::Cone {
                    frame: f,
                    radius,
                    semi_angle,
                } => (
                    Surface::Cone {
                        frame: frame(f),
                        radius: radius * s,
                        semi_angle: *semi_angle,
                    },
                    1.0,
                    s,
                ),
                Surface::Sphere { frame: f, radius } => (
                    Surface::Sphere {
                        frame: frame(f),
                        radius: radius * s,
                    },
                    1.0,
                    1.0,
                ),
                Surface::Torus {
                    frame: f,
                    major,
                    minor,
                } => (
                    Surface::Torus {
                        frame: frame(f),
                        major: major * s,
                        minor: minor * s,
                    },
                    1.0,
                    1.0,
                ),
                Surface::BSpline(patch) => (
                    Surface::BSpline(BSplinePatch {
                        poles: patch.poles.iter().map(|&p| map.apply(p)).collect(),
                        ..patch.clone()
                    }),
                    1.0,
                    1.0,
                ),
                // A surface kind added later can only be kept as it is, which
                // is right only for a map that does not move the model.
                other if *map == Affine3::IDENTITY => (other.clone(), 1.0, 1.0),
                other => {
                    return Err(openchuck_types::Error::invalid(
                        format!("face {}", face.id.0),
                        format!(
                            "its surface ({:?}) cannot be placed: this build does not know how \
                             to move it",
                            other.kind()
                        ),
                    ));
                }
            };
            let [u0, u1, v0, v1] = face.uv_bounds;
            Ok(BrepFace {
                geometry,
                reversed: face.reversed != mirrored,
                uv_bounds: [u0 * scale_u, u1 * scale_u, v0 * scale_v, v1 * scale_v],
                trims: face
                    .trims
                    .iter()
                    .map(|l| {
                        TrimLoop(
                            l.0.iter()
                                .map(|p| UvPoint {
                                    u: p.u * scale_u,
                                    v: p.v * scale_v,
                                })
                                .collect(),
                        )
                    })
                    .collect(),
                tessellation: transform_mesh(&face.tessellation, map),
                ..face.clone()
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(BrepModel {
        faces,
        edges: model
            .edges
            .iter()
            .map(|e| BrepEdge {
                polyline: e.polyline.iter().map(|&p| map.apply(p)).collect(),
                curve: e.curve.as_ref().and_then(|c| transform_curve(c, map, s)),
                ..e.clone()
            })
            .collect(),
        vertices: model
            .vertices
            .iter()
            .map(|v| BrepVertex {
                point: map.apply(v.point),
                ..v.clone()
            })
            .collect(),
        tessellation: model.tessellation,
    })
}

/// An edge's exact curve mapped by `map`, whose linear part is a scale `s`
/// times an orthogonal map (decision 0227): frames and poles move as for a
/// face, lengths scale by `s`, angles and spline parameters are kept, and a
/// line's parameter (a length) scales by `s`. `None` for a curve kind this
/// function does not know, so the edge keeps only its polyline.
fn transform_curve(curve: &EdgeCurve, map: &Affine3, s: f64) -> Option<EdgeCurve> {
    let axis = |v: Vec3| map.apply_vec(v) * (1.0 / s);
    let frame = |f: &Frame| Frame {
        origin: map.apply(f.origin),
        x: axis(f.x),
        y: axis(f.y),
        z: axis(f.z),
    };
    Some(match curve {
        EdgeCurve::Line {
            origin,
            direction,
            first,
            last,
        } => EdgeCurve::Line {
            origin: map.apply(*origin),
            direction: axis(*direction),
            first: first * s,
            last: last * s,
        },
        EdgeCurve::Circle {
            frame: f,
            radius,
            first,
            last,
        } => EdgeCurve::Circle {
            frame: frame(f),
            radius: radius * s,
            first: *first,
            last: *last,
        },
        EdgeCurve::Ellipse {
            frame: f,
            major,
            minor,
            first,
            last,
        } => EdgeCurve::Ellipse {
            frame: frame(f),
            major: major * s,
            minor: minor * s,
            first: *first,
            last: *last,
        },
        EdgeCurve::BSpline(c) => EdgeCurve::BSpline(BSplineCurve {
            poles: c.poles.iter().map(|&p| map.apply(p)).collect(),
            ..c.clone()
        }),
        _ => return None,
    })
}

/// The map of a placement for a B-rep model: as [`placement`] on its joined
/// tessellation, which [`TransformStep::CentreOnRotaryAxis`] centres.
pub fn brep_placement(model: &BrepModel, steps: &[TransformStep]) -> Result<Affine3> {
    placement(&model.joined_tessellation().0, steps)
}

/// The mesh placed by `steps` (IMP-05).
pub fn place(mesh: &TriangleMesh, steps: &[TransformStep]) -> Result<TriangleMesh> {
    Ok(transform_mesh(mesh, &placement(mesh, steps)?))
}

/// A direction along a model axis, such as `+z`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AxisDirection {
    /// The axis.
    pub axis: Axis,
    /// True for the negative direction.
    pub negative: bool,
}

impl AxisDirection {
    /// Reads `x`, `+x`, `-y` and so on, ignoring case.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        let (negative, letter) = match text.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, text.strip_prefix('+').unwrap_or(text)),
        };
        Some(Self {
            axis: Axis::parse(letter)?,
            negative,
        })
    }
}

impl std::fmt::Display for AxisDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}", if self.negative { '-' } else { '+' }, self.axis)
    }
}

/// The steps that orient a part to the rotary axis (IMP-05): a rotation
/// that turns the part's `along` direction onto +X, the rotary axis, then
/// [`TransformStep::CentreOnRotaryAxis`].
pub fn orient_to_rotary_axis(along: AxisDirection) -> Vec<TransformStep> {
    use openchuck_types::units::Degrees;
    let rotate = |axis, angle| TransformStep::Rotate {
        axis,
        angle: Degrees(angle),
    };
    let rotation = match (along.axis, along.negative) {
        (Axis::X, false) => None,
        (Axis::X, true) => Some(rotate(Axis::Z, 180.0)),
        // A quarter turn about Z takes +Y to -X, so -90 takes +Y to +X.
        (Axis::Y, false) => Some(rotate(Axis::Z, -90.0)),
        (Axis::Y, true) => Some(rotate(Axis::Z, 90.0)),
        // A quarter turn about Y takes +Z to +X.
        (Axis::Z, false) => Some(rotate(Axis::Y, 90.0)),
        (Axis::Z, true) => Some(rotate(Axis::Y, -90.0)),
    };
    rotation
        .into_iter()
        .chain(std::iter::once(TransformStep::CentreOnRotaryAxis))
        .collect()
}
