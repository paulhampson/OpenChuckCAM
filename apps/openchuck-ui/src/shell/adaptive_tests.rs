// SPDX-License-Identifier: AGPL-3.0-or-later
//! Turning adaptive spacing on for an operation saved with a Scallop height
//! of 0 asks for the height first (OP-04, UI-07; decision record 0239 item
//! 5).

#![allow(clippy::unwrap_used)]

use eframe::egui;
use openchuck_app::overview::Step;
use openchuck_app::project::ParameterValue;

use super::Selection;
use super::frame_tests::Rig;

fn click(rig: &mut Rig, text: &str) {
    let mut at = None;
    for _ in 0..4 {
        at = rig
            .visible_text_rects()
            .into_iter()
            .rev()
            .find(|(label, _)| label == text)
            .map(|(_, rect)| rect.center());
        if at.is_some() {
            break;
        }
        rig.frame(Vec::new());
    }
    let at = at.unwrap_or_else(|| panic!("{text} is visible: {:?}", rig.texts()));
    rig.click(at);
    for _ in 0..3 {
        rig.frame(Vec::new());
    }
}

/// Clicks the check box on the row labelled `label`: the small square drawn
/// to the right of the label.
fn tick(rig: &mut Rig, label: &str) {
    fn walk(shape: &egui::Shape, out: &mut Vec<egui::Rect>) {
        match shape {
            egui::Shape::Rect(r) => out.push(r.rect),
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| walk(s, out)),
            _ => {}
        }
    }
    let row = rig
        .visible_text_rects()
        .into_iter()
        .find(|(text, _)| text == label)
        .map(|(_, rect)| rect)
        .unwrap_or_else(|| panic!("{label} is visible: {:?}", rig.texts()));
    let mut rects = Vec::new();
    rig.shapes().iter().for_each(|s| walk(&s.shape, &mut rects));
    let boxed = rects
        .into_iter()
        .filter(|r| {
            r.left() > row.right()
                && (8.0..=24.0).contains(&r.width())
                && (8.0..=24.0).contains(&r.height())
                && (r.center().y - row.center().y).abs() < 8.0
        })
        .min_by(|a, b| a.left().total_cmp(&b.left()))
        .unwrap_or_else(|| panic!("{label} has a check box"));
    rig.click(boxed.center());
    for _ in 0..3 {
        rig.frame(Vec::new());
    }
}

#[test]
fn req_op_04_req_ui_07_adaptive_spacing_from_a_saved_zero_asks_for_the_height_first() {
    let mut rig = Rig::cube("adaptive-zero", None);
    crate::theme::install_style(&rig.ctx, crate::theme::Theme::Dark);
    rig.ctx
        .style_mut_of(egui::Theme::Dark, |style| style.animation_time = 0.0);
    rig.size = egui::vec2(1920.0, 1440.0);
    let op = rig.shell.app.as_ref().unwrap().project().operations()[0].id;
    // The cube's operation is saved with adaptive spacing off; a Scallop
    // height of 0, as an older project held it.
    rig.shell
        .app
        .as_mut()
        .unwrap()
        .set_parameter(op, "scallop", Some(ParameterValue::Number(0.0)))
        .unwrap();
    rig.shell.select(Selection::Operation(op));
    rig.shell.open_step(Step::Operations);
    for _ in 0..4 {
        rig.frame(Vec::new());
    }
    let value = |rig: &Rig, name: &str| {
        rig.shell
            .app
            .as_ref()
            .unwrap()
            .project()
            .operation(op)
            .unwrap()
            .parameters
            .get(name)
            .cloned()
    };
    let before = (value(&rig, "adaptive_spacing"), value(&rig, "scallop"));
    assert_eq!(before.0, Some(ParameterValue::Flag(false)));

    // Ticking the switch asks first; Cancel changes nothing.
    tick(&mut rig, "Adaptive spacing");
    assert!(rig.shows("Turn on adaptive spacing"), "{:?}", rig.texts());
    assert!(rig.shows("which meant no limit on the cusp"));
    assert_eq!(
        (value(&rig, "adaptive_spacing"), value(&rig, "scallop")),
        before
    );
    click(&mut rig, "Cancel");
    assert!(!rig.shows("Turn on adaptive spacing"));
    assert_eq!(
        (value(&rig, "adaptive_spacing"), value(&rig, "scallop")),
        before
    );

    // Turn on sets both, the suggested 0.01 mm, in one undoable step.
    tick(&mut rig, "Adaptive spacing");
    click(&mut rig, "Turn on");
    assert_eq!(
        value(&rig, "adaptive_spacing"),
        Some(ParameterValue::Flag(true))
    );
    assert_eq!(value(&rig, "scallop"), Some(ParameterValue::Number(0.01)));
    rig.shell.app.as_mut().unwrap().undo().unwrap();
    assert_eq!(
        (value(&rig, "adaptive_spacing"), value(&rig, "scallop")),
        before
    );
}

#[test]
fn req_nfr_07_the_height_typed_is_a_number_of_millimetres_at_least_0() {
    assert_eq!(super::adaptive_height(" 0.02 "), Some(0.02));
    assert_eq!(super::adaptive_height("0"), Some(0.0));
    assert_eq!(super::adaptive_height("-1"), None);
    assert_eq!(super::adaptive_height("NaN"), None);
    assert_eq!(super::adaptive_height("inf"), None);
    assert_eq!(super::adaptive_height("a"), None);
}
