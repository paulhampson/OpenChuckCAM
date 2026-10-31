// SPDX-License-Identifier: AGPL-3.0-or-later
//! Walls in the target (OP-12, OP-13, OP-14, NFR-06; decision record 0050,
//! closing OI-209): where the tool climbs onto a part wider than a tab, or
//! drops from a held region onto a narrower tab, path fitting bisects down
//! to micro-moves and the shared services merge them.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing
)]

mod support;

use std::collections::BTreeMap;
use std::sync::Arc;

use openchuck_path::params::names;
use openchuck_path::rotary::AxisPosition;
use openchuck_path::{
    AcceptAll, AnalyticStock, CompositeSurface, GeneratedPath, HelicalFinishing, MeshSurface,
    ParameterValue, RotaryMapping, Solid, StrategyInput, generate,
};
use openchuck_testkit::gouge::{Outline, Pose, max_gouge};
use openchuck_testkit::parts::{Cone, RadialPart};
use openchuck_types::geometry::{Point3, UnitVec3, Vec3};
use openchuck_types::ir::MoveKind;
use openchuck_types::machine::Clearance;
use openchuck_types::tool::RevolvedProfile;
use openchuck_types::units::{Degrees, Millimetres};
use support::{ball, machine, run, tags};

const FEED: f64 = 600.0;
const TOLERANCE: f64 = 0.01;

fn x_axis() -> UnitVec3 {
    UnitVec3::new(Vec3::new(1.0, 0.0, 0.0)).unwrap()
}

/// The cone of the reference jobs (diameter 20 to 10 over 30 mm).
fn cone() -> Cone {
    Cone {
        start_radius: 10.0,
        end_radius: 5.0,
        length: 30.0,
    }
}

/// A layout like the owner's default tabs (decision 0045 item 3): held 4 mm
/// at the start (as wide as the part's wide end), a frustum tab narrowing
/// from there to 10 mm on the part's 20 mm end (a wall: a climb of 2.5 mm
/// onto the part), a tab of 5 mm on its 10 mm end (a drop of 2.5 mm),
/// widening to 10 mm, and a held region of 20 mm at the end (a climb of 5
/// mm).
fn solids() -> Vec<Solid> {
    vec![
        Solid::cylinder(Point3::new(-12.0, 0.0, 0.0), x_axis(), 4.0, 10.0).unwrap(),
        Solid::frustum(Point3::new(-8.0, 0.0, 0.0), x_axis(), 8.0, 10.0, 5.0).unwrap(),
        Solid::frustum(Point3::new(30.0, 0.0, 0.0), x_axis(), 8.0, 2.5, 5.0).unwrap(),
        Solid::cylinder(Point3::new(38.0, 0.0, 0.0), x_axis(), 2.0, 10.0).unwrap(),
    ]
}

/// The cone with the solids: the exact surface of revolution for the
/// independent checker. A wall is where the radius jumps.
struct Exact {
    part: Cone,
    solids: Vec<Solid>,
}

impl RadialPart for Exact {
    fn name(&self) -> &'static str {
        "cone with tabs and walls"
    }

    fn x_range(&self) -> (f64, f64) {
        (-12.0, 40.0)
    }

    fn radius_at(&self, x: f64, angle: f64) -> f64 {
        let (x0, x1) = self.part.x_range();
        let mut r = if (x0..=x1).contains(&x) {
            self.part.radius_at(x, angle)
        } else {
            0.0
        };
        for s in &self.solids {
            let s0 = s.base().x;
            if (s0..=s0 + s.length()).contains(&x) {
                let k = (x - s0) / s.length();
                r = r.max(s.base_radius() + (s.top_radius() - s.base_radius()) * k);
            }
        }
        r
    }
}

impl Exact {
    /// The walls: at each X where the radius jumps, the annulus between the
    /// radius just before and just after.
    fn walls(&self) -> Vec<(f64, f64, f64)> {
        [-8.0, 0.0, 30.0, 38.0]
            .into_iter()
            .map(|x| {
                let before = self.radius_at(x - 1e-9, 0.0);
                let after = self.radius_at(x + 1e-9, 0.0);
                (x, before.min(after), before.max(after))
            })
            .filter(|(_, lo, hi)| hi - lo > 1e-6)
            .collect()
    }
}

struct Fixture {
    composite: CompositeSurface,
    stock: AnalyticStock,
    profile: RevolvedProfile,
}

impl Fixture {
    fn new() -> Self {
        let model = Arc::new(MeshSurface::new(&cone().mesh(61, 720)).unwrap());
        Self {
            composite: CompositeSurface::new(model, solids()),
            stock: AnalyticStock::Cylinder {
                radius: 10.0,
                x_start: -12.0,
                x_end: 40.0,
            },
            profile: ball(3.0).profile().unwrap(),
        }
    }

    fn path(&self) -> GeneratedPath {
        let machine = machine();
        let tag = tags();
        let input = StrategyInput {
            surface: &self.composite,
            stock: &self.stock,
            collision: &AcceptAll,
            tool: &self.profile,
            rotary: RotaryMapping::of_profile(&machine),
            clearance: Clearance {
                radius: Millimetres(15.0),
                safe_z: Millimetres(25.0),
            },
            max_feeds: machine.max_feeds,
            tags: &tag,
            entry_safety: None,
            facing: None,
            keep_out: &[],
            shank: None,
        };
        let mut values: BTreeMap<String, ParameterValue> = [
            (names::STEPOVER, 2.0),
            (names::FEED, FEED),
            (names::PLUNGE_FEED, 150.0),
            (names::TOLERANCE, TOLERANCE),
        ]
        .into_iter()
        .map(|(k, x)| (k.to_owned(), ParameterValue::Number(x)))
        .collect();
        values.insert("max_angle_step".to_owned(), ParameterValue::Number(5.0));
        run(|ctx| generate(&HelicalFinishing, &input, &values, ctx)).unwrap()
    }
}

/// The tool tip's motion over the part in a move, across the tool axis and
/// along it.
fn motion(rotary: &RotaryMapping, from: (Point3, Degrees), to: (Point3, Degrees)) -> (f64, f64) {
    let d = to.0 - from.0;
    let axis = rotary.tool_axis(to.1).get();
    let along = d.dot(axis);
    ((d - axis * along).length(), along.abs())
}

#[test]
fn req_op_13_req_op_14_helical_finishing_meets_walls_in_whole_moves() {
    let f = Fixture::new();
    let path = f.path();
    let rotary = RotaryMapping::of_profile(&machine());
    let mut pose = path.group.start;
    let (mut cutting, mut micro, mut walls, mut slowest) = (0, 0, 0, f64::INFINITY);
    for s in &path.group.segments {
        if s.kind == MoveKind::Cutting {
            cutting += 1;
            let (across, along) = motion(&rotary, (pose.tip, pose.a), (s.end.tip, s.end.a));
            if across < TOLERANCE {
                if along > TOLERANCE {
                    walls += 1;
                } else {
                    micro += 1;
                }
            }
            // How fast the tool moves over the part, against the feed: a
            // micro-move charged with part of a wall's contact jump crawls.
            let speed = 60.0 * across.hypot(along) / s.duration.0;
            slowest = slowest.min(speed / FEED);
        }
        pose = s.end;
    }
    assert!(cutting > 1000, "{cutting} cutting moves");
    // One move up or down each of the three walls; no micro-move is left
    // beside them (before the merge there were about ten at each).
    assert_eq!(walls, 3, "{walls} wall moves");
    assert_eq!(micro, 0, "{micro} micro-moves");
    // The slowest move is a wall move, timed for the jump of the contact from
    // one side of the wall to the other (0.17 of the feed here; OI-211); a
    // micro-move charged with part of that jump crawled at about 0.001.
    assert!(slowest > 0.1, "slowest move at {slowest:.4} of the feed");
}

/// Poses along every cutting move: its end and `k - 1` points between, on
/// the straight machine move.
fn poses_along(path: &GeneratedPath, rotary: &RotaryMapping, k: u32) -> Vec<Pose> {
    let mut out = Vec::new();
    let mut from = path.group.start;
    for s in &path.group.segments {
        if s.kind == MoveKind::Cutting {
            let a = rotary.to_axis(from.tip, from.a);
            let b = rotary.to_axis(s.end.tip, s.end.a);
            for i in 1..=k {
                let t = f64::from(i) / f64::from(k);
                let lerp = |p: f64, q: f64| p + t * (q - p);
                let angle = Degrees(lerp(from.a.0, s.end.a.0));
                let tip = rotary.to_model(
                    AxisPosition {
                        along: lerp(a.along, b.along),
                        across: lerp(a.across, b.across),
                        height: lerp(a.height, b.height),
                    },
                    angle,
                );
                out.push(Pose {
                    tip,
                    axis: rotary.tool_axis(angle).get(),
                });
            }
        }
        from = s.end;
    }
    out
}

/// The deepest the tool at any of `poses` goes into a wall's face: the
/// annuli of `exact`, sampled every `step` mm.
fn wall_gouge(outline: &Outline, exact: &Exact, poses: &[Pose], step: f64) -> f64 {
    let mut worst = 0.0_f64;
    for (x, lo, hi) in exact.walls() {
        for pose in poses {
            if (pose.tip.x - x).abs() > outline.radius() + step {
                continue;
            }
            let angle = pose.tip.z.atan2(pose.tip.y);
            let n = ((hi - lo) / step).ceil() as u32;
            for i in 0..=n {
                let r = lo + (hi - lo) * f64::from(i) / f64::from(n);
                for j in -20..=20 {
                    let t = angle + f64::from(j) * step / r;
                    let p = Point3::new(x, r * t.cos(), r * t.sin());
                    let d = p - pose.tip;
                    let z = d.dot(pose.axis);
                    let radial = (d - pose.axis * z).length();
                    worst = worst.max(outline.depth(radial, z));
                }
            }
        }
    }
    worst
}

#[test]
fn req_op_12_the_independent_checker_finds_no_gouge_at_the_walls() {
    let f = Fixture::new();
    let path = f.path();
    let rotary = RotaryMapping::of_profile(&machine());
    let exact = Exact {
        part: cone(),
        solids: solids(),
    };
    let outline = Outline::new(&f.profile);
    // Near the walls only: that is where the merged moves are.
    let near: Vec<Pose> = poses_along(&path, &rotary, 4)
        .into_iter()
        .filter(|p| {
            [-8.0, 0.0, 30.0, 38.0]
                .iter()
                .any(|x| (p.tip.x - x).abs() < 2.0)
        })
        .collect();
    assert!(near.len() > 200, "{} poses near the walls", near.len());
    let surface = max_gouge(&outline, &exact, &near, 0.05);
    let wall = wall_gouge(&outline, &exact, &near, 0.05);
    // Verification plan 4.1: no overcut beyond the tolerance.
    assert!(
        surface.depth <= TOLERANCE,
        "gouge {} at {:?}",
        surface.depth,
        surface.at
    );
    assert!(wall <= TOLERANCE, "wall gouge {wall}");
}

#[test]
fn req_nfr_06_req_gc_07_walls_are_merged_identically_at_1_2_and_n_threads() {
    let f = Fixture::new();
    let runs =
        openchuck_testkit::determinism::check(|| format!("{:?}", f.path().group).into_bytes())
            .unwrap();
    assert_eq!(runs.len(), 4);
}
