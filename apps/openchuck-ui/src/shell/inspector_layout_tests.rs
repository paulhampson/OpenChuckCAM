// SPDX-License-Identifier: AGPL-3.0-or-later
//! How the inspector shares its width between the labels and the entry boxes
//! (UI-07, UI-16; issue 434; decision 0440; specification section 8):
//! the label column fits its labels first, a number box stays compact, a
//! text box fills the rest.

#![allow(clippy::unwrap_used)]

use super::frame_tests::Rig;
use super::*;
use crate::theme::width;
use eframe::egui;

/// What the inspector drew at one pane width.
struct Drawn {
    left: f32,
    /// Every text in the pane with its rectangle.
    texts: Vec<(String, egui::Rect)>,
    /// The texts in the pane drawn cut short with an ellipsis.
    elided: Vec<String>,
    /// Every entry box in the pane.
    entries: Vec<egui::Rect>,
}

fn drawn(select: impl FnOnce(&mut Rig), pane: f32, name: &str) -> Drawn {
    let mut rig = Rig::cube(name, None);
    crate::theme::install_style(&rig.ctx, crate::theme::Theme::Dark);
    rig.size = egui::vec2(1920.0, 1080.0);
    select(&mut rig);
    for _ in 0..8 {
        rig.frame(Vec::new());
    }
    // The pane is dragged to its width by its splitter, as a user does.
    let from = egui::pos2(rig.size.x - rig.shell.settings.inspector.size, 400.0);
    rig.drag(from, egui::pos2(rig.size.x - pane, 400.0));
    for _ in 0..8 {
        rig.frame(Vec::new());
    }
    assert!(
        (rig.shell.settings.inspector.size - pane).abs() < 1.0,
        "the pane is {} wide, not {pane}",
        rig.shell.settings.inspector.size
    );
    let left = rig.size.x - rig.shell.settings.inspector.size;
    Drawn {
        left,
        texts: rig
            .text_rects()
            .into_iter()
            .filter(|(_, r)| r.left() >= left && r.top() > 60.0 && r.bottom() < rig.size.y - 40.0)
            .collect(),
        elided: rig
            .elided_texts()
            .into_iter()
            .filter(|(_, r)| r.left() >= left && r.top() > 60.0 && r.bottom() < rig.size.y - 40.0)
            .map(|(t, _)| t)
            .collect(),
        entries: rig
            .entry_rects()
            .into_iter()
            .filter(|r| r.left() > left && r.height() < 30.0)
            .collect(),
    }
}

fn operation(rig: &mut Rig) {
    let op = rig.shell.app.as_ref().unwrap().project().operations()[0].id;
    rig.shell.select(Selection::Operation(op));
}

fn setup(rig: &mut Rig) {
    let id = rig.shell.app.as_ref().unwrap().project().setups()[0].id;
    rig.shell.choose_setup(id, None);
}

/// The labels drawn cut short with an ellipsis (the galley's `elided` flag; a
/// galley's text is always the whole string, so its ending says nothing).
/// A default's hint ("0.800 \u{b7} from the tool's cutting data") and the text
/// shown in a drop-down list are cut by design and are left out.
fn cut(d: &Drawn) -> Vec<String> {
    d.elided
        .iter()
        .filter(|t| !t.contains('\u{b7}') && !t.starts_with("Setup's stock material"))
        .cloned()
        .collect()
}

#[test]
fn req_ui_07_no_inspector_label_is_cut_short_at_the_default_and_wide_widths() {
    // The operation inspector at the default and the widest pane, and the
    // setup inspector at the widest. At the default width the setup
    // inspector's longest label ("Tool length measurement") is over the 55%
    // share and is cut, with its whole text in the tooltip: the known cost
    // of decision 0440.
    for (name, select, pane) in [
        ("layout-op", operation as fn(&mut Rig), width::INSPECTOR.0),
        ("layout-op", operation as fn(&mut Rig), width::INSPECTOR.2),
        ("layout-setup", setup as fn(&mut Rig), width::INSPECTOR.2),
    ] {
        let d = drawn(select, pane, name);
        assert!(!d.entries.is_empty(), "{name} {pane}: no entry boxes");
        assert_eq!(cut(&d), Vec::<String>::new(), "{name} at {pane}");
    }
}

#[test]
fn req_ui_07_the_detector_sees_the_cut_label_of_the_setup_inspector_at_the_default_width() {
    // Proves `cut` is not blind: the label that sets the setup inspector's
    // column is over its share at 340 and is reported.
    let d = drawn(setup, width::INSPECTOR.0, "layout-setup-cut");
    assert!(
        cut(&d).iter().any(|t| t == "Tool length measurement"),
        "{:?}",
        d.elided
    );
}

#[test]
fn req_ui_07_every_compact_box_of_a_form_has_one_width_and_shared_edges() {
    for pane in [width::INSPECTOR.0, width::INSPECTOR.2] {
        let d = drawn(operation, pane, "layout-compact");
        let on_row = |label: &str, b: &egui::Rect| {
            d.texts
                .iter()
                .any(|(t, r)| t == label && (r.center().y - b.center().y).abs() < 6.0)
        };
        // The boxes with a unit in them, and the tool number's box.
        let compact: Vec<egui::Rect> = d
            .entries
            .iter()
            .filter(|b| {
                on_row("Tool number", b)
                    || d.texts.iter().any(|(t, r)| {
                        (t == "mm" || t == "mm/min" || t == "rpm" || t == "\u{b0}" || t == "deg")
                            && b.contains(r.center())
                    })
            })
            .copied()
            .collect();
        assert!(compact.len() >= 5, "{pane}: {compact:?}");
        assert!(
            compact.iter().any(|b| on_row("Tool number", b)),
            "{pane}: no tool number box"
        );
        for b in &compact {
            assert!(b.width() <= width::NUMBER_BOX + 0.5, "{pane}: {b:?}");
            assert!(
                (b.left() - compact[0].left()).abs() < 0.6,
                "left {pane}: {compact:?}"
            );
            assert!(
                (b.right() - compact[0].right()).abs() < 0.6,
                "right {pane}: {compact:?}"
            );
        }
        if pane > 450.0 {
            assert!(
                (compact[0].width() - width::NUMBER_BOX).abs() < 0.6,
                "{pane}: {compact:?}"
            );
        }
    }
}

#[test]
fn req_ui_07_widening_the_pane_gives_the_labels_room_before_the_boxes() {
    let narrow = drawn(operation, width::INSPECTOR.1, "layout-first-a");
    let wide = drawn(operation, width::INSPECTOR.2, "layout-first-b");
    // The left edge of the first number box, from the pane edge: it moves
    // right as labels need room, and never beyond the label column maximum.
    let first = |d: &Drawn| {
        d.entries
            .iter()
            .filter(|r| r.width() < 200.0)
            .map(|r| r.left() - d.left)
            .fold(f32::MAX, f32::min)
    };
    assert!(first(&wide) >= first(&narrow));
    assert!(first(&wide) <= 220.0 + 60.0, "{}", first(&wide));
}

/// Decision record 0536 section 4: "Show setting" on a message about a
/// setup's field opens the Setup step at that setup, with the field's
/// section open and the field marked; on a message about a tool, the tool
/// library at that tool's field.
#[test]
fn req_ui_12_show_setting_opens_a_setup_or_a_tool_at_its_field() {
    use openchuck_app::overview::{MessageAction, SettingRef};
    use openchuck_app::setup_form::SetupField;
    let mut rig = Rig::cube("show-setting-places", None);
    rig.size = egui::vec2(1920.0, 1080.0);
    for _ in 0..4 {
        rig.frame(Vec::new());
    }
    let project = rig.shell.app.as_ref().unwrap().project();
    let setup = project.setups()[0].id;
    let tool = project.tools()[0].id;
    rig.shell
        .message_action(MessageAction::ShowSetting(SettingRef::setup(
            setup,
            SetupField::SafeZ,
        )));
    for _ in 0..6 {
        rig.frame(Vec::new());
    }
    assert_eq!(rig.shell.step, Step::Setup);
    assert_eq!(rig.shell.selected, Some(Selection::Setup(setup)));
    assert!(rig.shows("Safe Z"), "the work offset section is open");
    assert!(
        rig.shell
            .form
            .is_shown(&inspector::setup_field_key(SetupField::SafeZ))
    );

    // A link to the setup's machine, which has no field of its own, leaves
    // no earlier mark behind (CodeRabbit on #659).
    rig.shell
        .message_action(MessageAction::ShowSetting(SettingRef::setup_machine(setup)));
    for _ in 0..3 {
        rig.frame(Vec::new());
    }
    assert!(
        !rig.shell
            .form
            .is_shown(&inspector::setup_field_key(SetupField::SafeZ))
    );

    rig.shell
        .message_action(MessageAction::ShowSetting(SettingRef::tool(
            tool,
            openchuck_app::tool_edit::fields::FLUTE_LENGTH,
        )));
    for _ in 0..6 {
        rig.frame(Vec::new());
    }
    assert!(rig.shell.tool_library.is_open());
    assert!(rig.shows("Flute length"));
    assert!(
        rig.shell
            .tool_library
            .is_shown(openchuck_app::tool_edit::fields::FLUTE_LENGTH)
    );
}
