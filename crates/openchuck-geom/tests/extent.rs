// SPDX-License-Identifier: AGPL-3.0-or-later
//! Part extent for setup checks (SET-06, decision record 0011).

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use openchuck_geom::part_extent;
use openchuck_testkit::parts::{Cylinder, RadialPart};
use openchuck_types::mesh::TriangleMesh;

#[test]
fn req_set_06_part_extent_measures_radius_about_the_rotary_axis() {
    let part = Cylinder {
        radius: 15.0,
        length: 60.0,
    };
    let (x0, x1) = part.x_range();
    let extent = part_extent(&part.mesh(4, 64)).unwrap();
    assert!((extent.radius.0 - 15.0).abs() < 1e-9, "{extent:?}");
    assert!((extent.bounds.min.x - x0).abs() < 1e-12 && (extent.bounds.max.x - x1).abs() < 1e-12);
    let empty = TriangleMesh {
        vertices: vec![],
        triangles: vec![],
    };
    assert_eq!(part_extent(&empty), None);
}
