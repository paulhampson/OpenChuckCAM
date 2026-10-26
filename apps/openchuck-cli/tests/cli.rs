// SPDX-License-Identifier: AGPL-3.0-or-later
//! End-to-end CLI scenarios (verification plan section 5, openchuck-cli row).

// Test code may unwrap, index and panic (NFR-07 applies to library code).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_cmp
)]

use assert_cmd::Command;

struct Outcome {
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

fn run(args: &[&std::ffi::OsStr]) -> Outcome {
    let output = Command::cargo_bin("openchuck-cli")
        .unwrap()
        .args(args)
        .output()
        .unwrap();
    Outcome {
        code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

fn empty_project(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("openchuck-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let out = run(&["new".as_ref(), dir.as_os_str()]);
    assert_eq!(out.code, Some(0), "{}", out.stderr);
    dir
}

#[test]
fn req_nfr_01_cli_regenerates_an_empty_project_headless() {
    let project = empty_project("regenerate");
    let out = run(&["regenerate".as_ref(), project.as_os_str()]);
    assert_eq!(out.code, Some(0), "{}", out.stderr);
    assert!(
        out.stdout.contains("0 toolpath groups, 0 G-code files"),
        "{}",
        out.stdout
    );
}

#[test]
fn req_nfr_12_cli_prints_the_verify_notice_on_regenerate() {
    let project = empty_project("notice");
    let out = run(&["regenerate".as_ref(), project.as_os_str()]);
    assert_eq!(out.code, Some(0), "{}", out.stderr);
    assert!(
        out.stdout.contains("must be verified by the user"),
        "{}",
        out.stdout
    );
}

#[test]
fn req_nfr_01_cli_validates_and_reports_an_empty_project() {
    let project = empty_project("report");
    let out = run(&["validate".as_ref(), project.as_os_str()]);
    assert_eq!(out.code, Some(0), "{}", out.stderr);
    let out = run(&[
        "--threads".as_ref(),
        "1".as_ref(),
        "report".as_ref(),
        project.as_os_str(),
    ]);
    assert_eq!(out.code, Some(0), "{}", out.stderr);
    assert!(out.stdout.contains("operations: 0"), "{}", out.stdout);
}

#[test]
fn req_nfr_07_cli_reports_a_missing_project_clearly() {
    let out = run(&["validate".as_ref(), "definitely/not/here".as_ref()]);
    assert_eq!(out.code, Some(1));
    assert!(
        out.stderr
            .contains("project folder 'definitely/not/here' does not exist"),
        "{}",
        out.stderr
    );
}

#[test]
fn req_nfr_07_cli_rejects_a_file_as_a_project() {
    let file = std::env::temp_dir().join(format!("openchuck-file-{}.txt", std::process::id()));
    std::fs::write(&file, "not a project").unwrap();
    let out = run(&["report".as_ref(), file.as_os_str()]);
    assert_eq!(out.code, Some(1));
    assert!(
        out.stderr.contains("is not a project folder"),
        "{}",
        out.stderr
    );
}

#[test]
fn req_nfr_07_cli_rejects_unknown_arguments_without_panicking() {
    let out = run(&["regenerate".as_ref(), "--no-such-flag".as_ref()]);
    assert_eq!(out.code, Some(2));
    assert!(out.stderr.contains("unexpected argument"), "{}", out.stderr);
    assert!(!out.stderr.contains("panicked"), "{}", out.stderr);
}

#[test]
fn req_prj_01_cli_creates_validates_and_resaves_a_project() {
    let project = empty_project("resave");
    let text = std::fs::read_to_string(project.join("project.toml")).unwrap();
    assert!(
        text.starts_with(&format!(
            "schema = {}\n",
            openchuck_app::project::file::SCHEMA
        )),
        "{text}"
    );
    let out = run(&["resave".as_ref(), project.as_os_str()]);
    assert_eq!(out.code, Some(0), "{}", out.stderr);
    assert_eq!(
        std::fs::read_to_string(project.join("project.toml")).unwrap(),
        text
    );
    // A second `new` in the same folder is refused.
    let out = run(&["new".as_ref(), project.as_os_str()]);
    assert_eq!(out.code, Some(1));
    assert!(out.stderr.contains("already exists"), "{}", out.stderr);
}

#[test]
fn req_prj_02_cli_validate_reports_a_changed_model() {
    let project = empty_project("model-check");
    std::fs::create_dir_all(project.join("models")).unwrap();
    std::fs::write(project.join("models/part.stl"), "solid part").unwrap();
    // A recorded hash that does not match the file, as after an edit.
    let hash = "blake3:0e8cb5af4c10b1b6c8b8f3b5c8f4c6c8b0a5e5f4a9d8b1c2e3f4a5b6c7d8e9f0";
    let file = project.join("project.toml");
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(text.contains("models = []\n"), "{text}");
    let text = text.replace(
        "models = []\n",
        &format!(
            "\n[[models]]\nid = \"00000000-0000-0000-0000-000000000001\"\npath = \"models/part.stl\"\nhash = \"{hash}\"\n"
        ),
    );
    std::fs::write(&file, text).unwrap();
    let out = run(&["validate".as_ref(), project.as_os_str()]);
    assert_eq!(out.code, Some(1), "{}", out.stdout);
    assert!(
        out.stderr.contains("has changed since it was added"),
        "{}",
        out.stderr
    );
}

#[test]
fn req_nfr_07_cli_warns_about_a_folder_without_a_project() {
    let dir = std::env::temp_dir().join(format!("openchuck-bare-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // Not on a terminal and without --create: a warning, an error, no project.
    let out = run(&["validate".as_ref(), dir.as_os_str()]);
    assert_eq!(out.code, Some(1));
    assert!(out.stderr.contains("warning:"), "{}", out.stderr);
    assert!(out.stderr.contains("has no project.toml"), "{}", out.stderr);
    assert!(out.stderr.contains("--create"), "{}", out.stderr);
    assert!(!dir.join("project.toml").exists());
}

#[test]
fn req_prj_01_cli_create_option_makes_the_project() {
    let dir = std::env::temp_dir().join(format!("openchuck-create-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let out = run(&["--create".as_ref(), "validate".as_ref(), dir.as_os_str()]);
    assert_eq!(out.code, Some(0), "{}", out.stderr);
    assert!(out.stderr.contains("warning:"), "{}", out.stderr);
    assert!(out.stdout.contains("created project"), "{}", out.stdout);
    assert!(dir.join("project.toml").exists());
    // The next run finds the project and says nothing.
    let out = run(&["validate".as_ref(), dir.as_os_str()]);
    assert_eq!(out.code, Some(0), "{}", out.stderr);
    assert!(!out.stderr.contains("warning:"), "{}", out.stderr);
}

fn corpus(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/stl")
        .join(name)
}

#[test]
fn req_imp_01_imp_04_cli_inspect_prints_the_summary_and_report() {
    let out = run(&["inspect".as_ref(), corpus("cube_binary.stl").as_os_str()]);
    assert_eq!(out.code, Some(0), "{}", out.stderr);
    assert!(
        out.stdout.contains("12 triangles, 8 vertices"),
        "{}",
        out.stdout
    );
    assert!(
        out.stdout.contains("X 0 to 10, Y 0 to 10, Z 0 to 10 mm"),
        "{}",
        out.stdout
    );
    assert!(
        out.stdout.contains("checks:    no problems found"),
        "{}",
        out.stdout
    );
    // The STL tessellation statement (slice 2 item 7; verification plan section 5).
    assert!(
        out.stdout
            .contains("gouge tolerance on this part is measured against this mesh"),
        "{}",
        out.stdout
    );

    // A part with defects: each problem is listed, and inspecting still succeeds.
    let out = run(&[
        "inspect".as_ref(),
        corpus("defect_non_manifold.stl").as_os_str(),
    ]);
    assert_eq!(out.code, Some(0), "{}", out.stderr);
    assert!(
        out.stdout.contains("checks:    2 problem(s)"),
        "{}",
        out.stdout
    );
    assert!(
        out.stdout.contains("1 non-manifold edge(s)"),
        "{}",
        out.stdout
    );
    assert!(
        out.stdout.contains("non-manifold edges: [0, 2]"),
        "{}",
        out.stdout
    );
    let out = run(&[
        "inspect".as_ref(),
        corpus("defect_inside_out.stl").as_os_str(),
    ]);
    assert!(
        out.stdout.contains("12 flipped triangle(s)"),
        "{}",
        out.stdout
    );
    assert!(out.stdout.contains("and 2 more"), "{}", out.stdout);
}

#[test]
fn req_imp_03_cli_inspect_reads_mm_and_asks_about_units() {
    // The header's unit is reported as a question; the part stays in mm.
    let out = run(&["inspect".as_ref(), corpus("cube_inch.stl").as_os_str()]);
    assert!(
        out.stdout.contains("units:     mm, the default"),
        "{}",
        out.stdout
    );
    assert!(
        out.stdout
            .contains("unit?      the file says its unit is in, but it was read in mm"),
        "{}",
        out.stdout
    );
    assert!(out.stdout.contains("X 0 to 1,"), "{}", out.stdout);

    // A part too small or too large in mm is asked about too.
    let out = run(&["inspect".as_ref(), corpus("cube_small.stl").as_os_str()]);
    assert!(
        out.stdout.contains("may be in in or cm or m"),
        "{}",
        out.stdout
    );
    let out = run(&["inspect".as_ref(), corpus("cube_huge.stl").as_os_str()]);
    assert!(out.stdout.contains("may be in um"), "{}", out.stdout);
    let out = run(&["inspect".as_ref(), corpus("cube_ascii.stl").as_os_str()]);
    assert!(!out.stdout.contains("unit?"), "{}", out.stdout);

    // --units answers it.
    let out = run(&[
        "inspect".as_ref(),
        corpus("cube_inch.stl").as_os_str(),
        "--units".as_ref(),
        "in".as_ref(),
    ]);
    assert!(
        out.stdout.contains("units:     in, set by the user"),
        "{}",
        out.stdout
    );
    assert!(out.stdout.contains("X 0 to 25.4"), "{}", out.stdout);
    assert!(!out.stdout.contains("unit?"), "{}", out.stdout);
    let out = run(&[
        "inspect".as_ref(),
        corpus("cube_inch.stl").as_os_str(),
        "--units".as_ref(),
        "furlong".as_ref(),
    ]);
    assert_eq!(out.code, Some(1));
    assert!(out.stderr.contains("not a length unit"), "{}", out.stderr);
}

#[test]
fn req_imp_03_cli_import_asks_and_stores_the_answer() {
    // Not on a terminal: the model is added in mm, unanswered, with a warning
    // that says how to answer.
    let project = empty_project("unit-question");
    std::fs::create_dir_all(project.join("models")).unwrap();
    let file = project.join("models/inch.stl");
    std::fs::copy(corpus("cube_inch.stl"), &file).unwrap();
    let p = project.as_os_str();
    let out = run(&["import".as_ref(), p, file.as_os_str()]);
    assert_eq!(out.code, Some(0), "{}", out.stderr);
    assert!(
        out.stderr.contains("warning: the file says its unit is in"),
        "{}",
        out.stderr
    );
    assert!(out.stderr.contains("openchuck-cli units"), "{}", out.stderr);
    let text = std::fs::read_to_string(project.join("project.toml")).unwrap();
    assert!(!text.contains("units"), "{text}");
    let out = run(&["validate".as_ref(), p]);
    assert_eq!(out.code, Some(0), "{}", out.stderr);
    assert!(out.stderr.contains("its unit is in"), "{}", out.stderr);

    // The answer is stored on the model and the question is settled.
    let out = run(&[
        "units".as_ref(),
        p,
        "models/inch.stl".as_ref(),
        "in".as_ref(),
    ]);
    assert_eq!(out.code, Some(0), "{}", out.stderr);
    assert!(out.stdout.contains("X 0 to 25.4"), "{}", out.stdout);
    let text = std::fs::read_to_string(project.join("project.toml")).unwrap();
    assert!(text.contains("units = \"in\"\n"), "{text}");
    let out = run(&["validate".as_ref(), p]);
    assert!(!out.stderr.contains("its unit is in"), "{}", out.stderr);

    // --units on import answers it up front: stored, nothing asked.
    let small = project.join("models/small.stl");
    std::fs::copy(corpus("cube_small.stl"), &small).unwrap();
    let out = run(&[
        "import".as_ref(),
        p,
        small.as_os_str(),
        "--units".as_ref(),
        "in".as_ref(),
    ]);
    assert_eq!(out.code, Some(0), "{}", out.stderr);
    assert!(!out.stderr.contains("warning"), "{}", out.stderr);
    assert!(out.stdout.contains("X 0 to 25.4"), "{}", out.stdout);

    // `default` unsets it again.
    let out = run(&[
        "units".as_ref(),
        p,
        "models/inch.stl".as_ref(),
        "default".as_ref(),
    ]);
    assert_eq!(out.code, Some(0), "{}", out.stderr);
    assert!(out.stdout.contains("unit?"), "{}", out.stdout);
}

#[test]
fn req_nfr_07_cli_inspect_reports_a_damaged_file() {
    for (name, expected) in [
        ("broken_truncated_binary.stl", "it is truncated"),
        ("broken_garbage.stl", "not a valid STL file"),
        ("missing.stl", "cannot access"),
    ] {
        let out = run(&["inspect".as_ref(), corpus(name).as_os_str()]);
        assert_eq!(out.code, Some(1), "{name}");
        assert!(out.stderr.contains(expected), "{name}: {}", out.stderr);
        assert!(!out.stderr.contains("panicked"), "{name}: {}", out.stderr);
    }
}

#[test]
fn req_imp_03_imp_05_cli_import_units_and_transform_survive_save_and_load() {
    let project = empty_project("model-edit");
    std::fs::create_dir_all(project.join("models")).unwrap();
    let file = project.join("models/cube.stl");
    std::fs::copy(corpus("cube_ascii.stl"), &file).unwrap();
    let p = project.as_os_str();

    let out = run(&["import".as_ref(), p, file.as_os_str()]);
    assert_eq!(out.code, Some(0), "{}", out.stderr);
    assert!(out.stdout.contains("added model"), "{}", out.stdout);

    let out = run(&[
        "units".as_ref(),
        p,
        "models/cube.stl".as_ref(),
        "in".as_ref(),
    ]);
    assert_eq!(out.code, Some(0), "{}", out.stderr);
    let out = run(&[
        "transform".as_ref(),
        p,
        "models/cube.stl".as_ref(),
        "orient=+z".as_ref(),
        "mirror=y".as_ref(),
    ]);
    assert_eq!(out.code, Some(0), "{}", out.stderr);

    // A fresh process reads the override and the placement back from the files.
    let out = run(&["inspect".as_ref(), p]);
    assert_eq!(out.code, Some(0), "{}", out.stderr);
    assert!(
        out.stdout.contains("units:     in, set by the user"),
        "{}",
        out.stdout
    );
    assert!(
        out.stdout
            .contains("placement: rotate 90 deg about y; centre on the rotary axis; mirror y"),
        "{}",
        out.stdout
    );
    assert!(
        out.stdout.contains("Y -127 to 127, Z -127 to 127"),
        "{}",
        out.stdout
    );
    assert!(out.stdout.contains("no problems found"), "{}", out.stdout);
    let text = std::fs::read_to_string(project.join("project.toml")).unwrap();
    assert!(text.contains("units = \"in\"\n"), "{text}");
    let out = run(&["validate".as_ref(), p]);
    assert_eq!(out.code, Some(0), "{}", out.stderr);

    // Back to the file's own unit, and the placement cleared.
    let out = run(&[
        "units".as_ref(),
        p,
        "models/cube.stl".as_ref(),
        "default".as_ref(),
    ]);
    assert_eq!(out.code, Some(0), "{}", out.stderr);
    let out = run(&[
        "transform".as_ref(),
        p,
        "models/cube.stl".as_ref(),
        "--clear".as_ref(),
    ]);
    assert_eq!(out.code, Some(0), "{}", out.stderr);
    let text = std::fs::read_to_string(project.join("project.toml")).unwrap();
    assert!(
        !text.contains("units") && !text.contains("transform"),
        "{text}"
    );

    // Bad input is refused with a message and leaves the project alone.
    for (args, expected) in [
        (
            vec!["transform", "models/cube.stl", "spin=3"],
            "is not a step",
        ),
        (
            vec!["transform", "models/cube.stl", "rotate=q:3"],
            "is not an axis",
        ),
        (
            vec!["transform", "models/cube.stl", "scale=0"],
            "greater than zero",
        ),
        (
            vec!["transform", "models/cube.stl", "translate=1,2"],
            "translate=X,Y,Z",
        ),
        (vec!["units", "models/other.stl", "mm"], "has no model"),
    ] {
        let mut full: Vec<&std::ffi::OsStr> = vec![args[0].as_ref(), p];
        full.extend(args[1..].iter().map(|a| std::ffi::OsStr::new(*a)));
        let out = run(&full);
        assert_eq!(out.code, Some(1), "{args:?}");
        assert!(out.stderr.contains(expected), "{args:?}: {}", out.stderr);
    }
    assert_eq!(
        std::fs::read_to_string(project.join("project.toml")).unwrap(),
        text
    );
}

#[test]
fn req_imp_04_cli_validate_warns_about_mesh_problems() {
    let project = empty_project("model-warn");
    std::fs::create_dir_all(project.join("models")).unwrap();
    let file = project.join("models/holey.stl");
    std::fs::copy(corpus("defect_hole.stl"), &file).unwrap();
    let out = run(&["import".as_ref(), project.as_os_str(), file.as_os_str()]);
    assert_eq!(out.code, Some(0), "{}", out.stderr);
    let out = run(&["validate".as_ref(), project.as_os_str()]);
    assert_eq!(out.code, Some(0), "{}", out.stderr);
    assert!(
        out.stderr
            .contains("warning: model 'models/holey.stl': 1 hole(s)"),
        "{}",
        out.stderr
    );
    // A model damaged after it was added fails validation.
    std::fs::write(&file, b"solid broken\nfacet").unwrap();
    let out = run(&["validate".as_ref(), project.as_os_str()]);
    assert_eq!(out.code, Some(1));
    assert!(out.stderr.contains("has changed"), "{}", out.stderr);
}

fn args<'a>(list: &'a [&'a str]) -> Vec<&'a std::ffi::OsStr> {
    list.iter().map(|a| a.as_ref()).collect()
}

#[test]
fn req_set_01_req_set_03_req_set_05_req_set_06_cli_sets_machine_and_setup() {
    let project = empty_project("setup");
    let p = project.to_str().unwrap();
    std::fs::create_dir_all(project.join("models")).unwrap();
    let cube =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/stl/cube_ascii.stl");
    let model = project.join("models/cube.stl");
    std::fs::copy(cube, &model).unwrap();
    assert_eq!(
        run(&args(&["import", p, model.to_str().unwrap()])).code,
        Some(0)
    );
    assert_eq!(
        run(&args(&["transform", p, "models/cube.stl", "centre"])).code,
        Some(0)
    );

    let out = run(&args(&["machines"]));
    assert!(out.stdout.contains("makera-z1") && out.stdout.contains("6 of 11 values unconfirmed"));
    let out = run(&args(&["machine", p, "makera-z1"]));
    assert_eq!(out.code, Some(0), "{}", out.stderr);
    // The A direction is stated from the shared kinematics, and every Z1
    // value is shown as confirmed or unconfirmed, including the measured block time.
    assert!(out.stdout.contains("faces -Y at A 90"), "{}", out.stdout);
    assert!(
        out.stdout
            .contains("block time: 7.000 ms, window 32, Report"),
        "{}",
        out.stdout
    );
    assert!(
        out.stdout.contains("spindle:    up to 13000 rpm"),
        "{}",
        out.stdout
    );
    assert_eq!(
        out.stdout.matches("UNCONFIRMED").count(),
        6,
        "{}",
        out.stdout
    );
    assert!(project.join("machine.toml").is_file());

    let out = run(&args(&[
        "setup",
        p,
        "--new",
        "Setup 1",
        "--part",
        "models/cube.stl",
        "--stock",
        "cylinder=20,14,-2",
        "--material",
        "oak",
        "--part-offset",
        "1",
        "--work-offset",
        "g55",
        "--work-x",
        "-2",
    ]));
    assert_eq!(out.code, Some(0), "{}", out.stderr);
    // The work zero stays where it is in the setup when the part moves, so
    // the register is the rotary reference point plus the work X alone
    // (decision record 0201).
    assert!(
        out.stdout
            .contains("set G55 to machine X -182.400 Y -108.800 Z -55.000"),
        "{}",
        out.stdout
    );
    assert!(
        out.stdout
            .contains("position:   part moved 1 mm along the rotary axis"),
        "{}",
        out.stdout
    );
    // The Z1 profile's axis height is the owner's (45 mm above the bed, OI-60), so the safe Z
    // fits the travel and the setup has nothing to warn about.
    assert!(out.stdout.contains("no problems found"), "{}", out.stdout);
    assert!(
        !out.stdout.lines().any(|l| l.starts_with("  - ")),
        "{}",
        out.stdout
    );

    // A clearance inside the stock is a warning in `setup` and `validate`,
    // not an error.
    let out = run(&args(&["setup", p, "Setup 1", "--clearance", "6,30"]));
    assert!(
        out.stdout
            .contains("does not enclose the stock (radius 10.000 mm)"),
        "{}",
        out.stdout
    );
    let out = run(&args(&["validate", p]));
    assert_eq!(out.code, Some(0), "{}", out.stderr);
    assert!(
        out.stderr
            .contains("warning: setup 'Setup 1': the safe cylinder"),
        "{}",
        out.stderr
    );
    let out = run(&args(&["report", p]));
    assert!(out.stdout.contains("machine:    Makera Z1") && out.stdout.contains("material:   oak"));

    // Bad values are refused with a clear message.
    let out = run(&args(&["setup", p, "Setup 1", "--clearance", "30,6"]));
    assert_ne!(out.code, Some(0));
    assert!(out.stderr.contains("safe Z"), "{}", out.stderr);
    let out = run(&args(&["setup", p, "Setup 1", "--stock", "cone=1"]));
    assert!(out.stderr.contains("is not a stock"), "{}", out.stderr);
    let out = run(&args(&["machine", p, "no-such-machine"]));
    assert!(
        out.stderr.contains("is not a built-in machine"),
        "{}",
        out.stderr
    );
    let out = run(&args(&["machine", p, "--clear"]));
    assert_eq!(out.code, Some(0), "{}", out.stderr);
    assert!(!project.join("machine.toml").exists());
}

#[test]
fn req_tl_01_req_tl_07_req_tl_11_cli_tool_library_and_snapshots() {
    let project = empty_project("tools");
    let corpus = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/tools");
    let library = project.with_extension("library");
    let _ = std::fs::remove_dir_all(&library);
    let lib = library.as_os_str();
    let p = project.as_os_str();

    let out = run(&[
        "library".as_ref(),
        lib,
        "import".as_ref(),
        corpus.join("library").as_os_str(),
    ]);
    assert_eq!(out.code, Some(0), "{}", out.stderr);
    assert!(out.stdout.contains("8 added"), "{}", out.stdout);
    assert!(library.join("6-mm-bull-nose-r1.toml").is_file());

    // A tool set for the project's machine gives the number (TL-11).
    let out = run(&["machine".as_ref(), p, "makera-z1".as_ref()]);
    assert_eq!(out.code, Some(0), "{}", out.stderr);
    let out = run(&[
        "library".as_ref(),
        lib,
        "tool-set".as_ref(),
        "Makera Z1".as_ref(),
        "--set".as_ref(),
        "4=6 mm bull-nose r1".as_ref(),
    ]);
    assert_eq!(out.code, Some(0), "{}", out.stderr);
    assert!(library.join("tool-sets/makera-z1.toml").is_file());
    let out = run(&[
        "tool".as_ref(),
        p,
        "T4".as_ref(),
        "--add".as_ref(),
        "--library".as_ref(),
        lib,
    ]);
    assert_eq!(out.code, Some(0), "{}", out.stderr);
    assert!(
        out.stdout.contains("as T4 (from the machine's tool set)"),
        "{}",
        out.stdout
    );
    let out = run(&[
        "tool".as_ref(),
        p,
        "3 mm ball".as_ref(),
        "--add".as_ref(),
        "--library".as_ref(),
        lib,
    ]);
    assert!(
        out.stdout.contains("as T1 (the next free number)"),
        "{}",
        out.stdout
    );

    // The number is the project's to change.
    let out = run(&[
        "tool".as_ref(),
        p,
        "T1".as_ref(),
        "--number".as_ref(),
        "7".as_ref(),
        "--length-offset".as_ref(),
        "17".as_ref(),
    ]);
    assert_eq!(out.code, Some(0), "{}", out.stderr);
    assert!(
        out.stdout.contains("controller: T7 H17 D7"),
        "{}",
        out.stdout
    );

    // Edit the library: the project keeps its copy, and the difference is reported.
    let file = library.join("6-mm-bull-nose-r1.toml");
    let text = std::fs::read_to_string(&file)
        .unwrap()
        .replace("corner_radius = 1.0", "corner_radius = 0.5");
    std::fs::write(&file, text).unwrap();
    let out = run(&["tools".as_ref(), p, "--library".as_ref(), lib]);
    assert_eq!(out.code, Some(0), "{}", out.stderr);
    assert!(out.stdout.contains("bull-nose r1,"), "{}", out.stdout);
    assert!(
        out.stdout.contains("differs in tool.shape.corner_radius"),
        "{}",
        out.stdout
    );

    // With the library gone the project still validates and reports.
    std::fs::remove_dir_all(&library).unwrap();
    let out = run(&["validate".as_ref(), p]);
    assert_eq!(out.code, Some(0), "{}", out.stderr);
    let out = run(&["report".as_ref(), p]);
    assert!(out.stdout.contains("6 mm bull-nose r1"), "{}", out.stdout);

    // A bad profile file is refused, naming the segment (in a fresh copy of
    // the sample library, so a regression cannot change the corpus).
    let out = run(&[
        "library".as_ref(),
        lib,
        "import".as_ref(),
        corpus.join("library").as_os_str(),
    ]);
    assert_eq!(out.code, Some(0), "{}", out.stderr);
    let out = run(&[
        "library".as_ref(),
        lib,
        "profile".as_ref(),
        "6 mm flat".as_ref(),
        corpus.join("profiles/broken_undercut.toml").as_os_str(),
    ]);
    assert_eq!(out.code, Some(1));
    assert!(out.stderr.contains("segment 2"), "{}", out.stderr);

    let out = run(&["tool".as_ref(), p, "T4".as_ref(), "--remove".as_ref()]);
    assert_eq!(out.code, Some(0), "{}", out.stderr);
    let out = run(&["tool".as_ref(), p, "T7".as_ref(), "--remove".as_ref()]);
    assert_eq!(out.code, Some(0), "{}", out.stderr);
    let out = run(&["tools".as_ref(), p]);
    assert!(out.stdout.contains("no tools"), "{}", out.stdout);
    std::fs::remove_dir_all(&library).ok();
}

#[test]
fn req_gc_14_gc_11_cli_asks_for_the_tool_numbers_before_writing_g_code() {
    let project = empty_project("tool-numbers");
    let out = run(&["tool-numbers".as_ref(), project.as_os_str()]);
    assert_eq!(out.code, Some(0), "{}", out.stderr);
    assert!(
        out.stdout.contains("no operation uses a tool"),
        "{}",
        out.stdout
    );
    // Not on a terminal and no option: the numbers are shown and the run stops.
    let out = run(&[
        "regenerate".as_ref(),
        project.as_os_str(),
        "--post".as_ref(),
        "linuxcnc".as_ref(),
    ]);
    assert_ne!(out.code, Some(0));
    assert!(out.stderr.contains("--tool-numbers keep"), "{}", out.stderr);
    // With the option it goes on, and the notice is printed (NFR-12).
    let out = run(&[
        "regenerate".as_ref(),
        project.as_os_str(),
        "--post".as_ref(),
        "linuxcnc".as_ref(),
        "--tool-numbers".as_ref(),
        "keep".as_ref(),
    ]);
    assert_eq!(out.code, Some(0), "{}", out.stderr);
    assert!(out.stdout.contains("0 G-code files"), "{}", out.stdout);
    // G-code without a post is refused rather than guessed.
    let out = run(&[
        "regenerate".as_ref(),
        project.as_os_str(),
        "--out".as_ref(),
        "x".as_ref(),
    ]);
    assert_ne!(out.code, Some(0));
    assert!(out.stderr.contains("--post"), "{}", out.stderr);
}

#[test]
fn req_gc_05_req_gc_12_cli_takes_a_grouping_and_writes_the_setup_sheet() {
    let project = empty_project("grouping");
    let out_dir = std::env::temp_dir().join(format!("openchuck-cli-sheet-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out_dir);
    let p = project.as_os_str();
    let run_with = |grouping: &str| {
        run(&[
            "regenerate".as_ref(),
            p,
            "--post".as_ref(),
            "linuxcnc".as_ref(),
            "--tool-numbers".as_ref(),
            "keep".as_ref(),
            "--grouping".as_ref(),
            grouping.as_ref(),
            "--setup-sheet".as_ref(),
            "--out".as_ref(),
            out_dir.as_os_str(),
        ])
    };
    for grouping in ["project", "setup", "operation", "tool"] {
        let out = run_with(grouping);
        assert_eq!(out.code, Some(0), "{grouping}: {}", out.stderr);
    }
    let names: Vec<String> = std::fs::read_dir(&out_dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        names.iter().any(|n| n.ends_with("-setup-sheet.html")),
        "{names:?}"
    );
    assert!(
        names.iter().any(|n| n.ends_with("-setup-sheet.pdf")),
        "{names:?}"
    );
    let out = run_with("everything");
    assert_ne!(out.code, Some(0));
    assert!(out.stderr.contains("everything"), "{}", out.stderr);
    // The sheet needs a folder to go in.
    let out = run(&["regenerate".as_ref(), p, "--setup-sheet".as_ref()]);
    assert_ne!(out.code, Some(0));
    assert!(out.stderr.contains("--out"), "{}", out.stderr);
    std::fs::remove_dir_all(&out_dir).ok();
}

#[test]
fn req_sim_07_cli_sets_the_simulation_cell_and_refuses_one_out_of_range() {
    let project = empty_project("simulate");
    let out = run(&["simulate".as_ref(), project.as_os_str()]);
    assert_eq!(out.code, Some(0), "{}", out.stderr);
    assert!(
        out.stdout.contains("simulation cell 0.1 mm"),
        "{}",
        out.stdout
    );
    assert!(
        out.stdout.contains("no operations to simulate"),
        "{}",
        out.stdout
    );
    // A finer cell is opt-in, saved in the project with its cost stated.
    let out = run(&[
        "simulate".as_ref(),
        project.as_os_str(),
        "--cell".as_ref(),
        "0.05".as_ref(),
    ]);
    assert_eq!(out.code, Some(0), "{}", out.stderr);
    assert!(
        out.stdout
            .contains("about 8.0 x the default's time and 4.0 x its memory"),
        "{}",
        out.stdout
    );
    let text = std::fs::read_to_string(project.join("project.toml")).unwrap();
    assert!(text.contains("[simulation]\ncell = 0.05\n"), "{text}");
    let out = run(&[
        "simulate".as_ref(),
        project.as_os_str(),
        "--cell".as_ref(),
        "0.001".as_ref(),
    ]);
    assert_eq!(out.code, Some(1), "{}", out.stdout);
    assert!(out.stderr.contains("0.025"), "{}", out.stderr);
    assert!(!out.stderr.contains("panicked"), "{}", out.stderr);
}

#[test]
fn req_set_09_req_op_21_cli_reports_and_refuses_workholding() {
    let project = empty_project("holding");
    let p = project.to_str().unwrap();
    let out = run(&args(&[
        "setup",
        p,
        "--new",
        "Setup 1",
        "--stock",
        "cylinder=40,100,0",
    ]));
    assert_eq!(out.code, Some(0), "{}", out.stderr);
    let out = run(&args(&["report", p]));
    assert!(out.stdout.contains("holding:    none"), "{}", out.stdout);

    let file = std::fs::read_dir(project.join("setups"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let original = std::fs::read_to_string(&file).unwrap();
    let held = "\n[workholding.start]\nlength = 15.0\n\n\
                [[workholding.tabs]]\nx_start = 80.0\nlength = 6.0\n\
                shape = { type = \"frustum\", start_diameter = 8.0, end_diameter = 3.0 }\n\
                layout = { type = \"on_axis\" }\n";
    std::fs::write(&file, format!("{original}{held}")).unwrap();
    let out = run(&args(&["validate", p]));
    assert_eq!(out.code, Some(0), "{}{}", out.stdout, out.stderr);
    let out = run(&args(&["report", p]));
    assert!(
        out.stdout
            .contains("held at the start of the stock for 15 mm"),
        "{}",
        out.stdout
    );
    assert!(
        out.stdout.contains("tab 1:") && out.stdout.contains("frustum"),
        "{}",
        out.stdout
    );

    // A tab that reaches into the held region is refused, not a crash.
    std::fs::write(
        &file,
        format!("{original}{held}").replace("x_start = 80.0", "x_start = 10.0"),
    )
    .unwrap();
    let out = run(&args(&["validate", p]));
    assert_ne!(out.code, Some(0));
    assert!(out.stderr.contains("held region"), "{}", out.stderr);
}

#[test]
fn req_set_09_req_op_21_cli_reports_the_end_region_and_tabs_around_the_axis() {
    let project = empty_project("holding-end");
    let p = project.to_str().unwrap();
    let out = run(&args(&[
        "setup",
        p,
        "--new",
        "Setup 1",
        "--stock",
        "cylinder=40,100,0",
    ]));
    assert_eq!(out.code, Some(0), "{}", out.stderr);
    let file = std::fs::read_dir(project.join("setups"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let original = std::fs::read_to_string(&file).unwrap();
    let held = "\n[workholding.end]\nlength = 12.0\ndiameter = 30.0\n\n\
                [[workholding.tabs]]\nx_start = 20.0\nlength = 5.0\n\
                shape = { type = \"cylinder\", diameter = 4.0 }\n\
                layout = { type = \"around_axis\", count = 3, start_angle = 30.0, radial_offset = 10.0 }\n";
    std::fs::write(&file, format!("{original}{held}")).unwrap();
    let out = run(&args(&["report", p]));
    assert_eq!(out.code, Some(0), "{}{}", out.stdout, out.stderr);
    assert!(
        out.stdout
            .contains("held at the end of the stock for 12 mm, diameter 30 mm"),
        "{}",
        out.stdout
    );
    assert!(
        out.stdout
            .contains("3 around the axis at 10 mm from it, first at 30 degrees"),
        "{}",
        out.stdout
    );
}

#[test]
fn req_set_09_cli_names_the_stock_outline_as_the_held_region_on_box_stock() {
    // Without a holding diameter box stock is held at its own outline
    // (decision 0160), and the summary says so rather than "diameter".
    let project = empty_project("holding-box");
    let p = project.to_str().unwrap();
    std::fs::create_dir_all(project.join("models")).unwrap();
    std::fs::copy(corpus("cube_ascii.stl"), project.join("models/cube.stl")).unwrap();
    let out = run(&args(&[
        "import",
        p,
        project.join("models/cube.stl").to_str().unwrap(),
    ]));
    assert_eq!(out.code, Some(0), "{}", out.stderr);
    let out = run(&args(&["transform", p, "models/cube.stl", "centre"]));
    assert_eq!(out.code, Some(0), "{}", out.stderr);
    let out = run(&args(&[
        "setup",
        p,
        "--new",
        "Setup 1",
        "--part",
        "models/cube.stl",
        "--stock",
        "box=-25,-15,-15,25,15,15",
        "--held-start",
        "5",
        "--held-end",
        "4,20",
    ]));
    assert_eq!(out.code, Some(0), "{}{}", out.stdout, out.stderr);
    assert!(
        out.stdout
            .contains("held at the start of the stock for 5 mm, the stock's outline"),
        "{}",
        out.stdout
    );
    assert!(
        out.stdout
            .contains("held at the end of the stock for 4 mm, diameter 20 mm"),
        "{}",
        out.stdout
    );
}

#[test]
fn req_set_09_req_op_21_cli_sets_held_regions_and_tabs() {
    let project = empty_project("holding-set");
    let p = project.to_str().unwrap();
    std::fs::create_dir_all(project.join("models")).unwrap();
    std::fs::copy(corpus("cube_ascii.stl"), project.join("models/cube.stl")).unwrap();
    let out = run(&args(&[
        "import",
        p,
        project.join("models/cube.stl").to_str().unwrap(),
    ]));
    assert_eq!(out.code, Some(0), "{}", out.stderr);
    let out = run(&args(&["transform", p, "models/cube.stl", "centre"]));
    assert_eq!(out.code, Some(0), "{}", out.stderr);
    let out = run(&args(&[
        "setup",
        p,
        "--new",
        "Setup 1",
        "--part",
        "models/cube.stl",
        "--stock",
        "cylinder=30,60,-25",
        "--held-start",
        "5",
        "--held-end",
        "4,20",
        "--tab",
        "frustum=-18,6,12,6",
        "--tab",
        "cylinder=14,6,4@3,30,8",
    ]));
    assert_eq!(out.code, Some(0), "{}{}", out.stdout, out.stderr);
    assert!(
        out.stdout
            .contains("held at the start of the stock for 5 mm"),
        "{}",
        out.stdout
    );
    assert!(
        out.stdout
            .contains("held at the end of the stock for 4 mm, diameter 20 mm"),
        "{}",
        out.stdout
    );
    assert!(
        out.stdout.contains("tab 1:") && out.stdout.contains("frustum"),
        "{}",
        out.stdout
    );
    assert!(
        out.stdout
            .contains("3 around the axis at 8 mm from it, first at 30 degrees"),
        "{}",
        out.stdout
    );
    // The edit is stored: a fresh process reports it, and clearing removes it.
    let out = run(&args(&["report", p]));
    assert!(out.stdout.contains("tab 2:"), "{}", out.stdout);
    let out = run(&args(&[
        "setup",
        p,
        "Setup 1",
        "--clear-tabs",
        "--held-start",
        "none",
    ]));
    assert_eq!(out.code, Some(0), "{}{}", out.stdout, out.stderr);
    assert!(!out.stdout.contains("tab 1:"), "{}", out.stdout);
    assert!(!out.stdout.contains("held at the start"), "{}", out.stdout);
    // A tab that is not a tab, and one that overlaps the held region, are refused.
    let out = run(&args(&["setup", p, "Setup 1", "--tab", "sphere=1,2,3"]));
    assert_ne!(out.code, Some(0));
    assert!(out.stderr.contains("is not a tab"), "{}", out.stderr);
    let out = run(&args(&["setup", p, "Setup 1", "--tab", "cylinder=32,2,4"]));
    assert_ne!(out.code, Some(0));
    assert!(out.stderr.contains("held region"), "{}", out.stderr);
    assert!(!out.stderr.contains("panicked"), "{}", out.stderr);
    // A tab across the part is outside the head and tail zones: refused when
    // editing, and nothing is stored (decision 0045 item 4).
    let out = run(&args(&["setup", p, "Setup 1", "--tab", "cylinder=-1,2,4"]));
    assert_ne!(out.code, Some(0), "{}{}", out.stdout, out.stderr);
    assert!(
        out.stderr
            .contains("Tab 1 (X -1.000 to 1.000 mm) is not wholly in the head or the tail zone"),
        "{}",
        out.stderr
    );
    assert!(!out.stderr.contains("panicked"), "{}", out.stderr);
    let out = run(&args(&["report", p]));
    assert_eq!(out.code, Some(0), "{}{}", out.stdout, out.stderr);
    assert!(!out.stderr.contains("panicked"), "{}", out.stderr);
    assert!(!out.stdout.contains("tab 1:"), "{}", out.stdout);
    // The default tabs come from the part: one at the head, one at the tail.
    let out = run(&args(&["setup", p, "Setup 1", "--default-tabs"]));
    assert_eq!(out.code, Some(0), "{}{}", out.stdout, out.stderr);
    assert!(
        out.stdout.contains("tab 1:") && out.stdout.contains("tab 2:"),
        "{}",
        out.stdout
    );
}

/// NFR-16: `--version` prints every build fact in plain text, in the same format the About window
/// copies (both read `openchuck_buildinfo`).
#[test]
fn req_nfr_16_cli_version_has_every_build_fact() {
    let out = run(&["--version".as_ref()]);
    assert_eq!(out.code, Some(0), "{}", out.stderr);
    for label in [
        "OpenChuckCAM ",
        "Commit: ",
        "Uncommitted changes: ",
        "Built: ",
        "Rust: rustc ",
        "Target: ",
        "Crates:",
        "  openchuck-types ",
        "  openchuck-ui ",
        "  openchuck-cli ",
    ] {
        assert!(out.stdout.contains(label), "{label:?} in {}", out.stdout);
    }
    // clap names the program first; the rest is the build text.
    assert!(
        out.stdout.starts_with("openchuck-cli OpenChuckCAM "),
        "{}",
        out.stdout
    );
}

/// NFR-16: the short flag still prints one line with the version.
#[test]
fn req_nfr_16_cli_short_version_is_one_line() {
    let out = run(&["-V".as_ref()]);
    assert_eq!(out.code, Some(0));
    assert_eq!(out.stdout.lines().count(), 1, "{}", out.stdout);
    assert!(out.stdout.contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn req_gc_10_cli_checks_a_chunk_against_the_original_source_range() {
    let dir = std::env::temp_dir().join(format!("openchuck-source-range-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("original.ngc");
    let report = dir.join("report.json");
    std::fs::write(
        &source,
        "G90 G21 G17 G61 G54\nT2 M6\nS9000 M3\nG0 X1 Y2 Z3 A450\nG1 X11 F60\nX21\nM5\nM30\n",
    )
    .unwrap();
    let start = r#"{"position":[11,2,3,450],"actual_position":[11,2,3,450],"gcodes":[10,170,210,540,610,900,940],"feed":60,"rpm":9000,"tool":2,"spindle_on":true,"direction":1,"g5x_offset":[0,0,0,0],"g92_offset":[0,0,0,0],"rotation_xy":0}"#;
    let end = start
        .replace("[11,2,3,450]", "[21,2,3,450]")
        .replace("\"spindle_on\":true", "\"spindle_on\":false")
        .replace("\"direction\":1", "\"direction\":0");
    let text = format!(
        r#"{{"rows":[],"errors":[],"final_position":[21,2,3,450],"path":[[11,2,3,450],[21,2,3,450]],"boundaries":[{start},{end}]}}"#
    );
    std::fs::write(&report, &text).unwrap();
    let out = run(&[
        "gcode-sim".as_ref(),
        source.as_os_str(),
        report.as_os_str(),
        "--source-lines".as_ref(),
        "6:8".as_ref(),
    ]);
    assert_eq!(out.code, Some(0), "{}", out.stderr);
    assert!(out.stdout.contains("reached all 1 moves"), "{}", out.stdout);
    std::fs::write(&report, text.replace("\"feed\":60", "\"feed\":61")).unwrap();
    let out = run(&[
        "gcode-sim".as_ref(),
        source.as_os_str(),
        report.as_os_str(),
        "--source-lines".as_ref(),
        "6:8".as_ref(),
    ]);
    assert_ne!(out.code, Some(0));
    assert!(out.stderr.contains("source boundary"), "{}", out.stderr);
    std::fs::remove_dir_all(dir).unwrap();
}
