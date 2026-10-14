// SPDX-License-Identifier: AGPL-3.0-or-later
//! The import dialog (IMP-03, IMP-04, IMP-05; OI-51; work breakdown slice 8
//! item 6; decision record 0029 item 13).
//!
//! The file is read, placed and checked by `openchuck-app` as a job each time
//! the units or the orientation change. The dialog shows the size, the STL
//! tessellation statement, the unit question and the check report; the
//! viewport shows the placed model with the listed triangles and edges in
//! red. Nothing reaches the project until "Add to project", which adds the
//! model with its units and placement as one undoable step.

use std::path::PathBuf;

use eframe::egui::{self, Color32, RichText};
use openchuck_app::model::{
    Axis, AxisDirection, ImportPreview, LengthUnit, PreparedModel, TransformStep,
    orient_to_rotary_axis,
};
use openchuck_app::{AppError, Application, JobHandle};

use crate::floating::{WindowEvent, floating};

const PROBLEM: Color32 = Color32::from_rgb(230, 80, 80);

/// How the model is placed on import.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Orientation {
    /// As the file has it.
    AsInFile,
    /// Centred on the rotary axis (model X).
    Centred,
    /// This direction of the file turned onto the rotary axis, and centred.
    Along(AxisDirection),
}

impl Orientation {
    fn steps(self) -> Vec<TransformStep> {
        match self {
            Self::AsInFile => Vec::new(),
            Self::Centred => vec![TransformStep::CentreOnRotaryAxis],
            Self::Along(direction) => orient_to_rotary_axis(direction),
        }
    }

    fn name(self) -> String {
        match self {
            Self::AsInFile => "As in the file".to_owned(),
            Self::Centred => "Centred on the rotary axis".to_owned(),
            Self::Along(d) => format!(
                "File {}{} along the rotary axis, centred",
                if d.negative { "-" } else { "+" },
                match d.axis {
                    Axis::X => "X",
                    Axis::Y => "Y",
                    Axis::Z => "Z",
                }
            ),
        }
    }

    fn all() -> Vec<Self> {
        let mut all = vec![Self::AsInFile, Self::Centred];
        for axis in [Axis::X, Axis::Y, Axis::Z] {
            for negative in [false, true] {
                all.push(Self::Along(AxisDirection { axis, negative }));
            }
        }
        all
    }
}

/// What the user chose in the dialog this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportAction {
    /// Keep the dialog open.
    None,
    /// Add the model to the project.
    Add,
    /// Close without adding.
    Cancel,
}

/// An import in progress.
pub struct ImportSession {
    /// The model file.
    pub path: PathBuf,
    /// The unit it is read in; `None` uses the file's own (millimetres when
    /// it names none).
    pub units: Option<LengthUnit>,
    /// How it is placed.
    pub orientation: Orientation,
    job: Option<JobHandle<ImportPreview>>,
    /// The latest check, once one has finished.
    pub preview: Option<ImportPreview>,
    /// Why the latest check or the add failed.
    pub error: Option<String>,
    /// The preview changed since the viewport last showed it.
    pub fresh: bool,
    /// The model being read and checked for adding, off the UI thread.
    adding: Option<JobHandle<PreparedModel>>,
}

impl std::fmt::Debug for ImportSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImportSession")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

impl ImportSession {
    /// Starts importing `path`.
    pub fn new(app: &Application, path: PathBuf) -> Self {
        let mut session = Self {
            path,
            units: None,
            orientation: Orientation::Centred,
            job: None,
            preview: None,
            error: None,
            fresh: false,
            adding: None,
        };
        session.check(app);
        session
    }

    /// The placement steps chosen.
    pub fn steps(&self) -> Vec<TransformStep> {
        self.orientation.steps()
    }

    fn check(&mut self, app: &Application) {
        if let Some(old) = self.job.take() {
            old.cancel();
        }
        self.job = Some(app.start_import_preview(&self.path, self.units, &self.steps()));
    }

    /// Takes the check's result once it is ready. Returns true while a check runs.
    pub fn poll(&mut self) -> bool {
        let Some(job) = &self.job else { return false };
        match job.try_result() {
            None => true,
            Some(result) => {
                self.job = None;
                match result {
                    Ok(preview) => {
                        self.preview = Some(preview);
                        self.error = None;
                        self.fresh = true;
                    }
                    Err(AppError::Engine(e)) if e.to_string().contains("cancel") => {}
                    Err(e) => {
                        self.preview = None;
                        self.error = Some(e.to_string());
                    }
                }
                false
            }
        }
    }

    /// Starts reading and checking the model for adding, as a background
    /// job, so the window stays responsive on a large file.
    pub fn start_adding(&mut self, app: &Application) {
        match app.start_prepare_model(&self.path, self.units, &self.steps()) {
            Ok(job) => self.adding = Some(job),
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    /// The prepared model once the add job ends; `None` while it runs or when
    /// none was started.
    pub fn poll_adding(&mut self) -> Option<Result<PreparedModel, AppError>> {
        let result = self.adding.as_ref()?.try_result()?;
        self.adding = None;
        Some(result)
    }

    /// Whether an add is in progress.
    pub fn is_adding(&self) -> bool {
        self.adding.is_some()
    }

    /// Draws the dialog.
    pub fn window(&mut self, ctx: &egui::Context, app: &Application) -> ImportAction {
        let mut action = ImportAction::None;
        let mut changed = false;
        // Its own window, so it can be moved off the main one (OI-130).
        let ((), event) = floating(ctx, "import model", "Import model", [440.0, 620.0], |ui| {
            ui.label(self.path.display().to_string());
            if self.job.is_some() || self.adding.is_some() {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(if self.adding.is_some() {
                        "Adding…"
                    } else {
                        "Checking…"
                    });
                });
            }
            if let Some(error) = &self.error {
                ui.colored_label(PROBLEM, error);
            }
            ui.separator();

            egui::Grid::new("import options")
                    .num_columns(2)
                    .show(ui, |ui| {
                        ui.label("Read in");
                        let label = self.units.map_or_else(
                            || "The file's unit (mm if it names none)".to_owned(),
                            |u| u.short_name().to_owned(),
                        );
                        egui::ComboBox::from_id_salt("import units")
                            .selected_text(label)
                            .show_ui(ui, |ui| {
                                changed |= ui
                                    .selectable_value(&mut self.units, None, "The file's unit")
                                    .changed();
                                for unit in LengthUnit::ALL {
                                    changed |= ui
                                        .selectable_value(
                                            &mut self.units,
                                            Some(unit),
                                            unit.short_name(),
                                        )
                                        .changed();
                                }
                            });
                        ui.end_row();
                        ui.label("Placement").on_hover_text(
                        "Moves the model in the 3D view behind this dialog; the orange line is the rotary axis.",
                    );
                        egui::ComboBox::from_id_salt("import orientation")
                            .selected_text(self.orientation.name())
                            .show_ui(ui, |ui| {
                                for o in Orientation::all() {
                                    changed |= ui
                                        .selectable_value(&mut self.orientation, o, o.name())
                                        .changed();
                                }
                            });
                        ui.end_row();
                    });

            ui.weak(
                "The 3D view previews the model as it will be added: the placement moves it \
                     there, and the orange line is the rotary axis.",
            );
            if let Some(preview) = &self.preview {
                let i = &preview.inspection;
                if let Some(question) = &i.unit_question {
                    ui.colored_label(Color32::from_rgb(230, 180, 60), question.message());
                }
                ui.label(format!("Units: {}", i.units.describe()));
                if let Some(b) = i.summary.bounds {
                    ui.label(format!(
                        "Size: {:.3} x {:.3} x {:.3} mm (X is the rotary axis)",
                        b.max.x - b.min.x,
                        b.max.y - b.min.y,
                        b.max.z - b.min.z
                    ));
                }
                if let Some(e) = i.extent {
                    ui.label(format!(
                        "Largest radius about the rotary axis: {:.3} mm",
                        e.radius.0
                    ));
                }
                ui.label(format!(
                    "{} triangles, {} piece(s)",
                    i.summary.triangles, i.report.shells
                ));
                for note in &i.notes {
                    ui.small(note);
                }
                ui.separator();
                let problems = i.report.problems();
                if problems.is_empty() {
                    ui.label("Checks: no problems found.");
                } else {
                    ui.label(
                        RichText::new("Checks found problems, shown in red in the view:")
                            .color(PROBLEM),
                    );
                    for p in &problems {
                        ui.label(RichText::new(format!("• {p}")).color(PROBLEM));
                    }
                }
            }
            ui.separator();
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        self.preview.is_some() && self.job.is_none() && self.adding.is_none(),
                        egui::Button::new("Add to project"),
                    )
                    .clicked()
                {
                    action = ImportAction::Add;
                }
                if ui.button("Cancel").clicked() {
                    action = ImportAction::Cancel;
                }
            });
        });
        if changed {
            self.check(app);
        }
        if event == WindowEvent::CloseRequested {
            action = ImportAction::Cancel;
        }
        action
    }
}
