// SPDX-License-Identifier: AGPL-3.0-or-later
//! The shell's view of the job: workflow steps (UI-10), item states (UI-11),
//! messages (UI-12) and the footer's primary action (specification section
//! 5). Verification plan rows UI-10 to UI-12; decision record 0117.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use openchuck_app::model::TransformStep;
use openchuck_app::overview::{
    ItemRef, ItemState, MessageAction, Overview, PrimaryKind, SetupPart, Severity, Step,
};
use openchuck_app::project::id::{ModelId, OperationId, SelectionSetId, SetupId, ToolId};
use openchuck_app::project::{
    Item, ItemId, Operation, OrderKey, ParameterValue, Placement, SelectionSet, SelectionTarget,
    Setup, ToolSnapshot,
};
use openchuck_app::setup::{
    HeldRegion, MachineProfile, Millimetres, Stock, Tab, TabLayout, TabShape,
};
use openchuck_app::{
    Application, Command, Event, JobId, JobLog, OutputGrouping, PostSelection, Threads,
};
use openchuck_path::{CutPoint, ParameterSchema, Parameters, Pass, Strategy, StrategyInput};
use openchuck_types::job::JobContext;
use openchuck_types::tool::{ControllerMapping, CuttingData, Tool, ToolShape};
use openchuck_types::units::{Degrees, FeedRate, SpindleSpeed};

/// A quick strategy: one ring of cut points round the part.
#[derive(Debug)]
struct OneRing;

impl Strategy for OneRing {
    fn name(&self) -> &str {
        "one_ring"
    }
    fn label(&self) -> &str {
        "One ring"
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
        let points = (0..=36)
            .filter_map(|i| {
                let a = Degrees(10.0 * f64::from(i));
                let line = input
                    .rotary
                    .tool_line(Millimetres(0.0), a, Millimetres(0.0));
                let c = input.surface.drop_tool(input.tool, &line)?;
                Some(CutPoint {
                    tip: c.tip,
                    a,
                    contact: c.contact,
                })
            })
            .collect();
        Ok(vec![Pass {
            points,
            stock: None,
            rapids: Vec::new(),
        }])
    }
}

/// A strategy that works until it is cancelled, to see an operation running.
#[derive(Debug)]
struct UntilCancelled;

impl Strategy for UntilCancelled {
    fn name(&self) -> &str {
        "until_cancelled"
    }
    fn label(&self) -> &str {
        "Until cancelled"
    }
    fn schema(&self) -> ParameterSchema {
        ParameterSchema::with_common(Vec::new())
    }
    fn passes(
        &self,
        _: &StrategyInput<'_>,
        _: &Parameters,
        context: &JobContext<'_>,
    ) -> openchuck_path::Result<Vec<Pass>> {
        loop {
            if context.check_cancelled().is_err() {
                return Err(openchuck_path::PathError::Cancelled);
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}

struct Fixture {
    dir: PathBuf,
    app: Application,
    model: ModelId,
    setup: SetupId,
    tool: ToolId,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn empty(name: &str) -> (PathBuf, Application) {
    let dir =
        std::env::temp_dir().join(format!("openchuck-overview-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut app = Application::create(&dir, name, Threads::Exactly(2)).unwrap();
    app.register_strategy(Arc::new(OneRing)).unwrap();
    app.register_strategy(Arc::new(UntilCancelled)).unwrap();
    (dir, app)
}

fn add_model(dir: &Path, app: &mut Application, file: &str) -> ModelId {
    std::fs::create_dir_all(dir.join("models")).unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/stl")
        .join(file);
    let target = dir.join("models").join(file);
    std::fs::copy(source, &target).unwrap();
    let (model, _) = app.add_model(&target, None).unwrap();
    app.execute(Command::SetModelTransform {
        model,
        transform: vec![TransformStep::CentreOnRotaryAxis],
    })
    .unwrap();
    model
}

/// The corpus cube on the example machine with an oak cylinder of stock, a
/// ball tool, and no operation yet.
fn fixture(name: &str, file: &str) -> Fixture {
    let (dir, mut app) = empty(name);
    let model = add_model(&dir, &mut app, file);
    app.execute(Command::SetMachine(Some(Box::new(
        MachineProfile::example_x_rotary(),
    ))))
    .unwrap();
    let setup = SetupId::new();
    let mut s = Setup::new(setup, "Setup 1");
    s.part = Some(model);
    s.material = Some("oak".into());
    s.stock = Some(Stock::Cylinder {
        diameter: Millimetres(20.0),
        length: Millimetres(14.0),
        x_start: Millimetres(-7.0),
    });
    let tool = ToolId::new();
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
    let snapshot = ToolSnapshot {
        id: tool,
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
    };
    for item in [Item::Setup(s), Item::Tool(snapshot)] {
        app.execute(Command::Add {
            item,
            placement: Placement::End,
        })
        .unwrap();
    }
    Fixture {
        dir,
        app,
        model,
        setup,
        tool,
    }
}

impl Fixture {
    fn add_operation(&mut self, name: &str, strategy: &str) -> OperationId {
        let id = OperationId::new();
        self.app
            .execute(Command::Add {
                item: Item::Operation(Operation {
                    enabled: true,
                    id,
                    order: OrderKey::default(),
                    name: name.into(),
                    setup: self.setup,
                    strategy: strategy.into(),
                    tool: Some(self.tool),
                    selections: Vec::new(),
                    boundary: None,
                    avoid: Vec::new(),
                    check: Vec::new(),
                    restriction_options: Default::default(),
                    parameters: BTreeMap::new(),
                }),
                placement: Placement::End,
            })
            .unwrap();
        id
    }

    /// The overview after the checks have run on the project as it is.
    fn overview(&self) -> Overview {
        let checks = self.app.start_checks(None).wait().unwrap();
        self.app.overview(Some(&checks), &JobLog::default())
    }

    fn state(&self, o: &Overview, op: OperationId) -> ItemState {
        o.status(ItemRef::Operation(op)).unwrap().state
    }

    fn set_stepover(&mut self, op: OperationId, value: f64) {
        self.app
            .execute(Command::SetParameter {
                operation: op,
                name: "stepover".into(),
                value: Some(ParameterValue::Number(value)),
            })
            .unwrap();
    }
}

#[test]
fn req_ui_10_req_ui_14_post_and_export_names_the_first_thing_missing() {
    // Decision 0148 item 1 (issue 140): the File menu's Post and export is
    // enabled with a model, a setup, a machine, a tool and an operation, and
    // otherwise names the first of them that is missing. Operations need not
    // be generated: the export dialog makes its own files.
    let (dir, mut app) = empty("post-blocker");
    let blocker = |app: &Application| app.overview(None, &JobLog::default()).post_blocker();
    assert_eq!(blocker(&app), Some("Import a model first"));
    let model = add_model(&dir, &mut app, "cube_ascii.stl");
    assert_eq!(blocker(&app), Some("Add a setup first"));
    let setup = SetupId::new();
    let mut s = Setup::new(setup, "Setup 1");
    s.part = Some(model);
    app.execute(Command::Add {
        item: Item::Setup(s),
        placement: Placement::End,
    })
    .unwrap();
    assert_eq!(blocker(&app), Some("Choose a machine first"));
    app.execute(Command::SetMachine(Some(Box::new(
        MachineProfile::example_x_rotary(),
    ))))
    .unwrap();
    assert_eq!(blocker(&app), Some("Add a tool first"));
    let tool = ToolId::new();
    app.execute(Command::Add {
        item: Item::Tool(ToolSnapshot {
            id: tool,
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
            cutting_data: BTreeMap::new(),
        }),
        placement: Placement::End,
    })
    .unwrap();
    assert_eq!(blocker(&app), Some("Add an operation first"));
    app.execute(Command::Add {
        item: Item::Operation(Operation {
            enabled: true,
            id: OperationId::new(),
            order: OrderKey::default(),
            name: "Ring".into(),
            setup,
            strategy: "one_ring".into(),
            tool: Some(tool),
            selections: Vec::new(),
            boundary: None,
            avoid: Vec::new(),
            check: Vec::new(),
            restriction_options: Default::default(),
            parameters: BTreeMap::new(),
        }),
        placement: Placement::End,
    })
    .unwrap();
    // Nothing is generated, and the menu item is still enabled. So is the
    // Post and export step, which gives the same reason (OI-436, decision
    // 0148 item 1); Simulate still needs a generated operation.
    assert_eq!(blocker(&app), None);
    let o = app.overview(None, &JobLog::default());
    assert!(o.step(Step::Export).is_available());
    assert!(!o.step(Step::Simulate).is_available());
    drop(app);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn req_ui_10_each_step_says_why_it_is_unavailable() {
    // An empty project: only the Model step, and "Import model…".
    let (dir, mut app) = empty("steps");
    let o = app.overview(None, &JobLog::default());
    assert!(o.step(Step::Model).is_available());
    let reason = |o: &Overview, s| o.step(s).unavailable.clone();
    assert_eq!(
        reason(&o, Step::Setup).as_deref(),
        Some("Import a model first")
    );
    assert_eq!(
        reason(&o, Step::Operations).as_deref(),
        Some("Set up the machine and stock first")
    );
    assert_eq!(
        reason(&o, Step::Simulate).as_deref(),
        Some("Generate an operation first")
    );
    // Post and export names what the File menu names (decision 0148).
    assert_eq!(
        reason(&o, Step::Export).as_deref(),
        Some("Import a model first")
    );
    assert_eq!(
        reason(&o, Step::Export).as_deref(),
        o.post_blocker().map(str::to_owned).as_deref()
    );
    assert_eq!(o.primary(Step::Model).kind, PrimaryKind::ImportModel);

    // A model only: Setup opens; Operations still waits for the machine.
    add_model(&dir, &mut app, "cube_ascii.stl");
    let o = app.overview(None, &JobLog::default());
    assert!(o.step(Step::Setup).is_available());
    assert!(!o.step(Step::Operations).is_available());
    let primary = o.primary(Step::Model);
    assert_eq!(primary.kind, PrimaryKind::Continue(Step::Setup));
    assert_eq!(primary.label, "Continue to setup");
    assert!(primary.blocked.is_none());
    // Disabled means explained (specification section 5).
    let primary = o.primary(Step::Setup);
    assert_eq!(primary.label, "Continue to operations");
    assert_eq!(
        primary.blocked.as_deref(),
        Some("Set up the machine and stock first")
    );
    drop(app);
    let _ = std::fs::remove_dir_all(dir);

    // A setup with a machine and stock: Operations opens.
    let mut f = fixture("steps-setup", "cube_ascii.stl");
    let o = f.overview();
    assert!(o.step(Step::Operations).is_available());
    assert!(!o.step(Step::Simulate).is_available());
    assert_eq!(
        o.step(Step::Export).unavailable.as_deref(),
        Some("Add an operation first")
    );
    assert!(o.step(Step::Model).complete);
    assert!(!o.step(Step::Operations).complete);

    // A generated operation: Simulate and Post and export open.
    let op = f.add_operation("Ring", "one_ring");
    let o = f.overview();
    assert_eq!(
        o.primary(Step::Operations).kind,
        PrimaryKind::GenerateStale(1)
    );
    assert_eq!(o.primary(Step::Operations).label, "Generate stale (1)");
    f.app.regenerate().unwrap();
    let o = f.overview();
    for s in Step::ALL {
        assert!(o.step(s).is_available(), "{s:?}");
    }
    assert!(o.step(Step::Operations).complete);
    assert!(!o.step(Step::Simulate).complete);
    assert_eq!(f.state(&o, op), ItemState::UpToDate);
    assert_eq!(
        o.primary(Step::Operations).kind,
        PrimaryKind::Continue(Step::Simulate)
    );
}

#[test]
fn req_ui_10_the_primary_action_follows_the_rule_on_simulate_and_export() {
    let mut f = fixture("primary", "cube_ascii.stl");
    let first = f.add_operation("Ring", "one_ring");
    f.add_operation("Ring again", "one_ring");
    // Never generated: generating and simulating names the first.
    let o = f.overview();
    let p = o.primary(Step::Simulate);
    assert_eq!(p.kind, PrimaryKind::GenerateAndSimulate(1));
    assert_eq!(p.label, "Generate and simulate op 1");
    // Generated but not simulated.
    f.app.regenerate().unwrap();
    assert_eq!(
        f.overview().primary(Step::Simulate).kind,
        PrimaryKind::RunSimulation
    );
    let run = f.app.simulate().unwrap();
    let o = f.overview();
    assert!(o.step(Step::Simulate).complete);
    assert_eq!(
        o.primary(Step::Simulate).kind,
        PrimaryKind::Continue(Step::Export)
    );
    assert_eq!(
        o.primary(Step::Simulate).label,
        "Continue to post and export"
    );
    assert_eq!(o.primary(Step::Export).kind, PrimaryKind::OpenExport);
    assert!(!o.step(Step::Export).complete);
    f.app.record_export(&run);
    assert!(f.overview().step(Step::Export).complete);
    // An edit makes the first stale; simulating it generates it too.
    f.set_stepover(first, 0.5);
    let o = f.overview();
    assert_eq!(
        o.primary(Step::Simulate).kind,
        PrimaryKind::GenerateAndSimulate(1)
    );
    assert!(!o.step(Step::Export).complete);
}

#[test]
fn req_ui_11_an_edit_to_operation_n_marks_n_and_later_operations_stale() {
    let mut f = fixture("stale", "cube_ascii.stl");
    let one = f.add_operation("Ring", "one_ring");
    let two = f.add_operation("Ring again", "one_ring");
    let three = f.add_operation("Ring last", "one_ring");
    let o = f.overview();
    assert_eq!(
        o.status(ItemRef::Operation(one)).unwrap().cause,
        "Stale: not generated yet"
    );
    f.app.regenerate().unwrap();
    let o = f.overview();
    for op in [one, two, three] {
        assert_eq!(f.state(&o, op), ItemState::UpToDate);
    }
    assert!(o.stale.is_empty());

    // Operation 2 changes: 2 and 3 are stale, 1 is not.
    f.set_stepover(two, 0.5);
    let o = f.overview();
    assert_eq!(f.state(&o, one), ItemState::UpToDate);
    assert_eq!(f.state(&o, two), ItemState::Stale);
    assert_eq!(f.state(&o, three), ItemState::Stale);
    assert_eq!(o.stale, [two, three]);
    assert_eq!(
        o.status(ItemRef::Operation(three)).unwrap().cause,
        "Stale: operation 2 changed"
    );
    assert_eq!(o.primary(Step::Operations).label, "Generate stale (2)");

    // Undoing the edit makes them up to date again: their inputs are the
    // ones they were made from.
    f.app.undo().unwrap();
    let o = f.overview();
    assert!(o.stale.is_empty());

    // A disabled operation is off, and the ones after it are stale.
    f.app
        .execute(Command::SetOperationEnabled {
            operation: two,
            enabled: false,
        })
        .unwrap();
    let o = f.overview();
    assert_eq!(f.state(&o, two), ItemState::Off);
    assert_eq!(f.state(&o, one), ItemState::UpToDate);
    assert_eq!(f.state(&o, three), ItemState::Stale);
    assert!(
        o.status(ItemRef::Operation(two))
            .unwrap()
            .cause
            .starts_with("Off: ")
    );
}

#[test]
fn req_ui_11_a_running_job_gives_the_operation_running() {
    let mut f = fixture("running", "cube_ascii.stl");
    let op = f.add_operation("Forever", "until_cancelled");
    let mut log = JobLog::default();
    let handle = f.app.start_regenerate();
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        log.update(f.app.drain_events());
        let o = f.app.overview(None, &log);
        if f.state(&o, op) == ItemState::Running {
            assert_eq!(
                o.status(ItemRef::Operation(op)).unwrap().cause,
                "Running: generating"
            );
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the operation never showed running"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    // The Jobs tab's Cancel stops it by its ID; nothing is recorded.
    assert!(f.app.cancel_job(handle.id()));
    assert!(handle.wait().unwrap_err().is_cancelled());
    log.update(f.app.drain_events());
    let o = f.app.overview(None, &log);
    assert_eq!(f.state(&o, op), ItemState::Stale);
    assert!(log.running().is_empty());
}

#[test]
fn req_ui_11_req_ui_12_a_selection_set_with_a_problem_is_an_error_up_the_tree() {
    let mut f = fixture("set-error", "cube_ascii.stl");
    let op = f.add_operation("Ring", "one_ring");
    let hash = f.app.project().models()[0].hash;
    let set = SelectionSetId::new();
    f.app
        .execute(Command::Add {
            item: Item::SelectionSet(SelectionSet {
                id: set,
                order: OrderKey::default(),
                name: "Roof".into(),
                model: f.model,
                model_hash: hash,
                target: SelectionTarget::Triangles(BTreeSet::new()),
                descriptors: Vec::new(),
                stock_to_leave: None,
                colour: None,
                keep_tangent_edges: None,
            }),
            placement: Placement::End,
        })
        .unwrap();
    let mut operation = f.app.project().operation(op).unwrap().clone();
    operation.selections = vec![set];
    f.app
        .execute(Command::Replace(Item::Operation(operation)))
        .unwrap();
    let o = f.overview();
    assert_eq!(
        o.status(ItemRef::SelectionSet(set)).unwrap().state,
        ItemState::Error
    );
    let op_status = o.status(ItemRef::Operation(op)).unwrap();
    assert_eq!(op_status.state, ItemState::Error);
    assert!(op_status.cause.contains("'Roof'"), "{}", op_status.cause);
    let (index, message) = o
        .messages
        .iter()
        .enumerate()
        .find(|(_, m)| m.source == Some(ItemRef::SelectionSet(set)))
        .unwrap();
    assert_eq!(message.severity, Severity::Error);
    assert_eq!(message.action, Some(MessageAction::Reselect(set)));
    assert_eq!(message.source_name, "Roof");
    // The error blocks generating, names the problem and points at its row.
    let primary = o.primary(Step::Operations);
    assert_eq!(primary.message, Some(index));
    assert!(primary.blocked.unwrap().contains("'Roof'"));
}

#[test]
fn req_ui_11_req_ui_12_a_mesh_finding_warns_on_the_model() {
    let f = fixture("mesh-warning", "defect_hole.stl");
    let o = f.overview();
    let status = o.status(ItemRef::Model(f.model)).unwrap();
    assert_eq!(status.state, ItemState::Warning);
    assert!(
        status.cause.starts_with("Warning: mesh check"),
        "{}",
        status.cause
    );
    let message = o
        .messages
        .iter()
        .find(|m| m.source == Some(ItemRef::Model(f.model)) && m.severity == Severity::Warning)
        .unwrap();
    assert!(
        message.text.starts_with("The mesh check found"),
        "{}",
        message.text
    );
    assert_eq!(message.source_name, "models/defect_hole.stl");
    assert_eq!(
        message.action,
        Some(MessageAction::Show(ItemRef::Model(f.model)))
    );
}

#[test]
fn req_ui_12_each_source_gives_one_message_and_fixing_it_removes_the_message() {
    let mut f = fixture("messages", "cube_ascii.stl");
    let op = f.add_operation("Ring", "one_ring");
    f.app.regenerate().unwrap();
    let baseline = f.overview().messages;

    // A seeded file change.
    let file = f.dir.join("models/cube_ascii.stl");
    let original = std::fs::read(&file).unwrap();
    let mut changed = original.clone();
    changed.extend_from_slice(b"\n");
    std::fs::write(&file, &changed).unwrap();
    // A setup check: the stock outgrows the safe cylinder.
    let mut setup = f.app.project().setup(f.setup).unwrap().clone();
    let before = setup.clone();
    setup.stock = Some(Stock::Cylinder {
        diameter: Millimetres(400.0),
        length: Millimetres(14.0),
        x_start: Millimetres(-7.0),
    });
    f.app.execute(Command::Replace(Item::Setup(setup))).unwrap();
    // A generation error: the operation loses its tool.
    let mut operation = f.app.project().operation(op).unwrap().clone();
    operation.tool = None;
    f.app
        .execute(Command::Replace(Item::Operation(operation)))
        .unwrap();
    assert!(f.app.regenerate().is_err());

    let o = f.overview();
    let file_messages: Vec<_> = o
        .messages
        .iter()
        .filter(|m| m.text.contains("has changed since it was added"))
        .collect();
    assert_eq!(file_messages.len(), 1, "{file_messages:?}");
    assert_eq!(file_messages[0].source, Some(ItemRef::Model(f.model)));
    let setup_messages: Vec<_> = o
        .messages
        .iter()
        .filter(|m| {
            m.source == Some(ItemRef::SetupPart(f.setup, SetupPart::WorkOffset))
                && m.text.contains("safe cylinder")
        })
        .collect();
    assert_eq!(setup_messages.len(), 1);
    assert_eq!(
        setup_messages[0].action,
        Some(MessageAction::ReviewSetup(f.setup))
    );
    assert!(
        o.setup_checks
            .iter()
            .any(|c| !c.passed && c.text.contains("safe cylinder"))
    );
    let generation: Vec<_> = o
        .messages
        .iter()
        .filter(|m| m.source == Some(ItemRef::Operation(op)))
        .collect();
    assert_eq!(generation.len(), 1);
    assert_eq!(generation[0].severity, Severity::Error);
    assert!(
        generation[0].text.contains("no tool"),
        "{}",
        generation[0].text
    );
    assert_eq!(generation[0].source_name, "1 Ring");
    assert_eq!(
        o.status(ItemRef::Operation(op)).unwrap().state,
        ItemState::Error
    );
    assert_eq!(
        o.status(ItemRef::Setup(f.setup)).unwrap().state,
        ItemState::Warning
    );
    // The error blocks generating, and the primary points at its message.
    let primary = o.primary(Step::Operations);
    let index = o
        .messages
        .iter()
        .position(|m| m.source == Some(ItemRef::Operation(op)))
        .unwrap();
    assert_eq!(primary.message, Some(index));
    assert!(primary.blocked.unwrap().contains("no tool"));
    // Sorted by severity.
    let severities: Vec<_> = o.messages.iter().map(|m| m.severity).collect();
    let mut sorted = severities.clone();
    sorted.sort();
    assert_eq!(severities, sorted);

    // Fixing each cause removes its message.
    std::fs::write(&file, &original).unwrap();
    f.app
        .execute(Command::Replace(Item::Setup(before)))
        .unwrap();
    let mut operation = f.app.project().operation(op).unwrap().clone();
    operation.tool = Some(f.tool);
    f.app
        .execute(Command::Replace(Item::Operation(operation)))
        .unwrap();
    let o = f.overview();
    assert_eq!(o.messages, baseline);
    assert_ne!(
        o.status(ItemRef::Operation(op)).unwrap().state,
        ItemState::Error
    );
}

#[test]
fn req_ui_12_checks_reuse_what_has_not_changed() {
    let mut f = fixture("reuse", "cube_ascii.stl");
    let op = f.add_operation("Ring", "one_ring");
    let first = Arc::new(f.app.start_checks(None).wait().unwrap());
    assert_eq!(first.revision(), f.app.revision());
    f.set_stepover(op, 0.5);
    let second = f.app.start_checks(Some(Arc::clone(&first))).wait().unwrap();
    assert_eq!(second.revision(), f.app.revision());
    let jobs = JobLog::default();
    let o = f.app.overview(Some(&second), &jobs);
    assert!(o.checks_current);
    let stale = f.app.overview(Some(&first), &jobs);
    assert!(!stale.checks_current);
    assert_eq!(o.messages, stale.messages);
}

#[test]
fn req_ui_11_req_ui_12_a_missing_model_file_is_an_error_that_blocks_setup() {
    let f = fixture("missing-file", "cube_ascii.stl");
    std::fs::remove_file(f.dir.join("models/cube_ascii.stl")).unwrap();
    let o = f.overview();
    let item = ItemRef::Model(f.model);
    assert_eq!(o.status(item).unwrap().state, ItemState::Error);
    let (index, message) = o
        .messages
        .iter()
        .enumerate()
        .find(|(_, m)| m.source == Some(item))
        .unwrap();
    assert_eq!(message.severity, Severity::Error);
    assert!(message.text.contains("is missing"), "{}", message.text);
    assert!(!o.step(Step::Model).complete);
    // The error blocks continuing, and points at its message.
    let primary = o.primary(Step::Model);
    assert_eq!(primary.kind, PrimaryKind::Continue(Step::Setup));
    assert_eq!(primary.message, Some(index));
    assert!(primary.blocked.is_some());
    // It does not block the Setup step's own primary.
    assert!(o.primary(Step::Setup).message.is_none());
}

#[test]
fn req_ui_11_req_ui_12_a_project_with_no_machine_warns_on_each_setup() {
    let mut f = fixture("no-machine", "cube_ascii.stl");
    f.app.execute(Command::SetMachine(None)).unwrap();
    let o = f.overview();
    let machine = ItemRef::SetupPart(f.setup, SetupPart::Machine);
    assert_eq!(o.status(machine).unwrap().state, ItemState::Warning);
    // Problems go up the tree.
    assert_eq!(
        o.status(ItemRef::Setup(f.setup)).unwrap().state,
        ItemState::Warning
    );
    let message = o
        .messages
        .iter()
        .find(|m| m.source == Some(machine))
        .unwrap();
    assert_eq!(message.action, Some(MessageAction::ReviewSetup(f.setup)));
    assert!(
        o.setup_checks
            .iter()
            .any(|c| !c.passed && c.setup == f.setup)
    );
    assert_eq!(
        o.step(Step::Operations).unavailable.as_deref(),
        Some("Set up the machine and stock first")
    );
    assert!(o.machine_warnings.is_empty());
}

#[test]
fn req_ui_11_a_setup_with_no_stock_warns_on_its_stock_row() {
    let mut f = fixture("no-stock", "cube_ascii.stl");
    let mut setup = f.app.project().setup(f.setup).unwrap().clone();
    setup.stock = None;
    f.app
        .execute(Command::Replace(Item::Setup(setup.clone())))
        .unwrap();
    let o = f.overview();
    let stock = ItemRef::SetupPart(f.setup, SetupPart::Stock);
    assert_eq!(o.status(stock).unwrap().state, ItemState::Warning);
    let row = openchuck_app::overview::setup_row(f.app.project(), &setup, SetupPart::Stock);
    assert_eq!(row.label, "Stock · none");
    assert!(!o.step(Step::Setup).complete);
}

#[test]
fn req_ui_11_req_ui_12_workholding_findings_and_reach_are_messages() {
    let mut f = fixture("workholding", "cube_ascii.stl");
    let op = f.add_operation("Ring", "one_ring");
    // The cube spans X 0 to 10 (its vertices are at its corners) and the
    // stock X -7 to 7: a held region from the stock's start over the cube's
    // low end, and a tab in the middle of the part, outside the head and tail
    // zones.
    let mut setup = f.app.project().setup(f.setup).unwrap().clone();
    setup.workholding.start = Some(HeldRegion {
        length: Millimetres(8.0),
        diameter: None,
    });
    setup.workholding.tabs = vec![Tab {
        x_start: Millimetres(4.0),
        length: Millimetres(2.0),
        shape: TabShape::Cylinder {
            diameter: Millimetres(4.0),
        },
        layout: TabLayout::OnAxis,
    }];
    f.app
        .execute(Command::Replace(Item::Setup(setup.clone())))
        .unwrap();
    let row = openchuck_app::overview::setup_row(f.app.project(), &setup, SetupPart::Workholding);
    assert_eq!(row.chip.as_deref(), Some("1 held · 1 tab"));
    // A selection of the whole part reaches the held region.
    let all = f
        .app
        .save_selection("Everything", f.model, (0..12).collect::<BTreeSet<u32>>())
        .unwrap();
    let mut operation = f.app.project().operation(op).unwrap().clone();
    operation.selections = vec![all];
    f.app
        .execute(Command::Replace(Item::Operation(operation)))
        .unwrap();
    let first = Arc::new(f.app.start_checks(None).wait().unwrap());
    let o = f.app.overview(Some(&first), &JobLog::default());
    let workholding = ItemRef::SetupPart(f.setup, SetupPart::Workholding);
    let status = o.status(workholding).unwrap();
    assert!(
        matches!(status.state, ItemState::Warning | ItemState::Error),
        "{status:?}"
    );
    assert!(
        o.messages.iter().any(|m| m.source == Some(workholding)
            && m.action == Some(MessageAction::EditWorkholding(f.setup))),
        "{:?}",
        o.messages
    );
    let reach: Vec<_> = o
        .messages
        .iter()
        .filter(|m| m.source == Some(ItemRef::Operation(op)))
        .collect();
    assert!(!reach.is_empty(), "{:?}", o.messages);
    assert!(
        reach
            .iter()
            .all(|m| m.action == Some(MessageAction::EditWorkholding(f.setup)))
    );
    // The reach notes are reused while the operation is unchanged.
    let again = f.app.start_checks(Some(first)).wait().unwrap();
    let o2 = f.app.overview(Some(&again), &JobLog::default());
    assert_eq!(o.messages, o2.messages);
}

#[test]
fn req_ui_12_a_unit_question_is_a_warning_on_the_model() {
    let f = fixture("unit-question", "cube_inch.stl");
    let o = f.overview();
    let item = ItemRef::Model(f.model);
    assert_eq!(o.status(item).unwrap().state, ItemState::Warning);
    assert!(
        o.messages
            .iter()
            .any(|m| m.source == Some(item) && m.severity == Severity::Warning)
    );
}

#[test]
fn req_ui_11_req_ui_12_jobs_show_on_items_and_failures_are_messages() {
    let f = fixture("job-messages", "cube_ascii.stl");
    let mut log = JobLog::default();
    log.update([Event::JobStarted {
        job: JobId(900),
        name: "build scene".into(),
    }]);
    let o = f.app.overview(None, &log);
    assert_eq!(
        o.status(ItemRef::Model(f.model)).unwrap().state,
        ItemState::Running
    );
    log.update([Event::JobFinished {
        job: JobId(900),
        error: Some("the mesh could not be placed".into()),
        cancelled: false,
    }]);
    let o = f.app.overview(None, &log);
    let failed = o
        .messages
        .iter()
        .find(|m| m.text.starts_with("Building the view failed"))
        .unwrap();
    assert_eq!(failed.severity, Severity::Error);
    assert!(failed.text.contains("could not be placed"));
}

#[test]
fn req_ui_12_a_posting_failure_after_the_operations_offers_to_generate() {
    let mut f = fixture("post-failure", "cube_ascii.stl");
    f.add_operation("Ring", "one_ring");
    let post = PostSelection::parse("no-such-post");
    let result = f
        .app
        .start_regenerate_with_post(post, OutputGrouping::default())
        .wait();
    assert!(result.is_err());
    let o = f.overview();
    let message = o
        .messages
        .iter()
        .find(|m| m.text.starts_with("Generating failed"))
        .unwrap();
    assert_eq!(message.source, None);
    assert_eq!(message.action, Some(MessageAction::Generate));
}

#[test]
fn req_ui_11_setup_rows_say_what_each_part_holds() {
    use openchuck_app::overview::setup_row;
    use openchuck_app::setup::{Aabb, Point3};

    let mut f = fixture("setup-rows", "cube_ascii.stl");
    let row = |f: &Fixture, part| {
        let s = f.app.project().setup(f.setup).unwrap();
        setup_row(f.app.project(), s, part)
    };
    let machine = row(&f, SetupPart::Machine);
    assert_eq!(machine.label, "Machine profile");
    assert!(machine.chip.is_some());
    assert_eq!(row(&f, SetupPart::Stock).label, "Stock · cylinder Ø20 × 14");
    assert_eq!(row(&f, SetupPart::PartPosition).label, "Part position");
    assert_eq!(row(&f, SetupPart::WorkOffset).chip.as_deref(), Some("G54"));
    assert_eq!(
        row(&f, SetupPart::Workholding).chip.as_deref(),
        Some("None")
    );
    let mut setup = f.app.project().setup(f.setup).unwrap().clone();
    let cases = [
        (
            Some(Stock::Box {
                bounds: Aabb {
                    min: Point3::new(-1.0, -6.0, -6.0),
                    max: Point3::new(11.0, 6.0, 6.0),
                },
            }),
            "Stock · box 12 × 12 × 12",
        ),
        (
            Some(Stock::OffsetBoundingBox {
                offset: Millimetres(1.5),
            }),
            "Stock · bounds + 1.5 mm",
        ),
    ];
    for (stock, label) in cases {
        setup.stock = stock;
        f.app
            .execute(Command::Replace(Item::Setup(setup.clone())))
            .unwrap();
        assert_eq!(row(&f, SetupPart::Stock).label, label);
    }
    let tab = |x: f64| Tab {
        x_start: Millimetres(x),
        length: Millimetres(1.0),
        shape: TabShape::Cylinder {
            diameter: Millimetres(4.0),
        },
        layout: TabLayout::OnAxis,
    };
    let held = HeldRegion {
        length: Millimetres(1.0),
        diameter: None,
    };
    let chip = |f: &mut Fixture, start: Option<HeldRegion>, tabs: Vec<Tab>| {
        let mut s = f.app.project().setup(f.setup).unwrap().clone();
        s.workholding.start = start;
        s.workholding.tabs = tabs;
        setup_row(f.app.project(), &s, SetupPart::Workholding).chip
    };
    assert_eq!(chip(&mut f, Some(held), vec![]).as_deref(), Some("1 held"));
    assert_eq!(
        chip(&mut f, None, vec![tab(-3.0)]).as_deref(),
        Some("1 tab")
    );
    assert_eq!(
        chip(&mut f, None, vec![tab(-3.0), tab(-5.0)]).as_deref(),
        Some("2 tabs")
    );
    assert_eq!(
        chip(&mut f, Some(held), vec![tab(-3.0), tab(-5.0)]).as_deref(),
        Some("1 held · 2 tabs")
    );
}

#[test]
fn req_ui_07_the_footer_shows_the_operation_tolerance() {
    let mut f = fixture("tolerance", "cube_ascii.stl");
    let op = f.add_operation("Ring", "one_ring");
    let default = f.app.operation_tolerance(op).unwrap();
    assert!(default > 0.0);
    f.app
        .execute(Command::SetParameter {
            operation: op,
            name: "tolerance".into(),
            value: Some(ParameterValue::Number(0.02)),
        })
        .unwrap();
    assert_eq!(f.app.operation_tolerance(op), Some(0.02));
    assert_eq!(f.app.operation_tolerance(OperationId::new()), None);
}

/// Two helical finishing operations and a third, on the fixture's cube.
fn three_operations(name: &str) -> (Fixture, [OperationId; 3]) {
    let mut f = fixture(name, "cube_ascii.stl");
    let one = f.add_operation("First", "helical_finishing");
    let two = f.add_operation("Second", "helical_finishing");
    let three = f.add_operation("Third", "helical_finishing");
    f.set_stepover(two, 0.6);
    f.set_stepover(three, 0.7);
    (f, [one, two, three])
}

#[test]
fn req_ui_14_req_ui_11_generating_up_to_an_operation_leaves_later_ones_stale() {
    let (mut f, [one, two, three]) = three_operations("until");
    // G on operation 2: operations 1 and 2 are made, 3 is not.
    let partial = f.app.regenerate_until(two).unwrap();
    assert_eq!(partial.stopped_after, Some(two));
    let made: Vec<_> = partial.groups.iter().map(|g| g.operation).collect();
    assert_eq!(made, [one, two]);
    assert!(partial.simulation.is_none());
    let o = f.overview();
    assert_eq!(f.state(&o, one), ItemState::UpToDate);
    assert_eq!(f.state(&o, two), ItemState::UpToDate);
    assert_eq!(f.state(&o, three), ItemState::Stale);
    assert_eq!(o.stale, [three]);
    assert_eq!(o.primary(Step::Operations).label, "Generate stale (1)");

    // Generating the rest reuses what the partial run made: no key changed.
    let full = f.app.regenerate().unwrap();
    assert_eq!(full.stopped_after, None);
    assert_eq!(full.reused, [one, two]);
    assert_eq!(full.generated, [three]);
    assert_eq!(full.toolpath.groups[..2], partial.toolpath.groups[..]);
    assert!(f.overview().stale.is_empty());

    // An edit to operation 1, then G on it: 2 and 3 stay stale, and say why.
    f.set_stepover(one, 0.5);
    f.app.regenerate_until(one).unwrap();
    let o = f.overview();
    assert_eq!(f.state(&o, one), ItemState::UpToDate);
    assert_eq!(o.stale, [two, three]);
    assert_eq!(
        o.status(ItemRef::Operation(three)).unwrap().cause,
        "Stale: operation 1 changed"
    );
}

#[test]
fn req_ui_14_generating_up_to_a_disabled_or_unknown_operation_is_refused() {
    let (mut f, [_, two, _]) = three_operations("until-refused");
    f.app
        .execute(Command::SetOperationEnabled {
            operation: two,
            enabled: false,
        })
        .unwrap();
    let e = f.app.regenerate_until(two).unwrap_err().to_string();
    assert!(e.contains("turned off"), "{e}");
    let e = f.app.regenerate_until(OperationId::new()).unwrap_err();
    assert!(
        matches!(e, openchuck_app::AppError::UnknownOperation(_)),
        "{e}"
    );
}

#[test]
fn req_gc_15_a_partial_run_leaves_later_operations_blocking_at_export() {
    let (f, [_, two, _]) = three_operations("until-export");
    let partial = f.app.regenerate_until(two).unwrap();
    let checks = f.app.export_checks(Some(&partial), None, None);
    let blocking: Vec<_> = checks
        .checks
        .iter()
        .filter(|c| c.level == openchuck_app::CheckLevel::Blocking)
        .map(|c| c.text.as_str())
        .collect();
    assert_eq!(blocking.len(), 1, "{blocking:?}");
    assert!(blocking[0].contains("'Third'"), "{blocking:?}");
}

#[test]
fn req_gc_07_req_nfr_06_a_run_up_to_an_operation_is_the_same_at_1_2_and_n_threads() {
    let (f, [_, two, _]) = three_operations("until-threads");
    f.app.save().unwrap();
    let runs: Vec<_> = [Threads::Exactly(1), Threads::Exactly(2), Threads::All]
        .into_iter()
        .map(|threads| {
            let app = Application::open(&f.dir, threads).unwrap();
            app.regenerate_until(two).unwrap()
        })
        .collect();
    for run in &runs[1..] {
        assert_eq!(run.toolpath, runs[0].toolpath);
        assert_eq!(run.generated, runs[0].generated);
    }
}

#[test]
fn req_nfr_15_setup_messages_name_machine_values_in_plain_words() {
    let mut f = fixture("plain-machine", "cube_ascii.stl");
    let z1 = openchuck_app::setup::builtin(openchuck_app::setup::MAKERA_Z1)
        .unwrap()
        .unwrap();
    f.app
        .execute(Command::SetMachine(Some(Box::new(z1))))
        .unwrap();
    // Part offset far along the axis: the stock runs past the X travel.
    let mut setup = f.app.project().setup(f.setup).unwrap().clone();
    setup.frame.part_offset = Millimetres(195.0);
    f.app.execute(Command::Replace(Item::Setup(setup))).unwrap();
    let checks = Arc::new(f.app.start_checks(None).wait().unwrap());
    let o = f.app.overview(Some(&checks), &JobLog::default());
    let text: Vec<&str> = o.messages.iter().map(|m| m.text.as_str()).collect();
    let uses = text
        .iter()
        .find(|t| t.contains("uses machine values"))
        .unwrap_or_else(|| panic!("{text:?}"));
    assert!(uses.contains("axis height, travel"), "{uses}");
    for field in ["rotary_origin", "max_feeds", "a_sense"] {
        assert!(!text.iter().any(|t| t.contains(field)), "{field}: {text:?}");
    }
}

/// Renames operation `op` with a `Replace`, as the inspector's name field does.
fn rename_operation(f: &mut Fixture, op: OperationId, name: &str) {
    let mut item = f.app.project().operation(op).unwrap().clone();
    item.name = name.into();
    f.app
        .execute(Command::Replace(Item::Operation(item)))
        .unwrap();
}

#[test]
fn req_ui_11_req_gc_07_a_rename_keeps_every_operation_up_to_date() {
    // The stale state follows the inputs of the operation's cache key, and the
    // name is not one of them (issue 209; decision 0172).
    let mut f = fixture("rename-stale", "cube_ascii.stl");
    let one = f.add_operation("Ring", "one_ring");
    let two = f.add_operation("Ring again", "one_ring");
    let run = f.app.simulate().unwrap();
    assert!(f.overview().stale.is_empty());

    rename_operation(&mut f, one, "Rough the barrel");
    rename_operation(&mut f, two, "Finish");
    let o = f.overview();
    assert_eq!(f.state(&o, one), ItemState::UpToDate);
    assert_eq!(f.state(&o, two), ItemState::UpToDate);
    assert!(o.stale.is_empty());

    // The export check agrees: nothing is out of date, and the simulation the
    // run made under the old names still counts for the renamed operations.
    let checks = f.app.export_checks(Some(&run), None, None);
    assert!(
        checks
            .checks
            .iter()
            .all(|c| !c.text.contains("out of date") && !c.text.contains("not been simulated")),
        "a rename left a check: {:?}",
        checks.checks
    );
    assert!(checks.can_write());

    // Generating again reuses every result: the key did not change.
    let again = f.app.regenerate().unwrap();
    assert!(again.generated.is_empty());
    assert_eq!(again.reused, [one, two]);
    assert_eq!(again.toolpath, run.toolpath);
}

#[test]
fn req_ui_11_renaming_a_setup_keeps_its_operations_up_to_date() {
    let mut f = fixture("rename-setup", "cube_ascii.stl");
    let one = f.add_operation("Ring", "one_ring");
    f.app.regenerate().unwrap();
    let mut setup = f.app.project().setup(f.setup).unwrap().clone();
    setup.name = "Barrel, end one".into();
    f.app.execute(Command::Replace(Item::Setup(setup))).unwrap();
    let o = f.overview();
    assert_eq!(f.state(&o, one), ItemState::UpToDate);
}

#[test]
fn req_ui_11_moving_and_turning_off_then_on_are_exact_inverses() {
    // The position is in the chain of keys, not in an operation's own key, so
    // putting an operation back where it was makes everything up to date.
    let (mut f, [one, two, three]) = three_operations("move-back");
    f.app.regenerate().unwrap();
    assert!(f.overview().stale.is_empty());

    f.app
        .execute(Command::Move {
            id: ItemId::Operation(three),
            index: 0,
        })
        .unwrap();
    let o = f.overview();
    assert!(o.stale.contains(&three), "the moved operation is stale");
    f.app
        .execute(Command::Move {
            id: ItemId::Operation(three),
            index: 2,
        })
        .unwrap();
    assert!(f.overview().stale.is_empty(), "back in place, up to date");

    f.app
        .execute(Command::SetOperationEnabled {
            operation: three,
            enabled: false,
        })
        .unwrap();
    f.app
        .execute(Command::SetOperationEnabled {
            operation: three,
            enabled: true,
        })
        .unwrap();
    let o = f.overview();
    assert!(o.stale.is_empty());
    assert_eq!(f.state(&o, one), ItemState::UpToDate);
    assert_eq!(f.state(&o, two), ItemState::UpToDate);
}

#[test]
fn req_nfr_12_req_gc_15_a_removed_or_disabled_operation_blocks_a_run_that_still_holds_it() {
    // Issue 196: the toolpath of an operation that is gone must not be used
    // as if it were current. A run that holds the path of an operation the
    // project no longer has, or has turned off, is out of date at export.
    let (mut f, [one, two, three]) = three_operations("removed");
    let run = f.app.regenerate().unwrap();
    assert!(f.app.export_checks(Some(&run), None, None).can_write());

    // Strategy change: the key changes, as before.
    let mut op = f.app.project().operation(three).unwrap().clone();
    op.strategy = "one_ring".into();
    f.app
        .execute(Command::Replace(Item::Operation(op)))
        .unwrap();
    assert!(!f.app.export_checks(Some(&run), None, None).can_write());
    f.app.undo().unwrap();
    assert!(f.app.export_checks(Some(&run), None, None).can_write());

    // Turned off.
    f.app
        .execute(Command::SetOperationEnabled {
            operation: three,
            enabled: false,
        })
        .unwrap();
    let checks = f.app.export_checks(Some(&run), None, None);
    assert!(!checks.can_write());
    assert!(
        checks
            .checks
            .iter()
            .any(|c| c.text.contains("Third") && c.text.contains("turned off")),
        "{:?}",
        checks.checks
    );
    f.app.undo().unwrap();

    // Removed.
    f.app.delete_operation(two).unwrap();
    let checks = f.app.export_checks(Some(&run), None, None);
    assert!(!checks.can_write());
    assert!(
        checks
            .checks
            .iter()
            .any(|c| c.text.contains("Second") && c.text.contains("removed")),
        "{:?}",
        checks.checks
    );

    // Generating again leaves the path of the removed operation out.
    let again = f.app.regenerate().unwrap();
    let made: Vec<_> = again.groups.iter().map(|g| g.operation).collect();
    assert_eq!(made, [one, three]);
    assert!(f.app.export_checks(Some(&again), None, None).can_write());
}
