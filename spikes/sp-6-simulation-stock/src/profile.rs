// SPDX-License-Identifier: AGPL-3.0-or-later
//! The revolved tool profile: ordered line and arc segments in the (radius,
//! height) half-plane, starting on the tool axis at the tip.
//!
//! The input form matches `openchuck_types::tool::ToolProfile` (each segment
//! runs from the previous point to `to`; arcs give `to` and `centre`). The
//! validity rule the spike proposes is *monotone*: along the profile neither
//! the radius nor the height may decrease. That one rule makes the profile
//! connected, non-self-intersecting and revolvable, makes an arc's direction
//! unambiguous without a flag, and makes the lower envelope `height_at(r)` a
//! function. It excludes undercut tools (dovetail, lollipop, keyseat).

use std::f64::consts::{FRAC_PI_2, PI};

/// Lengths below this are treated as zero (mm).
pub const LEN_EPS: f64 = 1e-9;
/// An arc's start and end may differ in distance from the centre by this much (mm).
pub const ARC_TOL: f64 = 1e-6;

/// A point in the profile half-plane.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pt {
    /// Distance from the tool axis.
    pub r: f64,
    /// Height above the tip.
    pub z: f64,
}

impl Pt {
    pub const fn new(r: f64, z: f64) -> Self {
        Self { r, z }
    }
    fn dist(self, o: Pt) -> f64 {
        (self.r - o.r).hypot(self.z - o.z)
    }
}

/// One input segment, as in `openchuck_types::tool::ProfileSegment`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Segment {
    Line { to: Pt },
    Arc { to: Pt, centre: Pt },
}

/// A validated piece of the profile.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Piece {
    Line {
        a: Pt,
        b: Pt,
    },
    /// `convex` arcs bulge away from the tool (ball-like, centre above and
    /// inside); concave arcs bulge into it (corner-rounding cutters).
    Arc {
        a: Pt,
        b: Pt,
        c: Pt,
        rho: f64,
        convex: bool,
    },
}

impl Piece {
    pub fn start(&self) -> Pt {
        match *self {
            Piece::Line { a, .. } | Piece::Arc { a, .. } => a,
        }
    }
    pub fn end(&self) -> Pt {
        match *self {
            Piece::Line { b, .. } | Piece::Arc { b, .. } => b,
        }
    }

    /// Height of the piece at radius `r`, for `r` within the piece's radius range.
    /// Not meaningful for a vertical line.
    pub fn eval(&self, r: f64) -> f64 {
        match *self {
            Piece::Line { a, b } => a.z + (r - a.r) * (b.z - a.z) / (b.r - a.r),
            Piece::Arc { c, rho, convex, .. } => {
                let q = (rho * rho - (r - c.r) * (r - c.r)).max(0.0).sqrt();
                if convex { c.z - q } else { c.z + q }
            }
        }
    }

    /// True when the height is a convex function of radius on this piece
    /// (lines and convex arcs), which makes the drop-cutter edge test unimodal.
    pub fn convex_in_r(&self) -> bool {
        match *self {
            Piece::Line { .. } => true,
            Piece::Arc { convex, .. } => convex,
        }
    }

    /// Outward normal angle at the start and end of the piece. The outward
    /// normal of direction (dr, dz) is (dz, -dr); its angle runs from -pi/2
    /// (pointing down the axis) to 0 (pointing radially out).
    fn normal_angles(&self) -> (f64, f64) {
        match *self {
            Piece::Line { a, b } => {
                let phi = (-(b.r - a.r)).atan2(b.z - a.z);
                (phi, phi)
            }
            Piece::Arc {
                a, b, c, convex, ..
            } => {
                let ta = (a.z - c.z).atan2(a.r - c.r);
                let tb = (b.z - c.z).atan2(b.r - c.r);
                if convex {
                    (ta.min(0.0), tb.min(0.0))
                } else {
                    // Concave ends sit at angles pi/2..pi; atan2 gives -pi for -0.0.
                    let wrap = |t: f64| if t < 0.0 { t + 2.0 * PI } else { t };
                    (wrap(ta) - PI, wrap(tb) - PI)
                }
            }
        }
    }
}

/// Why a profile was rejected. `segment` is the zero-based segment index.
#[derive(Debug, Clone, PartialEq)]
pub struct ProfileError {
    pub segment: Option<usize>,
    pub reason: String,
}

impl std::fmt::Display for ProfileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.segment {
            Some(i) => write!(f, "segment {}: {}", i + 1, self.reason),
            None => write!(f, "{}", self.reason),
        }
    }
}

fn err(segment: Option<usize>, reason: impl Into<String>) -> ProfileError {
    ProfileError {
        segment,
        reason: reason.into(),
    }
}

/// A piece whose radius range is not empty, for the lower envelope.
#[derive(Debug, Clone, Copy)]
pub struct RPiece {
    pub r0: f64,
    pub r1: f64,
    pub piece: Piece,
}

/// A validated revolved profile.
#[derive(Debug, Clone)]
pub struct Profile {
    pieces: Vec<Piece>,
    rpieces: Vec<RPiece>,
    r_max: f64,
    z_end: f64,
    convex: bool,
}

impl Profile {
    /// Validates the segments (starting at the tip, on the axis).
    pub fn new(segments: &[Segment]) -> Result<Self, ProfileError> {
        if segments.is_empty() {
            return Err(err(None, "a profile needs at least one segment"));
        }
        let mut prev = Pt::new(0.0, 0.0);
        let mut pieces = Vec::with_capacity(segments.len());
        for (i, seg) in segments.iter().enumerate() {
            let (to, centre) = match *seg {
                Segment::Line { to } => (to, None),
                Segment::Arc { to, centre } => (to, Some(centre)),
            };
            for p in std::iter::once(to).chain(centre) {
                if !p.r.is_finite() || !p.z.is_finite() {
                    return Err(err(Some(i), "a coordinate is not a finite number"));
                }
            }
            if prev.dist(to) < LEN_EPS {
                return Err(err(Some(i), "the segment has zero length"));
            }
            if to.r < prev.r - LEN_EPS || to.z < prev.z - LEN_EPS {
                return Err(err(
                    Some(i),
                    format!(
                        "the profile turns back from ({}, {}) to ({}, {}): radius and height \
                         must not decrease from the tip upwards (undercut tools are not supported)",
                        prev.r, prev.z, to.r, to.z
                    ),
                ));
            }
            if i == 0 && to.r < LEN_EPS {
                return Err(err(Some(i), "the first segment runs up the tool axis"));
            }
            let piece = match centre {
                None => Piece::Line { a: prev, b: to },
                Some(c) => {
                    let ra = prev.dist(c);
                    let rb = to.dist(c);
                    if (ra - rb).abs() > ARC_TOL {
                        return Err(err(
                            Some(i),
                            format!(
                                "the arc's start and end are {ra} and {rb} mm from its centre; \
                                 they must agree within {ARC_TOL} mm"
                            ),
                        ));
                    }
                    let rho = 0.5 * (ra + rb);
                    if rho < LEN_EPS {
                        return Err(err(Some(i), "the arc has zero radius"));
                    }
                    // Convex: both ends below and right of the centre (angles -pi/2..0).
                    // Concave: both ends above and left of the centre (angles pi/2..pi).
                    let tol = ARC_TOL;
                    let convex = prev.r >= c.r - tol
                        && to.r >= c.r - tol
                        && prev.z <= c.z + tol
                        && to.z <= c.z + tol;
                    let concave = prev.r <= c.r + tol
                        && to.r <= c.r + tol
                        && prev.z >= c.z - tol
                        && to.z >= c.z - tol;
                    if convex == concave {
                        return Err(err(
                            Some(i),
                            "the arc is not monotone: it must lie within one quarter turn, \
                             with radius and height both increasing along it",
                        ));
                    }
                    Piece::Arc {
                        a: prev,
                        b: to,
                        c,
                        rho,
                        convex,
                    }
                }
            };
            pieces.push(piece);
            prev = to;
        }
        let r_max = prev.r;
        let z_end = prev.z;
        let rpieces: Vec<RPiece> = pieces
            .iter()
            .filter(|p| p.end().r - p.start().r > LEN_EPS)
            .map(|p| RPiece {
                r0: p.start().r,
                r1: p.end().r,
                piece: *p,
            })
            .collect();
        // Convex when the outward normal angle never decreases along the profile.
        let mut convex = true;
        let mut last = -FRAC_PI_2;
        for p in &pieces {
            let (s, e) = p.normal_angles();
            if s < last - 1e-12 || e < s - 1e-12 {
                convex = false;
            }
            last = e;
        }
        Ok(Self {
            pieces,
            rpieces,
            r_max,
            z_end,
            convex,
        })
    }

    pub fn pieces(&self) -> &[Piece] {
        &self.pieces
    }
    pub fn rpieces(&self) -> &[RPiece] {
        &self.rpieces
    }
    /// Cutting radius (radius at the end of the profile).
    pub fn r_max(&self) -> f64 {
        self.r_max
    }
    /// Height at the end of the profile (the flute length for standard tools).
    pub fn z_end(&self) -> f64 {
        self.z_end
    }
    /// True when the profile is convex (no concave arcs or reflex joints).
    pub fn is_convex(&self) -> bool {
        self.convex
    }

    /// Lowest point of the tool at distance `r` from its axis (the lower
    /// envelope), or `None` beyond the cutting radius.
    pub fn height_at(&self, r: f64) -> Option<f64> {
        if !(0.0..=self.r_max + LEN_EPS).contains(&r) {
            return None;
        }
        for rp in &self.rpieces {
            if r <= rp.r1 {
                return Some(rp.piece.eval(r.max(rp.r0)));
            }
        }
        self.rpieces.last().map(|rp| rp.piece.eval(rp.r1))
    }

    /// The profile point whose outward normal has angle `phi` (in -pi/2..=0),
    /// for a convex profile. Returns `None` for a non-convex profile: there the
    /// local normal match is not the global contact, so callers must drop.
    pub fn contact_for_normal(&self, phi: f64) -> Option<Pt> {
        if !self.convex {
            return None;
        }
        let tol = 1e-12;
        // The tip is a vertex whose normal fan starts straight down.
        let mut fan_start = -FRAC_PI_2;
        let mut vertex = Pt::new(0.0, 0.0);
        for p in &self.pieces {
            let (s, e) = p.normal_angles();
            if phi >= fan_start - tol && phi <= s + tol {
                return Some(vertex);
            }
            if phi >= s - tol && phi <= e + tol {
                return Some(match *p {
                    Piece::Line { a, .. } => a,
                    Piece::Arc { c, rho, .. } => {
                        Pt::new(c.r + rho * phi.cos(), c.z + rho * phi.sin())
                    }
                });
            }
            fan_start = e;
            vertex = p.end();
        }
        // The top edge of the flutes carries the rest of the fan up to 0.
        (phi <= tol).then_some(vertex)
    }
}
