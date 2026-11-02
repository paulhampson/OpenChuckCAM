// SPDX-License-Identifier: AGPL-3.0-or-later
//! The held region is never machined, whatever the stock's shape and
//! whichever strategy cuts (SET-09, NFR-12; GitHub issues #202, #187 and
//! #188; decision record 0160).
//!
//! A cone held at both ends of its stock, as in the owner's trial project: the
//! held region is the stock's own section over the gripped length. The checks
//! are independent of the target the strategies see: each cutting pose of the
//! tool is measured against the held region's exact volume, and the simulated
//! stock must still hold material all through it.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_cmp
)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use openchuck_app::project::id::{OperationId, SetupId, ToolId};
use openchuck_app::project::{
    Item, Operation, OrderKey, ParameterValue, Placement, Setup, ToolSnapshot,
};
use openchuck_app::setup::{HeldRegion, Millimetres, Stock, Workholding, builtin};
use openchuck_app::simulation::SimulationSettings;
use openchuck_app::{Application, Command, Threads};
use openchuck_testkit::parts::{Cone, RadialPart};
use openchuck_types::geometry::{Aabb, Point3, Vec3};
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::tool::{ControllerMapping, CuttingData, Tool, ToolShape};
use openchuck_types::units::{FeedRate, SpindleSpeed};

/// The ball's diameter.
const BALL: f64 = 3.0;
/// The simulation cell, in millimetres.
const CELL: f64 = 0.25;
/// Stock X -12 to 42; the cone part spans X 0 to 30.
const STOCK_X: (f64, f64) = (-12.0, 42.0);
/// The stock's half width across (box) or radius (cylinder).
const HALF: f64 = 12.0;
/// The gripped length at each end.
const GRIP: f64 = 4.0;
/// How far the ball may sink into the held region before it counts as a cut:
/// rounding in the drop and the stock to leave of zero.
const TOLERANCE: f64 = 0.02;

fn cone() -> Cone {
    Cone {
        start_radius: 10.0,
        end_radius: 5.0,
        length: 30.0,
    }
}

fn write_stl(mesh: &TriangleMesh, path: &Path) {
    let mut text = String::from("solid part\n");
    for t in &mesh.triangles {
        let v: Vec<_> = t.iter().map(|i| mesh.vertices[*i as usize]).collect();
        text.push_str("facet normal 0 0 0\n outer loop\n");
        for p in v {
            text.push_str(&format!("  vertex {:e} {:e} {:e}\n", p.x, p.y, p.z));
        }
        text.push_str(" endloop\nendfacet\n");
    }
    text.push_str("endsolid part\n");
    std::fs::write(path, text).unwrap();
}

/// The shape of the stock.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Shape {
    /// A square box, `2 * HALF` across.
    Box,
    /// A cylinder of radius `HALF`.
    Round,
}

impl Shape {
    fn stock(self) -> Stock {
        match self {
            Self::Box => Stock::Box {
                bounds: Aabb {
                    min: Point3::new(STOCK_X.0, -HALF, -HALF),
                    max: Point3::new(STOCK_X.1, HALF, HALF),
                },
            },
            Self::Round => Stock::Cylinder {
                diameter: Millimetres(2.0 * HALF),
                length: Millimetres(STOCK_X.1 - STOCK_X.0),
                x_start: Millimetres(STOCK_X.0),
            },
        }
    }

    /// The held region on this stock with `diameter` (the owner's rule,
    /// decision 0160): a cylinder of the holding diameter where one is given,
    /// else the stock's own section.
    fn held(self, diameter: Option<f64>) -> Region {
        match (diameter, self) {
            (Some(d), _) => Region::Round(d / 2.0),
            (None, Self::Box) => Region::Box,
            (None, Self::Round) => Region::Round(HALF),
        }
    }
}

/// The section of a held region across the axis.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Region {
    /// The box stock's square section, `2 * HALF` across.
    Box,
    /// A circle of this radius about the axis.
    Round(f64),
}

impl Region {
    /// The distance from `(y, z)` to the section (zero inside).
    fn section_distance(self, y: f64, z: f64) -> f64 {
        match self {
            Self::Box => {
                let dy = (y.abs() - HALF).max(0.0);
                let dz = (z.abs() - HALF).max(0.0);
                dy.hypot(dz)
            }
            Self::Round(r) => (y.hypot(z) - r).max(0.0),
        }
    }

    /// True when `(y, z)` is at least `inset` inside the section.
    fn inside_section(self, y: f64, z: f64, inset: f64) -> bool {
        match self {
            Self::Box => y.abs() <= HALF - inset && z.abs() <= HALF - inset,
            Self::Round(r) => y.hypot(z) <= r - inset,
        }
    }
}

/// The two held regions, as X intervals.
fn held_intervals() -> [(f64, f64); 2] {
    [(STOCK_X.0, STOCK_X.0 + GRIP), (STOCK_X.1 - GRIP, STOCK_X.1)]
}

/// How deep a ball of radius `r` centred at `c` is inside the held region:
/// the ball's radius less its centre's distance to the region (positive when
/// it sinks in).
fn sink(region: Region, c: Point3, r: f64) -> f64 {
    held_intervals()
        .iter()
        .map(|&(lo, hi)| {
            let dx = (lo - c.x).max(c.x - hi).max(0.0);
            let across = region.section_distance(c.y, c.z);
            r - dx.hypot(across)
        })
        .fold(f64::NEG_INFINITY, f64::max)
}

struct Built {
    dir: PathBuf,
    app: Application,
    setup: SetupId,
}

impl Drop for Built {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// The cone in oak on `shape` stock, held `GRIP` at both ends with
/// `diameter`, cut by `operations` (strategy, parameters) in order.
fn build(
    name: &str,
    shape: Shape,
    diameter: Option<f64>,
    operations: &[(&str, Vec<(&str, ParameterValue)>)],
) -> Built {
    let dir = std::env::temp_dir().join(format!("openchuck-held-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut app = Application::create(&dir, name, Threads::Exactly(2)).unwrap();
    std::fs::create_dir_all(dir.join("models")).unwrap();
    write_stl(&cone().mesh(61, 72), &dir.join("models/part.stl"));
    let (model, _) = app.add_model(&dir.join("models/part.stl"), None).unwrap();
    let machine = builtin("makera-z1").unwrap().unwrap();
    app.execute(Command::SetMachine(Some(Box::new(machine))))
        .unwrap();
    app.execute(Command::SetSimulation(SimulationSettings {
        cell: Millimetres(CELL),
    }))
    .unwrap();
    let setup_id = SetupId::new();
    let mut setup = Setup::new(setup_id, "Setup 1");
    setup.part = Some(model);
    setup.material = Some("oak".into());
    setup.stock = Some(shape.stock());
    let held = HeldRegion {
        length: Millimetres(GRIP),
        diameter: diameter.map(Millimetres),
    };
    setup.workholding = Workholding {
        start: Some(held),
        end: Some(held),
        tabs: Vec::new(),
    };
    let tool_id = ToolId::new();
    let mut cutting_data = BTreeMap::new();
    cutting_data.insert(
        "oak".to_owned(),
        CuttingData {
            spindle_speed: SpindleSpeed(12000.0),
            feed_per_tooth: Millimetres(0.02),
            plunge_feed: FeedRate(150.0),
            stepover: Millimetres(2.0),
            stepdown: Millimetres(2.0),
        },
    );
    let snapshot = ToolSnapshot {
        id: tool_id,
        order: OrderKey::default(),
        name: "3 mm ball".into(),
        tool: Tool {
            shape: ToolShape::BallEnd,
            diameter: Millimetres(BALL),
            flute_length: Millimetres(8.0),
            overall_length: Millimetres(40.0),
            shank_diameter: Millimetres(BALL),
            flute_count: 2,
            material: None,
            centre_cutting: None,
        },
        controller: ControllerMapping {
            number: 1,
            length_offset: None,
            diameter_offset: None,
        },
        cutting_data,
    };
    let mut items = vec![Item::Setup(setup), Item::Tool(snapshot)];
    for (k, (strategy, parameters)) in operations.iter().enumerate() {
        items.push(Item::Operation(Operation {
            enabled: true,
            id: OperationId::new(),
            order: OrderKey::default(),
            name: format!("Op {}", k + 1),
            setup: setup_id,
            strategy: (*strategy).into(),
            tool: Some(tool_id),
            selections: Vec::new(),
            boundary: None,
            avoid: Vec::new(),
            check: Vec::new(),
            restriction_options: Default::default(),
            spindle_speed: None,
            material: None,
            parameters: free_ends_off(strategy)
                .into_iter()
                .chain(parameters.iter().map(|(k, v)| ((*k).to_owned(), v.clone())))
                .collect::<BTreeMap<_, _>>(),
        }));
    }
    for item in items {
        app.execute(Command::Add {
            item,
            placement: Placement::End,
        })
        .unwrap();
    }
    Built {
        dir,
        app,
        setup: setup_id,
    }
}

fn rough() -> (&'static str, Vec<(&'static str, ParameterValue)>) {
    (
        "rotary_roughing",
        vec![("stock_to_leave", ParameterValue::Number(0.5))],
    )
}

/// What a run did to the held regions: the deepest any pose of the ball (at
/// feed or rapid) sank into them, and how many sample points inside them lost their
/// material in the simulation (of how many checked).
#[derive(Debug)]
struct Outcome {
    deepest: f64,
    deepest_at: Option<Point3>,
    lost: usize,
    checked: usize,
    first_lost: Option<Point3>,
}

fn run(b: &Built, region: Region) -> Outcome {
    let run = b.app.simulate().unwrap();
    let r = BALL / 2.0;
    let mut deepest = f64::NEG_INFINITY;
    let mut deepest_at = None;
    for group in &run.toolpath.groups {
        let mut previous: Option<(Point3, Vec3)> = None;
        for s in &group.segments {
            let (end, a) = (s.end.tip, s.end.axis.get());
            // The ball's centre is one radius up the tool axis from the tip;
            // both ends of every move are measured, rapids and retracts
            // included, each with its own axis, and the middle of each move at
            // feed. A rapid may turn A far, so its straight midpoint in model
            // space is not on its path, and only its ends are.
            let (start, a0) = previous.unwrap_or((end, a));
            let mut poses = vec![(start, a0), (end, a)];
            if s.kind.is_fed() {
                poses.push((start + (end - start) * 0.5, a));
            }
            for (tip, axis) in poses {
                let d = sink(region, tip + axis * r, r);
                if d > deepest {
                    deepest = d;
                    deepest_at = Some(tip);
                }
            }
            previous = Some((end, a));
        }
    }
    let report = run.simulation.unwrap();
    // Links and retracts keep clear too: no rapid move removes material.
    for op in &report.setups[0].operations {
        assert!(
            op.rapid_cuts.is_empty(),
            "{}: {:?}",
            op.operation,
            op.rapid_cuts
        );
    }
    let stock = &report.setups[0].stock;
    let (mut lost, mut checked, mut first_lost) = (0, 0, None);
    let inset = 3.0 * CELL;
    for (lo, hi) in held_intervals() {
        let mut x = lo + inset;
        while x <= hi - inset {
            for iy in -24..=24 {
                for iz in -24..=24 {
                    let (y, z) = (f64::from(iy) * 0.5, f64::from(iz) * 0.5);
                    if !region.inside_section(y, z, inset) {
                        continue;
                    }
                    let p = Point3::new(x, y, z);
                    checked += 1;
                    if !stock.contains_point(p) {
                        lost += 1;
                        first_lost.get_or_insert(p);
                    }
                }
            }
            x += 0.5;
        }
    }
    Outcome {
        deepest,
        deepest_at,
        lost,
        checked,
        first_lost,
    }
}

fn assert_kept(name: &str, o: &Outcome) {
    assert!(o.checked > 1000, "{name}: {} points checked", o.checked);
    assert!(
        o.deepest <= TOLERANCE,
        "{name}: a cutting move sinks {:.3} mm into the held region at {:?}",
        o.deepest,
        o.deepest_at
    );
    assert_eq!(
        o.lost, 0,
        "{name}: the simulation removed material from the held region, first at {:?}",
        o.first_lost
    );
}

/// The owner's project (#202, #187): box stock held at both ends with a
/// holding diameter equal to the stock's width.
#[test]
fn req_set_09_req_nfr_12_roughing_never_enters_the_held_region_at_either_end() {
    // By the owner's rule the held region is the cylinder of the holding
    // diameter; the box's corners outside it are not part of it (decision
    // 0160), but no tool reaches over the gripped length either (0161).
    let b = build("rough-box-dia", Shape::Box, Some(2.0 * HALF), &[rough()]);
    assert_kept(
        "box stock, holding diameter",
        &run(&b, Shape::Box.held(Some(2.0 * HALF))),
    );
}

/// Box stock with no holding diameter is held at its own outline (#188):
/// generation is not refused, and roughing leaves the whole held region.
#[test]
fn req_set_09_box_stock_is_held_at_its_own_outline_over_the_gripped_length() {
    let b = build("rough-box", Shape::Box, None, &[rough()]);
    assert_kept(
        "box stock, no holding diameter",
        &run(&b, Shape::Box.held(None)),
    );
}

/// How far the tool's body reaches into the gripped length along the axis:
/// the tool is across the rotary axis, so its body spans the tip's X plus
/// and minus its radius (decision record 0161).
fn reach_into_grip(b: &Built) -> (f64, Option<Point3>) {
    let run = b.app.simulate().unwrap();
    let r = BALL / 2.0;
    let mut worst = (f64::NEG_INFINITY, None);
    for group in &run.toolpath.groups {
        for s in group.segments.iter().filter(|s| s.kind.is_fed()) {
            let x = s.end.tip.x;
            for (lo, hi) in held_intervals() {
                // Overlap of [x - r, x + r] with [lo, hi].
                let overlap = (x + r).min(hi) - (x - r).max(lo);
                if overlap > worst.0 {
                    worst = (overlap, Some(s.end.tip));
                }
            }
        }
    }
    worst
}

/// Roughing, then `strategy`, on round stock and on box stock: no cutting
/// move of either reaches over the gripped length, and the simulated stock
/// keeps the whole held region (SET-09, NFR-12; #202).
fn keeps_clear(strategy: &str, parameters: Vec<(&str, ParameterValue)>) {
    keeps_clear_in_folder(strategy, strategy, parameters);
}

/// [`keeps_clear`] with the project folder named after `folder`, so tests of
/// one strategy that run at the same time do not share a folder.
fn keeps_clear_in_folder(folder: &str, strategy: &str, parameters: Vec<(&str, ParameterValue)>) {
    for shape in [Shape::Round, Shape::Box] {
        let name = format!("{strategy} on {shape:?} stock");
        let b = build(
            &format!("{folder}-{shape:?}"),
            shape,
            None,
            &[rough(), (strategy, parameters.clone())],
        );
        let (overlap, at) = reach_into_grip(&b);
        assert!(
            overlap <= TOLERANCE,
            "{name}: a cutting move reaches {overlap:.3} mm over the gripped length at {at:?}"
        );
        assert_kept(&name, &run(&b, shape.held(None)));
    }
}

#[test]
fn req_set_09_req_nfr_12_roughing_keeps_clear_of_the_held_region() {
    let (strategy, parameters) = rough();
    keeps_clear(strategy, parameters);
}

/// Roughing in a cut direction issue 246 added keeps clear as the forms' own
/// directions do (decision record 0212 part A item 4).
fn keeps_clear_in(form: &str, direction: &str) {
    keeps_clear_in_folder(
        &format!("rotary_roughing-{form}"),
        "rotary_roughing",
        vec![
            ("stock_to_leave", ParameterValue::Number(0.5)),
            ("form", ParameterValue::Text(form.to_owned())),
            ("direction", ParameterValue::Text(direction.to_owned())),
        ],
    );
}

#[test]
fn req_set_09_req_nfr_12_req_op_03_continuous_roughing_along_the_length_keeps_clear_of_the_held_region()
 {
    keeps_clear_in("continuous", "along the length of the part");
}

#[test]
fn req_set_09_req_nfr_12_req_op_03_indexed_roughing_around_the_part_keeps_clear_of_the_held_region()
{
    keeps_clear_in("indexed", "around the part");
}

#[test]
fn req_set_09_req_nfr_12_helical_finishing_keeps_clear_of_the_held_region() {
    keeps_clear(
        "helical_finishing",
        vec![("max_angle_step", ParameterValue::Number(5.0))],
    );
}

#[test]
fn req_set_09_req_nfr_12_axial_finishing_keeps_clear_of_the_held_region() {
    keeps_clear("axial_finishing", Vec::new());
}

#[test]
fn req_set_09_req_nfr_12_indexed_finishing_keeps_clear_of_the_held_region() {
    keeps_clear("indexed_finishing", Vec::new());
}

#[test]
fn req_set_09_req_nfr_12_simultaneous_finishing_keeps_clear_of_the_held_region() {
    keeps_clear("simultaneous_finishing", Vec::new());
}

/// Simultaneous finishing beside a held start with the default tabs (OI-1372,
/// issue #411; decision record 0363 item 1): the drop surface holds the tab
/// from the held region's inner edge, so a spiral over the whole drop surface
/// drove inside the held keep-out band, rode past the held stock's face and
/// was refused ("would cut ... into the surface" a tool radius from that
/// face). The spiral now starts and ends outside the band, as the faces' turns
/// do, so the operation generates and keeps clear of the held regions.
#[test]
fn req_set_09_req_op_05_simultaneous_finishing_beside_a_held_start_with_tabs_generates() {
    for shape in [Shape::Round, Shape::Box] {
        let name = format!("simultaneous finishing with tabs on {shape:?} stock");
        let mut b = build(
            &format!("simultaneous-tabs-{shape:?}"),
            shape,
            None,
            &[(
                "simultaneous_finishing",
                vec![("stepover", ParameterValue::Number(1.0))],
            )],
        );
        let tabs = b.app.start_default_tabs(b.setup).wait().unwrap();
        assert!(!tabs.is_empty(), "{name}: no default tabs");
        let mut setup = b.app.project().setup(b.setup).unwrap().clone();
        setup.workholding.tabs = tabs;
        b.app.execute(Command::Replace(Item::Setup(setup))).unwrap();
        let (overlap, at) = reach_into_grip(&b);
        assert!(
            overlap <= TOLERANCE,
            "{name}: a cutting move reaches {overlap:.3} mm over the gripped length at {at:?}"
        );
        assert_kept(&name, &run(&b, shape.held(None)));
    }
}

#[test]
fn req_set_09_req_nfr_12_rest_machining_keeps_clear_of_the_held_region() {
    keeps_clear("rest_machining", Vec::new());
}

#[test]
fn req_set_09_req_nfr_12_edge_following_keeps_clear_of_the_held_region() {
    keeps_clear("edge_following", Vec::new());
}

/// What is drawn is what is used (#187): the held region in the scene is the
/// stock's own section over the gripped length, for box stock and for round
/// stock, or the cylinder of the holding diameter where one is given, on any
/// stock (the owner's rule, decision 0160).
#[test]
fn req_set_09_the_held_region_drawn_is_the_stocks_section() {
    use openchuck_app::scene::SceneRole;
    for (shape, diameter, half) in [
        (Shape::Box, None, HALF),
        (Shape::Round, None, HALF),
        (Shape::Round, Some(16.0), 8.0),
        (Shape::Box, Some(16.0), 8.0),
    ] {
        let b = build(
            &format!("scene-{shape:?}-{diameter:?}"),
            shape,
            diameter,
            &[],
        );
        let scene = b.app.start_scene(Some(b.setup)).wait().unwrap();
        let held: Vec<_> = scene
            .meshes
            .iter()
            .filter(|m| m.role == SceneRole::HeldRegion)
            .collect();
        assert_eq!(held.len(), 2, "{shape:?}");
        for (mesh, (lo, hi)) in held.iter().zip(held_intervals()) {
            let v = &mesh.mesh.vertices;
            let min = |f: fn(&Point3) -> f64| v.iter().map(f).fold(f64::INFINITY, f64::min);
            let max = |f: fn(&Point3) -> f64| v.iter().map(f).fold(f64::NEG_INFINITY, f64::max);
            let near = |a: f64, b: f64| (a - b).abs() < 1e-6;
            assert!(
                near(min(|p| p.x), lo) && near(max(|p| p.x), hi),
                "{shape:?}"
            );
            assert!(
                near(max(|p| p.y), half) && near(min(|p| p.y), -half),
                "{shape:?}"
            );
            assert!(
                near(max(|p| p.z), half) && near(min(|p| p.z), -half),
                "{shape:?}"
            );
            if shape == Shape::Box && diameter.is_none() {
                // The box's corners, not a circle inside it.
                assert!(v.iter().any(|p| near(p.y, HALF) && near(p.z, HALF)));
            }
        }
    }
}

/// Beside a held region, roughing clears the stock all round, as it does
/// beyond the part's ends: a helix cut short at the keep-out turns once there
/// first, so no wedge of full-height stock is left for a later operation to
/// meet at full depth (SET-09, OP-03, NFR-12; decision record 0161).
#[test]
fn req_set_09_req_op_03_roughing_clears_the_stock_beside_the_held_region_all_round() {
    let b = build("rough-wedge", Shape::Round, None, &[rough()]);
    let run = b.app.simulate().unwrap();
    let report = run.simulation.unwrap();
    let stock = &report.setups[0].stock;
    // The keep-out of the start's held region ends at X -8 + 1.5 + 0.5; the
    // tool's footprint reaches 1.5 back from there, to X -7.5. Up to the
    // cone (X 0) there is no part, so roughing goes down to its last layer;
    // at 11 mm from the axis every point between must be gone.
    let mut left = Vec::new();
    let mut checked = 0;
    for ix in 0..=10 {
        let x = -7.2 + 0.5 * f64::from(ix);
        for k in 0..72 {
            let angle = std::f64::consts::TAU * f64::from(k) / 72.0;
            let p = Point3::new(x, 11.0 * angle.cos(), 11.0 * angle.sin());
            checked += 1;
            if stock.contains_point(p) {
                left.push(p);
            }
        }
    }
    assert_eq!(checked, 11 * 72);
    assert!(
        left.is_empty(),
        "{} of {checked} points beside the held region still hold stock, first {:?}",
        left.len(),
        left.first()
    );
}

/// A ramp entry that would run over a held region is refused with a message
/// that names the held region and the way out, so the user sees why a ramp
/// beside the part fails (SET-09, NFR-07; decision records 0114, 0161; the
/// owner's decision on OI-553).
#[test]
fn req_set_09_req_nfr_07_a_ramp_over_the_held_region_is_refused_naming_it() {
    let ramp = (
        "helical_finishing",
        vec![
            ("max_angle_step", ParameterValue::Number(5.0)),
            ("entry", ParameterValue::Text("ramp".to_owned())),
            ("ramp_angle", ParameterValue::Number(3.0)),
        ],
    );
    let b = build("ramp", Shape::Round, None, &[rough(), ramp]);
    let error = b.app.regenerate().unwrap_err().to_string();
    assert!(
        error.contains("would run over the held region at the start of the stock")
            && error.contains("use a plunge entry"),
        "{error}"
    );
}

/// Face free ends off for the strategies that have it: these tests are not
/// about free ends, and a new operation faces them (decision record 0344,
/// OI-1396).
fn free_ends_off(strategy: &str) -> BTreeMap<String, ParameterValue> {
    let mut out = BTreeMap::new();
    if ["helical_finishing", "axial_finishing", "rest_machining"].contains(&strategy) {
        out.insert("face_free_ends".to_owned(), ParameterValue::Flag(false));
    }
    // Adaptive spacing off too: not what this checks, and a new operation
    // has it on (decision record 0239).
    if [
        "rotary_roughing",
        "helical_finishing",
        "axial_finishing",
        "indexed_finishing",
        "simultaneous_finishing",
    ]
    .contains(&strategy)
    {
        out.insert("adaptive_spacing".to_owned(), ParameterValue::Flag(false));
    }
    out
}
