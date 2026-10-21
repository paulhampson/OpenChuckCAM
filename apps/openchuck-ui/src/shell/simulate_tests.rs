// SPDX-License-Identifier: AGPL-3.0-or-later
//! The Simulate step and the import window, drawn whole without a window
//! (slice 22 lane X6; decision records 0114 C14, C17, C20 and 0125): the
//! results inspector with its resolution control (SIM-05, SIM-07), the
//! playback dock (SIM-04), the tree's eye buttons and the import window
//! (IMP-01, IMP-03, IMP-04, IMP-05).

#![allow(clippy::unwrap_used)]

use std::time::{Duration, Instant};

use eframe::egui::{self, Event, Key, Modifiers};
use openchuck_app::overview::Step;

use super::frame_tests::Rig;
use super::*;
use crate::import_panel::ImportSession;

/// The cube project, simulated, on the Simulate step.
fn simulated(name: &str) -> Rig {
    let mut rig = Rig::cube(name, None);
    rig.shell.generate(true);
    let deadline = Instant::now() + Duration::from_secs(120);
    while rig.shell.run.last().is_none_or(|r| r.simulation.is_none()) {
        rig.settle();
        assert!(Instant::now() < deadline, "timed out");
    }
    rig.shell.open_step(Step::Simulate);
    // The playback data and the lines are prepared in jobs, which take longer
    // on a slow runner: wait for them, with a deadline.
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        rig.frame(Vec::new());
        if rig.shell.play.has_run() && rig.shell.play.pose().is_some() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the playback data was never ready"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    rig.settle();
    rig
}

#[test]
fn req_sim_05_req_sim_07_the_results_inspector_shows_the_numbers_and_the_resolution() {
    let mut rig = simulated("sim-results");
    for text in [
        "SIMULATION RESULTS",
        "Colour by",
        "Deviation",
        "Remaining",
        "Overcut",
        "Largest overcut",
        "Largest material left",
        "DEVIATION DISTRIBUTION",
        "BY OPERATION",
        "Finish",
        "RESOLUTION",
        "Cell size",
        "Between 0.025 and 1 mm",
    ] {
        assert!(rig.shows(text), "{text} is on screen");
    }
    let run = rig.shell.run.last().unwrap().clone();
    let summary = crate::results_panel::summarise(&run);
    assert_eq!(summary.len(), 1);
    assert_eq!(summary[0].operations.len(), 1);
    assert_eq!(summary[0].operations[0].name, "Finish");
    assert!(summary[0].within.is_some_and(|w| (0.0..=1.0).contains(&w)));
    assert!(summary[0].histogram.iter().map(|b| b.count).sum::<usize>() > 0);
    // The resolution control changes the project's cell, finer than the
    // default at a stated cost, and refuses a value out of range.
    let app = rig.shell.app.as_mut().unwrap();
    assert!((app.project().simulation().cell.0 - 0.1).abs() < 1e-12);
    rig.shell.results.apply(app, 0.05);
    assert!(rig.shell.results.error.is_none());
    assert!((app.project().simulation().cell.0 - 0.05).abs() < 1e-12);
    rig.shell.results.apply(app, 0.001);
    assert!(rig.shell.results.error.is_some());
    assert!((app.project().simulation().cell.0 - 0.05).abs() < 1e-12);
    rig.frame(Vec::new());
    assert!(rig.shows("The project has changed since this simulation"));
    assert!(rig.shows("times the time"));
}

/// Scrolls the playback tab of the dock until `text` is painted, in at most
/// 40 wheel events, and panics with everything painted if it never is.
///
/// The tab is a scroll area, and a row below the dock's edge is not painted.
/// At the font metrics of Windows and a window of 1440 x 900 the dock is only
/// about 140 px tall, so "Go to move" is below the fold there, while on Linux
/// it is not. Scrolling to a row is how a user reaches it. This is a choice
/// of the test, not a change to the product.
fn scroll_dock_to(rig: &mut Rig, text: &str) {
    let tab_y = rig
        .texts()
        .iter()
        .find(|(t, _)| t == "Playback")
        .map(|(_, y)| *y)
        .expect("the dock's Playback tab is painted");
    let over_dock = egui::pos2(700.0, tab_y + 40.0);
    rig.frame(vec![Event::PointerMoved(over_dock)]);
    for _ in 0..40 {
        if rig.shows(text) {
            return;
        }
        rig.frame(vec![Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            phase: egui::TouchPhase::Move,
            delta: egui::vec2(0.0, -60.0),
            modifiers: Modifiers::NONE,
        }]);
    }
    assert!(
        rig.shows(text),
        "{text} is on screen; painted: {:?}",
        rig.texts()
    );
}

#[test]
fn req_sim_04_the_dock_plays_with_space_and_speeds_and_the_tree_has_eye_buttons() {
    let mut rig = simulated("sim-dock");
    assert_eq!(rig.shell.dock_tab, DockTab::Playback);
    for text in ["Speed", "1000×", "10 000×", "Whole program"] {
        assert!(
            rig.shows(text),
            "{text} is on screen; painted: {:?}",
            rig.texts()
        );
    }
    assert!(rig.shows("The toolpaths, the simulation and the G-code do not change"));
    assert!(!rig.shows("Max"), "there is no Max speed");
    // The timeline's "Go to move" is under the speeds: scroll the dock to it.
    scroll_dock_to(&mut rig, "Go to move");
    // Space plays and pauses.
    assert!(!rig.shell.play.is_playing());
    rig.press(Key::Space, Modifiers::NONE);
    assert!(rig.shell.play.is_playing());
    rig.press(Key::Space, Modifiers::NONE);
    assert!(!rig.shell.play.is_playing());
    // The eye button hides one operation in the view only (issue 213).
    let op = rig.shell.app.as_ref().unwrap().project().operations()[0].id;
    assert!(rig.shell.play.draws_group(0));
    rig.shell
        .tree_action(TreeAction::Command(Box::new(Command::SetPathsShown {
            operations: vec![op],
            shown: false,
        })));
    rig.frame(Vec::new());
    assert!(!rig.shell.app.as_ref().unwrap().project().path_shown(op));
    assert!(!rig.shell.play.draws_group(0));
    assert!(
        rig.shell.run.last().unwrap().simulation.is_some(),
        "hiding keeps the results"
    );
}

#[test]
fn req_imp_01_req_imp_04_the_import_window_has_the_path_the_checks_and_the_primary() {
    let mut rig = Rig::cube("import-window", None);
    // A blank window asks for the path first (11.2), inside the window.
    rig.shell.import_model();
    rig.frame(Vec::new());
    rig.frame(Vec::new());
    rig.frame(Vec::new());
    assert!(rig.shell.import.is_some());
    assert!(rig.shows("Browse…"));
    assert!(rig.shows("PLACEMENT"));
    assert!(rig.shell.dialog.is_none(), "no separate path dialog");
    let cube =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/stl/cube_ascii.stl");
    let app = rig.shell.app.as_ref().unwrap();
    rig.shell.import = Some(ImportSession::new(app, cube));
    let deadline = Instant::now() + Duration::from_secs(60);
    while rig.shell.import.as_ref().unwrap().preview.is_none() {
        rig.frame(Vec::new());
        assert!(Instant::now() < deadline, "timed out");
        std::thread::sleep(Duration::from_millis(10));
    }
    rig.frame(Vec::new());
    rig.frame(Vec::new());
    for text in [
        "UNITS",
        "MESH CHECK",
        "Holes",
        "Flipped triangles",
        "Degenerate triangles",
        "Non-manifold edges",
        "The file is not changed. Its path and hash are stored in the project.",
        "Add to project",
        "Rotary axis",
        "Size X",
        "Volume",
    ] {
        assert!(rig.shows(text), "{text} is on screen");
    }
    // The body sits between the header and the footer, and the buttons are
    // under the footer text: nothing is squashed into a thin strip (#222).
    let y_of = |rig: &Rig, text: &str| {
        rig.text_rects()
            .iter()
            .find(|(t, _)| t == text)
            .map(|(_, r)| r.top())
            .unwrap_or_else(|| panic!("{text} is painted"))
    };
    let footer_y = y_of(
        &rig,
        "The file is not changed. Its path and hash are stored in the project.",
    );
    assert!(y_of(&rig, "1 UNITS") < y_of(&rig, "Holes"));
    assert!(
        y_of(&rig, "Holes") < footer_y,
        "the body is above the footer"
    );
    assert!(
        y_of(&rig, "Add to project") > footer_y,
        "the buttons are under it: {} vs {footer_y}",
        y_of(&rig, "Add to project")
    );
    // The window has a fixed size, so the body scrolls (UI-16): the rest of
    // the check is reached by scrolling over it.
    // The dialog is centred in the main window (decision 0164), so the
    // pointer goes where the Holes row is painted.
    // (The first row of the body, above the default dock: the dock is
    // 260 high now, decision 0187, and the wheel goes to what is under the
    // pointer.)
    let holes = rig
        .text_rects()
        .iter()
        .find(|(t, _)| t == "1 UNITS")
        .map(|(_, r)| *r)
        .expect("the UNITS row is painted");
    rig.frame(vec![Event::PointerMoved(holes.center())]);
    for _ in 0..40 {
        if rig.shows("The check reports problems and changes nothing.") {
            break;
        }
        rig.frame(vec![Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            phase: egui::TouchPhase::Move,
            delta: egui::vec2(0.0, -60.0),
            modifiers: Modifiers::NONE,
        }]);
    }
    assert!(rig.shows("The check reports problems and changes nothing."));
    // Add imports the file that was checked, so it waits while the field
    // names another (OI-358).
    let session = rig.shell.import.as_mut().unwrap();
    assert!(session.can_add());
    let checked = session.path_text.clone();
    session.path_text = "another.stl".to_owned();
    assert!(
        !session.can_add(),
        "the field names a file that is not checked"
    );
    session.path_text = format!("  {checked} ");
    assert!(
        session.can_add(),
        "spaces round the checked path do not matter"
    );
    // Escape-free close: Cancel path leaves the project as it was.
    let before = rig.shell.app.as_ref().unwrap().project().models().len();
    rig.shell.import = None;
    rig.frame(Vec::new());
    assert_eq!(
        rig.shell.app.as_ref().unwrap().project().models().len(),
        before
    );
}

/// Where `text` is painted.
fn at(rig: &Rig, text: &str) -> egui::Pos2 {
    rig.text_rects()
        .iter()
        .find(|(t, _)| t == text)
        .unwrap_or_else(|| panic!("{text} is shown"))
        .1
        .center()
}

#[test]
fn req_sim_03_the_skip_tick_box_is_the_projects_and_generating_follows_it() {
    // Issue 214 (decision record 0168): the stock is simulated with the
    // toolpaths unless the project's tick box says to skip it; the choice is
    // one undoable step and the dock says why there is no cut part.
    let mut rig = Rig::cube("sim-skip", None);
    rig.shell.open_step(Step::Operations);
    rig.shell.primary();
    rig.settle();
    assert!(
        rig.shell.run.last().unwrap().simulation.is_some(),
        "generating simulates by default"
    );
    rig.shell.open_step(Step::Simulate);
    rig.settle();
    rig.frame(Vec::new());
    let skip = at(&rig, "Skip stock simulation");
    rig.click(skip);
    let app = rig.shell.app.as_ref().unwrap();
    assert!(app.project().skip_stock_simulation());
    assert!(app.is_modified(), "saved with the project");
    // Generating now makes the toolpaths only.
    rig.shell.generate(false);
    rig.settle();
    rig.frame(Vec::new());
    assert!(rig.shell.run.last().unwrap().simulation.is_none());
    assert!(rig.shows("No cut part: the stock simulation is skipped for this project"));
    // Undo turns it back on.
    rig.shell.undo();
    assert!(
        !rig.shell
            .app
            .as_ref()
            .unwrap()
            .project()
            .skip_stock_simulation()
    );
}

#[test]
fn req_ui_15_req_sim_02_the_simulate_choices_are_remembered_and_the_window_is_not() {
    // Issues 203, 204, 205: "Turn part with A", the view's tick boxes and
    // "Hide path outside window" are kept between runs; the window resets
    // when Simulate is left.
    let config = super::frame_tests::temp("sim-settings");
    let file = SettingsFile::in_folder(&config);
    let mut rig = simulated("sim-settings");
    rig.shell.settings_file = Some(file.clone());
    rig.dirs.push(config.clone());
    assert!(rig.shell.play.turns_part_with_a(), "on by default");
    rig.shell.play.set_turn_part_with_a(false);
    rig.shell.play.set_hide_outside_window(true);
    rig.shell
        .viewport
        .set_simulate_show(crate::viewport::SimulateShow {
            model: true,
            cut_part: false,
            toolpath: true,
        });
    rig.shell
        .play
        .set_window(Some(openchuck_app::playback::MoveWindow {
            first: 2,
            last: 5,
        }));
    rig.frame(Vec::new());
    rig.shell.save_settings();
    let kept = file.load().settings.simulate;
    assert!(!kept.turn_part_with_a && kept.hide_outside_window);
    assert!(kept.show_model && !kept.show_cut_part && kept.show_toolpath);
    // A restart starts from them.
    let shell = Shell::with_settings(false, None, Some(file));
    assert!(!shell.play.turns_part_with_a());
    assert!(shell.play.hides_outside_window());
    assert!(shell.viewport.simulate_show().toolpath);
    // The window is not remembered: leaving Simulate lifts it.
    assert!(rig.shell.play.window().is_some());
    rig.shell.open_step(Step::Operations);
    rig.frame(Vec::new());
    assert_eq!(rig.shell.play.window(), None);
}

#[test]
fn req_sim_03_a_new_project_takes_the_users_default_for_the_skip() {
    let mut shell = Shell::with_settings(false, None, None);
    shell.settings.simulate.skip_stock_simulation = true;
    let folder = super::frame_tests::temp("sim-new-skip");
    shell.create(folder.clone(), "Skipping");
    assert!(
        shell
            .app
            .as_ref()
            .unwrap()
            .project()
            .skip_stock_simulation()
    );
    drop(shell);
    let _ = std::fs::remove_dir_all(folder);
}

#[test]
fn req_sim_01_req_ui_11_the_tree_hides_paths_in_every_step_and_all_at_once() {
    // Issue 213 (decision record 0169): on Operations too, and for the whole
    // group; saved with the project, and the toolpath stays up to date.
    let mut rig = Rig::cube("tree-eyes", None);
    rig.shell.open_step(Step::Operations);
    rig.shell.primary();
    rig.settle();
    let ops: Vec<_> = rig
        .shell
        .app
        .as_ref()
        .unwrap()
        .project()
        .operations()
        .iter()
        .map(|o| o.id)
        .collect();
    rig.shell
        .tree_action(TreeAction::Command(Box::new(Command::SetPathsShown {
            operations: ops.clone(),
            shown: false,
        })));
    rig.settle();
    let app = rig.shell.app.as_ref().unwrap();
    assert!(ops.iter().all(|id| !app.project().path_shown(*id)));
    assert!(rig.shell.overview.stale.is_empty(), "nothing is made stale");
    assert!(!rig.shell.play.draws_group(0));
    // Simulate's line says what hiding changes.
    rig.shell.open_step(Step::Simulate);
    rig.frame(Vec::new());
    assert!(rig.shows("Hiding a toolpath changes the view only."));
}
