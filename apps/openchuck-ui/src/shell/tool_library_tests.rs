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
