// SPDX-License-Identifier: AGPL-3.0-or-later
//! Moves sampled into poses (OI-102).
//!
//! The controller moves X, Y, Z and A linearly together, so between two IR
//! poses the tool follows a straight line in machine coordinates while the
//! part turns. Each move is replayed that way through the shared kinematics
//! (`openchuck_types::kinematics`) and sampled so that no point of the tool
//! that can reach the stock moves further than the spacing (the simulation
//! cell) between samples: the swept volume between poses is not built.

use openchuck_types::geometry::Point3;
use openchuck_types::ir::{MoveKind, ToolPose};
use openchuck_types::kinematics::{
    AsRotaryFrame, MachinePosition, RotaryFrame, machine_xyz, tool_axis_in_model,
    tool_tip_in_model,
};
use openchuck_types::units::Degrees;

use crate::dexel::Pose;

/// The most samples one move may produce, a guard against a broken move.
pub const MAX_SAMPLES_PER_MOVE: usize = 1_000_000;

/// How finely to sample.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sampling {
    /// Largest movement of any tool point between samples, in millimetres.
    pub spacing: f64,
    /// The largest distance from the rotary axis at which the tool can meet
    /// material (the stock's radius): a point of the tool further out moves
    /// further per degree but cuts nothing.
    pub reach: f64,
    /// The tool's extent from its tip (the flute top and radius combined).
    pub tool_extent: f64,
}

/// The poses of the move from `from` to `to`, excluding `from` and ending
/// exactly at `to`.
pub fn sample_move(
    frame: &impl AsRotaryFrame,
    from: &ToolPose,
    to: &ToolPose,
    sampling: &Sampling,
    out: &mut Vec<Pose>,
) {
    let frame: RotaryFrame = frame.rotary_frame();
    let m0 = machine_xyz(&frame, from.tip, from.a);
    let m1 = machine_xyz(&frame, to.tip, to.a);
    let linear = m0.distance(m1);
    let turn = (to.a.0 - from.a.0).to_radians().abs();
    let radius = |p: Point3| p.y.hypot(p.z);
    let reach = (radius(from.tip).max(radius(to.tip)) + sampling.tool_extent).min(sampling.reach);
    let travel = linear.max(turn * reach.max(0.0));
    let spacing = sampling.spacing.max(1e-6);
    let n = ((travel / spacing).ceil() as usize).clamp(1, MAX_SAMPLES_PER_MOVE);
    for k in 1..=n {
        if k == n {
            out.push(Pose {
                tip: to.tip,
                axis: tool_axis_in_model(&frame, to.a).get(),
            });
            break;
        }
        let t = k as f64 / n as f64;
        let a = Degrees(from.a.0 + t * (to.a.0 - from.a.0));
        let xyz = m0 + (m1 - m0) * t;
        let tip = tool_tip_in_model(&frame, &MachinePosition { xyz, a });
        out.push(Pose {
            tip,
            axis: tool_axis_in_model(&frame, a).get(),
        });
    }
}

/// True for moves that may not remove material: a rapid that cuts is
/// reported (SIM-03 report; collision checking proper is COL-01).
pub fn must_not_cut(kind: MoveKind) -> bool {
    matches!(kind, MoveKind::Rapid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use openchuck_types::geometry::UnitVec3;
    use openchuck_types::machine::MachineProfile;

    fn pose(tip: Point3, a: f64) -> ToolPose {
        ToolPose {
            tip,
            axis: UnitVec3::Z,
            a: Degrees(a),
        }
    }

    #[test]
    fn req_sim_03_a_quarter_turn_is_sampled_around_the_axis() {
        let profile = MachineProfile::example_x_rotary();
        let from = pose(Point3::new(0.0, 0.0, 10.0), 0.0);
        // The same machine position after a quarter turn of A: the tip in the
        // model frame has turned by a quarter about X.
        let xyz = machine_xyz(&profile, from.tip, from.a);
        let tip = tool_tip_in_model(
            &profile,
            &MachinePosition {
                xyz,
                a: Degrees(90.0),
            },
        );
        let to = pose(tip, 90.0);
        let sampling = Sampling {
            spacing: 0.1,
            reach: 10.0,
            tool_extent: 0.0,
        };
        let mut out = Vec::new();
        sample_move(&profile, &from, &to, &sampling, &mut out);
        // An arc of radius 10 over a quarter turn is 15.7 mm: 158 samples.
        assert_eq!(out.len(), 158);
        for p in &out {
            assert!((p.tip.y.hypot(p.tip.z) - 10.0).abs() < 1e-9);
        }
        let last = out.last().unwrap();
        assert!(last.tip.distance(to.tip) < 1e-12);
    }
}
