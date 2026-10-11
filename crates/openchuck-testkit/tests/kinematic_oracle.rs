// SPDX-License-Identifier: AGPL-3.0-or-later
//! The independent kinematic oracle against the shared machine model
//! (work breakdown slice 0 item 11 and its review checklist; verification plan
//! section 4.1, kinematic round trip: 1e-6 mm and 1e-6 degrees).

// Test code may unwrap, index and panic (NFR-07 applies to library code).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_cmp
)]

use openchuck_testkit::oracle::{OracleMachine, distance, model_tip};
use openchuck_types::geometry::Point3;
use openchuck_types::kinematics::{machine_xyz, tool_axis_in_model};
use openchuck_types::machine::{ASense, MachineProfile};
use openchuck_types::units::Degrees;

const LIMIT_MM: f64 = 1e-6;

/// A angles including negative angles, the 0/360 seam and several turns.
const ANGLES: &[f64] = &[
    -1080.0, -725.5, -360.0, -359.9999, -180.0, -90.0, -1e-7, 0.0, 1e-7, 45.0, 90.0, 179.9, 180.0,
    270.0, 359.9999, 360.0, 360.0001, 540.0, 1234.5,
];

fn tips() -> Vec<Point3> {
    vec![
        Point3::new(0.0, 0.0, 15.0),
        Point3::new(12.5, 15.0, 0.0),
        Point3::new(-30.0, -7.25, 9.5),
        Point3::new(80.0, 0.0, -0.001),
        Point3::new(5.0, 0.0, 0.0), // on the rotary axis
    ]
}

fn oracle_for(profile: &MachineProfile) -> OracleMachine {
    // Read plain numbers only; no shared code.
    OracleMachine {
        origin_x: profile.rotary_origin.x,
        origin_y: profile.rotary_origin.y,
        origin_z: profile.rotary_origin.z,
        sense: match profile.a_sense {
            ASense::Positive => 1.0,
            ASense::Negative => -1.0,
        },
    }
}

/// Largest disagreement between the shared model and the oracle.
fn worst_error(shared: &MachineProfile, oracle: &OracleMachine) -> f64 {
    let mut worst: f64 = 0.0;
    for &a in ANGLES {
        for tip in tips() {
            let xyz = machine_xyz(shared, tip, Degrees(a));
            let back = model_tip(oracle, xyz.x, xyz.y, xyz.z, a);
            worst = worst.max(distance(back, [tip.x, tip.y, tip.z]));
        }
    }
    worst
}

#[test]
fn req_op_05_op_06_gc_09_oracle_agrees_with_shared_model() {
    for sense in [ASense::Positive, ASense::Negative] {
        let mut profile = MachineProfile::example_x_rotary();
        profile.a_sense = sense;
        let worst = worst_error(&profile, &oracle_for(&profile));
        assert!(worst < LIMIT_MM, "{sense:?}: worst error {worst} mm");
    }
}

#[test]
fn req_op_05_op_06_gc_09_oracle_agrees_with_zero_offset() {
    let mut profile = MachineProfile::example_x_rotary();
    profile.rotary_origin = Point3::ORIGIN;
    assert!(worst_error(&profile, &oracle_for(&profile)) < LIMIT_MM);
}

#[test]
fn req_op_05_op_06_gc_09_oracle_fails_when_shared_a_sign_is_flipped() {
    // Simulates a sign error in the shared model: it uses the opposite sense
    // to the machine the oracle describes. The check must catch it.
    let machine = MachineProfile::example_x_rotary();
    let mut wrong = machine.clone();
    wrong.a_sense = machine.a_sense.flipped();
    let worst = worst_error(&wrong, &oracle_for(&machine));
    assert!(
        worst > 1.0,
        "a flipped A sign went unnoticed (worst error {worst} mm)"
    );
}

#[test]
fn req_op_05_op_06_gc_09_oracle_fails_when_shared_offset_is_wrong() {
    let machine = MachineProfile::example_x_rotary();
    let mut wrong = machine.clone();
    wrong.rotary_origin.z += 0.5;
    assert!(worst_error(&wrong, &oracle_for(&machine)) > 0.1);
}

#[test]
fn req_op_06_tool_axis_agrees_with_oracle_angle() {
    // The tool axis in the model frame is the machine +Z direction turned back
    // by the part rotation; in polar form its angle is 90 - sense * A degrees.
    for sense in [ASense::Positive, ASense::Negative] {
        let mut profile = MachineProfile::example_x_rotary();
        profile.a_sense = sense;
        let s = oracle_for(&profile).sense;
        for &a in ANGLES {
            let axis = tool_axis_in_model(&profile, Degrees(a)).get();
            let expected = (90.0 - s * a).to_radians();
            let error =
                ((axis.y - expected.cos()).powi(2) + (axis.z - expected.sin()).powi(2)).sqrt();
            // 1e-6 degrees on a unit vector is about 1.7e-8.
            assert!(axis.x.abs() < 1e-12 && error < 1.7e-8, "a = {a}: {axis:?}");
        }
    }
}
