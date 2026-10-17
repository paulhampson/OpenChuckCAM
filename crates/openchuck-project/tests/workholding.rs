// SPDX-License-Identifier: AGPL-3.0-or-later
//! Held regions and retaining tabs in the setup (SET-09, OP-21; decision
//! records 0035 and 0041).

// Test code may unwrap, index and panic (NFR-07 applies to library code).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::PathBuf;

use openchuck_project::id::SetupId;
use openchuck_project::{Item, Placement, Project, Setup, file};
use openchuck_types::stock::Stock;
use openchuck_types::units::{Degrees, Millimetres};
use openchuck_types::workholding::{HeldRegion, Tab, TabLayout, TabShape, Workholding};

const SETUP: SetupId = SetupId::from_u128(10);

fn mm(v: f64) -> Millimetres {
    Millimetres(v)
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("openchuck-hold-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// Stock 40 mm across, X from 0 to 100.
fn project_with(workholding: Workholding) -> Project {
    let mut p = Project::new("hold");
    let mut setup = Setup::new(SETUP, "Setup");
    setup.stock = Some(Stock::Cylinder {
        diameter: mm(40.0),
        length: mm(100.0),
        x_start: mm(0.0),
    });
    setup.workholding = workholding;
    p.insert(Item::Setup(setup), Placement::End).unwrap();
    p
}

fn full() -> Workholding {
    Workholding {
        start: Some(HeldRegion {
            length: mm(15.0),
            diameter: Some(mm(30.0)),
        }),
        end: Some(HeldRegion {
            length: mm(10.0),
            diameter: None,
        }),
        tabs: vec![
            Tab {
                x_start: mm(80.0),
                length: mm(6.0),
                shape: TabShape::Frustum {
                    start_diameter: mm(8.0),
                    end_diameter: mm(3.0),
                },
                layout: TabLayout::OnAxis,
            },
            Tab {
                x_start: mm(40.0),
                length: mm(5.0),
                shape: TabShape::Cylinder { diameter: mm(4.0) },
                layout: TabLayout::AroundAxis {
                    count: 3,
                    start_angle: Degrees(30.0),
                    radial_offset: mm(12.0),
                },
            },
        ],
    }
}

#[test]
fn req_set_09_req_op_21_workholding_survives_save_and_load() {
    let project = project_with(full());
    let dir = temp_dir("roundtrip");
    file::save(&project, &dir).unwrap();
    let back = file::load(&dir).unwrap();
    assert_eq!(back, project);
    assert_eq!(
        file::render(&back).unwrap(),
        file::render(&project).unwrap()
    );
}

#[test]
fn req_set_09_req_op_21_no_workholding_writes_nothing() {
    let project = project_with(Workholding::default());
    let text = file::render(&project).unwrap();
    for (path, text) in &text {
        assert!(!text.contains("workholding"), "{path}: {text}");
    }
}

#[test]
fn req_prj_04_older_setup_loads_with_no_workholding() {
    let project = project_with(Workholding::default());
    let dir = temp_dir("migrate");
    file::save(&project, &dir).unwrap();
    for entry in std::fs::read_dir(dir.join("setups")).unwrap() {
        let path = entry.unwrap().path();
        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::write(&path, text.replacen("schema = 12", "schema = 9", 1)).unwrap();
    }
    let text = std::fs::read_to_string(dir.join("project.toml")).unwrap();
    std::fs::write(
        dir.join("project.toml"),
        text.replacen("schema = 12", "schema = 9", 1),
    )
    .unwrap();
    let back = file::load(&dir).unwrap();
    assert!(back.setup(SETUP).unwrap().workholding.is_empty());
}

fn refused(edit: impl FnOnce(&mut Workholding), expected: &str) {
    let base = project_with(full());
    let mut setup = base.setup(SETUP).unwrap().clone();
    edit(&mut setup.workholding);
    let mut p = base.clone();
    let err = p.replace(Item::Setup(setup)).unwrap_err().to_string();
    assert!(err.contains(expected), "{expected}: {err}");
    assert_eq!(p, base);
}

#[test]
fn req_set_09_impossible_held_regions_are_refused() {
    refused(
        |w| w.start.as_mut().unwrap().length = mm(0.0),
        "start.length",
    );
    refused(
        |w| w.start.as_mut().unwrap().length = mm(f64::NAN),
        "start.length",
    );
    refused(
        |w| w.start.as_mut().unwrap().length = mm(120.0),
        "longer than the stock",
    );
    refused(
        |w| w.end.as_mut().unwrap().diameter = Some(mm(50.0)),
        "wider than the stock",
    );
    refused(
        |w| w.end.as_mut().unwrap().diameter = Some(mm(-1.0)),
        "end.diameter",
    );
    refused(|w| w.end.as_mut().unwrap().length = mm(90.0), "overlap");
}

#[test]
fn req_op_21_impossible_tabs_are_refused() {
    refused(|w| w.tabs[0].length = mm(0.0), "tabs[0].length");
    refused(|w| w.tabs[0].x_start = mm(-3.0), "not inside the stock");
    refused(|w| w.tabs[0].x_start = mm(97.0), "not inside the stock");
    refused(|w| w.tabs[0].x_start = mm(92.0), "held region at the end");
    refused(|w| w.tabs[1].x_start = mm(10.0), "held region at the start");
    refused(
        |w| {
            w.tabs[0].shape = TabShape::Frustum {
                start_diameter: mm(5.0),
                end_diameter: mm(5.0),
            }
        },
        "use a cylinder",
    );
    refused(
        |w| w.tabs[0].shape = TabShape::Cylinder { diameter: mm(-2.0) },
        "tabs[0].diameter",
    );
    refused(
        |w| w.tabs[0].shape = TabShape::Cylinder { diameter: mm(41.0) },
        "outside the stock",
    );
    refused(
        |w| {
            w.tabs[1].layout = TabLayout::AroundAxis {
                count: 0,
                start_angle: Degrees(0.0),
                radial_offset: mm(5.0),
            }
        },
        "tabs[1].count",
    );
    refused(
        |w| {
            w.tabs[1].layout = TabLayout::AroundAxis {
                count: 3,
                start_angle: Degrees(0.0),
                radial_offset: mm(19.0),
            }
        },
        "outside the stock",
    );
    refused(
        |w| {
            let mut t = w.tabs[0];
            t.x_start = mm(83.0);
            w.tabs.push(t);
        },
        "overlaps",
    );
}
