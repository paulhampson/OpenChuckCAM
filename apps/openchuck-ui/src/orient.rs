// SPDX-License-Identifier: AGPL-3.0-or-later
//! Which frame the viewport shows (issue 531; UI-02, UI-03, SIM-02): whether
//! the part is turned to the simulated angle, and which frame the view cube's
//! faces refer to. One rule feeds the part's turn, the cube's caption and its
//! tooltip, so they cannot disagree.

use openchuck_app::playback::ViewFrame;

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
    fn req_ui_02_the_cube_caption_and_tooltip_are_plain_words() {
        assert_eq!(cube_caption(ViewFrame::Machine), "Machine");
        assert_eq!(cube_caption(ViewFrame::Part), "Part");
        assert!(cube_tooltip(ViewFrame::Machine).contains("machine's"));
        assert!(cube_tooltip(ViewFrame::Part).contains("part's"));
    }
}
