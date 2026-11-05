// SPDX-License-Identifier: AGPL-3.0-or-later
//! The tutorial's turned piece with adaptive spacing on (issue #462;
//! decision record 0239 items 8, 10 and 11, as amended by the PATHBUGS
//! record): helical finishing with its defaults makes a complete path, not
//! a partial one that must be acknowledged before export (OP-04, NFR-07,
//! NFR-12).
#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::{Path, PathBuf};

use openchuck_app::{Application, Threads};

/// A copy of the example project in a fresh folder, with the Finish
/// operation's adaptive spacing set as `adaptive`.
fn example(tag: &str, adaptive: bool) -> PathBuf {
    let from = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/examples/baluster/baluster-project");
    let dir = std::env::temp_dir().join(format!(
        "openchuck-adaptive-example-{tag}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    copy(&from, &dir);
    let finish = dir.join("operations/34e3aa5e-36cc-4614-be6d-ec39cf22f5f8.toml");
    let text = std::fs::read_to_string(&finish).unwrap();
    assert!(text.contains("strategy = \"helical_finishing\""));
    let text = text.replace(
        "adaptive_spacing = false",
        &format!("adaptive_spacing = {adaptive}"),
    );
    std::fs::write(&finish, text).unwrap();
    dir
}

fn copy(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).unwrap();
        }
    }
}

#[test]
fn req_op_04_req_nfr_12_the_tutorial_piece_finishes_completely_with_adaptive_spacing() {
    // Before: the query limit was met with half of it unused, and places
    // beside the flutes' edges were left unverified, so the path was
    // partial (issue #462).
    let dir = example("on", true);
    let app = Application::open(&dir, Threads::Exactly(4)).unwrap();
    let run = app.regenerate().unwrap();
    let partial = openchuck_app::partial_operations(app.project(), &run);
    let warnings: Vec<&str> = run
        .groups
        .iter()
        .flat_map(|g| &g.warnings)
        .filter(|w| w.requires_acknowledgement)
        .map(|w| w.message.as_str())
        .collect();
    assert!(partial.is_empty(), "{partial:?}: {warnings:#?}");
    assert!(
        run.notes
            .iter()
            .any(|n| n.contains("Finish") && n.contains("passes added")),
        "{:#?}",
        run.notes
    );
    let _ = std::fs::remove_dir_all(&dir);
}
