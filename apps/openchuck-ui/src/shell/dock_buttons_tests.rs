// SPDX-License-Identifier: AGPL-3.0-or-later
//! The dock's hide and show buttons are on the same side (UI-16; issue 433):
//! the right end of the dock's tab row, whether the dock is open or collapsed
//! to its rail.

#![allow(clippy::unwrap_used)]

use super::frame_tests::Rig;
use eframe::egui;

/// The rectangle of the first text exactly equal to `glyph`.
fn glyph(rig: &Rig, glyph: &str) -> egui::Rect {
    rig.text_rects()
        .into_iter()
        .find(|(t, _)| t == glyph)
        .unwrap_or_else(|| panic!("no {glyph:?} drawn: {:?}", rig.texts()))
        .1
}

#[test]
fn req_ui_16_the_dock_show_button_is_on_the_right_like_the_hide_button() {
    let mut rig = Rig::cube("dock-buttons", None);
    rig.size = egui::vec2(1440.0, 900.0);
    rig.settle();
    let hide = glyph(&rig, "\u{23f7}");
    assert!(hide.center().x > rig.size.x * 0.5, "hide at {hide:?}");

    rig.shell.toggle_region(super::frame::Region::Dock);
    rig.settle();
    let show = glyph(&rig, "\u{23f6}");
    let tabs = rig
        .text_rects()
        .into_iter()
        .filter(|(t, _)| t.starts_with("Messages") || t.starts_with("Jobs"))
        .map(|(_, r)| r)
        .reduce(|a, b| a.union(b))
        .expect("the dock tabs");
    assert!(show.left() > tabs.right(), "show {show:?} tabs {tabs:?}");
    assert!(
        (show.right() - hide.right()).abs() <= 4.0,
        "show {show:?} hide {hide:?}"
    );
}
