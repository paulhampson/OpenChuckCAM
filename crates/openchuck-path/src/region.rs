// SPDX-License-Identifier: AGPL-3.0-or-later
//! Where an operation may cut: selected regions, a containment boundary and
//! avoid and check surfaces (OP-02; work breakdown slice 10 item 5).
//!
//! A [`RegionConstraint`] reaches a strategy through
//! [`StrategyInput::region`](crate::StrategyInput::region) as plain data and
//! ports, so the library knows nothing about selection sets. The host maps
//! each set to the triangles it names and builds a [`MeshSurface`] of them.
//! The strategy still drops its tool on the **whole** part, so a cut next to
//! the region never gouges the rest; the constraint only decides where a cut
//! is made.
//!
//! A cut point at tool tip `tip` with cutter contact `contact` is allowed when
//!
//! - the contact lies on the selected region, if there is one;
//! - the contact lies inside the containment boundary, if there is one;
//! - the contact does not lie on an avoid surface; and
//! - the tool, dropped on the check surfaces alone, does not come to rest at
//!   the same tip, that is the tool is not touching a check surface.
//!
//! A tool that is not allowed to cut leaves a gap in the pass; the shared
//! services link the pieces (OP-14). Nothing is dropped silently: a
//! constraint that leaves no cut at all is an error of the strategy.
//!
//! **Boundary.** A [`Window`] is the bounding window of a set of triangles in
//! the development of the rotary axis: an interval of X and the smallest arc
//! of angle about X that holds the vertices. It is the first form of the
//! containment boundary; a boundary drawn as a curve on the surface is still open (OI-137).
//! Where a pass can reach the selected region is found through the region's
//! offset ([`crate::candidates`], slice 12).

use openchuck_types::geometry::Point3;
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::tool::profile::RevolvedProfile;

use crate::cutter::ToolLine;
use crate::error::{PathError, Result};
use crate::surface::SurfaceSource;

/// How close, in millimetres, a point must be to a surface to count as on it.
pub const ON_SURFACE: f64 = 1e-4;

/// A window of the development about the rotary axis (model X).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Window {
    /// The lowest X.
    pub x_min: f64,
    /// The highest X.
    pub x_max: f64,
    /// The arc of angle about X, `atan2(z, y)` in radians, as its start and
    /// its extent (0 to a full turn); `None` when the vertices go all the way
    /// round, so angle does not limit the window.
    pub arc: Option<(f64, f64)>,
}

impl Window {
    /// The window of the vertices that triangles of `mesh` use. Fails for a
    /// mesh with no triangles.
    pub fn of_mesh(mesh: &TriangleMesh) -> Result<Self> {
        let mut x = (f64::INFINITY, f64::NEG_INFINITY);
        let mut angles = Vec::new();
        for &index in mesh.triangles.iter().flatten() {
            let p = mesh
                .vertices
                .get(index as usize)
                .ok_or_else(|| PathError::Surface("a boundary vertex is missing".to_owned()))?;
            x = (x.0.min(p.x), x.1.max(p.x));
            // On the axis the angle means nothing.
            if p.y.hypot(p.z) > ON_SURFACE {
                angles.push(p.z.atan2(p.y));
            }
        }
        if x.0 > x.1 {
            return Err(PathError::Surface(
                "the containment boundary has no triangles".to_owned(),
            ));
        }
        Ok(Self {
            x_min: x.0,
            x_max: x.1,
            arc: smallest_arc(angles),
        })
    }

    /// Whether `p` is inside the window, to within [`ON_SURFACE`] in X and
    /// the same distance at the point's radius in angle.
    pub fn contains(&self, p: Point3) -> bool {
        if p.x < self.x_min - ON_SURFACE || p.x > self.x_max + ON_SURFACE {
            return false;
        }
        let Some((start, extent)) = self.arc else {
            return true;
        };
        let radius = p.y.hypot(p.z);
        if radius <= ON_SURFACE {
            return true;
        }
        let slack = ON_SURFACE / radius;
        let offset = (p.z.atan2(p.y) - start).rem_euclid(std::f64::consts::TAU);
        offset <= extent + slack || offset >= std::f64::consts::TAU - slack
    }
}

/// The smallest arc holding every angle, as (start, extent). `None` when the
/// angles go all the way round: at least eight distinct ones whose largest gap
/// is no more than four times their typical (median) gap, so a ring of
/// vertices is a full turn however fine its mesh, and a patch is not.
fn smallest_arc(mut angles: Vec<f64>) -> Option<(f64, f64)> {
    angles.sort_by(f64::total_cmp);
    angles.dedup_by(|b, a| *b - *a < 1e-9);
    let (first, last) = (*angles.first()?, *angles.last()?);
    // Gaps between neighbours round the circle; gap `i` follows angle `i`.
    let mut gaps: Vec<f64> = angles
        .iter()
        .zip(angles.iter().skip(1))
        .map(|(a, b)| b - a)
        .collect();
    gaps.push(first + std::f64::consts::TAU - last);
    let mut order = gaps.clone();
    order.sort_by(f64::total_cmp);
    let median = order.get(order.len() / 2).copied().unwrap_or(0.0);
    let (widest, widest_gap) =
        gaps.iter().enumerate().fold(
            (0, f64::MIN),
            |best, (i, &g)| if g > best.1 { (i, g) } else { best },
        );
    if angles.len() >= 8 && widest_gap <= 4.0 * median {
        return None;
    }
    // The arc starts after the widest gap.
    let start = angles.get(widest + 1).copied().unwrap_or(first);
    Some((start, std::f64::consts::TAU - widest_gap))
}

/// The limits on where an operation cuts. Every part is optional; with none,
/// the whole part is machined.
#[derive(Clone, Copy, Default)]
pub struct RegionConstraint<'a> {
    /// The selected region. Without one, the whole part.
    pub region: Option<&'a dyn SurfaceSource>,
    /// The containment boundary.
    pub boundary: Option<Window>,
    /// Surfaces that are never cut.
    pub avoid: Option<&'a dyn SurfaceSource>,
    /// Surfaces the tool must not touch.
    pub check: Option<&'a dyn SurfaceSource>,
}

impl std::fmt::Debug for RegionConstraint<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegionConstraint")
            .field("region", &self.region.is_some())
            .field("boundary", &self.boundary)
            .field("avoid", &self.avoid.is_some())
            .field("check", &self.check.is_some())
            .finish()
    }
}

impl RegionConstraint<'_> {
    /// True when nothing limits the cut.
    pub fn is_empty(&self) -> bool {
        self.region.is_none()
            && self.boundary.is_none()
            && self.avoid.is_none()
            && self.check.is_none()
    }

    /// Whether the tool may cut with its tip at `tip` and its contact at
    /// `contact`, having been dropped along `line` with `profile`.
    pub fn allows(
        &self,
        contact: Point3,
        tip: Point3,
        profile: &RevolvedProfile,
        line: &ToolLine,
    ) -> bool {
        let on = |surface: &dyn SurfaceSource| {
            surface.distance(contact).is_some_and(|d| d <= ON_SURFACE)
        };
        if self.region.is_some_and(|r| !on(r)) {
            return false;
        }
        if self.boundary.is_some_and(|b| !b.contains(contact)) {
            return false;
        }
        if self.avoid.is_some_and(on) {
            return false;
        }
        if let Some(check) = self.check
            && let Some(touch) = check.drop_tool(profile, line)
            && touch.tip.distance(tip) <= ON_SURFACE
        {
            return false;
        }
        true
    }
}
