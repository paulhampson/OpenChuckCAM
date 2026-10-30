// SPDX-License-Identifier: AGPL-3.0-or-later
//! Display-only projection of final simulation samples onto playback stock.

use crate::AppError;
use crate::pathview::{DeviationBands, deviation_bands};
use openchuck_geom::pick::MeshPicker;
use openchuck_project::id::{ModelId, SetupId};
use openchuck_sim::DeviationPoint;
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
        context: &JobContext<'_>,
    ) -> Result<Self, AppError> {
        context.check_cancelled()?;
        // The picker is built on one thread while the others colour the
        // triangles; neither reads what the other makes.
        let (bands, picker) = rayon::join(
            || samples.map(|s| s.bands(&mesh, context)).transpose(),
            || {
                (!mesh.triangles.is_empty())
                    .then(|| MeshPicker::new(&mesh))
                    .transpose()
            },
        );
        let (bands, picker) = (bands?, picker?);
        context.check_cancelled()?;
        Ok(Self {
            mesh,
            setup,
            model,
            bands,
            picker,
        })
    }

    /// First hit on the actual stock, in its workpiece frame. The triangle
    /// ID indexes both its drawn colour and its projected tooltip value.
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
    min: Point3,
    max: Point3,
    original: usize,
    axis: usize,
    left: Option<usize>,
    right: Option<usize>,
}

/// A balanced spatial index made once per setup, off the UI thread.
pub(crate) struct SurfaceSamples {
    nodes: Vec<Node>,
    root: Option<usize>,
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
    pub(crate) fn new(points: &[DeviationPoint], cell: f64) -> Self {
        let mut points: Vec<_> = points
            .iter()
            .copied()
            .enumerate()
            .filter(|(_, p)| p.counts() && p.point.is_finite() && p.value.is_finite())
            .collect();
        let limit = openchuck_sim::DeviationOptions::for_cell(cell).max_distance + cell;
        let mut index = Self {
            nodes: Vec::with_capacity(points.len()),
            root: None,
            cell,
            limit_squared: limit * limit,
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
        self.nodes.push(Node {
            point: point.point,
            value: (point.value, point.at_least),
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
        if let Some(node) = self.nodes.get_mut(id) {
            node.left = left;
            node.right = right;
            node.min = min;
            node.max = max;
        }
        Some(id)
    }

    fn nearest(
        &self,
        id: Option<usize>,
        point: Point3,
        best: &mut Option<(f64, usize, (f64, bool))>,
    ) {
        let Some(node) = id.and_then(|id| self.nodes.get(id)) else {
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
        if squared <= self.limit_squared
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
        let chunks: Vec<Vec<Option<(f64, bool)>>> = (0..chunks)
            .into_par_iter()
            .map(|chunk| {
                context.check_cancelled()?;
                let end = ((chunk + 1) * CHUNK).min(mesh.triangles.len());
                (chunk * CHUNK..end)
                    .map(|triangle| self.value_of(mesh, triangle))
                    .collect()
            })
            .collect::<Result<_, AppError>>()?;
        let values: Vec<_> = chunks.into_iter().flatten().collect();
        Ok(deviation_bands(&values, self.cell))
    }

    /// The value of the sample nearest to the centroid of `triangle`, if
    /// one is near enough.
    fn value_of(
        &self,
        mesh: &TriangleMesh,
        triangle: usize,
    ) -> Result<Option<(f64, bool)>, AppError> {
        let [a, b, c] = mesh.triangle(triangle).ok_or_else(|| {
            AppError::Playback("the stock surface has an invalid triangle".to_owned())
        })?;
        let point = Point3::new(
            a.x / 3.0 + b.x / 3.0 + c.x / 3.0,
            a.y / 3.0 + b.y / 3.0 + c.y / 3.0,
            a.z / 3.0 + b.z / 3.0 + c.z / 3.0,
        );
        let mut best = None;
        self.nearest(self.root, point, &mut best);
        Ok(best.map(|b| b.2))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use openchuck_sim::SampleOn;
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
        // The rule, one triangle after another: the nearest sample to the
        // triangle's centroid, as the one-thread mapping gave it.
        let expected: Vec<_> = (0..mesh.triangles.len())
            .map(|t| {
                let [a, b, c] = mesh.triangle(t).unwrap();
                let point = Point3::new(
                    a.x / 3.0 + b.x / 3.0 + c.x / 3.0,
                    a.y / 3.0 + b.y / 3.0 + c.y / 3.0,
                    a.z / 3.0 + b.z / 3.0 + c.z / 3.0,
                );
                let mut best = None;
                samples.nearest(samples.root, point, &mut best);
                best.map(|b| b.2)
            })
            .collect();
        assert!(expected.iter().any(Option::is_none) && expected.iter().any(Option::is_some));
        let expected = deviation_bands(&expected, 0.1);
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
}
