// SPDX-License-Identifier: AGPL-3.0-or-later
//! Machine and setup checks through the application (SET-01, SET-03, SET-05,
//! SET-06; decision record 0011).

// Test code may unwrap, index and panic (NFR-07 applies to library code).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};

use openchuck_app::model::TransformStep;
use openchuck_app::project::id::SetupId;
use openchuck_app::project::{Item, ItemId, Placement, Setup, file};
use openchuck_app::setup::{
    Clearance, MAKERA_Z1, MachineProfile, Millimetres, SetupWarningKind, Stock, builtin,
};
use openchuck_app::{Application, Command, Threads};

/// A project with the corpus cube (10 mm) centred on the rotary axis and one
/// setup machining it.
fn project(name: &str) -> (PathBuf, Application, SetupId) {
    let dir = std::env::temp_dir().join(format!("openchuck-setup-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut app = Application::create(&dir, name, Threads::Exactly(2)).unwrap();
    std::fs::create_dir_all(dir.join("models")).unwrap();
    let cube = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/stl/cube_ascii.stl");
    std::fs::copy(cube, dir.join("models/cube.stl")).unwrap();
    let (model, _) = app.add_model(&dir.join("models/cube.stl"), None).unwrap();
    app.execute(Command::SetModelTransform {
        model,
        transform: vec![TransformStep::CentreOnRotaryAxis],
    })
    .unwrap();
    let id = SetupId::new();
    let mut setup = Setup::new(id, "Setup 1");
    setup.part = Some(model);
    setup.stock = Some(Stock::Cylinder {
        diameter: Millimetres(20.0),
        length: Millimetres(14.0),
        x_start: Millimetres(-2.0),
    });
    app.execute(Command::Add {
        item: Item::Setup(setup),
        placement: Placement::End,
    })
    .unwrap();
    (dir, app, id)
}

#[test]
fn req_set_01_req_set_06_setups_are_checked_against_the_machine() {
    let (dir, mut app, id) = project("checks");

    // No machine yet: the setup is not checked, and the report says so.
    let report = &app.check_setups().unwrap()[0];
    assert!(report.warnings.is_empty());
    assert!(
        report.notes[0].contains("no machine profile"),
        "{:?}",
        report.notes
    );
    let radius = report.part.unwrap().radius.0;
    assert!((radius - 50f64.sqrt()).abs() < 1e-9, "{radius}");

    // With the example machine the setup is clean.
    app.execute(Command::SetMachine(Some(Box::new(
        MachineProfile::example_x_rotary(),
    ))))
    .unwrap();
    let report = &app.check_setups().unwrap()[0];
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
    assert!(report.expected_work_offset.is_some());

    // A clearance inside the stock and part warns about both.
    let mut setup = app.find_setup("Setup 1").unwrap().clone();
    setup.clearance = Some(Clearance {
        radius: Millimetres(6.0),
        safe_z: Millimetres(30.0),
    });
    app.execute(Command::Replace(Item::Setup(setup))).unwrap();
    let kinds: Vec<_> = app.check_setups().unwrap()[0]
        .warnings
        .iter()
        .map(|w| w.kind.clone())
        .collect();
    assert!(
        matches!(
            kinds[..],
            [
                SetupWarningKind::ClearanceInsideStock { .. },
                SetupWarningKind::ClearanceInsidePart { .. }
            ]
        ),
        "{kinds:?}"
    );

    // Undo restores the setup and the machine; the saved folder holds both.
    app.undo().unwrap();
    assert!(app.check_setups().unwrap()[0].warnings.is_empty());
    app.save().unwrap();
    let loaded = file::load(&dir).unwrap();
    assert_eq!(loaded.machine().unwrap().name, "Example X rotary");
    assert_eq!(loaded.setup(id).unwrap(), app.project().setup(id).unwrap());
    app.undo().unwrap();
    assert!(app.project().machine().is_none());
    assert!(app.find_setup("no such setup").is_err());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn req_set_01_z1_warnings_name_unconfirmed_values() {
    let (dir, mut app, _) = project("z1");
    let z1 = builtin(MAKERA_Z1).unwrap().unwrap();
    app.execute(Command::SetMachine(Some(Box::new(z1))))
        .unwrap();
    // Part offset far along the axis: the stock runs past the X travel.
    let mut setup = app.find_setup("Setup 1").unwrap().clone();
    setup.frame.part_offset = Millimetres(195.0);
    app.execute(Command::Replace(Item::Setup(setup))).unwrap();
    let report = &app.check_setups().unwrap()[0];
    let text: Vec<String> = report.warnings.iter().map(ToString::to_string).collect();
    assert!(
        text.iter().any(
            |t| t.starts_with("the end of the stock is at machine X 207.000")
                && t.ends_with(
                    "(uses machine values not confirmed on the machine: rotary_origin, travel)"
                )
        ),
        "{text:?}"
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn req_set_05_part_model_cannot_be_removed_while_a_setup_uses_it() {
    let (dir, mut app, _) = project("in-use");
    let model = app.project().models()[0].id;
    assert!(app.execute(Command::Remove(ItemId::Model(model))).is_err());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn req_set_01_a_direction_on_an_asymmetric_part_matches_the_oracle() {
    // A part with one nub on top (+Z at A 0) and none elsewhere: at A +90 the
    // Z1's documented convention (right-handed about +X, decision record
    // 0007) puts the nub toward the front, machine -Y. The shared kinematics,
    // the independent oracle and the stated direction must all agree. The
    // convention itself stays unconfirmed until the air cut (OI-03, OI-37).
    use openchuck_app::setup::{ASense, a_direction};
    use openchuck_testkit::oracle::{OracleMachine, distance, model_tip};
    use openchuck_types::geometry::Point3;
    use openchuck_types::kinematics::{RotaryFrame, machine_xyz};
    use openchuck_types::setup::SetupFrame;
    use openchuck_types::units::Degrees;

    let z1 = builtin(MAKERA_Z1).unwrap().unwrap();
    assert_eq!(z1.a_sense, ASense::Positive);
    let frame = RotaryFrame::work(&z1, &SetupFrame::default());
    let nub = Point3::new(5.0, 0.0, 12.0);
    let at_90 = machine_xyz(&frame, nub, Degrees(90.0));
    assert!(
        at_90.distance(Point3::new(5.0, -12.0, 0.0)) < 1e-9,
        "{at_90:?}"
    );
    let oracle = OracleMachine {
        origin_x: 0.0,
        origin_y: 0.0,
        origin_z: 0.0,
        sense: 1.0,
    };
    for a in [-90.0, 0.0, 45.0, 90.0, 270.0, 450.0] {
        let xyz = machine_xyz(&frame, nub, Degrees(a));
        assert!(distance(model_tip(&oracle, xyz.x, xyz.y, xyz.z, a), [5.0, 0.0, 12.0]) < 1e-6);
    }
    assert!(a_direction(&z1).ends_with("faces -Y at A 90"));
}
