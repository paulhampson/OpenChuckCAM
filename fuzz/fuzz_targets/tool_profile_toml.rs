// SPDX-License-Identifier: AGPL-3.0-or-later
//! Custom tool-profile text files are user-supplied (TL-03, NFR-07; work
//! breakdown slice 4 item 3): reading any text as a profile file must give a
//! profile that passes the monotone rule or an error, never a crash. Seeded
//! from `testdata/tools/profiles/`.
#![no_main]

use libfuzzer_sys::fuzz_target;
use openchuck_project::tools::{parse_profile, render_profile};

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    if let Ok(profile) = parse_profile(std::path::Path::new("fuzz.toml"), text) {
        // The lower envelope is defined and finite across the whole profile.
        let solved = profile.validate().expect("a read profile is valid");
        let end = solved.cutting_radius();
        for i in 0..=16 {
            let h = solved.height_at(end * f64::from(i) / 16.0);
            assert!(h.is_some_and(f64::is_finite));
        }
        // Written with centres, it reads back the same.
        let written = render_profile(&profile).expect("a valid profile renders");
        let back = parse_profile(std::path::Path::new("fuzz.toml"), &written)
            .expect("a rendered profile reads");
        assert_eq!(back, profile);
    }
});
