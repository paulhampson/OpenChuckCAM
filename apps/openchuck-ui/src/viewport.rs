// SPDX-License-Identifier: AGPL-3.0-or-later
//! The viewport widget: turns the application's scene view model into a
//! renderer scene, handles orbit, pan and zoom, and draws through an
//! `egui_wgpu` paint callback (UI-02, UI-03; decision record 0024).

use std::sync::Arc;

use eframe::egui::{self, Color32, PointerButton, Rect, Sense, pos2};
use eframe::egui_wgpu;
use openchuck_app::scene::{SceneRole, SceneView};
use openchuck_viewport::wgpu;
use openchuck_viewport::{
    Camera, Colour, DisplayMode, Frame, Projection, Renderer, Scene, StandardView,
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
        _ => (STOCK, false),
    }
}

/// The renderer scene for a view model.
pub fn to_scene(view: &SceneView) -> Scene {
    let mut scene = Scene::default();
    for mesh in &view.meshes {
        let (colour, _) = style(mesh.role);
        scene.add_mesh(&mesh.mesh, &mesh.feature_edges, colour, PART_EDGES);
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
}

/// One frame's draw request.
struct Callback {
    frame: Frame,
    /// A new scene to upload, with its generation.
    scene: Option<(u64, Arc<Scene>)>,
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
        }
    }

    /// Shows a new scene. With `fit` the camera is fitted to it.
    pub fn set_scene(&mut self, view: SceneView, fit: bool) {
        self.scene = Arc::new(to_scene(&view));
        self.view = view;
        self.generation += 1;
        self.fit_pending |= fit;
    }

    /// The scene view model shown.
    pub fn view(&self) -> &SceneView {
        &self.view
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

        let frame = Frame {
            camera: self.camera,
            aspect: self.aspect,
            mode: self.mode,
        };
        ui.painter().add(egui_wgpu::Callback::new_paint_callback(
            rect,
            Callback {
                frame,
                scene: Some((self.generation, Arc::clone(&self.scene))),
            },
        ));
        self.draw_labels(ui, rect);
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
}
