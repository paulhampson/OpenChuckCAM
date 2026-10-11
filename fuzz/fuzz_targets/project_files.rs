// SPDX-License-Identifier: AGPL-3.0-or-later
//! Project files are user-supplied (NFR-07; work breakdown slice 1 item 8):
//! reading any text as any kind of project file, including the merge-conflict,
//! schema and migration checks, must give a value or an error, never a crash.
#![no_main]

use libfuzzer_sys::fuzz_target;
use openchuck_project::file::{FileKind, parse_file};

const KINDS: [FileKind; 5] = [
    FileKind::Project,
    FileKind::Setup,
    FileKind::Tool,
    FileKind::SelectionSet,
    FileKind::Operation,
];

fuzz_target!(|data: &[u8]| {
    // The first byte picks the kind of file; the rest is its text.
    let Some((&kind, text)) = data.split_first() else {
        return;
    };
    let Ok(text) = std::str::from_utf8(text) else {
        return;
    };
    if let Some(&kind) = KINDS.get(usize::from(kind) % KINDS.len()) {
        let _ = parse_file(kind, text);
    }
});
