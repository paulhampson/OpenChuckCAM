// SPDX-License-Identifier: AGPL-3.0-or-later
//! The surfaces simultaneous finishing cannot reach, found on the part's
//! triangles for a front end to mark (OP-20, UI-11, UI-12, NFR-06, GC-07;
//! decision record 0515; issue #416).
//!
//! The strategy reports each area it cannot reach with the surface points it
//! sampled there ([`openchuck_path::Unreached::points`]). The area's X and A
//! ranges also take in surfaces the tool did reach (a lip's outside lies in
//! the range of the groove under it), so the front end marks the triangles
//! the points lie on, not the box. [`Application::start_unreached_areas`]
//! finds them off the interface thread, on the very mesh the scene draws for
//! the setup's part ([`crate::model::setup_part`]).

use std::collections::BTreeMap;
use std::sync::Arc;

use openchuck_geom::select::TriangleSet;
use openchuck_path::Unreached;
use openchuck_project::id::{ModelId, OperationId, SetupId};
use openchuck_types::geometry::{Point3, Vec3};
use openchuck_types::kinematics::{AsRotaryFrame, RotaryFrame, tool_axis_in_model};
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::units::Degrees;

use crate::{AppError, Application, JobHandle, Regeneration};

/// How far a sample may lie from the triangle it is taken to be on, in
/// millimetres. The samples lie on the part's exact surface, and the mesh the
/// view draws is a tessellation of it (a STEP part's deflection is a few
/// hundredths of a millimetre), so a sample is never exactly on a triangle;
/// half a millimetre is the order of the strategy's own sample spacing, and
/// the triangle taken is the nearest within it, not any within it, so a
/// neighbouring surface a little further off (a lip's outside beside the
/// groove's undercut) is not marked.
pub const MATCH_TOLERANCE: f64 = 0.5;

/// The start of the warning that lists the areas (`unreached_warning` of
/// simultaneous finishing), by which Messages finds it among an operation's
/// warnings.
pub const WARNING_START: &str = "simultaneous finishing cannot reach ";

/// The start of rest machining's warning that lists its areas, in the same
/// words and order as simultaneous finishing's (decision 0505).
pub const REST_WARNING_START: &str = "rest machining cannot reach ";

/// Whether `message` is a warning that lists unreached areas, largest first,
/// so that Messages offers "Show in view N" for each (decision record 0515
/// item 5).
#[must_use]
pub fn lists_areas(message: &str) -> bool {
    message.starts_with(WARNING_START) || message.starts_with(REST_WARNING_START)
}

/// The most areas Messages offers "Show in view" for: the warning lists the
/// largest five.
pub const LISTED_AREAS: usize = 5;

/// Where the view looks to frame an area (decision 0515 item 5).
#[derive(Debug, Clone, PartialEq)]
pub struct FrameTarget {
    /// The X range the area spans, millimetres.
    pub x: (f64, f64),
    /// The A at the middle of its range, degrees in 0 to 360; for an area
    /// all the way round, the A its marker faces at.
    pub a: f64,
    /// The unit direction from the part towards the viewer in the setup's
    /// frame (the tool's direction at that A): the side the area faces.
    pub direction: [f64; 3],
}

/// One area, found on the part.
#[derive(Debug, Clone, PartialEq)]
pub struct AreaMarks {
    /// Its number, from 1, in the order of the warning and of
    /// [`openchuck_app::GroupInfo::unreached`](crate::GroupInfo::unreached):
    /// "area 2" in Messages is "2" in the view.
    pub number: usize,
    /// The triangles of the part the area's points lie on.
    pub triangles: TriangleSet,
    /// The area's point nearest its middle, where its number is drawn.
    pub marker: Point3,
    /// How many of its points lie on no triangle within
    /// [`MATCH_TOLERANCE`] (none, when the part is the one the run was made
    /// with).
    pub unmatched: usize,
    /// How to look at it.
    pub frame: FrameTarget,
}

/// The areas of one operation.
#[derive(Debug, Clone, PartialEq)]
pub struct OperationAreas {
    /// The operation.
    pub operation: OperationId,
    /// Its areas, largest first.
    pub areas: Vec<AreaMarks>,
}

/// The areas of every operation of one setup, found on its part.
#[derive(Debug, Clone, PartialEq)]
pub struct SetupAreas {
    /// The setup.
    pub setup: SetupId,
    /// The model whose mesh the triangles index (the setup's part).
    pub model: ModelId,
    /// The operations that have areas, in machining order.
    pub operations: Vec<OperationAreas>,
}

/// The triangles of `mesh` that `points` lie on: for each point the nearest
/// triangle within `tolerance`, ties to the lower index. A point near no
/// triangle marks none. Returns also the number of such points.
#[must_use]
pub fn match_points(
    mesh: &TriangleMesh,
    points: &[Point3],
    tolerance: f64,
) -> (TriangleSet, usize) {
    let mut marked = TriangleSet::new();
    let grid = Grid::new(mesh, tolerance);
    let mut unmatched = 0;
    for p in points {
        match grid.nearest(mesh, *p, tolerance) {
            Some(t) => {
                marked.insert(t);
            }
            None => unmatched += 1,
        }
    }
    (marked, unmatched)
}

/// [`match_points`], without the count.
#[must_use]
pub fn triangles_on(mesh: &TriangleMesh, points: &[Point3], tolerance: f64) -> TriangleSet {
    match_points(mesh, points, tolerance).0
}

/// The most cells along an axis of the grid.
const GRID_CELLS: f64 = 48.0;

/// A uniform grid over a mesh: each cell lists the triangles whose box,
/// grown by the tolerance, reaches it. Keyed by cell so memory follows the
/// mesh, and ordered so nothing depends on a hash.
struct Grid {
    min: [f64; 3],
    cell: f64,
    cells: BTreeMap<[i64; 3], Vec<u32>>,
}

impl Grid {
    fn new(mesh: &TriangleMesh, tolerance: f64) -> Self {
        let axes = |v: &Point3| [v.x, v.y, v.z];
        let mut min = [f64::MAX; 3];
        let mut max = [f64::MIN; 3];
        for v in &mesh.vertices {
            for ((low, high), c) in min.iter_mut().zip(max.iter_mut()).zip(axes(v)) {
                *low = low.min(c);
                *high = high.max(c);
            }
        }
        let extent = min
            .iter()
            .zip(&max)
            .map(|(low, high)| high - low)
            .fold(0.0_f64, f64::max);
        let cell = if extent.is_finite() {
            (extent / GRID_CELLS).max(2.0 * tolerance).max(1e-9)
        } else {
            1.0
        };
        let mut grid = Self {
            min,
            cell,
            cells: BTreeMap::new(),
        };
        for (index, corners) in mesh.triangles.iter().enumerate() {
            let Some(t) = triangle(mesh, *corners) else {
                continue;
            };
            let low = grid.cell_of(Point3::new(
                t.iter().map(|p| p.x).fold(f64::MAX, f64::min) - tolerance,
                t.iter().map(|p| p.y).fold(f64::MAX, f64::min) - tolerance,
                t.iter().map(|p| p.z).fold(f64::MAX, f64::min) - tolerance,
            ));
            let high = grid.cell_of(Point3::new(
                t.iter().map(|p| p.x).fold(f64::MIN, f64::max) + tolerance,
                t.iter().map(|p| p.y).fold(f64::MIN, f64::max) + tolerance,
                t.iter().map(|p| p.z).fold(f64::MIN, f64::max) + tolerance,
            ));
            for i in low[0]..=high[0] {
                for j in low[1]..=high[1] {
                    for k in low[2]..=high[2] {
                        grid.cells.entry([i, j, k]).or_default().push(index as u32);
                    }
                }
            }
        }
        grid
    }

    #[allow(clippy::cast_possible_truncation)]
    fn cell_of(&self, p: Point3) -> [i64; 3] {
        [p.x, p.y, p.z]
            .iter()
            .zip(self.min)
            .map(|(c, m)| ((c - m) / self.cell).floor() as i64)
            .collect::<Vec<_>>()
            .try_into()
            .unwrap_or([0; 3])
    }

    fn nearest(&self, mesh: &TriangleMesh, p: Point3, tolerance: f64) -> Option<u32> {
        let candidates = self.cells.get(&self.cell_of(p))?;
        let mut best: Option<(f64, u32)> = None;
        for &index in candidates {
            let Some(t) = mesh
                .triangles
                .get(index as usize)
                .and_then(|c| triangle(mesh, *c))
            else {
                continue;
            };
            let d = closest_on_triangle(p, &t).distance(p);
            if d <= tolerance && best.is_none_or(|(b, _)| d < b) {
                best = Some((d, index));
            }
        }
        best.map(|(_, t)| t)
    }
}

fn triangle(mesh: &TriangleMesh, corners: [u32; 3]) -> Option<[Point3; 3]> {
    let [a, b, c] = corners.map(|i| mesh.vertices.get(i as usize).copied());
    Some([a?, b?, c?])
}

/// The closest point of triangle `t` to `p` (Ericson, Real-Time Collision
/// Detection, 5.1.5).
fn closest_on_triangle(p: Point3, t: &[Point3; 3]) -> Point3 {
    let [a, b, c] = *t;
    let (ab, ac, ap) = (b - a, c - a, p - a);
    let (d1, d2) = (ab.dot(ap), ac.dot(ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = p - b;
    let (d3, d4) = (ab.dot(bp), ac.dot(bp));
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return a + ab * (d1 / (d1 - d3));
    }
    let cp = p - c;
    let (d5, d6) = (ab.dot(cp), ac.dot(cp));
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return a + ac * (d2 / (d2 - d6));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        return b + (c - b) * ((d4 - d3) / ((d4 - d3) + (d5 - d6)));
    }
    let denom = va + vb + vc;
    if denom.abs() <= f64::MIN_POSITIVE {
        // A degenerate triangle: its nearest corner.
        return [a, b, c]
            .into_iter()
            .min_by(|x, y| x.distance(p).total_cmp(&y.distance(p)))
            .unwrap_or(a);
    }
    a + ab * (vb / denom) + ac * (vc / denom)
}

/// The area's point nearest the mean of its points: where its number goes.
/// For an area all the way round the mean lies on the axis and the nearest
/// point is the first of the equally near ones, which is deterministic.
fn marker_of(points: &[Point3]) -> Option<Point3> {
    let first = points.first()?;
    let n = points.len() as f64;
    let sum = points.iter().fold(Vec3::ZERO, |s, p| s + p.to_vec());
    let centre = Point3::ORIGIN + sum * (1.0 / n);
    let mut best = (centre.distance(*first), *first);
    for p in points {
        let d = centre.distance(*p);
        if d < best.0 {
            best = (d, *p);
        }
    }
    Some(best.1)
}

/// Whether the area goes all the way round, as the strategy's warning says
/// "all round" (a span of a full turn).
fn all_round(area: &Unreached) -> bool {
    area.a.1 - area.a.0 >= 360.0 - 1e-9
}

/// How to look at `area`, whose marker is `marker` (decision 0515 item 5):
/// from the side it faces, which is the tool's direction at the middle of its
/// A range, in the setup's frame. An area all round faces no one side; it is
/// looked at from the side its marker is on.
fn frame_target(area: &Unreached, marker: Point3, frame: &RotaryFrame) -> FrameTarget {
    let tool = |a: f64| {
        let d = tool_axis_in_model(frame, Degrees(a)).get();
        [d.x, d.y, d.z]
    };
    let radial = marker.y.hypot(marker.z);
    let (a, direction) = if all_round(area) && radial > 1e-6 {
        let d = [0.0, marker.y / radial, marker.z / radial];
        // The A this side faces the tool at is the tool's: the marker's polar
        // angle phi gives A = sense (90 - phi).
        let phi = marker.z.atan2(marker.y).to_degrees();
        let a = (frame.sense.sign() * (90.0 - phi)).rem_euclid(360.0);
        (a, d)
    } else {
        let a = (0.5 * (area.a.0 + area.a.1)).rem_euclid(360.0);
        (a, tool(a))
    };
    FrameTarget {
        x: area.x,
        a,
        direction,
    }
}

/// `areas` found on `mesh`, numbered from 1 in their order, for a machine
/// whose rotary axis is `frame`.
#[must_use]
pub fn map_areas(
    mesh: &TriangleMesh,
    areas: &[Unreached],
    frame: &impl AsRotaryFrame,
) -> Vec<AreaMarks> {
    let frame = frame.rotary_frame();
    areas
        .iter()
        .enumerate()
        .map(|(i, area)| {
            let (triangles, unmatched) = match_points(mesh, &area.points, MATCH_TOLERANCE);
            let marker = marker_of(&area.points)
                .unwrap_or_else(|| Point3::new(0.5 * (area.x.0 + area.x.1), 0.0, 0.0));
            AreaMarks {
                number: i + 1,
                triangles,
                marker,
                unmatched,
                frame: frame_target(area, marker, &frame),
            }
        })
        .collect()
}

impl Application {
    /// Finds the areas simultaneous finishing could not reach, for every
    /// operation of `setup` in `run`, on the triangles of the setup's part
    /// as the scene draws it, as a job (OP-20; decision record 0515). Refused
    /// with [`AppError::PartChanged`] when the model's file is no longer the
    /// one the run read, as the deviation colours are (decision 0492): its
    /// triangles would not be the ones the points lie on.
    pub fn start_unreached_areas(
        &self,
        run: Arc<Regeneration>,
        setup: SetupId,
    ) -> JobHandle<SetupAreas> {
        let document = self.document.clone();
        self.jobs.submit(
            "unreachable areas",
            self.events_tx.clone(),
            move |context| {
                // The part as the run was made with it.
                let setup_ref = run
                    .project
                    .setup(setup)
                    .ok_or_else(|| AppError::UnknownSetup(setup.to_string()))?;
                let model = setup_ref
                    .part
                    .and_then(|id| run.project.models().iter().find(|m| m.id == id))
                    .ok_or_else(|| AppError::Playback("the setup has no part".to_owned()))?;
                let frame = run
                    .project
                    .machine()
                    .map(RotaryFrame::of_profile)
                    .ok_or_else(|| AppError::Playback("the project has no machine".to_owned()))?;
                let wanted: Vec<_> = run
                    .groups
                    .iter()
                    .filter(|g| g.setup == setup && !g.unreached.is_empty())
                    .collect();
                if wanted.is_empty() {
                    return Ok(SetupAreas {
                        setup,
                        model: model.id,
                        operations: Vec::new(),
                    });
                }
                let part = crate::model::setup_part(&document.location, model, setup_ref, context)?;
                if part.hash != model.hash {
                    return Err(AppError::PartChanged(model.label()));
                }
                let mut operations = Vec::new();
                for group in wanted {
                    context.check_cancelled()?;
                    operations.push(OperationAreas {
                        operation: group.operation,
                        areas: map_areas(&part.mesh, &group.unreached, &frame),
                    });
                }
                Ok(SetupAreas {
                    setup,
                    model: model.id,
                    operations,
                })
            },
        )
    }
}
