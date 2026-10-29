// SPDX-License-Identifier: AGPL-3.0-or-later
//! OP-24: actual shared assembly, independently checked completion/clearance.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::float_cmp)]
use openchuck_path::facing::plan::{BoxFacingParameters, FacingSetup, plan_box_sides};
use openchuck_path::params::{CommonParameters, EntryParameters};
use openchuck_path::{
    AcceptAll, AnalyticStock, Facing, GeneratedPath, MeshSurface, ParameterValue, Parameters,
    RotaryMapping, Strategy, StrategyInput,
};
use openchuck_types::geometry::{Aabb, Point3};
use openchuck_types::ir::{MoveKind, PathTags};
use openchuck_types::job::{CancelToken, JobContext, NoProgress};
use openchuck_types::machine::{ASense, MachineProfile};
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::stock::Stock;
use openchuck_types::tool::{Tool, ToolShape};
use openchuck_types::units::{Millimetres, Seconds};
use std::collections::BTreeMap;

fn values(mask: u8, equal: bool, arbitrary: &str) -> BTreeMap<String, ParameterValue> {
    let mut values: BTreeMap<String, ParameterValue> = [
        ("shape", ParameterValue::Text("box".into())),
        ("feed", ParameterValue::Number(600.)),
        ("plunge_feed", ParameterValue::Number(200.)),
        ("stepover", ParameterValue::Number(1.5)),
        ("stepdown", ParameterValue::Number(1.3)),
        ("stock_to_leave", ParameterValue::Number(0.2)),
        (
            "layer_distribution",
            ParameterValue::Text(if equal { "equal" } else { "full" }.into()),
        ),
        ("arbitrary_faces", ParameterValue::Text(arbitrary.into())),
    ]
    .into_iter()
    .map(|(k, v)| (k.into(), v))
    .collect();
    for (i, name) in ["positive_z", "positive_y", "negative_z", "negative_y"]
        .into_iter()
        .enumerate()
    {
        values.insert(name.into(), ParameterValue::Flag(mask & (1 << i) != 0));
    }
    values
}

fn with_path<T>(
    values: &BTreeMap<String, ParameterValue>,
    sense: ASense,
    f: impl FnOnce(&StrategyInput<'_>, &Parameters, GeneratedPath, &JobContext<'_>) -> T,
) -> T {
    with_stock_path(
        values,
        sense,
        Stock::Box {
            bounds: Aabb {
                min: Point3::new(5., -4., -5.),
                max: Point3::new(25., 4., 5.),
            },
        },
        f,
    )
}
fn with_stock_path<T>(
    values: &BTreeMap<String, ParameterValue>,
    sense: ASense,
    stock: Stock,
    f: impl FnOnce(&StrategyInput<'_>, &Parameters, GeneratedPath, &JobContext<'_>) -> T,
) -> T {
    with_edited_plan(values, sense, stock, |_| {}, f)
}
fn with_edited_plan<T>(
    values: &BTreeMap<String, ParameterValue>,
    sense: ASense,
    stock: Stock,
    edit: impl FnOnce(&mut openchuck_path::facing::plan::FacingPassPlan),
    f: impl FnOnce(&StrategyInput<'_>, &Parameters, GeneratedPath, &JobContext<'_>) -> T,
) -> T {
    // All eight corners are referenced; this is the entire analytic block.
    let part = TriangleMesh {
        vertices: vec![
            Point3::new(10., -2., -2.),
            Point3::new(20., -2., -2.),
            Point3::new(10., 2., -2.),
            Point3::new(20., 2., -2.),
            Point3::new(10., -2., 2.),
            Point3::new(20., -2., 2.),
            Point3::new(10., 2., 2.),
            Point3::new(20., 2., 2.),
        ],
        triangles: vec![
            [0, 1, 2],
            [1, 3, 2],
            [4, 6, 5],
            [5, 6, 7],
            [0, 4, 1],
            [1, 4, 5],
            [2, 3, 6],
            [3, 7, 6],
            [0, 2, 4],
            [2, 6, 4],
            [1, 5, 3],
            [3, 5, 7],
        ],
    };
    let tool = Tool {
        shape: ToolShape::FlatEnd,
        diameter: Millimetres(3.),
        flute_length: Millimetres(10.),
        overall_length: Millimetres(30.),
        shank_diameter: Millimetres(3.),
        flute_count: 2,
        material: None,
        centre_cutting: Some(true),
    };
    let machine = MachineProfile {
        a_sense: sense,
        ..MachineProfile::example_x_rotary()
    };
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
    let context = JobContext::new(&token, &NoProgress);
    let parameters = Facing.resolve_parameters(values).unwrap();
    let mut plan = plan_box_sides(
        &setup,
        &BoxFacingParameters::read(&parameters).unwrap(),
        &context,
    )
    .unwrap();
    edit(&mut plan);
    let surface = MeshSurface::new(&part).unwrap();
    let profile = tool.profile().unwrap();
    let original = AnalyticStock::from_stock(&stock, None).unwrap();
    let tags = PathTags {
        operation: "box".into(),
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
    };
    let path = openchuck_path::assemble::assemble_with_entry(
        &plan.passes,
        &input,
        &CommonParameters::read(&parameters).unwrap(),
        &EntryParameters::read(&parameters).unwrap(),
        &context,
    )
    .unwrap();
    f(&input, &parameters, path, &context)
}

#[test]
fn req_op_24_entry_cannot_cut_extra_stock_outside_the_selected_extent_and_necessary_cutter_overlap()
{
    let mut v = values(1, false, "[]");
    v.insert("extent".into(), ParameterValue::Text("part".into()));
    with_path(&v, ASense::Positive, |input, p, mut path, ctx| {
        Facing.validate_path(input, p, &path, ctx).unwrap();
        let mut leg = *path.group.segments.last().unwrap();
        leg.end.tip = Point3::new(15., 3.5, 50.);
        leg.duration = Seconds(100.);
        leg.kind = MoveKind::Rapid;
        leg.contact_feed = None;
        path.group.segments.push(leg);
        leg.end.tip.z = 2.2;
        leg.kind = MoveKind::Plunge;
        leg.contact_feed = Some(openchuck_types::units::FeedRate(200.));
        path.group.segments.push(leg);
        // Earlier passes stop at centre Y2, so their Ø3 cutters reach only
        // Y3.5. The added plunge cuts the independent stock witness Y3.8.
        assert!(
            Facing.validate_path(input, p, &path, ctx).is_err(),
            "Complete requested coverage must not authorise extra outside-stock removal by entry motion"
        );
    });
}
#[test]
fn req_op_24_public_box_generation_returns_only_validated_complete_paths() {
    for equal in [false, true] {
        with_path(
            &values(15, equal, "[]"),
            ASense::Positive,
            |input, p, expected, ctx| {
                let generated =
                    openchuck_path::generate(&Facing, input, &values(15, equal, "[]"), ctx)
                        .unwrap();
                assert_eq!(generated.group, expected.group);
                Facing
                    .validate_group(input.facing.unwrap(), p, &generated.group, ctx)
                    .unwrap();
            },
        );
    }
}

#[test]
fn req_op_24_req_op_14_all_standard_combinations_and_arbitrary_sides_validate_actual_assembled_motion()
 {
    for sense in [ASense::Positive, ASense::Negative] {
        for mask in 1..16 {
            for equal in [false, true] {
                with_path(&values(mask, equal, "[]"), sense, |input, p, path, ctx| {
                    Facing
                        .validate_path(input, p, &path, ctx)
                        .unwrap_or_else(|e| {
                            panic!("sense {sense:?}, mask {mask}, equal {equal}: {e}")
                        });
                    assert!(
                        path.group
                            .segments
                            .iter()
                            .any(|s| s.kind == MoveKind::Rapid)
                    );
                    assert!(
                        path.group
                            .segments
                            .iter()
                            .any(|s| s.kind == MoveKind::Plunge)
                    );
                });
            }
        }
        with_path(
            &values(5, false, r#"[{"angle":33},{"angle":-27}]"#),
            sense,
            |input, p, path, ctx| Facing.validate_path(input, p, &path, ctx).unwrap(),
        );
    }
}

#[test]
fn req_op_24_req_op_12_final_validator_refuses_changed_sweeps_and_never_uses_candidate_counts() {
    with_path(
        &values(1, false, "[]"),
        ASense::Positive,
        |input, p, path, ctx| {
            Facing.validate_path(input, p, &path, ctx).unwrap();
            // Two adjacent final rows raised out of tolerance leave an internal
            // strip wider than their neighbours' combined cutter footprints.
            let mut raised = path.clone();
            for segment in &mut raised.group.segments {
                if (14.9..17.1).contains(&segment.end.tip.x) && segment.end.tip.z == 2.2 {
                    segment.end.tip.z += 0.02;
                }
                segment.duration = Seconds(100.);
            }
            assert!(
                Facing.validate_path(input, p, &raised, ctx).is_err(),
                "Raised actual strip cannot borrow the candidate's coverage"
            );
            // Remove the intermediate layers, retaining safe links/final rows.
            // A long flute does not authorise exceeding the selected stepdown.
            let mut missing = path.clone();
            missing
                .group
                .segments
                .retain(|s| s.end.tip.z > 5. || s.end.tip.z <= 2.2);
            for s in &mut missing.group.segments {
                s.duration = Seconds(100.);
            }
            assert!(
                Facing.validate_path(input, p, &missing, ctx).is_err(),
                "Final-plane coverage alone cannot certify the layer sequence"
            );
            let mut rapid = path.clone();
            let segment = rapid
                .group
                .segments
                .iter_mut()
                .find(|s| s.kind == MoveKind::Rapid)
                .unwrap();
            segment.end.tip = Point3::new(15., 0., 0.);
            segment.duration = Seconds(100.);
            assert!(Facing.validate_path(input, p, &rapid, ctx).is_err());
            let mut overcut = path;
            for s in &mut overcut.group.segments {
                if s.end.tip.z == 2.2 {
                    s.end.tip.z = 2.17;
                }
                s.duration = Seconds(100.);
            }
            assert!(Facing.validate_path(input, p, &overcut, ctx).is_err());
        },
    );
}

#[test]
fn req_op_24_req_op_21_box_extents_and_margins_preserve_thin_tabs_and_refuse_unknown_context() {
    use openchuck_path::Solid;
    for sense in [ASense::Positive, ASense::Negative] {
        for extent in ["whole_stock", "part", "part_plus_tabs"] {
            for margin in [0., 0.5] {
                let mut v = values(1, false, "[]");
                v.insert("extent".into(), ParameterValue::Text(extent.into()));
                v.insert("extent_margin".into(), ParameterValue::Number(margin));
                with_path(&v, sense, |input, p, path, ctx| {
                    Facing.validate_path(input, p, &path, ctx).unwrap();
                    // A tab's positive-volume witness is between both ends of
                    // an across-going row, wholly above the finished model.
                    // It is protected even in Part plus tabs mode.
                    let tab = Solid::cuboid(Aabb {
                        min: Point3::new(14.99, -0.01, 3.),
                        max: Point3::new(15.01, 0.01, 3.01),
                    })
                    .unwrap();
                    let setup = input.facing.unwrap();
                    let blocked = FacingSetup {
                        tabs: std::slice::from_ref(&tab),
                        protected: std::slice::from_ref(&tab),
                        ..*setup
                    };
                    let blocked_input = StrategyInput {
                        facing: Some(&blocked),
                        ..*input
                    };
                    let error = Facing
                        .validate_path(&blocked_input, p, &path, ctx)
                        .unwrap_err()
                        .to_string();
                    assert!(error.contains("protected geometry"), "{error}");
                    let unknown = FacingSetup {
                        projection_error: None,
                        ..*setup
                    };
                    let unknown_input = StrategyInput {
                        facing: Some(&unknown),
                        ..*input
                    };
                    assert!(Facing.validate_path(&unknown_input, p, &path, ctx).is_err());
                    let imprecise = FacingSetup {
                        projection_error: Some(0.001),
                        ..*setup
                    };
                    let imprecise_input = StrategyInput {
                        facing: Some(&imprecise),
                        ..*input
                    };
                    assert!(
                        Facing
                            .validate_path(&imprecise_input, p, &path, ctx)
                            .is_err()
                    );
                    let ball = Tool {
                        shape: ToolShape::BallEnd,
                        ..setup.tool.clone()
                    }
                    .profile()
                    .unwrap();
                    let wrong_tool = StrategyInput {
                        tool: &ball,
                        ..*input
                    };
                    assert!(Facing.validate_path(&wrong_tool, p, &path, ctx).is_err());
                });
            }
        }
    }
}

#[test]
fn req_op_24_req_op_11_selected_maximum_depth_is_not_relaxed_by_finishing_tolerance() {
    with_path(
        &values(1, false, "[]"),
        ASense::Positive,
        |input, p, path, ctx| {
            Facing.validate_path(input, p, &path, ctx).unwrap();
            for (from, to) in [(3.7, 3.7001), (2.4, 2.3999)] {
                let mut changed = path.clone();
                let mut count = 0;
                for s in &mut changed.group.segments {
                    if (s.end.tip.z - from).abs() < 1e-12 {
                        s.end.tip.z = to;
                        count += 1;
                    }
                    s.duration = Seconds(100.);
                }
                assert!(count > 10);
                // 3.7001 -> 2.4 or 3.7 -> 2.3999 is 1.3001 mm;
                // tolerance 0.01 mm cannot enlarge selected maximum 1.3 mm.
                assert!(
                    Facing.validate_path(input, p, &changed, ctx).is_err(),
                    "A selected maximum is an engagement limit, not a finish error budget"
                );
            }
        },
    );
}

#[test]
fn req_op_24_req_op_11_deep_assembled_box_uses_actual_upper_layers_to_clear_shank() {
    with_stock_path(
        &values(1, false, "[]"),
        ASense::Positive,
        Stock::Box {
            bounds: Aabb {
                min: Point3::new(5., -4., -16.),
                max: Point3::new(25., 4., 16.),
            },
        },
        |input, p, path, ctx| {
            // Depth 16 - 2.2 = 13.8 mm is longer than the 10 mm flute.
            // Completion alone cannot demonstrate that the shank can descend.
            Facing.validate_path(input, p, &path, ctx).unwrap();
            let mut missing = path;
            missing
                .group
                .segments
                .retain(|s| s.end.tip.z > 16. || s.end.tip.z <= 2.2);
            for s in &mut missing.group.segments {
                s.duration = Seconds(100.);
            }
            let error = Facing
                .validate_path(input, p, &missing, ctx)
                .unwrap_err()
                .to_string();
            assert!(error.contains("shank"), "{error}");
        },
    );
}

#[test]
fn req_op_24_req_set_09_box_held_ends_do_not_crop_requested_coverage_or_disappear_at_part_extents()
{
    use openchuck_path::Solid;
    for held_mask in 0..4 {
        for extent in ["whole_stock", "part"] {
            let mut v = values(1, false, "[]");
            v.insert("extent".into(), ParameterValue::Text(extent.into()));
            with_path(&v, ASense::Positive, |input, p, path, ctx| {
                let mut protections = Vec::new();
                let mut held = [None, None];
                for (i, (lo, hi)) in [(5., 8.), (22., 25.)].into_iter().enumerate() {
                    if held_mask & (1 << i) != 0 {
                        held[i] = Some((lo, hi));
                        protections.push(
                            Solid::cuboid(Aabb {
                                min: Point3::new(lo, -4., -5.),
                                max: Point3::new(hi, 4., 5.),
                            })
                            .unwrap(),
                        );
                    }
                }
                let setup = FacingSetup {
                    held,
                    protected: &protections,
                    ..*input.facing.unwrap()
                };
                let protected_input = StrategyInput {
                    facing: Some(&setup),
                    ..*input
                };
                let result = Facing.validate_path(&protected_input, p, &path, ctx);
                if held_mask == 0 || extent == "part" {
                    result.unwrap();
                } else {
                    assert!(
                        result
                            .unwrap_err()
                            .to_string()
                            .contains("protected geometry")
                    );
                }
            });
        }
    }
}

#[test]
fn req_op_24_req_op_11_declared_non_centre_cutting_tool_cannot_borrow_a_plunging_certificate() {
    with_path(
        &values(1, false, "[]"),
        ASense::Positive,
        |input, p, path, ctx| {
            let original = input.facing.unwrap();
            let tool = Tool {
                centre_cutting: Some(false),
                ..original.tool.clone()
            };
            let setup = FacingSetup {
                tool: &tool,
                ..*original
            };
            let actual = StrategyInput {
                facing: Some(&setup),
                ..*input
            };
            // Cutting profile equality alone does not carry this tool limit.
            assert_eq!(tool.profile().unwrap(), *input.tool);
            assert!(
                Facing.validate_path(&actual, p, &path, ctx).is_err(),
                "A declared tool limit is required even when nominal profile geometry matches"
            );
        },
    );
}

#[test]
fn req_op_24_req_op_11_deeper_cutter_overlap_cannot_enter_unremoved_outside_extent_stock() {
    let mut v = values(1, false, "[]");
    v.insert("extent".into(), ParameterValue::Text("part".into()));
    let stock = Stock::Box {
        bounds: Aabb {
            min: Point3::new(5., -4., -5.),
            max: Point3::new(25., 4., 5.),
        },
    };
    with_edited_plan(
        &v,
        ASense::Positive,
        stock,
        |plan| {
            plan.passes.retain(|pass| {
                !pass
                    .points
                    .first()
                    .is_some_and(|p| p.tip.x == 10. && p.tip.z > 2.2)
            });
        },
        |input, p, path, ctx| {
            // Neighbouring upper rows still cover the full requested X10..20
            // face, but reach left only to X9.92857. The final edge row's radius
            // reaches X8.5. A strict witness X9,Y0 therefore remains at stock
            // top5 until the Z2.2 cut: engagement2.8 exceeds selected1.3 OUTSIDE
            // the requested extent, even though every required layer is covered.
            assert!(
                Facing.validate_path(input, p, &path, ctx).is_err(),
                "Coverage of the rectangle alone is not an engagement certificate for its cutter overlap"
            );
        },
    );
}
