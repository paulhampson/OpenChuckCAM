// SPDX-License-Identifier: AGPL-3.0-or-later
//! Coverage is independent of target planes and protections (OP-24, OP-21).
#![allow(clippy::float_cmp)]
use openchuck_path::Solid;
use openchuck_path::facing::extents::FacingExtent;
use openchuck_path::facing::geometry::{FacingEnd, FacingPlane};
use openchuck_types::geometry::{Aabb, Point3, UnitVec3};
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::stock::Stock;
use openchuck_types::units::Millimetres;

fn part() -> TriangleMesh {
    TriangleMesh {
        vertices: vec![
            Point3::new(10., -2., -3.),
            Point3::new(50., 4., -3.),
            Point3::new(30., 1., 5.),
        ],
        triangles: vec![[0, 1, 2]],
    }
}
fn stock() -> Stock {
    Stock::Box {
        bounds: Aabb {
            min: Point3::new(0., -10., -8.),
            max: Point3::new(60., 10., 8.),
        },
    }
}

#[test]
fn req_op_24_all_planar_extents_and_margins_use_their_own_geometry() {
    let plane = FacingPlane::end(FacingEnd::Far);
    let part = part();
    let tab = Solid::cylinder(Point3::new(50., 7., 0.), UnitVec3::X, 10., 2.).unwrap();
    let whole = FacingExtent::default()
        .planar(plane, &stock(), &part, &[tab], 0.)
        .unwrap();
    assert_eq!(whole.min(), [-10., -8.]);
    assert_eq!(whole.max(), [10., 8.]);
    let p = FacingExtent::Part
        .planar(plane, &stock(), &part, &[tab], 0.)
        .unwrap();
    assert_eq!(p.min(), [-2., -3.]);
    assert_eq!(p.max(), [4., 5.]);
    let union = FacingExtent::PartPlusTabs
        .planar(plane, &stock(), &part, &[tab], 2.)
        .unwrap();
    assert_eq!(union.min(), [-4., -5.]);
    assert_eq!(union.max(), [11., 7.]);
}

#[test]
fn req_op_24_round_extent_is_axial_span_with_full_circumference_and_axial_margin() {
    let tab = Solid::cylinder(Point3::new(50., 7., 0.), UnitVec3::X, 10., 2.).unwrap();
    for (mode, expected) in [
        (FacingExtent::WholeStock, (-2., 62.)),
        (FacingExtent::Part, (8., 52.)),
        (FacingExtent::PartPlusTabs, (8., 62.)),
    ] {
        assert_eq!(mode.axial(&stock(), &part(), &[tab], 2.).unwrap(), expected);
    }
}

#[test]
fn req_op_24_cylindrical_stock_projection_and_offset_box_are_exact() {
    let cylinder = Stock::Cylinder {
        diameter: Millimetres(20.),
        length: Millimetres(60.),
        x_start: Millimetres(-5.),
    };
    let plane = FacingPlane::end(FacingEnd::Start);
    let b = FacingExtent::WholeStock
        .planar(plane, &cylinder, &part(), &[], 0.)
        .unwrap();
    assert_eq!(b.min(), [-10., -10.]);
    assert_eq!(b.max(), [10., 10.]);
    assert_eq!(
        FacingExtent::WholeStock
            .axial(&cylinder, &part(), &[], 0.)
            .unwrap(),
        (-5., 55.)
    );
    let offset = Stock::OffsetBoundingBox {
        offset: Millimetres(1.),
    };
    let b = FacingExtent::WholeStock
        .planar(plane, &offset, &part(), &[], 0.)
        .unwrap();
    assert_eq!(b.min(), [-3., -4.]);
    assert_eq!(b.max(), [5., 6.]);
}

#[test]
fn req_op_24_projection_ignores_unreferenced_vertices_and_refuses_empty_facets() {
    let mut p = part();
    p.vertices.push(Point3::new(900., 900., 900.));
    assert_eq!(
        FacingExtent::Part.axial(&stock(), &p, &[], 0.).unwrap(),
        (10., 50.)
    );
    p.triangles.clear();
    assert!(FacingExtent::Part.axial(&stock(), &p, &[], 0.).is_err());
}

#[test]
fn req_op_24_req_nfr_07_invalid_extents_refuse_instead_of_hiding_bad_input() {
    for margin in [-1., f64::NAN, f64::INFINITY] {
        assert!(
            FacingExtent::Part
                .axial(&stock(), &part(), &[], margin)
                .is_err()
        );
    }
    let mut p = part();
    p.triangles.push([0, 1, 999]);
    assert!(FacingExtent::Part.axial(&stock(), &p, &[], 0.).is_err());
    let invalid = Stock::Cylinder {
        diameter: Millimetres(0.),
        length: Millimetres(60.),
        x_start: Millimetres(0.),
    };
    assert!(
        FacingExtent::WholeStock
            .axial(&invalid, &part(), &[], 0.)
            .is_err()
    );
}
