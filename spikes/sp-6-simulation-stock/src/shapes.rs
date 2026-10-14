// SPDX-License-Identifier: AGPL-3.0-or-later
//! Standard tool types (TL-02, TL-03) mapped to the one profile type.
//!
//! Rule for every standard type: the cutting end (flat, ball, corner, taper,
//! V or point) runs from the tip to the cutting radius D/2, then a vertical
//! side runs to the flute length. A cutting end taller than the flute length
//! is rejected, so D, the angle and the flute length never silently disagree.

use crate::profile::{Profile, ProfileError, Pt, Segment};

fn bad(reason: impl Into<String>) -> ProfileError {
    ProfileError {
        segment: None,
        reason: reason.into(),
    }
}

fn positive(name: &str, v: f64) -> Result<(), ProfileError> {
    if v.is_finite() && v > 0.0 {
        Ok(())
    } else {
        Err(bad(format!("{name} must be a positive number, not {v}")))
    }
}

fn angle_between(name: &str, deg: f64, lo: f64, hi: f64) -> Result<f64, ProfileError> {
    if deg.is_finite() && deg > lo && deg < hi {
        Ok(deg.to_radians())
    } else {
        Err(bad(format!(
            "{name} must lie strictly between {lo} and {hi} degrees, not {deg}"
        )))
    }
}

/// Appends the vertical side from the end of the cutting end to the flute length.
fn side(
    mut segs: Vec<Segment>,
    r: f64,
    z_end_cut: f64,
    flute: f64,
    what: &str,
) -> Result<Vec<Segment>, ProfileError> {
    if z_end_cut > flute + 1e-9 {
        return Err(bad(format!(
            "the {what} reaches the cutting diameter at {z_end_cut:.4} mm, above the flute length {flute} mm"
        )));
    }
    if flute - z_end_cut > 1e-9 {
        segs.push(Segment::Line {
            to: Pt::new(r, flute),
        });
    }
    Ok(segs)
}

pub fn flat(d: f64, flute: f64) -> Result<Profile, ProfileError> {
    positive("diameter", d)?;
    positive("flute length", flute)?;
    let r = d / 2.0;
    Profile::new(&side(
        vec![Segment::Line {
            to: Pt::new(r, 0.0),
        }],
        r,
        0.0,
        flute,
        "flat end",
    )?)
}

pub fn ball(d: f64, flute: f64) -> Result<Profile, ProfileError> {
    positive("diameter", d)?;
    positive("flute length", flute)?;
    let r = d / 2.0;
    let segs = vec![Segment::Arc {
        to: Pt::new(r, r),
        centre: Pt::new(0.0, r),
    }];
    Profile::new(&side(segs, r, r, flute, "ball")?)
}

pub fn bull_nose(d: f64, corner: f64, flute: f64) -> Result<Profile, ProfileError> {
    positive("diameter", d)?;
    positive("corner radius", corner)?;
    positive("flute length", flute)?;
    let r = d / 2.0;
    if corner > r {
        return Err(bad(format!(
            "the corner radius {corner} mm exceeds the tool radius {r} mm"
        )));
    }
    let mut segs = Vec::new();
    if r - corner > 1e-9 {
        segs.push(Segment::Line {
            to: Pt::new(r - corner, 0.0),
        });
    }
    segs.push(Segment::Arc {
        to: Pt::new(r, corner),
        centre: Pt::new(r - corner, corner),
    });
    Profile::new(&side(segs, r, corner, flute, "corner")?)
}

/// Tapered ball: a ball of `tip_radius`, then a taper at `half_angle_deg`
/// from the axis, tangent to the ball, out to D/2.
pub fn tapered_ball(
    d: f64,
    tip_radius: f64,
    half_angle_deg: f64,
    flute: f64,
) -> Result<Profile, ProfileError> {
    positive("diameter", d)?;
    positive("tip radius", tip_radius)?;
    positive("flute length", flute)?;
    let a = angle_between("taper half angle", half_angle_deg, 0.0, 90.0)?;
    let r = d / 2.0;
    // Tangent point: the ball's normal there is at -a.
    let t = Pt::new(tip_radius * a.cos(), tip_radius * (1.0 - a.sin()));
    if t.r >= r - 1e-9 {
        return Err(bad(format!(
            "the tip radius {tip_radius} mm leaves no taper inside the tool radius {r} mm"
        )));
    }
    let z_top = t.z + (r - t.r) / a.tan();
    let segs = vec![
        Segment::Arc {
            to: t,
            centre: Pt::new(0.0, tip_radius),
        },
        Segment::Line {
            to: Pt::new(r, z_top),
        },
    ];
    Profile::new(&side(segs, r, z_top, flute, "taper")?)
}

/// V-bit or chamfer mill: a flat of `tip_diameter` (zero for a sharp point),
/// then flanks at half the included angle from the axis.
pub fn v_bit(
    d: f64,
    included_deg: f64,
    tip_diameter: f64,
    flute: f64,
) -> Result<Profile, ProfileError> {
    positive("diameter", d)?;
    positive("flute length", flute)?;
    let half = angle_between("included angle", included_deg, 0.0, 180.0)? / 2.0;
    if !(tip_diameter.is_finite() && tip_diameter >= 0.0 && tip_diameter < d) {
        return Err(bad(format!(
            "the tip diameter must be at least zero and below the diameter {d} mm, not {tip_diameter}"
        )));
    }
    let r = d / 2.0;
    let rt = tip_diameter / 2.0;
    let mut segs = Vec::new();
    if rt > 1e-9 {
        segs.push(Segment::Line {
            to: Pt::new(rt, 0.0),
        });
    }
    let z = (r - rt) / half.tan();
    segs.push(Segment::Line { to: Pt::new(r, z) });
    Profile::new(&side(segs, r, z, flute, "V")?)
}

pub fn chamfer(
    d: f64,
    included_deg: f64,
    tip_diameter: f64,
    flute: f64,
) -> Result<Profile, ProfileError> {
    v_bit(d, included_deg, tip_diameter, flute)
}

pub fn drill(d: f64, point_deg: f64, flute: f64) -> Result<Profile, ProfileError> {
    v_bit(d, point_deg, 0.0, flute)
}
