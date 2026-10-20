// SPDX-License-Identifier: AGPL-3.0-or-later
//! Which build is running (NFR-16, decision record 0151): the commit, whether the build tree had
//! uncommitted changes, the build date, the Rust compiler and target, and the version of each
//! workspace crate. The About window and `openchuck-cli --version` show the same text.
//!
//! Only the two front ends use this crate. The library family does not, so build data never
//! reaches G-code, golden files or project files (GC-07, NFR-06).

use std::fmt::Write as _;
use std::sync::OnceLock;

#[cfg(test)]
mod probe;

/// What a fact reads when the build could not find it (no Git directory in a source archive, no
/// Git installed).
pub const UNKNOWN: &str = "unknown";

/// The name of the program in the build text.
const PRODUCT: &str = "OpenChuckCAM";

/// One crate of the workspace and its version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrateVersion {
    /// The package name.
    pub name: String,
    /// Its version.
    pub version: String,
}

/// The facts about one build. `None` means the build could not find the fact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildInfo {
    /// The application version (the workspace version).
    pub version: String,
    /// The full commit hash.
    pub commit: Option<String>,
    /// Whether the tree had uncommitted changes.
    pub dirty: Option<bool>,
    /// The build time, UTC, as `2026-10-19T21:07:07Z`.
    pub built: Option<String>,
    /// The compiler's version line.
    pub rustc: Option<String>,
    /// The target triple.
    pub target: Option<String>,
    /// The workspace crates, in name order.
    pub crates: Vec<CrateVersion>,
}

impl BuildInfo {
    /// The facts of this build, from the build script.
    #[must_use]
    pub fn current() -> Self {
        let present = |s: &str| (!s.is_empty()).then(|| s.to_owned());
        Self {
            version: env!("CARGO_PKG_VERSION").to_owned(),
            commit: present(env!("OPENCHUCK_BUILD_COMMIT")),
            dirty: match env!("OPENCHUCK_BUILD_DIRTY") {
                "yes" => Some(true),
                "no" => Some(false),
                _ => None,
            },
            built: present(env!("OPENCHUCK_BUILD_DATE")),
            rustc: present(env!("OPENCHUCK_BUILD_RUSTC")),
            target: present(env!("OPENCHUCK_BUILD_TARGET")),
            crates: parse_crates(env!("OPENCHUCK_BUILD_CRATES")),
        }
    }

    /// The build information as plain text, one fact per line, in a fixed order. This is what
    /// the About window's Copy puts on the clipboard and what `--version` prints.
    #[must_use]
    pub fn text(&self) -> String {
        let unknown = |v: &Option<String>| v.clone().unwrap_or_else(|| UNKNOWN.to_owned());
        let dirty = match self.dirty {
            Some(true) => "yes",
            Some(false) => "no",
            None => UNKNOWN,
        };
        let mut out = format!("{PRODUCT} {}\n", self.version);
        let _ = writeln!(out, "Commit: {}", unknown(&self.commit));
        let _ = writeln!(out, "Uncommitted changes: {dirty}");
        let _ = writeln!(out, "Built: {}", unknown(&self.built));
        let _ = writeln!(out, "Rust: {}", unknown(&self.rustc));
        let _ = writeln!(out, "Target: {}", unknown(&self.target));
        out.push_str("Crates:\n");
        for c in &self.crates {
            let _ = writeln!(out, "  {} {}", c.name, c.version);
        }
        out
    }
}

/// The text of this build, made once.
#[must_use]
pub fn current_text() -> &'static str {
    static TEXT: OnceLock<String> = OnceLock::new();
    TEXT.get_or_init(|| BuildInfo::current().text())
}

/// `name=version;name=version` from the build script.
fn parse_crates(list: &str) -> Vec<CrateVersion> {
    list.split(';')
        .filter_map(|pair| pair.split_once('='))
        .map(|(name, version)| CrateVersion {
            name: name.to_owned(),
            version: version.to_owned(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]

    use std::path::Path;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::probe;

    const HASH: &str = "0123456789abcdef0123456789abcdef01234567";

    fn fixture() -> BuildInfo {
        BuildInfo {
            version: "1.2.3".to_owned(),
            commit: Some(HASH.to_owned()),
            dirty: Some(true),
            built: Some("2026-10-19T21:07:07Z".to_owned()),
            rustc: Some("rustc 1.97.0 (abcdef012 2026-09-01)".to_owned()),
            target: Some("x86_64-unknown-linux-gnu".to_owned()),
            crates: vec![
                CrateVersion {
                    name: "openchuck-types".to_owned(),
                    version: "1.2.3".to_owned(),
                },
                CrateVersion {
                    name: "openchuck-ui".to_owned(),
                    version: "1.2.3".to_owned(),
                },
            ],
        }
    }

    /// NFR-16: the build text has every fact, one per line, in a fixed order.
    #[test]
    fn req_nfr_16_text_from_a_fixed_fixture() {
        let expected = format!(
            "OpenChuckCAM 1.2.3\n\
             Commit: {HASH}\n\
             Uncommitted changes: yes\n\
             Built: 2026-10-19T21:07:07Z\n\
             Rust: rustc 1.97.0 (abcdef012 2026-09-01)\n\
             Target: x86_64-unknown-linux-gnu\n\
             Crates:\n  openchuck-types 1.2.3\n  openchuck-ui 1.2.3\n"
        );
        assert_eq!(fixture().text(), expected);
    }

    /// NFR-16: a fact the build could not find reads "unknown", it is not left out.
    #[test]
    fn req_nfr_16_missing_facts_read_unknown() {
        let info = BuildInfo {
            commit: None,
            dirty: None,
            built: None,
            rustc: None,
            target: None,
            crates: Vec::new(),
            ..fixture()
        };
        let text = info.text();
        for line in [
            "Commit: unknown",
            "Uncommitted changes: unknown",
            "Built: unknown",
            "Rust: unknown",
            "Target: unknown",
        ] {
            assert!(text.contains(line), "{line} in {text}");
        }
        let clean = BuildInfo {
            dirty: Some(false),
            ..fixture()
        };
        assert!(clean.text().contains("Uncommitted changes: no"));
    }

    /// NFR-16: this build reports its own version and the facts the script found.
    #[test]
    fn req_nfr_16_current_build_reports_the_workspace() {
        let info = BuildInfo::current();
        assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
        assert_eq!(current_text(), info.text());
        let names: Vec<_> = info.crates.iter().map(|c| c.name.as_str()).collect();
        for family in [
            "openchuck-types",
            "openchuck-path",
            "openchuck-post",
            "openchuck-gcode",
            "openchuck-app",
            "openchuck-cli",
            "openchuck-ui",
            "openchuck-buildinfo",
        ] {
            assert!(names.contains(&family), "{family} in {names:?}");
        }
        assert!(!names.contains(&"openchuck-testkit"));
        assert!(info.target.is_some_and(|t| t.contains('-')));
        assert!(info.rustc.is_some_and(|r| r.starts_with("rustc ")));
    }

    /// NFR-16: the dirty flag is true exactly when `git status --porcelain` lists something.
    #[test]
    fn req_nfr_16_dirty_flag_logic() {
        assert!(!probe::dirty_from_status(""));
        assert!(!probe::dirty_from_status("\n  \n"));
        assert!(probe::dirty_from_status(
            " M crates/openchuck-path/src/lib.rs\n"
        ));
        assert!(probe::dirty_from_status("?? new_file.rs\n"));
    }

    /// NFR-16: only a full hexadecimal hash is taken as the commit.
    #[test]
    fn req_nfr_16_commit_must_be_a_full_hash() {
        assert_eq!(
            probe::commit_from_output(&format!("{HASH}\n")).as_deref(),
            Some(HASH)
        );
        let sha256 = "a".repeat(64);
        assert_eq!(probe::commit_from_output(&sha256), Some(sha256));
        assert_eq!(probe::commit_from_output("HEAD"), None);
        assert_eq!(probe::commit_from_output(""), None);
        assert_eq!(probe::commit_from_output(&"g".repeat(40)), None);
    }

    /// NFR-16: with no Git installed, or outside a checkout, the facts are None (so "unknown")
    /// and nothing fails; for example a source archive has no `.git` directory.
    #[test]
    fn req_nfr_16_fallback_without_git() {
        let here = Path::new(env!("CARGO_MANIFEST_DIR"));
        assert_eq!(probe::commit("openchuck-no-such-git", here), None);
        assert_eq!(probe::dirty("openchuck-no-such-git", here), None);
        // A folder outside any repository.
        let dir = std::env::temp_dir().join(format!("openchuck-nogit-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        if probe::run("git", &dir, &["rev-parse", "--git-dir"]).is_none() {
            assert_eq!(probe::commit("git", &dir), None);
            assert_eq!(probe::dirty("git", &dir), None);
        }
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(probe::rustc_version("openchuck-no-such-rustc"), None);
    }

    /// NFR-16: the crate list comes from the workspace packages of Cargo.lock only.
    #[test]
    fn req_nfr_16_crate_versions_from_the_lock() {
        let lock = "version = 4\n\n\
            [[package]]\nname = \"openchuck-types\"\nversion = \"0.10.0\"\ndependencies = [\"serde\"]\n\n\
            [[package]]\nname = \"openchuck-testkit\"\nversion = \"0.10.0\"\n\n\
            [[package]]\nname = \"openchuck-other\"\nversion = \"9.9.9\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\n\n\
            [[package]]\nname = \"serde\"\nversion = \"1.0.229\"\nsource = \"registry+https://x\"\n\n\
            [[package]]\nname = \"openchuck-app\"\nversion = \"0.10.0\"\n";
        let found = probe::workspace_crates(lock, &["openchuck-testkit"]);
        assert_eq!(
            found,
            vec![
                ("openchuck-app".to_owned(), "0.10.0".to_owned()),
                ("openchuck-types".to_owned(), "0.10.0".to_owned()),
            ]
        );
        assert!(probe::workspace_crates("", &[]).is_empty());
        assert_eq!(
            parse_crates("a=1;b=2"),
            vec![
                CrateVersion {
                    name: "a".to_owned(),
                    version: "1".to_owned()
                },
                CrateVersion {
                    name: "b".to_owned(),
                    version: "2".to_owned()
                },
            ]
        );
        assert!(parse_crates("").is_empty());
    }

    /// NFR-16 and NFR-06: the build time is UTC, and `SOURCE_DATE_EPOCH` makes it reproducible.
    #[test]
    fn req_nfr_16_build_time_is_utc_and_reproducible() {
        assert_eq!(probe::iso_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(probe::iso_utc(1_781_903_227), "2026-06-19T21:07:07Z");
        assert_eq!(probe::iso_utc(951_782_400), "2000-02-29T00:00:00Z");
        let now = UNIX_EPOCH + Duration::from_secs(86_400);
        assert_eq!(
            probe::build_time(Some("1781903227"), now).as_deref(),
            Some("2026-06-19T21:07:07Z")
        );
        // An unusable value falls back to the clock.
        assert_eq!(
            probe::build_time(Some("soon"), now).as_deref(),
            Some("1970-01-02T00:00:00Z")
        );
        assert_eq!(
            probe::build_time(None, now).as_deref(),
            Some("1970-01-02T00:00:00Z")
        );
        // A clock before the epoch gives no date.
        let before = SystemTime::UNIX_EPOCH - Duration::from_secs(5);
        assert_eq!(probe::build_time(None, before), None);
    }
}
