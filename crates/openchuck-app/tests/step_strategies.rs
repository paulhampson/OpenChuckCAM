// SPDX-License-Identifier: AGPL-3.0-or-later
//! Every strategy on STEP parts through the application (decision record
//! 0056 item 2: a STEP part is machined on its exact `BrepSurface`). Each
//! corpus part is added from its STEP file, placed on the rotary axis and
//! machined with a 3 mm ball in oak on the Makera Z1; each job passes gates 1
//! and 2 on both posts, gives byte-identical G-code at 1, 2 and N threads
//! (GC-07, NFR-06), and does not gouge the exact B-rep: the independent
//! checker of `openchuck_testkit::gouge` (its tool outline only) is run
//! against points sampled on the exact faces, never against the tessellation
//! the tool drops onto. Needs the kernel: `--features kernel-tests`.
#![cfg(feature = "kernel-tests")]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::print_stdout
)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use openchuck_app::model::{Axis, Degrees, TransformStep};
use openchuck_app::project::id::{OperationId, SelectionSetId, SetupId, ToolId};
use openchuck_app::project::{
    ContentHash, Item, Operation, OrderKey, ParameterValue, Placement, SelectionSet,
    SelectionTarget, Setup, ToolSnapshot,
};
use openchuck_app::setup::{Millimetres, Stock, builtin};
use openchuck_app::{Application, Command, PostSelection, Threads};
use openchuck_geom::transform::{brep_placement, transform_brep};
use openchuck_import::step::StepImporter;
use openchuck_import::{ImportOptions, ModelImporter};
use openchuck_path::surface::{ParametricFace, Uv};
use openchuck_path::{BrepEdges, BrepSurface, CurveSource, ExactFace, SurfaceSource, ToolLine};
use openchuck_testkit::gouge::Outline;
use openchuck_types::brep::BrepModel;
use openchuck_types::geometry::{Point3, UnitVec3, Vec3};
use openchuck_types::ir::{MoveKind, Toolpath};
use openchuck_types::job::{CancelToken, JobContext, NoProgress};
use openchuck_types::model::ModelGeometry;
use openchuck_types::tool::{ControllerMapping, CuttingData, Tool, ToolShape};
use openchuck_types::units::{FeedRate, SpindleSpeed};

/// The deepest the tool may reach into the exact surface: the tessellation
/// it drops onto lies inside a convex face by up to the import's linear
/// deflection (0.01 mm), the path tolerance adds to that, and samples 0.25 mm
/// apart miss up to 0.005 mm of a 1.5 mm ball's reach.
const GOUGE_LIMIT: f64 = 0.03;

/// Spacing of the samples on the exact faces, mm.
const SAMPLE: f64 = 0.25;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/step/valid")
        .join(format!("{name}.step"))
}

/// Parts made along Z are turned onto the rotary axis and centred on it.
fn onto_axis() -> Vec<TransformStep> {
    vec![
        TransformStep::Rotate {
            axis: Axis::Y,
            angle: Degrees(90.0),
        },
        TransformStep::CentreOnRotaryAxis,
    ]
}

/// The fixture imported by the kernel and placed by `transform`, as the
/// application places it.
fn placed(name: &str, transform: &[TransformStep]) -> BrepModel {
    let bytes = std::fs::read(fixture(name)).unwrap();
    let cancel = CancelToken::new();
    let imported = StepImporter::new()
        .import(
            &bytes,
            &ImportOptions::default(),
            &JobContext::new(&cancel, &NoProgress),
        )
        .unwrap();
    let ModelGeometry::Brep(model) = imported.geometry else {
        panic!("{name} is not a B-rep");
    };
    let map = brep_placement(&model, transform).unwrap();
    transform_brep(&model, &map).unwrap()
}

fn ball() -> ToolSnapshot {
    let mut cutting_data = BTreeMap::new();
    cutting_data.insert(
        "oak".to_owned(),
        CuttingData {
            spindle_speed: SpindleSpeed(12000.0),
            feed_per_tooth: Millimetres(0.02),
            plunge_feed: FeedRate(150.0),
            stepover: Millimetres(2.0),
            stepdown: Millimetres(2.0),
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
            centre_cutting: Some(true),
        },
        controller: ControllerMapping {
            number: 1,
            length_offset: None,
            diameter_offset: None,
        },
        cutting_data,
    }
}

/// One job: a corpus part, its placement, and the operations of its setup.
struct Job {
    part: &'static str,
    transform: Vec<TransformStep>,
    operations: Vec<(&'static str, Vec<(&'static str, ParameterValue)>)>,
}

fn n(v: f64) -> ParameterValue {
    ParameterValue::Number(v)
}

/// Builds the job's project in a new folder: the STEP file copied into
/// `models/`, a cylinder of stock 1 mm larger in radius than the part and
/// 1 mm longer at each end, and the operations in order.
fn build(job: &Job, tag: &str) -> (PathBuf, Application, ContentHash) {
    let dir = std::env::temp_dir().join(format!(
        "openchuck-step-strategy-{tag}-{}-{}-{}",
        job.part,
        job.operations.first().map_or("none", |o| o.0),
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let mut app = Application::create(&dir, job.part, Threads::Exactly(2)).unwrap();
    std::fs::create_dir_all(dir.join("models")).unwrap();
    let file = dir.join(format!("models/{}.step", job.part));
    std::fs::copy(fixture(job.part), &file).unwrap();
    let hash = ContentHash::of_bytes(&std::fs::read(&file).unwrap());
    let (model, inspection) = app.add_model_with(&file, None, &job.transform).unwrap();
    let extent = inspection.extent.unwrap();
    app.execute(Command::SetMachine(Some(Box::new(
        builtin("makera-z1").unwrap().unwrap(),
    ))))
    .unwrap();
    let setup_id = SetupId::new();
    let mut setup = Setup::new(setup_id, "Setup 1");
    setup.part = Some(model);
    setup.material = Some("oak".into());
    setup.stock = Some(Stock::Cylinder {
        diameter: Millimetres(2.0 * extent.radius.0 + 2.0),
        length: Millimetres(extent.bounds.max.x - extent.bounds.min.x + 2.0),
        x_start: Millimetres(extent.bounds.min.x - 1.0),
    });
    let tool = ball();
    let tool_id = tool.id;
    let mut items = vec![Item::Setup(setup), Item::Tool(tool)];
    for (i, (strategy, parameters)) in job.operations.iter().enumerate() {
        items.push(Item::Operation(Operation {
            enabled: true,
            id: OperationId::new(),
            order: OrderKey::default(),
            name: format!("{} {i}", strategy),
            setup: setup_id,
            strategy: (*strategy).into(),
            tool: Some(tool_id),
            selections: Vec::new(),
            boundary: None,
            avoid: Vec::new(),
            check: Vec::new(),
            restriction_options: Default::default(),
            spindle_speed: None,
            material: None,
            parameters: free_ends_off(strategy)
                .into_iter()
                .chain(parameters.iter().map(|(k, v)| ((*k).to_owned(), v.clone())))
                .collect(),
        }));
    }
    for item in items {
        app.execute(Command::Add {
            item,
            placement: Placement::End,
        })
        .unwrap();
    }
    (dir, app, hash)
}

/// Points on the exact faces of `model`, about `SAMPLE` apart, inside each
/// face's trims, bucketed by a 2 mm grid for the gouge search.
struct Samples {
    cells: BTreeMap<[i64; 3], Vec<Point3>>,
}

const CELL: f64 = 2.0;

fn cell_of(p: Point3) -> [i64; 3] {
    [
        (p.x / CELL).floor() as i64,
        (p.y / CELL).floor() as i64,
        (p.z / CELL).floor() as i64,
    ]
}

impl Samples {
    fn of(model: &BrepModel) -> Self {
        let mut cells: BTreeMap<[i64; 3], Vec<Point3>> = BTreeMap::new();
        for face in &model.faces {
            let exact = ExactFace::new(face).unwrap();
            let (lo, hi) = exact.uv_bounds();
            let b = exact.bounding_box();
            let span = (b.max - b.min).length();
            let count = ((span / SAMPLE).ceil() as usize).clamp(8, 600);
            for i in 0..=count {
                for j in 0..=count {
                    let uv = Uv {
                        u: lo.u + (hi.u - lo.u) * i as f64 / count as f64,
                        v: lo.v + (hi.v - lo.v) * j as f64 / count as f64,
                    };
                    if !exact.contains(uv) {
                        continue;
                    }
                    if let Some(p) = exact.point_normal(uv) {
                        cells.entry(cell_of(p.point)).or_default().push(p.point);
                    }
                }
            }
        }
        Self { cells }
    }

    /// The deepest any sample lies inside the tool at any fed pose of
    /// `toolpath`, and where.
    fn deepest(&self, outline: &Outline, toolpath: &Toolpath) -> (f64, Point3) {
        let mut worst = (0.0, Point3::ORIGIN);
        let reach = outline.radius() + CELL;
        for segment in toolpath
            .groups
            .iter()
            .flat_map(|g| &g.segments)
            .filter(|s| s.kind.is_fed())
        {
            let (tip, axis) = (segment.end.tip, segment.end.axis.get());
            // The cells round the flutes, from the tip up the axis.
            let mut near = BTreeSet::new();
            for k in 0..=6 {
                let c = tip + axis * (f64::from(k) * CELL);
                let (a, b) = (
                    cell_of(Point3::new(c.x - reach, c.y - reach, c.z - reach)),
                    cell_of(Point3::new(c.x + reach, c.y + reach, c.z + reach)),
                );
                for x in a[0]..=b[0] {
                    for y in a[1]..=b[1] {
                        for z in a[2]..=b[2] {
                            near.insert([x, y, z]);
                        }
                    }
                }
            }
            for p in near.iter().filter_map(|c| self.cells.get(c)).flatten() {
                let off = *p - tip;
                let z = off.dot(axis);
                let r = (off - axis * z).length();
                let d = outline.depth(r, z);
                if d > worst.0 {
                    worst = (d, *p);
                }
            }
        }
        worst
    }
}

fn outline() -> Outline {
    Outline::new(&ball().tool.profile().unwrap())
}

/// Gates 1 and 2 on both posts, no gouge into the exact surface, and the
/// same G-code at 1, 2 and N threads.
fn check(job: &Job) {
    let (dir, mut app, _) = build(job, "check");
    let model = placed(job.part, &job.transform);
    let samples = Samples::of(&model);
    let outline = outline();
    let mut first = None;
    for post in ["makera-z1", "linuxcnc"] {
        app.set_post(Some(PostSelection::parse(post)));
        let result = app
            .regenerate()
            .unwrap_or_else(|e| panic!("{} / {post}: {e}", job.part));
        for report in &result.reports {
            assert!(
                report.is_clean(),
                "{} / {post}: {:?} {:?}",
                job.part,
                &report.findings[..report.findings.len().min(5)],
                report.comparison
            );
        }
        let cuts = result.toolpath.groups.iter().flat_map(|g| &g.segments);
        assert!(
            cuts.filter(|s| s.kind == MoveKind::Cutting).count() > 0,
            "{}: nothing cut",
            job.part
        );
        if first.is_none() {
            let (depth, at) = samples.deepest(&outline, &result.toolpath);
            assert!(
                depth <= GOUGE_LIMIT,
                "{}: the tool reaches {depth:.4} mm into the exact surface at {at:?}",
                job.part
            );
            println!("{}: deepest {depth:.4} mm", job.part);
            first = Some(result.gcode.clone());
        }
    }
    app.set_post(Some(PostSelection::parse("makera-z1")));
    app.save().unwrap();
    for threads in openchuck_testkit::determinism::thread_counts() {
        let mut again = Application::open(&dir, Threads::Exactly(threads)).unwrap();
        again.set_post(Some(PostSelection::parse("makera-z1")));
        assert_eq!(
            again.regenerate().unwrap().gcode,
            first.clone().unwrap(),
            "{} at {threads} threads",
            job.part
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
}

fn helical() -> (&'static str, Vec<(&'static str, ParameterValue)>) {
    ("helical_finishing", vec![("max_angle_step", n(10.0))])
}

// The square parts with helical finishing, the revolved spline with
// simultaneous finishing and the filleted box with roughing then rest
// machining were left out until the posting defects of OI-236 were fixed
// (decision 0076): the Z1's F word counting A went above the machine's limit
// at the corners, and LinuxCNC's one-decimal F word on moves that are nearly
// all A was more than 1 % of their time.

#[test]
fn req_imp_02_req_op_04_helical_finishing_on_step_parts_does_not_gouge_the_exact_surface() {
    for (part, transform) in [
        ("cylinder", onto_axis()),
        ("cone", onto_axis()),
        ("sphere", Vec::new()),
        ("revolved_spline", onto_axis()),
        ("box", onto_axis()),
        ("filleted_box", onto_axis()),
    ] {
        check(&Job {
            part,
            transform,
            operations: vec![helical()],
        });
    }
}

#[test]
fn req_imp_02_req_op_04_axial_and_indexed_finishing_on_step_parts_do_not_gouge() {
    for part in ["cone", "box"] {
        check(&Job {
            part,
            transform: onto_axis(),
            operations: vec![("axial_finishing", vec![("max_x_step", n(3.0))])],
        });
    }
    check(&Job {
        part: "filleted_box",
        transform: onto_axis(),
        operations: vec![(
            "indexed_finishing",
            vec![
                ("angles", ParameterValue::Text("0, 90, 180, 270".into())),
                ("max_x_step", n(3.0)),
            ],
        )],
    });
}

#[test]
fn req_imp_02_req_op_05_simultaneous_finishing_on_a_step_part_does_not_gouge() {
    for part in ["cone", "revolved_spline"] {
        check(&Job {
            part,
            transform: onto_axis(),
            operations: vec![("simultaneous_finishing", vec![("max_angle_step", n(10.0))])],
        });
    }
}

#[test]
fn req_imp_02_req_op_03_req_op_09_roughing_then_rest_and_pencil_on_a_step_part_do_not_gouge() {
    for part in ["cylinder", "filleted_box"] {
        check(&Job {
            part,
            transform: onto_axis(),
            operations: vec![
                ("rotary_roughing", vec![("stock_to_leave", n(0.5))]),
                ("rest_machining", vec![("max_angle_step", n(10.0))]),
            ],
        });
    }
    // Pencil clean-up follows the part's inside corners (decision 0226): the
    // filleted box has none, its edges are all outside corners, so it says
    // so. The shouldered shaft has one (below).
    let job = Job {
        part: "filleted_box",
        transform: onto_axis(),
        operations: vec![("pencil_finishing", Vec::new())],
    };
    let (dir, app, _) = build(&job, "pencil");
    let e = app.regenerate().unwrap_err().to_string();
    assert!(e.contains("no inside corner to follow"), "{e}");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_op_09_req_imp_02_pencil_on_a_step_part_follows_its_inside_corner_without_gouging() {
    // OI-234 (b), decision 0226: the shouldered shaft's one inside corner
    // (radius 8, the shoulder 12 mm along) is cut, with gates 1 and 2 clean
    // on both posts, no gouge into the exact faces and the same G-code at 1,
    // 2 and N threads. At a 2 degree crease angle the facets of its fillet
    // are creases in the import's tessellation; on the B-rep they are not.
    let job = Job {
        part: "shouldered_shaft",
        transform: onto_axis(),
        operations: vec![("pencil_finishing", vec![("crease_angle", n(2.0))])],
    };
    check(&job);
    let model = placed(job.part, &job.transform);
    let surface = BrepSurface::new(&model).unwrap();
    let chains = BrepEdges::concave(&model)
        .unwrap()
        .with_crease(2.0)
        .chains(&surface)
        .unwrap();
    assert_eq!(chains.len(), 1, "{chains:?}");
    let (mesh, _) = model.joined_tessellation();
    let in_facets = openchuck_path::pencil::concave_creases(&mesh, 2.0);
    println!(
        "shouldered_shaft: {} creases in the tessellation at 2 degrees, 1 on the B-rep",
        in_facets.len()
    );
    assert!(in_facets.len() > 1, "{}", in_facets.len());
    let x = chains[0].points[0].x;
    for p in &chains[0].points {
        assert!((p.x - x).abs() < 1e-6, "{p:?}");
        assert!((p.y.hypot(p.z) - 8.0).abs() < 0.011, "{p:?}");
    }
    // The 3 mm ball rests on the bar against the shoulder: its tip is on the
    // bar, a tool radius from the corner.
    let (dir, app, _) = build(&job, "pencil-corner");
    let result = app.regenerate().unwrap();
    let cuts: Vec<Point3> = result
        .toolpath
        .groups
        .iter()
        .flat_map(|g| &g.segments)
        .filter(|s| s.kind == MoveKind::Cutting)
        .map(|s| s.end.tip)
        .collect();
    assert!(!cuts.is_empty());
    for tip in &cuts {
        assert!(
            ((tip.x - x).abs() - 1.5).abs() <= 0.05,
            "a cut at X {} with the corner at {x}",
            tip.x
        );
        assert!(
            (tip.y.hypot(tip.z) - 8.0).abs() <= 0.05,
            "a cut at radius {}",
            tip.y.hypot(tip.z)
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_imp_02_tool_drops_on_a_step_part_agree_with_the_exact_surface_within_tolerance() {
    // A ball dropped radially onto the cylinder (radius 10) and the sphere
    // (radius 15) touches with its tip at the radius; the drop is made on the
    // joined tessellation (decision 0017, OI-81), which lies inside the exact
    // surface by at most the import's linear deflection.
    let profile = ball().tool.profile().unwrap();
    let deflection = 0.01;
    for (part, transform, radius, xs) in [
        ("cylinder", onto_axis(), 10.0, (2.0, 48.0)),
        ("sphere", Vec::new(), 15.0, (-0.1, 0.1)),
    ] {
        let model = placed(part, &transform);
        let surface = BrepSurface::new(&model).unwrap();
        let mut worst: f64 = 0.0;
        for i in 0..=20 {
            let x = xs.0 + (xs.1 - xs.0) * f64::from(i) / 20.0;
            for k in 0..72 {
                let a = f64::from(k) * 5.0_f64.to_radians() + 0.013;
                let out = Vec3::new(0.0, a.cos(), a.sin());
                let line = ToolLine {
                    origin: Point3::new(x, 0.0, 0.0) + out * 40.0,
                    axis: UnitVec3::new(out).unwrap(),
                };
                let contact = surface.drop_tool(&profile, &line).unwrap();
                let r = contact.tip.y.hypot(contact.tip.z);
                assert!(
                    r <= radius + 1e-9 && radius - r <= deflection + 1e-6,
                    "{part}: tip at radius {r} at x {x}, angle {k}"
                );
                worst = worst.max(radius - r);
            }
        }
        println!("{part}: drops at most {worst:.5} mm inside the exact surface");
    }
}

#[test]
fn req_op_08_req_nfr_07_a_step_parts_seam_alone_is_no_edge_and_a_rim_is_followed() {
    let job = Job {
        part: "cylinder",
        transform: onto_axis(),
        operations: vec![("edge_following", vec![("depth", n(0.2))])],
    };
    let (dir, mut app, hash) = build(&job, "seam");
    let model = placed(job.part, &job.transform);
    let seam = model.edges.iter().find(|e| e.faces.len() == 1).unwrap().id;
    let rim = model.edges.iter().find(|e| e.faces.len() == 2).unwrap().id;
    let part = app.project().models()[0].id;
    let op = app.project().operations()[0].clone();
    for (edges, ok) in [([seam], false), ([rim], true)] {
        let id = SelectionSetId::new();
        app.execute(Command::Add {
            item: Item::SelectionSet(SelectionSet {
                id,
                order: OrderKey::default(),
                name: "edges".into(),
                model: part,
                model_hash: hash,
                target: SelectionTarget::Edges(edges.into_iter().collect()),
                descriptors: Vec::new(),
                stock_to_leave: None,
                colour: None,
                keep_tangent_edges: None,
            }),
            placement: Placement::End,
        })
        .unwrap();
        let mut with = op.clone();
        with.selections = vec![id];
        app.execute(Command::Replace(Item::Operation(with)))
            .unwrap();
        match (app.regenerate(), ok) {
            (Ok(result), true) => {
                let cuts: Vec<_> = result.toolpath.groups[0]
                    .segments
                    .iter()
                    .filter(|s| s.kind == MoveKind::Cutting)
                    .collect();
                assert!(!cuts.is_empty());
                // One rim: every cut is at one end of the cylinder.
                let x = cuts[0].end.tip.x;
                assert!(cuts.iter().all(|s| (s.end.tip.x - x).abs() < 0.05));
            }
            (Err(e), false) => {
                assert!(e.to_string().contains("none of the chosen edges"), "{e}");
            }
            (other, _) => panic!("{edges:?}: {:?}", other.map(|r| r.notes)),
        }
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_nfr_07_a_damaged_step_part_is_an_error_not_a_crash() {
    let job = Job {
        part: "cylinder",
        transform: onto_axis(),
        operations: vec![helical()],
    };
    let (dir, app, _) = build(&job, "damaged");
    let broken = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/step/broken/truncated_half.step");
    std::fs::copy(broken, dir.join("models/cylinder.step")).unwrap();
    let err = app.regenerate().unwrap_err().to_string();
    assert!(err.contains("STEP"), "{err}");
    std::fs::remove_dir_all(dir).unwrap();
}

/// Runs operation `0` of `app` with `edges`, as the pipeline would.
fn run_with(
    dir: &Path,
    app: &Application,
    edges: openchuck_app::operation::EdgeOptions,
) -> Result<openchuck_app::operation::OperationPath, openchuck_app::AppError> {
    let cancel = CancelToken::new();
    let context = JobContext::new(&cancel, &NoProgress);
    openchuck_app::operation::generate_operation_with(
        dir,
        app.project(),
        app.project().operations()[0].id,
        app.strategies(),
        None,
        edges,
        &context,
    )
}

/// The chains a toolpath cut: runs of cutting moves between other moves.
fn cut_runs(path: &openchuck_app::operation::OperationPath) -> usize {
    let mut runs = 0;
    let mut cutting = false;
    for s in &path.group.segments {
        let now = s.kind == MoveKind::Cutting;
        if now && !cutting {
            runs += 1;
        }
        cutting = now;
    }
    runs
}

#[test]
fn req_op_08_keeping_tangent_edges_follows_the_fillet_boundaries_of_a_step_part() {
    use openchuck_app::operation::{EdgeOptions, edge_options};
    let job = Job {
        part: "filleted_box",
        transform: onto_axis(),
        operations: vec![("edge_following", vec![("depth", n(0.2))])],
    };
    let (dir, app, _) = build(&job, "tangent");
    // Off, the default: every edge of the filleted box is tangent (decision
    // 0053), so there is none to follow.
    let operation = &app.project().operations()[0];
    assert_eq!(
        edge_options(app.project(), operation),
        EdgeOptions::default()
    );
    let off = run_with(&dir, &app, EdgeOptions::default())
        .unwrap_err()
        .to_string();
    assert!(off.contains("no edge to follow"), "{off}");
    let err = app.regenerate().unwrap_err().to_string();
    assert!(err.contains("no edge to follow"), "{err}");
    // On: the 48 fillet boundaries, each its own chain, as the library finds
    // them at a crease angle of zero.
    let on = run_with(&dir, &app, EdgeOptions { keep_tangent: true }).unwrap();
    assert_eq!(cut_runs(&on), 48, "{:?}", on.notes);
    // Nothing of the choice is stored: a saved project reads back the same
    // and holds no new field (the stored setting is lane O's, OI-232).
    app.save().unwrap();
    let before: Vec<(PathBuf, String)> = walk(&dir);
    assert!(before.iter().all(|(_, text)| !text.contains("tangent")));
    let reopened = Application::open(&dir, Threads::Exactly(2)).unwrap();
    assert_eq!(reopened.project(), app.project());
    std::fs::remove_dir_all(dir).unwrap();
}

/// Every TOML file of a project folder, with its text.
fn walk(dir: &Path) -> Vec<(PathBuf, String)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "toml") {
                let text = std::fs::read_to_string(&path).unwrap();
                out.push((path, text));
            }
        }
    }
    out.sort();
    out
}

/// Face free ends off for the strategies that have it: these tests check the
/// strategies against the exact surface, not free ends, and their parts in
/// bar stock are deeper than the test tool's flutes, which a new operation
/// facing free ends would refuse (decision record 0344, OI-1396).
fn free_ends_off(strategy: &str) -> BTreeMap<String, ParameterValue> {
    let mut out = BTreeMap::new();
    if ["helical_finishing", "axial_finishing", "rest_machining"].contains(&strategy) {
        out.insert("face_free_ends".to_owned(), ParameterValue::Flag(false));
    }
    out
}
