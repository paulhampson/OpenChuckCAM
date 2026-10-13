// SPDX-License-Identifier: AGPL-3.0-or-later
//! Gate 3: the simulator report against the replay (work breakdown slice 6
//! items 9 and 11). The report is built from the replay itself, so these
//! tests prove the comparison, not LinuxCNC; `tools/linuxcnc/run_replay.sh`
//! produces the real one.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing
)]

use openchuck_gcode::{
    Dialect, Replay, SimError, SimReport, SimRow, compare_simulator, parse_sim_report, read, replay,
};

const PROGRAM: &str =
    "G90 G21 G17\nG54\nG0 X0 Y0 Z30 A0\nG1 Z10 F100\nX5 A90 F300\nX10 A180\nG0 Z30\nM2\n";
const NUMBERED: &str = "N10 G90 G21\nN20 G0 X0 Z30\nN30 G1 X5 A90 F300\nN40 M2\n";

fn replayed(source: &str) -> Replay {
    replay(&read(source).unwrap(), Dialect::Linuxcnc).unwrap()
}

/// What a faithful simulator would report for a replay.
fn faithful(replay: &Replay) -> SimReport {
    let rows = replay
        .moves()
        .filter(|m| m.from != m.to)
        .map(|m| SimRow {
            line: m.sequence.unwrap_or(m.line),
            seconds: 1.0,
            first: Some(m.from),
            end: m.to,
        })
        .collect();
    SimReport {
        rows,
        errors: Vec::new(),
        final_position: replay.moves().last().unwrap().to,
        path: Vec::new(),
    }
}

#[test]
fn req_gc_10_a_faithful_report_matches() {
    for source in [PROGRAM, NUMBERED] {
        let r = replayed(source);
        let c = compare_simulator(&r, &faithful(&r), 0.01).unwrap();
        assert!(c.moves >= 2);
        assert_eq!(c.max_error, 0.0);
    }
}

#[test]
fn req_gc_10_a_wrong_end_a_missing_line_a_fault_and_a_wrong_stop_are_caught() {
    let r = replayed(PROGRAM);
    let good = faithful(&r);

    let mut wrong = good.clone();
    wrong.rows[2].end[0] += 0.5;
    wrong.rows[3].first = wrong.rows[3].first.map(|mut f| {
        f[0] += 0.5;
        f
    });
    assert!(
        matches!(compare_simulator(&r, &wrong, 0.01), Err(SimError::Mismatch(m)) if m.contains("line 5"))
    );

    let mut missing = good.clone();
    missing.rows.remove(1);
    assert!(
        matches!(compare_simulator(&r, &missing, 0.01), Err(SimError::Mismatch(m)) if m.contains("did not move"))
    );

    let mut fault = good.clone();
    fault.errors.push("joint 2 on hard max limit".into());
    assert!(
        matches!(compare_simulator(&r, &fault, 0.01), Err(SimError::Faults(m)) if m.contains("limit"))
    );

    let mut stop = good;
    stop.final_position[3] = 90.0;
    assert!(
        matches!(compare_simulator(&r, &stop, 0.01), Err(SimError::Mismatch(m)) if m.contains("stopped"))
    );
}

#[test]
fn req_gc_10_the_driver_report_format_is_read() {
    let json = r#"{"program":"x.ngc","rows":[{"line":3,"seconds":0.5,"t0":0.1,"t1":0.6,"first":[0,0,0,0],"end":[0,0,30,0]}],
        "errors":[],"final_position":[0,0,30,0],"g92_offset":[0,0,0,0],"samples":900}"#;
    let report = parse_sim_report(json).unwrap();
    assert_eq!(report.rows[0].line, 3);
    assert!(parse_sim_report("{").is_err());
    assert!(parse_sim_report(r#"{"rows":[]}"#).is_err());
}

#[test]
fn req_gc_10_a_line_that_ends_one_sample_short_is_accepted_if_the_next_line_starts_past_it() {
    let r = replayed(PROGRAM);
    let mut report = faithful(&r);
    // Line 5 (X5) ends 0.04 short of its target; the next line's first sample is past it.
    report.rows[2].end[0] = 4.96;
    report.rows[3].first = Some([5.01, 0.0, 10.0, 90.0]);
    compare_simulator(&r, &report, 0.005).unwrap();
    // Not past it: a real shortfall.
    let mut short = faithful(&r);
    short.rows[1].end[0] -= 0.5;
    short.rows[2].first = short.rows[2].first.map(|mut f| {
        f[0] -= 0.5;
        f
    });
    assert!(compare_simulator(&r, &short, 0.005).is_err());
}

/// A path as the driver would sample it: the nodes, with a few points between.
fn with_path(replay: &Replay) -> SimReport {
    let mut report = faithful(replay);
    let mut path = vec![replay.moves().next().unwrap().from];
    for m in replay.moves().filter(|m| m.from != m.to) {
        for k in 1..=4 {
            let t = f64::from(k) / 4.0;
            path.push(std::array::from_fn(|i| {
                m.from[i] + (m.to[i] - m.from[i]) * t
            }));
        }
    }
    report.path = path;
    report
}

#[test]
fn req_gc_10_the_sampled_path_must_visit_every_target_in_order() {
    let r = replayed(PROGRAM);
    let good = with_path(&r);
    compare_simulator(&r, &good, 0.05).unwrap();
    // A pause in sampling that hides a node the machine passed through is fine.
    let mut paused = good.clone();
    paused
        .path
        .retain(|p| (p[0] - 5.0).abs() > 0.2 || (p[3] - 90.0).abs() > 1.0);
    compare_simulator(&r, &paused, 0.05).unwrap();
    // A node the machine never reached: everything from X 5 on is half a millimetre off.
    let mut missing = good.clone();
    for p in &mut missing.path {
        if p[0] >= 4.9 {
            p[0] += 0.5;
        }
    }
    assert!(
        matches!(compare_simulator(&r, &missing, 0.05), Err(SimError::Mismatch(m)) if m.contains("never reached"))
    );
    // Visited, but in the wrong order.
    let mut reversed = good.clone();
    reversed.path.reverse();
    assert!(compare_simulator(&r, &reversed, 0.05).is_err());
    // A fault still fails first.
    let mut fault = good;
    fault.errors.push("joint 0 on hard max limit".into());
    assert!(matches!(
        compare_simulator(&r, &fault, 0.05),
        Err(SimError::Faults(_))
    ));
}
