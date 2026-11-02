// SPDX-License-Identifier: AGPL-3.0-or-later
//! Transforms and orientation to the rotary axis (IMP-05): results match
//! reference matrices, and a mirrored part keeps outward normals.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_cmp
)]

use openchuck_geom::check::check_mesh;
use openchuck_geom::summary;
use openchuck_geom::transform::{
    Affine3, AxisDirection, bounds, orient_to_rotary_axis, place, placement, transform_brep,
    transform_mesh, transform_normals,
};
use openchuck_testkit::parts::{Cone, RadialPart, standard_parts};
use openchuck_types::geometry::Point3;
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::transform::{Axis, TransformStep};
use openchuck_types::units::Degrees;
use proptest::prelude::*;

const EPS: f64 = 1e-12;

fn close(p: Point3, q: Point3) -> bool {
    p.distance(q) <= EPS * (1.0 + p.to_vec().length())
}

/// Applies a reference 3x3 matrix written out by hand, plus a translation.
fn reference(m: [[f64; 3]; 3], t: [f64; 3], p: Point3) -> Point3 {
    Point3::new(
        m[0][0] * p.x + m[0][1] * p.y + m[0][2] * p.z + t[0],
        m[1][0] * p.x + m[1][1] * p.y + m[1][2] * p.z + t[1],
        m[2][0] * p.x + m[2][1] * p.y + m[2][2] * p.z + t[2],
    )
}

fn sample_points() -> Vec<Point3> {
    vec![
        Point3::new(1.0, 0.0, 0.0),
        Point3::new(0.0, 1.0, 0.0),
        Point3::new(0.0, 0.0, 1.0),
        Point3::new(3.5, -2.25, 7.125),
        Point3::new(-40.0, 12.0, 0.5),
    ]
}

fn rotate(axis: Axis, angle: f64) -> TransformStep {
    TransformStep::Rotate {
        axis,
        angle: Degrees(angle),
    }
}

fn one_triangle(p: Point3) -> TriangleMesh {
    TriangleMesh {
        vertices: vec![p],
        triangles: vec![[0, 0, 0]],
    }
}

fn map_point(steps: &[TransformStep], p: Point3) -> Point3 {
    placement(&one_triangle(p), steps).unwrap().apply(p)
}

#[test]
fn req_imp_05_each_step_matches_its_reference_matrix() {
    let (s, c) = 30f64.to_radians().sin_cos();
    let cases: Vec<(TransformStep, [[f64; 3]; 3], [f64; 3])> = vec![
        (
            TransformStep::Translate {
                x: 1.5,
                y: -2.0,
                z: 3.25,
            },
            [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            [1.5, -2.0, 3.25],
        ),
        (
            rotate(Axis::X, 30.0),
            [[1.0, 0.0, 0.0], [0.0, c, -s], [0.0, s, c]],
            [0.0; 3],
        ),
        (
            rotate(Axis::Y, 30.0),
            [[c, 0.0, s], [0.0, 1.0, 0.0], [-s, 0.0, c]],
            [0.0; 3],
        ),
        (
            rotate(Axis::Z, 30.0),
            [[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]],
            [0.0; 3],
        ),
        (
            rotate(Axis::Z, 90.0),
            [[0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]],
            [0.0; 3],
        ),
        (
            rotate(Axis::X, -270.0),
            [[1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]],
            [0.0; 3],
        ),
        (
            TransformStep::Scale { factor: 25.4 },
            [[25.4, 0.0, 0.0], [0.0, 25.4, 0.0], [0.0, 0.0, 25.4]],
            [0.0; 3],
        ),
        (
            TransformStep::Mirror { axis: Axis::X },
            [[-1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            [0.0; 3],
        ),
        (
            TransformStep::Mirror { axis: Axis::Y },
            [[1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, 1.0]],
            [0.0; 3],
        ),
        (
            TransformStep::Mirror { axis: Axis::Z },
            [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, -1.0]],
            [0.0; 3],
        ),
    ];
    for (step, m, t) in cases {
        for p in sample_points() {
            let got = map_point(&[step], p);
            let want = reference(m, t, p);
            assert!(close(got, want), "{step}: {p:?} -> {got:?}, want {want:?}");
        }
    }
}

#[test]
fn req_imp_05_quarter_turns_are_exact() {
    let p = Point3::new(3.0, 4.0, 5.0);
    assert_eq!(
        map_point(&[rotate(Axis::Z, 90.0)], p),
        Point3::new(-4.0, 3.0, 5.0)
    );
    assert_eq!(
        map_point(&[rotate(Axis::Y, 180.0)], p),
        Point3::new(-3.0, 4.0, -5.0)
    );
    assert_eq!(
        map_point(&[rotate(Axis::X, -90.0)], p),
        Point3::new(3.0, 5.0, -4.0)
    );
    assert_eq!(map_point(&[rotate(Axis::X, 720.0)], p), p);
}

#[test]
fn req_imp_05_steps_apply_first_to_last() {
    let p = Point3::new(1.0, 0.0, 0.0);
    let translate = TransformStep::Translate {
        x: 10.0,
        y: 0.0,
        z: 0.0,
    };
    let turn = rotate(Axis::Z, 90.0);
    // Translate, then turn: (11, 0, 0) turns to (0, 11, 0).
    assert_eq!(
        map_point(&[translate, turn], p),
        Point3::new(0.0, 11.0, 0.0)
    );
    // Turn, then translate: (0, 1, 0) moves to (10, 1, 0).
    assert_eq!(
        map_point(&[turn, translate], p),
        Point3::new(10.0, 1.0, 0.0)
    );
}

#[test]
fn req_imp_05_mirror_keeps_normals_outward() {
    for part in standard_parts() {
        let mesh = part.mesh(24, 32);
        let volume = check_mesh(&mesh, None).unwrap().volume.unwrap();
        for axis in [Axis::X, Axis::Y, Axis::Z] {
            let map = placement(&mesh, &[TransformStep::Mirror { axis }]).unwrap();
            assert!(map.determinant() < 0.0);
            let mirrored = transform_mesh(&mesh, &map);
            let report = check_mesh(&mirrored, None).unwrap();
            assert!(
                report.is_clean(),
                "{} mirrored in {axis}: {:?}",
                part.name(),
                report.problems()
            );
            assert!((report.volume.unwrap() - volume).abs() < 1e-9 * volume);
            // Without the winding correction the mirrored part would be inside out.
            let unfixed = TriangleMesh {
                vertices: mirrored.vertices.clone(),
                triangles: mesh.triangles.clone(),
            };
            assert_eq!(check_mesh(&unfixed, None).unwrap().inside_out_shells, 1);
        }
        // Two mirrors are a rotation: the winding is kept.
        let twice = place(
            &mesh,
            &[
                TransformStep::Mirror { axis: Axis::Y },
                TransformStep::Mirror { axis: Axis::Z },
            ],
        )
        .unwrap();
        assert_eq!(twice.triangles, mesh.triangles);
        assert!(check_mesh(&twice, None).unwrap().is_clean());
    }
}

#[test]
fn req_imp_05_orient_to_rotary_axis() {
    // A cone made along X, then turned so its axis lies along -Y and moved off
    // the axis, as a file might hold it.
    let cone = Cone {
        start_radius: 20.0,
        end_radius: 8.0,
        length: 50.0,
    }
    .mesh(10, 36);
    let as_drawn = place(
        &cone,
        &[
            rotate(Axis::Z, -90.0),
            TransformStep::Translate {
                x: 7.0,
                y: 3.0,
                z: 40.0,
            },
        ],
    )
    .unwrap();
    let b = bounds(&as_drawn, &Affine3::IDENTITY).unwrap();
    assert!((b.max.y - b.min.y - 50.0).abs() < 1e-9, "{b:?}");

    let steps = orient_to_rotary_axis(AxisDirection::parse("-y").unwrap());
    assert_eq!(steps.last(), Some(&TransformStep::CentreOnRotaryAxis));
    let oriented = place(&as_drawn, &steps).unwrap();
    let b = bounds(&oriented, &Affine3::IDENTITY).unwrap();
    // Its axis is the rotary axis again: 50 mm long along X, centred on Y and Z.
    assert!((b.max.x - b.min.x - 50.0).abs() < 1e-9, "{b:?}");
    assert!(
        (b.min.y + b.max.y).abs() < 1e-9 && (b.min.z + b.max.z).abs() < 1e-9,
        "{b:?}"
    );
    // The wide end stays at the start of the axis: the first ring is at min X.
    let first = oriented.vertices[0];
    assert!((first.x - b.min.x).abs() < 1e-9, "{first:?} {b:?}");
    assert!(check_mesh(&oriented, None).unwrap().is_clean());

    // Every direction maps onto +X.
    for text in ["+x", "-x", "y", "-y", "+Z", "-z"] {
        let along = AxisDirection::parse(text).unwrap();
        let unit = match along.axis {
            Axis::X => Point3::new(1.0, 0.0, 0.0),
            Axis::Y => Point3::new(0.0, 1.0, 0.0),
            Axis::Z => Point3::new(0.0, 0.0, 1.0),
        };
        let unit = if along.negative {
            Point3::ORIGIN + -unit.to_vec()
        } else {
            unit
        };
        let rotation: Vec<_> = orient_to_rotary_axis(along)
            .into_iter()
            .filter(|s| *s != TransformStep::CentreOnRotaryAxis)
            .collect();
        assert_eq!(
            map_point(&rotation, unit),
            Point3::new(1.0, 0.0, 0.0),
            "{text}"
        );
    }
    assert_eq!(AxisDirection::parse("w"), None);
}

#[test]
fn req_imp_05_centre_keeps_the_position_along_the_axis() {
    let cube = standard_parts().remove(0).mesh(4, 8);
    let steps = [
        TransformStep::Translate {
            x: 5.0,
            y: 7.0,
            z: -9.0,
        },
        TransformStep::CentreOnRotaryAxis,
    ];
    let before = bounds(&place(&cube, &steps[..1]).unwrap(), &Affine3::IDENTITY).unwrap();
    let after = bounds(&place(&cube, &steps).unwrap(), &Affine3::IDENTITY).unwrap();
    assert_eq!((after.min.x, after.max.x), (before.min.x, before.max.x));
    assert!((after.min.y + after.max.y).abs() < 1e-12);
    assert!((after.min.z + after.max.z).abs() < 1e-12);
    // An empty mesh has nothing to centre.
    assert_eq!(
        placement(
            &TriangleMesh::default(),
            &[TransformStep::CentreOnRotaryAxis]
        )
        .unwrap(),
        Affine3::IDENTITY
    );
}

#[test]
fn req_imp_05_nfr_07_invalid_steps_are_refused() {
    let mesh = one_triangle(Point3::ORIGIN);
    for (step, field) in [
        (TransformStep::Scale { factor: 0.0 }, "transform[0].factor"),
        (TransformStep::Scale { factor: -2.0 }, "transform[0].factor"),
        (
            TransformStep::Translate {
                x: f64::NAN,
                y: 0.0,
                z: 0.0,
            },
            "transform[0].x",
        ),
        (rotate(Axis::Z, f64::INFINITY), "transform[0].angle"),
    ] {
        let err = placement(&mesh, &[step]).unwrap_err();
        assert!(err.to_string().contains(field), "{err}");
    }
}

#[test]
fn summary_counts_and_measures() {
    let mesh = standard_parts().remove(0).mesh(2, 4);
    let s = summary(&mesh);
    assert_eq!(s.triangles, mesh.triangles.len());
    let b = s.bounds.unwrap();
    assert_eq!((b.min.x, b.max.x), (0.0, 60.0));
    assert!(s.area > 0.0);
}

fn arb_step() -> impl Strategy<Value = TransformStep> {
    let axis = prop_oneof![Just(Axis::X), Just(Axis::Y), Just(Axis::Z)];
    prop_oneof![
        (-50.0f64..50.0, -50.0f64..50.0, -50.0f64..50.0)
            .prop_map(|(x, y, z)| TransformStep::Translate { x, y, z }),
        (axis.clone(), -400.0f64..400.0).prop_map(|(axis, a)| TransformStep::Rotate {
            axis,
            angle: Degrees(a)
        }),
        (0.1f64..10.0).prop_map(|factor| TransformStep::Scale { factor }),
        axis.prop_map(|axis| TransformStep::Mirror { axis }),
        Just(TransformStep::CentreOnRotaryAxis),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Any placement keeps a clean closed part clean, with outward normals
    /// (mirrors included) and the volume scaled by the cube of the scales.
    #[test]
    fn req_imp_05_any_placement_keeps_a_clean_part_clean(steps in proptest::collection::vec(arb_step(), 0..6)) {
        let mesh = standard_parts().remove(2).mesh(12, 16);
        let volume = check_mesh(&mesh, None).unwrap().volume.unwrap();
        let scale: f64 = steps
            .iter()
            .map(|s| match s { TransformStep::Scale { factor } => *factor, _ => 1.0 })
            .product();
        let placed = place(&mesh, &steps).unwrap();
        let report = check_mesh(&placed, None).unwrap();
        prop_assert!(report.is_clean(), "{:?}", report.problems());
        let expected = volume * scale.powi(3);
        prop_assert!((report.volume.unwrap() - expected).abs() <= 1e-9 * expected);
        // The same steps give the same result every time (NFR-06).
        prop_assert_eq!(placed, place(&mesh, &steps).unwrap());
    }

    /// The combined map equals applying the steps one at a time.
    #[test]
    fn req_imp_05_combined_map_matches_step_by_step(steps in proptest::collection::vec(arb_step(), 0..6)) {
        let mesh = standard_parts().remove(1).mesh(6, 8);
        let combined = place(&mesh, &steps).unwrap();
        let mut stepwise = mesh.clone();
        for step in &steps {
            stepwise = place(&stepwise, std::slice::from_ref(step)).unwrap();
        }
        prop_assert_eq!(&combined.triangles, &stepwise.triangles);
        for (a, b) in combined.vertices.iter().zip(&stepwise.vertices) {
            prop_assert!(a.distance(*b) <= 1e-9 * (1.0 + a.to_vec().length()), "{a:?} {b:?}");
        }
    }
}

#[test]
fn req_imp_05_stated_normals_follow_the_placement() {
    let mesh = standard_parts().remove(0).mesh(6, 12);
    let normals: Vec<_> = (0..mesh.triangles.len())
        .map(|t| {
            let [a, b, c] = mesh.triangle(t).unwrap();
            (b - a).cross(c - a)
        })
        .collect();
    let steps = [
        rotate(Axis::Y, 37.0),
        TransformStep::Mirror { axis: Axis::Z },
        TransformStep::Scale { factor: 2.0 },
        TransformStep::CentreOnRotaryAxis,
    ];
    let map = placement(&mesh, &steps).unwrap();
    let placed = transform_mesh(&mesh, &map);
    let report = check_mesh(&placed, Some(&transform_normals(&normals, &map))).unwrap();
    assert!(report.is_clean(), "{:?}", report.problems());
    // The file's normals left unmapped would disagree with the mirrored mesh.
    assert!(
        !check_mesh(&placed, Some(&normals))
            .unwrap()
            .normal_mismatches
            .is_empty()
    );
}

// ---- the exact curve of a B-rep edge (decision 0227) -------------------------

fn curved_model() -> openchuck_types::brep::BrepModel {
    use openchuck_types::brep::{BSplineCurve, BrepEdge, BrepModel, EdgeCurve, EdgeId, Frame};
    use openchuck_types::geometry::Vec3;
    let frame = Frame {
        origin: Point3::new(1.0, 2.0, 3.0),
        x: Vec3::new(1.0, 0.0, 0.0),
        y: Vec3::new(0.0, 1.0, 0.0),
        z: Vec3::new(0.0, 0.0, 1.0),
    };
    let edge = |id: u32, curve: EdgeCurve| BrepEdge {
        id: EdgeId(id),
        faces: vec![],
        vertices: None,
        polyline: vec![Point3::new(1.0, 0.0, 0.0), Point3::new(2.0, 0.0, 0.0)],
        curve: Some(curve),
    };
    BrepModel {
        edges: vec![
            edge(
                0,
                EdgeCurve::Circle {
                    frame,
                    radius: 5.0,
                    first: 0.0,
                    last: 1.0,
                },
            ),
            edge(
                1,
                EdgeCurve::Line {
                    origin: Point3::new(1.0, 1.0, 1.0),
                    direction: Vec3::new(0.0, 1.0, 0.0),
                    first: 2.0,
                    last: 4.0,
                },
            ),
            edge(
                2,
                EdgeCurve::BSpline(BSplineCurve {
                    degree: 1,
                    knots: vec![0.0, 0.0, 1.0, 1.0],
                    poles: vec![Point3::new(0.0, 0.0, 0.0), Point3::new(1.0, 1.0, 1.0)],
                    weights: vec![1.0, 1.0],
                    first: 0.0,
                    last: 1.0,
                }),
            ),
        ],
        ..BrepModel::default()
    }
}

#[test]
fn req_imp_05_op_08_a_placed_edge_curve_is_the_curve_placed() {
    use openchuck_types::brep::EdgeCurve;
    let steps = [
        TransformStep::Scale { factor: 2.0 },
        TransformStep::Rotate {
            axis: Axis::Z,
            angle: Degrees(90.0),
        },
        TransformStep::Mirror { axis: Axis::X },
        TransformStep::Translate {
            x: 1.0,
            y: -2.0,
            z: 3.0,
        },
    ];
    let map = placement(&TriangleMesh::default(), &steps).unwrap();
    let placed = transform_brep(&curved_model(), &map).unwrap();
    let close = |a: Point3, b: Point3| a.distance(b) < 1e-12;

    // A circle: the point at the same angle is the original point, placed.
    let (EdgeCurve::Circle { frame: before, .. }, EdgeCurve::Circle { frame, radius, .. }) = (
        curved_model().edges[0].curve.clone().unwrap(),
        placed.edges[0].curve.clone().unwrap(),
    ) else {
        panic!("a circle stays a circle");
    };
    assert!((radius - 10.0).abs() < 1e-12, "the radius scales");
    let at = |f: &openchuck_types::brep::Frame, r: f64, t: f64| {
        Point3::new(
            f.origin.x + r * (t.cos() * f.x.x + t.sin() * f.y.x),
            f.origin.y + r * (t.cos() * f.x.y + t.sin() * f.y.y),
            f.origin.z + r * (t.cos() * f.x.z + t.sin() * f.y.z),
        )
    };
    assert!(close(
        map.apply(at(&before, 5.0, 0.7)),
        at(&frame, 10.0, 0.7)
    ));

    // A line: its parameter is a length, so it scales with the model.
    let EdgeCurve::Line {
        origin,
        direction,
        first,
        last,
    } = placed.edges[1].curve.clone().unwrap()
    else {
        panic!("a line stays a line");
    };
    assert!((first - 4.0).abs() < 1e-12 && (last - 8.0).abs() < 1e-12);
    let along = |t: f64| {
        Point3::new(
            origin.x + t * direction.x,
            origin.y + t * direction.y,
            origin.z + t * direction.z,
        )
    };
    assert!(close(
        map.apply(Point3::new(1.0, 1.0 + 3.0, 1.0)),
        along(6.0)
    ));

    // A spline: poles move, parameters stay.
    let EdgeCurve::BSpline(spline) = placed.edges[2].curve.clone().unwrap() else {
        panic!("a spline stays a spline");
    };
    assert!(close(
        spline.poles[1],
        map.apply(Point3::new(1.0, 1.0, 1.0))
    ));
    assert_eq!((spline.first, spline.last), (0.0, 1.0));
}
