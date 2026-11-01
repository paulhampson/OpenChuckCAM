// SPDX-License-Identifier: AGPL-3.0-or-later
//! PRJ-08, NFR-07 (issue #429, decision 0460): recovery data belongs to the
//! folder that wrote it. A copied project folder carries the original's
//! recovery data; it is never offered for the copy, and it is kept.
#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use openchuck_project::recovery::{Catalog, DIRECTORY, Kind, Snapshot, Store, read_choice};
use openchuck_project::{Project, file};
use std::path::{Path, PathBuf};

struct Folder(PathBuf);
impl Folder {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "openchuck-recovery-origin-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path.canonicalize().unwrap())
    }
}
impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A saved project whose session ended unexpectedly with one checkpoint.
fn crashed(root: &Path, name: &str) -> (String, String) {
    file::save(&Project::new(format!("{name} saved")), root).unwrap();
    let mut store = Store::open(root).unwrap();
    let snapshot = Snapshot::capture(
        &Project::new(format!("{name} checkpoint")),
        store.metadata(Kind::Periodic, 1, 1000),
    )
    .unwrap();
    let id = store.publish(&snapshot).unwrap().id;
    let identity = snapshot.metadata().project_id.clone();
    drop(store); // no close: an unexpected end
    (identity, id)
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

fn set_aside_folders(root: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(".openchuck-recovery-copied-")
        })
        .collect()
}

#[test]
fn req_prj_08_copied_folder_never_offers_the_originals_checkpoints() {
    let base = Folder::new();
    let original = base.0.join("original");
    let copy = base.0.join("copy");
    let (identity, id) = crashed(&original, "original");
    copy_tree(&original, &copy);

    let catalog = Catalog::read(&copy).unwrap();
    assert_eq!(catalog.copied_from.as_deref(), Some(original.as_path()));
    assert!(!catalog.recovery_needed, "the copy must not be offered");
    assert!(catalog.snapshots.is_empty());
    assert!(
        read_choice(&copy, &id, &identity).is_err(),
        "the copy must not load the original's checkpoint"
    );

    // The original still owns and offers its own checkpoint.
    let theirs = Catalog::read(&original).unwrap();
    assert!(theirs.copied_from.is_none());
    assert!(theirs.recovery_needed);
    assert_eq!(theirs.snapshots[0].id, id);
}

#[test]
fn req_prj_08_writer_for_a_copy_sets_the_copied_data_aside_and_starts_its_own() {
    let base = Folder::new();
    let original = base.0.join("original");
    let copy = base.0.join("copy");
    let (identity, id) = crashed(&original, "original");
    copy_tree(&original, &copy);

    let store = Store::open(&copy).unwrap();
    let mine = store.metadata(Kind::Periodic, 0, 0).project_id;
    assert_ne!(mine, identity, "the copy gets its own recovery identity");
    let aside = store.set_aside().unwrap().to_owned();
    assert_eq!(set_aside_folders(&copy), vec![aside.clone()]);
    // Nothing is deleted: the copied checkpoint is kept, set aside.
    assert!(aside.join(&id).is_file());
    assert!(aside.join(".gitignore").is_file());
    assert!(store.catalog().unwrap().snapshots.is_empty());
    drop(store);

    // Reopening the copy keeps its new identity; nothing more is set aside.
    let again = Store::open(&copy).unwrap();
    assert_eq!(again.metadata(Kind::Periodic, 0, 0).project_id, mine);
    assert!(again.set_aside().is_none());
    drop(again);
    assert_eq!(set_aside_folders(&copy).len(), 1);
    assert!(Catalog::read(&copy).unwrap().copied_from.is_none());

    // The original and its saved files are untouched.
    let theirs = Catalog::read(&original).unwrap();
    assert!(theirs.recovery_needed);
    assert_eq!(file::load(&original).unwrap().name(), "original saved");
    assert_eq!(file::load(&copy).unwrap().name(), "original saved");
}

#[test]
fn req_prj_08_moved_folder_keeps_offering_its_own_checkpoints() {
    let base = Folder::new();
    let before = base.0.join("before");
    let after = base.0.join("after");
    let (identity, id) = crashed(&before, "moved");
    std::fs::rename(&before, &after).unwrap();

    let catalog = Catalog::read(&after).unwrap();
    assert!(catalog.copied_from.is_none());
    assert!(catalog.recovery_needed);
    assert_eq!(catalog.snapshots[0].id, id);
    assert!(read_choice(&after, &id, &identity).is_ok());

    let store = Store::open(&after).unwrap();
    assert_eq!(store.metadata(Kind::Periodic, 0, 0).project_id, identity);
    assert!(store.set_aside().is_none());
    assert_eq!(store.catalog().unwrap().snapshots.len(), 1);
    drop(store);
    assert!(set_aside_folders(&after).is_empty());
}

#[test]
fn req_prj_08_after_a_move_a_later_copy_is_still_recognised() {
    let base = Folder::new();
    let first = base.0.join("first");
    let moved = base.0.join("moved");
    let copy = base.0.join("copy");
    crashed(&first, "project");
    std::fs::rename(&first, &moved).unwrap();
    drop(Store::open(&moved).unwrap()); // the writer records the new place
    copy_tree(&moved, &copy);
    assert_eq!(
        Catalog::read(&copy).unwrap().copied_from.as_deref(),
        Some(moved.as_path())
    );
}

#[test]
fn req_prj_08_recovery_written_before_folders_were_recorded_is_still_offered() {
    let base = Folder::new();
    let root = base.0.join("older");
    let (identity, id) = crashed(&root, "older");
    std::fs::remove_file(root.join(DIRECTORY).join("origin")).unwrap();

    let catalog = Catalog::read(&root).unwrap();
    assert!(catalog.copied_from.is_none());
    assert!(catalog.recovery_needed);
    assert!(read_choice(&root, &id, &identity).is_ok());
    let store = Store::open(&root).unwrap();
    assert_eq!(store.metadata(Kind::Periodic, 0, 0).project_id, identity);
    drop(store);
    assert!(root.join(DIRECTORY).join("origin").is_file());
}

#[test]
fn req_prj_08_req_nfr_07_a_damaged_origin_record_never_hides_or_crashes_recovery() {
    let base = Folder::new();
    let root = base.0.join("damaged");
    let (identity, id) = crashed(&root, "damaged");
    let origin = root.join(DIRECTORY).join("origin");
    for content in [
        b"".to_vec(),
        vec![0xff, 0xfe, 0x00, 0x41],
        b"relative/folder".to_vec(),
        vec![b'x'; 70_000],
    ] {
        std::fs::write(&origin, &content).unwrap();
        let catalog = Catalog::read(&root).unwrap();
        assert!(catalog.copied_from.is_none());
        assert!(catalog.recovery_needed);
        assert!(read_choice(&root, &id, &identity).is_ok());
    }
    let store = Store::open(&root).unwrap();
    assert_eq!(store.metadata(Kind::Periodic, 0, 0).project_id, identity);
    assert!(store.set_aside().is_none());
}

#[test]
fn req_prj_08_an_original_whose_recovery_was_replaced_does_not_make_a_copy() {
    // The original folder still exists but holds another identity now (its
    // recovery data was discarded and restarted): the other folder is the
    // project's only remaining owner, so treat it as moved, not copied.
    let base = Folder::new();
    let original = base.0.join("original");
    let other = base.0.join("other");
    let (identity, _) = crashed(&original, "original");
    copy_tree(&original, &other);
    std::fs::remove_dir_all(original.join(DIRECTORY)).unwrap();
    drop(Store::open(&original).unwrap());
    let catalog = Catalog::read(&other).unwrap();
    assert!(catalog.copied_from.is_none());
    assert!(catalog.recovery_needed);
    let store = Store::open(&other).unwrap();
    assert_eq!(store.metadata(Kind::Periodic, 0, 0).project_id, identity);
}
