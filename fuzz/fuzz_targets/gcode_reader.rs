// SPDX-License-Identifier: AGPL-3.0-or-later
//! The G-code reader and replay read text that may come from anywhere
//! (GC-08, NFR-07; decision record 0005; work breakdown slice 6 item 7):
//! any text gives a program or errors naming a line, and a program that reads
//! also replays or gives an error, never a crash. Seeded from
//! `testdata/gcode/` and `testdata/golden/post_*.ngc`.
#![no_main]

use libfuzzer_sys::fuzz_target;
use openchuck_gcode::{Dialect, read, replay};

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    if let Ok(program) = read(text) {
        let _ = replay(&program, Dialect::Linuxcnc);
        let _ = replay(&program, Dialect::MakeraZ1);
    }
});
