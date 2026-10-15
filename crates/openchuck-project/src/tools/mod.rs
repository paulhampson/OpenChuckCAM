// SPDX-License-Identifier: AGPL-3.0-or-later
//! The tool library, stored separately from projects (TL-01), machine tool
//! sets (TL-11), custom profile text files (TL-03) and the comparison of a
//! project's tool snapshots with a library (TL-07). Decision records 0012 and
//! 0014.
//!
//! A library is a folder with one TOML file per tool, and the default tool
//! sets of machines in `tool-sets/`:
//!
//! ```text
//! my-tools/
//!   6-mm-flat.toml
//!   3-mm-ball.toml
//!   tool-sets/
//!     makera-z1.toml
//! ```
//!
//! A tool file holds the tool's geometry and cutting data. Tool numbers are
//! not in it: they belong to the project, and a machine's tool set gives the
//! default number for each tool it holds.
//!
//! ```toml
//! schema = 1
//! id = "6f1c2e4a-0b7d-4c3e-9a51-2d8f0e6b7c19"
//! name = "6 mm flat"
//!
//! [tool]
//! diameter = 6.0
//! flute_length = 20.0
//! overall_length = 50.0
//! shank_diameter = 6.0
//! flute_count = 2
//! material = "carbide"
//!
//! [tool.shape]
//! type = "flat_end"
//!
//! [cutting_data.oak]
//! spindle_speed = 18000.0
//! feed_per_tooth = 0.03
//! plunge_feed = 300.0
//! stepover = 2.4
//! stepdown = 1.5
//! ```
//!
//! A tool set names its machine profile and lists tool numbers:
//!
//! ```toml
//! schema = 1
//! machine = "Makera Z1"
//!
//! [[tools]]
//! number = 1
//! tool = "6f1c2e4a-0b7d-4c3e-9a51-2d8f0e6b7c19"
//! ```
//!
//! The file name is only a label: the ID inside identifies the tool, and a
//! project snapshot keeps that ID, so a snapshot and its library tool can be
//! matched later. Import copies tool files from another library folder or a
//! single tool file; export writes chosen tools into a folder.
//!
//! A custom profile file holds the [`ToolProfile`](openchuck_types::tool::ToolProfile)
//! fields. An arc gives its `centre`, or its `radius` and whether it is
//! `convex` (bulging away from the tool, like a ball end) or `concave` (like a
//! corner-rounding cutter):
//!
//! ```toml
//! schema = 1
//!
//! [[segments]]
//! type = "line"
//! to = { radius = 1.0, height = 0.0 }
//!
//! [[segments]]
//! type = "arc"
//! to = { radius = 3.0, height = 2.0 }
//! radius = 2.0
//! curve = "concave"
//! ```

mod library;
mod profile_file;

pub use library::{
    ImportReport, LIBRARY_SCHEMA, LibraryTool, SnapshotStatus, TOOL_SETS_FOLDER, ToolLibrary,
    ToolSet, ToolSetEntry, compare_snapshot, file_stem_for, parse_tool_file, parse_tool_set,
    render_tool_file, render_tool_set,
};
pub use profile_file::{ArcCurve, PROFILE_SCHEMA, load_profile, parse_profile, render_profile};

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

    use std::path::{Path, PathBuf};

    use super::*;
    use crate::id::ToolId;
    use openchuck_types::tool::{
        ControllerMapping, CuttingData, CuttingTable, ProfilePoint, ProfileSegment, Tool, ToolShape,
    };
    use openchuck_types::units::{Degrees, FeedRate, Millimetres, SpindleSpeed};

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("openchuck-lib-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn flat(id: u128) -> LibraryTool {
        LibraryTool {
            id: ToolId::from_u128(id),
            name: "6 mm flat".into(),
            tool: Tool {
                shape: ToolShape::FlatEnd,
                diameter: Millimetres(6.0),
                flute_length: Millimetres(20.0),
                overall_length: Millimetres(50.0),
                shank_diameter: Millimetres(6.0),
                flute_count: 2,
                material: Some("carbide".into()),
                centre_cutting: None,
            },
            cutting_data: CuttingTable::from([(
                "oak".to_owned(),
                CuttingData {
                    spindle_speed: SpindleSpeed(18000.0),
                    feed_per_tooth: Millimetres(0.03),
                    plunge_feed: FeedRate(300.0),
                    stepover: Millimetres(2.4),
                    stepdown: Millimetres(1.5),
                },
            )]),
        }
    }

    fn vbit(id: u128) -> LibraryTool {
        LibraryTool {
            id: ToolId::from_u128(id),
            name: "60 degree V".into(),
            tool: Tool {
                shape: ToolShape::VBit {
                    included_angle: Degrees(60.0),
                    tip_diameter: Millimetres(0.2),
                },
                material: None,
                centre_cutting: None,
                ..flat(0).tool
            },
            cutting_data: CuttingTable::new(),
        }
    }

    #[test]
    fn req_tl_01_tool_file_round_trips_byte_for_byte() {
        let text = render_tool_file(&flat(1)).unwrap();
        assert!(text.starts_with("schema = 1\n"), "{text}");
        assert!(!text.contains("number"), "{text}");
        let back = parse_tool_file(Path::new("t.toml"), &text).unwrap();
        assert_eq!(back, flat(1));
        assert_eq!(render_tool_file(&back).unwrap(), text);
    }

    #[test]
    fn req_tl_01_library_folder_holds_one_file_per_tool() {
        let dir = temp_dir("folder");
        let mut lib = ToolLibrary::open(&dir).unwrap();
        assert_eq!(lib.tools().count(), 0);
        let a = lib.save_tool(flat(1)).unwrap();
        let b = lib.save_tool(vbit(2)).unwrap();
        // Same name, different tool: a second file, not an overwrite.
        let c = lib
            .save_tool(LibraryTool {
                id: ToolId::from_u128(3),
                ..flat(1)
            })
            .unwrap();
        assert!(
            a.ends_with("6-mm-flat.toml") && c.ends_with("6-mm-flat-2.toml"),
            "{c:?}"
        );
        assert!(b.ends_with("60-degree-v.toml"));

        // A one-value edit rewrites one line of one file.
        let before_b = std::fs::read_to_string(&b).unwrap();
        let before = std::fs::read_to_string(&a).unwrap();
        let mut edited = flat(1);
        edited.tool.flute_length = Millimetres(22.0);
        assert_eq!(lib.save_tool(edited).unwrap(), a);
        let after = std::fs::read_to_string(&a).unwrap();
        let changed: Vec<_> = before
            .lines()
            .zip(after.lines())
            .filter(|(x, y)| x != y)
            .collect();
        assert_eq!(
            changed,
            vec![("flute_length = 20.0", "flute_length = 22.0")]
        );
        assert_eq!(std::fs::read_to_string(&b).unwrap(), before_b);

        // Reopened, the folder gives the same library.
        let reopened = ToolLibrary::open(&dir).unwrap();
        assert_eq!(reopened, lib);
        assert!(reopened.problems().is_empty());
        lib.remove_tool(ToolId::from_u128(3)).unwrap().unwrap();
        assert!(!c.exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn req_tl_01_req_nfr_07_broken_files_are_skipped_and_reported() {
        let dir = temp_dir("broken");
        let mut lib = ToolLibrary::open(&dir).unwrap();
        let a = lib.save_tool(flat(1)).unwrap();
        std::fs::write(dir.join("broken.toml"), "schema = 1\nid = 3\n").unwrap();
        std::fs::copy(&a, dir.join("zz-copy.toml")).unwrap();
        std::fs::write(dir.join("notes.txt"), "ignored").unwrap();
        let lib = ToolLibrary::open(&dir).unwrap();
        assert_eq!(lib.tools().count(), 1);
        assert_eq!(lib.problems().len(), 2, "{:?}", lib.problems());
        assert!(lib.problems()[0].contains("broken.toml"));
        assert!(lib.problems()[1].contains("already holds"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn req_tl_01_export_then_import_is_lossless() {
        let (src, out, dst) = (temp_dir("src"), temp_dir("out"), temp_dir("dst"));
        let mut lib = ToolLibrary::open(&src).unwrap();
        lib.save_tool(flat(1)).unwrap();
        lib.save_tool(vbit(2)).unwrap();
        let written = lib.export(&[ToolId::from_u128(2)], &out).unwrap();
        assert_eq!(written.len(), 1);
        assert_eq!(
            std::fs::read_to_string(&written[0]).unwrap(),
            std::fs::read_to_string(lib.path_of(ToolId::from_u128(2)).unwrap()).unwrap()
        );
        let mut other = ToolLibrary::open(&dst).unwrap();
        other.save_tool(flat(9)).unwrap();
        let report = other.import(&out).unwrap();
        assert_eq!(report.added, vec![ToolId::from_u128(2)]);
        assert_eq!(
            other.get(ToolId::from_u128(2)),
            lib.get(ToolId::from_u128(2))
        );
        // Again changes nothing; a changed tool file replaces in place.
        assert_eq!(
            other.import(&out).unwrap().unchanged,
            vec![ToolId::from_u128(2)]
        );
        let mut renamed = vbit(2);
        renamed.name = "V renamed".into();
        let single = out.join("renamed.toml");
        std::fs::write(&single, render_tool_file(&renamed).unwrap()).unwrap();
        assert_eq!(
            other.import(&single).unwrap().replaced,
            vec![ToolId::from_u128(2)]
        );
        assert_eq!(other.get(ToolId::from_u128(2)).unwrap().name, "V renamed");
        assert!(lib.export(&[ToolId::from_u128(77)], &out).is_err());
        for d in [src, out, dst] {
            std::fs::remove_dir_all(d).ok();
        }
    }

    #[test]
    fn req_nfr_07_tool_set_with_marker_like_machine_name_round_trips() {
        // The input the nightly fuzzer found (tool_library_toml): a machine
        // name with control characters and a line starting `|||||||`.
        let path = Path::new("fuzz.toml");
        let text = "schema = 1\nmachine = \"av M\\n|||||||ee\\ema3\"\n";
        let set = parse_tool_set(path, text).unwrap();
        let written = render_tool_set(&set).unwrap();
        assert_eq!(parse_tool_set(path, &written).unwrap(), set);
    }

    #[test]
    fn req_tl_11_tool_set_file_round_trips_and_validates() {
        let dir = temp_dir("sets");
        let mut lib = ToolLibrary::open(&dir).unwrap();
        lib.save_tool(flat(1)).unwrap();
        lib.save_tool(vbit(2)).unwrap();
        let mut set = ToolSet {
            machine: "Makera Z1".into(),
            entries: Vec::new(),
        };
        set.assign(ToolId::from_u128(2), ControllerMapping::numbered(5));
        set.assign(ToolId::from_u128(1), ControllerMapping::numbered(1));
        let path = lib.save_tool_set(set.clone()).unwrap();
        assert!(path.ends_with("tool-sets/makera-z1.toml"), "{path:?}");
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("machine = \"Makera Z1\""), "{text}");
        assert_eq!(parse_tool_set(&path, &text).unwrap(), set);
        let lib = ToolLibrary::open(&dir).unwrap();
        let found = lib.tool_set("makera z1").unwrap();
        assert_eq!(found.entries[0].controller.number, 1);
        assert_eq!(
            found.mapping_for(ToolId::from_u128(2)),
            Some(ControllerMapping::numbered(5))
        );
        assert_eq!(found.tool_for(1), Some(ToolId::from_u128(1)));

        // Reassigning a number moves it.
        let mut moved = found.clone();
        moved.assign(ToolId::from_u128(2), ControllerMapping::numbered(1));
        assert_eq!(moved.entries.len(), 1);
        assert!(moved.unassign(1) && !moved.unassign(1));

        // A tool missing from the library is refused when saved and reported when read.
        let mut lib = lib;
        let mut stray = set.clone();
        stray.assign(ToolId::from_u128(77), ControllerMapping::numbered(9));
        assert!(lib.save_tool_set(stray.clone()).is_err());
        std::fs::write(
            &path,
            text.replace(
                &ToolId::from_u128(2).to_string(),
                &ToolId::from_u128(77).to_string(),
            ),
        )
        .unwrap();
        let lib = ToolLibrary::open(&dir).unwrap();
        assert!(
            lib.problems()[0].contains("not in the library"),
            "{:?}",
            lib.problems()
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn req_tl_11_req_nfr_07_bad_tool_sets_give_clear_errors() {
        let one = ToolId::from_u128(1);
        let two = ToolId::from_u128(2);
        let line = |n: u32, id: ToolId| format!("[[tools]]\nnumber = {n}\ntool = \"{id}\"\n");
        let cases = [
            (
                format!(
                    "schema = 1\nmachine = \"Z1\"\n{}{}",
                    line(1, one),
                    line(1, two)
                ),
                "listed twice",
            ),
            (
                format!(
                    "schema = 1\nmachine = \"Z1\"\n{}{}",
                    line(1, one),
                    line(2, one)
                ),
                "listed twice",
            ),
            (
                format!("schema = 1\nmachine = \"Z1\"\n{}", line(0, one)),
                "at least 1",
            ),
            (
                format!("schema = 1\nmachine = \" \"\n{}", line(1, one)),
                "machine",
            ),
            ("schema = 1\n".to_owned(), "machine"),
        ];
        for (text, expected) in cases {
            let err = parse_tool_set(Path::new("set.toml"), &text)
                .unwrap_err()
                .to_string();
            assert!(
                err.contains(expected) && err.contains("set.toml"),
                "{expected}: {err}"
            );
        }
    }

    #[test]
    fn req_nfr_07_bad_tool_files_give_clear_errors() {
        let text = render_tool_file(&flat(1)).unwrap();
        let cases = [
            ("schema = 2\n", "newer"),
            ("not toml [", "t.toml"),
            (
                &text.replace("diameter = 6.0\nflute", "diameter = -6.0\nflute"),
                "tool.diameter",
            ),
            (
                &text.replace("stepover = 2.4", "stepover = 9.0"),
                "stepover",
            ),
            (&text.replace("flute_count = 2", "flutes = 2"), "flutes"),
            (&text.replace("name = \"6 mm flat\"", "name = \"\""), "name"),
            (&format!("{text}\n[controller]\nnumber = 3\n"), "controller"),
        ];
        for (text, expected) in cases {
            let err = parse_tool_file(Path::new("t.toml"), text)
                .unwrap_err()
                .to_string();
            assert!(err.contains(expected), "{expected}: {err}");
        }
    }

    #[test]
    fn req_tl_01_file_stems_follow_the_name() {
        assert_eq!(file_stem_for("6 mm Flat (2F)"), "6-mm-flat-2f");
        assert_eq!(file_stem_for("  ++ "), "tool");
        assert_eq!(file_stem_for("Ø3 ball"), "3-ball");
    }

    #[test]
    fn req_tl_03_profile_file_reads_arcs_by_centre_or_radius() {
        let text = "schema = 1\n\n[[segments]]\ntype = \"line\"\nto = { radius = 1.0, height = 0.0 }\n\n\
                    [[segments]]\ntype = \"arc\"\nto = { radius = 3.0, height = 2.0 }\nradius = 2.0\ncurve = \"concave\"\n\n\
                    [[segments]]\ntype = \"line\"\nto = { radius = 3.0, height = 12.0 }\n";
        let profile = parse_profile(Path::new("p.toml"), text).unwrap();
        let ProfileSegment::Arc { to, centre } = profile.segments[1] else {
            panic!("{profile:?}");
        };
        assert_eq!(to, ProfilePoint::new(3.0, 2.0));
        assert!(
            centre.distance(ProfilePoint::new(3.0, 0.0)) < 1e-12,
            "{centre:?}"
        );
        let written = render_profile(&profile).unwrap();
        assert!(written.contains("centre"), "{written}");
        assert_eq!(
            parse_profile(Path::new("p.toml"), &written).unwrap(),
            profile
        );
        assert_eq!(render_profile(&profile).unwrap(), written);
    }

    #[test]
    fn req_tl_03_req_nfr_07_bad_profile_files_name_the_segment() {
        let line = |r: f64, z: f64| {
            format!("[[segments]]\ntype = \"line\"\nto = {{ radius = {r:?}, height = {z:?} }}\n")
        };
        let cases = [
            (
                format!(
                    "schema = 1\n{}{}{}",
                    line(3.0, 0.0),
                    line(3.0, 5.0),
                    line(2.0, 9.0)
                ),
                "segment 3",
            ),
            (
                format!(
                    "schema = 1\n{}[[segments]]\ntype = \"arc\"\nto = {{ radius = 3.0, height = 2.0 }}\nradius = 2.0\n",
                    line(1.0, 0.0)
                ),
                "segment 2: an arc gives",
            ),
            (
                format!(
                    "schema = 1\n{}[[segments]]\ntype = \"arc\"\nto = {{ radius = 9.0, height = 9.0 }}\nradius = 1.0\ncurve = \"convex\"\n",
                    line(1.0, 0.0)
                ),
                "segment 2: the arc's ends",
            ),
            ("schema = 1\n".to_owned(), "at least one segment"),
            (
                "schema = 1\n[[segments]]\ntype = \"spline\"\n".to_owned(),
                "spline",
            ),
        ];
        for (text, expected) in cases {
            let err = parse_profile(Path::new("p.toml"), &text)
                .unwrap_err()
                .to_string();
            assert!(
                err.contains(expected) && err.contains("p.toml"),
                "{expected}: {err}"
            );
        }
    }

    #[test]
    fn req_tl_07_snapshot_comparison_names_the_changed_fields() {
        let dir = temp_dir("compare");
        let mut lib = ToolLibrary::open(&dir).unwrap();
        lib.save_tool(flat(1)).unwrap();
        let snapshot = flat(1).snapshot(ControllerMapping::numbered(4));
        assert_eq!(compare_snapshot(&snapshot, &lib), SnapshotStatus::Same);
        let mut edited = snapshot.clone();
        edited.tool.diameter = Millimetres(5.0);
        edited.controller.number = 7; // project-only: not a difference
        edited.cutting_data.clear();
        assert_eq!(
            compare_snapshot(&edited, &lib),
            SnapshotStatus::Differs(vec!["cutting_data".into(), "tool.diameter".into()])
        );
        let mut reshaped = snapshot.clone();
        reshaped.tool.shape = ToolShape::BallEnd;
        assert_eq!(
            compare_snapshot(&reshaped, &lib),
            SnapshotStatus::Differs(vec!["tool.shape".into()])
        );
        lib.remove_tool(ToolId::from_u128(1)).unwrap();
        assert_eq!(
            compare_snapshot(&snapshot, &lib),
            SnapshotStatus::NotInLibrary
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    proptest::proptest! {
        #[test]
        fn req_nfr_07_tool_file_parser_never_panics_on_damaged_files(
            cut in 0usize..1200,
            insert in ".{0,12}",
        ) {
            let text = render_tool_file(&flat(1)).unwrap();
            let mut cut = cut.min(text.len());
            while !text.is_char_boundary(cut) {
                cut -= 1;
            }
            let damaged = format!("{}{insert}{}", &text[..cut], &text[cut..]);
            if let Ok(tool) = parse_tool_file(Path::new("fuzz.toml"), &damaged) {
                let written = render_tool_file(&tool).unwrap();
                proptest::prop_assert_eq!(parse_tool_file(Path::new("x.toml"), &written).unwrap(), tool);
            }
        }

        #[test]
        fn req_nfr_07_tool_set_parser_never_panics_on_text(text in ".{0,300}") {
            let _ = parse_tool_set(Path::new("fuzz.toml"), &text);
        }

        #[test]
        fn req_nfr_07_profile_parser_never_panics_on_text(text in ".{0,400}") {
            let _ = parse_profile(Path::new("fuzz.toml"), &text);
        }
    }
}
