// SPDX-License-Identifier: AGPL-3.0-or-later
//! The bounded tool-contact checker (OP-12, NFR-05, NFR-12; decision record
//! 0209 item 4.1, M2; decision record 0211; issue #346).
//!
//! The sampled checks ([`crate::gouge`] and the clouds of decision 0211)
//! read the clearance at points spaced on a face, and a point grid can miss
//! the deepest point by more than its spacing suggests: decision 0211's flat
//! cutter misses 0.0067 mm at 0.05 mm spacing. This checker instead returns,
//! for one tool pose, an interval `[lower, upper]` that holds the exact
//! smallest signed clearance between the tool and the faces, and it fails
//! rather than answer when it cannot make that interval narrower than the
//! tolerance asked for. The bound is proven, not sampled:
//!
//! - **The tool is convex.** Ball, tapered-ball, flat and bull-nose cutters
//!   are convex solids of revolution (checked from the profile, which is
//!   refused otherwise), capped flat at the top of the profile. The signed
//!   distance `f` to a convex solid is a convex function, so at any point
//!   `q` of the tool's surface with outward normal `n`,
//!   `f(x) >= n . (x - q)` for every `x`. That is the lower bound.
//! - **Each piece of face lies inside a known hull.** Every face is
//!   evaluated as rational Bézier patches with positive weights: B-spline
//!   faces by blossoming (knot insertion) in each knot span, analytic faces
//!   by their exact rational forms (circle arcs of at most 90 degrees).
//!   The image of a parameter box lies in the convex hull of its projected
//!   control points, so `min f` over it is at least
//!   `min_i n . (Q_i - q)`, and also at least `f(c) - max_i |Q_i - c|`
//!   for its centre point `c` (`f` is 1-Lipschitz). Tessellation triangles
//!   are their own hulls.
//! - **Branch and bound.** Pieces are split, nearest first, until every
//!   piece left has a lower bound within the tolerance of the smallest
//!   clearance actually found at a point of the face. That smallest found
//!   value is `upper`; the smallest lower bound left is `lower`.
//! - **Trims and boundaries.** A face is the part of its parameter box the
//!   trim polylines enclose, read as `openchuck-path` reads them (even-odd,
//!   one period either side for a periodic face). A piece the trims cross is
//!   bounded as a whole (it holds the face's part) and split until the face
//!   points found in it, at its centre or on a trim line, close the gap. The
//!   boundary is part of the face.
//!
//! What it does not establish: the trims are the B-rep's polylines in
//! (u, v), so how far those lie from the kernel's exact trim curves is
//! outside the bound; the tool is the cutting profile only (no shank or
//! holder above it); `upper` and `lower` are capped at the margin; and the
//! arithmetic is double precision, covered by a slack of [`SLACK`] mm.
//!
//! Nothing here uses `openchuck-path`: the faces are evaluated from their
//! plain data and the tool from its profile's pieces, so the strategies and
//! their evaluators do not grade themselves.

use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::f64::consts::{FRAC_PI_2, TAU};
use std::fmt;

use openchuck_types::brep::{BSplinePatch, BrepFace, BrepModel, Frame, Surface};
use openchuck_types::geometry::{Point3, Vec3};
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::tool::RevolvedProfile;
use openchuck_types::tool::profile::ProfilePiece;

/// Added to every lower bound against rounding, mm: double-precision error
/// on parts of a few hundred millimetres is far smaller, and a validated
/// profile's arcs may miss their stated radius by up to 1e-6 mm.
pub const SLACK: f64 = 2e-6;

/// Pieces are split before any pose is checked until their hull fits in a
/// ball of this radius, mm.
const LEAF: f64 = 0.5;

/// Angle tolerance of the convexity check, radians.
const TURN: f64 = 1e-9;

/// Why no bound was given.
#[derive(Debug, Clone, PartialEq)]
pub enum ContactError {
    /// The tool's profile is not convex, so the clearance is not convex and
    /// the lower bound does not hold.
    NotConvex(String),
    /// A face cannot be evaluated with a proven hull.
    Face {
        /// The face's number.
        face: u32,
        /// Why.
        reason: String,
    },
    /// The interval did not close within the budget of pieces.
    Budget {
        /// Pieces examined.
        cells: usize,
        /// The interval reached.
        lower: f64,
        /// The smallest clearance found.
        upper: f64,
    },
    /// The pose, the settings or a triangle are not usable numbers.
    Invalid(String),
}

impl fmt::Display for ContactError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotConvex(why) => write!(f, "no bound: the tool is not convex ({why})"),
            Self::Face { face, reason } => write!(f, "no bound for face {face}: {reason}"),
            Self::Budget {
                cells,
                lower,
                upper,
            } => write!(
                f,
                "no bound: after {cells} pieces the clearance is only known to lie in [{lower}, {upper}] mm"
            ),
            Self::Invalid(why) => write!(f, "no bound: {why}"),
        }
    }
}

impl std::error::Error for ContactError {}

/// How a pose is checked.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Settings {
    /// Clearances above this are not resolved: the answer is then
    /// `[margin - tolerance, margin]` or better, mm.
    pub margin: f64,
    /// The widest interval accepted, mm; must be positive.
    pub tolerance: f64,
    /// The most pieces examined for one pose before the check fails.
    pub budget: usize,
}

/// The smallest signed clearance between the tool at one pose and the
/// geometry: negative by the deepest a face point lies inside the tool.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounded {
    /// The smallest clearance found at a point of the geometry, or the
    /// margin when none is below it, mm. The exact value is at most this.
    pub upper: f64,
    /// A proven lower bound on the exact value (capped at the margin), mm.
    pub lower: f64,
    /// Where `upper` was found; `None` when nothing is within the margin.
    pub at: Option<Point3>,
    /// Pieces examined.
    pub cells: usize,
}

impl Bounded {
    /// The width of the interval: the most the found value can exceed the
    /// exact one, mm.
    pub fn error(&self) -> f64 {
        self.upper - self.lower
    }
}

// ---------------------------------------------------------------------------
// The tool.

/// A point of the tool's half-plane: distance from the axis and height
/// above the tip.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Rz {
    r: f64,
    z: f64,
}

impl Rz {
    fn dist(self, o: Self) -> f64 {
        (self.r - o.r).hypot(self.z - o.z)
    }
}

#[derive(Debug, Clone, Copy)]
enum Piece {
    Line {
        a: Rz,
        b: Rz,
    },
    /// A counter-clockwise arc bulging out of the tool, from angle `t0` to
    /// `t1` about `c`.
    Arc {
        c: Rz,
        rho: f64,
        t0: f64,
        t1: f64,
    },
}

impl Piece {
    /// The nearest point of the piece to `p`, and the piece's outward
    /// normal there.
    fn nearest(&self, p: Rz) -> (Rz, Rz) {
        match *self {
            Self::Line { a, b } => {
                let (dr, dz) = (b.r - a.r, b.z - a.z);
                let l2 = dr * dr + dz * dz;
                let t = if l2 > 0.0 {
                    (((p.r - a.r) * dr + (p.z - a.z) * dz) / l2).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let l = l2.sqrt().max(f64::MIN_POSITIVE);
                (
                    Rz {
                        r: a.r + t * dr,
                        z: a.z + t * dz,
                    },
                    Rz {
                        r: dz / l,
                        z: -dr / l,
                    },
                )
            }
            Self::Arc { c, rho, t0, t1 } => {
                let t = (p.z - c.z).atan2(p.r - c.r);
                let at = |t: f64| {
                    (
                        Rz {
                            r: c.r + rho * t.cos(),
                            z: c.z + rho * t.sin(),
                        },
                        Rz {
                            r: t.cos(),
                            z: t.sin(),
                        },
                    )
                };
                if t >= t0 && t <= t1 {
                    at(t)
                } else {
                    let (a, b) = (at(t0), at(t1));
                    if a.0.dist(p) <= b.0.dist(p) { a } else { b }
                }
            }
        }
    }

    /// The radius of the piece at height `z`, if the piece spans it.
    fn width_at(&self, z: f64) -> Option<f64> {
        match *self {
            Self::Line { a, b } => (b.z > a.z && z >= a.z && z <= b.z)
                .then(|| a.r + (z - a.z) * (b.r - a.r) / (b.z - a.z)),
            Self::Arc { c, rho, t0, t1 } => {
                let (z0, z1) = (c.z + rho * t0.sin(), c.z + rho * t1.sin());
                (z1 > z0 && z >= z0 && z <= z1)
                    .then(|| c.r + (rho * rho - (z - c.z) * (z - c.z)).max(0.0).sqrt())
            }
        }
    }
}

/// The cutting part of a tool as a convex solid of revolution: its profile
/// from the tip, capped flat at the profile's top.
#[derive(Debug, Clone)]
pub struct ConvexTool {
    /// The profile's pieces, then the cap from the rim to the axis.
    pieces: Vec<Piece>,
    top: f64,
}

impl ConvexTool {
    /// The tool of `profile`; refused unless the profile starts on the axis
    /// and turns only outwards (no concave arc, no reflex joint), so that
    /// the solid is convex.
    pub fn new(profile: &RevolvedProfile) -> Result<Self, ContactError> {
        let rz = |p: openchuck_types::tool::ProfilePoint| Rz {
            r: p.radius.0,
            z: p.height.0,
        };
        let bad = |why: String| Err(ContactError::NotConvex(why));
        let mut pieces = Vec::new();
        // The direction of travel, as an angle from +r towards +z: it may
        // only turn left, from at least 0 at the tip to at most 90 degrees.
        let mut heading = 0.0_f64;
        let mut first = true;
        for (i, piece) in profile.pieces().iter().enumerate() {
            let (start, end) = (rz(piece.start()), rz(piece.end()));
            if first && start.dist(Rz { r: 0.0, z: 0.0 }) > 1e-9 {
                return bad("the profile does not start at the tip".to_owned());
            }
            let (p, h0, h1) = match *piece {
                ProfilePiece::Line { .. } => {
                    let h = (end.z - start.z).atan2(end.r - start.r);
                    (Piece::Line { a: start, b: end }, h, h)
                }
                ProfilePiece::Arc {
                    centre,
                    radius,
                    convex,
                    ..
                } => {
                    let c = rz(centre);
                    let t0 = (start.z - c.z).atan2(start.r - c.r);
                    let t1 = (end.z - c.z).atan2(end.r - c.r);
                    if !convex || t1 <= t0 || t1 - t0 > FRAC_PI_2 + TURN {
                        return bad(format!("piece {} is a concave or reversed arc", i + 1));
                    }
                    if !(radius.is_finite() && radius > 0.0) {
                        return bad(format!("piece {} has no radius", i + 1));
                    }
                    (
                        Piece::Arc {
                            c,
                            rho: radius,
                            t0,
                            t1,
                        },
                        t0 + FRAC_PI_2,
                        t1 + FRAC_PI_2,
                    )
                }
            };
            if h0 < heading - TURN || h1 > FRAC_PI_2 + TURN || (first && h0 < -TURN) {
                return bad(format!("the profile turns inwards at piece {}", i + 1));
            }
            heading = h1;
            first = false;
            pieces.push(p);
        }
        let end = rz(profile.end());
        if pieces.is_empty() || !(end.r > 0.0 && end.z >= 0.0 && end.r.is_finite()) {
            return bad("the profile has no rim".to_owned());
        }
        pieces.push(Piece::Line {
            a: end,
            b: Rz { r: 0.0, z: end.z },
        });
        Ok(Self { pieces, top: end.z })
    }

    fn inside(&self, p: Rz) -> bool {
        if !(p.z > 0.0 && p.z < self.top) {
            return false;
        }
        let w = self
            .pieces
            .iter()
            .filter_map(|q| q.width_at(p.z))
            .fold(f64::NEG_INFINITY, f64::max);
        p.r < w
    }

    /// The signed distance of `p` from the tool (negative inside), a point
    /// `q` of the tool's surface and an outward normal `n` of a plane through
    /// `q` that the tool lies behind, with `f(p) = n . (p - q)`.
    fn nearest(&self, p: Rz) -> (f64, Rz, Rz) {
        let mut best: Option<(f64, Rz, Rz)> = None;
        for piece in &self.pieces {
            let (q, n) = piece.nearest(p);
            let d = q.dist(p);
            if best.is_none_or(|b| d < b.0) {
                best = Some((d, q, n));
            }
        }
        let (d, q, n) = best.unwrap_or((0.0, p, Rz { r: 0.0, z: -1.0 }));
        if self.inside(p) {
            (-d, q, n)
        } else if d > 1e-12 {
            (
                d,
                q,
                Rz {
                    r: (p.r - q.r) / d,
                    z: (p.z - q.z) / d,
                },
            )
        } else {
            (d, q, n)
        }
    }
}

/// A tool placed at a pose.
#[derive(Debug, Clone, Copy)]
struct Posed<'a> {
    tool: &'a ConvexTool,
    tip: Point3,
    axis: Vec3,
    /// A unit vector across the axis, for points on it.
    across: Vec3,
}

/// The lower bound at one point: `f(x) >= n . x - k` everywhere.
#[derive(Debug, Clone, Copy)]
struct Plane {
    value: f64,
    n: Vec3,
    k: f64,
}

impl<'a> Posed<'a> {
    fn new(tool: &'a ConvexTool, tip: Point3, axis: Vec3) -> Result<Self, ContactError> {
        let len = axis.length();
        if !tip.is_finite() || !axis.is_finite() || len.partial_cmp(&0.0) != Some(Ordering::Greater)
        {
            return Err(ContactError::Invalid(format!(
                "the pose {tip:?} along {axis:?} is not finite"
            )));
        }
        let axis = axis * (1.0 / len);
        let other = if axis.x.abs() < 0.6 {
            Vec3::new(1.0, 0.0, 0.0)
        } else {
            Vec3::new(0.0, 1.0, 0.0)
        };
        let c = axis.cross(other);
        let across = c * (1.0 / c.length());
        Ok(Self {
            tool,
            tip,
            axis,
            across,
        })
    }

    fn at(&self, x: Point3) -> Plane {
        let w = x - self.tip;
        let z = w.dot(self.axis);
        let radial = w - self.axis * z;
        let r = radial.length();
        let e = if r > 1e-12 {
            radial * (1.0 / r)
        } else {
            self.across
        };
        let (value, q, n) = self.tool.nearest(Rz { r, z });
        let q3 = self.tip + self.axis * q.z + e * q.r;
        let n3 = e * n.r + self.axis * n.z;
        Plane {
            value,
            n: n3,
            k: n3.dot(q3.to_vec()),
        }
    }

    /// A proven lower bound on the clearance over the convex hull of
    /// `hull`, and the clearance at `centre`, a point inside it.
    fn bound(&self, centre: Point3, hull: &[Point3]) -> (f64, f64) {
        let p = self.at(centre);
        let mut by_plane = f64::INFINITY;
        let mut reach = 0.0_f64;
        for q in hull {
            by_plane = by_plane.min(p.n.dot(q.to_vec()) - p.k);
            reach = reach.max(centre.distance(*q));
        }
        (by_plane.max(p.value - reach) - SLACK, p.value)
    }

    /// A proven lower bound over a box, by the 1-Lipschitz property alone.
    fn bound_box(&self, lo: Point3, hi: Point3) -> f64 {
        let c = Point3::new(
            0.5 * (lo.x + hi.x),
            0.5 * (lo.y + hi.y),
            0.5 * (lo.z + hi.z),
        );
        self.at(c).value - 0.5 * hi.distance(lo) - SLACK
    }
}

// ---------------------------------------------------------------------------
// The faces.

type H = [f64; 4];

fn homog(p: Point3, w: f64) -> H {
    [p.x * w, p.y * w, p.z * w, w]
}

fn project(h: H) -> Point3 {
    Point3::new(h[0] / h[3], h[1] / h[3], h[2] / h[3])
}

fn lerp(a: H, b: H, t: f64) -> H {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
        a[3] + (b[3] - a[3]) * t,
    ]
}

/// One direction of a B-spline: its degree and knots.
#[derive(Debug, Clone)]
struct Knots {
    degree: usize,
    t: Vec<f64>,
}

impl Knots {
    fn k(&self, i: usize) -> f64 {
        self.t.get(i).copied().unwrap_or(0.0)
    }

    /// The spans `s` (knots `t[s] < t[s+1]`) of the domain, clipped to
    /// `[lo, hi]`.
    fn spans(&self, lo: f64, hi: f64) -> Vec<(usize, f64, f64)> {
        let n = self.t.len() - self.degree - 1;
        (self.degree..n)
            .filter_map(|s| {
                let (a, b) = (self.k(s).max(lo), self.k(s + 1).min(hi));
                (self.k(s) < self.k(s + 1) && a < b).then_some((s, a, b))
            })
            .collect()
    }

    /// The blossom of the curve with local poles `d` (the `degree + 1` poles
    /// of span `s`) at `args`: de Boor's algorithm with a different
    /// parameter at each level. With every argument equal it is the point at
    /// that parameter; with `a` repeated `degree - k` times and `b` `k`
    /// times it is the `k`th Bézier pole of the curve on `[a, b]`. Within
    /// the span every step is a convex combination, so weights stay
    /// positive.
    fn blossom(&self, s: usize, d: &mut [H], args: &[f64]) -> H {
        let p = self.degree;
        for (r, x) in (1..=p).zip(args) {
            for j in (r..=p).rev() {
                let lo = self.k(s + j - p);
                let hi = self.k(s + 1 + j - r);
                let alpha = if hi > lo { (x - lo) / (hi - lo) } else { 0.0 };
                let prev = d.get(j - 1).copied().unwrap_or([0.0; 4]);
                if let Some(dj) = d.get_mut(j) {
                    *dj = lerp(prev, *dj, alpha);
                }
            }
        }
        d.get(p).copied().unwrap_or([0.0; 4])
    }

    /// The Bézier poles on `[a, b]` within span `s`.
    fn bezier(&self, s: usize, local: &[H], a: f64, b: f64) -> Vec<H> {
        let p = self.degree;
        let mut args = vec![a; p];
        (0..=p)
            .map(|k| {
                if k > 0
                    && let Some(x) = args.get_mut(p - k)
                {
                    *x = b;
                }
                let mut d = local.to_vec();
                self.blossom(s, &mut d, &args)
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
struct Nurbs {
    u: Knots,
    v: Knots,
    nv: usize,
    poles: Vec<H>,
}

impl Nurbs {
    fn new(p: &BSplinePatch) -> Result<Self, String> {
        let (du, dv) = (p.degree_u as usize, p.degree_v as usize);
        let (nu, nv) = (p.poles_u as usize, p.poles_v as usize);
        if du == 0 || dv == 0 || nu <= du || nv <= dv {
            return Err("the degrees need more poles".to_owned());
        }
        if p.knots_u.len() != nu + du + 1 || p.knots_v.len() != nv + dv + 1 {
            return Err("the knot vectors do not match the poles".to_owned());
        }
        for t in [&p.knots_u, &p.knots_v] {
            if t.iter().any(|k| !k.is_finite()) || t.windows(2).any(|w| w.first() > w.last()) {
                return Err("the knots are not finite and ascending".to_owned());
            }
        }
        if p.poles.len() != nu * nv || p.weights.len() != nu * nv {
            return Err("the pole and weight counts do not match".to_owned());
        }
        if p.poles.iter().any(|q| !q.is_finite()) {
            return Err("a pole is not finite".to_owned());
        }
        // Positive weights keep the patch inside its poles' hull.
        if p.weights.iter().any(|w| !(w.is_finite() && *w > 0.0)) {
            return Err("a weight is not positive and finite".to_owned());
        }
        Ok(Self {
            u: Knots {
                degree: du,
                t: p.knots_u.clone(),
            },
            v: Knots {
                degree: dv,
                t: p.knots_v.clone(),
            },
            nv,
            poles: p
                .poles
                .iter()
                .zip(&p.weights)
                .map(|(q, w)| homog(*q, *w))
                .collect(),
        })
    }

    fn domain(&self) -> [f64; 4] {
        let (nu, nv) = (
            self.u.t.len() - self.u.degree - 1,
            self.v.t.len() - self.v.degree - 1,
        );
        [
            self.u.k(self.u.degree),
            self.u.k(nu),
            self.v.k(self.v.degree),
            self.v.k(nv),
        ]
    }

    /// The homogeneous Bézier net of the box within spans `(su, sv)`,
    /// `(degree_u + 1) x (degree_v + 1)`, u major; with `a == b` in both
    /// directions its single distinct point is the surface point.
    fn net(&self, su: usize, sv: usize, b: [f64; 4]) -> Vec<Vec<H>> {
        let (pu, pv) = (self.u.degree, self.v.degree);
        // Along u for each local row in v.
        let mut rows: Vec<Vec<H>> = Vec::with_capacity(pv + 1);
        for jv in 0..=pv {
            let local: Vec<H> = (0..=pu)
                .map(|iu| {
                    self.poles
                        .get((su + iu - pu) * self.nv + (sv + jv - pv))
                        .copied()
                        .unwrap_or([0.0, 0.0, 0.0, 1.0])
                })
                .collect();
            rows.push(self.u.bezier(su, &local, b[0], b[1]));
        }
        (0..=pu)
            .map(|k| {
                let column: Vec<H> = rows
                    .iter()
                    .map(|r| r.get(k).copied().unwrap_or([0.0, 0.0, 0.0, 1.0]))
                    .collect();
                self.v.bezier(sv, &column, b[2], b[3])
            })
            .collect()
    }

    fn point(&self, su: usize, sv: usize, u: f64, v: f64) -> Point3 {
        let (pu, pv) = (self.u.degree, self.v.degree);
        let column: Vec<H> = (0..=pv)
            .map(|jv| {
                let mut local: Vec<H> = (0..=pu)
                    .map(|iu| {
                        self.poles
                            .get((su + iu - pu) * self.nv + (sv + jv - pv))
                            .copied()
                            .unwrap_or([0.0, 0.0, 0.0, 1.0])
                    })
                    .collect();
                self.u.blossom(su, &mut local, &vec![u; pu])
            })
            .collect();
        let mut column = column;
        project(self.v.blossom(sv, &mut column, &vec![v; pv]))
    }
}

/// The analytic surfaces, in `openchuck-types`' parametrisation.
#[derive(Debug, Clone, Copy)]
enum Analytic {
    Plane,
    Cylinder { r: f64 },
    Cone { r: f64, sin: f64, cos: f64 },
    Sphere { r: f64 },
    Torus { major: f64, minor: f64 },
}

#[derive(Debug, Clone)]
enum Kind {
    Analytic(Analytic, Frame),
    Nurbs(Box<Nurbs>),
}

/// The projected control points of the rational quadratic arc of the unit
/// circle from angle `a` to `b` (`b - a` at most 90 degrees; the middle
/// weight `cos((b - a) / 2)` is positive).
fn arc(a: f64, b: f64) -> [(f64, f64); 3] {
    let h = 0.5 * (b - a);
    let m = a + h;
    let s = 1.0 / h.cos();
    [
        (a.cos(), a.sin()),
        (m.cos() * s, m.sin() * s),
        (b.cos(), b.sin()),
    ]
}

impl Analytic {
    /// Which parameters are angles, which a rational arc must cover in
    /// pieces of at most 90 degrees.
    fn angles(self) -> (bool, bool) {
        match self {
            Self::Plane => (false, false),
            Self::Cylinder { .. } | Self::Cone { .. } => (true, false),
            Self::Sphere { .. } | Self::Torus { .. } => (true, true),
        }
    }

    fn periods(self) -> (Option<f64>, Option<f64>) {
        match self {
            Self::Plane => (None, None),
            Self::Torus { .. } => (Some(TAU), Some(TAU)),
            _ => (Some(TAU), None),
        }
    }

    /// Distance from the axis and height of the meridian at `v`.
    fn meridian(self, v: f64) -> (f64, f64) {
        match self {
            Self::Plane => (0.0, 0.0),
            Self::Cylinder { r } => (r, v),
            Self::Cone { r, sin, cos } => (r + v * sin, v * cos),
            Self::Sphere { r } => (r * v.cos(), r * v.sin()),
            Self::Torus { major, minor } => (major + minor * v.cos(), minor * v.sin()),
        }
    }

    /// The meridian's control points over `[v0, v1]`: the ends of a line,
    /// or a rational arc.
    fn meridian_net(self, v0: f64, v1: f64) -> Vec<(f64, f64)> {
        match self {
            Self::Plane => Vec::new(),
            Self::Cylinder { .. } | Self::Cone { .. } => {
                vec![self.meridian(v0), self.meridian(v1)]
            }
            Self::Sphere { r } => arc(v0, v1).iter().map(|(c, s)| (r * c, r * s)).collect(),
            Self::Torus { major, minor } => arc(v0, v1)
                .iter()
                .map(|(c, s)| (major + minor * c, minor * s))
                .collect(),
        }
    }

    fn point(self, f: &Frame, u: f64, v: f64) -> Point3 {
        if let Self::Plane = self {
            return f.origin + f.x * u + f.y * v;
        }
        let (rho, h) = self.meridian(v);
        f.origin + f.x * (rho * u.cos()) + f.y * (rho * u.sin()) + f.z * h
    }

    /// The hull of the box: the exact rational Bézier net of a surface of
    /// revolution is the tensor product of the arc in u and the meridian's
    /// net in v.
    fn hull(self, f: &Frame, b: [f64; 4]) -> Vec<Point3> {
        if let Self::Plane = self {
            return vec![
                self.point(f, b[0], b[2]),
                self.point(f, b[1], b[2]),
                self.point(f, b[0], b[3]),
                self.point(f, b[1], b[3]),
            ];
        }
        let around = arc(b[0], b[1]);
        let mut out = Vec::with_capacity(9);
        for (rho, h) in self.meridian_net(b[2], b[3]) {
            for (c, s) in around {
                out.push(f.origin + f.x * (rho * c) + f.y * (rho * s) + f.z * h);
            }
        }
        out
    }

    /// Lengths of the box along u and along v, at least.
    fn extents(self, b: [f64; 4]) -> (f64, f64) {
        let (du, dv) = (b[1] - b[0], b[3] - b[2]);
        match self {
            Self::Plane => (du, dv),
            Self::Cylinder { r } => (r * du, dv),
            Self::Cone { r, sin, .. } => {
                let rho = (r + b[2] * sin).abs().max((r + b[3] * sin).abs());
                (rho * du, dv)
            }
            Self::Sphere { r } => (r * du, r * dv),
            Self::Torus { major, minor } => ((major + minor) * du, minor * dv),
        }
    }
}

fn unit(v: Vec3) -> bool {
    v.is_finite() && (v.length() - 1.0).abs() < 1e-9
}

fn check_frame(f: &Frame) -> Result<(), String> {
    let ok = f.origin.is_finite()
        && unit(f.x)
        && unit(f.y)
        && unit(f.z)
        && f.x.dot(f.y).abs() < 1e-9
        && f.y.dot(f.z).abs() < 1e-9
        && f.z.dot(f.x).abs() < 1e-9;
    if ok {
        Ok(())
    } else {
        Err("the frame is not orthonormal and finite".to_owned())
    }
}

fn positive(x: f64, what: &str) -> Result<f64, String> {
    if x.is_finite() && x > 0.0 {
        Ok(x)
    } else {
        Err(format!("the {what} is not positive and finite"))
    }
}

/// A trim segment in (u, v).
type Seg = [(f64, f64); 2];

/// The trim segments of a face, indexed by v for the even-odd test.
#[derive(Debug, Clone)]
struct Trims {
    segs: Vec<Seg>,
    v0: f64,
    step: f64,
    buckets: Vec<Vec<u32>>,
}

impl Trims {
    fn new(face: &BrepFace) -> Result<Option<Self>, String> {
        let mut segs = Vec::new();
        for l in &face.trims {
            let n = l.0.len();
            for k in 0..n {
                let (Some(a), Some(b)) = (l.0.get(k), l.0.get((k + 1) % n)) else {
                    continue;
                };
                if ![a.u, a.v, b.u, b.v].iter().all(|x| x.is_finite()) {
                    return Err("a trim point is not finite".to_owned());
                }
                segs.push([(a.u, a.v), (b.u, b.v)]);
            }
        }
        if segs.is_empty() {
            return Ok(None);
        }
        let lo = segs
            .iter()
            .flat_map(|s| [s[0].1, s[1].1])
            .fold(f64::INFINITY, f64::min);
        let hi = segs
            .iter()
            .flat_map(|s| [s[0].1, s[1].1])
            .fold(f64::NEG_INFINITY, f64::max);
        let count = (segs.len() / 4).clamp(1, 4096);
        let step = ((hi - lo) / count as f64).max(1e-12);
        let mut buckets = vec![Vec::new(); count];
        for (i, s) in segs.iter().enumerate() {
            let (a, b) = (s[0].1.min(s[1].1), s[0].1.max(s[1].1));
            // A segment at the top height lands at `count`: clamp both ends,
            // or a top edge of the trims would be in no bucket.
            let (ka, kb) = (
                (((a - lo) / step).floor().max(0.0) as usize).min(count - 1),
                (((b - lo) / step).floor().max(0.0) as usize).min(count - 1),
            );
            for k in ka..=kb {
                if let Some(bucket) = buckets.get_mut(k) {
                    bucket.push(i as u32);
                }
            }
        }
        Ok(Some(Self {
            segs,
            v0: lo,
            step,
            buckets,
        }))
    }

    /// The segments that may reach heights `a` to `b`.
    fn near(&self, a: f64, b: f64) -> impl Iterator<Item = &Seg> {
        let last = self.buckets.len().saturating_sub(1);
        let k = |v: f64| (((v - self.v0) / self.step).floor().max(0.0) as usize).min(last);
        let (ka, kb) = if b < self.v0 || a > self.v0 + self.step * (last + 1) as f64 {
            (1, 0)
        } else {
            (k(a), k(b))
        };
        let mut seen = std::collections::BTreeSet::new();
        (ka..=kb)
            .filter_map(|i| self.buckets.get(i))
            .flatten()
            .filter(move |i| seen.insert(**i))
            .filter_map(|i| self.segs.get(*i as usize))
    }

    /// The even-odd test of `openchuck-path` (`inside_one`).
    fn odd(&self, u: f64, v: f64) -> bool {
        let mut odd = false;
        for s in self.near(v, v) {
            let ((px, py), (x, y)) = (s[0], s[1]);
            if (y > v) != (py > v) && u < (px - x) * (v - y) / (py - y) + x {
                odd = !odd;
            }
        }
        odd
    }

    /// A point where a segment crosses the closed box, if one does.
    fn crossing(&self, b: [f64; 4]) -> Option<(f64, f64)> {
        self.near(b[2], b[3]).find_map(|s| clip(*s, b))
    }
}

/// The middle of the part of segment `s` inside the open box `b`
/// (Liang-Barsky), if it enters the box's interior. A segment that only
/// runs along an edge of the box, or touches a corner, does not: the trims'
/// even-odd parity is then the same all over the inside, and the boundary
/// is covered by the neighbouring boxes.
fn clip(s: Seg, b: [f64; 4]) -> Option<(f64, f64)> {
    let ((x0, y0), (x1, y1)) = (s[0], s[1]);
    let (dx, dy) = (x1 - x0, y1 - y0);
    let (mut t0, mut t1) = (0.0_f64, 1.0_f64);
    for (p, q) in [
        (-dx, x0 - b[0]),
        (dx, b[1] - x0),
        (-dy, y0 - b[2]),
        (dy, b[3] - y0),
    ] {
        if p == 0.0 {
            if q <= 0.0 {
                return None;
            }
        } else {
            let t = q / p;
            if p < 0.0 {
                t0 = t0.max(t);
            } else {
                t1 = t1.min(t);
            }
        }
    }
    (t0 < t1).then(|| {
        let t = 0.5 * (t0 + t1);
        (x0 + t * dx, y0 + t * dy)
    })
}

/// Where a box of a face lies against its trims.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Region {
    Inside,
    Outside,
    Crossed,
}

#[derive(Debug, Clone)]
struct Face {
    kind: Kind,
    trims: Option<Trims>,
    periods: (Option<f64>, Option<f64>),
}

impl Face {
    fn new(face: &BrepFace) -> Result<(Self, Vec<Patch>), String> {
        let b = face.uv_bounds;
        if b.iter().any(|x| !x.is_finite()) || !(b[0] < b[1] && b[2] < b[3]) {
            return Err("the parameter box is empty or not finite".to_owned());
        }
        let kind = match &face.geometry {
            Surface::Plane { frame } => {
                check_frame(frame)?;
                Kind::Analytic(Analytic::Plane, *frame)
            }
            Surface::Cylinder { frame, radius } => {
                check_frame(frame)?;
                Kind::Analytic(
                    Analytic::Cylinder {
                        r: positive(*radius, "radius")?,
                    },
                    *frame,
                )
            }
            Surface::Cone {
                frame,
                radius,
                semi_angle,
            } => {
                check_frame(frame)?;
                if !radius.is_finite()
                    || semi_angle.abs().partial_cmp(&FRAC_PI_2) != Some(Ordering::Less)
                {
                    return Err("the cone's radius or angle is not usable".to_owned());
                }
                Kind::Analytic(
                    Analytic::Cone {
                        r: *radius,
                        sin: semi_angle.sin(),
                        cos: semi_angle.cos(),
                    },
                    *frame,
                )
            }
            Surface::Sphere { frame, radius } => {
                check_frame(frame)?;
                Kind::Analytic(
                    Analytic::Sphere {
                        r: positive(*radius, "radius")?,
                    },
                    *frame,
                )
            }
            Surface::Torus {
                frame,
                major,
                minor,
            } => {
                check_frame(frame)?;
                Kind::Analytic(
                    Analytic::Torus {
                        major: positive(*major, "major radius")?,
                        minor: positive(*minor, "minor radius")?,
                    },
                    *frame,
                )
            }
            Surface::BSpline(p) => Kind::Nurbs(Box::new(Nurbs::new(p)?)),
            _ => return Err("the surface kind is not supported".to_owned()),
        };
        let periods = match &kind {
            Kind::Analytic(a, _) => a.periods(),
            Kind::Nurbs(_) => (None, None),
        };
        let out = Self {
            kind,
            trims: Trims::new(face)?,
            periods,
        };
        let roots = out.roots(b)?;
        Ok((out, roots))
    }

    /// The boxes the face starts from: knot spans, or angles of at most 90
    /// degrees.
    fn roots(&self, b: [f64; 4]) -> Result<Vec<Patch>, String> {
        let mut out = Vec::new();
        match &self.kind {
            Kind::Nurbs(n) => {
                let d = n.domain();
                let c = [
                    b[0].max(d[0]),
                    b[1].min(d[1]),
                    b[2].max(d[2]),
                    b[3].min(d[3]),
                ];
                if !(c[0] < c[1] && c[2] < c[3]) {
                    return Err("the parameter box lies outside the patch's domain".to_owned());
                }
                for (su, u0, u1) in n.u.spans(c[0], c[1]) {
                    for (sv, v0, v1) in n.v.spans(c[2], c[3]) {
                        out.push(Patch::Face {
                            face: 0,
                            b: [u0, u1, v0, v1],
                            span: (su as u32, sv as u32),
                            region: Region::Crossed,
                        });
                    }
                }
            }
            Kind::Analytic(a, _) => {
                let (au, av) = a.angles();
                let pieces = |lo: f64, hi: f64, angle: bool| -> Vec<(f64, f64)> {
                    let n = if angle {
                        ((hi - lo) / FRAC_PI_2).ceil().clamp(1.0, 1e6) as usize
                    } else {
                        1
                    };
                    (0..n)
                        .map(|i| {
                            (
                                lo + (hi - lo) * i as f64 / n as f64,
                                if i + 1 == n {
                                    hi
                                } else {
                                    lo + (hi - lo) * (i + 1) as f64 / n as f64
                                },
                            )
                        })
                        .collect()
                };
                for (u0, u1) in pieces(b[0], b[1], au) {
                    for (v0, v1) in pieces(b[2], b[3], av) {
                        out.push(Patch::Face {
                            face: 0,
                            b: [u0, u1, v0, v1],
                            span: (0, 0),
                            region: Region::Crossed,
                        });
                    }
                }
            }
        }
        Ok(out)
    }

    fn shifts(&self) -> Vec<(f64, f64)> {
        let ks = |p: Option<f64>| match p {
            Some(p) => vec![0.0, -p, p],
            None => vec![0.0],
        };
        let (us, vs) = (ks(self.periods.0), ks(self.periods.1));
        us.iter()
            .flat_map(|u| vs.iter().map(move |v| (*u, *v)))
            .collect()
    }

    /// Whether (u, v) is on the face: inside the trims, at itself or one
    /// period either side.
    fn contains(&self, u: f64, v: f64) -> bool {
        let Some(t) = &self.trims else {
            return true;
        };
        self.shifts()
            .into_iter()
            .any(|(su, sv)| t.odd(u + su, v + sv))
    }

    fn region(&self, b: [f64; 4]) -> Region {
        let Some(t) = &self.trims else {
            return Region::Inside;
        };
        let (cu, cv) = (0.5 * (b[0] + b[1]), 0.5 * (b[2] + b[3]));
        let mut crossed = false;
        for (su, sv) in self.shifts() {
            let shifted = [b[0] + su, b[1] + su, b[2] + sv, b[3] + sv];
            if t.crossing(shifted).is_some() {
                crossed = true;
            } else if t.odd(cu + su, cv + sv) {
                return Region::Inside;
            }
        }
        if crossed {
            Region::Crossed
        } else {
            Region::Outside
        }
    }

    /// A point of the face in the box, for a crossed box whose centre is
    /// off the face: just beside a trim segment that crosses the box, on
    /// the side the face is. A segment can cross a box without bounding the
    /// face there (where it runs through a hole or along the box's edge),
    /// so the point is tested, never assumed.
    fn on_trim(&self, b: [f64; 4]) -> Option<(f64, f64)> {
        let t = self.trims.as_ref()?;
        let delta = 1e-7 * (b[1] - b[0]).max(b[3] - b[2]);
        for (su, sv) in self.shifts() {
            let shifted = [b[0] + su, b[1] + su, b[2] + sv, b[3] + sv];
            for s in t.near(shifted[2], shifted[3]) {
                let Some((mu, mv)) = clip(*s, shifted) else {
                    continue;
                };
                let (du, dv) = (s[1].0 - s[0].0, s[1].1 - s[0].1);
                let len = du.hypot(dv);
                if len.partial_cmp(&0.0) != Some(Ordering::Greater) {
                    continue;
                }
                for side in [1.0, -1.0] {
                    let (u, v) = (
                        mu - su - side * delta * dv / len,
                        mv - sv + side * delta * du / len,
                    );
                    if u >= b[0] && u <= b[1] && v >= b[2] && v <= b[3] && self.contains(u, v) {
                        return Some((u, v));
                    }
                }
            }
        }
        None
    }

    fn point(&self, span: (u32, u32), u: f64, v: f64) -> Point3 {
        match &self.kind {
            Kind::Analytic(a, f) => a.point(f, u, v),
            Kind::Nurbs(n) => n.point(span.0 as usize, span.1 as usize, u, v),
        }
    }

    fn hull(&self, span: (u32, u32), b: [f64; 4]) -> Vec<Point3> {
        match &self.kind {
            Kind::Analytic(a, f) => a.hull(f, b),
            Kind::Nurbs(n) => n
                .net(span.0 as usize, span.1 as usize, b)
                .into_iter()
                .flatten()
                .map(project)
                .collect(),
        }
    }

    /// Whether to split the box across u (rather than v): along its longer
    /// side.
    fn split_u(&self, span: (u32, u32), b: [f64; 4]) -> bool {
        match &self.kind {
            Kind::Analytic(a, _) => {
                let (eu, ev) = a.extents(b);
                eu >= ev
            }
            Kind::Nurbs(n) => {
                let net: Vec<Vec<Point3>> = n
                    .net(span.0 as usize, span.1 as usize, b)
                    .into_iter()
                    .map(|row| row.into_iter().map(project).collect())
                    .collect();
                let polyline = |pts: &mut dyn Iterator<Item = Point3>| {
                    let mut last: Option<Point3> = None;
                    let mut len = 0.0;
                    for p in pts {
                        if let Some(l) = last {
                            len += l.distance(p);
                        }
                        last = Some(p);
                    }
                    len
                };
                let cols = net.first().map_or(0, Vec::len);
                let eu = (0..cols)
                    .map(|j| polyline(&mut net.iter().filter_map(|r| r.get(j).copied())))
                    .fold(0.0, f64::max);
                let ev = net
                    .iter()
                    .map(|r| polyline(&mut r.iter().copied()))
                    .fold(0.0, f64::max);
                eu >= ev
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Pieces and the search.

#[derive(Debug, Clone)]
enum Patch {
    Face {
        face: u32,
        b: [f64; 4],
        span: (u32, u32),
        region: Region,
    },
    Tri([Point3; 3]),
}

/// A piece ready for the search: its hull, the point at its centre, and a
/// point of the face in it to read a clearance at.
#[derive(Debug, Clone)]
struct Ready {
    patch: Patch,
    hull: Vec<Point3>,
    centre: Point3,
    /// A point of the geometry in the piece, when the centre is not one.
    on: Option<Point3>,
    centre_on: bool,
}

#[derive(Debug, Clone)]
struct Leaf {
    ready: Ready,
    middle: Point3,
    radius: f64,
}

#[derive(Debug, Clone)]
struct Node {
    lo: Point3,
    hi: Point3,
    /// Children, or leaves `first..first + count` when `count > 0`.
    left: u32,
    right: u32,
    first: u32,
    count: u32,
}

/// Faces or a tessellation, prepared for bounded checks.
#[derive(Debug, Clone)]
pub struct Geometry {
    faces: Vec<Face>,
    leaves: Vec<Leaf>,
    nodes: Vec<Node>,
}

#[derive(Debug)]
enum Item {
    Node(u32),
    Piece(Box<Ready>),
}

#[derive(Debug)]
struct Entry {
    lower: f64,
    seq: u64,
    item: Item,
}

impl PartialEq for Entry {
    fn eq(&self, o: &Self) -> bool {
        self.cmp(o) == Ordering::Equal
    }
}
impl Eq for Entry {}
impl PartialOrd for Entry {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Entry {
    // The heap pops the smallest lower bound first, ties in order of entry.
    fn cmp(&self, o: &Self) -> Ordering {
        o.lower
            .total_cmp(&self.lower)
            .then_with(|| o.seq.cmp(&self.seq))
    }
}

impl Geometry {
    /// Every face of `model`, bounded as described in the module notes.
    /// Fails, naming the face, when one cannot be given a proven hull.
    pub fn of_faces(model: &BrepModel) -> Result<Self, ContactError> {
        let mut faces = Vec::new();
        let mut pieces = Vec::new();
        for (i, f) in model.faces.iter().enumerate() {
            let (face, roots) = Face::new(f).map_err(|reason| ContactError::Face {
                face: f.id.0,
                reason,
            })?;
            faces.push(face);
            for r in roots {
                if let Patch::Face { b, span, .. } = r {
                    pieces.push(Patch::Face {
                        face: i as u32,
                        b,
                        span,
                        region: Region::Crossed,
                    });
                }
            }
        }
        Self::build(faces, pieces)
    }

    /// The triangles of `mesh`.
    pub fn of_mesh(mesh: &TriangleMesh) -> Result<Self, ContactError> {
        let mut pieces = Vec::with_capacity(mesh.triangles.len());
        for t in &mesh.triangles {
            let mut p = [Point3::ORIGIN; 3];
            for (slot, i) in p.iter_mut().zip(t) {
                *slot = mesh
                    .vertices
                    .get(*i as usize)
                    .copied()
                    .filter(|v| v.is_finite())
                    .ok_or_else(|| {
                        ContactError::Invalid(format!("triangle {t:?} has no finite corner {i}"))
                    })?;
            }
            pieces.push(Patch::Tri(p));
        }
        Self::build(Vec::new(), pieces)
    }

    fn build(faces: Vec<Face>, roots: Vec<Patch>) -> Result<Self, ContactError> {
        let mut g = Self {
            faces,
            leaves: Vec::new(),
            nodes: Vec::new(),
        };
        let mut stack: Vec<Patch> = roots;
        stack.reverse();
        while let Some(p) = stack.pop() {
            let Some(ready) = g.ready(p) else {
                continue;
            };
            let middle = centroid(&ready.hull);
            let radius = ready
                .hull
                .iter()
                .map(|q| q.distance(middle))
                .fold(0.0, f64::max);
            if radius > LEAF
                && let Some((a, b)) = g.split(&ready.patch)
            {
                stack.push(b);
                stack.push(a);
                continue;
            }
            g.leaves.push(Leaf {
                ready,
                middle,
                radius,
            });
        }
        let mut order: Vec<u32> = (0..g.leaves.len() as u32).collect();
        if !order.is_empty() {
            let n = order.len();
            g.tree(&mut order, 0, n);
        }
        // Leaves in tree order, so each node's leaves are contiguous.
        let leaves = std::mem::take(&mut g.leaves);
        let mut slots: Vec<Option<Leaf>> = leaves.into_iter().map(Some).collect();
        g.leaves = order
            .iter()
            .filter_map(|i| slots.get_mut(*i as usize).and_then(Option::take))
            .collect();
        Ok(g)
    }

    /// Builds the hierarchy over `order[a..b]`, returning the node's index.
    fn tree(&mut self, order: &mut [u32], a: usize, b: usize) -> u32 {
        let (mut lo, mut hi) = (
            Point3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY),
            Point3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY),
        );
        for i in order.get(a..b).unwrap_or(&[]) {
            if let Some(l) = self.leaves.get(*i as usize) {
                let r = l.radius;
                lo = Point3::new(
                    lo.x.min(l.middle.x - r),
                    lo.y.min(l.middle.y - r),
                    lo.z.min(l.middle.z - r),
                );
                hi = Point3::new(
                    hi.x.max(l.middle.x + r),
                    hi.y.max(l.middle.y + r),
                    hi.z.max(l.middle.z + r),
                );
            }
        }
        let index = self.nodes.len() as u32;
        self.nodes.push(Node {
            lo,
            hi,
            left: 0,
            right: 0,
            first: a as u32,
            count: (b - a) as u32,
        });
        if b - a > 4 {
            let d = hi - lo;
            let key = |p: Point3| {
                if d.x >= d.y && d.x >= d.z {
                    p.x
                } else if d.y >= d.z {
                    p.y
                } else {
                    p.z
                }
            };
            let leaves = &self.leaves;
            if let Some(slice) = order.get_mut(a..b) {
                slice.sort_by(|x, y| {
                    let px = leaves.get(*x as usize).map_or(0.0, |l| key(l.middle));
                    let py = leaves.get(*y as usize).map_or(0.0, |l| key(l.middle));
                    px.total_cmp(&py).then(x.cmp(y))
                });
            }
            let m = a + (b - a) / 2;
            let left = self.tree(order, a, m);
            let right = self.tree(order, m, b);
            if let Some(n) = self.nodes.get_mut(index as usize) {
                n.left = left;
                n.right = right;
                n.count = 0;
            }
        }
        index
    }

    /// The hull and reading points of a piece; `None` for a piece off the
    /// face.
    fn ready(&self, patch: Patch) -> Option<Ready> {
        match patch {
            Patch::Tri(p) => Some(Ready {
                centre: centroid(&p),
                hull: p.to_vec(),
                patch,
                on: None,
                centre_on: true,
            }),
            Patch::Face {
                face,
                b,
                span,
                region,
            } => {
                let f = self.faces.get(face as usize)?;
                // A piece of a piece wholly on the face is wholly on it.
                let region = if region == Region::Inside {
                    region
                } else {
                    f.region(b)
                };
                if region == Region::Outside {
                    return None;
                }
                let (cu, cv) = (0.5 * (b[0] + b[1]), 0.5 * (b[2] + b[3]));
                let centre = f.point(span, cu, cv);
                let centre_on = region == Region::Inside || f.contains(cu, cv);
                let on = if centre_on {
                    None
                } else {
                    f.on_trim(b).map(|(u, v)| f.point(span, u, v))
                };
                Some(Ready {
                    hull: f.hull(span, b),
                    centre,
                    on,
                    centre_on,
                    patch: Patch::Face {
                        face,
                        b,
                        span,
                        region,
                    },
                })
            }
        }
    }

    fn split(&self, patch: &Patch) -> Option<(Patch, Patch)> {
        match patch {
            Patch::Tri([a, b, c]) => {
                let (ab, bc, ca) = (a.distance(*b), b.distance(*c), c.distance(*a));
                let mid = |p: Point3, q: Point3| p + (q - p) * 0.5;
                Some(if ab >= bc && ab >= ca {
                    let m = mid(*a, *b);
                    (Patch::Tri([*a, m, *c]), Patch::Tri([m, *b, *c]))
                } else if bc >= ca {
                    let m = mid(*b, *c);
                    (Patch::Tri([*a, *b, m]), Patch::Tri([*a, m, *c]))
                } else {
                    let m = mid(*c, *a);
                    (Patch::Tri([*a, *b, m]), Patch::Tri([m, *b, *c]))
                })
            }
            Patch::Face {
                face,
                b,
                span,
                region,
            } => {
                let f = self.faces.get(*face as usize)?;
                let along_u = f.split_u(*span, *b);
                let (x, y) = if along_u {
                    let m = 0.5 * (b[0] + b[1]);
                    ([b[0], m, b[2], b[3]], [m, b[1], b[2], b[3]])
                } else {
                    let m = 0.5 * (b[2] + b[3]);
                    ([b[0], b[1], b[2], m], [b[0], b[1], m, b[3]])
                };
                let degenerate = |q: [f64; 4]| !(q[0] < q[1] && q[2] < q[3]);
                if degenerate(x) || degenerate(y) {
                    return None;
                }
                let child = |q| Patch::Face {
                    face: *face,
                    b: q,
                    span: *span,
                    region: *region,
                };
                Some((child(x), child(y)))
            }
        }
    }

    /// The bounded smallest clearance between `tool`, its tip at `tip` and
    /// its axis along `axis` (tip to spindle), and this geometry.
    pub fn clearance(
        &self,
        tool: &ConvexTool,
        tip: Point3,
        axis: Vec3,
        settings: &Settings,
    ) -> Result<Bounded, ContactError> {
        let Settings {
            margin,
            tolerance,
            budget,
        } = *settings;
        if !(tolerance.is_finite() && tolerance > 0.0) || !(margin.is_finite() && margin > 0.0) {
            return Err(ContactError::Invalid(format!(
                "tolerance {tolerance} and margin {margin} must be positive and finite"
            )));
        }
        let posed = Posed::new(tool, tip, axis)?;
        let mut upper = margin;
        let mut at = None;
        let mut floor = f64::INFINITY;
        let mut heap = BinaryHeap::new();
        let mut seq = 0_u64;
        let mut cells = 0_usize;
        if !self.nodes.is_empty() {
            heap.push(Entry {
                lower: f64::NEG_INFINITY,
                seq,
                item: Item::Node(0),
            });
        }
        while let Some(entry) = heap.pop() {
            let threshold = upper - tolerance;
            if entry.lower >= threshold {
                floor = floor.min(entry.lower);
                break;
            }
            cells += 1;
            if cells > budget {
                return Err(ContactError::Budget {
                    cells,
                    lower: floor.min(entry.lower),
                    upper,
                });
            }
            let mut push = |lower: f64, item: Item, upper: f64, floor: &mut f64| {
                if lower >= upper - tolerance {
                    *floor = floor.min(lower);
                } else {
                    seq += 1;
                    heap.push(Entry { lower, seq, item });
                }
            };
            match entry.item {
                Item::Node(i) => {
                    let Some(n) = self.nodes.get(i as usize) else {
                        continue;
                    };
                    if n.count == 0 {
                        for c in [n.left, n.right] {
                            if let Some(child) = self.nodes.get(c as usize) {
                                let lower = posed.bound_box(child.lo, child.hi);
                                push(lower, Item::Node(c), upper, &mut floor);
                            }
                        }
                    } else {
                        let (a, b) = (n.first as usize, (n.first + n.count) as usize);
                        for leaf in self.leaves.get(a..b).unwrap_or(&[]) {
                            let lower = posed.bound_box(
                                leaf.middle + Vec3::new(-leaf.radius, -leaf.radius, -leaf.radius),
                                leaf.middle + Vec3::new(leaf.radius, leaf.radius, leaf.radius),
                            );
                            if lower >= upper - tolerance {
                                floor = floor.min(lower);
                                continue;
                            }
                            let (lower, value) = posed.bound(leaf.ready.centre, &leaf.ready.hull);
                            self.read(&posed, &leaf.ready, value, &mut upper, &mut at);
                            push(
                                lower,
                                Item::Piece(Box::new(leaf.ready.clone())),
                                upper,
                                &mut floor,
                            );
                        }
                    }
                }
                Item::Piece(ready) => {
                    let Some((a, b)) = self.split(&ready.patch) else {
                        // Too small to split: what it bounds stays open.
                        return Err(ContactError::Budget {
                            cells,
                            lower: floor.min(entry.lower),
                            upper,
                        });
                    };
                    for child in [a, b] {
                        let Some(r) = self.ready(child) else {
                            continue;
                        };
                        let (lower, value) = posed.bound(r.centre, &r.hull);
                        self.read(&posed, &r, value, &mut upper, &mut at);
                        push(lower, Item::Piece(Box::new(r)), upper, &mut floor);
                    }
                }
            }
        }
        let lower = floor.min(upper);
        Ok(Bounded {
            upper,
            lower,
            at,
            cells,
        })
    }

    /// Reads the clearance at the piece's point of the geometry, keeping
    /// the smallest.
    fn read(
        &self,
        posed: &Posed<'_>,
        ready: &Ready,
        centre_value: f64,
        upper: &mut f64,
        at: &mut Option<Point3>,
    ) {
        let (value, point) = if ready.centre_on {
            (centre_value, ready.centre)
        } else if let Some(p) = ready.on {
            (posed.at(p).value, p)
        } else {
            return;
        };
        if value < *upper {
            *upper = value;
            *at = Some(point);
        }
    }

    /// How many pieces the check starts from, after the split before any
    /// pose.
    pub fn leaves(&self) -> usize {
        self.leaves.len()
    }

    /// The number of faces.
    pub fn faces(&self) -> usize {
        self.faces.len()
    }
}

fn centroid(p: &[Point3]) -> Point3 {
    let n = p.len().max(1) as f64;
    let s = p.iter().fold(Vec3::ZERO, |s, q| s + q.to_vec());
    Point3::new(s.x / n, s.y / n, s.z / n)
}
