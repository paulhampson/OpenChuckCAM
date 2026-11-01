// SPDX-License-Identifier: AGPL-3.0-or-later
//! Owner-required regression mitigation for scoped legacy Facing.
#![allow(clippy::unwrap_used, clippy::float_cmp, clippy::indexing_slicing)]
use super::*;
use crate::facing::geometry::{CoverageRectangle, FacingPlane};
use crate::{AcceptAll, AnalyticStock, Facing, MeshSurface, ParameterValue, Strategy};
use openchuck_types::geometry::{Aabb, Point3, UnitVec3};
use openchuck_types::ir::PathTags;
use openchuck_types::job::{CancelToken, NoProgress};
use openchuck_types::machine::MachineProfile;
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::tool::{Tool, ToolShape};
use openchuck_types::units::Millimetres;

/// [`super::face`]'s passes: these tests are about the scope, not adaptive
/// spacing, which their parameters turn off.
fn face(
    input: &StrategyInput<'_>,
    parameters: &Parameters,
    round: bool,
    level: f64,
    scope: Option<&FacingScope>,
    context: &JobContext<'_>,
) -> Result<Vec<Pass>> {
    super::face(input, parameters, round, level, scope, context).map(|o| o.passes)
}

fn parameters(round: bool) -> Parameters {
    parameters_with(round, &[])
}
fn parameters_with(round: bool, extra: &[(&str, ParameterValue)]) -> Parameters {
    let mut values = std::collections::BTreeMap::new();
    for (k, v) in [
        ("size", if round { 8. } else { 4. }),
        ("feed", 600.),
        ("plunge_feed", 200.),
        ("stepover", 1.),
        ("stepdown", 2.),
        ("max_angle_step", 30.),
    ] {
        values.insert(k.into(), ParameterValue::Number(v));
    }
    values.insert(
        "shape".into(),
        ParameterValue::Text(if round { "round" } else { "flat" }.into()),
    );
    values.insert("angles".into(), ParameterValue::Text("0,90".into()));
    values.insert("adaptive_spacing".into(), ParameterValue::Flag(false));
    for (name, value) in extra {
        values.insert((*name).into(), value.clone());
    }
    Facing.resolve_parameters(&values).unwrap()
}
fn rectangle(x: (f64, f64), u: (f64, f64)) -> CoverageRectangle {
    FacingPlane::side(
        RotaryMapping::of_profile(&MachineProfile::example_x_rotary()),
        Degrees(0.),
    )
    .unwrap()
    .bounds(
        [Point3::new(x.0, u.0, 0.), Point3::new(x.1, u.1, 0.)],
        &[],
        0.,
    )
    .unwrap()
}
fn input_test(test: impl FnOnce(&StrategyInput<'_>, &JobContext<'_>)) {
    let mesh = TriangleMesh {
        vertices: vec![
            Point3::new(10., -2., 0.),
            Point3::new(20., -2., 0.),
            Point3::new(20., 2., 0.),
        ],
        triangles: vec![[0, 1, 2]],
    };
    let surface = MeshSurface::new(&mesh).unwrap();
    let stock = AnalyticStock::Box(Aabb {
        min: Point3::new(0., -8., -8.),
        max: Point3::new(30., 8., 8.),
    });
    let machine = MachineProfile::example_x_rotary();
    let tool = Tool {
        shape: ToolShape::FlatEnd,
        diameter: Millimetres(2.),
        flute_length: Millimetres(12.),
        overall_length: Millimetres(30.),
        shank_diameter: Millimetres(2.),
        flute_count: 2,
        material: None,
        centre_cutting: None,
    }
    .profile()
    .unwrap();
    let tags = PathTags {
        operation: "op".into(),
        setup: "setup".into(),
        tool: "tool".into(),
    };
    let input = StrategyInput {
        surface: &surface,
        stock: &stock,
        collision: &AcceptAll,
        tool: &tool,
        rotary: RotaryMapping::of_profile(&machine),
        clearance: machine.clearance,
        max_feeds: machine.max_feeds,
        tags: &tags,
        entry_safety: None,
        facing: None,
        keep_out: &[],
        shank: None,
    };
    let token = CancelToken::new();
    let ctx = JobContext::new(&token, &NoProgress);
    test(&input, &ctx);
}

#[test]
fn req_op_24_scoped_flat_plans_each_face_inside_its_own_bounds() {
    input_test(|input, ctx| {
        let scope = FacingScope::Flat(vec![
            rectangle((10., 20.), (-2., 3.)),
            rectangle((8., 16.), (-1., 1.)),
        ]);
        let passes = face(input, &parameters(false), false, 4., Some(&scope), ctx).unwrap();
        let mut seen = [false; 2];
        for p in passes.iter().flat_map(|p| &p.points) {
            let axis = input.rotary.to_axis(p.tip, p.a);
            if p.a.0 == 0. {
                seen[0] = true;
                assert!((10. - 1e-9..=20. + 1e-9).contains(&p.tip.x));
                assert!((-2. - 1e-9..=3. + 1e-9).contains(&axis.across));
            } else {
                seen[1] = true;
                assert_eq!(p.a.0, 90.);
                assert!((8. - 1e-9..=16. + 1e-9).contains(&p.tip.x));
                assert!((-1. - 1e-9..=1. + 1e-9).contains(&axis.across));
            }
        }
        assert!(seen.into_iter().all(|v| v));
    });
}

#[test]
fn req_op_24_req_op_14_scope_keeps_outside_stock_in_every_pass_snapshot_and_clearance_ray() {
    input_test(|input, ctx| {
        for round in [false, true] {
            let scope = if round {
                FacingScope::Round((10., 20.))
            } else {
                FacingScope::Flat(vec![rectangle((10., 20.), (-2., 3.)); 2])
            };
            let passes = face(input, &parameters(round), round, 4., Some(&scope), ctx).unwrap();
            assert!(!passes.is_empty());
            for pass in &passes {
                let stock = &pass.stock.as_ref().unwrap().0;
                for x in [2., 5., 25., 28.] {
                    assert!(
                        stock.contains(Point3::new(x, 0., 7.)),
                        "Outside extent stock disappeared at X{x}"
                    );
                    let hit = stock
                        .ray_entry(
                            Point3::new(x, 0., 20.),
                            UnitVec3::new(openchuck_types::geometry::Vec3::new(0., 0., -1.))
                                .unwrap(),
                            30.,
                        )
                        .unwrap();
                    assert!(
                        hit <= 12.5,
                        "A clearance ray must see retained stock, hit={hit}"
                    );
                }
            }
            let last = &passes.last().unwrap().stock.as_ref().unwrap().0;
            assert!(
                !last.contains(Point3::new(15., 0., 7.)),
                "Prior layers must actually remove inside-scope stock"
            );
            if !round {
                assert!(
                    last.contains(Point3::new(15., 7., 7.)),
                    "Uncut transverse stock must remain"
                );
            }
        }
    });
}

#[test]
fn req_op_24_scoped_round_keeps_full_circumference_within_selected_axial_span() {
    input_test(|input, ctx| {
        let passes = face(
            input,
            &parameters(true),
            true,
            4.,
            Some(&FacingScope::Round((10., 20.))),
            ctx,
        )
        .unwrap();
        let points = passes.iter().flat_map(|p| &p.points).collect::<Vec<_>>();
        assert!(
            points
                .iter()
                .all(|p| (10. - 1e-9..=20. + 1e-9).contains(&p.tip.x))
        );
        for end in [10., 20.] {
            let mut a = points
                .iter()
                .filter(|p| {
                    (p.tip.x - end).abs() < 1e-9 && (p.tip.y.hypot(p.tip.z) - 4.).abs() < 1e-9
                })
                .map(|p| p.a.0);
            let first = a.next().unwrap();
            assert!(
                a.any(|v| (v - first).abs() >= 360. - 1e-9),
                "Boundary X{end} needs a full turn"
            );
        }
    });
}

#[test]
fn req_op_24_req_set_09_invalid_or_fully_held_scope_refuses_without_whole_stock_fallback() {
    input_test(|input, ctx| {
        let p = parameters(false);
        for scope in [FacingScope::Flat(vec![]), FacingScope::Round((10., 20.))] {
            assert!(face(input, &p, false, 4., Some(&scope), ctx).is_err());
        }
        let scope = FacingScope::Flat(vec![rectangle((2., 6.), (-2., 3.)); 2]);
        let mut held = *input;
        held.keep_out = &[(0., 8.)];
        assert!(face(&held, &p, false, 4., Some(&scope), ctx).is_err());
        for keep_out in [
            vec![(0., 30.)],
            vec![(0., 18.), (12., 30.)],
            vec![(0., 16.)],
        ] {
            let held_case = StrategyInput {
                keep_out: &keep_out,
                ..*input
            };
            assert!(
                face(&held_case, &p, false, 4., Some(&scope), ctx).is_err(),
                "A held interval crossing the original stock midpoint must not bypass scoped refusal: {keep_out:?}"
            );
        }
        for x in [
            (20., 10.),
            (f64::NAN, 20.),
            (10., f64::INFINITY),
            (10., 10.),
        ] {
            assert!(
                face(
                    input,
                    &parameters(true),
                    true,
                    4.,
                    Some(&FacingScope::Round(x)),
                    ctx
                )
                .is_err()
            );
        }
    });
}

#[test]
fn req_op_24_scope_preserves_stock_edge_extension_distance_and_refuses_conflicts() {
    input_test(|given, ctx| {
        let stock = AnalyticStock::Cylinder {
            radius: 10.,
            x_start: 0.,
            x_end: 30.,
        };
        let mut input = *given;
        input.stock = &stock;
        let p = parameters_with(
            false,
            &[
                ("extend", ParameterValue::Text("far end".into())),
                ("extend_sides", ParameterValue::Flag(true)),
                ("extend_distance", ParameterValue::Number(0.4)),
            ],
        );
        let half = (100.0_f64 - 16.).sqrt();
        let conflict = FacingScope::Flat(vec![rectangle((10., 20.), (-half, half)); 2]);
        let err = face(&input, &p, false, 4., Some(&conflict), ctx)
            .unwrap_err()
            .to_string();
        assert!(err.contains("stock edge"), "{err}");
        let side_conflict = FacingScope::Flat(vec![rectangle((0., 30.), (-2., 3.)); 2]);
        let err = face(&input, &p, false, 4., Some(&side_conflict), ctx)
            .unwrap_err()
            .to_string();
        assert!(err.contains("stock sides"), "{err}");
        let scope = FacingScope::Flat(vec![rectangle((0., 30.), (-half, half)); 2]);
        let passes = face(&input, &p, false, 4., Some(&scope), ctx).unwrap();
        let points = passes.iter().flat_map(|p| &p.points).collect::<Vec<_>>();
        assert!(
            points.iter().any(|p| p.tip.x >= 31.4 - 1e-9),
            "Far end remains stock30 + tool radius1 + extension0.4"
        );
        let across = points
            .iter()
            .map(|p| input.rotary.to_axis(p.tip, p.a).across.abs())
            .fold(0., f64::max);
        assert!(
            across >= half + 1. + 0.4 - 1e-9,
            "Side run-out remains stock chord + tool radius + distance: {across}"
        );
        let both = parameters_with(
            false,
            &[
                ("extend", ParameterValue::Text("both ends".into())),
                ("extend_distance", ParameterValue::Number(0.4)),
            ],
        );
        let passes = face(&input, &both, false, 4., Some(&scope), ctx).unwrap();
        assert!(
            passes
                .iter()
                .flat_map(|p| &p.points)
                .any(|p| p.tip.x <= -1.4 + 1e-9),
            "Start end also keeps stock-edge + near-side distance"
        );
    });
}

#[test]
fn req_op_24_req_nfr_07_scope_refuses_nonfinite_held_limits_internal_splits_and_excessive_work() {
    input_test(|input, ctx| {
        let scope = FacingScope::Flat(vec![rectangle((10., 20.), (-2., 3.)); 2]);
        for keep_out in [vec![(f64::NAN, 18.)], vec![(18., 12.)], vec![(14., 16.)]] {
            let input = StrategyInput {
                keep_out: &keep_out,
                ..*input
            };
            let err = face(&input, &parameters(false), false, 4., Some(&scope), ctx)
                .unwrap_err()
                .to_string();
            assert!(err.contains("held"), "{err}");
        }
        let huge = FacingScope::Flat(vec![rectangle((10., 20.), (-1e12, 1e12)); 2]);
        let err = face(input, &parameters(false), false, 4., Some(&huge), ctx)
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("budget"),
            "Excessive work must refuse before allocating nodes: {err}"
        );
    });
}

#[test]
fn req_op_24_req_set_09_scoped_far_held_interval_crossing_midpoint_preserves_held_stock() {
    input_test(|input, ctx| {
        let input = StrategyInput {
            keep_out: &[(12., 30.)],
            ..*input
        };
        let scope = FacingScope::Flat(vec![rectangle((10., 20.), (-2., 3.)); 2]);
        let passes = face(&input, &parameters(false), false, 4., Some(&scope), ctx).unwrap();
        assert!(!passes.is_empty());
        assert!(
            passes
                .iter()
                .flat_map(|p| &p.points)
                .all(|p| p.tip.x <= 12. + 1e-9)
        );
        assert!(passes.iter().all(|p| {
            p.stock
                .as_ref()
                .unwrap()
                .0
                .contains(Point3::new(15., 0., 7.))
        }));
    });
}
