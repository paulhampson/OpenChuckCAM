// SPDX-License-Identifier: AGPL-3.0-or-later
//! Measures how swarf's tolerance changes the written path and the cut it
//! makes into the face (lane BUG-SWARF4; issue #296). It changes nothing: it
//! reads a STEP part, places it on the rotary axis, runs swarf on the chosen
//! faces at several tolerances, with and without an avoid set, and measures
//! the written motion with a checker of its own:
//!
//! - its own rotary kinematics (the machine's straight move in X, Y, Z and A
//!   rebuilt from the IR's poses, the sense of A read from the poses
//!   themselves, not from the library's `RotaryMapping`);
//! - the testkit's independent tool outline (`openchuck_testkit::gouge`);
//! - a dense grid of the exact face, and each move cut into short steps.
//!
//! For each run it prints the deviation swarf reports and the one measured,
//! and splits the measured cut into its sources:
//!
//! - **placement**: the poses alone, with no motion between them, on the
//!   path made at a very fine tolerance (what the placement leaves whatever
//!   the spacing);
//! - **fitting**: the written motion against the placement-only figure;
//! - **checking**: swarf's report against the measured written motion;
//! - **avoid handling**: the same tolerance with and without the avoid set.
//!
//! ```text
//! cargo run --release -p openchuck-app --features kernel-tests --example swarf_tolerance -- \
//!     testdata/step/valid/blisk.step [--list] [--faces 7,8] [--avoid 3,4] \
//!     [--tolerances 0.01,0.0132,0.02,0.05] [--tool 6] [--ball] [--sample 0.02] [--steps 0.002]
//! ```

#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::too_many_lines,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic
)]

use std::collections::BTreeMap;
use std::time::Instant;

use openchuck_app::model::{Axis, Degrees, TransformStep};
use openchuck_geom::transform::{brep_placement, transform_brep};
use openchuck_import::step::StepImporter;
use openchuck_import::{ImportOptions, ModelImporter};
use openchuck_path::params::names;
use openchuck_path::region::{BoundaryLimits, RegionConstraint};
use openchuck_path::surface::{ParametricFace, SurfaceSource, Uv};
use openchuck_path::swarf::{self, Settings, detect_rulings};
use openchuck_path::{
    AcceptAll, AnalyticStock, BrepSurface, ParameterValue, RotaryMapping, StrategyInput, Swarf,
    generate_in_region,
};
use openchuck_testkit::gouge::Outline;
use openchuck_types::brep::{BrepModel, FaceId};
use openchuck_types::geometry::{Aabb, Point3, Vec3};
use openchuck_types::ir::{MoveKind, PathTags, ToolPose};
use openchuck_types::job::{CancelToken, JobContext, NoProgress};
use openchuck_types::machine::{Clearance, MachineProfile};
use openchuck_types::model::ModelGeometry;
use openchuck_types::tool::profile::RevolvedProfile;
use openchuck_types::tool::{Tool, ToolShape};
use openchuck_types::units::Millimetres;
use rayon::prelude::*;

struct Args {
    file: String,
    list: bool,
    faces: Vec<u32>,
    avoid: Vec<u32>,
    tolerances: Vec<f64>,
    tool: f64,
    ball: bool,
    sample: f64,
    steps: f64,
    reference: f64,
    check: bool,
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
        file: it
            .next()
            .expect("usage: swarf_tolerance <file.step> [options]"),
        list: false,
        faces: Vec::new(),
        avoid: Vec::new(),
        tolerances: vec![0.01, 0.0131, 0.0132, 0.02, 0.05],
        tool: 6.0,
        ball: false,
        sample: 0.02,
        steps: 0.002,
        reference: 0.0005,
        check: true,
    };
    while let Some(x) = it.next() {
        let mut v = || it.next().expect("a value");
        match x.as_str() {
            "--list" => a.list = true,
            "--no-check" => a.check = false,
            "--faces" => a.faces = numbers(&v()),
            "--avoid" => a.avoid = numbers(&v()),
            "--tolerances" => a.tolerances = numbers(&v()),
            "--tool" => a.tool = v().parse().unwrap(),
            "--ball" => a.ball = true,
            "--sample" => a.sample = v().parse().unwrap(),
            "--steps" => a.steps = v().parse().unwrap(),
            "--reference" => a.reference = v().parse().unwrap(),
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

fn tool(diameter: f64, ball: bool) -> RevolvedProfile {
    Tool {
        shape: if ball {
            ToolShape::BallEnd
        } else {
            ToolShape::FlatEnd
        },
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

fn values(tolerance: f64) -> BTreeMap<String, ParameterValue> {
    [
        (names::FEED, 600.0),
        (names::PLUNGE_FEED, 200.0),
        (names::TOLERANCE, tolerance),
    ]
    .into_iter()
    .map(|(k, x)| (k.to_owned(), ParameterValue::Number(x)))
    .collect()
}

fn main() {
    let a = args();
    let t = Instant::now();
    let model = placed(&std::fs::read(&a.file).expect("read the STEP file"));
    let whole = BrepSurface::new(&model).unwrap();
    eprintln!("imported and placed in {:.1} s", t.elapsed().as_secs_f64());
    if a.list {
        println!("| face | family | residual mm | twist deg | ruled at 0.01 |");
        println!("| --- | --- | --- | --- | --- |");
        for f in whole.exact_faces() {
            let d = detect_rulings(f.face, 0.01);
            let b = d.best();
            println!(
                "| {} | {:?} | {:.5} | {:.2} | {} |",
                f.number,
                b.family,
                b.residual,
                b.twist,
                d.chosen.is_some()
            );
        }
        return;
    }
    let profile = tool(a.tool, a.ball);
    let outline = Outline::new(&profile);
    let height = 30.0;
    let machine = MachineProfile::example_x_rotary();
    let rotary = RotaryMapping::of_profile(&machine);
    let ids: Vec<FaceId> = a.faces.iter().map(|n| FaceId(*n)).collect();
    let selected = BrepSurface::with_faces(&model, &ids).unwrap();
    let avoid_ids: Vec<FaceId> = a.avoid.iter().map(|n| FaceId(*n)).collect();
    let avoid =
        (!avoid_ids.is_empty()).then(|| BrepSurface::with_faces(&model, &avoid_ids).unwrap());
    let stock_radius = {
        let b = surface_bounds(&model);
        [b.min.y, b.max.y, b.min.z, b.max.z]
            .iter()
            .fold(0.0f64, |m, x| m.max(x.abs()))
            + 2.0
    };
    let stock = AnalyticStock::Cylinder {
        radius: stock_radius,
        x_start: 0.0,
        x_end: 400.0,
    };
    let tags = PathTags {
        operation: "swarf".to_owned(),
        setup: "s1".to_owned(),
        tool: "t1".to_owned(),
    };
    let input = StrategyInput {
        surface: &selected,
        stock: &stock,
        collision: &AcceptAll,
        tool: &profile,
        rotary,
        clearance: Clearance {
            radius: Millimetres(stock_radius + 10.0),
            safe_z: Millimetres(stock_radius + 20.0),
        },
        max_feeds: machine.max_feeds,
        tags: &tags,
        entry_safety: None,
        facing: None,
        keep_out: &[],
    };
    let points: Vec<(u32, Vec<Point3>)> = selected
        .exact_faces()
        .iter()
        .map(|f| {
            (
                f.number,
                if a.check {
                    face_points(f.face, a.sample)
                } else {
                    Vec::new()
                },
            )
        })
        .collect();
    for (n, p) in &points {
        eprintln!("face {n}: {} check points at {} mm", p.len(), a.sample);
    }
    let cancel = CancelToken::new();
    let ctx = JobContext::new(&cancel, &NoProgress);

    // Placement alone: the poses of the path at a very fine tolerance, with
    // no motion between them.
    let settings = |tolerance: f64| Settings {
        tolerance,
        stock_to_leave: 0.0,
        tip_extension: 0.0,
        max_material_left: 0.0,
        reverse_tip_end: false,
        reverse_direction: false,
    };
    let fine = RegionConstraint::default();
    for f in selected.exact_faces().into_iter().filter(|_| a.check) {
        let fp =
            swarf::face_path(f, &profile, rotary, &settings(a.reference), &fine, &ctx).unwrap();
        let poses: Vec<At> = fp
            .poses()
            .map(|p| At {
                tip: p.tip,
                axis: p.axis.get(),
            })
            .collect();
        let pts = &points.iter().find(|(n, _)| *n == f.number).unwrap().1;
        let m = measure(pts, &poses, &outline, height);
        println!(
            "placement, face {}: {} poses at tolerance {} mm, reported {:.4} mm, measured at the poses alone {:.4} mm at ({:.3}, {:.3}, {:.3}); twist {:.2} deg, tilt {:.2} deg",
            f.number,
            poses.len(),
            a.reference,
            fp.gouge,
            m.gouge,
            m.at.x,
            m.at.y,
            m.at.z,
            fp.twist,
            fp.max_tilt
        );
    }
    println!();
    println!(
        "| avoid | tolerance mm | face | poses (library) | IR cut moves | reported mm | measured written mm | at | measured library poses+motion mm | warned |"
    );
    println!("| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |");
    for with_avoid in [false, true] {
        if with_avoid && avoid.is_none() {
            continue;
        }
        for &tol in &a.tolerances {
            let region = RegionConstraint {
                region: None,
                boundary: None,
                avoid: if with_avoid {
                    avoid.as_ref().map(|s| s as &dyn SurfaceSource)
                } else {
                    None
                },
                check: None,
                boundary_limits: BoundaryLimits {
                    pass: true,
                    tip: true,
                },
                watch: &[],
            };
            let t = Instant::now();
            let generated = match generate_in_region(&Swarf, &input, &region, &values(tol), &ctx) {
                Ok(g) => g,
                Err(e) => {
                    println!("| {with_avoid} | {tol} | - | refused: {e} |");
                    continue;
                }
            };
            let elapsed = t.elapsed().as_secs_f64();
            for w in &generated.warnings {
                eprintln!("  warning at tolerance {tol}: {}", w.message);
            }
            let mut poses = vec![generated.group.start];
            let mut cuts: Vec<(ToolPose, ToolPose)> = Vec::new();
            for s in &generated.group.segments {
                let prev = *poses.last().unwrap();
                if s.kind == MoveKind::Cutting {
                    cuts.push((prev, s.end));
                }
                poses.push(s.end);
            }
            let sense = sense_of(&poses);
            let mut written: Vec<At> = Vec::new();
            for (p, q) in &cuts {
                written.push(At {
                    tip: p.tip,
                    axis: p.axis.get(),
                });
                motion(p, q, sense, height, a.steps, &mut written);
            }
            for f in selected.exact_faces() {
                let fp =
                    swarf::face_path(f, &profile, rotary, &settings(tol), &region, &ctx).unwrap();
                // The library's own runs, moved by the checker's kinematics.
                let mut lib: Vec<At> = Vec::new();
                for run in &fp.runs {
                    let tp: Vec<ToolPose> = run
                        .iter()
                        .map(|p| ToolPose {
                            tip: p.tip,
                            axis: p.axis,
                            a: p.a,
                        })
                        .collect();
                    for w in tp.windows(2) {
                        lib.push(At {
                            tip: w[0].tip,
                            axis: w[0].axis.get(),
                        });
                        motion(&w[0], &w[1], sense, height, a.steps, &mut lib);
                    }
                }
                let pts = &points.iter().find(|(n, _)| *n == f.number).unwrap().1;
                let none = Measure {
                    gouge: f64::NAN,
                    at: Point3::ORIGIN,
                };
                let (m, ml) = if a.check {
                    (
                        measure(pts, &written, &outline, height),
                        measure(pts, &lib, &outline, height),
                    )
                } else {
                    (none, none)
                };
                let warned = generated
                    .warnings
                    .iter()
                    .any(|w| w.message.contains(&format!("face {} cuts", f.number)));
                println!(
                    "| {with_avoid} | {tol} | {} | {} | {} | {:.4} at ({:.3}, {:.3}, {:.3}) | {:.4} | ({:.3}, {:.3}, {:.3}) | {:.4} | {warned} |",
                    f.number,
                    fp.poses().count(),
                    cuts.len(),
                    fp.gouge,
                    fp.gouge_at.x,
                    fp.gouge_at.y,
                    fp.gouge_at.z,
                    m.gouge,
                    m.at.x,
                    m.at.y,
                    m.at.z,
                    ml.gouge,
                );
            }
            eprintln!("  tolerance {tol}, avoid {with_avoid}: generated in {elapsed:.2} s");
        }
    }
}
