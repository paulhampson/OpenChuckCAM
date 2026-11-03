// SPDX-License-Identifier: AGPL-3.0-or-later
//! End candidates use true flank geometry, equal axial slices and finite
//! flute bands. These checks do not certify assembled motion clearance.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::float_cmp)]
use openchuck_path::facing::end::{EndFacingParameters, plan_end};
use openchuck_path::facing::extents::FacingExtent;
use openchuck_path::facing::plan::FacingSetup;
use openchuck_types::geometry::{Aabb, Point3};
use openchuck_types::job::{CancelToken, JobContext, NoProgress};
use openchuck_types::machine::MachineProfile;
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::stock::Stock;
use openchuck_types::tool::{Tool, ToolShape};
use openchuck_types::units::Millimetres;

fn part() -> TriangleMesh {
    TriangleMesh {
        vertices: vec![
            Point3::new(10., -2., -3.),
            Point3::new(50., 4., 7.),
            Point3::new(50., -2., 7.),
        ],
        triangles: vec![[0, 1, 2]],
    }
}
const FLUTE: f64 = 25.;
fn tool() -> Tool {
    Tool {
        shape: ToolShape::FlatEnd,
        diameter: Millimetres(3.),
        // Long enough to reach the 24 mm ends below from one view: an end
        // deeper than the flutes is faced from two (the owner's F1, #499).
        flute_length: Millimetres(FLUTE),
        overall_length: Millimetres(40.),
        shank_diameter: Millimetres(3.),
        flute_count: 2,
        material: None,
        centre_cutting: None,
    }
}
fn settings() -> EndFacingParameters {
    EndFacingParameters {
        extent: FacingExtent::WholeStock,
        margin: 0.,
        axial_stepdown: 2.,
        flute_step: 10.,
        start_angle: openchuck_types::units::Degrees(0.),
        max_angle_step: openchuck_types::units::Degrees(5.),
        allowance: 0.2,
        tolerance: 0.01,
        approach: 1.,
    }
}

#[test]
fn req_op_24_end_flank_uses_equal_axial_slices_and_top_first_flute_bands() {
    let part = part();
    let tool = tool();
    let machine = MachineProfile::example_x_rotary();
    let stock = Stock::Box {
        bounds: Aabb {
            min: Point3::new(0., -10., -12.),
            max: Point3::new(60., 10., 12.),
        },
    };
    let setup = FacingSetup {
        part: &part,
        stock: &stock,
        tool: &tool,
        machine: &machine,
        tabs: &[],
        protected: &[],
        held: [Some((0., 5.)), None],
        projection_error: Some(0.),
    };
    let token = CancelToken::new();
    let ctx = JobContext::new(&token, &NoProgress);
    let plan = plan_end(&setup, &settings(), &ctx).unwrap();
    assert_eq!(plan.target.x, 50.2);
    assert_eq!(plan.target.length, 40.2);
    assert_eq!(plan.axial_planes.len(), 5);
    for (actual, expected) in plan
        .axial_planes
        .iter()
        .zip([58.04, 56.08, 54.12, 52.16, 50.2])
    {
        assert!((actual - expected).abs() < 1e-10);
    }
    // The bands reach 0.00125 (an eighth of the tolerance) past each edge
    // (issue #499); the 10 mm flute step leaves the 25 mm flutes overlapping.
    let heights = plan
        .bands
        .iter()
        .map(|band| band.heights)
        .collect::<Vec<_>>();
    let expected = [
        (2.00125, 12.00125),
        (-7.99875, 2.00125),
        (-12.00125, -7.99875),
    ];
    assert_eq!(heights.len(), expected.len());
    for (actual, expected) in heights.iter().zip(expected) {
        assert!(
            (actual.0 - expected.0).abs() < 1e-9 && (actual.1 - expected.1).abs() < 1e-9,
            "{heights:?}"
        );
    }
    assert_eq!(plan.passes.len(), 15);
    for (slice, passes) in plan.passes.chunks(3).enumerate() {
        for (band, pass) in passes.iter().enumerate() {
            assert_eq!(pass.points.len(), 4);
            assert_eq!(pass.points[0].tip.y, -10.);
            assert_eq!(pass.points.last().unwrap().tip.y, 10.);
            let cutting = pass
                .points
                .iter()
                .filter(|point| (point.contact.x - plan.axial_planes[slice]).abs() < 1e-12)
                .collect::<Vec<_>>();
            assert_eq!(cutting.len(), 2);
            for point in cutting {
                assert_eq!(point.a.0, 0.);
                assert!((point.tip.x - plan.axial_planes[slice] - 1.5).abs() < 1e-12);
                assert_eq!(point.tip.z, plan.bands[band].heights.0);
                assert!((point.contact.x - plan.axial_planes[slice]).abs() < 1e-12);
                assert!(point.contact.z >= point.tip.z && point.contact.z <= point.tip.z + FLUTE);
            }
        }
    }
    let reverse = FacingSetup {
        held: [None, Some((55., 60.))],
        ..setup
    };
    let plan = plan_end(&reverse, &settings(), &ctx).unwrap();
    assert_eq!(plan.target.x, 9.8);
    assert_eq!(*plan.axial_planes.last().unwrap(), 9.8);
    assert!(
        plan.passes
            .last()
            .unwrap()
            .points
            .iter()
            .any(|point| (point.tip.x - 8.3).abs() < 1e-12)
    );
}

#[test]
fn req_op_24_req_set_09_end_policy_and_cutting_diameter_limit_apply_before_candidates() {
    let part = part();
    let tool = tool();
    let machine = MachineProfile::example_x_rotary();
    let stock = Stock::Cylinder {
        diameter: Millimetres(24.),
        length: Millimetres(60.),
        x_start: Millimetres(0.),
    };
    let mut setup = FacingSetup {
        part: &part,
        stock: &stock,
        tool: &tool,
        machine: &machine,
        tabs: &[],
        protected: &[],
        held: [None, None],
        projection_error: Some(0.),
    };
    let token = CancelToken::new();
    let ctx = JobContext::new(&token, &NoProgress);
    assert!(
        plan_end(&setup, &settings(), &ctx)
            .unwrap_err()
            .to_string()
            .contains("Define which end is held")
    );
    setup.held = [Some((0., 5.)), Some((55., 60.))];
    assert!(
        plan_end(&setup, &settings(), &ctx)
            .unwrap_err()
            .to_string()
            .contains("both ends are held")
    );
    setup.held = [Some((0., 5.)), None];
    let mut bad = settings();
    bad.axial_stepdown = 3.01;
    assert!(plan_end(&setup, &bad, &ctx).is_err());
    token.cancel();
    assert!(matches!(
        plan_end(&setup, &settings(), &ctx),
        Err(openchuck_path::PathError::Cancelled)
    ));
}

#[test]
fn req_op_24_req_op_21_outward_widening_tab_refuses_with_independent_collision_witness() {
    use openchuck_path::Solid;
    use openchuck_types::geometry::UnitVec3;
    let part = part();
    let tool = tool();
    let machine = MachineProfile::example_x_rotary();
    let stock = Stock::Cylinder {
        diameter: Millimetres(24.),
        length: Millimetres(60.),
        x_start: Millimetres(0.),
    };
    let tab = Solid::frustum(Point3::new(50., 0., 0.), UnitVec3::X, 10., 5., 7.).unwrap();
    let setup = FacingSetup {
        part: &part,
        stock: &stock,
        tool: &tool,
        machine: &machine,
        tabs: &[tab],
        protected: &[tab],
        held: [Some((0., 5.)), None],
        projection_error: Some(0.),
    };
    let token = CancelToken::new();
    let ctx = JobContext::new(&token, &NoProgress);
    let mut p = settings();
    p.allowance = 0.;
    // At the target face this point lies outside the protected radius5.
    // Any A orientation has the same cutter cross-section along X. A
    // point2.99mm outward lies strictly inside both the Ø3 cutter and tab.
    let requested = Point3::new(50., 0., 5.3);
    let collision = Point3::new(52.99, 0., 5.3);
    assert!(!tab.contains(requested));
    assert!(tab.contains(collision));
    assert!((collision.x - 51.5).abs() < 1.5);
    let error = plan_end(&setup, &p, &ctx).unwrap_err().to_string();
    assert!(error.contains("outward-widening tab"), "{error}");
    assert!(error.contains("0.600"), "{error}");
    // A coverage rectangle wholly inside the protected end disc does not
    // contain the adjoining annulus; do not claim that annulus is requested.
    let small = TriangleMesh {
        vertices: vec![
            Point3::new(10., -1., -1.),
            Point3::new(50., 1., 1.),
            Point3::new(50., -1., 1.),
        ],
        triangles: vec![[0, 1, 2]],
    };
    let inside = FacingSetup {
        part: &small,
        ..setup
    };
    p.extent = FacingExtent::Part;
    // Candidate success is NOT a clearance/completion certificate.
    assert!(plan_end(&inside, &p, &ctx).is_ok());
}

#[test]
fn req_op_24_req_op_21_frustum_obstruction_mirrors_with_held_end_and_off_axis_tab() {
    use openchuck_path::Solid;
    use openchuck_types::geometry::UnitVec3;
    let part = part();
    let tool = tool();
    let machine = MachineProfile::example_x_rotary();
    let stock = Stock::Cylinder {
        diameter: Millimetres(30.),
        length: Millimetres(60.),
        x_start: Millimetres(0.),
    };
    let token = CancelToken::new();
    let ctx = JobContext::new(&token, &NoProgress);
    let mut p = settings();
    p.allowance = 0.;
    for (tab, held, face, witness) in [
        (
            Solid::frustum(Point3::new(0., 0., 0.), UnitVec3::X, 10., 7., 5.).unwrap(),
            [None, Some((55., 60.))],
            Point3::new(10., 0., 5.3),
            Point3::new(7.01, 0., 5.3),
        ),
        (
            Solid::frustum(Point3::new(50., 2., -3.), UnitVec3::X, 10., 5., 7.).unwrap(),
            [Some((0., 5.)), None],
            Point3::new(50., 2., 2.3),
            Point3::new(52.99, 2., 2.3),
        ),
    ] {
        assert!(!tab.contains(face));
        assert!(tab.contains(witness));
        let centre_x = face.x + if face.x > witness.x { -1.5 } else { 1.5 };
        assert!((centre_x - witness.x).abs() < 1.5);
        let setup = FacingSetup {
            part: &part,
            stock: &stock,
            tool: &tool,
            machine: &machine,
            tabs: &[tab],
            protected: &[tab],
            held,
            projection_error: Some(0.),
        };
        let error = plan_end(&setup, &p, &ctx).unwrap_err().to_string();
        assert!(error.contains("outward-widening tab"), "{error}");
    }
}

#[test]
fn req_op_24_req_op_21_cylindrical_tab_flanks_stay_outside_and_cover_the_remaining_end() {
    use openchuck_path::Solid;
    use openchuck_types::geometry::UnitVec3;
    let part = part();
    let tool = tool();
    let machine = MachineProfile::example_x_rotary();
    let stock = Stock::Cylinder {
        diameter: Millimetres(24.),
        length: Millimetres(60.),
        x_start: Millimetres(0.),
    };
    let tab = Solid::cylinder(Point3::new(50., 0., 0.), UnitVec3::X, 10., 5.).unwrap();
    let setup = FacingSetup {
        part: &part,
        stock: &stock,
        tool: &tool,
        machine: &machine,
        tabs: &[tab],
        protected: &[tab],
        held: [Some((0., 5.)), None],
        projection_error: Some(0.),
    };
    let token = CancelToken::new();
    let ctx = JobContext::new(&token, &NoProgress);
    let mut p = settings();
    p.allowance = 0.;
    let plan = plan_end(&setup, &p, &ctx).unwrap();
    let mut final_sweeps = vec![];
    let mut angles = std::collections::BTreeSet::new();
    for pass in &plan.passes {
        let first = pass.points.first().unwrap();
        let last = pass.points.last().unwrap();
        // Independent X-rotary basis: positive A90 presents model +Y.
        let angle = first.a.0.to_radians();
        let (ny, nz) = (angle.sin(), angle.cos());
        let height = first.tip.y * ny + first.tip.z * nz;
        assert!(
            height >= 5.,
            "A{} tool base enters the tab's supporting halfspace at{height}",
            first.a.0
        );
        assert_eq!(first.a, last.a);
        angles.insert(first.a.0.to_bits());
        for pair in pass.points.windows(2) {
            if (pair[0].contact.x - 50.).abs() < 1e-12 && (pair[1].contact.x - 50.).abs() < 1e-12 {
                let u = |point: Point3| point.y * nz - point.z * ny;
                let (a, b) = (u(pair[0].tip), u(pair[1].tip));
                final_sweeps.push((ny, nz, height, a.min(b), a.max(b)));
            }
        }
    }
    assert!(angles.len() >= 71);
    // Independent polygon/slab analysis also checks the unsampled region
    // between those witnesses, over the COMPLETE axial removal depth.
    openchuck_path::facing::coverage::verify_end_coverage(
        &setup,
        plan.end,
        plan.target,
        plan.coverage,
        &actual_sweeps(&plan.passes, &machine),
        0.01,
        0.,
        &ctx,
    )
    .unwrap();
    openchuck_path::facing::motion::verify_protected_clearance(
        &setup,
        &actual_sweeps(&plan.passes, &machine),
        0.01,
        0.,
        &ctx,
    )
    .unwrap();
    openchuck_path::facing::motion::verify_shank_clearance(
        &setup,
        &actual_sweeps(&plan.passes, &machine),
        0.,
        &ctx,
    )
    .unwrap();
    let head = Solid::cylinder(Point3::new(5., 0., 0.), UnitVec3::X, 5., 3.).unwrap();
    let both_tabs = FacingSetup {
        tabs: &[head, tab],
        protected: &[head, tab],
        ..setup
    };
    assert!(
        plan_end(&both_tabs, &p, &ctx).is_ok(),
        "An unreachable opposite-end tab must not become an unheld-end routing obstacle"
    );
    // Check the ACTUAL finite flute sweeps, not the requested polygons or
    // a view count. Every sampled point outside the permitted0.01mm halo
    // must be covered, including the stock bounding rectangle's corners.
    for iy in -24..=24 {
        for iz in -24..=24 {
            let (y, z) = (f64::from(iy) * 0.5, f64::from(iz) * 0.5);
            if y.hypot(z) <= 5.01 {
                continue;
            }
            assert!(
                final_sweeps.iter().any(|&(ny, nz, height, u0, u1)| {
                    let h = y * ny + z * nz;
                    let u = y * nz - z * ny;
                    h >= height - 1e-10
                        && h <= height + FLUTE + 1e-10
                        && u >= u0 - 1e-10
                        && u <= u1 + 1e-10
                }),
                "Unfaced point ({y},{z})"
            );
        }
    }
}

#[test]
fn req_op_24_req_nfr_07_end_views_refuse_angles_without_a_geometric_precision_bound() {
    let part = part();
    let tool = tool();
    let machine = MachineProfile::example_x_rotary();
    let stock = Stock::Cylinder {
        diameter: Millimetres(24.),
        length: Millimetres(60.),
        x_start: Millimetres(0.),
    };
    let setup = FacingSetup {
        part: &part,
        stock: &stock,
        tool: &tool,
        machine: &machine,
        tabs: &[],
        protected: &[],
        held: [Some((0., 5.)), None],
        projection_error: Some(0.),
    };
    let token = CancelToken::new();
    let ctx = JobContext::new(&token, &NoProgress);
    let mut p = settings();
    p.start_angle = openchuck_types::units::Degrees(1e30);
    assert!(plan_end(&setup, &p, &ctx).is_err());
}

#[test]
fn req_op_24_req_op_21_end_tab_views_follow_negative_a_sense_offset_and_maximum_step() {
    use openchuck_path::Solid;
    use openchuck_types::geometry::UnitVec3;
    let part = part();
    let tool = tool();
    let mut machine = MachineProfile::example_x_rotary();
    machine.a_sense = openchuck_types::machine::ASense::Negative;
    let stock = Stock::Cylinder {
        diameter: Millimetres(24.),
        length: Millimetres(60.),
        x_start: Millimetres(0.),
    };
    let tab = Solid::cylinder(Point3::new(50., 2., -3.), UnitVec3::X, 10., 3.).unwrap();
    let setup = FacingSetup {
        part: &part,
        stock: &stock,
        tool: &tool,
        machine: &machine,
        tabs: &[tab],
        protected: &[tab],
        held: [Some((0., 5.)), None],
        projection_error: Some(0.),
    };
    let token = CancelToken::new();
    let ctx = JobContext::new(&token, &NoProgress);
    let mut p = settings();
    p.allowance = 0.;
    p.start_angle = openchuck_types::units::Degrees(30.);
    p.max_angle_step = openchuck_types::units::Degrees(2.);
    let plan = plan_end(&setup, &p, &ctx).unwrap();
    let mut angles = plan.bands.iter().map(|b| b.angle.0).collect::<Vec<_>>();
    angles.dedup();
    assert_eq!(angles.len(), 180);
    assert_eq!(angles[0], 30.);
    assert!(angles.windows(2).all(|w| (w[1] - w[0] + 2.).abs() < 1e-10));
    assert!(plan.tab_residual_bound.unwrap() <= 0.005);
    openchuck_path::facing::coverage::verify_end_coverage(
        &setup,
        plan.end,
        plan.target,
        plan.coverage,
        &actual_sweeps(&plan.passes, &machine),
        0.01,
        0.,
        &ctx,
    )
    .unwrap();
    for pass in &plan.passes {
        for point in &pass.points {
            let theta = -point.a.0.to_radians();
            let h = (point.tip.y - 2.) * theta.sin() + (point.tip.z + 3.) * theta.cos();
            assert!(h >= 3., "{h}");
        }
    }
}

#[test]
fn req_op_24_stored_end_settings_control_axial_depth_flute_spacing_and_start_view() {
    use openchuck_path::{Facing, ParameterValue, Strategy};
    let values = [
        ("shape", ParameterValue::Text("end".into())),
        ("feed", ParameterValue::Number(600.)),
        ("plunge_feed", ParameterValue::Number(200.)),
        ("stepover", ParameterValue::Number(1.)),
        ("stepdown", ParameterValue::Number(2.)),
        ("stock_to_leave", ParameterValue::Number(0.2)),
        ("start_angle", ParameterValue::Number(30.)),
        ("max_angle_step", ParameterValue::Number(2.)),
    ]
    .into_iter()
    .map(|(k, v)| (k.into(), v))
    .collect();
    let p = EndFacingParameters::read(&Facing.resolve_parameters(&values).unwrap()).unwrap();
    let part = part();
    let tool = tool();
    let machine = MachineProfile::example_x_rotary();
    let stock = Stock::Cylinder {
        diameter: Millimetres(24.),
        length: Millimetres(60.),
        x_start: Millimetres(0.),
    };
    let setup = FacingSetup {
        part: &part,
        stock: &stock,
        tool: &tool,
        machine: &machine,
        tabs: &[],
        protected: &[],
        held: [Some((0., 5.)), None],
        projection_error: Some(0.),
    };
    let token = CancelToken::new();
    let ctx = JobContext::new(&token, &NoProgress);
    let plan = plan_end(&setup, &p, &ctx).unwrap();
    assert_eq!(plan.target.x, 50.2);
    assert_eq!(plan.axial_planes.len(), 5);
    // At A 30 the stock's bounding square is about 32.8 mm deep across the
    // tool's axis, more than the 25 mm flutes reach from one view, so the
    // end is faced from A 30 and A 210 (the owner's F1, issue #499).
    assert!(plan.bands.iter().all(
        |b| b.heights.1 - b.heights.0 <= 1. + 1e-12 && (b.angle.0 == 30. || b.angle.0 == 210.)
    ));
    assert!(plan.bands.iter().any(|b| b.angle.0 == 30.));
    assert!(
        plan.passes
            .iter()
            .flat_map(|pass| &pass.points)
            .all(|p| p.a.0 == 30. || p.a.0 == 210.)
    );
}

#[test]
fn req_op_24_req_op_14_end_slice_starts_and_ends_with_the_complete_tool_outside_stock() {
    let part = part();
    let tool = tool();
    let machine = MachineProfile::example_x_rotary();
    let stock = Stock::Cylinder {
        diameter: Millimetres(24.),
        length: Millimetres(50.5),
        x_start: Millimetres(0.),
    };
    let setup = FacingSetup {
        part: &part,
        stock: &stock,
        tool: &tool,
        machine: &machine,
        tabs: &[],
        protected: &[],
        held: [Some((0., 5.)), None],
        projection_error: Some(0.),
    };
    let token = CancelToken::new();
    let ctx = JobContext::new(&token, &NoProgress);
    let plan = plan_end(&setup, &settings(), &ctx).unwrap();
    // The shallow0.3mm axial slice puts most of the Ø3 tool beyond the
    // end. The shared entry/retract must start/end in genuinely clear air,
    // so no sparse stock-line query authorises a rapid through its edge.
    for pass in &plan.passes {
        for point in [pass.points.first().unwrap(), pass.points.last().unwrap()] {
            assert!(
                point.tip.x - 1.5 > 50.5,
                "Full tool reaches stock at entry/exit X{}",
                point.tip.x
            );
        }
    }
}

#[test]
fn req_op_24_req_op_14_actual_assembled_end_rapids_have_continuous_complete_tool_clearance() {
    use openchuck_path::facing::coverage::verify_end_coverage;
    use openchuck_path::facing::motion::{
        verify_protected_clearance, verify_rapid_clearance, verify_shank_clearance,
    };
    use openchuck_path::params::{CommonParameters, EntryParameters};
    use openchuck_path::rotary::RotaryMapping;
    use openchuck_path::{
        AcceptAll, AnalyticStock, Facing, MeshSurface, ParameterValue, Strategy, StrategyInput,
    };
    use openchuck_types::ir::{MoveKind, PathTags};
    use openchuck_types::units::Seconds;
    let part = part();
    let tool = tool();
    let machine = MachineProfile::example_x_rotary();
    let stock = Stock::Cylinder {
        diameter: Millimetres(24.),
        length: Millimetres(50.5),
        x_start: Millimetres(0.),
    };
    let setup = FacingSetup {
        part: &part,
        stock: &stock,
        tool: &tool,
        machine: &machine,
        tabs: &[],
        protected: &[],
        held: [Some((0., 5.)), None],
        projection_error: Some(0.),
    };
    let token = CancelToken::new();
    let ctx = JobContext::new(&token, &NoProgress);
    let plan = plan_end(&setup, &settings(), &ctx).unwrap();
    let values = [
        ("shape", ParameterValue::Text("end".into())),
        ("feed", ParameterValue::Number(600.)),
        ("plunge_feed", ParameterValue::Number(200.)),
        ("stepover", ParameterValue::Number(1.)),
        ("stepdown", ParameterValue::Number(2.)),
    ]
    .into_iter()
    .map(|(k, v)| (k.into(), v))
    .collect();
    let parameters = Facing.resolve_parameters(&values).unwrap();
    let surface = MeshSurface::new(&part).unwrap();
    let profile = tool.profile().unwrap();
    let original = AnalyticStock::Cylinder {
        radius: 12.,
        x_start: 0.,
        x_end: 50.5,
    };
    let tags = PathTags {
        operation: "op".into(),
        setup: "setup".into(),
        tool: "tool".into(),
    };
    let input = StrategyInput {
        surface: &surface,
        stock: &original,
        collision: &AcceptAll,
        tool: &profile,
        rotary: RotaryMapping::of_profile(&machine),
        clearance: machine.clearance,
        max_feeds: machine.max_feeds,
        tags: &tags,
        entry_safety: None,
        facing: Some(&setup),
        keep_out: &[],
        shank: None,
    };
    let path = openchuck_path::assemble::assemble_with_entry(
        &plan.passes,
        &input,
        &CommonParameters::read(&parameters).unwrap(),
        &EntryParameters::read(&parameters).unwrap(),
        &ctx,
    )
    .unwrap();
    verify_rapid_clearance(&setup, &path.group, 0., &ctx).unwrap();
    verify_protected_clearance(&setup, &path.group, 0.01, 0., &ctx).unwrap();
    verify_shank_clearance(&setup, &path.group, 0., &ctx).unwrap();
    verify_end_coverage(
        &setup,
        plan.end,
        plan.target,
        plan.coverage,
        &path.group,
        0.01,
        0.,
        &ctx,
    )
    .unwrap();
    // The production Strategy hook must use these independent checks too;
    // a default no-op would wrongly accept the strict stock/tool witness below.
    let mut checked_values = values.clone();
    checked_values.insert("stock_to_leave".into(), ParameterValue::Number(0.2));
    checked_values.insert("stepover".into(), ParameterValue::Number(10.));
    let checked_parameters = Facing.resolve_parameters(&checked_values).unwrap();
    Facing
        .validate_path(&input, &checked_parameters, &path, &ctx)
        .unwrap();
    let generated = openchuck_path::generate(&Facing, &input, &checked_values, &ctx).unwrap();
    Facing
        .validate_path(&input, &checked_parameters, &generated, &ctx)
        .unwrap();
    let unknown = FacingSetup {
        projection_error: None,
        ..setup
    };
    let unknown_input = StrategyInput {
        facing: Some(&unknown),
        ..input
    };
    assert!(
        Facing
            .validate_path(&unknown_input, &checked_parameters, &path, &ctx)
            .is_err(),
        "Unknown whole-source accuracy cannot certify completion"
    );
    let different_tool = Tool {
        shape: openchuck_types::tool::ToolShape::BallEnd,
        ..tool.clone()
    }
    .profile()
    .unwrap();
    let mismatched = StrategyInput {
        tool: &different_tool,
        ..input
    };
    assert!(
        Facing
            .validate_path(&mismatched, &checked_parameters, &path, &ctx)
            .is_err(),
        "Ball input cannot borrow a flat-flank setup's completion proof"
    );
    let mut unfinished = path.group.clone();
    // A raised final band leaves a thin INTERNAL stripe; plane tolerance is
    // not permission to ignore a ridge running back to the original stock.
    // The lowest band reaches the written-motion allowance below the edge
    // (issue #499), so it is raised by the tolerance, well past that.
    let top = plan
        .bands
        .iter()
        .map(|b| b.heights.0)
        .fold(f64::INFINITY, f64::min);
    let mut last_a = unfinished.start;
    for segment in &mut unfinished.segments {
        if segment.kind.is_fed() && last_a.tip.z == top && segment.end.tip.z == top {
            segment.end.tip.z += 0.01;
        }
        last_a = segment.end;
    }
    assert!(
        verify_end_coverage(
            &setup,
            plan.end,
            plan.target,
            plan.coverage,
            &unfinished,
            0.01,
            0.,
            &ctx
        )
        .is_err()
    );
    let mut previous = path.group.start;
    for segment in &path.group.segments {
        if segment.kind == MoveKind::Rapid {
            // Model X is linear even while A moves; these endpoint extrema
            // bound the full cylindrical tool for the ENTIRE rapid leg.
            assert!(previous.tip.x.min(segment.end.tip.x) - 1.5 > 50.5);
        }
        previous = segment.end;
    }
    let mut bad_path = path;
    let bad = &mut bad_path.group;
    let rapid = bad
        .segments
        .iter_mut()
        .find(|s| s.kind == MoveKind::Rapid)
        .unwrap();
    rapid.end.tip = Point3::new(50.4, 0., 0.);
    rapid.duration = Seconds(100.);
    // Strict witness inside original stock AND the Ø3 tool above its tip.
    let witness = Point3::new(50., 0., 0.1);
    assert!(witness.x < 50.5 && witness.y.hypot(witness.z) < 12.);
    assert!((witness.x - 50.4).abs() < 1.5 && witness.z > 0.);
    let error = verify_rapid_clearance(&setup, bad, 0., &ctx)
        .unwrap_err()
        .to_string();
    assert!(error.contains("rapid"), "{error}");
    assert!(
        Facing
            .validate_path(&input, &checked_parameters, &bad_path, &ctx)
            .is_err(),
        "The actual production hook must refuse a path whose full tool enters original stock"
    );
}

// This is an explicit IR sweep fixture, not a claim that candidate paths are
// exported. Production assembly and all clearance checks remain separate.
fn actual_sweeps(
    passes: &[openchuck_path::Pass],
    machine: &MachineProfile,
) -> openchuck_types::ir::ToolpathGroup {
    use openchuck_types::ir::{MoveKind, PathTags, Segment, ToolPose, ToolpathGroup};
    use openchuck_types::units::{FeedRate, Seconds};
    let rotary = openchuck_path::rotary::RotaryMapping::of_profile(machine);
    let pose = |p: &openchuck_path::CutPoint| ToolPose {
        tip: p.tip,
        a: p.a,
        axis: rotary.tool_axis(p.a),
    };
    let start = pose(&passes[0].points[0]);
    let segments = passes
        .iter()
        .flat_map(|p| &p.points)
        .map(|p| Segment {
            kind: MoveKind::Cutting,
            end: pose(p),
            contact_feed: Some(FeedRate(600.)),
            duration: Seconds(1000.),
            contact_length: Millimetres(0.),
        })
        .collect();
    ToolpathGroup {
        rotary_reset_after: Vec::new(),
        start,
        segments,
        tags: PathTags {
            operation: "op".into(),
            setup: "setup".into(),
            tool: "tool".into(),
        },
    }
}

#[test]
fn req_op_24_end_completion_requires_all_axial_stock_layers_in_both_directions() {
    use openchuck_path::facing::coverage::verify_end_coverage;
    let part = part();
    let tool = tool();
    let machine = MachineProfile::example_x_rotary();
    let stock = Stock::Box {
        bounds: Aabb {
            min: Point3::new(0., -10., -12.),
            max: Point3::new(60., 10., 12.),
        },
    };
    let token = CancelToken::new();
    let ctx = JobContext::new(&token, &NoProgress);
    for held in [[Some((0., 5.)), None], [None, Some((55., 60.))]] {
        let setup = FacingSetup {
            part: &part,
            stock: &stock,
            tool: &tool,
            machine: &machine,
            tabs: &[],
            protected: &[],
            held,
            projection_error: Some(0.),
        };
        let plan = plan_end(&setup, &settings(), &ctx).unwrap();
        let full = actual_sweeps(&plan.passes, &machine);
        verify_end_coverage(
            &setup,
            plan.end,
            plan.target,
            plan.coverage,
            &full,
            0.01,
            0.,
            &ctx,
        )
        .unwrap();
        let final_only = actual_sweeps(
            &plan.passes[plan.passes.len() - plan.bands.len()..],
            &machine,
        );
        let err = verify_end_coverage(
            &setup,
            plan.end,
            plan.target,
            plan.coverage,
            &final_only,
            0.01,
            0.,
            &ctx,
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("axial layer"), "{err}");
    }
}

#[test]
fn req_op_24_req_nfr_05_source_error_does_not_change_nominal_end_coordinate_or_length() {
    let part = part();
    let tool = tool();
    let machine = MachineProfile::example_x_rotary();
    let stock = Stock::Box {
        bounds: Aabb {
            min: Point3::new(0., -10., -12.),
            max: Point3::new(60., 10., 12.),
        },
    };
    let token = CancelToken::new();
    let ctx = JobContext::new(&token, &NoProgress);
    for (held, x, cut_x) in [
        ([Some((0., 5.)), None], 50.2, 50.201),
        ([None, Some((55., 60.))], 9.8, 9.799),
    ] {
        let setup = FacingSetup {
            part: &part,
            stock: &stock,
            tool: &tool,
            machine: &machine,
            tabs: &[],
            protected: &[],
            held,
            projection_error: Some(0.001),
        };
        let plan = plan_end(&setup, &settings(), &ctx).unwrap();
        assert_eq!(plan.target.x, x);
        assert_eq!(plan.target.length, 40.2);
        assert!((plan.axial_planes.last().unwrap() - cut_x).abs() < 1e-12);
    }
}

/// The air point beside the end keeps the operation's approach distance from
/// the stock's end face, as every other rapid keeps it above the stock
/// (OP-14; issue #499). A rapid that descends beside the end face then has
/// the same allowance for stock longer than modelled as one that stops
/// above the stock top.
#[test]
fn req_op_14_req_nfr_12_end_air_point_keeps_the_approach_distance_from_the_end_face() {
    let part = part();
    let tool = tool();
    let machine = MachineProfile::example_x_rotary();
    let stock = Stock::Box {
        bounds: Aabb {
            min: Point3::new(0., -10., -12.),
            max: Point3::new(60., 10., 12.),
        },
    };
    let token = CancelToken::new();
    let ctx = JobContext::new(&token, &NoProgress);
    for (held, outer, sign) in [
        ([Some((0., 5.)), None], 60., 1.),
        ([None, Some((55., 60.))], 0., -1.),
    ] {
        let setup = FacingSetup {
            part: &part,
            stock: &stock,
            tool: &tool,
            machine: &machine,
            tabs: &[],
            protected: &[],
            held,
            projection_error: Some(0.),
        };
        for approach in [1., 2.5] {
            let plan = plan_end(
                &setup,
                &EndFacingParameters {
                    approach,
                    ..settings()
                },
                &ctx,
            )
            .unwrap();
            for pass in &plan.passes {
                for point in [pass.points.first().unwrap(), pass.points.last().unwrap()] {
                    let gap = sign * (point.tip.x - outer) - 1.5;
                    assert!(
                        gap >= approach - 1e-9,
                        "the tool is {gap} mm from the end face, less than the approach distance {approach}"
                    );
                }
            }
        }
        // A zero approach distance still leaves the tool clear of the face.
        let plan = plan_end(
            &setup,
            &EndFacingParameters {
                approach: 0.,
                ..settings()
            },
            &ctx,
        )
        .unwrap();
        let first = plan.passes[0].points[0].tip.x;
        assert!(sign * (first - outer) - 1.5 > 0.);
    }
}

/// The coordinates a controller reads back from written G-code are not the
/// planned ones: words are rounded and the Z1 reads single-precision floats.
/// The outermost End bands therefore reach one eighth of the tolerance past
/// the requested edge (decision 0252's written-motion share), so the motion
/// read back still covers it (OP-24, NFR-05; issue #499: the Baluster End
/// facing could not be exported). A band raised by more still refuses.
#[test]
fn req_op_24_req_nfr_05_end_bands_cover_the_edge_with_the_written_motion_allowance() {
    use openchuck_path::facing::coverage::verify_end_coverage;
    let part = part();
    let tool = tool();
    let machine = MachineProfile::example_x_rotary();
    let stock = Stock::Box {
        bounds: Aabb {
            min: Point3::new(0., -10., -12.),
            max: Point3::new(60., 10., 12.),
        },
    };
    let token = CancelToken::new();
    let ctx = JobContext::new(&token, &NoProgress);
    let setup = FacingSetup {
        part: &part,
        stock: &stock,
        tool: &tool,
        machine: &machine,
        tabs: &[],
        protected: &[],
        held: [Some((0., 5.)), None],
        projection_error: Some(0.),
    };
    let plan = plan_end(&setup, &settings(), &ctx).unwrap();
    let lowest = plan
        .bands
        .iter()
        .map(|b| b.heights.0)
        .fold(f64::INFINITY, f64::min);
    let highest = plan
        .bands
        .iter()
        .map(|b| b.heights.1)
        .fold(f64::NEG_INFINITY, f64::max);
    let allowance = 0.125 * settings().tolerance;
    assert!((lowest - (-12. - allowance)).abs() < 1e-12, "{lowest}");
    assert!((highest - (12. + allowance)).abs() < 1e-12, "{highest}");
    // Neighbouring bands overlap by at least the allowance as well.
    for pair in plan.bands.windows(2) {
        assert!(pair[1].heights.0 + FLUTE >= pair[0].heights.0 + 2. * allowance - 1e-12);
    }
    let raised = |by: f64| {
        let mut group = actual_sweeps(&plan.passes, &machine);
        for segment in &mut group.segments {
            if segment.end.tip.z == lowest {
                segment.end.tip.z += by;
            }
        }
        if group.start.tip.z == lowest {
            group.start.tip.z += by;
        }
        group
    };
    let check = |group: &openchuck_types::ir::ToolpathGroup| {
        verify_end_coverage(
            &setup,
            plan.end,
            plan.target,
            plan.coverage,
            group,
            0.01,
            0.,
            &ctx,
        )
    };
    check(&raised(0.)).unwrap();
    // The Baluster's -17.5 read back as -17.499999999999993 through the
    // LinuxCNC frame, and as -17.4999985 through the Z1's floats; a word
    // rounded to three places moves by up to 0.0005.
    for by in [7e-15, 1.5e-6, 0.0005] {
        check(&raised(by)).unwrap();
    }
    let error = check(&raised(allowance + 1e-6)).unwrap_err().to_string();
    assert!(error.contains("unfinished"), "{error}");
}

fn end_setup<'a>(
    part: &'a TriangleMesh,
    stock: &'a Stock,
    tool: &'a Tool,
    machine: &'a MachineProfile,
) -> FacingSetup<'a> {
    FacingSetup {
        part,
        stock,
        tool,
        machine,
        tabs: &[],
        protected: &[],
        held: [Some((0., 5.)), None],
        projection_error: Some(0.),
    }
}

/// The owner's decision F1 on issue #499: one view reaches at most the flute
/// length below the stock's top at that view. An end the flutes reach from
/// one view is faced from one, as before.
#[test]
fn req_op_24_an_end_the_flutes_reach_is_faced_from_one_view() {
    let part = part();
    let tool = tool();
    let machine = MachineProfile::example_x_rotary();
    let stock = Stock::Box {
        bounds: Aabb {
            min: Point3::new(0., -10., -12.),
            max: Point3::new(60., 10., 12.),
        },
    };
    let token = CancelToken::new();
    let ctx = JobContext::new(&token, &NoProgress);
    let plan = plan_end(
        &end_setup(&part, &stock, &tool, &machine),
        &settings(),
        &ctx,
    )
    .unwrap();
    assert!(plan.bands.iter().all(|b| b.angle.0 == 0.));
    assert!(plan.bands.iter().all(|b| 12. - b.heights.0 <= FLUTE));
}

/// F1: an end taller than the flutes reach from one view is faced from A and
/// A + 180 degrees, each view no deeper than the flute length below the
/// stock's top there and down past the axis, and the two together finish
/// the whole end; each axial slice turns A once (decision 0252's alternating
/// view order).
#[test]
fn req_op_24_req_nfr_12_an_end_deeper_than_the_flutes_is_faced_from_both_sides() {
    use openchuck_path::facing::coverage::verify_end_coverage;
    let part = part();
    let tool = Tool {
        flute_length: Millimetres(15.),
        ..tool()
    };
    let machine = MachineProfile::example_x_rotary();
    let stock = Stock::Box {
        bounds: Aabb {
            min: Point3::new(0., -10., -12.),
            max: Point3::new(60., 10., 12.),
        },
    };
    let token = CancelToken::new();
    let ctx = JobContext::new(&token, &NoProgress);
    let setup = end_setup(&part, &stock, &tool, &machine);
    let plan = plan_end(&setup, &settings(), &ctx).unwrap();
    let mut angles = plan.bands.iter().map(|b| b.angle.0).collect::<Vec<_>>();
    angles.dedup();
    assert_eq!(angles, [0., 180.], "{angles:?}");
    for band in &plan.bands {
        assert!(
            12. - band.heights.0 <= 15.,
            "{band:?} reaches past the flutes"
        );
    }
    for angle in [0., 180.] {
        let lowest = plan
            .bands
            .iter()
            .filter(|b| b.angle.0 == angle)
            .map(|b| b.heights.0)
            .fold(f64::INFINITY, f64::min);
        assert!(lowest < 0., "A {angle} reaches past the axis: {lowest}");
    }
    // One A flip per axial slice: the passes of a slice are all at one A
    // before they turn, and the next slice starts where the last one ended.
    let mut turns = 0;
    for pair in plan.passes.windows(2) {
        if pair[0].points[0].a != pair[1].points[0].a {
            turns += 1;
        }
    }
    assert_eq!(turns, plan.axial_planes.len(), "one A turn per slice");
    verify_end_coverage(
        &setup,
        plan.end,
        plan.target,
        plan.coverage,
        &actual_sweeps(&plan.passes, &machine),
        0.01,
        0.,
        &ctx,
    )
    .unwrap();
}

/// F1: an end deeper than twice the flutes cannot be faced from two sides,
/// and the operation refuses, naming the place, the depth, the flutes and
/// each remedy (decision 0344 item 7's style; NFR-07).
#[test]
fn req_nfr_07_req_op_24_an_end_deeper_than_two_sides_reach_refuses_with_remedies() {
    let part = part();
    let tool = Tool {
        flute_length: Millimetres(10.),
        ..tool()
    };
    let machine = MachineProfile::example_x_rotary();
    let stock = Stock::Box {
        bounds: Aabb {
            min: Point3::new(0., -10., -12.),
            max: Point3::new(60., 10., 12.),
        },
    };
    let token = CancelToken::new();
    let ctx = JobContext::new(&token, &NoProgress);
    let error = plan_end(
        &end_setup(&part, &stock, &tool, &machine),
        &settings(),
        &ctx,
    )
    .unwrap_err()
    .to_string();
    for words in [
        "X 50.200",
        "10.000 mm of flute",
        "A 0.000 and A 180.000",
        "longer flutes",
        "Box facing",
    ] {
        assert!(error.contains(words), "{words:?} missing from: {error}");
    }
}

/// Invalid input gives a clear refusal, never a crash (NFR-07): a negative
/// approach distance, and flutes too short for neighbouring bands to overlap
/// by the written-motion allowance (issue #499, decision 0501).
#[test]
fn req_nfr_07_end_refuses_a_negative_approach_and_flutes_too_short_to_overlap() {
    let part = part();
    let tool = tool();
    let machine = MachineProfile::example_x_rotary();
    let stock = Stock::Box {
        bounds: Aabb {
            min: Point3::new(0., -10., -12.),
            max: Point3::new(60., 10., 12.),
        },
    };
    let token = CancelToken::new();
    let ctx = JobContext::new(&token, &NoProgress);
    let setup = FacingSetup {
        part: &part,
        stock: &stock,
        tool: &tool,
        machine: &machine,
        tabs: &[],
        protected: &[],
        held: [Some((0., 5.)), None],
        projection_error: Some(0.),
    };
    let error = plan_end(
        &setup,
        &EndFacingParameters {
            approach: -1.,
            ..settings()
        },
        &ctx,
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("approach distance"), "{error}");
    let short = Tool {
        flute_length: Millimetres(0.002),
        ..tool.clone()
    };
    let error = plan_end(
        &FacingSetup {
            tool: &short,
            ..setup
        },
        &EndFacingParameters {
            flute_step: 0.001,
            ..settings()
        },
        &ctx,
    )
    .unwrap_err()
    .to_string();
    // With the flute-depth limit (decision 0504) the depth refusal names the
    // flutes first; either way the operation refuses with a reason.
    assert!(
        error.contains("too short") || error.contains("mm of flute"),
        "{error}"
    );
}

#[test]
fn req_op_24_facing_an_end_flush_with_the_stock_says_there_is_nothing_past_it() {
    // Issue #471 (the owner's note N2): the stock ends at the part's far end,
    // so there is nothing past it to face. The refusal says so in the user's
    // words, and how to set the stock if the real blank is longer.
    let part = part();
    let tool = tool();
    let machine = MachineProfile::example_x_rotary();
    let stock = Stock::Box {
        bounds: Aabb {
            min: Point3::new(0., -10., -12.),
            max: Point3::new(50., 10., 12.),
        },
    };
    let setup = FacingSetup {
        part: &part,
        stock: &stock,
        tool: &tool,
        machine: &machine,
        tabs: &[],
        protected: &[],
        held: [Some((0., 5.)), None],
        projection_error: Some(0.),
    };
    let token = CancelToken::new();
    let ctx = JobContext::new(&token, &NoProgress);
    let text = plan_end(&setup, &settings(), &ctx).unwrap_err().to_string();
    assert!(
        text.contains("the stock ends at the part's end at X 50.000")
            && text.contains("there is no stock past this end to face")
            && text.contains("If the real blank is longer, set the stock's length to match it"),
        "{text}"
    );
    // The same at the start end, when the far end is the held one.
    let flush_start = Stock::Box {
        bounds: Aabb {
            min: Point3::new(10., -10., -12.),
            max: Point3::new(60., 10., 12.),
        },
    };
    let start = FacingSetup {
        stock: &flush_start,
        held: [None, Some((55., 60.))],
        ..setup
    };
    let text = plan_end(&start, &settings(), &ctx).unwrap_err().to_string();
    assert!(
        text.contains("the stock ends at the part's end at X 10.000")
            && text.contains("there is no stock past this end to face"),
        "{text}"
    );
}
