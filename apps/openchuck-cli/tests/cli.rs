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
    assert!(text.starts_with("schema = 2\n"), "{text}");
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
fn req_imp_03_cli_inspect_reads_and_overrides_units() {
    let out = run(&["inspect".as_ref(), corpus("cube_inch.stl").as_os_str()]);
    assert!(
        out.stdout.contains("units:     in, as the file states"),
        "{}",
        out.stdout
    );
    assert!(out.stdout.contains("X 0 to 25.4"), "{}", out.stdout);
    let out = run(&[
        "inspect".as_ref(),
        corpus("cube_inch.stl").as_os_str(),
        "--units".as_ref(),
        "mm".as_ref(),
    ]);
    assert!(
        out.stdout.contains("set by the user (the file states in)"),
        "{}",
        out.stdout
    );
    assert!(out.stdout.contains("X 0 to 1,"), "{}", out.stdout);
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
        "file".as_ref(),
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
