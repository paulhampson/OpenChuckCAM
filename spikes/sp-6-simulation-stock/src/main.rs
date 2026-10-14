// SPDX-License-Identifier: AGPL-3.0-or-later
//! Prints the SP-6 report (markdown) to stdout: accuracy, time and memory of
//! the three stock candidates on the turned test parts, a tool-type sweep
//! timing, and thread scaling. `cargo run --release > out/report.md`.

use std::time::Instant;

use sp6_simulation_stock::dexel::DexelStock;
use sp6_simulation_stock::par;
use sp6_simulation_stock::parts::{self, LEN, Part, RS};
use sp6_simulation_stock::profile::{Profile, Pt, Segment};
use sp6_simulation_stock::rotary::RotaryGrid;
use sp6_simulation_stock::shapes;
use sp6_simulation_stock::stock::{Pose, StockModel};
use sp6_simulation_stock::voxel::VoxelStock;

fn models(cell: f64) -> Vec<Box<dyn StockModel>> {
    vec![
        Box::new(RotaryGrid::new(RS, 0.0, LEN, cell)),
        Box::new(DexelStock::new(RS, 0.0, LEN, cell)),
        Box::new(VoxelStock::new(RS, 0.0, LEN, cell)),
    ]
}

fn radial_error(m: &dyn StockModel, part: &Part) -> f64 {
    let mut worst: f64 = 0.0;
    for ix in 0..60 {
        let x = part.xa + 0.7 + (part.xb - part.xa - 1.4) * ix as f64 / 59.0 + 0.0137;
        for it in 0..73 {
            let th = (it as f64 * 4.9 + 1.3).to_radians();
            let (st, ct) = th.sin_cos();
            let start = RS + 1.0;
            let hit = m
                .ray_first_hit([x, start * ct, start * st], [0.0, -ct, -st], start)
                .map(|t| start - t);
            worst = worst.max((hit.unwrap_or(0.0) - (part.r)(x)).abs());
        }
    }
    worst
}

fn custom_ball_two_arcs() -> Profile {
    // A 4 mm ball typed as two quarter-turn... one arc to the equator, then
    // the side: checks that a custom profile sweeps like the standard tool.
    Profile::new(&[
        Segment::Arc {
            to: Pt::new(2.0, 2.0),
            centre: Pt::new(0.0, 2.0),
        },
        Segment::Line {
            to: Pt::new(2.0, 30.0),
        },
    ])
    .expect("valid")
}

fn main() {
    let threads = par::threads();
    println!("# SP-6 report\n");
    println!("Release build, {threads} threads available. Stock Ø{} x {} mm.\n", 2.0 * RS, LEN);

    let ball = shapes::ball(4.0, 30.0).expect("ball");
    println!("## 1. Turned parts (ball Ø4, 0.25 mm stations, 1 degree of A)\n");
    println!("| Part | Cell | Model | Volume error mm3 | Error bound mm3 | Worst radial error mm | Time s ({threads} threads) | Memory MB |");
    println!("| --- | --- | --- | --- | --- | --- | --- | --- |");
    for part in [parts::cylinder(), parts::cone(), parts::sphere()] {
        let poses = parts::ball_turning_poses(&part, 2.0, 0.25, 1.0);
        let area = part.area();
        for cell in [0.2, 0.1, 0.05] {
            for mut m in models(cell) {
                let t = Instant::now();
                m.apply(&ball, &poses, threads);
                let secs = t.elapsed().as_secs_f64();
                let v = m.region_volume([part.xa, -RS - 1.0, -RS - 1.0], [part.xb, RS + 1.0, RS + 1.0]);
                println!(
                    "| {} ({} poses) | {cell} | {} | {:+.1} | {:.1} | {:.4} | {secs:.1} | {:.1} |",
                    part.name,
                    poses.len(),
                    m.name(),
                    v - part.volume,
                    area * (cell / 2.0 + 0.006),
                    radial_error(m.as_ref(), &part),
                    m.memory_bytes() as f64 / 1e6
                );
            }
        }
    }

    println!("\n## 2. Straight pass by tool type (A = 0, tip 3 mm into the cylinder, 20 mm long, 0.05 mm poses)\n");
    println!("Reference: quadrature of the plan view with the profile's `h(r)`, 0.01 mm. Removed volume in mm3, then microseconds per pose on one thread.\n");
    println!("| Tool | Reference | Model | Removed (0.1 cell) | Error | us per pose |");
    println!("| --- | --- | --- | --- | --- | --- |");
    let tools: Vec<(&str, Profile)> = vec![
        ("flat Ø6", shapes::flat(6.0, 30.0).expect("flat")),
        ("ball Ø6", shapes::ball(6.0, 30.0).expect("ball")),
        ("V-bit 90", shapes::v_bit(6.0, 90.0, 0.0, 30.0).expect("v")),
        ("custom ball (2 segments)", custom_ball_two_arcs()),
    ];
    let z0 = 22.0;
    let n = 400;
    let poses: Vec<Pose> = (0..=n)
        .map(|k| Pose {
            tip: [10.0 + 20.0 * k as f64 / n as f64, 0.0, z0],
            a: 0.0,
        })
        .collect();
    for (name, tool) in &tools {
        let reference = parts::groove_volume(tool, z0, 10.0, 30.0, 0.01);
        for mut m in models(0.1) {
            let before = m.volume();
            let t = Instant::now();
            m.apply(tool, &poses, 1);
            let us = t.elapsed().as_secs_f64() * 1e6 / poses.len() as f64;
            let removed = before - m.volume();
            println!(
                "| {name} | {reference:.1} | {} | {removed:.1} | {:+.1} | {us:.0} |",
                m.name(),
                removed - reference
            );
        }
    }

    println!("\n## 3. Reference job at the default 0.1 mm cell: cone, 52,200 poses\n");
    println!("| Model | 1 thread s | 2 threads s | 4 threads s | Hash equal at 1, 2, 4 |");
    println!("| --- | --- | --- | --- | --- |");
    let part = parts::cone();
    let poses = parts::ball_turning_poses(&part, 2.0, 0.25, 1.0);
    for k in 0..3 {
        let mut times = Vec::new();
        let mut hashes = Vec::new();
        let mut name = "";
        for t in [1, 2, 4] {
            let mut ms = models(0.1);
            let m = &mut ms[k];
            let start = Instant::now();
            m.apply(&ball, &poses, t);
            times.push(start.elapsed().as_secs_f64());
            hashes.push(m.state_hash());
            name = m.name();
        }
        println!(
            "| {name} | {:.1} | {:.1} | {:.1} | {} |",
            times[0],
            times[1],
            times[2],
            hashes[0] == hashes[1] && hashes[1] == hashes[2]
        );
    }

    println!("\n## 4. Memory at the default cell, untouched stock\n");
    println!("| Model | 0.2 MB | 0.1 MB | 0.05 MB |");
    println!("| --- | --- | --- | --- |");
    for k in 0..3 {
        let row: Vec<String> = [0.2, 0.1, 0.05]
            .iter()
            .map(|&c| format!("{:.1}", models(c)[k].memory_bytes() as f64 / 1e6))
            .collect();
        println!("| {} | {} |", models(0.1)[k].name(), row.join(" | "));
    }
}
