// SPDX-License-Identifier: AGPL-3.0-or-later
//! The exact closest point on the STEP corpus (lane STEPALL-X1, issue #348;
//! OI-839; requirement IMP-02): the kernel puts a face's tessellation
//! vertices on the face, so `ExactFace::closest` must answer each of them at
//! no distance. Before the fix, 13 vertices of the cone's large rim and 14 of
//! the shouldered shaft's torus were answered on the seam, up to 4.2 mm away.
//! Needs the kernel: `--features kernel-tests`.
#![cfg(feature = "kernel-tests")]
#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::Path;

use openchuck_import::step::StepImporter;
use openchuck_import::{ImportOptions, ModelImporter};
use openchuck_path::{BrepSurface, ParametricFace, SurfaceSource};
use openchuck_types::brep::{BrepModel, SurfaceKind};
use openchuck_types::job::{CancelToken, JobContext, NoProgress};
use openchuck_types::model::ModelGeometry;

fn import(name: &str) -> BrepModel {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/step/valid")
        .join(format!("{name}.step"));
    let bytes = std::fs::read(path).unwrap();
    let (cancel, progress) = (CancelToken::new(), NoProgress);
    let model = StepImporter::new()
        .import(
            &bytes,
            &ImportOptions::default(),
            &JobContext::new(&cancel, &progress),
        )
        .unwrap();
    match model.geometry {
        ModelGeometry::Brep(b) => b,
        ModelGeometry::Mesh(_) => panic!("{name} is not a B-rep"),
    }
}

/// Every vertex of every face of `kind` in `part`, asked of its own face and
/// of the whole part: the largest answer, mm.
fn worst(part: &str, kind: SurfaceKind) -> (f64, f64, usize) {
    let model = import(part);
    let source = BrepSurface::new(&model).unwrap();
    let (mut own, mut whole, mut n) = (0.0_f64, 0.0_f64, 0);
    for (face, exact) in model.faces.iter().zip(source.faces()) {
        if face.surface != kind {
            continue;
        }
        for v in &face.tessellation.vertices {
            own = own.max(exact.closest(*v).unwrap().distance);
            whole = whole.max(source.distance(*v).unwrap());
            n += 1;
        }
    }
    (own, whole, n)
}

#[test]
fn req_imp_02_corpus_cone_vertices_lie_on_their_face() {
    let (own, whole, n) = worst("cone", SurfaceKind::Cone);
    assert!(n > 1000, "{n} vertices");
    assert!(
        own < 1e-6,
        "a cone vertex is answered {own} mm from its face"
    );
    assert!(
        whole < 1e-6,
        "a cone vertex is answered {whole} mm from the part"
    );
}

#[test]
fn req_imp_02_corpus_torus_vertices_lie_on_their_face() {
    let (own, whole, n) = worst("shouldered_shaft", SurfaceKind::Torus);
    assert!(n > 1000, "{n} vertices");
    assert!(
        own < 1e-6,
        "a torus vertex is answered {own} mm from its face"
    );
    assert!(
        whole < 1e-6,
        "a torus vertex is answered {whole} mm from the part"
    );
}
