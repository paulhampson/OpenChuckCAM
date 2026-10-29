// SPDX-License-Identifier: AGPL-3.0-or-later
//! Exact ball drops on the faces (lane STEPALL-X2, issue #349; OI-81).
//!
//! A ball of radius `r` moves along a tool line from the spindle side; its
//! centre is `o + s a` when its tip is at offset `s` along the line. A point
//! `p` of a face is first held by the ball at the highest such offset,
//! `lift(p) = z + sqrt(r^2 - rho^2)` (`z` along the axis from `o`, `rho` from
//! the axis), and never when `rho > r`. The drop is the largest `lift` over
//! the faces. Every point the search evaluates gives a real contact, so the
//! best found is never above the answer; every bound it prunes with is an
//! upper bound on what the pruned part can give, so the answer is never more
//! than [`DROP_TOLERANCE`] above the best found, unless a search ran out of
//! its budget, in which case the bound it stopped at is reported.
//!
//! - **Interior of an analytic face**: the largest `lift` at an inside
//!   point is a point where the ball's centre lies on the face's normal at
//!   distance `r`, so on a parallel (offset) surface: planes, cylinders,
//!   cones and spheres offset by `r` are surfaces of the same kind, a torus
//!   offset is a torus, so the centres are the roots of a quadratic or a
//!   quartic along the line. Each root's foot is kept when it lies inside
//!   the trims; a cone's apex is a point of its own.
//! - **Interior of a B-spline face**: a branch and bound over the face's
//!   hull cells. The distance from a point to a cell's boxes is convex, so
//!   along the line the highest offset at which the ball reaches the boxes
//!   is found by Newton steps from above, each an upper bound; a cell whose
//!   bound is no higher than the best found plus the tolerance is passed
//!   over. A cell's middle gives a contact, and a few Newton steps of the
//!   contact condition from it tighten the best.
//! - **The boundary** (trim loops, or the parameter box): the same branch and
//!   bound over each segment's parameter, with the ball and the capsule that
//!   hold a piece of the segment's image (the `trim` module).

use std::collections::BinaryHeap;

use openchuck_types::geometry::{Point3, Vec3};

use super::hull::{Bound, HullTree, Net};
use super::trim::Segment;
use super::{Analytic, Cell, ExactFace, Geometry, Nurbs, Pending, Quad, SEARCH_CELLS, SEARCH_DEPTH};
use crate::cutter::{TIP_TIE, ToolLine};
use crate::surface::{ParametricFace, Uv};

/// Debug counters (stage 1 only): convex_top calls, iterations, split cells.
pub static DEBUG: [std::sync::atomic::AtomicUsize; 3] = [
    std::sync::atomic::AtomicUsize::new(0),
    std::sync::atomic::AtomicUsize::new(0),
    std::sync::atomic::AtomicUsize::new(0),
];

fn bump(i: usize, n: usize) {
    if let Some(c) = DEBUG.get(i) {
        c.fetch_add(n, std::sync::atomic::Ordering::Relaxed);
    }
}

/// The exact drop answers within this, mm, of the highest contact.
pub const DROP_TOLERANCE: f64 = 1e-4;

/// A ball along a tool line.
#[derive(Debug, Clone, Copy)]
pub(super) struct Ball {
    /// The centre when the tip is at the line's origin.
    o: Point3,
    a: Vec3,
    r: f64,
}

impl Ball {
    pub fn new(line: &ToolLine, r: f64) -> Self {
        let a = line.axis.get();
        Self {
            o: line.origin + a * r,
            a,
            r,
        }
    }

    fn centre(&self, s: f64) -> Point3 {
        self.o + self.a * s
    }

    /// The highest tip offset at which a ball of radius `q` about the line
    /// holds `m`, or `None` when `m` lies further than `q` from the axis.
    fn top_within(&self, m: Point3, q: f64) -> Option<f64> {
        let w = m - self.o;
        let z = w.dot(self.a);
        let rho2 = (w.dot(w) - z * z).max(0.0);
        let qq = q * q;
        (rho2 <= qq).then(|| z + (qq - rho2).sqrt())
    }

    /// `lift(p)`: the highest tip offset at which the ball holds `p`.
    fn lift(&self, p: Point3) -> Option<f64> {
        self.top_within(p, self.r)
    }

    /// The highest tip offset, no higher than `start`, at which a convex set
    /// comes within the ball, given the distance to the set and its gradient;
    /// `None` when it never does. Newton steps from above on a convex
    /// function never pass its largest root, so every step is an upper
    /// bound; the search stops at the first that is no higher than `floor`
    /// (the caller prunes it).
    fn convex_top(
        &self,
        start: f64,
        floor: f64,
        dist: impl Fn(Point3) -> (f64, Vec3),
    ) -> Option<f64> {
        let mut s = start;
        bump(0, 1);
        for _ in 0..24 {
            bump(1, 1);
            if s <= floor {
                return Some(s);
            }
            let (d, g) = dist(self.centre(s));
            if d <= self.r {
                return Some(s);
            }
            let slope = g.dot(self.a);
            if !(slope > 0.0) {
                // The distance falls (or stays) below `s` and is above the
                // radius here: it is above it everywhere below.
                return None;
            }
            let next = s - (d - self.r) / slope;
            if !next.is_finite() {
                return None;
            }
            if s - next <= 0.25 * DROP_TOLERANCE {
                return Some(s);
            }
            s = next;
        }
        Some(s)
    }

    /// An upper bound on the tip offset at which the ball can reach a cell
    /// held by `bound`, or `None` when it cannot.
    fn cell_top(&self, bound: &Bound, floor: f64) -> Option<f64> {
        let b = &bound.aabb;
        let mid = Point3::new(
            0.5 * (b.min.x + b.max.x),
            0.5 * (b.min.y + b.max.y),
            0.5 * (b.min.z + b.max.z),
        );
        let start = self.top_within(mid, self.r + 0.5 * bound.size())?;
        self.convex_top(start, floor, |p| bound.distance_gradient(p))
    }

    /// An upper bound on the tip offset at which the ball can reach a point
    /// within `q` of the chord from `a` to `b`.
    fn capsule_top(&self, a: Point3, b: Point3, q: f64, floor: f64) -> Option<f64> {
        let mid = a + (b - a) * 0.5;
        let start = self.top_within(mid, self.r + q + 0.5 * a.distance(b))?;
        let ball = Self { r: self.r + q, ..*self };
        ball.convex_top(start, floor, |p| {
            let d = b - a;
            let l2 = d.dot(d);
            let t = if l2 > 0.0 {
                ((p - a).dot(d) / l2).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let w = p - (a + d * t);
            let l = w.length();
            if l > 0.0 { (l, w * (1.0 / l)) } else { (0.0, Vec3::ZERO) }
        })
    }
}

/// The best contact so far.
#[derive(Debug, Clone, Copy)]
pub(super) struct Hit {
    pub s: f64,
    pub point: Point3,
    pub face: u32,
    pub uv: Uv,
}

/// One drop's search over the faces.
#[derive(Debug)]
pub(super) struct Search {
    pub ball: Ball,
    pub best: Option<Hit>,
    /// The highest bound left unresolved by a search that ran out of its
    /// budget or reached its smallest piece.
    pub unresolved: f64,
    /// Pieces and cells looked at, over all faces.
    pub looked: usize,
    /// A face with no exact drop (a B-spline face searched by its grid).
    pub unsupported: bool,
    /// Debug: boundary pieces, segments ordered, cells, contact solves.
    pub stats: [usize; 4],
}

impl Search {
    pub fn new(ball: Ball) -> Self {
        Self {
            ball,
            best: None,
            unresolved: f64::NEG_INFINITY,
            looked: 0,
            unsupported: false,
            stats: [0; 4],
        }
    }

    /// What a part must beat to matter.
    pub fn floor(&self) -> f64 {
        self.best.map_or(f64::NEG_INFINITY, |b| b.s + DROP_TOLERANCE)
    }

    fn offer(&mut self, p: Point3, face: u32, uv: Uv) {
        if let Some(s) = self.ball.lift(p)
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
}

/// A piece of a loop segment: its parameter range, the images of its ends
/// and middle.
type Piece = (f64, f64, Point3, Point3, Point3);

impl ExactFace {
    /// An upper bound on the tip offset at which the ball can reach the face
    /// (from its box), or `None` when it cannot.
    pub(super) fn drop_top(&self, ball: &Ball) -> Option<f64> {
        let b = self.bounding_box();
        let bound = Bound::from_aabb(b);
        ball.cell_top(&bound, f64::NEG_INFINITY)
    }

    /// Drops the ball onto this face, improving `search`.
    pub(super) fn drop_ball(&self, search: &mut Search) {
        self.boundary_drop(search);
        match (&self.geometry, &self.hull) {
            (Geometry::Quad(a), _) => self.quad_drop(a, search),
            (Geometry::Nurbs(n, _), Some(tree)) => self.nurbs_drop(n, tree, search),
            (Geometry::Nurbs(..), None) => search.unsupported = true,
        }
    }

    fn boundary_drop(&self, search: &mut Search) {
        let ball = search.ball;
        let floor = search.floor();
        let mut order: Vec<(f64, usize)> = Vec::new();
        for &(c, r, first, end) in &self.trim.chunks {
            if ball.top_within(c, ball.r + r).is_none_or(|t| t <= floor) {
                continue;
            }
            for i in first..end {
                if let Some(seg) = self.trim.segments.get(i)
                    && let Some(t) = ball.top_within(seg.centre, ball.r + seg.radius)
                    && t > floor
                {
                    order.push((t, i));
                }
            }
        }
        order.sort_by(|x, y| y.0.total_cmp(&x.0).then(x.1.cmp(&y.1)));
        search.stats[1] += self.trim.segments.len();
        for (top, i) in order {
            if top <= search.floor() {
                break;
            }
            if let Some(seg) = self.trim.segments.get(i) {
                self.segment_drop(seg, search);
            }
        }
    }

    /// The branch and bound over one segment's parameter (see the module).
    fn segment_drop(&self, seg: &Segment, search: &mut Search) {
        let ball = search.ball;
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
        let mut heap: BinaryHeap<Pending<Piece>> = BinaryHeap::new();
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
                let Some(mut top) = ball.top_within(pm, ball.r + 0.5 * speed * w) else {
                    continue;
                };
                if top > floor && seg.bend.is_finite() {
                    match ball.capsule_top(plo, phi, seg.bend * w * w / 8.0, floor) {
                        Some(c) => top = top.min(c),
                        None => continue,
                    }
                }
                if top <= floor {
                    continue;
                }
                if speed * w <= DROP_TOLERANCE {
                    search.leave(top);
                    continue;
                }
                order += 1;
                // The heap is nearest-first on `lower`: highest first is the
                // negated bound.
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
            search.stats[0] += 1;
            let m = 0.5 * (lo + hi);
            fresh.extend([(lo, m, plo, pm), (m, hi, pm, phi)]);
        }
    }

    /// The interior candidates of an analytic face (see the module).
    fn quad_drop(&self, a: &Analytic, search: &mut Search) {
        let ball = search.ball;
        let ax = &a.ax;
        let r = ball.r;
        // The line of centres in the face's frame.
        let (px, py, pz) = ax.local(ball.o);
        let (dx, dy, dz) = (ball.a.dot(ax.x), ball.a.dot(ax.y), ball.a.dot(ax.z));
        let mut ss: Vec<f64> = Vec::new();
        let mut extra: Vec<Point3> = Vec::new();
        match a.quad {
            Quad::Plane => {
                if dz.abs() > 1e-14 {
                    ss.push((r - pz) / dz);
                    ss.push((-r - pz) / dz);
                }
            }
            Quad::Cylinder { r: c } => {
                for k in [c + r, c - r] {
                    if k > 0.0 {
                        super::quad_roots(
                            dx * dx + dy * dy,
                            2.0 * (px * dx + py * dy),
                            px * px + py * py - k * k,
                            &mut ss,
                        );
                    }
                }
            }
            Quad::Sphere { r: c } => {
                for k in [c + r, c - r] {
                    if k > 0.0 {
                        super::quad_roots(
                            1.0,
                            2.0 * (px * dx + py * dy + pz * dz),
                            px * px + py * py + pz * pz - k * k,
                            &mut ss,
                        );
                    }
                }
            }
            Quad::Cone { r: c, a: angle } => {
                // In the meridian plane the cone is the line
                // rho cos(a) - z sin(a) = c cos(a) (rho signed across the
                // axis); the centres at distance r from it satisfy
                // rho^2 cos^2(a) = (z sin(a) + c cos(a) +- r)^2.
                let (sa, ca) = angle.sin_cos();
                for k in [r, -r] {
                    let e0 = pz * sa + c * ca + k;
                    let e1 = dz * sa;
                    super::quad_roots(
                        (dx * dx + dy * dy) * ca * ca - e1 * e1,
                        2.0 * ((px * dx + py * dy) * ca * ca - e0 * e1),
                        (px * px + py * py) * ca * ca - e0 * e0,
                        &mut ss,
                    );
                }
                // The apex, where the face reaches it.
                if sa.abs() > 1e-12 {
                    let v = -c / sa;
                    if v >= self.bounds[2] && v <= self.bounds[3] {
                        extra.push(ax.o + ax.z * (v * ca));
                    }
                }
            }
            Quad::Torus { major, minor } => {
                for k in [minor + r, minor - r] {
                    if k > 0.0 {
                        let t = Analytic {
                            ax: a.ax,
                            quad: Quad::Torus { major, minor: k },
                        };
                        ss.extend(t.ray_ts(ball.o, ball.a));
                    }
                }
            }
        }
        for s in ss {
            if !s.is_finite() {
                continue;
            }
            let c = ball.centre(s);
            // The foot of the centre on the untrimmed surface.
            let (cands, _) = a.project(c, self.scale);
            for (u, v) in cands {
                let (u, v) = self.wrapped(Uv { u, v });
                let Some(p) = self.eval(u, v) else { continue };
                if (p.distance(c) - r).abs() <= 1e-7 * (1.0 + r) && self.contains(Uv { u, v }) {
                    search.offer(p, self.number, Uv { u, v });
                }
            }
        }
        for p in extra {
            let (u, v) = a.uv_of(p);
            search.offer(p, self.number, Uv { u, v });
        }
    }

    /// The branch and bound over a B-spline face's cells (see the module).
    fn nurbs_drop(&self, n: &Nurbs, tree: &HullTree, search: &mut Search) {
        let ball = search.ball;
        let trimmed = !self.loops.is_empty();
        let mut heap = BinaryHeap::new();
        let mut order = 0u64;
        if let Some(root) = tree.nodes.get(tree.root as usize)
            && let Some(top) = ball.cell_top(&root.bound, search.floor())
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
            search.stats[2] += 1;
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
                                && let Some(t) = ball.cell_top(&child.bound, search.floor())
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
                    (std::borrow::Cow::Borrowed(net), SEARCH_DEPTH, node.bound.size())
                }
                Cell::Split(net, depth, size) => {
                    bump(2, 1);
                    if trimmed && self.trim.box_side(net.uv) == Some(false) {
                        continue;
                    }
                    (std::borrow::Cow::Owned(net), depth, size)
                }
            };
            // A cell where the ball cannot rest tangentially on the surface
            // holds no interior maximum of the lift: the face's maximum is
            // at such a point or on its boundary, which the boundary search
            // has settled.
            if !self.may_rest_on(&net, &ball) {
                search.stats[3] += 1;
                continue;
            }
            let [u0, u1, v0, v1] = net.uv;
            let (cu, cv) = (0.5 * (u0 + u1), 0.5 * (v0 + v1));
            let middle = net.centre();
            let inside = self.contains(Uv { u: cu, v: cv });
            if inside {
                // A contact solve only tightens the best, so it is spent
                // where it can: from a middle that beats the best so far, in
                // a cell where no earlier solve ended.
                let gain = ball
                    .lift(middle)
                    .is_some_and(|s| search.best.is_none_or(|b| s > b.s));
                search.offer(middle, self.number, Uv { u: cu, v: cv });
                let tried = settled
                    .iter()
                    .any(|&(u, v)| u >= u0 && u <= u1 && v >= v0 && v <= v1);
                if gain && !tried && top > search.floor() {
                    let end = self.nurbs_contact(n, cu, cv, search);
                    settled.push(end);
                }
            }
            if depth == 0 || size <= DROP_TOLERANCE {
                search.leave(top);
                continue;
            }
            let (a, b) = net.split(net.longer_in_u());
            for half in [a, b] {
                let bound = half.bound();
                let floor = search.floor();
                if let Some(t) = ball.cell_top(&bound, floor)
                    && t > floor
                {
                    order += 1;
                    heap.push(Pending {
                        lower: -t,
                        order,
                        item: Cell::Split(half, depth - 1, bound.size()),
                    });
                }
            }
        }
    }

    /// Whether the ball can rest tangentially on the surface of `net` with
    /// its centre on the line: false when the line keeps away from every
    /// point `p + r n` and `p - r n`, `p` in the net's box and `n` in its
    /// normal cone (the `hull` module). True when there is no cone.
    fn may_rest_on(&self, net: &Net, ball: &Ball) -> bool {
        let Some((m, sin_half)) = net.normal_cone() else {
            return true;
        };
        let b = net.bound().aabb;
        let spread = 2.0 * ball.r * sin_half;
        let size = (b.max - b.min).length() + b.max.to_vec().length() + ball.r;
        let margin = 1e-9 * (1.0 + size);
        [1.0, -1.0].into_iter().any(|k| {
            let shift = m * (k * ball.r);
            let moved = openchuck_types::geometry::Aabb {
                min: b.min + shift,
                max: b.max + shift,
            };
            super::line_box_distance(&moved, ball.o, ball.a) <= spread + margin
        })
    }

    /// Newton steps of the contact condition from (u, v): the ball's centre
    /// moves along the line by the gap over the rate at which the gap
    /// changes, and the face point nearest the centre gives a contact each
    /// step. Returns where it ended.
    fn nurbs_contact(&self, n: &Nurbs, u: f64, v: f64, search: &mut Search) -> (f64, f64) {
        let ball = search.ball;
        let Some(p) = self.eval(u, v) else {
            return (u, v);
        };
        let mut s = ball.lift(p).unwrap_or_else(|| (p - ball.o).dot(ball.a));
        let (mut u, mut v) = (u, v);
        for _ in 0..8 {
            let c = ball.centre(s);
            let (nu, nv, d) = self.lm_closest(n, c, u, v);
            (u, v) = (nu, nv);
            let Some(p) = self.eval(u, v) else { break };
            if self.contains(Uv { u, v }) {
                search.offer(p, self.number, Uv { u, v });
            }
            if !(d > 0.0) {
                break;
            }
            let slope = (c - p).dot(ball.a) / d;
            if !(slope > 1e-6) {
                break;
            }
            let next = s - (d - ball.r) / slope;
            if !next.is_finite() || (next - s).abs() <= 1e-10 {
                break;
            }
            s = next;
        }
        (u, v)
    }
}
