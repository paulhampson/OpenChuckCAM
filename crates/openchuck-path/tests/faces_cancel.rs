// SPDX-License-Identifier: AGPL-3.0-or-later
//! The search for faces across the rotary axis, which helical operations
//! turn at once each, can be cancelled and shows progress (UI-09, NFR-04;
//! with OP-02, OP-12, GC-07, NFR-06; issue #400).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

mod support;
use support::{generate, machine, tags, tool};

use std::collections::BTreeMap;
use std::sync::Mutex;

use openchuck_path::params::names;
use openchuck_path::stock::AnalyticStock;
use openchuck_path::{
    HelicalFinishing, MeshSurface, ParameterValue, PathError, StrategyInput, rotary::RotaryMapping,
};
use openchuck_testkit::parts::revolve;
use openchuck_types::job::{CancelToken, JobContext, Progress, ProgressSink};
use openchuck_types::machine::Clearance;
use openchuck_types::tool::ToolShape;
use openchuck_types::units::Millimetres;

/// A stepped shaft with a face across the axis at X 10 and one at X 25.
const SHAFT: [(f64, f64); 6] = [
    (0.0, 6.0),
    (10.0, 6.0),
    (10.0, 10.0),
    (25.0, 10.0),
    (25.0, 7.0),
    (40.0, 7.0),
];

const STAGE: &str = "finding faces across the axis";

fn values() -> BTreeMap<String, ParameterValue> {
    [
        (names::STEPOVER, 2.2225),
        (names::FEED, 600.0),
        (names::PLUNGE_FEED, 150.0),
        (names::TOLERANCE, 0.01),
    ]
    .into_iter()
    .map(|(k, x)| (k.to_owned(), ParameterValue::Number(x)))
    .collect()
}

/// Runs helical finishing of the shaft under `sink` and `cancel`.
fn run_with(
    sink: &dyn ProgressSink,
    cancel: &CancelToken,
) -> Result<openchuck_path::GeneratedPath, PathError> {
    let surface = MeshSurface::new(&revolve(&SHAFT, 720)).unwrap();
    run_on(&surface, 42.0, sink, cancel)
}

/// [`run_with`] on `surface`, with stock to `x_end`.
fn run_on(
    surface: &dyn openchuck_path::SurfaceSource,
    x_end: f64,
    sink: &dyn ProgressSink,
    cancel: &CancelToken,
) -> Result<openchuck_path::GeneratedPath, PathError> {
    let stock = AnalyticStock::Cylinder {
        radius: 11.0,
        x_start: -2.0,
        x_end,
    };
    let profile = tool(ToolShape::FlatEnd, 3.175).profile().unwrap();
    let tag = tags();
    let m = machine();
    let input = StrategyInput {
        surface,
        stock: &stock,
        collision: &openchuck_path::stock::AcceptAll,
        tool: &profile,
        rotary: RotaryMapping::of_profile(&m),
        clearance: Clearance {
            radius: Millimetres(25.0),
            safe_z: Millimetres(30.0),
        },
        max_feeds: m.max_feeds,
        tags: &tag,
        facing: None,
        keep_out: &[],
        shank: None,
    };
    let context = JobContext::new(cancel, sink);
    generate(&HelicalFinishing, &input, &values(), &context)
}

/// Records every report.
#[derive(Default)]
struct Reports(Mutex<Vec<Progress>>);

impl ProgressSink for Reports {
    fn report(&self, progress: Progress) {
        self.0.lock().unwrap().push(progress);
    }
}

impl Reports {
    fn face_search(&self) -> Vec<Progress> {
        self.0
            .lock()
            .unwrap()
            .iter()
            .filter(|p| p.stage == STAGE)
            .cloned()
            .collect()
    }
}

/// Cancels the job once the face search has counted `after` lines of A.
struct CancelAfter<'a> {
    token: &'a CancelToken,
    after: u64,
    reports: Reports,
}

impl ProgressSink for CancelAfter<'_> {
    fn report(&self, progress: Progress) {
        if progress.stage == STAGE && progress.done >= self.after {
            self.token.cancel();
        }
        self.reports.report(progress);
    }
}

#[test]
fn req_ui_09_face_search_counts_lines_of_a_and_only_goes_up() {
    let reports = Reports::default();
    run_with(&reports, &CancelToken::new()).unwrap();
    let search = reports.face_search();
    let total = search.first().and_then(|p| p.total).unwrap();
    // More than the one report at the start, and one total throughout.
    assert!(search.len() > 2, "{} reports", search.len());
    let mut done = 0;
    for p in &search {
        assert_eq!(p.total, Some(total));
        assert!(p.done >= done, "went back from {done} to {}", p.done);
        done = p.done;
    }
    assert_eq!(done, total, "the last report is the whole search");
}

#[test]
fn req_ui_09_face_search_stops_when_cancelled_during_the_search() {
    let token = CancelToken::new();
    let sink = CancelAfter {
        token: &token,
        after: 2,
        reports: Reports::default(),
    };
    let result = run_with(&sink, &token);
    assert!(matches!(result, Err(PathError::Cancelled)), "{result:?}");
    // It stopped part way: the search did not count every line.
    let search = sink.reports.face_search();
    let total = search.first().and_then(|p| p.total).unwrap();
    let done = search.iter().map(|p| p.done).max().unwrap();
    assert!(
        done < total,
        "counted {done} of {total} lines after the cancel"
    );
}

#[test]
fn req_ui_09_a_cancel_before_the_search_does_no_work() {
    let token = CancelToken::new();
    token.cancel();
    let reports = Reports::default();
    let result = run_with(&reports, &token);
    assert!(matches!(result, Err(PathError::Cancelled)), "{result:?}");
    assert!(reports.face_search().iter().all(|p| p.done == 0));
}

#[test]
fn req_gc_07_req_nfr_06_face_search_is_identical_at_1_2_and_n_threads() {
    let runs = openchuck_testkit::determinism::check(|| {
        let reports = Reports::default();
        let path = run_with(&reports, &CancelToken::new()).unwrap();
        format!("{:?}", path.group).into_bytes()
    })
    .unwrap();
    assert_eq!(runs.len(), 4);
}

/// A surface that cancels the job at its `at`th drop of the tool.
#[derive(Debug)]
struct CancelAtDrop<'a> {
    surface: &'a MeshSurface,
    at: usize,
    cancel: &'a CancelToken,
    drops: std::sync::atomic::AtomicUsize,
}

impl openchuck_path::SurfaceSource for CancelAtDrop<'_> {
    fn bounds(&self) -> openchuck_types::geometry::Aabb {
        self.surface.bounds()
    }
    fn project(
        &self,
        point: openchuck_types::geometry::Point3,
    ) -> Option<openchuck_path::SurfacePoint> {
        self.surface.project(point)
    }
    fn cast_ray(
        &self,
        origin: openchuck_types::geometry::Point3,
        direction: openchuck_types::geometry::UnitVec3,
        max_distance: f64,
    ) -> Option<openchuck_path::RayHit> {
        self.surface.cast_ray(origin, direction, max_distance)
    }
    fn tessellation(&self) -> &openchuck_types::mesh::TriangleMesh {
        self.surface.tessellation()
    }
    fn drop_tool(
        &self,
        profile: &openchuck_types::tool::RevolvedProfile,
        line: &openchuck_path::ToolLine,
    ) -> Option<openchuck_path::Contact> {
        let n = self.drops.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
        if n == self.at {
            self.cancel.cancel();
        }
        self.surface.drop_tool(profile, line)
    }
}

#[test]
fn req_ui_09_a_long_line_of_a_is_cancelled_part_way_along() {
    // A shaft 200 mm long has 400 samples along each line of A, so a cancel
    // during the first 256 samples is seen inside it, before the line is counted.
    let long = [(0.0, 6.0), (100.0, 6.0), (100.0, 10.0), (200.0, 10.0)];
    let mesh = MeshSurface::new(&revolve(&long, 360)).unwrap();
    let token = CancelToken::new();
    let surface = CancelAtDrop {
        surface: &mesh,
        at: 100,
        cancel: &token,
        drops: std::sync::atomic::AtomicUsize::new(0),
    };
    let reports = Reports::default();
    let result = run_on(&surface, 202.0, &reports, &token);
    assert!(matches!(result, Err(PathError::Cancelled)), "{result:?}");
    // Stopped inside the first line: no whole line was counted, and the
    // drops stopped soon after the cancel (the flag is looked at every 256 samples).
    assert!(reports.face_search().iter().all(|p| p.done == 0));
    let drops = surface.drops.load(std::sync::atomic::Ordering::SeqCst);
    // The next look at the flag is after 256 samples of the line.
    assert!((100..=256).contains(&drops), "{drops} drops");
}

/// A surface that counts the tool drops made after the face search has
/// counted its last line of A, and cancels the job at the first of them
/// when `cancel_at_first` is set.
#[derive(Debug)]
struct AfterTheLines<'a> {
    surface: &'a MeshSurface,
    armed: &'a std::sync::atomic::AtomicBool,
    cancel: &'a CancelToken,
    cancel_at_first: bool,
    after: std::sync::atomic::AtomicUsize,
}

impl openchuck_path::SurfaceSource for AfterTheLines<'_> {
    fn bounds(&self) -> openchuck_types::geometry::Aabb {
        self.surface.bounds()
    }
    fn project(
        &self,
        point: openchuck_types::geometry::Point3,
    ) -> Option<openchuck_path::SurfacePoint> {
        self.surface.project(point)
    }
    fn cast_ray(
        &self,
        origin: openchuck_types::geometry::Point3,
        direction: openchuck_types::geometry::UnitVec3,
        max_distance: f64,
    ) -> Option<openchuck_path::RayHit> {
        self.surface.cast_ray(origin, direction, max_distance)
    }
    fn tessellation(&self) -> &openchuck_types::mesh::TriangleMesh {
        self.surface.tessellation()
    }
    fn drop_tool(
        &self,
        profile: &openchuck_types::tool::RevolvedProfile,
        line: &openchuck_path::ToolLine,
    ) -> Option<openchuck_path::Contact> {
        use std::sync::atomic::Ordering::SeqCst;
        if self.armed.load(SeqCst) {
            let n = self.after.fetch_add(1, SeqCst) + 1;
            if n == 1 && self.cancel_at_first {
                self.cancel.cancel();
            }
        }
        self.surface.drop_tool(profile, line)
    }
}

/// Arms `armed` when the face search has counted all its lines of A.
struct ArmAtTheEnd<'a>(&'a std::sync::atomic::AtomicBool);

impl ProgressSink for ArmAtTheEnd<'_> {
    fn report(&self, p: Progress) {
        if p.stage == STAGE && Some(p.done) == p.total && p.done > 0 {
            self.0.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }
}

/// The drops made after the lines of A, for a run that cancels at the first
/// of them or not.
fn drops_after_the_lines(cancel_at_first: bool) -> (usize, Result<(), PathError>) {
    let mesh = MeshSurface::new(&revolve(&SHAFT, 720)).unwrap();
    let token = CancelToken::new();
    let armed = std::sync::atomic::AtomicBool::new(false);
    let surface = AfterTheLines {
        surface: &mesh,
        armed: &armed,
        cancel: &token,
        cancel_at_first,
        after: std::sync::atomic::AtomicUsize::new(0),
    };
    let sink = ArmAtTheEnd(&armed);
    let result = run_on(&surface, 42.0, &sink, &token).map(|_| ());
    (
        surface.after.load(std::sync::atomic::Ordering::SeqCst),
        result,
    )
}

#[test]
fn req_ui_09_a_cancel_during_the_free_end_search_stops_its_remaining_lines() {
    // The free ends of the shaft are searched for after the lines of A, one
    // bisection per line. A cancel that arrives at the first of them stops
    // the rest: without the check in each bisection they would all run
    // before the final look at the flag.
    let (whole, result) = drops_after_the_lines(false);
    assert!(result.is_ok(), "{result:?}");
    assert!(whole > 200, "the free-end search made only {whole} drops");
    let (cut, result) = drops_after_the_lines(true);
    assert!(matches!(result, Err(PathError::Cancelled)), "{result:?}");
    // Only the bisection under way finishes (about 25 drops); the rest of
    // the lines, over a thousand drops between them, do not run.
    assert!(
        cut <= 100,
        "{cut} drops after the cancel, {whole} in a whole run"
    );
}
