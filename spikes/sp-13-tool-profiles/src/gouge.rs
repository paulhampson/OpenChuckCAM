// SPDX-License-Identifier: AGPL-3.0-or-later
//! Independent gouge checker. It shares only the profile *data* with the
//! solver; its maths is different: it samples the exact analytic surface (not
//! the mesh), classifies each point against the closed revolved outline by a
//! crossing count, and measures depth as the distance to that outline. It
//! never calls `height_at`, `contact_for_normal` or the drop-cutter.

use crate::mesh::V3;
use crate::parts::Part;
use crate::profile::{Piece, Profile, Pt};

/// Height above the end of the profile at which the closed outline is capped.
const CAP: f64 = 1000.0;

fn outline(profile: &Profile) -> Vec<Piece> {
    let mut v: Vec<Piece> = profile.pieces().to_vec();
    let end = Pt::new(profile.r_max(), profile.z_end());
    v.push(Piece::Line {
        a: end,
        b: Pt::new(end.r, end.z + CAP),
    });
    v
}

/// True when (r, z) lies inside the revolved tool (half-plane crossing count).
fn inside(outline: &[Piece], r: f64, z: f64) -> bool {
    let mut crossings = 0;
    for p in outline {
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
    // The top cap and the axis close the outline; the ray toward +r crosses
    // neither, so an odd count means inside, provided z is below the cap.
    crossings % 2 == 1 && z < profile_cap(outline)
}

fn profile_cap(outline: &[Piece]) -> f64 {
    outline.last().map_or(0.0, |p| p.end().z)
}

fn dist_to_piece(p: &Piece, q: Pt) -> f64 {
    match *p {
        Piece::Line { a, b } => {
            let (dr, dz) = (b.r - a.r, b.z - a.z);
            let t = (((q.r - a.r) * dr + (q.z - a.z) * dz) / (dr * dr + dz * dz)).clamp(0.0, 1.0);
            (q.r - a.r - t * dr).hypot(q.z - a.z - t * dz)
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
            let ends = (q.r - a.r)
                .hypot(q.z - a.z)
                .min((q.r - b.r).hypot(q.z - b.z));
            if tq >= lo && tq <= hi {
                (rho - (q.r - c.r).hypot(q.z - c.z)).abs().min(ends)
            } else {
                ends
            }
        }
    }
}

/// Depth of (r, z) inside the tool, or zero outside.
pub fn depth(outline: &[Piece], r: f64, z: f64) -> f64 {
    if !inside(outline, r, z) {
        return 0.0;
    }
    let q = Pt::new(r, z);
    outline
        .iter()
        .map(|p| dist_to_piece(p, q))
        .fold(f64::MAX, f64::min)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Gouge {
    /// Deepest penetration of the part surface into the tool (mm).
    pub depth: f64,
    /// Where on the part.
    pub at: V3,
}

/// Samples the part's exact top surface on a grid of `step` under the tool
/// (axis +Z, tip at `tip`) and returns the deepest gouge.
pub fn max_gouge(profile: &Profile, part: &Part, tip: V3, step: f64) -> Gouge {
    let ol = outline(profile);
    let r = profile.r_max();
    let n = (r / step).ceil() as i64;
    let mut g = Gouge {
        depth: 0.0,
        at: tip,
    };
    for i in -n..=n {
        for j in -n..=n {
            let dx = i as f64 * step;
            let dy = j as f64 * step;
            let rr = dx.hypot(dy);
            if rr > r {
                continue;
            }
            let (x, y) = (tip[0] + dx, tip[1] + dy);
            let Some(z) = part.top(x, y) else { continue };
            let d = depth(&ol, rr, z - tip[2]);
            if d > g.depth {
                g = Gouge {
                    depth: d,
                    at: [x, y, z],
                };
            }
        }
    }
    g
}
