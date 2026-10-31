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

use std::path::{Path, PathBuf};

use openchuck_project::ParameterValue;
use openchuck_project::machines::{
    ProfileEntry, builtin_entry, builtin_keys, list_library_entries, load_profile_entry,
    new_machine_id, save_profile_entry,
};
use openchuck_types::machine::MachineProfile;

use crate::machine_edit::{FieldProblem, check_machine, set_machine_field};
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
        })
    }

    /// A draft that has never been saved, of `profile` with `id`.
    pub fn unsaved(profile: MachineProfile, id: Option<String>) -> Self {
        Self {
            path: None,
            id,
            saved: None,
            profile,
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
        self.profile = set_machine_field(&self.profile, name, value)?;
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

/// A name for `name` that no library or built-in profile has, adding
/// " (imported)", then " (imported 2)" and so on.
fn free_import_name(dir: &Path, name: &str) -> String {
    let known = known_profiles(dir);
    let taken = |n: &str| {
        known
            .iter()
            .any(|(_, e)| e.profile.name.trim().eq_ignore_ascii_case(n.trim()))
    };
    let mut candidate = format!("{name} (imported)");
    let mut n = 2;
    while taken(&candidate) {
        candidate = format!("{name} (imported {n})");
        n += 1;
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
                profile.name = free_import_name(dir, &profile.name);
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
            profile.name = free_import_name(dir, &profile.name);
            add_new(new_machine_id(), profile)
        }
    }
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
    fn req_set_01_a_draft_is_written_only_on_save() {
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
    fn req_nfr_07_save_refuses_a_taken_name_and_an_invalid_profile() {
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
    fn req_set_01_delete_removes_only_a_library_profile() {
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
    fn req_set_01_export_then_import_round_trips_and_clashes_are_resolved() {
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
    fn req_nfr_07_a_damaged_file_is_refused_with_its_reason() {
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
