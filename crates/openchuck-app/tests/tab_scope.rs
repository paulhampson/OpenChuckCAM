// SPDX-License-Identifier: AGPL-3.0-or-later
//! A tab shorter than the tool is warned of, and only for the operations that
//! can touch it (OP-21, NFR-07, NFR-12; issue #548; decision 0523, amending
//! 0494 items 1 and 2).
//!
//! The owner's case on a public part: a cone (radius 10 to 5, X 0 to 30) in
//! round stock, gripped at its start, with a tab on the axis 4 mm long and
//! 6 mm across between the grip and the part, and a 6 mm ball. The tab is
//! shorter than the tool. An operation on the whole part is made with a
//! warning (the owner's choice of 6 Nov 2026; a refusal before). An operation
//! limited to faces that keep the tool above the tab, the cone's side in the
//! owner's case, is made with no warning: its path never comes down to the
//! tab, which is within 3 mm of the axis.
//!
//! The owner's carving case of #555 (decision 0527), on the same part: a
//! V-bit of 20 degrees on the cone's side keeps its contact on the rim of the
//! cone's end while its tip sweeps down beside the end face, and came down
//! onto the tab. A selection now stops at its edge with an unselected face,
//! and an operation limited by one keeps its clearance from every tab.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use openchuck_app::project::id::{OperationId, SetupId, ToolId};
use openchuck_app::project::{
    Item, Operation, OrderKey, ParameterValue, Placement, Setup, ToolSnapshot,
};
use openchuck_app::selection::SelectionRole;
use openchuck_app::setup::{
    HeldRegion, Millimetres, Stock, Tab, TabLayout, TabShape, Workholding, builtin,
};
use openchuck_app::simulation::SimulationSettings;
use openchuck_app::{Application, Command, Threads};
use openchuck_testkit::parts::{Cone, RadialPart};
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::tool::{ControllerMapping, CuttingData, Tool, ToolShape};
use openchuck_types::units::{FeedRate, SpindleSpeed};

/// The tool's diameter: wider than the tab is long.
const BALL: f64 = 6.0;

fn cone() -> TriangleMesh {
    Cone {
        start_radius: 10.0,
        end_radius: 5.0,
        length: 30.0,
    }
    .mesh(31, 48)
}

fn write_stl(mesh: &TriangleMesh, path: &Path) {
    let mut text = String::from("solid part\n");
    for t in &mesh.triangles {
        text.push_str("facet normal 0 0 0\n outer loop\n");
        for i in t {
            let p = mesh.vertices[*i as usize];
            text.push_str(&format!("  vertex {:e} {:e} {:e}\n", p.x, p.y, p.z));
        }
        text.push_str(" endloop\nendfacet\n");
    }
    text.push_str("endsolid part\n");
    std::fs::write(path, text).unwrap();
}

struct Built {
    dir: PathBuf,
    app: Application,
    operation: OperationId,
    model: openchuck_app::project::id::ModelId,
}

impl Drop for Built {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// The cone in round stock 24 mm across from X -12 to 42, gripped 4 mm at
/// its start, with a tab on the axis from X -4 to 0, 6 mm across, and axial
/// finishing with a 6 mm ball.
fn build(name: &str) -> Built {
    build_with(name, ToolShape::BallEnd)
}

/// As [`build`], with a tool of `shape`, 6 mm across.
fn build_with(name: &str, shape: ToolShape) -> Built {
    let dir =
        std::env::temp_dir().join(format!("openchuck-tabscope-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut app = Application::create(&dir, name, Threads::Exactly(2)).unwrap();
    std::fs::create_dir_all(dir.join("models")).unwrap();
    write_stl(&cone(), &dir.join("models/part.stl"));
    let (model, _) = app.add_model(&dir.join("models/part.stl"), None).unwrap();
    let machine = builtin("makera-z1").unwrap().unwrap();
    app.execute(Command::SetMachine(Some(Box::new(machine))))
        .unwrap();
    app.execute(Command::SetSimulation(SimulationSettings {
        cell: Millimetres(0.5),
    }))
    .unwrap();
    let setup_id = SetupId::new();
    let mut setup = Setup::new(setup_id, "Setup 1");
    setup.part = Some(model);
    setup.material = Some("oak".into());
    setup.stock = Some(Stock::Cylinder {
        diameter: Millimetres(24.0),
        length: Millimetres(54.0),
        x_start: Millimetres(-12.0),
    });
    setup.workholding = Workholding {
        start: Some(HeldRegion {
            length: Millimetres(4.0),
            diameter: None,
        }),
        end: None,
        tabs: vec![Tab {
            x_start: Millimetres(-4.0),
            length: Millimetres(4.0),
            shape: TabShape::Cylinder {
                diameter: Millimetres(6.0),
            },
            layout: TabLayout::OnAxis,
            auto: None,
        }],
    };
    let tool_id = ToolId::new();
    let cutting_data = BTreeMap::from([(
        "oak".to_owned(),
        CuttingData {
            spindle_speed: SpindleSpeed(12000.0),
            feed_per_tooth: Millimetres(0.02),
            plunge_feed: FeedRate(150.0),
            stepover: Millimetres(2.0),
            stepdown: Millimetres(2.0),
        },
    )]);
    let snapshot = ToolSnapshot {
        id: tool_id,
        order: OrderKey::default(),
        name: "6 mm ball".into(),
        tool: Tool {
            shape,
            diameter: Millimetres(BALL),
            flute_length: Millimetres(30.0),
            overall_length: Millimetres(45.0),
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
    let operation = OperationId::new();
    let items = vec![
        Item::Setup(setup),
        Item::Tool(snapshot),
        Item::Operation(Operation {
            enabled: true,
            id: operation,
            order: OrderKey::default(),
            name: "Axial Finishing".into(),
            setup: setup_id,
            strategy: "axial_finishing".into(),
            tool: Some(tool_id),
            selections: Vec::new(),
            boundary: None,
            avoid: Vec::new(),
            check: Vec::new(),
            restriction_options: Default::default(),
            spindle_speed: None,
            material: None,
            parameters: [
                ("stepover", ParameterValue::Number(2.0)),
                ("face_free_ends", ParameterValue::Flag(false)),
                ("adaptive_spacing", ParameterValue::Flag(false)),
            ]
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v))
            .collect(),
        }),
    ];
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
        operation,
        model,
    }
}

/// The cone's triangles whose corners all lie in `x` (low, high) and off
/// the axis: its side there, not its end discs.
fn side_between(x: (f64, f64)) -> BTreeSet<u32> {
    let mesh = cone();
    mesh.triangles
        .iter()
        .enumerate()
        .filter(|(_, t)| {
            let p = t.map(|i| mesh.vertices[i as usize]);
            let xs = p.map(|v| v.x);
            let flat = xs.iter().all(|x| (x - xs[0]).abs() < 1e-9);
            !flat && xs.iter().all(|&v| v >= x.0 - 1e-9 && v <= x.1 + 1e-9)
        })
        .map(|(i, _)| u32::try_from(i).unwrap())
        .collect()
}

/// `b`'s operation limited to the cone's side between `x`.
fn limit_to(b: &mut Built, x: (f64, f64)) {
    let set = b
        .app
        .save_selection("Side", b.model, side_between(x))
        .unwrap();
    b.app
        .set_operation_selection(b.operation, SelectionRole::Machine, set, true)
        .unwrap();
}

/// The tab-length warnings among the run's paths (decision 0523).
fn tab_warnings(run: &openchuck_app::Regeneration) -> Vec<String> {
    run.groups
        .iter()
        .flat_map(|g| g.warnings.iter())
        .filter(|w| {
            w.message
                .starts_with(openchuck_app::workholding::TAB_LENGTH_LEAD)
        })
        .map(|w| w.message.clone())
        .collect()
}

#[test]
fn req_op_21_req_nfr_12_a_whole_part_operation_with_a_tab_shorter_than_the_tool_is_warned_of() {
    // With no region, every tab counts (decision 0494). By the owner's choice
    // of 6 Nov 2026 (decision 0523) the path is made and the tab is warned
    // of, with its number, X range and length and the tool's width; it was
    // refused before.
    let b = build("whole");
    let run = b
        .app
        .simulate()
        .unwrap_or_else(|e| panic!("refused, not warned: {e}"));
    let warnings = tab_warnings(&run);
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    let w = &warnings[0];
    assert!(
        w.contains("tab 1 (X -4.000 to 0.000 mm) is 4.000 mm long along X")
            && w.contains("shorter than the tool is wide (6.000 mm)")
            && w.contains("may be unable to cut the gap beside it to shape")
            && w.contains("check it before running it")
            && w.contains("Use a smaller tool, or make the tab longer"),
        "{w}"
    );
    assert!(!w.to_lowercase().contains("safe"), "{w}");
    // Messages name the operation and the tool, as they are now.
    let named: Vec<String> = b
        .app
        .overview(None, &openchuck_app::JobLog::default())
        .messages
        .into_iter()
        .map(|m| m.text)
        .filter(|t| t.starts_with("Tab length"))
        .collect();
    assert_eq!(named.len(), 1, "{named:?}");
    assert!(
        named[0]
            .starts_with("Tab length (operation 'Axial Finishing', tool '6 mm ball'): tab 1 (X"),
        "{named:?}"
    );
}

#[test]
fn req_op_21_req_nfr_12_an_operation_on_the_side_above_a_short_tab_is_made() {
    // The owner's case (#548): the operation's selection is the cone's side,
    // all of it, which keeps the ball at least 5 mm from the axis, above the
    // tab's 3 mm. Before, the tab refused it.
    let mut b = build("side");
    limit_to(&mut b, (0.0, 30.0));
    let run = b.app.simulate().unwrap_or_else(|e| panic!("{e}"));
    assert!(!run.toolpath.groups.is_empty());
    assert!(tab_warnings(&run).is_empty(), "{:?}", tab_warnings(&run));
}

#[test]
fn req_op_21_an_operation_far_from_a_short_tab_is_made() {
    // A band of the side from X 10 to 25: nowhere near the tab.
    let mut b = build("band");
    limit_to(&mut b, (10.0, 25.0));
    let run = b.app.simulate().unwrap_or_else(|e| panic!("{e}"));
    assert!(!run.toolpath.groups.is_empty());
    assert!(tab_warnings(&run).is_empty(), "{:?}", tab_warnings(&run));
}

/// The V-bit of the owner's carving project (#555): 20 degrees, a 0.3 mm tip.
const V_ANGLE: f64 = 20.0;
const V_TIP: f64 = 0.3;

/// The most the V-bit of `run`'s paths comes into the tab (X -4 to 0,
/// radius 3) grown by `margin`, over every move, fed or rapid, sampled along
/// it as the machine makes it; zero when it keeps clear. The tool's axis
/// crosses the rotary axis (axial finishing), so the tab is nearest in the
/// plane through both, where the V reaches `tip + h tan(half angle)` from its
/// axis at `h` above its tip.
fn into_tab(run: &openchuck_app::Regeneration, margin: f64) -> f64 {
    let (lo, hi, r) = (-4.0 - margin, margin, 3.0 + margin);
    let tan = (V_ANGLE / 2.0).to_radians().tan();
    let tip = V_TIP / 2.0;
    let into = |x: f64, h: f64| {
        if h >= r {
            return 0.0;
        }
        let dx = if (lo..=hi).contains(&x) {
            0.0
        } else {
            (x - lo).abs().min((x - hi).abs())
        };
        if dx <= tip {
            r - h
        } else {
            (r - (h + (dx - tip) / tan)).max(0.0)
        }
    };
    let height = |p: &openchuck_types::ir::ToolPose| {
        let axis = p.axis.get();
        p.tip.y * axis.y + p.tip.z * axis.z
    };
    let mut worst = 0.0_f64;
    for group in &run.toolpath.groups {
        let mut from = group.start;
        for segment in &group.segments {
            let to = segment.end;
            for k in 0..=16 {
                let t = f64::from(k) / 16.0;
                let x = from.tip.x + t * (to.tip.x - from.tip.x);
                let h = height(&from) + t * (height(&to) - height(&from));
                worst = worst.max(into(x, h));
            }
            from = to;
        }
    }
    worst
}

#[test]
fn req_op_02_req_op_21_req_nfr_12_a_v_bit_on_the_side_keeps_to_it_and_clear_of_the_tab() {
    // The owner's case (#555): axial finishing with the V-bit on the cone's
    // side. Before, every pass came down beside the cone's end onto the tab
    // (its tip at radius 3.000). Now its cuts stay on the side (none past
    // the end at X 0) and no move comes within the clearance of the tab.
    let mut b = build_with(
        "vbit",
        ToolShape::VBit {
            included_angle: openchuck_types::units::Degrees(V_ANGLE),
            tip_diameter: Millimetres(V_TIP),
        },
    );
    limit_to(&mut b, (0.0, 30.0));
    let run = b.app.regenerate().unwrap_or_else(|e| panic!("{e}"));
    assert!(!run.toolpath.groups.is_empty());
    let lowest = run
        .toolpath
        .groups
        .iter()
        .flat_map(|g| g.segments.iter())
        .filter(|s| s.kind.is_fed())
        .map(|s| s.end.tip.x)
        .fold(f64::INFINITY, f64::min);
    assert!(lowest >= -1e-3, "a cut at X {lowest}, past the selection");
    let tolerance = 0.01;
    let into = into_tab(&run, openchuck_path::HELD_CLEARANCE - tolerance);
    assert!(
        into <= 0.0,
        "the tool comes {into} mm into the tab's clearance"
    );
}
