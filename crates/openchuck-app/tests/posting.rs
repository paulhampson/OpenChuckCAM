// SPDX-License-Identifier: AGPL-3.0-or-later
//! Posting a project's toolpath to G-code files and checking them (GC-02,
//! GC-05, GC-07, GC-08, GC-11, GC-14; decision record 0020 item 4).

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
use openchuck_app::setup::{MachineProfile, Millimetres, Stock};
use openchuck_app::tools::{ToolNumbering, tool_number_plan, tool_numbering_command};
use openchuck_app::{Application, Command, PostSelection, Threads};
use openchuck_types::tool::{ControllerMapping, CuttingData, Tool, ToolShape};
use openchuck_types::units::{FeedRate, SpindleSpeed};

fn ball() -> Tool {
    Tool {
        shape: ToolShape::BallEnd,
        diameter: Millimetres(4.0),
        flute_length: Millimetres(10.0),
        overall_length: Millimetres(40.0),
        shank_diameter: Millimetres(4.0),
        flute_count: 2,
        material: None,
        centre_cutting: None,
    }
}

/// A project with the corpus cube (10 mm) centred on the rotary axis, the
/// example machine, an oak cylinder of stock, a ball tool with oak cutting
/// data, and one operation using the test strategy.
fn project(name: &str) -> (PathBuf, Application, OperationId) {
    let dir = std::env::temp_dir().join(format!("openchuck-op-{name}-{}", std::process::id()));
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
    let setup_id = SetupId::new();
    let mut setup = Setup::new(setup_id, "Setup 1");
    setup.part = Some(model);
    setup.material = Some("oak".into());
    setup.stock = Some(Stock::Cylinder {
        diameter: Millimetres(20.0),
        length: Millimetres(14.0),
        x_start: Millimetres(-7.0),
    });
    let tool_id = ToolId::new();
    let tool = ball();
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
        id: tool_id,
        order: OrderKey::default(),
        name: "4 mm ball".into(),
        tool,
        controller: ControllerMapping {
            number: 1,
            length_offset: None,
            diameter_offset: None,
        },
        cutting_data,
    };
    let op = OperationId::new();
    for item in [
        Item::Setup(setup),
        Item::Tool(snapshot),
        Item::Operation(Operation {
            enabled: true,
            id: op,
            order: OrderKey::default(),
            name: "Finish".into(),
            setup: setup_id,
            strategy: "helical_finishing".into(),
            tool: Some(tool_id),
            selections: Vec::new(),
            boundary: None,
            avoid: Vec::new(),
            check: Vec::new(),
            restriction_options: Default::default(),
            spindle_speed: None,
            material: None,
            // Not about free ends (decision record 0344, OI-1396).
            parameters: BTreeMap::from([(
                "face_free_ends".to_owned(),
                ParameterValue::Flag(false),
            )]),
        }),
    ] {
        app.execute(Command::Add {
            item,
            placement: Placement::End,
        })
        .unwrap();
    }
    (dir, app, op)
}

fn with_post(app: &mut Application, post: &str) {
    app.set_post(Some(PostSelection::parse(post)));
}

#[test]
fn req_gc_11_a_project_posts_to_g_code_that_passes_gates_1_and_2() {
    let (dir, mut app, _) = project("post-z1");
    for (post, ext) in [("linuxcnc", "ngc"), ("makera-z1", "nc")] {
        with_post(&mut app, post);
        let result = app.regenerate().unwrap();
        assert_eq!(result.gcode.len(), 1, "one setup, one file");
        let (name, text) = &result.gcode[0];
        assert_eq!(name, &format!("01-setup-1.{ext}"));
        assert!(text.starts_with("(post-z1 - Setup 1)\n"), "{}", &text[..60]);
        assert!(text.contains("T1 M6"), "the snapshot's tool number");
        assert!(
            text.contains("S10000 M3"),
            "spindle speed from the oak cutting data"
        );
        if std::env::var("DUMP").is_ok() {
            std::fs::write(format!("/tmp/dump-{post}.txt"), text).unwrap();
        }
        let report = &result.reports[0];
        assert!(report.is_clean(), "{post}: {report:?}");
        assert!(report.moves > 100);
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_07_the_same_project_gives_the_same_bytes_at_any_thread_count() {
    let (dir, mut app, _) = project("post-det");
    with_post(&mut app, "makera-z1");
    let first = app.regenerate().unwrap().gcode;
    app.save().unwrap();
    let again = Application::open(&dir, Threads::Exactly(1)).unwrap();
    let mut again = again;
    with_post(&mut again, "makera-z1");
    assert_eq!(again.regenerate().unwrap().gcode, first);
    let mut wide = Application::open(&dir, Threads::Exactly(4)).unwrap();
    with_post(&mut wide, "makera-z1");
    assert_eq!(wide.regenerate().unwrap().gcode, first);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_14_posting_uses_the_projects_tool_numbers_and_refuses_missing_cutting_data() {
    let (dir, mut app, _) = project("post-tool");
    with_post(&mut app, "linuxcnc");
    // A different material on the setup: no cutting data, so no spindle speed.
    let setup = app.project().setups()[0].clone();
    let mut changed = setup;
    changed.material = Some("pine".into());
    // Replacing the setup would also lose the feeds, so the error comes from regeneration.
    app.execute(Command::Replace(Item::Setup(changed))).unwrap();
    let e = app.regenerate().unwrap_err().to_string();
    assert!(e.contains("pine"), "{e}");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_02_an_unknown_post_is_refused_naming_the_choices() {
    let (dir, mut app, _) = project("post-unknown");
    app.set_post(Some(PostSelection::Builtin("nope".into())));
    let e = app.regenerate().unwrap_err().to_string();
    assert!(
        e.contains("nope") && e.contains("makera-z1") && e.contains("linuxcnc"),
        "{e}"
    );
    app.set_post(Some(PostSelection::Folder(dir.join("missing-post"))));
    assert!(app.regenerate().is_err());
    std::fs::remove_dir_all(dir).unwrap();
}

/// Adds a second tool and a second operation that uses it, after the first.
fn two_tools(app: &mut Application, first_number: u32, second_number: u32) -> (ToolId, ToolId) {
    let first = app.project().tools()[0].clone();
    let mut changed = first.clone();
    changed.controller.number = first_number;
    app.execute(Command::Replace(Item::Tool(changed))).unwrap();
    let second_id = ToolId::new();
    let mut second = first.clone();
    second.id = second_id;
    second.name = "6 mm ball".into();
    second.controller.number = second_number;
    app.execute(Command::Add {
        item: Item::Tool(second),
        placement: Placement::End,
    })
    .unwrap();
    let setup = app.project().setups()[0].id;
    let op = Operation {
        enabled: true,
        id: OperationId::new(),
        order: OrderKey::default(),
        name: "Second".into(),
        setup,
        strategy: "helical_finishing".into(),
        tool: Some(second_id),
        selections: Vec::new(),
        boundary: None,
        avoid: Vec::new(),
        check: Vec::new(),
        restriction_options: Default::default(),
        spindle_speed: None,
        material: None,
        // Not about free ends (decision record 0344, OI-1396).
        parameters: BTreeMap::from([("face_free_ends".to_owned(), ParameterValue::Flag(false))]),
    };
    app.execute(Command::Add {
        item: Item::Operation(op),
        placement: Placement::End,
    })
    .unwrap();
    (first.id, second_id)
}

fn numbers(app: &Application) -> Vec<(String, u32)> {
    tool_number_plan(app.project())
        .into_iter()
        .map(|r| (r.name, r.number))
        .collect()
}

#[test]
fn req_gc_14_the_plan_lists_the_tools_in_order_of_first_use() {
    let (dir, mut app, _) = project("tn-plan");
    two_tools(&mut app, 5, 2);
    assert_eq!(
        numbers(&app),
        [("4 mm ball".to_string(), 5), ("6 mm ball".to_string(), 2)]
    );
    // A tool no operation uses is not in the program.
    let mut spare = app.project().tools()[0].clone();
    spare.id = ToolId::new();
    spare.name = "spare".into();
    spare.controller.number = 9;
    app.execute(Command::Add {
        item: Item::Tool(spare),
        placement: Placement::End,
    })
    .unwrap();
    assert_eq!(numbers(&app).len(), 2);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_14_confirming_keeps_the_numbers_and_auto_follows_operation_order_in_one_undo_step() {
    let (dir, mut app, _) = project("tn-auto");
    two_tools(&mut app, 5, 2);
    assert!(
        tool_numbering_command(app.project(), &ToolNumbering::Keep, None)
            .unwrap()
            .is_none()
    );
    let command = tool_numbering_command(
        app.project(),
        &ToolNumbering::InOrderOfUse { first: 1 },
        None,
    )
    .unwrap()
    .unwrap();
    app.execute(command).unwrap();
    assert_eq!(
        numbers(&app),
        [("4 mm ball".to_string(), 1), ("6 mm ball".to_string(), 2)]
    );
    assert!(app.undo().unwrap());
    assert_eq!(
        numbers(&app),
        [("4 mm ball".to_string(), 5), ("6 mm ball".to_string(), 2)]
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_14_a_number_used_twice_or_out_of_range_is_refused_and_a_swap_is_allowed() {
    let (dir, mut app, _) = project("tn-refuse");
    let (a, b) = two_tools(&mut app, 1, 2);
    let manual = |pairs: Vec<(ToolId, u32)>, max| {
        tool_numbering_command(app.project(), &ToolNumbering::Manual(pairs), max)
    };
    let e = manual(vec![(b, 1)], None).unwrap_err().to_string();
    assert!(
        e.contains("T1") && e.contains("4 mm ball") && e.contains("6 mm ball"),
        "{e}"
    );
    assert!(
        manual(vec![(a, 0)], None)
            .unwrap_err()
            .to_string()
            .contains("start at 1")
    );
    assert!(
        manual(vec![(a, 30)], Some(20))
            .unwrap_err()
            .to_string()
            .contains("largest tool number 20")
    );
    // A swap goes through, as one command.
    let swap = manual(vec![(a, 2), (b, 1)], None).unwrap().unwrap();
    app.execute(swap).unwrap();
    assert_eq!(
        numbers(&app),
        [("4 mm ball".to_string(), 2), ("6 mm ball".to_string(), 1)]
    );
    // An unused tool holding a number a used tool would take is refused.
    let mut spare = app.project().tools()[0].clone();
    spare.id = ToolId::new();
    spare.name = "spare".into();
    spare.controller.number = 7;
    app.execute(Command::Add {
        item: Item::Tool(spare),
        placement: Placement::End,
    })
    .unwrap();
    let e = tool_numbering_command(app.project(), &ToolNumbering::Manual(vec![(a, 7)]), None)
        .unwrap_err()
        .to_string();
    assert!(e.contains("T7") && e.contains("spare"), "{e}");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_14_req_gc_07_a_second_post_after_renumbering_is_byte_identical() {
    let (dir, mut app, _) = project("tn-twice");
    two_tools(&mut app, 5, 2);
    let command = tool_numbering_command(
        app.project(),
        &ToolNumbering::InOrderOfUse { first: 1 },
        None,
    )
    .unwrap()
    .unwrap();
    app.execute(command).unwrap();
    app.set_post(Some(PostSelection::Builtin("makera-z1".into())));
    let first = app.regenerate().unwrap().gcode;
    assert!(
        first[0].1.contains("T1 M6") && first[0].1.contains("T2 M6"),
        "{}",
        first[0].1
    );
    app.save().unwrap();
    let mut again = Application::open(&dir, Threads::Exactly(3)).unwrap();
    again.set_post(Some(PostSelection::Builtin("makera-z1".into())));
    assert_eq!(again.regenerate().unwrap().gcode, first);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_14_a_tool_number_outside_the_machines_range_is_refused_when_posting() {
    let (dir, mut app, _) = project("tn-range");
    // The Z1 allows 1 to 1000 (OI-62).
    assert_eq!(app.project().machine().unwrap().max_tool_number, 1000);
    let mut tool = app.project().tools()[0].clone();
    tool.controller.number = 1001;
    app.execute(Command::Replace(Item::Tool(tool))).unwrap();
    app.set_post(Some(PostSelection::Builtin("linuxcnc".into())));
    let e = app.regenerate().unwrap_err().to_string();
    assert!(e.contains("T1001") && e.contains("1 to 1000"), "{e}");
    let command = tool_numbering_command(app.project(), &ToolNumbering::Keep, Some(1000))
        .unwrap_err()
        .to_string();
    assert!(command.contains("largest tool number 1000"), "{command}");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_04_a_post_with_a_reversed_b_axis_still_passes_the_gate_checks() {
    // A user post that writes the rotary axis as B with the opposite sign (GC-04): the replay
    // follows the letter and turns the sign back before comparing with the toolpath.
    let (dir, mut app, _) = project("post-reversed");
    let post_dir = dir.join("my-post");
    std::fs::create_dir_all(&post_dir).unwrap();
    let source =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/openchuck-post/posts/linuxcnc");
    let manifest = std::fs::read_to_string(source.join("post.toml"))
        .unwrap()
        .replace("a_letter = \"A\"", "a_letter = \"B\"")
        .replace("a_direction = \"same\"", "a_direction = \"reversed\"");
    std::fs::write(post_dir.join("post.toml"), manifest).unwrap();
    std::fs::copy(source.join("main.post"), post_dir.join("main.post")).unwrap();
    app.set_post(Some(PostSelection::Folder(post_dir)));
    let result = app.regenerate().unwrap();
    let (_, text) = &result.gcode[0];
    assert!(
        text.contains(" B-"),
        "the rotary axis is written as B with the reversed sign"
    );
    assert!(!text.contains(" A"), "no A word is written");
    let report = &result.reports[0];
    assert!(
        report.is_clean(),
        "{:?} {:?}",
        report.findings.first(),
        report.comparison
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_11_a_toolpath_that_starts_away_from_a_zero_passes_gate_2() {
    // The first move takes A from where the controller is to the group's
    // start; the IR counts no A travel for it, and the comparison must not
    // either (decision record 0036: found on the lobed reference job).
    let (dir, mut app, op) = project("post-start-angle");
    app.execute(Command::SetParameter {
        operation: op,
        name: "start_angle".into(),
        value: Some(ParameterValue::Number(30.0)),
    })
    .unwrap();
    for post in ["linuxcnc", "makera-z1"] {
        with_post(&mut app, post);
        let result = app.regenerate().unwrap();
        assert_eq!(result.toolpath.groups[0].start.a.0, 30.0);
        let report = &result.reports[0];
        assert!(report.is_clean(), "{post}: {report:?}");
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn req_gc_11_two_operations_with_a_turn_between_them_pass_gate_2() {
    // The explicit connection at the start of the second group turns A in
    // both the IR and G-code; only the program's first
    // move, from wherever the controller is, counts on neither side.
    let (dir, mut app, op) = project("post-two-operations");
    let mut second = app.project().operation(op).unwrap().clone();
    second.id = OperationId::new();
    second.name = "Finish 2".into();
    second
        .parameters
        .insert("start_angle".into(), ParameterValue::Number(100.0));
    app.execute(Command::Add {
        item: Item::Operation(second),
        placement: Placement::End,
    })
    .unwrap();
    for post in ["linuxcnc", "makera-z1"] {
        with_post(&mut app, post);
        let result = app.regenerate().unwrap();
        let groups = &result.toolpath.groups;
        assert_eq!(groups.len(), 2);
        let end = groups[0].segments.last().unwrap().end.a.0;
        assert_eq!(groups[1].start.a.0, end);
        let connection = groups[1].segments.first().unwrap();
        assert_eq!(connection.kind, openchuck_types::ir::MoveKind::Rapid);
        let turn = (connection.end.a.0 - end).abs();
        assert!(
            turn > 1.0 && turn <= 180.0,
            "connection turns {turn} degrees"
        );
        assert!(connection.duration.0 > 0.0);
        let report = &result.reports[0];
        assert!(report.is_clean(), "{post}: {report:?}");
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

/// The project with its setup's material changed to `material` and the
/// operation given every value cutting data would have given, so only the
/// spindle speed can be missing.
fn without_cutting_data(
    app: &mut Application,
    material: &str,
    spindle: Option<f64>,
) -> OperationId {
    let mut setup = app.project().setups()[0].clone();
    setup.material = Some(material.into());
    app.execute(Command::Replace(Item::Setup(setup))).unwrap();
    let mut op = app.project().operations()[0].clone();
    for (name, value) in [("feed", 240.0), ("plunge_feed", 150.0), ("stepover", 0.8)] {
        op.parameters
            .insert(name.into(), ParameterValue::Number(value));
    }
    op.spindle_speed = spindle.map(SpindleSpeed);
    let id = op.id;
    app.execute(Command::Replace(Item::Operation(op))).unwrap();
    id
}

#[test]
fn req_tl_05_req_gc_14_an_operations_spindle_speed_posts_when_the_tool_has_no_cutting_data() {
    let (dir, mut app, _) = project("post-op-spindle");
    with_post(&mut app, "linuxcnc");
    without_cutting_data(&mut app, "Softwood", Some(9000.0));
    let result = app.regenerate().unwrap();
    let text = &result.gcode[0].1;
    assert!(text.contains("S9000 M3"), "the operation's own speed");
    assert!(!text.contains("S10000"), "not the tool's");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_nfr_12_a_missing_spindle_speed_is_refused_with_what_to_do_and_never_written_without_s() {
    let (dir, mut app, op) = project("post-no-spindle");
    with_post(&mut app, "linuxcnc");
    without_cutting_data(&mut app, "Softwood", None);
    let needs = app.operation_requirements(op);
    assert!(
        needs
            .iter()
            .any(|r| r.field == openchuck_app::requirements::RequirementField::SpindleSpeed),
        "{needs:?}"
    );
    let e = app.regenerate().unwrap_err().to_string();
    assert!(
        e.contains("Finish")
            && e.contains("no spindle speed")
            && e.contains("Enter a spindle speed for this operation")
            && e.contains("\"Softwood\"")
            && e.contains("\"4 mm ball\"")
            && e.contains("tool library"),
        "{e}"
    );
    assert!(!e.contains("names must match exactly"), "{e}");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_tl_05_a_material_name_differing_in_case_or_spacing_still_finds_the_cutting_data() {
    let (dir, mut app, op) = project("post-case");
    with_post(&mut app, "linuxcnc");
    let mut setup = app.project().setups()[0].clone();
    setup.material = Some("OAK".into());
    app.execute(Command::Replace(Item::Setup(setup))).unwrap();
    assert!(app.operation_requirements(op).is_empty());
    let result = app.regenerate().unwrap();
    assert!(result.gcode[0].1.contains("S10000 M3"));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_tl_05_the_cutting_feed_follows_the_operations_spindle_speed() {
    let (dir, mut app, _) = project("post-feed-follows");
    with_post(&mut app, "linuxcnc");
    let at_tool_speed = app.regenerate().unwrap().gcode[0].1.clone();
    let mut op = app.project().operations()[0].clone();
    op.spindle_speed = Some(SpindleSpeed(5000.0));
    app.execute(Command::Replace(Item::Operation(op))).unwrap();
    let text = app.regenerate().unwrap().gcode[0].1.clone();
    assert!(text.contains("S5000 M3"));
    // The cutting moves (those that move X) halve; the plunge feed does not.
    let cutting_feeds = |gcode: &str| -> Vec<f64> {
        gcode
            .lines()
            .filter(|l| l.starts_with('X'))
            .filter_map(|l| l.rsplit_once('F'))
            .filter_map(|(_, f)| f.trim().parse().ok())
            .collect()
    };
    let (before, after) = (cutting_feeds(&at_tool_speed), cutting_feeds(&text));
    assert!(!before.is_empty() && before.len() == after.len());
    // A feed the machine's limit holds down stays; the rest halve.
    let mut halved = 0;
    for (b, a) in before.iter().zip(&after) {
        assert!(
            *a <= *b + 0.06,
            "a slower spindle never feeds faster: {b} -> {a}"
        );
        if (a - b / 2.0).abs() <= 0.06 {
            halved += 1;
        }
    }
    assert!(halved > 0, "half the speed, half the feed");
    std::fs::remove_dir_all(dir).unwrap();
}

/// Gives the project's machine copy the spindle speed range `min` to `max`.
fn spindle_range(app: &mut Application, min: Option<f64>, max: Option<f64>) {
    let mut machine = app.project().machine().unwrap().clone();
    machine.min_spindle_rpm = min.map(SpindleSpeed);
    machine.max_spindle_rpm = max.map(SpindleSpeed);
    app.execute(Command::SetMachine(Some(Box::new(machine))))
        .unwrap();
}

/// A speed outside the machine's spindle speed range, from the operation or
/// from the tool's cutting data, blocks Post and export with what to do; a
/// speed inside it posts as before, and a profile with no range checks
/// nothing (decision 0231; OI-754; NFR-12).
#[test]
fn req_gc_14_req_nfr_12_a_spindle_speed_outside_the_machines_range_is_refused() {
    use openchuck_app::requirements::RequirementField;
    let (dir, mut app, op) = project("post-spindle-range");
    with_post(&mut app, "linuxcnc");
    let before = app.regenerate().unwrap().gcode[0].1.clone();
    // The tool's data gives 10000 rpm.
    spindle_range(&mut app, None, Some(8000.0));
    let needs = app.operation_requirements(op);
    assert!(
        needs
            .iter()
            .any(|r| r.field == RequirementField::SpindleSpeed),
        "{needs:?}"
    );
    let e = app.regenerate().unwrap_err().to_string();
    for part in [
        "\"Finish\"",
        "10000 rpm",
        "above",
        "highest spindle speed, 8000 rpm",
        "Lower the spindle speed",
    ] {
        assert!(e.contains(part), "{part}: {e}");
    }
    // The operation's own speed, inside the range, posts.
    let mut o = app.project().operation(op).unwrap().clone();
    o.spindle_speed = Some(SpindleSpeed(7000.0));
    app.execute(Command::Replace(Item::Operation(o.clone())))
        .unwrap();
    assert!(app.operation_requirements(op).is_empty());
    assert!(app.regenerate().unwrap().gcode[0].1.contains("S7000 M3"));
    // Below the lowest speed.
    spindle_range(&mut app, Some(7500.0), Some(12000.0));
    let e = app.regenerate().unwrap_err().to_string();
    assert!(
        e.contains("below") && e.contains("Raise the spindle speed"),
        "{e}"
    );
    // No range: as before, byte for byte.
    o.spindle_speed = None;
    app.execute(Command::Replace(Item::Operation(o))).unwrap();
    spindle_range(&mut app, None, None);
    assert_eq!(app.regenerate().unwrap().gcode[0].1, before);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_07_req_op_11_ramp_option_off_posts_identical_bytes() {
    let (dir, mut app, op) = project("ramp-off-identity");
    let mut operation = app.project().operation(op).unwrap().clone();
    operation
        .parameters
        .insert("entry".into(), ParameterValue::Text("ramp".into()));
    operation
        .parameters
        .insert("ramp_angle".into(), ParameterValue::Number(45.0));
    app.execute(Command::Replace(Item::Operation(operation)))
        .unwrap();
    for post in ["makera-z1", "linuxcnc"] {
        with_post(&mut app, post);
        app.execute(Command::SetParameter {
            operation: op,
            name: "ramp_back_and_forth".into(),
            value: None,
        })
        .unwrap();
        let absent = app.regenerate().unwrap();
        app.execute(Command::SetParameter {
            operation: op,
            name: "ramp_back_and_forth".into(),
            value: Some(ParameterValue::Flag(false)),
        })
        .unwrap();
        let off = app.regenerate().unwrap();
        assert_eq!(absent.toolpath, off.toolpath);
        assert_eq!(absent.gcode, off.gcode, "{post}");
        assert!(
            off.reports.iter().all(|r| r.is_clean()),
            "{:?}",
            off.reports
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_op_11_req_gc_09_req_gc_10_reversing_ramp_posts_from_application() {
    let (dir, mut app, op) = project("reversing-ramp-post");
    let mut operation = app.project().operation(op).unwrap().clone();
    operation
        .parameters
        .insert("entry".into(), ParameterValue::Text("ramp".into()));
    operation
        .parameters
        .insert("ramp_angle".into(), ParameterValue::Number(15.0));
    operation
        .parameters
        .insert("ramp_back_and_forth".into(), ParameterValue::Flag(true));
    app.execute(Command::Replace(Item::Operation(operation)))
        .unwrap();
    for post in ["makera-z1", "linuxcnc"] {
        with_post(&mut app, post);
        let run = app.regenerate().unwrap();
        assert!(
            run.toolpath
                .groups
                .iter()
                .flat_map(|g| &g.segments)
                .filter(|s| s.kind == openchuck_types::ir::MoveKind::LeadIn)
                .count()
                > 2
        );
        assert!(
            run.reports.iter().all(|r| r.is_clean()),
            "{:?}",
            run.reports
        );
    }
    app.save().unwrap();
    for count in [
        1,
        2,
        std::thread::available_parallelism().map_or(4, usize::from),
    ] {
        let mut fresh = Application::open(&dir, Threads::Exactly(count)).unwrap();
        for post in ["makera-z1", "linuxcnc"] {
            with_post(&mut app, post);
            with_post(&mut fresh, post);
            let expected = app.regenerate().unwrap();
            let actual = fresh.regenerate().unwrap();
            assert_eq!(expected.toolpath, actual.toolpath);
            assert_eq!(expected.gcode, actual.gcode);
        }
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_op_03_req_op_11_indexed_roughing_uses_shared_reversing_ramp() {
    let (dir, mut app, op) = project("roughing-reversing-ramp");
    let mut operation = app.project().operation(op).unwrap().clone();
    operation.strategy = "rotary_roughing".into();
    operation.parameters.remove("face_free_ends");
    operation
        .parameters
        .insert("form".into(), ParameterValue::Text("indexed".into()));
    operation
        .parameters
        .insert("index_angle".into(), ParameterValue::Number(180.0));
    operation
        .parameters
        .insert("stepover".into(), ParameterValue::Number(5.0));
    operation
        .parameters
        .insert("entry".into(), ParameterValue::Text("ramp".into()));
    operation
        .parameters
        .insert("ramp_angle".into(), ParameterValue::Number(15.0));
    operation
        .parameters
        .insert("ramp_back_and_forth".into(), ParameterValue::Flag(true));
    app.execute(Command::Replace(Item::Operation(operation)))
        .unwrap();
    with_post(&mut app, "linuxcnc");
    let run = app.regenerate().unwrap();
    assert!(
        run.toolpath
            .groups
            .iter()
            .flat_map(|g| &g.segments)
            .filter(|s| s.kind == openchuck_types::ir::MoveKind::LeadIn)
            .count()
            > 2
    );
    assert!(
        run.reports.iter().all(|r| r.is_clean()),
        "{:?}",
        run.reports
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_op_11_req_gc_04_reversing_ramp_requires_certified_post_precision() {
    let (dir, mut app, op) = project("ramp-post-precision");
    let mut operation = app.project().operation(op).unwrap().clone();
    operation
        .parameters
        .insert("entry".into(), ParameterValue::Text("ramp".into()));
    operation
        .parameters
        .insert("ramp_angle".into(), ParameterValue::Number(15.0));
    operation
        .parameters
        .insert("ramp_back_and_forth".into(), ParameterValue::Flag(true));
    app.execute(Command::Replace(Item::Operation(operation)))
        .unwrap();
    let original = Path::new(env!("CARGO_MANIFEST_DIR")).join("../openchuck-post/posts/linuxcnc");
    let custom = dir.join("custom-post");
    std::fs::create_dir_all(&custom).unwrap();
    std::fs::copy(original.join("main.post"), custom.join("main.post")).unwrap();
    let manifest = std::fs::read_to_string(original.join("post.toml")).unwrap();
    app.set_post(Some(PostSelection::Folder(custom.clone())));
    for setting in ["pos", "a"] {
        std::fs::write(
            custom.join("post.toml"),
            manifest.replace(&format!("{setting} = 3"), &format!("{setting} = 2")),
        )
        .unwrap();
        match app.regenerate() {
            Ok(_) => panic!("reversing-ramp output accepted uncertified {setting} precision"),
            Err(error) => assert!(
                error.to_string().contains("at least three decimal"),
                "{error}"
            ),
        }
    }
    app.execute(Command::SetParameter {
        operation: op,
        name: "ramp_back_and_forth".into(),
        value: Some(ParameterValue::Flag(false)),
    })
    .unwrap();
    assert!(
        app.regenerate().is_ok(),
        "the new precision gate applies only to the optional ramp"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_09_req_op_11_reversing_ramp_refuses_uncertified_float_work_coordinates() {
    let (dir, mut app, op) = project("ramp-float-work-offset");
    let mut operation = app.project().operation(op).unwrap().clone();
    operation
        .parameters
        .insert("entry".into(), ParameterValue::Text("ramp".into()));
    operation
        .parameters
        .insert("ramp_angle".into(), ParameterValue::Number(15.0));
    operation
        .parameters
        .insert("ramp_back_and_forth".into(), ParameterValue::Flag(true));
    app.execute(Command::Replace(Item::Operation(operation)))
        .unwrap();
    let mut setup = app.project().setups()[0].clone();
    setup.frame.work_offset.x = Millimetres(100_000_000.0);
    app.execute(Command::Replace(Item::Setup(setup))).unwrap();
    with_post(&mut app, "makera-z1");
    match app.regenerate() {
        Ok(_) => panic!(
            "a float controller cannot preserve the certified XYZ clearance at this work offset"
        ),
        Err(error) => assert!(error.to_string().contains("numeric precision"), "{error}"),
    }
    // Small WCS words do not rescue a machine with huge absolute MCS
    // coordinates: the float controller still stores those positions.
    let mut setup = app.project().setups()[0].clone();
    setup.frame.work_offset.x = Millimetres(0.0);
    app.execute(Command::Replace(Item::Setup(setup))).unwrap();
    let mut machine = app.project().machine().unwrap().clone();
    machine.rotary_origin.x += 100_000_000.0;
    machine.travel.min.x += 100_000_000.0;
    machine.travel.max.x += 100_000_000.0;
    app.execute(Command::SetMachine(Some(Box::new(machine))))
        .unwrap();
    match app.regenerate() {
        Ok(_) => panic!("large float machine coordinates also exceed the certified precision"),
        Err(error) => assert!(error.to_string().contains("numeric precision"), "{error}"),
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_op_11_req_ui_07_inactive_reversing_ramp_is_identified_and_changes_no_output() {
    let (dir, mut app, op) = project("inactive-ramp-option");
    with_post(&mut app, "linuxcnc");
    let expected = app.regenerate().unwrap();
    app.execute(Command::SetParameter {
        operation: op,
        name: "ramp_back_and_forth".into(),
        value: Some(ParameterValue::Flag(true)),
    })
    .unwrap();
    let form = app.parameter_form(op).unwrap();
    let field = form
        .fields
        .iter()
        .find(|f| f.name == "ramp_back_and_forth")
        .unwrap();
    assert!(field.label.contains("inactive"), "{}", field.label);
    let actual = app.regenerate().unwrap();
    assert_eq!(expected.toolpath, actual.toolpath);
    assert_eq!(expected.gcode, actual.gcode);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_09_req_op_11_float_counter_context_must_match_ramp_certificate() {
    let (dir, mut app, op) = project("ramp-counter-context");
    let mut operation = app.project().operation(op).unwrap().clone();
    operation
        .parameters
        .insert("entry".into(), ParameterValue::Text("ramp".into()));
    operation
        .parameters
        .insert("ramp_angle".into(), ParameterValue::Number(15.0));
    operation
        .parameters
        .insert("ramp_back_and_forth".into(), ParameterValue::Flag(true));
    app.execute(Command::Replace(Item::Operation(operation.clone())))
        .unwrap();
    operation.id = OperationId::new();
    operation.name = "Second operation".into();
    app.execute(Command::Add {
        item: Item::Operation(operation),
        placement: Placement::End,
    })
    .unwrap();
    with_post(&mut app, "makera-z1");
    assert!(
        app.regenerate()
            .unwrap()
            .reports
            .iter()
            .all(|r| r.is_clean())
    );
    let original = Path::new(env!("CARGO_MANIFEST_DIR")).join("../openchuck-post/posts/makera-z1");
    let custom = dir.join("no-counter-reset");
    std::fs::create_dir_all(&custom).unwrap();
    std::fs::copy(original.join("main.post"), custom.join("main.post")).unwrap();
    let manifest = std::fs::read_to_string(original.join("post.toml"))
        .unwrap()
        .replace("unwind = \"drop_turns\"", "unwind = \"rotate\"");
    std::fs::write(custom.join("post.toml"), manifest).unwrap();
    app.set_post(Some(PostSelection::Folder(custom)));
    match app.regenerate() {
        Ok(_) => panic!("large unrebased float counter exceeds the canonical entry certificate"),
        Err(error) => assert!(error.to_string().contains("numeric precision"), "{error}"),
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_tl_07_req_gc_07_req_ui_08_cleanup_preserves_output_and_current_results() {
    use openchuck_app::tool_library::{remove_unused_tools, unused_project_tools};
    let (dir, mut app, op) = project("cleanup-output");
    let mut unused = app.project().tools()[0].clone();
    unused.id = ToolId::new();
    unused.name = "Unused copy".into();
    unused.controller.number = 12;
    let unused_id = unused.id;
    app.execute(Command::Add {
        item: Item::Tool(unused),
        placement: Placement::End,
    })
    .unwrap();
    with_post(&mut app, "makera-z1");
    let run = app.regenerate().unwrap();
    assert!(app.export_checks(Some(&run), None, None).can_write());
    app.save().unwrap();
    assert!(!app.is_modified());
    let reviewed = unused_project_tools(app.project());
    app.execute(
        remove_unused_tools(app.project(), &reviewed, &[unused_id])
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert!(app.is_modified());
    assert!(app.export_checks(Some(&run), None, None).can_write());
    assert!(openchuck_app::run_is_current(app.project(), &run));
    assert!(openchuck_app::simulation_out_of_date(app.project(), &run).is_none());
    let after = app.regenerate().unwrap();
    assert_eq!(after.gcode, run.gcode);
    assert_eq!(after.reused, [op]);
    assert!(after.generated.is_empty());
    app.undo().unwrap();
    assert!(app.export_checks(Some(&run), None, None).can_write());
    assert_eq!(app.regenerate().unwrap().gcode, run.gcode);
    std::fs::remove_dir_all(dir).unwrap();
}

/// GC-17 reporting leaves motion alone and carries the effective controller
/// settings and cost into the same-run HTML/PDF setup sheets.
#[test]
fn req_gc_17_req_gc_12_report_preserves_output_and_explains_short_blocks() {
    use openchuck_types::machine::ShortBlockMode;
    let (dir, mut app, _) = project("short-block-report");
    with_post(&mut app, "makera-z1");
    let off = app.regenerate().unwrap();
    let mut machine = app.project().machine().unwrap().clone();
    machine.min_block_ms = 500.0;
    machine.block_window = 1;
    machine.short_block_mode = ShortBlockMode::Report;
    machine.provenance.insert(
        openchuck_types::machine::ProfileField::MinBlockMs,
        openchuck_types::machine::Provenance {
            state: openchuck_types::machine::Confirmation::Unconfirmed,
            source: "Fixture calibration, firmware test".into(),
        },
    );
    app.execute(Command::SetMachine(Some(Box::new(machine))))
        .unwrap();
    let reported = app.regenerate().unwrap();
    assert_eq!(reported.toolpath, off.toolpath);
    assert_eq!(reported.gcode, off.gcode);
    assert!(!reported.short_blocks.is_empty());
    assert!(
        reported
            .short_blocks
            .iter()
            .any(|r| r.affected_windows > 0 && r.added_seconds > 0.0)
    );
    assert!(
        reported
            .notes
            .iter()
            .any(|n| n.contains("short blocks") && n.contains("500"))
    );
    let checks = app.export_checks(Some(&reported), Some(&reported), None);
    assert!(
        checks
            .checks
            .iter()
            .any(|c| c.text.contains("short blocks"))
    );
    let sheet = app.setup_sheet_for_run(&reported);
    let html = sheet.to_html();
    let pdf = String::from_utf8(sheet.to_pdf()).unwrap();
    // Both the per-operation result and the effective settings use the
    // singular controller-window unit (CodeRabbit, GC-17/GC-12).
    for text in [&html, &pdf] {
        // PDF escapes parentheses in its text stream; the visible unit is shared.
        assert_eq!(text.matches("1 block").count(), 2);
        assert!(!text.contains("1 blocks"));
    }
    for text in [
        "Short blocks",
        "500",
        "machine profile",
        "Report",
        "Worst window",
        "Added time",
        "Fixture calibration, firmware test",
    ] {
        assert!(html.contains(text), "missing {text}: {html}");
        assert!(pdf.contains(text), "missing {text} in PDF");
    }
    std::fs::remove_dir_all(dir).unwrap();
}

/// Gate 2 must compare the slowed durations, retaining its normal timing
/// tolerance instead of treating an intentional slowdown as a mismatch.
#[test]
fn req_gc_17_req_gc_11_slow_down_replays_and_reports_adjusted_time() {
    use openchuck_types::machine::ShortBlockMode;
    let (dir, mut app, _) = project("short-block-slowdown");
    with_post(&mut app, "makera-z1");
    let off = app.regenerate().unwrap();
    let mut machine = app.project().machine().unwrap().clone();
    machine.min_block_ms = 500.0;
    machine.block_window = 1;
    machine.short_block_mode = ShortBlockMode::SlowDown;
    app.execute(Command::SetMachine(Some(Box::new(machine))))
        .unwrap();
    let slowed = app.regenerate().unwrap();
    assert_eq!(
        slowed.toolpath, off.toolpath,
        "the canonical geometry and strategy time stay intact"
    );
    assert_ne!(slowed.gcode, off.gcode);
    assert!(
        slowed.reports.iter().all(|r| r.is_clean()),
        "{:?}",
        slowed.reports
    );
    let adjusted = slowed.posted_toolpath.as_ref().unwrap();
    assert!(adjusted.total_duration().0 > off.toolpath.total_duration().0);
    let comparison = slowed.reports[0].comparison.as_ref().unwrap();
    assert!(
        (comparison.ir_cut_seconds - comparison.gcode_cut_seconds).abs()
            / comparison.ir_cut_seconds
            < 0.01
    );
    let planned = app
        .planned_files(
            &slowed,
            &PostSelection::parse("makera-z1"),
            app.output_grouping(),
        )
        .unwrap();
    assert!(planned[0].programmed_seconds > off.toolpath.total_duration().0);
    // Previewing a post which explicitly disables management must recompute
    // its time, rather than reuse the first post's slowdown cost.
    let original = Path::new(env!("CARGO_MANIFEST_DIR")).join("../openchuck-post/posts/makera-z1");
    let disabled = dir.join("disabled-preview-post");
    std::fs::create_dir_all(&disabled).unwrap();
    std::fs::copy(original.join("main.post"), disabled.join("main.post")).unwrap();
    let manifest = std::fs::read_to_string(original.join("post.toml")).unwrap();
    std::fs::write(
        disabled.join("post.toml"),
        manifest.replace("[options]", "[options]\nshort_block_mode = \"off\""),
    )
    .unwrap();
    let preview = app
        .planned_files(
            &slowed,
            &PostSelection::Folder(disabled),
            app.output_grouping(),
        )
        .unwrap();
    assert!((preview[0].programmed_seconds - off.toolpath.total_duration().0).abs() < 1e-6);
    assert!(
        app.setup_sheet_for_run(&slowed)
            .to_html()
            .contains("Slow down")
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_17_disabled_post_override_is_visible_in_same_run_sheets() {
    use openchuck_types::machine::ShortBlockMode;
    let (dir, mut app, _) = project("short-block-disabled");
    let original = Path::new(env!("CARGO_MANIFEST_DIR")).join("../openchuck-post/posts/makera-z1");
    let custom = dir.join("custom-post");
    std::fs::create_dir_all(&custom).unwrap();
    std::fs::copy(original.join("main.post"), custom.join("main.post")).unwrap();
    let manifest = std::fs::read_to_string(original.join("post.toml")).unwrap();
    let mut machine = app.project().machine().unwrap().clone();
    machine.min_block_ms = 500.0;
    machine.block_window = 32;
    machine.short_block_mode = ShortBlockMode::Report;
    app.execute(Command::SetMachine(Some(Box::new(machine))))
        .unwrap();
    app.set_post(Some(PostSelection::Folder(custom.clone())));
    for override_text in [
        "short_block_mode = \"off\"",
        "short_block_mode = \"slow_down\"\nmin_block_ms = 0",
    ] {
        std::fs::write(
            custom.join("post.toml"),
            manifest.replace("[options]", &format!("[options]\n{override_text}")),
        )
        .unwrap();
        let run = app.regenerate().unwrap();
        assert!(run.short_blocks.is_empty());
        let (settings, sources) = run.effective_short_blocks.unwrap();
        assert_eq!(sources.mode, openchuck_post::SettingSource::Post);
        assert!(settings.mode == ShortBlockMode::Off || settings.min_block_ms == 0.0);
        let sheet = app.setup_sheet_for_run(&run);
        for text in [
            "Short-block settings",
            "post override",
            "Off; no slowdown applied",
            "32 blocks",
        ] {
            assert!(sheet.to_html().contains(text), "{text}");
            assert!(
                String::from_utf8(sheet.to_pdf()).unwrap().contains(text),
                "{text}"
            );
        }
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_17_req_gc_11_slowdown_with_max_feed_keeps_the_timing_gate() {
    use openchuck_types::machine::ShortBlockMode;
    let (dir, mut app, _) = project("short-block-clamped-gate");
    let original = Path::new(env!("CARGO_MANIFEST_DIR")).join("../openchuck-post/posts/makera-z1");
    let custom = dir.join("custom-post");
    std::fs::create_dir_all(&custom).unwrap();
    let template = std::fs::read_to_string(original.join("main.post")).unwrap();
    std::fs::write(custom.join("main.post"), &template).unwrap();
    let manifest = std::fs::read_to_string(original.join("post.toml")).unwrap();
    std::fs::write(
        custom.join("post.toml"),
        manifest.replace("[options]", "[options]\nmax_feed = 10"),
    )
    .unwrap();
    let mut machine = app.project().machine().unwrap().clone();
    machine.min_block_ms = 500.0;
    machine.block_window = 1;
    machine.short_block_mode = ShortBlockMode::SlowDown;
    app.execute(Command::SetMachine(Some(Box::new(machine))))
        .unwrap();
    app.set_post(Some(PostSelection::Folder(custom.clone())));
    let correct = app.regenerate().unwrap();
    assert!(
        correct.notes.iter().any(|n| n.contains("maximum feed")),
        "clamping is exercised"
    );
    assert!(
        correct.reports.iter().all(|r| r.is_clean()),
        "{:?}",
        correct.reports
    );
    // A faulty user template doubles each feed. Geometry remains identical;
    // the independent timing check must catch it despite max-feed clamping.
    let faulty = template.replace("it.feed.value|feed", "(it.feed.value * 2)|feed");
    assert_ne!(faulty, template);
    std::fs::write(custom.join("main.post"), faulty).unwrap();
    let wrong = app.regenerate().unwrap();
    assert!(
        wrong.reports.iter().any(|r| r.comparison.is_err()),
        "{:?}",
        wrong.reports
    );
    std::fs::remove_dir_all(dir).unwrap();
}
