// SPDX-License-Identifier: AGPL-3.0-or-later
//! A project whose model is outside its folder (issue 385, decision record
//! 0307): the question on opening, the project tree's link mark and the
//! Model inspector's "Copy model into project".

#![allow(clippy::unwrap_used)]

use eframe::egui::{Event, Modifiers, PointerButton};
use openchuck_app::ModelStorage;
use openchuck_app::project::ModelRef;

use super::frame_tests::Rig;
use super::*;

/// A saved project in a scratch folder whose one model is linked from a
/// folder beside it; the rig has it open.
fn linked(tag: &str) -> (Rig, PathBuf, ModelId) {
    let root = std::env::temp_dir().join(format!("oc-linked-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("elsewhere")).unwrap();
    let cube = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/stl/cube_ascii.stl");
    std::fs::copy(cube, root.join("elsewhere/cube.stl")).unwrap();
    let dir = root.join("project");
    let mut app = Application::create(&dir, "linked", Threads::Exactly(1)).unwrap();
    let (id, _) = app
        .add_model_as(&root.join("elsewhere/cube.stl"), None, ModelStorage::Link)
        .unwrap();
    app.save().unwrap();
    drop(app);
    let mut rig = Rig::new(
        Shell::with_settings(false, Some(dir), None),
        vec![root.clone()],
    );
    rig.settle();
    (rig, root, id)
}

/// Clicks the text painted with exactly `label`.
fn click(rig: &mut Rig, label: &str) {
    for _ in 0..3 {
        rig.frame(Vec::new());
    }
    let at = rig
        .text_rects()
        .iter()
        .rev()
        .find(|(t, _)| t == label)
        .map(|(_, r)| r.center())
        .unwrap_or_else(|| panic!("{label} is painted"));
    rig.frame(vec![Event::PointerMoved(at)]);
    let button = |pressed| Event::PointerButton {
        pos: at,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    rig.frame(vec![button(true)]);
    rig.frame(vec![button(false)]);
    rig.frame(Vec::new());
}

/// Draws until the copy job has ended and its result is stored.
fn copied(rig: &mut Rig) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while rig.shell.copy_job.is_some() {
        rig.frame(Vec::new());
        assert!(std::time::Instant::now() < deadline, "the copy timed out");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    rig.settle();
}

fn model(rig: &Rig, id: ModelId) -> ModelRef {
    rig.shell
        .app
        .as_ref()
        .unwrap()
        .find_model(&id.to_string())
        .unwrap()
        .clone()
}

#[test]
fn req_prj_02_opening_a_project_with_a_linked_model_asks_to_copy_it() {
    let (mut rig, root, id) = linked("ask");
    assert_eq!(rig.shell.dialog, Some(Dialog::CopyLinked(1)));
    rig.frame(Vec::new());
    assert!(rig.shows("This project's model is outside its folder. Copy it into the project now?"));
    assert_eq!(
        copy_linked_question(2),
        "This project has 2 models outside its folder. Copy them into the project now?"
    );
    click(&mut rig, "Copy");
    copied(&mut rig);
    assert_eq!(rig.shell.dialog, None);
    let copied = model(&rig, id);
    assert!(
        !copied.path.is_outside_project(),
        "{}",
        copied.path.as_str()
    );
    assert!(root.join("project").join(copied.path.as_str()).is_file());
    assert_eq!(rig.shell.status, "Copied 1 model into the project folder");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn req_prj_02_not_now_keeps_the_link_marked_with_a_way_to_copy_it() {
    let (mut rig, root, id) = linked("not-now");
    click(&mut rig, "Not now");
    assert_eq!(rig.shell.dialog, None);
    assert!(model(&rig, id).path.is_outside_project());
    // The tree's row says so in words as well as by its mark (UI-11).
    rig.settle();
    assert!(rig.shows(tree::LINKED_CHIP));
    // The Model inspector says where the file is, with the action.
    rig.shell.selected = Some(Selection::Model(id));
    rig.settle();
    assert!(rig.shows("cube.stl, linked from outside the project folder (../elsewhere/cube.stl)"));
    click(&mut rig, crate::model_panel::COPY_MODEL_LABEL);
    copied(&mut rig);
    let copied = model(&rig, id);
    assert!(
        !copied.path.is_outside_project(),
        "{}",
        copied.path.as_str()
    );
    assert!(rig.shows("cube.stl, stored in the project folder (models/"));
    assert!(!rig.shows(tree::LINKED_CHIP));
    let _ = std::fs::remove_dir_all(root);
}

/// Opens the import dialog on `file` and waits for its check.
fn import(rig: &mut Rig, file: PathBuf) {
    let app = rig.shell.app.as_ref().unwrap();
    rig.shell.import = Some(crate::import_panel::ImportSession::new(app, file));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while rig.shell.import.as_ref().unwrap().preview.is_none() {
        rig.frame(Vec::new());
        assert!(std::time::Instant::now() < deadline, "the check timed out");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    for _ in 0..4 {
        rig.frame(Vec::new());
    }
}

/// Clearing the tick box links the file instead of copying it; a file
/// already in the project folder is used where it is, and says so.
#[test]
fn req_prj_02_clearing_the_copy_box_links_the_file() {
    let (mut rig, root, _) = linked("untick");
    click(&mut rig, "Not now");
    std::fs::copy(
        root.join("elsewhere/cube.stl"),
        root.join("elsewhere/other.stl"),
    )
    .unwrap();
    import(&mut rig, root.join("elsewhere/other.stl"));
    assert!(
        rig.shows("It will be copied into the project's models folder and listed as other.stl")
    );
    click(&mut rig, crate::import_panel::COPY_LABEL);
    assert!(!rig.shell.import.as_ref().unwrap().copy);
    assert!(rig.shows("The file stays where it is and is linked by its path."));
    click(&mut rig, "Add to project");
    rig.settle();
    let app = rig.shell.app.as_ref().unwrap();
    let added = app.project().models().last().unwrap();
    assert_eq!(added.path.as_str(), "../elsewhere/other.stl");

    // A file inside the project folder.
    let inside = root.join("project/inside.stl");
    std::fs::copy(root.join("elsewhere/cube.stl"), &inside).unwrap();
    import(&mut rig, inside);
    assert!(rig.shows("The file is already in the project folder."));
    let _ = std::fs::remove_dir_all(root);
}

/// A changed linked file is refused with what to do, both from the opening
/// question and from the Model inspector; nothing changes (decision record
/// 0307 item 10).
#[test]
fn req_prj_02_req_nfr_07_a_changed_linked_file_is_refused_with_what_to_do() {
    let (mut rig, root, id) = linked("refuse");
    std::fs::write(
        root.join("elsewhere/cube.stl"),
        "solid edited\nendsolid edited\n",
    )
    .unwrap();
    click(&mut rig, "Copy");
    copied(&mut rig);
    let refusal =
        "'cube.stl' has changed since it was added, so copying it would change the model.";
    match &rig.shell.dialog {
        Some(Dialog::Error(text)) => assert!(text.starts_with(refusal), "{text}"),
        other => panic!("expected the refusal, got {other:?}"),
    }
    assert!(model(&rig, id).path.is_outside_project());
    rig.shell.dialog = None;
    rig.shell.selected = Some(Selection::Model(id));
    rig.settle();
    click(&mut rig, crate::model_panel::COPY_MODEL_LABEL);
    copied(&mut rig);
    assert!(rig.shows(refusal));
    assert!(model(&rig, id).path.is_outside_project());
    let _ = std::fs::remove_dir_all(root);
}
