// SPDX-License-Identifier: AGPL-3.0-or-later
//! The measuring functions of the `step_accuracy` example (decision record
//! 0211; decision 0209 item 4.1), checked on closed forms before their
//! numbers are trusted: the tessellation's distance from its exact face is
//! measured on the triangles, never taken from the kernel's setting (IMP-06,
//! decision 0003, OI-65), and the cut is judged against points on the exact
//! faces, with a move's middle and quarter points placed along the machine's
//! straight move (OP-12; decisions 0115 and 0133). Needs the kernel:
//! `--features kernel-tests`.
#![cfg(feature = "kernel-tests")]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::print_stdout,
    dead_code
)]

#[path = "../examples/step_accuracy/measure.rs"]
mod measure;

use std::path::Path;

use openchuck_app::model::{Axis, Degrees, TransformStep};
use openchuck_geom::transform::{brep_placement, transform_brep};
use openchuck_import::step::StepImporter;
use openchuck_import::{ImportOptions, ModelImporter};
use openchuck_path::RotaryMapping;
use openchuck_testkit::gouge::Outline;
use openchuck_types::brep::{BrepModel, TessellationSettings};
use openchuck_types::geometry::{Point3, Vec3};
use openchuck_types::ir::ToolPose;
use openchuck_types::job::{CancelToken, JobContext, NoProgress};
use openchuck_types::model::ModelGeometry;
use openchuck_types::tool::{Tool, ToolShape};
use openchuck_types::units::Millimetres;

use measure::{Cloud, Station};

/// The corpus cylinder (radius 10 about Z, 50 long), turned onto the rotary
/// axis and centred on it, tessellated at `linear` mm.
fn cylinder(linear: f64) -> BrepModel {
    let bytes = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/step/valid/cylinder.step"),
    )
    .unwrap();
    let cancel = CancelToken::new();
    let imported = StepImporter::new()
        .import(
            &bytes,
            &ImportOptions {
                tessellation: Some(TessellationSettings {
                    linear,
                    ..TessellationSettings::default()
                }),
                ..ImportOptions::default()
            },
            &JobContext::new(&cancel, &NoProgress),
        )
        .unwrap();
    let ModelGeometry::Brep(model) = imported.geometry else {
        panic!("not a B-rep");
    };
    let transform = [
        TransformStep::Rotate {
            axis: Axis::Y,
            angle: Degrees(90.0),
        },
        TransformStep::CentreOnRotaryAxis,
    ];
    let map = brep_placement(&model, &transform).unwrap();
    transform_brep(&model, &map).unwrap()
}

fn ball(diameter: f64) -> Outline {
    let tool = Tool {
        shape: ToolShape::BallEnd,
        diameter: Millimetres(diameter),
        flute_length: Millimetres(8.0),
        overall_length: Millimetres(40.0),
        shank_diameter: Millimetres(diameter),
        flute_count: 2,
        material: None,
        centre_cutting: Some(true),
    };
    Outline::new(&tool.profile().unwrap())
}

/// The largest sagitta of the cylinder's tessellation, from its vertices
/// alone: each triangle of the side face spans an arc between the polar
/// angles of its corners, and its chord lies `r (1 - cos(span / 2))` inside.
fn widest_sagitta(model: &BrepModel, radius: f64) -> f64 {
    let mut worst: f64 = 0.0;
    for face in model
        .faces
        .iter()
        .filter(|f| f.surface == openchuck_types::brep::SurfaceKind::Cylinder)
    {
        let mesh = &face.tessellation;
        for t in &mesh.triangles {
            let angles: Vec<f64> = t
                .iter()
                .map(|&i| {
                    let p = mesh.vertices[i as usize];
                    p.z.atan2(p.y)
                })
                .collect();
            let radial = t.iter().all(|&i| {
                let p = mesh.vertices[i as usize];
                (p.y.hypot(p.z) - radius).abs() < 1e-4
            });
            if !radial {
                continue;
            }
            let mut span: f64 = 0.0;
            for a in &angles {
                for b in &angles {
                    let d = (a - b).rem_euclid(std::f64::consts::TAU);
                    span = span.max(d.min(std::f64::consts::TAU - d));
                }
            }
            worst = worst.max(radius * (1.0 - (span / 2.0).cos()));
        }
    }
    worst
}

#[test]
fn req_imp_06_the_tessellation_error_is_measured_on_the_triangles_not_taken_from_the_setting() {
    for linear in [0.01, 0.0025] {
        let model = cylinder(linear);
        let report = measure::tessellation_error(&model, 4);
        let side = report
            .iter()
            .find(|r| r.kind == "cylinder")
            .expect("the side face");
        let expected = widest_sagitta(&model, 10.0);
        // The grid of each triangle holds the middle of its long edge only
        // to within the grid's step, which on a chord of length c misses at
        // most the sagitta's change over c / 8.
        assert!(
            side.max <= expected + 1e-9 && side.max >= 0.95 * expected,
            "{linear}: measured {} against the sagitta {expected}",
            side.max
        );
        assert!(side.p99 <= side.max && side.samples > side.triangles);
        // The closed form, from the face's data alone, reads the same.
        let closed = side.closed_form.unwrap();
        assert!((closed - side.max).abs() < 1e-9, "{closed}");
        // The caps' triangles lie in their planes: the planes' error is the
        // evaluator's. Measured against the trimmed face, a point by the rim
        // may lie outside the trim polyline, so that reading is no smaller.
        let planes = report.iter().find(|r| r.kind == "plane").unwrap();
        assert!(planes.max < 1e-9, "{}", planes.max);
        assert!(planes.trimmed >= planes.max);
        // The facet normals of a cylinder turn by half a triangle's span.
        assert!(side.normal_degrees > 0.0 && side.normal_degrees < 10.0);
        println!(
            "cylinder at {linear}: {} triangles, deepest {:.6} mm (sagitta {expected:.6})",
            side.triangles, side.max
        );
    }
}

#[test]
fn req_op_12_a_pose_reads_its_gouge_and_its_gap_against_the_exact_faces() {
    let model = cylinder(0.01);
    let outline = ball(3.0);
    let exact = Cloud::of_faces(&model, 0.02);
    // A ball dropped radially along +Y, its tip `depth` inside the cylinder.
    for (depth, expected) in [(0.02, -0.02), (0.0, 0.0), (-0.05, 0.05)] {
        let tip = Point3::new(25.0, 10.0 - depth, 0.0);
        let c = exact.clearance(&outline, tip, Vec3::new(0.0, 1.0, 0.0), 8.0, 0.5);
        // Samples 0.02 mm apart on a 1.5 mm ball miss at most 0.0001 mm.
        assert!((c - expected).abs() < 2e-4, "{depth}: {c}");
    }
    // Far from the part the clearance is the margin.
    let c = exact.clearance(
        &outline,
        Point3::new(25.0, 13.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
        8.0,
        0.5,
    );
    assert!((c - 0.5).abs() < 1e-12, "{c}");
}

#[test]
fn req_op_12_a_moves_middle_and_quarters_lie_on_the_machines_straight_move() {
    // The Z1's mapping: the rotary axis along X, the tool along machine Z.
    let rotary =
        RotaryMapping::of_profile(&openchuck_app::setup::builtin("makera-z1").unwrap().unwrap());
    let model = cylinder(0.01);
    let outline = ball(3.0);
    let exact = Cloud::of_faces(&model, 0.02);
    // The machine holds X, Y and Z while A turns 20 degrees: the tool's tip
    // stays on the cylinder in the part's frame, whereas the straight line
    // between the two model-frame tips would cut 0.152 mm into it at the
    // middle.
    let pose = |a: f64| {
        let tool = rotary.tool_axis(openchuck_types::units::Degrees(a));
        ToolPose {
            tip: Point3::ORIGIN + tool.get() * 10.0 + Vec3::new(25.0, 0.0, 0.0),
            axis: tool,
            a: openchuck_types::units::Degrees(a),
        }
    };
    let (start, end) = (pose(0.0), pose(20.0));
    let stations = Station::along(&rotary, &start, &end);
    assert_eq!(stations.len(), 3);
    for s in &stations {
        let r = s.tip.y.hypot(s.tip.z);
        assert!((r - 10.0).abs() < 1e-9, "{s:?}");
        let c = exact.clearance(&outline, s.tip, s.axis, 8.0, 0.5);
        assert!(c.abs() < 2e-4, "{s:?}: {c}");
    }
    let middle = Point3::new(
        25.0,
        (start.tip.y + end.tip.y) / 2.0,
        (start.tip.z + end.tip.z) / 2.0,
    );
    assert!(10.0 - middle.y.hypot(middle.z) > 0.15);
}

#[test]
fn req_op_12_a_mesh_is_sampled_by_its_area_and_read_like_the_exact_faces() {
    use openchuck_types::mesh::TriangleMesh;
    // A sliver 50 mm long and 0.5 mm wide: about its area over the spacing
    // squared, not the square of its length.
    let sliver = TriangleMesh {
        vertices: vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(50.0, 0.0, 0.0),
            Point3::new(25.0, 0.5, 0.0),
        ],
        triangles: vec![[0, 1, 2]],
    };
    let cloud = Cloud::of_mesh(&sliver, 0.05);
    let area_points = 12.5 / (0.05 * 0.05);
    assert!(
        (cloud.len() as f64) < 1.5 * area_points + 2000.0,
        "{} points",
        cloud.len()
    );
    // A ball standing on it reads its height above the plane.
    let outline = ball(3.0);
    for (height, expected) in [(0.0, 0.0), (0.03, 0.03), (-0.02, -0.02)] {
        let c = cloud.clearance(
            &outline,
            Point3::new(25.0, 0.1, height),
            Vec3::new(0.0, 0.0, 1.0),
            8.0,
            0.5,
        );
        assert!((c - expected).abs() < 1e-3, "{height}: {c}");
    }
    // The cylinder's tessellation lies inside its exact face by its sagitta
    // at most, so a ball on the exact face clears the tessellation by no more.
    let model = cylinder(0.01);
    let mesh = &model
        .faces
        .iter()
        .find(|f| f.surface == openchuck_types::brep::SurfaceKind::Cylinder)
        .unwrap()
        .tessellation;
    let tess = Cloud::of_mesh(mesh, 0.02);
    let c = tess.clearance(
        &outline,
        Point3::new(25.0, 10.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
        8.0,
        0.5,
    );
    assert!((0.0..=0.0032).contains(&c), "{c}");
}
