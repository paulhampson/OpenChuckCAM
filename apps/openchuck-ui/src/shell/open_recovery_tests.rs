// SPDX-License-Identifier: AGPL-3.0-or-later
//! PRJ-08 (issue #429, decision 0460): recovery is offered only for the
//! project being opened. Another project's kept recovery data is one line in
//! Messages; Keep saved and Discard never switch to another project; a copied
//! project folder never offers the original's checkpoints. Generated
//! projects only.
#![allow(clippy::unwrap_used, clippy::panic)]
use super::checkpoint_tests::click;
use super::{
    Shell,
    frame_tests::{Rig, temp},
};
use openchuck_app::checkpoints::{Notice, Session};
use openchuck_app::project::recovery::{Catalog, Kind, Snapshot, Store};
use openchuck_app::project::{Project, file};
use openchuck_app::settings::SettingsFile;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// A saved project `name`, registered for recovery, whose last session ended
/// unexpectedly with one checkpoint named "`name` checkpoint".
fn crashed(folder: &Path, config: &Path, name: &str) {
    file::save(&Project::new(format!("{name} saved")), folder).unwrap();
    let mut session = Session::open(folder.to_owned(), config.to_owned(), 3).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !session
        .poll(Duration::ZERO)
        .iter()
        .any(|n| matches!(n, Notice::Ready))
    {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    drop(session);
    while Catalog::read(folder).unwrap().live {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    let mut store = Store::open(folder).unwrap();
    let snapshot = Snapshot::capture(
        &Project::new(format!("{name} checkpoint")),
        store.metadata(Kind::Periodic, 1, 1000),
    )
    .unwrap();
    store.publish(&snapshot).unwrap();
}

fn launch(root: &Path, config: &Path, folder: Option<PathBuf>) -> Rig {
    Rig::new(
        Shell::with_settings(false, folder, Some(SettingsFile::in_folder(config))),
        vec![root.to_owned()],
    )
}

fn until(rig: &mut Rig, what: &str, mut done: impl FnMut(&Rig) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while !done(rig) {
        rig.frame(Vec::new());
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(2));
    }
    rig.frame(Vec::new());
    rig.frame(Vec::new());
}

fn writer_ready(rig: &Rig) -> bool {
    rig.shell
        .recovery
        .active
        .as_ref()
        .is_some_and(|s| s.is_ready())
}

fn location(rig: &Rig) -> PathBuf {
    rig.shell
        .app
        .as_ref()
        .unwrap()
        .location()
        .canonicalize()
        .unwrap()
}

/// The project names of the offered checkpoints (the dialog's choices; the
/// Messages lines behind it are not choices).
fn offered_names(rig: &Rig) -> Vec<String> {
    rig.shell
        .recovery
        .review
        .as_ref()
        .and_then(|r| r.candidate.catalog.as_ref())
        .map(|c| c.snapshots.iter().map(|e| e.project_name.clone()).collect())
        .unwrap_or_default()
}

/// Click the text that is exactly `text` (a button, not a heading).
fn click_exact(rig: &mut Rig, text: &str) {
    for _ in 0..4 {
        if let Some(at) = rig
            .visible_text_rects()
            .iter()
            .find(|(s, _)| s == text)
            .map(|(_, rect)| rect.center())
        {
            rig.click(at);
            return;
        }
        rig.frame(Vec::new());
    }
    panic!("missing {text}");
}

fn offered_folder(rig: &Rig) -> Option<PathBuf> {
    rig.shell
        .recovery
        .review
        .as_ref()
        .map(|r| r.candidate.folder.clone())
}

#[test]
fn req_prj_08_opening_a_project_offers_only_its_own_recovery() {
    let root = temp("open-recovery-own");
    let config = root.join("config");
    let (a, b) = (root.join("a"), root.join("b"));
    crashed(&a, &config, "Alpha");
    crashed(&b, &config, "Bravo");
    let (a, b) = (a.canonicalize().unwrap(), b.canonicalize().unwrap());
    let mut rig = launch(&root, &config, Some(b.clone()));
    until(&mut rig, "the offer", |r| {
        r.shows("Recover project changes")
    });
    // Give startup discovery time to land: it must not replace the offer.
    until(&mut rig, "startup discovery", |r| {
        !r.shell.recovery.elsewhere.is_empty()
    });
    assert_eq!(offered_folder(&rig), Some(b.clone()));
    assert_eq!(offered_names(&rig), vec!["Bravo checkpoint".to_owned()]);
    click(&mut rig, "Keep saved project");
    until(&mut rig, "the writer", writer_ready);
    assert_eq!(location(&rig), b, "Keep saved keeps the requested project");
    assert_eq!(
        rig.shell.app.as_ref().unwrap().project().name(),
        "Bravo saved"
    );
    assert!(!rig.shell.app.as_ref().unwrap().is_modified());
    assert!(offered_folder(&rig).is_none());
    // Alpha's data is untouched, and Messages says where it is.
    let alpha = Catalog::read(&a).unwrap();
    assert!(alpha.recovery_needed);
    assert_eq!(alpha.snapshots.len(), 1);
    assert!(rig.shows("Alpha checkpoint"));
    assert!(rig.shows("Open project"));
    // Opening Alpha offers Alpha's own checkpoint, and only that.
    rig.shell.open(a.clone());
    until(&mut rig, "Alpha's offer", |r| {
        offered_folder(r).as_deref() == Some(a.as_path())
    });
    assert_eq!(offered_names(&rig), vec!["Alpha checkpoint".to_owned()]);
}

#[test]
fn req_prj_08_another_projects_recovery_never_interrupts_the_opened_project() {
    let root = temp("open-recovery-other");
    let config = root.join("config");
    let (a, b) = (root.join("a"), root.join("b"));
    crashed(&a, &config, "Alpha");
    file::save(&Project::new("Bravo saved"), &b).unwrap();
    let a = a.canonicalize().unwrap();
    // A launch with a project (a double-click) races startup discovery.
    let mut rig = launch(&root, &config, Some(b.clone()));
    until(&mut rig, "discovery and the writer", |r| {
        !r.shell.recovery.elsewhere.is_empty() && writer_ready(r)
    });
    for _ in 0..20 {
        rig.frame(Vec::new());
        assert!(!rig.shows("Recover project changes"));
        assert!(rig.shell.recovery.review.is_none());
    }
    assert_eq!(location(&rig), b.canonicalize().unwrap());
    assert!(rig.shows("Alpha checkpoint"));
    // The line's action asks to open Alpha, through the ordinary dialog.
    click(&mut rig, "Open project");
    rig.frame(Vec::new());
    match &rig.shell.dialog {
        Some(super::Dialog::Folder { create, folder, .. }) => {
            assert!(!create);
            assert_eq!(Path::new(folder).canonicalize().unwrap(), a);
        }
        other => panic!("expected the Open dialog, got {other:?}"),
    }
    assert_eq!(location(&rig), b.canonicalize().unwrap());
    assert!(Catalog::read(&a).unwrap().recovery_needed);
}

#[test]
fn req_prj_08_file_open_after_startup_discovery_offers_nothing_for_another_project() {
    let root = temp("open-recovery-file-open");
    let config = root.join("config");
    let (a, b) = (root.join("a"), root.join("b"));
    crashed(&a, &config, "Alpha");
    file::save(&Project::new("Bravo saved"), &b).unwrap();
    let mut rig = launch(&root, &config, None);
    until(&mut rig, "startup discovery", |r| {
        !r.shell.recovery.elsewhere.is_empty()
    });
    assert!(!rig.shows("Recover project changes"));
    assert!(rig.shows("Alpha checkpoint"));
    rig.shell.open(b.clone());
    until(&mut rig, "the writer", writer_ready);
    for _ in 0..10 {
        rig.frame(Vec::new());
        assert!(rig.shell.recovery.review.is_none());
    }
    assert_eq!(location(&rig), b.canonicalize().unwrap());
}

#[test]
fn req_prj_08_discard_deletes_only_the_opened_projects_checkpoints() {
    let root = temp("open-recovery-discard");
    let config = root.join("config");
    let (a, b) = (root.join("a"), root.join("b"));
    crashed(&a, &config, "Alpha");
    crashed(&b, &config, "Bravo");
    let mut rig = launch(&root, &config, Some(b.clone()));
    until(&mut rig, "the offer", |r| {
        r.shows("Recover project changes")
    });
    until(&mut rig, "startup discovery", |r| {
        !r.shell.recovery.elsewhere.is_empty()
    });
    click(&mut rig, "Discard recovery data…");
    // The dialog grows and re-centres when it asks to confirm.
    rig.frame(Vec::new());
    rig.frame(Vec::new());
    click_exact(&mut rig, "Delete checkpoints");
    until(&mut rig, "the writer", writer_ready);
    assert_eq!(location(&rig), b.canonicalize().unwrap());
    assert!(Catalog::read(&b).unwrap().snapshots.is_empty());
    assert_eq!(Catalog::read(&a).unwrap().snapshots.len(), 1);
    assert!(Catalog::read(&a).unwrap().recovery_needed);
}

#[test]
fn req_prj_08_equivalent_path_forms_name_the_same_project() {
    let root = temp("open-recovery-paths");
    let config = root.join("config");
    let (a, b) = (root.join("a"), root.join("b"));
    crashed(&a, &config, "Alpha");
    crashed(&b, &config, "Bravo");
    let b_canonical = b.canonicalize().unwrap();
    #[cfg_attr(not(windows), allow(unused_mut))]
    let mut forms = vec![
        b.join("."),
        a.join("..").join("b"),
        PathBuf::from(format!("{}{}", b.display(), std::path::MAIN_SEPARATOR)),
    ];
    #[cfg(windows)]
    {
        // The verbatim form and a different letter case name the same folder.
        forms.push(b_canonical.clone());
        let text = b_canonical.display().to_string();
        let plain = text.trim_start_matches(r"\\?\").to_owned();
        forms.push(PathBuf::from(plain.to_uppercase()));
        forms.push(PathBuf::from(plain.to_lowercase()));
    }
    for form in forms {
        let mut rig = launch(&root, &config, Some(form.clone()));
        until(&mut rig, "the offer", |r| {
            r.shows("Recover project changes")
        });
        assert_eq!(offered_folder(&rig), Some(b_canonical.clone()), "{form:?}");
        let names = offered_names(&rig);
        assert!(
            !names.is_empty() && names.iter().all(|n| n == "Bravo checkpoint"),
            "{names:?}"
        );
        click(&mut rig, "Keep saved project");
        until(&mut rig, "the writer", writer_ready);
        assert_eq!(location(&rig), b_canonical, "{form:?}");
        // Keep saved dismissed Bravo's offer; make it pending again for the
        // next form (a new crash) without touching Alpha.
        rig.dirs.clear();
        drop(rig);
        crashed(&b, &config, "Bravo");
    }
    assert!(Catalog::read(&a).unwrap().recovery_needed);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn req_prj_08_a_copied_project_folder_never_offers_the_originals_checkpoints() {
    let root = temp("open-recovery-copy");
    let config = root.join("config");
    let (original, copy) = (root.join("original"), root.join("copy"));
    crashed(&original, &config, "Original");
    copy_tree(&original, &copy);
    let mut rig = launch(&root, &config, Some(copy.clone()));
    until(&mut rig, "the writer", writer_ready);
    for _ in 0..10 {
        rig.frame(Vec::new());
        assert!(rig.shell.recovery.review.is_none());
    }
    assert!(rig.shows("is a copy of"));
    assert_eq!(location(&rig), copy.canonicalize().unwrap());
    assert_eq!(
        rig.shell.app.as_ref().unwrap().project().name(),
        "Original saved"
    );
    let aside = std::fs::read_dir(&copy)
        .unwrap()
        .filter(|e| {
            e.as_ref()
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".openchuck-recovery-copied-")
        })
        .count();
    assert_eq!(aside, 1, "the copied recovery data is kept, set aside");
    let theirs = Catalog::read(&original).unwrap();
    assert!(theirs.recovery_needed);
    assert_eq!(theirs.snapshots.len(), 1);
}

#[test]
fn req_prj_08_a_moved_project_is_located_from_its_messages_line() {
    let root = temp("open-recovery-locate");
    let config = root.join("config");
    let (before, after) = (root.join("before"), root.join("after"));
    crashed(&before, &config, "Moved");
    std::fs::rename(&before, &after).unwrap();
    let mut rig = launch(&root, &config, None);
    until(&mut rig, "startup discovery", |r| {
        !r.shell.recovery.elsewhere.is_empty()
    });
    assert!(rig.shows("Locate folder"));
    assert!(!rig.shows("Recover project changes"));
    click(&mut rig, "Locate folder");
    until(&mut rig, "the Locate dialog", |r| {
        r.shows("Locate moved project")
    });
    rig.shell.recovery.locate.as_mut().unwrap().path = root.join("nowhere").display().to_string();
    rig.frame(Vec::new());
    click_exact(&mut rig, "Locate");
    until(&mut rig, "the failed lookup", |r| {
        !r.shell.recovery.reading()
    });
    assert!(
        rig.shows("Locate moved project"),
        "a failed Locate keeps the dialog"
    );
    assert!(
        rig.shell
            .recovery
            .locate
            .as_ref()
            .unwrap()
            .problem
            .is_some()
    );
    rig.shell.recovery.locate.as_mut().unwrap().path = after.display().to_string();
    rig.frame(Vec::new());
    click_exact(&mut rig, "Locate");
    until(&mut rig, "the lookup", |r| {
        r.shell.recovery.locate.is_none()
    });
    assert!(rig.shows("Moved checkpoint"));
    assert!(rig.shows("Open project"));
    assert!(rig.shell.recovery.review.is_none());
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for item in std::fs::read_dir(from).unwrap() {
        let item = item.unwrap();
        let target = to.join(item.file_name());
        if item.file_type().unwrap().is_dir() {
            copy_tree(&item.path(), &target);
        } else {
            std::fs::copy(item.path(), target).unwrap();
        }
    }
}
