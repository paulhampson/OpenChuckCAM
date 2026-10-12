// SPDX-License-Identifier: AGPL-3.0-or-later
//! The tool library corpus and project tool snapshots (slice 4; TL-01, TL-03,
//! TL-07, NFR-07; decision record 0014).
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};

use openchuck_project::id::ToolId;
use openchuck_project::tools::{
    self, SnapshotStatus, compare_snapshot, load_library, load_profile, save_library,
};
use openchuck_project::{Item, Placement, Project, file};
use openchuck_types::tool::ToolShape;
use openchuck_types::units::Millimetres;

fn corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/tools")
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("openchuck-tools-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn req_tl_01_req_tl_02_req_tl_03_sample_library_reads_every_type() {
    let lib = load_library(&corpus().join("library.toml")).unwrap();
    let types: Vec<&str> = lib
        .tools()
        .iter()
        .map(|t| t.tool.shape.type_name())
        .collect();
    assert_eq!(
        types,
        [
            "flat_end",
            "ball_end",
            "bull_nose",
            "tapered_ball",
            "v_bit",
            "chamfer",
            "drill",
            "custom"
        ]
    );
    for t in lib.tools() {
        let profile = t.tool.profile().unwrap();
        assert_eq!(
            profile.cutting_radius(),
            t.tool.diameter.0 / 2.0,
            "{}",
            t.name
        );
        assert_eq!(profile.top(), t.tool.flute_length.0, "{}", t.name);
    }
    // Written again it reads back the same (the comments are not kept).
    let text = tools::render_library(&lib).unwrap();
    assert_eq!(
        tools::parse_library(Path::new("x.toml"), &text).unwrap(),
        lib
    );
}

#[test]
fn req_tl_01_req_nfr_07_damaged_library_files_give_clear_errors() {
    let cases = [
        ("broken_corner_radius.toml", "corner_radius"),
        ("broken_duplicate_id.toml", "two tools use the ID"),
        ("broken_undercut_profile.toml", "segment 2"),
        ("broken_truncated.toml", "broken_truncated.toml"),
    ];
    for (name, expected) in cases {
        let err = load_library(&corpus().join(name)).unwrap_err().to_string();
        assert!(err.contains(expected), "{name}: {err}");
    }
}

#[test]
fn req_tl_03_req_nfr_07_profile_corpus_reads_or_names_the_fault() {
    let dir = corpus().join("profiles");
    let mut names: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    for name in names {
        let result = load_profile(&dir.join(&name));
        if name.starts_with("broken_") {
            let err = result.unwrap_err().to_string();
            assert!(err.contains(&name), "{name}: {err}");
            let expected = match name.as_str() {
                "broken_undercut.toml" => "segment 2",
                "broken_half_circle.toml" => "segment 2",
                "broken_arc_both_forms.toml" => "segment 1",
                _ => "",
            };
            assert!(err.contains(expected), "{name}: {err}");
        } else {
            result.unwrap_or_else(|e| panic!("{name}: {e}"));
        }
    }
    // The ball typed as two arcs agrees with the standard ball.
    let typed = load_profile(&dir.join("ball_two_arcs.toml")).unwrap();
    let lib = load_library(&corpus().join("library.toml")).unwrap();
    let mut flat = lib.tools()[0].tool.clone();
    flat.shape = ToolShape::Custom { profile: typed };
    let mut ball = flat.clone();
    ball.shape = ToolShape::BallEnd;
    let (a, b) = (flat.profile().unwrap(), ball.profile().unwrap());
    for i in 0..=30 {
        let r = f64::from(i) * 0.1;
        assert!((a.height_at(r).unwrap() - b.height_at(r).unwrap()).abs() < 1e-9);
    }
}

/// A project using two tools from the sample library.
fn project_from_library() -> (Project, tools::ToolLibrary) {
    let lib = load_library(&corpus().join("library.toml")).unwrap();
    let mut project = Project::new("tools");
    for t in &lib.tools()[..2] {
        project
            .insert(Item::Tool(t.snapshot()), Placement::End)
            .unwrap();
    }
    (project, lib)
}

#[test]
fn req_tl_07_project_opens_with_the_library_absent() {
    let (project, lib) = project_from_library();
    let dir = temp_dir("absent");
    let library_path = dir.join("library.toml");
    save_library(&lib, &library_path).unwrap();
    let folder = dir.join("project");
    file::save(&project, &folder).unwrap();
    let files = file::render(&project).unwrap();

    // Without the library the project loads the same and writes the same bytes.
    std::fs::remove_file(&library_path).unwrap();
    let loaded = file::load(&folder).unwrap();
    assert_eq!(loaded, project);
    assert_eq!(file::render(&loaded).unwrap(), files);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn req_tl_07_editing_the_library_never_changes_a_project() {
    let (project, mut lib) = project_from_library();
    let dir = temp_dir("edit");
    let folder = dir.join("project");
    file::save(&project, &folder).unwrap();

    // Change the library's copy of the first tool and remove the second.
    let first = lib.tools()[0].id;
    let second = lib.tools()[1].id;
    let mut edited = lib.get(first).unwrap().clone();
    edited.tool.diameter = Millimetres(5.0);
    edited.cutting_data.clear();
    lib.upsert(edited).unwrap();
    lib.remove(second);
    save_library(&lib, &dir.join("library.toml")).unwrap();

    // The project is untouched, and the differences are reported, not applied.
    let loaded = file::load(&folder).unwrap();
    assert_eq!(loaded, project);
    let statuses: Vec<SnapshotStatus> = loaded
        .tools()
        .iter()
        .map(|t| compare_snapshot(t, &lib))
        .collect();
    assert_eq!(
        statuses,
        [
            SnapshotStatus::Differs(vec!["cutting_data".into(), "tool.diameter".into()]),
            SnapshotStatus::NotInLibrary,
        ]
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn req_tl_04_req_tl_05_req_tl_06_snapshot_file_holds_every_value() {
    let (project, _) = project_from_library();
    let id = ToolId::from_u128(0x0d6b8a5e_3c1f_4e2a_9b7d_000000000001);
    let files = file::render(&project).unwrap();
    let text = &files[&format!("tools/{id}.toml")];
    for line in [
        "schema = 4",
        "name = \"6 mm flat\"",
        "overall_length = 50.0",
        "flute_count = 2",
        "material = \"carbide\"",
        "[controller]\nnumber = 1",
        "[cutting_data.oak]",
        "[cutting_data.\"6061 aluminium\"]",
        "spindle_speed = 18000.0",
    ] {
        assert!(text.contains(line), "{line} missing from:\n{text}");
    }
}

#[test]
fn req_prj_04_old_tool_snapshot_is_refused_with_what_to_do() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/schema-3-tool");
    let err = file::load(&fixture).unwrap_err().to_string();
    for part in [
        "6 mm ball",
        "schema 3",
        "overall_length",
        "flute_count",
        "[controller]",
    ] {
        assert!(err.contains(part), "{part}: {err}");
    }
}
