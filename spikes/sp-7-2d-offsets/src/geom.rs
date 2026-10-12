// SPDX-License-Identifier: AGPL-3.0-or-later
//! Regions in the unrolled plane (x along the axis, s = R * theta around it, both mm), and an
//! independent oracle: exact distance to a boundary that may hold arcs, and membership by
//! flattening. Nothing here calls either library under test except to build `Polyline` values.

use cavalier_contours::polyline::{PlineSource, PlineSourceMut, Polyline};

pub type Pt = [f64; 2];

/// One closed loop: vertices (x, y, bulge) as cavalier_contours stores them.
pub type Loop = Vec<[f64; 3]>;

/// A filled region: an outer boundary and holes. Orientation is fixed by [`Region::normalise`]
/// (outer counter-clockwise, holes clockwise).
#[derive(Clone, Debug)]
pub struct Region {
    pub outer: Loop,
    pub holes: Vec<Loop>,
}

pub fn signed_area(l: &Loop) -> f64 {
    // exact for arcs: shoelace on the chords plus circular segments.
    let n = l.len();
    let mut a = 0.0;
    for i in 0..n {
        let p = l[i];
        let q = l[(i + 1) % n];
        a += 0.5 * (p[0] * q[1] - q[0] * p[1]);
        let b = p[2];
        if b != 0.0 {
            let d = ((q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2)).sqrt();
            let theta = 4.0 * b.atan();
            let r = d * (1.0 + b * b) / (4.0 * b.abs());
            // segment area, signed by bulge direction
            a += 0.5 * r * r * (theta - theta.sin());
        }
    }
    a
}

pub fn reverse(l: &Loop) -> Loop {
    // vertex i keeps its position; the bulge of the segment i -> i+1 moves to the reversed
    // segment and flips sign.
    let n = l.len();
    let mut out = Vec::with_capacity(n);
    for k in 0..n {
        let i = n - 1 - k;
        let prev = (i + n - 1) % n; // segment prev -> i becomes i -> prev
        out.push([l[i][0], l[i][1], -l[prev][2]]);
    }
    out
}

impl Region {
    pub fn new(outer: Loop, holes: Vec<Loop>) -> Self {
        Region { outer, holes }.normalise()
    }

    pub fn normalise(mut self) -> Self {
        if signed_area(&self.outer) < 0.0 {
            self.outer = reverse(&self.outer);
        }
        for h in &mut self.holes {
            if signed_area(h) > 0.0 {
                *h = reverse(h);
            }
        }
        self
    }

    pub fn loops(&self) -> impl Iterator<Item = &Loop> {
        std::iter::once(&self.outer).chain(self.holes.iter())
    }

    pub fn area(&self) -> f64 {
        signed_area(&self.outer) + self.holes.iter().map(signed_area).sum::<f64>()
    }

    pub fn bounds(&self) -> [f64; 4] {
        let mut b = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
        for l in self.loops() {
            for v in l {
                b[0] = b[0].min(v[0]);
                b[1] = b[1].min(v[1]);
                b[2] = b[2].max(v[0]);
                b[3] = b[3].max(v[1]);
            }
        }
        b
    }

    /// Rotated by `ang` radians about (cx, cy). Bulges are unchanged by a rotation.
    pub fn rotated(&self, cx: f64, cy: f64, ang: f64) -> Region {
        let (sn, cs) = ang.sin_cos();
        let t = |l: &Loop| {
            l.iter()
                .map(|v| [cx + (v[0] - cx) * cs - (v[1] - cy) * sn, cy + (v[0] - cx) * sn + (v[1] - cy) * cs, v[2]])
                .collect::<Loop>()
        };
        Region { outer: t(&self.outer), holes: self.holes.iter().map(t).collect() }
    }

    pub fn translated(&self, dx: f64, dy: f64) -> Region {
        let t = |l: &Loop| l.iter().map(|v| [v[0] + dx, v[1] + dy, v[2]]).collect::<Loop>();
        Region { outer: t(&self.outer), holes: self.holes.iter().map(t).collect() }
    }
}

pub fn to_pline(l: &Loop) -> Polyline<f64> {
    let mut p = Polyline::new_closed();
    for v in l {
        p.add(v[0], v[1], v[2]);
    }
    p
}

pub fn from_pline(p: &Polyline<f64>) -> Loop {
    p.iter_vertexes().map(|v| [v.x, v.y, v.bulge]).collect()
}

// ---- constructors ----------------------------------------------------------------------

pub fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Loop {
    vec![[x0, y0, 0.0], [x1, y0, 0.0], [x1, y1, 0.0], [x0, y1, 0.0]]
}

/// A circle as two half-arcs (bulge 1), counter-clockwise.
pub fn circle(cx: f64, cy: f64, r: f64) -> Loop {
    vec![[cx - r, cy, 1.0], [cx + r, cy, 1.0]]
}

/// A rectangle with all four corners rounded by `r`.
pub fn rounded_rect(x0: f64, y0: f64, x1: f64, y1: f64, r: f64) -> Loop {
    let b = (std::f64::consts::PI / 8.0).tan(); // quarter turn: tan(90 degrees / 4)
    vec![
        [x0 + r, y0, 0.0],
        [x1 - r, y0, b],
        [x1, y0 + r, 0.0],
        [x1, y1 - r, b],
        [x1 - r, y1, 0.0],
        [x0 + r, y1, b],
        [x0, y1 - r, 0.0],
        [x0, y0 + r, b],
    ]
}

/// A slot (stadium): a rectangle with semicircular ends, axis along x.
pub fn slot(x0: f64, x1: f64, cy: f64, half_w: f64) -> Loop {
    vec![[x0, cy - half_w, 0.0], [x1, cy - half_w, 1.0], [x1, cy + half_w, 0.0], [x0, cy + half_w, 1.0]]
}

pub fn polygon(pts: &[Pt]) -> Loop {
    pts.iter().map(|p| [p[0], p[1], 0.0]).collect()
}

/// A regular star with `n` points, outer radius `ro`, inner radius `ri`.
pub fn star(cx: f64, cy: f64, n: usize, ro: f64, ri: f64, phase: f64) -> Loop {
    let mut v = Vec::new();
    for i in 0..2 * n {
        let r = if i % 2 == 0 { ro } else { ri };
        let a = phase + std::f64::consts::PI * i as f64 / n as f64;
        v.push([cx + r * a.cos(), cy + r * a.sin(), 0.0]);
    }
    v
}

// ---- oracle ----------------------------------------------------------------------------

fn dist_seg(p: Pt, a: Pt, b: Pt) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let l2 = dx * dx + dy * dy;
    let t = if l2 == 0.0 { 0.0 } else { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2).clamp(0.0, 1.0) };
    ((p[0] - a[0] - t * dx).powi(2) + (p[1] - a[1] - t * dy).powi(2)).sqrt()
}

/// Centre, radius, start angle and signed sweep of the arc a -> b with bulge `bg`.
pub fn arc_params(a: Pt, b: Pt, bg: f64) -> (Pt, f64, f64, f64) {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let d = (dx * dx + dy * dy).sqrt();
    let h = 0.5 * d * (1.0 - bg * bg) / (2.0 * bg);
    let (lx, ly) = (-dy / d, dx / d);
    let c = [0.5 * (a[0] + b[0]) + lx * h, 0.5 * (a[1] + b[1]) + ly * h];
    let r = (d * (1.0 + bg * bg) / (4.0 * bg.abs())).abs();
    let a0 = (a[1] - c[1]).atan2(a[0] - c[0]);
    (c, r, a0, 4.0 * bg.atan())
}

fn dist_arc(p: Pt, a: Pt, b: Pt, bg: f64) -> f64 {
    let (c, r, a0, sweep) = arc_params(a, b, bg);
    let ang = (p[1] - c[1]).atan2(p[0] - c[0]);
    // angle travelled from the start in the sweep direction, in [0, 2pi)
    let tau = std::f64::consts::TAU;
    let mut t = if sweep > 0.0 { ang - a0 } else { a0 - ang };
    t = t.rem_euclid(tau);
    let da = (p[0] - a[0]).hypot(p[1] - a[1]);
    let db = (p[0] - b[0]).hypot(p[1] - b[1]);
    let ends = da.min(db);
    if t <= sweep.abs() {
        (((p[0] - c[0]).hypot(p[1] - c[1])) - r).abs().min(ends)
    } else {
        ends
    }
}

/// Exact distance from `p` to the boundary of loop `l`.
pub fn dist_loop(p: Pt, l: &Loop) -> f64 {
    let n = l.len();
    let mut best = f64::MAX;
    for i in 0..n {
        let (a, b) = (l[i], l[(i + 1) % n]);
        let (a2, b2) = ([a[0], a[1]], [b[0], b[1]]);
        let d = if a[2] == 0.0 { dist_seg(p, a2, b2) } else { dist_arc(p, a2, b2, a[2]) };
        best = best.min(d);
    }
    best
}

pub fn dist_region(p: Pt, r: &Region) -> f64 {
    r.loops().map(|l| dist_loop(p, l)).fold(f64::MAX, f64::min)
}

/// Flatten a loop to a polygon with chord error at most `tol`.
pub fn flatten(l: &Loop, tol: f64) -> Vec<Pt> {
    let n = l.len();
    let mut out = Vec::new();
    for i in 0..n {
        let (a, b) = (l[i], l[(i + 1) % n]);
        out.push([a[0], a[1]]);
        if a[2] != 0.0 {
            let (c, r, a0, sweep) = arc_params([a[0], a[1]], [b[0], b[1]], a[2]);
            // chord error r(1 - cos(step/2)) <= tol
            let step = 2.0 * (1.0 - (tol / r).min(1.0)).acos();
            let k = if step > 0.0 { ((sweep.abs() / step).ceil() as usize).clamp(1, 200_000) } else { 200_000 };
            for j in 1..k {
                let t = a0 + sweep * j as f64 / k as f64;
                out.push([c[0] + r * t.cos(), c[1] + r * t.sin()]);
            }
        }
    }
    out
}

pub fn in_polygon(p: Pt, poly: &[Pt]) -> bool {
    let mut inside = false;
    let n = poly.len();
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (poly[i], poly[j]);
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0] {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Even-odd membership over a set of loops (outers and holes), flattened at `tol`.
pub struct Membership {
    polys: Vec<Vec<Pt>>,
}

impl Membership {
    pub fn new<'a>(loops: impl IntoIterator<Item = &'a Loop>, tol: f64) -> Self {
        Membership { polys: loops.into_iter().filter(|l| l.len() > 1).map(|l| flatten(l, tol)).collect() }
    }
    pub fn contains(&self, p: Pt) -> bool {
        self.polys.iter().filter(|q| in_polygon(p, q)).count() % 2 == 1
    }
}

pub fn region_membership(r: &Region, tol: f64) -> Membership {
    Membership::new(r.loops(), tol)
}

/// A small deterministic generator (xorshift64*), so the corpus is the same on every run.
pub struct Rng(pub u64);
impl Rng {
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
    pub fn range(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.unit()
    }
}
