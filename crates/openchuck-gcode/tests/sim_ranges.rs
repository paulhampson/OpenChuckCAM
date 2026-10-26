// SPDX-License-Identifier: AGPL-3.0-or-later
//! GC-10: a chunk must agree with the original unsplit program's interpreter state.
#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use openchuck_gcode::{compare_simulator_range, parse_sim_report, read};
use serde_json::{Value, json};

const SOURCE: &str =
    "G90 G21 G17 G61 G54\nT2 M6\nS9000 M3\nG0 X1 Y2 Z3 A450\nG1 X11 F60\nX21\nM5\nM30\n";
fn evidence() -> Value {
    let start = json!({"position":[11,2,3,450],"actual_position":[11,2,3,450],
        "gcodes":[10,170,210,540,610,900,940],"feed":60,"rpm":9000,
        "tool":2,"spindle_on":true,"direction":1,"g5x_offset":[0,0,0,0],
        "g92_offset":[0,0,0,0],"rotation_xy":0});
    let mut end = start.clone();
    end["position"] = json!([21, 2, 3, 450]);
    end["actual_position"] = json!([21, 2, 3, 450]);
    end["spindle_on"] = json!(false);
    end["direction"] = json!(0);
    json!({"rows":[],"errors":[],"final_position":[21,2,3,450],
        "path":[[11,2,3,450],[21,2,3,450]],"boundaries":[start,end]})
}
#[test]
fn req_gc_10_source_ranges_use_full_original_prefix_state_and_keep_original_move_numbers() {
    let program = read(SOURCE).unwrap();
    let report = parse_sim_report(&evidence().to_string()).unwrap();
    assert_eq!(
        compare_simulator_range(&program, &report, 6..=8, 0.1)
            .unwrap()
            .moves,
        1
    );
}

#[test]
fn req_gc_10_the_first_chunk_checks_the_original_initial_state() {
    let mut data = evidence();
    let end = data["boundaries"][0].clone();
    let mut start = end.clone();
    start["position"] = json!([0, 0, 0, 0]);
    start["actual_position"] = json!([0, 0, 0, 0]);
    start["gcodes"] = json!([0, 170, 210, 540, 610, 900, 940]);
    start["tool"] = json!(0);
    start["feed"] = json!(0);
    start["rpm"] = json!(0);
    start["spindle_on"] = json!(false);
    start["direction"] = json!(0);
    data["boundaries"] = json!([start, end]);
    data["path"] = json!([[0, 0, 0, 0], [1, 2, 3, 450], [11, 2, 3, 450]]);
    data["final_position"] = json!([11, 2, 3, 450]);
    let report = parse_sim_report(&data.to_string()).unwrap();
    assert_eq!(
        compare_simulator_range(&read(SOURCE).unwrap(), &report, 1..=5, 0.1)
            .unwrap()
            .moves,
        2
    );
}
#[test]
fn req_gc_10_req_nfr_07_a_self_consistent_chunk_cannot_mask_a_wrong_original_boundary() {
    for (key, value) in [
        ("position", json!([10, 2, 3, 450])),
        ("actual_position", json!([10, 2, 3, 450])),
        ("feed", json!(61)),
        ("tool", json!(1)),
        ("rpm", json!(8999)),
        ("spindle_on", json!(false)),
        ("direction", json!(-1)),
        ("gcodes", json!([0, 170, 210, 540, 610, 900, 940])),
        ("g5x_offset", json!([1, 0, 0, 0])),
        ("g92_offset", json!([0, 0, 0, 360])),
        ("rotation_xy", json!(1)),
    ] {
        let mut data = evidence();
        data["boundaries"][0][key] = value;
        let report = parse_sim_report(&data.to_string()).unwrap();
        let error =
            compare_simulator_range(&read(SOURCE).unwrap(), &report, 6..=8, 0.1).unwrap_err();
        assert!(error.to_string().contains("boundary"), "{key}: {error}");
    }
    let mut report = parse_sim_report(&evidence().to_string()).unwrap();
    report.boundaries[0].position[0] = f64::NAN;
    assert!(compare_simulator_range(&read(SOURCE).unwrap(), &report, 6..=8, 0.1).is_err());
}
#[test]
fn req_gc_10_req_nfr_07_missing_boundaries_empty_ranges_and_unsupported_modes_fail() {
    let program = read(SOURCE).unwrap();
    let report = parse_sim_report(&evidence().to_string()).unwrap();
    for (first, last) in [(0, 8), (8, 6), (6, 99), (7, 8)] {
        assert!(compare_simulator_range(&program, &report, first..=last, 0.1).is_err());
    }
    for tolerance in [f64::NAN, f64::INFINITY, -1.0] {
        assert!(compare_simulator_range(&program, &report, 6..=8, tolerance).is_err());
    }
    let mut missing = report.clone();
    missing.boundaries.clear();
    assert!(compare_simulator_range(&program, &missing, 6..=8, 0.1).is_err());
    for source in [
        SOURCE.replace("G90", "G91"),
        SOURCE.replace("M3\n", "M4\n"),
        format!("{SOURCE}G0 X1\n"),
        SOURCE.replace("M30", "M5 M30"),
    ] {
        let unsupported = read(&source).unwrap();
        assert!(compare_simulator_range(&unsupported, &report, 6..=8, 0.1).is_err());
    }
}

#[test]
fn req_gc_10_faults_wrong_end_state_and_empty_execution_cannot_pass() {
    let program = read(SOURCE).unwrap();
    let mut report = parse_sim_report(&evidence().to_string()).unwrap();
    report.errors.push("following error".into());
    assert!(
        compare_simulator_range(&program, &report, 6..=8, 0.1)
            .unwrap_err()
            .to_string()
            .contains("following error")
    );
    let mut report = parse_sim_report(&evidence().to_string()).unwrap();
    report.boundaries[1].actual_position[0] += 1.0;
    assert!(
        compare_simulator_range(&program, &report, 6..=8, 0.1)
            .unwrap_err()
            .to_string()
            .contains("boundary end")
    );
    let mut data = evidence();
    data["boundaries"][0]["position"] = json!([21, 2, 3, 450]);
    data["boundaries"][0]["actual_position"] = json!([21, 2, 3, 450]);
    data["path"] = json!([[21, 2, 3, 450]]);
    let report = parse_sim_report(&data.to_string()).unwrap();
    assert!(
        compare_simulator_range(&program, &report, 7..=8, 0.1)
            .unwrap_err()
            .to_string()
            .contains("no original moves")
    );
}
