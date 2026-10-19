// SPDX-License-Identifier: AGPL-3.0-or-later
//! The project panel: one tree in every step (UI-06), with each item's
//! status mark and its pinned legend (UI-11; specification section 6).
//!
//! Groups in pipeline order: Model, Setups (with the machine profile, stock,
//! part position, work offset and clearance, and workholding as child rows),
//! Selection sets, Tools and Operations. Each header has a count and a "+".
//! Rows are 28 high: a drag handle and an enable box for operations, the
//! status mark, an icon or swatch, the name and a chip. A right click (or
//! the menu key) opens the row's menu.

use eframe::egui::{self, Color32, RichText, Stroke};
use openchuck_app::overview::{ItemRef, ItemState, SetupPart, setup_row};
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
    Tools,
    Operations,
}

impl Group {
    fn title(self) -> &'static str {
        match self {
            Self::Model => "MODEL",
            Self::Setups => "SETUPS",
            Self::SelectionSets => "SELECTION SETS",
            Self::Tools => "TOOLS",
            Self::Operations => "OPERATIONS",
        }
    }

    /// The "+" button's tooltip, or `None` when the group has no "+" yet: a
    /// second setup is SET-07 (P2), and the tool library window is lane X9's.
    fn add(self) -> Option<&'static str> {
        match self {
            Self::Model => Some("Import a model (Ctrl I)"),
            Self::SelectionSets => Some("New selection set: pick on the part"),
            Self::Operations => Some("New operation (N)"),
            Self::Setups | Self::Tools => None,
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
                if ui.add_enabled(enabled, button).on_hover_text(tip).clicked() {
                    add = true;
                }
            }
            ui.label(RichText::new(count.to_string()).color(DARK.muted));
        });
    });
    add
}

/// A "None yet" row for an empty group (specification section 11.1).
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
                Group::Tools,
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
                    let response = row(
                        ui,
                        Row {
                            indent: 1,
                            selected: self.selected == Some(Selection::Model(m.id)),
                            status: status(item),
                            swatch: None,
                            name: RichText::new(m.path.as_str()),
                            chip: None,
                        },
                        |_| {},
                    );
                    if response.clicked() {
                        actions.push(TreeAction::SelectModel(m.id));
                    }
                    item_menu(&response, item, None, &mut actions);
                }
                // Setups, with their parts as child rows.
                if header(ui, Group::Setups, project.setups().len(), true) {
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
                // Tools.
                if header(ui, Group::Tools, project.tools().len(), true) {
                    actions.push(TreeAction::Add(Group::Tools));
                }
                if project.tools().is_empty() {
                    none_yet(ui);
                }
                for t in project.tools() {
                    let item = ItemRef::Tool(t.id);
                    let response = row(
                        ui,
                        Row {
                            indent: 1,
                            selected: self.selected == Some(Selection::Tool(t.id)),
                            status: None,
                            swatch: None,
                            name: RichText::new(format!("T{}  {}", t.controller.number, t.name)),
                            chip: None,
                        },
                        |_| {},
                    );
                    if response.clicked() {
                        actions.push(TreeAction::SelectTool(t.id));
                    }
                    item_menu(&response, item, None, &mut actions);
                }
                // Operations, in machining order.
                let count = project.operations().len();
                if header(ui, Group::Operations, count, true) {
                    actions.push(TreeAction::Add(Group::Operations));
                }
                if count == 0 {
                    none_yet(ui);
                }
                for (index, o) in project.operations().iter().enumerate() {
                    let item = ItemRef::Operation(o.id);
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
                                    egui::Id::new(("operation drag", id)),
                                    index,
                                    |ui| {
                                        ui.label(RichText::new("⠿").color(DARK.muted));
                                    },
                                )
                                .response;
                            handle.on_hover_text("Drag to move earlier or later (Alt ↑, Alt ↓)");
                            toggled = ui
                                .checkbox(&mut enabled, "")
                                .on_hover_text("Include in generation, simulation and G-code")
                                .changed();
                        },
                    );
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
                    if response.clicked() {
                        actions.push(TreeAction::Select(id));
                    }
                    item_menu(
                        &response,
                        item,
                        Some((o.enabled, index, count)),
                        &mut actions,
                    );
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
