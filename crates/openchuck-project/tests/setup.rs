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
        text.starts_with(&format!(
            "schema = {}\n\n[machine]\nname = \"Makera Z1\"\n",
            file::SCHEMA
        )),
        "{text}"
    );
    assert_eq!(file::load(&dir).unwrap().machine(), Some(&z1()));

    // A copy that records its library source writes the ID beside the
    // profile, not in it, and reads it back (OI-128).
    let mut with_id = project.clone();
    with_id
        .set_machine_from(
            Some(z1()),
            Some("8f0c2a4e-1b7d-4e3a-9c61-5d2e7f4a1b01".into()),
        )
        .unwrap();
    let id_dir = temp_dir("machine-id");
    file::save(&with_id, &id_dir).unwrap();
    let id_text = std::fs::read_to_string(id_dir.join(file::MACHINE_FILE)).unwrap();
    assert!(
        id_text.starts_with(
            &format!(
                "schema = {}\nid = \"8f0c2a4e-1b7d-4e3a-9c61-5d2e7f4a1b01\"\n\n[machine]\nname = \"Makera Z1\"\n",
                file::SCHEMA
            )
        ),
        "{id_text}"
    );
    let loaded = file::load(&id_dir).unwrap();
    assert_eq!(
        loaded.machine_id(),
        Some("8f0c2a4e-1b7d-4e3a-9c61-5d2e7f4a1b01")
    );
    assert_eq!(loaded, with_id);
    std::fs::remove_dir_all(&id_dir).ok();

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

#[test]
fn req_prj_04_req_op_13_an_older_z1_copy_migrates_to_count_a_in_its_feed_word() {
    use openchuck_types::machine::RotaryInFeed;
    // Schema 11 kept no `rotary_in_feed`; a copy of the built-in Z1 (known by
    // its library ID) gains the Z1's rule (decision record 0076).
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/schema-11-z1");
    let project = file::load(&fixture).unwrap();
    let machine = project.machine().unwrap();
    assert_eq!(machine.max_feeds.rotary_in_feed, RotaryInFeed::Makera);
    assert_eq!(machine.max_feeds, z1().max_feeds);
    assert_eq!(z1().max_feeds.rotary_in_feed, RotaryInFeed::Makera);

    // Any other profile keeps A counted separately, as it was timed before.
    let dir = temp_dir("schema-11-other");
    std::fs::create_dir_all(&dir).unwrap();
    for name in ["project.toml", "machine.toml"] {
        let text = std::fs::read_to_string(fixture.join(name)).unwrap();
        std::fs::write(
            dir.join(name),
            text.replace("8f0c2a4e-1b7d-4e3a-9c61-5d2e7f4a1b01", "my-own-machine"),
        )
        .unwrap();
    }
    let other = file::load(&dir).unwrap();
    assert_eq!(
        other.machine().unwrap().max_feeds.rotary_in_feed,
        RotaryInFeed::Separate
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn req_prj_04_req_set_01_a_z1_copy_without_an_id_migrates_by_its_name() {
    use openchuck_types::machine::RotaryInFeed;
    // A Z1 copy stored through `Command::SetMachine` before schema 13 has no
    // library ID, only the profile's name; it is still the Z1 and gains its
    // rule (decision records 0076 item 3 and 0109, OI-253).
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/schema-11-z1");
    let copy = |tag: &str, name: &str| {
        let dir = temp_dir(tag);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::copy(fixture.join("project.toml"), dir.join("project.toml")).unwrap();
        let machine = std::fs::read_to_string(fixture.join("machine.toml")).unwrap();
        let machine: String = machine
            .lines()
            .filter(|l| !l.starts_with("id = "))
            .map(|l| {
                if l == "name = \"Makera Z1\"" {
                    format!("name = \"{name}\"\n")
                } else {
                    format!("{l}\n")
                }
            })
            .collect();
        assert!(!machine.contains("rotary_in_feed ="));
        std::fs::write(dir.join("machine.toml"), machine).unwrap();
        let feed = file::load(&dir)
            .unwrap()
            .machine()
            .unwrap()
            .max_feeds
            .rotary_in_feed;
        std::fs::remove_dir_all(&dir).ok();
        feed
    };
    assert_eq!(
        copy("schema-11-z1-no-id", "Makera Z1"),
        RotaryInFeed::Makera
    );
    // A profile with no ID and another name keeps A counted separately.
    assert_eq!(
        copy("schema-11-other-no-id", "My mill"),
        RotaryInFeed::Separate
    );
}

/// Before schema 17 a setup's part offset placed the part, the stock, the
/// tabs and the work zero together, so it changed no toolpath; from 17 it
/// moves only the part. An older setup with an offset has its stock, tabs and
/// work zero moved by it, so it machines exactly as before; a setup without
/// one is unchanged (PRJ-04; decision record 0201).
#[test]
fn req_prj_04_req_set_05_an_older_part_offset_moves_the_stock_tabs_and_work_zero_with_it() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/schema-15-offset");
    let project = file::load(&fixture).unwrap();
    let by_name = |name: &str| project.setups().iter().find(|s| s.name == name).unwrap();

    let cylinder = by_name("Cylinder");
    assert_eq!(cylinder.frame.part_offset, Millimetres(20.0));
    assert_eq!(
        cylinder.frame.work_offset,
        WorkOffset {
            register: WorkOffsetRegister::G55,
            x: Millimetres(18.0),
        }
    );
    assert_eq!(
        cylinder.stock,
        Some(Stock::Cylinder {
            diameter: Millimetres(20.0),
            length: Millimetres(34.0),
            x_start: Millimetres(18.0),
        })
    );
    // The held region is measured from the end of the stock, so it moves with
    // it unchanged; the tab moves by the offset and stays a manual tab.
    let held = cylinder.workholding.start.unwrap();
    assert_eq!(held.length, Millimetres(4.0));
    let tab = cylinder.workholding.tabs[0];
    assert_eq!(
        (tab.x_start, tab.length),
        (Millimetres(23.0), Millimetres(2.0))
    );
    assert_eq!(tab.auto, None);

    // Whole numbers and a missing work offset (model X 0) are read too.
    let boxed = by_name("Box");
    assert_eq!(boxed.frame.work_offset.x, Millimetres(5.0));
    assert_eq!(
        boxed.frame.work_offset.register,
        WorkOffsetRegister::default()
    );
    let Some(Stock::Box { bounds }) = boxed.stock else {
        panic!("box stock expected");
    };
    assert_eq!((bounds.min.x, bounds.max.x), (2.0, 45.0));
    assert_eq!((bounds.min.y, bounds.max.z), (-10.0, 10.0));

    // Stock that follows the part moves with it already.
    let bounds = by_name("Bounds");
    assert_eq!(
        bounds.stock,
        Some(Stock::OffsetBoundingBox {
            offset: Millimetres(2.0)
        })
    );
    assert_eq!(bounds.frame.work_offset.x, Millimetres(3.0));

    let still = by_name("Still");
    assert_eq!(still.frame, SetupFrame::default());
    assert_eq!(
        still.stock,
        Some(Stock::Cylinder {
            diameter: Millimetres(20.0),
            length: Millimetres(34.0),
            x_start: Millimetres(-2.0),
        })
    );

    // Saved again, the folder is schema 17 and loads to the same project.
    let dir = temp_dir("schema-15-offset");
    file::save(&project, &dir).unwrap();
    let again = file::load(&dir).unwrap();
    assert_eq!(again.setups(), project.setups());
    std::fs::remove_dir_all(&dir).ok();
}

/// A tab's `auto` is new in schema 17 (decision record 0202): an older
/// file that holds one was not written by a version that knew it, so it is
/// refused with what to do; so is an older offset or position that is not a
/// number, which the step cannot move (NFR-07).
#[test]
fn req_prj_04_req_nfr_07_an_older_tab_with_auto_or_a_bad_offset_is_refused() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/schema-15-offset");
    let dir = temp_dir("schema-16-auto");
    std::fs::create_dir_all(dir.join("setups")).unwrap();
    std::fs::write(dir.join("project.toml"), "schema = 16\nname = \"auto\"\n").unwrap();
    let setup =
        std::fs::read_to_string(fixture.join("setups/00000000-0000-0000-0000-00000000000a.toml"))
            .unwrap()
            .replace("schema = 15", "schema = 16")
            .replace("x_start = 3.0\n", "x_start = 3.0\nauto = \"head\"\n");
    std::fs::write(
        dir.join("setups/00000000-0000-0000-0000-00000000000a.toml"),
        setup,
    )
    .unwrap();
    let error = file::load(&dir).unwrap_err().to_string();
    assert!(
        error.contains("`auto`, a field schema 17 introduced"),
        "{error}"
    );
    // An offset or a position that is not a number cannot be moved.
    let setup =
        std::fs::read_to_string(fixture.join("setups/00000000-0000-0000-0000-00000000000a.toml"))
            .unwrap()
            .replace("schema = 15", "schema = 16");
    for (from, to, what) in [
        (
            "part_offset = 20.0",
            "part_offset = \"far\"",
            "`part_offset` is not a number",
        ),
        (
            "x_start = -2.0",
            "x_start = \"start\"",
            "`stock.x_start` is not a number",
        ),
    ] {
        std::fs::write(
            dir.join("setups/00000000-0000-0000-0000-00000000000a.toml"),
            setup.replace(from, to),
        )
        .unwrap();
        let error = file::load(&dir).unwrap_err().to_string();
        assert!(error.contains(what), "{error}");
    }
    std::fs::remove_dir_all(&dir).ok();
}
