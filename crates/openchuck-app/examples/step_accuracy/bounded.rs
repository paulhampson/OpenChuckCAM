// SPDX-License-Identifier: AGPL-3.0-or-later
//! M2 with the bounded checker (issue #346; decision 0209 item 4.1;
//! decision 0211): every station's clearance against the exact faces and
//! against the tessellation the tool was dropped on is an interval proven to
//! hold the exact value (`openchuck_testkit::contact`), and M2 qualifies only
//! when every interval is narrower than its tolerance. A station whose bound
//! cannot be closed fails the measurement; raw observations are kept.

use std::fmt::Write as _;

use openchuck_testkit::contact::{Bounded, ConvexTool, Geometry, Settings};
use openchuck_types::geometry::{Point3, Vec3};
use rayon::prelude::*;

/// The widest interval accepted at a station, mm: half of decision 0209's
/// "under 0.001 mm", leaving room for the checker's rounding slack.
pub const TOLERANCE: f64 = 0.0005;

/// Clearances above this are not resolved, mm: enough to measure a gap over
/// the largest stock to leave, as the sampled M2 did.
pub const MARGIN: f64 = 1.0;

/// The most pieces examined for one station before it fails.
pub const BUDGET: usize = 4_000_000;

/// The checker's settings for M2.
pub fn settings() -> Settings {
    Settings {
        margin: MARGIN,
        tolerance: TOLERANCE,
        budget: BUDGET,
    }
}

/// Where a station sits on its move.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    /// A fed pose the strategy produced.
    Pose,
    /// The quarter, middle or three-quarter point of a fed move.
    Quarter(u8),
}

/// One station to check.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stop {
    /// The tool tip, model frame.
    pub tip: Point3,
    /// The tool axis, tip to spindle.
    pub axis: Vec3,
    /// Where on its move.
    pub place: Place,
    /// Whether the move is a cut (not a lead or link).
    pub cut: bool,
}

/// One station's bounded clearances.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Observation {
    /// The station.
    pub stop: Stop,
    /// Against the exact faces.
    pub exact: Bounded,
    /// Against the tessellation the tool was dropped on.
    pub mesh: Bounded,
}

/// Checks every station; fails on the first (in station order) whose
/// interval cannot be closed, naming it.
pub fn observe(
    exact: &Geometry,
    mesh: &Geometry,
    tool: &ConvexTool,
    stops: &[Stop],
    settings: &Settings,
) -> Result<Vec<Observation>, String> {
    let results: Vec<Result<Observation, String>> = stops
        .par_iter()
        .enumerate()
        .map(|(i, stop)| {
            let exact = exact
                .clearance(tool, stop.tip, stop.axis, settings)
                .map_err(|e| format!("station {i} ({stop:?}), exact faces: {e}"))?;
            let mesh = mesh
                .clearance(tool, stop.tip, stop.axis, settings)
                .map_err(|e| format!("station {i} ({stop:?}), tessellation: {e}"))?;
            Ok(Observation {
                stop: *stop,
                exact,
                mesh,
            })
        })
        .collect();
    results.into_iter().collect()
}

/// M2 for one operation, from its observations. Each reading is given as
/// found and as proven: the found value is attained at a point of the
/// geometry, the proven one bounds the exact value.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Summary {
    /// Stations checked.
    pub stations: usize,
    /// The deepest cut found into the exact faces, mm (0 when none).
    pub deepest: f64,
    /// The deepest cut the bounds allow, mm.
    pub deepest_bound: f64,
    /// The deepest found at the strategy's own poses.
    pub deepest_at_poses: f64,
    /// Contact stations: stations of cutting moves that touch the
    /// tessellation at the stock to leave, within 0.002 mm (as the sampled
    /// M2 counted them).
    pub contacts: usize,
    /// The largest gap found above the exact faces beyond the stock to
    /// leave at a contact station, mm. Found values are attained, so the
    /// exact gap is at most this and at least this less `proven_error`.
    pub gap: f64,
    /// At contact stations, how far the exact faces lie inside the
    /// tessellation the tool touched (found), mm.
    pub error_in: f64,
    /// And outside it.
    pub error_out: f64,
    /// The most the exact and tessellation clearances can differ at a
    /// contact station, from both intervals: the tessellation's proven
    /// share of the cut there, mm (verification plan 4.1 allows 0.005).
    pub drop_bound: f64,
    /// The widest interval of any station, exact or tessellation: the
    /// proven bound on what the checker adds, mm.
    pub proven_error: f64,
}

/// Aggregates the observations of an operation leaving `leave` mm.
pub fn summarise(observations: &[Observation], leave: f64) -> Summary {
    let mut s = Summary {
        stations: observations.len(),
        ..Summary::default()
    };
    for o in observations {
        let (e, t) = (o.exact, o.mesh);
        s.proven_error = s.proven_error.max(e.error()).max(t.error());
        s.deepest = s.deepest.max(-e.upper);
        s.deepest_bound = s.deepest_bound.max(-e.lower);
        if o.stop.place == Place::Pose {
            s.deepest_at_poses = s.deepest_at_poses.max(-e.upper);
        }
        if o.stop.cut && (t.upper - leave).abs() <= 0.002 {
            s.contacts += 1;
            s.gap = s.gap.max(e.upper - leave);
            s.error_in = s.error_in.max(t.upper - e.upper);
            s.error_out = s.error_out.max(e.upper - t.upper);
            s.drop_bound = s.drop_bound.max(e.upper - t.lower).max(t.upper - e.lower);
        }
    }
    s
}

/// The raw observations as tab-separated text: one line a station, every
/// number to 1e-9 mm, after `header` (the provenance).
pub fn raw(header: &str, observations: &[Observation]) -> String {
    let mut out = String::new();
    for line in header.lines() {
        let _ = writeln!(out, "# {line}");
    }
    out.push_str(
        "station\tplace\tcut\ttip_x\ttip_y\ttip_z\taxis_x\taxis_y\taxis_z\texact_upper\texact_lower\texact_cells\tmesh_upper\tmesh_lower\tmesh_cells\n",
    );
    for (i, o) in observations.iter().enumerate() {
        let place = match o.stop.place {
            Place::Pose => "pose".to_owned(),
            Place::Quarter(q) => format!("q{q}"),
        };
        let s = o.stop;
        let _ = writeln!(
            out,
            "{i}\t{place}\t{}\t{:.9}\t{:.9}\t{:.9}\t{:.9}\t{:.9}\t{:.9}\t{:.9}\t{:.9}\t{}\t{:.9}\t{:.9}\t{}",
            u8::from(s.cut),
            s.tip.x,
            s.tip.y,
            s.tip.z,
            s.axis.x,
            s.axis.y,
            s.axis.z,
            o.exact.upper,
            o.exact.lower,
            o.exact.cells,
            o.mesh.upper,
            o.mesh.lower,
            o.mesh.cells,
        );
    }
    out
}
