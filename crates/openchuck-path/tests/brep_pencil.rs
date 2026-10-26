// SPDX-License-Identifier: AGPL-3.0-or-later
//! Pencil clean-up on the concave edges of a B-rep part (OP-09, OP-23,
//! NFR-06, NFR-12; decision record 0226, decision 0209 item 2 (T), lane
//! STEPALL-P). The part is the closed-form shouldered shaft of
//! `openchuck_testkit::brep` (OI-234 (b)), so no kernel is involved; the
//! STEP fixture through the application is in
//! `crates/openchuck-app/tests/step_strategies.rs` (`--features
//! kernel-tests`).
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::cast_precision_loss
)]

mod support;

use std::collections::BTreeMap;

use openchuck_path::params::names;
use openchuck_path::pencil::{concave_creases, pencil_parameter};
use openchuck_path::rest::parameter as rest_parameter;
use openchuck_path::{
    AcceptAll, AnalyticStock, BrepEdges, BrepSurface, CurveSource, ParameterValue, Pass,
    PencilFinishing, RotaryMapping, Strategy, StrategyInput, StrategyOutput, SurfaceSource,
    generate,
};
use openchuck_testkit::brep::{banded_cylinder, shouldered_shaft};
use openchuck_types::brep::{BrepModel, EdgeId};
use openchuck_types::geometry::Point3;
use openchuck_types::ir::PathTags;
use openchuck_types::machine::{Clearance, MachineProfile};
use openchuck_types::tool::profile::RevolvedProfile;
use support::{ball, run};

/// The ball's diameter, mm.
const BALL: f64 = 2.0;

/// How far a rest may be from the closed form, mm: slice 15's exit
/// criterion (within 0.02 mm of the closed form).
const CLOSED_FORM: f64 = 0.02;

fn num(x: f64) -> ParameterValue {
    ParameterValue::Number(x)
}

fn values(extra: &[(&str, f64)]) -> BTreeMap<String, ParameterValue> {
    let mut v: BTreeMap<String, ParameterValue> = [
        (names::FEED, 400.0),
        (names::PLUNGE_FEED, 200.0),
        (names::STEPOVER, 1.0),
    ]
    .into_iter()
    .map(|(k, x)| (k.to_owned(), num(x)))
    .collect();
    for (k, x) in extra {
        v.insert((*k).to_owned(), num(*x));
    }
    v
}

struct Fixture {
    surface: BrepSurface,
    stock: AnalyticStock,
    tool: RevolvedProfile,
    rotary: RotaryMapping,
    clearance: Clearance,
    tags: PathTags,
    machine: MachineProfile,
}

impl Fixture {
    fn new(model: &BrepModel) -> Self {
        let machine = MachineProfile::example_x_rotary();
        let surface = BrepSurface::new(model).unwrap();
        let b = surface.bounds();
        let top = b.max.y.max(b.max.z);
        Self {
            surface,
            // Stock over the whole part: material is left in every corner.
            stock: AnalyticStock::Cylinder {
                radius: top + 1.0,
                x_start: b.min.x - 1.0,
                x_end: b.max.x + 1.0,
            },
            tool: ball(BALL).profile().unwrap(),
            rotary: RotaryMapping::of_profile(&machine),
            clearance: Clearance {
                radius: openchuck_types::units::Millimetres(top + 6.0),
                safe_z: openchuck_types::units::Millimetres(top + 16.0),
            },
            tags: support::tags(),
            machine,
        }
    }

    fn input(&self) -> StrategyInput<'_> {
        StrategyInput {
            surface: &self.surface,
            stock: &self.stock,
            collision: &AcceptAll,
            tool: &self.tool,
            rotary: self.rotary,
            clearance: self.clearance,
            max_feeds: self.machine.max_feeds,
            tags: &self.tags,
            entry_safety: None,
            keep_out: &[],
        }
    }
}

fn pencil(
    f: &Fixture,
    strategy: &PencilFinishing,
    extra: &[(&str, f64)],
) -> openchuck_path::Result<StrategyOutput> {
    let parameters = strategy.schema().resolve(&values(extra)).unwrap();
    run(|ctx| strategy.passes_with_notes(&f.input(), None, &parameters, ctx))
}

fn radius(q: Point3) -> f64 {
    q.y.hypot(q.z)
}

fn concave_edges(model: &BrepModel, degrees: f64) -> Vec<EdgeId> {
    BrepEdges::concave(model)
        .unwrap()
        .with_crease(degrees)
        .edge_ids()
}

// ---- which edges are creases ---------------------------------------------------

#[test]
fn req_op_09_brep_the_only_crease_of_the_shouldered_shaft_is_its_sharp_inside_corner() {
    let model = shouldered_shaft(4);
    // Edge 1 (X 12, radius 8) is the one inside corner. The outside corners
    // (edges 0, 2, 3 and 6) never are, the fillet's tangent boundaries
    // (edges 4 and 5) are not at any crease angle above zero, and the seams
    // are never edges.
    for degrees in [1.0, 20.0, 30.0, 89.0] {
        assert_eq!(concave_edges(&model, degrees), vec![EdgeId(1)], "{degrees}");
    }
    // The crease turns 90 degrees: above that it is not one.
    assert!(concave_edges(&model, 91.0).is_empty());
    // As a chain it is one closed circle at X 12, radius 8.
    let surface = BrepSurface::new(&model).unwrap();
    let chains = BrepEdges::concave(&model)
        .unwrap()
        .with_crease(30.0)
        .chains(&surface)
        .unwrap();
    assert_eq!(chains.len(), 1);
    assert!(chains[0].closed);
    for p in &chains[0].points {
        assert!((p.x - 12.0).abs() < 1e-9, "{p:?}");
        assert!((radius(*p) - 8.0).abs() < 1e-9, "{p:?}");
    }
    // Every real edge, concave or not, is still there for edge following.
    let all = BrepEdges::new(&model).unwrap().edge_ids();
    assert_eq!(
        all,
        [0, 1, 2, 3, 6].map(EdgeId).to_vec(),
        "the outside corners stay edges for edge following"
    );
}

#[test]
fn req_op_09_brep_no_false_creases_between_the_facets_of_a_fillet() {
    // The fillet in four facets: neighbouring facets turn 22.5 degrees, so
    // the tessellation has creases at 20 degrees that the part does not.
    let model = shouldered_shaft(4);
    let (mesh, _) = model.joined_tessellation();
    let in_facets = concave_creases(&mesh, 20.0);
    assert!(
        in_facets.len() > 1,
        "the facets of the fillet read as creases: {}",
        in_facets.len()
    );
    assert!(
        in_facets
            .iter()
            .any(|c| c.points.iter().all(|p| p.x > 24.0)),
        "a false crease lies on the fillet"
    );
    // The B-rep's edges give only the real one, and pencil clean-up follows
    // only it.
    let f = Fixture::new(&model);
    let out = pencil(
        &f,
        &PencilFinishing::on_brep(&model).unwrap(),
        &[(pencil_parameter::CREASE_ANGLE, 20.0)],
    )
    .unwrap();
    assert!(
        out.notes
            .iter()
            .any(|n| n.contains("1 concave crease") && n.contains("edges")),
        "{:?}",
        out.notes
    );
    for pass in &out.passes {
        for c in &pass.points {
            assert!(c.tip.x < 13.0, "a pass on the fillet at {:?}", c.tip);
        }
    }
}

// ---- the cut -----------------------------------------------------------------

#[test]
fn req_op_09_req_nfr_12_brep_pencil_rests_in_the_corner_within_tolerance_without_gouging() {
    let model = shouldered_shaft(8);
    let f = Fixture::new(&model);
    let out = pencil(
        &f,
        &PencilFinishing::on_brep(&model).unwrap(),
        &[(pencil_parameter::MAX_STEP, 0.5), (names::TOLERANCE, 0.01)],
    )
    .unwrap();
    let rb = BALL / 2.0;
    let points: Vec<_> = out.passes.iter().flat_map(|p| p.points.iter()).collect();
    assert!(!points.is_empty());
    // The ball rests against the shoulder (X 12) on the bar (radius 8): its
    // tip is on the bar one tool radius short of the shoulder.
    for c in &points {
        assert!(
            (c.tip.x - (12.0 - rb)).abs() <= CLOSED_FORM,
            "tip at X {} not {}",
            c.tip.x,
            12.0 - rb
        );
        assert!(
            (radius(c.tip) - 8.0).abs() <= CLOSED_FORM,
            "tip at radius {}",
            radius(c.tip)
        );
    }
    // No point of the ball is inside the exact faces by more than the
    // tolerance: its centre is at least its radius from every face.
    let tolerance = 0.01;
    for c in &points {
        let axis = (c.tip - Point3::ORIGIN) * (1.0 / radius(c.tip));
        let axis = openchuck_types::geometry::Vec3::new(0.0, axis.y, axis.z);
        let centre = c.tip + axis * rb;
        let gap = f
            .surface
            .exact_faces()
            .iter()
            .filter_map(|face| face.face.closest(centre))
            .map(|closest| closest.distance)
            .fold(f64::INFINITY, f64::min);
        assert!(
            gap >= rb - tolerance - CLOSED_FORM,
            "the ball reaches {} mm into the part at {:?}",
            rb - gap,
            c.tip
        );
    }
    // The crease is a closed circle: one pass round it.
    assert_eq!(out.passes.len(), 1);
}

#[test]
fn req_op_09_brep_a_part_with_no_inside_corner_says_so() {
    // A bar split into bands: its edges are tangent joins, none an inside
    // corner.
    let model = banded_cylinder(10.0, 30.0, &[10.0, 20.0]);
    let f = Fixture::new(&model);
    let err = pencil(&f, &PencilFinishing::on_brep(&model).unwrap(), &[])
        .unwrap_err()
        .to_string();
    assert!(err.contains("no inside corner to follow"), "{err}");
    assert!(err.contains("edge"), "{err}");
    // The shaft's crease turns 90 degrees: above that there is none either.
    let model = shouldered_shaft(4);
    let f = Fixture::new(&model);
    let err = pencil(
        &f,
        &PencilFinishing::on_brep(&model).unwrap(),
        &[(pencil_parameter::CREASE_ANGLE, 95.0)],
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains("no inside corner to follow"), "{err}");
}

#[test]
fn req_op_09_brep_pencil_cuts_only_where_material_is_left() {
    let model = shouldered_shaft(4);
    let mut f = Fixture::new(&model);
    let corner = PencilFinishing::on_brep(&model).unwrap();
    let min = [(rest_parameter::MIN_REMAINING, 0.1)];
    // Stock over the corner: it is cut.
    assert!(!pencil(&f, &corner, &min).unwrap().passes.is_empty());
    // Stock only beyond the shoulder (X 13 on) leaves material, but none in
    // the corner: pencil clean-up says so, as on a mesh part.
    f.stock = AnalyticStock::Cylinder {
        radius: 13.0,
        x_start: 13.0,
        x_end: 41.0,
    };
    let err = pencil(&f, &corner, &min).unwrap_err().to_string();
    assert!(
        err.contains("the tool cannot rest in any corner that holds it"),
        "{err}"
    );
}

#[test]
fn req_op_09_brep_pencil_takes_no_region() {
    let model = shouldered_shaft(4);
    let f = Fixture::new(&model);
    let strategy = PencilFinishing::on_brep(&model).unwrap();
    let parameters = strategy.schema().resolve(&values(&[])).unwrap();
    let region = openchuck_path::RegionConstraint {
        region: Some(&f.surface),
        ..Default::default()
    };
    let err = run(|ctx| strategy.passes_with_notes(&f.input(), Some(&region), &parameters, ctx))
        .unwrap_err()
        .to_string();
    assert!(err.contains("cannot yet be limited to a region"), "{err}");
}

#[test]
fn req_nfr_06_req_op_09_brep_pencil_is_identical_at_1_2_and_n_threads() {
    let model = shouldered_shaft(4);
    let f = Fixture::new(&model);
    let strategy = PencilFinishing::on_brep(&model).unwrap();
    let v = values(&[(pencil_parameter::MAX_STEP, 0.5)]);
    let runs = openchuck_testkit::determinism::check(|| {
        let path = run(|ctx| generate(&strategy, &f.input(), &v, ctx)).unwrap();
        format!("{:?}{:?}", path.group, path.notes).into_bytes()
    })
    .unwrap();
    assert_eq!(runs.len(), 4);
}

/// The mesh form is unchanged: without B-rep edges pencil clean-up finds
/// its creases in the tessellation, false ones included.
#[test]
fn req_op_09_without_brep_edges_pencil_reads_the_tessellation_as_before() {
    let model = shouldered_shaft(4);
    let f = Fixture::new(&model);
    let out = pencil(
        &f,
        &PencilFinishing::default(),
        &[(pencil_parameter::CREASE_ANGLE, 20.0)],
    )
    .unwrap();
    let on_fillet = |p: &Pass| p.points.iter().any(|c| c.tip.x > 23.0);
    assert!(
        out.passes.iter().any(on_fillet),
        "the mesh form follows the facets of the fillet"
    );
}

#[test]
fn req_op_09_req_nfr_12_brep_an_edge_whose_side_cannot_be_told_is_left_out_and_said() {
    // A damaged model: an edge listed between the bar (face 1) and the end
    // disc (face 0), but lying round the middle of the bar, 6 mm from the
    // disc. The faces turn 90 degrees there, but the edge bounds neither, so
    // which side is material cannot be told: it is not followed, and the
    // notes say how many such edges there are (decision 0226 item 3).
    let mut model = shouldered_shaft(4);
    let ring: Vec<Point3> = (0..=96)
        .map(|k| {
            let u = std::f64::consts::TAU * f64::from(k) / 96.0;
            Point3::new(6.0, 8.0 * u.cos(), 8.0 * u.sin())
        })
        .collect();
    model.edges.push(openchuck_types::brep::BrepEdge {
        id: EdgeId(11),
        faces: vec![
            openchuck_types::brep::FaceId(1),
            openchuck_types::brep::FaceId(0),
        ],
        vertices: None,
        curve: None,
        polyline: ring,
    });
    let edges = BrepEdges::concave(&model).unwrap().with_crease(30.0);
    assert_eq!(edges.edge_ids(), vec![EdgeId(1)]);
    assert_eq!(edges.not_inside_throughout(), 1);
    let f = Fixture::new(&model);
    let out = pencil(&f, &PencilFinishing::on_brep(&model).unwrap(), &[]).unwrap();
    assert!(
        out.notes
            .iter()
            .any(|n| n.contains("1 edge of the part that may be an inside corner is not followed")),
        "{:?}",
        out.notes
    );
}

#[test]
fn req_op_09_req_nfr_07_brep_an_edge_whose_faces_cannot_be_compared_is_counted_as_left_out() {
    // A damaged model: an edge between the bar and the end disc whose
    // points are not numbers, so neither face can be asked about it. It is
    // not followed, and it is counted with the edges left out (decision 0226
    // item 3), whatever the crease angle, since its turn is not known.
    let mut model = shouldered_shaft(4);
    model.edges.push(openchuck_types::brep::BrepEdge {
        id: EdgeId(11),
        faces: vec![
            openchuck_types::brep::FaceId(1),
            openchuck_types::brep::FaceId(0),
        ],
        vertices: None,
        curve: None,
        polyline: vec![
            Point3::new(f64::NAN, 0.0, 0.0),
            Point3::new(1.0, f64::NAN, 0.0),
        ],
    });
    for degrees in [1.0, 30.0, 89.0] {
        let edges = BrepEdges::concave(&model).unwrap().with_crease(degrees);
        assert_eq!(edges.edge_ids(), vec![EdgeId(1)], "{degrees}");
        assert_eq!(edges.not_inside_throughout(), 1, "{degrees}");
    }
    // Edge following keeps it, as an edge it cannot compare (unchanged).
    assert_eq!(BrepEdges::new(&model).unwrap().not_inside_throughout(), 0);
}
