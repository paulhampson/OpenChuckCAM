// SPDX-License-Identifier: AGPL-3.0-or-later
//! Gate 4's measured Z1 setting (SET-01, GC-09; decision 0222 item 7).
//! Written before the implementation: until the Report/profile work exists,
//! this test exposes the missing follow-through rather than accepting an
//! inert TOML key as a working controller check.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]

#[test]
fn req_set_01_req_gc_09_z1_records_the_measured_block_time_and_queue_window() {
    let text = include_str!("../machines/makera-z1.toml");
    let profile: toml::Value = toml::from_str(text).expect("built-in profile is TOML");
    let machine = &profile["machine"];
    assert_eq!(
        machine.get("min_block_ms").and_then(toml::Value::as_float),
        Some(7.0),
        "gate 4: 7 ms passed in both parts; 5, 3, 2 and 1 ms stuttered"
    );
    assert_eq!(
        machine
            .get("block_window")
            .and_then(toml::Value::as_integer),
        Some(32),
        "decision 0222 keeps the accepted averaging window"
    );
    assert_eq!(
        machine
            .get("short_block_mode")
            .and_then(toml::Value::as_str),
        Some("report"),
        "Report first; no slow-down or merge in this lane"
    );
    // A value that the loader cannot read is not an implemented setting.
    openchuck_project::machines::builtin(openchuck_project::machines::MAKERA_Z1)
        .expect("measured profile loads")
        .expect("built-in Z1 exists");
}

#[test]
fn req_gc_17_req_prj_04_older_z1_copies_stay_off_and_other_profiles_default_off() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let project = openchuck_project::file::load(&root.join("tests/fixtures/schema-11-z1")).unwrap();
    let copy = project.machine().unwrap();
    assert_eq!(copy.min_block_ms, 0.0);
    assert_eq!(copy.block_window, 1);
    assert_eq!(
        copy.short_block_mode,
        openchuck_types::machine::ShortBlockMode::Off
    );
    let example = openchuck_types::machine::MachineProfile::example_x_rotary();
    assert_eq!(example.min_block_ms, 0.0);
    assert_eq!(example.block_window, 1);
    assert_eq!(
        example.short_block_mode,
        openchuck_types::machine::ShortBlockMode::Off
    );
}

#[test]
fn req_gc_17_req_prj_04_new_fields_need_the_claimed_schema() {
    let text = include_str!("../machines/makera-z1.toml");
    let old = text.replacen("schema = 5", "schema = 4", 1);
    let error = openchuck_project::machines::parse_profile(std::path::Path::new("old.toml"), &old)
        .unwrap_err();
    assert!(error.to_string().contains("predates"));
}

#[test]
fn req_gc_17_off_does_not_add_unconfirmed_machine_warnings_to_older_projects() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let project = openchuck_project::file::load(&root.join("tests/fixtures/schema-11-z1")).unwrap();
    let fields = project.machine().unwrap().unconfirmed_fields();
    assert!(!fields.iter().any(|field| matches!(
        field,
        openchuck_types::machine::ProfileField::MinBlockMs
            | openchuck_types::machine::ProfileField::BlockWindow
            | openchuck_types::machine::ProfileField::ShortBlockMode
    )));
}
