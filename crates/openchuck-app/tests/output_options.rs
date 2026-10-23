// SPDX-License-Identifier: AGPL-3.0-or-later
//! Output grouping, comments and the setup sheet (GC-05, GC-06, GC-12;
//! decision record 0028).

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

use openchuck_app::model::TransformStep;
use openchuck_app::project::id::{OperationId, SetupId, ToolId};
use openchuck_app::project::{
    Item, Operation, OrderKey, ParameterValue, Placement, Setup, ToolSnapshot,
};
use openchuck_app::setup::{
    HeldRegion, MachineProfile, Millimetres, Stock, Tab, TabEnd, TabLayout, TabShape, Workholding,
};
use openchuck_app::{
    Application, Command, OutputGrouping, PostSelection, SetupSheet, Threads, Traceability,
};
use openchuck_types::tool::{ControllerMapping, CuttingData, Tool, ToolShape};
use openchuck_types::units::{FeedRate, SpindleSpeed};

fn ball(diameter: f64) -> Tool {
    Tool {
        shape: ToolShape::BallEnd,
        diameter: Millimetres(diameter),
        flute_length: Millimetres(10.0),
        overall_length: Millimetres(40.0),
        shank_diameter: Millimetres(diameter),
        flute_count: 2,
        material: Some("carbide".into()),
        centre_cutting: None,
    }
}

fn snapshot(name: &str, diameter: f64, number: u32) -> ToolSnapshot {
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
    ToolSnapshot {
        id: ToolId::new(),
        order: OrderKey::default(),
        name: name.into(),
        tool: ball(diameter),
        controller: ControllerMapping {
            number,
            length_offset: None,
            diameter_offset: None,
        },
        cutting_data,
    }
}

fn operation(name: &str, setup: SetupId, tool: ToolId, step: f64) -> Operation {
    let mut parameters = BTreeMap::new();
    parameters.insert("max_angle_step".to_owned(), ParameterValue::Number(step));
    // A wide pitch keeps the programs short.
    parameters.insert("stepover".to_owned(), ParameterValue::Number(5.0));
    Operation {
        id: OperationId::new(),
        order: OrderKey::default(),
        name: name.into(),
        enabled: true,
        setup,
        strategy: "helical_finishing".into(),
        tool: Some(tool),
        selections: Vec::new(),
        boundary: None,
        avoid: Vec::new(),
        check: Vec::new(),
        restriction_options: Default::default(),
        spindle_speed: None,
        parameters,
    }
}

fn add(app: &mut Application, item: Item) {
    app.execute(Command::Add {
        item,
        placement: Placement::End,
    })
    .unwrap();
}

/// The 10 mm corpus cube on the rotary axis, the example machine, one setup in
/// oak, two ball tools (T1 4 mm, T2 6 mm), and three operations in this order:
/// "Finish A" (T1), "Finish B" (T2) and "Touch up A" (T1). Coarse angle steps
/// keep the programs short.
fn project(name: &str, second_setup: bool) -> (PathBuf, Application) {
    // Tests run in parallel: each gets a folder of its own, whatever the project is called.
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let unique = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("openchuck-out-{unique}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut app = Application::create(&dir, name, Threads::Exactly(2)).unwrap();
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
    let mut setups = vec![(SetupId::new(), "Setup 1")];
    if second_setup {
        setups.push((SetupId::new(), "Flip / side 2"));
    }
    for (id, label) in &setups {
        let mut setup = Setup::new(*id, *label);
        setup.part = Some(model);
        setup.material = Some("oak".into());
        setup.stock = Some(Stock::Cylinder {
            diameter: Millimetres(20.0),
            length: Millimetres(14.0),
            x_start: Millimetres(-7.0),
        });
        add(&mut app, Item::Setup(setup));
    }
    let (a, b) = (snapshot("4 mm ball", 4.0, 1), snapshot("6 mm ball", 6.0, 2));
    let (a_id, b_id) = (a.id, b.id);
    add(&mut app, Item::Tool(a));
    add(&mut app, Item::Tool(b));
    let first = setups[0].0;
    add(
        &mut app,
        Item::Operation(operation("Finish A", first, a_id, 90.0)),
    );
    add(
        &mut app,
        Item::Operation(operation("Finish B", first, b_id, 60.0)),
    );
    add(
        &mut app,
        Item::Operation(operation("Touch up A", first, a_id, 90.0)),
    );
    if second_setup {
        add(
            &mut app,
            Item::Operation(operation("Side two", setups[1].0, a_id, 45.0)),
        );
    }
    (dir, app)
}

fn post(app: &mut Application, grouping: OutputGrouping) -> openchuck_app::Regeneration {
    app.set_post(Some(PostSelection::Builtin("makera-z1".into())));
    app.set_output_grouping(grouping);
    app.regenerate().unwrap()
}

fn names(result: &openchuck_app::Regeneration) -> Vec<&str> {
    result.gcode.iter().map(|(n, _)| n.as_str()).collect()
}

/// The end position of every fed move a program reaches (rapids between
/// operations are written only when the position changes, so they differ
/// with the grouping), written to six
/// decimals, in a stable order.
fn reached(text: &str) -> Vec<String> {
    let program = openchuck_app::gcode::read(text).unwrap();
    let replay =
        openchuck_app::gcode::replay_axis(&program, openchuck_app::gcode::Dialect::MakeraZ1, 'A')
            .unwrap();
    let mut ends: Vec<String> = replay
        .moves()
        .filter(|m| m.motion == openchuck_app::gcode::Motion::Linear)
        .map(|m| {
            format!(
                "{:.6} {:.6} {:.6} {:.6}",
                m.to[0], m.to[1], m.to[2], m.to[3]
            )
        })
        .collect();
    ends.sort();
    ends
}

#[test]
fn req_gc_05_the_default_is_one_file_per_setup() {
    let (dir, mut app) = project("default", false);
    assert_eq!(app.output_grouping(), OutputGrouping::Setup);
    app.set_post(Some(PostSelection::Builtin("makera-z1".into())));
    let result = app.regenerate().unwrap();
    assert_eq!(names(&result), ["01-setup-1.nc"]);
    assert!(result.reports.iter().all(|r| r.is_clean()));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_05_one_file_for_the_project_is_named_for_the_project() {
    let (dir, mut app) = project("whole", false);
    let result = post(&mut app, OutputGrouping::Project);
    assert_eq!(names(&result), ["01-whole.nc"]);
    assert!(result.reports[0].is_clean(), "{:?}", result.reports[0]);
    // With one setup it holds what the per-setup file holds.
    let per_setup = post(&mut app, OutputGrouping::Setup);
    assert_eq!(result.gcode[0].1, per_setup.gcode[0].1);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_05_one_file_for_the_project_is_refused_when_the_project_has_two_setups() {
    let (dir, mut app) = project("two-refused", true);
    app.set_post(Some(PostSelection::Builtin("makera-z1".into())));
    app.set_output_grouping(OutputGrouping::Project);
    let e = app.regenerate().unwrap_err().to_string();
    assert!(
        e.contains("one file for the project") && e.contains("per setup"),
        "{e}"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_05_one_file_per_setup_with_two_setups() {
    let (dir, mut app) = project("two-setups", true);
    let result = post(&mut app, OutputGrouping::Setup);
    assert_eq!(
        names(&result),
        ["01-setup-1.nc", "02-flip-side-2.nc"],
        "names are ASCII and numbered in machining order"
    );
    assert!(result.reports.iter().all(|r| r.is_clean()));
    assert!(result.gcode[1].1.contains("(operation Side two,"));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_05_one_file_per_operation() {
    let (dir, mut app) = project("per-op", false);
    let result = post(&mut app, OutputGrouping::Operation);
    assert_eq!(
        names(&result),
        [
            "01-setup-1-finish-a.nc",
            "02-setup-1-finish-b.nc",
            "03-setup-1-touch-up-a.nc"
        ]
    );
    assert!(result.reports.iter().all(|r| r.is_clean()));
    for ((_, text), operation) in result
        .gcode
        .iter()
        .zip(["Finish A", "Finish B", "Touch up A"])
    {
        assert!(
            text.starts_with(&format!("(per-op - Setup 1 - {operation})\n")),
            "{text}"
        );
        assert_eq!(
            text.matches("(operation ").count(),
            1,
            "one operation per file"
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_05_one_file_per_tool_keeps_the_machining_order() {
    // T1, T2, T1: Touch up A was planned on the stock Finish B leaves, so it
    // may not move ahead of Finish B into the first tool's file.
    let (dir, mut app) = project("per-tool", false);
    let result = post(&mut app, OutputGrouping::Tool);
    assert_eq!(
        names(&result),
        [
            "01-setup-1-t1-4-mm-ball.nc",
            "02-setup-1-t2-6-mm-ball.nc",
            "03-setup-1-t1-4-mm-ball.nc"
        ]
    );
    assert!(result.reports.iter().all(|r| r.is_clean()));
    for ((_, text), (operation, tool)) in result.gcode.iter().zip([
        ("Finish A", "T1 4 mm ball"),
        ("Finish B", "T2 6 mm ball"),
        ("Touch up A", "T1 4 mm ball"),
    ]) {
        assert!(text.contains(&format!("(operation {operation}, tool {tool},")));
        assert_eq!(text.matches("(operation ").count(), 1);
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_05_one_file_per_tool_gathers_consecutive_operations_of_one_tool() {
    // With Finish B off, Finish A and Touch up A follow each other on T1.
    let (dir, mut app) = project("per-tool-run", false);
    let off = app.project().operations()[1].id;
    app.execute(Command::SetOperationEnabled {
        operation: off,
        enabled: false,
    })
    .unwrap();
    let result = post(&mut app, OutputGrouping::Tool);
    assert_eq!(names(&result), ["01-setup-1-t1-4-mm-ball.nc"]);
    assert!(result.reports.iter().all(|r| r.is_clean()));
    let text = &result.gcode[0].1;
    assert!(text.contains("(operation Finish A, tool T1 4 mm ball,"));
    assert!(text.contains("(operation Touch up A, tool T1 4 mm ball,"));
    assert_eq!(text.matches("T1 M6").count(), 1, "one tool change per file");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_05_every_grouping_reaches_the_same_moves() {
    let (dir, mut app) = project("same-moves", false);
    let whole = reached(&post(&mut app, OutputGrouping::Setup).gcode[0].1);
    assert!(whole.len() > 50);
    for grouping in OutputGrouping::ALL {
        let result = post(&mut app, grouping);
        let mut together: Vec<String> = result.gcode.iter().flat_map(|(_, t)| reached(t)).collect();
        together.sort();
        assert_eq!(together, whole, "{grouping:?}");
        assert!(result.reports.iter().all(|r| r.is_clean()), "{grouping:?}");
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_05_req_gc_07_every_grouping_gives_the_same_bytes_at_any_thread_count() {
    let (dir, mut app) = project("det", false);
    app.save().unwrap();
    for grouping in OutputGrouping::ALL {
        let first = post(&mut app, grouping).gcode;
        for threads in [1, 4] {
            let mut again = Application::open(&dir, Threads::Exactly(threads)).unwrap();
            assert_eq!(
                post(&mut again, grouping).gcode,
                first,
                "{grouping:?} at {threads}"
            );
        }
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_05_a_grouping_is_spelled_by_name() {
    for grouping in OutputGrouping::ALL {
        assert_eq!(
            grouping.as_str().parse::<OutputGrouping>().unwrap(),
            grouping
        );
    }
    let e = "everything".parse::<OutputGrouping>().unwrap_err();
    assert!(e.contains("everything") && e.contains("operation"), "{e}");
}

#[test]
fn req_gc_06_comments_name_the_operation_and_the_tool_by_name_and_number() {
    let (dir, mut app) = project("comments", false);
    let text = post(&mut app, OutputGrouping::Setup).gcode.remove(0).1;
    let comments: Vec<&str> = text
        .lines()
        .filter(|l| l.starts_with("(operation "))
        .collect();
    assert_eq!(comments.len(), 3, "{text}");
    for (line, expected) in comments.iter().zip([
        "(operation Finish A, tool T1 4 mm ball, about ",
        "(operation Finish B, tool T2 6 mm ball, about ",
        "(operation Touch up A, tool T1 4 mm ball, about ",
    ]) {
        assert!(line.starts_with(expected), "{line}");
        assert!(line.ends_with(" s)"), "{line}");
    }
    // No identifier leaks into the file: the names are the project's.
    for operation in app.project().operations() {
        let id = operation.id.to_string();
        assert!(
            !text.contains(&id),
            "operation ID {id} leaks into the G-code"
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_06_a_name_with_brackets_or_accents_is_made_safe_in_the_comment() {
    let (dir, mut app) = project("comment-safe", false);
    let mut op = app.project().operations()[0].clone();
    op.name = "Fräsen (fine)".into();
    app.execute(Command::Replace(Item::Operation(op))).unwrap();
    let text = post(&mut app, OutputGrouping::Setup).gcode.remove(0).1;
    assert!(
        text.contains("(operation Fr?sen [fine], tool T1 4 mm ball,"),
        "{text}"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_05_file_names_are_ascii_and_unique_whatever_the_operation_names() {
    let (dir, mut app) = project("odd-names", false);
    for (i, name) in ["Rough (6 mm)/pass", "Rough (6 mm)/pass", "***"]
        .iter()
        .enumerate()
    {
        let mut op = app.project().operations()[i].clone();
        op.name = (*name).into();
        app.execute(Command::Replace(Item::Operation(op))).unwrap();
    }
    let result = post(&mut app, OutputGrouping::Operation);
    assert_eq!(
        names(&result),
        [
            "01-setup-1-rough-6-mm-pass.nc",
            "02-setup-1-rough-6-mm-pass.nc",
            "03-setup-1-operation.nc"
        ]
    );
    std::fs::remove_dir_all(dir).unwrap();
}

fn golden(folder: &str, name: &str, text: &str) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/golden/grouping")
        .join(folder)
        .join(name);
    openchuck_testkit::golden::check(&path, text).unwrap_or_else(|m| panic!("{m}"));
}

#[test]
fn req_gc_05_req_nfr_08_each_grouping_matches_its_golden_files() {
    let (dir, mut app) = project("golden", false);
    for grouping in OutputGrouping::ALL {
        for (name, text) in post(&mut app, grouping).gcode {
            golden(grouping.as_str(), &name, &text);
        }
    }
    std::fs::remove_dir_all(dir).unwrap();
}

/// NFR-16, GC-07: every program, in every grouping and for both posts, carries exactly one
/// `(OpenChuckCAM <workspace version>)` comment right after the title, and nothing else about the
/// build (no commit, date or dirty flag), so one version always writes the same bytes.
#[test]
fn req_nfr_16_req_gc_07_every_program_names_the_tool_and_the_workspace_version() {
    let (dir, mut app) = project("generator", false);
    let expected = format!("(OpenChuckCAM {})", env!("CARGO_PKG_VERSION"));
    for grouping in OutputGrouping::ALL {
        for (name, text) in post(&mut app, grouping).gcode {
            let lines: Vec<_> = text.lines().collect();
            assert_eq!(
                lines.iter().filter(|l| l.contains("OpenChuckCAM")).count(),
                1,
                "{name}"
            );
            assert_eq!(lines.get(1), Some(&expected.as_str()), "{name}");
            let commit = has_no_build_facts(&text);
            assert!(commit, "{name}: no build facts in the program");
        }
    }
    std::fs::remove_dir_all(dir).unwrap();
}

/// True when the text has no date, commit hash or dirty flag of a build.
fn has_no_build_facts(text: &str) -> bool {
    !text.contains("Uncommitted")
        && !text.contains("Built:")
        && !text.lines().any(|l| {
            l.split(|c: char| !c.is_ascii_hexdigit())
                .any(|w| w.len() == 40)
        })
}

// ---- the setup sheet (GC-12) ----

fn sheet_project() -> (PathBuf, Application) {
    let (dir, mut app) = project("sheet", true);
    // A tool no operation uses stays off the sheet.
    add(&mut app, Item::Tool(snapshot("spare <b>", 1.0, 9)));
    (dir, app)
}

fn cell_rows(sheet: &openchuck_app::SetupSheet, heading: &str) -> Vec<Vec<String>> {
    let section = sheet
        .sections
        .iter()
        .find(|s| s.heading == heading)
        .unwrap();
    match &section.body {
        openchuck_app::SheetBody::Table { rows, .. } => rows.clone(),
        other => panic!("{heading} is not a table: {other:?}"),
    }
}

#[test]
fn req_gc_12_the_sheet_lists_every_tool_used_with_its_controller_number() {
    let (dir, app) = sheet_project();
    let sheet = app.setup_sheet(None);
    let tools = cell_rows(&sheet, "Tools");
    let listed: Vec<(&str, &str)> = tools
        .iter()
        .map(|r| (r[0].as_str(), r[1].as_str()))
        .collect();
    assert_eq!(listed, [("T1", "4 mm ball"), ("T2", "6 mm ball")]);
    assert_eq!(
        tools[0][8], "Setup 1, Flip / side 2",
        "where each tool is used"
    );
    assert_eq!(tools[1][8], "Setup 1");
    let operations = cell_rows(&sheet, "Operations");
    assert_eq!(operations.len(), 4);
    assert_eq!(
        operations[1],
        [
            "2",
            "Finish B",
            "Helical finishing",
            "Setup 1",
            "T2 6 mm ball",
            "10000",
            "tool data: oak",
            "0.01 mm",
            "-"
        ]
    );
    std::fs::remove_dir_all(dir).unwrap();
}

/// GC-12, TL-05, decision 0196, OI-755 (owner decision, option A): the sheet says where each
/// operation's spindle speed came from, so it can be checked before the program runs (NFR-12).
#[test]
fn req_gc_12_req_tl_05_the_sheet_says_where_each_spindle_speed_came_from() {
    let (dir, mut app) = sheet_project();
    let mut typed = app.project().operations()[0].clone();
    typed.spindle_speed = Some(SpindleSpeed(8000.0));
    app.execute(Command::Replace(Item::Operation(typed)))
        .unwrap();
    let sheet = app.setup_sheet(None);
    let operations = cell_rows(&sheet, "Operations");
    assert_eq!(operations[0][5], "8000");
    assert_eq!(operations[0][6], "operation");
    assert_eq!(operations[1][5], "10000");
    assert_eq!(operations[1][6], "tool data: oak");
    let html = sheet.to_html();
    assert!(html.contains("Speed from"));
    assert!(html.contains("operation"));
    assert!(html.contains("tool data: oak"));
    // The same project gives the same sheet (GC-07).
    assert_eq!(html, app.setup_sheet(None).to_html());
    std::fs::remove_dir_all(dir).unwrap();
}

/// GC-12, NFR-12: an operation with no speed is shown as having none, never a guessed source.
#[test]
fn req_gc_12_req_nfr_12_the_sheet_shows_no_source_for_an_operation_without_a_speed() {
    let (dir, mut app) = sheet_project();
    let setup = app.project().setups()[0].clone();
    let mut setup = setup;
    setup.material = Some("pine".into());
    app.execute(Command::Replace(Item::Setup(setup))).unwrap();
    let operations = cell_rows(&app.setup_sheet(None), "Operations");
    assert_eq!(operations[0][5], "-");
    assert_eq!(operations[0][6], "none");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_12_the_sheet_shows_a_fractional_spindle_speed_as_set() {
    let (dir, mut app) = sheet_project();
    let mut op = app.project().operations()[0].clone();
    op.spindle_speed = Some(SpindleSpeed(0.4));
    app.execute(Command::Replace(Item::Operation(op))).unwrap();
    let operations = cell_rows(&app.setup_sheet(None), "Operations");
    assert_eq!(operations[0][5], "0.4");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_12_req_nfr_12_the_sheet_records_the_tolerance_each_operation_is_generated_with() {
    // Decision 0175 (issue #226): a path may be made with a known overcut
    // above its tolerance, so the sheet records the tolerance each operation
    // uses: its own value, as set and not rounded, or its strategy's default
    // (0.01 mm, unchanged).
    let (dir, mut app) = sheet_project();
    let first = app.project().operations()[0].id;
    app.execute(Command::SetParameter {
        operation: first,
        name: "tolerance".into(),
        value: Some(ParameterValue::Number(0.010049)),
    })
    .unwrap();
    // A whole number is stored without a decimal point (decision 0008
    // item 6), so a project read back holds it as an integer.
    let second = app.project().operations()[1].id;
    app.execute(Command::SetParameter {
        operation: second,
        name: "tolerance".into(),
        value: Some(ParameterValue::Integer(1)),
    })
    .unwrap();
    let sheet = app.setup_sheet(None);
    let operations = cell_rows(&sheet, "Operations");
    assert_eq!(operations[0][7], "0.010049 mm");
    assert_eq!(operations[1][7], "1 mm");
    assert!(
        operations[2..].iter().all(|r| r[7] == "0.01 mm"),
        "{operations:?}"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_12_the_sheet_describes_each_setups_stock_and_work_offset() {
    let (dir, app) = sheet_project();
    let sheet = app.setup_sheet(None);
    let setup = sheet
        .sections
        .iter()
        .find(|s| s.heading == "Setup: Setup 1")
        .unwrap();
    let openchuck_app::SheetBody::Facts(facts) = &setup.body else {
        panic!("facts expected");
    };
    let fact = |label: &str| facts.iter().find(|(l, _)| l == label).unwrap().1.as_str();
    assert_eq!(fact("Part"), "models/cube.stl");
    assert_eq!(
        fact("Stock"),
        "cylinder, diameter 20 mm, length 14 mm, from X -7 mm"
    );
    assert_eq!(fact("Stock material"), "oak");
    assert!(fact("Work offset").starts_with("G54, work zero at model X 0 mm"));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_12_times_come_from_the_toolpath_and_are_labelled_as_programmed_time() {
    let (dir, mut app) = sheet_project();
    app.set_post(None);
    let result = app.regenerate().unwrap();
    let sheet = app.setup_sheet(Some(&result.toolpath));
    let operations = cell_rows(&sheet, "Operations");
    for row in &operations {
        assert!(row[8].contains(':'), "a time in m:ss: {row:?}");
    }
    let html = sheet.to_html();
    assert!(html.contains("Programmed time"));
    assert!(html.contains("not the simulated estimate"));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_12_req_nfr_07_names_are_escaped_in_the_html() {
    let (dir, mut app) = sheet_project();
    let mut op = app.project().operations()[0].clone();
    op.name = "<script>alert(1)</script> & \"q\"".into();
    app.execute(Command::Replace(Item::Operation(op))).unwrap();
    let html = app.setup_sheet(None).to_html();
    assert!(!html.contains("<script"), "{html}");
    assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt; &amp; &quot;q&quot;"));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_12_the_html_opens_offline() {
    let (dir, app) = sheet_project();
    let html = app.setup_sheet(None).to_html();
    for external in [
        "http://", "https://", "<script", "<link", "src=", "@import", "url(",
    ] {
        assert!(!html.contains(external), "{external}");
    }
    assert!(html.starts_with("<!DOCTYPE html>\n<html lang=\"en\">"));
    assert!(html.contains("<meta charset=\"utf-8\">"));
    assert!(html.contains("@media print"));
    assert!(
        html.contains("must be verified by the user"),
        "the NFR-12 notice"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

/// Checks the file's structure: header, one cross-reference entry per object
/// at the offset the entry gives, a trailer pointing at the table.
fn check_pdf(bytes: &[u8]) -> String {
    let text = String::from_utf8(bytes.to_vec()).expect("the PDF is ASCII");
    assert!(text.starts_with("%PDF-1.4\n") && text.ends_with("%%EOF\n"));
    let start: usize = text
        .rsplit("startxref\n")
        .next()
        .unwrap()
        .lines()
        .next()
        .unwrap()
        .parse()
        .unwrap();
    assert!(text[start..].starts_with("xref\n0 "));
    let table: Vec<&str> = text[start..].lines().skip(1).collect();
    let count: usize = table[0].strip_prefix("0 ").unwrap().parse().unwrap();
    for (i, entry) in table[2..=count].iter().enumerate() {
        let offset: usize = entry[..10].parse().unwrap();
        assert!(
            text[offset..].starts_with(&format!("{} 0 obj\n", i + 1)),
            "object {} is at {offset}",
            i + 1
        );
    }
    text
}

#[test]
fn req_gc_12_the_pdf_is_well_formed_and_holds_the_sheets_text() {
    let (dir, app) = sheet_project();
    let sheet = app.setup_sheet(None);
    let text = check_pdf(&sheet.to_pdf());
    for expected in [
        "Setup sheet: sheet",
        "T1    4 mm ball",
        "T2    6 mm ball",
        "Finish B",
        "Page 1 of ",
    ] {
        assert!(text.contains(expected), "{expected}");
    }
    assert!(
        !text.contains("spare"),
        "an unused tool stays off the sheet"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_12_a_long_sheet_runs_onto_more_pages_and_keeps_every_row() {
    let (dir, mut app) = sheet_project();
    for i in 0..120 {
        let tool = app.project().tools()[0].id;
        let setup = app.project().setups()[0].id;
        add(
            &mut app,
            Item::Operation(operation(&format!("Extra pass {i:03}"), setup, tool, 60.0)),
        );
    }
    let text = check_pdf(&app.setup_sheet(None).to_pdf());
    assert!(
        ["/Count 3", "/Count 4", "/Count 5"]
            .iter()
            .any(|c| text.contains(c)),
        "several pages"
    );
    // A name too wide for its column wraps (the table fits 100 characters).
    for i in [0, 59, 119] {
        assert!(text.contains(&format!("{i:03}")), "{i}");
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_12_non_ascii_is_written_as_a_question_mark_in_the_pdf_and_kept_in_the_html() {
    let (dir, mut app) = sheet_project();
    let mut op = app.project().operations()[0].clone();
    op.name = "Fräs (a\\b)".into();
    app.execute(Command::Replace(Item::Operation(op))).unwrap();
    let sheet = app.setup_sheet(None);
    assert!(sheet.to_html().contains("Fräs (a\\b)"));
    let text = check_pdf(&sheet.to_pdf());
    assert!(
        text.contains("Fr?s \\(a\\\\b\\)"),
        "escaped for a PDF string"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_12_req_gc_07_the_sheet_is_the_same_bytes_each_time() {
    let (dir, mut app) = sheet_project();
    app.save().unwrap();
    app.set_clock(at(1_792_575_005));
    app.set_post(None);
    let result = app.regenerate().unwrap();
    let first = app.setup_sheet(Some(&result.toolpath));
    for threads in [1, 4] {
        let mut again = Application::open(&dir, Threads::Exactly(threads)).unwrap();
        again.set_clock(at(1_792_575_005));
        again.set_post(None);
        let regenerated = again.regenerate().unwrap();
        let sheet = again.setup_sheet(Some(&regenerated.toolpath));
        assert_eq!(sheet.to_html(), first.to_html(), "{threads} threads");
        assert_eq!(sheet.to_pdf(), first.to_pdf(), "{threads} threads");
    }
    std::fs::remove_dir_all(dir).unwrap();
}

/// Setup 1 of the sheet project held 1 mm at the start of the stock (X -7 to
/// -6), with a head tab placed automatically between the held region and
/// the part (the cube spans X 0 to 10: it is centred on the axis in Y and Z
/// only) and a frustum tab placed by hand.
fn held_sheet_project() -> (PathBuf, Application) {
    let (dir, mut app) = sheet_project();
    let mut setup = app.project().setups()[0].clone();
    setup.workholding = Workholding {
        start: Some(HeldRegion {
            length: Millimetres(1.0),
            diameter: None,
        }),
        end: None,
        tabs: vec![
            Tab {
                x_start: Millimetres(0.0),
                length: Millimetres(0.5),
                shape: TabShape::Cylinder {
                    diameter: Millimetres(6.0),
                },
                layout: TabLayout::OnAxis,
                auto: Some(TabEnd::Head),
            },
            Tab {
                x_start: Millimetres(5.0),
                length: Millimetres(2.0),
                shape: TabShape::Frustum {
                    start_diameter: Millimetres(6.0),
                    end_diameter: Millimetres(10.0),
                },
                layout: TabLayout::OnAxis,
                auto: None,
            },
        ],
    };
    app.execute(Command::Replace(Item::Setup(setup))).unwrap();
    (dir, app)
}

fn facts(sheet: &openchuck_app::SetupSheet, heading: &str) -> Vec<(String, String)> {
    let section = sheet
        .sections
        .iter()
        .find(|s| s.heading == heading)
        .unwrap_or_else(|| panic!("no section {heading}"));
    match &section.body {
        openchuck_app::SheetBody::Facts(facts) => facts.clone(),
        other => panic!("{heading} is not facts: {other:?}"),
    }
}

fn fact(facts: &[(String, String)], label: &str) -> String {
    facts
        .iter()
        .find(|(l, _)| l == label)
        .map(|(_, v)| v.clone())
        .unwrap_or_else(|| panic!("no fact {label} in {facts:?}"))
}

#[test]
fn req_gc_12_req_set_09_req_op_21_the_sheet_shows_each_setups_held_regions_and_tabs() {
    let (dir, app) = held_sheet_project();
    let sheet = app.setup_sheet(None);
    let first = facts(&sheet, "Setup: Setup 1");
    assert_eq!(
        fact(&first, "Held at the start"),
        "1 mm of the stock, X -7 to -6 mm, at the stock's own section"
    );
    assert_eq!(fact(&first, "Held at the end"), "none");
    assert_eq!(fact(&first, "Tabs"), "2, listed under Tabs: Setup 1");
    // The automatic head tab is where the part, the stock and the held
    // region put it (decision 0202), not where it was last stored.
    assert_eq!(
        cell_rows(&sheet, "Tabs: Setup 1"),
        [
            [
                "1",
                "-6",
                "0",
                "cylinder",
                "6",
                "on the axis",
                "automatically, at the head of the part"
            ],
            [
                "2",
                "5",
                "7",
                "frustum",
                "6 to 10",
                "on the axis",
                "by hand"
            ],
        ]
    );
    // The setup with none says so, and has no tab table.
    let second = facts(&sheet, "Setup: Flip / side 2");
    assert_eq!(fact(&second, "Held at the start"), "none");
    assert_eq!(fact(&second, "Held at the end"), "none");
    assert_eq!(fact(&second, "Tabs"), "none");
    assert!(
        !sheet
            .sections
            .iter()
            .any(|s| s.heading == "Tabs: Flip / side 2")
    );
    // Both formats carry it (decision 0028 item 4: one document, two writers).
    let html = sheet.to_html();
    assert!(html.contains("Held at the start"), "{html}");
    assert!(html.contains("Tabs: Setup 1"), "{html}");
    let pdf = check_pdf(&sheet.to_pdf());
    assert!(pdf.contains("Held at the start"), "{pdf}");
    assert!(pdf.contains("Tabs: Setup 1"), "{pdf}");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_12_req_set_09_a_holding_diameter_is_named_on_the_sheet() {
    let (dir, mut app) = sheet_project();
    let mut setup = app.project().setups()[0].clone();
    setup.workholding.end = Some(HeldRegion {
        length: Millimetres(2.0),
        diameter: Some(Millimetres(12.0)),
    });
    app.execute(Command::Replace(Item::Setup(setup))).unwrap();
    let sheet = app.setup_sheet(None);
    assert_eq!(
        fact(&facts(&sheet, "Setup: Setup 1"), "Held at the end"),
        "2 mm of the stock, X 5 to 7 mm, at diameter 12 mm"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_12_req_gc_07_req_set_09_the_sheet_with_held_regions_and_tabs_is_the_same_bytes_each_time()
{
    let (dir, app) = held_sheet_project();
    app.save().unwrap();
    let first = app.setup_sheet(None);
    for threads in [1, 4] {
        let again = Application::open(&dir, Threads::Exactly(threads)).unwrap();
        let sheet = again.setup_sheet(None);
        assert_eq!(sheet.sections, first.sections, "{threads} threads");
    }
    std::fs::remove_dir_all(dir).unwrap();
}

fn sheet_golden(name: &str, text: &str) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/golden/setup_sheet")
        .join(name);
    openchuck_testkit::golden::check(&path, text).unwrap_or_else(|m| panic!("{m}"));
}

#[test]
fn req_gc_12_req_nfr_08_the_sheet_content_matches_its_snapshot() {
    let (dir, mut app) = sheet_project();
    app.set_post(None);
    let result = app.regenerate().unwrap();
    // The block is fixed text here: a real one carries the project's random
    // item IDs in its fingerprint and the files' modification time.
    let sheet = SetupSheet::new(app.project(), app.strategies(), Some(&result.toolpath))
        .with_traceability(&Traceability {
            generated: "2026-10-21T09:30:05Z".into(),
            saved: "2026-10-21T09:12:00Z".into(),
            version: "0.0.0".into(),
            fingerprint: format!("blake3:{}", "ab".repeat(32)),
            version_control: Traceability::NOT_TRACKED.into(),
        });
    sheet_golden("small_project.html", &sheet.to_html());
    let pdf = String::from_utf8(sheet.to_pdf()).unwrap();
    sheet_golden("small_project.pdf", &pdf);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_12_req_set_09_req_nfr_08_a_sheet_with_held_regions_and_tabs_matches_its_snapshot() {
    let (dir, app) = held_sheet_project();
    // No toolpath and fixed traceability, as in the snapshot above.
    let sheet = SetupSheet::with_parts(app.project(), app.strategies(), None, &|setup| {
        let model = setup.part?;
        let extent = app.inspect_model(model).ok()?.extent?;
        Some(extent.in_setup(&setup.frame))
    })
    .with_traceability(&Traceability {
        generated: "2026-10-22T09:30:05Z".into(),
        saved: "2026-10-22T09:12:00Z".into(),
        version: "0.0.0".into(),
        fingerprint: format!("blake3:{}", "cd".repeat(32)),
        version_control: Traceability::NOT_TRACKED.into(),
    });
    sheet_golden("held_project.html", &sheet.to_html());
    let pdf = String::from_utf8(sheet.to_pdf()).unwrap();
    sheet_golden("held_project.pdf", &pdf);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_12_req_ui_06_a_disabled_operation_is_off_the_sheet_and_out_of_the_files() {
    let (dir, mut app) = sheet_project();
    let off = app.project().operations()[1].id; // the only operation using T2
    app.execute(Command::SetOperationEnabled {
        operation: off,
        enabled: false,
    })
    .unwrap();
    let sheet = app.setup_sheet(None);
    let tools = cell_rows(&sheet, "Tools");
    assert_eq!(tools.len(), 1, "T2 is no longer used: {tools:?}");
    let operations = cell_rows(&sheet, "Operations");
    assert_eq!(operations.len(), 3);
    assert!(operations.iter().all(|r| r[1] != "Finish B"));
    let result = post(&mut app, OutputGrouping::Operation);
    assert!(names(&result).iter().all(|n| !n.contains("finish-b")));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_12_the_programmed_total_counts_enabled_operations_only() {
    let (dir, mut app) = sheet_project();
    app.set_post(None);
    let result = app.regenerate().unwrap();
    let total = |app: &Application| {
        let sheet = app.setup_sheet(Some(&result.toolpath));
        let section = sheet.sections.iter().find(|s| s.heading == "Job").unwrap();
        let openchuck_app::SheetBody::Facts(facts) = &section.body else {
            panic!("facts expected");
        };
        facts
            .iter()
            .find(|(l, _)| l == "Programmed time")
            .unwrap()
            .1
            .clone()
    };
    let before = total(&app);
    let off = app.project().operations()[1].id;
    app.execute(Command::SetOperationEnabled {
        operation: off,
        enabled: false,
    })
    .unwrap();
    assert_ne!(total(&app), before, "the old toolpath still holds Finish B");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_12_a_long_title_and_heading_wrap_in_the_pdf_and_lose_nothing() {
    let (dir, mut app) = sheet_project();
    let mut setup = app.project().setups()[0].clone();
    setup.name =
        "A very long setup name that would not fit on one line of the sheet and ends here: ZEBRA"
            .into();
    app.execute(Command::Replace(Item::Setup(setup))).unwrap();
    let text = check_pdf(&app.setup_sheet(None).to_pdf());
    assert!(text.contains("ZEBRA"), "the end of the heading is kept");
    std::fs::remove_dir_all(dir).unwrap();
}

/// Unescapes a PDF literal string (`\\`, `\(`, `\)`) so its length is the
/// number of characters printed.
fn pdf_unescape(raw: &str) -> String {
    let mut out = String::new();
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            out.extend(chars.next());
        } else {
            out.push(c);
        }
    }
    out
}

/// Legible print (GC-12), the part a test can decide: on the PDF every line
/// of text, in its fixed-width font, ends inside the page's right margin, and
/// the text sits inside the page; on the HTML the print rules set A4 paper
/// with a margin, black text on white and wrapping cells, so a long name
/// cannot run off the paper. How it looks on a real printer stays a manual
/// check (work breakdown, slice 19 review checklist).
#[test]
fn req_gc_12_the_pdf_text_fits_inside_the_page_and_the_html_has_print_rules() {
    let (dir, mut app) = sheet_project();
    let mut setup = app.project().setups()[0].clone();
    setup.name = "A very long setup name ".repeat(12);
    app.execute(Command::Replace(Item::Setup(setup))).unwrap();
    let sheet = app.setup_sheet(None);
    let text = check_pdf(&sheet.to_pdf());
    let width: f64 = text
        .split("/MediaBox [0 0 ")
        .nth(1)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .parse()
        .unwrap();
    let mut lines = 0;
    for line in text.lines().filter(|l| l.starts_with("BT /F")) {
        let size: f64 = line
            .split(" Tf")
            .next()
            .unwrap()
            .rsplit(' ')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        let x: f64 = line
            .split(" Tf ")
            .nth(1)
            .unwrap()
            .split(' ')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        let open = line.find('(').unwrap();
        let close = line.rfind(") Tj").unwrap();
        let chars = pdf_unescape(&line[open + 1..close]).chars().count() as f64;
        // Courier advances 0.6 em per character.
        let right = x + chars * 0.6 * size;
        assert!(x >= 36.0, "left margin: {line}");
        assert!(
            right <= width - 36.0,
            "{right} past the right margin: {line}"
        );
        lines += 1;
    }
    assert!(lines > 20, "the text lines were found");
    let html = sheet.to_html();
    for rule in [
        "@page { size: A4; margin: 15mm; }",
        "@media print { main { max-width: none; padding: 0; } body { font-size: 11px; } }",
        "background: #fff; color: #111",
        "overflow-wrap: anywhere",
        "section { break-inside: avoid; }",
        "table { border-collapse: collapse; width: 100%; }",
    ] {
        assert!(html.contains(rule), "{rule}");
    }
    std::fs::remove_dir_all(dir).unwrap();
}

// Traceability block (GC-12, decision 0110, slice 19 item 6).

fn trace_facts(sheet: &openchuck_app::SetupSheet) -> BTreeMap<String, String> {
    let section = sheet
        .sections
        .iter()
        .find(|s| s.heading == "Traceability")
        .expect("the sheet has a Traceability section");
    match &section.body {
        openchuck_app::SheetBody::Facts(facts) => facts.iter().cloned().collect(),
        other => panic!("Traceability is not facts: {other:?}"),
    }
}

fn at(seconds: u64) -> Box<dyn openchuck_app::Clock> {
    Box::new(openchuck_app::FixedClock(
        std::time::UNIX_EPOCH + std::time::Duration::from_secs(seconds),
    ))
}

#[test]
fn req_gc_12_the_sheet_ends_with_a_traceability_block_of_five_labelled_lines() {
    let (dir, mut app) = sheet_project();
    app.set_clock(at(1_792_575_005));
    app.save().unwrap();
    let sheet = app.setup_sheet(None);
    assert_eq!(sheet.sections.last().unwrap().heading, "Traceability");
    let facts = trace_facts(&sheet);
    let labels: Vec<&str> = facts.keys().map(String::as_str).collect();
    assert_eq!(
        labels,
        [
            "Generated",
            "Project fingerprint",
            "Project last saved",
            "Software version",
            "Version control"
        ]
    );
    assert_eq!(facts["Generated"], "2026-10-21T09:30:05Z");
    assert_eq!(facts["Software version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(facts["Version control"], "not tracked");
    for text in [sheet.to_html(), check_pdf(&sheet.to_pdf())] {
        for line in ["Traceability", "2026-10-21T09:30:05Z", "not tracked"] {
            assert!(text.contains(line), "{line}");
        }
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_12_req_nfr_08_the_traceability_block_is_the_same_every_run_with_a_fixed_clock() {
    let (dir, mut app) = sheet_project();
    app.set_clock(at(1_792_575_005));
    app.save().unwrap();
    let first = app.setup_sheet(None);
    let second = app.setup_sheet(None);
    assert_eq!(first, second);
    assert_eq!(first.to_html(), second.to_html());
    assert_eq!(first.to_pdf(), second.to_pdf());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_12_the_last_saved_time_is_the_project_files_modification_time() {
    let (dir, mut app) = sheet_project();
    app.set_clock(at(1_792_575_005));
    app.save().unwrap();
    let newest = std::fs::read_dir(&dir)
        .unwrap()
        .chain(std::fs::read_dir(dir.join("operations")).unwrap())
        .map(|e| e.unwrap())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".toml"))
        .map(|e| e.metadata().unwrap().modified().unwrap())
        .max()
        .unwrap();
    let want = openchuck_app::iso8601_utc(newest);
    assert_eq!(
        trace_facts(&app.setup_sheet(None))["Project last saved"],
        want
    );
    // Nothing about the time is stored in the project (PRJ-01).
    let project_file = std::fs::read_to_string(dir.join("project.toml")).unwrap();
    assert!(!project_file.contains(&want) && !project_file.contains("fingerprint"));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_12_the_fingerprint_follows_the_saved_files() {
    let (dir, mut app) = sheet_project();
    app.save().unwrap();
    let saved = trace_facts(&app.setup_sheet(None))["Project fingerprint"].clone();
    assert!(saved.starts_with("blake3:") && saved.len() == 71, "{saved}");
    assert_eq!(
        trace_facts(&app.setup_sheet(None))["Project fingerprint"],
        saved
    );
    // An edit that is not saved leaves the fingerprint of the saved files and
    // says the project has moved on.
    let mut setup = app.project().setups()[0].clone();
    setup.name = "Renamed".into();
    app.execute(Command::Replace(Item::Setup(setup))).unwrap();
    let edited = trace_facts(&app.setup_sheet(None));
    assert_eq!(edited["Project fingerprint"], saved);
    assert!(edited["Project last saved"].ends_with("(changes made since are not saved)"));
    // Saving it gives the new files' fingerprint.
    app.save().unwrap();
    let after = trace_facts(&app.setup_sheet(None));
    assert_ne!(after["Project fingerprint"], saved);
    assert!(!after["Project last saved"].contains("not saved"));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_12_a_project_with_no_saved_file_reads_not_saved_yet() {
    let (dir, app) = sheet_project();
    std::fs::remove_file(dir.join("project.toml")).unwrap();
    let facts = trace_facts(&app.setup_sheet(None));
    assert_eq!(facts["Project last saved"], "not saved yet");
    assert_eq!(facts["Project fingerprint"], "not saved yet");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_12_req_gc_07_the_traceability_block_does_not_change_the_g_code() {
    let (dir, mut app) = sheet_project();
    app.set_post(None);
    let before = app.regenerate().unwrap();
    app.set_clock(at(1_792_575_005));
    app.save().unwrap();
    let _ = app.setup_sheet(Some(&before.toolpath));
    let after = app.regenerate().unwrap();
    assert_eq!(before.gcode, after.gcode);
    std::fs::remove_dir_all(dir).unwrap();
}
