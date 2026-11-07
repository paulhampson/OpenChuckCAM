// SPDX-License-Identifier: AGPL-3.0-or-later
//! Measures face-flow finishing (OP-28; decision record 0525) on a STEP part,
//! for the reference job of OI-2007 and the cutter-reach numbers of OI-2004.
//! It changes nothing: it reads a STEP part, places it on the rotary axis as
//! `swarf_floor` does, lists each chosen face's boundary, runs face-flow
//! finishing on it with a ball of the given diameter, and prints the passes,
//! the time and every area the tool cannot reach with its reason and size.
//! It prints numbers only.
//!
//! ```text
//! cargo run --release -p openchuck-app --features kernel-tests --example face_flow_blisk -- \
//!     testdata/step/valid/blisk.step --faces 8 [--ball 3] [--stepover 0.3] [--tolerance 0.01]
//! ```

#![allow(
    clippy::print_stdout,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::cast_precision_loss
)]

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Instant;

use openchuck_app::model::{Axis, Degrees, TransformStep};
use openchuck_geom::transform::{brep_placement, transform_brep};
use openchuck_import::step::StepImporter;
use openchuck_import::{ImportOptions, ModelImporter};
use openchuck_path::face_flow::FaceFlow;
use openchuck_path::params::names;
use openchuck_path::{
    AcceptAll, AnalyticStock, BrepSurface, ParameterValue, RotaryMapping, Strategy, StrategyInput,
};
use openchuck_types::brep::{BrepModel, EdgeId, FaceId};
use openchuck_types::geometry::{Aabb, Point3};
use openchuck_types::ir::PathTags;
use openchuck_types::job::{CancelToken, JobContext, NoProgress};
use openchuck_types::machine::{Clearance, MachineProfile};
use openchuck_types::model::ModelGeometry;
use openchuck_types::tool::{Tool, ToolShape};
use openchuck_types::units::Millimetres;

fn surface_bounds(model: &BrepModel) -> Aabb {
    let (t, _) = model.joined_tessellation();
    let mut b = Aabb {
        min: Point3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY),
        max: Point3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY),
    };
    for p in &t.vertices {
        b.min = Point3::new(b.min.x.min(p.x), b.min.y.min(p.y), b.min.z.min(p.z));
        b.max = Point3::new(b.max.x.max(p.x), b.max.y.max(p.y), b.max.z.max(p.z));
    }
    b
}

/// The part placed as `swarf_floor` places it: its longest span along X,
/// centred on the rotary axis, its end 10 mm from the chuck.
fn placed(bytes: &[u8]) -> BrepModel {
    let cancel = CancelToken::new();
    let imported = StepImporter::new()
        .import(
            bytes,
            &ImportOptions::default(),
            &JobContext::new(&cancel, &NoProgress),
        )
        .unwrap();
    let ModelGeometry::Brep(model) = imported.geometry else {
        panic!("not a B-rep");
    };
    let b = surface_bounds(&model);
    let spans = [b.max.x - b.min.x, b.max.y - b.min.y, b.max.z - b.min.z];
    let mut steps = Vec::new();
    if spans[2] > spans[0] && spans[2] > spans[1] {
        steps.push(TransformStep::Rotate {
            axis: Axis::Y,
            angle: Degrees(90.0),
        });
    } else if spans[1] > spans[0] && spans[1] > spans[2] {
        steps.push(TransformStep::Rotate {
            axis: Axis::Z,
            angle: Degrees(-90.0),
        });
    }
    steps.push(TransformStep::CentreOnRotaryAxis);
    let turned = transform_brep(&model, &brep_placement(&model, &steps).unwrap()).unwrap();
    steps.push(TransformStep::Translate {
        x: 10.0 - surface_bounds(&turned).min.x,
        y: 0.0,
        z: 0.0,
    });
    transform_brep(&model, &brep_placement(&model, &steps).unwrap()).unwrap()
}

fn main() {
    let mut it = std::env::args().skip(1);
    let file = it
        .next()
        .expect("usage: face_flow_blisk <file.step> [options]");
    let (mut faces, mut guide) = (vec![8u32], Vec::<u32>::new());
    let (mut ball, mut stepover, mut tolerance) = (3.0, 0.3, 0.01);
    while let Some(x) = it.next() {
        let mut v = || it.next().expect("a value");
        let list = |s: String| {
            s.split(',')
                .filter(|x| !x.is_empty())
                .map(|x| x.parse().unwrap())
                .collect()
        };
        match x.as_str() {
            "--faces" => faces = list(v()),
            "--guide" => guide = list(v()),
            "--ball" => ball = v().parse().unwrap(),
            "--stepover" => stepover = v().parse().unwrap(),
            "--tolerance" => tolerance = v().parse().unwrap(),
            other => panic!("unknown option {other}"),
        }
    }
    let model = placed(&std::fs::read(&file).unwrap());
    println!("faces {}, edges {}", model.faces.len(), model.edges.len());
    for f in &faces {
        let face = &model.faces[*f as usize];
        let t = &face.tessellation;
        let area: f64 = t
            .triangles
            .iter()
            .map(|[a, b, c]| {
                let (a, b, c) = (
                    t.vertices[*a as usize],
                    t.vertices[*b as usize],
                    t.vertices[*c as usize],
                );
                0.5 * (b - a).cross(c - a).length()
            })
            .sum();
        println!(
            "face {f}: {:?}, {area:.1} mm2, edges {:?}",
            face.surface,
            face.edges.iter().map(|e| e.0).collect::<Vec<_>>()
        );
        for e in &face.edges {
            let edge = &model.edges[e.0 as usize];
            let len: f64 = edge.polyline.windows(2).map(|w| w[0].distance(w[1])).sum();
            println!(
                "  edge {}: {:.3} mm, faces {:?}",
                e.0,
                len,
                edge.faces.iter().map(|f| f.0).collect::<Vec<_>>()
            );
        }
    }
    let tool = Tool {
        shape: ToolShape::BallEnd,
        diameter: Millimetres(ball),
        flute_length: Millimetres(15.0),
        overall_length: Millimetres(50.0),
        shank_diameter: Millimetres(ball),
        flute_count: 2,
        material: None,
        centre_cutting: None,
    };
    let profile = tool.profile().unwrap();
    let machine = MachineProfile::example_x_rotary();
    let surface = BrepSurface::new(&model).unwrap();
    let b = surface_bounds(&model);
    let radius = b
        .max
        .y
        .abs()
        .max(b.min.y.abs())
        .max(b.max.z.abs())
        .max(b.min.z.abs())
        + 1.0;
    let stock = AnalyticStock::Cylinder {
        radius,
        x_start: b.min.x - 1.0,
        x_end: b.max.x + 1.0,
    };
    let tags = PathTags {
        operation: "op".into(),
        setup: "setup".into(),
        tool: "tool".into(),
    };
    let input = StrategyInput {
        surface: &surface,
        stock: &stock,
        collision: &AcceptAll,
        tool: &profile,
        rotary: RotaryMapping::of_profile(&machine),
        clearance: Clearance {
            radius: Millimetres(radius + 5.0),
            safe_z: Millimetres(radius + 15.0),
        },
        max_feeds: machine.max_feeds,
        tags: &tags,
        entry_safety: None,
        facing: None,
        keep_out: &[],
        shank: None,
    };
    let values: BTreeMap<String, ParameterValue> = [
        (names::STEPOVER, stepover),
        (names::TOLERANCE, tolerance),
        (names::FEED, 300.0),
        (names::PLUNGE_FEED, 100.0),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), ParameterValue::Number(v)))
    .collect();
    let strategy = FaceFlow::on_brep(
        Arc::new(model.clone()),
        faces.iter().map(|f| FaceId(*f)).collect(),
        guide.iter().map(|e| EdgeId(*e)).collect(),
    );
    let parameters = strategy.resolve_parameters(&values).unwrap();
    let cancel = CancelToken::new();
    let context = JobContext::new(&cancel, &NoProgress);
    let start = Instant::now();
    match strategy.passes_with_notes(&input, None, &parameters, &context) {
        Ok(out) => {
            let points: usize = out.passes.iter().map(|p| p.points.len()).sum();
            println!(
                "{} passes, {points} points, {:.2} s",
                out.passes.len(),
                start.elapsed().as_secs_f64()
            );
            let a: Vec<f64> = out
                .passes
                .iter()
                .flat_map(|p| p.points.iter().map(|c| c.a.0))
                .collect();
            let (lo, hi) = a
                .iter()
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), x| {
                    (l.min(*x), h.max(*x))
                });
            println!("A of the cut points: {lo:.1} to {hi:.1}");
            for p in out.passes.iter().take(3) {
                let a: Vec<f64> = p.points.iter().map(|c| c.a.0).collect();
                println!(
                    "  a pass: A {:.1} to {:.1}",
                    a.iter().copied().fold(f64::INFINITY, f64::min),
                    a.iter().copied().fold(f64::NEG_INFINITY, f64::max)
                );
            }
            for n in out.notes() {
                println!("note: {n}");
            }
            let total: f64 = out.unreached.iter().map(|u| u.area).sum();
            println!("unreached: {} areas, {total:.1} mm2", out.unreached.len());
            for u in &out.unreached {
                println!(
                    "  {:?} X {:.2}..{:.2} A {:.0}..{:.0} {:.2} mm2",
                    u.reason, u.x.0, u.x.1, u.a.0, u.a.1, u.area
                );
            }
        }
        Err(e) => println!("refused after {:.2} s: {e}", start.elapsed().as_secs_f64()),
    }
}
