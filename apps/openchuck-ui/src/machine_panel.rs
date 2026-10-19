// SPDX-License-Identifier: AGPL-3.0-or-later
//! The project's machine (SET-01, UI-13): the Machine section of the setup
//! inspector (interface specification sections 8 and 11.3). It shows the
//! profile copy's values, each with an icon saying whether it is confirmed
//! on the machine (two icons on the profile's two states: decision record
//! 0114 section F answer 4, OI-339; OI-345 holds the UI-13 wording), and how
//! the copy compares with the library profile it came from (OI-54; decision
//! record 0029 item 16). The library is the platform's configuration folder
//! (OI-53). Choosing or updating the machine is a command, so it can be
//! undone.

use std::path::PathBuf;

use eframe::egui::{self, Color32, Pos2, RichText, Stroke};
use openchuck_app::Command;
use openchuck_app::library::default_folders;
use openchuck_app::machine_view::{MachineValue, machine_values};
use openchuck_app::setup::{
    AvailableMachine, MachineProfile, MachineSource, ProfileEntry, available_machines,
    compare_machine, new_machine_id, save_profile_entry,
};

use crate::field_row;
use crate::theme::{DARK, width};

const NOTE: Color32 = DARK.warning;

/// The side of a provenance icon (section 8: 20 pixels).
const ICON: f32 = 20.0;

/// Paints the provenance icon of a value: a green tick in a circle when it is
/// confirmed on the machine, an amber triangle when it is not (section 8, as
/// the owner chose, OI-339). Each differs in shape as well as colour
/// (NFR-15).
pub fn paint_icon(painter: &egui::Painter, centre: Pos2, confirmed: bool) {
    let r = 6.5;
    if confirmed {
        let stroke = Stroke::new(1.5, DARK.ok);
        painter.circle_stroke(centre, r, stroke);
        painter.line(
            vec![
                centre + egui::vec2(-3.0, 0.4),
                centre + egui::vec2(-1.0, 2.4),
                centre + egui::vec2(3.0, -2.0),
            ],
            stroke,
        );
    } else {
        let stroke = Stroke::new(1.5, DARK.warning);
        let top = centre + egui::vec2(0.0, -r);
        let right = centre + egui::vec2(r, r * 0.8);
        let left = centre + egui::vec2(-r, r * 0.8);
        painter.add(egui::Shape::closed_line(vec![top, right, left], stroke));
        painter.line_segment(
            [
                centre + egui::vec2(0.0, -2.0),
                centre + egui::vec2(0.0, 1.5),
            ],
            stroke,
        );
        painter.circle_filled(centre + egui::vec2(0.0, 3.3), 0.8, DARK.warning);
    }
}

/// A provenance icon as a focusable widget: its tooltip opens on hover and on
/// keyboard focus, and screen readers read its state (section 16).
pub fn icon(ui: &mut egui::Ui, value: &MachineValue) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(ICON, ICON), egui::Sense::click());
    paint_icon(ui.painter(), rect.center(), value.confirmed);
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect,
            3.0,
            Stroke::new(2.0, DARK.accent),
            egui::StrokeKind::Inside,
        );
    }
    let state = if value.confirmed {
        "confirmed on the machine"
    } else {
        "not confirmed on the machine"
    };
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Button,
            true,
            format!("{}: {state}", value.label),
        )
    });
    field_row::tip(response, &value.tooltip)
}

/// One value row: the label with its icon at the right end of the label
/// column, so the icons line up in one column, then the value (section 8).
fn value_row(ui: &mut egui::Ui, value: &MachineValue) {
    ui.horizontal(|ui| {
        let height = ui.spacing().interact_size.y;
        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(width::LABEL_COLUMN, height),
            egui::Sense::hover(),
        );
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(rect)
                .layout(egui::Layout::right_to_left(egui::Align::Center)),
        );
        icon(&mut child, value);
        child.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.add(egui::Label::new(RichText::new(value.label).color(DARK.muted)).truncate());
        });
        ui.add(egui::Label::new(&value.value).wrap());
    });
}

/// The machine panel's state: the profiles found when it was opened.
#[derive(Debug)]
pub struct MachinePanel {
    library: Option<PathBuf>,
    available: Vec<AvailableMachine>,
    chosen: usize,
    message: Option<String>,
}

impl Default for MachinePanel {
    fn default() -> Self {
        let library = default_folders().map(|f| f.machines);
        let available = available_machines(library.as_deref());
        Self {
            library,
            available,
            chosen: 0,
            message: None,
        }
    }
}

impl MachinePanel {
    fn refresh(&mut self) {
        self.available = available_machines(self.library.as_deref());
        self.chosen = self.chosen.min(self.available.len().saturating_sub(1));
    }

    /// Draws the panel for the project's machine `copy`. Returns a command
    /// to execute, if the user chose or updated the machine.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        copy: Option<&MachineProfile>,
        copy_id: Option<&str>,
    ) -> Option<Command> {
        let mut command = None;
        match copy {
            None => {
                ui.label(RichText::new("No machine is chosen.").color(NOTE));
                ui.label("Choose a machine profile below.");
            }
            Some(profile) => {
                field_row::read_only_row(
                    ui,
                    "Profile",
                    "The machine profile copied into the project.",
                    &profile.name,
                );
                let values = machine_values(profile);
                for value in values.iter().filter(|v| !v.collapsed) {
                    value_row(ui, value);
                }
                egui::CollapsingHeader::new(
                    RichText::new("Travel and maximum feeds").color(DARK.muted),
                )
                .id_salt("machine travel")
                .default_open(false)
                .show(ui, |ui| {
                    for value in values.iter().filter(|v| v.collapsed) {
                        value_row(ui, value);
                    }
                });
            }
        }
        ui.separator();

        // The copy against its source (OI-54).
        match compare_machine(copy, copy_id, &self.available) {
            MachineSource::NoMachine => {}
            MachineSource::Unrecorded => {
                ui.weak("This copy does not record which library profile it came from.");
            }
            MachineSource::NotFound { .. } => {
                ui.colored_label(
                    NOTE,
                    "The profile this copy came from is not in the library.",
                );
            }
            MachineSource::Same { source } => {
                ui.label(format!("Matches {source}."));
            }
            MachineSource::Differs {
                source,
                id,
                profile,
                fields,
            } => {
                ui.colored_label(
                    NOTE,
                    format!("Differs from {source} in: {}.", fields.join(", ")),
                );
                ui.weak("The project keeps its own copy until you update it.");
                if ui.button("Update the copy from the library").clicked() {
                    command = Some(Command::SetMachineFrom {
                        machine: Some(profile),
                        id: Some(id),
                    });
                }
            }
            _ => {}
        }
        ui.separator();

        // Choosing a machine (SET-01).
        ui.strong("Machine profiles");
        match &self.library {
            Some(dir) => {
                ui.weak(format!("Library: {}", dir.display()));
                if dir.is_dir() {
                    if ui.button("Open the library folder").clicked() {
                        self.message = open_folder(dir).err();
                    }
                } else {
                    ui.weak("The folder does not exist yet; only built-in profiles are listed.");
                    if ui.button("Create the library folder").clicked() {
                        self.message = Some(match std::fs::create_dir_all(dir) {
                            Ok(()) => format!("Created {}.", dir.display()),
                            Err(e) => format!("{}: {e}", dir.display()),
                        });
                    }
                }
            }
            None => {
                ui.weak("No configuration folder was found; only built-in profiles are listed.");
            }
        }
        let label = |a: &AvailableMachine| match &a.profile {
            Ok(p) => format!("{} ({})", p.name, a.source),
            Err(_) => format!("{} (cannot be read)", a.source),
        };
        let selected = self
            .available
            .get(self.chosen)
            .map(label)
            .unwrap_or_default();
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("machine choice")
                .selected_text(selected)
                .width(260.0)
                .show_ui(ui, |ui| {
                    for (i, a) in self.available.iter().enumerate() {
                        ui.selectable_value(&mut self.chosen, i, label(a));
                    }
                });
            if ui.button("Refresh").clicked() {
                self.refresh();
            }
        });
        if let Some(a) = self.available.get(self.chosen) {
            match &a.profile {
                Err(e) => {
                    ui.colored_label(NOTE, e);
                }
                Ok(p) => {
                    if ui.button("Use this machine").clicked() {
                        command = Some(Command::SetMachineFrom {
                            machine: Some(Box::new(p.clone())),
                            id: a.id.clone(),
                        });
                    }
                    if a.id.is_none() && !a.source.starts_with("built-in ") {
                        ui.weak(
                            "This library profile has no reference, so a project copy of it cannot \
                             be compared with it later.",
                        );
                        if ui
                            .button("Give it a reference (rewrites the library file)")
                            .clicked()
                        {
                            let with_id = ProfileEntry {
                                id: Some(new_machine_id()),
                                profile: p.clone(),
                            };
                            self.message = Some(
                                match save_profile_entry(&with_id, std::path::Path::new(&a.source))
                                {
                                    Ok(()) => format!("Saved {} with a reference.", a.source),
                                    Err(e) => e.to_string(),
                                },
                            );
                            self.refresh();
                        }
                    }
                }
            }
        }
        if let Some(message) = &self.message {
            ui.small(message);
        }
        command
    }
}

/// Opens `dir` in the platform's file browser. Returns why it could not.
pub fn open_folder(dir: &std::path::Path) -> Result<(), String> {
    let program = if cfg!(target_os = "windows") {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    std::process::Command::new(program)
        .arg(dir)
        .spawn()
        .map(drop)
        .map_err(|e| format!("could not open {} with {program}: {e}", dir.display()))
}
