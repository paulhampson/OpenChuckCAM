// SPDX-License-Identifier: AGPL-3.0-or-later
//! PRJ-08: recovery choices in real headless frames.
#![allow(clippy::unwrap_used, clippy::panic)]
use super::{
    Shell,
    frame_tests::{Rig, temp},
};
use openchuck_app::project::recovery::{Catalog, Kind, Snapshot, Store};
use openchuck_app::settings::SettingsFile;
use openchuck_app::{
    checkpoints::{Notice, Session},
    project::{Project, file},
};
use std::time::{Duration, Instant};

pub(super) fn ready_project(name: &str) -> Rig {
    let root = temp(name);
    let project = root.join("project");
    let config = root.join("config");
    file::save(&Project::new("saved"), &project).unwrap();
    let mut session = Session::open(project.clone(), config.clone(), 3).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if session
            .poll(Duration::ZERO)
            .iter()
            .any(|n| matches!(n, Notice::Ready))
        {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    drop(session);
    while Catalog::read(&project).unwrap().live {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    let mut store = Store::open(&project).unwrap();
    for (kind, revision, name) in [
        (Kind::Periodic, 1, "periodic state"),
        (Kind::Interim, 2, "interim state"),
    ] {
        let snapshot = Snapshot::capture(
            &Project::new(name),
            store.metadata(kind, revision, revision * 1000),
        )
        .unwrap();
        store.publish(&snapshot).unwrap();
    }
    drop(store);
    let mut rig = Rig::new(
        Shell::with_settings(false, None, Some(SettingsFile::in_folder(&config))),
        vec![root],
    );
    for _ in 0..100 {
        rig.frame(Vec::new());
        if rig.shows("Recover project changes") {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    rig
}

pub(super) fn click(rig: &mut Rig, text: &str) {
    let mut at = None;
    for _ in 0..4 {
        at = rig
            .visible_text_rects()
            .iter()
            .find(|(s, _)| s.contains(text))
            .map(|(_, rect)| rect.center());
        if at.is_some() {
            break;
        }
        rig.frame(Vec::new());
    }
    let at = at.unwrap_or_else(|| panic!("missing {text}: {:?}", rig.texts()));
    rig.click(at);
}

#[test]
fn req_prj_08_user_chooses_an_older_periodic_snapshot_without_saved_file_changes() {
    let mut rig = ready_project("checkpoint-choice");
    assert!(rig.shows("Recover project changes"));
    assert!(rig.shows("Periodic"));
    assert!(rig.shows("Interim"));
    assert!(rig.shows("periodic state"));
    assert!(rig.shows("interim state"));
    assert!(rig.shell.app.is_none());
    click(&mut rig, "Periodic");
    click(&mut rig, "Recover selected");
    let deadline = Instant::now() + Duration::from_secs(5);
    while rig.shell.app.is_none() {
        rig.frame(Vec::new());
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
    let app = rig.shell.app.as_ref().unwrap();
    assert_eq!(app.project().name(), "periodic state");
    assert!(app.is_modified());
    assert_eq!(file::load(app.location()).unwrap().name(), "saved");
    assert!(rig.shell.save());
    assert_eq!(
        file::load(rig.shell.app.as_ref().unwrap().location())
            .unwrap()
            .name(),
        "periodic state"
    );
}

#[test]
fn req_prj_08_keep_saved_is_distinct_from_explicit_checkpoint_deletion() {
    let mut rig = ready_project("checkpoint-keep");
    assert!(rig.shows("Recover project changes"));
    click(&mut rig, "Keep saved project");
    let deadline = Instant::now() + Duration::from_secs(5);
    while rig.shell.app.is_none() {
        rig.frame(Vec::new());
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
    let app = rig.shell.app.as_ref().unwrap();
    assert_eq!(app.project().name(), "saved");
    assert!(!app.is_modified());
    assert_eq!(Catalog::read(app.location()).unwrap().snapshots.len(), 2);
}

#[test]
fn req_prj_08_explicit_discard_requires_confirmation_and_deletes_only_after_answer() {
    let mut rig = ready_project("checkpoint-discard");
    assert!(rig.shows("Recover project changes"));
    let root = rig.dirs[0].join("project");
    click(&mut rig, "Discard recovery data…");
    assert_eq!(Catalog::read(&root).unwrap().snapshots.len(), 2);
    click(&mut rig, "Cancel deletion");
    assert_eq!(Catalog::read(&root).unwrap().snapshots.len(), 2);
    click(&mut rig, "Discard recovery data…");
    click(&mut rig, "Delete checkpoints");
    let deadline = Instant::now() + Duration::from_secs(5);
    while rig.shell.app.is_none() {
        rig.frame(Vec::new());
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(Catalog::read(&root).unwrap().snapshots.is_empty());
    assert_eq!(file::load(&root).unwrap().name(), "saved");
}

#[test]
fn req_prj_08_cancel_close_preserves_recovery_and_resolved_close_waits_for_retirement() {
    let mut rig = ready_project("checkpoint-close");
    click(&mut rig, "Interim");
    click(&mut rig, "Recover selected");
    let deadline = Instant::now() + Duration::from_secs(5);
    while rig.shell.app.is_none() {
        rig.frame(Vec::new());
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
    let root = rig.shell.app.as_ref().unwrap().location().to_owned();
    let ctx = rig.ctx.clone();
    rig.shell.request(super::Next::Quit, &ctx);
    rig.frame(Vec::new());
    assert!(matches!(rig.shell.dialog, Some(super::Dialog::Unsaved(_))));
    rig.frame(Vec::new());
    click(&mut rig, "Cancel");
    assert!(!rig.shell.quitting);
    assert!(
        Catalog::read(&root)
            .unwrap()
            .snapshots
            .iter()
            .any(|s| !s.retired)
    );
    rig.shell.request(super::Next::Quit, &ctx);
    rig.frame(Vec::new());
    rig.frame(Vec::new());
    click(&mut rig, "Discard changes");
    while !rig.shell.quitting {
        rig.frame(Vec::new());
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(!Catalog::read(&root).unwrap().recovery_needed);
    assert_eq!(file::load(&root).unwrap().name(), "saved");
}

#[test]
fn req_prj_08_read_only_recovery_can_restore_memory_and_reports_unavailable_writes() {
    let mut rig = ready_project("checkpoint-readonly");
    let folder = rig.dirs[0].join("project");
    let ignore = folder.join(".openchuck-recovery/.gitignore");
    std::fs::remove_file(&ignore).unwrap();
    std::fs::create_dir(&ignore).unwrap();
    click(&mut rig, "Periodic");
    click(&mut rig, "Recover selected");
    let deadline = Instant::now() + Duration::from_secs(5);
    while rig.shell.app.is_none() || rig.shell.recovery.problem.is_none() {
        rig.frame(Vec::new());
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    let app = rig.shell.app.as_ref().unwrap();
    assert!(app.is_modified());
    assert_eq!(app.project().name(), "periodic state");
    assert_eq!(file::load(&folder).unwrap().name(), "saved");
    assert!(
        rig.shell
            .recovery
            .problem
            .as_ref()
            .unwrap()
            .contains("Recovery")
    );
}

#[test]
fn req_prj_08_recovery_cannot_replace_another_unsaved_project_without_its_normal_warning() {
    let mut rig = ready_project("checkpoint-replace-unsaved");
    let other = rig.dirs[0].join("other");
    let app = openchuck_app::Application::create(
        &other,
        "Other saved project",
        openchuck_app::Threads::Exactly(1),
    )
    .unwrap();
    rig.shell.set_app(app);
    rig.shell
        .app
        .as_mut()
        .unwrap()
        .execute(openchuck_app::Command::RenameProject(
            "Other unsaved changes".into(),
        ))
        .unwrap();
    click(&mut rig, "Periodic");
    click(&mut rig, "Recover selected");
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        rig.frame(Vec::new());
        if matches!(rig.shell.dialog, Some(super::Dialog::Unsaved(_)))
            || rig.shell.app.as_ref().unwrap().project().name() == "periodic state"
        {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(
        rig.shell.app.as_ref().unwrap().project().name(),
        "Other unsaved changes"
    );
    assert!(matches!(rig.shell.dialog, Some(super::Dialog::Unsaved(_))));
    rig.frame(Vec::new());
    click(&mut rig, "Cancel");
    assert_eq!(
        rig.shell.app.as_ref().unwrap().project().name(),
        "Other unsaved changes"
    );
    assert_eq!(file::load(&other).unwrap().name(), "Other saved project");
}

#[test]
fn req_prj_08_req_ui_07_pending_field_text_is_excluded_and_recovery_cancel_preserves_it() {
    use openchuck_app::{Application, Command, Threads, project::ParameterValue};
    let mut rig = Rig::cube("checkpoint-pending-field", None);
    let op = rig.shell.app.as_ref().unwrap().project().operations()[0].id;
    rig.shell
        .app
        .as_mut()
        .unwrap()
        .set_parameter(op, "stepover", Some(ParameterValue::Number(0.8)))
        .unwrap();
    rig.shell.app.as_ref().unwrap().save().unwrap();
    rig.shell
        .app
        .as_mut()
        .unwrap()
        .execute(Command::RenameProject("Confirmed project edit".into()))
        .unwrap();
    rig.shell.select(super::Selection::Operation(op));
    rig.shell
        .open_step(openchuck_app::overview::Step::Operations);
    for _ in 0..4 {
        rig.frame(Vec::new());
    }
    super::pending_edit_tests::type_unconfirmed(&mut rig, "Stepover", "1.6");
    let root = rig.shell.app.as_ref().unwrap().location().to_owned();
    let mut session = Session::open(root.clone(), root.join("test-config"), 3).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !session.is_ready() {
        session.poll(Duration::ZERO);
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    let app = rig.shell.app.as_ref().unwrap();
    session.tick(
        Duration::ZERO,
        app.project(),
        app.revision(),
        app.is_modified(),
    );
    session.tick(
        Duration::from_secs(15),
        app.project(),
        app.revision(),
        app.is_modified(),
    );
    loop {
        if session
            .poll(Duration::from_secs(16))
            .iter()
            .any(|n| matches!(n, Notice::Published { .. }))
        {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    let choice = Catalog::read(&root).unwrap().snapshots.remove(0);
    let recovered =
        openchuck_app::checkpoints::recover(&root, &choice.id, &choice.metadata.project_id);
    assert!(recovered.is_err()); // another live writer is not crash recovery
    drop(session);
    while Catalog::read(&root).unwrap().live {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    let recovered =
        openchuck_app::checkpoints::recover(&root, &choice.id, &choice.metadata.project_id)
            .unwrap();
    assert_eq!(
        recovered.project.operations()[0].parameters.get("stepover"),
        Some(&ParameterValue::Number(0.8))
    );
    let target = root.join("other-project");
    std::fs::create_dir_all(&target).unwrap();
    let app =
        Application::from_checkpoint(&target, Project::new("Other recovery"), Threads::Exactly(1))
            .unwrap();
    rig.shell.recovery_project = Some((app, Vec::new(), true));
    let ctx = rig.ctx.clone();
    rig.shell.request(super::Next::Recovery, &ctx);
    for _ in 0..3 {
        rig.frame(Vec::new());
    }
    assert!(rig.shows("Unconfirmed changes"));
    click(&mut rig, "Cancel");
    rig.frame(Vec::new());
    assert_eq!(rig.shell.app.as_ref().unwrap().location(), root);
    assert_eq!(
        rig.shell.app.as_ref().unwrap().project().operations()[0]
            .parameters
            .get("stepover"),
        Some(&ParameterValue::Number(0.8))
    );
    assert!(!crate::pending_edits::entries(&rig.ctx).is_empty());
}
