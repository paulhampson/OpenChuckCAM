// SPDX-License-Identifier: AGPL-3.0-or-later
//! Issue #346 / decision 0209 M2: the measurement's tool-contact check is
//! bounded. Decision 0211's flat-cutter counterexample is kept as a
//! regression: the sampled check it replaced misses 0.0067 mm there, and M2
//! must detect the exact -0.01 mm before it may claim a pass. A station
//! whose bound cannot be closed fails M2. No STEP kernel is needed.

#![allow(
    clippy::unwrap_used,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    dead_code
)]

#[path = "../examples/step_accuracy/bounded.rs"]
mod bounded;
#[path = "../examples/step_accuracy/checks.rs"]
mod checks;
#[path = "../examples/step_accuracy/sampled.rs"]
mod sampled;

use openchuck_testkit::contact::{ConvexTool, Geometry, Settings};
use openchuck_testkit::gouge::Outline;
use openchuck_types::brep::{BrepFace, BrepModel, FaceId, Frame, Surface, SurfaceKind};
use openchuck_types::geometry::{Point3, Vec3};
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::tool::{RevolvedProfile, Tool, ToolShape};
use openchuck_types::units::Millimetres;

/// Decision 0211's plane: z = x - 0.98, 0.95 <= x <= 1.05, 0 <= y <= 0.05,
/// with its two-triangle tessellation.
fn counterexample() -> BrepModel {
    let s = 0.5_f64.sqrt();
    let face = BrepFace {
        id: FaceId(0),
        surface: SurfaceKind::Plane,
        body: 0,
        edges: vec![],
        reversed: false,
        geometry: Surface::Plane {
            frame: Frame {
                origin: Point3::new(0.95, 0.0, -0.03),
                x: Vec3::new(s, 0.0, s),
                y: Vec3::new(0.0, 1.0, 0.0),
                z: Vec3::new(-s, 0.0, s),
            },
        },
        uv_bounds: [0.0, 0.1 * 2.0_f64.sqrt(), 0.0, 0.05],
        trims: vec![],
        tessellation: TriangleMesh {
            vertices: vec![
                Point3::new(0.95, 0.0, -0.03),
                Point3::new(1.05, 0.0, 0.07),
                Point3::new(1.05, 0.05, 0.07),
                Point3::new(0.95, 0.05, -0.03),
            ],
            triangles: vec![[0, 1, 2], [0, 2, 3]],
        },
    };
    BrepModel {
        faces: vec![face],
        ..BrepModel::default()
    }
}

/// Pencil's 2 mm flat end mill.
fn flat2() -> RevolvedProfile {
    Tool {
        shape: ToolShape::FlatEnd,
        diameter: Millimetres(2.0),
        flute_length: Millimetres(8.0),
        overall_length: Millimetres(40.0),
        shank_diameter: Millimetres(2.0),
        flute_count: 2,
        material: None,
        centre_cutting: Some(true),
    }
    .profile()
    .unwrap()
}

fn stop(place: bounded::Place) -> bounded::Stop {
    bounded::Stop {
        tip: Point3::ORIGIN,
        axis: Vec3::new(0.0, 0.0, 1.0),
        place,
        cut: true,
    }
}

#[test]
fn req_op_12_m2_detects_the_flat_cutter_counterexample_the_samples_missed() {
    let model = counterexample();
    // The sampled check of decision 0211, as M2 used it: misses 0.0067 mm.
    let outline = Outline::new(&flat2());
    let cloud = sampled::Cloud::of_faces(&model, 0.05);
    let sampled = cloud.clearance(&outline, Point3::ORIGIN, Vec3::new(0.0, 0.0, 1.0), 8.0, 1.0);
    assert!(sampled + 0.01 > 0.006, "the old samples now read {sampled}");
    // The bounded M2 reads the exact -0.01 mm, on the faces and on their
    // tessellation, at a pose and at a quarter point, and qualifies.
    let exact = Geometry::of_faces(&model).unwrap();
    let mesh = Geometry::of_mesh(&model.faces[0].tessellation).unwrap();
    let tool = ConvexTool::new(&flat2()).unwrap();
    let stops = [stop(bounded::Place::Pose), stop(bounded::Place::Quarter(2))];
    let observations =
        bounded::observe(&exact, &mesh, &tool, &stops, &bounded::settings()).unwrap();
    let m = bounded::summarise(&observations, 0.0);
    assert_eq!(m.stations, 2);
    assert!((m.deepest - 0.01).abs() <= bounded::TOLERANCE, "{m:?}");
    assert!(m.deepest_bound >= 0.01 - 1e-9, "{m:?}");
    assert!((m.deepest_at_poses - 0.01).abs() <= bounded::TOLERANCE);
    assert!(m.proven_error <= bounded::TOLERANCE + 1e-9);
    assert!(checks::sampling_bound(Some(m.proven_error)).is_ok());
    // The tessellation is this plane exactly, so the drop error is nil.
    assert!(m.error_in.abs() <= bounded::TOLERANCE && m.error_out.abs() <= bounded::TOLERANCE);
}

#[test]
fn req_op_12_m2_fails_a_station_it_cannot_bound() {
    let model = counterexample();
    let exact = Geometry::of_faces(&model).unwrap();
    let mesh = Geometry::of_mesh(&model.faces[0].tessellation).unwrap();
    let tool = ConvexTool::new(&flat2()).unwrap();
    let tight = Settings {
        budget: 3,
        ..bounded::settings()
    };
    let stops = [stop(bounded::Place::Pose), stop(bounded::Place::Quarter(1))];
    let error = bounded::observe(&exact, &mesh, &tool, &stops, &tight).unwrap_err();
    assert!(error.contains("station 0"), "{error}");
    // A concave tool has no bound, so M2 cannot be run with it.
    assert!(checks::sampling_bound(None).is_err());
}

#[test]
fn req_op_12_m2_keeps_raw_observations_with_their_provenance() {
    let model = counterexample();
    let exact = Geometry::of_faces(&model).unwrap();
    let mesh = Geometry::of_mesh(&model.faces[0].tessellation).unwrap();
    let tool = ConvexTool::new(&flat2()).unwrap();
    let stops = [stop(bounded::Place::Pose), stop(bounded::Place::Quarter(3))];
    let observations =
        bounded::observe(&exact, &mesh, &tool, &stops, &bounded::settings()).unwrap();
    let text = bounded::raw("source: abc\npart: plane", &observations);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[0], "# source: abc");
    assert_eq!(lines[1], "# part: plane");
    assert!(lines[2].starts_with("station\tplace\tcut"));
    assert_eq!(lines.len(), 5);
    let fields: Vec<&str> = lines[4].split('\t').collect();
    assert_eq!(fields[1], "q3");
    let upper: f64 = fields[9].parse().unwrap();
    let lower: f64 = fields[10].parse().unwrap();
    assert!(lower <= -0.01 + 1e-9 && upper >= -0.01 - 1e-9);
}
