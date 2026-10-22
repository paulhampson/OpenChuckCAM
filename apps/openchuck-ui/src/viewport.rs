// SPDX-License-Identifier: AGPL-3.0-or-later
//! The viewport widget: turns the application's scene view model into a
//! renderer scene, handles orbit, pan and zoom, and draws through an
//! `egui_wgpu` paint callback (UI-02, UI-03; decision record 0029). It draws
//! the colour roles and line styles of specification 7.4, the held regions
//! and tabs of 7.5 with their hover and selection, its toolbar (7.1,
//! [`crate::viewport_toolbar`]) and its overlays (7.2,
//! [`crate::viewport_overlays`]); decision record 0119.

use std::sync::Arc;

use eframe::egui::{self, Color32, Modifiers, PointerButton, Rect, Sense, pos2};
use eframe::egui_wgpu;
use openchuck_app::pathview::DeviationBands;
use openchuck_app::pick::{PickTolerance, SceneEdgePick, ScenePick};
use openchuck_app::project::id::ModelId;
use openchuck_app::scene::{
    Mesh as TriangleMesh, SceneMesh, SceneRole, SceneSolid, SceneView, WorkholdingItem,
    triangles_mesh,
};
use openchuck_app::selection::TriangleSet;
use openchuck_viewport::camera::{IDENTITY, Mat4, rigid_inverse, rotation_x, transform};
use openchuck_viewport::wgpu;
use openchuck_viewport::{
    Camera, Colour, DisplayMode, Frame, Hatch, Layer, LineItem, LineStyle, Outline, Projection,
    Renderer, Scene, Slot, StandardView,
};

use crate::results_panel::ColourBy;
use crate::roles;
pub use crate::roles::band_colour;

/// The depth format eframe creates for `depth_buffer: 32`.
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// How near the pointer an edge must pass to be picked, in pixels (decision
/// record 0066).
const EDGE_PIXELS: f32 = 6.0;

/// Degrees of orbit per pixel dragged.
const ORBIT_PER_PIXEL: f64 = 0.4;
/// Zoom per scroll unit: one notch of about 50 points zooms by about 8 %.
const ZOOM_PER_SCROLL: f64 = 0.0015;
/// How long the pointer rests before the probe reads the value under it,
/// seconds (issue 208, decision record 0162).
pub(crate) const PROBE_PAUSE: f64 = 0.3;
/// How far the pointer may drift and still be resting, points.
const PROBE_STILL: f32 = 2.0;

/// The colour of a scene item, and whether it is a guide drawn over the part.
fn style(role: SceneRole) -> (Colour, bool) {
    match role {
        SceneRole::Part => (roles::PART, false),
        SceneRole::Stock => (roles::STOCK, false),
        SceneRole::HeldRegion => (roles::HELD, false),
        SceneRole::Tab => (roles::TAB, false),
        SceneRole::RotaryAxis | SceneRole::ADirection => (roles::ROTARY_AXIS, true),
        SceneRole::WorkX => (roles::WORK_X, true),
        SceneRole::WorkY => (roles::WORK_Y, true),
        SceneRole::WorkZ => (roles::WORK_Z, true),
        SceneRole::Problem => (roles::FINDING, true),
        SceneRole::Hole => (roles::HOLE, true),
        _ => (roles::STOCK, false),
    }
}

/// What the "Show or hide" control of the toolbar shows (7.1, V). Hiding
/// changes only the view, never what is machined.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Visibility {
    /// The part.
    pub part: bool,
    /// The stock outline, and the simulated stock.
    pub stock: bool,
    /// The rotary axis, the A+ arrow and the work axes.
    pub axes: bool,
    /// Held regions and tabs.
    pub workholding: bool,
    /// The selection highlights.
    pub selections: bool,
    /// Toolpaths.
    pub toolpaths: bool,
    /// What the mesh check found (the Model step's "Show findings").
    pub findings: bool,
}

impl Default for Visibility {
    fn default() -> Self {
        Self {
            part: true,
            stock: true,
            axes: true,
            workholding: true,
            selections: true,
            toolpaths: true,
            findings: true,
        }
    }
}

/// What Simulate shows: its three tick boxes, in any combination with at
/// least one ticked (issue 204; decision record 0168).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SimulateShow {
    /// "Model": the part's model.
    pub model: bool,
    /// "Cut part": the stock as the simulation cut it, at the play head, and
    /// the part coloured by deviation while that is on.
    pub cut_part: bool,
    /// "Toolpath".
    pub toolpath: bool,
}

impl Default for SimulateShow {
    /// "Cut part" only (the owner's choice on issue 204).
    fn default() -> Self {
        Self {
            model: false,
            cut_part: true,
            toolpath: false,
        }
    }
}

impl SimulateShow {
    /// How many boxes are ticked.
    pub fn ticked(self) -> usize {
        usize::from(self.model) + usize::from(self.cut_part) + usize::from(self.toolpath)
    }

    /// The choices with at least one ticked: none gives "Cut part", so the
    /// view is never empty.
    #[must_use]
    pub fn never_empty(self) -> Self {
        if self.ticked() == 0 {
            Self::default()
        } else {
            self
        }
    }
}

/// The workflow step the view is in, which chooses the toolbar's controls
/// and the overlays (7.1, 7.2). The shell sets it from its step; with none
/// set the view takes it from what it holds ([`Viewport::step`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewStep {
    /// Model: the import and its findings.
    Model,
    /// Setup.
    Setup,
    /// Operations.
    Operations,
    /// Simulate.
    Simulate,
    /// Post and export.
    Export,
}

/// The workholding draft to draw instead of what the setup stores, while
/// the workholding section has unapplied changes (7.5, 11.7; OI-334 k). The
/// inspector builds it with `workholding_view::display_items`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WorkholdingDraft {
    /// The draft's solids.
    pub meshes: Vec<SceneMesh>,
    /// Which held region or tab each is, indexing `meshes`.
    pub solids: Vec<SceneSolid>,
    /// The held regions and tabs of the draft that fail a check: drawn as
    /// their outline only, in `error` red.
    pub failing: Vec<WorkholdingItem>,
    /// Default tabs offered and not added: dashed sand outlines, no fill.
    pub offered: Vec<SceneMesh>,
    /// Which tab each offered mesh is, with its section for the Setup panes,
    /// indexing `offered`.
    pub offered_solids: Vec<SceneSolid>,
    /// Whether the draft differs from the stored workholding: only then do the views say they
    /// show unapplied workholding (OI-528). A draft that only carries the defaults offered or
    /// the failing parts is the stored one.
    pub unapplied: bool,
}

/// How the scene is drawn besides the view model itself.
#[derive(Debug, Clone, Copy, Default)]
pub struct SceneOptions<'a> {
    /// The part of `bands.0` drawn in the deviation bands of `bands.1`
    /// (SIM-05).
    pub bands: Option<(ModelId, &'a DeviationBands)>,
    /// Held regions and tabs as outlines only: while picking (11.4) and on
    /// Simulate (11.5).
    pub outlines_only: bool,
    /// What is shown.
    pub show: Visibility,
    /// The workholding draft drawn instead of the stored workholding.
    pub draft: Option<&'a WorkholdingDraft>,
}

/// The renderer scene for a view model.
#[cfg(test)]
pub fn to_scene(view: &SceneView) -> Scene {
    to_scene_with(
        view,
        &SceneOptions {
            show: Visibility::default(),
            ..SceneOptions::default()
        },
    )
}

/// How a held region or tab is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SolidLook {
    /// Filled: a translucent cross-hatched held region with a violet outline,
    /// an opaque tab with a dark one.
    Filled,
    /// Its outline only, in its role colour.
    Outline,
    /// Its outline only, in `error` red: a draft that fails a check.
    Failing,
    /// A dashed sand outline: a default tab offered and not added.
    Offered,
}

/// Adds a held region or tab solid to `scene` (7.5).
fn add_solid(scene: &mut Scene, mesh: &TriangleMesh, role: SceneRole, look: SolidLook) {
    let held = role == SceneRole::HeldRegion;
    let (fill, line) = if held {
        (roles::HELD, roles::HELD_LINE)
    } else {
        (roles::TAB, roles::TAB_LINE)
    };
    scene.add_mesh(mesh, &[], fill, line);
    let Some(added) = scene.meshes.last_mut() else {
        return;
    };
    added.slot = Slot::Part;
    let outline = |colour, style| Outline {
        colour,
        width: roles::OUTLINE_WIDTH,
        style,
    };
    match look {
        SolidLook::Filled => {
            added.translucent = held;
            added.hatch = held.then_some(Hatch {
                colour: roles::HELD_LINE,
                cross: true,
            });
            added.outline = Some(outline(line, LineStyle::Solid));
        }
        SolidLook::Outline => {
            let colour = if held { roles::HELD_LINE } else { roles::TAB };
            added.outline_only = true;
            added.outline = Some(outline(colour, LineStyle::Solid));
        }
        SolidLook::Failing => {
            added.outline_only = true;
            added.outline = Some(outline(roles::ERROR, LineStyle::Solid));
        }
        SolidLook::Offered => {
            added.outline_only = true;
            added.outline = Some(outline(roles::TAB, LineStyle::Dashed));
        }
    }
}

/// The renderer scene for a view model (7.4, 7.5). The part, the stock and
/// what is flagged or held on them are fixed to the workpiece, so they turn
/// with A in the machine view; the guides stay where they are.
pub fn to_scene_with(view: &SceneView, options: &SceneOptions<'_>) -> Scene {
    let show = options.show;
    let mut scene = Scene::default();
    for mesh in &view.meshes {
        let (colour, on_top) = style(mesh.role);
        match mesh.role {
            SceneRole::HeldRegion | SceneRole::Tab => {
                if !show.workholding || options.draft.is_some() {
                    continue;
                }
                let look = if options.outlines_only {
                    SolidLook::Outline
                } else {
                    SolidLook::Filled
                };
                add_solid(&mut scene, &mesh.mesh, mesh.role, look);
                continue;
            }
            SceneRole::Part if !show.part => continue,
            SceneRole::Problem if !show.findings => continue,
            _ => {}
        }
        let first = scene.meshes.len();
        match options.bands {
            Some((model, bands)) if mesh.role == SceneRole::Part && mesh.model == Some(model) => {
                // The edges once, then each band's triangles in its colour.
                let edges_only = TriangleMesh {
                    vertices: mesh.mesh.vertices.clone(),
                    triangles: Vec::new(),
                };
                scene.add_mesh(
                    &edges_only,
                    &mesh.feature_edges,
                    colour,
                    roles::FEATURE_EDGES,
                );
                // The triangles in no band shown keep the part's own colour
                // (SIM-05, SIM-15).
                let banded: TriangleSet = bands
                    .bands
                    .iter()
                    .flat_map(|(_, triangles)| triangles.iter().copied())
                    .collect();
                let plain: TriangleSet = (0..mesh.mesh.triangles.len() as u32)
                    .filter(|t| !banded.contains(t))
                    .collect();
                if !plain.is_empty() {
                    scene.add_mesh(
                        &triangles_mesh(&mesh.mesh, &plain),
                        &[],
                        colour,
                        roles::FEATURE_EDGES,
                    );
                }
                for (band, triangles) in &bands.bands {
                    let part = triangles_mesh(&mesh.mesh, triangles);
                    scene.add_mesh(&part, &[], band_colour(*band), roles::FEATURE_EDGES);
                    if roles::band_hatched(*band)
                        && let Some(added) = scene.meshes.last_mut()
                    {
                        added.hatch = Some(Hatch {
                            colour: roles::UNMEASURED_HATCH,
                            cross: false,
                        });
                    }
                }
            }
            _ => scene.add_mesh(
                &mesh.mesh,
                &mesh.feature_edges,
                colour,
                roles::FEATURE_EDGES,
            ),
        }
        // A STEP part's own edges, drawn as its feature edges are, so the
        // display mode shows or hides them alike (slice 11 item 5).
        if let Some(brep) = &mesh.brep {
            let mut edges = TriangleMesh::default();
            let mut pairs = Vec::new();
            for edge in &brep.edges {
                let base = edges.vertices.len() as u32;
                edges.vertices.extend(edge.points.iter().copied());
                let count = edge.points.len() as u32;
                pairs.extend((1..count).map(|i| [base + i - 1, base + i]));
            }
            scene.add_mesh(&edges, &pairs, colour, roles::FEATURE_EDGES);
        }
        for added in scene.meshes.iter_mut().skip(first) {
            added.slot = Slot::Part;
        }
        // Flagged triangles lie on the part's own surface, where their fill
        // fights the part's for depth: their outlines are drawn on top.
        if on_top {
            let point = |i: u32| {
                mesh.mesh
                    .vertices
                    .get(i as usize)
                    .map(|p| [p.x as f32, p.y as f32, p.z as f32])
            };
            let segments = mesh
                .mesh
                .triangles
                .iter()
                .flat_map(|&[a, b, c]| [a, b, b, c, c, a])
                .filter_map(point)
                .collect();
            scene.lines.push(LineItem {
                segments,
                colour,
                on_top: true,
                slot: Slot::Part,
                width: 0.0,
                style: LineStyle::Solid,
            });
        }
    }
    if let Some(draft) = options.draft.filter(|_| show.workholding) {
        for solid in &draft.solids {
            let Some(mesh) = draft.meshes.get(solid.mesh) else {
                continue;
            };
            let look = if draft.failing.contains(&solid.item) {
                SolidLook::Failing
            } else if options.outlines_only {
                SolidLook::Outline
            } else {
                SolidLook::Filled
            };
            add_solid(&mut scene, &mesh.mesh, mesh.role, look);
        }
        for offered in &draft.offered {
            add_solid(
                &mut scene,
                &offered.mesh,
                SceneRole::Tab,
                SolidLook::Offered,
            );
        }
    }
    for line in &view.lines {
        let visible = match line.role {
            // Drawn by the outline of their solid.
            SceneRole::HeldRegion | SceneRole::Tab => false,
            SceneRole::Stock => show.stock,
            SceneRole::RotaryAxis
            | SceneRole::ADirection
            | SceneRole::WorkX
            | SceneRole::WorkY
            | SceneRole::WorkZ => show.axes,
            SceneRole::Problem | SceneRole::Hole => show.findings,
            _ => true,
        };
        if !visible {
            continue;
        }
        let (colour, on_top) = style(line.role);
        let (width, line_style) = match line.role {
            SceneRole::Stock => (roles::LINE_WIDTH, LineStyle::Dashed),
            SceneRole::RotaryAxis => (roles::LINE_WIDTH, LineStyle::DashDot),
            SceneRole::ADirection => (roles::OUTLINE_WIDTH, LineStyle::Solid),
            SceneRole::WorkX | SceneRole::WorkY | SceneRole::WorkZ => {
                (roles::AXIS_WIDTH, LineStyle::Solid)
            }
            SceneRole::Hole => (roles::HOVERED_WIDTH, LineStyle::Solid),
            _ => (roles::LINE_WIDTH, LineStyle::Solid),
        };
        scene.add_styled_polyline(&line.points, colour, on_top, width, line_style);
        if matches!(
            line.role,
            SceneRole::Stock | SceneRole::Problem | SceneRole::Hole
        ) && let Some(added) = scene.lines.last_mut()
        {
            added.slot = Slot::Part;
        }
    }
    scene
}

fn to_color32(c: Colour) -> Color32 {
    roles::to_color32(c)
}

/// What the renderer keeps between frames, in egui's callback resources.
struct Resources {
    renderer: Renderer,
    /// The generation of each layer the renderer holds.
    generations: [u64; Layer::ALL.len()],
}

/// One frame's draw request.
struct Callback {
    frame: Frame,
    /// Every layer with its generation and whether it is shown: the
    /// renderer uploads the ones whose generation it has not seen, and keeps
    /// a hidden one uploaded without drawing it, so showing it again costs
    /// nothing (issue #200).
    layers: Vec<(Layer, u64, Arc<Scene>, bool)>,
}

impl egui_wgpu::CallbackTrait for Callback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen: &egui_wgpu::ScreenDescriptor,
        _encoder: &mut wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        if let Some(r) = resources.get_mut::<Resources>() {
            for (layer, generation, scene, shown) in &self.layers {
                if let Some(seen) = r.generations.get_mut(layer.index())
                    && *seen != *generation
                {
                    r.renderer.set_layer(device, *layer, scene);
                    *seen = *generation;
                }
                r.renderer.set_layer_shown(*layer, *shown);
            }
            r.renderer.prepare(device, queue, &self.frame);
        }
        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        pass: &mut wgpu::RenderPass<'static>,
        resources: &egui_wgpu::CallbackResources,
    ) {
        if let Some(r) = resources.get::<Resources>() {
            r.renderer.paint(pass);
        }
    }
}

/// Installs the renderer in eframe's wgpu state. Returns false when eframe
/// runs without wgpu, in which case the viewport shows a message.
pub fn install(cc: &eframe::CreationContext<'_>) -> bool {
    let Some(state) = cc.wgpu_render_state.as_ref() else {
        return false;
    };
    let renderer = Renderer::new(&state.device, state.target_format, DEPTH_FORMAT, 1);
    state.renderer.write().callback_resources.insert(Resources {
        renderer,
        generations: [0; Layer::ALL.len()],
    });
    true
}

/// The viewport's state between frames.
pub struct Viewport {
    /// The camera.
    pub camera: Camera,
    /// How meshes are drawn.
    pub mode: DisplayMode,
    available: bool,
    scene: Arc<Scene>,
    view: SceneView,
    generation: u64,
    /// Fit the camera to the scene on the next frame (a new project).
    fit_pending: bool,
    aspect: f64,
    /// The generation last handed to the renderer.
    drawn: u64,
    /// The viewport size in physical pixels, at the last frame.
    pixels: [u32; 2],
    /// The part coloured by deviation, when asked for (SIM-05).
    bands: Option<(ModelId, DeviationBands)>,
    /// What the part is coloured by, for the probe's text.
    pub(crate) colour_by: ColourBy,
    /// The deviation bands shown (SIM-15), for the colour scale's marks.
    pub(crate) band_filter: crate::results_panel::BandFilter,
    /// Where the pointer came to rest and when (`egui` time, seconds).
    rest: Option<(egui::Pos2, f64)>,
    /// The triangle of the coloured part under the resting pointer, and the
    /// one a click kept, with the point clicked in the part's frame (SIM-11
    /// in part; decision record 0162).
    pub(crate) probe_hover: Option<u32>,
    pub(crate) probe_pinned: Option<(u32, [f64; 3])>,
    /// The layers besides the scene: highlights, toolpaths, stock, tool.
    layers: Vec<(Layer, Arc<Scene>, u64)>,
    /// The transform of the part's slot and of the tool's slot.
    part_transform: Mat4,
    tool_transform: Mat4,
    /// The part's turn about the rotary axis, degrees.
    part_turn: f64,
    /// Whether the pointer picks triangles of the part (slice 10 item 1).
    picking: bool,
    /// What the pointer may pick (slice 11 item 5).
    filter: PickFilter,
    input: PickInput,
    /// What is shown (7.1, V).
    pub(crate) show: Visibility,
    /// What Simulate shows (7.1).
    pub(crate) simulate_show: SimulateShow,
    /// The step set by the shell, or `None` to take it from what is held.
    step: Option<ViewStep>,
    /// Whether the open project holds a model: the one input of the rule that
    /// switches the view directions on (`viewport_toolbar::directions_unavailable`).
    project_has_models: bool,
    /// The workholding draft drawn instead of the stored workholding.
    draft: Option<WorkholdingDraft>,
    /// The held region or tab under the pointer, by index into the shown
    /// solids.
    hover_solid: Option<usize>,
    /// The held region or tab whose inspector row has keyboard focus (7.5).
    focused_item: Option<WorkholdingItem>,
    /// The selected held region or tab (7.5).
    selected_item: Option<WorkholdingItem>,
    /// The highlights of the hovered and selected solids, as last built.
    focus_key: (Option<WorkholdingItem>, Option<WorkholdingItem>, u64),
    focus: (Arc<Scene>, u64),
    /// The point on the surface under the pointer, in the model frame, for
    /// the readout.
    pub(crate) cursor_point: Option<[f64; 3]>,
    /// The tool's position for the readout on Simulate.
    pub(crate) tool_readout: Option<ToolReadout>,
    /// The path legend is open while the view is too low to show it (7.2).
    pub(crate) legend_open: bool,
    /// The "Show or hide" list is open (V).
    pub(crate) show_menu_open: bool,
    /// Where the view was drawn last frame (the import preview's overlays
    /// sit at its corners).
    pub(crate) last_rect: Rect,
}

/// Where the tool is, for the readout (7.2): X, Y, Z and A as the program
/// gives them, in work coordinates, and the move.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ToolReadout {
    /// X, Y, Z in millimetres and A in degrees.
    pub work: [f64; 4],
    /// The move, counted from 1.
    pub move_number: usize,
}

/// What the pointer may pick (decision record 0066 item 3). With `Both`, the
/// default, an edge is picked when the pointer is within reach of one and the
/// surface under it otherwise; the other two restrict picking to one kind.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PickFilter {
    /// Edges near the pointer, else the surface under it.
    #[default]
    Both,
    /// Only the surface under the pointer: triangles, or a STEP part's faces.
    Faces,
    /// Only edges near the pointer.
    Edges,
}

/// What the pointer did over the part in the last frame (UI-04).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PickInput {
    /// The triangle under the pointer.
    pub hover: Option<ScenePick>,
    /// The triangle clicked, and the keys held.
    pub click: Option<(ScenePick, Modifiers)>,
    /// The edge near the pointer, unless only faces are picked.
    pub edge_hover: Option<SceneEdgePick>,
    /// The edge clicked, and the keys held.
    pub edge_click: Option<(SceneEdgePick, Modifiers)>,
    /// The held region or tab clicked (7.5): the shell selects its setup and
    /// opens the inspector at the workholding section with it expanded.
    pub solid_click: Option<WorkholdingItem>,
}

impl Viewport {
    /// A viewport; `available` is false when there is no wgpu renderer.
    pub fn new(available: bool) -> Self {
        Self {
            camera: Camera::default(),
            mode: DisplayMode::default(),
            available,
            scene: Arc::new(Scene::default()),
            view: SceneView::default(),
            generation: 0,
            fit_pending: true,
            aspect: 1.0,
            drawn: 0,
            pixels: [0, 0],
            bands: None,
            colour_by: ColourBy::default(),
            band_filter: crate::results_panel::BandFilter::default(),
            rest: None,
            probe_hover: None,
            probe_pinned: None,
            layers: Vec::new(),
            part_transform: IDENTITY,
            tool_transform: IDENTITY,
            part_turn: 0.0,
            picking: false,
            filter: PickFilter::Both,
            input: PickInput::default(),
            show: Visibility::default(),
            simulate_show: SimulateShow::default(),
            step: None,
            project_has_models: false,
            draft: None,
            hover_solid: None,
            focused_item: None,
            selected_item: None,
            focus_key: (None, None, 0),
            focus: (Arc::new(Scene::default()), 0),
            cursor_point: None,
            tool_readout: None,
            legend_open: false,
            show_menu_open: false,
            last_rect: Rect::NOTHING,
        }
    }

    /// Builds the renderer scene again from the view model and the options,
    /// for the next frame.
    fn rebuild(&mut self) {
        let options = SceneOptions {
            bands: self.bands.as_ref().map(|(m, b)| (*m, b)),
            outlines_only: self.picking || self.bands.is_some(),
            show: Visibility {
                part: self.part_shown(),
                ..self.show
            },
            draft: self.draft.as_ref(),
        };
        self.scene = Arc::new(to_scene_with(&self.view, &options));
        self.generation += 1;
    }

    /// Shows a new scene. With `fit` the camera is fitted to it.
    pub fn set_scene(&mut self, view: SceneView, fit: bool) {
        self.view = view;
        self.hover_solid = None;
        self.rebuild();
        self.fit_pending |= fit;
    }

    /// Colours the part of `model` by its deviation (SIM-05), or back to one
    /// colour for `None`. The scene is uploaded again.
    pub fn set_bands(&mut self, bands: Option<(ModelId, DeviationBands)>) {
        if self.bands == bands {
            return;
        }
        // A kept value belongs to the part it was read on.
        if bands.as_ref().map(|(m, _)| *m) != self.bands.as_ref().map(|(m, _)| *m) {
            self.probe_pinned = None;
            self.probe_hover = None;
        }
        self.bands = bands;
        self.rebuild();
    }

    /// The deviation bands shown, for the colour scale.
    pub(crate) fn bands(&self) -> Option<&DeviationBands> {
        self.bands.as_ref().map(|(_, b)| b)
    }

    /// Replaces the highlights drawn over the scene.
    pub fn set_overlay(&mut self, overlay: Scene) {
        self.set_layer(Layer::Overlay, overlay);
    }

    /// Replaces one layer: toolpaths, the stock, the tool, the highlights.
    /// The renderer uploads it on the next frame; the other layers are not
    /// touched.
    pub fn set_layer(&mut self, layer: Layer, scene: Scene) {
        let scene = Arc::new(scene);
        match self.layers.iter_mut().find(|(l, ..)| *l == layer) {
            Some((_, old, generation)) => {
                *old = scene;
                *generation += 1;
            }
            None => self.layers.push((layer, scene, 1)),
        }
    }

    /// Whether a layer holds anything to draw.
    fn holds(&self, layer: Layer) -> bool {
        self.layers
            .iter()
            .any(|(l, s, _)| *l == layer && (!s.meshes.is_empty() || !s.lines.is_empty()))
    }

    /// Sets the step the view is in (7.1, 7.2), or `None` to take it from
    /// what the view holds.
    pub fn set_step(&mut self, step: Option<ViewStep>) {
        if self.step != step {
            let part = self.part_shown();
            self.step = step;
            if self.part_shown() != part {
                self.rebuild();
            }
        }
    }

    /// Sets Simulate's tick boxes (issue 204); none ticked gives "Cut part".
    pub fn set_simulate_show(&mut self, show: SimulateShow) {
        let show = show.never_empty();
        if self.simulate_show != show {
            let part = self.part_shown();
            self.simulate_show = show;
            if self.part_shown() != part {
                self.rebuild();
            }
        }
    }

    /// Simulate's tick boxes.
    pub fn simulate_show(&self) -> SimulateShow {
        self.simulate_show
    }

    /// Whether the part is drawn: "Show or hide" says so and, on Simulate,
    /// "Model" is ticked, or "Cut part" while the part is coloured by
    /// deviation (what the cut left is shown on the part's surface).
    pub(crate) fn part_shown(&self) -> bool {
        let simulate = self.simulate_show;
        self.show.part
            && (self.step() != ViewStep::Simulate
                || simulate.model
                || (simulate.cut_part && self.bands.is_some()))
    }

    /// Tells the view whether the open project holds a model. The toolbar and the View
    /// menu read the same value, so they never disagree (OI-514).
    pub fn set_project_has_models(&mut self, has_models: bool) {
        self.project_has_models = has_models;
    }

    /// Whether the open project holds a model.
    #[must_use]
    pub fn project_has_models(&self) -> bool {
        self.project_has_models
    }

    /// The step the view is in: the one the shell set, or else Simulate when
    /// the part is coloured by deviation or the stock is shown, Operations
    /// while picking or when toolpaths are drawn, Model when the scene has
    /// findings, and Setup otherwise.
    pub fn step(&self) -> ViewStep {
        if let Some(step) = self.step {
            return step;
        }
        if self.bands.is_some() || self.holds(Layer::Stock) {
            ViewStep::Simulate
        } else if self.picking || self.holds(Layer::Paths) {
            ViewStep::Operations
        } else if self.has_findings() {
            ViewStep::Model
        } else {
            ViewStep::Setup
        }
    }

    /// Whether the scene has mesh findings to show or hide.
    pub(crate) fn has_findings(&self) -> bool {
        self.view
            .meshes
            .iter()
            .map(|m| m.role)
            .chain(self.view.lines.iter().map(|l| l.role))
            .any(|r| matches!(r, SceneRole::Problem | SceneRole::Hole))
    }

    /// Sets what is shown (7.1, V).
    pub fn set_visibility(&mut self, show: Visibility) {
        if self.show != show {
            self.show = show;
            self.rebuild();
        }
    }

    /// What is shown.
    pub fn visibility(&self) -> Visibility {
        self.show
    }

    /// Draws `draft` instead of the setup's stored workholding, or the
    /// stored one again for `None` (7.5, 11.7). While a draft is drawn a
    /// chip says so.
    pub fn set_workholding_draft(&mut self, draft: Option<WorkholdingDraft>) {
        if self.draft != draft {
            self.draft = draft;
            self.hover_solid = None;
            self.rebuild();
        }
    }

    /// Whether a draft is drawn.
    #[cfg(test)]
    pub(crate) fn shows_draft(&self) -> bool {
        self.draft.is_some()
    }

    /// Whether the draft drawn has changes the stored workholding does not (the chip's rule).
    pub(crate) fn shows_unapplied(&self) -> bool {
        self.draft.as_ref().is_some_and(|d| d.unapplied)
    }

    /// The held region or tab solids shown, with the meshes their indices
    /// name: the draft's while one is drawn.
    pub(crate) fn solids(&self) -> (&[SceneMesh], &[SceneSolid]) {
        match &self.draft {
            Some(d) => (&d.meshes, &d.solids),
            None => (&self.view.meshes, &self.view.solids),
        }
    }

    /// Outlines the held region or tab whose inspector row has keyboard
    /// focus, as a hover does (7.5, principle 9).
    pub fn set_focused_solid(&mut self, item: Option<WorkholdingItem>) {
        self.focused_item = item;
    }

    /// Selects a held region or tab, drawn with a 3-pixel accent outline
    /// while it stays selected (7.5), or none.
    pub fn set_selected_solid(&mut self, item: Option<WorkholdingItem>) {
        self.selected_item = item;
    }

    /// The selected held region or tab.
    pub fn selected_solid(&self) -> Option<WorkholdingItem> {
        self.selected_item
    }

    /// The default tabs offered and not added, for the Setup panes (7.5);
    /// none while the stored workholding is drawn.
    pub(crate) fn offered_solids(&self) -> &[SceneSolid] {
        self.draft
            .as_ref()
            .map_or(&[], |d| d.offered_solids.as_slice())
    }

    /// The held regions and tabs of the draft that fail a check, for the
    /// Setup panes (7.5); none while the stored workholding is drawn.
    pub(crate) fn failing_solids(&self) -> &[WorkholdingItem] {
        self.draft.as_ref().map_or(&[], |d| d.failing.as_slice())
    }

    /// The held regions and tabs the Setup panes outline: the one whose
    /// inspector row has keyboard focus and the selected one (7.5).
    pub(crate) fn marked_solids(&self) -> (Option<WorkholdingItem>, Option<WorkholdingItem>) {
        (self.focused_item, self.selected_item)
    }

    /// A click on a held region or tab in a Setup pane: selects it and
    /// reports it as a click in the view does (7.5).
    pub(crate) fn click_solid(&mut self, item: WorkholdingItem) {
        self.selected_item = Some(item);
        self.input.solid_click = Some(item);
    }

    /// The held region or tab under the pointer.
    pub fn hovered_solid(&self) -> Option<&SceneSolid> {
        self.hover_solid.and_then(|i| self.solids().1.get(i))
    }

    /// Turns the workpiece (and what is fixed to it) by `degrees` about the
    /// rotary axis in the machine view (SIM-02).
    pub fn set_part_turn(&mut self, degrees: f64) {
        self.part_turn = degrees;
        self.part_transform = if degrees.abs() < 1e-12 {
            IDENTITY
        } else {
            rotation_x(degrees.to_radians())
        };
    }

    /// The part's turn about the rotary axis, degrees.
    pub(crate) fn part_turn(&self) -> f64 {
        self.part_turn
    }

    /// Puts the tool where the playback is: a transform that takes a tool
    /// built with its tip at the origin and its axis along +Z to its place.
    pub fn set_tool_transform(&mut self, transform: Mat4) {
        self.tool_transform = transform;
    }

    /// Gives the readout the tool's position and move (7.2), or none.
    pub fn set_tool_readout(&mut self, readout: Option<ToolReadout>) {
        self.tool_readout = readout;
    }

    /// Whether picking is on.
    pub fn is_picking(&self) -> bool {
        self.picking
    }

    /// Chooses what the pointer may pick (slice 11 item 5).
    pub fn set_pick_filter(&mut self, filter: PickFilter) {
        self.filter = filter;
    }

    /// Lets the pointer pick triangles of the part: hover and click are then
    /// reported by [`take_input`](Self::take_input) and a click no longer
    /// only orbits. While picking, held regions and tabs are outlines only
    /// and cannot be picked (7.5).
    pub fn set_picking(&mut self, on: bool) {
        if self.picking != on {
            self.picking = on;
            self.hover_solid = None;
            self.rebuild();
        }
        if !on {
            self.input = PickInput::default();
        }
    }

    /// What the pointer did in the last frame, once.
    pub fn take_input(&mut self) -> PickInput {
        std::mem::take(&mut self.input)
    }

    /// The scene a layer holds (tests).
    #[cfg(test)]
    pub fn layer(&self, layer: Layer) -> Option<&Scene> {
        if layer == Layer::Focus {
            return Some(self.focus.0.as_ref());
        }
        self.layers
            .iter()
            .find(|(l, ..)| *l == layer)
            .map(|(_, s, _)| s.as_ref())
    }

    /// The scene drawn (tests).
    #[cfg(test)]
    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    /// The transforms of the part's slot and the tool's slot (tests).
    #[cfg(test)]
    pub fn transforms(&self) -> (Mat4, Mat4) {
        (self.part_transform, self.tool_transform)
    }

    /// The scene view model shown.
    pub fn view(&self) -> &SceneView {
        &self.view
    }

    /// Whether a non-empty scene has been handed to the renderer.
    pub fn is_drawn(&self) -> bool {
        self.drawn > 0 && self.drawn == self.generation && self.scene.triangle_count() > 0
    }

    /// Triangles and feature edges in the scene.
    pub fn counts(&self) -> (usize, usize) {
        let edges = self
            .scene
            .meshes
            .iter()
            .map(|m| m.feature_edges.len() / 2)
            .sum();
        (self.scene.triangle_count(), edges)
    }

    /// The viewport size in physical pixels at the last frame.
    pub fn pixels(&self) -> [u32; 2] {
        self.pixels
    }

    /// Sets the size the last frame had, in physical pixels (tests).
    #[cfg(test)]
    pub(crate) fn set_pixels_for_tests(&mut self, pixels: [u32; 2]) {
        self.pixels = pixels;
    }

    /// The millimetres one physical pixel spans on the nearest part of the
    /// scene, which the level of detail of the toolpath follows (decision
    /// record 0167): at the camera's target in the orthographic view, and
    /// in perspective where the scene's sphere comes nearest the eye, where
    /// a pixel spans least. `None` before the first frame, or with the eye
    /// inside the scene.
    pub fn mm_per_pixel(&self) -> Option<f64> {
        let rows = f64::from(self.pixels[1]);
        if rows < 1.0 {
            return None;
        }
        let camera = &self.camera;
        let nearest = match camera.projection {
            Projection::Orthographic => 1.0,
            Projection::Perspective => (camera.distance - camera.scene_radius) / camera.distance,
        };
        let mm = camera.visible_height() / rows * nearest;
        (mm.is_finite() && mm > 0.0).then_some(mm)
    }

    /// Fits the camera to the scene.
    pub fn fit(&mut self) {
        if let Some((min, max)) = self.scene.bounds() {
            self.camera.fit(min, max, self.aspect);
        }
    }

    /// Turns to a standard view and fits the scene.
    pub fn set_view(&mut self, view: StandardView) {
        self.camera.set_view(view);
        self.fit();
    }

    /// Sets the projection.
    pub fn set_projection(&mut self, projection: Projection) {
        self.camera.projection = projection;
    }

    /// Draws the toolbar strip (7.1) and under it the view, filling `ui`.
    pub fn show(&mut self, ui: &mut egui::Ui) {
        let rect = ui.available_rect_before_wrap();
        let height = crate::viewport_toolbar::HEIGHT.min(rect.height());
        let (strip, view) = rect.split_top_bottom_at_y(rect.top() + height);
        crate::viewport_toolbar::show(ui, self, strip);
        self.show_view_in(ui, view);
    }

    /// Draws the view in `rect`, with its overlays, and handles its input.
    fn show_view_in(&mut self, ui: &mut egui::Ui, rect: Rect) {
        self.last_rect = rect;
        let response = ui.allocate_rect(rect, Sense::click_and_drag());
        paint_gradient(ui.painter(), rect);
        if !self.available {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "The 3D view needs the wgpu renderer, which is not available.",
                egui::FontId::proportional(14.0),
                roles::OVERLAY_TEXT,
            );
            return;
        }
        if rect.width() < 1.0 || rect.height() < 1.0 {
            return;
        }
        self.aspect = f64::from(rect.width() / rect.height());
        let scale = ui.ctx().pixels_per_point();
        self.pixels = [
            (rect.width() * scale).round() as u32,
            (rect.height() * scale).round() as u32,
        ];
        if self.fit_pending && self.scene.bounds().is_some() {
            self.fit();
            self.fit_pending = false;
        }
        self.keys(ui);

        // The overlays take their clicks before the view does.
        let overlay_used = crate::viewport_overlays::interact(ui, self, rect);

        // Orbit with the primary button, pan with the secondary or middle.
        let delta = response.drag_delta();
        let (dx, dy) = (f64::from(delta.x), f64::from(delta.y));
        if response.dragged_by(PointerButton::Primary) {
            self.camera
                .orbit(dx * ORBIT_PER_PIXEL, dy * ORBIT_PER_PIXEL);
        } else if response.dragged_by(PointerButton::Secondary)
            || response.dragged_by(PointerButton::Middle)
        {
            self.camera.pan(dx, dy, f64::from(rect.height()));
        }
        if response.hovered() {
            let scroll = f64::from(ui.input(|i| i.smooth_scroll_delta.y));
            if scroll != 0.0 {
                self.camera.zoom((-scroll * ZOOM_PER_SCROLL).exp());
            }
        }

        if overlay_used {
            self.input = PickInput::default();
            self.hover_solid = None;
            self.cursor_point = None;
            self.probe_hover = None;
            self.rest = None;
        } else {
            self.pick(ui, &response, rect);
        }
        self.update_focus();

        let mut frame = Frame::new(self.camera, self.aspect, self.mode);
        frame.viewport = [rect.width(), rect.height()];
        frame.pixels_per_point = scale;
        frame.set_transform(Slot::Part, self.part_transform);
        frame.set_transform(Slot::Tool, self.tool_transform);
        let layers = self.frame_layers();
        ui.painter().add(egui_wgpu::Callback::new_paint_callback(
            rect,
            Callback { frame, layers },
        ));
        self.drawn = self.generation;
        self.draw_labels(ui, rect);
        self.draw_pinned(ui, rect);
        if let Some(solid) = self.hovered_solid() {
            let text = solid.description.clone();
            response.on_hover_ui_at_pointer(|ui| {
                ui.label(text);
            });
        } else if let Some(text) = self.probe_hover.and_then(|t| self.probe_text(t)) {
            response.on_hover_ui_at_pointer(|ui| {
                ui.label(text);
            });
        }
        crate::viewport_overlays::paint(ui, self, rect);
    }

    /// Every layer for the next frame, with its generation and whether it
    /// is shown. A hidden layer is sent as it is, so the renderer keeps it
    /// uploaded and showing it again uploads nothing (issue #200).
    fn frame_layers(&self) -> Vec<(Layer, u64, Arc<Scene>, bool)> {
        std::iter::once((Layer::Scene, self.generation, Arc::clone(&self.scene), true))
            .chain(
                self.layers
                    .iter()
                    .map(|(l, s, g)| (*l, *g, Arc::clone(s), self.layer_shown(*l))),
            )
            .chain(std::iter::once((
                Layer::Focus,
                self.focus.1,
                Arc::clone(&self.focus.0),
                true,
            )))
            .collect()
    }

    /// The probe (SIM-11 in part; issue 208, decision record 0162): with the
    /// part coloured by deviation, the triangle under the pointer once it
    /// has rested for [`PROBE_PAUSE`], and on a click the triangle clicked,
    /// kept until a click off the part. It reads the value held with the
    /// bands; the hit is the one the readout already picked on the CPU.
    fn probe(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        pointer: egui::Pos2,
        surface: Option<&ScenePick>,
    ) {
        let Some((model, _)) = &self.bands else {
            self.rest = None;
            return;
        };
        // A hidden part is still picked, but is not there to read.
        if !self.part_shown() {
            self.probe_hover = None;
            self.rest = None;
            return;
        }
        let on_part = surface.filter(|h| h.model == Some(*model));
        if response.clicked_by(PointerButton::Primary) {
            self.probe_pinned = on_part.map(|h| (h.triangle, [h.point.x, h.point.y, h.point.z]));
        }
        let now = ui.input(|i| i.time);
        if self
            .rest
            .is_none_or(|(at, _)| at.distance(pointer) > PROBE_STILL)
        {
            self.rest = Some((pointer, now));
        }
        let rested = self.rest.map_or(0.0, |(_, since)| now - since);
        if rested >= PROBE_PAUSE {
            self.probe_hover = on_part.map(|h| h.triangle);
        } else {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_secs_f64(PROBE_PAUSE - rested));
        }
    }

    /// The probe's text for a triangle of the coloured part.
    pub(crate) fn probe_text(&self, triangle: u32) -> Option<String> {
        let (_, bands) = self.bands.as_ref()?;
        let value = bands.values.get(triangle as usize).copied().flatten();
        Some(crate::results_panel::probe_text(
            self.colour_by,
            value,
            bands.tolerance,
        ))
    }

    /// Draws the kept probe value at its point, when the point is in view.
    fn draw_pinned(&self, ui: &egui::Ui, rect: Rect) {
        let Some((triangle, at)) = self.probe_pinned else {
            return;
        };
        if !self.part_shown() {
            return;
        }
        let Some(text) = self.probe_text(triangle) else {
            return;
        };
        let world = transform(&self.part_transform, [at[0], at[1], at[2], 1.0]);
        let Some(ndc) = self
            .camera
            .project([world[0], world[1], world[2]], self.aspect)
        else {
            return;
        };
        if !(0.0..=1.0).contains(&ndc[2]) {
            return;
        }
        let point = pos2(
            rect.left() + ((ndc[0] + 1.0) * 0.5) as f32 * rect.width(),
            rect.top() + ((1.0 - ndc[1]) * 0.5) as f32 * rect.height(),
        );
        if !rect.contains(point) {
            return;
        }
        let painter = ui.painter_at(rect);
        painter.circle(
            point,
            3.0,
            roles::OVERLAY_TEXT,
            egui::Stroke::new(1.0, Color32::BLACK),
        );
        let galley =
            painter.layout_no_wrap(text, egui::FontId::proportional(13.0), roles::OVERLAY_TEXT);
        let at = point + egui::vec2(8.0, -8.0 - galley.size().y);
        let frame = Rect::from_min_size(at, galley.size()).expand(4.0);
        painter.rect_filled(frame, 3.0, Color32::from_black_alpha(200));
        painter.galley(at, galley, roles::OVERLAY_TEXT);
    }

    /// Whether a layer other than the scene is drawn, after "Show or hide"
    /// and Simulate's "Show".
    fn layer_shown(&self, layer: Layer) -> bool {
        // The Show choice is Simulate's; it hides nothing on the other steps,
        // where the toolbar does not offer it (7.1).
        let simulate = if self.step() == ViewStep::Simulate {
            self.simulate_show
        } else {
            SimulateShow {
                model: true,
                cut_part: true,
                toolpath: true,
            }
        };
        match layer {
            Layer::Overlay => self.show.selections,
            Layer::Paths | Layer::Trace => self.show.toolpaths && simulate.toolpath,
            Layer::Stock => self.show.stock && simulate.cut_part,
            _ => true,
        }
    }

    /// F fits, V opens "Show or hide" (7.1, section 16), and Esc clears the
    /// selected held region or tab (7.5); not while a text field has the
    /// keyboard.
    fn keys(&mut self, ui: &egui::Ui) {
        let ctx = ui.ctx();
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, egui::Key::F)) {
            self.fit();
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, egui::Key::V)) {
            self.show_menu_open = !self.show_menu_open;
        }
        if !self.picking
            && self.selected_item.is_some()
            && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, egui::Key::Escape))
        {
            self.selected_item = None;
        }
    }

    /// Hover and click picking (UI-04): the pointer becomes a ray, the scene's
    /// picker finds the edge within reach of it or else the triangle under
    /// it, and the shell is told. Not run while the pointer is dragging the
    /// camera. When not picking, the ray finds the held region or tab under
    /// the pointer instead (7.5), unless the part is in front of it, and the
    /// point for the readout.
    fn pick(&mut self, ui: &egui::Ui, response: &egui::Response, rect: Rect) {
        self.input = PickInput::default();
        self.hover_solid = None;
        self.cursor_point = None;
        self.probe_hover = None;
        if response.dragged() {
            self.rest = None;
            return;
        }
        let Some(pointer) = response.hover_pos() else {
            self.rest = None;
            return;
        };
        let ndc = [
            f64::from((pointer.x - rect.left()) / rect.width()) * 2.0 - 1.0,
            1.0 - f64::from((pointer.y - rect.top()) / rect.height()) * 2.0,
        ];
        let (origin, direction) = self.camera.ray(ndc, self.aspect);
        // The part is drawn turned by its slot's transform (the machine view),
        // so the ray is carried into the part's own frame: the hit is then the
        // triangle under the pointer, in the frame the selections use.
        let back = rigid_inverse(&self.part_transform);
        let o = transform(&back, [origin[0], origin[1], origin[2], 1.0]);
        let d = transform(&back, [direction[0], direction[1], direction[2], 0.0]);
        let (o, d) = ([o[0], o[1], o[2]], [d[0], d[1], d[2]]);
        let surface = self
            .view
            .picking
            .picker()
            .and_then(|p| p.pick(o, d).ok().flatten());
        self.cursor_point = surface.map(|h| [h.point.x, h.point.y, h.point.z]);
        if !self.picking {
            self.probe(ui, response, pointer, surface.as_ref());
        }
        if !self.picking {
            if self.show.workholding {
                let near = surface.map_or(f64::INFINITY, |h| h.distance);
                self.hover_solid = self.solid_under(o, d, near);
                if let Some(solid) = self.hovered_solid() {
                    let item = solid.item;
                    if response.clicked_by(PointerButton::Primary) {
                        self.selected_item = Some(item);
                        self.input.solid_click = Some(item);
                    }
                }
            }
            return;
        }
        let Some(picker) = self.view.picking.picker() else {
            return;
        };
        if self.filter != PickFilter::Faces {
            // The reach is what EDGE_PIXELS span: the ray a few pixels over
            // gives how far apart the rays start and how fast they part.
            let beside = [ndc[0] + f64::from(2.0 * EDGE_PIXELS / rect.width()), ndc[1]];
            let (o2, d2) = self.camera.ray(beside, self.aspect);
            let unit = |v: [f64; 3]| {
                let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-300);
                v.map(|x| x / l)
            };
            let gap = |a: [f64; 3], b: [f64; 3]| {
                ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
            };
            let tolerance = PickTolerance {
                base: gap(origin, o2),
                per_distance: gap(unit(direction), unit(d2)),
            };
            if let Ok(Some(hit)) = picker.pick_edge(o, d, tolerance) {
                self.input.edge_hover = Some(hit);
                ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
                if response.clicked_by(PointerButton::Primary) {
                    self.input.edge_click = Some((hit, ui.input(|i| i.modifiers)));
                }
                return;
            }
        }
        if self.filter == PickFilter::Edges {
            return;
        }
        let Some(hit) = surface else {
            return;
        };
        self.input.hover = Some(hit);
        ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
        if response.clicked_by(PointerButton::Primary) {
            self.input.click = Some((hit, ui.input(|i| i.modifiers)));
        }
    }

    /// The nearest held region or tab the ray from `o` along `d` (in the
    /// part's frame) meets before `near`, by index into the shown solids.
    fn solid_under(&self, o: [f64; 3], d: [f64; 3], near: f64) -> Option<usize> {
        let (meshes, solids) = self.solids();
        let mut best: Option<(f64, usize)> = None;
        for (i, solid) in solids.iter().enumerate() {
            let Some(mesh) = meshes.get(solid.mesh) else {
                continue;
            };
            if let Some(t) = ray_mesh(o, d, &mesh.mesh)
                && t < near
                && best.is_none_or(|(b, _)| t < b)
            {
                best = Some((t, i));
            }
        }
        best.map(|(_, i)| i)
    }

    /// Builds the outlines of the hovered and selected held region or tab
    /// (7.5) when they change: hovered 2 pixels in yellow, selected 3 in
    /// `accent`, a tab around the axis all its solids together.
    fn update_focus(&mut self) {
        let hovered = self
            .hovered_solid()
            .map(|s| s.item)
            .or(self.focused_item)
            .filter(|_| !self.picking && self.show.workholding);
        let selected = self.selected_item.filter(|_| self.show.workholding);
        let key = (hovered, selected, self.generation);
        if key == self.focus_key {
            return;
        }
        self.focus_key = key;
        let mut scene = Scene::default();
        let (meshes, solids) = self.solids();
        for (item, colour, width) in [
            (selected, roles::ACCENT, roles::SELECTED_WIDTH),
            (
                hovered.filter(|h| Some(*h) != selected),
                roles::HOVER,
                roles::HOVERED_WIDTH,
            ),
        ] {
            let Some(item) = item else {
                continue;
            };
            for solid in solids.iter().filter(|s| s.item == item) {
                if let Some(mesh) = meshes.get(solid.mesh) {
                    scene.add_mesh(&mesh.mesh, &[], colour, colour);
                    if let Some(added) = scene.meshes.last_mut() {
                        added.slot = Slot::Part;
                        added.outline_only = true;
                        added.outline = Some(Outline {
                            colour,
                            width,
                            style: LineStyle::Solid,
                        });
                    }
                }
            }
        }
        self.focus = (Arc::new(scene), self.focus.1 + 1);
    }

    /// Draws the scene's labels ("A+", the work axes) at their projected
    /// points, and an arrowhead at the end of each work axis (7.3, 7.4).
    fn draw_labels(&self, ui: &egui::Ui, rect: Rect) {
        if !self.show.axes {
            return;
        }
        let painter = ui.painter_at(rect);
        let to_screen = |p: [f64; 3]| {
            let ndc = self.camera.project(p, self.aspect)?;
            (0.0..=1.0).contains(&ndc[2]).then(|| {
                pos2(
                    rect.left() + ((ndc[0] + 1.0) * 0.5) as f32 * rect.width(),
                    rect.top() + ((1.0 - ndc[1]) * 0.5) as f32 * rect.height(),
                )
            })
        };
        for line in self.view.lines.iter().filter(|l| {
            matches!(
                l.role,
                SceneRole::WorkX | SceneRole::WorkY | SceneRole::WorkZ
            )
        }) {
            let (Some(a), Some(b)) = (line.points.first(), line.points.last()) else {
                continue;
            };
            if let (Some(a), Some(b)) = (to_screen([a.x, a.y, a.z]), to_screen([b.x, b.y, b.z])) {
                let (colour, _) = style(line.role);
                crate::viewport_overlays::arrowhead(&painter, a, b, to_color32(colour));
            }
        }
        for label in &self.view.labels {
            let Some(at) = to_screen([label.at.x, label.at.y, label.at.z]) else {
                continue;
            };
            let (colour, _) = style(label.role);
            painter.text(
                at + egui::vec2(6.0, -4.0),
                egui::Align2::LEFT_BOTTOM,
                &label.text,
                egui::FontId::monospace(12.0),
                to_color32(colour),
            );
        }
    }
}

/// The view's dark gradient, `#2c3139` at the top to `#1b1e23` (14.2), in
/// both themes.
fn paint_gradient(painter: &egui::Painter, rect: Rect) {
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(rect.left_top(), roles::VIEW_TOP);
    mesh.colored_vertex(rect.right_top(), roles::VIEW_TOP);
    mesh.colored_vertex(rect.right_bottom(), roles::VIEW_BOTTOM);
    mesh.colored_vertex(rect.left_bottom(), roles::VIEW_BOTTOM);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter.add(egui::Shape::mesh(mesh));
}

/// The distance along the ray from `o` in direction `d` to the nearest
/// triangle of `mesh` it meets, or `None` (Möller and Trumbore).
fn ray_mesh(o: [f64; 3], d: [f64; 3], mesh: &TriangleMesh) -> Option<f64> {
    let sub = |a: [f64; 3], b: [f64; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let cross = |a: [f64; 3], b: [f64; 3]| {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let point = |i: u32| mesh.vertices.get(i as usize).map(|p| [p.x, p.y, p.z]);
    let mut best: Option<f64> = None;
    for &[a, b, c] in &mesh.triangles {
        let (Some(p0), Some(p1), Some(p2)) = (point(a), point(b), point(c)) else {
            continue;
        };
        let (e1, e2) = (sub(p1, p0), sub(p2, p0));
        let p = cross(d, e2);
        let det = dot(e1, p);
        if det.abs() < 1e-12 {
            continue;
        }
        let s = sub(o, p0);
        let u = dot(s, p) / det;
        if !(0.0..=1.0).contains(&u) {
            continue;
        }
        let q = cross(s, e1);
        let v = dot(d, q) / det;
        if v < 0.0 || u + v > 1.0 {
            continue;
        }
        let t = dot(e2, q) / det;
        if t > 0.0 && best.is_none_or(|b| t < b) {
            best = Some(t);
        }
    }
    best
}

#[cfg(test)]
pub(crate) mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::float_cmp)]

    use openchuck_app::scene::{SceneLabel, ScenePolyline};
    use openchuck_app::setup::Point3;

    use super::*;
    use crate::roles::{HELD, HELD_LINE, TAB, TAB_LINE, WORK_X};

    /// A workholding of a held region at the start and a cylinder tab on the
    /// axis, on 26 mm stock round a 16 mm part.
    pub(crate) fn held_and_tab() -> SceneView {
        use openchuck_app::setup::{
            Aabb, HeldRegion, Millimetres, PartExtent, Stock, Tab, TabLayout, TabShape, Workholding,
        };
        let stock = Stock::Cylinder {
            diameter: Millimetres(26.0),
            length: Millimetres(32.0),
            x_start: Millimetres(-16.0),
        };
        let w = Workholding {
            start: Some(HeldRegion {
                length: Millimetres(6.0),
                diameter: None,
            }),
            end: None,
            tabs: vec![Tab {
                auto: None,
                x_start: Millimetres(-10.0),
                length: Millimetres(2.0),
                shape: TabShape::Cylinder {
                    diameter: Millimetres(4.0),
                },
                layout: TabLayout::OnAxis,
            }],
        };
        let part = PartExtent {
            bounds: Aabb {
                min: Point3::new(-8.0, -5.0, -5.0),
                max: Point3::new(8.0, 5.0, 5.0),
            },
            radius: Millimetres(5.0),
        };
        openchuck_app::workholding_view::display_scene(&w, Some(&stock), &part).unwrap()
    }

    #[test]
    fn req_ui_25_req_ui_03_held_regions_and_tabs_differ_by_more_than_colour() {
        let view = held_and_tab();
        let scene = to_scene(&view);
        // The held region is translucent violet with a screen cross-hatch,
        // the tab opaque sand; each has a 1.5-pixel outline, violet for the
        // held region and dark for the tab: a second cue besides the fill
        // colour (7.5).
        assert_eq!(scene.meshes.len(), 2);
        let (held, tab) = (&scene.meshes[0], &scene.meshes[1]);
        assert!(held.translucent && !tab.translucent);
        assert_eq!((held.colour, tab.colour), (HELD, TAB));
        assert_eq!(
            held.hatch,
            Some(Hatch {
                colour: HELD_LINE,
                cross: true
            })
        );
        assert_eq!(tab.hatch, None);
        let outline = |m: &openchuck_viewport::MeshItem| m.outline.unwrap();
        assert_eq!(outline(held).colour, HELD_LINE);
        assert_eq!(outline(tab).colour, TAB_LINE);
        assert!(scene.meshes.iter().all(|m| outline(m).width == 1.5
            && outline(m).style == LineStyle::Solid
            && !m.outline_only
            && m.slot == Slot::Part));
        // Their end circles are drawn by the outline, not as lines.
        assert!(scene.lines.is_empty());
        assert_eq!(
            crate::viewport_overlays::workholding_entries(&view)
                .iter()
                .map(|e| e.name())
                .collect::<Vec<_>>(),
            ["Held region", "Tab"]
        );
    }

    #[test]
    fn req_ui_25_held_regions_and_tabs_are_outlines_while_picking_and_on_simulate() {
        let view = held_and_tab();
        let scene = to_scene_with(
            &view,
            &SceneOptions {
                outlines_only: true,
                show: Visibility::default(),
                ..SceneOptions::default()
            },
        );
        // Outlines only, in the role colours (sand for a tab), no fill.
        assert!(scene.meshes.iter().all(|m| m.outline_only));
        assert_eq!(scene.meshes[0].outline.unwrap().colour, HELD_LINE);
        assert_eq!(scene.meshes[1].outline.unwrap().colour, TAB);
        // Hidden by "Show or hide", they are not drawn at all.
        let hidden = to_scene_with(
            &view,
            &SceneOptions {
                show: Visibility {
                    workholding: false,
                    ..Visibility::default()
                },
                ..SceneOptions::default()
            },
        );
        assert!(hidden.meshes.is_empty());
        // The viewport draws them as outlines while picking.
        let mut viewport = Viewport::new(true);
        viewport.set_scene(view, true);
        assert!(viewport.scene().meshes.iter().all(|m| !m.outline_only));
        viewport.set_picking(true);
        assert!(viewport.scene().meshes.iter().all(|m| m.outline_only));
    }

    #[test]
    fn req_ui_25_a_draft_is_drawn_instead_with_failing_and_offered_tabs_marked() {
        let view = held_and_tab();
        // The draft: the same solids, with the tab failing a check, and one
        // default tab offered.
        let draft = WorkholdingDraft {
            meshes: view.meshes.clone(),
            solids: view.solids.clone(),
            failing: vec![WorkholdingItem::Tab(0)],
            offered: vec![view.meshes[1].clone()],
            offered_solids: vec![view.solids[1].clone()],
            unapplied: false,
        };
        let mut viewport = Viewport::new(true);
        viewport.set_scene(view, true);
        viewport.set_workholding_draft(Some(draft));
        assert!(viewport.shows_draft());
        let meshes = &viewport.scene().meshes;
        // The stored solids are not drawn; the draft's are, then the offered
        // tab.
        assert_eq!(meshes.len(), 3);
        assert!(!meshes[0].outline_only);
        assert_eq!(meshes[1].outline.unwrap().colour, roles::ERROR);
        assert!(meshes[1].outline_only);
        let offered = meshes[2].outline.unwrap();
        assert!(meshes[2].outline_only);
        assert_eq!((offered.colour, offered.style), (TAB, LineStyle::Dashed));
        viewport.set_workholding_draft(None);
        assert_eq!(viewport.scene().meshes.len(), 2);
    }

    #[test]
    fn req_ui_03_req_nfr_15_roles_have_the_specified_colours_and_line_styles() {
        let line = |role| ScenePolyline {
            role,
            points: vec![Point3::new(0.0, 0.0, 0.0), Point3::new(1.0, 0.0, 0.0)],
        };
        let view = SceneView {
            lines: vec![
                line(SceneRole::Stock),
                line(SceneRole::RotaryAxis),
                line(SceneRole::ADirection),
                line(SceneRole::WorkX),
                line(SceneRole::Hole),
                line(SceneRole::Problem),
            ],
            ..SceneView::default()
        };
        let scene = to_scene(&view);
        let looks: Vec<(Colour, LineStyle)> =
            scene.lines.iter().map(|l| (l.colour, l.style)).collect();
        assert_eq!(
            looks,
            [
                (roles::STOCK, LineStyle::Dashed),
                (roles::ROTARY_AXIS, LineStyle::DashDot),
                (roles::ROTARY_AXIS, LineStyle::Solid),
                (WORK_X, LineStyle::Solid),
                (roles::HOLE, LineStyle::Solid),
                (roles::FINDING, LineStyle::Solid),
            ]
        );
        // Holes are ringed wider than the 1-pixel lines; the work axes are
        // arrows of 2 pixels.
        assert_eq!(scene.lines[4].width, roles::HOVERED_WIDTH);
        assert_eq!(scene.lines[3].width, roles::AXIS_WIDTH);
        // Findings and the guides can be hidden.
        let hidden = to_scene_with(
            &view,
            &SceneOptions {
                show: Visibility {
                    findings: false,
                    axes: false,
                    stock: false,
                    ..Visibility::default()
                },
                ..SceneOptions::default()
            },
        );
        assert!(hidden.lines.is_empty());
    }

    #[test]
    fn req_ui_03_guides_are_drawn_over_the_part_and_stock_is_not() {
        let line = |role| ScenePolyline {
            role,
            points: vec![Point3::new(0.0, 0.0, 0.0), Point3::new(1.0, 0.0, 0.0)],
        };
        let view = SceneView {
            lines: vec![
                line(SceneRole::Stock),
                line(SceneRole::RotaryAxis),
                line(SceneRole::ADirection),
                line(SceneRole::WorkX),
            ],
            labels: vec![SceneLabel {
                role: SceneRole::ADirection,
                at: Point3::new(0.0, 0.0, 0.0),
                text: "A+".to_owned(),
            }],
            ..SceneView::default()
        };
        let scene = to_scene(&view);
        let on_top: Vec<bool> = scene.lines.iter().map(|l| l.on_top).collect();
        assert_eq!(on_top, [false, true, true, true]);
        assert_eq!(scene.lines[0].segments.len(), 2);
        // The work axes keep their conventional colours: X red.
        assert_eq!(scene.lines[3].colour, WORK_X);
    }

    #[test]
    fn req_imp_04_flagged_triangles_are_outlined_on_top_in_one_line_set() {
        use fixtures::mesh;
        use openchuck_app::scene::SceneMesh;
        let view = SceneView {
            meshes: vec![SceneMesh {
                brep: None,
                role: SceneRole::Problem,
                model: None,
                mesh: mesh(),
                feature_edges: Vec::new(),
            }],
            ..SceneView::default()
        };
        let scene = to_scene(&view);
        assert_eq!(scene.meshes.len(), 1);
        assert_eq!(scene.lines.len(), 1);
        assert!(scene.lines[0].on_top);
        // Two triangles, three sides each, two points a side.
        assert_eq!(scene.lines[0].segments.len(), 12);
    }

    #[test]
    fn req_sim_05_the_part_is_drawn_in_one_item_for_each_deviation_band() {
        use openchuck_app::pathview::deviation_bands;
        let view = cube_scene("bands");
        let model = view.meshes[0].model.unwrap();
        // Four triangles overcut, the other eight left thin.
        let values: Vec<Option<(f64, bool)>> = (0..12)
            .map(|i| Some(if i < 4 { (-0.5, false) } else { (0.2, false) }))
            .collect();
        let bands = deviation_bands(&values, 0.1);
        let plain = to_scene(&view);
        let with_bands = |model| SceneOptions {
            bands: Some((model, &bands)),
            show: Visibility::default(),
            ..SceneOptions::default()
        };
        let coloured = to_scene_with(&view, &with_bands(model));
        // The plain scene has the part once; the coloured one has its edges
        // and then a mesh for each band, in the band's colour, all turning
        // with the workpiece.
        assert_eq!(plain.meshes.len(), 1);
        assert_eq!(coloured.meshes.len(), 1 + bands.bands.len());
        let triangles: usize = coloured.meshes.iter().map(|m| m.indices.len() / 3).sum();
        assert_eq!(triangles, 12);
        for (mesh, (band, set)) in coloured.meshes.iter().skip(1).zip(&bands.bands) {
            assert_eq!(mesh.colour, band_colour(*band));
            assert_eq!(mesh.indices.len() / 3, set.len());
            assert_eq!(mesh.slot, Slot::Part);
        }
        assert!(coloured.meshes[0].indices.is_empty());
        assert!(!coloured.meshes[0].feature_edges.is_empty());
        // The part and what is flagged on it turn with A; the guides do not.
        assert!(plain.meshes.iter().all(|m| m.slot == Slot::Part));
        assert!(
            plain
                .lines
                .iter()
                .all(|l| l.slot == Slot::Part || l.slot == Slot::Fixed)
        );
        // A band for another model changes nothing.
        let other = to_scene_with(&view, &with_bands(ModelId::new()));
        assert_eq!(other.meshes.len(), 1);
    }

    #[test]
    fn req_sim_15_a_hidden_band_keeps_the_parts_colour_and_the_counts_stay() {
        use crate::playback_panel::shown_bands;
        use crate::results_panel::{BandFilter, ColourBy, bin_slot};
        use openchuck_app::pathview::deviation_bands;
        let view = cube_scene("bands-hidden");
        let model = view.meshes[0].model.unwrap();
        let part_colour = view_colour(&view);
        // An analytic case: two triangles overcut 0.7 mm, two overcut 0.3 mm,
        // four on the surface, four with 0.6 mm left.
        let value = |i: usize| match i {
            0..=1 => -0.7,
            2..=3 => -0.3,
            4..=7 => 0.0,
            _ => 0.6,
        };
        let values: Vec<Option<(f64, bool)>> = (0..12).map(|i| Some((value(i), false))).collect();
        let tolerance = 0.1;
        let bands = deviation_bands(&values, tolerance);
        let coloured = |shown: &DeviationBands| {
            let scene = to_scene_with(
                &view,
                &SceneOptions {
                    bands: Some((model, shown)),
                    show: Visibility::default(),
                    ..SceneOptions::default()
                },
            );
            // Triangles drawn in the part's colour, and in any band's.
            let plain: usize = scene
                .meshes
                .iter()
                .filter(|m| m.colour == part_colour)
                .map(|m| m.indices.len() / 3)
                .sum();
            let all: usize = scene.meshes.iter().map(|m| m.indices.len() / 3).sum();
            (plain, all)
        };
        assert_eq!(coloured(&bands), (0, 12));
        // Hide the bin of the overcut from 0.5 to 1 mm: exactly its two
        // triangles keep the part's colour; the part stays whole.
        let mut filter = BandFilter::default();
        filter.hidden[bin_slot(-0.7, tolerance)] = true;
        let shown = shown_bands(&bands, ColourBy::Deviation, &filter);
        assert_eq!(coloured(&shown), (2, 12));
        // A range alone, from -0.5 to 0 mm: the 0.3 mm overcut and the
        // surface (six triangles) are shown; the other six keep the part's
        // colour.
        let range = BandFilter {
            from: Some(-0.5),
            to: Some(0.0),
            ..BandFilter::default()
        };
        let shown = shown_bands(&bands, ColourBy::Deviation, &range);
        assert_eq!(coloured(&shown), (6, 12));
        let kept: usize = shown.bands.iter().map(|(_, t)| t.len()).sum();
        assert_eq!(kept, 6);
        // The values stay whole for the probe, whatever is hidden.
        assert_eq!(shown.values, bands.values);
        // Colour by Remaining hides the overcut the same way.
        let remaining = shown_bands(&bands, ColourBy::Remaining, &BandFilter::default());
        assert_eq!(coloured(&remaining), (4, 12));
    }

    #[test]
    fn req_sim_15_the_colour_scale_says_when_some_bands_are_hidden() {
        use crate::results_panel::{BandFilter, SOME_HIDDEN};
        use openchuck_app::pathview::deviation_bands;
        let ctx = egui::Context::default();
        let mut viewport = Viewport::new(true);
        let view = cube_scene("scale-hidden");
        let model = view.meshes[0].model.unwrap();
        viewport.set_scene(view, true);
        let values: Vec<Option<(f64, bool)>> = (0..12).map(|_| Some((0.2, false))).collect();
        viewport.set_bands(Some((model, deviation_bands(&values, 0.1))));
        let painted = |viewport: &mut Viewport| {
            let mut texts = Vec::new();
            for _ in 0..2 {
                let mut output = ctx.run_ui(egui::RawInput::default(), |ui| viewport.show(ui));
                output.textures_delta.clear();
                texts = output
                    .shapes
                    .iter()
                    .filter_map(|s| match &s.shape {
                        egui::Shape::Text(t) => Some(t.galley.text().to_owned()),
                        _ => None,
                    })
                    .collect();
            }
            texts
        };
        let texts = painted(&mut viewport);
        assert!(texts.iter().any(|t| t == "Not measured"), "{texts:?}");
        assert!(!texts.iter().any(|t| t == SOME_HIDDEN));
        viewport.band_filter = BandFilter {
            to: Some(0.0),
            ..BandFilter::default()
        };
        assert!(painted(&mut viewport).iter().any(|t| t == SOME_HIDDEN));
    }

    /// The colour the scene gives the cube part with no bands.
    fn view_colour(view: &SceneView) -> Colour {
        to_scene(view).meshes[0].colour
    }

    /// A scene of the corpus cube with its picker, built through the
    /// application as the shell does.
    fn cube_scene(name: &str) -> SceneView {
        use openchuck_app::{Application, Threads};
        let dir =
            std::env::temp_dir().join(format!("openchuck-ui-pick-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut app = Application::create(&dir, name, Threads::Exactly(2)).unwrap();
        std::fs::create_dir_all(dir.join("models")).unwrap();
        let cube = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../testdata/stl/cube_ascii.stl");
        std::fs::copy(cube, dir.join("models/cube.stl")).unwrap();
        app.add_model(&dir.join("models/cube.stl"), None).unwrap();
        let view = app.start_scene(None).wait().unwrap();
        let _ = std::fs::remove_dir_all(dir);
        view
    }

    /// Runs one frame with the viewport filling a 400 by 400 screen and the
    /// pointer at `pointer`, with a primary click when `click`.
    fn frame(viewport: &mut Viewport, ctx: &egui::Context, pointer: egui::Pos2, click: bool) {
        // egui hit-tests against the widgets of the previous frame, so a
        // frame with the pointer there and no click comes first.
        run_frame(viewport, ctx, vec![egui::Event::PointerMoved(pointer)]);
        let _ = viewport.take_input();
        let mut events = vec![egui::Event::PointerMoved(pointer)];
        if click {
            events.push(egui::Event::ModifiersChanged(Modifiers::SHIFT));
            for pressed in [true, false] {
                events.push(egui::Event::PointerButton {
                    pos: pointer,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::SHIFT,
                });
            }
        }
        run_frame(viewport, ctx, events);
    }

    fn run_frame(viewport: &mut Viewport, ctx: &egui::Context, events: Vec<egui::Event>) {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                pos2(0.0, 0.0),
                egui::vec2(400.0, 400.0),
            )),
            events,
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| viewport.show(ui));
        output.textures_delta.clear();
    }

    /// In the machine view the part is drawn turned by its slot's transform,
    /// so the pick ray must be carried into the part's frame: the hit is the
    /// triangle under the pointer on screen, with its normal in the part's
    /// own frame (the frame selections use).
    #[test]
    fn req_ui_04_picking_follows_the_part_when_it_is_turned_with_a() {
        let view = cube_scene("turned");
        let ctx = egui::Context::default();
        let mut viewport = Viewport::new(true);
        viewport.set_scene(view, true);
        viewport.set_picking(true);
        viewport.set_view(StandardView::Front);
        // Not turned: the face toward the viewer (looking along +Y) is the
        // one whose normal is -Y.
        frame(&mut viewport, &ctx, pos2(200.0, 200.0), false);
        let plain = viewport.take_input().hover.unwrap().normal.unwrap();
        assert!((plain.y + 1.0).abs() < 1e-9, "{plain:?}");
        // Turned a quarter turn about X, the face toward the viewer is the
        // one that was on top: normal +Z in the part's frame.
        viewport.set_part_turn(90.0);
        frame(&mut viewport, &ctx, pos2(200.0, 200.0), false);
        let turned = viewport
            .take_input()
            .hover
            .expect("the turned cube is under the pointer");
        let n = turned.normal.unwrap();
        assert!((n.z - 1.0).abs() < 1e-9 && n.y.abs() < 1e-9, "{n:?}");
        // Turned back, it is the original face again.
        viewport.set_part_turn(0.0);
        frame(&mut viewport, &ctx, pos2(200.0, 200.0), false);
        let again = viewport.take_input().hover.unwrap().normal.unwrap();
        assert!((again.y + 1.0).abs() < 1e-9);
    }

    #[test]
    fn req_ui_04_the_pointer_picks_the_triangle_it_is_over_and_a_click_reports_the_keys() {
        let view = cube_scene("pointer");
        let ctx = egui::Context::default();
        let mut viewport = Viewport::new(true);
        viewport.set_scene(view, true);
        viewport.set_picking(true);
        // Looking down from +Z, the middle of the view is the cube's top face.
        viewport.set_view(StandardView::Top);
        frame(&mut viewport, &ctx, pos2(200.0, 200.0), false);
        let hover = viewport
            .take_input()
            .hover
            .expect("the cube is under the pointer");
        assert!((hover.normal.unwrap().z - 1.0).abs() < 1e-9, "{hover:?}");
        // The front view looks along +Y at the face with normal -Y.
        viewport.set_view(StandardView::Front);
        frame(&mut viewport, &ctx, pos2(200.0, 200.0), false);
        let front = viewport.take_input().hover.unwrap();
        assert!((front.normal.unwrap().y + 1.0).abs() < 1e-9, "{front:?}");
        // The corner of the window is empty space.
        frame(&mut viewport, &ctx, pos2(2.0, 2.0), false);
        assert_eq!(viewport.take_input().hover, None);
        // A click carries the triangle and the modifiers held.
        frame(&mut viewport, &ctx, pos2(200.0, 200.0), true);
        let input = viewport.take_input();
        let (pick, modifiers) = input.click.expect("a click");
        assert_eq!(Some(pick), input.hover);
        assert!(modifiers.shift);
        // With picking off nothing is reported.
        viewport.set_picking(false);
        frame(&mut viewport, &ctx, pos2(200.0, 200.0), true);
        assert_eq!(viewport.take_input(), PickInput::default());
    }

    /// Runs one frame at `time` seconds with the pointer at `pointer`, with a
    /// primary click when `click`.
    fn frame_at(
        viewport: &mut Viewport,
        ctx: &egui::Context,
        pointer: egui::Pos2,
        click: bool,
        time: f64,
    ) {
        let mut events = vec![egui::Event::PointerMoved(pointer)];
        if click {
            for pressed in [true, false] {
                events.push(egui::Event::PointerButton {
                    pos: pointer,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                });
            }
        }
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                pos2(0.0, 0.0),
                egui::vec2(400.0, 400.0),
            )),
            events,
            time: Some(time),
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| viewport.show(ui));
        output.textures_delta.clear();
    }

    /// Issue 208 (SIM-11 in part, decision record 0162): with the part
    /// coloured by deviation, the value under the pointer is read once the
    /// pointer rests, a click keeps it, and a click off the part hides it.
    #[test]
    fn req_sim_11_the_probe_reads_the_value_under_a_resting_pointer_and_a_click_keeps_it() {
        let view = cube_scene("probe");
        let model = view.meshes[0].model.unwrap();
        let ctx = egui::Context::default();
        let mut viewport = Viewport::new(true);
        viewport.set_scene(view, true);
        viewport.set_view(StandardView::Top);
        let middle = pos2(200.0, 200.0);
        // Inside the view, beside the cube and clear of the overlays.
        let off = pos2(20.0, 200.0);
        // Without the colours there is no probe.
        frame_at(&mut viewport, &ctx, middle, false, 0.0);
        frame_at(&mut viewport, &ctx, middle, false, 1.0);
        assert_eq!(viewport.probe_hover, None);
        let values: Vec<Option<(f64, bool)>> = (0..12).map(|_| Some((-1.02, false))).collect();
        viewport.set_bands(Some((
            model,
            openchuck_app::pathview::deviation_bands(&values, 0.1),
        )));
        // Moving, then resting for less than the pause: nothing yet.
        frame_at(&mut viewport, &ctx, pos2(190.0, 200.0), false, 2.0);
        frame_at(&mut viewport, &ctx, middle, false, 2.1);
        frame_at(&mut viewport, &ctx, middle, false, 2.1 + PROBE_PAUSE / 2.0);
        assert_eq!(viewport.probe_hover, None);
        // Rested: the triangle under the pointer, and its value for the view.
        frame_at(&mut viewport, &ctx, middle, false, 2.2 + PROBE_PAUSE);
        let t = viewport.probe_hover.expect("the cube is under the pointer");
        assert_eq!(viewport.probe_text(t).as_deref(), Some("Overcut 1.020 mm"));
        viewport.colour_by = ColourBy::Remaining;
        assert_eq!(viewport.probe_text(t).as_deref(), Some("No material left"));
        viewport.colour_by = ColourBy::Deviation;
        // Off the part, nothing is read.
        frame_at(&mut viewport, &ctx, off, false, 4.0);
        frame_at(&mut viewport, &ctx, off, false, 5.0);
        assert_eq!(viewport.probe_hover, None);
        // A click on the part keeps its value; a click off it hides it.
        frame_at(&mut viewport, &ctx, middle, false, 6.0);
        frame_at(&mut viewport, &ctx, middle, true, 6.05);
        let (kept, at) = viewport.probe_pinned.expect("the click keeps the value");
        assert_eq!(kept, t);
        assert!((at[2] - viewport.cursor_point.unwrap()[2]).abs() < 1e-9);
        frame_at(&mut viewport, &ctx, off, false, 7.0);
        assert!(viewport.probe_pinned.is_some(), "moving away keeps it");
        frame_at(&mut viewport, &ctx, off, true, 7.05);
        assert_eq!(viewport.probe_pinned, None);
        // A kept value goes with the colours.
        frame_at(&mut viewport, &ctx, middle, true, 8.0);
        assert!(viewport.probe_pinned.is_some());
        viewport.set_bands(None);
        assert_eq!(viewport.probe_pinned, None);
    }

    #[test]
    fn req_sim_11_the_probe_reads_nothing_over_an_overlay() {
        let view = cube_scene("probe overlay");
        let model = view.meshes[0].model.unwrap();
        let ctx = egui::Context::default();
        let mut viewport = Viewport::new(true);
        viewport.set_scene(view, true);
        viewport.set_view(StandardView::Top);
        let values: Vec<Option<(f64, bool)>> = (0..12).map(|_| Some((-1.02, false))).collect();
        viewport.set_bands(Some((
            model,
            openchuck_app::pathview::deviation_bands(&values, 0.1),
        )));
        let middle = pos2(200.0, 200.0);
        frame_at(&mut viewport, &ctx, pos2(190.0, 200.0), false, 0.0);
        frame_at(&mut viewport, &ctx, middle, false, 0.1);
        frame_at(&mut viewport, &ctx, middle, false, 0.2 + PROBE_PAUSE);
        assert!(viewport.probe_hover.is_some());
        // Over the view cube (top right) the part is not under the pointer,
        // so no value is read and the old rest is forgotten.
        let cube = pos2(346.0, crate::viewport_toolbar::HEIGHT + 54.0);
        frame_at(&mut viewport, &ctx, cube, false, 1.0);
        assert_eq!(viewport.probe_hover, None);
        // Back on the part, the value waits for a new pause.
        frame_at(&mut viewport, &ctx, middle, false, 1.2);
        assert_eq!(viewport.probe_hover, None);
        frame_at(&mut viewport, &ctx, middle, false, 1.3 + PROBE_PAUSE);
        assert!(viewport.probe_hover.is_some());
    }

    #[test]
    fn req_sim_11_the_probe_reads_nothing_on_a_hidden_part() {
        let view = cube_scene("probe hidden");
        let model = view.meshes[0].model.unwrap();
        let ctx = egui::Context::default();
        let mut viewport = Viewport::new(true);
        viewport.set_scene(view, true);
        viewport.set_view(StandardView::Top);
        let values: Vec<Option<(f64, bool)>> = (0..12).map(|_| Some((-1.02, false))).collect();
        viewport.set_bands(Some((
            model,
            openchuck_app::pathview::deviation_bands(&values, 0.1),
        )));
        let middle = pos2(200.0, 200.0);
        frame_at(&mut viewport, &ctx, pos2(190.0, 200.0), false, 0.0);
        frame_at(&mut viewport, &ctx, middle, false, 0.1);
        frame_at(&mut viewport, &ctx, middle, false, 0.2 + PROBE_PAUSE);
        assert!(viewport.probe_hover.is_some());
        // Hidden, the part gives no value under the pointer, though it is
        // still where the pointer is.
        viewport.show.part = false;
        frame_at(&mut viewport, &ctx, middle, false, 1.0);
        frame_at(&mut viewport, &ctx, middle, false, 1.1 + PROBE_PAUSE);
        assert_eq!(viewport.probe_hover, None);
        // Shown again, the value waits for a new pause.
        viewport.show.part = true;
        frame_at(&mut viewport, &ctx, middle, false, 2.0);
        assert_eq!(viewport.probe_hover, None);
        frame_at(&mut viewport, &ctx, middle, false, 2.1 + PROBE_PAUSE);
        assert!(viewport.probe_hover.is_some());
    }

    /// The outlines the focus layer holds: colour and width.
    fn focus(viewport: &Viewport) -> Vec<(Colour, f32)> {
        viewport
            .layer(Layer::Focus)
            .unwrap()
            .meshes
            .iter()
            .map(|m| {
                assert!(m.outline_only);
                let o = m.outline.unwrap();
                (o.colour, o.width)
            })
            .collect()
    }

    #[test]
    fn req_ui_25_req_nfr_15_hovering_and_clicking_outline_the_held_region_or_tab() {
        let ctx = egui::Context::default();
        let mut viewport = Viewport::new(true);
        viewport.set_scene(held_and_tab(), true);
        viewport.set_view(StandardView::Front);
        // The held region (X -16 to -10, 26 mm across) fills the middle of
        // the view: pointing at it outlines it 2 pixels wide in yellow and
        // gives its tooltip.
        frame(&mut viewport, &ctx, pos2(200.0, 200.0), false);
        let hovered = viewport
            .hovered_solid()
            .expect("the held region is under the pointer");
        assert_eq!(hovered.item, WorkholdingItem::Start);
        assert!(hovered.description.starts_with("Held region at the start"));
        assert_eq!(focus(&viewport), [(roles::HOVER, roles::HOVERED_WIDTH)]);
        // A click selects it: a 3-pixel accent outline, and the shell is told.
        frame(&mut viewport, &ctx, pos2(200.0, 200.0), true);
        assert_eq!(
            viewport.take_input().solid_click,
            Some(WorkholdingItem::Start)
        );
        assert_eq!(viewport.selected_solid(), Some(WorkholdingItem::Start));
        assert_eq!(focus(&viewport), [(roles::ACCENT, roles::SELECTED_WIDTH)]);
        // Away from it, the selection stays and the hover goes.
        frame(&mut viewport, &ctx, pos2(2.0, 2.0), false);
        assert!(viewport.hovered_solid().is_none());
        assert_eq!(focus(&viewport), [(roles::ACCENT, roles::SELECTED_WIDTH)]);
        // Esc clears it.
        run_frame(
            &mut viewport,
            &ctx,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            }],
        );
        assert_eq!(viewport.selected_solid(), None);
        assert!(focus(&viewport).is_empty());
        // The inspector row with keyboard focus outlines its solid as a hover
        // does (principle 9), and a selection set from the inspector shows.
        viewport.set_focused_solid(Some(WorkholdingItem::Tab(0)));
        viewport.set_selected_solid(Some(WorkholdingItem::Start));
        frame(&mut viewport, &ctx, pos2(2.0, 2.0), false);
        assert_eq!(
            focus(&viewport),
            [
                (roles::ACCENT, roles::SELECTED_WIDTH),
                (roles::HOVER, roles::HOVERED_WIDTH)
            ]
        );
        // While picking they cannot be hovered or clicked (7.5).
        viewport.set_focused_solid(None);
        viewport.set_selected_solid(None);
        viewport.set_picking(true);
        frame(&mut viewport, &ctx, pos2(200.0, 200.0), true);
        assert!(viewport.hovered_solid().is_none());
        assert_eq!(viewport.take_input().solid_click, None);
    }

    #[test]
    fn req_nfr_03_the_pixel_size_is_taken_where_the_scene_is_nearest() {
        // Decision 0167: the level of detail is chosen from the millimetres
        // a pixel spans, so the view must say it. Before the first frame it
        // cannot.
        let mut viewport = Viewport::new(true);
        assert_eq!(viewport.mm_per_pixel(), None);
        viewport.pixels = [1000, 500];
        viewport.camera = Camera {
            distance: 100.0,
            scene_radius: 20.0,
            projection: Projection::Orthographic,
            ..Camera::default()
        };
        let ortho = viewport.mm_per_pixel().unwrap();
        assert!((ortho - viewport.camera.visible_height() / 500.0).abs() < 1e-12);
        // In perspective, pixels are smallest on the scene nearest the eye:
        // 80 mm away instead of 100.
        viewport.camera.projection = Projection::Perspective;
        let near = viewport.mm_per_pixel().unwrap();
        assert!((near - ortho * 0.8).abs() < 1e-12, "{near}");
        // Zoomed out twice as far, a pixel spans more.
        viewport.camera.distance = 200.0;
        assert!(viewport.mm_per_pixel().unwrap() > 2.0 * near);
        // With the eye inside the scene, no size: every point is drawn.
        viewport.camera.distance = 10.0;
        assert_eq!(viewport.mm_per_pixel(), None);
    }

    #[test]
    fn req_ui_03_req_sim_03_the_step_and_show_or_hide_choose_what_is_drawn() {
        let mut viewport = Viewport::new(true);
        // With nothing held the view is taken to be on Setup; toolpaths make
        // it Operations, deviation bands Simulate; the shell can say.
        assert_eq!(viewport.step(), ViewStep::Setup);
        let mut paths = Scene::default();
        paths.add_polyline(
            &[Point3::new(0.0, 0.0, 0.0), Point3::new(1.0, 0.0, 0.0)],
            roles::CUT,
            false,
        );
        viewport.set_layer(Layer::Paths, paths);
        assert_eq!(viewport.step(), ViewStep::Operations);
        viewport.set_step(Some(ViewStep::Export));
        assert_eq!(viewport.step(), ViewStep::Export);
        viewport.set_step(None);
        // Hiding toolpaths or the selections hides their layers; Simulate's
        // "Show: Stock" hides the toolpaths, "Part + path" the stock.
        assert!(viewport.layer_shown(Layer::Paths));
        viewport.set_visibility(Visibility {
            toolpaths: false,
            selections: false,
            ..Visibility::default()
        });
        assert!(!viewport.layer_shown(Layer::Paths));
        assert!(!viewport.layer_shown(Layer::Overlay));
        // Hidden, the path goes to the renderer as it is, with the same
        // generation, so it stays uploaded and showing it again uploads
        // nothing (issue #200).
        let sent = |v: &Viewport| {
            v.frame_layers()
                .into_iter()
                .find(|(l, ..)| *l == Layer::Paths)
                .map(|(_, g, s, shown)| (g, s.lines.len(), shown))
        };
        assert_eq!(sent(&viewport), Some((1, 1, false)));
        viewport.set_visibility(Visibility::default());
        assert_eq!(sent(&viewport), Some((1, 1, true)));
        viewport.set_step(Some(ViewStep::Simulate));
        // Simulate's default is "Cut part" only (issue 204): the stock, not
        // the path or its trace, nor the model.
        assert_eq!(viewport.simulate_show(), SimulateShow::default());
        assert!(!viewport.layer_shown(Layer::Paths) && viewport.layer_shown(Layer::Stock));
        assert!(!viewport.layer_shown(Layer::Trace) && !viewport.part_shown());
        viewport.set_simulate_show(SimulateShow {
            model: true,
            cut_part: false,
            toolpath: true,
        });
        assert!(viewport.layer_shown(Layer::Paths) && !viewport.layer_shown(Layer::Stock));
        assert!(viewport.layer_shown(Layer::Trace) && viewport.part_shown());
        // None ticked is never kept: the view would be empty.
        viewport.set_simulate_show(SimulateShow {
            model: false,
            cut_part: false,
            toolpath: false,
        });
        assert_eq!(viewport.simulate_show(), SimulateShow::default());
        // The choice is Simulate's: on Operations, where it is not offered,
        // the toolpaths, stock and part show (OI-355 wiring, #126 review).
        viewport.set_step(Some(ViewStep::Operations));
        assert!(viewport.layer_shown(Layer::Paths) && viewport.layer_shown(Layer::Stock));
        assert!(viewport.part_shown());
        viewport.set_step(None);
        // F fits and V opens the list, from the keyboard.
        let ctx = egui::Context::default();
        let key = |key| egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        run_frame(&mut viewport, &ctx, vec![key(egui::Key::V)]);
        assert!(viewport.show_menu_open);
    }

    /// Runs one frame of `viewport` on a `size` screen with the pointer at
    /// `pointer`, and returns where its overlays went.
    fn overlays_at(
        viewport: &mut Viewport,
        ctx: &egui::Context,
        size: egui::Vec2,
        pointer: egui::Pos2,
    ) -> (Rect, Vec<Rect>) {
        let input = |events| egui::RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), size)),
            events,
            ..Default::default()
        };
        for _ in 0..2 {
            let mut output = ctx.run_ui(input(vec![egui::Event::PointerMoved(pointer)]), |ui| {
                viewport.show(ui);
            });
            output.textures_delta.clear();
        }
        let mut found = (Rect::NOTHING, Vec::new());
        let mut output = ctx.run_ui(input(Vec::new()), |ui| {
            let full = ui.available_rect_before_wrap();
            let view = full.with_min_y(full.top() + crate::viewport_toolbar::HEIGHT);
            found = (
                view,
                crate::viewport_overlays::boxes(ui, viewport, view).all(),
            );
        });
        output.textures_delta.clear();
        found
    }

    #[test]
    fn req_ui_02_req_ui_16_the_overlays_never_overlap_on_any_step() {
        use openchuck_app::pathview::deviation_bands;
        let ctx = egui::Context::default();
        let mut viewport = Viewport::new(true);
        let mut view = cube_scene("overlays");
        let held = held_and_tab();
        let first = view.meshes.len();
        view.meshes.extend(held.meshes);
        view.solids.extend(held.solids.into_iter().map(|mut s| {
            s.mesh += first;
            s
        }));
        let model = view.meshes[0].model.unwrap();
        viewport.set_scene(view, true);
        let check = |viewport: &mut Viewport, size: egui::Vec2, count: usize| {
            // The pointer over the cube's front face, for the readout.
            let height = size.y - crate::viewport_toolbar::HEIGHT;
            let ndc = viewport
                .camera
                .project([5.0, 0.0, 5.0], f64::from(size.x / height))
                .unwrap();
            let pointer = pos2(
                ((ndc[0] + 1.0) * 0.5) as f32 * size.x,
                crate::viewport_toolbar::HEIGHT + ((1.0 - ndc[1]) * 0.5) as f32 * height,
            );
            let (rect, boxes) = overlays_at(viewport, &ctx, size, pointer);
            assert_eq!(
                boxes.len(),
                count,
                "{:?} at {size:?}: {boxes:?}",
                viewport.step()
            );
            // The cube is top right and the axes gadget bottom left, on
            // every step (decision 0149).
            assert!(boxes[0].right() > rect.center().x && boxes[0].top() < rect.center().y);
            assert!((boxes[1].left() - rect.left()).abs() < 20.0);
            assert!((rect.bottom() - boxes[1].bottom()).abs() < 20.0);
            for (i, a) in boxes.iter().enumerate() {
                assert!(rect.contains_rect(*a), "{a:?} outside {rect:?}");
                for b in boxes.iter().skip(i + 1) {
                    assert!(!a.intersects(*b), "{a:?} overlaps {b:?}");
                }
            }
        };
        let large = egui::vec2(1100.0, 760.0);
        let small = egui::vec2(900.0, 480.0);
        // Setup: the cube and the triad; the held region and tab legend is
        // the shell's legend strip under the view.
        check(&mut viewport, large, 2);
        // Operations: the path legend and the readout of the point under the
        // pointer join them; in a low view the legend is a button, and it
        // opens below it.
        let mut paths = Scene::default();
        paths.add_polyline(
            &[Point3::new(0.0, 0.0, 0.0), Point3::new(1.0, 0.0, 0.0)],
            roles::CUT,
            false,
        );
        viewport.set_layer(Layer::Paths, paths);
        viewport.set_view(StandardView::Front);
        check(&mut viewport, large, 4);
        check(&mut viewport, small, 4);
        viewport.legend_open = true;
        check(&mut viewport, small, 5);
        // Simulate: the colour scale with the kept roles, and the tool at
        // its move in the readout.
        let values: Vec<Option<(f64, bool)>> = (0..12)
            .map(|i| if i == 0 { None } else { Some((0.2, false)) })
            .collect();
        viewport.set_bands(Some((model, deviation_bands(&values, 0.1))));
        viewport.set_tool_readout(Some(ToolReadout {
            work: [1.0, 2.0, 3.0, 4.0],
            move_number: 7,
        }));
        check(&mut viewport, large, 4);
        // A draft shows its chip; the scale stays where it is.
        viewport.set_workholding_draft(Some(WorkholdingDraft::default()));
        check(&mut viewport, large, 4);
        // The toolbar offers Simulate's Show choices, and the scene is drawn.
        assert_eq!(viewport.step(), ViewStep::Simulate);
        assert!(viewport.is_drawn());
    }

    #[test]
    fn req_ui_25_the_ray_meets_the_nearest_triangle_in_front() {
        let mesh = fixtures::mesh();
        // The square lies in z = 0 from (0, 0) to (1, 1).
        let down = ray_mesh([0.5, 0.5, 5.0], [0.0, 0.0, -1.0], &mesh);
        assert!((down.unwrap() - 5.0).abs() < 1e-12);
        assert_eq!(ray_mesh([0.5, 0.5, 5.0], [0.0, 0.0, 1.0], &mesh), None);
        assert_eq!(ray_mesh([2.0, 0.5, 5.0], [0.0, 0.0, -1.0], &mesh), None);
        // Parallel to the square, nothing.
        assert_eq!(ray_mesh([0.5, 0.5, 0.0], [1.0, 0.0, 0.0], &mesh), None);
    }

    mod fixtures {
        use openchuck_app::setup::Point3;

        /// Two triangles of a square, through the scene's own mesh type.
        pub fn mesh() -> openchuck_app::scene::Mesh {
            openchuck_app::scene::Mesh {
                vertices: vec![
                    Point3::new(0.0, 0.0, 0.0),
                    Point3::new(1.0, 0.0, 0.0),
                    Point3::new(1.0, 1.0, 0.0),
                    Point3::new(0.0, 1.0, 0.0),
                ],
                triangles: vec![[0, 1, 2], [0, 2, 3]],
            }
        }
    }
}
