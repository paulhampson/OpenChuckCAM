// SPDX-License-Identifier: AGPL-3.0-or-later
//! Tool-centre solving by parry3d-f64 queries, in four methods, and the two sweeps.

use crate::tool::Tool;
use crate::{Vector, e2_dir, par, u_dir};
use parry3d_f64::query::{PointQuery, Ray, RayCast};
use parry3d_f64::shape::TriMesh;

pub struct Hit {
    /// Height of the hit above `b`, along `u`.
    pub z: f64,
    pub point: Vector,
    pub normal: Vector,
}

/// A mesh with its ray-casting setup. `far` is how high above the line base rays start.
pub struct Caster<'a> {
    pub mesh: &'a TriMesh,
    pub far: f64,
}

impl Caster<'_> {
    /// Cast down the line direction `-u`, from lateral offset `(a, b2)` about `b`.
    pub fn cast(&self, b: Vector, psi: f64, a: f64, b2: f64) -> Option<Hit> {
        let (u, e2) = (u_dir(psi), e2_dir(psi));
        let base = b + Vector::new(a, 0.0, 0.0) + e2 * b2;
        let ray = Ray::new(base + u * self.far, -u);
        let hit = self.mesh.cast_local_ray_and_get_normal(&ray, 2.0 * self.far, false)?;
        let point = ray.origin + ray.dir * hit.time_of_impact;
        Some(Hit { z: self.far - hit.time_of_impact, point, normal: hit.normal })
    }
}

/// Offsets `(a, b, d, h)` of ray origins covering the tool disc in concentric rings.
#[derive(Clone)]
pub struct Stencil {
    pub offsets: Vec<(f64, f64, f64, f64)>,
    pub fine_step: f64,
}

impl Stencil {
    /// Rings at spacing `s` out to the tool radius; the centre ray plus `2 pi k` samples on
    /// ring `k`. `fine_step` is the refinement grid spacing (0 for no refinement).
    pub fn new(tool: &Tool, s: f64, fine_step: f64) -> Self {
        let rt = tool.radius();
        let mut offsets = vec![(0.0, 0.0, 0.0, tool.h(0.0))];
        let rings = (rt / s).ceil() as usize;
        for k in 1..=rings {
            let r = rt * k as f64 / rings as f64;
            let n = ((std::f64::consts::TAU * r / s).ceil() as usize).max(6);
            for j in 0..n {
                let ph = std::f64::consts::TAU * j as f64 / n as f64;
                offsets.push((r * ph.cos(), r * ph.sin(), r, tool.h(r)));
            }
        }
        Stencil { offsets, fine_step }
    }
}

pub struct Drop {
    /// Tip height above `b` along `u`.
    pub z: f64,
    pub contact: Vector,
    pub rays: usize,
}

/// Method M2/M3: drop-cutter by a stencil of parallel rays, optionally refined by a 5 x 5 grid
/// around the best stencil ray. Valid for any profile `h(r)`.
pub fn drop_stencil(c: &Caster, tool: &Tool, st: &Stencil, b: Vector, psi: f64) -> Option<Drop> {
    let mut best: Option<(f64, f64, f64, Vector)> = None;
    let mut rays = 0;
    for &(a, b2, _d, h) in &st.offsets {
        rays += 1;
        if let Some(hit) = c.cast(b, psi, a, b2) {
            let z = hit.z - h;
            if best.is_none_or(|v| z > v.0) {
                best = Some((z, a, b2, hit.point));
            }
        }
    }
    let (z, a0, b0, contact) = best?;
    if st.fine_step > 0.0 {
        let (z, contact) = climb(c, tool, b, psi, (z, a0, b0, contact), st.fine_step, &mut rays);
        return Some(Drop { z, contact, rays });
    }
    Some(Drop { z, contact, rays })
}

/// Pattern search from `start = (z, a, b, contact)`: try the eight neighbours at step `fs`
/// (clamped to the tool disc), move to the best improvement, halve the step when none improves,
/// stop below 0.02 mm. Returns the best tip height and its contact point.
pub fn climb(
    c: &Caster,
    tool: &Tool,
    b: Vector,
    psi: f64,
    start: (f64, f64, f64, Vector),
    mut fs: f64,
    rays: &mut usize,
) -> (f64, Vector) {
    let rt = tool.radius();
    let (mut z, mut a0, mut b0, mut contact) = start;
    let mut moves = 0;
    while fs >= 0.02 && moves < 60 {
        let mut better: Option<(f64, f64, f64, Vector)> = None;
        for i in -1i32..=1 {
            for j in -1i32..=1 {
                if i == 0 && j == 0 {
                    continue;
                }
                let (mut a, mut b2) = (a0 + i as f64 * fs, b0 + j as f64 * fs);
                let d = (a * a + b2 * b2).sqrt();
                if d > rt {
                    a *= rt / d;
                    b2 *= rt / d;
                }
                *rays += 1;
                if let Some(hit) = c.cast(b, psi, a, b2) {
                    let v = hit.z - tool.h(d.min(rt));
                    if v > better.map_or(z, |q| q.0) {
                        better = Some((v, a, b2, hit.point));
                    }
                }
            }
        }
        match better {
            Some((v, a, b2, p)) => {
                z = v;
                a0 = a;
                b0 = b2;
                contact = p;
                moves += 1;
            }
            None => fs *= 0.5,
        }
    }
    (z, contact)
}

/// Method M5: climb from the central ray only, no stencil.
pub fn drop_climb(c: &Caster, tool: &Tool, b: Vector, psi: f64) -> Option<Drop> {
    let hit = c.cast(b, psi, 0.0, 0.0)?;
    let mut rays = 1;
    let start = (hit.z - tool.h(0.0), 0.0, 0.0, hit.point);
    let (z, contact) = climb(c, tool, b, psi, start, 0.5 * tool.radius(), &mut rays);
    Some(Drop { z, contact, rays })
}

/// Method M1: one central ray, then offset along the facet normal (ball tools only).
pub fn drop_normal_offset(c: &Caster, rt: f64, b: Vector, psi: f64) -> Option<Drop> {
    let hit = c.cast(b, psi, 0.0, 0.0)?;
    let centre = hit.point + hit.normal * rt;
    Some(Drop { z: (centre - b).dot(u_dir(psi)) - rt, contact: hit.point, rays: 1 })
}

/// Method M4: ball centre where the distance to the mesh equals the radius, found by
/// bisection on the line. One ray, then distance queries.
pub fn drop_distance(c: &Caster, rt: f64, b: Vector, psi: f64) -> Option<Drop> {
    let u = u_dir(psi);
    let hit = c.cast(b, psi, 0.0, 0.0)?;
    let dist = |s: f64| c.mesh.distance_to_local_point(b + u * s, false);
    let mut lo = hit.z;
    let mut hi = hit.z + rt;
    let mut n = 0;
    while dist(hi) < rt && n < 12 {
        lo = hi;
        hi += rt;
        n += 1;
    }
    for _ in 0..40 {
        let mid = 0.5 * (lo + hi);
        if dist(mid) < rt {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let s = 0.5 * (lo + hi);
    Some(Drop { z: s - rt, contact: hit.point, rays: 1 + 12 + 40 })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    NormalOffset,
    StencilCoarse,
    StencilRefined,
    Distance,
    Climb,
}

pub fn solve(
    c: &Caster,
    tool: &Tool,
    method: Method,
    coarse: &Stencil,
    refined: &Stencil,
    b: Vector,
    psi: f64,
) -> Option<Drop> {
    match method {
        Method::NormalOffset => drop_normal_offset(c, tool.radius(), b, psi),
        Method::StencilCoarse => drop_stencil(c, tool, coarse, b, psi),
        Method::StencilRefined => drop_stencil(c, tool, refined, b, psi),
        Method::Distance => drop_distance(c, tool.radius(), b, psi),
        Method::Climb => drop_climb(c, tool, b, psi),
    }
}

/// One tool pose from a sweep.
#[derive(Clone, Copy, Debug)]
pub struct Pose {
    /// Base of the line (axis point at height 0).
    pub b: Vector,
    /// Tool axis angle in the part frame (A = psi).
    pub psi: f64,
    /// Tip height above `b` along `u`.
    pub z: f64,
    /// Surface angle (th) of the station this pose belongs to, for reference lookups.
    pub th: f64,
    /// Normal-following only: the facet normal's in-plane angle at the first hit.
    pub psi_n: f64,
    /// Rays and distance queries' worth of work for this pose (one distance query counts as 1).
    pub rays: usize,
}

impl Pose {
    pub fn tip(&self) -> Vector {
        self.b + u_dir(self.psi) * self.z
    }
}

/// Mode A: stepped A, rays through the A axis. Stations are `(x, th)` in row order
/// (all `th` for each `x`); the tool axis is the through-axis line at angle `th`.
pub fn sweep_through_axis(
    c: &Caster,
    tool: &Tool,
    method: Method,
    coarse: &Stencil,
    refined: &Stencil,
    xs: &[f64],
    ths: &[f64],
    threads: usize,
) -> Vec<Option<Pose>> {
    let n = xs.len() * ths.len();
    par::map_items(n, threads, |k| {
        let (x, th) = (xs[k / ths.len()], ths[k % ths.len()]);
        let b = Vector::new(x, 0.0, 0.0);
        solve(c, tool, method, coarse, refined, b, th)
            .map(|d| Pose { b, psi: th, z: d.z, th, psi_n: th, rays: d.rays })
    })
}

/// Mode B: tool axis follows the surface normal. At each station a central through-axis ray
/// finds the surface point and the facet normal; the tool axis angle is the normal's angle in
/// the plane perpendicular to X (the best A can do, OP-06), and the tool is dropped along
/// that line through the hit point.
pub fn sweep_normal_following(
    c: &Caster,
    tool: &Tool,
    method: Method,
    coarse: &Stencil,
    refined: &Stencil,
    xs: &[f64],
    ths: &[f64],
    threads: usize,
) -> Vec<Option<Pose>> {
    let n = xs.len() * ths.len();
    par::map_items(n, threads, |k| {
        let (x, th) = (xs[k / ths.len()], ths[k % ths.len()]);
        let b0 = Vector::new(x, 0.0, 0.0);
        let hit = c.cast(b0, th, 0.0, 0.0)?;
        let psi_n = hit.normal.y.atan2(hit.normal.z);
        let d = solve(c, tool, method, coarse, refined, hit.point, psi_n)?;
        Some(Pose { b: hit.point, psi: psi_n, z: d.z, th, psi_n, rays: d.rays + 1 })
    })
}
