// SPDX-License-Identifier: AGPL-3.0-or-later
//! Bounding hulls of a rational B-spline patch, for searches that must not
//! miss a part of the surface (lane STEPALL-X1, issue #348; decision 0211
//! items 3.3 and 3.4).
//!
//! The patch is split into its Bezier pieces by knot insertion (with the
//! face's parameter box as extra knots), and each piece is split in halves
//! by de Casteljau's algorithm into a tree of cells. Every split is exact,
//! in homogeneous coordinates. With positive weights a rational Bezier
//! patch lies in the convex hull of its control points, so each cell's
//! surface lies in the axis-aligned box of its control points and in the
//! box of them along the cell's own frame (two tangents and the normal):
//! the distance from a point to either box is a lower bound on its
//! distance to the cell. A search that skips cells whose bound is no
//! nearer than an answer it already has cannot miss a nearer point.

use openchuck_types::geometry::{Aabb, Point3, Vec3};

/// A homogeneous control point `(w x, w y, w z, w)`.
pub(super) type Hom = [f64; 4];

fn lerp(a: Hom, b: Hom, t: f64) -> Hom {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
        a[3] + (b[3] - a[3]) * t,
    ]
}

fn project(h: Hom) -> Point3 {
    let w = h[3];
    Point3::new(h[0] / w, h[1] / w, h[2] / w)
}

/// A rational Bezier patch over the parameter box `uv`
/// (`[u_min, u_max, v_min, v_max]`): `(du + 1) x (dv + 1)` points, u major.
#[derive(Debug, Clone)]
pub(super) struct Net {
    pub du: usize,
    pub dv: usize,
    pub pts: Vec<Hom>,
    pub uv: [f64; 4],
}

impl Net {
    fn at(&self, i: usize, j: usize) -> Hom {
        self.pts
            .get(i * (self.dv + 1) + j)
            .copied()
            .unwrap_or([f64::NAN; 4])
    }

    /// The two halves at the middle of u (`along_u`) or of v.
    pub fn split(&self, along_u: bool) -> (Net, Net) {
        let (du, dv) = (self.du, self.dv);
        let mut lo = self.clone();
        let mut hi = self.clone();
        let (n_line, n_other) = if along_u { (du, dv) } else { (dv, du) };
        let index = |k: usize, o: usize| {
            if along_u {
                k * (dv + 1) + o
            } else {
                o * (dv + 1) + k
            }
        };
        let mut work = vec![[0.0; 4]; n_line + 1];
        for o in 0..=n_other {
            for (k, w) in work.iter_mut().enumerate() {
                *w = self.pts.get(index(k, o)).copied().unwrap_or([f64::NAN; 4]);
            }
            // de Casteljau at 1/2: the first point of each level is the low
            // half's, the last the high half's (from its far end).
            for level in 0..=n_line {
                if let (Some(&first), Some(&last)) = (work.first(), work.get(n_line - level)) {
                    if let Some(a) = lo.pts.get_mut(index(level, o)) {
                        *a = first;
                    }
                    if let Some(b) = hi.pts.get_mut(index(n_line - level, o)) {
                        *b = last;
                    }
                }
                for k in 0..n_line - level {
                    if let (Some(&a), Some(&b)) = (work.get(k), work.get(k + 1))
                        && let Some(w) = work.get_mut(k)
                    {
                        *w = lerp(a, b, 0.5);
                    }
                }
            }
        }
        let [u0, u1, v0, v1] = self.uv;
        if along_u {
            let m = 0.5 * (u0 + u1);
            lo.uv = [u0, m, v0, v1];
            hi.uv = [m, u1, v0, v1];
        } else {
            let m = 0.5 * (v0 + v1);
            lo.uv = [u0, u1, v0, m];
            hi.uv = [u0, u1, m, v1];
        }
        (lo, hi)
    }

    /// Whether to split along u: the direction in which the control net is
    /// longer.
    pub fn longer_in_u(&self) -> bool {
        let (du, dv) = (self.du, self.dv);
        let len = |a: Hom, b: Hom| project(a).distance(project(b));
        let lu = len(self.at(0, 0), self.at(du, 0)) + len(self.at(0, dv), self.at(du, dv));
        let lv = len(self.at(0, 0), self.at(0, dv)) + len(self.at(du, 0), self.at(du, dv));
        lu >= lv
    }

    /// The boxes that hold the patch.
    pub fn bound(&self) -> Bound {
        let pts: Vec<Point3> = self.pts.iter().map(|h| project(*h)).collect();
        let mut min = Point3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
        let mut max = Point3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
        for p in &pts {
            min = Point3::new(min.x.min(p.x), min.y.min(p.y), min.z.min(p.z));
            max = Point3::new(max.x.max(p.x), max.y.max(p.y), max.z.max(p.z));
        }
        let pad = 1e-12 * (1.0 + min.to_vec().length().max(max.to_vec().length()));
        let aabb = Aabb {
            min: Point3::new(min.x - pad, min.y - pad, min.z - pad),
            max: Point3::new(max.x + pad, max.y + pad, max.z + pad),
        };
        let (du, dv) = (self.du, self.dv);
        let p = |i: usize, j: usize| project(self.at(i, j));
        let tu = (p(du, 0) - p(0, 0)) + (p(du, dv) - p(0, dv));
        let tv = (p(0, dv) - p(0, 0)) + (p(du, dv) - p(du, 0));
        let frame = (|| {
            let n = tu.cross(tv);
            let (ln, lu) = (n.length(), tu.length());
            if !(ln > 1e-300 && lu > 1e-300) || !ln.is_finite() {
                return None;
            }
            let n = n * (1.0 / ln);
            let a = tu * (1.0 / lu);
            let b = n.cross(a);
            Some([a, b, n])
        })();
        let oriented = frame.map(|axes| {
            let origin = pts.first().copied().unwrap_or(Point3::ORIGIN);
            let mut range = [(f64::INFINITY, f64::NEG_INFINITY); 3];
            for q in &pts {
                let d = *q - origin;
                for (r, a) in range.iter_mut().zip(axes) {
                    let x = d.dot(a);
                    *r = (r.0.min(x), r.1.max(x));
                }
            }
            for r in &mut range {
                // The axes are unit only to rounding: widen by it.
                let w = 1e-12 * (1.0 + r.0.abs().max(r.1.abs())) + pad;
                *r = (r.0 - w, r.1 + w);
            }
            Oriented {
                origin,
                axes,
                range,
            }
        });
        Bound { aabb, oriented }
    }

    /// The surface point at the middle of the parameter box.
    pub fn centre(&self) -> Point3 {
        let (du, dv) = (self.du, self.dv);
        let mut col = vec![[0.0; 4]; du + 1];
        let mut row = vec![[0.0; 4]; dv + 1];
        for (i, c) in col.iter_mut().enumerate() {
            for (j, r) in row.iter_mut().enumerate() {
                *r = self.at(i, j);
            }
            *c = casteljau(&mut row);
        }
        project(casteljau(&mut col))
    }
}

/// The point at 1/2 of a Bezier curve (destroys `pts`).
fn casteljau(pts: &mut [Hom]) -> Hom {
    let n = pts.len();
    for level in 1..n {
        for k in 0..n - level {
            if let (Some(&a), Some(&b)) = (pts.get(k), pts.get(k + 1))
                && let Some(w) = pts.get_mut(k)
            {
                *w = lerp(a, b, 0.5);
            }
        }
    }
    pts.first().copied().unwrap_or([f64::NAN; 4])
}

/// A box along a cell's own frame.
#[derive(Debug, Clone, Copy)]
pub(super) struct Oriented {
    origin: Point3,
    axes: [Vec3; 3],
    range: [(f64, f64); 3],
}

/// The boxes that hold a cell of the surface.
#[derive(Debug, Clone, Copy)]
pub(super) struct Bound {
    pub aabb: Aabb,
    oriented: Option<Oriented>,
}

impl Bound {
    /// The bound of a box alone.
    pub fn from_aabb(aabb: Aabb) -> Self {
        Self {
            aabb,
            oriented: None,
        }
    }

    /// A lower bound on the distance from `q` to any point of the cell.
    pub fn lower(&self, q: Point3) -> f64 {
        let b = &self.aabb;
        let dx = (b.min.x - q.x).max(q.x - b.max.x).max(0.0);
        let dy = (b.min.y - q.y).max(q.y - b.max.y).max(0.0);
        let dz = (b.min.z - q.z).max(q.z - b.max.z).max(0.0);
        let mut lower = (dx * dx + dy * dy + dz * dz).sqrt();
        if let Some(o) = &self.oriented {
            let d = q - o.origin;
            let mut s = 0.0;
            for (a, r) in o.axes.iter().zip(o.range) {
                let x = d.dot(*a);
                let e = (r.0 - x).max(x - r.1).max(0.0);
                s += e * e;
            }
            // The axes are orthonormal only to rounding.
            lower = lower.max(s.sqrt() * (1.0 - 1e-12));
        }
        lower
    }

    /// The distance from `q` to the cell's boxes (the larger of the two
    /// box distances, a lower bound on the distance to the cell) and its
    /// gradient with respect to `q`: zero inside both boxes. The distance
    /// to a box is convex, and so is the larger of two, which is what lets
    /// a drop bound a cell by a Newton search from above (the `drop`
    /// module).
    pub fn distance_gradient(&self, q: Point3) -> (f64, Vec3) {
        let b = &self.aabb;
        let ex = |lo: f64, hi: f64, x: f64| {
            if x < lo {
                x - lo
            } else if x > hi {
                x - hi
            } else {
                0.0
            }
        };
        let e = Vec3::new(
            ex(b.min.x, b.max.x, q.x),
            ex(b.min.y, b.max.y, q.y),
            ex(b.min.z, b.max.z, q.z),
        );
        let mut best = (e.length(), e);
        if let Some(o) = &self.oriented {
            let d = q - o.origin;
            let mut g = Vec3::ZERO;
            for (a, r) in o.axes.iter().zip(o.range) {
                g = g + *a * ex(r.0, r.1, d.dot(*a));
            }
            let l = g.length() * (1.0 - 1e-12);
            if l > best.0 {
                best = (l, g);
            }
        }
        let (d, g) = best;
        if d > 0.0 { (d, g * (1.0 / g.length())) } else { (0.0, Vec3::ZERO) }
    }

    /// The diagonal of the axis-aligned box: no two points of the cell are
    /// further apart.
    pub fn size(&self) -> f64 {
        (self.aabb.max - self.aabb.min).length()
    }

    fn union(a: &Aabb, b: &Aabb) -> Aabb {
        Aabb {
            min: Point3::new(
                a.min.x.min(b.min.x),
                a.min.y.min(b.min.y),
                a.min.z.min(b.min.z),
            ),
            max: Point3::new(
                a.max.x.max(b.max.x),
                a.max.y.max(b.max.y),
                a.max.z.max(b.max.z),
            ),
        }
    }
}

/// One node of the tree: a cell with its net (a leaf), or a box over two
/// children.
#[derive(Debug, Clone)]
pub(super) struct Node {
    pub bound: Bound,
    pub uv: [f64; 4],
    pub children: Option<(u32, u32)>,
    pub net: Option<u32>,
}

/// The tree of cells over a face's parameter box.
#[derive(Debug, Clone)]
pub(super) struct HullTree {
    pub nodes: Vec<Node>,
    pub nets: Vec<Net>,
    pub root: u32,
}

/// Inserts `t` into the clamped knot vector `k` of a degree-`p` curve with
/// control points `c` (Boehm), once.
fn insert(k: &mut Vec<f64>, p: usize, c: &mut Vec<Hom>, t: f64) {
    let n = c.len();
    // The span: k[s] <= t < k[s + 1], s in p..n.
    let mut s = p;
    while s + 1 < n && k.get(s + 1).is_some_and(|&x| x <= t) {
        s += 1;
    }
    let mult = k.iter().filter(|&&x| x.total_cmp(&t).is_eq()).count();
    let mut out = Vec::with_capacity(n + 1);
    for i in 0..=n {
        let q = if i + p <= s {
            c.get(i).copied()
        } else if i + mult > s {
            c.get(i.wrapping_sub(1)).copied()
        } else {
            let (ki, kp) = (
                k.get(i).copied().unwrap_or(0.0),
                k.get(i + p).copied().unwrap_or(0.0),
            );
            let a = if kp > ki { (t - ki) / (kp - ki) } else { 0.0 };
            match (c.get(i - 1), c.get(i)) {
                (Some(&x), Some(&y)) => Some(lerp(x, y, a)),
                _ => None,
            }
        };
        out.push(q.unwrap_or([f64::NAN; 4]));
    }
    k.insert(s + 1, t);
    *c = out;
}

/// The Bezier pieces of a clamped degree-`p` curve, with `cuts` as extra
/// knots: `(t0, t1, points)` for each non-empty piece.
fn pieces(knots: &[f64], p: usize, ctrl: &[Hom], cuts: &[f64]) -> Vec<(f64, f64, Vec<Hom>)> {
    let mut k = knots.to_vec();
    let mut c = ctrl.to_vec();
    let (Some(&lo), Some(&hi)) = (k.get(p), k.get(c.len())) else {
        return Vec::new();
    };
    let mut breaks: Vec<f64> = k
        .iter()
        .copied()
        .chain(cuts.iter().copied())
        .filter(|&x| x > lo && x < hi)
        .collect();
    breaks.sort_by(f64::total_cmp);
    breaks.dedup();
    for &t in &breaks {
        while k.iter().filter(|&&x| x.total_cmp(&t).is_eq()).count() < p {
            insert(&mut k, p, &mut c, t);
        }
    }
    // Each interior break now has multiplicity at least p, so on each span
    // the p + 1 basis functions that do not vanish are the Bernstein
    // polynomials over it, and their control points are the span's Bezier
    // piece. A span whose knots do not show that (ends not clamped exactly)
    // gives no pieces, and the face keeps its grid search.
    let mut edges = vec![lo];
    edges.extend(breaks);
    edges.push(hi);
    let mut out = Vec::new();
    for w in edges.windows(2) {
        let (Some(&t0), Some(&t1)) = (w.first(), w.get(1)) else {
            continue;
        };
        // The span: the last knot at t0.
        let Some(s) = k.iter().rposition(|&x| x.total_cmp(&t0).is_eq()) else {
            return Vec::new();
        };
        let bezier = s >= p
            && (s + 1 - p..=s).all(|i| k.get(i).is_some_and(|x| x.total_cmp(&t0).is_eq()))
            && (s + 1..=s + p).all(|i| k.get(i).is_some_and(|x| x.total_cmp(&t1).is_eq()));
        match s.checked_sub(p).and_then(|first| c.get(first..=s)) {
            Some(seg) if bezier => out.push((t0, t1, seg.to_vec())),
            _ => return Vec::new(),
        }
    }
    out
}

impl HullTree {
    /// The tree for a patch of degrees `(pu, pv)` with `nu x nv` poles (u
    /// major), over the parameter box `clip`. Cells are split until their box
    /// is no larger than `leaf`, at most `depth` times.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        (pu, pv): (usize, usize),
        (nu, nv): (usize, usize),
        (uk, vk): (&[f64], &[f64]),
        poles: &[Point3],
        weights: &[f64],
        clip: [f64; 4],
        leaf: f64,
        depth: usize,
    ) -> Option<Self> {
        let hom: Vec<Hom> = poles
            .iter()
            .zip(weights)
            .map(|(p, &w)| [p.x * w, p.y * w, p.z * w, w])
            .collect();
        if hom.len() != nu * nv || pu == 0 || pv == 0 {
            return None;
        }
        // Along u for each column j, then along v for each u piece.
        let mut columns = Vec::with_capacity(nv);
        for j in 0..nv {
            let col: Vec<Hom> = (0..nu)
                .map(|i| hom.get(i * nv + j).copied().unwrap_or([f64::NAN; 4]))
                .collect();
            columns.push(pieces(uk, pu, &col, &[clip[0], clip[1]]));
        }
        let n_upieces = columns.first().map_or(0, Vec::len);
        let mut nets = Vec::new();
        for a in 0..n_upieces {
            let Some((u0, u1, _)) = columns.first().and_then(|c| c.get(a)) else {
                continue;
            };
            let (u0, u1) = (*u0, *u1);
            if u1 <= clip[0] || u0 >= clip[1] {
                continue;
            }
            // For each of the pu + 1 rows of this u piece, split along v.
            let mut rows = Vec::with_capacity(pu + 1);
            for i in 0..=pu {
                let row: Vec<Hom> = columns
                    .iter()
                    .map(|c| {
                        c.get(a)
                            .and_then(|p| p.2.get(i))
                            .copied()
                            .unwrap_or([f64::NAN; 4])
                    })
                    .collect();
                rows.push(pieces(vk, pv, &row, &[clip[2], clip[3]]));
            }
            let n_vpieces = rows.first().map_or(0, Vec::len);
            for b in 0..n_vpieces {
                let Some((v0, v1, _)) = rows.first().and_then(|r| r.get(b)) else {
                    continue;
                };
                let (v0, v1) = (*v0, *v1);
                if v1 <= clip[2] || v0 >= clip[3] {
                    continue;
                }
                let mut pts = Vec::with_capacity((pu + 1) * (pv + 1));
                for r in &rows {
                    let piece = r.get(b)?;
                    pts.extend_from_slice(piece.2.get(..=pv)?);
                }
                nets.push(Net {
                    du: pu,
                    dv: pv,
                    pts,
                    uv: [u0, u1, v0, v1],
                });
            }
        }
        if nets
            .iter()
            .any(|n| n.pts.iter().flatten().any(|x| !x.is_finite()))
            || nets.is_empty()
        {
            return None;
        }
        let mut tree = Self {
            nodes: Vec::new(),
            nets: Vec::new(),
            root: 0,
        };
        let mut tops = Vec::new();
        for n in nets {
            tops.push(tree.subdivide(n, leaf, depth));
        }
        tree.root = tree.join(&mut tops);
        Some(tree)
    }

    fn push(&mut self, node: Node) -> u32 {
        self.nodes.push(node);
        u32::try_from(self.nodes.len() - 1).unwrap_or(u32::MAX)
    }

    fn subdivide(&mut self, net: Net, leaf: f64, depth: usize) -> u32 {
        let bound = net.bound();
        if depth == 0 || bound.size() <= leaf {
            let uv = net.uv;
            self.nets.push(net);
            let id = u32::try_from(self.nets.len() - 1).unwrap_or(u32::MAX);
            return self.push(Node {
                bound,
                uv,
                children: None,
                net: Some(id),
            });
        }
        let uv = net.uv;
        let (a, b) = net.split(net.longer_in_u());
        let (a, b) = (
            self.subdivide(a, leaf, depth - 1),
            self.subdivide(b, leaf, depth - 1),
        );
        self.parent(a, b, Some(bound), uv)
    }

    fn parent(&mut self, a: u32, b: u32, own: Option<Bound>, uv: [f64; 4]) -> u32 {
        let (Some(na), Some(nb)) = (self.nodes.get(a as usize), self.nodes.get(b as usize)) else {
            return a;
        };
        let bound = own.unwrap_or(Bound {
            aabb: Bound::union(&na.bound.aabb, &nb.bound.aabb),
            oriented: None,
        });
        self.push(Node {
            bound,
            uv,
            children: Some((a, b)),
            net: None,
        })
    }

    /// A balanced tree over the Bezier pieces, split at the median of their
    /// centres along the wider spread of u or v.
    fn join(&mut self, ids: &mut [u32]) -> u32 {
        match ids {
            [] => 0,
            [one] => *one,
            _ => {
                let centre = |n: &Node| (0.5 * (n.uv[0] + n.uv[1]), 0.5 * (n.uv[2] + n.uv[3]));
                let cs: Vec<(f64, f64)> = ids
                    .iter()
                    .filter_map(|&i| self.nodes.get(i as usize).map(centre))
                    .collect();
                let spread = |f: fn(&(f64, f64)) -> f64| {
                    let lo = cs.iter().map(f).fold(f64::INFINITY, f64::min);
                    let hi = cs.iter().map(f).fold(f64::NEG_INFINITY, f64::max);
                    hi - lo
                };
                let by_u = spread(|c| c.0) >= spread(|c| c.1);
                ids.sort_by(|&x, &y| {
                    let (cx, cy) = (
                        self.nodes.get(x as usize).map_or((0.0, 0.0), centre),
                        self.nodes.get(y as usize).map_or((0.0, 0.0), centre),
                    );
                    let (kx, ky) = if by_u { (cx.0, cy.0) } else { (cx.1, cy.1) };
                    kx.total_cmp(&ky).then(x.cmp(&y))
                });
                let mid = ids.len() / 2;
                let (lo, hi) = ids.split_at_mut(mid);
                let (a, b) = (self.join(lo), self.join(hi));
                let uv = match (self.nodes.get(a as usize), self.nodes.get(b as usize)) {
                    (Some(x), Some(y)) => [
                        x.uv[0].min(y.uv[0]),
                        x.uv[1].max(y.uv[1]),
                        x.uv[2].min(y.uv[2]),
                        x.uv[3].max(y.uv[3]),
                    ],
                    _ => [0.0; 4],
                };
                self.parent(a, b, None, uv)
            }
        }
    }
}
