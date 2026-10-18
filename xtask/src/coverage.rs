// SPDX-License-Identifier: AGPL-3.0-or-later
//! Line coverage per crate against the development target (verification plan section 8).
//!
//! `cargo xtask coverage` reads the lcov file the Coverage job writes (`lcov.info`), adds up
//! the `LF` (lines with data) and `LH` (lines hit) records of each crate's `src/` files, and
//! prints a table. The crates, the target and the temporary per-crate floors are in
//! `xtask/coverage.toml`. With `--enforce` (and `enforce = true` in that file) a gated crate
//! under its threshold is a finding, which fails the nightly run; without it the table is a
//! report. A crate in the lcov file that the configuration does not list, and a gated crate
//! with no data, are findings in both modes, so a new crate or a broken report cannot pass
//! unnoticed.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::Findings;

/// The configuration in `xtask/coverage.toml`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Line coverage every gated crate should reach, in percent.
    pub target: f64,
    /// Whether `--enforce` may fail the run; false while the first measurement is read.
    pub enforce: bool,
    /// Crates that must reach their threshold.
    pub gated: Vec<String>,
    /// Crates that are listed in the report but never fail it.
    pub report_only: Vec<String>,
    /// Crates left out of the report (test support code).
    pub excluded: Vec<String>,
    /// A temporary threshold for a gated crate that is under the target, in percent.
    #[serde(default)]
    pub floors: BTreeMap<String, f64>,
}

/// Lines with data and lines hit, summed over a crate's `src/` files.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Lines {
    /// `LF`: lines with coverage data.
    pub found: u64,
    /// `LH`: lines executed at least once.
    pub hit: u64,
}

/// The crate directory a source file belongs to: `crates/<name>/src/...` or `apps/<name>/src/...`.
fn crate_of(path: &str) -> Option<String> {
    let normalised = path.replace('\\', "/");
    let parts: Vec<&str> = normalised.split('/').collect();
    parts.windows(3).find_map(|window| match window {
        [group, name, "src"] if matches!(*group, "crates" | "apps") => Some((*name).to_owned()),
        _ => None,
    })
}

/// Adds up `LF` and `LH` per crate from lcov text; files outside a crate's `src/` are skipped.
pub fn parse_lcov(text: &str) -> BTreeMap<String, Lines> {
    let mut out: BTreeMap<String, Lines> = BTreeMap::new();
    let mut current: Option<String> = None;
    for line in text.lines() {
        let line = line.trim();
        if let Some(file) = line.strip_prefix("SF:") {
            current = crate_of(file);
        } else if let Some(value) = line.strip_prefix("LF:") {
            if let (Some(name), Ok(n)) = (&current, value.parse::<u64>()) {
                out.entry(name.clone()).or_default().found += n;
            }
        } else if let Some(value) = line.strip_prefix("LH:") {
            if let (Some(name), Ok(n)) = (&current, value.parse::<u64>()) {
                out.entry(name.clone()).or_default().hit += n;
            }
        } else if line == "end_of_record" {
            current = None;
        }
    }
    out
}

/// What the table says about one crate.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    /// Crate directory name.
    pub name: String,
    /// Lines with data and lines hit.
    pub lines: Lines,
    /// The threshold that applies (target or floor); none for report-only crates.
    pub threshold: Option<f64>,
    /// Short status for the table.
    pub status: &'static str,
}

impl Row {
    /// Line coverage in percent, if the crate has data.
    pub fn percent(&self) -> Option<f64> {
        (self.lines.found > 0).then(|| 100.0 * self.lines.hit as f64 / self.lines.found as f64)
    }
}

fn below(lines: Lines, threshold: f64) -> bool {
    (lines.hit as f64) * 100.0 < threshold * (lines.found as f64)
}

/// Builds the rows and findings. `enforcing` makes a shortfall a finding.
pub fn evaluate(
    config: &Config,
    data: &BTreeMap<String, Lines>,
    enforcing: bool,
) -> (Vec<Row>, Findings) {
    let mut findings = Findings::new();
    let mut rows = Vec::new();
    let listed = |name: &str| {
        config
            .gated
            .iter()
            .chain(&config.report_only)
            .chain(&config.excluded)
            .any(|c| c == name)
    };
    for name in data.keys().filter(|n| !listed(n)) {
        findings.push(format!(
            "{name} has coverage data but is not listed in xtask/coverage.toml (add it to gated, report_only or excluded)"
        ));
    }
    for name in &config.gated {
        if config.excluded.contains(name) || config.report_only.contains(name) {
            findings.push(format!(
                "{name} is gated and also listed as excluded or report_only"
            ));
        }
    }
    for name in config.floors.keys() {
        if !config.gated.contains(name) {
            findings.push(format!(
                "a floor is set for {name}, which is not a gated crate"
            ));
        }
    }
    for name in &config.gated {
        let lines = data.get(name).copied().unwrap_or_default();
        let threshold = config.floors.get(name).copied().unwrap_or(config.target);
        let status = if lines.found == 0 {
            findings.push(format!(
                "no line data for gated crate {name}: the lcov file does not cover it"
            ));
            "no data"
        } else if below(lines, threshold) {
            if enforcing {
                findings.push(format!(
                    "{name}: line coverage {:.2}% ({} of {} lines) is below its threshold {threshold:.1}%",
                    100.0 * lines.hit as f64 / lines.found as f64,
                    lines.hit,
                    lines.found
                ));
                "BELOW"
            } else {
                "below (not enforced)"
            }
        } else {
            "ok"
        };
        rows.push(Row {
            name: name.clone(),
            lines,
            threshold: Some(threshold),
            status,
        });
    }
    for name in &config.report_only {
        let lines = data.get(name).copied().unwrap_or_default();
        rows.push(Row {
            name: name.clone(),
            lines,
            threshold: None,
            status: "report only",
        });
    }
    (rows, findings)
}

/// The Markdown table for the run summary and the issue body.
pub fn render(config: &Config, rows: &[Row], enforcing: bool) -> String {
    let mut out = format!(
        "## Line coverage per crate\n\nTarget {:.1}% of lines in each gated crate; enforcement is {}.\n\n| Crate | Lines | Covered | Line coverage | Threshold | Status |\n| --- | ---: | ---: | ---: | ---: | --- |\n",
        config.target,
        if enforcing { "on" } else { "off" }
    );
    for row in rows {
        let percent = row
            .percent()
            .map_or_else(|| "-".to_owned(), |p| format!("{p:.2}%"));
        let threshold = row
            .threshold
            .map_or_else(|| "-".to_owned(), |t| format!("{t:.1}%"));
        out.push_str(&format!(
            "| {} | {} | {} | {percent} | {threshold} | {} |\n",
            row.name, row.lines.found, row.lines.hit, row.status
        ));
    }
    out
}

/// Runs the check on `lcov` (default `lcov.info` at the workspace root).
pub fn check(
    root: &Path,
    lcov: Option<&Path>,
    report: Option<&Path>,
    enforce: bool,
) -> Result<Findings> {
    let config: Config = toml::from_str(
        &std::fs::read_to_string(root.join("xtask/coverage.toml"))
            .context("cannot read xtask/coverage.toml")?,
    )
    .context("cannot parse xtask/coverage.toml")?;
    let lcov_path = lcov.map_or_else(|| root.join("lcov.info"), Path::to_path_buf);
    let data = parse_lcov(
        &std::fs::read_to_string(&lcov_path)
            .with_context(|| format!("cannot read {}", lcov_path.display()))?,
    );
    let enforcing = enforce && config.enforce;
    if enforce && !config.enforce {
        println!("coverage: enforcement is off in xtask/coverage.toml; reporting only");
    }
    let (rows, findings) = evaluate(&config, &data, enforcing);
    let text = render(&config, &rows, enforcing);
    print!("{text}");
    let report_path = report.map_or_else(
        || root.join("target/coverage-summary.md"),
        Path::to_path_buf,
    );
    if let Some(parent) = report_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&report_path, &text)
        .with_context(|| format!("cannot write {}", report_path.display()))?;
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> Config {
        Config {
            target: 95.0,
            enforce: true,
            gated: vec!["openchuck-path".to_owned(), "openchuck-types".to_owned()],
            report_only: vec!["openchuck-ui".to_owned()],
            excluded: vec!["openchuck-testkit".to_owned()],
            floors: BTreeMap::new(),
        }
    }

    fn lcov(records: &[(&str, u64, u64)]) -> String {
        records
            .iter()
            .map(|(file, found, hit)| format!("SF:{file}\nLF:{found}\nLH:{hit}\nend_of_record\n"))
            .collect()
    }

    const PATH_A: &str = "/home/runner/work/o/o/crates/openchuck-path/src/a.rs";
    const PATH_B: &str = "/home/runner/work/o/o/crates/openchuck-path/src/b/c.rs";
    const TYPES: &str = "/home/runner/work/o/o/crates/openchuck-types/src/lib.rs";
    const UI: &str = "/home/runner/work/o/o/apps/openchuck-ui/src/main.rs";

    #[test]
    fn lines_of_one_crate_are_added_up_across_files() {
        let data = parse_lcov(&lcov(&[
            (PATH_A, 100, 90),
            (PATH_B, 50, 50),
            (TYPES, 10, 10),
        ]));
        assert_eq!(
            data.get("openchuck-path"),
            Some(&Lines {
                found: 150,
                hit: 140
            })
        );
        assert_eq!(
            data.get("openchuck-types"),
            Some(&Lines { found: 10, hit: 10 })
        );
    }

    #[test]
    fn files_outside_a_crates_src_directory_are_skipped() {
        let data = parse_lcov(&lcov(&[
            (
                "/home/runner/work/o/o/crates/openchuck-path/tests/x.rs",
                40,
                40,
            ),
            ("/home/runner/work/o/o/xtask/src/main.rs", 40, 0),
            (
                "/home/runner/.cargo/registry/src/index.crates.io-1/serde-1/src/lib.rs",
                9,
                9,
            ),
            (PATH_A, 10, 5),
        ]));
        assert_eq!(data.len(), 1);
        assert_eq!(
            data.get("openchuck-path"),
            Some(&Lines { found: 10, hit: 5 })
        );
    }

    #[test]
    fn windows_paths_are_read_too() {
        let data = parse_lcov(&lcov(&[(
            r"D:\a\o\o\crates\openchuck-types\src\lib.rs",
            4,
            4,
        )]));
        assert_eq!(
            data.get("openchuck-types"),
            Some(&Lines { found: 4, hit: 4 })
        );
    }

    #[test]
    fn a_crate_exactly_at_the_target_passes() {
        let data = parse_lcov(&lcov(&[(PATH_A, 100, 95), (TYPES, 20, 19)]));
        let (rows, findings) = evaluate(&config(), &data, true);
        assert!(findings.is_empty(), "{findings:?}");
        assert!(
            rows.iter()
                .all(|r| r.status == "ok" || r.status == "report only")
        );
    }

    #[test]
    fn a_crate_under_the_target_is_a_finding_only_when_enforcing() {
        let data = parse_lcov(&lcov(&[(PATH_A, 100, 94), (TYPES, 20, 20)]));
        let (_, enforced) = evaluate(&config(), &data, true);
        assert_eq!(enforced.len(), 1, "{enforced:?}");
        assert!(enforced.iter().all(|f| f.contains("openchuck-path")));
        let (rows, reported) = evaluate(&config(), &data, false);
        assert!(reported.is_empty(), "{reported:?}");
        assert!(rows.iter().any(|r| r.status == "below (not enforced)"));
    }

    #[test]
    fn a_floor_replaces_the_target_for_its_crate() {
        let mut with_floor = config();
        with_floor.floors.insert("openchuck-path".to_owned(), 90.0);
        let data = parse_lcov(&lcov(&[(PATH_A, 100, 91), (TYPES, 20, 20)]));
        assert!(evaluate(&with_floor, &data, true).1.is_empty());
        let falling = parse_lcov(&lcov(&[(PATH_A, 100, 89), (TYPES, 20, 20)]));
        assert_eq!(evaluate(&with_floor, &falling, true).1.len(), 1);
    }

    #[test]
    fn report_only_crates_never_fail_and_excluded_crates_are_not_listed() {
        let data = parse_lcov(&lcov(&[
            (PATH_A, 10, 10),
            (TYPES, 10, 10),
            (UI, 100, 1),
            ("/w/crates/openchuck-testkit/src/lib.rs", 10, 0),
        ]));
        let (rows, findings) = evaluate(&config(), &data, true);
        assert!(findings.is_empty(), "{findings:?}");
        assert!(
            rows.iter()
                .any(|r| r.name == "openchuck-ui" && r.status == "report only")
        );
        assert!(rows.iter().all(|r| r.name != "openchuck-testkit"));
    }

    #[test]
    fn an_unlisted_crate_and_a_gated_crate_without_data_are_findings_in_both_modes() {
        let data = parse_lcov(&lcov(&[
            (PATH_A, 10, 10),
            ("/w/crates/openchuck-new/src/lib.rs", 5, 5),
        ]));
        for enforcing in [true, false] {
            let (_, findings) = evaluate(&config(), &data, enforcing);
            assert!(
                findings.iter().any(|f| f.contains("openchuck-new")),
                "{findings:?}"
            );
            assert!(
                findings
                    .iter()
                    .any(|f| f.contains("no line data") && f.contains("openchuck-types")),
                "{findings:?}"
            );
        }
    }

    #[test]
    fn configuration_mistakes_are_findings() {
        let mut bad = config();
        bad.excluded.push("openchuck-path".to_owned());
        bad.floors.insert("openchuck-ui".to_owned(), 50.0);
        let data = parse_lcov(&lcov(&[(PATH_A, 10, 10), (TYPES, 10, 10)]));
        let (_, findings) = evaluate(&bad, &data, false);
        assert_eq!(findings.len(), 2, "{findings:?}");
    }

    #[test]
    fn the_table_names_each_crate_with_its_percentage() {
        let data = parse_lcov(&lcov(&[(PATH_A, 200, 190), (TYPES, 10, 10)]));
        let (rows, _) = evaluate(&config(), &data, true);
        let text = render(&config(), &rows, true);
        assert!(
            text.contains("| openchuck-path | 200 | 190 | 95.00% | 95.0% | ok |"),
            "{text}"
        );
        assert!(
            text.contains("| openchuck-ui | 0 | 0 | - | - | report only |"),
            "{text}"
        );
        assert!(text.contains("enforcement is on"));
    }
}
