// SPDX-License-Identifier: AGPL-3.0-or-later
//! OP-24/GC-07: exact default IR/diagnostics recorded before the optional
//! roughing::face scope change. Version comments are a post concern.
#![allow(clippy::unwrap_used, clippy::float_cmp, clippy::indexing_slicing)]
mod support;
use openchuck_path::{Facing, MeshSurface, ParameterValue, Strategy, generate};
use openchuck_types::geometry::Point3;
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::tool::ToolShape;
use std::collections::BTreeMap;
use support::{Fixture, run, tool};

#[test]
fn req_op_24_req_gc_07_legacy_defaults_match_pre_scope_ir_and_diagnostics() {
    let mesh = TriangleMesh {
        vertices: vec![
            Point3::new(10., -2., 0.),
            Point3::new(20., -2., 0.),
            Point3::new(20., 2., 0.),
        ],
        triangles: vec![[0, 1, 2]],
    };
    let mut f = Fixture::new(
        MeshSurface::new(&mesh).unwrap(),
        10.,
        &tool(ToolShape::FlatEnd, 2.),
    );
    f.stock = openchuck_path::AnalyticStock::Cylinder {
        radius: 10.,
        x_start: 0.,
        x_end: 30.,
    };
    for (shape, size) in [("flat", 6.), ("round", 12.)] {
        let values: BTreeMap<String, ParameterValue> = [
            ("shape", ParameterValue::Text(shape.into())),
            ("size", ParameterValue::Number(size)),
            ("angles", ParameterValue::Text("0,90".into())),
            ("feed", ParameterValue::Number(600.)),
            ("plunge_feed", ParameterValue::Number(200.)),
            ("stepover", ParameterValue::Number(1.)),
            ("stepdown", ParameterValue::Number(2.)),
            ("max_angle_step", ParameterValue::Number(30.)),
        ]
        .into_iter()
        .map(|(k, v)| (k.into(), v))
        .collect();
        let values = values
            .into_iter()
            .filter(|(k, _)| Facing.schema().get(k).is_some())
            .collect();
        for threads in [1, 2, 4] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            let path = pool
                .install(|| run(|ctx| generate(&Facing, &f.input(), &values, ctx)))
                .unwrap();
            let notes = path
                .notes
                .iter()
                .map(|n| (n.segment, n.message.as_str()))
                .collect::<Vec<_>>();
            let warnings = path
                .warnings
                .iter()
                .map(|w| (&w.message, &w.parameter))
                .collect::<Vec<_>>();
            let bytes = serde_json::to_vec(&(&path.group, notes, warnings)).unwrap();
            let hash = bytes.iter().fold(0xcbf29ce484222325u64, |h, b| {
                (h ^ u64::from(*b)).wrapping_mul(0x100000001b3)
            });
            println!(
                "{shape} threads={threads}: {hash:016x}, bytes={}, segments={}",
                bytes.len(),
                path.group.segments.len()
            );
            let (expected, length) = if shape == "flat" {
                (0x2ed1e05d2c113015u64, 346775)
            } else {
                (0x9a9c29f3ec238440u64, 184577)
            };
            assert_eq!(
                (hash, bytes.len()),
                (expected, length),
                "Pre-scope baseline at {threads} threads"
            );
        }
    }
}
