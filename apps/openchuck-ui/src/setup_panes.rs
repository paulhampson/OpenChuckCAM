// SPDX-License-Identifier: AGPL-3.0-or-later
//! The Setup step's two orthographic panes (UI-03; interface specification
//! 11.3 and 7.5; slice 22 lane X5, decision record 0129): the front view from
//! −Y and the end view from +X, drawn in 2D over the view model of
//! `openchuck_app::setup_panes`. The interface works out nothing: every
//! length, position and label comes from the view model.

use eframe::egui::{
    self, Align2, Color32, FontFamily, FontId, Painter, Pos2, Rect, Sense, Shape, Stroke, pos2,
    vec2,
};
use openchuck_app::scene::WorkholdingItem;
use openchuck_app::setup_panes::{
    Dimension, EndPane, EndStock, FrontPane, FrontSolid, LegendEntry, SetupPanes,
};
use openchuck_viewport::{Colour, LineStyle};

use crate::roles::{self, to_color32};
use crate::theme;
use crate::viewport_overlays::{LegendEntry as RoleEntry, hatch, swatch_for};

/// The width of the end view (11.3).
pub const END_PANE_WIDTH: f32 = 270.0;
/// Empty space round a pane's drawing, in pixels.
const MARGIN: f32 = 36.0;
/// How tall the travel limit's hatched band is.
const BAND: f32 = 10.0;

/// What is outlined in the panes besides what is hovered (7.5).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Marks {
    /// The solid whose inspector row has keyboard focus: outlined as a hover.
    pub focused: Option<WorkholdingItem>,
    /// The selected solid: a 3-pixel accent outline.
    pub selected: Option<WorkholdingItem>,
}

/// What the pointer did over the panes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Interaction {
    /// The held region or tab under the pointer.
    pub hovered: Option<WorkholdingItem>,
    /// The one clicked.
    pub clicked: Option<WorkholdingItem>,
}

/// A held region or tab as the pointer can hit it: which it is, its outline
/// on screen and its tooltip.
type Hit = (WorkholdingItem, Vec<Pos2>, String);

/// A pane's mapping from millimetres to screen pixels, with the vertical
/// axis pointing up.
#[derive(Debug, Clone, Copy)]
struct Map {
    origin: Pos2,
    scale: f32,
}

impl Map {
    /// The mapping that fits the box `h` by `v` (millimetres, as low and
    /// high) into `rect`, less the margin, centred.
    fn fit(rect: Rect, h: (f64, f64), v: (f64, f64)) -> Self {
        let area = rect.shrink(MARGIN);
        let (w, t) = (
            ((h.1 - h.0) as f32).max(1e-3),
            ((v.1 - v.0) as f32).max(1e-3),
        );
        let scale = (area.width().max(1.0) / w).min(area.height().max(1.0) / t);
        let centre = ((h.0 + h.1) as f32 / 2.0, (v.0 + v.1) as f32 / 2.0);
        Self {
            origin: area.center() - vec2(centre.0 * scale, -centre.1 * scale),
            scale,
        }
    }

    fn at(&self, h: f64, v: f64) -> Pos2 {
        pos2(
            self.origin.x + h as f32 * self.scale,
            self.origin.y - v as f32 * self.scale,
        )
    }

    fn polygon(&self, points: &[[f64; 2]]) -> Vec<Pos2> {
        points.iter().map(|p| self.at(p[0], p[1])).collect()
    }
}

fn font() -> FontId {
    FontId::new(theme::size::TEXT - 1.0, FontFamily::Monospace)
}

/// A line along `points` in a pattern of dashes (a [`LineStyle`]).
fn styled(painter: &Painter, points: &[Pos2], colour: Colour, width: f32, style: LineStyle) {
    let stroke = Stroke::new(width, to_color32(colour));
    let pattern = style.pattern();
    if pattern[0] <= 0.0 {
        painter.add(Shape::line(points.to_vec(), stroke));
        return;
    }
    // On, off, and for dash-dot a dot and its gap.
    let phases = if pattern[2] > 0.0 { 4 } else { 2 };
    let length_of = |k: usize| pattern.get(k).copied().unwrap_or(1.0);
    let (mut k, mut left) = (0, length_of(0));
    for pair in points.windows(2) {
        let [a, b] = pair else { continue };
        let (a, b) = (*a, *b);
        let length = (b - a).length();
        if length < 1e-3 {
            continue;
        }
        let along = (b - a) / length;
        let mut at = 0.0;
        while at < length {
            let take = left.min(length - at);
            if k % 2 == 0 {
                painter.line_segment([a + along * at, a + along * (at + take)], stroke);
            }
            at += take;
            left -= take;
            if left <= 1e-6 {
                k = (k + 1) % phases;
                left = length_of(k);
            }
        }
    }
}

fn closed(mut points: Vec<Pos2>) -> Vec<Pos2> {
    if let Some(first) = points.first().copied() {
        points.push(first);
    }
    points
}

fn circle_points(centre: Pos2, radius: f32) -> Vec<Pos2> {
    (0..=96)
        .map(|i| {
            let t = std::f32::consts::TAU * i as f32 / 96.0;
            centre + vec2(t.cos(), -t.sin()) * radius
        })
        .collect()
}

fn inside(polygon: &[Pos2], p: Pos2) -> bool {
    let previous = polygon.iter().cycle().skip(polygon.len().saturating_sub(1));
    let mut hit = false;
    for (&a, &b) in polygon.iter().zip(previous) {
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            hit = !hit;
        }
    }
    hit
}

/// The outline of a held region or tab: its role's, or red when the draft
/// fails a check on it, with the hovered and selected widths on top (7.5).
fn outline(
    painter: &Painter,
    points: &[Pos2],
    role: Colour,
    failing: bool,
    item: WorkholdingItem,
    marks: Marks,
    hovered: Option<WorkholdingItem>,
) {
    let colour = if failing { roles::ERROR } else { role };
    painter.add(Shape::line(
        closed(points.to_vec()),
        Stroke::new(roles::OUTLINE_WIDTH, to_color32(colour)),
    ));
    if hovered == Some(item) || marks.focused == Some(item) {
        painter.add(Shape::line(
            closed(points.to_vec()),
            Stroke::new(roles::HOVERED_WIDTH, to_color32(roles::HOVER)),
        ));
    }
    if marks.selected == Some(item) {
        painter.add(Shape::line(
            closed(points.to_vec()),
            Stroke::new(roles::SELECTED_WIDTH, to_color32(roles::ACCENT)),
        ));
    }
}

fn label(painter: &Painter, at: Pos2, anchor: Align2, text: &str, colour: Color32) {
    painter.text(at, anchor, text, font(), colour);
}

/// A dimension: a line with ticks at both ends, and its text.
fn dimension(painter: &Painter, map: &Map, d: &Dimension) {
    let (a, b) = (map.at(d.from[0], d.from[1]), map.at(d.to[0], d.to[1]));
    let colour = roles::OVERLAY_TEXT;
    let stroke = Stroke::new(roles::LINE_WIDTH, colour);
    painter.line_segment([a, b], stroke);
    let along = (b - a).normalized();
    let side = vec2(-along.y, along.x) * 4.0;
    for p in [a, b] {
        painter.line_segment([p - side, p + side], stroke);
    }
    let mid = a + (b - a) / 2.0;
    if (a.y - b.y).abs() < 1.0 {
        // Level: the text goes outside the axis.
        let above = d.from[1] > 0.0;
        let (offset, anchor) = if above {
            (vec2(0.0, -4.0), Align2::CENTER_BOTTOM)
        } else {
            (vec2(0.0, 4.0), Align2::CENTER_TOP)
        };
        label(painter, mid + offset, anchor, &d.text, colour);
    } else {
        label(
            painter,
            mid + vec2(6.0, 0.0),
            Align2::LEFT_CENTER,
            &d.text,
            colour,
        );
    }
}

/// An arrow from `from` to `to`, with its head.
fn arrow(painter: &Painter, from: Pos2, to: Pos2, colour: Color32, text: &str) {
    painter.line_segment([from, to], Stroke::new(roles::AXIS_WIDTH, colour));
    crate::viewport_overlays::arrowhead(painter, from, to, colour);
    label(
        painter,
        to + (to - from).normalized() * 8.0,
        Align2::CENTER_CENTER,
        text,
        colour,
    );
}

/// The vertical span and horizontal span the front view fits, in millimetres.
fn front_bounds(f: &FrontPane) -> ((f64, f64), (f64, f64)) {
    let mut h = f.axis;
    let mut v = (0.0_f64, 0.0_f64);
    let boxes = [
        f.stock.map(|r| (r.h0, r.h1, r.v0, r.v1)),
        f.clearance.map(|(r, _)| (r.h0, r.h1, r.v0, r.v1)),
        f.safe_z.map(|z| (h.0, h.1, 0.0, z)),
        f.travel_z.map(|z| (h.0, h.1, 0.0, z)),
    ];
    for (h0, h1, v0, v1) in boxes.into_iter().flatten() {
        h = (h.0.min(h0), h.1.max(h1));
        v = (v.0.min(v0), v.1.max(v1));
    }
    // Room under the clearance for the stock length's dimension.
    v.0 -= 0.18 * (v.1 - v.0);
    (h, v)
}

fn front_solid(
    painter: &Painter,
    map: &Map,
    s: &FrontSolid,
    held: bool,
    marks: Marks,
    hovered: Option<WorkholdingItem>,
) -> Vec<Pos2> {
    let points = map.polygon(&s.outline);
    if !s.failing {
        if held {
            let rect = Rect::from_points(&points);
            let violet = to_color32(roles::HELD_LINE);
            painter.rect_filled(rect, 0.0, violet.gamma_multiply(0.35));
            hatch(painter, rect, violet, true);
        } else {
            painter.add(Shape::convex_polygon(
                points.clone(),
                to_color32(roles::TAB),
                Stroke::NONE,
            ));
        }
    }
    let role = if held {
        roles::HELD_LINE
    } else {
        roles::TAB_LINE
    };
    outline(painter, &points, role, s.failing, s.item, marks, hovered);
    if let Some(text) = &s.label {
        let top = points.iter().map(|p| p.y).fold(f32::INFINITY, f32::min);
        let x = points.iter().map(|p| p.x).sum::<f32>() / points.len().max(1) as f32;
        label(
            painter,
            pos2(x, top - 4.0),
            Align2::CENTER_BOTTOM,
            text,
            roles::OVERLAY_TEXT,
        );
    }
    points
}

/// The front view: the stock, part, rotary axis, work zero,
/// clearance, safe Z, travel limit, dimensions, and the held regions and
/// tabs in section (11.3, 7.5). Returns the solids' screen outlines, for
/// hover.
fn front(
    painter: &Painter,
    rect: Rect,
    f: &FrontPane,
    marks: Marks,
    hovered: Option<WorkholdingItem>,
) -> Vec<Hit> {
    let painter = painter.with_clip_rect(rect);
    let (h, v) = front_bounds(f);
    let map = Map::fit(rect, h, v);
    let (left, right) = (map.at(h.0, 0.0).x, map.at(h.1, 0.0).x);
    if let Some(z) = f.travel_z {
        let y = map.at(0.0, z).y;
        let band = Rect::from_min_max(pos2(left, y - BAND), pos2(right, y));
        let red = to_color32(roles::AVOID);
        hatch(&painter, band, red, false);
        painter.line_segment(
            [band.left_bottom(), band.right_bottom()],
            Stroke::new(roles::LINE_WIDTH, red),
        );
        label(
            &painter,
            pos2(right, y - BAND - 2.0),
            Align2::RIGHT_BOTTOM,
            "Z+ travel limit",
            red,
        );
    }
    if let Some(z) = f.safe_z {
        let y = map.at(0.0, z).y;
        styled(
            &painter,
            &[pos2(left, y), pos2(right, y)],
            roles::CLEARANCE,
            1.0,
            LineStyle::Dashed,
        );
        label(
            &painter,
            pos2(right, y - 2.0),
            Align2::RIGHT_BOTTOM,
            &format!("Safe Z +{z:.0}"),
            to_color32(roles::CLEARANCE),
        );
    }
    if let Some((c, radius)) = f.clearance {
        let (a, b) = (map.at(c.h0, c.v1), map.at(c.h1, c.v0));
        let r = Rect::from_two_pos(a, b);
        painter.rect_filled(r, 0.0, to_color32(roles::CLEARANCE).gamma_multiply(0.06));
        styled(
            &painter,
            &closed(vec![
                r.left_top(),
                r.right_top(),
                r.right_bottom(),
                r.left_bottom(),
            ]),
            roles::CLEARANCE,
            1.0,
            LineStyle::Dashed,
        );
        label(
            &painter,
            r.left_top() + vec2(8.0, 4.0),
            Align2::LEFT_TOP,
            &format!("Clearance R{radius:.0}"),
            to_color32(roles::CLEARANCE),
        );
    }
    if f.part.len() > 1 {
        let mut mesh = egui::Mesh::default();
        let fill = to_color32(roles::PART);
        for pair in f.part.windows(2) {
            let [p, q] = pair else { continue };
            let corners = [
                map.at(p[0], p[2]),
                map.at(q[0], q[2]),
                map.at(q[0], q[1]),
                map.at(p[0], p[1]),
            ];
            let base = mesh.vertices.len() as u32;
            for c in corners {
                mesh.colored_vertex(c, fill);
            }
            mesh.add_triangle(base, base + 1, base + 2);
            mesh.add_triangle(base, base + 2, base + 3);
        }
        painter.add(Shape::mesh(mesh));
        let edge = to_color32(roles::FEATURE_EDGES);
        for upper in [true, false] {
            let line: Vec<Pos2> = f
                .part
                .iter()
                .map(|p| map.at(p[0], if upper { p[2] } else { p[1] }))
                .collect();
            painter.add(Shape::line(line, Stroke::new(roles::LINE_WIDTH, edge)));
        }
    }
    if let Some(s) = f.stock {
        let (a, b) = (map.at(s.h0, s.v1), map.at(s.h1, s.v0));
        let r = Rect::from_two_pos(a, b);
        styled(
            &painter,
            &closed(vec![
                r.left_top(),
                r.right_top(),
                r.right_bottom(),
                r.left_bottom(),
            ]),
            roles::STOCK,
            1.5,
            LineStyle::Dashed,
        );
    }
    styled(
        &painter,
        &[map.at(f.axis.0, 0.0), map.at(f.axis.1, 0.0)],
        roles::ROTARY_AXIS,
        1.0,
        LineStyle::DashDot,
    );
    label(
        &painter,
        map.at(f.axis.1, 0.0) + vec2(-4.0, 4.0),
        Align2::RIGHT_TOP,
        "A axis",
        to_color32(roles::ROTARY_AXIS),
    );

    let mut shapes = Vec::new();
    for (list, held) in [(&f.held, true), (&f.tabs, false)] {
        for s in list {
            let points = front_solid(&painter, &map, s, held, marks, hovered);
            shapes.push((s.item, points, s.description.clone()));
        }
    }

    // The work zero: X and Z arrows, and Y into the screen (a circle with a
    // cross).
    let zero = map.at(f.work_zero, 0.0);
    arrow(
        &painter,
        zero,
        zero + vec2(26.0, 0.0),
        to_color32(roles::WORK_X),
        "X",
    );
    arrow(
        &painter,
        zero,
        zero + vec2(0.0, -26.0),
        to_color32(roles::WORK_Z),
        "Z",
    );
    let green = to_color32(roles::WORK_Y);
    painter.circle_stroke(zero, 6.0, Stroke::new(roles::AXIS_WIDTH, green));
    painter.line_segment(
        [zero + vec2(-4.0, -4.0), zero + vec2(4.0, 4.0)],
        Stroke::new(1.0, green),
    );
    painter.line_segment(
        [zero + vec2(-4.0, 4.0), zero + vec2(4.0, -4.0)],
        Stroke::new(1.0, green),
    );
    for d in &f.dimensions {
        dimension(&painter, &map, d);
    }
    shapes
}

/// The end view from +X (11.3, 7.5). Returns the tabs' screen circles, for
/// hover.
fn end(
    painter: &Painter,
    rect: Rect,
    e: &EndPane,
    marks: Marks,
    hovered: Option<WorkholdingItem>,
) -> Vec<Hit> {
    let painter = painter.with_clip_rect(rect);
    let reach = e
        .a_arc
        .iter()
        .map(|p| p[0].hypot(p[1]))
        .chain(e.clearance_radius)
        .chain(e.part_radius)
        .chain(match e.stock {
            Some(EndStock::Circle(r)) => Some(r),
            Some(EndStock::Box(b)) => {
                Some(b.h1.abs().max(b.h0.abs()).hypot(b.v1.abs().max(b.v0.abs())))
            }
            None => None,
        })
        .fold(10.0, f64::max);
    let body = Rect::from_min_max(rect.min + vec2(0.0, 28.0), rect.max - vec2(0.0, 28.0));
    let map = Map::fit(body, (-reach, reach), (-reach, reach));
    let centre = map.at(0.0, 0.0);
    label(
        &painter,
        rect.left_top() + vec2(12.0, 12.0),
        Align2::LEFT_TOP,
        "END VIEW FROM +X",
        roles::OVERLAY_MUTED,
    );

    if let Some(r) = e.clearance_radius {
        painter.circle_filled(
            centre,
            r as f32 * map.scale,
            to_color32(roles::CLEARANCE).gamma_multiply(0.06),
        );
        styled(
            &painter,
            &circle_points(centre, r as f32 * map.scale),
            roles::CLEARANCE,
            1.0,
            LineStyle::Dashed,
        );
    }
    if let Some(r) = e.part_radius {
        let pr = r as f32 * map.scale;
        painter.circle_filled(centre, pr, to_color32(roles::PART));
        painter.circle_stroke(
            centre,
            pr,
            Stroke::new(roles::LINE_WIDTH, to_color32(roles::FEATURE_EDGES)),
        );
    }
    match e.stock {
        Some(EndStock::Circle(r)) => styled(
            &painter,
            &circle_points(centre, r as f32 * map.scale),
            roles::STOCK,
            1.5,
            LineStyle::Dashed,
        ),
        Some(EndStock::Box(b)) => {
            let r = Rect::from_two_pos(map.at(b.h0, b.v1), map.at(b.h1, b.v0));
            styled(
                &painter,
                &closed(vec![
                    r.left_top(),
                    r.right_top(),
                    r.right_bottom(),
                    r.left_bottom(),
                ]),
                roles::STOCK,
                1.5,
                LineStyle::Dashed,
            );
        }
        None => {}
    }
    let r = reach as f32 * map.scale;
    styled(
        &painter,
        &[centre - vec2(r, 0.0), centre + vec2(r, 0.0)],
        roles::ROTARY_AXIS,
        1.0,
        LineStyle::DashDot,
    );
    styled(
        &painter,
        &[centre - vec2(0.0, r), centre + vec2(0.0, r)],
        roles::ROTARY_AXIS,
        1.0,
        LineStyle::DashDot,
    );

    let mut shapes = Vec::new();
    for t in &e.tabs {
        let c = map.at(t.centre[0], t.centre[1]);
        let points = circle_points(c, t.radius as f32 * map.scale);
        if !t.failing {
            painter.add(Shape::convex_polygon(
                points.clone(),
                to_color32(roles::TAB),
                Stroke::NONE,
            ));
        }
        outline(
            &painter,
            &points,
            roles::TAB_LINE,
            t.failing,
            t.item,
            marks,
            hovered,
        );
        shapes.push((t.item, points, t.description.clone()));
    }
    if let Some((angle, text)) = &e.start_angle {
        let dir = vec2(
            angle.to_radians().cos() as f32,
            -angle.to_radians().sin() as f32,
        );
        let tip = centre + dir * (r * 0.8);
        painter.line_segment(
            [centre, tip],
            Stroke::new(roles::LINE_WIDTH, roles::OVERLAY_TEXT),
        );
        label(
            &painter,
            tip + dir * 6.0,
            Align2::LEFT_CENTER,
            text,
            roles::OVERLAY_TEXT,
        );
    }
    // The A+ arrow, along the arc the kinematics give.
    let points: Vec<Pos2> = e.a_arc.iter().map(|p| map.at(p[0], p[1])).collect();
    if let [.., before, tip] = points.as_slice() {
        let cyan = to_color32(roles::ROTARY_AXIS);
        painter.add(Shape::line(
            points.clone(),
            Stroke::new(roles::AXIS_WIDTH, cyan),
        ));
        crate::viewport_overlays::arrowhead(&painter, *before, *tip, cyan);
        if let Some(start) = points.get(points.len() / 6) {
            label(
                &painter,
                *start + vec2(-10.0, -8.0),
                Align2::CENTER_CENTER,
                "A+",
                cyan,
            );
        }
    }
    let muted = roles::OVERLAY_MUTED;
    label(
        &painter,
        centre + vec2(0.0, -r - 6.0),
        Align2::CENTER_BOTTOM,
        "+Z",
        muted,
    );
    label(
        &painter,
        centre + vec2(0.0, r + 6.0),
        Align2::CENTER_TOP,
        "−Z",
        muted,
    );
    label(
        &painter,
        centre + vec2(r + 6.0, 0.0),
        Align2::LEFT_CENTER,
        "+Y",
        muted,
    );
    label(
        &painter,
        centre + vec2(-r - 6.0, 0.0),
        Align2::RIGHT_CENTER,
        "−Y",
        muted,
    );
    label(
        &painter,
        centre + vec2(-r - 6.0, 14.0),
        Align2::RIGHT_CENTER,
        "front",
        muted,
    );
    label(
        &painter,
        rect.center_bottom() - vec2(0.0, 12.0),
        Align2::CENTER_BOTTOM,
        "A+ turns the top toward the front",
        roles::OVERLAY_TEXT,
    );
    shapes
}

/// Draws both panes in `ui`'s space: the front view, then the end view 270
/// pixels wide, side by side (11.3). Hovering a held region or a tab opens
/// its tooltip and outlines it (7.5); clicking one is reported. While the
/// workholding section has unapplied changes (`draft`), the panes draw the
/// draft and a chip at the top left says so (11.7).
pub fn show(ui: &mut egui::Ui, panes: &SetupPanes, marks: Marks, draft: bool) -> Interaction {
    let full = ui.available_rect_before_wrap();
    let response = ui.allocate_rect(full, Sense::click());
    let painter = ui.painter_at(full);
    painter.rect_filled(full, 0.0, roles::VIEW_BOTTOM);
    painter.rect_filled(
        Rect::from_min_max(full.min, pos2(full.max.x, full.center().y)),
        0.0,
        roles::VIEW_TOP.gamma_multiply(0.4),
    );
    let split = (full.max.x - END_PANE_WIDTH.min(full.width() / 2.0)).max(full.min.x);
    let (front_rect, end_rect) = (
        Rect::from_min_max(full.min, pos2(split, full.max.y)),
        Rect::from_min_max(pos2(split, full.min.y), full.max),
    );
    // Hover is decided from the outlines of the last frame, so each pane is
    // drawn once.
    let key = ui.id().with("setup panes hits");
    let previous: Vec<Hit> = ui.data(|d| d.get_temp(key)).unwrap_or_default();
    let hovered = response.hover_pos().and_then(|p| {
        previous
            .iter()
            .rev()
            .find(|(_, poly, _)| inside(poly, p))
            .cloned()
    });
    let item = hovered.as_ref().map(|(i, _, _)| *i);
    let mut hits = front(&painter, front_rect, &panes.front, marks, item);
    hits.extend(end(&painter, end_rect, &panes.end, marks, item));
    ui.data_mut(|d| d.insert_temp(key, hits));
    if draft {
        let text = "Showing unapplied workholding";
        let galley = painter.layout_no_wrap(text.to_owned(), font(), roles::OVERLAY_TEXT);
        let chip = Rect::from_min_size(
            front_rect.min + vec2(12.0, 12.0),
            galley.size() + vec2(16.0, 8.0),
        );
        painter.rect_filled(chip, 3.0, roles::OVERLAY_FILL);
        painter.rect_stroke(
            chip,
            3.0,
            Stroke::new(roles::LINE_WIDTH, roles::OVERLAY_BORDER),
            egui::StrokeKind::Inside,
        );
        painter.galley(chip.min + vec2(8.0, 4.0), galley, roles::OVERLAY_TEXT);
    }
    painter.line_segment(
        [pos2(split, full.min.y), pos2(split, full.max.y)],
        Stroke::new(roles::LINE_WIDTH, roles::OVERLAY_BORDER),
    );
    if let Some((_, _, text)) = hovered {
        response.clone().on_hover_text(text);
    }
    Interaction {
        hovered: item,
        clicked: if response.clicked() { item } else { None },
    }
}

/// A legend swatch for a pane role, drawn the way the role is drawn (7.4,
/// 7.5).
pub fn swatch(painter: &Painter, r: Rect, entry: LegendEntry) {
    let line = |colour: Colour, style: LineStyle| {
        let y = r.center().y;
        styled(
            painter,
            &[pos2(r.left(), y), pos2(r.right(), y)],
            colour,
            2.0,
            style,
        );
    };
    match entry {
        LegendEntry::RotaryAxis => line(roles::ROTARY_AXIS, LineStyle::DashDot),
        LegendEntry::Stock => line(roles::STOCK, LineStyle::Dashed),
        LegendEntry::Clearance => line(roles::CLEARANCE, LineStyle::Dashed),
        LegendEntry::TravelLimit => {
            let band = Rect::from_center_size(r.center(), vec2(r.width(), 8.0));
            hatch(painter, band, to_color32(roles::AVOID), false);
        }
        LegendEntry::YIntoScreen => {
            let green = to_color32(roles::WORK_Y);
            let c = r.center();
            painter.circle_stroke(c, 5.0, Stroke::new(1.5, green));
            painter.line_segment(
                [c + vec2(-3.0, -3.0), c + vec2(3.0, 3.0)],
                Stroke::new(1.0, green),
            );
            painter.line_segment(
                [c + vec2(-3.0, 3.0), c + vec2(3.0, -3.0)],
                Stroke::new(1.0, green),
            );
        }
        LegendEntry::HeldRegion => swatch_for(painter, r, RoleEntry::HeldRegion, false),
        LegendEntry::Tab => swatch_for(painter, r, RoleEntry::Tab, false),
    }
}

/// The legend strip's width for a swatch: lines are wider than boxes.
pub fn swatch_width(entry: LegendEntry) -> f32 {
    match entry {
        LegendEntry::HeldRegion | LegendEntry::Tab | LegendEntry::YIntoScreen => 14.0,
        _ => 22.0,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp, clippy::unwrap_used)]

    use openchuck_app::setup_panes::{FrontSolid, Rect2};

    use super::*;

    fn tab_panes() -> SetupPanes {
        let tab = FrontSolid {
            item: WorkholdingItem::Tab(0),
            outline: vec![[-2.0, -2.0], [2.0, -3.0], [2.0, 3.0], [-2.0, 2.0]],
            label: Some("Tab 1".to_owned()),
            description: "Tab 1 · frustum".to_owned(),
            failing: false,
        };
        let held = FrontSolid {
            item: WorkholdingItem::Start,
            outline: vec![[-16.0, -13.0], [-10.0, -13.0], [-10.0, 13.0], [-16.0, 13.0]],
            label: None,
            description: "Held region at the start".to_owned(),
            failing: true,
        };
        SetupPanes {
            front: FrontPane {
                stock: Some(Rect2 {
                    h0: -16.0,
                    h1: 16.0,
                    v0: -13.0,
                    v1: 13.0,
                }),
                part: vec![[-8.0, -10.0, 10.0], [0.0, -10.0, 10.0], [8.0, -9.0, 9.0]],
                axis: (-20.0, 20.0),
                tabs: vec![tab],
                held: vec![held],
                safe_z: Some(20.0),
                travel_z: Some(30.0),
                ..FrontPane::default()
            },
            end: EndPane {
                stock: Some(EndStock::Circle(13.0)),
                part_radius: Some(10.0),
                clearance_radius: Some(20.0),
                a_arc: vec![[0.0, 25.0], [10.0, 20.0], [20.0, 10.0]],
                start_angle: Some((90.0, "Start 90.00°".to_owned())),
                ..EndPane::default()
            },
            legend: Vec::new(),
        }
    }

    fn texts(shapes: &[egui::epaint::ClippedShape]) -> Vec<String> {
        shapes
            .iter()
            .filter_map(|c| match &c.shape {
                Shape::Text(t) => Some(t.galley.text().to_owned()),
                _ => None,
            })
            .collect()
    }

    /// Runs `show` for one frame at `size`, with the pointer at `pointer`.
    fn frame(
        ctx: &egui::Context,
        panes: &SetupPanes,
        marks: Marks,
        pointer: Option<Pos2>,
        click: bool,
    ) -> (Interaction, Vec<egui::epaint::ClippedShape>) {
        let mut events = Vec::new();
        if let Some(p) = pointer {
            events.push(egui::Event::PointerMoved(p));
            if click {
                for pressed in [true, false] {
                    events.push(egui::Event::PointerButton {
                        pos: p,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    });
                }
            }
        }
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(900.0, 600.0))),
            events,
            ..Default::default()
        };
        let mut seen = Interaction::default();
        let mut output = ctx.run_ui(input, |ui| seen = show(ui, panes, marks, false));
        output.textures_delta.clear();
        (seen, output.shapes)
    }

    #[test]
    fn req_ui_03_the_panes_draw_their_labels_and_dimensions() {
        let panes = tab_panes();
        let ctx = egui::Context::default();
        let (_, shapes) = frame(&ctx, &panes, Marks::default(), None, false);
        assert!(!texts(&shapes).iter().any(|s| s.starts_with("Showing")));
        let t = texts(&shapes);
        for needle in [
            "Z+ travel limit",
            "Safe Z +20",
            "A axis",
            "Tab 1",
            "END VIEW FROM +X",
            "A+ turns the top toward the front",
            "Start 90.00°",
            "A+",
            "+Z",
            "−Z",
            "+Y",
            "−Y",
            "front",
        ] {
            assert!(t.iter().any(|s| s == needle), "{needle}: {t:?}");
        }
    }

    #[test]
    fn req_ui_25_hovering_a_tab_outlines_it_and_clicking_reports_it() {
        let panes = tab_panes();
        let ctx = egui::Context::default();
        // The pane is laid out at 900 by 600, the end view 270 wide: find the
        // tab's centre from the same mapping the drawing uses.
        let front = Rect::from_min_max(pos2(0.0, 0.0), pos2(630.0, 600.0));
        let (h, v) = front_bounds(&panes.front);
        let map = Map::fit(front, h, v);
        let tab = map.at(0.0, 0.0);
        // The first frame learns the outlines; the next sees the pointer.
        let _ = frame(&ctx, &panes, Marks::default(), None, false);
        let (seen, plain) = frame(&ctx, &panes, Marks::default(), Some(tab), false);
        assert_eq!(seen.hovered, Some(WorkholdingItem::Tab(0)));
        assert_eq!(seen.clicked, None);
        // The hovered outline adds a 2-pixel yellow line.
        let (_, idle) = frame(&ctx, &panes, Marks::default(), Some(pos2(5.0, 5.0)), false);
        assert!(plain.len() > idle.len());
        let (seen, _) = frame(&ctx, &panes, Marks::default(), Some(tab), true);
        assert_eq!(seen.clicked, Some(WorkholdingItem::Tab(0)));
        // Focus and selection outline a solid too, without the pointer.
        let marks = Marks {
            focused: Some(WorkholdingItem::Tab(0)),
            selected: Some(WorkholdingItem::Start),
        };
        let (_, marked) = frame(&ctx, &panes, marks, None, false);
        assert!(marked.len() > idle.len());
    }

    #[test]
    fn req_ui_03_dashes_follow_the_line_style_patterns() {
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let painter = ui.painter().clone();
            let line = [pos2(0.0, 0.0), pos2(100.0, 0.0)];
            // Dashed: 5 on, 4 off: 12 dashes in 100 pixels.
            styled(&painter, &line, roles::STOCK, 1.0, LineStyle::Dashed);
            styled(&painter, &line, roles::ROTARY_AXIS, 1.0, LineStyle::DashDot);
            styled(&painter, &line, roles::ROTARY_AXIS, 1.0, LineStyle::Solid);
            // A line of no length draws nothing and does not hang.
            styled(
                &painter,
                &[pos2(1.0, 1.0), pos2(1.0, 1.0)],
                roles::STOCK,
                1.0,
                LineStyle::Dashed,
            );
        });
        output.textures_delta.clear();
        let segments = |colour: Color32| {
            output
                .shapes
                .iter()
                .filter(|c| matches!(&c.shape, Shape::LineSegment { stroke, .. } if stroke.color == colour))
                .count()
        };
        assert_eq!(segments(to_color32(roles::STOCK)), 12);
        // Dash-dot: a 14-pixel dash and a 2-pixel dot every 24 pixels: five
        // dashes and four dots in 100 pixels (a solid line is one path, not segments).
        assert_eq!(segments(to_color32(roles::ROTARY_AXIS)), 9);
    }

    #[test]
    fn req_ui_03_a_point_is_inside_a_polygon_only_when_it_is() {
        let square = [
            pos2(0.0, 0.0),
            pos2(10.0, 0.0),
            pos2(10.0, 10.0),
            pos2(0.0, 10.0),
        ];
        assert!(inside(&square, pos2(5.0, 5.0)));
        assert!(!inside(&square, pos2(15.0, 5.0)));
        assert!(!inside(&[], pos2(0.0, 0.0)));
        assert_eq!(closed(Vec::new()), Vec::<Pos2>::new());
        assert_eq!(closed(square.to_vec()).len(), 5);
    }

    #[test]
    fn req_ui_03_every_pane_legend_entry_has_a_swatch() {
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            for entry in [
                LegendEntry::RotaryAxis,
                LegendEntry::Stock,
                LegendEntry::Clearance,
                LegendEntry::TravelLimit,
                LegendEntry::YIntoScreen,
                LegendEntry::HeldRegion,
                LegendEntry::Tab,
            ] {
                let r = Rect::from_min_size(pos2(0.0, 0.0), vec2(swatch_width(entry), 14.0));
                swatch(ui.painter(), r, entry);
            }
        });
        output.textures_delta.clear();
        assert!(output.shapes.len() >= 7);
        assert_eq!(swatch_width(LegendEntry::Tab), 14.0);
        assert_eq!(swatch_width(LegendEntry::Stock), 22.0);
    }

    #[test]
    fn req_ui_03_a_box_stock_and_an_empty_pane_still_draw() {
        let mut panes = tab_panes();
        panes.end.stock = Some(EndStock::Box(Rect2 {
            h0: -8.0,
            h1: 8.0,
            v0: -6.0,
            v1: 6.0,
        }));
        panes.front.part.clear();
        let ctx = egui::Context::default();
        let (_, shapes) = frame(&ctx, &panes, Marks::default(), None, false);
        assert!(!shapes.is_empty());
        let (_, empty) = frame(&ctx, &SetupPanes::default(), Marks::default(), None, false);
        assert!(!empty.is_empty());
    }

    #[test]
    fn req_ui_25_a_draft_in_the_panes_carries_its_chip() {
        let ctx = egui::Context::default();
        let panes = tab_panes();
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(900.0, 600.0))),
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            show(ui, &panes, Marks::default(), true);
        });
        output.textures_delta.clear();
        assert!(
            texts(&output.shapes)
                .iter()
                .any(|s| s == "Showing unapplied workholding")
        );
    }
}
