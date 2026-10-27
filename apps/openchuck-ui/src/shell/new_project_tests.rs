// SPDX-License-Identifier: AGPL-3.0-or-later
//! The New project dialog: a parent folder, a project name, the cleaned
//! folder name it shows, and the safe handling of a folder that exists
//! (#219, decision record 0165; NFR-07).

#![allow(clippy::unwrap_used)]

use eframe::egui::{Event, Key, Modifiers, PointerButton};

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

fn type_name(rig: &mut Rig, name: &str) {
    let at = rig
        .visible_text_rects()
        .into_iter()
        .find(|(text, _)| text == "Untitled")
        .expect("the name field shows Untitled")
        .1
        .center();
    rig.click(at);
    rig.press(Key::A, Modifiers::COMMAND);
    rig.frame(vec![Event::Text(name.to_owned())]);
    assert!(!crate::pending_edits::entries(&rig.ctx).is_empty());
}

#[test]
fn req_prj_01_req_ui_07_new_project_ok_confirms_its_typed_name_without_a_warning() {
    let parent = scratch("typed-ok");
    let mut rig = blank();
    open_dialog(&mut rig, &parent, "Untitled");
    type_name(&mut rig, "Blisk");
    click(&mut rig, "OK");
    assert!(!rig.shows("Unconfirmed changes"));
    assert_eq!(rig.shell.app.as_ref().unwrap().project().name(), "Blisk");
    assert!(parent.join("Blisk/project.toml").is_file());
    let _ = std::fs::remove_dir_all(&parent);
}

#[test]
fn req_prj_01_req_nfr_07_new_project_ok_still_refuses_an_empty_typed_name() {
    let parent = scratch("typed-empty");
    let mut rig = blank();
    open_dialog(&mut rig, &parent, "Untitled");
    type_name(&mut rig, "   ");
    assert!(rig.shows("Give the project a name."));
    assert!(matches!(
        rig.shell.dialog,
        Some(Dialog::Folder { create: true, .. })
    ));
    assert!(rig.shell.app.is_none());
    assert_eq!(std::fs::read_dir(&parent).unwrap().count(), 0);
    let _ = std::fs::remove_dir_all(&parent);
}

#[test]
fn req_ui_07_new_project_cancel_discards_its_typed_name_without_a_warning() {
    let parent = scratch("typed-cancel");
    let mut rig = blank();
    open_dialog(&mut rig, &parent, "Untitled");
    type_name(&mut rig, "Blisk");
    click(&mut rig, "Cancel");
    assert!(!rig.shows("Unconfirmed changes"));
    assert!(rig.shell.dialog.is_none());
    assert!(crate::pending_edits::entries(&rig.ctx).is_empty());
    assert_eq!(std::fs::read_dir(&parent).unwrap().count(), 0);
    let _ = std::fs::remove_dir_all(&parent);
}

#[test]
fn req_ui_14_new_project_escape_restores_the_field_without_closing_the_dialog() {
    let parent = scratch("typed-escape");
    let mut rig = blank();
    open_dialog(&mut rig, &parent, "Untitled");
    type_name(&mut rig, "Blisk");
    rig.press(Key::Escape, Modifiers::NONE);
    rig.frame(Vec::new());
    assert!(!rig.shows("Unconfirmed changes"));
    assert!(matches!(
        &rig.shell.dialog,
        Some(Dialog::Folder { create: true, name, .. }) if name == "Untitled"
    ));
    let _ = std::fs::remove_dir_all(&parent);
}

#[test]
fn req_ui_07_req_ui_14_new_project_app_close_still_uses_the_departure_guard() {
    let parent = scratch("typed-close");
    let mut rig = blank();
    open_dialog(&mut rig, &parent, "Untitled");
    type_name(&mut rig, "Blisk");
    let mut input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, rig.size)),
        ..Default::default()
    };
    input
        .viewports
        .get_mut(&egui::ViewportId::ROOT)
        .unwrap()
        .events
        .push(egui::ViewportEvent::Close);
    let mut output = rig.ctx.run_ui(input, |ui| rig.shell.draw(ui));
    output.textures_delta.clear();
    assert!(
        output.viewport_output[&egui::ViewportId::ROOT]
            .commands
            .contains(&egui::ViewportCommand::CancelClose)
    );
    for _ in 0..3 {
        rig.frame(Vec::new());
    }
    assert!(rig.shows("Unconfirmed changes"));
    assert!(!rig.shell.quitting);
    let _ = std::fs::remove_dir_all(&parent);
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
