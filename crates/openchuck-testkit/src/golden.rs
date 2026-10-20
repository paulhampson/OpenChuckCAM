// SPDX-License-Identifier: AGPL-3.0-or-later
//! Plain-text golden files (NFR-08).
//!
//! Golden G-code is kept as plain text so a review shows a real diff
//! (architecture section 12). Set `OPENCHUCK_BLESS=1` to write the current
//! output as the new golden file; review the diff before committing it.

use std::path::Path;

/// The environment variable that turns comparison into an update.
pub const BLESS_VAR: &str = "OPENCHUCK_BLESS";

/// The placeholder a golden file holds in place of the software version (NFR-16).
pub const VERSION_PLACEHOLDER: &str = "<version>";

/// Replaces the version in the generator comment, `(OpenChuckCAM 0.10.0)`, with
/// [`VERSION_PLACEHOLDER`], so a version bump changes no golden file. The same input and post
/// still give byte-identical G-code for one software version (GC-07, NFR-06); this only keeps
/// the stored files stable across versions. Any other text is left as it is.
#[must_use]
pub fn mask_generator(text: &str) -> String {
    const PREFIX: &str = "(OpenChuckCAM ";
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        let body = line.trim_end_matches(['\r', '\n']);
        let ending = &line[body.len()..];
        // A post with line numbers writes `N20 ` first.
        let numbered = body
            .strip_prefix('N')
            .and_then(|rest| rest.split_once(' '))
            .filter(|(digits, _)| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()));
        let (lead, comment) = match numbered {
            Some((digits, rest)) => (format!("N{digits} "), rest),
            None => (String::new(), body),
        };
        let version = comment
            .strip_prefix(PREFIX)
            .and_then(|rest| rest.strip_suffix(')'))
            .filter(|v| {
                !v.is_empty()
                    && v.bytes()
                        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'+'))
            });
        if version.is_some() {
            out.push_str(&format!("{lead}{PREFIX}{VERSION_PLACEHOLDER}){ending}"));
        } else {
            out.push_str(line);
        }
    }
    out
}

/// Whether a bless run is asked for: the variable's value is exactly `1`.
#[must_use]
pub fn is_bless_value(value: Option<&str>) -> bool {
    value == Some("1")
}

/// Whether this process is a bless run (`OPENCHUCK_BLESS=1`; any other value, set or not, is
/// not).
#[must_use]
pub fn blessing() -> bool {
    is_bless_value(std::env::var(BLESS_VAR).ok().as_deref())
}

/// Compares `actual` with the golden file at `path`, after masking the generator version in
/// `actual` ([`mask_generator`]).
///
/// Returns a message naming the first differing line, or saying the file is
/// missing. With `OPENCHUCK_BLESS=1` it writes `actual` (masked) to `path` instead.
pub fn check(path: &Path, actual: &str) -> Result<(), String> {
    let masked = mask_generator(actual);
    let actual = masked.as_str();
    if blessing() {
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
