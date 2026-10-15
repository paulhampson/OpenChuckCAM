// SPDX-License-Identifier: AGPL-3.0-or-later
//! The viewport widget: turns the application's scene view model into a
//! renderer scene, handles orbit, pan and zoom, and draws through an
//! `egui_wgpu` paint callback (UI-02, UI-03; decision record 0029).

use std::sync::Arc;

use eframe::egui::{self, Color32, Modifiers, PointerButton, Rect, Sense, pos2};
use eframe::egui_wgpu;
use openchuck_app::pick::ScenePick;
use openchuck_app::scene::{SceneRole, SceneView};
use openchuck_viewport::wgpu;
use openchuck_viewport::{
    Camera, Colour, DisplayMode, Frame, LineItem, Projection, Renderer, Scene, StandardView,
};

/// The depth format eframe creates for `depth_buffer: 32`.
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

const PART: Colour = [0.62, 0.68, 0.78, 1.0];
const PART_EDGES: Colour = [0.08, 0.08, 0.1, 1.0];
const STOCK: Colour = [0.45, 0.45, 0.45, 1.0];
const AXIS: Colour = [0.95, 0.6, 0.1, 1.0];
const A_DIRECTION: Colour = [0.85, 0.2, 0.75, 1.0];
const WORK_X: Colour = [0.9, 0.15, 0.15, 1.0];
const WORK_Y: Colour = [0.1, 0.7, 0.2, 1.0];
const WORK_Z: Colour = [0.15, 0.35, 0.95, 1.0];
const PROBLEM: Colour = [0.95, 0.2, 0.2, 1.0];
const BACKGROUND: Color32 = Color32::from_rgb(38, 40, 46);

/// Degrees of orbit per pixel dragged.
const ORBIT_PER_PIXEL: f64 = 0.4;
/// Zoom per scroll unit: one notch of about 50 points zooms by about 8 %.
const ZOOM_PER_SCROLL: f64 = 0.0015;

/// The colour of a scene item, and whether it is a guide drawn over the part.
fn style(role: SceneRole) -> (Colour, bool) {
    match role {
        SceneRole::Part => (PART, false),
        SceneRole::Stock => (STOCK, false),
        SceneRole::RotaryAxis => (AXIS, true),
        SceneRole::ADirection => (A_DIRECTION, true),
        SceneRole::WorkX => (WORK_X, true),
        SceneRole::WorkY => (WORK_Y, true),
        SceneRole::WorkZ => (WORK_Z, true),
        SceneRole::Problem => (PROBLEM, true),
        _ => (STOCK, false),
    }
}

/// The renderer scene for a view model.
pub fn to_scene(view: &SceneView) -> Scene {
    let mut scene = Scene::default();
    for mesh in &view.meshes {
        let (colour, on_top) = style(mesh.role);
        scene.add_mesh(&mesh.mesh, &mesh.feature_edges, colour, PART_EDGES);
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
                ..LineItem::default()
            });
        }
    }
    for line in &view.lines {
        let (colour, on_top) = style(line.role);
        scene.add_polyline(&line.points, colour, on_top);
    }
    scene
}

fn to_color32(c: Colour) -> Color32 {
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color32::from_rgb(byte(c[0]), byte(c[1]), byte(c[2]))
}

/// What the renderer keeps between frames, in egui's callback resources.
struct Resources {
    renderer: Renderer,
    generation: u64,
    overlay_generation: u64,
}

/// One frame's draw request.
struct Callback {
    frame: Frame,
    /// A new scene to upload, with its generation.
    scene: Option<(u64, Arc<Scene>)>,
    /// The overlay and its generation.
    overlay: (u64, Arc<Scene>),
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
            if let Some((generation, scene)) = &self.scene
                && *generation != r.generation
            {
                r.renderer.set_scene(device, scene);
                r.generation = *generation;
            }
            let (generation, overlay) = &self.overlay;
            if *generation != r.overlay_generation {
                r.renderer.set_overlay(device, overlay);
                r.overlay_generation = *generation;
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
        generation: 0,
        overlay_generation: 0,
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
    /// Drawn over the scene: highlights (UI-04).
    overlay: Arc<Scene>,
    overlay_generation: u64,
    /// Whether the pointer picks triangles of the part (slice 10 item 1).
    picking: bool,
    input: PickInput,
}

/// What the pointer did over the part in the last frame (UI-04).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PickInput {
    /// The triangle under the pointer.
    pub hover: Option<ScenePick>,
    /// The triangle clicked, and the keys held.
    pub click: Option<(ScenePick, Modifiers)>,
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
            overlay: Arc::new(Scene::default()),
            overlay_generation: 0,
            picking: false,
            input: PickInput::default(),
        }
    }

    /// Shows a new scene. With `fit` the camera is fitted to it.
    pub fn set_scene(&mut self, view: SceneView, fit: bool) {
        self.scene = Arc::new(to_scene(&view));
        self.view = view;
        self.generation += 1;
        self.fit_pending |= fit;
    }

    /// Replaces the highlights drawn over the scene.
    pub fn set_overlay(&mut self, overlay: Scene) {
        self.overlay = Arc::new(overlay);
        self.overlay_generation += 1;
    }

    /// Lets the pointer pick triangles of the part: hover and click are then
    /// reported by [`take_input`](Self::take_input) and a click no longer
    /// only orbits.
    pub fn set_picking(&mut self, on: bool) {
        self.picking = on;
        if !on {
            self.input = PickInput::default();
        }
    }

    /// What the pointer did in the last frame, once.
    pub fn take_input(&mut self) -> PickInput {
        std::mem::take(&mut self.input)
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

    /// Draws the viewport filling `ui`, and handles its input.
    pub fn show(&mut self, ui: &mut egui::Ui) {
        let rect = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(rect, Sense::click_and_drag());
        ui.painter().rect_filled(rect, 0.0, BACKGROUND);
        if !self.available {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "The 3D view needs the wgpu renderer, which is not available.",
                egui::FontId::proportional(14.0),
                Color32::LIGHT_GRAY,
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

        self.pick(ui, &response, rect);

        let frame = Frame::new(self.camera, self.aspect, self.mode);
        ui.painter().add(egui_wgpu::Callback::new_paint_callback(
            rect,
            Callback {
                frame,
                scene: Some((self.generation, Arc::clone(&self.scene))),
                overlay: (self.overlay_generation, Arc::clone(&self.overlay)),
            },
        ));
        self.drawn = self.generation;
        self.draw_labels(ui, rect);
    }

    /// Hover and click picking (UI-04): the pointer becomes a ray, the scene's
    /// picker finds the triangle, and the shell is told. Not run while the
    /// pointer is dragging the camera.
    fn pick(&mut self, ui: &egui::Ui, response: &egui::Response, rect: Rect) {
        self.input = PickInput::default();
        if !self.picking || response.dragged() {
            return;
        }
        let Some(picker) = self.view.picking.picker() else {
            return;
        };
        let Some(pointer) = response.hover_pos() else {
            return;
        };
        let ndc = [
            f64::from((pointer.x - rect.left()) / rect.width()) * 2.0 - 1.0,
            1.0 - f64::from((pointer.y - rect.top()) / rect.height()) * 2.0,
        ];
        let (origin, direction) = self.camera.ray(ndc, self.aspect);
        let Ok(Some(hit)) = picker.pick(origin, direction) else {
            return;
        };
        self.input.hover = Some(hit);
        ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
        if response.clicked_by(PointerButton::Primary) {
            self.input.click = Some((hit, ui.input(|i| i.modifiers)));
        }
    }

    /// Draws the scene's labels ("A+", the work axes) at their projected points.
    fn draw_labels(&self, ui: &egui::Ui, rect: Rect) {
        let painter = ui.painter_at(rect);
        for label in &self.view.labels {
            let point = [label.at.x, label.at.y, label.at.z];
            let Some(ndc) = self.camera.project(point, self.aspect) else {
                continue;
            };
            if !(0.0..=1.0).contains(&ndc[2]) {
                continue;
            }
            let x = rect.left() + ((ndc[0] + 1.0) * 0.5) as f32 * rect.width();
            let y = rect.top() + ((1.0 - ndc[1]) * 0.5) as f32 * rect.height();
            let (colour, _) = style(label.role);
            painter.text(
                pos2(x + 4.0, y - 4.0),
                egui::Align2::LEFT_BOTTOM,
                &label.text,
                egui::FontId::proportional(14.0),
                to_color32(colour),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::float_cmp)]

    use openchuck_app::scene::{SceneLabel, ScenePolyline};
    use openchuck_app::setup::Point3;

    use super::*;

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
        // The work axes keep their conventional colours.
        assert_eq!(scene.lines[3].colour, WORK_X);
    }

    #[test]
    fn req_imp_04_flagged_triangles_are_outlined_on_top_in_one_line_set() {
        use fixtures::mesh;
        use openchuck_app::scene::SceneMesh;
        let view = SceneView {
            meshes: vec![SceneMesh {
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
