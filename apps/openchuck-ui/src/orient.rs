// SPDX-License-Identifier: AGPL-3.0-or-later
//! Which frame the viewport shows (issue 531; UI-02, UI-03, SIM-02): whether
//! the part is turned to the simulated angle, and which frame the view cube's
//! faces refer to. One rule feeds the part's turn, the cube's caption and its
//! tooltip, so they cannot disagree.

use eframe::egui::{Pos2, Rect, Vec2, pos2, vec2};
use openchuck_app::playback::ViewFrame;
use openchuck_viewport::Layer;

use crate::viewport::ViewStep;

/// How far the part is turned in the view, degrees. The simulated angle
/// (`pose_turn`, already for `view`) shows only on Simulate; every other step
/// shows the part as designed, at A = 0 (SIM-02), whatever the playback head
/// was left at.
pub fn part_turn_shown(step: ViewStep, pose_turn: f64) -> f64 {
    if step == ViewStep::Simulate {
        pose_turn
    } else {
        0.0
    }
}

/// Whether the viewport draws `layer` on `step` (issue 535): the cut part
/// (`Stock`), the tool and the faint trace belong to the playback, so they
/// show on Simulate only; the toolpath lines show on every step (each
/// operation's eye, section 6). The layers are kept, not cleared, so Simulate
/// shows them again at the kept cursor (decision 0238 item 5).
pub fn layer_drawn_on(step: ViewStep, layer: Layer) -> bool {
    !matches!(layer, Layer::Stock | Layer::Tool | Layer::Trace) || step == ViewStep::Simulate
}

/// The frame the view cube's faces refer to: the machine's while the part
/// turns inside the view (Simulate with "Turn part with A" on), the part's
/// otherwise.
pub fn cube_frame(step: ViewStep, view: ViewFrame) -> ViewFrame {
    if step == ViewStep::Simulate && view == ViewFrame::Machine {
        ViewFrame::Machine
    } else {
        ViewFrame::Part
    }
}

/// The caption under the view cube (decision 0114: plain words).
pub fn cube_caption(frame: ViewFrame) -> &'static str {
    match frame {
        ViewFrame::Machine => "Machine",
        ViewFrame::Part => "Part",
    }
}

/// The tooltip of the view cube's caption.
pub fn cube_tooltip(frame: ViewFrame) -> &'static str {
    match frame {
        ViewFrame::Machine => {
            "Machine view: the faces are the machine's, and Top is toward the tool. \
             The part turns inside the box as A changes."
        }
        ViewFrame::Part => {
            "Part view: the faces are the part's and stay with it. Where the tool moves round \
             the part, the part does not turn (\"Turn part with A\" is off, or this is not \
             Simulate)."
        }
    }
}

/// The least gap between two guide labels, pixels (issue 531).
pub const LABEL_GAP: f32 = 3.0;
/// The farthest a label is moved from where its point puts it, pixels.
pub const LABEL_REACH: f32 = 48.0;
/// How far from its point a label usually starts, right and up (7.3).
const LABEL_NUDGE: Vec2 = vec2(6.0, -4.0);

/// A guide label's size in pixels (12-point monospace).
pub fn label_size(text: &str) -> Vec2 {
    vec2(7.3 * text.chars().count() as f32, 14.0)
}

/// Where a guide label at the screen point `anchor` is drawn before any
/// moving: its lower left corner a little right of and above the point.
pub fn label_home(anchor: Pos2, size: Vec2) -> Rect {
    let corner = anchor + LABEL_NUDGE;
    Rect::from_min_size(pos2(corner.x, corner.y - size.y), size)
}

/// Where each guide label goes so that no two touch (issue 531): each, in
/// the order given (the axis labels first, which their arrowheads already
/// attribute), keeps its home unless it lies within `gap` of one already
/// placed; then it takes the nearest free place, trying eight directions at
/// growing distances up to [`LABEL_REACH`]. With no free place it stays home.
pub fn separate_labels(labels: &[(Pos2, Vec2)], gap: f32) -> Vec<Rect> {
    let mut placed: Vec<Rect> = Vec::with_capacity(labels.len());
    for &(anchor, size) in labels {
        let home = label_home(anchor, size);
        let free = |r: Rect| placed.iter().all(|p| !p.expand(gap).intersects(r));
        let mut spot = home;
        if !free(home) {
            let diagonal = std::f32::consts::FRAC_1_SQRT_2;
            let directions = [
                vec2(1.0, 0.0),
                vec2(-1.0, 0.0),
                vec2(0.0, -1.0),
                vec2(0.0, 1.0),
                vec2(diagonal, -diagonal),
                vec2(-diagonal, -diagonal),
                vec2(diagonal, diagonal),
                vec2(-diagonal, diagonal),
            ];
            let mut distance = 3.0;
            'search: while distance <= LABEL_REACH {
                for d in directions {
                    let r = home.translate(d * distance);
                    if free(r) {
                        spot = r;
                        break 'search;
                    }
                }
                distance += 3.0;
            }
        }
        placed.push(spot);
    }
    placed
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [ViewStep; 4] = [
        ViewStep::Model,
        ViewStep::Setup,
        ViewStep::Operations,
        ViewStep::Simulate,
    ];

    // The turn is passed through or exactly zero.
    #[test]
    #[allow(clippy::float_cmp)]
    fn req_sim_02_the_part_is_at_its_design_orientation_outside_simulate() {
        assert_eq!(part_turn_shown(ViewStep::Simulate, 123.5), 123.5);
        for step in [ViewStep::Model, ViewStep::Setup, ViewStep::Operations] {
            assert_eq!(part_turn_shown(step, 123.5), 0.0, "{step:?}");
        }
    }

    #[test]
    fn req_sim_01_the_cut_part_tool_and_trace_draw_on_simulate_only() {
        for step in ALL {
            let simulate = step == ViewStep::Simulate;
            for layer in [Layer::Stock, Layer::Tool, Layer::Trace] {
                assert_eq!(layer_drawn_on(step, layer), simulate, "{step:?} {layer:?}");
            }
            for layer in [Layer::Scene, Layer::Paths, Layer::Overlay, Layer::Focus] {
                assert!(layer_drawn_on(step, layer), "{step:?} {layer:?}");
            }
        }
    }

    #[test]
    fn req_ui_02_the_cube_names_the_machine_only_where_the_part_turns() {
        for step in ALL {
            for view in [ViewFrame::Machine, ViewFrame::Part] {
                let simulate_turning = step == ViewStep::Simulate && view == ViewFrame::Machine;
                assert_eq!(
                    cube_frame(step, view) == ViewFrame::Machine,
                    simulate_turning,
                    "{step:?} {view:?}"
                );
            }
        }
    }

    #[test]
    fn req_ui_03_separate_labels_keeps_a_free_label_home_and_moves_a_touching_one() {
        let size = label_size("A0");
        let a = pos2(100.0, 100.0);
        // A label with nothing near it stays at its home.
        let alone = separate_labels(&[(a, size)], LABEL_GAP);
        assert_eq!(alone, vec![label_home(a, size)]);
        // Two at one point: the first keeps its home, the second moves, by no
        // more than the reach, and they do not touch.
        let two = separate_labels(&[(a, size), (a, size)], LABEL_GAP);
        assert_eq!(two[0], label_home(a, size));
        assert!(!two[0].expand(LABEL_GAP).intersects(two[1]));
        assert!((two[1].min - two[0].min).length() <= LABEL_REACH);
        // The same input gives the same places.
        assert_eq!(two, separate_labels(&[(a, size), (a, size)], LABEL_GAP));
    }

    #[test]
    fn req_ui_02_the_cube_caption_and_tooltip_are_plain_words() {
        assert_eq!(cube_caption(ViewFrame::Machine), "Machine");
        assert_eq!(cube_caption(ViewFrame::Part), "Part");
        assert!(cube_tooltip(ViewFrame::Machine).contains("machine's"));
        assert!(cube_tooltip(ViewFrame::Part).contains("part's"));
    }
}
