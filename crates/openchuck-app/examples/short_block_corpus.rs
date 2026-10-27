// SPDX-License-Identifier: AGPL-3.0-or-later
//! Reanalyse existing absolute G94 Makera/LinuxCNC programs at 7 ms / 32 blocks.
//! Reads posted coordinates/feeds; preserves rapid/event boundaries and passes
//! reconstructed fed runs through the production command-rate implementation.
//! This is demand analysis, not a collision check or hardware validation.
//! cargo run -p openchuck-app --example short_block_corpus -- FILE.nc ...
#![allow(clippy::print_stdout, clippy::indexing_slicing)]
use openchuck_gcode::{Dialect, Motion, Step, read, replay};
use openchuck_post::{
    ConvertOptions, FeedRule, ShortBlockSettings, ToolSetting, to_machine_program,
};
use openchuck_types::{
    geometry::Point3,
    ir::{MoveKind, PathTags, Segment, ToolPose, Toolpath, ToolpathGroup},
    kinematics::{MachinePosition, RotaryFrame, tool_axis_in_model, tool_tip_in_model},
    machine::{ARange, ASense, RotaryAxis, ShortBlockMode},
    units::{Degrees, FeedRate, Millimetres, Seconds},
};
use std::{collections::BTreeMap, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "file,fed_blocks,short_blocks,longest_run,affected_windows,worst_mean_ms,added_seconds"
    );
    for file in std::env::args().skip(1) {
        if Path::new(&file).is_dir() {
            let mut app =
                openchuck_app::Application::open(Path::new(&file), openchuck_app::Threads::All)?;
            let mut machine = app
                .project()
                .machine()
                .ok_or("project has no machine")?
                .clone();
            machine.min_block_ms = 7.0;
            machine.block_window = 32;
            machine.short_block_mode = ShortBlockMode::Report;
            app.execute(openchuck_app::Command::SetMachine(Some(Box::new(machine))))?;
            app.set_output_grouping(openchuck_app::OutputGrouping::Operation);
            app.set_post(Some(openchuck_app::PostSelection::Builtin(
                "makera-z1".into(),
            )));
            let run = app.regenerate()?;
            for r in run.short_blocks {
                let name = app
                    .project()
                    .operations()
                    .iter()
                    .find(|o| o.id.to_string() == r.operation)
                    .map_or(r.operation.as_str(), |o| o.name.as_str());
                println!(
                    "{file}/{name},n/a,{},{},{},{:.6},{:.6}",
                    r.short_moves,
                    r.longest_run,
                    r.affected_windows,
                    r.worst_window_ms,
                    r.added_seconds
                );
            }
            continue;
        }
        let makera = Path::new(&file).extension().is_some_and(|x| x == "nc");
        let source = std::fs::read_to_string(&file)?;
        let replay = replay(
            &read(&source).map_err(|errors| std::io::Error::other(format!("{errors:?}")))?,
            if makera {
                Dialect::MakeraZ1
            } else {
                Dialect::Linuxcnc
            },
        )?;
        let frame = RotaryFrame {
            axis: RotaryAxis::X,
            sense: ASense::Positive,
            origin: Point3::ORIGIN,
        };
        let pose = |a: [f64; 4]| {
            let a_deg = Degrees(a[3]);
            ToolPose {
                tip: tool_tip_in_model(
                    &frame,
                    &MachinePosition {
                        xyz: Point3::new(a[0], a[1], a[2]),
                        a: a_deg,
                    },
                ),
                axis: tool_axis_in_model(&frame, a_deg),
                a: a_deg,
            }
        };
        let mut path = Toolpath::default();
        let mut current: Option<ToolpathGroup> = None;
        for step in replay.steps {
            let Step::Move(m) = step else {
                if let Some(g) = current.take() {
                    path.groups.push(g);
                }
                continue;
            };
            if m.motion != Motion::Linear || m.machine {
                if let Some(g) = current.take() {
                    path.groups.push(g);
                }
                continue;
            }
            if m.inverse_time || !m.metric {
                return Err("corpus example accepts absolute G94 millimetre programs only".into());
            }
            let length = ((m.to[0] - m.from[0]).powi(2)
                + (m.to[1] - m.from[1]).powi(2)
                + (m.to[2] - m.from[2]).powi(2))
            .sqrt();
            let da = (m.to[3] - m.from[3]).abs();
            if length <= 1e-9 && da <= 1e-9 {
                continue;
            }
            let amount = if makera {
                let p = 2.0 * std::f64::consts::PI * m.to[1].hypot(m.to[2]).max(1.0) + 30.0;
                if length > 1e-9 {
                    length.max(da * p / 360.0)
                } else {
                    da * p.min(360.0) / 360.0
                }
            } else if length > 1e-9 {
                length
            } else {
                da
            };
            let seconds = amount * 60.0 / m.feed.ok_or("fed move without feed")?;
            let group = current.get_or_insert_with(|| ToolpathGroup {
                tags: PathTags {
                    operation: file.clone(),
                    setup: "corpus".into(),
                    tool: "1".into(),
                },
                start: pose(m.from),
                segments: Vec::new(),
            });
            group.segments.push(Segment {
                kind: MoveKind::Cutting,
                end: pose(m.to),
                contact_feed: Some(FeedRate(1.0)),
                duration: Seconds(seconds),
                contact_length: Millimetres(length),
            });
        }
        if let Some(g) = current {
            path.groups.push(g);
        }
        let options = ConvertOptions {
            short_blocks: ShortBlockSettings {
                min_block_ms: 7.0,
                block_window: 32,
                mode: ShortBlockMode::SlowDown,
            },
            feed_decimals: Some(1),
            a_range: ARange::Continuous,
            safe_z: 0.0,
            frame,
            feed_rule: if makera {
                FeedRule::Makera
            } else {
                FeedRule::Linuxcnc
            },
            max_feed: None,
            tools: BTreeMap::from([(
                "1".into(),
                ToolSetting {
                    number: 1,
                    name: "corpus".into(),
                    spindle_rpm: 1000.0,
                },
            )]),
            decimals: Some((3, 3)),
            operation_names: BTreeMap::new(),
            operation_spindle: BTreeMap::new(),
        };
        let result = to_machine_program(&path, &options)?;
        let blocks: usize = path.groups.iter().map(|g| g.segments.len()).sum();
        let short: usize = result.short_blocks.iter().map(|r| r.short_moves).sum();
        let longest = result
            .short_blocks
            .iter()
            .map(|r| r.longest_run)
            .max()
            .unwrap_or(0);
        let windows: usize = result.short_blocks.iter().map(|r| r.affected_windows).sum();
        let worst = result
            .short_blocks
            .iter()
            .map(|r| r.worst_window_ms)
            .fold(f64::INFINITY, f64::min);
        let added: f64 = result.short_blocks.iter().map(|r| r.added_seconds).sum();
        println!("{file},{blocks},{short},{longest},{windows},{worst:.6},{added:.6}");
    }
    Ok(())
}
