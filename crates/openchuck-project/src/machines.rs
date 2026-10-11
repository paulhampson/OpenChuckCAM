// SPDX-License-Identifier: AGPL-3.0-or-later
//! Machine profiles stored separately from projects (SET-01; decision record
//! 0011).
//!
//! A profile file is TOML with its own schema version and the profile under
//! `[machine]`:
//!
//! ```toml
//! schema = 1
//!
//! [machine]
//! name = "Makera Z1"
//! rotary_axis = "x"
//! a_sense = "positive"
//! # …
//! ```
//!
//! A project keeps its own copy in `machine.toml` (the same layout, with the
//! project schema), so changing a library profile never changes a project.
//! Built-in profiles ship inside the program; today that is the Makera Z1
//! ([`MAKERA_Z1`]), every value marked unconfirmed until it is checked on a
//! Z1. A library is any folder of profile files.

use std::path::{Path, PathBuf};

use openchuck_types::machine::MachineProfile;

use crate::error::ProjectError;
use crate::file::{FileKind, MachineFile, parse, to_text};

/// The machine profile file schema this build writes and reads.
///
/// - 1: slice 3 (decision record 0011).
pub const MACHINE_SCHEMA: i64 = 1;

/// The built-in Makera Z1 profile's key.
pub const MAKERA_Z1: &str = "makera-z1";

/// Built-in profiles as (key, file text).
const BUILTIN: &[(&str, &str)] = &[(MAKERA_Z1, include_str!("../machines/makera-z1.toml"))];

/// Reads a profile file's text. `path` names the file in messages. The
/// profile is validated.
pub fn parse_profile(path: &Path, text: &str) -> Result<MachineProfile, ProjectError> {
    let file: MachineFile = parse(path, text, FileKind::Machine, &[], MACHINE_SCHEMA)?;
    file.machine.validate().map_err(|e| ProjectError::BadFile {
        path: path.to_path_buf(),
        reason: e.to_string(),
    })?;
    Ok(file.machine)
}

/// The text of a profile file for `profile`.
pub fn render_profile(profile: &MachineProfile) -> Result<String, ProjectError> {
    profile.validate()?;
    to_text(&MachineFile {
        schema: MACHINE_SCHEMA,
        machine: profile.clone(),
    })
}

/// Reads the profile file at `path`.
pub fn load_profile(path: &Path) -> Result<MachineProfile, ProjectError> {
    let text = std::fs::read_to_string(path).map_err(|source| ProjectError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    parse_profile(path, &text)
}

/// Writes `profile` to a profile file at `path`.
pub fn save_profile(profile: &MachineProfile, path: &Path) -> Result<(), ProjectError> {
    let text = render_profile(profile)?;
    std::fs::write(path, text).map_err(|source| ProjectError::Io {
        path: path.to_path_buf(),
        source,
    })
}

/// A library's profile files, each read or with its error.
pub type LibraryListing = Vec<(PathBuf, Result<MachineProfile, ProjectError>)>;

/// The keys of the built-in profiles, in order.
pub fn builtin_keys() -> Vec<&'static str> {
    BUILTIN.iter().map(|(key, _)| *key).collect()
}

/// The built-in profile whose key or name is `key` (ignoring case).
pub fn builtin(key: &str) -> Option<Result<MachineProfile, ProjectError>> {
    BUILTIN.iter().find_map(|(k, text)| {
        let profile = parse_profile(Path::new(&format!("built-in {k}")), text);
        let matches = k.eq_ignore_ascii_case(key)
            || profile
                .as_ref()
                .is_ok_and(|p| p.name.eq_ignore_ascii_case(key.trim()));
        matches.then_some(profile)
    })
}

/// Every `.toml` profile file directly in `dir`, by file name, each read or
/// with its error. A missing folder has none.
pub fn list_library(dir: &Path) -> Result<LibraryListing, ProjectError> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => {
            return Err(ProjectError::Io {
                path: dir.to_path_buf(),
                source,
            });
        }
    };
    let mut paths = Vec::new();
    for entry in entries {
        let path = entry
            .map_err(|source| ProjectError::Io {
                path: dir.to_path_buf(),
                source,
            })?
            .path();
        if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("toml") {
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths
        .into_iter()
        .map(|path| {
            let profile = load_profile(&path);
            (path, profile)
        })
        .collect())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::panic)]

    use super::*;
    use openchuck_types::machine::{ASense, ProfileField, RotaryAxis};

    #[test]
    fn req_set_01_builtin_z1_reads_and_marks_every_value() {
        let z1 = builtin(MAKERA_Z1).unwrap().unwrap();
        assert_eq!(z1.name, "Makera Z1");
        assert_eq!(z1.rotary_axis, RotaryAxis::X);
        assert_eq!(z1.a_sense, ASense::Positive);
        // Every field says where it came from.
        for field in ProfileField::ALL {
            assert!(z1.provenance.contains_key(&field), "{field} is not marked");
        }
        // The owner confirmed the axis, direction, range and feed limits from
        // use of the machine. The reference point (OI-57), the travel frame
        // and the default clearance are not machine-checked values.
        assert_eq!(
            z1.unconfirmed_fields(),
            vec![
                ProfileField::RotaryOrigin,
                ProfileField::Travel,
                ProfileField::Clearance
            ]
        );
        for field in [
            ProfileField::RotaryAxis,
            ProfileField::ASense,
            ProfileField::ARange,
            ProfileField::MaxFeeds,
        ] {
            assert!(z1.is_confirmed(field), "{field}");
        }
        assert_eq!(builtin("makera z1").unwrap().unwrap(), z1);
        assert!(builtin("no such machine").is_none());
    }

    #[test]
    fn req_set_01_profile_file_round_trips() {
        let z1 = builtin(MAKERA_Z1).unwrap().unwrap();
        let text = render_profile(&z1).unwrap();
        assert!(text.starts_with("schema = 1\n"), "{text}");
        let back = parse_profile(Path::new("z1.toml"), &text).unwrap();
        assert_eq!(back, z1);
        assert_eq!(render_profile(&back).unwrap(), text);
    }

    #[test]
    fn req_nfr_07_bad_profile_files_give_clear_errors() {
        let z1 = render_profile(&builtin(MAKERA_Z1).unwrap().unwrap()).unwrap();
        let cases = [
            ("not toml [", "z1.toml"),
            ("[machine]\nname = \"x\"\n", "schema"),
            (&z1.replace("schema = 1", "schema = 2"), "newer"),
            (
                &z1.replace("rotary_axis = \"x\"", "rotary_axis = \"y\""),
                "SET-02",
            ),
            (
                &z1.replace("linear = 1200.0", "linear = -1.0"),
                "max_feeds.linear",
            ),
            (&z1.replace("safe_z = 60.0", "safe_z = 10.0"), "safe Z"),
            (
                &z1.replace("a_sense = \"positive\"", "a_sense = \"up\""),
                "a_sense",
            ),
            (
                &z1.replace("[machine.clearance]", "[machine.clearance]\nwidth = 3.0"),
                "width",
            ),
        ];
        for (text, expected) in cases {
            let err = parse_profile(Path::new("z1.toml"), text)
                .unwrap_err()
                .to_string();
            assert!(err.contains(expected), "{expected}: {err}");
        }
    }

    #[test]
    fn req_set_01_library_lists_profile_files() {
        let dir = std::env::temp_dir().join(format!("openchuck-machines-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let z1 = builtin(MAKERA_Z1).unwrap().unwrap();
        save_profile(&z1, &dir.join("z1.toml")).unwrap();
        std::fs::write(dir.join("broken.toml"), "schema = 1\n").unwrap();
        std::fs::write(dir.join("notes.txt"), "ignored").unwrap();
        let listed = list_library(&dir).unwrap();
        assert_eq!(listed.len(), 2);
        assert!(listed[0].0.ends_with("broken.toml") && listed[0].1.is_err());
        assert_eq!(listed[1].1.as_ref().unwrap(), &z1);
        assert!(list_library(&dir.join("missing")).unwrap().is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    proptest::proptest! {
        #[test]
        fn req_nfr_07_profile_parser_never_panics_on_text(text in ".{0,400}") {
            let _ = parse_profile(Path::new("fuzz.toml"), &text);
        }

        #[test]
        fn req_nfr_07_profile_parser_never_panics_on_damaged_files(
            cut in 0usize..2000,
            insert in ".{0,12}",
        ) {
            let text = include_str!("../machines/makera-z1.toml");
            let mut cut = cut.min(text.len());
            while !text.is_char_boundary(cut) {
                cut -= 1;
            }
            let damaged = format!("{}{insert}{}", &text[..cut], &text[cut..]);
            if let Ok(profile) = parse_profile(Path::new("fuzz.toml"), &damaged) {
                let written = render_profile(&profile).unwrap();
                proptest::prop_assert_eq!(parse_profile(Path::new("x.toml"), &written).unwrap(), profile);
            }
        }
    }
}
