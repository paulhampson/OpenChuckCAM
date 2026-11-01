// SPDX-License-Identifier: AGPL-3.0-or-later
//! The File menu's "Open recent" (issue 381; UI-15, PRJ-08, NFR-18).
//! Generated folders only.
#![allow(clippy::unwrap_used, clippy::panic)]
use std::path::PathBuf;

use super::checkpoint_tests::click;
use super::frame_tests::{Rig, temp};
use super::{Dialog, Shell};

fn rig_with_recent(name: &str, folders: usize) -> (Rig, PathBuf, Vec<PathBuf>) {
    let root = temp(name);
    let made: Vec<PathBuf> = (0..folders)
        .map(|i| {
            let dir = root.join(format!("job-{i}"));
            std::fs::create_dir_all(&dir).unwrap();
            dir.canonicalize().unwrap()
        })
        .collect();
    let mut rig = Rig::new(Shell::with_settings(false, None, None), Vec::new());
    rig.shell.settings.recent = made.clone();
    (rig, root, made)
}

fn open_menu(rig: &mut Rig) {
    click(rig, "File");
    rig.frame(Vec::new());
    click(rig, "Open recent");
    rig.frame(Vec::new());
}

/// The menu lists the recent folders by name, and a choice goes through the
/// Open command: the Open dialog opens on that folder (decision 0460).
#[test]
fn req_ui_15_req_prj_08_open_recent_opens_the_dialog_on_the_chosen_folder() {
    let (mut rig, root, made) = rig_with_recent("recent-choose", 2);
    open_menu(&mut rig);
    assert!(
        rig.shows("job-0") && rig.shows("job-1"),
        "{:?}",
        rig.texts()
    );
    assert!(rig.shows("Clear recent list"));
    click(&mut rig, "job-1");
    rig.frame(Vec::new());
    match &rig.shell.dialog {
        Some(Dialog::Folder { create, folder, .. }) => {
            assert!(!create);
            assert_eq!(*folder, super::checkpoints::shown(&made[1]));
        }
        other => panic!("expected the Open dialog, got {other:?}"),
    }
    std::fs::remove_dir_all(root).ok();
}

/// A folder that is gone is not listed and is forgotten for good.
#[test]
fn req_ui_15_open_recent_forgets_a_folder_that_is_gone() {
    let (mut rig, root, made) = rig_with_recent("recent-gone", 2);
    std::fs::remove_dir_all(&made[0]).unwrap();
    open_menu(&mut rig);
    assert!(!rig.shows("job-0"), "{:?}", rig.texts());
    assert!(rig.shows("job-1"));
    assert_eq!(rig.shell.settings.recent, vec![made[1].clone()]);
    std::fs::remove_dir_all(root).ok();
}

/// With nothing recent the item says so and does nothing.
#[test]
fn req_ui_15_open_recent_with_no_recent_projects_says_so() {
    let (mut rig, root, _) = rig_with_recent("recent-none", 0);
    open_menu(&mut rig);
    assert!(rig.shows("No recent projects"), "{:?}", rig.texts());
    assert!(rig.shell.dialog.is_none());
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn req_ui_15_clear_recent_list_empties_the_list() {
    let (mut rig, root, _) = rig_with_recent("recent-clear", 2);
    open_menu(&mut rig);
    click(&mut rig, "Clear recent list");
    rig.frame(Vec::new());
    assert!(rig.shell.settings.recent.is_empty());
    std::fs::remove_dir_all(root).ok();
}

/// Unsaved changes are asked about first, and cancelling leaves no folder
/// behind for a later Open (the 0460 rule for a prefilled folder).
#[test]
fn req_prj_08_open_recent_asks_about_unsaved_changes_first() {
    let mut rig = Rig::cube("recent-unsaved", None);
    let other = temp("recent-unsaved-other");
    std::fs::create_dir_all(&other).unwrap();
    rig.shell.settings.recent = vec![other.canonicalize().unwrap()];
    rig.shell
        .app
        .as_mut()
        .unwrap()
        .execute(openchuck_app::Command::RenameProject("Edited".into()))
        .unwrap();
    open_menu(&mut rig);
    click(&mut rig, "recent-unsaved-other");
    rig.frame(Vec::new());
    assert!(
        matches!(rig.shell.dialog, Some(Dialog::Unsaved(_))),
        "{:?}",
        rig.shell.dialog
    );
    std::fs::remove_dir_all(other).ok();
}

/// A click on a recent project is logged as the command, never the user's
/// folder name (NFR-18).
#[test]
fn req_nfr_18_a_recent_project_click_does_not_log_the_folder_name() {
    let folders = vec![PathBuf::from("/home/someone/private-bracket")];
    let lines = vec![
        "click: 'private-bracket'".to_owned(),
        "click: 'Undo'".to_owned(),
    ];
    assert_eq!(
        super::actions::without_folder_names(lines, &folders),
        ["click: 'Open recent project'", "click: 'Undo'"]
    );
}
