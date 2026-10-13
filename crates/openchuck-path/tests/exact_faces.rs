// SPDX-License-Identifier: AGPL-3.0-or-later
//! The `SurfaceSource` port for exact faces (spike SP-9, decision record 0017
//! item 3; work breakdown slice 5): a trimmed cylinder face that implements
//! `ParametricFace`, a face set that implements `SurfaceSource` over such
//! faces, and the answers checked against closed forms. Slice 11 builds the
//! real face record and evaluators; this proves the contract they plug into.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing
)]

use openchuck_path::{
    Contact, FaceClosest, FacePosition, MeshSurface, ParametricFace, RayHit, SurfacePoint,
    SurfaceSource, ToolLine, Uv,
};
use openchuck_testkit::parts::{Cylinder, RadialPart};
use openchuck_types::geometry::{Aabb, Point3, UnitVec3, Vec3};
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::tool::profile::RevolvedProfile;

/// A cylinder of radius `r` about X, parameters (u = angle in radians from
/// +Y towards +Z, v = x), trimmed to a parameter box with a circular hole
/// (centre and radius in (u, v) units of mm along the surface).
#[derive(Debug)]
struct CylinderFace {
    r: f64,
    u: (f64, f64),
    v: (f64, f64),
    hole: Option<(f64, f64, f64)>, // (u centre, v centre, radius in mm on the surface)
}

impl CylinderFace {
    fn point(&self, uv: Uv) -> Point3 {
        Point3::new(uv.v, self.r * uv.u.cos(), self.r * uv.u.sin())
    }

    fn sp(&self, uv: Uv, face: u32) -> SurfacePoint {
        let n = UnitVec3::new(Vec3::new(0.0, uv.u.cos(), uv.u.sin())).unwrap();
        SurfacePoint {
            point: self.point(uv),
            normal: n,
            position: Some(FacePosition { face, uv }),
        }
    }
}

impl ParametricFace for CylinderFace {
    fn uv_bounds(&self) -> (Uv, Uv) {
        (
            Uv {
                u: self.u.0,
                v: self.v.0,
            },
            Uv {
                u: self.u.1,
                v: self.v.1,
            },
        )
    }

    fn point_normal(&self, uv: Uv) -> Option<SurfacePoint> {
        ((self.u.0..=self.u.1).contains(&uv.u) && (self.v.0..=self.v.1).contains(&uv.v))
            .then(|| self.sp(uv, 0))
    }

    fn closest(&self, p: Point3) -> Option<FaceClosest> {
        let u = p.z.atan2(p.y).clamp(self.u.0, self.u.1);
        let v = p.x.clamp(self.v.0, self.v.1);
        let uv = Uv { u, v };
        let sp = self.sp(uv, 0);
        Some(FaceClosest {
            uv,
            distance: sp.point.distance(p),
            point: sp,
        })
    }

    fn ray_hits(&self, o: Point3, d: UnitVec3, max: f64) -> Vec<RayHit> {
        let d = d.get();
        let a = d.y * d.y + d.z * d.z;
        let b = o.y * d.y + o.z * d.z;
        let c = o.y * o.y + o.z * o.z - self.r * self.r;
        let disc = b * b - a * c;
        if a < 1e-15 || disc < 0.0 {
            return Vec::new();
        }
        let mut out = Vec::new();
        for t in [(-b - disc.sqrt()) / a, (-b + disc.sqrt()) / a] {
            if !(0.0..=max).contains(&t) {
                continue;
            }
            let q = o + d * t;
            let uv = Uv {
                u: q.z.atan2(q.y),
                v: q.x,
            };
            if self.contains(uv) {
                let sp = self.sp(uv, 0);
                out.push(RayHit {
                    distance: t,
                    point: q,
                    normal: sp.normal,
                    position: sp.position,
                });
            }
        }
        out
    }

    fn contains(&self, uv: Uv) -> bool {
        let inside_box =
            (self.u.0..=self.u.1).contains(&uv.u) && (self.v.0..=self.v.1).contains(&uv.v);
        let in_hole = self
            .hole
            .is_some_and(|(hu, hv, hr)| (self.r * (uv.u - hu)).hypot(uv.v - hv) < hr);
        inside_box && !in_hole
    }
}

/// A source made of faces, with the tessellation it was built from.
#[derive(Debug)]
struct FaceSet {
    faces: Vec<Box<dyn ParametricFace>>,
    mesh: MeshSurface,
}

impl SurfaceSource for FaceSet {
    fn bounds(&self) -> Aabb {
        self.mesh.bounds()
    }

    fn project(&self, p: Point3) -> Option<SurfacePoint> {
        let mut best: Option<(f64, SurfacePoint)> = None;
        for (i, f) in self.faces.iter().enumerate() {
            if let Some(c) = f.closest(p)
                && best.is_none_or(|(d, _)| c.distance < d)
            {
                let mut sp = c.point;
                sp.position = Some(FacePosition {
                    face: u32::try_from(i).ok()?,
                    uv: c.uv,
                });
                best = Some((c.distance, sp));
            }
        }
        best.map(|(_, sp)| sp)
    }

    fn cast_ray(&self, o: Point3, d: UnitVec3, max: f64) -> Option<RayHit> {
        let mut best: Option<RayHit> = None;
        for (i, f) in self.faces.iter().enumerate() {
            if let Some(mut h) = f.ray_hits(o, d, max).into_iter().next()
                && best.is_none_or(|b| h.distance < b.distance)
            {
                h.position = h.position.map(|p| FacePosition {
                    face: u32::try_from(i).unwrap_or(u32::MAX),
                    ..p
                });
                best = Some(h);
            }
        }
        best
    }

    fn tessellation(&self) -> &TriangleMesh {
        self.mesh.tessellation()
    }

    fn drop_tool(&self, profile: &RevolvedProfile, line: &ToolLine) -> Option<Contact> {
        // No exact solver yet (OI-81): the fallback drops onto the tessellation.
        self.mesh.drop_tool(profile, line)
    }
}

fn set() -> FaceSet {
    let r = 15.0;
    let half = std::f64::consts::PI;
    let faces: Vec<Box<dyn ParametricFace>> = vec![
        // The upper half of the cylinder with a round hole in it...
        Box::new(CylinderFace {
            r,
            u: (0.0, half),
            v: (0.0, 40.0),
            hole: Some((0.5 * half, 20.0, 4.0)),
        }),
        // ...and the lower half.
        Box::new(CylinderFace {
            r,
            u: (-half, 0.0),
            v: (0.0, 40.0),
            hole: None,
        }),
    ];
    let part = Cylinder {
        radius: r,
        length: 40.0,
    };
    FaceSet {
        faces,
        mesh: MeshSurface::new(&part.mesh(3, 2880)).unwrap(),
    }
}

#[test]
fn req_imp_02_a_face_answers_point_normal_closest_ray_and_trim_queries() {
    let face = CylinderFace {
        r: 15.0,
        u: (0.0, std::f64::consts::PI),
        v: (0.0, 40.0),
        hole: Some((0.5 * std::f64::consts::PI, 20.0, 4.0)),
    };
    let (lo, hi) = face.uv_bounds();
    assert_eq!((lo.u, hi.v), (0.0, 40.0));
    // Point and unit normal at (u, v), outside the box refused.
    let sp = face.point_normal(Uv { u: 0.3, v: 12.0 }).unwrap();
    assert!((sp.point.y.hypot(sp.point.z) - 15.0).abs() < 1e-12);
    assert!((sp.normal.get().length() - 1.0).abs() < 1e-12);
    assert!(face.point_normal(Uv { u: -0.1, v: 12.0 }).is_none());
    // Closest point: radial projection, and its (u, v) reproduces the point.
    let c = face.closest(Point3::new(12.0, 0.0, 30.0)).unwrap();
    assert!((c.distance - 15.0).abs() < 1e-12);
    let again = face.point_normal(c.uv).unwrap();
    assert!(again.point.distance(c.point.point) < 1e-12);
    // The trim test: the hole is outside the face.
    assert!(face.contains(Uv { u: 0.3, v: 12.0 }));
    assert!(!face.contains(Uv {
        u: 0.5 * std::f64::consts::PI,
        v: 20.0
    }));
    assert!(!face.contains(Uv {
        u: 0.5 * std::f64::consts::PI,
        v: 41.0
    }));
    // Trimmed ray hits: straight down onto the hole misses the top, hits the
    // far wall nowhere on this face; beside the hole it hits the top.
    let down = UnitVec3::new(Vec3::new(0.0, 0.0, -1.0)).unwrap();
    assert!(
        face.ray_hits(Point3::new(20.0, 0.0, 50.0), down, 200.0)
            .is_empty()
    );
    let hits = face.ray_hits(Point3::new(10.0, 0.0, 50.0), down, 200.0);
    assert_eq!(hits.len(), 1);
    assert!((hits[0].point.z - 15.0).abs() < 1e-12 && hits[0].normal.get().z > 0.999);
    assert!((hits[0].distance - 35.0).abs() < 1e-12);
}

#[test]
fn req_imp_02_a_source_of_faces_answers_the_port_and_reports_the_face() {
    let source = set();
    // Project: nearest face wins and the answer says which face and where.
    let top = source.project(Point3::new(10.0, 0.0, 30.0)).unwrap();
    assert_eq!(top.position.unwrap().face, 0);
    let bottom = source.project(Point3::new(10.0, 0.0, -30.0)).unwrap();
    assert_eq!(bottom.position.unwrap().face, 1);
    assert!((bottom.point.z + 15.0).abs() < 1e-12 && bottom.normal.get().z < -0.999);
    // Ray: the first trimmed hit over all faces. Through the hole the ray
    // passes the top face and hits the bottom one.
    let down = UnitVec3::new(Vec3::new(0.0, 0.0, -1.0)).unwrap();
    let through_hole = source
        .cast_ray(Point3::new(20.0, 0.0, 50.0), down, 200.0)
        .unwrap();
    assert_eq!(through_hole.position.unwrap().face, 1);
    assert!((through_hole.point.z + 15.0).abs() < 1e-12);
    let beside = source
        .cast_ray(Point3::new(10.0, 0.0, 50.0), down, 200.0)
        .unwrap();
    assert_eq!(beside.position.unwrap().face, 0);
    // The distance default works through project.
    let d = source.distance(Point3::new(10.0, 0.0, 30.0)).unwrap();
    assert!((d - 15.0).abs() < 1e-12);
    // The tessellation is the mesh the source was built from.
    assert_eq!(
        source.tessellation().triangles.len(),
        2 * 2 * 2880 + 2 * 2880
    );
}

#[test]
fn req_imp_02_tessellation_fallback_drop_is_within_the_mesh_error() {
    use openchuck_types::tool::{Tool, ToolShape};
    use openchuck_types::units::Millimetres;
    let source = set();
    let tool = Tool {
        shape: ToolShape::BallEnd,
        diameter: Millimetres(6.0),
        flute_length: Millimetres(12.0),
        overall_length: Millimetres(50.0),
        shank_diameter: Millimetres(6.0),
        flute_count: 2,
        material: None,
    }
    .profile()
    .unwrap();
    let line = ToolLine {
        origin: Point3::new(30.0, 4.0, 0.0),
        axis: UnitVec3::Z,
    };
    let c = source.drop_tool(&tool, &line).unwrap();
    // Closed form for a ball of radius 3 over a cylinder of radius 15.
    let exact = ((15.0f64 + 3.0).powi(2) - 16.0).sqrt() - 3.0;
    let sagitta = 15.0 * (1.0 - (std::f64::consts::PI / 2880.0).cos());
    assert!((exact - c.tip.z).abs() <= 2.0 * sagitta + 1e-9);
}
