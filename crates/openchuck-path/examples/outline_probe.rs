// SPDX-License-Identifier: AGPL-3.0-or-later
//! Temporary probe (lane STEPALL-X3): time exact outline drops on the
//! testkit's banded cylinder.
#![allow(clippy::unwrap_used, clippy::print_stdout, clippy::cast_precision_loss)]
use std::time::Instant;

use openchuck_path::{BrepSurface, SurfaceSource, ToolLine};
use openchuck_types::geometry::{Point3, UnitVec3, Vec3};
use openchuck_types::tool::profile::{ProfilePoint, ProfileSegment, ToolProfile};

fn main() {
    let model = openchuck_testkit::brep::banded_cylinder(10.0, 60.0, &[20.0, 40.0]);
    let s = BrepSurface::new(&model).unwrap();
    let a = 5.0_f64.to_radians();
    let (rt, r) = (0.25, 1.5);
    let (tr, tz) = (rt * a.cos(), rt * (1.0 - a.sin()));
    let taper = ToolProfile {
        segments: vec![
            ProfileSegment::Arc { to: ProfilePoint::new(tr, tz), centre: ProfilePoint::new(0.0, rt) },
            ProfileSegment::Line { to: ProfilePoint::new(r, tz + (r - tr) / a.tan()) },
        ],
    }
    .validate()
    .unwrap();
    let flat = ToolProfile {
        segments: vec![
            ProfileSegment::Line { to: ProfilePoint::new(1.0, 0.0) },
            ProfileSegment::Line { to: ProfilePoint::new(1.0, 4.0) },
        ],
    }
    .validate()
    .unwrap();
    for (name, p) in [("taper", &taper), ("flat", &flat)] {
        let (mut te, mut tm, mut n, mut looked, mut raised) = (0.0, 0.0, 0, 0usize, 0);
        let mut worst = (0usize, 0.0, 0.0);
        let mut times = Vec::new();
        let count: usize = std::env::var("PROBE_N").ok().and_then(|v| v.parse().ok()).unwrap_or(400);
        for i in 0..count {
            let x = 5.0 + 50.0 * (i as f64) / count as f64;
            let ang = (i as f64 * 7.3_f64).to_radians();
            let line = ToolLine {
                origin: Point3::new(x, 0.0, 0.0),
                axis: UnitVec3::new(Vec3::new(0.0, ang.cos(), ang.sin())).unwrap(),
            };
            let t = Instant::now();
            let Some(e) = s.drop_exact(p, &line) else { println!("NONE {name} {i}"); continue };
            let dt = t.elapsed().as_secs_f64();
            te += dt;
            times.push((dt, e.looked));
            let t = Instant::now();
            let _m = s.drop_tool(p, &line).unwrap();
            tm += t.elapsed().as_secs_f64();
            n += 1;
            looked += e.looked;
            if e.raised > 0.0 {
                raised += 1;
            }
            if e.looked > worst.0 {
                worst = (e.looked, x, dt);
            }
        }
        let st: Vec<u64> = openchuck_path::brep::outline::STATS.iter().map(|c| c.swap(0, std::sync::atomic::Ordering::Relaxed)).collect();
        println!("  per drop: top {} full {} dual_at {} segs {} climb {} edge {}", st[0] / 800, st[1] / 800, st[2] / 800, st[3] / 800, st[4] / 800, st[5] / 800);
        times.sort_by(|a, b| a.0.total_cmp(&b.0));
        println!("  median {:?} p90 {:?} p99 {:?}", times[times.len() / 2], times[times.len() * 9 / 10], times[times.len() * 99 / 100]);
        println!(
            "{name}: exact {:.1} us, mesh {:.1} us, looked {:.0}, raised {raised}, worst {worst:?}",
            te / n as f64 * 1e6,
            tm / n as f64 * 1e6,
            looked as f64 / n as f64
        );
    }
}
