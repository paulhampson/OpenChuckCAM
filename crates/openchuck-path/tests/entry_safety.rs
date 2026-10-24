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
