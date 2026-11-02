// SPDX-License-Identifier: AGPL-3.0-or-later
//! Math in number boxes at the shell level (UI-26; issue 183): an unreadable
//! entry is refused under its box, the step bar marks the job, and a sum is
//! worked out when it is confirmed.

#![allow(clippy::float_cmp)]

use super::frame_tests::Rig;
use eframe::egui::{self, Event, Key, Modifiers};

/// The setup step of the cube project, drawn to rest.
fn setup_rig(name: &str) -> Rig {
    let mut rig = Rig::cube(name, None);
    crate::theme::install_style(&rig.ctx, crate::theme::Theme::Dark);
    rig.size = egui::vec2(1920.0, 1080.0);
    let id = rig.shell.app.as_ref().unwrap().project().setups()[0].id;
    rig.shell.choose_setup(id, None);
    for _ in 0..6 {
        rig.frame(Vec::new());
    }
    rig
}

/// The centre of the first entry box with a unit ("mm") drawn inside it.
fn unit_box(rig: &Rig) -> egui::Pos2 {
    let units: Vec<egui::Rect> = rig
        .text_rects()
        .into_iter()
        .filter(|(t, _)| t == "mm")
        .map(|(_, r)| r)
        .collect();
    rig.entry_rects()
        .into_iter()
        .filter(|b| b.width() > 60.0 && b.left() >= rig.size.x - 340.0)
        .find(|b| units.iter().any(|u| b.contains(u.center())))
        .map(|b| b.center())
        .expect("a number box with a unit in the setup step")
}

fn type_into(rig: &mut Rig, at: egui::Pos2, text: &str) {
    rig.click(at);
    rig.press(Key::A, Modifiers::COMMAND);
    rig.frame(vec![Event::Text(text.to_owned())]);
    rig.press(Key::Enter, Modifiers::NONE);
    for _ in 0..3 {
        rig.frame(Vec::new());
    }
}

#[test]
fn req_ui_26_req_nfr_07_an_unreadable_entry_shows_its_reason_under_the_box() {
    let mut rig = setup_rig("numbox-error");
    let at = unit_box(&rig);
    type_into(&mut rig, at, "12*");
    assert!(
        rig.shows("Cannot read '12*'; expected a number or a sum."),
        "{:?}",
        rig.texts()
    );
}

#[test]
fn req_ui_26_req_ui_11_an_unreadable_entry_marks_the_step_bar_until_fixed() {
    let mut rig = setup_rig("numbox-bar");
    assert!(!rig.shows("Error to fix"));
    let at = unit_box(&rig);
    type_into(&mut rig, at, "12*");
    assert!(rig.shows("Error to fix"), "{:?}", rig.texts());
    // Esc puts the last good value back and the mark goes.
    rig.click(at);
    rig.press(Key::Escape, Modifiers::NONE);
    for _ in 0..6 {
        rig.frame(Vec::new());
    }
    assert!(!rig.shows("Error to fix"), "{:?}", rig.texts());
    assert!(!rig.shows("Cannot read"));
}

#[test]
fn req_ui_26_a_sum_is_worked_out_and_only_the_number_is_kept() {
    let mut rig = setup_rig("numbox-sum");
    let at = unit_box(&rig);
    let diameter = |rig: &Rig| {
        let setup = &rig.shell.app.as_ref().unwrap().project().setups()[0];
        match setup.stock.as_ref() {
            Some(openchuck_app::setup::Stock::Cylinder { diameter, .. }) => diameter.0,
            other => panic!("the cube rig must have cylindrical stock: {other:?}"),
        }
    };
    // A value that differs from the result, so a sum that was never applied
    // cannot pass.
    type_into(&mut rig, at, "18");
    assert_eq!(diameter(&rig), 18.0);
    type_into(&mut rig, at, "10 + 5 * 2");
    assert_eq!(diameter(&rig), 20.0);
    assert!(!rig.shows("Cannot read"), "{:?}", rig.texts());
    assert!(
        !rig.shows("10 + 5"),
        "the sum is not kept: {:?}",
        rig.texts()
    );
    assert!(!rig.shows("Error to fix"));
}

#[test]
fn req_ui_26_the_last_good_value_is_kept_while_an_entry_is_unreadable() {
    let mut rig = setup_rig("numbox-last-good");
    let before = rig.shell.app.as_ref().unwrap().project().clone();
    let at = unit_box(&rig);
    type_into(&mut rig, at, "5 mm");
    assert!(rig.shows("Cannot read '5 mm'"), "{:?}", rig.texts());
    assert_eq!(&before, rig.shell.app.as_ref().unwrap().project());
}
