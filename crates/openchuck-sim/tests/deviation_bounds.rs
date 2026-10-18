// SPDX-License-Identifier: AGPL-3.0-or-later
//! The comparison with the target (SIM-05) stays bounded by the part's
//! surface area and the cell, whatever the shape of its triangles (NFR-07).
//!
//! A mesh of long thin triangles, such as the axial strips of a turned part
//! or the blades of the owner's blisk (OI-317), once gave each triangle the
//! square of its longest edge in cells as samples: 74 million for the blisk
//! at 0.1 mm, which ran a simulation out of memory. The cases here are
//! proportionate: the same strip shape, small enough to run in a second.
//!
//! This binary counts allocations with its own global allocator, so its tests
//! measure memory directly; it holds only these tests.

// The counting allocator below is the only unsafe code: it forwards each
// call to the system allocator unchanged.
#![allow(unsafe_code)]
// Test code may unwrap, index and panic (NFR-07 applies to library code).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use openchuck_sim::{DeviationOptions, DexelStock, MAX_DEVIATION_SAMPLES, compare};
use openchuck_types::geometry::Point3;
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::stock::Stock;
use openchuck_types::units::Millimetres;

/// The system allocator, counting the bytes held now and the most held.
struct Counting;

static HELD: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

// SAFETY: every call forwards to the system allocator unchanged; the
// counters only observe the sizes.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the caller's layout is passed through.
        let p = unsafe { System.alloc(layout) };
        if !p.is_null() {
            let now = HELD.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
            PEAK.fetch_max(now, Ordering::Relaxed);
        }
        p
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: the pointer came from `alloc` with this layout.
        unsafe { System.dealloc(ptr, layout) };
        HELD.fetch_sub(layout.size(), Ordering::Relaxed);
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// The tests in this binary measure memory one at a time.
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

/// Runs `f` and returns its result with the most bytes it held at once
/// beyond what was held before it.
fn peak_of<T>(f: impl FnOnce() -> T) -> (T, usize) {
    let base = HELD.load(Ordering::Relaxed);
    PEAK.store(base, Ordering::Relaxed);
    let out = f();
    (out, PEAK.load(Ordering::Relaxed).saturating_sub(base))
}

/// A closed cylinder about the X axis, from X = 0 to `length`, of `segments`
/// axial strips (two long thin triangles each) and fan caps, outward.
fn cylinder(radius: f64, length: f64, segments: u32) -> TriangleMesh {
    let mut vertices = Vec::new();
    for k in 0..segments {
        let t = std::f64::consts::TAU * f64::from(k) / f64::from(segments);
        let (y, z) = (radius * t.cos(), radius * t.sin());
        vertices.push(Point3::new(0.0, y, z));
        vertices.push(Point3::new(length, y, z));
    }
    let (c0, c1) = (2 * segments, 2 * segments + 1);
    vertices.push(Point3::new(0.0, 0.0, 0.0));
    vertices.push(Point3::new(length, 0.0, 0.0));
    let mut triangles = Vec::new();
    for k in 0..segments {
        let n = (k + 1) % segments;
        let (a0, a1, b0, b1) = (2 * k, 2 * k + 1, 2 * n, 2 * n + 1);
        triangles.push([a0, b0, b1]);
        triangles.push([a0, b1, a1]);
        triangles.push([c0, b0, a0]);
        triangles.push([c1, a1, b1]);
    }
    TriangleMesh {
        vertices,
        triangles,
    }
}

fn area(mesh: &TriangleMesh) -> f64 {
    (0..mesh.triangles.len())
        .map(|t| {
            let [a, b, c] = mesh.triangle(t).unwrap();
            0.5 * (b - a).cross(c - a).length()
        })
        .sum()
}

fn stock(radius: f64, length: f64, cell: f64) -> DexelStock {
    let stock = Stock::Cylinder {
        diameter: Millimetres(2.0 * radius),
        length: Millimetres(length),
        x_start: Millimetres(-1.0),
    };
    DexelStock::from_stock(&stock, None, cell).unwrap()
}

#[test]
fn req_sim_05_req_nfr_07_samples_and_memory_follow_the_area_not_the_longest_edge() {
    let _one = ONE_AT_A_TIME
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    // Strips 30 mm long and 0.35 mm wide, caps of 10 mm spokes, at a 0.5 mm
    // cell: the longest edge gave 60 x 60 samples to each strip, 1.3 million
    // in all, where the area needs about 15 thousand.
    let (radius, length, cell) = (10.0, 30.0, 0.5);
    let mesh = cylinder(radius, length, 180);
    let stock = stock(radius + 1.0, length + 2.0, cell);
    let options = DeviationOptions::for_cell(cell);
    let started = std::time::Instant::now();
    let (deviation, peak) = peak_of(|| compare(&stock, &mesh, &options).unwrap());
    let samples = deviation.points.len();
    // One sample covers a sub-triangle of up to half a cell squared (two per
    // cell square); a thin triangle also needs about one per cell of its
    // length. The bound allows twice that.
    let spokes_and_strips: f64 = 180.0 * (2.0 * length + 2.0 * radius) / cell;
    let bound = 2.0 * (2.0 * area(&mesh) / (cell * cell) + spokes_and_strips);
    println!(
        "{samples} samples (bound {bound:.0}), peak {peak} bytes, {:?}",
        started.elapsed()
    );
    assert!(
        (samples as f64) <= bound,
        "{samples} samples, bound {bound}"
    );
    // The samples are the memory: each is held once, as its result (a
    // point, its normal and value), measured in place.
    let per_sample = std::mem::size_of::<openchuck_sim::DeviationPoint>();
    assert!(
        peak <= per_sample * samples + (1 << 20),
        "{peak} bytes for {samples} samples"
    );
}

#[test]
fn req_sim_05_a_thin_triangle_is_sampled_no_further_apart_than_the_cell() {
    let _one = ONE_AT_A_TIME
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    // One strip 40 mm long and 0.02 mm wide, on the surface of the stock.
    let cell = 0.1;
    let (a, b, c) = (
        Point3::new(0.0, 0.0, 10.0),
        Point3::new(40.0, 0.0, 10.0),
        Point3::new(13.0, 0.02, 10.0),
    );
    let mesh = TriangleMesh {
        vertices: vec![a, b, c],
        triangles: vec![[0, 1, 2]],
    };
    let deviation = compare(
        &stock(11.0, 42.0, cell),
        &mesh,
        &DeviationOptions::for_cell(cell),
    )
    .unwrap();
    let points: Vec<Point3> = deviation.points.iter().map(|p| p.point).collect();
    println!("{} samples", points.len());
    // About one per cell of each edge, not 400 x 400.
    assert!(points.len() <= 3 * 400, "{}", points.len());
    // Every point of the triangle is within a cell of a sample.
    for i in 0..=400 {
        for j in 0..=4 {
            let (u, v) = (f64::from(i) / 400.0, f64::from(j) / 4.0);
            if u + v > 1.0 {
                continue;
            }
            let p = a + (b - a) * u + (c - a) * v;
            let near = points
                .iter()
                .map(|q| q.distance(p))
                .fold(f64::INFINITY, f64::min);
            assert!(near <= cell, "{p:?} is {near} from the nearest sample");
        }
    }
}

#[test]
fn req_sim_05_well_shaped_triangles_keep_their_subdivision() {
    let _one = ONE_AT_A_TIME
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    // The faces of a 10 mm cube are right isosceles triangles: each keeps the
    // n x n subdivision of its longest edge (n = 142 at 0.1 mm), so results
    // on well-shaped meshes are unchanged.
    let corner = |i: u32| {
        Point3::new(
            f64::from(i & 1) * 10.0,
            f64::from((i >> 1) & 1) * 10.0 - 5.0,
            f64::from((i >> 2) & 1) * 10.0 - 5.0,
        )
    };
    let mesh = TriangleMesh {
        vertices: (0..8).map(corner).collect(),
        triangles: vec![
            [0, 2, 1],
            [1, 2, 3],
            [4, 5, 6],
            [5, 7, 6],
            [0, 1, 4],
            [1, 5, 4],
            [2, 6, 3],
            [3, 6, 7],
            [0, 4, 2],
            [2, 4, 6],
            [1, 3, 5],
            [3, 7, 5],
        ],
    };
    let deviation = compare(
        &stock(10.0, 14.0, 0.1),
        &mesh,
        &DeviationOptions::for_cell(0.1),
    )
    .unwrap();
    assert_eq!(deviation.points.len(), 12 * 142 * 142);
}

#[test]
fn req_nfr_07_req_sim_07_a_surface_too_fine_to_compare_is_refused_with_a_clear_error() {
    let _one = ONE_AT_A_TIME
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let stock = stock(11.0, 42.0, 1.0);
    // A well-shaped triangle 2 m across and a thin one 20 m long, at the
    // finest cell: billions of samples each. Both are refused before any
    // is taken, quickly and without the memory.
    let (o, x, y) = (
        Point3::new(0.0, 0.0, 0.0),
        Point3::new(2000.0, 0.0, 0.0),
        Point3::new(0.0, 2000.0, 0.0),
    );
    let thin = Point3::new(20_000.0, 0.0, 0.0);
    for corners in [[o, x, y], [o, thin, Point3::new(9000.0, 10.0, 0.0)]] {
        let mesh = TriangleMesh {
            vertices: corners.to_vec(),
            triangles: vec![[0, 1, 2]],
        };
        let started = std::time::Instant::now();
        let (result, peak) = peak_of(|| compare(&stock, &mesh, &DeviationOptions::for_cell(0.025)));
        let message = result.unwrap_err().to_string();
        println!("{message} ({peak} bytes, {:?})", started.elapsed());
        assert!(message.contains("simulation.cell"), "{message}");
        assert!(message.contains("coarser resolution"), "{message}");
        assert!(
            message.contains(&MAX_DEVIATION_SAMPLES.to_string()),
            "{message}"
        );
        assert!(peak < 1 << 20, "{peak} bytes");
        assert!(started.elapsed().as_secs() < 10);
    }
}

#[test]
fn req_nfr_06_thin_triangle_samples_are_identical_at_1_2_and_n_threads() {
    let _one = ONE_AT_A_TIME
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let (radius, length, cell) = (10.0, 30.0, 0.5);
    let mesh = cylinder(radius, length, 180);
    let stock = stock(radius + 1.0, length + 2.0, cell);
    let run = |threads: usize| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap()
            .install(|| compare(&stock, &mesh, &DeviationOptions::for_cell(cell)).unwrap())
    };
    let one = run(1);
    for threads in [2, 7] {
        assert_eq!(run(threads), one, "{threads} threads");
    }
}
