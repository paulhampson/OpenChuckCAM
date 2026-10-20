// SPDX-License-Identifier: AGPL-3.0-or-later
//! Tools out of the tree, the operation's Tool section and the tool library
//! window, drawn whole without a window (UI-06, TL-01, TL-07, TL-11;
//! specification sections 6, 8 and 12.1; decision record 0127; trial
//! feedback F-10, issue 148).

#![allow(clippy::unwrap_used)]

use std::path::PathBuf;

use openchuck_app::tool_library::{self, ToolType};
use openchuck_app::tools::ToolLibrary;

use super::frame_tests::Rig;
use super::*;
use crate::tool_library_window::{Purpose, ToolLibraryWindow};

fn library_folder(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "openchuck-tool-library-window-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
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
    assert!(rig.shows("Not in the library; the project keeps its copy"));
    // The number is changed there, as one undoable command (TL-06).
    let tool = rig.shell.app.as_ref().unwrap().project().tools()[0].id;
    rig.shell
        .tool_act(op, super::inspector::ToolAct::Number(tool, "T7".into()));
    assert_eq!(
        rig.shell.app.as_ref().unwrap().project().tools()[0]
            .controller
            .number,
        7
    );
    rig.shell.undo();
    assert_eq!(
        rig.shell.app.as_ref().unwrap().project().tools()[0]
            .controller
            .number,
        1
    );
    std::fs::remove_dir_all(&folder).ok();
}

#[test]
fn req_tl_01_req_tl_07_the_tools_menu_opens_the_library_and_adds_a_tool() {
    let folder = library_folder("add");
    let mut library = ToolLibrary::open(&folder).unwrap();
    let id = tool_library::new_tool(&mut library, ToolType::FlatEnd).unwrap();
    tool_library::set_name(&mut library, id, "6 mm flat").unwrap();
    let mut rig = Rig::cube("toollib-add", None);
    rig.shell.tool_library = ToolLibraryWindow::new(Some(folder.clone()));
    rig.shell.open_tool_library(Purpose::Add, Some(id));
    rig.frame(Vec::new());
    rig.frame(Vec::new());
    assert!(rig.shows("Tool library"));
    assert!(rig.shows("FLAT END MILLS"), "{:?}", rig.texts());
    assert!(rig.shows("6 mm flat"));
    assert!(rig.shows("Add to project"));
    assert!(rig.shows("CUTTING DATA BY MATERIAL"));
    assert!(rig.shows("MACHINE TOOL SETS"));
    assert!(rig.shows("Import library…"));
    assert!(rig.shows("Export library…"));
    assert!(!rig.shows("in project"));
    // Add to project: one undoable command, then the chip shows.
    rig.shell.tool_library.add_chosen(rig.shell.app.as_mut());
    rig.frame(Vec::new());
    assert_eq!(rig.shell.app.as_ref().unwrap().project().tools().len(), 2);
    assert!(rig.shows("in project"));
    rig.shell.undo();
    assert_eq!(rig.shell.app.as_ref().unwrap().project().tools().len(), 1);
    std::fs::remove_dir_all(&folder).ok();
}

#[test]
fn req_tl_07_choosing_from_the_library_gives_the_operation_the_tool() {
    let folder = library_folder("operation");
    let mut library = ToolLibrary::open(&folder).unwrap();
    let id = tool_library::new_tool(&mut library, ToolType::BallEnd).unwrap();
    let mut rig = Rig::cube("toollib-op", None);
    rig.shell.tool_library = ToolLibraryWindow::new(Some(folder.clone()));
    let op = rig.shell.app.as_ref().unwrap().project().operations()[0].id;
    rig.shell
        .open_tool_library(Purpose::Operation(op), Some(id));
    rig.frame(Vec::new());
    rig.frame(Vec::new());
    assert!(rig.shows("choose the operation's tool"));
    rig.shell.tool_library.add_chosen(rig.shell.app.as_mut());
    let project = rig.shell.app.as_ref().unwrap().project();
    assert_eq!(project.operation(op).unwrap().tool, Some(id));
    assert_eq!(project.tools().len(), 2);
    // One undo step takes both back.
    rig.shell.undo();
    let project = rig.shell.app.as_ref().unwrap().project();
    assert_ne!(project.operation(op).unwrap().tool, Some(id));
    assert_eq!(project.tools().len(), 1);
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
    assert!(rig.shell.tool_library.is_open());
    std::fs::remove_dir_all(&folder).ok();
}
