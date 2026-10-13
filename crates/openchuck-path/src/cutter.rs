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
    let frame = Frame::new(line);
    // A tip height the answer cannot be below: the tool's tip rests on the
    // highest surface point of the axis line (h(0) = 0).
    let floor = mesh.highest_on_line(line);
    // Each candidate gets an upper bound on the tip height it can give: its
    // highest vertex, less the lowest the tool gets at the nearest the
    // triangle comes to the axis (h(r) never decreases with r). Triangles
    // are visited from the highest bound down, and the search stops once none
    // can beat the best so far, which skips most of a dense mesh.
    let mut order: Vec<(f64, u32, [Local; 3])> = mesh
        .triangles_for_drop(line, profile, floor)
        .into_iter()
        .filter_map(|i| {
            let [a, b, c] = mesh.triangle(i)?;
            let tri = [frame.local(a), frame.local(b), frame.local(c)];
            let top = tri.iter().map(|v| v.z).fold(f64::NEG_INFINITY, f64::max);
            let (lo_x, hi_x) = tri
                .iter()
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
                    (lo.min(v.x), hi.max(v.x))
                });
            let (lo_y, hi_y) = tri
                .iter()
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
                    (lo.min(v.y), hi.max(v.y))
                });
            // Distance from the axis to the triangle's box in the plane.
            let near_x = lo_x.max(0.0).max(-hi_x);
            let near_y = lo_y.max(0.0).max(-hi_y);
            let nearest = radial(near_x, near_y);
            let bound = top - profile.height_at(nearest)?;
            Some((bound, i, tri))
        })
        .collect();
    order.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    let mut best: Option<Best> = None;
    for (bound, _, tri) in &order {
        if best.is_some_and(|b| *bound <= b.tip_z) {
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

// ---- stock to leave -----------------------------------------------------------

use openchuck_types::tool::profile::{ProfilePoint, ProfileSegment, ToolProfile};

use crate::error::{PathError, Result};

fn pp(r: f64, z: f64) -> ProfilePoint {
    ProfilePoint::new(r, z)
}

fn r_of(p: ProfilePoint) -> f64 {
    p.radius.0
}

fn z_of(p: ProfilePoint) -> f64 {
    p.height.0
}

/// The outward unit normal of a convex profile piece at `p`, as (radius,
/// height) components: for a line, perpendicular to its direction; for a
/// convex arc, away from its centre.
fn piece_normal(piece: &ProfilePiece, p: ProfilePoint) -> (f64, f64) {
    match *piece {
        ProfilePiece::Line { start, end } => {
            let (dr, dz) = (r_of(end) - r_of(start), z_of(end) - z_of(start));
            let len = dr.hypot(dz);
            if len <= f64::MIN_POSITIVE {
                (0.0, -1.0)
            } else {
                (dz / len, -dr / len)
            }
        }
        ProfilePiece::Arc { centre, radius, .. } => (
            (r_of(p) - r_of(centre)) / radius,
            (z_of(p) - z_of(centre)) / radius,
        ),
    }
}

/// The tool grown by `offset` in every direction (the Minkowski sum of the
/// revolved tool and a ball of that radius), for stock to leave: a drop with
/// the grown tool leaves `offset` of material on the surface (OP-11).
///
/// The result is a profile of its own, measured from its own tip, which lies
/// `offset` below the original tip: a tool tip found with the grown tool is
/// `offset` below the original tool's tip along the axis, so add `offset`
/// along the axis to get the original tip. Only convex profiles can be
/// grown in closed form (every standard tool is convex); a concave custom
/// profile is refused with a message rather than approximated.
pub fn grow_profile(profile: &RevolvedProfile, offset: f64) -> Result<RevolvedProfile> {
    if !(offset.is_finite() && offset >= 0.0) {
        return Err(PathError::Strategy(format!(
            "the stock to leave {offset} mm must be finite and not negative"
        )));
    }
    if offset == 0.0 {
        return Ok(profile.clone());
    }
    if !profile.is_convex() {
        return Err(PathError::Strategy(
            "stock to leave is not supported for this tool: its profile is not convex \
             (a concave custom profile); set the stock to leave to 0 or use a convex tool"
                .to_owned(),
        ));
    }
    let s = offset;
    // Work in the original frame, then lift by `s` so the grown tip is the origin.
    let mut segments: Vec<ProfileSegment> = Vec::new();
    let lift = |r: f64, z: f64| pp(r, z + s);
    let eps = 1e-12;
    let mut previous_normal = (0.0, -1.0);
    for piece in profile.pieces() {
        let start = piece.start();
        let end = piece.end();
        let n_start = piece_normal(piece, start);
        // A join about the vertex where the normal turns.
        let turn = n_start.0 * previous_normal.1 - n_start.1 * previous_normal.0;
        let from = pp(
            r_of(start) + previous_normal.0 * s,
            z_of(start) + previous_normal.1 * s,
        );
        let to = pp(r_of(start) + n_start.0 * s, z_of(start) + n_start.1 * s);
        if from.distance(to) > eps && turn.abs() > eps {
            segments.push(ProfileSegment::Arc {
                to: lift(r_of(to), z_of(to)),
                centre: lift(r_of(start), z_of(start)),
            });
        }
        let n_end = piece_normal(piece, end);
        let new_end = pp(r_of(end) + n_end.0 * s, z_of(end) + n_end.1 * s);
        match *piece {
            ProfilePiece::Line { .. } => segments.push(ProfileSegment::Line {
                to: lift(r_of(new_end), z_of(new_end)),
            }),
            ProfilePiece::Arc { centre, .. } => segments.push(ProfileSegment::Arc {
                to: lift(r_of(new_end), z_of(new_end)),
                centre: lift(r_of(centre), z_of(centre)),
            }),
        }
        previous_normal = n_end;
    }
    // The first segment must leave the axis at the grown tip: a line that
    // starts at the original tip's offset needs no extra move because the
    // lifted start is (0, 0) when the first normal points straight down; a
    // first join arc starts there too.
    ToolProfile { segments }.validate().map_err(PathError::from)
}

/// The point of `profile` (in tool coordinates: radius, height) nearest to
/// (`r`, `z`).
fn nearest_on_profile(profile: &RevolvedProfile, r: f64, z: f64) -> (f64, f64) {
    let mut best = (f64::INFINITY, (0.0, 0.0));
    for piece in profile.pieces() {
        let cand = match *piece {
            ProfilePiece::Line { start, end } => {
                let (dr, dz) = (r_of(end) - r_of(start), z_of(end) - z_of(start));
                let l2 = dr * dr + dz * dz;
                let t = if l2 <= f64::MIN_POSITIVE {
                    0.0
                } else {
                    (((r - r_of(start)) * dr + (z - z_of(start)) * dz) / l2).clamp(0.0, 1.0)
                };
                (r_of(start) + t * dr, z_of(start) + t * dz)
            }
            ProfilePiece::Arc {
                start,
                end,
                centre,
                radius,
                ..
            } => {
                let (vr, vz) = (r - r_of(centre), z - z_of(centre));
                let len = vr.hypot(vz);
                let on_circle = if len <= f64::MIN_POSITIVE {
                    (r_of(start), z_of(start))
                } else {
                    (
                        r_of(centre) + radius * vr / len,
                        z_of(centre) + radius * vz / len,
                    )
                };
                // Within the arc's angular range?
                let ang = |p: (f64, f64)| (p.1 - z_of(centre)).atan2(p.0 - r_of(centre));
                let (a0, a1, aq) = (
                    ang((r_of(start), z_of(start))),
                    ang((r_of(end), z_of(end))),
                    ang(on_circle),
                );
                let (lo, hi) = (a0.min(a1), a0.max(a1));
                if aq >= lo - 1e-12 && aq <= hi + 1e-12 {
                    on_circle
                } else {
                    let ds = (r - r_of(start)).hypot(z - z_of(start));
                    let de = (r - r_of(end)).hypot(z - z_of(end));
                    if ds <= de {
                        (r_of(start), z_of(start))
                    } else {
                        (r_of(end), z_of(end))
                    }
                }
            }
        };
        let d = (r - cand.0).hypot(z - cand.1);
        if d < best.0 {
            best = (d, cand);
        }
    }
    best.1
}

/// Where the original tool, with its tip at `tip` and axis `axis`, comes
/// closest to the surface point `contact`: the cutter contact point of a
/// pass that leaves stock (it lies on the left-over surface, `offset` from
/// `contact`). For no stock it is `contact` itself.
pub fn cutter_contact(
    profile: &RevolvedProfile,
    tip: Point3,
    axis: UnitVec3,
    contact: Point3,
) -> Point3 {
    let e3 = axis.get();
    let w = contact - tip;
    let z = w.dot(e3);
    let radial = w - e3 * z;
    let r = radial.length();
    let (rq, zq) = nearest_on_profile(profile, r, z);
    let dir = if r > 1e-12 {
        radial * (1.0 / r)
    } else {
        // On the axis the radial direction is arbitrary and the profile
        // point has radius zero.
        Vec3::ZERO
    };
    tip + e3 * zq + dir * rq
}
