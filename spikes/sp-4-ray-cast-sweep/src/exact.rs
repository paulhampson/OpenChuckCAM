// SPDX-License-Identifier: AGPL-3.0-or-later
//! Reference answers computed on the exact analytic surface, with no mesh and no ray casts:
//! the drop of a tool along a line, by dense sampling of the surface parameters.

use crate::part::Part;
use crate::tool::Tool;
use crate::{Vector, e2_dir, u_dir};

/// Height of the tool tip above the point `b`, measured along `u(psi)`, when the tool is
/// dropped along the line through `b` until it touches the exact surface. `None` if the line
/// misses the part.
pub fn drop(part: &Part, tool: &Tool, b: Vector, psi: f64) -> Option<f64> {
    let u = u_dir(psi);
    let e2 = e2_dir(psi);
    let rt = tool.radius();
    let eval = |x: f64, th: f64| -> Option<f64> {
        let p = part.point(x, th) - b;
        let (a, bb) = (p.x, p.dot(e2));
        let d = (a * a + bb * bb).sqrt();
        if d > rt {
            return None;
        }
        Some(p.dot(u) - tool.h(d))
    };
    let (x0, x1) = ((b.x - rt).max(0.0), (b.x + rt).min(part.length));
    let step = 0.025;
    let nx = (((x1 - x0) / step).ceil() as usize).max(1);
    let nth = ((std::f64::consts::TAU * part.r_max() / step).ceil() as usize).max(8);
    let mut cands: Vec<(f64, f64, f64)> = Vec::new();
    let mut best = f64::MIN;
    for i in 0..=nx {
        let x = x0 + (x1 - x0) * i as f64 / nx as f64;
        for j in 0..nth {
            let th = std::f64::consts::TAU * j as f64 / nth as f64;
            if let Some(z) = eval(x, th) {
                if z > best {
                    best = z;
                }
                cands.push((z, x, th));
            }
        }
    }
    if cands.is_empty() {
        return None;
    }
    // Refine around every cluster of coarse points within 0.05 mm of the best, on a 0.002 mm grid.
    cands.retain(|c| c.0 >= best - 0.05);
    cands.sort_by(|p, q| q.0.total_cmp(&p.0).then(p.1.total_cmp(&q.1)).then(p.2.total_cmp(&q.2)));
    let fs = 0.002;
    let n = (0.06 / fs) as i32;
    let dth = fs / part.r_min();
    let mut centres: Vec<(f64, f64)> = Vec::new();
    let mut z = best;
    for &(_, bx, bth) in &cands {
        if centres.len() >= 8 {
            break;
        }
        let far = |&(cx, cth): &(f64, f64)| {
            let dth = (bth - cth + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI;
            (bx - cx).abs() > 0.15 || (dth * part.r_min()).abs() > 0.15
        };
        if !centres.iter().all(far) {
            continue;
        }
        centres.push((bx, bth));
        for i in -n..=n {
            for j in -n..=n {
                let x = (bx + i as f64 * fs).clamp(0.0, part.length);
                let th = bth + j as f64 * dth;
                if let Some(v) = eval(x, th) {
                    if v > z {
                        z = v;
                    }
                }
            }
        }
    }
    Some(z)
}

/// Distance from point `c` to the exact surface, by a coarse search around the point's own
/// `(x, th)` and a 0.002 mm refinement. Used to judge ball centres: a ball of radius `rt`
/// whose centre is at distance `rt` touches the surface without gouging it.
pub fn distance(part: &Part, c: Vector, reach: f64) -> f64 {
    let th_c = c.y.atan2(c.z);
    let rmin = part.r_min();
    let (x0, x1) = ((c.x - reach).max(0.0), (c.x + reach).min(part.length));
    let wth = reach / rmin;
    let step = 0.05;
    let nx = (((x1 - x0) / step).ceil() as usize).max(1);
    let nth = ((2.0 * wth * rmin / step).ceil() as usize).max(2);
    let mut best = (f64::MAX, 0.0, 0.0);
    for i in 0..=nx {
        let x = x0 + (x1 - x0) * i as f64 / nx as f64;
        for j in 0..=nth {
            let th = th_c - wth + 2.0 * wth * j as f64 / nth as f64;
            let d = (part.point(x, th) - c).length();
            if d < best.0 {
                best = (d, x, th);
            }
        }
    }
    let fs = 0.002;
    let n = 40;
    let (mut dmin, bx, bth) = best;
    for i in -n..=n {
        for j in -n..=n {
            let x = (bx + i as f64 * fs).clamp(0.0, part.length);
            let th = bth + j as f64 * fs / rmin;
            dmin = dmin.min((part.point(x, th) - c).length());
        }
    }
    dmin
}
