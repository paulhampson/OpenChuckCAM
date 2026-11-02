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
use openchuck_viewport::Layer;

use super::frame_tests::Rig;
use super::*;
use crate::import_panel::ImportSession;
use crate::results_panel::bin_slot;

#[path = "simulate_render_tests.rs"]
mod rendering;

/// The cube project, simulated, on the Simulate step.
fn simulated(name: &str) -> Rig {
    simulate(Rig::cube(name, None))
}

/// `rig`'s project generated with a simulation, open on Simulate with its
/// playback ready.
fn simulate(mut rig: Rig) -> Rig {
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
fn req_ui_06_req_ui_10_req_sim_04_simulate_row_opens_its_inspector_and_preserves_return_state() {
    for (keyboard, width, height) in [
        (false, 1920.0, 1080.0),
        (true, 1920.0, 1080.0),
        (false, 900.0, 700.0),
        (true, 900.0, 700.0),
    ] {
        let mut rig = simulated(&format!("simui-row-{keyboard}-{width}"));
        rig.size = egui::vec2(width, height);
        let operation = rig.shell.app.as_ref().unwrap().project().operations()[0].id;
        rig.shell
            .play
            .set_window(Some(openchuck_app::playback::MoveWindow {
                first: 20,
                last: 150,
            }));
        rig.shell.viewport.camera.orbit(13.0, 7.0);
        rig.shell.settings.inspector.collapsed = true;
        rig.settle();
        let cursor = rig.shell.play.pose().unwrap().cursor;
        let camera = rig.shell.viewport.camera;
        if keyboard {
            let row = rig
                .ctx
                .data(|d| {
                    d.get_temp::<egui::Id>(egui::Id::unique(("operation row focus", operation)))
                })
                .unwrap();
            rig.ctx.memory_mut(|m| m.request_focus(row));
            rig.press(Key::Enter, Modifiers::NONE);
        } else {
            rig.click(at(&rig, "1 Finish"));
        }
        rig.settle();
        assert_eq!(rig.shell.step, Step::Operations, "row activation navigates");
        assert_eq!(rig.shell.selected, Some(Selection::Operation(operation)));
        assert!(!rig.shell.settings.inspector.collapsed);
        assert!(!rig.shell.collapsed(super::frame::Region::Inspector));
        let inspector = rig.panel_state("properties").unwrap().outer_rect;
        assert!(rig.painted().iter().any(|(text, whole, seen)| {
            text.as_deref() == Some("Finish")
                && inspector.contains_rect(*whole)
                && seen.contains_rect(whole.shrink(0.5))
        }));
        if height >= 1080.0 {
            assert!(rig.shows("Stepover"));
        }
        assert_eq!(rig.shell.play.window(), None, "accepted 0169 reset");
        // Operations may use its own view while the Simulate camera is kept.
        rig.shell.viewport.camera.orbit(29.0, 11.0);
        rig.shell.open_step(Step::Simulate);
        rig.settle();
        assert_eq!(rig.shell.play.pose().unwrap().cursor, cursor);
        assert_eq!(rig.shell.viewport.camera, camera);
        rig.shell.open_step(Step::Operations);
        rig.shell
            .app
            .as_mut()
            .unwrap()
            .set_parameter(
                operation,
                "stepover",
                Some(openchuck_app::project::ParameterValue::Number(0.6)),
            )
            .unwrap();
        rig.shell.open_step(Step::Simulate);
        rig.settle();
        assert_eq!(count(&rig, super::out_of_date::TITLE), 1);
        assert_eq!(rig.shell.play.pose().unwrap().cursor, cursor);
    }
}

#[test]
fn req_ui_07_req_ui_10_simulate_row_navigation_uses_pending_edit_cancel_and_keep() {
    let mut rig = simulated("simui-row-pending");
    rig.size = egui::vec2(1920.0, 1080.0);
    rig.settle();
    let operation = rig.shell.app.as_ref().unwrap().project().operations()[0].id;
    let selected = rig.shell.selected;
    super::pending_edit_tests::type_unconfirmed(&mut rig, "Cell size", "0.05");
    rig.click(at(&rig, "1 Finish"));
    rig.settle();
    assert!(rig.shows("Unconfirmed changes"));
    assert_eq!(rig.shell.step, Step::Simulate);
    rig.click(at(&rig, "Cancel"));
    rig.settle();
    assert_eq!(rig.shell.step, Step::Simulate);
    assert_eq!(rig.shell.selected, selected);
    assert_eq!(crate::pending_edits::entries(&rig.ctx).len(), 1);
    assert!(
        (rig.shell
            .app
            .as_ref()
            .unwrap()
            .project()
            .simulation()
            .cell
            .0
            - 0.1)
            .abs()
            < 1e-12
    );
    rig.click(at(&rig, "1 Finish"));
    rig.settle();
    rig.click(at(&rig, "Discard"));
    rig.settle();
    assert_eq!(rig.shell.step, Step::Operations);
    assert!(
        (rig.shell
            .app
            .as_ref()
            .unwrap()
            .project()
            .simulation()
            .cell
            .0
            - 0.1)
            .abs()
            < 1e-12
    );
    rig.shell.open_step(Step::Simulate);
    rig.settle();
    super::pending_edit_tests::type_unconfirmed(&mut rig, "Cell size", "0.05");
    rig.click(at(&rig, "1 Finish"));
    rig.settle();
    rig.click(at(&rig, "Keep"));
    rig.settle();
    assert_eq!(rig.shell.step, Step::Operations);
    assert_eq!(rig.shell.selected, Some(Selection::Operation(operation)));
    assert!(
        (rig.shell
            .app
            .as_ref()
            .unwrap()
            .project()
            .simulation()
            .cell
            .0
            - 0.05)
            .abs()
            < 1e-12
    );
    rig.shell.open_step(Step::Simulate);
    rig.settle();
    assert_eq!(count(&rig, super::out_of_date::TITLE), 1);
}

#[test]
fn req_ui_06_req_sim_01_row_controls_do_not_navigate_and_toolpath_visibility_combines() {
    let mut rig = simulated("simui-row-controls");
    let operation = rig.shell.app.as_ref().unwrap().project().operations()[0].id;
    let run = rig.shell.run.last().unwrap().clone();
    let click_control = |rig: &mut Rig, kind: &str| {
        let rect = rig
            .ctx
            .data(|d| d.get_temp::<egui::Rect>(egui::Id::unique((kind, operation))))
            .unwrap();
        rig.click(rect.center());
        rig.settle();
        assert_eq!(rig.shell.step, Step::Simulate, "{kind} must not navigate");
    };
    for overall in [true, false, true] {
        rig.shell
            .viewport
            .set_simulate_show(crate::viewport::SimulateShow {
                model: false,
                cut_part: true,
                toolpath: overall,
            });
        for shown in [false, true] {
            click_control(&mut rig, "operation eye");
            assert_eq!(
                rig.shell
                    .app
                    .as_ref()
                    .unwrap()
                    .project()
                    .path_shown(operation),
                shown
            );
            assert_eq!(rig.shell.play.draws_group(0), shown);
            assert_eq!(rig.shell.viewport.visibility().toolpaths, overall);
            assert!(Arc::ptr_eq(rig.shell.run.last().unwrap(), &run));
            assert!(
                rig.shell.out_of_date.reason().is_none(),
                "display changes keep results current"
            );
        }
    }
    click_control(&mut rig, "operation enable");
    assert!(
        !rig.shell
            .app
            .as_ref()
            .unwrap()
            .project()
            .operation(operation)
            .unwrap()
            .enabled
    );
    assert!(
        !rig.shell.play.has_run(),
        "disabled operations keep the accepted run-drop rule"
    );
    rig.shell.undo();
    rig.settle();
    assert_eq!(rig.shell.step, Step::Simulate);
    // The row's context menu duplicates without activating its row.
    let row = at(&rig, "1 Finish");
    rig.frame(vec![
        Event::PointerMoved(row),
        Event::PointerButton {
            pos: row,
            button: egui::PointerButton::Secondary,
            pressed: true,
            modifiers: Modifiers::NONE,
        },
    ]);
    rig.frame(vec![Event::PointerButton {
        pos: row,
        button: egui::PointerButton::Secondary,
        pressed: false,
        modifiers: Modifiers::NONE,
    }]);
    rig.frame(Vec::new());
    rig.click(at(&rig, "Duplicate"));
    rig.settle();
    assert_eq!(rig.shell.step, Step::Simulate);
    assert_eq!(
        rig.shell.app.as_ref().unwrap().project().operations().len(),
        2
    );
    let order: Vec<_> = rig
        .shell
        .app
        .as_ref()
        .unwrap()
        .project()
        .operations()
        .iter()
        .map(|op| op.id)
        .collect();
    // Select the second operation without activating its row, then use the
    // actual reorder shortcut. Verify that it reorders, not just that the
    // step happens to remain Simulate.
    rig.shell.select(Selection::Operation(order[1]));
    rig.press(Key::ArrowUp, Modifiers::ALT);
    rig.settle();
    assert_eq!(
        rig.shell.app.as_ref().unwrap().project().operations()[0].id,
        order[1]
    );
    assert_eq!(
        rig.shell.step,
        Step::Simulate,
        "reorder shortcut must not navigate"
    );
    let handle = |id| {
        rig.ctx
            .data(|d| d.get_temp::<egui::Rect>(egui::Id::unique(("operation reorder", id))))
            .unwrap()
    };
    let from = handle(order[1]).center();
    let to = handle(order[0]).center() + egui::vec2(0.0, 8.0);
    rig.frame(vec![
        Event::PointerMoved(from),
        Event::PointerButton {
            pos: from,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::NONE,
        },
    ]);
    rig.frame(vec![Event::PointerMoved(to)]);
    rig.frame(Vec::new());
    rig.frame(vec![Event::PointerButton {
        pos: to,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    }]);
    rig.settle();
    assert_eq!(
        rig.shell.step,
        Step::Simulate,
        "the drag handle must not activate its row"
    );
    assert_eq!(
        rig.shell.app.as_ref().unwrap().project().operations()[1].id,
        order[1],
        "the actual drag reordered the operations"
    );
}

#[test]
fn req_gc_07_req_nfr_06_display_and_navigation_preserve_project_paths_and_posted_bytes() {
    let mut rig = simulated("simui-output-preserved");
    rig.shell
        .app
        .as_mut()
        .unwrap()
        .set_post(Some(openchuck_app::PostSelection::parse("linuxcnc")));
    rig.shell.generate(true);
    rig.settle();
    let before = rig.shell.run.last().unwrap().clone();
    assert!(!before.gcode.is_empty(), "compare actual posted output");
    let project = rig.shell.app.as_ref().unwrap().project().clone();
    for bits in 1u8..8 {
        rig.shell
            .viewport
            .set_simulate_show(crate::viewport::SimulateShow {
                model: bits & 1 != 0,
                cut_part: bits & 2 != 0,
                toolpath: bits & 4 != 0,
            });
        rig.frame(Vec::new());
    }
    rig.shell.viewport.set_cut_part_solid(true);
    rig.click(at(&rig, "Deviation colours"));
    let deadline = Instant::now() + Duration::from_secs(120);
    while rig.shell.viewport.bands().is_none() {
        rig.frame(Vec::new());
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    rig.click(at(&rig, "Deviation colours"));
    rig.shell.viewport.set_cut_part_solid(false);
    rig.click(at(&rig, "1 Finish"));
    rig.settle();
    assert_eq!(rig.shell.step, Step::Operations);
    rig.shell.open_step(Step::Simulate);
    rig.settle();
    assert_eq!(rig.shell.app.as_ref().unwrap().project(), &project);
    // Generate again from the unchanged project, rather than merely check
    // the retained run's identity. Compare neutral moves, posted bytes and
    // simulation values from the independent execution of the pipeline.
    rig.shell.generate(true);
    rig.settle();
    let after = rig.shell.run.last().unwrap();
    assert!(!Arc::ptr_eq(&before, after));
    assert_eq!(after.toolpath, before.toolpath);
    assert_eq!(after.gcode, before.gcode);
    assert_eq!(after.simulation, before.simulation);
    assert_eq!(rig.shell.app.as_ref().unwrap().project(), &project);
    assert_eq!(rig.shell.play.window(), None);
    assert_eq!(
        rig.shell.play.pose().unwrap().cursor.done,
        0,
        "a new run starts normally"
    );
}

#[test]
fn req_sim_05_req_ui_15_cut_part_opacity_defaults_to_translucent_and_changes_current_and_stale_surfaces()
 {
    let mut rig = simulated("simui-opacity");
    rig.size = egui::vec2(1920.0, 1080.0);
    let deadline = Instant::now() + Duration::from_secs(120);
    while !rig
        .shell
        .viewport
        .layer(openchuck_viewport::Layer::Stock)
        .is_some_and(|s| !s.meshes.is_empty())
    {
        rig.frame(Vec::new());
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(
        rig.shell
            .viewport
            .layer(openchuck_viewport::Layer::Stock)
            .unwrap()
            .meshes
            .iter()
            .all(|m| m.translucent)
    );
    let project = rig.shell.app.as_ref().unwrap().project().clone();
    rig.click(at(&rig, "Show ⏷"));
    rig.settle();
    assert!(
        rig.shows("Translucent") && rig.shows("Solid"),
        "opacity choices are beside the Show settings"
    );
    rig.click(at(&rig, "Solid"));
    rig.settle();
    assert!(
        rig.shell
            .viewport
            .layer(openchuck_viewport::Layer::Stock)
            .unwrap()
            .meshes
            .iter()
            .all(|m| !m.translucent && m.colour[3].to_bits() == 1.0_f32.to_bits())
    );
    assert_eq!(rig.shell.app.as_ref().unwrap().project(), &project);
    rig.press(Key::Escape, Modifiers::NONE);
    let operation = project.operations()[0].id;
    rig.shell
        .app
        .as_mut()
        .unwrap()
        .set_parameter(
            operation,
            "stepover",
            Some(openchuck_app::project::ParameterValue::Number(0.6)),
        )
        .unwrap();
    rig.settle();
    rig.click(at(&rig, "Deviation colours"));
    let deadline = Instant::now() + Duration::from_secs(120);
    while rig.shell.viewport.bands().is_none() {
        rig.frame(Vec::new());
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    let scene = rig
        .shell
        .viewport
        .layer(openchuck_viewport::Layer::Stock)
        .unwrap();
    assert!(
        scene.meshes.len() > 1,
        "the actual cut stock carries the colour bands"
    );
    assert!(
        scene
            .meshes
            .iter()
            .all(|m| !m.translucent && m.colour[3].to_bits() == 1.0_f32.to_bits())
    );
    assert_eq!(count(&rig, super::out_of_date::TITLE), 1);
    rig.click(at(&rig, "Show ⏷"));
    rig.settle();
    rig.click(at(&rig, "Translucent"));
    rig.settle();
    assert!(
        rig.shell
            .viewport
            .layer(openchuck_viewport::Layer::Stock)
            .unwrap()
            .meshes
            .iter()
            .all(|m| m.translucent && m.colour[3] < 1.0)
    );
    assert_eq!(count(&rig, super::out_of_date::TITLE), 1);
}

#[test]
fn req_nfr_04_req_ui_11_req_nfr_12_req_gc_15_preparation_failure_removes_progress_and_keeps_old_results_stale()
 {
    let mut rig = simulated("simui-progress-failed");
    let run = rig.shell.run.last().unwrap().clone();
    let operation = rig.shell.app.as_ref().unwrap().project().operations()[0].id;
    let app = rig.shell.app.as_mut().unwrap();
    let mut edited = app.project().operation(operation).unwrap().clone();
    edited.tool = None;
    app.execute(Command::Replace(openchuck_app::project::Item::Operation(
        edited,
    )))
    .unwrap();
    rig.shell.generate(true);
    rig.settle();
    assert!(
        rig.shell.run.last_error().is_some(),
        "the missing tool failed the real pipeline"
    );
    assert!(Arc::ptr_eq(&run, rig.shell.run.last().unwrap()));
    assert!(!rig.shows("Preparing simulation"));
    assert_eq!(count(&rig, super::out_of_date::TITLE), 1);
    let app = rig.shell.app.as_ref().unwrap();
    assert!(
        !app.export_checks(Some(&run), None, rig.shell.run.last_error())
            .can_write()
    );
}

#[test]
fn req_nfr_12_req_nfr_04_skipped_stock_progress_does_not_claim_to_simulate() {
    let mut rig = simulated("simui-skip-progress");
    rig.shell.tree_action(TreeAction::Command(Box::new(
        Command::SetSkipStockSimulation(true),
    )));
    let operation = rig.shell.app.as_ref().unwrap().project().operations()[0].id;
    rig.shell
        .app
        .as_mut()
        .unwrap()
        .set_parameter(
            operation,
            "stepover",
            Some(openchuck_app::project::ParameterValue::Number(0.6)),
        )
        .unwrap();
    rig.settle();
    rig.shell.generate(false);
    let deadline = Instant::now() + Duration::from_secs(120);
    let mut seen = false;
    while rig.shell.run.is_running() {
        rig.frame(Vec::new());
        if rig.shell.run.is_running() {
            assert!(rig.shows("Preparing toolpaths"));
            assert!(!rig.shows("Preparing simulation"));
            seen = true;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(seen);
    rig.settle();
    assert!(rig.shell.run.last().unwrap().simulation.is_none());
    assert!(!rig.shows("Preparing toolpaths"));
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
    // The viewport banner gets its layout frame before it is painted.
    rig.settle();
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
            source: egui::MouseWheelSource::Unknown,
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
            source: egui::MouseWheelSource::Unknown,
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
    assert!(!rig.shell.viewport.cut_part_solid());
    let before = rig.shell.app.as_ref().unwrap().project().clone();
    rig.shell.viewport.set_cut_part_solid(true);
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
    assert!(kept.cut_part_solid);
    assert_eq!(rig.shell.app.as_ref().unwrap().project(), &before);
    assert!(!kept.turn_part_with_a && kept.hide_outside_window);
    assert!(kept.show_model && !kept.show_cut_part && kept.show_toolpath);
    // A restart starts from them.
    let shell = Shell::with_settings(false, None, Some(file));
    assert!(!shell.play.turns_part_with_a());
    assert!(shell.play.hides_outside_window());
    assert!(shell.viewport.simulate_show().toolpath);
    assert!(shell.viewport.cut_part_solid());
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
    let drawn = rig
        .panel_state("dock")
        .expect("the dock was drawn")
        .outer_rect
        .height();
    (drawn, rig.shell.settings.dock.size)
}

/// How many times `needle` is painted.
fn count(rig: &Rig, needle: &str) -> usize {
    rig.texts().iter().filter(|(t, _)| t == needle).count()
}

#[test]
fn req_sim_05_req_nfr_12_deviation_colours_can_be_hidden_on_current_and_stale_results() {
    let mut rig = simulated("simui-colours");
    // Playback data and the cut-part checkpoints are separate jobs. Wait
    // for the stock mesh this test reads, not merely the timeline.
    let deadline = Instant::now() + Duration::from_secs(120);
    while !rig
        .shell
        .viewport
        .layer(openchuck_viewport::Layer::Stock)
        .is_some_and(|s| !s.meshes.is_empty())
    {
        rig.frame(Vec::new());
        assert!(Instant::now() < deadline, "the cut part was never ready");
        std::thread::sleep(Duration::from_millis(10));
    }
    for stale in [false, true] {
        if stale {
            let app = rig.shell.app.as_mut().unwrap();
            let operation = app.project().operations()[0].id;
            app.set_parameter(
                operation,
                "stepover",
                Some(openchuck_app::project::ParameterValue::Number(0.6)),
            )
            .unwrap();
            rig.settle();
        }
        assert!(
            rig.shows("Deviation colours"),
            "display choice stays accessible"
        );
        rig.click(at(&rig, "Deviation colours"));
        let deadline = Instant::now() + Duration::from_secs(120);
        while rig.shell.viewport.bands().is_none() {
            rig.frame(Vec::new());
            assert!(
                Instant::now() < deadline,
                "deviation colours did not appear"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        rig.settle();
        assert_eq!(count(&rig, super::out_of_date::TITLE), usize::from(stale));
        rig.click(at(&rig, "Deviation colours"));
        rig.frame(Vec::new());
        assert!(rig.shell.viewport.bands().is_none());
        assert_eq!(count(&rig, super::out_of_date::TITLE), usize::from(stale));
        let stock = rig.shell.viewport.layer(openchuck_viewport::Layer::Stock);
        assert!(
            stock.is_some_and(|s| !s.meshes.is_empty()),
            "the plain cut part stays drawn"
        );
        if stale {
            let app = rig.shell.app.as_ref().unwrap();
            let run = rig.shell.run.last().unwrap();
            assert!(!app.export_checks(Some(run), None, None).can_write());
        }
    }
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
    // Issue 297: one actionable message, on the viewport only.
    assert_eq!(
        count(&rig, super::out_of_date::TITLE),
        1,
        "{:?}",
        rig.texts()
    );
    assert!(rig.shows(
        "Operation 'Finish' has changed since this simulation. Simulate again to see the effect."
    ));
    assert!(rig.shows("Results of the out-of-date simulation:"));
    let label = "Run simulation again";
    assert_eq!(count(&rig, label), 1);
    assert!(rig.shell.play.has_run());
    for text in ["Go to move", "Speed", "Whole program"] {
        assert!(rig.shows(text), "{text}");
    }
    let (drawn, asked) = dock_heights(&rig);
    assert!((drawn - asked).abs() < 0.5, "{drawn} of {asked}");

    // The viewport action brings the result up to date.
    rig.shell.dock_tab = DockTab::Playback;
    rig.settle();
    let at = rig
        .text_rects()
        .into_iter()
        .filter(|(t, _)| t == label)
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
        1,
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
    // Skipping the stock simulation, the banner offers only the toolpaths,
    // and says so (OI-732).
    let skip = |rig: &mut Rig, on: bool| {
        rig.shell
            .app
            .as_mut()
            .unwrap()
            .execute(openchuck_app::Command::SetSkipStockSimulation(on))
            .unwrap();
        rig.settle();
    };
    skip(&mut rig, true);
    assert_eq!(
        rig.shell.overview.primary(Step::Simulate).kind,
        PrimaryKind::Generate
    );
    assert!(rig.shows(&format!(
        "Operation '{name}' was turned off. Generate the toolpaths again to see the effect."
    )));
    assert_eq!(
        count(&rig, "Generate toolpaths"),
        2,
        "one viewport action and the footer; stock simulation stays skipped"
    );
    skip(&mut rig, false);

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
    // The banner has no run button (decision 0238). Simulate's footer keeps
    // its simulation action in its place, disabled with the reason (decision
    // 0282 items 2 and 3).
    assert!(!rig.shows("Run simulation again"));
    let action = rig.shell.overview.simulation_action();
    assert_eq!(
        action.blocked.as_deref(),
        Some("Turn on an operation to simulate")
    );
    let footer = rig
        .visible_text_rects()
        .into_iter()
        .filter(|(t, _)| t == "Run simulation")
        .map(|(_, r)| r)
        .collect::<Vec<_>>();
    assert_eq!(footer.len(), 1, "only the footer's action");
    assert!(
        footer.first().unwrap().top() > rig.size.y - 48.0,
        "in the footer"
    );
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

#[test]
fn req_ui_15_req_ui_16_simulation_choices_are_adjacent_without_a_labelled_group() {
    let mut rig = simulated("simui-grouping");
    rig.size = egui::vec2(1920.0, 1080.0);
    rig.settle();
    let texts = rig.text_rects();
    let rect = |text| texts.iter().find(|(t, _)| t == text).unwrap().1;
    let skip = rect("Skip stock simulation");
    let turn = rect("Turn part with A");
    assert!(
        (skip.center().y - turn.center().y).abs() < 1.0,
        "both choices share one row"
    );
    assert!(
        turn.left() > skip.right(),
        "turn choice follows the skip choice"
    );
    let dock = rig.panel_state("dock").unwrap().outer_rect;
    assert!(dock.contains_rect(turn));

    // The regrouped controls must remain independently usable, including
    // the tool choice on the denser speed/display row.
    let project = rig.shell.app.as_ref().unwrap().project().clone();
    assert!(rig.shell.play.turns_part_with_a());
    rig.click(turn.center());
    rig.settle();
    assert!(!rig.shell.play.turns_part_with_a());
    assert!(!rig.shell.settings.simulate.turn_part_with_a);
    assert!(
        !rig.shell
            .viewport
            .layer(Layer::Tool)
            .unwrap()
            .meshes
            .is_empty()
    );
    rig.click(at(&rig, "Tool"));
    rig.settle();
    assert!(
        rig.shell
            .viewport
            .layer(Layer::Tool)
            .unwrap()
            .meshes
            .is_empty()
    );
    rig.click(at(&rig, "Tool"));
    rig.settle();
    assert!(
        !rig.shell
            .viewport
            .layer(Layer::Tool)
            .unwrap()
            .meshes
            .is_empty()
    );
    assert_eq!(rig.shell.app.as_ref().unwrap().project(), &project);

    rig.click(at(&rig, "Skip stock simulation"));
    rig.settle();
    assert!(
        rig.shell
            .app
            .as_ref()
            .unwrap()
            .project()
            .skip_stock_simulation()
    );
    assert!(!rig.shell.play.turns_part_with_a());
    rig.shell.undo();
    rig.settle();
    assert_eq!(rig.shell.app.as_ref().unwrap().project(), &project);
    assert!(!rig.shell.settings.simulate.turn_part_with_a);
}

#[test]
fn req_nfr_04_req_ui_16_req_sim_04_real_preparation_progress_initial_and_regeneration() {
    let mut rig = Rig::cube("simui-progress", None);
    rig.size = egui::vec2(1920.0, 1080.0);
    // Operations must have toolpaths before Simulate is available. Make
    // those without stock first, then exercise the first stock simulation.
    rig.shell.tree_action(TreeAction::Command(Box::new(
        Command::SetSkipStockSimulation(true),
    )));
    rig.shell.generate(false);
    rig.settle();
    rig.shell.tree_action(TreeAction::Command(Box::new(
        Command::SetSkipStockSimulation(false),
    )));
    rig.settle();
    rig.shell.open_step(Step::Simulate);
    assert_eq!(rig.shell.step, Step::Simulate);
    for regeneration in [false, true] {
        if regeneration {
            let operation = rig.shell.app.as_ref().unwrap().project().operations()[0].id;
            rig.shell
                .app
                .as_mut()
                .unwrap()
                .set_parameter(
                    operation,
                    "stepover",
                    Some(openchuck_app::project::ParameterValue::Number(0.6)),
                )
                .unwrap();
            rig.settle();
        }
        rig.shell.generate(true);
        let deadline = Instant::now() + Duration::from_secs(120);
        let mut saw_progress = false;
        let mut saw_numeric = false;
        while rig.shell.run.is_running() {
            rig.frame(Vec::new());
            if rig.shell.run.is_running() {
                assert!(
                    rig.shows("Preparing simulation"),
                    "progress is in Playback, initial/regeneration={regeneration}, step={:?}, tab={:?}, texts={:?}",
                    rig.shell.step,
                    rig.shell.dock_tab,
                    rig.texts()
                );
                saw_progress = true;
                if let Some(entry) = rig
                    .shell
                    .jobs
                    .running()
                    .iter()
                    .find(|e| Some(e.id) == rig.shell.run.job_id())
                    && let Some(percent) = entry.percent()
                {
                    assert!(
                        rig.shows(&format!("Preparing simulation · {percent} %")),
                        "display uses the pipeline's actual progress"
                    );
                    saw_numeric = true;
                }
                let dock = rig.panel_state("dock").unwrap().outer_rect;
                let progress = rig
                    .text_rects()
                    .into_iter()
                    .find(|(t, _)| t.starts_with("Preparing simulation"))
                    .unwrap()
                    .1;
                assert!(
                    dock.contains_rect(progress),
                    "progress stays inside Playback"
                );
                let position = rig
                    .painted()
                    .into_iter()
                    .filter(|(t, whole, _)| {
                        t.is_none()
                            && dock.contains_rect(*whole)
                            && (whole.height() - 26.0).abs() < 1.0
                            && whole.width() > 100.0
                    })
                    .map(|(_, whole, _)| whole)
                    .max_by(|a, b| a.width().total_cmp(&b.width()))
                    .unwrap();
                assert!(
                    progress.top() >= position.bottom()
                        && progress.bottom() < position.bottom() + 28.0,
                    "progress directly follows the position bar"
                );
                let (drawn, asked) = dock_heights(&rig);
                assert!((drawn - asked).abs() < 0.5);
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(
            saw_progress && saw_numeric,
            "the real run reported preparation progress: seen={saw_progress}, numeric={saw_numeric}, regeneration={regeneration}"
        );
        loop {
            rig.frame(Vec::new());
            if rig.shell.play.pose().is_some()
                && !rig.shows("Preparing simulation")
                && !rig.shows("Preparing the cut part")
            {
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(!rig.shows("Preparing simulation"), "ready removes progress");
    }
}

/// Checks the Playback tab as a whole (issue 262): the strip that does not
/// scroll (the transport row, "Skip stock simulation" and the timeline) is
/// wholly in the dock, and so is every row in `rows`; no two texts overlap;
/// and a scroll bar, where there is one, lies over neither a text nor the
/// timeline; and the dock keeps the height it was given. `what` names the
/// case in a failure.
fn check_dock(rig: &Rig, rows: &[&str], what: &str) {
    let dock = rig
        .panel_state("dock")
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
        .filter(|(_, whole, _)| dock.contains_rect(whole.shrink(0.5)))
        .collect();
    let texts: Vec<_> = painted
        .iter()
        .filter_map(|(t, _, r)| t.as_ref().map(|t| (t.as_str(), *r)))
        .filter(|(t, _)| !t.trim().is_empty())
        .collect();
    // A required control is wholly shown: in the dock, and not clipped by a
    // scroll area inside it.
    let whole = |whole: &egui::Rect, seen: &egui::Rect| seen.contains_rect(whole.shrink(0.5));
    for row in ["⏮", "⏭", "Skip stock simulation"].iter().chain(rows) {
        assert!(
            painted
                .iter()
                .any(|(t, w, r)| t.as_deref() == Some(*row) && whole(w, r)),
            "{what}: {row:?} is not wholly in the dock: {texts:?}"
        );
    }
    let timeline: Vec<egui::Rect> = painted
        .iter()
        .filter(|(t, w, r)| {
            t.is_none() && whole(w, r) && (r.height() - 26.0).abs() < 1.0 && r.width() > 40.0
        })
        .map(|(_, _, r)| *r)
        .collect();
    assert!(
        !timeline.is_empty(),
        "{what}: the timeline is not wholly in the dock"
    );
    assert!(
        !texts.iter().any(|(t, _)| *t == super::out_of_date::TITLE),
        "{what}: the dock must not duplicate the viewport stale message"
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
        .filter(|(t, _, r)| t.is_none() && r.width() < 12.0 && r.height() > 40.0)
        .map(|(_, _, r)| *r)
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
    // Fixed physical windows at 100% and 120%: the expanded dock keeps its
    // bounds, progress never covers controls, and the viewport stale message
    // is not repeated here. Below the supported logical minimum, first use
    // the existing small-window rule and reopen the dock as the user can.
    let all = [
        "Speed",
        "Go to move",
        "Turn part with A",
        "Hide path outside window",
    ];
    for (width, height, zoom) in [
        (1366.0, 768.0, 1.0),
        (1366.0, 768.0, 1.2),
        (1440.0, 900.0, 1.0),
        (1440.0, 900.0, 1.2),
        (1920.0, 1080.0, 1.0),
        (1920.0, 1080.0, 1.2),
    ] {
        let mut rig = simulated(&format!("sim-dock-layout-{width}-{}", (zoom * 10.0) as u32));
        crate::theme::install_style(&rig.ctx, crate::theme::Theme::Dark);
        // RawInput uses logical points. Keep the physical window fixed as
        // scaling increases, just as the native integration does.
        rig.size = egui::vec2(width / zoom, height / zoom);
        rig.ctx.set_zoom_factor(zoom);
        rig.shell.dock_tab = DockTab::Playback;
        rig.settle();
        if rig.size.x < 1366.0 || rig.size.y < 768.0 {
            assert!(rig.shell.collapsed(super::frame::Region::Dock));
            rig.press(egui::Key::Num3, egui::Modifiers::COMMAND);
            rig.settle();
            assert!(!rig.shell.collapsed(super::frame::Region::Dock));
        }
        assert!((rig.shell.settings.dock.size - 260.0).abs() < 1e-6);
        check_dock(&rig, &all, &format!("{zoom}: current"));

        let ready = |rig: &mut Rig| {
            let deadline = Instant::now() + Duration::from_secs(120);
            while rig
                .shell
                .play
                .preparation_progress(&rig.shell.jobs, None, false)
                .is_some()
            {
                rig.frame(Vec::new());
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(5));
            }
            rig.settle();
        };
        ready(&mut rig);
        check_dock(
            &rig,
            &all,
            &format!("{width}×{height}@{zoom}: ready without progress"),
        );
        let op = rig.shell.app.as_ref().unwrap().project().operations()[0].id;
        rig.shell
            .app
            .as_mut()
            .unwrap()
            .set_parameter(
                op,
                "stepover",
                Some(openchuck_app::project::ParameterValue::Number(0.7)),
            )
            .unwrap();
        rig.settle();
        rig.shell.generate(true);
        let deadline = Instant::now() + Duration::from_secs(120);
        let mut preparing = false;
        while rig.shell.run.is_running() {
            rig.frame(Vec::new());
            if rig.shell.run.is_running() {
                preparing = true;
                assert!(rig.shows("Preparing simulation"));
                check_dock(
                    &rig,
                    &all,
                    &format!("{width}×{height}@{zoom}: stale results and pipeline progress"),
                );
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(preparing);
        ready(&mut rig);
        assert!(!rig.shows("Preparing simulation"));
        check_dock(
            &rig,
            &all,
            &format!("{width}×{height}@{zoom}: regenerated without progress"),
        );

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
        check_dock(&rig, &all, &format!("{zoom}: out of date"));
        // Down to the shortest dock, the dock keeps its height and its
        // controls remain accessible through the existing scroll area.
        for dock in [200.0, 170.0] {
            rig.shell.settings.dock.size = dock;
            rig.settle();
            check_dock(&rig, &[], &format!("{zoom}: out of date, dock {dock}"));
        }
        rig.shell.settings.dock.size = openchuck_app::settings::region::DOCK.1;
        rig.settle();
        let (drawn, asked) = dock_heights(&rig);
        assert!(
            (drawn - asked).abs() < 0.5,
            "{zoom}: the shortest dock grew to {drawn} of {asked}"
        );
        assert!(
            !rig.bar_rects().is_empty(),
            "{zoom}: in the shortest dock everything scrolls, with a bar to say so"
        );

        rig.shell.undo();
        rig.shell.settings.dock.size = 200.0;
        rig.settle();
        assert!(!rig.shows(super::out_of_date::TITLE));
        check_dock(&rig, &[], &format!("{zoom}: short dock"));
        // The denser SIMUI layout may fit this height. The shortest case
        // above still requires and checks an accessible scroll bar.
    }
}

#[test]
fn req_sim_15_a_band_unticked_or_a_range_set_hides_its_deviation_and_says_so() {
    // Decision record 0163: a "Show" tick box on each histogram row and
    // "Show only from … to … mm", for the session; the part, the colour scale
    // and the histogram follow, the views say "Some bands are hidden", and
    // the worst values and counts stay those of the whole part.
    let mut rig = simulated("sim-bands");
    let colour = at(&rig, "Deviation colours");
    rig.click(colour);
    let deadline = Instant::now() + Duration::from_secs(120);
    while rig.shell.viewport.bands().is_none() {
        rig.frame(Vec::new());
        assert!(Instant::now() < deadline, "the bands were never ready");
        std::thread::sleep(Duration::from_millis(10));
    }
    rig.settle();
    let whole = rig.shell.viewport.bands().unwrap().clone();
    let triangles = |b: &openchuck_app::pathview::DeviationBands| -> usize {
        b.bands.iter().map(|(_, t)| t.len()).sum()
    };
    assert!(!rig.shows(crate::results_panel::SOME_HIDDEN));
    let run = rig.shell.run.last().unwrap().clone();
    let summary = crate::results_panel::summarise(&run);
    let bins = &summary[0].histogram;
    // Untick the row of the bin that holds the most triangles.
    let tolerance = whole.tolerance;
    let slot_of = |v: &Option<(f64, bool)>| v.map(|(v, _)| bin_slot(v, tolerance));
    let slot = (0..crate::results_panel::SLOTS)
        .max_by_key(|s| {
            whole
                .values
                .iter()
                .filter(|v| slot_of(v) == Some(*s))
                .count()
        })
        .unwrap();
    let in_slot = whole
        .values
        .iter()
        .filter(|v| slot_of(v) == Some(slot))
        .count();
    assert!(in_slot > 0);
    let (i, bin) = bins
        .iter()
        .enumerate()
        .find(|(_, b)| b.slot == slot)
        .unwrap();
    let label = crate::results_panel::bin_range(
        bin,
        i == 0 && summary[0].overcut_is_bound,
        i == bins.len() - 1 && summary[0].left_is_bound,
    );
    let row = at(&rig, &label);
    rig.click(row);
    rig.frame(Vec::new());
    assert!(rig.shell.results.filter.hidden[slot]);
    // Exactly the triangles in that bin keep the part's colour.
    let shown = rig.shell.viewport.bands().unwrap().clone();
    assert_eq!(triangles(&shown), triangles(&whole) - in_slot);
    // The inspector and the colour scale say so; the numbers do not move.
    rig.frame(Vec::new());
    assert!(rig.shows("Some bands are hidden."), "the inspector says so");
    assert_eq!(
        rig.shell.viewport.band_filter, rig.shell.results.filter,
        "the colour scale follows the choice"
    );
    assert_eq!(crate::results_panel::summarise(&run), summary);
    // A range narrows it further: only material left from 0.2 mm, typed in
    // the "from" field and taken when the field is left.
    let label = rig
        .text_rects()
        .into_iter()
        .find(|(t, _)| t == "Show only from")
        .expect("the range is shown")
        .1;
    rig.click(egui::pos2(label.right() + 30.0, label.center().y));
    rig.frame(vec![Event::Text("0.2".into())]);
    let away = at(&rig, "DEVIATION DISTRIBUTION");
    rig.click(away);
    rig.frame(Vec::new());
    assert_eq!(rig.shell.results.filter.from, Some(0.2));
    let ranged = rig.shell.viewport.bands().unwrap().clone();
    let expected = whole
        .values
        .iter()
        .filter(|v| slot_of(v).is_some_and(|s| s != slot) && v.is_some_and(|(v, _)| v >= 0.2))
        .count();
    let unmeasured = whole.values.iter().filter(|v| v.is_none()).count();
    assert_eq!(triangles(&ranged), expected + unmeasured);
    // A start above the end is refused, and the range stays.
    rig.shell.results.set_range_text("0.5", "0.1");
    assert!(rig.shell.results.range_error.is_some());
    assert_eq!(rig.shell.results.filter.from, Some(0.2));
    // So is a value that is not a number.
    rig.shell.results.set_range_text("two", "");
    assert!(rig.shell.results.range_error.is_some());
    assert_eq!(rig.shell.results.filter.from, Some(0.2));
    // The choice lasts for the session: a new run keeps it.
    rig.shell.generate(true);
    rig.settle();
    assert!(rig.shell.results.filter.hides_any());
    // "Show all" shows every band again.
    rig.frame(Vec::new());
    // The inspector's button, on the range's row (the tree has one too).
    let rects = rig.text_rects();
    let row = rects
        .iter()
        .find(|(t, _)| t == "Show only from")
        .expect("the range is shown")
        .1;
    let show_all = rects
        .iter()
        .find(|(t, r)| {
            t == "Show all" && r.left() > row.left() && (r.center().y - row.center().y).abs() < 30.0
        })
        .expect("Show all beside the range")
        .1
        .center();
    rig.click(show_all);
    rig.frame(Vec::new());
    assert!(!rig.shell.results.filter.hides_any());
    assert!(rig.shell.results.range_error.is_none());
    rig.frame(Vec::new());
    assert!(!rig.shows("Some bands are hidden."));
}

#[test]
fn req_sim_05_req_sim_15_a_bore_no_tool_reaches_shows_on_the_cut_part_and_can_be_hidden() {
    // Issue 478: a tube whose bore lies on the rotary axis leaves the bore
    // full of stock. The run warns once; the cut part's faces over the bore
    // are coloured Unreachable (not Not measured); the inspector counts it;
    // and its own "Show" tick box hides it, saying so, while the counts stay
    // those of the whole part.
    let mut rig = simulate(Rig::tube("sim-bore"));
    let run = rig.shell.run.last().unwrap().clone();
    let warnings: Vec<_> = run
        .notes
        .iter()
        .filter(|n| n.contains(openchuck_app::simulation::ENCLOSED_WORDS))
        .collect();
    assert_eq!(warnings.len(), 1, "{:?}", run.notes);
    // In the Messages panel too, as a warning on the setup.
    assert!(rig.shell.overview.messages.iter().any(|m| {
        m.severity == openchuck_app::overview::Severity::Warning
            && m.text.contains(openchuck_app::simulation::ENCLOSED_WORDS)
    }));
    let colour = at(&rig, "Deviation colours");
    rig.click(colour);
    let deadline = Instant::now() + Duration::from_secs(120);
    while rig.shell.viewport.bands().is_none() {
        rig.frame(Vec::new());
        assert!(Instant::now() < deadline, "the bands were never ready");
        std::thread::sleep(Duration::from_millis(10));
    }
    rig.settle();
    let unreachable = |rig: &Rig| {
        rig.shell.viewport.bands().map_or(0, |b| {
            b.bands
                .iter()
                .filter(|(band, _)| *band == openchuck_app::pathview::DeviationBand::Unreachable)
                .map(|(_, t)| t.len())
                .sum::<usize>()
        })
    };
    assert!(
        unreachable(&rig) > 0,
        "the faces over the bore are coloured"
    );
    assert!(rig.shows("Unreachable (enclosed)"));
    let summary = crate::results_panel::summarise(&run);
    assert!(summary[0].unreachable_samples.is_some_and(|n| n > 0));
    assert!(!rig.shows(crate::results_panel::SOME_HIDDEN));
    let tick = at(&rig, "Show unreachable (enclosed)");
    rig.click(tick);
    rig.frame(Vec::new());
    assert!(rig.shell.results.filter.hide_unreachable);
    assert_eq!(unreachable(&rig), 0, "hidden on the cut part");
    rig.frame(Vec::new());
    assert!(rig.shows("Some bands are hidden."), "the inspector says so");
    assert_eq!(
        crate::results_panel::summarise(&run),
        summary,
        "the counts stay"
    );
}
