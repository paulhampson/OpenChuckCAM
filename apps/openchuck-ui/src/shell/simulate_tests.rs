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
    assert!(rig.shows("The simulation settings have changed since this simulation."));
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

/// The dock's drawn height and the height asked for: equal while the dock's
/// contents fill it, whatever the tab holds (issues 259 and 215).
fn dock_heights(rig: &Rig) -> (f32, f32) {
    let drawn = egui::PanelState::load(&rig.ctx, egui::Id::new("dock"))
        .expect("the dock was drawn")
        .outer_rect
        .height();
    (drawn, rig.shell.settings.dock.size)
}

/// How many times `needle` is painted.
fn count(rig: &Rig, needle: &str) -> usize {
    rig.texts().iter().filter(|(t, _)| t == needle).count()
}

/// Draws until the run started is done and its playback is ready.
fn until_simulated_again(rig: &mut Rig) {
    let deadline = Instant::now() + Duration::from_secs(120);
    while rig.shell.run.is_running() || !rig.shell.play.has_run() || rig.shell.play.pose().is_none()
    {
        rig.frame(Vec::new());
        std::thread::sleep(Duration::from_millis(10));
        assert!(Instant::now() < deadline, "timed out");
    }
    rig.settle();
}

#[test]
fn req_nfr_12_req_ui_11_req_sim_04_a_changed_parameter_marks_the_simulation_out_of_date() {
    // Issues 259 and 260: after a parameter change in Simulate the old result
    // stays to compare, but the view, the dock and the results each say it
    // is out of date, why, and offer to simulate again from Simulate. The
    // playback controls stay, and the dock is drawn whole.
    let mut rig = simulated("sim-stale-parameter");
    assert!(!rig.shows(super::out_of_date::TITLE));
    let op = rig.shell.app.as_ref().unwrap().project().operations()[0].id;
    rig.shell
        .app
        .as_mut()
        .unwrap()
        .set_parameter(
            op,
            "stepover",
            Some(openchuck_app::project::ParameterValue::Number(0.6)),
        )
        .unwrap();
    rig.settle();
    // The view, the Playback tab and the results inspector.
    assert_eq!(
        count(&rig, super::out_of_date::TITLE),
        3,
        "{:?}",
        rig.texts()
    );
    assert!(rig.shows(
        "Operation 'Finish' has changed since this simulation. Simulate again to see the effect."
    ));
    assert!(rig.shows("Results of the out-of-date simulation:"));
    let label = rig.shell.overview.primary(Step::Simulate).label;
    assert_eq!(label, "Generate and simulate op 1");
    // Three banner buttons and the footer's primary.
    assert_eq!(count(&rig, &label), 4);
    assert!(rig.shell.play.has_run());
    for text in ["Go to move", "Speed", "Whole program"] {
        assert!(rig.shows(text), "{text}");
    }
    let (drawn, asked) = dock_heights(&rig);
    assert!((drawn - asked).abs() < 0.5, "{drawn} of {asked}");

    // Simulating again from the dock's banner brings the result up to date.
    rig.shell.dock_tab = DockTab::Playback;
    rig.settle();
    let at = rig
        .text_rects()
        .into_iter()
        .filter(|(t, _)| *t == label)
        .map(|(_, r)| r)
        .max_by(|a, b| a.top().total_cmp(&b.top()))
        .unwrap();
    rig.click(at.center());
    assert!(rig.shell.run.is_running(), "the banner's button runs it");
    until_simulated_again(&mut rig);
    assert!(!rig.shows(super::out_of_date::TITLE));
    assert!(!rig.shows("Results of the out-of-date simulation:"));
    assert!(rig.shows("Go to move"));
}

#[test]
fn req_nfr_12_req_ui_11_turning_off_an_operation_in_simulate_offers_to_simulate_again() {
    // Issue 260: turning off an operation drops its playback (decision 0172
    // item 3), and the dock offers to simulate again in its place instead of
    // going empty; the results stay, dimmed and marked.
    let mut rig = Rig::cube("sim-stale-off", None);
    let first = rig.shell.app.as_ref().unwrap().project().operations()[0].id;
    rig.shell.tree_action(super::TreeAction::Duplicate(first));
    rig.settle();
    let second = rig.shell.app.as_ref().unwrap().project().operations()[1].id;
    let name = rig.shell.app.as_ref().unwrap().project().operations()[1]
        .name
        .clone();
    rig.shell.generate(true);
    let deadline = Instant::now() + Duration::from_secs(120);
    while rig.shell.run.last().is_none_or(|r| r.simulation.is_none()) {
        rig.settle();
        assert!(Instant::now() < deadline, "timed out");
    }
    rig.shell.open_step(Step::Simulate);
    until_simulated_again(&mut rig);
    assert!(!rig.shows(super::out_of_date::TITLE));

    rig.shell
        .app
        .as_mut()
        .unwrap()
        .execute(openchuck_app::Command::SetOperationEnabled {
            operation: second,
            enabled: false,
        })
        .unwrap();
    rig.settle();
    assert!(!rig.shell.play.has_run(), "decision 0172 item 3");
    assert_eq!(
        count(&rig, super::out_of_date::TITLE),
        3,
        "{:?}",
        rig.texts()
    );
    assert!(rig.shows(&format!(
        "Operation '{name}' was turned off. Simulate again to see the effect."
    )));
    assert!(!rig.shows("Run the simulation to play the cut."));
    assert!(rig.shows("Results of the out-of-date simulation:"));
    assert_eq!(
        rig.shell.overview.primary(Step::Simulate).kind,
        PrimaryKind::RunSimulation
    );
    let (drawn, asked) = dock_heights(&rig);
    assert!((drawn - asked).abs() < 0.5, "{drawn} of {asked}");

    rig.shell.primary();
    until_simulated_again(&mut rig);
    assert!(!rig.shows(super::out_of_date::TITLE));
    let made: Vec<_> = rig
        .shell
        .run
        .last()
        .unwrap()
        .groups
        .iter()
        .map(|g| g.operation)
        .collect();
    assert_eq!(made, [first]);
}

#[test]
fn req_nfr_12_turning_off_the_only_operation_says_so_without_a_button() {
    let mut rig = simulated("sim-stale-only");
    let op = rig.shell.app.as_ref().unwrap().project().operations()[0].id;
    rig.shell
        .app
        .as_mut()
        .unwrap()
        .execute(openchuck_app::Command::SetOperationEnabled {
            operation: op,
            enabled: false,
        })
        .unwrap();
    rig.settle();
    assert!(
        rig.shows("Operation 'Finish' was turned off. Turn on an operation to simulate again.")
    );
    assert!(!rig.shows("Run simulation"));
}

#[test]
fn req_ui_16_req_sim_04_the_playback_tab_keeps_its_place_while_the_simulation_runs() {
    // Issue 215 (OI-571): the owner sees the playback box move down when the
    // simulation runs. While a run is going and its playback is prepared the
    // tab holds less, and the dock must keep its size and place throughout.
    let mut rig = simulated("sim-runs-in-place");
    rig.shell.dock_tab = DockTab::Playback;
    rig.settle();
    let top = |rig: &Rig| {
        rig.text_rects()
            .into_iter()
            .find(|(t, _)| t == "Skip stock simulation")
            .map(|(_, r)| r.top())
    };
    let before = top(&rig).unwrap();
    rig.shell.generate(true);
    let deadline = Instant::now() + Duration::from_secs(120);
    let mut frames = 0;
    while rig.shell.run.is_running() || rig.shell.play.pose().is_none() || frames < 5 {
        rig.frame(Vec::new());
        frames += 1;
        let (drawn, asked) = dock_heights(&rig);
        assert!(
            (drawn - asked).abs() < 0.5,
            "frame {frames}: {drawn} of {asked}"
        );
        if let Some(at) = top(&rig) {
            assert!(at <= before + 0.5, "frame {frames}: {at} below {before}");
        }
        std::thread::sleep(Duration::from_millis(5));
        assert!(Instant::now() < deadline, "timed out");
    }
    rig.settle();
    assert_eq!(top(&rig), Some(before));
}

/// Checks the Playback tab as a whole (issue 262): the strip that does not
/// scroll (the transport row, "Skip stock simulation" and the timeline) is
/// wholly in the dock, and so is every row in `rows`; no two texts overlap;
/// and a scroll bar, where there is one, lies over neither a text nor the
/// timeline; and the dock keeps the height it was given. `what` names the
/// case in a failure.
fn check_dock(rig: &Rig, rows: &[&str], what: &str) {
    let dock = egui::PanelState::load(&rig.ctx, egui::Id::new("dock"))
        .expect("the dock was drawn")
        .outer_rect;
    let (drawn, asked) = dock_heights(rig);
    assert!(
        (drawn - asked).abs() < 0.5,
        "{what}: the dock grew to {drawn} of {asked}"
    );
    let painted: Vec<_> = rig
        .painted()
        .into_iter()
        .filter(|(_, r)| dock.contains_rect(r.shrink(0.5)))
        .collect();
    let texts: Vec<_> = painted
        .iter()
        .filter_map(|(t, r)| t.as_ref().map(|t| (t.as_str(), *r)))
        .filter(|(t, _)| !t.trim().is_empty())
        .collect();
    for row in ["⏮", "⏭", "Skip stock simulation"].iter().chain(rows) {
        assert!(
            texts.iter().any(|(t, _)| t == row),
            "{what}: {row:?} is not wholly in the dock: {texts:?}"
        );
    }
    let timeline: Vec<egui::Rect> = painted
        .iter()
        .filter(|(t, r)| t.is_none() && (r.height() - 26.0).abs() < 1.0 && r.width() > 40.0)
        .map(|(_, r)| *r)
        .collect();
    assert!(
        !timeline.is_empty(),
        "{what}: the timeline is not wholly in the dock"
    );
    for (i, (a, ra)) in texts.iter().enumerate() {
        for (b, rb) in texts.iter().skip(i + 1) {
            assert!(
                !ra.shrink(1.0).intersects(rb.shrink(1.0)),
                "{what}: {a:?} {ra:?} overlaps {b:?} {rb:?}"
            );
        }
    }
    let bars: Vec<egui::Rect> = painted
        .iter()
        .filter(|(t, r)| t.is_none() && r.width() < 12.0 && r.height() > 40.0)
        .map(|(_, r)| *r)
        .collect();
    for bar in &bars {
        for (t, r) in &texts {
            assert!(
                !bar.intersects(r.shrink(0.5)),
                "{what}: the scroll bar lies over {t:?}"
            );
        }
        for segment in &timeline {
            assert!(
                !segment.intersects(bar.shrink(0.5)),
                "{what}: the timeline {segment:?} runs under the scroll bar {bar:?}"
            );
        }
    }
}

#[test]
fn req_ui_16_req_sim_04_the_playback_dock_lays_out_whole_in_every_state() {
    // Issue 262 (seen maximised at 1920 x 1080): at the default dock height,
    // at 100% and 120%, every playback control fits without scrolling; with
    // the out-of-date banner (issue 260) or in a shorter dock the transport
    // and the timeline stay in their strip and the rest scrolls, its bar in a
    // strip of its own; a playback window (issue 205) changes nothing.
    let all = [
        "Speed",
        "Go to move",
        "Turn part with A",
        "Colour the part by deviation",
        "Hide path outside window",
    ];
    for zoom in [1.0, 1.2] {
        let mut rig = simulated(&format!("sim-dock-layout-{}", (zoom * 10.0) as u32));
        crate::theme::install_style(&rig.ctx, crate::theme::Theme::Dark);
        rig.size = egui::vec2(1920.0, 1080.0);
        rig.ctx.set_zoom_factor(zoom);
        rig.shell.dock_tab = DockTab::Playback;
        rig.settle();
        assert!((rig.shell.settings.dock.size - 260.0).abs() < 1e-6);
        check_dock(&rig, &all, &format!("{zoom}: current"));

        rig.shell
            .play
            .set_window(Some(openchuck_app::playback::MoveWindow {
                first: 10,
                last: 200,
            }));
        rig.settle();
        check_dock(&rig, &all, &format!("{zoom}: window"));

        let op = rig.shell.app.as_ref().unwrap().project().operations()[0].id;
        rig.shell
            .app
            .as_mut()
            .unwrap()
            .set_parameter(
                op,
                "stepover",
                Some(openchuck_app::project::ParameterValue::Number(0.6)),
            )
            .unwrap();
        rig.settle();
        assert!(rig.shows(super::out_of_date::TITLE));
        check_dock(
            &rig,
            &[super::out_of_date::TITLE],
            &format!("{zoom}: out of date"),
        );

        rig.shell.undo();
        rig.shell.settings.dock.size = 200.0;
        rig.settle();
        assert!(!rig.shows(super::out_of_date::TITLE));
        check_dock(&rig, &[], &format!("{zoom}: short dock"));
        assert!(
            !rig.bar_rects().is_empty(),
            "{zoom}: the rest scrolls, with a bar to say so"
        );
    }
}
