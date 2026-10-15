// SPDX-License-Identifier: AGPL-3.0-or-later
//! Project model tests (work breakdown slice 1, items 1, 3, 4 and 7).

// Test code may unwrap, index and panic (NFR-07 applies to library code).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use openchuck_project::id::{ModelId, OperationId, SelectionSetId, SetupId, ToolId};
use openchuck_project::{
    ContentHash, Item, ItemId, ModelCheck, ModelRef, OldPlace, Operation, OrderKey, ParameterValue,
    Placement, Project, ProjectError, RelativePath, SelectionSet, SelectionTarget, Setup,
    ToolSnapshot, cache,
};
use openchuck_types::brep::FaceId;
use openchuck_types::tool::{ControllerMapping, Tool, ToolShape};
use openchuck_types::units::Millimetres;
use proptest::prelude::*;

fn temp_project(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("openchuck-project-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn tool() -> Tool {
    Tool {
        shape: ToolShape::BallEnd,
        diameter: Millimetres(6.0),
        flute_length: Millimetres(20.0),
        overall_length: Millimetres(30.0),
        shank_diameter: Millimetres(6.0),
        flute_count: 2,
        material: None,
        centre_cutting: None,
    }
}

fn model(id: u128) -> ModelRef {
    // A STEP path, because the tests select faces (a B-rep selection).
    ModelRef::new(
        ModelId::from_u128(id),
        RelativePath::new("models/part.step").unwrap(),
        ContentHash::of_bytes(b"solid part"),
    )
}

fn operation(id: u128, setup: SetupId) -> Operation {
    Operation {
        enabled: true,
        id: OperationId::from_u128(id),
        order: OrderKey::default(),
        name: format!("Operation {id}"),
        setup,
        strategy: "helical_finish".into(),
        tool: None,
        selections: Vec::new(),
        boundary: None,
        avoid: Vec::new(),
        check: Vec::new(),
        parameters: BTreeMap::from([("stepover".into(), ParameterValue::Number(0.5))]),
    }
}

/// A project with one of each item, all referring to each other.
fn sample() -> Project {
    let setup = SetupId::from_u128(10);
    let tool_id = ToolId::from_u128(20);
    let set = SelectionSetId::from_u128(30);
    let mut p = Project::new("sample");
    p.insert(Item::Model(model(1)), Placement::End).unwrap();
    p.insert(Item::Setup(Setup::new(setup, "Setup 1")), Placement::End)
        .unwrap();
    p.insert(
        Item::Tool(ToolSnapshot {
            id: tool_id,
            order: OrderKey::default(),
            name: "6 mm ball".into(),
            tool: tool(),
            controller: ControllerMapping::numbered(1),
            cutting_data: Default::default(),
        }),
        Placement::End,
    )
    .unwrap();
    p.insert(
        Item::SelectionSet(SelectionSet {
            id: set,
            order: OrderKey::default(),
            name: "Top faces".into(),
            model: ModelId::from_u128(1),
            model_hash: model(1).hash,
            target: SelectionTarget::Faces(BTreeSet::from([FaceId(3), FaceId(7)])),
            descriptors: Vec::new(),
        }),
        Placement::End,
    )
    .unwrap();
    let mut op = operation(40, setup);
    op.tool = Some(tool_id);
    op.selections = vec![set];
    p.insert(Item::Operation(op), Placement::End).unwrap();
    p.validate().unwrap();
    p
}

#[test]
fn req_prj_05_ids_have_one_text_form_and_differ() {
    let a = OperationId::new();
    let b = OperationId::new();
    assert_ne!(a, b);
    let text = a.to_string();
    assert_eq!(text.len(), 36);
    assert_eq!(text.parse::<OperationId>().unwrap(), a);
    assert!(text.to_uppercase().parse::<OperationId>().is_err());
    assert!(text.replace('-', "").parse::<OperationId>().is_err());
    let json = serde_json::to_string(&a).unwrap();
    assert_eq!(json, format!("\"{text}\""));
    assert_eq!(serde_json::from_str::<OperationId>(&json).unwrap(), a);
}

#[test]
fn req_prj_05_ids_are_unique_and_references_must_exist() {
    let mut p = sample();
    let setup = SetupId::from_u128(10);

    let duplicate = Item::Setup(Setup::new(setup, "again"));
    assert!(matches!(
        p.insert(duplicate, Placement::End),
        Err(ProjectError::DuplicateId { .. })
    ));

    let orphan = operation(41, SetupId::from_u128(99));
    let err = p
        .insert(Item::Operation(orphan), Placement::End)
        .unwrap_err();
    assert!(matches!(err, ProjectError::UnknownReference { .. }));
    assert!(err.to_string().contains("does not exist"), "{err}");

    // Items in use cannot be removed; the document is unchanged.
    let before = p.clone();
    for id in [
        ItemId::Model(ModelId::from_u128(1)),
        ItemId::Setup(setup),
        ItemId::Tool(ToolId::from_u128(20)),
        ItemId::SelectionSet(SelectionSetId::from_u128(30)),
    ] {
        let err = p.remove(id).unwrap_err();
        assert!(matches!(err, ProjectError::InUse { .. }), "{err}");
    }
    assert_eq!(p, before);

    // Removing the user first frees them, in reverse order of dependence.
    let (index, removed) = p
        .remove(ItemId::Operation(OperationId::from_u128(40)))
        .unwrap();
    assert_eq!(index, 0);
    assert!(matches!(removed, Item::Operation(_)));
    p.remove(ItemId::SelectionSet(SelectionSetId::from_u128(30)))
        .unwrap();
    p.remove(ItemId::Model(ModelId::from_u128(1))).unwrap();
    p.remove(ItemId::Setup(setup)).unwrap();
    p.validate().unwrap();
}

#[test]
fn req_prj_05_reorder_keeps_ids_and_contents() {
    let mut p = sample();
    let setup = SetupId::from_u128(10);
    for id in 41..44 {
        p.insert(Item::Operation(operation(id, setup)), Placement::End)
            .unwrap();
    }
    let before: BTreeSet<_> = p.operations().iter().map(|o| o.id).collect();
    let moved = ItemId::Operation(OperationId::from_u128(43));
    match p.move_item(moved, 0).unwrap() {
        // Only the moved item's key changes, so one file changes.
        OldPlace::Keys(old) => assert_eq!(old.len(), 1),
        other => panic!("expected keys, got {other:?}"),
    }
    let order: Vec<_> = p.operations().iter().map(|o| o.id.as_u128()).collect();
    assert_eq!(order, [43, 40, 41, 42]);
    let after: BTreeSet<_> = p.operations().iter().map(|o| o.id).collect();
    assert_eq!(before, after);
    assert!(matches!(
        p.move_item(moved, 4),
        Err(ProjectError::BadIndex { .. })
    ));
}

#[test]
fn req_nfr_07_bad_values_are_errors() {
    let mut p = sample();
    let mut op = operation(41, SetupId::from_u128(10));
    op.parameters
        .insert("depth".into(), ParameterValue::Number(f64::NAN));
    let err = p.insert(Item::Operation(op), Placement::End).unwrap_err();
    assert!(err.to_string().contains("not a finite number"), "{err}");

    let mut bad_tool = tool();
    bad_tool.diameter = Millimetres(-1.0);
    let err = p
        .insert(
            Item::Tool(ToolSnapshot {
                id: ToolId::from_u128(21),
                order: OrderKey::default(),
                name: "bad".into(),
                tool: bad_tool,
                controller: ControllerMapping::numbered(1),
                cutting_data: Default::default(),
            }),
            Placement::End,
        )
        .unwrap_err();
    assert!(err.to_string().contains("tool.diameter"), "{err}");

    assert!("not-an-id".parse::<SetupId>().is_err());
    assert!("blake3:00".parse::<ContentHash>().is_err());
    assert!(serde_json::from_str::<SetupId>("\"x\"").is_err());
}

#[test]
fn req_prj_02_model_paths_are_relative_with_forward_slashes() {
    for good in ["part.stl", "models/part.step", "../shared/part.stl"] {
        assert_eq!(RelativePath::new(good).unwrap().as_str(), good);
    }
    for bad in [
        "",
        "/abs/part.stl",
        "C:/part.stl",
        "models\\part.stl",
        "a//b",
        "./a",
    ] {
        assert!(RelativePath::new(bad).is_err(), "{bad:?} accepted");
    }
    let native: PathBuf = ["models", "part.stl"].iter().collect();
    assert_eq!(
        RelativePath::from_path(&native).unwrap().as_str(),
        "models/part.stl"
    );
    assert!(RelativePath::from_path(&std::env::temp_dir()).is_err());
    let resolved = RelativePath::new("models/part.stl")
        .unwrap()
        .resolve(Path::new("project"));
    assert_eq!(
        resolved,
        Path::new("project").join("models").join("part.stl")
    );
}

#[test]
fn req_prj_02_changed_model_is_detected_by_hash() {
    let root = temp_project("hash");
    std::fs::create_dir_all(root.join("models")).unwrap();
    let file = root.join("models").join("part.step");
    std::fs::write(&file, b"solid part").unwrap();

    let reference = model(1);
    assert_eq!(ContentHash::of_file(&file).unwrap(), reference.hash);
    assert_eq!(reference.check(&root).unwrap(), ModelCheck::Unchanged);

    std::fs::write(&file, b"solid part, edited").unwrap();
    match reference.check(&root).unwrap() {
        ModelCheck::Changed { recorded, found } => {
            assert_eq!(recorded, reference.hash);
            assert_eq!(found, ContentHash::of_bytes(b"solid part, edited"));
        }
        other => panic!("expected a change, got {other:?}"),
    }

    std::fs::remove_file(&file).unwrap();
    assert_eq!(reference.check(&root).unwrap(), ModelCheck::Missing);
}

#[test]
fn req_prj_03_cache_folder_ignores_itself() {
    let root = temp_project("cache");
    let dir = cache::ensure_cache_dir(&root).unwrap();
    assert_eq!(dir, cache::cache_dir(&root));
    let ignore = dir.join(".gitignore");
    assert_eq!(
        std::fs::read_to_string(&ignore).unwrap(),
        cache::CACHE_IGNORE
    );

    // A damaged ignore file is put back.
    std::fs::write(&ignore, "").unwrap();
    cache::ensure_cache_dir(&root).unwrap();
    assert_eq!(
        std::fs::read_to_string(&ignore).unwrap(),
        cache::CACHE_IGNORE
    );

    // Git ignores everything in the cache, including the ignore file itself.
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .args(args)
            .current_dir(&root)
            .output()
            .expect("git must be installed to run this test")
    };
    assert!(git(&["init", "-q"]).status.success());
    std::fs::write(dir.join("toolpath.bin"), b"generated").unwrap();
    std::fs::write(root.join("project.txt"), b"tracked").unwrap();
    let status = git(&["status", "--porcelain", "--untracked-files=all"]);
    let listed = String::from_utf8(status.stdout).unwrap();
    assert_eq!(listed.trim(), "?? project.txt");
}

#[test]
fn selection_sets_reserve_descriptors_and_round_trip_through_serde() {
    // Not the project file format (SP-8): checks that the model's serde
    // implementations keep every field, including the reserved descriptors.
    let p = sample();
    let set = &p.selection_sets()[0];
    let json = serde_json::to_string(set).unwrap();
    assert!(json.contains("\"descriptors\":[]"), "{json}");
    assert!(json.contains("blake3:"), "{json}");
    let back: SelectionSet = serde_json::from_str(&json).unwrap();
    assert_eq!(&back, set);
}

proptest! {
    #[test]
    fn content_hash_text_round_trips(bytes in proptest::collection::vec(any::<u8>(), 0..256)) {
        let hash = ContentHash::of_bytes(&bytes);
        let text = hash.to_string();
        prop_assert!(text.starts_with("blake3:"));
        prop_assert_eq!(text.parse::<ContentHash>().unwrap(), hash);
        prop_assert!(text.to_uppercase().parse::<ContentHash>().is_err());
    }
}
