// SPDX-License-Identifier: AGPL-3.0-or-later
//! Throwaway (lane STEPALL-X2 stage 1): writes the placed blisk and the
//! corpus parts as BrepModel JSON for path-library benchmarks.
#![allow(clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]
use openchuck_app::model::{Axis, Degrees, TransformStep};
use openchuck_geom::transform::{brep_placement, transform_brep};
use openchuck_import::step::StepImporter;
use openchuck_import::{ImportOptions, ModelImporter};
use openchuck_types::brep::BrepModel;
use openchuck_types::geometry::{Aabb, Point3};
use openchuck_types::job::{CancelToken, JobContext, NoProgress};
use openchuck_types::model::ModelGeometry;

fn bounds(model: &BrepModel) -> Aabb {
    let mut lo = Point3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
    let mut hi = Point3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for f in &model.faces {
        for v in &f.tessellation.vertices {
            lo = Point3::new(lo.x.min(v.x), lo.y.min(v.y), lo.z.min(v.z));
            hi = Point3::new(hi.x.max(v.x), hi.y.max(v.y), hi.z.max(v.z));
        }
    }
    Aabb { min: lo, max: hi }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let input = args.next().unwrap();
    let out = args.next().unwrap();
    let bytes = std::fs::read(&input).unwrap();
    let cancel = CancelToken::new();
    let imported = StepImporter::new()
        .import(
            &bytes,
            &ImportOptions::default(),
            &JobContext::new(&cancel, &NoProgress),
        )
        .unwrap();
    let ModelGeometry::Brep(model) = imported.geometry else {
        panic!()
    };
    let b = bounds(&model);
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
    let map = brep_placement(&model, &steps).unwrap();
    let rotated = transform_brep(&model, &map).unwrap();
    let x0 = bounds(&rotated).min.x;
    steps.push(TransformStep::Translate {
        x: 10.0 - x0,
        y: 0.0,
        z: 0.0,
    });
    let map = brep_placement(&model, &steps).unwrap();
    let placed = transform_brep(&model, &map).unwrap();
    println!("{:?}", bounds(&placed));
    std::fs::write(out, serde_json::to_vec(&placed).unwrap()).unwrap();
}
