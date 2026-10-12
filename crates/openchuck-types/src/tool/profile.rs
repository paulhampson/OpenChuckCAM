// SPDX-License-Identifier: AGPL-3.0-or-later
//! The revolved tool profile (TL-02, TL-03; decision record 0012).
//!
//! A profile is an ordered list of line and arc segments in the (radius,
//! height) half-plane. It starts on the tool axis at the tip and each segment
//! runs from the previous point. [`ToolProfile::validate`] applies the
//! *monotone rule*: along the profile neither the radius nor the height
//! decreases, no segment has zero length, an arc's ends lie on one circle
//! within [`ARC_TOLERANCE`] and the arc stays within one quarter turn, the
//! first segment leaves the axis, and every value is finite. That makes a
//! profile connected, non-self-intersecting and revolvable, fixes each arc's
//! direction without a flag, and makes the lower envelope `h(r)` a function.
//! It rejects undercut tools (dovetail, lollipop, keyseat, back chamfer).
//!
//! The validated [`RevolvedProfile`] is what path solving, the gouge checker
//! and the simulation sweep read (decision record 0012 item 5): the lower
//! envelope [`RevolvedProfile::height_at`], its pieces in radius order with a
//! convexity flag each, whether the whole profile is convex, and for a convex
//! profile the point with a given outward normal.

use std::f64::consts::{FRAC_PI_2, PI};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::units::Millimetres;

/// Lengths below this are treated as zero, in millimetres.
pub const LENGTH_EPSILON: f64 = 1e-9;

/// How far an arc's start and end may differ in distance from its centre, in
/// millimetres.
pub const ARC_TOLERANCE: f64 = 1e-6;

/// A point on a tool profile: distance from the tool axis and height above the tip.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilePoint {
    /// Distance from the tool axis.
    pub radius: Millimetres,
    /// Height above the tool tip.
    pub height: Millimetres,
}

impl ProfilePoint {
    /// The point at `radius` and `height`, in millimetres.
    pub const fn new(radius: f64, height: f64) -> Self {
        Self {
            radius: Millimetres(radius),
            height: Millimetres(height),
        }
    }

    /// The tip: on the axis at height zero.
    pub const TIP: Self = Self::new(0.0, 0.0);

    fn r(self) -> f64 {
        self.radius.0
    }

    fn z(self) -> f64 {
        self.height.0
    }

    /// Distance to `other`, evaluated directly (decision record 0012: the
    /// expanded form loses precision near the axis).
    pub fn distance(self, other: Self) -> f64 {
        (self.r() - other.r()).hypot(self.z() - other.z())
    }
}

/// One segment of a revolved tool profile, running from the previous point.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProfileSegment {
    /// A straight line to `to`.
    Line {
        /// End point.
        to: ProfilePoint,
    },
    /// A circular arc to `to` about `centre`. Under the monotone rule the
    /// direction follows from the ends and the centre.
    Arc {
        /// End point.
        to: ProfilePoint,
        /// Arc centre.
        centre: ProfilePoint,
    },
}

impl ProfileSegment {
    /// The segment's end point.
    pub fn end(&self) -> ProfilePoint {
        match *self {
            Self::Line { to } | Self::Arc { to, .. } => to,
        }
    }
}

/// A tool outline revolved about the tool axis, starting on the axis at the tip.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolProfile {
    /// Segments from the tip outwards and upwards.
    pub segments: Vec<ProfileSegment>,
}

impl ToolProfile {
    /// Checks the monotone rule and builds the solver form. Errors name the
    /// segment (`tool.profile.segments[i]`, and "segment i+1" in the reason).
    pub fn validate(&self) -> Result<RevolvedProfile> {
        RevolvedProfile::new(&self.segments)
    }

    /// The point where the profile ends: the tip for an empty profile.
    pub fn end(&self) -> ProfilePoint {
        self.segments
            .last()
            .map_or(ProfilePoint::TIP, ProfileSegment::end)
    }
}

/// A validated piece of a profile.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProfilePiece {
    /// A straight line from `start` to `end`.
    Line {
        /// Start point.
        start: ProfilePoint,
        /// End point.
        end: ProfilePoint,
    },
    /// A circular arc from `start` to `end` about `centre`.
    Arc {
        /// Start point.
        start: ProfilePoint,
        /// End point.
        end: ProfilePoint,
        /// Centre.
        centre: ProfilePoint,
        /// Radius (the mean of the two end distances).
        radius: f64,
        /// True when the arc bulges away from the tool, like a ball end
        /// (centre above and inside); false when it bulges into the tool,
        /// like a corner-rounding cutter.
        convex: bool,
    },
}

impl ProfilePiece {
    /// The piece's start point.
    pub fn start(&self) -> ProfilePoint {
        match *self {
            Self::Line { start, .. } | Self::Arc { start, .. } => start,
        }
    }

    /// The piece's end point.
    pub fn end(&self) -> ProfilePoint {
        match *self {
            Self::Line { end, .. } | Self::Arc { end, .. } => end,
        }
    }

    /// Height of the piece at radius `r`, for `r` within the piece's radius
    /// range. Not meaningful for a vertical line.
    pub fn height_at(&self, r: f64) -> f64 {
        match *self {
            Self::Line { start, end } => {
                start.z() + (r - start.r()) * (end.z() - start.z()) / (end.r() - start.r())
            }
            Self::Arc {
                centre,
                radius,
                convex,
                ..
            } => {
                let dr = r - centre.r();
                let q = (radius * radius - dr * dr).max(0.0).sqrt();
                if convex {
                    centre.z() - q
                } else {
                    centre.z() + q
                }
            }
        }
    }

    /// True when height is a convex function of radius on this piece (lines
    /// and convex arcs), which makes a drop-cutter edge search unimodal.
    pub fn is_convex_in_radius(&self) -> bool {
        match *self {
            Self::Line { .. } => true,
            Self::Arc { convex, .. } => convex,
        }
    }

    /// Outward normal angle at the start and end of the piece. The outward
    /// normal of direction (dr, dz) is (dz, -dr); its angle runs from -pi/2
    /// (straight down the axis) to 0 (radially out).
    fn normal_angles(&self) -> (f64, f64) {
        match *self {
            Self::Line { start, end } => {
                let phi = (-(end.r() - start.r())).atan2(end.z() - start.z());
                (phi, phi)
            }
            Self::Arc {
                start,
                end,
                centre,
                convex,
                ..
            } => {
                let ta = (start.z() - centre.z()).atan2(start.r() - centre.r());
                let tb = (end.z() - centre.z()).atan2(end.r() - centre.r());
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

/// A piece of the lower envelope: a piece whose radius range is not empty.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnvelopePiece {
    /// Radius where the piece starts.
    pub r_start: f64,
    /// Radius where the piece ends.
    pub r_end: f64,
    /// The piece.
    pub piece: ProfilePiece,
}

/// A profile that passed the monotone rule, in the form the solvers read.
#[derive(Debug, Clone, PartialEq)]
pub struct RevolvedProfile {
    pieces: Vec<ProfilePiece>,
    envelope: Vec<EnvelopePiece>,
    end: ProfilePoint,
    convex: bool,
}

fn segment_error(i: usize, reason: impl std::fmt::Display) -> Error {
    Error::invalid(
        format!("tool.profile.segments[{i}]"),
        format!("segment {}: {reason}", i + 1),
    )
}

impl RevolvedProfile {
    fn new(segments: &[ProfileSegment]) -> Result<Self> {
        if segments.is_empty() {
            return Err(Error::invalid(
                "tool.profile",
                "a custom profile needs at least one segment",
            ));
        }
        let mut prev = ProfilePoint::TIP;
        let mut pieces = Vec::with_capacity(segments.len());
        for (i, segment) in segments.iter().enumerate() {
            let (to, centre) = match *segment {
                ProfileSegment::Line { to } => (to, None),
                ProfileSegment::Arc { to, centre } => (to, Some(centre)),
            };
            for p in std::iter::once(to).chain(centre) {
                if !p.r().is_finite() || !p.z().is_finite() {
                    return Err(segment_error(i, "a coordinate is not a finite number"));
                }
            }
            if prev.distance(to) < LENGTH_EPSILON {
                return Err(segment_error(i, "the segment has zero length"));
            }
            if to.r() < prev.r() - LENGTH_EPSILON || to.z() < prev.z() - LENGTH_EPSILON {
                return Err(segment_error(
                    i,
                    format!(
                        "the profile turns back from ({}, {}) to ({}, {}); radius and height \
                         must not decrease from the tip upwards (undercut tools are not supported)",
                        prev.r(),
                        prev.z(),
                        to.r(),
                        to.z()
                    ),
                ));
            }
            if i == 0 && to.r() < LENGTH_EPSILON {
                return Err(segment_error(i, "the first segment runs up the tool axis"));
            }
            let piece = match centre {
                None => ProfilePiece::Line {
                    start: prev,
                    end: to,
                },
                Some(c) => arc_piece(i, prev, to, c)?,
            };
            pieces.push(piece);
            prev = to;
        }
        let envelope = pieces
            .iter()
            .filter(|p| p.end().r() - p.start().r() > LENGTH_EPSILON)
            .map(|p| EnvelopePiece {
                r_start: p.start().r(),
                r_end: p.end().r(),
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
            envelope,
            end: prev,
            convex,
        })
    }

    /// The pieces from the tip, one per segment.
    pub fn pieces(&self) -> &[ProfilePiece] {
        &self.pieces
    }

    /// The pieces with a non-empty radius range, in radius order: the lower
    /// envelope `h(r)` piece by piece.
    pub fn envelope(&self) -> &[EnvelopePiece] {
        &self.envelope
    }

    /// The cutting radius: the radius where the profile ends.
    pub fn cutting_radius(&self) -> f64 {
        self.end.r()
    }

    /// The height where the profile ends (the flute length for a whole tool).
    pub fn top(&self) -> f64 {
        self.end.z()
    }

    /// The profile's end point.
    pub fn end(&self) -> ProfilePoint {
        self.end
    }

    /// True when the profile is convex (no concave arc and no reflex joint).
    pub fn is_convex(&self) -> bool {
        self.convex
    }

    /// The lowest point of the tool at distance `r` from its axis (the lower
    /// envelope `h(r)`), or `None` beyond the cutting radius.
    pub fn height_at(&self, r: f64) -> Option<f64> {
        if !(0.0..=self.cutting_radius() + LENGTH_EPSILON).contains(&r) {
            return None;
        }
        for e in &self.envelope {
            if r <= e.r_end {
                return Some(e.piece.height_at(r.max(e.r_start)));
            }
        }
        self.envelope.last().map(|e| e.piece.height_at(e.r_end))
    }

    /// The profile point whose outward normal has angle `phi` (from -pi/2,
    /// straight down, to 0, radially out), for a convex profile. `None` for a
    /// non-convex profile, where a local normal match is not the contact, so
    /// callers must use the drop-cutter instead.
    pub fn point_for_normal(&self, phi: f64) -> Option<ProfilePoint> {
        if !self.convex {
            return None;
        }
        let tol = 1e-12;
        // The tip is a vertex whose normal fan starts straight down.
        let mut fan_start = -FRAC_PI_2;
        let mut vertex = ProfilePoint::TIP;
        for p in &self.pieces {
            let (s, e) = p.normal_angles();
            if phi >= fan_start - tol && phi <= s + tol {
                return Some(vertex);
            }
            if phi >= s - tol && phi <= e + tol {
                return Some(match *p {
                    ProfilePiece::Line { start, .. } => start,
                    ProfilePiece::Arc { centre, radius, .. } => ProfilePoint::new(
                        centre.r() + radius * phi.cos(),
                        centre.z() + radius * phi.sin(),
                    ),
                });
            }
            fan_start = e;
            vertex = p.end();
        }
        // The top edge of the flutes carries the rest of the fan up to 0.
        (phi <= tol).then_some(vertex)
    }
}

fn arc_piece(
    i: usize,
    start: ProfilePoint,
    end: ProfilePoint,
    centre: ProfilePoint,
) -> Result<ProfilePiece> {
    let ra = start.distance(centre);
    let rb = end.distance(centre);
    if (ra - rb).abs() > ARC_TOLERANCE {
        return Err(segment_error(
            i,
            format!(
                "the arc's start and end are {ra} and {rb} mm from its centre; \
                 they must agree within {ARC_TOLERANCE} mm"
            ),
        ));
    }
    let radius = 0.5 * (ra + rb);
    if radius < LENGTH_EPSILON {
        return Err(segment_error(i, "the arc has zero radius"));
    }
    // Convex: both ends below and outside the centre (angles -pi/2..0).
    // Concave: both ends above and inside the centre (angles pi/2..pi).
    let tol = ARC_TOLERANCE;
    let c = centre;
    let convex = start.r() >= c.r() - tol
        && end.r() >= c.r() - tol
        && start.z() <= c.z() + tol
        && end.z() <= c.z() + tol;
    let concave = start.r() <= c.r() + tol
        && end.r() <= c.r() + tol
        && start.z() >= c.z() - tol
        && end.z() >= c.z() - tol;
    if convex == concave {
        return Err(segment_error(
            i,
            "the arc is not monotone: it must lie within one quarter turn, \
             with radius and height both increasing along it",
        ));
    }
    Ok(ProfilePiece::Arc {
        start,
        end,
        centre,
        radius,
        convex,
    })
}

/// The centre of the arc from `start` to `end` with `radius`: convex (bulging
/// away from the tool, like a ball end) or concave (bulging into it). For the
/// custom profile text file, which may give an arc by radius instead of
/// centre (decision record 0012 item 6). Fails when the ends are further
/// apart than the diameter. The result still has to pass the monotone rule.
pub fn arc_centre(
    start: ProfilePoint,
    end: ProfilePoint,
    radius: f64,
    convex: bool,
) -> std::result::Result<ProfilePoint, String> {
    if !(radius.is_finite() && radius > 0.0) {
        return Err(format!("the arc radius {radius} must be greater than zero"));
    }
    let chord = start.distance(end);
    if chord < LENGTH_EPSILON {
        return Err("the arc has zero length".to_owned());
    }
    let half = chord / 2.0;
    if half > radius + ARC_TOLERANCE {
        return Err(format!(
            "the arc's ends are {chord} mm apart, more than its diameter {}",
            2.0 * radius
        ));
    }
    let along = (radius * radius - half * half).max(0.0).sqrt();
    let (mr, mz) = ((start.r() + end.r()) / 2.0, (start.z() + end.z()) / 2.0);
    // Unit chord direction and its left normal (rotated +90 degrees).
    let (ur, uz) = ((end.r() - start.r()) / chord, (end.z() - start.z()) / chord);
    let (nr, nz) = (-uz, ur);
    // Going up and out, the left side is above and inside: a convex arc's centre.
    let sign = if convex { 1.0 } else { -1.0 };
    Ok(ProfilePoint::new(
        mr + sign * along * nr,
        mz + sign * along * nz,
    ))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn line(r: f64, z: f64) -> ProfileSegment {
        ProfileSegment::Line {
            to: ProfilePoint::new(r, z),
        }
    }

    #[test]
    fn req_tl_03_arc_centre_by_radius_matches_the_given_centre() {
        let start = ProfilePoint::TIP;
        let end = ProfilePoint::new(3.0, 3.0);
        let c = arc_centre(start, end, 3.0, true).unwrap();
        assert!(c.distance(ProfilePoint::new(0.0, 3.0)) < 1e-12, "{c:?}");
        let c = arc_centre(
            ProfilePoint::new(1.0, 0.0),
            ProfilePoint::new(3.0, 2.0),
            2.0,
            false,
        )
        .unwrap();
        assert!(c.distance(ProfilePoint::new(3.0, 0.0)) < 1e-12, "{c:?}");
        assert!(arc_centre(start, end, 1.0, true).is_err());
    }

    #[test]
    fn req_tl_03_monotone_rule_rejects_a_turn_back_naming_the_segment() {
        let profile = ToolProfile {
            segments: vec![line(3.0, 0.0), line(3.0, 5.0), line(2.0, 8.0)],
        };
        let err = profile.validate().unwrap_err().to_string();
        assert!(
            err.contains("segments[2]") && err.contains("segment 3"),
            "{err}"
        );
        assert!(err.contains("undercut"), "{err}");
    }

    #[test]
    fn req_tl_03_envelope_of_a_ball_is_the_circle() {
        let profile = ToolProfile {
            segments: vec![
                ProfileSegment::Arc {
                    to: ProfilePoint::new(3.0, 3.0),
                    centre: ProfilePoint::new(0.0, 3.0),
                },
                line(3.0, 12.0),
            ],
        }
        .validate()
        .unwrap();
        for i in 0..=30 {
            let r = f64::from(i) * 0.1;
            let h = profile.height_at(r).unwrap();
            assert!((h - (3.0 - (9.0 - r * r).sqrt())).abs() < 1e-12);
        }
        assert!(profile.is_convex());
        assert_eq!(profile.height_at(3.1), None);
        let p = profile.point_for_normal(-FRAC_PI_2 / 2.0).unwrap();
        assert!((p.r() - 3.0 * (FRAC_PI_2 / 2.0).cos()).abs() < 1e-12);
    }
}
