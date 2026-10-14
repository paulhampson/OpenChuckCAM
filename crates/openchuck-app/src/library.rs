// SPDX-License-Identifier: AGPL-3.0-or-later
//! Where the user's machine and tool libraries live by default (SET-01,
//! TL-01; OI-53; decision record 0029 item 15): `OpenChuckCAM/machines` and
//! `OpenChuckCAM/tools` in the platform's configuration folder.
//!
//! | Platform | Configuration folder |
//! | --- | --- |
//! | Linux and other Unix | `$XDG_CONFIG_HOME`, else `$HOME/.config` |
//! | Windows | `%APPDATA%` |
//! | macOS | `$HOME/Library/Application Support` |
//!
//! The rule is a few lines over the environment, written here rather than
//! taken from the `dirs` crate, whose `option-ext` dependency is MPL-2.0 and
//! outside the licence allow list (decision record 0029 item 15). A relative
//! or empty variable is ignored, as the XDG specification asks.

use std::ffi::OsString;
use std::path::PathBuf;

/// The folder name under the configuration folder.
pub const APP_FOLDER: &str = "OpenChuckCAM";

/// The default library folders.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryFolders {
    /// Machine profile files (SET-01).
    pub machines: PathBuf,
    /// The tool library (TL-01).
    pub tools: PathBuf,
}

/// The platforms whose rules differ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    /// Linux and other Unix systems (the XDG rule).
    Unix,
    /// Windows.
    Windows,
    /// macOS.
    MacOs,
}

impl Platform {
    /// The platform this program was built for.
    pub const fn current() -> Self {
        if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::MacOs
        } else {
            Self::Unix
        }
    }
}

/// The configuration folder on `platform`, reading variables with `var`.
/// `None` when the variables it needs are unset, empty or relative.
pub fn config_folder_from(
    platform: Platform,
    var: impl Fn(&str) -> Option<OsString>,
) -> Option<PathBuf> {
    let absolute = |name: &str| var(name).map(PathBuf::from).filter(|p| p.is_absolute());
    match platform {
        Platform::Unix => {
            absolute("XDG_CONFIG_HOME").or_else(|| absolute("HOME").map(|h| h.join(".config")))
        }
        Platform::Windows => absolute("APPDATA"),
        Platform::MacOs => absolute("HOME").map(|h| h.join("Library").join("Application Support")),
    }
}

/// The default library folders on this machine, or `None` when the
/// configuration folder cannot be found. The folders may not exist yet.
pub fn default_folders() -> Option<LibraryFolders> {
    let base =
        config_folder_from(Platform::current(), |name| std::env::var_os(name))?.join(APP_FOLDER);
    Some(LibraryFolders {
        machines: base.join("machines"),
        tools: base.join("tools"),
    })
}
