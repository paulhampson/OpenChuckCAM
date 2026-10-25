// SPDX-License-Identifier: AGPL-3.0-or-later
//! Requirement-first checks for accepted decision 0222 (GC-17).
#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod support;
use openchuck_post::{BlockTimeSettings, PostProcessor, block_time_reports};
use openchuck_types::machine::ShortBlockMode;
use openchuck_types::units::{Millimetres, Seconds};
use support::{builtin_post, job, small_part};

fn settings(window: u32) -> BlockTimeSettings {
    BlockTimeSettings {
        min_block_ms: 7.0,
        block_window: window,
        mode: ShortBlockMode::Report,
    }
}
fn path(times: &[f64]) -> openchuck_types::ir::Toolpath {
    let mut p = small_part();
    let fed = *p.groups[0]
        .segments
        .iter()
        .find(|s| s.kind != openchuck_types::ir::MoveKind::Rapid)
        .unwrap();
    p.groups[0].segments = times
        .iter()
        .map(|ms| {
            let mut s = fed;
            s.duration = Seconds(ms / 1000.0);
            s.contact_length = Millimetres(1.0);
            s
        })
        .collect();
    p
}
#[test]
fn req_gc_17_report_counts_runs_and_windows() {
    let r = block_time_reports(&path(&[2.0, 2.0, 20.0, 2.0, 2.0]), settings(2)).unwrap();
    assert_eq!(r[0].affected_blocks, 4);
    assert_eq!(r[0].longest_run, 2);
    assert!((r[0].worst_mean_ms - 2.0).abs() < 1e-9);
    assert!((r[0].effective_feed - 30000.0).abs() < 1e-6);
    assert!((r[0].seconds_added - 0.020).abs() < 1e-9);
    // A full queue absorbs the isolated 2 ms block.
    assert_eq!(
        block_time_reports(&path(&[20.0, 2.0, 20.0]), settings(3)).unwrap()[0].affected_blocks,
        0
    );
}
#[test]
fn req_gc_17_off_leaves_output_unchanged_and_report_changes_no_gcode() {
    for id in ["makera-z1", "linuxcnc"] {
        let post = builtin_post(id);
        let mut j = job();
        let base = post.post(&small_part(), &j).unwrap();
        j.block_time = settings(1);
        let report = post.post(&small_part(), &j).unwrap();
        assert_eq!(base.text, report.text);
        assert!(!report.block_time_reports.is_empty());
        j.block_time.min_block_ms = 0.0;
        let off = post.post(&small_part(), &j).unwrap();
        assert_eq!(base.text, off.text);
        assert!(off.block_time_reports.is_empty());
    }
}
#[test]
fn req_gc_17_post_overrides_profile() {
    let mut d = openchuck_post::builtin("makera-z1").unwrap().unwrap();
    d.options.min_block_ms = Some(0.0);
    let post = openchuck_post::TemplatePost::new(d);
    let mut j = job();
    j.block_time = settings(32);
    assert!(
        post.post(&small_part(), &j)
            .unwrap()
            .block_time_reports
            .is_empty()
    );
}
#[test]
fn req_gc_17_deterministic_at_any_thread_count() {
    let p = path(&[2.0; 64]);
    let expected = block_time_reports(&p, settings(32)).unwrap();
    std::thread::scope(|scope| {
        for _ in 0..4 {
            let p = &p;
            let e = &expected;
            scope.spawn(move || assert_eq!(&block_time_reports(p, settings(32)).unwrap(), e));
        }
    });
}
#[test]
fn req_gc_17_req_nfr_07_invalid_settings_are_refused() {
    let p = path(&[2.0; 32]);
    let mut s = settings(0);
    assert!(block_time_reports(&p, s).is_err());
    s = settings(1);
    s.min_block_ms = f64::NAN;
    assert!(block_time_reports(&p, s).is_err());
    s.min_block_ms = -1.0;
    assert!(block_time_reports(&p, s).is_err());
}

#[test]
fn req_gc_17_rapids_reset_the_window_and_incomplete_windows_are_not_per_block_checks() {
    let mut p = path(&[2.0; 5]);
    p.groups[0].segments[2].kind = openchuck_types::ir::MoveKind::Rapid;
    p.groups[0].segments[2].contact_feed = None;
    let r = block_time_reports(&p, settings(3)).unwrap();
    assert_eq!(r[0].windows, 0);
    assert_eq!(r[0].affected_blocks, 0);
    assert!(r[0].summary("profile").contains("no complete window"));
}

#[test]
fn req_gc_17_effective_feed_is_for_the_longest_affected_run() {
    let report = block_time_reports(&path(&[1.0, 9.0, 1.0]), settings(2)).unwrap();
    assert_eq!(report[0].longest_run, 3);
    assert!((report[0].effective_feed - 180.0 / 0.011).abs() < 1e-6);
}

#[test]
fn req_gc_17_req_nfr_12_the_operator_report_names_the_source_and_measured_quantities() {
    let report = block_time_reports(&path(&[2.0, 2.0, 20.0, 2.0, 2.0]), settings(2)).unwrap();
    let text = report[0].summary("post");
    for required in [
        "7.000 ms (post)",
        "window 2",
        "4 affected blocks",
        "longest run 2",
        "worst mean 2.000 ms",
        "30000.000 mm/min",
        "adds 0.020 s",
        "G-code unchanged",
        "verify before running",
    ] {
        assert!(text.contains(required), "missing {required}: {text}");
    }
}
