// SPDX-License-Identifier: AGPL-3.0-or-later
//! NFR-07: measurement output must preserve pre-existing caller data.
#![allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "../examples/step_accuracy/workspace.rs"]
mod workspace;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "step-accuracy-workspace-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[test]
fn req_nfr_07_keep_and_derived_stl_destinations_preserve_existing_data() {
    let f = Fixture::new();
    let keep = f.0.join("measurement");
    for dir in [keep.clone(), keep.with_extension("stl")] {
        std::fs::create_dir(&dir).unwrap();
        let sentinel = dir.join("unrelated.txt");
        std::fs::write(&sentinel, b"caller-owned contents").unwrap();
        let result = workspace::create_destination(&dir);
        assert!(result.is_err(), "must refuse existing destination");
        assert_eq!(std::fs::read(&sentinel).unwrap(), b"caller-owned contents");
    }
}
#[test]
fn req_nfr_07_fresh_destination_is_reserved_and_existing_files_or_empty_dirs_refuse() {
    let f = Fixture::new();
    let fresh = f.0.join("nested").join("new");
    workspace::create_destination(&fresh).unwrap();
    assert!(fresh.is_dir());
    assert!(workspace::create_destination(&fresh).is_err());
    let file = f.0.join("existing-file");
    std::fs::write(&file, b"keep").unwrap();
    assert!(workspace::create_destination(&file).is_err());
    assert_eq!(std::fs::read(file).unwrap(), b"keep");
}
#[cfg(unix)]
#[test]
fn req_nfr_07_keep_symlink_never_replaces_its_target_or_link() {
    let f = Fixture::new();
    let target = f.0.join("target");
    std::fs::create_dir(&target).unwrap();
    std::fs::write(target.join("sentinel"), b"preserve").unwrap();
    let link = f.0.join("keep-link");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    assert!(workspace::create_destination(&link).is_err());
    assert!(link.is_symlink());
    assert_eq!(std::fs::read(target.join("sentinel")).unwrap(), b"preserve");
}
