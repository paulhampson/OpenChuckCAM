// SPDX-License-Identifier: AGPL-3.0-or-later
//! The per-user `settings.toml` in the configuration folder is user-editable
//! text (GC-16 export acknowledgements, UI-15 interface settings; NFR-07,
//! OI-327): reading any text through either reader gives a value or a
//! sentence for the user, never a crash or a hang. Seeded from
//! `fuzz/seeds/settings_toml/`.
#![no_main]

use std::path::Path;

use libfuzzer_sys::fuzz_target;
use openchuck_app::Acknowledgements;
use openchuck_app::settings::SettingsFile;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    // The export acknowledgements: a damaged file always means "ask again".
    let ack = Acknowledgements::in_folder(Path::new("fuzz"));
    let (shown, skipped) = ack.lookup_text(text, Path::new("/fuzz/project"));
    if shown.problem.is_some() {
        assert!(!shown.acknowledged && !skipped.acknowledged);
    }
    // The interface table: damage gives the defaults and a message; what is
    // read is already in range, so clamping it again changes nothing.
    let loaded = SettingsFile::in_folder(Path::new("fuzz")).load_text(text);
    let again = loaded.settings.clone().clamped();
    if let (Ok(a), Ok(b)) = (toml::to_string(&loaded.settings), toml::to_string(&again)) {
        assert_eq!(a, b);
    }
});
