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

pub(super) fn generated(name: &str) -> Rig {
    let mut rig = Rig::cube(name, None);
    crate::theme::install_style(&rig.ctx, crate::theme::Theme::Dark);
    rig.ctx
        .style_mut_of(egui::Theme::Dark, |style| style.animation_time = 0.0);
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
        // Floating panels can recolour their field background. The value's
        // painted text still identifies the entry beside its visible label.
        .or_else(|| {
            rig.visible_text_rects()
                .into_iter()
                .filter(|(_, rect)| {
                    rect.left() > row.right() && (rect.center().y - row.center().y).abs() < 3.0
                })
                .map(|(_, rect)| rect)
                .min_by(|a, b| a.left().total_cmp(&b.left()))
        })
        .unwrap_or_else(|| panic!("{label} has an entry box"))
        .center()
}

pub(super) fn type_unconfirmed(rig: &mut Rig, label: &str, text: &str) {
    let at = field(rig, label);
    rig.click(at);
    rig.press(Key::A, Modifiers::COMMAND);
    rig.frame(vec![Event::Text(text.to_owned())]);
    assert!(
        rig.ctx.egui_wants_keyboard_input(),
        "{label} must have keyboard focus"
    );
    assert!(
        !crate::pending_edits::entries(&rig.ctx).is_empty(),
        "{label}: {:?}",
        rig.texts()
    );
}

fn click(rig: &mut Rig, text: &str) {
    let at = rig
        .visible_text_rects()
        .into_iter()
        .rev()
        .find(|(label, _)| label == text)
        .map(|(_, rect)| rect.center())
        .unwrap_or_else(|| panic!("{text} is visible: {:?}", rig.texts()));
    rig.click(at);
    for _ in 0..3 {
        rig.frame(Vec::new());
    }
}

fn choose_export_post(rig: &mut Rig, choice: &str) {
    let posts = openchuck_app::builtin_post_summaries();
    let label = rig
        .visible_text_rects()
        .into_iter()
        .find(|(text, _)| text == "A post folder" || posts.iter().any(|p| p.name == *text))
        .unwrap()
        .1;
    rig.click(label.center());
    for _ in 0..2 {
        rig.frame(Vec::new());
    }
    click(rig, choice);
}

#[test]
fn req_ui_07_req_ui_12_req_gc_15_export_writes_and_history_use_the_departure_guard() {
    let mut rig = generated("output-pending-history");
    // Never write the host's per-user acknowledgement file in a test.
    rig.shell.export = crate::export_panel::ExportPanel::new(None);
    let out = rig.dirs[0].join("guarded-output");
    rig.shell.open_export();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
    while !rig.shows("Go to line") {
        rig.frame(Vec::new());
        assert!(
            std::time::Instant::now() < deadline,
            "export files prepared"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    type_unconfirmed(&mut rig, "Output folder", &out.display().to_string());
    // The pointer is over the left column: over the preview the wheel
    // scrolls the preview and not the dialog (decision 0327).
    let dialog = rig
        .ctx
        .memory(|m| m.area_rect(egui::Id::unique(("dialog", "export dialog"))))
        .unwrap();
    let at = egui::pos2(dialog.left() + 40.0, dialog.center().y);
    for _ in 0..12 {
        if rig
            .visible_text_rects()
            .iter()
            .any(|(text, _)| text == openchuck_app::EXPORT_ACKNOWLEDGE_LABEL)
        {
            break;
        }
        rig.frame(vec![
            Event::PointerMoved(at),
            Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                source: egui::MouseWheelSource::Unknown,
                phase: egui::TouchPhase::Move,
                delta: egui::vec2(0.0, -160.0),
                modifiers: Modifiers::NONE,
            },
        ]);
    }
    click(&mut rig, openchuck_app::EXPORT_ACKNOWLEDGE_LABEL);
    click(&mut rig, "Write G-code");
    assert!(rig.shows("Unconfirmed changes"));
    assert!(!out.exists(), "pending typed destination cannot write");
    click(&mut rig, "Cancel");
    assert!(!out.exists());
    click(&mut rig, "Write G-code");
    click(&mut rig, "Keep");
    for _ in 0..8 {
        rig.frame(Vec::new());
    }
    assert!(rig.shows("Export result"));
    assert!(
        rig.shell
            .overview
            .messages
            .iter()
            .any(|m| m.action == Some(openchuck_app::overview::MessageAction::ReviewExport(0)))
    );
    let paths = rig
        .shell
        .app
        .as_ref()
        .unwrap()
        .output_report(0)
        .unwrap()
        .written;
    assert_eq!(paths.len(), 3);
    assert!(
        paths
            .iter()
            .all(|path| path.starts_with(&out) && path.is_file())
    );
    click(&mut rig, "Dismiss");
    click(&mut rig, "Cancel"); // Close the export form, retaining history.
    assert!(
        !rig.shell.export.is_open(),
        "export closed: {:?}",
        rig.texts()
    );
    let op = rig.shell.app.as_ref().unwrap().project().operations()[0].id;
    rig.shell.select(Selection::Operation(op));
    rig.shell.open_step(Step::Operations);
    for _ in 0..4 {
        rig.frame(Vec::new());
    }
    type_unconfirmed(&mut rig, "Stepover", "1.6");
    let action = openchuck_app::overview::MessageAction::ReviewExport(0);
    rig.shell.message_action(action);
    rig.frame(Vec::new());
    assert!(rig.shows("Unconfirmed changes"));
    assert!(!rig.shows("Export result"));
    click(&mut rig, "Cancel");
    assert!(!rig.shows("Export result"));
    rig.shell.message_action(action);
    rig.frame(Vec::new());
    click(&mut rig, "Discard");
    for _ in 0..5 {
        rig.frame(Vec::new());
    }
    assert!(rig.shows("Export result"));
    assert_eq!(
        rig.shell
            .app
            .as_ref()
            .unwrap()
            .output_report(0)
            .unwrap()
            .written,
        paths
    );
    assert_eq!(
        std::fs::read_dir(&out).unwrap().count(),
        3,
        "history never writes again"
    );
    rig.press(Key::Num4, Modifiers::ALT);
    assert_eq!(
        rig.shell.step,
        Step::Operations,
        "global step keys stay behind the modal"
    );
    // A background job must not steal the result modal's explicit Esc.
    rig.shell.generate(true);
    assert!(rig.shell.run.is_running());
    rig.press(Key::Escape, Modifiers::NONE);
    assert!(
        !rig.shell.export.is_open(),
        "Esc dismisses the result before global shortcuts"
    );
}

#[test]
fn req_ui_07_req_ui_10_req_nfr_07_keep_of_a_hidden_field_preserves_drafts_and_allows_recovery() {
    for answer in ["Cancel", "Escape", "Discard"] {
        let mut rig = generated(&format!("pending-hidden-export-{answer}"));
        let before = rig.shell.app.as_ref().unwrap().project().clone();
        rig.shell.open_export();
        for _ in 0..4 {
            rig.frame(Vec::new());
        }
        choose_export_post(&mut rig, "A post folder");
        let post = rig
            .visible_text_rects()
            .into_iter()
            .find(|(text, _)| text == "Post")
            .unwrap()
            .1;
        let entry = rig
            .painted()
            .into_iter()
            .filter(|(text, r, _)| {
                text.is_none()
                    && r.height() < 45.0
                    && r.center().y > post.bottom() + 2.0
                    && r.center().y < post.bottom() + 50.0
                    && r.left() < post.left()
                    && r.right() > post.left()
            })
            .map(|(_, _, clipped)| clipped)
            .min_by(|a, b| a.top().total_cmp(&b.top()))
            .unwrap();
        rig.click(entry.center());
        rig.frame(vec![Event::Text("/unconfirmed/post".into())]);
        assert!(
            crate::pending_edits::entries(&rig.ctx)
                .iter()
                .any(|e| e.label == "Post folder"),
            "{:?}",
            crate::pending_edits::entries(&rig.ctx)
        );
        let builtin = openchuck_app::builtin_post_summaries()[0].name.clone();
        choose_export_post(&mut rig, &builtin);
        // A different choice hides the custom path without confirming it.
        assert!(!rig.shows("/unconfirmed/post"));
        type_unconfirmed(&mut rig, "Output folder", "/unconfirmed/output");
        let drafts = crate::pending_edits::entries(&rig.ctx);
        assert_eq!(drafts.len(), 2);
        click(&mut rig, "Cancel"); // export's close action
        assert!(rig.shows("Unconfirmed changes"));
        click(&mut rig, "Keep");
        for _ in 0..8 {
            rig.frame(Vec::new());
        }
        assert!(rig.shows("Unconfirmed changes"), "{:?}", rig.texts());
        assert!(rig.shows("not shown"), "{:?}", rig.texts());
        // Keep must check availability before applying even a visible field.
        let retained = crate::pending_edits::entries(&rig.ctx);
        assert_eq!(retained.len(), drafts.len());
        for original in &drafts {
            let entry = retained.iter().find(|e| e.id == original.id).unwrap();
            assert_eq!(entry.checked, original.checked);
        }
        assert_eq!(rig.shell.app.as_ref().unwrap().project(), &before);
        if answer == "Escape" {
            rig.press(Key::Escape, Modifiers::NONE);
            for _ in 0..3 {
                rig.frame(Vec::new());
            }
        } else {
            click(&mut rig, answer);
        }
        assert!(!rig.shell.edit_guard.active());
        if answer != "Discard" {
            assert!(rig.shell.export.is_open());
            assert_eq!(crate::pending_edits::entries(&rig.ctx).len(), 2);
            choose_export_post(&mut rig, "A post folder");
            assert!(rig.shows("/unconfirmed/post"));
            click(&mut rig, "Cancel");
            click(&mut rig, "Keep");
            for _ in 0..8 {
                rig.frame(Vec::new());
            }
        }
        assert!(
            !rig.shell.export.is_open(),
            "{answer}: {:?}; {:?}",
            crate::pending_edits::entries(&rig.ctx),
            rig.texts()
        );
        assert!(crate::pending_edits::entries(&rig.ctx).is_empty());
        assert_eq!(rig.shell.app.as_ref().unwrap().project(), &before);
        rig.shell.open_export();
        for _ in 0..4 {
            rig.frame(Vec::new());
        }
        choose_export_post(&mut rig, "A post folder");
        assert_eq!(rig.shows("/unconfirmed/post"), answer != "Discard");
        assert_eq!(rig.shows("/unconfirmed/output"), answer != "Discard");
        assert!(crate::pending_edits::entries(&rig.ctx).is_empty());
    }
}

#[test]
fn req_ui_07_req_ui_25_req_nfr_07_keep_checks_the_tab_count_range_before_changing_the_draft() {
    for (text, valid) in [
        ("0", false),
        ("65", false),
        ("bad", false),
        ("1.5", false),
        ("1", true),
        ("64", true),
    ] {
        let mut rig = Rig::cube(&format!("pending-tab-count-{text}"), None);
        crate::theme::install_style(&rig.ctx, crate::theme::Theme::Dark);
        rig.size = egui::vec2(1920.0, 1800.0);
        let before = rig.shell.app.as_ref().unwrap().project().clone();
        rig.shell
            .choose_setup(before.setups()[0].id, Some(super::SetupPart::Workholding));
        rig.shell.open_step(Step::Setup);
        for _ in 0..6 {
            rig.frame(Vec::new());
        }
        click(&mut rig, "Add tab");
        if !rig.shows("Around the axis") {
            click(&mut rig, "On the axis");
        }
        click(&mut rig, "Around the axis");
        let draft = rig
            .shell
            .workholding_panel
            .as_mut()
            .unwrap()
            .draft_mut()
            .clone();
        type_unconfirmed(&mut rig, "Count", text);
        click(&mut rig, "Operations");
        assert!(rig.shows("Unconfirmed changes"));
        click(&mut rig, "Keep");
        assert_eq!(rig.shell.app.as_ref().unwrap().project(), &before);
        if valid {
            assert_eq!(rig.shell.step, Step::Operations);
            let tab = &rig
                .shell
                .workholding_panel
                .as_mut()
                .unwrap()
                .draft_mut()
                .tabs[0];
            assert!(
                matches!(tab.layout, openchuck_app::setup::TabLayout::AroundAxis { count, .. }
                if count == text.parse::<u32>().unwrap())
            );
        } else {
            assert_eq!(rig.shell.step, Step::Setup);
            assert_eq!(
                rig.shell.workholding_panel.as_mut().unwrap().draft_mut(),
                &draft
            );
            assert!(
                rig.shows(if text == "bad" {
                    "Cannot read 'bad'"
                } else if text == "1.5" {
                    "Not a whole number"
                } else {
                    "From 1 to 64"
                }),
                "{:?}",
                rig.texts()
            );
            assert!(rig.ctx.egui_wants_keyboard_input());
        }
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
fn req_ui_07_req_ui_10_operation_form_still_warns_after_a_typed_edit() {
    let mut rig = generated("new-project-guard-regression");
    type_unconfirmed(&mut rig, "Stepover", "1.6");
    click(&mut rig, "Continue to simulate");
    assert!(rig.shows("Unconfirmed changes"));
    assert_eq!(rig.shell.step, Step::Operations);
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
    rig.ctx
        .style_mut_of(egui::Theme::Dark, |style| style.animation_time = 0.0);
    rig.size = egui::vec2(1920.0, 1080.0);
    let before = rig.shell.app.as_ref().unwrap().project().clone();
    rig.shell.choose_setup(before.setups()[0].id, None);
    rig.shell.open_step(Step::Setup);
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
    rig.ctx
        .style_mut_of(egui::Theme::Dark, |style| style.animation_time = 0.0);
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
    rig.ctx
        .style_mut_of(egui::Theme::Dark, |style| style.animation_time = 0.0);
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

#[test]
fn req_ui_07_req_nfr_07_keep_checks_tool_constraints_before_applying_any_field() {
    use crate::tool_library_window::{Chosen, Purpose, ToolLibraryWindow};
    let mut rig = Rig::cube("pending-tool-constraints", None);
    crate::theme::install_style(&rig.ctx, crate::theme::Theme::Dark);
    rig.ctx
        .style_mut_of(egui::Theme::Dark, |style| style.animation_time = 0.0);
    rig.size = egui::vec2(1920.0, 1080.0);
    let before = rig.shell.app.as_ref().unwrap().project().clone();
    let tool = before.tools()[0].id;
    rig.shell.tool_library = ToolLibraryWindow::new(Some(rig.dirs[0].join("library")));
    rig.shell
        .open_tool_library(Purpose::Add, Some(Chosen::Project(tool)));
    for _ in 0..6 {
        rig.frame(Vec::new());
    }
    type_unconfirmed(&mut rig, "Diameter", "5");
    // Leaving a box for a label retains its unconfirmed buffer. This makes
    // two pending fields without bypassing the real widgets.
    click(&mut rig, "TOOL");
    type_unconfirmed(&mut rig, "Flute length", "50");
    click(&mut rig, "Close");
    assert!(rig.shows("Unconfirmed changes"));
    click(&mut rig, "Keep");
    for _ in 0..6 {
        rig.frame(Vec::new());
    }
    assert_eq!(
        rig.shell.app.as_ref().unwrap().project(),
        &before,
        "a flute longer than the whole tool must prevent the diameter edit too"
    );
    assert!(rig.shell.tool_library.is_open());
    assert!(rig.ctx.egui_wants_keyboard_input());
}

#[test]
fn req_ui_07_req_ui_10_collapsed_pending_section_reopens_on_cancel_and_invalid_keep() {
    let mut rig = generated("pending-collapsed-section");
    let before = rig.shell.app.as_ref().unwrap().project().clone();
    type_unconfirmed(&mut rig, "Stepover", "12*");
    click(&mut rig, "PATTERN AND TOLERANCE");
    assert!(!rig.shows("Stepover"));
    click(&mut rig, "Setup");
    assert!(rig.shows("Unconfirmed changes"));
    click(&mut rig, "Cancel");
    assert!(rig.shows("Stepover"));
    assert!(rig.ctx.egui_wants_keyboard_input());
    click(&mut rig, "Setup");
    click(&mut rig, "Keep");
    assert_eq!(rig.shell.step, Step::Operations);
    assert!(rig.shows("Cannot read '12*'"));
    assert_eq!(rig.shell.app.as_ref().unwrap().project(), &before);
}

#[test]
fn req_ui_07_req_ui_10_escape_cancels_the_warning_without_reverting_the_field() {
    let mut rig = generated("pending-modal-escape");
    let before = rig.shell.app.as_ref().unwrap().project().clone();
    type_unconfirmed(&mut rig, "Stepover", "1.6");
    let focus = rig.ctx.memory(|m| m.focused());
    click(&mut rig, "Setup");
    assert!(rig.shows("Unconfirmed changes"));
    rig.press(Key::Escape, Modifiers::NONE);
    for _ in 0..4 {
        rig.frame(Vec::new());
    }
    assert!(!rig.shows("Unconfirmed changes"));
    assert_eq!(rig.shell.form.typed("stepover"), Some("1.6"));
    assert_eq!(rig.ctx.memory(|m| m.focused()), focus);
    assert_eq!(rig.shell.app.as_ref().unwrap().project(), &before);
    assert_eq!(rig.shell.step, Step::Operations);
}

#[test]
fn req_ui_07_req_ui_10_keyboard_save_keeps_then_saves_the_confirmed_value() {
    let mut rig = generated("pending-keyboard-save");
    type_unconfirmed(&mut rig, "Stepover", "1.6");
    rig.press(Key::S, Modifiers::COMMAND);
    for _ in 0..4 {
        rig.frame(Vec::new());
    }
    assert!(rig.shows("Unconfirmed changes"));
    click(&mut rig, "Keep");
    for _ in 0..5 {
        rig.frame(Vec::new());
    }
    let app = rig.shell.app.as_ref().unwrap();
    assert!(!app.is_modified());
    let reopened =
        openchuck_app::Application::open(app.location(), openchuck_app::Threads::Exactly(2))
            .unwrap();
    assert_eq!(reopened.project(), app.project());
    assert_eq!(
        app.project().operations()[0].parameters.get("stepover"),
        Some(&ParameterValue::Number(1.6))
    );
}

#[test]
fn req_ui_07_req_ui_10_real_tree_selection_waits_for_discard() {
    let mut rig = generated("pending-real-tree");
    let before = rig.shell.app.as_ref().unwrap().project().clone();
    type_unconfirmed(&mut rig, "Stepover", "1.6");
    click(&mut rig, "Setup 1 · Example X rotary");
    assert!(rig.shows("Unconfirmed changes"));
    assert!(matches!(rig.shell.selected, Some(Selection::Operation(_))));
    click(&mut rig, "Discard");
    assert_eq!(
        rig.shell.selected,
        Some(Selection::Setup(before.setups()[0].id))
    );
    assert_eq!(rig.shell.app.as_ref().unwrap().project(), &before);
}

#[test]
fn req_ui_07_req_ui_08_same_frame_field_and_choice_both_apply() {
    use crate::tool_library_window::{Chosen, Purpose, ToolLibraryWindow};
    let mut rig = Rig::cube("pending-field-choice", None);
    crate::theme::install_style(&rig.ctx, crate::theme::Theme::Dark);
    rig.ctx
        .style_mut_of(egui::Theme::Dark, |style| style.animation_time = 0.0);
    rig.size = egui::vec2(1920.0, 1080.0);
    let before = rig.shell.app.as_ref().unwrap().project().clone();
    let tool = before.tools()[0].id;
    rig.shell.tool_library = ToolLibraryWindow::new(Some(rig.dirs[0].join("library")));
    rig.shell
        .open_tool_library(Purpose::Add, Some(Chosen::Project(tool)));
    for _ in 0..6 {
        rig.frame(Vec::new());
    }
    type_unconfirmed(&mut rig, "Diameter", "5");
    click(&mut rig, "carbide");
    assert!(!rig.shows("Unconfirmed changes"));
    let changed = &rig.shell.app.as_ref().unwrap().project().tools()[0];
    assert_eq!(changed.tool.diameter.0, 5.0);
    assert_eq!(changed.tool.material.as_deref(), Some("carbide"));
    rig.shell.undo();
    rig.shell.undo();
    assert_eq!(rig.shell.app.as_ref().unwrap().project(), &before);
}

#[test]
fn req_ui_07_req_nfr_07_interacting_setup_entries_are_validated_together() {
    use openchuck_app::{
        Command,
        project::Item,
        setup_form::{self, StockShape},
    };
    let mut rig = Rig::cube("pending-interacting-setup", None);
    crate::theme::install_style(&rig.ctx, crate::theme::Theme::Dark);
    rig.size = egui::vec2(1920.0, 1400.0);
    let app = rig.shell.app.as_mut().unwrap();
    let setup = setup_form::with_stock_shape(&app.project().setups()[0], StockShape::Box, None);
    let id = setup.id;
    app.execute(Command::Replace(Item::Setup(setup))).unwrap();
    let before = app.project().clone();
    rig.shell.choose_setup(id, None);
    rig.shell.open_step(Step::Setup);
    for _ in 0..6 {
        rig.frame(Vec::new());
    }
    type_unconfirmed(&mut rig, "X from", "6");
    click(&mut rig, "X from");
    type_unconfirmed(&mut rig, "X to", "-6");
    click(&mut rig, "Operations");
    assert!(rig.shows("Unconfirmed changes"));
    click(&mut rig, "Keep");
    for _ in 0..6 {
        rig.frame(Vec::new());
    }
    assert_eq!(rig.shell.app.as_ref().unwrap().project(), &before);
    assert_eq!(rig.shell.step, Step::Setup);
    assert!(rig.shows("Must be more than"));
    assert!(rig.ctx.egui_wants_keyboard_input());
}

#[test]
fn req_ui_07_req_ui_25_keep_a_workholding_field_does_not_apply_the_draft() {
    let mut rig = Rig::cube("pending-workholding-boundary", None);
    crate::theme::install_style(&rig.ctx, crate::theme::Theme::Dark);
    rig.size = egui::vec2(1920.0, 1800.0);
    let before = rig.shell.app.as_ref().unwrap().project().clone();
    rig.shell
        .choose_setup(before.setups()[0].id, Some(super::SetupPart::Workholding));
    rig.shell.open_step(Step::Setup);
    for _ in 0..8 {
        rig.frame(Vec::new());
    }
    let held = rig
        .visible_text_rects()
        .into_iter()
        .find(|(text, _)| text == "Held at the start")
        .unwrap()
        .1;
    rig.click(egui::pos2(
        held.left() + crate::theme::width::LABEL_COLUMN + 10.0,
        held.center().y,
    ));
    for _ in 0..4 {
        rig.frame(Vec::new());
    }
    type_unconfirmed(&mut rig, "Grip length", "2");
    click(&mut rig, "Operations");
    assert!(rig.shows("Unconfirmed changes"));
    click(&mut rig, "Keep");
    rig.settle();
    assert_eq!(rig.shell.step, Step::Operations);
    assert_eq!(rig.shell.app.as_ref().unwrap().project(), &before);
    assert_eq!(
        rig.shell
            .workholding_panel
            .as_mut()
            .unwrap()
            .draft_mut()
            .start
            .unwrap()
            .length
            .0,
        2.0
    );
    assert!(rig.shell.viewport.shows_unapplied());
}

#[test]
fn req_ui_07_req_nfr_07_rename_dialog_keep_refuses_an_empty_name_inline() {
    let mut rig = Rig::cube("pending-rename-invalid", None);
    crate::theme::install_style(&rig.ctx, crate::theme::Theme::Dark);
    let before = rig.shell.app.as_ref().unwrap().project().clone();
    rig.shell
        .begin_rename(ItemRef::Operation(before.operations()[0].id));
    for _ in 0..4 {
        rig.frame(Vec::new());
    }
    rig.press(Key::A, Modifiers::COMMAND);
    rig.frame(vec![Event::Text(" ".into())]);
    click(&mut rig, "Rename");
    assert!(rig.shows("Unconfirmed changes"));
    click(&mut rig, "Keep");
    for _ in 0..6 {
        rig.frame(Vec::new());
    }
    assert!(matches!(
        rig.shell.dialog,
        Some(super::Dialog::Rename { .. })
    ));
    assert!(rig.shows("Give the item a name"));
    assert!(rig.ctx.egui_wants_keyboard_input());
    assert_eq!(rig.shell.app.as_ref().unwrap().project(), &before);
}

#[test]
fn req_ui_07_req_ui_04_deferred_pick_cannot_name_a_replaced_model_file() {
    use crate::viewport::PickInput;
    use openchuck_app::{
        Command,
        pick::ScenePick,
        project::{ContentHash, Item},
    };
    let mut rig = generated("pending-pick-identity");
    let model = rig.shell.app.as_ref().unwrap().project().models()[0].clone();
    let (mesh, item) = rig
        .shell
        .viewport
        .view()
        .meshes
        .iter()
        .enumerate()
        .find(|(_, item)| item.model == Some(model.id))
        .unwrap();
    let point = item.mesh.triangle(0).unwrap()[0];
    type_unconfirmed(&mut rig, "Stepover", "1.6");
    let ctx = rig.ctx.clone();
    rig.shell.pick_intent(
        PickInput {
            click: Some((
                ScenePick {
                    model: Some(model.id),
                    mesh,
                    triangle: 0,
                    face: None,
                    distance: 1.0,
                    point,
                    normal: None,
                },
                Modifiers::NONE,
            )),
            ..PickInput::default()
        },
        &ctx,
    );
    for _ in 0..3 {
        rig.frame(Vec::new());
    }
    assert!(rig.shows("Unconfirmed changes"));
    // Model replacement can complete while the warning is open. An old
    // triangle index must not be stamped with the replacement's hash.
    let mut replaced = model;
    replaced.hash = ContentHash::of_bytes(b"a different model file");
    rig.shell
        .app
        .as_mut()
        .unwrap()
        .execute(Command::Replace(Item::Model(replaced)))
        .unwrap();
    click(&mut rig, "Keep");
    for _ in 0..10 {
        rig.frame(Vec::new());
    }
    assert!(rig.shell.selection.working_triangles().is_empty());
}

#[test]
fn req_ui_07_req_tl_01_library_save_resolves_the_active_field_first() {
    use crate::tool_library_window::{Chosen, Purpose, ToolLibraryWindow};
    use openchuck_app::tool_library::{self, ToolType};
    use openchuck_app::tools::ToolLibrary;
    let mut rig = Rig::cube("pending-library-save", None);
    crate::theme::install_style(&rig.ctx, crate::theme::Theme::Dark);
    rig.size = egui::vec2(1920.0, 1080.0);
    let folder = rig.dirs[0].join("library");
    let mut library = ToolLibrary::open(&folder).unwrap();
    let mut draft = tool_library::new_draft(&library, ToolType::FlatEnd).unwrap();
    tool_library::edit_name(&mut draft, "Saved library tool").unwrap();
    tool_library::save_draft(&mut library, &draft, &[]).unwrap();
    let tool = draft.id;
    rig.shell.tool_library = ToolLibraryWindow::new(Some(folder.clone()));
    rig.shell
        .open_tool_library(Purpose::Add, Some(Chosen::Library(tool)));
    for _ in 0..6 {
        rig.frame(Vec::new());
    }
    type_unconfirmed(&mut rig, "Name", "Confirmed and saved");
    click(&mut rig, "Save");
    assert!(rig.shows("Unconfirmed changes"));
    assert_eq!(
        ToolLibrary::open(&folder).unwrap().get(tool).unwrap().name,
        "Saved library tool"
    );
    click(&mut rig, "Keep");
    for _ in 0..6 {
        rig.frame(Vec::new());
    }
    assert_eq!(
        ToolLibrary::open(&folder).unwrap().get(tool).unwrap().name,
        "Confirmed and saved"
    );
    assert!(rig.shell.tool_library.is_open());
}

#[test]
fn req_ui_07_req_ui_10_native_close_discard_finishes_and_keep_checks_unsaved_project() {
    for keep in [false, true] {
        let mut rig = generated(&format!("pending-close-answer-{keep}"));
        rig.shell.app.as_ref().unwrap().save().unwrap();
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
        for _ in 0..4 {
            rig.frame(Vec::new());
        }
        assert!(rig.shows("Unconfirmed changes"));
        click(&mut rig, if keep { "Keep" } else { "Discard" });
        for _ in 0..5 {
            rig.frame(Vec::new());
        }
        if keep {
            assert!(!rig.shell.quitting);
            assert!(matches!(
                rig.shell.dialog,
                Some(super::Dialog::Unsaved(super::Next::Quit))
            ));
            assert!(rig.shell.app.as_ref().unwrap().is_modified());
        } else {
            assert!(rig.shell.quitting);
            assert!(!rig.shell.app.as_ref().unwrap().is_modified());
        }
    }
}

#[test]
fn req_ui_07_req_ui_04_keep_placement_rebuilds_before_resuming_a_direction_pick() {
    use crate::viewport::PickInput;
    use openchuck_app::{
        Command,
        model::{Axis, Degrees, TransformStep},
        pick::ScenePick,
    };
    let mut rig = Rig::cube("pending-pick-placement", None);
    crate::theme::install_style(&rig.ctx, crate::theme::Theme::Dark);
    rig.size = egui::vec2(1920.0, 1400.0);
    let app = rig.shell.app.as_mut().unwrap();
    let model = app.project().models()[0].id;
    let mut transform = app.project().models()[0].transform.clone();
    transform.push(TransformStep::Rotate {
        axis: Axis::Y,
        angle: Degrees(0.0),
    });
    app.execute(Command::SetModelTransform { model, transform })
        .unwrap();
    rig.shell.rebuild_scene(false);
    rig.shell.start_picking();
    for _ in 0..4 {
        rig.frame(Vec::new());
    }
    rig.press(Key::R, Modifiers::NONE);
    assert_eq!(rig.shell.selection.choice_name(), "Same direction");
    rig.shell.select(Selection::Model(model));
    rig.shell.open_step(Step::Model);
    rig.settle();
    let (mesh, item) = rig
        .shell
        .viewport
        .view()
        .meshes
        .iter()
        .enumerate()
        .find(|(_, m)| m.model == Some(model))
        .unwrap();
    let [a, b, c] = item.mesh.triangle(0).unwrap();
    let normal = (b - a).cross(c - a);
    let normal = normal * (1.0 / normal.length());
    type_unconfirmed(&mut rig, "Angle", "90");
    let ctx = rig.ctx.clone();
    rig.shell.pick_intent(
        PickInput {
            click: Some((
                ScenePick {
                    mesh,
                    model: Some(model),
                    triangle: 0,
                    face: None,
                    distance: 1.0,
                    point: a,
                    normal: Some(normal),
                },
                Modifiers::NONE,
            )),
            ..PickInput::default()
        },
        &ctx,
    );
    for _ in 0..4 {
        rig.frame(Vec::new());
    }
    assert!(rig.shows("Unconfirmed changes"));
    click(&mut rig, "Keep");
    rig.settle();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while rig.shell.selection.has_region_pick() {
        assert!(std::time::Instant::now() < deadline);
        rig.frame(Vec::new());
        std::thread::yield_now();
    }
    let picked = rig.shell.selection.working_triangles();
    assert!(
        picked.contains(&0),
        "the intended face must survive rotation: {picked:?}; guard={}, scene={}, selected={:?}, \
         entries={:?}, status={}, texts={:?}",
        rig.shell.edit_guard.active(),
        rig.shell.scene_job.is_some(),
        rig.shell.selected,
        crate::pending_edits::entries(&rig.ctx),
        rig.shell.status,
        rig.texts()
    );
    assert_eq!(picked.len(), 2);
}

#[test]
fn req_ui_04_deferred_edge_keeps_its_identity_after_placement_and_scene_reordering() {
    use crate::viewport::PickInput;
    use openchuck_app::{Command, model::TransformStep, pick::PickTolerance};
    let (dir, mut app, _, _) = crate::testing::cube_project("pending-edge-placement");
    let view = app.start_scene(None).wait().unwrap();
    let pick = view
        .picking
        .picker()
        .unwrap()
        .pick_edge(
            [0.0, 0.0, 100.0],
            [0.0, 0.0, -1.0],
            PickTolerance {
                base: 0.1,
                per_distance: 0.0,
            },
        )
        .unwrap()
        .unwrap();
    let intent = super::pending_pick::Intent::capture(
        &app,
        &view,
        PickInput {
            edge_click: Some((pick, Modifiers::SHIFT)),
            ..PickInput::default()
        },
    )
    .unwrap();
    let model = pick.model.unwrap();
    let mut transform = app.project().models()[0].transform.clone();
    transform.push(TransformStep::Translate {
        x: 20.0,
        y: 0.0,
        z: 0.0,
    });
    app.execute(Command::SetModelTransform { model, transform })
        .unwrap();
    let mut moved = app.start_scene(None).wait().unwrap();
    // A scene can insert another item ahead of this part. Its index is
    // presentation state; the intended model and mesh edge stay the same.
    let mut extra = moved.meshes[0].clone();
    extra.model = None;
    moved.meshes.insert(0, extra);
    let (resolved, modifiers) = intent
        .resolve(&app, &moved, None)
        .unwrap()
        .edge_click
        .unwrap();
    assert_eq!(resolved.model, Some(model));
    assert_eq!(resolved.edge, pick.edge);
    assert_eq!(modifiers, Modifiers::SHIFT);
    assert_eq!(resolved.mesh, pick.mesh + 1);
    assert!(resolved.point.x >= 20.0);
    drop(app);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_ui_07_req_ui_08_setup_field_confirmation_and_shape_choice_both_apply() {
    use openchuck_app::setup::Stock;
    let mut rig = Rig::cube("pending-setup-field-choice", None);
    crate::theme::install_style(&rig.ctx, crate::theme::Theme::Dark);
    rig.size = egui::vec2(1920.0, 1400.0);
    let id = rig.shell.app.as_ref().unwrap().project().setups()[0].id;
    rig.shell.choose_setup(id, None);
    rig.shell.open_step(Step::Setup);
    for _ in 0..6 {
        rig.frame(Vec::new());
    }
    type_unconfirmed(&mut rig, "Diameter", "22");
    click(&mut rig, "Box");
    assert!(!rig.shows("Unconfirmed changes"));
    match rig.shell.app.as_ref().unwrap().project().setups()[0].stock {
        Some(Stock::Box { bounds }) => assert_eq!(bounds.max.y - bounds.min.y, 22.0),
        ref other => panic!("both the confirmed size and chosen shape must survive: {other:?}"),
    }
}

#[test]
fn req_ui_07_same_form_preset_drop_down_confirms_the_active_field() {
    let mut rig = generated("pending-preset-field-transfer");
    type_unconfirmed(&mut rig, "Stepover", "1.6");
    click(&mut rig, "No preset");
    assert!(!rig.shows("Unconfirmed changes"));
    assert_eq!(
        rig.shell.app.as_ref().unwrap().project().operations()[0]
            .parameters
            .get("stepover"),
        Some(&ParameterValue::Number(1.6))
    );
}

#[test]
fn req_ui_07_req_ui_25_deferred_pick_can_select_an_unapplied_held_region() {
    use crate::viewport::PickInput;
    use openchuck_app::scene::WorkholdingItem;
    let mut rig = Rig::cube("pending-draft-solid-pick", None);
    crate::theme::install_style(&rig.ctx, crate::theme::Theme::Dark);
    rig.size = egui::vec2(1920.0, 1800.0);
    let before = rig.shell.app.as_ref().unwrap().project().clone();
    rig.shell
        .choose_setup(before.setups()[0].id, Some(super::SetupPart::Workholding));
    rig.shell.open_step(Step::Setup);
    rig.settle();
    for _ in 0..6 {
        rig.frame(Vec::new());
    }
    let held = rig
        .visible_text_rects()
        .into_iter()
        .find(|(text, _)| text == "Held at the start")
        .unwrap()
        .1;
    rig.click(egui::pos2(
        held.left() + crate::theme::width::LABEL_COLUMN + 10.0,
        held.center().y,
    ));
    for _ in 0..4 {
        rig.frame(Vec::new());
    }
    type_unconfirmed(&mut rig, "Grip length", "2");
    let ctx = rig.ctx.clone();
    rig.shell.pick_intent(
        PickInput {
            solid_click: Some(WorkholdingItem::Start),
            ..PickInput::default()
        },
        &ctx,
    );
    for _ in 0..4 {
        rig.frame(Vec::new());
    }
    assert!(rig.shows("Unconfirmed changes"));
    click(&mut rig, "Keep");
    rig.settle();
    assert_eq!(
        rig.shell.viewport.selected_solid(),
        Some(WorkholdingItem::Start)
    );
    assert_eq!(rig.shell.app.as_ref().unwrap().project(), &before);
    assert!(rig.shell.viewport.shows_unapplied());
}

#[test]
fn req_ui_07_discard_uses_this_dialogs_original_after_an_earlier_edit_was_reverted() {
    let mut rig = Rig::cube("pending-dialog-reused", None);
    crate::theme::install_style(&rig.ctx, crate::theme::Theme::Dark);
    let before = rig.shell.app.as_ref().unwrap().project().clone();
    rig.shell
        .begin_rename(ItemRef::Operation(before.operations()[0].id));
    for _ in 0..4 {
        rig.frame(Vec::new());
    }
    rig.press(Key::A, Modifiers::COMMAND);
    rig.frame(vec![Event::Text("Temporary name".into())]);
    rig.press(Key::A, Modifiers::COMMAND);
    rig.frame(vec![Event::Text(before.operations()[0].name.clone())]);
    click(&mut rig, "Cancel");
    assert!(rig.shell.dialog.is_none());
    assert!(!rig.shows("Unconfirmed changes"));
    rig.shell
        .begin_rename(ItemRef::Setup(before.setups()[0].id));
    for _ in 0..4 {
        rig.frame(Vec::new());
    }
    rig.press(Key::A, Modifiers::COMMAND);
    rig.frame(vec![Event::Text("Another name".into())]);
    click(&mut rig, "Rename");
    assert!(rig.shows("Unconfirmed changes"));
    click(&mut rig, "Discard");
    for _ in 0..4 {
        rig.frame(Vec::new());
    }
    assert_eq!(rig.shell.app.as_ref().unwrap().project(), &before);
}

#[test]
fn req_ui_07_project_folder_browse_resolves_pending_text_before_the_native_dialog() {
    for keep in [false, true] {
        let mut rig = Rig::new(super::Shell::with_settings(false, None, None), Vec::new());
        super::PICKER_INPUTS.with(|inputs| *inputs.borrow_mut() = Some(Vec::new()));
        rig.shell.open_dialog(super::Dialog::Folder {
            create: true,
            folder: "/tmp".into(),
            name: "Untitled".into(),
        });
        for _ in 0..4 {
            rig.frame(Vec::new());
        }
        let at = rig
            .visible_text_rects()
            .into_iter()
            .find(|(text, _)| text == "/tmp")
            .unwrap()
            .1
            .center();
        rig.click(at);
        rig.press(Key::A, Modifiers::COMMAND);
        rig.frame(vec![Event::Text("/tmp/another-parent".into())]);
        click(&mut rig, "Browse…");
        super::PICKER_INPUTS.with(|inputs| {
            assert!(
                inputs.borrow().as_ref().unwrap().is_empty(),
                "the native picker must wait for confirmation"
            )
        });
        assert!(rig.shows("Unconfirmed changes"));
        click(&mut rig, if keep { "Keep" } else { "Discard" });
        for _ in 0..5 {
            rig.frame(Vec::new());
        }
        super::PICKER_INPUTS.with(|inputs| {
            assert_eq!(
                inputs.borrow().as_ref().unwrap(),
                &[if keep { "/tmp/another-parent" } else { "/tmp" }]
            )
        });
        super::PICKER_INPUTS.with(|inputs| *inputs.borrow_mut() = None);
    }
}

#[test]
fn req_ui_07_model_replacement_waits_for_the_pending_placement_field() {
    use openchuck_app::{
        Command,
        model::{Axis, Degrees, TransformStep},
    };
    let mut rig = Rig::cube("pending-replacement-picker", None);
    crate::theme::install_style(&rig.ctx, crate::theme::Theme::Dark);
    rig.size = egui::vec2(1920.0, 1800.0);
    let app = rig.shell.app.as_mut().unwrap();
    let model = app.project().models()[0].id;
    let mut transform = app.project().models()[0].transform.clone();
    transform.push(TransformStep::Rotate {
        axis: Axis::Y,
        angle: Degrees(0.0),
    });
    app.execute(Command::SetModelTransform { model, transform })
        .unwrap();
    rig.shell.select(Selection::Model(model));
    rig.shell.open_step(Step::Model);
    rig.settle();
    click(&mut rig, "SOURCE");
    type_unconfirmed(&mut rig, "Angle", "90");
    super::PICKER_INPUTS.with(|inputs| *inputs.borrow_mut() = Some(Vec::new()));
    click(&mut rig, "Replace file…");
    super::PICKER_INPUTS.with(|inputs| {
        assert!(
            inputs.borrow().as_ref().unwrap().is_empty(),
            "replacement must wait for confirmation"
        )
    });
    assert!(rig.shows("Unconfirmed changes"));
    click(&mut rig, "Keep");
    rig.settle();
    super::PICKER_INPUTS
        .with(|inputs| assert_eq!(inputs.borrow().as_ref().unwrap(), &["models/cube.stl"]));
    super::PICKER_INPUTS.with(|inputs| *inputs.borrow_mut() = None);
    assert!(
        matches!(rig.shell.app.as_ref().unwrap().project().models()[0].transform.last(), Some(TransformStep::Rotate { angle, .. }) if angle.0 == 90.0)
    );
}

#[test]
fn req_ui_07_model_report_waits_for_pending_placement_confirmation() {
    use openchuck_app::{
        Command,
        model::{Axis, Degrees, TransformStep},
    };
    let mut rig = Rig::cube("pending-model-report", None);
    crate::theme::install_style(&rig.ctx, crate::theme::Theme::Dark);
    rig.size = egui::vec2(1920.0, 1800.0);
    let app = rig.shell.app.as_mut().unwrap();
    let model = app.project().models()[0].id;
    let mut transform = app.project().models()[0].transform.clone();
    transform.push(TransformStep::Rotate {
        axis: Axis::Y,
        angle: Degrees(0.0),
    });
    app.execute(Command::SetModelTransform { model, transform })
        .unwrap();
    rig.shell.select(Selection::Model(model));
    rig.shell.open_step(Step::Model);
    rig.settle();
    for _ in 0..6 {
        rig.frame(Vec::new());
    }
    type_unconfirmed(&mut rig, "Angle", "90");
    click(&mut rig, "Open full report");
    assert!(rig.shows("Unconfirmed changes"));
    assert!(!rig.shows("Mesh check report"));
    click(&mut rig, "Keep");
    rig.settle();
    for _ in 0..20 {
        rig.frame(Vec::new());
    }
    assert!(rig.shows("Mesh check report"));
    assert!(
        matches!(rig.shell.app.as_ref().unwrap().project().models()[0].transform.last(), Some(TransformStep::Rotate { angle, .. }) if angle.0 == 90.0)
    );
}

#[test]
fn req_ui_07_req_ui_08_new_operation_add_resumes_only_after_field_resolution() {
    for answer in ["Keep", "Discard", "Cancel"] {
        let mut rig = Rig::cube(&format!("pending-local-add-{answer}"), None);
        crate::theme::install_style(&rig.ctx, crate::theme::Theme::Dark);
        rig.size = egui::vec2(1920.0, 1400.0);
        let before = rig.shell.app.as_ref().unwrap().project().clone();
        rig.shell.new_operation();
        for _ in 0..6 {
            rig.frame(Vec::new());
        }
        let name_label = rig
            .visible_text_rects()
            .into_iter()
            .find(|(text, _)| text == "Name")
            .unwrap()
            .1;
        let original_name = rig
            .visible_text_rects()
            .into_iter()
            .filter(|(_, rect)| {
                rect.left() > name_label.right()
                    && (rect.center().y - name_label.center().y).abs() < 3.0
            })
            .min_by(|(_, a), (_, b)| a.left().total_cmp(&b.left()))
            .unwrap()
            .0;
        type_unconfirmed(&mut rig, "Name", "New confirmed operation");
        click(&mut rig, "Add");
        assert!(rig.shows("Unconfirmed changes"));
        assert_eq!(rig.shell.app.as_ref().unwrap().project(), &before);
        click(&mut rig, answer);
        for _ in 0..6 {
            rig.frame(Vec::new());
        }
        let project = rig.shell.app.as_ref().unwrap().project();
        if answer == "Cancel" {
            assert_eq!(project, &before);
            assert!(rig.ctx.egui_wants_keyboard_input());
        } else {
            assert_eq!(project.operations().len(), before.operations().len() + 1);
            assert_eq!(
                project.operations().last().unwrap().name,
                if answer == "Keep" {
                    "New confirmed operation"
                } else {
                    &original_name
                }
            );
            rig.shell.undo();
            assert_eq!(rig.shell.app.as_ref().unwrap().project(), &before);
        }
    }
}

#[test]
fn req_ui_07_req_ui_08_req_ui_25_explicit_apply_resumes_after_keeping_the_workholding_field() {
    let mut rig = Rig::cube("pending-local-workholding-apply", None);
    crate::theme::install_style(&rig.ctx, crate::theme::Theme::Dark);
    rig.size = egui::vec2(1920.0, 1800.0);
    let before = rig.shell.app.as_ref().unwrap().project().clone();
    let setup = before.setups()[0].id;
    rig.shell
        .choose_setup(setup, Some(super::SetupPart::Workholding));
    rig.shell.open_step(Step::Setup);
    rig.settle();
    for _ in 0..6 {
        rig.frame(Vec::new());
    }
    let held = rig
        .visible_text_rects()
        .into_iter()
        .find(|(text, _)| text == "Held at the start")
        .unwrap()
        .1;
    rig.click(egui::pos2(
        held.left() + crate::theme::width::LABEL_COLUMN + 10.0,
        held.center().y,
    ));
    for _ in 0..4 {
        rig.frame(Vec::new());
    }
    type_unconfirmed(&mut rig, "Grip length", "2");
    click(&mut rig, "Apply");
    assert!(rig.shows("Unconfirmed changes"));
    assert_eq!(rig.shell.app.as_ref().unwrap().project(), &before);
    click(&mut rig, "Keep");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while rig
        .shell
        .app
        .as_ref()
        .unwrap()
        .project()
        .setup(setup)
        .unwrap()
        .workholding
        .start
        .is_none()
    {
        assert!(
            std::time::Instant::now() < deadline,
            "Apply did not finish: {:?}",
            rig.texts()
        );
        rig.frame(Vec::new());
        std::thread::yield_now();
    }
    assert_eq!(
        rig.shell
            .app
            .as_ref()
            .unwrap()
            .project()
            .setup(setup)
            .unwrap()
            .workholding
            .start
            .unwrap()
            .length
            .0,
        2.0
    );
    for _ in 0..4 {
        rig.frame(Vec::new());
    }
    rig.shell.undo();
    assert_eq!(rig.shell.app.as_ref().unwrap().project(), &before);
}

#[test]
fn req_ui_07_req_ui_08_inspector_header_name_obeys_keep_discard_cancel_and_validation() {
    for answer in ["Keep", "Discard", "Cancel", "Invalid"] {
        let mut rig = generated(&format!("pending-header-{answer}"));
        let before = rig.shell.app.as_ref().unwrap().project().clone();
        let op = before.operations()[0].id;
        let at = rig
            .visible_text_rects()
            .into_iter()
            .find(|(text, _)| text == &before.operations()[0].name)
            .unwrap()
            .1
            .center();
        rig.click(at);
        rig.press(Key::A, Modifiers::COMMAND);
        rig.frame(vec![Event::Text(
            if answer == "Invalid" {
                " "
            } else {
                "Pending name"
            }
            .into(),
        )]);
        click(&mut rig, "Setup");
        assert!(rig.shows("Unconfirmed changes"));
        assert_eq!(rig.shell.app.as_ref().unwrap().project(), &before);
        click(&mut rig, if answer == "Invalid" { "Keep" } else { answer });
        for _ in 0..5 {
            rig.frame(Vec::new());
        }
        if answer == "Keep" {
            assert_eq!(
                rig.shell
                    .app
                    .as_ref()
                    .unwrap()
                    .project()
                    .operation(op)
                    .unwrap()
                    .name,
                "Pending name"
            );
            assert_eq!(rig.shell.step, Step::Setup);
            assert_eq!(
                rig.shell
                    .overview
                    .status(ItemRef::Operation(op))
                    .unwrap()
                    .state,
                ItemState::UpToDate
            );
            rig.shell.undo();
            assert_eq!(rig.shell.app.as_ref().unwrap().project(), &before);
        } else {
            assert_eq!(rig.shell.app.as_ref().unwrap().project(), &before);
            if answer == "Discard" {
                assert_eq!(rig.shell.step, Step::Setup);
            } else {
                assert_eq!(rig.shell.step, Step::Operations);
                assert!(rig.ctx.egui_wants_keyboard_input());
                assert!(rig.shows(if answer == "Invalid" {
                    "Give the item a name"
                } else {
                    "Pending name"
                }));
            }
        }
    }
}

#[test]
fn req_ui_07_req_tl_01_req_tl_07_cleanup_guards_pending_library_fields_without_saving() {
    use crate::tool_library_window::{Chosen, Purpose, ToolLibraryWindow};
    use openchuck_app::tool_library::{self, ToolType};
    use openchuck_app::tools::ToolLibrary;
    for answer in ["Keep", "Discard", "Cancel"] {
        let mut rig = Rig::cube(&format!("cleanup-pending-{answer}"), None);
        let folder = rig.dirs[0].join("library");
        let mut library = ToolLibrary::open(&folder).unwrap();
        let mut draft = tool_library::new_draft(&library, ToolType::FlatEnd).unwrap();
        tool_library::edit_name(&mut draft, "Library original").unwrap();
        tool_library::save_draft(&mut library, &draft, &[]).unwrap();
        let id = draft.id;
        rig.shell.tool_library = ToolLibraryWindow::new(Some(folder.clone()));
        rig.shell
            .open_tool_library(Purpose::Browse, Some(Chosen::Library(id)));
        for _ in 0..4 {
            rig.frame(Vec::new());
        }
        let before = rig.shell.app.as_ref().unwrap().project().clone();
        type_unconfirmed(&mut rig, "Name", "Pending library name");
        click(&mut rig, "Remove unused tools…");
        assert!(rig.shows("Unconfirmed changes"));
        click(&mut rig, answer);
        for _ in 0..4 {
            rig.frame(Vec::new());
        }
        assert_eq!(
            ToolLibrary::open(&folder).unwrap().get(id).unwrap().name,
            "Library original"
        );
        assert_eq!(rig.shell.app.as_ref().unwrap().project(), &before);
        assert_eq!(rig.shows("No unused project tools."), answer != "Cancel");
        if answer == "Cancel" {
            assert!(!crate::pending_edits::entries(&rig.ctx).is_empty());
        } else {
            click(&mut rig, "Close");
            assert!(rig.shows(if answer == "Keep" {
                "Pending library name"
            } else {
                "Library original"
            }));
        }
    }
}
