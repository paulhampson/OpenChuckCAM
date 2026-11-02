// SPDX-License-Identifier: AGPL-3.0-or-later
//! Licence checks (NFR-10, architecture section 12): every source file
//! carries the project's SPDX identifier near its top, and every crate's
//! `license` field names the project licence.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::Findings;

/// The project licence, as an SPDX expression.
pub const PROJECT_LICENCE: &str = "AGPL-3.0-or-later";

/// File extensions that must carry an SPDX header.
const CHECKED_EXTENSIONS: &[&str] = &["rs", "toml", "yml", "yaml"];

/// Directories never scanned.
const SKIPPED_DIRS: &[&str] = &[
    ".git",
    "target",
    "docs",
    "LICENSES",
    "testdata",
    "corpus",
    "artifacts",
];

/// Lines from the top of a file within which the header must appear.
const HEADER_LINES: usize = 5;

/// True when `text` carries the project's SPDX identifier in its first lines.
pub fn has_header(text: &str) -> bool {
    let wanted = format!("SPDX-License-Identifier: {PROJECT_LICENCE}");
    text.lines()
        .take(HEADER_LINES)
        .any(|line| line.contains(&wanted))
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir).with_context(|| format!("cannot read {}", dir.display()))? {
        let path = entry?.path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        if path.is_dir() {
            if !SKIPPED_DIRS.contains(&name) {
                collect(&path, out)?;
            }
        } else if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| CHECKED_EXTENSIONS.contains(&e))
            && name != "Cargo.lock"
        {
            out.push(path);
        }
    }
    Ok(())
}

/// Checks headers under `root` and the licence field of every workspace crate.
pub fn check(root: &Path) -> Result<Findings> {
    let mut files = Vec::new();
    collect(root, &mut files)?;
    files.sort();
    let mut findings = Findings::new();
    for file in files {
        let text = std::fs::read_to_string(&file)
            .with_context(|| format!("cannot read {}", file.display()))?;
        if !has_header(&text) {
            let shown = file.strip_prefix(root).unwrap_or(&file);
            findings.push(format!(
                "{} has no `SPDX-License-Identifier: {PROJECT_LICENCE}` in its first {HEADER_LINES} lines",
                shown.display()
            ));
        }
    }
    let metadata = cargo_metadata::MetadataCommand::new()
        .manifest_path(root.join("Cargo.toml"))
        .no_deps()
        .exec()
        .context("cannot run cargo metadata")?;
    for package in metadata.workspace_packages() {
        if package.license.as_deref() != Some(PROJECT_LICENCE) {
            findings.push(format!(
                "{} has licence {:?}; every crate is {PROJECT_LICENCE} (NFR-10)",
                package.name, package.license
            ));
        }
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn req_nfr_10_missing_spdx_header_is_found() {
        assert!(has_header(
            "// SPDX-License-Identifier: AGPL-3.0-or-later\nfn main() {}\n"
        ));
        assert!(has_header(
            "#!/bin/sh\n# SPDX-License-Identifier: AGPL-3.0-or-later\n"
        ));
        assert!(!has_header("fn main() {}\n"));
        assert!(!has_header("// SPDX-License-Identifier: MIT\n"));
        assert!(!has_header(
            "\n\n\n\n\n// SPDX-License-Identifier: AGPL-3.0-or-later\n"
        ));
    }
}
