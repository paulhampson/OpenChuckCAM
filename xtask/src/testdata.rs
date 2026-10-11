// SPDX-License-Identifier: AGPL-3.0-or-later
//! The test-data size policy (work breakdown slice 0 item 10, decision record 0004):
//! no file in `testdata/` above 1 MiB and no more than 20 MiB in total.
//! Larger models are generated procedurally or, if ever needed, go to Git LFS
//! or a separate repository (architecture section 13, open).

use std::path::Path;

use anyhow::{Context, Result};

use crate::Findings;

/// Largest single file.
pub const MAX_FILE_BYTES: u64 = 1024 * 1024;
/// Largest total.
pub const MAX_TOTAL_BYTES: u64 = 20 * 1024 * 1024;

/// Applies the policy to a list of (path, size) pairs.
pub fn check_sizes(files: &[(String, u64)]) -> Findings {
    let mut findings = Findings::new();
    let mut total = 0u64;
    for (path, size) in files {
        total = total.saturating_add(*size);
        if *size > MAX_FILE_BYTES {
            findings.push(format!(
                "{path} is {size} bytes; the limit per file is {MAX_FILE_BYTES} (generate it procedurally instead)"
            ));
        }
    }
    if total > MAX_TOTAL_BYTES {
        findings.push(format!(
            "testdata/ holds {total} bytes; the limit is {MAX_TOTAL_BYTES}"
        ));
    }
    findings
}

fn walk(dir: &Path, root: &Path, out: &mut Vec<(String, u64)>) -> Result<()> {
    for entry in std::fs::read_dir(dir).with_context(|| format!("cannot read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            walk(&path, root, out)?;
        } else {
            let shown = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .display()
                .to_string();
            out.push((shown, entry.metadata()?.len()));
        }
    }
    Ok(())
}

/// Checks `testdata/` under `root`.
pub fn check(root: &Path) -> Result<Findings> {
    let mut files = Vec::new();
    walk(&root.join("testdata"), root, &mut files)?;
    files.sort();
    Ok(check_sizes(&files))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn req_nfr_08_oversized_test_data_is_found() {
        assert!(check_sizes(&[("testdata/a.stl".to_owned(), 10)]).is_empty());
        assert_eq!(
            check_sizes(&[("testdata/big.stl".to_owned(), MAX_FILE_BYTES + 1)]).len(),
            1
        );
        let many: Vec<_> = (0..21)
            .map(|i| (format!("testdata/{i}"), MAX_FILE_BYTES))
            .collect();
        assert_eq!(check_sizes(&many).len(), 1);
    }
}
