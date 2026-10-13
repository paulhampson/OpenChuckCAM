// SPDX-License-Identifier: AGPL-3.0-or-later
//! Flank tool placement along fitted rulings, the 4-axis (XYZA) projection of the tool axis,
//! and an independent signed-deviation measure against the tool's flank cone.

use crate::detect::{Family, fit_line};
use crate::surface::{Surface, normal};
use crate::vec3::{V3, v3};

/// A straight (`half_angle` 0) or tapered tool, radius `radius` at the contact height. The
/// radius grows by `tan(half_angle)` per mm along the axis toward the spindle.
#[derive(Clone, Copy, Debug)]
pub struct Tool {
    pub radius: f64,
    pub half_angle: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// The tool axis exactly as the flank contact needs it (a 5-axis reference).
    Ideal,
    /// The axis projected into the YZ plane so that rotation about X can make it vertical.
    Axis4,
}

#[derive(Clone, Copy, Debug)]
pub struct Pose {
    /// Parameter of the ruling this pose follows.
    pub t: f64,
    pub contact: V3,
    /// Tool-axis point at the contact height (part frame).
    pub centre: V3,
    /// Unit axis toward the spindle (part frame).
    pub axis: V3,
    /// Angle (degrees) between the ideal axis and the axis used.
    pub tilt_deg: f64,
    /// A angle (radians, wrapped to (-pi, pi]) that makes `axis` vertical.
    pub a: f64,
}

#[derive(Debug)]
pub enum PlaceError {
    /// The needed axis lies along X: no A rotation makes it vertical.
    AxisAlongA,
    DegenerateNormal,
}

/// Which ruling direction `+d` or `-d` to use is decided once per toolpath: it must not flip
/// along the path, or A would jump by about pi.
#[derive(Clone, Copy, Debug)]
pub struct Plan {
    pub family: Family,
    /// `+1` puts the tool on the geometric normal side, `-1` on the other.
    pub side: f64,
    pub flip: bool,
}

fn line_at(s: &dyn Surface, fam: Family, t: f64, samples: usize) -> (V3, V3, V3, Option<V3>) {
    let (u0, u1, v0, v1) = s.domain();
    let (lo, hi) = if fam == Family::AlongV {
        (v0, v1)
    } else {
        (u0, u1)
    };
    let at = |x: f64| {
        if fam == Family::AlongV {
            (t, x)
        } else {
            (x, t)
        }
    };
    let pts: Vec<V3> = (0..samples)
        .map(|k| {
            let (u, v) = at(lo + (hi - lo) * k as f64 / (samples - 1) as f64);
            s.point(u, v)
        })
        .collect();
    let (_, d) = fit_line(&pts);
    let mid = at(0.5 * (lo + hi));
    (s.point(mid.0, mid.1), d, pts[0], normal(s, mid.0, mid.1))
}

/// Ideal flank axis for one ruling: `ta = cos(a) d + sin(a) n` with `d` the ruling and `n`
/// the unit normal on the tool side; the tool body is `-n` of the contact point.
fn ideal(plan: Plan, tool: Tool, d: V3, nrm: V3) -> Result<(V3, V3), PlaceError> {
    let n = nrm * plan.side;
    let mut d = d;
    d = (d - n * d.dot(n))
        .unit()
        .ok_or(PlaceError::DegenerateNormal)?;
    if plan.flip {
        d = -d;
    }
    let (sa, ca) = tool.half_angle.sin_cos();
    let axis = d * ca + n * sa;
    // Centre of the cone's axis at the contact height: q - r e, e = sin(a) d - cos(a) n.
    let back = (n * ca - d * sa) * tool.radius;
    Ok((axis, back))
}

pub fn place(
    s: &dyn Surface,
    tool: Tool,
    plan: Plan,
    t: f64,
    mode: Mode,
    samples: usize,
) -> Result<Pose, PlaceError> {
    let (q, d, first, n) = line_at(s, plan.family, t, samples);
    let n = n.ok_or(PlaceError::DegenerateNormal)?;
    // Orient the fitted direction along increasing parameter, as `fit_line` does.
    let d = if d.dot(q - first) < 0.0 { -d } else { d };
    let (ta, back) = ideal(plan, tool, d, n)?;
    let centre = q + back;
    let (axis, tilt) = match mode {
        Mode::Ideal => (ta, 0.0),
        Mode::Axis4 => {
            let yz = v3(0.0, ta.y, ta.z).unit().ok_or(PlaceError::AxisAlongA)?;
            (yz, ta.dot(yz).clamp(-1.0, 1.0).acos().to_degrees())
        }
    };
    let a = axis.y.atan2(axis.z);
    Ok(Pose {
        t,
        contact: q,
        centre,
        axis,
        tilt_deg: tilt,
        a,
    })
}

/// Choose `flip` at the first ruling so the 4-axis tilt is smallest.
pub fn plan_for(s: &dyn Surface, tool: Tool, family: Family, side: f64, samples: usize) -> Plan {
    let (u0, u1, v0, v1) = s.domain();
    let t = if family == Family::AlongV {
        0.5 * (u0 + u1)
    } else {
        0.5 * (v0 + v1)
    };
    let tilt = |flip: bool| {
        let plan = Plan { family, side, flip };
        place(s, tool, plan, t, Mode::Axis4, samples)
            .map(|p| p.tilt_deg)
            .unwrap_or(f64::MAX)
    };
    let (a, b) = (tilt(false), tilt(true));
    Plan {
        family,
        side,
        flip: b < a - 1e-9,
    }
}

/// `n` poses over the parameter range of the chosen family (the rulings are the iso-lines
/// of the other parameter), with A unwrapped to be continuous.
pub fn toolpath(
    s: &dyn Surface,
    tool: Tool,
    plan: Plan,
    n: usize,
    mode: Mode,
    samples: usize,
) -> Result<Vec<Pose>, PlaceError> {
    let (u0, u1, v0, v1) = s.domain();
    let (lo, hi) = if plan.family == Family::AlongV {
        (u0, u1)
    } else {
        (v0, v1)
    };
    let mut out: Vec<Pose> = Vec::with_capacity(n);
    for k in 0..n {
        let t = lo + (hi - lo) * (k as f64 + 0.5) / n as f64;
        let mut p = place(s, tool, plan, t, mode, samples)?;
        if let Some(prev) = out.last() {
            let two_pi = std::f64::consts::TAU;
            while p.a - prev.a > std::f64::consts::PI {
                p.a -= two_pi;
            }
            while p.a - prev.a < -std::f64::consts::PI {
                p.a += two_pi;
            }
        }
        out.push(p);
    }
    Ok(out)
}

/// Signed distance (mm) from `p` to the tool's flank cone: negative inside the tool
/// (gouge), positive outside (material left).
pub fn signed_dev(tool: Tool, pose: &Pose, p: V3) -> f64 {
    let w = p - pose.centre;
    let h = w.dot(pose.axis);
    let rho = (w - pose.axis * h).len();
    let rr = tool.radius + h * tool.half_angle.tan();
    (rho - rr) * tool.half_angle.cos()
}

/// The same measure in the machine frame: rotate the point and the pose by A so the axis is
/// vertical. Used to check the part-frame and machine-frame answers agree.
pub fn signed_dev_machine(tool: Tool, pose: &Pose, p: V3) -> f64 {
    let c = pose.centre.rot_x(pose.a);
    let ax = pose.axis.rot_x(pose.a);
    let q = p.rot_x(pose.a);
    let w = q - c;
    let h = w.dot(ax);
    let rho = (w - ax * h).len();
    (rho - (tool.radius + h * tool.half_angle.tan())) * tool.half_angle.cos()
}

#[derive(Clone, Copy, Debug)]
pub struct Metrics {
    /// Deepest overcut (mm, >= 0) over the surface samples.
    pub gouge: f64,
    /// Largest material left (mm, >= 0).
    pub left: f64,
    pub samples: usize,
}

/// Union of the tool bodies at all poses against a grid over the whole patch: the effective
/// signed distance at a sample is the smallest over poses. Samples on the patch boundary rim
/// are skipped (no pose covers beyond the first and last ruling).
pub fn measure(s: &dyn Surface, tool: Tool, poses: &[Pose], nu: usize, nv: usize) -> Metrics {
    let (u0, u1, v0, v1) = s.domain();
    let (mut gouge, mut left) = (0.0f64, 0.0f64);
    for i in 0..nu {
        let u = u0 + (u1 - u0) * (0.05 + 0.9 * i as f64 / (nu - 1) as f64);
        for j in 0..nv {
            let v = v0 + (v1 - v0) * (j as f64 / (nv - 1) as f64);
            let p = s.point(u, v);
            let d = poses
                .iter()
                .map(|ps| signed_dev(tool, ps, p))
                .fold(f64::INFINITY, f64::min);
            if d < 0.0 {
                gouge = gouge.max(-d);
            } else {
                left = left.max(d);
            }
        }
    }
    Metrics {
        gouge,
        left,
        samples: nu * nv,
    }
}

/// Total A travel and the largest step between consecutive poses, in degrees.
pub fn a_travel(poses: &[Pose]) -> (f64, f64) {
    let mut total = 0.0f64;
    let mut worst = 0.0f64;
    for w in poses.windows(2) {
        let d = (w[1].a - w[0].a).abs().to_degrees();
        total += d;
        worst = worst.max(d);
    }
    (total, worst)
}
