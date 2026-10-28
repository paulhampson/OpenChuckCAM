// SPDX-License-Identifier: AGPL-3.0-or-later
//! PRJ-08: complete checkpoint state, independent of the saved project.
#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use openchuck_project::id::ModelId;
use openchuck_project::recovery::read_choice;
use openchuck_project::recovery::{Catalog, Store};
use openchuck_project::recovery::{Kind, Metadata, Snapshot};
use openchuck_project::{ContentHash, Item, ModelRef, Placement, Project, RelativePath, file};

struct Folder(std::path::PathBuf);
impl Folder {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!("openchuck-recovery-{}", uuid::Uuid::new_v4())))
    }
}
impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn publish(store: &mut Store, kind: Kind, revision: u64) -> String {
    let snapshot = Snapshot::capture(
        &Project::new(format!("revision {revision}")),
        store.metadata(kind, revision, 123),
    )
    .unwrap();
    store.publish(&snapshot).unwrap().id
}

fn metadata(kind: Kind) -> Metadata {
    Metadata {
        project_id: "00000000-0000-0000-0000-000000000001".into(),
        session_id: "00000000-0000-0000-0000-000000000002".into(),
        revision: 42,
        timestamp_ms: 123456789,
        kind,
    }
}

#[test]
fn req_prj_08_req_prj_02_req_prj_04_complete_state_and_model_identity_roundtrip() {
    let mut project = Project::new("Recovery é");
    let model = ModelRef::new(
        ModelId::from_u128(17),
        RelativePath::new("models/missing.step").unwrap(),
        ContentHash::of_bytes(b"original"),
    );
    project.insert(Item::Model(model), Placement::End).unwrap();
    let before = file::render(&project).unwrap();
    let snapshot = Snapshot::capture(&project, metadata(Kind::Periodic)).unwrap();
    let encoded = snapshot.encode().unwrap();
    let recovered = Snapshot::decode(&encoded).unwrap();
    assert_eq!(recovered.metadata(), snapshot.metadata());
    assert_eq!(recovered.project().unwrap(), project);
    assert_eq!(recovered.files(), &before);
    assert_eq!(file::render(&recovered.project().unwrap()).unwrap(), before);
    assert_eq!(snapshot.encode().unwrap(), encoded);
}

#[test]
fn req_prj_08_req_prj_04_req_prj_06_older_project_with_operations_and_sets_recovers_completely() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/schema-1");
    let project = file::load(&fixture).unwrap();
    assert!(!project.operations().is_empty());
    assert!(!project.selection_sets().is_empty());
    let before = file::render(&project).unwrap();
    let snapshot = Snapshot::capture(&project, metadata(Kind::Periodic)).unwrap();
    let restored = Snapshot::decode(&snapshot.encode().unwrap()).unwrap();
    assert_eq!(restored.project().unwrap(), project);
    assert_eq!(file::render(&restored.project().unwrap()).unwrap(), before);
}

#[test]
fn req_prj_08_req_nfr_07_oversized_and_invalid_file_maps_are_refused_before_recovery() {
    assert!(Snapshot::decode(&" ".repeat(openchuck_project::recovery::MAX_BYTES + 1)).is_err());
    let encoded = Snapshot::capture(&Project::new("p"), metadata(Kind::Periodic))
        .unwrap()
        .encode()
        .unwrap();
    for extra in [Some("../escape.toml"), Some("generated.nc"), None] {
        let mut envelope: toml::Value = toml::from_str(&encoded).unwrap();
        let mut payload: toml::Value =
            toml::from_str(envelope["payload"].as_str().unwrap()).unwrap();
        let files = payload["files"].as_table_mut().unwrap();
        if let Some(path) = extra {
            files.insert(path.into(), toml::Value::String("unexpected data".into()));
        } else {
            files.remove("project.toml");
        }
        let payload = toml::to_string(&payload).unwrap();
        envelope["checksum"] =
            toml::Value::String(blake3::hash(payload.as_bytes()).to_hex().to_string());
        envelope["payload"] = toml::Value::String(payload);
        assert!(Snapshot::decode(&toml::to_string(&envelope).unwrap()).is_err());
    }
}

#[test]
fn req_prj_08_req_nfr_07_truncation_corruption_and_incompatible_format_are_errors() {
    let snapshot = Snapshot::capture(&Project::new("kept"), metadata(Kind::Interim)).unwrap();
    let encoded = snapshot.encode().unwrap();
    for (end, _) in encoded
        .char_indices()
        .filter(|(i, _)| *i < encoded.trim_end().len())
    {
        assert!(
            Snapshot::decode(&encoded[..end]).is_err(),
            "accepted prefix at {end}"
        );
    }
    let changed = encoded.replace("kept", "lost");
    assert!(
        Snapshot::decode(&changed)
            .unwrap_err()
            .to_string()
            .contains("checksum")
    );
    let mut incompatible: toml::Value = toml::from_str(&encoded).unwrap();
    incompatible["format"] = toml::Value::Integer(999);
    assert!(
        Snapshot::decode(&toml::to_string(&incompatible).unwrap())
            .unwrap_err()
            .to_string()
            .contains("format")
    );
    assert!(Snapshot::decode("format = 1\npayload = 'broken'\nchecksum = 'wrong'\n").is_err());
}

#[test]
fn req_prj_08_req_nfr_07_identity_is_validated_and_unknown_envelope_fields_refused() {
    let mut bad = metadata(Kind::Interim);
    bad.project_id = "not an identity".into();
    assert!(Snapshot::capture(&Project::new("p"), bad).is_err());
    let good = Snapshot::capture(&Project::new("p"), metadata(Kind::Periodic))
        .unwrap()
        .encode()
        .unwrap();
    assert!(Snapshot::decode(&(good + "unexpected = 'data'\n")).is_err());
}

#[test]
fn req_prj_08_three_periodic_plus_one_interim_and_saved_files_untouched() {
    let folder = Folder::new();
    file::save(&Project::new("saved"), &folder.0).unwrap();
    let before = std::fs::read(folder.0.join("project.toml")).unwrap();
    let mut store = Store::open(&folder.0).unwrap();
    for revision in 1..=4 {
        publish(&mut store, Kind::Periodic, revision);
    }
    for revision in 5..=10 {
        publish(&mut store, Kind::Interim, revision);
    }
    let catalog = store.catalog().unwrap();
    let periodic: Vec<_> = catalog
        .snapshots
        .iter()
        .filter(|e| e.metadata.kind == Kind::Periodic)
        .map(|e| e.metadata.revision)
        .collect();
    assert_eq!(periodic, [4, 3, 2]);
    let interim: Vec<_> = catalog
        .snapshots
        .iter()
        .filter(|e| e.metadata.kind == Kind::Interim)
        .map(|e| e.metadata.revision)
        .collect();
    assert_eq!(interim, [10]);
    let chosen = &catalog.snapshots[2];
    assert_eq!(
        store.read(&chosen.id).unwrap().project().unwrap().name(),
        format!("revision {}", chosen.metadata.revision)
    );
    assert_eq!(
        std::fs::read(folder.0.join("project.toml")).unwrap(),
        before
    );
    assert_eq!(file::load(&folder.0).unwrap().name(), "saved");
}

#[test]
fn req_prj_08_single_instance_lock_and_unexpected_termination_discovery() {
    let folder = Folder::new();
    let mut store = Store::open(&folder.0).unwrap();
    publish(&mut store, Kind::Interim, 1);
    assert!(Store::open(&folder.0).is_err());
    assert!(Catalog::read(&folder.0).unwrap().live);
    // Dropping a session without explicit clean close models an unexpected
    // termination: lock releases but the unretired checkpoint remains offered.
    drop(store);
    let found = Catalog::read(&folder.0).unwrap();
    assert!(!found.live);
    assert!(found.recovery_needed);
    assert_eq!(found.snapshots.len(), 1);
}

#[test]
fn req_prj_08_save_and_clean_exit_preserve_newer_state_and_retained_choices() {
    let folder = Folder::new();
    let mut store = Store::open(&folder.0).unwrap();
    publish(&mut store, Kind::Periodic, 1);
    publish(&mut store, Kind::Interim, 3);
    store.retire_saved(1).unwrap();
    store.retire_saved(0).unwrap(); // an older completion cannot undo retirement
    let snapshots = store.catalog().unwrap().snapshots;
    assert!(!snapshots[0].retired);
    assert!(snapshots[1].retired);
    // An old asynchronous capture may finish after Save, but is still retired.
    let stale = publish(&mut store, Kind::Periodic, 1);
    assert!(
        store
            .catalog()
            .unwrap()
            .snapshots
            .iter()
            .find(|s| s.id == stale)
            .unwrap()
            .retired
    );
    store.close(3).unwrap();
    assert!(
        store
            .publish(
                &Snapshot::capture(&Project::new("late"), store.metadata(Kind::Interim, 4, 124))
                    .unwrap()
            )
            .is_err()
    );
    drop(store);
    let catalog = Catalog::read(&folder.0).unwrap();
    assert!(!catalog.recovery_needed);
    assert_eq!(catalog.snapshots.len(), 2);
    assert!(catalog.snapshots.iter().all(|s| s.retired));
}

#[test]
fn req_prj_08_req_nfr_07_corrupt_partial_and_foreign_data_are_preserved() {
    let folder = Folder::new();
    let mut store = Store::open(&folder.0).unwrap();
    let id = publish(&mut store, Kind::Periodic, 1);
    let dir = folder.0.join(".openchuck-recovery");
    let bad = dir.join("p-00000000000000000099-00000000-0000-0000-0000-000000000099.checkpoint");
    std::fs::write(&bad, "truncated").unwrap();
    let partial = dir.join("interrupted.part");
    std::fs::write(&partial, "unfinished").unwrap();
    for revision in 2..=6 {
        publish(&mut store, Kind::Periodic, revision);
    }
    let catalog = store.catalog().unwrap();
    assert_eq!(catalog.snapshots.len(), 3);
    assert!(catalog.problems.iter().any(|p| p.contains("99")));
    assert!(bad.exists());
    assert!(partial.exists());
    assert!(store.read("../../project.toml").is_err());
    assert!(store.read(&id).is_err()); // correctly rotated, not a stale recovery choice
    let foreign = Snapshot::capture(&Project::new("foreign"), metadata(Kind::Interim)).unwrap();
    assert!(store.publish(&foreign).is_err());
}

#[test]
fn req_prj_08_keep_saved_dismisses_without_deleting_and_new_changes_reoffer() {
    let folder = Folder::new();
    let mut store = Store::open(&folder.0).unwrap();
    publish(&mut store, Kind::Periodic, 1);
    store.dismiss().unwrap();
    drop(store);
    let catalog = Catalog::read(&folder.0).unwrap();
    assert!(!catalog.recovery_needed);
    assert_eq!(catalog.snapshots.len(), 1);
    let mut store = Store::open(&folder.0).unwrap();
    publish(&mut store, Kind::Interim, 2);
    drop(store);
    assert!(Catalog::read(&folder.0).unwrap().recovery_needed);
    let mut store = Store::open(&folder.0).unwrap();
    store.discard().unwrap();
    assert!(store.catalog().unwrap().snapshots.is_empty());
    publish(&mut store, Kind::Interim, 3);
    drop(store);
    assert!(Catalog::read(&folder.0).unwrap().recovery_needed);
}

#[test]
fn req_prj_08_a_stale_interim_job_cannot_replace_a_newer_checkpoint() {
    let folder = Folder::new();
    let mut store = Store::open(&folder.0).unwrap();
    let newest = publish(&mut store, Kind::Interim, 10);
    let old =
        Snapshot::capture(&Project::new("old"), store.metadata(Kind::Interim, 9, 999)).unwrap();
    assert!(store.publish(&old).is_err());
    let catalog = store.catalog().unwrap();
    assert_eq!(catalog.snapshots[0].id, newest);
}

#[test]
fn req_prj_08_req_nfr_07_corrupt_retirement_metadata_does_not_block_other_choices() {
    let folder = Folder::new();
    let id = {
        let mut store = Store::open(&folder.0).unwrap();
        publish(&mut store, Kind::Periodic, 1)
    };
    let dir = folder.0.join(".openchuck-recovery");
    std::fs::write(dir.join("state.toml"), "truncated = [").unwrap();
    let catalog = Catalog::read(&folder.0).unwrap();
    assert_eq!(catalog.snapshots.len(), 1);
    assert!(!catalog.problems.is_empty());
    let store = Store::open(&folder.0).unwrap();
    assert_eq!(
        store.read(&id).unwrap().project().unwrap().name(),
        "revision 1"
    );
    assert!(
        store
            .catalog()
            .unwrap()
            .problems
            .iter()
            .any(|p| p.contains("corrupt"))
    );
}

#[test]
fn req_prj_08_read_only_recovery_does_not_require_a_writable_store() {
    let folder = Folder::new();
    let (id, identity) = {
        let mut store = Store::open(&folder.0).unwrap();
        let id = publish(&mut store, Kind::Periodic, 1);
        (id, store.metadata(Kind::Periodic, 1, 1).project_id)
    };
    let ignore = folder.0.join(".openchuck-recovery/.gitignore");
    std::fs::remove_file(&ignore).unwrap();
    std::fs::create_dir(&ignore).unwrap(); // reproducible write failure, even when tests run as root
    assert!(Store::open(&folder.0).is_err());
    let before = std::fs::read_dir(folder.0.join(".openchuck-recovery"))
        .unwrap()
        .count();
    assert_eq!(
        read_choice(&folder.0, &id, &identity)
            .unwrap()
            .project()
            .unwrap()
            .name(),
        "revision 1"
    );
    assert_eq!(
        std::fs::read_dir(folder.0.join(".openchuck-recovery"))
            .unwrap()
            .count(),
        before
    );
}

#[test]
fn req_prj_08_req_prj_03_checkpoints_stay_out_of_git_and_normal_project_enumeration() {
    let folder = Folder::new();
    let saved = Project::new("tracked project");
    file::save(&saved, &folder.0).unwrap();
    let git = |args: &[&str]| {
        let result = std::process::Command::new("git")
            .args(args)
            .current_dir(&folder.0)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        result.stdout
    };
    git(&["init", "-q"]);
    git(&["add", "project.toml"]);
    let before = git(&["status", "--porcelain=v1", "--untracked-files=all"]);
    let mut store = Store::open(&folder.0).unwrap();
    publish(&mut store, Kind::Periodic, 1);
    publish(&mut store, Kind::Interim, 2);
    git(&["add", "."]);
    assert_eq!(
        git(&["status", "--porcelain=v1", "--untracked-files=all"]),
        before
    );
    assert_eq!(git(&["ls-files"]), b"project.toml\n");
    assert_eq!(file::load(&folder.0).unwrap(), saved);
}

#[test]
fn req_prj_08_retrying_a_published_revision_does_not_grow_recovery_storage() {
    let folder = Folder::new();
    let mut store = Store::open(&folder.0).unwrap();
    let snapshot = Snapshot::capture(
        &Project::new("same state"),
        store.metadata(Kind::Periodic, 1, 1),
    )
    .unwrap();
    let first = store.publish(&snapshot).unwrap();
    let retried = store.publish(&snapshot).unwrap();
    assert_eq!(first.id, retried.id);
    assert_eq!(store.catalog().unwrap().snapshots.len(), 1);
}

#[test]
fn req_prj_08_req_nfr_07_write_failure_preserves_the_last_good_checkpoint_and_saved_files() {
    let folder = Folder::new();
    file::save(&Project::new("saved"), &folder.0).unwrap();
    let saved = std::fs::read(folder.0.join("project.toml")).unwrap();
    let mut store = Store::open(&folder.0).unwrap();
    let last = publish(&mut store, Kind::Periodic, 1);
    let dir = folder.0.join(".openchuck-recovery");
    std::fs::rename(dir.join("state.toml"), dir.join("state.saved")).unwrap();
    std::fs::create_dir(dir.join("state.toml")).unwrap();
    let failed = Snapshot::capture(
        &Project::new("not published"),
        store.metadata(Kind::Periodic, 2, 2),
    )
    .unwrap();
    assert!(store.publish(&failed).is_err());
    assert_eq!(
        store.read(&last).unwrap().project().unwrap().name(),
        "revision 1"
    );
    assert_eq!(store.catalog().unwrap().snapshots.len(), 1);
    assert_eq!(std::fs::read(folder.0.join("project.toml")).unwrap(), saved);
    std::fs::remove_dir(dir.join("state.toml")).unwrap();
    std::fs::rename(dir.join("state.saved"), dir.join("state.toml")).unwrap();
    assert!(store.publish(&failed).is_ok());
}

#[cfg(unix)]
#[test]
fn req_prj_08_req_nfr_07_generated_ignore_and_lock_cannot_follow_external_symlinks() {
    use std::os::unix::fs::symlink;
    for name in [".gitignore", "writer.lock"] {
        let folder = Folder::new();
        let dir = folder.0.join(".openchuck-recovery");
        std::fs::create_dir_all(&dir).unwrap();
        let outside = folder.0.join("user-file.txt");
        std::fs::write(&outside, "user content").unwrap();
        symlink(&outside, dir.join(name)).unwrap();
        let opened = Store::open(&folder.0);
        if name == "writer.lock" {
            assert!(opened.is_err());
        }
        assert_eq!(std::fs::read_to_string(&outside).unwrap(), "user content");
    }
}

#[test]
fn req_prj_08_req_prj_04_req_gc_17_current_options_survive_format_1_and_future_project_schema_is_refused()
 {
    use openchuck_project::ParameterValue;
    assert_eq!(openchuck_project::recovery::FORMAT, 1);
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/schema-1");
    for flag in [None, Some(false), Some(true)] {
        let mut project = file::load(&fixture).unwrap();
        // The schema-18 ramp option and schema-19 machine settings coexist
        // in a format-1 recovery snapshot without losing either.
        project
            .set_machine(Some(
                openchuck_project::machines::builtin(openchuck_project::machines::MAKERA_Z1)
                    .unwrap()
                    .unwrap(),
            ))
            .unwrap();
        let mut operation = project.operations()[0].clone();
        if let Some(flag) = flag {
            operation
                .parameters
                .insert("ramp_back_and_forth".into(), ParameterValue::Flag(flag));
        }
        project.replace(Item::Operation(operation)).unwrap();
        let saved = file::render(&project).unwrap();
        let encoded = Snapshot::capture(&project, metadata(Kind::Periodic))
            .unwrap()
            .encode()
            .unwrap();
        let recovered = Snapshot::decode(&encoded).unwrap();
        assert_eq!(recovered.project().unwrap(), project);
        assert_eq!(recovered.files(), &saved);
        assert_eq!(file::render(&recovered.project().unwrap()).unwrap(), saved);

        let mut envelope: toml::Value = toml::from_str(&encoded).unwrap();
        let mut payload: toml::Value =
            toml::from_str(envelope["payload"].as_str().unwrap()).unwrap();
        let mut newer: toml::Value =
            toml::from_str(payload["files"]["project.toml"].as_str().unwrap()).unwrap();
        newer["schema"] = toml::Value::Integer(file::SCHEMA + 1);
        payload["files"]["project.toml"] = toml::Value::String(toml::to_string(&newer).unwrap());
        let payload = toml::to_string(&payload).unwrap();
        envelope["checksum"] =
            toml::Value::String(blake3::hash(payload.as_bytes()).to_hex().to_string());
        envelope["payload"] = toml::Value::String(payload);
        assert!(Snapshot::decode(&toml::to_string(&envelope).unwrap()).is_err());
    }
}
