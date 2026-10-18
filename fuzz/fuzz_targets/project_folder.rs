// SPDX-License-Identifier: AGPL-3.0-or-later
//! A whole project folder is user-supplied (NFR-07; work breakdown slice 20
//! item 4): loading any set of file texts, with the schema and migration
//! checks of every file and the checks across files (references between
//! items, order keys, file names), must give a project or an error, never a
//! crash. The `project_files` target reads one file at a time; this one
//! reaches the checks that need the whole folder.
//!
//! The input is the folder's files one after another, each after a line
//! `=== <path in the folder>`. CI seeds the corpus from the fixture folders in
//! `crates/openchuck-project/tests/fixtures/` written in that form.
#![no_main]

use std::collections::BTreeMap;

use libfuzzer_sys::fuzz_target;
use openchuck_project::file::load_files;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let mut files = BTreeMap::new();
    let mut current: Option<(String, String)> = None;
    for line in text.split_inclusive('\n') {
        if let Some(path) = line.strip_prefix("=== ") {
            if let Some((path, body)) = current.take() {
                files.insert(path, body);
            }
            current = Some((path.trim_end().to_owned(), String::new()));
        } else if let Some((_, body)) = &mut current {
            body.push_str(line);
        }
    }
    if let Some((path, body)) = current {
        files.insert(path, body);
    }
    let _ = load_files(&files);
});
