// SPDX-License-Identifier: AGPL-3.0-or-later
//! Creating the project's one setup from the interface (SET-01 to SET-06
//! need a setup; decision record 0144, lane SU).

#![allow(clippy::unwrap_used)]

use std::path::Path;

use openchuck_app::{AppError, Application, Threads, can_add_setup, new_setup};

fn project(name: &str, with_model: bool) -> (std::path::PathBuf, Application) {
    let dir =
        std::env::temp_dir().join(format!("openchuck-new-setup-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut app = Application::create(&dir, name, Threads::Exactly(1)).unwrap();
    if with_model {
        std::fs::create_dir_all(dir.join("models")).unwrap();
        let cube = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/stl/cube_ascii.stl");
        std::fs::copy(cube, dir.join("models/cube.stl")).unwrap();
        app.add_model(&dir.join("models/cube.stl"), None).unwrap();
    }
    (dir, app)
}

#[test]
fn req_set_01_a_new_project_can_get_its_one_setup_with_the_first_model_as_part() {
    let (dir, mut app) = project("with-model", true);
    assert!(can_add_setup(app.project()));
    let first = app.project().models()[0].id;
    app.execute(new_setup(app.project()).unwrap()).unwrap();
    let setups = app.project().setups();
    assert_eq!(setups.len(), 1);
    assert_eq!(setups[0].name, "Setup 1");
    assert_eq!(setups[0].part, Some(first));
    // Application defaults only: no stock, G54, the machine's clearance.
    assert_eq!(setups[0].stock, None);
    assert_eq!(setups[0].clearance, None);
    assert_eq!(
        setups[0].frame,
        openchuck_app::project::Setup::new(setups[0].id, "x").frame
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn req_set_01_with_no_model_the_setup_has_no_part() {
    let (dir, mut app) = project("no-model", false);
    app.execute(new_setup(app.project()).unwrap()).unwrap();
    assert_eq!(app.project().setups()[0].part, None);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn req_ui_08_req_set_01_the_new_setup_is_one_undo_and_undo_allows_another() {
    let (dir, mut app) = project("undo", true);
    app.execute(new_setup(app.project()).unwrap()).unwrap();
    assert!(!can_add_setup(app.project()));
    assert!(app.undo().unwrap());
    assert!(app.project().setups().is_empty());
    assert!(can_add_setup(app.project()));
    app.execute(new_setup(app.project()).unwrap()).unwrap();
    assert_eq!(app.project().setups().len(), 1);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn req_set_01_a_second_setup_is_refused_until_set_07() {
    let (dir, mut app) = project("second", true);
    app.execute(new_setup(app.project()).unwrap()).unwrap();
    assert!(matches!(
        new_setup(app.project()),
        Err(AppError::OneSetupOnly)
    ));
    assert_eq!(app.project().setups().len(), 1);
    let _ = std::fs::remove_dir_all(dir);
}
