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
use std::collections::BTreeMap;

use openchuck_path::edge_following::parameter as p;
use openchuck_path::params::names;
use openchuck_path::{
    AcceptAll, AnalyticStock, BrepEdges, BrepSurface, CurveSource, EdgeChain, EdgeFollowing,
    ParameterValue, RotaryMapping, Strategy, StrategyInput, SurfaceSource,
};
use openchuck_types::brep::BrepModel;
use openchuck_types::ir::PathTags;
use openchuck_types::job::{CancelToken, JobContext, NoProgress};
use openchuck_types::machine::{Clearance, MachineProfile};
use openchuck_types::model::ModelGeometry;
use openchuck_types::tool::{Tool, ToolShape};
use openchuck_types::units::Millimetres;

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

fn chains(name: &str, crease: Option<f64>) -> Vec<EdgeChain> {
    let model = import(name);
    let surface = BrepSurface::new(&model).unwrap();
    let mut source = BrepEdges::new(&model).unwrap();
    if let Some(c) = crease {
        source = source.with_crease(c);
    }
    source.chains(&surface).unwrap()
}

fn counts(chains: &[EdgeChain]) -> (usize, usize) {
    (
        chains.iter().filter(|c| !c.closed).count(),
        chains.iter().filter(|c| c.closed).count(),
    )
}

#[test]
fn req_op_08_step_a_box_has_twelve_open_chains() {
    // Three edges meet at every corner, so each edge is a chain of its own.
    assert_eq!(counts(&chains("box", None)), (12, 0));
}

#[test]
fn req_op_08_step_seams_poles_and_degenerate_edges_are_no_edges() {
    // Rims only: the cylinder's and cone's seams, the sphere's meridian and poles,
    // the torus's two seams and the revolved spline's seam are dropped.
    assert_eq!(counts(&chains("cylinder", None)), (0, 2));
    assert_eq!(counts(&chains("cone", None)), (0, 2));
    assert_eq!(counts(&chains("sphere", None)), (0, 0));
    assert_eq!(counts(&chains("torus", None)), (0, 0));
    assert_eq!(counts(&chains("revolved_spline", None)), (0, 2));
    // Keeping tangent edges does not bring a seam back.
    assert_eq!(counts(&chains("cylinder", Some(0.0))), (0, 2));
    assert_eq!(counts(&chains("sphere", Some(0.0))), (0, 0));
}

#[test]
fn req_op_08_step_the_boundaries_of_fillets_are_edges_only_at_crease_angle_zero() {
    // Every edge of the filleted box is blended: all its edges are tangent.
    assert!(chains("filleted_box", None).is_empty());
    assert_eq!(chains("filleted_box", Some(0.0)).len(), 48);
}

#[test]
fn req_op_08_step_chains_are_the_same_on_every_call() {
    for name in ["box", "multi_body", "rotary_blank_drilled"] {
        assert_eq!(chains(name, None), chains(name, None), "{name}");
    }
    let (open, closed) = counts(&chains("multi_body", None));
    assert_eq!((open + closed, closed), (14, 2));
}

fn ball() -> Tool {
    Tool {
        shape: ToolShape::BallEnd,
        diameter: Millimetres(1.0),
        flute_length: Millimetres(12.0),
        overall_length: Millimetres(50.0),
        shank_diameter: Millimetres(1.0),
        flute_count: 2,
        material: None,
        centre_cutting: None,
    }
}

#[test]
fn req_op_08_step_a_rotary_blank_is_engraved_along_its_b_rep_edges_at_constant_depth() {
    // The blank is a Ø24 cylinder about X with a radial hole: its rims and the hole's
    // rim are closed edges on the Ø24 wall.
    let model = import("rotary_blank_drilled");
    let surface = BrepSurface::new(&model).unwrap();
    let machine = MachineProfile::example_x_rotary();
    let b = surface.bounds();
    let stock = AnalyticStock::Cylinder {
        radius: 14.0,
        x_start: b.min.x - 1.0,
        x_end: b.max.x + 1.0,
    };
    let tool = ball().profile().unwrap();
    let tags = PathTags {
        operation: "op".to_owned(),
        setup: "setup".to_owned(),
        tool: "tool".to_owned(),
    };
    let input = StrategyInput {
        surface: &surface,
        stock: &stock,
        collision: &AcceptAll,
        tool: &tool,
        rotary: RotaryMapping::of_profile(&machine),
        clearance: Clearance {
            radius: Millimetres(19.0),
            safe_z: Millimetres(29.0),
        },
        max_feeds: machine.max_feeds,
        tags: &tags,
        entry_safety: None,
        facing: None,
        keep_out: &[],
    };
    let strategy = EdgeFollowing::with_source(BrepEdges::new(&model).unwrap().shared());
    let depth = 0.2;
    let values: BTreeMap<String, ParameterValue> = [
        (names::STEPOVER, 0.5),
        (names::FEED, 300.0),
        (names::PLUNGE_FEED, 100.0),
        (p::DEPTH, depth),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), ParameterValue::Number(v)))
    .collect();
    let parameters = strategy.schema().resolve(&values).unwrap();
    let cancel = CancelToken::new();
    let context = JobContext::new(&cancel, &NoProgress);
    let out = strategy
        .passes_with_notes(&input, None, &parameters, &context)
        .unwrap();
    assert_eq!(out.passes.len(), 4, "the four closed chains");
    for pass in &out.passes {
        assert!(!pass.points.is_empty());
        for c in &pass.points {
            let r = c.tip.y.hypot(c.tip.z);
            assert!(
                (r - (12.0 - depth)).abs() <= 0.02,
                "tip at radius {r}, wanted {}",
                12.0 - depth
            );
        }
    }
}

// ---- the exact edge curve from the kernel (decision 0227, OI-229) ------------

const CORPUS: [&str; 14] = [
    "box",
    "box_inch",
    "bspline_face",
    "cone",
    "cylinder",
    "extruded_spline",
    "filleted_box",
    "hex_bar",
    "multi_body",
    "revolved_spline",
    "revolved_spline_partial",
    "rotary_blank_drilled",
    "sphere",
    "torus",
];

/// How far `p` is from the polyline `line`.
fn distance_to(
    line: &[openchuck_types::geometry::Point3],
    p: openchuck_types::geometry::Point3,
) -> f64 {
    line.windows(2)
        .map(|w| {
            let (a, b) = (w[0], w[1]);
            let ab = [b.x - a.x, b.y - a.y, b.z - a.z];
            let ap = [p.x - a.x, p.y - a.y, p.z - a.z];
            let len2: f64 = ab.iter().map(|v| v * v).sum();
            let s = if len2 > 0.0 {
                (ab.iter().zip(&ap).map(|(x, y)| x * y).sum::<f64>() / len2).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let foot = openchuck_types::geometry::Point3::new(
                a.x + s * ab[0],
                a.y + s * ab[1],
                a.z + s * ab[2],
            );
            foot.distance(p)
        })
        .fold(f64::MAX, f64::min)
}

#[test]
fn req_op_08_step_every_edge_curve_runs_where_the_kernels_polyline_runs() {
    use openchuck_path::edge_curve::sample;
    let mut seen = 0;
    for name in CORPUS {
        let model = import(name);
        let tolerance = model.tessellation.linear + 1e-6;
        for edge in &model.edges {
            let Some(curve) = &edge.curve else { continue };
            seen += 1;
            let dense =
                sample(curve).unwrap_or_else(|| panic!("{name}: edge {} samples", edge.id.0));
            let (f, l) = (
                edge.polyline.first().unwrap(),
                edge.polyline.last().unwrap(),
            );
            assert!(
                dense[0].distance(*f) < 1e-6,
                "{name}: edge {} start",
                edge.id.0
            );
            assert!(
                dense.last().unwrap().distance(*l) < 1e-6,
                "{name}: edge {} end",
                edge.id.0
            );
            for q in &edge.polyline {
                let d = distance_to(&dense, *q);
                assert!(
                    d <= openchuck_path::edge_curve::SAG + 1e-9,
                    "{name}: edge {} vertex {d} off the curve",
                    edge.id.0
                );
            }
            // The sample is as close to the polyline as the polyline is to the curve.
            for q in &dense {
                assert!(
                    distance_to(&edge.polyline, *q) <= tolerance,
                    "{name}: edge {} is more than the tessellation tolerance from the polyline",
                    edge.id.0
                );
            }
        }
    }
    assert!(seen > 50, "the corpus gave only {seen} edge curves");
}

#[test]
fn req_op_08_step_the_kernel_gives_the_curve_kinds_of_the_corpus_edges() {
    use openchuck_types::brep::EdgeCurve;
    let kinds = |name: &str| {
        let model = import(name);
        let (mut lines, mut circles, mut splines, mut none) = (0, 0, 0, 0);
        for e in &model.edges {
            match e.curve {
                Some(EdgeCurve::Line { .. }) => lines += 1,
                Some(EdgeCurve::Circle { .. }) => circles += 1,
                Some(EdgeCurve::BSpline(_)) => splines += 1,
                _ => none += 1,
            }
        }
        (lines, circles, splines, none)
    };
    let (lines, circles, _, _) = kinds("box");
    assert_eq!((lines, circles), (12, 0));
    let (_, circles, _, _) = kinds("cylinder");
    assert!(circles >= 2, "the rims are circles");
    let (_, _, splines, _) = kinds("bspline_face");
    assert!(splines > 0, "a spline face has spline edges");
}

#[test]
fn req_op_08_step_chain_points_are_on_the_edge_curve_not_the_tessellation() {
    // The rim of the cylinder: every chain point is on the circle to rounding.
    use openchuck_types::brep::EdgeCurve;
    let model = import("cylinder");
    let rims: Vec<_> = model
        .edges
        .iter()
        .filter_map(|e| match &e.curve {
            Some(EdgeCurve::Circle { frame, radius, .. }) => Some((e.id, *frame, *radius)),
            _ => None,
        })
        .collect();
    assert!(!rims.is_empty());
    let (id, frame, radius) = rims[0];
    let surface = BrepSurface::new(&model).unwrap();
    let source = BrepEdges::with_edges(&model, &[id]).unwrap();
    let chains = source.chains(&surface).unwrap();
    assert_eq!(chains.len(), 1);
    for q in &chains[0].points {
        let d = [
            q.x - frame.origin.x,
            q.y - frame.origin.y,
            q.z - frame.origin.z,
        ];
        let along = d[0] * frame.z.x + d[1] * frame.z.y + d[2] * frame.z.z;
        let ring = (d.iter().map(|v| v * v).sum::<f64>() - along * along).sqrt();
        assert!(
            along.abs() < 1e-9 && (ring - radius).abs() < 1e-9,
            "{along} {ring}"
        );
    }
}
