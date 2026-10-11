// SPDX-License-Identifier: AGPL-3.0-or-later
//! Maps a wrapped path on a cylinder to machine XYZA and back with the shared
//! kinematics. Run with `cargo run -p openchuck-types --example kinematics`.

#![allow(clippy::print_stdout)]

use openchuck_types::geometry::Point3;
use openchuck_types::kinematics::{MachinePosition, machine_xyz, tool_tip_in_model};
use openchuck_types::machine::MachineProfile;
use openchuck_types::units::Degrees;

fn main() {
    let profile = MachineProfile::example_x_rotary();
    let radius = 15.0;
    println!("model tip (x, y, z) -> machine (X, Y, Z, A)");
    for step in 0..=8 {
        let a = Degrees(f64::from(step) * 45.0);
        // A point on top of the cylinder in the machine is, in the model frame,
        // at polar angle 90 - A degrees.
        let angle = (90.0 - a.0).to_radians();
        let tip = Point3::new(f64::from(step), radius * angle.cos(), radius * angle.sin());
        let xyz = machine_xyz(&profile, tip, a);
        let back = tool_tip_in_model(&profile, &MachinePosition { xyz, a });
        println!(
            "({:7.3}, {:7.3}, {:7.3}) -> ({:7.3}, {:7.3}, {:7.3}, {:6.1})  round trip {:.1e} mm",
            tip.x,
            tip.y,
            tip.z,
            xyz.x,
            xyz.y,
            xyz.z,
            a.0,
            back.distance(tip)
        );
    }
}
