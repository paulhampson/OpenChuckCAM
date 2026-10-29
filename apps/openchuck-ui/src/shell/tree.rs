// SPDX-License-Identifier: AGPL-3.0-or-later
//! The project panel: one tree in every step (UI-06), with each item's
//! status mark and its pinned legend (UI-11; specification section 6).
//!
//! Groups in pipeline order: Model, Setups (with the machine profile, stock,
//! part position, work offset and clearance, and workholding as child rows),
//! Selection sets and Operations. Each header has a count and a "+". Tools
//! are not in the tree (UI-06; decision record 0127): an operation's tool is
//! in its inspector, and the tool library window lists and manages tools.
//! Rows are 28 high: a drag handle and an enable box for operations, the
//! status mark, an icon or swatch, the name and a chip. A right click (or
//! the menu key) opens the row's menu.

use eframe::egui::{self, Color32, RichText, Stroke};
use openchuck_app::overview::{ItemRef, ItemState, SetupPart, Step, setup_row};
use openchuck_app::project::ItemId;

use super::{Command, Selection, Shell, TreeAction, marks};
use crate::theme::DARK;

/// The height of a row (specification section 6).
const ROW: f32 = 28.0;
/// The indent of a child row.
const INDENT: f32 = 14.0;

/// A group of the tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Group {
    Model,
    Setups,
    SelectionSets,
    Operations,
}

impl Group {
    fn title(self) -> &'static str {
        match self {
            Self::Model => "MODEL",
            Self::Setups => "SETUPS",
            Self::SelectionSets => "SELECTION SETS",
            Self::Operations => "OPERATIONS",
        }
    }

    /// The "+" button's tooltip, or `None` when the group has no "+" yet.
    /// The Setups "+" creates the project's one setup, once there is a model
    /// (decision record 0144); a second is SET-07 (P2).
    fn add(self) -> Option<&'static str> {
        match self {
            Self::Setups => Some("New setup"),
            Self::Model => Some("Import a model (Ctrl I)"),
            Self::SelectionSets => Some("New selection set: pick on the part"),
            Self::Operations => Some("New operation (N)"),
        }
    }
}

/// What one row shows.
struct Row<'a> {
    indent: usize,
    selected: bool,
    status: Option<(ItemState, &'a str, Option<f32>)>,
    swatch: Option<Color32>,
    name: RichText,
    chip: Option<String>,
}

/// Draws a row and returns its response; `before` draws the operation's
/// handle and enable box first.
fn row(ui: &mut egui::Ui, r: Row<'_>, before: impl FnOnce(&mut egui::Ui)) -> egui::Response {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, ROW), egui::Sense::click());
    let painter = ui.painter();
    if r.selected {
        painter.rect_filled(rect, 0.0, DARK.selected);
        painter.vline(
            rect.left() + 1.0,
            rect.y_range(),
            Stroke::new(2.0, DARK.accent),
        );
    } else if response.hovered() {
        painter.rect_filled(rect, 0.0, ui.visuals().widgets.hovered.weak_bg_fill);
    }
    #[allow(clippy::cast_precision_loss)]
    let inner = rect.with_min_x(rect.left() + 8.0 + INDENT * r.indent as f32);
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(inner)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    child.spacing_mut().item_spacing.x = 6.0;
    before(&mut child);
    if let Some((state, cause, _)) = r.status {
        marks::mark(&mut child, state, cause);
    } else {
        child.add_space(marks::SIZE + 4.0);
    }
    if let Some(colour) = r.swatch {
        let (sw, _) = child.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
        child.painter().rect_filled(sw, 2.0, colour);
    }
    child.add(egui::Label::new(r.name).truncate().selectable(false));
    if let Some(chip) = r.chip {
        child.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            super::frame::chip(ui, &chip);
        });
    }
    if let Some((ItemState::Running, _, Some(fraction))) = r.status {
        marks::progress_bar(ui, rect, fraction);
    }
    response
}

/// A group's header: its title, count and "+" (22 pixels).
fn header(ui: &mut egui::Ui, group: Group, count: usize, enabled: bool) -> bool {
    header_with(ui, group, count, enabled, None, |_| {})
}

/// A header whose "+" says `disabled_tip` while it is disabled.
fn header_with(
    ui: &mut egui::Ui,
    group: Group,
    count: usize,
    enabled: bool,
    disabled_tip: Option<&str>,
    extra: impl FnOnce(&mut egui::Ui),
) -> bool {
    let mut add = false;
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(group.title())
                .small()
                .strong()
                .color(DARK.muted),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if let Some(tip) = group.add() {
                let button = egui::Button::new("+").min_size(egui::vec2(22.0, 22.0));
                let tip = disabled_tip.filter(|_| !enabled).unwrap_or(tip);
                let response = ui.add_enabled(enabled, button);
                let response = response.on_disabled_hover_text(tip).on_hover_text(tip);
                if response.clicked() {
                    add = true;
                }
            }
            extra(ui);
            ui.label(RichText::new(count.to_string()).color(DARK.muted));
        });
    });
    add
}

/// The line under the operations (11.5, for every step as decision record
/// 0169 proposes).
pub(super) const HIDING_NOTE: &str = "Hiding a toolpath changes the view only. The toolpaths, the simulation and the G-code do not change.";

/// The tooltip of an operation's eye button (issue 213).
pub(super) const EYE_TIP: &str =
    "Show or hide this operation's toolpath in the view. Nothing it makes changes";

/// The visibility eye of an operation's toolpath (issue 213; decision
/// record 0169): an open eye while the path is shown, the same eye struck
/// through while it is hidden, so the state reads without colour (UI-11).
/// It is a button the keyboard reaches (Tab, then Space or Enter), and says
/// its state to a screen reader.
pub(super) fn eye(ui: &mut egui::Ui, shown: bool) -> egui::Response {
    let response = eye_shape(ui, shown, "Show toolpath");
    response
        .on_hover_text(EYE_TIP)
        .on_hover_text(if shown { "Shown" } else { "Hidden" })
}

/// The eye drawn as a button named `name`: open for `shown`, struck through
/// otherwise.
fn eye_shape(ui: &mut egui::Ui, shown: bool, name: &str) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(20.0, 18.0), egui::Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::CheckBox, ui.is_enabled(), shown, name)
    });
    let visuals = ui.style().interact(&response);
    let painter = ui.painter_at(rect.expand(1.0));
    let stroke = Stroke::new(1.4, visuals.fg_stroke.color);
    let c = rect.center();
    // The eye's outline: two arcs meeting at its corners.
    let outline: Vec<egui::Pos2> = (0..=16)
        .map(|i| {
            let t = std::f32::consts::PI * i as f32 / 16.0;
            egui::pos2(c.x - 7.0 * t.cos(), c.y - 4.5 * t.sin())
        })
        .chain((0..=16).map(|i| {
            let t = std::f32::consts::PI * i as f32 / 16.0;
            egui::pos2(c.x + 7.0 * t.cos(), c.y + 4.5 * t.sin())
        }))
        .collect();
    painter.add(egui::Shape::closed_line(outline, stroke));
    if shown {
        painter.circle_filled(c, 2.4, stroke.color);
    } else {
        painter.circle_stroke(c, 2.4, stroke);
        painter.line_segment(
            [c + egui::vec2(-7.5, 6.0), c + egui::vec2(7.5, -6.0)],
            Stroke::new(1.8, stroke.color),
        );
    }
    if response.has_focus() {
        painter.rect_stroke(
            rect,
            2.0,
            Stroke::new(1.0, DARK.accent),
            egui::StrokeKind::Inside,
        );
    }
    response
}

/// "Show all" and "Hide all" for the operations' toolpaths, as an open and a
/// struck-through eye in the group's header (issue 213). Returns the choice
/// clicked.
fn all_eyes(ui: &mut egui::Ui, hidden: usize, count: usize) -> Option<bool> {
    let mut chosen = None;
    // Right to left: "Hide all" is drawn first, so "Show all" is on its left.
    for (shown, name, enabled, done) in [
        (
            false,
            "Hide all toolpaths",
            hidden < count,
            "Every toolpath is hidden",
        ),
        (
            true,
            "Show all toolpaths",
            hidden > 0,
            "Every toolpath is shown",
        ),
    ] {
        let response = ui
            .add_enabled_ui(enabled, |ui| eye_shape(ui, shown, name))
            .inner
            .on_hover_text(format!("{name} in the view"))
            .on_disabled_hover_text(done);
        if response.clicked() {
            chosen = Some(shown);
        }
    }
    chosen
}

/// A "None yet" row for an empty group (specification section 11.1).
/// The chip of a model's Placement row: how many placement steps it has.
fn placement_chip(steps: usize) -> String {
    match steps {
        0 => "as in file".to_owned(),
        1 => "1 step".to_owned(),
        n => format!("{n} steps"),
    }
}

fn none_yet(ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.add_space(8.0 + INDENT);
        ui.label(RichText::new("None yet").color(DARK.muted));
    });
}

impl Shell {
    /// The project panel: the tree, and the legend pinned under it.
    #[allow(clippy::too_many_lines)]
    pub(super) fn tree(&mut self, ui: &mut egui::Ui) {
        egui::Panel::bottom("tree legend")
            .resizable(false)
            .show(ui, marks::legend);
        let Some(app) = &self.app else {
            ui.label("No project open. Use File, Open project or New project.");
            for group in [
                Group::Model,
                Group::Setups,
                Group::SelectionSets,
                Group::Operations,
            ] {
                header(ui, group, 0, false);
                none_yet(ui);
            }
            return;
        };
        let project = app.project();
        let overview = &self.overview;
        let status = |item: ItemRef| {
            overview
                .status(item)
                .map(|s| (s.state, s.cause.as_str(), s.progress))
        };
        let mut actions: Vec<TreeAction> = Vec::new();
        egui::ScrollArea::vertical()
            .id_salt("project tree")
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                // Models.
                if header(ui, Group::Model, project.models().len(), true) {
                    actions.push(TreeAction::Add(Group::Model));
                }
                if project.models().is_empty() {
                    none_yet(ui);
                }
                for m in project.models() {
                    let item = ItemRef::Model(m.id);
                    let chosen = self.selected == Some(Selection::Model(m.id));
                    let response = row(
                        ui,
                        Row {
                            indent: 1,
                            selected: chosen && self.model_section.is_none(),
                            status: status(item),
                            swatch: None,
                            name: RichText::new(m.path.as_str()),
                            chip: None,
                        },
                        |_| {},
                    );
                    if response.clicked() {
                        actions.push(TreeAction::SelectModel(m.id, None));
                    }
                    item_menu(&response, item, None, &mut actions);
                    // Its placement (shared by every setup) and its mesh
                    // check, as child rows (section 6; issue #181).
                    for (key, label, chip, mark) in [
                        (
                            crate::model_panel::PLACEMENT,
                            "Placement",
                            Some(placement_chip(m.transform.len())),
                            None,
                        ),
                        (
                            crate::model_panel::MESH_CHECK,
                            "Mesh check",
                            None,
                            status(item),
                        ),
                    ] {
                        let response = row(
                            ui,
                            Row {
                                indent: 2,
                                selected: chosen && self.model_section == Some(key),
                                status: mark,
                                swatch: None,
                                name: RichText::new(label).color(DARK.muted),
                                chip,
                            },
                            |_| {},
                        );
                        let response = if key == crate::model_panel::PLACEMENT {
                            response.on_hover_text(openchuck_app::setup_form::PLACEMENT_EXPLAINED)
                        } else {
                            response
                        };
                        if response.clicked() {
                            actions.push(TreeAction::SelectModel(m.id, Some(key)));
                        }
                    }
                }
                // Setups, with their parts as child rows.
                let blocked = openchuck_app::add_setup_blocked(project);
                if header_with(
                    ui,
                    Group::Setups,
                    project.setups().len(),
                    blocked.is_none(),
                    blocked,
                    |_| {},
                ) {
                    actions.push(TreeAction::Add(Group::Setups));
                }
                if project.setups().is_empty() {
                    none_yet(ui);
                }
                for s in project.setups() {
                    let item = ItemRef::Setup(s.id);
                    let chosen = self.selected == Some(Selection::Setup(s.id));
                    let machine = project.machine().map(|m| m.name.clone());
                    let name = match machine {
                        Some(machine) => format!("{} · {machine}", s.name),
                        None => s.name.clone(),
                    };
                    let response = row(
                        ui,
                        Row {
                            indent: 1,
                            selected: chosen && self.setup_section.is_none(),
                            status: status(item),
                            swatch: None,
                            name: RichText::new(name).strong(),
                            chip: Some(s.frame.work_offset.register.to_string()),
                        },
                        |_| {},
                    );
                    if response.clicked() {
                        actions.push(TreeAction::SelectSetup(s.id, None));
                    }
                    item_menu(&response, item, None, &mut actions);
                    for part in SetupPart::ALL {
                        let r = setup_row(project, s, part);
                        let part_item = ItemRef::SetupPart(s.id, part);
                        let chip = if part == SetupPart::Machine {
                            None
                        } else {
                            r.chip
                        };
                        let response = row(
                            ui,
                            Row {
                                indent: 2,
                                selected: chosen && self.setup_section == Some(part),
                                status: status(part_item),
                                swatch: None,
                                name: RichText::new(r.label).color(DARK.muted),
                                chip,
                            },
                            |_| {},
                        );
                        if response.clicked() {
                            actions.push(TreeAction::SelectSetup(s.id, Some(part)));
                        }
                    }
                }
                // Selection sets.
                let summaries = app.selection_summaries(&[]);
                if header(ui, Group::SelectionSets, summaries.len(), true) {
                    actions.push(TreeAction::Add(Group::SelectionSets));
                }
                if summaries.is_empty() {
                    none_yet(ui);
                }
                for (index, s) in summaries.iter().enumerate() {
                    let item = ItemRef::SelectionSet(s.id);
                    let [r, g, b] = crate::selection_panel::set_rgb(s.colour.as_deref(), index);
                    let response = row(
                        ui,
                        Row {
                            indent: 1,
                            selected: self.selected == Some(Selection::Set(s.id)),
                            status: status(item),
                            swatch: Some(Color32::from_rgb(r, g, b)),
                            name: RichText::new(&s.name),
                            chip: Some(format!("{} {}", s.count, s.kind)),
                        },
                        |_| {},
                    );
                    if response.clicked() {
                        actions.push(TreeAction::SelectSet(s.id));
                    }
                    item_menu(&response, item, None, &mut actions);
                }
                // Operations, in machining order.
                let count = project.operations().len();
                // Show all and Hide all, for the whole group (issue 213).
                let ids: Vec<_> = project.operations().iter().map(|o| o.id).collect();
                let hidden = ids.iter().filter(|id| !project.path_shown(**id)).count();
                let mut all = None;
                if header_with(ui, Group::Operations, count, true, None, |ui| {
                    if count > 0 {
                        all = all_eyes(ui, hidden, count);
                    }
                }) {
                    actions.push(TreeAction::Add(Group::Operations));
                }
                if let Some(shown) = all {
                    actions.push(TreeAction::Command(Box::new(Command::SetPathsShown {
                        operations: ids.clone(),
                        shown,
                    })));
                }
                if count == 0 {
                    none_yet(ui);
                }
                for (index, o) in project.operations().iter().enumerate() {
                    let item = ItemRef::Operation(o.id);
                    let shown = project.path_shown(o.id);
                    let mut name = RichText::new(format!("{} {}", index + 1, o.name));
                    if !o.enabled {
                        // D5: struck through and muted.
                        name = name.strikethrough().color(DARK.muted);
                    }
                    let tool = o
                        .tool
                        .and_then(|id| project.tools().iter().find(|t| t.id == id))
                        .map(|t| format!("T{}", t.controller.number));
                    let mut enabled = o.enabled;
                    let mut toggled = false;
                    let id = o.id;
                    let response = row(
                        ui,
                        Row {
                            indent: 1,
                            selected: self.selected == Some(Selection::Operation(id)),
                            status: status(item),
                            swatch: None,
                            name,
                            chip: tool,
                        },
                        |ui| {
                            let handle = ui
                                .dnd_drag_source(
                                    egui::Id::unique(("operation drag", id)),
                                    index,
                                    |ui| {
                                        ui.label(RichText::new("\u{2630}").color(DARK.muted));
                                    },
                                )
                                .response;
                            #[cfg(test)]
                            ui.data_mut(|data| {
                                data.insert_temp(
                                    egui::Id::unique(("operation reorder", id)),
                                    handle.rect,
                                );
                            });
                            handle.on_hover_text("Drag to move earlier or later (Alt ↑, Alt ↓)");
                            let enable = ui
                                .checkbox(&mut enabled, "")
                                .on_hover_text("Include in generation, simulation and G-code");
                            toggled = enable.changed();
                            let eye = eye(ui, shown);
                            #[cfg(test)]
                            ui.data_mut(|data| {
                                data.insert_temp(
                                    egui::Id::unique(("operation enable", id)),
                                    enable.rect,
                                );
                                data.insert_temp(egui::Id::unique(("operation eye", id)), eye.rect);
                            });
                            if eye.clicked() {
                                actions.push(TreeAction::Command(Box::new(
                                    Command::SetPathsShown {
                                        operations: vec![id],
                                        shown: !shown,
                                    },
                                )));
                            }
                        },
                    );
                    #[cfg(test)]
                    ui.data_mut(|data| {
                        data.insert_temp(
                            egui::Id::unique(("operation row focus", id)),
                            response.id,
                        );
                    });
                    if toggled {
                        actions.push(TreeAction::Command(Box::new(
                            Command::SetOperationEnabled {
                                operation: id,
                                enabled,
                            },
                        )));
                    }
                    if let Some(from) = response.dnd_release_payload::<usize>()
                        && *from != index
                        && let Some(dragged) = project.operations().get(*from)
                    {
                        actions.push(TreeAction::Command(Box::new(Command::Move {
                            id: ItemId::Operation(dragged.id),
                            index,
                        })));
                    }
                    if response.dnd_hover_payload::<usize>().is_some() {
                        ui.painter().hline(
                            response.rect.x_range(),
                            response.rect.top(),
                            Stroke::new(2.0, DARK.accent),
                        );
                    }
                    if response.clicked()
                        || (response.has_focus()
                            && ui.input_mut(|i| {
                                i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
                            }))
                    {
                        actions.push(TreeAction::Select(id));
                    }
                    item_menu(
                        &response,
                        item,
                        Some((o.enabled, index, count)),
                        &mut actions,
                    );
                }
                // The line under the tree on Simulate (11.5); elsewhere the
                // eye's tooltip says the same.
                if count > 0 && self.step == Step::Simulate {
                    ui.add_space(4.0);
                    ui.label(RichText::new(HIDING_NOTE).small().color(DARK.muted));
                }
            });
        for action in actions {
            self.tree_action(action);
        }
    }
}

/// The row's menu (right click or the menu key): Rename, Duplicate, Enable
/// or Disable, Delete, Show in view, and Generate (specification section 6).
/// `operation` is the operation's enabled state, place and the count.
fn item_menu(
    response: &egui::Response,
    item: ItemRef,
    operation: Option<(bool, usize, usize)>,
    actions: &mut Vec<TreeAction>,
) {
    response.context_menu(|ui| {
        if !matches!(item, ItemRef::Model(_)) && ui.button("Rename…").clicked() {
            actions.push(TreeAction::Rename(item));
            ui.close();
        }
        if let (ItemRef::Operation(id), Some((enabled, index, count))) = (item, operation) {
            if ui.button("Duplicate").clicked() {
                actions.push(TreeAction::Duplicate(id));
                ui.close();
            }
            let toggle = if enabled { "Disable" } else { "Enable" };
            if ui.button(toggle).clicked() {
                actions.push(TreeAction::Command(Box::new(
                    Command::SetOperationEnabled {
                        operation: id,
                        enabled: !enabled,
                    },
                )));
                ui.close();
            }
            if ui
                .add_enabled(index > 0, egui::Button::new("Move earlier"))
                .clicked()
            {
                actions.push(TreeAction::Command(Box::new(Command::Move {
                    id: ItemId::Operation(id),
                    index: index - 1,
                })));
                ui.close();
            }
            if ui
                .add_enabled(index + 1 < count, egui::Button::new("Move later"))
                .clicked()
            {
                actions.push(TreeAction::Command(Box::new(Command::Move {
                    id: ItemId::Operation(id),
                    index: index + 1,
                })));
                ui.close();
            }
            if ui.button("Delete").clicked() {
                actions.push(TreeAction::Delete(id));
                ui.close();
            }
        }
        if let ItemRef::SelectionSet(id) = item
            && ui
                .button("Delete")
                .on_hover_text("Delete this set; refused while an operation uses it (Del)")
                .clicked()
        {
            actions.push(TreeAction::DeleteSet(id));
            ui.close();
        }
        if ui.button("Show in view").clicked() {
            actions.push(TreeAction::ShowInView(item));
            ui.close();
        }
        if let ItemRef::Operation(id) = item
            && ui
                .button("Generate")
                .on_hover_text("Generate up to and including this operation (G)")
                .clicked()
        {
            actions.push(TreeAction::Generate(id));
            ui.close();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One frame of `draw` in a bare context, with `events`.
    fn frame(ctx: &egui::Context, events: Vec<egui::Event>, draw: impl Fn(&mut egui::Ui)) {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(200.0, 100.0),
            )),
            events,
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| draw(ui));
        output.textures_delta.clear();
    }

    #[test]
    fn req_ui_11_req_ui_15_the_eye_is_a_button_the_mouse_and_the_keyboard_press() {
        // Issue 213: a click and Space on the focused eye both press it; the
        // two states are drawn differently (a stroke more when hidden), so
        // they read without colour.
        let ctx = egui::Context::default();
        let rect = std::cell::Cell::new(egui::Rect::NOTHING);
        let clicks = std::cell::Cell::new(0);
        let draw = |ui: &mut egui::Ui| {
            let r = eye(ui, true);
            rect.set(r.rect);
            if r.clicked() {
                clicks.set(clicks.get() + 1);
            }
        };
        frame(&ctx, Vec::new(), draw);
        let at = rect.get().center();
        let button = |pressed| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        frame(&ctx, vec![egui::Event::PointerMoved(at)], draw);
        frame(&ctx, vec![button(true)], draw);
        frame(&ctx, vec![button(false)], draw);
        assert_eq!(clicks.get(), 1);
        // The keyboard: Tab to it, then Space.
        let key = |key| egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        frame(&ctx, vec![key(egui::Key::Tab)], draw);
        frame(&ctx, vec![key(egui::Key::Space)], draw);
        frame(&ctx, Vec::new(), draw);
        assert_eq!(clicks.get(), 2, "Space presses the focused eye");
        // Shown and hidden are different shapes.
        let shapes = |shown| {
            let ctx = egui::Context::default();
            let input = egui::RawInput::default();
            let mut output = ctx.run_ui(input, |ui| {
                eye(ui, shown);
            });
            output.textures_delta.clear();
            output.shapes.len()
        };
        assert!(shapes(false) > shapes(true));
    }

    #[test]
    fn req_sim_01_show_all_and_hide_all_say_which_was_pressed_and_rest_when_done() {
        // Issue 213: "Show all" is on the left of "Hide all"; each is
        // disabled when there is nothing for it to do.
        let ctx = egui::Context::default();
        let rects = std::cell::RefCell::new(Vec::new());
        let chosen = std::cell::Cell::new(None);
        let hidden = std::cell::Cell::new(1);
        let draw = |ui: &mut egui::Ui| {
            ui.horizontal(|ui| {
                let at = ui.cursor().min;
                let gap = ui.spacing().item_spacing.x;
                if let Some(c) = all_eyes(ui, hidden.get(), 3) {
                    chosen.set(Some(c));
                }
                rects.borrow_mut().push((at, gap));
            });
        };
        frame(&ctx, Vec::new(), draw);
        // Drawn in order: Hide all first, then Show all (in the header's
        // right-to-left row that puts Show all on the left).
        let (at, gap) = rects.borrow()[0];
        let hide = at + egui::vec2(10.0, 9.0);
        let show = at + egui::vec2(30.0 + gap, 9.0);
        let click = |at: egui::Pos2| {
            let button = |pressed| egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            };
            frame(&ctx, vec![egui::Event::PointerMoved(at)], draw);
            frame(&ctx, vec![button(true)], draw);
            frame(&ctx, vec![button(false)], draw);
        };
        click(hide);
        assert_eq!(chosen.take(), Some(false));
        click(show);
        assert_eq!(chosen.take(), Some(true));
        // All hidden: Hide all does nothing.
        hidden.set(3);
        click(hide);
        assert_eq!(chosen.take(), None);
        // None hidden: Show all does nothing.
        hidden.set(0);
        click(show);
        assert_eq!(chosen.take(), None);
    }
}
