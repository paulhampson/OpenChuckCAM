// SPDX-License-Identifier: AGPL-3.0-or-later
//! Every warning and every piece of information the path crate makes names
//! its check (decision record 0536 section 3): a `Notice::warning(...)` or
//! `Notice::information(...)` is followed in its call chain by
//! `.about(...)`, and a `Notice { level: Level::Warning, ... }` or
//! `Level::Information` literal sets `topic: Some(...)`. The application
//! then shows it in the one form, "<Topic> (operation '<name>'): <text>".
//!
//! Every refusal names its check too (decision record 0644): a refusal is
//! a `PathError::refused(topic::..., ...)`. The variants that carry no topic
//! (`PathError::Strategy`, `Surface`, `Region`, `RotaryLimit`) are counted
//! per file against the `[strategy_refusals]` baseline of
//! `xtask/messages.toml`, which only falls.
//! Test code is not checked.

use std::path::Path;

use anyhow::Result;

use crate::Findings;

/// The source folder whose notices must carry a topic.
const PATH_SRC: &str = "crates/openchuck-path/src";

/// The text from `open` (an opening bracket) to its matching close, or to
/// the end when it never closes. Strings are not treated specially: the
/// warning texts this reads hold balanced brackets.
fn balanced(text: &str, open: usize) -> &str {
    let bytes = text.as_bytes();
    let (o, c) = match bytes.get(open) {
        Some(b'(') => (b'(', b')'),
        Some(b'{') => (b'{', b'}'),
        _ => return text.get(open..).unwrap_or_default(),
    };
    let mut depth = 0usize;
    for (i, &b) in bytes.iter().enumerate().skip(open) {
        if b == o {
            depth += 1;
        } else if b == c {
            depth -= 1;
            if depth == 0 {
                return &text[open..=i];
            }
        }
    }
    &text[open..]
}

/// Whether the call chain that starts right after `rest` names a topic:
/// `.name(...)` calls, one after another, one of them `.about(...)`.
fn chain_has_about(mut rest: &str) -> bool {
    loop {
        rest = rest.trim_start();
        let Some(after_dot) = rest.strip_prefix('.') else {
            return false;
        };
        let name_len = after_dot
            .find(|c: char| !(c.is_alphanumeric() || c == '_'))
            .unwrap_or(after_dot.len());
        let (name, tail) = after_dot.split_at(name_len);
        if name == "about" {
            return true;
        }
        let tail = tail.trim_start();
        if !tail.starts_with('(') {
            return false;
        }
        let call = balanced(tail, 0);
        rest = &tail[call.len()..];
    }
}

/// The constructors of a notice whose call chain must name a topic.
const CONSTRUCTORS: [&str; 2] = ["Notice::warning", "Notice::information"];

/// The fields of a notice literal that must set a topic.
const LEVELS: [&str; 2] = ["level: Level::Warning", "level: Level::Information"];

/// The places in `code` where a warning or a piece of information is made
/// without a topic, each as its line number from 1.
pub fn missing_topics(code: &str) -> Vec<usize> {
    let line_of = |at: usize| code[..at].matches('\n').count() + 1;
    let mut missing = Vec::new();
    for constructor in CONSTRUCTORS {
        for (at, _) in code.match_indices(&format!("{constructor}(")) {
            let open = at + constructor.len();
            let args = balanced(code, open);
            if !chain_has_about(&code[open + args.len()..]) {
                missing.push(line_of(at));
            }
        }
    }
    for (at, _) in LEVELS.iter().flat_map(|level| code.match_indices(level)) {
        // The literal that holds it: its last opening brace before it.
        let Some(open) = code[..at].rfind('{') else {
            continue;
        };
        let literal = balanced(code, open);
        if !literal.contains("topic: Some(") {
            missing.push(line_of(at));
        }
    }
    missing.sort_unstable();
    missing
}

/// The variants of a path error that carry no topic of their own.
const UNTOPICED: [&str; 4] = [
    "PathError::Strategy(",
    "PathError::Surface(",
    "PathError::Region(",
    "PathError::RotaryLimit(",
];

/// How many refusals in `code` are made with a variant that carries no
/// topic (decision record 0644).
pub fn untopiced_refusals(code: &str) -> usize {
    UNTOPICED.iter().map(|v| code.matches(v).count()).sum()
}

/// The places in `code` where a refusal's topic is not a constant of
/// `openchuck_path::topic` (or a helper's parameter named `topic`), each as
/// its line number from 1.
pub fn refusals_without_a_topic(code: &str) -> Vec<usize> {
    let line_of = |at: usize| code[..at].matches('\n').count() + 1;
    let call = "PathError::refused(";
    code.match_indices(call)
        .filter(|(at, _)| {
            let first = code[at + call.len()..].trim_start();
            let named = first.split_once("topic::").is_some_and(|(path, _)| {
                path.chars()
                    .all(|c| c.is_alphanumeric() || c == '_' || c == ':')
            });
            let parameter = first.starts_with("topic,") || first.starts_with("topic)");
            !(named || parameter)
        })
        .map(|(at, _)| line_of(at))
        .collect()
}

/// Checks the path crate's product code.
pub fn check(root: &Path) -> Result<Findings> {
    let mut findings = Findings::new();
    let dir = root.join(PATH_SRC);
    let mut files = Vec::new();
    collect(&dir, &mut files)?;
    files.sort();
    // notice.rs defines the type, so its constructors are not notices.
    let mut untopiced = std::collections::BTreeMap::new();
    for file in files.into_iter().filter(|f| !f.ends_with("notice.rs")) {
        let text = std::fs::read_to_string(&file)?;
        let code = crate::messages::product_code(&text);
        let name = file
            .strip_prefix(root)
            .unwrap_or(&file)
            .to_string_lossy()
            .replace('\\', "/");
        let n = untopiced_refusals(code);
        if n > 0 && !crate::messages::is_test_file(root, &file) {
            untopiced.insert(name.clone(), n);
        }
        for line in refusals_without_a_topic(code) {
            findings.push(format!(
                "{name}:{line}: a refusal whose topic is not a constant of \
                 openchuck_path::topic (decision 0644)"
            ));
        }
        for line in missing_topics(code) {
            findings.push(format!(
                "{}:{line}: a notice without a topic; give it one from \
                 openchuck_path::topic with .about(...) (decision 0536 section 3)",
                file.strip_prefix(root).unwrap_or(&file).display()
            ));
        }
    }
    let text = std::fs::read_to_string(root.join(crate::messages::BASELINE))?;
    let baseline: toml::Table = toml::from_str(&text)?;
    findings.extend(crate::messages::compare(
        "refusals made with a variant that has no topic",
        "refuse with PathError::refused(topic::..., ...), naming the check (decision 0644)",
        &untopiced,
        &crate::messages::table(&baseline, "strategy_refusals")?,
    ));
    Ok(findings)
}

/// The `.rs` files under `dir`.
fn collect(dir: &Path, out: &mut Vec<std::path::PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect(&path, out)?;
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn req_ui_12_a_warning_with_a_topic_passes() {
        let code = "let w = Notice::warning(format!(\"a {}\", b))\n    .about(topic::OVERCUT)\n    \
                    .with_setting(names::TOLERANCE);";
        assert!(missing_topics(code).is_empty());
        let later = "Notice::warning(\"x\").with_setting(\"t\").about(topic::OVERCUT)";
        assert!(missing_topics(later).is_empty());
    }

    #[test]
    fn req_ui_12_a_warning_without_a_topic_is_found() {
        let code = "fn f() {\n    let w = Notice::warning(\"x\").with_setting(\"t\");\n}";
        assert_eq!(missing_topics(code), [2]);
    }

    #[test]
    fn req_ui_12_information_needs_a_topic_as_a_warning_does() {
        let with = "notes.push(\n    Notice::information(format!(\"{n} rings\"))\n        \
                    .about(topic::RINGS),\n);";
        assert!(missing_topics(with).is_empty());
        let segment = "Notice::information(text).at_segment(3).about(topic)";
        assert!(missing_topics(segment).is_empty());
        let without = "fn f() {\n    let n = Notice::information(\"x\").at_segment(3);\n}";
        assert_eq!(missing_topics(without), [2]);
        let literal = "Notice {\n    level: Level::Information,\n    topic: None,\n}";
        assert_eq!(missing_topics(literal), [2]);
    }

    #[test]
    fn req_ui_12_a_warning_literal_needs_a_topic() {
        let with =
            "Notice {\n    level: Level::Warning,\n    topic: Some(topic::OVERCUT.to_owned()),\n}";
        assert!(missing_topics(with).is_empty());
        let without = "Notice {\n    level: Level::Warning,\n    topic: None,\n}";
        assert_eq!(missing_topics(without), [2]);
    }

    #[test]
    fn req_ui_12_a_refusal_without_a_topic_is_counted() {
        let code = "return Err(PathError::Strategy(\"x\".into()));\n\
                    Err(PathError::Surface(s))\n\
                    Err(PathError::refused(topic::SWARF, \"y\"))";
        assert_eq!(untopiced_refusals(code), 2);
    }

    #[test]
    fn req_ui_12_a_refusal_names_its_topic_by_a_constant() {
        let with = "PathError::refused(topic::SWARF, \"y\")\n\
                    PathError::refused(crate::topic::LINKS, x)\n\
                    fn refuse(topic: &str, r: &str) -> PathError { PathError::refused(topic, r) }";
        assert!(refusals_without_a_topic(with).is_empty());
        let without = "fn f() {\n    PathError::refused(\"Swarf\", \"y\")\n}";
        assert_eq!(refusals_without_a_topic(without), [2]);
    }
}
