// SPDX-License-Identifier: AGPL-3.0-or-later
//! Analytic test parts, generated procedurally (NFR-08, work breakdown slice 0 item 10).
//!
//! Each part is a closed surface of revolution about the X axis (the rotary
//! axis) whose exact radius is known at every `(x, angle)`, so a toolpath or a
//! simulation can be compared with the closed-form answer. The analytic
//! description is the oracle; the mesh is what the engine sees.
//!
//! The ruled surface (a hyperboloid of one sheet) is made of straight rulings,
//! the case swarf machining needs. Generated parts need no files in
//! `testdata/`, so they carry no third-party licence.

use openchuck_types::geometry::Point3;
use openchuck_types::mesh::TriangleMesh;

/// A closed surface of revolution about X with an exact radius function.
pub trait RadialPart {
    /// Short name for reports.
    fn name(&self) -> &'static str;
    /// The X range the part spans.
    fn x_range(&self) -> (f64, f64);
    /// Exact distance from the X axis to the surface at `x`, polar angle
    /// `angle` (radians, measured from +Y towards +Z).
    fn radius_at(&self, x: f64, angle: f64) -> f64;

    /// The surface as a closed mesh with `rings` stations along X and
    /// `around` vertices per station. Rings of zero radius collapse to one
    /// vertex, so poles give no degenerate triangles.
    fn mesh(&self, rings: u32, around: u32) -> TriangleMesh {
        radial_mesh(self, rings.max(2), around.max(3))
    }
}

/// A plain cylinder.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cylinder {
    /// Radius.
    pub radius: f64,
    /// Length along X, starting at x = 0.
    pub length: f64,
}

/// A truncated cone.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cone {
    /// Radius at x = 0.
    pub start_radius: f64,
    /// Radius at x = `length`.
    pub end_radius: f64,
    /// Length along X.
    pub length: f64,
}

/// A sphere centred on the origin.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sphere {
    /// Radius.
    pub radius: f64,
}

/// A cylinder with a helical groove of cosine section.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HelicalGroove {
    /// Radius of the uncut cylinder.
    pub radius: f64,
    /// Length along X.
    pub length: f64,
    /// Axial advance per turn.
    pub pitch: f64,
    /// Groove depth.
    pub depth: f64,
}

/// A hyperboloid of one sheet: a ruled surface whose rulings join a circle at
/// x = 0 to the same circle at x = `length`, turned by `twist` radians.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RuledHyperboloid {
    /// Radius of the end circles.
    pub end_radius: f64,
    /// Length along X.
    pub length: f64,
    /// Twist between the ends, in radians (less than pi).
    pub twist: f64,
}

/// The analytic part of spike SP-4 (decision record 0015):
/// `r(x, angle) = base + wave sin(2 pi x / wave_length) + lobe cos(lobes angle)`.
/// With no lobe it is a vase whose profile is an S-curve along X; with lobes
/// its normal leaves the plane through the axis, so the best tool axis is not
/// the one through the axis (OP-06).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LobedVase {
    /// Mean radius.
    pub base: f64,
    /// Amplitude of the S-curve along X.
    pub wave: f64,
    /// Wave length of the S-curve.
    pub wave_length: f64,
    /// Amplitude of the lobes round the axis.
    pub lobe: f64,
    /// Number of lobes.
    pub lobes: u32,
    /// Length along X, starting at x = 0.
    pub length: f64,
}

impl LobedVase {
    /// The exact outward unit normal at `(x, angle)`, from the partial
    /// derivatives of the radius in closed form.
    pub fn normal(&self, x: f64, angle: f64) -> [f64; 3] {
        let tau = std::f64::consts::TAU;
        let r = self.radius_at(x, angle);
        let dr_dx = self.wave * tau / self.wave_length * (tau * x / self.wave_length).cos();
        let n = f64::from(self.lobes);
        let dr_da = -self.lobe * n * (n * angle).sin();
        let (s, c) = angle.sin_cos();
        // Surface p(x, a) = (x, r c, r s); normal = p_a x p_x, outward.
        let pa = [0.0, dr_da * c - r * s, dr_da * s + r * c];
        let px = [1.0, dr_dx * c, dr_dx * s];
        let v = [
            pa[1] * px[2] - pa[2] * px[1],
            pa[2] * px[0] - pa[0] * px[2],
            pa[0] * px[1] - pa[1] * px[0],
        ];
        let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        [v[0] / l, v[1] / l, v[2] / l]
    }
}

impl RadialPart for LobedVase {
    fn name(&self) -> &'static str {
        if self.lobe == 0.0 {
            "vase"
        } else {
            "lobed_vase"
        }
    }
    fn x_range(&self) -> (f64, f64) {
        (0.0, self.length)
    }
    fn radius_at(&self, x: f64, angle: f64) -> f64 {
        let tau = std::f64::consts::TAU;
        self.base
            + self.wave * (tau * x / self.wave_length).sin()
            + self.lobe * (f64::from(self.lobes) * angle).cos()
    }
}

impl RadialPart for Cylinder {
    fn name(&self) -> &'static str {
        "cylinder"
    }
    fn x_range(&self) -> (f64, f64) {
        (0.0, self.length)
    }
    fn radius_at(&self, _x: f64, _angle: f64) -> f64 {
        self.radius
    }
}

impl RadialPart for Cone {
    fn name(&self) -> &'static str {
        "cone"
    }
    fn x_range(&self) -> (f64, f64) {
        (0.0, self.length)
    }
    fn radius_at(&self, x: f64, _angle: f64) -> f64 {
        self.start_radius + (self.end_radius - self.start_radius) * x / self.length
    }
}

impl RadialPart for Sphere {
    fn name(&self) -> &'static str {
        "sphere"
    }
    fn x_range(&self) -> (f64, f64) {
        (-self.radius, self.radius)
    }
    fn radius_at(&self, x: f64, _angle: f64) -> f64 {
        (self.radius * self.radius - x * x).max(0.0).sqrt()
    }
}

impl RadialPart for HelicalGroove {
    fn name(&self) -> &'static str {
        "helical_groove"
    }
    fn x_range(&self) -> (f64, f64) {
        (0.0, self.length)
    }
    fn radius_at(&self, x: f64, angle: f64) -> f64 {
        // Phase along the helix: zero on the groove centre line.
        let phase = std::f64::consts::TAU * x / self.pitch - angle;
        self.radius - self.depth * 0.5 * (1.0 + phase.cos())
    }
}

impl RadialPart for RuledHyperboloid {
    fn name(&self) -> &'static str {
        "ruled_hyperboloid"
    }
    fn x_range(&self) -> (f64, f64) {
        (0.0, self.length)
    }
    fn radius_at(&self, x: f64, _angle: f64) -> f64 {
        // A ruling runs between two points of the end circles `twist` apart;
        // its closest approach to the axis is at mid-length.
        let waist = self.end_radius * (self.twist / 2.0).cos();
        let half_chord = self.end_radius * (self.twist / 2.0).sin();
        let t = 2.0 * x / self.length - 1.0; // -1 at x = 0, +1 at x = length
        (waist * waist + (half_chord * t).powi(2)).sqrt()
    }
}

impl RuledHyperboloid {
    /// The ruling through the point at angle `angle` on the x = 0 circle, as
    /// its two end points. Every point of the segment lies on the surface.
    pub fn ruling(&self, angle: f64) -> (Point3, Point3) {
        let r = self.end_radius;
        let end = angle + self.twist;
        (
            Point3::new(0.0, r * angle.cos(), r * angle.sin()),
            Point3::new(self.length, r * end.cos(), r * end.sin()),
        )
    }
}

fn radial_mesh<P: RadialPart + ?Sized>(part: &P, rings: u32, around: u32) -> TriangleMesh {
    let (x0, x1) = part.x_range();
    let mut vertices = Vec::new();
    let mut triangles = Vec::new();
    // Each ring is either a single pole vertex or `around` vertices.
    let mut ring_starts: Vec<(u32, bool)> = Vec::new();
    for i in 0..rings {
        let x = x0 + (x1 - x0) * f64::from(i) / f64::from(rings - 1);
        let start = u32::try_from(vertices.len()).unwrap_or(u32::MAX);
        let pole = part.radius_at(x, 0.0) <= 1e-12
            && (0..around).all(|j| {
                part.radius_at(x, std::f64::consts::TAU * f64::from(j) / f64::from(around)) <= 1e-12
            });
        if pole {
            vertices.push(Point3::new(x, 0.0, 0.0));
        } else {
            for j in 0..around {
                let angle = std::f64::consts::TAU * f64::from(j) / f64::from(around);
                let r = part.radius_at(x, angle);
                vertices.push(Point3::new(x, r * angle.cos(), r * angle.sin()));
            }
        }
        ring_starts.push((start, pole));
    }
    // Side strips; the outward normal points away from the axis.
    for pair in ring_starts.windows(2) {
        let &[(a, a_pole), (b, b_pole)] = pair else {
            continue;
        };
        for j in 0..around {
            let k = (j + 1) % around;
            match (a_pole, b_pole) {
                (false, false) => {
                    triangles.push([a + j, b + k, b + j]);
                    triangles.push([a + j, a + k, b + k]);
                }
                (true, false) => triangles.push([a, b + k, b + j]),
                (false, true) => triangles.push([a + j, a + k, b]),
                (true, true) => {}
            }
        }
    }
    // End caps for open ends.
    let mut cap = |ring: (u32, bool), x: f64, outward_positive: bool| {
        let (start, pole) = ring;
        if pole {
            return;
        }
        let centre = u32::try_from(vertices.len()).unwrap_or(u32::MAX);
        vertices.push(Point3::new(x, 0.0, 0.0));
        for j in 0..around {
            let k = (j + 1) % around;
            if outward_positive {
                triangles.push([centre, start + j, start + k]);
            } else {
                triangles.push([centre, start + k, start + j]);
            }
        }
    };
    if let (Some(&first), Some(&last)) = (ring_starts.first(), ring_starts.last()) {
        cap(first, x0, false);
        cap(last, x1, true);
    }
    TriangleMesh {
        vertices,
        triangles,
    }
}

/// The standard set of analytic parts with modest dimensions (millimetres).
pub fn standard_parts() -> Vec<Box<dyn RadialPart>> {
    vec![
        Box::new(Cylinder {
            radius: 15.0,
            length: 60.0,
        }),
        Box::new(Cone {
            start_radius: 20.0,
            end_radius: 8.0,
            length: 50.0,
        }),
        Box::new(Sphere { radius: 20.0 }),
        Box::new(HelicalGroove {
            radius: 15.0,
            length: 60.0,
            pitch: 20.0,
            depth: 2.0,
        }),
        Box::new(RuledHyperboloid {
            end_radius: 15.0,
            length: 40.0,
            twist: 1.0,
        }),
    ]
}

/// The benchmark model for NFR-03: a cylinder of 1,000,000 side triangles
/// (plus 2,000 cap triangles), generated so nothing large is committed.
pub fn benchmark_model_1m() -> TriangleMesh {
    Cylinder {
        radius: 25.0,
        length: 100.0,
    }
    .mesh(501, 1000)
}
