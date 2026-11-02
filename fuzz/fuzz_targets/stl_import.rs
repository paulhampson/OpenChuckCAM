// SPDX-License-Identifier: AGPL-3.0-or-later
//! STL files are user-supplied (NFR-07; work breakdown slice 2 item 5):
//! importing any bytes, then checking and placing the mesh, must give a value
//! or an error, never a crash or a hang. CI seeds the corpus from
//! `testdata/stl/`.
#![no_main]

use libfuzzer_sys::fuzz_target;
use openchuck_geom::check::check_mesh;
use openchuck_geom::transform::orient_to_rotary_axis;
use openchuck_geom::transform::{AxisDirection, place};
use openchuck_import::{ImportOptions, import_bytes};
use openchuck_types::job::{CancelToken, JobContext, NoProgress};
use openchuck_types::model::ModelGeometry;

fuzz_target!(|data: &[u8]| {
    let (cancel, progress) = (CancelToken::new(), NoProgress);
    let context = JobContext::new(&cancel, &progress);
    let Ok(model) = import_bytes("fuzz.stl", data, &ImportOptions::default(), &context) else {
        return;
    };
    let ModelGeometry::Mesh(mesh) = &model.geometry else {
        return;
    };
    let _ = check_mesh(mesh, model.stated_normals.as_deref());
    if let Some(along) = AxisDirection::parse("+z") {
        let _ = place(mesh, &orient_to_rotary_axis(along));
    }
});
