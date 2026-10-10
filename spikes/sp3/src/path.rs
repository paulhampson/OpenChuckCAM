//! Fixed wrapped engraving on a cylinder -> IR. Cylinder axis is model +X.

use crate::ir::{Kind, Segment};

pub const RADIUS: f64 = 15.0;
pub const DEPTH: f64 = 0.2;
pub const TIP_R: f64 = RADIUS - DEPTH;
pub const CLEARANCE_R: f64 = RADIUS + 5.0;
pub const CONTACT_FEED: f64 = 300.0;
pub const PLUNGE_FEED: f64 = 100.0;
/// Chord tolerance in the unrolled plane, mm. Half the 0.01 mm toolpath tolerance (NFR-05).
pub const CHORD_TOL: f64 = 0.005;

/// The design in unrolled coordinates: x along the axis, theta wrapping 1.5 turns.
/// u in [0, 1]. Returns (x mm, theta degrees, continuous).
pub fn design(u: f64) -> (f64, f64) {
    let x = 30.0 + 10.0 * (2.0 * std::f64::consts::PI * 3.0 * u).sin();
    let theta = 540.0 * u;
    (x, theta)
}

fn unrolled(u: f64) -> [f64; 2] {
    let (x, th) = design(u);
    [x, TIP_R * th.to_radians()]
}

fn dist_to_chord(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len = dx.hypot(dy);
    if len == 0.0 {
        return (p[0] - a[0]).hypot(p[1] - a[1]);
    }
    ((p[0] - a[0]) * dy - (p[1] - a[1]) * dx).abs() / len
}

/// Deterministic bisection until every chord midpoint is within tolerance.
fn subdivide(u0: f64, u1: f64, out: &mut Vec<f64>, depth: u32) {
    let um = 0.5 * (u0 + u1);
    let dev = [0.25, 0.5, 0.75]
        .iter()
        .map(|t| dist_to_chord(unrolled(u0 + t * (u1 - u0)), unrolled(u0), unrolled(u1)))
        .fold(0.0, f64::max);
    if dev > CHORD_TOL && depth < 20 {
        subdivide(u0, um, out, depth + 1);
        subdivide(um, u1, out, depth + 1);
    } else {
        out.push(u1);
    }
}

pub fn model_point(x: f64, theta_deg: f64, r: f64) -> [f64; 3] {
    let (s, c) = theta_deg.to_radians().sin_cos();
    [x, r * c, r * s]
}

fn radial_axis(theta_deg: f64) -> [f64; 3] {
    // Tip towards holder = outward radial.
    let (s, c) = theta_deg.to_radians().sin_cos();
    [0.0, c, s]
}

/// A for "contact point at the top of the part" (machine +Z of the axis), unwrapped.
/// Machine angle of the contact point is 90 deg: theta + sigma*A = 90.
pub fn a_for_top(theta_deg: f64, a_sign: f64) -> f64 {
    a_sign * (90.0 - theta_deg)
}

pub struct Plan {
    pub segments: Vec<Segment>,
    /// u parameter of each Cut segment end (for deviation checks), same order as the Cut segments.
    pub cut_u: Vec<f64>,
    pub u_start: f64,
}

pub fn plan(a_sign: f64) -> Plan {
    let (x0, th0) = design(0.0);
    let seg = |kind, tip: [f64; 3], theta: f64, feed: Option<f64>, len: Option<f64>| Segment {
        kind,
        tip,
        axis: radial_axis(theta),
        a_deg: a_for_top(theta, a_sign),
        contact_feed: feed,
        duration_s: feed.zip(len).map(|(f, l)| l / f * 60.0),
        contact_len: len,
    };
    let mut segments = vec![
        seg(Kind::Rapid, model_point(x0, th0, CLEARANCE_R), th0, None, None),
        seg(Kind::Plunge, model_point(x0, th0, TIP_R), th0, Some(PLUNGE_FEED), Some(CLEARANCE_R - TIP_R)),
    ];
    let mut us = vec![0.0];
    subdivide(0.0, 1.0, &mut us, 0);
    let mut cut_u = Vec::new();
    for w in us.windows(2) {
        let (xa, ta) = design(w[0]);
        let (xb, tb) = design(w[1]);
        // Contact path inside the move is a helix: length = hypot(dx, r*dtheta).
        let len = (xb - xa).hypot(TIP_R * (tb - ta).to_radians());
        segments.push(seg(Kind::Cut, model_point(xb, tb, TIP_R), tb, Some(CONTACT_FEED), Some(len)));
        cut_u.push(w[1]);
    }
    let (xe, te) = design(1.0);
    segments.push(seg(Kind::Retract, model_point(xe, te, CLEARANCE_R), te, Some(PLUNGE_FEED), Some(CLEARANCE_R - TIP_R)));
    // Unwind: rapid A to the equivalent angle nearest zero, tip unchanged in the model frame.
    // The model-frame tip is identical; only the A label changes by whole turns.
    let a_end = a_for_top(te, a_sign);
    let a_unwound = a_end - 360.0 * (a_end / 360.0).round();
    let mut unwind = seg(Kind::Rapid, model_point(xe, te, CLEARANCE_R), te, None, None);
    unwind.a_deg = a_unwound;
    segments.push(unwind);
    Plan { segments, cut_u, u_start: 0.0 }
}
