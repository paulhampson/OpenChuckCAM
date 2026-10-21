// SPDX-License-Identifier: AGPL-3.0-or-later
//! Swarf (flank) machining on ruled B-rep faces (OP-07; work breakdown
//! slice 17; decision records 0021 and 0061): the analytic oracle cases
//! (cylinder, cone, plane, helicoid; sphere and torus refused), the
//! independent checker, the A limits, the kinematic sign and determinism.
//! No kernel is involved: the faces are built from closed forms.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::cast_precision_loss
)]

use openchuck_path::params::names;
use openchuck_path::swarf::{self, Family, Flank, Settings, detect_rulings, face_path};
use openchuck_path::{
    AcceptAll, AnalyticStock, BrepSurface, FaceClosest, FaceRef, MeshSurface, ParameterValue,
    ParametricFace, RayHit, RegionConstraint, Registry, RotaryMapping, Strategy, StrategyInput,
    SurfacePoint, SurfaceSource, Swarf, ToolLine, Uv, Window, generate, generate_in_region,
};
use openchuck_testkit::gouge::Outline;
use openchuck_testkit::ruled::{
    boss_cylinder, coaxial_cylinder, cross_plane, draft_cone, floor_plane, pocket_cylinder, point,
    sphere_band, torus_band, wall_plane,
};
use openchuck_types::brep::BrepModel;
use openchuck_types::geometry::{Aabb, Point3, UnitVec3, Vec3};
use openchuck_types::ir::{MoveKind, PathTags, ToolPose, ToolpathGroup};
use openchuck_types::job::{CancelToken, JobContext, NoProgress};
use openchuck_types::machine::{ARange, Clearance, MachineProfile};
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::tool::profile::RevolvedProfile;
use openchuck_types::tool::{Tool, ToolShape};
use openchuck_types::units::{Degrees, Millimetres};
use std::collections::BTreeMap;

// ---- fixtures -------------------------------------------------------------

const BOSS_X: f64 = 30.0;
const BOSS_RADIUS: f64 = 8.0;
const Z0: f64 = 10.0;
const Z1: f64 = 18.0;
const TOOL_RADIUS: f64 = 3.0;
const TOL: f64 = 0.01;
const LIMIT: f64 = swarf::DEFAULT_MAX_MATERIAL_LEFT;

fn tool(shape: ToolShape, flute_length: f64) -> RevolvedProfile {
    Tool {
        shape,
        diameter: Millimetres(2.0 * TOOL_RADIUS),
        flute_length: Millimetres(flute_length),
        overall_length: Millimetres(60.0),
        shank_diameter: Millimetres(2.0 * TOOL_RADIUS),
        flute_count: 2,
        material: None,
        centre_cutting: None,
    }
    .profile()
    .unwrap()
}

fn flat() -> RevolvedProfile {
    tool(ToolShape::FlatEnd, 15.0)
}

fn tapered(half_angle_degrees: f64) -> RevolvedProfile {
    tool(
        ToolShape::TaperedBall {
            tip_radius: Millimetres(1.0),
            taper_half_angle: Degrees(half_angle_degrees),
        },
        20.0,
    )
}

fn machine() -> MachineProfile {
    MachineProfile::example_x_rotary()
}

fn values(extra: &[(&str, ParameterValue)]) -> BTreeMap<String, ParameterValue> {
    let mut v: BTreeMap<String, ParameterValue> =
        [(names::FEED, 600.0), (names::PLUNGE_FEED, 200.0)]
            .into_iter()
            .map(|(k, x)| (k.to_owned(), ParameterValue::Number(x)))
            .collect();
    for (k, x) in extra {
        v.insert((*k).to_owned(), x.clone());
    }
    v
}

fn num(x: f64) -> ParameterValue {
    ParameterValue::Number(x)
}

fn text(s: &str) -> ParameterValue {
    ParameterValue::Text(s.to_owned())
}

struct Fixture {
    surface: Box<dyn SurfaceSource>,
    stock: AnalyticStock,
    tool: RevolvedProfile,
    tags: PathTags,
    rotary: RotaryMapping,
}

impl Fixture {
    fn brep(model: &BrepModel, tool: RevolvedProfile) -> Self {
        Self::with_surface(Box::new(BrepSurface::new(model).unwrap()), tool)
    }

    fn with_surface(surface: Box<dyn SurfaceSource>, tool: RevolvedProfile) -> Self {
        Self {
            surface,
            stock: AnalyticStock::Cylinder {
                radius: 22.0,
                x_start: 0.0,
                x_end: 60.0,
            },
            tool,
            tags: PathTags {
                operation: "swarf".to_owned(),
                setup: "s1".to_owned(),
                tool: "t1".to_owned(),
            },
            rotary: RotaryMapping::of_profile(&machine()),
        }
    }

    fn input(&self) -> StrategyInput<'_> {
        StrategyInput {
            surface: self.surface.as_ref(),
            stock: &self.stock,
            collision: &AcceptAll,
            tool: &self.tool,
            rotary: self.rotary,
            clearance: Clearance {
                radius: Millimetres(40.0),
                safe_z: Millimetres(50.0),
            },
            max_feeds: machine().max_feeds,
            tags: &self.tags,
            keep_out: &[],
        }
    }

    fn settings(&self) -> Settings {
        Settings {
            tolerance: TOL,
            stock_to_leave: 0.0,
            tip_extension: 0.0,
            max_material_left: LIMIT,
            reverse_tip_end: false,
            reverse_direction: false,
            boundary_limits_pass: true,
            boundary_limits_tip: true,
        }
    }

    fn face(&self) -> FaceRef<'_> {
        self.surface.exact_faces().into_iter().next().unwrap()
    }
}

fn run<T>(f: impl FnOnce(&JobContext<'_>) -> T) -> T {
    let cancel = CancelToken::new();
    let ctx = JobContext::new(&cancel, &NoProgress);
    f(&ctx)
}

fn generated(fixture: &Fixture, extra: &[(&str, ParameterValue)]) -> openchuck_path::GeneratedPath {
    run(|ctx| generate(&Swarf, &fixture.input(), &values(extra), ctx)).unwrap()
}

/// The one warning of `path`, which must be swarf's overcut warning: it
/// points at the operation's tolerance (decision 0175).
fn overcut_warning(path: &openchuck_path::GeneratedPath) -> String {
    assert_eq!(path.warnings.len(), 1, "{:?}", path.warnings);
    let warning = &path.warnings[0];
    assert_eq!(warning.parameter.as_deref(), Some(names::TOLERANCE));
    warning.message.clone()
}

fn refused(fixture: &Fixture, extra: &[(&str, ParameterValue)]) -> String {
    run(|ctx| generate(&Swarf, &fixture.input(), &values(extra), ctx))
        .expect_err("the operation is refused")
        .to_string()
}

/// The poses of the cutting moves, with the pose each run of them starts
/// from (the end of the entry), so the moves can be swept.
fn cutting_poses(group: &ToolpathGroup) -> Vec<ToolPose> {
    let mut out = Vec::new();
    let mut previous = group.start;
    let mut cutting = false;
    for s in &group.segments {
        if s.kind == MoveKind::Cutting {
            if !cutting {
                out.push(previous);
            }
            out.push(s.end);
        }
        cutting = s.kind == MoveKind::Cutting;
        previous = s.end;
    }
    out
}

/// Signed clearance of the exact surface sample `p` from the tool at `pose`,
/// by the independent checker.
fn checker_clearance(outline: &Outline, pose: &ToolPose, p: Point3) -> f64 {
    let w = p - pose.tip;
    let z = w.dot(pose.axis.get());
    let r = (w - pose.axis.get() * z).length();
    outline.clearance(r, z)
}

/// The smallest clearance of `p` over all poses: negative by the deepest
/// gouge, about the stock to leave where the flank reaches the sample.
fn best_clearance(outline: &Outline, poses: &[ToolPose], p: Point3) -> f64 {
    poses
        .iter()
        .map(|pose| checker_clearance(outline, pose, p))
        .fold(f64::INFINITY, f64::min)
}

/// A model point as the machine sees it at angle `a`, from first principles
/// for the example machine: positive A turns the part by the right-hand
/// rule about +X, so a model point appears rotated by +A.
fn to_machine(p: Point3, a: f64) -> Point3 {
    let (s, c) = a.to_radians().sin_cos();
    Point3::new(p.x, c * p.y - s * p.z, s * p.y + c * p.z)
}

fn to_model(m: Point3, a: f64) -> Point3 {
    to_machine(m, -a)
}

/// The poses the machine passes through: the cutting poses and `steps - 1`
/// points of each straight move between them (X, Y, Z and A linear in the
/// machine frame), so the checker sees the swept flank and not only the
/// discrete poses (whose scallop a tool of a small radius would show).
fn motion(poses: &[ToolPose], steps: usize) -> Vec<ToolPose> {
    let mut out = Vec::new();
    for w in poses.windows(2) {
        let (p, q) = (w[0], w[1]);
        let (mp, mq) = (to_machine(p.tip, p.a.0), to_machine(q.tip, q.a.0));
        for k in 0..steps {
            let f = k as f64 / steps as f64;
            let a = p.a.0 + (q.a.0 - p.a.0) * f;
            let m = mp + (mq - mp) * f;
            let tip = to_model(m, a);
            let axis = UnitVec3::new(to_model(Point3::new(0.0, 0.0, 1.0), a).to_vec()).unwrap();
            out.push(ToolPose {
                tip,
                axis,
                a: Degrees(a),
            });
        }
    }
    if let Some(last) = poses.last() {
        out.push(*last);
    }
    out
}

/// Samples of the exact surface of a single-face model over its parameter
/// box, `nu` by `nv`, inset from the ends of the rulings by `inset` of the
/// range (the first and last pose cover the ends only to the chord).
fn samples(model: &BrepModel, nu: usize, nv: usize, inset: f64) -> Vec<Point3> {
    let face = &model.faces[0];
    let b = face.uv_bounds;
    let mut out = Vec::new();
    for i in 0..=nu {
        for j in 0..=nv {
            let u = b[0] + (b[1] - b[0]) * i as f64 / nu as f64;
            let v = b[2] + (b[3] - b[2]) * (inset + (1.0 - 2.0 * inset) * j as f64 / nv as f64);
            out.push(point(&face.geometry, u, v));
        }
    }
    out
}

/// The checker's verdict on a path over the exact surface: the deepest gouge
/// and the largest material left where the flank should have reached.
fn verdict(outline: &Outline, poses: &[ToolPose], points: &[Point3], leave: f64) -> (f64, f64) {
    let swept = motion(poses, 8);
    let mut gouge = 0.0f64;
    let mut left = 0.0f64;
    for p in points {
        let c = best_clearance(outline, &swept, *p) - leave;
        if c < 0.0 {
            gouge = gouge.max(-c);
        } else {
            left = left.max(c);
        }
    }
    (gouge, left)
}

// ---- a helicoid as a hand-written exact face --------------------------------

/// The helicoid `S(u, v) = (pitch u, v cos u, v sin u)` about the rotary
/// axis: exactly ruled (its rulings are radial lines perpendicular to X),
/// developable only in the limit of zero pitch. The tool side is -X.
#[derive(Debug)]
struct Helicoid {
    pitch: f64,
    u: (f64, f64),
    v: (f64, f64),
}

impl Helicoid {
    fn at(&self, u: f64, v: f64) -> Point3 {
        Point3::new(self.pitch * u, v * u.cos(), v * u.sin())
    }

    fn normal(&self, u: f64, v: f64) -> UnitVec3 {
        UnitVec3::new(Vec3::new(-v, -self.pitch * u.sin(), self.pitch * u.cos())).unwrap()
    }
}

impl ParametricFace for Helicoid {
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
        self.contains(uv).then(|| SurfacePoint {
            point: self.at(uv.u, uv.v),
            normal: self.normal(uv.u, uv.v),
            position: None,
        })
    }

    fn closest(&self, point: Point3) -> Option<FaceClosest> {
        let n = 64;
        let mut best: Option<(f64, Uv)> = None;
        for i in 0..=n {
            for j in 0..=n {
                let uv = Uv {
                    u: self.u.0 + (self.u.1 - self.u.0) * i as f64 / n as f64,
                    v: self.v.0 + (self.v.1 - self.v.0) * j as f64 / n as f64,
                };
                let d = self.at(uv.u, uv.v).distance(point);
                if best.is_none_or(|b| d < b.0) {
                    best = Some((d, uv));
                }
            }
        }
        let (distance, uv) = best?;
        Some(FaceClosest {
            uv,
            point: self.point_normal(uv)?,
            distance,
        })
    }

    fn ray_hits(&self, _origin: Point3, _direction: UnitVec3, _max: f64) -> Vec<RayHit> {
        Vec::new()
    }

    fn contains(&self, uv: Uv) -> bool {
        (self.u.0..=self.u.1).contains(&uv.u) && (self.v.0..=self.v.1).contains(&uv.v)
    }
}

/// A `SurfaceSource` over hand-written exact faces, numbered in order; the
/// mesh queries come from a tessellation of the first face.
#[derive(Debug)]
struct ExactFaces {
    faces: Vec<Box<dyn ParametricFace>>,
    mesh: MeshSurface,
}

impl ExactFaces {
    fn helicoid(pitch: f64, degrees: f64) -> Self {
        let h = Helicoid {
            pitch,
            u: (0.0, degrees.to_radians()),
            v: (8.0, 20.0),
        };
        let mut mesh = TriangleMesh::default();
        let (nu, nv) = (64, 4);
        for i in 0..=nu {
            for j in 0..=nv {
                let u = h.u.0 + (h.u.1 - h.u.0) * i as f64 / nu as f64;
                let v = h.v.0 + (h.v.1 - h.v.0) * j as f64 / nv as f64;
                mesh.vertices.push(h.at(u, v));
            }
        }
        let at = |i: usize, j: usize| (i * (nv + 1) + j) as u32;
        for i in 0..nu {
            for j in 0..nv {
                mesh.triangles
                    .push([at(i, j), at(i + 1, j + 1), at(i + 1, j)]);
                mesh.triangles
                    .push([at(i, j), at(i, j + 1), at(i + 1, j + 1)]);
            }
        }
        Self {
            faces: vec![Box::new(h)],
            mesh: MeshSurface::new(&mesh).unwrap(),
        }
    }
}

impl SurfaceSource for ExactFaces {
    fn bounds(&self) -> Aabb {
        self.mesh.bounds()
    }

    fn project(&self, point: Point3) -> Option<SurfacePoint> {
        self.mesh.project(point)
    }

    fn cast_ray(&self, origin: Point3, direction: UnitVec3, max: f64) -> Option<RayHit> {
        self.mesh.cast_ray(origin, direction, max)
    }

    fn tessellation(&self) -> &TriangleMesh {
        self.mesh.tessellation()
    }

    fn drop_tool(
        &self,
        profile: &RevolvedProfile,
        line: &ToolLine,
    ) -> Option<openchuck_path::Contact> {
        self.mesh.drop_tool(profile, line)
    }

    fn exact_faces(&self) -> Vec<FaceRef<'_>> {
        self.faces
            .iter()
            .enumerate()
            .map(|(i, f)| FaceRef {
                number: i as u32,
                face: f.as_ref(),
            })
            .collect()
    }
}

fn helicoid_samples(pitch: f64, degrees: f64, nu: usize, nv: usize, inset: f64) -> Vec<Point3> {
    let h = Helicoid {
        pitch,
        u: (0.0, degrees.to_radians()),
        v: (8.0, 20.0),
    };
    let mut out = Vec::new();
    for i in 0..=nu {
        for j in 0..=nv {
            let u = h.u.0 + (h.u.1 - h.u.0) * i as f64 / nu as f64;
            let v = h.v.0 + (h.v.1 - h.v.0) * (inset + (1.0 - 2.0 * inset) * j as f64 / nv as f64);
            out.push(h.at(u, v));
        }
    }
    out
}

// ---- detection ---------------------------------------------------------------

#[test]
fn req_op_07_detection_fits_rulings_on_ruled_faces_and_refuses_a_sphere_and_a_torus() {
    let ruled = [
        (
            boss_cylinder(BOSS_X, BOSS_RADIUS, Z0, Z1),
            Family::AlongV,
            0.0,
        ),
        (
            draft_cone(BOSS_X, 10.0, 6f64.to_radians(), 10.0, 16.0),
            Family::AlongV,
            0.0,
        ),
        (wall_plane(8.0, 22.0, 38.0, Z0, Z1), Family::AlongU, 0.0),
    ];
    for (model, family, twist) in ruled {
        let f = Fixture::brep(&model, flat());
        let d = detect_rulings(f.face().face, TOL);
        assert_eq!(d.chosen, Some(family), "{d:?}");
        assert!(d.fit(family).residual < 1e-9, "{d:?}");
        assert!((d.fit(family).twist - twist).abs() < 1e-6, "{d:?}");
        assert!(d.refusal().is_none());
    }
    for model in [
        sphere_band(BOSS_X, 12.0, -0.6, 0.6),
        torus_band(BOSS_X, 12.0, 4.0),
    ] {
        let f = Fixture::brep(&model, flat());
        let d = detect_rulings(f.face().face, TOL);
        assert_eq!(d.chosen, None, "{d:?}");
        assert!(d.best().residual > 0.5, "{d:?}");
        let why = d.refusal().unwrap();
        assert!(why.contains("not a ruled surface"), "{why}");
        assert!(why.contains("limit 0.0100 mm"), "{why}");
        let message = refused(&f, &[]);
        assert!(message.contains("swarf cannot cut face 0"), "{message}");
        assert!(message.contains("misses by"), "{message}");
    }
    // The helicoid is ruled exactly, with a twist the detection reports.
    let faces = ExactFaces::helicoid(1.5, 200.0);
    let d = detect_rulings(faces.exact_faces()[0].face, TOL);
    assert_eq!(d.chosen, Some(Family::AlongV), "{d:?}");
    assert!(d.v.residual < 1e-9, "{d:?}");
    assert!(d.v.twist > 2.0 && d.v.twist < 6.0, "{d:?}");
}

#[test]
fn req_op_07_the_flank_is_the_straight_side_of_the_tool_as_one_cone() {
    let f = Flank::of(&flat()).unwrap();
    assert_eq!(f.half_angle, 0.0);
    assert_eq!((f.z_start, f.z_end), (0.0, 15.0));
    assert_eq!((f.r_start, f.r_end), (TOOL_RADIUS, TOOL_RADIUS));
    let ball = tool(ToolShape::BallEnd, 12.0);
    let f = Flank::of(&ball).unwrap();
    assert_eq!(f.half_angle, 0.0);
    assert_eq!((f.z_start, f.z_end), (TOOL_RADIUS, 12.0));
    let t = Flank::of(&tapered(6.0)).unwrap();
    assert!((t.half_angle.to_degrees() - 6.0).abs() < 1e-9, "{t:?}");
    assert!(t.z_start > 0.0 && t.r_start < TOOL_RADIUS && t.r_end == TOOL_RADIUS);
    let v = Flank::of(&tool(
        ToolShape::VBit {
            included_angle: Degrees(60.0),
            tip_diameter: Millimetres(0.0),
        },
        15.0,
    ))
    .unwrap();
    assert!((v.half_angle.to_degrees() - 30.0).abs() < 1e-9, "{v:?}");
    // A ball whose flute length is its radius has no straight side.
    let none = Flank::of(&tool(ToolShape::BallEnd, TOOL_RADIUS)).unwrap_err();
    assert!(none.to_string().contains("no straight flank"), "{none}");
}

// ---- the oracle cases --------------------------------------------------------

#[test]
fn req_op_07_a_radial_cylinder_boss_is_cut_with_the_flank_at_constant_a_within_tolerance() {
    let model = boss_cylinder(BOSS_X, BOSS_RADIUS, Z0, Z1);
    let f = Fixture::brep(&model, flat());
    let path = run(|ctx| {
        face_path(
            f.face(),
            &f.tool,
            f.rotary,
            &f.settings(),
            &RegionConstraint::default(),
            ctx,
        )
    })
    .unwrap();
    assert!(path.closed, "a whole boss is one closed run");
    assert_eq!(path.runs.len(), 1);
    assert!(path.max_tilt < 1e-9, "{}", path.max_tilt);
    assert!(path.twist < 1e-9);
    assert!(path.gouge <= TOL && path.left <= TOL, "{path:?}");
    assert!(path.refusal(LIMIT).is_none());
    let poses: Vec<_> = path.poses().collect();
    assert!(poses.len() > 50, "{} poses", poses.len());
    for p in &poses {
        // The tool is vertical, its tip at the foot of the wall and its axis
        // one tool radius outside the boss (closed form).
        assert!(
            (p.a.0.rem_euclid(360.0)).min(360.0 - p.a.0.rem_euclid(360.0)) < 1e-9,
            "{p:?}"
        );
        assert!((p.tip.z - Z0).abs() < 1e-9, "{p:?}");
        let r = (p.tip.x - BOSS_X).hypot(p.tip.y);
        assert!((r - (BOSS_RADIUS + TOOL_RADIUS)).abs() < 1e-9, "{p:?}");
        assert!((p.contact.z - 0.5 * (Z0 + Z1)).abs() < 1e-9, "{p:?}");
        assert!((p.ruling_length - (Z1 - Z0)).abs() < 1e-9);
    }
    let (first, last) = (poses[0], poses[poses.len() - 1]);
    assert!(first.tip.distance(last.tip) < 1e-9, "the run closes");
    // Through the strategy and the shared services: one pass, A constant,
    // the independent checker finds neither gouge nor uncut wall.
    let path = generated(&f, &[]);
    let poses = cutting_poses(&path.group);
    assert!(poses.len() > 50);
    assert!(poses.iter().all(|p| p.a.0.abs() < 1e-9), "A stays at zero");
    let outline = Outline::new(&flat());
    let (gouge, left) = verdict(&outline, &poses, &samples(&model, 180, 8, 0.05), 0.0);
    assert!(gouge <= TOL, "gouge {gouge}");
    assert!(left <= TOL, "left {left}");
    assert!(
        path.notes
            .iter()
            .any(|n| n.message.contains("rulings along v")),
        "{:?}",
        path.notes
    );
}

#[test]
fn req_op_07_a_plane_wall_uses_the_family_the_four_axes_can_reach() {
    // The wall is ruled in both families; the lines along X cannot be cut on
    // four axes, so the lines along Z are used (decision 0061 item 1).
    let model = wall_plane(8.0, 22.0, 38.0, Z0, Z1);
    let f = Fixture::brep(&model, flat());
    let path = run(|ctx| {
        face_path(
            f.face(),
            &f.tool,
            f.rotary,
            &f.settings(),
            &RegionConstraint::default(),
            ctx,
        )
    })
    .unwrap();
    assert_eq!(path.family, Family::AlongU);
    assert!(!path.closed);
    assert_eq!(path.runs.len(), 1);
    assert!(
        path.max_tilt < 1e-9 && path.gouge <= TOL && path.left <= TOL,
        "{path:?}"
    );
    for p in path.poses() {
        assert!((p.tip.y - (8.0 + TOOL_RADIUS)).abs() < 1e-9, "{p:?}");
        assert!((p.tip.z - Z0).abs() < 1e-9, "{p:?}");
        assert!(p.a.0.abs() < 1e-9);
    }
    // A straight wall needs no extra poses: the initial spacing is kept.
    assert_eq!(path.poses().count(), 5);
    let xs: Vec<f64> = path.poses().map(|p| p.tip.x).collect();
    assert!(xs.windows(2).all(|w| w[1] > w[0]), "{xs:?}");
}

#[test]
fn req_op_07_a_draft_cone_needs_the_matching_taper_and_a_straight_tool_is_refused_naming_it() {
    let half = 6f64;
    let model = draft_cone(BOSS_X, 10.0, half.to_radians(), 10.0, 16.0);
    // The matching taper: the tool axis is vertical and the flank follows
    // the cone exactly.
    let f = Fixture::brep(&model, tapered(half));
    let path = run(|ctx| {
        face_path(
            f.face(),
            &f.tool,
            f.rotary,
            &f.settings(),
            &RegionConstraint::default(),
            ctx,
        )
    })
    .unwrap();
    assert!(path.max_tilt < 1e-9, "{}", path.max_tilt);
    assert!(path.gouge <= TOL && path.left <= TOL, "{path:?}");
    assert!(path.suggested_taper.is_none());
    let flank = Flank::of(&tapered(half)).unwrap();
    for p in path.poses() {
        assert!(
            p.a.0.abs() < 1e-9 || (p.a.0.abs() - 360.0).abs() < 1e-9,
            "{p:?}"
        );
        // The tip sits where the flank's start meets the foot of the wall.
        let foot = point(&model.faces[0].geometry, 0.0, 10.0);
        assert!((p.tip.z - (foot.z - flank.z_start)).abs() < 1e-9, "{p:?}");
    }
    let generated = generated(&f, &[]);
    let poses = cutting_poses(&generated.group);
    let (gouge, left) = verdict(
        &Outline::new(&tapered(half)),
        &poses,
        &samples(&model, 180, 8, 0.05),
        0.0,
    );
    assert!(gouge <= TOL, "gouge {gouge}");
    assert!(left <= TOL, "left {left}");
    // A straight tool is off by the draft angle where the rulings lean
    // along X, and gouges: refused, naming the tilt and the taper that
    // would remove it (OI-94).
    let f = Fixture::brep(&model, flat());
    let path = run(|ctx| {
        face_path(
            f.face(),
            &f.tool,
            f.rotary,
            &f.settings(),
            &RegionConstraint::default(),
            ctx,
        )
    })
    .unwrap();
    assert!((path.max_tilt - half).abs() < 0.01, "{}", path.max_tilt);
    assert!(path.gouge > TOL, "{path:?}");
    assert!(
        path.suggested_taper
            .is_some_and(|a| (a - half).abs() < 0.01),
        "{path:?}"
    );
    // The overcut alone would be a warning (decision 0175, issue #226), but
    // the tilted flank also leaves more than the limit on the other side of
    // the ruling, which is still refused, naming the tilt and the taper that
    // would remove it; the warning names them too.
    let message = refused(&f, &[]);
    assert!(message.contains("would leave"), "{message}");
    assert!(
        message.contains("four-axis tilt reaches 6.00 degrees"),
        "{message}"
    );
    assert!(
        message.contains("flank half angle of 6.00 degrees would remove it"),
        "{message}"
    );
    let warning = path.overcut_warning(TOL).unwrap();
    assert!(
        warning.contains("cuts into the finished surface by")
            && warning.contains("flank half angle of 6.00 degrees would remove it"),
        "{warning}"
    );
}

#[test]
fn req_op_07_a_helicoid_is_accepted_at_a_small_twist_and_refused_with_the_twist_named_at_a_large_one()
 {
    // Pitch 0.3 mm per radian: the normal twists under a degree along the
    // 12 mm rulings and the straight flank stays within tolerance.
    let faces = ExactFaces::helicoid(0.3, 200.0);
    let f = Fixture::with_surface(Box::new(faces), flat());
    let path = run(|ctx| {
        face_path(
            f.face(),
            &f.tool,
            f.rotary,
            &f.settings(),
            &RegionConstraint::default(),
            ctx,
        )
    })
    .unwrap();
    assert!(path.twist > 0.3 && path.twist < 1.0, "{}", path.twist);
    assert!(path.max_tilt < 1e-9, "the rulings are perpendicular to X");
    assert!(path.gouge <= TOL, "{path:?}");
    assert!(path.refusal(LIMIT).is_none());
    // A turns with the rulings: continuous, through the face's 200 degrees.
    let a: Vec<f64> = path.poses().map(|p| p.a.0).collect();
    assert!(a.windows(2).all(|w| (w[1] - w[0]).abs() < 10.0), "{a:?}");
    assert!(
        ((a[a.len() - 1] - a[0]).abs() - 200.0).abs() < 1e-6,
        "{a:?}"
    );
    // The tip sits one tool radius off the inner end of each ruling (the
    // end nearer the rotary axis), square to the ruling.
    for p in path.poses() {
        let inner = Point3::new(0.3 * p.t, 8.0 * p.t.cos(), 8.0 * p.t.sin());
        let d = Vec3::new(0.0, p.t.cos(), p.t.sin());
        assert!((p.tip.distance(inner) - TOOL_RADIUS).abs() < 1e-9, "{p:?}");
        assert!((p.tip - inner).dot(d).abs() < 1e-9, "{p:?}");
    }
    let generated = generated(&f, &[]);
    let poses = cutting_poses(&generated.group);
    let (gouge, _) = verdict(
        &Outline::new(&flat()),
        &poses,
        &helicoid_samples(0.3, 200.0, 200, 8, 0.05),
        0.0,
    );
    assert!(gouge <= TOL, "gouge {gouge}");
    assert!(
        generated
            .notes
            .iter()
            .any(|n| n.message.contains("the normal twists") && n.message.contains("degrees")),
        "{:?}",
        generated.notes
    );
    assert!(generated.warnings.is_empty(), "{:?}", generated.warnings);
    // Pitch 3: the twist is over ten degrees and the flank gouges either
    // side of the ruling. The path is made with a warning that names the
    // twist (decision 0175, issue #226).
    let faces = ExactFaces::helicoid(3.0, 200.0);
    let f = Fixture::with_surface(Box::new(faces), flat());
    let path = run(|ctx| {
        face_path(
            f.face(),
            &f.tool,
            f.rotary,
            &f.settings(),
            &RegionConstraint::default(),
            ctx,
        )
    })
    .unwrap();
    assert!(path.twist > 5.0, "{}", path.twist);
    assert!(path.gouge > TOL, "{path:?}");
    let made = self::generated(&f, &[]);
    let message = overcut_warning(&made);
    assert!(
        message.contains("cuts into the finished surface by"),
        "{message}"
    );
    assert!(message.contains("ruled but not developable"), "{message}");
    assert!(!message.contains("tilt"), "{message}");
}

#[test]
fn req_op_07_rulings_along_the_rotary_axis_are_refused() {
    let f = Fixture::brep(&coaxial_cylinder(12.0, 10.0, 40.0), flat());
    let message = refused(&f, &[]);
    assert!(
        message.contains("rulings run along the rotary axis"),
        "{message}"
    );
    assert!(message.contains("no tool taper helps"), "{message}");
}

#[test]
fn req_op_07_a_ruling_longer_than_the_flank_is_refused_and_so_is_a_tool_without_a_flank() {
    // The wall is 8 mm tall; a tool with 6 mm of flute cannot reach, and a
    // tip extension counts against the flank too.
    let model = boss_cylinder(BOSS_X, BOSS_RADIUS, Z0, Z1);
    let f = Fixture::brep(&model, tool(ToolShape::FlatEnd, 6.0));
    let message = refused(&f, &[]);
    assert!(message.contains("needs 8.000 mm of flank"), "{message}");
    assert!(message.contains("flank is 6.000 mm long"), "{message}");
    let f = Fixture::brep(&model, tool(ToolShape::FlatEnd, 9.0));
    assert!(generated(&f, &[]).group.segments.len() > 10);
    let message = refused(&f, &[(swarf::parameter::TIP_EXTENSION, num(1.5))]);
    assert!(message.contains("needs 9.500 mm of flank"), "{message}");
    let f = Fixture::brep(&model, tool(ToolShape::BallEnd, TOOL_RADIUS));
    assert!(refused(&f, &[]).contains("no straight flank"));
}

#[test]
fn req_op_07_the_tip_extension_tip_end_and_direction_parameters_move_the_tool() {
    let model = boss_cylinder(BOSS_X, BOSS_RADIUS, Z0, Z1);
    let f = Fixture::brep(&model, flat());
    let forward = generated(&f, &[]);
    let extended = generated(&f, &[(swarf::parameter::TIP_EXTENSION, num(1.5))]);
    let tips = |p: &openchuck_path::GeneratedPath| {
        cutting_poses(&p.group)
            .iter()
            .map(|q| q.tip)
            .collect::<Vec<_>>()
    };
    assert!(
        tips(&extended)
            .iter()
            .all(|t| (t.z - (Z0 - 1.5)).abs() < 1e-9)
    );
    // The other end: the tool comes from below, upside down in the model,
    // which A = 180 makes upright on the machine.
    let reversed = generated(&f, &[(swarf::parameter::TIP_END, text("reversed"))]);
    let poses = cutting_poses(&reversed.group);
    assert!(
        poses.iter().all(|p| (p.tip.z - Z1).abs() < 1e-9),
        "{:?}",
        poses[0]
    );
    assert!(
        poses.iter().all(|p| (p.a.0.abs() - 180.0).abs() < 1e-9),
        "{:?}",
        poses[0]
    );
    // The direction reverses the order round the boss.
    let back = generated(&f, &[(swarf::parameter::DIRECTION, text("reverse"))]);
    let mut t = tips(&back);
    t.reverse();
    assert_eq!(t, tips(&forward));
}

#[test]
fn req_op_11_req_op_07_stock_to_leave_offsets_the_flank_by_the_allowance() {
    let model = boss_cylinder(BOSS_X, BOSS_RADIUS, Z0, Z1);
    let f = Fixture::brep(&model, flat());
    let leave = 0.5;
    let path = generated(&f, &[(names::STOCK_TO_LEAVE, num(leave))]);
    let poses = cutting_poses(&path.group);
    for p in &poses {
        let r = (p.tip.x - BOSS_X).hypot(p.tip.y);
        assert!(
            (r - (BOSS_RADIUS + TOOL_RADIUS + leave)).abs() < 1e-9,
            "{p:?}"
        );
    }
    let (gouge, left) = verdict(
        &Outline::new(&flat()),
        &poses,
        &samples(&model, 180, 8, 0.05),
        leave,
    );
    assert!(gouge <= TOL, "gouge {gouge}");
    assert!(left <= TOL, "left {left}");
}

#[test]
fn req_op_07_req_op_12_material_left_is_silent_within_the_tolerance_noted_within_the_limit_and_refused_beyond_it()
 {
    // The levels of decision 0061 item 4, on a face path with a measured
    // leftover, and the default limit on the schema.
    let model = boss_cylinder(BOSS_X, BOSS_RADIUS, Z0, Z1);
    let f = Fixture::brep(&model, flat());
    let measured = run(|ctx| {
        face_path(
            f.face(),
            &f.tool,
            f.rotary,
            &f.settings(),
            &RegionConstraint::default(),
            ctx,
        )
    })
    .unwrap();
    assert!(measured.left <= TOL, "{}", measured.left);
    let at = Point3::new(1.0, 2.0, 3.0);
    let with = |left: f64| swarf::FacePath {
        left,
        left_at: at,
        ..measured.clone()
    };
    // Within the tolerance: nothing said.
    let silent = with(0.5 * TOL);
    assert!(silent.refusal(LIMIT).is_none());
    assert!(!silent.note(TOL, LIMIT).contains("leaves up to"));
    // Between the tolerance and the limit: a note, with the amount, the
    // place and the limit.
    let noted = with(0.05);
    assert!(noted.refusal(LIMIT).is_none());
    let note = noted.note(TOL, LIMIT);
    assert!(
        note.contains("leaves up to 0.0500 mm beyond the stock to leave at (1.000, 2.000, 3.000)"),
        "{note}"
    );
    assert!(
        note.contains("within the max_material_left of 0.1 mm"),
        "{note}"
    );
    // Beyond the limit: refused, naming the amount, the place, the limit
    // and what to change.
    let refused = with(0.2);
    let why = refused.refusal(LIMIT).unwrap();
    assert!(
        why.contains("would leave 0.2000 mm beyond the stock to leave at (1.000, 2.000, 3.000)"),
        "{why}"
    );
    assert!(
        why.contains("more than the max_material_left of 0.1 mm"),
        "{why}"
    );
    assert!(
        why.contains("tighter tolerance") && why.contains("larger max_material_left"),
        "{why}"
    );
    // A gouge is a warning, not a refusal (decision 0175), so a face that
    // also leaves too much is refused for what it leaves, and warned of the
    // gouge; a tilted face names the taper.
    let both = swarf::FacePath {
        gouge: 0.05,
        ..with(0.2)
    };
    assert!(both.refusal(LIMIT).unwrap().contains("would leave"));
    assert!(both.overcut_warning(TOL).is_some());
    assert!(with(0.2).overcut_warning(TOL).is_none());
    let tilted = swarf::FacePath {
        max_tilt: 6.0,
        suggested_taper: Some(6.0),
        ..with(0.2)
    };
    let why = tilted.refusal(LIMIT).unwrap();
    assert!(
        why.contains("flank half angle of 6.00 degrees would remove it"),
        "{why}"
    );
    assert!(!why.contains("tighter tolerance"), "{why}");
    // The schema carries the limit, zero for the automatic one.
    let schema = Swarf.schema();
    let spec = schema.get(swarf::parameter::MAX_MATERIAL_LEFT).unwrap();
    assert!(matches!(
        spec.kind,
        openchuck_path::ParameterKind::Number {
            default: Some(0.0),
            min: Some(0.0),
            ..
        }
    ));
    assert_eq!(swarf::DEFAULT_MAX_MATERIAL_LEFT, 0.1);
}

#[test]
fn req_op_07_req_op_12_the_default_material_left_limit_is_the_same_on_every_pass_and_a_set_value_wins()
 {
    // One limit, configured per operation (decision 0061 item 7, revised by
    // the owner): zero takes 0.1 mm whatever the stock to leave, and a set
    // value wins.
    let finishing = Settings {
        tolerance: TOL,
        stock_to_leave: 0.0,
        tip_extension: 0.0,
        max_material_left: 0.0,
        reverse_tip_end: false,
        reverse_direction: false,
        boundary_limits_pass: true,
        boundary_limits_tip: true,
    };
    let with_stock = Settings {
        stock_to_leave: 0.5,
        ..finishing
    };
    let semi = Settings {
        stock_to_leave: 0.05,
        ..finishing
    };
    let set = Settings {
        max_material_left: 0.02,
        ..with_stock
    };
    assert_eq!(finishing.material_left_limit(), 0.1);
    assert_eq!(with_stock.material_left_limit(), 0.1);
    assert_eq!(semi.material_left_limit(), 0.1);
    assert_eq!(set.material_left_limit(), 0.02);
    // The same leftover on a measured face path is judged the same way with
    // and without stock to leave: refused beyond 0.1 mm, noted within it.
    let model = boss_cylinder(BOSS_X, BOSS_RADIUS, Z0, Z1);
    let f = Fixture::brep(&model, flat());
    let measured = run(|ctx| {
        face_path(
            f.face(),
            &f.tool,
            f.rotary,
            &with_stock,
            &RegionConstraint::default(),
            ctx,
        )
    })
    .unwrap();
    let path = swarf::FacePath {
        left: 0.3,
        ..measured
    };
    for settings in [finishing, with_stock] {
        let why = path.refusal(settings.material_left_limit()).unwrap();
        assert!(why.contains("would leave 0.3000 mm"), "{why}");
        assert!(
            why.contains("more than the max_material_left of 0.1 mm"),
            "{why}"
        );
    }
    let noted = swarf::FacePath { left: 0.05, ..path };
    assert!(noted.refusal(with_stock.material_left_limit()).is_none());
    assert!(noted.refusal(set.material_left_limit()).is_some());
    // End to end, a pass with stock to leave is silent at the default, and
    // inside a pocket (where the chord leaves material) a set limit below
    // the chord refuses it, the set value winning over the default.
    let rough = generated(&f, &[(names::STOCK_TO_LEAVE, num(0.5))]);
    assert!(
        !rough
            .notes
            .iter()
            .any(|n| n.message.contains("leaves up to"))
    );
    let f = Fixture::brep(&pocket_cylinder(BOSS_X, BOSS_RADIUS, Z0, Z1), flat());
    assert_eq!(
        generated(&f, &[(names::STOCK_TO_LEAVE, num(0.5))])
            .notes
            .len(),
        1
    );
    let message = refused(
        &f,
        &[
            (names::STOCK_TO_LEAVE, num(0.5)),
            (swarf::parameter::MAX_MATERIAL_LEFT, num(1e-4)),
        ],
    );
    assert!(
        message.contains("more than the max_material_left of 0.0001 mm"),
        "{message}"
    );
}

#[test]
fn req_op_07_req_op_12_a_pocket_wall_is_cut_from_inside_and_a_limit_below_its_chord_refuses_it() {
    // Inside a round pocket the straight machine move between poses takes
    // the flank away from the wall, so the material left between poses is
    // the chord (within 0.4 of the tolerance, decision 0061 item 6), never a
    // gouge. With the default limit it is silent; a limit below the chord
    // refuses with the amount and the advice.
    let model = pocket_cylinder(BOSS_X, BOSS_RADIUS, Z0, Z1);
    let f = Fixture::brep(&model, flat());
    let path = run(|ctx| {
        face_path(
            f.face(),
            &f.tool,
            f.rotary,
            &f.settings(),
            &RegionConstraint::default(),
            ctx,
        )
    })
    .unwrap();
    assert!(path.gouge <= 1e-6, "{}", path.gouge);
    assert!(path.left > 1e-4 && path.left <= 0.4 * TOL, "{}", path.left);
    for p in path.poses() {
        let r = (p.tip.x - BOSS_X).hypot(p.tip.y);
        assert!((r - (BOSS_RADIUS - TOOL_RADIUS)).abs() < 1e-9, "{p:?}");
        assert!(p.a.0.abs() < 1e-9);
    }
    let generated = generated(&f, &[]);
    assert!(
        !generated
            .notes
            .iter()
            .any(|n| n.message.contains("leaves up to"))
    );
    let poses = cutting_poses(&generated.group);
    let (gouge, left) = verdict(
        &Outline::new(&flat()),
        &poses,
        &samples(&model, 240, 8, 0.05),
        0.0,
    );
    assert!(gouge <= TOL && left <= TOL, "gouge {gouge} left {left}");
    let message = refused(&f, &[(swarf::parameter::MAX_MATERIAL_LEFT, num(1e-4))]);
    assert!(message.contains("would leave"), "{message}");
    assert!(
        message.contains("more than the max_material_left of 0.0001 mm"),
        "{message}"
    );
    assert!(message.contains("tighter tolerance"), "{message}");
    let err = run(|ctx| {
        generate(
            &Swarf,
            &f.input(),
            &values(&[(swarf::parameter::MAX_MATERIAL_LEFT, num(-0.1))]),
            ctx,
        )
    })
    .unwrap_err();
    assert!(err.to_string().contains("max_material_left"), "{err}");
}

#[test]
fn req_op_12_req_op_07_the_independent_checker_finds_no_gouge_on_the_oracle_cases() {
    let cases: Vec<(BrepModel, RevolvedProfile)> = vec![
        (boss_cylinder(BOSS_X, BOSS_RADIUS, Z0, Z1), flat()),
        (
            boss_cylinder(BOSS_X, BOSS_RADIUS, Z0, Z1),
            tool(ToolShape::BallEnd, 15.0),
        ),
        (
            draft_cone(BOSS_X, 10.0, 6f64.to_radians(), 10.0, 16.0),
            tapered(6.0),
        ),
        (wall_plane(8.0, 22.0, 38.0, Z0, Z1), flat()),
    ];
    for (model, tool) in cases {
        let f = Fixture::brep(&model, tool.clone());
        let path = generated(&f, &[]);
        let poses = cutting_poses(&path.group);
        let (gouge, left) = verdict(
            &Outline::new(&tool),
            &poses,
            &samples(&model, 240, 8, 0.05),
            0.0,
        );
        assert!(
            gouge <= TOL,
            "gouge {gouge} on {:?}",
            model.faces[0].surface
        );
        assert!(left <= TOL, "left {left} on {:?}", model.faces[0].surface);
        // Nothing moves the tip into the wall: every cutting pose keeps the
        // whole exact surface clear of the tool body.
        for p in &poses {
            let worst = samples(&model, 120, 6, 0.0)
                .iter()
                .map(|q| checker_clearance(&Outline::new(&tool), p, *q))
                .fold(f64::INFINITY, f64::min);
            assert!(worst > -TOL, "pose {p:?} gouges by {}", -worst);
        }
    }
}

#[test]
fn req_op_06_req_op_07_the_tool_axis_is_the_machine_vertical_at_each_pose() {
    // First principles for the example machine (A positive turns the part
    // by the right-hand rule about +X): the machine's Z axis, seen in the
    // model, is (0, sin A, cos A). Every cutting pose on the helicoid, where
    // A runs through 200 degrees, must have that axis (OI-98).
    let faces = ExactFaces::helicoid(0.3, 200.0);
    let f = Fixture::with_surface(Box::new(faces), flat());
    let path = generated(&f, &[]);
    let poses = cutting_poses(&path.group);
    assert!(poses.len() > 20);
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for p in &poses {
        let a = p.a.0.to_radians();
        let expected = Vec3::new(0.0, a.sin(), a.cos());
        assert!((p.axis.get() - expected).length() < 1e-9, "{p:?}");
        lo = lo.min(p.a.0);
        hi = hi.max(p.a.0);
    }
    assert!(hi - lo > 150.0, "A turned from {lo} to {hi}");
}

#[test]
fn req_op_15_req_op_07_the_a_range_is_respected_along_a_helicoid() {
    let make = |range: ARange| {
        let faces = ExactFaces::helicoid(0.3, 200.0);
        let mut f = Fixture::with_surface(Box::new(faces), flat());
        f.rotary.range = range;
        f
    };
    // 200 degrees of travel do not fit a 180 degree range: refused, naming
    // the pass, by the shared A-axis management.
    let f = make(ARange::Limited {
        min: Degrees(-90.0),
        max: Degrees(90.0),
    });
    let message = refused(&f, &[]);
    assert!(
        message.contains("more than the machine's range"),
        "{message}"
    );
    // They fit a whole turn, and every A stays inside it.
    let f = make(ARange::Limited {
        min: Degrees(-180.0),
        max: Degrees(180.0),
    });
    let path = generated(&f, &[]);
    let a: Vec<f64> = path.group.segments.iter().map(|s| s.end.a.0).collect();
    assert!(
        a.iter().all(|x| (-180.0 - 1e-9..=180.0 + 1e-9).contains(x)),
        "{a:?}"
    );
    assert!(a.windows(2).all(|w| (w[1] - w[0]).abs() < 180.0), "no jump");
}

#[test]
fn req_op_16_swarf_is_in_the_registry_with_the_common_parameters() {
    let registry = Registry::builtin();
    let s = registry.get("swarf").unwrap();
    assert_eq!(s.label(), "Swarf (flank) machining");
    let schema = s.schema();
    for name in [
        names::TOLERANCE,
        names::STEPOVER,
        names::STOCK_TO_LEAVE,
        names::FEED,
        names::ENTRY,
        swarf::parameter::TIP_EXTENSION,
        swarf::parameter::MAX_MATERIAL_LEFT,
        swarf::parameter::TIP_END,
        swarf::parameter::DIRECTION,
        swarf::parameter::BOUNDARY_LIMITS_PASS,
        swarf::parameter::BOUNDARY_LIMITS_TIP,
    ] {
        assert!(schema.get(name).is_some(), "{name}");
    }
    // A boundary, avoid and check set limit swarf (decision 0176).
    assert!(s.honours_region());
    assert!(!s.honours_leave_zones());
}

#[test]
fn req_nfr_07_req_op_07_a_mesh_a_region_and_a_cancelled_job_are_refused_clearly() {
    // A mesh has no rulings (decision 0021, OI-96).
    let model = boss_cylinder(BOSS_X, BOSS_RADIUS, Z0, Z1);
    let mesh = MeshSurface::new(&model.faces[0].tessellation).unwrap();
    let f = Fixture::with_surface(Box::new(mesh), flat());
    let message = refused(&f, &[]);
    assert!(message.contains("a mesh has no rulings"), "{message}");
    // A selected region is not what swarf cuts: its faces are its selection
    // (decision 0176).
    let f = Fixture::brep(&model, flat());
    let selected = BrepSurface::new(&model).unwrap();
    let region = RegionConstraint {
        region: Some(&selected),
        ..RegionConstraint::default()
    };
    let err = run(|ctx| generate_in_region(&Swarf, &f.input(), &region, &values(&[]), ctx))
        .err()
        .unwrap();
    assert!(err.to_string().contains("not a region"), "{err}");
    // A negative tip extension is refused by the schema.
    let err = run(|ctx| {
        generate(
            &Swarf,
            &f.input(),
            &values(&[(swarf::parameter::TIP_EXTENSION, num(-1.0))]),
            ctx,
        )
    })
    .err()
    .unwrap();
    assert!(err.to_string().contains("tip_extension"), "{err}");
    // Cancellation is an error, not a path.
    let cancel = CancelToken::new();
    cancel.cancel();
    let ctx = JobContext::new(&cancel, &NoProgress);
    assert!(matches!(
        generate(&Swarf, &f.input(), &values(&[]), &ctx),
        Err(openchuck_path::PathError::Cancelled)
    ));
}

#[test]
fn req_nfr_06_req_op_07_swarf_is_identical_at_1_2_and_n_threads() {
    let model = draft_cone(BOSS_X, 10.0, 6f64.to_radians(), 10.0, 16.0);
    let f = Fixture::brep(&model, tapered(6.0));
    let runs = openchuck_testkit::determinism::check(|| {
        let path = generated(&f, &[]);
        format!("{:?}{:?}", path.group, path.notes).into_bytes()
    })
    .unwrap();
    assert_eq!(runs.len(), 4);
    let faces = ExactFaces::helicoid(0.3, 200.0);
    let f = Fixture::with_surface(Box::new(faces), flat());
    let runs = openchuck_testkit::determinism::check(|| {
        let path = generated(&f, &[]);
        format!("{:?}{:?}", path.group, path.notes).into_bytes()
    })
    .unwrap();
    assert_eq!(runs.len(), 4);
    // The leftover levels too: the pocket wall's note-free paths with and
    // without stock to leave and its refusal under a limit below the chord
    // are the same on every pool.
    let f = Fixture::brep(&pocket_cylinder(BOSS_X, BOSS_RADIUS, Z0, Z1), flat());
    let runs = openchuck_testkit::determinism::check(|| {
        let path = generated(&f, &[]);
        let rough = generated(&f, &[(names::STOCK_TO_LEAVE, num(0.5))]);
        let why = refused(&f, &[(swarf::parameter::MAX_MATERIAL_LEFT, num(1e-4))]);
        format!(
            "{:?}{:?}{:?}{:?}{why}",
            path.group, path.notes, rough.group, rough.notes
        )
        .into_bytes()
    })
    .unwrap();
    assert_eq!(runs.len(), 4);
}

#[test]
fn req_op_07_a_selection_of_several_faces_is_cut_face_by_face_in_order() {
    let mut model = boss_cylinder(BOSS_X, BOSS_RADIUS, Z0, Z1);
    let mut second = wall_plane(-14.0, 22.0, 38.0, Z0, Z1).faces.remove(0);
    second.id = openchuck_types::brep::FaceId(1);
    // The wall at y = -14 faces +Y, into the part: flip it so its normal
    // faces -Y, towards the tool.
    second.reversed = true;
    model.faces.push(second);
    let f = Fixture::brep(&model, flat());
    let path = generated(&f, &[]);
    assert_eq!(
        path.notes
            .iter()
            .filter(|n| n.message.starts_with("swarf on face"))
            .count(),
        2
    );
    let poses = cutting_poses(&path.group);
    let on_wall: Vec<_> = poses.iter().filter(|p| p.tip.y < -12.0).collect();
    assert!(!on_wall.is_empty());
    assert!(
        on_wall
            .iter()
            .all(|p| (p.tip.y - (-14.0 - TOOL_RADIUS)).abs() < 1e-9)
    );
    // The flipped wall's rulings are vertical, so it is cut at A = 0 with
    // the tool on its -Y side: the sign of the face decides the side the
    // tool is on.
    assert!(
        on_wall.iter().all(|p| p.a.0.abs() < 1e-9),
        "{:?}",
        on_wall[0]
    );
}

#[test]
fn req_op_07_req_nfr_12_an_overcut_above_the_tolerance_is_a_warning_that_says_how_to_accept_it() {
    // Decision 0175 (issue #226): a face the tool cuts into by more than the
    // tolerance is still cut, and the path carries one warning that names the
    // face, the depth and the place, says in plain words what the tolerance
    // is and where to raise it, and names the other choices. The path is the
    // one the gate measured: the same moves whether or not it warns.
    let faces = ExactFaces::helicoid(3.0, 200.0);
    let f = Fixture::with_surface(Box::new(faces), flat());
    let path = run(|ctx| {
        face_path(
            f.face(),
            &f.tool,
            f.rotary,
            &f.settings(),
            &RegionConstraint::default(),
            ctx,
        )
    })
    .unwrap();
    let made = generated(&f, &[]);
    let message = overcut_warning(&made);
    let depth = format!("{:.4} mm", path.gouge);
    let place = format!(
        "({:.3}, {:.3}, {:.3})",
        path.gouge_at.x, path.gouge_at.y, path.gouge_at.z
    );
    for words in [
        "Swarf on face 0 cuts into the finished surface by",
        depth.as_str(),
        place.as_str(),
        "more than this operation's tolerance of 0.01 mm",
        "Tolerance",
        "operation inspector",
        "smaller tool radius",
        "another strategy",
        "check the program",
    ] {
        assert!(message.contains(words), "{words:?} in {message}");
    }
    // The warning is also a note, so a host that shows only notes says it.
    assert!(
        made.notes.iter().any(|n| n.message == message),
        "{:?}",
        made.notes
    );
    // The default tolerance is not loosened (decision 0175 item 4).
    let schema = Swarf.schema();
    let spec = schema.get(names::TOLERANCE).unwrap();
    assert!(
        matches!(spec.kind, openchuck_path::ParameterKind::Number { default: Some(d), .. } if d == TOL),
        "{spec:?}"
    );
    // Raising the tolerance past the depth accepts it: no warning.
    let raised = path.gouge * 1.5;
    let made = generated(&f, &[(names::TOLERANCE, num(raised))]);
    assert!(made.warnings.is_empty(), "{:?}", made.warnings);
}

#[test]
fn req_op_16_req_op_07_strategies_say_in_their_registration_whether_they_need_a_step_part() {
    // Decision 0174 (issue #206): the host disables a strategy on a mesh
    // part from its registration, not from a fixed list. Swarf and radial
    // drilling read exact faces; the others work on a mesh.
    let registry = Registry::builtin();
    let needs: Vec<&str> = registry
        .iter()
        .filter(|s| s.needs_brep_part().is_some())
        .map(|s| s.name())
        .collect();
    assert_eq!(needs, ["radial_drilling", "swarf"]);
    for s in registry.iter() {
        if let Some(why) = s.needs_brep_part() {
            assert!(why.contains("STEP"), "{}: {why}", s.name());
        }
    }
}

// ---- limits: boundary, avoid and check (OP-02; decision 0176) -------------

/// The wall of the limit tests: `y = 8`, X from 22 to 38, Z from `Z0` to
/// `Z1`, facing +Y, so the flat tool stands at `y = 11` with its tip at
/// `Z0`.
fn limit_wall() -> BrepModel {
    wall_plane(8.0, 22.0, 38.0, Z0, Z1)
}

fn limited(
    f: &Fixture,
    region: &RegionConstraint<'_>,
    settings: &Settings,
) -> openchuck_path::Result<swarf::FacePath> {
    run(|ctx| face_path(f.face(), &f.tool, f.rotary, settings, region, ctx))
}

fn boundary(window: Window) -> RegionConstraint<'static> {
    RegionConstraint {
        boundary: Some(window),
        ..RegionConstraint::default()
    }
}

/// The angle about X of a point of the wall at height `z`.
fn wall_angle(z: f64) -> f64 {
    z.atan2(8.0)
}

#[test]
fn req_op_02_req_op_07_a_boundary_limits_the_swarf_pass_across_the_rulings() {
    // Limit A: only the rulings inside the boundary are cut, each whole, and
    // the pass stops where they leave it (decision 0176).
    let f = Fixture::brep(&limit_wall(), flat());
    let window = Window {
        x_min: 22.0,
        x_max: 30.0,
        arc: None,
    };
    let settings = Settings {
        boundary_limits_tip: false,
        ..f.settings()
    };
    let path = limited(&f, &boundary(window), &settings).unwrap();
    let poses: Vec<_> = path.poses().collect();
    assert!(!poses.is_empty());
    for p in &poses {
        assert!(p.contact.x <= 30.0 + 1e-3, "{p:?}");
        assert!(
            (p.ruling_length - (Z1 - Z0)).abs() < 1e-9,
            "cut whole: {p:?}"
        );
        assert!((p.tip.z - Z0).abs() < 1e-9, "the tip is not raised: {p:?}");
    }
    let reach = poses.iter().map(|p| p.contact.x).fold(f64::MIN, f64::max);
    assert!(
        reach > 30.0 - 1e-3,
        "the pass reaches the boundary: {reach}"
    );
    assert!(
        (path.limits.outside_boundary - 8.0).abs() < 1e-3,
        "{:?}",
        path.limits
    );
    let note = path.note(TOL, LIMIT);
    assert!(
        note.contains("The boundary limits the pass: about 8.000 mm"),
        "{note}"
    );
    // A boundary in the middle splits the pass into two runs; one between
    // them leaves two pieces, each its own run with a retract between.
    let middle = Window {
        x_min: 25.0,
        x_max: 35.0,
        arc: None,
    };
    let path = limited(&f, &boundary(middle), &settings).unwrap();
    assert_eq!(path.runs.len(), 1);
    assert!(
        path.poses()
            .all(|p| (25.0 - 1e-3..=35.0 + 1e-3).contains(&p.contact.x))
    );
}

#[test]
fn req_op_02_req_op_07_a_boundary_limits_the_cut_length_at_the_tool_tip() {
    // Limit B: the tip is raised along each ruling to where it enters the
    // boundary, so the bottom of the flank stops there (decision 0176).
    let f = Fixture::brep(&limit_wall(), flat());
    let window = Window {
        x_min: 0.0,
        x_max: 60.0,
        arc: Some((wall_angle(12.0), wall_angle(25.0) - wall_angle(12.0))),
    };
    let tip_only = Settings {
        boundary_limits_pass: false,
        ..f.settings()
    };
    for settings in [tip_only, f.settings()] {
        let path = limited(&f, &boundary(window), &settings).unwrap();
        for p in path.poses() {
            assert!((p.tip.z - 12.0).abs() < 1e-3, "{p:?}");
            assert!((p.ruling_length - (Z1 - 12.0)).abs() < 1e-3, "{p:?}");
        }
        assert!(
            (path.limits.tip_raised - 2.0).abs() < 1e-3,
            "{:?}",
            path.limits
        );
        let note = path.note(TOL, LIMIT);
        assert!(note.contains("raised by up to 2.000 mm"), "{note}");
        assert!(
            note.contains("far end of the rulings is not limited"),
            "{note}"
        );
        // The face below the boundary is not judged as material left: the
        // tool is raised off it on purpose.
        assert!(path.left <= TOL, "{path:?}");
    }
    // The tip limit adds to the tip extension.
    let extended = Settings {
        tip_extension: 0.5,
        ..f.settings()
    };
    let path = limited(&f, &boundary(window), &extended).unwrap();
    assert!(path.poses().all(|p| (p.tip.z - 11.5).abs() < 1e-3));
    // With the pass limited and the tip not, no ruling lies whole inside,
    // so nothing is left to cut, and generation says why.
    let pass_only = Settings {
        boundary_limits_tip: false,
        ..f.settings()
    };
    let err = limited(&f, &boundary(window), &pass_only).unwrap_err();
    assert!(
        err.to_string()
            .contains("none of its rulings lies inside the boundary set"),
        "{err}"
    );
}

#[test]
fn req_op_02_req_op_07_the_two_boundary_limits_compose_and_both_off_is_a_warning() {
    let f = Fixture::brep(&limit_wall(), flat());
    // The boundary leaves out the bottom 2 mm everywhere and the wall beyond
    // X = 30. With both limits the tip is raised and the pass stops at 30;
    // with the tip limit alone the whole length is cut, raised.
    let window = Window {
        x_min: 0.0,
        x_max: 30.0,
        arc: Some((wall_angle(12.0), wall_angle(25.0) - wall_angle(12.0))),
    };
    let both = limited(&f, &boundary(window), &f.settings()).unwrap();
    assert!(
        both.poses()
            .all(|p| p.contact.x <= 30.0 + 1e-3 && (p.tip.z - 12.0).abs() < 1e-3)
    );
    assert!(both.limits.outside_boundary > 7.9);
    let note = both.note(TOL, LIMIT);
    assert!(
        note.contains("limits the pass") && note.contains("raised by up to"),
        "{note}"
    );
    let tip_only = Settings {
        boundary_limits_pass: false,
        ..f.settings()
    };
    let path = limited(&f, &boundary(window), &tip_only).unwrap();
    // Beyond X = 30 no part of a ruling is inside, so those rulings are not
    // cut either: the tip cannot be raised into the boundary there.
    assert!(path.poses().all(|p| p.contact.x <= 30.0 + 1e-3));
    let note = path.note(TOL, LIMIT);
    assert!(
        note.contains("no part of its rulings lies inside the boundary"),
        "{note}"
    );
    // With the tip at the top of the wall (Tip end reversed), limit B
    // lowers the tip from the top to where the rulings enter the boundary.
    let below = Window {
        x_min: 0.0,
        x_max: 60.0,
        arc: Some((wall_angle(5.0), wall_angle(16.0) - wall_angle(5.0))),
    };
    let reversed = Settings {
        reverse_tip_end: true,
        ..f.settings()
    };
    let path = limited(&f, &boundary(below), &reversed).unwrap();
    for p in path.poses() {
        assert!((p.ruling_length - (16.0 - Z0)).abs() < 1e-3, "{p:?}");
        assert!((p.contact.z - 0.5 * (16.0 + Z0)).abs() < 1e-3, "{p:?}");
    }
    assert!((path.limits.tip_raised - (Z1 - 16.0)).abs() < 1e-3);
    // Both off: the boundary limits nothing, the path is the one without a
    // boundary, and a warning points at the setting.
    let off = [
        (
            swarf::parameter::BOUNDARY_LIMITS_PASS,
            ParameterValue::Flag(false),
        ),
        (
            swarf::parameter::BOUNDARY_LIMITS_TIP,
            ParameterValue::Flag(false),
        ),
    ];
    let region = boundary(window);
    let with =
        run(|ctx| generate_in_region(&Swarf, &f.input(), &region, &values(&off), ctx)).unwrap();
    let without = generated(&f, &off);
    assert_eq!(with.group, without.group);
    assert_eq!(with.warnings.len(), 1, "{:?}", with.warnings);
    assert_eq!(
        with.warnings[0].parameter.as_deref(),
        Some(swarf::parameter::BOUNDARY_LIMITS_PASS)
    );
    assert!(with.warnings[0].message.contains("limits nothing"));
    assert!(without.warnings.is_empty());
}

#[test]
fn req_op_02_req_op_07_req_nfr_12_swarf_stops_short_of_an_avoid_surface_and_further_of_a_check_surface()
 {
    // A wall across the end of the cut wall, at X = 38: the flat tool, of
    // radius 3, cannot pass X = 35 without cutting into it (decision 0176).
    let f = Fixture::brep(&limit_wall(), flat());
    let end = BrepSurface::new(&cross_plane(38.0, 8.0, 20.0, Z0, Z1)).unwrap();
    let avoid = RegionConstraint {
        avoid: Some(&end),
        ..RegionConstraint::default()
    };
    let path = limited(&f, &avoid, &f.settings()).unwrap();
    let reach = path.poses().map(|p| p.contact.x).fold(f64::MIN, f64::max);
    assert!(reach <= 35.0 + TOL, "{reach}");
    assert!(
        reach > 35.0 - 0.01,
        "the pass stops close to the surface: {reach}"
    );
    assert!(
        (path.limits.avoid_uncut - (38.0 - reach)).abs() < 1e-3,
        "{:?}",
        path.limits
    );
    assert!(path.note(TOL, LIMIT).contains("cut into an avoid surface"));
    // A thin rib across the middle of the wall, at X = 30: the pass stops
    // short of it and starts again beyond it, as two runs.
    let rib = BrepSurface::new(&cross_plane(30.0, 8.0, 20.0, Z0, Z1)).unwrap();
    let middle = RegionConstraint {
        avoid: Some(&rib),
        ..RegionConstraint::default()
    };
    let split = limited(&f, &middle, &f.settings()).unwrap();
    assert_eq!(split.runs.len(), 2, "{:?}", split.limits);
    assert!(split.runs[0].iter().all(|p| p.contact.x <= 27.0 + TOL));
    assert!(split.runs[1].iter().all(|p| p.contact.x >= 33.0 - TOL));
    assert!(!split.closed);
    // A check surface keeps the stock to leave clear too: with 0.5 mm left
    // the tool stops 0.5 mm earlier than for an avoid surface.
    let leave = Settings {
        stock_to_leave: 0.5,
        ..f.settings()
    };
    let check = RegionConstraint {
        check: Some(&end),
        ..RegionConstraint::default()
    };
    let as_avoid = limited(&f, &avoid, &leave).unwrap();
    let as_check = limited(&f, &check, &leave).unwrap();
    let last = |p: &swarf::FacePath| p.poses().map(|p| p.contact.x).fold(f64::MIN, f64::max);
    assert!(
        (last(&as_avoid) - last(&as_check) - 0.5).abs() < 0.02,
        "{} {}",
        last(&as_avoid),
        last(&as_check)
    );
    assert!(as_check.limits.check_uncut > 0.0 && as_check.limits.avoid_uncut == 0.0);
    assert!(as_check.note(TOL, LIMIT).contains("touch a check surface"));
    // The independent checker: no pose of the generated path comes into the
    // end wall.
    let region = avoid;
    let generated =
        run(|ctx| generate_in_region(&Swarf, &f.input(), &region, &values(&[]), ctx)).unwrap();
    let outline = Outline::new(&flat());
    let poses = motion(&cutting_poses(&generated.group), 8);
    let end_points = samples(&cross_plane(38.0, 8.0, 20.0, Z0, Z1), 24, 24, 0.0);
    let (gouge, _) = verdict(&outline, &poses, &end_points, 0.0);
    assert!(gouge <= TOL, "the end wall is cut into by {gouge}");
}

#[test]
fn req_op_02_req_op_07_a_floor_the_flank_only_meets_is_kept_and_a_tip_extension_into_it_is_refused()
{
    // The floor at the foot of the wall, as an avoid surface: a flat tool
    // with its tip at the foot only meets it, so nothing is dropped; a tip
    // extension cuts into it everywhere, so nothing is left, and the face is
    // refused naming why (decision 0176).
    let f = Fixture::brep(&limit_wall(), flat());
    let floor = BrepSurface::new(&floor_plane(Z0, 15.0, 45.0, 8.0, 30.0)).unwrap();
    let avoid = RegionConstraint {
        avoid: Some(&floor),
        ..RegionConstraint::default()
    };
    let path = limited(&f, &avoid, &f.settings()).unwrap();
    assert_eq!(path.limits.avoid_uncut, 0.0, "{:?}", path.limits);
    let plain = limited(&f, &RegionConstraint::default(), &f.settings()).unwrap();
    assert_eq!(path.runs, plain.runs);
    let extended = Settings {
        tip_extension: 0.5,
        ..f.settings()
    };
    let err = limited(&f, &avoid, &extended).unwrap_err().to_string();
    assert!(err.contains("swarf cannot cut face 0"), "{err}");
    assert!(
        err.contains("cut into the avoid surfaces wherever it cuts it"),
        "{err}"
    );
}
