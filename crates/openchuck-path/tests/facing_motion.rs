// SPDX-License-Identifier: AGPL-3.0-or-later
//! Independent machine-frame travel/timing checks (OP-24, OP-13/15).
#![allow(clippy::unwrap_used)]
use openchuck_path::facing::motion::verify_machine_motion;
use openchuck_types::geometry::{Aabb, Point3, UnitVec3};
use openchuck_types::ir::{MoveKind, PathTags, Segment, ToolPose, ToolpathGroup};
use openchuck_types::job::{CancelToken, JobContext, NoProgress};
use openchuck_types::machine::{ARange, MachineProfile, RotaryInFeed};
use openchuck_types::units::{Degrees, FeedRate, Millimetres, RotaryFeedRate, Seconds};

fn machine() -> MachineProfile {
    let mut m = MachineProfile::example_x_rotary();
    m.rotary_origin = Point3::new(100., 100., 100.);
    m.travel = Aabb {
        min: Point3::new(0., 0., 0.),
        max: Point3::new(200., 200., 200.),
    };
    m.max_feeds.linear = FeedRate(600.);
    m.max_feeds.z = FeedRate(300.);
    m.max_feeds.rotary = RotaryFeedRate(60000.);
    m.max_feeds.rapid_linear = FeedRate(1200.);
    m.max_feeds.rapid_z = FeedRate(600.);
    m.max_feeds.rapid_rotary = RotaryFeedRate(180.);
    m.max_feeds.rotary_in_feed = RotaryInFeed::Separate;
    m
}
fn group() -> ToolpathGroup {
    ToolpathGroup {
        tags: PathTags {
            operation: "op".into(),
            setup: "setup".into(),
            tool: "tool".into(),
        },
        start: ToolPose {
            tip: Point3::new(0., 0., 10.),
            axis: UnitVec3::Z,
            a: Degrees(0.),
        },
        segments: vec![Segment {
            kind: MoveKind::Cutting,
            end: ToolPose {
                tip: Point3::new(10., 0., 10.),
                axis: UnitVec3::Z,
                a: Degrees(0.),
            },
            contact_feed: Some(FeedRate(600.)),
            duration: Seconds(1.),
            contact_length: Millimetres(10.),
        }],
    }
}
#[test]
fn req_op_24_req_set_06_machine_frame_travel_applies_to_all_motion_and_start_pose() {
    let m = machine();
    let mut g = group();
    let token = CancelToken::new();
    let ctx = JobContext::new(&token, &NoProgress);
    verify_machine_motion(&g, &m, &ctx).unwrap();
    g.segments[0].end.tip.x = 101.;
    assert!(
        verify_machine_motion(&g, &m, &ctx)
            .unwrap_err()
            .to_string()
            .contains("travel")
    );
    g = group();
    g.start.tip.z = 101.;
    assert!(verify_machine_motion(&g, &m, &ctx).is_err());
    g = group();
    g.segments[0].kind = MoveKind::Rapid;
    g.segments[0].contact_feed = None;
    g.segments[0].duration = Seconds(0.5);
    verify_machine_motion(&g, &m, &ctx).unwrap();
    g.segments[0].duration = Seconds(0.49);
    assert!(
        verify_machine_motion(&g, &m, &ctx)
            .unwrap_err()
            .to_string()
            .contains("feed")
    );
}
#[test]
fn req_op_24_req_op_13_req_op_15_rotary_range_axis_and_coupled_feed_are_independently_checked() {
    let mut m = machine();
    let mut g = group();
    let token = CancelToken::new();
    let ctx = JobContext::new(&token, &NoProgress);
    g.segments[0].end = ToolPose {
        tip: Point3::new(10., 10., 0.),
        axis: UnitVec3::Y,
        a: Degrees(90.),
    };
    g.segments[0].duration = Seconds(1.1);
    verify_machine_motion(&g, &m, &ctx).unwrap();
    m.max_feeds.rotary_in_feed = RotaryInFeed::Makera;
    assert!(
        verify_machine_motion(&g, &m, &ctx)
            .unwrap_err()
            .to_string()
            .contains("feed")
    );
    g.segments[0].duration = Seconds(3.);
    verify_machine_motion(&g, &m, &ctx).unwrap();
    m.a_range = ARange::Limited {
        min: Degrees(-45.),
        max: Degrees(45.),
    };
    assert!(
        verify_machine_motion(&g, &m, &ctx)
            .unwrap_err()
            .to_string()
            .contains("A travel")
    );
    m = machine();
    g.segments[0].end.axis = UnitVec3::Z;
    assert!(verify_machine_motion(&g, &m, &ctx).is_err());
    token.cancel();
    assert!(matches!(
        verify_machine_motion(&group(), &m, &ctx),
        Err(openchuck_path::PathError::Cancelled)
    ));
}

#[test]
fn req_op_24_req_op_21_rapid_with_clear_endpoints_still_refuses_a_thin_intervening_tab() {
    use openchuck_path::Solid;
    use openchuck_path::facing::motion::verify_rapid_clearance;
    use openchuck_path::facing::plan::FacingSetup;
    use openchuck_types::mesh::TriangleMesh;
    use openchuck_types::stock::Stock;
    use openchuck_types::tool::{Tool, ToolShape};
    let m = machine();
    let part = TriangleMesh {
        vertices: vec![
            Point3::new(30.123003, -0.1, 15.9),
            Point3::new(30.123008, 0.1, 16.1),
            Point3::new(30.123008, -0.1, 16.1),
        ],
        triangles: vec![[0, 1, 2]],
    };
    let stock = Stock::Box {
        bounds: Aabb {
            min: Point3::new(30.123, -2., 14.),
            max: Point3::new(30.12301, 2., 18.),
        },
    };
    let tool = Tool {
        shape: ToolShape::FlatEnd,
        diameter: Millimetres(0.02),
        flute_length: Millimetres(10.),
        overall_length: Millimetres(30.),
        shank_diameter: Millimetres(0.02),
        flute_count: 2,
        material: None,
        centre_cutting: None,
    };
    let tab = Solid::cylinder(Point3::new(30.123, 0., 16.), UnitVec3::X, 0.000002, 2.).unwrap();
    let setup = FacingSetup {
        part: &part,
        stock: &stock,
        tool: &tool,
        machine: &m,
        tabs: &[tab],
        protected: &[tab],
        held: [None, None],
        projection_error: Some(0.),
    };
    let mut g = group();
    g.start.tip = Point3::new(20., 0., 2.);
    g.segments[0].kind = MoveKind::Rapid;
    g.segments[0].contact_feed = None;
    g.segments[0].duration = Seconds(1.);
    g.segments[0].end.tip = Point3::new(40., 0., 2.);
    for pose in [g.start, g.segments[0].end] {
        assert!(pose.tip.x + 0.01 < 30.123 || pose.tip.x - 0.01 > 30.12301);
        assert!(
            pose.tip.z + 10. < 14.,
            "The cutting flute stays below the stock"
        );
    }
    // A strict collision witness between the clear endpoints. No sampling
    // grid, tool-tip-only or warning-only observation establishes clearance.
    let witness = Point3::new(30.123001, 0., 15.);
    assert!(tab.contains(witness));
    assert!(
        (witness.x - 30.123001).hypot(witness.y) < 0.01 && witness.z > 12. && witness.z < 32.,
        "Witness lies in the shank above the flute"
    );
    let token = CancelToken::new();
    let ctx = JobContext::new(&token, &NoProgress);
    assert!(
        verify_rapid_clearance(&setup, &g, 0., &ctx)
            .unwrap_err()
            .to_string()
            .contains("rapid")
    );
}

#[test]
fn req_op_24_req_op_12_req_op_21_complete_fed_body_preserves_tabs_and_whole_part() {
    use openchuck_path::Solid;
    use openchuck_path::facing::motion::verify_protected_clearance;
    use openchuck_path::facing::plan::FacingSetup;
    use openchuck_types::mesh::TriangleMesh;
    use openchuck_types::stock::Stock;
    use openchuck_types::tool::{Tool, ToolShape};
    let m = machine();
    let part = TriangleMesh {
        vertices: vec![
            Point3::new(10., -2., -3.),
            Point3::new(50., 4., 7.),
            Point3::new(50., -2., 7.),
        ],
        triangles: vec![[0, 1, 2]],
    };
    let stock = Stock::Box {
        bounds: Aabb {
            min: Point3::new(0., -10., -12.),
            max: Point3::new(60., 10., 12.),
        },
    };
    let tool = Tool {
        shape: ToolShape::FlatEnd,
        diameter: Millimetres(3.),
        flute_length: Millimetres(10.),
        overall_length: Millimetres(30.),
        shank_diameter: Millimetres(3.),
        flute_count: 2,
        material: None,
        centre_cutting: None,
    };
    let tab = Solid::cylinder(Point3::new(50., 0., 0.), UnitVec3::X, 10., 5.).unwrap();
    let held = Solid::cuboid(Aabb {
        min: Point3::new(0., -10., -12.),
        max: Point3::new(5., 10., 12.),
    })
    .unwrap();
    let protections = [tab, held];
    let setup = FacingSetup {
        part: &part,
        stock: &stock,
        tool: &tool,
        machine: &m,
        tabs: &[tab],
        protected: &protections,
        held: [Some((0., 5.)), None],
        projection_error: Some(0.),
    };
    let token = CancelToken::new();
    let ctx = JobContext::new(&token, &NoProgress);
    let mut g = group();
    g.start.tip = Point3::new(51.7, -10., 5.01);
    g.segments[0].end.tip = Point3::new(51.7, 10., 5.01);
    g.segments[0].duration = Seconds(2.);
    verify_protected_clearance(&setup, &g, 0.01, 0.001, &ctx).unwrap();
    // Both end poses clear the tab; the intervening fed flute cuts it.
    g.start.tip.z = 4.;
    g.segments[0].end.tip.z = 4.;
    assert!(g.start.tip.y.abs() - 1.5 > 5.);
    assert!(g.segments[0].end.tip.y.abs() - 1.5 > 5.);
    assert!(
        verify_protected_clearance(&setup, &g, 0.01, 0.001, &ctx)
            .unwrap_err()
            .to_string()
            .contains("protected")
    );
    // Reaching behind the final plane is a whole-part gouge even when the
    // selected/preview surface might omit that part of the source.
    g.start.tip = Point3::new(49., -2., 0.);
    g.segments[0].end.tip = Point3::new(49., 4., 0.);
    assert!(
        verify_protected_clearance(&setup, &g, 0.01, 0., &ctx)
            .unwrap_err()
            .to_string()
            .contains("whole part")
    );
    // On an ordinary planar side, zero allowance is valid within OP-12's
    // model tolerance; the same tolerance never permits cutting a tab.
    g.start.tip = Point3::new(20., -10., 7.);
    g.segments[0].end.tip = Point3::new(20., 10., 7.);
    verify_protected_clearance(&setup, &g, 0.01, 0.001, &ctx).unwrap();
    g.start.tip.z = 6.98;
    g.segments[0].end.tip.z = 6.98;
    assert!(verify_protected_clearance(&setup, &g, 0.01, 0., &ctx).is_err());
}

#[test]
fn req_op_24_fed_shank_requires_actual_prior_removal_not_candidate_passes() {
    use openchuck_path::facing::motion::verify_shank_clearance;
    use openchuck_path::facing::plan::FacingSetup;
    use openchuck_types::mesh::TriangleMesh;
    use openchuck_types::stock::Stock;
    use openchuck_types::tool::{Tool, ToolShape};
    let m = machine();
    let part = TriangleMesh {
        vertices: vec![
            Point3::new(10., -2., -3.),
            Point3::new(50., 4., 7.),
            Point3::new(50., -2., 7.),
        ],
        triangles: vec![[0, 1, 2]],
    };
    let stock = Stock::Box {
        bounds: Aabb {
            min: Point3::new(0., -10., -12.),
            max: Point3::new(60., 10., 12.),
        },
    };
    let tool = Tool {
        shape: ToolShape::FlatEnd,
        diameter: Millimetres(3.),
        flute_length: Millimetres(10.),
        overall_length: Millimetres(30.),
        shank_diameter: Millimetres(3.),
        flute_count: 2,
        material: None,
        centre_cutting: None,
    };
    let setup = FacingSetup {
        part: &part,
        stock: &stock,
        tool: &tool,
        machine: &m,
        tabs: &[],
        protected: &[],
        held: [Some((0., 5.)), None],
        projection_error: Some(0.),
    };
    let token = CancelToken::new();
    let ctx = JobContext::new(&token, &NoProgress);
    let mut g = group();
    g.start.tip = Point3::new(60.5, -10., 2.);
    g.segments[0].end.tip = Point3::new(60.5, 10., 2.);
    g.segments[0].duration = Seconds(2.);
    // Flute2..12 clears the upper band across X59..62. Then an air
    // excursion to the lower band and a second across-going cut.
    for (kind, p) in [
        (MoveKind::Cutting, Point3::new(65., 10., 2.)),
        (MoveKind::Rapid, Point3::new(65., -10., 2.)),
        (MoveKind::Rapid, Point3::new(65., -10., -8.)),
        (MoveKind::Cutting, Point3::new(60.5, -10., -8.)),
        (MoveKind::Cutting, Point3::new(60.5, 10., -8.)),
    ] {
        let mut segment = g.segments[0];
        segment.kind = kind;
        segment.contact_feed = kind.is_fed().then_some(FeedRate(600.));
        segment.end.tip = p;
        segment.duration = Seconds(2.);
        g.segments.push(segment);
    }
    verify_shank_clearance(&setup, &g, 0., &ctx).unwrap();
    // A90mm round stock has radius45, not the63.64mm radius of its YZ box.
    // At tip height50 the entire flat tool stays above it during indexing.
    let cylinder = Stock::Cylinder {
        diameter: Millimetres(90.),
        length: Millimetres(60.),
        x_start: Millimetres(0.),
    };
    let round_setup = FacingSetup {
        stock: &cylinder,
        ..setup
    };
    let rotary = openchuck_path::RotaryMapping::of_profile(&m);
    let mut clear = group();
    clear.start.tip.z = 50.;
    clear.segments[0].kind = MoveKind::Rapid;
    clear.segments[0].contact_feed = None;
    clear.segments[0].duration = Seconds(100.);
    clear.segments[0].end = ToolPose {
        tip: rotary.to_model(
            openchuck_path::rotary::AxisPosition {
                along: 10.,
                across: 0.,
                height: 50.,
            },
            Degrees(90.),
        ),
        axis: rotary.tool_axis(Degrees(90.)),
        a: Degrees(90.),
    };
    openchuck_path::facing::motion::verify_rapid_clearance(&round_setup, &clear, 0., &ctx).unwrap();
    // Restrict the across span to +/-5 while original stock remains +/-10.
    // The upper flute's circular end caps, not just its inscribed rectangle,
    // clear the lower shank at those interior endpoints.
    let mut interior = g.clone();
    interior.start.tip.y *= 0.5;
    for segment in &mut interior.segments {
        segment.end.tip.y *= 0.5;
    }
    let mut approach = interior.segments[0];
    approach.end = interior.start;
    interior.start.tip.x = 65.;
    interior.segments.insert(0, approach);
    verify_shank_clearance(&setup, &interior, 0., &ctx).unwrap();
    let mut uncut = interior;
    for segment in &mut uncut.segments[..2] {
        segment.kind = MoveKind::Rapid;
        segment.contact_feed = None;
    }
    assert!(
        verify_shank_clearance(&setup, &uncut, 0., &ctx).is_err(),
        "No proposed cap clearance may replace actual upper flute sweeps"
    );
    // Omitting the ACTUAL top cut must not borrow the proposed plan's
    // clearance. Its endpoints are unchanged: the lower shank hits stock.
    let mut missing = g.clone();
    missing.start.tip = Point3::new(65., -10., -8.);
    missing.segments = g.segments[4..].to_vec();
    assert!(
        verify_shank_clearance(&setup, &missing, 0., &ctx)
            .unwrap_err()
            .to_string()
            .contains("shank")
    );
    // A raised top cut leaves a thin internal band that the lower shank
    // crosses. No sampled stock grid is allowed to overlook that band.
    let mut raised = g.clone();
    raised.start.tip.z += 0.00001;
    for s in &mut raised.segments[..3] {
        s.end.tip.z += 0.00001;
    }
    assert!(verify_shank_clearance(&setup, &raised, 0., &ctx).is_err());
    // Rapid motion never supplies removal evidence.
    let mut rapid = g;
    rapid.segments[0].kind = MoveKind::Rapid;
    rapid.segments[0].contact_feed = None;
    assert!(verify_shank_clearance(&setup, &rapid, 0., &ctx).is_err());
}

#[test]
fn req_op_24_req_nfr_04_axially_separated_end_motion_does_not_rescan_all_source_facets_per_move() {
    use openchuck_path::facing::motion::verify_protected_clearance;
    use openchuck_path::facing::plan::FacingSetup;
    use openchuck_types::mesh::TriangleMesh;
    use openchuck_types::stock::Stock;
    use openchuck_types::tool::{Tool, ToolShape};
    // Repeated source facets have the same analytic X bound50. The proof
    // must use that bound for these clear motions, not exhaust an artificial
    // per-move facet budget and call a clearly separated end unreachable.
    let part = TriangleMesh {
        vertices: vec![
            Point3::new(10., -2., -3.),
            Point3::new(50., 4., 7.),
            Point3::new(50., -2., 7.),
        ],
        triangles: vec![[0, 1, 2]; 300_000],
    };
    let stock = Stock::Box {
        bounds: Aabb {
            min: Point3::new(0., -10., -12.),
            max: Point3::new(60., 10., 12.),
        },
    };
    let tool = Tool {
        shape: ToolShape::FlatEnd,
        diameter: Millimetres(3.),
        flute_length: Millimetres(10.),
        overall_length: Millimetres(30.),
        shank_diameter: Millimetres(3.),
        flute_count: 2,
        material: None,
        centre_cutting: None,
    };
    let m = machine();
    let setup = FacingSetup {
        part: &part,
        stock: &stock,
        tool: &tool,
        machine: &m,
        tabs: &[],
        protected: &[],
        held: [Some((0., 5.)), None],
        projection_error: Some(0.),
    };
    let mut g = group();
    g.start.tip = Point3::new(70., -10., 0.);
    let segment = g.segments[0];
    g.segments = (0..30)
        .map(|i| {
            let mut s = segment;
            s.end.tip = Point3::new(70., if i % 2 == 0 { 10. } else { -10. }, 0.);
            s.duration = Seconds(2.);
            s
        })
        .collect();
    let token = CancelToken::new();
    verify_protected_clearance(&setup, &g, 0.01, 0., &JobContext::new(&token, &NoProgress))
        .unwrap();
}
