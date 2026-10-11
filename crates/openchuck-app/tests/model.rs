// SPDX-License-Identifier: AGPL-3.0-or-later
//! Adding, inspecting, units and placement of models through the
//! application API (IMP-01, IMP-03, IMP-04, IMP-05, UI-08; work breakdown
//! slice 2).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_cmp
)]

use std::path::{Path, PathBuf};

use openchuck_app::model::Axis;
use openchuck_app::model::{
    AxisDirection, LengthUnit, STL_TESSELLATION_NOTICE, TransformStep, UnitConcern, UnitSource,
    orient_to_rotary_axis,
};
use openchuck_app::project::file;
use openchuck_app::{AppError, Application, Command, Threads, inspect_file};
use openchuck_testkit::determinism;

fn corpus(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/stl")
        .join(name)
}

/// A new project with `files` from the corpus copied into its `models/`.
fn project_with(name: &str, files: &[&str]) -> (PathBuf, Application) {
    let dir = std::env::temp_dir().join(format!("openchuck-model-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let app = Application::create(&dir, name, Threads::Exactly(2)).unwrap();
    std::fs::create_dir_all(dir.join("models")).unwrap();
    for file in files {
        std::fs::copy(corpus(file), dir.join("models").join(file)).unwrap();
    }
    (dir, app)
}

#[test]
fn req_imp_01_add_model_references_the_file_and_can_be_undone() {
    let (dir, mut app) = project_with("add", &["cube_binary.stl"]);
    let (id, inspection) = app
        .add_model(&dir.join("models/cube_binary.stl"), None)
        .unwrap();
    assert_eq!(inspection.summary.triangles, 12);
    assert!(inspection.report.is_clean());
    assert!(
        inspection
            .notes
            .iter()
            .any(|n| n == STL_TESSELLATION_NOTICE)
    );
    let model = app.find_model("models/cube_binary.stl").unwrap();
    assert_eq!(model.id, id);
    assert_eq!(app.find_model(&id.to_string()).unwrap().id, id);
    app.save().unwrap();
    let text = std::fs::read_to_string(dir.join("project.toml")).unwrap();
    assert!(text.contains("path = \"models/cube_binary.stl\""), "{text}");
    assert!(app.check_models().unwrap()[0].1 == openchuck_app::project::ModelCheck::Unchanged);

    assert!(app.undo().unwrap());
    assert!(app.project().models().is_empty());
    assert!(app.redo().unwrap());
    assert_eq!(app.project().models().len(), 1);

    // A file beside the project folder is referenced with "..". It is copied
    // next to the project, because the corpus can be on another drive than
    // the temporary folder (as on the Windows CI runner), which cannot be
    // referenced relatively.
    let shared = dir.with_file_name(format!(
        "{}-shared",
        dir.file_name().unwrap().to_string_lossy()
    ));
    std::fs::create_dir_all(&shared).unwrap();
    std::fs::copy(corpus("cube_ascii.stl"), shared.join("cube.stl")).unwrap();
    let (id, _) = app.add_model(&shared.join("cube.stl"), None).unwrap();
    let outside = app.find_model(&id.to_string()).unwrap();
    let expected = format!(
        "../{}/cube.stl",
        shared.file_name().unwrap().to_string_lossy()
    );
    assert_eq!(outside.path.as_str(), expected);
}

#[test]
fn req_nfr_07_a_damaged_model_is_refused_and_nothing_changes() {
    let (dir, mut app) = project_with("refuse", &["broken_truncated_binary.stl"]);
    let err = app
        .add_model(&dir.join("models/broken_truncated_binary.stl"), None)
        .unwrap_err();
    assert!(matches!(err, AppError::Import(_)), "{err}");
    assert!(err.to_string().contains("truncated"), "{err}");
    assert!(app.project().models().is_empty());
    assert!(!app.can_undo());
    let err = app
        .add_model(&dir.join("models/none.stl"), None)
        .unwrap_err();
    assert!(err.to_string().contains("cannot access"), "{err}");
    let err = app.find_model("nothing").unwrap_err();
    assert!(err.to_string().contains("has no model 'nothing'"), "{err}");
}

#[test]
fn req_imp_03_unit_override_survives_save_and_load() {
    let (dir, mut app) = project_with("units", &["cube_inch.stl"]);
    let (id, inspection) = app
        .add_model(&dir.join("models/cube_inch.stl"), None)
        .unwrap();
    // Read in millimetres; the file's "units=in" is a question, not a conversion.
    assert_eq!(inspection.units.source, UnitSource::Default);
    assert_eq!(inspection.summary.bounds.unwrap().max.x, 1.0);
    let question = inspection.unit_question.unwrap();
    assert_eq!(question.concern, UnitConcern::FileNames(LengthUnit::Inch));
    assert_eq!(app.project().models()[0].units, None);

    // The user confirms inches: stored on the model, and nothing is asked again.
    app.execute(Command::SetModelUnits {
        model: id,
        units: Some(LengthUnit::Inch),
    })
    .unwrap();
    app.save().unwrap();
    let reopened = Application::open(&dir, Threads::Exactly(1)).unwrap();
    assert_eq!(reopened.project().models()[0].units, Some(LengthUnit::Inch));
    let inspection = reopened.inspect_model(id).unwrap();
    assert_eq!(inspection.units.source, UnitSource::User);
    assert_eq!(inspection.unit_question, None);
    assert_eq!(inspection.summary.bounds.unwrap().max.x, 25.4);

    // Undo goes back to the default, and the question returns.
    assert!(app.undo().unwrap());
    assert_eq!(app.project().models()[0].units, None);
    let inspection = app.inspect_model(id).unwrap();
    assert_eq!(inspection.summary.bounds.unwrap().max.x, 1.0);
    assert!(inspection.unit_question.is_some());

    // Answering "keep millimetres" is stored too, so it is not asked again.
    app.execute(Command::SetModelUnits {
        model: id,
        units: Some(LengthUnit::Millimetre),
    })
    .unwrap();
    let inspection = app.inspect_model(id).unwrap();
    assert_eq!(inspection.unit_question, None);
    assert_eq!(inspection.summary.bounds.unwrap().max.x, 1.0);

    // Added with an override from the start.
    let (cm, _) = app
        .add_model(
            &dir.join("models/cube_inch.stl"),
            Some(LengthUnit::Centimetre),
        )
        .unwrap();
    assert_eq!(
        app.inspect_model(cm).unwrap().summary.bounds.unwrap().max.x,
        10.0
    );
}

#[test]
fn req_imp_05_placement_is_stored_undoable_and_keeps_normals_outward() {
    let (dir, mut app) = project_with("place", &["cube_ascii.stl"]);
    let (id, _) = app
        .add_model(&dir.join("models/cube_ascii.stl"), None)
        .unwrap();

    // Orient the cube's Z axis along the rotary axis and mirror it.
    let mut steps = orient_to_rotary_axis(AxisDirection::parse("+z").unwrap());
    steps.push(TransformStep::Mirror { axis: Axis::Y });
    app.execute(Command::SetModelTransform {
        model: id,
        transform: steps.clone(),
    })
    .unwrap();
    let inspection = app.inspect_model(id).unwrap();
    assert!(
        inspection.report.is_clean(),
        "{:?}",
        inspection.report.problems()
    );
    assert!(inspection.report.volume.unwrap() > 0.0);
    let b = inspection.summary.bounds.unwrap();
    assert_eq!((b.min.y, b.max.y, b.min.z, b.max.z), (-5.0, 5.0, -5.0, 5.0));
    assert_eq!(inspection.transform, steps);

    app.save().unwrap();
    let back = file::load(&dir).unwrap();
    assert_eq!(back.models()[0].transform, steps);

    assert!(app.undo().unwrap());
    assert!(app.project().models()[0].transform.is_empty());
    let b = app.inspect_model(id).unwrap().summary.bounds.unwrap();
    assert_eq!((b.min.y, b.max.y), (0.0, 10.0));

    // An invalid step is refused and not recorded.
    let err = app
        .execute(Command::SetModelTransform {
            model: id,
            transform: vec![TransformStep::Scale { factor: -1.0 }],
        })
        .unwrap_err();
    assert!(err.to_string().contains("greater than zero"), "{err}");
    assert!(app.can_redo());
}

#[test]
fn req_imp_04_inspection_reports_known_defects_and_changes() {
    let (dir, mut app) = project_with("defects", &["defect_hole.stl", "defect_flipped.stl"]);
    let (hole, _) = app
        .add_model(&dir.join("models/defect_hole.stl"), None)
        .unwrap();
    let (flipped, inspection) = app
        .add_model(&dir.join("models/defect_flipped.stl"), None)
        .unwrap();
    assert_eq!(inspection.report.flipped_triangles, vec![5]);
    assert_eq!(app.inspect_model(hole).unwrap().report.holes.len(), 1);
    // A mirror does not hide or invent a flipped triangle.
    app.execute(Command::SetModelTransform {
        model: flipped,
        transform: vec![TransformStep::Mirror { axis: Axis::X }],
    })
    .unwrap();
    assert_eq!(
        app.inspect_model(flipped).unwrap().report.flipped_triangles,
        vec![5]
    );

    // A model changed on disk is still inspected, with a note (PRJ-02).
    std::fs::copy(corpus("cube_ascii.stl"), dir.join("models/defect_hole.stl")).unwrap();
    let inspection = app.inspect_model(hole).unwrap();
    assert!(inspection.report.is_clean());
    assert!(
        inspection.notes.iter().any(|n| n.contains("has changed")),
        "{:?}",
        inspection.notes
    );
}

#[test]
fn req_imp_01_inspect_a_file_without_a_project() {
    let inspection =
        inspect_file(&corpus("cube_metre_binary.stl"), None, Threads::Exactly(1)).unwrap();
    assert_eq!(inspection.units.applied, LengthUnit::Millimetre);
    assert_eq!(inspection.summary.bounds.unwrap().max.x, 0.5);
    assert_eq!(
        inspection.unit_question.unwrap().concern,
        UnitConcern::FileNames(LengthUnit::Metre)
    );
    let inspection = inspect_file(
        &corpus("cube_metre_binary.stl"),
        Some(LengthUnit::Metre),
        Threads::Exactly(1),
    )
    .unwrap();
    assert_eq!(inspection.summary.bounds.unwrap().max.x, 500.0);
    let err = inspect_file(&corpus("broken_empty.stl"), None, Threads::Exactly(1)).unwrap_err();
    assert!(err.to_string().contains("the file is empty"), "{err}");
}

#[test]
fn req_nfr_06_inspection_is_identical_at_any_thread_count() {
    let runs = determinism::check(|| {
        // The harness runs this on pools of 1, 2 and N threads.
        let threads = Threads::Exactly(rayon::current_num_threads());
        let inspection =
            inspect_file(&corpus("two_cubes.stl"), Some(LengthUnit::Inch), threads).unwrap();
        format!("{inspection:?}").into_bytes()
    })
    .unwrap();
    assert!(runs.len() >= 3);
}
