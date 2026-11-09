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
//! - **No internal references.** No user-visible text may name a
//!   requirement ("(OP-14)", "SET-02"), a decision record ("decision
//!   0495") or an open item ("OI-360"): a user reads what to change, not
//!   where the rule came from (decision record 0536 step 4; decision record
//!   0644). The string literals are scanned and counted per file in the same
//!   way, against the `[ids]` baseline.
//!
//! - **A count agrees with its noun.** A count that can be one is written
//!   with `openchuck_path::counted`, which gives "1 layer" and "3 layers";
//!   a text that prints `{layers} layers` reads "1 layers" when there is
//!   one (decision record 0644, after OWNERCHECKS2). The string literals
//!   are scanned for a format placeholder followed by a plural noun of
//!   [`PLURAL_NOUNS`] (`{} rings`, `{n} moves`) and counted per file in
//!   the same way, against the `[plural_counts]` baseline: what is left is
//!   a count that is never one ("... is too many", a mesh's own size).
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

/// The prefixes of the requirement IDs (docs/requirements.md) and of the
/// open items.
pub const ID_PREFIXES: &[&str] = &[
    "COL", "GC", "IMP", "INT", "NFR", "OP", "PRJ", "SET", "SIM", "SUG", "TL", "UI", "VC", "OI",
];

/// The plural nouns a count is checked before: a placeholder followed by
/// one of them is a count that reads wrongly when it is one.
pub const PLURAL_NOUNS: &[&str] = &[
    "layers",
    "rings",
    "moves",
    "turns",
    "passes",
    "points",
    "faces",
    "edges",
    "holes",
    "tabs",
    "chains",
    "sets",
    "positions",
    "lines",
    "legs",
    "operations",
    "reversals",
    "entries",
    "segments",
    "links",
    "plunges",
    "steps",
    "bands",
    "slices",
    "areas",
    "nubs",
    "tools",
    "files",
    "models",
    "setups",
    "creases",
];

/// How many counts before a plural noun the string literals of `code` hold
/// (decision record 0644).
pub fn plural_count(code: &str) -> usize {
    string_literals(code).iter().map(|l| plurals_in(l)).sum()
}

/// The counts in one text written as it is in the source: a format
/// placeholder (`{}`, `{n}`, `{:.0}`), one space and a noun of
/// [`PLURAL_NOUNS`] as a whole word. A line continuation (a backslash, the
/// line break and the indent after it) is read as nothing, as the compiler
/// reads it.
pub fn plurals_in(text: &str) -> usize {
    let mut joined = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' && chars.peek() == Some(&'\n') {
            while chars.peek().is_some_and(|c| c.is_whitespace()) {
                chars.next();
            }
        } else {
            joined.push(c);
        }
    }
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let mut n = 0;
    for (at, _) in joined.match_indices("} ") {
        let head = &joined[..at];
        let Some(open) = head.rfind('{') else {
            continue;
        };
        // An escaped brace (`{{`) is no placeholder.
        if head[..open].ends_with('{') || head[open..].contains('}') {
            continue;
        }
        let rest = &joined[at + 2..];
        if PLURAL_NOUNS
            .iter()
            .any(|noun| rest.starts_with(noun) && !rest[noun.len()..].starts_with(is_word))
        {
            n += 1;
        }
    }
    n
}

/// How many references to a requirement, an open item or a decision record
/// the string literals of `code` hold (decision record 0644).
pub fn id_count(code: &str) -> usize {
    string_literals(code).iter().map(|l| ids_in(l)).sum()
}

/// The references in one text: "<PREFIX>-<digits>" as a whole word, and
/// "decision <4 digits>" or "decision record <4 digits>", ignoring case.
pub fn ids_in(text: &str) -> usize {
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let digits = |rest: &str| rest.chars().take_while(char::is_ascii_digit).count();
    let mut n = 0;
    for prefix in ID_PREFIXES {
        let pattern = format!("{prefix}-");
        for (at, _) in text.match_indices(&pattern) {
            let before = text[..at].chars().next_back();
            let rest = &text[at + pattern.len()..];
            let d = digits(rest);
            if !before.is_some_and(is_word) && d > 0 && !rest[d..].starts_with(is_word) {
                n += 1;
            }
        }
    }
    let lower = text.to_lowercase();
    for (at, word) in lower.match_indices("decision ") {
        let rest = &lower[at + word.len()..];
        let rest = rest.strip_prefix("record ").unwrap_or(rest);
        if digits(rest) == 4 {
            n += 1;
        }
    }
    n
}

/// The part of a source file that is not test code: everything before its
/// first `#[cfg(test)]` that opens a module.
pub fn product_code(text: &str) -> &str {
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let mut offset = 0;
    for (i, line) in lines.iter().enumerate() {
        // A test module written in the file, not one declared from another
        // file (`mod x_tests;`), which leaves the rest of the file product
        // code. Other attributes may stand between the two, over one line
        // or several (`#[allow(\n    clippy::unwrap_used,\n)]`).
        if line.trim() == "#[cfg(test)]" {
            let mut rest = lines.iter().skip(i + 1).map(|l| {
                // Without a trailing comment.
                l.split("//").next().unwrap_or_default().trim()
            });
            let mut in_attribute = false;
            let declaration = rest.find(|next| {
                if in_attribute {
                    in_attribute = !next.ends_with(")]");
                    return false;
                }
                if next.starts_with("#[") {
                    in_attribute = !next.ends_with(']');
                    return false;
                }
                true
            });
            if declaration.is_some_and(|next| next.starts_with("mod ") && !next.ends_with(';')) {
                return &text[..offset];
            }
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
/// written. A raw string (`r"..."`, `r#"..."#`, `br"..."`) ends only at its
/// own closing quote and hashes, so a backslash in it escapes nothing.
pub fn string_literals(code: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = code.chars().peekable();
    let mut current: Option<String> = None;
    let mut previous: Option<char> = None;
    let mut before_previous: Option<char> = None;
    let is_ident = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
    while let Some(c) = chars.next() {
        let (p1, p2) = (previous, before_previous);
        before_previous = previous;
        previous = Some(c);
        // The start of a raw string: an `r` (or `br`) that is not the end
        // of a name, then hashes and a quote.
        if current.is_none()
            && c == 'r'
            && (!is_ident(p1) || (p1 == Some('b') && !is_ident(p2)))
            && matches!(chars.peek(), Some('#' | '"'))
        {
            let mut hashes = 0;
            while chars.peek() == Some(&'#') {
                chars.next();
                hashes += 1;
            }
            if chars.peek() != Some(&'"') {
                continue;
            }
            chars.next();
            let close: String = std::iter::once('"')
                .chain(std::iter::repeat_n('#', hashes))
                .collect();
            let mut text = String::new();
            for r in chars.by_ref() {
                text.push(r);
                if text.ends_with(&close) {
                    text.truncate(text.len() - close.len());
                    break;
                }
            }
            out.push(text);
            previous = Some('"');
            continue;
        }
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

/// Whether `path` holds test code only. Only its part below `root` is
/// looked at, so a folder named `tests` above the workspace does not count.
pub fn is_test_file(root: &Path, path: &Path) -> bool {
    let below = path.strip_prefix(root).unwrap_or(path);
    let name = below
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    name == "tests.rs"
        || name.ends_with("_tests.rs")
        || below.components().any(|c| c.as_os_str() == "tests")
}

fn collect(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir).with_context(|| format!("cannot read {}", dir.display()))? {
        let path = entry?.path();
        if path.is_dir() {
            collect(root, &path, out)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs")
            && !is_test_file(root, &path)
        {
            out.push(path);
        }
    }
    Ok(())
}

/// The counts of every product source file under the scanned folders, by
/// path from `root` with forward slashes: messages built with
/// `Message::new`, claim words, and references to a requirement, decision
/// record or open item, and counts before a plural noun.
#[allow(clippy::type_complexity)]
pub fn all_counts(
    root: &Path,
) -> Result<(
    BTreeMap<String, usize>,
    BTreeMap<String, usize>,
    BTreeMap<String, usize>,
    BTreeMap<String, usize>,
)> {
    let mut files = Vec::new();
    for folder in SCANNED {
        for krate in std::fs::read_dir(root.join(folder))? {
            let src = krate?.path().join("src");
            if src.is_dir() {
                collect(root, &src, &mut files)?;
            }
        }
    }
    files.sort();
    let mut built = BTreeMap::new();
    let mut claims = BTreeMap::new();
    let mut ids = BTreeMap::new();
    let mut plurals = BTreeMap::new();
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
            claims.insert(name.clone(), n);
        }
        let n = id_count(code);
        if n > 0 {
            ids.insert(name.clone(), n);
        }
        let n = plural_count(code);
        if n > 0 {
            plurals.insert(name, n);
        }
    }
    Ok((built, claims, ids, plurals))
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

pub fn table(baseline: &toml::Table, key: &str) -> Result<BTreeMap<String, usize>> {
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

/// Runs the message rules on the workspace at `root`.
pub fn check(root: &Path) -> Result<Findings> {
    let text = std::fs::read_to_string(root.join(BASELINE))
        .with_context(|| format!("cannot read {BASELINE}"))?;
    let baseline: toml::Table = toml::from_str(&text).context(BASELINE)?;
    let (built, claims, ids, plurals) = all_counts(root)?;
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
    findings.extend(compare(
        "references to a requirement, a decision record or an open item",
        "a user reads what to change, not where the rule came from: no requirement, decision or OI number in a text (decision 0644)",
        &ids,
        &table(&baseline, "ids")?,
    ));
    findings.extend(compare(
        "counts before a plural noun",
        "a count that can be one is written with openchuck_path::counted, so one reads \"1 layer\", not \"1 layers\" (decision 0644)",
        &plurals,
        &table(&baseline, "plural_counts")?,
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

    /// A test module declared from its own file does not end the product
    /// code: the file goes on after it (decision record 0536 step 5).
    #[test]
    fn req_nfr_12_a_declared_test_module_does_not_hide_the_rest_of_the_file() {
        let code = "#[cfg(test)]\nmod shell_tests;\n\
                    fn f() { let a = Message::new(s, \"Verified\", None, None); }\n\
                    #[cfg(test)]\nmod tests {\n    fn g() { Message::new(s, t, None, None); }\n}\n";
        assert_eq!(message_new_count(product_code(code)), 1);
        assert_eq!(claim_count(product_code(code)), 1);
        // A trailing comment on the declaration changes nothing.
        let commented = code.replace("mod shell_tests;", "mod shell_tests; // its own file");
        assert_eq!(message_new_count(product_code(&commented)), 1);
        assert_eq!(claim_count(product_code(&commented)), 1);
    }

    /// Every claim word is gone from the product texts (OI-2097, decision
    /// record 0536 step 5), and the baseline allows none back.
    #[test]
    fn req_nfr_12_no_product_text_claims_safety() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let (_, claims, _, _) = all_counts(&root).unwrap();
        assert!(claims.is_empty(), "{claims:?}");
        let text = std::fs::read_to_string(root.join(BASELINE)).unwrap();
        let baseline: toml::Table = toml::from_str(&text).unwrap();
        assert!(table(&baseline, "claims").unwrap().is_empty());
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
    fn req_ui_12_a_raw_string_ends_at_its_own_quote() {
        // A backslash before the closing quote of a raw string escapes
        // nothing; read as an escape, the rest of the file read as strings.
        let code = "let a = text.strip_prefix(r\"\\\\?\\\");\n// decision 0114 in a comment\n\
                    let b = r#\"say \"hi\"\"#; let c = br\"x\"; let rr = 1;\n";
        assert_eq!(string_literals(code), ["\\\\?\\", "say \"hi\"", "x"]);
        assert_eq!(id_count(code), 0);
    }

    #[test]
    fn req_ui_12_a_test_module_after_other_attributes_is_test_code() {
        let one =
            "fn f() {}\n#[cfg(test)]\n#[allow(clippy::float_cmp)]\nmod tests {\n    \"x\"\n}\n";
        assert_eq!(product_code(one), "fn f() {}\n");
        let several = "fn f() {}\n#[cfg(test)]\n#[allow(\n    clippy::unwrap_used,\n    \
                       clippy::panic\n)]\nmod tests {\n}\n";
        assert_eq!(product_code(several), "fn f() {}\n");
        // A test-only item that is not a module leaves the rest product code.
        let item = "#[cfg(test)]\n#[allow(dead_code)]\nfn helper() {}\nfn g() {}\n";
        assert_eq!(product_code(item), item);
    }

    #[test]
    fn req_ui_12_a_text_naming_a_requirement_or_a_decision_is_counted() {
        // Decision record 0644: OWNERCHECKS2 found "(OP-14)" in a refusal.
        assert_eq!(ids_in("so links could cross it (OP-14); enlarge"), 1);
        assert_eq!(ids_in("held (SET-09), see decision 0495 and OI-360"), 3);
        assert_eq!(ids_in("see Decision record 0644"), 1);
        assert_eq!(ids_in("a STOP-1 tool, T1-2, op-14, 3 decisions"), 0);
        let code = "let s = format!(\"a (OP-14) b\");\n// OP-15 in a comment\n";
        assert_eq!(id_count(code), 1);
    }

    #[test]
    fn req_ui_12_a_count_before_a_plural_noun_is_counted() {
        // Decision record 0644: OWNERCHECKS2 found "in 1 layers of 1 mm".
        assert_eq!(
            plurals_in("beside it in {layers} layers of {stepdown} mm"),
            1
        );
        assert_eq!(plurals_in("cut {} rings, {:.0} moves and {n} passes"), 3);
        // Over a line continuation, as the compiler reads it.
        assert_eq!(plurals_in("cut {} \\\n                 rings"), 1);
        // No placeholder, an escaped brace, another word or a longer word.
        assert_eq!(
            plurals_in("3 layers, {{x}} layers, {} layered, {} layer"),
            0
        );
        assert_eq!(plurals_in("{} concave creases, {} linesmen"), 0);
        let code = "let s = format!(\"{} moves\");\n// {} moves in a comment\n\
                    let t = counted(n, \"move\", \"moves\");\n";
        assert_eq!(plural_count(code), 1);
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
        let root = Path::new("/work/OpenChuckCAM");
        let at = |p: &str| root.join(p);
        assert!(is_test_file(
            root,
            &at("crates/x/src/shell/simulate_tests.rs")
        ));
        assert!(is_test_file(
            root,
            &at("crates/x/src/entry_safety/tests.rs")
        ));
        assert!(is_test_file(root, &at("crates/x/tests/a.rs")));
        assert!(!is_test_file(root, &at("crates/x/src/overview.rs")));
    }

    /// A folder named `tests` above the workspace does not make every file a
    /// test file: only the path below the root is looked at.
    #[test]
    fn req_ui_12_a_folder_above_the_workspace_is_not_read_as_tests() {
        let root = Path::new("/home/dev/tests/OpenChuckCAM");
        assert!(!is_test_file(root, &root.join("crates/x/src/overview.rs")));
        assert!(is_test_file(root, &root.join("crates/x/tests/a.rs")));
    }
}
