// SPDX-License-Identifier: AGPL-3.0-or-later
//! Disposable, uninstrumented PRJ-08 latency/memory/poll-gap measurement.
//! Run each case in a separate process: small, operations, selection.
#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use openchuck_app::{
    checkpoints::{Notice, Session},
    project::{
        self, ContentHash, Item, ModelRef, Operation, OrderKey, ParameterValue, Placement, Project,
        RelativePath, SelectionSet, SelectionTarget, Setup,
    },
};
use project::id::{ModelId, OperationId, SelectionSetId, SetupId};
use std::collections::BTreeMap;
use std::io::Write;
use std::time::{Duration, Instant};

fn memory(key: &str) -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| s.lines().find(|l| l.starts_with(key)).map(str::to_owned))
        .and_then(|s| s.split_whitespace().nth(1).and_then(|n| n.parse().ok()))
        .unwrap_or(0)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let case = std::env::args().nth(1).unwrap_or_else(|| "small".into());
    let (operations, triangles) = match case.as_str() {
        "small" => (10, 0),
        "operations" => (1000, 0),
        "selection" => (100, 200_000),
        _ => return Err("case must be small, operations or selection".into()),
    };
    let root = std::env::temp_dir().join(format!(
        "openchuck-checkpoint-measure-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let folder = root.join("project");
    let mut project = Project::new(format!("Synthetic {case}"));
    let setup = SetupId::from_u128(1);
    project.insert(Item::Setup(Setup::new(setup, "Setup")), Placement::End)?;
    if triangles > 0 {
        let model = ModelId::from_u128(2);
        let hash = ContentHash::of_bytes(b"synthetic referenced model");
        project.insert(
            Item::Model(ModelRef::new(model, RelativePath::new("model.stl")?, hash)),
            Placement::End,
        )?;
        project.insert(
            Item::SelectionSet(SelectionSet {
                id: SelectionSetId::from_u128(3),
                order: OrderKey::default(),
                name: "Large selection".into(),
                model,
                model_hash: hash,
                target: SelectionTarget::Triangles((0..triangles).collect()),
                descriptors: Vec::new(),
                stock_to_leave: None,
                colour: None,
                keep_tangent_edges: None,
            }),
            Placement::End,
        )?;
    }
    for n in 0..operations {
        project.insert(
            Item::Operation(Operation {
                enabled: true,
                id: OperationId::from_u128(100 + n),
                order: OrderKey::default(),
                name: format!("Operation {n}"),
                setup,
                strategy: "helical_finishing".into(),
                tool: None,
                selections: Vec::new(),
                boundary: None,
                avoid: Vec::new(),
                check: Vec::new(),
                restriction_options: Default::default(),
                spindle_speed: None,
                material: None,
                parameters: BTreeMap::from([("stepover".into(), ParameterValue::Number(0.5))]),
            }),
            Placement::End,
        )?;
    }
    project::file::save(&Project::new("saved baseline"), &folder)?;
    let bytes: usize = project::file::render(&project)?
        .values()
        .map(String::len)
        .sum();
    let baseline = memory("VmRSS:");
    let mut session = Session::open(folder, root.join("config"), 3)?;
    let start = Instant::now();
    loop {
        if session
            .poll(Duration::ZERO)
            .iter()
            .any(|n| matches!(n, Notice::Ready))
        {
            break;
        }
        if start.elapsed() > Duration::from_secs(10) {
            return Err("worker startup timeout".into());
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    let mut dispatch = Duration::ZERO;
    let mut gap = Duration::ZERO;
    let mut durations = Vec::new();
    for (n, (edit, capture)) in [(0, 15), (180, 180), (360, 360), (540, 540), (560, 575)]
        .into_iter()
        .enumerate()
    {
        let started = Instant::now();
        session.tick(Duration::from_secs(edit), &project, n as u64 + 1, true);
        session.tick(Duration::from_secs(capture), &project, n as u64 + 1, true);
        dispatch = dispatch.max(started.elapsed());
        let mut previous = Instant::now();
        loop {
            gap = gap.max(previous.elapsed());
            previous = Instant::now();
            let notices = session.poll(Duration::from_secs(capture));
            let mut done = false;
            for notice in notices {
                match notice {
                    Notice::Published { .. } => done = true,
                    Notice::Problem(e) => return Err(e.into()),
                    _ => {}
                }
            }
            if done {
                break;
            }
            if started.elapsed() > Duration::from_secs(120) {
                return Err("publication exceeded measurement timeout".into());
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        durations.push(started.elapsed());
    }
    durations.sort();
    writeln!(
        std::io::stdout(),
        "case={case},operations={operations},triangles={triangles},project_bytes={bytes},dispatch_max_ms={:.3},publication_median_ms={:.3},poll_gap_max_ms={:.3},baseline_rss_kib={baseline},peak_extra_kib={}",
        dispatch.as_secs_f64() * 1000.0,
        durations[2].as_secs_f64() * 1000.0,
        gap.as_secs_f64() * 1000.0,
        memory("VmHWM:").saturating_sub(baseline)
    )?;
    session.close(5)?;
    loop {
        if session
            .poll(Duration::from_secs(576))
            .iter()
            .any(|n| matches!(n, Notice::Closed { .. }))
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    drop(session);
    std::fs::remove_dir_all(root)?;
    Ok(())
}
