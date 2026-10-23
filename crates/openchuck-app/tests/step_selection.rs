// SPDX-License-Identifier: AGPL-3.0-or-later
//! A STEP part's faces and edges through the application (slice 11 items 5
//! and 7; decision record 0066, OI-242): the scene draws the part with its
//! own edges, a pick names its face or edge, face and edge sets are saved
//! against the file's content hash, and they name the same faces and edges
//! after the project is reopened and the file is imported again (slice 11
//! review box 3). The keep-tangent-edges choice of an edge set, or the
//! project's default, reaches edge following. Needs the kernel:
//! `--features kernel-tests`.
#![cfg(feature = "kernel-tests")]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use openchuck_app::model::TransformStep;
use openchuck_app::pick::{EdgeId, EdgeRef, FaceId, PickMode, PickTolerance};
use openchuck_app::project::id::{ModelId, OperationId, SelectionSetId, SetupId, ToolId};
use openchuck_app::project::{
    Item, Operation, OrderKey, ParameterValue, Placement, SelectionTarget, Setup, ToolSnapshot,
};
use openchuck_app::scene::{SceneBrep, SceneView};
use openchuck_app::setup::{Millimetres, Stock, builtin};
use openchuck_app::{Application, Command, PostSelection, SelectionEdit, Threads};
use openchuck_types::ir::MoveKind;
use openchuck_types::tool::{ControllerMapping, CuttingData, Tool, ToolShape};
use openchuck_types::units::{FeedRate, SpindleSpeed};

/// A box with filleted edges: flat faces, cylindrical fillets and the
/// tangent edges between them (testdata/step/generate).
const FIXTURE: &str = "filleted_box";

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/step/valid")
        .join(format!("{FIXTURE}.step"))
}

/// A project in a new folder with the fixture copied into `models/` as
/// `name`, centred on the rotary axis.
fn project(tag: &str) -> (PathBuf, Application, ModelId) {
    let dir = std::env::temp_dir().join(format!("openchuck-step-sel-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut app = Application::create(&dir, "selection", Threads::Exactly(2)).unwrap();
    let model = add(&mut app, &dir, "part.step");
    (dir, app, model)
}

fn add(app: &mut Application, dir: &Path, name: &str) -> ModelId {
    std::fs::create_dir_all(dir.join("models")).unwrap();
    let file = dir.join("models").join(name);
    std::fs::copy(fixture(), &file).unwrap();
    app.add_model_with(&file, None, &[TransformStep::CentreOnRotaryAxis])
        .unwrap()
        .0
}

/// The scene item of `model` and its faces and edges.
fn part(view: &SceneView, model: ModelId) -> (usize, &SceneBrep) {
    let (index, mesh) = view
        .meshes
        .iter()
        .enumerate()
        .find(|(_, m)| m.model == Some(model))
        .expect("the part is in the scene");
    (
        index,
        mesh.brep
            .as_ref()
            .expect("a STEP part brings its faces and edges"),
    )
}

/// A ray onto the middle of triangle `t` of scene mesh `index`, along its
/// normal from outside.
fn ray_onto_triangle(view: &SceneView, index: usize, t: usize) -> ([f64; 3], [f64; 3]) {
    let mesh = &view.meshes[index].mesh;
    let [a, b, c] = mesh.triangles[t].map(|i| mesh.vertices[i as usize]);
    let centre = [
        (a.x + b.x + c.x) / 3.0,
        (a.y + b.y + c.y) / 3.0,
        (a.z + b.z + c.z) / 3.0,
    ];
    let n = (b - a).cross(c - a);
    let n = n * (1.0 / n.length());
    (
        [
            centre[0] + 100.0 * n.x,
            centre[1] + 100.0 * n.y,
            centre[2] + 100.0 * n.z,
        ],
        [-n.x, -n.y, -n.z],
    )
}

/// A ray onto the middle point of the `k`th drawn edge, from outside the
/// part, away from its centre.
fn ray_onto_edge(brep: &SceneBrep, k: usize) -> ([f64; 3], [f64; 3]) {
    let all: Vec<_> = brep.edges.iter().flat_map(|e| e.points.iter()).collect();
    let n = all.len() as f64;
    let centre = all.iter().fold([0.0; 3], |s, p| {
        [s[0] + p.x / n, s[1] + p.y / n, s[2] + p.z / n]
    });
    let points = &brep.edges[k].points;
    let m = points[points.len() / 2];
    let d = [m.x - centre[0], m.y - centre[1], m.z - centre[2]];
    let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    let d = d.map(|x| x / l);
    (
        [m.x + 100.0 * d[0], m.y + 100.0 * d[1], m.z + 100.0 * d[2]],
        d.map(|x| -x),
    )
}

const REACH: PickTolerance = PickTolerance {
    base: 0.3,
    per_distance: 0.0,
};

/// What a face pick and an edge pick name on `model` in `view`.
fn picks(view: &SceneView, model: ModelId) -> (FaceId, BTreeSet<EdgeId>) {
    let (index, brep) = part(view, model);
    let picker = view.picking.picker().expect("the scene can be picked on");
    let (o, d) = ray_onto_triangle(view, index, 0);
    let pick = picker.pick(o, d).unwrap().expect("the ray meets the part");
    assert_eq!(pick.model, Some(model));
    let face = pick.face.expect("a pick on a STEP part names its face");
    assert_eq!(
        picker.select_faces(&pick, PickMode::Triangle),
        Some(BTreeSet::from([face]))
    );
    // The middle drawn edge, picked where it is and from outside the part.
    let k = brep.edges.len() / 2;
    let (o, d) = ray_onto_edge(brep, k);
    let edge = picker
        .pick_edge(o, d, REACH)
        .unwrap()
        .expect("the ray passes over the edge");
    assert_eq!(edge.edge, EdgeRef::Brep(brep.edges[k].id));
    let chain = picker
        .select_edges(&edge, PickMode::Connected)
        .into_iter()
        .map(|e| match e {
            EdgeRef::Brep(id) => id,
            EdgeRef::Mesh(_) => panic!("a STEP part has no mesh edges"),
        })
        .collect();
    (face, chain)
}

#[test]
fn req_ui_04_req_prj_06_faces_and_edges_of_a_step_file_are_the_same_after_reopening_and_a_second_import()
 {
    let (dir, mut app, model) = project("reopen");
    let view = app.start_scene(None).wait().unwrap();
    let (_, brep) = part(&view, model);
    let first = brep.clone();
    assert!(!first.edges.is_empty());
    // Every edge is drawn, tangent ones included (a fillet meets its faces
    // with no turn), so each face's outline shows.
    assert!(first.edges.iter().any(|e| e.turn.is_some_and(|t| t < 1.0)));
    let (face, chain) = picks(&view, model);

    // Saved as sets against the file's hash; a triangle set is refused.
    let faces = app
        .save_selection("Face", model, BTreeSet::from([face]))
        .unwrap();
    let edges = app.save_selection("Edges", model, chain.clone()).unwrap();
    let triangles: BTreeSet<u32> = [0].into();
    let e = app
        .save_selection("Triangles", model, triangles)
        .unwrap_err()
        .to_string();
    assert!(e.contains("face and edge"), "{e}");
    app.save().unwrap();
    drop(app);

    // Reopened: the file is read again, and the scene, the picks and the
    // sets name the same faces and edges.
    let mut app = Application::open(&dir, Threads::Exactly(2)).unwrap();
    let view = app.start_scene(None).wait().unwrap();
    assert_eq!(part(&view, model).1, &first);
    assert_eq!(picks(&view, model), (face, chain.clone()));
    let target = |app: &Application, id: SelectionSetId| {
        app.project()
            .selection_sets()
            .iter()
            .find(|s| s.id == id)
            .unwrap()
            .target
            .clone()
    };
    assert_eq!(target(&app, faces), SelectionTarget::Faces([face].into()));
    assert_eq!(target(&app, edges), SelectionTarget::Edges(chain.clone()));
    // The sets still check against the part read now.
    app.edit_selection(edges, SelectionEdit::Replace(chain.clone().into()))
        .unwrap();

    // The same file imported a second time gives the same IDs.
    let again = add(&mut app, &dir, "again.step");
    let view = app.start_scene(None).wait().unwrap();
    let (_, second) = part(&view, again);
    assert_eq!(second.faces, first.faces);
    let ids = |b: &SceneBrep| b.edges.iter().map(|e| e.id).collect::<Vec<_>>();
    assert_eq!(ids(second), ids(&first));
    std::fs::remove_dir_all(dir).unwrap();
}

/// The reference jobs' 3 mm ball in oak.
fn ball() -> ToolSnapshot {
    let mut cutting_data = BTreeMap::new();
    cutting_data.insert(
        "oak".to_owned(),
        CuttingData {
            spindle_speed: SpindleSpeed(12000.0),
            feed_per_tooth: Millimetres(0.02),
            plunge_feed: FeedRate(150.0),
            stepover: Millimetres(1.0),
            stepdown: Millimetres(1.0),
        },
    );
    ToolSnapshot {
        id: ToolId::new(),
        order: OrderKey::default(),
        name: "3 mm ball".into(),
        tool: Tool {
            shape: ToolShape::BallEnd,
            diameter: Millimetres(3.0),
            flute_length: Millimetres(8.0),
            overall_length: Millimetres(40.0),
            shank_diameter: Millimetres(3.0),
            flute_count: 2,
            material: None,
            centre_cutting: None,
        },
        controller: ControllerMapping {
            number: 1,
            length_offset: None,
            diameter_offset: None,
        },
        cutting_data,
    }
}

#[test]
fn req_op_08_the_tangent_edge_choice_of_a_set_or_the_project_reaches_edge_following_on_a_step_part()
{
    let (dir, mut app, model) = project("tangent");
    let view = app.start_scene(None).wait().unwrap();
    let (_, brep) = part(&view, model);
    // The part's extent in the model frame, for the stock around it.
    let points: Vec<_> = brep.edges.iter().flat_map(|e| e.points.clone()).collect();
    let (min_x, max_x) = points
        .iter()
        .fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.x), b.max(p.x)));
    let radius = points
        .iter()
        .map(|p| p.y.hypot(p.z))
        .fold(0.0_f64, f64::max);
    let every: BTreeSet<EdgeId> = brep.edges.iter().map(|e| e.id).collect();
    let set = app.save_selection("All edges", model, every).unwrap();

    app.execute(Command::SetMachine(Some(Box::new(
        builtin("makera-z1").unwrap().unwrap(),
    ))))
    .unwrap();
    let setup_id = SetupId::new();
    let mut setup = Setup::new(setup_id, "Setup 1");
    setup.part = Some(model);
    setup.material = Some("oak".into());
    setup.stock = Some(Stock::Cylinder {
        diameter: Millimetres(2.0 * radius + 2.0),
        length: Millimetres(max_x - min_x + 2.0),
        x_start: Millimetres(min_x - 1.0),
    });
    let snapshot = ball();
    let tool = snapshot.id;
    for item in [
        Item::Setup(setup),
        Item::Tool(snapshot),
        Item::Operation(Operation {
            enabled: true,
            id: OperationId::new(),
            order: OrderKey::default(),
            name: "Edges".into(),
            setup: setup_id,
            strategy: "edge_following".into(),
            tool: Some(tool),
            selections: vec![set],
            boundary: None,
            avoid: Vec::new(),
            check: Vec::new(),
            restriction_options: Default::default(),
            spindle_speed: None,
            parameters: [("depth".to_owned(), ParameterValue::Number(0.2))].into(),
        }),
    ] {
        app.execute(Command::Add {
            item,
            placement: Placement::End,
        })
        .unwrap();
    }
    app.set_post(Some(PostSelection::parse("linuxcnc")));
    let cuts = |app: &Application| match app.regenerate() {
        Ok(result) => result.toolpath.groups[0]
            .segments
            .iter()
            .filter(|s| s.kind == MoveKind::Cutting)
            .map(|s| s.end.tip)
            .collect::<Vec<_>>(),
        // With the tangent edges dropped, the selected fillet edges are not
        // followed and there may be nothing left.
        Err(_) => Vec::new(),
    };

    // The set does not say and the project's default is off: the edges where
    // a fillet meets a face are dropped.
    let dropped = cuts(&app);
    // The set keeps them: more is followed.
    app.edit_selection(set, SelectionEdit::KeepTangentEdges(Some(true)))
        .unwrap();
    let kept = cuts(&app);
    assert!(
        kept.len() > dropped.len(),
        "{} <= {}",
        kept.len(),
        dropped.len()
    );
    // The project's default does the same for a set that does not say, and
    // the set's own choice wins over it.
    app.edit_selection(set, SelectionEdit::KeepTangentEdges(None))
        .unwrap();
    app.execute(Command::SetKeepTangentEdges(true)).unwrap();
    assert_eq!(cuts(&app), kept);
    app.edit_selection(set, SelectionEdit::KeepTangentEdges(Some(false)))
        .unwrap();
    assert_eq!(cuts(&app), dropped);
    std::fs::remove_dir_all(dir).unwrap();
}
