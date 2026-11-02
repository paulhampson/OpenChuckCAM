// SPDX-License-Identifier: AGPL-3.0-or-later
//! OPCREATE2: the creation choice and its explanation belong together.
#![allow(clippy::unwrap_used)]

use super::frame_tests::Rig;
use eframe::egui::{self, Key, Modifiers};

fn open(name: &str) -> Rig {
    let mut rig = Rig::cube(name, None);
    crate::theme::install_style(&rig.ctx, crate::theme::Theme::Dark);
    rig.ctx
        .style_mut_of(egui::Theme::Dark, |s| s.animation_time = 0.0);
    rig.shell.new_operation();
    for _ in 0..4 {
        rig.frame(Vec::new());
    }
    rig
}

fn click(rig: &mut Rig, label: &str) {
    let rect = rig
        .visible_text_rects()
        .into_iter()
        .rev()
        .find(|(t, _)| t == label)
        .unwrap_or_else(|| panic!("{label}: {:?}", rig.texts()))
        .1;
    rig.click(rect.center());
    for _ in 0..3 {
        rig.frame(Vec::new());
    }
}

#[test]
fn req_op_16_req_ui_07_picker_shows_only_the_selected_explanation() {
    let mut rig = open("picker-selected-information");
    let choices = rig.shell.app.as_ref().unwrap().strategy_choices();
    assert!(rig.shows(&choices[0].description));
    for other in &choices[1..] {
        assert!(
            !rig.shows(&other.description),
            "closed dropdown must not show {}'s explanation",
            other.label
        );
    }
    let before = rig.shell.app.as_ref().unwrap().project().clone();
    // The closed dropdown is above the information panel's duplicate name.
    let rect = rig
        .visible_text_rects()
        .into_iter()
        .find(|(t, _)| t == &choices[0].label)
        .unwrap()
        .1;
    rig.click(rect.center());
    for _ in 0..3 {
        rig.frame(Vec::new());
    }
    assert!(rig.shows("Finishing"));
    click(&mut rig, "Helical finishing");
    let helical = choices
        .iter()
        .find(|c| c.name == "helical_finishing")
        .unwrap();
    assert!(rig.shows(&helical.description));
    assert!(!rig.shows(&choices[0].description));
    rig.frame(Vec::new());
    assert!(
        rig.shows(&helical.description),
        "selection survives repaint"
    );
    assert_eq!(rig.shell.app.as_ref().unwrap().project(), &before);
}

#[test]
fn req_op_16_req_ui_14_picker_lists_disabled_strategies_with_readable_reasons() {
    let mut rig = open("picker-disabled-reasons");
    let choices = rig.shell.app.as_ref().unwrap().strategy_choices_for(None);
    let selected = choices[0].label.clone();
    let rect = rig
        .visible_text_rects()
        .into_iter()
        .find(|(t, _)| t == &selected)
        .unwrap()
        .1;
    rig.click(rect.center());
    for _ in 0..3 {
        rig.frame(Vec::new());
    }
    let setup = rig.shell.app.as_ref().unwrap().project().setups()[0].id;
    let disabled: Vec<_> = rig
        .shell
        .app
        .as_ref()
        .unwrap()
        .strategy_choices_for(Some(setup))
        .into_iter()
        .filter(|c| c.unavailable.is_some())
        .collect();
    assert!(!disabled.is_empty());
    for c in disabled {
        if !rig.shows(c.unavailable.as_ref().unwrap()) {
            let at = rig
                .visible_text_rects()
                .into_iter()
                .rev()
                .find(|(t, _)| t.contains("Swarf"))
                .unwrap()
                .1
                .center();
            rig.frame(vec![
                egui::Event::PointerMoved(at),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    source: egui::MouseWheelSource::Unknown,
                    phase: egui::TouchPhase::Move,
                    delta: egui::vec2(0.0, -500.0),
                    modifiers: Modifiers::NONE,
                },
            ]);
            for _ in 0..5 {
                rig.frame(Vec::new());
            }
        }
        assert!(
            rig.shows(c.unavailable.as_ref().unwrap()),
            "reason for {} must be readable without hover",
            c.label
        );
    }
    rig.press(Key::Escape, Modifiers::NONE);
    assert!(
        rig.shell.run.is_adding(),
        "Escape in popup must leave creation open"
    );
}

#[test]
fn req_ui_16_req_nfr_15_picker_dialog_keeps_its_controls_inside_without_overlap() {
    let mut rig = open("picker-whole-dialog");
    for (w, h, scale) in [
        (1366.0, 768.0, 1.0),
        (1366.0, 768.0, 1.2),
        (1920.0, 1080.0, 1.0),
        (1920.0, 1080.0, 1.2),
        (1366.0, 768.0, 0.8),
        (1366.0, 768.0, 2.0),
    ] {
        rig.size = egui::vec2(w / scale, h / scale);
        rig.ctx.set_pixels_per_point(scale);
        for _ in 0..5 {
            rig.frame(Vec::new());
        }
        let dialog = rig
            .ctx
            .memory(|m| m.area_rect(egui::Id::unique(("dialog", "add operation"))))
            .expect("bounded creation dialog");
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, rig.size);
        assert!(
            screen.expand(0.5).contains_rect(dialog),
            "{w}x{h}/{scale}: {dialog:?}"
        );
        let painted = rig.painted();
        let first = painted
            .iter()
            .rposition(|(text, _, _)| text.as_deref() == Some("Name"))
            .unwrap();
        let texts: Vec<_> = painted[first..]
            .iter()
            .filter(|(text, _, clipped)| text.is_some() && dialog.contains_rect(*clipped))
            .collect();
        for required in ["Add", "Cancel"] {
            let (_, full, visible) = texts
                .iter()
                .find(|(t, _, _)| t.as_deref() == Some(required))
                .unwrap();
            assert!(
                visible.expand(0.5).contains_rect(*full),
                "{required} clipped at {scale}"
            );
        }
        if scale <= 1.2 {
            for required in [
                "Name",
                "Strategy",
                "Setup",
                "Tool",
                "From the library…",
                "Material preset",
            ] {
                let (_, full, visible) = texts
                    .iter()
                    .find(|(t, _, _)| t.as_deref() == Some(required))
                    .unwrap_or_else(|| panic!("{w}x{h}/{scale}: {required} is visible"));
                assert!(
                    visible.expand(0.5).contains_rect(*full),
                    "{required} clipped at {scale}"
                );
            }
        }
        for (i, (a, _, ra)) in texts.iter().enumerate() {
            for (b, _, rb) in texts.iter().skip(i + 1) {
                let overlap = ra.intersect(*rb);
                assert!(
                    overlap.width() < 1.0 || overlap.height() < 1.0,
                    "{scale}: {a:?} overlaps {b:?}"
                );
            }
        }
    }
}

#[test]
fn req_ui_14_req_ui_07_picker_can_be_opened_and_chosen_with_the_keyboard() {
    let mut rig = open("picker-keyboard");
    click(&mut rig, "Operation 2");
    rig.press(Key::Tab, Modifiers::NONE);
    rig.press(Key::Space, Modifiers::NONE);
    for _ in 0..3 {
        rig.frame(Vec::new());
    }
    assert!(
        rig.shows("Finishing"),
        "Space opens the focused dropdown: {:?}",
        rig.texts()
    );
    // The popup's scroll area is not a Tab stop (egui 0.37 senses drag only
    // there), so the third choice is three Tabs away; with egui 0.36 it was four.
    for _ in 0..3 {
        rig.press(Key::Tab, Modifiers::NONE);
    }
    rig.press(Key::Enter, Modifiers::NONE);
    for _ in 0..3 {
        rig.frame(Vec::new());
    }
    let text = rig
        .shell
        .app
        .as_ref()
        .unwrap()
        .strategy_description("helical_finishing");
    assert!(
        rig.shows(&text),
        "keyboard chooses helical: {:?}",
        rig.texts()
    );
    assert!(!rig.shows("Finishing"), "Enter closes the popup");
    let selected = rig
        .visible_text_rects()
        .into_iter()
        .find(|(s, _)| s == "Helical finishing")
        .unwrap()
        .1;
    let focused = rig
        .ctx
        .memory(|m| m.focused())
        .and_then(|id| rig.ctx.read_response(id));
    assert!(
        focused.is_some_and(|r| r.rect.contains_rect(selected)),
        "selection returns focus to the dropdown"
    );
    assert!(!rig.shows("Unconfirmed changes"));
    rig.press(Key::Space, Modifiers::NONE);
    for _ in 0..3 {
        rig.frame(Vec::new());
    }
    assert!(rig.shows("Finishing"));
    rig.press(Key::Tab, Modifiers::NONE);
    rig.press(Key::Tab, Modifiers::NONE);
    rig.press(Key::Escape, Modifiers::NONE);
    for _ in 0..3 {
        rig.frame(Vec::new());
    }
    assert!(!rig.shows("Finishing"));
    assert!(rig.shell.run.is_adding());
    let focused = rig
        .ctx
        .memory(|m| m.focused())
        .and_then(|id| rig.ctx.read_response(id));
    assert!(
        focused.is_some_and(|r| r.rect.contains_rect(selected)),
        "Escape returns focus to the dropdown"
    );
}

#[test]
fn req_ui_07_req_ui_08_picker_departure_keeps_the_shared_pending_edit_guard() {
    for answer in ["Keep", "Discard", "Cancel"] {
        let mut rig = open(&format!("picker-close-{answer}"));
        let before = rig.shell.app.as_ref().unwrap().project().clone();
        click(&mut rig, "Operation 2");
        rig.press(Key::A, Modifiers::COMMAND);
        rig.frame(vec![egui::Event::Text("Unconfirmed operation".into())]);
        assert!(!crate::pending_edits::entries(&rig.ctx).is_empty());
        click(&mut rig, "Cancel");
        assert!(rig.shows("Unconfirmed changes"));
        click(&mut rig, answer);
        assert_eq!(rig.shell.app.as_ref().unwrap().project(), &before);
        assert_eq!(rig.shell.run.is_adding(), answer == "Cancel");
        if answer == "Cancel" {
            assert!(
                rig.ctx.egui_wants_keyboard_input(),
                "cancel restores field focus"
            );
            assert!(rig.shows("Unconfirmed operation"));
        }
    }
}

#[test]
fn req_gc_07_req_nfr_06_viewing_strategy_information_preserves_parameters_paths_and_gcode() {
    let mut rig = Rig::cube("picker-output-invariance", None);
    rig.shell
        .app
        .as_mut()
        .unwrap()
        .set_post(Some(openchuck_app::PostSelection::parse("linuxcnc")));
    let before = rig.shell.app.as_ref().unwrap().regenerate().unwrap();
    assert!(!before.gcode.is_empty());
    rig.shell.new_operation();
    for _ in 0..4 {
        rig.frame(Vec::new());
    }
    let choices = rig.shell.app.as_ref().unwrap().strategy_choices();
    let selected = rig
        .visible_text_rects()
        .into_iter()
        .find(|(s, _)| s == &choices[0].label)
        .unwrap()
        .1;
    rig.click(selected.center());
    for _ in 0..3 {
        rig.frame(Vec::new());
    }
    click(&mut rig, "Helical finishing");
    click(&mut rig, "Cancel");
    let op = before.project.operations()[0].id;
    rig.shell.select(super::Selection::Operation(op));
    for _ in 0..3 {
        rig.frame(Vec::new());
    }
    let after = rig.shell.app.as_ref().unwrap().regenerate().unwrap();
    assert_eq!(after.project, before.project);
    assert_eq!(after.toolpath, before.toolpath);
    assert_eq!(after.gcode, before.gcode);
}

#[test]
fn req_ui_07_req_ui_08_strategy_choice_confirms_a_name_in_the_same_creation_form() {
    let mut rig = open("picker-same-form-name");
    click(&mut rig, "Operation 2");
    rig.press(Key::A, Modifiers::COMMAND);
    rig.frame(vec![egui::Event::Text("Chosen finish".into())]);
    let dropdown = rig
        .visible_text_rects()
        .into_iter()
        .find(|(t, _)| t == "Rotary roughing")
        .unwrap()
        .1;
    rig.click(dropdown.center());
    for _ in 0..3 {
        rig.frame(Vec::new());
    }
    click(&mut rig, "Helical finishing");
    assert!(
        !rig.shows("Unconfirmed changes"),
        "field-to-field confirmation does not ask"
    );
    click(&mut rig, "Add");
    assert!(!rig.shows("Unconfirmed changes"));
    let operation = rig
        .shell
        .app
        .as_ref()
        .unwrap()
        .project()
        .operations()
        .last()
        .unwrap();
    assert_eq!(operation.name, "Chosen finish");
    assert_eq!(operation.strategy, "helical_finishing");
}

#[test]
fn req_ui_07_req_tl_01_creation_library_handoff_resolves_the_name_first() {
    for answer in ["Keep", "Discard", "Cancel"] {
        let mut rig = open(&format!("picker-library-{answer}"));
        click(&mut rig, "Operation 2");
        rig.press(Key::A, Modifiers::COMMAND);
        rig.frame(vec![egui::Event::Text("Draft operation".into())]);
        click(&mut rig, "From the library…");
        assert!(rig.shows("Unconfirmed changes"));
        assert!(!rig.shell.tool_library.is_open());
        click(&mut rig, answer);
        assert_eq!(rig.shell.tool_library.is_open(), answer != "Cancel");
        assert!(
            rig.shell.run.is_adding(),
            "the creation draft survives the library handoff"
        );
        if answer == "Cancel" {
            assert!(rig.ctx.egui_wants_keyboard_input());
        }
    }
}

#[test]
fn req_ui_07_creation_choices_keep_normal_field_to_field_confirmation() {
    for (tag, choice) in [
        ("setup", "Setup 1"),
        ("tool", "4 mm ball"),
        ("preset", "Setup's stock material: oak"),
    ] {
        let mut rig = open(&format!("picker-creation-choice-{tag}"));
        click(&mut rig, "Operation 2");
        rig.press(Key::A, Modifiers::COMMAND);
        rig.frame(vec![egui::Event::Text("Confirmed creation name".into())]);
        click(&mut rig, choice);
        click(&mut rig, choice);
        click(&mut rig, "Add");
        assert!(
            !rig.shows("Unconfirmed changes"),
            "{tag} stays in the creation form"
        );
        assert_eq!(
            rig.shell
                .app
                .as_ref()
                .unwrap()
                .project()
                .operations()
                .last()
                .unwrap()
                .name,
            "Confirmed creation name"
        );
    }
}
