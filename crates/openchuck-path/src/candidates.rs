// SPDX-License-Identifier: AGPL-3.0-or-later
//! Where on a pass the cutter contact can lie inside a selected region (OP-02;
//! work breakdown slice 12 item 5; decision record 0034 item 5).
//!
//! A pass is a line in the development of the rotary axis: `x` along it and
//! `u = R * A` round it (R the part's largest radius, A in radians; the
//! development of [`crate::unroll`]). A selected region is a set of triangles,
//! each a convex polygon there. The cutter contact is never further from the
//! tool axis than the tool reaches, so a pass can touch the region only where
//! its line is within that reach of a triangle: the region **offset outward**
//! by the reach. The offset of a union is the union of the offsets, and the
//! offset of a convex triangle is convex, so the pass meets each triangle's
//! offset in one interval, found by minimising the distance along the line.
//! That needs no loop assembly, no seam rule (the triangles are tiled round
//! the period) and no offset library, and it holds for holes and for regions
//! that go all the way round.
//!
//! The strategy then drops the tool only inside these intervals, as finely as
//! the toolpath tolerance asks, and judges the real contact there
//! ([`crate::RegionConstraint::allows`]). Outside them the contact cannot be
//! in the region, so nothing is sampled: a feature of the selection narrower
//! than the old 0.1 mm spacing is found because the line crosses its offset
//! (OI-138), and a pass that never comes near the region costs nothing.

use openchuck_types::geometry::Point3;
use openchuck_types::mesh::TriangleMesh;

use crate::rotary::RotaryMapping;

/// A line in the development: `x` and `u` at parameter `t` from 0 to 1.
#[derive(Debug, Clone, Copy)]
pub(crate) struct DevLine {
    pub(crate) x0: f64,
    pub(crate) dx: f64,
    pub(crate) u0: f64,
    pub(crate) du: f64,
}

/// A triangle in the development with the distance within which a pass
/// may touch it.
#[derive(Debug, Clone, Copy)]
struct DevTri {
    /// Corners (x, u), with u unwrapped relative to the first.
    pts: [(f64, f64); 3],
    /// How far from the triangle the tool axis can be with the contact on it.
    reach: f64,
    /// Every u: the triangle is too near the rotary axis for angle to limit it.
    strip: bool,
    /// Bounding box of the corners.
    x: (f64, f64),
    u: (f64, f64),
}

/// The triangles of a region in the development.
#[derive(Debug, Clone)]
pub(crate) struct DevIndex {
    tris: Vec<DevTri>,
    period: f64,
    /// The radius the development is made at.
    radius: f64,
    /// The triangles by where they reach round the period (issue #382).
    cells: Cells,
}

/// The triangles that can come within reach of a pass at each part of the
/// period in `u`: the period is cut into `count` cells, and a triangle is in
/// every cell its `u` range, widened by its reach, meets once wrapped round
/// the period, and in the cells either side, so rounding at a cell's edge
/// cannot leave it out. A strip, or a triangle that reaches round the whole
/// period, is in `always`. A pass that covers less than half the period
/// looks only in the cells it crosses: every triangle it could reach is
/// there, so the intervals are the same as from every triangle.
#[derive(Debug, Clone)]
struct Cells {
    count: usize,
    width: f64,
    /// Where each cell's triangles start in `items`; one more than `count`.
    start: Vec<usize>,
    items: Vec<u32>,
    always: Vec<u32>,
}

impl Cells {
    fn build(tris: &[DevTri], period: f64) -> Self {
        let spans: Vec<Option<(f64, f64)>> = tris
            .iter()
            .map(|t| {
                let (lo, hi) = (t.u.0 - t.reach, t.u.1 + t.reach);
                (!t.strip && hi - lo < 0.5 * period).then_some((lo, hi))
            })
            .collect();
        // Cells about as wide as a triangle's span, so each is in a few.
        let (sum, n) = spans
            .iter()
            .flatten()
            .fold((0.0, 0_usize), |(s, n), (lo, hi)| (s + (hi - lo), n + 1));
        let mean = if n > 0 { sum / n as f64 } else { period };
        let count = if mean > 0.0 {
            (period / mean).floor().clamp(1.0, 65_536.0) as usize
        } else {
            1
        };
        let width = period / count as f64;
        let cell_of = |u: f64| ((u.rem_euclid(period) / width).floor() as usize).min(count - 1);
        // The cells a span meets, with one either side, wrapping round.
        let range = |(lo, hi): (f64, f64)| {
            let first = cell_of(lo) + count - 1;
            let steps = ((hi - lo) / width).ceil() as usize + 3;
            (first..first + steps.min(count)).map(move |c| c % count)
        };
        let mut counts = vec![0_usize; count + 1];
        let mut always = Vec::new();
        for (i, span) in spans.iter().enumerate() {
            match span {
                Some(sp) => range(*sp).for_each(|c| {
                    if let Some(n) = counts.get_mut(c + 1) {
                        *n += 1;
                    }
                }),
                None => always.push(i as u32),
            }
        }
        for c in 1..counts.len() {
            let previous = counts.get(c - 1).copied().unwrap_or(0);
            if let Some(n) = counts.get_mut(c) {
                *n += previous;
            }
        }
        let start = counts.clone();
        let mut fill = counts;
        let mut items = vec![0_u32; start.last().copied().unwrap_or(0)];
        for (i, span) in spans.iter().enumerate() {
            if let Some(sp) = span {
                for c in range(*sp) {
                    if let Some(at) = fill.get_mut(c)
                        && let Some(slot) = items.get_mut(*at)
                    {
                        *slot = i as u32;
                        *at += 1;
                    }
                }
            }
        }
        Self {
            count,
            width,
            start,
            items,
            always,
        }
    }

    /// The triangles, in index order, that a line over `[lu0, lu1]` in `u`
    /// could reach; `None` when the line covers half the period or more (then
    /// every triangle is tried).
    fn near(&self, lu0: f64, lu1: f64, period: f64) -> Option<Vec<u32>> {
        if !(lu1 - lu0 < 0.5 * period) {
            return None;
        }
        let cell_of =
            |u: f64| ((u.rem_euclid(period) / self.width).floor() as usize).min(self.count - 1);
        let first = cell_of(lu0) + self.count - 1;
        let steps = (((lu1 - lu0) / self.width).ceil() as usize + 3).min(self.count);
        let mut out = self.always.clone();
        for c in (first..first + steps).map(|c| c % self.count) {
            let (Some(a), Some(b)) = (self.start.get(c), self.start.get(c + 1)) else {
                continue;
            };
            out.extend(self.items.get(*a..*b).unwrap_or_default());
        }
        out.sort_unstable();
        out.dedup();
        Some(out)
    }
}

fn wrap(d: f64, period: f64) -> f64 {
    let m = d.rem_euclid(period);
    if m > 0.5 * period { m - period } else { m }
}

fn dist_point_segment(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let (abx, aby) = (b.0 - a.0, b.1 - a.1);
    let len2 = abx * abx + aby * aby;
    let t = if len2 > 0.0 {
        (((p.0 - a.0) * abx + (p.1 - a.1) * aby) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (p.0 - (a.0 + t * abx)).hypot(p.1 - (a.1 + t * aby))
}

fn inside(p: (f64, f64), t: &[(f64, f64); 3]) -> bool {
    let side = |a: (f64, f64), b: (f64, f64)| (b.0 - a.0) * (p.1 - a.1) - (b.1 - a.1) * (p.0 - a.0);
    let (d0, d1, d2) = (side(t[0], t[1]), side(t[1], t[2]), side(t[2], t[0]));
    (d0 >= 0.0 && d1 >= 0.0 && d2 >= 0.0) || (d0 <= 0.0 && d1 <= 0.0 && d2 <= 0.0)
}

/// Distance from `p` to the triangle `t`.
fn dist_point_triangle(p: (f64, f64), t: &[(f64, f64); 3]) -> f64 {
    if inside(p, t) {
        return 0.0;
    }
    dist_point_segment(p, t[0], t[1])
        .min(dist_point_segment(p, t[1], t[2]))
        .min(dist_point_segment(p, t[2], t[0]))
}

impl DevIndex {
    /// The triangles of `mesh` in the development of the rotary axis with
    /// radius `radius` (the part's largest), for a tool that reaches `reach`
    /// from its axis. `None` when the mesh is empty or `radius` is not
    /// positive.
    pub(crate) fn build(
        mesh: &TriangleMesh,
        rotary: &RotaryMapping,
        radius: f64,
        reach: f64,
    ) -> Option<Self> {
        if !(radius.is_finite() && radius > 0.0 && reach.is_finite() && reach >= 0.0) {
            return None;
        }
        let period = std::f64::consts::TAU * radius;
        // A at which the tool faces the polar angle `phi` (radians), as u.
        let u_of = |p: Point3| -> Option<f64> {
            let a = rotary.a_facing(openchuck_types::geometry::Vec3::new(0.0, p.y, p.z))?;
            Some(radius * a.0.to_radians())
        };
        let mut tris = Vec::with_capacity(mesh.triangles.len());
        for t in &mesh.triangles {
            let [p0, p1, p2] = t.map(|i| mesh.vertices.get(i as usize).copied());
            let (p0, p1, p2) = (p0?, p1?, p2?);
            let yz = |p: Point3| (p.y, p.z);
            let flat = [yz(p0), yz(p1), yz(p2)];
            let rho_min = dist_point_triangle((0.0, 0.0), &flat);
            let xs = (p0.x.min(p1.x).min(p2.x), p0.x.max(p1.x).max(p2.x));
            let strip = rho_min <= reach;
            let us = [u_of(p0), u_of(p1), u_of(p2)];
            let (pts, u) = match (strip, us) {
                (false, [Some(u0), Some(u1), Some(u2)]) => {
                    let pts = [
                        (p0.x, u0),
                        (p1.x, u0 + wrap(u1 - u0, period)),
                        (p2.x, u0 + wrap(u2 - u0, period)),
                    ];
                    let lo = pts.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
                    let hi = pts.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max);
                    (pts, (lo, hi))
                }
                _ => ([(p0.x, 0.0), (p1.x, 0.0), (p2.x, 0.0)], (0.0, 0.0)),
            };
            let strip = strip || us.iter().any(Option::is_none);
            // The tool axis lies through the rotary axis, so a contact at
            // radius rho is within reach of it when rho * sin(angle) <= reach.
            let angular = if strip {
                0.0
            } else {
                radius * (reach / rho_min).min(1.0).asin()
            };
            tris.push(DevTri {
                pts,
                reach: reach.max(angular),
                strip,
                x: xs,
                u,
            });
        }
        let cells = Cells::build(&tris, period);
        (!tris.is_empty()).then_some(Self {
            tris,
            period,
            radius,
            cells,
        })
    }

    /// The radius the development is made at.
    pub(crate) fn radius(&self) -> f64 {
        self.radius
    }

    /// The parameter intervals, merged and in order, where `line` is within
    /// reach of some triangle.
    pub(crate) fn intervals(&self, line: &DevLine) -> Vec<(f64, f64)> {
        let (lu0, lu1) = (
            line.u0.min(line.u0 + line.du),
            line.u0.max(line.u0 + line.du),
        );
        let mut found = Union::default();
        // The triangles the line passes nearest first: their intervals are
        // the longest, so the others are mostly inside them already and are
        // skipped. The order changes the work, never the answer.
        let near = self.cells.near(lu0, lu1, self.period).map(|mut near| {
            let (mid, half) = (0.5 * (lu0 + lu1), 0.5 * (lu1 - lu0));
            let gap = |i: u32| {
                self.tris.get(i as usize).map_or(f64::INFINITY, |t| {
                    let centre = wrap(0.5 * (t.u.0 + t.u.1) - mid, self.period).abs();
                    (centre - 0.5 * (t.u.1 - t.u.0) - half).max(0.0)
                })
            };
            let mut keyed: Vec<(f64, u32)> = near.drain(..).map(|i| (gap(i), i)).collect();
            keyed.sort_unstable_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
            near.extend(keyed.into_iter().map(|(_, i)| i));
            near
        });
        let tris: Box<dyn Iterator<Item = &DevTri>> = match &near {
            Some(near) => Box::new(near.iter().filter_map(|&i| self.tris.get(i as usize))),
            None => Box::new(self.tris.iter()),
        };
        for tri in tris {
            let g = tri.reach;
            // The line's range of t where x is within g of the triangle's.
            let Some((ta, tb)) = slab(line.x0, line.dx, tri.x.0 - g, tri.x.1 + g, 0.0, 1.0) else {
                continue;
            };
            // Every interval of the triangle lies in `[ta, tb]`; when that is
            // already found, the triangle cannot change the answer, so its
            // distances are not worked out (issue #382: on a pass across a
            // large region, nearly every triangle is like this).
            if found.covers(ta, tb) {
                continue;
            }
            if tri.strip {
                found.add(ta, tb);
                continue;
            }
            let k0 = ((lu0 - (tri.u.1 + g)) / self.period).ceil() as i64;
            let k1 = ((lu1 - (tri.u.0 - g)) / self.period).floor() as i64;
            for k in k0..=k1 {
                let shift = k as f64 * self.period;
                let Some((sa, sb)) =
                    slab(line.u0 - shift, line.du, tri.u.0 - g, tri.u.1 + g, ta, tb)
                else {
                    continue;
                };
                if let Some((a, b)) = self.within(tri, line, shift, sa, sb) {
                    found.add(a, b);
                }
            }
        }
        found.groups
    }

    /// The part of `[ta, tb]` where the line, with the triangle moved by
    /// `shift` in u, is within the triangle's reach. The distance along the
    /// line is convex, so it is found by minimising and then bisecting.
    fn within(
        &self,
        tri: &DevTri,
        line: &DevLine,
        shift: f64,
        ta: f64,
        tb: f64,
    ) -> Option<(f64, f64)> {
        let moved = [
            (tri.pts[0].0, tri.pts[0].1 + shift),
            (tri.pts[1].0, tri.pts[1].1 + shift),
            (tri.pts[2].0, tri.pts[2].1 + shift),
        ];
        let f =
            |t: f64| dist_point_triangle((line.x0 + line.dx * t, line.u0 + line.du * t), &moved);
        let g = tri.reach;
        // Ternary search for the nearest point.
        let (mut lo, mut hi) = (ta, tb);
        for _ in 0..48 {
            let (m1, m2) = (lo + (hi - lo) / 3.0, hi - (hi - lo) / 3.0);
            if f(m1) <= f(m2) {
                hi = m2;
            } else {
                lo = m1;
            }
        }
        let best = 0.5 * (lo + hi);
        if f(best) > g {
            return None;
        }
        let edge = |near: f64, far: f64| {
            if f(far) <= g {
                return far;
            }
            let (mut inside, mut outside) = (near, far);
            for _ in 0..48 {
                let mid = 0.5 * (inside + outside);
                if f(mid) <= g {
                    inside = mid;
                } else {
                    outside = mid;
                }
            }
            inside
        };
        Some((edge(best, ta), edge(best, tb)))
    }
}

/// Intervals joined where they overlap or are no more than [`JOIN`] apart,
/// kept in order. Joining as each interval comes gives the same groups, bit
/// for bit, as sorting them all and joining in one sweep: a group is a set
/// of intervals linked by those rules whatever order they come in, and its
/// ends are the least start and the greatest end, which do not depend on
/// the order either.
#[derive(Debug, Default)]
struct Union {
    /// Disjoint, in order, each more than [`JOIN`] from the next.
    groups: Vec<(f64, f64)>,
}

/// Two intervals this close are one (the gap a sweep would close).
const JOIN: f64 = 1e-12;

impl Union {
    /// Whether `[a, b]` lies in one group. Adding such an interval changes
    /// nothing: it adds no point and closes no gap.
    fn covers(&self, a: f64, b: f64) -> bool {
        let k = self.groups.partition_point(|g| g.0 <= a);
        k.checked_sub(1)
            .and_then(|k| self.groups.get(k))
            .is_some_and(|g| b <= g.1)
    }

    /// Adds `[a, b]`, joining it to every group it overlaps or comes within
    /// [`JOIN`] of.
    fn add(&mut self, a: f64, b: f64) {
        // The first group that ends no more than JOIN before `a` (the groups
        // are in order of their starts and so of their ends), and the groups
        // from there that start no more than JOIN after `b`.
        let first = self.groups.partition_point(|g| g.1 + JOIN < a);
        let after = first
            + self
                .groups
                .get(first..)
                .map_or(0, |rest| rest.partition_point(|g| g.0 <= b + JOIN));
        let (mut lo, mut hi) = (a, b);
        if let (Some(f), Some(l)) = (
            self.groups.get(first),
            after.checked_sub(1).and_then(|l| self.groups.get(l)),
        ) && after > first
        {
            lo = lo.min(f.0);
            hi = hi.max(l.1);
        }
        self.groups.splice(first..after, [(lo, hi)]);
    }
}

/// The part of `[ta, tb]` where `p0 + d * t` lies in `[lo, hi]`.
fn slab(p0: f64, d: f64, lo: f64, hi: f64, ta: f64, tb: f64) -> Option<(f64, f64)> {
    if d.abs() < 1e-15 {
        return (p0 >= lo && p0 <= hi).then_some((ta, tb));
    }
    let (t1, t2) = ((lo - p0) / d, (hi - p0) / d);
    let (a, b) = (t1.min(t2).max(ta), t1.max(t2).min(tb));
    (a <= b).then_some((a, b))
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::float_cmp
)]
mod tests {
    use super::*;
    use openchuck_types::machine::MachineProfile;

    /// A deterministic stream of numbers in [0, 1).
    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self) -> f64 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (self.0 >> 11) as f64 / (1u64 << 53) as f64
        }
    }

    /// Whether the line is within reach of some triangle at `t`, by trying
    /// every tile of every triangle.
    fn near(index: &DevIndex, line: &DevLine, t: f64) -> bool {
        let p = (line.x0 + line.dx * t, line.u0 + line.du * t);
        index.tris.iter().any(|tri| {
            if tri.strip {
                return p.0 >= tri.x.0 - tri.reach && p.0 <= tri.x.1 + tri.reach;
            }
            (-12..=12).any(|k| {
                let s = k as f64 * index.period;
                let moved = [
                    (tri.pts[0].0, tri.pts[0].1 + s),
                    (tri.pts[1].0, tri.pts[1].1 + s),
                    (tri.pts[2].0, tri.pts[2].1 + s),
                ];
                dist_point_triangle(p, &moved) <= tri.reach
            })
        })
    }

    /// The intervals as worked out before issue #382: every triangle, every
    /// tile, then one sort and one sweep.
    fn intervals_by_sweep(index: &DevIndex, line: &DevLine) -> Vec<(f64, f64)> {
        let (lu0, lu1) = (
            line.u0.min(line.u0 + line.du),
            line.u0.max(line.u0 + line.du),
        );
        let mut found: Vec<(f64, f64)> = Vec::new();
        for tri in &index.tris {
            let g = tri.reach;
            let Some((ta, tb)) = slab(line.x0, line.dx, tri.x.0 - g, tri.x.1 + g, 0.0, 1.0) else {
                continue;
            };
            if tri.strip {
                found.push((ta, tb));
                continue;
            }
            let k0 = ((lu0 - (tri.u.1 + g)) / index.period).ceil() as i64;
            let k1 = ((lu1 - (tri.u.0 - g)) / index.period).floor() as i64;
            for k in k0..=k1 {
                let shift = k as f64 * index.period;
                let Some((sa, sb)) =
                    slab(line.u0 - shift, line.du, tri.u.0 - g, tri.u.1 + g, ta, tb)
                else {
                    continue;
                };
                if let Some(interval) = index.within(tri, line, shift, sa, sb) {
                    found.push(interval);
                }
            }
        }
        found.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut merged: Vec<(f64, f64)> = Vec::new();
        for (a, b) in found {
            match merged.last_mut() {
                Some(last) if a <= last.1 + 1e-12 => last.1 = last.1.max(b),
                _ => merged.push((a, b)),
            }
        }
        merged
    }

    fn bits(intervals: &[(f64, f64)]) -> Vec<(u64, u64)> {
        intervals
            .iter()
            .map(|(a, b)| (a.to_bits(), b.to_bits()))
            .collect()
    }

    /// The cells and the running union give the same intervals, bit for bit,
    /// as every triangle sorted and swept (GC-07, NFR-06; issue #382): on a
    /// carving's dense region and on scattered triangles across the seam, for
    /// passes along X at many A (an axial pass, which the cells serve),
    /// short slanted lines, and helices of several turns (which try every
    /// triangle).
    #[test]
    fn req_gc_07_candidate_intervals_are_the_same_as_a_sweep_of_every_triangle() {
        use openchuck_testkit::parts::{CarvedRelief, RadialPart};
        let rotary = RotaryMapping::of_profile(&MachineProfile::example_x_rotary());
        let part = CarvedRelief::STANDARD;
        let carving = part.mesh(61, 151);
        let region = TriangleMesh {
            vertices: carving.vertices.clone(),
            triangles: carving
                .triangles
                .iter()
                .copied()
                .filter(|t| {
                    t.iter().all(|&i| {
                        let p = carving.vertices[i as usize];
                        p.x > 6.0 && p.x < 21.0 && (p.z < 4.0 || p.y > 0.0)
                    })
                })
                .collect(),
        };
        let mut rng = Lcg(11);
        let mut scattered = TriangleMesh {
            vertices: Vec::new(),
            triangles: Vec::new(),
        };
        for _ in 0..60 {
            let (x, phi) = (rng.next() * 30.0, rng.next() * std::f64::consts::TAU);
            let base = scattered.vertices.len() as u32;
            for (dx, dphi) in [(0.0, 0.0), (0.5 + rng.next(), 0.0), (0.0, 0.3 * rng.next())] {
                let a = phi + dphi;
                scattered
                    .vertices
                    .push(Point3::new(x + dx, 12.0 * a.cos(), 12.0 * a.sin()));
            }
            scattered.triangles.push([base, base + 1, base + 2]);
        }
        let radius = part.radius + part.depth + 0.3;
        let mut lines = 0;
        for (mesh, reach) in [(&region, 1.7), (&scattered, 1.7), (&scattered, 0.2)] {
            let index = DevIndex::build(mesh, &rotary, radius, reach).unwrap();
            let period = index.period;
            let mut check = |line: DevLine| {
                assert_eq!(
                    bits(&index.intervals(&line)),
                    bits(&intervals_by_sweep(&index, &line)),
                    "{line:?}"
                );
                lines += 1;
            };
            for i in 0..90 {
                let u0 = period * (f64::from(i) / 90.0 - 0.5) + 0.013;
                check(DevLine {
                    x0: -1.0,
                    dx: 32.0,
                    u0,
                    du: 0.0,
                });
                check(DevLine {
                    x0: 31.0,
                    dx: -32.0,
                    u0: u0 + 3.0 * period,
                    du: 0.0,
                });
            }
            for _ in 0..40 {
                check(DevLine {
                    x0: rng.next() * 30.0,
                    dx: (rng.next() - 0.5) * 20.0,
                    u0: (rng.next() - 0.5) * 3.0 * period,
                    du: (rng.next() - 0.5) * 0.4 * period,
                });
                check(DevLine {
                    x0: rng.next() * 5.0,
                    dx: 25.0,
                    u0: (rng.next() - 0.5) * period,
                    du: (2.0 + 3.0 * rng.next()) * period,
                });
            }
        }
        assert_eq!(lines, 3 * (180 + 80));
    }

    /// Intervals joined as they come give the groups of one sort and sweep,
    /// whatever the order they come in (the ends of a group do not depend on
    /// it), and an interval inside a group changes nothing.
    #[test]
    fn req_gc_07_a_running_union_is_the_same_as_one_sweep() {
        let mut rng = Lcg(3);
        for case in 0..300 {
            let n = 1 + case % 40;
            let intervals: Vec<(f64, f64)> = (0..n)
                .map(|_| {
                    let a = (rng.next() * 40.0).floor() / 40.0;
                    // Ends that meet exactly, or within the join, or not.
                    let b = a + [0.0, 0.025, 0.1, 1e-13, 0.3][(rng.next() * 5.0) as usize];
                    (a, b.min(1.0))
                })
                .collect();
            let mut sorted = intervals.clone();
            sorted.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut swept: Vec<(f64, f64)> = Vec::new();
            for (a, b) in sorted {
                match swept.last_mut() {
                    Some(last) if a <= last.1 + 1e-12 => last.1 = last.1.max(b),
                    _ => swept.push((a, b)),
                }
            }
            let mut union = Union::default();
            for &(a, b) in intervals.iter().rev() {
                if !union.covers(a, b) {
                    union.add(a, b);
                }
            }
            assert_eq!(bits(&union.groups), bits(&swept), "{intervals:?}");
        }
    }

    #[test]
    fn req_op_02_candidate_intervals_are_the_region_offset_along_random_lines() {
        let rotary = RotaryMapping::of_profile(&MachineProfile::example_x_rotary());
        let radius = 10.0;
        let mut rng = Lcg(7);
        // Scattered small triangles on a cylinder, some straddling the seam.
        let mut mesh = TriangleMesh {
            vertices: Vec::new(),
            triangles: Vec::new(),
        };
        for _ in 0..25 {
            let (x, phi) = (rng.next() * 30.0, rng.next() * std::f64::consts::TAU);
            let base = mesh.vertices.len() as u32;
            for (dx, dphi) in [
                (0.0, 0.0),
                (0.5 + rng.next(), 0.0),
                (0.0, 0.1 + 0.2 * rng.next()),
            ] {
                let a = phi + dphi;
                mesh.vertices
                    .push(Point3::new(x + dx, radius * a.cos(), radius * a.sin()));
            }
            mesh.triangles.push([base, base + 1, base + 2]);
        }
        let index = DevIndex::build(&mesh, &rotary, radius, 1.5).unwrap();
        for _ in 0..16 {
            let line = DevLine {
                x0: rng.next() * 30.0,
                dx: (rng.next() - 0.5) * 30.0,
                u0: (rng.next() - 0.5) * 200.0,
                du: (rng.next() - 0.5) * 400.0,
            };
            let intervals = index.intervals(&line);
            let steps = 1500;
            for i in 0..=steps {
                let t = i as f64 / steps as f64;
                let inside = intervals
                    .iter()
                    .any(|(a, b)| t >= *a - 1e-9 && t <= *b + 1e-9);
                let want = near(&index, &line, t);
                if want != inside {
                    // Only a grid point within one step of an interval end can
                    // disagree, by rounding.
                    let edge = intervals.iter().any(|(a, b)| {
                        (t - a).abs() < 2.0 / steps as f64 || (t - b).abs() < 2.0 / steps as f64
                    });
                    assert!(edge, "t {t}: brute {want}, intervals {intervals:?}");
                }
            }
        }
    }
}
