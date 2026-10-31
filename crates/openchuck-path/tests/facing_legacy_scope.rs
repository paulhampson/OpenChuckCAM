// SPDX-License-Identifier: AGPL-3.0-or-later
//! OP-24: public Facing maps whole placed geometry to scoped cut bounds,
//! retains original stock, and never publishes an unverified operation.
#![allow(clippy::unwrap_used, clippy::float_cmp, clippy::indexing_slicing)]
use openchuck_path::facing::plan::FacingSetup;
use openchuck_path::{
    AcceptAll, AnalyticStock, Facing, MeshSurface, ParameterValue, Solid, Strategy, StrategyInput,
    generate,
};
use openchuck_types::geometry::{Aabb, Point3, UnitVec3};
use openchuck_types::ir::PathTags;
use openchuck_types::job::{CancelToken, JobContext, NoProgress};
use openchuck_types::machine::MachineProfile;
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::stock::Stock;
use openchuck_types::tool::{Tool, ToolShape};
use openchuck_types::units::Millimetres;
use std::collections::BTreeMap;

fn fixture(test: impl FnOnce(&StrategyInput<'_>, &FacingSetup<'_>, &JobContext<'_>)) {
    let part = TriangleMesh {
        vertices: vec![
            Point3::new(10., -2., 1.),
            Point3::new(20., -2., 4.),
            Point3::new(20., 3., 2.),
        ],
        triangles: vec![[0, 1, 2]],
    };
    let bounds = Aabb {
        min: Point3::new(0., -8., -8.),
        max: Point3::new(30., 8., 8.),
    };
    let stock = Stock::Box { bounds };
    let port = AnalyticStock::Box(bounds);
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
    };
    let profile = tool.profile().unwrap();
    let surface = MeshSurface::new(&part).unwrap();
    let tab = Solid::cylinder(Point3::new(21., 5., 0.), UnitVec3::X, 4., 1.).unwrap();
    let protections = [tab];
    let setup = FacingSetup {
        part: &part,
        stock: &stock,
        tool: &tool,
        machine: &machine,
        tabs: &protections,
        protected: &protections,
        held: [None, None],
        projection_error: Some(0.),
    };
    let tags = PathTags {
        operation: "op".into(),
        setup: "setup".into(),
        tool: "tool".into(),
    };
    let input = StrategyInput {
        surface: &surface,
        stock: &port,
        collision: &AcceptAll,
        tool: &profile,
        rotary: openchuck_path::RotaryMapping::of_profile(&machine),
        clearance: machine.clearance,
        max_feeds: machine.max_feeds,
        tags: &tags,
        entry_safety: None,
        facing: Some(&setup),
        keep_out: &[],
        shank: None,
    };
    let token = CancelToken::new();
    test(&input, &setup, &JobContext::new(&token, &NoProgress));
}
fn values(extent: &str, margin: f64) -> BTreeMap<String, ParameterValue> {
    [
        ("size", ParameterValue::Number(6.)),
        ("angles", ParameterValue::Text("0,90".into())),
        ("feed", ParameterValue::Number(600.)),
        ("plunge_feed", ParameterValue::Number(200.)),
        ("stepover", ParameterValue::Number(1.)),
        ("stepdown", ParameterValue::Number(2.)),
        ("extent", ParameterValue::Text(extent.into())),
        ("extent_margin", ParameterValue::Number(margin)),
    ]
    .into_iter()
    .map(|(k, v)| (k.into(), v))
    .collect()
}

#[test]
fn req_op_24_restricted_legacy_flat_and_round_return_only_independently_complete_geometry() {
    fixture(|input, _setup, ctx| {
        for shape in ["flat", "round"] {
            let stock = if shape == "round" {
                Stock::Cylinder {
                    diameter: Millimetres(20.),
                    length: Millimetres(30.),
                    x_start: Millimetres(0.),
                }
            } else {
                input.facing.unwrap().stock.clone()
            };
            let setup = FacingSetup {
                stock: &stock,
                ..*input.facing.unwrap()
            };
            let port = AnalyticStock::from_stock(&stock, None).unwrap();
            let input = &StrategyInput {
                stock: &port,
                facing: Some(&setup),
                ..*input
            };
            let mut v = values("part", 0.);
            v.insert("shape".into(), ParameterValue::Text(shape.into()));
            v.insert("angles".into(), ParameterValue::Text("0".into()));
            v.insert("min_cut".into(), ParameterValue::Number(0.));
            v.insert(
                "size".into(),
                ParameterValue::Number(if shape == "round" { 14. } else { 6. }),
            );
            let generated =
                generate(&Facing, input, &v, ctx).unwrap_or_else(|e| panic!("{shape}: {e}"));
            Facing
                .validate_group(
                    input.facing.unwrap(),
                    &Facing.resolve_parameters(&v).unwrap(),
                    &generated.group,
                    ctx,
                )
                .unwrap();
            let mut broken = generated.group;
            broken
                .segments
                .retain(|s| s.kind != openchuck_types::ir::MoveKind::Cutting);
            assert!(
                Facing
                    .validate_group(
                        input.facing.unwrap(),
                        &Facing.resolve_parameters(&v).unwrap(),
                        &broken,
                        ctx
                    )
                    .is_err()
            );
        }
    });
}
#[test]
fn req_op_24_public_legacy_scope_uses_part_tab_union_and_margin_in_each_plane() {
    fixture(|input, _setup, ctx| {
        for extent in ["part", "part_plus_tabs"] {
            for margin in [0., 0.5] {
                let passes = Facing
                    .passes(
                        input,
                        &Facing.resolve_parameters(&values(extent, margin)).unwrap(),
                        ctx,
                    )
                    .unwrap();
                assert!(!passes.is_empty());
                let xhi = if extent == "part" { 20. } else { 25. };
                let umax = if extent == "part" { 3. } else { 6. };
                let vhi = if extent == "part" { -1. } else { 1. };
                let mut max_x = f64::NEG_INFINITY;
                for p in passes.iter().flat_map(|p| &p.points) {
                    max_x = max_x.max(p.tip.x);
                    assert!((10. - margin - 1e-9..=xhi + margin + 1e-9).contains(&p.tip.x));
                    let (u, lo, hi) = if p.a.0 == 0. {
                        (p.tip.y, -2., umax)
                    } else {
                        assert_eq!(p.a.0, 90.);
                        (-p.tip.z, -4., vhi)
                    };
                    assert!((lo - margin - 1e-9..=hi + margin + 1e-9).contains(&u));
                }
                assert!(
                    max_x >= xhi + margin - 1e-9,
                    "The actual selected axial edge must be reached"
                );
                assert!(
                    passes.iter().all(|p| p
                        .stock
                        .as_ref()
                        .unwrap()
                        .0
                        .contains(Point3::new(3., 0., 7.))),
                    "Original outside-extent stock must remain visible"
                );
            }
        }
    });
}

#[test]
fn req_op_24_req_set_06_unreachable_scoped_candidates_cannot_be_returned_as_usable_paths() {
    fixture(|input, setup, ctx| {
        let mut machine = setup.machine.clone();
        machine.travel.min.x = 0.;
        machine.travel.max.x = 1.;
        let setup = FacingSetup {
            machine: &machine,
            ..*setup
        };
        let input = StrategyInput {
            facing: Some(&setup),
            ..*input
        };
        let v = values("part", 0.);
        let p = Facing.resolve_parameters(&v).unwrap();
        let passes = Facing.passes(&input, &p, ctx).unwrap();
        assert!(
            passes
                .iter()
                .flat_map(|p| &p.points)
                .all(|p| p.tip.x >= 10.)
        );
        assert!(
            generate(&Facing, &input, &v, ctx).is_err(),
            "A valid candidate list is not a usable completed operation when the machine cannot reach it"
        );
    });
}
