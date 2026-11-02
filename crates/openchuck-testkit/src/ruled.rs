// SPDX-License-Identifier: AGPL-3.0-or-later
//! Single-face B-rep models of closed-form surfaces for the swarf tests
//! (OP-07; work breakdown slice 17; decision record 0061): the analytic
//! oracle cases of architecture section 9 (cylinder, cone, plane, sphere
//! and torus), each as a `BrepModel` with one trimmed-to-its-box face and a
//! grid tessellation. Each builder also gives the closed-form point of the
//! surface, so a test can sample the exact surface without the pathing
//! library's evaluators.

use std::f64::consts::TAU;

use openchuck_types::brep::{BrepFace, BrepModel, FaceId, Frame, Surface};
use openchuck_types::geometry::{Point3, Vec3};
use openchuck_types::mesh::TriangleMesh;

/// A frame at `origin` with `x` and `z` as given and `y = z x x`.
fn frame(origin: Point3, x: Vec3, z: Vec3) -> Frame {
    Frame {
        origin,
        x,
        y: z.cross(x),
        z,
    }
}

/// The closed-form point of `surface` at (u, v), as the `openchuck_types`
/// B-rep module documents the parametrisations. Panics on a surface it has
/// no closed form for (a B-spline): an oracle must fail loudly, never judge
/// a toolpath against a wrong point.
#[allow(
    clippy::panic,
    reason = "a test oracle must fail loudly on a surface it has no closed form for"
)]
pub fn point(surface: &Surface, u: f64, v: f64) -> Point3 {
    let radial = |f: &Frame, u: f64| f.x * u.cos() + f.y * u.sin();
    match surface {
        Surface::Plane { frame } => frame.origin + frame.x * u + frame.y * v,
        Surface::Cylinder { frame, radius } => {
            frame.origin + radial(frame, u) * *radius + frame.z * v
        }
        Surface::Cone {
            frame,
            radius,
            semi_angle,
        } => {
            frame.origin
                + radial(frame, u) * (radius + v * semi_angle.sin())
                + frame.z * (v * semi_angle.cos())
        }
        Surface::Sphere { frame, radius } => {
            frame.origin + radial(frame, u) * (radius * v.cos()) + frame.z * (radius * v.sin())
        }
        Surface::Torus {
            frame,
            major,
            minor,
        } => {
            frame.origin
                + radial(frame, u) * (major + minor * v.cos())
                + frame.z * (minor * v.sin())
        }
        other => panic!(
            "ruled::point has no closed form for a {:?} surface",
            other.kind()
        ),
    }
}

/// A grid tessellation of `surface` over `uv`, `nu` by `nv` cells.
fn tessellate(surface: &Surface, uv: [f64; 4], nu: usize, nv: usize) -> TriangleMesh {
    let mut mesh = TriangleMesh::default();
    for i in 0..=nu {
        for j in 0..=nv {
            let u = uv[0] + (uv[1] - uv[0]) * i as f64 / nu as f64;
            let v = uv[2] + (uv[3] - uv[2]) * j as f64 / nv as f64;
            mesh.vertices.push(point(surface, u, v));
        }
    }
    let at = |i: usize, j: usize| u32::try_from(i * (nv + 1) + j).unwrap_or(u32::MAX);
    for i in 0..nu {
        for j in 0..nv {
            mesh.triangles
                .push([at(i, j), at(i + 1, j), at(i + 1, j + 1)]);
            mesh.triangles
                .push([at(i, j), at(i + 1, j + 1), at(i, j + 1)]);
        }
    }
    mesh
}

/// A model of the one face `surface` over the parameter box `uv`, with its
/// outward normal flipped when `reversed`.
pub fn single_face(surface: Surface, uv: [f64; 4], reversed: bool) -> BrepModel {
    let tessellation = tessellate(&surface, uv, 48, 8);
    BrepModel {
        faces: vec![BrepFace {
            id: FaceId(0),
            surface: surface.kind(),
            body: 0,
            edges: Vec::new(),
            geometry: surface,
            reversed,
            uv_bounds: uv,
            trims: Vec::new(),
            tessellation,
        }],
        edges: Vec::new(),
        vertices: Vec::new(),
        tessellation: Default::default(),
    }
}

/// The wall of a round boss: a cylinder about an axis parallel to model Z
/// through (`x`, 0, 0), of `radius`, between heights `z0` and `z1`. Its
/// rulings are parallel to Z and its normal points away from its axis.
pub fn boss_cylinder(x: f64, radius: f64, z0: f64, z1: f64) -> BrepModel {
    single_face(
        Surface::Cylinder {
            frame: frame(
                Point3::new(x, 0.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.0, 0.0, 1.0),
            ),
            radius,
        },
        [0.0, TAU, z0, z1],
        false,
    )
}

/// The wall of a round pocket: the same cylinder as [`boss_cylinder`] with
/// its normal pointing in, towards the axis, so the tool cuts from inside.
pub fn pocket_cylinder(x: f64, radius: f64, z0: f64, z1: f64) -> BrepModel {
    single_face(
        Surface::Cylinder {
            frame: frame(
                Point3::new(x, 0.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.0, 0.0, 1.0),
            ),
            radius,
        },
        [0.0, TAU, z0, z1],
        true,
    )
}

/// The wall of a boss with draft: a cone about an axis parallel to Z through
/// (`x`, 0, 0) that narrows upward by `half_angle` radians, of `radius` at
/// height zero, between heights `v0` and `v1` along its surface line. Its
/// normal points away from the axis and up.
pub fn draft_cone(x: f64, radius: f64, half_angle: f64, v0: f64, v1: f64) -> BrepModel {
    single_face(
        Surface::Cone {
            frame: frame(
                Point3::new(x, 0.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.0, 0.0, 1.0),
            ),
            radius,
            semi_angle: -half_angle,
        },
        [0.0, TAU, v0, v1],
        false,
    )
}

/// A flat wall: the plane `y = y0`, with `u` along Z from `z0` to `z1` and
/// `v` along X from `x0` to `x1`, facing +Y. Ruled in both families; only
/// the Z lines can be cut on four axes.
pub fn wall_plane(y0: f64, x0: f64, x1: f64, z0: f64, z1: f64) -> BrepModel {
    single_face(
        Surface::Plane {
            frame: frame(
                Point3::new(0.0, y0, 0.0),
                Vec3::new(0.0, 0.0, 1.0),
                Vec3::new(0.0, 1.0, 0.0),
            ),
        },
        [z0, z1, x0, x1],
        false,
    )
}

/// A flat wall across X: the plane `x = x0`, with `u` along Z from `z0` to
/// `z1` and `v` along Y from `y0` to `y1`, facing -X.
pub fn cross_plane(x0: f64, y0: f64, y1: f64, z0: f64, z1: f64) -> BrepModel {
    single_face(
        Surface::Plane {
            frame: frame(
                Point3::new(x0, 0.0, 0.0),
                Vec3::new(0.0, 0.0, 1.0),
                Vec3::new(-1.0, 0.0, 0.0),
            ),
        },
        [z0, z1, y0, y1],
        false,
    )
}

/// A floor: the plane `z = z0`, with `u` along X from `x0` to `x1` and `v`
/// along Y from `y0` to `y1`, facing +Z.
pub fn floor_plane(z0: f64, x0: f64, x1: f64, y0: f64, y1: f64) -> BrepModel {
    single_face(
        Surface::Plane {
            frame: frame(
                Point3::new(0.0, 0.0, z0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.0, 0.0, 1.0),
            ),
        },
        [x0, x1, y0, y1],
        false,
    )
}

/// A cylinder coaxial with the rotary axis, of `radius` from `x0` to `x1`:
/// its rulings run along the rotary axis.
pub fn coaxial_cylinder(radius: f64, x0: f64, x1: f64) -> BrepModel {
    single_face(
        Surface::Cylinder {
            frame: frame(
                Point3::ORIGIN,
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
            ),
            radius,
        },
        [0.0, TAU, x0, x1],
        false,
    )
}

/// A band of a sphere of `radius` centred at (`x`, 0, 0), between latitudes
/// `v0` and `v1` (radians): not ruled.
pub fn sphere_band(x: f64, radius: f64, v0: f64, v1: f64) -> BrepModel {
    single_face(
        Surface::Sphere {
            frame: frame(
                Point3::new(x, 0.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
            ),
            radius,
        },
        [0.0, TAU, v0, v1],
        false,
    )
}

/// The outer half of a torus about an axis parallel to Z through (`x`, 0,
/// 0): not ruled.
pub fn torus_band(x: f64, major: f64, minor: f64) -> BrepModel {
    single_face(
        Surface::Torus {
            frame: frame(
                Point3::new(x, 0.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.0, 0.0, 1.0),
            ),
            major,
            minor,
        },
        [0.0, TAU, -1.2, 1.2],
        false,
    )
}
