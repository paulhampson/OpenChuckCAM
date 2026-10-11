// SPDX-License-Identifier: AGPL-3.0-or-later
//! STL import and the mesh checks on the 1M-triangle benchmark model, written
//! as binary STL in memory (IMP-01, IMP-04; NFR-04). A baseline for the 10%
//! regression flag (verification plan section 4.1).

#![allow(missing_docs, clippy::unwrap_used)]

use criterion::{Criterion, criterion_group, criterion_main};
use openchuck_geom::check::check_mesh;
use openchuck_import::{ImportOptions, import_bytes};
use openchuck_testkit::parts::benchmark_model_1m;
use openchuck_types::job::{CancelToken, JobContext, NoProgress};
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::model::ModelGeometry;

fn binary_stl(mesh: &TriangleMesh) -> Vec<u8> {
    let mut out = vec![0u8; 80];
    out.extend((mesh.triangles.len() as u32).to_le_bytes());
    for t in 0..mesh.triangles.len() {
        out.extend([0u8; 12]);
        for p in mesh.triangle(t).unwrap() {
            for c in [p.x, p.y, p.z] {
                out.extend((c as f32).to_le_bytes());
            }
        }
        out.extend([0, 0]);
    }
    out
}

fn stl(c: &mut Criterion) {
    let bytes = binary_stl(&benchmark_model_1m());
    let (cancel, progress) = (CancelToken::new(), NoProgress);
    let context = JobContext::new(&cancel, &progress);
    let import = || import_bytes("bench.stl", &bytes, &ImportOptions::default(), &context).unwrap();
    let mut group = c.benchmark_group("stl");
    group.sample_size(10);
    group.bench_function("import 1M-triangle binary STL", |b| {
        b.iter(|| std::hint::black_box(import()));
    });
    let model = import();
    let ModelGeometry::Mesh(mesh) = &model.geometry else {
        return;
    };
    group.bench_function("check 1M-triangle mesh", |b| {
        b.iter(|| std::hint::black_box(check_mesh(mesh, model.stated_normals.as_deref()).unwrap()));
    });
    group.finish();
}

criterion_group!(benches, stl);
criterion_main!(benches);
