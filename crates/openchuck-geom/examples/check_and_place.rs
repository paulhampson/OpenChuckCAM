// SPDX-License-Identifier: AGPL-3.0-or-later
//! Checks a mesh with a seeded defect, then orients a clean part to the
//! rotary axis (IMP-04, IMP-05). Run with
//! `cargo run -p openchuck-geom --example check_and_place`.

#![allow(clippy::print_stdout)]

use openchuck_geom::check::check_mesh;
use openchuck_geom::summary;
use openchuck_geom::transform::{AxisDirection, orient_to_rotary_axis, place};
use openchuck_types::geometry::Point3;
use openchuck_types::mesh::TriangleMesh;

fn main() -> Result<(), openchuck_types::Error> {
    // A tetrahedron standing on the XY plane, with its tip up the Z axis.
    let mut mesh = TriangleMesh {
        vertices: vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(20.0, 0.0, 0.0),
            Point3::new(0.0, 20.0, 0.0),
            Point3::new(5.0, 5.0, 40.0),
        ],
        triangles: vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]],
    };
    println!("clean: {}", check_mesh(&mesh, None)?.is_clean());

    // Wind one triangle the wrong way: the check names it.
    let good = std::mem::take(&mut mesh.triangles);
    mesh.triangles = vec![[0, 2, 1], [0, 3, 1], [0, 3, 2], [1, 2, 3]];
    for problem in check_mesh(&mesh, None)?.problems() {
        println!("problem: {problem}");
    }
    mesh.triangles = good;

    // Turn the part's Z axis onto the rotary axis (X) and centre it there.
    let along = AxisDirection::parse("+z")
        .ok_or_else(|| openchuck_types::Error::invalid("direction", "not an axis direction"))?;
    let steps = orient_to_rotary_axis(along);
    let placed = place(&mesh, &steps)?;
    if let Some(b) = summary(&placed).bounds {
        println!(
            "placed: X {} to {}, Y {} to {}, Z {} to {}",
            b.min.x, b.max.x, b.min.y, b.max.y, b.min.z, b.max.z
        );
    }
    Ok(())
}
