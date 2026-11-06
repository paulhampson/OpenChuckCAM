// SPDX-License-Identifier: AGPL-3.0-or-later
//! Every edit of a setup, its held regions and tabs, the machine, the tool
//! or the simulation settings makes the operations that read it stale, and
//! a tab or held-region edit gives a different path when generated again
//! (UI-11, NFR-12, OP-21, SET-09; issue #558; decision records 0030, 0172,
//! 0192).
//!
//! The owner's report (#558): shortening the tab at the free (unheld) end of
//! the CrankShaft project left the operations up to date. Its public
//! stand-in is the cone of `tab_scope.rs` in round stock, gripped at its
//! start, with a tab between the grip and the part and a second tab on the
//! free end. After one generation each edit must show three ways:
//!
//! - the operation's state in the project tree is Stale (UI-11, 0172);
//! - the generated result is out of date for Simulate (0192);
//! - writing G-code from it is blocked as out of date (GC-15, NFR-12).
//!
//! Undoing the edit makes the operation up to date again, so the tests also
//! hold that nothing else in the key moved.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use openchuck_app::CheckLevel;
use openchuck_app::overview::{ItemRef, ItemState};
use openchuck_app::project::id::{OperationId, SetupId, ToolId};
use openchuck_app::project::{
    Item, Operation, OrderKey, ParameterValue, Placement, Setup, ToolSnapshot,
};
use openchuck_app::setup::{
    HeldRegion, Millimetres, Stock, Tab, TabEnd, TabLayout, TabShape, Workholding, builtin,
};
use openchuck_app::simulation::SimulationSettings;
use openchuck_app::{Application, Command, JobLog, Regeneration, Threads};
use openchuck_testkit::parts::{Cone, RadialPart};
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::tool::{ControllerMapping, CuttingData, Tool, ToolShape};
use openchuck_types::units::{Degrees, FeedRate, SpindleSpeed};

/// The tab on the free end of the part: the higher X end, where nothing
/// holds the stock (the owner's case).
const FREE_END_TAB: usize = 1;

fn cone() -> TriangleMesh {
    Cone {
        start_radius: 10.0,
        end_radius: 5.0,
        length: 30.0,
    }
    .mesh(31, 48)
}

fn write_stl(mesh: &TriangleMesh, path: &Path) {
    let mut text = String::from("solid part\n");
    for t in &mesh.triangles {
        text.push_str("facet normal 0 0 0\n outer loop\n");
        for i in t {
            let p = mesh.vertices[*i as usize];
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
    operation: OperationId,
    setup: SetupId,
    tool: ToolId,
}

impl Drop for Built {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// The cone (X 0 to 30) in round stock 24 mm across from X -12 to 42,
/// gripped 4 mm at its start and free at its end, with a tab on the axis
/// from X -4 to 0 between the grip and the part and one from X 30 to 36 on
/// the free end, finished axially with a 6 mm ball.
fn build(name: &str) -> Built {
    let dir = std::env::temp_dir().join(format!("openchuck-stale-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut app = Application::create(&dir, name, Threads::Exactly(2)).unwrap();
    std::fs::create_dir_all(dir.join("models")).unwrap();
    write_stl(&cone(), &dir.join("models/part.stl"));
    let (model, _) = app.add_model(&dir.join("models/part.stl"), None).unwrap();
    let machine = builtin("makera-z1").unwrap().unwrap();
    app.execute(Command::SetMachine(Some(Box::new(machine))))
        .unwrap();
    app.execute(Command::SetSimulation(SimulationSettings {
        cell: Millimetres(0.5),
    }))
    .unwrap();
    let setup_id = SetupId::new();
    let mut setup = Setup::new(setup_id, "Setup 1");
    setup.part = Some(model);
    setup.material = Some("oak".into());
    setup.stock = Some(Stock::Cylinder {
        diameter: Millimetres(24.0),
        length: Millimetres(54.0),
        x_start: Millimetres(-12.0),
    });
    let on_axis = |x_start: f64, length: f64| Tab {
        x_start: Millimetres(x_start),
        length: Millimetres(length),
        shape: TabShape::Cylinder {
            diameter: Millimetres(6.0),
        },
        layout: TabLayout::OnAxis,
        auto: None,
    };
    setup.workholding = Workholding {
        start: Some(HeldRegion {
            length: Millimetres(4.0),
            diameter: None,
        }),
        end: None,
        tabs: vec![on_axis(-4.0, 4.0), on_axis(30.0, 6.0)],
    };
    let tool_id = ToolId::new();
    let cutting_data = BTreeMap::from([(
        "oak".to_owned(),
        CuttingData {
            spindle_speed: SpindleSpeed(12000.0),
            feed_per_tooth: Millimetres(0.02),
            plunge_feed: FeedRate(150.0),
            stepover: Millimetres(2.0),
            stepdown: Millimetres(2.0),
        },
    )]);
    let snapshot = ToolSnapshot {
        id: tool_id,
        order: OrderKey::default(),
        name: "6 mm ball".into(),
        tool: Tool {
            shape: ToolShape::BallEnd,
            diameter: Millimetres(6.0),
            flute_length: Millimetres(30.0),
            overall_length: Millimetres(45.0),
            shank_diameter: Millimetres(6.0),
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
    let operation = OperationId::new();
    let items = vec![
        Item::Setup(setup),
        Item::Tool(snapshot),
        Item::Operation(Operation {
            enabled: true,
            id: operation,
            order: OrderKey::default(),
            name: "Axial Finishing".into(),
            setup: setup_id,
            strategy: "axial_finishing".into(),
            tool: Some(tool_id),
            selections: Vec::new(),
            boundary: None,
            avoid: Vec::new(),
            check: Vec::new(),
            restriction_options: Default::default(),
            spindle_speed: None,
            material: None,
            parameters: [
                ("stepover", ParameterValue::Number(2.0)),
                ("face_free_ends", ParameterValue::Flag(false)),
                ("adaptive_spacing", ParameterValue::Flag(false)),
            ]
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v))
            .collect(),
        }),
    ];
    for item in items {
        app.execute(Command::Add {
            item,
            placement: Placement::End,
        })
        .unwrap();
    }
    Built {
        dir,
        app,
        operation,
        setup: setup_id,
        tool: tool_id,
    }
}

impl Built {
    fn stored_setup(&self) -> Setup {
        self.app.project().setup(self.setup).unwrap().clone()
    }

    /// Stores the setup as `edit` leaves it, as the workholding section's
    /// Apply and the setup forms do (one undoable `Replace`).
    fn edit_setup(&mut self, edit: impl FnOnce(&mut Setup)) {
        let mut setup = self.stored_setup();
        edit(&mut setup);
        assert_ne!(setup, self.stored_setup(), "the edit changes nothing");
        self.app
            .execute(Command::Replace(Item::Setup(setup)))
            .unwrap();
    }

    fn state(&self) -> ItemState {
        self.app.overview(None, &JobLog::default()).items[&ItemRef::Operation(self.operation)].state
    }

    /// The operation is shown stale, the run is out of date for Simulate
    /// and writing G-code from it is blocked (UI-11, NFR-12; 0172, 0192). A
    /// change to the simulation settings is named as such (0192).
    fn assert_stale(&self, run: &Regeneration, what: &str) {
        assert_eq!(self.state(), ItemState::Stale, "{what}: not stale");
        let reason = openchuck_app::simulation_out_of_date(self.app.project(), run);
        assert!(
            reason.as_deref().is_some_and(|r| {
                r.contains("'Axial Finishing' has changed")
                    || r == "The simulation settings have changed since this simulation."
            }),
            "{what}: Simulate result not out of date: {reason:?}"
        );
        let checks = self.app.export_checks(Some(run), None, None);
        assert!(
            checks.checks.iter().any(|c| c.level == CheckLevel::Blocking
                && c.text.contains("'Axial Finishing' is out of date")),
            "{what}: export not blocked: {:?}",
            checks.checks
        );
    }

    /// The operation is up to date for `run`: generated, with the tab
    /// warning the setup gives (the 4 mm head tab is shorter than the 6 mm
    /// ball; decision 0523), and nothing out of date.
    fn assert_current(&self, run: &Regeneration, what: &str) {
        let state = self.state();
        assert!(
            matches!(state, ItemState::UpToDate | ItemState::Warning),
            "{what}: {state:?}"
        );
        let reason = openchuck_app::simulation_out_of_date(self.app.project(), run);
        assert_eq!(reason, None, "{what}");
        let checks = self.app.export_checks(Some(run), None, None);
        assert!(
            !checks
                .checks
                .iter()
                .any(|c| c.text.contains("is out of date")),
            "{what}: {:?}",
            checks.checks
        );
    }
}

/// Edits a field, checks the operation is stale, generates again and checks
/// the path changed and the operation is up to date (#558).
fn regenerates_differently(name: &str, edit: impl FnOnce(&mut Setup)) {
    let mut b = build(name);
    let before = b.app.regenerate().unwrap();
    b.assert_current(&before, name);
    b.edit_setup(edit);
    b.assert_stale(&before, name);
    let after = b.app.regenerate().unwrap();
    assert_ne!(
        before.toolpath, after.toolpath,
        "{name}: the path did not change"
    );
    b.assert_current(&after, name);
}

#[test]
fn req_ui_11_req_nfr_12_every_setup_machine_tool_and_simulation_edit_makes_the_operation_stale() {
    // One generation; each edit is checked, then undone, which makes the
    // operation up to date again (the same key, so the same result).
    type Edit = Box<dyn Fn(&mut Built)>;
    let setup_edit = |f: fn(&mut Setup)| -> Edit { Box::new(move |b: &mut Built| b.edit_setup(f)) };
    let edits: Vec<(&str, Edit)> = vec![
        (
            "free-end tab length",
            setup_edit(|s| s.workholding.tabs[FREE_END_TAB].length = Millimetres(3.0)),
        ),
        (
            "free-end tab start X",
            setup_edit(|s| s.workholding.tabs[FREE_END_TAB].x_start = Millimetres(31.0)),
        ),
        (
            "free-end tab diameter",
            setup_edit(|s| {
                s.workholding.tabs[FREE_END_TAB].shape = TabShape::Cylinder {
                    diameter: Millimetres(5.0),
                };
            }),
        ),
        (
            "free-end tab shape",
            setup_edit(|s| {
                s.workholding.tabs[FREE_END_TAB].shape = TabShape::Frustum {
                    start_diameter: Millimetres(6.0),
                    end_diameter: Millimetres(9.0),
                };
            }),
        ),
        (
            "free-end tab layout",
            setup_edit(|s| {
                s.workholding.tabs[FREE_END_TAB].layout = TabLayout::AroundAxis {
                    count: 2,
                    start_angle: Degrees(0.0),
                    radial_offset: Millimetres(2.0),
                };
            }),
        ),
        (
            "free-end tab placed automatically",
            setup_edit(|s| s.workholding.tabs[FREE_END_TAB].auto = Some(TabEnd::Tail)),
        ),
        (
            "free-end tab removed",
            setup_edit(|s| {
                s.workholding.tabs.remove(FREE_END_TAB);
            }),
        ),
        (
            "held-end tab length",
            setup_edit(|s| s.workholding.tabs[0].length = Millimetres(3.0)),
        ),
        (
            "held start length",
            setup_edit(|s| {
                s.workholding.start = Some(HeldRegion {
                    length: Millimetres(3.0),
                    diameter: None,
                });
            }),
        ),
        (
            "held start diameter",
            setup_edit(|s| {
                s.workholding.start = Some(HeldRegion {
                    length: Millimetres(4.0),
                    diameter: Some(Millimetres(20.0)),
                });
            }),
        ),
        (
            "free end held",
            setup_edit(|s| {
                s.workholding.end = Some(HeldRegion {
                    length: Millimetres(3.0),
                    diameter: None,
                });
            }),
        ),
        (
            "stock",
            setup_edit(|s| {
                s.stock = Some(Stock::Cylinder {
                    diameter: Millimetres(26.0),
                    length: Millimetres(54.0),
                    x_start: Millimetres(-12.0),
                });
            }),
        ),
        (
            "part offset",
            setup_edit(|s| s.frame.part_offset = Millimetres(1.0)),
        ),
        (
            "machine",
            Box::new(|b: &mut Built| {
                let mut machine = b.app.project().machine().unwrap().clone();
                machine.rotary_origin.z += 1.0;
                b.app
                    .execute(Command::SetMachine(Some(Box::new(machine))))
                    .unwrap();
            }),
        ),
        (
            "tool",
            Box::new(|b: &mut Built| {
                let mut tool = b
                    .app
                    .project()
                    .tools()
                    .iter()
                    .find(|t| t.id == b.tool)
                    .unwrap()
                    .clone();
                tool.tool.diameter = Millimetres(5.0);
                tool.tool.shank_diameter = Millimetres(5.0);
                b.app.execute(Command::Replace(Item::Tool(tool))).unwrap();
            }),
        ),
        (
            "simulation settings",
            Box::new(|b: &mut Built| {
                b.app
                    .execute(Command::SetSimulation(SimulationSettings {
                        cell: Millimetres(1.0),
                    }))
                    .unwrap();
            }),
        ),
    ];
    let mut b = build("matrix");
    let run = b.app.regenerate().unwrap();
    b.assert_current(&run, "generated");
    for (what, edit) in &edits {
        edit(&mut b);
        b.assert_stale(&run, what);
        assert!(b.app.undo().unwrap(), "{what}: nothing to undo");
        b.assert_current(&run, &format!("{what} undone"));
    }
}

#[test]
fn req_op_21_req_ui_11_req_nfr_12_shortening_the_free_end_tab_regenerates_a_different_path() {
    // The owner's case (#558): the tab on the end nothing holds, shortened.
    regenerates_differently("free-length", |s| {
        s.workholding.tabs[FREE_END_TAB].length = Millimetres(3.0);
    });
}

#[test]
fn req_op_21_req_ui_11_req_nfr_12_moving_the_free_end_tab_regenerates_a_different_path() {
    regenerates_differently("free-start", |s| {
        s.workholding.tabs[FREE_END_TAB].x_start = Millimetres(31.0);
    });
}

#[test]
fn req_op_21_req_ui_11_req_nfr_12_narrowing_the_free_end_tab_regenerates_a_different_path() {
    regenerates_differently("free-diameter", |s| {
        s.workholding.tabs[FREE_END_TAB].shape = TabShape::Cylinder {
            diameter: Millimetres(5.0),
        };
    });
}

#[test]
fn req_op_21_req_ui_11_req_nfr_12_shortening_the_held_end_tab_regenerates_a_different_path() {
    regenerates_differently("held-length", |s| {
        s.workholding.tabs[0].length = Millimetres(3.0);
    });
}

#[test]
fn req_set_09_req_ui_11_req_nfr_12_changing_the_grip_regenerates_a_different_path() {
    regenerates_differently("grip", |s| {
        s.workholding.start = Some(HeldRegion {
            length: Millimetres(3.0),
            diameter: None,
        });
    });
}

#[test]
fn req_set_09_req_ui_11_req_nfr_12_holding_the_free_end_regenerates_a_different_path() {
    regenerates_differently("hold-free-end", |s| {
        s.workholding.end = Some(HeldRegion {
            length: Millimetres(3.0),
            diameter: None,
        });
    });
}
