// SPDX-License-Identifier: AGPL-3.0-or-later
//! Sampling the exact curve under a B-rep edge (OP-08; decision record 0227,
//! OI-229).
#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::float_cmp,
    clippy::cast_precision_loss
)]

use std::f64::consts::{FRAC_1_SQRT_2, FRAC_PI_2, TAU};

use openchuck_path::edge_curve::{SAG, point, sample};
use openchuck_types::brep::{BSplineCurve, EdgeCurve, Frame};
use openchuck_types::geometry::{Point3, Vec3};

fn xy_frame() -> Frame {
    Frame {
        origin: Point3::new(1.0, 2.0, 3.0),
        x: Vec3::new(1.0, 0.0, 0.0),
        y: Vec3::new(0.0, 1.0, 0.0),
        z: Vec3::new(0.0, 0.0, 1.0),
    }
}

fn mid_sag(points: &[Point3], on: impl Fn(Point3) -> f64) -> f64 {
    points
        .iter()
        .zip(points.iter().skip(1))
        .map(|(a, b)| {
            let m = Point3::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0, (a.z + b.z) / 2.0);
            on(m).abs()
        })
        .fold(0.0, f64::max)
}

#[test]
fn req_op_08_exact_curve_a_line_is_its_two_ends() {
    let line = EdgeCurve::Line {
        origin: Point3::new(0.0, 0.0, 1.0),
        direction: Vec3::new(0.0, 1.0, 0.0),
        first: 2.0,
        last: 7.0,
    };
    assert_eq!(
        sample(&line).unwrap(),
        vec![Point3::new(0.0, 2.0, 1.0), Point3::new(0.0, 7.0, 1.0)]
    );
}

#[test]
fn req_op_08_exact_curve_a_circle_arc_lies_on_the_circle_within_the_sag_limit() {
    let circle = EdgeCurve::Circle {
        frame: xy_frame(),
        radius: 5.0,
        first: 0.0,
        last: TAU,
    };
    let points = sample(&circle).unwrap();
    let off = |p: Point3| (p.x - 1.0).hypot(p.y - 2.0) - 5.0;
    assert!(points.iter().all(|p| off(*p).abs() < 1e-12 && p.z == 3.0));
    assert!(mid_sag(&points, off) <= SAG + 1e-12);
    assert!(points.first().unwrap().distance(*points.last().unwrap()) < 1e-12);
}

#[test]
fn req_op_08_exact_curve_a_reversed_range_runs_backwards() {
    let circle = EdgeCurve::Circle {
        frame: xy_frame(),
        radius: 5.0,
        first: FRAC_PI_2,
        last: 0.0,
    };
    let points = sample(&circle).unwrap();
    assert!(points[0].distance(Point3::new(1.0, 7.0, 3.0)) < 1e-12);
    assert!(points.last().unwrap().distance(Point3::new(6.0, 2.0, 3.0)) < 1e-12);
}

#[test]
fn req_op_08_exact_curve_an_ellipse_satisfies_its_equation() {
    let ellipse = EdgeCurve::Ellipse {
        frame: xy_frame(),
        major: 8.0,
        minor: 3.0,
        first: 0.0,
        last: TAU,
    };
    let points = sample(&ellipse).unwrap();
    let off = |p: Point3| ((p.x - 1.0) / 8.0).powi(2) + ((p.y - 2.0) / 3.0).powi(2) - 1.0;
    assert!(points.iter().all(|p| off(*p).abs() < 1e-12));
    assert!(points.len() > 16);
}

/// A rational quadratic quarter circle of radius 4 about the origin.
fn quarter(first: f64, last: f64) -> EdgeCurve {
    EdgeCurve::BSpline(BSplineCurve {
        degree: 2,
        knots: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        poles: vec![
            Point3::new(4.0, 0.0, 0.0),
            Point3::new(4.0, 4.0, 0.0),
            Point3::new(0.0, 4.0, 0.0),
        ],
        weights: vec![1.0, FRAC_1_SQRT_2, 1.0],
        first,
        last,
    })
}

#[test]
fn req_op_08_exact_curve_a_rational_spline_quarter_circle_is_a_circle() {
    let points = sample(&quarter(0.0, 1.0)).unwrap();
    let off = |p: Point3| p.x.hypot(p.y) - 4.0;
    assert!(points.iter().all(|p| off(*p).abs() < 1e-12));
    assert!(mid_sag(&points, off) <= SAG + 1e-12);
    assert!(points[0].distance(Point3::new(4.0, 0.0, 0.0)) < 1e-12);
    assert!(points.last().unwrap().distance(Point3::new(0.0, 4.0, 0.0)) < 1e-12);
    // Part of the range, run backwards.
    let part = sample(&quarter(0.75, 0.25)).unwrap();
    assert!(part.iter().all(|p| off(*p).abs() < 1e-12));
    assert!(part[0].y > part.last().unwrap().y);
}

#[test]
fn req_op_08_exact_curve_a_curve_that_cannot_be_evaluated_gives_nothing() {
    let EdgeCurve::BSpline(mut spline) = quarter(0.0, 1.0) else {
        unreachable!()
    };
    spline.weights.pop();
    assert!(sample(&EdgeCurve::BSpline(spline.clone())).is_none());
    assert!(point(&EdgeCurve::BSpline(spline), 0.5).is_none());
    let broken = EdgeCurve::Circle {
        frame: xy_frame(),
        radius: f64::NAN,
        first: 0.0,
        last: 1.0,
    };
    assert!(sample(&broken).is_none());
}

#[test]
fn req_op_08_exact_curve_validation_refuses_a_damaged_curve() {
    let EdgeCurve::BSpline(good) = quarter(0.0, 1.0) else {
        unreachable!()
    };
    assert!(good.validate().is_ok());
    let mut unclamped = good.clone();
    unclamped.knots = vec![0.0, 0.5, 0.5, 1.0, 1.0, 1.0];
    assert!(unclamped.validate().is_err());
    let mut outside = good.clone();
    outside.last = 2.0;
    assert!(outside.validate().is_err());
    let mut zero_weight = good;
    zero_weight.weights[1] = 0.0;
    assert!(zero_weight.validate().is_err());
    let line = EdgeCurve::Line {
        origin: Point3::ORIGIN,
        direction: Vec3::new(0.0, 2.0, 0.0),
        first: 0.0,
        last: 1.0,
    };
    assert!(line.validate("edge").is_err());
}

/// A degree-1 spline whose one tall vertex sits in a very short knot span
/// that even steps and midpoints alone would step over (CodeRabbit finding
/// on pull request 317).
fn spike(first: f64, last: f64) -> EdgeCurve {
    EdgeCurve::BSpline(BSplineCurve {
        degree: 1,
        knots: vec![0.0, 0.0, 1.0 / 64.0, 1.0 / 32.0, 1.0, 1.0],
        poles: vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0 / 64.0, 1.0, 0.0),
            Point3::new(1.0 / 32.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
        ],
        weights: vec![1.0; 4],
        first,
        last,
    })
}

#[test]
fn req_op_08_exact_curve_a_spline_feature_in_a_short_knot_span_is_not_missed() {
    for (first, last) in [(0.0, 1.0), (1.0, 0.0)] {
        let points = sample(&spike(first, last)).unwrap();
        let top = points.iter().map(|p| p.y).fold(0.0, f64::max);
        assert!(
            (top - 1.0).abs() < 1e-12,
            "the tall vertex is sampled: {top}"
        );
        assert!(points[0].distance(point(&spike(first, last), first).unwrap()) < 1e-12);
        assert!(
            points
                .last()
                .unwrap()
                .distance(point(&spike(first, last), last).unwrap())
                < 1e-12
        );
    }
}

#[test]
fn req_op_08_exact_curve_a_chord_is_checked_beyond_its_midpoint() {
    // Degree 3, knots 0 x4, 1/16 x3, 1 x4: the first span is the Bezier cubic
    // of the first four poles. With y = 0, 32/9, -32/9, 0 its ends and middle
    // are on one chord while its quarter point is 1 mm off it (a CodeRabbit
    // finding on pull request 317).
    let w = 1.0 / 16.0;
    let mut poles: Vec<Point3> = [0.0, 32.0 / 9.0, -32.0 / 9.0, 0.0]
        .iter()
        .enumerate()
        .map(|(i, y)| Point3::new(w * i as f64 / 3.0, *y, 0.0))
        .collect();
    poles.extend((1..=3).map(|i| Point3::new(w + (1.0 - w) * f64::from(i) / 3.0, 0.0, 0.0)));
    let mut knots = vec![0.0; 4];
    knots.extend([w; 3]);
    knots.extend([1.0; 4]);
    let curve = EdgeCurve::BSpline(BSplineCurve {
        degree: 3,
        knots,
        weights: vec![1.0; poles.len()],
        poles,
        first: 0.0,
        last: 1.0,
    });
    let q = point(&curve, w / 4.0).unwrap();
    assert!(
        (q.y - 1.0).abs() < 1e-9,
        "the quarter point is 1 mm off: {}",
        q.y
    );
    let points = sample(&curve).unwrap();
    let nearest = points
        .windows(2)
        .map(|v| {
            let (a, b) = (v[0], v[1]);
            let ab = [b.x - a.x, b.y - a.y];
            let aq = [q.x - a.x, q.y - a.y];
            let len2 = ab[0] * ab[0] + ab[1] * ab[1];
            let s = if len2 > 0.0 {
                ((ab[0] * aq[0] + ab[1] * aq[1]) / len2).clamp(0.0, 1.0)
            } else {
                0.0
            };
            (a.x + s * ab[0] - q.x).hypot(a.y + s * ab[1] - q.y)
        })
        .fold(f64::MAX, f64::min);
    assert!(
        nearest <= SAG + 1e-9,
        "the quarter point is {nearest} from the chords"
    );
}
