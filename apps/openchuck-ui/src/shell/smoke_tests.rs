// SPDX-License-Identifier: AGPL-3.0-or-later
//! The UI smoke test (slice 22 step 3, lane X8; UI-01, UI-10, UI-16): the shell is drawn
//! whole, without a window, at 1366 × 768 and 100% (UI-16), through every step, panel and
//! dialog. A step, panel or dialog that panics when drawn, or that does not open, fails here.
//! What only a display can judge (contrast, pixel widths, tooltips on focus) is in the lane's
//! report, not here.

#![allow(clippy::unwrap_used, clippy::float_cmp)]

use eframe::egui::{self, Key, Modifiers};
use openchuck_app::overview::{SetupPart, Step};

use super::frame_tests::Rig;
use super::*;
use crate::tool_library_window::Purpose;

/// The reference window of UI-16.
const WINDOW: egui::Vec2 = egui::vec2(1366.0, 768.0);

fn rig(name: &str) -> Rig {
    let mut rig = Rig::cube(name, None);
    // Simulate waits for a run (UI-10).
    rig.shell.generate(true);
    rig.settle();
    rig.size = WINDOW;
    rig.frame(Vec::new());
    rig.frame(Vec::new());
    rig
}

/// Draws `frames` frames so that every area is laid out and shown.
fn draw(rig: &mut Rig) {
    for _ in 0..3 {
        rig.frame(Vec::new());
    }
}

#[test]
fn req_ui_01_req_ui_10_req_ui_16_every_step_opens_and_draws_at_1366_by_768() {
    let mut rig = rig("smoke-steps");
    for step in Step::ALL {
        assert!(
            rig.shell.overview.step(step).is_available(),
            "{} is available in the cube project",
            step.name()
        );
        rig.shell.open_step(step);
        draw(&mut rig);
        assert_eq!(rig.shell.step, step);
        // The workflow bar names every step, and the footer carries the notice.
        for each in Step::ALL {
            assert!(rig.shows(each.name()), "{} on {}", each.name(), step.name());
        }
        assert!(
            rig.shows("Verify G-code before running it"),
            "{}",
            step.name()
        );
    }
    // The keys reach every step too (Alt 1 to Alt 5).
    for (key, step) in [
        (Key::Num1, Step::Model),
        (Key::Num2, Step::Setup),
        (Key::Num3, Step::Operations),
        (Key::Num4, Step::Simulate),
        (Key::Num5, Step::Export),
    ] {
        rig.press(key, Modifiers::ALT);
        assert_eq!(rig.shell.step, step);
    }
}

#[test]
fn req_ui_01_req_ui_07_every_inspector_and_setup_section_draws() {
    let mut rig = rig("smoke-inspectors");
    let model = rig.shell.app.as_ref().unwrap().project().models()[0].id;
    let op = rig.shell.app.as_ref().unwrap().project().operations()[0].id;
    let setup = rig.shell.app.as_ref().unwrap().project().setups()[0].id;

    rig.shell.select(Selection::Model(model));
    draw(&mut rig);
    assert!(rig.shows("Model"));

    rig.shell.select(Selection::Operation(op));
    draw(&mut rig);
    assert!(rig.shows("Geometry") || rig.shows("GEOMETRY"));

    for part in SetupPart::ALL {
        rig.shell.choose_setup(setup, Some(part));
        draw(&mut rig);
        let title = super::inspector::section_title(part).to_uppercase();
        assert!(rig.shows(&title), "the {title} section is shown");
    }
    // The workholding section opens its panel (OI-334; 11.7).
    rig.shell.choose_setup(setup, Some(SetupPart::Workholding));
    draw(&mut rig);
    assert!(rig.shows("WORKHOLDING AND TABS"));

    // Picking on the part opens the selection tools and New selection in the
    // inspector (decision 0215).
    rig.shell.start_picking();
    draw(&mut rig);
    assert!(matches!(rig.shell.selected, Some(Selection::Picking)));
}

#[test]
fn req_ui_01_req_ui_12_every_dock_tab_draws() {
    let mut rig = rig("smoke-dock");
    rig.shell.settings.dock.collapsed = false;
    // The Setup checks tab belongs to the Setup step and Playback to Simulate; the others
    // stay on any step.
    for (tab, step) in [
        (DockTab::Messages, Step::Operations),
        (DockTab::Jobs, Step::Operations),
        (DockTab::SetupChecks, Step::Setup),
        (DockTab::Playback, Step::Simulate),
    ] {
        rig.shell.open_step(step);
        rig.shell.dock_tab = tab;
        draw(&mut rig);
        assert_eq!(rig.shell.dock_tab, tab);
    }
}

#[test]
fn req_ui_01_every_dialog_and_window_opens_and_draws() {
    let mut rig = rig("smoke-windows");
    let op = rig.shell.app.as_ref().unwrap().project().operations()[0].id;

    // The modal dialogs.
    let dialogs = [
        Dialog::Keys,
        Dialog::About,
        Dialog::Settings,
        Dialog::Error("The setup was not added: smoke".to_owned()),
        Dialog::Unsaved(Next::Quit),
        Dialog::OfferCreate {
            folder: std::env::temp_dir(),
        },
        Dialog::FolderExists {
            parent: "parent".to_owned(),
            name: "Smoke".to_owned(),
            folder: std::env::temp_dir(),
        },
        Dialog::Folder {
            create: false,
            folder: String::new(),
            name: String::new(),
        },
        Dialog::Folder {
            create: true,
            folder: String::new(),
            name: String::new(),
        },
        Dialog::Rename {
            item: openchuck_app::overview::ItemRef::Operation(op),
            name: "Smoke".to_owned(),
        },
    ];
    for dialog in dialogs {
        let name = dialog.log_name();
        rig.shell.dialog = Some(dialog);
        draw(&mut rig);
        assert!(rig.shell.dialog.is_some(), "{name} stays open");
        rig.shell.dialog = None;
        draw(&mut rig);
    }

    // The import window, the export dialog, the tool library in each purpose, and the new
    // operation dialog.
    rig.shell.import_model();
    draw(&mut rig);
    assert!(rig.shell.import.is_some());
    rig.shell.import = None;

    rig.shell.open_export();
    draw(&mut rig);
    assert!(rig.shell.export.is_open());

    for purpose in [Purpose::Browse, Purpose::Add] {
        rig.shell.open_tool_library(purpose, None);
        draw(&mut rig);
        assert!(rig.shell.tool_library.is_open());
    }

    rig.shell.new_operation();
    draw(&mut rig);
    assert!(rig.shell.run.is_adding());
}

#[test]
fn req_ui_01_req_ui_16_the_results_and_playback_draw_after_a_run() {
    let mut rig = rig("smoke-run");
    rig.shell.generate(true);
    rig.settle();
    rig.shell.open_step(Step::Simulate);
    draw(&mut rig);
    assert_eq!(rig.shell.step, Step::Simulate);
    assert!(rig.shell.play.open);
    rig.shell.run.show_results();
    draw(&mut rig);
}

#[test]
fn req_ui_16_the_three_regions_collapse_and_reopen_at_1366_by_768() {
    let mut rig = rig("smoke-regions");
    for key in [Key::Num1, Key::Num2, Key::Num3] {
        rig.press(key, Modifiers::COMMAND);
        draw(&mut rig);
        rig.press(key, Modifiers::COMMAND);
        draw(&mut rig);
    }
    assert!(rig.shows("Verify G-code before running it"));
}

/// OI-359: the dock's default height fits its content at 1366 × 768 (the owner checks it on
/// Windows), and a size the window clamps is not remembered as the user's choice.
#[test]
fn req_ui_16_the_default_dock_shows_the_playback_controls_and_a_clamp_is_not_remembered() {
    let mut rig = rig("smoke-dock-height");
    rig.shell.open_step(Step::Simulate);
    draw(&mut rig);
    assert_eq!(rig.shell.settings.dock.size, 260.0);
    // Every row of the playback tab is on screen without scrolling.
    for text in ["Speed", "Whole program", "Go to move"] {
        assert!(rig.shows(text), "{text}: {:?}", rig.texts());
    }
    // A saved size larger than half the window is drawn clamped and kept as it was.
    rig.shell.settings.dock.size = 600.0;
    draw(&mut rig);
    assert_eq!(rig.shell.settings.dock.size, 600.0);
    // The window grows: the dock goes back to the saved size, not to the clamped one.
    rig.size = egui::vec2(1366.0, 1400.0);
    draw(&mut rig);
    assert_eq!(rig.shell.settings.dock.size, 600.0);
}

/// OI-525: a message's source link never lies over its wrapped sentence.
#[test]
fn req_ui_12_req_ui_16_a_message_link_keeps_clear_of_its_wrapped_sentence() {
    let mut rig = rig("smoke-message-links");
    rig.shell.dock_tab = DockTab::Messages;
    draw(&mut rig);
    let rects = rig.text_rects();
    let sentence = rects
        .iter()
        .find(|(t, _)| t.starts_with("STL holds triangles only"))
        .expect("the STL message is shown");
    let link = rects
        .iter()
        .find(|(t, _)| t == "models/cube.stl")
        .expect("its source link is shown");
    assert!(
        !sentence.1.intersects(link.1),
        "{:?} over {:?}",
        link.1,
        sentence.1
    );
}

/// Issue 215 (F-59, OI-570, decision 0187): the default dock shows the whole
/// playback tab, down to its last row, above the footer, and no row moves
/// when the simulation plays or steps. At the reference window and at
/// 1920 × 1080.
#[test]
fn req_ui_16_req_sim_04_the_whole_playback_tab_shows_in_the_default_dock_and_stays_put() {
    let rows = [
        "Speed",
        "Whole program",
        "Go to move",
        "Turn part with A",
        "Colour the part by deviation",
        // Issues 205 and 214 (decision records 0168 and 0169).
        "Window: moves",
        "Hide path outside window",
        "Skip stock simulation",
    ];
    for window in [WINDOW, egui::vec2(1920.0, 1080.0)] {
        let mut rig = rig("smoke-playback-tab");
        crate::theme::install_style(&rig.ctx, crate::theme::Theme::Dark);
        rig.size = window;
        rig.shell.open_step(Step::Simulate);
        draw(&mut rig);
        let footer = rig
            .text_rects()
            .iter()
            .find(|(t, _)| t == "Verify G-code before running it")
            .unwrap()
            .1
            .top();
        let probe = |rig: &Rig| -> Vec<(f32, f32)> {
            rows.iter()
                .map(|row| {
                    let r = rig
                        .text_rects()
                        .iter()
                        .find(|(t, _)| t == row)
                        .unwrap_or_else(|| panic!("{row:?} is not shown at {window:?}"))
                        .1;
                    (r.top(), r.bottom())
                })
                .collect()
        };
        let idle = probe(&rig);
        for (row, (_, bottom)) in rows.iter().zip(&idle) {
            assert!(
                *bottom <= footer,
                "{row:?} reaches the footer at {window:?}"
            );
        }
        rig.press(Key::Space, Modifiers::NONE);
        for _ in 0..10 {
            rig.frame(Vec::new());
        }
        assert_eq!(probe(&rig), idle, "playing, {window:?}");
        rig.press(Key::ArrowRight, Modifiers::NONE);
        draw(&mut rig);
        assert_eq!(probe(&rig), idle, "stepped, {window:?}");
    }
}
