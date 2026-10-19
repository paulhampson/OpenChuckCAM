// SPDX-License-Identifier: AGPL-3.0-or-later
//! The viewport's overlays (interface specification 7.2; UI-02, SIM-01,
//! SIM-05, UI-25): at most five dark boxes in fixed places, never
//! overlapping, dark in both themes. Top left is the selection tools box,
//! which the selection panel draws; under it the path legend (Operations);
//! top right the view cube and under it the axis triad; bottom right the
//! readout (Operations, Simulate); bottom left the colour scale (Simulate).
//! Held regions and tabs join the legend of the step (7.5).

use eframe::egui::{self, Color32, Pos2, Rect, Stroke, pos2, vec2};
use openchuck_app::pathview::DeviationBand;
use openchuck_app::scene::{SceneRole, SceneView};
use openchuck_viewport::{Camera, StandardView};

use crate::roles::{self, to_color32};
use crate::viewport::{ViewStep, Viewport};

/// The gap between an overlay and the view's edge.
const MARGIN: f32 = 12.0;
/// The view cube's box.
const CUBE: f32 = 84.0;
/// The triad's box height.
const TRIAD: f32 = 78.0;
/// Below this height the path legend collapses to a button (7.2).
pub const LEGEND_MIN_HEIGHT: f32 = 480.0;
/// A legend row's height.
const ROW: f32 = 18.0;

/// Where the boxes are this frame.
#[derive(Debug, Default)]
pub(crate) struct Boxes {
    cube: Option<Rect>,
    triad: Option<Rect>,
    legend: Option<Rect>,
    legend_button: Option<Rect>,
    readout: Option<Rect>,
    scale: Option<Rect>,
}

impl Boxes {
    /// Every box shown.
    pub(crate) fn all(&self) -> Vec<Rect> {
        [
            self.cube,
            self.triad,
            self.legend,
            self.legend_button,
            self.readout,
            self.scale,
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

/// What the path legend lists (7.2), with the held region and tab roles
/// present after "Avoid" (7.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegendEntry {
    /// Cut moves.
    Cut,
    /// Link moves.
    Link,
    /// Rapid moves.
    Rapid,
    /// Entry and exit moves.
    Entry,
    /// The working selection.
    Selected,
    /// Avoid surfaces.
    Avoid,
    /// A held region.
    HeldRegion,
    /// A tab.
    Tab,
}

impl LegendEntry {
    /// Its name in the legend.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Cut => "Cut",
            Self::Link => "Link",
            Self::Rapid => "Rapid",
            Self::Entry => "Entry",
            Self::Selected => "Selected",
            Self::Avoid => "Avoid",
            Self::HeldRegion => "Held region",
            Self::Tab => "Tab",
        }
    }
}

/// The held region and tab roles present in `view` (7.5).
pub fn workholding_entries(view: &SceneView) -> Vec<LegendEntry> {
    let has = |role| view.meshes.iter().any(|m| m.role == role);
    let mut out = Vec::new();
    if has(SceneRole::HeldRegion) {
        out.push(LegendEntry::HeldRegion);
    }
    if has(SceneRole::Tab) {
        out.push(LegendEntry::Tab);
    }
    out
}

/// The path legend's entries: the moves, the selection roles, then the held
/// region and tab roles of `view`.
pub fn path_legend(view: &SceneView) -> Vec<LegendEntry> {
    let mut out = vec![
        LegendEntry::Cut,
        LegendEntry::Link,
        LegendEntry::Rapid,
        LegendEntry::Entry,
        LegendEntry::Selected,
        LegendEntry::Avoid,
    ];
    out.extend(workholding_entries(view));
    out
}

/// Whether the path legend collapses to a button: the view is under 480
/// pixels high (7.2).
pub fn legend_collapsed(height: f32) -> bool {
    height < LEGEND_MIN_HEIGHT
}

/// The readout's lines (7.2): X, Y and Z in mm to 3 places and A in degrees
/// to 2, then the source and the frame ("G54 work coordinates").
pub fn readout_lines(work: [f64; 4], source: &str, register: Option<&str>) -> Vec<String> {
    let frame = match register {
        Some(r) => format!("{r} work coordinates"),
        None => "work coordinates".to_owned(),
    };
    vec![
        format!("X {:>10.3} mm", work[0]),
        format!("Y {:>10.3} mm", work[1]),
        format!("Z {:>10.3} mm", work[2]),
        format!("A {:>10.2} deg", work[3]),
        format!("{source}, {frame}"),
    ]
}

/// What the readout shows, if anything: on Simulate the tool at its move,
/// else the point of the surface under the pointer, in work coordinates
/// (the work zero is where the work axes start).
fn readout(viewport: &Viewport) -> Option<([f64; 4], String)> {
    let step = viewport.step();
    if !matches!(step, ViewStep::Operations | ViewStep::Simulate) {
        return None;
    }
    if step == ViewStep::Simulate
        && let Some(tool) = viewport.tool_readout
    {
        return Some((tool.work, format!("Tool at move {}", tool.move_number)));
    }
    let p = viewport.cursor_point?;
    let zero = viewport
        .view()
        .lines
        .iter()
        .find(|l| l.role == SceneRole::WorkX)
        .and_then(|l| l.points.first())
        .map_or([0.0; 3], |z| [z.x, z.y, z.z]);
    Some((
        [
            p[0] - zero[0],
            p[1] - zero[1],
            p[2] - zero[2],
            viewport.part_turn(),
        ],
        "Cursor on surface".to_owned(),
    ))
}

/// The faces of the view cube that face the eye, back to front: the view
/// each gives, its label, its corners on screen about the cube's centre (in
/// units of the cube's half size, y down) and how squarely it faces the eye.
pub fn cube_faces(camera: &Camera) -> Vec<(StandardView, &'static str, [[f32; 2]; 4], f32)> {
    let (right, up, back) = camera.basis();
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let faces: [(StandardView, &str, [f64; 3]); 6] = [
        (StandardView::Front, "FRONT", [0.0, -1.0, 0.0]),
        (StandardView::Back, "BACK", [0.0, 1.0, 0.0]),
        (StandardView::Left, "LEFT", [-1.0, 0.0, 0.0]),
        (StandardView::Right, "RIGHT", [1.0, 0.0, 0.0]),
        (StandardView::Top, "TOP", [0.0, 0.0, 1.0]),
        (StandardView::Bottom, "BOTTOM", [0.0, 0.0, -1.0]),
    ];
    let mut out: Vec<(StandardView, &str, [[f32; 2]; 4], f32)> = faces
        .iter()
        .filter_map(|&(view, label, n)| {
            let facing = dot(n, back);
            if facing <= 1e-3 {
                return None;
            }
            // Two directions across the face, so its corners are n ± u ± v.
            let u = if n[2].abs() > 0.5 {
                [1.0, 0.0, 0.0]
            } else {
                [0.0, 0.0, 1.0]
            };
            let v = [
                n[1] * u[2] - n[2] * u[1],
                n[2] * u[0] - n[0] * u[2],
                n[0] * u[1] - n[1] * u[0],
            ];
            let corner = |a: f64, b: f64| {
                let p = [
                    n[0] + a * u[0] + b * v[0],
                    n[1] + a * u[1] + b * v[1],
                    n[2] + a * u[2] + b * v[2],
                ];
                [dot(p, right) as f32, -dot(p, up) as f32]
            };
            Some((
                view,
                label,
                [
                    corner(-1.0, -1.0),
                    corner(1.0, -1.0),
                    corner(1.0, 1.0),
                    corner(-1.0, 1.0),
                ],
                facing as f32,
            ))
        })
        .collect();
    out.sort_by(|a, b| a.3.total_cmp(&b.3));
    out
}

/// Whether `p` is inside the convex polygon `corners`.
fn inside(p: [f32; 2], corners: &[[f32; 2]; 4]) -> bool {
    let mut side: Option<bool> = None;
    for (a, b) in corners.iter().zip(corners.iter().cycle().skip(1)) {
        let cross = (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0]);
        if cross.abs() < 1e-9 {
            continue;
        }
        let left = cross > 0.0;
        if *side.get_or_insert(left) != left {
            return false;
        }
    }
    true
}

/// The face of the view cube in `cube` under the point `p`, front-most
/// first.
fn cube_face_at(camera: &Camera, cube: Rect, p: Pos2) -> Option<StandardView> {
    let half = cube.width() * 0.25;
    let local = [
        (p.x - cube.center().x) / half,
        (p.y - cube.center().y) / half,
    ];
    cube_faces(camera)
        .iter()
        .rev()
        .find(|(_, _, corners, _)| inside(local, corners))
        .map(|(view, ..)| *view)
}

/// Where the overlays go in the view `rect` this frame (7.2).
pub(crate) fn boxes(ui: &egui::Ui, viewport: &Viewport, rect: Rect) -> Boxes {
    let mut b = Boxes::default();
    let step = viewport.step();
    let right = rect.right() - MARGIN;
    b.cube = Some(Rect::from_min_size(
        pos2(right - CUBE, rect.top() + MARGIN),
        vec2(CUBE, CUBE),
    ));
    b.triad = Some(Rect::from_min_size(
        pos2(right - CUBE, rect.top() + MARGIN + CUBE + 8.0),
        vec2(CUBE, TRIAD),
    ));
    if readout(viewport).is_some() {
        b.readout = Some(Rect::from_min_size(
            pos2(right - 212.0, rect.bottom() - MARGIN - 96.0),
            vec2(212.0, 96.0),
        ));
    }
    if step == ViewStep::Operations {
        // Under the selection tools box while it shows.
        let top = if viewport.is_picking() {
            ui.ctx()
                .memory(|m| m.area_rect(egui::Id::new("selection tools")))
                .map_or(rect.top() + MARGIN, |r| r.bottom() + 8.0)
        } else {
            rect.top() + MARGIN
        };
        let left = rect.left() + MARGIN;
        if legend_collapsed(rect.height()) {
            b.legend_button = Some(Rect::from_min_size(pos2(left, top), vec2(84.0, 26.0)));
            if viewport.legend_open {
                let rows = path_legend(viewport.view()).len() as f32;
                b.legend = Some(Rect::from_min_size(
                    pos2(left, top + 30.0),
                    vec2(124.0, rows * ROW + 12.0),
                ));
            }
        } else {
            let rows = path_legend(viewport.view()).len() as f32;
            b.legend = Some(Rect::from_min_size(
                pos2(left, top),
                vec2(124.0, rows * ROW + 12.0),
            ));
        }
    }
    let held = workholding_entries(viewport.view()).len() as f32;
    if step == ViewStep::Simulate && viewport.bands().is_some() {
        let height = 74.0 + held * ROW;
        b.scale = Some(Rect::from_min_size(
            pos2(rect.left() + MARGIN, rect.bottom() - MARGIN - height),
            vec2(236.0, height),
        ));
    }
    b
}

/// Takes the overlays' clicks: a face of the view cube gives that standard
/// view, and the collapsed legend's button opens it. Returns true when the
/// pointer is over a box, so the view does not pick through it.
pub fn interact(ui: &egui::Ui, viewport: &mut Viewport, rect: Rect) -> bool {
    let b = boxes(ui, viewport, rect);
    let Some(pointer) = ui.input(|i| i.pointer.hover_pos()) else {
        return false;
    };
    let clicked = ui.input(|i| i.pointer.primary_clicked());
    let over = b.all().iter().any(|r| r.contains(pointer));
    if clicked {
        if let Some(cube) = b.cube
            && cube.contains(pointer)
            && let Some(view) = cube_face_at(&viewport.camera, cube, pointer)
        {
            viewport.set_view(view);
        }
        if b.legend_button.is_some_and(|r| r.contains(pointer)) {
            viewport.legend_open = !viewport.legend_open;
        }
    }
    over
}

fn frame(painter: &egui::Painter, r: Rect) {
    painter.rect(
        r,
        4.0,
        roles::OVERLAY_FILL,
        Stroke::new(1.0, roles::OVERLAY_BORDER),
        egui::StrokeKind::Inside,
    );
}

/// Draws the overlays.
pub fn paint(ui: &egui::Ui, viewport: &Viewport, rect: Rect) {
    let painter = ui.painter_at(rect);
    let b = boxes(ui, viewport, rect);
    let font = egui::FontId::proportional(12.0);
    if let Some(cube) = b.cube {
        frame(&painter, cube);
        let half = cube.width() * 0.25;
        for (_, label, corners, facing) in cube_faces(&viewport.camera) {
            let points: Vec<Pos2> = corners
                .iter()
                .map(|c| cube.center() + vec2(c[0], c[1]) * half)
                .collect();
            let shade = (0x2b as f32 + 0x18 as f32 * facing) as u8;
            painter.add(egui::Shape::convex_polygon(
                points.clone(),
                Color32::from_rgb(shade, shade + 4, shade + 9),
                Stroke::new(1.0, Color32::from_rgb(0x7b, 0x84, 0x90)),
            ));
            if facing > 0.35 {
                let centre = points.iter().fold(Pos2::ZERO, |a, p| a + p.to_vec2()) / 4.0;
                painter.text(
                    centre,
                    egui::Align2::CENTER_CENTER,
                    label,
                    egui::FontId::proportional(8.0),
                    roles::OVERLAY_TEXT,
                );
            }
        }
    }
    if let Some(triad) = b.triad {
        frame(&painter, triad);
        let (right, up, _) = viewport.camera.basis();
        let centre = triad.center();
        for (axis, name, colour) in [
            ([1.0, 0.0, 0.0], "X", roles::WORK_X),
            ([0.0, 1.0, 0.0], "Y", roles::WORK_Y),
            ([0.0, 0.0, 1.0], "Z", roles::WORK_Z),
        ] {
            let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
            let tip = centre + vec2(dot(axis, right) as f32, -dot(axis, up) as f32) * 26.0;
            let colour = to_color32(colour);
            painter.line_segment([centre, tip], Stroke::new(2.0, colour));
            arrowhead(&painter, centre, tip, colour);
            let away = (tip - centre).normalized() * 9.0;
            painter.text(
                tip + away,
                egui::Align2::CENTER_CENTER,
                name,
                egui::FontId::monospace(12.0),
                colour,
            );
        }
    }
    if let (Some(r), Some((work, source))) = (b.readout, readout(viewport)) {
        frame(&painter, r);
        let colours = [
            roles::WORK_X,
            roles::WORK_Y,
            roles::WORK_Z,
            roles::ROTARY_AXIS,
        ];
        let register = viewport.view().work_register.as_deref();
        for (i, line) in readout_lines(work, &source, register).iter().enumerate() {
            let at = r.left_top() + vec2(10.0, 8.0 + 16.0 * i as f32);
            let colour = colours
                .get(i)
                .map_or(roles::OVERLAY_MUTED, |c| to_color32(*c));
            if i < 4 {
                // The axis letter in its colour, the value in the text colour.
                painter.text(
                    at,
                    egui::Align2::LEFT_TOP,
                    &line[..1],
                    egui::FontId::monospace(12.0),
                    colour,
                );
                painter.text(
                    at + vec2(14.0, 0.0),
                    egui::Align2::LEFT_TOP,
                    &line[1..],
                    egui::FontId::monospace(12.0),
                    roles::OVERLAY_TEXT,
                );
            } else {
                painter.text(
                    at,
                    egui::Align2::LEFT_TOP,
                    line,
                    egui::FontId::proportional(11.0),
                    colour,
                );
            }
        }
    }
    if let Some(button) = b.legend_button {
        frame(&painter, button);
        painter.line_segment(
            [
                button.left_center() + vec2(8.0, 0.0),
                button.left_center() + vec2(24.0, 0.0),
            ],
            Stroke::new(2.0, to_color32(roles::CUT)),
        );
        painter.text(
            button.left_center() + vec2(30.0, 0.0),
            egui::Align2::LEFT_CENTER,
            "Legend",
            font.clone(),
            roles::OVERLAY_TEXT,
        );
    }
    if let Some(r) = b.legend {
        frame(&painter, r);
        for (i, entry) in path_legend(viewport.view()).into_iter().enumerate() {
            let y = r.top() + 6.0 + ROW * i as f32;
            let swatch = Rect::from_min_size(pos2(r.left() + 8.0, y + 3.0), vec2(18.0, 10.0));
            swatch_for(&painter, swatch, entry, false);
            painter.text(
                pos2(swatch.right() + 8.0, swatch.center().y),
                egui::Align2::LEFT_CENTER,
                entry.name(),
                font.clone(),
                roles::OVERLAY_TEXT,
            );
        }
    }
    if let (Some(r), Some(bands)) = (b.scale, viewport.bands()) {
        frame(&painter, r);
        colour_scale(&painter, r, bands.tolerance);
        for (i, entry) in workholding_entries(viewport.view()).into_iter().enumerate() {
            let y = r.top() + 74.0 + ROW * i as f32 - 4.0;
            let swatch = Rect::from_min_size(pos2(r.left() + 8.0, y + 2.0), vec2(14.0, 12.0));
            swatch_for(&painter, swatch, entry, true);
            let name = match entry {
                LegendEntry::HeldRegion => "Held region (kept)",
                _ => "Tab (kept)",
            };
            painter.text(
                pos2(swatch.right() + 8.0, swatch.center().y),
                egui::Align2::LEFT_CENTER,
                name,
                font.clone(),
                roles::OVERLAY_TEXT,
            );
        }
    }
    if viewport.shows_draft() {
        let text = "Showing unapplied workholding";
        let galley = painter.layout_no_wrap(text.to_owned(), font.clone(), roles::OVERLAY_TEXT);
        let chip = Rect::from_center_size(
            pos2(rect.center().x, rect.top() + MARGIN + 12.0),
            galley.size() + vec2(16.0, 8.0),
        );
        frame(&painter, chip);
        painter.galley(chip.min + vec2(8.0, 4.0), galley, roles::OVERLAY_TEXT);
    }
}

/// The bands of the colour scale in order, with the labels at its ends
/// (11.5; D12: decision 0040's bands in the colours of 11.5).
pub const SCALE: [DeviationBand; 5] = [
    DeviationBand::Overcut,
    DeviationBand::OnTarget,
    DeviationBand::Thin,
    DeviationBand::Medium,
    DeviationBand::Thick,
];

/// Draws the colour scale: a bar of the bands, its labelled ends, the hatched
/// "not measured" swatch and the band of "on the surface".
fn colour_scale(painter: &egui::Painter, r: Rect, tolerance: f64) {
    let bar = Rect::from_min_size(r.left_top() + vec2(8.0, 8.0), vec2(r.width() - 16.0, 10.0));
    let step = bar.width() / SCALE.len() as f32;
    for (i, band) in SCALE.iter().enumerate() {
        let cell = Rect::from_min_size(
            bar.left_top() + vec2(step * i as f32, 0.0),
            vec2(step, bar.height()),
        );
        painter.rect_filled(cell, 0.0, to_color32(roles::band_colour(*band)));
    }
    let font = egui::FontId::proportional(11.0);
    painter.text(
        bar.left_bottom() + vec2(0.0, 4.0),
        egui::Align2::LEFT_TOP,
        "Overcut",
        font.clone(),
        roles::OVERLAY_TEXT,
    );
    painter.text(
        bar.center_bottom() + vec2(-step / 2.0, 4.0),
        egui::Align2::CENTER_TOP,
        "On",
        font.clone(),
        roles::OVERLAY_TEXT,
    );
    painter.text(
        bar.right_bottom() + vec2(0.0, 4.0),
        egui::Align2::RIGHT_TOP,
        "Left > 1 mm",
        font.clone(),
        roles::OVERLAY_TEXT,
    );
    let unmeasured = Rect::from_min_size(r.left_top() + vec2(8.0, 38.0), vec2(14.0, 10.0));
    painter.rect_filled(unmeasured, 0.0, to_color32(roles::WITHIN));
    hatch(
        painter,
        unmeasured,
        to_color32(roles::UNMEASURED_HATCH),
        false,
    );
    painter.text(
        unmeasured.right_center() + vec2(8.0, 0.0),
        egui::Align2::LEFT_CENTER,
        "Not measured",
        font.clone(),
        roles::OVERLAY_TEXT,
    );
    painter.text(
        r.left_top() + vec2(8.0, 52.0),
        egui::Align2::LEFT_TOP,
        format!("On the surface: within {tolerance:.3} mm"),
        font,
        roles::OVERLAY_MUTED,
    );
}

/// Diagonal lines 6 pixels apart across `r`, clipped to it: one direction,
/// or both.
fn hatch(painter: &egui::Painter, r: Rect, colour: Color32, cross: bool) {
    let clipped = painter.with_clip_rect(r);
    let stroke = Stroke::new(1.0, colour);
    let span = r.width() + r.height();
    let mut k = 0.0;
    while k <= span {
        clipped.line_segment(
            [
                r.left_top() + vec2(k, 0.0),
                r.left_top() + vec2(k - span, span),
            ],
            stroke,
        );
        if cross {
            clipped.line_segment(
                [
                    r.right_top() - vec2(k, 0.0),
                    r.right_top() + vec2(span - k, span),
                ],
                stroke,
            );
        }
        k += 6.0;
    }
}

/// A legend swatch drawn the way the role is drawn (7.5): `kept` is the
/// Simulate form, an outline in the role colour.
pub fn swatch_for(painter: &egui::Painter, r: Rect, entry: LegendEntry, kept: bool) {
    let line = |colour: openchuck_viewport::Colour, width: f32, dotted: bool| {
        let y = r.center().y;
        let stroke = Stroke::new(width, to_color32(colour));
        if dotted {
            painter.add(egui::Shape::dashed_line(
                &[pos2(r.left(), y), pos2(r.right(), y)],
                stroke,
                2.0,
                3.0,
            ));
        } else {
            painter.line_segment([pos2(r.left(), y), pos2(r.right(), y)], stroke);
        }
    };
    match entry {
        LegendEntry::Cut => line(roles::CUT, roles::PATH_WIDTH, false),
        LegendEntry::Link => line(roles::LINK, roles::LINE_WIDTH, false),
        LegendEntry::Rapid => line(roles::RAPID, roles::PATH_WIDTH, true),
        LegendEntry::Entry => line(roles::ENTRY, roles::PATH_WIDTH, false),
        LegendEntry::Selected => {
            painter.rect_filled(r, 0.0, to_color32(roles::SELECTED).gamma_multiply(0.4));
        }
        LegendEntry::Avoid => {
            hatch(painter, r, to_color32(roles::AVOID), false);
            painter.rect_stroke(
                r,
                0.0,
                Stroke::new(1.0, to_color32(roles::AVOID)),
                egui::StrokeKind::Inside,
            );
        }
        LegendEntry::HeldRegion => {
            let violet = to_color32(roles::HELD_LINE);
            if !kept {
                painter.rect_filled(r, 0.0, violet.gamma_multiply(0.35));
                hatch(painter, r, violet, true);
            }
            painter.rect_stroke(
                r,
                0.0,
                Stroke::new(roles::OUTLINE_WIDTH, violet),
                egui::StrokeKind::Inside,
            );
        }
        LegendEntry::Tab => {
            let sand = to_color32(roles::TAB);
            if kept {
                painter.rect_stroke(
                    r,
                    0.0,
                    Stroke::new(roles::OUTLINE_WIDTH, sand),
                    egui::StrokeKind::Inside,
                );
            } else {
                painter.rect_filled(r, 0.0, sand);
                painter.rect_stroke(
                    r,
                    0.0,
                    Stroke::new(roles::OUTLINE_WIDTH, to_color32(roles::TAB_LINE)),
                    egui::StrokeKind::Inside,
                );
            }
        }
    }
}

/// An arrowhead at `tip`, pointing away from `from`.
pub fn arrowhead(painter: &egui::Painter, from: Pos2, tip: Pos2, colour: Color32) {
    let along = tip - from;
    if along.length() < 1.0 {
        return;
    }
    let d = along.normalized() * 7.0;
    let side = vec2(-d.y, d.x) * 0.5;
    painter.add(egui::Shape::convex_polygon(
        vec![tip + d * 0.5, tip - d * 0.5 + side, tip - d * 0.5 - side],
        colour,
        Stroke::NONE,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn req_ui_02_the_view_cube_shows_the_faces_toward_the_eye_and_gives_their_view() {
        let mut camera = Camera::default();
        // Isometric, from +X, -Y and +Z: front, right and top face the eye.
        camera.set_view(StandardView::Isometric);
        let faces: Vec<StandardView> = cube_faces(&camera).iter().map(|f| f.0).collect();
        assert_eq!(faces.len(), 3);
        for view in [StandardView::Front, StandardView::Right, StandardView::Top] {
            assert!(faces.contains(&view), "{faces:?}");
        }
        // Looking from the front only the front faces the eye, and the
        // middle of the cube gives the front view.
        camera.set_view(StandardView::Front);
        let faces = cube_faces(&camera);
        assert_eq!(faces.len(), 1);
        let cube = Rect::from_min_size(pos2(0.0, 0.0), vec2(84.0, 84.0));
        assert_eq!(
            cube_face_at(&camera, cube, cube.center()),
            Some(StandardView::Front)
        );
        // From the top, the point above the centre on screen is on the top
        // face; outside the cube is nothing.
        camera.set_view(StandardView::Top);
        assert_eq!(
            cube_face_at(&camera, cube, cube.center()),
            Some(StandardView::Top)
        );
        assert_eq!(cube_face_at(&camera, cube, pos2(1.0, 1.0)), None);
    }

    #[test]
    fn req_ui_02_req_sim_01_the_readout_gives_places_and_units() {
        let lines = readout_lines(
            [142.3184, -11.9, 3.45, -53.2],
            "Cursor on surface",
            Some("G54"),
        );
        assert_eq!(lines[0], "X    142.318 mm");
        assert_eq!(lines[1], "Y    -11.900 mm");
        assert_eq!(lines[3], "A     -53.20 deg");
        assert_eq!(lines[4], "Cursor on surface, G54 work coordinates");
        let lines = readout_lines([0.0; 4], "Tool at move 12", None);
        assert_eq!(lines[4], "Tool at move 12, work coordinates");
    }

    #[test]
    fn req_ui_25_req_sim_01_the_path_legend_lists_moves_then_held_regions_and_tabs() {
        let view = SceneView::default();
        let names: Vec<&str> = path_legend(&view).iter().map(|e| e.name()).collect();
        assert_eq!(
            names,
            ["Cut", "Link", "Rapid", "Entry", "Selected", "Avoid"]
        );
        // Below 480 pixels high it collapses to a button (7.2).
        assert!(legend_collapsed(479.0));
        assert!(!legend_collapsed(480.0));
    }

    #[test]
    fn req_ui_02_inside_tests_a_convex_polygon() {
        let square = [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]];
        assert!(inside([0.0, 0.0], &square));
        assert!(!inside([2.0, 0.0], &square));
    }
}
