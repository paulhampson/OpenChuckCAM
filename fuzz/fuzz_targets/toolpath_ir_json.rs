// SPDX-License-Identifier: AGPL-3.0-or-later
//! The serialised neutral toolpath IR (GC-01): loading and validating any bytes
//! must give a value or an error, never a crash (NFR-07).
#![no_main]

use libfuzzer_sys::fuzz_target;
use openchuck_types::ir::Toolpath;

fuzz_target!(|data: &[u8]| {
    if let Ok(toolpath) = serde_json::from_slice::<Toolpath>(data) {
        let _ = toolpath.validate();
        let _ = toolpath.total_duration();
    }
});
