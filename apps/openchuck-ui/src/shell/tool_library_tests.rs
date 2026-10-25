// SPDX-License-Identifier: AGPL-3.0-or-later
//! Tools out of the tree, the operation's Tool section and the tool library
//! window, drawn whole without a window (UI-06, TL-01, TL-07, TL-11;
//! specification sections 6, 8 and 12.1; decision record 0127; trial
//! feedback F-10, issue 148).

#![allow(clippy::unwrap_used)]

use std::path::PathBuf;

use openchuck_app::project::id::ToolId;
use openchuck_app::tool_library::{self, ToolType};
use openchuck_app::tools::ToolLibrary;

use super::frame_tests::Rig;
use super::*;
use crate::tool_library_window::{Chosen, Purpose, ToolLibraryWindow};
use eframe::egui::{self, Event};

fn library_folder(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "openchuck-tool-library-window-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// A library in `folder` with one saved tool of `kind` named `name`.
fn library_with(folder: &std::path::Path, kind: ToolType, name: &str) -> ToolId {
    let mut library = ToolLibrary::open(folder).unwrap();
    let mut draft = tool_library::new_draft(&library, kind).unwrap();
    tool_library::edit_name(&mut draft, name).unwrap();
    tool_library::save_draft(&mut library, &draft, &[]).unwrap();
    draft.id
}

/// A library in `folder` with a saved flat end mill that has cutting data for
/// each of `materials`.
fn library_with_cutting_data(folder: &std::path::Path, materials: &[&str]) -> ToolId {
    use openchuck_app::setup::Millimetres;
    use openchuck_app::tools::{CuttingData, FeedRate, SpindleSpeed};
    let mut library = ToolLibrary::open(folder).unwrap();
    let mut draft = tool_library::new_draft(&library, ToolType::FlatEnd).unwrap();
    tool_library::edit_name(&mut draft, "6 mm flat").unwrap();
    for material in materials {
        let data = CuttingData {
            spindle_speed: SpindleSpeed(10000.0),
            feed_per_tooth: Millimetres(0.02),
            plunge_feed: FeedRate(150.0),
            stepover: Millimetres(0.8),
            stepdown: Millimetres(1.0),
        };
        tool_library::edit_cutting_data(&mut draft, material, data, None).unwrap();
    }
    tool_library::save_draft(&mut library, &draft, &[]).unwrap();
    draft.id
}

fn tools(rig: &Rig) -> Vec<String> {
    rig.shell
        .app
        .as_ref()
        .unwrap()
        .project()
        .tools()
        .iter()
        .map(|t| t.name.clone())
        .collect()
}

#[test]
fn req_ui_06_the_tree_lists_no_tools_and_the_operation_shows_its_tool() {
    let folder = library_folder("tree");
    let mut rig = Rig::cube("toollib-tree", None);
    rig.shell.tool_library = ToolLibraryWindow::new(Some(folder.clone()));
    rig.frame(Vec::new());
    assert!(rig.shows("OPERATIONS"));
    assert!(!rig.shows("TOOLS"), "tools are not in the tree (UI-06)");
    assert!(!rig.shows("T1  4 mm ball"));
    // The operation's inspector shows its tool, number and copy state.
    let op = rig.shell.app.as_ref().unwrap().project().operations()[0].id;
    rig.shell.select(Selection::Operation(op));
    rig.frame(Vec::new());
    rig.frame(Vec::new());
    assert!(rig.shows("TOOL"));
    assert!(rig.shows("4 mm ball"));
    assert!(rig.shows("Tool number"));
    assert!(rig.shows("Not in the library (kept in the project)"));
    assert!(rig.shows("Edit tool…"));
    assert!(rig.shows("From the library…"));
    // The number is changed there, as one undoable command (TL-06).
    let tool = rig.shell.app.as_ref().unwrap().project().tools()[0].id;
    rig.shell
        .tool_act(op, super::inspector::ToolAct::Number(tool, "T7".into()));
    let number = |rig: &Rig| {
        rig.shell.app.as_ref().unwrap().project().tools()[0]
            .controller
            .number
    };
    assert_eq!(number(&rig), 7);
    rig.shell.undo();
    assert_eq!(number(&rig), 1);
    // "Edit tool…" opens the window on the project's copy.
    rig.shell
        .tool_act(op, super::inspector::ToolAct::Edit(tool));
    assert_eq!(rig.shell.tool_library.chosen(), Some(Chosen::Project(tool)));
    rig.frame(Vec::new());
    rig.frame(Vec::new());
    assert!(rig.shows("THIS PROJECT"));
    assert!(rig.shows("The project's copy: each change can be undone"));
    assert!(rig.shows("Remove from project"));
    std::fs::remove_dir_all(&folder).ok();
}

#[test]
fn req_tl_01_req_tl_07_the_library_opens_and_adds_a_tool_to_the_project() {
    let folder = library_folder("add");
    let id = library_with(&folder, ToolType::FlatEnd, "6 mm flat");
    let mut rig = Rig::cube("toollib-add", None);
    rig.shell.tool_library = ToolLibraryWindow::new(Some(folder.clone()));
    rig.shell
        .open_tool_library(Purpose::Add, Some(Chosen::Library(id)));
    rig.frame(Vec::new());
    rig.frame(Vec::new());
    for text in [
        "Tool library",
        "THIS PROJECT",
        "FLAT END MILLS",
        "6 mm flat",
        "Add to project",
        "CUTTING DATA BY MATERIAL",
        "MACHINE TOOL SETS",
        "Import library…",
        "Export library…",
        "Saved in the library",
    ] {
        assert!(rig.shows(text), "{text}");
    }
    assert!(!rig.shows("in project"));
    // Add to project: one undoable command; the chip shows, and the tool is
    // then shown as the project's copy.
    rig.shell.tool_library.add_chosen(rig.shell.app.as_mut());
    rig.frame(Vec::new());
    assert_eq!(tools(&rig), ["4 mm ball", "6 mm flat"]);
    assert!(rig.shows("in project"));
    assert_eq!(rig.shell.tool_library.chosen(), Some(Chosen::Project(id)));
    rig.shell.undo();
    assert_eq!(tools(&rig), ["4 mm ball"]);
    std::fs::remove_dir_all(&folder).ok();
}

#[test]
fn req_tl_07_choosing_from_the_library_gives_the_operation_the_tool() {
    let folder = library_folder("operation");
    let id = library_with(&folder, ToolType::BallEnd, "3 mm ball");
    let mut rig = Rig::cube("toollib-op", None);
    rig.shell.tool_library = ToolLibraryWindow::new(Some(folder.clone()));
    let op = rig.shell.app.as_ref().unwrap().project().operations()[0].id;
    rig.shell
        .open_tool_library(Purpose::Operation(op), Some(Chosen::Library(id)));
    rig.frame(Vec::new());
    rig.frame(Vec::new());
    assert!(rig.shows("choose the operation's tool"));
    rig.shell.tool_library.add_chosen(rig.shell.app.as_mut());
    let project = rig.shell.app.as_ref().unwrap().project();
    assert_eq!(project.operation(op).unwrap().tool, Some(id));
    assert_eq!(project.tools().len(), 2);
    assert!(!rig.shell.tool_library.is_open());
    // One undo step takes both back.
    rig.shell.undo();
    let project = rig.shell.app.as_ref().unwrap().project();
    assert_ne!(project.operation(op).unwrap().tool, Some(id));
    assert_eq!(project.tools().len(), 1);
    std::fs::remove_dir_all(&folder).ok();
}

#[test]
fn req_tl_07_a_tool_for_the_new_operation_dialog_is_handed_back() {
    let folder = library_folder("new-op");
    let id = library_with(&folder, ToolType::BallEnd, "2 mm ball");
    let mut rig = Rig::cube("toollib-new-op", None);
    let mut window = ToolLibraryWindow::new(Some(folder.clone()));
    window.open(Purpose::NewOperation, Some(Chosen::Library(id)));
    let (_, handed) = window.add_chosen(rig.shell.app.as_mut());
    assert_eq!(handed, Some(id));
    assert_eq!(tools(&rig), ["4 mm ball", "2 mm ball"]);
    // A tool already in the project is handed back without a change.
    let existing = rig.shell.app.as_ref().unwrap().project().tools()[0].id;
    window.open(Purpose::NewOperation, Some(Chosen::Project(existing)));
    let (_, handed) = window.add_chosen(rig.shell.app.as_mut());
    assert_eq!(handed, Some(existing));
    assert_eq!(tools(&rig).len(), 2);
    std::fs::remove_dir_all(&folder).ok();
}

#[test]
fn req_tl_01_library_tools_are_drafts_until_saved_and_leaving_asks_first() {
    let folder = library_folder("drafts");
    let kept = library_with(&folder, ToolType::FlatEnd, "6 mm flat");
    let mut window = ToolLibraryWindow::new(Some(folder.clone()));
    window.open(Purpose::Browse, None);
    // New tool: a draft, not in the library until Save.
    window.new_tool(ToolType::Drill);
    let Some(Chosen::Library(new)) = window.chosen() else {
        panic!("{:?}", window.chosen())
    };
    assert!(window.read().unwrap().get(new).is_none());
    // Leaving with changes asks first; Keep editing stays.
    window.click(Chosen::Library(kept));
    assert!(window.asks_to_discard());
    window.answer_discard(false);
    assert_eq!(window.chosen(), Some(Chosen::Library(new)));
    window.save();
    assert!(ToolLibrary::open(&folder).unwrap().get(new).is_some());
    // An edit of a saved tool waits for Save; Discard drops it.
    window.click(Chosen::Library(kept));
    assert!(!window.asks_to_discard());
    window.rename(None, "Renamed flat");
    window.click(Chosen::Library(new));
    assert!(window.asks_to_discard());
    window.answer_discard(true);
    assert_eq!(window.chosen(), Some(Chosen::Library(new)));
    let library = ToolLibrary::open(&folder).unwrap();
    assert_eq!(library.get(kept).unwrap().name, "6 mm flat");
    std::fs::remove_dir_all(&folder).ok();
}

#[test]
fn req_tl_07_req_ui_08_a_project_tool_is_edited_by_undoable_commands() {
    let folder = library_folder("project-edit");
    let mut rig = Rig::cube("toollib-project-edit", None);
    let mut window = ToolLibraryWindow::new(Some(folder.clone()));
    let tool = rig.shell.app.as_ref().unwrap().project().tools()[0].id;
    window.open(Purpose::Add, Some(Chosen::Project(tool)));
    window.rename(rig.shell.app.as_mut(), "Project ball");
    assert_eq!(tools(&rig), ["Project ball"]);
    // Nothing went to the library.
    assert_eq!(ToolLibrary::open(&folder).unwrap().tools().count(), 0);
    rig.shell.undo();
    assert_eq!(tools(&rig), ["4 mm ball"]);
    std::fs::remove_dir_all(&folder).ok();
}

#[test]
fn req_tl_01_the_library_opens_without_a_project() {
    let folder = library_folder("no-project");
    let mut rig = Rig::new(Shell::with_settings(false, None, None), Vec::new());
    rig.shell.tool_library = ToolLibraryWindow::new(Some(folder.clone()));
    rig.shell.open_tool_library(Purpose::Browse, None);
    rig.frame(Vec::new());
    rig.frame(Vec::new());
    assert!(rig.shows("Choose a tool in the list"));
    assert!(rig.shows("Add to project"));
    assert!(!rig.shows("THIS PROJECT"));
    assert!(rig.shell.tool_library.is_open());
    std::fs::remove_dir_all(&folder).ok();
}

#[test]
fn req_tl_01_the_keys_leave_the_selected_operation_alone_behind_the_library() {
    let folder = library_folder("keys-behind");
    let mut rig = Rig::cube("toollib-keys", None);
    rig.shell.tool_library = ToolLibraryWindow::new(Some(folder.clone()));
    let op = rig.shell.app.as_ref().unwrap().project().operations()[0].id;
    rig.shell.selected = Some(Selection::Operation(op));
    rig.shell.open_tool_library(Purpose::Operation(op), None);
    rig.frame(Vec::new());
    assert!(rig.shell.tool_library.is_open());
    let count = |rig: &Rig| rig.shell.app.as_ref().unwrap().project().operations().len();
    rig.press(Key::Delete, Modifiers::NONE);
    rig.press(Key::D, Modifiers::COMMAND);
    assert_eq!(count(&rig), 1);
    std::fs::remove_dir_all(&folder).ok();
}

#[test]
fn req_tl_01_esc_closes_the_library_window_unless_a_field_has_the_keyboard() {
    // OI-526: Esc did not close the window.
    let folder = library_folder("esc-closes");
    let mut rig = Rig::new(Shell::with_settings(false, None, None), Vec::new());
    rig.shell.tool_library = ToolLibraryWindow::new(Some(folder.clone()));
    rig.shell.open_tool_library(Purpose::Browse, None);
    rig.frame(Vec::new());
    rig.frame(Vec::new());
    assert!(rig.shell.tool_library.is_open());
    rig.press(Key::Escape, Modifiers::NONE);
    rig.frame(Vec::new());
    assert!(!rig.shell.tool_library.is_open());
    std::fs::remove_dir_all(&folder).ok();
}

fn left_of(rig: &Rig, text: &str) -> f32 {
    rig.text_rects()
        .iter()
        .find(|(t, _)| t == text)
        .unwrap_or_else(|| panic!("{text:?} is not shown"))
        .1
        .left()
}

/// The tool library's panes keep their size and place whatever the mouse
/// does (issue 190, F-34: the cutting material pane grew to the left
/// whenever the mouse moved), at 100% and 120%.
#[test]
fn req_ui_16_req_tl_01_the_library_panes_do_not_move_with_the_mouse() {
    for scale in [1.0, 1.2] {
        let folder = library_folder("mouse");
        let id = library_with_cutting_data(&folder, &["oak", "walnut", "aluminium"]);
        let mut rig = Rig::cube("toollib-mouse", None);
        crate::theme::install_style(&rig.ctx, crate::theme::Theme::Dark);
        rig.ctx.set_zoom_factor(scale);
        rig.size = egui::vec2(1366.0 / scale, 768.0 / scale);
        rig.shell.tool_library = ToolLibraryWindow::new(Some(folder.clone()));
        rig.shell
            .open_tool_library(Purpose::Add, Some(Chosen::Library(id)));
        // Frames run only while the mouse moves in the application, so a
        // pane that creeps a little each frame must be caught from the
        // first frames on, not after it has settled.
        let headings = ["CUTTING DATA BY MATERIAL", "\u{23f7} FLAT END MILLS (1)"];
        for _ in 0..3 {
            rig.frame(Vec::new());
        }
        let before: Vec<f32> = headings.iter().map(|h| left_of(&rig, h)).collect();
        for n in 0..30 {
            rig.frame(Vec::new());
            let now: Vec<f32> = headings.iter().map(|h| left_of(&rig, h)).collect();
            assert_eq!(now, before, "frame {n} after opening, scale {scale}");
        }
        let over = |rig: &Rig, text: &str| {
            let r = rig.text_rects().iter().find(|(t, _)| t == text).unwrap().1;
            (r.center().x, r.center().y)
        };
        let spots = [
            (100.0, 100.0),
            (500.0, 300.0),
            over(&rig, "oak"),
            over(&rig, "walnut"),
            over(&rig, "6 mm flat"),
            over(&rig, "Add material"),
        ];
        for (x, y) in spots {
            rig.frame(vec![Event::PointerMoved(egui::pos2(x, y))]);
            for _ in 0..3 {
                rig.frame(Vec::new());
            }
            let now: Vec<f32> = headings.iter().map(|h| left_of(&rig, h)).collect();
            assert_eq!(now, before, "pointer at ({x}, {y}), scale {scale}");
        }
        std::fs::remove_dir_all(&folder).ok();
    }
}

/// The tool list's scroll bar sits at the right edge of its pane, in a strip
/// of its own, and does not lie over the search box or the rows (issue 191,
/// F-35: it sat in the middle of the pane).
#[test]
fn req_ui_16_req_tl_01_the_tool_list_scroll_bar_is_at_its_pane_edge() {
    for scale in [1.0, 1.2] {
        let folder = library_folder("bar");
        let mut library = ToolLibrary::open(&folder).unwrap();
        let mut first = None;
        for i in 0..40 {
            let mut draft = tool_library::new_draft(&library, ToolType::FlatEnd).unwrap();
            tool_library::edit_name(&mut draft, &format!("Tool number {i}")).unwrap();
            tool_library::save_draft(&mut library, &draft, &[]).unwrap();
            first.get_or_insert(draft.id);
        }
        let mut rig = Rig::cube("toollib-bar", None);
        crate::theme::install_style(&rig.ctx, crate::theme::Theme::Dark);
        rig.ctx.set_zoom_factor(scale);
        rig.size = egui::vec2(1366.0 / scale, 768.0 / scale);
        rig.shell.tool_library = ToolLibraryWindow::new(Some(folder.clone()));
        rig.shell
            .open_tool_library(Purpose::Add, Some(Chosen::Library(first.unwrap())));
        for _ in 0..30 {
            rig.frame(Vec::new());
        }
        let hint = rig
            .text_rects()
            .iter()
            .find(|(t, _)| t == "Search tools")
            .expect("the search hint")
            .1;
        let search = rig
            .entry_rects()
            .into_iter()
            .find(|r| r.contains_rect(hint))
            .expect("the search box");
        // The right-most bar near the list: the project tree behind the
        // window may have a bar of its own further left.
        let bar = rig
            .bar_rects()
            .into_iter()
            .filter(|r| r.left() < search.right() + 20.0)
            .max_by(|a, b| a.right().total_cmp(&b.right()))
            .expect("the list's scroll bar");
        assert!(
            search.right() - bar.right() < 8.0 && bar.right() <= search.right() + 1.0,
            "the bar is not at the pane's edge: {bar:?} against {search:?}, scale {scale}"
        );
        for (text, r) in rig.text_rects() {
            // The rows of the list: a tool's name and its number chip.
            if (text.starts_with("Tool number") || text == "T1")
                && r.left() > search.left()
                && r.left() < bar.left() - 8.0
                && r.top() >= bar.top()
                && r.bottom() <= bar.bottom()
            {
                assert!(
                    r.right() <= bar.left(),
                    "{text:?} lies under the bar: {r:?}, scale {scale}"
                );
            }
        }
        std::fs::remove_dir_all(&folder).ok();
    }
}

/// The cutting data boxes show the largest value in full, with the box's
/// padding, at 100% and 120% and at every width the pane can have (issue 250,
/// F-80); a box is never narrower than its minimum.
#[test]
fn req_ui_16_req_tl_05_the_spindle_speed_box_shows_a_five_digit_speed_in_full() {
    use crate::tool_library_window::cutting_box_width;
    for scale in [1.0_f32, 1.2] {
        let rig = Rig::cube("toollib-box", None);
        crate::theme::install_style(&rig.ctx, crate::theme::Theme::Dark);
        rig.ctx.set_zoom_factor(scale);
        let mut needed = 0.0_f32;
        let mut output = rig.ctx.run_ui(egui::RawInput::default(), |ui| {
            let font = egui::TextStyle::Monospace.resolve(ui.style());
            // The widest speed the library accepts in practice, and a long
            // feed, with the box's padding each side.
            needed = ui
                .painter()
                .layout_no_wrap("99999.99".to_owned(), font, egui::Color32::WHITE)
                .size()
                .x
                + 2.0 * ui.spacing().button_padding.x;
        });
        output.textures_delta.clear();
        for available in [0.0, 50.0, 120.0, 300.0, 700.0] {
            let width = cutting_box_width(available);
            assert!(
                width >= needed,
                "available {available}, scale {scale}: box {width} < {needed}"
            );
        }
        assert!(
            cutting_box_width(2000.0) > cutting_box_width(300.0),
            "it uses the room"
        );
    }
}

/// Every entry of the tool library has a tooltip in plain words that gives
/// its unit and does not say a value is safe to run (issue 250, F-80;
/// decision 0114; NFR-12).
#[test]
fn req_tl_04_req_tl_05_every_tool_library_entry_has_a_plain_tooltip() {
    for (label, unit, help) in crate::tool_library_window::cutting_help() {
        assert!(help.len() > 40, "{label}: {help}");
        assert!(!help.to_lowercase().contains("safe to run"), "{label}");
        assert!(!unit.is_empty(), "{label}");
    }
    let spindle = crate::tool_library_window::cutting_help()[0];
    assert!(
        spindle.2.contains("rpm") && spindle.2.contains("S value"),
        "{}",
        spindle.2
    );
    let tool = openchuck_app::tools::Tool {
        shape: openchuck_app::tools::ToolShape::BullNose {
            corner_radius: openchuck_app::tools::Millimetres(1.0),
        },
        diameter: openchuck_app::tools::Millimetres(6.0),
        flute_length: openchuck_app::tools::Millimetres(12.0),
        overall_length: openchuck_app::tools::Millimetres(50.0),
        shank_diameter: openchuck_app::tools::Millimetres(6.0),
        flute_count: 2,
        material: None,
        centre_cutting: None,
    };
    for field in openchuck_app::tool_edit::tool_form(&tool) {
        assert!(
            field.description.len() > 25,
            "{}: {}",
            field.label,
            field.description
        );
        assert!(!field.description.to_lowercase().contains("safe to run"));
    }
}

/// The operation's Tool section has a spindle speed row (issue 249, F-79;
/// decision 0196): the tool's cutting data fills it as a default, a missing
/// one is marked with what to do, and a value set there is one undoable step.
#[test]
fn req_tl_05_req_nfr_12_the_operation_has_a_spindle_speed_row() {
    use super::inspector::ToolAct;
    let folder = library_folder("spindle");
    let mut rig = Rig::cube("toollib-spindle", None);
    rig.shell.tool_library = ToolLibraryWindow::new(Some(folder.clone()));
    let op = rig.shell.app.as_ref().unwrap().project().operations()[0].id;
    rig.shell.select(Selection::Operation(op));
    for _ in 0..3 {
        rig.frame(Vec::new());
    }
    assert!(rig.shows("Spindle speed"), "the row is there");
    assert!(rig.shows("rpm"), "with its unit");
    let spindle = |rig: &Rig| {
        rig.shell.app.as_ref().unwrap().project().operations()[0]
            .spindle_speed
            .map(|s| s.0)
    };
    assert_eq!(spindle(&rig), None);
    rig.shell.tool_act(op, ToolAct::Spindle("9000".into()));
    assert_eq!(spindle(&rig), Some(9000.0));
    rig.frame(Vec::new());
    rig.shell.undo();
    assert_eq!(spindle(&rig), None);
    // A bad value is refused and leaves the operation alone.
    rig.shell.tool_act(op, ToolAct::Spindle("fast".into()));
    assert_eq!(spindle(&rig), None);
    rig.shell.tool_act(op, ToolAct::Spindle("12000".into()));
    rig.shell.tool_act(op, ToolAct::SpindleReset);
    assert_eq!(spindle(&rig), None);
    std::fs::remove_dir_all(&folder).ok();
}

/// Applying a cutting data edit that leaves the material name alone keeps its
/// spelling, even when another tool spells the same material differently
/// (decision 0197).
#[test]
fn req_tl_05_applying_a_speed_edit_keeps_the_materials_spelling() {
    let folder = library_folder("keep-spelling");
    let id = library_with_cutting_data(&folder, &["oak"]);
    library_with_cutting_data(&folder, &["Oak"]);
    let mut rig = Rig::cube("toollib-spelling", None);
    crate::theme::install_style(&rig.ctx, crate::theme::Theme::Dark);
    rig.size = egui::vec2(1366.0, 768.0);
    rig.shell.tool_library = ToolLibraryWindow::new(Some(folder.clone()));
    rig.shell
        .open_tool_library(Purpose::Add, Some(Chosen::Library(id)));
    for _ in 0..3 {
        rig.frame(Vec::new());
    }
    let row = centre_of(&rig, "oak");
    rig.click(row);
    for _ in 0..3 {
        rig.frame(Vec::new());
    }
    let apply = centre_of(&rig, "Apply");
    rig.click(apply);
    for _ in 0..3 {
        rig.frame(Vec::new());
    }
    rig.shell.tool_library.save();
    let reread = ToolLibrary::open(&folder).unwrap();
    let materials: Vec<_> = reread
        .get(id)
        .unwrap()
        .cutting_data
        .keys()
        .cloned()
        .collect();
    assert_eq!(materials, vec!["oak".to_owned()]);
    std::fs::remove_dir_all(&folder).ok();
}

/// The centre of the first text that contains `needle`.
fn centre_of(rig: &Rig, needle: &str) -> egui::Pos2 {
    rig.text_rects()
        .into_iter()
        .find(|(t, _)| t.contains(needle))
        .unwrap_or_else(|| panic!("no text {needle:?}"))
        .1
        .center()
}

/// A library with a Makera Z1 tool set holding one ball end mill, and one
/// flat end mill that no machine holds.
fn library_with_machine(folder: &std::path::Path) -> (ToolId, ToolId) {
    use openchuck_app::tools::{ControllerMapping, ToolSet, ToolSetEntry};
    let ball = library_with(folder, ToolType::BallEnd, "Z1 ball");
    let flat = library_with(folder, ToolType::FlatEnd, "Loose flat");
    let mut library = ToolLibrary::open(folder).unwrap();
    library
        .save_tool_set(ToolSet {
            machine: "Makera Z1".into(),
            entries: vec![ToolSetEntry {
                tool: ball,
                controller: ControllerMapping::numbered(1),
            }],
        })
        .unwrap();
    (ball, flat)
}

#[test]
fn req_tl_11_req_ui_15_the_list_has_foldable_machine_and_standard_sections_that_are_remembered() {
    let folder = library_folder("sections");
    let (ball, _) = library_with_machine(&folder);
    let mut rig = Rig::cube("toollib-sections", None);
    rig.shell.tool_library = ToolLibraryWindow::new(Some(folder.clone()));
    rig.shell
        .open_tool_library(Purpose::Browse, Some(Chosen::Library(ball)));
    rig.frame(Vec::new());
    rig.frame(Vec::new());
    for text in [
        "MAKERA Z1 (1)",
        "STANDARD TOOLS (1)",
        "BALL END MILLS (1)",
        "FLAT END MILLS (1)",
        "Loose flat",
    ] {
        assert!(rig.shows(text), "{text}");
    }
    // Folding the type group hides its tools; the section stays.
    rig.click(centre_of(&rig, "FLAT END MILLS (1)"));
    rig.frame(Vec::new());
    assert!(rig.shows("FLAT END MILLS (1)"));
    assert!(!rig.shows("Loose flat"), "a folded group shows no tools");
    assert!(rig.shows("STANDARD TOOLS (1)"));
    // Folding the machine section hides its groups.
    rig.click(centre_of(&rig, "MAKERA Z1 (1)"));
    rig.frame(Vec::new());
    assert!(!rig.shows("BALL END MILLS (1)"));
    // Both folds are in the settings, per user (UI-15).
    let view = rig.shell.settings.tool_library.clone();
    assert!(view.is_folded("machine:makera z1"));
    assert!(view.is_folded("standard/flat-end"));
    // Closing and opening the window again keeps them.
    rig.shell.tool_library.click(Chosen::Library(ball));
    rig.shell.tool_library.close_for_test();
    rig.frame(Vec::new());
    rig.shell.open_tool_library(Purpose::Browse, None);
    rig.frame(Vec::new());
    rig.frame(Vec::new());
    assert!(rig.shows("MAKERA Z1 (1)") && !rig.shows("BALL END MILLS (1)"));
    assert!(!rig.shows("Loose flat"));
    // Unfolding shows them again.
    rig.click(centre_of(&rig, "MAKERA Z1 (1)"));
    rig.frame(Vec::new());
    assert!(rig.shows("BALL END MILLS (1)"));
    assert!(
        !rig.shell
            .settings
            .tool_library
            .is_folded("machine:makera z1")
    );
    std::fs::remove_dir_all(&folder).ok();
}

#[test]
fn req_tl_04_the_size_filter_hides_tools_keeps_its_state_and_is_cleared_by_hand() {
    use openchuck_app::tool_library::{SizeFilter, ToolFilter};
    let folder = library_folder("filter");
    let (ball, _) = library_with_machine(&folder);
    // The new tools are 6 mm ball and 6 mm flat: make the flat one 10 mm.
    {
        let mut library = ToolLibrary::open(&folder).unwrap();
        let mut flat = library
            .tools()
            .find(|t| t.name == "Loose flat")
            .unwrap()
            .clone();
        flat.tool.diameter = openchuck_app::tools::Millimetres(10.0);
        flat.tool.shank_diameter = openchuck_app::tools::Millimetres(10.0);
        tool_library::save_draft(&mut library, &flat, &[]).unwrap();
        let mut ball = library.get(ball).unwrap().clone();
        ball.tool.diameter = openchuck_app::tools::Millimetres(6.0);
        ball.tool.shank_diameter = openchuck_app::tools::Millimetres(6.0);
        tool_library::save_draft(&mut library, &ball, &[]).unwrap();
    }
    let mut rig = Rig::cube("toollib-filter", None);
    rig.shell.tool_library = ToolLibraryWindow::new(Some(folder.clone()));
    rig.shell.settings.tool_library.filter = ToolFilter {
        diameter: SizeFilter {
            exact: "10".into(),
            ..SizeFilter::default()
        },
        ..ToolFilter::default()
    };
    rig.shell
        .open_tool_library(Purpose::Browse, Some(Chosen::Library(ball)));
    rig.frame(Vec::new());
    rig.frame(Vec::new());
    assert!(rig.shows("Loose flat"));
    // The machine section had only the 6 mm ball: gone with its group.
    assert!(!rig.shows("MAKERA Z1 ("), "a filter hides empty sections");
    assert!(rig.shows("Clear filter"));
    // Closed and opened again, the filter is still on (no reset).
    rig.shell.tool_library.close_for_test();
    rig.frame(Vec::new());
    rig.shell.open_tool_library(Purpose::Browse, None);
    rig.frame(Vec::new());
    rig.frame(Vec::new());
    assert!(rig.shows("Loose flat") && !rig.shows("MAKERA Z1 ("));
    // Clear filter empties it by hand and the machine section is back.
    rig.click(centre_of(&rig, "Clear filter"));
    rig.frame(Vec::new());
    assert!(rig.shell.settings.tool_library.filter.is_empty());
    assert!(rig.shows("MAKERA Z1 (1)"));
    std::fs::remove_dir_all(&folder).ok();
}

#[test]
fn req_tl_12_a_default_tool_shows_its_dimensions_read_only_and_copies_to_edit() {
    let folder = library_folder("default");
    let mut library = ToolLibrary::open(&folder).unwrap();
    library.install_defaults().unwrap();
    let default = library
        .tools()
        .find(|t| ToolType::of(&t.tool.shape) == ToolType::BullNose)
        .unwrap()
        .clone();
    let mut rig = Rig::cube("toollib-default", None);
    rig.shell.tool_library = ToolLibraryWindow::new(Some(folder.clone()));
    rig.shell
        .open_tool_library(Purpose::Browse, Some(Chosen::Library(default.id)));
    rig.frame(Vec::new());
    rig.frame(Vec::new());
    for text in [
        "A default tool: it cannot be edited",
        "Copy to edit",
        "Diameter",
        "Flute length",
        "Overall length",
        "Shank diameter",
        "Corner radius",
        "copy it to edit",
    ] {
        assert!(rig.shows(text), "{text}");
    }
    // Nothing to save or change in a default.
    assert!(!rig.shows("Saved in the library"));
    assert!(!rig.shows("Add material"));
    assert!(!rig.shows("Edit the profile"));
    // The copy is a new library draft; the default is unchanged.
    rig.click(centre_of(&rig, "Copy to edit"));
    let Some(Chosen::Library(copy)) = rig.shell.tool_library.chosen() else {
        panic!("{:?}", rig.shell.tool_library.chosen())
    };
    assert_ne!(copy, default.id);
    rig.frame(Vec::new());
    assert!(rig.shows("New: not saved yet"));
    assert!(!rig.shows("A default tool: it cannot be edited"));
    assert!(rig.shows(&format!("{} copy", default.name)));
    rig.shell.tool_library.save();
    let reread = ToolLibrary::open(&folder).unwrap();
    assert_eq!(reread.get(default.id), Some(&default));
    assert!(reread.get(copy).is_some());
    std::fs::remove_dir_all(&folder).ok();
}

#[test]
fn req_tl_01_esc_in_a_text_field_ends_the_edit_and_a_second_esc_closes_the_window() {
    // OI-536: Esc in a text field ended the edit and acted on the window too.
    let folder = library_folder("esc-field");
    let id = library_with(&folder, ToolType::FlatEnd, "6 mm flat");
    let mut rig = Rig::new(Shell::with_settings(false, None, None), Vec::new());
    rig.shell.tool_library = ToolLibraryWindow::new(Some(folder.clone()));
    rig.shell
        .open_tool_library(Purpose::Browse, Some(Chosen::Library(id)));
    rig.frame(Vec::new());
    rig.frame(Vec::new());
    // The search box has the keyboard: Esc ends the edit, the window stays.
    rig.click(centre_of(&rig, "Search tools"));
    rig.frame(Vec::new());
    assert!(rig.ctx.egui_wants_keyboard_input());
    rig.press(Key::Escape, Modifiers::NONE);
    rig.frame(Vec::new());
    assert!(rig.shell.tool_library.is_open(), "the first Esc closed it");
    assert!(!rig.ctx.egui_wants_keyboard_input());
    // With no field editing, Esc closes the window.
    rig.press(Key::Escape, Modifiers::NONE);
    rig.frame(Vec::new());
    assert!(!rig.shell.tool_library.is_open());
    std::fs::remove_dir_all(&folder).ok();
}

#[test]
fn req_tl_11_req_tl_12_a_changed_tool_set_number_of_a_default_tool_can_be_saved_or_cancelled() {
    // The default tool is read-only, but its machine tool set number is the
    // machine's (decision 0199 item 6): a change must offer Save and Cancel.
    let folder = library_folder("default-number");
    let mut library = ToolLibrary::open(&folder).unwrap();
    library.install_defaults().unwrap();
    let id = ToolId::from_u128(0xc0de_0001_0000_4000_8000_0000_0000_0101);
    let tool = library.get(id).expect("the first Z1 default tool").clone();
    let mut rig = Rig::cube("toollib-default-number", None);
    rig.shell.tool_library = ToolLibraryWindow::new(Some(folder.clone()));
    rig.shell
        .open_tool_library(Purpose::Browse, Some(Chosen::Library(id)));
    rig.frame(Vec::new());
    rig.frame(Vec::new());
    assert!(rig.shows("A default tool: it cannot be edited"));
    assert!(!rig.shows("Changed: not saved yet"));
    rig.shell.tool_library.set_set_number("Makera Z1", "777");
    rig.frame(Vec::new());
    assert!(rig.shows("Changed: not saved yet"), "no way to commit it");
    assert!(rig.shows("Save") && rig.shows("Cancel"));
    // Save writes the number; the tool itself is unchanged.
    rig.shell.tool_library.save();
    let reread = ToolLibrary::open(&folder).unwrap();
    assert_eq!(reread.get(id), Some(&tool));
    let set = reread.tool_set("Makera Z1").unwrap();
    assert_eq!(set.mapping_for(id).unwrap().number, 777);
    std::fs::remove_dir_all(&folder).ok();
}

/// The operation's Tool section has a Material preset row (issue 197;
/// decision record 0230): the setup's material by default, a warning when
/// another of the tool's materials is chosen, and "No preset" once a cutting
/// value is edited. New operation offers the same choice.
#[test]
fn req_tl_05_req_tl_09_the_operation_has_a_material_preset_row() {
    use super::inspector::ToolAct;
    use openchuck_app::project::Item;
    let mut rig = Rig::cube("toollib-preset", None);
    rig.size = egui::vec2(1440.0, 2400.0);
    let app = rig.shell.app.as_mut().unwrap();
    let mut tool = app.project().tools()[0].clone();
    let mut steel = tool.cutting_data["oak"];
    steel.spindle_speed = openchuck_app::tools::SpindleSpeed(6000.0);
    tool.cutting_data.insert("Steel".into(), steel);
    app.execute(openchuck_app::Command::Replace(Item::Tool(tool)))
        .unwrap();
    let op = app.project().operations()[0].id;
    rig.shell.select(Selection::Operation(op));
    for _ in 0..3 {
        rig.frame(Vec::new());
    }
    assert!(rig.shows("Material preset"), "the row is there");
    assert!(
        rig.shows("Setup's stock material: oak"),
        "the setup's by default"
    );
    rig.shell
        .tool_act(op, ToolAct::Preset(Some("Steel".into())));
    rig.frame(Vec::new());
    rig.frame(Vec::new());
    assert!(
        rig.shows("is not the setup's stock material"),
        "the warning"
    );
    rig.shell.tool_act(op, ToolAct::Spindle("9000".into()));
    rig.frame(Vec::new());
    rig.frame(Vec::new());
    assert!(rig.shows("No preset"), "an edit drops the preset");
    rig.shell.tool_act(op, ToolAct::Preset(None));
    let project = rig.shell.app.as_ref().unwrap().project();
    assert_eq!(project.operations()[0].material, None);
    assert_eq!(project.operations()[0].spindle_speed, None);
    // New operation offers the preset under the tool.
    let app = rig.shell.app.as_ref().unwrap();
    rig.shell.run.begin_add(app, None, None);
    rig.frame(Vec::new());
    rig.frame(Vec::new());
    assert!(rig.shows("Material preset"));
}

fn library_bytes(folder: &std::path::Path) -> std::collections::BTreeMap<PathBuf, Vec<u8>> {
    let mut files = std::collections::BTreeMap::new();
    for entry in std::fs::read_dir(folder).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files.extend(library_bytes(&path));
        } else {
            files.insert(path.clone(), std::fs::read(path).unwrap());
        }
    }
    files
}

fn cleanup_click(rig: &mut Rig, label: &str) {
    let at = rig
        .visible_text_rects()
        .into_iter()
        .rev()
        .find(|(text, _)| text == label)
        .unwrap_or_else(|| panic!("missing {label}: {:?}", rig.texts()))
        .1
        .center();
    rig.click(at);
    for _ in 0..3 {
        rig.frame(Vec::new());
    }
}

#[test]
fn req_tl_07_cleanup_reviews_confirms_cancels_and_undoes_without_library_changes() {
    let folder = library_folder("cleanup");
    let id = library_with(&folder, ToolType::BullNose, "Unused cleanup tool");
    let mut rig = Rig::cube("cleanup", None);
    let app = rig.shell.app.as_mut().unwrap();
    let lib = ToolLibrary::open(&folder).unwrap();
    let (cmd, _, _) = tool_library::add_to_project(app.project(), &lib, id).unwrap();
    app.execute(cmd).unwrap();
    let before = app.project().clone();
    rig.shell.tool_library = ToolLibraryWindow::new(Some(folder.clone()));
    rig.shell.tool_library.open(Purpose::Browse, None);
    rig.frame(Vec::new());
    rig.frame(Vec::new());
    let library_before = library_bytes(&folder);
    cleanup_click(&mut rig, "Remove unused tools…");
    rig.frame(Vec::new());
    rig.frame(Vec::new());
    assert!(
        rig.shows("Only project copies are removed."),
        "{:?}",
        rig.texts()
    );
    assert!(rig.shows("Tools used by disabled operations are retained."));
    assert!(
        rig.shows("Corner radius"),
        "Subtype dimensions must distinguish tools of the same diameter"
    );
    cleanup_click(&mut rig, "Unused cleanup tool — T2");
    cleanup_click(&mut rig, "Unused cleanup tool — T2");
    cleanup_click(&mut rig, "Remove selected…");
    assert!(!rig.shows("Remove these project copies?"));
    cleanup_click(&mut rig, "Unused cleanup tool — T2");
    cleanup_click(&mut rig, "Remove selected…");
    rig.frame(Vec::new());
    assert!(rig.shows("Remove these project copies?"));
    cleanup_click(&mut rig, "Cancel");
    assert_eq!(rig.shell.app.as_ref().unwrap().project(), &before);
    cleanup_click(&mut rig, "Remove unused tools…");
    rig.frame(Vec::new());
    cleanup_click(&mut rig, "Unused cleanup tool — T2");
    cleanup_click(&mut rig, "Remove selected…");
    rig.frame(Vec::new());
    cleanup_click(&mut rig, "Remove");
    assert_eq!(rig.shell.app.as_ref().unwrap().project().tools().len(), 1);
    assert!(ToolLibrary::open(&folder).unwrap().get(id).is_some());
    assert_eq!(library_bytes(&folder), library_before);
    rig.shell.undo();
    assert_eq!(rig.shell.app.as_ref().unwrap().project(), &before);
    std::fs::remove_dir_all(folder).unwrap();
}

#[test]
fn req_tl_07_cleanup_refreshes_changed_candidates_and_clears_selection() {
    let folder = library_folder("cleanup-refresh");
    let id = library_with(&folder, ToolType::FlatEnd, "Refresh me");
    let mut rig = Rig::cube("cleanup-refresh", None);
    let lib = ToolLibrary::open(&folder).unwrap();
    let app = rig.shell.app.as_mut().unwrap();
    let (cmd, _, _) = tool_library::add_to_project(app.project(), &lib, id).unwrap();
    app.execute(cmd).unwrap();
    rig.shell.tool_library = ToolLibraryWindow::new(Some(folder.clone()));
    rig.shell.tool_library.open(Purpose::Browse, None);
    for _ in 0..3 {
        rig.frame(Vec::new());
    }
    cleanup_click(&mut rig, "Remove unused tools…");
    cleanup_click(&mut rig, "Refresh me — T2");
    cleanup_click(&mut rig, "Remove selected…");
    let app = rig.shell.app.as_mut().unwrap();
    let cmd = tool_library::set_project_tool_number(app.project(), id, 7).unwrap();
    app.execute(cmd).unwrap();
    let before = app.project().clone();
    cleanup_click(&mut rig, "Remove");
    assert!(rig.shows("Nothing was removed"));
    assert!(rig.shows("Refresh me — T7"));
    assert_eq!(rig.shell.app.as_ref().unwrap().project(), &before);
    // A cleared selection cannot be confirmed by clicking the disabled button.
    cleanup_click(&mut rig, "Remove selected…");
    assert!(!rig.shows("Remove these project copies?"));
    cleanup_click(&mut rig, "Cancel");
    assert_eq!(rig.shell.app.as_ref().unwrap().project(), &before);
    std::fs::remove_dir_all(folder).unwrap();
}

#[test]
fn req_tl_07_req_ui_14_req_ui_16_cleanup_empty_escape_and_scaled_layout() {
    for (width, height, scale) in [
        (1366.0, 768.0, 1.0),
        (1366.0, 768.0, 1.2),
        (1920.0, 1080.0, 1.0),
    ] {
        let mut rig = Rig::cube(&format!("cleanup-empty-{scale}-{width}"), None);
        crate::theme::install_fonts(&rig.ctx);
        crate::theme::install_style(&rig.ctx, crate::theme::Theme::Dark);
        rig.size = egui::vec2(width, height);
        rig.ctx.set_pixels_per_point(scale);
        let folder = rig.dirs[0].join("empty-library");
        rig.shell.tool_library = ToolLibraryWindow::new(Some(folder));
        rig.shell.tool_library.open(Purpose::Browse, None);
        for _ in 0..3 {
            rig.frame(Vec::new());
        }
        let before = rig.shell.app.as_ref().unwrap().project().clone();
        cleanup_click(&mut rig, "Remove unused tools…");
        assert!(rig.shows("No unused project tools."));
        let rect = rig
            .visible_text_rects()
            .into_iter()
            .rev()
            .find(|(t, _)| t == "Close")
            .unwrap()
            .1;
        assert!(rect.top() >= 0.0 && rect.bottom() <= height && rect.right() <= width);
        rig.press(egui::Key::Escape, egui::Modifiers::NONE);
        for _ in 0..3 {
            rig.frame(Vec::new());
        }
        assert!(rig.shell.tool_library.is_open());
        assert!(!rig.shows("No unused project tools."));
        assert_eq!(rig.shell.app.as_ref().unwrap().project(), &before);
    }
}

#[test]
fn req_tl_07_req_ui_14_cleanup_checkboxes_and_confirmation_use_the_keyboard() {
    let folder = library_folder("cleanup-keys");
    let id = library_with(&folder, ToolType::FlatEnd, "Keyboard tool");
    let mut rig = Rig::cube("cleanup-keys", None);
    let lib = ToolLibrary::open(&folder).unwrap();
    let app = rig.shell.app.as_mut().unwrap();
    let (cmd, _, _) = tool_library::add_to_project(app.project(), &lib, id).unwrap();
    app.execute(cmd).unwrap();
    rig.shell.tool_library = ToolLibraryWindow::new(Some(folder.clone()));
    rig.shell.tool_library.open(Purpose::Browse, None);
    for _ in 0..3 {
        rig.frame(Vec::new());
    }
    cleanup_click(&mut rig, "Remove unused tools…");
    // The first Tab enters the checklist; Space selects the focused tool.
    rig.press(egui::Key::Tab, egui::Modifiers::NONE);
    rig.press(egui::Key::Space, egui::Modifiers::NONE);
    rig.press(egui::Key::Tab, egui::Modifiers::NONE);
    rig.press(egui::Key::Enter, egui::Modifiers::NONE);
    for _ in 0..3 {
        rig.frame(Vec::new());
    }
    assert!(
        rig.shows("Remove these project copies?"),
        "{:?}",
        rig.texts()
    );
    // Focus stays on the explicit confirmation action.
    rig.press(egui::Key::Enter, egui::Modifiers::NONE);
    for _ in 0..3 {
        rig.frame(Vec::new());
    }
    assert_eq!(rig.shell.app.as_ref().unwrap().project().tools().len(), 1);
    std::fs::remove_dir_all(folder).unwrap();
}

#[test]
fn req_tl_07_cleanup_works_when_the_library_is_unavailable() {
    let mut rig = Rig::cube("cleanup-without-library", None);
    let app = rig.shell.app.as_mut().unwrap();
    let mut tool = app.project().tools()[0].clone();
    tool.id = ToolId::new();
    tool.name = "Standalone copy".into();
    tool.controller.number = 2;
    app.execute(openchuck_app::Command::Add {
        item: openchuck_app::project::Item::Tool(tool),
        placement: openchuck_app::project::Placement::End,
    })
    .unwrap();
    // An ordinary file cannot be opened as a library directory.
    let path = rig.dirs[0].join("unavailable-library");
    std::fs::write(&path, b"leave this file unchanged").unwrap();
    rig.shell.tool_library = ToolLibraryWindow::new(Some(path.clone()));
    rig.shell.tool_library.open(Purpose::Browse, None);
    for _ in 0..3 {
        rig.frame(Vec::new());
    }
    cleanup_click(&mut rig, "Remove unused tools…");
    cleanup_click(&mut rig, "Standalone copy — T2");
    cleanup_click(&mut rig, "Remove selected…");
    cleanup_click(&mut rig, "Remove");
    assert_eq!(rig.shell.app.as_ref().unwrap().project().tools().len(), 1);
    assert_eq!(std::fs::read(path).unwrap(), b"leave this file unchanged");
}

#[test]
fn req_tl_07_req_ui_16_cleanup_large_list_keeps_actions_visible_at_supported_sizes() {
    for (width, height, scale) in [
        (1366.0, 768.0, 1.0),
        (1366.0, 768.0, 1.2),
        (1920.0, 1080.0, 1.0),
    ] {
        let mut rig = Rig::cube(&format!("cleanup-large-{width}-{scale}"), None);
        crate::theme::install_fonts(&rig.ctx);
        crate::theme::install_style(&rig.ctx, crate::theme::Theme::Dark);
        rig.size = egui::vec2(width, height);
        rig.ctx.set_pixels_per_point(scale);
        let app = rig.shell.app.as_mut().unwrap();
        let template = app.project().tools()[0].clone();
        for i in 0..40 {
            let mut tool = template.clone();
            tool.id = ToolId::new();
            tool.name = format!("Unused tool {i:02}");
            tool.controller.number = i + 2;
            app.execute(openchuck_app::Command::Add {
                item: openchuck_app::project::Item::Tool(tool),
                placement: openchuck_app::project::Placement::End,
            })
            .unwrap();
        }
        let folder = rig.dirs[0].join("library");
        rig.shell.tool_library = ToolLibraryWindow::new(Some(folder));
        rig.shell.tool_library.open(Purpose::Browse, None);
        for _ in 0..3 {
            rig.frame(Vec::new());
        }
        cleanup_click(&mut rig, "Remove unused tools…");
        for label in [
            "Only project copies are removed. The tool library is unchanged.",
            "Unused tool 00 — T2",
            "Remove selected…",
            "Cancel",
        ] {
            let rect = rig
                .visible_text_rects()
                .into_iter()
                .rev()
                .find(|(t, _)| t == label)
                .unwrap_or_else(|| panic!("missing {label}"))
                .1;
            assert!(
                rect.top() >= 0.0
                    && rect.bottom() <= height
                    && rect.left() >= 0.0
                    && rect.right() <= width,
                "{label}: {rect:?}"
            );
        }
        cleanup_click(&mut rig, "Unused tool 00 — T2");
        cleanup_click(&mut rig, "Remove selected…");
        assert!(rig.shows("Remove these project copies?"));
        cleanup_click(&mut rig, "Cancel");
        assert_eq!(rig.shell.app.as_ref().unwrap().project().tools().len(), 41);
    }
}
