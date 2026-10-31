// SPDX-License-Identifier: AGPL-3.0-or-later
//! Surfaces hidden from simultaneous finishing's drive (OP-20; decision
//! record 0368).
//!
//! The drive takes the first point a ray from outside meets on its way to
//! the rotary axis, or to a section's own centre (OP-26). Anything such a
//! ray does not meet first is never driven: the walls of a slot along X, an
//! undercut, the sides of separate pieces of a section facing each other.
//!
//! **Where.** The part is cut across X at sections the stepover apart, each
//! a set of closed chains of the mesh's (or the B-rep's tessellation's)
//! triangles. Points along each chain are classed: met by the drive's ray
//! (visible) or not (hidden). Each run of hidden points, grown by a
//! stepover into the visible surface on either side so that no ridge is
//! left where they meet, is a *contour*: one pass along the chain at that X,
//! cut after the drive's passes. The drive's own passes are unchanged, so a
//! part with nothing hidden gives the same program.
//!
//! **The tool axis.** At each point of a contour the tool is tried with its
//! axis at angles across X: it reaches the point when, dropped along that
//! axis, it rests on the point to within the tolerance, and its shank, where
//! wider than the cutting part, is clear of the part. The axis is the
//! reachable one nearest the normal (plus the lead), within the largest lean
//! on hidden surfaces; A is smoothed as on the drive's passes within the
//! largest tilt of that axis, and never outside the angles that reach.
//!
//! **What cannot be reached** is classed by why: blocked (no straight line
//! across X reaches the point from outside: the tool would have to lean
//! along the rotary axis), too narrow (a line does, but the tool, its flutes
//! or its shank do not fit), or needing more lean than allowed. Points too
//! narrow for the tool within a tool radius of points it finished are a
//! corner, which rest machining finishes (OP-09) as on visible surfaces,
//! and are not reported. The rest are grouped into areas and reported.

use std::collections::{BTreeMap, BTreeSet};

use openchuck_types::geometry::{Point3, UnitVec3, Vec3};
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::tool::RevolvedProfile;
use openchuck_types::tool::profile::{ProfilePiece, ProfilePoint, ProfileSegment, ToolProfile};
use openchuck_types::units::Degrees;

use crate::assemble::{Unreached, UnreachedReason};
use crate::cutter::{self, ToolLine};
use crate::parallel;
use crate::rotary::TURN;
use crate::strategy::Shank;

use super::{NORMAL_SPAN, Plan, near, polar};

/// How far apart the points of a contour are tried for reach, mm along it.
pub(super) const REACH_SPACING: f64 = 0.2;

/// A point met by the drive's ray within this distance is visible, mm: the
/// ray and the section meet the same surface, give or take a tessellation's
/// deflection.
const VISIBLE_GAP: f64 = 0.1;

/// How far inside the angles found to reach the axis is kept, degrees.
const GATE_MARGIN: f64 = 0.5;

/// The largest angle across X between a drive ray and the surface normal
/// at which the drive's passes finish a surface, degrees.
const GRAZING: f64 = 75.0;

/// The first steps of the search for a reachable axis, degrees.
const SEARCH_STEP: f64 = 5.0;

/// Halvings that place the edge of the reachable angles.
const SEARCH_HALVINGS: u32 = 6;

/// How far the shank may rest above the cutting part's drop before it is
/// taken as touching, as a share of the tolerance.
const SHANK_SLACK: f64 = 0.1;

/// The fewest points of an area worth reporting: one or two are where a
/// grazing ray happens to miss.
const MIN_REPORTED_POINTS: usize = 3;

/// Seeded tool positions along a contour, mm apart, before path fitting.
pub(super) const CONTOUR_SEED: f64 = 0.5;

/// A gap in angle round the axis, degrees, below which an area goes all
/// round.
const ALL_ROUND_GAP: f64 = 10.0;

/// The longest run of unreached points, in tool radii along the section,
/// that is a corner: a round end in a square corner leaves two radii.
const CORNER_SPAN: f64 = 2.5;

/// The least inward turn of the surface round a corner, degrees.
const CORNER_TURN: f64 = 30.0;

/// The bucket width of [`Slicer`], mm along X.
const BUCKET: f64 = 0.5;

/// The triangles of a mesh by where they lie along X, to cut it across X.
pub(super) struct Slicer<'a> {
    mesh: &'a TriangleMesh,
    x0: f64,
    buckets: Vec<Vec<u32>>,
}

/// An edge of the mesh by its end points' bits, lower first, so the two
/// triangles that share it name it alike whether or not the mesh is welded.
type EdgeKey = [u64; 6];

/// A segment of a section, oriented with the material on its left seen from
/// +X (counter-clockwise round the material), and its triangle's normal.
#[derive(Debug, Clone, Copy)]
struct Segment {
    from: EdgeKey,
    to: EdgeKey,
    a: Point3,
    b: Point3,
    normal: Vec3,
}

/// A chain of a section: points in order with the material on their left
/// seen from +X, the outward facet normal of each segment, and whether the
/// last point joins the first.
#[derive(Debug, Clone, Default)]
pub(super) struct Chain {
    points: Vec<Point3>,
    normals: Vec<Vec3>,
    closed: bool,
    /// The length from the first point to each point (and, closed, back to
    /// the first as the last entry).
    s: Vec<f64>,
}

fn bits(p: Point3) -> [u64; 3] {
    [p.x.to_bits(), p.y.to_bits(), p.z.to_bits()]
}

impl<'a> Slicer<'a> {
    pub(super) fn new(mesh: &'a TriangleMesh) -> Self {
        let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
        for v in &mesh.vertices {
            lo = lo.min(v.x);
            hi = hi.max(v.x);
        }
        if !(lo.is_finite() && hi.is_finite()) {
            return Self {
                mesh,
                x0: 0.0,
                buckets: Vec::new(),
            };
        }
        let n = (((hi - lo) / BUCKET).floor() as usize).saturating_add(1);
        let mut buckets = vec![Vec::new(); n];
        for (i, t) in mesh.triangles.iter().enumerate() {
            let xs = t.map(|k| mesh.vertices.get(k as usize).map_or(f64::NAN, |v| v.x));
            let (a, b) = xs
                .iter()
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), x| {
                    (a.min(*x), b.max(*x))
                });
            if !(a.is_finite() && b.is_finite()) {
                continue;
            }
            let first = ((a - lo) / BUCKET).floor() as usize;
            let last = (((b - lo) / BUCKET).floor() as usize).min(n - 1);
            for bucket in buckets.iter_mut().take(last + 1).skip(first) {
                bucket.push(i as u32);
            }
        }
        Self {
            mesh,
            x0: lo,
            buckets,
        }
    }

    /// The chains of the section at `x`. A vertex exactly at `x` counts as
    /// beyond it, so every triangle crossed gives one segment and the
    /// chains close wherever the mesh does.
    pub(super) fn chains(&self, x: f64) -> Vec<Chain> {
        let k = ((x - self.x0) / BUCKET).floor();
        let Some(list) = (k >= 0.0).then(|| self.buckets.get(k as usize)).flatten() else {
            return Vec::new();
        };
        let v = |i: u32| self.mesh.vertices.get(i as usize).copied();
        let mut segments: Vec<Segment> = Vec::new();
        for &t in list {
            let Some(tri) = self.mesh.triangles.get(t as usize) else {
                continue;
            };
            let (Some(p0), Some(p1), Some(p2)) = (v(tri[0]), v(tri[1]), v(tri[2])) else {
                continue;
            };
            let pts = [p0, p1, p2];
            let beyond = pts.map(|p| p.x >= x);
            if beyond.iter().all(|b| *b) || beyond.iter().all(|b| !*b) {
                continue;
            }
            let normal = (p1 - p0).cross(p2 - p0);
            // The vertex alone on its side, and the two edges it cuts. Seen
            // from +X the segment runs counter-clockwise round the material
            // from its edge to the next vertex towards the edge from the one
            // before when that vertex is beyond `x`, and back otherwise: the
            // triangle's winding decides, so a segment of no length at a
            // vertex is oriented like its neighbours.
            let Some(k) = (0..3usize).find(|&k| {
                let b = beyond.get(k);
                b != beyond.get((k + 1) % 3) && b != beyond.get((k + 2) % 3)
            }) else {
                continue;
            };
            let cut = |i: usize, j: usize| -> Option<(EdgeKey, Point3)> {
                let (pi, pj) = (*pts.get(i)?, *pts.get(j)?);
                let (lo, hi) = if bits(pi) <= bits(pj) {
                    (pi, pj)
                } else {
                    (pj, pi)
                };
                let f = (x - lo.x) / (hi.x - lo.x);
                let point = lo + (hi - lo) * f;
                let [a, b, c] = bits(lo);
                let [d, e, g] = bits(hi);
                Some(([a, b, c, d, e, g], Point3::new(x, point.y, point.z)))
            };
            let (Some((ka, a)), Some((kb, b))) = (cut(k, (k + 1) % 3), cut((k + 2) % 3, k)) else {
                continue;
            };
            let (from, to, a, b) = if beyond.get(k).copied().unwrap_or(false) {
                (ka, kb, a, b)
            } else {
                (kb, ka, b, a)
            };
            segments.push(Segment {
                from,
                to,
                a,
                b,
                normal,
            });
        }
        chain_up(&segments)
    }
}

/// The segments joined end to start into chains: open chains first, from
/// their free starts, then the closed ones, each from its first segment in
/// the triangles' order.
fn chain_up(segments: &[Segment]) -> Vec<Chain> {
    let mut by_start: BTreeMap<EdgeKey, usize> = BTreeMap::new();
    for (i, s) in segments.iter().enumerate() {
        by_start.entry(s.from).or_insert(i);
    }
    let ends: BTreeSet<EdgeKey> = segments.iter().map(|s| s.to).collect();
    let mut used = vec![false; segments.len()];
    let mut out = Vec::new();
    let starts: Vec<usize> = (0..segments.len())
        .filter(|&i| segments.get(i).is_some_and(|s| !ends.contains(&s.from)))
        .chain(0..segments.len())
        .collect();
    for first in starts {
        if used.get(first).copied().unwrap_or(true) {
            continue;
        }
        let mut chain = Chain::default();
        let mut cur = first;
        while let Some(s) = segments.get(cur) {
            if let Some(u) = used.get_mut(cur) {
                *u = true;
            }
            chain.points.push(s.a);
            chain.normals.push(s.normal);
            match by_start.get(&s.to).copied() {
                Some(next) if next == first => {
                    chain.closed = true;
                    break;
                }
                Some(next) if !used.get(next).copied().unwrap_or(true) => cur = next,
                _ => {
                    chain.points.push(s.b);
                    break;
                }
            }
        }
        chain.measure();
        if chain.length() > 0.0 {
            out.push(chain);
        }
    }
    out
}

impl Chain {
    fn measure(&mut self) {
        let mut s = Vec::with_capacity(self.points.len() + 1);
        let mut total = 0.0;
        s.push(0.0);
        for w in self.points.windows(2) {
            if let [a, b] = w {
                total += (*b - *a).length();
                s.push(total);
            }
        }
        if self.closed
            && let (Some(first), Some(last)) = (self.points.first(), self.points.last())
        {
            total += (*first - *last).length();
            s.push(total);
        }
        self.s = s;
    }

    pub(super) fn length(&self) -> f64 {
        self.s.last().copied().unwrap_or(0.0)
    }

    /// The point at `s` along the chain (wrapping when closed, clamped
    /// when open), with the outward facet normal of its segment.
    pub(super) fn at(&self, s: f64) -> Option<(Point3, Vec3)> {
        let len = self.length();
        let s = if self.closed && len > 0.0 {
            s.rem_euclid(len)
        } else {
            s.clamp(0.0, len)
        };
        let i = self.s.partition_point(|v| *v <= s).saturating_sub(1);
        let a = *self.points.get(i)?;
        let b = if i + 1 < self.points.len() {
            *self.points.get(i + 1)?
        } else {
            *self.points.first()?
        };
        let normal = *self.normals.get(i).or(self.normals.last())?;
        let s0 = *self.s.get(i)?;
        let s1 = self.s.get(i + 1).copied().unwrap_or(s0);
        let f = if s1 > s0 { (s - s0) / (s1 - s0) } else { 0.0 };
        Some((a + (b - a) * f, normal))
    }

    /// The chain's own points strictly between `s0` and `s1` along it
    /// (`s1` may pass the end of a closed chain), in order.
    fn vertices_between(&self, s0: f64, s1: f64) -> Vec<Point3> {
        let len = self.length();
        let mut out = Vec::new();
        let turns = if self.closed && len > 0.0 {
            [0.0, len, 2.0 * len]
        } else {
            [0.0, f64::NAN, f64::NAN]
        };
        for base in turns.iter().filter(|b| b.is_finite()) {
            for (p, s) in self.points.iter().zip(&self.s) {
                let at = base + s;
                if at > s0 + 1e-9 && at < s1 - 1e-9 {
                    out.push(*p);
                }
            }
        }
        out
    }

    /// The point of the chain nearest `p`, and its distance.
    fn nearest(&self, p: Point3) -> Option<(Point3, f64)> {
        let n = self.points.len();
        let segs = if self.closed { n } else { n.saturating_sub(1) };
        let mut best: Option<(Point3, f64)> = None;
        for i in 0..segs {
            let (Some(&a), Some(&b)) = (self.points.get(i), self.points.get((i + 1) % n)) else {
                continue;
            };
            let d = b - a;
            let l2 = d.dot(d);
            let t = if l2 > 0.0 {
                ((p - a).dot(d) / l2).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let q = a + d * t;
            let dist = (p - q).length();
            if best.is_none_or(|(_, b)| dist < b) {
                best = Some((q, dist));
            }
        }
        best
    }
}

/// What a point of a contour is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Status {
    /// Met by the drive's ray: a margin of the contour.
    Visible,
    /// Hidden, and the tool reaches it.
    Reached,
    /// Hidden, and the tool cannot reach it.
    Unreached(UnreachedReason),
}

/// A point of a contour where the tool's axis was chosen.
#[derive(Debug, Clone, Copy)]
pub(super) struct Sample {
    /// Its place along the contour, 0 to 1.
    pub t: f64,
    pub point: Point3,
    /// The normal's angle across X, degrees (the smoothed normal's on a
    /// mesh).
    pub phi_n: f64,
    /// The angles of the axis that reach it: the one nearest the target,
    /// and the band A may be smoothed in. `None` where it is not reached.
    pub gate: Option<(f64, f64, f64)>,
    pub status: Status,
    /// Beyond the run the pass cuts, tried only to tell a corner from a
    /// surface the tool cannot reach.
    pub probe: bool,
}

/// One pass along a chain at one X, over a run of hidden points and a
/// stepover of visible surface each side.
#[derive(Debug, Clone)]
pub(super) struct Contour {
    pub x: f64,
    /// The section's index, for the order of the passes.
    pub section: usize,
    /// The chain's points along the pass, and their lengths from its start.
    points: Vec<Point3>,
    s: Vec<f64>,
    /// The facet normal of each segment between them.
    facets: Vec<Vec3>,
    pub samples: Vec<Sample>,
    /// +1 when the chain runs counter-clockwise round the material seen
    /// from +X (its normals point to the right of its direction), -1 when
    /// a tessellation wound the other way made it run clockwise.
    sense: f64,
    /// Whether the exact faces give the points and normals.
    exact: bool,
}

impl Contour {
    pub(super) fn length(&self) -> f64 {
        self.s.last().copied().unwrap_or(0.0)
    }

    /// The point at `t` along the contour, 0 to 1.
    fn point_at(&self, t: f64) -> Option<Point3> {
        let s = t.clamp(0.0, 1.0) * self.length();
        let i = self.s.partition_point(|v| *v <= s).saturating_sub(1);
        let a = *self.points.get(i)?;
        let Some(&b) = self.points.get(i + 1) else {
            return Some(a);
        };
        let (s0, s1) = (*self.s.get(i)?, *self.s.get(i + 1)?);
        let f = if s1 > s0 { (s - s0) / (s1 - s0) } else { 0.0 };
        Some(a + (b - a) * f)
    }

    /// The drive point at `t` and the normal the tool is placed by there:
    /// the exact face's, or the normal of the facet the point lies on, the
    /// only one with which the tool can touch it (the smoothed normal of a
    /// mesh, which the axis follows, leans across a convex edge).
    pub(super) fn drive(&self, plan: &Plan<'_>, t: f64) -> Option<(Point3, UnitVec3, bool)> {
        let p = self.point_at(t)?;
        if self.exact
            && let Some(q) = plan.input.surface.project(p)
            && q.position.is_some()
        {
            return Some((q.point, q.normal, true));
        }
        let s = t.clamp(0.0, 1.0) * self.length();
        let i = self.s.partition_point(|v| *v <= s).saturating_sub(1);
        let facet = self.facets.get(i).or(self.facets.last())?;
        Some((p, UnitVec3::new(*facet).ok()?, false))
    }
}

/// What the hidden surfaces gave: the contours to cut and the areas left.
#[derive(Debug, Default)]
pub(super) struct Hidden {
    pub contours: Vec<Contour>,
    pub unreached: Vec<Unreached>,
}

/// The tool's outline with its shank where wider than the cutting part, for
/// the drop that tells whether the shank meets the part.
pub(super) fn body(
    grown: &RevolvedProfile,
    shank: Option<Shank>,
    leave: f64,
) -> Option<RevolvedProfile> {
    let shank = shank?;
    let end = grown.end();
    if !(shank.radius.is_finite() && shank.height.is_finite()) || shank.radius <= end.radius.0 {
        return None;
    }
    let mut segments: Vec<ProfileSegment> = grown
        .pieces()
        .iter()
        .map(|p| match *p {
            ProfilePiece::Line { end, .. } => ProfileSegment::Line { to: end },
            ProfilePiece::Arc { end, centre, .. } => ProfileSegment::Arc { to: end, centre },
        })
        .collect();
    let height = (shank.height + leave).max(end.height.0);
    if height > end.height.0 + 1e-9 {
        segments.push(ProfileSegment::Line {
            to: ProfilePoint::new(end.radius.0, height),
        });
    }
    segments.push(ProfileSegment::Line {
        to: ProfilePoint::new(shank.radius + leave, height),
    });
    ToolProfile { segments }.validate().ok()
}

/// The distance from the point `(r, z)` in a tool's frame to the tool's
/// surface, the tool above its profile being a cylinder of its end radius.
pub(super) fn to_tool(profile: &RevolvedProfile, r: f64, z: f64) -> f64 {
    let end = profile.end();
    if z >= end.height.0 {
        return (r - end.radius.0).abs();
    }
    let (nr, nz) = cutter::nearest_on_profile(profile, r, z);
    (r - nr).hypot(z - nz)
}

/// The order of the contours: by area of hidden surface (contours on
/// neighbouring sections that come within `near` of each other), then along
/// X, every other one reversed so that each starts near where the one
/// before ended.
fn order(contours: &mut Vec<Contour>, near_mm: f64) {
    let n = contours.len();
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(parent: &mut [usize], mut i: usize) -> usize {
        while let Some(&p) = parent.get(i) {
            if p == i {
                break;
            }
            let gp = parent.get(p).copied().unwrap_or(p);
            if let Some(slot) = parent.get_mut(i) {
                *slot = gp;
            }
            i = gp;
        }
        i
    }
    for i in 0..n {
        for j in i + 1..n {
            let (Some(a), Some(b)) = (contours.get(i), contours.get(j)) else {
                continue;
            };
            if b.section != a.section + 1 && a.section != b.section + 1 {
                continue;
            }
            let close = a.samples.iter().any(|p| {
                b.samples
                    .iter()
                    .any(|q| (p.point - q.point).length() <= near_mm)
            });
            if close {
                let (ri, rj) = (find(&mut parent, i), find(&mut parent, j));
                if ri != rj {
                    let (lo, hi) = (ri.min(rj), ri.max(rj));
                    if let Some(slot) = parent.get_mut(hi) {
                        *slot = lo;
                    }
                }
            }
        }
    }
    let mut keyed: Vec<(usize, usize, usize, Contour)> = std::mem::take(contours)
        .into_iter()
        .enumerate()
        .map(|(i, c)| (find(&mut parent, i), c.section, i, c))
        .collect();
    keyed.sort_by_key(|k| (k.0, k.1, k.2));
    let mut last_group = usize::MAX;
    let mut flip = false;
    for (group, _, _, mut c) in keyed {
        if group != last_group {
            flip = false;
            last_group = group;
        }
        if flip {
            c.reverse();
        }
        flip = !flip;
        contours.push(c);
    }
}

impl Contour {
    fn reverse(&mut self) {
        let len = self.length();
        self.sense = -self.sense;
        self.points.reverse();
        self.facets.reverse();
        self.s = self.s.iter().rev().map(|s| len - s).collect();
        self.samples.reverse();
        for s in &mut self.samples {
            s.t = 1.0 - s.t;
        }
    }
}

/// A run of points along one chain: where it starts and ends along it, mm
/// (the end may pass the end of a closed chain).
#[derive(Debug, Clone, Copy)]
struct Run {
    chain: usize,
    s0: f64,
    s1: f64,
}

/// The settings of the hidden passes, from the operation.
#[derive(Debug, Clone, Copy)]
pub(super) struct Settings {
    pub stepover: f64,
    pub lead: f64,
    pub max_tilt: f64,
    pub max_lean: f64,
    pub x0: f64,
    pub x1: f64,
}

impl Plan<'_> {
    /// The hidden surfaces of the part between `settings.x0` and `x1`: the
    /// contours that finish what the tool reaches, and the areas it does
    /// not (OP-20).
    pub(super) fn hidden(&self, settings: Settings) -> Hidden {
        let mesh = self.input.surface.tessellation();
        let slicer = Slicer::new(mesh);
        let length = settings.x1 - settings.x0;
        let n = (length / settings.stepover).ceil().max(1.0) as usize;
        let dx = length / n as f64;
        let xs: Vec<f64> = (0..n)
            .map(|k| settings.x0 + (k as f64 + 0.5) * dx)
            .collect();
        let exact = !self.input.surface.exact_faces().is_empty();
        // Each section's chains and the runs of hidden points on them,
        // grown by a stepover each side.
        let found: Vec<(Vec<Chain>, Vec<Run>)> = parallel::map_in_order(&xs, |x| {
            let chains = slicer.chains(*x);
            let runs = self.hidden_runs(*x, &chains, settings.stepover.max(self.reach), exact);
            (chains, runs)
        });
        // The neighbouring sections, for the smoothed normals of a mesh.
        let span = NORMAL_SPAN;
        let mut jobs: Vec<(usize, f64, &Chain, Run)> = Vec::new();
        for (k, (x, (chains, runs))) in xs.iter().zip(&found).enumerate() {
            for run in runs {
                if let Some(chain) = chains.get(run.chain) {
                    jobs.push((k, *x, chain, *run));
                }
            }
        }
        if jobs.is_empty() {
            return Hidden::default();
        }
        let mut contours: Vec<Contour> = parallel::map_in_order(&jobs, |(k, x, chain, run)| {
            let beside = if exact {
                (Vec::new(), Vec::new())
            } else {
                (slicer.chains(x - span), slicer.chains(x + span))
            };
            self.contour(*k, *x, chain, *run, &beside, exact, settings)
        })
        .into_iter()
        .flatten()
        .collect();
        let unreached = self.areas(&contours, dx, settings);
        contours.retain(|c| {
            c.samples
                .iter()
                .any(|s| s.status == Status::Reached && !s.probe)
        });
        order(&mut contours, 2.0 * dx.max(REACH_SPACING));
        Hidden {
            contours,
            unreached,
        }
    }

    /// Whether the drive's ray at `p`'s angle meets `p` first, and not at
    /// a grazing angle: across X, within [`GRAZING`] of the facet normal
    /// `facet`. A surface nearly along the rays (the side of a blade, the
    /// edge of a pin seen from beside) is met by rays far apart on it and
    /// with the tool's axis across it, and is finished as a hidden one.
    ///
    /// On exact faces (`exact`) the point of the tessellation is first put
    /// on the face it lies on, with the face's normal, as the rays meet the
    /// faces, not their tessellation.
    fn visible(&self, x: f64, p: Point3, facet: Vec3, exact: bool) -> bool {
        // The face's own normal too: a tessellation's winding need not
        // match it.
        let (p, facet) = if exact {
            self.input
                .surface
                .project(p)
                .filter(|q| q.position.is_some())
                .map_or((p, facet), |q| (q.point, q.normal.get()))
        } else {
            (p, facet)
        };
        let zone = self
            .zones
            .iter()
            .position(|z| x >= z.x0 && x <= z.x1)
            .unwrap_or(0);
        let (cy, cz) = self.zones.get(zone).map_or((0.0, 0.0), |z| z.centre_at(x));
        let outward = Vec3::new(0.0, p.y - cy, p.z - cz);
        let across = Vec3::new(0.0, facet.y, facet.z);
        let (lo, la) = (outward.length(), across.length());
        if lo > 0.0 && la > 0.0 && outward.dot(across) < lo * la * GRAZING.to_radians().cos() {
            return false;
        }
        let th = (p.z - cz).atan2(p.y - cy).to_degrees();
        self.ray(zone, x, th)
            .is_some_and(|h| (h.point - p).length() <= VISIBLE_GAP)
    }

    /// The runs of points on `chains` at `x` the drive's rays do not meet,
    /// each grown by `grow` along its chain on both sides, joined where
    /// they then overlap.
    fn hidden_runs(&self, x: f64, chains: &[Chain], grow: f64, exact: bool) -> Vec<Run> {
        let mut out = Vec::new();
        for (c, chain) in chains.iter().enumerate() {
            let len = chain.length();
            let n = (len / REACH_SPACING).ceil().max(1.0) as usize;
            let ds = len / n as f64;
            let count = if chain.closed { n } else { n + 1 };
            let hidden: Vec<bool> = (0..count)
                .map(|i| {
                    chain
                        .at(i as f64 * ds)
                        .is_some_and(|(p, facet)| !self.visible(x, p, facet, exact))
                })
                .collect();
            if !hidden.iter().any(|h| *h) {
                continue;
            }
            // Runs of hidden points as index ranges; on a closed chain a run
            // through the start joins the one at the end.
            let mut runs: Vec<(usize, usize)> = Vec::new();
            let mut start = None;
            for (i, h) in hidden.iter().enumerate() {
                match (h, start) {
                    (true, None) => start = Some(i),
                    (false, Some(s)) => {
                        runs.push((s, i - 1));
                        start = None;
                    }
                    _ => {}
                }
            }
            if let Some(s) = start {
                runs.push((s, count - 1));
            }
            let all = hidden.iter().all(|h| *h);
            if chain.closed
                && !all
                && runs.len() > 1
                && let (Some(&(f0, _)), Some(&(_, l1))) = (runs.first(), runs.last())
                && f0 == 0
                && l1 == count - 1
            {
                let (_, f1) = runs.remove(0);
                if let Some(last) = runs.last_mut() {
                    last.1 = count + f1;
                }
            }
            let mut spans: Vec<(f64, f64)> = runs
                .iter()
                .map(|&(a, b)| {
                    let (s0, s1) = (a as f64 * ds - grow, b as f64 * ds + grow);
                    if chain.closed {
                        (s0, s1)
                    } else {
                        (s0.max(0.0), s1.min(len))
                    }
                })
                .collect();
            if chain.closed && all {
                spans = vec![(0.0, len)];
            }
            spans.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut merged: Vec<(f64, f64)> = Vec::new();
            for (a, b) in spans {
                match merged.last_mut() {
                    Some(m) if a <= m.1 => m.1 = m.1.max(b),
                    _ => merged.push((a, b)),
                }
            }
            if chain.closed
                && merged.len() > 1
                && let (Some(&(f0, f1)), Some(last)) = (merged.first(), merged.last().copied())
                && last.1 >= f0 + len
            {
                merged.remove(0);
                if let Some(l) = merged.last_mut() {
                    l.1 = l.1.max(f1 + len);
                }
            }
            for (s0, s1) in merged {
                let s1 = if chain.closed { s1.min(s0 + len) } else { s1 };
                out.push(Run { chain: c, s0, s1 });
            }
        }
        out
    }

    /// The contour of `run`, with each sample's normal and reach.
    #[allow(clippy::too_many_arguments)]
    fn contour(
        &self,
        section: usize,
        x: f64,
        chain: &Chain,
        run: Run,
        beside: &(Vec<Chain>, Vec<Chain>),
        exact: bool,
        settings: Settings,
    ) -> Option<Contour> {
        if run.s1 <= run.s0 {
            return None;
        }
        // Tried a tool radius beyond the run's ends as well, to tell a
        // corner from a surface the tool cannot reach (see `areas`).
        let len = chain.length();
        let (s0, s1) = if chain.closed {
            let room = 0.5 * (len - (run.s1 - run.s0)).max(0.0);
            (run.s0 - self.reach.min(room), run.s1 + self.reach.min(room))
        } else {
            (
                (run.s0 - self.reach).max(0.0),
                (run.s1 + self.reach).min(len),
            )
        };
        let length = s1 - s0;
        let mut points = Vec::new();
        points.push(chain.at(s0)?.0);
        points.extend(chain.vertices_between(s0, s1));
        points.push(chain.at(s1)?.0);
        let mut s = Vec::with_capacity(points.len());
        let mut total = 0.0;
        s.push(0.0);
        for w in points.windows(2) {
            if let [a, b] = w {
                total += (*b - *a).length();
                s.push(total);
            }
        }
        let facets: Vec<Vec3> = s
            .windows(2)
            .filter_map(|w| match w {
                [a, b] => chain.at(s0 + 0.5 * (a + b)).map(|q| q.1),
                _ => None,
            })
            .collect();
        let n = (length / REACH_SPACING).ceil().max(1.0) as usize;
        let mut samples: Vec<Sample> = Vec::with_capacity(n + 1);
        let mut warm: Option<f64> = None;
        let mut previous_phi: Option<f64> = None;
        let mut sense = 0.0_f64;
        let window = settings.max_lean;
        for i in 0..=n {
            let at = s0 + length * i as f64 / n as f64;
            let probe = at < run.s0 - 1e-9 || at > run.s1 + 1e-9;
            let Some((p, facet)) = chain.at(at) else {
                continue;
            };
            // The tool is placed by the exact or the facet normal, and its
            // axis aims by the exact or the smoothed one.
            let (point, seed, normal) = if exact {
                match self.input.surface.project(p) {
                    Some(q) => (q.point, q.normal, q.normal),
                    None => {
                        let f = UnitVec3::new(facet).ok()?;
                        (p, f, f)
                    }
                }
            } else {
                let f = UnitVec3::new(facet).ok()?;
                (p, f, self.contour_normal(chain, at, p, facet, beside)?)
            };
            let t = i as f64 / n as f64;
            let visible = self.visible(x, p, facet, exact);
            let phi_n = match polar(normal.get()) {
                Some(phi) if normal.get().y.hypot(normal.get().z) > 1e-6 => phi,
                _ => {
                    let (cy, cz) = self.centre(x);
                    (point.z - cz).atan2(point.y - cy).to_degrees()
                }
            };
            let phi_n = previous_phi.map_or(phi_n, |prev| near(phi_n, prev));
            previous_phi = Some(phi_n);
            // Lead leans the tool towards the way the pass travels round the
            // section's centre.
            let ahead = chain.at(at + REACH_SPACING).map_or(point, |q| q.0);
            sense += normal
                .get()
                .dot((ahead - point).cross(Vec3::new(1.0, 0.0, 0.0)));
            let travel = {
                let (cy, cz) = self.centre(x);
                let r = Vec3::new(0.0, point.y - cy, point.z - cz);
                let d = ahead - point;
                r.cross(d).x.signum()
            };
            let target = phi_n + settings.lead * travel;
            let reach = self.reach(point, seed, phi_n, target, window, settings.max_tilt, warm);
            let (gate, status) = match reach {
                Ok(g) => {
                    warm = Some(g.0);
                    (
                        Some(g),
                        if visible {
                            Status::Visible
                        } else {
                            Status::Reached
                        },
                    )
                }
                Err(reason) => {
                    warm = None;
                    (
                        None,
                        if visible {
                            Status::Visible
                        } else {
                            Status::Unreached(reason)
                        },
                    )
                }
            };
            samples.push(Sample {
                t,
                point,
                phi_n,
                gate,
                status,
                probe,
            });
        }
        Some(Contour {
            x,
            section,
            points,
            s,
            facets,
            samples,
            exact,
            sense: if sense < 0.0 { -1.0 } else { 1.0 },
        })
    }

    /// The centre the drive's rays aim at, at `x` (OP-26).
    fn centre(&self, x: f64) -> (f64, f64) {
        self.zones
            .iter()
            .find(|z| x >= z.x0 && x <= z.x1)
            .map_or((0.0, 0.0), |z| z.centre_at(x))
    }

    /// The normal of a mesh at `p`, `at` along `chain`: the surface through
    /// the chain [`NORMAL_SPAN`] either side and the sections that far
    /// either side along X, as the drive smooths it (OI-67); the facet
    /// normal where a neighbour is missing or the result leaves it by more
    /// than 60 degrees.
    fn contour_normal(
        &self,
        chain: &Chain,
        at: f64,
        p: Point3,
        facet: Vec3,
        beside: &(Vec<Chain>, Vec<Chain>),
    ) -> Option<UnitVec3> {
        let facet = UnitVec3::new(facet).ok()?;
        let along = |s: f64| chain.at(s).map(|q| q.0);
        let open_end = !chain.closed && (at < NORMAL_SPAN || at > chain.length() - NORMAL_SPAN);
        let (Some(fwd), Some(back)) = (along(at + NORMAL_SPAN), along(at - NORMAL_SPAN)) else {
            return Some(facet);
        };
        let nearest = |chains: &[Chain]| {
            chains
                .iter()
                .filter_map(|c| c.nearest(p))
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .filter(|(_, d)| *d <= 2.0 * NORMAL_SPAN)
                .map(|(q, _)| q)
        };
        let (Some(lo), Some(hi)) = (nearest(&beside.0), nearest(&beside.1)) else {
            return Some(facet);
        };
        if open_end {
            return Some(facet);
        }
        let n = (hi - lo).cross(fwd - back);
        let n = if n.dot(facet.get()) < 0.0 { -n } else { n };
        match UnitVec3::new(n) {
            Ok(u) if u.get().dot(facet.get()) > 0.5 => Some(u),
            _ => Some(facet),
        }
    }

    /// Whether the tool with its axis at `phi` (degrees across X) reaches
    /// `p`: dropped along that axis from where its point with the normal
    /// `-n` would sit on `p`, it rests on `p` to within the tolerance, and
    /// its shank does not rest higher.
    fn reaches(&self, p: Point3, n: UnitVec3, phi: f64) -> bool {
        let a = Degrees(self.sense * (90.0 - phi));
        let axis = self.input.rotary.tool_axis(a);
        let line = ToolLine {
            origin: self.seed_tip(p, n, axis),
            axis,
        };
        // Placed touching `p`, the tool rests there or higher.
        let surface = self.input.surface;
        let Some(c) = surface.drop_tool_above(self.drop_profile, &line, 0.0) else {
            return false;
        };
        let d = p - c.tip;
        let z = d.dot(axis.get());
        let r = (d - axis.get() * z).length();
        if to_tool(self.drop_profile, r, z) > self.tolerance {
            return false;
        }
        match self.body {
            Some(body) => surface
                .drop_tool_above(body, &line, 0.0)
                .is_some_and(|b| b.offset <= c.offset + SHANK_SLACK * self.tolerance),
            None => true,
        }
    }

    /// Whether the shank, where wider than the cutting part, rests above
    /// the cutting part's drop on `line`: it would meet the part there.
    /// `at_least`, where known, is where the tool rests at least (see
    /// [`SurfaceSource::drop_tool_above`](crate::surface::SurfaceSource::drop_tool_above)).
    pub(super) fn shank_clashes(&self, line: &ToolLine, at_least: Option<f64>) -> bool {
        let Some(body) = self.body else {
            return false;
        };
        let surface = self.input.surface;
        let drop = |profile: &RevolvedProfile| match at_least {
            Some(floor) => surface.drop_tool_above(profile, line, floor),
            None => surface.drop_tool(profile, line),
        };
        match (drop(self.drop_profile), drop(body)) {
            (Some(c), Some(b)) => b.offset > c.offset + SHANK_SLACK * self.tolerance,
            _ => false,
        }
    }

    /// The axis angles that reach `p`: the one nearest `target` within
    /// `max_lean` of the normal's angle `phi_n`, and the band within
    /// `max_tilt` of it that still reaches, as `(best, low, high)`; or why
    /// none does. `warm` is where the point before was reached.
    #[allow(clippy::too_many_arguments)]
    fn reach(
        &self,
        p: Point3,
        n: UnitVec3,
        phi_n: f64,
        target: f64,
        max_lean: f64,
        max_tilt: f64,
        warm: Option<f64>,
    ) -> Result<(f64, f64, f64), UnreachedReason> {
        let (w0, w1) = (phi_n - max_lean, phi_n + max_lean);
        let inside = |phi: f64| phi >= w0 - 1e-9 && phi <= w1 + 1e-9;
        let test = |phi: f64| self.reaches(p, n, phi);
        // The boundary between a good angle and a bad one, nearer the bad.
        let edge = |good: f64, bad: f64| {
            let (mut g, mut b) = (good, bad);
            for _ in 0..SEARCH_HALVINGS {
                let m = 0.5 * (g + b);
                if test(m) { g = m } else { b = m }
            }
            g
        };
        let t0 = target.clamp(w0, w1);
        // From a good angle towards the target a step at a time, then the
        // edge within the last step.
        let towards = |good: f64| {
            let dir = (t0 - good).signum();
            let mut good = good;
            loop {
                let next = good + dir * SEARCH_STEP;
                if (next - t0) * dir >= 0.0 {
                    return edge(good, t0);
                }
                if test(next) {
                    good = next;
                } else {
                    return edge(good, next);
                }
            }
        };
        let best = if test(t0) {
            Some(t0)
        } else {
            let mut found = warm.map(|w| near(w, t0)).filter(|w| inside(*w) && test(*w));
            if found.is_none() {
                let mut k = 1.0;
                'scan: while k * SEARCH_STEP <= 2.0 * max_lean + SEARCH_STEP {
                    for sign in [1.0, -1.0] {
                        let c = t0 + sign * k * SEARCH_STEP;
                        if inside(c) && test(c) {
                            found = Some(c);
                            break 'scan;
                        }
                    }
                    k += 1.0;
                }
            }
            found.map(towards)
        };
        let Some(best) = best else {
            return Err(self.why(p, n, phi_n, max_lean));
        };
        // An edge found by halving lies within a fraction of a degree of
        // angles that do not reach: the axis keeps `GATE_MARGIN` inside it,
        // as the reach between samples changes.
        let away = (best - t0).signum();
        let best = if away != 0.0 && test(best + away * GATE_MARGIN) {
            best + away * GATE_MARGIN
        } else {
            best
        };
        let band = |far: f64| {
            let far = far.clamp(w0, w1);
            let edge = if (far - best).abs() < 1e-12 || test(far) {
                far
            } else {
                edge(best, far)
            };
            // Kept inside by the margin, never past the best angle.
            let inward = (best - edge).signum() * GATE_MARGIN;
            if (best - edge).abs() > GATE_MARGIN {
                edge + inward
            } else {
                best
            }
        };
        if max_tilt <= 0.0 {
            return Ok((best, best, best));
        }
        if away == 0.0 {
            Ok((best, band(best - max_tilt), band(best + max_tilt)))
        } else {
            let other = band(best + away * max_tilt);
            Ok((best, best.min(other), best.max(other)))
        }
    }

    /// Why no axis within `max_lean` of the normal reaches `p`.
    fn why(&self, p: Point3, n: UnitVec3, phi_n: f64, max_lean: f64) -> UnreachedReason {
        if max_lean < MAX_LEAN {
            let mut lean = max_lean + SEARCH_STEP;
            while lean <= MAX_LEAN + 1e-9 {
                for sign in [1.0, -1.0] {
                    if self.reaches(p, n, phi_n + sign * lean) {
                        return UnreachedReason::NeedsLean { degrees: lean };
                    }
                }
                lean += SEARCH_STEP;
            }
        }
        // A straight line across X from outside to the point, within a
        // right angle of its normal, with nothing of the part on it.
        let seen = (-17..=17).any(|k| {
            let th = (phi_n + f64::from(k) * SEARCH_STEP).to_radians();
            let d = Vec3::new(0.0, th.cos(), th.sin());
            let far = self.outside + p.y.hypot(p.z) + 1.0;
            let origin = p + d * far;
            UnitVec3::new(-d).ok().is_some_and(|dir| {
                self.input
                    .surface
                    .cast_ray(origin, dir, far + 1.0)
                    .is_some_and(|h| (h.point - p).length() <= VISIBLE_GAP)
            })
        });
        if seen {
            UnreachedReason::TooNarrow
        } else {
            UnreachedReason::Blocked
        }
    }

    /// The areas the tool cannot reach: the unreached points of the
    /// contours, less the corners beside points the tool finished, grouped
    /// by reason into areas of neighbouring points.
    fn areas(&self, contours: &[Contour], dx: f64, settings: Settings) -> Vec<Unreached> {
        // (contour, sample) of every point to report.
        let mut kept: Vec<(usize, usize)> = Vec::new();
        for (c, contour) in contours.iter().enumerate() {
            let corners = self.corners(contour);
            for (i, s) in contour.samples.iter().enumerate() {
                let Status::Unreached(reason) = s.status else {
                    continue;
                };
                if s.probe
                    || (reason == UnreachedReason::TooNarrow
                        && corners.get(i).copied().unwrap_or(false))
                {
                    continue;
                }
                if !self.in_scope(s.point) {
                    continue;
                }
                kept.push((c, i));
            }
        }
        let sample = |k: &(usize, usize)| {
            contours
                .get(k.0)
                .and_then(|c| c.samples.get(k.1).map(|s| (c, s)))
        };
        let kind = |r: UnreachedReason| match r {
            UnreachedReason::Blocked => 0,
            UnreachedReason::TooNarrow => 1,
            UnreachedReason::NeedsLean { .. } => 2,
        };
        // Neighbours: the same reason, on the same or a neighbouring
        // section, near enough.
        let near_mm = 1.5 * dx.max(REACH_SPACING);
        let mut parent: Vec<usize> = (0..kept.len()).collect();
        let find = |parent: &mut Vec<usize>, mut i: usize| {
            while let Some(&p) = parent.get(i) {
                if p == i {
                    break;
                }
                i = p;
            }
            i
        };
        for i in 0..kept.len() {
            let Some((ci, si)) = kept.get(i).and_then(sample) else {
                continue;
            };
            let Status::Unreached(ri) = si.status else {
                continue;
            };
            for j in i + 1..kept.len() {
                let Some((cj, sj)) = kept.get(j).and_then(sample) else {
                    continue;
                };
                let Status::Unreached(rj) = sj.status else {
                    continue;
                };
                if kind(ri) != kind(rj)
                    || ci.section.abs_diff(cj.section) > 1
                    || (si.point - sj.point).length() > near_mm
                {
                    continue;
                }
                let (a, b) = (find(&mut parent, i), find(&mut parent, j));
                if a != b
                    && let Some(slot) = parent.get_mut(a.max(b))
                {
                    *slot = a.min(b);
                }
            }
        }
        let mut groups: Vec<Vec<usize>> = Vec::new();
        let mut index: BTreeMap<usize, usize> = BTreeMap::new();
        for i in 0..kept.len() {
            let root = find(&mut parent, i);
            let g = *index.entry(root).or_insert_with(|| {
                groups.push(Vec::new());
                groups.len() - 1
            });
            if let Some(list) = groups.get_mut(g) {
                list.push(i);
            }
        }
        let mut out: Vec<Unreached> = groups
            .into_iter()
            .filter(|g| g.len() >= MIN_REPORTED_POINTS)
            .filter_map(|g| {
                let members: Vec<(&Contour, &Sample)> = g
                    .iter()
                    .filter_map(|i| kept.get(*i).and_then(sample))
                    .collect();
                let (first_c, first_s) = members.first().copied()?;
                let Status::Unreached(reason) = first_s.status else {
                    return None;
                };
                let reason = match reason {
                    UnreachedReason::NeedsLean { .. } => {
                        let most = members
                            .iter()
                            .filter_map(|(_, s)| match s.status {
                                Status::Unreached(UnreachedReason::NeedsLean { degrees }) => {
                                    Some(degrees)
                                }
                                _ => None,
                            })
                            .fold(0.0_f64, f64::max);
                        UnreachedReason::NeedsLean { degrees: most }
                    }
                    other => other,
                };
                let (mut x0, mut x1) = (first_c.x, first_c.x);
                let mut angles = Vec::with_capacity(members.len());
                let mut area = 0.0;
                for (c, s) in &members {
                    x0 = x0.min(c.x);
                    x1 = x1.max(c.x);
                    let a = (self.sense * (90.0 - s.phi_n)).rem_euclid(TURN);
                    angles.push(a);
                    area += dx * c.length() / (c.samples.len().saturating_sub(1).max(1)) as f64;
                }
                Some(Unreached {
                    x: (
                        (x0 - 0.5 * dx).max(settings.x0),
                        (x1 + 0.5 * dx).min(settings.x1),
                    ),
                    a: arc_of(&mut angles),
                    area,
                    reason,
                })
            })
            .collect();
        out.sort_by(|a, b| {
            b.area
                .total_cmp(&a.area)
                .then(a.x.0.total_cmp(&b.x.0))
                .then(a.a.0.total_cmp(&b.a.0))
        });
        out
    }

    /// Which samples of `contour` lie in a concave corner the tool's round
    /// end cannot fill: a run of samples it does not reach, with samples it
    /// reaches on both sides no more than [`CORNER_SPAN`] tool radii apart
    /// along the section, round which the surface turns inwards by more
    /// than [`CORNER_TURN`]. Rest machining finishes such corners (OP-09),
    /// as on the surfaces the drive meets; a slot narrower than the tool is
    /// longer, and the side of a pin facing another turns outwards.
    fn corners(&self, contour: &Contour) -> Vec<bool> {
        let samples = &contour.samples;
        let mut out = vec![false; samples.len()];
        let mut i = 0;
        while i < samples.len() {
            if samples.get(i).is_none_or(|s| s.gate.is_some()) {
                i += 1;
                continue;
            }
            let start = i;
            while samples.get(i).is_some_and(|s| s.gate.is_none()) {
                i += 1;
            }
            let (Some(before), Some(after)) = (
                start.checked_sub(1).and_then(|k| samples.get(k)),
                samples.get(i),
            ) else {
                continue;
            };
            let span = (after.t - before.t) * contour.length();
            let turn = contour.sense * (after.phi_n - before.phi_n);
            if span <= CORNER_SPAN * self.reach && turn < -CORNER_TURN {
                for slot in out.iter_mut().take(i).skip(start) {
                    *slot = true;
                }
            }
        }
        out
    }

    /// Whether the operation's region, boundary and avoid sets let a
    /// strategy cut at `p` (OP-02): an area outside them is not reported.
    fn in_scope(&self, p: Point3) -> bool {
        let Some(region) = self.region else {
            return true;
        };
        let on =
            |s: &dyn crate::surface::SurfaceSource| s.within(p, VISIBLE_GAP + s.drop_accuracy());
        region.region.is_none_or(on)
            && region.boundary.as_ref().is_none_or(|b| b.contains(p))
            && !region.avoid.is_some_and(on)
    }
}

/// The largest lean, degrees: the tool's side.
const MAX_LEAN: f64 = 90.0;

/// The smallest arc, `(low, high)` in degrees, holding every angle in
/// `angles` (each in 0 to 360); `(0, 360)` when they go all round.
fn arc_of(angles: &mut [f64]) -> (f64, f64) {
    angles.sort_by(f64::total_cmp);
    let (Some(&first), Some(&last)) = (angles.first(), angles.last()) else {
        return (0.0, 0.0);
    };
    // The largest gap between neighbours, round through 360.
    let mut gap = first + TURN - last;
    let mut after = first;
    for w in angles.windows(2) {
        if let [a, b] = w
            && b - a > gap
        {
            gap = b - a;
            after = *b;
        }
    }
    if gap <= ALL_ROUND_GAP {
        return (0.0, TURN);
    }
    (after, after + TURN - gap)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]
    use super::*;

    fn square(x0: f64, x1: f64) -> TriangleMesh {
        // A box X x0..x1, Y and Z -1..1 (a prism of a square section).
        let section = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)];
        let mut vertices = Vec::new();
        for x in [x0, x1] {
            vertices.extend(section.iter().map(|&(y, z)| Point3::new(x, y, z)));
        }
        let mut triangles = Vec::new();
        for i in 0..4u32 {
            let j = (i + 1) % 4;
            triangles.push([i, j, 4 + j]);
            triangles.push([i, 4 + j, 4 + i]);
        }
        triangles.extend([[4, 5, 6], [4, 6, 7], [0, 2, 1], [0, 3, 2]]);
        TriangleMesh {
            vertices,
            triangles,
        }
    }

    #[test]
    fn req_op_20_a_section_is_one_closed_chain_counter_clockwise_with_outward_normals() {
        let mesh = square(0.0, 4.0);
        let chains = Slicer::new(&mesh).chains(1.3);
        assert_eq!(chains.len(), 1);
        let c = &chains[0];
        assert!(c.closed);
        assert!((c.length() - 8.0).abs() < 1e-12, "{}", c.length());
        // Counter-clockwise seen from +X: the signed area is positive.
        let area: f64 = c
            .points
            .iter()
            .zip(c.points.iter().cycle().skip(1))
            .map(|(a, b)| a.y * b.z - b.y * a.z)
            .sum();
        assert!(area > 0.0, "{area}");
        // Each segment's facet normal points away from the middle.
        for (k, n) in c.normals.iter().enumerate() {
            let mid = c.at(c.s[k] + 1e-6).unwrap().0;
            assert!(n.dot(Vec3::new(0.0, mid.y, mid.z)) > 0.0, "{k}");
        }
        // A plane through a ring of vertices, and beyond the part.
        assert_eq!(Slicer::new(&mesh).chains(4.0).len(), 1);
        assert!(Slicer::new(&mesh).chains(0.0).len() <= 1);
        assert!(Slicer::new(&mesh).chains(9.0).is_empty());
        assert!(Slicer::new(&mesh).chains(-9.0).is_empty());
    }

    #[test]
    fn req_op_20_an_open_mesh_gives_an_open_chain_and_points_along_it() {
        let mut mesh = square(0.0, 4.0);
        // Remove one side's two triangles: the section is open there.
        mesh.triangles.drain(0..2);
        let chains = Slicer::new(&mesh).chains(2.0);
        assert_eq!(chains.len(), 1);
        let c = &chains[0];
        assert!(!c.closed);
        assert!((c.length() - 6.0).abs() < 1e-12);
        let (p, _) = c.at(-1.0).unwrap();
        assert_eq!(p, c.points[0]);
        let (q, _) = c.at(99.0).unwrap();
        assert_eq!(q, *c.points.last().unwrap());
        // Each side is two triangles, so each gives two segments.
        assert_eq!(c.vertices_between(0.0, 6.0).len(), c.points.len() - 2);
        assert_eq!(c.vertices_between(1.5, 2.5).len(), 1);
        // The open side is the bottom (Z -1): the top is nearest above.
        let (n, d) = c.nearest(Point3::new(2.0, 0.0, 3.0)).unwrap();
        assert!((d - 2.0).abs() < 1e-12 && (n.z - 1.0).abs() < 1e-12);
    }

    #[test]
    fn req_op_20_an_area_round_the_axis_is_all_round_and_a_patch_is_not() {
        let mut ring: Vec<f64> = (0..72).map(|k| f64::from(k) * 5.0).collect();
        assert_eq!(arc_of(&mut ring), (0.0, 360.0));
        let mut patch = vec![350.0, 355.0, 2.0, 8.0];
        let (lo, hi) = arc_of(&mut patch);
        assert_eq!((lo, hi), (350.0, 368.0));
        assert_eq!(arc_of(&mut []), (0.0, 0.0));
    }

    #[test]
    fn req_op_20_the_body_adds_a_wider_shank_above_the_flutes() {
        let ball = openchuck_types::tool::Tool {
            shape: openchuck_types::tool::ToolShape::BallEnd,
            diameter: openchuck_types::units::Millimetres(1.0),
            flute_length: openchuck_types::units::Millimetres(1.5),
            overall_length: openchuck_types::units::Millimetres(40.0),
            shank_diameter: openchuck_types::units::Millimetres(3.0),
            flute_count: 2,
            material: None,
            centre_cutting: None,
        }
        .profile()
        .unwrap();
        let shank = Shank {
            radius: 1.5,
            height: 1.5,
        };
        let b = body(&ball, Some(shank), 0.0).unwrap();
        assert_eq!(b.end(), ProfilePoint::new(1.5, 1.5));
        assert!((to_tool(&b, 1.5, 3.0)).abs() < 1e-12);
        assert!((to_tool(&b, 0.0, 0.0)).abs() < 1e-12);
        // No wider shank, or none given: no body of its own.
        assert!(body(&ball, None, 0.0).is_none());
        assert!(
            body(
                &ball,
                Some(Shank {
                    radius: 0.5,
                    height: 1.5
                }),
                0.0
            )
            .is_none()
        );
    }
}
