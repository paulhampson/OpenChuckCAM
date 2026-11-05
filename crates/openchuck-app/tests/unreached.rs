// SPDX-License-Identifier: AGPL-3.0-or-later
//! The surfaces simultaneous finishing cannot reach, as the application hands
//! them to a front end (OP-20, UI-11, UI-12, NFR-06, GC-07; decision record
//! 0515; issue #416): the part triangles each area lies on, its marker and the
//! view that looks at it, going with the toolpath they came from.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_cmp
)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use openchuck_app::project::id::{OperationId, SetupId, ToolId};
use openchuck_app::project::{
    Item, Operation, OrderKey, ParameterValue, Placement, Setup, ToolSnapshot,
};
use openchuck_app::setup::{Millimetres, Stock, builtin};
use openchuck_app::unreached::{MATCH_TOLERANCE, map_areas, triangles_on};
use openchuck_app::{Application, Command, Threads};
use openchuck_path::{Unreached, UnreachedReason};
use openchuck_testkit::parts::revolve;
use openchuck_types::geometry::Point3;
use openchuck_types::machine::ASense;
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::tool::{ControllerMapping, CuttingData, Tool, ToolShape};
use openchuck_types::units::{FeedRate, SpindleSpeed};

/// The dovetail of the path library's own test: two lips of radius 10 with
/// a groove between them, undercut at both ends. The lips' outsides lie in
/// the same X and A range as the groove's floor below them.
const DOVETAIL: [(f64, f64); 6] = [
    (0.0, 10.0),
    (11.0, 10.0),
    (10.0, 6.0),
    (20.0, 6.0),
    (19.0, 10.0),
    (30.0, 10.0),
];

fn radius(p: Point3) -> f64 {
    p.y.hypot(p.z)
}

/// Whether the triangle lies in the outside of a lip (every corner at the
/// lip's radius).
fn on_lip_outside(mesh: &TriangleMesh, t: u32) -> bool {
    mesh.triangles[t as usize]
        .iter()
        .all(|i| (radius(mesh.vertices[*i as usize]) - 10.0).abs() < 1e-6)
}

/// Whether the triangle lies in the groove's floor.
fn on_floor(mesh: &TriangleMesh, t: u32) -> bool {
    mesh.triangles[t as usize]
        .iter()
        .all(|i| (radius(mesh.vertices[*i as usize]) - 6.0).abs() < 1e-6)
}

fn area(points: Vec<Point3>) -> Unreached {
    let (x0, x1) = points
        .iter()
        .fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.x), b.max(p.x)));
    Unreached {
        x: (x0, x1),
        a: (0.0, 360.0),
        area: 100.0,
        reason: UnreachedReason::Blocked,
        points,
    }
}

/// OP-20, decision 0515 item 2: the triangles marked are those the points
/// lie on. Under a lip's undercut the lip's outside is in the same X and A
/// range as the groove, and it is not marked.
#[test]
fn req_op_20_areas_map_to_the_triangles_they_lie_on() {
    let mesh = revolve(&DOVETAIL, 144);
    // The groove's floor and the first undercut wall, all round (the wall
    // runs from (10, 6) out to the lip's corner at (11, 10)).
    let mut points = Vec::new();
    for degrees in 0..360 {
        let (s, c) = f64::from(degrees).to_radians().sin_cos();
        for k in 0..=16 {
            let x = 11.0 + 0.5 * f64::from(k);
            points.push(Point3::new(x, 6.0 * c, 6.0 * s));
        }
        for x in [10.2, 10.4, 10.6, 10.8] {
            let r = 6.0 + 4.0 * (x - 10.0);
            points.push(Point3::new(x, r * c, r * s));
        }
    }
    let marked = triangles_on(&mesh, &points, MATCH_TOLERANCE);
    assert!(!marked.is_empty());
    for t in &marked {
        assert!(
            !on_lip_outside(&mesh, *t),
            "triangle {t} is a lip's outside"
        );
    }
    // Every triangle of the floor is marked.
    let floor: Vec<u32> = (0..mesh.triangles.len() as u32)
        .filter(|t| on_floor(&mesh, *t))
        .collect();
    assert!(!floor.is_empty());
    for t in &floor {
        assert!(marked.contains(t), "floor triangle {t} is not marked");
    }
    // A point off the surface by more than the tolerance marks nothing; one
    // within it marks the triangle beneath.
    let far = Point3::new(15.0, 6.0 + 2.0 * MATCH_TOLERANCE, 0.0);
    assert!(triangles_on(&mesh, &[far], MATCH_TOLERANCE).is_empty());
    let near = Point3::new(15.0, 6.0 + 0.5 * MATCH_TOLERANCE, 0.0);
    let hit = triangles_on(&mesh, &[near], MATCH_TOLERANCE);
    assert_eq!(hit.len(), 1);
    assert!(on_floor(&mesh, *hit.iter().next().unwrap()));
    // Deterministic: the same input gives the same set (GC-07).
    assert_eq!(triangles_on(&mesh, &points, MATCH_TOLERANCE), marked);
    // Nothing to mark: nothing, not a failure (NFR-07).
    assert!(triangles_on(&mesh, &[], MATCH_TOLERANCE).is_empty());
    assert!(triangles_on(&TriangleMesh::default(), &points, MATCH_TOLERANCE).is_empty());
}

/// UI-12, decision 0515 item 5: the view turns to look straight at the area
/// from the side it faces, whichever way positive A turns. A surface at
/// polar angle `phi` (from +Y towards +Z) faces the tool at A = sense (90 -
/// phi), as the strategy reports it (decision 0368).
#[test]
fn req_ui_12_the_view_looks_at_the_side_the_area_faces_for_both_a_senses() {
    let mesh = revolve(&DOVETAIL, 144);
    for sense in [ASense::Positive, ASense::Negative] {
        let mut machine = openchuck_types::machine::MachineProfile::example_x_rotary();
        machine.a_sense = sense;
        let frame = openchuck_types::kinematics::RotaryFrame::of_profile(&machine);
        for phi in [0.0_f64, 30.0, 90.0, 135.0, 250.0, 330.0] {
            let (s, c) = phi.to_radians().sin_cos();
            let points: Vec<Point3> = (0..8)
                .map(|k| Point3::new(12.0 + f64::from(k), 6.0 * c, 6.0 * s))
                .collect();
            let a = sense.sign() * (90.0 - phi);
            let mut unreached = area(points);
            unreached.a = (a - 5.0, a + 5.0);
            let marks = map_areas(&mesh, std::slice::from_ref(&unreached), &frame);
            let target = &marks[0].frame;
            let expected = [0.0, c, s];
            let got = target.direction;
            let dot: f64 = expected.iter().zip(got).map(|(e, g)| e * g).sum();
            assert!(
                dot > 0.999,
                "{sense:?} phi {phi}: {got:?} against {expected:?}"
            );
            let wrapped = (target.a - a).rem_euclid(360.0);
            assert!(
                wrapped < 1e-9 || 360.0 - wrapped < 1e-9,
                "{} against {a}",
                target.a
            );
            assert_eq!(target.x, unreached.x);
        }
    }
}

/// An area all the way round has no side it faces: the view takes the side
/// of its marker, and the middle of A, which is nothing, is not used.
#[test]
fn req_ui_12_an_area_all_round_is_viewed_from_its_marker() {
    let mesh = revolve(&DOVETAIL, 144);
    let frame = openchuck_types::kinematics::RotaryFrame::of_profile(
        &openchuck_types::machine::MachineProfile::example_x_rotary(),
    );
    // Points on the floor in a half ring from +Z to -Z through +Y.
    let points: Vec<Point3> = (0..=18)
        .map(|k| {
            let t = (-90.0 + 10.0 * f64::from(k)).to_radians();
            Point3::new(15.0, 6.0 * t.cos(), 6.0 * t.sin())
        })
        .collect();
    let all_round = area(points);
    assert!(all_round.a.1 - all_round.a.0 >= 360.0);
    let marks = map_areas(&mesh, &[all_round], &frame);
    let marker = marks[0].marker;
    let d = marks[0].frame.direction;
    let r = radius(marker);
    assert!(
        (d[1] * marker.y + d[2] * marker.z - r).abs() < 1e-9,
        "{d:?}"
    );
    assert_eq!(d[0], 0.0);
}

/// OP-20, NFR-06: the marker is the area's point nearest its middle; the
/// numbers follow the order the areas come in (the warning's).
#[test]
fn req_op_20_the_markers_follow_the_area_order_and_sit_on_their_points() {
    let mesh = revolve(&DOVETAIL, 144);
    let frame = openchuck_types::kinematics::RotaryFrame::of_profile(
        &openchuck_types::machine::MachineProfile::example_x_rotary(),
    );
    let arc = |x: f64, lo: f64, hi: f64| {
        let points: Vec<Point3> = (0..=10)
            .map(|k| {
                let t = (lo + (hi - lo) * f64::from(k) / 10.0).to_radians();
                Point3::new(x, 6.0 * t.cos(), 6.0 * t.sin())
            })
            .collect();
        let mut u = area(points);
        u.a = (lo, hi);
        u
    };
    let areas = [arc(12.0, 0.0, 100.0), arc(16.0, 200.0, 300.0)];
    let marks = map_areas(&mesh, &areas, &frame);
    assert_eq!(marks.iter().map(|m| m.number).collect::<Vec<_>>(), [1, 2]);
    for (m, u) in marks.iter().zip(&areas) {
        assert!(u.points.contains(&m.marker), "{:?}", m.marker);
        assert_eq!(m.unmatched, 0);
        assert!(!m.triangles.is_empty());
    }
    // The first arc is 0 to 100 degrees: its middle is at 50.
    let marker = marks[0].marker;
    let polar = marker.z.atan2(marker.y).to_degrees();
    assert!((polar - 50.0).abs() < 6.0, "{polar}");
}

// The end to end tests: a dovetail part, simultaneous finishing, and the
// application's own generation.

fn write_stl(mesh: &TriangleMesh, path: &Path) {
    let mut text = String::from("solid part\n");
    for t in &mesh.triangles {
        let v: Vec<_> = t.iter().map(|i| mesh.vertices[*i as usize]).collect();
        let n = {
            let c = (v[1] - v[0]).cross(v[2] - v[0]);
            let len = c.length();
            if len > 0.0 { c * (1.0 / len) } else { c }
        };
        text.push_str(&format!(
            "facet normal {:e} {:e} {:e}\n outer loop\n",
            n.x, n.y, n.z
        ));
        for p in v {
            text.push_str(&format!("  vertex {:e} {:e} {:e}\n", p.x, p.y, p.z));
        }
        text.push_str(" endloop\nendfacet\n");
    }
    text.push_str("endsolid part\n");
    std::fs::write(path, text).unwrap();
}

struct Built {
    dir: PathBuf,
    app: Application,
    setup: SetupId,
    operation: OperationId,
    tool: ToolId,
}

fn build(name: &str, part_offset: f64) -> Built {
    let dir = std::env::temp_dir().join(format!(
        "openchuck-unreached-{name}-{}-{}",
        std::process::id(),
        std::thread::current()
            .name()
            .unwrap_or("main")
            .replace("::", "-")
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let mut app = Application::create(&dir, name, Threads::Exactly(2)).unwrap();
    std::fs::create_dir_all(dir.join("models")).unwrap();
    write_stl(&revolve(&DOVETAIL, 72), &dir.join("models/part.stl"));
    let (model, _) = app.add_model(&dir.join("models/part.stl"), None).unwrap();
    let machine = builtin("makera-z1").unwrap().unwrap();
    app.execute(Command::SetMachine(Some(Box::new(machine))))
        .unwrap();
    let setup_id = SetupId::new();
    let mut setup = Setup::new(setup_id, "Setup 1");
    setup.part = Some(model);
    setup.material = Some("oak".into());
    setup.frame.part_offset = Millimetres(part_offset);
    setup.stock = Some(Stock::Cylinder {
        diameter: Millimetres(22.0),
        length: Millimetres(32.0),
        x_start: Millimetres(-1.0 + part_offset),
    });
    let tool_id = ToolId::new();
    let mut cutting_data = BTreeMap::new();
    cutting_data.insert(
        "oak".to_owned(),
        CuttingData {
            spindle_speed: SpindleSpeed(12000.0),
            feed_per_tooth: Millimetres(0.02),
            plunge_feed: FeedRate(150.0),
            stepover: Millimetres(0.5),
            stepdown: Millimetres(1.0),
        },
    );
    let snapshot = ToolSnapshot {
        id: tool_id,
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
    };
    let parameters: BTreeMap<String, ParameterValue> = [
        ("adaptive_spacing".to_owned(), ParameterValue::Flag(false)),
        ("stepover".to_owned(), ParameterValue::Number(0.5)),
    ]
    .into();
    let operation = OperationId::new();
    for item in [
        Item::Setup(setup),
        Item::Tool(snapshot),
        Item::Operation(Operation {
            enabled: true,
            id: operation,
            order: OrderKey::default(),
            name: "Finish".into(),
            setup: setup_id,
            strategy: "simultaneous_finishing".into(),
            tool: Some(tool_id),
            selections: Vec::new(),
            boundary: None,
            avoid: Vec::new(),
            check: Vec::new(),
            restriction_options: Default::default(),
            spindle_speed: None,
            material: None,
            parameters,
        }),
    ] {
        app.execute(Command::Add {
            item,
            placement: Placement::End,
        })
        .unwrap();
    }
    Built {
        dir,
        app,
        setup: setup_id,
        operation,
        tool: tool_id,
    }
}

/// OP-20, decision 0515 items 1 and 2: the areas of a real run are mapped
/// onto the triangles of the part the view draws, in the same frame (the part
/// offset moves both), and the lips' outsides are not among them.
#[test]
fn req_op_20_the_areas_of_a_run_lie_on_the_part_the_view_draws() {
    let b = build("frame", 3.0);
    let run = Arc::new(b.app.regenerate().unwrap());
    let areas = b.app.operation_unreached(&run, b.operation);
    assert!(!areas.is_empty(), "the dovetail has areas to report");
    let found = b
        .app
        .start_unreached_areas(Arc::clone(&run), b.setup)
        .wait()
        .unwrap();
    assert_eq!(found.setup, b.setup);
    let ours = found
        .operations
        .iter()
        .find(|o| o.operation == b.operation)
        .unwrap();
    assert_eq!(ours.areas.len(), areas.len());
    // The mesh the viewport draws for the part of this setup.
    let view = b.app.start_scene(Some(b.setup)).wait().unwrap();
    let part = view
        .meshes
        .iter()
        .find(|m| m.model == Some(found.model))
        .expect("the scene draws the part");
    // The offset reached the part the scene draws.
    let min_x = part
        .mesh
        .vertices
        .iter()
        .map(|p| p.x)
        .fold(f64::MAX, f64::min);
    assert!((min_x - 3.0).abs() < 1e-9, "{min_x}");
    for (mapped, original) in ours.areas.iter().zip(areas) {
        assert_eq!(mapped.unmatched, 0, "every point lands on the part");
        assert!(!mapped.triangles.is_empty());
        for t in &mapped.triangles {
            // Triangle indices are the scene mesh's own, and none is the
            // outside of a lip, which lies in the same X and A range.
            let corners = part.mesh.triangles[*t as usize];
            assert!(
                !corners
                    .iter()
                    .all(|i| (radius(part.mesh.vertices[*i as usize]) - 10.0).abs() < 1e-6),
                "area {} marks a lip's outside",
                mapped.number
            );
        }
        // The marker is one of the area's points, on a marked triangle.
        assert!(original.points.contains(&mapped.marker));
    }
    let _ = std::fs::remove_dir_all(&b.dir);
}

/// OP-20, decision 0515 items 1 and 3: the markers are numbered in the
/// warning's order, and the areas go with the toolpath they came from: an
/// edit makes them stale and a failed generation brings none.
#[test]
fn req_op_20_the_markers_follow_the_warning_order() {
    let b = build("order", 0.0);
    let run = Arc::new(b.app.regenerate().unwrap());
    let areas = b.app.operation_unreached(&run, b.operation);
    assert!(areas.len() >= 2, "{} areas", areas.len());
    let warning = run.groups[0]
        .warnings
        .iter()
        .find(|w| {
            w.message
                .starts_with(openchuck_app::unreached::WARNING_START)
        })
        .expect("the run warns of the areas");
    // The warning lists the largest five first, as "X a to b near A c".
    let listed: Vec<&str> = warning.message.split("; ").collect();
    let first = format!("X {:.3} to {:.3}", areas[0].x.0, areas[0].x.1);
    assert!(listed[0].contains(&first), "{} / {first}", listed[0]);
    for pair in areas.windows(2) {
        assert!(pair[0].area >= pair[1].area, "largest first");
    }
    let found = b
        .app
        .start_unreached_areas(Arc::clone(&run), b.setup)
        .wait()
        .unwrap();
    let ours = &found.operations[0];
    assert_eq!(
        ours.areas.iter().map(|a| a.number).collect::<Vec<_>>(),
        (1..=areas.len()).collect::<Vec<_>>()
    );
    for (mapped, original) in ours.areas.iter().zip(areas) {
        assert_eq!(mapped.frame.x, original.x);
    }
    // Again, the same: the mapping is deterministic whatever the threads.
    let again = b
        .app
        .start_unreached_areas(Arc::clone(&run), b.setup)
        .wait()
        .unwrap();
    assert_eq!(found, again);
    let _ = std::fs::remove_dir_all(&b.dir);
}

#[test]
fn req_op_20_the_areas_go_with_their_toolpath() {
    let mut b = build("stale", 0.0);
    let run = Arc::new(b.app.regenerate().unwrap());
    assert!(!b.app.operation_unreached(&run, b.operation).is_empty());
    // An unknown operation has none.
    assert!(
        b.app
            .operation_unreached(&run, OperationId::new())
            .is_empty()
    );
    // Edit the operation: the run is out of date and its areas are not
    // shown (they would mark a part the edit may have changed the reach of).
    let mut operation = b.app.project().operation(b.operation).unwrap().clone();
    operation
        .parameters
        .insert("stepover".to_owned(), ParameterValue::Number(0.6));
    b.app
        .execute(Command::Replace(Item::Operation(operation.clone())))
        .unwrap();
    assert!(b.app.operation_unreached(&run, b.operation).is_empty());
    // Generated again: current, with areas.
    let fresh = Arc::new(b.app.regenerate().unwrap());
    assert!(!b.app.operation_unreached(&fresh, b.operation).is_empty());
    // The old run's areas stay out: the operation's path is another's.
    assert!(b.app.operation_unreached(&run, b.operation).is_empty());
    // A generation that fails (no tool) brings no run; what is shown from the
    // last run is nothing.
    operation.tool = None;
    b.app
        .execute(Command::Replace(Item::Operation(operation)))
        .unwrap();
    assert!(b.app.regenerate().is_err());
    assert!(b.app.operation_unreached(&fresh, b.operation).is_empty());
    let _ = b.tool;
    let _ = std::fs::remove_dir_all(&b.dir);
}

/// UI-12, decision 0515 item 5 with decision 0505: rest machining's
/// warning lists its areas in the same words and order as simultaneous
/// finishing's, so Messages offers "Show in view N" on either; no other
/// warning is taken for one.
#[test]
fn req_ui_12_both_warnings_that_list_areas_offer_show_in_view() {
    use openchuck_app::unreached::{REST_WARNING_START, WARNING_START, lists_areas};
    assert!(lists_areas(&format!(
        "{WARNING_START}2 hidden areas of the part"
    )));
    assert!(lists_areas(&format!(
        "{REST_WARNING_START}2 areas of the part where material is left"
    )));
    assert!(!lists_areas(
        "rest machining found material left but the tool cannot reach any of it"
    ));
    assert!(!lists_areas(
        "rest machining added 3 rings between X 1 and 2"
    ));
}

/// UI-12, decision 0515 item 5: the warning that lists unreachable areas
/// offers "Show in view N" for each area it lists (at most five, as it
/// lists), in its order; no other message does.
#[test]
fn req_ui_12_the_unreached_warning_offers_show_in_view_for_each_listed_area() {
    use openchuck_app::overview::{ItemRef, MessageAction, Severity};
    let b = build("messages", 0.0);
    // Nothing generated: nothing to offer.
    let before = b.app.overview(None, &openchuck_app::JobLog::default());
    assert!(before.messages.iter().all(|m| m.views.is_empty()));
    let run = b.app.regenerate().unwrap();
    let count = b.app.operation_unreached(&run, b.operation).len();
    assert!(count > 0);
    let overview = b.app.overview(None, &openchuck_app::JobLog::default());
    let warning = overview
        .messages
        .iter()
        .find(|m| {
            m.severity == Severity::Warning
                && m.text.starts_with("Simultaneous finishing cannot reach")
        })
        .unwrap_or_else(|| panic!("{:?}", overview.messages));
    assert_eq!(warning.source, Some(ItemRef::Operation(b.operation)));
    let wanted: Vec<MessageAction> = (1..=count.min(5))
        .map(|n| MessageAction::ShowInView {
            operation: b.operation,
            area: n as u8,
        })
        .collect();
    assert_eq!(warning.views, wanted);
    let labels: Vec<&str> = warning.views.iter().map(|v| v.label()).collect();
    let words: Vec<String> = (1..=count.min(5))
        .map(|n| format!("Show in view {n}"))
        .collect();
    assert_eq!(labels, words);
    // Every other message is without these links.
    assert!(
        overview
            .messages
            .iter()
            .filter(|m| !std::ptr::eq(*m, warning))
            .all(|m| m.views.is_empty())
    );
    // Out of date: the warning is no longer claimed for the operation.
    let mut b = b;
    let mut operation = b.app.project().operation(b.operation).unwrap().clone();
    operation
        .parameters
        .insert("stepover".to_owned(), ParameterValue::Number(0.6));
    b.app
        .execute(Command::Replace(Item::Operation(operation)))
        .unwrap();
    let stale = b.app.overview(None, &openchuck_app::JobLog::default());
    assert!(stale.messages.iter().all(|m| m.views.is_empty()));
    let _ = std::fs::remove_dir_all(&b.dir);
}
