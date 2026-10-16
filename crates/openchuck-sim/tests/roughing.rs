// SPDX-License-Identifier: AGPL-3.0-or-later
//! Rotary roughing against the simulated stock (OP-03, OP-14, SIM-05, SIM-06;
//! work breakdown slice 14 review checklist; decision record 0033).
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_cmp
)]

use std::collections::BTreeMap;

use openchuck_path::params::names;
use openchuck_path::region::RegionConstraint;
use openchuck_path::roughing::parameter;
use openchuck_path::{
    AcceptAll, GeneratedPath, MeshSurface, ParameterValue, RotaryMapping, RotaryRoughing,
    StrategyInput, SurfaceSource, generate, generate_in_region,
};
use openchuck_sim::{DeviationOptions, DexelStock, Simulator, compare};
use openchuck_testkit::parts::{Cone, Cylinder, RadialPart};
use openchuck_types::ir::PathTags;
use openchuck_types::job::{CancelToken, JobContext, NoProgress};
use openchuck_types::machine::{Clearance, MachineProfile};
use openchuck_types::stock::Stock;
use openchuck_types::tool::{RevolvedProfile, Tool, ToolShape};
use openchuck_types::units::Millimetres;

const CELL: f64 = 0.1;

fn tool(shape: ToolShape, d: f64) -> Tool {
    Tool {
        shape,
        diameter: Millimetres(d),
        flute_length: Millimetres(20.0),
        overall_length: Millimetres(50.0),
        shank_diameter: Millimetres(d),
        flute_count: 2,
        material: None,
        centre_cutting: None,
    }
}

fn num(x: f64) -> ParameterValue {
    ParameterValue::Number(x)
}

fn values(extra: &[(&str, ParameterValue)]) -> BTreeMap<String, ParameterValue> {
    let mut v: BTreeMap<String, ParameterValue> = [
        (names::STEPOVER, 3.0),
        (names::FEED, 600.0),
        (names::PLUNGE_FEED, 200.0),
        (names::STOCK_TO_LEAVE, 0.5),
        (parameter::STEPDOWN, 2.0),
    ]
    .into_iter()
    .map(|(k, x)| (k.to_owned(), num(x)))
    .collect();
    for (k, x) in extra {
        v.insert((*k).to_owned(), x.clone());
    }
    v
}

struct Rig {
    machine: MachineProfile,
    surface: MeshSurface,
    target: openchuck_types::mesh::TriangleMesh,
    profile: RevolvedProfile,
    stock_radius: f64,
    tags: PathTags,
    /// How far the stock runs past the part at each end.
    margin: f64,
}

impl Rig {
    fn new<P: RadialPart>(part: &P, stock_radius: f64, tool: &Tool) -> Self {
        let target = part.mesh(20, 360);
        Self {
            machine: MachineProfile::example_x_rotary(),
            surface: MeshSurface::new(&target).unwrap(),
            target,
            profile: tool.profile().unwrap(),
            stock_radius,
            margin: 2.0,
            tags: PathTags {
                operation: "rough".to_owned(),
                setup: "setup".to_owned(),
                tool: "tool".to_owned(),
            },
        }
    }

    fn stock(&self) -> DexelStock {
        let b = self.surface.bounds();
        let stock = Stock::Cylinder {
            diameter: Millimetres(2.0 * self.stock_radius),
            length: Millimetres(b.max.x - b.min.x + 2.0 * self.margin),
            x_start: Millimetres(b.min.x - self.margin),
        };
        DexelStock::from_stock(&stock, None, CELL).unwrap()
    }

    fn generate(
        &self,
        stock: &DexelStock,
        v: &BTreeMap<String, ParameterValue>,
    ) -> openchuck_path::Result<GeneratedPath> {
        let cancel = CancelToken::new();
        let ctx = JobContext::new(&cancel, &NoProgress);
        let input = StrategyInput {
            surface: &self.surface,
            stock,
            collision: &AcceptAll,
            tool: &self.profile,
            rotary: RotaryMapping::of_profile(&self.machine),
            clearance: Clearance {
                radius: Millimetres(self.stock_radius + 5.0),
                safe_z: Millimetres(self.stock_radius + 15.0),
            },
            max_feeds: self.machine.max_feeds,
            tags: &self.tags,
        };
        generate(&RotaryRoughing, &input, v, &ctx)
    }

    fn generate_in(
        &self,
        stock: &DexelStock,
        region: &RegionConstraint<'_>,
        v: &BTreeMap<String, ParameterValue>,
    ) -> openchuck_path::Result<GeneratedPath> {
        let cancel = CancelToken::new();
        let ctx = JobContext::new(&cancel, &NoProgress);
        let input = StrategyInput {
            surface: &self.surface,
            stock,
            collision: &AcceptAll,
            tool: &self.profile,
            rotary: RotaryMapping::of_profile(&self.machine),
            clearance: Clearance {
                radius: Millimetres(self.stock_radius + 5.0),
                safe_z: Millimetres(self.stock_radius + 15.0),
            },
            max_feeds: self.machine.max_feeds,
            tags: &self.tags,
        };
        generate_in_region(&RotaryRoughing, &input, region, v, &ctx)
    }

    /// Simulates `path` on `stock`, returning the stock and the removed volume.
    fn simulate(&self, stock: DexelStock, path: &GeneratedPath) -> (DexelStock, Vec<usize>) {
        let cancel = CancelToken::new();
        let ctx = JobContext::new(&cancel, &NoProgress);
        let mut sim = Simulator::without_checkpoints(stock, &self.machine);
        let result = sim.run_group(&path.group, &self.profile, &ctx).unwrap();
        let faults = result.rapid_cuts.iter().map(|c| c.segment).collect();
        (sim.into_stock(), faults)
    }
}

/// The remaining material above the target (normal distance) on the part's
/// side surface, between `x0` and `x1`: (least, most).
fn leftover(rig: &Rig, stock: &DexelStock, x0: f64, x1: f64) -> (f64, f64) {
    let d = compare(
        stock,
        &rig.target,
        &DeviationOptions {
            spacing: 0.5,
            ..DeviationOptions::for_cell(CELL)
        },
    )
    .unwrap();
    let mut least = f64::INFINITY;
    let mut most = f64::NEG_INFINITY;
    for p in &d.points {
        let side = p.normal.x.abs() < 0.2;
        if side && (x0..=x1).contains(&p.point.x) {
            least = least.min(p.value);
            most = most.max(p.value);
        }
    }
    (least, most)
}

#[test]
fn req_op_03_continuous_roughing_leaves_a_uniform_stock_to_leave_on_a_cylinder() {
    let part = Cylinder {
        radius: 10.0,
        length: 40.0,
    };
    let rig = Rig::new(&part, 15.0, &tool(ToolShape::FlatEnd, 6.0));
    let t = std::time::Instant::now();
    let path = rig.generate(&rig.stock(), &values(&[])).unwrap();
    eprintln!(
        "generate {:?}, {} segments",
        t.elapsed(),
        path.group.segments.len()
    );
    let t = std::time::Instant::now();
    let (stock, faults) = rig.simulate(rig.stock(), &path);
    eprintln!("simulate {:?}", t.elapsed());
    assert!(faults.is_empty(), "rapid moves cut: {faults:?}");
    let (least, most) = leftover(&rig, &stock, 4.0, 36.0);
    eprintln!("leftover {least:.4} to {most:.4}");
    assert!(least > 0.5 - 2.0 * CELL, "overcut: {least}");
    assert!(most < 0.5 + 2.0 * CELL, "uncut: {most}");
}

fn text(s: &str) -> ParameterValue {
    ParameterValue::Text(s.to_owned())
}

fn short(radius: f64, length: f64) -> Cylinder {
    Cylinder { radius, length }
}

#[test]
fn req_op_03_a_second_roughing_run_over_its_own_result_cuts_nothing() {
    let rig = Rig::new(&short(10.0, 24.0), 15.0, &tool(ToolShape::FlatEnd, 6.0));
    let path = rig.generate(&rig.stock(), &values(&[])).unwrap();
    let (left, _) = rig.simulate(rig.stock(), &path);
    // The stock now stands at the stock to leave: nothing is left to rough.
    let err = rig.generate(&left, &values(&[])).unwrap_err().to_string();
    assert!(err.contains("no cut"), "{err}");
}

#[test]
fn req_op_03_indexed_and_continuous_forms_reach_the_same_envelope() {
    let rig = Rig::new(&short(10.0, 24.0), 15.0, &tool(ToolShape::BallEnd, 3.0));
    let mut ranges = Vec::new();
    for form in ["continuous", "indexed"] {
        let v = values(&[
            (parameter::FORM, text(form)),
            (names::STEPOVER, num(1.0)),
            (parameter::STEPDOWN, num(3.0)),
        ]);
        let path = rig.generate(&rig.stock(), &v).unwrap();
        let (stock, faults) = rig.simulate(rig.stock(), &path);
        assert!(faults.is_empty(), "{form}: rapid moves cut: {faults:?}");
        ranges.push(leftover(&rig, &stock, 4.0, 20.0));
    }
    for (form, (least, most)) in ["continuous", "indexed"].iter().zip(&ranges) {
        eprintln!("{form}: {least:.4} to {most:.4}");
        assert!(*least > 0.5 - 2.0 * CELL, "{form} overcut: {least}");
        assert!(*most < 0.5 + 0.2, "{form} left too much: {most}");
    }
    // The two forms agree on the envelope to within the scallop of the
    // stepover.
    assert!((ranges[0].1 - ranges[1].1).abs() < 0.2, "{ranges:?}");
}

#[test]
fn req_op_03_roughing_follows_a_cone_and_leaves_the_stock_to_leave() {
    let part = Cone {
        start_radius: 8.0,
        end_radius: 12.0,
        length: 30.0,
    };
    let rig = Rig::new(&part, 15.0, &tool(ToolShape::BallEnd, 4.0));
    let v = values(&[(names::STEPOVER, num(1.5)), (parameter::STEPDOWN, num(2.5))]);
    let path = rig.generate(&rig.stock(), &v).unwrap();
    let (stock, faults) = rig.simulate(rig.stock(), &path);
    assert!(faults.is_empty(), "{faults:?}");
    let (least, most) = leftover(&rig, &stock, 5.0, 25.0);
    eprintln!("cone leftover {least:.4} to {most:.4}");
    assert!(least > 0.5 - 2.0 * CELL, "overcut: {least}");
    assert!(most < 0.5 + 0.25, "left too much: {most}");
}

#[test]
fn req_op_14_links_stay_below_the_clearance_cylinder_without_cutting() {
    let rig = Rig::new(&short(10.0, 24.0), 15.0, &tool(ToolShape::BallEnd, 3.0));
    let v = values(&[
        (parameter::FORM, text("indexed")),
        (names::STEPOVER, num(1.0)),
        (parameter::STEPDOWN, num(3.0)),
        (parameter::INDEX_ANGLE, num(60.0)),
    ]);
    let path = rig.generate(&rig.stock(), &v).unwrap();
    let (_, faults) = rig.simulate(rig.stock(), &path);
    assert!(faults.is_empty(), "a link cuts: {faults:?}");
    let rotary = RotaryMapping::of_profile(&rig.machine);
    let clearance = rig.stock_radius + 5.0;
    let low_links = path
        .group
        .segments
        .iter()
        .filter(|s| s.kind == openchuck_types::ir::MoveKind::Rapid)
        .filter(|s| rotary.to_axis(s.end.tip, s.end.a).height < clearance - 1.0)
        .count();
    assert!(
        low_links > 10,
        "links retract to the cylinder every time: {low_links}"
    );
}

#[test]
fn req_op_11_helix_and_ramp_entries_work_in_roughing() {
    let mut rig = Rig::new(&short(10.0, 24.0), 15.0, &tool(ToolShape::FlatEnd, 4.0));
    // Room beyond the part's ends for an entry circle.
    rig.margin = 12.0;
    for style in ["helix", "ramp", "plunge"] {
        let v = values(&[
            ("entry", text(style)),
            (parameter::FORM, text("continuous")),
        ]);
        match rig.generate(&rig.stock(), &v) {
            Ok(path) => {
                let (_, faults) = rig.simulate(rig.stock(), &path);
                assert!(faults.is_empty(), "{style}: {faults:?}");
            }
            Err(e) => panic!("{style}: {e}"),
        }
    }
}

#[test]
fn req_sim_05_leftover_counts_samples_against_a_stock_to_leave() {
    use openchuck_sim::{Deviation, DeviationPoint};
    use openchuck_types::geometry::{Point3, Vec3};
    let point = |value: f64, at_least: bool| DeviationPoint {
        point: Point3::new(value, 0.0, 0.0),
        normal: Vec3::new(0.0, 0.0, 1.0),
        value,
        at_least,
    };
    let points = vec![
        point(0.50, false),
        point(0.52, false),
        point(0.30, false),
        point(0.90, false),
        point(3.0, true),
    ];
    let d = Deviation {
        points,
        worst_remaining: None,
        worst_overcut: None,
        overcut_count: 0,
        overcut_places: Vec::new(),
        overcut_limit: 0.1,
    };
    let l = d.leftover(0.5, 0.05);
    assert_eq!((l.samples, l.below, l.above), (5, 1, 2));
    assert_eq!((l.least, l.most), (0.30, 3.0));
    assert!((l.mean - (0.5 + 0.52 + 0.3 + 0.9 + 3.0) / 5.0).abs() < 1e-12);
    assert_eq!(l.worst_above.unwrap().value, 3.0);
    assert!((l.at_leave() - 0.4).abs() < 1e-12);
    let none = Deviation {
        points: Vec::new(),
        ..d
    };
    let l = none.leftover(0.5, 0.05);
    assert_eq!((l.samples, l.at_leave()), (0, 1.0));
}

/// Roughing a cone within a selected region (OP-02, decision 0040 item 1):
/// the flank under the tool is what the site is tied to, so the edge of the
/// region falls on a slope. Inside the region the stock to leave is uniform,
/// outside the stock is untouched, and across the edge nothing is overcut.
#[test]
fn req_op_02_roughing_a_cone_within_a_region_leaves_the_stock_to_leave_inside_and_stock_outside() {
    let part = Cone {
        start_radius: 8.0,
        end_radius: 12.0,
        length: 30.0,
    };
    let rig = Rig::new(&part, 15.0, &tool(ToolShape::BallEnd, 4.0));
    let edge = 15.0;
    let target = &rig.target;
    let triangles = (0..target.triangles.len())
        .filter(|&t| {
            target
                .triangle(t)
                .unwrap()
                .into_iter()
                .all(|c| c.x >= edge - 1e-6)
        })
        .map(|t| target.triangles[t])
        .collect();
    let selected = MeshSurface::new(&openchuck_types::mesh::TriangleMesh {
        vertices: target.vertices.clone(),
        triangles,
    })
    .unwrap();
    let region = RegionConstraint {
        region: Some(&selected),
        ..Default::default()
    };
    let v = values(&[(names::STEPOVER, num(1.5)), (parameter::STEPDOWN, num(2.5))]);
    let path = rig.generate_in(&rig.stock(), &region, &v).unwrap();
    let (stock, faults) = rig.simulate(rig.stock(), &path);
    assert!(faults.is_empty(), "{faults:?}");

    let (least_in, most_in) = leftover(&rig, &stock, edge + 4.0, 26.0);
    let (least_out, _) = leftover(&rig, &stock, 3.0, edge - 6.0);
    let (least_edge, most_edge) = leftover(&rig, &stock, edge - 6.0, edge + 4.0);
    eprintln!(
        "inside {least_in:.3} to {most_in:.3}; outside from {least_out:.3}; \
         edge band {least_edge:.3} to {most_edge:.3}"
    );
    // The edge falls on the slope, and a 4 mm ball is cut where its contact
    // is in the region: the leftover tapers from the stock to leave at about
    // a tool radius past the edge to the whole stock about a tool diameter
    // before it (measured: 0.61 at 14.4, 0.47 at 15.0, 2.1 at 12.0 for an
    // edge at 15.0).
    let (_, past) = leftover(&rig, &stock, edge + 1.5, edge + 4.0);
    assert!(
        past < 0.5 + 0.25,
        "not at the stock to leave 1.5 mm in: {past}"
    );
    let (before, _) = leftover(&rig, &stock, edge - 3.0, edge - 2.4);
    assert!(
        before > 2.0,
        "cut more than 2.4 mm before the edge: {before}"
    );
    // Inside: the stock to leave, as for the whole part.
    assert!(least_in > 0.5 - 2.0 * CELL, "overcut inside: {least_in}");
    assert!(most_in < 0.5 + 0.25, "left too much inside: {most_in}");
    // Outside: the stock stands, at least 3 mm over a cone of 8 to 12 mm
    // radius in a 15 mm stock.
    assert!(least_out > 2.9, "cut outside the region: {least_out}");
    // Across the edge nothing is cut below the stock to leave.
    assert!(
        least_edge > 0.5 - 2.0 * CELL,
        "overcut at the edge: {least_edge}"
    );
}
