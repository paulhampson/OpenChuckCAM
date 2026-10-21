// SPDX-License-Identifier: AGPL-3.0-or-later
//! Limiting helical finishing to selected regions, with a containment
//! boundary and avoid and check surfaces (OP-02; work breakdown slice 10
//! item 5 and its review checklist).
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing
)]

mod support;

use std::collections::BTreeMap;

use openchuck_path::params::names;
use openchuck_path::region::{
    BoundaryLimits, Protection, RegionConstraint, WatchedSurface, Window,
};
use openchuck_path::{
    GeneratedPath, HelicalFinishing, MeshSurface, ParameterValue, PathError, generate,
    generate_in_region,
};
use openchuck_testkit::parts::{Cylinder, RadialPart};
use openchuck_types::geometry::Point3;
use openchuck_types::ir::MoveKind;
use openchuck_types::mesh::TriangleMesh;
use support::{Fixture, ball, run};

const RADIUS: f64 = 15.0;
const LENGTH: f64 = 40.0;

fn values() -> BTreeMap<String, ParameterValue> {
    [
        (names::STEPOVER, 1.0),
        (names::FEED, 300.0),
        (names::PLUNGE_FEED, 100.0),
    ]
    .into_iter()
    .map(|(k, x)| (k.to_owned(), ParameterValue::Number(x)))
    .collect()
}

fn cylinder() -> TriangleMesh {
    Cylinder {
        radius: RADIUS,
        length: LENGTH,
    }
    .mesh(81, 720)
}

fn fixture(mesh: &TriangleMesh) -> Fixture {
    Fixture::new(MeshSurface::new(mesh).unwrap(), RADIUS + 1.0, &ball(6.0))
}

/// The triangles of `mesh` all of whose corners satisfy `keep`, on the same
/// vertices. (Selecting by corner keeps the edge of a selection on a ring of
/// the mesh, where the test can say exactly where it is.)
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

fn run_with(f: &Fixture, region: &RegionConstraint<'_>) -> Result<GeneratedPath, PathError> {
    run(|ctx| generate_in_region(&HelicalFinishing, &f.input(), region, &values(), ctx))
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

fn angle(p: Point3) -> f64 {
    p.z.atan2(p.y).to_degrees()
}

#[test]
fn req_op_02_cuts_stay_on_the_selected_region_and_reach_its_edges() {
    let mesh = cylinder();
    let f = fixture(&mesh);
    let upper = MeshSurface::new(&pick(&mesh, |c| c.x >= 20.0 - 1e-9)).unwrap();
    let region = RegionConstraint {
        region: Some(&upper),
        ..Default::default()
    };
    let path = run_with(&f, &region).unwrap();
    let tips = cut_tips(&path);
    assert!(!tips.is_empty());
    let (lo, hi) = tips
        .iter()
        .fold((f64::MAX, f64::MIN), |(l, h), p| (l.min(p.x), h.max(p.x)));
    // Nothing is cut outside the region, and the cut runs right up to the
    // edge of it (within a small fraction of the step) and to the far end.
    assert!(lo >= 20.0 - 1e-3, "lowest cut at x = {lo}");
    assert!(lo <= 20.0 + 0.05, "the cut stops short at x = {lo}");
    assert!(hi >= LENGTH - 0.1, "highest cut at x = {hi}");
}

#[test]
fn req_op_02_an_empty_constraint_changes_nothing() {
    let mesh = cylinder();
    let f = fixture(&mesh);
    let whole = run(|ctx| generate(&HelicalFinishing, &f.input(), &values(), ctx)).unwrap();
    let limited = run_with(&f, &RegionConstraint::default()).unwrap();
    assert_eq!(whole.group, limited.group);
}

#[test]
fn req_op_02_a_strategy_without_restriction_options_refuses_them() {
    // A boundary limit turned off, or a set that only warns, is built for
    // swarf only (decision 0176; OI-637): another strategy refuses it,
    // naming itself, rather than ignoring it.
    let mesh = cylinder();
    let f = fixture(&mesh);
    let strip =
        MeshSurface::new(&pick(&mesh, |c| (18.0 - 1e-9..22.0 + 1e-9).contains(&c.x))).unwrap();
    let watch = [WatchedSurface {
        name: "Strip",
        protection: Protection::Avoid,
        surface: &strip,
    }];
    let window = Window {
        x_min: 0.0,
        x_max: 20.0,
        arc: None,
    };
    for region in [
        RegionConstraint {
            watch: &watch,
            ..Default::default()
        },
        RegionConstraint {
            boundary: Some(window),
            boundary_limits: BoundaryLimits {
                pass: true,
                tip: false,
            },
            ..Default::default()
        },
    ] {
        assert!(region.has_restriction_options());
        let err = run_with(&f, &region).unwrap_err().to_string();
        assert!(
            err.contains("takes no options on its boundary, avoid or check sets"),
            "{err}"
        );
    }
    // The default options are no options.
    let plain = RegionConstraint {
        boundary: Some(window),
        ..Default::default()
    };
    assert!(!plain.has_restriction_options());
    assert!(run_with(&f, &plain).is_ok());
}

#[test]
fn req_op_02_an_avoid_surface_is_never_cut() {
    let mesh = cylinder();
    let f = fixture(&mesh);
    let strip =
        MeshSurface::new(&pick(&mesh, |c| (18.0 - 1e-9..22.0 + 1e-9).contains(&c.x))).unwrap();
    let region = RegionConstraint {
        avoid: Some(&strip),
        ..Default::default()
    };
    let path = run_with(&f, &region).unwrap();
    let tips = cut_tips(&path);
    let inside: Vec<_> = tips
        .iter()
        .filter(|p| p.x > 18.0 + 1e-3 && p.x < 22.0 - 1e-3)
        .collect();
    assert!(
        inside.is_empty(),
        "{} cuts on the avoid surface",
        inside.len()
    );
    // Either side is still cut, right up to the strip.
    assert!(tips.iter().any(|p| p.x < 18.0 && p.x > 17.9));
    assert!(tips.iter().any(|p| p.x > 22.0 && p.x < 22.1));
    assert!(tips.iter().any(|p| p.x < 5.0) && tips.iter().any(|p| p.x > 35.0));
}

#[test]
fn req_op_02_a_small_avoid_patch_between_two_default_seeds_is_not_missed() {
    // 1 degree wide (0.26 mm) and half a millimetre long: the helix crosses
    // each once, and the default angle step is five times as wide. Patches at
    // several angles, so some fall between the default seeds and their middles.
    let mesh = cylinder();
    let f = fixture(&mesh);
    for start in [-9.0, -4.0, 1.0, 3.0, 6.0, 8.0, 12.0] {
        let patch = pick(&mesh, |c| {
            (20.0 - 1e-9..20.5 + 1e-9).contains(&c.x)
                && (start - 1e-6..start + 1.0 + 1e-6).contains(&angle(c))
        });
        assert!(!patch.triangles.is_empty());
        let patch = MeshSurface::new(&patch).unwrap();
        let region = RegionConstraint {
            avoid: Some(&patch),
            ..Default::default()
        };
        let path = run_with(&f, &region).unwrap();
        let free = run(|ctx| generate(&HelicalFinishing, &f.input(), &values(), ctx)).unwrap();
        let on_patch = |path: &GeneratedPath| {
            let mut prev = path.group.start.tip;
            let mut count = 0;
            for s in &path.group.segments {
                if s.kind == MoveKind::Cutting {
                    // Along the move, between its two ends.
                    for k in 1..=40 {
                        let w = f64::from(k) / 40.0;
                        let p = Point3::new(
                            prev.x + (s.end.tip.x - prev.x) * w,
                            prev.y + (s.end.tip.y - prev.y) * w,
                            prev.z + (s.end.tip.z - prev.z) * w,
                        );
                        if p.x > 20.0 + 1e-3
                            && p.x < 20.5 - 1e-3
                            && angle(p) > start + 1e-3
                            && angle(p) < start + 1.0 - 1e-3
                        {
                            count += 1;
                        }
                    }
                }
                prev = s.end.tip;
            }
            count
        };
        // The free path does go over the patch, so the test can fail.
        assert!(
            on_patch(&free) > 0,
            "patch at {start} degrees is not crossed"
        );
        let on_patch = on_patch(&path);
        assert_eq!(on_patch, 0, "patch at {start} degrees");
    }
}

#[test]
fn req_op_02_the_tool_does_not_touch_a_check_surface() {
    let mesh = cylinder();
    let f = fixture(&mesh);
    let strip =
        MeshSurface::new(&pick(&mesh, |c| (18.0 - 1e-9..22.0 + 1e-9).contains(&c.x))).unwrap();
    let region = RegionConstraint {
        check: Some(&strip),
        ..Default::default()
    };
    let tips = cut_tips(&run_with(&f, &region).unwrap());
    assert!(tips.iter().all(|p| !(p.x > 18.05 && p.x < 21.95)));
    assert!(tips.iter().any(|p| p.x < 18.0) && tips.iter().any(|p| p.x > 22.0));
}

#[test]
fn req_op_02_a_boundary_limits_x_and_angle() {
    let mesh = cylinder();
    let f = fixture(&mesh);
    // A quarter-turn patch, x from 10 to 20, at angles 0 to 90 degrees.
    let patch = pick(&mesh, |c| {
        (10.0 - 1e-9..20.0 + 1e-9).contains(&c.x)
            && (-1e-6..90.0 + 1e-6).contains(&c.z.atan2(c.y).to_degrees())
    });
    let window = Window::of_mesh(&patch).unwrap();
    assert!((window.x_min - 10.0).abs() < 0.5 && (window.x_max - 20.0).abs() < 0.5);
    let (_, extent) = window.arc.unwrap();
    assert!((extent.to_degrees() - 90.0).abs() < 1.0, "{extent}");
    // A full ring has no angular limit.
    let ring = pick(&mesh, |c| (10.0 - 1e-9..20.0 + 1e-9).contains(&c.x));
    assert_eq!(Window::of_mesh(&ring).unwrap().arc, None);

    let region = RegionConstraint {
        boundary: Some(window),
        ..Default::default()
    };
    let tips = cut_tips(&run_with(&f, &region).unwrap());
    assert!(!tips.is_empty());
    for p in &tips {
        assert!(
            p.x > 10.0 - 0.5 - 1e-3 && p.x < 20.0 + 0.5 + 1e-3,
            "x = {}",
            p.x
        );
        let a = angle(*p);
        assert!(a > -1.0 && a < 91.0, "angle = {a}");
    }
}

#[test]
fn req_op_02_a_region_and_an_avoid_surface_combine() {
    let mesh = cylinder();
    let f = fixture(&mesh);
    let upper = MeshSurface::new(&pick(&mesh, |c| c.x >= 10.0 - 1e-9)).unwrap();
    let strip =
        MeshSurface::new(&pick(&mesh, |c| (18.0 - 1e-9..22.0 + 1e-9).contains(&c.x))).unwrap();
    let region = RegionConstraint {
        region: Some(&upper),
        avoid: Some(&strip),
        ..Default::default()
    };
    let tips = cut_tips(&run_with(&f, &region).unwrap());
    assert!(tips.iter().all(|p| p.x >= 10.0 - 1e-3));
    assert!(
        tips.iter()
            .all(|p| !(p.x > 18.0 + 1e-3 && p.x < 22.0 - 1e-3))
    );
    assert!(tips.iter().any(|p| p.x < 18.0) && tips.iter().any(|p| p.x > 22.0));
}

#[test]
fn req_op_02_req_nfr_07_a_constraint_with_nothing_to_cut_is_an_error() {
    let mesh = cylinder();
    let f = fixture(&mesh);
    // The avoid surface is the whole part.
    let all = MeshSurface::new(&mesh).unwrap();
    let region = RegionConstraint {
        avoid: Some(&all),
        ..Default::default()
    };
    let error = run_with(&f, &region).unwrap_err();
    assert!(error.to_string().contains("no cut"), "{error}");
    // A boundary of no triangles cannot be made.
    let none = pick(&mesh, |_| false);
    assert!(Window::of_mesh(&none).is_err());
}
