// SPDX-License-Identifier: AGPL-3.0-or-later
//! Shared test inputs: a small part, a job and the built-in posts.
#![allow(dead_code, clippy::unwrap_used)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use openchuck_post::{PostJob, TemplatePost, ToolSetting, builtin};
use openchuck_types::geometry::{Point3, UnitVec3};
use openchuck_types::ir::{MoveKind, PathTags, Segment, ToolPose, Toolpath, ToolpathGroup};
use openchuck_types::kinematics::RotaryFrame;
use openchuck_types::machine::{ASense, RotaryAxis};
use openchuck_types::units::{Degrees, FeedRate, Millimetres, Seconds};

/// A path under the built-in posts' folder, which is inside this crate (decision 0103).
pub fn posts_path(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("posts")
        .join(path)
}

pub fn repo(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

pub fn pose(x: f64, y: f64, z: f64, a: f64) -> ToolPose {
    ToolPose {
        tip: Point3::new(x, y, z),
        axis: UnitVec3::Z,
        a: Degrees(a),
    }
}

pub fn seg(kind: MoveKind, end: ToolPose, duration: f64, length: f64) -> Segment {
    Segment {
        kind,
        end,
        contact_feed: kind.is_fed().then_some(FeedRate(300.0)),
        duration: Seconds(duration),
        contact_length: Millimetres(length),
    }
}

/// A small hand-reviewable part: a 20 mm cylinder with a helical groove of
/// three turns (A reaches 1080) over 6 mm of X in 45 degree steps, then the safe retract with an
/// unwind of three turns. The tool tip rides the top of the part.
pub fn small_part() -> Toolpath {
    let top = 10.0;
    let mut segments = vec![
        seg(MoveKind::Rapid, pose(0.0, 0.0, top + 2.0, 0.0), 0.1, 0.0),
        seg(MoveKind::Plunge, pose(0.0, 0.0, top, 0.0), 1.2, 2.0),
    ];
    for i in 1..=24 {
        let step = f64::from(i);
        segments.push(seg(
            MoveKind::Cutting,
            pose(0.25 * step, 0.0, top, 45.0 * step),
            1.0,
            7.9,
        ));
    }
    segments.push(seg(
        MoveKind::Linking,
        pose(6.0, 0.0, top + 2.0, 1080.0),
        0.4,
        2.0,
    ));
    segments.push(seg(MoveKind::Rapid, pose(6.0, 0.0, 30.0, 1080.0), 0.2, 0.0));
    segments.push(seg(MoveKind::Rapid, pose(6.0, 0.0, 30.0, 0.0), 0.3, 0.0));
    Toolpath {
        groups: vec![ToolpathGroup {
            tags: PathTags {
                operation: "groove (finish)".into(),
                setup: "s1".into(),
                tool: "t1".into(),
            },
            start: pose(0.0, 0.0, 30.0, 0.0),
            segments,
        }],
    }
}

pub fn job() -> PostJob {
    let mut tools = BTreeMap::new();
    tools.insert(
        "t1".to_string(),
        ToolSetting {
            number: 1,
            name: "4 mm ball".into(),
            spindle_rpm: 10000.0,
        },
    );
    PostJob {
        block_time: Default::default(),
        frame: RotaryFrame {
            axis: RotaryAxis::X,
            sense: ASense::Positive,
            origin: Point3::ORIGIN,
        },
        tools,
        title: "Small part".into(),
        operation_names: BTreeMap::new(),
        operation_spindle: BTreeMap::new(),
        revision: Some("abc1234".into()),
        generator: Some("OpenChuckCAM 0.10.0".into()),
        work_offset: None,
    }
}

pub fn builtin_post(id: &str) -> TemplatePost {
    TemplatePost::new(builtin(id).unwrap().unwrap())
}
