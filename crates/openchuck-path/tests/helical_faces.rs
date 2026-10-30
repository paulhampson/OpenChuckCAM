// SPDX-License-Identifier: AGPL-3.0-or-later
//! Faces across the rotary axis under the helix strategies (issue 384;
//! decision record 0297; OP-03, OP-04, OP-05, OP-12, GC-07, NFR-06;
//! verification plan 4.1).
//!
//! A helix advances along the axis by its pitch every turn, so before
//! decision 0297 a face perpendicular to the axis kept a step of material up
//! to one pitch thick. The checks here measure what is left on such faces
//! independently: points on the face, and their distance to the nearest
//! tool surface over every fed pose (the tool's outline from
//! `openchuck-testkit`, none of the strategy's code).
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::print_stderr
)]

mod support;

use std::collections::BTreeMap;
use std::path::PathBuf;

use openchuck_path::params::names;
use openchuck_path::rotary::AxisPosition;
use openchuck_path::{
    AcceptAll, AnalyticStock, CompositeSurface, GeneratedPath, HelicalFinishing, MeshSurface,
    ParameterValue, RestMachining, RotaryMapping, RotaryRoughing, SimultaneousFinishing, Solid,
    Strategy, StrategyInput, SurfaceSource, generate,
};
use openchuck_testkit::gouge::{Outline, Pose};
use openchuck_testkit::parts::{Cone, RadialPart, revolve};
use openchuck_types::geometry::{Point3, Vec3};
use openchuck_types::ir::MoveKind;
use openchuck_types::machine::Clearance;
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::tool::{RevolvedProfile, ToolShape};
use openchuck_types::units::{Degrees, Millimetres};
use support::{machine, read_binary_stl, run, tags, tool};

const TOLERANCE: f64 = 0.01;
/// The owner's stepover for the 3.175 mm tool (cutting data of the CrankShaft
/// project), so the step left before the fix was up to 2.22 mm.
const STEPOVER: f64 = 2.2225;
const DIAMETER: f64 = 3.175;

/// A stepped shaft: radius 6 to X 10, 10 to X 25, 7 to X 40. Its faces across
/// the axis are the annuli at X 10 (r 6 to 10, facing -X) and X 25 (r 7 to 10,
/// facing +X).
const SHAFT: [(f64, f64); 6] = [
    (0.0, 6.0),
    (10.0, 6.0),
    (10.0, 10.0),
    (25.0, 10.0),
    (25.0, 7.0),
    (40.0, 7.0),
];

/// Points on the shaft's two faces, `step` mm apart in radius and around.
fn shaft_faces(step: f64) -> Vec<Vec<Point3>> {
    [(10.0, 6.0, 10.0), (25.0, 7.0, 10.0)]
        .into_iter()
        .map(|(x, lo, hi)| {
            let nr = ((hi - lo) / step).ceil() as u32;
            let mut out = Vec::new();
            for i in 0..=nr {
                let r = lo + (hi - lo) * f64::from(i) / f64::from(nr);
                let na = (std::f64::consts::TAU * r / step).ceil() as u32;
                for j in 0..na {
                    let t = std::f64::consts::TAU * f64::from(j) / f64::from(na);
                    out.push(Point3::new(x, r * t.cos(), r * t.sin()));
                }
            }
            out
        })
        .collect()
}

/// The owner's CrankShaft as placed in its project (centred on the rotary
/// axis, X 0 to 40), tessellated by the STEP import.
fn crankshaft() -> TriangleMesh {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/stl/crankshaft.stl");
    read_binary_stl(&path)
}

/// Points on the crankshaft's faces across the axis inside its length (its
/// end faces carry the tabs and are Facing's to cut, decision record 0252),
/// a grid of `step` mm on every triangle whose normal is along X, by face X,
/// each with the sign of its outward normal along X.
fn mesh_faces(mesh: &TriangleMesh, step: f64) -> BTreeMap<i64, Vec<(Point3, f64)>> {
    let mut out: BTreeMap<i64, Vec<(Point3, f64)>> = BTreeMap::new();
    for t in &mesh.triangles {
        let [a, b, c] = t.map(|i| mesh.vertices[i as usize]);
        let n = (b - a).cross(c - a);
        if n.length() < 1e-12 || (n.x / n.length()).abs() < 0.999 || a.x < 0.5 || a.x > 39.5 {
            continue;
        }
        let e = (b - a).length().max((c - a).length()).max((c - b).length());
        let k = (e / step).ceil().max(1.0) as u32;
        for i in 0..=k {
            for j in 0..=(k - i) {
                let (u, v) = (f64::from(i) / f64::from(k), f64::from(j) / f64::from(k));
                out.entry((a.x * 100.0).round() as i64)
                    .or_default()
                    .push((a + (b - a) * u + (c - a) * v, n.x.signum()));
            }
        }
    }
    out
}

/// The tool at the end of every cutting move and at `k - 1` points between,
/// or more where the move turns A by more than a degree a point (a tool with
/// its tip on the rotary axis turns in place), on the straight machine move.
/// The tool is vertical on the machine, so its axis in the part follows A.
fn poses_along(path: &GeneratedPath, rotary: &RotaryMapping, k: u32) -> Vec<Pose> {
    let mut out = Vec::new();
    let mut from = path.group.start;
    for s in &path.group.segments {
        if s.kind == MoveKind::Cutting {
            let a = rotary.to_axis(from.tip, from.a);
            let b = rotary.to_axis(s.end.tip, s.end.a);
            let k = k.max((s.end.a.0 - from.a.0).abs().ceil() as u32);
            for i in 1..=k {
                let t = f64::from(i) / f64::from(k);
                let lerp = |p: f64, q: f64| p + t * (q - p);
                let angle = Degrees(lerp(from.a.0, s.end.a.0));
                let tip = rotary.to_model(
                    AxisPosition {
                        along: lerp(a.along, b.along),
                        across: lerp(a.across, b.across),
                        height: lerp(a.height, b.height),
                    },
                    angle,
                );
                out.push(Pose {
                    tip,
                    axis: rotary.tool_axis(angle).get(),
                });
            }
        }
        from = s.end;
    }
    out
}

/// Poses binned by X and by the polar angle of the tool axis, for
/// [`left_on`].
struct Bins {
    poses: Vec<Pose>,
    bins: BTreeMap<(i64, i64), Vec<usize>>,
}

const BIN_X: f64 = 0.5;
const BIN_DEGREES: f64 = 2.0;
const BINS_AROUND: i64 = 180;

impl Bins {
    fn new(poses: Vec<Pose>) -> Self {
        let mut bins: BTreeMap<(i64, i64), Vec<usize>> = BTreeMap::new();
        for (k, q) in poses.iter().enumerate() {
            // By the direction of the tool axis, not the tip: a tool can
            // reach past the rotary axis, with its tip on the far side.
            let angle = q.axis.z.atan2(q.axis.y).to_degrees().rem_euclid(360.0);
            bins.entry((
                (q.tip.x / BIN_X).floor() as i64,
                (angle / BIN_DEGREES).floor() as i64,
            ))
            .or_default()
            .push(k);
        }
        Self { poses, bins }
    }
}

/// For each point, its signed distance to the nearest tool surface over the
/// poses: positive where material is left, negative where a tool cut into
/// it. Capped at `cap`. Only poses whose tip is within reach by X and, for a
/// point well off the axis, whose tool axis points near the point's polar
/// angle are looked at (the tool's axis passes near the rotary axis, so it
/// reaches a point only from near the point's angle; the window is half as
/// wide again as the reach, for a tilted or offset tool).
fn left_on(outline: &Outline, bins: &Bins, points: &[Point3], cap: f64) -> Vec<f64> {
    let reach = outline.radius() + cap;
    points
        .iter()
        .map(|p| {
            let r = p.y.hypot(p.z);
            let theta = p.z.atan2(p.y).to_degrees().rem_euclid(360.0);
            let half = if r <= 1.5 * reach {
                180.0
            } else {
                (1.5 * reach / r).asin().to_degrees() + BIN_DEGREES
            };
            let (x0, x1) = (
                ((p.x - reach) / BIN_X).floor() as i64,
                ((p.x + reach) / BIN_X).floor() as i64,
            );
            let a0 = ((theta - half) / BIN_DEGREES).floor() as i64;
            let a1 = (((theta + half) / BIN_DEGREES).floor() as i64).min(a0 + BINS_AROUND - 1);
            let mut best = cap;
            for xb in x0..=x1 {
                for ab in a0..=a1 {
                    let key = (xb, ab.rem_euclid(BINS_AROUND));
                    for k in bins.bins.get(&key).into_iter().flatten() {
                        let pose = &bins.poses[*k];
                        let d = *p - pose.tip;
                        let z = d.dot(pose.axis);
                        let radial = (d - pose.axis * z).length();
                        if radial - outline.radius() > best {
                            continue;
                        }
                        best = best.min(outline.clearance(radial, z));
                    }
                }
            }
            best
        })
        .collect()
}

fn input_for<'a>(
    surface: &'a dyn SurfaceSource,
    stock: &'a AnalyticStock,
    profile: &'a RevolvedProfile,
    tag: &'a openchuck_types::ir::PathTags,
) -> StrategyInput<'a> {
    let machine = machine();
    StrategyInput {
        surface,
        stock,
        collision: &AcceptAll,
        tool: profile,
        rotary: RotaryMapping::of_profile(&machine),
        clearance: Clearance {
            radius: Millimetres(25.0),
            safe_z: Millimetres(30.0),
        },
        max_feeds: machine.max_feeds,
        tags: tag,
        entry_safety: None,
        facing: None,
        keep_out: &[],
    }
}

struct Job {
    surface: Box<dyn SurfaceSource>,
    stock: AnalyticStock,
    profile: RevolvedProfile,
}

impl Job {
    fn shaft(shape: ToolShape) -> Self {
        Self {
            surface: Box::new(MeshSurface::new(&revolve(&SHAFT, 720)).unwrap()),
            stock: AnalyticStock::Cylinder {
                radius: 11.0,
                x_start: -2.0,
                x_end: 42.0,
            },
            profile: tool(shape, DIAMETER).profile().unwrap(),
        }
    }

    fn crankshaft() -> Self {
        Self {
            surface: Box::new(MeshSurface::new(&crankshaft()).unwrap()),
            stock: AnalyticStock::Cylinder {
                radius: 18.0,
                x_start: -2.0,
                x_end: 42.0,
            },
            profile: tool(ToolShape::FlatEnd, DIAMETER).profile().unwrap(),
        }
    }

    fn path(&self, strategy: &dyn Strategy, extra: &[(&str, f64)]) -> GeneratedPath {
        let tag = tags();
        let input = input_for(self.surface.as_ref(), &self.stock, &self.profile, &tag);
        let mut values: BTreeMap<String, ParameterValue> = [
            (names::STEPOVER, STEPOVER),
            (names::FEED, 600.0),
            (names::PLUNGE_FEED, 150.0),
            (names::TOLERANCE, TOLERANCE),
        ]
        .into_iter()
        .chain(extra.iter().copied())
        .map(|(k, x)| (k.to_owned(), ParameterValue::Number(x)))
        .collect();
        if strategy.name() == "rotary_roughing" {
            values
                .entry("stepdown".to_owned())
                .or_insert(ParameterValue::Number(1.0));
        }
        run(|ctx| generate(strategy, &input, &values, ctx)).unwrap()
    }

    /// What `path` leaves on each group of points, capped at `cap` mm.
    fn left(&self, path: &GeneratedPath, faces: &[Vec<Point3>], cap: f64) -> Vec<Vec<f64>> {
        let rotary = RotaryMapping::of_profile(&machine());
        let bins = Bins::new(poses_along(path, &rotary, 8));
        let outline = Outline::new(&self.profile);
        faces
            .iter()
            .map(|pts| left_on(&outline, &bins, pts, cap))
            .collect()
    }
}

fn worst(v: &[f64]) -> f64 {
    v.iter().copied().fold(f64::NEG_INFINITY, f64::max)
}

fn deepest(v: &[f64]) -> f64 {
    v.iter().copied().fold(f64::INFINITY, f64::min)
}

/// Checks the shaft's faces: left at `leave` within the tolerance, never cut
/// into, beyond `corner` of the inside corner of each face (inner radii 6
/// and 7). A sharp-cornered tool kept `leave` off both the floor and the
/// face is `leave` times root 2 from the corner itself, so with stock to
/// leave the band within `leave` of the corner is left out.
fn shaft_faces_left_at(job: &Job, path: &GeneratedPath, leave: f64, corner: f64, label: &str) {
    let faces = shaft_faces(0.1);
    for ((face, left), inner) in faces
        .iter()
        .zip(job.left(path, &faces, 3.0))
        .zip([6.0, 7.0])
    {
        let beyond: Vec<f64> = face
            .iter()
            .zip(&left)
            .filter(|(p, _)| p.y.hypot(p.z) >= inner + corner)
            .map(|(_, l)| *l)
            .collect();
        assert!(!beyond.is_empty());
        let (w, d) = (worst(&beyond), deepest(&left));
        assert!(
            w <= leave + TOLERANCE,
            "{label}, face at r {inner}: {w:.4} mm left"
        );
        assert!(
            d >= leave - TOLERANCE,
            "{label}, face at r {inner}: cut to {d:.4}"
        );
    }
}

#[test]
fn req_op_04_helical_finishing_cleans_the_faces_across_the_axis() {
    // The side of a flat end mill reaches the whole face, so it is left at
    // the stock to leave, within the tolerance (verification plan 4.1).
    // Before decision 0297 the worst was 2.07 mm.
    let job = Job::shaft(ToolShape::FlatEnd);
    for leave in [0.0, 0.3] {
        let path = job.path(&HelicalFinishing, &[(names::STOCK_TO_LEAVE, leave)]);
        let corner = if leave > 0.0 { leave + 0.05 } else { 0.0 };
        shaft_faces_left_at(&job, &path, leave, corner, &format!("leave {leave}"));
    }
}

#[test]
fn req_op_04_a_ball_end_leaves_only_its_corner_on_a_face() {
    // The ball cannot reach into the corner: within its radius of the inside
    // corner the face keeps a fillet by design. Beyond that its straight side
    // reaches the face. Before decision 0297 the worst was 2.38 mm.
    let job = Job::shaft(ToolShape::BallEnd);
    let path = job.path(&HelicalFinishing, &[]);
    let faces = shaft_faces(0.1);
    let cut = job.left(&path, &faces, 3.0);
    assert!(cut.iter().all(|l| deepest(l) >= -TOLERANCE));
    shaft_faces_left_at(&job, &path, 0.0, 0.5 * DIAMETER + 0.05, "ball end");
}

#[test]
fn req_op_03_continuous_roughing_leaves_the_stock_to_leave_on_the_faces() {
    // Before decision 0297: up to 2.35 mm where 0.3 was asked for.
    let job = Job::shaft(ToolShape::FlatEnd);
    let leave = 0.3;
    let path = job.path(&RotaryRoughing, &[(names::STOCK_TO_LEAVE, leave)]);
    shaft_faces_left_at(&job, &path, leave, leave + 0.05, "roughing");
}

/// The nearest distance along the ray from `origin` along `d` (unit) at which
/// it meets one of `triangles`, beyond a small distance (Moller-Trumbore).
fn ray_hit(triangles: &[[Point3; 3]], origin: Point3, d: Vec3) -> Option<f64> {
    triangles
        .iter()
        .filter_map(|&[a, b, c]| {
            let (e1, e2) = (b - a, c - a);
            let h = d.cross(e2);
            let det = e1.dot(h);
            if det.abs() < 1e-12 {
                return None;
            }
            let s = origin - a;
            let u = s.dot(h) / det;
            if !(0.0..=1.0).contains(&u) {
                return None;
            }
            let q = s.cross(e1);
            let v = d.dot(q) / det;
            if v < 0.0 || u + v > 1.0 {
                return None;
            }
            let t = e2.dot(q) / det;
            (t > 1e-6).then_some(t)
        })
        .reduce(f64::min)
}

/// How a vertical tool's side can meet a face point in 4-axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reach {
    /// The tool's body beside the point is clear of the part, and some part
    /// lies on the tool's line for a drop to stand on.
    Reached,
    /// The part covers the tool's body beside the point: no vertical tool of
    /// this size touches it.
    Blocked,
    /// The point is in the inside corner where the face meets the surface
    /// under the tool: the part under the tool's section, widened by the
    /// stock to leave, comes within the stock to leave (and the margin)
    /// below the point. A tool kept the stock
    /// to leave off both surfaces is up to root 2 times that from the corner,
    /// by design.
    Corner,
    /// The tool's body is clear, but nothing lies on its whole line: a drop
    /// has nothing to stand on, so no strategy that drops the tool places
    /// it there (the pass is split there, as it always was).
    NothingUnder,
}

/// How a vertical tool of radius `radius` meets the face point `p` (outward
/// normal `sign` along X) with its side, `leave` off the face, in 4-axis.
/// The only tool line whose side meets the face at `p` is the one along the
/// radius through `p`; its body from `p`'s height outwards must be clear of
/// the part, and for a drop to place it some part must lie on its line
/// beyond. Checked by rays from a grid over the tool's section at `p`'s
/// height, outwards and inwards along its axis; independent of the
/// strategies and of the drop-cutter. The tool is taken 0.1 mm larger for
/// the body and 0.1 mm smaller for what it stands on, so a point counts as
/// reached only where it surely is.
fn reach(triangles: &[[Point3; 3]], p: Point3, sign: f64, tool: f64, leave: f64) -> Reach {
    // A margin on the tool's radius either way, so only points the tool
    // surely reaches, or surely does not, are told apart.
    let margin = 0.1;
    let radius = tool + margin;
    let r = p.y.hypot(p.z);
    if r < 1e-9 {
        return Reach::Blocked;
    }
    let d = Vec3::new(0.0, p.y / r, p.z / r);
    let across = Vec3::new(0.0, -d.z, d.y);
    let centre = p.x + sign * (radius + leave);
    let (lo, hi) = (
        p.x.min(p.x + sign * 2.0 * (radius + leave)) + 1e-3,
        p.x.max(p.x + sign * 2.0 * (radius + leave)) - 1e-3,
    );
    // Only the triangles the tool's section can meet along its axis.
    let span = (lo - leave - margin, hi + leave + margin);
    let triangles: Vec<[Point3; 3]> = triangles
        .iter()
        .filter(|t| {
            let (a, b) = t
                .iter()
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| {
                    (a.min(v.x), b.max(v.x))
                });
            a <= span.1 && b >= span.0
        })
        .copied()
        .collect();
    let triangles = &triangles[..];
    let n = 4;
    let mut under = false;
    let mut floor = f64::INFINITY;
    for i in -n..=n {
        for j in -n..=n {
            let (u, v) = (f64::from(i) / f64::from(n), f64::from(j) / f64::from(n));
            if u * u + v * v > 1.0 {
                continue;
            }
            // Short of the face by a little, so the face itself is not hit.
            let x = (centre + radius * u).clamp(lo, hi);
            let origin = Point3::new(x, p.y, p.z) + across * (radius * v);
            if ray_hit(triangles, origin, d).is_some() {
                return Reach::Blocked;
            }
            // The tool keeps the stock to leave off every surface, so
            // what holds it up may lie that far beyond its side.
            let wide = Point3::new(centre + (radius + leave) * u, p.y, p.z)
                + across * ((radius + leave) * v);
            if (lo - 1e-3..=hi + 1e-3).contains(&wide.x) || leave > 0.0 {
                floor = floor.min(ray_hit(triangles, wide, d * -1.0).unwrap_or(f64::INFINITY));
            }
            // Something to stand on under the tool, shrunk by `margin` so
            // that the tool itself surely meets it.
            let inner = Point3::new(
                (centre + (radius - 2.0 * margin) * u).clamp(lo, hi),
                p.y,
                p.z,
            ) + across * ((radius - 2.0 * margin) * v);
            under = under || ray_hit(triangles, inner, d * -1.0).is_some();
        }
    }
    if leave > 0.0 && floor <= leave + margin {
        Reach::Corner
    } else if under {
        Reach::Reached
    } else {
        Reach::NothingUnder
    }
}

/// The owner's CrankShaft with each helix strategy: every point of its faces
/// across the axis that a vertical tool's side surely reaches in 4-axis
/// ([`reach`]) is left at the
/// stock to leave; the rest (where the plate of the crank covers the face
/// near the axis, or the tool cannot fit beside it) cannot be reached by any
/// helix with this tool and is only counted. `extra` sets the operation's
/// parameters, its stock to leave among them.
fn crankshaft_faces_cleaned(strategy: &dyn Strategy, extra: &[(&str, f64)]) {
    let leave = extra
        .iter()
        .find(|(k, _)| *k == names::STOCK_TO_LEAVE)
        .map_or(0.0, |(_, v)| *v);
    let mesh = crankshaft();
    let job = Job::crankshaft();
    let faces = mesh_faces(&mesh, 0.4);
    assert_eq!(
        faces.keys().copied().collect::<Vec<_>>(),
        vec![1000, 1500, 2500, 3000],
        "the crankshaft's faces across the axis"
    );
    let radius = 0.5 * DIAMETER;
    let path = job.path(strategy, extra);
    // How close to the rotary axis the tool's tip comes anywhere: the lowest
    // a cut goes, on the path itself.
    let lowest = path
        .group
        .segments
        .iter()
        .filter(|s| s.kind == MoveKind::Cutting)
        .map(|s| s.end.tip.to_vec().dot(s.end.axis.get()))
        .fold(f64::INFINITY, f64::min);
    eprintln!(
        "{}: the lowest cut is {lowest:.3} mm from the axis",
        strategy.name()
    );
    let points: Vec<Vec<Point3>> = faces
        .values()
        .map(|face| face.iter().map(|(p, _)| *p).collect())
        .collect();
    let lefts = job.left(&path, &points, 1.0);
    for ((key, face), left) in faces.into_iter().zip(lefts) {
        let x = key as f64 / 100.0;
        let near: Vec<[Point3; 3]> = mesh
            .triangles
            .iter()
            .map(|t| t.map(|i| mesh.vertices[i as usize]))
            .filter(|t| {
                let lo = t.iter().map(|v| v.x).fold(f64::INFINITY, f64::min);
                let hi = t.iter().map(|v| v.x).fold(f64::NEG_INFINITY, f64::max);
                lo < x + 2.0 * DIAMETER && hi > x - 2.0 * DIAMETER
            })
            .collect();
        let (mut blocked, mut nothing_under, mut corner, mut below) = (0, 0, 0, 0);
        let (mut worst_reached, mut deepest_cut) = (f64::NEG_INFINITY, f64::INFINITY);
        for ((p, sign), l) in face.iter().zip(&left) {
            deepest_cut = deepest_cut.min(*l);
            // Only the points left over the limit need the reach check.
            if *l <= leave + TOLERANCE {
                worst_reached = worst_reached.max(*l);
                continue;
            }
            // Roughing cuts no lower than its lowest layer: a point nearer
            // the axis is left to finishing (decision record 0033).
            if p.y.hypot(p.z) < lowest {
                below += 1;
                continue;
            }
            match reach(&near, *p, *sign, radius, leave) {
                Reach::Reached => worst_reached = worst_reached.max(*l),
                Reach::Blocked => blocked += 1,
                Reach::Corner => corner += 1,
                Reach::NothingUnder => nothing_under += 1,
            }
        }
        eprintln!(
            "{} face X {x}: {} points, {blocked} blocked, {nothing_under} with nothing under \
             the tool, {corner} in the corner, {below} below the lowest cut, worst where reached {worst_reached:.4}, deepest {deepest_cut:.4}",
            strategy.name(),
            face.len()
        );
        assert!(
            worst_reached <= leave + TOLERANCE,
            "{} face X {x}: {worst_reached:.4} mm left where the tool reaches",
            strategy.name()
        );
        assert!(
            deepest_cut >= leave - TOLERANCE,
            "{} face X {x}: cut to {deepest_cut:.4}",
            strategy.name()
        );
        let out_of_reach = blocked + nothing_under;
        // The part the tool cannot reach is small: the covered corners near
        // the axis (about 9 % of each face here, at radii below 7.5 mm).
        assert!(
            out_of_reach * 6 < face.len(),
            "{} face X {x}: {out_of_reach} of {} points out of reach",
            strategy.name(),
            face.len()
        );
    }
}

#[test]
fn req_op_04_helical_finishing_cleans_the_crankshaft_faces() {
    crankshaft_faces_cleaned(&HelicalFinishing, &[]);
}

#[test]
fn req_op_03_continuous_roughing_cleans_the_crankshaft_faces() {
    // The owner's stock to leave; layers of 3 mm rather than the owner's 1 mm
    // keep the test short, and the turn is cut in every layer either way.
    crankshaft_faces_cleaned(
        &RotaryRoughing,
        &[(names::STOCK_TO_LEAVE, 0.3), ("stepdown", 3.0)],
    );
}

#[test]
fn req_op_05_the_simultaneous_spiral_cleans_the_crankshaft_faces() {
    crankshaft_faces_cleaned(&SimultaneousFinishing, &[]);
}

/// Runs of cutting moves that turn A by at least a whole turn without moving
/// along the axis: the turns at the faces.
fn turns_in_place(path: &GeneratedPath) -> Vec<f64> {
    let mut out = Vec::new();
    let mut from = path.group.start;
    let (mut x, mut turned) = (f64::NAN, 0.0);
    for s in &path.group.segments {
        let still = s.kind == MoveKind::Cutting && (s.end.tip.x - from.tip.x).abs() < 1e-9;
        if still && (s.end.tip.x - x).abs() < 1e-9 {
            turned += (s.end.a.0 - from.a.0).abs();
        } else {
            if turned >= 359.999 {
                out.push(x);
            }
            (x, turned) = if still {
                (s.end.tip.x, (s.end.a.0 - from.a.0).abs())
            } else {
                (f64::NAN, 0.0)
            };
        }
        from = s.end;
    }
    if turned >= 359.999 {
        out.push(x);
    }
    out
}

#[test]
fn req_op_04_one_turn_at_each_face_where_the_side_touches_it() {
    let job = Job::shaft(ToolShape::FlatEnd);
    let path = job.path(&HelicalFinishing, &[]);
    let r = 0.5 * DIAMETER;
    let turns = turns_in_place(&path);
    assert_eq!(turns.len(), 2, "{turns:?}");
    // The tool's side on the face: a tool radius in front of X 10, behind X 25.
    // A tenth of the tolerance clear of the face, so the side meets it.
    let clear = 0.1 * TOLERANCE;
    assert!((turns[0] - (10.0 - r - clear)).abs() < 1e-5, "{turns:?}");
    assert!((turns[1] - (25.0 + r + clear)).abs() < 1e-5, "{turns:?}");
    // The helix, then each turn as its own cut after it, so the helix is as
    // it was (decision record 0297 item 3).
    let cuts = path
        .group
        .segments
        .windows(2)
        .filter(|w| w[0].kind != MoveKind::Cutting && w[1].kind == MoveKind::Cutting)
        .count();
    assert_eq!(cuts, 3, "{cuts} separate cuts");
}

#[test]
fn req_op_04_a_part_with_no_face_across_the_axis_gets_no_turn() {
    let cone = Cone {
        start_radius: 10.0,
        end_radius: 5.0,
        length: 30.0,
    };
    let job = Job {
        surface: Box::new(MeshSurface::new(&cone.mesh(61, 720)).unwrap()),
        ..Job::shaft(ToolShape::FlatEnd)
    };
    assert!(turns_in_place(&job.path(&HelicalFinishing, &[])).is_empty());
}

#[test]
fn req_nfr_06_req_gc_07_face_turns_are_identical_at_1_2_and_n_threads() {
    let job = Job::shaft(ToolShape::FlatEnd);
    for strategy in [&HelicalFinishing as &dyn Strategy, &RotaryRoughing] {
        let runs = openchuck_testkit::determinism::check(|| {
            format!("{:?}", job.path(strategy, &[]).group).into_bytes()
        })
        .unwrap();
        assert_eq!(runs.len(), 4);
    }
}

/// A plain cylinder of radius 10 from X 0 to X 30 with a short retaining
/// tab on the axis at each end (OP-21): radius 7.25, 0.8 mm long, so each tab
/// stands out from the part's end by less than the tool's radius. The shape
/// of the owner's blisk project at its far end (issue 404), where the turn
/// at the end face lies one tool radius beyond the face, past the tab's end.
const TAB_RADIUS: f64 = 7.25;
const TAB_LENGTH: f64 = 0.8;

fn short_tabs() -> Job {
    let along = openchuck_types::geometry::UnitVec3::new(Vec3::new(1.0, 0.0, 0.0)).unwrap();
    let tab = |x: f64| Solid::cylinder(Point3::new(x, 0.0, 0.0), along, TAB_LENGTH, TAB_RADIUS);
    let model: std::sync::Arc<dyn SurfaceSource> =
        std::sync::Arc::new(MeshSurface::new(&revolve(&[(0.0, 10.0), (30.0, 10.0)], 720)).unwrap());
    Job {
        surface: Box::new(CompositeSurface::new(
            model,
            vec![tab(-TAB_LENGTH).unwrap(), tab(30.0).unwrap()],
        )),
        stock: AnalyticStock::Cylinder {
            radius: 11.0,
            x_start: -TAB_LENGTH,
            x_end: 30.0 + TAB_LENGTH,
        },
        profile: tool(ToolShape::FlatEnd, DIAMETER).profile().unwrap(),
    }
}

/// Points on the part's two end faces outside the tabs, `step` mm apart:
/// from 0.05 mm beyond the tab's radius (the corner the tool's tip stands
/// in on top of the tab) to the part's radius.
fn end_faces(step: f64) -> Vec<Vec<Point3>> {
    let lo = TAB_RADIUS + 0.05;
    [0.0, 30.0]
        .into_iter()
        .map(|x| {
            let nr = ((10.0 - lo) / step).ceil() as u32;
            let mut out = Vec::new();
            for i in 0..=nr {
                let r = lo + (10.0 - lo) * f64::from(i) / f64::from(nr);
                let na = (std::f64::consts::TAU * r / step).ceil() as u32;
                for j in 0..na {
                    let t = std::f64::consts::TAU * f64::from(j) / f64::from(na);
                    out.push(Point3::new(x, r * t.cos(), r * t.sin()));
                }
            }
            out
        })
        .collect()
}

/// Points on each tab's round side and outer end, `step` mm apart.
fn tab_surfaces(step: f64) -> Vec<Point3> {
    let mut out = Vec::new();
    let na = (std::f64::consts::TAU * TAB_RADIUS / step).ceil() as u32;
    for (x0, outer) in [(-TAB_LENGTH, -TAB_LENGTH), (30.0, 30.0 + TAB_LENGTH)] {
        let nx = (TAB_LENGTH / step).ceil() as u32;
        for i in 0..=nx {
            let x = x0 + TAB_LENGTH * f64::from(i) / f64::from(nx);
            for j in 0..na {
                let t = std::f64::consts::TAU * f64::from(j) / f64::from(na);
                out.push(Point3::new(x, TAB_RADIUS * t.cos(), TAB_RADIUS * t.sin()));
            }
        }
        let nr = (TAB_RADIUS / step).ceil() as u32;
        for i in 0..nr {
            let r = TAB_RADIUS * f64::from(i) / f64::from(nr);
            let na = ((std::f64::consts::TAU * r / step).ceil() as u32).max(1);
            for j in 0..na {
                let t = std::f64::consts::TAU * f64::from(j) / f64::from(na);
                out.push(Point3::new(outer, r * t.cos(), r * t.sin()));
            }
        }
    }
    out
}

/// Each end face of the part beside a short tab is left at the stock to
/// leave within the tolerance, never cut into, and the tabs are never cut
/// (issue 404).
fn end_faces_beside_short_tabs_cleaned(strategy: &dyn Strategy) {
    let job = short_tabs();
    let path = job.path(strategy, &[]);
    let faces = end_faces(0.1);
    for (face, left) in faces.iter().zip(job.left(&path, &faces, 3.0)) {
        let (w, d) = (worst(&left), deepest(&left));
        let x = face[0].x;
        assert!(
            w <= TOLERANCE,
            "{} end face X {x}: {w:.4} mm left",
            strategy.name()
        );
        assert!(
            d >= -TOLERANCE,
            "{} end face X {x}: cut to {d:.4}",
            strategy.name()
        );
    }
    let tabs = tab_surfaces(0.1);
    let cut = deepest(&job.left(&path, &[tabs], 3.0)[0]);
    assert!(
        cut >= -TOLERANCE,
        "{}: a tab is cut into by {cut:.4} mm",
        strategy.name()
    );
}

#[test]
fn req_op_04_req_op_21_helical_finishing_faces_the_ends_beside_a_short_tab() {
    // Before the fix the turn at each end face, one tool radius beyond it,
    // lay past the tab's end, outside the range searched, and the face kept
    // what roughing left (0.43 mm mean on the owner's blisk project).
    end_faces_beside_short_tabs_cleaned(&HelicalFinishing);
}

#[test]
fn req_op_05_req_op_21_the_simultaneous_spiral_faces_the_ends_beside_a_short_tab() {
    end_faces_beside_short_tabs_cleaned(&SimultaneousFinishing);
}

#[test]
fn req_op_09_req_op_21_rest_machining_faces_the_ends_beside_a_short_tab() {
    // Rest machining is helical finishing where material is left, so it
    // turns at the faces as helical finishing does (decision record 0297).
    end_faces_beside_short_tabs_cleaned(&RestMachining);
}
