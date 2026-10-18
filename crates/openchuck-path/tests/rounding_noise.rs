// SPDX-License-Identifier: AGPL-3.0-or-later
//! The toolpath does not depend on rounding noise (GC-07, NFR-06; decision
//! record 0112, closing OI-319).
//!
//! The platform's mathematics library (`sin`, `cos`, `atan2`) differs in the
//! last bit between Linux, Windows and macOS, so every geometric number the
//! library computes may differ by an ulp or so from one platform to the
//! next. The output must not: a choice between two answers that are equal
//! up to rounding (two facets at a mesh edge, two flanks a ball rests on at
//! once) must be made by a rule that rounding cannot flip, and a search for
//! a contact must find it to rounding precision, not to the square root of
//! it. Each test here perturbs the input by one unit in the last place and
//! requires the same answer.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing
)]

mod support;

use std::collections::BTreeMap;
use std::path::Path;

use openchuck_path::helical::parameter;
use openchuck_path::params::names;
use openchuck_path::{
    GeneratedPath, HelicalFinishing, MeshSurface, ParameterValue, SurfaceSource, ToolLine, generate,
};
use openchuck_types::geometry::{Point3, UnitVec3, Vec3};
use openchuck_types::mesh::TriangleMesh;
use support::{Fixture, ball, run};

fn mesh(vertices: &[[f64; 3]], triangles: &[[u32; 3]]) -> TriangleMesh {
    TriangleMesh {
        vertices: vertices
            .iter()
            .map(|v| Point3::new(v[0], v[1], v[2]))
            .collect(),
        triangles: triangles.to_vec(),
    }
}

fn vertical(x: f64, y: f64) -> ToolLine {
    ToolLine {
        origin: Point3::new(x, y, 0.0),
        axis: UnitVec3::Z,
    }
}

/// The chosen vertices' coordinates across the rotary axis (y and z) moved
/// one unit in the last place, upwards or downwards. The x coordinates stay,
/// so the part's length along the axis, which sets the number of seeds of a
/// pass, is the same.
fn nudged(m: &TriangleMesh, up: bool, which: impl Fn(usize) -> bool) -> TriangleMesh {
    let step = |v: f64| if up { v.next_up() } else { v.next_down() };
    TriangleMesh {
        vertices: m
            .vertices
            .iter()
            .enumerate()
            .map(|(i, p)| {
                if which(i) {
                    Point3::new(p.x, step(p.y), step(p.z))
                } else {
                    *p
                }
            })
            .collect(),
        triangles: m.triangles.clone(),
    }
}

/// A fold: the plane z = 0 for y below 0 and the plane z = y above it. The
/// two facets share the edge along x. A point on that edge is as close to
/// one facet as to the other, so the normal there must not depend on which
/// facet a rounding error favours: it is the same from either side.
#[test]
fn req_gc_07_req_nfr_06_a_point_on_a_mesh_edge_projects_to_the_same_normal_from_either_side() {
    let m = mesh(
        &[
            [0.0, 0.0, 0.0],
            [10.0, 0.0, 0.0],
            [0.0, -5.0, 0.0],
            [10.0, -5.0, 0.0],
            [0.0, 5.0, 5.0],
            [10.0, 5.0, 5.0],
        ],
        &[[0, 2, 1], [1, 2, 3], [0, 1, 4], [1, 5, 4]],
    );
    let surface = MeshSurface::new(&m).unwrap();
    let on = surface.project(Point3::new(5.0, 0.0, 0.0)).unwrap();
    let below = surface.project(Point3::new(5.0, -1e-13, 0.0)).unwrap();
    let above = surface.project(Point3::new(5.0, 1e-13, 0.0)).unwrap();
    // The flat facet's normal is +Z, the sloping facet's is (0, -1, 1) / sqrt 2;
    // on the edge the normal is their mean direction, whichever side the
    // query falls.
    let mean = UnitVec3::new(
        Vec3::new(0.0, -1.0, 1.0) * std::f64::consts::FRAC_1_SQRT_2 + Vec3::new(0.0, 0.0, 1.0),
    )
    .unwrap()
    .get();
    for (name, p) in [("on", on), ("below", below), ("above", above)] {
        let n = p.normal.get();
        assert!(
            (n - mean).length() < 1e-9,
            "{name}: normal {n:?}, expected the edge's mean {mean:?}"
        );
        assert!(p.point.distance(Point3::new(5.0, 0.0, 0.0)) < 1e-9);
    }
    // Away from the edge each facet keeps its own normal.
    let flat = surface.project(Point3::new(5.0, -1.0, 0.0)).unwrap();
    assert!((flat.normal.get() - Vec3::new(0.0, 0.0, 1.0)).length() < 1e-12);
    let slope = surface.project(Point3::new(5.0, 1.0, 1.0)).unwrap();
    assert!(
        (slope.normal.get() - Vec3::new(0.0, -1.0, 1.0) * std::f64::consts::FRAC_1_SQRT_2).length()
            < 1e-12
    );
}

/// A ball dropped into a symmetric V-groove rests on both flanks at once:
/// the two facets lift the tip to the same height, up to rounding. Which
/// flank gives the contact point (and so the contact feed, OP-13) must not
/// depend on which comes out a few ulps higher: moving either flank by one
/// unit in the last place must leave the contact where it was.
#[test]
fn req_gc_07_req_nfr_06_a_drop_tied_between_two_flanks_keeps_its_contact_under_rounding_noise() {
    let m = mesh(
        &[
            [0.0, 0.0, 0.0],
            [10.0, 0.0, 0.0],
            [5.0, -5.0, 5.0],
            [5.0, 5.0, 5.0],
        ],
        &[[0, 2, 1], [0, 1, 3]],
    );
    let profile = ball(3.0).profile().unwrap();
    let line = vertical(5.0, 0.0);
    let reference = MeshSurface::new(&m)
        .unwrap()
        .drop_tool(&profile, &line)
        .unwrap();
    // The ball's centre is 1.5 mm from each 45 degree flank: tip height 1.5 (sqrt 2 - 1).
    assert!((reference.tip.z - 1.5 * (std::f64::consts::SQRT_2 - 1.0)).abs() < 1e-9);
    assert!(reference.contact.y.abs() > 1.0, "{:?}", reference.contact);
    let left = |i: usize| i == 2;
    let right = |i: usize| i == 3;
    for (name, variant) in [
        ("left flank up", nudged(&m, true, left)),
        ("left flank down", nudged(&m, false, left)),
        ("right flank up", nudged(&m, true, right)),
        ("right flank down", nudged(&m, false, right)),
        ("both up", nudged(&m, true, |_| true)),
    ] {
        let got = MeshSurface::new(&variant)
            .unwrap()
            .drop_tool(&profile, &line)
            .unwrap();
        assert!(
            got.contact.distance(reference.contact) < 1e-9,
            "{name}: contact {:?} against {:?}",
            got.contact,
            reference.contact
        );
        assert!((got.tip.z - reference.tip.z).abs() < 1e-9);
    }
}

/// A ball resting on a mesh edge touches it where the tip height along the
/// edge peaks. The peak is flat, so a search that compares heights finds it
/// only to the square root of rounding (about 1e-8 of the edge). The contact
/// must be found to rounding precision: a one-ulp change of the tool line
/// moves it by less than 1e-12 mm.
#[test]
fn req_gc_07_req_nfr_06_an_edge_contact_moves_only_by_rounding_under_rounding_noise() {
    // One sloping triangle whose top edge, from (0, 0, 0) to (10, 0.4, 1),
    // is the highest the ball can touch from above its +y side.
    let m = mesh(
        &[[0.0, 0.0, 0.0], [10.0, 0.4, 1.0], [5.0, -6.0, -6.0]],
        &[[0, 2, 1]],
    );
    let surface = MeshSurface::new(&m).unwrap();
    let profile = ball(3.0).profile().unwrap();
    let reference = surface.drop_tool(&profile, &vertical(5.0, 0.9)).unwrap();
    // The contact is on the edge, not at a vertex.
    assert!(
        reference.contact.x > 0.5 && reference.contact.x < 9.5,
        "{:?}",
        reference.contact
    );
    let mut worst = 0.0_f64;
    for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, -1)] {
        let step = |v: f64, d: i32| match d {
            1 => v.next_up(),
            -1 => v.next_down(),
            _ => v,
        };
        let line = vertical(step(5.0, dx), step(0.9, dy));
        let got = surface.drop_tool(&profile, &line).unwrap();
        worst = worst.max(got.contact.distance(reference.contact));
    }
    assert!(
        worst < 1e-12,
        "the contact moved by {worst:e} mm for one ulp of the tool line"
    );
}

/// Reads a binary STL as a mesh, one vertex per corner.
fn read_binary_stl(path: &Path) -> TriangleMesh {
    let bytes = std::fs::read(path).unwrap();
    let count = u32::from_le_bytes(bytes[80..84].try_into().unwrap()) as usize;
    let mut vertices = Vec::with_capacity(3 * count);
    let mut triangles = Vec::with_capacity(count);
    for t in 0..count {
        let base = 84 + 50 * t + 12;
        for v in 0..3 {
            let at = base + 12 * v;
            let f = |k: usize| {
                f64::from(f32::from_le_bytes(
                    bytes[at + 4 * k..at + 4 * k + 4].try_into().unwrap(),
                ))
            };
            vertices.push(Point3::new(f(0), f(1), f(2)));
        }
        let i = (3 * t) as u32;
        triangles.push([i, i + 1, i + 2]);
    }
    TriangleMesh {
        vertices,
        triangles,
    }
}

fn blisk() -> TriangleMesh {
    read_binary_stl(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/stl/blisk.stl"))
}

fn helical_on(m: &TriangleMesh) -> GeneratedPath {
    let fixture = Fixture::new(MeshSurface::new(m).unwrap(), 21.0, &ball(3.0));
    let values: BTreeMap<String, ParameterValue> = [
        (names::STEPOVER, 2.0),
        (names::FEED, 480.0),
        (names::PLUNGE_FEED, 150.0),
        (parameter::MAX_ANGLE_STEP, 5.0),
    ]
    .into_iter()
    .map(|(k, x)| (k.to_owned(), ParameterValue::Number(x)))
    .collect();
    run(|ctx| generate(&HelicalFinishing, &fixture.input(), &values, ctx)).unwrap()
}

/// The owner's blisk (OI-319): helical finishing at a 5 degree step, as the
/// `blisk_helical_5` sample, on the mesh as read and on the mesh with every
/// coordinate one ulp up. The two toolpaths must have the same moves, each
/// with the same tip, A and contact length, so a platform whose arithmetic
/// differs by an ulp writes the same program.
#[test]
fn req_gc_07_req_nfr_06_the_blisk_helical_pass_is_the_same_under_one_ulp_of_mesh_noise() {
    let m = blisk();
    let reference = helical_on(&m);
    let noisy = helical_on(&nudged(&m, true, |_| true));
    assert_eq!(
        reference.group.segments.len(),
        noisy.group.segments.len(),
        "the noisy mesh gives a different number of moves"
    );
    for (i, (a, b)) in reference
        .group
        .segments
        .iter()
        .zip(&noisy.group.segments)
        .enumerate()
    {
        assert_eq!(a.kind, b.kind, "move {i}");
        assert!(
            a.end.tip.distance(b.end.tip) < 1e-9,
            "move {i}: tip {:?} against {:?}",
            a.end.tip,
            b.end.tip
        );
        assert!((a.end.a.0 - b.end.a.0).abs() < 1e-9, "move {i}: A");
        assert!(
            (a.contact_length.0 - b.contact_length.0).abs() < 1e-9,
            "move {i} to {:?}: contact length {} against {}",
            a.end.tip,
            a.contact_length.0,
            b.contact_length.0
        );
        assert!(
            (a.duration.0 - b.duration.0).abs() < 1e-9 * a.duration.0.max(1.0),
            "move {i}: duration"
        );
    }
}
