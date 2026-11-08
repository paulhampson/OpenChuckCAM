// SPDX-License-Identifier: AGPL-3.0-or-later
//! The surfaces simultaneous finishing could not reach, in the view (OP-20,
//! UI-11, UI-12, NFR-15; decision record 0515; issue #416): a two-tone dot
//! pattern on the triangles of each area, a numbered marker for each, the
//! legend's rows, and the move of the camera to look at one. What is shown,
//! and when, is decided by [`crate::unreached_view`]; this draws it.

use eframe::egui::{self, Color32, Rect, pos2};
use openchuck_app::project::id::{ModelId, OperationId};
use openchuck_app::scene::{SceneRole, triangles_mesh};
use openchuck_app::selection::TriangleSet;
use openchuck_app::unreached::AreaMarks;
use openchuck_viewport::camera::{FIELD_OF_VIEW_DEGREES, transform};
use openchuck_viewport::{Camera, Hatch, HatchKind, Layer, Scene, Slot};

use super::Viewport;
use crate::roles;

/// How long the camera takes to turn to an area, seconds (decision 0515
/// item 5).
pub const ORBIT_SECONDS: f64 = 0.3;
/// How long a marker pulses after "Show in view", seconds.
pub const PULSE_SECONDS: f64 = 1.2;
/// The radius of a marker's badge, points.
const BADGE_RADIUS: f32 = 9.0;
/// The margin round the X range of an area when it is framed.
const FRAME_MARGIN: f64 = 1.25;
/// The least width framed, millimetres.
const FRAME_MIN_WIDTH: f64 = 4.0;
/// How many camera steps can be undone.
const HISTORY: usize = 32;

/// One area to draw.
#[derive(Debug, Clone, PartialEq)]
pub struct ShownArea {
    /// Its number, from 1: the warning's order.
    pub number: usize,
    /// Where its number is drawn, in the part's frame.
    pub marker: [f64; 3],
    /// The part triangles it lies on.
    pub triangles: TriangleSet,
}

/// The areas of one operation to draw.
#[derive(Debug, Clone, PartialEq)]
pub struct ShownOperation {
    /// The operation.
    pub operation: OperationId,
    /// Its name, for the legend's row.
    pub name: String,
    /// Its areas.
    pub areas: Vec<ShownArea>,
}

/// A marker pulsing after "Show in view".
#[derive(Debug, Clone, Copy, PartialEq)]
struct Pulse {
    operation: OperationId,
    number: usize,
    started: f64,
}

/// A move of the camera under way.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Move {
    from: Camera,
    to: Camera,
    started: f64,
}

/// What the view keeps for the areas (decision 0515).
#[derive(Debug, Default)]
pub(super) struct AreaState {
    model: Option<ModelId>,
    shown: Vec<ShownOperation>,
    pulse: Option<Pulse>,
    moving: Option<Move>,
    /// The cameras before each "Fit", standard view and "Show in view", the
    /// latest last.
    history: Vec<Camera>,
    /// Whether the turn to an area is animated. No setting for reduced
    /// motion exists yet, so it is instant (decision 0515 item 5).
    animate: bool,
}

/// The camera `t` of the way from `from` to `to` (0 to 1), eased: the yaw
/// by the shorter way round, the distance by its logarithm so a zoom has an
/// even pace.
pub(crate) fn blend(from: &Camera, to: &Camera, t: f64) -> Camera {
    let t = t.clamp(0.0, 1.0);
    let eased = t * t * (3.0 - 2.0 * t);
    let lerp = |a: f64, b: f64| a + (b - a) * eased;
    let turn = (to.yaw - from.yaw + 180.0).rem_euclid(360.0) - 180.0;
    Camera {
        target: [
            lerp(from.target[0], to.target[0]),
            lerp(from.target[1], to.target[1]),
            lerp(from.target[2], to.target[2]),
        ],
        distance: (lerp(from.distance.ln(), to.distance.ln())).exp(),
        yaw: (from.yaw + turn * eased).rem_euclid(360.0),
        pitch: lerp(from.pitch, to.pitch),
        projection: to.projection,
        scene_radius: lerp(from.scene_radius, to.scene_radius),
    }
}

impl Viewport {
    /// Sets the areas to draw: the triangles of the part of `model` that
    /// each lies on, with its marker. An empty list draws nothing. The dots
    /// are in the layer of their own; the Show menu's "Unreachable areas"
    /// hides them without dropping them.
    pub fn set_unreached(&mut self, model: Option<ModelId>, shown: Vec<ShownOperation>) {
        if self.areas.model == model && self.areas.shown == shown {
            return;
        }
        self.areas.model = model;
        self.areas.shown = shown;
        self.rebuild_areas();
    }

    /// Builds the layer of dots from the areas and the part the scene holds.
    pub(super) fn rebuild_areas(&mut self) {
        let mut scene = Scene::default();
        let part = self.areas.model.and_then(|model| {
            self.view
                .meshes
                .iter()
                .find(|m| m.role == SceneRole::Part && m.model == Some(model))
        });
        if let Some(part) = part {
            for area in self.areas.shown.iter().flat_map(|o| &o.areas) {
                if area.triangles.is_empty() {
                    continue;
                }
                let mesh = triangles_mesh(&part.mesh, &area.triangles);
                // Review build only (decision 0515 amendment, issue 637):
                // OPENCHUCK_AREA_PATTERN = dots | a | b | c.
                let pattern = std::env::var("OPENCHUCK_AREA_PATTERN").unwrap_or_default();
                let tint = [0.047, 0.055, 0.071, 0.40];
                let (fill, hatch, outline) = match pattern.as_str() {
                    "a" => (
                        [0.0; 4],
                        Some(HatchKind::Surface {
                            ring: roles::AREA_DOT_RING,
                        }),
                        false,
                    ),
                    "b" => ([tint[0], tint[1], tint[2], tint[3]], None, true),
                    "c" => (
                        [0.0; 4],
                        Some(HatchKind::Surface {
                            ring: roles::AREA_DOT_RING,
                        }),
                        true,
                    ),
                    _ => (
                        [0.0; 4],
                        Some(HatchKind::Dots {
                            ring: roles::AREA_DOT_RING,
                        }),
                        false,
                    ),
                };
                scene.add_mesh(&mesh, &[], fill, roles::FEATURE_EDGES);
                if let Some(added) = scene.meshes.last_mut() {
                    added.slot = Slot::Part;
                                        added.hatch = hatch.map(|kind| Hatch {
                        colour: roles::AREA_DOT_CORE,
                        kind,
                    });
                    if outline {
                        added.outline = Some(openchuck_viewport::Outline {
                            colour: roles::AREA_DOT_RING,
                            width: 1.5,
                            style: openchuck_viewport::LineStyle::Solid,
                        });
                    }
                }
            }
        }
        self.set_layer(Layer::Areas, scene);
    }

    /// The legend's rows for the areas drawn: "Unreachable (operation name)"
    /// for each operation with one, while the Show menu has them ticked.
    pub(crate) fn unreached_legend(&self) -> Vec<String> {
        if !self.visibility().unreachable {
            return Vec::new();
        }
        self.areas
            .shown
            .iter()
            .filter(|o| !o.areas.is_empty())
            .map(|o| format!("Unreachable ({})", o.name))
            .collect()
    }

    /// Whether the camera has a step to go back to.
    pub fn can_undo_camera(&self) -> bool {
        !self.areas.history.is_empty()
    }

    /// Goes back to the camera before the latest Fit, standard view or
    /// "Show in view", and stops a move under way. Returns whether there was
    /// one.
    pub fn undo_camera(&mut self) -> bool {
        self.areas.moving = None;
        match self.areas.history.pop() {
            Some(camera) => {
                self.camera = camera;
                true
            }
            None => false,
        }
    }

    /// Keeps the camera as it is, to go back to (one step).
    pub(super) fn remember_camera(&mut self) {
        if self.areas.history.last() == Some(&self.camera) {
            return;
        }
        if self.areas.history.len() == HISTORY {
            self.areas.history.remove(0);
        }
        self.areas.history.push(self.camera);
    }

    /// Whether the turn to an area is animated (about 0.3 s) rather than
    /// instant. The application has no reduced-motion setting yet, so the
    /// shell leaves it off.
    #[cfg(test)]
    pub fn set_animated_orbit(&mut self, animated: bool) {
        self.areas.animate = animated;
        if !animated {
            self.areas.moving = None;
        }
    }

    /// Turns the camera to look straight at `area` of `operation` from the
    /// side it faces, frames its X range with a margin and pulses its marker
    /// (decision 0515 item 5). The part keeps its turn and the playback its
    /// place: only the camera moves, and the area's side is taken where the
    /// part is drawn now, so a part turned by A in the machine view is looked
    /// at from the side the area faces there. One camera step, undone by
    /// [`Viewport::undo_camera`]. `now` is the time in seconds.
    pub fn look_at_area(&mut self, operation: OperationId, area: &AreaMarks, now: f64) {
        let part_transform = self.part_transform;
        let turn = |p: [f64; 3], w: f64| {
            let v = transform(&part_transform, [p[0], p[1], p[2], w]);
            [v[0], v[1], v[2]]
        };
        let d = turn(area.frame.direction, 0.0);
        let (x0, x1) = area.frame.x;
        // The middle of its X range, at the marker's place round the axis.
        let target = turn([0.5 * (x0 + x1), area.marker.y, area.marker.z], 1.0);
        let pitch = d[2].clamp(-1.0, 1.0).asin();
        let yaw = if pitch.cos() < 1e-6 {
            // Straight along Z: the top and bottom views, X to the right.
            -90.0
        } else {
            d[1].atan2(d[0]).to_degrees().rem_euclid(360.0)
        };
        let width = ((x1 - x0).abs() * FRAME_MARGIN).max(FRAME_MIN_WIDTH);
        let half = (0.5 * FIELD_OF_VIEW_DEGREES).to_radians().tan();
        let aspect = if self.aspect.is_finite() && self.aspect > 0.0 {
            self.aspect
        } else {
            1.0
        };
        // The width fits across the view, and the height is at least half
        // of it, so a narrow area is not framed as a sliver.
        let distance = (width / (2.0 * half * aspect)).max(0.25 * width / half);
        let to = Camera {
            target,
            distance: distance.max(1.0),
            yaw,
            pitch: pitch.to_degrees(),
            ..self.camera
        };
        self.remember_camera();
        // The camera is where the user asked for it, not at a fit a new
        // scene is waiting to make.
        self.fit_pending = false;
        if self.areas.animate {
            self.areas.moving = Some(Move {
                from: self.camera,
                to,
                started: now,
            });
        } else {
            self.areas.moving = None;
            self.camera = to;
        }
        self.areas.pulse = Some(Pulse {
            operation,
            number: area.number,
            started: now,
        });
    }

    /// Moves the camera along the turn under way. Returns whether a move or
    /// a pulse still needs frames.
    pub(super) fn advance_motion(&mut self, now: f64) -> bool {
        if let Some(m) = self.areas.moving {
            let t = (now - m.started) / ORBIT_SECONDS;
            if t >= 1.0 {
                self.camera = m.to;
                self.areas.moving = None;
            } else {
                self.camera = blend(&m.from, &m.to, t);
            }
        }
        if let Some(p) = self.areas.pulse
            && now - p.started >= PULSE_SECONDS
        {
            self.areas.pulse = None;
        }
        self.areas.moving.is_some() || self.areas.pulse.is_some()
    }

    /// Stops a move under way: the user took the camera.
    pub(super) fn stop_motion(&mut self) {
        self.areas.moving = None;
    }

    /// The marker pulsing, if any: which area, and how far through the
    /// pulse, 0 to 1.
    #[cfg(test)]
    pub(crate) fn pulsing(&self, now: f64) -> Option<(OperationId, usize, f64)> {
        self.areas
            .pulse
            .map(|p| (p.operation, p.number, (now - p.started) / PULSE_SECONDS))
    }

    /// Draws the numbered markers of the areas (NFR-15: text of 13 points on
    /// a dark badge with a white ring, the dot's own two tones).
    pub(super) fn draw_area_markers(&self, ui: &egui::Ui, rect: Rect, now: f64) {
        if !self.visibility().unreachable || self.areas.shown.is_empty() {
            return;
        }
        let painter = ui.painter_at(rect);
        let core = roles::to_color32(roles::AREA_MARKER_FILL);
        let ring = roles::to_color32(roles::AREA_MARKER_TEXT);
        let font = egui::FontId::proportional(13.0);
        for operation in &self.areas.shown {
            for area in &operation.areas {
                let Some(at) = self.marker_position(area.marker, rect) else {
                    continue;
                };
                if let Some(p) = self
                    .areas
                    .pulse
                    .filter(|p| p.operation == operation.operation && p.number == area.number)
                {
                    // A ring that opens and fades; held at one size, fading,
                    // when the camera does not animate.
                    let t = ((now - p.started) / PULSE_SECONDS).clamp(0.0, 1.0) as f32;
                    let grown = if self.areas.animate { t } else { 0.3 };
                    let radius = BADGE_RADIUS + 4.0 + 16.0 * grown;
                    let alpha = ((1.0 - t) * 255.0) as u8;
                    painter.circle_stroke(
                        at,
                        radius,
                        egui::Stroke::new(
                            3.0,
                            Color32::from_rgba_unmultiplied(ring.r(), ring.g(), ring.b(), alpha),
                        ),
                    );
                }
                painter.circle(at, BADGE_RADIUS, core, egui::Stroke::new(2.0, ring));
                painter.text(
                    at,
                    egui::Align2::CENTER_CENTER,
                    area.number.to_string(),
                    font.clone(),
                    ring,
                );
            }
        }
    }

    /// Where a marker at `p` (in the part's frame) is on the screen, or
    /// `None` when it is behind the eye, outside the view or hidden behind
    /// the part.
    fn marker_position(&self, p: [f64; 3], rect: Rect) -> Option<egui::Pos2> {
        let world = transform(&self.part_transform, [p[0], p[1], p[2], 1.0]);
        let ndc = self
            .camera
            .project([world[0], world[1], world[2]], self.aspect)?;
        if !(0.0..=1.0).contains(&ndc[2]) {
            return None;
        }
        let point = pos2(
            rect.left() + ((ndc[0] + 1.0) * 0.5) as f32 * rect.width(),
            rect.top() + ((1.0 - ndc[1]) * 0.5) as f32 * rect.height(),
        );
        if !rect.contains(point) {
            return None;
        }
        // The far side of the part hides a marker, as it hides a pinned
        // value: the part's nearer faces, by a ray from the eye.
        if self.part_shown()
            && let Some(picker) = self.view.picking.picker()
        {
            let (origin, direction) = self.camera.ray([ndc[0], ndc[1]], self.aspect);
            let back = openchuck_viewport::camera::rigid_inverse(&self.part_transform);
            let o = transform(&back, [origin[0], origin[1], origin[2], 1.0]);
            let d = transform(&back, [direction[0], direction[1], direction[2], 0.0]);
            let (o, d) = ([o[0], o[1], o[2]], [d[0], d[1], d[2]]);
            let distance: f64 = p
                .into_iter()
                .zip(o)
                .zip(d)
                .map(|((point, origin), direction)| (point - origin) * direction)
                .sum();
            if picker
                .pick(o, d)
                .ok()
                .flatten()
                .is_some_and(|hit| hit.distance + 0.05 < distance)
            {
                return None;
            }
        }
        Some(point)
    }
}
