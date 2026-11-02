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
        let bands = samples.map(|s| s.bands(&mesh, context)).transpose()?;
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

struct Node {
    point: Point3,
    value: (f64, bool),
    /// Whether the sample counts (decision 0194): only those colour the
    /// stock within the search.
    counts: bool,
    /// A counted part sample whose value is only a bound: stock beyond the
    /// search nearest to it has at least that much left (issue 478).
    part_bound: bool,
    /// Its material left is enclosed: no tool reaches it. Only a counted
    /// part sample with material left is classed so.
    enclosed: bool,
    /// Whether it or a node below it counts, so a search for counted
    /// samples skips the subtrees of a held region or tab alone.
    any_counts: bool,
    min: Point3,
    max: Point3,
    original: usize,
    axis: usize,
    left: Option<usize>,
    right: Option<usize>,
}

/// A triangle's projected value (signed, and whether a bound), if any, and
/// whether its material left is enclosed.
type Projected = (Option<(f64, bool)>, bool);

/// A balanced spatial index made once per setup, off the UI thread.
pub(crate) struct SurfaceSamples {
    nodes: Vec<Node>,
    root: Option<usize>,
    cell: f64,
    limit_squared: f64,
    /// Each sample's class, by its original index, when known.
    classes: Option<Vec<ReachClass>>,
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

    /// The index over every finite sample of the final comparison, with the
    /// class of each one's material left when known (`classes[i]` for
    /// `points[i]`).
    pub(crate) fn with_reach(
        points: &[DeviationPoint],
        classes: Option<&[ReachClass]>,
        cell: f64,
    ) -> Self {
        let mut points: Vec<_> = points
            .iter()
            .copied()
            .enumerate()
            .filter(|(_, p)| p.point.is_finite() && p.value.is_finite())
            .collect();
        let limit = openchuck_sim::DeviationOptions::for_cell(cell).max_distance + cell;
        let mut index = Self {
            nodes: Vec::with_capacity(points.len()),
            root: None,
            cell,
            limit_squared: limit * limit,
            classes: classes.map(<[ReachClass]>::to_vec),
        };
        index.root = index.build(&mut points, 0);
        index
    }

    fn build(&mut self, points: &mut [(usize, DeviationPoint)], depth: usize) -> Option<usize> {
        if points.is_empty() {
            return None;
        }
        let axis = depth % 3;
        let middle = points.len() / 2;
        points.select_nth_unstable_by(middle, |a, b| {
            coordinate(a.1.point, axis)
                .total_cmp(&coordinate(b.1.point, axis))
                .then(a.0.cmp(&b.0))
        });
        let (left, rest) = points.split_at_mut(middle);
        let ((original, point), right) = rest.split_first_mut()?;
        let id = self.nodes.len();
        let enclosed = self
            .classes
            .as_ref()
            .and_then(|c| c.get(*original))
            .is_some_and(|c| *c == ReachClass::Enclosed);
        self.nodes.push(Node {
            point: point.point,
            value: (point.value, point.at_least),
            counts: point.counts(),
            part_bound: point.counts() && point.on == SampleOn::Part && point.at_least,
            enclosed,
            any_counts: point.counts(),
            min: point.point,
            max: point.point,
            original: *original,
            axis,
            left: None,
            right: None,
        });
        let left = self.build(left, depth + 1);
        let right = self.build(right, depth + 1);
        let mut min = point.point;
        let mut max = point.point;
        for child in [left, right]
            .into_iter()
            .flatten()
            .filter_map(|id| self.nodes.get(id))
        {
            min = Point3::new(
                min.x.min(child.min.x),
                min.y.min(child.min.y),
                min.z.min(child.min.z),
            );
            max = Point3::new(
                max.x.max(child.max.x),
                max.y.max(child.max.y),
                max.z.max(child.max.z),
            );
        }
        let below = [left, right]
            .into_iter()
            .flatten()
            .filter_map(|id| self.nodes.get(id))
            .any(|child| child.any_counts);
        if let Some(node) = self.nodes.get_mut(id) {
            node.left = left;
            node.right = right;
            node.min = min;
            node.max = max;
            node.any_counts |= below;
        }
        Some(id)
    }

    fn nearest(
        &self,
        id: Option<usize>,
        point: Point3,
        best: &mut Option<(f64, usize, (f64, bool))>,
    ) {
        let Some(node) = id
            .and_then(|id| self.nodes.get(id))
            .filter(|node| node.any_counts)
        else {
            return;
        };
        let to_box = Vec3::new(
            (node.min.x - point.x).max(0.0).max(point.x - node.max.x),
            (node.min.y - point.y).max(0.0).max(point.y - node.max.y),
            (node.min.z - point.z).max(0.0).max(point.z - node.max.z),
        );
        if to_box.dot(to_box) > best.as_ref().map_or(self.limit_squared, |b| b.0) {
            return;
        }
        let offset = point - node.point;
        let squared = offset.dot(offset);
        if node.counts
            && squared <= self.limit_squared
            && best.as_ref().is_none_or(|b| {
                squared < b.0 || (squared.total_cmp(&b.0).is_eq() && node.original < b.1)
            })
        {
            *best = Some((squared, node.original, node.value));
        }
        let delta = coordinate(point, node.axis) - coordinate(node.point, node.axis);
        let (near, far) = if delta <= 0.0 {
            (node.left, node.right)
        } else {
            (node.right, node.left)
        };
        self.nearest(near, point, best);
        if delta * delta <= best.as_ref().map_or(self.limit_squared, |b| b.0) {
            self.nearest(far, point, best);
        }
    }

    /// The sample of any kind nearest to `point`, with no limit, as
    /// (squared distance, node): ties to the first in the comparison's order.
    fn nearest_any(
        &self,
        id: Option<usize>,
        point: Point3,
        best: &mut Option<(f64, usize, usize)>,
    ) {
        let Some(node) = id.and_then(|id| self.nodes.get(id)) else {
            return;
        };
        let to_box = Vec3::new(
            (node.min.x - point.x).max(0.0).max(point.x - node.max.x),
            (node.min.y - point.y).max(0.0).max(point.y - node.max.y),
            (node.min.z - point.z).max(0.0).max(point.z - node.max.z),
        );
        if best.as_ref().is_some_and(|b| to_box.dot(to_box) > b.0) {
            return;
        }
        let offset = point - node.point;
        let squared = offset.dot(offset);
        if best.as_ref().is_none_or(|b| {
            squared < b.0 || (squared.total_cmp(&b.0).is_eq() && node.original < b.1)
        }) {
            *best = Some((squared, node.original, id.unwrap_or(0)));
        }
        let delta = coordinate(point, node.axis) - coordinate(node.point, node.axis);
        let (near, far) = if delta <= 0.0 {
            (node.left, node.right)
        } else {
            (node.right, node.left)
        };
        self.nearest_any(near, point, best);
        if best.as_ref().is_none_or(|b| delta * delta <= b.0) {
            self.nearest_any(far, point, best);
        }
    }

    /// The final comparison's value nearest to each triangle of `mesh`, in
    /// triangle order, sorted into bands. Each triangle's value depends on
    /// that triangle alone, so the triangles are shared out over the job's
    /// threads in fixed chunks and the values are the same at any thread
    /// count (NFR-06).
    pub(crate) fn bands(
        &self,
        mesh: &TriangleMesh,
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
        Ok(deviation_bands_with(&values, &unreachable, self.cell))
    }

    /// The value of the counted sample nearest to the centroid of
    /// `triangle` within the search, and whether its material left is
    /// enclosed. Beyond the search, the face stands for the material behind
    /// it: the sample of any kind nearest to the point one search distance
    /// behind the face, into the stock, when that is a counted part sample
    /// whose value is only a bound (stock past an end that was not faced:
    /// at least that much is left there) or whose material left is enclosed
    /// (a bore full of stock behind its cap at the part's end, however thin
    /// its wall) (issue 478). Else none: not measured.
    fn value_of(&self, mesh: &TriangleMesh, triangle: usize) -> Result<Projected, AppError> {
        let [a, b, c] = mesh.triangle(triangle).ok_or_else(|| {
            AppError::Playback("the stock surface has an invalid triangle".to_owned())
        })?;
        let point = Point3::new(
            a.x / 3.0 + b.x / 3.0 + c.x / 3.0,
            a.y / 3.0 + b.y / 3.0 + c.y / 3.0,
            a.z / 3.0 + b.z / 3.0 + c.z / 3.0,
        );
        let enclosed_of = |original: usize| {
            self.classes
                .as_ref()
                .and_then(|c| c.get(original))
                .is_some_and(|c| *c == ReachClass::Enclosed)
        };
        let mut best = None;
        self.nearest(self.root, point, &mut best);
        if let Some((_, original, value)) = best {
            return Ok((Some(value), enclosed_of(original)));
        }
        // One search distance behind the face, into the stock.
        let normal = (b - a).cross(c - a);
        let length = normal.length();
        let behind = if length.is_finite() && length > 0.0 {
            point + normal * (-self.limit_squared.sqrt() / length)
        } else {
            point
        };
        let mut any = None;
        self.nearest_any(self.root, behind, &mut any);
        Ok(any
            .and_then(|(_, _, id)| self.nodes.get(id))
            .filter(|node| node.part_bound || node.enclosed)
            .map_or((None, false), |node| (Some(node.value), node.enclosed)))
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
            vertices: [-0.8, 0.8, 0.0, 10.0]
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
            .bands(&mesh, &JobContext::new(&cancel, &NoProgress))
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
            let bands = pool.install(|| samples.bands(&mesh, &context)).unwrap();
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
            let mut actual = None;
            index.nearest(index.root, query, &mut actual);
            assert_eq!(actual, reference);
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
            .bands(&mesh, &JobContext::new(&cancel, &NoProgress))
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
            .bands(&mesh, &JobContext::new(&cancel, &NoProgress))
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
            .bands(&mesh, &JobContext::new(&cancel, &NoProgress))
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
            .bands(&mesh, &JobContext::new(&cancel, &NoProgress))
            .unwrap();
        assert_eq!(bands.values[0], None);
        // The same face turned to face -X has the open end behind it, the
        // rim nearest: not measured.
        let flipped = TriangleMesh {
            vertices: mesh.vertices.clone(),
            triangles: vec![[0, 2, 1]],
        };
        let bands = samples
            .bands(&flipped, &JobContext::new(&cancel, &NoProgress))
            .unwrap();
        assert_eq!(bands.values[0], None);
    }
}
