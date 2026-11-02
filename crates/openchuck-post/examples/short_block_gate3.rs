// SPDX-License-Identifier: AGPL-3.0-or-later
//! Small simulator-only GC-17 before/after programs, generated with the production post.
//! cargo run -p openchuck-post --example short_block_gate3 -- OUTPUT_DIRECTORY
#![allow(clippy::print_stdout)]
use openchuck_post::{
    BlockTimeSettings, PostJob, PostProcessor, TemplatePost, ToolSetting, builtin,
};
use openchuck_types::{
    geometry::{Point3, UnitVec3},
    ir::{MoveKind, PathTags, Segment, ToolPose, Toolpath, ToolpathGroup},
    kinematics::RotaryFrame,
    machine::{ARange, ASense, RotaryAxis, ShortBlockMode},
    units::{Degrees, FeedRate, Millimetres, Seconds},
};
use std::{collections::BTreeMap, path::PathBuf};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let folder = PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("supply a simulator-output directory")?,
    );
    std::fs::create_dir_all(&folder)?;
    let pose = |x| ToolPose {
        tip: Point3::new(x, 0.0, 50.0),
        axis: UnitVec3::Z,
        a: Degrees(0.0),
    };
    let path = Toolpath {
        groups: vec![ToolpathGroup {
            rotary_reset_after: Vec::new(),
            tags: PathTags {
                operation: "gate3-short-blocks".into(),
                setup: "simulator".into(),
                tool: "1".into(),
            },
            start: pose(0.0),
            segments: (1..=40)
                .map(|i| Segment {
                    kind: MoveKind::Cutting,
                    end: pose(f64::from(i) * 0.01),
                    contact_feed: Some(FeedRate(600.0)),
                    duration: Seconds(0.001),
                    contact_length: Millimetres(0.01),
                })
                .collect(),
        }],
    };
    let mut job = PostJob {
        block_time: BlockTimeSettings {
            min_block_ms: 7.0,
            block_window: 32,
            mode: ShortBlockMode::Report,
        },
        a_range: ARange::Continuous,
        safe_z: 50.0,
        frame: RotaryFrame {
            axis: RotaryAxis::X,
            sense: ASense::Positive,
            origin: Point3::ORIGIN,
        },
        tools: BTreeMap::from([(
            "1".into(),
            ToolSetting {
                number: 1,
                name: "simulator only".into(),
                spindle_rpm: 1000.0,
            },
        )]),
        title: "GC-17 simulator-only verification".into(),
        operation_names: BTreeMap::new(),
        operation_spindle: BTreeMap::new(),
        revision: None,
        generator: None,
        work_offset: Some("G54".into()),
    };
    let post = TemplatePost::new(builtin("linuxcnc").ok_or("missing LinuxCNC post")??);
    for (name, mode) in [
        ("original", ShortBlockMode::Report),
        ("slowed", ShortBlockMode::SlowDown),
    ] {
        job.block_time.mode = mode;
        let output = post.post(&path, &job)?;
        std::fs::write(folder.join(format!("{name}.ngc")), output.text)?;
        let time = output.estimated_seconds;
        let added: f64 = output.short_blocks.iter().map(|r| r.added_seconds).sum();
        println!("{name}: programmed {time:.6}s; predicted/applied added {added:.6}s");
    }
    Ok(())
}
