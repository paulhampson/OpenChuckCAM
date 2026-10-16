// SPDX-License-Identifier: AGPL-3.0-or-later
//! Limiting rotary roughing to selected regions, with a containment boundary
//! and avoid and check surfaces (OP-02; work breakdown slice 14 item 5,
//! decision record 0043).
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing
)]

mod support;

use std::collections::BTreeMap;

use openchuck_path::params::names;
use openchuck_path::region::{RegionConstraint, Window};
use openchuck_path::roughing::{NAME, parameter};
use openchuck_path::{
    GeneratedPath, MeshSurface, ParameterValue, PathError, RotaryRoughing, generate,
    generate_in_region,
};
use openchuck_testkit::parts::{Cylinder, RadialPart};
use openchuck_types::geometry::Point3;
use openchuck_types::ir::MoveKind;
use openchuck_types::mesh::TriangleMesh;
use support::{Fixture, ball, run};

const RADIUS: f64 = 10.0;
const LENGTH: f64 = 20.0;

fn values(form: &str) -> BTreeMap<String, ParameterValue> {
    let mut v: BTreeMap<String, ParameterValue> = [
        (names::STEPOVER, 3.0),
        (names::FEED, 600.0),
        (names::PLUNGE_FEED, 200.0),
        (names::STOCK_TO_LEAVE, 0.5),
        (parameter::STEPDOWN, 2.0),
    ]
    .into_iter()
    .map(|(k, x)| (k.to_owned(), ParameterValue::Number(x)))
    .collect();
    v.insert(
        parameter::FORM.to_owned(),
        ParameterValue::Text(form.to_owned()),
    );
    v
}

fn cylinder() -> TriangleMesh {
    Cylinder {
        radius: RADIUS,
        length: LENGTH,
    }
    .mesh(41, 360)
}

fn fixture(mesh: &TriangleMesh) -> Fixture {
    Fixture::new(MeshSurface::new(mesh).unwrap(), RADIUS + 5.0, &ball(6.0))
}

/// The triangles of `mesh` all of whose corners satisfy `keep`, on the same
/// vertices.
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

fn rough(
    f: &Fixture,
    form: &str,
    region: &RegionConstraint<'_>,
) -> Result<GeneratedPath, PathError> {
    run(|ctx| generate_in_region(&RotaryRoughing, &f.input(), region, &values(form), ctx))
}

/// The tips of the cutting moves.
fn cut_tips(path: &GeneratedPath) -> Vec<Point3> {
    path.group
        .segments
        .iter()
        .filter(|s| s.kind == MoveKind::Cutting)
        .map(|s| s.end.tip)
        .collect()
}

#[test]
fn req_op_02_roughing_honours_regions() {
    use openchuck_path::Strategy;
    assert_eq!(RotaryRoughing.name(), NAME);
    assert!(RotaryRoughing.honours_region());
}

#[test]
fn req_op_02_an_empty_constraint_changes_nothing_for_roughing() {
    let mesh = cylinder();
    let f = fixture(&mesh);
    for form in ["continuous", "indexed"] {
        let whole = run(|ctx| generate(&RotaryRoughing, &f.input(), &values(form), ctx)).unwrap();
        let limited = rough(&f, form, &RegionConstraint::default()).unwrap();
        assert_eq!(whole.group, limited.group, "{form}");
    }
}

#[test]
fn req_op_02_roughing_stays_on_the_selected_region() {
    let mesh = cylinder();
    let f = fixture(&mesh);
    let upper = MeshSurface::new(&pick(&mesh, |c| c.x >= 10.0 - 1e-9)).unwrap();
    let region = RegionConstraint {
        region: Some(&upper),
        ..Default::default()
    };
    for form in ["continuous", "indexed"] {
        let whole = run(|ctx| generate(&RotaryRoughing, &f.input(), &values(form), ctx)).unwrap();
        let tips = cut_tips(&rough(&f, form, &region).unwrap());
        assert!(!tips.is_empty(), "{form}");
        let lo = tips.iter().map(|p| p.x).fold(f64::MAX, f64::min);
        // A ball tool's contact is under its axis, so nothing is cut before
        // the region begins, and the cut runs up to the edge of it.
        assert!(lo >= 10.0 - 1e-3, "{form}: lowest cut at x = {lo}");
        assert!(lo <= 10.0 + 1.0, "{form}: the cut stops short at x = {lo}");
        // The first point of a pass is on the edge itself, found by
        // bisection, not on the last node before it (OI-187).
        let path = rough(&f, form, &region).unwrap();
        let first = path
            .group
            .segments
            .iter()
            .filter(|s| s.kind != MoveKind::Rapid)
            .map(|s| s.end.tip.x)
            .fold(f64::MAX, f64::min);
        assert!(
            (10.0 - 1e-3..=10.0 + 0.01).contains(&first),
            "{form}: nearest point to the edge at x = {first}"
        );
        assert!(
            tips.len() < cut_tips(&whole).len(),
            "{form}: the region did not reduce the cutting"
        );
    }
}

#[test]
fn req_op_02_roughing_never_cuts_an_avoid_surface() {
    let mesh = cylinder();
    let f = fixture(&mesh);
    let strip =
        MeshSurface::new(&pick(&mesh, |c| (8.0 - 1e-9..12.0 + 1e-9).contains(&c.x))).unwrap();
    let region = RegionConstraint {
        avoid: Some(&strip),
        ..Default::default()
    };
    for form in ["continuous", "indexed"] {
        let tips = cut_tips(&rough(&f, form, &region).unwrap());
        let inside = tips
            .iter()
            .filter(|p| p.x > 8.0 + 1e-3 && p.x < 12.0 - 1e-3)
            .count();
        assert_eq!(inside, 0, "{form}: {inside} cuts on the avoid surface");
        assert!(tips.iter().any(|p| p.x < 8.0), "{form}: nothing before");
        assert!(tips.iter().any(|p| p.x > 12.0), "{form}: nothing after");
    }
}

#[test]
fn req_op_02_roughing_stays_inside_a_containment_boundary() {
    let mesh = cylinder();
    let f = fixture(&mesh);
    let window =
        Window::of_mesh(&pick(&mesh, |c| (6.0 - 1e-9..14.0 + 1e-9).contains(&c.x))).unwrap();
    let region = RegionConstraint {
        boundary: Some(window),
        ..Default::default()
    };
    for form in ["continuous", "indexed"] {
        let tips = cut_tips(&rough(&f, form, &region).unwrap());
        assert!(!tips.is_empty(), "{form}");
        for p in &tips {
            assert!(
                (6.0 - 1e-3..=14.0 + 1e-3).contains(&p.x),
                "{form}: cut at x = {}",
                p.x
            );
        }
    }
}

#[test]
fn req_op_02_a_selection_the_tool_cannot_reach_is_an_error_for_roughing() {
    let mesh = cylinder();
    let f = fixture(&mesh);
    let far = TriangleMesh {
        vertices: vec![
            Point3 {
                x: 100.0,
                y: 0.0,
                z: 10.0,
            },
            Point3 {
                x: 101.0,
                y: 0.0,
                z: 10.0,
            },
            Point3 {
                x: 100.0,
                y: 1.0,
                z: 10.0,
            },
        ],
        triangles: vec![[0, 1, 2]],
    };
    let far = MeshSurface::new(&far).unwrap();
    let region = RegionConstraint {
        region: Some(&far),
        ..Default::default()
    };
    let error = rough(&f, "continuous", &region).unwrap_err();
    assert!(matches!(error, PathError::Strategy(_)), "{error}");
}

#[test]
fn req_nfr_06_regional_roughing_is_identical_at_1_2_and_n_threads() {
    let mesh = cylinder();
    let f = fixture(&mesh);
    let upper = MeshSurface::new(&pick(&mesh, |c| c.x >= 10.0 - 1e-9)).unwrap();
    let region = RegionConstraint {
        region: Some(&upper),
        ..Default::default()
    };
    let mut groups = Vec::new();
    for threads in [1, 2, 0] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        groups.push(pool.install(|| rough(&f, "continuous", &region).unwrap().group));
    }
    assert_eq!(groups[0], groups[1]);
    assert_eq!(groups[0], groups[2]);
}

#[test]
fn req_op_02_a_check_surface_keeps_roughing_above_it_and_beside_it_is_still_cut() {
    let mesh = cylinder();
    let f = fixture(&mesh);
    // A wider cylinder over the middle of the part: the check surface.
    let wide = Cylinder {
        radius: 12.0,
        length: LENGTH,
    }
    .mesh(41, 360);
    let strip =
        MeshSurface::new(&pick(&wide, |c| (8.0 - 1e-9..12.0 + 1e-9).contains(&c.x))).unwrap();
    let region = RegionConstraint {
        check: Some(&strip),
        ..Default::default()
    };
    for form in ["continuous", "indexed"] {
        let tips = cut_tips(&rough(&f, form, &region).unwrap());
        let over: Vec<f64> = tips
            .iter()
            .filter(|p| p.x > 8.0 + 0.3 && p.x < 12.0 - 0.3)
            .map(|p| p.y.hypot(p.z))
            .collect();
        // Material above the check surface is still removed, with the tool
        // kept the stock to leave (0.5 mm) clear of it.
        assert!(!over.is_empty(), "{form}: nothing is cut over the surface");
        let lowest = over.iter().copied().fold(f64::MAX, f64::min);
        assert!(lowest >= 12.5 - 0.05, "{form}: tool at radius {lowest}");
        // Beside it the tool goes down to the part.
        assert!(
            tips.iter().any(|p| p.x < 7.0 && p.y.hypot(p.z) < 11.0),
            "{form}: the part beside the surface is not roughed"
        );
    }
}

#[test]
fn req_op_02_a_check_surface_beyond_the_part_still_keeps_roughing_above_it() {
    let mesh = cylinder();
    let mut f = fixture(&mesh);
    // Stock that runs well past the part's end, so there is material to cut
    // where the part is not under the tool.
    f.stock = openchuck_path::AnalyticStock::Cylinder {
        radius: RADIUS + 5.0,
        x_start: -1.0,
        x_end: 31.0,
    };
    // A wider, longer cylinder than the part, checked only past the part's end.
    let wide = Cylinder {
        radius: 12.0,
        length: 30.0,
    }
    .mesh(61, 360);
    let beyond =
        MeshSurface::new(&pick(&wide, |c| (22.0 - 1e-9..28.0 + 1e-9).contains(&c.x))).unwrap();
    let region = RegionConstraint {
        check: Some(&beyond),
        ..Default::default()
    };
    for form in ["continuous", "indexed"] {
        let tips = cut_tips(&rough(&f, form, &region).unwrap());
        let over: Vec<f64> = tips
            .iter()
            .filter(|p| p.x > 22.3 && p.x < 27.7)
            .map(|p| p.y.hypot(p.z))
            .collect();
        assert!(!over.is_empty(), "{form}: nothing is cut over the surface");
        let lowest = over.iter().copied().fold(f64::MAX, f64::min);
        assert!(lowest >= 12.5 - 0.05, "{form}: tool at radius {lowest}");
    }
}
