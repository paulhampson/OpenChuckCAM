// SPDX-License-Identifier: AGPL-3.0-or-later
//! An unconfirmed inspector value must never masquerade as a generated
//! input (UI-07, UI-08, UI-10, UI-11, UI-26, NFR-12; issue 304).

#![allow(clippy::unwrap_used)]
#![allow(clippy::float_cmp)]

use eframe::egui::{self, Event, Key, Modifiers};
use openchuck_app::overview::{ItemRef, ItemState, Step};
use openchuck_app::project::ParameterValue;

use super::Selection;
use super::frame_tests::Rig;

fn generated(name: &str) -> Rig {
    let mut rig = Rig::cube(name, None);
    crate::theme::install_style(&rig.ctx, crate::theme::Theme::Dark);
    rig.size = egui::vec2(1920.0, 1080.0);
    let op = rig.shell.app.as_ref().unwrap().project().operations()[0].id;
    rig.shell
        .app
        .as_mut()
        .unwrap()
        .set_parameter(op, "stepover", Some(ParameterValue::Number(0.8)))
        .unwrap();
    rig.shell.generate(true);
    rig.settle();
    assert!(rig.shell.run.last().unwrap().simulation.is_some());
    rig.shell.select(Selection::Operation(op));
    rig.shell.open_step(Step::Operations);
    for _ in 0..4 {
        rig.frame(Vec::new());
    }
    rig
}

fn field(rig: &Rig, label: &str) -> egui::Pos2 {
    let row = rig
        .visible_text_rects()
        .into_iter()
        .find(|(text, _)| text == label)
        .map(|(_, rect)| rect)
        .unwrap_or_else(|| panic!("{label} is visible: {:?}", rig.texts()));
    rig.entry_rects()
        .into_iter()
        .find(|rect| {
            rect.left() > row.right()
                && rect.width() > 60.0
                && (rect.center().y - row.center().y).abs() < 12.0
        })
        .unwrap_or_else(|| panic!("{label} has an entry box"))
        .center()
}

fn type_unconfirmed(rig: &mut Rig, label: &str, text: &str) {
    let at = field(rig, label);
    rig.click(at);
    rig.press(Key::A, Modifiers::COMMAND);
    rig.frame(vec![Event::Text(text.to_owned())]);
    assert!(rig.ctx.egui_wants_keyboard_input());
}

fn click(rig: &mut Rig, text: &str) {
    let at = rig
        .visible_text_rects()
        .into_iter()
        .find(|(label, _)| label == text)
        .map(|(_, rect)| rect.center())
        .unwrap_or_else(|| panic!("{text} is visible: {:?}", rig.texts()));
    rig.click(at);
    for _ in 0..3 {
        rig.frame(Vec::new());
    }
}

#[test]
fn req_ui_07_req_ui_10_req_nfr_12_continue_with_unconfirmed_stepover_asks_first() {
    let mut rig = generated("pending-stepover-continue");
    let project = rig.shell.app.as_ref().unwrap().project().clone();
    let run = rig.shell.run.last().unwrap().clone();
    let op = project.operations()[0].id;
    assert!(rig.shows("Continue to simulate"));
    type_unconfirmed(&mut rig, "Stepover", "1.6");
    assert_eq!(rig.shell.app.as_ref().unwrap().project(), &project);
    click(&mut rig, "Continue to simulate");

    // The saved project and the run's snapshot are the authoritative
    // inputs; visible field text is not evidence of a committed value.
    rig.shell.app.as_ref().unwrap().save().unwrap();
    let reopened = openchuck_app::Application::open(
        rig.shell.app.as_ref().unwrap().location(),
        openchuck_app::Threads::Exactly(2),
    )
    .unwrap();
    assert_eq!(reopened.project(), &project);
    assert_eq!(rig.shell.run.last().unwrap().project, run.project);
    assert_eq!(rig.shell.run.last().unwrap().toolpath, run.toolpath);
    assert_eq!(rig.shell.run.last().unwrap().simulation, run.simulation);
    assert_eq!(
        rig.shell
            .overview
            .status(ItemRef::Operation(op))
            .unwrap()
            .state,
        ItemState::UpToDate
    );

    assert!(rig.shows("Unconfirmed changes"), "{:?}", rig.texts());
    assert_eq!(rig.shell.step, Step::Operations);
    assert!(rig.shows("Keep"));
    assert!(rig.shows("Discard"));
    assert!(rig.shows("Cancel"));
}

#[test]
fn req_ui_07_req_ui_10_discard_restores_stepover_and_then_continues() {
    let mut rig = generated("pending-stepover-discard");
    let before = rig.shell.app.as_ref().unwrap().project().clone();
    let run = rig.shell.run.last().unwrap().clone();
    type_unconfirmed(&mut rig, "Stepover", "1.6");
    click(&mut rig, "Continue to simulate");
    assert!(rig.shows("Unconfirmed changes"));
    click(&mut rig, "Discard");
    rig.settle();
    assert_eq!(rig.shell.step, Step::Simulate);
    assert_eq!(rig.shell.app.as_ref().unwrap().project(), &before);
    assert_eq!(rig.shell.run.last().unwrap().toolpath, run.toolpath);
    rig.shell.open_step(Step::Operations);
    for _ in 0..4 {
        rig.frame(Vec::new());
    }
    assert!(rig.shell.form.typed("stepover").is_none());
}

#[test]
fn req_ui_07_req_ui_10_cancel_preserves_edit_and_returns_keyboard_focus() {
    let mut rig = generated("pending-stepover-cancel");
    let before = rig.shell.app.as_ref().unwrap().project().clone();
    type_unconfirmed(&mut rig, "Stepover", "1.6");
    let focused = rig.ctx.memory(|m| m.focused());
    click(&mut rig, "Continue to simulate");
    assert!(rig.shows("Unconfirmed changes"));
    click(&mut rig, "Cancel");
    assert_eq!(rig.shell.step, Step::Operations);
    assert_eq!(rig.shell.app.as_ref().unwrap().project(), &before);
    assert_eq!(rig.shell.form.typed("stepover"), Some("1.6"));
    assert_eq!(rig.ctx.memory(|m| m.focused()), focused);
    rig.frame(vec![Event::Text("2".into())]);
    assert!(rig.shell.form.typed("stepover").unwrap().contains('2'));
}

#[test]
fn req_ui_07_req_ui_08_req_ui_11_keep_commits_then_rechecks_the_original_primary_action() {
    let mut rig = generated("pending-stepover-keep");
    let before = rig.shell.app.as_ref().unwrap().project().clone();
    let op = before.operations()[0].id;
    let run = rig.shell.run.last().unwrap().clone();
    type_unconfirmed(&mut rig, "Stepover", "0.8 * 2");
    click(&mut rig, "Continue to simulate");
    assert!(rig.shows("Unconfirmed changes"));
    click(&mut rig, "Keep");
    rig.settle();
    assert_eq!(
        rig.shell
            .app
            .as_ref()
            .unwrap()
            .project()
            .operation(op)
            .unwrap()
            .parameters
            .get("stepover"),
        Some(&ParameterValue::Number(1.6))
    );
    // Continue was valid before Keep, but a changed input requires a new
    // path. The same semantic primary action must now regenerate it.
    assert_eq!(rig.shell.step, Step::Operations);
    let changed = rig.shell.run.last().unwrap();
    assert_eq!(
        changed
            .project
            .operation(op)
            .unwrap()
            .parameters
            .get("stepover"),
        Some(&ParameterValue::Number(1.6))
    );
    assert_ne!(changed.toolpath, run.toolpath);
    rig.shell.undo();
    assert_eq!(rig.shell.app.as_ref().unwrap().project(), &before);
}

#[test]
fn req_ui_07_req_ui_26_req_nfr_07_keep_an_invalid_expression_stops_departure() {
    let mut rig = generated("pending-stepover-invalid");
    let before = rig.shell.app.as_ref().unwrap().project().clone();
    let run = rig.shell.run.last().unwrap().clone();
    type_unconfirmed(&mut rig, "Stepover", "12*");
    click(&mut rig, "Continue to simulate");
    assert!(rig.shows("Unconfirmed changes"));
    click(&mut rig, "Keep");
    assert_eq!(rig.shell.step, Step::Operations);
    assert!(rig.shows("Cannot read '12*'"), "{:?}", rig.texts());
    assert_eq!(rig.shell.app.as_ref().unwrap().project(), &before);
    assert_eq!(rig.shell.run.last().unwrap().toolpath, run.toolpath);
    assert!(rig.ctx.egui_wants_keyboard_input());
}

#[test]
fn req_ui_07_req_ui_08_same_form_field_transfer_still_confirms_automatically() {
    let mut rig = generated("pending-stepover-field-transfer");
    let op = rig.shell.app.as_ref().unwrap().project().operations()[0].id;
    type_unconfirmed(&mut rig, "Stepover", "1.6");
    let next = field(&rig, "Tolerance");
    rig.click(next);
    assert!(!rig.shows("Unconfirmed changes"));
    assert_eq!(
        rig.shell
            .app
            .as_ref()
            .unwrap()
            .project()
            .operation(op)
            .unwrap()
            .parameters
            .get("stepover"),
        Some(&ParameterValue::Number(1.6))
    );
    assert!(rig.ctx.egui_wants_keyboard_input());
}

#[test]
fn req_ui_07_req_ui_26_req_nfr_07_all_pending_fields_are_checked_before_any_keep() {
    let mut rig = generated("pending-multi-field-invalid");
    let before = rig.shell.app.as_ref().unwrap().project().clone();
    type_unconfirmed(&mut rig, "Stepover", "12*");
    // Moving to a second field tries the ordinary auto-confirm and refuses
    // the first entry. Both fields still belong to this form.
    type_unconfirmed(&mut rig, "Tolerance", "0.2");
    assert!(rig.shows("Cannot read '12*'"));
    click(&mut rig, "Continue to simulate");
    assert!(rig.shows("Unconfirmed changes"));
    click(&mut rig, "Keep");
    assert_eq!(rig.shell.app.as_ref().unwrap().project(), &before);
    assert_eq!(rig.shell.step, Step::Operations);
    assert!(rig.shows("Cannot read '12*'"));
}

#[test]
fn req_ui_07_req_ui_10_navigation_selection_dialog_generation_and_save_share_the_guard() {
    for departure in 0..5 {
        let mut rig = generated(&format!("pending-departure-{departure}"));
        let before = rig.shell.app.as_ref().unwrap().project().clone();
        let selected = rig.shell.selected;
        type_unconfirmed(&mut rig, "Stepover", "1.6");
        match departure {
            0 => rig.shell.open_step(Step::Setup),
            1 => rig
                .shell
                .tree_action(super::TreeAction::SelectSetup(before.setups()[0].id, None)),
            2 => rig.shell.import_model(),
            3 => rig.shell.generate(true),
            4 => {
                assert!(!rig.shell.save());
            }
            _ => unreachable!(),
        }
        for _ in 0..4 {
            rig.frame(Vec::new());
        }
        assert!(rig.shows("Unconfirmed changes"), "departure {departure}");
        assert_eq!(rig.shell.app.as_ref().unwrap().project(), &before);
        assert_eq!(rig.shell.step, Step::Operations);
        assert_eq!(rig.shell.selected, selected);
        assert!(rig.shell.import.is_none());
        assert!(!rig.shell.run.is_running());
        click(&mut rig, "Cancel");
        assert_eq!(rig.shell.app.as_ref().unwrap().project(), &before);
        assert_eq!(rig.shell.form.typed("stepover"), Some("1.6"));
    }
}

#[test]
fn req_ui_07_req_ui_10_native_close_is_vetoed_even_when_the_project_is_saved() {
    let mut rig = generated("pending-native-close");
    rig.shell.app.as_ref().unwrap().save().unwrap();
    assert!(!rig.shell.app.as_ref().unwrap().is_modified());
    type_unconfirmed(&mut rig, "Stepover", "1.6");
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
    click(&mut rig, "Cancel");
    assert!(!rig.shell.quitting);
    assert_eq!(rig.shell.form.typed("stepover"), Some("1.6"));
    assert!(rig.ctx.egui_wants_keyboard_input());
}

#[test]
fn req_ui_07_req_ui_08_setup_form_keep_uses_the_existing_undoable_edit() {
    let mut rig = Rig::cube("pending-setup-keep", None);
    crate::theme::install_style(&rig.ctx, crate::theme::Theme::Dark);
    rig.size = egui::vec2(1920.0, 1080.0);
    let before = rig.shell.app.as_ref().unwrap().project().clone();
    rig.shell.choose_setup(before.setups()[0].id, None);
    for _ in 0..5 {
        rig.frame(Vec::new());
    }
    type_unconfirmed(&mut rig, "Diameter", "22");
    rig.shell.open_step(Step::Operations);
    for _ in 0..3 {
        rig.frame(Vec::new());
    }
    assert!(rig.shows("Unconfirmed changes"));
    assert_eq!(rig.shell.app.as_ref().unwrap().project(), &before);
    click(&mut rig, "Keep");
    rig.settle();
    assert_eq!(rig.shell.step, Step::Operations);
    assert!(
        matches!(rig.shell.app.as_ref().unwrap().project().setups()[0].stock,
        Some(openchuck_app::setup::Stock::Cylinder { diameter, .. }) if diameter.0 == 22.0)
    );
    rig.shell.undo();
    assert_eq!(rig.shell.app.as_ref().unwrap().project(), &before);
}

#[test]
fn req_ui_07_req_ui_08_req_tl_07_project_tool_close_keeps_through_a_command() {
    use crate::tool_library_window::{Chosen, Purpose, ToolLibraryWindow};
    let mut rig = Rig::cube("pending-project-tool", None);
    crate::theme::install_style(&rig.ctx, crate::theme::Theme::Dark);
    rig.size = egui::vec2(1920.0, 1080.0);
    let before = rig.shell.app.as_ref().unwrap().project().clone();
    let tool = before.tools()[0].id;
    rig.shell.tool_library = ToolLibraryWindow::new(Some(rig.dirs[0].join("library")));
    rig.shell
        .open_tool_library(Purpose::Add, Some(Chosen::Project(tool)));
    for _ in 0..6 {
        rig.frame(Vec::new());
    }
    type_unconfirmed(&mut rig, "Name", "Renamed project tool");
    click(&mut rig, "Close");
    assert!(rig.shows("Unconfirmed changes"));
    assert_eq!(rig.shell.app.as_ref().unwrap().project(), &before);
    click(&mut rig, "Keep");
    rig.settle();
    assert!(!rig.shell.tool_library.is_open());
    assert_eq!(
        rig.shell.app.as_ref().unwrap().project().tools()[0].name,
        "Renamed project tool"
    );
    rig.shell.undo();
    assert_eq!(rig.shell.app.as_ref().unwrap().project(), &before);
}

#[test]
fn req_ui_07_req_tl_07_keep_a_library_field_does_not_silently_save_the_draft() {
    use crate::tool_library_window::{Chosen, Purpose, ToolLibraryWindow};
    use openchuck_app::tool_library::{self, ToolType};
    use openchuck_app::tools::ToolLibrary;
    let mut rig = Rig::cube("pending-library-draft", None);
    crate::theme::install_style(&rig.ctx, crate::theme::Theme::Dark);
    rig.size = egui::vec2(1920.0, 1080.0);
    let folder = rig.dirs[0].join("library");
    let mut library = ToolLibrary::open(&folder).unwrap();
    let mut draft = tool_library::new_draft(&library, ToolType::FlatEnd).unwrap();
    tool_library::edit_name(&mut draft, "Original library tool").unwrap();
    tool_library::save_draft(&mut library, &draft, &[]).unwrap();
    let tool = draft.id;
    rig.shell.tool_library = ToolLibraryWindow::new(Some(folder.clone()));
    rig.shell
        .open_tool_library(Purpose::Add, Some(Chosen::Library(tool)));
    for _ in 0..6 {
        rig.frame(Vec::new());
    }
    type_unconfirmed(&mut rig, "Name", "Unpublished library draft");
    click(&mut rig, "Close");
    assert!(rig.shows("Unconfirmed changes"));
    click(&mut rig, "Keep");
    rig.settle();
    assert!(rig.shell.tool_library.is_open());
    assert!(
        rig.shell.tool_library.asks_to_discard(),
        "the existing enclosing-draft guard remains"
    );
    assert_eq!(
        ToolLibrary::open(&folder).unwrap().get(tool).unwrap().name,
        "Original library tool"
    );
}

#[test]
fn req_ui_07_req_ui_08_req_ui_11_enter_commits_stepover_and_marks_the_old_run_stale() {
    let mut rig = generated("pending-stepover-enter");
    let before = rig.shell.app.as_ref().unwrap().project().clone();
    let op = before.operations()[0].id;
    let run = rig.shell.run.last().unwrap().clone();
    type_unconfirmed(&mut rig, "Stepover", "1.6");
    rig.press(Key::Enter, Modifiers::NONE);
    rig.settle();
    assert_eq!(
        rig.shell
            .app
            .as_ref()
            .unwrap()
            .project()
            .operation(op)
            .unwrap()
            .parameters
            .get("stepover"),
        Some(&ParameterValue::Number(1.6))
    );
    assert_eq!(
        rig.shell
            .overview
            .status(ItemRef::Operation(op))
            .unwrap()
            .state,
        ItemState::Stale
    );
    assert_eq!(rig.shell.run.last().unwrap().toolpath, run.toolpath);
    assert_eq!(rig.shell.run.last().unwrap().simulation, run.simulation);
    rig.shell.undo();
    assert_eq!(rig.shell.app.as_ref().unwrap().project(), &before);
    rig.shell.redo();
    rig.shell.generate(true);
    rig.settle();
    let changed = rig.shell.run.last().unwrap();
    assert_eq!(
        changed.project,
        std::sync::Arc::new(rig.shell.app.as_ref().unwrap().project().clone())
    );
    assert_ne!(changed.toolpath, run.toolpath);
    assert!(changed.simulation.is_some());
}
