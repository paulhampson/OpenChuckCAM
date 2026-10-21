// SPDX-License-Identifier: AGPL-3.0-or-later
//! Remaining-material detection, crease finding and the parameters of rest
//! machining and pencil clean-up (OP-09, OP-16; work breakdown slice 15;
//! decision record 0044). The strategies against the simulated stock are in
//! `crates/openchuck-sim/tests/rest.rs`.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing
)]

use std::collections::BTreeMap;

use openchuck_path::pencil::{concave_creases, pencil_parameter};
use openchuck_path::rest::{DEFAULT_MIN_REMAINING, RemainingMap, parameter};
use openchuck_path::{
    AnalyticStock, MeshSurface, ParameterValue, PencilFinishing, Registry, RestMachining, Strategy,
    SurfaceSource,
};
use openchuck_testkit::parts::revolve;
use openchuck_types::job::{CancelToken, JobContext, NoProgress};
use openchuck_types::stock::Stock;
use openchuck_types::units::Millimetres;

const SHAFT: [(f64, f64); 4] = [(0.0, 6.0), (12.0, 6.0), (13.6, 12.0), (24.0, 12.0)];
const WALLED: [(f64, f64); 4] = [(0.0, 6.0), (12.0, 6.0), (12.0, 12.0), (24.0, 12.0)];

fn detect(stock: &AnalyticStock, probe: f64) -> RemainingMap {
    let surface = MeshSurface::new(&revolve(&SHAFT, 90)).unwrap();
    let cancel = CancelToken::new();
    let ctx = JobContext::new(&cancel, &NoProgress);
    RemainingMap::detect(&surface, stock, probe, 0.5, 1.0, &ctx).unwrap()
}

fn cylinder(radius: f64) -> AnalyticStock {
    AnalyticStock::from_stock(
        &Stock::Cylinder {
            diameter: Millimetres(2.0 * radius),
            length: Millimetres(24.0),
            x_start: Millimetres(0.0),
        },
        None,
    )
    .unwrap()
}

#[test]
fn req_op_09_stock_larger_than_the_part_has_material_left_everywhere_and_stock_at_it_has_none() {
    // Stock 3 mm over the large radius: every side sample has material 0.15
    // out. The end faces are at the stock's ends, so they have none.
    let proud = detect(&cylinder(15.0), 0.15);
    assert!(proud.len() > 1000, "{}", proud.len());
    assert!(proud.len() < proud.tested(), "the end faces are clean");
    // Stock that is the large cylinder only: the small shaft has material
    // left, the large cylinder none.
    let large = detect(&cylinder(12.0), 0.15);
    assert!(!large.is_empty() && large.len() < proud.len());
    assert!(
        large
            .samples()
            .iter()
            .all(|p| p.x < 14.0 || p.y.hypot(p.z) < 11.0)
    );
    // A threshold above the material there is leaves nothing.
    assert!(detect(&cylinder(12.0), 20.0).is_empty());
}

#[test]
fn req_op_09_near_finds_a_sample_within_the_reach_and_not_beyond_it() {
    let map = detect(&cylinder(12.0), 0.15);
    let sample = map.samples()[0];
    assert!(map.near(sample));
    let mut away = sample;
    away.x += 100.0;
    assert!(!map.near(away));
}

#[test]
fn req_op_09_detection_is_deterministic() {
    let a = detect(&cylinder(15.0), 0.15);
    let b = detect(&cylinder(15.0), 0.15);
    assert_eq!(a.samples(), b.samples());
    assert_eq!(a.len(), b.len());
}

#[test]
fn req_op_09_the_inside_corner_of_a_step_is_one_closed_crease() {
    for profile in [SHAFT, WALLED] {
        let mesh = revolve(&profile, 90);
        let creases = concave_creases(&mesh, 30.0);
        assert_eq!(creases.len(), 1, "{profile:?}");
        let crease = &creases[0];
        assert!(crease.closed);
        // The ring is at X = 12 and the small radius, with the first point
        // repeated at the end.
        assert_eq!(crease.points.len(), 91);
        for p in &crease.points {
            assert!((p.x - 12.0).abs() < 1e-6 && (p.y.hypot(p.z) - 6.0).abs() < 1e-6);
        }
        assert_eq!(crease.points.first(), crease.points.last());
    }
}

#[test]
fn req_op_09_smooth_and_convex_edges_are_not_creases() {
    // A plain cylinder has only convex edges at its ends.
    assert!(concave_creases(&revolve(&[(0.0, 8.0), (30.0, 8.0)], 60), 20.0).is_empty());
    // The flank turns 75 degrees; asked for 80 it is not a crease.
    assert!(concave_creases(&revolve(&SHAFT, 90), 80.0).is_empty());
    assert_eq!(concave_creases(&revolve(&SHAFT, 90), 70.0).len(), 1);
}

#[test]
fn req_op_09_creases_join_a_mesh_with_a_vertex_per_triangle() {
    let mesh = revolve(&SHAFT, 60);
    let mut loose = openchuck_types::mesh::TriangleMesh::default();
    for tri in &mesh.triangles {
        let base = loose.vertices.len() as u32;
        for &v in tri {
            loose.vertices.push(mesh.vertices[v as usize]);
        }
        loose.triangles.push([base, base + 1, base + 2]);
    }
    let creases = concave_creases(&loose, 30.0);
    assert_eq!(creases.len(), 1);
    assert!(creases[0].closed);
}

fn resolve(strategy: &dyn Strategy, extra: &[(&str, f64)]) -> openchuck_path::Result<()> {
    let mut v: BTreeMap<String, ParameterValue> =
        [("stepover", 1.0), ("feed", 400.0), ("plunge_feed", 200.0)]
            .into_iter()
            .map(|(k, x)| (k.to_owned(), ParameterValue::Number(x)))
            .collect();
    for (k, x) in extra {
        v.insert((*k).to_owned(), ParameterValue::Number(*x));
    }
    strategy.schema().resolve(&v).map(|_| ())
}

#[test]
fn req_op_16_the_new_strategies_are_registered_and_check_their_parameters() {
    let registry = Registry::builtin();
    for name in [openchuck_path::rest::NAME, openchuck_path::pencil::NAME] {
        assert!(registry.get(name).is_ok(), "{name}");
    }
    assert!(resolve(&RestMachining, &[]).is_ok());
    assert!(resolve(&PencilFinishing, &[]).is_ok());
    let defaults = RestMachining.schema().resolve(&BTreeMap::from([
        ("stepover".to_owned(), ParameterValue::Number(1.0)),
        ("feed".to_owned(), ParameterValue::Number(400.0)),
        ("plunge_feed".to_owned(), ParameterValue::Number(200.0)),
    ]));
    assert_eq!(
        defaults.unwrap().number(parameter::MIN_REMAINING).unwrap(),
        DEFAULT_MIN_REMAINING
    );
    for bad in [
        (parameter::MIN_REMAINING, -0.1),
        (parameter::MAX_ANGLE_STEP, 0.0),
    ] {
        let err = resolve(&RestMachining, &[bad]).unwrap_err().to_string();
        assert!(err.contains(bad.0), "{err}");
    }
    for bad in [
        (pencil_parameter::CREASE_ANGLE, 0.0),
        (pencil_parameter::CREASE_ANGLE, 181.0),
        (pencil_parameter::MAX_STEP, 0.0),
    ] {
        let err = resolve(&PencilFinishing, &[bad]).unwrap_err().to_string();
        assert!(err.contains(bad.0), "{err}");
    }
}

#[test]
fn req_op_09_detection_refuses_nonsense_without_panicking() {
    let surface = MeshSurface::new(&revolve(&SHAFT, 30)).unwrap();
    let stock = cylinder(15.0);
    let cancel = CancelToken::new();
    let ctx = JobContext::new(&cancel, &NoProgress);
    for (probe, spacing, reach) in [
        (-1.0, 0.5, 1.0),
        (f64::NAN, 0.5, 1.0),
        (0.1, 0.0, 1.0),
        (0.1, 0.5, 0.0),
    ] {
        assert!(
            RemainingMap::detect(&surface, &stock, probe, spacing, reach, &ctx).is_err(),
            "{probe} {spacing} {reach}"
        );
    }
    assert!(surface.bounds().max.x > 0.0);
}

/// Stock everywhere but in balls: the material an earlier operation cleaned
/// out round some points.
#[derive(Debug)]
struct Holed {
    centres: Vec<openchuck_types::geometry::Point3>,
    radius: f64,
}

impl openchuck_path::StockModel for Holed {
    fn contains(&self, p: openchuck_types::geometry::Point3) -> bool {
        self.centres.iter().all(|c| p.distance(*c) > self.radius)
    }

    fn ray_entry(
        &self,
        _origin: openchuck_types::geometry::Point3,
        _direction: openchuck_types::geometry::UnitVec3,
        _max_distance: f64,
    ) -> Option<f64> {
        None
    }

    fn max_radius(&self) -> Option<f64> {
        Some(20.0)
    }
}

#[test]
fn req_op_09_a_closed_crease_clean_at_its_start_is_not_joined_across_the_gap() {
    use openchuck_path::{AcceptAll, RotaryMapping, StrategyInput};
    use openchuck_types::geometry::Point3;
    use openchuck_types::ir::PathTags;
    use openchuck_types::machine::{Clearance, MachineProfile};
    use openchuck_types::tool::{Tool, ToolShape};

    let surface = MeshSurface::new(&revolve(&SHAFT, 90)).unwrap();
    // The ring's first vertex is at angle 0: (12, 6, 0). The stock is gone
    // there and half way round, so the crease is clean at its start and in
    // the middle: two stretches that must not be joined across the start.
    let stock = Holed {
        centres: vec![Point3::new(12.0, 6.0, 0.0), Point3::new(12.0, -6.0, 0.0)],
        radius: 2.5,
    };
    let tool = Tool {
        shape: ToolShape::BallEnd,
        diameter: Millimetres(1.0),
        flute_length: Millimetres(10.0),
        overall_length: Millimetres(40.0),
        shank_diameter: Millimetres(1.0),
        flute_count: 2,
        material: None,
        centre_cutting: None,
    }
    .profile()
    .unwrap();
    let machine = MachineProfile::example_x_rotary();
    let tags = PathTags {
        operation: "op".to_owned(),
        setup: "setup".to_owned(),
        tool: "tool".to_owned(),
    };
    let input = StrategyInput {
        surface: &surface,
        stock: &stock,
        collision: &AcceptAll,
        tool: &tool,
        rotary: RotaryMapping::of_profile(&machine),
        clearance: Clearance {
            radius: Millimetres(19.0),
            safe_z: Millimetres(29.0),
        },
        max_feeds: machine.max_feeds,
        tags: &tags,
        keep_out: &[],
    };
    let values: BTreeMap<String, ParameterValue> =
        [("feed", 400.0), ("plunge_feed", 200.0), ("stepover", 1.0)]
            .into_iter()
            .map(|(k, x)| (k.to_owned(), ParameterValue::Number(x)))
            .collect();
    let parameters = PencilFinishing.schema().resolve(&values).unwrap();
    let cancel = CancelToken::new();
    let ctx = JobContext::new(&cancel, &NoProgress);
    let passes = PencilFinishing.passes(&input, &parameters, &ctx).unwrap();
    assert_eq!(passes.len(), 2, "two stretches of crease have material");
    for pass in &passes {
        for pair in pass.points.windows(2) {
            let jump = pair[0].tip.distance(pair[1].tip);
            assert!(
                jump < 1.0,
                "a pass jumps {jump} mm across the clean stretch"
            );
        }
    }
}

#[test]
fn req_op_09_a_part_with_more_triangles_than_samples_allowed_is_refused_not_looped_on() {
    // 2.4 million triangles: more than the 2 million samples allowed, with
    // one sample each at the least.
    let mesh = revolve(&[(0.0, 6.0), (12.0, 6.0)], 600_000);
    assert!(mesh.triangles.len() > 2_000_000);
    let surface = MeshSurface::new(&mesh).unwrap();
    let cancel = CancelToken::new();
    let ctx = JobContext::new(&cancel, &NoProgress);
    let err = RemainingMap::detect(&surface, &cylinder(15.0), 0.15, 0.5, 1.0, &ctx)
        .unwrap_err()
        .to_string();
    assert!(err.contains("at least one sample per triangle"), "{err}");
}
