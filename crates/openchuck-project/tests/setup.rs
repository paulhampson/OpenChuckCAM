// SPDX-License-Identifier: AGPL-3.0-or-later
//! Machine, stock and setup persistence (SET-01, SET-03, SET-05, SET-06;
//! decision record 0011).

// Test code may unwrap, index and panic (NFR-07 applies to library code).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};

use openchuck_project::id::{ModelId, SetupId};
use openchuck_project::machines::{MAKERA_Z1, builtin};
use openchuck_project::{
    ContentHash, Item, ItemId, ModelRef, Placement, Project, ProjectError, RelativePath, Setup,
    file,
};
use openchuck_types::geometry::{Aabb, Point3};
use openchuck_types::machine::{Clearance, MachineProfile};
use openchuck_types::setup::{SetupFrame, WorkOffset, WorkOffsetRegister};
use openchuck_types::stock::Stock;
use openchuck_types::units::Millimetres;

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("openchuck-setup-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

const MODEL: ModelId = ModelId::from_u128(1);

fn z1() -> MachineProfile {
    builtin(MAKERA_Z1).unwrap().unwrap()
}

fn project_with(stocks: &[Stock]) -> Project {
    let mut p = Project::new("setups");
    p.set_machine(Some(z1())).unwrap();
    p.insert(
        Item::Model(ModelRef::new(
            MODEL,
            RelativePath::new("models/part.stl").unwrap(),
            ContentHash::of_bytes(b"part"),
        )),
        Placement::End,
    )
    .unwrap();
    for (i, stock) in stocks.iter().enumerate() {
        let mut setup = Setup::new(SetupId::from_u128(10 + i as u128), format!("Setup {i}"));
        setup.part = Some(MODEL);
        setup.stock = Some(stock.clone());
        setup.material = Some("6061 aluminium".into());
        setup.frame = SetupFrame {
            part_offset: Millimetres(12.5),
            work_offset: WorkOffset {
                register: WorkOffsetRegister::G55,
                x: Millimetres(-2.0),
            },
        };
        setup.clearance = Some(Clearance {
            radius: Millimetres(30.0),
            safe_z: Millimetres(40.0),
        });
        p.insert(Item::Setup(setup), Placement::End).unwrap();
    }
    p
}

fn all_stocks() -> Vec<Stock> {
    vec![
        Stock::Cylinder {
            diameter: Millimetres(40.0),
            length: Millimetres(80.0),
            x_start: Millimetres(-5.0),
        },
        Stock::Box {
            bounds: Aabb {
                min: Point3::new(0.0, -15.0, -15.0),
                max: Point3::new(70.0, 15.0, 15.0),
            },
        },
        Stock::OffsetBoundingBox {
            offset: Millimetres(1.5),
        },
    ]
}

#[test]
fn req_set_03_req_set_05_all_three_stock_types_survive_save_and_load() {
    let project = project_with(&all_stocks());
    let dir = temp_dir("stocks");
    file::save(&project, &dir).unwrap();
    let back = file::load(&dir).unwrap();
    assert_eq!(back, project);
    // Saving the loaded project rewrites nothing.
    assert_eq!(
        file::render(&back).unwrap(),
        file::render(&project).unwrap()
    );
    let text =
        std::fs::read_to_string(dir.join("setups/00000000-0000-0000-0000-00000000000a.toml"))
            .unwrap();
    for line in [
        "material = \"6061 aluminium\"",
        "part_offset = 12.5",
        "register = \"g55\"",
        "type = \"cylinder\"",
        "safe_z = 40.0",
    ] {
        assert!(text.contains(line), "{line} missing from:\n{text}");
    }
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn req_set_01_machine_copy_survives_save_and_load() {
    let mut project = project_with(&[]);
    let dir = temp_dir("machine");
    file::save(&project, &dir).unwrap();
    let text = std::fs::read_to_string(dir.join(file::MACHINE_FILE)).unwrap();
    assert!(
        text.starts_with("schema = 5\n\n[machine]\nname = \"Makera Z1\"\n"),
        "{text}"
    );
    assert_eq!(file::load(&dir).unwrap().machine(), Some(&z1()));

    // Editing one value of the copy changes one line.
    let mut edited = z1();
    edited.max_feeds.linear.0 = 1000.0;
    project.set_machine(Some(edited)).unwrap();
    file::save(&project, &dir).unwrap();
    let after = std::fs::read_to_string(dir.join(file::MACHINE_FILE)).unwrap();
    let changed = text
        .lines()
        .zip(after.lines())
        .filter(|(a, b)| a != b)
        .count();
    assert_eq!((changed, text.lines().count()), (1, after.lines().count()));

    // No machine, no file.
    project.set_machine(None).unwrap();
    file::save(&project, &dir).unwrap();
    assert!(!dir.join(file::MACHINE_FILE).exists());
    assert_eq!(file::load(&dir).unwrap().machine(), None);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn req_set_01_invalid_machine_is_refused() {
    let mut project = Project::new("p");
    let mut bad = z1();
    bad.clearance.safe_z = Millimetres(1.0);
    assert!(project.set_machine(Some(bad)).is_err());
    assert_eq!(project.machine(), None);

    let dir = temp_dir("bad-machine");
    file::save(&project_with(&[]), &dir).unwrap();
    let path = dir.join(file::MACHINE_FILE);
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        text.replace("rotary_axis = \"x\"", "rotary_axis = \"z\""),
    )
    .unwrap();
    let err = file::load(&dir).unwrap_err().to_string();
    assert!(
        err.contains("machine.toml") && err.contains("rotary_axis"),
        "{err}"
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn req_set_05_req_set_06_invalid_setups_are_refused() {
    let base = project_with(&all_stocks()[..1]);
    let setup = base.setup(SetupId::from_u128(10)).unwrap().clone();
    let cases: Vec<(Setup, &str)> = vec![
        (
            Setup {
                clearance: Some(Clearance {
                    radius: Millimetres(30.0),
                    safe_z: Millimetres(20.0),
                }),
                ..setup.clone()
            },
            "safe Z",
        ),
        (
            Setup {
                frame: SetupFrame {
                    part_offset: Millimetres(f64::NAN),
                    ..setup.frame
                },
                ..setup.clone()
            },
            "part_offset",
        ),
        (
            Setup {
                stock: Some(Stock::Cylinder {
                    diameter: Millimetres(0.0),
                    length: Millimetres(10.0),
                    x_start: Millimetres(0.0),
                }),
                ..setup.clone()
            },
            "stock.diameter",
        ),
        (
            Setup {
                material: Some("  ".into()),
                ..setup.clone()
            },
            "material",
        ),
        (
            Setup {
                part: Some(ModelId::from_u128(99)),
                ..setup.clone()
            },
            "does not exist",
        ),
    ];
    for (edited, expected) in cases {
        let mut p = base.clone();
        let err = p.replace(Item::Setup(edited)).unwrap_err().to_string();
        assert!(err.contains(expected), "{expected}: {err}");
        assert_eq!(p, base);
    }
}

#[test]
fn req_set_05_a_part_in_use_by_a_setup_cannot_be_removed() {
    let mut p = project_with(&all_stocks()[..1]);
    let err = p.remove(ItemId::Model(MODEL)).unwrap_err();
    assert!(matches!(err, ProjectError::InUse { .. }), "{err}");
}

#[test]
fn req_prj_04_slice_two_folder_with_a_machine_name_migrates() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/schema-2");
    let project = file::load(&fixture).unwrap();
    assert_eq!(project.name(), "Schema 2 fixture");
    // The placeholder machine name cannot become a profile; it is dropped.
    assert_eq!(project.machine(), None);
    let setup = &project.setups()[0];
    assert_eq!(setup.frame, SetupFrame::default());
    assert_eq!((setup.stock.as_ref(), setup.clearance), (None, None));
}
