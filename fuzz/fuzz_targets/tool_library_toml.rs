// SPDX-License-Identifier: AGPL-3.0-or-later
//! Tool library files are user-supplied and are the import and export format
//! (TL-01, NFR-07; work breakdown slice 4 item 3): reading any text as a
//! library, including the schema check and every tool's validation, must give
//! a library or an error, never a crash. Seeded from `testdata/tools/*.toml`.
#![no_main]

use libfuzzer_sys::fuzz_target;
use openchuck_project::tools::{parse_library, render_library};

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    if let Ok(library) = parse_library(std::path::Path::new("fuzz.toml"), text) {
        // A library that reads also writes, and reads back the same.
        let written = render_library(&library).expect("a valid library renders");
        let back = parse_library(std::path::Path::new("fuzz.toml"), &written)
            .expect("a rendered library reads");
        assert_eq!(back, library);
        // Every tool's outline can be built for the solvers.
        for tool in library.tools() {
            tool.tool.profile().expect("a valid tool has a profile");
        }
    }
});
