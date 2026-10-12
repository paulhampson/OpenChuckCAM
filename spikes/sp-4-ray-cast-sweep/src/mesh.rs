// SPDX-License-Identifier: AGPL-3.0-or-later
//! Inscribed tessellation of the part into a parry3d-f64 `TriMesh` (vertices on the exact
//! surface, so the mesh lies inside it by at most the chord error).

use crate::Vector;
use crate::part::Part;
use parry3d_f64::shape::TriMesh;

/// Angular and axial step sizes giving a chord (sagitta) of about `tol` mm.
pub fn steps(part: &Part, tol: f64) -> (usize, usize) {
    let dth = 2.0 * (1.0 - tol / part.r_max()).acos().max(1e-6);
    let nth = (std::f64::consts::TAU / dth).ceil() as usize;
    // Axial curvature is r_xx; lobes add angular curvature already covered by `nth` only
    // roughly, so take the finer of the two for the lobe order.
    let rxx = part.a1.abs() * part.k1 * part.k1;
    let h = (8.0 * tol / rxx.max(1e-9)).sqrt();
    let nx = (part.length / h).ceil() as usize;
    (nx.max(8), nth.max(32 * part.m as usize))
}

pub fn build(part: &Part, nx: usize, nth: usize) -> TriMesh {
    let mut v = Vec::with_capacity((nx + 1) * nth + 2);
    for i in 0..=nx {
        let x = part.length * i as f64 / nx as f64;
        for j in 0..nth {
            let th = std::f64::consts::TAU * j as f64 / nth as f64;
            v.push(part.point(x, th));
        }
    }
    let c0 = v.len() as u32;
    v.push(Vector::new(0.0, 0.0, 0.0));
    v.push(Vector::new(part.length, 0.0, 0.0));
    let c1 = c0 + 1;
    let mut idx = Vec::with_capacity(2 * nx * nth + 2 * nth);
    let at = |i: usize, j: usize| (i * nth + (j % nth)) as u32;
    for i in 0..nx {
        for j in 0..nth {
            // Outward winding: x increasing, th increasing.
            idx.push([at(i, j), at(i + 1, j), at(i + 1, j + 1)]);
            idx.push([at(i, j), at(i + 1, j + 1), at(i, j + 1)]);
        }
    }
    for j in 0..nth {
        idx.push([c0, at(0, j), at(0, j + 1)]);
        idx.push([c1, at(nx, j + 1), at(nx, j)]);
    }
    TriMesh::new(v, idx).expect("valid mesh")
}
