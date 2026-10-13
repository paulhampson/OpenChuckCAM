// SPDX-License-Identifier: AGPL-3.0-or-later
//! Ruled-surface detection from point samples only (OP-07): for each family of iso-lines,
//! fit a straight line to the samples and report the worst perpendicular distance, plus
//! how far the surface normal turns along the line (zero for a developable surface).

use crate::surface::{Surface, normal};
use crate::vec3::{V3, v3};

/// Which parameter runs along the candidate rulings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    /// Lines of constant `u`, running in `v`.
    AlongV,
    /// Lines of constant `v`, running in `u`.
    AlongU,
}

#[derive(Clone, Copy, Debug)]
pub struct FamilyFit {
    pub family: Family,
    /// Worst distance (mm) of any sample from its fitted line.
    pub residual: f64,
    /// Largest angle (degrees) between the normals at the two ends of a line and the middle.
    pub twist_deg: f64,
}

#[derive(Clone, Debug)]
pub struct Detection {
    pub u: FamilyFit,
    pub v: FamilyFit,
    /// The better family (smaller residual; `AlongV` on a tie), when it is within `tol`.
    pub chosen: Option<FamilyFit>,
    pub tol: f64,
}

impl Detection {
    pub fn best(&self) -> FamilyFit {
        if self.u.residual < self.v.residual {
            self.u
        } else {
            self.v
        }
    }
    /// The refusal message of slice 17 item 4.
    pub fn refusal(&self) -> Option<String> {
        if self.chosen.is_some() {
            return None;
        }
        let b = self.best();
        Some(format!(
            "not a ruled surface within tolerance: best straight-line fit misses by {:.4} mm (limit {:.4} mm)",
            b.residual, self.tol
        ))
    }
}

/// Best-fit line through `pts`: centroid and unit direction (principal axis of the
/// covariance, by a fixed number of power iterations started from the end-to-end chord, so
/// the result is deterministic).
pub fn fit_line(pts: &[V3]) -> (V3, V3) {
    let n = pts.len() as f64;
    let mut c = V3::default();
    for p in pts {
        c = c + *p;
    }
    c = c * (1.0 / n);
    let mut m = [[0.0f64; 3]; 3];
    for p in pts {
        let d = *p - c;
        let a = [d.x, d.y, d.z];
        for i in 0..3 {
            for j in 0..3 {
                m[i][j] += a[i] * a[j];
            }
        }
    }
    let chord = (*pts.last().unwrap() - pts[0])
        .unit()
        .unwrap_or(v3(1.0, 0.0, 0.0));
    let mut d = chord;
    for _ in 0..60 {
        let a = [d.x, d.y, d.z];
        let w = v3(
            m[0][0] * a[0] + m[0][1] * a[1] + m[0][2] * a[2],
            m[1][0] * a[0] + m[1][1] * a[1] + m[1][2] * a[2],
            m[2][0] * a[0] + m[2][1] * a[1] + m[2][2] * a[2],
        );
        match w.unit() {
            Some(nw) => d = nw,
            None => break,
        }
    }
    if d.dot(chord) < 0.0 {
        d = -d;
    }
    (c, d)
}

fn dist_to_line(p: V3, c: V3, d: V3) -> f64 {
    let w = p - c;
    (w - d * w.dot(d)).len()
}

fn point_at(s: &dyn Surface, fam: Family, fixed: f64, t: f64) -> V3 {
    match fam {
        Family::AlongV => s.point(fixed, t),
        Family::AlongU => s.point(t, fixed),
    }
}
fn normal_at(s: &dyn Surface, fam: Family, fixed: f64, t: f64) -> Option<V3> {
    match fam {
        Family::AlongV => normal(s, fixed, t),
        Family::AlongU => normal(s, t, fixed),
    }
}

/// The worst straight-line residual and normal twist over `lines` iso-lines of one family,
/// each sampled at `samples` points.
pub fn fit_family(s: &dyn Surface, fam: Family, lines: usize, samples: usize) -> FamilyFit {
    let (u0, u1, v0, v1) = s.domain();
    let ((f0, f1), (t0, t1)) = match fam {
        Family::AlongV => ((u0, u1), (v0, v1)),
        Family::AlongU => ((v0, v1), (u0, u1)),
    };
    let mut residual = 0.0f64;
    let mut twist = 0.0f64;
    for i in 0..lines {
        let fixed = f0 + (f1 - f0) * (i as f64 + 0.5) / lines as f64;
        let pts: Vec<V3> = (0..samples)
            .map(|k| {
                point_at(
                    s,
                    fam,
                    fixed,
                    t0 + (t1 - t0) * k as f64 / (samples - 1) as f64,
                )
            })
            .collect();
        let (c, d) = fit_line(&pts);
        for p in &pts {
            residual = residual.max(dist_to_line(*p, c, d));
        }
        let nm = normal_at(s, fam, fixed, 0.5 * (t0 + t1));
        for t in [t0 + 1e-3 * (t1 - t0), t1 - 1e-3 * (t1 - t0)] {
            if let (Some(a), Some(b)) = (normal_at(s, fam, fixed, t), nm) {
                twist = twist.max(a.dot(b).clamp(-1.0, 1.0).acos().to_degrees());
            }
        }
    }
    FamilyFit {
        family: fam,
        residual,
        twist_deg: twist,
    }
}

/// Detect rulings. A surface is accepted when one family fits within `tol` (mm).
pub fn detect(s: &dyn Surface, tol: f64, lines: usize, samples: usize) -> Detection {
    let v = fit_family(s, Family::AlongV, lines, samples);
    let u = fit_family(s, Family::AlongU, lines, samples);
    let pick = if u.residual < v.residual { u } else { v };
    Detection {
        u,
        v,
        chosen: (pick.residual <= tol).then_some(pick),
        tol,
    }
}
