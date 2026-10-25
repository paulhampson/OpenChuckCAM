// SPDX-License-Identifier: AGPL-3.0-or-later
//! PRJ-08: asynchronous recovery orchestration, independent of the UI.
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
use openchuck_app::checkpoints::{Notice, Session, discover, locate};
use openchuck_app::{Application, Threads};
use openchuck_project::recovery::Catalog;
use openchuck_project::{Project, file};
use std::time::{Duration, Instant};

struct Folders(std::path::PathBuf);
impl Folders {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        Self(std::env::temp_dir().join(format!(
            "openchuck-checkpoint-app-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        )))
    }
    fn path(&self, name: &str) -> std::path::PathBuf {
        self.0.join(name)
    }
}
impl Drop for Folders {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn until(session: &mut Session, mut predicate: impl FnMut(&Notice) -> bool) {
    let started = Instant::now();
    loop {
        for notice in session.poll(Duration::from_secs(200)) {
            if let Notice::Problem(message) = &notice {
                panic!("{message}");
            }
            if predicate(&notice) {
                return;
            }
        }
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "worker did not complete"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn req_prj_08_worker_snapshot_and_recovery_do_not_modify_saved_project() {
    let folders = Folders::new();
    let project = folders.path("project");
    let config = folders.path("config");
    file::save(&Project::new("saved"), &project).unwrap();
    let original = std::fs::read(project.join("project.toml")).unwrap();
    let mut session = Session::open(project.clone(), config.clone(), 3).unwrap();
    until(&mut session, |n| matches!(n, Notice::Ready));
    let changed = Project::new("checkpointed");
    session.tick(Duration::ZERO, &changed, 1, true);
    session.tick(Duration::from_secs(15), &changed, 1, true);
    until(&mut session, |n| {
        matches!(n, Notice::Published { revision: 1, .. })
    });
    let id = Catalog::read(&project).unwrap().snapshots[0].id.clone();
    session.restore(id).unwrap();
    let mut recovered = None;
    until(&mut session, |n| {
        if let Notice::Restored { project, .. } = n {
            recovered = Some(project.as_ref().clone());
            true
        } else {
            false
        }
    });
    assert_eq!(
        std::fs::read(project.join("project.toml")).unwrap(),
        original
    );
    let app =
        Application::from_checkpoint(&project, recovered.unwrap(), Threads::Exactly(1)).unwrap();
    assert!(app.is_modified());
    assert_eq!(app.project().name(), "checkpointed");
    app.save().unwrap();
    assert!(!app.is_modified());
    assert_eq!(file::load(&project).unwrap().name(), "checkpointed");
    session.close(1).unwrap();
    until(&mut session, |n| matches!(n, Notice::Closed { .. }));
}

#[test]
fn req_prj_08_discovery_deduplicates_paths_and_tracks_unexpected_sessions() {
    let folders = Folders::new();
    let project = folders.path("project");
    let config = folders.path("config");
    file::save(&Project::new("saved"), &project).unwrap();
    let mut session = Session::open(project.clone(), config.clone(), 3).unwrap();
    until(&mut session, |n| matches!(n, Notice::Ready));
    session.tick(Duration::ZERO, &Project::new("new"), 1, true);
    session.tick(Duration::from_secs(15), &Project::new("new"), 1, true);
    until(&mut session, |n| matches!(n, Notice::Published { .. }));
    assert!(
        !discover(&config).projects[0]
            .catalog
            .as_ref()
            .unwrap()
            .recovery_needed
    );
    drop(session);
    let started = Instant::now();
    loop {
        let found = discover(&config);
        assert_eq!(found.projects.len(), 1);
        if !found.projects[0].catalog.as_ref().unwrap().live {
            assert!(found.projects[0].catalog.as_ref().unwrap().recovery_needed);
            break;
        }
        assert!(started.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(1));
    }
    let moved = folders.path("moved");
    std::fs::rename(&project, &moved).unwrap();
    assert!(discover(&config).projects[0].problem.is_some());
    let missing = discover(&config).projects.remove(0);
    let located = locate(&config, &missing, &moved).unwrap();
    assert_eq!(located.folder, moved.canonicalize().unwrap());
    assert_eq!(discover(&config).projects.len(), 1);
    let unrelated = folders.path("unrelated");
    let unrelated_store = openchuck_project::recovery::Store::open(&unrelated).unwrap();
    drop(unrelated_store);
    assert!(locate(&config, &missing, &unrelated).is_err());
}

#[test]
fn req_prj_08_save_job_cannot_retire_a_newer_queued_snapshot_and_close_is_async() {
    let folders = Folders::new();
    let project = folders.path("project");
    file::save(&Project::new("saved"), &project).unwrap();
    let mut session = Session::open(project.clone(), folders.path("config"), 1).unwrap();
    until(&mut session, |n| matches!(n, Notice::Ready));
    session.tick(Duration::ZERO, &Project::new("new"), 2, true);
    session.tick(Duration::from_secs(15), &Project::new("new"), 2, true);
    session.saved(1).unwrap();
    until(&mut session, |n| matches!(n, Notice::Saved));
    assert!(!Catalog::read(&project).unwrap().snapshots[0].retired);
    session.close(2).unwrap();
    until(&mut session, |n| {
        matches!(n, Notice::Closed { problem: None })
    });
    assert!(!Catalog::read(&project).unwrap().recovery_needed);
}

#[test]
fn req_prj_08_reusing_a_moved_projects_old_path_cannot_erase_its_discovery_entry() {
    let folders = Folders::new();
    let original = folders.path("original");
    let config = folders.path("config");
    file::save(&Project::new("original"), &original).unwrap();
    let mut old = Session::open(original.clone(), config.clone(), 3).unwrap();
    until(&mut old, |n| matches!(n, Notice::Ready));
    old.close(0).unwrap();
    until(&mut old, |n| matches!(n, Notice::Closed { .. }));
    drop(old);
    let moved = folders.path("moved");
    std::fs::rename(&original, &moved).unwrap();
    file::save(&Project::new("different"), &original).unwrap();
    let mut new = Session::open(original.clone(), config.clone(), 3).unwrap();
    until(&mut new, |n| matches!(n, Notice::Ready));
    let found = discover(&config);
    assert_eq!(found.projects.len(), 2);
    let missing = found.projects.iter().find(|p| p.problem.is_some()).unwrap();
    locate(&config, missing, &moved).unwrap();
    let found = discover(&config);
    assert_eq!(found.projects.len(), 2);
    assert!(found.projects.iter().all(|p| p.problem.is_none()));
    new.close(0).unwrap();
    until(&mut new, |n| matches!(n, Notice::Closed { .. }));
}

#[test]
fn req_prj_08_req_prj_02_selected_recovery_reports_missing_and_changed_model_files() {
    use openchuck_project::recovery::{Kind, Snapshot, Store};
    use openchuck_project::{ContentHash, Item, ModelRef, Placement, RelativePath, id::ModelId};
    let folders = Folders::new();
    let root = folders.path("models");
    let mut project = Project::new("model references");
    for (id, path) in [(1, "missing.stl"), (2, "changed.stl")] {
        project
            .insert(
                Item::Model(ModelRef::new(
                    ModelId::from_u128(id),
                    RelativePath::new(path).unwrap(),
                    ContentHash::of_bytes(b"expected"),
                )),
                Placement::End,
            )
            .unwrap();
    }
    file::save(&Project::new("saved"), &root).unwrap();
    std::fs::write(root.join("changed.stl"), b"changed").unwrap();
    let mut store = Store::open(&root).unwrap();
    let metadata = store.metadata(Kind::Periodic, 1, 1);
    let identity = metadata.project_id.clone();
    let id = store
        .publish(&Snapshot::capture(&project, metadata).unwrap())
        .unwrap()
        .id;
    drop(store);
    let before = std::fs::read(root.join("project.toml")).unwrap();
    let recovered = openchuck_app::checkpoints::recover(&root, &id, &identity).unwrap();
    assert_eq!(*recovered.project, project);
    assert!(
        recovered
            .warnings
            .iter()
            .any(|w| w.contains("missing.stl") && w.contains("missing"))
    );
    assert!(
        recovered
            .warnings
            .iter()
            .any(|w| w.contains("changed.stl") && w.contains("changed"))
    );
    assert_eq!(std::fs::read(root.join("project.toml")).unwrap(), before);
}
