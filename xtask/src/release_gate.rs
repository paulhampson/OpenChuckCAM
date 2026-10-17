// SPDX-License-Identifier: AGPL-3.0-or-later
//! `cargo xtask release-gate`: the release gate (work breakdown slice 20 item 9;
//! verification plan section 7, first-release exit criteria; decision records
//! 0031 and 0091).
//!
//! Four checks must pass before a release is published:
//!
//! - `deps`: the dependency rules (architecture sections 4 and 12; NFR-01, NFR-09).
//! - `deny`: `cargo deny check` over licences, advisories, bans and sources (NFR-10, NFR-14).
//! - `golden`: every golden-file test (NFR-08): the generated G-code and toolpaths compared byte
//!   for byte with the committed files, and the golden audit of those files against the exact
//!   parts (GC-09, GC-10). The STEP `drilling` reference job is included, so the kernel is built.
//! - `replay`: gate 3, every LinuxCNC golden program in the LinuxCNC simulator (GC-10).
//!
//! Every check runs, even after one fails, so one run reports everything that blocks the
//! release. A check that cannot run (a missing tool, a build failure) fails the gate like a check
//! that finds a problem. There is no option to skip or soften a check; the release workflow's
//! build and publish jobs need this gate's job.
//!
//! With `--tag`, the gate also checks that the tag is a release tag (`lib-v<version>` or
//! `app-v<version>`, decision 0031 item 2) whose version is the workspace version.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::{Findings, replay};

/// How the gate runs LinuxCNC, and the tag being released.
#[derive(Debug)]
pub struct Options {
    /// Run LinuxCNC in this Docker image (see `docker/linuxcnc`).
    pub docker: Option<String>,
    /// Keep each simulator report in this folder.
    pub keep_reports: Option<PathBuf>,
    /// The tag being released, checked against the workspace version.
    pub tag: Option<String>,
}

/// One check of the gate: its name and what it found, or why it could not run.
type Step<'a> = (&'static str, Box<dyn Fn() -> Result<Findings> + 'a>);

/// The steps of the release gate for `root`.
fn steps<'a>(root: &'a Path, options: &'a Options) -> Vec<Step<'a>> {
    let mut steps: Vec<Step<'a>> = Vec::new();
    if let Some(tag) = &options.tag {
        steps.push(("tag", Box::new(move || tag_check(root, tag))));
    }
    steps.push(("deps", Box::new(move || crate::deps::check(root))));
    steps.push(("deny", Box::new(move || deny(root))));
    steps.push(("golden", Box::new(move || golden(root))));
    steps.push((
        "replay",
        Box::new(move || {
            replay::check(
                root,
                &replay::Options {
                    programs: Vec::new(),
                    docker: options.docker.clone(),
                    keep_reports: options.keep_reports.clone(),
                    tolerance: replay::TOLERANCE,
                    timeout_seconds: replay::TIMEOUT_SECONDS,
                    shard: None,
                },
            )
        }),
    ));
    steps
}

/// Runs the release gate and returns each check's findings; the gate passes only if every list
/// is empty.
pub fn check(root: &Path, options: &Options) -> Vec<(&'static str, Findings)> {
    run(&steps(root, options))
}

/// Runs every step, turning a step that could not run into a finding, so it fails the gate.
fn run(steps: &[Step<'_>]) -> Vec<(&'static str, Findings)> {
    steps
        .iter()
        .map(|(name, step)| {
            println!("release gate: {name}");
            let findings = match step() {
                Ok(findings) => findings,
                Err(error) => vec![format!("could not run: {error:#}")],
            };
            (*name, findings)
        })
        .collect()
}

/// `cargo deny check` with the configuration in `deny.toml`, as in pull request CI.
fn deny(root: &Path) -> Result<Findings> {
    let output = Command::new("cargo")
        .args(["deny", "--all-features", "check"])
        .current_dir(root)
        .output()
        .context("cannot run cargo deny")?;
    if output.status.success() {
        return Ok(Vec::new());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.contains("no such command") {
        bail!("cargo-deny is not installed (cargo install --locked cargo-deny)");
    }
    let errors: Vec<String> = stderr
        .lines()
        .filter(|l| l.starts_with("error"))
        .take(10)
        .map(str::to_owned)
        .collect();
    if errors.is_empty() {
        Ok(vec![format!(
            "cargo deny failed: {}",
            last_lines(&stderr, 6)
        )])
    } else {
        Ok(errors)
    }
}

/// Every test whose name says `golden`, with the kernel, so the STEP reference job runs too.
fn golden(root: &Path) -> Result<Findings> {
    let output = Command::new("cargo")
        .args([
            "test",
            "--workspace",
            "--locked",
            "--features",
            "openchuck-app/kernel-tests",
            "--no-fail-fast",
            "--",
            "golden",
        ])
        .current_dir(root)
        .output()
        .context("cannot run cargo test")?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    Ok(golden_findings(output.status.success(), &stdout, &stderr))
}

/// What the golden test run found: its failures, or that it ran no golden test at all (the
/// filter must not pass by matching nothing).
fn golden_findings(success: bool, stdout: &str, stderr: &str) -> Findings {
    let passed = stdout
        .lines()
        .filter(|l| l.starts_with("test ") && l.contains("golden") && l.ends_with("... ok"))
        .count();
    let mut findings: Findings = stdout
        .lines()
        .filter(|l| l.starts_with("test ") && l.ends_with("... FAILED"))
        .map(|l| format!("failed: {}", l.trim_start_matches("test ")))
        .collect();
    if !success && findings.is_empty() {
        findings.push(format!(
            "the golden tests did not run: {}",
            last_lines(stderr, 6)
        ));
    }
    if success && passed == 0 {
        findings.push("no golden-file test ran".to_owned());
    } else {
        println!("golden: {passed} golden-file test(s) passed");
    }
    findings
}

/// The last `n` non-empty lines of `text`, joined for a one-line finding.
fn last_lines(text: &str, n: usize) -> String {
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    lines
        .get(lines.len().saturating_sub(n)..)
        .unwrap_or_default()
        .join(" | ")
}

#[derive(Debug, Deserialize)]
struct Manifest {
    workspace: Workspace,
}

#[derive(Debug, Deserialize)]
struct Workspace {
    package: Package,
}

#[derive(Debug, Deserialize)]
struct Package {
    version: String,
}

/// The release tag must name the workspace version.
fn tag_check(root: &Path, tag: &str) -> Result<Findings> {
    let path = root.join("Cargo.toml");
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("cannot read '{}'", path.display()))?;
    let manifest: Manifest = toml::from_str(&text).context("cannot read the workspace version")?;
    Ok(tag_findings(tag, &manifest.workspace.package.version))
}

/// Checks `tag` against the workspace `version`.
fn tag_findings(tag: &str, version: &str) -> Findings {
    let Some(tagged) = tag
        .strip_prefix("lib-v")
        .or_else(|| tag.strip_prefix("app-v"))
    else {
        return vec![format!(
            "'{tag}' is not a release tag: use lib-v<version> or app-v<version> (decision 0031)"
        )];
    };
    if tagged == version {
        Vec::new()
    } else {
        vec![format!(
            "the tag '{tag}' names version {tagged}, but the workspace version is {version}"
        )]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The checks the gate runs, in order (slice 20 item 9). A change to `steps` that drops one
    /// fails `the_release_gate_runs_every_check`.
    const CHECKS: &[&str] = &["deps", "deny", "golden", "replay"];

    /// A step that passes.
    fn pass() -> Box<dyn Fn() -> Result<Findings>> {
        Box::new(|| Ok(Vec::new()))
    }

    fn blocked(results: &[(&str, Findings)]) -> bool {
        results.iter().any(|(_, f)| !f.is_empty())
    }

    #[test]
    fn req_gc_09_req_gc_10_req_nfr_08_a_failing_check_blocks_the_release_gate() {
        // Every check passes: the gate passes.
        let all_pass: Vec<Step<'_>> = CHECKS.iter().map(|name| (*name, pass())).collect();
        assert!(!blocked(&run(&all_pass)));
        // Each check in turn finds a problem, or cannot run: the gate fails, names that check,
        // and still runs the checks after it.
        for failing in CHECKS {
            for error in [false, true] {
                let steps: Vec<Step<'_>> = CHECKS
                    .iter()
                    .map(|name| -> Step<'_> {
                        if name != failing {
                            return (*name, pass());
                        }
                        if error {
                            (*name, Box::new(|| bail!("cargo-deny is not installed")))
                        } else {
                            (
                                *name,
                                Box::new(|| Ok(vec!["deliberate failure".to_owned()])),
                            )
                        }
                    })
                    .collect();
                let results = run(&steps);
                assert!(blocked(&results), "{failing} failing did not block");
                assert_eq!(results.len(), CHECKS.len(), "a check was not run");
                for (name, findings) in &results {
                    assert_eq!(findings.is_empty(), name != failing, "{name}");
                }
            }
        }
    }

    #[test]
    fn req_gc_09_req_gc_10_req_nfr_08_the_release_gate_runs_every_check() {
        let root = Path::new(".");
        let options = Options {
            docker: None,
            keep_reports: None,
            tag: None,
        };
        let names: Vec<&str> = steps(root, &options).iter().map(|(n, _)| *n).collect();
        assert_eq!(names, CHECKS);
        let tagged = Options {
            tag: Some("app-v0.1.0".to_owned()),
            ..options
        };
        let names: Vec<&str> = steps(root, &tagged).iter().map(|(n, _)| *n).collect();
        assert_eq!(names.first(), Some(&"tag"));
        assert_eq!(names.get(1..), Some(CHECKS));
    }

    #[test]
    fn req_nfr_08_the_golden_check_fails_on_a_failed_test_or_on_no_test() {
        let ok = "test req_nfr_08_x_matches_its_golden_file ... ok\n";
        assert!(golden_findings(true, ok, "").is_empty());
        // The filter matched nothing.
        assert!(!golden_findings(true, "running 0 tests\n", "").is_empty());
        let failed = "test req_nfr_08_x_matches_its_golden_file ... FAILED\n";
        let findings = golden_findings(false, failed, "");
        assert_eq!(findings.len(), 1);
        assert!(findings.iter().all(|f| f.contains("x_matches")));
        // A build failure runs no test and fails too.
        assert!(!golden_findings(false, "", "error[E0425]: cannot find value").is_empty());
    }

    #[test]
    fn release_tag_must_name_the_workspace_version() {
        assert!(tag_findings("lib-v0.8.0", "0.8.0").is_empty());
        assert!(tag_findings("app-v0.8.0", "0.8.0").is_empty());
        for bad in ["app-v0.7.0", "v0.8.0", "nightly", "app-v", "lib-v0.8.0-rc"] {
            assert!(
                !tag_findings(bad, "0.8.0").is_empty(),
                "'{bad}' was accepted"
            );
        }
    }
}
