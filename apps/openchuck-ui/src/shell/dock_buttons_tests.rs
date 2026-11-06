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

/// The collapsed rail at `width`: the show button must be drawn inside the
/// window, clear of every tab, and clicking it must open the dock (UI-16;
/// issue 468, OI-2040).
fn rail_keeps_its_show_button(width: f32) {
    let mut rig = Rig::cube(&format!("dock-rail-{width}"), None);
    rig.size = egui::vec2(width, 700.0);
    rig.settle();
    // The Setup step has the longest tab labels ("Setup checks (n)").
    rig.shell.step = super::Step::Setup;
    if !rig.shell.collapsed(super::frame::Region::Dock) {
        rig.shell.toggle_region(super::frame::Region::Dock);
    }
    rig.settle();
    assert!(rig.shell.collapsed(super::frame::Region::Dock));

    let show = glyph(&rig, "\u{23f6}");
    assert!(
        show.left() >= 0.0 && show.right() <= rig.size.x,
        "show button {show:?} is outside the {width} pt window"
    );
    // What shows of each tab label (after the row's clip) in the button's row.
    let tabs: Vec<_> = rig
        .painted()
        .into_iter()
        .filter_map(|(text, _, seen)| Some((text?, seen)))
        .filter(|(t, seen)| {
            (seen.center().y - show.center().y).abs() < 12.0
                && ["Setup checks", "Messages", "Jobs", "Results", "Playback"]
                    .iter()
                    .any(|n| t.starts_with(n))
        })
        .collect();
    assert!(!tabs.is_empty(), "no dock tab labels found at {width} pt");
    for (name, seen) in &tabs {
        assert!(
            !seen.intersects(show),
            "tab {name:?} {seen:?} overlaps the show button {show:?} at {width} pt"
        );
    }
    rig.click(show.center());
    rig.settle();
    assert!(
        !rig.shell.collapsed(super::frame::Region::Dock),
        "the show button did not open the dock at {width} pt"
    );
}

#[test]
fn req_ui_16_the_collapsed_rail_keeps_its_show_button_at_540_pt() {
    rail_keeps_its_show_button(540.0);
}

#[test]
fn req_ui_16_the_collapsed_rail_keeps_its_show_button_at_440_pt() {
    rail_keeps_its_show_button(440.0);
}
