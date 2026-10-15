// SPDX-License-Identifier: AGPL-3.0-or-later
//! Regions found through their offset instead of by sampling (OP-02; work
//! breakdown slice 12 item 5; decision record 0033 item 5; OI-138).
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
use openchuck_path::region::RegionConstraint;
use openchuck_path::{
    AxialFinishing, GeneratedPath, HelicalFinishing, IndexedFinishing, MeshSurface, ParameterValue,
    Strategy, generate_in_region,
};
use openchuck_testkit::parts::{Cylinder, RadialPart};
use openchuck_types::geometry::Point3;
use openchuck_types::ir::MoveKind;
use openchuck_types::mesh::TriangleMesh;
use support::{Fixture, ball, run};

const RADIUS: f64 = 15.0;
const LENGTH: f64 = 40.0;
/// Rings of the mesh: 0.04 mm apart.
const RINGS: u32 = 1001;

fn values(extra: &[(&str, ParameterValue)]) -> BTreeMap<String, ParameterValue> {
    let mut v: BTreeMap<String, ParameterValue> = [
        (names::STEPOVER, 3.0),
        (names::FEED, 300.0),
        (names::PLUNGE_FEED, 100.0),
    ]
    .into_iter()
    .map(|(k, x)| (k.to_owned(), ParameterValue::Number(x)))
    .collect();
    for (k, x) in extra {
        v.insert((*k).to_owned(), x.clone());
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

struct Setup {
    fixture: Fixture,
    strip: MeshSurface,
}

/// A cylinder with a ring of one mesh row selected: a band 0.04 mm wide.
fn setup() -> Setup {
    let mesh = Cylinder {
        radius: RADIUS,
        length: LENGTH,
    }
    .mesh(RINGS, 360);
    // Between two of the old 0.1 mm points (X = 20.0 and 20.1).
    let (x0, x1) = (20.04 - 1e-6, 20.08 + 1e-6);
    Setup {
        fixture: Fixture::new(MeshSurface::new(&mesh).unwrap(), RADIUS + 1.0, &ball(6.0)),
        strip: MeshSurface::new(&pick(&mesh, |p| p.x >= x0 && p.x <= x1)).unwrap(),
    }
}

fn cut(s: &Setup, strategy: &dyn Strategy, extra: &[(&str, ParameterValue)]) -> GeneratedPath {
    let region = RegionConstraint {
        region: Some(&s.strip),
        ..Default::default()
    };
    run(|ctx| generate_in_region(strategy, &s.fixture.input(), &region, &values(extra), ctx))
        .unwrap()
}

/// The X of every cutting move's end.
fn cut_x(path: &GeneratedPath) -> Vec<f64> {
    path.group
        .segments
        .iter()
        .filter(|s| s.kind == MoveKind::Cutting)
        .map(|s| s.end.tip.x)
        .collect()
}

#[test]
fn req_op_02_req_op_04_a_band_narrower_than_the_old_spacing_is_cut_on_every_pass() {
    let s = setup();
    let path = cut(
        &s,
        &AxialFinishing,
        &[(axial_parameter::MAX_X_STEP, ParameterValue::Number(0.5))],
    );
    let xs = cut_x(&path);
    assert!(!xs.is_empty());
    // Only the band is cut: the ball rides the surface under the tool, so the
    // tool's X is the contact's X (to the tolerance and the mesh).
    for x in &xs {
        assert!(
            (20.02..=20.10).contains(x),
            "a cut at x = {x} is outside the band"
        );
    }
    // Every pass (every A) finds it: 3 mm stepover on R = 15 is 32 passes.
    let mut angles: Vec<i64> = path
        .group
        .segments
        .iter()
        .filter(|s| s.kind == MoveKind::Cutting)
        .map(|s| (s.end.a.0 * 1000.0).round() as i64)
        .collect();
    angles.sort_unstable();
    angles.dedup();
    let passes = (std::f64::consts::TAU * RADIUS / 3.0).ceil() as usize;
    assert_eq!(angles.len(), passes, "a pass missed the band");
}

#[test]
fn req_op_02_req_op_04_helical_and_indexed_find_the_band_too() {
    let s = setup();
    let helical = cut(&s, &HelicalFinishing, &[]);
    let xs = cut_x(&helical);
    assert!(!xs.is_empty());
    for x in &xs {
        assert!(
            (20.02..=20.10).contains(x),
            "a cut at x = {x} is outside the band"
        );
    }
    // The helix (pitch 3 mm) crosses a band 0.04 mm wide once, in 360 * 0.04 / 3
    // = 4.8 degrees of A, less than the 5 degree step: one move.
    assert_eq!(xs.len(), 1, "{xs:?}");

    let indexed = cut(
        &s,
        &IndexedFinishing,
        &[("angles", ParameterValue::Text("0, 90, 180, 270".into()))],
    );
    let mut angles: Vec<i64> = indexed
        .group
        .segments
        .iter()
        .filter(|s| s.kind == MoveKind::Cutting)
        .map(|s| (s.end.a.0 * 1000.0).round() as i64)
        .collect();
    angles.dedup();
    assert_eq!(angles.len(), 4);
}

#[test]
fn req_op_02_a_pass_that_cannot_reach_the_region_drops_nothing() {
    // A region at one end: passes elsewhere along the part are not sampled,
    // so the work is a small part of the whole-part work.
    let mesh = Cylinder {
        radius: RADIUS,
        length: LENGTH,
    }
    .mesh(81, 360);
    let f = Fixture::new(MeshSurface::new(&mesh).unwrap(), RADIUS + 1.0, &ball(6.0));
    let region_mesh = pick(&mesh, |p| p.x <= 2.0 + 1e-9);
    let region = MeshSurface::new(&region_mesh).unwrap();
    let constraint = RegionConstraint {
        region: Some(&region),
        ..Default::default()
    };
    let path =
        run(|ctx| generate_in_region(&AxialFinishing, &f.input(), &constraint, &values(&[]), ctx))
            .unwrap();
    let xs = cut_x(&path);
    assert!(!xs.is_empty());
    assert!(
        xs.iter().all(|x| *x <= 2.0 + 1e-3),
        "{:?}",
        xs.iter().cloned().fold(0.0, f64::max)
    );
    // The region reaches the end of the part: the pass reaches X = 0 and 2.
    assert!(xs.iter().cloned().fold(f64::MAX, f64::min) < 0.01);
}
