// SPDX-License-Identifier: AGPL-3.0-or-later
//! Times what the Simulate step waits on before it shows the cut part and
//! moves the tool (lane SIMPERF, refs issue #382; SIM-04, SIM-05, NFR-04):
//! generating and simulating the project, preparing the stock playback (the
//! simulation again, with its checkpoints), and then, at points along the
//! run, each part of a cut-part frame: the stock replayed from the nearest
//! checkpoint, its mesh, the colours and the picker, and the whole surface
//! job as the viewport asks for it. While a surface is being made the tool
//! stays where the cut part shows it (issue #333), so the time of one
//! surface is how long the tool stands still.
//!
//! ```text
//! cargo run --release -p openchuck-app --example playback_times -- PROJECT [THREADS [MEMORY_MB [on]]]
//! cargo run --release -p openchuck-app --example playback_times -- PART.stl [THREADS [MEMORY_MB [on]]]
//! ```
//!
//! With `on`, playback information is made with toolpath generation
//! (decision record 0333): the run keeps playback's checkpoints within
//! MEMORY_MB, and preparing playback takes them from it.
//!
//! Given an STL file (such as the carving test piece of issue #382), it
//! builds a project around it in a temporary folder: box stock, rotary
//! roughing with a 3 mm ball, then axial finishing with a 60 degree V-bit
//! with a 0.1 mm tip at a 0.03 mm stepover. Nothing is written beside the
//! STL. Only counts, sizes and times are printed, never geometry.

#![allow(clippy::print_stdout, clippy::print_stderr, clippy::unwrap_used)]

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use openchuck_app::playback::{Cursor, Timeline};
use openchuck_app::{Application, Threads};
use openchuck_types::job::{CancelToken, JobContext, NoProgress};

#[path = "support/carving.rs"]
mod carving;

use carving::{carving_project, peak_mb};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let usage = "usage: playback_times PROJECT|PART.stl [THREADS [MEMORY_MB [on]]]";
    let input = PathBuf::from(args.next().ok_or(usage)?);
    let threads = match args.next() {
        Some(n) => Threads::Exactly(n.parse()?),
        None => Threads::All,
    };
    // The memory for playback checkpoints, MB (the user's setting).
    let memory = match args.next() {
        Some(mb) => mb.parse::<usize>()? * 1024 * 1024,
        None => openchuck_app::stockplay::DEFAULT_PLAYBACK_MEMORY,
    };
    let with_generation = args.next().is_some_and(|a| a == "on");
    let temporary = std::env::temp_dir().join(format!("playback-times-{}", std::process::id()));
    let mut app = if input
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("stl"))
    {
        carving_project(&input, &temporary, threads)?
    } else {
        Application::open(&input, threads)?
    };
    if with_generation {
        app.set_playback_with_generation(Some(memory));
    }
    println!(
        "threads: {threads:?}, simulation cell {} mm, playback information with generation: {}",
        app.project().simulation().cell.0,
        if with_generation { "on" } else { "off" }
    );

    let started = Instant::now();
    let run = Arc::new(app.simulate()?);
    let moves: usize = run.toolpath.groups.iter().map(|g| g.segments.len()).sum();
    println!(
        "generate and simulate: {:.1} s, {moves} moves in {} groups, peak {:.0} MB",
        started.elapsed().as_secs_f64(),
        run.toolpath.groups.len(),
        peak_mb()
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
    let playback = Arc::new(
        app.start_stock_playback(Arc::clone(&timeline), memory)
            .wait()?,
    );
    println!(
        "prepare playback: {:.1} s{}, {} checkpoints holding {:.0} MB, peak {:.0} MB",
        started.elapsed().as_secs_f64(),
        if playback.from_the_run() {
            " (from the run)"
        } else {
            ""
        },
        playback.checkpoints(),
        playback.checkpoint_bytes() as f64 / 1e6,
        peak_mb()
    );

    let (cancel, progress) = (CancelToken::new(), NoProgress);
    let context = JobContext::new(&cancel, &progress);
    let n = timeline.move_count();
    // Jumps along the run, as "go to move" asks for them: the whole surface
    // job first (from the nearest checkpoint, or the last frame's stock when
    // that is nearer), then, on the stock it shows, the mesh and the picker
    // again alone, for the split.
    println!("move | surface job s | stock mesh s | triangles | picker alone s | stock MB");
    let shares: &[f64] = if std::env::var_os("SKIP_SEEKS").is_some() {
        &[]
    } else {
        &[0.0, 0.05, 0.3, 0.5, 0.7, 0.95, 1.0]
    };
    for &share in shares {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let done = (((n as f64) * share) as usize).min(n);
        let cursor = Cursor {
            done,
            fraction: 0.5,
        };
        let started = Instant::now();
        let surface = app.start_stock_surface(&playback, cursor).wait()?;
        let whole = started.elapsed().as_secs_f64();
        drop(surface);
        let stock = playback.stock_at(cursor, &context)?;
        let started = Instant::now();
        let mesh = openchuck_sim::stock_mesh(&stock);
        let meshing = started.elapsed().as_secs_f64();
        let started = Instant::now();
        let picker = (!mesh.triangles.is_empty())
            .then(|| openchuck_geom::pick::MeshPicker::new(&mesh))
            .transpose()?;
        let picking = started.elapsed().as_secs_f64();
        drop(picker);
        println!(
            "{done} | {whole:.2} | {meshing:.2} | {} | {picking:.2} | {:.1}",
            mesh.triangles.len(),
            stock.memory_bytes() as f64 / 1e6,
        );
    }
    // Playing forward as the viewport does: a new surface is asked for at
    // the clock's cursor when the last one is finished, so the tool moves
    // once per surface. Frames made while playing have no picker (the
    // owner's decision D5). From 30% of the program time, eight surfaces at
    // each speed of the speed list and at "whole program in 60 s".
    println!("speed x | surfaces s (each) | moves per surface (each)");
    for speed in [10.0, 100.0, 1000.0, timeline.total_time() / 60.0] {
        let mut time = 0.3 * timeline.total_time();
        let (mut times, mut steps) = (Vec::new(), Vec::new());
        let mut last = timeline.cursor_at_time(time).done;
        for _ in 0..8 {
            let cursor = timeline.cursor_at_time(time);
            let started = Instant::now();
            drop(app.start_playing_stock_surface(&playback, cursor).wait()?);
            let took = started.elapsed().as_secs_f64();
            times.push(format!("{took:.2}"));
            steps.push((cursor.done - last).to_string());
            last = cursor.done;
            time = (time + took * speed).min(timeline.total_time());
        }
        println!("{speed:.0} | {} | {}", times.join(" "), steps.join(" "));
    }
    println!("peak {:.0} MB", peak_mb());
    let _ = std::fs::remove_dir_all(&temporary);
    Ok(())
}
