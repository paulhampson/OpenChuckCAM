// SPDX-License-Identifier: AGPL-3.0-or-later
//! The traceability report (verification plan section 2 policy 2, work breakdown
//! slice 0 items 6 and 10).
//!
//! Tests name the requirements they cover: `req_op_05_roundtrip` covers OP-05,
//! and `req_gc_01_nfr_05_ir_round_trip` covers GC-01 and NFR-05. The report
//! maps every requirement in `docs/requirements.md` to its tests.
//!
//! A P1 requirement with no test fails the check once the last slice that
//! touches it has closed (work breakdown, traceability section). Slices are
//! closed in `xtask/traceability.toml`, which also holds the closing slice of
//! requirements that span slices.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::Findings;

/// A requirement from the requirements document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Requirement {
    /// For example `OP-05`.
    pub id: String,
    /// `P1`, `P2` or `P3`.
    pub priority: String,
}

/// The slice configuration in `xtask/traceability.toml`.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Slices whose review checklist is complete.
    pub closed_slices: BTreeSet<u32>,
    /// Requirement ID to the slice at which it closes, where that is later
    /// than the last slice listing it in the work breakdown table.
    #[serde(default)]
    pub close_at: BTreeMap<String, u32>,
}

/// Reads requirement rows (`| ID | text | P1 |`) from the requirements document.
pub fn parse_requirements(text: &str) -> Vec<Requirement> {
    let mut out = Vec::new();
    for line in text.lines() {
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        let Some(id) = cells.get(1) else { continue };
        // Section 6 suggestions (SUG-nn) are not requirements: those marked
        // Include were copied into sections 3 to 5 under their own IDs.
        if !is_requirement_id(id) || id.starts_with("SUG-") {
            continue;
        }
        if let Some(priority) = cells.iter().find(|c| matches!(**c, "P1" | "P2" | "P3")) {
            out.push(Requirement {
                id: (*id).to_owned(),
                priority: (*priority).to_owned(),
            });
        }
    }
    out
}

fn is_requirement_id(text: &str) -> bool {
    match text.split_once('-') {
        Some((prefix, number)) => {
            !prefix.is_empty()
                && prefix.chars().all(|c| c.is_ascii_uppercase())
                && !number.is_empty()
                && number.chars().all(|c| c.is_ascii_digit())
        }
        None => false,
    }
}

/// Reads the slice table under "Traceability" in the work breakdown:
/// requirement ID to the slices that list it.
pub fn parse_slices(text: &str) -> BTreeMap<String, BTreeSet<u32>> {
    let mut out: BTreeMap<String, BTreeSet<u32>> = BTreeMap::new();
    let mut in_section = false;
    for line in text.lines() {
        if line.starts_with("## ") {
            in_section = line.starts_with("## Traceability");
            continue;
        }
        if !in_section {
            continue;
        }
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        if let [_, slice, ids, ..] = cells.as_slice()
            && let Ok(slice) = slice.parse::<u32>()
        {
            for id in ids
                .split(',')
                .map(str::trim)
                .filter(|id| is_requirement_id(id))
            {
                out.entry(id.to_owned()).or_default().insert(slice);
            }
        }
    }
    out
}

/// The requirement IDs a test name covers, for example
/// `module::req_gc_01_nfr_05_round_trip` gives GC-01 and NFR-05.
pub fn tags(test_name: &str) -> Vec<String> {
    let last = test_name.rsplit("::").next().unwrap_or(test_name);
    let Some(rest) = last.strip_prefix("req_") else {
        return Vec::new();
    };
    let tokens: Vec<&str> = rest.split('_').collect();
    let mut ids = Vec::new();
    let mut i = 0;
    while let (Some(prefix), Some(number)) = (tokens.get(i), tokens.get(i + 1)) {
        let id = format!("{}-{}", prefix.to_ascii_uppercase(), number);
        if prefix.chars().all(|c| c.is_ascii_alphabetic()) && is_requirement_id(&id) {
            ids.push(id);
            i += 2;
        } else {
            break;
        }
    }
    ids
}

/// One row of the report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// Requirement ID.
    pub id: String,
    /// Priority.
    pub priority: String,
    /// Slice at which it closes, if it is mapped to one.
    pub closes_at: Option<u32>,
    /// True once that slice is closed.
    pub closed: bool,
    /// Tests that name it.
    pub tests: Vec<String>,
}

/// Builds the report rows and the findings that fail the check.
pub fn evaluate(
    requirements: &[Requirement],
    slices: &BTreeMap<String, BTreeSet<u32>>,
    config: &Config,
    test_names: &[String],
) -> (Vec<Row>, Findings) {
    let known: BTreeSet<&str> = requirements.iter().map(|r| r.id.as_str()).collect();
    let mut by_id: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut findings = Findings::new();
    for name in test_names {
        for id in tags(name) {
            if known.contains(id.as_str()) {
                by_id.entry(id).or_default().push(name.clone());
            } else {
                findings.push(format!(
                    "test {name} names {id}, which is not in docs/requirements.md"
                ));
            }
        }
    }
    let mut rows = Vec::new();
    for req in requirements {
        let closes_at = config
            .close_at
            .get(&req.id)
            .copied()
            .or_else(|| slices.get(&req.id).and_then(|s| s.last().copied()));
        let closed = closes_at.is_some_and(|s| config.closed_slices.contains(&s));
        let tests = by_id.remove(&req.id).unwrap_or_default();
        if req.priority == "P1" {
            if closes_at.is_none() {
                findings.push(format!(
                    "{} is P1 but no slice in the work breakdown traceability table covers it",
                    req.id
                ));
            } else if closed && tests.is_empty() {
                findings.push(format!(
                    "{} is P1, its last slice ({}) is closed, and no test names it",
                    req.id,
                    closes_at.unwrap_or_default()
                ));
            }
        }
        rows.push(Row {
            id: req.id.clone(),
            priority: req.priority.clone(),
            closes_at,
            closed,
            tests,
        });
    }
    (rows, findings)
}

/// Renders the report as Markdown.
pub fn render(rows: &[Row]) -> String {
    let mut out = String::from(
        "# Traceability report\n\nGenerated by `cargo xtask trace`.\n\n| Requirement | Priority | Closes at slice | Slice closed | Tests |\n| --- | --- | --- | --- | --- |\n",
    );
    for row in rows {
        let slice = row
            .closes_at
            .map_or_else(|| "-".to_owned(), |s| s.to_string());
        let tests = if row.tests.is_empty() {
            "none".to_owned()
        } else {
            row.tests
                .iter()
                .map(|t| format!("`{t}`"))
                .collect::<Vec<_>>()
                .join("<br>")
        };
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} |",
            row.id,
            row.priority,
            slice,
            if row.closed { "yes" } else { "no" },
            tests
        );
    }
    out
}

fn list_tests(root: &Path) -> Result<Vec<String>> {
    let output = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned()))
        .current_dir(root)
        .args([
            "test",
            "--workspace",
            "--all-targets",
            "--quiet",
            "--",
            "--list",
            "--format",
            "terse",
        ])
        .output()
        .context("cannot run cargo test -- --list")?;
    if !output.status.success() {
        bail!(
            "cargo test -- --list failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(parse_test_list(&String::from_utf8_lossy(&output.stdout)))
}

/// Test names from `--list --format terse` output (`name: test` lines).
pub fn parse_test_list(text: &str) -> Vec<String> {
    let mut names: Vec<String> = text
        .lines()
        .filter_map(|l| l.strip_suffix(": test"))
        .map(str::trim)
        .map(ToOwned::to_owned)
        .collect();
    names.sort();
    names.dedup();
    names
}

/// Whether `line` is a file-level feature gate, in any spacing rustfmt or a
/// person might write: `#![cfg(feature = "x")]`, `#![cfg( feature="x" )]`
/// (OI-691).
fn is_feature_gate_line(line: &str) -> bool {
    let compact: String = line.chars().filter(|c| !c.is_whitespace()).collect();
    compact.starts_with("#![cfg(feature=\"")
}

/// Names of the `#[test]` functions in a file that a file-level feature gate
/// (`#![cfg(feature = "...")]`) hides from `cargo test -- --list` in a default
/// build, sorted. A file with no such gate gives none, because cargo lists its
/// tests already.
pub fn feature_gated_tests(text: &str) -> Vec<String> {
    if !text.lines().any(is_feature_gate_line) {
        return Vec::new();
    }
    let mut names = Vec::new();
    let mut after_test = false;
    for line in text.lines().map(str::trim) {
        if line == "#[test]" {
            after_test = true;
        } else if after_test && line.starts_with("#[") {
            // Another attribute on the same function, for example `#[ignore]`.
        } else if after_test {
            after_test = false;
            let rest = line
                .strip_prefix("pub ")
                .unwrap_or(line)
                .strip_prefix("fn ");
            if let Some(name) = rest.and_then(|r| r.split('(').next()) {
                names.push(name.trim().to_owned());
            }
        }
    }
    names.sort();
    names.dedup();
    names
}

/// The tests of every integration test file under `crates/*/tests` and
/// `apps/*/tests` that a feature gate hides from the default test list (the
/// STEP kernel tests, decision 0003). Tests in these files are real `#[test]`
/// functions that the kernel CI job runs; reading them keeps the report from
/// ignoring them (OI-537).
pub fn gated_tests(root: &Path) -> Result<Vec<String>> {
    let mut names = Vec::new();
    for group in ["crates", "apps"] {
        let Ok(members) = std::fs::read_dir(root.join(group)) else {
            continue;
        };
        for member in members {
            let dir = member?.path().join("tests");
            let Ok(files) = std::fs::read_dir(&dir) else {
                continue;
            };
            for file in files {
                let path = file?.path();
                if path.extension().is_some_and(|e| e == "rs") {
                    let text = std::fs::read_to_string(&path)
                        .with_context(|| format!("cannot read {}", path.display()))?;
                    names.extend(feature_gated_tests(&text));
                }
            }
        }
    }
    names.sort();
    names.dedup();
    Ok(names)
}

/// Runs the traceability check.
pub fn check(root: &Path, tests_from: Option<&Path>, report: Option<&Path>) -> Result<Findings> {
    let requirements = parse_requirements(
        &std::fs::read_to_string(root.join("docs/requirements.md"))
            .context("cannot read docs/requirements.md")?,
    );
    let slices = parse_slices(
        &std::fs::read_to_string(root.join("docs/work-breakdown.md"))
            .context("cannot read docs/work-breakdown.md")?,
    );
    let config: Config = toml::from_str(
        &std::fs::read_to_string(root.join("xtask/traceability.toml"))
            .context("cannot read xtask/traceability.toml")?,
    )
    .context("cannot parse xtask/traceability.toml")?;
    let tests = match tests_from {
        Some(path) => parse_test_list(
            &std::fs::read_to_string(path)
                .with_context(|| format!("cannot read {}", path.display()))?,
        ),
        None => {
            // `cargo test --list` omits the tests behind the `kernel` features: add them.
            let mut all = list_tests(root)?;
            all.extend(gated_tests(root)?);
            all.sort();
            all.dedup();
            all
        }
    };
    let (rows, findings) = evaluate(&requirements, &slices, &config, &tests);
    let report_path = report.map_or_else(|| root.join("target/traceability.md"), Path::to_path_buf);
    if let Some(parent) = report_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&report_path, render(&rows))
        .with_context(|| format!("cannot write {}", report_path.display()))?;
    let p1 = rows.iter().filter(|r| r.priority == "P1").count();
    let covered = rows
        .iter()
        .filter(|r| r.priority == "P1" && !r.tests.is_empty())
        .count();
    println!(
        "trace: {covered} of {p1} P1 requirements have tests; report in {}",
        report_path.display()
    );
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reqs() -> Vec<Requirement> {
        parse_requirements(
            "| ID | Requirement | Priority |\n|---|---|---|\n| GC-01 | IR. | P1 |\n| NFR-05 | Precision. | P1 |\n| OP-05 | Simultaneous. | P1 |\n| SIM-09 | G-code sim. | P3 |\n| SUG-04 | Setup sheet. | P1 | Include |\n",
        )
    }

    fn slices() -> BTreeMap<String, BTreeSet<u32>> {
        parse_slices(
            "## Slice 0\n| 0 | not | here |\n## Traceability: P1 requirement to slice\n\n| Slice | P1 requirements | Spikes |\n| --- | --- | --- |\n| 0 | GC-01, NFR-05 | SP-3 |\n| 13 | OP-05 | SP-4 |\n",
        )
    }

    #[test]
    fn parses_requirement_rows_and_slices() {
        assert_eq!(reqs().len(), 4);
        let s = slices();
        assert_eq!(s.get("GC-01"), Some(&BTreeSet::from([0])));
        assert_eq!(s.get("OP-05"), Some(&BTreeSet::from([13])));
    }

    #[test]
    fn test_names_give_tags() {
        assert_eq!(tags("ir::req_gc_01_round_trip"), vec!["GC-01"]);
        assert_eq!(
            tags("req_op_05_op_06_gc_09_oracle"),
            vec!["OP-05", "OP-06", "GC-09"]
        );
        assert_eq!(tags("req_nfr_05"), vec!["NFR-05"]);
        assert!(tags("plain_test").is_empty());
        assert!(tags("a::requirement_free").is_empty());
    }

    #[test]
    fn req_nfr_08_traceability_flags_a_gap_in_a_closed_slice() {
        let config = Config {
            closed_slices: BTreeSet::from([0]),
            close_at: BTreeMap::new(),
        };
        let tests = vec!["t::req_gc_01_round_trip".to_owned()];
        let (_, findings) = evaluate(&reqs(), &slices(), &config, &tests);
        assert_eq!(findings.len(), 1, "{findings:?}");
        assert!(findings[0].contains("NFR-05"), "{findings:?}");
    }

    #[test]
    fn open_slices_do_not_fail() {
        let (rows, findings) = evaluate(&reqs(), &slices(), &Config::default(), &[]);
        assert!(findings.is_empty(), "{findings:?}");
        assert_eq!(rows.len(), 4);
    }

    #[test]
    fn close_at_override_delays_the_rule() {
        let config = Config {
            closed_slices: BTreeSet::from([0]),
            close_at: BTreeMap::from([("NFR-05".to_owned(), 20)]),
        };
        let tests = vec!["req_gc_01_x".to_owned()];
        let (_, findings) = evaluate(&reqs(), &slices(), &config, &tests);
        assert!(findings.is_empty(), "{findings:?}");
    }

    #[test]
    fn unknown_tag_and_unmapped_p1_fail() {
        let mut requirements = reqs();
        requirements.push(Requirement {
            id: "UI-01".to_owned(),
            priority: "P1".to_owned(),
        });
        let tests = vec!["req_zz_99_nothing".to_owned()];
        let (_, findings) = evaluate(&requirements, &slices(), &Config::default(), &tests);
        assert_eq!(findings.len(), 2, "{findings:?}");
    }

    #[test]
    fn parses_terse_list() {
        let names = parse_test_list("a::req_gc_01_x: test\nb: benchmark\nc::d: test\n");
        assert_eq!(names, vec!["a::req_gc_01_x", "c::d"]);
    }

    #[test]
    fn req_nfr_13_a_file_gated_by_a_feature_still_counts_its_tests() {
        let gated = "// header\n#![cfg(feature = \"kernel\")]\n\n#[test]\nfn req_nfr_13_exits() {}\n\n#[test]\n#[ignore = \"slow\"]\nfn req_imp_06_slow() {}\n\nfn helper() {}\n";
        assert_eq!(
            feature_gated_tests(gated),
            vec!["req_imp_06_slow", "req_nfr_13_exits"]
        );
    }

    #[test]
    fn req_nfr_13_a_gate_in_any_spacing_still_counts_its_tests() {
        for gate in [
            "#![cfg(feature=\"kernel\")]",
            "#![cfg( feature = \"kernel\" )]",
            "  #![cfg(feature =\"kernel\")]",
            "#![ cfg(feature\t=\t\"kernel\")]",
        ] {
            let text = format!("{gate}\n#[test]\nfn req_nfr_13_exits() {{}}\n");
            assert_eq!(
                feature_gated_tests(&text),
                vec!["req_nfr_13_exits"],
                "{gate}"
            );
        }
        // A gate on something other than a feature, or a commented-out gate,
        // is not a feature gate.
        let other = "#![cfg(unix)]\n#[test]\nfn a() {}\n// #![cfg(feature = \"x\")]\n";
        assert!(feature_gated_tests(other).is_empty());
    }

    #[test]
    fn req_nfr_13_an_ungated_file_adds_nothing_because_cargo_already_lists_it() {
        let plain = "#[test]\nfn req_nfr_13_exits() {}\n";
        assert!(feature_gated_tests(plain).is_empty());
        let outer_gate = "#[cfg(feature = \"kernel\")]\nmod m {\n#[test]\nfn x() {}\n}\n";
        assert!(feature_gated_tests(outer_gate).is_empty());
    }

    #[test]
    fn req_nfr_13_the_kernel_tests_of_this_repository_are_found() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let names = gated_tests(&root).expect("walk");
        assert!(
            names
                .iter()
                .any(|n| n == "req_nfr_13_the_worker_exits_cleanly"),
            "{names:?}"
        );
    }
}
