// SPDX-License-Identifier: AGPL-3.0-or-later
//! Toolpaths and oracles for the simulation tests. The oracles are closed
//! forms and quadratures written here, sharing no code with the simulator.

#![allow(dead_code, clippy::unwrap_used, clippy::indexing_slicing)]

use openchuck_types::geometry::{Point3, UnitVec3};
use openchuck_types::ir::{MoveKind, PathTags, Segment, ToolPose, ToolpathGroup};
use openchuck_types::kinematics::{
    MachinePosition, machine_xyz, tool_axis_in_model, tool_tip_in_model,
};
use openchuck_types::machine::MachineProfile;
use openchuck_types::stock::Stock;
use openchuck_types::tool::{RevolvedProfile, Tool, ToolShape};
use openchuck_types::units::{Degrees, FeedRate, Millimetres, Seconds};

pub fn machine() -> MachineProfile {
    MachineProfile::example_x_rotary()
}

pub fn tags(op: &str) -> PathTags {
    PathTags {
        operation: op.to_owned(),
        setup: "setup".to_owned(),
        tool: "tool".to_owned(),
    }
}

pub fn tool(shape: ToolShape, diameter: f64) -> Tool {
    Tool {
        shape,
        diameter: Millimetres(diameter),
        flute_length: Millimetres(20.0),
        overall_length: Millimetres(50.0),
        shank_diameter: Millimetres(diameter),
        flute_count: 2,
        material: None,
    }
}

pub fn ball(diameter: f64) -> RevolvedProfile {
    tool(ToolShape::BallEnd, diameter).profile().unwrap()
}

pub fn cylinder_stock(diameter: f64, x_start: f64, length: f64) -> Stock {
    Stock::Cylinder {
        diameter: Millimetres(diameter),
        length: Millimetres(length),
        x_start: Millimetres(x_start),
    }
}

/// The model-frame pose of a tool held still in the machine with its tip at
/// (x, 0, height) above the rotary axis at A = 0, once the part has turned to `a`.
pub fn turned_pose(m: &MachineProfile, x: f64, height: f64, a: f64) -> ToolPose {
    let xyz = machine_xyz(m, Point3::new(x, 0.0, height), Degrees(0.0));
    let tip = tool_tip_in_model(m, &MachinePosition { xyz, a: Degrees(a) });
    ToolPose {
        tip,
        axis: tool_axis_in_model(m, Degrees(a)),
        a: Degrees(a),
    }
}

pub fn segment(kind: MoveKind, end: ToolPose) -> Segment {
    Segment {
        kind,
        end,
        contact_feed: kind.is_fed().then_some(FeedRate(1000.0)),
        duration: Seconds(0.01),
        contact_length: Millimetres(0.0),
    }
}

pub fn group(op: &str, poses: &[ToolPose], kind: MoveKind) -> ToolpathGroup {
    let start = poses[0];
    ToolpathGroup {
        tags: tags(op),
        start,
        segments: poses[1..].iter().map(|p| segment(kind, *p)).collect(),
    }
}

/// A turned surface of revolution about X: radius and slope at x.
pub trait Turned {
    fn radius(&self, x: f64) -> f64;
    fn slope(&self, x: f64) -> f64 {
        let h = 1e-6;
        (self.radius(x + h) - self.radius(x - h)) / (2.0 * h)
    }
}

pub struct Cyl(pub f64);
impl Turned for Cyl {
    fn radius(&self, _x: f64) -> f64 {
        self.0
    }
}

/// Radius r0 at x = 0, changing by `slope` per mm.
pub struct ConeT(pub f64, pub f64);
impl Turned for ConeT {
    fn radius(&self, x: f64) -> f64 {
        self.0 + self.1 * x
    }
}

pub struct Sph(pub f64);
impl Turned for Sph {
    fn radius(&self, x: f64) -> f64 {
        (self.0 * self.0 - x * x).max(0.0).sqrt()
    }
}

/// A ball of radius `rb` turning the surface: the ball centre follows the
/// offset curve; at each surface station from `xa` to `xb` (step `dx`) the
/// part makes one full turn in steps of `da` degrees.
pub fn turning_poses(
    m: &MachineProfile,
    part: &dyn Turned,
    rb: f64,
    (xa, xb): (f64, f64),
    dx: f64,
    da: f64,
) -> Vec<ToolPose> {
    let stations = ((xb - xa) / dx).round() as usize;
    let per_turn = (360.0 / da).round() as usize;
    let mut out = Vec::new();
    for i in 0..=stations {
        let xs = xa + i as f64 * dx;
        let (rho, s) = (part.radius(xs), part.slope(xs));
        let n = (1.0 + s * s).sqrt();
        let (cx, cr) = (xs - rb * s / n, rho + rb / n);
        for k in 0..per_turn {
            let a = 360.0 * i as f64 + k as f64 * da;
            out.push(turned_pose(m, cx, cr - rb, a));
        }
    }
    out
}

/// Simpson's rule.
pub fn integrate(f: impl Fn(f64) -> f64, a: f64, b: f64) -> f64 {
    let n = 2000;
    let h = (b - a) / n as f64;
    let mut s = f(a) + f(b);
    for i in 1..n {
        let x = a + i as f64 * h;
        s += if i % 2 == 1 { 4.0 } else { 2.0 } * f(x);
    }
    s * h / 3.0
}

pub fn unit(x: f64, y: f64, z: f64) -> UnitVec3 {
    UnitVec3::new(openchuck_types::geometry::Vec3::new(x, y, z)).unwrap()
}
