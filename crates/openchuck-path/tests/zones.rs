// SPDX-License-Identifier: AGPL-3.0-or-later
//! Stock to leave per selection (OP-11; work breakdown slice 12 item 4;
//! decision record 0033).
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing
)]

mod support;

use std::collections::BTreeMap;

use openchuck_path::axial::parameter as axial_parameter;
use openchuck_path::params::names;
use openchuck_path::{
    AxialFinishing, GeneratedPath, HelicalFinishing, IndexedFinishing, LeaveZone, MeshSurface,
    ParameterValue, Strategy, generate, generate_in_zones,
};
use openchuck_testkit::parts::{Cylinder, RadialPart};
use openchuck_types::geometry::Point3;
use openchuck_types::ir::MoveKind;
use openchuck_types::mesh::TriangleMesh;
use support::{Fixture, ball, run};

const RADIUS: f64 = 15.0;
const LENGTH: f64 = 40.0;

fn values(extra: &[(&str, f64)]) -> BTreeMap<String, ParameterValue> {
    let mut v: BTreeMap<String, ParameterValue> = [
        (names::STEPOVER, 2.0),
        (names::FEED, 300.0),
        (names::PLUNGE_FEED, 100.0),
    ]
    .into_iter()
    .map(|(k, x)| (k.to_owned(), ParameterValue::Number(x)))
    .collect();
    for (k, x) in extra {
        v.insert((*k).to_owned(), ParameterValue::Number(*x));
    }
    v
}

fn pick(mesh: &TriangleMesh, keep: impl Fn(Point3) -> bool) -> TriangleMesh {
    let triangles = (0..mesh.triangles.len())
        .filter(|&t| mesh.triangle(t).unwrap().into_iter().all(&keep))
        .map(|t| mesh.triangles[t])
        .collect();
    TriangleMesh {
        vertices: mesh.vertices.clone(),
        triangles,
    }
}

struct Parts {
    fixture: Fixture,
    low: MeshSurface,
    high: MeshSurface,
}

/// A cylinder split at X = 20 into a low half and a high half.
fn parts() -> Parts {
    let mesh = Cylinder {
        radius: RADIUS,
        length: LENGTH,
    }
    .mesh(81, 720);
    Parts {
        fixture: Fixture::new(MeshSurface::new(&mesh).unwrap(), RADIUS + 1.0, &ball(6.0)),
        low: MeshSurface::new(&pick(&mesh, |p| p.x <= 20.0 + 1e-9)).unwrap(),
        high: MeshSurface::new(&pick(&mesh, |p| p.x >= 20.0 - 1e-9)).unwrap(),
    }
}

fn radius_at(path: &GeneratedPath) -> Vec<(f64, f64)> {
    path.group
        .segments
        .iter()
        .filter(|s| s.kind == MoveKind::Cutting)
        .map(|s| (s.end.tip.x, s.end.tip.y.hypot(s.end.tip.z)))
        .collect()
}

fn check_leaves(cuts: &[(f64, f64)], what: &str) {
    assert!(cuts.len() > 50, "{what}");
    let mut by_x: Vec<(f64, f64)> = cuts.to_vec();
    by_x.sort_by(|a, b| a.0.total_cmp(&b.0));
    for &(x, r) in cuts {
        // Never closer than the smaller leave, never beyond the larger.
        assert!(
            (RADIUS - 2e-3..=RADIUS + 0.5 + 2e-3).contains(&r),
            "{what}: x {x} r {r}"
        );
        // Further than the 3 mm ball can reach from the other zone, each
        // zone keeps its own amount.
        if x <= 16.0 {
            assert!((r - (RADIUS + 0.5)).abs() < 2e-3, "{what}: x {x} r {r}");
        }
        if x >= 24.0 {
            assert!((r - RADIUS).abs() < 2e-3, "{what}: x {x} r {r}");
        }
    }
}

#[test]
fn req_op_11_each_zone_is_left_its_own_stock_with_all_three_strategies() {
    let p = parts();
    let zones = [
        LeaveZone {
            surface: &p.low,
            leave: 0.5,
        },
        LeaveZone {
            surface: &p.high,
            leave: 0.0,
        },
    ];
    let input = p.fixture.input();
    let helical =
        run(|ctx| generate_in_zones(&HelicalFinishing, &input, None, &zones, &values(&[]), ctx))
            .unwrap();
    check_leaves(&radius_at(&helical), "helical");
    let axial = run(|ctx| {
        generate_in_zones(
            &AxialFinishing,
            &input,
            None,
            &zones,
            &values(&[(axial_parameter::MAX_X_STEP, 0.5)]),
            ctx,
        )
    })
    .unwrap();
    check_leaves(&radius_at(&axial), "axial");
    let indexed = run(|ctx| {
        generate_in_zones(
            &IndexedFinishing,
            &input,
            None,
            &zones,
            &values(&[(axial_parameter::MAX_X_STEP, 0.5)]),
            ctx,
        )
    })
    .unwrap();
    check_leaves(&radius_at(&indexed), "indexed");
}

#[test]
fn req_op_11_one_zone_of_the_operations_value_changes_nothing() {
    let p = parts();
    let input = p.fixture.input();
    let whole = run(|ctx| generate(&HelicalFinishing, &input, &values(&[]), ctx)).unwrap();
    let zones = [LeaveZone {
        surface: &p.fixture.surface,
        leave: 0.0,
    }];
    let zoned =
        run(|ctx| generate_in_zones(&HelicalFinishing, &input, None, &zones, &values(&[]), ctx))
            .unwrap();
    assert_eq!(whole.group, zoned.group);
}

#[test]
fn req_op_11_a_bad_amount_or_a_strategy_without_zones_is_refused() {
    let p = parts();
    let input = p.fixture.input();
    for leave in [-0.1, f64::NAN] {
        let zones = [LeaveZone {
            surface: &p.low,
            leave,
        }];
        let err = run(|ctx| {
            generate_in_zones(&HelicalFinishing, &input, None, &zones, &values(&[]), ctx)
        })
        .unwrap_err();
        assert!(err.to_string().contains("stock to leave"), "{err}");
    }

    #[derive(Debug)]
    struct NoZones;
    impl Strategy for NoZones {
        fn name(&self) -> &str {
            "no_zones"
        }
        fn label(&self) -> &str {
            "No zones"
        }
        fn schema(&self) -> openchuck_path::params::ParameterSchema {
            HelicalFinishing.schema()
        }
        fn passes(
            &self,
            _: &openchuck_path::StrategyInput<'_>,
            _: &openchuck_path::params::Parameters,
            _: &openchuck_types::job::JobContext<'_>,
        ) -> openchuck_path::Result<Vec<openchuck_path::Pass>> {
            Ok(Vec::new())
        }
    }
    let zones = [LeaveZone {
        surface: &p.low,
        leave: 0.5,
    }];
    let err = run(|ctx| generate_in_zones(&NoZones, &input, None, &zones, &values(&[]), ctx))
        .unwrap_err();
    assert!(err.to_string().contains("different stock"), "{err}");
}
