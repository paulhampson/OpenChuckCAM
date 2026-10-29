// SPDX-License-Identifier: AGPL-3.0-or-later
//! Radial drilling of a STEP model the kernel wrote (OP-10, IMP-02; decision
//! record 0051): the hole is found from the imported cylinder face and its
//! neighbours, with the kernel's own tessellation. Needs the kernel:
//! `--features kernel-tests`.
#![cfg(feature = "kernel-tests")]
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing
)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use openchuck_import::step::StepImporter;
use openchuck_import::{ImportOptions, ModelImporter};
use openchuck_path::params::names;
use openchuck_path::{
    AcceptAll, AnalyticStock, BrepSurface, HoleEnd, ParameterValue, RadialDrilling, RotaryMapping,
    Strategy, StrategyInput, SurfaceSource,
};
use openchuck_types::geometry::Point3;
use openchuck_types::ir::PathTags;
use openchuck_types::job::{CancelToken, JobContext, NoProgress};
use openchuck_types::machine::{Clearance, MachineProfile};
use openchuck_types::model::ModelGeometry;
use openchuck_types::tool::{Tool, ToolShape};
use openchuck_types::units::{Degrees, Millimetres};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/step/valid")
        .join(format!("{name}.step"))
}

#[test]
fn req_op_10_a_step_blank_with_a_radial_hole_is_found_and_drilled() {
    let bytes = std::fs::read(fixture("rotary_blank_drilled")).unwrap();
    let (cancel, progress) = (CancelToken::new(), NoProgress);
    let ctx = JobContext::new(&cancel, &progress);
    let imported = StepImporter::new()
        .import(&bytes, &ImportOptions::default(), &ctx)
        .unwrap();
    let ModelGeometry::Brep(model) = imported.geometry else {
        panic!("a STEP model is a B-rep");
    };
    let surface = BrepSurface::new(&model).unwrap();
    // The blank is a 24 mm rod 60 mm long with a 6 mm hole across it at X 30,
    // opening towards +Z (testdata/step/generate).
    let holes = surface.holes();
    assert_eq!(holes.len(), 1, "{holes:?}");
    let hole = &holes[0];
    assert_eq!(hole.end, HoleEnd::Through);
    assert!((hole.diameter - 6.0).abs() < 1e-6, "{hole:?}");
    assert!((hole.length - 24.0).abs() < 1e-3, "{hole:?}");
    let d = hole.direction.get();
    assert!(
        d.x.abs() < 1e-9 && d.y.abs() < 1e-9 && (d.z - 1.0).abs() < 1e-9,
        "{d:?}"
    );
    assert!(
        hole.entry.distance(Point3::new(30.0, 0.0, 12.0)) < 1e-3,
        "{hole:?}"
    );

    let machine = MachineProfile::example_x_rotary();
    let tool = Tool {
        shape: ToolShape::Drill {
            point_angle: Degrees(118.0),
        },
        diameter: Millimetres(6.0),
        flute_length: Millimetres(40.0),
        overall_length: Millimetres(80.0),
        shank_diameter: Millimetres(6.0),
        flute_count: 2,
        material: None,
        centre_cutting: Some(true),
    }
    .profile()
    .unwrap();
    let stock = AnalyticStock::Cylinder {
        radius: 12.0,
        x_start: 0.0,
        x_end: 60.0,
    };
    let tags = PathTags {
        operation: "drill".to_owned(),
        setup: "s1".to_owned(),
        tool: "t1".to_owned(),
    };
    let input = StrategyInput {
        surface: &surface,
        stock: &stock,
        collision: &AcceptAll,
        tool: &tool,
        rotary: RotaryMapping::of_profile(&machine),
        clearance: Clearance {
            radius: Millimetres(17.0),
            safe_z: Millimetres(27.0),
        },
        max_feeds: machine.max_feeds,
        tags: &tags,
        entry_safety: None,
        facing: None,
        keep_out: &[],
    };
    let values: BTreeMap<String, ParameterValue> =
        [(names::FEED, 100.0), (names::PLUNGE_FEED, 300.0)]
            .into_iter()
            .map(|(k, x)| (k.to_owned(), ParameterValue::Number(x)))
            .collect();
    let parameters = RadialDrilling.schema().resolve(&values).unwrap();
    let passes = RadialDrilling.passes(&input, &parameters, &ctx).unwrap();
    assert_eq!(passes.len(), 1);
    let points = &passes[0].points;
    assert!(points[0].a.0.abs() < 1e-9, "A {}", points[0].a.0);
    let point = 3.0 / (59.0_f64).to_radians().tan();
    let bottom = points[points.len() - 1].tip;
    // Out of the far side by the point and the allowance.
    assert!(
        bottom.distance(Point3::new(30.0, 0.0, 12.0 - 24.0 - point - 0.5)) < 1e-3,
        "{bottom:?}"
    );
}
