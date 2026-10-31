// SPDX-License-Identifier: AGPL-3.0-or-later
//! The project folder on disk (work breakdown slice 1 items 2 and 5; decision
//! records 0006 and 0008). The Git merge scenarios S1 to S6 follow SP-8.

// Test code may unwrap, index and panic (NFR-07 applies to library code).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime};

use openchuck_project::id::{ModelId, OperationId, SelectionSetId, SetupId, ToolId};
use openchuck_project::{
    BoundaryOptions, ContentHash, Item, ItemId, MeshEdge, ModelCheck, ModelRef, OnBreak, Operation,
    OrderKey, ParameterValue, Placement, Project, ProjectError, RelativePath, SelectionSet,
    SelectionTarget, SetOptions, Setup, ToolSnapshot, file,
};
use openchuck_types::brep::{EdgeId, FaceId};
use openchuck_types::tool::{ControllerMapping, Tool, ToolShape};
use openchuck_types::units::Millimetres;

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("openchuck-persist-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

const SETUP: SetupId = SetupId::from_u128(10);

fn op_id(n: u128) -> OperationId {
    OperationId::from_u128(n)
}

fn model(n: u128) -> ModelRef {
    // STEP paths, because the sample selects faces (a B-rep selection).
    ModelRef::new(
        ModelId::from_u128(n),
        RelativePath::new(format!("models/part{n}.step")).unwrap(),
        ContentHash::of_bytes(&n.to_le_bytes()),
    )
}

fn operation(n: u128) -> Operation {
    Operation {
        enabled: true,
        id: op_id(n),
        order: OrderKey::default(),
        name: format!("Operation {n}"),
        setup: SETUP,
        strategy: "helical_finish".into(),
        tool: Some(ToolId::from_u128(20)),
        selections: vec![SelectionSetId::from_u128(30)],
        boundary: None,
        avoid: Vec::new(),
        check: Vec::new(),
        restriction_options: Default::default(),
        spindle_speed: None,
        material: None,
        parameters: BTreeMap::from([
            ("stepover".into(), ParameterValue::Number(0.5)),
            ("passes".into(), ParameterValue::Integer(3)),
            ("climb".into(), ParameterValue::Flag(true)),
            ("pattern".into(), ParameterValue::Text("spiral".into())),
        ]),
    }
}

/// One model, setup, tool and selection set, and operations 101 to 104.
fn sample() -> Project {
    let mut p = Project::new("sample");
    p.insert(Item::Model(model(1)), Placement::End).unwrap();
    p.insert(Item::Setup(Setup::new(SETUP, "Setup 1")), Placement::End)
        .unwrap();
    p.insert(
        Item::Tool(ToolSnapshot {
            id: ToolId::from_u128(20),
            order: OrderKey::default(),
            name: "6 mm ball".into(),
            tool: Tool {
                shape: ToolShape::BallEnd,
                diameter: Millimetres(6.0),
                flute_length: Millimetres(20.0),
                overall_length: Millimetres(30.0),
                shank_diameter: Millimetres(6.0),
                flute_count: 2,
                material: None,
                centre_cutting: None,
            },
            controller: ControllerMapping::numbered(1),
            cutting_data: Default::default(),
        }),
        Placement::End,
    )
    .unwrap();
    p.insert(
        Item::SelectionSet(SelectionSet {
            id: SelectionSetId::from_u128(30),
            order: OrderKey::default(),
            name: "Outer faces".into(),
            model: ModelId::from_u128(1),
            model_hash: model(1).hash,
            target: SelectionTarget::Faces(BTreeSet::from([FaceId(3), FaceId(7), FaceId(12)])),
            descriptors: Vec::new(),
            stock_to_leave: None,
            colour: None,
            keep_tangent_edges: None,
        }),
        Placement::End,
    )
    .unwrap();
    for n in 101..=104 {
        p.insert(Item::Operation(operation(n)), Placement::End)
            .unwrap();
    }
    p
}

/// Every file under `root` (not `.git`), by relative path.
fn read_tree(root: &Path) -> BTreeMap<String, String> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, String>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.file_name().is_some_and(|n| n == ".git") {
                continue;
            }
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                let rel = path
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                out.insert(rel, std::fs::read_to_string(&path).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

/// Sets every file's modification time well into the past and returns it.
fn age_files(root: &Path) -> SystemTime {
    let past = SystemTime::now() - Duration::from_secs(86_400);
    for rel in read_tree(root).keys() {
        let file = std::fs::File::options()
            .write(true)
            .open(root.join(rel))
            .unwrap();
        file.set_modified(past).unwrap();
    }
    past
}

fn modified(root: &Path) -> BTreeMap<String, SystemTime> {
    read_tree(root)
        .keys()
        .map(|rel| {
            let time = std::fs::metadata(root.join(rel))
                .unwrap()
                .modified()
                .unwrap();
            (rel.clone(), time)
        })
        .collect()
}

/// (files changed, lines changed) between two trees, by a line-by-line
/// comparison of files with the same path.
fn diff_size(a: &BTreeMap<String, String>, b: &BTreeMap<String, String>) -> (usize, usize) {
    let paths: BTreeSet<_> = a.keys().chain(b.keys()).collect();
    let mut files = 0;
    let mut lines = 0;
    for path in paths {
        let (x, y) = (a.get(path), b.get(path));
        if x == y {
            continue;
        }
        files += 1;
        let x: Vec<_> = x.map(|t| t.lines().collect()).unwrap_or_default();
        let y: Vec<_> = y.map(|t| t.lines().collect()).unwrap_or_default();
        lines += x.len().max(y.len()) - x.iter().zip(&y).filter(|(p, q)| p == q).count();
    }
    (files, lines)
}

fn set_parameter(p: &mut Project, op: u128, value: f64) {
    let mut edited = p.operation(op_id(op)).unwrap().clone();
    edited
        .parameters
        .insert("stepover".into(), ParameterValue::Number(value));
    p.replace(Item::Operation(edited)).unwrap();
}

fn order_of(p: &Project) -> Vec<u128> {
    p.operations().iter().map(|o| o.id.as_u128()).collect()
}

/// The first line of every file this build writes.
fn current() -> String {
    format!("schema = {}\n", file::SCHEMA)
}

#[test]
fn req_prj_01_saving_twice_is_byte_identical() {
    let p = sample();
    let a = temp_dir("twice-a");
    let b = temp_dir("twice-b");
    file::save(&p, &a).unwrap();
    let first = read_tree(&a);
    file::save(&p, &a).unwrap();
    file::save(&p, &b).unwrap();
    assert_eq!(read_tree(&a), first);
    assert_eq!(read_tree(&b), first);
    for (path, text) in &first {
        assert!(text.ends_with('\n'), "{path} has no final newline");
        assert!(!text.contains('\r'), "{path} has a carriage return");
    }
}

#[test]
fn req_prj_01_load_then_save_is_a_no_op() {
    let dir = temp_dir("noop");
    file::save(&sample(), &dir).unwrap();
    let before = read_tree(&dir);
    let past = age_files(&dir);
    let loaded = file::load(&dir).unwrap();
    assert_eq!(loaded, sample());
    file::save(&loaded, &dir).unwrap();
    assert_eq!(read_tree(&dir), before);
    for (path, time) in modified(&dir) {
        assert_eq!(time, past, "{path} was rewritten");
    }
}

#[test]
fn req_prj_01_windows_line_endings_are_not_rewritten() {
    let dir = temp_dir("crlf");
    file::save(&sample(), &dir).unwrap();
    for (rel, text) in read_tree(&dir) {
        std::fs::write(dir.join(rel), text.replace('\n', "\r\n")).unwrap();
    }
    let past = age_files(&dir);
    let loaded = file::load(&dir).unwrap();
    assert_eq!(loaded, sample());
    file::save(&loaded, &dir).unwrap();
    for (path, time) in modified(&dir) {
        assert_eq!(time, past, "{path} was rewritten");
    }
}

#[test]
fn req_prj_01_one_parameter_edit_changes_one_line() {
    let dir = temp_dir("one-line");
    let mut p = sample();
    file::save(&p, &dir).unwrap();
    let before = read_tree(&dir);
    set_parameter(&mut p, 102, 0.25);
    file::save(&p, &dir).unwrap();
    assert_eq!(diff_size(&before, &read_tree(&dir)), (1, 1));
}

#[test]
fn req_prj_05_reorder_and_add_touch_one_file() {
    let dir = temp_dir("reorder");
    let mut p = sample();
    file::save(&p, &dir).unwrap();
    let before = read_tree(&dir);
    p.move_item(ItemId::Operation(op_id(104)), 0).unwrap();
    file::save(&p, &dir).unwrap();
    let moved = read_tree(&dir);
    assert_eq!(diff_size(&before, &moved), (1, 1));
    assert_eq!(order_of(&file::load(&dir).unwrap()), [104, 101, 102, 103]);

    p.insert(Item::Operation(operation(105)), Placement::Index(2))
        .unwrap();
    file::save(&p, &dir).unwrap();
    assert_eq!(diff_size(&moved, &read_tree(&dir)).0, 1);
    assert_eq!(
        order_of(&file::load(&dir).unwrap()),
        [104, 101, 105, 102, 103]
    );
}

#[test]
fn req_prj_05_removed_items_lose_their_files() {
    let dir = temp_dir("remove");
    let mut p = sample();
    file::save(&p, &dir).unwrap();
    for n in 101..=104 {
        p.remove(ItemId::Operation(op_id(n))).unwrap();
    }
    file::save(&p, &dir).unwrap();
    assert!(!dir.join("operations").exists());
    // Other files in the folder are left alone.
    std::fs::create_dir_all(dir.join("setups")).unwrap();
    std::fs::write(dir.join("setups").join("notes.txt"), "mine").unwrap();
    file::save(&p, &dir).unwrap();
    assert!(dir.join("setups").join("notes.txt").exists());
    assert_eq!(file::load(&dir).unwrap(), p);
}

#[test]
fn req_prj_06_selections_survive_save_and_load() {
    let dir = temp_dir("selections");
    let p = sample();
    file::save(&p, &dir).unwrap();
    let loaded = file::load(&dir).unwrap();
    assert_eq!(loaded.selection_sets(), p.selection_sets());
    assert_eq!(
        loaded.operations()[0].selections,
        p.operations()[0].selections
    );
}

#[test]
fn req_prj_03_project_files_hold_no_generated_data() {
    let dir = temp_dir("no-generated");
    file::save(&sample(), &dir).unwrap();
    let folders: BTreeSet<_> = read_tree(&dir)
        .keys()
        .map(|path| path.split('/').next().unwrap().to_owned())
        .collect();
    let expected: BTreeSet<_> = [
        "operations",
        "project.toml",
        "selections",
        "setups",
        "tools",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    assert_eq!(folders, expected);
}

fn git(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new("git")
        .args(["-c", "user.name=Test", "-c", "user.email=test@example.com"])
        .args(["-c", "core.autocrlf=false", "-c", "init.defaultBranch=main"])
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git must be installed to run this test")
}

fn git_ok(dir: &Path, args: &[&str]) {
    let out = git(dir, args);
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Commits the sample, applies `a` and `b` on two branches, saving and
/// committing each, then merges `a` into `b`. Returns whether the merge was
/// clean and the folder.
fn merge(name: &str, a: impl Fn(&mut Project), b: impl Fn(&mut Project)) -> (bool, PathBuf) {
    let dir = temp_dir(&format!("merge-{name}"));
    git_ok(&dir, &["init", "-q"]);
    file::save(&sample(), &dir).unwrap();
    git_ok(&dir, &["add", "-A"]);
    git_ok(&dir, &["commit", "-q", "-m", "base"]);
    for (branch, edit) in [("a", &a as &dyn Fn(&mut Project)), ("b", &b)] {
        git_ok(&dir, &["checkout", "-q", "-b", branch, "main"]);
        let mut p = file::load(&dir).unwrap();
        edit(&mut p);
        file::save(&p, &dir).unwrap();
        git_ok(&dir, &["add", "-A"]);
        git_ok(&dir, &["commit", "-q", "-m", branch]);
    }
    let clean = git(&dir, &["merge", "-q", "--no-edit", "a"])
        .status
        .success();
    (clean, dir)
}

fn merged(name: &str, a: impl Fn(&mut Project), b: impl Fn(&mut Project)) -> Project {
    let (clean, dir) = merge(name, a, b);
    assert!(clean, "{name}: the merge conflicted");
    file::load(&dir).unwrap()
}

fn param(p: &Project, op: u128) -> Option<&ParameterValue> {
    p.operation(op_id(op)).unwrap().parameters.get("stepover")
}

#[test]
fn req_prj_05_git_merges_of_different_items_are_clean() {
    // S1: reorder one operation / edit another.
    let p = merged(
        "s1",
        |p| drop(p.move_item(ItemId::Operation(op_id(104)), 0).unwrap()),
        |p| set_parameter(p, 102, 0.25),
    );
    assert_eq!(order_of(&p), [104, 101, 102, 103]);
    assert_eq!(param(&p, 102), Some(&ParameterValue::Number(0.25)));

    // S2: reorder one operation / edit the same operation.
    let p = merged(
        "s2",
        |p| drop(p.move_item(ItemId::Operation(op_id(104)), 0).unwrap()),
        |p| set_parameter(p, 104, 0.25),
    );
    assert_eq!(order_of(&p), [104, 101, 102, 103]);
    assert_eq!(param(&p, 104), Some(&ParameterValue::Number(0.25)));

    // S3: move one to the top / move another to the end.
    let p = merged(
        "s3",
        |p| drop(p.move_item(ItemId::Operation(op_id(103)), 0).unwrap()),
        |p| drop(p.move_item(ItemId::Operation(op_id(102)), 3).unwrap()),
    );
    assert_eq!(order_of(&p), [103, 101, 104, 102]);

    // S4: add after the first / add at the end.
    let p = merged(
        "s4",
        |p| {
            drop(
                p.insert(Item::Operation(operation(105)), Placement::Index(1))
                    .unwrap(),
            )
        },
        |p| {
            drop(
                p.insert(Item::Operation(operation(106)), Placement::End)
                    .unwrap(),
            )
        },
    );
    assert_eq!(order_of(&p), [101, 105, 102, 103, 104, 106]);

    // S5: both add at the same spot; equal keys order by ID.
    let p = merged(
        "s5",
        |p| {
            drop(
                p.insert(Item::Operation(operation(106)), Placement::Index(1))
                    .unwrap(),
            )
        },
        |p| {
            drop(
                p.insert(Item::Operation(operation(105)), Placement::Index(1))
                    .unwrap(),
            )
        },
    );
    assert_eq!(order_of(&p), [101, 105, 106, 102, 103, 104]);
    // Inserting between the two equal keys re-keys the upper one only.
    let mut p = p;
    p.insert(Item::Operation(operation(107)), Placement::Index(2))
        .unwrap();
    assert_eq!(order_of(&p), [101, 105, 107, 106, 102, 103, 104]);
    p.validate().unwrap();

    // S6: delete one / edit another.
    let p = merged(
        "s6",
        |p| drop(p.remove(ItemId::Operation(op_id(103))).unwrap()),
        |p| set_parameter(p, 102, 0.25),
    );
    assert_eq!(order_of(&p), [101, 102, 104]);
    assert_eq!(param(&p, 102), Some(&ParameterValue::Number(0.25)));
}

#[test]
fn req_prj_05_conflicting_model_additions_are_reported_clearly() {
    // Model references share project.toml (decision record 0008), so adding
    // one on each branch conflicts. Loading names the file and line.
    let (clean, dir) = merge(
        "models",
        |p| drop(p.insert(Item::Model(model(2)), Placement::End).unwrap()),
        |p| drop(p.insert(Item::Model(model(3)), Placement::End).unwrap()),
    );
    assert!(!clean);
    let err = file::load(&dir).unwrap_err();
    assert!(matches!(err, ProjectError::MergeConflict { .. }), "{err}");
    let message = err.to_string();
    assert!(message.contains("project.toml"), "{message}");
    assert!(
        message.contains("unresolved merge conflict at line"),
        "{message}"
    );
}

/// An operation using all three roles, with a set per role.
fn with_roles() -> Project {
    let mut p = sample();
    let set = |n: u128| {
        Item::SelectionSet(SelectionSet {
            id: SelectionSetId::from_u128(n),
            order: OrderKey::default(),
            name: format!("Set {n}"),
            model: ModelId::from_u128(1),
            model_hash: model(1).hash,
            target: SelectionTarget::Faces(BTreeSet::from([FaceId(n as u32)])),
            descriptors: Vec::new(),
            stock_to_leave: None,
            colour: None,
            keep_tangent_edges: None,
        })
    };
    for n in [31, 32, 33] {
        p.insert(set(n), Placement::End).unwrap();
    }
    let mut o = operation(101);
    o.boundary = Some(SelectionSetId::from_u128(31));
    o.avoid = vec![SelectionSetId::from_u128(32)];
    o.check = vec![SelectionSetId::from_u128(33)];
    p.replace(Item::Operation(o)).unwrap();
    p
}

#[test]
fn req_op_02_prj_06_operation_roles_survive_save_and_load() {
    let dir = temp_dir("roles");
    let p = with_roles();
    file::save(&p, &dir).unwrap();
    let loaded = file::load(&dir).unwrap();
    let o = &loaded.operations()[0];
    assert_eq!(o.boundary, Some(SelectionSetId::from_u128(31)));
    assert_eq!(o.avoid, vec![SelectionSetId::from_u128(32)]);
    assert_eq!(o.check, vec![SelectionSetId::from_u128(33)]);
    assert_eq!(loaded, p);
}

#[test]
fn req_op_02_nfr_07_unknown_set_in_any_role_is_refused_on_load() {
    let base = temp_dir("roles-unknown-base");
    file::save(&with_roles(), &base).unwrap();
    let op_file = base.join("operations/00000000-0000-0000-0000-000000000065.toml");
    let text = std::fs::read_to_string(&op_file).unwrap();
    for (role, known) in [("boundary", 31u128), ("avoid", 32), ("check", 33)] {
        let known = SelectionSetId::from_u128(known).to_string();
        assert!(text.contains(&known), "{role}: {text}");
        let dir = temp_dir(&format!("roles-unknown-{role}"));
        file::save(&with_roles(), &dir).unwrap();
        let path = dir.join("operations/00000000-0000-0000-0000-000000000065.toml");
        let bad = SelectionSetId::from_u128(999).to_string();
        std::fs::write(&path, text.replace(&known, &bad)).unwrap();
        let err = file::load(&dir).expect_err(role);
        assert!(err.to_string().contains(&bad), "{role}: {err}");
    }
}

#[test]
fn req_nfr_07_damaged_project_folders_are_errors() {
    let base = temp_dir("damaged-base");
    file::save(&sample(), &base).unwrap();
    let op_file = "operations/00000000-0000-0000-0000-000000000065.toml";

    // Each case edits one file of a fresh copy and expects this text in the error.
    // (case name, file to damage, damage, text expected in the error)
    type Case = (&'static str, &'static str, fn(&str) -> String, &'static str);
    let cases: &[Case] = &[
        (
            "syntax",
            op_file,
            |t| format!("{t}\nname = "),
            "cannot read",
        ),
        (
            "unknown-field",
            op_file,
            |t| t.replace(&current(), &format!("{}speed = 3\n", current())),
            "unknown field",
        ),
        (
            "no-schema",
            op_file,
            |t| t.replace(&current(), ""),
            "no schema version",
        ),
        (
            "newer",
            "project.toml",
            |t| t.replace(&current(), &format!("schema = {}\n", file::SCHEMA + 1)),
            "newer OpenChuckCAM",
        ),
        (
            "bad-key",
            op_file,
            |t| t.replace("order = \"V\"", "order = \"V0\""),
            "order key",
        ),
        (
            "bad-id",
            op_file,
            |t| t.replace("setup = \"00000000", "setup = \"x0000000"),
            "setup ID",
        ),
        (
            "dangling",
            op_file,
            |t| t.replace("-00000000000a\"", "-00000000000b\""),
            "does not exist",
        ),
        (
            "nan",
            op_file,
            |t| t.replace("stepover = 0.5", "stepover = nan"),
            "not a finite number",
        ),
    ];
    for (name, rel, damage, expected) in cases {
        let dir = temp_dir(&format!("damaged-{name}"));
        for (path, text) in read_tree(&base) {
            let target = dir.join(&path);
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            let text = if path == *rel { damage(&text) } else { text };
            std::fs::write(target, text).unwrap();
        }
        let err = file::load(&dir).unwrap_err().to_string();
        assert!(err.contains(expected), "{name}: {err}");
        // The same texts in memory give the same error.
        let err = file::load_files(&read_tree(&dir)).unwrap_err().to_string();
        assert!(err.contains(expected), "{name} in memory: {err}");
    }

    // A file not named by an ID, and a file named after another item.
    let dir = temp_dir("damaged-name");
    file::save(&sample(), &dir).unwrap();
    std::fs::write(dir.join("operations/extra.toml"), current()).unwrap();
    let err = file::load(&dir).unwrap_err().to_string();
    assert!(err.contains("not a operation ID"), "{err}");
    std::fs::remove_file(dir.join("operations/extra.toml")).unwrap();
    std::fs::rename(
        dir.join(op_file),
        dir.join("operations/00000000-0000-0000-0000-0000000000ff.toml"),
    )
    .unwrap();
    let err = file::load(&dir).unwrap_err().to_string();
    assert!(err.contains("not named after it"), "{err}");

    // No project file.
    let empty = temp_dir("damaged-empty");
    let err = file::load(&empty).unwrap_err();
    assert!(matches!(err, ProjectError::NotAProject(_)), "{err}");
}

/// The fuzz target for whole project folders loads them from memory, so
/// that loader must be the folder loader with its checks (NFR-07).
#[test]
fn req_nfr_07_project_texts_load_from_memory_as_from_the_folder() {
    let dir = temp_dir("in-memory");
    file::save(&sample(), &dir).unwrap();
    let mut texts = read_tree(&dir);
    let loaded = file::load(&dir).unwrap();
    assert_eq!(file::load_files(&texts).unwrap(), loaded);

    // As in a folder, files other than `.toml` files directly in an item
    // folder are not read.
    texts.insert("notes.md".into(), "not a project file".into());
    texts.insert("operations/notes.txt".into(), "not TOML".into());
    texts.insert("operations/old/extra.toml".into(), "not TOML".into());
    texts.insert("operations/.toml".into(), "not TOML".into());
    texts.insert("models/part1.toml".into(), "not TOML".into());
    assert_eq!(file::load_files(&texts).unwrap(), loaded);

    // An item file not named by an ID is refused, by its key.
    texts.insert("operations/extra.toml".into(), current());
    let err = file::load_files(&texts).unwrap_err().to_string();
    assert!(err.contains("operations/extra.toml"), "{err}");
    assert!(err.contains("not a operation ID"), "{err}");

    // No project file.
    texts.remove("project.toml");
    let err = file::load_files(&texts).unwrap_err();
    assert!(matches!(err, ProjectError::NotAProject(_)), "{err}");
}

#[test]
fn req_prj_05_long_keys_are_reported_and_renumber_fixes_them() {
    let mut p = sample();
    // Inserting again and again right after the first item grows keys.
    for n in 0..250 {
        p.insert(Item::Operation(operation(1000 + n)), Placement::Index(1))
            .unwrap();
    }
    assert!(!p.long_order_keys().is_empty());
    let before = order_of(&p);
    p.renumber(openchuck_project::ListKind::Operations).unwrap();
    assert!(p.long_order_keys().is_empty());
    assert_eq!(order_of(&p), before);
}

mod parser {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use openchuck_project::file::{FileKind, parse_file};
    use proptest::prelude::*;

    const KINDS: [FileKind; 5] = [
        FileKind::Project,
        FileKind::Setup,
        FileKind::Tool,
        FileKind::SelectionSet,
        FileKind::Operation,
    ];

    fn kind_of(path: &str) -> FileKind {
        match path.split('/').next() {
            Some("setups") => FileKind::Setup,
            Some("tools") => FileKind::Tool,
            Some("selections") => FileKind::SelectionSet,
            Some("operations") => FileKind::Operation,
            _ => FileKind::Project,
        }
    }

    proptest! {
        // The fuzz target runs this parser for longer in CI; this is the
        // quick local check that it returns errors, never panics (NFR-07).
        #[test]
        fn req_nfr_07_project_file_parser_never_panics_on_text(
            kind in 0usize..5,
            text in ".{0,200}",
        ) {
            let _ = parse_file(KINDS[kind], &text);
        }

        #[test]
        fn req_nfr_07_project_file_parser_never_panics_on_damaged_files(
            pick in any::<usize>(),
            at in any::<usize>(),
            cut in 0usize..20,
            insert in ".{0,12}",
        ) {
            let files = file::render(&sample()).unwrap();
            let (path, text) = files.iter().nth(pick % files.len()).unwrap();
            // Damage the text at a character boundary.
            let mut chars: Vec<char> = text.chars().collect();
            let at = at % (chars.len() + 1);
            let end = (at + cut).min(chars.len());
            chars.splice(at..end, insert.chars());
            let damaged: String = chars.into_iter().collect();
            let _ = parse_file(kind_of(path), &damaged);
            // The undamaged text always parses.
            prop_assert!(parse_file(kind_of(path), text).is_ok());
        }
    }
}

#[test]
fn req_prj_04_older_folder_migrates_to_the_current_schema() {
    // Schema 1 is slice 1's format; schema 2 added model units and transform
    // (decision record 0010). The fixture loads through the real migration.
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/schema-1");
    let project = file::load(&fixture).unwrap();
    assert_eq!(project.name(), "Schema 1 fixture");
    let model = &project.models()[0];
    assert_eq!(model.units, None);
    assert!(model.transform.is_empty());
    assert_eq!(model.check(&fixture).unwrap(), ModelCheck::Unchanged);
    assert_eq!(project.selection_sets().len(), 1);
    assert_eq!(project.operations().len(), 1);

    // Saved again, every file is the current schema and otherwise unchanged,
    // except that setups gain their default part and work offsets (schema 3,
    // decision record 0011).
    let dir = temp_dir("schema-1-resave");
    file::save(&project, &dir).unwrap();
    for (path, text) in file::render(&project).unwrap() {
        assert!(text.starts_with(&current()), "{path}");
        let old = std::fs::read_to_string(fixture.join(&path)).unwrap();
        let mut old: String = old
            .lines()
            .filter(|l| !l.starts_with('#'))
            .map(|l| format!("{l}\n"))
            .collect::<String>()
            .replace("schema = 1\n", &current());
        if path.starts_with("setups/") {
            old.push_str("part_offset = 0.0\n\n[work_offset]\nregister = \"g54\"\nx = 0.0\n");
        }
        assert_eq!(text, old, "{path}");
    }
    assert_eq!(file::load(&dir).unwrap(), project);
}

fn placed_model() -> ModelRef {
    use openchuck_types::transform::{Axis, TransformStep};
    use openchuck_types::units::{Degrees, LengthUnit};
    let mut model = ModelRef::new(
        ModelId::from_u128(7),
        RelativePath::new("models/shaft.stl").unwrap(),
        ContentHash::of_bytes(b"shaft"),
    );
    model.units = Some(LengthUnit::Inch);
    model.transform = vec![
        TransformStep::Rotate {
            axis: Axis::Y,
            angle: Degrees(90.0),
        },
        TransformStep::Mirror { axis: Axis::Z },
        TransformStep::Scale { factor: 1.5 },
        TransformStep::Translate {
            x: 2.5,
            y: 0.0,
            z: -1.0,
        },
        TransformStep::CentreOnRotaryAxis,
    ];
    model
}

#[test]
fn req_imp_03_imp_05_units_and_transform_survive_save_and_load() {
    let mut project = Project::new("placed");
    project
        .insert(Item::Model(placed_model()), Placement::End)
        .unwrap();
    let dir = temp_dir("placed");
    file::save(&project, &dir).unwrap();
    let back = file::load(&dir).unwrap();
    assert_eq!(back, project);
    let text = std::fs::read_to_string(dir.join("project.toml")).unwrap();
    // One value per line, so changing one step is a one-line diff (PRJ-01).
    assert!(text.contains("units = \"in\"\n"), "{text}");
    assert!(
        text.contains("kind = \"rotate\"\naxis = \"y\"\nangle = 90.0\n"),
        "{text}"
    );
    assert!(
        text.contains("kind = \"centre_on_rotary_axis\"\n"),
        "{text}"
    );

    // A model with neither is written exactly as schema 1 wrote it, apart
    // from the schema number.
    let mut plain = Project::new("plain");
    plain.insert(Item::Model(model(1)), Placement::End).unwrap();
    let text = file::render(&plain).unwrap()["project.toml"].clone();
    assert!(
        !text.contains("units") && !text.contains("transform"),
        "{text}"
    );
}

#[test]
fn req_imp_05_nfr_07_invalid_transforms_are_refused() {
    use openchuck_types::transform::TransformStep;
    let mut bad = placed_model();
    bad.transform.push(TransformStep::Scale { factor: 0.0 });
    let mut project = Project::new("bad");
    let err = project
        .insert(Item::Model(bad), Placement::End)
        .unwrap_err();
    assert!(err.to_string().contains("transform[5].factor"), "{err}");

    // The same from a damaged file, and an unknown step kind or unit.
    let mut good = Project::new("good");
    good.insert(Item::Model(placed_model()), Placement::End)
        .unwrap();
    for (name, from, to, expected) in [
        (
            "zero-scale",
            "factor = 1.5",
            "factor = 0.0",
            "greater than zero",
        ),
        (
            "bad-kind",
            "kind = \"mirror\"",
            "kind = \"shear\"",
            "unknown variant",
        ),
        (
            "bad-axis",
            "axis = \"z\"",
            "axis = \"w\"",
            "unknown variant",
        ),
        (
            "bad-unit",
            "units = \"in\"",
            "units = \"furlong\"",
            "unknown variant",
        ),
        (
            "extra",
            "kind = \"mirror\"",
            "kind = \"mirror\"\nangle = 3.0",
            "unknown field",
        ),
    ] {
        let dir = temp_dir(&format!("bad-transform-{name}"));
        file::save(&good, &dir).unwrap();
        let path = dir.join("project.toml");
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains(from), "{name}: {text}");
        std::fs::write(&path, text.replace(from, to)).unwrap();
        let err = file::load(&dir).unwrap_err().to_string();
        assert!(err.contains(expected), "{name}: {err}");
        // The same texts in memory give the same error.
        let err = file::load_files(&read_tree(&dir)).unwrap_err().to_string();
        assert!(err.contains(expected), "{name} in memory: {err}");
    }
}

#[test]
fn req_prj_06_selections_match_the_model_kind() {
    // Triangles on a mesh (STL) and faces on a B-rep (STEP) are accepted;
    // the other way round is refused (architecture section 5.6, OI-44).
    let selection = |model: &ModelRef, target: SelectionTarget| SelectionSet {
        id: SelectionSetId::from_u128(30),
        order: OrderKey::default(),
        name: "Set".into(),
        model: model.id,
        model_hash: model.hash,
        target,
        descriptors: Vec::new(),
        stock_to_leave: None,
        colour: None,
        keep_tangent_edges: None,
    };
    let stl = placed_model();
    let step = model(1);
    let triangles = || SelectionTarget::Triangles(BTreeSet::from([0, 4]));
    let faces = || SelectionTarget::Faces(BTreeSet::from([FaceId(2)]));

    let mut project = Project::new("kinds");
    project
        .insert(Item::Model(stl.clone()), Placement::End)
        .unwrap();
    project
        .insert(Item::Model(step.clone()), Placement::End)
        .unwrap();
    let err = project
        .insert(Item::SelectionSet(selection(&stl, faces())), Placement::End)
        .unwrap_err();
    assert!(err.to_string().contains("select triangles"), "{err}");
    let err = project
        .insert(
            Item::SelectionSet(selection(&step, triangles())),
            Placement::End,
        )
        .unwrap_err();
    assert!(err.to_string().contains("by face and edge"), "{err}");
    project
        .insert(
            Item::SelectionSet(selection(&stl, triangles())),
            Placement::End,
        )
        .unwrap();

    // Renaming the STL model to a STEP file would strand its triangle selection.
    let mut renamed = stl.clone();
    renamed.path = RelativePath::new("models/shaft.step").unwrap();
    let err = project.replace(Item::Model(renamed)).unwrap_err();
    assert!(err.to_string().contains("by face and edge"), "{err}");
    assert_eq!(project.models()[0], stl);
}

/// `with_roles` with a stock to leave on the boundary set.
fn with_leave(leave: Option<f64>) -> Project {
    let mut p = with_roles();
    let mut set = p
        .selection_sets()
        .iter()
        .find(|s| s.id == SelectionSetId::from_u128(31))
        .unwrap()
        .clone();
    set.stock_to_leave = leave;
    p.replace(Item::SelectionSet(set)).unwrap();
    p
}

#[test]
fn req_op_11_prj_06_stock_to_leave_on_a_set_survives_save_and_load() {
    let dir = temp_dir("leave-roundtrip");
    let p = with_leave(Some(0.25));
    file::save(&p, &dir).unwrap();
    let text =
        std::fs::read_to_string(dir.join("selections/00000000-0000-0000-0000-00000000001f.toml"))
            .unwrap();
    assert!(text.contains("stock_to_leave = 0.25\n"), "{text}");
    assert_eq!(file::load(&dir).unwrap(), p);
    // Written only when set: a set without one has no line, so unchanged
    // files stay byte for byte unchanged.
    for (path, text) in file::render(&with_leave(None)).unwrap() {
        assert!(!text.contains("stock_to_leave"), "{path}");
    }
}

#[test]
fn req_op_11_nfr_07_bad_stock_to_leave_is_refused_and_schema_7_still_loads() {
    let dir = temp_dir("leave-bad");
    let p = with_leave(Some(0.25));
    file::save(&p, &dir).unwrap();
    let path = dir.join("selections/00000000-0000-0000-0000-00000000001f.toml");
    let good = std::fs::read_to_string(&path).unwrap();
    for bad in ["-0.5", "nan"] {
        std::fs::write(&path, good.replace("0.25", bad)).unwrap();
        let err = file::load(&dir).unwrap_err();
        assert!(err.to_string().contains("stock_to_leave"), "{bad}: {err}");
    }
    // A schema 7 folder (no such field anywhere) loads unchanged.
    let old = temp_dir("leave-schema-7");
    let q = with_leave(None);
    file::save(&q, &old).unwrap();
    for (path, text) in file::render(&q).unwrap() {
        std::fs::write(
            old.join(&path),
            text.replacen(&current(), "schema = 7\n", 1),
        )
        .unwrap();
    }
    assert_eq!(file::load(&old).unwrap(), q);
}

#[test]
fn req_op_11_nfr_07_a_bad_stock_to_leave_cannot_be_put_in_the_project() {
    for bad in [-0.5, f64::NAN, f64::INFINITY] {
        let mut p = with_roles();
        let mut set = p
            .selection_sets()
            .iter()
            .find(|s| s.id == SelectionSetId::from_u128(31))
            .unwrap()
            .clone();
        set.stock_to_leave = Some(bad);
        let err = p.replace(Item::SelectionSet(set.clone())).unwrap_err();
        assert!(err.to_string().contains("stock_to_leave"), "{bad}: {err}");
        // Zero is allowed.
        set.stock_to_leave = Some(0.0);
        p.replace(Item::SelectionSet(set)).unwrap();
    }
}

#[test]
fn req_ui_05_prj_06_a_chosen_set_colour_survives_save_and_load_and_is_optional() {
    let dir = temp_dir("colour-roundtrip");
    let mut p = with_roles();
    let mut set = p
        .selection_sets()
        .iter()
        .find(|s| s.id == SelectionSetId::from_u128(31))
        .unwrap()
        .clone();
    set.colour = Some("#7d5bb5".to_owned());
    p.replace(Item::SelectionSet(set)).unwrap();
    file::save(&p, &dir).unwrap();
    let path = dir.join("selections/00000000-0000-0000-0000-00000000001f.toml");
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("colour = \"#7d5bb5\"\n"), "{text}");
    assert_eq!(file::load(&dir).unwrap(), p);
    // Written only when chosen, so older files stay byte for byte unchanged.
    for (path, text) in file::render(&with_roles()).unwrap() {
        assert!(!text.contains("colour"), "{path}");
    }
    // A bad value is refused on load.
    for bad in ["#7D5BB5", "7d5bb5", "#7d5bb", "#7d5bbg", "red"] {
        std::fs::write(&path, text.replace("#7d5bb5", bad)).unwrap();
        let err = file::load(&dir).unwrap_err();
        assert!(err.to_string().contains("colour"), "{bad}: {err}");
    }
}

#[test]
fn req_ui_05_nfr_07_a_bad_set_colour_cannot_be_put_in_the_project() {
    for bad in ["#7D5BB5", "7d5bb5", "", "#12345", "#1234567", "#ggg000"] {
        let mut p = with_roles();
        let mut set = p
            .selection_sets()
            .iter()
            .find(|s| s.id == SelectionSetId::from_u128(31))
            .unwrap()
            .clone();
        set.colour = Some(bad.to_owned());
        let err = p.replace(Item::SelectionSet(set)).unwrap_err();
        assert!(err.to_string().contains("colour"), "{bad}: {err}");
    }
}

#[test]
fn req_prj_04_a_schema_10_project_loads_with_default_set_colours() {
    let old = temp_dir("colour-schema-10");
    let q = with_roles();
    file::save(&q, &old).unwrap();
    for (path, text) in file::render(&q).unwrap() {
        std::fs::write(
            old.join(&path),
            text.replacen(&current(), "schema = 10\n", 1),
        )
        .unwrap();
    }
    let loaded = file::load(&old).unwrap();
    assert_eq!(loaded, q);
    assert!(loaded.selection_sets().iter().all(|s| s.colour.is_none()));
}

/// The tool snapshot file of `sample()` and its text.
const TOOL_FILE: &str = "tools/00000000-0000-0000-0000-000000000014.toml";

fn copy_tree_with(
    base: &BTreeMap<String, String>,
    name: &str,
    edit: impl Fn(&str, &str) -> String,
) -> PathBuf {
    let dir = temp_dir(name);
    for (path, text) in base {
        let target = dir.join(path);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(target, edit(path, text)).unwrap();
    }
    dir
}

#[test]
fn req_prj_04_req_tl_04_centre_cutting_round_trips_and_older_files_load_without_it() {
    let base_dir = temp_dir("centre-base");
    file::save(&sample(), &base_dir).unwrap();
    let base = read_tree(&base_dir);
    let tool_text = base.get(TOOL_FILE).unwrap();
    assert!(
        !tool_text.contains("centre_cutting"),
        "unset is not written: {tool_text}"
    );

    // A stated value is read and written back byte for byte.
    let stated = copy_tree_with(&base, "centre-stated", |path, text| {
        if path == TOOL_FILE {
            text.replace(
                "flute_count = 2\n",
                "flute_count = 2\ncentre_cutting = false\n",
            )
        } else {
            text.to_owned()
        }
    });
    let loaded = file::load(&stated).unwrap();
    let tool = loaded.tools().first().unwrap();
    assert_eq!(tool.tool.centre_cutting, Some(false));
    let again = temp_dir("centre-resaved");
    file::save(&loaded, &again).unwrap();
    assert_eq!(read_tree(&again), read_tree(&stated));

    // A schema 7 project loads, and its tool states nothing.
    let old = copy_tree_with(&base, "centre-schema7", |_, text| {
        text.replace(&current(), "schema = 7\n")
    });
    let loaded = file::load(&old).unwrap();
    assert_eq!(loaded.tools().first().unwrap().tool.centre_cutting, None);
}

#[test]
fn req_nfr_07_a_malformed_or_premature_centre_cutting_is_refused_naming_the_file() {
    let base_dir = temp_dir("centre-bad-base");
    file::save(&sample(), &base_dir).unwrap();
    let base = read_tree(&base_dir);
    let add = |value: &'static str, schema: String| {
        move |path: &str, text: &str| {
            if path == TOOL_FILE {
                text.replace(
                    "flute_count = 2\n",
                    &format!("flute_count = 2\ncentre_cutting = {value}\n"),
                )
                .replace(&current(), &schema)
            } else {
                text.to_owned()
            }
        }
    };
    let bad = copy_tree_with(&base, "centre-yes", add("\"yes\"", current()));
    let err = file::load(&bad).unwrap_err().to_string();
    assert!(
        err.contains("000000000014") && err.contains("centre_cutting"),
        "{err}"
    );

    // The field did not exist in schema 7: its presence there is not guessed at.
    let early = copy_tree_with(
        &base,
        "centre-early",
        add("false", "schema = 7\n".to_owned()),
    );
    let err = file::load(&early).unwrap_err().to_string();
    assert!(
        err.contains("000000000014") && err.contains("centre_cutting"),
        "{err}"
    );
}

/// A project with the STL `placed_model()` and a set of edges of its
/// triangles (schema 12; OI-204, decision record 0066).
fn with_mesh_edges() -> Project {
    let stl = placed_model();
    let mut p = Project::new("mesh edges");
    p.insert(Item::Model(stl.clone()), Placement::End).unwrap();
    let edges = [(4, 0), (4, 2), (9, 1), (120, 0)]
        .into_iter()
        .map(|(t, s)| MeshEdge::new(t, s).unwrap())
        .collect();
    p.insert(
        Item::SelectionSet(SelectionSet {
            id: SelectionSetId::from_u128(40),
            order: OrderKey::default(),
            name: "Lettering".into(),
            model: stl.id,
            model_hash: stl.hash,
            target: SelectionTarget::MeshEdges(edges),
            descriptors: Vec::new(),
            stock_to_leave: None,
            colour: None,
            keep_tangent_edges: None,
        }),
        Placement::End,
    )
    .unwrap();
    p
}

const MESH_EDGE_FILE: &str = "selections/00000000-0000-0000-0000-000000000028.toml";

#[test]
fn req_prj_06_req_op_08_a_mesh_edge_selection_survives_save_and_load() {
    let dir = temp_dir("mesh-edges");
    let p = with_mesh_edges();
    file::save(&p, &dir).unwrap();
    let text = std::fs::read_to_string(dir.join(MESH_EDGE_FILE)).unwrap();
    assert!(text.starts_with(&current()), "{text}");
    assert!(text.contains("[target]\nmesh_edges = ["), "{text}");
    let loaded = file::load(&dir).unwrap();
    assert_eq!(loaded, p);
    let SelectionTarget::MeshEdges(edges) = &loaded.selection_sets()[0].target else {
        panic!("{:?}", loaded.selection_sets()[0].target);
    };
    assert_eq!(
        edges
            .iter()
            .map(|e| (e.triangle, e.side(), e.corners()))
            .collect::<Vec<_>>(),
        [
            (4, 0, [0, 1]),
            (4, 2, [2, 0]),
            (9, 1, [1, 2]),
            (120, 0, [0, 1])
        ]
    );
    // Saving what was loaded writes the same bytes (PRJ-01).
    let again = temp_dir("mesh-edges-again");
    file::save(&loaded, &again).unwrap();
    assert_eq!(read_tree(&again), read_tree(&dir));
}

#[test]
fn req_prj_04_req_nfr_07_mesh_edges_need_schema_12_and_a_side_of_0_to_2() {
    let dir = temp_dir("mesh-edges-bad");
    file::save(&with_mesh_edges(), &dir).unwrap();
    let path = dir.join(MESH_EDGE_FILE);
    let good = std::fs::read_to_string(&path).unwrap();

    // Schema 11 had no mesh edges: such a file is refused, naming the target.
    std::fs::write(&path, good.replacen(&current(), "schema = 11\n", 1)).unwrap();
    let err = file::load(&dir).unwrap_err().to_string();
    assert!(
        err.contains("000000000028") && err.contains("mesh_edges") && err.contains("schema 12"),
        "{err}"
    );

    // A side is 0, 1 or 2.
    std::fs::write(&path, good.replacen("120,\n    0,", "120,\n    3,", 1)).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert_ne!(text, good, "the edit must change the file: {good}");
    let err = file::load(&dir).unwrap_err().to_string();
    assert!(
        err.contains("000000000028") && err.contains("0, 1 or 2"),
        "{err}"
    );

    // A schema 11 project without mesh edges loads as it is.
    let old = temp_dir("mesh-edges-schema-11");
    let q = with_roles();
    for (path, text) in file::render(&q).unwrap() {
        let target = old.join(&path);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(target, text.replacen(&current(), "schema = 11\n", 1)).unwrap();
    }
    assert_eq!(file::load(&old).unwrap(), q);
}

#[test]
fn req_prj_06_mesh_edges_apply_to_mesh_models_only() {
    let mut p = with_mesh_edges();
    let step = model(1);
    p.insert(Item::Model(step.clone()), Placement::End).unwrap();
    let mut set = p.selection_sets()[0].clone();
    set.id = SelectionSetId::from_u128(41);
    set.model = step.id;
    set.model_hash = step.hash;
    let err = p
        .insert(Item::SelectionSet(set), Placement::End)
        .unwrap_err();
    assert!(err.to_string().contains("its own edges"), "{err}");
    // An STL model stays one while it has mesh edges selected on it.
    let mut renamed = placed_model();
    renamed.path = RelativePath::new("models/shaft.step").unwrap();
    let err = p.replace(Item::Model(renamed)).unwrap_err();
    assert!(err.to_string().contains("its own edges"), "{err}");
    assert_eq!(MeshEdge::new(0, 3), None);
}

/// `sample()` (a STEP model, an operation using set 30) with set 34, a set of
/// B-rep edges, used by the operation too (decision record 0066).
fn with_edge_set(keep: Option<bool>) -> Project {
    let mut p = sample();
    p.insert(
        Item::SelectionSet(SelectionSet {
            id: SelectionSetId::from_u128(34),
            order: OrderKey::default(),
            name: "Outline".into(),
            model: ModelId::from_u128(1),
            model_hash: model(1).hash,
            target: SelectionTarget::Edges(BTreeSet::from([EdgeId(4), EdgeId(9)])),
            descriptors: Vec::new(),
            stock_to_leave: None,
            colour: None,
            keep_tangent_edges: keep,
        }),
        Placement::End,
    )
    .unwrap();
    let mut o = p.operations()[0].clone();
    o.selections.push(SelectionSetId::from_u128(34));
    p.replace(Item::Operation(o)).unwrap();
    p
}

const EDGE_SET_FILE: &str = "selections/00000000-0000-0000-0000-000000000022.toml";

#[test]
fn req_prj_06_req_op_08_keep_tangent_edges_survives_save_and_load_per_set_and_for_the_project() {
    // Unset everywhere: nothing is written, and the project's default is off.
    let plain = with_edge_set(None);
    assert!(!plain.keep_tangent_edges());
    for (path, text) in file::render(&plain).unwrap() {
        assert!(!text.contains("keep_tangent_edges"), "{path}");
    }
    let set = |p: &Project| {
        p.selection_sets()
            .iter()
            .find(|s| s.id == SelectionSetId::from_u128(34))
            .unwrap()
            .clone()
    };
    assert!(!plain.keeps_tangent_edges(&set(&plain)));

    // A set's own value and the project's default are written and read back.
    let mut p = with_edge_set(Some(false));
    assert!(!p.set_keep_tangent_edges(true));
    let dir = temp_dir("tangent");
    file::save(&p, &dir).unwrap();
    let project_text = std::fs::read_to_string(dir.join("project.toml")).unwrap();
    assert!(
        project_text.contains("\nkeep_tangent_edges = true\n"),
        "{project_text}"
    );
    let set_text = std::fs::read_to_string(dir.join(EDGE_SET_FILE)).unwrap();
    assert!(
        set_text.contains("keep_tangent_edges = false\n"),
        "{set_text}"
    );
    let loaded = file::load(&dir).unwrap();
    assert_eq!(loaded, p);
    // The set's value wins over the project's default; a set without one
    // takes the default.
    assert!(loaded.keep_tangent_edges());
    assert!(!loaded.keeps_tangent_edges(&set(&loaded)));
    let mut inherits = set(&loaded);
    inherits.keep_tangent_edges = None;
    assert!(loaded.keeps_tangent_edges(&inherits));
}

#[test]
fn req_prj_04_req_nfr_07_keep_tangent_edges_needs_schema_12_and_an_edge_set() {
    let mut p = with_edge_set(Some(true));
    p.set_keep_tangent_edges(true);
    let dir = temp_dir("tangent-bad");
    file::save(&p, &dir).unwrap();
    let base = read_tree(&dir);

    // In a schema 11 file, in either place, it is refused naming the file.
    for (name, file) in [
        ("tangent-11-set", EDGE_SET_FILE),
        ("tangent-11-project", "project.toml"),
    ] {
        let old = copy_tree_with(&base, name, |path, text| {
            if path == file {
                text.replacen(&current(), "schema = 11\n", 1)
            } else {
                text.to_owned()
            }
        });
        let err = file::load(&old).unwrap_err().to_string();
        assert!(
            err.contains("keep_tangent_edges") && err.contains("schema 12"),
            "{name}: {err}"
        );
    }
    // A schema 11 project without it loads with the default off.
    let q = with_edge_set(None);
    let old = copy_tree_with(
        &read_tree(&{
            let d = temp_dir("tangent-11-plain");
            file::save(&q, &d).unwrap();
            d
        }),
        "tangent-11-plain-copy",
        |_, text| text.replacen(&current(), "schema = 11\n", 1),
    );
    let loaded = file::load(&old).unwrap();
    assert_eq!(loaded, q);
    assert!(!loaded.keep_tangent_edges());

    // Only a set of B-rep edges may hold it.
    let mut faces = p.selection_sets()[0].clone();
    assert!(matches!(faces.target, SelectionTarget::Faces(_)));
    faces.keep_tangent_edges = Some(true);
    let err = p
        .replace(Item::SelectionSet(faces))
        .unwrap_err()
        .to_string();
    assert!(err.contains("keep_tangent_edges"), "{err}");
}

#[test]
fn req_op_08_req_prj_03_keep_tangent_edges_changes_the_cache_key_of_operations_on_edge_sets_only() {
    let hash = |p: &Project, op: u128| file::selection_inputs_hash(p, op_id(op)).unwrap();
    // The first operation uses the edge set; operation 102 only the face set.
    let mut p = with_edge_set(None);
    let edges_op = p.operations()[0].id;
    assert_ne!(edges_op, op_id(102));
    let (before_edges, before_faces) = (
        file::selection_inputs_hash(&p, edges_op).unwrap(),
        hash(&p, 102),
    );
    // The project's default changes what an inheriting edge set keeps.
    p.set_keep_tangent_edges(true);
    assert_ne!(
        file::selection_inputs_hash(&p, edges_op).unwrap(),
        before_edges
    );
    assert_eq!(hash(&p, 102), before_faces);
    // A set's own value equal to the resolved one gives the same key.
    let mut set = p
        .selection_sets()
        .iter()
        .find(|s| s.id == SelectionSetId::from_u128(34))
        .unwrap()
        .clone();
    let inherited = file::selection_inputs_hash(&p, edges_op).unwrap();
    set.keep_tangent_edges = Some(true);
    p.replace(Item::SelectionSet(set)).unwrap();
    assert_eq!(
        file::selection_inputs_hash(&p, edges_op).unwrap(),
        inherited
    );
}

#[test]
fn req_prj_06_the_selection_reset_warning_setting_survives_save_and_load() {
    // On by default and not written.
    let plain = with_edge_set(None);
    assert!(plain.warn_on_selection_reset());
    for (path, text) in file::render(&plain).unwrap() {
        assert!(!text.contains("warn_on_selection_reset"), "{path}");
    }
    // Turned off ("Don't show me this warning again"): written and read back.
    let mut p = with_edge_set(None);
    assert!(p.set_warn_on_selection_reset(false));
    let dir = temp_dir("reset-warning");
    file::save(&p, &dir).unwrap();
    let text = std::fs::read_to_string(dir.join("project.toml")).unwrap();
    assert!(
        text.contains("\nwarn_on_selection_reset = false\n"),
        "{text}"
    );
    let loaded = file::load(&dir).unwrap();
    assert_eq!(loaded, p);
    assert!(!loaded.warn_on_selection_reset());
}

#[test]
fn req_prj_04_req_nfr_07_the_selection_reset_warning_setting_needs_schema_12() {
    let mut p = with_edge_set(None);
    p.set_warn_on_selection_reset(false);
    let dir = temp_dir("reset-warning-11");
    file::save(&p, &dir).unwrap();
    let old = copy_tree_with(&read_tree(&dir), "reset-warning-11-copy", |path, text| {
        if path == "project.toml" {
            text.replacen(&current(), "schema = 11\n", 1)
        } else {
            text.to_owned()
        }
    });
    let err = file::load(&old).unwrap_err().to_string();
    assert!(
        err.contains("warn_on_selection_reset") && err.contains("schema 12"),
        "{err}"
    );
}

/// The operation file of operation 101 of `sample()`.
const OP_101_FILE: &str = "operations/00000000-0000-0000-0000-000000000065.toml";

/// `sample()` with operation 101 holding set 30 as its boundary and sets 31
/// and 32 as avoid and check sets, all on model 1.
fn with_restrictions() -> Project {
    let mut p = sample();
    for (n, name) in [(31, "Rib"), (32, "Floor")] {
        p.insert(
            Item::SelectionSet(SelectionSet {
                id: SelectionSetId::from_u128(n),
                order: OrderKey::default(),
                name: name.into(),
                model: ModelId::from_u128(1),
                model_hash: model(1).hash,
                target: SelectionTarget::Faces(BTreeSet::from([FaceId(n as u32)])),
                descriptors: Vec::new(),
                stock_to_leave: None,
                colour: None,
                keep_tangent_edges: None,
            }),
            Placement::End,
        )
        .unwrap();
    }
    let mut o = p.operation(op_id(101)).unwrap().clone();
    o.boundary = Some(SelectionSetId::from_u128(30));
    o.avoid = vec![SelectionSetId::from_u128(31)];
    o.check = vec![SelectionSetId::from_u128(32)];
    p.replace(Item::Operation(o)).unwrap();
    p
}

#[test]
fn req_prj_04_req_op_02_restriction_set_options_round_trip_and_older_files_take_the_defaults() {
    // Schema 14 (decision record 0176 item 3): each boundary, avoid and
    // check set of an operation carries its own options, written only when
    // they differ from the defaults.
    let plain = with_restrictions();
    let dir = temp_dir("restrict-plain");
    file::save(&plain, &dir).unwrap();
    let text = std::fs::read_to_string(dir.join(OP_101_FILE)).unwrap();
    assert!(
        !text.contains("_options"),
        "defaults are not written: {text}"
    );

    let mut p = plain.clone();
    let mut o = p.operation(op_id(101)).unwrap().clone();
    o.restriction_options.boundary = BoundaryOptions {
        limits_pass: true,
        limits_tip: false,
    };
    o.restriction_options.set_avoid(
        SelectionSetId::from_u128(31),
        SetOptions {
            on_break: OnBreak::Warn,
        },
    );
    o.restriction_options.set_check(
        SelectionSetId::from_u128(32),
        SetOptions {
            on_break: OnBreak::Ignore,
        },
    );
    p.replace(Item::Operation(o)).unwrap();
    let dir = temp_dir("restrict-set");
    file::save(&p, &dir).unwrap();
    let text = std::fs::read_to_string(dir.join(OP_101_FILE)).unwrap();
    for needle in [
        "[boundary_options]\nlimits_pass = true\nlimits_tip = false\n",
        "[avoid_options.00000000-0000-0000-0000-00000000001f]\non_break = \"warn\"\n",
        "[check_options.00000000-0000-0000-0000-000000000020]\non_break = \"ignore\"\n",
    ] {
        assert!(text.contains(needle), "{needle} in {text}");
    }
    assert_eq!(file::load(&dir).unwrap(), p);
    // A schema 14 file holding them opens through the later steps as the
    // same project, and saves again at the current schema with them intact.
    let at_14 = copy_tree_with(&read_tree(&dir), "restrict-set-14", |_, text| {
        text.replacen(&current(), "schema = 14\n", 1)
    });
    let opened = file::load(&at_14).unwrap();
    assert_eq!(opened, p);
    let again = temp_dir_saved(&opened, "restrict-set-14-saved");
    assert_eq!(read_tree(&again), read_tree(&dir));
    // An entry equal to the defaults is not written, and reads back as none.
    let mut q = p.clone();
    let mut o = q.operation(op_id(101)).unwrap().clone();
    o.restriction_options
        .avoid
        .insert(SelectionSetId::from_u128(31), SetOptions::default());
    q.replace(Item::Operation(o)).unwrap();
    let dir = temp_dir("restrict-default-entry");
    file::save(&q, &dir).unwrap();
    let back = file::load(&dir).unwrap();
    assert!(
        back.operation(op_id(101))
            .unwrap()
            .restriction_options
            .avoid
            .is_empty()
    );

    // A schema 13 file has none: its sets take the defaults, and it loads
    // as the same project.
    let base = read_tree(&temp_dir_saved(&plain, "restrict-base"));
    let old = copy_tree_with(&base, "restrict-13", |_, text| {
        text.replacen(&current(), "schema = 13\n", 1)
    });
    assert_eq!(file::load(&old).unwrap(), plain);
}

fn temp_dir_saved(p: &Project, name: &str) -> PathBuf {
    let dir = temp_dir(name);
    file::save(p, &dir).unwrap();
    dir
}

#[test]
fn req_prj_04_req_nfr_07_restriction_set_options_need_schema_14_and_their_set() {
    let base = read_tree(&temp_dir_saved(&with_restrictions(), "restrict-bad-base"));
    let edited = |name: &str, schema: i64, table: &str| {
        let dir = copy_tree_with(&base, name, |path, text| {
            let text = text.replacen(&current(), &format!("schema = {schema}\n"), 1);
            if path == OP_101_FILE {
                format!("{text}\n{table}")
            } else {
                text
            }
        });
        file::load(&dir).unwrap_err().to_string()
    };
    // In a schema 13 file, any of the three is refused, naming it.
    for key in ["boundary_options", "avoid_options", "check_options"] {
        let table = if key == "boundary_options" {
            "[boundary_options]\nlimits_pass = false\n".to_owned()
        } else {
            format!("[{key}.00000000-0000-0000-0000-00000000001f]\non_break = \"warn\"\n")
        };
        let err = edited(&format!("restrict-13-{key}"), 13, &table);
        assert!(
            err.contains(key) && err.contains("schema 14"),
            "{key}: {err}"
        );
    }
    // Options for a set that is not in that role, or an unknown choice or
    // field, are refused naming what is wrong.
    for (name, table, says) in [
        (
            "restrict-stray",
            "[check_options.00000000-0000-0000-0000-00000000001f]\non_break = \"warn\"\n",
            "is not one of the operation's check sets",
        ),
        (
            "restrict-choice",
            "[avoid_options.00000000-0000-0000-0000-00000000001f]\non_break = \"sometimes\"\n",
            "sometimes",
        ),
        (
            "restrict-field",
            "[boundary_options]\nlimits_everything = true\n",
            "limits_everything",
        ),
    ] {
        let err = edited(name, file::SCHEMA, table);
        assert!(err.contains(says), "{name}: {err}");
    }
    // Boundary options with no boundary set are refused.
    let mut p = with_restrictions();
    let mut o = p.operation(op_id(101)).unwrap().clone();
    o.boundary = None;
    o.restriction_options.boundary.limits_tip = false;
    let err = p.replace(Item::Operation(o)).unwrap_err().to_string();
    assert!(err.contains("no boundary set"), "{err}");
}

#[test]
fn req_sim_01_req_prj_06_hidden_toolpaths_are_saved_with_the_project() {
    // Issue 213 (decision record 0169): every path shown by default, and
    // nothing written.
    let plain = sample();
    let ops: Vec<OperationId> = plain.operations().iter().map(|o| o.id).collect();
    assert!(ops.len() >= 2);
    assert!(ops.iter().all(|id| plain.path_shown(*id)));
    for (path, text) in file::render(&plain).unwrap() {
        assert!(!text.contains("hidden_paths"), "{path}");
    }
    // Hidden: written in machining order however it was reached, and read
    // back; only the operations whose state changed are returned.
    let mut p = sample();
    assert_eq!(
        p.set_paths_shown(&[ops[1], ops[0]], false),
        [ops[1], ops[0]]
    );
    assert!(p.set_paths_shown(&[ops[0]], false).is_empty());
    let dir = temp_dir("hidden-paths");
    file::save(&p, &dir).unwrap();
    let text = std::fs::read_to_string(dir.join("project.toml")).unwrap();
    assert!(
        text.contains(&format!(
            "hidden_paths = [\n    \"{}\",\n    \"{}\",\n]",
            ops[0], ops[1]
        )),
        "{text}"
    );
    let loaded = file::load(&dir).unwrap();
    assert_eq!(loaded, p);
    assert!(!loaded.path_shown(ops[0]) && !loaded.path_shown(ops[1]));
    // Shown again: back to the plain file.
    assert_eq!(p.set_paths_shown(&ops, true), [ops[0], ops[1]]);
    assert_eq!(file::render(&p).unwrap(), file::render(&plain).unwrap());
    // A removed operation's choice is kept for an undo but never written.
    let mut p = sample();
    p.set_paths_shown(&[ops[0]], false);
    p.remove(ItemId::Operation(ops[0])).unwrap();
    assert!(!p.path_shown(ops[0]));
    let text = file::render(&p).unwrap().remove("project.toml").unwrap();
    assert!(!text.contains("hidden_paths"), "{text}");
}

#[test]
fn req_sim_03_req_prj_06_skipping_the_stock_simulation_is_saved_with_the_project() {
    // Issue 214 (decision record 0168): off by default, and not written.
    let plain = sample();
    assert!(!plain.skip_stock_simulation());
    for (path, text) in file::render(&plain).unwrap() {
        assert!(!text.contains("skip_stock_simulation"), "{path}");
    }
    let mut p = sample();
    assert!(!p.set_skip_stock_simulation(true));
    let dir = temp_dir("skip-stock");
    file::save(&p, &dir).unwrap();
    let text = std::fs::read_to_string(dir.join("project.toml")).unwrap();
    assert!(text.contains("\nskip_stock_simulation = true\n"), "{text}");
    let loaded = file::load(&dir).unwrap();
    assert_eq!(loaded, p);
    assert!(loaded.skip_stock_simulation());
}

#[test]
fn req_gc_07_view_state_and_the_skip_do_not_make_a_different_output() {
    // The export dialog's files stay valid when only these change.
    let plain = sample();
    let mut p = sample();
    p.set_skip_stock_simulation(true);
    p.set_paths_shown(&[plain.operations()[0].id], false);
    assert_ne!(p, plain);
    assert!(p.same_output(&plain) && plain.same_output(&p));
    let mut renamed = sample();
    renamed.set_name("Another");
    assert!(!renamed.same_output(&plain));
    let mut edited = sample();
    set_parameter(&mut edited, 101, 0.9);
    assert!(!edited.same_output(&plain));
}

#[test]
fn req_prj_04_req_nfr_07_the_schema_15_fields_need_schema_15() {
    let mut p = sample();
    p.set_skip_stock_simulation(true);
    p.set_paths_shown(&[p.operations()[0].id], false);
    let dir = temp_dir("schema-15");
    file::save(&p, &dir).unwrap();
    let old = copy_tree_with(&read_tree(&dir), "schema-15-as-14", |path, text| {
        if path == "project.toml" {
            text.replacen(&current(), "schema = 14\n", 1)
        } else {
            text.to_owned()
        }
    });
    let err = file::load(&old).unwrap_err().to_string();
    assert!(
        err.contains("skip_stock_simulation") && err.contains("schema 15"),
        "{err}"
    );
    // An older project without them loads with every path shown and the
    // stock simulated.
    let q = sample();
    let d = temp_dir("schema-13-plain");
    file::save(&q, &d).unwrap();
    let old = copy_tree_with(&read_tree(&d), "schema-13-plain-copy", |_, text| {
        text.replacen(&current(), "schema = 13\n", 1)
    });
    let loaded = file::load(&old).unwrap();
    assert_eq!(loaded, q);
    assert!(!loaded.skip_stock_simulation());
}

#[test]
fn req_tl_05_req_prj_04_an_operation_spindle_speed_round_trips_and_needs_schema_16() {
    let mut p = sample();
    let id = p.operations()[0].id;
    let mut edited = p.operation(id).unwrap().clone();
    edited.spindle_speed = Some(openchuck_types::units::SpindleSpeed(12000.0));
    p.replace(Item::Operation(edited)).unwrap();
    let dir = temp_dir("schema-16");
    file::save(&p, &dir).unwrap();
    let tree = read_tree(&dir);
    assert!(tree.values().any(|t| t.contains("spindle_speed = 12000")));
    assert_eq!(file::load(&dir).unwrap(), p);
    // Written only when set: the plain sample has no such line.
    let plain = temp_dir("schema-16-plain");
    file::save(&sample(), &plain).unwrap();
    assert!(
        !read_tree(&plain)
            .values()
            .any(|t| t.contains("spindle_speed"))
    );
    // A file that says schema 15 cannot hold it.
    let old = copy_tree_with(&tree, "schema-16-as-15", |_, text| {
        text.replacen(&current(), "schema = 15\n", 1)
    });
    let err = file::load(&old).unwrap_err().to_string();
    assert!(
        err.contains("spindle_speed") && err.contains("schema 16"),
        "{err}"
    );
    // An older file without it loads with none.
    let older = copy_tree_with(&read_tree(&plain), "schema-16-plain-as-15", |_, text| {
        text.replacen(&current(), "schema = 15\n", 1)
    });
    assert_eq!(file::load(&older).unwrap(), sample());
}

/// An operation's preset material (issue 197; decision 0230) is written
/// only when chosen, survives save and load, and needs schema 18.
#[test]
fn req_tl_05_req_prj_04_an_operation_material_round_trips_and_needs_schema_18() {
    let mut p = sample();
    let id = p.operations()[0].id;
    let mut edited = p.operation(id).unwrap().clone();
    edited.material = Some("Hard maple".to_owned());
    p.replace(Item::Operation(edited)).unwrap();
    let dir = temp_dir("schema-18");
    file::save(&p, &dir).unwrap();
    let tree = read_tree(&dir);
    assert!(
        tree.values()
            .any(|t| t.contains("material = \"Hard maple\""))
    );
    assert_eq!(file::load(&dir).unwrap(), p);
    // A file that says schema 17 cannot hold it.
    let old = copy_tree_with(&tree, "schema-18-as-17", |_, text| {
        text.replacen(&current(), "schema = 17\n", 1)
    });
    let err = file::load(&old).unwrap_err().to_string();
    assert!(
        err.contains("material") && err.contains("schema 18"),
        "{err}"
    );
    // An older operation without it loads with none: the setup's material.
    let plain = temp_dir("schema-18-plain");
    file::save(&sample(), &plain).unwrap();
    let older = copy_tree_with(&read_tree(&plain), "schema-18-plain-as-17", |_, text| {
        text.replacen(&current(), "schema = 17\n", 1)
    });
    let back = file::load(&older).unwrap();
    assert_eq!(back, sample());
    assert!(back.operations().iter().all(|o| o.material.is_none()));
}

/// The project's machine copy keeps a spindle speed range, written only
/// when set; a schema 17 copy cannot hold one (decision 0231).
#[test]
fn req_gc_14_req_prj_04_the_machine_copys_spindle_range_round_trips_and_needs_schema_18() {
    let mut p = sample();
    let mut machine = openchuck_types::machine::MachineProfile::example_x_rotary();
    machine.max_spindle_rpm = Some(openchuck_types::units::SpindleSpeed(13000.0));
    p.set_machine(Some(machine)).unwrap();
    let dir = temp_dir("schema-18-machine");
    file::save(&p, &dir).unwrap();
    let tree = read_tree(&dir);
    assert!(tree.values().any(|t| t.contains("max_spindle_rpm = 13000")));
    assert_eq!(file::load(&dir).unwrap(), p);
    let old = copy_tree_with(&tree, "schema-18-machine-as-17", |_, text| {
        text.replacen(&current(), "schema = 17\n", 1)
    });
    let err = file::load(&old).unwrap_err().to_string();
    assert!(
        err.contains("max_spindle_rpm") && err.contains("schema 18"),
        "{err}"
    );
}

#[test]
fn req_nfr_07_an_empty_operation_material_is_refused() {
    let mut p = sample();
    let id = p.operations()[0].id;
    for bad in ["", "   "] {
        let mut edited = p.operation(id).unwrap().clone();
        edited.material = Some(bad.to_owned());
        let err = p.replace(Item::Operation(edited)).unwrap_err().to_string();
        assert!(err.contains("material"), "{err}");
    }
}

#[test]
fn req_nfr_07_a_bad_operation_spindle_speed_is_refused() {
    let mut p = sample();
    let id = p.operations()[0].id;
    for bad in [0.0, -5.0, f64::NAN, f64::INFINITY] {
        let mut edited = p.operation(id).unwrap().clone();
        edited.spindle_speed = Some(openchuck_types::units::SpindleSpeed(bad));
        let err = p.replace(Item::Operation(edited)).unwrap_err().to_string();
        assert!(err.contains("spindle_speed"), "{err}");
    }
}

/// Schema 21 (issue 385, decision record 0307 item 4): a model reference may
/// record its original file name, written only when recorded; a schema 20
/// project loads unchanged, and a schema 20 file that already holds a name is
/// refused with what to do.
#[test]
fn req_prj_04_a_models_name_round_trips_and_schema_20_projects_load() {
    let plain = sample();
    let dir = temp_dir_saved(&plain, "model-name-plain");
    let text = std::fs::read_to_string(dir.join("project.toml")).unwrap();
    assert!(!text.contains("name = \"part"), "{text}");

    let mut p = plain.clone();
    let mut m = p.models()[0].clone();
    m.name = Some("Blade rev B.STL".to_owned());
    p.replace(Item::Model(m)).unwrap();
    let named = temp_dir_saved(&p, "model-name-set");
    let text = std::fs::read_to_string(named.join("project.toml")).unwrap();
    assert!(text.contains("name = \"Blade rev B.STL\"\n"), "{text}");
    assert_eq!(file::load(&named).unwrap(), p);

    // A schema 20 folder opens as the same project and saves at schema 21.
    let at_20 = copy_tree_with(&read_tree(&dir), "model-name-20", |_, text| {
        text.replacen(&current(), "schema = 20\n", 1)
    });
    let opened = file::load(&at_20).unwrap();
    assert_eq!(opened, plain);
    assert_eq!(
        read_tree(&temp_dir_saved(&opened, "model-name-20-saved")),
        read_tree(&dir)
    );

    // A schema 20 file with a name is refused, naming the field.
    let early = copy_tree_with(&read_tree(&named), "model-name-early", |_, text| {
        text.replacen(&current(), "schema = 20\n", 1)
    });
    let err = file::load(&early).unwrap_err().to_string();
    assert!(err.contains("`name`") && err.contains("schema 21"), "{err}");
}

/// Schema 23 (decision record 0344): helical finishing, axial finishing and
/// rest machining gain the per-operation `face_free_ends` switch, on for a
/// new operation. An operation of those strategies saved before schema 23
/// is migrated to an explicit `face_free_ends = false`, so a saved project
/// machines exactly as before; other strategies are not touched, and an
/// older file that already holds the parameter is refused.
#[test]
fn req_prj_04_req_op_04_older_finishing_operations_keep_free_ends_off() {
    let mut p = sample();
    let strategies = [
        (101, "helical_finishing"),
        (102, "axial_finishing"),
        (103, "rest_machining"),
        (104, "rotary_roughing"),
    ];
    for (n, strategy) in strategies {
        let mut op = p.operation(op_id(n)).unwrap().clone();
        op.strategy = strategy.into();
        p.replace(Item::Operation(op)).unwrap();
    }
    let dir = temp_dir_saved(&p, "free-ends-current");
    for older in [21, 22] {
        let at = copy_tree_with(
            &read_tree(&dir),
            &format!("free-ends-{older}"),
            |_, text| text.replacen(&current(), &format!("schema = {older}\n"), 1),
        );
        let opened = file::load(&at).unwrap();
        for (n, strategy) in strategies {
            let flag = opened
                .operation(op_id(n))
                .unwrap()
                .parameters
                .get("face_free_ends")
                .cloned();
            if strategy == "rotary_roughing" {
                assert_eq!(flag, None, "schema {older}, {strategy}");
            } else {
                assert_eq!(
                    flag,
                    Some(ParameterValue::Flag(false)),
                    "schema {older}, {strategy}"
                );
            }
        }
    }
    // A current file keeps what it holds: no switch written means none set.
    let opened = file::load(&dir).unwrap();
    assert_eq!(opened, p);

    // An older file that already holds the switch is refused, naming it.
    let mut early = p.clone();
    let mut op = early.operation(op_id(101)).unwrap().clone();
    op.parameters
        .insert("face_free_ends".into(), ParameterValue::Flag(true));
    early.replace(Item::Operation(op)).unwrap();
    let saved = temp_dir_saved(&early, "free-ends-early");
    let at_22 = copy_tree_with(&read_tree(&saved), "free-ends-early-22", |_, text| {
        text.replacen(&current(), "schema = 22\n", 1)
    });
    let err = file::load(&at_22).unwrap_err().to_string();
    assert!(
        err.contains("`face_free_ends`") && err.contains("schema 23"),
        "{err}"
    );
}

#[test]
fn req_prj_04_req_op_04_older_operations_keep_adaptive_spacing_off() {
    let mut p = sample();
    let strategies = [
        (101, "helical_finishing"),
        (102, "axial_finishing"),
        (103, "simultaneous_finishing"),
        (104, "rotary_roughing"),
        (105, "indexed_finishing"),
    ];
    // A fifth operation, for indexed finishing.
    let mut fifth = p.operation(op_id(104)).unwrap().clone();
    fifth.id = op_id(105);
    p.insert(Item::Operation(fifth), Placement::End).unwrap();
    for (n, strategy) in strategies {
        let mut op = p.operation(op_id(n)).unwrap().clone();
        op.strategy = strategy.into();
        p.replace(Item::Operation(op)).unwrap();
    }
    let dir = temp_dir_saved(&p, "adaptive-current");
    for older in [22, 23] {
        let at = copy_tree_with(&read_tree(&dir), &format!("adaptive-{older}"), |_, text| {
            text.replacen(&current(), &format!("schema = {older}\n"), 1)
        });
        let opened = file::load(&at).unwrap();
        for (n, strategy) in strategies {
            let flag = opened
                .operation(op_id(n))
                .unwrap()
                .parameters
                .get("adaptive_spacing")
                .cloned();
            // Rotary roughing gained the switch in schema 25 (OI-1453),
            // also as an explicit off.
            assert_eq!(
                flag,
                Some(ParameterValue::Flag(false)),
                "schema {older}, {strategy}"
            );
        }
    }
    // A current file keeps what it holds: no switch written means none set.
    let opened = file::load(&dir).unwrap();
    assert_eq!(opened, p);

    // An older file that already holds the switch is refused, naming it.
    let mut early = p.clone();
    let mut op = early.operation(op_id(101)).unwrap().clone();
    op.parameters
        .insert("adaptive_spacing".into(), ParameterValue::Flag(true));
    early.replace(Item::Operation(op)).unwrap();
    let saved = temp_dir_saved(&early, "adaptive-early");
    let at_23 = copy_tree_with(&read_tree(&saved), "adaptive-early-23", |_, text| {
        text.replacen(&current(), "schema = 23\n", 1)
    });
    let err = file::load(&at_23).unwrap_err().to_string();
    assert!(
        err.contains("`adaptive_spacing`") && err.contains("schema 24"),
        "{err}"
    );
}

#[test]
fn req_prj_04_req_op_04_older_roughing_and_facing_operations_keep_adaptive_spacing_off() {
    // Schema 25 (decision record 0239; OI-1453): rotary roughing and Facing
    // gain the switch; their older operations open with it off, and the
    // finishing operations keep what schema 24 gave them.
    let mut p = sample();
    let strategies = [
        (101, "rotary_roughing"),
        (102, "facing"),
        (103, "axial_finishing"),
    ];
    for (n, strategy) in strategies {
        let mut op = p.operation(op_id(n)).unwrap().clone();
        op.strategy = strategy.into();
        if strategy == "axial_finishing" {
            op.parameters
                .insert("adaptive_spacing".into(), ParameterValue::Flag(true));
        }
        p.replace(Item::Operation(op)).unwrap();
    }
    let dir = temp_dir_saved(&p, "line-adaptive-current");
    let at_24 = copy_tree_with(&read_tree(&dir), "line-adaptive-24", |_, text| {
        text.replacen(&current(), "schema = 24\n", 1)
    });
    let opened = file::load(&at_24).unwrap();
    let flag = |n: u128| {
        opened
            .operation(op_id(n))
            .unwrap()
            .parameters
            .get("adaptive_spacing")
            .cloned()
    };
    assert_eq!(flag(101), Some(ParameterValue::Flag(false)));
    assert_eq!(flag(102), Some(ParameterValue::Flag(false)));
    assert_eq!(flag(103), Some(ParameterValue::Flag(true)));
    // A current file keeps what it holds.
    assert_eq!(file::load(&dir).unwrap(), p);

    // A schema 24 roughing operation that already holds the switch is
    // refused, naming it.
    let mut early = p.clone();
    let mut op = early.operation(op_id(101)).unwrap().clone();
    op.parameters
        .insert("adaptive_spacing".into(), ParameterValue::Flag(true));
    early.replace(Item::Operation(op)).unwrap();
    let saved = temp_dir_saved(&early, "line-adaptive-early");
    let at_24 = copy_tree_with(&read_tree(&saved), "line-adaptive-early-24", |_, text| {
        text.replacen(&current(), "schema = 24\n", 1)
    });
    let err = file::load(&at_24).unwrap_err().to_string();
    assert!(
        err.contains("`adaptive_spacing`") && err.contains("schema 25"),
        "{err}"
    );
}
