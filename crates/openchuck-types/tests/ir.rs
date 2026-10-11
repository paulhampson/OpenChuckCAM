// SPDX-License-Identifier: AGPL-3.0-or-later
//! Neutral toolpath IR tests (GC-01; verification plan section 5, openchuck-types row).

// Test code may unwrap, index and panic (NFR-07 applies to library code).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_cmp
)]

use openchuck_types::geometry::{Point3, UnitVec3, Vec3};
use openchuck_types::ir::{MoveKind, PathTags, Segment, ToolPose, Toolpath, ToolpathGroup};
use openchuck_types::units::{Degrees, FeedRate, Millimetres, Seconds};
use proptest::prelude::*;

fn pose(x: f64, a: f64) -> ToolPose {
    ToolPose {
        tip: Point3::new(x, 0.0, 15.0),
        axis: UnitVec3::Z,
        a: Degrees(a),
    }
}

fn sample() -> Toolpath {
    Toolpath {
        groups: vec![ToolpathGroup {
            tags: PathTags {
                operation: "op-1".to_owned(),
                setup: "setup-1".to_owned(),
                tool: "tool-3mm-ball".to_owned(),
            },
            start: pose(0.0, 0.0),
            segments: vec![
                Segment {
                    kind: MoveKind::Rapid,
                    end: pose(0.0, 0.0),
                    contact_feed: None,
                    duration: Seconds(0.25),
                    contact_length: Millimetres(0.0),
                },
                Segment {
                    kind: MoveKind::Plunge,
                    end: pose(0.0, 0.0),
                    contact_feed: Some(FeedRate(100.0)),
                    duration: Seconds(0.12),
                    contact_length: Millimetres(0.2),
                },
                Segment {
                    kind: MoveKind::Cutting,
                    end: pose(10.0, 540.0), // unwrapped A past one turn
                    contact_feed: Some(FeedRate(300.0)),
                    duration: Seconds(28.4),
                    contact_length: Millimetres(142.0),
                },
                Segment {
                    kind: MoveKind::Rapid,
                    end: pose(10.0, 180.0), // unwind: same tip, A changed by whole turns
                    contact_feed: None,
                    duration: Seconds(0.1),
                    contact_length: Millimetres(0.0),
                },
            ],
        }],
    }
}

#[test]
fn req_gc_01_ir_carries_every_slice_0_field_and_round_trips() {
    let ir = sample();
    ir.validate().unwrap();
    let json = serde_json::to_string(&ir).unwrap();
    let back: Toolpath = serde_json::from_str(&json).unwrap();
    assert_eq!(back, ir);
    // Every field listed in work breakdown slice 0 item 4 and decision record 0001.
    for field in [
        "\"tip\"",
        "\"axis\"",
        "\"a\"",
        "\"kind\"",
        "\"contact_feed\"",
        "\"duration\"",
        "\"contact_length\"",
        "\"operation\"",
        "\"setup\"",
        "\"tool\"",
    ] {
        assert!(json.contains(field), "missing {field} in {json}");
    }
}

#[test]
fn req_gc_01_ir_json_is_stable() {
    insta::assert_json_snapshot!("sample_ir", sample());
}

#[test]
fn req_gc_01_ir_names_no_controller_or_axis_letter() {
    // GC-01: the IR holds machine-neutral intent. Its serialised field names
    // must not mention G-code words, machine axes or feed modes.
    let json = serde_json::to_string(&sample()).unwrap().to_lowercase();
    for banned in [
        "g93",
        "g94",
        "g0",
        "g1",
        "inverse_time",
        "machine_x",
        "\"f\":",
    ] {
        assert!(!json.contains(banned), "IR mentions {banned}");
    }
}

#[test]
fn req_gc_01_total_duration_adds_in_order() {
    let total = sample().total_duration().0;
    assert!((total - (0.25 + 0.12 + 28.4 + 0.1)).abs() < 1e-12);
    assert_eq!(sample().segments().count(), 4);
}

#[test]
fn req_nfr_07_invalid_segments_give_errors() {
    let mut ir = sample();
    ir.groups[0].segments[2].contact_feed = None;
    let err = ir.validate().unwrap_err().to_string();
    assert!(err.contains("groups[0].segments[2].contact_feed"), "{err}");

    let mut ir = sample();
    ir.groups[0].segments[0].contact_feed = Some(FeedRate(10.0));
    assert!(ir.validate().unwrap_err().to_string().contains("rapid"));

    let mut ir = sample();
    ir.groups[0].segments[1].duration = Seconds(-1.0);
    assert!(ir.validate().is_err());

    let mut ir = sample();
    ir.groups[0].segments[1].end.a = Degrees(f64::NAN);
    assert!(ir.validate().is_err());
}

#[test]
fn req_nfr_07_non_unit_tool_axis_is_rejected_on_load() {
    let json = serde_json::to_string(&sample())
        .unwrap()
        .replacen("\"z\":1.0", "\"z\":2.0", 1);
    let err = serde_json::from_str::<Toolpath>(&json)
        .unwrap_err()
        .to_string();
    assert!(err.contains("unit vector"), "{err}");
}

#[test]
fn req_nfr_07_unknown_fields_are_rejected() {
    let json = serde_json::to_string(&sample()).unwrap().replacen(
        "\"kind\"",
        "\"feed_mode\":\"g93\",\"kind\"",
        1,
    );
    assert!(serde_json::from_str::<Toolpath>(&json).is_err());
}

fn arb_pose() -> impl Strategy<Value = ToolPose> {
    (
        -1e4..1e4f64,
        -1e4..1e4f64,
        -1e4..1e4f64,
        (-1.0..1.0f64, -1.0..1.0f64, 0.1..1.0f64),
        -1e5..1e5f64,
    )
        .prop_map(|(x, y, z, (ax, ay, az), a)| ToolPose {
            tip: Point3::new(x, y, z),
            axis: UnitVec3::new(Vec3::new(ax, ay, az)).unwrap(),
            a: Degrees(a),
        })
}

proptest! {
    #[test]
    fn req_gc_01_nfr_05_ir_round_trips_bit_exactly(
        start in arb_pose(),
        ends in prop::collection::vec((arb_pose(), 0.0..1e3f64, 1.0..1e4f64), 0..20),
    ) {
        let ir = Toolpath {
            groups: vec![ToolpathGroup {
                tags: PathTags { operation: "o".into(), setup: "s".into(), tool: "t".into() },
                start,
                segments: ends
                    .into_iter()
                    .map(|(end, duration, feed)| Segment {
                        kind: MoveKind::Cutting,
                        end,
                        contact_feed: Some(FeedRate(feed)),
                        duration: Seconds(duration),
                        contact_length: Millimetres(duration * feed / 60.0),
                    })
                    .collect(),
            }],
        };
        prop_assert!(ir.validate().is_ok());
        let json = serde_json::to_string(&ir).unwrap();
        let back: Toolpath = serde_json::from_str(&json).unwrap();
        // Double precision survives the text form exactly (NFR-05).
        prop_assert_eq!(back, ir);
    }
}
