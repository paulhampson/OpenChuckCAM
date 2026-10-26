// SPDX-License-Identifier: AGPL-3.0-or-later
//! Operation boundaries, not cutting-pass angle unwrapping (decision 0241).
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::float_cmp)]

mod support;

use openchuck_path::RotaryMapping;
use openchuck_path::program::{ConnectionOptions, connect_group};
use openchuck_path::rotary::AxisPosition;
use openchuck_types::geometry::{Aabb, Point3};
use openchuck_types::ir::{MoveKind, PathTags, Segment, ToolPose, ToolpathGroup};
use openchuck_types::job::{CancelToken, JobContext, NoProgress};
use openchuck_types::machine::{ARange, MachineProfile};
use openchuck_types::units::{Degrees, FeedRate, Millimetres, Seconds};

fn machine() -> MachineProfile {
    let mut m = MachineProfile::example_x_rotary();
    m.rotary_origin = Point3::ORIGIN;
    m.travel = Aabb {
        min: Point3::new(-100.0, -100.0, -100.0),
        max: Point3::new(100.0, 100.0, 100.0),
    };
    m.clearance.safe_z = Millimetres(30.0);
    m.clearance.radius = Millimetres(25.0);
    m
}

fn pose(m: &MachineProfile, x: f64, z: f64, a: f64) -> ToolPose {
    let r = RotaryMapping::of_profile(m);
    ToolPose {
        tip: r.to_model(
            AxisPosition {
                along: x,
                across: 0.0,
                height: z,
            },
            Degrees(a),
        ),
        axis: r.tool_axis(Degrees(a)),
        a: Degrees(a),
    }
}

fn group(m: &MachineProfile, start: f64, end: f64) -> ToolpathGroup {
    ToolpathGroup {
        rotary_reset_after: Vec::new(),
        tags: PathTags {
            setup: "setup".into(),
            operation: "cut".into(),
            tool: "tool".into(),
        },
        start: pose(m, 10.0, 30.0, start),
        segments: vec![
            Segment {
                kind: MoveKind::Cutting,
                end: pose(m, 20.0, 10.0, end),
                contact_feed: Some(FeedRate(100.0)),
                duration: Seconds(10.0),
                contact_length: Millimetres(10.0),
            },
            Segment {
                kind: MoveKind::Rapid,
                end: pose(m, 20.0, 30.0, end),
                contact_feed: None,
                duration: Seconds(1.0),
                contact_length: Millimetres(0.0),
            },
        ],
    }
}

fn options(m: &MachineProfile) -> ConnectionOptions<'_> {
    ConnectionOptions {
        machine: m,
        clearance: m.clearance,
        enclosing_radius: 20.0,
        required_start_a: None,
    }
}

#[test]
fn req_op_14_req_op_15_links_preserve_signed_turns_and_cutting_travel() {
    let m = machine();
    let cancel = CancelToken::new();
    let ctx = JobContext::new(&cancel, &NoProgress);
    for (end, start, expected) in [(593275.0, 45.0, 593325.0), (-593275.0, -45.0, -593325.0)] {
        for tool_change in [false, true] {
            let previous = group(&m, 0.0, end);
            let mut next = group(&m, start, start + 720.0);
            if tool_change {
                next.tags.tool = "other tool".into();
            }
            let original = next.clone();
            let linked = connect_group(Some(&previous), &next, &options(&m), &ctx).unwrap();
            assert_eq!(next, original, "the cached canonical group must not change");
            assert_eq!(linked.group.start, previous.segments.last().unwrap().end);
            assert!(linked.link_segments > 0);
            let entry = linked.group.segments[linked.link_segments - 1].end;
            assert_eq!(entry.a.0, expected);
            for (source, placed) in original
                .segments
                .iter()
                .zip(&linked.group.segments[linked.link_segments..])
            {
                let mut expected = *source;
                expected.end.a.0 += linked.a_offset.0;
                assert_eq!(
                    *placed, expected,
                    "only a constant whole-turn A offset may change cuts"
                );
            }
            assert!(
                linked.group.segments[..linked.link_segments]
                    .iter()
                    .all(|s| s.kind == MoveKind::Rapid
                        && s.duration.0 > 0.0
                        && s.contact_feed.is_none())
            );
        }
    }
}

#[test]
fn req_op_15_boundary_ties_prefer_positive_machine_a_and_respect_whole_group_limits() {
    let mut m = machine();
    let cancel = CancelToken::new();
    let ctx = JobContext::new(&cancel, &NoProgress);
    for (previous_a, start_a) in [(0.0, -180.0), (0.0, 180.0), (-720.0, -900.0)] {
        let p = group(&m, previous_a, previous_a);
        let n = group(&m, start_a, start_a);
        let linked = connect_group(Some(&p), &n, &options(&m), &ctx).unwrap();
        assert_eq!(n.start.a.0 + linked.a_offset.0, previous_a + 180.0);
    }
    m.a_range = ARange::Limited {
        min: Degrees(-200.0),
        max: Degrees(200.0),
    };
    let p = group(&m, 0.0, 0.0);
    let n = group(&m, 180.0, 300.0);
    let linked = connect_group(Some(&p), &n, &options(&m), &ctx).unwrap();
    assert_eq!(
        linked.a_offset.0, -360.0,
        "positive tied start would put the later cut outside the limit"
    );
    assert!(connect_group(Some(&p), &group(&m, 0.0, 720.0), &options(&m), &ctx).is_err());
}

#[test]
fn req_op_14_req_set_09_req_op_21_connection_refuses_unproved_clearance_and_travel() {
    let m = machine();
    let cancel = CancelToken::new();
    let ctx = JobContext::new(&cancel, &NoProgress);
    let p = group(&m, 0.0, 720.0);
    let n = group(&m, 45.0, 90.0);
    let mut opts = options(&m);
    // An enclosing bound includes a held region/tab/fixture that rises into
    // the connection. Clear tips at the two ends alone are not a proof.
    opts.enclosing_radius = 31.0;
    assert!(connect_group(Some(&p), &n, &opts, &ctx).is_err());
    opts.enclosing_radius = f64::NAN;
    assert!(connect_group(Some(&p), &n, &opts, &ctx).is_err());
    let mut narrow = m.clone();
    narrow.travel.max.x = 15.0;
    assert!(connect_group(Some(&p), &n, &options(&narrow), &ctx).is_err());
}

#[test]
fn req_op_15_req_gc_05_standalone_starts_and_required_orientation_are_preserved() {
    let m = machine();
    let cancel = CancelToken::new();
    let ctx = JobContext::new(&cancel, &NoProgress);
    let p = group(&m, 0.0, 1080.0);
    let n = group(&m, 45.0, 135.0);
    let single = connect_group(None, &n, &options(&m), &ctx).unwrap();
    assert_eq!(single.group, n);
    assert_eq!(single.link_segments, 0);
    let mut opts = options(&m);
    opts.required_start_a = Some(Degrees(45.0));
    let explicit = connect_group(Some(&p), &n, &opts, &ctx).unwrap();
    assert_eq!(explicit.a_offset.0, 0.0);
    assert!(explicit.link_segments > 0);
    opts.required_start_a = Some(Degrees(46.0));
    assert!(connect_group(Some(&p), &n, &opts, &ctx).is_err());
}

#[test]
fn req_nfr_04_program_connection_observes_cancellation() {
    let m = machine();
    let cancel = CancelToken::new();
    cancel.cancel();
    let ctx = JobContext::new(&cancel, &NoProgress);
    assert!(connect_group(None, &group(&m, 0.0, 90.0), &options(&m), &ctx).is_err());
}

#[test]
fn req_op_15_shared_retract_retains_both_turn_directions_and_limited_position() {
    use openchuck_path::{CommonParameters, CutPoint, MeshSurface, Pass, Strategy};
    use openchuck_testkit::parts::{Cylinder, RadialPart};
    use support::{Fixture, Rings, ball, ring_values};
    let mesh = Cylinder {
        radius: 8.0,
        length: 30.0,
    }
    .mesh(2, 36);
    let mut fixture = Fixture::new(MeshSurface::new(&mesh).unwrap(), 10.0, &ball(3.0));
    let parameters = Rings
        .schema()
        .resolve(&ring_values(10.0, 20.0, 1.0, 0.0))
        .unwrap();
    let common = CommonParameters::read(&parameters).unwrap();
    let cancel = CancelToken::new();
    let ctx = JobContext::new(&cancel, &NoProgress);
    for (range, sign, final_a) in [
        (ARange::Continuous, 1.0, 720.0),
        (ARange::Continuous, -1.0, -720.0),
        (
            ARange::Limited {
                min: Degrees(355.0),
                max: Degrees(1100.0),
            },
            1.0,
            1080.0,
        ),
    ] {
        fixture.machine.a_range = range;
        fixture.rotary = RotaryMapping::of_profile(&fixture.machine);
        let points = (0..=8)
            .map(|i| {
                let a = Degrees(sign * f64::from(i) * 90.0);
                let tip = fixture.rotary.to_model(
                    AxisPosition {
                        along: 15.0,
                        across: 0.0,
                        height: 10.0,
                    },
                    a,
                );
                CutPoint {
                    tip,
                    a,
                    contact: tip,
                }
            })
            .collect();
        let path = openchuck_path::assemble::assemble(
            &[Pass {
                helical_pass_start: false,
                points,
                stock: None,
                rapids: Vec::new(),
            }],
            &fixture.input(),
            &common,
            &ctx,
        )
        .unwrap();
        assert_eq!(path.group.segments.last().unwrap().end.a.0, final_a);
    }
}

#[test]
fn req_gc_01_req_op_15_connections_preserve_declared_pass_boundaries_in_canonical_copies() {
    let m = machine();
    let cancel = CancelToken::new();
    let ctx = JobContext::new(&cancel, &NoProgress);
    let previous = group(&m, 0.0, 1080.0);
    let mut next = group(&m, 0.0, 720.0);
    next.segments.push(Segment {
        end: pose(&m, 10.0, 30.0, 740.0),
        ..next.segments[1]
    });
    next.rotary_reset_after = vec![1];
    let original = next.clone();
    let placed = connect_group(Some(&previous), &next, &options(&m), &ctx).unwrap();
    assert_eq!(
        next, original,
        "cached canonical geometry must remain context-independent"
    );
    assert_eq!(
        placed.group.rotary_reset_after,
        vec![1 + placed.link_segments]
    );
    openchuck_types::ir::Toolpath {
        groups: vec![placed.group],
    }
    .validate()
    .unwrap();
    let split = connect_group(None, &next, &options(&m), &ctx).unwrap();
    assert_eq!(split.group.rotary_reset_after, vec![1]);
}
