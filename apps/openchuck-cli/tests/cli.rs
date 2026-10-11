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
    assert!(text.starts_with("schema = 1\n"), "{text}");
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
fn req_nfr_07_cli_reports_a_folder_without_a_project() {
    let dir = std::env::temp_dir().join(format!("openchuck-bare-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let out = run(&["validate".as_ref(), dir.as_os_str()]);
    assert_eq!(out.code, Some(1));
    assert!(out.stderr.contains("has no project.toml"), "{}", out.stderr);
}
