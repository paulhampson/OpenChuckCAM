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
use crate::roles;
use crate::theme::DARK;

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
            Self::AsInFile => "Keep as in the file".to_owned(),
            Self::Centred => "Centre on the rotary axis".to_owned(),
            Self::Along(d) => format!(
                "Turn the file's {}{} onto the rotary axis, centred",
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
    /// The model file; empty until one is chosen.
    pub path: PathBuf,
    /// The path as typed in the window's field.
    pub(crate) path_text: String,
    /// The mesh check row selected, whose triangles the view highlights.
    pub selected: Option<Finding>,
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
        let path_text = path.display().to_string();
        let mut session = Self {
            path,
            path_text,
            selected: None,
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

    /// An import with no file chosen yet: the window asks for the path
    /// (specification 11.2).
    pub fn blank() -> Self {
        Self {
            path: PathBuf::new(),
            path_text: String::new(),
            selected: None,
            units: None,
            orientation: Orientation::Centred,
            job: None,
            preview: None,
            error: None,
            fresh: false,
            adding: None,
        }
    }

    /// Checks the file the path field names.
    fn submit_path(&mut self, app: &Application) {
        let text = self.path_text.trim();
        if text.is_empty() {
            return;
        }
        self.path = PathBuf::from(text);
        self.preview = None;
        self.error = None;
        self.selected = None;
        // A unit chosen for the previous file does not apply to this one.
        self.units = None;
        self.check(app);
    }

    /// Takes `path` as the file to check, as a dropped file does.
    pub fn set_path(&mut self, app: &Application, path: PathBuf) {
        self.path_text = path.display().to_string();
        self.submit_path(app);
    }

    /// The placement steps chosen.
    pub fn steps(&self) -> Vec<TransformStep> {
        self.orientation.steps()
    }

    fn check(&mut self, app: &Application) {
        if self.path.as_os_str().is_empty() {
            return;
        }
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
                    Err(e) if e.is_cancelled() => {}
                    Err(e) => {
                        self.preview = None;
                        self.error = Some(e.to_string());
                    }
                }
                false
            }
        }
    }

    /// Whether Add may be pressed: the file is checked and nothing is
    /// running. Add imports the file that was checked, so the path field
    /// must still name it (OI-358).
    pub(crate) fn can_add(&self) -> bool {
        self.preview.is_some()
            && self.job.is_none()
            && self.adding.is_none()
            && *self.path_text.trim() == *self.path
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

    /// Draws the window: the file, the unit, the placement, the mesh check
    /// and the footer (specification 11.2). The preview is the main view
    /// behind it; [`Self::overlays`] draws its legend and readout.
    pub fn window(&mut self, ctx: &egui::Context, app: &Application) -> ImportAction {
        let mut action = ImportAction::None;
        let mut changed = false;
        let mut submit = false;
        // Its own window, so it can be moved off the main one (OI-130).
        let ((), event) = floating(ctx, "import model", "Import model", [440.0, 620.0], |ui| {
            ui.label(RichText::new("FILE").small().strong().color(DARK.muted));
            ui.horizontal(|ui| {
                let field = ui.add(
                    egui::TextEdit::singleline(&mut self.path_text)
                        .desired_width(ui.available_width() - 140.0)
                        .hint_text("Path of an STL file"),
                );
                // The file being added is the one checked: no new path meanwhile.
                let busy = self.adding.is_some();
                if ui
                    .add_enabled(!busy, egui::Button::new("Browse…"))
                    .clicked()
                    && let Some(picked) = crate::shell::pick_model(&self.path_text, Some(app))
                {
                    self.path_text = picked;
                    submit = true;
                }
                let enter =
                    !busy && field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                let ready = !busy && !self.path_text.trim().is_empty();
                if ui.add_enabled(ready, egui::Button::new("Check")).clicked() || enter {
                    submit = true;
                }
            });
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
                ui.colored_label(DARK.error, error);
            }
            ui.separator();

            if let Some(preview) = &self.preview {
                let i = &preview.inspection;
                ui.label(RichText::new("1 UNITS").small().strong().color(DARK.muted));
                if let Some(question) = &i.unit_question {
                    let colours = DARK.warning_box;
                    egui::Frame::new()
                        .fill(colours.bg)
                        .stroke(egui::Stroke::new(1.0, colours.border))
                        .corner_radius(3.0)
                        .inner_margin(8.0)
                        .show(ui, |ui| {
                            ui.colored_label(colours.text, question.message());
                        });
                }
                let used = self.units.unwrap_or(i.units.applied);
                ui.horizontal(|ui| {
                    for unit in LengthUnit::ALL {
                        if ui
                            .selectable_label(used == unit, unit.short_name())
                            .clicked()
                            && self.units != Some(unit)
                        {
                            self.units = Some(unit);
                            changed = true;
                        }
                    }
                });
                if let Some(b) = i.summary.bounds {
                    let largest = (b.max.x - b.min.x)
                        .max(b.max.y - b.min.y)
                        .max(b.max.z - b.min.z);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Largest side after conversion").color(DARK.muted));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(RichText::new(format!("{largest:.1} mm")).monospace());
                        });
                    });
                }
                ui.weak(i.units.describe());
                ui.separator();
            }

            ui.label(
                RichText::new("2 PLACEMENT")
                    .small()
                    .strong()
                    .color(DARK.muted),
            );
            egui::ComboBox::from_id_salt("import orientation")
                .width(ui.available_width())
                .selected_text(self.orientation.name())
                .show_ui(ui, |ui| {
                    for o in Orientation::all() {
                        changed |= ui
                            .selectable_value(&mut self.orientation, o, o.name())
                            .changed();
                    }
                });
            ui.weak(
                "Places the model on the rotary axis as it is added; the dashed cyan line in \
                 the view is the axis. The Model step's placement steps change it later.",
            );
            ui.separator();

            if let Some(preview) = &self.preview {
                let i = &preview.inspection;
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("3 MESH CHECK")
                            .small()
                            .strong()
                            .color(DARK.muted),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!("{} triangles", i.summary.triangles))
                                .color(DARK.muted),
                        );
                    });
                });
                for finding_row in mesh_rows(&i.report) {
                    let selected = self.selected == Some(finding_row.kind);
                    ui.horizontal(|ui| {
                        if finding_row.count == 0 {
                            ui.colored_label(DARK.ok, "✔");
                        } else {
                            ui.colored_label(DARK.warning, "▲");
                        }
                        let label = ui.selectable_label(selected, finding_row.name);
                        if label.clicked() && finding_row.count > 0 {
                            self.selected = if selected {
                                None
                            } else {
                                Some(finding_row.kind)
                            };
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(RichText::new(finding_row.count.to_string()).monospace());
                        });
                    });
                }
                ui.weak("The check reports problems and changes nothing.");
                for note in &i.notes {
                    ui.separator();
                    ui.label(RichText::new(note).small());
                }
            }
            ui.separator();
            ui.weak("The file is not changed. Its path and hash are stored in the project.");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let can_add = self.can_add();
                let add =
                    egui::Button::new(RichText::new("Add to project").color(DARK.primary_text))
                        .fill(DARK.primary)
                        .stroke(egui::Stroke::new(1.0, DARK.primary_border));
                if ui.add_enabled(can_add, add).clicked() {
                    action = ImportAction::Add;
                }
                if ui.button("Cancel").clicked() {
                    action = ImportAction::Cancel;
                }
            });
        });
        if submit {
            self.submit_path(app);
        } else if changed {
            self.check(app);
        }
        if event == WindowEvent::CloseRequested {
            action = ImportAction::Cancel;
        }
        action
    }

    /// The preview's overlays in the main view: the legend bottom left, and
    /// the size, largest radius and volume bottom right (specification 11.2).
    pub fn overlays(&self, ctx: &egui::Context, area: egui::Rect) {
        let Some(preview) = &self.preview else { return };
        if !area.is_positive() {
            return;
        }
        let i = &preview.inspection;
        let frame = || {
            egui::Frame::new()
                .fill(roles::OVERLAY_FILL)
                .stroke(egui::Stroke::new(1.0, roles::OVERLAY_BORDER))
                .corner_radius(4.0)
                .inner_margin(egui::Margin::symmetric(10, 6))
        };
        egui::Area::new(egui::Id::new("import legend"))
            .order(egui::Order::Foreground)
            .fixed_pos(area.left_bottom() + egui::vec2(12.0, -12.0))
            .pivot(egui::Align2::LEFT_BOTTOM)
            .show(ctx, |ui| {
                frame().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        legend(ui, "Rotary axis", roles::to_color32(roles::ROTARY_AXIS));
                        legend(ui, "Hole", roles::to_color32(roles::HOLE));
                        legend(ui, "Flipped", roles::to_color32(roles::FINDING));
                    });
                });
            });
        let lines = readout_lines(i);
        egui::Area::new(egui::Id::new("import readout"))
            .order(egui::Order::Foreground)
            .fixed_pos(area.right_bottom() + egui::vec2(-12.0 - 460.0, -12.0))
            .pivot(egui::Align2::RIGHT_BOTTOM)
            .show(ctx, |ui| {
                frame().show(ui, |ui| {
                    egui::Grid::new("import readout grid").show(ui, |ui| {
                        for (name, value) in lines {
                            ui.label(RichText::new(name).color(roles::OVERLAY_MUTED));
                            ui.label(RichText::new(value).monospace());
                            ui.end_row();
                        }
                    });
                });
            });
    }
}

fn legend(ui: &mut egui::Ui, name: &str, colour: Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 2.0, colour);
    ui.label(name);
}

/// Which mesh finding a row of the check is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Finding {
    /// Holes in the surface.
    Holes,
    /// Triangles wound against their neighbours.
    Flipped,
    /// Zero-area triangles.
    Degenerate,
    /// Edges shared by more than two triangles.
    NonManifold,
    /// Triangles whose stated normal disagrees with their winding.
    StatedNormals,
    /// Closed pieces that are inside out.
    InsideOut,
}

/// One row of the mesh check: a kind of finding and how many there are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeshRow {
    /// The kind.
    pub kind: Finding,
    /// Its name in the window.
    pub name: &'static str,
    /// How many were found; 0 shows a tick.
    pub count: usize,
}

/// One row per kind of finding, in the order of the specification's board.
pub fn mesh_rows(report: &openchuck_app::model::MeshReport) -> Vec<MeshRow> {
    let row = |kind, name, count| MeshRow { kind, name, count };
    vec![
        row(Finding::Holes, "Holes", report.holes.len()),
        row(
            Finding::Flipped,
            "Flipped triangles",
            report.flipped_triangles.len(),
        ),
        row(
            Finding::Degenerate,
            "Degenerate triangles",
            report.degenerate_triangles.len(),
        ),
        row(
            Finding::NonManifold,
            "Non-manifold edges",
            report.non_manifold_edges.len(),
        ),
        row(
            Finding::StatedNormals,
            "Stated normals against winding",
            report.normal_mismatches.len(),
        ),
        row(
            Finding::InsideOut,
            "Closed pieces inside out",
            report.inside_out_shells,
        ),
    ]
}

/// The size, largest radius and volume lines of the view's readout.
pub fn readout_lines(i: &openchuck_app::model::ModelInspection) -> Vec<(&'static str, String)> {
    let mut lines = Vec::new();
    if let Some(b) = i.summary.bounds {
        lines.push(("Size X", format!("{:.1} mm", b.max.x - b.min.x)));
        lines.push(("Size Y", format!("{:.1} mm", b.max.y - b.min.y)));
        lines.push(("Size Z", format!("{:.1} mm", b.max.z - b.min.z)));
    }
    if let Some(e) = i.extent {
        lines.push(("Largest radius", format!("{:.1} mm", e.radius.0)));
    }
    lines.push((
        "Volume",
        i.report
            .volume
            .map_or_else(|| "n/a (open mesh)".to_owned(), |v| format!("{v:.0} mm³")),
    ));
    lines
}

#[cfg(test)]
mod tests {
    use openchuck_app::model::MeshReport;

    use super::*;

    #[test]
    fn req_imp_04_the_mesh_check_has_one_row_per_kind_with_its_count() {
        let rows = mesh_rows(&MeshReport::default());
        assert_eq!(rows.len(), 6);
        assert!(rows.iter().all(|r| r.count == 0));
        let report = MeshReport {
            flipped_triangles: vec![3, 4, 9],
            degenerate_triangles: vec![1],
            inside_out_shells: 2,
            ..MeshReport::default()
        };
        let count = |kind| {
            mesh_rows(&report)
                .iter()
                .find(|r| r.kind == kind)
                .unwrap()
                .count
        };
        assert_eq!(count(Finding::Flipped), 3);
        assert_eq!(count(Finding::Degenerate), 1);
        assert_eq!(count(Finding::InsideOut), 2);
        assert_eq!(count(Finding::Holes), 0);
    }

    #[test]
    fn req_imp_05_the_placement_choices_are_named_in_plain_words() {
        let names: Vec<String> = Orientation::all().iter().map(|o| o.name()).collect();
        assert_eq!(names.len(), 8);
        assert_eq!(names[0], "Keep as in the file");
        assert_eq!(names[1], "Centre on the rotary axis");
        assert!(names.iter().all(|n| !n.contains("orange")));
        assert!(Orientation::AsInFile.steps().is_empty());
        assert_eq!(
            Orientation::Centred.steps(),
            vec![TransformStep::CentreOnRotaryAxis]
        );
    }

    #[test]
    fn req_imp_01_a_blank_import_has_no_file_and_checks_nothing() {
        let mut session = ImportSession::blank();
        assert!(session.path.as_os_str().is_empty());
        assert!(!session.poll());
        assert!(session.preview.is_none() && !session.is_adding());
        assert!(session.poll_adding().is_none());
    }
}
