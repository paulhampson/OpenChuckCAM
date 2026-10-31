// SPDX-License-Identifier: AGPL-3.0-or-later
//! The machine library window (SET-10, UI-13, UI-08, NFR-07; specification
//! 12.5; lane MACHINES, decision record 0380; issue #335), driven as the
//! user does and drawn whole without a window.

#![allow(clippy::unwrap_used, clippy::float_cmp)]

use std::path::PathBuf;

use openchuck_app::machine_edit::fields;
use openchuck_app::machine_library::{ImportChoice, ImportStatus, MachineDraft};
use openchuck_app::project::ParameterValue;
use openchuck_app::setup::{MachineProfile, ProfileField};
use openchuck_app::tools::{ToolLibrary, ToolSet};

use super::frame_tests::Rig;
use super::*;
use crate::machine_library_window::{MachineLibraryWindow, Selected};

const Z1_ID: &str = "8f0c2a4e-1b7d-4e3a-9c61-5d2e7f4a1b01";

fn folder(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "openchuck-machine-library-window-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn z1() -> MachineProfile {
    openchuck_app::setup::builtin("makera-z1").unwrap().unwrap()
}

fn num(v: f64) -> Option<ParameterValue> {
    Some(ParameterValue::Number(v))
}

fn text(t: &str) -> Option<ParameterValue> {
    Some(ParameterValue::Text(t.to_owned()))
}

/// The cube project with the built-in Makera Z1 as its machine, recorded
/// with the built-in profile's ID.
fn rig_with_z1(name: &str) -> Rig {
    let mut rig = Rig::cube(name, None);
    rig.shell
        .app
        .as_mut()
        .unwrap()
        .execute(Command::SetMachineFrom {
            machine: Some(Box::new(z1())),
            id: Some(Z1_ID.to_owned()),
        })
        .unwrap();
    rig
}

fn copy(rig: &Rig) -> (MachineProfile, Option<String>) {
    let project = rig.shell.app.as_ref().unwrap().project();
    (
        project.machine().unwrap().clone(),
        project.machine_id().map(str::to_owned),
    )
}

#[test]
fn req_set_10_a_built_in_machine_is_copied_to_edit_and_saved_as_a_new_one() {
    let dir = folder("copy");
    let mut window = MachineLibraryWindow::new(Some(dir.clone()), Some(folder("copy-tools")));
    window.open(false);
    window.click(Selected::BuiltIn("built-in makera-z1".into()));
    assert!(
        window.draft().is_none(),
        "a built-in profile is not a draft"
    );
    // A built-in value cannot be set.
    assert_eq!(window.set_value(None, fields::SAFE_Z, num(55.0)), None);
    assert_eq!(z1().clearance.safe_z.0, 40.0);
    window.duplicate(None);
    let draft = window.draft().unwrap();
    assert_eq!(draft.profile.name, "Makera Z1 copy");
    assert_ne!(draft.id.as_deref(), Some(Z1_ID));
    assert_eq!(window.selected(), Some(&Selected::Library(None)));
    assert_eq!(window.set_value(None, fields::SAFE_Z, num(55.0)), None);
    assert_eq!(window.save(), None);
    let saved = window
        .listed()
        .iter()
        .find(|a| a.profile.as_ref().is_ok_and(|p| p.name == "Makera Z1 copy"))
        .unwrap();
    assert_eq!(saved.profile.as_ref().unwrap().clearance.safe_z.0, 55.0);
    assert!(matches!(
        window.selected(),
        Some(Selected::Library(Some(_)))
    ));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn req_set_10_req_nfr_07_a_new_machine_names_what_is_missing_and_bad_values_in_plain_words() {
    let dir = folder("new");
    let mut window = MachineLibraryWindow::new(Some(dir.clone()), Some(folder("new-tools")));
    window.open(false);
    window.new_machine();
    assert_eq!(window.save(), None);
    let (message, problem) = window.message().unwrap().clone();
    assert!(problem);
    assert!(
        message.starts_with("Fill in before saving: Name, Rotary reference X"),
        "{message}"
    );
    assert!(!dir.exists(), "nothing is written");
    // A bad value is refused in plain words and the draft is unchanged.
    let before = window.draft().unwrap().clone();
    assert_eq!(
        window.set_value(None, fields::FEED_Z, num(-1.0)).as_deref(),
        Some("Must be more than 0 mm/min; -1 mm/min was entered.")
    );
    assert_eq!(window.draft().unwrap(), &before);
    // Leaving the new draft asks first; keeping it stays.
    window.click(Selected::BuiltIn("built-in makera-z1".into()));
    assert!(window.asks_to_discard());
    window.answer_discard(false);
    assert_eq!(window.selected(), Some(&Selected::Library(None)));
    window.click(Selected::BuiltIn("built-in makera-z1".into()));
    window.answer_discard(true);
    assert_eq!(
        window.selected(),
        Some(&Selected::BuiltIn("built-in makera-z1".into()))
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn req_set_10_req_ui_08_the_projects_copy_is_edited_by_undoable_commands() {
    let mut rig = rig_with_z1("machines-project-edit");
    let mut window = MachineLibraryWindow::new(Some(folder("project-edit")), None);
    window.open(true);
    assert_eq!(window.selected(), Some(&Selected::Project));
    window.set_value(rig.shell.app.as_mut(), fields::FEED_Z, num(500.0));
    let (edited, id) = copy(&rig);
    assert_eq!(edited.max_feeds.z.0, 500.0);
    assert!(!edited.is_confirmed(ProfileField::MaxFeeds));
    assert_eq!(id.as_deref(), Some(Z1_ID), "the copy keeps its source");
    // A refused value leaves the project as it was.
    assert!(
        window
            .set_value(rig.shell.app.as_mut(), fields::SAFE_Z, num(10.0))
            .unwrap()
            .contains("must not be below the safe cylinder radius")
    );
    assert_eq!(copy(&rig).0, edited);
    // Confirmed again only with how; one more undoable step.
    window.mark_confirmed(
        rig.shell.app.as_mut(),
        ProfileField::MaxFeeds,
        "Jogged Z at F500 on 31 Oct",
    );
    assert!(copy(&rig).0.is_confirmed(ProfileField::MaxFeeds));
    rig.shell.undo();
    assert!(!copy(&rig).0.is_confirmed(ProfileField::MaxFeeds));
    rig.shell.undo();
    assert_eq!(copy(&rig).0, z1());
}

#[test]
fn req_set_10_the_projects_copy_saves_to_the_library_with_a_new_id_or_onto_its_source() {
    let dir = folder("save-copy");
    let mut rig = rig_with_z1("machines-save-copy");
    let mut window = MachineLibraryWindow::new(Some(dir.clone()), None);
    window.open(true);
    window.set_value(rig.shell.app.as_mut(), fields::SAFE_Z, num(50.0));
    // The source is built in, so it can only be saved as a new profile.
    assert_eq!(window.save_copy(rig.shell.app.as_mut(), false), None);
    let (saved, id) = copy(&rig);
    let id = id.unwrap();
    assert_ne!(id, Z1_ID, "the copy now records its new source");
    let file = window
        .listed()
        .iter()
        .find(|a| a.id.as_deref() == Some(id.as_str()))
        .map(|a| PathBuf::from(&a.source))
        .unwrap();
    assert_eq!(MachineDraft::open(&file).unwrap().profile, saved);
    assert_eq!(saved.name, "Makera Z1 copy");
    // A later edit is written onto that source, keeping its ID.
    window.set_value(rig.shell.app.as_mut(), fields::SAFE_Z, num(60.0));
    assert_eq!(
        window.save_copy(rig.shell.app.as_mut(), true),
        Some(vec!["clearance"])
    );
    let written = MachineDraft::open(&file).unwrap();
    assert_eq!(written.profile.clearance.safe_z.0, 60.0);
    assert_eq!(written.id.as_deref(), Some(id.as_str()));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn req_set_10_req_tl_11_renaming_a_machine_can_rename_its_tool_set() {
    let dir = folder("rename");
    let tools_dir = folder("rename-tools");
    let mut window = MachineLibraryWindow::new(Some(dir.clone()), Some(tools_dir.clone()));
    window.open(false);
    window.click(Selected::BuiltIn("built-in makera-z1".into()));
    window.duplicate(None);
    window.set_value(None, fields::NAME, text("My Z1"));
    assert_eq!(window.save(), None);
    ToolLibrary::open(&tools_dir)
        .unwrap()
        .save_tool_set(ToolSet {
            machine: "My Z1".into(),
            entries: Vec::new(),
        })
        .unwrap();
    window.set_value(None, fields::NAME, text("Z1 workshop"));
    let question = window.save().unwrap();
    assert!(
        question.contains("Also rename the tool set to \"Z1 workshop\"?"),
        "{question}"
    );
    window.answer_rename(true);
    let tools = ToolLibrary::open(&tools_dir).unwrap();
    assert!(tools.tool_set("Z1 workshop").is_some());
    assert!(tools.tool_set("My Z1").is_none());
    // Not ticked: the machine is renamed, the tool set is not.
    window.set_value(None, fields::NAME, text("Z1 bench"));
    assert!(window.save().is_some());
    window.answer_rename(false);
    assert_eq!(window.draft().unwrap().profile.name, "Z1 bench");
    assert!(
        ToolLibrary::open(&tools_dir)
            .unwrap()
            .tool_set("Z1 workshop")
            .is_some()
    );
    std::fs::remove_dir_all(&dir).ok();
    std::fs::remove_dir_all(&tools_dir).ok();
}

#[test]
fn req_set_10_delete_asks_first_and_export_then_import_round_trips() {
    let dir = folder("files");
    let out = folder("files-out");
    std::fs::create_dir_all(&out).unwrap();
    let mut rig = rig_with_z1("machines-files");
    let mut window = MachineLibraryWindow::new(Some(dir.clone()), None);
    window.open(true);
    let file = out.join("project-machine.toml");
    window.export(rig.shell.app.as_mut(), &file);
    assert!(file.exists());
    // The project's copy has the built-in's ID and its values: the same.
    assert_eq!(
        window.import(&file, ImportChoice::KeepMine),
        Some(ImportStatus::Same { path: None })
    );
    // Duplicated and saved, then deleted after the question.
    window.duplicate(rig.shell.app.as_mut());
    assert_eq!(window.save(), None);
    let question = window.delete(None, false).unwrap();
    assert!(
        question.starts_with("Delete \"Makera Z1 copy\" from the library?"),
        "{question}"
    );
    assert_eq!(window.listed().len(), 2, "kept when the answer is Cancel");
    window.delete(None, true);
    assert_eq!(
        window.listed().len(),
        1,
        "only the built-in profile is left"
    );
    assert_eq!(window.selected(), None);
    std::fs::remove_dir_all(&dir).ok();
    std::fs::remove_dir_all(&out).ok();
}

#[test]
fn req_set_10_req_ui_13_the_window_draws_every_group_with_its_provenance() {
    let mut rig = rig_with_z1("machines-draw");
    rig.shell.machine_library = MachineLibraryWindow::new(Some(folder("draw")), None);
    rig.shell.open_machine_library();
    rig.frame(Vec::new());
    rig.frame(Vec::new());
    for text in [
        "THIS PROJECT",
        "BUILT IN",
        "LIBRARY",
        "The project's copy: each change can be undone",
        "Save to library…",
        "ROTARY AXIS",
        "MAXIMUM FEEDS",
        "TRAVEL",
        "Axis height (machine Z)",
        "Confirmed on the machine",
        "Not confirmed on the machine",
        "Mark confirmed…",
        "Import…",
        "Export…",
    ] {
        assert!(rig.shows(text), "{text} is not drawn");
    }
    // No wording that says the G-code is safe or ready (NFR-12).
    for word in ["safe to run", "verified", "ready to run", "machine-ready"] {
        assert!(!rig.shows(word), "{word}");
    }
}
