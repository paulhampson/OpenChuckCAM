// SPDX-License-Identifier: AGPL-3.0-or-later
//! The default library folders on each platform (SET-01, TL-01; OI-53;
//! decision record 0029 item 15).

#![allow(clippy::unwrap_used)]

use std::ffi::OsString;
use std::path::PathBuf;

use openchuck_app::library::{Platform, config_folder_from};

fn env<'a>(vars: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<OsString> + 'a {
    move |name| {
        vars.iter()
            .find(|(k, _)| *k == name)
            .map(|(_, v)| OsString::from(v))
    }
}

#[cfg(unix)]
#[test]
fn req_set_01_req_tl_01_unix_and_macos_config_folders() {
    // XDG_CONFIG_HOME wins; otherwise ~/.config; relative values are ignored.
    let unix = |vars| config_folder_from(Platform::Unix, env(vars));
    assert_eq!(
        unix(&[("XDG_CONFIG_HOME", "/x/cfg"), ("HOME", "/home/p")]),
        Some(PathBuf::from("/x/cfg"))
    );
    assert_eq!(
        unix(&[("HOME", "/home/p")]),
        Some(PathBuf::from("/home/p/.config"))
    );
    assert_eq!(
        unix(&[("XDG_CONFIG_HOME", "relative"), ("HOME", "/home/p")]),
        Some(PathBuf::from("/home/p/.config"))
    );
    assert_eq!(unix(&[("XDG_CONFIG_HOME", "")]), None);
    assert_eq!(unix(&[]), None);
    assert_eq!(
        config_folder_from(Platform::MacOs, env(&[("HOME", "/Users/p")])),
        Some(PathBuf::from("/Users/p/Library/Application Support"))
    );
}

#[cfg(windows)]
#[test]
fn req_set_01_req_tl_01_windows_config_folder() {
    assert_eq!(
        config_folder_from(
            Platform::Windows,
            env(&[("APPDATA", r"C:\Users\p\AppData\Roaming")])
        ),
        Some(PathBuf::from(r"C:\Users\p\AppData\Roaming"))
    );
    assert_eq!(
        config_folder_from(Platform::Windows, env(&[("APPDATA", "rel")])),
        None
    );
}

#[test]
fn req_set_01_req_tl_01_default_folders_sit_under_the_app_folder() {
    if let Some(folders) = openchuck_app::library::default_folders() {
        assert!(folders.machines.ends_with("OpenChuckCAM/machines"));
        assert!(folders.tools.ends_with("OpenChuckCAM/tools"));
    }
}

#[test]
fn req_tl_12_installing_defaults_twice_into_a_folder_installs_nothing_the_second_time() {
    let dir = std::env::temp_dir().join(format!("openchuck-app-defaults-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let first = openchuck_app::library::install_default_tools(&dir).unwrap();
    assert_eq!(first.installed.len(), 77);
    let second = openchuck_app::library::install_default_tools(&dir).unwrap();
    assert!(second.installed.is_empty());
}
