// SPDX-License-Identifier: AGPL-3.0-or-later
//! The STEP importer against the corpus (work breakdown slice 11 items 1 to 4 and 8;
//! decisions 0003 and 0017). Needs the OpenCASCADE kernel: `--features kernel`.
//!
//! Every file in `testdata/step` was written by OpenCASCADE itself, so these
//! tests show the importer reads what the kernel writes, not that it reads
//! every CAD tool's STEP (OI-25).
#![cfg(feature = "kernel")]
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};
use std::time::Duration;

use openchuck_import::step::StepImporter;
use openchuck_import::{ImportError, ImportOptions, ImportedModel, ModelImporter, UnitSource};
use openchuck_types::brep::{BrepModel, Surface, SurfaceKind, TessellationSettings};
use openchuck_types::geometry::{Point3, Vec3};
use openchuck_types::job::{CancelToken, JobContext, NoProgress};
use openchuck_types::model::ModelGeometry;
use openchuck_types::units::LengthUnit;

fn testdata(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/step")
        .join(name)
}

fn importer() -> StepImporter {
    StepImporter::new().with_worker(env!("CARGO_BIN_EXE_openchuck-step-worker"))
}

fn import_with(
    name: &str,
    options: &ImportOptions,
    importer: &StepImporter,
) -> Result<ImportedModel, ImportError> {
    let bytes = std::fs::read(testdata(name)).unwrap();
    let (cancel, progress) = (CancelToken::new(), NoProgress);
    importer.import(&bytes, options, &JobContext::new(&cancel, &progress))
}

fn import(name: &str) -> ImportedModel {
    import_with(name, &ImportOptions::default(), &importer()).unwrap()
}

fn options_at(linear: f64, angular: f64) -> ImportOptions {
    ImportOptions {
        tessellation: Some(TessellationSettings { linear, angular }),
        ..ImportOptions::default()
    }
}

fn brep(model: &ImportedModel) -> &BrepModel {
    match &model.geometry {
        ModelGeometry::Brep(b) => b,
        ModelGeometry::Mesh(_) => panic!("expected a B-rep"),
    }
}

fn extent(b: &BrepModel) -> Vec3 {
    let pts: Vec<Point3> = b.vertices.iter().map(|v| v.point).collect();
    let min = |f: fn(&Point3) -> f64| pts.iter().map(f).fold(f64::INFINITY, f64::min);
    let max = |f: fn(&Point3) -> f64| pts.iter().map(f).fold(f64::NEG_INFINITY, f64::max);
    Vec3::new(
        max(|p| p.x) - min(|p| p.x),
        max(|p| p.y) - min(|p| p.y),
        max(|p| p.z) - min(|p| p.z),
    )
}

fn triangle_area(t: [Point3; 3]) -> f64 {
    0.5 * (t[1] - t[0]).cross(t[2] - t[0]).length()
}

fn mesh_area(b: &BrepModel) -> f64 {
    b.faces
        .iter()
        .flat_map(|f| {
            (0..f.tessellation.triangles.len()).filter_map(|i| f.tessellation.triangle(i))
        })
        .map(triangle_area)
        .sum()
}

fn triangle_count(b: &BrepModel) -> usize {
    b.faces.iter().map(|f| f.tessellation.triangles.len()).sum()
}

#[test]
fn req_imp_02_box_matches_its_cad_dimensions() {
    let model = import("valid/box.step");
    let b = brep(&model);
    // A 20 x 30 x 40 mm box: six planes, twelve edges, eight vertices.
    assert_eq!((b.faces.len(), b.edges.len(), b.vertices.len()), (6, 12, 8));
    assert!(b.faces.iter().all(|f| f.surface == SurfaceKind::Plane));
    let size = extent(b);
    for (got, want) in [(size.x, 20.0), (size.y, 30.0), (size.z, 40.0)] {
        assert!((got - want).abs() < 1e-9, "{got} against {want}");
    }
    // Surface area 2 (20*30 + 20*40 + 30*40), from the tessellation of planar faces.
    assert!((mesh_area(b) - 5200.0).abs() < 1e-6, "{}", mesh_area(b));
    // Each edge joins two faces, each face has four edges.
    assert!(
        b.edges
            .iter()
            .all(|e| e.faces.len() == 2 && e.vertices.is_some())
    );
    assert!(b.faces.iter().all(|f| f.edges.len() == 4 && f.body == 0));
    assert_eq!(model.units.applied, LengthUnit::Millimetre);
    assert_eq!(model.units.source, UnitSource::File);
}

#[test]
fn req_imp_02_analytic_faces_carry_their_exact_parameters() {
    let cyl = import("valid/cylinder.step");
    let b = brep(&cyl);
    let side = b
        .faces
        .iter()
        .find(|f| f.surface == SurfaceKind::Cylinder)
        .unwrap();
    let Surface::Cylinder { radius, frame } = &side.geometry else {
        panic!("not a cylinder")
    };
    assert_eq!(*radius, 10.0);
    // The axis is Z for this fixture, and the frame is orthonormal.
    assert!((frame.z.z.abs() - 1.0).abs() < 1e-12);

    let sphere = import("valid/sphere.step");
    let Surface::Sphere { radius, .. } = &brep(&sphere).faces[0].geometry else {
        panic!("not a sphere")
    };
    assert_eq!(*radius, 15.0);

    let torus = import("valid/torus.step");
    let Surface::Torus { major, minor, .. } = &brep(&torus).faces[0].geometry else {
        panic!("not a torus")
    };
    assert_eq!((*major, *minor), (20.0, 10.0));

    let cone = import("valid/cone.step");
    let found = brep(&cone)
        .faces
        .iter()
        .find_map(|f| match &f.geometry {
            Surface::Cone {
                radius, semi_angle, ..
            } => Some((*radius, *semi_angle)),
            _ => None,
        })
        .unwrap();
    assert_eq!(found.0, 12.0);
    assert!((found.1 - 0.260_602_391_747).abs() < 1e-12);
}

#[test]
fn req_imp_02_every_face_has_uv_bounds_trims_and_a_tessellation() {
    for name in [
        "box",
        "cylinder",
        "cone",
        "sphere",
        "torus",
        "filleted_box",
        "rotary_blank_drilled",
        "bspline_face",
    ] {
        let model = import(&format!("valid/{name}.step"));
        let b = brep(&model);
        b.validate().unwrap();
        for f in &b.faces {
            assert!(
                !f.tessellation.triangles.is_empty(),
                "{name} face {}",
                f.id.0
            );
            assert!(!f.trims.is_empty(), "{name} face {}: no trim loop", f.id.0);
            assert!(f.uv_bounds[0] < f.uv_bounds[1] && f.uv_bounds[2] < f.uv_bounds[3]);
        }
    }
}

#[test]
fn req_imp_02_a_drilled_cylinder_has_a_hole_loop() {
    let model = import("valid/rotary_blank_drilled.step");
    let b = brep(&model);
    // The radial hole cuts the cylinder face: two trim loops (outer boundary and the hole).
    assert!(
        b.faces.iter().any(|f| f.trims.len() == 2),
        "no face with a hole"
    );
}

#[test]
fn req_imp_02_every_nonanalytic_face_is_a_clamped_patch() {
    for name in [
        "bspline_face",
        "revolved_spline",
        "revolved_spline_partial",
        "extruded_spline",
    ] {
        let model = import(&format!("valid/{name}.step"));
        let b = brep(&model);
        let mut patches = 0;
        for f in &b.faces {
            if let Surface::BSpline(p) = &f.geometry {
                patches += 1;
                p.validate().unwrap();
                // Clamped: the end knots repeat degree + 1 times, so the patch is not periodic.
                let (du, dv) = (p.degree_u as usize, p.degree_v as usize);
                assert!(
                    p.knots_u[..=du].iter().all(|k| *k == p.knots_u[0]),
                    "{name}"
                );
                assert!(
                    p.knots_u[p.knots_u.len() - 1 - du..]
                        .iter()
                        .all(|k| *k == p.knots_u[p.knots_u.len() - 1])
                );
                assert!(
                    p.knots_v[..=dv].iter().all(|k| *k == p.knots_v[0]),
                    "{name}"
                );
                assert!(
                    p.knots_v[p.knots_v.len() - 1 - dv..]
                        .iter()
                        .all(|k| *k == p.knots_v[p.knots_v.len() - 1])
                );
                // The face's parameters lie inside the patch's domain.
                let (u0, u1) = (p.knots_u[0], p.knots_u[p.knots_u.len() - 1]);
                let (v0, v1) = (p.knots_v[0], p.knots_v[p.knots_v.len() - 1]);
                let eps = 1e-9;
                assert!(
                    f.uv_bounds[0] >= u0 - eps && f.uv_bounds[1] <= u1 + eps,
                    "{name} u"
                );
                assert!(
                    f.uv_bounds[2] >= v0 - eps && f.uv_bounds[3] <= v1 + eps,
                    "{name} v"
                );
            }
        }
        assert!(patches > 0, "{name} has no B-spline face");
    }
    // A surface of revolution reads as `other` in the file and as a patch in the data.
    let vase = import("valid/revolved_spline.step");
    assert!(
        brep(&vase)
            .faces
            .iter()
            .any(|f| f.surface == SurfaceKind::Other && matches!(f.geometry, Surface::BSpline(_)))
    );
}

#[test]
fn req_imp_02_ids_are_identical_across_two_imports_and_a_save() {
    for name in [
        "box",
        "filleted_box",
        "rotary_blank_drilled",
        "sphere",
        "bspline_face",
    ] {
        let first = import(&format!("valid/{name}.step"));
        let second = import(&format!("valid/{name}.step"));
        // Two worker processes, one answer: IDs, geometry and tessellation.
        assert_eq!(brep(&first), brep(&second), "{name}");
        // Saving and loading the data (the JSON form the project stores) keeps every ID.
        let text = serde_json::to_string(brep(&first)).unwrap();
        let back: BrepModel = serde_json::from_str(&text).unwrap();
        assert_eq!(&back, brep(&first), "{name}");
    }
}

#[test]
fn req_imp_02_face_and_edge_ids_are_their_positions() {
    let model = import("valid/filleted_box.step");
    let b = brep(&model);
    // A 26-face part: IDs run 0..n in order, and every reference is in range.
    assert_eq!(b.faces.len(), 26);
    for (i, f) in b.faces.iter().enumerate() {
        assert_eq!(f.id.0 as usize, i);
        assert!(f.edges.windows(2).all(|w| w[0] != w[1]));
    }
    for (i, e) in b.edges.iter().enumerate() {
        assert_eq!(e.id.0 as usize, i);
        assert!(
            e.faces.windows(2).all(|w| w[0] < w[1]),
            "faces listed once, ascending"
        );
    }
}

#[test]
fn req_imp_06_tolerance_controls_triangle_count_and_deviation() {
    let mut counts = Vec::new();
    let mut worst = Vec::new();
    for (linear, angular) in [(0.5, 0.5), (0.1, 0.25), (0.01, 0.1)] {
        let model = import_with(
            "valid/cylinder.step",
            &options_at(linear, angular),
            &importer(),
        )
        .unwrap();
        let b = brep(&model);
        assert_eq!(b.tessellation, TessellationSettings { linear, angular });
        counts.push(triangle_count(b));
        // Deviation of the side face's triangles from the exact radius-10 cylinder about Z:
        // the triangle centroid is the farthest point of a chord from the surface.
        let side = b
            .faces
            .iter()
            .find(|f| f.surface == SurfaceKind::Cylinder)
            .unwrap();
        let mut dev = 0.0_f64;
        for i in 0..side.tessellation.triangles.len() {
            let t = side.tessellation.triangle(i).unwrap();
            let c = Point3::new(
                (t[0].x + t[1].x + t[2].x) / 3.0,
                (t[0].y + t[1].y + t[2].y) / 3.0,
                0.0,
            );
            dev = dev.max((10.0 - c.to_vec().length()).abs());
            // Every vertex is on the surface.
            for p in t {
                assert!((p.x.hypot(p.y) - 10.0).abs() < 1e-9);
            }
        }
        worst.push(dev);
    }
    // Finer settings give strictly more triangles and a smaller worst deviation.
    assert!(counts[0] < counts[1] && counts[1] < counts[2], "{counts:?}");
    assert!(worst[0] > worst[1] && worst[1] > worst[2], "{worst:?}");
    // The kernel's tolerance is not a bound (decision 0003): 0.01 mm can give more. The recorded
    // behaviour is a deviation under twice the setting on this part.
    assert!(worst[2] < 0.02, "{worst:?}");
}

#[test]
fn req_imp_06_triangles_wind_outward_and_belong_to_one_face() {
    // A convex part: every triangle's normal points away from the centre of the shape.
    let model = import("valid/sphere.step");
    let b = brep(&model);
    let (joined, face_of_triangle) = b.joined_tessellation();
    assert_eq!(face_of_triangle.len(), joined.triangles.len());
    for i in 0..joined.triangles.len() {
        let t = joined.triangle(i).unwrap();
        let n = (t[1] - t[0]).cross(t[2] - t[0]);
        let centroid = Point3::new(
            (t[0].x + t[1].x + t[2].x) / 3.0,
            (t[0].y + t[1].y + t[2].y) / 3.0,
            (t[0].z + t[1].z + t[2].z) / 3.0,
        );
        // The sphere fixture is centred on the origin. A sliver at the pole has no direction.
        if n.length() > 1e-9 {
            assert!(n.dot(centroid.to_vec()) > 0.0, "triangle {i} faces inward");
        }
    }
    let bx = import("valid/box.step");
    let bb = brep(&bx);
    let centre = Point3::new(10.0, 15.0, 20.0);
    for f in &bb.faces {
        for i in 0..f.tessellation.triangles.len() {
            let t = f.tessellation.triangle(i).unwrap();
            let n = (t[1] - t[0]).cross(t[2] - t[0]);
            assert!(
                n.dot(t[0] - centre) > 0.0,
                "box face {} faces inward",
                f.id.0
            );
        }
    }
}

#[test]
fn req_imp_03_a_file_in_inches_is_converted_and_can_be_overridden() {
    let model = import("valid/box_inch.step");
    // The file says inches; the model is in millimetres: 1 in = 25.4 mm.
    assert_eq!(model.units.file, Some(LengthUnit::Inch));
    assert_eq!(model.units.applied, LengthUnit::Inch);
    assert_eq!(model.units.source, UnitSource::File);
    let size = extent(brep(&model));
    assert!(
        (size.x - 25.4).abs() < 1e-9
            && (size.y - 25.4).abs() < 1e-9
            && (size.z - 25.4).abs() < 1e-9
    );

    // The user says the numbers are millimetres: the file's 1.0 becomes 1 mm.
    let as_mm = ImportOptions {
        units: Some(LengthUnit::Millimetre),
        ..ImportOptions::default()
    };
    let model = import_with("valid/box_inch.step", &as_mm, &importer()).unwrap();
    assert_eq!(model.units.source, UnitSource::User);
    assert_eq!(model.units.file, Some(LengthUnit::Inch));
    let size = extent(brep(&model));
    assert!(
        (size.x - 1.0).abs() < 1e-9 && (size.z - 1.0).abs() < 1e-9,
        "{size:?}"
    );
    // The tessellation was made after the scaling, so its tolerance is in true millimetres.
    let b = brep(&model);
    b.validate().unwrap();
    assert!((mesh_area(b) - 6.0).abs() < 1e-9);

    // A millimetre file read as centimetres is ten times larger.
    let as_cm = ImportOptions {
        units: Some(LengthUnit::Centimetre),
        ..ImportOptions::default()
    };
    let model = import_with("valid/box.step", &as_cm, &importer()).unwrap();
    let size = extent(brep(&model));
    assert!(
        (size.x - 200.0).abs() < 1e-9 && (size.z - 400.0).abs() < 1e-9,
        "{size:?}"
    );
}

#[test]
fn req_imp_02_separate_solids_are_separate_bodies() {
    let model = import("valid/multi_body.step");
    let b = brep(&model);
    let faces_of = |body: u32| b.faces.iter().filter(|f| f.body == body).count();
    // A box (6 faces), a cylinder (3) and a sphere (1).
    assert_eq!((faces_of(0), faces_of(1), faces_of(2)), (6, 3, 1));
    assert_eq!(b.faces.len(), 10);
    assert!(b.faces.iter().all(|f| f.body < 3));
    // Bodies are numbered in file order and faces keep that order.
    assert!(b.faces.windows(2).all(|w| w[0].body <= w[1].body));
}

#[test]
fn req_nfr_07_malformed_files_give_errors_not_crashes() {
    for name in [
        "empty",
        "garbage",
        "header_only",
        "truncated_half",
        "dangling_ref",
        "unknown_entity",
    ] {
        let result = import_with(
            &format!("broken/{name}.step"),
            &ImportOptions::default(),
            &importer(),
        );
        let err = result
            .err()
            .unwrap_or_else(|| panic!("{name} was accepted"));
        assert!(
            matches!(err, ImportError::Invalid { .. } | ImportError::Worker(_)),
            "{name}: {err}"
        );
        assert!(!err.to_string().is_empty());
    }
}

#[test]
fn req_nfr_07_an_unknown_entity_gives_an_empty_shell_and_that_is_an_error() {
    // SP-1: the reader loads such a file with exit 0 and no faces; zero faces must be an error.
    let err = import_with(
        "broken/unknown_entity.step",
        &ImportOptions::default(),
        &importer(),
    )
    .err()
    .unwrap();
    assert!(
        err.to_string().to_lowercase().contains("no faces")
            || matches!(err, ImportError::Invalid { .. }),
        "{err}"
    );
}

#[test]
fn req_nfr_07_files_that_crash_the_reader_do_not_crash_the_application() {
    // Twelve mutants that segfaulted the OpenCASCADE reader in SP-1 (decision 0003).
    let mut crashes = 0;
    let mut seen = 0;
    for entry in std::fs::read_dir(testdata("crash")).unwrap() {
        let path = entry.unwrap().path();
        seen += 1;
        let bytes = std::fs::read(&path).unwrap();
        let (cancel, progress) = (CancelToken::new(), NoProgress);
        let result = importer().import(
            &bytes,
            &ImportOptions::default(),
            &JobContext::new(&cancel, &progress),
        );
        // Here, with this process still alive, the reader either failed cleanly or was caught.
        if let Err(ImportError::Worker(_)) = &result {
            crashes += 1;
        }
        assert!(result.is_err(), "{} was accepted", path.display());
    }
    assert!(seen >= 12);
    // Not every platform's kernel segfaults on every file, but the test is only meaningful if
    // the process boundary was exercised somewhere: report how many were caught.
    eprintln!("{crashes} of {seen} crash files were caught by the process boundary");
}

#[test]
fn req_nfr_07_a_worker_that_takes_too_long_is_stopped() {
    let slow = importer().with_timeout(Duration::ZERO);
    let err = import_with("valid/filleted_box.step", &ImportOptions::default(), &slow)
        .err()
        .unwrap();
    assert!(
        matches!(&err, ImportError::Worker(m) if m.contains("did not finish")),
        "{err}"
    );
}

#[test]
fn req_nfr_13_the_worker_exits_cleanly() {
    // Decision 0003: with the toolkits linked in the wrong order, the process finished its work and
    // then segfaulted in static destruction on macOS and Windows. The worker ends by returning from
    // main so that destruction runs; its exit status is the test of the link order.
    let dir = std::env::temp_dir().join(format!("openchuck-step-exit-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::copy(testdata("valid/cylinder.step"), dir.join("in.step")).unwrap();
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_openchuck-step-worker"))
        .args(["in.step", "out.json", "0.1", "0.5", "0"])
        .current_dir(&dir)
        .status()
        .unwrap();
    let wrote = dir.join("out.json").is_file();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(status.success(), "the worker ended with {status}");
    assert!(wrote);
}

#[test]
fn req_nfr_13_the_worker_reports_a_bad_file_with_exit_code_two() {
    let dir = std::env::temp_dir().join(format!("openchuck-step-bad-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::copy(testdata("broken/garbage.step"), dir.join("in.step")).unwrap();
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_openchuck-step-worker"))
        .args(["in.step", "out.json", "0.1", "0.5", "0"])
        .current_dir(&dir)
        .status()
        .unwrap();
    let text = std::fs::read_to_string(dir.join("out.json")).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(status.code(), Some(2));
    assert!(text.contains("\"error\""), "{text}");
}

#[test]
fn req_nfr_06_the_answer_is_byte_identical_between_runs() {
    let run = || {
        let dir = std::env::temp_dir().join(format!(
            "openchuck-step-det-{}-{}",
            std::process::id(),
            rand_suffix()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::copy(testdata("valid/filleted_box.step"), dir.join("in.step")).unwrap();
        let ok = std::process::Command::new(env!("CARGO_BIN_EXE_openchuck-step-worker"))
            .args(["in.step", "out.json", "0.05", "0.2", "0"])
            .current_dir(&dir)
            .status()
            .unwrap()
            .success();
        let bytes = std::fs::read(dir.join("out.json")).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(ok);
        bytes
    };
    assert_eq!(run(), run());
}

fn rand_suffix() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
}
