// SPDX-License-Identifier: AGPL-3.0-only
// Minimal 3-vector helpers for the spike (the product uses openchuck-types).
pub type V3 = [f64; 3];
pub fn add(a: V3, b: V3) -> V3 { [a[0] + b[0], a[1] + b[1], a[2] + b[2]] }
pub fn sub(a: V3, b: V3) -> V3 { [a[0] - b[0], a[1] - b[1], a[2] - b[2]] }
pub fn mul(a: V3, s: f64) -> V3 { [a[0] * s, a[1] * s, a[2] * s] }
pub fn dot(a: V3, b: V3) -> f64 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] }
pub fn cross(a: V3, b: V3) -> V3 { [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]] }
pub fn len(a: V3) -> f64 { dot(a, a).sqrt() }
pub fn norm(a: V3) -> V3 { let l = len(a); if l == 0.0 { a } else { mul(a, 1.0 / l) } }
pub fn dist(a: V3, b: V3) -> f64 { len(sub(a, b)) }
/// Angle between two directions, in degrees.
pub fn angle_deg(a: V3, b: V3) -> f64 { dot(norm(a), norm(b)).clamp(-1.0, 1.0).acos().to_degrees() }
