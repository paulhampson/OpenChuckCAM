// SPDX-License-Identifier: AGPL-3.0-or-later
//! Post definition files are user-editable data (GC-03, NFR-07; work breakdown
//! slice 6 item 3 and slice 20 item 4): any manifest text and template text
//! give a definition or an error naming the key or line, and a definition
//! that loads renders a small toolpath or gives an error, never a crash or a
//! hang. Seeded from `posts/*/`.
#![no_main]

use std::collections::BTreeMap;

use libfuzzer_sys::fuzz_target;
use openchuck_post::{PostDefinition, PostJob, PostProcessor, TemplatePost, ToolSetting};
use openchuck_types::geometry::{Point3, UnitVec3};
use openchuck_types::ir::{MoveKind, PathTags, Segment, ToolPose, Toolpath, ToolpathGroup};
use openchuck_types::kinematics::RotaryFrame;
use openchuck_types::machine::{ASense, RotaryAxis};
use openchuck_types::units::{Degrees, FeedRate, Millimetres, Seconds};

fn pose(x: f64, z: f64, a: f64) -> ToolPose {
    ToolPose {
        tip: Point3::new(x, 0.0, z),
        axis: UnitVec3::Z,
        a: Degrees(a),
    }
}

fn segment(kind: MoveKind, end: ToolPose) -> Segment {
    Segment {
        kind,
        end,
        contact_feed: kind.is_fed().then_some(FeedRate(300.0)),
        duration: Seconds(1.0),
        contact_length: Millimetres(2.0),
    }
}

/// A rapid, a plunge, two cuts with A and the retract: every kind of step a
/// template is given.
fn toolpath() -> Toolpath {
    Toolpath {
        groups: vec![ToolpathGroup {
            tags: PathTags {
                operation: "op (finish)".into(),
                setup: "s1".into(),
                tool: "t1".into(),
            },
            start: pose(0.0, 30.0, 0.0),
            segments: vec![
                segment(MoveKind::Rapid, pose(0.0, 12.0, 0.0)),
                segment(MoveKind::Plunge, pose(0.0, 10.0, 0.0)),
                segment(MoveKind::Cutting, pose(1.0, 10.0, 90.0)),
                segment(MoveKind::Cutting, pose(2.0, 10.0, 400.0)),
                segment(MoveKind::Linking, pose(2.0, 12.0, 400.0)),
                segment(MoveKind::Rapid, pose(2.0, 30.0, 0.0)),
            ],
        }],
    }
}

fn job() -> PostJob {
    PostJob {
        frame: RotaryFrame {
            axis: RotaryAxis::X,
            sense: ASense::Positive,
            origin: Point3::ORIGIN,
        },
        tools: BTreeMap::from([(
            "t1".to_owned(),
            ToolSetting {
                number: 1,
                name: "4 mm ball".into(),
                spindle_rpm: 10000.0,
            },
        )]),
        title: "Fuzz".into(),
        operation_names: BTreeMap::new(),
        revision: Some("abc1234".into()),
        work_offset: None,
    }
}

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    // A line `---` splits manifest from template for the fuzzer.
    let (manifest, template) = text.split_once("\n---\n").unwrap_or((text, ""));
    if let Ok(definition) = PostDefinition::from_parts("fuzz", manifest, template) {
        let _ = TemplatePost::new(definition).post(&toolpath(), &job());
    }
});
