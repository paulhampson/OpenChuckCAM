// SPDX-License-Identifier: AGPL-3.0-or-later
//! Exact drops of a tool outline other than a ball (lane STEPALL-X3, issue
//! #350; OI-81; decision 0338): the tapered ball of axial finishing, the flat
//! end of pencil clean-up, and any other outline of the same family.
//!
//! The tool's lower envelope is `h(rho)`, its height above the tip at `rho`
//! from the axis, out to its radius `R`. Moving along a tool line, the tool
//! first holds a point `p` at the tip offset `lift(p) = z - h(rho)` (`z`
//! along the axis from the line's origin, `rho` from the axis), and never
//! when `rho > R`. The drop is the largest `lift` over the faces, found by
//! the branch and bound of the ball drop (the `drop` module, decision 0277):
//! every point the search evaluates gives a real contact, and every part it
//! passes over has a proven upper bound no higher than the best found plus
//! [`DROP_TOLERANCE`].
//!
//! **Which outlines.** `h` is convex and never falls (a convex profile under
//! the monotone rule), so `lift` is a concave function of `p` on the
//! cylinder `rho <= R`. The envelope must be lines of finite slope and arcs
//! centred on the axis (a ball at the tip) that end before they turn
//! upright, and must leave the axis level (no point at the tip). That holds
//! for the flat end, the tapered ball (and the tapered ball grown for stock
//! to leave), the ball's own tip, and a V-bit or chamfer with a flat at the
//! tip. A bull nose (an arc off the axis, upright at the rim), a flat end
//! grown for stock to leave (which is a bull nose), a sharp V-bit and a
//! concave custom profile have no outline drop here; the ball keeps the
//! ball drop.
//!
//! **Bounds.** A concave function lies under each of its tangent planes, so
//! over a box its largest value is no more than the tangent plane's largest
//! value over the box. The rim `rho <= R` is taken in with a multiplier: the
//! distance from the axis is convex, so `rho(p) >= rho(q) + e . (p - q)` for
//! its gradient `e` at `q`, and for any `lambda >= 0` the bound may add
//! `lambda (R - rho(q) - e . (p - q))`, which is not negative inside the
//! rim. The bound is tight to the second order in the box's size, as the
//! ball's is. A box is a cell's boxes (a B-spline face's hull cells, or a
//! box about a piece of an analytic face), or the box about a piece of a
//! trim loop's image.
//!
//! **Where the highest contact can be.** Inside a face, the lift is
//! greatest where the tool's surface is tangent to the face (the face's
//! normal along the tool's normal at that radius, which turns from the axis
//! by `atan h'(rho)`), or on a circle where the outline has an edge (the
//! rim, or a joint where the slope jumps), where the face's normal has no
//! part around the axis. A cell whose normal cone allows neither is passed
//! over: its face's highest contact is then elsewhere, on its boundary
//! (searched along the loops), or on a crease of the patch (a cell along
//! one is always searched). This is the normal-cone test of decision 0277,
//! for the outline.

use std::collections::BinaryHeap;

use openchuck_types::geometry::{Point3, Vec3};
use openchuck_types::tool::profile::{ProfilePiece, RevolvedProfile};

use super::drop::{DROP_TOLERANCE, Hit};
use super::hull::{HullTree, Net, Para};
use super::trim::Segment;
use super::{
    Analytic, Cell, ExactFace, Geometry, Nurbs, Pending, Quad, SEARCH_CELLS, SEARCH_DEPTH,
};
use crate::cutter::{TIP_TIE, ToolLine};
use crate::surface::{ParametricFace, Uv};

/// One piece of the lower envelope, over `[r0, r1]`.
#[derive(Debug, Clone, Copy)]
enum Piece {
    /// `h = z0 + k (rho - r0)`.
    Line { z0: f64, k: f64 },
    /// A ball about the axis: `h = c - sqrt(q^2 - rho^2)`.
    Ball { c: f64, q: f64 },
}

#[derive(Debug, Clone, Copy)]
struct Span {
    r0: f64,
    r1: f64,
    piece: Piece,
}

impl Span {
    fn h(&self, rho: f64) -> f64 {
        match self.piece {
            Piece::Line { z0, k } => z0 + k * (rho - self.r0),
            Piece::Ball { c, q } => c - (q * q - rho * rho).max(0.0).sqrt(),
        }
    }

    fn slope(&self, rho: f64) -> f64 {
        match self.piece {
            Piece::Line { k, .. } => k,
            Piece::Ball { q, .. } => {
                let rho = rho.max(0.0);
                rho / (q * q - rho * rho).max(f64::MIN_POSITIVE).sqrt()
            }
        }
    }
}

/// A tool outline with an exact drop: its lower envelope piece by piece.
#[derive(Debug, Clone)]
pub(super) struct Outline {
    spans: Vec<Span>,
    /// The radius `R` where the envelope ends.
    radius: f64,
    /// The radii of the outline's edge circles: joints where the slope
    /// jumps, and the rim.
    edges: Vec<f64>,
}

/// Radii and heights closer than this, relative to the tool's size, are the
/// same.
const TIE: f64 = 1e-9;

impl Outline {
    /// The outline of `profile`, or `None` when it has no exact outline drop
    /// (see the module).
    pub fn new(profile: &RevolvedProfile) -> Option<Self> {
        if !profile.is_convex() {
            return None;
        }
        let env = profile.envelope();
        let radius = env.last()?.r_end;
        if !(radius.is_finite() && radius > 0.0) {
            return None;
        }
        let tie = TIE * (1.0 + radius);
        let mut spans = Vec::with_capacity(env.len());
        let mut at = 0.0;
        for e in env {
            if (e.r_start - at).abs() > tie || e.r_end <= e.r_start {
                return None;
            }
            let piece = match e.piece {
                ProfilePiece::Line { start, end } => {
                    let k = (end.height.0 - start.height.0) / (end.radius.0 - start.radius.0);
                    if !k.is_finite() || k < -tie {
                        return None;
                    }
                    Piece::Line {
                        z0: start.height.0 + k * (e.r_start - start.radius.0),
                        k: k.max(0.0),
                    }
                }
                ProfilePiece::Arc {
                    centre,
                    radius: q,
                    convex: true,
                    ..
                } => {
                    // A ball about the axis that ends before it turns upright.
                    if centre.radius.0.abs() > tie || !(e.r_end < q * (1.0 - 1e-9)) {
                        return None;
                    }
                    Piece::Ball {
                        c: centre.height.0,
                        q,
                    }
                }
                ProfilePiece::Arc { .. } => return None,
            };
            spans.push(Span {
                r0: e.r_start,
                r1: e.r_end,
                piece,
            });
            at = e.r_end;
        }
        // Level at the axis: no point at the tip.
        let first = spans.first()?;
        if first.slope(first.r0) > tie || first.h(first.r0).abs() > tie {
            return None;
        }
        let mut edges = Vec::new();
        for w in spans.windows(2) {
            if let [a, b] = w {
                if (a.h(a.r1) - b.h(b.r0)).abs() > tie {
                    return None;
                }
                if (a.slope(a.r1) - b.slope(b.r0)).abs() > 1e-9 * (1.0 + a.slope(a.r1)) {
                    edges.push(a.r1);
                }
            }
        }
        edges.push(radius);
        Some(Self {
            spans,
            radius,
            edges,
        })
    }

    fn span(&self, rho: f64) -> Option<&Span> {
        self.spans
            .iter()
            .find(|s| rho <= s.r1)
            .or_else(|| self.spans.last())
    }

    /// The radius, widened by rounding: a point found on the rim may lie a
    /// last bit outside it.
    fn reach(&self) -> f64 {
        self.radius * (1.0 + 1e-12) + 1e-12
    }

    /// `h(rho)`, or `None` beyond the radius.
    fn h(&self, rho: f64) -> Option<f64> {
        if !(rho <= self.reach()) {
            return None;
        }
        let rho = rho.clamp(0.0, self.radius);
        self.span(rho).map(|s| s.h(rho))
    }

    /// The slope at `rho` from the side of the axis (a supergradient of the
    /// lift at a joint).
    fn slope_in(&self, rho: f64) -> f64 {
        let rho = rho.clamp(0.0, self.radius);
        self.span(rho).map_or(0.0, |s| s.slope(rho))
    }

    /// The slope at `rho` from the side of the rim.
    fn slope_out(&self, rho: f64) -> f64 {
        let rho = rho.clamp(0.0, self.radius);
        self.spans
            .iter()
            .find(|s| rho < s.r1)
            .or_else(|| self.spans.last())
            .map_or(0.0, |s| s.slope(rho))
    }
}

/// The tool line as the search reads it.
#[derive(Debug, Clone, Copy)]
struct Line {
    o: Point3,
    a: Vec3,
}

impl Line {
    /// Along the axis, and the part across it.
    fn split(&self, p: Point3) -> (f64, Vec3) {
        let w = p - self.o;
        let z = w.dot(self.a);
        (z, w - self.a * z)
    }
}

/// A cone of directions: a unit axis and a half angle.
#[derive(Debug, Clone, Copy)]
struct Cone {
    axis: Vec3,
    angle: f64,
}

/// One drop's search over the faces.
#[derive(Debug)]
pub(super) struct Search<'t> {
    tool: &'t Outline,
    line: Line,
    /// Two unit vectors across the axis.
    across: (Vec3, Vec3),
    pub best: Option<Hit>,
    /// The highest bound left unresolved by a search that ran out of its
    /// budget or reached its smallest piece.
    pub unresolved: f64,
    /// Pieces and cells looked at, over all faces.
    pub looked: usize,
    /// A face with no exact drop (a B-spline face without its cells).
    pub unsupported: bool,
}

impl<'t> Search<'t> {
    pub fn new(tool: &'t Outline, line: &ToolLine) -> Self {
        let a = line.axis.get();
        let seed = if a.x.abs() < 0.9 {
            Vec3::new(1.0, 0.0, 0.0)
        } else {
            Vec3::new(0.0, 1.0, 0.0)
        };
        let e1 = seed - a * seed.dot(a);
        let e1 = e1 * (1.0 / e1.length());
        Self {
            tool,
            line: Line { o: line.origin, a },
            across: (e1, a.cross(e1)),
            best: None,
            unresolved: f64::NEG_INFINITY,
            looked: 0,
            unsupported: false,
        }
    }

    /// [`Self::top`], for the unit tests.
    #[cfg(test)]
    pub fn top_for_test(&self, b: &Para) -> Option<f64> {
        self.top(b)
    }

    /// The lift of a point, for the unit tests.
    #[cfg(test)]
    pub fn lift_for_test(&self, p: Point3) -> Option<f64> {
        self.lift(p)
    }

    /// What a part must beat to matter.
    pub fn floor(&self) -> f64 {
        self.best
            .map_or(f64::NEG_INFINITY, |b| b.s + DROP_TOLERANCE)
    }

    /// The lift of a point the tool really holds: within the radius
    /// itself, not its rounding (the bounds take the rounding).
    fn lift(&self, p: Point3) -> Option<f64> {
        let (z, perp) = self.line.split(p);
        let rho = perp.length();
        if !(rho <= self.tool.radius) {
            return None;
        }
        self.tool.h(rho).map(|h| z - h)
    }

    fn offer(&mut self, p: Point3, face: u32, uv: Uv) {
        if let Some(s) = self.lift(p)
            && s.is_finite()
            && self.best.is_none_or(|b| s > b.s + TIP_TIE)
        {
            self.best = Some(Hit {
                s,
                point: p,
                face,
                uv,
            });
        }
    }

    fn leave(&mut self, top: f64) {
        if top > self.floor() {
            self.unresolved = self.unresolved.max(top);
        }
    }

    /// The distance from the axis at the box's middle, and a bound on how
    /// far the box reaches across the axis from it.
    fn spread(&self, b: &Para) -> (f64, Vec3, f64) {
        let (_, perp) = self.line.split(b.centre);
        let reach = b
            .axes
            .iter()
            .zip(b.half)
            .map(|(e, h)| {
                let along = e.dot(self.line.a);
                h * (1.0 - along * along).max(0.0).sqrt()
            })
            .sum::<f64>();
        (perp.length(), perp, reach)
    }

    /// An upper bound on the tip offset at which the tool can reach a point
    /// of the box, or `None` when it cannot (see the module).
    fn top(&self, b: &Para) -> Option<f64> {
        let tool = self.tool;
        let a = self.line.a;
        let (rho_c, perp, reach) = self.spread(b);
        let rho_min = (rho_c - reach).max(0.0);
        if !(rho_min <= tool.reach()) {
            return None;
        }
        let (zc, _) = self.line.split(b.centre);
        let z_max = zc
            + b.axes
                .iter()
                .zip(b.half)
                .map(|(e, h)| h * e.dot(a).abs())
                .sum::<f64>();
        // First order: the highest point at the nearest radius.
        let first = z_max - tool.h(rho_min.min(tool.radius))?;
        let size = b.half.iter().sum::<f64>();
        let dual = self.dual_bound(b, rho_c, perp);
        Some(first.min(dual) + 1e-12 * (1.0 + zc.abs() + rho_c + size + tool.radius))
    }

    /// `h*(kappa)`: the largest `kappa rho - h(rho)` over the outline,
    /// `rho` from 0 to the radius (the conjugate of the envelope).
    fn conjugate(&self, kappa: f64) -> f64 {
        self.tool
            .spans
            .iter()
            .map(|s| {
                let rho = match s.piece {
                    Piece::Line { k, .. } => {
                        if kappa >= k {
                            s.r1
                        } else {
                            s.r0
                        }
                    }
                    Piece::Ball { q, .. } => {
                        (kappa * q / (1.0 + kappa * kappa).sqrt()).clamp(s.r0, s.r1)
                    }
                };
                kappa * rho - s.h(rho)
            })
            .fold(f64::NEG_INFINITY, f64::max)
    }

    /// The dual bound at `v`, a vector across the axis: for every point `p`
    /// of the box within the radius, `lift(p) <= h*(|v|) + (a - v) . (p - o)`
    /// (as `rho |v| >= -v . (p - o)` across the axis, and
    /// `h(rho) >= |v| rho - h*(|v|)`), so the largest value of the right side
    /// over the box bounds the lift there. It is a convex function of `v`,
    /// and its least value is the largest lift over the box (the minimax
    /// theorem): the bound is exact for the box.
    fn dual_at(&self, b: &Para, v: Vec3) -> f64 {
        let d = self.line.a - v;
        self.conjugate(v.length())
            + d.dot(b.centre - self.line.o)
            + b.axes
                .iter()
                .zip(b.half)
                .map(|(x, h)| h * d.dot(*x).abs())
                .sum::<f64>()
    }

    /// The least dual bound found by a short pattern search over `v`, from
    /// the tangent plane at the box's middle: every value it takes is a
    /// bound, so the search only tightens it.
    fn dual_bound(&self, b: &Para, rho_c: f64, perp: Vec3) -> f64 {
        let tool = self.tool;
        let a = self.line.a;
        let rho = rho_c.min(tool.radius);
        let e = if rho_c > 1e-300 {
            perp * (1.0 / rho_c)
        } else {
            Vec3::ZERO
        };
        // From the tangent plane at the middle (moved inside the rim), with
        // the multipliers that cancel the slope along each axis of the box.
        let k = tool.slope_in(rho);
        let mut best = (self.dual_at(b, e * k), e * k);
        for x in &b.axes {
            let ex = e.dot(*x);
            if ex.abs() > 1e-15 {
                let lambda = (a - e * k).dot(*x) / ex;
                if lambda > 0.0 && lambda.is_finite() {
                    let v = e * (k + lambda);
                    let f = self.dual_at(b, v);
                    if f < best.0 {
                        best = (f, v);
                    }
                }
            }
        }
        // The bound is the conjugate of |v| plus a piecewise linear function
        // of v, whose creases are the lines where `(a - v) . x` changes sign
        // for an axis `x` of the box. Its least value is where it is
        // stationary inside a piece (the slope of the conjugate's radius
        // along the piece's gradient), on a crease, or where two creases
        // cross; each is tried.
        let (p1, p2) = self.across;
        let to = |x: f64, y: f64| p1 * x + p2 * y;
        let w0 = b.centre - self.line.o;
        let cc = (w0.dot(p1), w0.dot(p2));
        let creases: Vec<((f64, f64), f64, f64)> = b
            .axes
            .iter()
            .zip(b.half)
            .map(|(x, h)| ((x.dot(p1), x.dot(p2)), a.dot(*x), h))
            .filter(|((dx, dy), _, h)| dx * dx + dy * dy > 1e-24 && *h > 0.0)
            .collect();
        let take = |v: Vec3, best: &mut (f64, Vec3)| {
            let f = self.dual_at(b, v);
            if f < best.0 {
                *best = (f, v);
            }
        };
        take(Vec3::ZERO, &mut best);
        let n = creases.len();
        for signs in 0..(1u32 << n) {
            let (mut wx, mut wy) = cc;
            for (j, ((dx, dy), _, h)) in creases.iter().enumerate() {
                let sg = if signs & (1 << j) == 0 { 1.0 } else { -1.0 };
                wx += sg * h * dx;
                wy += sg * h * dy;
            }
            let lw = wx.hypot(wy);
            if lw > 1e-300 {
                let r = lw.min(tool.radius);
                for k in [tool.slope_in(r), tool.slope_out(r)] {
                    take(to(wx * (k / lw), wy * (k / lw)), &mut best);
                }
            }
        }
        for (i, ((ax, ay), aa, _)) in creases.iter().enumerate() {
            for ((bx, by), ba, _) in creases.iter().skip(i + 1) {
                let det = ax * by - ay * bx;
                if det.abs() > 1e-12 {
                    let x = (aa * by - ay * ba) / det;
                    let y = (ax * ba - aa * bx) / det;
                    take(to(x, y), &mut best);
                }
            }
        }
        let k_rim = tool.slope_out(tool.radius);
        for ((dx, dy), al, _) in &creases {
            let l2 = dx * dx + dy * dy;
            let (x0, y0) = (al * dx / l2, al * dy / l2);
            let l = l2.sqrt();
            let (tx, ty) = (-dy / l, dx / l);
            let span = 4.0 * (k_rim + 1.0) + 4.0 * x0.hypot(y0);
            let along = |t: f64| self.dual_at(b, to(x0 + t * tx, y0 + t * ty));
            let (mut lo, mut hi) = (-span, span);
            let g = 0.381_966_011_250_105_1;
            let (mut m1, mut m2) = (lo + g * (hi - lo), hi - g * (hi - lo));
            let (mut f1, mut f2) = (along(m1), along(m2));
            for _ in 0..48 {
                if f1 <= f2 {
                    hi = m2;
                    (m2, f2) = (m1, f1);
                    m1 = lo + g * (hi - lo);
                    f1 = along(m1);
                } else {
                    lo = m1;
                    (m1, f1) = (m2, f2);
                    m2 = hi - g * (hi - lo);
                    f2 = along(m2);
                }
            }
            let t = if f1 <= f2 { m1 } else { m2 };
            take(to(x0 + t * tx, y0 + t * ty), &mut best);
        }
        best.0
    }

    /// Whether the box can hold a point where the face's highest contact
    /// lies inside it, given a cone that holds the face's normals there
    /// (see the module). True when there is no cone.
    fn may_hold(&self, b: &Para, cone: Option<Cone>) -> bool {
        let Some(cone) = cone else {
            return true;
        };
        let theta = cone.angle + 1e-9;
        if theta >= std::f64::consts::FRAC_PI_2 {
            return true;
        }
        let tool = self.tool;
        let a = self.line.a;
        let (rho_c, perp, reach) = self.spread(b);
        let rho_lo = (rho_c - reach).max(0.0);
        let rho_hi = (rho_c + reach).min(tool.radius);
        if rho_lo > tool.reach() {
            return false;
        }
        // Around the axis: how far the box turns about it from its middle.
        let turn = if rho_c > reach && rho_c > 0.0 {
            (reach / rho_c).min(1.0).asin()
        } else {
            std::f64::consts::PI
        };
        let (er, et) = if rho_c > 1e-300 {
            let er = perp * (1.0 / rho_c);
            (er, a.cross(er))
        } else {
            (Vec3::ZERO, Vec3::ZERO)
        };
        // Tangency: the tool's normal at radius rho and angle phi about the
        // axis is sin(beta) e_rho(phi) - cos(beta) a, beta = atan(h').
        let b0 = tool.slope_in(rho_lo).atan();
        let b1 = tool.slope_out(rho_hi).atan();
        for m in [cone.axis, -cone.axis] {
            let beta = (-m.dot(a)).clamp(-1.0, 1.0).acos();
            let polar = (b0 - beta).max(beta - b1).max(0.0);
            let mut lune = 0.0;
            if turn < std::f64::consts::PI {
                let (x, y) = (m.dot(er), m.dot(et));
                if x * x + y * y > 1e-24 {
                    let gap = y.abs().atan2(x) - turn;
                    if gap >= std::f64::consts::FRAC_PI_2 {
                        lune = beta.min(std::f64::consts::PI - beta);
                    } else if gap > 0.0 {
                        lune = (beta.sin() * gap.sin()).clamp(-1.0, 1.0).asin();
                    }
                }
            }
            if polar.max(lune) <= theta {
                return true;
            }
        }
        // An edge circle: the face's normal has no part about the axis.
        let tie = TIE * (1.0 + tool.radius);
        tool.edges
            .iter()
            .any(|&r| r >= rho_lo - tie && r <= (rho_c + reach) + tie)
            && (turn >= std::f64::consts::FRAC_PI_2 - theta
                || cone.axis.dot(et).abs().min(1.0).asin() <= theta + turn)
    }
}

/// The box that holds every point within `q` of the chord from `a` to `b`.
fn capsule(a: Point3, b: Point3, q: f64) -> Para {
    let d = b - a;
    let l = d.length();
    let centre = a + d * 0.5;
    if !(l > 1e-300) {
        return Para::cube(centre, q + 0.5 * l);
    }
    let e1 = d * (1.0 / l);
    let seed = if e1.x.abs() < 0.9 {
        Vec3::new(1.0, 0.0, 0.0)
    } else {
        Vec3::new(0.0, 1.0, 0.0)
    };
    let e2 = seed - e1 * seed.dot(e1);
    let e2 = e2 * (1.0 / e2.length());
    let e3 = e1.cross(e2);
    // The axes are unit only to rounding: widen by it.
    let w = 1e-12 * (1.0 + l + q + centre.to_vec().length());
    Para {
        centre,
        axes: [e1, e2, e3],
        half: [0.5 * l + q + w, q + w, q + w],
    }
}

/// A piece of a loop segment: its parameter range, the images of its ends
/// and middle.
type Piece3 = (f64, f64, Point3, Point3, Point3);

/// An analytic cell: its parameter box and the splits left.
type QuadCell = ([f64; 4], usize);

impl ExactFace {
    /// An upper bound on the tip offset at which the tool can reach the
    /// face (from its box), or `None` when it cannot.
    pub(super) fn outline_top(&self, search: &Search<'_>) -> Option<f64> {
        search.top(&Para::of_aabb(&self.bounding_box()))
    }

    /// Drops the outline onto this face, improving `search`.
    pub(super) fn drop_outline(&self, search: &mut Search<'_>) {
        self.outline_boundary(search);
        match (&self.geometry, &self.hull) {
            (Geometry::Quad(a), _) => self.outline_quad(a, search),
            (Geometry::Nurbs(n, _), Some(tree)) => self.outline_nurbs(n, tree, search),
            (Geometry::Nurbs(..), None) => search.unsupported = true,
        }
    }

    fn outline_boundary(&self, search: &mut Search<'_>) {
        let floor = search.floor();
        let mut order: Vec<(f64, usize)> = Vec::new();
        for &(c, r, first, end) in &self.trim.chunks {
            if search.top(&Para::cube(c, r)).is_none_or(|t| t <= floor) {
                continue;
            }
            for i in first..end {
                if let Some(seg) = self.trim.segments.get(i)
                    && let Some(t) = search.top(&Para::cube(seg.centre, seg.radius))
                    && t > floor
                {
                    order.push((t, i));
                }
            }
        }
        order.sort_by(|x, y| y.0.total_cmp(&x.0).then(x.1.cmp(&y.1)));
        for (top, i) in order {
            if top <= search.floor() {
                break;
            }
            if let Some(seg) = self.trim.segments.get(i) {
                self.outline_segment(seg, search);
            }
        }
    }

    /// The branch and bound over one segment's parameter.
    fn outline_segment(&self, seg: &Segment, search: &mut Search<'_>) {
        let ((px, py), (x, y)) = (seg.a, seg.b);
        let speed = 2.0 * seg.radius;
        let uv_at = |t: f64| self.clamp_search_if_nurbs(px + t * (x - px), py + t * (y - py));
        let at_t = |t: f64| {
            let (u, v) = uv_at(t);
            (
                self.eval(u, v)
                    .unwrap_or(Point3::new(f64::NAN, f64::NAN, f64::NAN)),
                Uv { u, v },
            )
        };
        let number = self.number;
        for (t, p) in [(0.0, seg.ends.0), (1.0, seg.ends.1)] {
            let (u, v) = uv_at(t);
            search.offer(p, number, Uv { u, v });
        }
        let mut heap: BinaryHeap<Pending<Piece3>> = BinaryHeap::new();
        let mut order = 0u64;
        let mut fresh = vec![(0.0, 1.0, seg.ends.0, seg.ends.1)];
        let mut looked = 0;
        loop {
            for (lo, hi, plo, phi) in fresh.drain(..) {
                let m = 0.5 * (lo + hi);
                let (pm, uv) = at_t(m);
                if !pm.is_finite() {
                    continue;
                }
                search.offer(pm, number, uv);
                let w = hi - lo;
                let floor = search.floor();
                let Some(mut top) = search.top(&Para::cube(pm, 0.5 * speed * w)) else {
                    continue;
                };
                if top > floor && seg.bend.is_finite() {
                    match search.top(&capsule(plo, phi, seg.bend * w * w / 8.0)) {
                        Some(c) => top = top.min(c),
                        None => continue,
                    }
                }
                if top <= floor {
                    continue;
                }
                // Where the image crosses an edge circle of the outline, the
                // crossing is a contact of its own.
                self.edge_crossings(search, (lo, plo), (m, pm), &at_t);
                self.edge_crossings(search, (m, pm), (hi, phi), &at_t);
                if speed * w <= DROP_TOLERANCE {
                    search.leave(top);
                    continue;
                }
                order += 1;
                heap.push(Pending {
                    lower: -top,
                    order,
                    item: (lo, hi, plo, phi, pm),
                });
            }
            let Some(Pending {
                lower,
                item: (lo, hi, plo, phi, pm),
                ..
            }) = heap.pop()
            else {
                break;
            };
            if -lower <= search.floor() {
                break;
            }
            if looked >= SEARCH_CELLS {
                search.leave(-lower);
                break;
            }
            looked += 1;
            search.looked += 1;
            let m = 0.5 * (lo + hi);
            fresh.extend([(lo, m, plo, pm), (m, hi, pm, phi)]);
        }
    }

    /// Offers the points where the loop's image between `t0` and `t1`
    /// crosses an edge circle of the outline, found by false position.
    fn edge_crossings(
        &self,
        search: &mut Search<'_>,
        (t0, p0): (f64, Point3),
        (t1, p1): (f64, Point3),
        at_t: &impl Fn(f64) -> (Point3, Uv),
    ) {
        let line = search.line;
        let rho = |p: Point3| line.split(p).1.length();
        let (r0, r1) = (rho(p0), rho(p1));
        let edges = search.tool.edges.clone();
        for r in edges {
            let (f0, f1) = (r0 - r, r1 - r);
            if !(f0 * f1 < 0.0) {
                continue;
            }
            let (mut a, mut fa, mut b, mut fb) = (t0, f0, t1, f1);
            let mut side = 0i8;
            let mut inside: Option<(Point3, Uv)> = None;
            for _ in 0..60 {
                let t = (a * fb - b * fa) / (fb - fa);
                if !t.is_finite() {
                    break;
                }
                let (p, uv) = at_t(t);
                if !p.is_finite() {
                    break;
                }
                let ft = rho(p) - r;
                if ft <= 0.0 {
                    inside = Some((p, uv));
                }
                if ft * fa > 0.0 {
                    (a, fa) = (t, ft);
                    if side == -1 {
                        fb *= 0.5;
                    }
                    side = -1;
                } else {
                    (b, fb) = (t, ft);
                    if side == 1 {
                        fa *= 0.5;
                    }
                    side = 1;
                }
                if (b - a).abs() <= 1e-14 || ft.abs() <= 1e-13 * (1.0 + r) {
                    break;
                }
            }
            if let Some((p, uv)) = inside {
                search.offer(p, self.number, uv);
            }
        }
    }

    /// The box that holds an analytic face over a parameter box, and a cone
    /// that holds its normals there (see `hull` for the B-spline's).
    fn quad_cell(&self, a: &Analytic, [u0, u1, v0, v1]: [f64; 4]) -> (Para, Option<Cone>) {
        let (uc, vc) = (0.5 * (u0 + u1), 0.5 * (v0 + v1));
        let (hu, hv) = (0.5 * (u1 - u0), 0.5 * (v1 - v0));
        let (p, su, sv) = a.d1(uc, vc);
        // Bounds on the second derivatives over the box, and how fast the
        // normal turns per unit of each parameter.
        let (muu, muv, mvv, ku, kv) = match a.quad {
            Quad::Plane => (0.0, 0.0, 0.0, 0.0, 0.0),
            Quad::Cylinder { r } => (r.abs(), 0.0, 0.0, 1.0, 0.0),
            Quad::Cone { r, a: angle } => {
                let s = angle.sin();
                let w = (r + v0 * s).abs().max((r + v1 * s).abs());
                (w, s.abs(), 0.0, 1.0, 0.0)
            }
            Quad::Sphere { r } => (r.abs(), r.abs(), r.abs(), 1.0, 1.0),
            Quad::Torus { major, minor } => (
                major.abs() + minor.abs(),
                minor.abs(),
                minor.abs(),
                1.0,
                1.0,
            ),
        };
        let dev = 0.5 * (muu * hu * hu + 2.0 * muv * hu * hv + mvv * hv * hv);
        let w = 1e-12 * (1.0 + p.to_vec().length() + su.length() * hu + sv.length() * hv);
        let n = su.cross(sv);
        let (ln, lu) = (n.length(), su.length());
        let para = if ln > 1e-12 * (1.0 + lu * sv.length()) && lu > 0.0 && ln.is_finite() {
            let e3 = n * (1.0 / ln);
            let e1 = su * (1.0 / lu);
            let e2 = e3.cross(e1);
            let axes = [e1, e2, e3];
            let half = axes.map(|e| su.dot(e).abs() * hu + sv.dot(e).abs() * hv + dev + w);
            Para {
                centre: p,
                axes,
                half,
            }
        } else {
            Para::cube(p, su.length() * hu + sv.length() * hv + dev + w)
        };
        let cone = a.normal(uc, vc).and_then(|m| {
            let angle = ku * hu + kv * hv;
            (angle < std::f64::consts::FRAC_PI_2 && m.is_finite())
                .then_some(Cone { axis: m, angle })
        });
        (para, cone)
    }

    /// The branch and bound over an analytic face's parameter box.
    fn outline_quad(&self, a: &Analytic, search: &mut Search<'_>) {
        // The apex of a cone is a point of its own, where the face has no
        // normal.
        if let Quad::Cone { r, a: angle } = a.quad {
            let (sa, ca) = angle.sin_cos();
            if sa.abs() > 1e-12 {
                let v = -r / sa;
                if v >= self.bounds[2] && v <= self.bounds[3] {
                    let p = a.ax.o + a.ax.z * (v * ca);
                    let (u, v) = a.uv_of(p);
                    search.offer(p, self.number, Uv { u, v });
                }
            }
        }
        let trimmed = !self.loops.is_empty();
        let mut heap: BinaryHeap<Pending<QuadCell>> = BinaryHeap::new();
        let mut order = 0u64;
        let root = self.bounds;
        let (para, _) = self.quad_cell(a, root);
        if let Some(top) = search.top(&para) {
            heap.push(Pending {
                lower: -top,
                order,
                item: (root, SEARCH_DEPTH),
            });
        }
        let mut looked = 0;
        let mut settled: Vec<(f64, f64)> = Vec::new();
        while let Some(Pending {
            lower,
            item: (uv, depth),
            ..
        }) = heap.pop()
        {
            let top = -lower;
            if top <= search.floor() {
                break;
            }
            if looked >= SEARCH_CELLS {
                search.leave(top);
                break;
            }
            looked += 1;
            search.looked += 1;
            if trimmed && self.trim.box_side(uv) == Some(false) {
                continue;
            }
            let (para, cone) = self.quad_cell(a, uv);
            if !search.may_hold(&para, cone) {
                continue;
            }
            self.cell_contacts(search, uv, &para, top, &mut settled);
            let size = para.half.iter().sum::<f64>();
            if depth == 0 || size <= 0.5 * DROP_TOLERANCE {
                search.leave(top);
                continue;
            }
            let [u0, u1, v0, v1] = uv;
            let (_, su, sv) = a.d1(0.5 * (u0 + u1), 0.5 * (v0 + v1));
            let halves = if su.length() * (u1 - u0) >= sv.length() * (v1 - v0) {
                let m = 0.5 * (u0 + u1);
                [[u0, m, v0, v1], [m, u1, v0, v1]]
            } else {
                let m = 0.5 * (v0 + v1);
                [[u0, u1, v0, m], [u0, u1, m, v1]]
            };
            for half in halves {
                let (p, _) = self.quad_cell(a, half);
                let floor = search.floor();
                if let Some(t) = search.top(&p)
                    && t > floor
                {
                    order += 1;
                    heap.push(Pending {
                        lower: -t,
                        order,
                        item: (half, depth - 1),
                    });
                }
            }
        }
    }

    /// The branch and bound over a B-spline face's cells.
    fn outline_nurbs(&self, n: &Nurbs, tree: &HullTree, search: &mut Search<'_>) {
        let trimmed = !self.loops.is_empty();
        let mut heap = BinaryHeap::new();
        let mut order = 0u64;
        let bound_top = |s: &Search<'_>, b: &super::hull::Bound| {
            let (x, y) = b.paras();
            let t = s.top(&x)?;
            Some(match y.map(|y| s.top(&y)) {
                Some(None) => return None,
                Some(Some(u)) => t.min(u),
                None => t,
            })
        };
        if let Some(root) = tree.nodes.get(tree.root as usize)
            && let Some(top) = bound_top(search, &root.bound)
        {
            heap.push(Pending {
                lower: -top,
                order,
                item: Cell::Stored(tree.root),
            });
        }
        let mut looked = 0;
        let mut settled: Vec<(f64, f64)> = Vec::new();
        while let Some(Pending {
            lower, item: cell, ..
        }) = heap.pop()
        {
            let top = -lower;
            if top <= search.floor() {
                break;
            }
            if looked >= SEARCH_CELLS {
                search.leave(top);
                break;
            }
            looked += 1;
            search.looked += 1;
            let (net, depth, size): (std::borrow::Cow<'_, Net>, usize, f64) = match cell {
                Cell::Stored(i) => {
                    let Some(node) = tree.nodes.get(i as usize) else {
                        continue;
                    };
                    if trimmed && self.trim.box_side(node.uv) == Some(false) {
                        continue;
                    }
                    if let Some((a, b)) = node.children {
                        for c in [a, b] {
                            if let Some(child) = tree.nodes.get(c as usize)
                                && let Some(t) = bound_top(search, &child.bound)
                                && t > search.floor()
                            {
                                order += 1;
                                heap.push(Pending {
                                    lower: -t,
                                    order,
                                    item: Cell::Stored(c),
                                });
                            }
                        }
                        continue;
                    }
                    let Some(net) = node.net.and_then(|k| tree.nets.get(k as usize)) else {
                        continue;
                    };
                    (
                        std::borrow::Cow::Borrowed(net),
                        SEARCH_DEPTH,
                        node.bound.size(),
                    )
                }
                Cell::Split(net, depth, size) => {
                    if trimmed && self.trim.box_side(net.uv) == Some(false) {
                        continue;
                    }
                    (std::borrow::Cow::Owned(net), depth, size)
                }
            };
            let (aligned, along) = net.bound().paras();
            let para = along.unwrap_or(aligned);
            if !n.touches_crease(net.uv) {
                let cone = net.normal_cone().map(|(axis, sin_half)| Cone {
                    axis,
                    angle: 2.0 * sin_half.min(1.0).asin(),
                });
                if !search.may_hold(&para, cone) {
                    continue;
                }
            }
            self.cell_contacts(search, net.uv, &para, top, &mut settled);
            if depth == 0 || size <= DROP_TOLERANCE {
                search.leave(top);
                continue;
            }
            let (a, b) = net.split(net.longer_in_u());
            for half in [a, b] {
                let hb = half.bound();
                let floor = search.floor();
                if let Some(t) = bound_top(search, &hb)
                    && t > floor
                {
                    order += 1;
                    heap.push(Pending {
                        lower: -t,
                        order,
                        item: Cell::Split(half, depth - 1, hb.size()),
                    });
                }
            }
        }
    }

    /// The contacts a cell offers: its middle; where the outline has an edge
    /// circle across the cell, the middle moved onto it; and, from a middle
    /// that beats the best so far, a climb to where the tool rests on the
    /// face.
    fn cell_contacts(
        &self,
        search: &mut Search<'_>,
        [u0, u1, v0, v1]: [f64; 4],
        para: &Para,
        top: f64,
        settled: &mut Vec<(f64, f64)>,
    ) {
        let (cu, cv) = (0.5 * (u0 + u1), 0.5 * (v0 + v1));
        let Some(middle) = self.eval(cu, cv) else {
            return;
        };
        if !middle.is_finite() {
            return;
        }
        let inside = self.contains(Uv { u: cu, v: cv });
        let gain = inside
            && search
                .lift(middle)
                .is_some_and(|s| search.best.is_none_or(|b| s > b.s));
        if inside {
            search.offer(middle, self.number, Uv { u: cu, v: cv });
        }
        let (rho_c, _, reach) = search.spread(para);
        let edges = search.tool.edges.clone();
        for r in edges {
            if r >= rho_c - reach && r <= rho_c + reach {
                self.onto_edge(search, (cu, cv), r, [u0, u1, v0, v1]);
            }
        }
        let tried = settled
            .iter()
            .any(|&(u, v)| u >= u0 && u <= u1 && v >= v0 && v <= v1);
        if gain && !tried && top > search.floor() {
            settled.push(self.climb(search, cu, cv));
        }
    }

    /// Moves (u, v) onto the circle of radius `r` about the axis by Newton
    /// steps on the distance from the axis, and offers the point when it
    /// stays near the cell and inside the face.
    fn onto_edge(&self, search: &mut Search<'_>, (mut u, mut v): (f64, f64), r: f64, b: [f64; 4]) {
        let [u0, u1, v0, v1] = b;
        let (wu, wv) = (u1 - u0, v1 - v0);
        // A hair inside the circle, so that rounding keeps the point within
        // the rim (far below the tolerance).
        let r = r - 1e-12 * (1.0 + r);
        for _ in 0..8 {
            let Some((p, su, sv)) = self.d1(u, v) else {
                return;
            };
            let (_, perp) = search.line.split(p);
            let rho = perp.length();
            if !(rho > 1e-300) {
                return;
            }
            let e = perp * (1.0 / rho);
            // e is across the axis, so it reads the change of rho directly.
            let (gu, gv) = (e.dot(su), e.dot(sv));
            let g2 = gu * gu + gv * gv;
            if !(g2 > 1e-300) {
                return;
            }
            let k = (r - rho) / g2;
            let (nu, nv) = self.clamp_search_if_nurbs(u + k * gu, v + k * gv);
            if !(nu.is_finite() && nv.is_finite())
                || nu < u0 - wu
                || nu > u1 + wu
                || nv < v0 - wv
                || nv > v1 + wv
            {
                return;
            }
            (u, v) = (nu, nv);
            if (rho - r).abs() <= 1e-13 * (1.0 + r) {
                break;
            }
        }
        if let Some(p) = self.eval(u, v)
            && p.is_finite()
            && self.contains(Uv { u, v })
        {
            // Only points within the rim are contacts; a last bit outside
            // is within its rounding.
            search.offer(p, self.number, Uv { u, v });
        }
    }

    /// Newton steps on the lift over (u, v) from (u, v), its second
    /// derivatives taken by differences; offers each point inside the face
    /// and returns where it ended.
    fn climb(&self, search: &mut Search<'_>, mut u: f64, mut v: f64) -> (f64, f64) {
        let tool = search.tool;
        let line = search.line;
        let grad = |u: f64, v: f64| -> Option<(Point3, f64, f64)> {
            let (p, su, sv) = self.d1(u, v)?;
            let (_, perp) = line.split(p);
            let rho = perp.length();
            if !(rho <= tool.radius) {
                return None;
            }
            let g = if rho > 1e-300 {
                line.a - perp * (tool.slope_in(rho) / rho)
            } else {
                line.a
            };
            Some((p, g.dot(su), g.dot(sv)))
        };
        let span =
            (self.bounds[1] - self.bounds[0]).abs() + (self.bounds[3] - self.bounds[2]).abs();
        let h = 1e-7 * (1.0 + span);
        for _ in 0..6 {
            let Some((p, gu, gv)) = grad(u, v) else { break };
            if self.contains(Uv { u, v }) {
                search.offer(p, self.number, Uv { u, v });
            }
            let (Some((_, guu, gvu)), Some((_, guv, gvv))) = (grad(u + h, v), grad(u, v + h))
            else {
                break;
            };
            let (a11, a12, a21, a22) = (
                (guu - gu) / h,
                (guv - gu) / h,
                (gvu - gv) / h,
                (gvv - gv) / h,
            );
            let off = 0.5 * (a12 + a21);
            let det = a11 * a22 - off * off;
            // A maximum needs the second derivatives negative definite.
            if !(a11 < 0.0 && det > 0.0) {
                break;
            }
            let du = -(a22 * gu - off * gv) / det;
            let dv = -(a11 * gv - off * gu) / det;
            let (nu, nv) = self.clamp_search_if_nurbs(u + du, v + dv);
            if !(nu.is_finite() && nv.is_finite()) {
                break;
            }
            let moved = (nu - u).abs() + (nv - v).abs();
            (u, v) = (nu, nv);
            if moved <= 1e-14 * (1.0 + span) {
                break;
            }
        }
        if let Some(p) = self.eval(u, v)
            && self.contains(Uv { u, v })
        {
            search.offer(p, self.number, Uv { u, v });
        }
        (u, v)
    }
}
