// SPDX-License-Identifier: AGPL-3.0-or-later
//! Tooltips stay where they first appear while the pointer moves over their
//! control (issue #294, F-101: the "Warn only" choice of a swarf operation's
//! avoid set).

#![allow(clippy::unwrap_used)]

use eframe::egui::{self, Event};

use super::frame_tests::Rig;
use super::*;

impl Rig {
    /// The rectangle of the first text painted exactly `text`.
    fn text_rect(&self, text: &str) -> Option<egui::Rect> {
        fn walk(shape: &egui::Shape, text: &str) -> Option<egui::Rect> {
            match shape {
                egui::Shape::Text(t) if t.galley.text() == text => {
                    Some(egui::Rect::from_min_size(t.pos, t.galley.size()))
                }
                egui::Shape::Vec(shapes) => shapes.iter().find_map(|s| walk(s, text)),
                _ => None,
            }
        }
        self.shapes().iter().find_map(|s| walk(&s.shape, text))
    }

    /// Where the first text that starts with `start` is painted.
    fn text_starting(&self, start: &str) -> Option<egui::Pos2> {
        fn walk(shape: &egui::Shape, start: &str) -> Option<egui::Pos2> {
            match shape {
                egui::Shape::Text(t) if t.galley.text().starts_with(start) => Some(t.pos),
                egui::Shape::Vec(shapes) => shapes.iter().find_map(|s| walk(s, start)),
                _ => None,
            }
        }
        self.shapes().iter().find_map(|s| walk(&s.shape, start))
    }
}

/// A swarf operation whose avoid set is on Warn only, its inspector open.
fn warn_only_rig(name: &str) -> Rig {
    use openchuck_app::project::{Item, OnBreak, SelectionTarget};
    use openchuck_app::selection::SelectionRole;
    let mut rig = Rig::cube(name, None);
    rig.size = egui::vec2(1440.0, 2400.0);
    let app = rig.shell.app.as_mut().unwrap();
    let op = app.project().operations()[0].clone();
    let model = app.project().models()[0].id;
    let rib = app
        .save_selection("Rib", model, SelectionTarget::Triangles([1].into()))
        .unwrap();
    let mut swarf = op.clone();
    swarf.strategy = "swarf".into();
    app.execute(Command::Replace(Item::Operation(swarf)))
        .unwrap();
    app.set_operation_selection(op.id, SelectionRole::Avoid, rib, true)
        .unwrap();
    app.set_on_break(op.id, SelectionRole::Avoid, rib, OnBreak::Warn)
        .unwrap();
    rig.settle();
    rig.shell.select(Selection::Operation(op.id));
    rig.frame(Vec::new());
    rig.frame(Vec::new());
    rig
}

const WARN_TIP: &str = "The path is kept where the tool would break this set";

/// Issue #294: moving the pointer over "Warn only" does not move its
/// tooltip.
#[test]
fn req_ui_07_the_warn_only_tooltip_stays_put_while_the_pointer_moves_over_it() {
    let mut rig = warn_only_rig("tooltip-warn-only");
    let label = rig.text_rect("Warn only").expect("Warn only is shown");
    // Rest on the label until the tooltip shows.
    let start = label.left_center() + egui::vec2(2.0, 0.0);
    rig.frame(vec![Event::PointerMoved(start)]);
    for _ in 0..60 {
        rig.frame(Vec::new());
    }
    let first = rig.text_starting(WARN_TIP).expect("the tooltip shows");
    let mut seen = vec![first];
    // Sweep across the label and back, one point per frame.
    let steps = 40;
    for i in 0..=2 * steps {
        let f = if i <= steps { i } else { 2 * steps - i } as f32 / steps as f32;
        let at = egui::pos2(
            label.left() + 2.0 + f * (label.width() - 4.0),
            label.center().y + if i % 2 == 0 { -2.0 } else { 2.0 },
        );
        rig.frame(vec![Event::PointerMoved(at)]);
        let p = rig
            .text_starting(WARN_TIP)
            .expect("the tooltip stays shown while the pointer is over the label");
        seen.push(p);
    }
    let moved: Vec<&egui::Pos2> = seen.iter().filter(|p| p.distance(first) > 0.5).collect();
    assert!(
        moved.is_empty(),
        "the tooltip first at {first:?} moved to {moved:?} (label {label:?})"
    );
}
