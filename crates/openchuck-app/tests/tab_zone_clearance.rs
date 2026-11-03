// SPDX-License-Identifier: AGPL-3.0-or-later
//! Retaining tabs beside a held region on the public blisk (issue #511;
//! OP-21, SET-09; decision 0502): roughing and rest machining never cut into
//! the tab, the simulated stock keeps it, the stock round it is cleared down
//! to it where the tool has room, and where the tool has no room the
//! operation says so instead of leaving the stock round the tab silently.
//!
//! The setup is the one of the report in numbers: the blisk centred on the
//! rotary axis and moved 4 mm along it, Ø42 stock from X -8 to 56 held for
//! 8 mm at the start, an automatic Ø10 tab in the head zone (X 0 to 4), and a
//! 3.175 mm flat roughing tool.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_cmp
)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use openchuck_app::model::TransformStep;
use openchuck_app::project::id::{OperationId, SetupId, ToolId};
use openchuck_app::project::{
    Item, Operation, OrderKey, ParameterValue, Placement, Setup, ToolSnapshot,
};
use openchuck_app::setup::{
    HeldRegion, Millimetres, Point3, Stock, Tab, TabEnd, TabLayout, TabShape, Workholding, builtin,
};
use openchuck_app::simulation::SimulationSettings;
use openchuck_app::{Application, Command, Regeneration, Threads};
use openchuck_types::tool::{ControllerMapping, CuttingData, Tool, ToolShape};
use openchuck_types::units::{FeedRate, SpindleSpeed};

/// The tools' diameter, in millimetres.
const TOOL: f64 = 3.175;
/// The tab: Ø10 from X 0 to 4, filling the head zone.
const TAB: (f64, f64, f64) = (0.0, 4.0, 5.0);
/// The simulation cell, coarse to keep the debug-build run short.
const CELL: f64 = 0.25;

fn tool(shape: ToolShape) -> Tool {
    Tool {
        shape,
        diameter: Millimetres(TOOL),
        flute_length: Millimetres(12.0),
        overall_length: Millimetres(38.0),
        shank_diameter: Millimetres(TOOL),
        flute_count: 2,
        material: None,
        centre_cutting: None,
    }
}

fn snapshot(id: ToolId, number: u32, name: &str, shape: ToolShape) -> ToolSnapshot {
    let mut cutting_data = BTreeMap::new();
    cutting_data.insert(
        "Plastic".to_owned(),
        CuttingData {
            spindle_speed: SpindleSpeed(10000.0),
            feed_per_tooth: Millimetres(0.05),
            plunge_feed: FeedRate(300.0),
            stepover: Millimetres(2.0),
            stepdown: Millimetres(3.0),
        },
    );
    ToolSnapshot {
        id,
        order: OrderKey::default(),
        name: name.into(),
        tool: tool(shape),
        controller: ControllerMapping {
            number,
            length_offset: None,
            diameter_offset: None,
        },
        cutting_data,
    }
}

fn operation(
    setup: SetupId,
    tool: ToolId,
    name: &str,
    strategy: &str,
    parameters: Vec<(&str, ParameterValue)>,
) -> Item {
    Item::Operation(Operation {
        enabled: true,
        id: OperationId::new(),
        order: OrderKey::default(),
        name: name.into(),
        setup,
        strategy: strategy.into(),
        tool: Some(tool),
        selections: Vec::new(),
        boundary: None,
        avoid: Vec::new(),
        check: Vec::new(),
        restriction_options: Default::default(),
        spindle_speed: None,
        material: None,
        parameters: parameters
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v))
            .collect(),
    })
}

/// The blisk set up as in the report, roughed along its length with `leave`
/// stock to leave, then (with `rest`) rest machined with a ball of the same
/// size.
fn build(name: &str, leave: f64, rest: bool) -> (PathBuf, Application) {
    let dir = std::env::temp_dir().join(format!("openchuck-tabzone-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut app = Application::create(&dir, name, Threads::Exactly(2)).unwrap();
    std::fs::create_dir_all(dir.join("models")).unwrap();
    let blisk = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/stl/blisk.stl");
    std::fs::copy(blisk, dir.join("models/blisk.stl")).unwrap();
    let (model, _) = app.add_model(&dir.join("models/blisk.stl"), None).unwrap();
    app.execute(Command::SetModelTransform {
        model,
        transform: vec![TransformStep::CentreOnRotaryAxis],
    })
    .unwrap();
    app.execute(Command::SetMachine(Some(Box::new(
        builtin("makera-z1").unwrap().unwrap(),
    ))))
    .unwrap();
    app.execute(Command::SetSimulation(SimulationSettings {
        cell: Millimetres(CELL),
    }))
    .unwrap();
    let setup_id = SetupId::new();
    let mut setup = Setup::new(setup_id, "Setup 1");
    setup.part = Some(model);
    setup.material = Some("Plastic".into());
    setup.frame.part_offset = Millimetres(4.0);
    setup.stock = Some(Stock::Cylinder {
        diameter: Millimetres(42.0),
        length: Millimetres(64.0),
        x_start: Millimetres(-8.0),
    });
    setup.workholding = Workholding {
        start: Some(HeldRegion {
            length: Millimetres(8.0),
            diameter: None,
        }),
        end: None,
        tabs: vec![Tab {
            auto: Some(TabEnd::Head),
            x_start: Millimetres(TAB.0),
            length: Millimetres(TAB.1 - TAB.0),
            shape: TabShape::Cylinder {
                diameter: Millimetres(2.0 * TAB.2),
            },
            layout: TabLayout::OnAxis,
        }],
    };
    let (flat, ball) = (ToolId::new(), ToolId::new());
    let mut items = vec![
        Item::Setup(setup),
        Item::Tool(snapshot(flat, 1, "Flat 3.175", ToolShape::FlatEnd)),
        Item::Tool(snapshot(ball, 2, "Ball 3.175", ToolShape::BallEnd)),
        operation(
            setup_id,
            flat,
            "Roughing",
            "rotary_roughing",
            vec![
                (
                    "direction",
                    ParameterValue::Text("along the length of the part".into()),
                ),
                ("stock_to_leave", ParameterValue::Number(leave)),
                ("adaptive_spacing", ParameterValue::Flag(false)),
            ],
        ),
    ];
    if rest {
        items.push(operation(
            setup_id,
            ball,
            "Rest",
            "rest_machining",
            // The free end at X 54 is not what this checks, and the stock
            // beside it is deeper than the flutes.
            vec![
                ("stepdown", ParameterValue::Number(1.0)),
                ("face_free_ends", ParameterValue::Flag(false)),
            ],
        ));
    }
    for item in items {
        app.execute(Command::Add {
            item,
            placement: Placement::End,
        })
        .unwrap();
    }
    (dir, app)
}

/// The tip of every fed move of `group`, and its distance from the rotary
/// axis. The tools stand across the axis, so the tip is the nearest point of
/// a flat or ball end to it.
fn fed_tips(run: &Regeneration, group: usize) -> Vec<(Point3, f64)> {
    run.toolpath.groups[group]
        .segments
        .iter()
        .filter(|s| s.kind.is_fed())
        .map(|s| {
            let p = s.end.tip;
            (p, p.y.hypot(p.z))
        })
        .collect()
}

/// True when a tool whose tip is at `p` overlaps the tab along X.
fn over_tab(p: Point3) -> bool {
    p.x + TOOL / 2.0 > TAB.0 && p.x - TOOL / 2.0 < TAB.1
}

/// No fed move of `group` puts the tool's tip inside the tab's cylinder.
fn assert_tab_not_cut(run: &Regeneration, group: usize, what: &str) {
    let tips = fed_tips(run, group);
    assert!(tips.len() > 100, "{what}: {} fed moves", tips.len());
    let worst = tips
        .iter()
        .filter(|(p, _)| over_tab(*p))
        .map(|&(_, r)| r)
        .fold(f64::INFINITY, f64::min);
    assert!(
        worst >= TAB.2 - 1e-3,
        "{what}: a fed move reaches {worst:.4} mm from the axis over the tab (radius {})",
        TAB.2
    );
}

/// Issue #511: 4 mm between the held region and the part, less 0.5 mm kept
/// from the held region and 0.5 mm stock to leave, is 3 mm: the 3.175 mm tool
/// cannot cut down beside the tab. The roughing says so, still makes its
/// path, and does not cut the tab.
#[test]
fn req_op_21_req_set_09_roughing_says_when_it_cannot_cut_down_beside_a_tab() {
    let (dir, app) = build("no-room", 0.5, false);
    let run = app.regenerate().unwrap();
    let note = run
        .notes
        .iter()
        .find(|n| n.contains("cannot cut down beside tab 1"))
        .unwrap_or_else(|| panic!("no note in {:#?}", run.notes));
    for want in [
        "1 Roughing",
        "X 0.000 to 4.000 mm",
        "Flat 3.175 (3.175 mm) has 3.000 mm of room",
    ] {
        assert!(note.contains(want), "{want:?} not in {note}");
    }
    assert_tab_not_cut(&run, 0, "roughing");
    std::fs::remove_dir_all(&dir).unwrap();
}

/// With no stock to leave there is 3.5 mm of room: no note, the roughing
/// cuts down to the tab beside it (to within the cell) and never into it,
/// rest machining keeps it too, and the simulated stock keeps the tab whole
/// while the stock round it is gone.
#[test]
fn req_op_21_req_set_09_roughing_and_rest_clear_round_a_tab_and_keep_it() {
    let (dir, app) = build("room", 0.0, true);
    let run = app.simulate().unwrap();
    assert!(
        !run.notes.iter().any(|n| n.contains("beside tab")),
        "{:#?}",
        run.notes
    );
    assert_tab_not_cut(&run, 0, "roughing");
    assert_tab_not_cut(&run, 1, "rest machining");
    // Cleared down to the tab: some roughing move over the tab's middle is
    // on its surface.
    let lowest = fed_tips(&run, 0)
        .into_iter()
        .filter(|(p, _)| (TAB.0 + 1.0..=TAB.1 - 1.0).contains(&p.x))
        .map(|(_, r)| r)
        .fold(f64::INFINITY, f64::min);
    assert!(
        lowest <= TAB.2 + 0.01,
        "roughing reaches only {lowest:.4} mm from the axis over the tab"
    );
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
    let mut kept = 0;
    let mut cleared = 0;
    for k in 0..36 {
        let angle = std::f64::consts::TAU * f64::from(k) / 36.0;
        let at = |x: f64, r: f64| Point3::new(x, r * angle.cos(), r * angle.sin());
        for x in [1.0, 2.0, 3.0] {
            // Inside the tab, three cells in from its surface: still material.
            let inside = at(x, TAB.2 - 3.0 * CELL);
            assert!(setup.stock.contains_point(inside), "tab cut at {inside:?}");
            kept += 1;
        }
        // Round the tab, under the tool's flat end at X 2 (its middle stays
        // at X 2.0875 or more): cut away.
        let round = at(2.0, TAB.2 + 3.0);
        assert!(
            !setup.stock.contains_point(round),
            "stock left at {round:?}"
        );
        cleared += 1;
    }
    assert_eq!((kept, cleared), (108, 36));
    std::fs::remove_dir_all(&dir).unwrap();
}
