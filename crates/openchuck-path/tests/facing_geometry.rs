// SPDX-License-Identifier: AGPL-3.0-or-later
//! Independent geometric oracles for accepted decision 0252 (OP-24).
#![allow(clippy::float_cmp)] // Exact end-frame integer extrema/defaults are required.

use openchuck_path::Solid;
use openchuck_path::facing::geometry::{FacingEnd, FacingPlane, LayerDistribution};
use openchuck_path::rotary::RotaryMapping;
use openchuck_types::geometry::{Aabb, Point3, UnitVec3};
use openchuck_types::machine::{ARange, ASense, RotaryAxis};
use openchuck_types::units::Degrees;

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-10, "{a} != {b}");
}

#[test]
fn req_op_24_req_set_09_end_follows_exactly_one_held_end() {
    assert_eq!(
        FacingEnd::opposite_held(true, false).unwrap(),
        FacingEnd::Far
    );
    assert_eq!(
        FacingEnd::opposite_held(false, true).unwrap(),
        FacingEnd::Start
    );
    let neither = FacingEnd::opposite_held(false, false)
        .unwrap_err()
        .to_string();
    assert!(neither.contains("Define which end is held"), "{neither}");
    let both = FacingEnd::opposite_held(true, true)
        .unwrap_err()
        .to_string();
    assert!(both.contains("both ends"), "{both}");
}

#[test]
fn req_op_24_end_allowance_is_applied_outward_once() {
    let far = FacingEnd::Far.target((10.0, 50.0), 0.2).unwrap();
    let start = FacingEnd::Start.target((10.0, 50.0), 0.2).unwrap();
    close(far.x, 50.2);
    close(start.x, 9.8);
    close(far.length, 40.2);
    close(start.length, 40.2);
}

#[test]
fn req_op_24_side_layers_full_or_equal_land_on_exact_target() {
    assert_eq!(LayerDistribution::default(), LayerDistribution::Full);
    assert_eq!(
        LayerDistribution::Full.levels(12.0, 7.0, 2.0).unwrap(),
        vec![10.0, 8.0, 7.0]
    );
    let equal = LayerDistribution::Equal.levels(12.0, 7.0, 2.0).unwrap();
    assert_eq!(equal.len(), 3);
    for (actual, expected) in equal.iter().zip([31.0 / 3.0, 26.0 / 3.0, 7.0]) {
        close(*actual, expected);
    }
    assert_eq!(equal.last(), Some(&7.0));
    assert_eq!(
        LayerDistribution::Full.levels(12.0, 8.0, 2.0).unwrap(),
        vec![10.0, 8.0]
    );
    assert_eq!(
        LayerDistribution::Full.levels(12.0, 11.75, 2.0).unwrap(),
        vec![11.75]
    );
    assert!(
        LayerDistribution::Full
            .levels(7.0, 7.0, 2.0)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn req_op_24_req_nfr_07_geometry_refuses_invalid_or_unbounded_work() {
    for distribution in [LayerDistribution::Full, LayerDistribution::Equal] {
        for d in [0.0, -1.0, f64::NAN, f64::INFINITY, 1e-300] {
            assert!(distribution.levels(12.0, 7.0, d).is_err(), "{d}");
        }
        assert!(distribution.levels(7.0, 12.0, 2.0).is_err());
        assert!(distribution.levels(f64::INFINITY, 7.0, 2.0).is_err());
    }
    for s in [-0.1, f64::NAN, f64::INFINITY] {
        assert!(FacingEnd::Far.target((10.0, 50.0), s).is_err());
    }
    assert!(FacingEnd::Far.target((50.0, 10.0), 0.0).is_err());
    assert!(FacingEnd::Far.target((-f64::MAX, f64::MAX), 0.0).is_err());
}

#[test]
fn req_op_24_end_rectangle_and_zero_or_nonzero_margin_use_full_projection() {
    // L-shaped outline: the empty corner is deliberately included by its bounds.
    let l = [
        (0., 0.),
        (20., 0.),
        (20., 2.),
        (2., 2.),
        (2., 10.),
        (0., 10.),
    ]
    .map(|(y, z)| Point3::new(31.0, y, z));
    let plane = FacingPlane::end(FacingEnd::Far);
    let zero = plane.bounds(l, &[], 0.0).unwrap();
    assert_eq!(zero.min(), [0.0, 0.0]);
    assert_eq!(zero.max(), [20.0, 10.0]);
    let margin = plane.bounds(l, &[], 2.0).unwrap();
    assert_eq!(margin.min(), [-2.0, -2.0]);
    assert_eq!(margin.max(), [22.0, 12.0]);
    assert!(plane.bounds([], &[], 0.0).is_err());
    for bad in [-1.0, f64::NAN, f64::INFINITY] {
        assert!(plane.bounds(l, &[], bad).is_err());
    }
    assert!(
        plane
            .bounds([Point3::new(0.0, f64::NAN, 0.0)], &[], 0.0)
            .is_err()
    );
}

#[test]
fn req_op_24_arbitrary_projection_uses_part_not_its_aabb_corners() {
    // At A=-45 the transverse coordinate is (Y+Z)/sqrt(2). The empty
    // (20,10) AABB corner must not enlarge the projection to 30/sqrt(2).
    let rotary = RotaryMapping {
        axis: RotaryAxis::X,
        sense: ASense::Positive,
        range: ARange::Continuous,
    };
    let plane = FacingPlane::side(rotary, Degrees(-45.0)).unwrap();
    let vertices = [(0., 0.), (20., 0.), (0., 10.)].map(|(y, z)| Point3::new(4.0, y, z));
    let b = plane.bounds(vertices, &[], 0.0).unwrap();
    close(b.min()[0], 4.0);
    close(b.max()[0], 4.0);
    close(b.min()[1], 0.0);
    close(b.max()[1], 20.0 / 2.0_f64.sqrt());
}

#[test]
fn req_op_24_req_op_21_part_plus_tabs_uses_exact_analytic_projections() {
    let plane = FacingPlane::end(FacingEnd::Far);
    let part = [Point3::new(30., -1., -1.), Point3::new(35., 1., 1.)];
    let tab = Solid::frustum(Point3::new(35., 4., 0.), UnitVec3::X, 10., 2., 3.).unwrap();
    let b = plane.bounds(part, &[tab], 0.0).unwrap();
    assert_eq!(b.min(), [-1., -3.]);
    assert_eq!(b.max(), [7., 3.]);
    let box_tab = Solid::cuboid(Aabb {
        min: Point3::new(20., -8., -2.),
        max: Point3::new(25., -6., 2.),
    })
    .unwrap();
    let b = plane.bounds(part, &[tab, box_tab], 0.0).unwrap();
    assert_eq!(b.min(), [-8., -3.]);
    assert_eq!(b.max(), [7., 3.]);
    // Tilted disc support is r*sqrt(1-(axis dot direction)^2), not its sphere.
    let tilted = Solid::cylinder(
        Point3::ORIGIN,
        UnitVec3::new(openchuck_types::geometry::Vec3::new(1., 1., 0.)).unwrap(),
        2.,
        2.,
    )
    .unwrap();
    let b = plane.bounds([], &[tilted], 0.0).unwrap();
    close(b.min()[0], -2.0_f64.sqrt());
    close(b.max()[0], 2.0 * 2.0_f64.sqrt());
    close(b.min()[1], -2.0);
    close(b.max()[1], 2.0);
}

#[test]
fn req_op_24_side_normal_and_transverse_coordinates_follow_both_a_senses() {
    for (sense, sign) in [(ASense::Positive, 1.0), (ASense::Negative, -1.0)] {
        let rotary = RotaryMapping {
            axis: RotaryAxis::X,
            sense,
            range: ARange::Continuous,
        };
        let plane = FacingPlane::side(rotary, Degrees(90.0)).unwrap();
        let b = plane.bounds([Point3::new(3., 2., 5.)], &[], 0.0).unwrap();
        close(b.min()[0], 3.0);
        close(b.min()[1], -sign * 5.0);
        close(
            plane.support([Point3::new(3., 2., 5.)], &[]).unwrap(),
            sign * 2.0,
        );
    }
}
