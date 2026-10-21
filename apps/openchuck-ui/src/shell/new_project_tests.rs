// SPDX-License-Identifier: AGPL-3.0-or-later
//! The New project dialog: a parent folder, a project name, the cleaned
//! folder name it shows, and the safe handling of a folder that exists
//! (#219, decision record 0165; NFR-07).

#![allow(clippy::unwrap_used)]

use eframe::egui::{Event, Modifiers, PointerButton};

use super::frame_tests::Rig;
use super::*;

fn blank() -> Rig {
    let mut rig = Rig::new(Shell::with_settings(false, None, None), Vec::new());
    rig.frame(Vec::new());
    rig
}

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("oc-newproject-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn open_dialog(rig: &mut Rig, parent: &std::path::Path, name: &str) {
    rig.shell.dialog = Some(Dialog::Folder {
        create: true,
        folder: parent.display().to_string(),
        name: name.to_owned(),
    });
    rig.frame(Vec::new());
    rig.frame(Vec::new());
}

/// Clicks the button painted with exactly `label`.
fn click(rig: &mut Rig, label: &str) {
    // A dialog that has just opened is placed after its first frames.
    for _ in 0..3 {
        rig.frame(Vec::new());
    }
    let at = rig
        .text_rects()
        .iter()
        .find(|(t, _)| t == label)
        .map(|(_, r)| r.center())
        .unwrap_or_else(|| panic!("{label} is painted"));
    rig.frame(vec![Event::PointerMoved(at)]);
    let button = |pressed| Event::PointerButton {
        pos: at,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    rig.frame(vec![button(true)]);
    rig.frame(vec![button(false)]);
    rig.frame(Vec::new());
}

#[test]
fn req_prj_01_the_dialog_shows_the_cleaned_folder_name() {
    let parent = scratch("shows");
    let mut rig = blank();
    open_dialog(&mut rig, &parent, "CON");
    assert!(rig.shows("Parent folder:"));
    assert!(
        rig.shows("Folder to create: _CON"),
        "the cleaned name is shown"
    );
    let _ = std::fs::remove_dir_all(&parent);
}

#[test]
fn req_prj_01_ok_makes_the_folder_named_after_the_project_and_opens_it() {
    let parent = scratch("make");
    let mut rig = blank();
    open_dialog(&mut rig, &parent, "Blisk: v2");
    click(&mut rig, "OK");
    let folder = parent.join("Blisk_ v2");
    assert!(folder.join("project.toml").is_file());
    let app = rig.shell.app.as_ref().expect("the project is open");
    assert_eq!(app.location(), folder);
    assert_eq!(
        app.project().name(),
        "Blisk: v2",
        "the project keeps its own name"
    );
    let _ = std::fs::remove_dir_all(&parent);
}

#[test]
fn req_nfr_07_an_existing_folder_asks_and_using_it_keeps_its_files() {
    let parent = scratch("exists");
    let existing = parent.join("Blisk");
    std::fs::create_dir(&existing).unwrap();
    std::fs::write(existing.join("part.step"), "keep me").unwrap();
    let mut rig = blank();
    open_dialog(&mut rig, &parent, "Blisk");
    click(&mut rig, "OK");
    assert!(rig.shows("Folder already exists"), "the user is asked");
    assert!(
        rig.shell.app.is_none(),
        "nothing is created before the answer"
    );
    assert!(!existing.join("project.toml").exists());
    click(&mut rig, "Use the existing folder");
    assert!(existing.join("project.toml").is_file());
    assert_eq!(
        std::fs::read_to_string(existing.join("part.step")).unwrap(),
        "keep me"
    );
    assert!(rig.shell.app.is_some());
    let _ = std::fs::remove_dir_all(&parent);
}

#[test]
fn req_nfr_07_changing_the_name_goes_back_to_the_form_with_what_was_typed() {
    let parent = scratch("change");
    std::fs::create_dir(parent.join("Blisk")).unwrap();
    let mut rig = blank();
    open_dialog(&mut rig, &parent, "Blisk");
    click(&mut rig, "OK");
    click(&mut rig, "Change the name");
    assert!(rig.shows("Project name:"));
    assert!(matches!(
        &rig.shell.dialog,
        Some(Dialog::Folder { create: true, name, .. }) if name == "Blisk"
    ));
    assert!(rig.shell.app.is_none());
    let _ = std::fs::remove_dir_all(&parent);
}

#[test]
fn req_nfr_07_a_folder_holding_a_project_is_refused_and_untouched() {
    let parent = scratch("project");
    let existing = parent.join("Blisk");
    std::fs::create_dir(&existing).unwrap();
    std::fs::write(existing.join("project.toml"), "mine").unwrap();
    std::fs::write(existing.join("machine.toml"), "mine too").unwrap();
    let mut rig = blank();
    open_dialog(&mut rig, &parent, "Blisk");
    assert!(
        rig.text_rects()
            .iter()
            .any(|(t, _)| t.contains("already holds an OpenChuckCAM project")),
        "the refusal is said plainly"
    );
    click(&mut rig, "OK");
    assert!(rig.shell.app.is_none());
    assert!(
        !rig.shows("Folder already exists"),
        "there is nothing to choose"
    );
    assert_eq!(
        std::fs::read_to_string(existing.join("project.toml")).unwrap(),
        "mine"
    );
    assert_eq!(
        std::fs::read_to_string(existing.join("machine.toml")).unwrap(),
        "mine too"
    );
    let _ = std::fs::remove_dir_all(&parent);
}

#[test]
fn req_nfr_07_a_missing_parent_folder_is_said() {
    let parent = scratch("noparent").join("missing");
    let mut rig = blank();
    open_dialog(&mut rig, &parent, "Blisk");
    assert!(rig.shows("The parent folder does not exist."));
}

#[test]
fn req_nfr_07_a_blank_project_name_is_said_and_makes_nothing() {
    // OI-694: a blank or whitespace-only name left OK enabled and made a
    // project named "". The dialog says what is missing and OK does nothing.
    for name in ["", "   ", "\t "] {
        let parent = scratch("blank");
        let mut rig = blank();
        open_dialog(&mut rig, &parent, name);
        assert!(rig.shows("Give the project a name."), "{name:?}");
        click(&mut rig, "OK");
        assert!(
            matches!(rig.shell.dialog, Some(Dialog::Folder { .. })),
            "the dialog stays open"
        );
        assert!(rig.shell.app.is_none(), "no project was made");
        assert_eq!(
            std::fs::read_dir(&parent).unwrap().count(),
            0,
            "nothing was created in {}",
            parent.display()
        );
        let _ = std::fs::remove_dir_all(&parent);
    }
}
