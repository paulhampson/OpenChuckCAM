// SPDX-License-Identifier: AGPL-3.0-or-later
//! The Simulate step and the import window, drawn whole without a window
//! (slice 22 lane X6; decision records 0114 C14, C17, C20 and 0125): the
//! results inspector with its resolution control (SIM-05, SIM-07), the
//! playback dock (SIM-04), the tree's eye buttons and the import window
//! (IMP-01, IMP-03, IMP-04, IMP-05).

#![allow(clippy::unwrap_used)]

use std::time::{Duration, Instant};

use eframe::egui::{Key, Modifiers};
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
    assert!(summary[0].histogram.iter().sum::<usize>() > 0);
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

#[test]
fn req_sim_04_the_dock_plays_with_space_and_speeds_and_the_tree_has_eye_buttons() {
    let mut rig = simulated("sim-dock");
    assert_eq!(rig.shell.dock_tab, DockTab::Playback);
    for text in ["Speed", "1000×", "10 000×", "Whole program", "Go to move"] {
        assert!(rig.shows(text), "{text} is on screen");
    }
    assert!(rig.shows("The simulation results do not change"));
    assert!(!rig.shows("Max"), "there is no Max speed");
    // Space plays and pauses.
    assert!(!rig.shell.play.is_playing());
    rig.press(Key::Space, Modifiers::NONE);
    assert!(rig.shell.play.is_playing());
    rig.press(Key::Space, Modifiers::NONE);
    assert!(!rig.shell.play.is_playing());
    // The eye button hides one operation in the view only.
    let op = rig.shell.app.as_ref().unwrap().project().operations()[0].id;
    assert_eq!(rig.shell.play.is_shown(op), Some(true));
    rig.shell.tree_action(TreeAction::ToggleShown(op));
    assert_eq!(rig.shell.play.is_shown(op), Some(false));
    rig.frame(Vec::new());
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
        "The check reports problems and changes nothing.",
        "The file is not changed. Its path and hash are stored in the project.",
        "Add to project",
        "Rotary axis",
        "Size X",
        "Volume",
    ] {
        assert!(rig.shows(text), "{text} is on screen");
    }
    // Escape-free close: Cancel path leaves the project as it was.
    let before = rig.shell.app.as_ref().unwrap().project().models().len();
    rig.shell.import = None;
    rig.frame(Vec::new());
    assert_eq!(
        rig.shell.app.as_ref().unwrap().project().models().len(),
        before
    );
}
