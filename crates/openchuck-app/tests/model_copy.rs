// SPDX-License-Identifier: AGPL-3.0-or-later
//! A model from outside the project folder is copied into it, so the folder
//! can be moved, zipped, shared or committed and still open (issue 385,
//! decision record 0307; PRJ-02, PRJ-04, PRJ-06, GC-07, NFR-07).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use openchuck_app::model::TransformStep;
use openchuck_app::overview::Severity;
use openchuck_app::project::id::{OperationId, SelectionSetId, SetupId, ToolId};
use openchuck_app::project::{
    ContentHash, Item, ModelCheck, ModelRef, Operation, OrderKey, Placement, SelectionSet,
    SelectionTarget, Setup, ToolSnapshot,
};
use openchuck_app::setup::{MachineProfile, Millimetres, Stock};
use openchuck_app::{AppError, Application, Command, JobLog, ModelStorage, PostSelection, Threads};
use openchuck_types::tool::{ControllerMapping, CuttingData, Tool, ToolShape};
use openchuck_types::units::{FeedRate, SpindleSpeed};

fn corpus(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/stl")
        .join(name)
}

/// A scratch folder holding a project folder `project` and, beside it, a
/// folder `elsewhere` with the corpus files `files`. Both are in the same
/// temporary folder, so a link between them is possible on every platform.
struct Scratch {
    root: PathBuf,
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

impl Scratch {
    fn new(name: &str, files: &[&str]) -> Self {
        let root =
            std::env::temp_dir().join(format!("openchuck-copy-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("elsewhere")).unwrap();
        for file in files {
            std::fs::copy(corpus(file), root.join("elsewhere").join(file)).unwrap();
        }
        Self { root }
    }

    fn project(&self) -> PathBuf {
        self.root.join("project")
    }

    fn outside(&self, file: &str) -> PathBuf {
        self.root.join("elsewhere").join(file)
    }

    fn create(&self) -> Application {
        Application::create(&self.project(), "copy", Threads::Exactly(2)).unwrap()
    }
}

fn hex32(hash: &ContentHash) -> String {
    hash.to_string()["blake3:".len()..][..32].to_owned()
}

#[test]
fn req_prj_02_importing_from_outside_copies_into_models_and_keeps_the_hash() {
    let s = Scratch::new("import", &["cube_ascii.stl"]);
    let mut app = s.create();
    let original = std::fs::read(s.outside("cube_ascii.stl")).unwrap();
    let hash = ContentHash::of_bytes(&original);

    let (id, _) = app.add_model(&s.outside("cube_ascii.stl"), None).unwrap();
    let model = app.find_model(&id.to_string()).unwrap().clone();
    let expected = format!("models/{}.stl", hex32(&hash));
    assert_eq!(model.path.as_str(), expected);
    assert_eq!(model.hash, hash, "the copy keeps the file's hash (PRJ-02)");
    assert_eq!(model.name.as_deref(), Some("cube_ascii.stl"));
    assert_eq!(model.display_name(), "cube_ascii.stl");
    assert!(!model.path.is_outside_project());
    assert_eq!(
        std::fs::read(s.project().join(&expected)).unwrap(),
        original
    );
    assert_eq!(
        std::fs::read(s.outside("cube_ascii.stl")).unwrap(),
        original,
        "the original is not changed"
    );
    // No staging file is left behind.
    let names: Vec<_> = std::fs::read_dir(s.project().join("models"))
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(names.len(), 1, "{names:?}");
    // Undo leaves the copied file; adding again reuses it.
    assert!(app.undo().unwrap());
    assert!(s.project().join(&expected).is_file());
    assert!(app.redo().unwrap());
    app.save().unwrap();

    // The folder moves, with nothing beside it, and still opens.
    let moved = s.root.join("moved").join("deeper");
    std::fs::create_dir_all(moved.parent().unwrap()).unwrap();
    std::fs::rename(s.project(), &moved).unwrap();
    std::fs::remove_dir_all(s.root.join("elsewhere")).unwrap();
    let opened = Application::open(&moved, Threads::Exactly(1)).unwrap();
    assert_eq!(
        opened.check_models().unwrap(),
        [(id, ModelCheck::Unchanged)]
    );
    assert!(opened.model_file_warnings().is_empty());
    assert!(opened.linked_models().is_empty());
}

#[test]
fn req_prj_02_the_same_contents_are_reused_and_a_different_file_is_never_overwritten() {
    let s = Scratch::new("clash", &["cube_ascii.stl", "cube_binary.stl"]);
    let mut app = s.create();
    let hash = ContentHash::of_file(&s.outside("cube_ascii.stl")).unwrap();
    let first = format!("models/{}.stl", hex32(&hash));

    // A file inside the project folder is used where it is, never copied.
    std::fs::create_dir_all(s.project().join("parts")).unwrap();
    std::fs::copy(corpus("two_cubes.stl"), s.project().join("parts/two.stl")).unwrap();
    let (inside, _) = app
        .add_model(&s.project().join("parts/two.stl"), None)
        .unwrap();
    let inside = app.find_model(&inside.to_string()).unwrap();
    assert_eq!(inside.path.as_str(), "parts/two.stl");
    assert_eq!(inside.name, None, "the stored name is the file's own");

    // Something else already holds the first name: it is kept, and the copy
    // takes the next one.
    std::fs::create_dir_all(s.project().join("models")).unwrap();
    std::fs::write(s.project().join(&first), b"solid other\nendsolid other\n").unwrap();
    let (a, _) = app.add_model(&s.outside("cube_ascii.stl"), None).unwrap();
    let second = format!("models/{}-2.stl", hex32(&hash));
    assert_eq!(
        app.find_model(&a.to_string()).unwrap().path.as_str(),
        second
    );
    assert_eq!(
        std::fs::read(s.project().join(&first)).unwrap(),
        b"solid other\nendsolid other\n"
    );
    // The same bytes again share the copy: nothing new is written.
    let (b, _) = app.add_model(&s.outside("cube_ascii.stl"), None).unwrap();
    assert_eq!(
        app.find_model(&b.to_string()).unwrap().path.as_str(),
        second
    );
    // Different bytes get their own name.
    let (c, _) = app.add_model(&s.outside("cube_binary.stl"), None).unwrap();
    let binary = ContentHash::of_file(&s.outside("cube_binary.stl")).unwrap();
    assert_eq!(
        app.find_model(&c.to_string()).unwrap().path.as_str(),
        format!("models/{}.stl", hex32(&binary))
    );
    assert_eq!(
        std::fs::read_dir(s.project().join("models"))
            .unwrap()
            .count(),
        3
    );
}

#[test]
fn req_prj_02_a_linked_model_stays_supported_and_can_be_copied_in_later() {
    let s = Scratch::new("link", &["cube_ascii.stl"]);
    let mut app = s.create();
    let (id, _) = app
        .add_model_as(&s.outside("cube_ascii.stl"), None, ModelStorage::Link)
        .unwrap();
    app.execute(Command::SetModelTransform {
        model: id,
        transform: vec![TransformStep::CentreOnRotaryAxis],
    })
    .unwrap();
    let linked = app.find_model(&id.to_string()).unwrap().clone();
    assert_eq!(linked.path.as_str(), "../elsewhere/cube_ascii.stl");
    assert!(linked.path.is_outside_project());
    assert_eq!(linked.name, None);
    assert!(!s.project().join("models").exists(), "nothing was copied");
    assert_eq!(app.linked_models(), [id]);
    // Messages has the same note, as information (decision record 0307 item 8).
    let checks = app.start_checks(None).wait().unwrap();
    let overview = app.overview(Some(&checks), &JobLog::default());
    assert!(
        overview
            .messages
            .iter()
            .any(|m| m.severity == Severity::Info
                && m.text
                    .starts_with("'cube_ascii.stl' is outside the project folder")),
        "{:?}",
        overview.messages
    );
    assert_eq!(
        app.linked_model_notes(),
        [
            "'cube_ascii.stl' is outside the project folder, so the folder is not \
          self-contained. Model inspector, Copy model into project."
        ]
    );

    // A selection made on the linked file (PRJ-06).
    let set = SelectionSet {
        id: SelectionSetId::new(),
        order: OrderKey::default(),
        name: "Top".into(),
        model: id,
        model_hash: linked.hash,
        target: SelectionTarget::Triangles(BTreeSet::from([0, 1])),
        descriptors: Vec::new(),
        stock_to_leave: None,
        colour: None,
        keep_tangent_edges: None,
    };
    app.execute(Command::Add {
        item: Item::SelectionSet(set.clone()),
        placement: Placement::End,
    })
    .unwrap();

    let path = app.copy_model_into_project(id).unwrap();
    let copied = app.find_model(&id.to_string()).unwrap().clone();
    assert_eq!(path, copied.path);
    assert_eq!(
        copied.path.as_str(),
        format!("models/{}.stl", hex32(&linked.hash))
    );
    assert_eq!(copied.name.as_deref(), Some("cube_ascii.stl"));
    assert_eq!(
        ModelRef {
            path: linked.path.clone(),
            name: None,
            ..copied.clone()
        },
        linked,
        "only the stored path and name change"
    );
    assert_eq!(app.project().selection_sets(), std::slice::from_ref(&set));
    assert_eq!(app.project().selection_sets()[0].model_hash, copied.hash);
    assert!(app.linked_models().is_empty());
    assert!(app.linked_model_notes().is_empty());
    // One undoable step; the copied file stays.
    assert!(app.undo().unwrap());
    assert_eq!(app.find_model(&id.to_string()).unwrap(), &linked);
    assert!(s.project().join(copied.path.as_str()).is_file());
    // A model already in the folder is not copied again.
    assert!(app.redo().unwrap());
    let err = app.copy_model_into_project(id).unwrap_err().to_string();
    assert!(err.contains("already in the project folder"), "{err}");
}

#[test]
fn req_prj_02_req_nfr_07_a_changed_or_missing_linked_file_is_refused() {
    let s = Scratch::new("refuse", &["cube_ascii.stl", "cube_binary.stl"]);
    let mut app = s.create();
    let (changed, _) = app
        .add_model_as(&s.outside("cube_ascii.stl"), None, ModelStorage::Link)
        .unwrap();
    let (missing, _) = app
        .add_model_as(&s.outside("cube_binary.stl"), None, ModelStorage::Link)
        .unwrap();
    std::fs::write(
        s.outside("cube_ascii.stl"),
        "solid edited\nendsolid edited\n",
    )
    .unwrap();
    std::fs::remove_file(s.outside("cube_binary.stl")).unwrap();
    let before = app.project().clone();

    let err = app
        .copy_model_into_project(changed)
        .unwrap_err()
        .to_string();
    assert_eq!(
        err,
        "'cube_ascii.stl' has changed since it was added, so copying it would change the \
         model. Check it first with Replace file…, which also copies it."
    );
    let err = app
        .copy_model_into_project(missing)
        .unwrap_err()
        .to_string();
    assert_eq!(
        err,
        "'../elsewhere/cube_binary.stl' is missing, so it cannot be copied. Use Replace \
         file… to choose it again."
    );
    let (copied, refused) = app.copy_linked_models();
    assert!(copied.is_empty());
    assert_eq!(refused.len(), 2);
    assert_eq!(app.project(), &before, "nothing changed");
    assert!(!s.project().join("models").exists(), "nothing was written");
}

#[test]
fn req_nfr_07_a_copy_that_cannot_be_written_adds_nothing() {
    let s = Scratch::new("unwritable", &["cube_ascii.stl"]);
    let mut app = s.create();
    // A file where the models folder should be.
    std::fs::write(s.project().join("models"), b"not a folder").unwrap();
    let err = app
        .add_model(&s.outside("cube_ascii.stl"), None)
        .unwrap_err();
    assert!(matches!(err, AppError::Io { .. }), "{err}");
    assert!(err.to_string().contains("models"), "{err}");
    assert!(app.project().models().is_empty());
    assert!(!app.can_undo());
    // A linked file on another drive cannot be linked; copying is the way
    // (here, a missing source is a clear error, not a crash).
    let err = app
        .add_model(&s.outside("none.stl"), None)
        .unwrap_err()
        .to_string();
    assert!(err.contains("cannot access"), "{err}");
}

/// The setup sheet's Part row names the original file and where it is
/// stored; a model without a recorded name shows its path as before.
#[test]
fn req_prj_02_a_model_label_names_the_original_and_the_stored_file() {
    let s = Scratch::new("label", &["cube_ascii.stl"]);
    let mut app = s.create();
    let (id, _) = app.add_model(&s.outside("cube_ascii.stl"), None).unwrap();
    let model = app.find_model(&id.to_string()).unwrap();
    assert_eq!(
        model.label(),
        format!("cube_ascii.stl ({})", model.path.as_str())
    );
    // The CLI finds it by its name too.
    assert_eq!(app.find_model("cube_ascii.stl").unwrap().id, id);
}

fn ball() -> Tool {
    Tool {
        shape: ToolShape::BallEnd,
        diameter: Millimetres(4.0),
        flute_length: Millimetres(10.0),
        overall_length: Millimetres(40.0),
        shank_diameter: Millimetres(4.0),
        flute_count: 2,
        material: None,
        centre_cutting: None,
    }
}

/// Adds a setup, a tool and a finishing operation on model `model`.
fn machine_it(app: &mut Application, model: openchuck_app::project::id::ModelId) -> OperationId {
    app.execute(Command::SetModelTransform {
        model,
        transform: vec![TransformStep::CentreOnRotaryAxis],
    })
    .unwrap();
    app.execute(Command::SetMachine(Some(Box::new(
        MachineProfile::example_x_rotary(),
    ))))
    .unwrap();
    let setup_id = SetupId::new();
    let mut setup = Setup::new(setup_id, "Setup 1");
    setup.part = Some(model);
    setup.material = Some("oak".into());
    setup.stock = Some(Stock::Cylinder {
        diameter: Millimetres(20.0),
        length: Millimetres(14.0),
        x_start: Millimetres(-7.0),
    });
    let tool_id = ToolId::new();
    let mut cutting_data = BTreeMap::new();
    cutting_data.insert(
        "oak".to_owned(),
        CuttingData {
            spindle_speed: SpindleSpeed(10000.0),
            feed_per_tooth: Millimetres(0.02),
            plunge_feed: FeedRate(150.0),
            stepover: Millimetres(0.8),
            stepdown: Millimetres(1.0),
        },
    );
    let op = OperationId::new();
    for item in [
        Item::Setup(setup),
        Item::Tool(ToolSnapshot {
            id: tool_id,
            order: OrderKey::default(),
            name: "4 mm ball".into(),
            tool: ball(),
            controller: ControllerMapping {
                number: 1,
                length_offset: None,
                diameter_offset: None,
            },
            cutting_data,
        }),
        Item::Operation(Operation {
            enabled: true,
            id: op,
            order: OrderKey::default(),
            name: "Finish".into(),
            setup: setup_id,
            strategy: "helical_finishing".into(),
            tool: Some(tool_id),
            selections: Vec::new(),
            boundary: None,
            avoid: Vec::new(),
            check: Vec::new(),
            restriction_options: Default::default(),
            spindle_speed: None,
            material: None,
            parameters: BTreeMap::new(),
        }),
    ] {
        app.execute(Command::Add {
            item,
            placement: Placement::End,
        })
        .unwrap();
    }
    app.set_post(Some(PostSelection::parse("makera-z1")));
    op
}

/// Copying a linked model changes neither the G-code nor whether the
/// toolpaths are current: the cache key follows the file's contents and
/// format, not where it is stored (decision record 0307 item 7; GC-07).
#[test]
fn req_gc_07_copying_a_model_keeps_the_g_code_and_the_toolpaths_current() {
    let s = Scratch::new("gcode", &["cube_ascii.stl"]);
    let mut app = s.create();
    let (model, _) = app
        .add_model_as(&s.outside("cube_ascii.stl"), None, ModelStorage::Link)
        .unwrap();
    let op = machine_it(&mut app, model);
    let linked = app.regenerate().unwrap();
    assert_eq!(linked.generated, [op]);

    app.copy_model_into_project(model).unwrap();
    let checks = app.start_checks(None).wait().unwrap();
    let overview = app.overview(Some(&checks), &JobLog::default());
    assert!(overview.stale.is_empty(), "nothing is out of date");
    let copied = app.regenerate().unwrap();
    assert!(copied.generated.is_empty(), "every result is reused");
    assert_eq!(copied.gcode, linked.gcode, "byte-identical G-code");

    // Saved, moved without its neighbour and opened at another thread count.
    app.save().unwrap();
    let moved = s.root.join("moved");
    std::fs::rename(s.project(), &moved).unwrap();
    std::fs::remove_dir_all(s.root.join("elsewhere")).unwrap();
    let mut opened = Application::open(&moved, Threads::Exactly(1)).unwrap();
    opened.set_post(Some(PostSelection::parse("makera-z1")));
    assert_eq!(opened.regenerate().unwrap().gcode, linked.gcode);
}

#[test]
fn req_prj_02_the_import_dialog_knows_whether_a_file_is_in_the_project_folder() {
    let s = Scratch::new("where", &["cube_ascii.stl"]);
    let app = s.create();
    std::fs::create_dir_all(s.project().join("models")).unwrap();
    std::fs::copy(corpus("cube_ascii.stl"), s.project().join("models/in.stl")).unwrap();
    assert!(app.is_in_project_folder(&s.project().join("models/in.stl")));
    assert!(!app.is_in_project_folder(&s.outside("cube_ascii.stl")));
    assert!(!app.is_in_project_folder(&s.outside("none.stl")));
}
