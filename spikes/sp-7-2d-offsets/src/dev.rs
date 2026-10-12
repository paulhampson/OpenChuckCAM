// SPDX-License-Identifier: AGPL-3.0-or-later
//! The cylinder's development: (x, A degrees) <-> (x, s = R * A in radians). The map is an
//! isometry, so a distance in the plane is a distance on the surface (a cylinder is
//! developable), and a straight segment in the plane is a straight (X, A) move.

use crate::geom::{flatten, Loop};

#[derive(Clone, Copy, Debug)]
pub struct Cyl {
    pub radius: f64,
}

impl Cyl {
    pub fn circumference(&self) -> f64 {
        std::f64::consts::TAU * self.radius
    }
    pub fn to_dev(&self, x: f64, a_deg: f64) -> [f64; 2] {
        [x, self.radius * a_deg.to_radians()]
    }
    pub fn to_angle(&self, p: [f64; 2]) -> [f64; 2] {
        [p[0], (p[1] / self.radius).to_degrees()]
    }
}

#[derive(Default, Clone, Copy, Debug)]
pub struct Counts {
    pub lines: usize,
    pub arcs: usize,
    pub moves: usize,
}

/// Convert a development loop back to (X, A) moves. Lines stay single moves; an arc becomes
/// chords with sagitta at most `tol`. Returns the closed vertex list and the segment counts.
pub fn loop_to_angles(l: &Loop, cyl: Cyl, tol: f64) -> (Vec<[f64; 2]>, Counts) {
    let mut c = Counts::default();
    for i in 0..l.len() {
        if l[i][2] == 0.0 {
            c.lines += 1;
        } else {
            c.arcs += 1;
        }
    }
    let pts: Vec<[f64; 2]> = flatten(l, tol).into_iter().map(|p| cyl.to_angle(p)).collect();
    c.moves = pts.len();
    (pts, c)
}
