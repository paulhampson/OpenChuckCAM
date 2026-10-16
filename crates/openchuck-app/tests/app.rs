// SPDX-License-Identifier: AGPL-3.0-or-later
//! Application layer skeleton tests (work breakdown slice 0 item 8).

// Test code may unwrap, index and panic (NFR-07 applies to library code).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_cmp
)]

use openchuck_app::{AppError, Application, Event, JobManager, Query, QueryResult, Threads};
use openchuck_testkit::determinism;

fn empty_project(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("openchuck-app-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    Application::create(&dir, name, Threads::Exactly(1)).unwrap();
    dir
}

#[test]
fn req_nfr_01_empty_project_regenerates_headless() {
    let project = empty_project("regen");
    let app = Application::open(&project, Threads::Exactly(2)).unwrap();
    let result = app.regenerate().unwrap();
    assert!(result.toolpath.groups.is_empty());
    assert!(result.gcode.is_empty());
    let events = app.drain_events();
    assert!(
        matches!(events.first(), Some(Event::JobStarted { .. })),
        "{events:?}"
    );
    assert!(
        matches!(events.last(), Some(Event::JobFinished { error: None, .. })),
        "{events:?}"
    );
    match app.query(Query::Summary) {
        QueryResult::Summary(s) => assert_eq!(s.operations, 0),
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn req_nfr_07_missing_project_is_an_error() {
    let err = Application::open(std::path::Path::new("no/such/project"), Threads::All).unwrap_err();
    assert!(matches!(err, AppError::ProjectNotFound(_)));
    assert!(err.to_string().contains("does not exist"));
}

#[test]
fn req_nfr_07_a_folder_without_a_project_is_an_error() {
    let dir = std::env::temp_dir().join(format!("openchuck-app-bare-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let err = Application::open(&dir, Threads::Exactly(1)).unwrap_err();
    assert!(matches!(err, AppError::NoProject(_)), "{err}");
    assert!(err.to_string().contains("has no project.toml"), "{err}");
    // The front end can then offer to create one, and the folder opens.
    Application::create(&dir, "bare", Threads::Exactly(1)).unwrap();
    Application::open(&dir, Threads::Exactly(1)).unwrap();
    // Creating twice in one folder is refused.
    let project = empty_project("twice");
    assert!(Application::create(&project, "again", Threads::Exactly(1)).is_err());
}

#[test]
fn req_nfr_06_regeneration_is_identical_at_any_thread_count() {
    let project = empty_project("determinism");
    determinism::check(|| {
        let threads = Threads::Exactly(rayon::current_num_threads());
        let app = Application::open(&project, threads).unwrap();
        format!("{:?}", app.regenerate().unwrap()).into_bytes()
    })
    .unwrap();
}

#[test]
fn cancelled_job_reports_cancellation() {
    let (tx, _rx) = std::sync::mpsc::channel();
    let manager = JobManager::new(Threads::Exactly(1)).unwrap();
    let (go_tx, go_rx) = std::sync::mpsc::channel::<()>();
    let handle = manager.submit("wait", tx, move |context| {
        go_rx.recv().ok();
        context.check_cancelled()?;
        Ok(())
    });
    handle.cancel();
    go_tx.send(()).unwrap();
    let err = handle.wait().unwrap_err();
    assert!(err.to_string().contains("cancelled"), "{err}");
}

#[test]
fn req_nfr_07_a_panicking_job_becomes_an_error() {
    let (tx, _rx) = std::sync::mpsc::channel();
    let manager = JobManager::new(Threads::Exactly(1)).unwrap();
    let handle = manager.submit("boom", tx, |_| -> Result<(), AppError> { panic!("defect") });
    assert!(matches!(handle.wait(), Err(AppError::JobPanicked(_))));
    // The pool still works afterwards.
    let (tx, _rx) = std::sync::mpsc::channel();
    assert!(manager.submit("ok", tx, |_| Ok(1)).wait().is_ok());
}

#[test]
fn req_ui_09_cancellation_is_told_by_kind_not_by_wording() {
    use openchuck_app::AppError;
    assert!(AppError::Engine(openchuck_types::Error::Cancelled).is_cancelled());
    assert!(AppError::Path(openchuck_path::PathError::Cancelled).is_cancelled());
    // An operation the user named "Cancel test" that fails for another reason
    // is a failure, and its message is kept.
    let failed = AppError::Operation {
        name: "Cancel test".to_owned(),
        reason: "its setup has no stock".to_owned(),
    };
    assert!(failed.to_string().contains("Cancel"));
    assert!(!failed.is_cancelled());
}
