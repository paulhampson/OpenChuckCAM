// SPDX-License-Identifier: AGPL-3.0-or-later
//! The tool library window's view model (specification 12.1; TL-01 to
//! TL-07, TL-11, TL-12; decision record 0127).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::path::PathBuf;

use openchuck_app::Command;
use openchuck_app::project::id::{OperationId, SetupId, ToolId};
use openchuck_app::project::tools::LEDGER_FILE;
use openchuck_app::project::{
    Item, Operation, OrderKey, ParameterValue, Placement, Project, Setup,
};
use openchuck_app::tool_edit::starting_profile;
use openchuck_app::tool_library::{
    ConflictChoice, CopyState, ListGroup, ListQuery, ToolType, add_to_project, apply_import,
    cutting_rows, delete_impact, delete_tool, duplicate_draft, edit_cutting_data, edit_field,
    edit_material, edit_name, edit_profile, edit_type, export_profile, export_tools, is_dirty,
    new_draft, operation_tool, preview_import, project_rows, project_tool_as_edit,
    remove_cutting_data, remove_from_project, save_draft, save_to_library, save_to_library_changes,
    set_operation_tool, set_project_tool_number, set_tool_set_number, snapshot_command, tool_list,
    tool_set_numbers, update_from_library,
};
use openchuck_app::tools::{
    CuttingData, FeedRate, LibraryTool, Millimetres, NumberSource, SpindleSpeed, ToolLibrary,
    ToolSet, ToolShape, load_profile,
};

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("openchuck-toolwin-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn oak() -> CuttingData {
    CuttingData {
        spindle_speed: SpindleSpeed(12000.0),
        feed_per_tooth: Millimetres(0.02),
        plunge_feed: FeedRate(150.0),
        stepover: Millimetres(2.0),
        stepdown: Millimetres(1.0),
    }
}

/// A new tool of `kind` named `name`, saved.
fn saved(lib: &mut ToolLibrary, kind: ToolType, name: &str) -> ToolId {
    let mut draft = new_draft(lib, kind).unwrap();
    edit_name(&mut draft, name).unwrap();
    save_draft(lib, &draft, &[]).unwrap();
    draft.id
}

fn z1_set(lib: &mut ToolLibrary) {
    lib.save_tool_set(ToolSet {
        machine: "Makera Z1".into(),
        entries: Vec::new(),
    })
    .unwrap();
}

fn project_with_operation(tool: Option<ToolId>) -> Project {
    let mut p = Project::new("p");
    p.insert(
        Item::Setup(Setup::new(SetupId::from_u128(1), "s")),
        Placement::End,
    )
    .unwrap();
    p.insert(
        Item::Operation(Operation {
            enabled: true,
            id: OperationId::from_u128(5),
            order: OrderKey::default(),
            name: "finish".into(),
            setup: SetupId::from_u128(1),
            strategy: "helical".into(),
            tool,
            selections: Vec::new(),
            boundary: None,
            avoid: Vec::new(),
            check: Vec::new(),
            restriction_options: Default::default(),
            spindle_speed: None,
            material: None,
            parameters: Default::default(),
        }),
        Placement::End,
    )
    .unwrap();
    p
}

fn add(project: &mut Project, lib: &ToolLibrary, id: ToolId) {
    let (command, _, _) = add_to_project(project, lib, id).unwrap();
    let Command::Add { item, placement } = command else {
        panic!("{command:?}")
    };
    project.insert(item, placement).unwrap();
}

fn apply(project: &mut Project, make: impl FnOnce(&Project) -> Command) {
    let command = make(project);
    let Command::Replace(item) = command else {
        panic!("{command:?}")
    };
    project.replace(item).unwrap();
}

#[test]
fn req_tl_02_req_tl_03_a_new_tool_of_every_type_is_a_draft_until_saved() {
    let dir = temp_dir("new");
    let mut lib = ToolLibrary::open(&dir).unwrap();
    for (saved_so_far, kind) in ToolType::ALL.into_iter().enumerate() {
        let draft = new_draft(&lib, kind).unwrap();
        assert_eq!(ToolType::of(&draft.tool.shape), kind, "{kind:?}");
        assert_eq!(draft.name, format!("New {}", kind.label().to_lowercase()));
        assert!(draft.cutting_data.is_empty() && draft.tool.material.is_none());
        // Nothing is written until Save (decision 0127 item 4).
        assert!(is_dirty(&lib, &draft));
        assert!(lib.get(draft.id).is_none());
        assert_eq!(
            std::fs::read_dir(&dir).map_or(0, Iterator::count),
            saved_so_far
        );
        save_draft(&mut lib, &draft, &[]).unwrap();
        assert!(!is_dirty(&lib, &draft));
        assert!(lib.path_of(draft.id).unwrap().exists());
    }
    // A second new flat end mill is numbered.
    assert_eq!(
        new_draft(&lib, ToolType::FlatEnd).unwrap().name,
        "New flat end mill 2"
    );
    // Reopened, the folder gives the same tools.
    assert_eq!(ToolLibrary::open(&dir).unwrap(), lib);
    let sections = tool_list(&lib, None, &ListQuery::default());
    assert_eq!(sections.len(), 1);
    assert_eq!(sections[0].title, "Standard tools");
    let groups: Vec<ListGroup> = sections[0].groups.iter().map(|g| g.group).collect();
    assert_eq!(
        groups,
        [
            ListGroup::BallEnd,
            ListGroup::FlatEnd,
            ListGroup::BullNoseAndTapered,
            ListGroup::PointedAndDrill,
            ListGroup::Custom
        ]
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn req_tl_01_search_matches_name_type_and_material_and_marks_project_tools() {
    let dir = temp_dir("search");
    let mut lib = ToolLibrary::open(&dir).unwrap();
    let ball = saved(&mut lib, ToolType::BallEnd, "Ball 3 mm");
    saved(&mut lib, ToolType::FlatEnd, "Flat 10 mm");
    let mut flat2 = new_draft(&lib, ToolType::FlatEnd).unwrap();
    edit_name(&mut flat2, "Flat 2 mm").unwrap();
    edit_material(&mut flat2, " HSS ").unwrap();
    assert_eq!(flat2.tool.material.as_deref(), Some("HSS"));
    save_draft(&mut lib, &flat2, &[]).unwrap();

    let mut project = Project::new("p");
    add(&mut project, &lib, ball);
    let all = tool_list(&lib, Some(&project), &ListQuery::default());
    let groups = &all[0].groups;
    assert!(groups[0].rows[0].in_project);
    // Names sort by their numbers: 2 before 10.
    let flats: Vec<&str> = groups[1].rows.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(flats, ["Flat 2 mm", "Flat 10 mm"]);
    assert!(groups[1].rows.iter().all(|r| !r.in_project));
    assert!(groups[1].rows[0].summary.starts_with("Flat end mill · Ø"));
    let find = |text: &str| tool_list(&lib, None, &ListQuery::searching(text));
    assert_eq!(find("BALL END")[0].groups.len(), 1);
    assert_eq!(find("hss")[0].groups[0].rows[0].id, flat2.id);
    assert!(find("nothing like it").is_empty());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn req_tl_04_edits_wait_for_save_and_refusals_change_nothing() {
    let dir = temp_dir("form");
    let mut lib = ToolLibrary::open(&dir).unwrap();
    let id = saved(&mut lib, ToolType::FlatEnd, "6 mm flat");
    let path = lib.path_of(id).unwrap().to_path_buf();
    let before = std::fs::read_to_string(&path).unwrap();
    let mut draft = lib.get(id).unwrap().clone();
    edit_field(&mut draft, "flute_length", &ParameterValue::Number(14.0)).unwrap();
    // The edit is in the draft only.
    assert!(is_dirty(&lib, &draft));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), before);
    // Refused edits leave the draft as it was.
    let kept = draft.clone();
    assert!(edit_field(&mut draft, "diameter", &ParameterValue::Number(-1.0)).is_err());
    let err = edit_name(&mut draft, "  ").unwrap_err().to_string();
    assert!(err.contains("name"), "{err}");
    assert_eq!(draft, kept);
    // Save writes one line of one file.
    save_draft(&mut lib, &draft, &[]).unwrap();
    let after = std::fs::read_to_string(&path).unwrap();
    assert!(after.contains("flute_length = 14.0"), "{after}");
    assert!(!is_dirty(&lib, &draft));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn req_tl_02_req_tl_03_changing_the_type_keeps_the_sizes_or_names_the_problem() {
    let lib = ToolLibrary::open(&temp_dir("type")).unwrap();
    let mut tool = new_draft(&lib, ToolType::FlatEnd).unwrap();
    let diameter = tool.tool.diameter;
    edit_type(&mut tool, ToolType::BullNose).unwrap();
    assert!(matches!(tool.tool.shape, ToolShape::BullNose { .. }));
    assert_eq!(tool.tool.diameter, diameter);
    // To a custom profile: the outline of the tool as it was.
    edit_type(&mut tool, ToolType::Custom).unwrap();
    assert!(matches!(tool.tool.shape, ToolShape::Custom { .. }));
    assert_eq!(tool.tool.diameter, diameter);
    edit_type(&mut tool, ToolType::FlatEnd).unwrap();
    // A V-bit whose cone cannot reach the diameter within the flutes.
    edit_field(&mut tool, "flute_length", &ParameterValue::Number(0.5)).unwrap();
    assert!(edit_type(&mut tool, ToolType::VBit).is_err());
    assert!(matches!(tool.tool.shape, ToolShape::FlatEnd));
}

#[test]
fn req_tl_01_duplicate_is_a_draft_copy_with_a_new_id_and_no_tool_set_number() {
    let dir = temp_dir("dup");
    let mut lib = ToolLibrary::open(&dir).unwrap();
    z1_set(&mut lib);
    let mut original = new_draft(&lib, ToolType::BallEnd).unwrap();
    edit_cutting_data(&mut original, "Oak", oak(), None).unwrap();
    save_draft(&mut lib, &original, &[("Makera Z1".into(), Some(3))]).unwrap();
    let copy = duplicate_draft(&lib, original.id).unwrap();
    assert_ne!(copy.id, original.id);
    assert_eq!(copy.name, "New ball end mill copy");
    assert_eq!(
        (&copy.tool, &copy.cutting_data),
        (&original.tool, &original.cutting_data)
    );
    assert!(lib.get(copy.id).is_none());
    save_draft(&mut lib, &copy, &[]).unwrap();
    assert_eq!(
        tool_set_numbers(&lib, copy.id),
        [("Makera Z1".into(), None)]
    );
    assert_eq!(
        tool_set_numbers(&lib, original.id),
        [("Makera Z1".into(), Some(3))]
    );
    assert_eq!(
        duplicate_draft(&lib, original.id).unwrap().name,
        "New ball end mill copy 2"
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn req_tl_05_cutting_data_by_material_is_added_checked_renamed_and_removed() {
    let lib = ToolLibrary::open(&temp_dir("cut")).unwrap();
    let mut tool = new_draft(&lib, ToolType::FlatEnd).unwrap();
    edit_cutting_data(&mut tool, " Oak ", oak(), None).unwrap();
    let mdf = CuttingData {
        spindle_speed: SpindleSpeed(14000.0),
        ..oak()
    };
    edit_cutting_data(&mut tool, "MDF", mdf, None).unwrap();
    let rows = cutting_rows(&tool);
    let names: Vec<&str> = rows.iter().map(|r| r.material.as_str()).collect();
    assert_eq!(names, ["MDF", "Oak"]);
    let flutes = f64::from(tool.tool.flute_count);
    assert!((rows[1].feed - 12000.0 * 0.02 * flutes).abs() < 1e-9);
    // A stepover wider than the tool is refused; the tool is unchanged.
    let kept = tool.clone();
    let wide = CuttingData {
        stepover: Millimetres(99.0),
        ..oak()
    };
    let err = edit_cutting_data(&mut tool, "Oak", wide, None)
        .unwrap_err()
        .to_string();
    assert!(err.contains("stepover"), "{err}");
    assert!(edit_cutting_data(&mut tool, " ", oak(), None).is_err());
    assert_eq!(tool, kept);
    // Renamed: the old material goes.
    edit_cutting_data(&mut tool, "Medium density fibreboard", mdf, Some("MDF")).unwrap();
    assert!(!tool.cutting_data.contains_key("MDF"));
    // A rename onto another entry, in any spelling, is refused and changes
    // neither entry (decision 0197).
    let kept = tool.clone();
    for target in ["Oak", "oak"] {
        let err = edit_cutting_data(&mut tool, target, mdf, Some("Medium density fibreboard"))
            .unwrap_err()
            .to_string();
        assert!(err.contains("already has"), "{target}: {err}");
        assert_eq!(tool, kept);
    }
    remove_cutting_data(&mut tool, "Oak");
    assert_eq!(cutting_rows(&tool).len(), 1);
}

#[test]
fn req_tl_11_tool_set_numbers_are_saved_refused_when_taken_and_removed() {
    let dir = temp_dir("sets");
    let mut lib = ToolLibrary::open(&dir).unwrap();
    z1_set(&mut lib);
    let a = saved(&mut lib, ToolType::FlatEnd, "6 mm flat");
    let b = new_draft(&lib, ToolType::BallEnd).unwrap();
    set_tool_set_number(&mut lib, "makera z1", a, Some(3)).unwrap();
    // A taken number refuses the whole save: not even the tool is written.
    let err = save_draft(&mut lib, &b, &[("Makera Z1".into(), Some(3))])
        .unwrap_err()
        .to_string();
    assert!(err.contains("T3 is already \"6 mm flat\""), "{err}");
    assert!(lib.get(b.id).is_none());
    assert!(save_draft(&mut lib, &b, &[("Makera Z1".into(), Some(0))]).is_err());
    assert!(save_draft(&mut lib, &b, &[("Other".into(), Some(4))]).is_err());
    set_tool_set_number(&mut lib, "Makera Z1", a, Some(7)).unwrap();
    assert_eq!(lib.tool_set("Makera Z1").unwrap().tool_for(7), Some(a));
    assert_eq!(lib.tool_set("Makera Z1").unwrap().tool_for(3), None);
    set_tool_set_number(&mut lib, "Makera Z1", a, None).unwrap();
    assert!(lib.tool_set("Makera Z1").unwrap().entries.is_empty());
    assert!(ToolLibrary::open(&dir).unwrap().problems().is_empty());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn req_tl_07_req_tl_11_req_tl_12_delete_keeps_the_project_copy_clears_tool_sets_and_stays_deleted()
{
    let dir = temp_dir("delete");
    let mut lib = ToolLibrary::open(&dir).unwrap();
    lib.install_defaults().unwrap();
    let entry = lib.tool_set("Makera Z1").unwrap().entries[0];
    let mut project = Project::new("p");
    add(&mut project, &lib, entry.tool);
    let impact = delete_impact(&lib, Some(&project), entry.tool).unwrap();
    assert!(impact.in_project);
    assert_eq!(
        impact.tool_sets,
        [("Makera Z1".to_owned(), entry.controller.number)]
    );

    delete_tool(&mut lib, entry.tool).unwrap();
    assert!(lib.get(entry.tool).is_none());
    assert!(
        lib.tool_set("Makera Z1")
            .unwrap()
            .mapping_for(entry.tool)
            .is_none()
    );
    // The project keeps its copy, now not in the library.
    assert_eq!(project.tools().len(), 1);
    assert_eq!(
        project_rows(&project, Some(&lib), "")[0].copy,
        CopyState::NotInLibrary
    );
    // Reopened, no tool set names a missing tool, and an update does not
    // bring the deleted default back (TL-12).
    let mut reopened = ToolLibrary::open(&dir).unwrap();
    assert!(reopened.problems().is_empty(), "{:?}", reopened.problems());
    reopened.install_defaults().unwrap();
    assert!(reopened.get(entry.tool).is_none());
    assert!(dir.join(LEDGER_FILE).exists());
    assert!(delete_tool(&mut reopened, entry.tool).is_err());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn req_tl_12_an_edited_default_is_not_overwritten_by_an_update() {
    let dir = temp_dir("edited-default");
    let mut lib = ToolLibrary::open(&dir).unwrap();
    lib.install_defaults().unwrap();
    let mut draft = lib.tools().next().unwrap().clone();
    edit_name(&mut draft, "My edited default").unwrap();
    save_draft(&mut lib, &draft, &[]).unwrap();
    let mut again = ToolLibrary::open(&dir).unwrap();
    again.install_defaults().unwrap();
    assert_eq!(again.get(draft.id).unwrap().name, "My edited default");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn req_tl_03_profile_files_import_and_export_with_the_file_checks() {
    let dir = temp_dir("profile");
    std::fs::create_dir_all(&dir).unwrap();
    let lib = ToolLibrary::open(&dir).unwrap();
    let custom = new_draft(&lib, ToolType::Custom).unwrap();
    let ToolShape::Custom { profile } = &custom.tool.shape else {
        panic!("{custom:?}")
    };
    let file = dir.join("profile.toml");
    export_profile(&custom.tool, profile, &file).unwrap();
    let mut flat = new_draft(&lib, ToolType::FlatEnd).unwrap();
    edit_profile(&mut flat, load_profile(&file).unwrap()).unwrap();
    assert_eq!(flat.tool.shape, custom.tool.shape);
    // An invalid profile is not written, and a bad file is refused naming it.
    let mut broken = profile.clone();
    broken.segments.reverse();
    let bad = dir.join("bad.toml");
    assert!(export_profile(&custom.tool, &broken, &bad).is_err());
    assert!(!bad.exists());
    std::fs::write(&bad, "schema = 1\n").unwrap();
    let err = load_profile(&bad).unwrap_err().to_string();
    assert!(err.contains("bad.toml"), "{err}");
    assert_eq!(starting_profile(&flat.tool).as_ref(), Some(profile));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn req_tl_01_import_previews_conflicts_and_applies_each_choice() {
    let (src, out, dst) = (temp_dir("isrc"), temp_dir("iout"), temp_dir("idst"));
    let mut theirs = ToolLibrary::open(&src).unwrap();
    let same = saved(&mut theirs, ToolType::FlatEnd, "Flat");
    let changed = saved(&mut theirs, ToolType::BallEnd, "Ball");
    let both = saved(&mut theirs, ToolType::Drill, "New drill");
    let fresh = saved(&mut theirs, ToolType::VBit, "V");
    let mut mine = ToolLibrary::open(&dst).unwrap();
    for id in [same, changed, both] {
        mine.save_tool(theirs.get(id).unwrap().clone()).unwrap();
    }
    let mut draft = theirs.get(changed).unwrap().clone();
    edit_field(&mut draft, "flute_length", &ParameterValue::Number(11.0)).unwrap();
    save_draft(&mut theirs, &draft, &[]).unwrap();
    let mut draft = theirs.get(both).unwrap().clone();
    edit_name(&mut draft, "Their drill").unwrap();
    save_draft(&mut theirs, &draft, &[]).unwrap();
    assert_eq!(export_tools(&theirs, &[], &out).unwrap().len(), 4);

    let preview = preview_import(&mine, &out).unwrap();
    assert_eq!(preview.unchanged, [same]);
    let added: Vec<ToolId> = preview.added.iter().map(|t| t.id).collect();
    assert_eq!(added, [fresh]);
    assert_eq!(preview.conflicts.len(), 2);
    let flute = &preview
        .conflicts
        .iter()
        .find(|c| c.1.id == changed)
        .unwrap()
        .1;
    assert_eq!(flute.fields, ["tool.flute_length"]);
    // Nothing is written by a preview.
    assert_eq!(mine, ToolLibrary::open(&dst).unwrap());

    let outcome = apply_import(&mut mine, &preview, |id| {
        if id == changed {
            ConflictChoice::Replace
        } else {
            ConflictChoice::KeepBoth
        }
    })
    .unwrap();
    assert_eq!(outcome.replaced, [changed]);
    assert_eq!(outcome.added.len(), 2);
    assert_eq!(
        mine.get(changed).unwrap().tool.flute_length,
        Millimetres(11.0)
    );
    assert_eq!(mine.get(both).unwrap().name, "New drill");
    assert!(mine.tools().any(|t| t.name == "Their drill (imported)"));

    // Keep mine changes nothing; a single tool file imports too.
    let mut again = ToolLibrary::open(&dst).unwrap();
    let single = preview_import(&again, theirs.path_of(both).unwrap()).unwrap();
    assert_eq!(single.conflicts.len(), 1);
    let kept = apply_import(&mut again, &single, |_| ConflictChoice::KeepMine).unwrap();
    assert_eq!(kept.kept, [both]);
    assert_eq!(again.get(both).unwrap().name, "New drill");
    // A path that is neither is refused.
    assert!(preview_import(&again, &out.join("missing")).is_err());
    // A folder with a file that cannot be read imports nothing (NFR-07).
    std::fs::write(out.join("broken.toml"), "schema = 1\nid = 3\n").unwrap();
    let err = preview_import(&again, &out).unwrap_err().to_string();
    assert!(err.contains("nothing imported"), "{err}");
    for d in [src, out, dst] {
        std::fs::remove_dir_all(d).ok();
    }
}

#[test]
fn req_tl_07_req_tl_11_add_update_save_and_the_operation_tool_section() {
    let dir = temp_dir("project");
    let mut lib = ToolLibrary::open(&dir).unwrap();
    z1_set(&mut lib);
    let mut draft = new_draft(&lib, ToolType::FlatEnd).unwrap();
    save_draft(&mut lib, &draft, &[("Makera Z1".into(), Some(12))]).unwrap();
    let id = draft.id;
    let mut project = project_with_operation(None);
    let z1 = openchuck_app::project::machines::builtin(openchuck_app::project::machines::MAKERA_Z1)
        .unwrap()
        .unwrap();
    project.set_machine(Some(z1)).unwrap();

    let (_, controller, source) = add_to_project(&project, &lib, id).unwrap();
    assert_eq!((controller.number, source), (12, NumberSource::ToolSet));
    add(&mut project, &lib, id);
    assert!(add_to_project(&project, &lib, id).is_err());
    assert!(operation_tool(&project, Some(&lib), OperationId::from_u128(5)).is_none());

    apply(&mut project, |p| {
        set_operation_tool(p, OperationId::from_u128(5), id).unwrap()
    });
    let shown = operation_tool(&project, Some(&lib), OperationId::from_u128(5)).unwrap();
    assert_eq!(
        (shown.controller.number, &shown.copy),
        (12, &CopyState::Same)
    );
    assert_eq!(shown.tool_set_number, None);
    assert_eq!(shown.copy.text(), "Same as the library");
    assert_eq!(
        operation_tool(&project, None, OperationId::from_u128(5))
            .unwrap()
            .copy,
        CopyState::Unknown
    );
    let rows = project_rows(&project, Some(&lib), "");
    assert_eq!(
        (rows[0].number, &rows[0].used_by[..]),
        (12, &["finish".to_owned()][..])
    );

    // The project's number changes; the tool set's number is then shown.
    apply(&mut project, |p| set_project_tool_number(p, id, 4).unwrap());
    let shown = operation_tool(&project, Some(&lib), OperationId::from_u128(5)).unwrap();
    assert_eq!(shown.tool_set_number, Some(("Makera Z1".into(), 12)));
    assert!(set_project_tool_number(&project, id, 0).is_err());
    let other = saved(&mut lib, ToolType::Drill, "2 mm drill");
    add(&mut project, &lib, other);
    let err = set_project_tool_number(&project, other, 4)
        .unwrap_err()
        .to_string();
    assert!(err.contains("T4 is already"), "{err}");

    // The library tool changes: the copy differs until updated, keeping T4.
    edit_field(&mut draft, "flute_length", &ParameterValue::Number(9.0)).unwrap();
    save_draft(&mut lib, &draft, &[]).unwrap();
    let shown = operation_tool(&project, Some(&lib), OperationId::from_u128(5)).unwrap();
    assert_eq!(
        shown.copy,
        CopyState::Differs(vec!["tool.flute_length".into()])
    );
    assert_eq!(shown.copy.text(), "Differs from the library: flute length");
    assert_eq!(
        save_to_library_changes(&project, &lib, id).unwrap(),
        Some(vec!["flute length".to_owned()])
    );
    apply(&mut project, |p| {
        update_from_library(p, &lib, id).unwrap().unwrap()
    });
    assert!(update_from_library(&project, &lib, id).unwrap().is_none());
    let snapshot = &project.tools()[0];
    assert_eq!(
        (snapshot.controller.number, snapshot.tool.flute_length),
        (4, Millimetres(9.0))
    );

    // The project's copy is edited on its own (one command per change),
    // keeping its number; the library is untouched.
    let mut copy: LibraryTool = project_tool_as_edit(&project, id).unwrap();
    edit_name(&mut copy, "Project flat").unwrap();
    apply(&mut project, |p| {
        snapshot_command(p, &copy).unwrap().unwrap()
    });
    assert!(snapshot_command(&project, &copy).unwrap().is_none());
    assert_eq!(project.tools()[0].controller.number, 4);
    assert_ne!(lib.get(id).unwrap().name, "Project flat");

    // Save to library writes the project's copy back, without its number.
    save_to_library(&project, &mut lib, id).unwrap();
    assert_eq!(lib.get(id).unwrap().name, "Project flat");
    let file = std::fs::read_to_string(lib.path_of(id).unwrap()).unwrap();
    assert!(!file.contains("number"), "{file}");

    // Removed from the library: the project keeps its copy.
    delete_tool(&mut lib, id).unwrap();
    let shown = operation_tool(&project, Some(&lib), OperationId::from_u128(5)).unwrap();
    assert_eq!(shown.copy, CopyState::NotInLibrary);
    assert_eq!(
        shown.copy.text(),
        "Not in the library (kept in the project)"
    );
    assert_eq!(save_to_library_changes(&project, &lib, id).unwrap(), None);
    assert!(update_from_library(&project, &lib, id).is_err());

    // Remove from project is refused while the operation uses it.
    let err = remove_from_project(&project, id).unwrap_err().to_string();
    assert!(err.contains("\"finish\""), "{err}");
    let mut op = project
        .operation(OperationId::from_u128(5))
        .unwrap()
        .clone();
    op.tool = None;
    project.replace(Item::Operation(op)).unwrap();
    assert!(matches!(
        remove_from_project(&project, id).unwrap(),
        Command::Remove(_)
    ));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn req_tl_05_a_material_added_to_one_tool_is_known_to_every_tool_and_setup() {
    use openchuck_app::materials::{canonical_material, known_materials};
    let mut lib = ToolLibrary::open(&temp_dir("materials")).unwrap();
    let mut a = new_draft(&lib, ToolType::FlatEnd).unwrap();
    edit_cutting_data(&mut a, "Softwood", oak(), None).unwrap();
    edit_name(&mut a, "Flat").unwrap();
    save_draft(&mut lib, &a, &[]).unwrap();
    let mut b = new_draft(&lib, ToolType::BallEnd).unwrap();
    edit_cutting_data(&mut b, "aluminium", oak(), None).unwrap();
    edit_name(&mut b, "Ball").unwrap();
    save_draft(&mut lib, &b, &[]).unwrap();
    let known = known_materials(None, Some(&lib));
    assert_eq!(known, ["Softwood", "aluminium"]);
    // Typing a known material in another spelling takes the known one, so
    // a second tool cannot hold "softwood" beside "Softwood".
    let typed = canonical_material(&known, "softwood");
    assert_eq!(typed, "Softwood");
    // The same tool refuses two spellings of one material (decision 0196).
    let mut c = new_draft(&lib, ToolType::FlatEnd).unwrap();
    edit_cutting_data(&mut c, "Softwood", oak(), None).unwrap();
    let err = edit_cutting_data(&mut c, "softwood", oak(), None)
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("Softwood") && err.contains("softwood"),
        "{err}"
    );
    assert_eq!(cutting_rows(&c).len(), 1);
}

#[test]
fn req_tl_05_the_operations_spindle_speed_comes_from_itself_then_the_tool_then_is_missing() {
    use openchuck_app::tool_library::{operation_spindle, set_operation_spindle};
    let registry = openchuck_app::path::Registry::builtin();
    let mut lib = ToolLibrary::open(&temp_dir("spindle")).unwrap();
    let mut draft = new_draft(&lib, ToolType::FlatEnd).unwrap();
    edit_cutting_data(&mut draft, "Softwood", oak(), None).unwrap();
    save_draft(&mut lib, &draft, &[]).unwrap();
    let mut project = project_with_operation(None);
    add(&mut project, &lib, draft.id);
    let op = OperationId::from_u128(5);
    apply(&mut project, |p| {
        set_operation_tool(p, op, draft.id).unwrap()
    });
    // No material on the setup: nothing to take, so the speed is missing and
    // the message says what to enter and where.
    let s = operation_spindle(&project, op).unwrap();
    assert_eq!((s.set, s.from_tool), (None, None));
    let message = s.missing.unwrap();
    assert!(
        message.contains("Enter a spindle speed") && message.contains("stock material"),
        "{message}"
    );
    // The setup's material in another spelling still finds the data.
    let mut setup = project.setups()[0].clone();
    setup.material = Some("softwood".into());
    project.replace(Item::Setup(setup)).unwrap();
    let s = operation_spindle(&project, op).unwrap();
    assert_eq!(s.from_tool.map(|f| f.0), Some(12000.0));
    assert!(s.missing.is_none());
    // The user's own value wins and ↺ (None) takes it away again.
    apply(&mut project, |p| {
        set_operation_spindle(p, &registry, op, Some(" 9000 rpm ")).unwrap()
    });
    assert_eq!(operation_spindle(&project, op).unwrap().set, Some(9000.0));
    apply(&mut project, |p| {
        set_operation_spindle(p, &registry, op, None).unwrap()
    });
    assert_eq!(operation_spindle(&project, op).unwrap().set, None);
    // A bad value is refused in plain words and nothing changes.
    for bad in ["", "fast", "0", "-5", "NaN", "inf", "9000rpmrpm"] {
        let e = set_operation_spindle(&project, &registry, op, Some(bad))
            .unwrap_err()
            .to_string();
        assert!(e.contains("spindle speed"), "{bad}: {e}");
        assert!(!e.contains("tool number"), "{bad}: {e}");
    }
    assert!(set_operation_spindle(&project, &registry, OperationId::from_u128(99), None).is_err());
    assert!(operation_spindle(&project, OperationId::from_u128(99)).is_none());
}

#[test]
fn req_tl_07_unused_candidates_include_only_unreferenced_snapshots_across_setups() {
    use openchuck_app::tool_library::unused_project_tools;
    let dir = temp_dir("unused-query");
    let mut lib = ToolLibrary::open(&dir).unwrap();
    let ids: Vec<_> = ["enabled", "disabled", "unused"]
        .into_iter()
        .map(|name| saved(&mut lib, ToolType::FlatEnd, name))
        .collect();
    let mut project = project_with_operation(None);
    for id in &ids {
        add(&mut project, &lib, *id);
    }
    apply(&mut project, |p| {
        set_operation_tool(p, OperationId::from_u128(5), ids[0]).unwrap()
    });
    let setup = SetupId::from_u128(2);
    project
        .insert(
            Item::Setup(Setup::new(setup, "second setup")),
            Placement::End,
        )
        .unwrap();
    let mut disabled = project.operations()[0].clone();
    disabled.id = OperationId::from_u128(6);
    disabled.setup = setup;
    disabled.enabled = false;
    disabled.tool = Some(ids[1]);
    project
        .insert(Item::Operation(disabled), Placement::End)
        .unwrap();
    let rows = unused_project_tools(&project);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0], project.tools()[2]);
    assert_eq!(rows[0].name, "unused");
    assert_eq!(rows[0].controller.number, 3);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_tl_07_req_tl_11_req_gc_14_req_ui_08_cleanup_is_atomic_and_restores_exact_snapshots() {
    use openchuck_app::tool_library::{remove_unused_tools, unused_project_tools};
    let dir = temp_dir("unused-history");
    let mut lib = ToolLibrary::open(&dir).unwrap();
    let ids: Vec<_> = ["first", "second", "third"]
        .into_iter()
        .map(|name| saved(&mut lib, ToolType::FlatEnd, name))
        .collect();
    let mut project = project_with_operation(None);
    for id in &ids {
        add(&mut project, &lib, *id);
    }
    let before = project.clone();
    let reviewed = unused_project_tools(&project);
    assert!(
        remove_unused_tools(&project, &reviewed, &[])
            .unwrap()
            .is_none()
    );
    let project_dir = temp_dir("unused-project-history");
    openchuck_app::project::file::save(&project, &project_dir).unwrap();
    let mut app =
        openchuck_app::Application::open(&project_dir, openchuck_app::Threads::Exactly(1)).unwrap();
    let command = remove_unused_tools(&project, &reviewed, &[ids[0], ids[2]])
        .unwrap()
        .unwrap();
    app.execute(command).unwrap();
    project = app.project().clone();
    assert_eq!(project.tools().len(), 1);
    assert_eq!(project.tools()[0], before.tools()[1]);
    app.undo().unwrap();
    project = app.project().clone();
    assert_eq!(project, before);
    app.redo().unwrap();
    project = app.project().clone();
    assert_eq!(project.tools().len(), 1);
    app.save().unwrap();
    assert!(!app.is_modified());
    assert_eq!(
        openchuck_app::project::file::load(&project_dir).unwrap(),
        project
    );
    // A freed number is reused by the existing allocator. Undo that later
    // addition before restoring the exact original snapshots.
    let fresh = saved(&mut lib, ToolType::Drill, "fresh");
    let (add, controller, _) = add_to_project(app.project(), &lib, fresh).unwrap();
    assert_eq!(controller.number, 1);
    app.execute(add).unwrap();
    assert!(app.is_modified());
    let reused = app.project().clone();
    app.undo().unwrap();
    app.undo().unwrap();
    assert_eq!(app.project(), &before);
    app.save().unwrap();
    assert_eq!(
        openchuck_app::project::file::load(&project_dir).unwrap(),
        before
    );
    app.redo().unwrap();
    app.redo().unwrap();
    assert_eq!(app.project(), &reused);
    assert_eq!(ToolLibrary::open(&dir).unwrap().tools().count(), 4);
    std::fs::remove_dir_all(project_dir).unwrap();
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_tl_07_cleanup_refuses_entire_changed_selection() {
    use openchuck_app::tool_library::{remove_unused_tools, unused_project_tools};
    let dir = temp_dir("unused-changed");
    let mut lib = ToolLibrary::open(&dir).unwrap();
    let a = saved(&mut lib, ToolType::FlatEnd, "a");
    let b = saved(&mut lib, ToolType::Drill, "b");
    let mut project = project_with_operation(None);
    for id in [a, b] {
        add(&mut project, &lib, id);
    }
    let reviewed = unused_project_tools(&project);
    for change in 0..4 {
        let mut changed = project.clone();
        match change {
            0 => {
                let mut t = changed.tools()[1].clone();
                t.name = "changed".into();
                changed.replace(Item::Tool(t)).unwrap();
            }
            1 => apply(&mut changed, |p| set_project_tool_number(p, b, 17).unwrap()),
            2 => {
                changed
                    .remove(openchuck_app::project::ItemId::Tool(b))
                    .unwrap();
            }
            _ => apply(&mut changed, |p| {
                set_operation_tool(p, OperationId::from_u128(5), b).unwrap()
            }),
        }
        let before = changed.clone();
        assert!(remove_unused_tools(&changed, &reviewed, &[a, b]).is_err());
        assert_eq!(changed, before);
    }
    assert!(remove_unused_tools(&project, &[], &[a]).is_err());
    std::fs::remove_dir_all(dir).unwrap();
}
