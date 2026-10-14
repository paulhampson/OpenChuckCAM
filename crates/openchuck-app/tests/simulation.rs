// SPDX-License-Identifier: AGPL-3.0-or-later
//! Simulation through the application (work breakdown slice 7; SIM-03,
//! SIM-05, SIM-06, SIM-07, GC-07; decision record 0025).

// Test code may unwrap, index and panic (NFR-07 applies to library code).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_cmp
)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use openchuck_app::model::TransformStep;
use openchuck_app::project::id::{OperationId, SetupId, ToolId};
use openchuck_app::project::{Item, Operation, OrderKey, Placement, Setup, ToolSnapshot};
use openchuck_app::setup::{MachineProfile, Millimetres, Stock};
use openchuck_app::simulation::SimulationSettings;
use openchuck_app::{Application, Command, Threads};
use openchuck_path::{CutPoint, ParameterSchema, Parameters, Pass, Strategy, StrategyInput};
use openchuck_types::ir::MoveKind;
use openchuck_types::job::JobContext;
use openchuck_types::tool::{ControllerMapping, CuttingData, Tool, ToolShape};
use openchuck_types::units::{Degrees, FeedRate, SpindleSpeed};

/// One ring of cuts at model X = 5, in 1 degree steps (a test strategy).
#[derive(Debug)]
struct FineRing;

impl Strategy for FineRing {
    fn name(&self) -> &str {
        "fine_ring"
    }
    fn label(&self) -> &str {
        "Fine ring"
    }
    fn schema(&self) -> ParameterSchema {
        ParameterSchema::with_common(Vec::new())
    }
    fn passes(
        &self,
        input: &StrategyInput<'_>,
        _: &Parameters,
        _: &JobContext<'_>,
    ) -> openchuck_path::Result<Vec<Pass>> {
        let points = (0..=360)
            .filter_map(|i| {
                let a = Degrees(f64::from(i));
                let line = input
                    .rotary
                    .tool_line(Millimetres(5.0), a, Millimetres(0.0));
                let c = input.surface.drop_tool(input.tool, &line)?;
                Some(CutPoint {
                    tip: c.tip,
                    a,
                    contact: c.contact,
                })
            })
            .collect();
        Ok(vec![Pass { points }])
    }
}

/// The corpus cube (10 mm, X from 0 to 10) on the rotary axis in Ø20 stock
/// from X -2 to 12, a 4 mm ball,
/// and `ops` operations of the ring strategy in one setup.
fn project(name: &str, ops: usize, threads: usize) -> (PathBuf, Application) {
    let dir = std::env::temp_dir().join(format!("openchuck-sim-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut app = Application::create(&dir, name, Threads::Exactly(threads)).unwrap();
    app.register_strategy(Arc::new(FineRing)).unwrap();
    std::fs::create_dir_all(dir.join("models")).unwrap();
    let cube = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/stl/cube_ascii.stl");
    std::fs::copy(cube, dir.join("models/cube.stl")).unwrap();
    let (model, _) = app.add_model(&dir.join("models/cube.stl"), None).unwrap();
    app.execute(Command::SetModelTransform {
        model,
        transform: vec![TransformStep::CentreOnRotaryAxis],
    })
    .unwrap();
    app.execute(Command::SetMachine(Some(Box::new(
        MachineProfile::example_x_rotary(),
    ))))
    .unwrap();
    let setup_id = SetupId::new();
    let mut setup = Setup::new(setup_id, "Setup 1");
    setup.part = Some(model);
    setup.material = Some("oak".into());
    setup.stock = Some(Stock::Cylinder {
        diameter: Millimetres(20.0),
        length: Millimetres(14.0),
        x_start: Millimetres(-2.0),
    });
    let tool_id = ToolId::new();
    let mut cutting_data = BTreeMap::new();
    cutting_data.insert(
        "oak".to_owned(),
        CuttingData {
            spindle_speed: SpindleSpeed(10000.0),
            feed_per_tooth: Millimetres(0.02),
            plunge_feed: FeedRate(150.0),
            stepover: Millimetres(0.8),
            stepdown: Millimetres(1.0),
        },
    );
    let mut items = vec![
        Item::Setup(setup),
        Item::Tool(ToolSnapshot {
            id: tool_id,
            order: OrderKey::default(),
            name: "4 mm ball".into(),
            tool: Tool {
                shape: ToolShape::BallEnd,
                diameter: Millimetres(4.0),
                flute_length: Millimetres(10.0),
                overall_length: Millimetres(40.0),
                shank_diameter: Millimetres(4.0),
                flute_count: 2,
                material: None,
            },
            controller: ControllerMapping {
                number: 1,
                length_offset: None,
                diameter_offset: None,
            },
            cutting_data,
        }),
    ];
    for i in 0..ops {
        items.push(Item::Operation(Operation {
            id: OperationId::new(),
            order: OrderKey::default(),
            name: format!("Ring {}", i + 1),
            setup: setup_id,
            strategy: "fine_ring".into(),
            tool: Some(tool_id),
            selections: Vec::new(),
            boundary: None,
            avoid: Vec::new(),
            check: Vec::new(),
            parameters: BTreeMap::new(),
        }));
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

/// Total length of the fed descent before the first cut of a group.
fn plunge_length(group: &openchuck_types::ir::ToolpathGroup) -> f64 {
    let mut at = group.start.tip;
    let mut length = 0.0;
    for s in &group.segments {
        if s.kind == MoveKind::Cutting {
            break;
        }
        if s.kind.is_fed() {
            length += at.distance(s.end.tip);
        }
        at = s.end.tip;
    }
    length
}

#[test]
fn req_sim_06_the_second_operation_is_generated_against_the_stock_the_first_left() {
    let (dir, app) = project("carry", 2, 2);
    let result = app.regenerate().unwrap();
    let (first, second) = (&result.toolpath.groups[0], &result.toolpath.groups[1]);
    // The same strategy and parameters: the first plunges from the approach
    // distance above the uncut stock, the second only from above the ring
    // the first cut, so it plunges much less.
    let (p1, p2) = (plunge_length(first), plunge_length(second));
    println!("plunge: first {p1:.3} mm, second {p2:.3} mm");
    assert!(p2 < p1 - 1.0, "{p1} vs {p2}");
    // Alone, an operation is generated against the stock definition.
    let (dir1, single) = project("carry-single", 1, 2);
    let alone = single.regenerate().unwrap();
    assert_eq!(alone.toolpath.groups[0].segments, first.segments);
    // Regeneration without a simulation request reports none.
    assert!(result.simulation.is_none());
    std::fs::remove_dir_all(&dir).unwrap();
    std::fs::remove_dir_all(&dir1).unwrap();
}

#[test]
fn req_sim_03_req_sim_05_a_project_simulates_with_a_report_against_its_part() {
    let (dir, app) = project("report", 2, 2);
    let result = app.simulate().unwrap();
    let report = result.simulation.unwrap();
    assert_eq!(report.settings, SimulationSettings::default());
    assert_eq!(report.setups.len(), 1);
    let setup = &report.setups[0];
    assert_eq!(setup.operations.len(), 2);
    let (r1, r2) = (
        setup.operations[0].removed_volume,
        setup.operations[1].removed_volume,
    );
    println!("removed: {r1:.2} then {r2:.4} mm3");
    assert!(r1 > 10.0);
    // The second ring repeats the first: it finds almost nothing left.
    assert!(r2 < 0.01 * r1, "{r2}");
    assert!((setup.initial_volume - setup.final_volume - r1 - r2).abs() < 1e-6);
    assert!(setup.operations.iter().all(|o| o.rapid_cuts.is_empty()));
    let d = setup.deviation.as_ref().unwrap();
    println!(
        "deviation: worst remaining {:?}, worst overcut {:?}, {} beyond {}",
        d.worst_remaining.map(|p| p.value),
        d.worst_overcut.map(|p| p.value),
        d.overcut_count,
        d.overcut_limit
    );
    assert!(!d.points.is_empty());
    assert_eq!(d.overcut_count, 0);
    // The ring at X = 5 leaves the cube's faces under it within a cell
    // plus the 1 degree scallop.
    let near: Vec<f64> = d
        .points
        .iter()
        .filter(|p| (p.point.x - 5.0).abs() < 0.2 && p.normal.x.abs() < 1e-9)
        .map(|p| p.value)
        .collect();
    assert!(!near.is_empty());
    assert!(near.iter().all(|v| *v <= 0.1 + 0.01), "{near:?}");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn req_gc_07_req_nfr_06_carried_stock_gives_the_same_toolpath_at_any_thread_count() {
    let (dir1, one) = project("threads-1", 2, 1);
    let (dir4, four) = project("threads-4", 2, 4);
    // The projects' IDs differ; the moves must not.
    let moves = |app: &Application| {
        app.regenerate()
            .unwrap()
            .toolpath
            .groups
            .into_iter()
            .map(|g| (g.start, g.segments))
            .collect::<Vec<_>>()
    };
    assert_eq!(moves(&one), moves(&four));
    std::fs::remove_dir_all(&dir1).unwrap();
    std::fs::remove_dir_all(&dir4).unwrap();
}

#[test]
fn req_sim_07_the_cell_is_an_undoable_project_setting() {
    let (dir, mut app) = project("setting", 0, 2);
    assert_eq!(app.project().simulation(), SimulationSettings::default());
    app.save().unwrap();
    let text = std::fs::read_to_string(dir.join("project.toml")).unwrap();
    assert!(
        !text.contains("[simulation]"),
        "the default is not written: {text}"
    );
    let fine = SimulationSettings {
        cell: Millimetres(0.05),
    };
    app.execute(Command::SetSimulation(fine)).unwrap();
    assert_eq!(app.project().simulation(), fine);
    assert!(app.undo().unwrap());
    assert_eq!(app.project().simulation(), SimulationSettings::default());
    assert!(app.redo().unwrap());
    app.save().unwrap();
    let reopened = Application::open(&dir, Threads::Exactly(1)).unwrap();
    assert_eq!(reopened.project().simulation(), fine);
    // Out of range is refused and changes nothing.
    for bad in [0.01, 5.0] {
        let e = app
            .execute(Command::SetSimulation(SimulationSettings {
                cell: Millimetres(bad),
            }))
            .unwrap_err()
            .to_string();
        assert!(e.contains("0.025"), "{e}");
        assert_eq!(app.project().simulation(), fine);
    }
    std::fs::remove_dir_all(&dir).unwrap();
}
