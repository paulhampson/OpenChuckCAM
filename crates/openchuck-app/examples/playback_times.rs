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
//! cargo run --release -p openchuck-app --example playback_times -- PROJECT [THREADS [MEMORY_MB]]
//! cargo run --release -p openchuck-app --example playback_times -- PART.stl [THREADS [MEMORY_MB]]
//! ```
//!
//! Given an STL file (such as the carving test piece of issue #382), it
//! builds a project around it in a temporary folder: box stock, rotary
//! roughing with a 3 mm ball, then axial finishing with a 60 degree V-bit
//! with a 0.1 mm tip at a 0.03 mm stepover. Nothing is written beside the
//! STL. Only counts, sizes and times are printed, never geometry.

#![allow(clippy::print_stdout, clippy::print_stderr, clippy::unwrap_used)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use openchuck_app::playback::{Cursor, Timeline};
use openchuck_app::{Application, Command, Threads};
use openchuck_project::id::{OperationId, SetupId, ToolId};
use openchuck_project::{Item, Operation, OrderKey, Placement, Setup, ToolSnapshot};
use openchuck_types::job::{CancelToken, JobContext, NoProgress};
use openchuck_types::machine::MachineProfile;
use openchuck_types::stock::Stock;
use openchuck_types::tool::{ControllerMapping, CuttingData, Tool, ToolShape};
use openchuck_types::transform::TransformStep;
use openchuck_types::units::{Degrees, FeedRate, Millimetres, SpindleSpeed};

/// The peak resident memory of this process so far, in MB (Linux).
fn peak_mb() -> f64 {
    let read = || -> Option<f64> {
        let status = std::fs::read_to_string("/proc/self/status").ok()?;
        let line = status.lines().find(|l| l.starts_with("VmHWM:"))?;
        let kb: f64 = line.split_whitespace().nth(1)?.parse().ok()?;
        Some(kb / 1024.0)
    };
    read().unwrap_or(f64::NAN)
}

fn tool(name: &str, shape: ToolShape, diameter: f64, stepover: f64) -> ToolSnapshot {
    let mut cutting_data = BTreeMap::new();
    cutting_data.insert(
        "oak".to_owned(),
        CuttingData {
            spindle_speed: SpindleSpeed(12000.0),
            feed_per_tooth: Millimetres(0.02),
            plunge_feed: FeedRate(150.0),
            stepover: Millimetres(stepover),
            stepdown: Millimetres(1.0),
        },
    );
    ToolSnapshot {
        id: ToolId::new(),
        order: OrderKey::default(),
        name: name.into(),
        tool: Tool {
            shape,
            diameter: Millimetres(diameter),
            flute_length: Millimetres(10.0),
            overall_length: Millimetres(40.0),
            shank_diameter: Millimetres(diameter.max(3.0)),
            flute_count: 2,
            material: None,
            centre_cutting: None,
        },
        controller: ControllerMapping {
            number: 1,
            length_offset: None,
            diameter_offset: None,
        },
        cutting_data,
    }
}

/// A project around `stl` in `dir`: the carving set-up described above.
fn carving_project(
    stl: &Path,
    dir: &Path,
    threads: Threads,
) -> Result<Application, Box<dyn std::error::Error>> {
    let _ = std::fs::remove_dir_all(dir);
    let mut app = Application::create(dir, "carving", threads)?;
    std::fs::create_dir_all(dir.join("models"))?;
    std::fs::copy(stl, dir.join("models/part.stl"))?;
    let (model, _) = app.add_model(&dir.join("models/part.stl"), None)?;
    app.execute(Command::SetModelTransform {
        model,
        transform: vec![TransformStep::CentreOnRotaryAxis],
    })?;
    app.execute(Command::SetMachine(Some(Box::new(
        MachineProfile::example_x_rotary(),
    ))))?;
    let setup = SetupId::new();
    let mut s = Setup::new(setup, "Setup 1");
    s.part = Some(model);
    s.material = Some("oak".into());
    s.stock = Some(Stock::Cylinder {
        diameter: Millimetres(28.0),
        length: Millimetres(34.0),
        x_start: Millimetres(-2.0),
    });
    let ball = tool("3 mm ball", ToolShape::BallEnd, 3.0, 1.0);
    let vbit = tool(
        "60 degree V-bit",
        ToolShape::VBit {
            included_angle: Degrees(60.0),
            tip_diameter: Millimetres(0.1),
        },
        3.0,
        0.03,
    );
    let operations = [
        ("Roughing", "rotary_roughing", ball.id),
        ("Finishing", "axial_finishing", vbit.id),
    ];
    for item in [Item::Setup(s), Item::Tool(ball), Item::Tool(vbit)] {
        app.execute(Command::Add {
            item,
            placement: Placement::End,
        })?;
    }
    for (name, strategy, tool) in operations {
        app.execute(Command::Add {
            item: Item::Operation(Operation {
                enabled: true,
                id: OperationId::new(),
                order: OrderKey::default(),
                name: name.into(),
                setup,
                strategy: strategy.into(),
                tool: Some(tool),
                selections: Vec::new(),
                boundary: None,
                avoid: Vec::new(),
                check: Vec::new(),
                restriction_options: Default::default(),
                spindle_speed: None,
                material: None,
                parameters: BTreeMap::new(),
            }),
            placement: Placement::End,
        })?;
    }
    Ok(app)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let usage = "usage: playback_times PROJECT|PART.stl [THREADS [MEMORY_MB]]";
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
    let temporary = std::env::temp_dir().join(format!("playback-times-{}", std::process::id()));
    let app = if input
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("stl"))
    {
        carving_project(&input, &temporary, threads)?
    } else {
        Application::open(&input, threads)?
    };
    println!(
        "threads: {threads:?}, simulation cell {} mm",
        app.project().simulation().cell.0
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
        "prepare playback: {:.1} s, {} checkpoints holding {:.0} MB, peak {:.0} MB",
        started.elapsed().as_secs_f64(),
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
    // once per surface. From 30% of the program time, eight surfaces at
    // each speed of the speed list and at "whole program in 60 s".
    println!("speed x | surfaces s (each) | moves per surface (each)");
    for speed in [10.0, 100.0, 1000.0, timeline.total_time() / 60.0] {
        let mut time = 0.3 * timeline.total_time();
        let (mut times, mut steps) = (Vec::new(), Vec::new());
        let mut last = timeline.cursor_at_time(time).done;
        for _ in 0..8 {
            let cursor = timeline.cursor_at_time(time);
            let started = Instant::now();
            drop(app.start_stock_surface(&playback, cursor).wait()?);
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
