// SPDX-License-Identifier: AGPL-3.0-or-later
//! The Results tab of the dock (UI-12, UI-16; issue 475; decision 0630): the
//! Results window is a tab after Jobs on every step, opened by a click or by a
//! failed run, with no entry in the Project or View menu.

#![allow(clippy::unwrap_used)]

use super::DockTab;
use super::frame::Region;
use super::frame_tests::Rig;
use eframe::egui;
use openchuck_app::Regeneration;
use openchuck_app::overview::Step;
use std::sync::Arc;

fn rig(name: &str) -> Rig {
    let mut rig = Rig::cube(name, None);
    rig.size = egui::vec2(1440.0, 900.0);
    rig.settle();
    rig
}

fn centre(rig: &Rig, text: &str) -> egui::Pos2 {
    rig.text_rects()
        .into_iter()
        .find(|(t, _)| t == text)
        .unwrap_or_else(|| panic!("no {text:?}: {:?}", rig.texts()))
        .1
        .center()
}

fn exact(rig: &Rig, text: &str) -> usize {
    rig.texts().iter().filter(|(t, _)| t == text).count()
}

fn two_warnings() -> Arc<Regeneration> {
    Arc::new(Regeneration {
        notes: vec!["alpha: a note".to_owned(), "bravo: another note".to_owned()],
        ..Regeneration::default()
    })
}

#[test]
fn req_ui_12_neither_project_nor_view_menu_has_a_results_item() {
    for menu in ["Project", "View"] {
        let mut rig = rig(&format!("results-menu-{menu}"));
        let tab_only = exact(&rig, "Results");
        let at = centre(&rig, menu);
        rig.click(at);
        rig.frame(Vec::new());
        assert!(!rig.shows("Results…"), "{menu}: no Results… item");
        assert_eq!(exact(&rig, "Results"), tab_only, "{menu}: no Results item");
    }
}

#[test]
fn req_ui_12_results_is_a_tab_after_jobs_on_every_step() {
    let mut rig = rig("results-tab-order");
    let results = DockTab::Results;
    rig.shell.step = Step::Operations;
    assert_eq!(
        rig.shell.tabs(),
        [DockTab::Messages, DockTab::Jobs, results]
    );
    rig.shell.step = Step::Setup;
    assert_eq!(
        rig.shell.tabs(),
        [
            DockTab::SetupChecks,
            DockTab::Messages,
            DockTab::Jobs,
            results
        ]
    );
    rig.shell.step = Step::Simulate;
    assert_eq!(
        rig.shell.tabs(),
        [DockTab::Messages, DockTab::Jobs, results, DockTab::Playback]
    );
}

#[test]
fn req_ui_12_there_is_no_results_tab_without_a_project() {
    let mut rig = rig("results-tab-no-project");
    rig.shell.app = None;
    assert!(!rig.shell.tabs().contains(&DockTab::Results));
}

#[test]
fn req_ui_12_the_label_shows_warnings_and_errors_of_the_latest_run() {
    let mut rig = rig("results-tab-label");
    assert_eq!(exact(&rig, "Results"), 1, "no run: no number");
    rig.shell.run.record_run(two_warnings());
    rig.frame(Vec::new());
    assert_eq!(exact(&rig, "Results (2)"), 1);
    // The chips filter the lines, not the count.
    rig.shell.result_filter.warnings = false;
    rig.frame(Vec::new());
    assert_eq!(exact(&rig, "Results (2)"), 1);
}

#[test]
fn req_ui_12_clicking_the_tab_on_the_rail_opens_the_dock_on_it() {
    let mut rig = rig("results-tab-rail");
    rig.shell.toggle_region(Region::Dock);
    rig.settle();
    assert!(rig.shell.collapsed(Region::Dock));
    let at = centre(&rig, "Results");
    rig.click(at);
    rig.frame(Vec::new());
    assert_eq!(rig.shell.dock_tab, DockTab::Results);
    assert!(!rig.shell.collapsed(Region::Dock));
}

#[test]
fn req_ui_12_a_failed_run_selects_the_results_tab_and_opens_the_dock() {
    let mut rig = rig("results-tab-failed");
    rig.shell.toggle_region(Region::Dock);
    rig.settle();
    rig.shell
        .run
        .record_failure("the engine stopped".to_owned());
    rig.frame(Vec::new());
    rig.frame(Vec::new());
    assert_eq!(rig.shell.dock_tab, DockTab::Results);
    assert!(!rig.shell.collapsed(Region::Dock));
    assert!(rig.shows("the engine stopped"));
}

#[test]
fn req_ui_12_the_results_window_is_gone_so_shortcuts_are_not_held_back() {
    let mut rig = rig("results-tab-no-window");
    rig.shell
        .run
        .record_failure("the engine stopped".to_owned());
    rig.frame(Vec::new());
    assert!(!rig.shell.window_open(), "Results is a tab, not a window");
}
