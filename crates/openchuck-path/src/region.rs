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
use openchuck_types::job::JobContext;
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
    /// How the boundary limits the cut, for a strategy that offers a choice
    /// ([`Strategy::honours_restriction_options`](crate::strategy::Strategy::honours_restriction_options);
    /// swarf, decision record 0176). Both on by default.
    pub boundary_limits: BoundaryLimits,
    /// Avoid and check surfaces the tool is measured against but not kept
    /// clear of: where it breaks one, the path is kept and a warning names
    /// the surface (decision record 0176). Only a strategy that honours
    /// restriction options takes them.
    pub watch: &'a [WatchedSurface<'a>],
}

/// How a boundary limits a strategy that offers a choice (decision record
/// 0176): swarf limits the pass across the rulings and the cut length at the
/// tool tip. With both off, the boundary limits nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundaryLimits {
    /// The boundary limits the pass (swarf's limit A).
    pub pass: bool,
    /// The boundary limits the cut at the tool tip (swarf's limit B).
    pub tip: bool,
}

impl Default for BoundaryLimits {
    fn default() -> Self {
        Self {
            pass: true,
            tip: true,
        }
    }
}

/// Whether a surface is one the tool must never cut into (avoid) or must
/// keep its stock to leave clear of (check), OP-02.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protection {
    /// Never cut into, beyond the tolerance.
    Avoid,
    /// Kept the stock to leave clear of, less the tolerance.
    Check,
}

/// An avoid or check surface that is only measured (decision record 0176):
/// the strategy warns, naming it, where the tool would break it.
#[derive(Clone, Copy)]
pub struct WatchedSurface<'a> {
    /// The name the warning gives it, such as the selection set's.
    pub name: &'a str,
    /// Whether it is an avoid or a check surface.
    pub protection: Protection,
    /// The surface.
    pub surface: &'a dyn SurfaceSource,
}

impl std::fmt::Debug for WatchedSurface<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WatchedSurface")
            .field("name", &self.name)
            .field("protection", &self.protection)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for RegionConstraint<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegionConstraint")
            .field("region", &self.region.is_some())
            .field("boundary", &self.boundary)
            .field("avoid", &self.avoid.is_some())
            .field("check", &self.check.is_some())
            .field("boundary_limits", &self.boundary_limits)
            .field("watch", &self.watch)
            .finish()
    }
}

impl RegionConstraint<'_> {
    /// True when the constraint sets a restriction option: a boundary limit
    /// turned off, or a watched surface (decision record 0176). A boundary
    /// limit needs a strategy that honours restriction options (swarf);
    /// watched surfaces are measured by the shared services for the other
    /// strategies that honour a region (decision record 0229).
    pub fn has_restriction_options(&self) -> bool {
        self.boundary_limits != BoundaryLimits::default() || !self.watch.is_empty()
    }

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
        // A contact found on exact faces lies on them, but its tip only
        // within the drop's accuracy of the highest (decision 0277, OI-833).
        let on = |surface: &dyn SurfaceSource| {
            surface.within(contact, ON_SURFACE + surface.drop_accuracy())
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
            && touch.tip.distance(tip) <= ON_SURFACE + 2.0 * check.drop_accuracy()
        {
            return false;
        }
        true
    }
}

/// Most samples along one move between two cut points when it is measured
/// against the watched surfaces (NFR-07): at the spacing below, a move of
/// several metres.
const MAX_WATCH_SAMPLES: usize = 100_000;

/// Pieces a move is first cut into to estimate its length, since a move that
/// turns the part is an arc on it, not the chord between its ends.
const WATCH_ESTIMATE_PIECES: usize = 8;

/// The warnings for the watched surfaces of a strategy that does not measure
/// them itself (decision record 0229 item 1): roughing, helical, axial,
/// indexed and simultaneous finishing.
///
/// The passes were made as if the watched surfaces were not there. Each
/// move between two cut points of a pass is followed as the machine makes
/// it (straight in the machine's axes), with a nominal sample spacing of
/// an eighth of the tool's cutting radius. The length estimate, minimum
/// spacing and sample cap below make this an approximation: a narrow set
/// can be missed between samples (OI-970). At each sample the
/// tool, grown by the operation's stock to leave, is dropped along its axis
/// onto each watched surface alone. It breaks the surface when
///
/// - it comes to rest there at its tip or above it: the tool where it is
///   touches or reaches into the surface; or
/// - the tool rides on the part (it rests on the part within the toolpath
///   tolerance, `slack`, of where it is) and the surface is where it rests:
///   the test Cut short makes at a cut point (a contact on an avoid surface,
///   or a tool touching a check surface, [`RegionConstraint::allows`]), made
///   between cut points too, where a straight move lies up to the tolerance
///   off the part.
///
/// Roughing's upper layers ride on no part, so they break only a surface
/// they reach into.
///
/// A piece of a move between two samples counts whole when both break the
/// surface and half when one does. One warning names each surface broken,
/// with the length, in the order of `watch`.
pub(crate) fn watched_warnings(
    passes: &[crate::strategy::Pass],
    input: &crate::strategy::StrategyInput<'_>,
    watch: &[WatchedSurface<'_>],
    leave: f64,
    slack: f64,
    context: &JobContext<'_>,
) -> Result<Vec<crate::assemble::PathWarning>> {
    if watch.is_empty() {
        return Ok(Vec::new());
    }
    let grown = crate::cutter::grow_profile(input.tool, leave)?;
    let spacing = (input.tool.cutting_radius() / 8.0).max(ON_SURFACE * 100.0);
    let moves: Vec<_> = passes
        .iter()
        .flat_map(|pass| {
            pass.points
                .windows(2)
                .enumerate()
                .filter(|(i, _)| !pass.rapids.contains(&(i + 1)))
                .filter_map(|(_, pair)| Some((*pair.first()?, *pair.get(1)?)))
        })
        .collect();
    let rotary = input.rotary;
    let measured = crate::parallel::map_in_order(&moves, |(start, end)| -> Result<Vec<f64>> {
        context.check_cancelled()?;
        let a0 = start.a;
        let a1 = crate::rotary::nearest_turn(end.a, a0);
        let (p0, p1) = (rotary.to_axis(start.tip, a0), rotary.to_axis(end.tip, a1));
        let at = |t: f64| {
            let a = openchuck_types::units::Degrees(a0.0 + (a1.0 - a0.0) * t);
            let position = crate::rotary::AxisPosition {
                along: p0.along + (p1.along - p0.along) * t,
                across: p0.across + (p1.across - p0.across) * t,
                height: p0.height + (p1.height - p0.height) * t,
            };
            (rotary.to_model(position, a), rotary.tool_axis(a))
        };
        let estimate: f64 = (0..WATCH_ESTIMATE_PIECES)
            .map(|k| {
                let t = |k: usize| k as f64 / WATCH_ESTIMATE_PIECES as f64;
                at(t(k)).0.distance(at(t(k + 1)).0)
            })
            .sum();
        let samples = ((estimate / spacing).ceil() as usize).clamp(1, MAX_WATCH_SAMPLES);
        let poses: Vec<_> = (0..=samples)
            .map(|k| at(k as f64 / samples as f64))
            .collect();
        // Each pose's line from the grown tool's tip, and where the grown
        // tool rests on the part along it when the tool rides on the part.
        let lines: Vec<(ToolLine, Option<f64>)> = poses
            .iter()
            .map(|(tip, axis)| {
                context.check_cancelled()?;
                let line = ToolLine {
                    origin: *tip + axis.get() * -leave,
                    axis: *axis,
                };
                let rest = input
                    .surface
                    .drop_tool(&grown, &line)
                    .map(|c| c.offset)
                    .filter(|offset| {
                        offset.abs() <= slack + ON_SURFACE + input.surface.drop_accuracy()
                    });
                Ok((line, rest))
            })
            .collect::<Result<_>>()?;
        let broken: Vec<Vec<bool>> = watch
            .iter()
            .map(|w| {
                lines
                    .iter()
                    .map(|(line, rest)| {
                        context.check_cancelled()?;
                        // Within the drops' accuracy, as at a cut point.
                        let tie = ON_SURFACE + 2.0 * w.surface.drop_accuracy();
                        Ok(w.surface.drop_tool(&grown, line).is_some_and(|touch| {
                            touch.offset >= -tie || rest.is_some_and(|r| touch.offset >= r - tie)
                        }))
                    })
                    .collect::<Result<Vec<_>>>()
            })
            .collect::<Result<_>>()?;
        Ok(broken
            .iter()
            .map(|flags| {
                poses
                    .windows(2)
                    .zip(flags.windows(2))
                    .map(|(pair, hit)| {
                        let weight = hit.iter().filter(|&&b| b).count() as f64 / 2.0;
                        match (pair.first(), pair.get(1)) {
                            (Some(a), Some(b)) if weight > 0.0 => weight * a.0.distance(b.0),
                            _ => 0.0,
                        }
                    })
                    .sum::<f64>()
            })
            .collect::<Vec<f64>>())
    });
    let mut lengths = vec![0.0f64; watch.len()];
    for per_move in measured {
        for (total, length) in lengths.iter_mut().zip(per_move?) {
            *total += length;
        }
    }
    Ok(watch
        .iter()
        .zip(lengths)
        .filter(|(_, length)| *length > 0.0)
        .map(|(w, length)| {
            let breaks = match w.protection {
                Protection::Avoid => "cuts into the avoid set",
                Protection::Check if leave > 0.0 => {
                    "comes closer than the stock to leave to the check set"
                }
                Protection::Check => "touches the check set",
            };
            crate::assemble::PathWarning {
                message: format!(
                    "The tool {breaks} '{}' along about {length:.3} mm of the toolpath. The set \
                     only warns, so the path is kept: set it to cut the pass short, or check the \
                     cut in the simulation before using the path.",
                    w.name
                ),
                parameter: None,
                requires_acknowledgement: false,
            }
        })
        .collect())
}
