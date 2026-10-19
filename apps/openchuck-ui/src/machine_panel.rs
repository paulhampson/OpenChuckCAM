// SPDX-License-Identifier: AGPL-3.0-or-later
//! The project's machine (SET-01): its profile copy, the direction of A,
//! the values not yet confirmed on the machine, and how the copy compares
//! with the library profile it came from (OI-54; decision record 0029
//! item 16). The library is the platform's configuration folder (OI-53).
//! Choosing or updating the machine is a command, so it can be undone.

use std::path::PathBuf;

use eframe::egui::{self, Color32};
use openchuck_app::Command;
use openchuck_app::library::default_folders;
use openchuck_app::setup::{
    AvailableMachine, MachineProfile, MachineSource, ProfileEntry, ProfileField, a_direction,
    available_machines, compare_machine, new_machine_id, profile_field_label, save_profile_entry,
};

use crate::theme::DARK;

const NOTE: Color32 = DARK.warning;

/// Said under the profile's name when some values are not confirmed: the
/// values in the user's words, not their names in the profile file
/// (specification section 15). `None` when every value is confirmed.
fn unconfirmed_text(profile: &MachineProfile) -> Option<String> {
    let names: Vec<&str> = ProfileField::ALL
        .iter()
        .filter(|f| !profile.is_confirmed(**f))
        .map(|f| profile_field_label(*f))
        .collect();
    (!names.is_empty()).then(|| format!("Not confirmed on the machine: {}.", names.join(", ")))
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
                ui.heading("No machine");
                ui.label("Choose a machine profile below.");
            }
            Some(profile) => {
                ui.heading(&profile.name);
                ui.label(format!("A direction: {}.", a_direction(profile)));
                if let Some(text) = unconfirmed_text(profile) {
                    ui.colored_label(NOTE, text);
                }
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
                            "This library profile has no reference, so a project copy of it cannot be \
                             compared with it later.",
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
fn open_folder(dir: &std::path::Path) -> Result<(), String> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use openchuck_app::setup::{MAKERA_Z1, builtin};

    /// Specification section 15 and UI-13: the machine's unconfirmed values
    /// are named in plain words, never by the file's field names.
    #[test]
    fn req_nfr_15_unconfirmed_values_are_named_in_plain_words() {
        let z1 = builtin(MAKERA_Z1).unwrap().unwrap();
        let text = unconfirmed_text(&z1).unwrap();
        assert!(text.starts_with("Not confirmed on the machine: "), "{text}");
        assert!(text.contains("axis height"), "{text}");
        assert!(!text.contains('_'), "{text}");
    }
}
