// SPDX-License-Identifier: AGPL-3.0-or-later
//! A project model's properties: its file, with a note when it changed
//! (PRJ-02, OI-45), its unit (IMP-03), its placement steps (IMP-05) and its check
//! report (IMP-04) (work breakdown slice 8 item 6; decision record 0029
//! item 13). Every change is a command, so it can be undone.

use eframe::egui::{self, Color32, RichText};
use openchuck_app::model::{
    Axis, AxisDirection, Degrees, LengthUnit, ModelInspection, TransformStep, orient_to_rotary_axis,
};
use openchuck_app::project::ModelRef;
use openchuck_app::project::id::ModelId;
use openchuck_app::{Command, JobHandle};

const PROBLEM: Color32 = Color32::from_rgb(230, 80, 80);

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

/// The model panel's state between frames.
pub struct ModelPanel {
    model: ModelId,
    new_step: NewStep,
    /// The check of the model as placed now, and what it was run for.
    check: Option<(ModelRef, Result<ModelInspection, String>)>,
    job: Option<(ModelRef, JobHandle<ModelInspection>)>,
}

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

fn axis_combo(ui: &mut egui::Ui, id: &str, axis: &mut Axis) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(axis_name(*axis))
        .width(40.0)
        .show_ui(ui, |ui| {
            for a in [Axis::X, Axis::Y, Axis::Z] {
                ui.selectable_value(axis, a, axis_name(a));
            }
        });
}

impl ModelPanel {
    /// The panel for `model`.
    pub fn new(model: ModelId) -> Self {
        Self {
            model,
            new_step: NewStep::Centre,
            check: None,
            job: None,
        }
    }

    /// The model shown.
    pub fn model(&self) -> ModelId {
        self.model
    }

    /// Draws the panel for `reference` as the project holds it now, starting
    /// a check when it changed. Returns a command to execute, if the user
    /// made a change.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        app: &openchuck_app::Application,
        reference: &ModelRef,
    ) -> Option<Command> {
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
        if self.job.is_some() {
            ui.ctx().request_repaint();
        }

        let mut command = None;
        // A changed file shows in the check's notes and a missing one as its
        // error (PRJ-02), so the file is not hashed every frame.
        ui.heading(reference.path.as_str());
        ui.separator();

        // Units (IMP-03).
        ui.horizontal(|ui| {
            ui.label("Read in");
            let mut units = reference.units;
            egui::ComboBox::from_id_salt("model units")
                .selected_text(units.map_or("the file's unit", LengthUnit::short_name))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut units, None, "the file's unit");
                    for unit in LengthUnit::ALL {
                        ui.selectable_value(&mut units, Some(unit), unit.short_name());
                    }
                });
            if units != reference.units {
                command = Some(Command::SetModelUnits {
                    model: reference.id,
                    units,
                });
            }
        });

        // Placement steps (IMP-05), applied first to last.
        ui.strong("Placement");
        if reference.transform.is_empty() {
            ui.weak("As in the file.");
        }
        for (i, step) in reference.transform.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(format!("{}. {step}", i + 1));
                if ui.small_button("Remove").clicked() {
                    let mut steps = reference.transform.clone();
                    steps.remove(i);
                    command = Some(Command::SetModelTransform {
                        model: reference.id,
                        transform: steps,
                    });
                }
            });
        }
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
                ui.add(egui::DragValue::new(angle).speed(1.0).suffix(" deg"));
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
                    .selected_text(if direction.negative { "-" } else { "+" })
                    .width(30.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut direction.negative, false, "+");
                        ui.selectable_value(&mut direction.negative, true, "-");
                    });
                axis_combo(ui, "orient axis", &mut direction.axis);
                ui.label("along the rotary axis");
            }
            NewStep::Centre => {}
        });
        ui.separator();

        // The check report (IMP-04).
        match &self.check {
            _ if self.job.is_some() => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Checking…");
                });
            }
            Some((_, Ok(i))) => {
                if let Some(question) = &i.unit_question {
                    ui.colored_label(Color32::from_rgb(230, 180, 60), question.message());
                }
                if let Some(b) = i.summary.bounds {
                    ui.label(format!(
                        "Size: {:.3} x {:.3} x {:.3} mm",
                        b.max.x - b.min.x,
                        b.max.y - b.min.y,
                        b.max.z - b.min.z
                    ));
                }
                for note in &i.notes {
                    ui.small(note);
                }
                let problems = i.report.problems();
                if problems.is_empty() {
                    ui.label("Checks: no problems found.");
                }
                for p in problems {
                    ui.label(RichText::new(format!("• {p}")).color(PROBLEM));
                }
            }
            Some((_, Err(e))) => {
                ui.colored_label(PROBLEM, e);
            }
            None => {}
        }
        command
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
}
