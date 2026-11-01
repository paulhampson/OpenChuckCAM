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

/// A label text cut short ends in an ellipsis and is not one of the
/// buttons and menu entries that end in one by design.
fn cut(d: &Drawn) -> Vec<String> {
    d.texts
        .iter()
        .filter(|(t, r)| t.ends_with('\u{2026}') && r.width() < 160.0 && t.chars().count() > 3)
        .filter(|(t, _)| {
            !matches!(
                t.as_str(),
                "Pick\u{2026}"
                    | "Edit tool\u{2026}"
                    | "Save to library\u{2026}"
                    | "From the library\u{2026}"
                    | "Manage machines\u{2026}"
            )
        })
        .map(|(t, _)| t.clone())
        .collect()
}

#[test]
fn req_ui_07_no_inspector_label_is_cut_short_at_the_default_and_wide_widths() {
    for pane in [width::INSPECTOR.0, width::INSPECTOR.2] {
        for (name, select) in [
            ("layout-op", operation as fn(&mut Rig)),
            ("layout-setup", setup as fn(&mut Rig)),
        ] {
            let d = drawn(select, pane, name);
            assert!(!d.entries.is_empty(), "{name} {pane}: no entry boxes");
            assert_eq!(cut(&d), Vec::<String>::new(), "{name} at {pane}");
        }
    }
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
