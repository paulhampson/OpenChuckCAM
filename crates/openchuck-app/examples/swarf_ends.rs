// SPDX-License-Identifier: AGPL-3.0-or-later
//! Measures how far swarf reaches past the ends of the face it cuts into the
//! faces beside it (lane SWARFEND; issue #442; OI-1622, OI-237). It changes
//! nothing: it places a STEP part as `swarf_floor` does, runs swarf on the
//! chosen faces with several tools, no avoid or check set, and checks every
//! fed pose and the quarter, middle and three-quarter points of every fed
//! move (on the machine's straight move in X, Y, Z and A) with the test kit's
//! bounded checker (`openchuck_testkit::contact`, decision record 0255)
//! against: the faces that share an edge with the cut face, one by one, and
//! every other face of the part. For each face it prints the deepest cut
//! (proven interval), where, and how far along the pass from its nearer end
//! the deepest station is; and for each run end, how far the end pose would
//! have to move back along the pass (between the poses swarf wrote) for the
//! tool to clear the neighbouring faces.
//!
//! ```text
//! cargo run --release -p openchuck-app --features kernel-tests --example swarf_ends -- \
//!     testdata/step/valid/blisk.step --faces 7,9,13
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

use std::collections::BTreeSet;

use openchuck_app::model::{Axis, Degrees, TransformStep};
use openchuck_geom::transform::{brep_placement, transform_brep};
use openchuck_import::step::StepImporter;
use openchuck_import::{ImportOptions, ModelImporter};
use openchuck_path::region::RegionConstraint;
use openchuck_path::surface::SurfaceSource;
use openchuck_path::swarf::{self, Settings};
use openchuck_path::{BrepSurface, RotaryMapping};
use openchuck_testkit::contact::{self, ConvexTool, Geometry};
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

/// The machine's straight move from `p` to `q` at fraction `f` (X, Y, Z and
/// A linear), as tip and axis in the model frame.
fn on_move(p: &ToolPose, q: &ToolPose, s: f64, f: f64) -> (Point3, Vec3) {
    let mp = rot_x(p.tip.to_vec(), s * p.a.0);
    let mq = rot_x(q.tip.to_vec(), s * q.a.0);
    let m = mp + (mq - mp) * f;
    let a = p.a.0 + (q.a.0 - p.a.0) * f;
    let tip = rot_x(m, -s * a);
    (
        Point3::new(tip.x, tip.y, tip.z),
        rot_x(Vec3::new(0.0, 0.0, 1.0), -s * a),
    )
}

fn sub_model(model: &BrepModel, keep: &BTreeSet<u32>) -> BrepModel {
    let mut m = model.clone();
    m.faces.retain(|f| keep.contains(&f.id.0));
    m
}

struct Station {
    tip: Point3,
    axis: Vec3,
    /// Distance along the pass (contact points) from the run's start.
    s: f64,
    run: usize,
}

fn main() {
    let mut it = std::env::args().skip(1);
    let file = it
        .next()
        .expect("usage: swarf_ends <file.step> --faces 7,9,13");
    let mut faces: Vec<u32> = vec![7, 9, 13];
    let mut exts: Vec<f64> = vec![0.0];
    // `--before` runs swarf as before decision record 0532, with no faces
    // next to the cut face given; `--off` gives them with Keep off the end
    // faces off.
    let mut before = false;
    let mut keep = true;
    let mut ends_of: Vec<(u32, Vec<u32>)> =
        vec![(7, vec![2, 15]), (9, vec![2, 15]), (13, vec![3, 15])];
    while let Some(x) = it.next() {
        match x.as_str() {
            "--faces" => {
                faces = it
                    .next()
                    .unwrap()
                    .split(',')
                    .map(|v| v.parse().unwrap())
                    .collect()
            }
            "--ends" => {
                ends_of = it
                    .next()
                    .unwrap()
                    .split(',')
                    .map(|e| {
                        let (f, l) = e.split_once(':').unwrap();
                        (
                            f.parse().unwrap(),
                            l.split('+').map(|v| v.parse().unwrap()).collect(),
                        )
                    })
                    .collect()
            }
            "--before" => before = true,
            "--off" => keep = false,
            "--extensions" => {
                exts = it
                    .next()
                    .unwrap()
                    .split(',')
                    .map(|v| v.parse().unwrap())
                    .collect()
            }
            o => panic!("unknown option {o}"),
        }
    }
    let model = placed(&std::fs::read(&file).expect("read the STEP file"));
    let rotary = RotaryMapping::of_profile(&MachineProfile::example_x_rotary());
    let cancel = CancelToken::new();
    let ctx = JobContext::new(&cancel, &NoProgress);
    let settings_c = contact::Settings {
        margin: 5.0,
        tolerance: 0.0005,
        budget: 4_000_000,
    };
    let all: BTreeSet<u32> = model.faces.iter().map(|f| f.id.0).collect();
    let tools = [
        ("flat 3 mm", tool(ToolShape::FlatEnd, 3.0)),
        ("flat 6 mm", tool(ToolShape::FlatEnd, 6.0)),
        ("ball 3 mm", tool(ToolShape::BallEnd, 3.0)),
        (
            "tapered ball 0.5 tip 3 deg 3 mm",
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
        "| face | tool | ext | against | deepest cut mm [proven at most] | where (x, r) | from nearer run end mm | stations cutting |"
    );
    println!("| --- | --- | --- | --- | --- | --- | --- | --- |");
    for n in &faces {
        let neighbours: BTreeSet<u32> = model
            .edges
            .iter()
            .filter(|e| e.faces.contains(&FaceId(*n)))
            .flat_map(|e| e.faces.iter().map(|f| f.0))
            .filter(|f| f != n)
            .collect();
        let end_faces: Vec<u32> = ends_of
            .iter()
            .find(|(f, _)| f == n)
            .map(|e| e.1.clone())
            .unwrap_or_default();
        eprintln!("face {n}: neighbours {neighbours:?}");
        for f in model
            .faces
            .iter()
            .filter(|f| neighbours.contains(&f.id.0) || f.id.0 == *n)
        {
            let (mut lo, mut hi) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
            for v in &f.tessellation.vertices {
                let r = v.y.hypot(v.z);
                lo = [lo[0].min(v.x), lo[1].min(r)];
                hi = [hi[0].max(v.x), hi[1].max(r)];
            }
            eprintln!(
                "  face {}: {:?} x {:.2}..{:.2} r {:.2}..{:.2}",
                f.id.0, f.surface, lo[0], hi[0], lo[1], hi[1]
            );
        }
        let mut groups: Vec<(String, Geometry)> = neighbours
            .iter()
            .map(|f| {
                (
                    format!("face {f}"),
                    Geometry::of_faces(&sub_model(&model, &BTreeSet::from([*f]))).unwrap(),
                )
            })
            .collect();
        let rest: BTreeSet<u32> = all
            .iter()
            .copied()
            .filter(|f| f != n && !neighbours.contains(f))
            .collect();
        groups.push((
            "every other face".to_owned(),
            Geometry::of_faces(&sub_model(&model, &rest)).unwrap(),
        ));
        let selected = BrepSurface::with_faces(&model, &[FaceId(*n)]).unwrap();
        let face = selected.exact_faces().into_iter().next().unwrap();
        let ids: Vec<FaceId> = neighbours.iter().map(|f| FaceId(*f)).collect();
        let next_to = BrepSurface::with_faces(&model, &ids).unwrap();
        let region = RegionConstraint {
            neighbours: (!before).then_some(&next_to as &dyn SurfaceSource),
            ..RegionConstraint::default()
        };
        for (name, profile) in &tools {
            let ct = ConvexTool::new(profile).unwrap();
            for ext in &exts {
                let settings = Settings {
                    tolerance: 0.01,
                    stock_to_leave: 0.0,
                    tip_extension: *ext,
                    max_material_left: 0.1,
                    reverse_tip_end: false,
                    reverse_direction: false,
                    keep_off_end_faces: keep,
                };
                let started = std::time::Instant::now();
                let made = swarf::face_path(face, profile, rotary, &settings, &region, &ctx);
                let took = started.elapsed().as_secs_f64();
                let path = match made {
                    Ok(p) => {
                        let stops: Vec<String> = p
                            .limits
                            .end_stops
                            .iter()
                            .map(|e| format!("face {} {:.3} mm", e.face, e.length))
                            .collect();
                        let cuts: Vec<String> = p
                            .limits
                            .neighbour_cuts
                            .iter()
                            .map(|c| {
                                format!(
                                    "face {} {:.3} mm{}",
                                    c.face,
                                    c.depth,
                                    if c.end { " (end)" } else { "" }
                                )
                            })
                            .collect();
                        println!(
                            "| {n} | {name} | {ext} | swarf in {took:.2} s: gouge {:.4}, left {:.4}; stops short of {}; warns of {} |",
                            p.gouge,
                            p.left,
                            if stops.is_empty() {
                                "nothing".to_owned()
                            } else {
                                stops.join(", ")
                            },
                            if cuts.is_empty() {
                                "nothing".to_owned()
                            } else {
                                cuts.join(", ")
                            },
                        );
                        p
                    }
                    Err(e) => {
                        println!("| {n} | {name} | {ext} | refused: {e} |");
                        continue;
                    }
                };
                let mut stations: Vec<Station> = Vec::new();
                let mut run_len = Vec::new();
                for (ri, run) in path.runs.iter().enumerate() {
                    let poses: Vec<ToolPose> = run
                        .iter()
                        .map(|p| ToolPose {
                            tip: p.tip,
                            axis: p.axis,
                            a: p.a,
                        })
                        .collect();
                    let sense = sense_of(&poses);
                    let mut s = 0.0;
                    for (i, p) in run.iter().enumerate() {
                        stations.push(Station {
                            tip: p.tip,
                            axis: p.axis.get(),
                            s,
                            run: ri,
                        });
                        if let Some(q) = run.get(i + 1) {
                            let d = p.contact.distance(q.contact);
                            for k in 1..4 {
                                let f = f64::from(k) / 4.0;
                                let (tip, axis) = on_move(&poses[i], &poses[i + 1], sense, f);
                                stations.push(Station {
                                    tip,
                                    axis,
                                    s: s + d * f,
                                    run: ri,
                                });
                            }
                            s += d;
                        }
                    }
                    run_len.push(s);
                }
                for (gname, geom) in &groups {
                    let obs: Vec<contact::Bounded> = stations
                        .par_iter()
                        .map(|st| geom.clearance(&ct, st.tip, st.axis, &settings_c).unwrap())
                        .collect();
                    let mut worst = (f64::INFINITY, f64::INFINITY, None, 0.0);
                    let mut cutting = 0;
                    for (st, b) in stations.iter().zip(&obs) {
                        if b.upper < -0.001 {
                            cutting += 1;
                        }
                        if b.upper < worst.0 {
                            let from_end = st.s.min(run_len[st.run] - st.s);
                            worst = (b.upper, b.lower, b.at, from_end);
                        }
                    }
                    if std::env::var("DUMP").is_ok_and(|v| gname.contains(&v)) {
                        for (st, b) in stations.iter().zip(&obs).step_by(8) {
                            let at = b.at.unwrap_or(Point3::ORIGIN);
                            eprintln!(
                                "{name} {gname} s={:.2} tip=({:.2},{:.2},{:.2}) axis=({:.3},{:.3},{:.3}) c={:.3} at=({:.2},{:.2},{:.2})",
                                st.s,
                                st.tip.x,
                                st.tip.y,
                                st.tip.z,
                                st.axis.x,
                                st.axis.y,
                                st.axis.z,
                                b.upper,
                                at.x,
                                at.y,
                                at.z
                            );
                        }
                    }
                    if worst.0 < 0.001 {
                        let at = worst.2.unwrap_or(Point3::ORIGIN);
                        println!(
                            "| {n} | {name} | {ext} | {gname} | {:.3} [{:.3}] | ({:.2}, {:.2}) | {:.2} | {cutting}/{} |",
                            -worst.0,
                            -worst.1,
                            at.x,
                            at.y.hypot(at.z),
                            worst.3,
                            stations.len()
                        );
                    }
                }
                // How far back from each run end the tool first clears the end
                // faces (those across the end rulings), within the tolerance.
                let ends: Vec<&Geometry> = groups
                    .iter()
                    .filter(|(g, _)| end_faces.iter().any(|f| *g == format!("face {f}")))
                    .map(|g| &g.1)
                    .collect();
                let radius = profile.cutting_radius();
                for (ri, len) in run_len.iter().enumerate() {
                    let mine: Vec<&Station> = stations.iter().filter(|s| s.run == ri).collect();
                    let cut: Vec<f64> = mine
                        .par_iter()
                        .map(|st| {
                            ends.iter()
                                .map(|g| {
                                    g.clearance(&ct, st.tip, st.axis, &settings_c)
                                        .unwrap()
                                        .upper
                                })
                                .fold(f64::INFINITY, f64::min)
                        })
                        .collect();
                    let bad = |c: &f64| *c < -0.01;
                    let start = mine
                        .iter()
                        .zip(&cut)
                        .filter(|(_, c)| bad(c))
                        .map(|(s, _)| s.s)
                        .filter(|v| *v < 0.5 * len)
                        .fold(0.0, f64::max);
                    let end = mine
                        .iter()
                        .zip(&cut)
                        .filter(|(_, c)| bad(c))
                        .map(|(s, _)| len - s.s)
                        .filter(|v| *v < 0.5 * len)
                        .fold(0.0, f64::max);
                    println!(
                        "| {n} | {name} | {ext} | run {ri} of {len:.2} mm: stop short at start / end to clear faces {end_faces:?} (tool radius {radius}) | {start:.2} / {end:.2} | | | |"
                    );
                }
            }
        }
    }
}
