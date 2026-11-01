// SPDX-License-Identifier: AGPL-3.0-or-later
//! The window's regions (specification section 3): the title bar with the
//! menus and the save state, the workflow bar with the steps (section 4),
//! the project panel, the inspector and the dock with their collapse rails
//! and splitters, the viewport with its toolbar and legend strips, the
//! footer with the primary action (sections 5 and 10), and the empty state
//! (section 11.1). The panel layout and the scale are kept per user (UI-15,
//! UI-16; decision record 0117).

use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, RichText, Stroke};
use openchuck_app::overview::{ItemStatus, Overview, PrimaryAction, Step, StepHealth, StepState};
use openchuck_app::settings::{SCALE_RANGE, region};
use openchuck_viewport::{DisplayMode, Projection, StandardView};

use super::{Dialog, EXPORT, IMPORT, NEW, Next, OPEN, REDO, SAVE, Selection, Shell, UNDO, marks};
use crate::theme::{DARK, RADIUS};
use crate::toolbar;
use crate::viewport::ViewStep;
use crate::viewport_toolbar::directions_unavailable;

/// The three regions that collapse to a rail (Ctrl 1, 2 and 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Region {
    Project,
    Inspector,
    Dock,
}

impl Region {
    pub(super) const ALL: [Self; 3] = [Self::Project, Self::Inspector, Self::Dock];

    fn name(self) -> &'static str {
        match self {
            Self::Project => "project panel",
            Self::Inspector => "inspector",
            Self::Dock => "dock",
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::Project => "Ctrl 1",
            Self::Inspector => "Ctrl 2",
            Self::Dock => "Ctrl 3",
        }
    }
}

/// Regions the small-window rule has collapsed for now, without changing
/// what the user chose (specification section 3, "Smallest window").
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct AutoCollapse {
    dock: bool,
    inspector: bool,
    /// The user opened the region the rule closed; it stays open until the
    /// window is large again.
    dock_kept_open: bool,
    inspector_kept_open: bool,
}

/// Below this window size, in logical pixels, the dock collapses first
/// (UI-16).
pub(super) const SMALLEST: egui::Vec2 = egui::vec2(1366.0, 768.0);
/// The narrowest the viewport may get before the inspector collapses too.
const NARROWEST_VIEW: f32 = 420.0;
/// The width of a collapsed side panel's rail.
const RAIL: f32 = 28.0;
/// How long the settings wait after a change before they are written.
/// The scales the View menu offers, in percent (issue 141; NFR-15).
pub(super) const UI_SCALES: [u32; 5] = [80, 90, 100, 110, 120];
const SETTINGS_DELAY: Duration = Duration::from_secs(1);

/// Heights of the bars (specification section 3).
const TITLE_BAR: f32 = 32.0;
const WORKFLOW_BAR: f32 = 44.0;
const FOOTER: f32 = 48.0;
const LEGEND_STRIP: f32 = 36.0;

/// The keys, for Help > Keyboard shortcuts (specification section 16). The
/// viewport's F, V and numpad keys and playback's Space and arrows come with
/// the viewport toolbar and the playback dock (lanes X3 and X6).
pub(super) const KEY_MAP: &[(&str, &str)] = &[
    ("Ctrl N, Ctrl O, Ctrl S", "New, open, save project"),
    ("Ctrl I", "Import model"),
    ("Ctrl E", "Post and export"),
    ("Ctrl Z, Ctrl Y", "Undo, redo"),
    ("Alt 1 to Alt 4", "Workflow steps"),
    ("Alt 5", "Post and export (as Ctrl E)"),
    (
        "Ctrl 1, Ctrl 2, Ctrl 3",
        "Collapse the project panel, inspector, dock",
    ),
    ("N", "New operation"),
    (
        "G or F5",
        "Generate the selected operation, and the ones before it",
    ),
    ("Shift G or Shift F5", "Generate every stale operation"),
    (
        "Ctrl D, Del, F2",
        "Duplicate, delete, rename the selected item",
    ),
    (
        "Alt ↑, Alt ↓",
        "Move the selected operation earlier or later",
    ),
    (
        "Q, W, E, R",
        "Select one, flood fill, by angle, same direction (while picking)",
    ),
    ("Esc", "Cancel the running job, or leave picking"),
];

/// A status chip in the words of the item's mark (specification section 8).
pub(super) fn status_chip(ui: &mut egui::Ui, status: Option<&ItemStatus>) {
    if let Some(status) = status {
        ui.horizontal(|ui| {
            marks::mark(ui, status.state, &status.cause);
            ui.label(RichText::new(status.state.name()).small());
        });
    }
}

/// A small bordered chip, measured and drawn in one rectangle so its place
/// does not depend on the layout it is in (see `field_row::chip`).
pub(crate) fn chip(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let colour = ui.visuals().text_color();
    crate::field_row::chip(ui, text, colour)
}

/// The filled primary button (specification sections 5 and 14.1).
pub(crate) fn primary_button(ui: &mut egui::Ui, label: &str, enabled: bool) -> egui::Response {
    let button = egui::Button::new(RichText::new(label).color(Color32::WHITE).strong())
        .fill(DARK.primary)
        .stroke(Stroke::new(1.0, DARK.primary_border))
        .min_size(egui::vec2(0.0, 28.0));
    ui.add_enabled(enabled, button)
}

impl Shell {
    /// Reveal the operation inspector after guarded Simulate-row navigation,
    /// including when the small-window rule would otherwise close it again.
    pub(super) fn reveal_operation_inspector(&mut self) {
        self.settings.inspector.collapsed = false;
        self.auto_collapsed.inspector = false;
        self.auto_collapsed.inspector_kept_open = true;
        self.settings_touched();
    }

    /// Collapses or opens `region` (Ctrl 1, 2 and 3; the chevrons).
    pub(super) fn toggle_region(&mut self, region: Region) {
        if region == Region::Inspector && self.defer(move |s| s.toggle_region(region)) {
            return;
        }
        let layout = match region {
            Region::Project => &mut self.settings.project_panel,
            Region::Inspector => &mut self.settings.inspector,
            Region::Dock => &mut self.settings.dock,
        };
        let auto = match region {
            Region::Project => false,
            Region::Inspector => self.auto_collapsed.inspector,
            Region::Dock => self.auto_collapsed.dock,
        };
        // A region the small-window rule closed opens on the user's word.
        if auto {
            layout.collapsed = false;
            match region {
                Region::Inspector => {
                    self.auto_collapsed.inspector = false;
                    self.auto_collapsed.inspector_kept_open = true;
                }
                Region::Dock => {
                    self.auto_collapsed.dock = false;
                    self.auto_collapsed.dock_kept_open = true;
                }
                Region::Project => {}
            }
        } else {
            layout.collapsed = !layout.collapsed;
        }
        self.settings_touched();
    }

    pub(super) fn collapsed(&self, region: Region) -> bool {
        match region {
            Region::Project => self.settings.project_panel.collapsed,
            Region::Inspector => self.settings.inspector.collapsed || self.auto_collapsed.inspector,
            Region::Dock => self.settings.dock.collapsed || self.auto_collapsed.dock,
        }
    }

    /// Notes that the settings changed; they are written a moment later.
    pub(super) fn settings_touched(&mut self) {
        self.settings_changed = Some(Instant::now());
    }

    /// Writes the settings if they changed a while ago, and follows the
    /// scale the user chose with egui's own keys, within its range.
    pub(super) fn settings_frame(&mut self, ctx: &egui::Context) {
        let zoom = ctx.zoom_factor();
        let clamped = zoom.clamp(SCALE_RANGE.0, SCALE_RANGE.1);
        if (clamped - zoom).abs() > f32::EPSILON {
            ctx.set_zoom_factor(clamped);
        }
        if (clamped - self.settings.scale).abs() > 0.001 && self.bench.is_none() {
            self.settings.scale = clamped;
            self.settings_touched();
        }
        // The window's size is part of its layout (UI-15).
        if self.bench.is_none()
            && let Some(rect) = ctx.input(|i| i.viewport().inner_rect)
        {
            // In the window's logical pixels, as the platform sizes it: egui's
            // points are those divided by the scale.
            let size = [
                (rect.width() * zoom).round(),
                (rect.height() * zoom).round(),
            ];
            if self.settings.window != Some(size) {
                self.settings.window = Some(size);
                self.settings_touched();
            }
        }
        if let Some(at) = self.settings_changed {
            if at.elapsed() >= SETTINGS_DELAY {
                self.save_settings();
            } else {
                ctx.request_repaint_after(SETTINGS_DELAY);
            }
        }
    }

    /// Writes the settings now, if they changed. A failure is a message,
    /// never a dialog: nothing the user asked for failed.
    pub(super) fn save_settings(&mut self) {
        if self.settings_changed.take().is_none() {
            return;
        }
        if let Some(file) = &self.settings_file
            && let Err(e) = file.save(&self.settings)
        {
            self.settings_problem = Some(format!(
                "Your settings could not be kept on this computer: {e}"
            ));
            self.overview_dirty = true;
        }
    }

    /// The small-window rule: below 1366 × 768 the dock collapses, and the
    /// inspector too when the viewport would get narrower than it can use.
    fn small_window(&mut self, screen: egui::Vec2) {
        let small = screen.x < SMALLEST.x || screen.y < SMALLEST.y;
        if !small {
            self.auto_collapsed.dock_kept_open = false;
            self.auto_collapsed.inspector_kept_open = false;
        }
        self.auto_collapsed.dock =
            small && !self.settings.dock.collapsed && !self.auto_collapsed.dock_kept_open;
        let side =
            |l: &openchuck_app::settings::PanelLayout| if l.collapsed { RAIL } else { l.size };
        let view = screen.x - side(&self.settings.project_panel) - side(&self.settings.inspector);
        self.auto_collapsed.inspector = small
            && view < NARROWEST_VIEW
            && !self.settings.inspector.collapsed
            && !self.auto_collapsed.inspector_kept_open;
    }

    /// Lays out every region and draws it (specification section 3).
    pub(super) fn layout(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.small_window(ctx.content_rect().size());
        egui::Panel::top("title bar")
            .exact_size(TITLE_BAR)
            .frame(bar_frame())
            .show(ui, |ui| self.title_bar(ui));
        egui::Panel::top("workflow bar")
            .exact_size(WORKFLOW_BAR)
            .show(ui, |ui| self.workflow_bar(ui));
        egui::Panel::bottom("footer")
            .exact_size(FOOTER)
            .frame(bar_frame())
            .show(ui, |ui| self.footer(ui));
        self.side_panel(ui, Region::Project);
        self.side_panel(ui, Region::Inspector);
        self.viewport.set_picking(matches!(
            self.selected,
            Some(Selection::Picking | Selection::Set(_))
        ));
        self.viewport.set_pick_filter(self.selection.pick_filter());
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                self.dock_panel(ui);
                // The view's toolbar and overlays follow the step (7.1, 7.2).
                self.viewport.set_step(Some(view_step(self.step)));
                self.viewport.set_project_has_models(
                    self.app
                        .as_ref()
                        .is_some_and(|a| !a.project().models().is_empty()),
                );
                // The Setup step shows two orthographic panes instead of the
                // 3D view (11.3).
                let panes = self.setup_panes();
                if let Some(panes) = &panes {
                    egui::Panel::bottom("legend strip")
                        .exact_size(LEGEND_STRIP)
                        .show(ui, |ui| toolbar::pane_legend(ui, &panes.legend));
                } else if toolbar::has_legend(self.step, self.viewport.view()) {
                    egui::Panel::bottom("legend strip")
                        .exact_size(LEGEND_STRIP)
                        .show(ui, |ui| toolbar::legend(ui, self.viewport.view()));
                }
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(ui, |ui| {
                        if self.app.is_none()
                            || self
                                .app
                                .as_ref()
                                .is_some_and(|a| a.project().models().is_empty())
                                && self.import.is_none()
                        {
                            self.empty_state(ui);
                            return;
                        }
                        if let Some(panes) = &panes {
                            let (focused, selected) = self.viewport.marked_solids();
                            let marks = crate::setup_panes::Marks { focused, selected };
                            // Each setup keeps its panes' zoom and pan for
                            // the session (decision record 0203).
                            let seen = crate::setup_panes::show(
                                ui,
                                panes,
                                marks,
                                self.viewport.shows_unapplied(),
                                egui::Id::unique(("setup panes view", self.setup)),
                            );
                            if let Some(item) = seen.clicked {
                                self.viewport.click_solid(item);
                            }
                            return;
                        }
                        let corner = crate::selection_panel::tools_corner(ui.max_rect());
                        self.viewport.show(ui);
                        // A result that no longer matches the project says so on
                        // the result itself (issue 260).
                        self.view_banner(ui.ctx());
                        // The selection tools sit over the top left of the view.
                        if self.viewport.is_picking() {
                            self.selection.tools(ui.ctx(), corner);
                        }
                    });
            });
    }

    /// The two orthographic panes of the Setup step for the setup the view
    /// shows (11.3), or `None` on another step or with no setup to draw. They
    /// draw the draft while the workholding section has unapplied changes
    /// (7.5), as the view does.
    fn setup_panes(&self) -> Option<openchuck_app::setup_panes::SetupPanes> {
        if self.step != Step::Setup {
            return None;
        }
        let app = self.app.as_ref()?;
        let (_, solids) = self.viewport.solids();
        openchuck_app::setup_panes::setup_panes(
            app.project(),
            self.viewport.view(),
            solids,
            self.viewport.failing_solids(),
            self.viewport.offered_solids(),
        )
    }

    /// The project panel or the inspector, or its rail when collapsed.
    fn side_panel(&mut self, ui: &mut egui::Ui, region: Region) {
        let (id, (default, min, max), left) = match region {
            Region::Project => ("tree", region::PROJECT, true),
            Region::Inspector => ("properties", region::INSPECTOR, false),
            Region::Dock => return,
        };
        let panel = |id: &str| {
            if left {
                egui::Panel::left(id.to_owned())
            } else {
                egui::Panel::right(id.to_owned())
            }
        };
        if self.collapsed(region) {
            panel(&format!("{id} rail"))
                .exact_size(RAIL)
                .resizable(false)
                .show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        let chevron = if left { "»" } else { "«" };
                        if ui
                            .button(chevron)
                            .on_hover_text(format!("Open the {} ({})", region.name(), region.key()))
                            .clicked()
                        {
                            self.toggle_region(region);
                        }
                    });
                });
            return;
        }
        let stored = match region {
            Region::Project => self.settings.project_panel.size,
            _ => self.settings.inspector.size,
        };
        let mut builder = panel(id)
            .resizable(true)
            .default_size(stored)
            .size_range(min..=max);
        if self.reset == Some(region) {
            builder = builder.exact_size(default);
            self.reset = None;
        }
        let response = builder.show(ui, |ui| {
            self.panel_header(ui, region);
            match region {
                Region::Project => self.tree(ui),
                _ => {
                    crate::theme::solid_scroll_bar(ui);
                    egui::ScrollArea::vertical()
                        .id_salt("inspector scroll")
                        // The pane's width is the user's, not the content's.
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            let ctx = ui.ctx().clone();
                            crate::pending_edits::form(
                                &ctx,
                                egui::Id::unique("inspector form"),
                                || self.properties(ui),
                            );
                        });
                }
            }
        });
        let rect = response.response.rect;
        let edge = if left { rect.right() } else { rect.left() };
        self.remember_size(ui, region, rect.width(), |p| (p.x - edge).abs() <= 4.0);
    }

    /// Keeps a region's size, and resets it on a double-click of its
    /// splitter (specification section 3). `on_splitter` says whether a
    /// pointer position is on the splitter.
    fn remember_size(
        &mut self,
        ui: &egui::Ui,
        region: Region,
        size: f32,
        on_splitter: impl Fn(egui::Pos2) -> bool,
    ) {
        let double = ui.input(|i| {
            i.pointer
                .button_double_clicked(egui::PointerButton::Primary)
                && i.pointer.interact_pos().is_some_and(&on_splitter)
        });
        if double {
            self.reset = Some(region);
        }
        let layout = match region {
            Region::Project => &mut self.settings.project_panel,
            Region::Inspector => &mut self.settings.inspector,
            Region::Dock => &mut self.settings.dock,
        };
        if (layout.size - size).abs() > 0.5 {
            layout.size = size;
            self.settings_changed = Some(Instant::now());
        }
    }

    /// A region's header: its title and the chevron that collapses it.
    fn panel_header(&mut self, ui: &mut egui::Ui, region: Region) {
        ui.horizontal(|ui| {
            let title = match region {
                Region::Project => "PROJECT".to_owned(),
                Region::Inspector => "INSPECTOR".to_owned(),
                Region::Dock => String::new(),
            };
            ui.label(RichText::new(title).small().strong().color(DARK.heading));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let chevron = if region == Region::Project {
                    "«"
                } else {
                    "»"
                };
                if ui
                    .small_button(chevron)
                    .on_hover_text(format!("Collapse the {} ({})", region.name(), region.key()))
                    .clicked()
                {
                    self.toggle_region(region);
                }
            });
        });
        ui.separator();
    }

    /// The dock under the view, or its rail when collapsed (specification
    /// section 9).
    fn dock_panel(&mut self, ui: &mut egui::Ui) {
        if self.collapsed(Region::Dock) {
            egui::Panel::bottom("dock rail")
                .exact_size(RAIL)
                .resizable(false)
                .show(ui, |ui| {
                    // The show button is at the right end, where the hide
                    // button of the open dock is (issue 433).
                    ui.horizontal_centered(|ui| {
                        self.dock_tabs(ui);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .small_button("\u{23f6}")
                                .on_hover_text("Open the dock (Ctrl 3)")
                                .clicked()
                            {
                                self.toggle_region(Region::Dock);
                            }
                        });
                    });
                });
            return;
        }
        let (default, min, _) = region::DOCK;
        // The limit comes from the window, not from what is left after the other panels in
        // the first frame, which can be small before the window has its size.
        let max = (ui.ctx().content_rect().height() / 2.0).max(min);
        let requested = self.settings.dock.size.min(max);
        let mut builder = egui::Panel::bottom("dock")
            .resizable(true)
            .default_size(requested)
            .size_range(min..=max);
        if self.reset == Some(Region::Dock) {
            builder = builder.exact_size(default.min(max));
            self.settings.dock.size = default;
            self.settings_touched();
            self.reset = None;
        } else if self.dock_stale {
            builder = builder.exact_size(requested);
        }
        // egui 0.37 keys a panel's state by its parent's scope id and its salt;
        // the dock's parent is the central panel's `Ui`, whose id the tests
        // cannot compute.
        #[cfg(test)]
        ui.ctx()
            .data_mut(|d| d.insert_temp(egui::Id::unique("test dock parent"), ui.scope_id()));
        let response = builder.show(ui, |ui| self.dock(ui));
        let rect = response.response.rect;
        let top = rect.top();
        let dragging = ui.input(|i| {
            i.pointer.primary_down()
                && i.pointer
                    .press_origin()
                    .is_some_and(|p| (p.y - top).abs() <= 4.0)
        });
        self.dock_stale = !dragging && (rect.height() - requested).abs() > 0.5;
        // Only a drag by the user changes the saved size: a size the window clamped is not
        // the user's choice and must not replace it (OI-359).
        let size = if dragging {
            rect.height()
        } else {
            self.settings.dock.size
        };
        self.remember_size(ui, Region::Dock, size, |p| (p.y - top).abs() <= 4.0);
    }

    /// The title bar: the menus, the project's name and save state, and
    /// Undo and Redo (specification section 3).
    fn title_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_centered(|ui| {
            self.menu(ui);
            let (name, modified) = self
                .app
                .as_ref()
                .map_or(("No project".to_owned(), false), |a| {
                    (a.project().name().to_owned(), a.is_modified())
                });
            ui.add_space(24.0);
            let folder = self
                .app
                .as_ref()
                .map(|a| a.location().display().to_string());
            let label = ui.label(RichText::new(name).strong());
            if let Some(folder) = folder {
                label.on_hover_text(folder);
            }
            if self.app.is_some() {
                // A neutral dot and "Edited", or "· saved"; amber is kept
                // for warnings (specification section 10).
                let (text, tip) = crate::window_title::save_state(modified);
                if modified {
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
                    ui.painter().circle_filled(rect.center(), 3.5, DARK.muted);
                }
                let label = ui.label(RichText::new(text).color(DARK.muted));
                if let Some(tip) = tip {
                    label.on_hover_text(tip);
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let ctx = ui.ctx().clone();
                let (can_undo, can_redo) = self
                    .app
                    .as_ref()
                    .map_or((false, false), |a| (a.can_undo(), a.can_redo()));
                if ui
                    .add_enabled(can_redo, egui::Button::new("↷"))
                    .on_hover_text(format!("Redo ({})", ctx.format_shortcut(&REDO)))
                    .clicked()
                {
                    self.redo();
                }
                if ui
                    .add_enabled(can_undo, egui::Button::new("↶"))
                    .on_hover_text(format!("Undo ({})", ctx.format_shortcut(&UNDO)))
                    .clicked()
                {
                    self.undo();
                }
            });
        });
    }

    /// The File menu's "Open recent" submenu (issue 381; UI-15): the recent
    /// project folders, newest first, each by its folder name with the full
    /// path on hover, then "Clear recent list". A folder that is gone is
    /// forgotten when the menu is drawn. A choice goes through the ordinary
    /// Open command, so the unsaved-changes question and the recovery rules
    /// (decision 0460) apply; the Open dialog shows the folder to confirm.
    fn recent_menu(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.menu_button("Open recent", |ui| {
            let recent = self.recent_projects();
            if recent.is_empty() {
                ui.add_enabled(false, egui::Button::new("No recent projects"));
                return;
            }
            for (index, folder) in recent.iter().enumerate() {
                let clicked = ui
                    .push_id(index, |ui| {
                        ui.button(recent_label(folder))
                            .on_hover_text(super::checkpoints::shown(folder))
                            .clicked()
                    })
                    .inner;
                if clicked {
                    self.open_recent(folder, ctx);
                    ui.close();
                }
            }
            ui.separator();
            if ui.button("Clear recent list").clicked() {
                self.settings.clear_recent();
                self.settings_touched();
                ui.close();
            }
        });
    }

    /// The menus: File, Edit, View, Project, Tools and Help (specification
    /// section 3). Every command is here, with its key (UI-14).
    #[allow(clippy::too_many_lines)]
    fn menu(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let open = self.app.is_some();
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                let new =
                    egui::Button::new("New project…").shortcut_text(ctx.format_shortcut(&NEW));
                if ui.add(new).clicked() {
                    self.request(Next::New, &ctx);
                }
                let o =
                    egui::Button::new("Open project…").shortcut_text(ctx.format_shortcut(&OPEN));
                if ui.add(o).clicked() {
                    self.request(Next::Open, &ctx);
                }
                self.recent_menu(ui, &ctx);
                let import =
                    egui::Button::new("Import model…").shortcut_text(ctx.format_shortcut(&IMPORT));
                if ui
                    .add_enabled(open && self.import.is_none(), import)
                    .clicked()
                {
                    self.import_model();
                }
                let save = egui::Button::new("Save").shortcut_text(ctx.format_shortcut(&SAVE));
                if ui.add_enabled(open, save).clicked() {
                    self.save();
                }
                let export = egui::Button::new("Post and export…")
                    .shortcut_text(ctx.format_shortcut(&EXPORT));
                let blocker = self.post_blocker();
                let response = ui.add_enabled(blocker.is_none(), export);
                let response = match blocker {
                    Some(reason) => response.on_disabled_hover_text(reason),
                    None => response,
                };
                if response.clicked() {
                    self.open_export();
                    ui.close();
                }
                ui.separator();
                if ui.button("Settings…").clicked() {
                    self.open_dialog(Dialog::Settings);
                    ui.close();
                }
                if ui.button("Quit").clicked() {
                    self.request(Next::Quit, &ctx);
                }
            });
            ui.menu_button("Edit", |ui| {
                let (can_undo, can_redo) = self
                    .app
                    .as_ref()
                    .map_or((false, false), |a| (a.can_undo(), a.can_redo()));
                let undo = egui::Button::new("Undo").shortcut_text(ctx.format_shortcut(&UNDO));
                if ui.add_enabled(can_undo, undo).clicked() {
                    self.undo();
                }
                let redo = egui::Button::new("Redo").shortcut_text(ctx.format_shortcut(&REDO));
                if ui.add_enabled(can_redo, redo).clicked() {
                    self.redo();
                }
                ui.separator();
                let operation = match self.selected {
                    Some(Selection::Operation(id)) => Some(id),
                    _ => None,
                };
                let duplicate = egui::Button::new("Duplicate").shortcut_text("Ctrl+D");
                if ui.add_enabled(operation.is_some(), duplicate).clicked()
                    && let Some(id) = operation
                {
                    self.tree_action(super::TreeAction::Duplicate(id));
                    ui.close();
                }
                let delete = egui::Button::new("Delete").shortcut_text("Del");
                if ui.add_enabled(operation.is_some(), delete).clicked()
                    && let Some(id) = operation
                {
                    self.tree_action(super::TreeAction::Delete(id));
                    ui.close();
                }
                let item = self.selected_item();
                let rename = egui::Button::new("Rename…").shortcut_text("F2");
                if ui.add_enabled(item.is_some(), rename).clicked()
                    && let Some(item) = item
                {
                    self.begin_rename(item);
                    ui.close();
                }
            });
            ui.menu_button("View", |ui| {
                for step in Step::ALL {
                    let available = self.overview.step(step).is_available();
                    let button = egui::Button::selectable(self.step == step, step.name())
                        .shortcut_text(format!("Alt+{}", step.number()));
                    let response = ui.add_enabled(available, button);
                    let response = match self.overview.step(step).unavailable {
                        Some(reason) => response.on_disabled_hover_text(reason),
                        None => response,
                    };
                    if response.clicked() {
                        self.open_step(step);
                        ui.close();
                    }
                }
                ui.separator();
                ui.menu_button("Panels", |ui| {
                    for region in Region::ALL {
                        let mut shown = !self.collapsed(region);
                        let label = match region {
                            Region::Project => "Project panel",
                            Region::Inspector => "Inspector",
                            Region::Dock => "Dock",
                        };
                        if ui
                            .checkbox(&mut shown, format!("{label}  ({})", region.key()))
                            .changed()
                        {
                            self.toggle_region(region);
                        }
                    }
                });
                let unavailable = directions_unavailable(
                    self.setup_panes().is_some(),
                    self.viewport.project_has_models(),
                );
                ui.menu_button("View direction", |ui| {
                    for view in StandardView::ALL {
                        let response =
                            ui.add_enabled(unavailable.is_none(), egui::Button::new(view.name()));
                        let response = match unavailable {
                            Some(reason) => response.on_disabled_hover_text(reason),
                            None => response,
                        };
                        if response.clicked() {
                            self.viewport.set_view(view);
                            ui.close();
                        }
                    }
                });
                if ui.button("Fit all").clicked() {
                    self.viewport.fit();
                    ui.close();
                }
                ui.menu_button("Projection", |ui| {
                    let mut projection = self.viewport.camera.projection;
                    ui.radio_value(&mut projection, Projection::Perspective, "Perspective");
                    ui.radio_value(&mut projection, Projection::Orthographic, "Orthographic");
                    self.viewport.set_projection(projection);
                });
                ui.menu_button("Display", |ui| {
                    for mode in DisplayMode::ALL {
                        ui.radio_value(&mut self.viewport.mode, mode, mode.name());
                    }
                });
                // Text and controls scale together, 80% to 200% (NFR-15); the
                // menu offers fixed steps, and Ctrl plus and minus the rest.
                ui.menu_button(
                    format!("Scale ({:.0} %)", ctx.zoom_factor() * 100.0),
                    |ui| {
                        for percent in UI_SCALES {
                            let scale = percent as f32 / 100.0;
                            let chosen = (ctx.zoom_factor() - scale).abs() < 0.005;
                            if ui
                                .add(egui::Button::selectable(chosen, format!("{percent} %")))
                                .clicked()
                            {
                                ctx.set_zoom_factor(scale);
                                ui.close();
                            }
                        }
                    },
                );
            });
            ui.menu_button("Project", |ui| {
                let idle = !self.run.is_running();
                let operations = self.overview.step(Step::Operations).is_available();
                let new = egui::Button::new("New operation…").shortcut_text("N");
                if ui.add_enabled(open && operations, new).clicked() {
                    self.new_operation();
                    ui.close();
                }
                ui.separator();
                let selected = matches!(self.selected, Some(Selection::Operation(_)));
                let generate = egui::Button::new("Generate selected").shortcut_text("G");
                if ui
                    .add_enabled(open && idle && selected, generate)
                    .on_hover_text("The selected operation, and the operations before it")
                    .clicked()
                {
                    self.generate_selected();
                    ui.close();
                }
                let stale = egui::Button::new("Generate stale").shortcut_text("Shift+G");
                if ui.add_enabled(open && idle, stale).clicked() {
                    self.generate(false);
                    ui.close();
                }
                let simulate = egui::Button::new("Generate and simulate");
                if ui.add_enabled(open && idle, simulate).clicked() {
                    self.generate(true);
                    ui.close();
                }
                let cancel = egui::Button::new("Cancel").shortcut_text("Esc");
                if ui.add_enabled(!idle, cancel).clicked() {
                    self.run.cancel();
                    ui.close();
                }
                ui.separator();
                if ui
                    .add_enabled(open, egui::Button::new("Results…"))
                    .clicked()
                {
                    self.show_results();
                    ui.close();
                }
            });
            ui.menu_button("Tools", |ui| {
                // The tool library opens with or without a project (12.1).
                if ui.button("Tool library…").clicked() {
                    let purpose = if open {
                        super::Purpose::Add
                    } else {
                        super::Purpose::Browse
                    };
                    self.open_tool_library(purpose, None);
                    ui.close();
                }
                // The machine library too (SET-10; 12.5).
                if ui.button("Machine library…").clicked() {
                    self.open_machine_library();
                    ui.close();
                }
                ui.separator();
                let operations = self.overview.step(Step::Operations).is_available();
                if ui
                    .add_enabled(open && operations, egui::Button::new("Select on the part…"))
                    .clicked()
                {
                    self.start_picking();
                    ui.close();
                }
                let setup = self.setup;
                if ui
                    .add_enabled(setup.is_some(), egui::Button::new("Machine profile…"))
                    .clicked()
                    && let Some(setup) = setup
                {
                    self.choose_setup(setup, Some(openchuck_app::overview::SetupPart::Machine));
                    ui.close();
                }
            });
            ui.menu_button("Help", |ui| {
                if ui.button("Keyboard shortcuts…").clicked() {
                    self.open_dialog(Dialog::Keys);
                    ui.close();
                }
                if ui.button("Settings…").clicked() {
                    self.open_dialog(Dialog::Settings);
                    ui.close();
                }
                if ui.button("About OpenChuckCAM…").clicked() {
                    self.open_dialog(Dialog::About);
                    ui.close();
                }
            });
        });
    }

    /// The workflow bar: the five steps on the left, and the actions that
    /// start something new on the right (specification sections 3 and 4).
    /// The right side is laid out first, so the steps never run under it.
    fn workflow_bar(&mut self, ui: &mut egui::Ui) {
        // The bar's colour follows the whole job's health (issue 210): a line
        // along its foot, and the badge at its right end, which also has the
        // shape (UI-11).
        // A number box holding an entry that cannot be read marks the step
        // it is in, and so the whole job, as an error to fix (UI-26).
        let unreadable = crate::field_row::unreadable_entries(ui.ctx());
        let steps = steps_with_unreadable(&self.overview, self.step, &unreadable);
        let first_problem = self.overview.first_problem().is_some();
        let health = worst(&steps);
        let foot = ui.max_rect();
        if self.app.is_some() {
            ui.painter().hline(
                foot.x_range(),
                foot.bottom() - 1.0,
                Stroke::new(2.0, marks::colour_of(health.mark())),
            );
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if self.app.is_some() && self.health_badge(ui, &steps, first_problem) {
                self.go_to_first_problem();
            }
            match self.step {
                Step::Model if self.app.is_some() => {
                    if ui
                        .add_enabled(self.import.is_none(), egui::Button::new("+ Import model…"))
                        .on_hover_text("Import model (Ctrl I)")
                        .clicked()
                    {
                        self.import_model();
                    }
                }
                Step::Setup => {
                    if let Some(app) = &self.app {
                        let blocked = openchuck_app::add_setup_blocked(app.project());
                        let tip = blocked.unwrap_or("New setup");
                        if ui
                            .add_enabled(blocked.is_none(), egui::Button::new("+ Add setup"))
                            .on_disabled_hover_text(tip)
                            .on_hover_text(tip)
                            .clicked()
                        {
                            self.add_setup();
                        }
                    }
                    ui.add(
                        egui::Label::new(
                            RichText::new("Changing a setup marks its operations stale.")
                                .color(DARK.muted),
                        )
                        .truncate(),
                    );
                }
                Step::Operations => {
                    if ui
                        .button("Tool library…")
                        .on_hover_text("Open the tool library to add, edit or import tools")
                        .clicked()
                    {
                        self.open_tool_library(super::Purpose::Add, None);
                    }
                    if ui
                        .button("+ New operation")
                        .on_hover_text("New operation (N)")
                        .clicked()
                    {
                        self.new_operation();
                    }
                    if let Some(Selection::Operation(id)) = self.selected
                        && ui.button("Duplicate").on_hover_text("Ctrl D").clicked()
                    {
                        self.tree_action(super::TreeAction::Duplicate(id));
                    }
                }
                _ => {}
            }
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                egui::ScrollArea::horizontal()
                    .id_salt("steps")
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                    .show(ui, |ui| {
                        ui.horizontal_centered(|ui| {
                            for step in Step::ALL {
                                let state = steps
                                    .iter()
                                    .find(|s| s.step == step)
                                    .cloned()
                                    .unwrap_or_else(|| self.overview.step(step));
                                let active = self.step == step;
                                if step_tab(ui, step, active, &state).clicked() {
                                    self.open_step(step);
                                }
                            }
                            ui.separator();
                            self.g_code_status(ui);
                        });
                    });
            });
        });
    }

    /// The overall status at the right end of the step bar: the shape and
    /// the words of the job's health. Its tooltip names what needs attention
    /// in each step; clicking it goes to the first problem. Returns true when
    /// clicked.
    fn health_badge(&self, ui: &mut egui::Ui, steps: &[StepState], first_problem: bool) -> bool {
        let health = worst(steps);
        let label = health.name();
        let galley = ui.painter().layout_no_wrap(
            label.to_owned(),
            egui::FontId::proportional(13.0),
            DARK.text,
        );
        let size = egui::vec2(galley.size().x + 34.0, WORKFLOW_BAR - 12.0);
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
        let colour = marks::colour_of(health.mark());
        let painter = ui.painter();
        painter.rect(
            rect,
            RADIUS,
            DARK.field,
            Stroke::new(if response.has_focus() { 2.0 } else { 1.5 }, colour),
            egui::StrokeKind::Inside,
        );
        marks::paint(
            painter,
            egui::pos2(rect.left() + 14.0, rect.center().y),
            health.mark(),
            marks::SIZE,
        );
        painter.galley(
            egui::pos2(rect.left() + 26.0, rect.center().y - galley.size().y / 2.0),
            galley,
            DARK.text,
        );
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::Role::Button, true, format!("Overall status: {label}"))
        });
        let lines: Vec<_> = steps
            .iter()
            .filter(|s| s.health != StepHealth::Good)
            .filter_map(|s| Some((s.step, s.health, s.reason.clone()?)))
            .collect();
        let first = first_problem;
        let response = response.on_hover_ui(|ui| {
            ui.strong(format!("Overall status: {label}"));
            for (step, health, reason) in &lines {
                ui.horizontal(|ui| {
                    marks::mark(ui, health.mark(), health.name());
                    ui.label(format!("{}: {reason}", step.name()));
                });
            }
            if first {
                ui.label(RichText::new("Click to go to the first problem").color(DARK.muted));
            }
        });
        response.clicked()
    }

    /// The footer: the job or "Ready", the step's note and the short G-code
    /// notice on the left; units, tolerance, machine warnings and the
    /// step's actions with the primary right-most (specification section 10).
    /// The right side is laid out first; the left side's text is cut to the
    /// room left.
    fn footer(&mut self, ui: &mut egui::Ui) {
        let primary = self.app.as_ref().map(|_| self.overview.primary(self.step));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            match &primary {
                Some(p) if self.step == Step::Simulate => self.simulate_actions(ui, p),
                Some(p) => {
                    let enabled = p.blocked.is_none() && !self.run.is_running();
                    let response = primary_button(ui, &p.label, enabled);
                    let response = match &p.blocked {
                        Some(reason) => response.on_disabled_hover_text(reason),
                        None if self.run.is_running() => response
                            .on_disabled_hover_text("Wait for the running job, or cancel it"),
                        None => response,
                    };
                    if response.clicked() {
                        self.primary();
                    }
                    if !enabled && response.hovered() {
                        self.highlight = p.message;
                    }
                }
                None => {
                    if primary_button(ui, "New project…", true).clicked() {
                        let ctx = ui.ctx().clone();
                        self.request(Next::New, &ctx);
                    }
                }
            }
            if self.run.is_running() && ui.button("Cancel").on_hover_text("Esc").clicked() {
                self.run.cancel();
            }
            ui.separator();
            let warnings = self.overview.machine_warnings.clone();
            let text = match warnings.as_slice() {
                [] => None,
                [one] => Some(one.clone()),
                many => Some(format!("{} machine values unconfirmed", many.len())),
            };
            if let Some(text) = text
                && ui
                    .link(RichText::new(text).color(DARK.warning))
                    .on_hover_text(format!(
                        "{}. Open the machine in Setup.",
                        warnings.join(", ")
                    ))
                    .clicked()
                && let Some(setup) = self.setup
            {
                self.open_step(Step::Setup);
                self.choose_setup(setup, Some(openchuck_app::overview::SetupPart::Machine));
            }
            if let Some(tolerance) = self.footer_tolerance() {
                ui.monospace(format!("Tol {tolerance:.3} mm"));
            }
            ui.label("mm · deg").on_hover_text("Display units");
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                // The current job, with its one percentage and the
                // operation it works on; choosing it opens the Jobs tab, with
                // every job, their history and Cancel (issue 201, the
                // owner's option C).
                let running = self.run.running_text(self.app.as_ref()).or_else(|| {
                    self.jobs.running().first().map(|e| {
                        let percent = e.percent().map(|p| format!(" {p} %")).unwrap_or_default();
                        format!("{}…{percent}", e.label)
                    })
                });
                let text = match running {
                    Some(text) => {
                        ui.spinner();
                        text + &more_jobs(self.jobs.running().len())
                    }
                    None => "Ready".to_owned(),
                };
                if ui
                    .add(egui::Label::new(text).sense(egui::Sense::click()))
                    .on_hover_text("Show the Jobs tab: every job, with its progress and Cancel")
                    .clicked()
                {
                    self.show_jobs();
                }
                ui.separator();
                // D2: the short form; the full notice is at export.
                ui.label("Verify G-code before running it");
                ui.separator();
                if self
                    .workholding_panel
                    .as_ref()
                    .is_some_and(crate::workholding_panel::WorkholdingPanel::has_unapplied_changes)
                {
                    ui.label(RichText::new("Workholding changes not applied").color(DARK.warning));
                    ui.separator();
                }
                let note = primary
                    .as_ref()
                    .and_then(|p| p.blocked.clone())
                    .unwrap_or_else(|| step_note(self.step).to_owned());
                ui.add(egui::Label::new(RichText::new(note).color(DARK.muted)).truncate());
            });
        });
    }

    /// Simulate's two footer actions in fixed places, in workflow order:
    /// the simulation action, then "Post and export…" right-most. Exactly
    /// one is filled, `primary`, the next required step; neither moves or
    /// changes into the other (decision record 0282 item 2). Each is put in
    /// its own place measured from the right edge, and they are made in
    /// reading order, so Tab reaches the simulation action first.
    fn simulate_actions(&mut self, ui: &mut egui::Ui, primary: &PrimaryAction) {
        let actions = [
            self.overview.simulation_action(),
            self.overview.export_action(),
        ];
        let gap = ui.spacing().item_spacing.x;
        let widths = actions.clone().map(|a| {
            let text = egui::WidgetText::from(RichText::new(&a.label).strong()).into_galley(
                ui,
                Some(egui::TextWrapMode::Extend),
                f32::INFINITY,
                egui::TextStyle::Button,
            );
            (text.size().x + 2.0 * ui.spacing().button_padding.x).ceil()
        });
        let (area, _) = ui.allocate_exact_size(
            egui::vec2(widths[0] + gap + widths[1], 28.0),
            egui::Sense::hover(),
        );
        let export =
            egui::Rect::from_min_max(egui::pos2(area.right() - widths[1], area.top()), area.max);
        let simulation =
            egui::Rect::from_min_max(area.min, egui::pos2(export.left() - gap, area.bottom()));
        let running = self.run.is_running();
        let mut clicked = [false; 2];
        for (index, (action, place)) in actions.iter().zip([simulation, export]).enumerate() {
            let enabled = action.blocked.is_none() && !running;
            // A child in the space allocated above: it takes no more room
            // from the footer's row.
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(place)
                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
            );
            let response = if action == primary {
                primary_button(&mut child, &action.label, enabled)
            } else {
                child.add_enabled(
                    enabled,
                    egui::Button::new(&action.label).min_size(egui::vec2(0.0, 28.0)),
                )
            };
            let response = match &action.blocked {
                Some(reason) => response.on_disabled_hover_text(reason),
                None if running => {
                    response.on_disabled_hover_text("Wait for the running job, or cancel it")
                }
                // An enabled action says what it does on focus as well as
                // hover (section 16).
                None => crate::field_row::tip(response, action_tip(action.kind)),
            };
            if !enabled && response.hovered() {
                self.highlight = action.message;
            }
            // The keyboard goes back to "Post and export…" when the dialog
            // it opened closes.
            if index == 1 && self.export_from_footer && !self.export.is_open() {
                self.export_from_footer = false;
                response.request_focus();
            }
            if let Some(click) = clicked.get_mut(index) {
                *click = response.clicked();
            }
        }
        if clicked[0] {
            self.simulation();
        }
        if clicked[1] {
            self.post_and_export();
            self.export_from_footer = self.export.is_open();
        }
    }

    /// The step bar's G-code status, after the steps: whether G-code was
    /// last written, completely, from every enabled operation as it is now
    /// (decision record 0282 item 5). Not a button, and no part of the
    /// health; past write results are in Messages (decision 0240).
    fn g_code_status(&self, ui: &mut egui::Ui) {
        let written = self.app.is_some() && self.overview.g_code_written();
        let (label, tip) = if written {
            (
                "G-code written",
                "G-code was written from the project as it is now",
            )
        } else if self.overview.wrote_g_code_before() {
            (
                "G-code not written",
                "The project has changed since G-code was last written",
            )
        } else {
            (
                "G-code not written",
                "No G-code has been written in this session",
            )
        };
        let font = egui::FontId::proportional(13.0);
        let galley = ui
            .painter()
            .layout_no_wrap(label.to_owned(), font, Color32::PLACEHOLDER);
        let size = egui::vec2(galley.size().x + 40.0, WORKFLOW_BAR - 4.0);
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::hover());
        let painter = ui.painter();
        let centre = egui::pos2(rect.left() + 18.0, rect.center().y);
        let text = if written { DARK.ok } else { DARK.muted };
        if written {
            // The ring the fifth step had, round a tick.
            painter.circle_stroke(centre, 9.0, Stroke::new(2.0, DARK.ok));
            painter.line(
                vec![
                    centre + egui::vec2(-4.0, 0.0),
                    centre + egui::vec2(-1.0, 3.0),
                    centre + egui::vec2(4.5, -3.5),
                ],
                Stroke::new(2.0, DARK.ok),
            );
        } else {
            painter.circle_stroke(centre, 9.0, Stroke::new(1.0, DARK.muted));
        }
        painter.galley(
            egui::pos2(rect.left() + 32.0, rect.center().y - galley.size().y / 2.0),
            galley,
            text,
        );
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, label));
        response.on_hover_text(format!(
            "{label}: {tip}.\nPast write results are in Messages."
        ));
    }

    /// The tolerance of the selected operation, for the footer.
    fn footer_tolerance(&self) -> Option<f64> {
        let app = self.app.as_ref()?;
        match self.selected? {
            Selection::Operation(id) => app.operation_tolerance(id),
            _ => None,
        }
    }

    /// The empty state: a card in the view with the next things to do
    /// (specification section 11.1). Recent projects and dropping a file are
    /// UI-23 (P2).
    fn empty_state(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let rect = ui.max_rect();
        ui.painter()
            .rect_filled(rect, 0.0, DARK.bar.gamma_multiply(0.6));
        let card = egui::Rect::from_center_size(rect.center(), egui::vec2(420.0, 300.0))
            .intersect(rect.shrink(16.0));
        ui.scope_builder(egui::UiBuilder::new().max_rect(card), |ui| {
            egui::Frame::group(ui.style())
                .corner_radius(6.0)
                .inner_margin(16.0)
                .show(ui, |ui| {
                    ui.label(RichText::new("Start with a model").size(18.0).strong());
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if self.app.is_some() {
                            if primary_button(ui, "Import model…", self.import.is_none()).clicked()
                            {
                                self.import_model();
                            }
                        } else if primary_button(ui, "New project…", true).clicked() {
                            self.request(Next::New, &ctx);
                        }
                        if ui.button("Open project…").clicked() {
                            self.request(Next::Open, &ctx);
                        }
                    });
                    ui.add_space(12.0);
                    for step in Step::ALL {
                        ui.label(
                            RichText::new(format!(
                                "{}  {} — {}",
                                step.number(),
                                step.name(),
                                step_purpose(step)
                            ))
                            .color(DARK.muted),
                        );
                    }
                });
        });
    }
}

/// What the status bar adds after the current job when others run too:
/// " (and 1 more job)". Nothing when it is the only one.
fn more_jobs(running: usize) -> String {
    match running.saturating_sub(1) {
        0 => String::new(),
        1 => " (and 1 more job)".to_owned(),
        n => format!(" (and {n} more jobs)"),
    }
}

fn bar_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(DARK.bar)
        .inner_margin(egui::Margin::symmetric(8, 0))
}

/// One step of the workflow bar: a numbered circle and the name, an accent
/// underline when active, a green ring when complete, dimmed with its reason
/// when unavailable (specification section 4).
/// The five steps' states with `step` marked as an error to fix while number
/// boxes in it hold entries that cannot be read (UI-26).
fn steps_with_unreadable(overview: &Overview, step: Step, unreadable: &[String]) -> Vec<StepState> {
    let mut steps = overview.steps.clone();
    if !unreadable.is_empty()
        && let Some(state) = steps.iter_mut().find(|s| s.step == step)
    {
        state.health = StepHealth::Error;
        state.reason = Some(format!("Fix {}: it cannot be read", unreadable.join(", ")));
    }
    steps
}

/// The worst health of `steps`, as the whole job's.
fn worst(steps: &[StepState]) -> StepHealth {
    steps
        .iter()
        .map(|s| s.health)
        .max()
        .unwrap_or(StepHealth::Attention)
}

fn step_tab(
    ui: &mut egui::Ui,
    step: Step,
    active: bool,
    state: &openchuck_app::overview::StepState,
) -> egui::Response {
    let available = state.is_available();
    let font = egui::FontId::proportional(13.0);
    let galley =
        ui.painter()
            .layout_no_wrap(step.name().to_owned(), font.clone(), Color32::PLACEHOLDER);
    let size = egui::vec2(galley.size().x + 68.0, WORKFLOW_BAR - 4.0);
    let sense = if available {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(size, sense);
    let text = if active {
        ui.visuals().strong_text_color()
    } else if available {
        DARK.muted
    } else {
        DARK.muted.gamma_multiply(0.45)
    };
    let painter = ui.painter();
    let centre = egui::pos2(rect.left() + 22.0, rect.center().y);
    if active {
        painter.circle_filled(centre, 10.0, DARK.accent);
    } else if state.complete {
        painter.circle_stroke(centre, 10.0, Stroke::new(2.0, DARK.ok));
    } else {
        painter.circle_stroke(centre, 10.0, Stroke::new(1.0, text));
    }
    if active && state.complete {
        painter.circle_stroke(centre, 11.5, Stroke::new(2.0, DARK.ok));
    }
    painter.text(
        centre,
        egui::Align2::CENTER_CENTER,
        step.number().to_string(),
        egui::FontId::proportional(11.0),
        if active { Color32::WHITE } else { text },
    );
    painter.galley(
        egui::pos2(rect.left() + 38.0, rect.center().y - galley.size().y / 2.0),
        galley,
        text,
    );
    if active {
        painter.hline(
            rect.x_range(),
            rect.bottom() - 1.0,
            Stroke::new(2.0, DARK.accent),
        );
    }
    // The step's own health, by shape as well as colour (issue 210; UI-11).
    // A step out of reach shows none: its reason is the tooltip.
    if available {
        marks::paint(
            painter,
            egui::pos2(rect.right() - 10.0, rect.center().y),
            state.health.mark(),
            marks::SIZE,
        );
    }
    let name = step.name();
    let health = state.health.name();
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::Role::Button,
            available,
            active,
            format!("{name}, {health}"),
        )
    });
    let mut tip = match &state.unavailable {
        Some(reason) => format!("{name}: {reason}"),
        None if state.complete => format!("{name} (Alt {}): complete", step.number()),
        None => format!("{name} (Alt {})", step.number()),
    };
    if available {
        tip.push_str(&format!("\n{health}"));
        if let Some(reason) = &state.reason {
            tip.push_str(&format!(": {reason}"));
        }
    }
    response.on_hover_text(tip)
}

/// What each step is for, for the empty state's list.
fn step_purpose(step: Step) -> &'static str {
    match step {
        Step::Model => "import, check units and mesh, place on the rotary axis",
        Step::Setup => "machine, stock, part position, work offset, clearance",
        Step::Operations => "select geometry, add operations, generate",
        Step::Simulate => "play the cut and compare stock with model; then post and export",
    }
}

/// What a footer action does, for its tooltip (section 16).
fn action_tip(kind: openchuck_app::overview::PrimaryKind) -> &'static str {
    use openchuck_app::overview::PrimaryKind;
    match kind {
        PrimaryKind::OpenExport => "Open the export dialog (Ctrl E or Alt 5)",
        PrimaryKind::RunSimulation => "Simulate the cut",
        PrimaryKind::GenerateAndSimulate(_) => "Generate the operation, then simulate the cut",
        PrimaryKind::GenerateStale(_) => "Generate the stale operations",
        PrimaryKind::Generate => "Generate the toolpaths; the stock simulation is skipped",
        PrimaryKind::ImportModel | PrimaryKind::Continue(_) => "",
    }
}

/// The footer's short note about the step (specification section 10).
fn step_note(step: Step) -> &'static str {
    match step {
        Step::Model => "Placement steps run in order; undo with Ctrl Z",
        Step::Setup | Step::Operations => {
            "Enter confirms; leaving a form asks about unconfirmed changes; undo with Ctrl Z"
        }
        Step::Simulate => "Results show the latest simulation",
    }
}

/// The view's step for a workflow step, one to one (decision record 0119).
fn view_step(step: Step) -> ViewStep {
    match step {
        Step::Model => ViewStep::Model,
        Step::Setup => ViewStep::Setup,
        Step::Operations => ViewStep::Operations,
        Step::Simulate => ViewStep::Simulate,
    }
}

/// How a recent project is listed: its folder's name, or the whole path for
/// a folder with no name (such as a drive root).
pub(super) fn recent_label(folder: &std::path::Path) -> String {
    folder.file_name().map_or_else(
        || super::checkpoints::shown(folder),
        |name| name.to_string_lossy().into_owned(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn req_ui_09_the_status_bar_says_when_more_jobs_run() {
        // Issue 201: the status bar shows one job; the others are counted,
        // and listed in the Jobs tab.
        assert_eq!(more_jobs(0), "");
        assert_eq!(more_jobs(1), "");
        assert_eq!(more_jobs(2), " (and 1 more job)");
        assert_eq!(more_jobs(3), " (and 2 more jobs)");
    }
}
