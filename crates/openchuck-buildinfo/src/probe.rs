// SPDX-License-Identifier: AGPL-3.0-or-later
//! The facts the build script gathers (NFR-16). The build script includes this file with
//! `#[path]`; the library includes it only for its tests, so the logic is tested once.
//! Nothing here touches the network (NFR-09); every probe gives `None` when it cannot answer.

#![allow(dead_code)]

use std::path::Path;
use std::process::Command;

/// Runs `program args` in `dir`; its trimmed standard output, or `None` when it cannot run or
/// exits with an error (no Git installed, not a repository).
pub fn run(program: &str, dir: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new(program)
        .args(args)
        .current_dir(dir)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

/// A full commit hash: 40 (SHA-1) or 64 (SHA-256) hexadecimal digits.
pub fn commit_from_output(output: &str) -> Option<String> {
    let hash = output.trim();
    let digits = hash.len() == 40 || hash.len() == 64;
    (digits && hash.bytes().all(|b| b.is_ascii_hexdigit())).then(|| hash.to_ascii_lowercase())
}

/// The tree has uncommitted changes when `git status --porcelain --untracked-files=no` lists
/// anything: a changed, staged, removed or renamed tracked file (decision 0191).
pub fn dirty_from_status(porcelain: &str) -> bool {
    !porcelain.trim().is_empty()
}

/// The commit of the tree at `dir`, or `None` outside a Git checkout or without Git.
pub fn commit(git: &str, dir: &Path) -> Option<String> {
    commit_from_output(&run(git, dir, &["rev-parse", "HEAD"])?)
}

/// Whether the tree at `dir` has uncommitted changes, or `None` when that cannot be told.
pub fn dirty(git: &str, dir: &Path) -> Option<bool> {
    run(git, dir, &["status", "--porcelain", "--untracked-files=no"]).map(|s| dirty_from_status(&s))
}

/// The first line of `rustc --version`.
pub fn rustc_version(rustc: &str) -> Option<String> {
    let out = run(rustc, Path::new("."), &["--version"])?;
    out.lines()
        .next()
        .map(str::to_owned)
        .filter(|l| !l.is_empty())
}

/// The packages of a `Cargo.lock` that are in the workspace (no `source` line) and named
/// `openchuck-*`, as `(name, version)`, without the crates that are never shipped.
pub fn workspace_crates(lock: &str, not_shipped: &[&str]) -> Vec<(String, String)> {
    let mut found = Vec::new();
    for block in lock.split("[[package]]") {
        let mut name = None;
        let mut version = None;
        let mut from_registry = false;
        for line in block.lines() {
            if let Some(v) = value(line, "name") {
                name = Some(v);
            } else if let Some(v) = value(line, "version") {
                version = Some(v);
            } else if line.starts_with("source = ") {
                from_registry = true;
            }
        }
        if let (Some(name), Some(version)) = (name, version)
            && !from_registry
            && name.starts_with("openchuck-")
            && !not_shipped.contains(&name.as_str())
        {
            found.push((name, version));
        }
    }
    found.sort();
    found
}

/// The quoted value of `key = "value"` on a line.
fn value(line: &str, key: &str) -> Option<String> {
    let rest = line.strip_prefix(key)?.trim_start().strip_prefix('=')?;
    Some(rest.trim().trim_matches('"').to_owned())
}

/// UTC date and time as `2026-10-19T21:07:07Z` from seconds since the Unix epoch.
pub fn iso_utc(epoch_seconds: u64) -> String {
    let days = epoch_seconds / 86_400;
    let rest = epoch_seconds % 86_400;
    // Days since 1970-01-01 to a civil date (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + u64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

/// The build time: `SOURCE_DATE_EPOCH` when set (a reproducible build), else now.
pub fn build_time(source_date_epoch: Option<&str>, now: std::time::SystemTime) -> Option<String> {
    let seconds = match source_date_epoch.and_then(|s| s.trim().parse::<u64>().ok()) {
        Some(s) => s,
        None => now.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs(),
    };
    Some(iso_utc(seconds))
}
