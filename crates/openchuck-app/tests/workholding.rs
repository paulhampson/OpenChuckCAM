// SPDX-License-Identifier: AGPL-3.0-or-later
//! Workholding and retaining tabs through the application: the held regions
//! and tabs join the target of every operation, the simulation keeps them and
//! the deviation report counts them as part (SET-09, OP-21; work breakdown
//! slice 21c; decision records 0035 and 0045).

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
use openchuck_app::setup::{
    HeldRegion, Millimetres, Stock, Tab, TabLayout, TabShape, Workholding, builtin,
};
use openchuck_app::simulation::SimulationSettings;
use openchuck_app::{Application, Command, PostSelection, Threads};
use openchuck_testkit::gouge::{Outline, Pose, max_gouge};
use openchuck_testkit::parts::{Cone, RadialPart};
use openchuck_types::ir::Toolpath;
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::tool::{ControllerMapping, CuttingData, Tool, ToolShape};
use openchuck_types::units::{FeedRate, SpindleSpeed};

const BALL: f64 = 3.0;
/// The simulation cell, in millimetres.
const CELL: f64 = 0.25;
/// Stock: Ø24, X -12 to 42; the cone part spans X 0 to 30.
const STOCK_X: (f64, f64) = (-12.0, 42.0);

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
        let n = {
            let c = (v[1] - v[0]).cross(v[2] - v[0]);
            let len = c.length();
            if len > 0.0 { c * (1.0 / len) } else { c }
        };
        text.push_str(&format!(
            "facet normal {:e} {:e} {:e}\n outer loop\n",
            n.x, n.y, n.z
        ));
        for p in v {
            text.push_str(&format!("  vertex {:e} {:e} {:e}\n", p.x, p.y, p.z));
        }
        text.push_str(" endloop\nendfacet\n");
    }
    text.push_str("endsolid part\n");
    std::fs::write(path, text).unwrap();
}

fn tab(x_start: f64, length: f64, small: f64, large: f64, small_at_start: bool) -> Tab {
    let (a, b) = if small_at_start {
        (small, large)
    } else {
        (large, small)
    };
    Tab {
        x_start: Millimetres(x_start),
        length: Millimetres(length),
        shape: TabShape::Frustum {
            start_diameter: Millimetres(a),
            end_diameter: Millimetres(b),
        },
        layout: TabLayout::OnAxis,
    }
}

/// A held region of 4 mm at the start, a 10 to 20 mm frustum from the held
/// region to the part (small end on the part), a 5 to 10 mm frustum from the
/// part to the stock's end, and a 2 mm held region at the end of the stock.
fn workholding() -> Workholding {
    Workholding {
        start: Some(HeldRegion {
            length: Millimetres(4.0),
            diameter: None,
        }),
        end: Some(HeldRegion {
            length: Millimetres(2.0),
            diameter: Some(Millimetres(20.0)),
        }),
        tabs: vec![
            // Head: X -8 to 0, large end at the held region.
            tab(-8.0, 8.0, 10.0, 20.0, false),
            // Tail: X 30 to 40, small end on the part.
            tab(30.0, 10.0, 5.0, 10.0, true),
        ],
    }
}

struct Built {
    dir: PathBuf,
    app: Application,
    setup: SetupId,
}

/// The cone in oak with the workholding, roughed and then finished.
fn build(name: &str, workholding: Workholding, threads: usize, tool_diameter: f64) -> Built {
    build_with_mesh(name, workholding, threads, tool_diameter, (121, 144), true)
}

/// [`build`] with the part's mesh at `mesh` stations along X and around.
fn build_with_mesh(
    name: &str,
    workholding: Workholding,
    threads: usize,
    tool_diameter: f64,
    mesh: (u32, u32),
    rough: bool,
) -> Built {
    let dir = std::env::temp_dir().join(format!("openchuck-wh-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut app = Application::create(&dir, name, Threads::Exactly(threads)).unwrap();
    std::fs::create_dir_all(dir.join("models")).unwrap();
    write_stl(&cone().mesh(mesh.0, mesh.1), &dir.join("models/part.stl"));
    let (model, _) = app.add_model(&dir.join("models/part.stl"), None).unwrap();
    let machine = builtin("makera-z1").unwrap().unwrap();
    app.execute(Command::SetMachine(Some(Box::new(machine))))
        .unwrap();
    // A coarse cell keeps the debug-build runs short; the checks below allow
    // for it.
    app.execute(Command::SetSimulation(SimulationSettings {
        cell: Millimetres(CELL),
    }))
    .unwrap();
    let setup_id = SetupId::new();
    let mut setup = Setup::new(setup_id, "Setup 1");
    setup.part = Some(model);
    setup.material = Some("oak".into());
    setup.stock = Some(Stock::Cylinder {
        diameter: Millimetres(24.0),
        length: Millimetres(STOCK_X.1 - STOCK_X.0),
        x_start: Millimetres(STOCK_X.0),
    });
    setup.workholding = workholding;
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
        name: format!("{tool_diameter} mm ball"),
        tool: Tool {
            shape: ToolShape::BallEnd,
            diameter: Millimetres(tool_diameter),
            flute_length: Millimetres(8.0),
            overall_length: Millimetres(40.0),
            shank_diameter: Millimetres(tool_diameter),
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
    let operation = |name: &str, strategy: &str, parameters: Vec<(&str, ParameterValue)>| {
        Item::Operation(Operation {
            enabled: true,
            id: OperationId::new(),
            order: OrderKey::default(),
            name: name.into(),
            setup: setup_id,
            strategy: strategy.into(),
            tool: Some(tool_id),
            selections: Vec::new(),
            boundary: None,
            avoid: Vec::new(),
            check: Vec::new(),
            parameters: parameters
                .into_iter()
                .map(|(k, v)| (k.to_owned(), v))
                .collect::<BTreeMap<_, _>>(),
        })
    };
    let mut items = vec![
        Item::Setup(setup),
        Item::Tool(snapshot),
        operation(
            "Finish",
            "helical_finishing",
            vec![("max_angle_step", ParameterValue::Number(5.0))],
        ),
    ];
    if rough {
        items.insert(
            2,
            operation(
                "Rough",
                "rotary_roughing",
                vec![("stock_to_leave", ParameterValue::Number(0.5))],
            ),
        );
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

/// The union of the cone and the workholding as a surface of revolution:
/// the independent checker's exact surface for the composite target.
struct Held {
    workholding: Workholding,
}

impl Held {
    fn tab_radius(t: &Tab, x: f64) -> Option<f64> {
        let (lo, hi) = (t.x_start.0, t.x_start.0 + t.length.0);
        if x < lo || x > hi {
            return None;
        }
        let TabShape::Frustum {
            start_diameter,
            end_diameter,
        } = t.shape
        else {
            return None;
        };
        let f = (x - lo) / (hi - lo);
        Some((start_diameter.0 + f * (end_diameter.0 - start_diameter.0)) / 2.0)
    }

    fn radius(&self, x: f64) -> f64 {
        let mut r: f64 = 0.0;
        if (0.0..=30.0).contains(&x) {
            r = r.max(cone().radius_at(x, 0.0));
        }
        if let Some(h) = self.workholding.start
            && x <= STOCK_X.0 + h.length.0
        {
            r = r.max(h.diameter.map_or(12.0, |d| d.0 / 2.0));
        }
        if let Some(h) = self.workholding.end
            && x >= STOCK_X.1 - h.length.0
        {
            r = r.max(h.diameter.map_or(12.0, |d| d.0 / 2.0));
        }
        for t in &self.workholding.tabs {
            if let Some(tr) = Self::tab_radius(t, x) {
                r = r.max(tr);
            }
        }
        r
    }
}

impl RadialPart for Held {
    fn name(&self) -> &'static str {
        "cone with workholding"
    }
    fn x_range(&self) -> (f64, f64) {
        STOCK_X
    }
    fn radius_at(&self, x: f64, _angle: f64) -> f64 {
        self.radius(x)
    }
}

fn fed_poses(toolpath: &Toolpath, group: usize) -> Vec<Pose> {
    toolpath.groups[group]
        .segments
        .iter()
        .filter(|s| s.kind.is_fed())
        .map(|s| Pose {
            tip: s.end.tip,
            axis: s.end.axis.get(),
        })
        .collect()
}

#[test]
fn req_set_09_req_op_21_operations_never_cut_a_held_region_or_a_tab() {
    let b = build("never-cut", workholding(), 2, BALL);
    let run = b.app.simulate().unwrap();
    let outline = Outline::new(
        &Tool {
            shape: ToolShape::BallEnd,
            diameter: Millimetres(BALL),
            flute_length: Millimetres(8.0),
            overall_length: Millimetres(40.0),
            shank_diameter: Millimetres(BALL),
            flute_count: 2,
            material: None,
            centre_cutting: None,
        }
        .profile()
        .unwrap(),
    );
    let part = Held {
        workholding: workholding(),
    };
    // The independent checker, on the exact surface of the part with the
    // held regions and tabs: no pose of either operation goes into it.
    for group in 0..2 {
        let poses = fed_poses(&run.toolpath, group);
        assert!(poses.len() > 100, "group {group}: {} poses", poses.len());
        let gouge = max_gouge(&outline, &part, &poses, 0.25);
        println!(
            "group {group}: {} poses, gouge {:.4} mm",
            poses.len(),
            gouge.depth
        );
        assert!(
            gouge.depth <= 0.02,
            "group {group}: gouge {:.4} mm at {:?}",
            gouge.depth,
            gouge.at
        );
    }
    // The simulated stock keeps them: points well inside each solid are
    // still material, and no rapid move cut anything (links and retracts
    // keep clear).
    let report = run.simulation.unwrap();
    let setup = &report.setups[0];
    for op in &setup.operations {
        assert!(
            op.rapid_cuts.is_empty(),
            "{}: {:?}",
            op.operation,
            op.rapid_cuts
        );
    }
    let cell = setup.stock.cell().0;
    let mut checked = 0;
    for step in 0..=108 {
        let x = STOCK_X.0 + 0.5 * f64::from(step);
        if x > STOCK_X.1 {
            break;
        }
        if (0.5..=29.5).contains(&x) {
            continue; // the cone is cut to its own shape
        }
        let r = part.radius(x) - 3.0 * cell;
        for k in 0..24 {
            let angle = std::f64::consts::TAU * f64::from(k) / 24.0;
            let p = openchuck_types::geometry::Point3::new(x, r * angle.cos(), r * angle.sin());
            // Inside the solid's radius and not within a cell of the part's
            // own surface or a shoulder.
            if r > 0.0 && near_solid_interior(x, r) {
                assert!(
                    setup.stock.contains_point(p),
                    "material is gone at {p:?} (radius {r:.2} at X {x})"
                );
                checked += 1;
            }
        }
    }
    assert!(checked > 500, "{checked}");
    std::fs::remove_dir_all(&b.dir).unwrap();
}

/// True when `(x, r)` is at least 0.5 mm inside the held regions and tabs
/// along X, away from the faces where the solids end.
fn near_solid_interior(x: f64, _r: f64) -> bool {
    let w = workholding();
    let inside = |lo: f64, hi: f64| x >= lo + 0.5 && x <= hi - 0.5;
    inside(STOCK_X.0, STOCK_X.0 + 4.0)
        || inside(STOCK_X.1 - 2.0, STOCK_X.1)
        || w.tabs
            .iter()
            .any(|t| inside(t.x_start.0, t.x_start.0 + t.length.0))
}

#[test]
fn req_set_09_req_op_21_the_deviation_report_counts_them_as_part() {
    let b = build("deviation", workholding(), 2, BALL);
    let run = b.app.simulate().unwrap();
    let report = run.simulation.unwrap();
    let d = report.setups[0].deviation.as_ref().unwrap();
    // Nothing is overcut, including on the held regions and tabs.
    assert_eq!(
        d.overcut_count,
        0,
        "overcut at {:?}",
        d.overcut_places.first().map(|p| p.point)
    );
    // Samples lie on the held regions and the tabs, not only on the cone.
    let on_solids = d
        .points
        .iter()
        .filter(|p| p.point.x < -0.5 || p.point.x > 30.5)
        .count();
    assert!(on_solids > 1000, "{on_solids}");
    // Where they show, the stock is the solid within a cell or two. A face
    // inside the union (the tab against the part, the held region against
    // the tab) would read as material left, as deep as the solid is thick,
    // so none may be there. Within 4 mm of a shoulder the 3 mm ball cannot
    // reach into the inside corner, so material is left there by the tool
    // (not by the target); those samples are not judged.
    let shoulders = [-8.0, 0.0, 30.0, 40.0];
    let mut shown: Vec<_> = d
        .points
        .iter()
        .filter(|p| p.point.x < -0.5 || p.point.x > 30.5)
        .filter(|p| shoulders.iter().all(|s| (p.point.x - s).abs() > 4.0))
        .collect();
    assert!(shown.len() > 100, "{}", shown.len());
    shown.sort_by(|a, b| b.value.abs().total_cmp(&a.value.abs()));
    for p in shown.iter().take(4) {
        println!(
            "worst: {:.3} at {:?} normal {:?}",
            p.value, p.point, p.normal
        );
    }
    let worst = shown[0].value.abs();
    // The scallop of a 3 mm ball at a 2 mm stepover is 0.38 mm at most.
    assert!(worst <= 0.4, "{worst}");
    std::fs::remove_dir_all(&b.dir).unwrap();
}

#[test]
fn req_gc_07_req_nfr_06_workholding_gives_the_same_gcode_at_any_thread_count() {
    let mut first = None;
    for threads in [1, 2, 4] {
        // A coarser mesh: this builds the project three times.
        let mut b = build_with_mesh(
            &format!("threads-{threads}"),
            workholding(),
            threads,
            BALL,
            (61, 72),
            false,
        );
        b.app.set_post(Some(PostSelection::parse("makera-z1")));
        let text: Vec<(String, String)> = b.app.regenerate().unwrap().gcode;
        // The operations' IDs are in comments and differ between builds.
        let stable: Vec<String> = text
            .iter()
            .flat_map(|(_, t)| t.lines())
            .filter(|l| !l.starts_with("(operation ") && !l.starts_with("(threads-"))
            .map(str::to_owned)
            .collect();
        match &first {
            None => first = Some(stable),
            Some(f) => assert_eq!(f, &stable, "at {threads} threads"),
        }
        std::fs::remove_dir_all(&b.dir).unwrap();
    }
}

#[test]
fn req_op_21_a_tab_outside_the_end_zones_or_narrower_than_the_tool_is_refused() {
    // Across the middle of the part.
    let mut w = workholding();
    w.tabs = vec![tab(10.0, 5.0, 6.0, 8.0, true)];
    let b = build("zone", w, 1, BALL);
    let error = b.app.regenerate().unwrap_err().to_string();
    assert!(error.contains("head or the tail zone"), "{error}");
    std::fs::remove_dir_all(&b.dir).unwrap();
    // Narrower than a 6 mm tool at its small end (5 mm).
    let b = build("narrow", workholding(), 1, 6.0);
    let error = b.app.regenerate().unwrap_err().to_string();
    assert!(error.contains("narrower than the tool"), "{error}");
    assert!(error.contains("tab 2"), "{error}");
    std::fs::remove_dir_all(&b.dir).unwrap();
}

#[test]
fn req_op_21_default_tabs_follow_the_owners_answers() {
    // Held 4 mm at the start only.
    let mut w = workholding();
    w.tabs.clear();
    w.end = None;
    let b = build("defaults", w, 1, BALL);
    let tabs = b.app.start_default_tabs(b.setup).wait().unwrap();
    assert_eq!(tabs.len(), 2, "{tabs:?}");
    // Head: from the held region's inner edge (X -8) to the part (X 0), small
    // end on the part: the smaller of 10 mm and half the part's diameter
    // there (the radius, 10 mm), large end twice that.
    let head = tabs[0];
    assert_eq!(head.x_start.0, -8.0);
    assert!((head.length.0 - 8.0).abs() < 1e-9);
    // The STL holds single precision, so the diameters match to 1e-5.
    let near = |shape: TabShape, a: f64, b: f64| {
        let TabShape::Frustum {
            start_diameter,
            end_diameter,
        } = shape
        else {
            panic!("not a frustum: {shape:?}");
        };
        assert!(
            (start_diameter.0 - a).abs() < 1e-5 && (end_diameter.0 - b).abs() < 1e-5,
            "{shape:?} is not {a} to {b}"
        );
    };
    near(head.shape, 20.0, 10.0);
    // Tail: from the part's end (X 30) to the end of the stock (X 42); the
    // part is 10 mm across there (radius 5).
    let tail = tabs[1];
    assert_eq!(tail.x_start.0, 30.0);
    assert!((tail.length.0 - 12.0).abs() < 1e-9);
    near(tail.shape, 5.0, 10.0);
    assert_eq!(head.layout, TabLayout::OnAxis);
    // Adding them is one undoable edit, and they then pass the checks.
    let mut app = b.app;
    let mut setup = app.project().setup(b.setup).unwrap().clone();
    setup.workholding.tabs = tabs;
    app.execute(Command::Replace(Item::Setup(setup))).unwrap();
    app.regenerate().unwrap();
    assert!(app.undo().unwrap());
    assert!(
        app.project()
            .setup(b.setup)
            .unwrap()
            .workholding
            .tabs
            .is_empty()
    );
    // With tabs already there, defaults are refused.
    assert!(app.redo().unwrap());
    let error = app
        .start_default_tabs(b.setup)
        .wait()
        .unwrap_err()
        .to_string();
    assert!(error.contains("already has tabs"), "{error}");
    std::fs::remove_dir_all(&b.dir).unwrap();
}

/// The default tabs (decision 0045 item 3) put a tab's small end on the
/// part's end face, so the tool meets a wall there. Path assembly merges the
/// micro-moves path fitting leaves at a wall (decision 0050, closing
/// OI-209), so the job passes gates 1 and 2 with both posts and gives the
/// same G-code at 1, 2 and N threads.
#[test]
fn req_op_13_req_op_14_req_gc_07_req_nfr_06_default_tabs_pass_gates_1_and_2() {
    let mut w = workholding();
    w.tabs.clear();
    let mut b = build_with_mesh("default-gates", w, 2, BALL, (121, 144), false);
    let tabs = b.app.start_default_tabs(b.setup).wait().unwrap();
    assert_eq!(tabs.len(), 2, "{tabs:?}");
    // The head tab's small end (10 mm) sits on the part's 20 mm end: a wall.
    let TabShape::Frustum { end_diameter, .. } = tabs[0].shape else {
        panic!("not a frustum: {:?}", tabs[0].shape);
    };
    assert!(end_diameter.0 < 19.0, "{tabs:?}");
    let mut setup = b.app.project().setup(b.setup).unwrap().clone();
    setup.workholding.tabs = tabs;
    b.app.execute(Command::Replace(Item::Setup(setup))).unwrap();
    let stable = |text: &[(String, String)]| -> Vec<String> {
        text.iter()
            .flat_map(|(_, t)| t.lines())
            .filter(|l| !l.starts_with("(operation ") && !l.starts_with("(default-gates"))
            .map(str::to_owned)
            .collect()
    };
    let mut first = None;
    for post in ["makera-z1", "linuxcnc"] {
        b.app.set_post(Some(PostSelection::parse(post)));
        let result = b.app.regenerate().unwrap();
        for report in &result.reports {
            // Before the merge, gate 1 found `F600.1` against Z's limit and
            // gate 2 a move 7 % off the IR's time on this layout (OI-209).
            assert!(
                report.is_clean(),
                "{post}: {:?} {:?}",
                &report.findings[..report.findings.len().min(5)],
                report.comparison
            );
        }
        if first.is_none() {
            first = Some(stable(&result.gcode));
        }
    }
    b.app.save().unwrap();
    let first = first.unwrap();
    for threads in openchuck_testkit::determinism::thread_counts() {
        let mut again = Application::open(&b.dir, Threads::Exactly(threads)).unwrap();
        again.set_post(Some(PostSelection::parse("makera-z1")));
        assert_eq!(
            stable(&again.regenerate().unwrap().gcode),
            first,
            "at {threads} threads"
        );
    }
    std::fs::remove_dir_all(&b.dir).unwrap();
}

#[test]
fn req_set_09_req_op_21_the_scene_shows_them() {
    let b = build("scene", workholding(), 1, BALL);
    let scene = b.app.start_scene(Some(b.setup)).wait().unwrap();
    use openchuck_app::scene::SceneRole;
    let held = scene
        .meshes
        .iter()
        .filter(|m| m.role == SceneRole::HeldRegion)
        .count();
    let tabs = scene
        .meshes
        .iter()
        .filter(|m| m.role == SceneRole::Tab)
        .count();
    assert_eq!((held, tabs), (2, 2));
    std::fs::remove_dir_all(&b.dir).unwrap();
}

#[test]
fn req_op_21_tabs_around_the_axis_stay_in_the_simulated_stock() {
    // Three cylinders of 4 mm, parallel to the axis at 7 mm from it, in the
    // head zone (X -8 to 0, held 4 mm at the start); not a surface of
    // revolution, so the check is by the simulation.
    let w = Workholding {
        start: Some(HeldRegion {
            length: Millimetres(4.0),
            diameter: None,
        }),
        end: None,
        tabs: vec![Tab {
            x_start: Millimetres(-8.0),
            length: Millimetres(8.0),
            shape: TabShape::Cylinder {
                diameter: Millimetres(4.0),
            },
            layout: TabLayout::AroundAxis {
                count: 3,
                start_angle: openchuck_app::model::Degrees(30.0),
                radial_offset: Millimetres(7.0),
            },
        }],
    };
    let b = build("around", w, 2, BALL);
    let run = b.app.simulate().unwrap();
    let report = run.simulation.unwrap();
    let setup = &report.setups[0];
    for op in &setup.operations {
        assert!(
            op.rapid_cuts.is_empty(),
            "{}: {:?}",
            op.operation,
            op.rapid_cuts
        );
    }
    let d = setup.deviation.as_ref().unwrap();
    assert_eq!(d.overcut_count, 0, "{:?}", d.overcut_places.first());
    // The axis of each tab is material from end to end.
    let mut checked = 0;
    for k in 0..3 {
        let angle = (30.0 + 120.0 * f64::from(k)).to_radians();
        for step in 1..16 {
            let x = -8.0 + 0.5 * f64::from(step);
            for inset in [0.0, 0.8] {
                let r = 7.0 + inset;
                let p = openchuck_types::geometry::Point3::new(x, r * angle.cos(), r * angle.sin());
                assert!(setup.stock.contains_point(p), "material gone at {p:?}");
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 90);
    // The tabs show on the stock's surface in the deviation report.
    assert!(
        d.points
            .iter()
            .any(|p| p.point.x > -7.0 && p.point.x < -1.0 && p.point.y.hypot(p.point.z) > 8.0),
        "no sample on the outer side of a tab"
    );
    std::fs::remove_dir_all(&b.dir).unwrap();
}
