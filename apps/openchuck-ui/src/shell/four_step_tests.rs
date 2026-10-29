// SPDX-License-Identifier: AGPL-3.0-or-later
//! Four steps, and Post and export as an action of its own (UI-10, UI-11,
//! UI-12, UI-14, UI-16; decision record 0282, issue 322). Simulate has two
//! footer actions in fixed places, the simulation action then "Post and
//! export…" right-most, with exactly one filled; the step bar says whether
//! G-code was written from the project as it is now.

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use eframe::egui::{self, Key, Modifiers};
use openchuck_app::overview::{PrimaryKind, Step};
use openchuck_app::project::ParameterValue;

use super::Dialog;
use super::frame_tests::Rig;
use super::pending_edit_tests::{generated, type_unconfirmed};
use crate::theme::DARK;

/// The cube project generated and simulated, open on Simulate.
fn on_simulate(name: &str, size: egui::Vec2) -> Rig {
    let mut rig = Rig::cube(name, None);
    rig.size = size;
    rig.shell.generate(true);
    rig.settle();
    rig.press(Key::Num4, Modifiers::ALT);
    assert_eq!(rig.shell.step, Step::Simulate);
    for _ in 0..3 {
        rig.frame(Vec::new());
    }
    rig
}

/// The visible rectangle of the footer text `label`, the lowest one drawn.
fn footer_text(rig: &Rig, label: &str) -> egui::Rect {
    rig.visible_text_rects()
        .into_iter()
        .filter(|(text, _)| text == label)
        .map(|(_, rect)| rect)
        .max_by(|a, b| a.top().total_cmp(&b.top()))
        .unwrap_or_else(|| panic!("{label} is drawn: {:?}", rig.texts()))
}

/// The visible rectangle of the step-bar text `label`, the highest one drawn.
fn bar_text(rig: &Rig, label: &str) -> egui::Rect {
    rig.visible_text_rects()
        .into_iter()
        .filter(|(text, _)| text == label)
        .map(|(_, rect)| rect)
        .min_by(|a, b| a.top().total_cmp(&b.top()))
        .unwrap_or_else(|| panic!("{label} is drawn: {:?}", rig.texts()))
}

/// The visible rectangle of `label` painted last: a modal dialog's button.
fn last_text(rig: &Rig, label: &str) -> egui::Rect {
    rig.visible_text_rects()
        .into_iter()
        .rev()
        .find(|(text, _)| text == label)
        .map(|(_, rect)| rect)
        .unwrap_or_else(|| panic!("{label} is drawn: {:?}", rig.texts()))
}

/// Whether a rectangle filled in the primary colour lies under `label`.
fn filled(rig: &Rig, label: egui::Rect) -> bool {
    fn walk(shape: &egui::Shape, label: egui::Rect, found: &mut bool) {
        match shape {
            egui::Shape::Rect(r) if r.fill == DARK.primary && r.rect.contains_rect(label) => {
                *found = true;
            }
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| walk(s, label, found)),
            _ => {}
        }
    }
    let mut found = false;
    rig.shapes
        .iter()
        .for_each(|s| walk(&s.shape, label, &mut found));
    found
}

/// The stepover of the project's operation set to `value`, and the overview
/// worked out again.
fn set_stepover(rig: &mut Rig, value: f64) {
    let op = rig.shell.app.as_ref().unwrap().project().operations()[0].id;
    rig.shell
        .app
        .as_mut()
        .unwrap()
        .set_parameter(op, "stepover", Some(ParameterValue::Number(value)))
        .unwrap();
    rig.settle();
}

#[test]
fn req_ui_10_req_ui_14_simulate_has_two_actions_in_fixed_places_with_one_filled() {
    let mut rig = on_simulate("four-two-actions", egui::vec2(1440.0, 900.0));
    // Simulated: Post and export is the next step, so it is the filled one,
    // and the simulation action stays in its place, disabled with its reason.
    let export = footer_text(&rig, "Post and export…");
    let simulation = footer_text(&rig, "Run simulation");
    assert!(
        simulation.right() < export.left(),
        "simulation, then export"
    );
    assert!((simulation.center().y - export.center().y).abs() < 2.0);
    assert!(
        export.right() > rig.size.x - 40.0,
        "Post and export is right-most"
    );
    assert!(filled(&rig, export));
    assert!(!filled(&rig, simulation));
    assert_eq!(
        rig.shell.overview.simulation_action().blocked.as_deref(),
        Some("The simulation matches the project")
    );

    // An edit: the simulation action is filled and Post and export stays,
    // plain and in the same place (never one button that changes).
    set_stepover(&mut rig, 0.5);
    assert_eq!(
        rig.shell.overview.primary(Step::Simulate).kind,
        PrimaryKind::GenerateAndSimulate(1)
    );
    let after = footer_text(&rig, "Post and export…");
    let simulation = footer_text(&rig, "Generate and simulate op 1");
    // The button's place is fixed from the right edge; its label is
    // centred in it, and bold and plain words differ by part of a pixel.
    assert!(
        (after.right() - export.right()).abs() <= 1.5,
        "it did not move: {export:?} then {after:?}"
    );
    assert!(simulation.right() < after.left());
    assert!(filled(&rig, simulation));
    assert!(!filled(&rig, after));
    // Its rules are the fifth step's: a stale run does not disable it, as
    // the dialog blocks writing from one (GC-15; decision 0282 item 3).
    assert!(rig.shell.overview.export_action().blocked.is_none());

    // Each is reachable on its own: a click on Post and export opens the
    // dialog without simulating.
    let run = rig.shell.run.last().cloned();
    rig.click(after.center());
    assert!(rig.shell.export.is_open());
    assert!(!rig.shell.run.is_running());
    assert!(std::sync::Arc::ptr_eq(
        rig.shell.run.last().unwrap(),
        run.as_ref().unwrap()
    ));
    rig.shell.export.clear();
    rig.frame(Vec::new());
    // And a click on the simulation action runs the simulation.
    let simulation = footer_text(&rig, "Generate and simulate op 1");
    rig.click(simulation.center());
    assert!(!rig.shell.export.is_open());
    rig.settle();
    assert_eq!(
        rig.shell.overview.primary(Step::Simulate).kind,
        PrimaryKind::OpenExport
    );
}

#[test]
fn req_ui_14_tab_reaches_the_simulate_actions_in_reading_order_and_enter_opens_the_dialog() {
    let mut rig = on_simulate("four-keyboard", egui::vec2(1440.0, 900.0));
    let simulation = footer_text(&rig, "Run simulation");
    let export = footer_text(&rig, "Post and export…");
    let focused = |rig: &Rig| {
        rig.ctx
            .memory(|m| m.focused())
            .and_then(|id| rig.ctx.read_response(id))
            .map(|r| r.rect)
    };
    // The simulation action is disabled (the simulation matches), so Tab
    // passes over it; with a stale run it is reached first.
    set_stepover(&mut rig, 0.5);
    let simulation_now = footer_text(&rig, "Generate and simulate op 1");
    let mut order = Vec::new();
    for _ in 0..400 {
        rig.press(Key::Tab, Modifiers::NONE);
        if let Some(rect) = focused(&rig) {
            if rect.contains_rect(simulation_now) {
                order.push("simulation");
            } else if rect.contains_rect(export) {
                order.push("export");
                break;
            }
        }
    }
    assert_eq!(order, ["simulation", "export"], "reading order");
    let _ = simulation;
    // Enter on the focused "Post and export…" opens the dialog.
    rig.press(Key::Enter, Modifiers::NONE);
    rig.frame(Vec::new());
    assert!(rig.shell.export.is_open());
}

#[test]
fn req_ui_07_req_ui_10_both_simulate_actions_wait_for_the_pending_edit_guard() {
    // Decision 0233: an unconfirmed field is resolved before either action
    // acts. Cancel keeps the draft and does nothing.
    let mut rig = generated("four-guard");
    type_unconfirmed(&mut rig, "Stepover", "1.6");
    rig.shell.post_and_export();
    for _ in 0..3 {
        rig.frame(Vec::new());
    }
    assert!(rig.shows("Unconfirmed changes"), "{:?}", rig.texts());
    assert!(!rig.shell.export.is_open());
    let cancel = last_text(&rig, "Cancel");
    rig.click(cancel.center());
    assert!(!rig.shell.export.is_open());
    assert_eq!(rig.shell.form.typed("stepover"), Some("1.6"));

    // Discard, then the action goes on: the dialog opens.
    rig.shell.post_and_export();
    for _ in 0..3 {
        rig.frame(Vec::new());
    }
    let discard = last_text(&rig, "Discard");
    rig.click(discard.center());
    for _ in 0..3 {
        rig.frame(Vec::new());
    }
    assert!(rig.shell.export.is_open());
    rig.shell.export.clear();

    // The simulation action asks the same way.
    rig.frame(Vec::new());
    type_unconfirmed(&mut rig, "Stepover", "1.6");
    rig.shell.simulation();
    for _ in 0..3 {
        rig.frame(Vec::new());
    }
    assert!(rig.shows("Unconfirmed changes"));
    assert!(!rig.shell.run.is_running());
}

#[test]
fn req_ui_10_req_ui_14_alt_5_opens_post_and_export_as_ctrl_e_does() {
    // Decision 0282 item 4: Alt 5 is kept, with Ctrl E's blocker and guard.
    let mut rig = Rig::new(super::Shell::with_settings(false, None, None), Vec::new());
    rig.press(Key::Num5, Modifiers::ALT);
    assert!(!rig.shell.export.is_open(), "no project, nothing to export");

    let mut rig = Rig::cube("four-alt5", None);
    let step = rig.shell.step;
    // Not behind a modal dialog.
    rig.shell.dialog = Some(Dialog::Settings);
    rig.frame(Vec::new());
    rig.press(Key::Num5, Modifiers::ALT);
    assert!(!rig.shell.export.is_open());
    rig.shell.dialog = None;
    rig.frame(Vec::new());
    rig.frame(Vec::new());
    rig.press(Key::Num5, Modifiers::ALT);
    assert!(rig.shell.export.is_open());
    // It is not a step: the view stays where it was.
    assert_eq!(rig.shell.step, step);
    // Help lists it, and four steps under Alt 1 to Alt 4.
    let keys = super::frame::KEY_MAP;
    assert!(keys.contains(&("Alt 1 to Alt 4", "Workflow steps")));
    assert!(keys.contains(&("Alt 5", "Post and export (as Ctrl E)")));
}

#[test]
fn req_ui_10_req_ui_11_the_step_bar_says_whether_g_code_was_written_from_the_project_now() {
    let mut rig = on_simulate("four-written", egui::vec2(1440.0, 900.0));
    // Four steps and the status after them, on every step.
    for step in Step::ALL {
        rig.shell.open_step(step);
        rig.frame(Vec::new());
        rig.frame(Vec::new());
        assert!(rig.shows("G-code not written"), "{}", step.name());
        assert!(!rig.shows("Post and export ("), "no fifth step");
    }
    let simulate = bar_text(&rig, "Simulate");
    let status = bar_text(&rig, "G-code not written");
    assert!(status.left() > simulate.right(), "after step 4");
    assert!(!rig.shell.overview.wrote_g_code_before());

    // A complete write from the run as it is now.
    let run = rig.shell.run.last().unwrap().clone();
    rig.shell.app.as_ref().unwrap().record_export(&run);
    // The dialog's written report marks the overview to be worked out again.
    rig.shell.overview_dirty = true;
    rig.settle();
    assert!(rig.shows("G-code written"));
    assert!(!rig.shows("G-code not written"));
    // Its words carry the state, not only its colour (UI-11): the ring and
    // tick are drawn in the colour for complete, and the words say it.
    let written = bar_text(&rig, "G-code written");
    assert!(written.left() > simulate.right());

    // An edit: not written for the project as it is now, though a past write
    // is known (its result stays in Messages, decision 0240).
    set_stepover(&mut rig, 0.5);
    assert!(rig.shows("G-code not written"));
    assert!(rig.shell.overview.wrote_g_code_before());
}

#[test]
fn req_ui_16_the_step_bar_and_simulate_footer_fit_at_the_supported_sizes() {
    // UI-16 and the X8 checks: 1366 × 768 and 1920 × 1080, at 100% and 120%.
    let mut rig = on_simulate("four-layout", egui::vec2(1366.0, 768.0));
    set_stepover(&mut rig, 0.5);
    for (size, ppp) in [
        (egui::vec2(1366.0, 768.0), 1.0),
        (egui::vec2(1366.0, 768.0), 1.2),
        (egui::vec2(1920.0, 1080.0), 1.0),
        (egui::vec2(1920.0, 1080.0), 1.2),
    ] {
        // Physical pixels at the scale: the window in logical pixels.
        rig.size = size / ppp;
        rig.ctx.set_pixels_per_point(ppp);
        for _ in 0..3 {
            rig.frame(Vec::new());
        }
        let window = egui::Rect::from_min_size(egui::Pos2::ZERO, rig.size);
        let whole = rig.text_rects();
        // Glyph rectangles: a label's galley can be wider than its text.
        let glyphs: Vec<(String, egui::Rect)> = rig
            .painted()
            .into_iter()
            .filter_map(|(text, _, seen)| Some((text?, seen)))
            .collect();
        for (label, in_bar) in [
            ("Model", true),
            ("Setup", true),
            ("Operations", true),
            ("Simulate", true),
            ("G-code not written", true),
            ("Generate and simulate op 1", false),
            ("Post and export…", false),
        ] {
            let seen = if in_bar {
                bar_text(&rig, label)
            } else {
                footer_text(&rig, label)
            };
            let drawn = whole
                .iter()
                .filter(|(t, r)| t == label && (r.center().y - seen.center().y).abs() < 1.0)
                .map(|(_, r)| *r)
                .next()
                .unwrap();
            assert!(
                (seen.width() - drawn.width()).abs() < 0.5,
                "{label} is clipped at {size:?} and {ppp}"
            );
            assert!(window.contains_rect(seen), "{label} at {size:?} and {ppp}");
            // Nothing else on its row lies over it.
            let seen = glyphs
                .iter()
                .filter(|(t, r)| t == label && (r.center().y - seen.center().y).abs() < 2.0)
                .map(|(_, r)| *r)
                .next()
                .unwrap();
            for (other, rect) in &glyphs {
                if other != label
                    && !other.is_empty()
                    && (rect.center().y - seen.center().y).abs() < 6.0
                {
                    assert!(
                        !rect.shrink(0.5).intersects(seen.shrink(0.5)),
                        "{other} {rect:?} overlaps {label} {seen:?} at {size:?} and {ppp}"
                    );
                }
            }
        }
    }
}
