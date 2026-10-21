// SPDX-License-Identifier: AGPL-3.0-or-later
//! Regenerates a project and reports its toolpath as the viewport draws it
//! (issue #200; decision records 0166 and 0167): moves, runs of one class,
//! points at full detail and at each level of detail. With an output file it
//! also writes the runs for `openchuck-viewport`'s `path_frames` example
//! (`--runs FILE`), so a real project's path can be measured on any adapter,
//! at full detail or at the level of detail `LEVEL` (0 to 6), as a zoomed-out
//! view draws it.
//!
//! ```text
//! cargo run --release -p openchuck-app --example path_runs -- PROJECT [RUNS_FILE [LEVEL]]
//! ```
//!
//! The file is text: a line `run CLASS COUNT` (`rapid`, `link`, `entry` or
//! `cut`) and then `COUNT` lines `X Y Z` in millimetres, in the part's frame.

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use openchuck_app::pathview::{LEVEL_TOLERANCES, PathClass, ToolpathDisplay};
use openchuck_app::playback::Timeline;
use openchuck_app::{Application, Threads};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let usage = "usage: path_runs PROJECT [RUNS_FILE [LEVEL]]";
    let project = PathBuf::from(args.next().ok_or(usage)?);
    let out = args.next().map(PathBuf::from);
    let level: usize = args.next().map_or(Ok(0), |l| l.parse())?;

    let app = Application::open(&project, Threads::All)?;
    let started = Instant::now();
    let run = Arc::new(app.regenerate()?);
    let regenerated = started.elapsed();
    let moves: usize = run.toolpath.groups.iter().map(|g| g.segments.len()).sum();
    let timeline = Timeline::new(app.project(), Arc::clone(&run))?;
    let display = ToolpathDisplay::build(&timeline);
    let all = |_: usize| true;

    println!("project:      {}", project.display());
    println!("regenerated:  {:.1} s", regenerated.as_secs_f64());
    println!("groups:       {}", run.toolpath.groups.len());
    println!("moves:        {moves}");
    println!("runs:         {}", display.runs().len());
    for class in PathClass::ALL {
        let runs = display.runs().iter().filter(|r| r.class == class);
        let (count, points) = runs.fold((0, 0), |(c, p), r| (c + 1, p + r.points.len()));
        println!("  {:<10} {count} runs, {points} points", class.name());
    }
    println!("points:       {} at full detail", display.full_points(&all));
    for level in 1..LEVEL_TOLERANCES.len() {
        let lines = display.lines_for(&all, usize::MAX, level);
        println!(
            "  level {level} ({} mm): {} points",
            LEVEL_TOLERANCES.get(level).copied().unwrap_or(0.0),
            lines.points
        );
    }

    if let Some(out) = out {
        let mut file = std::io::BufWriter::new(std::fs::File::create(&out)?);
        for r in display.lines_for(&all, usize::MAX, level).lines {
            writeln!(
                file,
                "run {} {}",
                r.class.name().to_lowercase(),
                r.points.len()
            )?;
            for p in &r.points {
                writeln!(file, "{} {} {}", p.x, p.y, p.z)?;
            }
        }
        file.flush()?;
        println!("runs written: {} (level {level})", out.display());
    }
    Ok(())
}
