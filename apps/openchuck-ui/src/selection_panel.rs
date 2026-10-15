// SPDX-License-Identifier: AGPL-3.0-or-later
//! Picking triangles of the part and the selection panel (UI-04, UI-05,
//! OP-02; work breakdown slice 10 items 1 and 6; decision record 0036).
//!
//! The viewport reports the triangle under the pointer and clicks
//! ([`crate::viewport::PickInput`]); this module turns a click into a
//! selection with the chosen [`PickMode`], keeps the working selection, saves
//! it as a named set, edits sets, and gives an operation's roles to sets.
//! Every project change is a command of `openchuck-app`, so it can be undone;
//! the highlights it asks the viewport to draw are plain meshes from
//! [`triangles_mesh`].

use eframe::egui::{self, Color32, Modifiers, RichText};
use openchuck_app::pick::{PickMode, ScenePick};
use openchuck_app::project::id::{ModelId, OperationId, SelectionSetId};
use openchuck_app::scene::{SceneRole, SceneView, triangles_mesh};
use openchuck_app::{
    Application, JobHandle, SelectionEdit, SelectionRole, SelectionSummary, TriangleSet,
};
use openchuck_viewport::{Colour, Scene, Slot};

use crate::viewport::PickInput;

const PROBLEM: Color32 = Color32::from_rgb(230, 80, 80);
const WORKING: Colour = [1.0, 0.55, 0.0, 1.0];
const SHOWN: Colour = [0.2, 0.8, 0.35, 1.0];
const HOVER: Colour = [1.0, 0.9, 0.2, 1.0];
const NO_EDGES: Colour = [0.0, 0.0, 0.0, 0.0];

/// What a click selects, as the panel's radio buttons name it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Choice {
    Triangle,
    Connected,
    Smooth,
    Direction,
}

/// How a pick joins the working selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Combine {
    /// Start a new selection.
    Replace,
    /// Add to the selection (Shift).
    Add,
    /// Take from the selection (Ctrl or Command).
    Remove,
}

impl Combine {
    /// The way the held keys join a click to the selection.
    pub fn of(modifiers: Modifiers) -> Self {
        if modifiers.command {
            Self::Remove
        } else if modifiers.shift {
            Self::Add
        } else {
            Self::Replace
        }
    }

    /// `current` joined with `picked`.
    pub fn apply(self, current: &mut TriangleSet, picked: TriangleSet) {
        match self {
            Self::Replace => *current = picked,
            Self::Add => current.extend(picked),
            Self::Remove => current.retain(|t| !picked.contains(t)),
        }
    }
}

struct Pending {
    handle: JobHandle<TriangleSet>,
    model: ModelId,
    combine: Combine,
}

/// The panel's state between frames.
pub struct SelectionPanel {
    choice: Choice,
    smooth_angle: f64,
    direction_angle: f64,
    working: TriangleSet,
    working_model: Option<ModelId>,
    name: String,
    rename: String,
    pending: Option<Pending>,
    hover: Option<ScenePick>,
    shown: Option<SelectionSetId>,
    /// The last thing that went wrong or the last notice.
    message: Option<String>,
    /// The highlights need drawing again.
    dirty: bool,
}

impl Default for SelectionPanel {
    fn default() -> Self {
        Self {
            choice: Choice::Smooth,
            smooth_angle: 30.0,
            direction_angle: 10.0,
            working: TriangleSet::new(),
            working_model: None,
            name: String::new(),
            rename: String::new(),
            pending: None,
            hover: None,
            shown: None,
            message: None,
            dirty: true,
        }
    }
}

impl SelectionPanel {
    /// Forgets everything that belongs to the open project.
    pub fn clear(&mut self) {
        *self = Self {
            choice: self.choice,
            smooth_angle: self.smooth_angle,
            direction_angle: self.direction_angle,
            ..Self::default()
        };
    }

    /// The scene was built again: its meshes may differ, so the highlights are
    /// drawn again and the hover is forgotten. A region pick still running was
    /// computed from the previous meshes, so it is cancelled.
    pub fn scene_changed(&mut self) {
        if let Some(pending) = self.pending.take() {
            pending.handle.cancel();
        }
        self.hover = None;
        self.dirty = true;
    }

    /// Chooses the set whose triangles are shown in green.
    pub fn show_set(&mut self, set: Option<SelectionSetId>) {
        if self.shown != set {
            self.shown = set;
            self.dirty = true;
        }
    }

    fn mode(&self) -> PickMode {
        match self.choice {
            Choice::Triangle => PickMode::Triangle,
            Choice::Connected => PickMode::Connected,
            Choice::Smooth => PickMode::SmoothPatch {
                max_edge_angle: self.smooth_angle,
            },
            Choice::Direction => PickMode::SameDirection {
                max_angle: self.direction_angle,
            },
        }
    }

    /// Takes in what the pointer did over the part, and finished fills.
    /// Returns true while a fill is running, so the frame is drawn again.
    pub fn frame(&mut self, app: &Application, input: PickInput) -> bool {
        let hovered = input.hover.map(|h| (h.mesh, h.triangle));
        if hovered != self.hover.map(|h| (h.mesh, h.triangle)) {
            self.dirty = true;
        }
        self.hover = input.hover;
        if let Some((pick, modifiers)) = input.click {
            self.click(app, &pick, Combine::of(modifiers));
        }
        let Some(pending) = &self.pending else {
            return false;
        };
        match pending.handle.try_result() {
            None => true,
            Some(result) => {
                let Some(Pending { model, combine, .. }) = self.pending.take() else {
                    return false;
                };
                match result {
                    Ok(picked) => self.join(model, combine, picked),
                    Err(e) => self.message = Some(e.to_string()),
                }
                false
            }
        }
    }

    fn click(&mut self, app: &Application, pick: &ScenePick, combine: Combine) {
        let Some(model) = pick.model else { return };
        self.message = None;
        // An older region pick must not finish later and overwrite this click.
        if let Some(old) = self.pending.take() {
            old.handle.cancel();
        }
        match self.mode().request(pick) {
            None => self.join(model, combine, TriangleSet::from([pick.triangle])),
            Some(request) => match app.start_select_region(model, &request) {
                Ok(handle) => {
                    self.pending = Some(Pending {
                        handle,
                        model,
                        combine,
                    });
                }
                Err(e) => self.message = Some(e.to_string()),
            },
        }
    }

    fn join(&mut self, model: ModelId, combine: Combine, picked: TriangleSet) {
        // A selection is on one model; picking on another starts again.
        if self.working_model != Some(model) {
            self.working.clear();
            self.working_model = Some(model);
        }
        combine.apply(&mut self.working, picked);
        if self.working.is_empty() {
            self.working_model = None;
        }
        self.dirty = true;
    }

    /// The highlights to draw over the scene when they changed since the last
    /// call: the working selection in orange, the chosen set in green, the
    /// triangle under the pointer in yellow.
    pub fn overlay(&mut self, app: &Application, view: &SceneView) -> Option<Scene> {
        if !std::mem::take(&mut self.dirty) {
            return None;
        }
        let part_of = |model: ModelId| {
            view.meshes
                .iter()
                .find(|m| m.role == SceneRole::Part && m.model == Some(model))
        };
        let mut scene = Scene::default();
        if let Some(model) = self.shown_model(app)
            && let Some(set) = self.shown_triangles(app)
            && let Some(part) = part_of(model)
        {
            scene.add_mesh(&triangles_mesh(&part.mesh, &set), &[], SHOWN, NO_EDGES);
        }
        if let Some(model) = self.working_model
            && let Some(part) = part_of(model)
        {
            scene.add_mesh(
                &triangles_mesh(&part.mesh, &self.working),
                &[],
                WORKING,
                NO_EDGES,
            );
        }
        if let Some(hover) = self.hover
            && let Some(part) = view.meshes.get(hover.mesh)
        {
            let one = TriangleSet::from([hover.triangle]);
            scene.add_mesh(&triangles_mesh(&part.mesh, &one), &[], HOVER, NO_EDGES);
        }
        // The highlights are on the workpiece, which turns with A.
        for mesh in &mut scene.meshes {
            mesh.slot = Slot::Part;
        }
        Some(scene)
    }

    fn shown_model(&self, app: &Application) -> Option<ModelId> {
        let id = self.shown?;
        app.project()
            .selection_sets()
            .iter()
            .find(|s| s.id == id)
            .map(|s| s.model)
    }

    fn shown_triangles(&self, app: &Application) -> Option<TriangleSet> {
        use openchuck_app::project::SelectionTarget;
        let id = self.shown?;
        match &app
            .project()
            .selection_sets()
            .iter()
            .find(|s| s.id == id)?
            .target
        {
            SelectionTarget::Triangles(t) => Some(t.clone()),
            _ => None,
        }
    }

    /// Draws the panel. Returns the text of an error to show in a dialog.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        app: &mut Application,
        summaries: &[SelectionSummary],
        view: &SceneView,
    ) -> Option<String> {
        let mut error = None;
        ui.heading("Selection");
        if view.picking.picker().is_none() {
            ui.label("There is no part to select on. Choose a setup with a part.");
        }
        ui.strong("A click selects");
        for (choice, text, tip) in [
            (
                Choice::Triangle,
                "One triangle",
                "Only the triangle clicked.",
            ),
            (
                Choice::Connected,
                "Connected shell",
                "Every triangle joined to it through shared edges.",
            ),
            (
                Choice::Smooth,
                "Smooth patch",
                "The connected triangles reached without turning more than the edge angle at an edge, such as one face of a block.",
            ),
            (
                Choice::Direction,
                "Same direction",
                "Every triangle facing within the angle of the clicked one, wherever it is on the part.",
            ),
        ] {
            if ui
                .radio_value(&mut self.choice, choice, text)
                .on_hover_text(tip)
                .changed()
            {
                self.message = None;
            }
        }
        match self.choice {
            Choice::Smooth => {
                ui.add(
                    egui::Slider::new(&mut self.smooth_angle, 1.0..=90.0)
                        .text("edge angle")
                        .suffix("°"),
                );
            }
            Choice::Direction => {
                ui.add(
                    egui::Slider::new(&mut self.direction_angle, 1.0..=90.0)
                        .text("angle")
                        .suffix("°"),
                );
            }
            _ => {}
        }
        ui.weak("Click selects. Shift adds. Ctrl removes. Drag to orbit.");
        match self.hover {
            Some(h) => ui.label(format!("Pointing at triangle {}", h.triangle)),
            None => ui.weak("Not pointing at the part"),
        };
        if self.pending.is_some() {
            ui.label("Selecting…");
        }
        if let Some(message) = &self.message {
            ui.colored_label(PROBLEM, message);
        }

        ui.separator();
        ui.strong("Working selection");
        ui.label(format!("{} triangles", self.working.len()));
        ui.horizontal(|ui| {
            if ui
                .add_enabled(!self.working.is_empty(), egui::Button::new("Clear"))
                .clicked()
            {
                self.working.clear();
                self.working_model = None;
                self.dirty = true;
            }
        });
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.name)
                    .hint_text(format!("Selection {}", summaries.len() + 1))
                    .desired_width(140.0),
            );
            let ready = !self.working.is_empty() && self.working_model.is_some();
            if ui
                .add_enabled(ready, egui::Button::new("Save as set"))
                .on_hover_text("Name this selection so operations can use it (UI-05)")
                .clicked()
                && let Some(model) = self.working_model
            {
                let name = if self.name.trim().is_empty() {
                    format!("Selection {}", summaries.len() + 1)
                } else {
                    self.name.trim().to_owned()
                };
                match app.save_selection(&name, model, self.working.clone()) {
                    Ok(id) => {
                        self.name.clear();
                        self.working.clear();
                        self.working_model = None;
                        self.shown = Some(id);
                        self.dirty = true;
                    }
                    Err(e) => error = Some(e.to_string()),
                }
            }
        });

        ui.separator();
        ui.strong(format!("Sets ({})", summaries.len()));
        if summaries.is_empty() {
            ui.weak("None yet. Select triangles and save them as a set.");
        }
        for summary in summaries {
            let chosen = self.shown == Some(summary.id);
            let mut text = RichText::new(format!(
                "{}  ({} {})",
                summary.name, summary.count, summary.kind
            ));
            if summary.problem.is_some() {
                text = text.color(PROBLEM);
            }
            let row = ui.selectable_label(chosen, text);
            if let Some(problem) = &summary.problem {
                row.clone().on_hover_text(problem);
            }
            if row.clicked() {
                self.show_set(if chosen { None } else { Some(summary.id) });
                self.rename = summary.name.clone();
            }
        }
        if let Some(summary) = summaries.iter().find(|s| Some(s.id) == self.shown) {
            ui.separator();
            if let Some(e) = self.set_details(ui, app, summary) {
                error = Some(e);
            }
        }
        error
    }

    fn set_details(
        &mut self,
        ui: &mut egui::Ui,
        app: &mut Application,
        summary: &SelectionSummary,
    ) -> Option<String> {
        let mut error = None;
        ui.strong(&summary.name);
        ui.weak(format!(
            "{} {} on {}",
            summary.count,
            summary.kind,
            summary
                .model_path
                .as_deref()
                .unwrap_or("a model that is gone")
        ));
        if let Some(problem) = &summary.problem {
            ui.colored_label(PROBLEM, format!("⚠ {problem}"));
        }
        if summary.uses.is_empty() {
            ui.weak("No operation uses this set.");
        }
        for u in &summary.uses {
            let roles: Vec<&str> = u.roles.iter().map(|r| r.name()).collect();
            ui.label(format!("Used by {}: {}", u.name, roles.join(", ")));
        }
        let editable = !self.working.is_empty()
            && self.working_model == Some(summary.model)
            && summary.kind == "triangles";
        let edit = |edit: SelectionEdit, app: &mut Application| {
            app.edit_selection(summary.id, edit)
                .err()
                .map(|e| e.to_string())
        };
        ui.horizontal_wrapped(|ui| {
            for (label, tip, kind) in [
                ("Add selection", "Add the working selection to this set", 0),
                (
                    "Remove selection",
                    "Take the working selection out of this set",
                    1,
                ),
                (
                    "Replace with selection",
                    "Replace this set's triangles with the working selection",
                    2,
                ),
            ] {
                if ui
                    .add_enabled(editable, egui::Button::new(label))
                    .on_hover_text(tip)
                    .clicked()
                {
                    let triangles = self.working.clone();
                    let which = match kind {
                        0 => SelectionEdit::Add(triangles),
                        1 => SelectionEdit::Remove(triangles),
                        _ => SelectionEdit::Replace(triangles),
                    };
                    if let Some(e) = edit(which, app) {
                        error = Some(e);
                    }
                    self.dirty = true;
                }
            }
        });
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.rename).desired_width(140.0));
            if ui
                .add_enabled(
                    !self.rename.trim().is_empty() && self.rename.trim() != summary.name,
                    egui::Button::new("Rename"),
                )
                .clicked()
                && let Some(e) = edit(SelectionEdit::Rename(self.rename.trim().to_owned()), app)
            {
                error = Some(e);
            }
        });
        let used = !summary.uses.is_empty();
        if ui
            .add_enabled(!used, egui::Button::new("Delete set"))
            .on_disabled_hover_text("Take the set out of its operations first")
            .clicked()
        {
            match app.delete_selection(summary.id) {
                Ok(()) => {
                    self.shown = None;
                    self.dirty = true;
                }
                Err(e) => error = Some(e.to_string()),
            }
        }
        error
    }
}

/// The selections section of an operation's properties (OP-02): for each
/// role the sets in it, a button to take one out, and a list to add one from.
/// Returns the text of an error to show in a dialog.
pub fn operation_roles(
    ui: &mut egui::Ui,
    app: &mut Application,
    operation: OperationId,
    summaries: &[SelectionSummary],
) -> Option<String> {
    let mut error = None;
    let op = app.project().operation(operation).cloned()?;
    ui.strong("Selections");
    if summaries.is_empty() {
        ui.weak("No selection sets yet. Make some with Select on the part.");
        return None;
    }
    ui.weak("With none in Machine the operation cuts the whole part.");
    let name_of = |id: SelectionSetId| {
        summaries
            .iter()
            .find(|s| s.id == id)
            .map_or_else(|| "(missing set)".to_owned(), |s| s.name.clone())
    };
    for role in SelectionRole::ALL {
        let members: Vec<SelectionSetId> = match role {
            SelectionRole::Machine => op.selections.clone(),
            SelectionRole::Boundary => op.boundary.into_iter().collect(),
            SelectionRole::Avoid => op.avoid.clone(),
            SelectionRole::Check => op.check.clone(),
            _ => Vec::new(),
        };
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(role.name()).strong())
                .on_hover_text(role.description());
            for id in &members {
                let problem = summaries
                    .iter()
                    .find(|s| s.id == *id)
                    .and_then(|s| s.problem.clone());
                let mut text = RichText::new(format!("{} ✕", name_of(*id)));
                if problem.is_some() {
                    text = text.color(PROBLEM);
                }
                let button = ui.small_button(text);
                if let Some(problem) = &problem {
                    button.clone().on_hover_text(format!("⚠ {problem}"));
                }
                if button.on_hover_text("Take it out of this role").clicked()
                    && let Err(e) = app.set_operation_selection(operation, role, *id, false)
                {
                    error = Some(e.to_string());
                }
            }
            let free: Vec<&SelectionSummary> = summaries
                .iter()
                .filter(|s| !members.contains(&s.id))
                .collect();
            if !free.is_empty() {
                egui::ComboBox::from_id_salt(("role", role.name()))
                    .selected_text("add…")
                    .show_ui(ui, |ui| {
                        for s in free {
                            if ui.selectable_label(false, &s.name).clicked()
                                && let Err(e) =
                                    app.set_operation_selection(operation, role, s.id, true)
                            {
                                error = Some(e.to_string());
                            }
                        }
                    });
            }
        });
    }
    error
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn set(items: &[u32]) -> TriangleSet {
        items.iter().copied().collect()
    }

    #[test]
    fn req_ui_04_keys_choose_how_a_click_joins_the_selection() {
        assert_eq!(Combine::of(Modifiers::NONE), Combine::Replace);
        assert_eq!(Combine::of(Modifiers::SHIFT), Combine::Add);
        assert_eq!(Combine::of(Modifiers::COMMAND), Combine::Remove);
        let mut current = set(&[1, 2]);
        Combine::Add.apply(&mut current, set(&[2, 3]));
        assert_eq!(current, set(&[1, 2, 3]));
        Combine::Remove.apply(&mut current, set(&[1, 9]));
        assert_eq!(current, set(&[2, 3]));
        Combine::Replace.apply(&mut current, set(&[7]));
        assert_eq!(current, set(&[7]));
    }

    #[test]
    fn req_ui_04_picking_on_another_model_starts_a_new_selection() {
        let mut panel = SelectionPanel::default();
        let (a, b) = (ModelId::new(), ModelId::new());
        panel.join(a, Combine::Replace, set(&[1, 2]));
        panel.join(a, Combine::Add, set(&[3]));
        assert_eq!(panel.working, set(&[1, 2, 3]));
        panel.join(b, Combine::Add, set(&[9]));
        assert_eq!(panel.working, set(&[9]));
        assert_eq!(panel.working_model, Some(b));
        // Taking away everything leaves no model.
        panel.join(b, Combine::Remove, set(&[9]));
        assert!(panel.working.is_empty());
        assert_eq!(panel.working_model, None);
    }

    /// The triangles of the first mesh the overlay holds.
    fn overlay_triangles(scene: &Scene) -> usize {
        scene.meshes.iter().map(|m| m.indices.len() / 3).sum()
    }

    #[test]
    fn req_ui_05_a_saved_set_is_highlighted_again_after_the_project_is_reopened() {
        use openchuck_app::Threads;
        let dir = std::env::temp_dir().join(format!("openchuck-ui-reopen-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut app = Application::create(&dir, "reopen", Threads::Exactly(2)).unwrap();
        std::fs::create_dir_all(dir.join("models")).unwrap();
        let cube = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../testdata/stl/cube_ascii.stl");
        std::fs::copy(cube, dir.join("models/cube.stl")).unwrap();
        let (model, _) = app.add_model(&dir.join("models/cube.stl"), None).unwrap();
        let id = app.save_selection("Top", model, set(&[2, 3])).unwrap();
        app.save().unwrap();
        drop(app);

        let app = Application::open(&dir, Threads::Exactly(2)).unwrap();
        let view = app.start_scene(None).wait().unwrap();
        let mut panel = SelectionPanel::default();
        // Nothing is chosen, so only the (empty) overlay is drawn.
        assert_eq!(overlay_triangles(&panel.overlay(&app, &view).unwrap()), 0);
        // Choosing the set draws exactly its two triangles, in green.
        panel.show_set(Some(id));
        let scene = panel.overlay(&app, &view).unwrap();
        assert_eq!(overlay_triangles(&scene), 2);
        assert!(
            scene.meshes[0]
                .colour
                .iter()
                .zip(SHOWN)
                .all(|(a, b)| (a - b).abs() < 1e-6)
        );
        // Nothing changed since: nothing to draw again.
        assert!(panel.overlay(&app, &view).is_none());
        // A set made on a model the scene does not show draws nothing.
        let empty_view = SceneView::default();
        panel.scene_changed();
        assert_eq!(
            overlay_triangles(&panel.overlay(&app, &empty_view).unwrap()),
            0
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn req_ui_09_a_scene_rebuild_cancels_a_region_pick_in_flight() {
        use openchuck_app::RegionRequest;
        use openchuck_app::Threads;
        let dir = std::env::temp_dir().join(format!("openchuck-ui-stale-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut app = Application::create(&dir, "stale", Threads::Exactly(2)).unwrap();
        std::fs::create_dir_all(dir.join("models")).unwrap();
        let cube = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../testdata/stl/cube_ascii.stl");
        std::fs::copy(cube, dir.join("models/cube.stl")).unwrap();
        let (model, _) = app.add_model(&dir.join("models/cube.stl"), None).unwrap();
        let handle = app
            .start_select_region(model, &RegionRequest::FloodFill { seeds: vec![0] })
            .unwrap();
        let mut panel = SelectionPanel {
            pending: Some(Pending {
                handle,
                model,
                combine: Combine::Replace,
            }),
            ..SelectionPanel::default()
        };
        panel.scene_changed();
        assert!(panel.pending.is_none());
        // The next frame applies nothing from the old scene.
        assert!(!panel.frame(&app, PickInput::default()));
        assert!(panel.working.is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn req_ui_09_a_single_triangle_click_cancels_a_region_pick_in_flight() {
        use openchuck_app::RegionRequest;
        use openchuck_app::Threads;
        use openchuck_app::setup::Point3;
        let dir = std::env::temp_dir().join(format!("openchuck-ui-click-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut app = Application::create(&dir, "click", Threads::Exactly(2)).unwrap();
        std::fs::create_dir_all(dir.join("models")).unwrap();
        let cube = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../testdata/stl/cube_ascii.stl");
        std::fs::copy(cube, dir.join("models/cube.stl")).unwrap();
        let (model, _) = app.add_model(&dir.join("models/cube.stl"), None).unwrap();
        let handle = app
            .start_select_region(model, &RegionRequest::FloodFill { seeds: vec![0] })
            .unwrap();
        let mut panel = SelectionPanel {
            pending: Some(Pending {
                handle,
                model,
                combine: Combine::Replace,
            }),
            choice: Choice::Triangle,
            ..SelectionPanel::default()
        };
        let pick = ScenePick {
            mesh: 0,
            model: Some(model),
            triangle: 5,
            distance: 1.0,
            point: Point3::new(0.0, 0.0, 0.0),
            normal: None,
        };
        panel.click(&app, &pick, Combine::Replace);
        // The newer click stands alone; the older job cannot overwrite it.
        assert!(panel.pending.is_none());
        assert_eq!(panel.working, set(&[5]));
        let _ = std::fs::remove_dir_all(dir);
    }
}
