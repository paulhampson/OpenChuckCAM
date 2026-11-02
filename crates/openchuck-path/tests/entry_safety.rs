// SPDX-License-Identifier: AGPL-3.0-or-later
//! Entry sweep certificates must detect obstacles between clear endpoints.
#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod support;
use openchuck_path::entry_safety::EntrySafety;
use openchuck_path::{MeshSurface, Solid};
use openchuck_types::geometry::{Aabb, Point3, UnitVec3};
use openchuck_types::ir::ToolPose;
use openchuck_types::job::{CancelToken, JobContext, NoProgress};
use openchuck_types::machine::MachineProfile;
use openchuck_types::tool::ToolShape;
use openchuck_types::units::Degrees;
fn box_at(x: f64, top: f64) -> Solid {
    Solid::cuboid(Aabb {
        min: Point3::new(x - 0.001, -1.0, -1.0),
        max: Point3::new(x + 0.001, 1.0, top),
    })
    .unwrap()
}
fn pose(x: f64) -> ToolPose {
    ToolPose {
        tip: Point3::new(x, 0.0, 10.0),
        axis: UnitVec3::Z,
        a: Degrees(0.0),
    }
}
#[test]
fn req_op_11_req_op_12_sweep_detects_thin_obstacle_between_clear_endpoints() {
    let obstacle = box_at(0.137, 11.0);
    let target = MeshSurface::new(&obstacle.mesh(8)).unwrap();
    let body = support::tool(ToolShape::FlatEnd, 1.0).profile().unwrap();
    let mut machine = MachineProfile::example_x_rotary();
    machine.rotary_origin = Point3::ORIGIN;
    let proof = EntrySafety {
        rotary_precision: openchuck_path::entry_safety::RotaryPrecision::Float32,
        target: &target,
        protected: &[],
        body: &body,
        machine: &machine,
    };
    let cancel = CancelToken::new();
    let ctx = JobContext::new(&cancel, &NoProgress);
    assert!(
        proof
            .verify(pose(-4.0), pose(-3.0), 0.0, 0.01, &mut 100_000, &ctx)
            .is_ok()
    );
    assert!(
        proof
            .verify(pose(-4.0), pose(4.0), 0.0, 0.01, &mut 100_000, &ctx)
            .is_err()
    );
}
#[test]
fn req_set_09_req_op_21_sweep_checks_protected_geometry_budget_and_cancellation() {
    let target = MeshSurface::new(&box_at(20.0, 1.0).mesh(8)).unwrap();
    let body = support::tool(ToolShape::FlatEnd, 1.0).profile().unwrap();
    let mut machine = MachineProfile::example_x_rotary();
    machine.rotary_origin = Point3::ORIGIN;
    let solids = [box_at(0.0, 12.0)];
    let proof = EntrySafety {
        rotary_precision: openchuck_path::entry_safety::RotaryPrecision::Float32,
        target: &target,
        protected: &solids,
        body: &body,
        machine: &machine,
    };
    let cancel = CancelToken::new();
    let ctx = JobContext::new(&cancel, &NoProgress);
    assert!(
        proof
            .verify(pose(-4.0), pose(4.0), 0.0, 0.01, &mut 100_000, &ctx)
            .is_err()
    );
    assert!(
        proof
            .verify(pose(-4.0), pose(-3.0), 0.0, 0.01, &mut 0, &ctx)
            .is_err()
    );
    cancel.cancel();
    assert!(
        proof
            .verify(pose(-4.0), pose(-3.0), 0.0, 0.01, &mut 100_000, &ctx)
            .is_err()
    );
}

#[test]
fn req_op_11_req_op_21_later_leg_reversal_and_final_connection_are_checked() {
    let target = MeshSurface::new(&box_at(20.0, 1.0).mesh(8)).unwrap();
    let body = support::tool(ToolShape::FlatEnd, 1.0).profile().unwrap();
    let mut machine = MachineProfile::example_x_rotary();
    machine.rotary_origin = Point3::ORIGIN;
    let solids = [box_at(0.137, 12.0)];
    let proof = EntrySafety {
        rotary_precision: openchuck_path::entry_safety::RotaryPrecision::Float32,
        target: &target,
        protected: &solids,
        body: &body,
        machine: &machine,
    };
    let cancel = CancelToken::new();
    let ctx = JobContext::new(&cancel, &NoProgress);
    let high = |x| ToolPose {
        tip: Point3::new(x, 0.0, 30.0),
        ..pose(x)
    };
    let mut budget = 100_000;
    assert!(
        proof
            .verify(high(-4.0), high(4.0), 0.0, 0.01, &mut budget, &ctx)
            .is_ok()
    );
    // A later, lower reverse leg reaches the protected solid.
    assert!(
        proof
            .verify(pose(4.0), pose(-4.0), 0.0, 0.01, &mut budget, &ctx)
            .is_err()
    );
    // The tool also cannot be stationed at an unsafe reversal, or connect
    // through that solid after a partial final leg.
    assert!(
        proof
            .verify(pose(0.137), pose(0.137), 0.0, 0.01, &mut budget, &ctx)
            .is_err()
    );
    assert!(
        proof
            .verify(pose(-2.0), pose(2.0), 0.0, 0.01, &mut budget, &ctx)
            .is_err()
    );
}

#[test]
fn req_op_11_req_set_09_complete_shank_envelope_and_machine_travel_are_checked() {
    use openchuck_types::tool::profile::{ProfilePoint, ProfileSegment, ToolProfile};
    let target = MeshSurface::new(&box_at(20.0, 1.0).mesh(8)).unwrap();
    let cutter = support::tool(ToolShape::FlatEnd, 1.0).profile().unwrap();
    let body = ToolProfile {
        segments: vec![
            ProfileSegment::Line {
                to: ProfilePoint::new(0.5, 0.0),
            },
            ProfileSegment::Line {
                to: ProfilePoint::new(0.5, 50.0),
            },
        ],
    }
    .validate()
    .unwrap();
    let mut machine = MachineProfile::example_x_rotary();
    machine.rotary_origin = Point3::ORIGIN;
    let solids = [Solid::cuboid(Aabb {
        min: Point3::new(-0.1, -1.0, 40.0),
        max: Point3::new(0.1, 1.0, 41.0),
    })
    .unwrap()];
    let cancel = CancelToken::new();
    let ctx = JobContext::new(&cancel, &NoProgress);
    let cutter_only = EntrySafety {
        rotary_precision: openchuck_path::entry_safety::RotaryPrecision::Float32,
        target: &target,
        protected: &solids,
        body: &cutter,
        machine: &machine,
    };
    assert!(
        cutter_only
            .verify(pose(-4.0), pose(4.0), 0.0, 0.01, &mut 100_000, &ctx)
            .is_ok()
    );
    let complete = EntrySafety {
        rotary_precision: openchuck_path::entry_safety::RotaryPrecision::Float32,
        target: &target,
        protected: &solids,
        body: &body,
        machine: &machine,
    };
    assert!(
        complete
            .verify(pose(-4.0), pose(4.0), 0.0, 0.01, &mut 100_000, &ctx)
            .is_err()
    );
    let bounded = EntrySafety {
        rotary_precision: openchuck_path::entry_safety::RotaryPrecision::Float32,
        target: &target,
        protected: &[],
        body: &body,
        machine: &machine,
    };
    assert!(
        bounded
            .verify(pose(-4.0), pose(1000.0), 0.0, 0.01, &mut 100_000, &ctx)
            .is_err()
    );
}

#[test]
fn req_op_11_req_set_09_sweep_reserves_controller_rounding_clearance() {
    let target = MeshSurface::new(&box_at(20.0, 1.0).mesh(8)).unwrap();
    let body = support::tool(ToolShape::FlatEnd, 1.0).profile().unwrap();
    let mut machine = MachineProfile::example_x_rotary();
    machine.rotary_origin = Point3::ORIGIN;
    // The raw endpoint's radius clears this box by 0.0002 mm. Writing
    // X-1.0004 to three decimals moves it into the box by 0.0002 mm.
    let solids = [Solid::cuboid(Aabb {
        min: Point3::new(-0.5002, -1.0, 9.0),
        max: Point3::new(-0.5001, 1.0, 11.0),
    })
    .unwrap()];
    let proof = EntrySafety {
        rotary_precision: openchuck_path::entry_safety::RotaryPrecision::Float32,
        target: &target,
        protected: &solids,
        body: &body,
        machine: &machine,
    };
    let cancel = CancelToken::new();
    let ctx = JobContext::new(&cancel, &NoProgress);
    assert!(
        proof
            .verify(pose(-2.0), pose(-1.0004), 0.0, 0.01, &mut 100_000, &ctx)
            .is_err()
    );
}

#[test]
fn req_gc_09_req_op_11_large_z1_counters_reserve_firmware_precision() {
    use openchuck_types::machine::RotaryInFeed;
    let target = MeshSurface::new(
        &Solid::cuboid(Aabb {
            min: Point3::new(-5.0, -1.0, 0.0),
            max: Point3::new(5.0, 1.0, 9.9),
        })
        .unwrap()
        .mesh(8),
    )
    .unwrap();
    let body = support::tool(ToolShape::FlatEnd, 1.0).profile().unwrap();
    let mut machine = MachineProfile::example_x_rotary();
    machine.rotary_origin = Point3::ORIGIN;
    machine.max_feeds.rotary_in_feed = RotaryInFeed::Makera;
    let proof = EntrySafety {
        rotary_precision: openchuck_path::entry_safety::RotaryPrecision::Float32,
        target: &target,
        protected: &[],
        body: &body,
        machine: &machine,
    };
    let cancel = CancelToken::new();
    let ctx = JobContext::new(&cancel, &NoProgress);
    assert!(
        proof
            .verify(pose(-1.0), pose(1.0), 0.0, 0.01, &mut 100_000, &ctx)
            .is_ok()
    );
    let large = |x| ToolPose {
        a: Degrees(360_000_000.0),
        ..pose(x)
    };
    assert!(
        proof
            .verify(large(-1.0), large(1.0), 0.0, 0.01, &mut 100_000, &ctx)
            .is_err(),
        "the float controller's uncertainty cannot certify this narrow clearance at a huge counter"
    );
    let precise = EntrySafety {
        rotary_precision: openchuck_path::entry_safety::RotaryPrecision::Decimal,
        ..proof
    };
    assert!(
        precise
            .verify(large(-1.0), large(1.0), 0.0, 0.01, &mut 100_000, &ctx)
            .is_ok()
    );
}

#[test]
fn req_op_11_req_op_12_req_nfr_04_adaptive_certificate_encloses_the_whole_sweep_with_leave_above_tolerance()
 {
    let target = MeshSurface::new(&box_at(0.0, 1.0).mesh(8)).unwrap();
    let body = support::tool(ToolShape::FlatEnd, 1.0).profile().unwrap();
    let mut machine = MachineProfile::example_x_rotary();
    machine.rotary_origin = Point3::ORIGIN;
    let proof = EntrySafety {
        rotary_precision: openchuck_path::entry_safety::RotaryPrecision::Float32,
        target: &target,
        protected: &[],
        body: &body,
        machine: &machine,
    };
    let cancel = CancelToken::new();
    let ctx = JobContext::new(&cancel, &NoProgress);
    // A single inflated certificate covers all 8 mm, including the target
    // between the endpoints. Positive leave is 50 times the tolerance.
    let mut budget = 1;
    proof
        .verify(pose(-4.0), pose(4.0), 0.5, 0.01, &mut budget, &ctx)
        .unwrap();
    assert_eq!(budget, 0);
}

#[test]
fn req_op_12_req_nfr_04_inconclusive_coarse_sweep_refines_without_spending_the_uniform_grid() {
    let target = MeshSurface::new(&box_at(0.0, 9.9).mesh(8)).unwrap();
    let body = support::tool(ToolShape::FlatEnd, 1.0).profile().unwrap();
    let mut machine = MachineProfile::example_x_rotary();
    machine.rotary_origin = Point3::ORIGIN;
    let proof = EntrySafety {
        rotary_precision: openchuck_path::entry_safety::RotaryPrecision::Float32,
        target: &target,
        protected: &[],
        body: &body,
        machine: &machine,
    };
    let cancel = CancelToken::new();
    let ctx = JobContext::new(&cancel, &NoProgress);
    let mut budget = 512;
    proof
        .verify(pose(-4.0), pose(4.0), 0.0, 0.01, &mut budget, &ctx)
        .unwrap();
    assert!(
        budget < 511,
        "the whole-interval enclosure must first be inconclusive"
    );
}

#[test]
fn req_op_12_req_nfr_07_unsuccessful_adaptive_attempts_are_bounded() {
    let target = MeshSurface::new(&box_at(0.0, 11.0).mesh(8)).unwrap();
    let body = support::tool(ToolShape::FlatEnd, 1.0).profile().unwrap();
    let mut machine = MachineProfile::example_x_rotary();
    machine.rotary_origin = Point3::ORIGIN;
    let proof = EntrySafety {
        rotary_precision: openchuck_path::entry_safety::RotaryPrecision::Float32,
        target: &target,
        protected: &[],
        body: &body,
        machine: &machine,
    };
    let cancel = CancelToken::new();
    let ctx = JobContext::new(&cancel, &NoProgress);
    let mut budget = 1;
    let error = proof
        .verify(pose(-4.0), pose(4.0), 0.0, 0.01, &mut budget, &ctx)
        .unwrap_err();
    assert_eq!(
        budget, 0,
        "even an inconclusive certificate consumes an attempt"
    );
    assert!(
        error
            .to_string()
            .contains("the clearance check reached its limit of 100000 checks for one entry"),
        "{error}"
    );
}
