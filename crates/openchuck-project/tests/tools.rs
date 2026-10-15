// SPDX-License-Identifier: AGPL-3.0-or-later
//! The tool library corpus and project tool snapshots (slice 4; TL-01, TL-03,
//! TL-07, TL-11, NFR-07; decision record 0014).
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};

use openchuck_project::id::ToolId;
use openchuck_project::tools::{
    SnapshotStatus, ToolLibrary, compare_snapshot, load_profile, parse_tool_file, parse_tool_set,
};
use openchuck_project::{Item, Placement, Project, file};
use openchuck_types::tool::{ControllerMapping, ToolShape};
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

/// A copy of the sample library folder, so tests that write never touch the corpus.
fn library_copy(name: &str) -> PathBuf {
    let dir = temp_dir(name).join("library");
    std::fs::create_dir_all(dir.join("tool-sets")).unwrap();
    for sub in ["", "tool-sets"] {
        for entry in std::fs::read_dir(corpus().join("library").join(sub)).unwrap() {
            let path = entry.unwrap().path();
            if path.is_file() {
                std::fs::copy(&path, dir.join(sub).join(path.file_name().unwrap())).unwrap();
            }
        }
    }
    dir
}

#[test]
fn req_tl_01_req_tl_02_req_tl_03_sample_library_reads_every_type() {
    let lib = ToolLibrary::open(&corpus().join("library")).unwrap();
    assert!(lib.problems().is_empty(), "{:?}", lib.problems());
    let mut types: Vec<&str> = lib.tools().map(|t| t.tool.shape.type_name()).collect();
    types.sort_unstable();
    assert_eq!(
        types,
        [
            "ball_end",
            "bull_nose",
            "chamfer",
            "custom",
            "drill",
            "flat_end",
            "tapered_ball",
            "v_bit"
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
    let set = lib.tool_set("Makera Z1").unwrap();
    assert_eq!(set.entries.len(), 6);
    assert_eq!(
        set.mapping_for(ToolId::from_u128(0x0d6b8a5e_3c1f_4e2a_9b7d_000000000003)),
        Some(ControllerMapping {
            number: 3,
            length_offset: Some(13),
            diameter_offset: None
        })
    );
}

#[test]
fn req_tl_01_req_tl_11_req_nfr_07_damaged_files_give_clear_errors() {
    let dir = corpus().join("broken");
    let cases = [
        ("corner_radius.toml", "corner_radius"),
        ("undercut_profile.toml", "segment 2"),
        ("truncated.toml", "truncated.toml"),
        ("tool_number.toml", "controller"),
        ("stepover.toml", "stepover"),
    ];
    for (name, expected) in cases {
        let path = dir.join(name);
        let text = std::fs::read_to_string(&path).unwrap();
        let err = parse_tool_file(&path, &text).unwrap_err().to_string();
        assert!(err.contains(expected), "{name}: {err}");
    }
    for (name, expected) in [
        ("tool_set_duplicate_number.toml", "listed twice"),
        ("tool_set_number_zero.toml", "at least 1"),
    ] {
        let path = dir.join(name);
        let text = std::fs::read_to_string(&path).unwrap();
        let err = parse_tool_set(&path, &text).unwrap_err().to_string();
        assert!(
            err.contains(expected) && err.contains(name),
            "{name}: {err}"
        );
    }
    // In a library folder a broken file is skipped and reported; the rest read.
    let lib = library_copy("broken-in-folder");
    std::fs::copy(dir.join("corner_radius.toml"), lib.join("bad.toml")).unwrap();
    let opened = ToolLibrary::open(&lib).unwrap();
    assert_eq!(opened.tools().count(), 8);
    assert!(
        opened.problems()[0].contains("bad.toml"),
        "{:?}",
        opened.problems()
    );
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
                "broken_undercut.toml" | "broken_half_circle.toml" => "segment 2",
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
    let lib = ToolLibrary::open(&corpus().join("library")).unwrap();
    let mut custom = lib.find("6 mm flat").unwrap().tool.clone();
    custom.shape = ToolShape::Custom { profile: typed };
    let mut ball = custom.clone();
    ball.shape = ToolShape::BallEnd;
    let (a, b) = (custom.profile().unwrap(), ball.profile().unwrap());
    for i in 0..=30 {
        let r = f64::from(i) * 0.1;
        assert!((a.height_at(r).unwrap() - b.height_at(r).unwrap()).abs() < 1e-9);
    }
}

/// A project using two tools from a copy of the sample library.
fn project_from_library(name: &str) -> (Project, ToolLibrary) {
    let lib = ToolLibrary::open(&library_copy(name)).unwrap();
    let mut project = Project::new("tools");
    for (n, tool) in ["6 mm flat", "3 mm ball"].into_iter().enumerate() {
        let t = lib.find(tool).unwrap();
        let number = u32::try_from(n).unwrap() + 1;
        project
            .insert(
                Item::Tool(t.snapshot(ControllerMapping::numbered(number))),
                Placement::End,
            )
            .unwrap();
    }
    (project, lib)
}

#[test]
fn req_tl_07_project_opens_with_the_library_absent() {
    let (project, lib) = project_from_library("absent");
    let folder = lib.dir().parent().unwrap().join("project");
    file::save(&project, &folder).unwrap();
    let files = file::render(&project).unwrap();

    // Without the library the project loads the same and writes the same bytes.
    std::fs::remove_dir_all(lib.dir()).unwrap();
    let loaded = file::load(&folder).unwrap();
    assert_eq!(loaded, project);
    assert_eq!(file::render(&loaded).unwrap(), files);
    std::fs::remove_dir_all(folder.parent().unwrap()).ok();
}

#[test]
fn req_tl_07_editing_the_library_never_changes_a_project() {
    let (project, mut lib) = project_from_library("edit");
    let folder = lib.dir().parent().unwrap().join("project");
    file::save(&project, &folder).unwrap();

    // Change the library's copy of the first tool and remove the second.
    let first = lib.find("6 mm flat").unwrap().id;
    let second = lib.find("3 mm ball").unwrap().id;
    let mut edited = lib.get(first).unwrap().clone();
    edited.tool.diameter = Millimetres(5.0);
    edited.cutting_data.clear();
    lib.save_tool(edited).unwrap();
    lib.remove_tool(second).unwrap();

    // The project is untouched, and the differences are reported, not applied.
    let lib = ToolLibrary::open(lib.dir()).unwrap();
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
    std::fs::remove_dir_all(folder.parent().unwrap()).ok();
}

#[test]
fn req_tl_04_req_tl_05_req_tl_06_snapshot_file_holds_every_value() {
    let (project, lib) = project_from_library("values");
    let id = ToolId::from_u128(0x0d6b8a5e_3c1f_4e2a_9b7d_000000000001);
    let files = file::render(&project).unwrap();
    let text = &files[&format!("tools/{id}.toml")];
    for line in [
        "schema = 8",
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
    std::fs::remove_dir_all(lib.dir().parent().unwrap()).ok();
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

#[test]
fn req_tl_04_centre_cutting_is_read_from_a_tool_file_and_defaults_to_the_shape() {
    let text = std::fs::read_to_string(corpus().join("library/6-mm-flat.toml")).unwrap();
    let plain = parse_tool_file(Path::new("flat.toml"), &text).unwrap();
    assert_eq!(plain.tool.centre_cutting, None);
    assert!(plain.tool.can_plunge());

    let stated = text.replace(
        "material = \"carbide\"\n",
        "material = \"carbide\"\ncentre_cutting = false\n",
    );
    let tool = parse_tool_file(Path::new("flat.toml"), &stated)
        .unwrap()
        .tool;
    assert_eq!(tool.centre_cutting, Some(false));
    assert!(!tool.can_plunge());
}
