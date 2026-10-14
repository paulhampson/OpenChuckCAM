// SPDX-License-Identifier: AGPL-3.0-or-later
//! The slice 7 budget (decision record 0022 item 4): the SP-6 reference job
//! (a 16.7 degree cone turned out of Ø50 x 40 stock with a Ø4 ball at 0.25 mm
//! stations, one full turn of A in 1 degree steps at each, 52,200 moves) at
//! the default 0.1 mm cell finishes in no more than 30 s on 4 threads and
//! 120 s on one, in no more than 256 MB.
//!
//! Timing needs an optimised build and a quiet machine, so the test is
//! ignored by default:
//! `cargo test --release -p openchuck-sim --test budget -- --ignored --nocapture`.

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod support;

use std::time::Instant;

use openchuck_sim::{DexelStock, Simulator};
use openchuck_types::ir::MoveKind;
use openchuck_types::job::{CancelToken, JobContext, NoProgress};
use openchuck_types::tool::ToolShape;

use support::*;

#[test]
#[ignore = "timing: run in release by hand (decision record 0023)"]
fn req_sim_03_req_nfr_04_reference_job_meets_the_budget() {
    let m = machine();
    let mut t = tool(ToolShape::BallEnd, 4.0);
    t.flute_length = openchuck_types::units::Millimetres(30.0);
    let profile = t.profile().unwrap();
    let poses = turning_poses(&m, &ConeT(8.0, 0.3), 2.0, (2.0, 38.0), 0.25, 1.0);
    assert_eq!(poses.len(), 52_200);
    let g = group("cone", &poses, MoveKind::Cutting);
    let mut times = Vec::new();
    for threads in [4, 1] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        let stock = DexelStock::from_stock(&cylinder_stock(50.0, 0.0, 40.0), None, 0.1).unwrap();
        let start = Instant::now();
        let (bytes, poses, hash) = pool.install(|| {
            let cancel = CancelToken::new();
            let ctx = JobContext::new(&cancel, &NoProgress);
            let mut sim = Simulator::new(stock, &m, None);
            let r = sim.run_group(&g, &profile, &ctx).unwrap();
            (
                sim.stock().memory_bytes() * sim.checkpoints().len().max(1),
                r.poses,
                sim.stock().state_hash(),
            )
        });
        let secs = start.elapsed().as_secs_f64();
        println!(
            "{threads} thread(s): {secs:.1} s, {poses} poses, stock and checkpoints {:.1} MB, hash {hash:016x}",
            bytes as f64 / 1e6
        );
        times.push((threads, secs, bytes, hash));
    }
    assert_eq!(times[0].3, times[1].3);
    assert!(times[0].1 <= 30.0, "4 threads: {} s", times[0].1);
    assert!(times[1].1 <= 120.0, "1 thread: {} s", times[1].1);
    assert!(times[0].2 <= 256 * 1024 * 1024);
}
