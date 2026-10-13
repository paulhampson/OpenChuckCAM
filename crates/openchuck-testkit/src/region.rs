// SPDX-License-Identifier: AGPL-3.0-or-later
//! The independent oracle for 2D offsets on an unrolled cylinder, and the
//! corpus of awkward regions (OP-04, NFR-05, NFR-07; decision record 0016
//! consequences: "builds the oracle from the spike into its tests"; the
//! corpus and the oracle come from spike SP-7).
//!
//! A region lives in the development plane: x along the rotary axis and
//! s = R times the angle around it, both in millimetres. Loops are lists of
//! `[x, s, bulge]` vertices, the bulge being `tan(sweep / 4)` of the segment
//! to the next vertex (zero for a line, as in `cavalier_contours`).
//!
//! **Nothing here calls the offset library**: the exact distance to a boundary
//! of lines and arcs, area, flattening and membership are written from first
//! principles, so the library does not grade itself. It is test-only code
//! (the crate is never a normal dependency), so indexing is allowed in it
//! and a bad index is a test failure, not a crash of the product.
#![allow(clippy::indexing_slicing)]

/// A point in the development plane.
pub type Pt = [f64; 2];

/// One closed loop: vertices `[x, s, bulge]`.
pub type Loop = Vec<[f64; 3]>;

/// A filled region: an outer boundary and holes. Orientation is fixed by
/// [`Region::new`] (outer counter-clockwise, holes clockwise).
#[derive(Clone, Debug)]
pub struct Region {
    /// The outer boundary.
    pub outer: Loop,
    /// The holes.
    pub holes: Vec<Loop>,
}

/// The exact signed area of a loop, arcs included.
pub fn signed_area(l: &Loop) -> f64 {
    let n = l.len();
    let mut a = 0.0;
    for i in 0..n {
        let p = l[i];
        let q = l[(i + 1) % n];
        a += 0.5 * (p[0] * q[1] - q[0] * p[1]);
        let b = p[2];
        if b != 0.0 {
            let d = (q[0] - p[0]).hypot(q[1] - p[1]);
            let theta = 4.0 * b.atan();
            let r = d * (1.0 + b * b) / (4.0 * b.abs());
            a += 0.5 * r * r * (theta - theta.sin());
        }
    }
    a
}

/// The loop traversed the other way.
pub fn reverse(l: &Loop) -> Loop {
    let n = l.len();
    (0..n)
        .map(|k| {
            let i = n - 1 - k;
            let prev = (i + n - 1) % n;
            [l[i][0], l[i][1], -l[prev][2]]
        })
        .collect()
}

impl Region {
    /// A region with the orientation rules applied.
    pub fn new(outer: Loop, holes: Vec<Loop>) -> Self {
        let outer = if signed_area(&outer) < 0.0 {
            reverse(&outer)
        } else {
            outer
        };
        let holes = holes
            .into_iter()
            .map(|h| {
                if signed_area(&h) > 0.0 {
                    reverse(&h)
                } else {
                    h
                }
            })
            .collect();
        Self { outer, holes }
    }

    /// The outer boundary, then the holes.
    pub fn loops(&self) -> impl Iterator<Item = &Loop> {
        std::iter::once(&self.outer).chain(self.holes.iter())
    }

    /// Area of the region.
    pub fn area(&self) -> f64 {
        signed_area(&self.outer) + self.holes.iter().map(signed_area).sum::<f64>()
    }

    /// `[min_x, min_s, max_x, max_s]` of the vertices.
    pub fn bounds(&self) -> [f64; 4] {
        let mut b = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
        for v in self.loops().flatten() {
            b[0] = b[0].min(v[0]);
            b[1] = b[1].min(v[1]);
            b[2] = b[2].max(v[0]);
            b[3] = b[3].max(v[1]);
        }
        b
    }
}

// ---- constructors ----------------------------------------------------------

/// An axis-aligned rectangle.
pub fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Loop {
    vec![[x0, y0, 0.0], [x1, y0, 0.0], [x1, y1, 0.0], [x0, y1, 0.0]]
}

/// A circle as two half arcs (bulge 1), counter-clockwise.
pub fn circle(cx: f64, cy: f64, r: f64) -> Loop {
    vec![[cx - r, cy, 1.0], [cx + r, cy, 1.0]]
}

/// A rectangle with all four corners rounded by `r`.
pub fn rounded_rect(x0: f64, y0: f64, x1: f64, y1: f64, r: f64) -> Loop {
    let b = (std::f64::consts::PI / 8.0).tan();
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

/// A slot (stadium) with its axis along x.
pub fn slot(x0: f64, x1: f64, cy: f64, half_w: f64) -> Loop {
    vec![
        [x0, cy - half_w, 0.0],
        [x1, cy - half_w, 1.0],
        [x1, cy + half_w, 0.0],
        [x0, cy + half_w, 1.0],
    ]
}

/// A polygon through `pts`.
pub fn polygon(pts: &[Pt]) -> Loop {
    pts.iter().map(|p| [p[0], p[1], 0.0]).collect()
}

/// A regular star with `n` points, outer radius `ro` and inner radius `ri`.
pub fn star(cx: f64, cy: f64, n: usize, ro: f64, ri: f64, phase: f64) -> Loop {
    (0..2 * n)
        .map(|i| {
            let r = if i % 2 == 0 { ro } else { ri };
            let a = phase + std::f64::consts::PI * i as f64 / n as f64;
            [cx + r * a.cos(), cy + r * a.sin(), 0.0]
        })
        .collect()
}

// ---- the oracle ------------------------------------------------------------

fn dist_seg(p: Pt, a: Pt, b: Pt) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let l2 = dx * dx + dy * dy;
    let t = if l2 == 0.0 {
        0.0
    } else {
        (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2).clamp(0.0, 1.0)
    };
    (p[0] - a[0] - t * dx).hypot(p[1] - a[1] - t * dy)
}

/// Centre, radius, start angle and signed sweep of the arc a to b with
/// bulge `bg`.
pub fn arc_params(a: Pt, b: Pt, bg: f64) -> (Pt, f64, f64, f64) {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let d = dx.hypot(dy);
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
    let mut t = if sweep > 0.0 { ang - a0 } else { a0 - ang };
    t = t.rem_euclid(std::f64::consts::TAU);
    let ends = (p[0] - a[0])
        .hypot(p[1] - a[1])
        .min((p[0] - b[0]).hypot(p[1] - b[1]));
    if t <= sweep.abs() {
        ((p[0] - c[0]).hypot(p[1] - c[1]) - r).abs().min(ends)
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
        let d = if a[2] == 0.0 {
            dist_seg(p, a2, b2)
        } else {
            dist_arc(p, a2, b2, a[2])
        };
        best = best.min(d);
    }
    best
}

/// Exact distance from `p` to the boundary of the region.
pub fn dist_region(p: Pt, r: &Region) -> f64 {
    r.loops().map(|l| dist_loop(p, l)).fold(f64::MAX, f64::min)
}

/// A loop as a polygon with chord error at most `tol`.
pub fn flatten(l: &Loop, tol: f64) -> Vec<Pt> {
    let n = l.len();
    let mut out = Vec::new();
    for i in 0..n {
        let (a, b) = (l[i], l[(i + 1) % n]);
        out.push([a[0], a[1]]);
        if a[2] != 0.0 {
            let (c, r, a0, sweep) = arc_params([a[0], a[1]], [b[0], b[1]], a[2]);
            let step = 2.0 * (1.0 - (tol / r).min(1.0)).acos();
            let k = if step > 0.0 {
                ((sweep.abs() / step).ceil() as usize).clamp(1, 200_000)
            } else {
                200_000
            };
            for j in 1..k {
                let t = a0 + sweep * j as f64 / k as f64;
                out.push([c[0] + r * t.cos(), c[1] + r * t.sin()]);
            }
        }
    }
    out
}

fn in_polygon(p: Pt, poly: &[Pt]) -> bool {
    let mut inside = false;
    let n = poly.len();
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (poly[i], poly[j]);
        if (a[1] > p[1]) != (b[1] > p[1])
            && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0]
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Even-odd membership over a set of loops.
#[derive(Debug)]
pub struct Membership {
    polys: Vec<Vec<Pt>>,
}

impl Membership {
    /// Loops flattened at `tol`.
    pub fn new<'a>(loops: impl IntoIterator<Item = &'a Loop>, tol: f64) -> Self {
        Self {
            polys: loops
                .into_iter()
                .filter(|l| l.len() > 1)
                .map(|l| flatten(l, tol))
                .collect(),
        }
    }

    /// True when `p` is inside an odd number of the loops.
    pub fn contains(&self, p: Pt) -> bool {
        self.polys.iter().filter(|q| in_polygon(p, q)).count() % 2 == 1
    }
}

/// Membership of a region.
pub fn region_membership(r: &Region, tol: f64) -> Membership {
    Membership::new(r.loops(), tol)
}

/// A small deterministic generator (xorshift64*), so the corpus is the same
/// on every run.
#[derive(Debug)]
pub struct Rng(pub u64);

impl Rng {
    /// The next 64 bits.
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// A number in `[0, 1)`.
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// A number in `[a, b)`.
    pub fn range(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.unit()
    }
}

// ---- checking an offset result ---------------------------------------------

/// Largest allowed distance between a result boundary point and the exact
/// offset curve, mm: a tenth of the 0.01 mm toolpath deviation (verification
/// plan 4.1).
pub const DEV_LIMIT: f64 = 1e-3;
/// Chord tolerance the oracle uses for its own membership tests.
pub const ARC_TOL: f64 = 1e-4;

/// What an offset check found.
#[derive(Clone, Debug, Default)]
pub struct Verdict {
    /// Loops in the result.
    pub loops: usize,
    /// Arc segments in the result.
    pub arcs: usize,
    /// Line segments in the result.
    pub lines: usize,
    /// Largest distance of a boundary sample from the exact offset curve.
    pub max_dev: f64,
    /// Random points on the wrong side of the exact offset curve.
    pub mismatches: usize,
    /// Random points judged.
    pub samples: usize,
    /// A coordinate was not finite.
    pub nonfinite: bool,
}

impl Verdict {
    /// True when the result agrees with the exact offset.
    pub fn ok(&self, limit: f64) -> bool {
        !self.nonfinite && self.max_dev <= limit && self.mismatches == 0
    }
}

/// Points along a loop: flattened vertices and the midpoints of every
/// flattened segment, at a chord error of 1e-5 mm.
pub fn boundary_samples(l: &Loop) -> Vec<Pt> {
    let f = flatten(l, 1e-5);
    let n = f.len();
    let mut out = Vec::with_capacity(2 * n);
    for i in 0..n {
        let (a, b) = (f[i], f[(i + 1) % n]);
        out.push(a);
        out.push([0.5 * (a[0] + b[0]), 0.5 * (a[1] + b[1])]);
    }
    out
}

fn signed_dist(base: &Region, mem: &Membership, p: Pt) -> f64 {
    let d = dist_region(p, base);
    if mem.contains(p) { d } else { -d }
}

/// Checks `result` as the inward offset of `base` by `d` (`d < 0` grows the
/// region): boundary samples lie on the exact offset curve, and random points
/// away from it agree with the exact signed distance.
pub fn check_offset(
    base: &Region,
    result: &[Loop],
    d: f64,
    rng: &mut Rng,
    n_random: usize,
    limit: f64,
) -> Verdict {
    let margin = 2.0 * limit;
    let mut v = Verdict {
        loops: result.len(),
        ..Verdict::default()
    };
    for l in result {
        for a in l {
            if !(a[0].is_finite() && a[1].is_finite() && a[2].is_finite()) {
                v.nonfinite = true;
            }
            if a[2] == 0.0 {
                v.lines += 1;
            } else {
                v.arcs += 1;
            }
        }
    }
    if v.nonfinite {
        return v;
    }
    let base_mem = region_membership(base, ARC_TOL);
    for l in result {
        for p in boundary_samples(l) {
            let dev = (signed_dist(base, &base_mem, p) - d).abs();
            v.max_dev = v.max_dev.max(dev);
        }
    }
    let res_mem = Membership::new(result.iter(), ARC_TOL);
    let b = base.bounds();
    let pad = d.abs() + 1.0;
    for _ in 0..n_random {
        let p = [
            rng.range(b[0] - pad, b[2] + pad),
            rng.range(b[1] - pad, b[3] + pad),
        ];
        let sd = signed_dist(base, &base_mem, p);
        if (sd - d).abs() < margin {
            continue;
        }
        v.samples += 1;
        if (sd >= d) != res_mem.contains(p) {
            v.mismatches += 1;
        }
    }
    v
}

/// The exact signed distance of `p` inside `base` (negative outside).
pub fn signed_distance(base: &Region, mem: &Membership, p: Pt) -> f64 {
    signed_dist(base, mem, p)
}

// ---- the corpus ------------------------------------------------------------

/// One awkward region and the pass spacing to sweep it with.
#[derive(Debug)]
pub struct Case {
    /// Name for reports.
    pub name: String,
    /// The region.
    pub region: Region,
    /// Pass spacing, mm.
    pub step: f64,
    /// Deliberately degenerate (touching, invalid): the product must refuse
    /// it with a message; a wrong region or a panic is not acceptable.
    pub degenerate: bool,
}

fn case(name: &str, region: Region, step: f64) -> Case {
    Case {
        name: name.into(),
        region,
        step,
        degenerate: false,
    }
}

fn degenerate(name: &str, region: Region, step: f64) -> Case {
    Case {
        name: name.into(),
        region,
        step,
        degenerate: true,
    }
}

/// The hand-made cases of SP-7, in the unrolled plane (mm).
pub fn hand_made() -> Vec<Case> {
    let mut v = Vec::new();
    v.push(case(
        "bore",
        Region::new(rect(0.0, 0.0, 60.0, 120.0), vec![circle(30.0, 60.0, 8.0)]),
        0.5,
    ));
    let mut holes = Vec::new();
    for i in 0..6 {
        for j in 0..6 {
            holes.push(circle(
                10.0 + 14.0 * f64::from(i),
                10.0 + 14.0 * f64::from(j),
                5.0,
            ));
        }
    }
    v.push(case(
        "hole_grid_6x6_gap4",
        Region::new(rect(0.0, 0.0, 90.0, 90.0), holes),
        0.4,
    ));
    v.push(case(
        "rounded_rect_r6",
        Region::new(rounded_rect(0.0, 0.0, 80.0, 50.0, 6.0), vec![]),
        0.5,
    ));
    v.push(case(
        "rounded_rect_with_slot_hole",
        Region::new(
            rounded_rect(0.0, 0.0, 80.0, 50.0, 6.0),
            vec![slot(20.0, 60.0, 25.0, 4.0)],
        ),
        0.5,
    ));
    v.push(case(
        "slot_w6",
        Region::new(slot(0.0, 40.0, 0.0, 3.0), vec![]),
        0.25,
    ));
    v.push(case(
        "dumbbell_neck4",
        Region::new(
            polygon(&[
                [0.0, 0.0],
                [30.0, 0.0],
                [30.0, 13.0],
                [50.0, 13.0],
                [50.0, 0.0],
                [80.0, 0.0],
                [80.0, 30.0],
                [50.0, 30.0],
                [50.0, 17.0],
                [30.0, 17.0],
                [30.0, 30.0],
                [0.0, 30.0],
            ]),
            vec![],
        ),
        0.5,
    ));
    v.push(case(
        "star5_deep",
        Region::new(star(0.0, 0.0, 5, 30.0, 6.0, 0.3), vec![]),
        0.3,
    ));
    v.push(case(
        "star12",
        Region::new(
            star(0.0, 0.0, 12, 30.0, 18.0, 0.0),
            vec![circle(0.0, 0.0, 6.0)],
        ),
        0.3,
    ));
    for (g, nm) in [
        (1e-1, "1e-1"),
        (1e-2, "1e-2"),
        (1e-3, "1e-3"),
        (1e-4, "1e-4"),
    ] {
        v.push(case(
            &format!("hole_near_edge_gap_{nm}"),
            Region::new(
                rect(0.0, 0.0, 60.0, 60.0),
                vec![circle(30.0, 10.0 + g, 10.0)],
            ),
            0.5,
        ));
    }
    v.push(degenerate(
        "hole_tangent_to_edge_gap_0",
        Region::new(rect(0.0, 0.0, 60.0, 60.0), vec![circle(30.0, 10.0, 10.0)]),
        0.5,
    ));
    v.push(degenerate(
        "hole_tangent_to_edge_gap_1e-7",
        Region::new(
            rect(0.0, 0.0, 60.0, 60.0),
            vec![circle(30.0, 10.0 + 1e-7, 10.0)],
        ),
        0.5,
    ));
    v.push(case(
        "two_holes_gap_1e-3",
        Region::new(
            rect(0.0, 0.0, 60.0, 40.0),
            vec![circle(20.0, 20.0, 8.0), circle(36.001, 20.0, 8.0)],
        ),
        0.5,
    ));
    v.push(case(
        "sliver_w0.02",
        Region::new(rect(0.0, 0.0, 50.0, 0.02), vec![]),
        0.005,
    ));
    v.push(case(
        "spike_1deg",
        Region::new(
            polygon(&[
                [0.0, 0.0],
                [40.0, 0.35],
                [40.0, 0.0],
                [20.0, -15.0],
                [-10.0, -5.0],
                [-10.0, 15.0],
            ]),
            vec![],
        ),
        0.3,
    ));
    v.push(case(
        "redundant_vertices",
        Region::new(
            vec![
                [0.0, 0.0, 0.0],
                [0.0, 0.0, 0.0],
                [10.0, 0.0, 0.0],
                [20.0, 0.0, 1e-12],
                [40.0, 0.0, 0.0],
                [40.0, 10.0, 0.0],
                [40.0, 30.0, 1e-9],
                [40.0, 30.0, 0.0],
                [0.0, 30.0, 0.0],
                [0.0, 15.0, 0.0],
            ],
            vec![],
        ),
        0.5,
    ));
    v.push(case(
        "far_from_origin_s2000",
        Region::new(
            rounded_rect(0.0, 2000.0, 60.0, 2060.0, 8.0),
            vec![circle(30.0, 2030.0, 9.0)],
        ),
        0.5,
    ));
    v.push(case(
        "tiny_hole_r0.3",
        Region::new(rect(0.0, 0.0, 30.0, 30.0), vec![circle(15.0, 15.0, 0.3)]),
        0.5,
    ));
    v.push(case(
        "disc_r20",
        Region::new(circle(0.0, 0.0, 20.0), vec![]),
        0.5,
    ));
    v.push(case(
        "annulus_r20_r8",
        Region::new(circle(0.0, 0.0, 20.0), vec![circle(0.0, 0.0, 8.0)]),
        0.5,
    ));
    v.push(case(
        "disc_off_centre_hole",
        Region::new(circle(0.0, 0.0, 20.0), vec![circle(1.2, 0.0, 17.0)]),
        0.2,
    ));
    v
}

/// True if no two non-adjacent edges of the flattened loop cross (the
/// corpus's own check, independent of the libraries).
pub fn is_simple(l: &Loop) -> bool {
    let p = flatten(l, 0.01);
    let n = p.len();
    fn cross(a: Pt, b: Pt, c: Pt) -> f64 {
        (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
    }
    for i in 0..n {
        for j in i + 2..n {
            if i == 0 && j == n - 1 {
                continue;
            }
            let (a, b, c, d) = (p[i], p[(i + 1) % n], p[j], p[(j + 1) % n]);
            let (d1, d2, d3, d4) = (
                cross(a, b, c),
                cross(a, b, d),
                cross(c, d, a),
                cross(c, d, b),
            );
            if d1 * d2 < 0.0 && d3 * d4 < 0.0 {
                return false;
            }
        }
    }
    true
}

/// Seeded random cases: star-shaped polygons (some with arcs on their edges)
/// and rectangles with random non-overlapping holes. Candidates that fail
/// the corpus's own validity check are dropped.
pub fn random_cases(seed: u64, count: usize) -> Vec<Case> {
    let mut rng = Rng(seed);
    let mut out = Vec::new();
    let mut i = 0;
    while out.len() < count {
        i += 1;
        let kind = i % 3;
        if kind == 0 || kind == 1 {
            let n = 5 + (rng.next_u64() % 9) as usize;
            let arcs = kind == 1;
            let mut v: Loop = Vec::new();
            for k in 0..n {
                let a = std::f64::consts::TAU * (k as f64 + rng.range(-0.3, 0.3)) / n as f64;
                let r = rng.range(8.0, 30.0);
                let bulge = if arcs { rng.range(-0.25, 0.25) } else { 0.0 };
                v.push([r * a.cos(), r * a.sin(), bulge]);
            }
            if !is_simple(&v) {
                continue;
            }
            let mut holes = Vec::new();
            if rng.unit() < 0.5 {
                let r = rng.range(1.0, 4.0);
                let h = circle(rng.range(-2.0, 2.0), rng.range(-2.0, 2.0), r);
                if dist_loop([0.0, 0.0], &v) > r + 3.0 {
                    holes.push(h);
                }
            }
            let nm = if arcs { "rand_arc_poly" } else { "rand_poly" };
            out.push(case(
                &format!("{nm}_{i}"),
                Region::new(v, holes),
                rng.range(0.2, 0.8),
            ));
        } else {
            let (w, h) = (rng.range(30.0, 80.0), rng.range(30.0, 80.0));
            let mut holes: Vec<Loop> = Vec::new();
            let mut centres: Vec<(f64, f64, f64)> = Vec::new();
            for _ in 0..(2 + rng.next_u64() % 10) {
                let r = rng.range(1.5, 7.0);
                let (cx, cy) = (
                    rng.range(r + 1.0, w - r - 1.0),
                    rng.range(r + 1.0, h - r - 1.0),
                );
                if centres
                    .iter()
                    .all(|&(x, y, q)| (x - cx).hypot(y - cy) > q + r + 0.5)
                {
                    centres.push((cx, cy, r));
                    holes.push(circle(cx, cy, r));
                }
            }
            out.push(case(
                &format!("rand_holes_{i}"),
                Region::new(rect(0.0, 0.0, w, h), holes),
                rng.range(0.3, 1.0),
            ));
        }
    }
    out
}
