// SPDX-License-Identifier: AGPL-3.0-or-later
//! Message rules (decision record 0536 item 6.3; NFR-12), kept as a ratchet
//! against `xtask/messages.toml`:
//!
//! - **One constructor.** Every message is to be made with
//!   `Message::about`, which writes the form `<Topic> (<context>): <text>`.
//!   The calls of `Message::new` that build a message outside it are
//!   counted per file; a file may not have more than its baseline, and a
//!   file that has fewer must lower its baseline, so the count only falls.
//!   When it reaches nothing, `Message::new` becomes private (decision
//!   record 0536 section 7, steps 3 to 5).
//! - **No claim of safety.** No user-visible text may call a path or a
//!   program safe, verified, certified or ready to run (NFR-12;
//!   interface specification section 15). The string literals of the
//!   product code are scanned for those words and counted per file in the
//!   same way: the texts that use them today are listed (OI-2097) and
//!   reworded in step 5, and no new one is let in.
//!
//! Test code is not scanned: files under `tests/`, files named `tests.rs`
//! or ending in `_tests.rs`, and everything after a file's `#[cfg(test)]`
//! module.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::Findings;

/// The baseline file, from the workspace root.
pub const BASELINE: &str = "xtask/messages.toml";

/// The folders whose crates are scanned, from the workspace root.
const SCANNED: &[&str] = &["crates", "apps"];

/// What a message built outside the constructor looks like.
const MESSAGE_NEW: &str = "Message::new(";

/// Words a user-visible text may not use, as a claim about a path or a
/// program (NFR-12; specification section 15). Matched as whole words,
/// ignoring case.
pub const CLAIMS: &[&str] = &[
    "verified",
    "certified",
    "certify",
    "certificate",
    "certification",
    "ready to run",
    "machine-ready",
    "safe to run",
    "good to go",
];

/// The part of a source file that is not test code: everything before its
/// first `#[cfg(test)]` that opens a module.
pub fn product_code(text: &str) -> &str {
    let mut offset = 0;
    let mut lines = text.split_inclusive('\n').peekable();
    while let Some(line) = lines.next() {
        if line.trim() == "#[cfg(test)]"
            && lines
                .peek()
                .is_some_and(|next| next.trim_start().starts_with("mod "))
        {
            return &text[..offset];
        }
        offset += line.len();
    }
    text
}

/// How many messages `code` builds with `Message::new`.
pub fn message_new_count(code: &str) -> usize {
    code.lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .map(|line| line.matches(MESSAGE_NEW).count())
        .sum()
}

/// The string literals of `code`, outside comments, with their escapes as
/// written.
pub fn string_literals(code: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = code.chars().peekable();
    let mut current: Option<String> = None;
    while let Some(c) = chars.next() {
        match &mut current {
            Some(text) => match c {
                '\\' => {
                    text.push(c);
                    if let Some(next) = chars.next() {
                        text.push(next);
                    }
                }
                '"' => out.push(current.take().unwrap_or_default()),
                _ => text.push(c),
            },
            None => match c {
                '/' if chars.peek() == Some(&'/') => {
                    // A comment runs to the end of the line.
                    for skipped in chars.by_ref() {
                        if skipped == '\n' {
                            break;
                        }
                    }
                }
                '\'' => {
                    // A character literal, such as '"', is not a string; a
                    // lifetime ('a) is left as it is.
                    let rest: String = chars.clone().take(3).collect();
                    if rest.starts_with("\"'") {
                        chars.next();
                        chars.next();
                    } else if rest.starts_with("\\\"'") {
                        chars.next();
                        chars.next();
                        chars.next();
                    }
                }
                '"' => current = Some(String::new()),
                _ => {}
            },
        }
    }
    out
}

/// How many times the string literals of `code` use a word of [`CLAIMS`].
pub fn claim_count(code: &str) -> usize {
    string_literals(code)
        .iter()
        .map(|literal| {
            let lower = literal.to_lowercase();
            CLAIMS
                .iter()
                .map(|claim| whole_word_matches(&lower, claim))
                .sum::<usize>()
        })
        .sum()
}

/// Words that, just before a claim word, make it no claim: "must be
/// verified by the user", "has not been verified", "not certified".
const NOT_A_CLAIM_AFTER: &[&str] = &["be ", "been ", "not "];

/// How many times `word` stands in `text` as a whole word, as a claim.
fn whole_word_matches(text: &str, word: &str) -> usize {
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    text.match_indices(word)
        .filter(|(at, _)| {
            let head = &text[..*at];
            let before = head.chars().next_back();
            let after = text[at + word.len()..].chars().next();
            !before.is_some_and(is_word)
                && !after.is_some_and(is_word)
                && !NOT_A_CLAIM_AFTER.iter().any(|w| head.ends_with(w))
        })
        .count()
}

/// Whether `path` holds test code only.
fn is_test_file(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    name == "tests.rs"
        || name.ends_with("_tests.rs")
        || path.components().any(|c| c.as_os_str() == "tests")
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir).with_context(|| format!("cannot read {}", dir.display()))? {
        let path = entry?.path();
        if path.is_dir() {
            collect(&path, out)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") && !is_test_file(&path) {
            out.push(path);
        }
    }
    Ok(())
}

/// The counts of every product source file under the scanned folders, by
/// path from `root` with forward slashes: messages built with
/// `Message::new`, and claim words.
pub fn counts(root: &Path) -> Result<(BTreeMap<String, usize>, BTreeMap<String, usize>)> {
    let mut files = Vec::new();
    for folder in SCANNED {
        for krate in std::fs::read_dir(root.join(folder))? {
            let src = krate?.path().join("src");
            if src.is_dir() {
                collect(&src, &mut files)?;
            }
        }
    }
    files.sort();
    let mut built = BTreeMap::new();
    let mut claims = BTreeMap::new();
    for path in files {
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("cannot read {}", path.display()))?;
        let code = product_code(&text);
        let name = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        let n = message_new_count(code);
        if n > 0 {
            built.insert(name.clone(), n);
        }
        let n = claim_count(code);
        if n > 0 {
            claims.insert(name, n);
        }
    }
    Ok((built, claims))
}

/// The findings of `found` against `allowed`: a file over its baseline, and
/// a file under it whose baseline must come down.
pub fn compare(
    what: &str,
    rule: &str,
    found: &BTreeMap<String, usize>,
    allowed: &BTreeMap<String, usize>,
) -> Findings {
    let mut findings = Vec::new();
    for (file, &n) in found {
        let limit = allowed.get(file).copied().unwrap_or(0);
        if n > limit {
            findings.push(format!(
                "{file}: {n} {what}, {limit} allowed by {BASELINE}: {rule}"
            ));
        }
    }
    for (file, &limit) in allowed {
        let n = found.get(file).copied().unwrap_or(0);
        if n < limit {
            findings.push(format!(
                "{file}: {n} {what}, fewer than the {limit} in {BASELINE}: lower its baseline \
                 to {n} (the count only falls)"
            ));
        }
    }
    findings
}

fn table(baseline: &toml::Table, key: &str) -> Result<BTreeMap<String, usize>> {
    let mut out = BTreeMap::new();
    if let Some(t) = baseline.get(key).and_then(toml::Value::as_table) {
        for (file, value) in t {
            let n = value
                .as_integer()
                .and_then(|n| usize::try_from(n).ok())
                .with_context(|| format!("{BASELINE}: [{key}] {file} is not a count"))?;
            out.insert(file.clone(), n);
        }
    }
    Ok(out)
}

/// Runs both message rules on the workspace at `root`.
pub fn check(root: &Path) -> Result<Findings> {
    let text = std::fs::read_to_string(root.join(BASELINE))
        .with_context(|| format!("cannot read {BASELINE}"))?;
    let baseline: toml::Table = toml::from_str(&text).context(BASELINE)?;
    let (built, claims) = counts(root)?;
    let mut findings = compare(
        "messages built with Message::new",
        "build a message with Message::about, which writes the form `<Topic> (<context>): <text>` (decision 0536)",
        &built,
        &table(&baseline, "message_new")?,
    );
    findings.extend(compare(
        "uses of a word that claims safety",
        "no text may call a path or a program safe, verified, certified or ready to run (NFR-12; specification section 15)",
        &claims,
        &table(&baseline, "claims")?,
    ));
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn req_ui_12_message_new_is_counted_outside_comments_and_tests() {
        let code = "let a = Message::new(s, t, None, None);\n\
                    // Message::new( in a comment\n\
                    let b = Message::about(s, \"T\", &c, \"x\", None, None);\n\
                    #[cfg(test)]\n\
                    mod tests {\n    fn f() { Message::new(s, t, None, None); }\n}\n";
        assert_eq!(message_new_count(product_code(code)), 1);
    }

    #[test]
    fn req_nfr_12_claim_words_are_found_in_strings_only() {
        let code = "// verified in a comment is fine\n\
                    let a = \"Facing coverage complete (verified path geometry)\";\n\
                    let b = \"a certificate\"; let c = '\"'; let d = \"unverified is another word\";\n\
                    fn verified() {}\n";
        assert_eq!(claim_count(code), 2);
        assert_eq!(claim_count("let x = \"Ready to run\";"), 1);
        assert_eq!(
            claim_count("let x = \"Verify this G-code before you run it.\";"),
            0
        );
        // Saying the user must verify is no claim.
        assert_eq!(
            claim_count("let x = \"Generated G-code must be verified by the user.\";"),
            0
        );
        assert_eq!(claim_count("let x = \"It has not been verified.\";"), 0);
    }

    #[test]
    fn req_nfr_12_an_escaped_quote_does_not_end_a_string() {
        let code = r#"let a = "say \"verified\" here"; let b = "fine";"#;
        assert_eq!(string_literals(code).len(), 2);
        assert_eq!(claim_count(code), 1);
    }

    #[test]
    fn req_ui_12_the_ratchet_refuses_more_and_asks_for_less() {
        let allowed = BTreeMap::from([("a.rs".to_owned(), 2), ("b.rs".to_owned(), 1)]);
        let found = BTreeMap::from([("a.rs".to_owned(), 3), ("c.rs".to_owned(), 1)]);
        let findings = compare("things", "rule", &found, &allowed);
        assert_eq!(findings.len(), 3, "{findings:?}");
        assert!(findings[0].starts_with("a.rs: 3 things, 2 allowed"));
        assert!(findings[1].starts_with("c.rs: 1 things, 0 allowed"));
        assert!(findings[2].starts_with("b.rs: 0 things, fewer than the 1"));
        assert!(compare("things", "rule", &allowed, &allowed).is_empty());
    }

    #[test]
    fn req_ui_12_test_files_are_not_scanned() {
        assert!(is_test_file(Path::new(
            "crates/x/src/shell/simulate_tests.rs"
        )));
        assert!(is_test_file(Path::new(
            "crates/x/src/entry_safety/tests.rs"
        )));
        assert!(is_test_file(Path::new("crates/x/tests/a.rs")));
        assert!(!is_test_file(Path::new("crates/x/src/overview.rs")));
    }
}
