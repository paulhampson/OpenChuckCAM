// SPDX-License-Identifier: AGPL-3.0-or-later
//! Times the stock simulation alone (lane SIMSPEED, refs issue #382; SIM-04,
//! NFR-04, NFR-06): the project is generated once, then each group of its
//! toolpath is simulated again from the setup's stock at each thread count
//! asked for, and the time of every group and the stock's state hash are
//! printed. The hash must be the same at every thread count (NFR-06).
//!
//! ```text
//! cargo run --release -p openchuck-app --example sim_times -- PROJECT|PART.stl [THREADS,...] [REPEATS]
//! ```
//!
//! Given an STL file it builds the carving project of `playback_times`
//! around it in a temporary folder. Only counts, times and hashes are
//! printed, never geometry.

#![allow(clippy::print_stdout, clippy::print_stderr, clippy::unwrap_used)]

use std::path::PathBuf;
use std::time::Instant;

use openchuck_app::{Application, Threads};
use openchuck_sim::{DexelStock, Simulator};
use openchuck_types::job::{CancelToken, JobContext, NoProgress};

#[path = "support/carving.rs"]
#[allow(dead_code)]
mod carving;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let usage = "usage: sim_times PROJECT|PART.stl [THREADS,...] [REPEATS]";
    let input = PathBuf::from(args.next().ok_or(usage)?);
    let counts: Vec<usize> = match args.next() {
        Some(list) => list
            .split(',')
            .map(str::parse)
            .collect::<Result<_, _>>()?,
        None => vec![1, 2, 4],
    };
    let repeats: usize = args.next().map(|r| r.parse()).transpose()?.unwrap_or(1);
    let temporary = std::env::temp_dir().join(format!("sim-times-{}", std::process::id()));
    let app = if input
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("stl"))
    {
        carving::carving_project(&input, &temporary, Threads::All)?
    } else {
        Application::open(&input, Threads::All)?
    };
    let started = Instant::now();
    let run = app.simulate()?;
    println!(
        "generate and simulate: {:.1} s",
        started.elapsed().as_secs_f64()
    );
    let project = app.project();
    let cell = project.simulation().cell.0;
    let machine = project.machine().ok_or("the project has no machine")?;
    let (cancel, progress) = (CancelToken::new(), NoProgress);
    let context = JobContext::new(&cancel, &progress);
    for threads in counts {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()?;
        for repeat in 0..repeats {
            pool.install(|| -> Result<(), Box<dyn std::error::Error>> {
                // One simulator per setup, in order of first use.
                let mut setups: Vec<(_, Simulator)> = Vec::new();
                let mut whole = 0.0;
                for (g, (path, info)) in run.toolpath.groups.iter().zip(&run.groups).enumerate() {
                    let at = match setups.iter().position(|(s, _)| *s == info.setup) {
                        Some(at) => at,
                        None => {
                            let setup = project.setup(info.setup).ok_or("no setup")?;
                            let stock = setup.stock.as_ref().ok_or("no stock")?;
                            let stock = DexelStock::from_stock(stock, Some(info.part_bounds), cell)?;
                            setups.push((info.setup, Simulator::without_checkpoints(stock, machine)));
                            setups.len() - 1
                        }
                    };
                    let simulator = &mut setups.get_mut(at).ok_or("no simulator")?.1;
                    let started = Instant::now();
                    let result = simulator.run_group(path, &info.tool, &context)?;
                    let seconds = started.elapsed().as_secs_f64();
                    whole += seconds;
                    println!(
                        "threads {threads} run {repeat} group {g}: {seconds:.2} s, {} moves, {} poses, hash {:016x}",
                        result.moves,
                        result.poses,
                        simulator.stock().state_hash()
                    );
                }
                println!("threads {threads} run {repeat} all groups: {whole:.2} s");
                Ok(())
            })?;
        }
    }
    let _ = std::fs::remove_dir_all(&temporary);
    Ok(())
}
