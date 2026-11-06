// SPDX-License-Identifier: AGPL-3.0-or-later
//! The arbitrary-face rows of a facing operation are JSON text kept in a
//! project parameter (OP-24, NFR-07): decoding any text gives rows that pass
//! the row checks or an error, never a crash, and rows that decode encode and
//! decode again to the same rows. Seeded from `fuzz/seeds/facing_rows/`.
#![no_main]

use libfuzzer_sys::fuzz_target;
use openchuck_path::facing::selection::{decode_rows, encode_rows};

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    if let Ok(rows) = decode_rows(text) {
        let written = encode_rows(&rows).expect("rows that decode encode");
        assert_eq!(decode_rows(&written).expect("written rows decode"), rows);
    }
});
