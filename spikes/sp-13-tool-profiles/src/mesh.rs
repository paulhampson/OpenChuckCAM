// SPDX-License-Identifier: AGPL-3.0-or-later
//! A plain triangle mesh with an XY grid index for vertical drop queries.

pub type V3 = [f64; 3];

#[derive(Debug, Clone, Default)]
pub struct Mesh {
    pub verts: Vec<V3>,
    pub tris: Vec<[u32; 3]>,
}

impl Mesh {
    /// Rotates the mesh about the X axis by `deg` (right-handed), as the A axis does.
    pub fn rotated_x(&self, deg: f64) -> Mesh {
        let (s, c) = deg.to_radians().sin_cos();
        Mesh {
            verts: self
                .verts
                .iter()
                .map(|v| [v[0], c * v[1] - s * v[2], s * v[1] + c * v[2]])
                .collect(),
            tris: self.tris.clone(),
        }
    }

    pub fn tri(&self, i: usize) -> [V3; 3] {
        let t = self.tris[i];
        [
            self.verts[t[0] as usize],
            self.verts[t[1] as usize],
            self.verts[t[2] as usize],
        ]
    }
}

/// Uniform XY grid of triangle indices, by each triangle's XY bounding box.
#[derive(Debug)]
pub struct XyIndex {
    pub mesh: Mesh,
    x0: f64,
    y0: f64,
    cell: f64,
    nx: usize,
    ny: usize,
    cells: Vec<Vec<u32>>,
}

impl XyIndex {
    pub fn new(mesh: Mesh, cell: f64) -> Self {
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for v in &mesh.verts {
            x0 = x0.min(v[0]);
            y0 = y0.min(v[1]);
            x1 = x1.max(v[0]);
            y1 = y1.max(v[1]);
        }
        let nx = (((x1 - x0) / cell).ceil() as usize).max(1);
        let ny = (((y1 - y0) / cell).ceil() as usize).max(1);
        let mut cells = vec![Vec::new(); nx * ny];
        for i in 0..mesh.tris.len() {
            let t = mesh.tri(i);
            let bx0 = t.iter().map(|v| v[0]).fold(f64::MAX, f64::min);
            let bx1 = t.iter().map(|v| v[0]).fold(f64::MIN, f64::max);
            let by0 = t.iter().map(|v| v[1]).fold(f64::MAX, f64::min);
            let by1 = t.iter().map(|v| v[1]).fold(f64::MIN, f64::max);
            let (i0, i1) = Self::span(x0, cell, nx, bx0, bx1);
            let (j0, j1) = Self::span(y0, cell, ny, by0, by1);
            for j in j0..=j1 {
                for k in i0..=i1 {
                    cells[j * nx + k].push(i as u32);
                }
            }
        }
        Self {
            mesh,
            x0,
            y0,
            cell,
            nx,
            ny,
            cells,
        }
    }

    fn span(o: f64, cell: f64, n: usize, lo: f64, hi: f64) -> (usize, usize) {
        let a = ((lo - o) / cell).floor().max(0.0) as usize;
        let b = ((hi - o) / cell).floor().max(0.0) as usize;
        (a.min(n - 1), b.min(n - 1))
    }

    /// Triangles whose XY box may meet the square of half-width `r` about (x, y),
    /// sorted and without repeats, so the order never depends on the grid.
    pub fn query(&self, x: f64, y: f64, r: f64, out: &mut Vec<u32>) {
        out.clear();
        let (i0, i1) = Self::span(self.x0, self.cell, self.nx, x - r, x + r);
        let (j0, j1) = Self::span(self.y0, self.cell, self.ny, y - r, y + r);
        for j in j0..=j1 {
            for k in i0..=i1 {
                out.extend_from_slice(&self.cells[j * self.nx + k]);
            }
        }
        out.sort_unstable();
        out.dedup();
    }
}
