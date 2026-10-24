// SPDX-License-Identifier: AGPL-3.0-or-later
//! Decision 0209 M4: a differing path or posted program must reject the
//! measurement, as must a failed replay gate or a different application path.
//! These are rejection-policy tests, not geometry or dispatch equivalence
//! tests. They need no STEP kernel; the full M4 run must establish equivalence.

#[path = "../examples/step_accuracy/checks.rs"]
mod checks;

#[test]
fn req_gc_07_req_nfr_06_m4_rejects_changed_gcode_and_toolpaths() {
    let gcode = "G1 X1 F100\n";
    assert!(checks::identical(gcode, gcode, "form b, 2 threads").is_ok());
    let changed = checks::identical(gcode, "G1 X1 F101\n", "form b, 2 threads");
    assert!(changed.is_err());
    assert!(changed.is_err_and(|message| message.contains("form b, 2 threads")));
    // Equal-length serialized paths can still represent different motions.
    let path = r#"[{"x":1,"a":90}]"#;
    assert!(checks::identical(path, path, "form c toolpath").is_ok());
    assert!(checks::identical(path, r#"[{"x":2,"a":90}]"#, "form c toolpath").is_err());
}

#[test]
fn req_op_23_m4_gate_rejects_differing_harness_and_application_paths() {
    let harness = [(10.0, 0.0), (11.0, 90.0)];
    let application = [(10.0, 0.0), (11.0, 91.0)];
    assert!(checks::identical(&harness, &application, "form b application path").is_err());
    assert!(checks::identical(&application, &application, "form b application path").is_ok());
}

#[test]
fn req_gc_09_m4_rejects_unclean_replay_even_when_output_is_identical() {
    assert!(checks::clean(true, "form a replay gates").is_ok());
    let result = checks::clean(false, "form a replay gates");
    assert!(result.is_err());
    assert!(result.is_err_and(|message| message.contains("form a replay gates")));
}
