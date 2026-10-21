// SPDX-License-Identifier: AGPL-3.0-or-later
//! The tool list's machine sections, type groups, filter and remembered
//! folding, and the read-only view of a shipped default tool (TL-02, TL-03,
//! TL-04, TL-11, TL-12, UI-15; specification 12.1 and 13; decision record
//! 0199; trial feedback F-36, F-37, F-38, issues 192, 193, 194).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::path::PathBuf;

use openchuck_app::project::id::ToolId;
use openchuck_app::settings::{SettingsFile, UserSettings};
use openchuck_app::tool_edit;
use openchuck_app::tool_library::{
    ListGroup, ListQuery, SizeFilter, ToolFilter, ToolLibraryView, ToolType, default_dimensions,
    duplicate_draft, edit_name, is_shipped_default, new_draft, save_draft, tool_list,
};
use openchuck_app::tools::{ControllerMapping, ToolLibrary, ToolSet, ToolSetEntry};

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("openchuck-toolsec-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// A saved tool of `kind` named `name` with the given sizes.
fn saved(lib: &mut ToolLibrary, kind: ToolType, name: &str, diameter: f64, flute: f64) -> ToolId {
    let mut draft = new_draft(lib, kind).unwrap();
    edit_name(&mut draft, name).unwrap();
    draft.tool.diameter = openchuck_app::tools::Millimetres(diameter);
    draft.tool.flute_length = openchuck_app::tools::Millimetres(flute);
    draft.tool.overall_length = openchuck_app::tools::Millimetres(flute + 30.0);
    draft.tool.shank_diameter = openchuck_app::tools::Millimetres(diameter.max(3.0));
    save_draft(lib, &draft, &[]).unwrap();
    draft.id
}

fn set_for(lib: &mut ToolLibrary, machine: &str, tools: &[ToolId]) {
    let entries = tools
        .iter()
        .zip(1u32..)
        .map(|(tool, number)| ToolSetEntry {
            tool: *tool,
            controller: ControllerMapping {
                number,
                length_offset: None,
                diameter_offset: None,
            },
        })
        .collect();
    lib.save_tool_set(ToolSet {
        machine: machine.into(),
        entries,
    })
    .unwrap();
}

fn names(section: &openchuck_app::tool_library::ListSection) -> Vec<String> {
    section
        .groups
        .iter()
        .flat_map(|g| g.rows.iter().map(|r| r.name.clone()))
        .collect()
}

#[test]
fn req_tl_11_tools_of_a_machine_sit_under_its_section_and_the_rest_under_standard_tools() {
    let dir = temp_dir("sections");
    let mut lib = ToolLibrary::open(&dir).unwrap();
    let z1_ball = saved(&mut lib, ToolType::BallEnd, "Z1 ball", 3.0, 8.0);
    let z1_flat = saved(&mut lib, ToolType::FlatEnd, "Z1 flat", 6.0, 20.0);
    let other = saved(&mut lib, ToolType::FlatEnd, "Other flat", 4.0, 12.0);
    let both = saved(&mut lib, ToolType::VBit, "In both", 6.0, 10.0);
    saved(&mut lib, ToolType::Drill, "Loose drill", 3.0, 20.0);
    set_for(&mut lib, "Makera Z1", &[z1_ball, z1_flat, both]);
    set_for(&mut lib, "Another machine", &[other, both]);

    let sections = tool_list(&lib, None, &ListQuery::default());
    let titles: Vec<&str> = sections.iter().map(|s| s.title.as_str()).collect();
    // Machines in name order, then the tools no machine holds.
    assert_eq!(titles, ["Another machine", "Makera Z1", "Standard tools"]);
    assert_eq!(names(&sections[0]), ["Other flat", "In both"]);
    assert_eq!(names(&sections[1]), ["Z1 ball", "Z1 flat", "In both"]);
    assert_eq!(names(&sections[2]), ["Loose drill"]);
    // Type groups inside a section, in the board's order.
    let z1_groups: Vec<ListGroup> = sections[1].groups.iter().map(|g| g.group).collect();
    assert_eq!(
        z1_groups,
        [
            ListGroup::BallEnd,
            ListGroup::FlatEnd,
            ListGroup::PointedAndDrill
        ]
    );
    // Section and group keys are stable and distinct, for remembering folds.
    let mut keys: Vec<String> = Vec::new();
    for s in &sections {
        keys.push(s.key.clone());
        keys.extend(s.groups.iter().map(|g| g.key.clone()));
    }
    let count = keys.len();
    keys.sort();
    keys.dedup();
    assert_eq!(keys.len(), count);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn req_tl_11_a_machine_without_tools_still_shows_no_empty_section() {
    let dir = temp_dir("empty-machine");
    let mut lib = ToolLibrary::open(&dir).unwrap();
    saved(&mut lib, ToolType::FlatEnd, "Flat", 6.0, 20.0);
    set_for(&mut lib, "Empty machine", &[]);
    let sections = tool_list(&lib, None, &ListQuery::default());
    assert_eq!(sections.len(), 1);
    assert_eq!(sections[0].title, "Standard tools");
    std::fs::remove_dir_all(&dir).ok();
}

fn filter(diameter: (&str, &str, &str), flute: (&str, &str, &str)) -> ToolFilter {
    let size = |(exact, min, max): (&str, &str, &str)| SizeFilter {
        exact: exact.into(),
        min: min.into(),
        max: max.into(),
    };
    ToolFilter {
        diameter: size(diameter),
        flute_length: size(flute),
    }
}

fn filtered_names(lib: &ToolLibrary, f: &ToolFilter) -> Vec<String> {
    let query = ListQuery {
        search: String::new(),
        filter: f.clone(),
    };
    tool_list(lib, None, &query)
        .iter()
        .flat_map(names)
        .collect()
}

#[test]
fn req_tl_04_the_list_filters_by_diameter_and_flute_length_as_a_value_or_a_range() {
    let dir = temp_dir("filter");
    let mut lib = ToolLibrary::open(&dir).unwrap();
    saved(&mut lib, ToolType::FlatEnd, "A", 3.0, 8.0);
    saved(&mut lib, ToolType::BallEnd, "B", 6.0, 20.0);
    saved(&mut lib, ToolType::FlatEnd, "C", 6.0, 12.0);
    saved(&mut lib, ToolType::FlatEnd, "D", 10.0, 25.0);
    let none = ("", "", "");
    let all = filtered_names(&lib, &filter(none, none));
    assert_eq!(all.len(), 4);
    // An exact diameter.
    let six = filtered_names(&lib, &filter(("6", "", ""), none));
    assert_eq!(six.len(), 2);
    assert!(six.contains(&"B".into()) && six.contains(&"C".into()));
    // A diameter range, both ends included.
    let range = filtered_names(&lib, &filter(("", "3", "6"), none));
    assert_eq!(range.len(), 3);
    // Only a minimum, only a maximum.
    assert_eq!(filtered_names(&lib, &filter(("", "6.5", ""), none)), ["D"]);
    assert_eq!(filtered_names(&lib, &filter(("", "", "3"), none)), ["A"]);
    // Flute length the same way, and both filters together.
    assert_eq!(filtered_names(&lib, &filter(none, ("20", "", ""))), ["B"]);
    assert_eq!(
        filtered_names(&lib, &filter(none, ("", "10", "22"))).len(),
        2
    );
    assert_eq!(
        filtered_names(&lib, &filter(("6", "", ""), ("", "", "15"))),
        ["C"]
    );
    // An exact value wins over a range typed beside it.
    assert_eq!(filtered_names(&lib, &filter(("3", "6", "10"), none)), ["A"]);
    // A filter hides the groups and sections it empties.
    let sections = tool_list(
        &lib,
        None,
        &ListQuery {
            search: String::new(),
            filter: filter(("10", "", ""), none),
        },
    );
    assert_eq!(sections.len(), 1);
    assert_eq!(sections[0].groups.len(), 1);
    // Search and filter work together.
    let both = tool_list(
        &lib,
        None,
        &ListQuery {
            search: "ball".into(),
            filter: filter(("6", "", ""), none),
        },
    );
    assert_eq!(both.iter().flat_map(names).collect::<Vec<_>>(), ["B"]);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn req_tl_04_a_filter_that_cannot_be_read_says_what_is_wrong_and_does_not_hide_tools() {
    let none = ("", "", "");
    assert!(filter(none, none).problem().is_none());
    assert!(filter(("6", "", ""), ("", "2", "9")).problem().is_none());
    let text = filter(("six", "", ""), none).problem().unwrap();
    assert!(
        text.contains("Diameter") && text.contains("number"),
        "{text}"
    );
    let text = filter(none, ("", "-1", "")).problem().unwrap();
    assert!(
        text.contains("Flute length") && text.contains("negative"),
        "{text}"
    );
    let text = filter(none, ("", "9", "2")).problem().unwrap();
    assert!(
        text.contains("Flute length") && text.contains("minimum"),
        "{text}"
    );
    assert!(filter(none, none).is_empty());
    assert!(!filter(("1", "", ""), none).is_empty());

    let dir = temp_dir("bad-filter");
    let mut lib = ToolLibrary::open(&dir).unwrap();
    saved(&mut lib, ToolType::FlatEnd, "A", 3.0, 8.0);
    saved(&mut lib, ToolType::FlatEnd, "B", 6.0, 8.0);
    // The unreadable field is left out; the readable one still applies.
    assert_eq!(
        filtered_names(&lib, &filter(("six", "", ""), ("8", "", ""))).len(),
        2
    );
    assert_eq!(
        filtered_names(&lib, &filter(("six", "", ""), ("9", "", ""))).len(),
        0
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn req_ui_15_folded_groups_and_the_filter_are_remembered_per_user() {
    let mut view = ToolLibraryView::default();
    assert!(!view.is_folded("standard"));
    view.set_folded("standard", true);
    view.set_folded("machine:makera z1/ball-end", true);
    view.set_folded("standard", true);
    assert!(view.is_folded("standard") && view.is_folded("machine:makera z1/ball-end"));
    view.set_folded("standard", false);
    assert!(!view.is_folded("standard"));
    view.filter = filter(("6", "", ""), ("", "5", "20"));

    let config = temp_dir("settings");
    let file = SettingsFile::in_folder(&config);
    let settings = UserSettings {
        tool_library: view.clone(),
        ..UserSettings::default()
    };
    file.save(&settings).unwrap();
    let loaded = file.load();
    assert!(loaded.problem.is_none());
    assert_eq!(loaded.settings.tool_library, view);

    // A file written before the table existed reads as the defaults.
    std::fs::write(file.file(), "[interface]\nscale = 1.0\n").unwrap();
    assert_eq!(
        file.load().settings.tool_library,
        ToolLibraryView::default()
    );
    std::fs::remove_dir_all(&config).ok();
}

#[test]
fn req_tl_12_a_shipped_default_tool_is_read_only_until_it_is_copied() {
    let dir = temp_dir("default");
    let mut lib = ToolLibrary::open(&dir).unwrap();
    lib.install_defaults().unwrap();
    let default = lib.tools().next().unwrap().clone();
    assert!(is_shipped_default(&default));
    // A tool of the user's own is not.
    let own = saved(&mut lib, ToolType::FlatEnd, "Mine", 6.0, 20.0);
    assert!(!is_shipped_default(lib.get(own).unwrap()));
    // A default the user changed before is theirs, and stays editable.
    let mut edited = default.clone();
    edit_name(&mut edited, "My edited default").unwrap();
    assert!(!is_shipped_default(&edited));

    // Copy to edit: a new library draft; the default is untouched.
    let copy = duplicate_draft(&lib, default.id).unwrap();
    assert_ne!(copy.id, default.id);
    assert!(!is_shipped_default(&copy));
    assert!(copy.name.starts_with(&default.name));
    assert_eq!(lib.get(default.id), Some(&default));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn req_tl_03_req_tl_04_a_default_tool_shows_the_dimensions_that_make_it_up() {
    let dir = temp_dir("dimensions");
    let mut lib = ToolLibrary::open(&dir).unwrap();
    lib.install_defaults().unwrap();
    let label_set = |tool: &openchuck_app::tools::LibraryTool| -> Vec<String> {
        default_dimensions(&tool.tool)
            .into_iter()
            .map(|(label, _)| label)
            .collect()
    };
    let mut seen = std::collections::BTreeSet::new();
    for tool in lib.tools() {
        let labels = label_set(tool);
        let kind = ToolType::of(&tool.tool.shape);
        // Always the sizes every tool has.
        for always in [
            "Diameter",
            "Flute length",
            "Overall length",
            "Shank diameter",
        ] {
            assert!(labels.contains(&always.to_owned()), "{kind:?}: {labels:?}");
        }
        // The ones that apply to the shape only.
        let has = |l: &str| labels.contains(&l.to_owned());
        assert_eq!(has("Corner radius"), kind == ToolType::BullNose, "{kind:?}");
        assert_eq!(
            has("Taper angle"),
            kind == ToolType::TaperedBall,
            "{kind:?}"
        );
        assert_eq!(
            has("Included angle"),
            matches!(kind, ToolType::VBit | ToolType::Chamfer),
            "{kind:?}"
        );
        // Every value is a number with its unit.
        for (_, value) in default_dimensions(&tool.tool) {
            assert!(value.contains("mm") || value.contains('°'), "{value}");
        }
        // The profile drawing exists for the standard types.
        assert!(tool_edit::starting_profile(&tool.tool).is_some());
        seen.insert(kind);
    }
    assert!(seen.contains(&ToolType::BullNose) || seen.contains(&ToolType::FlatEnd));
    std::fs::remove_dir_all(&dir).ok();
}
