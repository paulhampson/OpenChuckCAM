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
use crate::theme::width;

/// A shell drawn frame by frame at a window size.
pub(super) struct Rig {
    pub(super) shell: Shell,
    pub(super) ctx: egui::Context,
    pub(super) size: egui::Vec2,
    pub(super) shapes: Vec<egui::epaint::ClippedShape>,
    pub(super) dirs: Vec<PathBuf>,
    /// The scope id of the root `Ui` the shell draws into: egui 0.37 derives a
    /// panel's id from its parent's scope id and its salt.
    root: egui::Id,
}

impl Rig {
    pub(super) fn new(shell: Shell, dirs: Vec<PathBuf>) -> Self {
        let mut rig = Self {
            shell,
            ctx: egui::Context::default(),
            size: egui::vec2(1440.0, 900.0),
            shapes: Vec::new(),
            dirs,
            root: egui::Id::NULL,
        };
        rig.frame(Vec::new());
        rig
    }

    /// The cube project open, its scene built and its checks done.
    pub(super) fn cube(name: &str, settings: Option<SettingsFile>) -> Self {
        let (dir, app, _, _) = cube_project(name);
        app.save().unwrap();
        drop(app);
        let shell = Shell::with_settings(false, Some(dir.clone()), settings);
        let mut rig = Self::new(shell, vec![dir]);
        rig.settle();
        rig
    }

    /// Draws until the scene, the checks and any run are done, and the run
    /// is prepared for playback. A run that lands is handed to the playback
    /// panel in the same frame, which then prepares it in a job; until that
    /// job is done the dock draws only the transport row, so a test that
    /// read the rows under it straight after a run raced the job (OI-840).
    pub(super) fn settle(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(120);
        loop {
            self.frame(Vec::new());
            let s = &self.shell;
            if s.scene_job.is_none()
                && s.checks_job.is_none()
                && !s.run.is_running()
                && !s.play.is_preparing()
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

    pub(super) fn frame(&mut self, events: Vec<Event>) {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), self.size)),
            events,
            ..Default::default()
        };
        let Self {
            shell, ctx, root, ..
        } = self;
        let mut output = ctx.run_ui(input, |ui| {
            *root = ui.scope_id();
            shell.draw(ui);
        });
        output.textures_delta.clear();
        self.shapes = output.shapes;
    }

    /// The state egui kept for the panel salted `salt`: the side panels are shown in
    /// the root `Ui`. The dock is shown in the central panel's `Ui`, whose id the shell records.
    pub(super) fn panel_state(&self, salt: &str) -> Option<egui::PanelState> {
        let parent = if salt == "dock" {
            self.ctx
                .data(|d| d.get_temp(egui::Id::unique("test dock parent")))?
        } else {
            self.root
        };
        egui::PanelState::load(&self.ctx, parent.with(salt))
    }

    /// What the last frame painted.
    pub(super) fn shapes(&self) -> &[egui::epaint::ClippedShape] {
        &self.shapes
    }

    pub(super) fn press(&mut self, key: Key, modifiers: Modifiers) {
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
    /// Every text painted, with its top edge: what a failed `shows` was up
    /// against.
    pub(super) fn texts(&self) -> Vec<(String, f32)> {
        fn walk(shape: &egui::Shape, out: &mut Vec<(String, f32)>) {
            match shape {
                egui::Shape::Text(t) => out.push((t.galley.text().to_string(), t.pos.y)),
                egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| walk(s, out)),
                _ => {}
            }
        }
        let mut out = Vec::new();
        self.shapes.iter().for_each(|s| walk(&s.shape, &mut out));
        out
    }

    /// The rectangles of every entry box painted (the field fill), left to
    /// right as drawn.
    pub(super) fn entry_rects(&self) -> Vec<egui::Rect> {
        fn walk(shape: &egui::Shape, out: &mut Vec<egui::Rect>) {
            match shape {
                egui::Shape::Rect(r) if r.fill == crate::theme::DARK.field => out.push(r.rect),
                egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| walk(s, out)),
                _ => {}
            }
        }
        let mut out = Vec::new();
        self.shapes.iter().for_each(|s| walk(&s.shape, &mut out));
        out
    }

    /// The rectangles of every narrow, tall shape painted (a scroll bar's
    /// handle or track).
    pub(super) fn bar_rects(&self) -> Vec<egui::Rect> {
        fn walk(shape: &egui::Shape, out: &mut Vec<egui::Rect>) {
            match shape {
                egui::Shape::Rect(r) if r.rect.width() < 12.0 && r.rect.height() > 40.0 => {
                    out.push(r.rect)
                }
                egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| walk(s, out)),
                _ => {}
            }
        }
        let mut out = Vec::new();
        self.shapes.iter().for_each(|s| walk(&s.shape, &mut out));
        out
    }

    /// Every text painted with the rectangle it covers.
    pub(super) fn text_rects(&self) -> Vec<(String, egui::Rect)> {
        fn walk(shape: &egui::Shape, out: &mut Vec<(String, egui::Rect)>) {
            match shape {
                egui::Shape::Text(t) => out.push((
                    t.galley.text().to_string(),
                    egui::Rect::from_min_size(t.pos, t.galley.size()),
                )),
                egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| walk(s, out)),
                _ => {}
            }
        }
        let mut out = Vec::new();
        self.shapes.iter().for_each(|s| walk(&s.shape, &mut out));
        out
    }

    /// Every text and filled rectangle painted: the text (none for a
    /// rectangle), its whole rectangle and as much of it as its clip lets
    /// show. A text's rectangle covers its glyphs, not the indent a wrapped
    /// row starts with. What a scroll area clips away entirely is left out.
    pub(super) fn painted(&self) -> Vec<(Option<String>, egui::Rect, egui::Rect)> {
        type Painted = Vec<(Option<String>, egui::Rect, egui::Rect)>;
        fn walk(shape: &egui::Shape, clip: egui::Rect, out: &mut Painted) {
            let (text, rect) = match shape {
                egui::Shape::Text(t) => (
                    Some(t.galley.text().to_string()),
                    t.galley
                        .rows
                        .iter()
                        .map(|r| r.rect_without_leading_space().translate(t.pos.to_vec2()))
                        .reduce(|a, b| a.union(b))
                        .unwrap_or(egui::Rect::NOTHING),
                ),
                egui::Shape::Rect(r) => (None, r.rect),
                egui::Shape::Vec(shapes) => {
                    shapes.iter().for_each(|s| walk(s, clip, out));
                    return;
                }
                _ => return,
            };
            let seen = rect.intersect(clip);
            if seen.is_positive() {
                out.push((text, rect, seen));
            }
        }
        let mut out = Vec::new();
        self.shapes
            .iter()
            .for_each(|s| walk(&s.shape, s.clip_rect, &mut out));
        out
    }

    /// Every text painted with the part of its rectangle that is not clipped away, in paint
    /// order; texts wholly clipped away are left out.
    pub(super) fn visible_text_rects(&self) -> Vec<(String, egui::Rect)> {
        fn walk(shape: &egui::Shape, clip: egui::Rect, out: &mut Vec<(String, egui::Rect)>) {
            match shape {
                egui::Shape::Text(t) => {
                    let rect = egui::Rect::from_min_size(t.pos, t.galley.size()).intersect(clip);
                    if rect.is_positive() {
                        out.push((t.galley.text().to_string(), rect));
                    }
                }
                egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| walk(s, clip, out)),
                _ => {}
            }
        }
        let mut out = Vec::new();
        self.shapes
            .iter()
            .for_each(|s| walk(&s.shape, s.clip_rect, &mut out));
        out
    }

    pub(super) fn shows(&self, needle: &str) -> bool {
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

pub(super) fn temp(name: &str) -> PathBuf {
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
    for group in ["MODEL", "SETUPS", "SELECTION SETS", "OPERATIONS"] {
        assert!(rig.shows(group), "{group}");
    }
    assert!(rig.shows("None yet"));
    assert!(rig.shows("Verify G-code before running it"));
    // Alt 3 does not open an unavailable step.
    rig.press(Key::Num3, Modifiers::ALT);
    assert_eq!(rig.shell.step, Step::Model);
}

#[test]
fn req_ui_10_req_ui_14_post_and_export_opens_once_the_project_has_what_it_needs() {
    // Decision 0148 item 1 (issue 140): not before the project has a model,
    // a setup, a machine, a tool and an operation, and then without the
    // operation having been generated.
    let mut rig = Rig::new(Shell::with_settings(false, None, None), Vec::new());
    rig.press(Key::E, Modifiers::COMMAND);
    assert!(!rig.shell.export.is_open());
    assert_eq!(
        rig.shell.post_blocker(),
        Some("Open or start a project first")
    );

    let mut rig = Rig::cube("frame-post", None);
    assert!(rig.shell.overview.export_action().blocked.is_none());
    assert_eq!(rig.shell.post_blocker(), None);
    let id = rig.shell.app.as_ref().unwrap().project().operations()[0].id;
    rig.shell.tree_action(super::TreeAction::Delete(id));
    rig.settle();
    assert_eq!(rig.shell.post_blocker(), Some("Add an operation first"));
    rig.press(Key::E, Modifiers::COMMAND);
    assert!(!rig.shell.export.is_open());
    rig.shell.undo();
    rig.settle();
    assert_eq!(rig.shell.post_blocker(), None);
    rig.press(Key::E, Modifiers::COMMAND);
    assert!(rig.shell.export.is_open());
}

#[test]
fn req_nfr_15_req_ui_16_the_scale_menu_offers_fixed_steps_within_the_range() {
    use openchuck_app::settings::SCALE_RANGE;
    assert_eq!(super::frame::UI_SCALES, [80, 90, 100, 110, 120]);
    for percent in super::frame::UI_SCALES {
        let scale = percent as f32 / 100.0;
        assert!((SCALE_RANGE.0..=SCALE_RANGE.1).contains(&scale));
    }
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
    // Generating simulated the stock too (issue 214), so Post and export is
    // the next step on Simulate (decision record 0282 item 2).
    assert!(rig.shows("Post and export…"));
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
fn req_ui_15_a_dock_splitter_reset_is_kept_for_the_next_start() {
    // CodeRabbit on pull request 173: the reset wrote the default but never marked the
    // settings changed, so it was lost on a restart.
    let mut rig = Rig::cube("frame-dock-reset", None);
    rig.shell.settings.dock.size = 300.0;
    rig.frame(Vec::new());
    rig.shell.settings_changed = None;
    rig.shell.reset = Some(Region::Dock);
    rig.frame(Vec::new());
    assert!((rig.shell.settings.dock.size - 260.0).abs() < 0.01);
    assert!(rig.shell.settings_changed.is_some());
}

#[test]
fn req_ui_15_the_layout_and_scale_survive_a_restart() {
    let config = temp("frame-settings");
    let file = SettingsFile::in_folder(&config);
    let mut rig = Rig::cube("frame-settings", Some(file.clone()));
    rig.dirs.push(config.clone());
    rig.press(Key::Num2, Modifiers::COMMAND);
    rig.ctx.set_zoom_factor(0.9);
    rig.frame(Vec::new());
    // Written a moment after the change, or on quitting.
    rig.shell.save_settings();
    let kept = file.load();
    assert!(kept.problem.is_none());
    assert!(kept.settings.inspector.collapsed);
    assert!((kept.settings.scale - 0.9).abs() < 1e-6);
    let project = rig.shell.app.as_ref().unwrap().location().to_path_buf();
    let project = std::fs::canonicalize(&project).unwrap_or(project);
    assert_eq!(kept.settings.recent.first(), Some(&project));
    // A restart starts from them.
    let shell = Shell::with_settings(false, None, Some(file));
    assert!(shell.settings.inspector.collapsed);
    assert!((shell.scale() - 0.9).abs() < 1e-6);
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
    assert!(rig.shell.run.last().is_none());
    rig.press(Key::G, Modifiers::NONE);
    // G started a run: still going, or on a fast machine already finished
    // within the frame that pressed G.
    assert!(rig.shell.run.is_running() || rig.shell.run.last().is_some());
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
    // Shift G: every stale operation, after the last run (the results show
    // in the inspector; the floating window opens only on a failure).
    assert!(rig.shell.run.last().is_some() && !rig.shell.run.has_window());
    rig.press(Key::G, Modifiers::SHIFT);
    rig.settle();
    let deadline = Instant::now() + Duration::from_secs(60);
    while !rig.shell.overview.stale.is_empty() {
        assert!(Instant::now() < deadline, "timed out");
        rig.settle();
    }
}

/// Choosing a setup shows its five sections (11.3) in field rows: the
/// machine's values with their provenance (UI-13), the stock, the part
/// position, and the work offset and clearance with the zero read-only
/// (decision record 0114 C10, C15).
#[test]
fn req_ui_07_req_ui_13_the_setup_inspector_shows_its_five_sections() {
    let mut rig = Rig::cube("inspector-setup", None);
    // Tall enough that every section is on screen.
    rig.size = egui::vec2(1440.0, 2400.0);
    let setup = rig.shell.app.as_ref().unwrap().project().setups()[0].id;
    rig.shell.choose_setup(setup, None);
    rig.frame(Vec::new());
    rig.frame(Vec::new());
    for needle in [
        "MACHINE",
        "STOCK",
        "PART POSITION",
        "WORK OFFSET AND CLEARANCE",
        "WORKHOLDING AND TABS",
        "Rotary axis",
        "Axis height",
        "Cylinder",
        "Diameter",
        "Offset along X",
        "On the A axis (Y0 Z0)",
        "Safe radius",
    ] {
        assert!(rig.shows(needle), "{needle} is not shown");
    }
    // No file field names in the machine section (section 15).
    assert!(!rig.shows("rotary_origin"));
}

/// An operation whose selection set cannot be used shows the error banner of
/// board 3b with "Reselect in the view", the broken set with a red border in
/// its Geometry role, and the closing line about export (11.4; C16).
#[test]
fn req_ui_11_an_operation_in_error_shows_the_banner_and_its_actions() {
    use openchuck_app::project::{Item, SelectionTarget};
    let mut rig = Rig::cube("inspector-error", None);
    let app = rig.shell.app.as_mut().unwrap();
    let op = app.project().operations()[0].clone();
    let model = app.project().models()[0].id;
    let set = app
        .save_selection("Lettering", model, SelectionTarget::Triangles([0].into()))
        .unwrap();
    app.set_operation_selection(
        op.id,
        openchuck_app::selection::SelectionRole::Machine,
        set,
        true,
    )
    .unwrap();
    // The set no longer selects anything: a problem up the tree.
    let mut broken = app
        .project()
        .selection_sets()
        .iter()
        .find(|s| s.id == set)
        .unwrap()
        .clone();
    broken.target = SelectionTarget::Triangles(Default::default());
    app.execute(Command::Replace(Item::SelectionSet(broken)))
        .unwrap();
    rig.settle();
    rig.shell.select(Selection::Operation(op.id));
    rig.frame(Vec::new());
    rig.frame(Vec::new());
    assert!(rig.shows("This operation can't be generated"));
    assert!(rig.shows("Reselect in the view"));
    assert!(rig.shows("Show missing"));
    assert!(rig.shows("Export is blocked while this operation is in error"));
    assert!(rig.shows("GEOMETRY"));
    assert!(rig.shows("Cut on"));
    assert!(rig.shows("Lettering"));
}

/// A swarf operation's Geometry section shows its sets' options beside them
/// (decision record 0176 item 3): the boundary's two limits and each avoid
/// or check set's choice, and a set whose options leave it limiting nothing
/// says so. Another strategy that takes a region shows each avoid or check
/// set's choice and no boundary options (decision record 0229).
#[test]
fn req_op_02_req_ui_07_the_geometry_section_shows_each_sets_options() {
    use openchuck_app::project::{BoundaryOptions, Item, OnBreak, SelectionTarget};
    use openchuck_app::selection::SelectionRole;
    let mut rig = Rig::cube("inspector-set-options", None);
    rig.size = egui::vec2(1440.0, 2400.0);
    let app = rig.shell.app.as_mut().unwrap();
    let op = app.project().operations()[0].clone();
    let model = app.project().models()[0].id;
    let fence = app
        .save_selection("Fence", model, SelectionTarget::Triangles([0].into()))
        .unwrap();
    let rib = app
        .save_selection("Rib", model, SelectionTarget::Triangles([1].into()))
        .unwrap();
    app.set_operation_selection(op.id, SelectionRole::Boundary, fence, true)
        .unwrap();
    app.set_operation_selection(op.id, SelectionRole::Avoid, rib, true)
        .unwrap();
    // Helical finishing: the avoid set's choice, no boundary options; the
    // rib on Ignore says it limits nothing.
    app.set_on_break(op.id, SelectionRole::Avoid, rib, OnBreak::Ignore)
        .unwrap();
    rig.settle();
    rig.shell.select(Selection::Operation(op.id));
    rig.frame(Vec::new());
    rig.frame(Vec::new());
    assert!(rig.shows("Contain within"));
    assert!(!rig.shows("Limits the pass"));
    assert!(!rig.shows("Limits the tip"));
    for needle in [
        "Cut short",
        "Warn only",
        "Ignore",
        "The set 'Rib' limits nothing.",
    ] {
        assert!(
            rig.shows(needle),
            "{needle} is not shown for helical finishing"
        );
    }
    // As swarf: the options, with the boundary's limits both off and the
    // rib ignored, each saying it limits nothing.
    let app = rig.shell.app.as_mut().unwrap();
    let mut swarf = app.project().operation(op.id).unwrap().clone();
    swarf.strategy = "swarf".into();
    app.execute(Command::Replace(Item::Operation(swarf)))
        .unwrap();
    app.set_boundary_options(
        op.id,
        BoundaryOptions {
            limits_pass: false,
            limits_tip: false,
        },
    )
    .unwrap();
    rig.settle();
    rig.frame(Vec::new());
    rig.frame(Vec::new());
    for needle in [
        "Limits the pass",
        "Limits the tip",
        "Cut short",
        "Warn only",
        "Ignore",
        "The set 'Fence' limits nothing.",
        "The set 'Rib' limits nothing.",
    ] {
        assert!(rig.shows(needle), "{needle} is not shown");
    }
}

#[test]
fn req_op_02_req_nfr_12_warn_only_explains_sampling_before_generation() {
    // OI-970, owner choice relayed 23 Oct 2026: an absent hit warning must
    // not suggest clearance. Explain the limitation as soon as Warn only is
    // selected, without relying on generation or a detected intersection.
    use openchuck_app::project::{OnBreak, SelectionTarget};
    use openchuck_app::selection::SelectionRole;
    let mut rig = Rig::cube("inspector-watch-sampling", None);
    rig.size = egui::vec2(1440.0, 2400.0);
    let app = rig.shell.app.as_mut().unwrap();
    let op = app.project().operations()[0].id;
    let model = app.project().models()[0].id;
    let set = app
        .save_selection("Narrow set", model, SelectionTarget::Triangles([1].into()))
        .unwrap();
    rig.shell.select(Selection::Operation(op));
    let limitation = "Warn only samples the path and can miss a narrow set. No warning does not \
                      establish clearance; verify the cut before running G-code.";
    for role in [SelectionRole::Avoid, SelectionRole::Check] {
        let app = rig.shell.app.as_mut().unwrap();
        app.set_operation_selection(op, role, set, true).unwrap();
        for choice in [OnBreak::Warn, OnBreak::CutShort, OnBreak::Ignore] {
            rig.shell
                .app
                .as_mut()
                .unwrap()
                .set_on_break(op, role, set, choice)
                .unwrap();
            rig.settle();
            rig.frame(Vec::new());
            rig.frame(Vec::new());
            assert_eq!(
                rig.shows(limitation),
                choice == OnBreak::Warn,
                "{role:?} {choice:?}"
            );
        }
        rig.shell
            .app
            .as_mut()
            .unwrap()
            .set_operation_selection(op, role, set, false)
            .unwrap();
    }
}

/// On a STEP part, a set the strategy cannot use shows a Warning line under
/// the role that holds it, with the reason and what to do, before anything
/// is generated, and the operation is in error (decision record 0209 item
/// 10). The part is named as a STEP file; nothing reads it.
#[test]
fn req_op_23_req_ui_07_the_geometry_section_warns_of_a_set_the_strategy_cannot_use() {
    use openchuck_app::project::{
        Item, OrderKey, Placement, RelativePath, SelectionSet, SelectionTarget,
    };
    use openchuck_app::selection::SelectionRole;
    let mut rig = Rig::cube("inspector-set-refused", None);
    rig.size = egui::vec2(1440.0, 2400.0);
    let app = rig.shell.app.as_mut().unwrap();
    let mut op = app.project().operations()[0].clone();
    let mut model = app.project().models()[0].clone();
    model.path = RelativePath::new("models/blade.step").unwrap();
    app.execute(Command::Replace(Item::Model(model.clone())))
        .unwrap();
    let set = openchuck_app::project::id::SelectionSetId::new();
    app.execute(Command::Add {
        item: Item::SelectionSet(SelectionSet {
            id: set,
            order: OrderKey::default(),
            name: "Blades".into(),
            model: model.id,
            model_hash: model.hash,
            target: SelectionTarget::Faces([openchuck_app::pick::FaceId(1)].into()),
            descriptors: Vec::new(),
            stock_to_leave: None,
            colour: None,
            keep_tangent_edges: None,
        }),
        placement: Placement::End,
    })
    .unwrap();
    op.strategy = "rest_machining".into();
    app.execute(Command::Replace(Item::Operation(op.clone())))
        .unwrap();
    app.set_operation_selection(op.id, SelectionRole::Avoid, set, true)
        .unwrap();
    rig.settle();
    rig.shell.select(Selection::Operation(op.id));
    rig.frame(Vec::new());
    rig.frame(Vec::new());
    let said = "Rest machining does not use a boundary, avoid or check set. Take the set 'Blades' \
                out of the operation.";
    assert!(rig.shows(said), "the warning is not shown");
    assert!(rig.shows("This operation can't be generated"));
    assert!(!rig.shows("rest_machining"));
}

/// The header's name is an editable field: a name is applied as one
/// undoable command, and an empty one is refused under it (section 8).
#[test]
fn req_ui_06_req_ui_08_the_header_renames_as_one_undoable_command() {
    let mut rig = Rig::cube("inspector-rename", None);
    let op = rig.shell.app.as_ref().unwrap().project().operations()[0].id;
    let item = ItemRef::Operation(op);
    assert_eq!(
        rig.shell.rename_checked(item, "  "),
        Err("A name cannot be empty. Not applied.".to_owned())
    );
    rig.shell.rename_checked(item, "Rough").unwrap();
    let app = rig.shell.app.as_mut().unwrap();
    assert_eq!(app.project().operation(op).unwrap().name, "Rough");
    assert!(app.undo().unwrap());
    assert_ne!(app.project().operation(op).unwrap().name, "Rough");
    rig.shell.select(Selection::Operation(op));
    rig.frame(Vec::new());
    assert!(rig.shows("TOOL AND FEEDS"));
}

/// Choosing a model shows its inspector (11.2; C14): the file, format,
/// triangle count and size once the check has run, the unit with where it
/// came from, the placement steps, the mesh check and the source.
#[test]
fn req_imp_03_req_imp_04_req_imp_05_the_model_inspector_shows_its_sections() {
    let mut rig = Rig::cube("inspector-model", None);
    rig.size = egui::vec2(1440.0, 2400.0);
    let model = rig.shell.app.as_ref().unwrap().project().models()[0].id;
    rig.shell.select(Selection::Model(model));
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        rig.frame(Vec::new());
        if rig.shows("Triangles") {
            break;
        }
        assert!(Instant::now() < deadline, "timed out");
        std::thread::sleep(Duration::from_millis(5));
    }
    rig.frame(Vec::new());
    for needle in [
        "FILE",
        "Format",
        "STL",
        "UNITS",
        "Read in mm",
        "PLACEMENT",
        "1. Centre on the rotary axis",
        "MESH CHECK",
        "The check changes nothing",
        "Open full report",
    ] {
        assert!(rig.shows(needle), "{needle} is not shown");
    }
}

/// G with a turned-off operation selected is refused, not a full run
/// (lane X3, OI-352).
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

/// The workholding section drives the view (7.5, 11.7; OI-334 k, OI-355):
/// an unapplied draft is drawn in place of the stored workholding, whatever
/// is selected; a click on a solid selects its setup, opens the section at
/// its row and outlines it; Esc clears that selection.
#[test]
fn req_ui_25_the_draft_and_the_selected_solid_reach_the_view() {
    use openchuck_app::scene::WorkholdingItem;
    use openchuck_app::setup::{Millimetres, Tab, TabLayout, TabShape};
    let mut rig = Rig::cube("inspector-draft-view", None);
    rig.size = egui::vec2(1440.0, 2400.0);
    let setup = rig.shell.app.as_ref().unwrap().project().setups()[0].id;
    rig.shell.choose_setup(setup, Some(SetupPart::Workholding));
    // The part-extent job places the solids.
    let deadline = Instant::now() + Duration::from_secs(60);
    while rig
        .shell
        .workholding_panel
        .as_ref()
        .and_then(crate::workholding_panel::WorkholdingPanel::part_extent)
        .is_none()
    {
        assert!(Instant::now() < deadline, "timed out");
        rig.frame(Vec::new());
        std::thread::sleep(Duration::from_millis(5));
    }
    // Nothing edited: the defaults offered are drawn, and the chip does not say anything is
    // unapplied (OI-528).
    rig.frame(Vec::new());
    assert!(!rig.shell.viewport.shows_unapplied());
    rig.shell
        .workholding_panel
        .as_mut()
        .unwrap()
        .draft_mut()
        .tabs = vec![Tab {
        auto: None,
        x_start: Millimetres(-7.0),
        length: Millimetres(1.0),
        shape: TabShape::Cylinder {
            diameter: Millimetres(4.0),
        },
        layout: TabLayout::OnAxis,
    }];
    rig.frame(Vec::new());
    assert!(rig.shell.viewport.shows_draft());
    assert!(rig.shell.viewport.shows_unapplied());
    // Still drawn with an operation selected (11.7: the draft is kept).
    let op = rig.shell.app.as_ref().unwrap().project().operations()[0].id;
    rig.shell.select(Selection::Operation(op));
    rig.frame(Vec::new());
    assert!(rig.shell.viewport.shows_draft());
    // A click on the tab's solid.
    rig.shell.select_solid(WorkholdingItem::Tab(0));
    rig.frame(Vec::new());
    rig.frame(Vec::new());
    assert_eq!(rig.shell.selected, Some(Selection::Setup(setup)));
    assert_eq!(
        rig.shell.viewport.selected_solid(),
        Some(WorkholdingItem::Tab(0))
    );
    assert!(rig.shows("Start X"));
    // Esc clears it once no field has the keyboard.
    rig.ctx.memory_mut(|m| {
        if let Some(id) = m.focused() {
            m.surrender_focus(id);
        }
    });
    rig.frame(Vec::new());
    rig.press(Key::Escape, Modifiers::NONE);
    rig.frame(Vec::new());
    assert_eq!(rig.shell.viewport.selected_solid(), None);
}

/// Esc clears a selected held region or tab on the Setup step too, where
/// the panes replace the 3D view (7.5; OI-354).
#[test]
fn req_ui_25_esc_clears_the_selected_solid_while_the_panes_are_shown() {
    use openchuck_app::scene::WorkholdingItem;
    let mut rig = Rig::cube("panes-esc", None);
    rig.press(Key::Num2, Modifiers::ALT);
    rig.settle();
    assert_eq!(rig.shell.step, Step::Setup);
    let setup = rig.shell.app.as_ref().unwrap().project().setups()[0].id;
    rig.shell.choose_setup(setup, Some(SetupPart::Workholding));
    rig.shell
        .viewport
        .set_selected_solid(Some(WorkholdingItem::Start));
    rig.ctx.memory_mut(|m| {
        if let Some(id) = m.focused() {
            m.surrender_focus(id);
        }
    });
    rig.frame(Vec::new());
    assert!(rig.shows("END VIEW FROM +X"));
    // It stays selected until Esc.
    assert_eq!(
        rig.shell.viewport.selected_solid(),
        Some(WorkholdingItem::Start)
    );
    rig.press(Key::Escape, Modifiers::NONE);
    rig.frame(Vec::new());
    assert_eq!(rig.shell.viewport.selected_solid(), None);
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

#[test]
fn req_nfr_18_the_settings_dialog_switches_the_recording_and_keeps_the_choice() {
    let _held = crate::logging::SWITCH_LOCK.lock().unwrap();
    let config = temp("frame-record-actions");
    let file = SettingsFile::in_folder(&config);
    let mut rig = Rig::cube("frame-record-actions", Some(file.clone()));
    rig.dirs.push(config);
    assert!(crate::logging::recording());
    assert!(rig.shell.settings.record_actions);
    // The File menu's item opens the dialog; a frame with it open names it.
    rig.shell.dialog = Some(Dialog::Settings);
    rig.frame(Vec::new());
    assert_eq!(rig.shell.recorder_windows(), ["Settings"]);
    // The checkbox's action turns it off, in the settings and the log.
    rig.shell.set_record_actions(false);
    assert!(!crate::logging::recording());
    rig.shell.save_settings();
    assert!(!file.load().settings.record_actions);
    // A restart starts with it off.
    let shell = Shell::with_settings(false, None, Some(file.clone()));
    assert!(!crate::logging::recording());
    assert!(!shell.settings.record_actions);
    // The file is not changed by turning it back on in memory only.
    rig.shell.set_record_actions(true);
    assert!(crate::logging::recording());
    rig.shell.dialog = None;
    rig.frame(Vec::new());
    assert!(rig.shell.recorder_windows().is_empty());
}

/// Issue #382, the owner's decision D1 (decision record 0322): the memory
/// for playback checkpoints is set in Settings, kept in the user's settings,
/// given to playback, and brought into its range.
#[test]
fn req_ui_15_req_nfr_07_settings_sets_the_memory_for_playback_checkpoints() {
    let config = temp("frame-playback-memory");
    let file = SettingsFile::in_folder(&config);
    let mut rig = Rig::cube("frame-playback-memory", Some(file.clone()));
    rig.dirs.push(config);
    const MIB: usize = 1024 * 1024;
    // A computer of 32 GB, as the owner's laptop: no cap below 8,192 MB.
    rig.shell.physical_memory = Some(32 * 1024 * MIB as u64);
    rig.shell.set_playback_memory_mb(1_536);
    assert_eq!(rig.shell.play.playback_memory(), 1_536 * MIB);
    rig.shell.dialog = Some(Dialog::Settings);
    // A window is laid out on its first frame and drawn on the next.
    rig.frame(Vec::new());
    rig.frame(Vec::new());
    assert!(rig.shows("Recovery checkpoint interval"));
    assert!(rig.shows("Memory for playback checkpoints (MB)"));
    rig.shell.set_playback_memory_mb(4_096);
    assert_eq!(rig.shell.settings.simulate.playback_memory_mb, 4_096);
    assert_eq!(rig.shell.play.playback_memory(), 4_096 * MIB);
    rig.shell.set_playback_memory_mb(50);
    assert_eq!(rig.shell.settings.simulate.playback_memory_mb, 256);
    assert_eq!(rig.shell.play.playback_memory(), 256 * MIB);
    rig.shell.set_playback_memory_mb(3_000);
    rig.shell.save_settings();
    assert_eq!(file.load().settings.simulate.playback_memory_mb, 3_072);
    // A restart gives playback the kept value (capped at a quarter of this
    // computer's memory, as the owner decided on OI-1315).
    let shell = Shell::with_settings(false, None, Some(file.clone()));
    let expected = crate::memory::playback_memory(3_072, crate::memory::physical()).0;
    assert_eq!(shell.play.playback_memory(), expected);
    // On a computer of 4 GB the 3,072 MB asked for is capped at 1,024 MB,
    // the setting is kept, and Settings says so.
    rig.shell.physical_memory = Some(4 * 1024 * MIB as u64);
    rig.shell.set_playback_memory_mb(3_072);
    assert_eq!(rig.shell.settings.simulate.playback_memory_mb, 3_072);
    assert_eq!(rig.shell.play.playback_memory(), 1_024 * MIB);
    rig.frame(Vec::new());
    assert!(rig.shows("This computer: at most 1024 MB"));
    rig.shell.set_playback_memory_mb(512);
    assert_eq!(rig.shell.play.playback_memory(), 512 * MIB);
    rig.frame(Vec::new());
    assert!(!rig.shows("This computer: at most"));
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

/// Makes `rig`'s first operation lack its tool and lets the shell see it.
fn without_tool(rig: &mut Rig) -> openchuck_app::project::id::OperationId {
    use openchuck_app::project::Item;
    let app = rig.shell.app.as_mut().unwrap();
    let mut op = app.project().operations()[0].clone();
    op.tool = None;
    let id = op.id;
    app.execute(Command::Replace(Item::Operation(op))).unwrap();
    rig.settle();
    id
}

#[test]
fn req_ui_11_the_step_bar_says_the_overall_status_in_words_and_shape() {
    // Issue 210: the bar follows the whole job (green, yellow, red) and says
    // so in words, not by colour alone; each step has its own mark.
    let mut rig = Rig::cube("bar-status", None);
    // A fresh cube project has an operation never generated: stale.
    assert_eq!(
        rig.shell.overview.health(),
        openchuck_app::overview::StepHealth::Attention
    );
    assert!(rig.shows("Needs attention"));
    let op = without_tool(&mut rig);
    assert_eq!(
        rig.shell.overview.health(),
        openchuck_app::overview::StepHealth::Error
    );
    assert!(rig.shows("Error to fix"));
    // The badge and the step names do not run into each other, at the
    // smallest window the specification names (UI-16) and at 120 % scale.
    for (size, ppp) in [
        (egui::vec2(1366.0, 768.0), 1.0),
        (egui::vec2(1366.0, 768.0), 1.2),
    ] {
        rig.size = size;
        rig.ctx.set_pixels_per_point(ppp);
        rig.frame(Vec::new());
        rig.frame(Vec::new());
        let rects = rig.text_rects();
        let badge = rects
            .iter()
            .find(|(t, _)| t == "Error to fix")
            .map(|(_, r)| *r)
            .expect("the badge is drawn");
        for (text, rect) in &rects {
            if [
                "Model",
                "Setup",
                "Operations",
                "Simulate",
                "G-code not written",
            ]
            .contains(&text.as_str())
                && (rect.center().y - badge.center().y).abs() < 6.0
            {
                assert!(
                    !rect.intersects(badge),
                    "{text} overlaps the badge at {ppp}"
                );
            }
        }
    }
    // Clicking it goes to the first problem: the operation, at the tool.
    rig.shell.go_to_first_problem();
    assert_eq!(rig.shell.selected, Some(Selection::Operation(op)));
    assert_eq!(
        rig.shell.requirement_jump,
        Some(openchuck_app::requirements::RequirementField::Tool)
    );
    rig.frame(Vec::new());
    rig.frame(Vec::new());
    assert!(rig.shows("1 value is missing"));
    assert!(rig.shows("Choose a tool"));
    assert!(
        rig.shell.requirement_jump.is_none(),
        "the jump is used once"
    );
}

#[test]
fn req_ui_11_a_setup_without_stock_says_so_under_its_section_and_in_the_bar() {
    use openchuck_app::project::Item;
    let mut rig = Rig::cube("bar-setup", None);
    let app = rig.shell.app.as_mut().unwrap();
    let mut setup = app.project().setups()[0].clone();
    let id = setup.id;
    setup.stock = None;
    app.execute(Command::Replace(Item::Setup(setup))).unwrap();
    rig.settle();
    assert_eq!(
        rig.shell.overview.step(Step::Setup).health,
        openchuck_app::overview::StepHealth::Error
    );
    rig.shell.go_to_first_problem();
    assert_eq!(rig.shell.step, Step::Setup);
    assert_eq!(rig.shell.selected, Some(Selection::Setup(id)));
    rig.frame(Vec::new());
    rig.frame(Vec::new());
    assert!(rig.shows("1 value is missing"));
    assert!(rig.shows("Set the stock"));
}

#[test]
fn req_nfr_12_req_ui_06_undoing_a_turn_off_brings_the_path_back() {
    // OI-692: a run dropped because its operation was turned off is shown
    // again when the operation is back (undo or redo), as the export check
    // allows it again.
    let mut rig = Rig::cube("undo-path", None);
    let op = rig.shell.app.as_ref().unwrap().project().operations()[0].id;
    rig.shell.primary();
    rig.settle();
    let deadline = Instant::now() + Duration::from_secs(120);
    let drawn = |rig: &Rig| {
        rig.shell
            .viewport
            .layer(openchuck_viewport::Layer::Paths)
            .is_some_and(|p| !p.lines.is_empty())
    };
    while !(rig.shell.play.has_run() && drawn(&rig)) {
        assert!(Instant::now() < deadline, "timed out waiting for the path");
        rig.settle();
    }
    rig.shell
        .app
        .as_mut()
        .unwrap()
        .execute(Command::SetOperationEnabled {
            operation: op,
            enabled: false,
        })
        .unwrap();
    rig.frame(Vec::new());
    rig.frame(Vec::new());
    assert!(!rig.shell.play.has_run());
    assert!(!drawn(&rig), "the path of a turned-off operation is gone");

    rig.shell.undo();
    while !(rig.shell.play.has_run() && drawn(&rig)) {
        assert!(Instant::now() < deadline, "timed out waiting for the path");
        rig.settle();
    }
    // Redo turns it off again, and the path goes again.
    rig.shell.redo();
    rig.frame(Vec::new());
    rig.frame(Vec::new());
    assert!(!rig.shell.play.has_run());
    assert!(!drawn(&rig));
}

/// The window title names the application, the project and its folder, and
/// is sent to the system once it is known (#218).
#[test]
fn req_ui_01_the_window_title_follows_the_open_project() {
    let mut blank = Rig::new(Shell::with_settings(false, None, None), Vec::new());
    blank.frame(Vec::new());
    assert_eq!(blank.shell.sent_title, "OpenChuckCAM");

    let mut rig = Rig::cube("window-title", None);
    rig.frame(Vec::new());
    let folder = rig
        .shell
        .app
        .as_ref()
        .unwrap()
        .location()
        .display()
        .to_string();
    let title = rig.shell.sent_title.clone();
    assert!(title.starts_with("OpenChuckCAM - "), "{title}");
    assert!(
        !title.contains("* - "),
        "saved project has no mark: {title}"
    );
    assert!(
        title.ends_with(&crate::window_title::shorten_middle(&folder, 70)),
        "{title}"
    );
}

/// Draws the Setup inspector at `window` physical pixels, UI `scale` and an
/// inspector `pane` width (points) and returns the entry boxes in it with the
/// pane's left and right edges.
fn setup_entries(window: (f32, f32), scale: f32, pane: f32) -> (Vec<egui::Rect>, f32, f32, f32) {
    let mut rig = Rig::cube("inspector-width", None);
    // The application's own style, as the layout depends on its spacing.
    crate::theme::install_style(&rig.ctx, crate::theme::Theme::Dark);
    rig.ctx.set_zoom_factor(scale);
    rig.size = egui::vec2(window.0 / scale, window.1 / scale);
    rig.shell.settings.inspector.size = pane;
    let setup = rig.shell.app.as_ref().unwrap().project().setups()[0].id;
    rig.shell.choose_setup(setup, None);
    for _ in 0..6 {
        rig.frame(Vec::new());
    }
    let right = rig.size.x;
    let left = right - rig.shell.settings.inspector.size;
    let entries = rig
        .entry_rects()
        .into_iter()
        .filter(|r| r.left() > left)
        .collect();
    let scroll = egui::style::ScrollStyle::solid();
    let bar = scroll.bar_width + scroll.bar_inner_margin + scroll.bar_outer_margin;
    (entries, left, right, bar)
}

/// Field-row layout (specification section 8, UI-16, finding F-19 of issue
/// 174): the entry boxes use the width the pane leaves after the label column, and the scroll bar never covers them, at the window sizes
/// and UI scales of the trial.
#[test]
fn req_ui_16_req_ui_07_entry_boxes_fit_the_pane_and_the_scroll_bar_never_covers_them() {
    for window in [(1366.0, 768.0), (1920.0, 700.0), (1100.0, 700.0)] {
        for scale in [1.0, 1.2] {
            for pane in [280.0, 388.0, 520.0] {
                let (entries, left, right, bar) = setup_entries(window, scale, pane);
                let texts: Vec<_> = entries
                    .iter()
                    .filter(|r| r.width() > 60.0 && r.height() < 30.0)
                    .collect();
                if texts.is_empty() {
                    // The inspector collapsed to its rail (UI-16).
                    assert!(window.0 / scale < 1200.0, "{window:?} x{scale} {pane}");
                    continue;
                }
                let width = right - left;
                for r in &texts {
                    let at = format!("{r:?} at {window:?} x{scale}, pane {pane}");
                    assert!(r.right() <= right - bar, "covered by the scroll bar: {at}");
                    assert!(
                        r.left() <= left + 48.0 + width::LABEL_MAX + 12.0,
                        "pushed right of the label column: {at}"
                    );
                }
                let widest = texts.iter().map(|r| r.width()).fold(0.0, f32::max);
                // A number box keeps its compact width (decision 0440); the
                // pane's width goes to the labels first.
                assert!(
                    widest >= 60.0,
                    "entries {widest} wide in a pane {width} wide at {window:?} x{scale}"
                );
            }
        }
    }
}

/// Whether two widths are the same to a hundredth of a point.
fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.01
}

/// Whether an entry rectangle is a number or text box of the pane (the
/// segmented controls and buttons are narrower).
fn is_text_box(r: &egui::Rect, window_width: f32) -> bool {
    r.width() >= 100.0 && r.height() < 24.0 && r.left() >= window_width - 340.0
}

impl Rig {
    /// A click at `at`, over three frames.
    pub(super) fn click(&mut self, at: egui::Pos2) {
        let button = |pressed| Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        self.frame(vec![Event::PointerMoved(at)]);
        self.frame(vec![button(true)]);
        self.frame(vec![button(false)]);
    }

    /// Draws until the inspector's layout has not changed for three frames
    /// (the scroll bar's strip animates in over a few frames).
    fn settle_layout(&mut self) {
        let mut same = 0;
        let mut last = self.inspector_layout();
        for _ in 0..120 {
            self.frame(Vec::new());
            let now = self.inspector_layout();
            same = if now == last { same + 1 } else { 0 };
            if same >= 3 {
                return;
            }
            last = now;
        }
        panic!("the inspector's layout does not settle");
    }

    /// The inspector's width as drawn and the entry boxes in it.
    fn inspector_layout(&self) -> (f32, Vec<egui::Rect>) {
        let left = self.size.x - self.shell.settings.inspector.size;
        let entries = self
            .entry_rects()
            .into_iter()
            .filter(|r| r.left() > left)
            .collect();
        (self.shell.settings.inspector.size, entries)
    }
}

/// Typing in an entry box, confirming it and leaving it never changes the
/// size or position of any box or of the pane (issue 198, F-42; the shared
/// layout cause of issues 174, 178, 179, 190 and 215: a row that grows with
/// its content makes the pane widen). Checked in the operation inspector and
/// the Setup inspector, at 100% and 120%.
#[test]
fn req_ui_16_req_ui_07_typing_in_an_entry_box_moves_nothing() {
    for scale in [1.0, 1.2] {
        for setup in [false, true] {
            let mut rig = Rig::cube("typing-stable", None);
            crate::theme::install_style(&rig.ctx, crate::theme::Theme::Dark);
            rig.ctx.set_zoom_factor(scale);
            rig.size = egui::vec2(1920.0 / scale, 1080.0 / scale);
            if setup {
                let id = rig.shell.app.as_ref().unwrap().project().setups()[0].id;
                // Focus the stock section so the numeric field is visible at both scales.
                rig.shell.choose_setup(id, Some(SetupPart::Stock));
            } else {
                let op = rig.shell.app.as_ref().unwrap().project().operations()[0].id;
                rig.shell.select(Selection::Operation(op));
            }
            rig.settle_layout();
            let (width, before) = rig.inspector_layout();
            let at = format!("scale {scale}, setup step {setup}");
            // Content never widens the pane beyond the width it was set to
            // (issues 178 and 179: it crept wider each frame).
            assert!(
                near(width, width::INSPECTOR.0),
                "the pane's width: {at} {width}"
            );
            let target = if setup {
                // This fixture originally edited the cylinder stock's Length.
                // Machine throughput controls now precede Stock, so ordinal 1
                // selects the bounded Block window instead. Refusing a huge
                // window legitimately adds an error row and clips later boxes.
                // Find the intended stock field by its label, keeping the same
                // valid, long numeric edit and every layout assertion below.
                let label = rig
                    .text_rects()
                    .into_iter()
                    .find(|(text, rect)| text == "Length" && rect.left() > rig.size.x - width)
                    .map(|(_, rect)| rect)
                    .unwrap_or_else(|| panic!("no stock Length label: {at}"));
                before
                    .iter()
                    .find(|rect| {
                        is_text_box(rect, rig.size.x)
                            && rect.top() <= label.center().y
                            && rect.bottom() >= label.center().y
                    })
                    .copied()
                    .unwrap_or_else(|| panic!("no stock Length entry: {at}"))
            } else {
                before
                    .iter()
                    .filter(|r| is_text_box(r, rig.size.x))
                    .nth(1)
                    .copied()
                    .unwrap_or_else(|| panic!("no second text box: {at}: {before:?}"))
            };
            assert!(
                egui::Rect::from_min_size(egui::Pos2::ZERO, rig.size).contains(target.center()),
                "the intended entry is visible before clicking: {at}"
            );
            // All the wide boxes share one left and one right edge.
            let wide: Vec<_> = before
                .iter()
                .filter(|r| is_text_box(r, rig.size.x))
                .collect();
            // Each section has the label column its labels need, so the boxes
            // of one section share a left edge, within the column's range.
            let pane_left = rig.size.x - width;
            for r in &wide {
                let from = r.left() - pane_left;
                assert!(
                    (width::LABEL_MIN..=width::LABEL_MAX + 40.0).contains(&from),
                    "left edges: {at}: {from}"
                );
            }
            // Nothing in the pane is drawn past its right edge (issue 178:
            // chips and text were cut off at the right).
            let pane_left = rig.size.x - width;
            for (text, r) in rig.text_rects() {
                if r.left() > pane_left && r.top() > 90.0 && r.bottom() < rig.size.y - 50.0 {
                    assert!(r.right() <= rig.size.x, "{text:?} runs past the pane: {at}");
                }
            }
            rig.click(target.center());
            rig.settle_layout();
            let (width, before) = rig.inspector_layout();
            rig.frame(vec![Event::Text("123456789012345678901234567890".into())]);
            rig.frame(Vec::new());
            let (typing, during) = rig.inspector_layout();
            assert!(
                near(typing, width) && during == before,
                "while typing: {at}"
            );
            rig.press(Key::Enter, Modifiers::NONE);
            rig.frame(Vec::new());
            let (after, confirmed) = rig.inspector_layout();
            assert!(near(after, width), "after Enter: {at}");
            assert_eq!(confirmed.len(), before.len(), "after Enter: {at}");
            if setup {
                let app = rig.shell.app.as_ref().unwrap();
                // Length is displayed with three decimal places. A click
                // chooses a caret position, so digits may extend the fraction
                // rather than create a huge magnitude. Field identity follows
                // from the matched label; the machine window stays untouched.
                assert_eq!(
                    app.project().machine().unwrap().block_window,
                    1,
                    "the bounded machine window was not edited: {at}"
                );
            }
        }
    }
}

impl Rig {
    /// Drags from `from` to `to` in small steps and lets go.
    pub(super) fn drag(&mut self, from: egui::Pos2, to: egui::Pos2) {
        let button = |pos, pressed| Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        self.frame(vec![Event::PointerMoved(from)]);
        self.frame(vec![button(from, true)]);
        for i in 1..=10 {
            let pos = from + (to - from) * (i as f32 / 10.0);
            self.frame(vec![Event::PointerMoved(pos)]);
        }
        self.frame(vec![button(to, false)]);
    }
}

/// The inspector's width follows its splitter between the specification's
/// limits (section 3: 340 wide, 280 to 520), stays where it is dropped, and
/// its content stays aligned at the widest (issues 178 and 179, F-23 and
/// F-24: at 1920 × 1080 maximised the pane sprang back and its contents
/// were misaligned).
#[test]
fn req_ui_16_the_inspector_follows_its_splitter_within_its_range_and_stays_aligned() {
    let mut rig = Rig::cube("inspector-drag", None);
    crate::theme::install_style(&rig.ctx, crate::theme::Theme::Dark);
    rig.size = egui::vec2(1920.0, 1080.0);
    let id = rig.shell.app.as_ref().unwrap().project().setups()[0].id;
    rig.shell.choose_setup(id, None);
    rig.settle_layout();
    let y = 400.0;
    let (default, min, max) = width::INSPECTOR;
    assert!(near(rig.inspector_layout().0, default));
    let edge = |rig: &Rig| egui::pos2(rig.size.x - rig.inspector_layout().0, y);
    // Far to the left: the widest the specification allows.
    let from = edge(&rig);
    rig.drag(from, egui::pos2(from.x - 600.0, y));
    rig.settle_layout();
    assert!(near(rig.inspector_layout().0, max), "dragged to the widest");
    let (_, entries) = rig.inspector_layout();
    // The number boxes: compact whatever the width of the pane.
    let boxes: Vec<_> = entries
        .iter()
        .filter(|r| is_text_box(r, rig.size.x) && (r.width() - width::NUMBER_BOX).abs() < 0.6)
        .collect();
    assert!(!boxes.is_empty());
    for r in &boxes {
        assert!(
            (r.left() - boxes[0].left()).abs() < 0.6,
            "left edges at the widest"
        );
        assert!(
            (r.right() - boxes[0].right()).abs() < 0.6,
            "right edges at the widest"
        );
    }
    let pane_left = rig.size.x - max;
    for (text, r) in rig.text_rects() {
        if r.left() > pane_left && r.top() > 90.0 && r.bottom() < rig.size.y - 50.0 {
            assert!(r.right() <= rig.size.x, "{text:?} runs past the pane");
        }
    }
    // It stays where it was dropped, and follows the splitter back.
    for _ in 0..10 {
        rig.frame(Vec::new());
    }
    assert!(near(rig.inspector_layout().0, max), "stays at the widest");
    let from = edge(&rig) + egui::vec2(3.0, 0.0);
    rig.drag(from, egui::pos2(from.x + 300.0, y));
    rig.settle_layout();
    assert!(
        near(rig.inspector_layout().0, min),
        "dragged to the narrowest"
    );
}

/// Each model has its Placement and Mesh check rows in the tree (section 6;
/// issue #181, decision record 0201): a click on one selects the model and
/// opens that section of its inspector, and the Placement section says it
/// is shared by every setup.
#[test]
fn req_ui_06_the_model_has_placement_and_mesh_check_rows() {
    let mut rig = Rig::cube("model-rows", None);
    let defaults = ["check", "file", "placement", "units"];
    rig.shell
        .settings
        .set_section_open("model", "check", false, &defaults);
    rig.settle();
    let model = rig.shell.app.as_ref().unwrap().project().models()[0].id;
    let row = |rig: &Rig, text: &str| {
        rig.text_rects()
            .into_iter()
            .find(|(t, r)| t == text && r.left() < 300.0)
            .unwrap_or_else(|| panic!("no tree row '{text}'"))
            .1
    };
    let placement = row(&rig, "Placement");
    let check = row(&rig, "Mesh check");
    assert!(check.top() > placement.top());
    assert!(rig.shows("1 step"), "the placement chip");
    rig.click(placement.center());
    rig.settle();
    assert_eq!(rig.shell.selected, Some(Selection::Model(model)));
    assert_eq!(rig.shell.model_section, Some(crate::model_panel::PLACEMENT));
    assert!(rig.shows("shared by every setup"), "the Placement line");
    rig.click(row(&rig, "Mesh check").center());
    rig.settle();
    assert_eq!(
        rig.shell.model_section,
        Some(crate::model_panel::MESH_CHECK)
    );
    // A section opened to show it is not kept open in the settings.
    assert!(!rig.shell.settings.section_open("model", "check", true));
    // Choosing the model another way chooses no section of it.
    rig.shell.show_item(ItemRef::Model(model));
    assert_eq!(rig.shell.model_section, None);
}

/// The operation inspector names the strategy under its own heading with the
/// same plain description the New operation list shows, apart from the
/// operation's own name; a rename leaves the strategy as it was (issue 242;
/// decision record 0204; OP-16, UI-07, GC-07).
#[test]
fn req_op_16_req_ui_07_the_inspector_shows_the_strategy_apart_from_the_name() {
    let mut rig = Rig::cube("inspector-strategy", None);
    rig.size = egui::vec2(1440.0, 2400.0);
    let app = rig.shell.app.as_mut().unwrap();
    let mut op = app.project().operations()[0].clone();
    op.name = "Operation 3".to_owned();
    app.execute(Command::Replace(openchuck_app::project::Item::Operation(
        op.clone(),
    )))
    .unwrap();
    let label = app.strategy_label(&op.strategy);
    let text = app.strategy_description(&op.strategy);
    assert!(!text.is_empty());
    rig.settle();
    rig.shell.select(Selection::Operation(op.id));
    rig.frame(Vec::new());
    rig.frame(Vec::new());
    assert!(rig.shows("Operation 3"));
    assert!(rig.shows("Strategy"));
    assert!(rig.shows(&label), "{label}");
    assert!(rig.shows(&text[..40]), "{text}");
}

/// New operation shows the selected strategy's explanation and keeps the
/// other choices in a dropdown (decision 0237 supersedes 0204's full list).
#[test]
fn req_op_16_req_ui_06_the_new_operation_list_groups_the_strategies_with_descriptions() {
    let mut rig = Rig::cube("new-operation-list", None);
    rig.size = egui::vec2(1440.0, 2400.0);
    let app = rig.shell.app.as_ref().unwrap();
    let choices = app.strategy_choices();
    rig.shell.run.begin_add(app, None, None);
    rig.frame(Vec::new());
    rig.frame(Vec::new());
    assert!(rig.shows("Strategy"));
    assert!(!rig.shows("Finishing"));
    assert!(rig.shows("Rotary roughing"));
    // The list scrolls, so only the first entries are drawn; the first
    // entry's description is under its name.
    assert!(rig.shows(&choices[0].description[..40]));
}

/// Issue 201, the owner's option C: the status bar shows the current job,
/// and choosing it opens the Jobs tab, with every job, its history and
/// Cancel, even when the dock was collapsed.
#[test]
fn req_ui_09_the_job_in_the_status_bar_opens_the_jobs_tab() {
    let mut rig = Rig::cube("frame-jobs-status", None);
    rig.shell.toggle_region(super::frame::Region::Dock);
    rig.frame(Vec::new());
    assert!(rig.shell.settings.dock.collapsed);
    let (_, at) = rig
        .text_rects()
        .into_iter()
        .find(|(t, _)| t == "Ready")
        .expect("the status bar says Ready");
    rig.click(at.center());
    assert_eq!(rig.shell.dock_tab, super::dock::DockTab::Jobs);
    assert!(!rig.shell.settings.dock.collapsed);
}

#[test]
fn req_ui_09_req_ui_16_the_job_in_the_status_bar_opens_a_dock_the_small_window_closed() {
    // CodeRabbit on pull request 281: the small-window rule closes the dock
    // without the user's setting, and the status bar left it closed.
    let mut rig = Rig::cube("frame-jobs-small", None);
    rig.size = egui::vec2(1300.0, 700.0);
    rig.frame(Vec::new());
    assert!(rig.shell.collapsed(Region::Dock));
    assert!(!rig.shell.settings.dock.collapsed);
    let (_, at) = rig
        .text_rects()
        .into_iter()
        .find(|(t, _)| t == "Ready")
        .expect("the status bar says Ready");
    rig.click(at.center());
    assert_eq!(rig.shell.dock_tab, super::dock::DockTab::Jobs);
    assert!(!rig.shell.collapsed(Region::Dock));
}

#[test]
fn req_ui_02_req_ui_03_replacing_a_scene_job_preserves_pending_camera_fit() {
    let mut rig = Rig::cube("replace-scene-camera-fit", None);
    // Do not poll between requests: the pending fit belongs to the result
    // still owed to the viewport, even if its worker has already finished.
    rig.shell.rebuild_scene(true);
    rig.shell.rebuild_scene(false);
    assert!(rig.shell.scene_job.as_ref().unwrap().1);
    rig.settle();
    // Once consumed, the next ordinary rebuild must not request another fit.
    rig.shell.rebuild_scene(false);
    assert!(!rig.shell.scene_job.as_ref().unwrap().1);
    rig.shell.rebuild_scene(true);
    assert!(rig.shell.scene_job.as_ref().unwrap().1);
    rig.settle();
}

/// Issues 308 and 310: a template the user confirms in the export dialog is
/// their setting, kept for the next project and the next start; the dialog
/// opens with it.
#[test]
fn req_ui_15_req_gc_05_a_confirmed_file_name_template_is_kept_across_projects() {
    let config = temp("frame-file-names-config");
    let file = SettingsFile::in_folder(&config);
    let mut rig = Rig::cube("frame-file-names", Some(file.clone()));
    rig.dirs.push(config.clone());
    rig.press(Key::E, Modifiers::COMMAND);
    assert!(rig.shell.export.is_open());
    let mine = openchuck_app::FileNameTemplates {
        gcode: "{project}_{n}".into(),
        setup_sheet: "{project}_sheet".into(),
    };
    rig.shell.export.set_file_names(mine.clone());
    rig.shell.settings_changed = None;
    rig.frame(Vec::new());
    assert_eq!(rig.shell.settings.file_names, mine);
    assert!(rig.shell.settings_changed.is_some());
    rig.shell.save_settings();
    assert_eq!(file.load().settings.file_names, mine);
    // A restart opens the dialog with them.
    let mut again = Rig::cube("frame-file-names-2", Some(file));
    // Frames before the dialog opens leave the user's templates alone.
    again.frame(Vec::new());
    assert_eq!(again.shell.settings.file_names, mine);
    again.press(Key::E, Modifiers::COMMAND);
    assert!(
        again.shell.export.is_open(),
        "{:?}",
        again.shell.post_blocker()
    );
    assert_eq!(
        again.shell.export.file_names_to_keep(&again.ctx),
        Some(&mine)
    );
}
