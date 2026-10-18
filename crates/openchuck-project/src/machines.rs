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
//! ([`MAKERA_Z1`]), each value marked confirmed or unconfirmed (the A direction
//! was confirmed by the SP-12 air cut, decision 0007). A library is any folder of profile files.

use std::path::{Path, PathBuf};

use openchuck_types::machine::MachineProfile;

use crate::error::ProjectError;
use crate::file::{FileKind, MachineFile, parse, to_text};

/// The machine profile file schema this build writes and reads.
///
/// - 1: slice 3 (decision record 0011).
/// - 2: the profile may hold an `id` that project copies keep, so a copy can
///   be compared with its source (OI-54, decision record 0029 item 16).
/// - 3: the feed limits may say how the controller counts A in the feed word
///   (`max_feeds.rotary_in_feed`, decision record 0076).
pub const MACHINE_SCHEMA: i64 = 3;

/// Profile file migrations, oldest first (see `file::MIGRATIONS`).
const MACHINE_MIGRATIONS: &[crate::file::Migration] = &[machine_1_to_2, machine_2_to_3];

/// Schema 2 only adds the optional `id`, so a schema 1 file needs no change
/// but its number. Its profile has no ID until it is saved with one.
fn machine_1_to_2(_kind: FileKind, _table: &mut toml::Table) -> Result<(), String> {
    Ok(())
}

/// Schema 3 adds `max_feeds.rotary_in_feed`; see [`fill_rotary_in_feed`].
fn machine_2_to_3(_kind: FileKind, table: &mut toml::Table) -> Result<(), String> {
    fill_rotary_in_feed(table);
    Ok(())
}

/// The ID of the built-in Makera Z1 profile, which every copy made from it
/// keeps.
pub const MAKERA_Z1_ID: &str = "8f0c2a4e-1b7d-4e3a-9c61-5d2e7f4a1b01";

/// The built-in Makera Z1 profile's name, which a copy stored without its ID
/// keeps (decision record 0109).
const MAKERA_Z1_NAME: &str = "Makera Z1";

/// Migrates a machine file from before `max_feeds.rotary_in_feed` (decision
/// record 0076): a profile with the built-in Z1's ID came from the Z1, whose
/// firmware counts A in the feed word (decision record 0007), so it gains
/// `rotary_in_feed = "makera"`; any other profile keeps the default, A
/// counted separately, which is how every profile was timed before.
///
/// A copy with no ID whose name is the built-in Z1's is the Z1 too: the
/// application's `SetMachine` stored the profile without its ID (decision
/// record 0109). Erring this way only slows a move, never speeds one up.
pub(crate) fn fill_rotary_in_feed(table: &mut toml::Table) {
    let from_z1 = match table.get("id") {
        Some(id) => id.as_str() == Some(MAKERA_Z1_ID),
        None => {
            table
                .get("machine")
                .and_then(|m| m.get("name"))
                .and_then(toml::Value::as_str)
                == Some(MAKERA_Z1_NAME)
        }
    };
    if !from_z1 {
        return;
    }
    if let Some(feeds) = table
        .get_mut("machine")
        .and_then(toml::Value::as_table_mut)
        .and_then(|m| m.get_mut("max_feeds"))
        .and_then(toml::Value::as_table_mut)
    {
        feeds
            .entry("rotary_in_feed")
            .or_insert_with(|| toml::Value::String("makera".into()));
    }
}

/// A new machine ID, for a profile entering a library without one.
pub fn new_machine_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// The built-in Makera Z1 profile's key.
pub const MAKERA_Z1: &str = "makera-z1";

/// Built-in profiles as (key, file text).
const BUILTIN: &[(&str, &str)] = &[(MAKERA_Z1, include_str!("../machines/makera-z1.toml"))];

/// A profile as a library or a project stores it: the profile and the ID
/// that a library profile shares with every project copy made from it
/// (OI-128). The ID is kept beside the profile rather than in it, so the
/// shared profile type in `openchuck-types` does not change (decision record
/// 0029 item 16).
#[derive(Debug, Clone, PartialEq)]
pub struct ProfileEntry {
    /// Opaque text, usually a UUID; `None` for a profile never given one.
    pub id: Option<String>,
    /// The profile.
    pub profile: MachineProfile,
}

/// Checks a machine ID: 1 to 64 characters with no leading or trailing
/// space.
pub fn validate_machine_id(id: &str) -> Result<(), openchuck_types::Error> {
    if id.trim().is_empty() || id.trim() != id || id.len() > 64 {
        return Err(openchuck_types::Error::invalid(
            "id",
            "the machine ID must be 1 to 64 characters with no leading or trailing space",
        ));
    }
    Ok(())
}

/// Reads a profile file's text with its ID. `path` names the file in
/// messages. The profile and the ID are validated.
pub fn parse_profile_entry(path: &Path, text: &str) -> Result<ProfileEntry, ProjectError> {
    let file: MachineFile = parse(
        path,
        text,
        FileKind::Machine,
        MACHINE_MIGRATIONS,
        MACHINE_SCHEMA,
    )?;
    let bad = |reason: String| ProjectError::BadFile {
        path: path.to_path_buf(),
        reason,
    };
    file.machine.validate().map_err(|e| bad(e.to_string()))?;
    if let Some(id) = &file.id {
        validate_machine_id(id).map_err(|e| bad(e.to_string()))?;
    }
    Ok(ProfileEntry {
        id: file.id,
        profile: file.machine,
    })
}

/// Reads a profile file's text. `path` names the file in messages. The
/// profile is validated.
pub fn parse_profile(path: &Path, text: &str) -> Result<MachineProfile, ProjectError> {
    parse_profile_entry(path, text).map(|e| e.profile)
}

/// The text of a profile file for `entry`.
pub fn render_profile_entry(entry: &ProfileEntry) -> Result<String, ProjectError> {
    entry.profile.validate()?;
    if let Some(id) = &entry.id {
        validate_machine_id(id)?;
    }
    to_text(&MachineFile {
        schema: MACHINE_SCHEMA,
        id: entry.id.clone(),
        machine: entry.profile.clone(),
    })
}

/// The text of a profile file for `profile`, with no ID.
pub fn render_profile(profile: &MachineProfile) -> Result<String, ProjectError> {
    render_profile_entry(&ProfileEntry {
        id: None,
        profile: profile.clone(),
    })
}

/// Reads the profile file at `path` with its ID.
pub fn load_profile_entry(path: &Path) -> Result<ProfileEntry, ProjectError> {
    let text = std::fs::read_to_string(path).map_err(|source| ProjectError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    parse_profile_entry(path, &text)
}

/// Reads the profile file at `path`.
pub fn load_profile(path: &Path) -> Result<MachineProfile, ProjectError> {
    load_profile_entry(path).map(|e| e.profile)
}

/// Writes `entry` to a profile file at `path`.
pub fn save_profile_entry(entry: &ProfileEntry, path: &Path) -> Result<(), ProjectError> {
    let text = render_profile_entry(entry)?;
    std::fs::write(path, text).map_err(|source| ProjectError::Io {
        path: path.to_path_buf(),
        source,
    })
}

/// Writes `profile` to a profile file at `path`, with no ID.
pub fn save_profile(profile: &MachineProfile, path: &Path) -> Result<(), ProjectError> {
    save_profile_entry(
        &ProfileEntry {
            id: None,
            profile: profile.clone(),
        },
        path,
    )
}

/// A library's profile files, each read or with its error.
pub type LibraryListing = Vec<(PathBuf, Result<MachineProfile, ProjectError>)>;

/// A library's profile files, each read with its ID or with its error.
pub type LibraryEntries = Vec<(PathBuf, Result<ProfileEntry, ProjectError>)>;

/// The keys of the built-in profiles, in order.
pub fn builtin_keys() -> Vec<&'static str> {
    BUILTIN.iter().map(|(key, _)| *key).collect()
}

/// The built-in profile whose key or name is `key` (ignoring case), with its
/// fixed ID.
pub fn builtin_entry(key: &str) -> Option<Result<ProfileEntry, ProjectError>> {
    BUILTIN.iter().find_map(|(k, text)| {
        let entry = parse_profile_entry(Path::new(&format!("built-in {k}")), text);
        let matches = k.eq_ignore_ascii_case(key)
            || entry
                .as_ref()
                .is_ok_and(|e| e.profile.name.eq_ignore_ascii_case(key.trim()));
        matches.then_some(entry)
    })
}

/// The built-in profile whose key or name is `key` (ignoring case).
pub fn builtin(key: &str) -> Option<Result<MachineProfile, ProjectError>> {
    builtin_entry(key).map(|r| r.map(|e| e.profile))
}

/// Every `.toml` profile file directly in `dir`, by file name, each read
/// with its ID or with its error. A missing folder has none.
pub fn list_library_entries(dir: &Path) -> Result<LibraryEntries, ProjectError> {
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
            let entry = load_profile_entry(&path);
            (path, entry)
        })
        .collect())
}

/// Every `.toml` profile file directly in `dir`, by file name, each read or
/// with its error. A missing folder has none.
pub fn list_library(dir: &Path) -> Result<LibraryListing, ProjectError> {
    Ok(list_library_entries(dir)?
        .into_iter()
        .map(|(path, entry)| (path, entry.map(|e| e.profile)))
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
        assert_eq!(z1.name, MAKERA_Z1_NAME);
        assert_eq!(z1.rotary_axis, RotaryAxis::X);
        assert_eq!(z1.a_sense, ASense::Positive);
        // Every field says where it came from.
        for field in ProfileField::ALL {
            assert!(z1.provenance.contains_key(&field), "{field} is not marked");
        }
        // The owner confirmed the axis, direction, range and feed limits from
        // use of the machine. The reference point (OI-60), the travel frame
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
        assert!(
            text.starts_with(&format!("schema = {MACHINE_SCHEMA}\n")),
            "{text}"
        );
        let back = parse_profile(Path::new("z1.toml"), &text).unwrap();
        assert_eq!(back, z1);
        assert_eq!(render_profile(&back).unwrap(), text);
    }

    #[test]
    fn req_nfr_07_bad_profile_files_give_clear_errors() {
        let z1 = render_profile(&builtin(MAKERA_Z1).unwrap().unwrap()).unwrap();
        let current = format!("schema = {MACHINE_SCHEMA}");
        let newer = format!("schema = {}", MACHINE_SCHEMA + 1);
        let cases = [
            ("not toml [", "z1.toml"),
            ("[machine]\nname = \"x\"\n", "schema"),
            (&z1.replace(&current, &newer), "newer"),
            (
                &z1.replace("rotary_axis = \"x\"", "rotary_axis = \"y\""),
                "SET-02",
            ),
            (
                &z1.replace("linear = 1200.0", "linear = -1.0"),
                "max_feeds.linear",
            ),
            (&z1.replace("safe_z = 50.0", "safe_z = 10.0"), "safe Z"),
            (
                &z1.replace("a_sense = \"positive\"", "a_sense = \"up\""),
                "a_sense",
            ),
            (
                &z1.replace("[machine.clearance]", "[machine.clearance]\nwidth = 3.0"),
                "width",
            ),
        ];
        // A bad ID is refused too.
        let with_id = render_profile_entry(&builtin_entry(MAKERA_Z1).unwrap().unwrap()).unwrap();
        let bad_id = with_id.replace("id = \"8f0c2a4e", "id = \" 8f0c2a4e");
        let err = parse_profile_entry(Path::new("z1.toml"), &bad_id)
            .unwrap_err()
            .to_string();
        assert!(err.contains("machine ID"), "{err}");
        for (text, expected) in cases {
            let err = parse_profile(Path::new("z1.toml"), text)
                .unwrap_err()
                .to_string();
            assert!(err.contains(expected), "{expected}: {err}");
        }
    }

    #[test]
    fn req_set_01_profiles_written_before_ids_still_read() {
        // A profile file written before IDs (OI-54) still reads; its profile
        // has no ID, so nothing can be compared with it until it gets one.
        let z1 = render_profile(&builtin(MAKERA_Z1).unwrap().unwrap()).unwrap();
        let old: String = z1
            .replace(&format!("schema = {MACHINE_SCHEMA}"), "schema = 1")
            .lines()
            .filter(|l| !l.starts_with("id = "))
            .map(|l| format!("{l}\n"))
            .collect();
        let entry = parse_profile_entry(Path::new("old.toml"), &old).unwrap();
        assert_eq!(entry.id, None);
        assert_eq!(entry.profile.name, "Makera Z1");
        // The built-in profile has its fixed ID, and it round-trips.
        let z1 = builtin_entry(MAKERA_Z1).unwrap().unwrap();
        assert_eq!(
            z1.id.as_deref(),
            Some("8f0c2a4e-1b7d-4e3a-9c61-5d2e7f4a1b01")
        );
        let text = render_profile_entry(&z1).unwrap();
        assert_eq!(
            parse_profile_entry(Path::new("z1.toml"), &text).unwrap(),
            z1
        );
        assert_ne!(new_machine_id(), new_machine_id());
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
