// SPDX-License-Identifier: AGPL-3.0-or-later
//! Where material stays after a project's operations (lane RESTCORNERS,
//! refs issue #508; OP-09, SIM-05): the project is generated and simulated,
//! and the setup's counted deviation samples are grouped by X, in 1 mm bins,
//! and by the kind of surface (facing along the rotary axis, or round it).
//! For each group it prints the most material left, where along the part
//! and at what radius, and how many samples have more than the threshold.
//! Numbers only, never geometry files, so it can run on a private project.
//!
//! ```text
//! cargo run --release -p openchuck-app --example rest_corners -- PROJECT [THRESHOLD]
//! ```

#![allow(clippy::print_stdout, clippy::print_stderr, clippy::unwrap_used)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use openchuck_app::{Application, Threads};

/// One group's figures.
#[derive(Default, Clone, Copy)]
struct Group {
    worst: f64,
    worst_x: f64,
    worst_radius: f64,
    above: usize,
    samples: usize,
}

impl Group {
    fn add(&mut self, value: f64, x: f64, radius: f64, threshold: f64) {
        self.samples += 1;
        if value > threshold {
            self.above += 1;
        }
        if value > self.worst {
            self.worst = value;
            self.worst_x = x;
            self.worst_radius = radius;
        }
    }

    fn show(&self) -> String {
        format!(
            "{:7.3} ({:7.3},{:6.3}) {:6}/{:6}",
            self.worst, self.worst_x, self.worst_radius, self.above, self.samples
        )
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let project = PathBuf::from(
        args.next()
            .ok_or("usage: rest_corners PROJECT [THRESHOLD]")?,
    );
    let threshold: f64 = args.next().map(|t| t.parse()).transpose()?.unwrap_or(0.05);
    let app = Application::open(&project, Threads::All)?;
    let run = app.simulate()?;
    let report = run.simulation.ok_or("the run gave no simulation")?;
    for setup in &report.setups {
        println!("setup '{}'", setup.setup);
        for operation in &setup.operations {
            println!("  {operation:?}");
        }
        let deviation = setup
            .deviation
            .as_ref()
            .ok_or("the setup has no comparison")?;
        // Per X bin: (facing along the axis, round the axis).
        let mut bins: BTreeMap<i64, (Group, Group)> = BTreeMap::new();
        for p in deviation.points.iter().filter(|p| p.counts()) {
            let radius = p.point.y.hypot(p.point.z);
            let bin = bins.entry(p.point.x.floor() as i64).or_default();
            let group = if p.normal.x.abs() > 0.7 {
                &mut bin.0
            } else {
                &mut bin.1
            };
            group.add(p.value, p.point.x, radius, threshold);
        }
        println!(
            "X bin | facing along the axis: most left (at X, radius) above {threshold}/samples \
             | round the axis: the same"
        );
        for (x, (along, round)) in &bins {
            println!("{x:5} | {} | {}", along.show(), round.show());
        }
    }
    Ok(())
}
