// SPDX-License-Identifier: AGPL-3.0-or-later
//! The tool dropped on a mesh scales across threads and gives the same
//! answer on any of them (NFR-04, NFR-06, GC-07; issue #344, OI-990).
//!
//! STL pencil clean-up on carried stock gained nothing from four threads:
//! every drop on the mesh built its candidate lists in new heap memory, and
//! the threads queued for the allocator's lock (about 5 s of system time a
//! run on the owner's blisk). A drop now keeps its lists on its thread and
//! reuses them, so once a thread has made a drop as large, the next makes no
//! allocation; the answer is the one a fresh thread gives.
//!
//! This binary counts each thread's allocations with its own global
//! allocator, so it holds only these tests and pencil clean-up of a mesh,
//! whose drops they are.

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

mod support;

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use openchuck_path::{
    MeshSurface, ParameterValue, PencilFinishing, RotaryMapping, Strategy, SurfaceSource, ToolLine,
};
use openchuck_testkit::parts::revolve;
use openchuck_types::tool::ToolShape;
use openchuck_types::units::{Degrees, Millimetres};
use rayon::prelude::*;
use support::{Fixture, blisk, machine, run, tool};

thread_local! {
    /// Allocations made by this thread.
    static MADE: Cell<usize> = const { Cell::new(0) };
}

/// The system allocator, counting each thread's allocations.
struct Counting;

// SAFETY: every call forwards to the system allocator unchanged; the
// counter only observes that a call was made.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let _ = MADE.try_with(|m| m.set(m.get() + 1));
        // SAFETY: the caller's layout is passed through.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: the pointer came from `alloc` with this layout.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let _ = MADE.try_with(|m| m.set(m.get() + 1));
        // SAFETY: the pointer came from `alloc` with this layout.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

fn made() -> usize {
    MADE.with(Cell::get)
}

/// The owner's blisk, placed from X 10 on the rotary axis as decision 0211
/// places it, and vertical lines over it: across the blades and the hub at
/// several angles, so the drops meet from a few triangles to a few hundred.
fn blisk_lines() -> (MeshSurface, Vec<ToolLine>) {
    let surface = placed_blisk();
    let rotary = RotaryMapping::of_profile(&machine());
    let mut lines = Vec::new();
    for a in [0.0, 17.0, 36.0, 90.0, 211.0] {
        for i in 0..24 {
            for j in 0..9 {
                lines.push(rotary.tool_line(
                    Millimetres(9.0 + 52.0 * f64::from(i) / 23.0),
                    Degrees(a),
                    Millimetres(-8.0 + 2.0 * f64::from(j)),
                ));
            }
        }
    }
    (surface, lines)
}

/// The owner's blisk, from X 10 to 60 and centred on the rotary axis.
fn placed_blisk() -> MeshSurface {
    let mut mesh = blisk();
    let b = MeshSurface::new(&mesh).unwrap().bounds();
    let shift = openchuck_types::geometry::Vec3::new(
        10.0 - b.min.x,
        -(b.min.y + b.max.y) / 2.0,
        -(b.min.z + b.max.z) / 2.0,
    );
    for v in &mut mesh.vertices {
        *v = *v + shift;
    }
    MeshSurface::new(&mesh).unwrap()
}

/// Every answer, written out in full, so that equal text is equal bits.
fn answers(
    surface: &MeshSurface,
    lines: &[ToolLine],
    profile: &openchuck_types::tool::RevolvedProfile,
) -> Vec<String> {
    lines
        .iter()
        .map(|line| format!("{:?}", surface.drop_tool(profile, line)))
        .collect()
}

#[test]
fn req_nfr_04_a_drop_on_a_mesh_allocates_nothing_once_its_thread_has_made_one_as_large() {
    let (surface, lines) = blisk_lines();
    let profile = tool(ToolShape::FlatEnd, 2.0).profile().unwrap();
    // The thread's first drops may make its lists; the same drops again
    // must not allocate at all.
    let first = answers(&surface, &lines, &profile);
    let mut again = Vec::with_capacity(lines.len());
    let before = made();
    for line in &lines {
        again.push(surface.drop_tool(&profile, line));
    }
    let allocations = made() - before;
    assert_eq!(
        allocations,
        0,
        "{allocations} allocations in {} drops",
        lines.len()
    );
    let again: Vec<String> = again.iter().map(|c| format!("{c:?}")).collect();
    assert_eq!(first, again);
    // The lines meet the part, and some meet many triangles.
    assert!(first.iter().filter(|a| a.starts_with("Some")).count() > lines.len() / 2);
}

#[test]
fn req_nfr_06_req_gc_07_a_drop_on_a_mesh_is_the_same_on_any_thread_and_after_any_other() {
    let (surface, lines) = blisk_lines();
    for (shape, diameter) in [(ToolShape::FlatEnd, 2.0), (ToolShape::BallEnd, 6.0)] {
        let profile = tool(shape.clone(), diameter).profile().unwrap();
        // Each line on a thread of its own, which has made no drop before.
        let fresh: Vec<String> = std::thread::scope(|s| {
            lines
                .chunks(1)
                .map(|line| s.spawn(|| answers(&surface, line, &profile)))
                .collect::<Vec<_>>()
                .into_iter()
                .flat_map(|h| h.join().unwrap())
                .collect()
        });
        // In order, backwards (so each follows drops of other sizes), and on
        // pools of 1, 2 and 4 threads.
        assert_eq!(answers(&surface, &lines, &profile), fresh);
        let reversed: Vec<ToolLine> = lines.iter().rev().copied().collect();
        let mut backwards = answers(&surface, &reversed, &profile);
        backwards.reverse();
        assert_eq!(backwards, fresh);
        for threads in [1, 2, 4] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            let parallel: Vec<String> = pool.install(|| {
                lines
                    .par_iter()
                    .map(|line| format!("{:?}", surface.drop_tool(&profile, line)))
                    .collect()
            });
            assert_eq!(parallel, fresh, "{threads} threads, {shape:?}");
        }
    }
}

#[test]
fn req_nfr_06_req_gc_07_req_op_09_stl_pencil_is_the_same_at_any_thread_count() {
    // Pencil clean-up of a mesh part, as issue #344's STL case: a shaft
    // turned on 180 facets with two inside corners, each a closed crease of
    // 180 edges, so the placements of a crease are split among threads.
    let part = revolve(
        &[
            (0.0, 6.0),
            (12.0, 6.0),
            (12.0, 12.0),
            (24.0, 12.0),
            (24.0, 8.0),
            (36.0, 8.0),
        ],
        180,
    );
    let fixture = Fixture::new(
        MeshSurface::new(&part).unwrap(),
        13.0,
        &tool(ToolShape::FlatEnd, 2.0),
    );
    let values = [("feed", 500.0), ("plunge_feed", 200.0), ("stepover", 1.0)]
        .into_iter()
        .map(|(k, x)| (k.to_owned(), ParameterValue::Number(x)))
        .collect();
    let strategy = PencilFinishing::default();
    let parameters = strategy.schema().resolve(&values).unwrap();
    let passes = |threads: usize| {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        pool.install(|| {
            run(|context| {
                strategy
                    .passes(&fixture.input(), &parameters, context)
                    .unwrap()
            })
        })
    };
    let one = passes(1);
    // Both creases are cut, each in more than one chunk of placements.
    assert_eq!(one.len(), 2);
    assert!(
        one.iter()
            .all(|p| p.points.len() > 2 * openchuck_path::parallel::CHUNK),
        "{:?}",
        one.iter().map(|p| p.points.len()).collect::<Vec<_>>()
    );
    let one = format!("{one:?}");
    for threads in [2, 4] {
        assert!(format!("{:?}", passes(threads)) == one, "{threads} threads");
    }
}

#[test]
fn req_nfr_04_a_thread_does_not_keep_the_lists_of_a_drop_on_very_many_triangles() {
    // A flat 1.4 by 1.4 mm square of 80,000 triangles, all under a 2 mm flat end
    // dropped on its middle: more candidates than a thread keeps between
    // drops, so each such drop makes its lists anew and frees them after.
    let n = 200u32;
    let mut vertices = Vec::new();
    for i in 0..=n {
        for j in 0..=n {
            vertices.push(openchuck_types::geometry::Point3::new(
                -0.7 + 1.4 * f64::from(i) / f64::from(n),
                -0.7 + 1.4 * f64::from(j) / f64::from(n),
                0.0,
            ));
        }
    }
    let at = |i: u32, j: u32| i * (n + 1) + j;
    let mut triangles = Vec::new();
    for i in 0..n {
        for j in 0..n {
            triangles.push([at(i, j), at(i + 1, j), at(i + 1, j + 1)]);
            triangles.push([at(i, j), at(i + 1, j + 1), at(i, j + 1)]);
        }
    }
    let mesh = openchuck_types::mesh::TriangleMesh {
        vertices,
        triangles,
    };
    let surface = MeshSurface::new(&mesh).unwrap();
    let profile = tool(ToolShape::FlatEnd, 2.0).profile().unwrap();
    let line = ToolLine {
        origin: openchuck_types::geometry::Point3::new(0.0, 0.0, 10.0),
        axis: openchuck_types::geometry::UnitVec3::Z,
    };
    std::thread::spawn(move || {
        let first = surface.drop_tool(&profile, &line).unwrap();
        assert!(first.tip.z.abs() < 1e-12, "{first:?}");
        let before = made();
        let again = surface.drop_tool(&profile, &line).unwrap();
        assert!(made() > before, "the lists were kept");
        assert_eq!(format!("{first:?}"), format!("{again:?}"));
    })
    .join()
    .unwrap();
}
