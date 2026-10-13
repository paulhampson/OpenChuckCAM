// SPDX-License-Identifier: AGPL-3.0-or-later
//! The independent gouge and coverage checker (OP-12; architecture section
//! 9; work breakdown slice 5 item 9; decision record 0012 item 7).
//!
//! Used only in tests, and written separately from the strategies and from
//! the drop-cutter in `openchuck-path`, so the strategies do not grade
//! themselves. It shares only the tool profile *data* (the segments of
//! `RevolvedProfile::pieces`) and nothing else: no `height_at`, no mesh, no
//! drop. It samples the **exact** analytic surface of a [`RadialPart`], not
//! its mesh; classifies each sample against the closed revolved outline of
//! the tool by a crossing count in the tool's half-plane; and measures depth
//! as the distance to that outline. The tool's axis may point anywhere, so
//! 4-axis poses are checked as they are.

use openchuck_types::geometry::{Point3, Vec3};
use openchuck_types::tool::profile::{ProfilePiece, RevolvedProfile};

use crate::parts::RadialPart;

/// Height above the end of the profile at which the closed outline is
/// capped: the tool is treated as a solid of its cutting radius up to here.
const CAP: f64 = 1000.0;

/// A tool position to check: tip and unit axis (tip towards spindle), in the
/// model frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pose {
    /// The tool tip.
    pub tip: Point3,
    /// The tool axis, unit length.
    pub axis: Vec3,
}

#[derive(Debug, Clone, Copy)]
struct Pt {
    r: f64,
    z: f64,
}

#[derive(Debug, Clone, Copy)]
enum Piece {
    Line {
        a: Pt,
        b: Pt,
    },
    Arc {
        a: Pt,
        b: Pt,
        c: Pt,
        rho: f64,
        convex: bool,
    },
}

impl Piece {
    fn start(&self) -> Pt {
        match *self {
            Self::Line { a, .. } | Self::Arc { a, .. } => a,
        }
    }
    fn end(&self) -> Pt {
        match *self {
            Self::Line { b, .. } | Self::Arc { b, .. } => b,
        }
    }
}

/// The tool's closed outline in its half-plane: the profile, then a vertical
/// cap line up from its end.
#[derive(Debug, Clone)]
pub struct Outline {
    pieces: Vec<Piece>,
    radius: f64,
    top: f64,
}

impl Outline {
    /// The outline of `profile`.
    pub fn new(profile: &RevolvedProfile) -> Self {
        let pt = |p: openchuck_types::tool::ProfilePoint| Pt {
            r: p.radius.0,
            z: p.height.0,
        };
        let mut pieces: Vec<Piece> = profile
            .pieces()
            .iter()
            .map(|p| match *p {
                ProfilePiece::Line { start, end } => Piece::Line {
                    a: pt(start),
                    b: pt(end),
                },
                ProfilePiece::Arc {
                    start,
                    end,
                    centre,
                    radius,
                    convex,
                } => Piece::Arc {
                    a: pt(start),
                    b: pt(end),
                    c: pt(centre),
                    rho: radius,
                    convex,
                },
            })
            .collect();
        let end = pt(profile.end());
        pieces.push(Piece::Line {
            a: end,
            b: Pt {
                r: end.r,
                z: end.z + CAP,
            },
        });
        Self {
            pieces,
            radius: end.r,
            top: end.z + CAP,
        }
    }

    /// The tool's largest radius.
    pub fn radius(&self) -> f64 {
        self.radius
    }

    /// True when (r, z) lies inside the revolved tool.
    fn inside(&self, r: f64, z: f64) -> bool {
        if z >= self.top {
            return false;
        }
        let mut crossings = 0;
        for p in &self.pieces {
            let (a, b) = (p.start(), p.end());
            if (a.z > z) == (b.z > z) {
                continue;
            }
            let rc = match *p {
                Piece::Line { a, b } => a.r + (z - a.z) * (b.r - a.r) / (b.z - a.z),
                Piece::Arc { c, rho, convex, .. } => {
                    let q = (rho * rho - (z - c.z) * (z - c.z)).max(0.0).sqrt();
                    if convex { c.r + q } else { c.r - q }
                }
            };
            if rc > r {
                crossings += 1;
            }
        }
        crossings % 2 == 1
    }

    /// Distance from (r, z) to the outline.
    fn distance(&self, r: f64, z: f64) -> f64 {
        self.pieces
            .iter()
            .map(|p| dist_to_piece(p, Pt { r, z }))
            .fold(f64::INFINITY, f64::min)
    }

    /// The depth of (r, z) inside the tool, or zero outside.
    pub fn depth(&self, r: f64, z: f64) -> f64 {
        if self.inside(r, z) {
            self.distance(r, z)
        } else {
            0.0
        }
    }

    /// Signed clearance of (r, z): the distance to the outline outside the
    /// tool, negative inside.
    pub fn clearance(&self, r: f64, z: f64) -> f64 {
        let d = self.distance(r, z);
        if self.inside(r, z) { -d } else { d }
    }
}

fn dist_to_piece(p: &Piece, q: Pt) -> f64 {
    let hyp = |a: f64, b: f64| (a * a + b * b).sqrt();
    match *p {
        Piece::Line { a, b } => {
            let (dr, dz) = (b.r - a.r, b.z - a.z);
            let l2 = dr * dr + dz * dz;
            let t = if l2 > 0.0 {
                (((q.r - a.r) * dr + (q.z - a.z) * dz) / l2).clamp(0.0, 1.0)
            } else {
                0.0
            };
            hyp(q.r - a.r - t * dr, q.z - a.z - t * dz)
        }
        Piece::Arc { a, b, c, rho, .. } => {
            let ang = |p: Pt| (p.z - c.z).atan2(p.r - c.r);
            let wrap = |t: f64| {
                if t < -std::f64::consts::FRAC_PI_2 {
                    t + 2.0 * std::f64::consts::PI
                } else {
                    t
                }
            };
            let (ta, tb, tq) = (wrap(ang(a)), wrap(ang(b)), wrap(ang(q)));
            let (lo, hi) = (ta.min(tb), ta.max(tb));
            let ends = hyp(q.r - a.r, q.z - a.z).min(hyp(q.r - b.r, q.z - b.z));
            if tq >= lo && tq <= hi {
                (rho - hyp(q.r - c.r, q.z - c.z)).abs().min(ends)
            } else {
                ends
            }
        }
    }
}

/// Where a surface point sits relative to a tool pose: distance from the
/// tool axis and height above the tip along it.
fn in_tool_frame(pose: &Pose, p: Point3) -> (f64, f64) {
    let w = p - pose.tip;
    let z = w.dot(pose.axis);
    let radial = w - pose.axis * z;
    (radial.length(), z)
}

/// The deepest gouge found.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Gouge {
    /// Deepest penetration of the exact surface into the tool, in mm (zero
    /// when there is none).
    pub depth: f64,
    /// Where on the part.
    pub at: Point3,
    /// Which pose, by index.
    pub pose: usize,
}

/// Samples of the exact surface of `part` near a pose: every point within
/// the tool's radius (plus `margin`) of its axis line, on a grid of about
/// `step` mm along X and around.
fn samples_near<P: RadialPart + ?Sized>(
    part: &P,
    pose: &Pose,
    reach: f64,
    step: f64,
) -> Vec<Point3> {
    let (x0, x1) = part.x_range();
    let mut out = Vec::new();
    // Along X the reach is at most `reach` divided by how much the axis
    // stays perpendicular to X.
    let perpendicular = (1.0 - pose.axis.x * pose.axis.x).max(1e-6).sqrt();
    let half_x = reach / perpendicular;
    let lo = (pose.tip.x - half_x).max(x0);
    let hi = (pose.tip.x + half_x).min(x1);
    if lo > hi {
        return out;
    }
    let nx = ((hi - lo) / step).ceil().max(1.0) as u32;
    // Polar angle of the tip, from +Y towards +Z, seeds the window around.
    let tip_angle = pose.tip.z.atan2(pose.tip.y);
    for i in 0..=nx {
        let x = lo + (hi - lo) * f64::from(i) / f64::from(nx);
        let rho = part.radius_at(x, tip_angle).max(1e-6);
        let half = if reach * 2.0 >= rho {
            std::f64::consts::PI
        } else {
            (2.0 * reach / rho).min(std::f64::consts::PI)
        };
        let na = ((2.0 * half * rho) / step).ceil().clamp(2.0, 100_000.0) as u32;
        for j in 0..=na {
            let angle = tip_angle - half + 2.0 * half * f64::from(j) / f64::from(na);
            let r = part.radius_at(x, angle);
            out.push(Point3::new(x, r * angle.cos(), r * angle.sin()));
        }
    }
    out
}

/// The deepest gouge of the tool at any of `poses` into the exact surface of
/// `part`, sampled at about `step` mm.
pub fn max_gouge<P: RadialPart + ?Sized>(
    outline: &Outline,
    part: &P,
    poses: &[Pose],
    step: f64,
) -> Gouge {
    let mut worst = Gouge {
        depth: 0.0,
        at: Point3::ORIGIN,
        pose: 0,
    };
    for (k, pose) in poses.iter().enumerate() {
        for p in samples_near(part, pose, outline.radius() + step, step) {
            let (r, z) = in_tool_frame(pose, p);
            let d = outline.depth(r, z);
            if d > worst.depth {
                worst = Gouge {
                    depth: d,
                    at: p,
                    pose: k,
                };
            }
        }
    }
    worst
}

/// The smallest signed clearance between the tool at `pose` and the exact
/// surface of `part`, sampled at about `step` mm: positive when the tool is
/// clear of the surface everywhere, negative by the deepest gouge. A tool
/// that touches the surface (a drop) reads about zero, and one that leaves
/// stock `s` reads about `s`.
pub fn min_clearance<P: RadialPart + ?Sized>(
    outline: &Outline,
    part: &P,
    pose: &Pose,
    step: f64,
) -> f64 {
    samples_near(part, pose, outline.radius() + 1.0, step)
        .into_iter()
        .map(|p| {
            let (r, z) = in_tool_frame(pose, p);
            outline.clearance(r, z)
        })
        .fold(f64::INFINITY, f64::min)
}

/// The result of a coverage check.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coverage {
    /// Surface samples checked.
    pub samples: usize,
    /// Samples further than the limit from every pose's tool surface.
    pub uncovered: usize,
    /// The largest distance from a sample to the nearest tool surface.
    pub worst: f64,
    /// Where that sample is.
    pub at: Point3,
}

/// Checks that every exact surface point of `part` with X in `x_range`, on a
/// grid of `step` mm along X and `step` mm of arc around, comes within
/// `limit` of the tool surface at one of `poses` (a gouge counts as
/// covered). Poses must be dense enough that the tool between two of them
/// cuts nothing they do not, and must have their tips near the part's
/// surface (the search looks at poses near a sample by X and by angle).
pub fn coverage<P: RadialPart + ?Sized>(
    outline: &Outline,
    part: &P,
    poses: &[Pose],
    x_range: (f64, f64),
    step: f64,
    limit: f64,
) -> Coverage {
    // Bucket poses by X and by the polar angle of the tip, so each sample
    // looks only at poses that can reach it.
    let reach = outline.radius() + limit;
    let width = reach.max(step);
    let (x0, x1) = x_range;
    let rho_min = (0..=16)
        .map(|k| part.radius_at(x0 + (x1 - x0) * f64::from(k) / 16.0, 0.0))
        .fold(f64::INFINITY, f64::min)
        .max(reach);
    let sectors = ((std::f64::consts::TAU * rho_min / (2.0 * reach)).floor() as i64).clamp(1, 720);
    let sector_of = |angle: f64| {
        let t = angle.rem_euclid(std::f64::consts::TAU) / std::f64::consts::TAU;
        ((t * sectors as f64).floor() as i64).clamp(0, sectors - 1)
    };
    let mut buckets: std::collections::BTreeMap<(i64, i64), Vec<usize>> = Default::default();
    for (k, pose) in poses.iter().enumerate() {
        let b = (pose.tip.x / width).floor() as i64;
        let s = sector_of(pose.tip.z.atan2(pose.tip.y));
        buckets.entry((b, s)).or_default().push(k);
    }
    let nx = ((x1 - x0) / step).ceil().max(1.0) as u32;
    let mut result = Coverage {
        samples: 0,
        uncovered: 0,
        worst: 0.0,
        at: Point3::ORIGIN,
    };
    for i in 0..=nx {
        let x = x0 + (x1 - x0) * f64::from(i) / f64::from(nx);
        let rho = part.radius_at(x, 0.0).max(1e-6);
        let na = ((std::f64::consts::TAU * rho) / step).ceil().max(8.0) as u32;
        let b = (x / width).floor() as i64;
        for j in 0..na {
            let angle = std::f64::consts::TAU * f64::from(j) / f64::from(na);
            let r = part.radius_at(x, angle);
            let p = Point3::new(x, r * angle.cos(), r * angle.sin());
            let s = sector_of(angle);
            let mut best = f64::INFINITY;
            for bx in b - 1..=b + 1 {
                for ds in -1..=1 {
                    let key = (bx, (s + ds).rem_euclid(sectors));
                    for pose in buckets
                        .get(&key)
                        .into_iter()
                        .flatten()
                        .filter_map(|k| poses.get(*k))
                    {
                        let (r, z) = in_tool_frame(pose, p);
                        best = best.min(outline.clearance(r, z).max(0.0));
                    }
                }
            }
            result.samples += 1;
            if best > limit {
                result.uncovered += 1;
            }
            if best > result.worst {
                result.worst = best;
                result.at = p;
            }
        }
    }
    result
}
