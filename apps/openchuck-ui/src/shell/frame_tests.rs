// SPDX-License-Identifier: AGPL-3.0-or-later
//! The shell's frame, drawn whole without a window: the steps and their keys
//! (UI-10), the footer's primary action (specification section 5), the tree
//! marks (UI-11), collapsing and the small-window rule (UI-16), the empty
//! state (11.1) and the settings kept per user (UI-15). Decision record 0117.

#![allow(clippy::unwrap_used)]

use std::path::PathBuf;
use std::time::{Duration, Instant};

use eframe::egui::{self, Event, Key, Modifiers};
use openchuck_app::overview::{ItemRef, ItemState, Step};
use openchuck_app::settings::SettingsFile;

use super::frame::Region;
use super::*;
use crate::testing::cube_project;

/// A shell drawn frame by frame at a window size.
struct Rig {
    shell: Shell,
    ctx: egui::Context,
    size: egui::Vec2,
    shapes: Vec<egui::epaint::ClippedShape>,
    dirs: Vec<PathBuf>,
}

impl Rig {
    fn new(shell: Shell, dirs: Vec<PathBuf>) -> Self {
        let mut rig = Self {
            shell,
            ctx: egui::Context::default(),
            size: egui::vec2(1440.0, 900.0),
            shapes: Vec::new(),
            dirs,
        };
        rig.frame(Vec::new());
        rig
    }

    /// The cube project open, its scene built and its checks done.
    fn cube(name: &str, settings: Option<SettingsFile>) -> Self {
        let (dir, app, _, _) = cube_project(name);
        app.save().unwrap();
        drop(app);
        let shell = Shell::with_settings(false, Some(dir.clone()), settings);
        let mut rig = Self::new(shell, vec![dir]);
        rig.settle();
        rig
    }

    /// Draws until the scene, the checks and any run are done.
    fn settle(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(120);
        loop {
            self.frame(Vec::new());
            let s = &self.shell;
            if s.scene_job.is_none()
                && s.checks_job.is_none()
                && !s.run.is_running()
                && s.overview.checks_current
            {
                break;
            }
            assert!(Instant::now() < deadline, "timed out");
            std::thread::sleep(Duration::from_millis(5));
        }
        // An area is laid out in its first frame and shown in the next.
        self.frame(Vec::new());
        self.frame(Vec::new());
    }

    fn frame(&mut self, events: Vec<Event>) {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), self.size)),
            events,
            ..Default::default()
        };
        let Self { shell, ctx, .. } = self;
        let mut output = ctx.run_ui(input, |ui| shell.draw(ui));
        output.textures_delta.clear();
        self.shapes = output.shapes;
    }

    fn press(&mut self, key: Key, modifiers: Modifiers) {
        let event = |pressed| Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers,
        };
        self.frame(vec![event(true), event(false)]);
    }

    /// Whether some text on screen contains `needle`.
    fn shows(&self, needle: &str) -> bool {
        fn walk(shape: &egui::Shape, needle: &str) -> bool {
            match shape {
                egui::Shape::Text(t) => t.galley.text().contains(needle),
                egui::Shape::Vec(shapes) => shapes.iter().any(|s| walk(s, needle)),
                _ => false,
            }
        }
        self.shapes.iter().any(|s| walk(&s.shape, needle))
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        for dir in &self.dirs {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("openchuck-ui-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn req_ui_10_the_empty_state_offers_a_project_and_dims_the_other_steps() {
    let mut rig = Rig::new(Shell::with_settings(false, None, None), Vec::new());
    rig.frame(Vec::new());
    assert!(rig.shows("Start with a model"));
    assert!(rig.shows("Open project…"));
    assert!(rig.shows("New project…"));
    for group in ["MODEL", "SETUPS", "SELECTION SETS", "TOOLS", "OPERATIONS"] {
        assert!(rig.shows(group), "{group}");
    }
    assert!(rig.shows("None yet"));
    assert!(rig.shows("Verify G-code before running it"));
    // Alt 3 does not open an unavailable step.
    rig.press(Key::Num3, Modifiers::ALT);
    assert_eq!(rig.shell.step, Step::Model);
}

#[test]
fn req_ui_10_req_ui_11_the_steps_marks_and_primary_follow_the_project() {
    let mut rig = Rig::cube("frame-steps", None);
    let op = rig.shell.app.as_ref().unwrap().project().operations()[0].id;
    // A project with an operation opens on Operations; Simulate waits.
    assert_eq!(rig.shell.step, Step::Operations);
    assert!(!rig.shell.overview.step(Step::Simulate).is_available());
    assert_eq!(
        rig.shell
            .overview
            .status(ItemRef::Operation(op))
            .unwrap()
            .state,
        ItemState::Stale
    );
    assert!(rig.shows("Generate stale (1)"));
    assert!(rig.shows("1 Finish"));
    assert!(rig.shows("Stale"), "the legend is drawn");
    // Alt 4 is refused until an operation is generated.
    rig.press(Key::Num4, Modifiers::ALT);
    assert_eq!(rig.shell.step, Step::Operations);
    // The primary action generates; then the steps open.
    rig.shell.primary();
    rig.settle();
    let deadline = Instant::now() + Duration::from_secs(60);
    while !rig.shell.overview.stale.is_empty() {
        assert!(Instant::now() < deadline, "timed out");
        rig.settle();
    }
    assert_eq!(
        rig.shell
            .overview
            .status(ItemRef::Operation(op))
            .unwrap()
            .state,
        ItemState::UpToDate
    );
    assert!(rig.shows("Continue to simulate"));
    rig.press(Key::Num4, Modifiers::ALT);
    assert_eq!(rig.shell.step, Step::Simulate);
    assert!(rig.shows("Run simulation"));
    rig.press(Key::Num1, Modifiers::ALT);
    assert_eq!(rig.shell.step, Step::Model);
    assert!(rig.shows("Continue to setup"));
    // The machine's unconfirmed values are in the footer, in words.
    assert!(
        rig.shell
            .overview
            .machine_warnings
            .iter()
            .all(|w| w.ends_with("unconfirmed"))
    );
}

#[test]
fn req_ui_16_panels_collapse_with_ctrl_1_2_3_and_on_a_small_window() {
    let mut rig = Rig::cube("frame-collapse", None);
    for (key, region) in [Key::Num1, Key::Num2, Key::Num3]
        .into_iter()
        .zip(Region::ALL)
    {
        assert!(!rig.shell.collapsed(region));
        rig.press(key, Modifiers::COMMAND);
        assert!(rig.shell.collapsed(region), "{region:?}");
        rig.press(key, Modifiers::COMMAND);
        assert!(!rig.shell.collapsed(region), "{region:?}");
    }
    // Below 1366 × 768 the dock collapses first, then the inspector, and
    // neither choice is kept.
    rig.size = egui::vec2(1300.0, 700.0);
    rig.frame(Vec::new());
    assert!(rig.shell.collapsed(Region::Dock));
    assert!(!rig.shell.collapsed(Region::Inspector));
    rig.size = egui::vec2(900.0, 700.0);
    rig.frame(Vec::new());
    assert!(rig.shell.collapsed(Region::Inspector));
    assert!(!rig.shell.settings.dock.collapsed);
    assert!(!rig.shell.settings.inspector.collapsed);
    // Ctrl 3 opens the dock the rule closed.
    rig.press(Key::Num3, Modifiers::COMMAND);
    assert!(!rig.shell.collapsed(Region::Dock));
    // At 1366 × 768 itself every region stays open (UI-16).
    rig.size = super::frame::SMALLEST;
    rig.frame(Vec::new());
    for region in Region::ALL {
        assert!(!rig.shell.collapsed(region), "{region:?}");
    }
}

#[test]
fn req_ui_15_the_layout_and_scale_survive_a_restart() {
    let config = temp("frame-settings");
    let file = SettingsFile::in_folder(&config);
    let mut rig = Rig::cube("frame-settings", Some(file.clone()));
    rig.dirs.push(config.clone());
    rig.press(Key::Num2, Modifiers::COMMAND);
    rig.ctx.set_zoom_factor(1.0);
    rig.frame(Vec::new());
    // Written a moment after the change, or on quitting.
    rig.shell.save_settings();
    let kept = file.load();
    assert!(kept.problem.is_none());
    assert!(kept.settings.inspector.collapsed);
    assert!((kept.settings.scale - 1.0).abs() < 1e-6);
    let project = rig.shell.app.as_ref().unwrap().location().to_path_buf();
    let project = std::fs::canonicalize(&project).unwrap_or(project);
    assert_eq!(kept.settings.recent.first(), Some(&project));
    // A restart starts from them.
    let shell = Shell::with_settings(false, None, Some(file));
    assert!(shell.settings.inspector.collapsed);
    assert!((shell.scale() - 1.0).abs() < 1e-6);
    // The scale stays within 80% to 200%.
    rig.ctx.set_zoom_factor(5.0);
    rig.frame(Vec::new());
    rig.frame(Vec::new());
    assert!((rig.ctx.zoom_factor() - 2.0).abs() < 1e-6);
}

#[test]
fn req_ui_14_rename_duplicate_and_move_by_key() {
    let mut rig = Rig::cube("frame-keys", None);
    let op = rig.shell.app.as_ref().unwrap().project().operations()[0].id;
    rig.shell.select(Selection::Operation(op));
    rig.frame(Vec::new());
    rig.press(Key::D, Modifiers::COMMAND);
    let ops = || {
        rig.shell
            .app
            .as_ref()
            .unwrap()
            .project()
            .operations()
            .iter()
            .map(|o| o.id)
            .collect::<Vec<_>>()
    };
    let after = ops();
    assert_eq!(after.len(), 2);
    assert_eq!(after[0], op);
    // Alt ↓ moves it later, one undoable command.
    rig.press(Key::ArrowDown, Modifiers::ALT);
    assert_eq!(
        rig.shell.app.as_ref().unwrap().project().operations()[1].id,
        op
    );
    // F2 renames it.
    rig.press(Key::F2, Modifiers::NONE);
    assert!(matches!(rig.shell.dialog, Some(Dialog::Rename { .. })));
    // Its Rename button: the name is applied and the field gives up the
    // keyboard.
    rig.shell.dialog = Some(Dialog::Rename {
        item: ItemRef::Operation(op),
        name: "Roughing".into(),
    });
    rig.press(Key::Enter, Modifiers::NONE);
    rig.frame(Vec::new());
    assert_eq!(
        rig.shell
            .app
            .as_ref()
            .unwrap()
            .project()
            .operation(op)
            .unwrap()
            .name,
        "Roughing"
    );
    assert!(rig.shell.dialog.is_none());
    // Del deletes it; Ctrl Z brings it back.
    rig.press(Key::Delete, Modifiers::NONE);
    assert!(
        rig.shell
            .app
            .as_ref()
            .unwrap()
            .project()
            .operation(op)
            .is_none()
    );
    rig.press(Key::Z, Modifiers::COMMAND);
    assert!(
        rig.shell
            .app
            .as_ref()
            .unwrap()
            .project()
            .operation(op)
            .is_some()
    );
}

#[test]
fn req_ui_12_a_message_action_selects_its_source() {
    let mut rig = Rig::cube("frame-messages", None);
    // An operation with no tool fails to generate: the message names it and
    // its link selects it.
    let app = rig.shell.app.as_mut().unwrap();
    let mut operation = app.project().operations()[0].clone();
    let op = operation.id;
    operation.tool = None;
    app.execute(Command::Replace(openchuck_app::project::Item::Operation(
        operation,
    )))
    .unwrap();
    rig.shell.generate(false);
    rig.settle();
    let message = rig
        .shell
        .overview
        .messages
        .iter()
        .find(|m| m.source == Some(ItemRef::Operation(op)))
        .cloned()
        .unwrap();
    rig.shell.dock_tab = super::dock::DockTab::Messages;
    rig.frame(Vec::new());
    assert!(rig.shows(&message.text));
    rig.shell.message_action(message.action.unwrap());
    assert_eq!(rig.shell.selected, Some(Selection::Operation(op)));
    // The error disables the primary action and names it.
    let primary = rig.shell.overview.primary(Step::Operations);
    assert!(primary.blocked.is_some());
    rig.shell.primary_blocked_click();
    assert_eq!(rig.shell.highlight, primary.message);
}

impl Shell {
    /// Presses the primary button while it is disabled, as a click on it
    /// does.
    fn primary_blocked_click(&mut self) {
        self.step = Step::Operations;
        self.primary();
    }
}

#[test]
fn req_ui_14_g_generates_the_selected_operation_and_shift_g_every_stale_one() {
    let mut rig = Rig::cube("frame-generate", None);
    let first = rig.shell.app.as_ref().unwrap().project().operations()[0].id;
    rig.shell.select(Selection::Operation(first));
    rig.frame(Vec::new());
    rig.press(Key::D, Modifiers::COMMAND);
    rig.shell.select(Selection::Operation(first));
    rig.frame(Vec::new());
    rig.settle();
    assert_eq!(rig.shell.overview.stale.len(), 2);
    // G: the first operation only; the copy after it stays stale.
    rig.press(Key::G, Modifiers::NONE);
    assert!(rig.shell.run.is_running());
    rig.settle();
    let deadline = Instant::now() + Duration::from_secs(60);
    while rig.shell.overview.stale.len() != 1 {
        assert!(Instant::now() < deadline, "timed out");
        rig.settle();
    }
    assert_eq!(
        rig.shell
            .overview
            .status(ItemRef::Operation(first))
            .unwrap()
            .state,
        ItemState::UpToDate
    );
    assert!(rig.shell.jobs.finished().any(|e| matches!(
        &e.outcome,
        Some(openchuck_app::JobOutcome::Done(Some(s))) if s.contains("later operations stay stale")
    )));
    // Shift G: every stale operation, with the results window open after the
    // last run.
    assert!(rig.shell.run.has_window());
    rig.press(Key::G, Modifiers::SHIFT);
    rig.settle();
    let deadline = Instant::now() + Duration::from_secs(60);
    while !rig.shell.overview.stale.is_empty() {
        assert!(Instant::now() < deadline, "timed out");
        rig.settle();
    }
}

#[test]
fn req_ui_14_g_on_a_turned_off_operation_is_refused_not_a_full_run() {
    let mut rig = Rig::cube("frame-generate-off", None);
    let first = rig.shell.app.as_ref().unwrap().project().operations()[0].id;
    rig.shell
        .app
        .as_mut()
        .unwrap()
        .execute(openchuck_app::Command::SetOperationEnabled {
            operation: first,
            enabled: false,
        })
        .unwrap();
    rig.shell.select(Selection::Operation(first));
    rig.frame(Vec::new());
    rig.press(Key::G, Modifiers::NONE);
    let deadline = Instant::now() + Duration::from_secs(60);
    while rig.shell.run.is_running() {
        assert!(Instant::now() < deadline, "timed out");
        rig.settle();
    }
    rig.settle();
    let error = rig.shell.run.last_error().unwrap_or_default();
    assert!(error.contains("turned off"), "{error:?}");
    assert!(rig.shell.run.last().is_none());
}

#[test]
fn req_ui_03_the_setup_step_shows_the_front_and_end_panes_with_their_legend() {
    let mut rig = Rig::cube("frame-panes", None);
    // Operations draws the 3D view.
    assert_eq!(rig.shell.step, Step::Operations);
    assert!(!rig.shows("END VIEW FROM +X"));
    rig.press(Key::Num2, Modifiers::ALT);
    rig.settle();
    assert_eq!(rig.shell.step, Step::Setup);
    for text in [
        "END VIEW FROM +X",
        "A+ turns the top toward the front",
        "Stock length 14.000",
        "A axis",
        "Clearance R",
        "Rotary axis",
        "Clearance and safe Z",
        "Travel limit",
        "Y into the screen",
    ] {
        assert!(rig.shows(text), "{text}");
    }
    // No chuck is drawn (OI-380).
    assert!(!rig.shows("Chuck"));
    // Back on Operations the panes and their legend are gone.
    rig.press(Key::Num3, Modifiers::ALT);
    rig.settle();
    assert!(!rig.shows("END VIEW FROM +X"));
    assert!(!rig.shows("Travel limit"));
}

/// A project with a model and no setup, which is what File, New project and
/// an import give (decision record 0144).
fn no_setup_rig(name: &str, with_model: bool) -> Rig {
    let dir = temp(name);
    let mut app =
        openchuck_app::Application::create(&dir, name, openchuck_app::Threads::Exactly(2)).unwrap();
    if with_model {
        std::fs::create_dir_all(dir.join("models")).unwrap();
        let cube = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../testdata/stl/cube_ascii.stl");
        std::fs::copy(cube, dir.join("models/cube.stl")).unwrap();
        app.add_model(&dir.join("models/cube.stl"), None).unwrap();
    }
    app.save().unwrap();
    drop(app);
    let shell = Shell::with_settings(false, Some(dir.clone()), None);
    let mut rig = Rig::new(shell, vec![dir]);
    rig.settle();
    rig
}

#[test]
fn req_set_01_req_ui_10_the_setups_plus_creates_the_one_setup_and_opens_the_setup_step() {
    let mut rig = no_setup_rig("add-setup", true);
    assert!(
        rig.shell
            .app
            .as_ref()
            .unwrap()
            .project()
            .setups()
            .is_empty()
    );
    assert!(rig.shows("SETUPS"));
    rig.shell.tree_action(TreeAction::Add(tree::Group::Setups));
    rig.settle();
    let project = rig.shell.app.as_ref().unwrap().project();
    assert_eq!(project.setups().len(), 1);
    let setup = &project.setups()[0];
    assert_eq!(setup.name, "Setup 1");
    assert_eq!(setup.part, Some(project.models()[0].id));
    assert_eq!(rig.shell.step, Step::Setup);
    assert_eq!(rig.shell.selected, Some(Selection::Setup(setup.id)));
    assert!(rig.shows("Setup 1"));
}

#[test]
fn req_set_01_a_second_click_does_nothing_and_undo_allows_the_plus_again() {
    let mut rig = no_setup_rig("add-setup-twice", true);
    rig.shell.tree_action(TreeAction::Add(tree::Group::Setups));
    rig.settle();
    rig.shell.tree_action(TreeAction::Add(tree::Group::Setups));
    rig.shell.add_setup();
    rig.settle();
    let project = rig.shell.app.as_ref().unwrap().project();
    assert_eq!(project.setups().len(), 1);
    assert_eq!(
        openchuck_app::add_setup_blocked(project),
        Some("More than one setup per project is not supported yet (SET-07)")
    );
    // The workflow bar's action is disabled too, with the same words.
    assert!(rig.shows("Add setup"));
    // Undo removes the setup, so the "+" is enabled again.
    let app = rig.shell.app.as_mut().unwrap();
    assert!(app.undo().unwrap());
    assert!(openchuck_app::can_add_setup(app.project()));
    rig.shell.add_setup();
    assert_eq!(rig.shell.app.as_ref().unwrap().project().setups().len(), 1);
}

#[test]
fn req_set_01_a_project_that_has_a_setup_shows_the_plus_disabled() {
    // The cube project opens with its setup.
    let mut rig = Rig::cube("add-setup-existing", None);
    assert!(!openchuck_app::can_add_setup(
        rig.shell.app.as_ref().unwrap().project()
    ));
    rig.shell.add_setup();
    rig.settle();
    assert_eq!(rig.shell.app.as_ref().unwrap().project().setups().len(), 1);
    rig.press(Key::Num2, Modifiers::ALT);
    rig.settle();
    assert!(rig.shows("Add setup"));
}

#[test]
fn req_set_01_the_plus_is_disabled_until_a_model_is_imported() {
    let mut rig = no_setup_rig("add-setup-no-model", false);
    assert_eq!(
        openchuck_app::add_setup_blocked(rig.shell.app.as_ref().unwrap().project()),
        Some("Import a model first")
    );
    rig.shell.tree_action(TreeAction::Add(tree::Group::Setups));
    rig.shell.add_setup();
    rig.settle();
    assert!(
        rig.shell
            .app
            .as_ref()
            .unwrap()
            .project()
            .setups()
            .is_empty()
    );
    assert_eq!(rig.shell.step, Step::Model);
}
