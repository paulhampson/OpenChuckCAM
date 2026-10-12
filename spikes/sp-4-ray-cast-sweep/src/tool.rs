// SPDX-License-Identifier: AGPL-3.0-or-later
//! The two tool profiles the spike needs. SP-13 proved the general revolved profile; here only
//! the lower envelope `h(r)` is used (decision 0012 item 5).

#[derive(Clone, Copy, Debug)]
pub enum Tool {
    Flat { radius: f64 },
    Ball { radius: f64 },
}

impl Tool {
    pub fn radius(&self) -> f64 {
        match *self {
            Tool::Flat { radius } | Tool::Ball { radius } => radius,
        }
    }
    /// Height of the tool's lowest point at distance `r` from its axis, above the tip.
    pub fn h(&self, r: f64) -> f64 {
        match *self {
            Tool::Flat { .. } => 0.0,
            Tool::Ball { radius } => radius - (radius * radius - r * r).max(0.0).sqrt(),
        }
    }
    pub fn name(&self) -> &'static str {
        match self {
            Tool::Flat { .. } => "flat",
            Tool::Ball { .. } => "ball",
        }
    }
}
