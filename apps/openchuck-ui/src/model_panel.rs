// SPDX-License-Identifier: AGPL-3.0-or-later
//! A project model's inspector (interface specification 11.2; decision
//! records 0029 item 13 and 0114 C14): its file, format and triangle count
//! and size; its unit, with where the unit came from (IMP-03); its placement
//! steps, which can be reordered, edited and removed (IMP-05); its check
//! report (IMP-04) with the full report a click away; and its source, with
//! the content hash, "Replace file…" and "Show in folder" (PRJ-02). Every
//! change is a command, so it can be undone.

use eframe::egui::{self, RichText};
use openchuck_app::model::{
    Axis, AxisDirection, Degrees, LengthUnit, ModelFormat, ModelInspection, PreparedModel,
    TransformStep, UnitSource, orient_to_rotary_axis,
};
use openchuck_app::project::ModelRef;
use openchuck_app::project::id::ModelId;
use openchuck_app::settings::UserSettings;
use openchuck_app::{Command, JobHandle};

use crate::field_row::{self, FieldState, Outcome, TextRow};
use crate::theme::DARK;

/// A step being set up before it is added.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NewStep {
    /// Translate by x, y, z.
    Translate([f64; 3]),
    /// Rotate about an axis.
    Rotate(Axis, f64),
    /// Scale by a factor.
    Scale(f64),
    /// Mirror across the plane normal to an axis.
    Mirror(Axis),
    /// Centre on the rotary axis.
    Centre,
    /// Turn a direction of the model onto the rotary axis, and centre.
    Orient(AxisDirection),
}

impl NewStep {
    fn kind(&self) -> &'static str {
        match self {
            Self::Translate(_) => "Translate",
            Self::Rotate(..) => "Rotate",
            Self::Scale(_) => "Scale",
            Self::Mirror(_) => "Mirror",
            Self::Centre => "Centre on the rotary axis",
            Self::Orient(_) => "Orient to the rotary axis",
        }
    }

    /// The steps it adds.
    pub fn steps(self) -> Vec<TransformStep> {
        match self {
            Self::Translate([x, y, z]) => vec![TransformStep::Translate { x, y, z }],
            Self::Rotate(axis, angle) => vec![TransformStep::Rotate {
                axis,
                angle: Degrees(angle),
            }],
            Self::Scale(factor) => vec![TransformStep::Scale { factor }],
            Self::Mirror(axis) => vec![TransformStep::Mirror { axis }],
            Self::Centre => vec![TransformStep::CentreOnRotaryAxis],
            Self::Orient(direction) => orient_to_rotary_axis(direction),
        }
    }
}

/// What the panel gave back this frame.
#[derive(Debug, Default)]
pub struct ModelOutcome {
    /// A command to execute.
    pub command: Option<Command>,
    /// A section was opened or closed: the settings need writing.
    pub settings_changed: bool,
    /// The user asked to choose a file to replace the model's.
    pub choose_replacement: bool,
    /// A replacement file has been read and checked: store it.
    pub replaced: Option<PreparedModel>,
}

/// The model panel's state between frames.
pub struct ModelPanel {
    model: ModelId,
    new_step: NewStep,
    /// The check of the model as placed now, and what it was run for.
    check: Option<(ModelRef, Result<ModelInspection, String>)>,
    job: Option<(ModelRef, JobHandle<ModelInspection>)>,
    /// A replacement file being read, and why the last one was refused.
    replacing: Option<JobHandle<PreparedModel>>,
    replace_error: Option<String>,
    /// Typed text of the placement steps' values.
    fields: FieldState,
    /// The full report window is open.
    report_open: bool,
    /// Why "Show in folder" failed.
    folder_error: Option<String>,
    /// A section to open and scroll to on the next frame, chosen by its row
    /// in the project tree (issue #181).
    focus: Option<&'static str>,
    /// Sections closed in the settings that were opened only to show them:
    /// they stay closed in the settings until the user closes one.
    shown: Vec<&'static str>,
}

/// The key of the Placement section, which the model's "Placement" row in
/// the project tree opens (section 6; issue #181).
pub const PLACEMENT: &str = "placement";
/// The key of the Mesh check section, which the model's "Mesh check" row
/// opens (section 6).
pub const MESH_CHECK: &str = "check";

impl std::fmt::Debug for ModelPanel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModelPanel")
            .field("model", &self.model)
            .finish_non_exhaustive()
    }
}

fn axis_name(axis: Axis) -> &'static str {
    match axis {
        Axis::X => "X",
        Axis::Y => "Y",
        Axis::Z => "Z",
    }
}

const AXES: [Axis; 3] = [Axis::X, Axis::Y, Axis::Z];

fn axis_combo(ui: &mut egui::Ui, id: &str, axis: &mut Axis) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(axis_name(*axis))
        .width(40.0)
        .show_ui(ui, |ui| {
            for a in AXES {
                ui.selectable_value(axis, a, axis_name(a));
            }
        });
}

/// Where the unit came from, for the hint under the unit (11.2).
pub fn unit_source(units: &openchuck_app::model::Units) -> String {
    let unit = units.applied.short_name();
    match units.source {
        UnitSource::User => format!("Read in {unit}: chosen by you."),
        UnitSource::File => format!("Read in {unit}: the file states it."),
        UnitSource::Default => match units.file {
            Some(named) => format!(
                "Read in {unit}, the default; the file names {}.",
                named.short_name()
            ),
            None => format!("Read in {unit}, the default: the file names no unit."),
        },
    }
}

/// A placement step in words for its row, with its values to fixed places.
pub fn describe(step: &TransformStep) -> String {
    let mm = |v: f64| field_row::number(v, Some(3));
    match step {
        TransformStep::Translate { x, y, z } => {
            format!("Translate X {} Y {} Z {} mm", mm(*x), mm(*y), mm(*z))
        }
        TransformStep::Rotate { axis, angle } => format!(
            "Rotate {} about {}",
            field_row::number(angle.0, Some(2)) + "\u{b0}",
            axis_name(*axis)
        ),
        TransformStep::Scale { factor } => {
            format!("Scale \u{d7} {}", field_row::number(*factor, None))
        }
        TransformStep::Mirror { axis } => format!("Mirror {}", axis_name(*axis)),
        TransformStep::CentreOnRotaryAxis => "Centre on the rotary axis".to_owned(),
    }
}

/// `steps` with the step at `from` moved to `to`.
pub fn moved(steps: &[TransformStep], from: usize, to: usize) -> Vec<TransformStep> {
    let mut steps = steps.to_vec();
    if from < steps.len() && to < steps.len() && from != to {
        let step = steps.remove(from);
        steps.insert(to, step);
    }
    steps
}

/// The sections of the panel and whether each is open by default.
const SECTIONS: [(&str, &str); 5] = [
    ("file", "File"),
    ("units", "Units"),
    ("placement", "Placement"),
    ("check", "Mesh check"),
    ("source", "Source"),
];
const DEFAULT_OPEN: [&str; 4] = ["file", "units", "placement", "check"];

impl ModelPanel {
    /// The panel for `model`.
    pub fn new(model: ModelId) -> Self {
        Self {
            model,
            new_step: NewStep::Centre,
            check: None,
            job: None,
            replacing: None,
            replace_error: None,
            fields: FieldState::default(),
            report_open: false,
            folder_error: None,
            focus: None,
            shown: Vec::new(),
        }
    }

    /// Opens the section `key` and scrolls to it on the next frame.
    pub fn open_section(&mut self, key: &'static str) {
        self.focus = Some(key);
    }

    /// The model shown.
    pub fn model(&self) -> ModelId {
        self.model
    }

    /// Reads a replacement file chosen by the user (a job); the panel hands
    /// it back to store when it is ready.
    pub fn replace_with(&mut self, job: Result<JobHandle<PreparedModel>, String>) {
        match job {
            Ok(job) => {
                self.replacing = Some(job);
                self.replace_error = None;
            }
            Err(e) => self.replace_error = Some(e),
        }
    }

    /// Draws the panel for `reference` as the project holds it now, starting
    /// a check when it changed.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        app: &openchuck_app::Application,
        reference: &ModelRef,
        settings: &mut UserSettings,
    ) -> ModelOutcome {
        let mut outcome = ModelOutcome::default();
        // Keep the check in step with the model's units and placement.
        if let Some((for_ref, job)) = &self.job
            && let Some(result) = job.try_result()
        {
            self.check = Some((for_ref.clone(), result.map_err(|e| e.to_string())));
            self.job = None;
        }
        let current = self.check.as_ref().is_some_and(|(r, _)| r == reference)
            || self.job.as_ref().is_some_and(|(r, _)| r == reference);
        if !current && let Ok(job) = app.start_inspect_model(reference.id) {
            self.job = Some((reference.clone(), job));
        }
        if let Some(job) = &self.replacing {
            match job.try_result() {
                None => ui.ctx().request_repaint(),
                Some(Ok(prepared)) => {
                    self.replacing = None;
                    outcome.replaced = Some(prepared);
                }
                Some(Err(e)) => {
                    self.replacing = None;
                    self.replace_error = Some(format!("The file was not replaced: {e}"));
                }
            }
        }
        if self.job.is_some() {
            ui.ctx().request_repaint();
        }
        let inspection = match &self.check {
            Some((_, Ok(i))) if self.job.is_none() => Some(i.clone()),
            _ => None,
        };

        let focus = self.focus.take();
        for (key, title) in SECTIONS {
            let open = settings.section_open("model", key, DEFAULT_OPEN.contains(&key));
            let focused = focus == Some(key);
            let section = field_row::section(
                ui,
                egui::Id::new(("model section", key)),
                title,
                open,
                focused,
                |_| {},
                |ui| match key {
                    "file" => self.file_section(ui, reference, inspection.as_ref()),
                    "units" => self.units_section(ui, reference, inspection.as_ref()),
                    "placement" => self.placement_section(ui, reference),
                    "check" => {
                        self.check_section(ui, inspection.as_ref());
                        None
                    }
                    _ => {
                        if self.source_section(ui, app, reference) {
                            outcome.choose_replacement = true;
                        }
                        None
                    }
                },
            );
            if let Some(Some(command)) = section.body {
                outcome.command = Some(command);
            }
            if focused {
                section.header.scroll_to_me(Some(egui::Align::TOP));
            }
            // A section opened to show it is not the user's choice to keep.
            if focused && !open && !self.shown.contains(&key) {
                self.shown.push(key);
            }
            if !section.open {
                self.shown.retain(|k| *k != key);
            }
            if section.open != open && !self.shown.contains(&key) {
                settings.set_section_open("model", key, section.open, &DEFAULT_OPEN);
                outcome.settings_changed = true;
            }
        }
        self.report_window(ui.ctx(), inspection.as_ref());
        outcome
    }

    /// File, format and triangle count, and size (11.2).
    fn file_section(
        &self,
        ui: &mut egui::Ui,
        reference: &ModelRef,
        inspection: Option<&ModelInspection>,
    ) -> Option<Command> {
        field_row::read_only_row(
            ui,
            "File",
            "Relative to the project folder.",
            reference.path.as_str(),
        );
        match inspection {
            Some(i) => {
                let format = match i.format {
                    ModelFormat::Stl => "STL",
                    ModelFormat::Step => "STEP",
                    _ => "Other",
                };
                field_row::read_only_row(ui, "Format", "", format);
                field_row::read_only_row(ui, "Triangles", "", i.summary.triangles.to_string());
                if let Some(b) = i.summary.bounds {
                    field_row::read_only_row(
                        ui,
                        "Size",
                        "Along X, across Y and Z, as placed.",
                        format!(
                            "{} \u{d7} {} \u{d7} {} mm",
                            field_row::number(b.max.x - b.min.x, Some(3)),
                            field_row::number(b.max.y - b.min.y, Some(3)),
                            field_row::number(b.max.z - b.min.z, Some(3))
                        ),
                    );
                }
            }
            None => self.checking(ui),
        }
        None
    }

    fn checking(&self, ui: &mut egui::Ui) {
        match &self.check {
            Some((_, Err(e))) if self.job.is_none() => {
                ui.label(RichText::new(e).color(DARK.error));
            }
            _ => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(RichText::new("Checking\u{2026}").color(DARK.muted));
                });
            }
        }
    }

    /// The unit, with where it came from, and the unit question in the
    /// warning box (IMP-03; 11.2).
    fn units_section(
        &self,
        ui: &mut egui::Ui,
        reference: &ModelRef,
        inspection: Option<&ModelInspection>,
    ) -> Option<Command> {
        let mut options = vec!["From the file"];
        options.extend(LengthUnit::ALL.iter().map(|u| u.short_name()));
        let current = match reference.units {
            None => Some(0),
            Some(u) => LengthUnit::ALL.iter().position(|a| *a == u).map(|i| i + 1),
        };
        let command = field_row::choice_row(
            ui,
            "model units",
            "Unit",
            "The unit the file's numbers are in. Changing it converts the model; it can be \
             undone.",
            &options,
            current,
            true,
        )
        .map(|i| Command::SetModelUnits {
            model: reference.id,
            units: i
                .checked_sub(1)
                .and_then(|i| LengthUnit::ALL.get(i).copied()),
        });
        if let Some(i) = inspection {
            field_row::note_line(ui, &unit_source(&i.units), DARK.muted);
            if let Some(question) = &i.unit_question {
                let boxed = DARK.warning_box;
                egui::Frame::new()
                    .fill(boxed.bg)
                    .stroke(egui::Stroke::new(1.0, boxed.border))
                    .corner_radius(crate::theme::RADIUS)
                    .inner_margin(egui::Margin::same(8))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        let text = question.message();
                        let mut chars = text.chars();
                        let sentence = chars
                            .next()
                            .map(|c| c.to_uppercase().collect::<String>() + chars.as_str())
                            .unwrap_or_default();
                        ui.label(RichText::new(sentence).color(boxed.text));
                    });
            }
        }
        command
    }

    /// The placement steps (IMP-05; 11.2): an ordered list with a drag
    /// handle, a description and the values of each step, which can be
    /// edited; removing a step is one undoable command; Add step offers
    /// centre, orient, translate, rotate, scale and mirror.
    fn placement_section(&mut self, ui: &mut egui::Ui, reference: &ModelRef) -> Option<Command> {
        let mut command = None;
        let set = |steps: Vec<TransformStep>| Command::SetModelTransform {
            model: reference.id,
            transform: steps,
        };
        ui.label(RichText::new(openchuck_app::setup_form::PLACEMENT_EXPLAINED).color(DARK.muted));
        if reference.transform.is_empty() {
            ui.label(RichText::new("As in the file.").color(DARK.muted));
        }
        let mut moved_to: Option<(usize, usize)> = None;
        for (i, step) in reference.transform.iter().enumerate() {
            let row = ui.horizontal(|ui| {
                let handle = ui
                    .add(
                        egui::Label::new(RichText::new("\u{2630}").color(DARK.muted))
                            .sense(egui::Sense::drag()),
                    )
                    .on_hover_text("Drag to reorder");
                handle.dnd_set_drag_payload(i);
                ui.label(format!("{}. {}", i + 1, describe(step)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .small_button("\u{d7}")
                        .on_hover_text("Remove this step (it can be undone)")
                        .clicked()
                    {
                        let mut steps = reference.transform.clone();
                        steps.remove(i);
                        command = Some(set(steps));
                    }
                    if ui
                        .add_enabled(
                            i + 1 < reference.transform.len(),
                            egui::Button::new("\u{2193}").small(),
                        )
                        .on_hover_text("Move later")
                        .clicked()
                    {
                        moved_to = Some((i, i + 1));
                    }
                    if ui
                        .add_enabled(i > 0, egui::Button::new("\u{2191}").small())
                        .on_hover_text("Move earlier")
                        .clicked()
                    {
                        moved_to = Some((i, i - 1));
                    }
                });
            });
            if let Some(from) = row.response.dnd_release_payload::<usize>() {
                moved_to = Some((*from, i));
            }
            if let Some(edited) = self.step_values(ui, i, step) {
                let mut steps = reference.transform.clone();
                if let Some(slot) = steps.get_mut(i) {
                    *slot = edited;
                }
                command = Some(set(steps));
            }
        }
        if let Some((from, to)) = moved_to {
            command = Some(set(moved(&reference.transform, from, to)));
        }
        if let Some(c) = self.add_step(ui, reference) {
            command = Some(c);
        }
        command
    }

    /// The values of step `i` as field rows; returns the edited step when a
    /// value was committed (11.2: "editing a step's value after adding it").
    fn step_values(
        &mut self,
        ui: &mut egui::Ui,
        i: usize,
        step: &TransformStep,
    ) -> Option<TransformStep> {
        let mut edited = None;
        ui.indent(("step values", i), |ui| match *step {
            TransformStep::Translate { x, y, z } => {
                let mut v = [x, y, z];
                for (k, label) in ["X", "Y", "Z"].into_iter().enumerate() {
                    let Some(slot) = v.get_mut(k) else { continue };
                    if let Some(n) =
                        self.number(ui, &format!("step {i} {label}"), label, "mm", *slot)
                    {
                        *slot = n;
                        let [x, y, z] = v;
                        edited = Some(TransformStep::Translate { x, y, z });
                    }
                }
            }
            TransformStep::Rotate { axis, angle } => {
                let names = ["X", "Y", "Z"];
                if let Some(a) = field_row::choice_row(
                    ui,
                    &format!("step {i} axis"),
                    "About",
                    "",
                    &names,
                    AXES.iter().position(|x| *x == axis),
                    true,
                ) && let Some(axis) = AXES.get(a)
                {
                    edited = Some(TransformStep::Rotate { axis: *axis, angle });
                }
                if let Some(n) =
                    self.number(ui, &format!("step {i} angle"), "Angle", "\u{b0}", angle.0)
                {
                    edited = Some(TransformStep::Rotate {
                        axis,
                        angle: Degrees(n),
                    });
                }
            }
            TransformStep::Scale { factor } => {
                if let Some(n) = self.number(ui, &format!("step {i} factor"), "Factor", "", factor)
                {
                    if n > 0.0 {
                        edited = Some(TransformStep::Scale { factor: n });
                    } else {
                        self.fields.refuse(
                            &format!("step {i} factor"),
                            &n.to_string(),
                            "Must be more than 0. Not applied.".to_owned(),
                        );
                    }
                }
            }
            TransformStep::Mirror { axis } => {
                if let Some(a) = field_row::choice_row(
                    ui,
                    &format!("step {i} mirror"),
                    "Across",
                    "",
                    &["X", "Y", "Z"],
                    AXES.iter().position(|x| *x == axis),
                    true,
                ) && let Some(axis) = AXES.get(a)
                {
                    edited = Some(TransformStep::Mirror { axis: *axis });
                }
            }
            _ => {}
        });
        edited
    }

    /// A number row of a step; returns a committed value.
    fn number(
        &mut self,
        ui: &mut egui::Ui,
        key: &str,
        label: &str,
        unit: &str,
        value: f64,
    ) -> Option<f64> {
        let outcome = field_row::text_row(
            ui,
            &mut self.fields,
            &TextRow {
                key,
                label,
                unit,
                applied: field_row::number(value, field_row::places(unit)),
                enabled: true,
                number: true,
                ..TextRow::default()
            },
        );
        let Outcome::Commit(typed) = outcome else {
            return None;
        };
        match typed.trim().parse::<f64>().ok().filter(|v| v.is_finite()) {
            Some(v) => {
                self.fields.result(key, Ok(()));
                Some(v)
            }
            None => {
                self.fields
                    .refuse(key, &typed, "Not a number. Not applied.".to_owned());
                None
            }
        }
    }

    /// The "Add step" row and the new step's values.
    fn add_step(&mut self, ui: &mut egui::Ui, reference: &ModelRef) -> Option<Command> {
        let mut command = None;
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("new step")
                .selected_text(self.new_step.kind())
                .show_ui(ui, |ui| {
                    let options = [
                        NewStep::Centre,
                        NewStep::Orient(AxisDirection {
                            axis: Axis::Z,
                            negative: false,
                        }),
                        NewStep::Translate([0.0; 3]),
                        NewStep::Rotate(Axis::X, 90.0),
                        NewStep::Scale(1.0),
                        NewStep::Mirror(Axis::X),
                    ];
                    for option in options {
                        let selected = self.new_step.kind() == option.kind();
                        if ui.selectable_label(selected, option.kind()).clicked() && !selected {
                            self.new_step = option;
                        }
                    }
                });
            if ui.button("Add step").clicked() {
                let mut steps = reference.transform.clone();
                steps.extend(self.new_step.steps());
                command = Some(Command::SetModelTransform {
                    model: reference.id,
                    transform: steps,
                });
            }
        });
        ui.horizontal(|ui| match &mut self.new_step {
            NewStep::Translate(v) => {
                for (value, label) in v.iter_mut().zip(["X", "Y", "Z"]) {
                    ui.label(label);
                    ui.add(egui::DragValue::new(value).speed(0.1).suffix(" mm"));
                }
            }
            NewStep::Rotate(axis, angle) => {
                ui.label("about");
                axis_combo(ui, "rotate axis", axis);
                ui.add(egui::DragValue::new(angle).speed(1.0).suffix("\u{b0}"));
            }
            NewStep::Scale(factor) => {
                ui.label("factor");
                ui.add(egui::DragValue::new(factor).speed(0.01).range(1e-6..=1e6));
            }
            NewStep::Mirror(axis) => {
                ui.label("across");
                axis_combo(ui, "mirror axis", axis);
            }
            NewStep::Orient(direction) => {
                ui.label("model direction");
                egui::ComboBox::from_id_salt("orient sign")
                    .selected_text(if direction.negative { "\u{2212}" } else { "+" })
                    .width(30.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut direction.negative, false, "+");
                        ui.selectable_value(&mut direction.negative, true, "\u{2212}");
                    });
                axis_combo(ui, "orient axis", &mut direction.axis);
                ui.label("along the rotary axis");
            }
            NewStep::Centre => {}
        });
        command
    }

    /// One row per kind of finding with its count, or a tick; the sentence
    /// that the check changes nothing; "Open full report" (IMP-04; 11.2).
    fn check_section(&mut self, ui: &mut egui::Ui, inspection: Option<&ModelInspection>) {
        let Some(i) = inspection else {
            self.checking(ui);
            return;
        };
        let problems = i.report.problems();
        if problems.is_empty() {
            ui.horizontal(|ui| {
                crate::shell::tick(ui);
                ui.label("No problems found.");
            });
        }
        for p in &problems {
            ui.horizontal_wrapped(|ui| {
                crate::shell::mark(ui, openchuck_app::overview::ItemState::Warning, p);
                ui.label(RichText::new(p).color(DARK.warning));
            });
        }
        ui.label(
            RichText::new("The check changes nothing in the model or its file.").color(DARK.muted),
        );
        if ui.link("Open full report").clicked() {
            self.report_open = true;
        }
    }

    /// The full check report, in a window of its own.
    fn report_window(&mut self, ctx: &egui::Context, inspection: Option<&ModelInspection>) {
        if !self.report_open {
            return;
        }
        let Some(i) = inspection else { return };
        egui::Window::new("Mesh check report")
            .open(&mut self.report_open)
            .default_width(460.0)
            .show(ctx, |ui| {
                ui.label(format!("{}", i.path.display()));
                ui.label(format!(
                    "{} triangles, {} vertices, surface {} mm\u{b2}",
                    i.summary.triangles,
                    i.summary.vertices,
                    field_row::number(i.summary.area, Some(3))
                ));
                ui.separator();
                let problems = i.report.problems();
                if problems.is_empty() {
                    ui.label("No problems found.");
                }
                for p in problems {
                    ui.label(p);
                }
                for note in &i.notes {
                    ui.label(RichText::new(note).color(DARK.muted));
                }
            });
    }

    /// The content hash, "Replace file…" and "Show in folder" (PRJ-02;
    /// 11.2). Returns true when the user asked to choose a replacement.
    fn source_section(
        &mut self,
        ui: &mut egui::Ui,
        app: &openchuck_app::Application,
        reference: &ModelRef,
    ) -> bool {
        let hash = reference.hash.to_string();
        let short: String = hash.chars().take(20).collect();
        field_row::read_only_row(
            ui,
            "Content hash",
            "The hash of the file as it was added; a changed file is reported.",
            RichText::new(format!("{short}\u{2026}")).monospace(),
        )
        .on_hover_text(&hash);
        let mut choose = false;
        ui.horizontal(|ui| {
            if self.replacing.is_some() {
                ui.spinner();
                ui.label(RichText::new("Reading the file\u{2026}").color(DARK.muted));
            } else if ui
                .button("Replace file\u{2026}")
                .on_hover_text("Use another file for this model, keeping its unit and placement")
                .clicked()
            {
                choose = true;
            }
            if ui.button("Show in folder").clicked() {
                let file = app.location().join(reference.path.as_str());
                let folder = file.parent().map(std::path::Path::to_path_buf);
                self.folder_error = folder
                    .map_or(Err("the file has no folder".to_owned()), |f| {
                        crate::machine_panel::open_folder(&f)
                    })
                    .err();
            }
        });
        for e in [&self.replace_error, &self.folder_error]
            .into_iter()
            .flatten()
        {
            ui.label(RichText::new(e).color(DARK.error));
        }
        choose
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn req_imp_05_new_steps_become_placement_steps() {
        assert_eq!(NewStep::Centre.steps(), [TransformStep::CentreOnRotaryAxis]);
        assert_eq!(
            NewStep::Translate([1.0, 2.0, 3.0]).steps(),
            [TransformStep::Translate {
                x: 1.0,
                y: 2.0,
                z: 3.0
            }]
        );
        // Orienting is the same steps the CLI's `orient=` adds.
        let z = AxisDirection {
            axis: Axis::Z,
            negative: false,
        };
        assert_eq!(NewStep::Orient(z).steps(), orient_to_rotary_axis(z));
        assert_eq!(
            NewStep::Rotate(Axis::Y, 90.0).steps(),
            [TransformStep::Rotate {
                axis: Axis::Y,
                angle: Degrees(90.0)
            }]
        );
    }

    #[test]
    fn req_imp_05_steps_reorder_and_read_in_words() {
        let steps = [
            TransformStep::CentreOnRotaryAxis,
            TransformStep::Scale { factor: 2.0 },
            TransformStep::Mirror { axis: Axis::Y },
        ];
        let reordered = moved(&steps, 2, 0);
        assert_eq!(reordered[0], TransformStep::Mirror { axis: Axis::Y });
        assert_eq!(reordered[1], TransformStep::CentreOnRotaryAxis);
        assert_eq!(moved(&steps, 0, 9), steps.to_vec());
        assert_eq!(
            describe(&TransformStep::Rotate {
                axis: Axis::X,
                angle: Degrees(90.0)
            }),
            "Rotate 90.00\u{b0} about X"
        );
        assert_eq!(
            describe(&TransformStep::Translate {
                x: 1.0,
                y: 0.0,
                z: -2.5
            }),
            "Translate X 1.000 Y 0.000 Z -2.500 mm"
        );
    }

    #[test]
    fn req_imp_03_the_unit_says_where_it_came_from() {
        use openchuck_app::model::Units;
        let file = Units {
            file: Some(LengthUnit::Inch),
            applied: LengthUnit::Millimetre,
            source: UnitSource::Default,
        };
        assert!(
            unit_source(&file).contains("the file names in"),
            "{}",
            unit_source(&file)
        );
        let user = Units {
            file: None,
            applied: LengthUnit::Inch,
            source: UnitSource::User,
        };
        assert_eq!(unit_source(&user), "Read in in: chosen by you.");
    }
}
