// SPDX-License-Identifier: AGPL-3.0-or-later
//! A B-rep rotary blank with drilled holes, built from closed forms (OP-10;
//! work breakdown slice 18).
//!
//! The blank is a cylinder about the model X axis. Each hole is a round
//! cylinder face whose wall faces the axis, with its edges, and a flat or
//! conical bottom face when it is blind. The blank's side face is tessellated
//! with the holes cut out, as a kernel does, so a face that covers a hole's
//! axis really is the bottom. Nothing here uses the importer, so the pathing
//! tests need no STEP kernel.

use std::f64::consts::TAU;

use openchuck_types::brep::{
    BrepEdge, BrepFace, BrepModel, EdgeId, FaceId, Frame, Surface, TrimLoop, UvPoint,
};
use openchuck_types::geometry::{Point3, Vec3};
use openchuck_types::mesh::TriangleMesh;

/// A hole to drill in the blank.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HoleSpec {
    /// Where the hole's axis crosses the rotary axis' plane of symmetry:
    /// the X of the axis line.
    pub x: f64,
    /// The direction out of the hole (any length; it is normalised).
    pub direction: [f64; 3],
    /// How far the axis line lies to one side of the rotary axis.
    pub offset: f64,
    /// The diameter.
    pub diameter: f64,
    /// The depth of the full diameter below the blank's surface on the axis,
    /// or `None` to go through to the far side.
    pub depth: Option<f64>,
    /// Whether a blind hole ends in a 118 degree point rather than flat.
    pub point: bool,
    /// Whether the wall is two half-cylinder faces, as a kernel may give.
    pub split: bool,
}

impl HoleSpec {
    /// A flat-bottomed blind hole straight in from direction `direction`.
    pub fn blind(x: f64, direction: [f64; 3], diameter: f64, depth: f64) -> Self {
        Self {
            x,
            direction,
            offset: 0.0,
            diameter,
            depth: Some(depth),
            point: false,
            split: false,
        }
    }

    /// A hole through the blank.
    pub fn through(x: f64, direction: [f64; 3], diameter: f64) -> Self {
        Self {
            depth: None,
            ..Self::blind(x, direction, diameter, 0.0)
        }
    }
}

/// What was built for one hole, for the tests to compare against.
#[derive(Debug, Clone, PartialEq)]
pub struct Built {
    /// The wall faces.
    pub walls: Vec<FaceId>,
    /// Where the axis meets the blank's surface.
    pub entry: Point3,
    /// The unit direction out of the hole.
    pub direction: Vec3,
    /// The depth of the full diameter below the entry.
    pub length: f64,
    /// The depth of the deepest point below the entry (equal to `length` for
    /// a through hole).
    pub tip_depth: f64,
}

fn unit(v: Vec3) -> Vec3 {
    v * (1.0 / v.length())
}

fn at(o: Point3, v: Vec3) -> Point3 {
    o + v
}

/// A frame at `origin` with `z` on `z` and `x` perpendicular to it.
fn frame_on(origin: Point3, z: Vec3) -> Frame {
    let helper = if z.x.abs() < 0.9 {
        Vec3::new(1.0, 0.0, 0.0)
    } else {
        Vec3::new(0.0, 1.0, 0.0)
    };
    let x = unit(helper.cross(z));
    let y = z.cross(x);
    Frame { origin, x, y, z }
}

fn ring(f: &Frame, r: f64, n: usize, along: f64) -> Vec<Point3> {
    (0..n)
        .map(|i| {
            let a = TAU * i as f64 / n as f64;
            at(
                f.origin,
                f.x * (r * a.cos()) + f.y * (r * a.sin()) + f.z * along,
            )
        })
        .collect()
}

/// A blank of `radius` and `length` along X with `specs` drilled. The faces
/// are the blank's side (ID 0) and then each hole's faces, in order. Returns
/// the model and what was built.
pub fn drilled_blank(radius: f64, length: f64, specs: &[HoleSpec]) -> (BrepModel, Vec<Built>) {
    const SEGMENTS: usize = 64;
    let mut faces: Vec<BrepFace> = Vec::new();
    let mut edges: Vec<BrepEdge> = Vec::new();
    let mut built = Vec::new();
    // The side face is added last (it needs the holes), but takes ID 0.
    faces.push(placeholder());
    let mut axes: Vec<(Point3, Vec3, f64)> = Vec::new();

    for spec in specs {
        let d = unit(Vec3::new(
            spec.direction[0],
            spec.direction[1],
            spec.direction[2],
        ));
        let rh = 0.5 * spec.diameter;
        let in_plane = Vec3::new(0.0, d.y, d.z);
        let perp = unit(Vec3::new(0.0, -d.z, d.y));
        let origin = Point3::new(spec.x, 0.0, 0.0) + perp * spec.offset;
        // Where the axis meets the blank's surface (outward), and the far side.
        let (a, b, c) = (
            in_plane.dot(in_plane),
            2.0 * (origin.y * d.y + origin.z * d.z),
            origin.y * origin.y + origin.z * origin.z - radius * radius,
        );
        let root = (b * b - 4.0 * a * c).max(0.0).sqrt();
        let t_out = (-b + root) / (2.0 * a);
        let t_far = (-b - root) / (2.0 * a);
        let entry = origin + d * t_out;
        let (length_full, tip, bottom_t) = match spec.depth {
            Some(depth) => {
                let cone = if spec.point {
                    rh / (59.0_f64).to_radians().tan()
                } else {
                    0.0
                };
                (depth, depth + cone, t_out - depth)
            }
            None => (t_out - t_far, t_out - t_far, t_far),
        };
        let wall_frame = frame_on(origin + d * bottom_t, d);
        let top_ring = ring(&wall_frame, rh, SEGMENTS, length_full);
        let bottom_ring = ring(&wall_frame, rh, SEGMENTS, 0.0);
        let id0 = faces.len() as u32;
        let halves = if spec.split { 2 } else { 1 };
        let mut walls = Vec::new();
        for h in 0..halves {
            let id = FaceId(id0 + h as u32);
            walls.push(id);
            let (u0, u1) = (
                TAU * h as f64 / halves as f64,
                TAU * (h + 1) as f64 / halves as f64,
            );
            let per = SEGMENTS / halves;
            let mut tess = TriangleMesh::default();
            for i in 0..=per {
                let idx = (h * per + i) % SEGMENTS;
                tess.vertices
                    .extend(bottom_ring.get(idx).into_iter().chain(top_ring.get(idx)));
            }
            for i in 0..per as u32 {
                let (b0, t0, b1, t1) = (2 * i, 2 * i + 1, 2 * i + 2, 2 * i + 3);
                // Wound so the normals face the axis.
                tess.triangles.push([b0, t1, b1]);
                tess.triangles.push([b0, t0, t1]);
            }
            faces.push(BrepFace {
                id,
                surface: openchuck_types::brep::SurfaceKind::Cylinder,
                body: 0,
                edges: Vec::new(),
                geometry: Surface::Cylinder {
                    frame: wall_frame,
                    radius: rh,
                },
                reversed: true,
                uv_bounds: [u0, u1, 0.0, length_full],
                trims: Vec::new(),
                tessellation: tess,
            });
        }
        // The bottom, for a blind hole.
        let mut bottom_face = None;
        if spec.depth.is_some() {
            let id = FaceId(faces.len() as u32);
            let mut tess = TriangleMesh::default();
            if spec.point {
                let cone = rh / (59.0_f64).to_radians().tan();
                tess.vertices.push(wall_frame.origin + d * -cone);
                tess.vertices.extend(bottom_ring.iter().copied());
                for i in 0..SEGMENTS as u32 {
                    tess.triangles
                        .push([0, 1 + i, 1 + (i + 1) % SEGMENTS as u32]);
                }
                let f = Frame {
                    origin: wall_frame.origin,
                    x: wall_frame.x,
                    y: -wall_frame.y,
                    z: -d,
                };
                faces.push(BrepFace {
                    id,
                    surface: openchuck_types::brep::SurfaceKind::Cone,
                    body: 0,
                    edges: Vec::new(),
                    geometry: Surface::Cone {
                        frame: f,
                        radius: rh,
                        semi_angle: -(59.0_f64).to_radians(),
                    },
                    reversed: false,
                    uv_bounds: [0.0, TAU, 0.0, rh / (59.0_f64).to_radians().sin()],
                    trims: Vec::new(),
                    tessellation: tess,
                });
            } else {
                tess.vertices.push(wall_frame.origin);
                tess.vertices.extend(bottom_ring.iter().copied());
                for i in 0..SEGMENTS as u32 {
                    tess.triangles
                        .push([0, 1 + i, 1 + (i + 1) % SEGMENTS as u32]);
                }
                faces.push(BrepFace {
                    id,
                    surface: openchuck_types::brep::SurfaceKind::Plane,
                    body: 0,
                    edges: Vec::new(),
                    geometry: Surface::Plane { frame: wall_frame },
                    reversed: false,
                    uv_bounds: [-rh, rh, -rh, rh],
                    trims: vec![TrimLoop(
                        (0..SEGMENTS)
                            .map(|i| {
                                let t = TAU * i as f64 / SEGMENTS as f64;
                                UvPoint {
                                    u: rh * t.cos(),
                                    v: rh * t.sin(),
                                }
                            })
                            .collect(),
                    )],
                    tessellation: tess,
                });
            }
            bottom_face = Some(id);
        }
        // Edges: the rim on the blank's surface, and the bottom circle.
        let mut add_edge = |polyline: Vec<Point3>, other: FaceId| -> EdgeId {
            let id = EdgeId(edges.len() as u32);
            let mut shared = walls.clone();
            shared.push(other);
            edges.push(BrepEdge {
                id,
                faces: shared,
                vertices: None,
                polyline,
            });
            id
        };
        let rim = add_edge(top_ring.clone(), FaceId(0));
        let mut wall_edges = vec![rim];
        if let Some(bottom) = bottom_face {
            wall_edges.push(add_edge(bottom_ring.clone(), bottom));
        } else {
            wall_edges.push(add_edge(bottom_ring.clone(), FaceId(0)));
        }
        for w in &walls {
            if let Some(face) = faces.get_mut(w.0 as usize) {
                face.edges.clone_from(&wall_edges);
            }
        }
        axes.push((origin, d, rh));
        built.push(Built {
            walls,
            entry,
            direction: d,
            length: length_full,
            tip_depth: tip,
        });
    }

    // The blank's side, cut around every hole.
    let (nu, nv) = (360_usize, ((length / 0.5).ceil() as usize).max(1));
    let mut side = TriangleMesh::default();
    let vertex = |i: usize, j: usize| {
        let u = TAU * i as f64 / nu as f64;
        Point3::new(
            length * j as f64 / nv as f64,
            radius * u.cos(),
            radius * u.sin(),
        )
    };
    for j in 0..nv {
        for i in 0..nu {
            let quad = [
                vertex(i, j),
                vertex(i + 1, j),
                vertex(i + 1, j + 1),
                vertex(i, j + 1),
            ];
            let centre = Point3::new(
                0.25 * quad.iter().map(|p| p.x).sum::<f64>(),
                0.25 * quad.iter().map(|p| p.y).sum::<f64>(),
                0.25 * quad.iter().map(|p| p.z).sum::<f64>(),
            );
            let in_hole = axes.iter().any(|(o, d, rh)| {
                let v = centre - *o;
                let along = v.dot(*d);
                let off = v + *d * -along;
                // The cell's centre is within the hole plus a cell's diagonal.
                off.length() <= rh + 0.9
            });
            if in_hole {
                continue;
            }
            let base = side.vertices.len() as u32;
            side.vertices.extend(quad);
            side.triangles.push([base, base + 1, base + 2]);
            side.triangles.push([base, base + 2, base + 3]);
        }
    }
    let frame = Frame {
        origin: Point3::ORIGIN,
        x: Vec3::new(0.0, 1.0, 0.0),
        y: Vec3::new(0.0, 0.0, 1.0),
        z: Vec3::new(1.0, 0.0, 0.0),
    };
    if let Some(slot) = faces.first_mut() {
        *slot = BrepFace {
            id: FaceId(0),
            surface: openchuck_types::brep::SurfaceKind::Cylinder,
            body: 0,
            edges: (0..edges.len() as u32).map(EdgeId).collect(),
            geometry: Surface::Cylinder { frame, radius },
            reversed: false,
            uv_bounds: [0.0, TAU, 0.0, length],
            trims: Vec::new(),
            tessellation: side,
        };
    }
    (
        BrepModel {
            faces,
            edges,
            ..BrepModel::default()
        },
        built,
    )
}

fn placeholder() -> BrepFace {
    BrepFace {
        id: FaceId(0),
        surface: openchuck_types::brep::SurfaceKind::Other,
        body: 0,
        edges: Vec::new(),
        geometry: Surface::Plane {
            frame: Frame {
                origin: Point3::ORIGIN,
                x: Vec3::new(1.0, 0.0, 0.0),
                y: Vec3::new(0.0, 1.0, 0.0),
                z: Vec3::new(0.0, 0.0, 1.0),
            },
        },
        reversed: false,
        uv_bounds: [0.0, 1.0, 0.0, 1.0],
        trims: Vec::new(),
        tessellation: TriangleMesh::default(),
    }
}

/// A plain round bar of `radius` about the model X axis, from X = 0 to
/// `length`, whose side is split along X into one cylinder face per band:
/// the bands run between 0, each of `breaks` (in increasing order, inside
/// the bar) and `length`, and face `i` is band `i`. Each face is tessellated
/// on its own, 96 facets round and about 1 mm along, as a kernel does, so the
/// joined tessellation repeats the corners where two bands meet. For the face
/// selection tests of decision 0209 item 3 (a set of chosen faces machined as
/// the same triangles on a mesh).
pub fn banded_cylinder(radius: f64, length: f64, breaks: &[f64]) -> BrepModel {
    const ROUND: usize = 96;
    let frame = Frame {
        origin: Point3::ORIGIN,
        x: Vec3::new(0.0, 1.0, 0.0),
        y: Vec3::new(0.0, 0.0, 1.0),
        z: Vec3::new(1.0, 0.0, 0.0),
    };
    let ends: Vec<f64> = std::iter::once(0.0)
        .chain(breaks.iter().copied())
        .chain(std::iter::once(length))
        .collect();
    let faces = ends
        .iter()
        .zip(ends.iter().skip(1))
        .enumerate()
        .map(|(i, (&start, &end))| {
            let along = ((end - start).ceil() as usize).max(1);
            let mut tessellation = TriangleMesh::default();
            for j in 0..=along {
                let x = start + (end - start) * j as f64 / along as f64;
                for k in 0..ROUND {
                    let u = TAU * k as f64 / ROUND as f64;
                    tessellation
                        .vertices
                        .push(Point3::new(x, radius * u.cos(), radius * u.sin()));
                }
            }
            for j in 0..along as u32 {
                for k in 0..ROUND as u32 {
                    let next = (k + 1) % ROUND as u32;
                    let (a, b) = (j * ROUND as u32 + k, j * ROUND as u32 + next);
                    let (c, d) = (a + ROUND as u32, b + ROUND as u32);
                    // Wound so the normals face away from the axis.
                    tessellation.triangles.push([a, b, d]);
                    tessellation.triangles.push([a, d, c]);
                }
            }
            BrepFace {
                id: FaceId(i as u32),
                surface: openchuck_types::brep::SurfaceKind::Cylinder,
                body: 0,
                edges: Vec::new(),
                geometry: Surface::Cylinder { frame, radius },
                reversed: false,
                uv_bounds: [0.0, TAU, start, end],
                trims: Vec::new(),
                tessellation,
            }
        })
        .collect();
    BrepModel {
        faces,
        ..BrepModel::default()
    }
}
