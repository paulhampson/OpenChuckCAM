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

/// Places round the rotary axis where a [`shouldered_shaft`] face is
/// tessellated and its circular edges are sampled.
const SHAFT_ROUND: usize = 96;

/// The frame of every revolved face of [`shouldered_shaft`]: z on the model
/// X axis, x on Y and y on Z, at X = `x`.
fn shaft_frame(x: f64) -> Frame {
    Frame {
        origin: Point3::new(x, 0.0, 0.0),
        x: Vec3::new(0.0, 1.0, 0.0),
        y: Vec3::new(0.0, 0.0, 1.0),
        z: Vec3::new(1.0, 0.0, 0.0),
    }
}

/// The point at X `x`, `r` from the axis, `u` radians round it from +Y
/// towards +Z.
fn shaft_point(x: f64, r: f64, u: f64) -> Point3 {
    Point3::new(x, r * u.cos(), r * u.sin())
}

/// The circle at X `x` and radius `r`, at the tessellation's places round
/// the axis, closed (its first point repeated last).
fn shaft_circle(x: f64, r: f64) -> Vec<Point3> {
    (0..=SHAFT_ROUND)
        .map(|k| shaft_point(x, r, TAU * k as f64 / SHAFT_ROUND as f64))
        .collect()
}

/// The surface of revolution through the profile `stations` (X, radius), as
/// rings joined in order, wound so each triangle faces along `outward`. A
/// station of radius 0 is the one point on the axis.
fn revolved(stations: &[(f64, f64)], outward: impl Fn(Point3) -> Vec3) -> TriangleMesh {
    let mut mesh = TriangleMesh::default();
    let mut rings: Vec<Vec<u32>> = Vec::new();
    for &(x, r) in stations {
        let base = mesh.vertices.len() as u32;
        if r == 0.0 {
            mesh.vertices.push(Point3::new(x, 0.0, 0.0));
            rings.push(vec![base; SHAFT_ROUND]);
        } else {
            mesh.vertices.extend(
                (0..SHAFT_ROUND).map(|k| shaft_point(x, r, TAU * k as f64 / SHAFT_ROUND as f64)),
            );
            rings.push((0..SHAFT_ROUND as u32).map(|k| base + k).collect());
        }
    }
    let push = |mesh: &mut TriangleMesh, t: [u32; 3]| {
        if t[0] == t[1] || t[1] == t[2] || t[0] == t[2] {
            return;
        }
        let [Some(a), Some(b), Some(c)] = t.map(|i| mesh.vertices.get(i as usize).copied()) else {
            return;
        };
        let centre = Point3::new(
            (a.x + b.x + c.x) / 3.0,
            (a.y + b.y + c.y) / 3.0,
            (a.z + b.z + c.z) / 3.0,
        );
        let facing = (b - a).cross(c - a).dot(outward(centre));
        mesh.triangles
            .push(if facing >= 0.0 { t } else { [t[0], t[2], t[1]] });
    };
    for (near, far) in rings.iter().zip(rings.iter().skip(1)) {
        let here = near.iter().zip(far);
        let next = near.iter().zip(far).cycle().skip(1);
        for ((&n0, &f0), (&n1, &f1)) in here.zip(next) {
            push(&mut mesh, [n0, n1, f1]);
            push(&mut mesh, [n0, f1, f0]);
        }
    }
    mesh
}

/// A face of [`shouldered_shaft`].
fn shaft_face(
    id: u32,
    geometry: Surface,
    reversed: bool,
    uv_bounds: [f64; 4],
    trims: Vec<TrimLoop>,
    tessellation: TriangleMesh,
) -> BrepFace {
    BrepFace {
        id: FaceId(id),
        surface: geometry.kind(),
        body: 0,
        edges: Vec::new(),
        geometry,
        reversed,
        uv_bounds,
        trims,
        tessellation,
    }
}

/// A flat face at X `x` facing +X (`forward`) or -X, from radius `inner`
/// (0 for a disc) to `outer`. The plane's (u, v) are Y and +Z or -Z.
fn shaft_ring(id: u32, x: f64, inner: f64, outer: f64, forward: bool) -> BrepFace {
    let (sign, normal) = if forward {
        (1.0, Vec3::new(1.0, 0.0, 0.0))
    } else {
        (-1.0, Vec3::new(-1.0, 0.0, 0.0))
    };
    let frame = Frame {
        origin: Point3::new(x, 0.0, 0.0),
        x: Vec3::new(0.0, 1.0, 0.0),
        y: Vec3::new(0.0, 0.0, sign),
        z: normal,
    };
    let circle = |r: f64| {
        TrimLoop(
            (0..SHAFT_ROUND)
                .map(|k| {
                    let u = TAU * k as f64 / SHAFT_ROUND as f64;
                    UvPoint {
                        u: r * u.cos(),
                        v: sign * r * u.sin(),
                    }
                })
                .collect(),
        )
    };
    let mut trims = vec![circle(outer)];
    if inner > 0.0 {
        trims.push(circle(inner));
    }
    shaft_face(
        id,
        Surface::Plane { frame },
        false,
        [-outer, outer, -outer, outer],
        trims,
        revolved(&[(x, inner), (x, outer)], |_| normal),
    )
}

/// The side of a cylinder of `radius` from X `from` to `to`, facing away
/// from the axis, tessellated about 1 mm along.
fn shaft_side(id: u32, radius: f64, from: f64, to: f64) -> BrepFace {
    let along = ((to - from).ceil() as usize).max(1);
    let stations: Vec<(f64, f64)> = (0..=along)
        .map(|j| (from + (to - from) * j as f64 / along as f64, radius))
        .collect();
    shaft_face(
        id,
        Surface::Cylinder {
            frame: shaft_frame(from),
            radius,
        },
        false,
        [0.0, TAU, 0.0, to - from],
        Vec::new(),
        revolved(&stations, |p| Vec3::new(0.0, p.y, p.z)),
    )
}

/// The edge `id` between `faces` along `polyline`.
fn shaft_edge(id: u32, faces: &[u32], polyline: Vec<Point3>) -> BrepEdge {
    BrepEdge {
        id: EdgeId(id),
        faces: faces.iter().map(|f| FaceId(*f)).collect(),
        vertices: None,
        polyline,
    }
}

/// A round bar about the model X axis with one sharp inside corner and one
/// rounded one, for pencil clean-up on B-rep edges (OP-09, OI-234 (b);
/// decision 0226). From X = 0 to 40:
///
/// | Face | What |
/// | --- | --- |
/// | 0 | the end at X = 0, a disc of radius 8 facing -X |
/// | 1 | a cylinder of radius 8 from X = 0 to 12 |
/// | 2 | the shoulder at X = 12, a ring from radius 8 to 12 facing -X |
/// | 3 | a cylinder of radius 12 from X = 12 to 24 |
/// | 4 | the shoulder at X = 24, a ring from radius 10 to 12 facing +X |
/// | 5 | a quarter-round fillet of radius 2 from the shoulder (X 24, radius 10) to the bar (X 26, radius 8): a torus about X, tangent to both |
/// | 6 | a cylinder of radius 8 from X = 26 to 40 |
/// | 7 | the end at X = 40, a disc of radius 8 facing +X |
///
/// Edges 0 to 6 are the circles between faces `i` and `i + 1`: edge 1 (X
/// 12, radius 8) is the one sharp inside corner, a 90 degree concave
/// crease; edges 0, 2, 3 and 6 are outside corners; edges 4 and 5 are the
/// fillet's tangent boundaries. Edges 7 to 10 are the seams of faces 1, 3,
/// 5 and 6 at +Y. The fillet is tessellated in `fillet_steps` facets across
/// its quarter turn, so neighbouring facets turn 90 / `fillet_steps` degrees
/// and a crease found in the facets at a smaller angle is a false one.
pub fn shouldered_shaft(fillet_steps: usize) -> BrepModel {
    let steps = fillet_steps.max(1);
    // The fillet: the tube's centre circle is at X 26, radius 10; v runs
    // from -pi (X 26, radius 8) to -pi/2 (X 24, radius 10).
    let (cx, cr, minor) = (26.0, 10.0, 2.0);
    let v =
        |j: usize| -std::f64::consts::PI + std::f64::consts::FRAC_PI_2 * j as f64 / steps as f64;
    let fillet_stations: Vec<(f64, f64)> = (0..=steps)
        .map(|j| (cx + minor * v(j).sin(), cr + minor * v(j).cos()))
        .collect();
    let fillet = shaft_face(
        5,
        Surface::Torus {
            frame: shaft_frame(cx),
            major: cr,
            minor,
        },
        // The material is outside the tube: out of the material is towards
        // the tube's centre.
        true,
        [
            0.0,
            TAU,
            -std::f64::consts::PI,
            -std::f64::consts::FRAC_PI_2,
        ],
        Vec::new(),
        revolved(&fillet_stations, |p| {
            let r = p.y.hypot(p.z);
            let towards = cr - r;
            Vec3::new(cx - p.x, p.y / r * towards, p.z / r * towards)
        }),
    );
    let seam = |id: u32, face: u32, points: &[(f64, f64)]| {
        shaft_edge(
            id,
            &[face],
            points
                .iter()
                .map(|&(x, r)| shaft_point(x, r, 0.0))
                .collect(),
        )
    };
    BrepModel {
        faces: vec![
            shaft_ring(0, 0.0, 0.0, 8.0, false),
            shaft_side(1, 8.0, 0.0, 12.0),
            shaft_ring(2, 12.0, 8.0, 12.0, false),
            shaft_side(3, 12.0, 12.0, 24.0),
            shaft_ring(4, 24.0, 10.0, 12.0, true),
            fillet,
            shaft_side(6, 8.0, 26.0, 40.0),
            shaft_ring(7, 40.0, 0.0, 8.0, true),
        ],
        edges: vec![
            shaft_edge(0, &[0, 1], shaft_circle(0.0, 8.0)),
            shaft_edge(1, &[1, 2], shaft_circle(12.0, 8.0)),
            shaft_edge(2, &[2, 3], shaft_circle(12.0, 12.0)),
            shaft_edge(3, &[3, 4], shaft_circle(24.0, 12.0)),
            shaft_edge(4, &[4, 5], shaft_circle(24.0, 10.0)),
            shaft_edge(5, &[5, 6], shaft_circle(26.0, 8.0)),
            shaft_edge(6, &[6, 7], shaft_circle(40.0, 8.0)),
            seam(7, 1, &[(0.0, 8.0), (12.0, 8.0)]),
            seam(8, 3, &[(12.0, 12.0), (24.0, 12.0)]),
            seam(9, 5, &fillet_stations),
            seam(10, 6, &[(26.0, 8.0), (40.0, 8.0)]),
        ],
        ..BrepModel::default()
    }
}
