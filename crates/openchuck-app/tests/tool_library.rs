// SPDX-License-Identifier: AGPL-3.0-or-later
//! The tool library window's view model (specification 12.1; TL-01 to
//! TL-07, TL-11, TL-12; decision record 0127).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::path::PathBuf;

use openchuck_app::project::id::{OperationId, SetupId, ToolId};
use openchuck_app::project::tools::LEDGER_FILE;
use openchuck_app::project::{
    Item, Operation, OrderKey, ParameterValue, Placement, Project, Setup,
};
use openchuck_app::tool_edit::starting_profile;
use openchuck_app::tool_library::{
    ConflictChoice, CopyState, ListGroup, ToolType, add_to_project, apply_import, cutting_rows,
    delete_impact, delete_tool, duplicate_tool, export_profile, export_tools, import_profile,
    new_tool, operation_tool, preview_import, remove_cutting_data, remove_from_project,
    save_to_library, set_cutting_data, set_field, set_material, set_name, set_operation_tool,
    set_project_tool_number, set_tool_set_number, set_type, tool_list, tool_set_numbers,
    update_from_library,
};
use openchuck_app::tools::{
    ControllerMapping, CuttingData, FeedRate, NumberSource, SpindleSpeed, ToolLibrary, ToolSet,
    ToolShape,
};
use openchuck_types::units::Millimetres;

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
            parameters: Default::default(),
        }),
        Placement::End,
    )
    .unwrap();
    p
}

#[test]
fn req_tl_02_req_tl_03_new_tool_of_every_type_is_saved_and_listed_by_group() {
    let dir = temp_dir("new");
    let mut lib = ToolLibrary::open(&dir).unwrap();
    let mut ids = Vec::new();
    for kind in ToolType::ALL {
        let id = new_tool(&mut lib, kind).unwrap();
        let tool = lib.get(id).unwrap();
        assert_eq!(ToolType::of(&tool.tool.shape), kind, "{kind:?}");
        assert_eq!(tool.name, format!("New {}", kind.label().to_lowercase()));
        assert!(tool.cutting_data.is_empty() && tool.tool.material.is_none());
        assert!(lib.path_of(id).unwrap().exists());
        ids.push(id);
    }
    // A second new flat end mill is numbered.
    let second = new_tool(&mut lib, ToolType::FlatEnd).unwrap();
    assert_eq!(lib.get(second).unwrap().name, "New flat end mill 2");
    // Reopened, the folder gives the same tools.
    assert_eq!(ToolLibrary::open(&dir).unwrap(), lib);

    let groups: Vec<ListGroup> = tool_list(&lib, None, "").iter().map(|g| g.0).collect();
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
    let flats = &tool_list(&lib, None, "")[1].1;
    assert_eq!(flats.len(), 2);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn req_tl_01_search_matches_name_type_and_material_and_marks_project_tools() {
    let dir = temp_dir("search");
    let mut lib = ToolLibrary::open(&dir).unwrap();
    let ball = new_tool(&mut lib, ToolType::BallEnd).unwrap();
    set_name(&mut lib, ball, "Ball 3 mm").unwrap();
    let flat10 = new_tool(&mut lib, ToolType::FlatEnd).unwrap();
    set_name(&mut lib, flat10, "Flat 10 mm").unwrap();
    let flat2 = new_tool(&mut lib, ToolType::FlatEnd).unwrap();
    set_name(&mut lib, flat2, "Flat 2 mm").unwrap();
    set_material(&mut lib, flat2, " HSS ").unwrap();
    assert_eq!(
        lib.get(flat2).unwrap().tool.material.as_deref(),
        Some("HSS")
    );

    let mut project = Project::new("p");
    let (add, _, _) = add_to_project(&project, &lib, ball).unwrap();
    let Command::Add { item, placement } = add else {
        panic!("{add:?}")
    };
    project.insert(item, placement).unwrap();

    let all = tool_list(&lib, Some(&project), "");
    assert!(all[0].1[0].in_project);
    // Names sort by their numbers: 2 before 10.
    let flats: Vec<&str> = all[1].1.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(flats, ["Flat 2 mm", "Flat 10 mm"]);
    assert!(all[1].1.iter().all(|r| !r.in_project));
    assert!(all[1].1[0].summary.starts_with("Flat end mill · Ø"));

    let by_type = tool_list(&lib, None, "BALL END");
    assert_eq!(by_type.len(), 1);
    let by_material = tool_list(&lib, None, "hss");
    assert_eq!(by_material[0].1[0].id, flat2);
    assert!(tool_list(&lib, None, "nothing like it").is_empty());
    std::fs::remove_dir_all(&dir).ok();
}

use openchuck_app::Command;

#[test]
fn req_tl_04_form_edits_save_the_file_and_refusals_change_nothing() {
    let dir = temp_dir("form");
    let mut lib = ToolLibrary::open(&dir).unwrap();
    let id = new_tool(&mut lib, ToolType::FlatEnd).unwrap();
    let path = lib.path_of(id).unwrap().to_path_buf();
    set_field(&mut lib, id, "flute_length", &ParameterValue::Number(14.0)).unwrap();
    assert!(
        std::fs::read_to_string(&path)
            .unwrap()
            .contains("flute_length = 14.0")
    );
    let before = std::fs::read_to_string(&path).unwrap();
    // Refused: a negative diameter, an empty name.
    assert!(set_field(&mut lib, id, "diameter", &ParameterValue::Number(-1.0)).is_err());
    let err = set_name(&mut lib, id, "  ").unwrap_err().to_string();
    assert!(err.contains("name"), "{err}");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), before);
    // Unknown tools are refused, naming nothing that exists.
    assert!(set_name(&mut lib, ToolId::from_u128(99), "x").is_err());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn req_tl_02_req_tl_03_changing_the_type_keeps_the_sizes_or_names_the_problem() {
    let dir = temp_dir("type");
    let mut lib = ToolLibrary::open(&dir).unwrap();
    let id = new_tool(&mut lib, ToolType::FlatEnd).unwrap();
    let diameter = lib.get(id).unwrap().tool.diameter;
    set_type(&mut lib, id, ToolType::BullNose).unwrap();
    let tool = &lib.get(id).unwrap().tool;
    assert!(matches!(tool.shape, ToolShape::BullNose { .. }));
    assert_eq!(tool.diameter, diameter);
    // To a custom profile: the outline of the tool as it was.
    set_type(&mut lib, id, ToolType::Custom).unwrap();
    assert!(matches!(
        lib.get(id).unwrap().tool.shape,
        ToolShape::Custom { .. }
    ));
    assert_eq!(lib.get(id).unwrap().tool.diameter, diameter);
    // Back to a flat: valid again.
    set_type(&mut lib, id, ToolType::FlatEnd).unwrap();
    // A V-bit whose cone cannot reach the diameter within the flutes.
    set_field(&mut lib, id, "flute_length", &ParameterValue::Number(0.5)).unwrap();
    let err = set_type(&mut lib, id, ToolType::VBit).unwrap_err();
    assert!(!err.to_string().is_empty());
    assert!(matches!(
        lib.get(id).unwrap().tool.shape,
        ToolShape::FlatEnd
    ));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn req_tl_01_duplicate_copies_the_tool_with_a_new_id_and_no_tool_set_number() {
    let dir = temp_dir("dup");
    let mut lib = ToolLibrary::open(&dir).unwrap();
    let id = new_tool(&mut lib, ToolType::BallEnd).unwrap();
    set_cutting_data(&mut lib, id, "Oak", oak()).unwrap();
    lib.save_tool_set(ToolSet {
        machine: "Makera Z1".into(),
        entries: Vec::new(),
    })
    .unwrap();
    set_tool_set_number(&mut lib, "Makera Z1", id, Some(3)).unwrap();
    let copy = duplicate_tool(&mut lib, id).unwrap();
    assert_ne!(copy, id);
    let (a, b) = (lib.get(id).unwrap(), lib.get(copy).unwrap());
    assert_eq!(b.name, "New ball end mill copy");
    assert_eq!((&a.tool, &a.cutting_data), (&b.tool, &b.cutting_data));
    assert_eq!(tool_set_numbers(&lib, copy), [("Makera Z1".into(), None)]);
    assert_eq!(tool_set_numbers(&lib, id), [("Makera Z1".into(), Some(3))]);
    let second = duplicate_tool(&mut lib, id).unwrap();
    assert_eq!(lib.get(second).unwrap().name, "New ball end mill copy 2");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn req_tl_05_cutting_data_by_material_is_added_checked_and_removed() {
    let dir = temp_dir("cut");
    let mut lib = ToolLibrary::open(&dir).unwrap();
    let id = new_tool(&mut lib, ToolType::FlatEnd).unwrap();
    set_cutting_data(&mut lib, id, " Oak ", oak()).unwrap();
    set_cutting_data(
        &mut lib,
        id,
        "MDF",
        CuttingData {
            spindle_speed: SpindleSpeed(14000.0),
            ..oak()
        },
    )
    .unwrap();
    let rows = cutting_rows(lib.get(id).unwrap());
    assert_eq!(
        rows.iter().map(|r| r.material.as_str()).collect::<Vec<_>>(),
        ["MDF", "Oak"]
    );
    let flutes = f64::from(lib.get(id).unwrap().tool.flute_count);
    assert!((rows[1].feed - 12000.0 * 0.02 * flutes).abs() < 1e-9);
    // A stepover wider than the tool is refused, and the file is unchanged.
    let path = lib.path_of(id).unwrap().to_path_buf();
    let before = std::fs::read_to_string(&path).unwrap();
    let wide = CuttingData {
        stepover: Millimetres(99.0),
        ..oak()
    };
    let err = set_cutting_data(&mut lib, id, "Oak", wide)
        .unwrap_err()
        .to_string();
    assert!(err.contains("stepover"), "{err}");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), before);
    remove_cutting_data(&mut lib, id, "MDF").unwrap();
    assert_eq!(cutting_rows(lib.get(id).unwrap()).len(), 1);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn req_tl_11_tool_set_numbers_are_set_refused_when_taken_and_removed() {
    let dir = temp_dir("sets");
    let mut lib = ToolLibrary::open(&dir).unwrap();
    let a = new_tool(&mut lib, ToolType::FlatEnd).unwrap();
    let b = new_tool(&mut lib, ToolType::BallEnd).unwrap();
    lib.save_tool_set(ToolSet {
        machine: "Makera Z1".into(),
        entries: Vec::new(),
    })
    .unwrap();
    set_tool_set_number(&mut lib, "makera z1", a, Some(3)).unwrap();
    let err = set_tool_set_number(&mut lib, "Makera Z1", b, Some(3))
        .unwrap_err()
        .to_string();
    assert!(err.contains("T3 is already \"New flat end mill\""), "{err}");
    assert!(set_tool_set_number(&mut lib, "Makera Z1", b, Some(0)).is_err());
    assert!(set_tool_set_number(&mut lib, "Other", b, Some(4)).is_err());
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
    let set = lib.tool_set("Makera Z1").unwrap().clone();
    let entry = set.entries[0];
    let mut project = Project::new("p");
    let (add, _, _) = add_to_project(&project, &lib, entry.tool).unwrap();
    if let Command::Add { item, placement } = add {
        project.insert(item, placement).unwrap();
    }
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
    // Reopened, no tool set names a missing tool, and an update does not
    // bring the deleted default back (TL-12).
    let mut reopened = ToolLibrary::open(&dir).unwrap();
    assert!(reopened.problems().is_empty(), "{:?}", reopened.problems());
    let report = reopened.install_defaults().unwrap();
    assert!(reopened.get(entry.tool).is_none(), "{report:?}");
    assert!(dir.join(LEDGER_FILE).exists());
    assert!(delete_tool(&mut reopened, entry.tool).is_err());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn req_tl_12_an_edited_default_is_not_overwritten_by_an_update() {
    let dir = temp_dir("edited-default");
    let mut lib = ToolLibrary::open(&dir).unwrap();
    lib.install_defaults().unwrap();
    let id = lib.tools().next().unwrap().id;
    set_name(&mut lib, id, "My edited default").unwrap();
    let mut again = ToolLibrary::open(&dir).unwrap();
    again.install_defaults().unwrap();
    assert_eq!(again.get(id).unwrap().name, "My edited default");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn req_tl_03_profile_files_import_and_export_with_the_file_checks() {
    let dir = temp_dir("profile");
    let mut lib = ToolLibrary::open(&dir).unwrap();
    let id = new_tool(&mut lib, ToolType::Custom).unwrap();
    let tool = lib.get(id).unwrap().tool.clone();
    let ToolShape::Custom { profile } = &tool.shape else {
        panic!("{tool:?}")
    };
    let file = dir.join("profile.txt");
    export_profile(&tool, profile, &file).unwrap();
    let flat = new_tool(&mut lib, ToolType::FlatEnd).unwrap();
    import_profile(&mut lib, flat, &file).unwrap();
    assert_eq!(lib.get(flat).unwrap().tool.shape, tool.shape);
    // An invalid profile is not written, and a bad file is refused naming it.
    let mut broken = profile.clone();
    broken.segments.reverse();
    let bad = dir.join("bad.txt");
    assert!(export_profile(&tool, &broken, &bad).is_err());
    assert!(!bad.exists());
    std::fs::write(&bad, "schema = 1\n").unwrap();
    let err = import_profile(&mut lib, flat, &bad)
        .unwrap_err()
        .to_string();
    assert!(err.contains("bad.txt"), "{err}");
    assert_eq!(
        starting_profile(&lib.get(flat).unwrap().tool).as_ref(),
        Some(profile)
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn req_tl_01_import_previews_conflicts_and_applies_each_choice() {
    let (src, out, dst) = (temp_dir("isrc"), temp_dir("iout"), temp_dir("idst"));
    let mut theirs = ToolLibrary::open(&src).unwrap();
    let same = new_tool(&mut theirs, ToolType::FlatEnd).unwrap();
    let changed = new_tool(&mut theirs, ToolType::BallEnd).unwrap();
    let both = new_tool(&mut theirs, ToolType::Drill).unwrap();
    let fresh = new_tool(&mut theirs, ToolType::VBit).unwrap();
    let mut mine = ToolLibrary::open(&dst).unwrap();
    for id in [same, changed, both] {
        mine.save_tool(theirs.get(id).unwrap().clone()).unwrap();
    }
    set_field(
        &mut theirs,
        changed,
        "flute_length",
        &ParameterValue::Number(11.0),
    )
    .unwrap();
    set_name(&mut theirs, both, "Their drill").unwrap();
    let files = export_tools(&theirs, &[], &out).unwrap();
    assert_eq!(files.len(), 4);

    let preview = preview_import(&mine, &out).unwrap();
    assert_eq!(preview.unchanged, [same]);
    assert_eq!(
        preview.added.iter().map(|t| t.id).collect::<Vec<_>>(),
        [fresh]
    );
    let conflicts: Vec<_> = preview.conflicts.iter().map(|c| &c.1).collect();
    assert_eq!(conflicts.len(), 2);
    let flute = conflicts.iter().find(|c| c.id == changed).unwrap();
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
    for d in [src, out, dst] {
        std::fs::remove_dir_all(d).ok();
    }
}

#[test]
fn req_tl_07_req_tl_11_add_update_save_and_the_operation_tool_section() {
    let dir = temp_dir("project");
    let mut lib = ToolLibrary::open(&dir).unwrap();
    let id = new_tool(&mut lib, ToolType::FlatEnd).unwrap();
    let mut project = project_with_operation(None);
    let z1 = openchuck_app::project::machines::builtin(openchuck_app::project::machines::MAKERA_Z1)
        .unwrap()
        .unwrap();
    project.set_machine(Some(z1)).unwrap();
    lib.save_tool_set(ToolSet {
        machine: "Makera Z1".into(),
        entries: Vec::new(),
    })
    .unwrap();
    set_tool_set_number(&mut lib, "Makera Z1", id, Some(12)).unwrap();

    let (add, controller, source) = add_to_project(&project, &lib, id).unwrap();
    assert_eq!((controller.number, source), (12, NumberSource::ToolSet));
    let Command::Add { item, placement } = add else {
        panic!()
    };
    project.insert(item, placement).unwrap();
    assert!(add_to_project(&project, &lib, id).is_err());
    assert!(operation_tool(&project, Some(&lib), OperationId::from_u128(5)).is_none());

    let Command::Replace(item) =
        set_operation_tool(&project, OperationId::from_u128(5), id).unwrap()
    else {
        panic!()
    };
    project.replace(item).unwrap();
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

    // The project's number changes; the tool set's number is then shown.
    let Command::Replace(item) = set_project_tool_number(&project, id, 4).unwrap() else {
        panic!()
    };
    project.replace(item).unwrap();
    let shown = operation_tool(&project, Some(&lib), OperationId::from_u128(5)).unwrap();
    assert_eq!(shown.tool_set_number, Some(("Makera Z1".into(), 12)));
    assert_eq!(shown.copy, CopyState::Same);
    assert!(set_project_tool_number(&project, id, 0).is_err());

    // The library tool changes: the copy differs until updated, keeping T4.
    set_field(&mut lib, id, "flute_length", &ParameterValue::Number(9.0)).unwrap();
    let shown = operation_tool(&project, Some(&lib), OperationId::from_u128(5)).unwrap();
    assert_eq!(
        shown.copy,
        CopyState::Differs(vec!["tool.flute_length".into()])
    );
    assert_eq!(shown.copy.text(), "Differs from the library: flute length");
    let Some(Command::Replace(item)) = update_from_library(&project, &lib, id).unwrap() else {
        panic!()
    };
    project.replace(item).unwrap();
    assert!(update_from_library(&project, &lib, id).unwrap().is_none());
    let snapshot = &project.tools()[0];
    assert_eq!(
        (snapshot.controller.number, snapshot.tool.flute_length),
        (4, Millimetres(9.0))
    );

    // Save to library writes the project's copy back, without its number.
    let mut renamed = snapshot.clone();
    renamed.name = "Project flat".into();
    project.replace(Item::Tool(renamed)).unwrap();
    save_to_library(&project, &mut lib, id).unwrap();
    assert_eq!(lib.get(id).unwrap().name, "Project flat");
    assert!(
        !std::fs::read_to_string(lib.path_of(id).unwrap())
            .unwrap()
            .contains("number")
    );

    // Removed from the library: the project keeps its copy.
    delete_tool(&mut lib, id).unwrap();
    let shown = operation_tool(&project, Some(&lib), OperationId::from_u128(5)).unwrap();
    assert_eq!(shown.copy, CopyState::NotInLibrary);
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
    let _ = ControllerMapping::numbered(1);
    std::fs::remove_dir_all(&dir).ok();
}
