// SPDX-License-Identifier: AGPL-3.0-or-later
//! Generates a project's operations one at a time and reports what each
//! costs (issue #252; NFR-03, NFR-04): the time, the moves and points made,
//! a hash of the toolpath (the same project must give the same hash at any
//! thread count, GC-07 and NFR-06), and the progress reports the job sent,
//! with how often the shown text changed its total (issue #248).
//!
//! ```text
//! cargo run --release -p openchuck-app --example operation_times -- PROJECT [THREADS] [--simulate] [--only NAME]
//! ```
//!
//! Each operation is generated with "generate selected" up to it, so the ones
//! before it come from the cache and only its own work is timed. With
//! `--simulate` the run up to the last operation is also simulated and timed.

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::time::Instant;

use openchuck_app::{Application, Event, Threads};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut project = None;
    let mut threads = Threads::All;
    let mut simulate = false;
    let mut only = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--simulate" => simulate = true,
            "--only" => only = Some(args.next().ok_or("--only needs a name")?),
            _ if project.is_none() => project = Some(PathBuf::from(arg)),
            n => threads = Threads::Exactly(n.parse()?),
        }
    }
    let usage = "usage: operation_times PROJECT [THREADS] [--simulate] [--only NAME]";
    let project = project.ok_or(usage)?;
    let app = Application::open(&project, threads)?;
    println!("project:  {}", project.display());
    println!("threads:  {threads:?}");
    let operations: Vec<_> = app
        .project()
        .operations()
        .iter()
        .filter(|o| o.enabled)
        .map(|o| (o.id, o.name.clone(), o.strategy.clone()))
        .collect();
    for (id, name, strategy) in &operations {
        let started = Instant::now();
        let run = app.regenerate_until(*id)?;
        let took = started.elapsed();
        let events = app.drain_events();
        if only.as_ref().is_some_and(|o| o != name) {
            continue;
        }
        let group = run.groups.iter().position(|g| g.operation == *id);
        let Some(group) = group.and_then(|g| run.toolpath.groups.get(g)) else {
            continue;
        };
        let mut hasher = DefaultHasher::new();
        format!("{group:?}").hash(&mut hasher);
        let (reports, changes) = progress(&events);
        println!(
            "{name} ({strategy}): {:.2} s, {} moves, hash {:016x}, {reports} progress reports, \
             the shown total changed {changes} times",
            took.as_secs_f64(),
            group.segments.len(),
            hasher.finish(),
        );
    }
    if simulate {
        let started = Instant::now();
        let run = app.simulate()?;
        println!("simulate: {:.2} s", started.elapsed().as_secs_f64());
        if let Some(report) = &run.simulation {
            for setup in &report.setups {
                if let Some(d) = &setup.deviation {
                    println!(
                        "  {}: {} samples, most left {:?}, deepest overcut {:?}",
                        setup.setup,
                        d.points.len(),
                        d.worst_remaining.map(|p| (p.value, p.at_least, p.point)),
                        d.worst_overcut.map(|p| (p.value, p.point)),
                    );
                    // Where material of 1 mm or more is left on the counted
                    // samples, by 1 mm of X, and how far from the axis.
                    let mut slabs = std::collections::BTreeMap::new();
                    println!(
                        "    counted: {} samples",
                        d.points.iter().filter(|p| p.counts()).count()
                    );
                    for p in d.points.iter().filter(|p| p.counts() && p.value >= 1.0) {
                        let slab = slabs.entry(p.point.x.floor() as i64).or_insert((
                            0_usize,
                            f64::INFINITY,
                            0.0_f64,
                            0_usize,
                        ));
                        let r = p.point.y.hypot(p.point.z);
                        slab.0 += 1;
                        slab.1 = slab.1.min(r);
                        slab.2 = slab.2.max(r);
                        slab.3 += usize::from(p.at_least);
                    }
                    for (x, (count, r0, r1, bound)) in slabs {
                        println!(
                            "    X {x} to {}: {count} samples left 1 mm or more \
                             ({bound} at the 3 mm bound), radius {r0:.2} to {r1:.2}",
                            x + 1
                        );
                    }
                }
            }
        }
    }
    Ok(())
}

/// The progress reports in `events`, and how many times the total shown
/// (the "n of m" of the footer) changed from one report to the next.
fn progress(events: &[Event]) -> (usize, usize) {
    let mut reports = 0;
    let mut changes = 0;
    let mut last = None;
    for event in events {
        if let Event::JobProgress { progress, .. } = event {
            reports += 1;
            if last.is_some_and(|t| t != progress.total) {
                changes += 1;
            }
            last = Some(progress.total);
        }
    }
    (reports, changes)
}
