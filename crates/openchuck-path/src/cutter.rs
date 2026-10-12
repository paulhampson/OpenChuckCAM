// SPDX-License-Identifier: AGPL-3.0-or-later
//! Tool-centre solving: the drop-cutter, generic over the tool profile
//! (decision record 0012 items 4 and 5; work breakdown slice 5).
//!
//! The tool is moved along a [`ToolLine`] from the spindle side until it
//! touches; the answer is the highest of three tests over the triangles near
//! the line, each of which asks the profile only for its lower envelope
//! `h(r)` and the envelope's pieces:
//!
//! - **vertex**: a vertex at distance `d` from the axis lifts the tip to
//!   `z - h(d)`;
//! - **facet**: along the facet's uphill direction, maximise `g s - h(s)`
//!   (a line piece peaks at an end, a convex arc where `h'(s) = g`, a concave
//!   arc at an end);
//! - **edge**: split the edge where its distance from the axis crosses a piece
//!   boundary; on each part `z(t) - h(d(t))` is concave for lines and convex
//!   arcs (a golden-section search is exact to rounding) and is sampled then
//!   refined for concave arcs.
//!
//! Distances are evaluated directly, never from an expanded quadratic, which
//! loses about 1e-8 mm near the axis (decision record 0012, findings). The
//! profile never dips below its tip (`h(r) >= 0`), so a triangle can lift the
//! tip at most to its highest vertex: triangles are visited from the highest
//! down, and the search stops once none can beat the best so far.

use openchuck_types::geometry::{Point3, UnitVec3, Vec3};
use openchuck_types::tool::profile::{EnvelopePiece, ProfilePiece, RevolvedProfile};

use crate::surface::MeshSurface;

/// The line a tool moves along: through `origin`, with the tool axis
/// `axis` pointing from the tip towards the spindle. Model frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ToolLine {
    /// A point on the tool axis.
    pub origin: Point3,
    /// The tool axis, from the tip towards the spindle.
    pub axis: UnitVec3,
}

impl ToolLine {
    /// The tip position `offset` along the axis from the origin.
    pub fn at(&self, offset: f64) -> Point3 {
        self.origin + self.axis.get() * offset
    }
}

/// Where a dropped tool touches the surface.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Contact {
    /// The tool tip, in the model frame.
    pub tip: Point3,
    /// The cutter contact point on the surface, in the model frame. The
    /// contact feed applies here (OP-13).
    pub contact: Point3,
    /// The tip's position along the line, from its origin.
    pub offset: f64,
}

/// A point in the line's own frame: `z` along the tool axis.
#[derive(Debug, Clone, Copy)]
struct Local {
    x: f64,
    y: f64,
    z: f64,
}

/// An orthonormal frame with the tool axis as its third direction.
struct Frame {
    origin: Point3,
    e1: Vec3,
    e2: Vec3,
    e3: Vec3,
}

impl Frame {
    fn new(line: &ToolLine) -> Self {
        let e3 = line.axis.get();
        // The model axis least aligned with the tool axis seeds the frame.
        let seed = if e3.x.abs() <= e3.y.abs() && e3.x.abs() <= e3.z.abs() {
            Vec3::new(1.0, 0.0, 0.0)
        } else if e3.y.abs() <= e3.z.abs() {
            Vec3::new(0.0, 1.0, 0.0)
        } else {
            Vec3::new(0.0, 0.0, 1.0)
        };
        let e1 = UnitVec3::new(seed - e3 * seed.dot(e3))
            .map(UnitVec3::get)
            .unwrap_or(Vec3::new(1.0, 0.0, 0.0));
        let e2 = e3.cross(e1);
        Self {
            origin: line.origin,
            e1,
            e2,
            e3,
        }
    }

    fn local(&self, p: Point3) -> Local {
        let w = p - self.origin;
        Local {
            x: w.dot(self.e1),
            y: w.dot(self.e2),
            z: w.dot(self.e3),
        }
    }

    fn model(&self, l: Local) -> Point3 {
        self.origin + self.e1 * l.x + self.e2 * l.y + self.e3 * l.z
    }
}

#[derive(Debug, Clone, Copy)]
struct Best {
    tip_z: f64,
    contact: Local,
}

fn better(best: &mut Option<Best>, tip_z: f64, contact: Local) {
    if tip_z.is_finite() && best.is_none_or(|b| tip_z > b.tip_z) {
        *best = Some(Best { tip_z, contact });
    }
}

/// Drops the tool along `line` onto `mesh`.
pub(crate) fn drop_on_mesh(
    mesh: &MeshSurface,
    profile: &RevolvedProfile,
    line: &ToolLine,
) -> Option<Contact> {
    let radius = profile.cutting_radius();
    let frame = Frame::new(line);
    let mut order: Vec<(f64, u32, [Local; 3])> = mesh
        .triangles_near_line(line, radius)
        .into_iter()
        .filter_map(|i| {
            let [a, b, c] = *mesh.triangle(i)?;
            let tri = [frame.local(a), frame.local(b), frame.local(c)];
            let top = tri.iter().map(|v| v.z).fold(f64::NEG_INFINITY, f64::max);
            Some((top, i, tri))
        })
        .collect();
    order.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    let mut best: Option<Best> = None;
    for (top, _, tri) in &order {
        if best.is_some_and(|b| *top <= b.tip_z) {
            break;
        }
        let [a, b, c] = *tri;
        for v in [a, b, c] {
            vertex(profile, v, &mut best);
        }
        for (p, q) in [(a, b), (b, c), (c, a)] {
            edge(profile, p, q, &mut best);
        }
        facet(profile, tri, &mut best);
    }
    best.map(|b| Contact {
        tip: line.at(b.tip_z),
        contact: frame.model(b.contact),
        offset: b.tip_z,
    })
}

/// Horizontal distance from the axis, evaluated directly.
fn radial(x: f64, y: f64) -> f64 {
    (x * x + y * y).sqrt()
}

fn vertex(profile: &RevolvedProfile, v: Local, best: &mut Option<Best>) {
    if let Some(h) = profile.height_at(radial(v.x, v.y)) {
        better(best, v.z - h, v);
    }
}

fn facet(profile: &RevolvedProfile, tri: &[Local; 3], best: &mut Option<Best>) {
    let [a, b, c] = *tri;
    let u = Vec3::new(b.x - a.x, b.y - a.y, b.z - a.z);
    let v = Vec3::new(c.x - a.x, c.y - a.y, c.z - a.z);
    let mut n = u.cross(v);
    let len = n.length();
    if len <= f64::MIN_POSITIVE {
        return;
    }
    n = n * (1.0 / len);
    if n.z < 0.0 {
        n = -n;
    }
    if n.z < 1e-12 {
        return; // A facet parallel to the axis: its edges carry the contact.
    }
    let gx = -n.x / n.z;
    let gy = -n.y / n.z;
    let g = radial(gx, gy);
    let mut best_s = 0.0;
    let mut best_f = f64::NEG_INFINITY;
    let mut consider = |s: f64, h: f64| {
        let f = g * s - h;
        if f > best_f {
            best_f = f;
            best_s = s;
        }
    };
    if g < 1e-15 {
        consider(0.0, 0.0);
    } else {
        for e in profile.envelope() {
            consider(e.r_start, e.piece.height_at(e.r_start));
            consider(e.r_end, e.piece.height_at(e.r_end));
            if let ProfilePiece::Arc {
                centre,
                radius,
                convex: true,
                ..
            } = e.piece
            {
                let s = centre.radius.0 + g * radius / (1.0 + g * g).sqrt();
                if s > e.r_start && s < e.r_end {
                    consider(s, e.piece.height_at(s));
                }
            }
        }
    }
    let (ux, uy) = if g < 1e-15 {
        (0.0, 0.0)
    } else {
        (gx / g, gy / g)
    };
    let px = best_s * ux;
    let py = best_s * uy;
    if inside(tri, px, py) {
        let pz = a.z - (n.x * (px - a.x) + n.y * (py - a.y)) / n.z;
        if let Some(h) = profile.height_at(best_s) {
            better(
                best,
                pz - h,
                Local {
                    x: px,
                    y: py,
                    z: pz,
                },
            );
        }
    }
}

fn inside(tri: &[Local; 3], px: f64, py: f64) -> bool {
    let cross = |a: Local, b: Local| (b.x - a.x) * (py - a.y) - (b.y - a.y) * (px - a.x);
    let [a, b, c] = *tri;
    let d0 = cross(a, b);
    let d1 = cross(b, c);
    let d2 = cross(c, a);
    let eps = 1e-12;
    (d0 >= -eps && d1 >= -eps && d2 >= -eps) || (d0 <= eps && d1 <= eps && d2 <= eps)
}

fn edge(profile: &RevolvedProfile, p0: Local, p1: Local, best: &mut Option<Best>) {
    let ex = p1.x - p0.x;
    let ey = p1.y - p0.y;
    let l2 = ex * ex + ey * ey;
    if l2 < 1e-24 {
        return; // An edge along the axis: its top vertex carries the contact.
    }
    let wx = p0.x;
    let wy = p0.y;
    let bq = wx * ex + wy * ey;
    let cq = wx * wx + wy * wy;
    // Where the edge's distance from the axis equals r: d(t)^2 = l2 t^2 + 2 bq t + cq.
    let roots = |r: f64| -> Option<(f64, f64)> {
        let disc = bq * bq - l2 * (cq - r * r);
        if disc < 0.0 {
            return None;
        }
        let s = disc.sqrt();
        Some(((-bq - s) / l2, (-bq + s) / l2))
    };
    let r_max = profile.cutting_radius();
    let Some((ta, tb)) = roots(r_max) else {
        return;
    };
    let ta = ta.max(0.0);
    let tb = tb.min(1.0);
    if ta > tb {
        return;
    }
    let mut ts: Vec<f64> = vec![ta, tb];
    let t_near = -bq / l2;
    if t_near > ta && t_near < tb {
        ts.push(t_near);
    }
    for e in profile.envelope() {
        for rk in [e.r_start, e.r_end] {
            if rk > 0.0
                && rk < r_max
                && let Some((t0, t1)) = roots(rk)
            {
                ts.extend([t0, t1].into_iter().filter(|t| *t > ta && *t < tb));
            }
        }
    }
    ts.sort_by(f64::total_cmp);
    ts.dedup();
    let point = |t: f64| Local {
        x: p0.x + t * ex,
        y: p0.y + t * ey,
        z: p0.z + t * (p1.z - p0.z),
    };
    let dist = |t: f64| radial(wx + t * ex, wy + t * ey);
    for w in ts.windows(2) {
        let [t0, t1] = [w.first().copied(), w.get(1).copied()].map(|t| t.unwrap_or(0.0));
        let Some(e) = piece_at(profile.envelope(), dist(0.5 * (t0 + t1))) else {
            continue;
        };
        let f = |t: f64| {
            let d = dist(t).clamp(e.r_start, e.r_end);
            point(t).z - e.piece.height_at(d)
        };
        let t = if e.piece.is_convex_in_radius() {
            golden_max(&f, t0, t1)
        } else {
            sampled_max(&f, t0, t1, 32)
        };
        for tt in [t0, t, t1] {
            better(best, f(tt), point(tt));
        }
    }
}

fn piece_at(envelope: &[EnvelopePiece], d: f64) -> Option<&EnvelopePiece> {
    envelope.iter().find(|e| d >= e.r_start && d <= e.r_end)
}

/// Golden-section maximum of a unimodal function on `[a, b]`.
fn golden_max(f: &impl Fn(f64) -> f64, mut a: f64, mut b: f64) -> f64 {
    let k = (5f64.sqrt() - 1.0) / 2.0;
    let mut c = b - k * (b - a);
    let mut d = a + k * (b - a);
    let (mut fc, mut fd) = (f(c), f(d));
    for _ in 0..100 {
        if (b - a).abs() <= 1e-15 * (1.0 + a.abs()) {
            break;
        }
        if fc < fd {
            a = c;
            c = d;
            fc = fd;
            d = a + k * (b - a);
            fd = f(d);
        } else {
            b = d;
            d = c;
            fd = fc;
            c = b - k * (b - a);
            fc = f(c);
        }
    }
    0.5 * (a + b)
}

/// Samples `n` intervals, then refines about the best sample.
fn sampled_max(f: &impl Fn(f64) -> f64, a: f64, b: f64, n: u32) -> f64 {
    let step = (b - a) / f64::from(n);
    let mut best_i = 0;
    let mut best_f = f64::NEG_INFINITY;
    for i in 0..=n {
        let v = f(a + step * f64::from(i));
        if v > best_f {
            best_f = v;
            best_i = i;
        }
    }
    let lo = a + step * f64::from(best_i.saturating_sub(1));
    let hi = (a + step * f64::from(best_i + 1)).min(b);
    golden_max(f, lo, hi)
}
