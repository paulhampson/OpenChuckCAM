// SPDX-License-Identifier: AGPL-3.0-or-later
//! Unresolved merge conflict markers in tracked files.
//!
//! CI hygiene supporting NFR-06 and the common review checks; no requirement covers it directly.
//! A line that starts with exactly seven `<` or `>` followed by a space or the end of the line, or
//! that is exactly seven `=`, is a conflict marker left by Git. Documents are outside CodeRabbit,
//! so this check is what catches them there.
//!
//! False positives are handled as follows:
//!
//! - `testdata/` holds golden and fixture files that are not ours to edit, so it is skipped.
//! - Files that are not UTF-8 text (binary files) are skipped.
//! - In Markdown, fenced code blocks are skipped (a document may quote the markers), and a line
//!   of seven `=` directly under a line of text is a setext heading underline, not a marker; the
//!   other two markers of the conflict are still found.
//! - A marker needs exactly seven characters, so longer rules (`========`) are not markers.

use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::Findings;

/// Folders whose files are fixtures and are not scanned.
const SKIPPED_PREFIXES: &[&str] = &["testdata/"];

fn is_markdown(path: &str) -> bool {
    Path::new(path)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("md"))
}

/// Whether `line` is a start (`<`) or end (`>`) marker with the given character.
fn is_arrow_marker(line: &str, c: char) -> bool {
    let rest = match line.strip_prefix(&c.to_string().repeat(7)) {
        Some(rest) => rest,
        None => return false,
    };
    rest.is_empty() || rest.starts_with(' ') || rest.starts_with('\t')
}

/// Scans one file's text; each finding names `path`, the line number and the marker.
pub fn scan_text(path: &str, text: &str) -> Findings {
    let markdown = is_markdown(path);
    let mut findings = Findings::new();
    let mut fence: Option<(char, usize)> = None;
    let mut previous_blank = true;
    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim_end_matches('\r');
        let number = index + 1;
        if markdown {
            // Four or more leading spaces make an indented code block, not a fence (CommonMark).
            let indent = line.len() - line.trim_start_matches(' ').len();
            let trimmed = if indent <= 3 { line.trim_start() } else { "" };
            let run = ['`', '~'].into_iter().find_map(|c| {
                let length = trimmed.chars().take_while(|&x| x == c).count();
                (length >= 3).then_some((c, length))
            });
            match (fence, run) {
                (None, Some(opening)) => {
                    fence = Some(opening);
                    previous_blank = false;
                    continue;
                }
                // A closing fence has the same character, at least as long a run, and nothing
                // but whitespace after it (CommonMark).
                (Some((open, length)), Some((c, run_length)))
                    if open == c
                        && run_length >= length
                        && trimmed[run_length..].trim().is_empty() =>
                {
                    fence = None;
                    previous_blank = false;
                    continue;
                }
                _ => {}
            }
            if fence.is_some() {
                continue;
            }
        }
        let marker = if is_arrow_marker(line, '<') {
            Some("start marker")
        } else if is_arrow_marker(line, '>') {
            Some("end marker")
        } else if line == "=======" && (!markdown || previous_blank) {
            Some("separator")
        } else {
            None
        };
        if let Some(kind) = marker {
            findings.push(format!(
                "{path}:{number}: unresolved merge conflict {kind}: {line}"
            ));
        }
        previous_blank = line.trim().is_empty();
    }
    findings
}

/// Scans the listed files under `root` (paths relative to it), in order.
pub fn scan_files(root: &Path, files: &[String]) -> Result<Findings> {
    let mut findings = Findings::new();
    for file in files {
        if SKIPPED_PREFIXES.iter().any(|p| file.starts_with(p)) {
            continue;
        }
        let path = root.join(file);
        if !path.is_file() {
            continue;
        }
        let bytes = std::fs::read(&path).with_context(|| format!("cannot read {file}"))?;
        let Ok(text) = String::from_utf8(bytes) else {
            continue;
        };
        if text.contains('\0') {
            continue;
        }
        findings.extend(scan_text(file, &text));
    }
    Ok(findings)
}

/// The tracked files of the repository at `root`, sorted.
fn tracked_files(root: &Path) -> Result<Vec<String>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["ls-files", "-z"])
        .output()
        .context("cannot run git ls-files")?;
    if !output.status.success() {
        bail!(
            "git ls-files failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let mut files: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .split('\0')
        .filter(|f| !f.is_empty())
        .map(str::to_owned)
        .collect();
    files.sort();
    Ok(files)
}

/// Checks every tracked file under `root`.
pub fn check(root: &Path) -> Result<Findings> {
    scan_files(root, &tracked_files(root)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    // The markers are built here so that no line of this file starts with one.
    fn start() -> String {
        format!("{} HEAD", "<".repeat(7))
    }
    fn middle() -> String {
        "=".repeat(7)
    }
    fn end() -> String {
        format!("{} origin/main", ">".repeat(7))
    }

    fn conflict() -> String {
        format!("a\n{}\nb\n{}\nc\n{}\nd\n", start(), middle(), end())
    }

    #[test]
    fn req_nfr_06_conflict_markers_are_found() {
        let findings = scan_text("src/a.rs", &conflict());
        assert_eq!(findings.len(), 3, "{findings:?}");
        assert!(findings[0].starts_with("src/a.rs:2:"));
        assert!(findings[1].contains("separator"));
        assert!(findings[2].contains("end marker"));
        // In Markdown the separator under a text line reads as an underline; the other two
        // markers of the conflict are still found.
        assert_eq!(scan_text("docs/a.md", &conflict()).len(), 2);
    }

    #[test]
    fn req_nfr_06_bare_and_crlf_markers_are_found() {
        let text = format!("{}\r\n{}\r\n{}\r\n", "<".repeat(7), middle(), ">".repeat(7));
        assert_eq!(scan_text("a.toml", &text).len(), 3);
    }

    #[test]
    fn req_nfr_06_other_lines_are_not_markers() {
        let text = format!(
            "{}\n{}\n{}\n  {}\n{}x\n// {}\n",
            "<".repeat(6),
            "<".repeat(8),
            "=".repeat(8),
            start(),
            ">".repeat(7),
            end()
        );
        assert!(scan_text("a.rs", &text).is_empty());
    }

    #[test]
    fn req_nfr_06_setext_underline_is_not_a_marker() {
        let text = format!("Title\n{}\n\nbody\n", middle());
        assert!(scan_text("docs/a.md", &text).is_empty());
        // The same line in a non-Markdown file is a separator.
        assert_eq!(scan_text("a.txt", &text).len(), 1);
        // A separator after a blank line is not an underline.
        let loose = format!("a\n\n{}\n\nb\n", middle());
        assert_eq!(scan_text("docs/a.md", &loose).len(), 1);
    }

    #[test]
    fn req_nfr_06_fenced_code_in_markdown_may_quote_markers() {
        let text = format!("```text\n{}\n```\n~~~\n{}\n~~~\n", start(), end());
        assert!(scan_text("docs/a.md", &text).is_empty());
        // After the fence closes, markers count again.
        let after = format!("```\nx\n```\n{}\n", start());
        assert_eq!(scan_text("docs/a.md", &after).len(), 1);
        // Fences mean nothing outside Markdown.
        assert_eq!(scan_text("a.rs", &format!("```\n{}\n", start())).len(), 1);
    }

    #[test]
    fn req_nfr_06_a_fence_closes_only_on_a_long_enough_bare_run() {
        // A shorter run, an info string and another character do not close a four-tick fence.
        let text = format!("````\n```\n````text\n~~~~\n{}\n````\n{}\n", start(), end());
        let findings = scan_text("docs/a.md", &text);
        assert_eq!(findings.len(), 1, "{findings:?}");
        assert!(findings[0].contains("end marker"));
        // A longer closing run with trailing spaces closes it.
        let long = format!("```\nx\n`````  \n{}\n", start());
        assert_eq!(scan_text("docs/a.md", &long).len(), 1);
    }

    #[test]
    fn req_nfr_06_an_indented_code_line_does_not_open_a_fence() {
        let text = format!("    ```\n{}\n", start());
        assert_eq!(scan_text("docs/a.md", &text).len(), 1);
        let allowed = format!("   ```\n{}\n```\n", start());
        assert!(scan_text("docs/a.md", &allowed).is_empty());
    }

    #[test]
    fn req_nfr_06_scan_skips_fixtures_and_binary_files() {
        let dir = std::env::temp_dir().join(format!("xtask-conflicts-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("testdata")).unwrap();
        std::fs::create_dir_all(dir.join("docs")).unwrap();
        std::fs::write(dir.join("testdata/golden.nc"), conflict()).unwrap();
        std::fs::write(dir.join("docs/bad.md"), conflict()).unwrap();
        std::fs::write(dir.join("docs/ok.md"), "fine\n").unwrap();
        let mut binary = conflict().into_bytes();
        binary.push(0xff);
        std::fs::write(dir.join("docs/image.bin"), binary).unwrap();
        let mut nul = conflict().into_bytes();
        nul.push(0);
        std::fs::write(dir.join("docs/nul.dat"), nul).unwrap();
        let files: Vec<String> = [
            "testdata/golden.nc",
            "docs/bad.md",
            "docs/ok.md",
            "docs/image.bin",
            "docs/nul.dat",
            "docs/missing.md",
        ]
        .map(str::to_owned)
        .to_vec();
        let findings = scan_files(&dir, &files).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(findings.len(), 2, "{findings:?}");
        assert!(findings.iter().all(|f| f.starts_with("docs/bad.md:")));
    }

    #[test]
    fn req_nfr_06_the_repository_has_no_unresolved_markers() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        if root.join(".git").exists() {
            assert_eq!(check(root).unwrap(), Findings::new());
        }
    }
}
