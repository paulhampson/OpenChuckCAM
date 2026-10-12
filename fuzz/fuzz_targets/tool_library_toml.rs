// SPDX-License-Identifier: AGPL-3.0-or-later
//! Tool files and machine tool set files in a tool library are user-supplied
//! and are the import and export format (TL-01, TL-11, NFR-07; work breakdown
//! slice 4 item 3): reading any text as either must give a value or an
//! error, never a crash. Seeded from `testdata/tools/library/` and
//! `testdata/tools/broken/`.
#![no_main]

use libfuzzer_sys::fuzz_target;
use openchuck_project::tools::{
    parse_tool_file, parse_tool_set, render_tool_file, render_tool_set,
};

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let path = std::path::Path::new("fuzz.toml");
    if let Ok(tool) = parse_tool_file(path, text) {
        // A tool that reads also writes, reads back the same and has an outline.
        let written = render_tool_file(&tool).expect("a valid tool renders");
        let back = parse_tool_file(path, &written).expect("a rendered tool reads");
        assert_eq!(back, tool);
        tool.tool.profile().expect("a valid tool has a profile");
    }
    if let Ok(set) = parse_tool_set(path, text) {
        let written = render_tool_set(&set).expect("a valid tool set renders");
        let back = parse_tool_set(path, &written).expect("a rendered tool set reads");
        assert_eq!(back, set);
    }
});
