// SPDX-License-Identifier: AGPL-3.0-or-later
//! Fine finishing of a carving with a V-bit (issue #382; NFR-04; decision
//! record 0292): the drops of one axial pass on the carving test piece, the
//! same next to a held box and a tab, and axial finishing of a region of it.
//! A baseline for the 10% regression flag (verification plan section 4.1).
//! The whole-strategy timings at 1, 2 and N threads are taken with
//! `openchuck-path`'s `carving_profile` example.

#![allow(missing_docs, clippy::unwrap_used)]

use std::collections::BTreeMap;
use std::sync::Arc;

use criterion::{Criterion, criterion_group, criterion_main};
use openchuck_path::params::names;
use openchuck_path::{
    AcceptAll, AnalyticStock, AxialFinishing, CompositeSurface, MeshSurface, ParameterValue,
    RegionConstraint, RotaryMapping, Solid, StrategyInput, SurfaceSource, generate_in_region,
};
use openchuck_testkit::parts::{CarvedRelief, RadialPart};
use openchuck_types::geometry::{Aabb, Point3, UnitVec3};
use openchuck_types::ir::PathTags;
use openchuck_types::job::{CancelToken, JobContext, NoProgress};
use openchuck_types::machine::{Clearance, MachineProfile};
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::tool::{RevolvedProfile, Tool, ToolShape};
use openchuck_types::units::{Degrees, Millimetres};

fn relief(spacing: f64) -> TriangleMesh {
    let part = CarvedRelief::STANDARD;
    let rings = (part.length / spacing).round() as u32 + 1;
    let around = (std::f64::consts::TAU * part.radius / spacing).round() as u32;
    part.mesh(rings, around)
}

fn v_bit() -> RevolvedProfile {
    Tool {
        shape: ToolShape::VBit {
            included_angle: Degrees(60.0),
            tip_diameter: Millimetres(0.1),
        },
        diameter: Millimetres(3.175),
        flute_length: Millimetres(8.0),
        overall_length: Millimetres(40.0),
        shank_diameter: Millimetres(3.175),
        flute_count: 1,
        material: None,
        centre_cutting: None,
    }
    .profile()
    .unwrap()
}

/// The tool dropped every 0.01 mm along one axial pass from X -10 to 30, as
/// a pass in a region is seeded.
fn drops(surface: &dyn SurfaceSource, tool: &RevolvedProfile, rotary: &RotaryMapping) -> usize {
    (0..4000)
        .filter_map(|i| {
            let x = -10.0 + 0.01 * f64::from(i);
            let line = rotary.tool_line(Millimetres(x), Degrees(37.0), Millimetres(0.0));
            surface.drop_tool(tool, &line)
        })
        .count()
}

fn carving(c: &mut Criterion) {
    let mesh = relief(0.1);
    let model = Arc::new(MeshSurface::new(&mesh).unwrap());
    let tool = v_bit();
    let machine = MachineProfile::example_x_rotary();
    let rotary = RotaryMapping::of_profile(&machine);
    let held = CompositeSurface::new(
        model.clone(),
        vec![
            Solid::cuboid(Aabb {
                min: Point3::new(-10.0, -15.0, -15.0),
                max: Point3::new(-4.0, 15.0, 15.0),
            })
            .unwrap(),
            Solid::cylinder(Point3::new(-4.0, 0.0, 0.0), UnitVec3::X, 4.0, 5.0).unwrap(),
        ],
    );
    let mut group = c.benchmark_group("carving");
    group.sample_size(10);
    group.bench_function("4000 V-bit drops along a pass, 454k-triangle carving", |b| {
        b.iter(|| std::hint::black_box(drops(model.as_ref(), &tool, &rotary)));
    });
    group.bench_function("4000 V-bit drops along a pass, with a held box and a tab", |b| {
        b.iter(|| std::hint::black_box(drops(&held, &tool, &rotary)));
    });

    let middle = MeshSurface::new(&TriangleMesh {
        vertices: mesh.vertices.clone(),
        triangles: mesh
            .triangles
            .iter()
            .copied()
            .filter(|t| {
                t.iter().all(|&i| {
                    let x = mesh.vertices[i as usize].x;
                    (5.0..=25.0).contains(&x)
                })
            })
            .collect(),
    })
    .unwrap();
    let region = RegionConstraint {
        region: Some(&middle),
        ..Default::default()
    };
    let radius = CarvedRelief::STANDARD.radius + CarvedRelief::STANDARD.depth + 1.5;
    let stock = AnalyticStock::Cylinder {
        radius,
        x_start: -1.0,
        x_end: 31.0,
    };
    let tags = PathTags {
        operation: "carving".to_owned(),
        setup: "setup".to_owned(),
        tool: "v-bit".to_owned(),
    };
    let input = StrategyInput {
        surface: model.as_ref(),
        stock: &stock,
        collision: &AcceptAll,
        tool: &tool,
        rotary,
        clearance: Clearance {
            radius: Millimetres(radius + 5.0),
            safe_z: Millimetres(radius + 15.0),
        },
        max_feeds: machine.max_feeds,
        tags: &tags,
        entry_safety: None,
        facing: None,
        keep_out: &[],
    };
    // Few passes, so the run is short: each still seeds its region interval
    // every 0.01 mm and finds it among 300k region triangles.
    let values: BTreeMap<String, ParameterValue> = [
        (names::STEPOVER, 4.0),
        (names::FEED, 300.0),
        (names::PLUNGE_FEED, 100.0),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), ParameterValue::Number(v)))
    .collect();
    let (cancel, progress) = (CancelToken::new(), NoProgress);
    let context = JobContext::new(&cancel, &progress);
    group.bench_function("axial finishing of a 300k-triangle region, 21 passes", |b| {
        b.iter(|| {
            std::hint::black_box(
                generate_in_region(&AxialFinishing, &input, &region, &values, &context).unwrap(),
            )
        });
    });
    group.finish();
}

criterion_group!(benches, carving);
criterion_main!(benches);
