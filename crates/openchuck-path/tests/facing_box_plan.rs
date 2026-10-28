// SPDX-License-Identifier: AGPL-3.0-or-later
//! Independent pass-position checks before assembly (OP-24, NFR-06).
#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::float_cmp,
    clippy::panic
)]
use openchuck_path::facing::extents::FacingExtent;
use openchuck_path::facing::geometry::LayerDistribution;
use openchuck_path::facing::plan::{BoxFacingParameters, FacingSetup, plan_box_sides};
use openchuck_path::facing::selection::{FaceOffset, SideSelection};
use openchuck_types::geometry::{Aabb, Point3};
use openchuck_types::job::{CancelToken, JobContext, NoProgress};
use openchuck_types::machine::MachineProfile;
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::stock::Stock;
use openchuck_types::tool::{Tool, ToolShape};
use openchuck_types::units::Millimetres;

fn tool() -> Tool {
    Tool {
        shape: ToolShape::FlatEnd,
        diameter: Millimetres(3.),
        flute_length: Millimetres(10.),
        overall_length: Millimetres(30.),
        shank_diameter: Millimetres(3.),
        flute_count: 2,
        material: None,
        centre_cutting: None,
    }
}
fn mesh() -> TriangleMesh {
    TriangleMesh {
        vertices: vec![
            Point3::new(10., -2., -3.),
            Point3::new(50., 4., 7.),
            Point3::new(50., -2., 7.),
        ],
        triangles: vec![[0, 1, 2]],
    }
}
fn stock() -> Stock {
    Stock::Box {
        bounds: Aabb {
            min: Point3::new(0., -10., -12.),
            max: Point3::new(60., 10., 12.),
        },
    }
}
fn settings() -> BoxFacingParameters {
    BoxFacingParameters {
        selection: SideSelection {
            standard: [Some(FaceOffset::FromPart), None, None, None],
            arbitrary: vec![],
        },
        extent: FacingExtent::Part,
        margin: 0.,
        layers: LayerDistribution::Full,
        stepdown: 2.,
        stepover: 1.5,
        allowance: 0.,
        tolerance: 0.01,
    }
}

#[test]
fn req_op_24_box_plan_reaches_each_requested_layer_with_a_fixed() {
    let part = mesh();
    let stock = stock();
    let tool = tool();
    let machine = MachineProfile::example_x_rotary();
    let setup = FacingSetup {
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
    let plan = plan_box_sides(&setup, &settings(), &ctx).unwrap();
    assert_eq!(plan.faces.len(), 1);
    let face = &plan.faces[0];
    assert_eq!(face.levels, vec![10., 8., 7.]);
    assert_eq!(face.target.offset, 7.);
    assert_eq!(face.coverage.min(), [10., -2.]);
    assert_eq!(face.coverage.max(), [50., 4.]);
    for pass in &plan.passes {
        assert_eq!(pass.points.len(), 2);
        for point in &pass.points {
            assert_eq!(point.a.0, 0.);
            assert!([10., 8., 7.].contains(&point.tip.z));
            assert!(point.tip.x >= 10. && point.tip.x <= 50.);
            assert!(point.tip.y >= -2. && point.tip.y <= 4.);
        }
    }
    assert!(
        plan.passes
            .iter()
            .any(|p| p.points.iter().any(|p| p.tip.x == 10.))
    );
    assert!(
        plan.passes
            .iter()
            .any(|p| p.points.iter().any(|p| p.tip.x == 50.))
    );
}

#[test]
fn req_op_24_equal_plan_and_cancellation_do_not_mutate_inputs() {
    let part = mesh();
    let stock = stock();
    let tool = tool();
    let machine = MachineProfile::example_x_rotary();
    let setup = FacingSetup {
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
    let mut parameters = settings();
    parameters.layers = LayerDistribution::Equal;
    let plan = plan_box_sides(&setup, &parameters, &ctx).unwrap();
    for (got, want) in plan.faces[0].levels.iter().zip([31. / 3., 26. / 3., 7.]) {
        assert!((got - want).abs() < 1e-10);
    }
    token.cancel();
    assert!(matches!(
        plan_box_sides(&setup, &parameters, &ctx),
        Err(openchuck_path::PathError::Cancelled)
    ));
    assert_eq!(part, mesh());
}

#[test]
fn req_op_24_req_op_12_box_plan_refuses_unsupported_tool_or_unknown_source_bound() {
    let part = mesh();
    let stock = stock();
    let mut tool = tool();
    let machine = MachineProfile::example_x_rotary();
    tool.shape = ToolShape::BallEnd;
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
    assert!(plan_box_sides(&setup, &settings(), &ctx).is_err());
    let flat = Tool {
        shape: ToolShape::FlatEnd,
        ..tool.clone()
    };
    setup.tool = &flat;
    setup.projection_error = None;
    assert!(plan_box_sides(&setup, &settings(), &ctx).is_err());
    setup.projection_error = Some(0.);
    let mut bad = settings();
    bad.stepdown = 11.;
    assert!(plan_box_sides(&setup, &bad, &ctx).is_err());
}

#[test]
fn req_op_24_req_nfr_07_projection_uncertainty_cannot_hide_negative_allowance() {
    let part = mesh();
    let stock = stock();
    let tool = tool();
    let machine = MachineProfile::example_x_rotary();
    let setup = FacingSetup {
        part: &part,
        stock: &stock,
        tool: &tool,
        machine: &machine,
        tabs: &[],
        protected: &[],
        held: [None, None],
        projection_error: Some(0.001),
    };
    let mut parameters = settings();
    parameters.allowance = -0.0005;
    let token = CancelToken::new();
    let ctx = JobContext::new(&token, &NoProgress);
    assert!(plan_box_sides(&setup, &parameters, &ctx).is_err());
}

#[test]
fn req_op_24_box_candidates_are_checked_against_final_footprints_not_layer_count() {
    use openchuck_path::facing::coverage::{FlatSweep, verify_flat_raster};
    use openchuck_types::geometry::UnitVec3;
    use openchuck_types::ir::ToolPose;
    let part = mesh();
    let stock = stock();
    let tool = tool();
    let machine = MachineProfile::example_x_rotary();
    let setup = FacingSetup {
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
    let plan = plan_box_sides(&setup, &settings(), &ctx).unwrap();
    let face = &plan.faces[0];
    let sweeps: Vec<_> = plan
        .passes
        .iter()
        .map(|p| {
            let pose = |p: &openchuck_path::CutPoint| ToolPose {
                tip: p.tip,
                axis: UnitVec3::Z,
                a: p.a,
            };
            FlatSweep {
                from: pose(&p.points[0]),
                to: pose(&p.points[1]),
            }
        })
        .collect();
    let rect = face.coverage.transposed();
    let plane = face.target.plane.transposed();
    verify_flat_raster(rect, plane, 7., &sweeps, 1.5, 0.01, &ctx).unwrap();
    let unfinished: Vec<_> = sweeps.into_iter().filter(|s| s.from.tip.z > 7.).collect();
    assert!(verify_flat_raster(rect, plane, 7., &unfinished, 1.5, 0.01, &ctx).is_err());
}

#[test]
fn req_op_24_req_nfr_05_source_error_does_not_change_nominal_side_target_or_override() {
    let part = mesh();
    let stock = stock();
    let tool = tool();
    let machine = MachineProfile::example_x_rotary();
    let setup = FacingSetup {
        part: &part,
        stock: &stock,
        tool: &tool,
        machine: &machine,
        tabs: &[],
        protected: &[],
        held: [None, None],
        projection_error: Some(0.001),
    };
    let token = CancelToken::new();
    let ctx = JobContext::new(&token, &NoProgress);
    let mut p = settings();
    p.allowance = 0.2;
    for offset in [FaceOffset::FromPart, FaceOffset::Explicit(7.2)] {
        p.selection.standard[0] = Some(offset);
        let plan = plan_box_sides(&setup, &p, &ctx).unwrap();
        assert_eq!(
            plan.faces[0].target.offset, 7.2,
            "Nominal target remains part plus allowance, including explicit values"
        );
        assert!(
            (plan.faces[0].levels.last().unwrap() - 7.201).abs() < 1e-12,
            "Conservative cutting level reserves only the source accuracy bound"
        );
    }
}

#[test]
fn req_op_24_req_ui_25_shared_face_description_reads_targets_extents_and_margin() {
    use openchuck_path::facing::description::{FacingGeometry, describe};
    use openchuck_path::{Facing, ParameterValue, Strategy};
    let part = mesh();
    let stock = stock();
    let tool = tool();
    let machine = MachineProfile::example_x_rotary();
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
    let values = |shape: &str| {
        [
            ("shape", ParameterValue::Text(shape.into())),
            ("feed", ParameterValue::Number(600.)),
            ("plunge_feed", ParameterValue::Number(200.)),
            ("stepover", ParameterValue::Number(1.)),
            ("stepdown", ParameterValue::Number(2.)),
            ("stock_to_leave", ParameterValue::Number(0.2)),
            ("extent", ParameterValue::Text("part".into())),
            ("extent_margin", ParameterValue::Number(2.)),
        ]
        .into_iter()
        .map(|(k, v)| (k.into(), v))
        .collect::<std::collections::BTreeMap<_, _>>()
    };
    let end = describe(&setup, &Facing.resolve_parameters(&values("end")).unwrap()).unwrap();
    let FacingGeometry::End {
        end,
        target,
        coverage,
    } = end.geometry
    else {
        panic!("End geometry expected")
    };
    assert_eq!(end, openchuck_path::facing::geometry::FacingEnd::Far);
    assert_eq!(target.x, 50.2);
    assert_eq!(target.length, 40.2);
    assert_eq!(coverage.min(), [-4., -5.]);
    assert_eq!(coverage.max(), [6., 9.]);
    let mut box_values = values("box");
    box_values.insert("positive_z".into(), ParameterValue::Flag(true));
    box_values.insert(
        "arbitrary_faces".into(),
        ParameterValue::Text(r#"[{"angle":90,"offset":9}]"#.into()),
    );
    let desc = describe(&setup, &Facing.resolve_parameters(&box_values).unwrap()).unwrap();
    let FacingGeometry::Box { faces } = desc.geometry else {
        panic!("Box geometry expected")
    };
    assert_eq!(faces.len(), 2);
    assert_eq!(faces[0].target.offset, 7.2);
    assert_eq!(faces[1].target.offset, 9.);
    assert_eq!(faces[0].coverage.min(), [8., -4.]);
    assert_eq!(faces[0].coverage.max(), [52., 6.]);
    let corners = faces[0].corners().unwrap();
    assert_eq!(corners[0], Point3::new(8., -4., 7.2));
    assert_eq!(corners[2], Point3::new(52., 6., 7.2));
}

#[test]
fn req_op_24_shared_description_preserves_legacy_nominal_planes_and_full_round_span() {
    use openchuck_path::facing::description::{FacingGeometry, describe};
    use openchuck_path::{Facing, ParameterValue, Strategy};
    let part = mesh();
    let stock = stock();
    let tool = tool();
    let machine = MachineProfile::example_x_rotary();
    let setup = FacingSetup {
        part: &part,
        stock: &stock,
        tool: &tool,
        machine: &machine,
        tabs: &[],
        protected: &[],
        held: [None, None],
        projection_error: Some(0.),
    };
    let mut values = [
        ("shape", ParameterValue::Text("flat".into())),
        ("size", ParameterValue::Number(5.)),
        ("feed", ParameterValue::Number(600.)),
        ("plunge_feed", ParameterValue::Number(200.)),
        ("stepover", ParameterValue::Number(1.)),
        ("stepdown", ParameterValue::Number(2.)),
    ]
    .into_iter()
    .map(|(k, v)| (k.into(), v))
    .collect::<std::collections::BTreeMap<_, _>>();
    let desc = describe(&setup, &Facing.resolve_parameters(&values).unwrap()).unwrap();
    let FacingGeometry::Flat { faces } = desc.geometry else {
        panic!("Flat geometry expected")
    };
    // The model reaches Z7; legacy flat may rise over it. Its nominal size
    // remains5, and must not become the new mode's derived target7.
    assert_eq!(faces[0].target.offset, 5.);
    assert_eq!(faces[0].coverage.min(), [0., -10.]);
    values.insert("shape".into(), ParameterValue::Text("round".into()));
    values.insert("extent".into(), ParameterValue::Text("part".into()));
    values.insert("extent_margin".into(), ParameterValue::Number(2.));
    let desc = describe(&setup, &Facing.resolve_parameters(&values).unwrap()).unwrap();
    assert!(matches!(
        desc.geometry,
        FacingGeometry::Round {
            diameter: 5.,
            x_extent: (8., 52.)
        }
    ));
    values.insert("shape".into(), ParameterValue::Text("end".into()));
    assert!(
        describe(&setup, &Facing.resolve_parameters(&values).unwrap())
            .unwrap_err()
            .to_string()
            .contains("Define which end is held")
    );
}

#[test]
fn req_op_24_stored_box_settings_drive_equal_layers_and_explicit_plane_without_double_allowance() {
    use openchuck_path::{Facing, ParameterValue, Strategy};
    let values = [
        ("shape", ParameterValue::Text("box".into())),
        ("feed", ParameterValue::Number(600.)),
        ("plunge_feed", ParameterValue::Number(200.)),
        ("stepover", ParameterValue::Number(1.5)),
        ("stepdown", ParameterValue::Number(2.)),
        ("stock_to_leave", ParameterValue::Number(0.2)),
        ("extent", ParameterValue::Text("part".into())),
        ("extent_margin", ParameterValue::Number(2.)),
        ("layer_distribution", ParameterValue::Text("equal".into())),
        ("positive_z", ParameterValue::Flag(true)),
        ("positive_z_from_part", ParameterValue::Flag(false)),
        ("positive_z_offset", ParameterValue::Number(9.)),
    ]
    .into_iter()
    .map(|(k, v)| (k.into(), v))
    .collect();
    let p = BoxFacingParameters::read(&Facing.resolve_parameters(&values).unwrap()).unwrap();
    let part = mesh();
    let stock = stock();
    let tool = tool();
    let machine = MachineProfile::example_x_rotary();
    let setup = FacingSetup {
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
    let plan = plan_box_sides(&setup, &p, &ctx).unwrap();
    assert_eq!(plan.faces[0].target.offset, 9.);
    assert_eq!(plan.faces[0].levels, vec![10.5, 9.]);
    assert_eq!(plan.faces[0].coverage.min(), [8., -4.]);
    assert_eq!(plan.faces[0].coverage.max(), [52., 6.]);
}
