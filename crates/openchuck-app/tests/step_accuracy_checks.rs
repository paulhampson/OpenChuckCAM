// SPDX-License-Identifier: AGPL-3.0-or-later
//! Decision 0209 M2/M4: an unproven sampling bound, differing path or posted program rejects the
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

#[test]
fn req_op_12_m2_requires_a_finite_proven_sub_micron_sampling_bound() {
    for bound in [
        None,
        Some(-0.1),
        Some(f64::NAN),
        Some(f64::INFINITY),
        Some(0.001),
        Some(0.006666667),
    ] {
        assert!(
            checks::sampling_bound(bound).is_err(),
            "invalid bound: {bound:?}"
        );
    }
    assert!(checks::sampling_bound(Some(0.0)).is_ok());
    assert!(checks::sampling_bound(Some(0.0009)).is_ok());
}

#[test]
fn req_op_23_m4_paths_agree_only_within_a_stated_rounding() {
    // OI-1110 (decision 0277): the harness's and the application's indexed
    // finishing differ by about 1e-12 mm when roughing and rest run first,
    // with byte-identical G-code. The path check then accepts numbers within
    // a stated rounding, reports the largest difference, and still rejects
    // any other difference.
    let path = serde_json::json!([{"x": 10.0, "a": 90.0, "kind": "cutting"}]);
    assert_eq!(checks::within_rounding(&path, &path, 1e-9, "p"), Ok(0.0));
    let rounded = serde_json::json!([{"x": 10.000_000_000_001, "a": 90.0, "kind": "cutting"}]);
    let got = checks::within_rounding(&path, &rounded, 1e-9, "p").unwrap();
    assert!(got > 0.0 && got < 1e-11, "{got}");
    for other in [
        serde_json::json!([{"x": 10.000_001, "a": 90.0, "kind": "cutting"}]),
        serde_json::json!([{"x": 10.0, "a": 90.0, "kind": "rapid"}]),
        serde_json::json!([{"x": 10.0, "a": 90.0}]),
        serde_json::json!([{"x": 10.0, "a": 90.0, "kind": "cutting"}, {"x": 1.0}]),
    ] {
        let r = checks::within_rounding(&path, &other, 1e-9, "form d application path");
        assert!(r.is_err_and(|m| m.contains("form d application path")), "{other}");
    }
}

#[test]
fn req_op_12_m2_says_when_a_cut_or_a_gap_is_over_the_tolerance() {
    // Decision 0277: the import tessellation's 0.042 mm cut is a clear
    // failure against the 0.01 mm toolpath tolerance; the exact drop's
    // 0.0045 mm is within it.
    assert_eq!(checks::verdict(0.0045, 0.01), "yes");
    assert_eq!(checks::verdict(0.01, 0.01), "yes");
    assert_eq!(checks::verdict(0.0426, 0.01), "NO");
    assert_eq!(checks::verdict(f64::NAN, 0.01), "NO");
}
