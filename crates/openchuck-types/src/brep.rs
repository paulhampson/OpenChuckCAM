// SPDX-License-Identifier: AGPL-3.0-or-later
//! Boundary-representation data handed from the STEP importer to the libraries (INT-02).
//!
//! Face, edge and vertex IDs are assigned deterministically by the importer,
//! so the same file always gives the same IDs (architecture section 5.4).
//! SP-1 (decision 0003) settled how: the importer numbers them in the
//! OpenCASCADE `TopExp_Explorer` order, which is identical on Linux, macOS and
//! Windows, and treats a file that yields no faces as an error. Kernel
//! tessellation and kernel-computed numbers are not bit-identical across CPU
//! architectures, so no ID is derived from them.
//!
//! Exact faces reach the pathing library as plain data (SP-9, decision 0017
//! item 2): a [`BrepFace`] carries its [`Surface`] (an analytic frame and
//! size, or a clamped, unwrapped rational B-spline [`BSplinePatch`]), its
//! orientation, the UV bounds, the trim loops as UV polylines and the
//! tessellation. `openchuck-path` evaluates that data in pure Rust; no kernel
//! type appears here.
//!
//! # Parametrisation
//!
//! The analytic surfaces use OpenCASCADE's parametrisation, so (u, v) agree
//! with the kernel. With the frame's axes `x`, `y`, `z` (orthonormal) and `radial(u) = cos(u) x + sin(u) y`:
//!
//! | Surface | Point at (u, v) | Size |
//! | --- | --- | --- |
//! | plane | `origin + u x + v y` | none |
//! | cylinder | `origin + r radial(u) + v z` | `radius` |
//! | cone | `origin + (r + v sin a) radial(u) + v cos a z` | `radius`, `semi_angle` |
//! | sphere | `origin + r cos(v) radial(u) + r sin(v) z` | `radius` |
//! | torus | `origin + (R + r cos v) radial(u) + r sin(v) z` | `major`, `minor` |
//!
//! The surface normal is `du x dv`, reversed when [`BrepFace::reversed`] is
//! set, so it points out of the material.

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::geometry::{Point3, Vec3};
use crate::mesh::TriangleMesh;

/// A stable face identifier within one model file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FaceId(pub u32);

/// A stable edge identifier within one model file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EdgeId(pub u32);

/// The kind of surface underlying a face.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SurfaceKind {
    /// A plane.
    Plane,
    /// A circular cylinder.
    Cylinder,
    /// A circular cone.
    Cone,
    /// A sphere.
    Sphere,
    /// A torus.
    Torus,
    /// A B-spline or NURBS surface.
    BSpline,
    /// Any other surface type.
    Other,
}

/// A stable vertex identifier within one model file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct VertexId(pub u32);

/// An orthonormal frame: an origin and three unit axes, exactly as the kernel
/// states them. The frame is usually right-handed, but the kernel allows a
/// mirrored one, so evaluators must use `y` as given and never recompute it
/// from `z` and `x`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Frame {
    /// The origin.
    pub origin: Point3,
    /// The first axis (angle zero of a revolved surface).
    pub x: Vec3,
    /// The second axis (`z` cross `x` for a right-handed frame).
    pub y: Vec3,
    /// The third axis (the axis of a revolved surface, the normal of a plane).
    pub z: Vec3,
}

/// A clamped, unwrapped rational B-spline patch (decision 0017 item 2).
///
/// The kernel's periodic and non-clamped forms are converted by the importer
/// so the knot vectors are clamped (the end knots repeat `degree + 1` times)
/// and the patch is not periodic.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BSplinePatch {
    /// Degree in u.
    pub degree_u: u32,
    /// Degree in v.
    pub degree_v: u32,
    /// Full knot vector in u (repeated knots listed as often as they occur).
    pub knots_u: Vec<f64>,
    /// Full knot vector in v.
    pub knots_v: Vec<f64>,
    /// Number of poles in u.
    pub poles_u: u32,
    /// Number of poles in v.
    pub poles_v: u32,
    /// Poles, `poles_u * poles_v` of them, with v varying fastest:
    /// index `iu * poles_v + iv`.
    pub poles: Vec<Point3>,
    /// One weight per pole, in the same order; all 1 for a non-rational patch.
    pub weights: Vec<f64>,
}

impl BSplinePatch {
    /// Fails unless the knot vectors, pole count and weights agree and
    /// everything is finite and ascending. The check the evaluators rely on.
    pub fn validate(&self) -> Result<()> {
        let bad = |what: String| Error::invalid("b-spline patch", what);
        let (pu, pv) = (self.poles_u as usize, self.poles_v as usize);
        let (du, dv) = (self.degree_u as usize, self.degree_v as usize);
        if du == 0 || dv == 0 || pu <= du || pv <= dv {
            return Err(bad(format!(
                "degrees ({du}, {dv}) need more poles than {pu} by {pv}"
            )));
        }
        if self.knots_u.len() != pu + du + 1 || self.knots_v.len() != pv + dv + 1 {
            return Err(bad("knot vectors do not match the pole counts".to_owned()));
        }
        for knots in [&self.knots_u, &self.knots_v] {
            if knots.iter().any(|k| !k.is_finite()) || !knots.is_sorted() {
                return Err(bad("knots must be finite and ascending".to_owned()));
            }
        }
        if self.poles.len() != pu * pv || self.weights.len() != pu * pv {
            return Err(bad("pole and weight counts do not match".to_owned()));
        }
        if self.poles.iter().any(|p| !p.is_finite())
            || self.weights.iter().any(|w| !w.is_finite() || *w <= 0.0)
        {
            return Err(bad("poles must be finite and weights positive".to_owned()));
        }
        Ok(())
    }
}

/// The surface under a face, with the parameters the evaluators need.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", deny_unknown_fields)]
#[non_exhaustive]
pub enum Surface {
    /// A plane through the frame's origin with normal `z`.
    Plane {
        /// The frame.
        frame: Frame,
    },
    /// A circular cylinder about the frame's `z` axis.
    Cylinder {
        /// The frame.
        frame: Frame,
        /// The radius in millimetres.
        radius: f64,
    },
    /// A circular cone about the frame's `z` axis.
    Cone {
        /// The frame.
        frame: Frame,
        /// The radius at v = 0 in millimetres.
        radius: f64,
        /// The semi-angle in radians, between the axis and the surface line.
        semi_angle: f64,
    },
    /// A sphere centred on the frame's origin.
    Sphere {
        /// The frame.
        frame: Frame,
        /// The radius in millimetres.
        radius: f64,
    },
    /// A torus about the frame's `z` axis.
    Torus {
        /// The frame.
        frame: Frame,
        /// The distance from the axis to the tube's centre circle.
        major: f64,
        /// The tube radius.
        minor: f64,
    },
    /// Any other surface, converted to a clamped rational B-spline patch.
    BSpline(BSplinePatch),
}

impl Surface {
    /// The kind of this surface.
    pub const fn kind(&self) -> SurfaceKind {
        match self {
            Self::Plane { .. } => SurfaceKind::Plane,
            Self::Cylinder { .. } => SurfaceKind::Cylinder,
            Self::Cone { .. } => SurfaceKind::Cone,
            Self::Sphere { .. } => SurfaceKind::Sphere,
            Self::Torus { .. } => SurfaceKind::Torus,
            Self::BSpline(_) => SurfaceKind::BSpline,
        }
    }
}

/// A point in a face's parameter space.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UvPoint {
    /// First surface parameter.
    pub u: f64,
    /// Second surface parameter.
    pub v: f64,
}

/// One closed trim loop of a face: a polyline in (u, v), sampled from the
/// edges' parameter curves. The last point joins the first.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TrimLoop(pub Vec<UvPoint>);

/// One B-rep face.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrepFace {
    /// Stable identifier.
    pub id: FaceId,
    /// Underlying surface kind as the file states it. [`BrepFace::geometry`]
    /// holds the evaluable form, so a revolved or extruded surface reads
    /// `Other` here and `BSpline` there.
    pub surface: SurfaceKind,
    /// The solid (body) the face belongs to, counting from 0 in file order.
    #[serde(default)]
    pub body: u32,
    /// Bounding edges.
    pub edges: Vec<EdgeId>,
    /// The evaluable surface (decision 0017 item 2).
    pub geometry: Surface,
    /// True when the face's outward side is opposite `du x dv`.
    pub reversed: bool,
    /// The parameter box `[u_min, u_max, v_min, v_max]` of the face.
    pub uv_bounds: [f64; 4],
    /// The trim loops in (u, v): the outer boundary and any holes, as
    /// polylines. Even-odd classification gives the trim test.
    pub trims: Vec<TrimLoop>,
    /// Tessellation at the model's tolerance (IMP-06), wound counter-clockwise
    /// seen from outside the material, for display and mesh checks.
    pub tessellation: TriangleMesh,
}

/// One B-rep edge.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrepEdge {
    /// Stable identifier.
    pub id: EdgeId,
    /// Faces that share this edge.
    pub faces: Vec<FaceId>,
    /// The end vertices: first and last (equal for a closed edge); `None`
    /// for an edge without vertices.
    #[serde(default)]
    pub vertices: Option<[VertexId; 2]>,
    /// The edge sampled as a polyline at the model's tolerance.
    pub polyline: Vec<Point3>,
}

/// One B-rep vertex.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrepVertex {
    /// Stable identifier.
    pub id: VertexId,
    /// Its position in millimetres.
    pub point: Point3,
}

/// The tessellation settings a B-rep was meshed with (IMP-06).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TessellationSettings {
    /// Linear deflection in millimetres. The kernel's tolerance is not a
    /// bound on deviation (decision 0003): measure it.
    pub linear: f64,
    /// Angular deflection in radians.
    pub angular: f64,
}

impl Default for TessellationSettings {
    /// 0.01 mm and 0.1 rad, the finest setting SP-9 measured.
    fn default() -> Self {
        Self {
            linear: 0.01,
            angular: 0.1,
        }
    }
}

/// A B-rep model: faces, edges and vertices with stable IDs.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrepModel {
    /// Faces, ordered by ID.
    pub faces: Vec<BrepFace>,
    /// Edges, ordered by ID.
    pub edges: Vec<BrepEdge>,
    /// Vertices, ordered by ID.
    #[serde(default)]
    pub vertices: Vec<BrepVertex>,
    /// How the faces were tessellated.
    #[serde(default)]
    pub tessellation: TessellationSettings,
}

impl Frame {
    fn validate(&self, what: &str) -> Result<()> {
        let axes = [self.x, self.y, self.z];
        if !self.origin.is_finite() || axes.iter().any(|a| !a.is_finite()) {
            return Err(Error::invalid(what, "the frame is not finite"));
        }
        if axes.iter().any(|a| (a.length() - 1.0).abs() > 1e-6) {
            return Err(Error::invalid(what, "the frame axes are not unit length"));
        }
        let dots = [self.x.dot(self.y), self.y.dot(self.z), self.z.dot(self.x)];
        if dots.iter().any(|d| d.abs() > 1e-6) {
            return Err(Error::invalid(what, "the frame axes are not perpendicular"));
        }
        Ok(())
    }
}

impl Surface {
    /// Fails unless the frame and sizes are finite and usable.
    pub fn validate(&self, what: &str) -> Result<()> {
        let size = |name: &str, v: f64| {
            if v.is_finite() && v > 0.0 {
                Ok(())
            } else {
                Err(Error::invalid(
                    what,
                    format!("{name} must be positive and finite"),
                ))
            }
        };
        match self {
            Self::Plane { frame } => frame.validate(what),
            Self::Cylinder { frame, radius } | Self::Sphere { frame, radius } => {
                frame.validate(what)?;
                size("radius", *radius)
            }
            Self::Cone {
                frame,
                radius,
                semi_angle,
            } => {
                frame.validate(what)?;
                if !radius.is_finite() || *radius < 0.0 {
                    return Err(Error::invalid(
                        what,
                        "radius must be finite and not negative",
                    ));
                }
                if !semi_angle.is_finite() || semi_angle.abs() >= std::f64::consts::FRAC_PI_2 {
                    return Err(Error::invalid(
                        what,
                        "semi-angle must be inside (-90, 90) degrees",
                    ));
                }
                Ok(())
            }
            Self::Torus {
                frame,
                major,
                minor,
            } => {
                frame.validate(what)?;
                size("major radius", *major)?;
                size("minor radius", *minor)
            }
            Self::BSpline(patch) => patch.validate(),
        }
    }
}

impl BrepModel {
    /// Fails unless the model is consistent: IDs equal positions, every
    /// reference is in range, and all geometry is finite. Importers call it
    /// on what the kernel returns, so a damaged result is an error, not a
    /// panic downstream (NFR-07).
    pub fn validate(&self) -> Result<()> {
        let bad = |what: String, why: String| Error::invalid(what, why);
        for (i, v) in self.vertices.iter().enumerate() {
            if v.id.0 as usize != i {
                return Err(bad(
                    format!("vertices[{i}]"),
                    "ID is not its position".to_owned(),
                ));
            }
            v.point.validate(&format!("vertices[{i}]"))?;
        }
        for (i, e) in self.edges.iter().enumerate() {
            let what = format!("edges[{i}]");
            if e.id.0 as usize != i {
                return Err(bad(what, "ID is not its position".to_owned()));
            }
            if e.faces.iter().any(|f| f.0 as usize >= self.faces.len()) {
                return Err(bad(what, "refers to a missing face".to_owned()));
            }
            if let Some([a, b]) = e.vertices
                && (a.0 as usize >= self.vertices.len() || b.0 as usize >= self.vertices.len())
            {
                return Err(bad(what, "refers to a missing vertex".to_owned()));
            }
            for p in &e.polyline {
                p.validate(&what)?;
            }
        }
        for (i, f) in self.faces.iter().enumerate() {
            let what = format!("faces[{i}]");
            if f.id.0 as usize != i {
                return Err(bad(what, "ID is not its position".to_owned()));
            }
            if f.edges.iter().any(|e| e.0 as usize >= self.edges.len()) {
                return Err(bad(what, "refers to a missing edge".to_owned()));
            }
            f.geometry.validate(&what)?;
            if f.uv_bounds.iter().any(|b| !b.is_finite())
                || f.uv_bounds[0] > f.uv_bounds[1]
                || f.uv_bounds[2] > f.uv_bounds[3]
            {
                return Err(bad(
                    what,
                    "the UV bounds are not a finite, ordered box".to_owned(),
                ));
            }
            if f.trims
                .iter()
                .flat_map(|t| &t.0)
                .any(|p| !p.u.is_finite() || !p.v.is_finite())
            {
                return Err(bad(what, "a trim loop point is not finite".to_owned()));
            }
            f.tessellation.validate()?;
        }
        Ok(())
    }

    /// Every face's tessellation joined into one mesh, with the face of each
    /// triangle in `face_of_triangle` (IMP-06: face IDs mapped to triangles).
    /// Vertices are not shared between faces, so a face's triangles keep
    /// their own corners.
    pub fn joined_tessellation(&self) -> (TriangleMesh, Vec<FaceId>) {
        let mut mesh = TriangleMesh::default();
        let mut face_of_triangle = Vec::new();
        for face in &self.faces {
            let base = u32::try_from(mesh.vertices.len()).unwrap_or(u32::MAX);
            mesh.vertices.extend_from_slice(&face.tessellation.vertices);
            for t in &face.tessellation.triangles {
                mesh.triangles.push(t.map(|i| i.saturating_add(base)));
                face_of_triangle.push(face.id);
            }
        }
        (mesh, face_of_triangle)
    }
}
