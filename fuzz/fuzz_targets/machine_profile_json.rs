// SPDX-License-Identifier: AGPL-3.0-or-later
//! Machine profiles are user-supplied files (NFR-07, verification plan section 5):
//! loading and validating any bytes must give a value or an error, never a crash.
#![no_main]

use libfuzzer_sys::fuzz_target;
use openchuck_types::machine::MachineProfile;

fuzz_target!(|data: &[u8]| {
    if let Ok(profile) = serde_json::from_slice::<MachineProfile>(data) {
        let _ = profile.validate();
    }
});
