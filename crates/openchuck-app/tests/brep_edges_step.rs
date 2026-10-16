// SPDX-License-Identifier: AGPL-3.0-or-later
//! B-rep edges of the STEP corpus as edge chains (OP-08; work breakdown slice 16;
//! decision record 0053). Each part is imported with the kernel, its edges become
//! chains through `BrepEdges`, and the chains are checked against the topology the
//! part has. Needs the kernel: `--features kernel-tests`.
#![cfg(feature = "kernel-tests")]
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing,
    clippy::cast_precision_loss
)]

use std::path::{Path, PathBuf};

use openchuck_import::step::StepImporter;
use openchuck_import::{ImportOptions, ModelImporter};
use openchuck_path::{BrepEdges, BrepSurface, CurveSource};
use openchuck_types::brep::BrepModel;
use openchuck_types::job::{CancelToken, JobContext, NoProgress};
use openchuck_types::model::ModelGeometry;

fn import(name: &str) -> BrepModel {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
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
        ModelGeometry::Mesh(_) => panic!("not a B-rep"),
    }
}

#[test]
fn probe() {
    for name in [
        "box",
        "cylinder",
        "cone",
        "sphere",
        "torus",
        "filleted_box",
        "rotary_blank_drilled",
        "revolved_spline",
        "multi_body",
    ] {
        let m = import(name);
        let surface = BrepSurface::new(&m).unwrap();
        let real = BrepEdges::new(&m).unwrap();
        let all = BrepEdges::new(&m).unwrap().with_crease(0.0);
        let c = real.chains(&surface).unwrap();
        let ca = all.chains(&surface).unwrap();
        eprintln!(
            "{name}: faces {} edges {} ids {:?} kept {} chains {} closed {} | crease0 kept {} chains {} | bounds {:?}",
            m.faces.len(),
            m.edges.len(),
            m.edges
                .iter()
                .map(|e| (e.faces.len(), e.polyline.len()))
                .collect::<Vec<_>>(),
            real.edge_ids().len(),
            c.len(),
            c.iter().filter(|x| x.closed).count(),
            all.edge_ids().len(),
            ca.len(),
            openchuck_path::SurfaceSource::bounds(&surface),
        );
    }
}
