// SPDX-License-Identifier: AGPL-3.0-or-later
//! The export dialog's headless side: the checks before writing, the files a
//! grouping would write, the per-computer acknowledgement and writing files
//! (GC-05, GC-14, GC-15, GC-16; decision record 0071; specification section
//! 11.6).

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
use openchuck_app::{
    Acknowledgements, Application, CheckLevel, Command, OutputGrouping, PostSelection, Threads,
    write_gcode_files,
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

fn folder(label: &str) -> PathBuf {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let unique = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "openchuck-export-{label}-{unique}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn blockers(checks: &openchuck_app::ExportChecks) -> Vec<&str> {
    checks
        .checks
        .iter()
        .filter(|c| c.level == CheckLevel::Blocking)
        .map(|c| c.text.as_str())
        .collect()
}

fn warnings(checks: &openchuck_app::ExportChecks) -> Vec<&str> {
    checks
        .checks
        .iter()
        .filter(|c| c.level == CheckLevel::Warning)
        .map(|c| c.text.as_str())
        .collect()
}

#[test]
fn req_gc_15_writing_is_refused_before_anything_is_generated() {
    let (dir, app) = project("checks-none", false);
    let checks = app.export_checks(None, None, None);
    assert!(!checks.can_write());
    assert!(
        checks.first_blocker().unwrap().contains("generate"),
        "{checks:?}"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_15_writing_is_allowed_with_warnings_for_an_unsimulated_operation() {
    let (dir, mut app) = project("checks-fresh", false);
    let run = post(&mut app, OutputGrouping::Setup);
    let checks = app.export_checks(Some(&run), None, None);
    assert!(checks.can_write(), "{:?}", blockers(&checks));
    let w = warnings(&checks);
    for name in ["Finish A", "Finish B", "Touch up A"] {
        assert!(
            w.iter()
                .any(|t| t.contains(name) && t.contains("not been simulated")),
            "{name}: {w:?}"
        );
    }
    // Simulating clears those warnings.
    app.set_post(Some(PostSelection::Builtin("makera-z1".into())));
    let simulated = app.simulate().unwrap();
    let checks = app.export_checks(Some(&simulated), None, None);
    assert!(checks.can_write());
    assert!(
        !warnings(&checks)
            .iter()
            .any(|t| t.contains("not been simulated")),
        "{:?}",
        warnings(&checks)
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_15_req_nfr_12_a_path_made_with_a_warning_is_said_at_writing_and_does_not_block_it() {
    // Decision 0175 (issue #226): a path made with a warning (a swarf
    // overcut above the tolerance) is written with it, so the checks before
    // writing repeat the warning, naming the operation, and writing is still
    // allowed (the owner chose a warning, not an acknowledgement).
    let (dir, mut app) = project("checks-warned", false);
    let mut run = post(&mut app, OutputGrouping::Setup);
    let message = "Swarf on face 7 cuts into the finished surface by 0.0131 mm.";
    run.groups[0].warnings.push(openchuck_path::PathWarning {
        message: message.to_owned(),
        parameter: Some("tolerance".to_owned()),
    });
    let name = app
        .project()
        .operation(run.groups[0].operation)
        .unwrap()
        .name
        .clone();
    let checks = app.export_checks(Some(&run), None, None);
    assert!(checks.can_write(), "{:?}", blockers(&checks));
    let w = warnings(&checks);
    assert!(
        w.iter()
            .any(|t| t.contains(&format!("'{name}'")) && t.contains(message)),
        "{w:?}"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_15_an_edit_makes_that_operation_and_the_ones_after_it_stale() {
    let (dir, mut app) = project("checks-stale", false);
    let run = post(&mut app, OutputGrouping::Setup);
    let second = app.project().operations()[1].clone();
    let mut changed = second.clone();
    changed
        .parameters
        .insert("stepover".to_owned(), ParameterValue::Number(4.0));
    app.execute(Command::Replace(Item::Operation(changed)))
        .unwrap();
    let checks = app.export_checks(Some(&run), None, None);
    assert!(!checks.can_write());
    let b = blockers(&checks);
    assert_eq!(b.len(), 2, "{b:?}");
    assert!(
        b[0].contains("Finish B") && b[0].contains("out of date"),
        "{b:?}"
    );
    assert!(b[1].contains("Touch up A"), "{b:?}");
    // Undoing the edit makes the run current again.
    app.undo().unwrap();
    assert!(app.export_checks(Some(&run), None, None).can_write());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_15_a_failed_generation_blocks_and_says_why() {
    let (dir, app) = project("checks-error", false);
    let checks = app.export_checks(None, None, Some("operation 'Finish A': no stock."));
    let b = blockers(&checks);
    assert!(b.iter().any(|t| t.contains("no stock")), "{b:?}");
    assert!(!b.iter().any(|t| t.contains("Nothing has been generated")));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_15_disabled_operations_are_listed_as_not_posted_and_do_not_block() {
    let (dir, mut app) = project("checks-disabled", false);
    let third = app.project().operations()[2].id;
    app.execute(Command::SetOperationEnabled {
        operation: third,
        enabled: false,
    })
    .unwrap();
    let run = post(&mut app, OutputGrouping::Setup);
    let checks = app.export_checks(Some(&run), None, None);
    assert!(checks.can_write(), "{:?}", blockers(&checks));
    assert_eq!(checks.not_posted, vec!["Touch up A".to_owned()]);
    assert!(
        !warnings(&checks).iter().any(|t| t.contains("Touch up A")),
        "a disabled operation is not asked about"
    );
    let text = &run.gcode[0].1;
    assert!(!text.contains("Touch up A"), "and it is not written");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_15_unconfirmed_machine_values_give_a_warning() {
    let (dir, mut app) = project("checks-machine", false);
    let run = post(&mut app, OutputGrouping::Setup);
    let checks = app.export_checks(Some(&run), None, None);
    let unconfirmed = app.project().machine().unwrap().unconfirmed_fields();
    assert!(
        !unconfirmed.is_empty(),
        "the example profile confirms nothing"
    );
    assert!(
        warnings(&checks)
            .iter()
            .any(|t| t.contains("not confirmed on the machine")),
        "{:?}",
        warnings(&checks)
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_05_the_planned_files_are_the_files_that_are_written() {
    let (dir, mut app) = project("plan", false);
    for grouping in OutputGrouping::ALL {
        let run = post(&mut app, grouping);
        let plan = app
            .planned_files(
                &run.toolpath,
                &PostSelection::Builtin("makera-z1".into()),
                grouping,
            )
            .unwrap();
        let planned: Vec<&str> = plan.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(planned, names(&run), "{grouping:?}");
        assert!(plan.iter().all(|f| f.setup == "Setup 1"));
        assert!(plan.iter().all(|f| f.programmed_seconds > 0.0));
        let total: f64 = plan.iter().map(|f| f.programmed_seconds).sum();
        let sheet: f64 = run
            .toolpath
            .groups
            .iter()
            .flat_map(|g| g.segments.iter())
            .map(|s| s.duration.0)
            .sum();
        assert!((total - sheet).abs() < 1e-9, "{grouping:?}");
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_05_a_project_file_across_two_setups_is_refused_in_the_plan_too() {
    let (dir, mut app) = project("plan-two", true);
    let run = post(&mut app, OutputGrouping::Setup);
    let e = app
        .planned_files(
            &run.toolpath,
            &PostSelection::Builtin("makera-z1".into()),
            OutputGrouping::Project,
        )
        .unwrap_err()
        .to_string();
    assert!(e.contains("setup runs"), "{e}");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_14_numbers_chosen_in_the_dialog_are_saved_and_give_the_same_bytes_again() {
    use openchuck_app::tools::{ToolNumbering, tool_number_plan, tool_numbering_command};
    let (dir, mut app) = project("numbers", false);
    let before = tool_number_plan(app.project());
    assert_eq!(before.iter().map(|r| r.number).collect::<Vec<_>>(), [1, 2]);
    let ids: Vec<_> = before.iter().map(|r| r.tool).collect();
    // By hand: a number twice is refused and nothing changes.
    let twice = ToolNumbering::Manual(vec![(ids[0], 7), (ids[1], 7)]);
    assert!(tool_numbering_command(app.project(), &twice, Some(99)).is_err());
    // Above the machine's range is refused too.
    let high = ToolNumbering::Manual(vec![(ids[0], 5000)]);
    let max = app.project().machine().map(|m| m.max_tool_number);
    assert!(tool_numbering_command(app.project(), &high, max).is_err());
    // Chosen numbers apply as one undo step.
    let by_hand = ToolNumbering::Manual(vec![(ids[0], 7), (ids[1], 3)]);
    let command = tool_numbering_command(app.project(), &by_hand, max)
        .unwrap()
        .unwrap();
    app.execute(command).unwrap();
    let first = post(&mut app, OutputGrouping::Setup).gcode;
    assert!(first[0].1.contains("T7 M6") && first[0].1.contains("T3 M6"));
    app.save().unwrap();
    let mut again = Application::open(&dir, Threads::Exactly(1)).unwrap();
    again.set_post(Some(PostSelection::Builtin("makera-z1".into())));
    assert_eq!(again.regenerate().unwrap().gcode, first);
    // Renumbering only changes how the program is posted: the toolpaths stay current.
    let run = again.regenerate().unwrap();
    assert!(again.export_checks(Some(&run), None, None).can_write());
    // Renumbering in order of use gives T1 and T2 again, in one undo step.
    let auto = tool_numbering_command(
        again.project(),
        &ToolNumbering::InOrderOfUse { first: 1 },
        max,
    )
    .unwrap()
    .unwrap();
    again.execute(auto).unwrap();
    assert!(
        again.export_checks(Some(&run), None, None).can_write(),
        "a new tool number does not make an operation out of date"
    );
    assert_eq!(
        tool_number_plan(again.project())
            .iter()
            .map(|r| r.number)
            .collect::<Vec<_>>(),
        [1, 2]
    );
    again.undo().unwrap();
    assert_eq!(
        tool_number_plan(again.project())
            .iter()
            .map(|r| r.number)
            .collect::<Vec<_>>(),
        [7, 3]
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_16_the_acknowledgement_is_per_project_per_settings_folder_and_never_in_the_project() {
    let (dir, app) = project("ack", false);
    let settings = folder("ack-settings");
    let here = Acknowledgements::in_folder(&settings);
    let lookup = here.lookup(app.location());
    assert!(!lookup.acknowledged && lookup.problem.is_none());
    let project_files = |dir: &Path| -> BTreeMap<PathBuf, Vec<u8>> {
        let mut out = BTreeMap::new();
        let mut stack = vec![dir.to_path_buf()];
        while let Some(d) = stack.pop() {
            for entry in std::fs::read_dir(d).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    stack.push(path);
                } else {
                    out.insert(path.clone(), std::fs::read(&path).unwrap());
                }
            }
        }
        out
    };
    let before = project_files(&dir);
    here.acknowledge(app.location()).unwrap();
    assert!(here.lookup(app.location()).acknowledged);
    // Asked once per project: not asked again in that folder, after a restart.
    assert!(
        Acknowledgements::in_folder(&settings)
            .lookup(app.location())
            .acknowledged
    );
    // Asked again with a fresh settings folder (another computer).
    let fresh = Acknowledgements::in_folder(&folder("ack-fresh"));
    assert!(!fresh.lookup(app.location()).acknowledged);
    // Another project in the same settings folder is asked.
    let (other_dir, other) = project("ack-other", false);
    assert!(!here.lookup(other.location()).acknowledged);
    // The project's files are untouched.
    assert_eq!(project_files(&dir), before);
    let saved = std::fs::read_to_string(settings.join("settings.toml")).unwrap();
    assert!(
        saved.contains("[export]") && saved.contains("acknowledged"),
        "{saved}"
    );
    for d in [dir, other_dir, settings] {
        std::fs::remove_dir_all(d).unwrap();
    }
}

#[test]
fn req_gc_16_a_bad_settings_file_asks_again_with_a_message_and_is_kept_and_other_keys_survive() {
    let (dir, app) = project("ack-bad", false);
    let settings = folder("ack-bad-settings");
    std::fs::create_dir_all(&settings).unwrap();
    let ack = Acknowledgements::in_folder(&settings);
    std::fs::write(settings.join("settings.toml"), "this is = = not toml").unwrap();
    let lookup = ack.lookup(app.location());
    assert!(!lookup.acknowledged);
    assert!(lookup.problem.unwrap().contains("could not be read"));
    ack.acknowledge(app.location()).unwrap();
    assert!(ack.lookup(app.location()).acknowledged);
    assert_eq!(
        std::fs::read_to_string(settings.join("settings.toml.bad")).unwrap(),
        "this is = = not toml"
    );
    // A key another part of the application keeps is not lost.
    let text = std::fs::read_to_string(settings.join("settings.toml")).unwrap();
    std::fs::write(
        settings.join("settings.toml"),
        format!("scale = 1.5\n{text}"),
    )
    .unwrap();
    let (other_dir, other) = project("ack-bad-other", false);
    ack.acknowledge(other.location()).unwrap();
    let kept = std::fs::read_to_string(settings.join("settings.toml")).unwrap();
    assert!(kept.contains("scale = 1.5"), "{kept}");
    assert!(ack.lookup(app.location()).acknowledged && ack.lookup(other.location()).acknowledged);
    for d in [dir, other_dir, settings] {
        std::fs::remove_dir_all(d).unwrap();
    }
}

#[test]
fn req_gc_05_writing_puts_each_file_in_the_chosen_folder() {
    let (dir, mut app) = project("write", false);
    let run = post(&mut app, OutputGrouping::Operation);
    let out = folder("write-out").join("nested");
    let written = write_gcode_files(&out, &run.gcode).unwrap();
    assert_eq!(written.len(), 3);
    for ((name, text), path) in run.gcode.iter().zip(&written) {
        assert_eq!(path, &out.join(name));
        assert_eq!(&std::fs::read_to_string(path).unwrap(), text);
    }
    assert!(
        std::fs::read_dir(&out).unwrap().all(|e| !e
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".part"))
    );
    // A file that cannot be written is an error naming it, not a crash.
    let blocked = out.join("01-setup-1-finish-a.nc");
    std::fs::remove_file(&blocked).unwrap();
    std::fs::create_dir(&blocked).unwrap();
    assert!(write_gcode_files(&out, &run.gcode).is_err());
    std::fs::remove_dir_all(out.parent().unwrap()).unwrap();
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_04_the_post_summary_states_what_the_post_does() {
    let posts = openchuck_app::builtin_post_summaries();
    let ids: Vec<String> = posts
        .iter()
        .map(|p| match &p.selection {
            PostSelection::Builtin(id) => id.clone(),
            PostSelection::Folder(_) => unreachable!(),
        })
        .collect();
    assert_eq!(ids, ["makera-z1", "linuxcnc"]);
    let z1 = &posts[0];
    assert_eq!(z1.units, "millimetres");
    assert_eq!(z1.a_letter, "A");
    assert!(
        z1.a_wrap.contains("dropped without motion"),
        "{}",
        z1.a_wrap
    );
    assert!(z1.feed_mode.contains("G94") || z1.feed_mode.contains("G93"));
    assert!(
        openchuck_app::post_summary(&PostSelection::Folder("/nonexistent/post".into())).is_err()
    );
}

#[test]
fn req_gc_02_a_run_with_a_post_leaves_the_applications_own_post_and_grouping_alone() {
    let (dir, mut app) = project("with-post", false);
    app.set_output_grouping(OutputGrouping::Setup);
    assert!(app.post().is_none());
    let run = app
        .start_regenerate_with_post(
            PostSelection::Builtin("makera-z1".into()),
            OutputGrouping::Operation,
        )
        .wait()
        .unwrap();
    assert_eq!(run.gcode.len(), 3, "the run used its own grouping");
    assert!(app.post().is_none(), "the application's post is unchanged");
    assert_eq!(app.output_grouping(), OutputGrouping::Setup);
    assert!(
        app.regenerate().unwrap().gcode.is_empty(),
        "a later generation posts nothing unasked"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_12_the_setup_sheet_is_written_as_html_and_pdf_named_for_the_project() {
    let (dir, mut app) = project("Sheet Project", false);
    let run = post(&mut app, OutputGrouping::Setup);
    let out = folder("sheet-out");
    let written = app.write_setup_sheet(&out, Some(&run.toolpath)).unwrap();
    let names: Vec<_> = written
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        names,
        [
            "sheet-project-setup-sheet.html",
            "sheet-project-setup-sheet.pdf"
        ]
    );
    let html = std::fs::read_to_string(&written[0]).unwrap();
    assert!(html.contains("4 mm ball") && html.contains("Finish A"));
    assert!(std::fs::read(&written[1]).unwrap().starts_with(b"%PDF-1.4"));
    std::fs::remove_dir_all(out).unwrap();
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_nfr_15_export_warning_names_machine_values_in_plain_words() {
    let (dir, mut app) = project("checks-words", false);
    let run = post(&mut app, OutputGrouping::Setup);
    let checks = app.export_checks(Some(&run), None, None);
    let text = warnings(&checks).join(" ");
    assert!(text.contains("axis height"), "{text}");
    for field in ["rotary_origin", "rotary_axis", "a_sense", "max_feeds"] {
        assert!(
            !text.contains(field),
            "{field} is a file field name: {text}"
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_12_req_gc_07_a_rename_is_posted_without_regenerating_and_is_deterministic() {
    // Issue 209 (decision 0172): the operation name is a comment in the
    // G-code (GC-12) but not an input to the toolpath, so a rename makes the
    // next run reuse every path and post again with the new name. The same
    // project always gives the same bytes (GC-07, NFR-06).
    let (dir, mut app) = project("rename-post", false);
    let before = post(&mut app, OutputGrouping::Setup);
    assert!(before.gcode[0].1.contains("Finish B"));

    let id = app.project().operations()[1].id;
    let mut renamed = app.project().operation(id).unwrap().clone();
    assert_eq!(renamed.name, "Finish B");
    renamed.name = "Finish the barrel".into();
    app.execute(Command::Replace(Item::Operation(renamed)))
        .unwrap();

    // The export check is satisfied by the old run: the toolpath is current.
    let checks = app.export_checks(Some(&before), None, None);
    assert!(checks.can_write(), "{:?}", blockers(&checks));

    let after = app.regenerate().unwrap();
    assert!(after.generated.is_empty(), "no toolpath was regenerated");
    assert_eq!(after.reused.len(), before.groups.len());
    assert_eq!(after.toolpath, before.toolpath);
    let text = &after.gcode[0].1;
    assert!(text.contains("Finish the barrel") && !text.contains("Finish B"));
    assert_eq!(app.regenerate().unwrap().gcode, after.gcode);
    std::fs::remove_dir_all(dir).unwrap();
}
