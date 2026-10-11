// SPDX-License-Identifier: AGPL-3.0-or-later
//! Machine profile files are user-supplied (NFR-07; work breakdown slice 3
//! item 1): reading any text as a profile file, including the schema check
//! and validation, must give a profile or an error, never a crash. Seeded
//! from the built-in profiles in `crates/openchuck-project/machines/`.
#![no_main]

use libfuzzer_sys::fuzz_target;
use openchuck_project::machines::{parse_profile, render_profile};

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    if let Ok(profile) = parse_profile(std::path::Path::new("fuzz.toml"), text) {
        // A profile that reads also writes, and reads back the same.
        let written = render_profile(&profile).expect("a valid profile renders");
        let back = parse_profile(std::path::Path::new("fuzz.toml"), &written)
            .expect("a rendered profile reads");
        assert_eq!(back, profile);
    }
});
