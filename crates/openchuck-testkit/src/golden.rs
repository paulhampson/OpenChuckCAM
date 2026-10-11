// SPDX-License-Identifier: AGPL-3.0-or-later
//! Plain-text golden files (NFR-08).
//!
//! Golden G-code is kept as plain text so a review shows a real diff
//! (architecture section 12). Set `OPENCHUCK_BLESS=1` to write the current
//! output as the new golden file; review the diff before committing it.

use std::path::Path;

/// The environment variable that turns comparison into an update.
pub const BLESS_VAR: &str = "OPENCHUCK_BLESS";

/// Compares `actual` with the golden file at `path`.
///
/// Returns a message naming the first differing line, or saying the file is
/// missing. With `OPENCHUCK_BLESS=1` it writes `actual` to `path` instead.
pub fn check(path: &Path, actual: &str) -> Result<(), String> {
    if std::env::var(BLESS_VAR).is_ok_and(|v| v == "1") {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
        }
        return std::fs::write(path, actual)
            .map_err(|e| format!("cannot write {}: {e}", path.display()));
    }
    let expected = std::fs::read_to_string(path).map_err(|e| {
        format!(
            "cannot read golden file {}: {e} (run with {BLESS_VAR}=1 to create it)",
            path.display()
        )
    })?;
    compare(&expected, actual).map_err(|m| format!("{}: {m}", path.display()))
}

/// Compares two texts line by line and describes the first difference.
pub fn compare(expected: &str, actual: &str) -> Result<(), String> {
    if expected == actual {
        return Ok(());
    }
    let mut expected_lines = expected.lines();
    let mut actual_lines = actual.lines();
    let mut line = 1;
    loop {
        match (expected_lines.next(), actual_lines.next()) {
            (Some(e), Some(a)) if e == a => line += 1,
            (Some(e), Some(a)) => {
                return Err(format!(
                    "line {line} differs\n  expected: {e}\n  actual:   {a}"
                ));
            }
            (Some(e), None) => return Err(format!("output ends before line {line}: {e}")),
            (None, Some(a)) => return Err(format!("extra output at line {line}: {a}")),
            (None, None) => return Err("line endings or a trailing newline differ".to_owned()),
        }
    }
}
