// SPDX-License-Identifier: AGPL-3.0-or-later
//! The shell routes keys and clicks to the selection work and to dialogs
//! (UI-04, UI-05, UI-14; decision record 0046 item 4; specification section
//! 15). The selection panel's own tests cover its keys alone; these draw the
//! whole shell without a window and press keys and click widgets in it, so
//! what reaches the panel, and what must not, is checked where it is routed.

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use eframe::egui::{self, Event, Key, Modifiers, PointerButton};

use super::*;
use crate::export_panel::ExportPanel;
use crate::testing::cube_project;

/// The click mode names, in the order of the keys Q, W, E and R.
const MODES: [(Key, &str); 4] = [
    (Key::Q, "Select one"),
    (Key::W, "Flood fill"),
    (Key::E, "By angle"),
    (Key::R, "Same direction"),
];
/// The click mode a new panel starts in.
const DEFAULT_MODE: &str = "By angle";

/// A shell drawn frame by frame, with a text box it can give the keyboard to.
struct Rig {
    shell: Shell,
    ctx: egui::Context,
    dir: PathBuf,
    probe_text: String,
    probe_focus: bool,
    shapes: Vec<egui::epaint::ClippedShape>,
}

impl Rig {
    /// A shell on the cube project with the scene built, picking on.
    fn picking(name: &str) -> Self {
        let (dir, app, _, _) = cube_project(name);
        app.save().unwrap();
        drop(app);
        // Not the real configuration folder.
        let mut shell = Shell::with_settings(false, Some(dir.clone()), None);
        shell.export = ExportPanel::new(None);
        // The selection sets are a tab of the dock (specification 12.2),
        // tall enough here to show a set's details under the table.
        shell.dock_tab = super::dock::DockTab::SelectionSets;
        shell.settings.dock.size = 380.0;
        let mut rig = Self {
            shell,
            ctx: egui::Context::default(),
            dir,
            probe_text: String::new(),
            probe_focus: false,
            shapes: Vec::new(),
        };
        let deadline = Instant::now() + Duration::from_secs(120);
        while rig.shell.scene_job.is_some() {
            assert!(Instant::now() < deadline, "timed out");
            rig.frame(Vec::new());
            std::thread::sleep(Duration::from_millis(5));
        }
        rig.shell.selected = Some(Selection::Picking);
        // An area is laid out in its first frame and shown in the next.
        for _ in 0..3 {
            rig.frame(Vec::new());
        }
        rig
    }

    fn frame(&mut self, events: Vec<Event>) {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(1400.0, 900.0),
            )),
            events,
            ..Default::default()
        };
        let Self {
            shell,
            ctx,
            probe_text,
            probe_focus,
            ..
        } = self;
        let mut output = ctx.run_ui(input, |ui| {
            shell.draw(ui);
            if *probe_focus {
                egui::Area::new(egui::Id::new("probe area")).show(ui.ctx(), |ui| {
                    let id = egui::Id::new("probe text");
                    let response = ui.add(egui::TextEdit::singleline(probe_text).id(id));
                    response.request_focus();
                });
            }
        });
        output.textures_delta.clear();
        self.shapes = output.shapes;
    }

    fn press(&mut self, key: Key) {
        let event = |pressed| Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        self.frame(vec![event(true), event(false)]);
    }

    /// The centre of the first text on screen that contains `needle`.
    fn text_centre(&self, needle: &str) -> Option<egui::Pos2> {
        fn walk(shape: &egui::Shape, needle: &str) -> Option<egui::Pos2> {
            match shape {
                egui::Shape::Text(t) if t.galley.text().contains(needle) => {
                    Some(t.pos + t.galley.rect.center().to_vec2())
                }
                egui::Shape::Vec(shapes) => shapes.iter().find_map(|s| walk(s, needle)),
                _ => None,
            }
        }
        self.shapes.iter().find_map(|s| walk(&s.shape, needle))
    }

    /// The fill of every square button of about `side` points on screen, in
    /// drawing order.
    fn square_fills(&self, side: f32) -> Vec<(egui::Pos2, egui::Color32)> {
        fn walk(shape: &egui::Shape, side: f32, out: &mut Vec<(egui::Pos2, egui::Color32)>) {
            match shape {
                egui::Shape::Rect(r)
                    if (r.rect.width() - side).abs() < 0.5
                        && (r.rect.height() - side).abs() < 0.5
                        && r.fill != egui::Color32::TRANSPARENT =>
                {
                    out.push((r.rect.center(), r.fill));
                }
                egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| walk(s, side, out)),
                _ => {}
            }
        }
        let mut out = Vec::new();
        self.shapes
            .iter()
            .for_each(|s| walk(&s.shape, side, &mut out));
        out
    }

    /// A primary click at `at`, over three frames the way a pointer arrives.
    fn click(&mut self, at: egui::Pos2) {
        self.frame(vec![Event::PointerMoved(at)]);
        let button = |pressed| Event::PointerButton {
            pos: at,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        self.frame(vec![button(true)]);
        self.frame(vec![button(false)]);
        self.frame(Vec::new());
    }

    fn mode(&self) -> &'static str {
        self.shell.selection.choice_name()
    }

    fn model(&self) -> openchuck_app::project::id::ModelId {
        self.shell.app.as_ref().unwrap().project().models()[0].id
    }

    fn cleanup(self) {
        let _ = std::fs::remove_dir_all(self.dir);
    }
}

#[test]
fn req_ui_14_the_keys_q_w_e_and_r_choose_the_click_mode_while_picking() {
    let mut rig = Rig::picking("keys-choose");
    assert_eq!(rig.mode(), DEFAULT_MODE);
    for (key, name) in MODES {
        rig.press(key);
        assert_eq!(rig.mode(), name, "{key:?}");
    }
    // Any other key does nothing to it.
    rig.press(Key::B);
    assert_eq!(rig.mode(), "Same direction");
    // The choice is made while a saved set is the chosen item too.
    let model = rig.model();
    let id = rig
        .shell
        .app
        .as_mut()
        .unwrap()
        .save_selection("Set A", model, BTreeSet::from([0u32, 1]))
        .unwrap();
    rig.shell.selected = Some(Selection::Set(id));
    rig.press(Key::Q);
    assert_eq!(rig.mode(), "Select one");
    rig.cleanup();
}

#[test]
fn req_ui_14_the_keys_do_nothing_when_nothing_is_being_picked() {
    let mut rig = Rig::picking("keys-idle");
    rig.shell.selected = None;
    rig.frame(Vec::new());
    rig.press(Key::Q);
    assert_eq!(rig.mode(), DEFAULT_MODE);
    // Esc with nothing picked changes nothing either.
    rig.press(Key::Escape);
    assert_eq!(rig.shell.selected, None);
    rig.cleanup();
}

#[test]
fn req_ui_14_the_keys_are_left_to_a_text_field_that_has_the_keyboard() {
    let mut rig = Rig::picking("keys-text");
    rig.probe_focus = true;
    rig.frame(Vec::new());
    rig.frame(Vec::new());
    assert!(rig.ctx.egui_wants_keyboard_input());
    rig.press(Key::Q);
    assert_eq!(rig.mode(), DEFAULT_MODE, "typing q is not choosing a mode");
    rig.press(Key::Escape);
    assert_eq!(
        rig.shell.selected,
        Some(Selection::Picking),
        "Esc in a text field does not leave picking"
    );
    rig.cleanup();
}

#[test]
fn req_ui_14_the_keys_are_left_to_any_dialog_or_window_in_front_of_the_view() {
    // Each way a dialog or window can be open, and how it is opened.
    type Open = fn(&mut Shell);
    let cases: [(&str, Open); 4] = [
        ("an error dialog", |s| {
            s.dialog = Some(Dialog::Error("Something failed.".to_owned()));
        }),
        ("the export dialog", Shell::open_export),
        ("the results window", |s| s.run.show_results()),
        ("the add-operation dialog", |s| {
            if let Some(app) = &s.app {
                s.run.begin_add(app);
            }
        }),
    ];
    for (name, open) in cases {
        let mut rig = Rig::picking("keys-dialog");
        open(&mut rig.shell);
        rig.frame(Vec::new());
        assert!(rig.shell.window_open(), "{name} is open");
        rig.press(Key::Q);
        assert_eq!(rig.mode(), DEFAULT_MODE, "Q behind {name}");
        rig.press(Key::Escape);
        assert_eq!(
            rig.shell.selected,
            Some(Selection::Picking),
            "Esc behind {name} leaves picking"
        );
        assert!(rig.shell.window_open(), "Esc does not close {name}");
        rig.cleanup();
    }
}

#[test]
fn req_ui_14_the_keys_work_again_when_the_dialog_is_closed() {
    let mut rig = Rig::picking("keys-closed");
    rig.shell.open_export();
    rig.frame(Vec::new());
    rig.press(Key::Q);
    assert_eq!(rig.mode(), DEFAULT_MODE);
    rig.shell.export.clear();
    rig.shell.export = ExportPanel::new(None);
    rig.frame(Vec::new());
    assert!(!rig.shell.window_open());
    rig.press(Key::Q);
    assert_eq!(rig.mode(), "Select one");
    rig.cleanup();
}

#[test]
fn req_ui_14_esc_cancels_a_running_region_pick_and_then_leaves_picking() {
    let mut rig = Rig::picking("keys-esc");
    let model = rig.model();
    let app = rig.shell.app.as_ref().unwrap();
    rig.shell.selection.hold_region_pick(app, model);
    assert!(rig.shell.selection.has_region_pick());
    // The first Esc stops the fill and keeps picking on.
    rig.press(Key::Escape);
    assert!(!rig.shell.selection.has_region_pick());
    assert_eq!(rig.shell.selected, Some(Selection::Picking));
    assert!(rig.shell.selection.working_triangles().is_empty());
    // The next leaves picking.
    rig.press(Key::Escape);
    assert_eq!(rig.shell.selected, None);
    rig.cleanup();
}

#[test]
fn req_ui_14_esc_leaves_picking_from_a_saved_set_too() {
    let mut rig = Rig::picking("keys-esc-set");
    let model = rig.model();
    let id = rig
        .shell
        .app
        .as_mut()
        .unwrap()
        .save_selection("Set A", model, BTreeSet::from([0u32, 1]))
        .unwrap();
    rig.shell.selected = Some(Selection::Set(id));
    rig.frame(Vec::new());
    rig.press(Key::Escape);
    assert_eq!(rig.shell.selected, None);
    rig.cleanup();
}

#[test]
fn req_ui_04_the_selection_tools_box_chooses_the_click_mode_by_a_click() {
    let mut rig = Rig::picking("tools-click");
    // Each button shows its key and sets the mode it names.
    for (key, name) in MODES.iter().rev() {
        let label = format!("{name} ({})", key.name());
        let at = rig
            .text_centre(&label)
            .unwrap_or_else(|| panic!("no button '{label}' on screen"));
        rig.click(at);
        assert_eq!(rig.mode(), *name, "clicked '{label}'");
    }
    rig.cleanup();
}

#[test]
fn req_ui_05_the_sets_panel_selects_a_set_in_view_and_takes_its_swatch_colour() {
    let mut rig = Rig::picking("sets-panel");
    let model = rig.model();
    let id = rig
        .shell
        .app
        .as_mut()
        .unwrap()
        .save_selection("Set A", model, BTreeSet::from([0u32, 1]))
        .unwrap();
    // A grid is laid out in its first frame and shown in the next.
    for _ in 0..3 {
        rig.frame(Vec::new());
    }
    assert!(rig.shell.selection.working_triangles().is_empty());
    // "Select in view" makes the set the working selection.
    // The table is wider than the panel: scroll it sideways to the link.
    let header = rig.text_centre("Kind").expect("the table is shown");
    let over_panel = header;
    rig.frame(vec![Event::PointerMoved(over_panel)]);
    for _ in 0..4 {
        rig.frame(vec![Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            phase: egui::TouchPhase::Move,
            delta: egui::vec2(-400.0, 0.0),
            modifiers: Modifiers::NONE,
        }]);
    }
    // Let the scroll settle before looking for the link: it has stopped when
    // the link does not move between frames.
    let mut last = None;
    for _ in 0..200 {
        rig.frame(Vec::new());
        let now = rig.text_centre("Select in view");
        if now.is_some() && now == last {
            break;
        }
        last = now;
        std::thread::sleep(Duration::from_millis(5));
    }
    let at = rig
        .text_centre("Select in view")
        .expect("the link is shown");
    // The table's scroll bar floats over the lower part of its rows: click
    // the link near its top.
    rig.click(egui::pos2(at.x, at.y - 4.0));
    assert_eq!(rig.shell.selection.working_triangles(), vec![0, 1]);
    // The set's details are now shown with a row of swatches: choosing the
    // third stores that colour on the set, through an undoable command.
    // The details are under the table in the dock: scroll down to them.
    let mut swatches = rig.square_fills(18.0);
    for _ in 0..40 {
        if swatches.len() >= 8 {
            break;
        }
        rig.frame(vec![
            Event::PointerMoved(over_panel),
            Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                phase: egui::TouchPhase::Move,
                delta: egui::vec2(0.0, -60.0),
                modifiers: Modifiers::NONE,
            },
        ]);
        std::thread::sleep(Duration::from_millis(5));
        swatches = rig.square_fills(18.0);
    }
    // Let the scroll settle, so the swatches are where they will be clicked.
    for _ in 0..20 {
        rig.frame(Vec::new());
        std::thread::sleep(Duration::from_millis(5));
    }
    let swatches = rig.square_fills(18.0);
    assert!(swatches.len() >= 8, "{} swatches", swatches.len());
    let (at, fill) = swatches[2];
    rig.click(at);
    let colour = format!("#{:02x}{:02x}{:02x}", fill.r(), fill.g(), fill.b());
    let stored = rig
        .shell
        .app
        .as_ref()
        .unwrap()
        .project()
        .selection_sets()
        .iter()
        .find(|s| s.id == id)
        .unwrap()
        .colour
        .clone();
    assert_eq!(stored, Some(colour));
    assert!(rig.shell.app.as_ref().unwrap().can_undo());
    // "Default" takes the colour back.
    let at = rig.text_centre("Default").expect("the button is shown");
    rig.click(at);
    let stored = rig.shell.app.as_ref().unwrap().project().selection_sets()[0]
        .colour
        .clone();
    assert_eq!(stored, None);
    rig.cleanup();
}
