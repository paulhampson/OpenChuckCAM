// SPDX-License-Identifier: AGPL-3.0-or-later
//! PRJ-08, NFR-07: both envelope and checksummed inner payload are untrusted.
#![no_main]
use libfuzzer_sys::fuzz_target;
use openchuck_project::recovery::Snapshot;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let _ = Snapshot::decode(text);
    // Refresh the checksum as well, so mutations reach the inner project
    // reader instead of being stopped only by the integrity boundary.
    let mut envelope = toml::Table::new();
    envelope.insert("format".into(), toml::Value::Integer(1));
    envelope.insert("payload".into(), toml::Value::String(text.into()));
    envelope.insert(
        "checksum".into(),
        toml::Value::String(blake3::hash(data).to_hex().to_string()),
    );
    if let Ok(encoded) = toml::to_string(&envelope) {
        if let Ok(snapshot) = Snapshot::decode(&encoded) {
            let _ = snapshot.project();
            if let Ok(roundtrip) = snapshot.encode() {
                assert!(Snapshot::decode(&roundtrip).is_ok());
            }
        }
    }
});
