// SPDX-License-Identifier: AGPL-3.0-or-later
//! Picking the part and the selection panel (UI-04, UI-05, OP-02; work
//! breakdown slice 10 items 1 and 6 and slice 11 items 5 and 7; decision
//! records 0036 and 0066).
//!
//! The viewport reports the triangle under the pointer, or the edge near it,
//! and clicks ([`crate::viewport::PickInput`]); this module turns a click into
//! a selection with the chosen [`PickMode`], keeps the working selection,
//! saves it as a named set, edits sets, and gives an operation's roles to
//! sets. A mesh is selected by triangle or by the edges of its triangles, a
//! STEP part by face or by its own edges: the box on the view chooses
//! surfaces or edges. Every project change is a command of `openchuck-app`,
//! so it can be undone; the highlights it asks the viewport to draw are plain
//! meshes from [`triangles_mesh`] and the edges' polylines.

use eframe::egui::{self, Color32, Modifiers, RichText};
use openchuck_app::pick::{EdgeRef, PickMode, SceneEdgePick, ScenePick};
use openchuck_app::project::id::{ModelId, OperationId, SelectionSetId};
use openchuck_app::project::{ContentHash, SelectionTarget};
use openchuck_app::scene::{SceneMesh, SceneRole, SceneView, triangles_mesh};
use openchuck_app::{
    Application, JobHandle, SelectionEdit, SelectionRole, SelectionSummary, TriangleSet,
};
use openchuck_viewport::{Colour, Scene, Slot};

use crate::viewport::PickInput;

const PROBLEM: Color32 = Color32::from_rgb(230, 80, 80);
/// Working selection: blue `#3d8bff` at 40%, a fill (specification 7.4;
/// decision 0038 amends 0037 item 5).
const WORKING: Colour = [0.239, 0.545, 1.0, FILL_ALPHA];
/// The triangle under the pointer: a yellow `#ffe066` outline.
const HOVER: Colour = [1.0, 0.878, 0.4, 1.0];
const NO_EDGES: Colour = [0.0, 0.0, 0.0, 0.0];
/// How much of a selection's colour shows over the surface.
const FILL_ALPHA: f32 = 0.4;

/// The fixed set of eight swatches a saved set takes its colour from, by its
/// place in the project's list of sets (so up to eight sets are told apart).
/// None is the working blue, the hover yellow or the avoid red.
const SWATCHES: [[u8; 3]; 8] = [
    [0x2d, 0x6c, 0xb5],
    [0xa0, 0x4a, 0x46],
    [0x7d, 0x5b, 0xb5],
    [0x3f, 0x9c, 0x8a],
    [0xb5, 0x8a, 0x2d],
    [0x6f, 0x9a, 0x3a],
    [0xb5, 0x56, 0x8f],
    [0x7a, 0x85, 0x91],
];

/// The swatch of the set at `index` in the project's list.
fn swatch(index: usize) -> [u8; 3] {
    SWATCHES
        .get(index % SWATCHES.len())
        .copied()
        .unwrap_or_default()
}

/// The colour of a set: the one the user chose (`#rrggbb`), or the default
/// swatch of its place in the list.
fn set_rgb(chosen: Option<&str>, index: usize) -> [u8; 3] {
    chosen
        .and_then(|c| {
            let byte = |i: usize| u8::from_str_radix(c.get(i..i + 2)?, 16).ok();
            Some([byte(1)?, byte(3)?, byte(5)?])
        })
        .unwrap_or_else(|| swatch(index))
}

fn hex(rgb: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2])
}

/// The fill colour of a set's colour.
fn rgb_fill([r, g, b]: [u8; 3]) -> Colour {
    [
        f32::from(r) / 255.0,
        f32::from(g) / 255.0,
        f32::from(b) / 255.0,
        FILL_ALPHA,
    ]
}

/// What a key chooses (specification section 15): Q one triangle, W flood
/// fill (connected shell), E select by angle (smooth patch). R, same
/// direction, is added by the owner (decision 0046 item 4); the
/// specification lists only Q, W, E and B. B, box selection, is not built
/// (OI-201).
fn choice_for_key(key: egui::Key) -> Option<Choice> {
    match key {
        egui::Key::Q => Some(Choice::Triangle),
        egui::Key::W => Some(Choice::Connected),
        egui::Key::E => Some(Choice::Smooth),
        egui::Key::R => Some(Choice::Direction),
        _ => None,
    }
}

fn capitalise(text: &str) -> String {
    let mut chars = text.chars();
    chars
        .next()
        .map_or_else(String::new, |c| c.to_uppercase().chain(chars).collect())
}

/// What a click selects, as the panel's radio buttons name it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Choice {
    Triangle,
    Connected,
    Smooth,
    Direction,
}

impl Choice {
    const ALL: [Self; 4] = [
        Self::Triangle,
        Self::Connected,
        Self::Smooth,
        Self::Direction,
    ];

    fn name(self) -> &'static str {
        match self {
            Self::Triangle => "Select one",
            Self::Connected => "Flood fill",
            Self::Smooth => "By angle",
            Self::Direction => "Same direction",
        }
    }

    /// The name when edges are picked: W follows the chain, E the smooth
    /// chain, and R is not offered (decision record 0066).
    fn edge_name(self) -> &'static str {
        match self {
            Self::Triangle => "Select one",
            Self::Connected => "Chain",
            Self::Smooth => "Smooth chain",
            Self::Direction => "Same direction",
        }
    }

    fn edge_tip(self) -> &'static str {
        match self {
            Self::Triangle => "Only the edge clicked.",
            Self::Connected => {
                "The edge and the edges joined to it end to end, through corners where only two meet."
            }
            Self::Smooth => {
                "The chain onward through each corner to the edge that turns least, while it turns no more than the angle."
            }
            Self::Direction => "Not used for edges.",
        }
    }

    fn key(self) -> Option<egui::Key> {
        match self {
            Self::Triangle => Some(egui::Key::Q),
            Self::Connected => Some(egui::Key::W),
            Self::Smooth => Some(egui::Key::E),
            Self::Direction => Some(egui::Key::R),
        }
    }

    fn tip(self) -> &'static str {
        match self {
            Self::Triangle => "Only the triangle clicked, or on a STEP model the face.",
            Self::Connected => "Everything joined to it through shared edges.",
            Self::Smooth => {
                "Everything joined to it without turning more than the edge angle at an edge, such as one face of a block."
            }
            Self::Direction => {
                "Every triangle, or every face of a STEP model, facing within the angle of the clicked one, wherever it is on the part."
            }
        }
    }
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

    /// `current` joined with `picked`. A selection holds one kind: adding
    /// another kind starts again with it, and taking one away changes nothing.
    pub fn apply(self, current: &mut SelectionTarget, picked: SelectionTarget) {
        fn join<T: Ord>(
            how: Combine,
            current: &mut std::collections::BTreeSet<T>,
            picked: std::collections::BTreeSet<T>,
        ) {
            match how {
                Combine::Replace => *current = picked,
                Combine::Add => current.extend(picked),
                Combine::Remove => current.retain(|t| !picked.contains(t)),
            }
        }
        match (current, picked) {
            (SelectionTarget::Triangles(a), SelectionTarget::Triangles(b)) => join(self, a, b),
            (SelectionTarget::MeshEdges(a), SelectionTarget::MeshEdges(b)) => join(self, a, b),
            (SelectionTarget::Faces(a), SelectionTarget::Faces(b)) => join(self, a, b),
            (SelectionTarget::Edges(a), SelectionTarget::Edges(b)) => join(self, a, b),
            (current, picked) => {
                if self != Self::Remove {
                    *current = picked;
                }
            }
        }
    }
}

/// What the pointer is over.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Hover {
    Surface(ScenePick),
    Edge(SceneEdgePick),
}

/// An empty working selection.
fn nothing() -> SelectionTarget {
    SelectionTarget::Triangles(TriangleSet::new())
}

struct Pending {
    handle: JobHandle<TriangleSet>,
    model: ModelId,
    combine: Combine,
}

/// The panel's state between frames.
pub struct SelectionPanel {
    choice: Choice,
    /// Whether a click picks edges rather than surfaces (decision record
    /// 0066).
    edges: bool,
    smooth_angle: f64,
    direction_angle: f64,
    working: SelectionTarget,
    working_model: Option<ModelId>,
    /// The hash the project recorded for `working_model` when the working
    /// selection began. Triangle numbers mean the file they were picked on, so
    /// if the project accepts a changed file the selection is dropped.
    working_hash: Option<ContentHash>,
    name: String,
    rename: String,
    pending: Option<Pending>,
    hover: Option<Hover>,
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
            edges: false,
            smooth_angle: 30.0,
            direction_angle: 10.0,
            working: nothing(),
            working_model: None,
            working_hash: None,
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
            edges: self.edges,
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

    /// Whether a click picks edges rather than surfaces.
    pub fn picks_edges(&self) -> bool {
        self.edges
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

    /// Takes in what the pointer did over the part in `view`, and finished
    /// fills. Returns true while a fill is running, so the frame is drawn
    /// again.
    pub fn frame(&mut self, app: &Application, view: &SceneView, input: PickInput) -> bool {
        self.drop_stale_working(app);
        let hover = match (input.edge_hover, input.hover) {
            (Some(edge), _) => Some(Hover::Edge(edge)),
            (None, Some(pick)) => Some(Hover::Surface(pick)),
            (None, None) => None,
        };
        let key = |h: Option<Hover>| match h {
            Some(Hover::Surface(p)) => Some((p.mesh, 0, p.triangle as usize)),
            Some(Hover::Edge(e)) => Some((e.mesh, 1, e.index)),
            None => None,
        };
        if key(hover) != key(self.hover) {
            self.dirty = true;
        }
        self.hover = hover;
        if let Some((pick, modifiers)) = input.click {
            self.click(app, view, &pick, Combine::of(modifiers));
        }
        if let Some((pick, modifiers)) = input.edge_click {
            self.click_edge(app, view, &pick, Combine::of(modifiers));
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
                    Ok(picked) => self.join(model, combine, picked, recorded_hash(app, model)),
                    Err(e) => self.message = Some(e.to_string()),
                }
                false
            }
        }
    }

    fn click(&mut self, app: &Application, view: &SceneView, pick: &ScenePick, combine: Combine) {
        let Some(model) = pick.model else { return };
        self.message = None;
        // An older region pick must not finish later and overwrite this click.
        if let Some(old) = self.pending.take() {
            old.handle.cancel();
        }
        // A STEP part is selected by face, from the scene's own data.
        if pick.face.is_some() {
            if let Some(faces) = view
                .picking
                .picker()
                .and_then(|p| p.select_faces(pick, self.mode()))
            {
                self.join(model, combine, faces, recorded_hash(app, model));
            }
            return;
        }
        match self.mode().request(pick) {
            None => self.join(
                model,
                combine,
                TriangleSet::from([pick.triangle]),
                recorded_hash(app, model),
            ),
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

    fn click_edge(
        &mut self,
        app: &Application,
        view: &SceneView,
        pick: &SceneEdgePick,
        combine: Combine,
    ) {
        let Some(model) = pick.model else { return };
        let Some(picker) = view.picking.picker() else {
            return;
        };
        self.message = None;
        if let Some(old) = self.pending.take() {
            old.handle.cancel();
        }
        let edges = picker.select_edges(pick, self.mode());
        let target = match pick.edge {
            EdgeRef::Brep(_) => SelectionTarget::Edges(
                edges
                    .into_iter()
                    .filter_map(|e| match e {
                        EdgeRef::Brep(id) => Some(id),
                        EdgeRef::Mesh(_) => None,
                    })
                    .collect(),
            ),
            EdgeRef::Mesh(_) => SelectionTarget::MeshEdges(
                edges
                    .into_iter()
                    .filter_map(|e| match e {
                        EdgeRef::Mesh(m) => Some(m),
                        EdgeRef::Brep(_) => None,
                    })
                    .collect(),
            ),
        };
        self.join(model, combine, target, recorded_hash(app, model));
    }

    /// `hash` is what the project records for `model` now; the selection keeps
    /// the one it began with.
    fn join(
        &mut self,
        model: ModelId,
        combine: Combine,
        picked: impl Into<SelectionTarget>,
        hash: Option<ContentHash>,
    ) {
        // A selection is on one model; picking on another starts again.
        if self.working_model != Some(model) {
            self.working = nothing();
            self.working_model = Some(model);
            self.working_hash = hash;
        }
        let picked = picked.into();
        combine.apply(&mut self.working, picked);
        if self.working.is_empty() {
            self.working_model = None;
            self.working_hash = None;
        }
        self.dirty = true;
    }

    /// Drops the working selection when the project no longer holds the model
    /// file it was picked on: the numbers would name other triangles, and
    /// saving them would stamp the new file's hash on the wrong ones. Undo,
    /// a new placement or another setup leave the hash alone, so they keep it.
    fn drop_stale_working(&mut self, app: &Application) {
        let Some(model) = self.working_model else {
            return;
        };
        if recorded_hash(app, model) == self.working_hash {
            return;
        }
        self.working = nothing();
        self.working_model = None;
        self.working_hash = None;
        self.dirty = true;
        self.message = Some(
            "The model file changed, so the working selection was cleared; select again."
                .to_owned(),
        );
    }

    /// The highlights to draw over the scene when they changed since the last
    /// call: the chosen set in its swatch, the working selection in blue
    /// (filled surfaces, or edges drawn over the part), and what is under
    /// the pointer outlined in yellow: a triangle, a STEP part's face, or an
    /// edge.
    pub fn overlay(&mut self, app: &Application, view: &SceneView) -> Option<Scene> {
        if !std::mem::take(&mut self.dirty) {
            return None;
        }
        let part_of = |model: ModelId| {
            view.meshes
                .iter()
                .enumerate()
                .find(|(_, m)| m.role == SceneRole::Part && m.model == Some(model))
        };
        let mut scene = Scene::default();
        if let Some(model) = self.shown_model(app)
            && let Some(set) = self.shown_target(app)
            && let Some((index, part)) = part_of(model)
        {
            let sets = app.project().selection_sets();
            let place = sets
                .iter()
                .position(|s| Some(s.id) == self.shown)
                .unwrap_or(0);
            let chosen = sets.get(place).and_then(|s| s.colour.as_deref());
            let rgb = set_rgb(chosen, place);
            draw_target(&mut scene, view, index, part, &set, rgb_fill(rgb), {
                let [r, g, b] = rgb;
                [
                    f32::from(r) / 255.0,
                    f32::from(g) / 255.0,
                    f32::from(b) / 255.0,
                    1.0,
                ]
            });
        }
        if let Some(model) = self.working_model
            && let Some((index, part)) = part_of(model)
        {
            let line = [WORKING[0], WORKING[1], WORKING[2], 1.0];
            draw_target(&mut scene, view, index, part, &self.working, WORKING, line);
        }
        match self.hover {
            Some(Hover::Surface(hover)) => {
                if let Some(part) = view.meshes.get(hover.mesh) {
                    match (hover.face, &part.brep) {
                        // A face is outlined by its edges.
                        (Some(face), Some(brep)) => {
                            for edge in brep.edges.iter().filter(|e| e.faces.contains(&face)) {
                                scene.add_polyline(&edge.points, HOVER, false);
                            }
                        }
                        _ => {
                            if let Some(corners) = part.mesh.triangles.get(hover.triangle as usize)
                            {
                                let outline: Vec<_> = corners
                                    .iter()
                                    .chain(corners.first())
                                    .filter_map(|&i| part.mesh.vertices.get(i as usize).copied())
                                    .collect();
                                if outline.len() == 4 {
                                    scene.add_polyline(&outline, HOVER, false);
                                }
                            }
                        }
                    }
                }
            }
            Some(Hover::Edge(hover)) => {
                if let Some(points) = view
                    .picking
                    .picker()
                    .and_then(|p| p.edge_points(hover.mesh, hover.index))
                {
                    scene.add_polyline(points, HOVER, true);
                }
            }
            None => {}
        }
        // The highlights are on the workpiece, which turns with A.
        for mesh in &mut scene.meshes {
            mesh.slot = Slot::Part;
        }
        for line in &mut scene.lines {
            line.slot = Slot::Part;
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

    fn shown_target(&self, app: &Application) -> Option<SelectionTarget> {
        set_target(app, self.shown?)
    }

    /// Chooses what a click selects from a key (Q, W, E; specification
    /// section 15), and reports Esc. Returns true when Esc asks to leave
    /// picking; a region pick still running is cancelled first and keeps
    /// picking on. Does nothing while a text field has the keyboard; the
    /// caller skips it while a dialog is open.
    pub fn keys(&mut self, ctx: &egui::Context) -> bool {
        if ctx.egui_wants_keyboard_input() {
            return false;
        }
        let pressed = |key: egui::Key| ctx.input_mut(|i| i.consume_key(Modifiers::NONE, key));
        for key in [egui::Key::Q, egui::Key::W, egui::Key::E, egui::Key::R] {
            if pressed(key)
                && let Some(choice) = choice_for_key(key)
            {
                self.choice = choice;
                self.message = None;
            }
        }
        if !pressed(egui::Key::Escape) {
            return false;
        }
        match self.pending.take() {
            Some(pending) => {
                pending.handle.cancel();
                false
            }
            None => true,
        }
    }

    /// The selection tools box, drawn over the top left of the viewport
    /// (specification section 7.2): one button for each way a click selects,
    /// with its key.
    pub fn tools(&mut self, ctx: &egui::Context, corner: egui::Pos2) {
        egui::Area::new(egui::Id::new("selection tools"))
            .order(egui::Order::Foreground)
            .fixed_pos(corner)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(Color32::from_rgba_unmultiplied(16, 18, 21, 225))
                    .corner_radius(4)
                    .inner_margin(6)
                    .show(ui, |ui| {
                        ui.visuals_mut().override_text_color = Some(Color32::from_gray(228));
                        ui.horizontal(|ui| {
                            for (edges, label, tip) in [
                                (
                                    false,
                                    "Surfaces",
                                    "Click selects triangles, or the faces of a STEP model.",
                                ),
                                (
                                    true,
                                    "Edges",
                                    "Click selects edges: the edges drawn on the part.",
                                ),
                            ] {
                                if ui
                                    .selectable_label(self.edges == edges, label)
                                    .on_hover_text(tip)
                                    .clicked()
                                {
                                    self.edges = edges;
                                    self.message = None;
                                    self.hover = None;
                                    self.dirty = true;
                                }
                            }
                        });
                        ui.separator();
                        for choice in Choice::ALL {
                            if self.edges && choice == Choice::Direction {
                                continue;
                            }
                            let key = choice
                                .key()
                                .map_or(String::new(), |k| format!(" ({})", k.name()));
                            let (name, tip) = if self.edges {
                                (choice.edge_name(), choice.edge_tip())
                            } else {
                                (choice.name(), choice.tip())
                            };
                            if ui
                                .selectable_label(self.choice == choice, format!("{name}{key}"))
                                .on_hover_text(tip)
                                .clicked()
                            {
                                self.choice = choice;
                                self.message = None;
                            }
                        }
                    });
            });
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
        self.drop_stale_working(app);
        ui.heading("Selection sets");
        if view.picking.picker().is_none() {
            ui.label("There is no part to select on. Choose a setup with a part.");
        }
        ui.weak("Meshes are selected by triangle or edge; STEP models by face and edge.");
        ui.horizontal_wrapped(|ui| {
            let name = if self.edges {
                self.choice.edge_name()
            } else {
                self.choice.name()
            };
            let what = if self.edges { "edges" } else { "surfaces" };
            ui.label(format!("Click: {}, {what}.", name.to_lowercase()));
            ui.weak("Choose in the box on the view, or press Q, W, E or R.");
        });
        ui.add(
            egui::Slider::new(&mut self.smooth_angle, 1.0..=90.0)
                .text("select by angle within")
                .suffix("°"),
        )
        .on_hover_text("The edge angle for smooth patch: the most a surface may turn at an edge.");
        if self.choice == Choice::Direction {
            ui.add(
                egui::Slider::new(&mut self.direction_angle, 1.0..=90.0)
                    .text("same direction within")
                    .suffix("°"),
            );
        }
        ui.weak("Click selects. Shift adds. Ctrl removes. Drag to orbit. Esc leaves.");
        match self.hover {
            Some(Hover::Surface(ScenePick {
                face: Some(face), ..
            })) => ui.label(format!("Pointing at face {}", face.0)),
            Some(Hover::Surface(h)) => ui.label(format!("Pointing at triangle {}", h.triangle)),
            Some(Hover::Edge(e)) => ui.label(match e.edge {
                EdgeRef::Brep(id) => format!("Pointing at edge {}", id.0),
                EdgeRef::Mesh(m) => format!("Pointing at an edge of triangle {}", m.triangle),
            }),
            None => ui.weak("Not pointing at the part"),
        };
        if self.pending.is_some() {
            ui.label("Selecting…");
        }
        if let Some(message) = &self.message {
            ui.colored_label(PROBLEM, message);
        }

        ui.separator();
        ui.strong(format!(
            "Working selection: {} {}",
            self.working.len(),
            self.working.kind_name()
        ));
        let chosen = summaries.iter().find(|s| Some(s.id) == self.shown);
        let same_kind = chosen
            .and_then(|s| set_target(app, s.id))
            .is_some_and(|t| std::mem::discriminant(&t) == std::mem::discriminant(&self.working));
        let can_edit = chosen.is_some_and(|s| {
            !self.working.is_empty() && self.working_model == Some(s.model) && same_kind
        });
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.name)
                    .hint_text(format!("Selection {}", summaries.len() + 1))
                    .desired_width(120.0),
            );
            let ready = !self.working.is_empty() && self.working_model.is_some();
            if ui
                .add_enabled(ready, egui::Button::new("New set from selection"))
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
                        self.working = nothing();
                        self.working_model = None;
                        self.shown = Some(id);
                        self.dirty = true;
                    }
                    Err(e) => error = Some(e.to_string()),
                }
            }
            if ui
                .add_enabled(!self.working.is_empty(), egui::Button::new("Clear"))
                .clicked()
            {
                self.working = nothing();
                self.working_model = None;
                self.dirty = true;
            }
        });
        ui.horizontal_wrapped(|ui| {
            for (label, tip, kind) in [
                (
                    "Add selection to set",
                    "Add the working selection to the chosen set",
                    0,
                ),
                (
                    "Remove from set",
                    "Take the working selection out of the chosen set",
                    1,
                ),
                (
                    "Replace set",
                    "Replace what the chosen set selects with the working selection",
                    2,
                ),
            ] {
                let enabled = can_edit;
                let button = ui.add_enabled(enabled, egui::Button::new(label));
                if !enabled {
                    button.clone().on_disabled_hover_text(
                        "Choose a set of the same kind on the model you selected on",
                    );
                }
                if button.on_hover_text(tip).clicked()
                    && let Some(set) = chosen
                {
                    let target = self.working.clone();
                    let which = match kind {
                        0 => SelectionEdit::Add(target),
                        1 => SelectionEdit::Remove(target),
                        _ => SelectionEdit::Replace(target),
                    };
                    match app.edit_selection(set.id, which) {
                        Ok(_) => self.dirty = true,
                        Err(e) => error = Some(e.to_string()),
                    }
                }
            }
        });

        ui.separator();
        if summaries.is_empty() {
            ui.weak("No sets yet. Select on the part and make a set from it.");
        } else {
            self.table(ui, app, summaries);
        }
        if let Some(summary) = chosen {
            ui.separator();
            if let Some(e) = self.set_details(ui, app, summary) {
                error = Some(e);
            }
        }
        ui.separator();
        ui.weak("Editing a set marks every operation that uses it stale.");
        error
    }

    /// The table of sets (specification section 12.2): swatch, name, kind,
    /// size, used by, status and the action. A set with a problem is marked
    /// with a triangle as well as red.
    fn table(&mut self, ui: &mut egui::Ui, app: &Application, summaries: &[SelectionSummary]) {
        egui::ScrollArea::horizontal().show(ui, |ui| {
            egui::Grid::new("selection sets")
                .striped(true)
                .num_columns(7)
                .show(ui, |ui| {
                    for head in ["", "Name", "Kind", "Size", "Used by", "Status", ""] {
                        ui.weak(head);
                    }
                    ui.end_row();
                    for (index, summary) in summaries.iter().enumerate() {
                        let chosen = self.shown == Some(summary.id);
                        let [r, g, b] = set_rgb(summary.colour.as_deref(), index);
                        let (rect, _) =
                            ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                        ui.painter()
                            .rect_filled(rect, 2.0, Color32::from_rgb(r, g, b));
                        let name = ui.selectable_label(chosen, &summary.name);
                        if name.clicked() {
                            self.show_set(if chosen { None } else { Some(summary.id) });
                            self.rename = summary.name.clone();
                        }
                        ui.label(capitalise(summary.kind));
                        ui.label(summary.count.to_string());
                        let used: Vec<String> = summary
                            .uses
                            .iter()
                            .map(|u| {
                                let roles: Vec<&str> = u.roles.iter().map(|r| r.name()).collect();
                                format!("{} ({})", u.name, roles.join(", "))
                            })
                            .collect();
                        ui.label(if used.is_empty() {
                            "Not used".to_owned()
                        } else {
                            used.join("; ")
                        });
                        match &summary.problem {
                            Some(problem) => {
                                ui.colored_label(PROBLEM, "▲ Problem")
                                    .on_hover_text(problem);
                            }
                            None => {
                                ui.label("● Matches model");
                            }
                        }
                        let usable = summary.problem.is_none();
                        if usable
                            && ui
                                .link("Select in view")
                                .on_hover_text("Make the set the working selection, to edit it")
                                .clicked()
                            && let Some(t) = set_target(app, summary.id)
                        {
                            self.select_set_in_view(app, summary, t);
                        }
                        ui.end_row();
                    }
                });
        });
    }

    /// Makes `target`, the contents of `summary`'s set, the working
    /// selection and shows the set.
    fn select_set_in_view(
        &mut self,
        app: &Application,
        summary: &SelectionSummary,
        target: SelectionTarget,
    ) {
        // A region pick still running would overwrite what is loaded.
        if let Some(pending) = self.pending.take() {
            pending.handle.cancel();
        }
        // Picking goes on in the kind the set holds.
        self.edges = matches!(
            target,
            SelectionTarget::Edges(_) | SelectionTarget::MeshEdges(_)
        );
        self.working = target;
        self.working_model = Some(summary.model);
        self.working_hash = recorded_hash(app, summary.model);
        self.shown = Some(summary.id);
        self.rename = summary.name.clone();
        self.dirty = true;
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
            ui.colored_label(PROBLEM, format!("▲ {problem}"));
        }
        if summary.uses.is_empty() {
            ui.weak("No operation uses this set.");
        }
        for u in &summary.uses {
            let roles: Vec<&str> = u.roles.iter().map(|r| r.name()).collect();
            ui.label(format!("Used by {}: {}", u.name, roles.join(", ")));
        }
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.rename).desired_width(140.0));
            if ui
                .add_enabled(
                    !self.rename.trim().is_empty() && self.rename.trim() != summary.name,
                    egui::Button::new("Rename"),
                )
                .clicked()
                && let Err(e) = app.edit_selection(
                    summary.id,
                    SelectionEdit::Rename(self.rename.trim().to_owned()),
                )
            {
                error = Some(e.to_string());
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.label("Colour");
            for rgb in SWATCHES {
                let current = summary.colour.as_deref() == Some(hex(rgb).as_str());
                let (r, g, b) = (rgb[0], rgb[1], rgb[2]);
                let button = egui::Button::new(if current { "✓" } else { "  " })
                    .fill(Color32::from_rgb(r, g, b))
                    .min_size(egui::vec2(18.0, 18.0));
                if ui
                    .add(button)
                    .on_hover_text(format!("Show this set in {}", hex(rgb)))
                    .clicked()
                {
                    match app.edit_selection(summary.id, SelectionEdit::Colour(Some(hex(rgb)))) {
                        Ok(_) => self.dirty = true,
                        Err(e) => error = Some(e.to_string()),
                    }
                }
            }
            if ui
                .add_enabled(summary.colour.is_some(), egui::Button::new("Default"))
                .on_hover_text("Take the swatch of the set's place in the list")
                .clicked()
            {
                match app.edit_selection(summary.id, SelectionEdit::Colour(None)) {
                    Ok(_) => self.dirty = true,
                    Err(e) => error = Some(e.to_string()),
                }
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

/// What the set `id` selects.
fn set_target(app: &Application, id: SelectionSetId) -> Option<SelectionTarget> {
    app.project()
        .selection_sets()
        .iter()
        .find(|s| s.id == id)
        .map(|s| s.target.clone())
}

/// Draws `target` on `part` (mesh `index` of `view`): surfaces filled in
/// `fill`, edges as lines in `line` over the part.
fn draw_target(
    scene: &mut Scene,
    view: &SceneView,
    index: usize,
    part: &SceneMesh,
    target: &SelectionTarget,
    fill: Colour,
    line: Colour,
) {
    let edges = |refs: std::collections::BTreeSet<EdgeRef>| {
        view.picking
            .picker()
            .map(|p| {
                p.edges_points(index, &refs)
                    .into_iter()
                    .map(<[_]>::to_vec)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    };
    let lines = match target {
        SelectionTarget::Triangles(t) => {
            scene.add_mesh(&triangles_mesh(&part.mesh, t), &[], fill, NO_EDGES);
            return;
        }
        SelectionTarget::Faces(faces) => {
            if let Some(brep) = &part.brep {
                let triangles = brep.triangles_of(faces);
                scene.add_mesh(&triangles_mesh(&part.mesh, &triangles), &[], fill, NO_EDGES);
            }
            return;
        }
        SelectionTarget::Edges(ids) => edges(ids.iter().map(|i| EdgeRef::Brep(*i)).collect()),
        SelectionTarget::MeshEdges(m) => edges(m.iter().map(|e| EdgeRef::Mesh(*e)).collect()),
    };
    for points in lines {
        scene.add_polyline(&points, line, true);
    }
}

/// What the project records as the hash of `model`'s file, if it has the model.
fn recorded_hash(app: &Application, model: ModelId) -> Option<ContentHash> {
    app.project()
        .models()
        .iter()
        .find(|m| m.id == model)
        .map(|m| m.hash)
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
    #![allow(clippy::unwrap_used, clippy::float_cmp)]

    use super::*;

    fn set(items: &[u32]) -> TriangleSet {
        items.iter().copied().collect()
    }

    fn tri(items: &[u32]) -> SelectionTarget {
        SelectionTarget::Triangles(set(items))
    }

    #[test]
    fn req_ui_04_keys_choose_how_a_click_joins_the_selection() {
        assert_eq!(Combine::of(Modifiers::NONE), Combine::Replace);
        assert_eq!(Combine::of(Modifiers::SHIFT), Combine::Add);
        assert_eq!(Combine::of(Modifiers::COMMAND), Combine::Remove);
        let mut current = tri(&[1, 2]);
        Combine::Add.apply(&mut current, tri(&[2, 3]));
        assert_eq!(current, tri(&[1, 2, 3]));
        Combine::Remove.apply(&mut current, tri(&[1, 9]));
        assert_eq!(current, tri(&[2, 3]));
        Combine::Replace.apply(&mut current, tri(&[7]));
        assert_eq!(current, tri(&[7]));
        // A selection holds one kind: adding faces starts again with them,
        // and taking faces from triangles changes nothing.
        let faces = SelectionTarget::Faces([openchuck_app::pick::FaceId(3)].into());
        Combine::Remove.apply(&mut current, faces.clone());
        assert_eq!(current, tri(&[7]));
        Combine::Add.apply(&mut current, faces.clone());
        assert_eq!(current, faces);
    }

    #[test]
    fn req_ui_14_keys_q_w_e_r_choose_how_a_click_selects() {
        assert_eq!(choice_for_key(egui::Key::Q), Some(Choice::Triangle));
        assert_eq!(choice_for_key(egui::Key::W), Some(Choice::Connected));
        assert_eq!(choice_for_key(egui::Key::E), Some(Choice::Smooth));
        assert_eq!(choice_for_key(egui::Key::R), Some(Choice::Direction));
        assert_eq!(choice_for_key(egui::Key::B), None);
        // Every way a click selects has a key, shown in its tooltip label.
        assert!(Choice::ALL.iter().all(|c| c.key().is_some()));
        // Every key the tools box shows is one that chooses that tool.
        for choice in Choice::ALL {
            if let Some(key) = choice.key() {
                assert_eq!(choice_for_key(key), Some(choice));
            }
        }
        let ctx = egui::Context::default();
        let mut panel = SelectionPanel::default();
        let mut input = egui::RawInput::default();
        input.events.push(egui::Event::Key {
            key: egui::Key::Q,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        });
        let mut output = ctx.run_ui(input, |ui| {
            assert!(!panel.keys(ui.ctx()));
        });
        output.textures_delta.clear();
        assert_eq!(panel.choice, Choice::Triangle);
        let mut input = egui::RawInput::default();
        input.events.push(egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        });
        let mut left = false;
        let mut output = ctx.run_ui(input, |ui| left = panel.keys(ui.ctx()));
        output.textures_delta.clear();
        assert!(left, "Esc leaves picking when no fill is running");
    }

    #[test]
    fn req_ui_05_the_swatches_are_eight_and_none_is_a_highlight_colour() {
        let all: std::collections::BTreeSet<_> = SWATCHES.iter().collect();
        assert_eq!(all.len(), 8);
        assert_eq!(swatch(8), swatch(0));
        let to_u8 = |c: Colour| [0, 1, 2].map(|i| (c[i] * 255.0).round() as u8);
        for s in SWATCHES {
            assert_ne!(s, to_u8(WORKING));
            assert_ne!(s, to_u8(HOVER));
        }
        assert_eq!(to_u8(WORKING), [0x3d, 0x8b, 0xff]);
        assert_eq!(to_u8(HOVER), [0xff, 0xe0, 0x66]);
    }

    #[test]
    fn req_ui_05_a_chosen_colour_replaces_the_default_swatch() {
        assert_eq!(set_rgb(None, 1), swatch(1));
        assert_eq!(set_rgb(Some("#102030"), 1), [0x10, 0x20, 0x30]);
        // A malformed stored value falls back to the swatch, never a panic.
        assert_eq!(set_rgb(Some("#12"), 2), swatch(2));
        assert_eq!(set_rgb(Some("zzzzzzz"), 2), swatch(2));
        assert_eq!(hex([0x2d, 0x6c, 0xb5]), "#2d6cb5");
    }

    #[test]
    fn req_ui_04_hover_is_a_yellow_outline_and_the_working_selection_a_blue_fill() {
        use openchuck_app::project::id::ModelId;
        let (dir, app, model) = cube_project("hover-outline");
        let view = app.start_scene(None).wait().unwrap();
        let mut panel = SelectionPanel::default();
        panel.join(model, Combine::Replace, set(&[0, 1]), None);
        panel.hover = Some(Hover::Surface(ScenePick {
            face: None,
            mesh: view
                .meshes
                .iter()
                .position(|m| m.role == SceneRole::Part)
                .unwrap(),
            model: Some(model),
            triangle: 3,
            distance: 1.0,
            point: openchuck_app::setup::Point3::new(0.0, 0.0, 0.0),
            normal: None,
        }));
        let scene = panel.overlay(&app, &view).unwrap();
        // One blue translucent fill of the two working triangles ...
        assert_eq!(scene.meshes.len(), 1);
        assert_eq!(overlay_triangles(&scene), 2);
        assert_eq!(scene.meshes[0].colour, WORKING);
        assert!(scene.meshes[0].colour[3] < 1.0);
        // ... and the hovered triangle as a closed yellow loop of 3 sides
        // (6 end points), not a fill.
        assert_eq!(scene.lines.len(), 1);
        assert_eq!(scene.lines[0].colour, HOVER);
        assert_eq!(scene.lines[0].segments.len(), 6);
        assert!(!scene.lines[0].on_top);
        assert_eq!(scene.lines[0].slot, Slot::Part);
        let _ = ModelId::new();
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn req_ui_04_picking_on_another_model_starts_a_new_selection() {
        let mut panel = SelectionPanel::default();
        let (a, b) = (ModelId::new(), ModelId::new());
        panel.join(a, Combine::Replace, set(&[1, 2]), None);
        panel.join(a, Combine::Add, set(&[3]), None);
        assert_eq!(panel.working, tri(&[1, 2, 3]));
        panel.join(b, Combine::Add, set(&[9]), None);
        assert_eq!(panel.working, tri(&[9]));
        assert_eq!(panel.working_model, Some(b));
        // Taking away everything leaves no model.
        panel.join(b, Combine::Remove, set(&[9]), None);
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
        // Choosing the set draws exactly its two triangles.
        panel.show_set(Some(id));
        let scene = panel.overlay(&app, &view).unwrap();
        assert_eq!(overlay_triangles(&scene), 2);
        assert!(
            scene.meshes[0]
                .colour
                .iter()
                .zip(rgb_fill(swatch(0)))
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
        assert!(!panel.frame(&app, &SceneView::default(), PickInput::default()));
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
            face: None,
            mesh: 0,
            model: Some(model),
            triangle: 5,
            distance: 1.0,
            point: Point3::new(0.0, 0.0, 0.0),
            normal: None,
        };
        panel.click(&app, &SceneView::default(), &pick, Combine::Replace);
        // The newer click stands alone; the older job cannot overwrite it.
        assert!(panel.pending.is_none());
        assert_eq!(panel.working, tri(&[5]));
        let _ = std::fs::remove_dir_all(dir);
    }

    /// A project with the corpus cube added, and its model.
    fn cube_project(name: &str) -> (std::path::PathBuf, Application, ModelId) {
        use openchuck_app::Threads;
        let dir = std::env::temp_dir().join(format!("openchuck-ui-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut app = Application::create(&dir, name, Threads::Exactly(2)).unwrap();
        std::fs::create_dir_all(dir.join("models")).unwrap();
        let cube = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../testdata/stl/cube_ascii.stl");
        std::fs::copy(cube, dir.join("models/cube.stl")).unwrap();
        let (model, _) = app.add_model(&dir.join("models/cube.stl"), None).unwrap();
        (dir, app, model)
    }

    /// Triangle numbers mean the file they were picked on, so a working
    /// selection does not survive the project accepting a changed file; the
    /// edits that leave the file alone (undo, a placement) keep it (UI-04,
    /// UI-05, PRJ-02).
    #[test]
    fn req_ui_04_the_working_selection_is_dropped_when_the_model_file_changes() {
        use openchuck_app::Command;
        use openchuck_app::model::TransformStep;
        use openchuck_app::project::Item;
        let (dir, mut app, model) = cube_project("stale-working");
        let mut panel = SelectionPanel::default();
        let hash = recorded_hash(&app, model);
        assert!(hash.is_some());
        panel.join(model, Combine::Replace, set(&[1, 2]), hash);
        panel.drop_stale_working(&app);
        assert_eq!(panel.working, tri(&[1, 2]));

        // A new placement does not change the file, and keeps the selection.
        app.execute(Command::SetModelTransform {
            model,
            transform: vec![TransformStep::CentreOnRotaryAxis],
        })
        .unwrap();
        panel.drop_stale_working(&app);
        assert_eq!(panel.working, tri(&[1, 2]));
        assert_eq!(panel.working_model, Some(model));
        // So does undoing it.
        assert!(app.undo().unwrap());
        panel.drop_stale_working(&app);
        assert_eq!(panel.working, tri(&[1, 2]));

        // Accepting a changed file drops it, and says why.
        let mut edited = app.project().models()[0].clone();
        edited.hash = ContentHash::of_bytes(b"another version");
        app.execute(Command::Replace(Item::Model(edited))).unwrap();
        panel.drop_stale_working(&app);
        assert!(panel.working.is_empty());
        assert_eq!(panel.working_model, None);
        assert_eq!(panel.working_hash, None);
        assert!(
            panel
                .message
                .as_deref()
                .is_some_and(|m| m.contains("changed"))
        );
        // Starting again works on the new hash.
        let new_hash = recorded_hash(&app, model);
        assert_ne!(new_hash, hash);
        panel.join(model, Combine::Replace, set(&[7]), new_hash);
        panel.drop_stale_working(&app);
        assert_eq!(panel.working, tri(&[7]));

        // A model that is no longer in the project drops it too.
        let gone = ModelId::new();
        panel.join(gone, Combine::Replace, set(&[3]), Some(new_hash.unwrap()));
        panel.drop_stale_working(&app);
        assert!(panel.working.is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn req_ui_04_a_hovered_face_is_outlined_by_its_edges_and_chosen_faces_are_filled() {
        use openchuck_app::pick::{EdgeId, FaceId};
        use openchuck_app::scene::{Mesh, SceneBrep, SceneEdge};
        use openchuck_app::setup::Point3;
        // Two faces of one triangle each, side by side, with three edges:
        // face 0's outer edge, the edge between them, face 1's outer edge.
        let p = |x: f64, y: f64| Point3::new(x, y, 0.0);
        let edge = |id: u32, faces: &[u32], points: Vec<Point3>| SceneEdge {
            id: EdgeId(id),
            faces: faces.iter().map(|f| FaceId(*f)).collect(),
            turn: None,
            points,
        };
        let model = ModelId::new();
        let view = SceneView {
            meshes: vec![SceneMesh {
                role: SceneRole::Part,
                model: Some(model),
                mesh: Mesh {
                    vertices: vec![p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0), p(2.0, 0.0)],
                    triangles: vec![[0, 1, 2], [1, 3, 2]],
                },
                feature_edges: Vec::new(),
                brep: Some(SceneBrep {
                    faces: vec![FaceId(0), FaceId(1)],
                    edges: vec![
                        edge(0, &[0], vec![p(1.0, 1.0), p(0.0, 0.0), p(1.0, 0.0)]),
                        edge(1, &[0, 1], vec![p(1.0, 0.0), p(1.0, 1.0)]),
                        edge(2, &[1], vec![p(1.0, 0.0), p(2.0, 0.0), p(1.0, 1.0)]),
                    ],
                }),
            }],
            ..SceneView::default()
        };
        let (dir, app, _) = cube_project("face-overlay");
        let mut panel = SelectionPanel::default();
        panel.join(
            model,
            Combine::Replace,
            std::collections::BTreeSet::from([FaceId(1)]),
            None,
        );
        panel.hover = Some(Hover::Surface(ScenePick {
            mesh: 0,
            model: Some(model),
            triangle: 0,
            face: Some(FaceId(0)),
            distance: 1.0,
            point: p(0.5, 0.2),
            normal: None,
        }));
        let scene = panel.overlay(&app, &view).unwrap();
        // Face 1 filled blue: its one triangle.
        assert_eq!(scene.meshes.len(), 1);
        assert_eq!(overlay_triangles(&scene), 1);
        assert_eq!(scene.meshes[0].colour, WORKING);
        // Face 0 outlined in yellow by its two edges, not by its triangle.
        assert_eq!(scene.lines.len(), 2);
        assert!(scene.lines.iter().all(|l| l.colour == HOVER));
        assert_eq!(scene.lines[0].segments.len(), 4);
        assert_eq!(scene.lines[1].segments.len(), 2);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn req_ui_04_a_clicked_edge_chain_is_drawn_over_the_part() {
        use openchuck_app::Command;
        use openchuck_app::model::TransformStep;
        use openchuck_app::pick::PickTolerance;
        let (dir, mut app, model) = cube_project("edge-overlay");
        app.execute(Command::SetModelTransform {
            model,
            transform: vec![TransformStep::CentreOnRotaryAxis],
        })
        .unwrap();
        let view = app.start_scene(None).wait().unwrap();
        let reach = PickTolerance {
            base: 0.3,
            per_distance: 0.0,
        };
        // The top front edge of the cube (y = -5, z = 5).
        let pick = view
            .picking
            .picker()
            .unwrap()
            .pick_edge([5.0, -4.9, 40.0], [0.0, 0.0, -1.0], reach)
            .unwrap()
            .unwrap();
        let mut panel = SelectionPanel {
            edges: true,
            choice: Choice::Connected,
            ..SelectionPanel::default()
        };
        assert!(panel.picks_edges());
        let input = PickInput {
            edge_hover: Some(pick),
            edge_click: Some((pick, Modifiers::NONE)),
            ..PickInput::default()
        };
        assert!(!panel.frame(&app, &view, input));
        // A cube's corners join three edges: the chain is the edge alone.
        assert!(matches!(&panel.working, SelectionTarget::MeshEdges(e) if e.len() == 1));
        assert_eq!(panel.working_model, Some(model));
        let scene = panel.overlay(&app, &view).unwrap();
        // The selected edge in blue and the hovered one in yellow, both over
        // the part, and nothing filled.
        assert!(scene.meshes.is_empty());
        assert_eq!(scene.lines.len(), 2);
        assert_eq!(
            scene.lines[0].colour,
            [WORKING[0], WORKING[1], WORKING[2], 1.0]
        );
        assert_eq!(scene.lines[1].colour, HOVER);
        assert!(scene.lines.iter().all(|l| l.on_top && l.slot == Slot::Part));
        // A surface click starts the working selection again, as triangles.
        let surface = view
            .picking
            .picker()
            .unwrap()
            .pick([3.0, 1.0, 40.0], [0.0, 0.0, -1.0])
            .unwrap()
            .unwrap();
        panel.choice = Choice::Triangle;
        panel.click(&app, &view, &surface, Combine::Add);
        assert_eq!(panel.working, tri(&[surface.triangle]));
        let _ = std::fs::remove_dir_all(dir);
    }
}
