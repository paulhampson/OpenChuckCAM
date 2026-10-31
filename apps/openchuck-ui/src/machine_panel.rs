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
    AvailableMachine, Confirmation, MachineProfile, MachineSource, ProfileEntry, ProfileField,
    Provenance, ShortBlockMode, available_machines, compare_machine, new_machine_id,
    save_profile_entry,
};

use crate::field_row::{self, FieldState, Outcome, TextRow};
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
            egui::Role::Button,
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
        if value.field != ProfileField::ShortBlockMode {
            icon(&mut child, value);
        }
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
    fields: FieldState,
    /// "Manage machines…" was pressed: the shell opens the machine library
    /// window (SET-10; 12.5).
    manage: bool,
}

impl Default for MachinePanel {
    fn default() -> Self {
        Self::with_library(default_folders().map(|f| f.machines))
    }
}

impl MachinePanel {
    /// A panel on the machine library folder `library` (the user's default
    /// folder in [`Default`]).
    pub fn with_library(library: Option<PathBuf>) -> Self {
        let available = available_machines(library.as_deref());
        Self {
            library,
            available,
            chosen: 0,
            message: None,
            fields: FieldState::default(),
            manage: false,
        }
    }
}

impl MachinePanel {
    /// Whether "Manage machines…" was pressed since last asked.
    pub fn take_manage_request(&mut self) -> bool {
        std::mem::take(&mut self.manage)
    }

    fn refresh(&mut self) {
        self.available = available_machines(self.library.as_deref());
        self.chosen = self.chosen.min(self.available.len().saturating_sub(1));
    }

    fn short_blocks(
        &mut self,
        ui: &mut egui::Ui,
        profile: &MachineProfile,
    ) -> Option<MachineProfile> {
        ui.strong("Short moves");
        ui.weak("Report shows command demand. Slow down preserves the programmed path at a lower feed. Zero milliseconds turns the check off.");
        let mut edited = None;
        for (field, label, unit, range, applied) in [
            (
                ProfileField::MinBlockMs,
                "Minimum block time",
                "ms",
                "0 or more",
                profile.min_block_ms.to_string(),
            ),
            (
                ProfileField::BlockWindow,
                "Block window",
                "blocks",
                "whole number from 1 to 4096",
                profile.block_window.to_string(),
            ),
        ] {
            let validate = |text: &str| edit_short_block(profile, field, text).map(|_| ());
            let row = TextRow {
                key: field.name(),
                label,
                unit,
                range: Some(range),
                applied,
                enabled: true,
                number: true,
                description: "Set from the machine's measured block rate and planner queue depth. Applies to this project copy.",
                validate: Some(&validate),
                ..TextRow::default()
            };
            if let Outcome::Commit(text) = field_row::text_row(ui, &mut self.fields, &row) {
                match edit_short_block(edited.as_ref().unwrap_or(profile), field, &text) {
                    Ok(value) => edited = Some(value),
                    Err(reason) => self.fields.refuse(field.name(), &text, reason),
                }
            }
            let value = machine_values(profile)
                .into_iter()
                .find(|v| v.field == field);
            if let Some(value) = value {
                icon(ui, &value);
            }
        }
        let modes = [
            ShortBlockMode::Off,
            ShortBlockMode::Report,
            ShortBlockMode::SlowDown,
        ];
        let current = modes.iter().position(|m| *m == profile.short_block_mode);
        if let Some(index) = field_row::list_row(
            ui,
            "short_block_mode",
            "Mode",
            "Report leaves motion unchanged. Slow down lengthens blocks to meet the configured average; geometry is unchanged.",
            (&["Off", "Report", "Slow down"], current),
            profile.short_block_mode.label(),
        ) && let Some(mode) = modes.get(index)
        {
            let mut next = edited.take().unwrap_or_else(|| profile.clone());
            next.short_block_mode = *mode;
            user_source(&mut next, ProfileField::ShortBlockMode);
            edited = Some(next);
        }
        edited
    }

    /// The spindle-height values (decision record 0317): whether the machine
    /// measures tool lengths, the spindle-to-axis distance and the recommended
    /// minimum safe Z, edited on the project copy.
    fn spindle_height(
        &mut self,
        ui: &mut egui::Ui,
        profile: &MachineProfile,
    ) -> Option<MachineProfile> {
        ui.strong("Spindle height");
        ui.weak("A tool longer than the one the axis height was measured with raises the spindle at the same Z. Leave a value blank when it is not known.");
        let mut edited = None;
        let choices = ["Automatic (tool setter)", "None"];
        let current = if profile.automatic_tool_length { 0 } else { 1 };
        if let Some(index) = field_row::list_row(
            ui,
            "tool_length_measurement",
            "Tool length measurement",
            "Whether the controller measures each tool on a tool setter and offsets it.",
            (&choices, Some(current)),
            choices.get(current).copied().unwrap_or(""),
        ) && index != current
        {
            edited = Some(set_tool_length_measurement(profile, index == 0));
        }
        let shown = |v: Option<openchuck_app::setup::Millimetres>| {
            v.map_or_else(String::new, |v| v.0.to_string())
        };
        // The distance matters only on a machine that measures tool
        // lengths; the minimum shows there or once given (as `fields`).
        let rows = [
            (
                ProfileField::SpindleToAxis,
                "Spindle to axis",
                shown(profile.spindle_to_axis),
                "From the spindle at the top of the Z travel down to the rotary axis. Blank when not known.",
            ),
            (
                ProfileField::RecommendedMinSafeZ,
                "Recommended minimum safe Z",
                shown(profile.recommended_min_safe_z),
                "The lowest safe Z recommended for this machine. Advice to check on the machine, not a limit. Blank for none.",
            ),
        ];
        let shown_fields = profile.fields();
        for (field, label, applied, description) in rows
            .into_iter()
            // The minimum stays editable on a measuring machine even when
            // blank, so it can be set and cleared; without measurement it
            // shows once given (switching measurement on shows it too).
            .filter(|(field, ..)| {
                shown_fields.contains(field)
                    || (*field == ProfileField::RecommendedMinSafeZ
                        && profile.automatic_tool_length)
            })
        {
            let validate = |text: &str| edit_spindle_height(profile, field, text).map(|_| ());
            let row = TextRow {
                key: field.name(),
                label,
                unit: "mm",
                range: Some("above 0, or blank"),
                applied,
                enabled: true,
                number: true,
                description,
                validate: Some(&validate),
                ..TextRow::default()
            };
            if let Outcome::Commit(text) = field_row::text_row(ui, &mut self.fields, &row) {
                match edit_spindle_height(edited.as_ref().unwrap_or(profile), field, &text) {
                    Ok(value) => edited = Some(value),
                    Err(reason) => self.fields.refuse(field.name(), &text, reason),
                }
            }
            if let Some(value) = machine_values(profile)
                .into_iter()
                .find(|v| v.field == field)
            {
                icon(ui, &value);
            }
        }
        edited
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
                for value in values.iter().filter(|v| {
                    !v.collapsed
                        && !matches!(
                            v.field,
                            ProfileField::MinBlockMs
                                | ProfileField::BlockWindow
                                | ProfileField::ShortBlockMode
                                | ProfileField::ToolLengthMeasurement
                                | ProfileField::SpindleToAxis
                                | ProfileField::RecommendedMinSafeZ
                        )
                }) {
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
        if let Some(profile) = copy
            && let Some(edited) = self
                .short_blocks(ui, profile)
                .or_else(|| self.spindle_height(ui, profile))
        {
            command = Some(Command::SetMachineFrom {
                machine: Some(Box::new(edited)),
                id: copy_id.map(str::to_owned),
            });
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

        // Choosing a machine (SET-01); managing them is the machine library
        // window (SET-10; 12.5).
        // "Manage machines…" shares the folder button's row, so the panel is
        // no taller than before it.
        ui.strong("Machine profiles");
        let mut manage = false;
        match self.library.clone() {
            Some(dir) => {
                // One line however long the path is (a Windows profile path
                // would wrap and push the sections below it down); the whole
                // path is in the tooltip.
                let path = format!("Library: {}", dir.display());
                ui.add(egui::Label::new(RichText::new(&path).color(DARK.muted)).truncate())
                    .on_hover_text(&path);
                if dir.is_dir() {
                    ui.horizontal(|ui| {
                        if ui.button("Open the library folder").clicked() {
                            self.message = open_folder(&dir).err();
                        }
                        manage = ui.button("Manage machines…").clicked();
                    });
                } else {
                    ui.weak("The folder does not exist yet; only built-in profiles are listed.");
                    ui.horizontal(|ui| {
                        if ui.button("Create the library folder").clicked() {
                            self.message = Some(match std::fs::create_dir_all(&dir) {
                                Ok(()) => format!("Created {}.", dir.display()),
                                Err(e) => format!("{}: {e}", dir.display()),
                            });
                        }
                        manage = ui.button("Manage machines…").clicked();
                    });
                }
            }
            None => {
                ui.weak("No configuration folder was found; only built-in profiles are listed.");
                manage = ui.button("Manage machines…").clicked();
            }
        }
        if manage {
            self.manage = true;
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
            // The list takes what the Refresh button leaves, so the row
            // never outgrows the pane.
            let refresh = ui
                .painter()
                .layout_no_wrap(
                    "Refresh".to_owned(),
                    egui::TextStyle::Button.resolve(ui.style()),
                    DARK.text,
                )
                .size()
                .x
                + 2.0 * ui.spacing().button_padding.x;
            let list = (ui.available_width() - refresh - ui.spacing().item_spacing.x).max(80.0);
            // The list is laid out in a cell of its own width, so a long
            // profile name is cut to it (the combo box only truncates at the
            // width it is offered) instead of holding the pane open.
            ui.allocate_ui_with_layout(
                egui::vec2(list, ui.spacing().interact_size.y),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    egui::ComboBox::from_id_salt("machine choice")
                        .selected_text(selected)
                        .truncate()
                        .width(list)
                        .show_ui(ui, |ui| {
                            for (i, a) in self.available.iter().enumerate() {
                                ui.selectable_value(&mut self.chosen, i, label(a));
                            }
                        });
                },
            );
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

fn user_source(profile: &mut MachineProfile, field: ProfileField) {
    profile.provenance.insert(field, Provenance { state: Confirmation::Unconfirmed, source: "User configured for this project; validate on the machine with its firmware and program delivery method".into() });
}

/// Records a spindle-height value as the user's own for this project copy.
fn spindle_height_source(profile: &mut MachineProfile, field: ProfileField) {
    profile.provenance.insert(
        field,
        Provenance {
            state: Confirmation::Unconfirmed,
            source: "Set by the user for this project; check it on the machine".into(),
        },
    );
}

/// `profile` with tool length measurement on or off.
fn set_tool_length_measurement(profile: &MachineProfile, on: bool) -> MachineProfile {
    let mut next = profile.clone();
    next.automatic_tool_length = on;
    spindle_height_source(&mut next, ProfileField::ToolLengthMeasurement);
    next
}

/// `profile` with the spindle-to-axis distance or the recommended minimum
/// safe Z set from `text`: blank for none, else millimetres above zero.
fn edit_spindle_height(
    profile: &MachineProfile,
    field: ProfileField,
    text: &str,
) -> Result<MachineProfile, String> {
    let value = match text.trim() {
        "" => None,
        t => Some(openchuck_app::setup::Millimetres(
            t.parse::<f64>()
                .map_err(|_| "Enter millimetres above 0, or leave it blank.".to_owned())?,
        )),
    };
    let mut next = profile.clone();
    match field {
        ProfileField::SpindleToAxis => next.spindle_to_axis = value,
        ProfileField::RecommendedMinSafeZ => next.recommended_min_safe_z = value,
        _ => return Err("Choose a spindle-height value.".into()),
    }
    next.validate()
        .map_err(|_| "Enter millimetres above 0, or leave it blank.".to_owned())?;
    spindle_height_source(&mut next, field);
    Ok(next)
}

fn edit_short_block(
    profile: &MachineProfile,
    field: ProfileField,
    text: &str,
) -> Result<MachineProfile, String> {
    let mut next = profile.clone();
    match field {
        ProfileField::MinBlockMs => {
            next.min_block_ms = text
                .trim()
                .parse::<f64>()
                .map_err(|_| "Enter a finite number of milliseconds, 0 or more.".to_owned())?
        }
        ProfileField::BlockWindow => {
            let number = text
                .trim()
                .parse::<f64>()
                .map_err(|_| "Enter a whole number of blocks, 1 to 4096.".to_owned())?;
            if !number.is_finite() || number.fract() != 0.0 || !(1.0..=4096.0).contains(&number) {
                return Err("Enter a whole number of blocks, 1 to 4096.".into());
            }
            next.block_window = number as usize;
        }
        _ => return Err("Choose a short-move setting.".into()),
    }
    next.validate().map_err(|e| e.to_string())?;
    user_source(&mut next, field);
    Ok(next)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn req_gc_17_req_ui_13_machine_mode_control_selects_slowdown() {
        let ctx = egui::Context::default();
        let mut panel = MachinePanel::default();
        let machine = MachineProfile::example_x_rotary();
        let mut frame = |events| {
            let mut edited = None;
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        Pos2::ZERO,
                        egui::vec2(420.0, 400.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    edited = panel.short_blocks(ui, &machine);
                },
            );
            output.textures_delta.clear();
            (edited, output.shapes)
        };
        fn find(shape: &egui::Shape, needle: &str) -> Option<Pos2> {
            match shape {
                egui::Shape::Text(text) if text.galley.text() == needle => {
                    Some(text.pos + text.galley.rect.center().to_vec2())
                }
                egui::Shape::Vec(shapes) => shapes.iter().find_map(|s| find(s, needle)),
                _ => None,
            }
        }
        let (_, shapes) = frame(Vec::new());
        let at = shapes
            .iter()
            .find_map(|s| find(&s.shape, "Off"))
            .expect("Mode dropdown is shown");
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        frame(vec![egui::Event::PointerMoved(at)]);
        frame(vec![button(at, true)]);
        frame(vec![button(at, false)]);
        let (_, shapes) = frame(Vec::new());
        let at = shapes
            .iter()
            .find_map(|s| find(&s.shape, "Slow down"))
            .expect("Slow down option is shown");
        frame(vec![egui::Event::PointerMoved(at)]);
        frame(vec![button(at, true)]);
        let (edited, _) = frame(vec![button(at, false)]);
        let edited = edited.unwrap();
        assert_eq!(edited.short_block_mode, ShortBlockMode::SlowDown);
        assert_eq!(
            edited.min_block_ms.to_bits(),
            machine.min_block_ms.to_bits()
        );
        assert_eq!(edited.block_window, machine.block_window);
        assert!(!edited.is_confirmed(ProfileField::ShortBlockMode));
    }

    /// The spindle-height values are edited on the project copy: a blank
    /// distance or minimum means not known or none, a number must be above
    /// zero, and an edited value is the user's, not confirmed (SET-01, UI-13,
    /// decision 0317).
    #[test]
    fn req_set_01_req_ui_13_spindle_height_controls_validate_and_keep_other_values() {
        let original = MachineProfile::example_x_rotary();
        let measured = set_tool_length_measurement(&original, true);
        assert!(measured.automatic_tool_length);
        assert!(!measured.is_confirmed(ProfileField::ToolLengthMeasurement));
        assert_eq!(measured.max_feeds, original.max_feeds);
        let distance =
            edit_spindle_height(&measured, ProfileField::SpindleToAxis, " 78.6 ").unwrap();
        assert_eq!(distance.spindle_to_axis.map(|d| d.0), Some(78.6));
        assert!(distance.automatic_tool_length);
        let minimum =
            edit_spindle_height(&distance, ProfileField::RecommendedMinSafeZ, "30").unwrap();
        assert_eq!(minimum.recommended_min_safe_z.map(|d| d.0), Some(30.0));
        assert_eq!(minimum.spindle_to_axis, distance.spindle_to_axis);
        let cleared = edit_spindle_height(&minimum, ProfileField::SpindleToAxis, "").unwrap();
        assert_eq!(cleared.spindle_to_axis, None);
        assert!(!cleared.is_confirmed(ProfileField::SpindleToAxis));
        for field in [
            ProfileField::SpindleToAxis,
            ProfileField::RecommendedMinSafeZ,
        ] {
            for text in ["0", "-1", "NaN", "inf", "abc"] {
                assert!(
                    edit_spindle_height(&original, field, text).is_err(),
                    "{text}"
                );
            }
        }
        assert!(edit_spindle_height(&original, ProfileField::Travel, "1").is_err());
    }

    #[test]
    fn req_gc_17_req_set_01_machine_controls_validate_and_keep_other_values() {
        let original = MachineProfile::example_x_rotary();
        let limit = edit_short_block(&original, ProfileField::MinBlockMs, "7.5").unwrap();
        assert_eq!(limit.min_block_ms.to_bits(), 7.5f64.to_bits());
        assert_eq!(limit.max_feeds, original.max_feeds);
        assert!(!limit.is_confirmed(ProfileField::MinBlockMs));
        let window = edit_short_block(&limit, ProfileField::BlockWindow, "32.0").unwrap();
        assert_eq!(window.block_window, 32);
        assert_eq!(window.min_block_ms.to_bits(), 7.5f64.to_bits());
        for text in ["0", "-1", "2.5", "4097", "NaN", "abc"] {
            assert!(edit_short_block(&original, ProfileField::BlockWindow, text).is_err());
        }
        for text in ["-1", "NaN", "inf", "abc"] {
            assert!(edit_short_block(&original, ProfileField::MinBlockMs, text).is_err());
        }
    }
}
