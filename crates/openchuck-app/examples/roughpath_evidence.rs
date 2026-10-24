// SPDX-License-Identifier: AGPL-3.0-or-later
//! Read-only ROUGHPATH reproduction: assembled motion, playback and both posts.
#![allow(clippy::print_stdout)]
use openchuck_app::{
    Application, PostSelection, Threads,
    playback::{Cursor, Timeline},
};
use std::{path::PathBuf, sync::Arc, time::Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let project = PathBuf::from(args.next().ok_or("project required")?);
    let out = PathBuf::from(args.next().ok_or("output folder required")?);
    std::fs::create_dir_all(&out)?;
    let mut app = Application::open(&project, Threads::Exactly(1))?;
    let started = Instant::now();
    let run = Arc::new(app.regenerate()?);
    println!(
        "generation_seconds={:.3} groups={}",
        started.elapsed().as_secs_f64(),
        run.toolpath.groups.len()
    );
    let timeline = Timeline::new(app.project(), Arc::clone(&run))?;
    let mut offset = 0;
    for (g, group) in run.toolpath.groups.iter().enumerate() {
        println!(
            "group={g} segments={} start_a={}",
            group.segments.len(),
            group.start.a.0
        );
        let n = group.segments.len();
        for (i, segment) in group.segments.iter().enumerate().skip(n.saturating_sub(5)) {
            let from = if i == 0 {
                group.start
            } else {
                group
                    .segments
                    .get(i - 1)
                    .ok_or("missing prior segment")?
                    .end
            };
            println!(
                "segment={i} kind={:?} from_a={} to_a={} turns={} seconds={}",
                segment.kind,
                from.a.0,
                segment.end.a.0,
                (segment.end.a.0 - from.a.0) / 360.0,
                segment.duration.0
            );
        }
        if n > 0 {
            for fraction in [0.0, 0.25, 0.5, 0.75, 1.0] {
                let pose = timeline.sample(Cursor {
                    done: offset + n - 1,
                    fraction,
                })?;
                println!(
                    "playback_last fraction={fraction} a={} work={:?}",
                    pose.a, pose.work
                );
            }
        }
        offset += n;
    }
    for post in ["makera-z1", "linuxcnc"] {
        app.set_post(Some(PostSelection::parse(post)));
        let run = app.regenerate()?;
        for (name, text) in &run.gcode {
            std::fs::write(out.join(format!("{post}-{name}")), text)?;
            println!("post={post} file={name} lines={}", text.lines().count());
            for (i, line) in text
                .lines()
                .enumerate()
                .skip(text.lines().count().saturating_sub(10))
            {
                println!("{}: {line}", i + 1);
            }
        }
        for report in &run.reports {
            println!("report={report:?}");
        }
    }
    Ok(())
}
