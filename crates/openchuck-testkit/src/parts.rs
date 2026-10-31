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

/// A carving test piece (issue #382): a cylinder with a relief of raised
/// strokes, like lettering or an ornament, on a grid of cells, each with
/// chamfered walls that meet the background and the stroke's top in sharp
/// edges, over a gentle wave. Which stroke a cell holds comes from a fixed
/// hash of the cell, so the part is the same on every platform. Fine detail
/// and sharp edges are what make a fine-stepover V-bit finish slow.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CarvedRelief {
    /// Radius of the background.
    pub radius: f64,
    /// Length along X, starting at x = 0.
    pub length: f64,
    /// Height of the strokes above the background.
    pub depth: f64,
    /// Size of a cell along X; round the axis the cells are as near this as
    /// closes the circle.
    pub cell: f64,
}

impl CarvedRelief {
    /// The test piece the carving benchmarks use: 24 mm across, 30 mm long,
    /// strokes 0.8 mm high on 5 mm cells.
    pub const STANDARD: Self = Self {
        radius: 12.0,
        length: 30.0,
        depth: 0.8,
        cell: 5.0,
    };

    /// The height of the relief above the background at `(x, angle)`.
    fn relief(&self, x: f64, angle: f64) -> f64 {
        let around = (std::f64::consts::TAU * self.radius / self.cell)
            .round()
            .max(1.0);
        // Cell coordinates: whole numbers at the cell corners.
        let u = x / self.cell;
        let v = angle.rem_euclid(std::f64::consts::TAU) / std::f64::consts::TAU * around;
        let (iu, iv) = (u.floor(), v.floor());
        let (fu, fv) = (u - iu, v - iv);
        // A fixed integer hash of the cell (wrapping round the axis).
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let key = {
            let a = iu as i64 as u64;
            let b = (iv as i64).rem_euclid(around as i64) as u64;
            let mut h =
                a.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ b.wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
            h ^= h >> 29;
            h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
            h ^ (h >> 32)
        };
        // Distance, in cells, from the stroke's centre line.
        let segment = |(ax, ay): (f64, f64), (bx, by): (f64, f64)| {
            let (dx, dy) = (bx - ax, by - ay);
            let t = (((fu - ax) * dx + (fv - ay) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
            ((fu - ax - t * dx).powi(2) + (fv - ay - t * dy).powi(2)).sqrt()
        };
        let d = match key % 4 {
            0 => segment((0.2, 0.2), (0.8, 0.8)),
            1 => segment((0.2, 0.8), (0.8, 0.2)),
            2 => (((fu - 0.5).powi(2) + (fv - 0.5).powi(2)).sqrt() - 0.28).abs(),
            _ => segment((0.2, 0.5), (0.8, 0.5)).min(segment((0.5, 0.2), (0.5, 0.8))),
        };
        // A flat top 0.12 cells across, then a chamfer 0.08 cells wide.
        let (half, wall) = (0.06, 0.08);
        self.depth * ((half + wall - d) / wall).clamp(0.0, 1.0)
    }
}

impl RadialPart for CarvedRelief {
    fn name(&self) -> &'static str {
        "carved_relief"
    }
    fn x_range(&self) -> (f64, f64) {
        (0.0, self.length)
    }
    fn radius_at(&self, x: f64, angle: f64) -> f64 {
        let wave = 0.3 * (std::f64::consts::TAU * x / 7.0).sin() * (3.0 * angle).cos();
        self.radius + wave + self.relief(x, angle)
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

/// A closed surface of revolution about X from a profile of `(x, radius)`
/// points, in increasing X (a profile may turn back in X, for an undercut
/// such as a dovetail groove) and joined by straight lines, with `around`
/// vertices per ring. Two points at the same X make a wall (an annulus), so a
/// stepped shaft has a true inside corner. The first and last points are
/// capped; their radius must be above zero.
pub fn revolve(profile: &[(f64, f64)], around: u32) -> TriangleMesh {
    let around = around.max(3);
    let mut vertices = Vec::new();
    let mut triangles = Vec::new();
    let mut starts = Vec::new();
    for &(x, r) in profile {
        starts.push(u32::try_from(vertices.len()).unwrap_or(u32::MAX));
        for j in 0..around {
            let angle = std::f64::consts::TAU * f64::from(j) / f64::from(around);
            vertices.push(Point3::new(x, r * angle.cos(), r * angle.sin()));
        }
    }
    for pair in starts.windows(2) {
        let &[a, b] = pair else { continue };
        for j in 0..around {
            let k = (j + 1) % around;
            triangles.push([a + j, b + k, b + j]);
            triangles.push([a + j, a + k, b + k]);
        }
    }
    for (ring, x, outward_positive) in [
        (starts.first().copied(), profile.first(), false),
        (starts.last().copied(), profile.last(), true),
    ]
    .into_iter()
    .filter_map(|(s, p, o)| Some((s?, p?.0, o)))
    {
        let centre = u32::try_from(vertices.len()).unwrap_or(u32::MAX);
        vertices.push(Point3::new(x, 0.0, 0.0));
        for j in 0..around {
            let k = (j + 1) % around;
            if outward_positive {
                triangles.push([centre, ring + j, ring + k]);
            } else {
                triangles.push([centre, ring + k, ring + j]);
            }
        }
    }
    TriangleMesh {
        vertices,
        triangles,
    }
}

/// A closed prism along X from `x0` to `x1` whose section across X is the
/// simple polygon `section` of `(y, z)` points, counter-clockwise seen from
/// +X (OP-20's test parts, decision record 0368): a shaft with a keyway, a
/// dovetail slot, pins beside each other, a blade. The sides are cut into
/// `rings` bands along X (at least one), as a real mesh would be, so a tool
/// dropped on them meets only the triangles nearby. The polygon need not be
/// star-shaped about the axis or contain it; its ends are capped by ear
/// clipping. An exact oracle is the polygon itself.
pub fn prism(section: &[(f64, f64)], x0: f64, x1: f64, rings: u32) -> TriangleMesh {
    let n = section.len();
    let rings = rings.max(1);
    let mut vertices = Vec::with_capacity((rings as usize + 1) * n);
    for k in 0..=rings {
        let x = x0 + (x1 - x0) * f64::from(k) / f64::from(rings);
        vertices.extend(section.iter().map(|&(y, z)| Point3::new(x, y, z)));
    }
    let id = |k: usize| u32::try_from(k).unwrap_or(u32::MAX);
    let mut triangles = Vec::with_capacity(2 * n * rings as usize + 2 * n);
    for k in 0..rings as usize {
        let (lo, hi) = (k * n, (k + 1) * n);
        for i in 0..n {
            let j = (i + 1) % n;
            let (a0, a1, b0, b1) = (id(lo + i), id(lo + j), id(hi + i), id(hi + j));
            triangles.push([a0, a1, b1]);
            triangles.push([a0, b1, b0]);
        }
    }
    let last = rings as usize * n;
    for [a, b, c] in ear_clip(section) {
        triangles.push([id(last + a), id(last + b), id(last + c)]);
        triangles.push([id(a), id(c), id(b)]);
    }
    TriangleMesh {
        vertices,
        triangles,
    }
}

/// Several closed meshes as one (separate pieces of one part).
pub fn join(meshes: &[TriangleMesh]) -> TriangleMesh {
    let mut out = TriangleMesh {
        vertices: Vec::new(),
        triangles: Vec::new(),
    };
    for m in meshes {
        let base = u32::try_from(out.vertices.len()).unwrap_or(u32::MAX);
        out.vertices.extend_from_slice(&m.vertices);
        out.triangles.extend(
            m.triangles
                .iter()
                .map(|t| [t[0] + base, t[1] + base, t[2] + base]),
        );
    }
    out
}

/// The counter-clockwise triangles of a simple counter-clockwise polygon.
fn ear_clip(poly: &[(f64, f64)]) -> Vec<[usize; 3]> {
    let at = |q: usize| poly.get(q).copied().unwrap_or((0.0, 0.0));
    let cross = |o: (f64, f64), a: (f64, f64), b: (f64, f64)| {
        (a.0 - o.0) * (b.1 - o.1) - (a.1 - o.1) * (b.0 - o.0)
    };
    let mut left: Vec<usize> = (0..poly.len()).collect();
    let mut out = Vec::new();
    while left.len() > 3 {
        let m = left.len();
        let corner = |k: usize| {
            let v = |d: usize| left.get((k + d) % m).copied().unwrap_or(0);
            [v(m - 1), v(0), v(1)]
        };
        let ear = (0..m).find(|&k| {
            let [i, j, l] = corner(k);
            let (a, b, c) = (at(i), at(j), at(l));
            cross(a, b, c) > 1e-12
                && left.iter().all(|&q| {
                    [i, j, l].contains(&q)
                        || cross(a, b, at(q)) < 0.0
                        || cross(b, c, at(q)) < 0.0
                        || cross(c, a, at(q)) < 0.0
                })
        });
        let Some(k) = ear else { break };
        out.push(corner(k));
        left.remove(k);
    }
    if let [a, b, c] = left[..] {
        out.push([a, b, c]);
    }
    out
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
