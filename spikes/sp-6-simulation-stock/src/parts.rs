// SPDX-License-Identifier: AGPL-3.0-or-later
//! Analytic test parts and the reference oracles. Nothing here shares code
//! with the stock candidates except the tool profile's `height_at`.

use std::f64::consts::PI;

use crate::profile::Profile;
use crate::stock::Pose;

/// Stock radius and length used by every test (mm).
pub const RS: f64 = 25.0;
pub const LEN: f64 = 40.0;

/// A body of revolution on the A axis, `r(x)` for `x` in `[xa, xb]`.
pub struct Part {
    pub name: &'static str,
    pub xa: f64,
    pub xb: f64,
    pub r: Box<dyn Fn(f64) -> f64 + Send + Sync>,
    pub dr: Box<dyn Fn(f64) -> f64 + Send + Sync>,
    pub volume: f64,
}

impl Part {
    /// Lateral area by quadrature.
    pub fn area(&self) -> f64 {
        let n = 20000;
        let h = (self.xb - self.xa) / n as f64;
        (0..n)
            .map(|k| {
                let x = self.xa + (k as f64 + 0.5) * h;
                2.0 * PI * (self.r)(x) * (1.0 + (self.dr)(x).powi(2)).sqrt() * h
            })
            .sum()
    }
}

pub fn cylinder() -> Part {
    let (xa, xb, r) = (2.0, 38.0, 20.0);
    Part {
        name: "cylinder r 20",
        xa,
        xb,
        r: Box::new(move |_| r),
        dr: Box::new(|_| 0.0),
        volume: PI * r * r * (xb - xa),
    }
}

pub fn cone() -> Part {
    // r = 8 + 0.3 x; frustum volume pi h / 3 (r1^2 + r1 r2 + r2^2).
    let (xa, xb) = (2.0, 38.0);
    let (r1, r2) = (8.6, 19.4);
    Part {
        name: "cone 16.7 deg",
        xa,
        xb,
        r: Box::new(|x| 8.0 + 0.3 * x),
        dr: Box::new(|_| 0.3),
        volume: PI * (xb - xa) / 3.0 * (r1 * r1 + r1 * r2 + r2 * r2),
    }
}

pub fn sphere() -> Part {
    // Sphere r 20 centred at x 20, the zone |x - 20| <= 8.
    let (xa, xb, c, big) = (12.0, 28.0, 20.0, 20.0);
    Part {
        name: "sphere r 20",
        xa,
        xb,
        r: Box::new(move |x| (big * big - (x - c) * (x - c)).sqrt()),
        dr: Box::new(move |x| -(x - c) / (big * big - (x - c) * (x - c)).sqrt()),
        volume: PI * (big * big * (xb - xa) - ((xb - c).powi(3) - (xa - c).powi(3)) / 3.0),
    }
}

/// Poses that turn `part` out of the stock with a ball tool of radius `rho`:
/// the ball centre follows the offset curve, the part makes a full turn at
/// each station, and the envelope of the balls is the part.
pub fn ball_turning_poses(part: &Part, rho: f64, x_step: f64, a_step_deg: f64) -> Vec<Pose> {
    let ns = ((part.xb - part.xa) / x_step).ceil() as usize;
    let na = (360.0 / a_step_deg).round() as usize;
    let mut v = Vec::with_capacity((ns + 1) * na);
    for k in 0..=ns {
        let x = part.xa + (part.xb - part.xa) * k as f64 / ns as f64;
        let (r, s) = ((part.r)(x), (part.dr)(x));
        let n = (1.0 + s * s).sqrt();
        let (cx, cr) = (x - rho * s / n, r + rho / n);
        for m in 0..na {
            v.push(Pose {
                tip: [cx, 0.0, cr - rho],
                a: (m as f64 * a_step_deg).to_radians(),
            });
        }
    }
    v
}

/// Reference volume removed from the cylinder `(x in [0, LEN], radius RS)` by
/// one straight pass of a vertical tool at A = 0: tip at height `z0` along
/// `x in [xs, xe]` on the plane y = 0. Quadrature over the plan view; the
/// bottom of the cut at a point is `z0 + h(distance to the path)`.
pub fn groove_volume(profile: &Profile, z0: f64, xs: f64, xe: f64, step: f64) -> f64 {
    let r = profile.r_max();
    let ze = profile.z_end();
    let (nx, ny) = (
        (((xe - xs) + 2.0 * r) / step).ceil() as usize,
        (2.0 * r / step).ceil() as usize,
    );
    let mut v = 0.0;
    for i in 0..nx {
        let x = xs - r + (i as f64 + 0.5) * step;
        if !(0.0..=LEN).contains(&x) {
            continue;
        }
        let dx = if x < xs {
            xs - x
        } else if x > xe {
            x - xe
        } else {
            0.0
        };
        for j in 0..ny {
            let y = -r + (j as f64 + 0.5) * step;
            let dist = dx.hypot(y);
            let Some(h) = profile.height_at(dist) else {
                continue;
            };
            if y.abs() >= RS {
                continue;
            }
            let top = (RS * RS - y * y).sqrt();
            let lo = (z0 + h).max(-top);
            let hi = (z0 + ze).min(top);
            v += (hi - lo).max(0.0) * step * step;
        }
    }
    v
}
