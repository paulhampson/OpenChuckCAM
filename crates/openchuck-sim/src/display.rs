// SPDX-License-Identifier: AGPL-3.0-or-later
//! The simulated stock as a triangle mesh for the viewport (slice 7 item 6).
//! Plain data only: the viewport draws it (slice 9); nothing here draws.
//!
//! Each column interval becomes a box one cell across: its top and bottom
//! caps, and wall pieces where a neighbouring column has no material at that
//! height. The result is closed and blocky, with outward triangle winding;
//! smoothing is a viewport concern.

use openchuck_types::geometry::Point3;
use openchuck_types::mesh::TriangleMesh;

use crate::dexel::DexelStock;
use crate::span::subtract;

/// The stock's surface as a closed mesh. Quads do not share vertices.
pub fn stock_mesh(stock: &DexelStock) -> TriangleMesh {
    let (nx, ny) = stock.columns();
    let (cx, cy) = stock.cell();
    let mut mesh = TriangleMesh::default();
    let mut gaps: Vec<[f64; 2]> = Vec::new();
    for i in 0..nx {
        for j in 0..ny {
            let col = stock.column(i, j);
            if col.is_empty() {
                continue;
            }
            let (xc, yc) = stock.column_centre(i, j);
            let (x0, x1, y0, y1) = (xc - 0.5 * cx, xc + 0.5 * cx, yc - 0.5 * cy, yc + 0.5 * cy);
            for iv in col {
                let (lo, hi) = (iv[0], iv[1]);
                // Top, facing +Z, and bottom, facing -Z.
                quad(
                    &mut mesh,
                    [x0, y0, hi],
                    [x1, y0, hi],
                    [x1, y1, hi],
                    [x0, y1, hi],
                );
                quad(
                    &mut mesh,
                    [x0, y0, lo],
                    [x0, y1, lo],
                    [x1, y1, lo],
                    [x1, y0, lo],
                );
            }
            // Walls where the neighbour holds no material.
            let neighbours: [(Option<(usize, usize)>, u8); 4] = [
                (i.checked_sub(1).map(|i| (i, j)), 0),
                ((i + 1 < nx).then_some((i + 1, j)), 1),
                (j.checked_sub(1).map(|j| (i, j)), 2),
                ((j + 1 < ny).then_some((i, j + 1)), 3),
            ];
            for (n, side) in neighbours {
                gaps.clear();
                let other = n.map_or(&[][..], |(a, b)| stock.column(a, b));
                let spans: Vec<(f64, f64)> = other.iter().map(|iv| (iv[0], iv[1])).collect();
                subtract(col, &spans, &mut gaps);
                for g in &gaps {
                    let (lo, hi) = (g[0], g[1]);
                    match side {
                        0 => quad(
                            &mut mesh,
                            [x0, y0, lo],
                            [x0, y0, hi],
                            [x0, y1, hi],
                            [x0, y1, lo],
                        ),
                        1 => quad(
                            &mut mesh,
                            [x1, y0, lo],
                            [x1, y1, lo],
                            [x1, y1, hi],
                            [x1, y0, hi],
                        ),
                        2 => quad(
                            &mut mesh,
                            [x0, y0, lo],
                            [x1, y0, lo],
                            [x1, y0, hi],
                            [x0, y0, hi],
                        ),
                        _ => quad(
                            &mut mesh,
                            [x0, y1, lo],
                            [x0, y1, hi],
                            [x1, y1, hi],
                            [x1, y1, lo],
                        ),
                    }
                }
            }
        }
    }
    mesh
}

/// Adds the quad a-b-c-d (counter-clockwise seen from outside) as two triangles.
fn quad(mesh: &mut TriangleMesh, a: [f64; 3], b: [f64; 3], c: [f64; 3], d: [f64; 3]) {
    let base = mesh.vertices.len() as u32;
    for p in [a, b, c, d] {
        mesh.vertices.push(Point3::new(p[0], p[1], p[2]));
    }
    mesh.triangles.push([base, base + 1, base + 2]);
    mesh.triangles.push([base, base + 2, base + 3]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use openchuck_types::geometry::Aabb;
    use openchuck_types::stock::Stock;

    #[test]
    fn req_sim_03_a_box_stock_mesh_encloses_its_volume() {
        let stock = Stock::Box {
            bounds: Aabb {
                min: Point3::new(0.0, 0.0, 0.0),
                max: Point3::new(2.0, 1.0, 3.0),
            },
        };
        let d = DexelStock::from_stock(&stock, None, 0.5).unwrap();
        let mesh = stock_mesh(&d);
        // Signed volume by the divergence theorem.
        let mut v = 0.0;
        for t in 0..mesh.triangles.len() {
            let [a, b, c] = mesh.triangle(t).unwrap();
            v += a.to_vec().dot(b.to_vec().cross(c.to_vec())) / 6.0;
        }
        assert!((v - 6.0).abs() < 1e-9, "{v}");
    }
}
