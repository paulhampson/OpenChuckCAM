// SPDX-License-Identifier: AGPL-3.0-or-later
//! Faces perpendicular to the rotary axis under helical operations (issue
//! 384; OP-03, OP-04, OP-05, OP-12; verification plan 4.1).
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
    AcceptAll, AnalyticStock, AxialFinishing, GeneratedPath, HelicalFinishing, MeshSurface,
    ParameterValue, RotaryMapping, RotaryRoughing, SimultaneousFinishing, Strategy, StrategyInput,
    generate,
};
use openchuck_testkit::gouge::{Outline, Pose};
use openchuck_testkit::parts::revolve;
use openchuck_types::geometry::Point3;
use openchuck_types::ir::MoveKind;
use openchuck_types::machine::Clearance;
use openchuck_types::tool::{RevolvedProfile, ToolShape};
use openchuck_types::units::Millimetres;
use support::{machine, run, tags, tool};

/// A stepped shaft: radius 6 to X 10, 10 to X 25, 7 to X 40. Its faces
/// perpendicular to the axis are the annuli at X 10 (r 6 to 10, facing -X)
/// and X 25 (r 7 to 10, facing +X).
const PROFILE: [(f64, f64); 6] = [
    (0.0, 6.0),
    (10.0, 6.0),
    (10.0, 10.0),
    (25.0, 10.0),
    (25.0, 7.0),
    (40.0, 7.0),
];
const FACES: [(f64, f64, f64); 2] = [(10.0, 6.0, 10.0), (25.0, 7.0, 10.0)];

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
                let angle = openchuck_types::units::Degrees(lerp(from.a.0, s.end.a.0));
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

/// Material left on a face: for samples on the annulus, the distance to the
/// nearest tool surface over all poses (zero where a pose reaches or cuts it).
fn face_left(
    outline: &Outline,
    poses: &[Pose],
    face: (f64, f64, f64),
    step: f64,
) -> Vec<(f64, f64, f64)> {
    let (x, lo, hi) = face;
    let near: Vec<&Pose> = poses
        .iter()
        .filter(|p| (p.tip.x - x).abs() < outline.radius() + 3.0)
        .collect();
    let nr = ((hi - lo) / step).ceil() as u32;
    let mut out = Vec::new();
    for i in 0..=nr {
        let r = lo + (hi - lo) * f64::from(i) / f64::from(nr);
        let na = (std::f64::consts::TAU * r / step).ceil() as u32;
        for j in 0..na {
            let t = std::f64::consts::TAU * f64::from(j) / f64::from(na);
            let p = Point3::new(x, r * t.cos(), r * t.sin());
            let mut best = f64::INFINITY;
            for pose in &near {
                let d = p - pose.tip;
                let z = d.dot(pose.axis);
                let radial = (d - pose.axis * z).length();
                if radial > outline.radius() + best {
                    continue;
                }
                best = best.min(outline.clearance(radial, z).max(0.0));
            }
            out.push((r, t, best));
        }
    }
    out
}

fn job(
    strategy: &dyn Strategy,
    shape: ToolShape,
    extra: &[(&str, ParameterValue)],
) -> (GeneratedPath, RevolvedProfile) {
    let surface = MeshSurface::new(&revolve(&PROFILE, 720)).unwrap();
    let profile = tool(shape, 3.175).profile().unwrap();
    let stock = AnalyticStock::Cylinder {
        radius: 11.0,
        x_start: -2.0,
        x_end: 42.0,
    };
    let machine = machine();
    let tag = tags();
    let input = StrategyInput {
        surface: &surface,
        stock: &stock,
        collision: &AcceptAll,
        tool: &profile,
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
    };
    let mut values: BTreeMap<String, ParameterValue> = [
        (names::STEPOVER, 2.2225),
        (names::FEED, 600.0),
        (names::PLUNGE_FEED, 150.0),
        (names::TOLERANCE, 0.01),
    ]
    .into_iter()
    .map(|(k, x)| (k.to_owned(), ParameterValue::Number(x)))
    .collect();
    for (k, v) in extra {
        values.insert((*k).to_owned(), v.clone());
    }
    let _ = Arc::new(());
    (
        run(|ctx| generate(strategy, &input, &values, ctx)).unwrap(),
        profile,
    )
}

fn report(label: &str, path: &GeneratedPath, profile: &RevolvedProfile) {
    let rotary = RotaryMapping::of_profile(&machine());
    let poses = poses_along(path, &rotary, 8);
    let outline = Outline::new(profile);
    for face in FACES {
        let s = face_left(&outline, &poses, face, 0.1);
        let worst = s.iter().map(|v| v.2).fold(0.0_f64, f64::max);
        let mean = s.iter().map(|v| v.2).sum::<f64>() / s.len() as f64;
        let over = s.iter().filter(|v| v.2 > 0.02).count();
        // By radius band.
        let mut bands = String::new();
        for b in 0..4 {
            let (lo, hi) = (
                face.1 + (face.2 - face.1) * b as f64 / 4.0,
                face.1 + (face.2 - face.1) * (b + 1) as f64 / 4.0,
            );
            let w = s
                .iter()
                .filter(|v| v.0 >= lo && v.0 <= hi)
                .map(|v| v.2)
                .fold(0.0_f64, f64::max);
            bands.push_str(&format!(" r{lo:.1}-{hi:.1}:{w:.3}"));
        }
        eprintln!(
            "{label} face X{}: samples {} worst {worst:.3} mean {mean:.3} over0.02 {over}{bands}",
            face.0,
            s.len()
        );
    }
}

#[test]
fn measure_helical_finishing_faces() {
    for shape in [ToolShape::FlatEnd, ToolShape::BallEnd] {
        let (path, profile) = job(&HelicalFinishing, shape.clone(), &[]);
        report(&format!("helical {shape:?}"), &path, &profile);
    }
}

#[test]
fn measure_other_helical_faces() {
    let (path, profile) = job(
        &RotaryRoughing,
        ToolShape::FlatEnd,
        &[
            ("stepdown", ParameterValue::Number(1.0)),
            (names::STOCK_TO_LEAVE, ParameterValue::Number(0.3)),
        ],
    );
    report("roughing continuous leave0.3 FlatEnd", &path, &profile);
}

#[test]
fn measure_axial_faces() {
    let (path, profile) = job(&AxialFinishing, ToolShape::FlatEnd, &[]);
    report("axial FlatEnd", &path, &profile);
}
