// SPDX-License-Identifier: AGPL-3.0-or-later
//! Material left that no tool can reach, through the application (SIM-05 as
//! amended on 1 Nov 2026; issue 478): a part with a bore along the rotary
//! axis leaves it full of stock, and the run says so once per setup, in
//! plain words, with where it is.

// Test code may unwrap, index and panic (NFR-07 applies to library code).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_cmp
)]

use std::collections::BTreeMap;
use std::f64::consts::TAU;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::Arc;

use openchuck_app::project::id::{OperationId, SetupId, ToolId};
use openchuck_app::project::{Item, Operation, OrderKey, Placement, Setup, ToolSnapshot};
use openchuck_app::setup::{MachineProfile, Millimetres, Stock};
use openchuck_app::sim::ReachClass;
use openchuck_app::{Application, Command, Threads};
use openchuck_path::{CutPoint, ParameterSchema, Parameters, Pass, Strategy, StrategyInput};
use openchuck_types::job::JobContext;
use openchuck_types::tool::{ControllerMapping, CuttingData, Tool, ToolShape};
use openchuck_types::units::{Degrees, FeedRate, SpindleSpeed};

/// What the warning about enclosed material says, in every case.
const ENCLOSED: &str = "is enclosed by the part across the rotary axis";

/// One ring of cuts at model X = 5 (a test strategy): it leaves material
/// everywhere else, outside and in the bore.
#[derive(Debug)]
struct Ring;

impl Strategy for Ring {
    fn name(&self) -> &str {
        "ring"
    }
    fn label(&self) -> &str {
        "Ring"
    }
    fn description(&self) -> &str {
        "A strategy for tests."
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
            .step_by(2)
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
        Ok(vec![Pass {
            helical_pass_start: false,
            points,
            stock: None,
            rapids: Vec::new(),
        }])
    }
}

/// An ASCII STL of a closed tube along X from 0 to `length`, outer radius
/// `outer`, bore radius `inner`, 64 facets round (the public stand-in for
/// the owner's carving, a tube whose bore lies on the rotary axis).
fn tube_stl(outer: f64, inner: f64, length: f64) -> String {
    let n = 64;
    let at = |x: f64, r: f64, i: usize| {
        let t = TAU * (i % n) as f64 / n as f64;
        [x, r * t.cos(), r * t.sin()]
    };
    let mut facets: Vec<[[f64; 3]; 3]> = Vec::new();
    for i in 0..n {
        let j = i + 1;
        let (o0i, o0j, o1i, o1j) = (
            at(0.0, outer, i),
            at(0.0, outer, j),
            at(length, outer, i),
            at(length, outer, j),
        );
        let (i0i, i0j, i1i, i1j) = (
            at(0.0, inner, i),
            at(0.0, inner, j),
            at(length, inner, i),
            at(length, inner, j),
        );
        facets.extend([
            [o0i, o0j, o1i],
            [o0j, o1j, o1i],
            [i0i, i1i, i0j],
            [i0j, i1i, i1j],
            [o0i, i0i, i0j],
            [o0i, i0j, o0j],
            [o1i, o1j, i1j],
            [o1i, i1j, i1i],
        ]);
    }
    let mut out = String::from("solid tube\n");
    for f in facets {
        out.push_str(" facet normal 0 0 0\n  outer loop\n");
        for v in f {
            writeln!(out, "   vertex {} {} {}", v[0], v[1], v[2]).unwrap();
        }
        out.push_str("  endloop\n endfacet\n");
    }
    out.push_str("endsolid tube\n");
    out
}

/// A project with `part` (an STL's text) in Ø`stock` stock from X -2 to
/// `length` + 2, a 4 mm ball and one ring operation.
fn project(name: &str, part: &str, stock: f64, length: f64) -> (PathBuf, Application) {
    let dir = std::env::temp_dir().join(format!("openchuck-reach-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut app = Application::create(&dir, name, Threads::Exactly(2)).unwrap();
    app.register_strategy(Arc::new(Ring)).unwrap();
    std::fs::create_dir_all(dir.join("models")).unwrap();
    std::fs::write(dir.join("models/part.stl"), part).unwrap();
    let (model, _) = app.add_model(&dir.join("models/part.stl"), None).unwrap();
    app.execute(Command::SetMachine(Some(Box::new(
        MachineProfile::example_x_rotary(),
    ))))
    .unwrap();
    let setup_id = SetupId::new();
    let mut setup = Setup::new(setup_id, "Setup 1");
    setup.part = Some(model);
    setup.material = Some("oak".into());
    setup.stock = Some(Stock::Cylinder {
        diameter: Millimetres(stock),
        length: Millimetres(length + 4.0),
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
    let items = vec![
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
                centre_cutting: None,
            },
            controller: ControllerMapping {
                number: 1,
                length_offset: None,
                diameter_offset: None,
            },
            cutting_data,
        }),
        Item::Operation(Operation {
            enabled: true,
            id: OperationId::new(),
            order: OrderKey::default(),
            name: "Ring".into(),
            setup: setup_id,
            strategy: "ring".into(),
            tool: Some(tool_id),
            selections: Vec::new(),
            boundary: None,
            avoid: Vec::new(),
            check: Vec::new(),
            restriction_options: Default::default(),
            spindle_speed: None,
            material: None,
            parameters: BTreeMap::new(),
        }),
    ];
    for item in items {
        app.execute(Command::Add {
            item,
            placement: Placement::End,
        })
        .unwrap();
    }
    (dir, app)
}

fn enclosed_notes(notes: &[String]) -> Vec<&String> {
    notes.iter().filter(|n| n.contains(ENCLOSED)).collect()
}

#[test]
fn req_sim_05_req_nfr_12_an_enclosed_leftover_gives_one_warning_per_setup_with_its_share_and_range()
{
    let (dir, app) = project("tube", &tube_stl(8.0, 4.0, 20.0), 20.0, 20.0);
    let run = app.simulate().unwrap();
    let notes = enclosed_notes(&run.notes);
    assert_eq!(notes.len(), 1, "{:?}", run.notes);
    let note = notes[0];
    println!("{note}");
    let setup = &run.simulation.as_ref().unwrap().setups[0];
    let reach = setup
        .reach
        .as_ref()
        .expect("the setup's leftover is classified");
    let summary = reach.enclosed.as_ref().expect("the bore is enclosed");
    // The share is of the counted samples, as the histogram's are.
    let deviation = setup.deviation.as_ref().unwrap();
    let counted = deviation.points.iter().filter(|p| p.counts()).count();
    let enclosed = reach
        .classes
        .iter()
        .filter(|c| **c == ReachClass::Enclosed)
        .count();
    assert_eq!(summary.share, enclosed as f64 / counted as f64);
    // The bore is 2π·4·20 = 503 mm² of surface; the slab at each end of the
    // part is never called enclosed.
    assert!(
        (450.0..503.0).contains(&summary.area),
        "area {}",
        summary.area
    );
    assert!(summary.from <= 0.02 && summary.to >= 0.98, "{summary:?}");
    assert!(summary.facing_axis >= 0.99, "{summary:?}");
    for words in [
        "Setup 'Setup 1': ",
        &format!("{:.1}% of the part's surface", summary.share * 100.0),
        &format!("(about {:.0} mm²)", summary.area),
        "of the part's length from its start",
        "on surfaces facing the rotary axis",
        "No tool on this machine can reach it",
        "drilling or boring along the rotary axis",
        "Generated G-code must be verified",
    ] {
        assert!(note.contains(words), "{words:?} not in {note:?}");
    }
    // Only ranges and shares: no coordinates.
    assert!(!note.contains("X "), "{note}");

    // The same from the cache, without comparing again (decision 0217).
    let again = app.simulate().unwrap();
    assert!(again.compared.is_empty());
    assert_eq!(enclosed_notes(&again.notes), notes);
    assert_eq!(again.simulation, run.simulation);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn req_sim_05_material_left_within_reach_gives_no_enclosed_warning() {
    let cube = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/stl/cube_ascii.stl"),
    )
    .unwrap();
    let (dir, app) = project("cube", &cube, 20.0, 10.0);
    let run = app.simulate().unwrap();
    assert!(enclosed_notes(&run.notes).is_empty(), "{:?}", run.notes);
    let reach = run.simulation.as_ref().unwrap().setups[0]
        .reach
        .as_ref()
        .unwrap();
    assert!(reach.enclosed.is_none());
    assert!(!reach.classes.contains(&ReachClass::Enclosed));
    assert!(reach.classes.contains(&ReachClass::Open));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn req_sim_05_req_nfr_12_a_necked_tool_leaves_the_leftover_unclassified() {
    // A shank narrower than the cutter could reach past the rim of an
    // opening with its head: nothing is called enclosed.
    let (dir, mut app) = project("necked", &tube_stl(8.0, 4.0, 20.0), 20.0, 20.0);
    let mut snapshot = app
        .project()
        .tools()
        .first()
        .cloned()
        .expect("the project's tool");
    snapshot.tool.shank_diameter = Millimetres(3.0);
    app.execute(Command::Replace(Item::Tool(snapshot))).unwrap();
    let run = app.simulate().unwrap();
    assert!(enclosed_notes(&run.notes).is_empty(), "{:?}", run.notes);
    let reach = run.simulation.as_ref().unwrap().setups[0]
        .reach
        .as_ref()
        .unwrap();
    assert!(
        reach
            .classes
            .iter()
            .all(|c| matches!(c, ReachClass::NotLeft | ReachClass::Undecided))
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn req_sim_05_req_nfr_12_enclosed_material_is_a_warning_in_messages_while_the_simulation_is_current()
 {
    // Issue 478: the owner asked for a message. The warning is in the
    // Messages panel on the setup, once, for as long as the simulation it
    // was found in is current; a change that makes it stale takes it away.
    use openchuck_app::overview::{ItemRef, Severity};
    let (dir, mut app) = project("messages", &tube_stl(8.0, 4.0, 20.0), 20.0, 20.0);
    let setup = app.project().setups()[0].id;
    let warnings = |app: &Application| -> Vec<_> {
        app.overview(None, &openchuck_app::JobLog::default())
            .messages
            .into_iter()
            .filter(|m| m.text.contains(ENCLOSED))
            .collect()
    };
    assert!(warnings(&app).is_empty(), "nothing before a simulation");
    app.simulate().unwrap();
    let found = warnings(&app);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].severity, Severity::Warning);
    assert_eq!(found[0].source, Some(ItemRef::Setup(setup)));
    // Generating without a simulation keeps it while nothing changed.
    app.regenerate().unwrap();
    assert_eq!(warnings(&app).len(), 1);
    // Another stock makes the simulation stale: the warning goes.
    let mut changed = app.project().setups()[0].clone();
    changed.stock = Some(Stock::Cylinder {
        diameter: Millimetres(19.0),
        length: Millimetres(24.0),
        x_start: Millimetres(-2.0),
    });
    app.execute(Command::Replace(Item::Setup(changed))).unwrap();
    assert!(warnings(&app).is_empty(), "stale: not shown");
    std::fs::remove_dir_all(&dir).unwrap();
}
