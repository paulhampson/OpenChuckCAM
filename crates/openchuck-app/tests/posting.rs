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
            parameters: BTreeMap::new(),
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
        parameters: BTreeMap::new(),
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
    // The rapid from one group's end to the next group's start turns A in
    // the G-code, and the IR counts that turn too; only the program's first
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
        assert!(
            (groups[1].start.a.0 - end).abs() > 1.0,
            "{end} {}",
            groups[1].start.a.0
        );
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
    assert_ne!(text, at_tool_speed, "half the speed, half the feed");
    std::fs::remove_dir_all(dir).unwrap();
}
