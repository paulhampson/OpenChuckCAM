// SPDX-License-Identifier: AGPL-3.0-or-later
//! Times what the Simulate step waits on before the cut part and the tool
//! move (issue #382; SIM-04, UI-09, NFR-04): generating and simulating the
//! project, preparing the stock playback (the simulation again, with its
//! checkpoints), and then, at points along the run, the stock at a move
//! (the replay from the nearest checkpoint), its mesh and the display
//! surface the viewport is given. While a surface is being made the tool
//! stays where the cut part shows it (issue 333), so the time of one
//! surface is how long the tool stands still.
//!
//! ```text
//! cargo run --release -p openchuck-app --example playback_times -- PROJECT [THREADS]
//! ```

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use openchuck_app::playback::{Cursor, Timeline};
use openchuck_app::{Application, Threads};
use openchuck_types::job::{CancelToken, JobContext, NoProgress};

/// The peak resident memory of this process so far, in MB (Linux).
fn peak_mb() -> Option<f64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|l| l.starts_with("VmHWM:"))?;
    let kb: f64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb / 1024.0)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let usage = "usage: playback_times PROJECT [THREADS]";
    let project = PathBuf::from(args.next().ok_or(usage)?);
    let threads = match args.next() {
        Some(n) => Threads::Exactly(n.parse()?),
        None => Threads::All,
    };
    let app = Application::open(&project, threads)?;
    println!("threads: {threads:?}");

    let started = Instant::now();
    let run = Arc::new(app.simulate()?);
    let moves: usize = run.toolpath.groups.iter().map(|g| g.segments.len()).sum();
    println!(
        "generate and simulate: {:.2} s, {moves} moves in {} groups, peak {:.0} MB",
        started.elapsed().as_secs_f64(),
        run.toolpath.groups.len(),
        peak_mb().unwrap_or(f64::NAN)
    );

    let started = Instant::now();
    let timeline = Arc::new(Timeline::new(app.project(), Arc::clone(&run))?);
    println!(
        "timeline: {:.2} s, {} moves, {:.0} s of machining",
        started.elapsed().as_secs_f64(),
        timeline.move_count(),
        timeline.total_time()
    );

    let started = Instant::now();
    let playback = Arc::new(app.start_stock_playback(Arc::clone(&timeline)).wait()?);
    println!(
        "prepare playback: {:.2} s, {} checkpoints, peak {:.0} MB",
        started.elapsed().as_secs_f64(),
        playback.checkpoints(),
        peak_mb().unwrap_or(f64::NAN)
    );

    let (cancel, progress) = (CancelToken::new(), NoProgress);
    let context = JobContext::new(&cancel, &progress);
    let n = timeline.move_count();
    for share in [0.0, 0.05, 0.3, 0.5, 0.7, 0.95, 1.0] {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let done = ((n as f64) * share) as usize;
        let cursor = Cursor {
            done: done.min(n),
            fraction: 0.5,
        };
        let started = Instant::now();
        let stock = playback.stock_at(cursor, &context)?;
        let replay = started.elapsed().as_secs_f64();
        let started = Instant::now();
        let mesh = openchuck_sim::stock_mesh(&stock);
        let meshing = started.elapsed().as_secs_f64();
        let started = Instant::now();
        let surface = app.start_stock_surface(&playback, cursor).wait()?;
        let whole = started.elapsed().as_secs_f64();
        println!(
            "move {done}: stock {replay:.2} s, mesh {meshing:.2} s ({} triangles), \
             display surface (stock, mesh and colours) {whole:.2} s, stock {:.0} MB",
            mesh.triangles.len(),
            stock.memory_bytes() as f64 / 1e6,
        );
        drop(surface);
    }
    println!("peak {:.0} MB", peak_mb().unwrap_or(f64::NAN));
    Ok(())
}
