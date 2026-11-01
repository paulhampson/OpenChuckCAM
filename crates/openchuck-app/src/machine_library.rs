// SPDX-License-Identifier: AGPL-3.0-or-later
//! The machine library: editing, saving, deleting, importing and exporting
//! the user's machine profiles (SET-01, UI-13, NFR-07; lane MACHINES,
//! decision record 0380, proposed; issue #335).
//!
//! A library is a folder of profile files (decision 0011 item 7), by default
//! the platform configuration folder's `OpenChuckCAM/machines` (decision
//! 0029 item 15, [`crate::library`]). A library profile is edited as a
//! [`MachineDraft`]: nothing is written until [`save_draft`], as in the tool
//! library (decision 0127). Built-in profiles are never written. Projects
//! keep their own copy (decision 0011 item 7), so nothing here changes a
//! project; a project copy compares with its source by ID (OI-54).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use openchuck_project::ParameterValue;
use openchuck_project::machines::{
    ProfileEntry, builtin_entry, builtin_keys, list_library_entries, load_profile_entry,
    new_machine_id, save_profile_entry,
};
use openchuck_project::tools::ToolLibrary;
use openchuck_types::geometry::{Aabb, Point3};
use openchuck_types::machine::{
    ARange, ASense, Clearance, Confirmation, MachineProfile, MaxFeeds, ProfileField, Provenance,
    RotaryAxis, RotaryInFeed, ShortBlockMode,
};
use openchuck_types::units::{FeedRate, Millimetres, RotaryFeedRate};

use crate::machine_edit::{
    FieldProblem, MachineField, USER_CHANGED, check_machine, fields, machine_form,
    set_machine_field,
};
use crate::setup::differing_fields;

/// Why a library action failed, in plain words.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum MachineLibraryError {
    /// A value of the profile is not allowed.
    #[error("{0}")]
    Invalid(FieldProblem),
    /// Anything else, already in plain words: a file that cannot be read or
    /// written, a name in use, a built-in profile.
    #[error("{0}")]
    Refused(String),
}

impl From<FieldProblem> for MachineLibraryError {
    fn from(p: FieldProblem) -> Self {
        Self::Invalid(p)
    }
}

type Result<T> = std::result::Result<T, MachineLibraryError>;

fn refused(message: impl Into<String>) -> MachineLibraryError {
    MachineLibraryError::Refused(message.into())
}

/// A library profile being edited. `saved` is what its file holds (`None`
/// before its first save); `profile` is the edited profile.
#[derive(Debug, Clone, PartialEq)]
pub struct MachineDraft {
    /// The file it is saved in; `None` until its first save.
    pub path: Option<PathBuf>,
    /// Its ID, which project copies keep (decision 0029 item 16).
    pub id: Option<String>,
    /// The profile as saved.
    pub saved: Option<MachineProfile>,
    /// The profile as edited.
    pub profile: MachineProfile,
    /// Required values of a new machine not filled in yet ([`fields`]
    /// names). They hold placeholders that are never shown or saved: the
    /// form shows them blank and [`save_draft`] refuses while any is left.
    pub blanks: BTreeSet<&'static str>,
}

/// The source of a value a new machine starts with (OI-1472).
pub const NEW_MACHINE_DEFAULT: &str = "Default for a new machine; check it on the machine";

/// The values a new machine must be given before it is saved (the owner's
/// choice on OI-1472, decision 0380 item 9), in form order.
pub const REQUIRED_FOR_NEW: [&str; 18] = [
    fields::NAME,
    fields::ORIGIN_X,
    fields::ORIGIN_Y,
    fields::ORIGIN_Z,
    fields::TRAVEL_MIN_X,
    fields::TRAVEL_MAX_X,
    fields::TRAVEL_MIN_Y,
    fields::TRAVEL_MAX_Y,
    fields::TRAVEL_MIN_Z,
    fields::TRAVEL_MAX_Z,
    fields::FEED_LINEAR,
    fields::FEED_Z,
    fields::FEED_ROTARY,
    fields::RAPID_LINEAR,
    fields::RAPID_Z,
    fields::RAPID_ROTARY,
    fields::CLEARANCE_RADIUS,
    fields::SAFE_Z,
];

/// Placeholders far from any real value, so that filling the required
/// values one at a time never meets a rule between two values that the
/// user has not both given.
const FAR: f64 = 1.0e9;

fn blank_profile() -> MachineProfile {
    MachineProfile {
        name: "New machine".to_owned(),
        rotary_axis: RotaryAxis::X,
        a_sense: ASense::Positive,
        rotary_origin: Point3::new(0.0, 0.0, 0.0),
        a_range: ARange::Continuous,
        travel: Aabb {
            min: Point3::new(-FAR, -FAR, -FAR),
            max: Point3::new(FAR, FAR, FAR),
        },
        max_feeds: MaxFeeds {
            linear: FeedRate(1.0),
            z: FeedRate(1.0),
            rotary: RotaryFeedRate(1.0),
            rapid_linear: FeedRate(1.0),
            rapid_z: FeedRate(1.0),
            rapid_rotary: RotaryFeedRate(1.0),
            rotary_in_feed: RotaryInFeed::Separate,
        },
        clearance: Clearance {
            radius: Millimetres(1.0 / FAR),
            safe_z: Millimetres(FAR),
        },
        max_tool_number: 99,
        min_clearance_radius: None,
        min_spindle_rpm: None,
        max_spindle_rpm: None,
        min_block_ms: 0.0,
        block_window: 1,
        short_block_mode: ShortBlockMode::Off,
        automatic_tool_length: false,
        spindle_to_axis: None,
        recommended_min_safe_z: None,
        provenance: ProfileField::ALL
            .into_iter()
            .map(|f| {
                (
                    f,
                    Provenance {
                        state: Confirmation::Unconfirmed,
                        source: NEW_MACHINE_DEFAULT.to_owned(),
                    },
                )
            })
            .collect(),
    }
}

/// Where a draft stands, shown beside Save and Cancel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DraftStatus {
    /// Never saved.
    New,
    /// Saved before, with changes since.
    Changed,
    /// The same as its file.
    Saved,
}

impl DraftStatus {
    /// The words shown (specification 12.1's wording for tools).
    pub const fn text(self) -> &'static str {
        match self {
            Self::New => "New: not saved yet",
            Self::Changed => "Changed: not saved yet",
            Self::Saved => "Saved in the library",
        }
    }
}

impl MachineDraft {
    /// A draft of the library profile in `path`.
    pub fn open(path: &Path) -> Result<Self> {
        let entry = load_profile_entry(path).map_err(|e| refused(e.to_string()))?;
        Ok(Self {
            path: Some(path.to_path_buf()),
            id: entry.id,
            saved: Some(entry.profile.clone()),
            profile: entry.profile,
            blanks: BTreeSet::new(),
        })
    }

    /// A new machine (New): its required values blank
    /// ([`REQUIRED_FOR_NEW`]), the rest neutral (A continuous and positive,
    /// A separate from the feed word, tool numbers to 99, short moves off,
    /// no spindle range, no tool length measurement), every value not
    /// confirmed. It has no ID until it is saved.
    pub fn new_machine() -> Self {
        Self {
            path: None,
            id: None,
            saved: None,
            profile: blank_profile(),
            blanks: REQUIRED_FOR_NEW.into_iter().collect(),
        }
    }

    /// A copy of `profile` to edit (Duplicate, or Copy to edit on a built-in
    /// profile), named "<name> copy" (then "copy 2" and so on) so its name is
    /// free in the library folder `dir`. It always gets a new ID, so it never
    /// becomes the source that projects made from the original compare with
    /// (the owner's choice on OI-1473, decision 0380 item 10). Its values
    /// keep their provenance until edited.
    pub fn copy_of(dir: &Path, profile: &MachineProfile) -> Self {
        let mut copy = profile.clone();
        copy.name = free_name(dir, &profile.name, "copy");
        Self {
            path: None,
            id: Some(new_machine_id()),
            saved: None,
            profile: copy,
            blanks: BTreeSet::new(),
        }
    }

    /// The form of the draft: [`machine_form`], with the values not filled
    /// in yet shown blank.
    pub fn form(&self) -> Vec<MachineField> {
        let mut form = machine_form(&self.profile);
        for f in &mut form {
            if self.blanks.contains(f.name) {
                f.value = None;
            }
        }
        form
    }

    /// The name the draft had when it was last saved, if it has been
    /// renamed since.
    pub fn renamed_from(&self) -> Option<&str> {
        self.saved
            .as_ref()
            .map(|s| s.name.as_str())
            .filter(|old| old.trim() != self.profile.name.trim())
    }

    /// A draft that has never been saved, of `profile` with `id`.
    pub fn unsaved(profile: MachineProfile, id: Option<String>) -> Self {
        Self {
            path: None,
            id,
            saved: None,
            profile,
            blanks: BTreeSet::new(),
        }
    }

    /// Where the draft stands.
    pub fn status(&self) -> DraftStatus {
        match &self.saved {
            None => DraftStatus::New,
            Some(saved) if *saved == self.profile => DraftStatus::Saved,
            Some(_) => DraftStatus::Changed,
        }
    }

    /// True when leaving the draft would lose changes, so the window asks
    /// "Discard the changes?" first.
    pub fn is_dirty(&self) -> bool {
        self.status() != DraftStatus::Saved
    }

    /// Sets one value of the draft ([`set_machine_field`]). A refused value
    /// leaves the draft as it was.
    pub fn set(
        &mut self,
        name: &str,
        value: Option<&ParameterValue>,
    ) -> std::result::Result<(), FieldProblem> {
        let mut next = set_machine_field(&self.profile, name, value)?;
        if self.blanks.contains(name) {
            // A required value given for the first time is the user's,
            // even when it equals the hidden placeholder.
            if let Some(field) = machine_form(&next)
                .into_iter()
                .find(|f| f.name == name)
                .and_then(|f| f.provenance)
            {
                next.provenance.insert(
                    field,
                    Provenance {
                        state: Confirmation::Unconfirmed,
                        source: USER_CHANGED.to_owned(),
                    },
                );
            }
            self.blanks.remove(name);
        }
        self.profile = next;
        Ok(())
    }
}

/// A file name in `dir` for a profile called `name` that no file has yet:
/// its name in lower-case letters, digits and hyphens, then `-2`, `-3` and
/// so on if that file exists.
pub fn file_for(dir: &Path, name: &str) -> PathBuf {
    let stem = crate::posting::slug(name, "machine");
    let mut path = dir.join(format!("{stem}.toml"));
    let mut n = 2;
    while path.exists() {
        path = dir.join(format!("{stem}-{n}.toml"));
        n += 1;
    }
    path
}

/// The library's profiles and the built-in ones that can be read, with
/// where each is (the file, or `None` for a built-in one).
fn known_profiles(dir: &Path) -> Vec<(Option<PathBuf>, ProfileEntry)> {
    let mut out: Vec<_> = list_library_entries(dir)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(path, entry)| entry.ok().map(|e| (Some(path), e)))
        .collect();
    out.extend(
        builtin_keys()
            .into_iter()
            .filter_map(builtin_entry)
            .filter_map(std::result::Result::ok)
            .map(|e| (None, e)),
    );
    out
}

fn same_file(a: &Path, b: &Path) -> bool {
    a == b
        || matches!(
            (a.canonicalize(), b.canonicalize()),
            (Ok(a), Ok(b)) if a == b
        )
}

/// Refuses a name another profile in the library or built in already has
/// (letter case ignored): the CLI chooses profiles by name and machine tool
/// sets are tied to a machine by name (OI-770).
fn check_name_free(dir: &Path, name: &str, own: Option<&Path>) -> Result<()> {
    let taken = known_profiles(dir).into_iter().find(|(path, e)| {
        e.profile.name.trim().eq_ignore_ascii_case(name.trim())
            && !matches!((path, own), (Some(p), Some(o)) if same_file(p, o))
    });
    match taken {
        None => Ok(()),
        Some((None, _)) => Err(refused(format!(
            "A built-in machine is called \"{name}\"; choose another name."
        ))),
        Some((Some(_), _)) => Err(refused(format!(
            "A machine called \"{name}\" is already in the library; choose another name."
        ))),
    }
}

/// Writes the draft to the library folder `dir` (created if missing): to its
/// own file, or to a new file named after it on its first save. A profile
/// saved from the editor always has an ID, so project copies of it can be
/// compared with it (OI-54). Returns the file.
pub fn save_draft(dir: &Path, draft: &mut MachineDraft) -> Result<PathBuf> {
    if !draft.blanks.is_empty() {
        let form = draft.form();
        let missing: Vec<&str> = form
            .iter()
            .filter(|f| draft.blanks.contains(f.name))
            .map(|f| f.label)
            .collect();
        return Err(refused(format!(
            "Fill in before saving: {}.",
            missing.join(", ")
        )));
    }
    check_machine(&draft.profile)?;
    check_name_free(dir, &draft.profile.name, draft.path.as_deref())?;
    std::fs::create_dir_all(dir).map_err(|e| {
        refused(format!(
            "The library folder {} cannot be created: {e}.",
            dir.display()
        ))
    })?;
    let path = match &draft.path {
        Some(p) => p.clone(),
        None => file_for(dir, &draft.profile.name),
    };
    let id = draft.id.clone().unwrap_or_else(new_machine_id);
    let entry = ProfileEntry {
        id: Some(id.clone()),
        profile: draft.profile.clone(),
    };
    save_profile_entry(&entry, &path).map_err(|e| refused(e.to_string()))?;
    draft.path = Some(path.clone());
    draft.id = Some(id);
    draft.saved = Some(draft.profile.clone());
    Ok(path)
}

/// What deleting a library profile affects, for the question asked first.
pub fn delete_message(name: &str, used_by_project: bool) -> String {
    let mut text = format!("Delete \"{name}\" from the library? This cannot be undone.");
    if used_by_project {
        text.push_str(
            " This project keeps its own copy of the machine; it will then say the profile \
             it came from is not in the library.",
        );
    } else {
        text.push_str(" Projects keep their own copy of the machine and are not changed.");
    }
    text
}

/// Deletes the library profile file `path`, which must be a profile file
/// directly in the library folder `dir`. Built-in profiles have no file and
/// cannot be deleted.
pub fn delete_profile(dir: &Path, path: &Path) -> Result<()> {
    let inside = path.parent().is_some_and(|parent| same_file(parent, dir));
    if !inside || path.extension().and_then(|e| e.to_str()) != Some("toml") {
        return Err(refused(format!(
            "{} is not a profile in the library folder {}.",
            path.display(),
            dir.display()
        )));
    }
    load_profile_entry(path).map_err(|e| {
        refused(format!(
            "{} is not deleted because it cannot be read as a machine profile: {e}",
            path.display()
        ))
    })?;
    std::fs::remove_file(path)
        .map_err(|e| refused(format!("{} cannot be deleted: {e}.", path.display())))
}

/// Writes a profile with its ID to the file `path`, for sharing (any
/// profile: built-in, library or the project's copy).
pub fn export_profile(profile: &MachineProfile, id: Option<&str>, path: &Path) -> Result<()> {
    check_machine(profile)?;
    let entry = ProfileEntry {
        id: id.map(str::to_owned),
        profile: profile.clone(),
    };
    save_profile_entry(&entry, path).map_err(|e| refused(e.to_string()))
}

/// How an imported profile compares with what is already known.
#[derive(Debug, Clone, PartialEq)]
pub enum ImportStatus {
    /// No library or built-in profile has its ID.
    New,
    /// A profile with its ID is the same in every value.
    Same {
        /// The library file, or `None` for a built-in profile.
        path: Option<PathBuf>,
    },
    /// A profile with its ID differs.
    Differs {
        /// The library file, or `None` for a built-in profile.
        path: Option<PathBuf>,
        /// The values that differ, in the user's words.
        fields: Vec<&'static str>,
    },
}

/// A profile file read for import, and what importing it would do. Nothing
/// is written until [`apply_import`].
#[derive(Debug, Clone, PartialEq)]
pub struct ImportPlan {
    /// The profile read, with its ID if the file has one.
    pub entry: ProfileEntry,
    /// How it compares with the library and the built-in profiles.
    pub status: ImportStatus,
}

/// What to do with an imported profile that has the ID of one already known.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportChoice {
    /// Keep the library's profile and import nothing (chosen at first).
    KeepMine,
    /// Write the imported values over the library profile, keeping its file.
    Replace,
    /// Add the import as a separate machine, with a new ID and "(imported)"
    /// after its name.
    KeepBoth,
}

/// Reads the profile file `file` for import into the library folder `dir`.
/// A file that cannot be read gives its reason; nothing panics (NFR-07).
pub fn plan_import(dir: &Path, file: &Path) -> Result<ImportPlan> {
    let entry = load_profile_entry(file).map_err(|e| {
        refused(format!(
            "{} cannot be imported as a machine profile: {e}",
            file.display()
        ))
    })?;
    check_machine(&entry.profile)?;
    let status = match entry.id.as_deref().and_then(|id| {
        known_profiles(dir)
            .into_iter()
            .find(|(_, known)| known.id.as_deref() == Some(id))
    }) {
        None => ImportStatus::New,
        Some((path, known)) => {
            let fields = differing_fields(&known.profile, &entry.profile);
            if fields.is_empty() && known.profile == entry.profile {
                ImportStatus::Same { path }
            } else {
                ImportStatus::Differs { path, fields }
            }
        }
    };
    Ok(ImportPlan { entry, status })
}

/// A name for `name` that no library or built-in profile has: for the word
/// "imported", " (imported)", then " (imported 2)" and so on; for "copy",
/// " copy", then " copy 2" and so on.
fn free_name(dir: &Path, name: &str, word: &str) -> String {
    let known = known_profiles(dir);
    let taken = |n: &str| {
        known
            .iter()
            .any(|(_, e)| e.profile.name.trim().eq_ignore_ascii_case(n.trim()))
    };
    let name = name.trim();
    let make = |n: u32| match (word, n) {
        ("copy", 1) => format!("{name} copy"),
        ("copy", n) => format!("{name} copy {n}"),
        (w, 1) => format!("{name} ({w})"),
        (w, n) => format!("{name} ({w} {n})"),
    };
    let mut n = 1;
    let mut candidate = make(n);
    while taken(&candidate) {
        n += 1;
        candidate = make(n);
    }
    candidate
}

/// Carries out `plan` with `choice` (used only when a profile with its ID is
/// known). Returns the file written, or `None` when nothing was imported.
pub fn apply_import(
    dir: &Path,
    plan: &ImportPlan,
    choice: ImportChoice,
) -> Result<Option<PathBuf>> {
    let add_new = |id: String, profile: MachineProfile| -> Result<Option<PathBuf>> {
        let mut draft = MachineDraft::unsaved(profile, Some(id));
        save_draft(dir, &mut draft).map(Some)
    };
    match (&plan.status, choice) {
        (ImportStatus::New, _) => {
            let mut profile = plan.entry.profile.clone();
            if known_profiles(dir).iter().any(|(_, e)| {
                e.profile
                    .name
                    .trim()
                    .eq_ignore_ascii_case(profile.name.trim())
            }) {
                profile.name = free_name(dir, &profile.name, "imported");
            }
            add_new(
                plan.entry.id.clone().unwrap_or_else(new_machine_id),
                profile,
            )
        }
        (ImportStatus::Same { .. }, _) | (ImportStatus::Differs { .. }, ImportChoice::KeepMine) => {
            Ok(None)
        }
        (ImportStatus::Differs { path: None, .. }, ImportChoice::Replace) => Err(refused(
            "A built-in machine cannot be replaced; keep both to add the import as another \
             machine.",
        )),
        (
            ImportStatus::Differs {
                path: Some(path), ..
            },
            ImportChoice::Replace,
        ) => {
            let mut draft = MachineDraft::open(path)?;
            draft.profile = plan.entry.profile.clone();
            save_draft(dir, &mut draft).map(Some)
        }
        (ImportStatus::Differs { .. }, ImportChoice::KeepBoth) => {
            let mut profile = plan.entry.profile.clone();
            profile.name = free_name(dir, &profile.name, "imported");
            add_new(new_machine_id(), profile)
        }
    }
}

/// The values in which the project's machine `copy` differs from the
/// library profile in `path`, in the user's words, for the confirmation
/// before "Save to library" writes the copy over it.
pub fn copy_changes(path: &Path, copy: &MachineProfile) -> Result<Vec<&'static str>> {
    let entry = load_profile_entry(path).map_err(|e| refused(e.to_string()))?;
    Ok(differing_fields(&entry.profile, copy))
}

/// "Save to library" for the project's machine `copy` onto the library
/// profile it came from, the file `path`: the file takes the copy's values
/// and keeps its ID (decision 0380 item 10). Its name must still be free.
pub fn save_copy_to_source(dir: &Path, path: &Path, copy: &MachineProfile) -> Result<PathBuf> {
    let mut draft = MachineDraft::open(path)?;
    draft.profile = copy.clone();
    save_draft(dir, &mut draft)
}

/// "Save to library as a new profile" for the project's machine `copy`: a
/// new library profile with a new ID (OI-1473), named after the copy, or
/// "<name> copy" when that name is taken. Returns the file and the ID, which
/// the project's copy then records as its source.
pub fn save_copy_as_new(dir: &Path, copy: &MachineProfile) -> Result<(PathBuf, String)> {
    let mut profile = copy.clone();
    if check_name_free(dir, &profile.name, None).is_err() {
        profile.name = free_name(dir, &copy.name, "copy");
    }
    let id = new_machine_id();
    let mut draft = MachineDraft::unsaved(profile, Some(id.clone()));
    let path = save_draft(dir, &mut draft)?;
    Ok((path, id))
}

/// The question to ask before saving a renamed draft when a machine tool
/// set (TL-11) is tied to its old name (the owner's choice on OI-1474,
/// decision 0380 item 11): the window offers "Also rename the tool set",
/// ticked at first, in the same confirmation. `None` when no set names the
/// old name or the draft is not renamed.
pub fn tool_set_rename_question(tools: &ToolLibrary, draft: &MachineDraft) -> Option<String> {
    let old = draft.renamed_from()?;
    tools.tool_set(old)?;
    Some(format!(
        "The tool set for \"{}\" holds this machine's tool numbers. Also rename the tool set \
         to \"{}\"?",
        old.trim(),
        draft.profile.name.trim()
    ))
}

/// Renames the tool set tied to `old` so it follows the machine to `new`,
/// after the machine profile is saved. Returns the tool set's file, or
/// `None` when no set names `old`.
pub fn rename_tool_set(tools: &mut ToolLibrary, old: &str, new: &str) -> Result<Option<PathBuf>> {
    tools.rename_tool_set(old, new).map_err(|e| {
        refused(format!(
            "The machine is saved, but its tool set was not renamed: {e}."
        ))
    })
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;
    use crate::machine_edit::fields;
    use openchuck_project::machines::builtin;

    const Z1_ID: &str = "8f0c2a4e-1b7d-4e3a-9c61-5d2e7f4a1b01";

    fn z1() -> MachineProfile {
        builtin("makera-z1").expect("built in").expect("reads")
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "openchuck-machine-library-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn renamed(name: &str) -> MachineProfile {
        let mut p = z1();
        p.name = name.to_owned();
        p
    }

    #[test]
    fn req_set_10_a_draft_is_written_only_on_save() {
        let dir = scratch("save");
        let mut draft = MachineDraft::unsaved(renamed("Shop Z1"), None);
        assert_eq!(draft.status(), DraftStatus::New);
        assert_eq!(draft.status().text(), "New: not saved yet");
        assert!(draft.is_dirty());
        assert!(!dir.exists());
        let path = save_draft(&dir, &mut draft).expect("saves");
        assert_eq!(path, dir.join("shop-z1.toml"));
        assert_eq!(draft.status(), DraftStatus::Saved);
        assert_eq!(draft.status().text(), "Saved in the library");
        let id = draft.id.clone().expect("given an ID on save");
        assert_ne!(id, Z1_ID);

        // Editing changes the draft, not the file, until it is saved.
        draft
            .set(fields::SAFE_Z, Some(&ParameterValue::Number(50.0)))
            .expect("valid");
        assert_eq!(draft.status(), DraftStatus::Changed);
        assert_eq!(draft.status().text(), "Changed: not saved yet");
        assert_eq!(
            MachineDraft::open(&path)
                .expect("reads")
                .profile
                .clearance
                .safe_z
                .0,
            40.0
        );
        // A refused value leaves the draft as it was.
        let before = draft.clone();
        assert!(
            draft
                .set(fields::FEED_Z, Some(&ParameterValue::Number(0.0)))
                .is_err()
        );
        assert_eq!(draft, before);
        save_draft(&dir, &mut draft).expect("saves");
        let again = MachineDraft::open(&path).expect("reads");
        assert_eq!(again.profile, draft.profile);
        assert_eq!(again.id.as_deref(), Some(id.as_str()));
        assert_eq!(again.status(), DraftStatus::Saved);

        // A second profile with a name whose file exists gets its own file.
        let mut other = MachineDraft::unsaved(renamed("Shop-Z1!"), None);
        assert_eq!(
            save_draft(&dir, &mut other).expect("saves"),
            dir.join("shop-z1-2.toml")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn req_set_10_req_nfr_07_save_refuses_a_taken_name_and_an_invalid_profile() {
        let dir = scratch("names");
        let mut draft = MachineDraft::unsaved(renamed("Makera Z1"), None);
        assert_eq!(
            save_draft(&dir, &mut draft)
                .expect_err("built-in name")
                .to_string(),
            "A built-in machine is called \"Makera Z1\"; choose another name."
        );
        let mut first = MachineDraft::unsaved(renamed("Shop"), None);
        save_draft(&dir, &mut first).expect("saves");
        let mut second = MachineDraft::unsaved(renamed("  shop "), None);
        assert_eq!(
            save_draft(&dir, &mut second)
                .expect_err("taken")
                .to_string(),
            "A machine called \"  shop \" is already in the library; choose another name."
        );
        // Saving a profile again under its own name is fine.
        save_draft(&dir, &mut first).expect("its own name");
        let mut bad = MachineDraft::unsaved(renamed("Bad"), None);
        bad.profile.clearance.safe_z.0 = 10.0;
        assert_eq!(
            save_draft(&dir, &mut bad).expect_err("invalid").to_string(),
            "Safe Z: The safe Z (10 mm) must not be below the safe cylinder radius (40 mm)."
        );
        assert!(!dir.join("bad.toml").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn req_set_10_delete_removes_only_a_library_profile() {
        let dir = scratch("delete");
        let mut draft = MachineDraft::unsaved(renamed("Old"), None);
        let path = save_draft(&dir, &mut draft).expect("saves");
        let outside = scratch("delete-outside");
        std::fs::create_dir_all(&outside).expect("folder");
        let elsewhere = outside.join("old.toml");
        std::fs::copy(&path, &elsewhere).expect("copy");
        assert!(delete_profile(&dir, &elsewhere).is_err());
        assert!(elsewhere.exists());
        let junk = dir.join("notes.toml");
        std::fs::write(&junk, "not a profile").expect("write");
        assert!(
            delete_profile(&dir, &junk)
                .expect_err("unreadable")
                .to_string()
                .contains("cannot be read as a machine profile")
        );
        assert!(junk.exists());
        delete_profile(&dir, &path).expect("deletes");
        assert!(!path.exists());
        assert!(
            delete_message("Old", true)
                .contains("it will then say the profile it came from is not in the library")
        );
        assert!(delete_message("Old", false).contains("are not changed"));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&outside);
    }

    #[test]
    fn req_set_10_export_then_import_round_trips_and_clashes_are_resolved() {
        let dir = scratch("import");
        let out = scratch("import-out");
        std::fs::create_dir_all(&out).expect("folder");

        // An exported built-in profile has the built-in's ID: importing it
        // unchanged is "the same", and changed is "differs".
        let file = out.join("z1.toml");
        export_profile(&z1(), Some(Z1_ID), &file).expect("exports");
        let plan = plan_import(&dir, &file).expect("reads");
        assert_eq!(plan.status, ImportStatus::Same { path: None });
        assert_eq!(apply_import(&dir, &plan, ImportChoice::KeepBoth), Ok(None));
        let mut changed = z1();
        changed.max_feeds.z.0 = 500.0;
        export_profile(&changed, Some(Z1_ID), &file).expect("exports");
        let plan = plan_import(&dir, &file).expect("reads");
        assert_eq!(
            plan.status,
            ImportStatus::Differs {
                path: None,
                fields: vec!["maximum feeds"]
            }
        );
        assert!(apply_import(&dir, &plan, ImportChoice::Replace).is_err());
        assert_eq!(apply_import(&dir, &plan, ImportChoice::KeepMine), Ok(None));
        let both = apply_import(&dir, &plan, ImportChoice::KeepBoth)
            .expect("imports")
            .expect("a file");
        let added = MachineDraft::open(&both).expect("reads");
        assert_eq!(added.profile.name, "Makera Z1 (imported)");
        assert_ne!(added.id.as_deref(), Some(Z1_ID));
        // Keeping both again gives the next free name and another ID.
        let again = apply_import(&dir, &plan, ImportChoice::KeepBoth)
            .expect("imports")
            .expect("a file");
        let second = MachineDraft::open(&again).expect("reads");
        assert_eq!(second.profile.name, "Makera Z1 (imported 2)");
        assert_ne!(second.id, added.id);

        // A library profile: Replace writes over its own file.
        let mut mine = MachineDraft::unsaved(renamed("Shop"), None);
        let mine_path = save_draft(&dir, &mut mine).expect("saves");
        let mut theirs = mine.profile.clone();
        theirs.clearance.safe_z.0 = 60.0;
        let shared = out.join("shop.toml");
        export_profile(&theirs, mine.id.as_deref(), &shared).expect("exports");
        let plan = plan_import(&dir, &shared).expect("reads");
        assert_eq!(
            plan.status,
            ImportStatus::Differs {
                path: Some(mine_path.clone()),
                fields: vec!["clearance"]
            }
        );
        assert_eq!(
            apply_import(&dir, &plan, ImportChoice::Replace),
            Ok(Some(mine_path.clone()))
        );
        let replaced = MachineDraft::open(&mine_path).expect("reads");
        assert_eq!(replaced.profile.clearance.safe_z.0, 60.0);
        assert_eq!(replaced.id, mine.id);

        // A file with no ID is new, given an ID, and renamed if its name is
        // taken.
        let plain = out.join("plain.toml");
        export_profile(&renamed("Shop"), None, &plain).expect("exports");
        let plan = plan_import(&dir, &plain).expect("reads");
        assert_eq!(plan.status, ImportStatus::New);
        let path = apply_import(&dir, &plan, ImportChoice::KeepMine)
            .expect("imports")
            .expect("a file");
        let imported = MachineDraft::open(&path).expect("reads");
        assert_eq!(imported.profile.name, "Shop (imported)");
        assert!(imported.id.is_some());
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&out);
    }

    #[test]
    fn req_set_10_a_new_machine_starts_blank_and_saves_only_when_filled() {
        let dir = scratch("new");
        let mut draft = MachineDraft::new_machine();
        assert_eq!(draft.status(), DraftStatus::New);
        assert_eq!(draft.id, None);
        // The required values are blank in the form; the rest are neutral.
        let form = draft.form();
        for name in REQUIRED_FOR_NEW {
            let f = form.iter().find(|f| f.name == name).expect(name);
            assert_eq!(f.value, None, "{name}");
        }
        let value =
            |d: &MachineDraft, n: &str| d.form().into_iter().find(|f| f.name == n).expect(n).value;
        assert_eq!(
            value(&draft, fields::A_RANGE),
            Some(ParameterValue::Text("continuous".into()))
        );
        assert_eq!(
            value(&draft, fields::SHORT_BLOCK_MODE),
            Some(ParameterValue::Text("off".into()))
        );
        assert_eq!(
            value(&draft, fields::MAX_TOOL_NUMBER),
            Some(ParameterValue::Integer(99))
        );
        assert_eq!(value(&draft, fields::MAX_SPINDLE_RPM), None);
        assert!(
            ProfileField::ALL
                .iter()
                .all(|f| !draft.profile.is_confirmed(*f))
        );
        assert_eq!(
            draft.profile.provenance[&ProfileField::ARange].source,
            NEW_MACHINE_DEFAULT
        );

        // Save names every value still blank.
        let e = save_draft(&dir, &mut draft).expect_err("blank");
        assert_eq!(
            e.to_string(),
            "Fill in before saving: Name, Rotary reference X, Rotary reference Y, \
             Axis height (machine Z), Lowest X, Highest X, Lowest Y, Highest Y, Lowest Z, \
             Highest Z, X and Y feed, Z feed, A feed, X and Y rapid, Z rapid, A rapid, \
             Safe cylinder radius, Safe Z."
        );
        assert!(!dir.exists());

        // Filling them one at a time meets only the rules between values
        // the user has given.
        let n = |v: f64| ParameterValue::Number(v);
        let values = [
            (fields::NAME, ParameterValue::Text("Router".into())),
            (fields::ORIGIN_X, n(0.0)),
            (fields::ORIGIN_Y, n(-50.0)),
            (fields::ORIGIN_Z, n(-60.0)),
            (fields::TRAVEL_MAX_X, n(300.0)),
            (fields::TRAVEL_MIN_X, n(0.0)),
            (fields::TRAVEL_MIN_Y, n(-200.0)),
            (fields::TRAVEL_MAX_Y, n(0.0)),
            (fields::TRAVEL_MIN_Z, n(-120.0)),
            (fields::TRAVEL_MAX_Z, n(0.0)),
            (fields::FEED_LINEAR, n(2000.0)),
            (fields::FEED_Z, n(1.0)),
            (fields::FEED_ROTARY, n(3600.0)),
            (fields::RAPID_LINEAR, n(4000.0)),
            (fields::RAPID_Z, n(1000.0)),
            (fields::RAPID_ROTARY, n(7200.0)),
            (fields::SAFE_Z, n(60.0)),
        ];
        for (name, v) in &values {
            draft
                .set(name, Some(v))
                .unwrap_or_else(|e| panic!("{name}: {e}"));
        }
        assert_eq!(
            save_draft(&dir, &mut draft)
                .expect_err("one left")
                .to_string(),
            "Fill in before saving: Safe cylinder radius."
        );
        // A value the user gives is theirs, even if it equals the hidden
        // placeholder (the Z feed of 1 above).
        assert_eq!(
            draft.profile.provenance[&ProfileField::MaxFeeds].source,
            USER_CHANGED
        );
        // A rule between two given values still applies.
        assert!(draft.set(fields::CLEARANCE_RADIUS, Some(&n(70.0))).is_err());
        assert!(draft.blanks.contains(fields::CLEARANCE_RADIUS));
        draft
            .set(fields::CLEARANCE_RADIUS, Some(&n(50.0)))
            .expect("valid");
        let path = save_draft(&dir, &mut draft).expect("saves");
        let saved = MachineDraft::open(&path).expect("reads");
        assert_eq!(saved.profile.name, "Router");
        assert_eq!(saved.profile.travel.max.x, 300.0);
        assert!(saved.id.is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn req_set_10_a_copy_has_a_free_name_and_a_new_id() {
        let dir = scratch("copy");
        let z1 = z1();
        let mut copy = MachineDraft::copy_of(&dir, &z1);
        assert_eq!(copy.profile.name, "Makera Z1 copy");
        assert_eq!(copy.status(), DraftStatus::New);
        let first_id = copy.id.clone().expect("an ID");
        assert_ne!(first_id, Z1_ID);
        // The copy keeps the values and their provenance until edited.
        assert_eq!(copy.profile.provenance, z1.provenance);
        assert_eq!(copy.profile.rotary_origin, z1.rotary_origin);
        let path = save_draft(&dir, &mut copy).expect("saves");
        assert_eq!(
            MachineDraft::open(&path).expect("reads").id,
            Some(first_id.clone())
        );
        let again = MachineDraft::copy_of(&dir, &z1);
        assert_eq!(again.profile.name, "Makera Z1 copy 2");
        assert_ne!(again.id, Some(first_id));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn req_set_10_the_project_copy_saves_to_its_source_or_as_a_new_profile() {
        let dir = scratch("copy-save");
        let mut mine = MachineDraft::unsaved(renamed("Shop"), None);
        let path = save_draft(&dir, &mut mine).expect("saves");
        let id = mine.id.clone().expect("an ID");

        // The project's copy, edited in the project.
        let copy = crate::machine_edit::set_machine_field(
            &mine.profile,
            fields::SAFE_Z,
            Some(&ParameterValue::Number(65.0)),
        )
        .expect("valid");
        assert_eq!(
            copy_changes(&path, &copy),
            Ok(vec!["clearance", "which values are confirmed"])
        );
        assert_eq!(save_copy_to_source(&dir, &path, &copy), Ok(path.clone()));
        let written = MachineDraft::open(&path).expect("reads");
        assert_eq!(written.profile, copy);
        assert_eq!(written.id.as_deref(), Some(id.as_str()));
        assert_eq!(copy_changes(&path, &copy), Ok(vec![]));

        // As a new profile: a new ID, and a free name.
        let (new_path, new_id) = save_copy_as_new(&dir, &copy).expect("saves");
        assert_ne!(new_id, id);
        let added = MachineDraft::open(&new_path).expect("reads");
        assert_eq!(added.profile.name, "Shop copy");
        assert_eq!(added.id, Some(new_id));
        let mut free = copy.clone();
        free.name = "Annex".into();
        let (annex, _) = save_copy_as_new(&dir, &free).expect("saves");
        assert_eq!(
            MachineDraft::open(&annex).expect("reads").profile.name,
            "Annex"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn req_set_10_req_tl_11_renaming_a_machine_offers_to_rename_its_tool_set() {
        let dir = scratch("rename");
        let tools_dir = scratch("rename-tools");
        let mut draft = MachineDraft::unsaved(renamed("My Z1"), None);
        save_draft(&dir, &mut draft).expect("saves");
        let mut tools = ToolLibrary::open(&tools_dir).expect("opens");
        tools
            .save_tool_set(openchuck_project::tools::ToolSet {
                machine: "My Z1".into(),
                entries: Vec::new(),
            })
            .expect("saves");
        assert_eq!(tool_set_rename_question(&tools, &draft), None);
        draft
            .set(
                fields::NAME,
                Some(&ParameterValue::Text("Z1 workshop".into())),
            )
            .expect("valid");
        assert_eq!(draft.renamed_from(), Some("My Z1"));
        assert_eq!(
            tool_set_rename_question(&tools, &draft).as_deref(),
            Some(
                "The tool set for \"My Z1\" holds this machine's tool numbers. Also rename the \
                 tool set to \"Z1 workshop\"?"
            )
        );
        save_draft(&dir, &mut draft).expect("saves");
        assert!(
            rename_tool_set(&mut tools, "My Z1", "Z1 workshop")
                .expect("renames")
                .is_some()
        );
        assert!(tools.tool_set("Z1 workshop").is_some());
        assert_eq!(rename_tool_set(&mut tools, "Gone", "Other"), Ok(None));
        // A machine whose tool set is not tied to it asks nothing.
        let mut other = MachineDraft::unsaved(renamed("Lathe"), None);
        save_draft(&dir, &mut other).expect("saves");
        other
            .set(fields::NAME, Some(&ParameterValue::Text("Lathe 2".into())))
            .expect("valid");
        assert_eq!(tool_set_rename_question(&tools, &other), None);
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&tools_dir);
    }

    #[test]
    fn req_set_10_req_nfr_07_a_damaged_file_is_refused_with_its_reason() {
        let dir = scratch("damaged");
        let out = scratch("damaged-out");
        std::fs::create_dir_all(&out).expect("folder");
        for (name, text) in [
            ("empty.toml", ""),
            ("syntax.toml", "schema = 7\n[machine\n"),
            ("newer.toml", "schema = 99\n"),
        ] {
            let file = out.join(name);
            std::fs::write(&file, text).expect("write");
            let e = plan_import(&dir, &file).expect_err(name).to_string();
            assert!(
                e.contains("cannot be imported as a machine profile"),
                "{name}: {e}"
            );
        }
        assert!(plan_import(&dir, &out.join("missing.toml")).is_err());
        assert!(!dir.exists(), "a refused import writes nothing");
        let _ = std::fs::remove_dir_all(&out);
    }
}
