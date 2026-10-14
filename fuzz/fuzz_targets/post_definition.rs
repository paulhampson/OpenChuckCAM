// SPDX-License-Identifier: AGPL-3.0-or-later
//! Post definition files are user-editable data (GC-03, NFR-07; work breakdown
//! slice 6 item 3): any manifest text and template text give a definition or
//! an error naming the key or line, and a definition that loads renders a
//! small toolpath or gives an error, never a crash or a hang. Seeded from
//! `posts/*/`.
#![no_main]

use libfuzzer_sys::fuzz_target;
use openchuck_post::PostDefinition;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    // The first line break splits manifest from template for the fuzzer.
    let (manifest, template) = text.split_once("\n---\n").unwrap_or((text, ""));
    let _ = PostDefinition::from_parts("fuzz", manifest, template);
});
