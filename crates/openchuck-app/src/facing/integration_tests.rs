// SPDX-License-Identifier: AGPL-3.0-or-later
//! Whole project -> generation -> both posts -> replay/diagrams/defaults.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::float_cmp)]
use crate::test_project::TestProject;
use crate::{Application, Command, OutputGrouping, PostSelection, Threads};
use openchuck_project::{Item, ParameterValue};
use openchuck_types::geometry::{Aabb, Point3};
use openchuck_types::stock::Stock;
use openchuck_types::tool::ToolShape;
use openchuck_types::units::Millimetres;

#[test]
fn req_op_24_req_ui_03_invalid_box_stock_draft_explains_why_it_cannot_generate() {
    let (mut p, _) = project("facing-invalid-draft", "box");
    let mut setup = p.app.project().setups()[0].clone();
    setup.stock = Some(Stock::Cylinder {
        diameter: Millimetres(24.),
        length: Millimetres(20.),
        x_start: Millimetres(-5.),
    });
    p.app.execute(Command::Replace(Item::Setup(setup))).unwrap();
    let view = p.app.start_scene(None).wait().unwrap();
    assert!(
        view.notes.iter().any(|n| n.contains("box/block")),
        "An unsupported stock must not look like a valid Box draft"
    );
}

#[test]
fn req_op_24_req_gc_08_controller_programs_cover_all_modes_with_bounded_run_times() {
    for shape in ["box", "end", "flat", "round"] {
        let (mut p, op) = project(&format!("facing-controller-{shape}"), shape);
        for name in ["feed", "plunge_feed"] {
            p.app
                .set_parameter(op, name, Some(ParameterValue::Number(1500.)))
                .unwrap();
        }
        if shape == "round" {
            assert!(
                p.app
                    .start_regenerate_with_post(
                        PostSelection::Builtin("linuxcnc".into()),
                        OutputGrouping::Operation
                    )
                    .wait()
                    .is_err(),
                "Rounded saturated rotary feed must not bypass the actual axis limit"
            );
            p.app
                .set_parameter(op, "feed", Some(ParameterValue::Number(600.)))
                .unwrap();
        }
        let run = p
            .app
            .start_regenerate_with_post(
                PostSelection::Builtin("linuxcnc".into()),
                OutputGrouping::Operation,
            )
            .wait()
            .unwrap();
        let seconds = run
            .toolpath
            .groups
            .iter()
            .flat_map(|g| &g.segments)
            .map(|s| s.duration.0)
            .sum::<f64>();
        assert!(
            seconds < 540.,
            "Keep individual controller verification below the ten-minute development limit"
        );
        assert_eq!(run.gcode.len(), 1);
        // Opt-in retention lets the independent LinuxCNC gate consume these
        // exact, already-verified bytes without adding generated goldens.
        if let Some(folder) = std::env::var_os("OPENCHUCK_FACING_PROGRAMS") {
            let folder = std::path::PathBuf::from(folder);
            std::fs::create_dir_all(&folder).unwrap();
            std::fs::write(
                folder.join(format!("facing_{shape}_linuxcnc.ngc")),
                &run.gcode[0].1,
            )
            .unwrap();
            eprintln!(
                "{shape}: {seconds:.3} programmed seconds, {} bytes",
                run.gcode[0].1.len()
            );
        }
    }
}

#[test]
fn req_op_24_req_gc_07_all_selected_side_combinations_both_senses_and_layer_choices_post() {
    for sense in [
        openchuck_types::machine::ASense::Positive,
        openchuck_types::machine::ASense::Negative,
    ] {
        let (mut p, op) = project(&format!("facing-post-combinations-{sense:?}"), "box");
        let mut machine = p.app.project().machine().unwrap().clone();
        machine.a_sense = sense;
        p.app
            .execute(Command::SetMachine(Some(Box::new(machine))))
            .unwrap();
        for layers in ["full", "equal"] {
            p.app
                .set_parameter(
                    op,
                    "layer_distribution",
                    Some(ParameterValue::Text(layers.into())),
                )
                .unwrap();
            for mask in 1..16 {
                for (i, name) in ["positive_z", "positive_y", "negative_z", "negative_y"]
                    .into_iter()
                    .enumerate()
                {
                    p.app
                        .set_parameter(op, name, Some(ParameterValue::Flag(mask & (1 << i) != 0)))
                        .unwrap();
                }
                for post in ["linuxcnc", "makera-z1"] {
                    let run = p
                        .app
                        .start_regenerate_with_post(
                            PostSelection::Builtin(post.into()),
                            OutputGrouping::Operation,
                        )
                        .wait()
                        .unwrap_or_else(|e| panic!("{sense:?}, {layers}, mask{mask}, {post}: {e}"));
                    assert_eq!(run.gcode.len(), 1);
                }
            }
        }
    }
}

#[test]
fn req_op_24_req_gc_07_req_nfr_06_facing_paths_and_both_posts_are_identical_at_one_two_available_threads()
 {
    let available = std::thread::available_parallelism().unwrap().get();
    for (shape, layer, angle) in [
        ("box", "full", 45.),
        ("box", "equal", 33.3333),
        ("end", "equal", 0.),
        ("flat", "equal", 0.),
        ("round", "equal", 0.),
    ] {
        let (mut p, op) = project(&format!("facing-thread-{shape}-{layer}"), shape);
        if shape == "box" {
            for (name, value) in [
                ("layer_distribution", ParameterValue::Text(layer.into())),
                ("stepdown", ParameterValue::Number(2.)),
                (
                    "arbitrary_faces",
                    ParameterValue::Text(format!(r#"[{{"angle":{angle}}}]"#)),
                ),
            ] {
                p.app.set_parameter(op, name, Some(value)).unwrap();
            }
        }
        p.app.save().unwrap();
        for post in ["linuxcnc", "makera-z1"] {
            let mut expected = None;
            for threads in [1, 2, available] {
                let mut app = Application::open(&p.dir, Threads::Exactly(threads)).unwrap();
                let before = std::time::Instant::now();
                let run = app
                    .start_regenerate_with_post(
                        PostSelection::Builtin(post.into()),
                        OutputGrouping::Operation,
                    )
                    .wait()
                    .unwrap();
                eprintln!(
                    "{shape}/{layer}/{post}/threads{threads}: {:?}, {} moves, {} bytes",
                    before.elapsed(),
                    run.toolpath.groups[0].segments.len(),
                    run.gcode[0].1.len()
                );
                let actual = (run.canonical_toolpath, run.gcode);
                if let Some(expected) = &expected {
                    assert_eq!(&actual, expected);
                } else {
                    expected = Some(actual);
                }
            }
        }
    }
}

#[test]
fn req_op_24_req_sim_05_intentionally_unfaced_stock_remains_in_global_deviation_and_stale_sweep_is_hidden()
 {
    let (mut p, op) = project("facing-global-deviation", "box");
    let mut settings = p.app.project().simulation();
    settings.cell = Millimetres(0.5);
    p.app.execute(Command::SetSimulation(settings)).unwrap();
    let run = std::sync::Arc::new(p.app.start_simulate().wait().unwrap());
    assert!(
        run.notes
            .iter()
            .any(|n| n.contains("Intentionally unfaced:"))
    );
    let report = run.simulation.as_ref().unwrap();
    let global = report.setups[0].deviation.as_ref().unwrap();
    assert!(
        global.worst_remaining.as_ref().unwrap().value > 2.5,
        "The unfaced lower side must remain a global miss"
    );
    let before = report.clone();
    let shown = p
        .app
        .start_scene_with_facing(None, Some(run.clone()))
        .wait()
        .unwrap();
    assert!(
        shown
            .lines
            .iter()
            .any(|l| l.role == crate::scene::SceneRole::FacingSweep)
    );
    assert_eq!(run.simulation.as_ref().unwrap(), &before);
    p.app
        .set_parameter(op, "extent_margin", Some(ParameterValue::Number(0.5)))
        .unwrap();
    let edited = p
        .app
        .start_scene_with_facing(None, Some(run))
        .wait()
        .unwrap();
    assert!(
        !edited
            .lines
            .iter()
            .any(|l| l.role == crate::scene::SceneRole::FacingSweep),
        "An old generated sweep cannot decorate an edited draft"
    );
}

fn project(name: &str, shape: &str) -> (TestProject, openchuck_project::id::OperationId) {
    let mut p = TestProject::new(name);
    let mut machine = p.app.project().machine().unwrap().clone();
    machine.rotary_origin.z = -55.;
    p.app
        .execute(Command::SetMachine(Some(Box::new(machine))))
        .unwrap();
    let mut tool = p.app.project().tools()[0].clone();
    tool.tool.shape = ToolShape::FlatEnd;
    tool.tool.diameter = Millimetres(3.);
    tool.tool.shank_diameter = Millimetres(3.);
    tool.tool.centre_cutting = Some(true);
    p.app.execute(Command::Replace(Item::Tool(tool))).unwrap();
    let mut setup = p.app.project().setups()[0].clone();
    setup.stock = Some(if shape == "round" {
        Stock::Cylinder {
            diameter: Millimetres(24.),
            length: Millimetres(20.),
            x_start: Millimetres(-5.),
        }
    } else {
        Stock::Box {
            bounds: Aabb {
                min: Point3::new(-5., -8., -8.),
                max: Point3::new(15., 8., 8.),
            },
        }
    });
    if shape == "end" {
        setup.workholding.start = Some(openchuck_types::workholding::HeldRegion {
            length: Millimetres(3.),
            diameter: None,
        });
    }
    p.app.execute(Command::Replace(Item::Setup(setup))).unwrap();
    let op = p.add_operation("Facing", "facing");
    for (name, value) in [
        ("shape", ParameterValue::Text(shape.into())),
        ("positive_z", ParameterValue::Flag(true)),
        ("extent", ParameterValue::Text("part".into())),
        ("stock_to_leave", ParameterValue::Number(0.2)),
        ("stepdown", ParameterValue::Number(1.3)),
        ("stepover", ParameterValue::Number(1.5)),
    ] {
        p.app.set_parameter(op, name, Some(value)).unwrap();
    }
    if matches!(shape, "round" | "flat") {
        p.app
            .set_parameter(
                op,
                "size",
                Some(ParameterValue::Number(if shape == "round" {
                    16.
                } else {
                    6.
                })),
            )
            .unwrap();
        p.app
            .set_parameter(op, "min_cut", Some(ParameterValue::Number(0.)))
            .unwrap();
    }
    (p, op)
}

#[test]
fn req_op_24_req_gc_07_req_ui_03_req_sim_05_all_modes_complete_through_real_application_both_posts()
{
    for shape in ["box", "end", "flat", "round"] {
        let (mut p, op) = project(&format!("facing-e2e-{shape}"), shape);
        let draft = p.app.start_scene(None).wait().unwrap();
        assert!(draft.notes.is_empty(), "{:?}", draft.notes);
        assert!(
            draft
                .lines
                .iter()
                .any(|l| l.role == crate::scene::SceneRole::FacingExtent)
        );
        let mut canonical = None;
        for post in ["linuxcnc", "makera-z1"] {
            let start = std::time::Instant::now();
            let run = p
                .app
                .start_regenerate_with_post(
                    PostSelection::Builtin(post.into()),
                    OutputGrouping::Operation,
                )
                .wait()
                .unwrap();
            eprintln!(
                "{shape}/{post}: {:?}, {} moves, {} output bytes",
                start.elapsed(),
                run.toolpath.groups[0].segments.len(),
                run.gcode.iter().map(|(_, s)| s.len()).sum::<usize>()
            );
            assert_eq!(run.gcode.len(), 1);
            if let Some(expected) = &canonical {
                assert_eq!(&run.canonical_toolpath, expected);
            } else {
                canonical = Some(run.canonical_toolpath.clone());
            }
            assert!(
                run.notes
                    .iter()
                    .any(|n| n.contains("Intentionally unfaced:"))
            );
            assert!(run.groups[0].facing.is_some());
            let scene = p
                .app
                .start_scene_with_facing(None, Some(std::sync::Arc::new(run)))
                .wait()
                .unwrap();
            assert!(
                scene
                    .lines
                    .iter()
                    .any(|l| l.role == crate::scene::SceneRole::FacingSweep)
            );
            if shape == "round" {
                assert!(
                    scene
                        .lines
                        .iter()
                        .filter(|l| l.role == crate::scene::SceneRole::FacingSweep)
                        .flat_map(|l| &l.points)
                        .any(|p| p.y > 7.5 && p.z.abs() < 0.1),
                    "Actual sweep must include the rotating cutter at a quarter turn, not only fixed-A entries"
                );
            }
        }
        p.app.save().unwrap();
        let saved = p.app.project().operation(op).unwrap().parameters.clone();
        let restored = Application::open(&p.dir, Threads::Exactly(1)).unwrap();
        assert_eq!(restored.project().operation(op).unwrap().parameters, saved);
        assert_eq!(
            restored.regenerate().unwrap().canonical_toolpath,
            canonical.unwrap()
        );
        p.app
            .set_parameter(op, "extent_margin", Some(ParameterValue::Number(0.5)))
            .unwrap();
        assert!(
            !p.app
                .start_scene(None)
                .wait()
                .unwrap()
                .labels
                .iter()
                .all(|l| !l.text.contains("0.500"))
        );
    }
}
