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
        spindle_speed: None,
        material: None,
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
/// A clock that reads one second later each time anything asks it, across
/// every app of the test. A setup sheet reads it once, so two sheets are a
/// second apart, as when the real clock crosses a second between two builds
/// (OI-1001, issue #361). The test project and [`open`] install it, so a test
/// that compares two sheets without pinning the clock fails every run instead
/// of once in a while; such a test pins a `FixedClock` (GC-07).
#[derive(Debug)]
struct TickingClock;

static TICKS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1_792_575_000);

impl openchuck_app::Clock for TickingClock {
    fn now(&self) -> std::time::SystemTime {
        let seconds = TICKS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        std::time::UNIX_EPOCH + std::time::Duration::from_secs(seconds)
    }
}

fn ticking_clock() -> Box<dyn openchuck_app::Clock> {
    Box::new(TickingClock)
}

/// Opens the project at `dir` with the ticking clock, as [`project`] does.
fn open(dir: &Path, threads: usize) -> Application {
    let mut app = Application::open(dir, Threads::Exactly(threads)).unwrap();
    app.set_clock(ticking_clock());
    app
}

fn project(name: &str, second_setup: bool) -> (PathBuf, Application) {
    // Tests run in parallel: each gets a folder of its own, whatever the project is called.
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let unique = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("openchuck-out-{unique}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut app = Application::create(&dir, name, Threads::Exactly(2)).unwrap();
    app.set_clock(ticking_clock());
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
            .planned_files(&run, &PostSelection::Builtin("makera-z1".into()), grouping)
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
        // An operation file starts independently: no generated connection
        // from a previous file is included in its programmed time.
        let canonical = run.canonical_toolpath.total_duration().0;
        match grouping {
            OutputGrouping::Operation => assert!((total - canonical).abs() < 1e-9),
            OutputGrouping::Project | OutputGrouping::Setup => {
                assert!((total - sheet).abs() < 1e-9)
            }
            OutputGrouping::Tool => assert!(total >= canonical - 1e-9 && total <= sheet + 1e-9),
        }
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_05_a_project_file_across_two_setups_is_refused_in_the_plan_too() {
    let (dir, mut app) = project("plan-two", true);
    let run = post(&mut app, OutputGrouping::Setup);
    let e = app
        .planned_files(
            &run,
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
    let mut again = open(&dir, 1);
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
fn req_gc_12_req_gc_05_writing_the_g_code_writes_the_setup_sheet_from_the_same_run() {
    // Decision 0189: the sheet is written with the G-code, from the run the
    // G-code was posted from, so its times and settings match the program.
    let (dir, mut app) = project("Together", false);
    // The sheet written and the sheet rebuilt below are two builds: pin the
    // clock so the Generated second cannot differ between them (GC-07,
    // OI-1001). Only the clock is pinned; every other line is compared.
    app.set_clock(Box::new(openchuck_app::FixedClock(
        std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_792_575_005),
    )));
    let run = post(&mut app, OutputGrouping::Operation);
    let out = folder("together-out");
    let run = std::sync::Arc::new(run);
    let plan = app.prepare_output(&out, &run).unwrap();
    let written_now = |app: &openchuck_app::Application| {
        app.overview(None, &openchuck_app::JobLog::default())
            .g_code_written()
    };
    assert!(!written_now(&app));
    let report = app.write_output(&plan, false);
    assert!(report.is_complete(), "{report:?}");
    // A complete write is the current export the step bar shows (UI-10,
    // decision 0282 item 5).
    assert!(written_now(&app));
    let written = report.written;
    let names: Vec<_> = written
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    let mut expected: Vec<String> = run.gcode.iter().map(|(n, _)| n.clone()).collect();
    expected.extend(app.setup_sheet_file_names().unwrap());
    assert_eq!(names, expected);
    assert_eq!(
        app.setup_sheet_file_names().unwrap(),
        ["together-setup-sheet.html", "together-setup-sheet.pdf"]
    );
    for ((name, text), path) in run.gcode.iter().zip(&written) {
        assert_eq!(path, &out.join(name));
        assert_eq!(&std::fs::read_to_string(path).unwrap(), text);
    }
    let sheet = app.setup_sheet_for_run(&run);
    assert_eq!(
        std::fs::read_to_string(out.join(&expected[expected.len() - 2])).unwrap(),
        sheet.to_html()
    );
    assert_eq!(
        std::fs::read(out.join(&expected[expected.len() - 1])).unwrap(),
        sheet.to_pdf()
    );
    std::fs::remove_dir_all(out).unwrap();
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_12_req_gc_15_req_nfr_12_a_run_made_before_an_edit_writes_nothing() {
    // An edit after posting (here the tolerance the sheet records) means the
    // run no longer matches the project: nothing is written, and the error
    // says to make the files again.
    let (dir, mut app) = project("together-stale", false);
    let run = post(&mut app, OutputGrouping::Setup);
    let run = std::sync::Arc::new(run);
    let out = folder("together-stale-out");
    let plan = app.prepare_output(&out, &run).unwrap();
    let first = app.project().operations()[0].id;
    app.set_parameter(first, "tolerance", Some(ParameterValue::Number(0.02)))
        .unwrap();
    let report = app.write_output(&plan, false);
    assert!(
        report
            .failed
            .as_ref()
            .unwrap()
            .reason
            .contains("made again")
    );
    assert!(report.written.is_empty());
    assert!(!out.exists(), "nothing is written");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_05_req_gc_12_req_ui_12_write_messages_retain_every_actual_full_path_and_bytes() {
    for grouping in [OutputGrouping::Project, OutputGrouping::Operation] {
        let (dir, mut app) = project("Write feedback", false);
        app.set_clock(Box::new(openchuck_app::FixedClock(
            std::time::UNIX_EPOCH + std::time::Duration::from_secs(1234),
        )));
        let run = post(&mut app, grouping);
        let out = folder("feedback");
        let run = std::sync::Arc::new(run);
        let plan = app.prepare_output(&out, &run).unwrap();
        let report = app.write_output(&plan, false);
        assert!(report.is_complete(), "{report:?}");
        let written = report.written;
        assert_eq!(written.len(), run.gcode.len() + 2);
        for ((name, text), path) in run.gcode.iter().zip(&written) {
            assert!(path.is_absolute());
            assert_eq!(path, &out.join(name));
            assert_eq!(std::fs::read(path).unwrap(), text.as_bytes());
        }
        let [html, pdf] = app.setup_sheet_file_names().unwrap();
        let sheet = app.setup_sheet_for_run(&run);
        assert_eq!(
            std::fs::read(out.join(html)).unwrap(),
            sheet.to_html().as_bytes()
        );
        assert_eq!(std::fs::read(out.join(pdf)).unwrap(), sheet.to_pdf());
        let overview = app.overview(None, &openchuck_app::JobLog::default());
        assert!(
            overview.messages.iter().any(|message| written
                .iter()
                .all(|path| message.text.contains(&path.display().to_string()))
                && message
                    .action
                    .is_some_and(|action| action.label() == "Show result")),
            "Messages must retain all actual paths and offer the result again"
        );
        std::fs::remove_dir_all(out).unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn req_gc_05_req_nfr_07_overwrite_plan_lists_existing_files_without_writing() {
    let (dir, mut app) = project("Replace review", false);
    let run = std::sync::Arc::new(post(&mut app, OutputGrouping::Operation));
    let out = folder("replace-review");
    let fresh = app.prepare_output(&out, &run).unwrap();
    assert!(fresh.existing().is_empty());
    assert!(!out.exists(), "preflight and cancellation create nothing");
    std::fs::create_dir_all(&out).unwrap();
    let old = out.join(&run.gcode[0].0);
    let sheet = out.join(app.setup_sheet_file_names().unwrap()[1].clone());
    std::fs::write(&old, b"old NC").unwrap();
    std::fs::write(&sheet, b"old PDF").unwrap();
    let review = app.prepare_output(&out, &run).unwrap();
    assert_eq!(review.existing(), &[old.clone(), sheet.clone()]);
    assert!(review.paths().iter().all(|path| path.is_absolute()));
    drop(review); // Cancel: the plan has no write side effects.
    assert_eq!(std::fs::read(&old).unwrap(), b"old NC");
    assert_eq!(std::fs::read(&sheet).unwrap(), b"old PDF");
    assert_eq!(std::fs::read_dir(&out).unwrap().count(), 2);
    let review = app.prepare_output(&out, &run).unwrap();
    let refused = app.write_output(&review, false);
    assert!(refused.written.is_empty());
    assert!(refused.failed.is_some());
    assert_eq!(std::fs::read(&old).unwrap(), b"old NC");
    let approved = app.write_output(&review, true);
    assert!(approved.is_complete(), "{approved:?}");
    assert_eq!(approved.written, review.paths());
    assert_eq!(std::fs::read(&old).unwrap(), run.gcode[0].1.as_bytes());
    std::fs::remove_dir_all(out).unwrap();
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_15_req_nfr_07_destination_changes_or_stale_inputs_write_nothing() {
    let (dir, mut app) = project("Revalidate", false);
    let run = std::sync::Arc::new(post(&mut app, OutputGrouping::Operation));
    let out = folder("revalidate");
    let plan = app.prepare_output(&out, &run).unwrap();
    std::fs::create_dir_all(&out).unwrap();
    let appeared = out.join(&run.gcode[1].0);
    std::fs::write(&appeared, b"appeared after review").unwrap();
    let report = app.write_output(&plan, true);
    assert!(report.written.is_empty());
    assert_eq!(report.failed.as_ref().unwrap().path, appeared);
    // A failed or refused write never reads as the current export (UI-10,
    // decision 0282 item 5); its result stays in Messages (0240).
    let written_now = |app: &openchuck_app::Application| {
        app.overview(None, &openchuck_app::JobLog::default())
            .g_code_written()
    };
    assert!(!written_now(&app));
    assert_eq!(std::fs::read(&appeared).unwrap(), b"appeared after review");
    assert_eq!(std::fs::read_dir(&out).unwrap().count(), 1);
    let plan = app.prepare_output(&out, &run).unwrap();
    app.set_parameter(
        app.project().operations()[0].id,
        "tolerance",
        Some(ParameterValue::Number(0.02)),
    )
    .unwrap();
    let report = app.write_output(&plan, true);
    assert!(report.written.is_empty());
    assert!(
        report
            .failed
            .as_ref()
            .unwrap()
            .reason
            .contains("made again")
    );
    assert!(!written_now(&app));
    assert_eq!(std::fs::read_dir(&out).unwrap().count(), 1);
    assert!(
        app.overview(None, &openchuck_app::JobLog::default())
            .messages
            .iter()
            .filter(|m| m.action.is_some_and(|a| a.label() == "Show result"))
            .count()
            >= 2
    );
    std::fs::remove_dir_all(out).unwrap();
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_15_req_nfr_07_no_export_is_prepared_without_gcode() {
    let (dir, mut app) = project("Missing NC", false);
    let mut run = post(&mut app, OutputGrouping::Project);
    run.gcode.clear();
    let out = folder("missing-nc");
    assert!(app.prepare_output(&out, &std::sync::Arc::new(run)).is_err());
    assert!(!out.exists());
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

#[test]
fn req_nfr_12_req_gc_16_skipping_the_stock_simulation_is_acknowledged_apart_from_the_export() {
    // OI-602 (the owner's option C; decision record 0168): its own
    // acknowledgement, per project on each computer, never in the project and
    // independent of the export's.
    let (dir, app) = project("ack-skip", false);
    let settings = folder("ack-skip-settings");
    let here = Acknowledgements::in_folder(&settings);
    assert!(!here.lookup_stock_skipped(app.location()).acknowledged);
    here.acknowledge(app.location()).unwrap();
    assert!(
        !here.lookup_stock_skipped(app.location()).acknowledged,
        "the export's acknowledgement does not cover the skip"
    );
    here.acknowledge_stock_skipped(app.location()).unwrap();
    let again = Acknowledgements::in_folder(&settings);
    assert!(again.lookup_stock_skipped(app.location()).acknowledged);
    assert!(again.lookup(app.location()).acknowledged, "both are kept");
    assert!(
        !Acknowledgements::in_folder(&folder("ack-skip-fresh"))
            .lookup_stock_skipped(app.location())
            .acknowledged
    );
    let project_text = std::fs::read_to_string(dir.join("project.toml")).unwrap();
    assert!(!project_text.contains("acknowledged"));
    for d in [dir, settings] {
        std::fs::remove_dir_all(d).unwrap();
    }
}

/// Issue 331: Messages adds a full stop to text that does not end a
/// sentence, so a path on the last line gained a "." that is not part of the
/// file name. Each path line must be exactly the path.
#[test]
fn req_ui_12_req_gc_05_the_retained_export_message_keeps_every_path_exact() {
    let (dir, app) = project("Exact paths", false);
    let written = PathBuf::from("/out/01-part.nc");
    let failed = PathBuf::from("/out/02-part.nc");
    let skipped = PathBuf::from("/out/part-setup-sheet.pdf");
    app.record_output_report(openchuck_app::OutputReport {
        written: vec![written],
        failed: Some(openchuck_app::OutputFailure {
            path: failed,
            reason: "The disk is full".to_owned(),
        }),
        not_attempted: vec![skipped],
    });
    let overview = app.overview(None, &openchuck_app::JobLog::default());
    let message = overview
        .messages
        .iter()
        .find(|m| m.action.is_some_and(|a| a.label() == "Show result"))
        .unwrap();
    assert_eq!(
        message.text,
        "Export: Wrote 1 file; export incomplete.\n\
         Written: /out/01-part.nc\n\
         Failed: /out/02-part.nc — The disk is full\n\
         Not attempted: /out/part-setup-sheet.pdf\n\
         End of the export result."
    );
    std::fs::remove_dir_all(dir).unwrap();
}

fn templates(gcode: &str, setup_sheet: &str) -> openchuck_app::FileNameTemplates {
    openchuck_app::FileNameTemplates {
        gcode: gcode.to_owned(),
        setup_sheet: setup_sheet.to_owned(),
    }
}

fn file_names(paths: &[PathBuf]) -> Vec<String> {
    paths
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect()
}

/// Issue 308: untouched templates keep every name and byte as before, for
/// every grouping (GC-05, GC-07).
#[test]
fn req_gc_05_req_gc_07_the_default_templates_keep_the_names_and_bytes() {
    for grouping in [
        OutputGrouping::Project,
        OutputGrouping::Setup,
        OutputGrouping::Operation,
        OutputGrouping::Tool,
    ] {
        let (dir, mut app) = project("Default names", false);
        let run = std::sync::Arc::new(post(&mut app, grouping));
        let named = app.output_files(&run).unwrap();
        let names: Vec<&str> = named.programs.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, self::names(&run), "{grouping:?}");
        assert_eq!(
            named.setup_sheet,
            [
                "default-names-setup-sheet.html",
                "default-names-setup-sheet.pdf"
            ]
        );
        let plan = app.prepare_output(&folder("default-names"), &run).unwrap();
        let expected: Vec<String> = run
            .gcode
            .iter()
            .map(|(n, _)| n.clone())
            .chain(named.setup_sheet.iter().cloned())
            .collect();
        assert_eq!(file_names(plan.paths()), expected);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

/// Issue 308: the names the dialog previews are the names written, and a
/// template changes only the names, never the program bytes (GC-07).
#[test]
fn req_gc_05_req_gc_07_a_template_names_the_written_files_as_previewed() {
    let (dir, mut app) = project("Bracket", false);
    app.set_clock(Box::new(openchuck_app::FixedClock(
        std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_792_575_005),
    )));
    let run = std::sync::Arc::new(post(&mut app, OutputGrouping::Tool));
    app.set_file_name_templates(templates(
        "{project}_{setup_no}_{n}_T{tool_no}_{tool}_{date}",
        "{machine} {post} sheet",
    ));
    let named = app.output_files(&run).unwrap();
    let previewed: Vec<String> = named.programs.iter().map(|p| p.name.clone()).collect();
    assert_eq!(
        previewed,
        [
            "bracket_1_01_T1_4-mm-ball_2026-10-21.nc",
            "bracket_1_02_T2_6-mm-ball_2026-10-21.nc",
            "bracket_1_03_T1_4-mm-ball_2026-10-21.nc",
        ]
    );
    assert_eq!(
        named.setup_sheet,
        [
            "example-x-rotary makera-z1 sheet.html",
            "example-x-rotary makera-z1 sheet.pdf"
        ]
    );
    let out = folder("templated");
    let plan = app.prepare_output(&out, &run).unwrap();
    let report = app.write_output(&plan, false);
    assert!(report.is_complete(), "{report:?}");
    let mut expected = previewed.clone();
    expected.extend(named.setup_sheet.iter().cloned());
    assert_eq!(file_names(&report.written), expected);
    for ((_, text), path) in run.gcode.iter().zip(&report.written) {
        assert_eq!(&std::fs::read_to_string(path).unwrap(), text);
    }
    std::fs::remove_dir_all(out).unwrap();
    std::fs::remove_dir_all(dir).unwrap();
}

/// Issue 308 with the owner's option B on OI-1072: outputs whose templates
/// give one name, across every program file and both sheets and ignoring
/// letter case, are written under numbered names, with a warning, and none
/// is written over another; the sheet lists the names written.
#[test]
fn req_gc_05_req_nfr_07_clashing_names_are_numbered_and_none_is_overwritten() {
    let (dir, mut app) = project("Clash", false);
    let run = std::sync::Arc::new(post(&mut app, OutputGrouping::Operation));
    // Every program is "clash.nc"; the sheet clashes with nothing.
    app.set_file_name_templates(templates("{project}", "{project}-setup-sheet"));
    let named = app.output_files(&run).unwrap();
    let names: Vec<&str> = named.programs.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["clash.nc", "clash-2.nc", "clash-3.nc"]);
    let warning = named.warning().unwrap();
    assert!(
        warning.contains("'clash.nc' is written as 'clash-2.nc'"),
        "{warning}"
    );
    assert_eq!(named.renamed.len(), 2);
    // (Letter case and the sheet pair are covered in `output_names`: the
    // fields here are all lower case, and the sheet's extensions differ.)
    app.set_file_name_templates(templates("{project}", "{project}"));
    let out = folder("clash");
    let plan = app.prepare_output(&out, &run).unwrap();
    let report = app.write_output(&plan, false);
    assert!(report.is_complete(), "{report:?}");
    assert_eq!(
        file_names(&report.written),
        [
            "clash.nc",
            "clash-2.nc",
            "clash-3.nc",
            "clash.html",
            "clash.pdf"
        ]
    );
    for ((_, text), path) in run.gcode.iter().zip(&report.written) {
        assert_eq!(&std::fs::read_to_string(path).unwrap(), text);
    }
    let html = std::fs::read_to_string(out.join("clash.html")).unwrap();
    for name in ["clash.nc", "clash-2.nc", "clash-3.nc"] {
        let bytes = std::fs::read(out.join(name)).unwrap();
        assert!(html.contains(&format!(": {name}</h2>")), "{name}");
        assert!(html.contains(&openchuck_app::output_names::file_hash::hex(&bytes)));
    }
    std::fs::remove_dir_all(out).unwrap();
    std::fs::remove_dir_all(dir).unwrap();
}

/// Decision 1 on OI-1070: a template that cannot name a file at all is
/// still an error, writes nothing and creates nothing.
#[test]
fn req_gc_05_req_nfr_07_a_template_that_cannot_name_a_file_writes_nothing() {
    let (dir, mut app) = project("Unnamed", false);
    let run = std::sync::Arc::new(post(&mut app, OutputGrouping::Operation));
    let out = folder("unnamed");
    for (gcode, sheet, says) in [
        ("{n}", "{project", "no matching }"),
        ("{n}-{nope}", "{project}", "no field {nope}"),
        ("{n}-{operation}", "{tool}", "covers every file"),
    ] {
        app.set_file_name_templates(templates(gcode, sheet));
        let error = app.prepare_output(&out, &run).unwrap_err().to_string();
        assert!(error.contains(says), "{gcode} / {sheet}: {error}");
        assert_eq!(app.output_files(&run).unwrap_err().to_string(), error);
        assert!(
            !out.exists(),
            "nothing is created before the names are settled"
        );
    }
    // The same-run sheet, if shown, says why it lists no files.
    let html = app.setup_sheet_for_run(&run).to_html();
    assert!(
        html.contains("The program files could not be named"),
        "{html}"
    );
    // One file per setup holds three operations: {operation} has no value.
    let (dir2, mut app2) = project("Unnamed setup", false);
    let run2 = std::sync::Arc::new(post(&mut app2, OutputGrouping::Setup));
    app2.set_file_name_templates(templates("{operation}", "{project}-setup-sheet"));
    let error = app2.output_files(&run2).unwrap_err().to_string();
    assert!(
        error.contains("holds 3 operations") && error.contains("one file per operation"),
        "{error}"
    );
    std::fs::remove_dir_all(dir).unwrap();
    std::fs::remove_dir_all(dir2).unwrap();
}

/// Issue 309: the sheet lists each program file by its written name, in
/// the order they run, with the SHA-256 of the bytes on disk, its setup,
/// and its operations, tools and tool changes in order.
#[test]
fn req_gc_12_req_gc_05_the_sheet_lists_each_file_its_hash_and_its_tool_changes() {
    let (dir, mut app) = project("Hashes", false);
    app.set_clock(Box::new(openchuck_app::FixedClock(
        std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_792_575_005),
    )));
    let run = std::sync::Arc::new(post(&mut app, OutputGrouping::Setup));
    app.set_file_name_templates(templates("{project}-{setup_no}", "{project}-sheet"));
    let out = folder("hashes");
    let plan = app.prepare_output(&out, &run).unwrap();
    let report = app.write_output(&plan, false);
    assert!(report.is_complete(), "{report:?}");
    let html = std::fs::read_to_string(out.join("hashes-sheet.html")).unwrap();
    let nc = std::fs::read(out.join("hashes-1.nc")).unwrap();
    let hash = openchuck_app::output_names::file_hash::hex(&nc);
    assert_eq!(hash.len(), 64);
    assert!(html.contains("File 1: hashes-1.nc"), "{html}");
    assert!(html.contains(&format!("<dt>SHA-256</dt><dd>{hash}</dd>")));
    assert!(html.contains(&format!("<dt>Size</dt><dd>{} bytes</dd>", nc.len())));
    assert!(html.contains("<dt>Setup</dt><dd>1: Setup 1</dd>"));
    assert!(html.contains("sha256sum"));
    // T1, then T2, then back to T1: three tool changes, in program order,
    // as the post writes them (one M6 for each).
    let rows = [
        "<tr><td>1</td><td>1</td><td>Finish A</td><td>T1 4 mm ball</td><td>Change to T1</td></tr>",
        "<tr><td>2</td><td>2</td><td>Finish B</td><td>T2 6 mm ball</td><td>Change to T2</td></tr>",
        "<tr><td>3</td><td>3</td><td>Touch up A</td><td>T1 4 mm ball</td><td>Change to T1</td></tr>",
    ];
    let mut at = 0;
    for row in rows {
        let found = html[at..]
            .find(row)
            .unwrap_or_else(|| panic!("{row}\n{html}"));
        at += found + row.len();
    }
    let text = String::from_utf8(nc).unwrap();
    assert_eq!(text.matches("M6").count(), 3);
    // The PDF carries the same full hash on one line.
    let pdf = std::fs::read(out.join("hashes-sheet.pdf")).unwrap();
    assert!(
        String::from_utf8_lossy(&pdf).contains(&hash),
        "the PDF holds the whole hash"
    );
    std::fs::remove_dir_all(out).unwrap();
    std::fs::remove_dir_all(dir).unwrap();
}

/// Issue 309: consecutive operations with one tool change tool once.
#[test]
fn req_gc_12_the_sheet_marks_only_real_tool_changes() {
    let (dir, mut app) = project("Changes", false);
    let run = std::sync::Arc::new(post(&mut app, OutputGrouping::Tool));
    let named = app.output_files(&run).unwrap();
    for program in &named.programs {
        let changes: Vec<bool> = program.steps.iter().map(|s| s.tool_change).collect();
        assert_eq!(changes.first(), Some(&true), "{program:?}");
        assert!(changes.iter().skip(1).all(|c| !c), "{program:?}");
    }
    std::fs::remove_dir_all(dir).unwrap();
}

/// Issue 309: a sheet written without program files says so, and lists
/// none.
#[test]
fn req_gc_12_a_sheet_written_alone_says_no_program_files_were_written_with_it() {
    let (dir, mut app) = project("Alone", false);
    app.set_post(None);
    let run = app.regenerate().unwrap();
    assert!(run.gcode.is_empty());
    let out = folder("alone");
    let written = app.write_setup_sheet_for_run(&out, &run).unwrap();
    let html = std::fs::read_to_string(&written[0]).unwrap();
    assert!(
        html.contains("No program files were written with this sheet"),
        "{html}"
    );
    assert!(!html.contains("SHA-256"));
    let preview = app.setup_sheet(None).to_html();
    assert!(preview.contains("No program files were written with this sheet"));
    std::fs::remove_dir_all(out).unwrap();
    std::fs::remove_dir_all(dir).unwrap();
}

/// Issue 309 and decision 0240 item 3: the sheets are written after every
/// program file, and writing stops at the first failure, so a sheet is
/// written only once every file it lists has been written.
#[test]
fn req_gc_12_req_nfr_07_the_sheets_are_written_after_every_program_file() {
    let (dir, mut app) = project("Order", false);
    let run = std::sync::Arc::new(post(&mut app, OutputGrouping::Operation));
    let plan = app.prepare_output(&folder("order"), &run).unwrap();
    let names = file_names(plan.paths());
    let sheets = app.output_files(&run).unwrap().setup_sheet;
    assert_eq!(names.len(), run.gcode.len() + 2);
    assert_eq!(names[run.gcode.len()..], sheets);
    std::fs::remove_dir_all(dir).unwrap();
}

// OUTPUT3 (#312, decision 0327): the read-only preview of the program files.

#[test]
fn req_gc_07_req_gc_05_the_preview_bytes_are_the_bytes_written_with_their_hashes() {
    let (dir, mut app) = project("Preview", true);
    let run = std::sync::Arc::new(post(&mut app, OutputGrouping::Setup));
    app.set_file_name_templates(templates("{project}-{setup_no}", "{project}-sheet"));
    let preview = app.output_preview(&run).unwrap();
    assert!(preview.files().len() >= 2, "one file per setup");
    let out = folder("preview-bytes");
    let plan = app.prepare_output(&out, &run).unwrap();
    assert!(app.write_output(&plan, false).is_complete());
    for file in preview.files() {
        let on_disk = std::fs::read(out.join(file.name())).unwrap();
        assert_eq!(file.bytes(), on_disk.as_slice(), "{}", file.name());
        assert_eq!(file.size(), on_disk.len());
        assert_eq!(
            file.sha256(),
            openchuck_app::output_names::file_hash::hex(&on_disk)
        );
        // The setup sheet lists the same hash.
        let html = std::fs::read_to_string(out.join("preview-sheet.html")).unwrap();
        assert!(html.contains(file.sha256()), "{}", file.name());
    }
    assert_eq!(
        preview.files().iter().map(|f| f.name()).collect::<Vec<_>>(),
        app.output_files(&run)
            .unwrap()
            .programs
            .iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>()
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_05_the_preview_follows_the_templates_and_numbers_clashing_names() {
    let (dir, mut app) = project("Preview names", true);
    let run = std::sync::Arc::new(post(&mut app, OutputGrouping::Setup));
    app.set_file_name_templates(templates("same", "sheet"));
    let preview = app.output_preview(&run).unwrap();
    let names: Vec<&str> = preview.files().iter().map(|f| f.name()).collect();
    assert_eq!(names[0], "same.nc");
    assert_eq!(names[1], "same-2.nc");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_15_a_preview_of_a_run_the_project_has_left_behind_is_refused() {
    let (dir, mut app) = project("Preview stale", false);
    let run = std::sync::Arc::new(post(&mut app, OutputGrouping::Project));
    let first = app.project().operations()[0].id;
    app.execute(Command::SetOperationEnabled {
        operation: first,
        enabled: false,
    })
    .unwrap();
    assert!(matches!(
        app.output_preview(&run),
        Err(openchuck_app::AppError::OutOfDate)
    ));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_07_the_preview_lines_are_the_lines_of_the_file() {
    let (dir, mut app) = project("Preview lines", false);
    let run = std::sync::Arc::new(post(&mut app, OutputGrouping::Project));
    let preview = app.output_preview(&run).unwrap();
    let file = &preview.files()[0];
    let text = std::str::from_utf8(file.bytes()).unwrap();
    assert_eq!(file.line_count(), text.lines().count());
    for (n, line) in text.lines().enumerate() {
        assert_eq!(file.line(n), Some(line));
    }
    assert_eq!(file.line(file.line_count()), None);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_07_search_finds_the_next_and_previous_line_and_wraps() {
    let (dir, mut app) = project("Preview search", false);
    let run = std::sync::Arc::new(post(&mut app, OutputGrouping::Project));
    let preview = app.output_preview(&run).unwrap();
    let file = &preview.files()[0];
    let text = std::str::from_utf8(file.bytes()).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    let hits: Vec<usize> = (0..lines.len())
        .filter(|n| lines[*n].contains("M6"))
        .collect();
    assert!(!hits.is_empty(), "the program changes tool at least once");
    assert_eq!(file.find("M6", None, true), Some(hits[0]));
    assert_eq!(file.find("M6", None, false), hits.last().copied());
    // From a hit, forward goes to the next one, and wraps after the last.
    let mut at = None;
    let mut seen = Vec::new();
    for _ in 0..hits.len() {
        at = file.find("M6", at, true);
        seen.push(at.unwrap());
    }
    assert_eq!(seen, hits);
    assert_eq!(file.find("M6", at, true), Some(hits[0]));
    assert_eq!(file.find("M6", Some(hits[0]), false), hits.last().copied());
    assert_eq!(file.find("no such text anywhere", None, true), None);
    assert_eq!(file.find("", None, true), None);
    std::fs::remove_dir_all(dir).unwrap();
}
