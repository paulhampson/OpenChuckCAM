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
//! STEP part by face or by its own edges. A click picks the edge when the
//! pointer is within reach of one and the surface otherwise; the box on the
//! view can restrict picking to faces or to edges. Every project change is a command of `openchuck-app`,
//! so it can be undone; the highlights it asks the viewport to draw are plain
//! meshes from [`triangles_mesh`] and the edges' polylines.

use eframe::egui::{self, Color32, Modifiers, RichText};
use openchuck_app::pick::{EdgeRef, PickMode, SceneEdgePick, ScenePick};
use openchuck_app::project::id::{ModelId, OperationId, SelectionSetId};
use openchuck_app::project::{ContentHash, SelectionTarget};
use openchuck_app::scene::{SceneMesh, SceneRole, SceneView, triangles_mesh};
use openchuck_app::{
    Application, CheckedSet, EditStart, JobHandle, SelectionEdit, SelectionRole, SelectionSummary,
    TriangleSet,
};
use openchuck_viewport::{Colour, Hatch, Scene, Slot};

use crate::roles;
use crate::theme::DARK;
use crate::viewport::{PickFilter, PickInput};

const PROBLEM: Color32 = DARK.error;
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
pub(crate) fn set_rgb(chosen: Option<&str>, index: usize) -> [u8; 3] {
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
    /// chain, and R selects the edge alone (decision record 0066 item 4).
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
            Self::Direction => "Only the edge clicked: same direction is for surfaces.",
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

    /// `current` joined with `picked`. A selection holds one kind: replacing
    /// it with another kind starts again with that kind, and adding or taking
    /// away another kind changes nothing (the panel refuses such a click
    /// before it gets here, saying why).
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
                if self == Self::Replace || current.is_empty() {
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

/// A plain click of the other kind waiting for the user to confirm that it
/// resets the working selection (decision record 0066 item 3).
struct Reset {
    model: ModelId,
    picked: SelectionTarget,
    hash: Option<ContentHash>,
}

/// The widths of a drawn edge, in logical pixels: the part's feature edges
/// are hairlines, a hovered edge is wider, and a selected edge (the working
/// selection or a shown set) wider still, so the state shows by width as well
/// as colour (specification section 16; OI-245, decision record 0112).
const EDGE_HOVER_WIDTH: f32 = 2.0;
const EDGE_SELECTED_WIDTH: f32 = 3.0;

/// A save or an edit of a set whose check runs as a job (NFR-04). The panel
/// stores the set when the job finishes.
struct Writing {
    handle: JobHandle<CheckedSet>,
    what: Write,
}

enum Write {
    /// A new set; the tangent choice it takes if it is of STEP edges.
    Save { tangent: Option<bool> },
    /// A change to what a set selects.
    Edit,
}

struct Pending {
    handle: JobHandle<TriangleSet>,
    model: ModelId,
    combine: Combine,
}

/// The panel's state between frames.
pub struct SelectionPanel {
    choice: Choice,
    /// What a click may pick (decision record 0066 item 3).
    filter: PickFilter,
    smooth_angle: f64,
    direction_angle: f64,
    working: SelectionTarget,
    working_model: Option<ModelId>,
    /// The hash the project recorded for `working_model` when the working
    /// selection began. Triangle numbers mean the file they were picked on, so
    /// if the project accepts a changed file the selection is dropped.
    working_hash: Option<ContentHash>,
    name: String,
    /// Whether a new set of B-rep edges keeps tangent edges, or `None` for
    /// the project's default (decision record 0066).
    new_tangent: Option<bool>,
    rename: String,
    /// What the row menu of the table asked for, handled after the table.
    row_action: Option<(SelectionSetId, RowAction)>,
    /// Why the last delete was refused.
    delete_note: Option<String>,
    pending: Option<Pending>,
    /// The save or edit of a set being checked against its model.
    writing: Option<Writing>,
    /// Whether a plain click of the other kind asks first, as the project
    /// says when the click came.
    warn_reset: bool,
    /// The click waiting for that answer, and whether the user ticked
    /// "Don't show me this warning again".
    reset: Option<Reset>,
    dont_warn: bool,
    hover: Option<Hover>,
    shown: Option<SelectionSetId>,
    /// The operation whose avoid and check sets are drawn, hatched red
    /// (specification 7.4).
    roles_of: Option<OperationId>,
    /// The avoid and check sets drawn for `roles_of`.
    roles_sets: Option<(Vec<SelectionSetId>, Vec<SelectionSetId>)>,
    /// The last thing that went wrong or the last notice.
    message: Option<String>,
    /// The highlights need drawing again.
    dirty: bool,
    /// Test hook: leave a region pick pending when frames are drawn, so a
    /// shell test can press Esc while one is certainly still running.
    #[cfg(test)]
    hold_region_pick: bool,
}

impl Default for SelectionPanel {
    fn default() -> Self {
        Self {
            choice: Choice::Smooth,
            filter: PickFilter::Both,
            smooth_angle: 30.0,
            direction_angle: 10.0,
            working: nothing(),
            working_model: None,
            working_hash: None,
            name: String::new(),
            new_tangent: None,
            rename: String::new(),
            row_action: None,
            delete_note: None,
            pending: None,
            writing: None,
            warn_reset: true,
            reset: None,
            dont_warn: false,
            hover: None,
            shown: None,
            roles_of: None,
            roles_sets: None,
            message: None,
            dirty: true,
            #[cfg(test)]
            hold_region_pick: false,
        }
    }
}

impl SelectionPanel {
    /// Test hook: the name of the click mode now chosen.
    #[cfg(test)]
    pub(crate) fn choice_name(&self) -> &'static str {
        self.choice.name()
    }

    /// Test hook: whether a region pick is running.
    #[cfg(test)]
    pub(crate) fn has_region_pick(&self) -> bool {
        self.pending.is_some()
    }

    /// Test hook: the triangles of the working selection (none when it holds
    /// another kind).
    #[cfg(test)]
    pub(crate) fn working_triangles(&self) -> Vec<u32> {
        match &self.working {
            SelectionTarget::Triangles(t) => t.iter().copied().collect(),
            _ => Vec::new(),
        }
    }

    /// Test hook: starts a flood fill on `model` and keeps it pending however
    /// fast it is, until it is cancelled.
    #[cfg(test)]
    pub(crate) fn hold_region_pick(&mut self, app: &Application, model: ModelId) {
        let handle = app
            .start_select_region(
                model,
                &openchuck_app::RegionRequest::FloodFill { seeds: vec![0] },
            )
            .unwrap();
        self.pending = Some(Pending {
            handle,
            model,
            combine: Combine::Replace,
        });
        self.hold_region_pick = true;
    }

    /// Forgets everything that belongs to the open project.
    pub fn clear(&mut self) {
        if let Some(writing) = self.writing.take() {
            writing.handle.cancel();
        }
        *self = Self {
            choice: self.choice,
            filter: self.filter,
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

    /// Draws the avoid and check sets of `operation`, hatched red over the
    /// part (specification 7.4), or none.
    /// They are drawn again when the operation's sets change too, as an edit
    /// in its properties or an undo does.
    pub fn show_roles_of(&mut self, app: &Application, operation: Option<OperationId>) {
        let sets = operation
            .and_then(|id| app.project().operation(id))
            .map(|op| (op.avoid.clone(), op.check.clone()));
        if self.roles_of != operation || self.roles_sets != sets {
            self.roles_of = operation;
            self.roles_sets = sets;
            self.dirty = true;
        }
    }

    /// Chooses the set whose triangles are shown in its swatch.
    pub fn show_set(&mut self, set: Option<SelectionSetId>) {
        if self.shown != set {
            self.shown = set;
            self.dirty = true;
        }
    }

    /// What a click may pick.
    pub fn pick_filter(&self) -> PickFilter {
        self.filter
    }

    /// The tangent-edge choice a new set takes: only a set of a STEP model's
    /// edges holds one (decision record 0066 item 9).
    fn new_set_tangent(&self) -> Option<bool> {
        match self.working {
            SelectionTarget::Edges(_) => self.new_tangent,
            _ => None,
        }
    }

    /// Whether the tools are named for edges: only edges are picked, or the
    /// working selection holds edges.
    fn names_edges(&self) -> bool {
        self.filter == PickFilter::Edges
            || matches!(
                self.working,
                SelectionTarget::Edges(_) | SelectionTarget::MeshEdges(_)
            )
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
        self.warn_reset = app.project().warn_on_selection_reset();
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
        #[cfg(test)]
        if self.hold_region_pick {
            return true;
        }
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
        self.warn_reset = app.project().warn_on_selection_reset();
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
        self.warn_reset = app.project().warn_on_selection_reset();
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
        let picked = picked.into();
        // A selection holds one kind (decision record 0066 item 3). Adding or
        // taking away a pick of another kind is refused with the way out, and
        // the selection stays as it was; a plain click starts a new one, after
        // asking unless the project says not to.
        let other_kind = self.working_model == Some(model)
            && !self.working.is_empty()
            && std::mem::discriminant(&self.working) != std::mem::discriminant(&picked);
        if other_kind && combine != Combine::Replace {
            self.message = Some(refusal(&self.working, &picked));
            return;
        }
        if other_kind && self.warn_reset {
            self.reset = Some(Reset {
                model,
                picked,
                hash,
            });
            self.dont_warn = false;
            return;
        }
        // A selection is on one model; picking on another starts again.
        if self.working_model != Some(model) {
            self.working = nothing();
            self.working_model = Some(model);
            self.working_hash = hash;
        }
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
        // The surfaces the operation must not cut or must check, hatched red
        // so they read without colour (7.4).
        if let Some(op) = self.roles_of.and_then(|id| app.project().operation(id)) {
            let sets = app.project().selection_sets();
            for id in op.avoid.iter().chain(&op.check) {
                let Some(set) = sets.iter().find(|s| s.id == *id) else {
                    continue;
                };
                let Some((index, part)) = part_of(set.model) else {
                    continue;
                };
                let first = scene.meshes.len();
                let [r, g, b, _] = roles::AVOID;
                draw_target(
                    &mut scene,
                    view,
                    index,
                    part,
                    &set.target,
                    [r, g, b, 0.18],
                    roles::AVOID,
                );
                for added in scene.meshes.iter_mut().skip(first) {
                    added.hatch = Some(Hatch {
                        colour: roles::AVOID,
                        cross: false,
                    });
                }
            }
        }
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
                                scene.add_wide_polyline(
                                    &edge.points,
                                    HOVER,
                                    false,
                                    EDGE_HOVER_WIDTH,
                                );
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
                                // Hovered marks are 2 pixels wide (7.5; OI-341).
                                if outline.len() == 4 {
                                    scene.add_wide_polyline(
                                        &outline,
                                        HOVER,
                                        false,
                                        EDGE_HOVER_WIDTH,
                                    );
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
                    scene.add_wide_polyline(points, HOVER, true, EDGE_HOVER_WIDTH);
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

    /// Chooses what a click selects from a key (Q, W, E, R; specification
    /// section 15), and reports Esc. Returns true when Esc asks to leave
    /// picking; a region pick still running is cancelled first and keeps
    /// picking on. Does nothing while a text field has the keyboard, or had it
    /// at the end of the last frame (`text_had_focus`: egui lets go of a text
    /// field's focus at the start of the frame an Esc arrives in, so Esc that
    /// ends an edit would otherwise also leave picking); the caller skips it
    /// while a dialog is open.
    pub fn keys(&mut self, ctx: &egui::Context, text_had_focus: bool) -> bool {
        if text_had_focus || ctx.egui_wants_keyboard_input() {
            return false;
        }
        let pressed = |key: egui::Key| ctx.input_mut(|i| i.consume_key(Modifiers::NONE, key));
        for key in [egui::Key::Q, egui::Key::W, egui::Key::E, egui::Key::R] {
            if pressed(key)
                && !(self.filter == PickFilter::Edges && key == egui::Key::R)
                && let Some(choice) = choice_for_key(key)
            {
                self.choice = choice;
                self.message = None;
            }
        }
        if !pressed(egui::Key::Escape) {
            return false;
        }
        if self.reset.take().is_some() {
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
                            ui.label("Pick");
                            for (filter, label, tip) in [
                                (
                                    PickFilter::Both,
                                    "Both",
                                    "An edge when the pointer is near one, the surface otherwise.",
                                ),
                                (
                                    PickFilter::Faces,
                                    "Faces",
                                    "Only surfaces: triangles, or the faces of a STEP model.",
                                ),
                                (
                                    PickFilter::Edges,
                                    "Edges",
                                    "Only edges: the edges drawn on the part.",
                                ),
                            ] {
                                if ui
                                    .selectable_label(self.filter == filter, label)
                                    .on_hover_text(tip)
                                    .clicked()
                                {
                                    self.filter = filter;
                                    // Same direction is not offered for edges.
                                    if filter == PickFilter::Edges
                                        && self.choice == Choice::Direction
                                    {
                                        self.choice = Choice::Triangle;
                                    }
                                    self.message = None;
                                    self.hover = None;
                                    self.dirty = true;
                                }
                            }
                        });
                        ui.separator();
                        for choice in Choice::ALL {
                            if self.filter == PickFilter::Edges && choice == Choice::Direction {
                                continue;
                            }
                            let key = choice
                                .key()
                                .map_or(String::new(), |k| format!(" ({})", k.name()));
                            let edge_tip = format!("On an edge: {}", choice.edge_tip());
                            let (name, tip) = match (self.names_edges(), self.filter) {
                                (true, _) => (choice.edge_name(), choice.edge_tip().to_owned()),
                                (false, PickFilter::Faces) => {
                                    (choice.name(), choice.tip().to_owned())
                                }
                                (false, _) => {
                                    (choice.name(), format!("{}\n{edge_tip}", choice.tip()))
                                }
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

    /// Finishes a save or an edit whose job has ended, whether or not the
    /// panel is shown this frame (the dock tab may be closed). Returns the
    /// text of an error to show.
    pub fn poll(&mut self, app: &mut Application) -> Option<String> {
        let error = self.finish_writing(app);
        self.drop_stale_working(app);
        error
    }

    /// Draws the panel. Returns the text of an error to show in a dialog.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        app: &mut Application,
        summaries: &[SelectionSummary],
        view: &SceneView,
    ) -> Option<String> {
        let mut error = self.finish_writing(app);
        self.drop_stale_working(app);
        ui.heading("Selection sets");
        if view.picking.picker().is_none() {
            ui.label("There is no part to select on. Choose a setup with a part.");
        }
        ui.weak("Meshes are selected by triangle or edge; STEP models by face and edge.");
        ui.horizontal_wrapped(|ui| {
            let name = if self.names_edges() {
                self.choice.edge_name()
            } else {
                self.choice.name()
            };
            let what = match self.filter {
                PickFilter::Both => "edges near the pointer, else surfaces",
                PickFilter::Faces => "surfaces only",
                PickFilter::Edges => "edges only",
            };
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
            let ready =
                !self.working.is_empty() && self.working_model.is_some() && self.writing.is_none();
            if ui
                .add_enabled(ready, egui::Button::new("New set from selection"))
                .on_hover_text("Name this selection so operations can use it")
                .clicked()
                && let Some(model) = self.working_model
            {
                let name = if self.name.trim().is_empty() {
                    format!("Selection {}", summaries.len() + 1)
                } else {
                    self.name.trim().to_owned()
                };
                let tangent = self.new_set_tangent();
                match app.start_save_selection(&name, model, self.working.clone()) {
                    Ok(handle) => {
                        self.writing = Some(Writing {
                            handle,
                            what: Write::Save { tangent },
                        });
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
        if matches!(self.working, SelectionTarget::Edges(_))
            && let Some(choice) = tangent_choice(
                ui,
                "new set tangent edges",
                self.new_tangent,
                app.project().keep_tangent_edges(),
            )
        {
            self.new_tangent = choice;
        }
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
                let enabled = can_edit && self.writing.is_none();
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
                    match app.start_edit_selection(set.id, which) {
                        Ok(EditStart::Done(checked)) => match app.finish_edit_selection(*checked) {
                            Ok(_) => self.dirty = true,
                            Err(e) => error = Some(e.to_string()),
                        },
                        Ok(EditStart::Checking(handle)) => {
                            self.writing = Some(Writing {
                                handle,
                                what: Write::Edit,
                            });
                        }
                        Err(e) => error = Some(e.to_string()),
                    }
                }
            }
        });
        if self.writing.is_some() {
            ui.horizontal(|ui| {
                ui.label("Checking the selection against the model…");
                if ui.button("Cancel").clicked()
                    && let Some(writing) = self.writing.take()
                {
                    writing.handle.cancel();
                }
            });
            ui.ctx().request_repaint();
        }

        ui.separator();
        if summaries.is_empty() {
            ui.weak("No sets yet. Select on the part and make a set from it.");
        } else {
            self.table(ui, app, summaries);
            if let Some((id, action)) = self.row_action.take()
                && let Some(summary) = summaries.iter().find(|s| s.id == id)
            {
                match action {
                    RowAction::Rename => {
                        self.show_set(Some(id));
                        self.rename = summary.name.clone();
                    }
                    RowAction::Delete => match delete_refusal(summary) {
                        Some(note) => self.delete_note = Some(note),
                        None => match app.delete_selection(id) {
                            Ok(()) => {
                                self.delete_note = None;
                                if self.shown == Some(id) {
                                    self.shown = None;
                                }
                                self.dirty = true;
                            }
                            Err(e) => error = Some(e.to_string()),
                        },
                    },
                }
            }
            if let Some(note) = &self.delete_note {
                ui.colored_label(PROBLEM, note);
            }
        }
        if let Some(summary) = chosen {
            ui.separator();
            if let Some(e) = self.set_details(ui, app, summary) {
                error = Some(e);
            }
        }
        ui.separator();
        let mut keep = app.project().keep_tangent_edges();
        if ui
            .checkbox(&mut keep, "Keep tangent edges by default")
            .on_hover_text(
                "For sets of a STEP model's edges that do not say: whether edge following keeps \
                 the edges where two faces meet smoothly. Saved with the project.",
            )
            .changed()
            && let Err(e) = app.execute(openchuck_app::Command::SetKeepTangentEdges(keep))
        {
            error = Some(e.to_string());
        }
        let mut warn = app.project().warn_on_selection_reset();
        if ui
            .checkbox(&mut warn, "Warn before a click starts a new selection")
            .on_hover_text(
                "Ask before a click that picks the other kind (an edge on a selection of faces, \
                 say) clears the selection. Saved with the project.",
            )
            .changed()
            && let Err(e) = app.execute(openchuck_app::Command::SetWarnOnSelectionReset(warn))
        {
            error = Some(e.to_string());
        }
        ui.weak("Editing a set marks every operation that uses it stale.");
        error
    }

    /// The warning before a plain click of the other kind resets the working
    /// selection (decision record 0066 item 3), drawn while one waits.
    /// Returns the text of an error to show in a dialog.
    pub fn reset_dialog(&mut self, ctx: &egui::Context, app: &mut Application) -> Option<String> {
        let reset = self.reset.as_ref()?;
        let text = reset_warning(&self.working, &reset.picked);
        let mut answer = None;
        egui::Window::new("Start a new selection?")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(text);
                ui.checkbox(&mut self.dont_warn, "Don't show me this warning again");
                ui.horizontal(|ui| {
                    if ui.button("Start a new selection").clicked() {
                        answer = Some(true);
                    }
                    if ui.button("Cancel").clicked() {
                        answer = Some(false);
                    }
                });
            });
        answer.and_then(|start| self.answer_reset(app, start))
    }

    /// Answers the warning: `start` resets the working selection to the
    /// click's pick, storing "Don't show me this warning again" as an
    /// undoable project setting; otherwise the selection stays as it was.
    fn answer_reset(&mut self, app: &mut Application, start: bool) -> Option<String> {
        let Reset {
            model,
            picked,
            hash,
        } = self.reset.take()?;
        if !start {
            return None;
        }
        if self.dont_warn
            && let Err(e) = app.execute(openchuck_app::Command::SetWarnOnSelectionReset(false))
        {
            return Some(e.to_string());
        }
        // The answer is given: this click resets without asking again.
        self.warn_reset = false;
        self.join(model, Combine::Replace, picked, hash);
        self.warn_reset = app.project().warn_on_selection_reset();
        None
    }

    /// The table of sets (specification section 12.2): swatch, name, kind,
    /// size, used by, status and the action. A set with a problem is marked
    /// with a triangle as well as red.
    /// Stores the set of a finished save or edit job. Returns the text of an
    /// error to show: the check failed, or the project changed meanwhile.
    fn finish_writing(&mut self, app: &mut Application) -> Option<String> {
        let result = self.writing.as_ref()?.handle.try_result()?;
        let Writing { what, .. } = self.writing.take()?;
        let stored = result.and_then(|checked| match what {
            Write::Save { tangent } => {
                let id = app.finish_save_selection(checked)?;
                if let Some(keep) = tangent {
                    app.edit_selection(id, SelectionEdit::KeepTangentEdges(Some(keep)))?;
                }
                self.name.clear();
                self.working = nothing();
                self.working_model = None;
                self.new_tangent = None;
                self.shown = Some(id);
                Ok(())
            }
            Write::Edit => app.finish_edit_selection(checked).map(|_| ()),
        });
        match stored {
            Ok(()) => {
                self.dirty = true;
                None
            }
            // Cancelling is the user's own choice, not a failure.
            Err(e) if e.is_cancelled() => None,
            Err(e) => Some(e.to_string()),
        }
    }

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
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(format!("▲ {problem}")).color(PROBLEM),
                                    )
                                    .truncate(),
                                )
                                .on_hover_text(problem);
                            }
                            None => {
                                ui.label("● Matches model");
                            }
                        }
                        ui.horizontal(|ui| {
                            let usable = summary.problem.is_none();
                            let (text, tip) = if usable {
                                ("Select in view", "Make the set the working selection, to edit it")
                            } else {
                                (
                                    "Reselect",
                                    "Load the set as the working selection, change it and press Replace set",
                                )
                            };
                            if ui.link(text).on_hover_text(tip).clicked()
                                && let Some(t) = set_target(app, summary.id)
                            {
                                self.select_set_in_view(app, summary, t);
                            }
                            ui.menu_button("⋯", |ui| {
                                if ui.button("Rename").clicked() {
                                    self.row_action = Some((summary.id, RowAction::Rename));
                                    ui.close();
                                }
                                if ui.button("Delete").clicked() {
                                    self.row_action = Some((summary.id, RowAction::Delete));
                                    ui.close();
                                }
                            });
                        });
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
        if let Some(set) = app
            .project()
            .selection_sets()
            .iter()
            .find(|s| s.id == summary.id)
            .filter(|s| matches!(s.target, SelectionTarget::Edges(_)))
            .cloned()
            && let Some(choice) = tangent_choice(
                ui,
                "set tangent edges",
                set.keep_tangent_edges,
                app.project().keep_tangent_edges(),
            )
            && let Err(e) = app.edit_selection(set.id, SelectionEdit::KeepTangentEdges(choice))
        {
            error = Some(e.to_string());
        }
        error
    }
}

/// What a row's menu asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RowAction {
    Rename,
    Delete,
}

/// Why a set cannot be deleted: the operations that use it, by name, or
/// `None` when nothing does (specification 12.2).
fn delete_refusal(summary: &SelectionSummary) -> Option<String> {
    if summary.uses.is_empty() {
        return None;
    }
    let names: Vec<&str> = summary.uses.iter().map(|u| u.name.as_str()).collect();
    Some(format!(
        "“{}” is used by {}. Take it out of {} first.",
        summary.name,
        names.join(", "),
        if names.len() == 1 {
            "that operation"
        } else {
            "those operations"
        }
    ))
}

/// The choice of keeping tangent edges, as the user reads it: the project's
/// default, keep, or drop (decision record 0066).
fn tangent_label(value: Option<bool>, default: bool) -> String {
    match value {
        None => format!(
            "Project default ({})",
            if default { "keep" } else { "drop" }
        ),
        Some(true) => "Keep tangent edges".to_owned(),
        Some(false) => "Drop tangent edges".to_owned(),
    }
}

/// A list to choose whether a set of a STEP model's edges keeps the edges
/// where two faces meet smoothly. Returns the new choice when it changed.
fn tangent_choice(
    ui: &mut egui::Ui,
    salt: &str,
    value: Option<bool>,
    default: bool,
) -> Option<Option<bool>> {
    let mut chosen = None;
    ui.horizontal(|ui| {
        ui.label("Tangent edges").on_hover_text(
            "Edges where two faces meet smoothly, such as the edge of a fillet. Edge \
                 following drops them unless they are kept.",
        );
        egui::ComboBox::from_id_salt(salt)
            .selected_text(tangent_label(value, default))
            .show_ui(ui, |ui| {
                for option in [None, Some(true), Some(false)] {
                    if ui
                        .selectable_label(value == option, tangent_label(option, default))
                        .clicked()
                        && option != value
                    {
                        chosen = Some(option);
                    }
                }
            });
    });
    chosen
}

/// What the set `id` selects.
/// Why a pick of another kind is not added to the working selection, and the
/// way out (decision record 0066 item 3).
fn refusal(working: &SelectionTarget, picked: &SelectionTarget) -> String {
    let noun = |t: &SelectionTarget| match t {
        SelectionTarget::Triangles(_) => ("triangles", "a triangle", "Faces"),
        SelectionTarget::Faces(_) => ("faces", "a face", "Faces"),
        SelectionTarget::Edges(_) | SelectionTarget::MeshEdges(_) => ("edges", "an edge", "Edges"),
    };
    let (holds, _, filter) = noun(working);
    let (_, one, _) = noun(picked);
    format!(
        "The selection holds {holds}, so {one} cannot join it: a selection holds one kind. \
         Choose {filter} in the box on the view to pick only {holds}, or click without \
         Shift or Ctrl to start a new selection."
    )
}

/// What the warning before a reset says (decision record 0066 item 3).
fn reset_warning(working: &SelectionTarget, picked: &SelectionTarget) -> String {
    let noun = |t: &SelectionTarget| match t {
        SelectionTarget::Triangles(_) => ("triangles", "a triangle"),
        SelectionTarget::Faces(_) => ("faces", "a face"),
        SelectionTarget::Edges(_) | SelectionTarget::MeshEdges(_) => ("edges", "an edge"),
    };
    let (holds, _) = noun(working);
    let (_, one) = noun(picked);
    format!(
        "This click picks {one}, and the selection holds {holds}: a selection holds one kind. \
         Starting a new selection with it clears the {} {holds} selected now.",
        working.len()
    )
}

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
        scene.add_wide_polyline(&points, line, true, EDGE_SELECTED_WIDTH);
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

    #[test]
    fn req_ui_05_a_refused_delete_names_the_operations_that_use_the_set() {
        use openchuck_app::project::id::OperationId;
        let mut summary = SelectionSummary {
            id: SelectionSetId::new(),
            name: "Rim".to_owned(),
            model: openchuck_app::project::id::ModelId::new(),
            model_path: None,
            kind: "triangles",
            count: 3,
            uses: Vec::new(),
            problem: None,
            colour: None,
        };
        assert_eq!(delete_refusal(&summary), None);
        let use_of = |name: &str| openchuck_app::SelectionUse {
            operation: OperationId::new(),
            name: name.to_owned(),
            roles: Vec::new(),
        };
        summary.uses.push(use_of("2 Finish"));
        let one = delete_refusal(&summary).unwrap();
        assert!(
            one.contains("2 Finish") && one.contains("that operation"),
            "{one}"
        );
        summary.uses.push(use_of("4 Engrave"));
        let two = delete_refusal(&summary).unwrap();
        assert!(
            two.contains("2 Finish, 4 Engrave") && two.contains("those operations"),
            "{two}"
        );
    }

    fn set(items: &[u32]) -> TriangleSet {
        items.iter().copied().collect()
    }

    fn tri(items: &[u32]) -> SelectionTarget {
        SelectionTarget::Triangles(set(items))
    }

    #[test]
    fn req_op_08_req_ui_05_only_a_set_of_step_edges_takes_the_tangent_choice() {
        let mut panel = SelectionPanel {
            new_tangent: Some(true),
            ..SelectionPanel::default()
        };
        // Chosen while a set of STEP edges was loaded, then the selection
        // became triangles: the new set takes no tangent choice.
        panel.working = tri(&[1]);
        assert_eq!(panel.new_set_tangent(), None);
        panel.working = SelectionTarget::Edges([openchuck_app::pick::EdgeId(4)].into());
        assert_eq!(panel.new_set_tangent(), Some(true));
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
        // A selection holds one kind: adding or taking faces from triangles
        // changes nothing, and replacing starts again with the faces. An
        // empty selection takes either kind.
        let faces = SelectionTarget::Faces([openchuck_app::pick::FaceId(3)].into());
        Combine::Remove.apply(&mut current, faces.clone());
        assert_eq!(current, tri(&[7]));
        Combine::Add.apply(&mut current, faces.clone());
        assert_eq!(current, tri(&[7]));
        Combine::Replace.apply(&mut current, faces.clone());
        assert_eq!(current, faces);
        let mut empty = tri(&[]);
        Combine::Add.apply(&mut empty, faces.clone());
        assert_eq!(empty, faces);
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
            assert!(!panel.keys(ui.ctx(), false));
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
        let mut output = ctx.run_ui(input, |ui| left = panel.keys(ui.ctx(), false));
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
        // Hovered marks are 2 pixels wide (7.5; OI-341).
        assert_eq!(scene.lines[0].width, EDGE_HOVER_WIDTH);
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

    #[test]
    fn req_op_02_req_nfr_15_the_operations_avoid_and_check_sets_are_hatched_red() {
        let (dir, mut app, op, _) = crate::testing::cube_project("avoid-hatch");
        let model = app.project().models()[0].id;
        let side = app.save_selection("Side", model, set(&[4, 5])).unwrap();
        let top = app.save_selection("Top", model, set(&[6])).unwrap();
        app.set_operation_selection(op, SelectionRole::Avoid, side, true)
            .unwrap();
        app.set_operation_selection(op, SelectionRole::Check, top, true)
            .unwrap();
        let view = app.start_scene(None).wait().unwrap();
        let mut panel = SelectionPanel::default();
        // Nothing while no operation is chosen.
        let scene = panel.overlay(&app, &view).unwrap();
        assert!(scene.meshes.is_empty());
        // With the operation chosen, both sets are drawn red with a single
        // hatch, a cue besides the colour (7.4).
        panel.show_roles_of(&app, Some(op));
        let scene = panel.overlay(&app, &view).unwrap();
        assert_eq!(scene.meshes.len(), 2);
        assert_eq!(overlay_triangles(&scene), 3);
        for mesh in &scene.meshes {
            assert_eq!(
                mesh.hatch,
                Some(Hatch {
                    colour: roles::AVOID,
                    cross: false
                })
            );
            assert_eq!(mesh.slot, Slot::Part);
        }
        // Choosing it again changes nothing, so nothing is drawn again.
        panel.show_roles_of(&app, Some(op));
        assert!(panel.overlay(&app, &view).is_none());
        // Taking a set out of its role while the operation stays chosen draws
        // the sets again (#126 review).
        app.set_operation_selection(op, SelectionRole::Avoid, side, false)
            .unwrap();
        panel.show_roles_of(&app, Some(op));
        let scene = panel.overlay(&app, &view).unwrap();
        assert_eq!(scene.meshes.len(), 1);
        let _ = std::fs::remove_dir_all(dir);
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

    #[test]
    fn req_nfr_04_req_ui_05_a_new_set_is_stored_when_its_check_job_finishes() {
        let (dir, mut app, model) = cube_project("save-job");
        let mut panel = SelectionPanel {
            working: tri(&[2, 3]),
            working_model: Some(model),
            name: "Top".into(),
            ..SelectionPanel::default()
        };
        let handle = app
            .start_save_selection("Top", model, set(&[2, 3]))
            .unwrap();
        panel.writing = Some(Writing {
            handle,
            what: Write::Save { tangent: None },
        });
        // Polling stores the set once the job has finished and starts a fresh
        // selection.
        let error = loop {
            let error = panel.finish_writing(&mut app);
            if panel.writing.is_none() {
                break error;
            }
            std::thread::yield_now();
        };
        assert_eq!(error, None);
        assert_eq!(app.project().selection_sets().len(), 1);
        assert!(panel.working.is_empty() && panel.name.is_empty());
        assert_eq!(panel.shown, Some(app.project().selection_sets()[0].id));
        // A failed check shows its error and stores nothing.
        let handle = app.start_save_selection("Far", model, set(&[99])).unwrap();
        panel.writing = Some(Writing {
            handle,
            what: Write::Save { tangent: None },
        });
        let error = loop {
            let error = panel.finish_writing(&mut app);
            if panel.writing.is_none() {
                break error;
            }
            std::thread::yield_now();
        };
        assert!(error.unwrap().contains("triangle 99"));
        assert_eq!(app.project().selection_sets().len(), 1);
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
        // No switch is needed: by default an edge near the pointer is picked.
        let mut panel = SelectionPanel {
            choice: Choice::Connected,
            ..SelectionPanel::default()
        };
        assert_eq!(panel.pick_filter(), PickFilter::Both);
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
        // State is not shown by colour alone (specification 16, OI-245): the
        // selected edge is wider than the hovered one, and both are wider than
        // the part's hairline feature edges.
        assert_eq!(scene.lines[0].width, EDGE_SELECTED_WIDTH);
        assert_eq!(scene.lines[1].width, EDGE_HOVER_WIDTH);
        const { assert!(EDGE_SELECTED_WIDTH > EDGE_HOVER_WIDTH && EDGE_HOVER_WIDTH > 1.0) };
        // A surface clicked with Shift is not added to the edges: the click
        // is refused, saying why and the way out, and the edges stay.
        let surface = view
            .picking
            .picker()
            .unwrap()
            .pick([3.0, 1.0, 40.0], [0.0, 0.0, -1.0])
            .unwrap()
            .unwrap();
        panel.choice = Choice::Triangle;
        let before = panel.working.clone();
        panel.click(&app, &view, &surface, Combine::Add);
        assert_eq!(panel.working, before);
        let message = panel.message.clone().unwrap();
        assert!(message.contains("holds edges"), "{message}");
        assert!(message.contains("Choose Edges"), "{message}");
        assert!(message.contains("start a new selection"), "{message}");
        // Ctrl is refused the same way; a plain click starts a new selection.
        panel.click(&app, &view, &surface, Combine::Remove);
        assert_eq!(panel.working, before);
        // A plain click of the other kind warns first: nothing changes until
        // the user answers, and Cancel keeps the edges.
        panel.click(&app, &view, &surface, Combine::Replace);
        assert_eq!(panel.working, before);
        let warning = reset_warning(&panel.working, &panel.reset.as_ref().unwrap().picked);
        assert!(warning.contains("picks a triangle"), "{warning}");
        assert!(warning.contains("clears the 1 edges"), "{warning}");
        assert!(panel.answer_reset(&mut app, false).is_none());
        assert_eq!(panel.working, before);
        assert!(panel.reset.is_none());
        // Starting a new selection with "Don't show me this warning again"
        // ticked resets it and stores the choice in the project, undoably.
        panel.click(&app, &view, &surface, Combine::Replace);
        panel.dont_warn = true;
        assert!(panel.answer_reset(&mut app, true).is_none());
        assert_eq!(panel.working, tri(&[surface.triangle]));
        assert!(panel.message.is_none());
        assert!(!app.project().warn_on_selection_reset());
        // With the warning off, a plain click of the other kind resets
        // silently.
        panel.click_edge(&app, &view, &pick, Combine::Replace);
        assert!(panel.reset.is_none());
        assert!(matches!(&panel.working, SelectionTarget::MeshEdges(_)));
        // Undo turns the warning on again: the next one asks.
        app.undo().unwrap();
        assert!(app.project().warn_on_selection_reset());
        panel.click(&app, &view, &surface, Combine::Replace);
        assert!(panel.reset.is_some());
        assert!(panel.answer_reset(&mut app, true).is_none());
        assert!(app.project().warn_on_selection_reset());
        assert_eq!(panel.working, tri(&[surface.triangle]));
        // And an edge clicked with Shift is refused on the triangles.
        panel.click_edge(&app, &view, &pick, Combine::Add);
        assert_eq!(panel.working, tri(&[surface.triangle]));
        assert!(panel.message.as_deref().unwrap().contains("Choose Faces"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn req_op_08_req_ui_05_the_tangent_edge_choice_reads_inherit_keep_or_drop() {
        use openchuck_app::Command;
        use openchuck_app::pick::EdgeId;
        use openchuck_app::project::{Item, ModelRef, Placement, RelativePath, SelectionSet};
        assert_eq!(tangent_label(None, false), "Project default (drop)");
        assert_eq!(tangent_label(None, true), "Project default (keep)");
        assert_eq!(tangent_label(Some(true), false), "Keep tangent edges");
        assert_eq!(tangent_label(Some(false), true), "Drop tangent edges");

        // A project with a STEP model and a set of its edges: the panel shows
        // the set with its choice, and the project's default, without error.
        let (dir, mut app, _) = cube_project("tangent-choice");
        let step = ModelRef::new(
            ModelId::new(),
            RelativePath::new("models/part.step").unwrap(),
            ContentHash::of_bytes(b"part"),
        );
        let model = step.id;
        app.execute(Command::Add {
            item: Item::Model(step),
            placement: Placement::End,
        })
        .unwrap();
        let id = SelectionSetId::new();
        app.execute(Command::Add {
            item: Item::SelectionSet(SelectionSet {
                id,
                order: Default::default(),
                name: "Outline".into(),
                model,
                model_hash: ContentHash::of_bytes(b"part"),
                target: SelectionTarget::Edges([EdgeId(3)].into()),
                descriptors: Vec::new(),
                stock_to_leave: None,
                colour: None,
                keep_tangent_edges: Some(true),
            }),
            placement: Placement::End,
        })
        .unwrap();
        let mut panel = SelectionPanel::default();
        panel.show_set(Some(id));
        panel.working = SelectionTarget::Edges([EdgeId(5)].into());
        panel.working_model = Some(model);
        let ctx = egui::Context::default();
        let summaries = app.selection_summaries(&[]);
        assert_eq!(summaries.iter().find(|s| s.id == id).unwrap().kind, "edges");
        let mut error = Some("not drawn".to_owned());
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            error = panel.show(ui, &mut app, &summaries, &SceneView::default());
        });
        output.textures_delta.clear();
        assert_eq!(error, None);
        // The set's own choice stands when the project's default changes.
        app.execute(Command::SetKeepTangentEdges(true)).unwrap();
        let set = app
            .project()
            .selection_sets()
            .iter()
            .find(|s| s.id == id)
            .unwrap()
            .clone();
        assert_eq!(set.keep_tangent_edges, Some(true));
        app.edit_selection(id, SelectionEdit::KeepTangentEdges(None))
            .unwrap();
        let set = app
            .project()
            .selection_sets()
            .iter()
            .find(|s| s.id == id)
            .unwrap()
            .clone();
        assert!(app.project().keeps_tangent_edges(&set));
        let _ = std::fs::remove_dir_all(dir);
    }
}
