// SPDX-License-Identifier: AGPL-3.0-or-later
//! The import dialog's model: checking a file before it is added, with the
//! problems the checks list highlighted, and adding it with its placement as
//! one undoable step (IMP-03, IMP-04, IMP-05; OI-51; decision record 0029
//! item 13).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};

use openchuck_app::model::{ImportPreview, LengthUnit, TransformStep};
use openchuck_app::scene::SceneRole;
use openchuck_app::{Application, Threads};

fn stl(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/stl")
        .join(name)
}

fn app(name: &str) -> (PathBuf, Application) {
    let dir = std::env::temp_dir().join(format!("openchuck-import-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    (
        dir.clone(),
        Application::create(&dir, name, Threads::Exactly(1)).unwrap(),
    )
}

fn preview(
    app: &Application,
    file: &str,
    units: Option<LengthUnit>,
    steps: &[TransformStep],
) -> ImportPreview {
    app.start_import_preview(&stl(file), units, steps)
        .wait()
        .unwrap()
}

fn problems(p: &ImportPreview) -> (usize, usize) {
    let triangles = p
        .scene
        .meshes
        .iter()
        .filter(|m| m.role == SceneRole::Problem)
        .map(|m| m.mesh.triangles.len())
        .sum();
    let edges = p
        .scene
        .lines
        .iter()
        .filter(|l| l.role == SceneRole::Problem)
        .count();
    (triangles, edges)
}

#[test]
fn req_imp_04_import_preview_highlights_what_the_checks_list() {
    let (dir, app) = app("highlight");
    // A clean cube: the part and the axis, nothing highlighted, and the STL
    // tessellation statement.
    let clean = preview(&app, "cube_ascii.stl", None, &[]);
    assert!(clean.inspection.report.is_clean());
    assert_eq!(problems(&clean), (0, 0));
    assert_eq!(clean.scene.meshes[0].role, SceneRole::Part);
    assert!(
        clean
            .inspection
            .notes
            .iter()
            .any(|n| n.contains("tessellation")),
        "{:?}",
        clean.inspection.notes
    );
    assert!(
        clean
            .scene
            .lines
            .iter()
            .any(|l| l.role == SceneRole::RotaryAxis)
    );

    // A hole: its open edges are drawn.
    let hole = preview(&app, "defect_hole.stl", None, &[]);
    assert_eq!(problems(&hole).1, hole.inspection.report.boundary_edges());
    assert!(problems(&hole).1 > 0);
    // A flipped triangle: drawn as a problem mesh.
    let flipped = preview(&app, "defect_flipped.stl", None, &[]);
    assert_eq!(
        problems(&flipped).0,
        flipped.inspection.report.flipped_triangles.len()
    );
    assert!(problems(&flipped).0 > 0);
    // Non-manifold edges.
    let nm = preview(&app, "defect_non_manifold.stl", None, &[]);
    assert!(problems(&nm).1 >= nm.inspection.report.non_manifold_edges.len());
    assert!(problems(&nm).1 > 0);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn req_imp_03_req_imp_05_preview_follows_units_and_placement() {
    let (dir, app) = app("place");
    // An inch file asks about units; reading it in inches scales it by 25.4.
    let asked = preview(&app, "cube_inch.stl", None, &[]);
    assert!(asked.inspection.unit_question.is_some());
    let mm = asked.inspection.summary.bounds.unwrap();
    let inch = preview(&app, "cube_inch.stl", Some(LengthUnit::Inch), &[]);
    let inch_bounds = inch.inspection.summary.bounds.unwrap();
    let width = |b: &openchuck_app::setup::Aabb| b.max.x - b.min.x;
    assert!((width(&inch_bounds) / width(&mm) - 25.4).abs() < 1e-9);
    // Centred on the rotary axis: the preview moves with it.
    let centred = preview(
        &app,
        "cube_ascii.stl",
        None,
        &[TransformStep::CentreOnRotaryAxis],
    );
    let b = centred.inspection.summary.bounds.unwrap();
    assert!((b.min.y + b.max.y).abs() < 1e-9 && (b.min.z + b.max.z).abs() < 1e-9);
    // A missing file is an error, not a crash.
    assert!(
        app.start_import_preview(&stl("no_such.stl"), None, &[])
            .wait()
            .is_err()
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn req_imp_05_model_is_added_with_its_placement_in_one_step() {
    let (dir, mut app) = app("add");
    std::fs::create_dir_all(dir.join("models")).unwrap();
    std::fs::copy(stl("cube_ascii.stl"), dir.join("models/cube.stl")).unwrap();
    let steps = [TransformStep::CentreOnRotaryAxis];
    let (id, inspection) = app
        .add_model_with(
            &dir.join("models/cube.stl"),
            Some(LengthUnit::Millimetre),
            &steps,
        )
        .unwrap();
    let model = app.find_model(&id.to_string()).unwrap();
    assert_eq!(model.transform, steps);
    assert_eq!(model.units, Some(LengthUnit::Millimetre));
    assert_eq!(inspection.transform, steps);
    app.undo().unwrap();
    assert!(app.project().models().is_empty());
    std::fs::remove_dir_all(&dir).ok();
}
