// SPDX-License-Identifier: AGPL-3.0-or-later
//! Gathers the build facts for `openchuck-buildinfo` (NFR-16, decision 0151) with `git` and
//! `rustc` only, offline (NFR-09). A fact it cannot find is passed on empty and reads `unknown`.

// The build script reports through cargo's `cargo:` lines on standard output.
#![allow(clippy::print_stdout)]

#[path = "src/probe.rs"]
mod probe;

use std::path::{Path, PathBuf};

/// Workspace crates that are not part of a shipped program.
const NOT_SHIPPED: &[&str] = &["openchuck-benches", "openchuck-testkit"];

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default());
    let root = manifest.join("../..");
    let lock = root.join("Cargo.lock");

    let commit = probe::commit("git", &root);
    let dirty = probe::dirty("git", &root);
    let rustc = std::env::var("RUSTC")
        .ok()
        .and_then(|rustc| probe::rustc_version(&rustc));
    let built = probe::build_time(
        std::env::var("SOURCE_DATE_EPOCH").ok().as_deref(),
        std::time::SystemTime::now(),
    );
    let crates = std::fs::read_to_string(&lock)
        .map(|text| probe::workspace_crates(&text, NOT_SHIPPED))
        .unwrap_or_default()
        .iter()
        .map(|(name, version)| format!("{name}={version}"))
        .collect::<Vec<_>>()
        .join(";");

    let set = |key: &str, value: Option<String>| {
        println!("cargo:rustc-env={key}={}", value.unwrap_or_default());
    };
    set("OPENCHUCK_BUILD_COMMIT", commit);
    set(
        "OPENCHUCK_BUILD_DIRTY",
        dirty.map(|d| if d { "yes" } else { "no" }.to_owned()),
    );
    set("OPENCHUCK_BUILD_DATE", built);
    set("OPENCHUCK_BUILD_RUSTC", rustc);
    set("OPENCHUCK_BUILD_TARGET", std::env::var("TARGET").ok());
    set("OPENCHUCK_BUILD_CRATES", Some(crates));

    rerun_when_the_tree_changes(&root);
}

/// Runs the script again when the commit or the sources change, so the commit and the dirty flag
/// follow the tree (decision 0151, OI-446).
fn rerun_when_the_tree_changes(root: &Path) {
    for path in ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml"] {
        println!("cargo:rerun-if-changed={}", root.join(path).display());
    }
    // LICENSES holds the texts the About window includes; the repository root is not watched, as
    // it holds `target/`.
    for dir in ["crates", "apps", "benches", "xtask", "LICENSES"] {
        println!("cargo:rerun-if-changed={}", root.join(dir).display());
    }
    println!("cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH");
    let git_path = |what: &str| probe::run("git", root, &["rev-parse", "--git-path", what]);
    let mut watch = vec![git_path("HEAD"), git_path("index")];
    if let Some(branch) = probe::run("git", root, &["symbolic-ref", "-q", "HEAD"]) {
        watch.push(git_path(&branch));
    }
    for path in watch.into_iter().flatten() {
        println!("cargo:rerun-if-changed={}", root.join(path).display());
    }
}
