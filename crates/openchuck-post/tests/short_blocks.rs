// SPDX-License-Identifier: AGPL-3.0-or-later
//! GC-17: measured command throughput, reported or met without changing geometry.
#![allow(clippy::unwrap_used, clippy::float_cmp, clippy::indexing_slicing)]
mod support;
use openchuck_post::{PostProcessor, ShortBlockSettings};
use openchuck_types::ir::{MoveKind, Toolpath};
use openchuck_types::machine::ShortBlockMode;

fn path(times: &[f64]) -> Toolpath {
    let mut p = support::small_part();
    p.groups[0].start = support::pose(0.0, 0.0, 10.0, 0.0);
    p.groups[0].segments = times
        .iter()
        .enumerate()
        .map(|(i, t)| {
            support::seg(
                MoveKind::Cutting,
                support::pose((i + 1) as f64 * 0.1, 0.0, 10.0, 0.0),
                *t,
                0.1,
            )
        })
        .collect();
    p
}
fn settings(mode: ShortBlockMode, window: usize) -> ShortBlockSettings {
    ShortBlockSettings {
        min_block_ms: 7.0,
        block_window: window,
        mode,
    }
}
#[test]
fn req_gc_17_off_and_report_leave_program_unchanged() {
    let p = path(&[0.002, 0.004, 0.012, 0.003]);
    let mut job = support::job();
    let post = support::builtin_post("makera-z1");
    let off = post.post(&p, &job).unwrap();
    job.short_blocks = settings(ShortBlockMode::Report, 2);
    let report = post.post(&p, &job).unwrap();
    assert_eq!(off.text, report.text);
    assert_eq!(off.estimated_seconds, report.estimated_seconds);
    let r = &report.short_blocks[0];
    assert_eq!(r.short_moves, 3);
    assert_eq!(r.longest_run, 2);
    assert_eq!(r.affected_windows, 1);
    assert!((r.worst_window_ms - 3.0).abs() < 0.01);
    assert!(r.added_seconds >= 0.008 - 1e-9);
}
#[test]
fn req_gc_17_slow_down_preserves_geometry_and_meets_overlapping_windows() {
    let p = path(&[0.001, 0.002, 0.003, 0.004, 0.005]);
    let mut job = support::job();
    job.short_blocks = settings(ShortBlockMode::SlowDown, 3);
    let result = support::builtin_post("makera-z1").post(&p, &job).unwrap();
    let adjusted = &result.verification_path.groups[0].segments;
    for (a, b) in p.groups[0].segments.iter().zip(adjusted) {
        assert_eq!(a.end, b.end);
        assert_eq!(a.kind, b.kind);
        assert!(b.duration.0 >= a.duration.0);
    }
    for window in adjusted.windows(3) {
        assert!(window.iter().map(|s| s.duration.0).sum::<f64>() >= 0.021 - 1e-12);
    }
    let added = adjusted
        .iter()
        .zip(&p.groups[0].segments)
        .map(|(a, b)| a.duration.0 - b.duration.0)
        .sum::<f64>();
    assert!((added - result.short_blocks[0].added_seconds).abs() < 1e-9);
    assert!((result.estimated_seconds - p.total_duration().0 - added).abs() < 1e-9);
}
#[test]
fn req_gc_17_window_absorbs_short_bursts_but_short_runs_are_checked() {
    let mut job = support::job();
    job.short_blocks = settings(ShortBlockMode::SlowDown, 4);
    let post = support::builtin_post("makera-z1");
    let p = path(&[0.001, 0.001, 0.020, 0.020]);
    let result = post.post(&p, &job).unwrap();
    assert_eq!(result.short_blocks[0].slowed_moves, 0);
    let result = post.post(&path(&[0.001, 0.001]), &job).unwrap();
    assert_eq!(result.short_blocks[0].slowed_moves, 2);
    assert!(
        result.verification_path.groups[0]
            .segments
            .iter()
            .all(|s| s.duration.0 >= 0.007 - 1e-12)
    );
}
#[test]
fn req_gc_17_post_overrides_profile_and_rejects_invalid_settings() {
    let mut d = openchuck_post::builtin("makera-z1").unwrap().unwrap();
    d.options.min_block_ms = Some(10.0);
    d.options.block_window = Some(1);
    d.options.short_block_mode = Some(ShortBlockMode::SlowDown);
    let result = openchuck_post::TemplatePost::new(d.clone())
        .post(&path(&[0.002]), &support::job())
        .unwrap();
    assert_eq!(result.short_blocks[0].settings.min_block_ms, 10.0);
    assert!(result.verification_path.groups[0].segments[0].duration.0 >= 0.010 - 1e-12);
    d.options.block_window = Some(0);
    assert!(d.options.validate().is_err());
    d.options.block_window = Some(1);
    d.options.min_block_ms = Some(f64::NAN);
    assert!(d.options.validate().is_err());
}

#[test]
fn req_gc_17_rapid_boundaries_do_not_hide_short_runs() {
    let mut p = path(&[0.001, 0.030, 0.001]);
    p.groups[0].segments[1].kind = MoveKind::Rapid;
    p.groups[0].segments[1].contact_feed = None;
    let mut job = support::job();
    job.short_blocks = settings(ShortBlockMode::SlowDown, 32);
    let result = support::builtin_post("makera-z1").post(&p, &job).unwrap();
    assert_eq!(result.short_blocks[0].slowed_moves, 2);
    assert_eq!(result.short_blocks[0].longest_run, 1);
    assert_eq!(
        result.verification_path.groups[0].segments[1],
        p.groups[0].segments[1]
    );
}
#[test]
fn req_gc_17_mixed_rotary_and_inverse_time_use_rendered_feeds() {
    for inverse in [false, true] {
        let mut d = openchuck_post::builtin("makera-z1").unwrap().unwrap();
        if inverse {
            d.feed_rule = openchuck_post::FeedRule::InverseTime;
            d.options.feed_mode = openchuck_post::FeedMode::G93;
        }
        let post = openchuck_post::TemplatePost::new(d);
        let mut p = path(&[0.001, 0.002, 0.003]);
        for (i, s) in p.groups[0].segments.iter_mut().enumerate() {
            s.end.a.0 = (i + 1) as f64 * 0.1;
        }
        let mut job = support::job();
        job.short_blocks = settings(ShortBlockMode::SlowDown, 1);
        let result = post.post(&p, &job).unwrap();
        for s in &result.verification_path.groups[0].segments {
            assert!(s.duration.0 >= 0.007 - 1e-12);
        }
        assert_eq!(result.short_blocks[0].slowed_moves, 3);
        assert_eq!(
            result,
            post.post(&p, &job).unwrap(),
            "deterministic repeated render"
        );
    }
}
#[test]
fn req_gc_17_max_feed_is_included_in_demand() {
    let mut d = openchuck_post::builtin("makera-z1").unwrap().unwrap();
    // 0.1 mm at F60 is a 100ms block despite the original 1ms duration.
    d.options.max_feed = Some(60.0);
    let mut job = support::job();
    job.short_blocks = settings(ShortBlockMode::Report, 1);
    let result = openchuck_post::TemplatePost::new(d)
        .post(&path(&[0.001]), &job)
        .unwrap();
    assert_eq!(result.short_blocks[0].short_moves, 0);
    assert_eq!(result.short_blocks[0].added_seconds, 0.0);
    assert!((result.short_blocks[0].worst_window_ms - 100.0).abs() < 0.001);
}
#[test]
fn req_gc_17_contact_requires_controller_rule_but_off_is_compatible() {
    let mut d = openchuck_post::builtin("linuxcnc").unwrap().unwrap();
    d.feed_rule = openchuck_post::FeedRule::Contact;
    let post = openchuck_post::TemplatePost::new(d);
    let mut job = support::job();
    let p = path(&[0.001]);
    let baseline = post.post(&p, &job).unwrap();
    job.short_blocks = settings(ShortBlockMode::Off, 1);
    assert_eq!(post.post(&p, &job).unwrap().text, baseline.text);
    for mode in [ShortBlockMode::Report, ShortBlockMode::SlowDown] {
        job.short_blocks = settings(mode, 1);
        let error = post.post(&p, &job).unwrap_err();
        assert!(error.to_string().contains("controller timing model"));
    }
}
#[test]
fn req_gc_17_invalid_and_unrepresentable_limits_return_errors() {
    let post = support::builtin_post("makera-z1");
    for (ms, n) in [
        (f64::NAN, 1),
        (-1.0, 1),
        (7.0, 0),
        (7.0, 4097),
        (f64::MAX, 4096),
        (1e100, 1),
    ] {
        let mut job = support::job();
        job.short_blocks = ShortBlockSettings {
            min_block_ms: ms,
            block_window: n,
            mode: ShortBlockMode::SlowDown,
        };
        assert!(post.post(&path(&[0.001]), &job).is_err(), "{ms}, {n}");
    }
}

#[test]
fn req_gc_17_req_gc_07_same_output_at_one_and_many_threads() {
    let p = path(&[0.0001, 0.002, 0.003, 0.004, 0.005, 0.001, 0.030]);
    let mut job = support::job();
    job.short_blocks = settings(ShortBlockMode::SlowDown, 3);
    let post = support::builtin_post("makera-z1");
    let expected = post.post(&p, &job).unwrap();
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..4)
            .map(|_| scope.spawn(|| post.post(&p, &job).unwrap()))
            .collect();
        for handle in handles {
            assert_eq!(handle.join().unwrap(), expected);
        }
    });
}

// A deliberately independent reader of the emitted modal words. No production
// feed conversion or adjusted IR timing is used to establish controller time.
fn emitted_times(text: &str, inverse: bool) -> Vec<f64> {
    let mut position = [0.0_f64; 4];
    let mut feed = 0.0;
    let mut fed = false;
    let mut times = Vec::new();
    for line in text.lines().filter(|line| !line.starts_with('(')) {
        let before = position;
        let mut motion = false;
        for word in line.split_whitespace() {
            match word {
                "G0" => fed = false,
                "G1" => fed = true,
                _ => {}
            }
            if let Some(value) = word.strip_prefix('F') {
                feed = value.parse().unwrap();
            }
            for (i, letter) in ['X', 'Y', 'Z', 'A'].iter().enumerate() {
                if let Some(value) = word.strip_prefix(*letter) {
                    position[i] = value.parse().unwrap();
                    motion = true;
                }
            }
        }
        if fed && motion {
            let xyz = ((position[0] - before[0]).powi(2)
                + (position[1] - before[1]).powi(2)
                + (position[2] - before[2]).powi(2))
            .sqrt();
            let a = (position[3] - before[3]).abs();
            let p = 2.0
                * std::f64::consts::PI
                * (position[1].powi(2) + position[2].powi(2)).sqrt().max(1.0)
                + 30.0;
            let amount = if xyz > 1e-9 {
                xyz.max(a * p / 360.0)
            } else {
                a * p.min(360.0) / 360.0
            };
            times.push(if inverse {
                60.0 / feed
            } else {
                amount * 60.0 / feed
            });
        }
    }
    times
}
#[test]
fn req_gc_17_rendered_feeds_meet_window_limit_at_rounding_boundaries_and_a_only() {
    for inverse in [false, true] {
        for a_only_radius in [None, Some(10.0_f64), Some(100.0_f64)] {
            let mut d = openchuck_post::builtin("makera-z1").unwrap().unwrap();
            d.options.decimals.f = 1;
            if inverse {
                d.feed_rule = openchuck_post::FeedRule::InverseTime;
                d.options.feed_mode = openchuck_post::FeedMode::G93;
            }
            let post = openchuck_post::TemplatePost::new(d);
            let mut p = path(&[0.001, 0.002, 0.003, 0.004, 0.005]);
            for (i, s) in p.groups[0].segments.iter_mut().enumerate() {
                s.end.a.0 = (i + 1) as f64 * 0.057;
                if let Some(radius) = a_only_radius {
                    // Counter-rotate the model-space tip so the written XYZ
                    // stays fixed while A changes, below and above the P cap.
                    let angle = s.end.a.0.to_radians();
                    s.end.tip = openchuck_types::geometry::Point3::new(
                        0.0,
                        radius * angle.sin(),
                        radius * angle.cos(),
                    );
                }
            }
            if let Some(radius) = a_only_radius {
                p.groups[0].start.tip.z = radius;
            }
            let mut job = support::job();
            job.short_blocks = ShortBlockSettings {
                min_block_ms: 7.001,
                block_window: 3,
                mode: ShortBlockMode::SlowDown,
            };
            let result = post.post(&p, &job).unwrap();
            let times = emitted_times(&result.text, inverse);
            assert_eq!(times.len(), 5, "{}", result.text);
            for window in times.windows(3) {
                assert!(window.iter().sum::<f64>() >= 3.0 * 0.007001, "{times:?}");
            }
            for (time, segment) in times
                .iter()
                .zip(&result.verification_path.groups[0].segments)
            {
                assert!((time - segment.duration.0).abs() < 1e-12);
            }
        }
    }
}

#[test]
fn req_gc_17_sources_are_visible_even_when_post_disables_checking() {
    let mut d = openchuck_post::builtin("makera-z1").unwrap().unwrap();
    d.options.min_block_ms = Some(0.0);
    let (effective, sources) = d
        .options
        .effective_short_blocks(settings(ShortBlockMode::Report, 32));
    assert_eq!(effective.min_block_ms, 0.0);
    assert_eq!(effective.block_window, 32);
    assert_eq!(sources.min_block_ms.as_str(), "post override");
    assert_eq!(sources.block_window.as_str(), "machine profile");
    assert_eq!(sources.mode.as_str(), "machine profile");
    let mut job = support::job();
    job.short_blocks = settings(ShortBlockMode::Report, 32);
    assert!(
        openchuck_post::TemplatePost::new(d)
            .post(&path(&[0.001]), &job)
            .unwrap()
            .short_blocks
            .is_empty()
    );
}
#[test]
fn req_gc_17_rounded_away_moves_do_not_consume_a_command_window() {
    let mut p = path(&[0.001, 0.001]);
    p.groups[0].segments[0].end.tip.x = 0.0001;
    let mut job = support::job();
    job.short_blocks = settings(ShortBlockMode::SlowDown, 1);
    let result = support::builtin_post("makera-z1").post(&p, &job).unwrap();
    assert_eq!(result.short_blocks[0].short_moves, 1);
    assert_eq!(
        result.verification_path.groups[0].segments[0].duration.0,
        0.0
    );
    p.groups[0].segments.truncate(1);
    let result = support::builtin_post("makera-z1").post(&p, &job).unwrap();
    assert_eq!(result.short_blocks[0].worst_window_ms, 0.0);
    assert_eq!(result.short_blocks[0].short_moves, 0);
}
#[test]
fn req_gc_17_neutral_conversion_checks_unrounded_block_durations() {
    let job = support::job();
    let options = openchuck_post::ConvertOptions {
        short_blocks: settings(ShortBlockMode::SlowDown, 1),
        feed_decimals: None,
        a_range: job.a_range,
        safe_z: job.safe_z,
        frame: job.frame,
        feed_rule: openchuck_post::FeedRule::Makera,
        max_feed: None,
        tools: job.tools,
        decimals: None,
        operation_names: job.operation_names,
        operation_spindle: job.operation_spindle,
    };
    let p = path(&[0.001]);
    let program = openchuck_post::to_machine_program(&p, &options).unwrap();
    assert!(program.verification_path.groups[0].segments[0].duration.0 >= 0.007);
}
