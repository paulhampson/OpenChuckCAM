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
                s.run.begin_add(app, None, None);
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
fn req_ui_05_a_selected_set_keeps_the_selection_options_in_the_inspector() {
    // A selected set also turns picking on, so By angle and Same direction
    // work there and their angles must be reachable (decision 0153).
    let mut rig = Rig::picking("set-options");
    let model = rig.model();
    let id = rig
        .shell
        .app
        .as_mut()
        .unwrap()
        .save_selection("Set A", model, BTreeSet::from([0u32, 1]))
        .unwrap();
    rig.shell.selected = Some(Selection::Set(id));
    for _ in 0..3 {
        rig.frame(Vec::new());
    }
    assert!(
        rig.text_centre("Keep tangent edges by default").is_some(),
        "the options are painted for a selected set"
    );
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
fn req_ui_05_a_chosen_sets_inspector_takes_its_swatch_colour() {
    // The set's colour is in its inspector (12.2; decision 0215 item 4).
    let mut rig = Rig::picking("sets-inspector");
    let id = select_a_set(&mut rig);
    for _ in 0..3 {
        rig.frame(Vec::new());
    }
    assert_eq!(rig.shell.selected, Some(Selection::Set(id)));
    assert!(rig.text_centre("Change this set on the part").is_some());
    // Choosing the third swatch stores that colour on the set, through an
    // undoable command.
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

#[test]
fn req_ui_05_the_dock_has_no_selection_sets_tab() {
    // Decision 0215 item 1 (issue 225): everything moved to the inspector.
    let mut rig = Rig::picking("no-sets-tab");
    for step in [Step::Model, Step::Setup, Step::Operations, Step::Simulate] {
        rig.shell.step = step;
        assert!(
            rig.shell
                .tabs()
                .iter()
                .all(|t| t.name() != "Selection sets"),
            "{step:?}"
        );
    }
    // The tree's "+" opens New selection in the inspector.
    rig.shell.step = Step::Operations;
    rig.shell.start_picking();
    for _ in 0..3 {
        rig.frame(Vec::new());
    }
    assert!(rig.text_centre("New selection").is_some());
    assert!(rig.text_centre("Save as new set").is_some());
    rig.cleanup();
}

#[test]
fn req_ui_05_a_set_is_deleted_from_the_tree_and_refused_while_used() {
    // Decision 0215 item 2: Delete is in the tree's row menu and on Del.
    let mut rig = Rig::picking("tree-delete");
    let id = select_a_set(&mut rig);
    let op = rig.shell.app.as_ref().unwrap().project().operations()[0].id;
    rig.shell
        .app
        .as_mut()
        .unwrap()
        .set_operation_selection(
            op,
            openchuck_app::selection::SelectionRole::Machine,
            id,
            true,
        )
        .unwrap();
    rig.shell.tree_action(super::TreeAction::DeleteSet(id));
    match &rig.shell.dialog {
        Some(Dialog::Error(note)) => assert!(note.contains("is used by"), "{note}"),
        other => panic!("refused with a note, not {other:?}"),
    }
    assert_eq!(
        rig.shell
            .app
            .as_ref()
            .unwrap()
            .project()
            .selection_sets()
            .len(),
        1
    );
    rig.shell.dialog = None;
    rig.shell
        .app
        .as_mut()
        .unwrap()
        .set_operation_selection(
            op,
            openchuck_app::selection::SelectionRole::Machine,
            id,
            false,
        )
        .unwrap();
    rig.shell.tree_action(super::TreeAction::SelectSet(id));
    rig.frame(Vec::new());
    rig.press(Key::Delete);
    assert!(
        rig.shell
            .app
            .as_ref()
            .unwrap()
            .project()
            .selection_sets()
            .is_empty()
    );
    assert_eq!(rig.shell.selected, Some(Selection::Picking));
    rig.cleanup();
}

/// A plain click that the user confirms leaves the chosen set, and the
/// inspector shows New selection; saving a new set shows its inspector
/// (decision 0215 items 3 and 5).
#[test]
fn req_ui_05_the_inspector_follows_the_set_chosen_or_left() {
    let mut rig = Rig::picking("follow-panel");
    let id = select_a_set(&mut rig);
    rig.shell.selection.leave_set_for_test();
    rig.frame(Vec::new());
    assert_eq!(rig.shell.selected, Some(Selection::Picking));
    assert_eq!(rig.shell.selection.shown_set(), None);
    // Choosing it again shows its inspector.
    rig.shell.tree_action(super::TreeAction::SelectSet(id));
    rig.frame(Vec::new());
    assert_eq!(rig.shell.selected, Some(Selection::Set(id)));
    rig.cleanup();
}

/// A set made on an older version of the model is not loaded as the working
/// selection by "Pick again": its numbers may name other geometry now. It is
/// drawn as a ghost, the user picks again and Replace makes it current
/// (specification 12.2; decisions 0125 and 0215).
#[test]
fn req_ui_05_req_prj_06_reselect_on_a_stale_set_starts_a_fresh_pick_and_draws_a_ghost() {
    use openchuck_app::project::{ContentHash, Item};
    let mut rig = Rig::picking("reselect-stale");
    let model = rig.model();
    let app = rig.shell.app.as_mut().unwrap();
    let id = app
        .save_selection("Set A", model, BTreeSet::from([0u32, 1]))
        .unwrap();
    // The set becomes stale: made on another version of the file.
    let mut old = app.project().selection_sets()[0].clone();
    old.model_hash = ContentHash::of_bytes(b"an older version");
    app.execute(Command::Replace(Item::SelectionSet(old)))
        .unwrap();
    let summary = app.selection_summaries(&[])[0].clone();
    assert!(summary.problem.is_some());
    // "Pick again" in the inspector's banner, as a message's action does.
    rig.shell.message_action(MessageAction::Reselect(id));
    // Nothing is loaded; the set is shown, with what to do next.
    assert!(rig.shell.selection.working_triangles().is_empty());
    assert_eq!(rig.shell.selection.shown_set(), Some(id));
    assert!(
        rig.shell
            .selection
            .notice()
            .is_some_and(|m| m.contains("Pick the new selection, then Replace."))
    );
    // The overlay draws the old selection hatched (a ghost), not in the
    // working colour.
    rig.shell.selection.mark_dirty_for_test();
    let overlay = rig
        .shell
        .selection
        .overlay(rig.shell.app.as_ref().unwrap(), rig.shell.viewport.view())
        .expect("the overlay is rebuilt");
    assert!(
        overlay.meshes.iter().any(|m| m.hatch.is_some()),
        "a ghost is hatched"
    );
    rig.cleanup();
}

/// Whether anything is drawn in the highlight layer over the part.
fn highlighted(rig: &Rig) -> bool {
    rig.shell
        .viewport
        .layer(openchuck_viewport::Layer::Overlay)
        .is_some_and(|s| !s.meshes.is_empty() || !s.lines.is_empty())
}

/// Saves a set of two triangles, selects it in the tree and draws a frame.
fn select_a_set(rig: &mut Rig) -> SelectionSetId {
    let model = rig.model();
    let id = rig
        .shell
        .app
        .as_mut()
        .unwrap()
        .save_selection("Set A", model, BTreeSet::from([0u32, 1]))
        .unwrap();
    rig.shell.tree_action(super::TreeAction::SelectSet(id));
    rig.frame(Vec::new());
    id
}

#[test]
fn req_ui_04_req_ui_05_leaving_selection_mode_leaves_no_highlight() {
    // Issue 261: the pending selection is drawn only while selection mode is
    // on. It is kept for the next time picking starts.
    let mut rig = Rig::picking("highlight-leave");
    let model = rig.model();
    rig.shell
        .selection
        .set_working_for_test(rig.shell.app.as_ref().unwrap(), model, &[0, 1]);
    rig.frame(Vec::new());
    assert!(highlighted(&rig));
    rig.press(Key::Escape);
    rig.frame(Vec::new());
    assert_eq!(rig.shell.selected, None);
    assert!(!highlighted(&rig), "Esc leaves no highlight");
    rig.shell.selected = Some(Selection::Picking);
    rig.frame(Vec::new());
    assert!(highlighted(&rig), "picking again shows it again");
    // Leaving a selected set by Esc takes its highlight away too.
    let id = select_a_set(&mut rig);
    assert_eq!(rig.shell.selected, Some(Selection::Set(id)));
    rig.press(Key::Escape);
    rig.frame(Vec::new());
    assert!(!highlighted(&rig));
    assert_eq!(rig.shell.selection.shown_set(), None);
    rig.cleanup();
}

#[test]
fn req_ui_05_deselecting_a_set_leaves_no_highlight() {
    let mut rig = Rig::picking("highlight-deselect");
    let id = select_a_set(&mut rig);
    assert!(highlighted(&rig));
    // The list's click on the chosen set deselects it.
    rig.shell.selection.show_set(None);
    rig.frame(Vec::new());
    assert!(!highlighted(&rig), "deselected in the list");
    // Picked again, then deselected: the new pick goes with it.
    rig.shell.selection.pick_again(id);
    let model = rig.model();
    rig.shell
        .selection
        .set_working_for_test(rig.shell.app.as_ref().unwrap(), model, &[4]);
    rig.frame(Vec::new());
    assert!(highlighted(&rig));
    rig.shell.selection.show_set(None);
    rig.frame(Vec::new());
    assert!(!highlighted(&rig), "the new pick goes too");
    assert!(rig.shell.selection.working_triangles().is_empty());
    // Selecting another item deselects the set.
    rig.shell.tree_action(super::TreeAction::SelectSet(id));
    rig.frame(Vec::new());
    assert!(highlighted(&rig));
    let op = rig.shell.app.as_ref().unwrap().project().operations()[0].id;
    rig.shell.tree_action(super::TreeAction::Select(op));
    rig.frame(Vec::new());
    assert!(!highlighted(&rig), "another item selected");
    rig.cleanup();
}

#[test]
fn req_ui_05_changing_step_leaves_no_highlight_in_any_view() {
    // Model, Operations and Simulate share the highlight layer; leaving the
    // step ends selection mode, from a pending selection or a selected set.
    let mut rig = Rig::picking("highlight-step");
    rig.shell.generate(false);
    let deadline = Instant::now() + Duration::from_secs(120);
    while rig.shell.run.last().is_none() {
        rig.frame(Vec::new());
        std::thread::sleep(Duration::from_millis(10));
        assert!(Instant::now() < deadline, "timed out");
    }
    let model = rig.model();
    rig.shell.step = Step::Operations;
    for (from, to) in [
        (Step::Operations, Step::Model),
        (Step::Model, Step::Simulate),
        (Step::Simulate, Step::Operations),
    ] {
        rig.shell.step = from;
        rig.shell.selected = Some(Selection::Picking);
        rig.shell
            .selection
            .set_working_for_test(rig.shell.app.as_ref().unwrap(), model, &[0, 1]);
        rig.frame(Vec::new());
        assert!(highlighted(&rig), "{from:?}");
        rig.shell.open_step(to);
        rig.frame(Vec::new());
        assert_eq!(rig.shell.step, to);
        assert_eq!(rig.shell.selected, None);
        assert!(!highlighted(&rig), "{from:?} to {to:?}, pending selection");

        rig.shell.step = from;
        let id = select_a_set(&mut rig);
        assert!(highlighted(&rig), "{from:?}, set");
        rig.shell.open_step(to);
        rig.frame(Vec::new());
        assert!(!highlighted(&rig), "{from:?} to {to:?}, set");
        rig.shell
            .app
            .as_mut()
            .unwrap()
            .execute(Command::Remove(
                openchuck_app::project::ItemId::SelectionSet(id),
            ))
            .unwrap();
    }
    rig.cleanup();
}

/// "Pick again" from a set's banner or a message waits while a change of a
/// set is checked, as the inspector's button does, and leaves the shell as it
/// was (decision 0215 item 6; OI-874).
#[test]
fn req_ui_05_reselect_waits_while_a_change_is_checked() {
    let mut rig = Rig::picking("reselect-while-checking");
    let model = rig.model();
    let app = rig.shell.app.as_mut().unwrap();
    let a = app
        .save_selection("Set A", model, BTreeSet::from([0u32, 1]))
        .unwrap();
    let b = app
        .save_selection("Set B", model, BTreeSet::from([5u32]))
        .unwrap();
    rig.shell.selection.show_set(Some(a));
    rig.shell.selected = Some(Selection::Set(a));
    let app = rig.shell.app.as_ref().unwrap();
    rig.shell.selection.add_to_chosen_for_test(app, model, &[2]);
    assert!(rig.shell.selection.is_writing());
    rig.shell.message_action(MessageAction::Reselect(b));
    assert_eq!(rig.shell.selected, Some(Selection::Set(a)));
    assert_eq!(rig.shell.selection.shown_set(), Some(a));
    rig.cleanup();
}
