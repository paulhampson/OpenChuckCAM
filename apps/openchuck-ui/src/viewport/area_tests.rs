// SPDX-License-Identifier: AGPL-3.0-or-later
//! The unreachable areas in the view (OP-20, UI-11, UI-12, NFR-15; decision
//! record 0515; issue #416): the hatch layer, the Show menu's tick, the
//! markers and the legend, and the camera's turn to an area with its undo.

#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::float_cmp)]

use eframe::egui::{self, Rect, pos2};
use openchuck_app::project::id::{ModelId, OperationId};
use openchuck_app::scene::SceneRole;
use openchuck_app::selection::TriangleSet;
use openchuck_app::setup::Point3;
use openchuck_app::unreached::{AreaMarks, FrameTarget};
use openchuck_viewport::{HatchKind, Layer};

use super::{ShownArea, ShownOperation, ViewStep, Viewport, Visibility, areas};
use crate::roles;

/// A viewport holding the corpus cube (centred on the rotary axis), and the
/// model's id.
fn cube_viewport(name: &str) -> (Viewport, ModelId) {
    let (dir, app, _, _) = crate::testing::cube_project(name);
    let view = app.start_scene(None).wait().unwrap();
    let _ = std::fs::remove_dir_all(dir);
    let model = view.meshes[0].model.unwrap();
    let mut viewport = Viewport::new(true);
    viewport.set_scene(view, true);
    (viewport, model)
}

/// The triangles of the cube's part whose centres are on its `+Z` face
/// (`axis` 2) or `+X` face (`axis` 0), and the face's centre.
fn face(viewport: &Viewport, axis: usize) -> (TriangleSet, [f64; 3]) {
    let mesh = &viewport
        .view()
        .meshes
        .iter()
        .find(|m| m.role == SceneRole::Part)
        .unwrap()
        .mesh;
    let coordinate = |p: Point3, k: usize| [p.x, p.y, p.z][k];
    let top = mesh
        .vertices
        .iter()
        .map(|p| coordinate(*p, axis))
        .fold(f64::MIN, f64::max);
    let mut set = TriangleSet::new();
    for (i, t) in mesh.triangles.iter().enumerate() {
        if t.iter()
            .all(|v| (coordinate(mesh.vertices[*v as usize], axis) - top).abs() < 1e-9)
        {
            set.insert(i as u32);
        }
    }
    let mut centre = [0.0; 3];
    for (k, c) in centre.iter_mut().enumerate() {
        *c = if k == axis {
            top
        } else {
            let values: Vec<f64> = mesh.vertices.iter().map(|p| coordinate(*p, k)).collect();
            0.5 * (values.iter().copied().fold(f64::MAX, f64::min)
                + values.iter().copied().fold(f64::MIN, f64::max))
        };
    }
    (set, centre)
}

fn shown(viewport: &Viewport, operation: OperationId, name: &str) -> ShownOperation {
    let (top, top_at) = face(viewport, 2);
    let (side, side_at) = face(viewport, 0);
    ShownOperation {
        operation,
        name: name.to_owned(),
        areas: vec![
            ShownArea {
                number: 1,
                marker: top_at,
                triangles: top,
            },
            ShownArea {
                number: 2,
                marker: side_at,
                triangles: side,
            },
        ],
    }
}

/// UI-11, decision 0515 items 3 and 4: the areas are hatched on their
/// triangles in a layer of their own, drawn in the part's frame with a
/// transparent fill; the Show menu's "Unreachable areas" (menu only, on by
/// default) hides them and their legend rows, and Simulate's tick boxes do
/// not carry it.
#[test]
fn req_ui_11_the_unreachable_areas_tick_hides_them() {
    assert!(Visibility::default().unreachable);
    let names: Vec<&str> = crate::viewport_toolbar::SHOW_OR_HIDE
        .iter()
        .map(|(n, _)| *n)
        .collect();
    assert!(names.contains(&"Unreachable areas"), "{names:?}");
    assert!(
        crate::viewport_toolbar::SIMULATE_SHOW
            .iter()
            .all(|(n, ..)| !n.contains("Unreachable")),
        "menu only"
    );
    let (mut viewport, model) = cube_viewport("area-tick");
    let operation = OperationId::new();
    let ops = vec![shown(&viewport, operation, "Finish")];
    let counts: Vec<usize> = ops[0].areas.iter().map(|a| a.triangles.len()).collect();
    assert_eq!(counts, [2, 2], "each face of the cube is two triangles");
    viewport.set_unreached(Some(model), ops.clone());
    let layer = viewport.layer(Layer::Areas).unwrap();
    assert_eq!(layer.meshes.len(), 2);
    for (mesh, area) in layer.meshes.iter().zip(&ops[0].areas) {
        assert_eq!(mesh.indices.len(), 3 * area.triangles.len());
        assert_eq!(mesh.slot, openchuck_viewport::Slot::Part);
        assert_eq!(mesh.colour[3], 0.0, "nothing is filled, only hatched");
        let hatch = mesh.hatch.unwrap();
        assert_eq!(hatch.colour, roles::AREA_HATCH_CORE);
        assert_eq!(
            hatch.kind,
            HatchKind::Surface {
                ring: roles::AREA_HATCH_CASING
            }
        );
        assert!(!mesh.translucent && !mesh.outline_only && mesh.outline.is_none());
    }
    assert_eq!(viewport.unreached_legend(), ["Unreachable (Finish)"]);
    let drawn = |viewport: &Viewport| {
        viewport
            .frame_layers()
            .into_iter()
            .find(|(l, ..)| *l == Layer::Areas)
            .map(|(.., shown)| shown)
    };
    assert_eq!(drawn(&viewport), Some(true));
    // Ticked off in the menu: the layer is kept but not drawn, and the
    // legend loses its rows.
    for step in [ViewStep::Operations, ViewStep::Simulate] {
        viewport.set_step(Some(step));
        let mut show = viewport.visibility();
        assert!(show.unreachable);
        show.unreachable = false;
        viewport.set_visibility(show);
        assert_eq!(drawn(&viewport), Some(false), "{step:?}");
        assert!(viewport.unreached_legend().is_empty(), "{step:?}");
        assert!(!viewport.visibility().unreachable, "{step:?}");
        show.unreachable = true;
        viewport.set_visibility(show);
        assert_eq!(drawn(&viewport), Some(true), "{step:?}");
    }
    // Nothing to show: no hatch, no rows.
    viewport.set_unreached(None, Vec::new());
    assert!(viewport.layer(Layer::Areas).unwrap().meshes.is_empty());
    assert!(viewport.unreached_legend().is_empty());
}

/// All the text the viewport paints in a frame, with its font size.
fn painted(viewport: &mut Viewport, ctx: &egui::Context) -> Vec<(String, f32)> {
    fn walk(shape: &egui::Shape, out: &mut Vec<(String, f32)>) {
        match shape {
            egui::Shape::Text(t) => {
                let size = t
                    .galley
                    .job
                    .sections
                    .first()
                    .map_or(0.0, |s| s.format.font_id.size);
                out.push((t.galley.text().to_owned(), size));
            }
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            _ => {}
        }
    }
    let mut texts = Vec::new();
    for _ in 0..2 {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                pos2(0.0, 0.0),
                egui::vec2(900.0, 700.0),
            )),
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| viewport.show(ui));
        output.textures_delta.clear();
        texts.clear();
        output
            .shapes
            .iter()
            .for_each(|s| walk(&s.shape, &mut texts));
    }
    texts
}

/// UI-11, NFR-15, decision 0515 item 4: one numbered marker per area, in the
/// order given (the warning's), in text of at least 12 points, and a legend
/// row naming the operation, on Operations and on Simulate.
#[test]
fn req_ui_11_markers_are_numbered_and_the_legend_names_the_operation() {
    let (mut viewport, model) = cube_viewport("area-markers");
    let operation = OperationId::new();
    viewport.set_unreached(Some(model), vec![shown(&viewport, operation, "Finish")]);
    let ctx = egui::Context::default();
    for step in [ViewStep::Operations, ViewStep::Simulate] {
        viewport.set_step(Some(step));
        let texts = painted(&mut viewport, &ctx);
        for number in ["1", "2"] {
            let (_, size) = texts
                .iter()
                .find(|(t, _)| t == number)
                .unwrap_or_else(|| panic!("{step:?}: no marker {number} in {texts:?}"));
            assert!(*size >= 12.0, "{step:?}: marker {number} is {size} points");
        }
        let (_, size) = texts
            .iter()
            .find(|(t, _)| t == "Unreachable (Finish)")
            .unwrap_or_else(|| panic!("{step:?}: no legend row in {texts:?}"));
        assert!(*size >= 12.0);
    }
    // Ticked off: neither markers nor the row.
    let mut show = viewport.visibility();
    show.unreachable = false;
    viewport.set_visibility(show);
    let texts = painted(&mut viewport, &ctx);
    assert!(!texts.iter().any(|(t, _)| t == "1" || t == "2"));
    assert!(!texts.iter().any(|(t, _)| t.starts_with("Unreachable (")));
    // A second operation has its own row.
    let mut show = viewport.visibility();
    show.unreachable = true;
    viewport.set_visibility(show);
    let mut second = shown(&viewport, OperationId::new(), "Second");
    second.areas.truncate(1);
    let first = shown(&viewport, operation, "Finish");
    viewport.set_unreached(Some(model), vec![first, second]);
    assert_eq!(
        viewport.unreached_legend(),
        ["Unreachable (Finish)", "Unreachable (Second)"]
    );
}

/// The text the viewport shows in a frame as laid out, glyph by glyph: a
/// row cut short to fit its box ends in "…" here, though its galley still
/// holds the whole string.
fn shown_text(viewport: &mut Viewport, ctx: &egui::Context) -> Vec<String> {
    fn walk(shape: &egui::Shape, out: &mut Vec<String>) {
        match shape {
            egui::Shape::Text(t) => out.push(
                t.galley
                    .rows
                    .iter()
                    .flat_map(|r| r.row.glyphs.iter().map(|g| g.chr))
                    .collect(),
            ),
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            _ => {}
        }
    }
    let mut texts = Vec::new();
    for _ in 0..2 {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                pos2(0.0, 0.0),
                egui::vec2(900.0, 700.0),
            )),
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| viewport.show(ui));
        output.textures_delta.clear();
        texts.clear();
        output
            .shapes
            .iter()
            .for_each(|s| walk(&s.shape, &mut texts));
    }
    texts
}

/// UI-11, NFR-15, decision 0515 item 4: the legend row names the operation
/// in full, on Operations and on Simulate, so the box is as wide as its
/// longest row (CodeRabbit on #489: the 124-point Operations box cut
/// "Unreachable (Finish)" short).
#[test]
fn req_ui_11_the_legend_row_shows_the_whole_operation_name() {
    let (mut viewport, model) = cube_viewport("area-legend-width");
    let ctx = egui::Context::default();
    for name in ["Finish", "Finish the dovetail undercut"] {
        viewport.set_unreached(
            Some(model),
            vec![shown(&viewport, OperationId::new(), name)],
        );
        let row = format!("Unreachable ({name})");
        for step in [ViewStep::Operations, ViewStep::Simulate] {
            viewport.set_step(Some(step));
            let texts = shown_text(&mut viewport, &ctx);
            assert!(
                texts.contains(&row),
                "{step:?}: {row:?} not shown whole in {texts:?}"
            );
        }
    }
}

/// UI-11, decision 0515 item 4: a legend widened for a long operation name
/// still stops short of the view cube on Operations; a name longer than
/// that ends in "…" (CodeRabbit on #543).
#[test]
fn req_ui_11_a_long_legend_row_stays_clear_of_the_view_cube() {
    let (mut viewport, model) = cube_viewport("area-legend-cube");
    let long = "Finish every hidden face of the dovetail undercut and the lip above it, \
                then the neck between them, then the far end";
    viewport.set_unreached(
        Some(model),
        vec![shown(&viewport, OperationId::new(), long)],
    );
    viewport.set_step(Some(ViewStep::Operations));
    let ctx = egui::Context::default();
    let rect = Rect::from_min_size(pos2(0.0, 0.0), egui::vec2(600.0, 500.0));
    let mut boxes = (None, None);
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        boxes = crate::viewport_overlays::legend_and_cube(ui, &viewport, rect);
    });
    output.textures_delta.clear();
    let (legend, cube) = (boxes.0.unwrap(), boxes.1.unwrap());
    assert!(
        legend.right() + 8.0 <= cube.left(),
        "legend {legend:?} runs under the cube {cube:?}"
    );
    assert!(
        legend.width() > 124.0,
        "the legend still widens: {legend:?}"
    );
}

fn area(number: usize, x: (f64, f64), direction: [f64; 3], marker: [f64; 3]) -> AreaMarks {
    AreaMarks {
        number,
        triangles: TriangleSet::new(),
        marker: Point3::new(marker[0], marker[1], marker[2]),
        unmatched: 0,
        frame: FrameTarget {
            x,
            a: 0.0,
            direction,
        },
    }
}

/// UI-12, decision 0515 item 5: the camera looks straight at the area from
/// the side it faces, its X range framed with a margin; the part keeps its
/// turn (the playback's placement) and only the camera moves.
#[test]
fn req_ui_12_show_in_view_frames_the_area_and_keeps_the_playback() {
    let (mut viewport, _) = cube_viewport("area-look");
    let ctx = egui::Context::default();
    let _ = painted(&mut viewport, &ctx);
    let operation = OperationId::new();
    // The playback has turned the part 40 degrees in the machine view.
    viewport.set_part_turn(40.0);
    let (turn_before, transforms_before) = (viewport.part_turn(), viewport.transforms());
    let direction = [0.0, 0.6, 0.8];
    let target = area(2, (3.0, 9.0), direction, [6.0, 2.0, 2.0]);
    viewport.look_at_area(operation, &target, 1.0);
    // Looked at where the part is drawn: the direction turned with it.
    let turned = openchuck_viewport::camera::transform(
        &openchuck_viewport::camera::rotation_x(40.0_f64.to_radians()),
        [direction[0], direction[1], direction[2], 0.0],
    );
    let (_, _, back) = viewport.camera.basis();
    for k in 0..3 {
        assert!((back[k] - turned[k]).abs() < 1e-9, "{back:?} / {turned:?}");
    }
    // Centred on the middle of the X range, which fits across the view with
    // a margin.
    let world = openchuck_viewport::camera::transform(
        &openchuck_viewport::camera::rotation_x(40.0_f64.to_radians()),
        [6.0, 2.0, 2.0, 1.0],
    );
    assert!((viewport.camera.target[0] - 6.0).abs() < 1e-9);
    assert!((viewport.camera.target[1] - world[1]).abs() < 1e-9);
    assert!((viewport.camera.target[2] - world[2]).abs() < 1e-9);
    let width = viewport.camera.visible_height() * viewport.aspect;
    assert!((6.0 * 1.2..=6.0 * 1.3 + 1e-6).contains(&width), "{width}");
    // The part is where the playback put it.
    assert_eq!(viewport.part_turn(), turn_before);
    assert_eq!(viewport.transforms(), transforms_before);
    // The marker pulses for a moment, then not.
    assert_eq!(
        viewport.pulsing(1.5).map(|(o, n, _)| (o, n)),
        Some((operation, 2))
    );
    assert!(!viewport.advance_motion(2.3), "the pulse is over");
    // From straight above: the top view, X to the right.
    viewport.set_part_turn(0.0);
    viewport.look_at_area(
        operation,
        &area(1, (0.0, 10.0), [0.0, 0.0, 1.0], [5.0, 0.0, 5.0]),
        3.0,
    );
    assert_eq!((viewport.camera.yaw, viewport.camera.pitch), (-90.0, 90.0));
    // A narrow area is not framed as a sliver.
    viewport.look_at_area(
        operation,
        &area(1, (5.0, 5.01), [0.0, 1.0, 0.0], [5.0, 5.0, 0.0]),
        4.0,
    );
    assert!(viewport.camera.visible_height() >= 2.0 - 1e-9);
}

/// UI-12: the move is one camera step, undone as Fit is; a move under way
/// is stopped by the undo; with the animation on it takes about 0.3 s and
/// lands on the same camera (instant otherwise, as no reduced-motion
/// setting exists yet).
#[test]
fn req_ui_12_show_in_view_is_one_camera_undo() {
    let (mut viewport, _) = cube_viewport("area-undo");
    let ctx = egui::Context::default();
    let _ = painted(&mut viewport, &ctx);
    let operation = OperationId::new();
    let target = area(1, (2.0, 8.0), [0.0, 1.0, 0.0], [5.0, 4.0, 0.0]);
    assert!(!viewport.can_undo_camera());
    let before = viewport.camera;
    viewport.look_at_area(operation, &target, 1.0);
    let after = viewport.camera;
    assert_ne!(after, before);
    assert!(viewport.can_undo_camera());
    assert!(viewport.undo_camera());
    assert_eq!(viewport.camera, before);
    assert!(!viewport.undo_camera(), "one step only");
    // Fit is a step as well.
    viewport.look_at_area(operation, &target, 1.0);
    viewport.fit();
    assert!(viewport.undo_camera());
    assert_eq!(viewport.camera, after);
    assert!(viewport.undo_camera());
    assert_eq!(viewport.camera, before);
    // Animated: the camera stays until the frames carry it, then lands.
    viewport.set_animated_orbit(true);
    viewport.look_at_area(operation, &target, 10.0);
    assert_eq!(viewport.camera, before);
    assert!(viewport.advance_motion(10.15));
    assert_ne!(viewport.camera, before);
    assert_ne!(viewport.camera, after);
    assert!(viewport.advance_motion(10.31), "the pulse goes on");
    assert_eq!(viewport.camera, after);
    // Undone in the middle of a move: back to where it was, and stopped.
    viewport.undo_camera();
    viewport.look_at_area(operation, &target, 20.0);
    viewport.advance_motion(20.1);
    assert!(viewport.undo_camera());
    viewport.advance_motion(20.2);
    assert_eq!(viewport.camera, before);
}

/// The blend of two cameras starts and ends on them, takes the short way
/// round for the yaw, and stays between.
#[test]
fn req_ui_12_the_orbit_goes_the_short_way_round() {
    let mut from = openchuck_viewport::Camera::default();
    let mut to = from;
    from.yaw = 350.0;
    to.yaw = 10.0;
    from.distance = 100.0;
    to.distance = 25.0;
    assert_eq!(areas::blend(&from, &to, 0.0).yaw, 350.0);
    assert!((areas::blend(&from, &to, 1.0).yaw - 10.0).abs() < 1e-9);
    let middle = areas::blend(&from, &to, 0.5);
    assert!((middle.yaw - 0.0).abs() < 1e-9 || (middle.yaw - 360.0).abs() < 1e-9);
    assert!((middle.distance - 50.0).abs() < 1e-9, "geometric middle");
}
