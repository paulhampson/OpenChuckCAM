// SPDX-License-Identifier: AGPL-3.0-or-later
//! Measures how far swarf cuts into the floor below the faces it cuts (lane
//! SWARF-FLOOR; issue #435; decision record 0491). It changes nothing: it
//! reads a STEP part, places it on the rotary axis as `swarf_tolerance` does,
//! runs swarf on the chosen faces with several tools and tip extensions, with
//! and without the floor faces in an avoid set on Cut short, and measures the
//! written motion against the floor faces with the test kit's independent
//! tool outline (`openchuck_testkit::gouge`), the machine's straight moves in
//! X, Y, Z and A rebuilt from the poses. The library's gate measures only the
//! cut face (OI-237), so this is the measure of the floor.
//!
//! For each run it prints the tip extension, whether the floor is in an avoid
//! set, the most the tool is lifted, the length along the face left uncut,
//! the deepest cut into the floor and where, and the material left on the face
//! above the lifted tip; a refusal is printed in its place.
//!
//! ```text
//! cargo run --release -p openchuck-app --features kernel-tests --example swarf_floor -- \
//!     testdata/step/valid/blisk.step --faces 7,9,13 --floor 8,12,17,20,23 \
//!     [--extensions 0,-0.5,-1] [--sample 0.05]
//! ```

#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::too_many_lines,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::BTreeMap;

use openchuck_app::model::{Axis, Degrees, TransformStep};
use openchuck_geom::transform::{brep_placement, transform_brep};
use openchuck_import::step::StepImporter;
use openchuck_import::{ImportOptions, ModelImporter};
use openchuck_path::region::RegionConstraint;
use openchuck_path::surface::{ParametricFace, SurfaceSource, Uv};
use openchuck_path::swarf::{self, Settings};
use openchuck_path::{BrepSurface, RotaryMapping};
use openchuck_testkit::gouge::Outline;
use openchuck_types::brep::{BrepModel, FaceId};
use openchuck_types::geometry::{Aabb, Point3, Vec3};
use openchuck_types::ir::ToolPose;
use openchuck_types::job::{CancelToken, JobContext, NoProgress};
use openchuck_types::machine::MachineProfile;
use openchuck_types::model::ModelGeometry;
use openchuck_types::tool::profile::RevolvedProfile;
use openchuck_types::tool::{Tool, ToolShape};
use openchuck_types::units::Millimetres;
use rayon::prelude::*;

struct Args {
    file: String,
    faces: Vec<u32>,
    floor: Vec<u32>,
    extensions: Vec<f64>,
    sample: f64,
}

fn numbers<T: std::str::FromStr>(s: &str) -> Vec<T> {
    s.split(',')
        .filter(|x| !x.is_empty())
        .map(|x| x.trim().parse().ok().expect("a number"))
        .collect()
}

fn args() -> Args {
    let mut it = std::env::args().skip(1);
    let mut a = Args {
        file: it.next().expect("usage: swarf_floor <file.step> [options]"),
        faces: vec![7, 9, 13],
        floor: vec![8, 12, 17, 20, 23],
        extensions: vec![0.0],
        sample: 0.05,
    };
    while let Some(x) = it.next() {
        let mut v = || it.next().expect("a value");
        match x.as_str() {
            "--faces" => a.faces = numbers(&v()),
            "--floor" => a.floor = numbers(&v()),
            "--extensions" => a.extensions = numbers(&v()),
            "--sample" => a.sample = v().parse().unwrap(),
            other => panic!("unknown option {other}"),
        }
    }
    a
}

fn surface_bounds(model: &BrepModel) -> Aabb {
    let (t, _) = model.joined_tessellation();
    let mut b = Aabb {
        min: Point3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY),
        max: Point3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY),
    };
    for p in &t.vertices {
        b.min = Point3::new(b.min.x.min(p.x), b.min.y.min(p.y), b.min.z.min(p.z));
        b.max = Point3::new(b.max.x.max(p.x), b.max.y.max(p.y), b.max.z.max(p.z));
    }
    b
}

/// The part as pencil_creases and step_accuracy place it: its longest
/// span along X, centred on the rotary axis, its end 10 mm from the chuck.
fn placed(bytes: &[u8]) -> BrepModel {
    let cancel = CancelToken::new();
    let imported = StepImporter::new()
        .import(
            bytes,
            &ImportOptions::default(),
            &JobContext::new(&cancel, &NoProgress),
        )
        .unwrap();
    let ModelGeometry::Brep(model) = imported.geometry else {
        panic!("not a B-rep");
    };
    let b = surface_bounds(&model);
    let spans = [b.max.x - b.min.x, b.max.y - b.min.y, b.max.z - b.min.z];
    let mut steps = Vec::new();
    if spans[2] > spans[0] && spans[2] > spans[1] {
        steps.push(TransformStep::Rotate {
            axis: Axis::Y,
            angle: Degrees(90.0),
        });
    } else if spans[1] > spans[0] && spans[1] > spans[2] {
        steps.push(TransformStep::Rotate {
            axis: Axis::Z,
            angle: Degrees(-90.0),
        });
    }
    steps.push(TransformStep::CentreOnRotaryAxis);
    let turned = transform_brep(&model, &brep_placement(&model, &steps).unwrap()).unwrap();
    steps.push(TransformStep::Translate {
        x: 10.0 - surface_bounds(&turned).min.x,
        y: 0.0,
        z: 0.0,
    });
    transform_brep(&model, &brep_placement(&model, &steps).unwrap()).unwrap()
}

// ---------------------------------------------------------------------------
// The checker's own kinematics: a rotation of the model about X by s * A
// takes the tool axis to machine +Z; s is read from the poses.

fn rot_x(p: Vec3, degrees: f64) -> Vec3 {
    let (s, c) = degrees.to_radians().sin_cos();
    Vec3::new(p.x, c * p.y - s * p.z, s * p.y + c * p.z)
}

fn sense_of(poses: &[ToolPose]) -> f64 {
    let up = Vec3::new(0.0, 0.0, 1.0);
    let err = |s: f64| {
        poses
            .iter()
            .map(|p| (rot_x(p.axis.get(), s * p.a.0) - up).length())
            .fold(0.0, f64::max)
    };
    let (plus, minus) = (err(1.0), err(-1.0));
    let s = if plus < minus { 1.0 } else { -1.0 };
    assert!(
        plus.min(minus) < 1e-9,
        "the poses' axes are not the machine vertical under a rotation about X ({plus}, {minus})"
    );
    s
}

/// A tool position: tip and unit axis in the model frame.
#[derive(Clone, Copy)]
struct At {
    tip: Point3,
    axis: Vec3,
}

/// The machine's straight move from `p` to `q`, cut into steps no longer
/// than `step` mm at the tip and the flank's far end.
fn motion(p: &ToolPose, q: &ToolPose, s: f64, reach: f64, step: f64, out: &mut Vec<At>) {
    let mp = rot_x(p.tip.to_vec(), s * p.a.0);
    let mq = rot_x(q.tip.to_vec(), s * q.a.0);
    let turn = (q.a.0 - p.a.0).abs().to_radians() * reach;
    let n = (((mq - mp).length() + turn) / step).ceil().max(1.0) as usize;
    for k in 1..=n {
        let f = k as f64 / n as f64;
        let m = mp + (mq - mp) * f;
        let a = p.a.0 + (q.a.0 - p.a.0) * f;
        let tip = rot_x(m, -s * a);
        out.push(At {
            tip: Point3::new(tip.x, tip.y, tip.z),
            axis: rot_x(Vec3::new(0.0, 0.0, 1.0), -s * a),
        });
    }
}

/// Dense points of the trimmed face.
fn face_points(face: &dyn ParametricFace, spacing: f64) -> Vec<Point3> {
    let (lo, hi) = face.uv_bounds();
    let uv = |a: f64, b: f64| Uv {
        u: lo.u + (hi.u - lo.u) * a,
        v: lo.v + (hi.v - lo.v) * b,
    };
    let length = |line: &dyn Fn(f64) -> Uv| {
        let pts: Vec<Point3> = (0..=64)
            .filter_map(|k| face.point_normal(line(k as f64 / 64.0)).map(|p| p.point))
            .collect();
        pts.windows(2).map(|w| w[0].distance(w[1])).sum::<f64>()
    };
    let lu = [0.1, 0.5, 0.9]
        .iter()
        .map(|b| length(&|a| uv(a, *b)))
        .fold(0.0, f64::max);
    let lv = [0.1, 0.5, 0.9]
        .iter()
        .map(|a| length(&|b| uv(*a, b)))
        .fold(0.0, f64::max);
    let nu = ((lu / spacing).ceil() as usize + 1).max(2);
    let nv = ((lv / spacing).ceil() as usize + 1).max(2);
    (0..nu)
        .into_par_iter()
        .flat_map_iter(|i| {
            (0..nv).filter_map(move |j| {
                let at = uv(i as f64 / (nu - 1) as f64, j as f64 / (nv - 1) as f64);
                if face.contains(at) {
                    face.point_normal(at).map(|p| p.point)
                } else {
                    None
                }
            })
        })
        .collect()
}

/// The worst cut into `points` (positive = into the face) by the tool at
/// any of `positions`, with where, and the material left at the points the
/// tool passes within `band` mm of.
#[derive(Clone, Copy)]
struct Measure {
    gouge: f64,
    at: Point3,
}

fn measure(points: &[Point3], positions: &[At], outline: &Outline, height: f64) -> Measure {
    let radius = outline.radius();
    let bound = (radius * radius + 0.25 * height * height).sqrt() + 0.01;
    let cell = bound;
    let key = |c: Point3| {
        (
            (c.x / cell).floor() as i64,
            (c.y / cell).floor() as i64,
            (c.z / cell).floor() as i64,
        )
    };
    let mut grid: BTreeMap<(i64, i64, i64), Vec<usize>> = BTreeMap::new();
    for (i, p) in positions.iter().enumerate() {
        let c = p.tip + p.axis * (0.5 * height);
        grid.entry(key(c)).or_default().push(i);
    }
    let worst: Vec<(f64, Point3)> = points
        .par_iter()
        .map(|p| {
            let (kx, ky, kz) = key(*p);
            let mut best = f64::INFINITY;
            for dx in -1..=1 {
                for dy in -1..=1 {
                    for dz in -1..=1 {
                        let Some(list) = grid.get(&(kx + dx, ky + dy, kz + dz)) else {
                            continue;
                        };
                        for i in list {
                            let t = positions[*i];
                            let d = *p - t.tip;
                            let z = d.dot(t.axis);
                            if z > height + 0.01 {
                                continue;
                            }
                            let r = (d - t.axis * z).length();
                            if r > radius + 0.05 {
                                continue;
                            }
                            best = best.min(outline.clearance(r, z));
                        }
                    }
                }
            }
            (best, *p)
        })
        .collect();
    let mut m = Measure {
        gouge: 0.0,
        at: Point3::ORIGIN,
    };
    for (c, p) in worst {
        if c.is_finite() && -c > m.gouge {
            m.gouge = -c;
            m.at = p;
        }
    }
    m
}

fn tool(shape: ToolShape, diameter: f64) -> RevolvedProfile {
    Tool {
        shape,
        diameter: Millimetres(diameter),
        flute_length: Millimetres(30.0),
        overall_length: Millimetres(80.0),
        shank_diameter: Millimetres(diameter),
        flute_count: 2,
        material: None,
        centre_cutting: None,
    }
    .profile()
    .unwrap()
}

fn main() {
    let a = args();
    let model = placed(&std::fs::read(&a.file).expect("read the STEP file"));
    let rotary = RotaryMapping::of_profile(&MachineProfile::example_x_rotary());
    let cancel = CancelToken::new();
    let ctx = JobContext::new(&cancel, &NoProgress);
    let floor_ids: Vec<FaceId> = a.floor.iter().map(|n| FaceId(*n)).collect();
    let floor = BrepSurface::with_faces(&model, &floor_ids).unwrap();
    let floor_points: Vec<Point3> = floor
        .exact_faces()
        .iter()
        .flat_map(|f| face_points(f.face, a.sample))
        .collect();
    eprintln!("{} floor points at {} mm", floor_points.len(), a.sample);
    let tools = [
        ("flat 3 mm", tool(ToolShape::FlatEnd, 3.0)),
        ("flat 6 mm", tool(ToolShape::FlatEnd, 6.0)),
        ("ball 3 mm", tool(ToolShape::BallEnd, 3.0)),
        (
            "tapered ball 0.5 mm tip, 3 deg, 3 mm",
            tool(
                ToolShape::TaperedBall {
                    tip_radius: Millimetres(0.5),
                    taper_half_angle: Degrees(3.0),
                },
                3.0,
            ),
        ),
    ];
    println!(
        "| face | tool | tip extension mm | floor in avoid set | most lift mm | uncut along face mm | cut into floor mm | where (x, r) | material left on face mm |"
    );
    println!("| --- | --- | --- | --- | --- | --- | --- | --- | --- |");
    for n in &a.faces {
        let selected = BrepSurface::with_faces(&model, &[FaceId(*n)]).unwrap();
        let face = selected.exact_faces().into_iter().next().unwrap();
        for (name, profile) in &tools {
            let outline = Outline::new(profile);
            for ext in &a.extensions {
                for avoid in [false, true] {
                    let settings = Settings {
                        tolerance: 0.01,
                        stock_to_leave: 0.0,
                        tip_extension: *ext,
                        max_material_left: 0.1,
                        reverse_tip_end: false,
                        reverse_direction: false,
                    };
                    let region = RegionConstraint {
                        avoid: avoid.then_some(&floor as &dyn SurfaceSource),
                        ..RegionConstraint::default()
                    };
                    let path =
                        match swarf::face_path(face, profile, rotary, &settings, &region, &ctx) {
                            Ok(p) => p,
                            Err(e) => {
                                println!("| {n} | {name} | {ext} | {avoid} | refused: {e} |");
                                continue;
                            }
                        };
                    let mut positions: Vec<At> = Vec::new();
                    for run in &path.runs {
                        let poses: Vec<ToolPose> = run
                            .iter()
                            .map(|p| ToolPose {
                                tip: p.tip,
                                axis: p.axis,
                                a: p.a,
                            })
                            .collect();
                        let sense = sense_of(&poses);
                        if let Some(first) = poses.first() {
                            positions.push(At {
                                tip: first.tip,
                                axis: first.axis.get(),
                            });
                        }
                        for w in poses.windows(2) {
                            motion(&w[0], &w[1], sense, 30.0, 0.02, &mut positions);
                        }
                    }
                    let m = measure(&floor_points, &positions, &outline, 30.0);
                    let r = m.at.y.hypot(m.at.z);
                    let uncut =
                        path.limits.avoid_uncut + path.limits.check_uncut + path.limits.too_short;
                    println!(
                        "| {n} | {name} | {ext} | {avoid} | {:.3} | {uncut:.2} | {:.3} | ({:.2}, {r:.2}) | {:.4} |",
                        path.limits.tip_lifted, m.gouge, m.at.x, path.left
                    );
                }
            }
        }
    }
}
