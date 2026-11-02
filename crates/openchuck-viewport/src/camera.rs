// SPDX-License-Identifier: AGPL-3.0-or-later
//! The viewport camera: orbit, pan, zoom, standard views, perspective and
//! orthographic projection (UI-02).
//!
//! The camera looks at a target from a distance, in a direction given by a
//! yaw about model +Z and a pitch above the XY plane. Model Z is up on
//! screen except in the top and bottom views. Computation is in `f64`
//! (NFR-05); matrices are handed to the GPU as `f32`.
//!
//! Matrices are column-major (`m[column][row]`), right-handed, with clip
//! depth from 0 to 1 as wgpu expects.

/// A 4 by 4 matrix, column-major.
pub type Mat4 = [[f64; 4]; 4];

/// How the scene is projected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Projection {
    /// Perspective with a fixed vertical field of view.
    Perspective,
    /// Orthographic: parallel lines stay parallel, sizes do not change with depth.
    Orthographic,
}

/// The standard views (UI-02). Front looks along +Y, so model X runs to the
/// right and Z up; right looks along -X; top looks down -Z with Y up on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StandardView {
    /// From -Y.
    Front,
    /// From +Y.
    Back,
    /// From -X.
    Left,
    /// From +X.
    Right,
    /// From +Z.
    Top,
    /// From -Z.
    Bottom,
    /// From +X, -Y and +Z, equally.
    Isometric,
}

impl StandardView {
    /// Every standard view, in menu order.
    pub const ALL: [Self; 7] = [
        Self::Front,
        Self::Back,
        Self::Left,
        Self::Right,
        Self::Top,
        Self::Bottom,
        Self::Isometric,
    ];

    /// The view's name for menus.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Front => "Front",
            Self::Back => "Back",
            Self::Left => "Left",
            Self::Right => "Right",
            Self::Top => "Top",
            Self::Bottom => "Bottom",
            Self::Isometric => "Isometric",
        }
    }

    /// Yaw and pitch in degrees.
    const fn angles(self) -> (f64, f64) {
        match self {
            Self::Front => (-90.0, 0.0),
            Self::Back => (90.0, 0.0),
            Self::Left => (180.0, 0.0),
            Self::Right => (0.0, 0.0),
            Self::Top => (-90.0, 90.0),
            Self::Bottom => (-90.0, -90.0),
            // atan(1 / sqrt 2): the direction (1, -1, 1).
            Self::Isometric => (-45.0, 35.264_389_682_754_654),
        }
    }
}

/// Vertical field of view of the perspective camera, in degrees.
pub const FIELD_OF_VIEW_DEGREES: f64 = 35.0;

/// The camera.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    /// The point looked at, in the model frame.
    pub target: [f64; 3],
    /// Distance from the target to the eye, in millimetres.
    pub distance: f64,
    /// Angle about model +Z, degrees; 0 puts the eye on +X of the target.
    pub yaw: f64,
    /// Angle above the XY plane, degrees, from -90 to 90.
    pub pitch: f64,
    /// Perspective or orthographic.
    pub projection: Projection,
    /// Radius of the scene around the target, for the depth range.
    pub scene_radius: f64,
}

impl Default for Camera {
    fn default() -> Self {
        let mut camera = Self {
            target: [0.0; 3],
            distance: 100.0,
            yaw: 0.0,
            pitch: 0.0,
            projection: Projection::Perspective,
            scene_radius: 50.0,
        };
        camera.set_view(StandardView::Isometric);
        camera
    }
}

/// Closest the eye may come to the target, in millimetres.
const MIN_DISTANCE: f64 = 0.01;
/// Furthest the eye may go from the target, in millimetres.
const MAX_DISTANCE: f64 = 1.0e6;

impl Camera {
    /// Turns to a standard view, keeping target and distance.
    pub fn set_view(&mut self, view: StandardView) {
        let (yaw, pitch) = view.angles();
        self.yaw = yaw;
        self.pitch = pitch;
    }

    /// Centres the view on a box and sets the distance so the whole box fits
    /// in a viewport of `aspect` (width over height). A box that is not
    /// finite is ignored.
    pub fn fit(&mut self, min: [f64; 3], max: [f64; 3], aspect: f64) {
        if !min.iter().chain(&max).all(|v| v.is_finite()) {
            return;
        }
        let centre = [
            0.5 * (min[0] + max[0]),
            0.5 * (min[1] + max[1]),
            0.5 * (min[2] + max[2]),
        ];
        let radius = (0.5 * length(sub(max, min))).max(1.0);
        self.target = centre;
        self.scene_radius = radius;
        // The sphere around the box fits the narrower of the two half-angles.
        let half = (0.5 * FIELD_OF_VIEW_DEGREES).to_radians();
        let half_narrow = if aspect.is_finite() && aspect > 0.0 && aspect < 1.0 {
            (half.tan() * aspect).atan()
        } else {
            half
        };
        self.distance = (1.1 * radius / half_narrow.sin()).clamp(MIN_DISTANCE, MAX_DISTANCE);
    }

    /// Turns the camera about the target: `dx` and `dy` in degrees of yaw and
    /// pitch.
    pub fn orbit(&mut self, dx: f64, dy: f64) {
        if dx.is_finite() && dy.is_finite() {
            self.yaw = (self.yaw - dx).rem_euclid(360.0);
            self.pitch = (self.pitch + dy).clamp(-90.0, 90.0);
        }
    }

    /// Moves the target across the screen by `dx` and `dy` pixels (right and
    /// down) in a viewport `height` pixels tall, so the model follows the
    /// pointer.
    pub fn pan(&mut self, dx: f64, dy: f64, height: f64) {
        if !(dx.is_finite() && dy.is_finite() && height.is_finite() && height > 0.0) {
            return;
        }
        let per_pixel = self.visible_height() / height;
        let (right, up, _) = self.basis();
        let delta = add(scale(right, -dx * per_pixel), scale(up, dy * per_pixel));
        self.target = add(self.target, delta);
    }

    /// Moves toward the target by `factor` (below 1 zooms in).
    pub fn zoom(&mut self, factor: f64) {
        if factor.is_finite() && factor > 0.0 {
            self.distance = (self.distance * factor).clamp(MIN_DISTANCE, MAX_DISTANCE);
        }
    }

    /// The eye position.
    pub fn eye(&self) -> [f64; 3] {
        let (_, _, back) = self.basis();
        add(self.target, scale(back, self.distance))
    }

    /// The camera's right, up and backward unit vectors (backward points
    /// from the target to the eye). Built from yaw and pitch, so the top and
    /// bottom views are well defined.
    pub fn basis(&self) -> ([f64; 3], [f64; 3], [f64; 3]) {
        let (yaw, pitch) = (self.yaw.to_radians(), self.pitch.to_radians());
        let back = [
            pitch.cos() * yaw.cos(),
            pitch.cos() * yaw.sin(),
            pitch.sin(),
        ];
        let right = [-yaw.sin(), yaw.cos(), 0.0];
        let up = cross(back, right);
        (right, up, back)
    }

    /// Height of the view at the target, in millimetres. The orthographic
    /// view shows the same height as the perspective one at the target, so
    /// switching keeps the model's size on screen.
    pub fn visible_height(&self) -> f64 {
        2.0 * self.distance * (0.5 * FIELD_OF_VIEW_DEGREES).to_radians().tan()
    }

    /// Near and far clip distances, around the scene sphere.
    fn depth_range(&self) -> (f64, f64) {
        let reach = 2.0 * self.scene_radius.max(1.0);
        let far = self.distance + reach;
        let near = match self.projection {
            Projection::Perspective => (self.distance - reach).max(far * 1.0e-4),
            Projection::Orthographic => self.distance - reach,
        };
        (near, far)
    }

    /// The world-to-view matrix.
    pub fn view_matrix(&self) -> Mat4 {
        let (r, u, b) = self.basis();
        let e = self.eye();
        [
            [r[0], u[0], b[0], 0.0],
            [r[1], u[1], b[1], 0.0],
            [r[2], u[2], b[2], 0.0],
            [-dot(r, e), -dot(u, e), -dot(b, e), 1.0],
        ]
    }

    /// The view-to-clip matrix for a viewport of `aspect` (width over height).
    pub fn projection_matrix(&self, aspect: f64) -> Mat4 {
        let aspect = if aspect.is_finite() && aspect > 0.0 {
            aspect
        } else {
            1.0
        };
        let (near, far) = self.depth_range();
        match self.projection {
            Projection::Perspective => {
                let f = 1.0 / (0.5 * FIELD_OF_VIEW_DEGREES).to_radians().tan();
                [
                    [f / aspect, 0.0, 0.0, 0.0],
                    [0.0, f, 0.0, 0.0],
                    [0.0, 0.0, far / (near - far), -1.0],
                    [0.0, 0.0, near * far / (near - far), 0.0],
                ]
            }
            Projection::Orthographic => {
                let h = self.visible_height();
                let w = h * aspect;
                [
                    [2.0 / w, 0.0, 0.0, 0.0],
                    [0.0, 2.0 / h, 0.0, 0.0],
                    [0.0, 0.0, 1.0 / (near - far), 0.0],
                    [0.0, 0.0, near / (near - far), 1.0],
                ]
            }
        }
    }

    /// The world-to-clip matrix.
    pub fn view_projection(&self, aspect: f64) -> Mat4 {
        multiply(&self.projection_matrix(aspect), &self.view_matrix())
    }

    /// Where a model point lands in normalised device coordinates (x right
    /// and y up, from -1 to 1; z from 0 near to 1 far), or `None` behind the
    /// eye.
    pub fn project(&self, point: [f64; 3], aspect: f64) -> Option<[f64; 3]> {
        let m = self.view_projection(aspect);
        let c = transform(&m, [point[0], point[1], point[2], 1.0]);
        (c[3] > 0.0).then(|| [c[0] / c[3], c[1] / c[3], c[2] / c[3]])
    }

    /// The ray through a point in normalised device coordinates, as an origin
    /// and a unit direction in the model frame (for picking, slice 10).
    pub fn ray(&self, ndc: [f64; 2], aspect: f64) -> ([f64; 3], [f64; 3]) {
        let (r, u, b) = self.basis();
        let aspect = if aspect.is_finite() && aspect > 0.0 {
            aspect
        } else {
            1.0
        };
        let half_h = 0.5 * self.visible_height();
        let half_w = half_h * aspect;
        let offset = add(scale(r, ndc[0] * half_w), scale(u, ndc[1] * half_h));
        match self.projection {
            Projection::Perspective => {
                let eye = self.eye();
                let at = add(self.target, offset);
                (eye, normalise(sub(at, eye)))
            }
            Projection::Orthographic => {
                let origin = add(self.eye(), offset);
                (origin, scale(b, -1.0))
            }
        }
    }
}

/// The identity matrix.
pub const IDENTITY: Mat4 = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

/// The right-handed rotation by `radians` about the X axis.
pub fn rotation_x(radians: f64) -> Mat4 {
    let (s, c) = radians.sin_cos();
    [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, c, s, 0.0],
        [0.0, -s, c, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

/// The rigid transform that puts an object built with its origin at the
/// origin and its axis along +Z at `origin`, with its axis along `axis`
/// (a vector that need not be a unit vector; zero keeps +Z). Turns about the
/// axis are free: the rotation is the shortest one from +Z.
pub fn place(origin: [f64; 3], axis: [f64; 3]) -> Mat4 {
    let z = normalise(axis);
    let z = if length(z) > 0.0 { z } else { [0.0, 0.0, 1.0] };
    // A vector not parallel to z to build the other two axes from.
    let helper = if z[0].abs() < 0.9 {
        [1.0, 0.0, 0.0]
    } else {
        [0.0, 1.0, 0.0]
    };
    let x = normalise(cross(helper, z));
    let y = cross(z, x);
    [
        [x[0], x[1], x[2], 0.0],
        [y[0], y[1], y[2], 0.0],
        [z[0], z[1], z[2], 0.0],
        [origin[0], origin[1], origin[2], 1.0],
    ]
}

/// The inverse of a rigid transform (a rotation and a translation): the
/// rotation's transpose, and the translation turned back.
pub fn rigid_inverse(m: &Mat4) -> Mat4 {
    let [c0, c1, c2, c3] = *m;
    let dot = |c: [f64; 4]| c[0] * c3[0] + c[1] * c3[1] + c[2] * c3[2];
    [
        [c0[0], c1[0], c2[0], 0.0],
        [c0[1], c1[1], c2[1], 0.0],
        [c0[2], c1[2], c2[2], 0.0],
        [-dot(c0), -dot(c1), -dot(c2), 1.0],
    ]
}

/// `a * b`.
pub fn multiply(a: &Mat4, b: &Mat4) -> Mat4 {
    let mut out = [[0.0; 4]; 4];
    for (column, b_column) in out.iter_mut().zip(b) {
        *column = transform(a, *b_column);
    }
    out
}

/// `m * v`.
pub fn transform(m: &Mat4, v: [f64; 4]) -> [f64; 4] {
    let mut out = [0.0; 4];
    for (column, factor) in m.iter().zip(v) {
        for (o, c) in out.iter_mut().zip(column) {
            *o += c * factor;
        }
    }
    out
}

/// The matrix as `f32` columns for the GPU.
pub fn to_f32(m: &Mat4) -> [[f32; 4]; 4] {
    m.map(|column| column.map(|v| v as f32))
}

fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn scale(a: [f64; 3], s: f64) -> [f64; 3] {
    [a[0] * s, a[1] * s, a[2] * s]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn length(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}

fn normalise(a: [f64; 3]) -> [f64; 3] {
    let l = length(a);
    if l > 0.0 { scale(a, 1.0 / l) } else { a }
}
