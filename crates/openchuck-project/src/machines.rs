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
/// - 4: the profile may give the spindle speed range, `min_spindle_rpm` and
///   `max_spindle_rpm` (decision record 0231).
/// - 5: optional minimum block time, window and mode (decision record 0222).
/// - 6: Slow down integration; schema 5 Report settings retain their meaning.
pub const MACHINE_SCHEMA: i64 = 6;

/// Profile file migrations, oldest first (see `file::MIGRATIONS`).
const MACHINE_MIGRATIONS: &[crate::file::Migration] = &[
    machine_1_to_2,
    machine_2_to_3,
    machine_3_to_4,
    machine_4_to_5,
    machine_5_to_6,
];

/// Existing Off/Report settings keep their meaning. Slow down did not exist in schema 5.
fn machine_5_to_6(_kind: FileKind, table: &mut toml::Table) -> Result<(), String> {
    premature_slow_down(table, 5, 6)
}

/// Refuse the new policy in a file whose version could only express Off or Report.
/// This applies even when a zero threshold would make the policy inactive.
pub(crate) fn premature_slow_down(
    table: &toml::Table,
    schema: i64,
    introduced: i64,
) -> Result<(), String> {
    if table
        .get("machine")
        .and_then(|machine| machine.get("short_block_mode"))
        .and_then(toml::Value::as_str)
        == Some("slow_down")
    {
        return Err(format!(
            "schema {schema} predates `short_block_mode = slow_down`; Slow down requires schema {introduced}. Choose off/report, or set `schema = {introduced}` if this file was written for Slow down"
        ));
    }
    Ok(())
}

/// Schema 4 adds the optional spindle speed range, which a schema 3 file
/// does not have, so it needs no change but its number: nothing more is
/// checked. A schema 3 file that already holds one was not written by any
/// version that knew it, so it is refused rather than guessed at.
fn machine_3_to_4(_kind: FileKind, table: &mut toml::Table) -> Result<(), String> {
    premature_spindle_range(table, 3)
}

/// Refuses a machine file of schema `schema` that already holds a spindle
/// speed range, a field introduced after it (decision record 0231).
pub(crate) fn premature_spindle_range(table: &toml::Table, schema: i64) -> Result<(), String> {
    let machine = table.get("machine").and_then(toml::Value::as_table);
    match ["min_spindle_rpm", "max_spindle_rpm"]
        .into_iter()
        .find(|f| machine.is_some_and(|m| m.contains_key(*f)))
    {
        Some(field) => Err(format!(
            "this schema {schema} file has `{field}`, a field schema {} introduced; remove it, \
             or set `schema = {}` if the file was written for it",
            schema + 1,
            schema + 1
        )),
        None => Ok(()),
    }
}

fn machine_4_to_5(_kind: FileKind, table: &mut toml::Table) -> Result<(), String> {
    premature_block_time(table, 4)
}

/// Refuse fields labelled with a schema that predates them.
pub(crate) fn premature_block_time(table: &toml::Table, schema: i64) -> Result<(), String> {
    if let Some(machine) = table.get("machine").and_then(toml::Value::as_table) {
        for field in ["min_block_ms", "block_window", "short_block_mode"] {
            if machine.contains_key(field) {
                return Err(format!(
                    "schema {schema} predates `{field}`; use the current schema or remove the field"
                ));
            }
        }
    }
    Ok(())
}

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

/// The axis height the built-in Z1 profile held before the owner's
/// measurement (decision record 0317): machine Z -55, from the chuck centre
/// 45 mm above a bed taken at the bottom of the Z travel.
const OLD_Z1_AXIS_HEIGHT: f64 = -55.0;

/// Whether `profile`, stored with library ID `id`, is a copy of the built-in
/// Makera Z1 that still has the old placeholder axis height (decision record
/// 0317). It is the Z1 by its ID, or, with no ID, by its name (decision
/// record 0109). The copy is not changed: the project's copy is the source of
/// truth (SET-01), so the caller warns and the user chooses the Z1 again.
pub fn has_old_z1_axis_height(id: Option<&str>, profile: &MachineProfile) -> bool {
    let z1 = match id {
        Some(id) => id == MAKERA_Z1_ID,
        None => profile.name == MAKERA_Z1_NAME,
    };
    #[allow(clippy::float_cmp)] // The exact value the old built-in profile wrote.
    let old = profile.rotary_origin.z == OLD_Z1_AXIS_HEIGHT;
    z1 && old
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
    #![allow(clippy::unwrap_used, clippy::panic, clippy::float_cmp)]

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
        // use of the machine, and the axis height by Makera's probing routine
        // (decision 0317). The travel frame and the default clearance are not
        // machine-checked values, and the spindle range is Makera's published
        // figure (decision 0231).
        assert_eq!(
            z1.unconfirmed_fields(),
            vec![
                ProfileField::Travel,
                ProfileField::Clearance,
                ProfileField::BlockWindow,
                ProfileField::SpindleRange,
            ]
        );
        for field in [
            ProfileField::RotaryAxis,
            ProfileField::ASense,
            ProfileField::RotaryOrigin,
            ProfileField::ARange,
            ProfileField::MaxFeeds,
        ] {
            assert!(z1.is_confirmed(field), "{field}");
        }
        assert_eq!(builtin("makera z1").unwrap().unwrap(), z1);
        assert!(builtin("no such machine").is_none());
    }

    /// The axis height is the owner's probing result, for the probe as the
    /// reference tool; the default safe Z and safe cylinder are 40 mm, so the
    /// safe Z is inside the Z travel (decision 0317, OI-60).
    #[test]
    fn req_set_01_req_set_06_the_z1_axis_height_is_measured_and_its_safe_z_is_in_the_travel() {
        let z1 = builtin(MAKERA_Z1).unwrap().unwrap();
        assert_eq!(z1.rotary_origin.z, -48.6);
        assert_eq!((z1.rotary_origin.x, z1.rotary_origin.y), (-180.4, -108.8));
        assert!(z1.is_confirmed(ProfileField::RotaryOrigin));
        let source = &z1.provenance[&ProfileField::RotaryOrigin].source;
        // It says what was measured, with what, and what was not recorded.
        for fact in [
            "-48.6",
            "probe",
            "reference tool",
            "G54",
            "tool length offset",
            "datum",
            "0317",
        ] {
            assert!(source.contains(fact), "{fact}: {source}");
        }
        assert_eq!(z1.clearance.radius.0, 40.0);
        assert_eq!(z1.clearance.safe_z.0, 40.0);
        let safe = z1.rotary_origin.z + z1.clearance.safe_z.0;
        assert!((safe - -8.6).abs() < 1e-9, "{safe}");
        assert!(safe <= z1.travel.max.z && safe >= z1.travel.min.z);
        assert!(!z1.is_confirmed(ProfileField::Clearance));
    }

    /// A project copy of the Z1 that still has the old placeholder axis
    /// height (machine Z -55) is recognised by its ID, or by its name when it
    /// has none (decision records 0109 and 0317).
    #[test]
    fn req_set_01_req_prj_04_a_z1_copy_with_the_old_axis_height_is_recognised() {
        let z1 = builtin(MAKERA_Z1).unwrap().unwrap();
        let mut old = z1.clone();
        old.rotary_origin.z = -55.0;
        assert!(has_old_z1_axis_height(Some(MAKERA_Z1_ID), &old));
        assert!(has_old_z1_axis_height(None, &old));
        assert!(!has_old_z1_axis_height(Some(MAKERA_Z1_ID), &z1));
        assert!(!has_old_z1_axis_height(None, &z1));
        // Another machine's profile, or a renamed copy with no ID, is the
        // user's own: its axis height is theirs.
        assert!(!has_old_z1_axis_height(Some("another-id"), &old));
        let mut renamed = old.clone();
        renamed.name = "My Z1".to_owned();
        assert!(!has_old_z1_axis_height(None, &renamed));
        // A user's own measured value on the Z1 copy is not flagged.
        let mut measured = old;
        measured.rotary_origin.z = -50.2;
        assert!(!has_old_z1_axis_height(Some(MAKERA_Z1_ID), &measured));
    }

    #[test]
    fn req_gc_17_req_set_01_new_z1_uses_measured_report_settings_and_old_profiles_stay_off() {
        use openchuck_types::machine::ShortBlockMode;
        let z1 = builtin(MAKERA_Z1).unwrap().unwrap();
        assert_eq!(
            (z1.min_block_ms, z1.block_window, z1.short_block_mode),
            (7.0, 32, ShortBlockMode::Report)
        );
        assert!(z1.is_confirmed(ProfileField::MinBlockMs));
        let source = &z1.provenance[&ProfileField::MinBlockMs].source;
        for fact in ["284", "1.1.2", "WiFi", "version not supplied"] {
            assert!(source.contains(fact), "{source}");
        }
        let written = render_profile(&z1).unwrap();
        let old = written.replace(&format!("schema = {MACHINE_SCHEMA}"), "schema = 4");
        assert!(
            parse_profile(Path::new("old.toml"), &old)
                .unwrap_err()
                .to_string()
                .contains("min_block_ms")
        );
        let mut table: toml::Table = old.parse().unwrap();
        let machine = table.get_mut("machine").unwrap().as_table_mut().unwrap();
        for key in ["min_block_ms", "block_window", "short_block_mode"] {
            machine.remove(key);
        }
        let old = toml::to_string(&table).unwrap();
        let loaded = parse_profile(Path::new("old.toml"), &old).unwrap();
        assert_eq!(
            (
                loaded.min_block_ms,
                loaded.block_window,
                loaded.short_block_mode
            ),
            (0.0, 1, ShortBlockMode::Off)
        );
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
            (&z1.replace("safe_z = 40.0", "safe_z = 10.0"), "safe Z"),
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

    /// The built-in Z1 gives Makera's published highest spindle speed,
    /// unconfirmed; the range needs profile schema 4 (decision 0231).
    #[test]
    fn req_gc_14_req_set_01_the_z1_spindle_range_is_unconfirmed_and_needs_schema_4() {
        use openchuck_types::machine::{Confirmation, ProfileField};
        let mut z1 = builtin(MAKERA_Z1).unwrap().unwrap();
        assert_eq!(z1.max_spindle_rpm.map(|s| s.0), Some(13000.0));
        assert_eq!(z1.min_spindle_rpm, None);
        let p = &z1.provenance[&ProfileField::SpindleRange];
        assert_eq!(p.state, Confirmation::Unconfirmed);
        assert!(p.source.contains("Safety Guidelines"), "{}", p.source);
        // A pre-schema-5 fixture cannot contain the later GC-17 fields.
        z1.min_block_ms = 0.0;
        z1.block_window = 1;
        z1.short_block_mode = openchuck_types::machine::ShortBlockMode::Off;
        for field in [
            ProfileField::MinBlockMs,
            ProfileField::BlockWindow,
            ProfileField::ShortBlockMode,
        ] {
            z1.provenance.remove(&field);
        }
        let text = render_profile(&z1).unwrap();
        let old = text.replace(&format!("schema = {MACHINE_SCHEMA}"), "schema = 3");
        let err = parse_profile(Path::new("z1.toml"), &old)
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("max_spindle_rpm") && err.contains("schema 4"),
            "{err}"
        );
        // Without the range, a schema 3 file reads with none.
        let plain: String = old
            .lines()
            .filter(|l| !l.starts_with("max_spindle_rpm"))
            .map(|l| format!("{l}\n"))
            .collect();
        assert_eq!(
            parse_profile(Path::new("z1.toml"), &plain)
                .unwrap()
                .max_spindle_rpm,
            None
        );
    }

    #[test]
    fn req_set_01_profiles_written_before_ids_still_read() {
        // A profile file written before IDs (OI-54) still reads; its profile
        // has no ID, so nothing can be compared with it until it gets one.
        let mut profile = builtin(MAKERA_Z1).unwrap().unwrap();
        profile.min_block_ms = 0.0;
        profile.block_window = 1;
        profile.short_block_mode = openchuck_types::machine::ShortBlockMode::Off;
        for field in [
            ProfileField::MinBlockMs,
            ProfileField::BlockWindow,
            ProfileField::ShortBlockMode,
        ] {
            profile.provenance.remove(&field);
        }
        let z1 = render_profile(&profile).unwrap();
        let old: String = z1
            .replace(&format!("schema = {MACHINE_SCHEMA}"), "schema = 1")
            .lines()
            .filter(|l| !l.starts_with("id = ") && !l.starts_with("max_spindle_rpm"))
            .map(|l| format!("{l}\n"))
            .collect();
        let entry = parse_profile_entry(Path::new("old.toml"), &old).unwrap();
        assert_eq!(entry.id, None);
        assert_eq!(entry.profile.name, "Makera Z1");
        assert_eq!(entry.profile.min_block_ms, 0.0);
        assert_eq!(entry.profile.block_window, 1);
        assert_eq!(
            entry.profile.short_block_mode,
            openchuck_types::machine::ShortBlockMode::Off
        );
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
