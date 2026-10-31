// SPDX-License-Identifier: AGPL-3.0-or-later
//! Path fitting holds the chord limit along the whole of a move, not only at
//! its middle (OP-12, NFR-05; decision record 0115, closing OI-331).
//!
//! The machine moves in a straight line in X, Y, Z and A between two points
//! of a pass. Bisection tests the straight move against the dropped tool at
//! the middle of the move, and on the owner's blisk that test was blind:
//! where the ball rides up a blade's leaning flank the dropped tool's height
//! along the move is S-shaped, and when the inflection falls on the middle
//! the straight move passes through it while it is 1 mm in the air a quarter
//! of the way along and 1 mm too deep three quarters along, a 0.2 mm gouge
//! into the flank. The fitter now tests the quarter points too, so a move is
//! accepted only when the straight move is within the chord limit at its
//! middle and at the middles of its halves.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing
)]

mod support;
use support::generate;

use std::collections::BTreeMap;
use std::f64::consts::{PI, TAU};

use openchuck_path::helical::parameter;
use openchuck_path::params::names;
use openchuck_path::{GeneratedPath, HelicalFinishing, MeshSurface, ParameterValue, SurfaceSource};
use openchuck_testkit::gouge::{Outline, Pose, max_gouge};
use openchuck_testkit::parts::RadialPart;
use openchuck_types::geometry::{Point3, Vec3};
use openchuck_types::ir::MoveKind;
use openchuck_types::kinematics::{
    MachinePosition, machine_xyz, tool_axis_in_model, tool_tip_in_model,
};
use openchuck_types::machine::MachineProfile;
use openchuck_types::units::Degrees;
use rayon::prelude::*;
use support::{Fixture, ball, blisk, machine, run};

/// A cylinder whose radius follows a cubic ramp over the angles `centre ±
/// half_width`: `radius + height u³` for `u` from -1 to 1, so it is `height`
/// below the radius on one side of the centre and `height` above it on the
/// other, and the same ramp back on the opposite side of the part. Each ramp
/// is point-symmetric about its centre, where its slope is zero: the straight
/// move across it passes through the surface at the centre, and the dropped
/// tool's height along a helix over the ramp is S-shaped with its inflection
/// there.
struct CubicRamp {
    radius: f64,
    length: f64,
    height: f64,
    centre: f64,
    half_width: f64,
}

impl CubicRamp {
    /// The ramp as a fraction of `height`, from -1 to 1 and back.
    fn ramp(&self, angle: f64) -> f64 {
        let hw = self.half_width;
        let d = (angle - self.centre + PI).rem_euclid(TAU) - PI;
        let cubic = |u: f64| u * u * u;
        if d.abs() <= hw {
            cubic(d / hw)
        } else if d > 0.0 {
            if d < PI - hw {
                1.0
            } else {
                -cubic((d - PI) / hw)
            }
        } else if d > hw - PI {
            -1.0
        } else {
            -cubic((d + PI) / hw)
        }
    }
}

impl RadialPart for CubicRamp {
    fn name(&self) -> &'static str {
        "cubic ramp"
    }
    fn x_range(&self) -> (f64, f64) {
        (0.0, self.length)
    }
    fn radius_at(&self, _x: f64, angle: f64) -> f64 {
        self.radius + self.height * self.ramp(angle)
    }
}

fn values(pitch: f64, step: f64) -> BTreeMap<String, ParameterValue> {
    [
        (names::STEPOVER, pitch),
        (names::FEED, 300.0),
        (names::PLUNGE_FEED, 100.0),
        (parameter::MAX_ANGLE_STEP, step),
    ]
    .into_iter()
    .map(|(k, x)| (k.to_owned(), ParameterValue::Number(x)))
    .collect()
}

/// `n - 1` poses inside each cutting move of `path`, on the straight machine
/// move from its start to its end, as the controller runs it and the
/// simulator samples it (decision record 0025 item 3).
fn interior_poses(path: &GeneratedPath, profile: &MachineProfile, n: usize) -> Vec<Pose> {
    let mut prev = path.group.start;
    let mut out = Vec::new();
    for s in &path.group.segments {
        if s.kind == MoveKind::Cutting {
            let m0 = machine_xyz(profile, prev.tip, prev.a);
            let m1 = machine_xyz(profile, s.end.tip, s.end.a);
            for k in 1..n {
                let t = k as f64 / n as f64;
                let a = Degrees(prev.a.0 + t * (s.end.a.0 - prev.a.0));
                let xyz = m0 + (m1 - m0) * t;
                out.push(Pose {
                    tip: tool_tip_in_model(profile, &MachinePosition { xyz, a }),
                    axis: tool_axis_in_model(profile, a).get(),
                });
            }
        }
        prev = s.end;
    }
    out
}

/// The exact cubic ramp with its inflection at the middle of a seed
/// interval: a 72 degree step from A 0 puts seeds at A 0 and 72, where the
/// tool faces the part's angles 90 and 18 (the tool is over +Z at A 0 and A
/// turns the part the other way), so a ramp centred on the angle 54 spans
/// one seed interval, and the ramp back, centred on the angle 234, spans the
/// interval from A 180 to 252. The ramp's ends are gentle (a slope of 0.06),
/// so the dropped tool at the ends is lifted by less than the chord limit
/// and the straight move across a ramp passes within it of the dropped tool
/// at the middle: a test at the middle alone keeps the whole 72 degree move,
/// which cuts 0.19 mm into the ramp a quarter of the way along and leaves
/// as much three quarters along. The independent checker (OP-12;
/// verification plan 4.1) samples the exact surface at the tool between the
/// points of every cutting move: no gouge beyond the tolerance.
#[test]
fn req_op_12_req_nfr_05_a_move_is_within_the_chord_between_its_points_not_only_at_its_middle() {
    let part = CubicRamp {
        radius: 40.0,
        length: 10.0,
        height: 0.5,
        centre: 54.0_f64.to_radians(),
        half_width: 36.0_f64.to_radians(),
    };
    let fixture = Fixture::new(
        MeshSurface::new(&part.mesh(11, 2880)).unwrap(),
        42.0,
        &ball(3.0),
    );
    let path =
        run(|ctx| generate(&HelicalFinishing, &fixture.input(), &values(2.0, 72.0), ctx)).unwrap();
    let outline = Outline::new(&fixture.tool);
    let inside = interior_poses(&path, &machine(), 8);
    assert!(inside.len() > 500, "{} poses", inside.len());
    let g = max_gouge(&outline, &part, &inside, 0.05);
    assert!(
        g.depth <= 0.01,
        "the tool cuts {:.4} mm into the exact surface at {:?} between two points of the path",
        g.depth,
        g.at
    );
    // The points themselves were never the problem: the dropped tool touches.
    let ends: Vec<Pose> = path
        .group
        .segments
        .iter()
        .filter(|s| s.kind == MoveKind::Cutting)
        .map(|s| Pose {
            tip: s.end.tip,
            axis: s.end.axis.get(),
        })
        .collect();
    let g = max_gouge(&outline, &part, &ends, 0.05);
    assert!(g.depth <= 0.002, "at a dropped point: {:.4} mm", g.depth);
}

/// The owner's blisk with lane BM's tool (3 mm ball, 12 mm flutes, 0.3 mm
/// stepover) at a 5 degree step, the run of OI-331: the ball between the
/// points of every cutting move stays out of the mesh to the toolpath
/// tolerance (measured against the mesh itself, which has no exact surface:
/// the distance from the ball's centre to the mesh is at least the ball's
/// radius less the tolerance). Before decision 0115 it cut 0.199 mm into a
/// blade's flank, where the simulation reported 0.177 mm of overcut.
#[test]
fn req_op_12_req_sim_05_the_blisk_helical_pass_cuts_no_deeper_than_the_tolerance_between_its_points()
 {
    let fixture = Fixture::new(MeshSurface::new(&blisk()).unwrap(), 21.0, &ball(3.0));
    let path =
        run(|ctx| generate(&HelicalFinishing, &fixture.input(), &values(0.3, 5.0), ctx)).unwrap();
    let radius = fixture.tool.cutting_radius();
    // The ball, and the flutes above it at 4.5, 7.5 and 10.5 mm.
    let cut = |pose: &Pose| {
        (0..4)
            .map(|k| {
                let centre = pose.tip + pose.axis * (radius + 3.0 * f64::from(k));
                radius - fixture.surface.distance(centre).unwrap()
            })
            .fold(0.0_f64, f64::max)
    };
    let worst = interior_poses(&path, &machine(), 8)
        .par_iter()
        .map(|pose| (cut(pose), *pose))
        .reduce(
            || {
                (
                    0.0,
                    Pose {
                        tip: Point3::ORIGIN,
                        axis: Vec3::ZERO,
                    },
                )
            },
            |a, b| if b.0 > a.0 { b } else { a },
        );
    assert!(
        worst.0 <= 0.01,
        "the tool cuts {:.4} mm into the blisk at tip {:?} between two points of the path",
        worst.0,
        worst.1.tip
    );
}

// ---------------------------------------------------------------------------
// Simultaneous finishing's own fitter (decision record 0036 item 7; OI-348).
// ---------------------------------------------------------------------------

fn simultaneous_values(pitch: f64, step: f64, tilt: f64) -> BTreeMap<String, ParameterValue> {
    use openchuck_path::simultaneous::parameter as sp;
    [
        (names::STEPOVER, pitch),
        (names::FEED, 300.0),
        (names::PLUNGE_FEED, 100.0),
        (sp::MAX_ANGLE_STEP, step),
        (sp::MAX_TILT, tilt),
    ]
    .into_iter()
    .map(|(k, x)| (k.to_owned(), ParameterValue::Number(x)))
    .collect()
}

/// The deepest cut of the tool into the exact `part` between the points of
/// `path`, with how many cutting moves cut deeper than the tolerance.
fn interior_gouge<P: RadialPart + ?Sized + Sync>(
    part: &P,
    tool: &openchuck_types::tool::RevolvedProfile,
    path: &GeneratedPath,
    n: usize,
) -> (f64, usize, usize) {
    let outline = Outline::new(tool);
    let inside = interior_poses(path, &machine(), n);
    let per_move: Vec<f64> = inside
        .par_chunks(n - 1)
        .map(|poses| max_gouge(&outline, part, poses, 0.05).depth)
        .collect();
    let worst = per_move.iter().copied().fold(0.0, f64::max);
    let over = per_move.iter().filter(|d| **d > 0.01).count();
    (worst, over, per_move.len())
}

/// The exact cubic ramp of the test above under simultaneous finishing, with
/// the tool axis on the normal (`max_tilt` 0): on a surface whose normal lies
/// in the plane through the axis the path is the helical one (decision 0036
/// item 1), so the same S-shaped drop along the 72 degree move meets
/// simultaneous finishing's own fitter, which tested the move at its middle
/// alone (decision 0036 item 7; OI-348).
#[test]
fn req_op_12_req_nfr_05_a_simultaneous_move_is_within_the_chord_between_its_points() {
    let part = CubicRamp {
        radius: 40.0,
        length: 10.0,
        height: 0.5,
        centre: 54.0_f64.to_radians(),
        half_width: 36.0_f64.to_radians(),
    };
    let fixture = Fixture::new(
        MeshSurface::new(&part.mesh(11, 2880)).unwrap(),
        42.0,
        &ball(3.0),
    );
    let path = run(|ctx| {
        generate(
            &openchuck_path::SimultaneousFinishing,
            &fixture.input(),
            &simultaneous_values(2.0, 72.0, 0.0),
            ctx,
        )
    })
    .unwrap();
    let (worst, over, moves) = interior_gouge(&part, &fixture.tool, &path, 8);
    eprintln!("ramp, simultaneous: {moves} moves, worst {worst:.4} mm, {over} over 0.01 mm");
    assert!(
        worst <= 0.01,
        "the tool cuts {worst:.4} mm into the exact surface between two points of the path \
         ({over} of {moves} moves over the tolerance)"
    );
}

/// The `lobed` reference job (decision 0036 item 12; `reference-jobs.md`):
/// the exact lobed vase, the 3 mm ball, a 2 mm stepover and a 2 degree step,
/// generated as `openchuck-app/tests/reference_jobs.rs` generates it (the
/// mesh of 201 rings by 720), checked between the points of every cutting
/// move against the exact part.
#[test]
fn req_op_12_req_nfr_05_the_lobed_reference_job_cuts_no_deeper_than_the_tolerance_between_its_points()
 {
    use openchuck_testkit::parts::LobedVase;
    let part = LobedVase {
        base: 10.0,
        wave: 1.5,
        wave_length: 40.0,
        lobe: 1.0,
        lobes: 3,
        length: 20.0,
    };
    let fixture = Fixture::new(
        MeshSurface::new(&part.mesh(201, 720)).unwrap(),
        13.5,
        &ball(3.0),
    );
    let path = run(|ctx| {
        generate(
            &openchuck_path::SimultaneousFinishing,
            &fixture.input(),
            &simultaneous_values(2.0, 2.0, 10.0),
            ctx,
        )
    })
    .unwrap();
    let (worst, over, moves) = interior_gouge(&part, &fixture.tool, &path, 8);
    eprintln!("lobed, simultaneous: {moves} moves, worst {worst:.4} mm, {over} over 0.01 mm");
    assert!(
        worst <= 0.01,
        "the tool cuts {worst:.4} mm into the exact lobed vase between two points of the path \
         ({over} of {moves} moves over the tolerance)"
    );
}
