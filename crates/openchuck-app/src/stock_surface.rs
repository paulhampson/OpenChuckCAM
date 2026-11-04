// SPDX-License-Identifier: AGPL-3.0-or-later
//! Display-only projection of final simulation samples onto playback stock.

use crate::AppError;
use crate::pathview::{DeviationBands, deviation_bands_with};
use openchuck_geom::pick::MeshPicker;
use openchuck_project::id::{ModelId, SetupId};
use openchuck_sim::{DeviationPoint, ReachClass, SampleOn};
use openchuck_types::geometry::{Point3, Vec3};
use openchuck_types::job::JobContext;
use openchuck_types::mesh::TriangleMesh;
use rayon::prelude::*;

/// Triangles coloured between two looks at the cancel flag, and the share
/// of the work one thread takes at a time.
const CHUNK: usize = 4096;

/// The stock surface drawn and picked in playback. Values are projections
/// of the existing final comparison, never a new simulation measurement.
#[derive(Debug, PartialEq)]
pub struct StockSurface {
    /// Actual stock geometry at the requested playback cursor.
    pub mesh: TriangleMesh,
    /// The setup the stock belongs to.
    pub setup: SetupId,
    /// The target model the final comparison used.
    pub model: Option<ModelId>,
    /// Projected final values indexed by the stock's triangle IDs.
    pub bands: Option<DeviationBands>,
    picker: Option<MeshPicker>,
}

impl StockSurface {
    pub(crate) fn new(
        mesh: TriangleMesh,
        setup: SetupId,
        model: Option<ModelId>,
        samples: Option<&SurfaceSamples>,
        picker: bool,
        context: &JobContext<'_>,
    ) -> Result<Self, AppError> {
        context.check_cancelled()?;
        // A frame shown while playback plays (no picker) is coloured by
        // whole triangles; a still frame is drawn in pieces (decision 0638,
        // the owner's choice B).
        let bands = samples
            .map(|s| s.bands(&mesh, picker, context))
            .transpose()?;
        let picker = if !picker || mesh.triangles.is_empty() {
            None
        } else {
            Some(MeshPicker::new(&mesh)?)
        };
        context.check_cancelled()?;
        Ok(Self {
            mesh,
            setup,
            model,
            bands,
            picker,
        })
    }

    /// Whether the surface was made with its picker, or has nothing to pick:
    /// a frame made while playback plays has none, so pointing at it finds
    /// nothing (the owner's decision D5 on issue #382; decision record 0322).
    pub fn can_pick(&self) -> bool {
        self.picker.is_some() || self.mesh.triangles.is_empty()
    }

    /// First hit on the actual stock, in its workpiece frame. The triangle
    /// ID indexes both its drawn colour and its projected tooltip value.
    /// None when the surface has no triangles or was made without a picker.
    pub fn pick(
        &self,
        origin: [f64; 3],
        direction: [f64; 3],
    ) -> Result<Option<crate::pick::ScenePick>, AppError> {
        let Some(picker) = &self.picker else {
            return Ok(None);
        };
        let [ox, oy, oz] = origin;
        let [dx, dy, dz] = direction;
        let hit = picker.pick(Point3::new(ox, oy, oz), Vec3::new(dx, dy, dz))?;
        Ok(hit.map(|h| crate::pick::ScenePick {
            mesh: 0,
            model: self.model,
            triangle: h.triangle,
            face: None,
            distance: h.distance,
            point: h.point,
            normal: picker.normal(h.triangle),
        }))
    }
}

/// A balanced k-d tree kept implicitly: the samples in median order, so
/// that the node of the run `[low, high)` is its middle and its children
/// are the runs on either side, split along the depth modulo three. It
/// holds only each sample's point and number, a few bytes each, where a
/// tree of linked nodes with boxes holds more than a hundred (issue 523:
/// 3.4 million samples took 490 MB).
struct Tree {
    points: Vec<Point3>,
    /// Each node's sample, by its place in the comparison's points.
    samples: Vec<u32>,
    /// The tight bounds of each run of at least [`BOXED`] samples, by the
    /// run's place in heap order (the root 0, the children of `h` at
    /// `2h + 1` and `2h + 2`): a few per cent of the samples, so a query far
    /// from every sample is turned away near the root.
    boxes: Vec<(Point3, Point3)>,
}

/// The fewest samples in a run that keeps its bounds.
const BOXED: usize = 64;

/// Records the bounds of the run `[low, high)` of `points` at `heap`, and
/// of the runs below it, while they hold at least [`BOXED`] samples.
fn bound(
    points: &[Point3],
    low: usize,
    high: usize,
    heap: usize,
    boxes: &mut Vec<(Point3, Point3)>,
) {
    if high.saturating_sub(low) < BOXED {
        return;
    }
    let mut min = Point3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
    let mut max = Point3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for p in points.get(low..high).unwrap_or_default() {
        min = Point3::new(min.x.min(p.x), min.y.min(p.y), min.z.min(p.z));
        max = Point3::new(max.x.max(p.x), max.y.max(p.y), max.z.max(p.z));
    }
    if boxes.len() <= heap {
        boxes.resize(heap + 1, (min, max));
    }
    if let Some(slot) = boxes.get_mut(heap) {
        *slot = (min, max);
    }
    let middle = low + (high - low) / 2;
    bound(points, low, middle, 2 * heap + 1, boxes);
    bound(points, middle + 1, high, 2 * heap + 2, boxes);
}

impl Tree {
    /// The tree over `ids`, numbers of `points`.
    fn new(points: &[DeviationPoint], mut ids: Vec<u32>) -> Self {
        fn arrange(points: &[DeviationPoint], ids: &mut [u32], depth: usize) {
            if ids.len() < 2 {
                return;
            }
            let axis = depth % 3;
            let middle = ids.len() / 2;
            let at = |id: u32| {
                points
                    .get(id as usize)
                    .map_or(f64::NAN, |p| coordinate(p.point, axis))
            };
            ids.select_nth_unstable_by(middle, |a, b| at(*a).total_cmp(&at(*b)).then(a.cmp(b)));
            let (left, rest) = ids.split_at_mut(middle);
            arrange(points, left, depth + 1);
            if let Some((_, right)) = rest.split_first_mut() {
                arrange(points, right, depth + 1);
            }
        }
        arrange(points, &mut ids, 0);
        let points: Vec<Point3> = ids
            .iter()
            .map(|id| {
                points
                    .get(*id as usize)
                    .map_or(Point3::new(0.0, 0.0, 0.0), |p| p.point)
            })
            .collect();
        let mut boxes = Vec::new();
        bound(&points, 0, points.len(), 0, &mut boxes);
        Self {
            points,
            samples: ids,
            boxes,
        }
    }

    /// The node nearest to `point` within `limit` (squared), as (squared
    /// distance, sample, node): ties to the first sample in the
    /// comparison's order.
    fn nearest(&self, point: Point3, limit: f64) -> Option<(f64, u32, usize)> {
        let mut best = None;
        let mut query = Query {
            point,
            limit,
            best: &mut best,
        };
        self.search(0, self.points.len(), (0, 0), [0.0; 3], 0.0, &mut query);
        best
    }

    /// Searches the run `[low, high)` at `depth`. `offsets` are the query's
    /// distances from the run's cell along each axis, and `reach` the
    /// squared distance from the query to the cell (their sum of squares):
    /// a cell further than the best so far, or the limit, is skipped, as a
    /// bounding box would allow, without storing one (Arya and Mount's
    /// incremental distance).
    fn search(
        &self,
        low: usize,
        high: usize,
        (depth, heap): (usize, usize),
        offsets: [f64; 3],
        reach: f64,
        query: &mut Query<'_>,
    ) {
        if low >= high || reach > query.bound() {
            return;
        }
        if high - low >= BOXED
            && let Some((min, max)) = self.boxes.get(heap)
        {
            let p = query.point;
            let to_box = Vec3::new(
                (min.x - p.x).max(0.0).max(p.x - max.x),
                (min.y - p.y).max(0.0).max(p.y - max.y),
                (min.z - p.z).max(0.0).max(p.z - max.z),
            );
            if to_box.dot(to_box) > query.bound() {
                return;
            }
        }
        let middle = low + (high - low) / 2;
        let (Some(&node), Some(&sample)) = (self.points.get(middle), self.samples.get(middle))
        else {
            return;
        };
        let offset = query.point - node;
        let squared = offset.dot(offset);
        if squared <= query.limit
            && query
                .best
                .as_ref()
                .is_none_or(|b| squared < b.0 || (squared.total_cmp(&b.0).is_eq() && sample < b.1))
        {
            *query.best = Some((squared, sample, middle));
        }
        let axis = depth % 3;
        let delta = coordinate(query.point, axis) - coordinate(node, axis);
        let left = (low, middle, 2 * heap + 1);
        let right = (middle + 1, high, 2 * heap + 2);
        let (near, far) = if delta <= 0.0 {
            (left, right)
        } else {
            (right, left)
        };
        self.search(near.0, near.1, (depth + 1, near.2), offsets, reach, query);
        let old = offsets.get(axis).copied().unwrap_or(0.0);
        let far_reach = reach - old * old + delta * delta;
        let mut far_offsets = offsets;
        if let Some(o) = far_offsets.get_mut(axis) {
            *o = delta;
        }
        self.search(
            far.0,
            far.1,
            (depth + 1, far.2),
            far_offsets,
            far_reach,
            query,
        );
    }
}

/// A nearest-sample search under way.
struct Query<'a> {
    point: Point3,
    limit: f64,
    best: &'a mut Option<(f64, u32, usize)>,
}

impl Query<'_> {
    /// The squared distance beyond which a cell is skipped: the best so far,
    /// or the limit, with a margin of a few rounding errors so that a sample
    /// as near as the best (a tie, which goes to the first sample) is never
    /// skipped.
    fn bound(&self) -> f64 {
        let bound = self.best.as_ref().map_or(self.limit, |b| b.0);
        bound + bound.abs() * 1e-9
    }
}

/// A triangle's projected value (signed, and whether a bound), if any, and
/// whether its material left is enclosed.
type Projected = (Option<(f64, bool)>, bool);

/// A sample's value, and whether its material left is enclosed.
type Valued = ((f64, bool), bool);

/// No sample of the behind table.
const NONE: u32 = u32::MAX;

/// Two spatial indexes over the final comparison, made once per setup, off
/// the UI thread.
pub(crate) struct SurfaceSamples {
    /// The counted samples (decision 0194), which colour the stock within
    /// the search.
    counted: Tree,
    /// Each counted node's value, and whether its material left is enclosed.
    values: Vec<Valued>,
    /// Every finite sample, of any kind, for the material behind a face
    /// beyond the search (decision 0633 item 4).
    all: Tree,
    /// For each node of `all`, its place in `behind` when it stands for the
    /// material behind (a counted part bound, or enclosed), else [`NONE`].
    slots: Vec<u32>,
    /// The value of each sample that stands for the material behind, and
    /// whether it is enclosed.
    behind: Vec<Valued>,
    cell: f64,
    limit_squared: f64,
}

fn coordinate(point: Point3, axis: usize) -> f64 {
    match axis {
        0 => point.x,
        1 => point.y,
        _ => point.z,
    }
}

impl SurfaceSamples {
    #[cfg(test)]
    pub(crate) fn new(points: &[DeviationPoint], cell: f64) -> Self {
        Self::with_reach(points, None, cell)
    }

    /// The indexes over the finite samples of the final comparison, with the
    /// class of each one's material left when known (`classes[i]` for
    /// `points[i]`). Samples are numbered in 32 bits; a comparison has far
    /// fewer (one sample takes 64 bytes).
    pub(crate) fn with_reach(
        points: &[DeviationPoint],
        classes: Option<&[ReachClass]>,
        cell: f64,
    ) -> Self {
        let enclosed = |id: usize| {
            classes
                .and_then(|c| c.get(id))
                .is_some_and(|c| *c == ReachClass::Enclosed)
        };
        let finite: Vec<u32> = points
            .iter()
            .enumerate()
            .filter(|(_, p)| p.point.is_finite() && p.value.is_finite())
            .filter_map(|(id, _)| u32::try_from(id).ok())
            .collect();
        let counted_ids: Vec<u32> = finite
            .iter()
            .copied()
            .filter(|id| points.get(*id as usize).is_some_and(DeviationPoint::counts))
            .collect();
        let counted = Tree::new(points, counted_ids);
        let values = counted
            .samples
            .iter()
            .map(|id| {
                let p = points.get(*id as usize);
                (
                    p.map_or((0.0, false), |p| (p.value, p.at_least)),
                    enclosed(*id as usize),
                )
            })
            .collect();
        let all = Tree::new(points, finite);
        let mut behind = Vec::new();
        let slots = all
            .samples
            .iter()
            .map(|id| {
                let Some(p) = points.get(*id as usize) else {
                    return NONE;
                };
                let inside = enclosed(*id as usize);
                if (p.counts() && p.on == SampleOn::Part && p.at_least) || inside {
                    behind.push(((p.value, p.at_least), inside));
                    u32::try_from(behind.len() - 1).unwrap_or(NONE)
                } else {
                    NONE
                }
            })
            .collect();
        let limit = openchuck_sim::DeviationOptions::for_cell(cell).max_distance + cell;
        Self {
            counted,
            values,
            all,
            slots,
            behind,
            cell,
            limit_squared: limit * limit,
        }
    }

    /// The counted sample nearest to `point` within the search, ties to the
    /// first in the comparison's order, as (squared distance, sample,
    /// value, whether enclosed).
    fn within(&self, point: Point3) -> Option<(f64, usize, (f64, bool), bool)> {
        let (squared, sample, node) = self.counted.nearest(point, self.limit_squared)?;
        let (value, enclosed) = *self.values.get(node)?;
        Some((squared, sample as usize, value, enclosed))
    }

    /// The sample of any kind nearest to `point`, with no limit, ties to the
    /// first in the comparison's order, as (squared distance, sample) with
    /// its value and whether enclosed when it stands for the material
    /// behind a face (a counted part bound, or enclosed).
    fn behind(&self, point: Point3) -> Option<(f64, usize, Option<Valued>)> {
        let (squared, sample, node) = self.all.nearest(point, f64::INFINITY)?;
        let stands = self
            .slots
            .get(node)
            .filter(|slot| **slot != NONE)
            .and_then(|slot| self.behind.get(*slot as usize))
            .copied();
        Some((squared, sample as usize, stands))
    }

    /// The final comparison's value nearest to each triangle of `mesh`, in
    /// triangle order, sorted into bands. Each triangle's value depends on
    /// that triangle alone, so the triangles are shared out over the job's
    /// threads in fixed chunks and the values are the same at any thread
    /// count (NFR-06). With `pieces`, a face holding more than one band is
    /// drawn in pieces (decision 0638).
    pub(crate) fn bands(
        &self,
        mesh: &TriangleMesh,
        pieces: bool,
        context: &JobContext<'_>,
    ) -> Result<DeviationBands, AppError> {
        context.check_cancelled()?;
        mesh.validate()?;
        let chunks = mesh.triangles.len().div_ceil(CHUNK);
        let chunks: Vec<Vec<Projected>> = (0..chunks)
            .into_par_iter()
            .map(|chunk| {
                context.check_cancelled()?;
                let end = ((chunk + 1) * CHUNK).min(mesh.triangles.len());
                (chunk * CHUNK..end)
                    .map(|triangle| self.value_of(mesh, triangle))
                    .collect()
            })
            .collect::<Result<_, AppError>>()?;
        let (values, unreachable): (Vec<_>, Vec<_>) = chunks.into_iter().flatten().unzip();
        let bands = deviation_bands_with(&values, &unreachable, self.cell);
        if !pieces {
            return Ok(bands);
        }
        // A face across a band's edge is drawn in pieces (decision 0638).
        let pieces = crate::pieces::refine(
            mesh,
            &|_| true,
            &|_, point, corners| self.value_at(corners, point),
            None,
            crate::pieces::Limits::for_cell(self.cell),
            context,
        )?;
        Ok(crate::pieces::with_pieces(bands, pieces))
    }

    /// The value of the counted sample nearest to the centroid of
    /// `triangle` within two cells (on the part's surface), and whether its
    /// material left is enclosed. Further off, the face stands for the
    /// material behind it (decision 0638 amending decision 0633 item 4): the sample of any kind nearest to the point one search distance
    /// behind the face, into the stock, when that is a counted part sample
    /// whose value is only a bound (stock past an end that was not faced:
    /// at least that much is left there) or whose material left is enclosed
    /// (a bore full of stock behind its cap at the part's end, however thin
    /// its wall) (issue 478). Else the nearest counted sample within the
    /// search, and beyond it none: not measured.
    fn value_of(&self, mesh: &TriangleMesh, triangle: usize) -> Result<Projected, AppError> {
        let [a, b, c] = mesh.triangle(triangle).ok_or_else(|| {
            AppError::Playback("the stock surface has an invalid triangle".to_owned())
        })?;
        let point = Point3::new(
            a.x / 3.0 + b.x / 3.0 + c.x / 3.0,
            a.y / 3.0 + b.y / 3.0 + c.y / 3.0,
            a.z / 3.0 + b.z / 3.0 + c.z / 3.0,
        );
        Ok(self.value_at([a, b, c], point))
    }

    /// [`SurfaceSamples::value_of`] at `point` on a face with the corners
    /// `[a, b, c]`: the same rule at a point of a piece (decision 0638).
    fn value_at(&self, [a, b, c]: [Point3; 3], point: Point3) -> Projected {
        let near = self.within(point);
        // On the part's surface: within two cells of a counted sample.
        let surface = 2.0 * self.cell;
        if let Some((squared, _, value, enclosed)) = near
            && squared <= surface * surface
        {
            return (Some(value), enclosed);
        }
        // Off the surface, the face stands for what is behind it (decision
        // 0638 amending decision 0633 item 4): one search distance behind
        // the face, into the stock.
        let normal = (b - a).cross(c - a);
        let length = normal.length();
        let behind = if length.is_finite() && length > 0.0 {
            point + normal * (-self.limit_squared.sqrt() / length)
        } else {
            point
        };
        if let Some((value, enclosed)) = self.behind(behind).and_then(|(_, _, stands)| stands) {
            return (Some(value), enclosed);
        }
        // Nothing bound or enclosed behind it: the nearest counted sample
        // within the search, else not measured.
        near.map_or((None, false), |(_, _, value, enclosed)| {
            (Some(value), enclosed)
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use crate::pathview::DeviationBand;
    use openchuck_types::geometry::{Point3, Vec3};
    use openchuck_types::job::{CancelToken, NoProgress};

    fn sample(x: f64, value: f64, at_least: bool, on: SampleOn) -> DeviationPoint {
        DeviationPoint {
            point: Point3::new(x, 0.0, 0.0),
            normal: Vec3::new(0.0, 0.0, 1.0),
            value,
            at_least,
            on,
        }
    }

    #[test]
    fn req_sim_05_req_sim_11_stock_colours_and_probes_use_nearest_existing_samples_with_bounds() {
        let points = vec![
            sample(-1.0, -0.4, true, SampleOn::Part),
            sample(1.0, 0.7, false, SampleOn::Part),
            sample(0.0, 8.0, false, SampleOn::Inside),
        ];
        let before = points.clone();
        let samples = SurfaceSamples::new(&points, 0.1);
        let mesh = TriangleMesh {
            // Within two cells of the samples at ±1 (on the part's surface,
            // decision 0638 item 5), between them, and far from both.
            vertices: [-0.9, 0.9, 0.0, 10.0]
                .into_iter()
                .flat_map(|x| {
                    [
                        Point3::new(x, -0.01, 0.0),
                        Point3::new(x, 0.01, 0.0),
                        Point3::new(x, 0.0, 0.01),
                    ]
                })
                .collect(),
            triangles: vec![[0, 1, 2], [3, 4, 5], [6, 7, 8], [9, 10, 11]],
        };
        let cancel = CancelToken::new();
        let bands = samples
            .bands(&mesh, true, &JobContext::new(&cancel, &NoProgress))
            .unwrap();
        assert_eq!(
            &*bands.values,
            &[
                Some((-0.4, true)),
                Some((0.7, false)),
                Some((-0.4, true)),
                None
            ],
            "signed values/bounds remain original; ties choose the first sample; remote stock stays unmeasured"
        );
        assert_eq!(
            points, before,
            "projection never changes simulation measurements"
        );
    }

    #[test]
    fn req_nfr_04_stock_surface_mapping_can_be_cancelled() {
        let samples = SurfaceSamples::new(&[sample(0.0, 0.2, false, SampleOn::Part)], 0.1);
        let cancel = CancelToken::new();
        cancel.cancel();
        assert!(
            samples
                .bands(
                    &TriangleMesh::default(),
                    true,
                    &JobContext::new(&cancel, &NoProgress)
                )
                .is_err()
        );
    }

    #[test]
    fn req_sim_05_req_sim_11_stock_surface_picker_hits_the_coloured_outer_surface() {
        let cancel = CancelToken::new();
        let context = JobContext::new(&cancel, &NoProgress);
        let samples = SurfaceSamples::new(&[sample(0.0, 0.7, false, SampleOn::Part)], 0.1);
        let mesh = TriangleMesh {
            vertices: vec![
                Point3::new(-1.0, -1.0, 2.0),
                Point3::new(1.0, -1.0, 2.0),
                Point3::new(0.0, 1.0, 2.0),
            ],
            triangles: vec![[0, 1, 2]],
        };
        let surface = StockSurface::new(
            mesh,
            SetupId::new(),
            Some(ModelId::new()),
            Some(&samples),
            true,
            &context,
        )
        .unwrap();
        let hit = surface
            .pick([0.0, 0.0, 5.0], [0.0, 0.0, -1.0])
            .unwrap()
            .unwrap();
        assert!(
            (hit.point.z - 2.0).abs() < 1e-12,
            "the target lies at z=0; the probe must hit stock at z=2"
        );
        assert_eq!(
            surface
                .bands
                .as_ref()
                .unwrap()
                .values
                .get(hit.triangle as usize),
            Some(&Some((0.7, false)))
        );
        assert!(surface.pick([0.0; 3], [0.0; 3]).is_err());
        let empty = StockSurface::new(
            TriangleMesh::default(),
            SetupId::new(),
            None,
            None,
            true,
            &context,
        )
        .unwrap();
        assert!(empty.pick([0.0; 3], [0.0, 0.0, 1.0]).unwrap().is_none());
        assert!(empty.bands.is_none());
    }

    /// Samples on a grid with ties (two samples at one point), and a stock
    /// mesh of many chunks: triangles near the samples, on them, between
    /// them, and some too far from any to be measured.
    fn many_samples_and_triangles() -> (Vec<DeviationPoint>, TriangleMesh) {
        let mut points: Vec<_> = (0..60)
            .flat_map(|x| {
                (0..60).map(move |y| {
                    let mut p = sample(
                        f64::from(x) * 0.1,
                        f64::from((x * 7 + y * 13) % 23) * 0.01 - 0.1,
                        (x + y) % 5 == 0,
                        SampleOn::Part,
                    );
                    p.point.y = f64::from(y) * 0.1;
                    p
                })
            })
            .collect();
        // A tie: the same point again with another value; the first wins.
        points.push(sample(0.0, 0.33, false, SampleOn::Part));
        let mut vertices = Vec::new();
        let mut triangles = Vec::new();
        for i in 0..20_000_u32 {
            // Every tenth triangle is far from every sample.
            let far = if i % 10 == 0 { 20.0 } else { 0.0 };
            let x = f64::from(i % 157) * 0.041 - 0.5 + far;
            let y = f64::from(i % 211) * 0.031 - 0.4;
            let z = f64::from(i % 7) * 0.05;
            let base = u32::try_from(vertices.len()).unwrap();
            vertices.extend([
                Point3::new(x, y, z),
                Point3::new(x + 0.01, y, z),
                Point3::new(x, y + 0.01, z),
            ]);
            triangles.push([base, base + 1, base + 2]);
        }
        (
            points,
            TriangleMesh {
                vertices,
                triangles,
            },
        )
    }

    #[test]
    fn req_sim_05_req_nfr_06_stock_colours_are_the_same_at_any_thread_count() {
        let (points, mesh) = many_samples_and_triangles();
        let samples = SurfaceSamples::new(&points, 0.1);
        // The rule, one triangle after another: the nearest counted sample
        // to the triangle's centroid within the search, else the nearest
        // sample when it is a bound on the part (issue 478).
        let expected: Vec<_> = (0..mesh.triangles.len())
            .map(|t| samples.value_of(&mesh, t).unwrap())
            .collect();
        assert!(
            expected.iter().any(|(v, _)| v.is_none()) && expected.iter().any(|(v, _)| v.is_some())
        );
        let (values, unreachable): (Vec<_>, Vec<_>) = expected.into_iter().unzip();
        let expected = crate::pathview::deviation_bands_with(&values, &unreachable, 0.1);
        let cancel = CancelToken::new();
        let context = JobContext::new(&cancel, &NoProgress);
        for threads in [1, 2, 4] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            let bands = pool
                .install(|| samples.bands(&mesh, true, &context))
                .unwrap();
            assert_eq!(bands, expected, "{threads} threads");
        }
    }

    /// The owner's decision D5 on issue #382 (decision record 0322): while
    /// playback plays, a frame is made without its picker. It has the same
    /// mesh and colours, and pointing at it finds nothing; made still, its
    /// picks are those of a frame with a picker.
    #[test]
    fn req_sim_11_req_nfr_04_a_playing_frame_has_no_picker_and_is_otherwise_the_same() {
        let (points, mesh) = many_samples_and_triangles();
        let samples = SurfaceSamples::new(&points, 0.1);
        let cancel = CancelToken::new();
        let context = JobContext::new(&cancel, &NoProgress);
        let (setup, model) = (SetupId::new(), Some(ModelId::new()));
        let still =
            StockSurface::new(mesh.clone(), setup, model, Some(&samples), true, &context).unwrap();
        let playing =
            StockSurface::new(mesh.clone(), setup, model, Some(&samples), false, &context).unwrap();
        assert!(still.can_pick() && !playing.can_pick());
        assert_eq!((&playing.mesh, &playing.bands), (&still.mesh, &still.bands));
        let picker = MeshPicker::new(&mesh).unwrap();
        let mut hits = 0;
        for i in 0..200_u32 {
            let x = f64::from(i % 20) * 0.3 - 0.4;
            let y = f64::from(i / 20) * 0.6 - 0.3;
            let (origin, down) = ([x, y, 5.0], [0.0, 0.0, -1.0]);
            assert!(playing.pick(origin, down).unwrap().is_none());
            let expected = picker
                .pick(Point3::new(x, y, 5.0), Vec3::new(0.0, 0.0, -1.0))
                .unwrap();
            let got = still.pick(origin, down).unwrap();
            assert_eq!(
                got.map(|h| (h.triangle, h.distance)),
                expected.map(|h| (h.triangle, h.distance))
            );
            hits += usize::from(got.is_some());
        }
        assert!(hits > 0);
    }

    /// A cut part with no triangles has nothing to pick: a still frame of it
    /// is complete, so the playback panel does not make it again and again
    /// (decision record 0322 item 6; review of pull request 401).
    #[test]
    fn req_sim_11_req_nfr_04_an_empty_still_frame_needs_no_picker() {
        let cancel = CancelToken::new();
        let context = JobContext::new(&cancel, &NoProgress);
        let empty = TriangleMesh {
            vertices: Vec::new(),
            triangles: Vec::new(),
        };
        let still = StockSurface::new(empty, SetupId::new(), None, None, true, &context).unwrap();
        assert!(
            still.can_pick(),
            "nothing to pick, so the still frame is done"
        );
        assert!(
            still
                .pick([0.0, 0.0, 5.0], [0.0, 0.0, -1.0])
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn req_sim_05_spatial_projection_matches_an_independent_exhaustive_nearest_sample_search() {
        let points: Vec<_> = (-10..=10)
            .flat_map(|x| {
                (-10..=10).map(move |y| {
                    let mut p = sample(
                        f64::from(x) * 0.2,
                        f64::from(x + y) * 0.01,
                        x % 3 == 0,
                        SampleOn::Part,
                    );
                    p.point.y = f64::from(y) * 0.2;
                    p
                })
            })
            .collect();
        let index = SurfaceSamples::new(&points, 0.1);
        for x in -15..15 {
            let query = Point3::new(f64::from(x) * 0.19, 0.071, f64::from(x % 4));
            let reference = points
                .iter()
                .enumerate()
                .map(|(id, p)| {
                    let d = query - p.point;
                    (d.dot(d), id, (p.value, p.at_least))
                })
                .filter(|(d, _, _)| *d <= 3.1 * 3.1)
                .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
            let actual = index.within(query).map(|(d, id, value, _)| (d, id, value));
            assert_eq!(actual, reference);
        }
    }

    /// Samples of every kind on a small grid, some at the same point (the
    /// tie rule), some counted, some bounds and some enclosed.
    fn mixed_samples() -> (Vec<DeviationPoint>, Vec<ReachClass>) {
        let kinds = [
            SampleOn::Part,
            SampleOn::Workholding,
            SampleOn::Inside,
            SampleOn::Part,
        ];
        let mut points = Vec::new();
        let mut classes = Vec::new();
        for i in 0..600_i32 {
            let x = f64::from(i % 7) * 0.3;
            let y = f64::from((i / 7) % 9) * 0.25;
            let z = f64::from(i % 5) * 0.4 - 0.8;
            let mut p = sample(
                x,
                f64::from(i % 11) * 0.3 - 0.5,
                i % 3 == 0,
                kinds[(i % 4) as usize],
            );
            p.point.y = y;
            p.point.z = z;
            points.push(p);
            classes.push(match i % 6 {
                0 => ReachClass::Enclosed,
                1 => ReachClass::Open,
                2 => ReachClass::NotLeft,
                _ => ReachClass::Undecided,
            });
        }
        // A sample that is not finite is never in the index.
        let mut bad = sample(0.0, f64::NAN, false, SampleOn::Part);
        bad.point.x = 0.15;
        points.push(bad);
        classes.push(ReachClass::Enclosed);
        (points, classes)
    }

    #[test]
    fn req_sim_05_the_material_behind_is_read_from_an_exhaustive_nearest_sample_of_any_kind() {
        // Decision 0633 item 4: the sample of any kind (part, held region,
        // tab or inside the part) nearest to the point behind a face, ties to
        // the first in the comparison's order, stands for the material there
        // only when it is a counted part bound or enclosed. The index must
        // give what an exhaustive search over every finite sample gives.
        let (points, classes) = mixed_samples();
        let index = SurfaceSamples::with_reach(&points, Some(&classes), 0.1);
        let mut checked = (0, 0);
        for k in 0..400_i32 {
            let query = Point3::new(
                f64::from(k % 13) * 0.17 - 0.4,
                f64::from(k % 17) * 0.15 - 0.3,
                f64::from(k % 7) * 0.5 - 1.6,
            );
            let reference = points
                .iter()
                .enumerate()
                .filter(|(_, p)| p.point.is_finite() && p.value.is_finite())
                .map(|(id, p)| {
                    let d = query - p.point;
                    (d.dot(d), id)
                })
                .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
                .map(|(d, id)| {
                    let p = &points[id];
                    let enclosed = classes[id] == ReachClass::Enclosed;
                    let stands = (p.counts() && p.on == SampleOn::Part && p.at_least) || enclosed;
                    (d, id, stands.then_some(((p.value, p.at_least), enclosed)))
                });
            let actual = index.behind(query);
            assert_eq!(actual, reference, "query {query:?}");
            if reference.is_some_and(|r| r.2.is_some()) {
                checked.0 += 1;
            } else {
                checked.1 += 1;
            }
        }
        assert!(checked.0 > 20 && checked.1 > 20, "{checked:?}");
        // Within the search, the counted samples only, with their class.
        for k in 0..400_i32 {
            let query = Point3::new(
                f64::from(k % 13) * 0.17,
                f64::from(k % 17) * 0.15,
                f64::from(k % 5) * 0.9,
            );
            let reference = points
                .iter()
                .enumerate()
                .filter(|(_, p)| p.counts() && p.point.is_finite() && p.value.is_finite())
                .map(|(id, p)| {
                    let d = query - p.point;
                    (d.dot(d), id, (p.value, p.at_least))
                })
                .filter(|(d, _, _)| *d <= 3.1 * 3.1)
                .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
                .map(|(d, id, v)| (d, id, v, classes[id] == ReachClass::Enclosed));
            assert_eq!(index.within(query), reference, "query {query:?}");
        }
    }

    #[test]
    fn req_sim_05_material_left_that_no_tool_reaches_is_its_own_band() {
        let values = [
            Some((2.0, false)),
            Some((3.0, true)),
            Some((0.05, false)),
            Some((-0.4, false)),
            None,
            Some((2.0, false)),
        ];
        let bands = crate::pathview::deviation_bands_with(
            &values,
            &[true, true, true, true, true, false],
            0.1,
        );
        let band_of = |t: u32| {
            bands
                .bands
                .iter()
                .find(|(_, set)| set.contains(&t))
                .map(|(b, _)| *b)
        };
        assert_eq!(band_of(0), Some(DeviationBand::Unreachable));
        assert_eq!(band_of(1), Some(DeviationBand::Unreachable));
        // Only material left is unreachable; the rest keep their bands.
        assert_eq!(band_of(2), Some(DeviationBand::OnTarget));
        assert_eq!(band_of(3), Some(DeviationBand::Overcut));
        assert_eq!(band_of(4), Some(DeviationBand::Unmeasured));
        assert_eq!(band_of(5), Some(DeviationBand::Thick));
        assert_eq!(&*bands.values, &values, "the probe's values are unchanged");
    }

    /// One tiny triangle round `x` on the axis.
    fn triangle_at(mesh: &mut TriangleMesh, x: f64) {
        let first = u32::try_from(mesh.vertices.len()).unwrap();
        mesh.vertices.extend([
            Point3::new(x, -0.01, 0.0),
            Point3::new(x, 0.01, 0.0),
            Point3::new(x, 0.0, 0.01),
        ]);
        mesh.triangles.push([first, first + 1, first + 2]);
    }

    #[test]
    fn req_sim_05_req_nfr_12_stock_beyond_the_search_takes_the_class_of_a_bound_part_sample() {
        // Stock left far from the part (a bore full of it, or stock past an
        // end that was not faced) is far from every sample; when the nearest
        // is on the part and its value is only a bound, the stock has at
        // least that much left: it is not "not measured" (issue 478).
        let points = vec![
            sample(0.0, 3.0, true, SampleOn::Part),
            sample(100.0, 3.0, true, SampleOn::Part),
            sample(50.0, 5.0, true, SampleOn::Workholding),
            sample(-50.0, 8.0, true, SampleOn::Inside),
            sample(200.0, 0.5, false, SampleOn::Part),
        ];
        let classes = [
            ReachClass::Open,
            ReachClass::Enclosed,
            ReachClass::NotLeft,
            ReachClass::NotLeft,
            ReachClass::Undecided,
        ];
        let samples = SurfaceSamples::with_reach(&points, Some(&classes), 0.1);
        let mut mesh = TriangleMesh::default();
        for x in [10.0, 95.0, 45.0, -45.0, 190.0, 1.0] {
            triangle_at(&mut mesh, x);
        }
        let cancel = CancelToken::new();
        let bands = samples
            .bands(&mesh, true, &JobContext::new(&cancel, &NoProgress))
            .unwrap();
        let band_of = |t: u32| {
            bands
                .bands
                .iter()
                .find(|(_, set)| set.contains(&t))
                .map(|(b, _)| *b)
        };
        assert_eq!(bands.values[0], Some((3.0, true)));
        assert_eq!(band_of(0), Some(DeviationBand::Thick));
        assert_eq!(band_of(1), Some(DeviationBand::Unreachable));
        // Nearest a held region or tab, or a face inside the part: not
        // measured, as before (decision 0194).
        assert_eq!(bands.values[2], None);
        assert_eq!(bands.values[3], None);
        // Nearest a part sample that is not a bound: the stock is further
        // than its value, so nothing is claimed.
        assert_eq!(bands.values[4], None);
        // Within the search, the nearest counted sample, as before.
        assert_eq!(bands.values[5], Some((3.0, true)));
        assert_eq!(band_of(5), Some(DeviationBand::Thick));
    }

    #[test]
    fn req_sim_05_a_stock_face_over_a_bore_takes_the_class_of_the_material_behind_it() {
        // Issue 478: the stock's cap over a bore full of stock, at the part's
        // faced end, is beyond the search from every sample, and nearest to
        // the rim of the opening, where nothing is left. The material behind
        // the cap is in the bore: the face takes the class of what lies one
        // search distance behind it, into the stock.
        let points = vec![
            // The rim of the opening, at the end X 0: nothing left there.
            sample(0.0, 0.02, false, SampleOn::Part),
            // The bore wall 3.1 mm in from the end: a bound, enclosed.
            sample(-3.1, 3.0, true, SampleOn::Part),
        ];
        let mut points = points;
        points[0].point.y = 6.0;
        points[1].point.y = 6.5;
        let classes = [ReachClass::NotLeft, ReachClass::Enclosed];
        let samples = SurfaceSamples::with_reach(&points, Some(&classes), 0.1);
        // A cap triangle at X 0 on the axis, facing +X (out of the stock).
        let mesh = TriangleMesh {
            vertices: vec![
                Point3::new(0.0, -0.01, -0.01),
                Point3::new(0.0, 0.01, -0.01),
                Point3::new(0.0, 0.0, 0.01),
            ],
            triangles: vec![[0, 1, 2]],
        };
        let cancel = CancelToken::new();
        let bands = samples
            .bands(&mesh, true, &JobContext::new(&cancel, &NoProgress))
            .unwrap();
        assert_eq!(bands.values[0], Some((3.0, true)));
        assert!(
            bands
                .bands
                .iter()
                .any(|(b, set)| *b == DeviationBand::Unreachable && set.contains(&0)),
            "{:?}",
            bands.bands
        );
        // A thin wall: the bore's material left is the wall's thickness, not
        // a bound; still enclosed, still the bore's.
        let mut thin = points.clone();
        thin[1].value = 2.0;
        thin[1].at_least = false;
        let thin_samples = SurfaceSamples::with_reach(&thin, Some(&classes), 0.1);
        let bands = thin_samples
            .bands(&mesh, true, &JobContext::new(&cancel, &NoProgress))
            .unwrap();
        assert_eq!(bands.values[0], Some((2.0, false)));
        assert!(
            bands
                .bands
                .iter()
                .any(|(b, set)| *b == DeviationBand::Unreachable && set.contains(&0))
        );
        // Not enclosed and not a bound: nothing is claimed.
        let open =
            SurfaceSamples::with_reach(&thin, Some(&[ReachClass::NotLeft, ReachClass::Open]), 0.1);
        let bands = open
            .bands(&mesh, true, &JobContext::new(&cancel, &NoProgress))
            .unwrap();
        assert_eq!(bands.values[0], None);
        // The same face turned to face -X has the open end behind it, the
        // rim nearest: not measured.
        let flipped = TriangleMesh {
            vertices: mesh.vertices.clone(),
            triangles: vec![[0, 2, 1]],
        };
        let bands = samples
            .bands(&flipped, true, &JobContext::new(&cancel, &NoProgress))
            .unwrap();
        assert_eq!(bands.values[0], None);
    }

    #[test]
    fn req_sim_05_a_stock_face_off_the_part_s_surface_takes_the_material_behind_it() {
        // Decision 0638 amending decision 0633 item 4 (owner, 2 Nov 2026):
        // a cap over a bore lies within the search of the opening's rim, so
        // the rim's "nothing left" covered most of it. A face more than two
        // cells from every counted sample is off the part's surface and
        // takes the material behind it when that is a bound or enclosed.
        let mut points = vec![
            // The rim of the opening, at the end X 0: nothing left there.
            sample(0.0, 0.02, false, SampleOn::Part),
            // The bore wall 3.1 mm in from the end: a bound, enclosed.
            sample(-3.1, 3.0, true, SampleOn::Part),
        ];
        points[0].point.y = 0.5;
        points[1].point.y = 0.6;
        let classes = [ReachClass::NotLeft, ReachClass::Enclosed];
        let samples = SurfaceSamples::with_reach(&points, Some(&classes), 0.1);
        // Cap faces at X 0 facing +X: 0.5 mm (five cells) from the rim, and
        // 0.15 mm (one and a half cells) from it.
        let cap = |y: f64| {
            [
                Point3::new(0.0, y - 0.01, -0.01),
                Point3::new(0.0, y + 0.01, -0.01),
                Point3::new(0.0, y, 0.01),
            ]
        };
        let mesh = TriangleMesh {
            vertices: cap(0.0).into_iter().chain(cap(0.35)).collect(),
            triangles: vec![[0, 1, 2], [3, 4, 5]],
        };
        let cancel = CancelToken::new();
        let bands = samples
            .bands(&mesh, true, &JobContext::new(&cancel, &NoProgress))
            .unwrap();
        assert_eq!(bands.values[0], Some((3.0, true)), "off the surface");
        assert!(
            bands
                .bands
                .iter()
                .any(|(b, set)| *b == DeviationBand::Unreachable && set.contains(&0))
        );
        // Within two cells of a counted sample: on the surface, as before.
        assert_eq!(bands.values[1], Some((0.02, false)), "on the surface");
        // Off the surface, with nothing bound or enclosed behind it: the
        // nearest counted sample within the search, as before.
        let mut open_points = points.clone();
        open_points[1].at_least = false;
        let open = SurfaceSamples::with_reach(
            &open_points,
            Some(&[ReachClass::NotLeft, ReachClass::Open]),
            0.1,
        );
        let bands = open
            .bands(&mesh, true, &JobContext::new(&cancel, &NoProgress))
            .unwrap();
        assert_eq!(bands.values[0], Some((0.02, false)));
    }

    #[test]
    fn req_sim_05_a_stock_face_across_a_band_edge_is_drawn_in_pieces() {
        // Decision 0638: a large stock face, half over material on the
        // surface and half over a bore, is drawn in pieces, each in its band.
        let mut points = Vec::new();
        for i in 0..100 {
            let x = f64::from(i) * 0.1;
            let left = x < 5.0;
            let mut p = sample(x, if left { 0.0 } else { 3.0 }, !left, SampleOn::Part);
            p.point.z = -0.05;
            points.push(p);
        }
        let classes: Vec<_> = points
            .iter()
            .map(|p| {
                if p.value > 0.1 {
                    ReachClass::Enclosed
                } else {
                    ReachClass::NotLeft
                }
            })
            .collect();
        let samples = SurfaceSamples::with_reach(&points, Some(&classes), 0.1);
        let mesh = TriangleMesh {
            vertices: vec![
                Point3::new(0.0, -0.05, 0.0),
                Point3::new(10.0, -0.05, 0.0),
                Point3::new(5.0, 0.05, 0.0),
            ],
            triangles: vec![[0, 1, 2]],
        };
        let cancel = CancelToken::new();
        let bands = samples
            .bands(&mesh, true, &JobContext::new(&cancel, &NoProgress))
            .unwrap();
        let pieces = bands.pieces.as_ref().expect("drawn in pieces");
        assert!(pieces.parents.contains(&0));
        assert_eq!(
            bands.band_at(0, [1.0, 0.0, 0.0]),
            Some(DeviationBand::OnTarget)
        );
        assert_eq!(
            bands.band_at(0, [9.0, -0.04, 0.0]),
            Some(DeviationBand::Unreachable)
        );
        assert!(bands.bands.iter().all(|(_, set)| !set.contains(&0)));
    }
}
