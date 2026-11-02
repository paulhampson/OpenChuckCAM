// SPDX-License-Identifier: AGPL-3.0-or-later
//! The unreachable areas through the shell, from a real run of simultaneous
//! finishing on a dovetail (OP-20, UI-11, UI-12; decision record 0637; issue
//! #416): when the areas are drawn, and what "Show in view N" in Messages
//! does.

#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::float_cmp)]

use std::time::{Duration, Instant};

use openchuck_app::Command;
use openchuck_app::overview::{ItemRef, MessageAction, Step};
use openchuck_app::project::Item;
use openchuck_app::project::id::OperationId;
use openchuck_viewport::Layer;

use super::frame_tests::Rig;
use super::{Selection, Shell};
use crate::viewport::{SimulateShow, ViewStep};

/// The dovetail project generated and simulated, on the Operations step,
/// with the areas found. Takes a few seconds.
fn generated(name: &str) -> (Rig, OperationId) {
    let (dir, app, op, _) = crate::testing::dovetail_project(name);
    app.save().unwrap();
    drop(app);
    let shell = Shell::with_settings(false, Some(dir.clone()), None);
    let mut rig = Rig::new(shell, vec![dir]);
    rig.settle();
    rig.shell.generate(true);
    rig.settle();
    rig.shell.open_step(Step::Operations);
    wait_for_areas(&mut rig);
    (rig, op)
}

/// Draws until the viewport holds dots.
fn wait_for_areas(rig: &mut Rig) {
    let deadline = Instant::now() + Duration::from_secs(120);
    while rig
        .shell
        .viewport
        .layer(Layer::Areas)
        .is_none_or(|l| l.meshes.is_empty())
    {
        assert!(Instant::now() < deadline, "no unreachable areas were drawn");
        rig.frame(Vec::new());
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn dots(rig: &Rig) -> usize {
    rig.shell
        .viewport
        .layer(Layer::Areas)
        .map_or(0, |l| l.meshes.len())
}

/// OP-20, UI-11, decision 0637 item 3: on Operations and on Simulate an
/// operation's areas are drawn while its toolpath is shown (its eye, and the
/// overall Toolpaths choice) and its result is current; not once it is out
/// of date.
#[test]
fn req_ui_11_areas_are_drawn_while_the_operations_path_is_shown_and_current() {
    let (mut rig, op) = generated("area-shown");
    assert!(dots(&rig) > 0);
    let drawn = dots(&rig);
    // The eye in the tree: hidden, no areas; shown again, the same areas.
    for shown in [false, true] {
        rig.shell
            .app
            .as_mut()
            .unwrap()
            .execute(Command::SetPathsShown {
                operations: vec![op],
                shown,
            })
            .unwrap();
        rig.settle();
        rig.frame(Vec::new());
        assert_eq!(dots(&rig), if shown { drawn } else { 0 }, "eye {shown}");
    }
    // The overall Toolpaths choice.
    let mut show = rig.shell.viewport.visibility();
    show.toolpaths = false;
    rig.shell.viewport.set_visibility(show);
    rig.frame(Vec::new());
    assert_eq!(dots(&rig), 0);
    show.toolpaths = true;
    rig.shell.viewport.set_visibility(show);
    rig.frame(Vec::new());
    assert_eq!(dots(&rig), drawn);
    // Simulate: its Toolpaths box is that choice there.
    rig.shell.open_step(Step::Simulate);
    rig.shell.viewport.set_simulate_show(SimulateShow {
        model: true,
        cut_part: true,
        toolpath: false,
    });
    for _ in 0..3 {
        rig.frame(Vec::new());
    }
    assert_eq!(rig.shell.viewport.step(), ViewStep::Simulate);
    assert_eq!(dots(&rig), 0, "Simulate's Toolpaths box is off");
    rig.shell.viewport.set_simulate_show(SimulateShow {
        model: true,
        cut_part: true,
        toolpath: true,
    });
    for _ in 0..3 {
        rig.frame(Vec::new());
    }
    assert_eq!(dots(&rig), drawn);
    // Out of date: the project changed after the run.
    let app = rig.shell.app.as_mut().unwrap();
    let mut operation = app.project().operation(op).unwrap().clone();
    operation.name = "Renamed".into();
    operation.parameters.insert(
        "stepover".to_owned(),
        openchuck_app::project::ParameterValue::Number(0.6),
    );
    app.execute(Command::Replace(Item::Operation(operation)))
        .unwrap();
    for _ in 0..3 {
        rig.frame(Vec::new());
    }
    assert_eq!(dots(&rig), 0, "the areas go with their toolpath");
}

/// UI-12, decision 0637 item 5: from Simulate, "Show in view N" stays on
/// Simulate, selects the operation, turns the camera to look at the area
/// and leaves the playback where it was and the part where it is.
#[test]
fn req_ui_12_show_in_view_frames_the_area_and_keeps_the_playback_in_the_shell() {
    let (mut rig, op) = generated("area-simulate");
    rig.shell.open_step(Step::Simulate);
    rig.shell.viewport.set_simulate_show(SimulateShow {
        model: true,
        cut_part: true,
        toolpath: true,
    });
    for _ in 0..3 {
        rig.frame(Vec::new());
    }
    wait_for_areas(&mut rig);
    // The playback as it stands.
    let cursor = rig.shell.play.pose().map(|p| p.cursor);
    let turn = rig.shell.viewport.part_turn();
    let (transforms, camera) = (rig.shell.viewport.transforms(), rig.shell.viewport.camera);
    rig.shell.message_action(MessageAction::ShowInView {
        operation: op,
        area: 1,
    });
    let deadline = Instant::now() + Duration::from_secs(60);
    while rig.shell.areas.is_waiting() {
        assert!(Instant::now() < deadline, "the view never turned");
        rig.frame(Vec::new());
        std::thread::sleep(Duration::from_millis(5));
    }
    rig.frame(Vec::new());
    assert_eq!(rig.shell.step, Step::Simulate);
    assert_eq!(rig.shell.selected, Some(Selection::Operation(op)));
    assert_ne!(rig.shell.viewport.camera, camera, "the camera turned");
    assert_eq!(rig.shell.play.pose().map(|p| p.cursor), cursor);
    assert_eq!(rig.shell.viewport.part_turn(), turn);
    assert_eq!(rig.shell.viewport.transforms(), transforms);
    // The camera is on the area's X range.
    let run = rig.shell.run.last().unwrap().clone();
    let area = &run.groups[0].unreached[0];
    let x = rig.shell.viewport.camera.target[0];
    assert!(
        (area.x.0 - 1e-6..=area.x.1 + 1e-6).contains(&x),
        "{x} against {:?}",
        area.x
    );
    // One step back undoes it.
    assert!(rig.shell.viewport.undo_camera());
    assert_eq!(rig.shell.viewport.camera, camera);
}

/// UI-12: from another step "Show in view N" goes to Operations; the links
/// are in Messages under the warning, one for each area it lists.
#[test]
fn req_ui_12_messages_link_to_each_area_and_go_to_operations() {
    let (mut rig, op) = generated("area-messages");
    let run = rig.shell.run.last().unwrap().clone();
    let count = run.groups[0].unreached.len().min(5);
    assert!(count >= 1);
    rig.shell.dock_tab = super::dock::DockTab::Messages;
    rig.shell.overview_dirty = true;
    for _ in 0..4 {
        rig.frame(Vec::new());
    }
    for n in 1..=count {
        assert!(
            rig.shows(&format!("Show in view {n}")),
            "no link {n} in {:?}",
            rig.texts()
        );
    }
    assert!(!rig.shows(&format!("Show in view {}", count + 1)));
    // Click the first: the area's number is asked for.
    rig.shell.open_step(Step::Setup);
    rig.shell.dock_tab = super::dock::DockTab::Messages;
    for _ in 0..4 {
        rig.frame(Vec::new());
    }
    let (_, rect) = rig
        .text_rects()
        .into_iter()
        .find(|(t, _)| t == "Show in view 1")
        .expect("the link is shown on Setup too");
    rig.click(rect.center());
    assert_eq!(rig.shell.step, Step::Operations);
    assert_eq!(rig.shell.selected, Some(Selection::Operation(op)));
    // The overview offers the item.
    assert!(
        rig.shell
            .overview
            .messages
            .iter()
            .any(|m| m.source == Some(ItemRef::Operation(op)) && !m.views.is_empty())
    );
}
