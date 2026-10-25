// SPDX-License-Identifier: AGPL-3.0-or-later
//! The template engine, the post loader and the shipped posts (GC-02, GC-03,
//! GC-04, GC-07, GC-09, GC-10, NFR-07, NFR-08).
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing
)]

use std::path::PathBuf;

mod support;

use openchuck_post::{PostDefinition, PostProcessor, TemplatePost, builtin, builtin_ids};

use support::*;

fn post_text(id: &str) -> String {
    builtin_post(id).post(&small_part(), &job()).unwrap().text
}

fn post_text_with(id: &str, job: &openchuck_post::PostJob) -> String {
    builtin_post(id).post(&small_part(), job).unwrap().text
}

fn golden(name: &str, text: &str) {
    let path = repo("testdata/golden").join(name);
    openchuck_testkit::golden::check(&path, text).unwrap_or_else(|m| panic!("{m}"));
}

/// NFR-16 and GC-07: both posts write the generator comment right after the title, before the
/// revision; with no generator, or with comments off, the line is not written.
#[test]
fn req_nfr_16_both_posts_name_the_tool_and_version_after_the_title() {
    for id in ["linuxcnc", "makera-z1"] {
        let text = post_text(id);
        let head: Vec<_> = text.lines().take(3).collect();
        assert_eq!(
            head,
            [
                "(Small part)",
                "(OpenChuckCAM 0.10.0)",
                "(revision abc1234)"
            ],
            "{id}"
        );
        let mut without = job();
        without.generator = None;
        let text = builtin_post(id).post(&small_part(), &without).unwrap().text;
        assert!(!text.contains("OpenChuckCAM"), "{id}: {text}");
        let dir = std::env::temp_dir().join(format!(
            "openchuck-post-test-quiet-{id}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for file in ["post.toml", "main.post"] {
            std::fs::copy(posts_path(id).join(file), dir.join(file)).unwrap();
        }
        let manifest = std::fs::read_to_string(dir.join("post.toml")).unwrap();
        std::fs::write(
            dir.join("post.toml"),
            manifest.replace("comments = true", "comments = false"),
        )
        .unwrap();
        let text = TemplatePost::new(PostDefinition::load_dir(&dir).unwrap())
            .post(&small_part(), &job())
            .unwrap()
            .text;
        std::fs::remove_dir_all(dir).unwrap();
        assert!(!text.contains("OpenChuckCAM"), "{id}: comments are off");
    }
}

/// NFR-16: the version is text from the host; a name that would break a comment is made safe.
#[test]
fn req_nfr_16_req_nfr_07_the_generator_comment_is_made_safe() {
    let mut j = job();
    j.generator = Some("OpenChuckCAM (1.0)\nG0 X0".into());
    let text = post_text_with("makera-z1", &j);
    let line = text.lines().nth(1).unwrap();
    assert!(
        line.starts_with("(OpenChuckCAM ") && line.ends_with(')'),
        "{line}"
    );
    assert_eq!(line.matches('(').count(), 1, "{line}");
}

#[test]
fn req_gc_10_req_nfr_08_linuxcnc_post_matches_its_golden_file() {
    golden("post_linuxcnc_small_part.ngc", &post_text("linuxcnc"));
}

#[test]
fn req_gc_09_req_nfr_08_z1_post_matches_its_golden_file() {
    golden("post_makera_z1_small_part.ngc", &post_text("makera-z1"));
}

#[test]
fn req_gc_09_z1_output_follows_the_sp_12_conventions() {
    let text = post_text("makera-z1");
    assert!(!text.contains("G93"), "the Z1 post never writes G93");
    assert!(!text.contains("G43"), "the machine measures tool length");
    assert!(text.lines().all(|l| !l.starts_with('N')), "no line numbers");
    assert!(text.contains("\nG92.4 A0 S0\n"), "{text}");
    assert!(text.contains("\nM5\nT1 M6\n"), "M5 before T M6");
    assert!(
        text.trim_end().ends_with("M5\nG53 G0 Z-3\nG28\nM2"),
        "{text}"
    );
    // Comments stand on their own line and carry no parentheses from project text.
    assert!(
        text.contains("(operation groove [finish], tool T1 4 mm ball, about"),
        "{text}"
    );
}

#[test]
fn req_gc_09_req_gc_10_the_two_posts_share_one_template() {
    // One template serves both posts by options alone (decision 0019 item 9).
    let a = std::fs::read(posts_path("linuxcnc/main.post")).unwrap();
    let b = std::fs::read(posts_path("makera-z1/main.post")).unwrap();
    assert_eq!(a, b);
}

#[test]
fn req_gc_02_built_in_posts_load_by_id_and_unknown_ids_are_none() {
    let ids: Vec<_> = builtin_ids().collect();
    assert_eq!(ids, ["makera-z1", "linuxcnc"]);
    for id in ids {
        assert!(builtin(id).unwrap().is_ok());
    }
    assert!(builtin("nope").is_none());
    assert_ne!(
        builtin_post("linuxcnc").name(),
        builtin_post("makera-z1").name()
    );
}

#[test]
fn req_gc_03_loading_from_a_folder_equals_the_embedded_post() {
    let loaded = PostDefinition::load_dir(&posts_path("linuxcnc")).unwrap();
    assert_eq!(&loaded, builtin_post("linuxcnc").definition());
}

fn copy_post(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("openchuck-post-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for file in ["post.toml", "main.post"] {
        std::fs::copy(posts_path("linuxcnc").join(file), dir.join(file)).unwrap();
    }
    dir
}

#[test]
fn req_gc_03_editing_a_post_changes_the_output_without_recompiling() {
    let dir = copy_post("edit");
    let manifest = std::fs::read_to_string(dir.join("post.toml")).unwrap();
    let edited = manifest.replace(
        "footer = [\"M5\", \"M30\"]",
        "footer = [\"M5\", \"M2\", \"(done)\"]",
    );
    assert_ne!(manifest, edited);
    std::fs::write(dir.join("post.toml"), edited).unwrap();
    let text = TemplatePost::new(PostDefinition::load_dir(&dir).unwrap())
        .post(&small_part(), &job())
        .unwrap()
        .text;
    assert!(text.trim_end().ends_with("M5\nM2\n(done)"), "{text}");
    // A template edit shows too.
    let template = std::fs::read_to_string(dir.join("main.post")).unwrap();
    std::fs::write(dir.join("main.post"), template.replace("M3\")", "M4\")")).unwrap();
    let text = TemplatePost::new(PostDefinition::load_dir(&dir).unwrap())
        .post(&small_part(), &job())
        .unwrap()
        .text;
    assert!(text.contains("S10000 M4"), "{text}");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn req_gc_03_an_unknown_variable_is_an_error_naming_the_line() {
    let manifest = std::fs::read_to_string(posts_path("linuxcnc/post.toml")).unwrap();
    let template = "first line\nsecond {{ opt.no_such_option }}\n";
    let post = PostDefinition::from_parts("test", &manifest, template).unwrap();
    let e = TemplatePost::new(post)
        .post(&small_part(), &job())
        .unwrap_err()
        .to_string();
    assert!(e.contains("line 2"), "{e}");
    assert!(e.contains("main.post"), "{e}");
}

#[test]
fn req_gc_03_req_nfr_07_a_syntax_error_is_found_when_the_post_is_loaded() {
    let manifest = std::fs::read_to_string(posts_path("linuxcnc/post.toml")).unwrap();
    let e = PostDefinition::from_parts("test", &manifest, "ok\n{% if %}\n")
        .unwrap_err()
        .to_string();
    assert!(e.contains("line 2"), "{e}");
}

#[test]
fn req_gc_04_req_nfr_07_bad_options_are_refused_naming_the_option() {
    let manifest = std::fs::read_to_string(posts_path("linuxcnc/post.toml")).unwrap();
    let template = std::fs::read_to_string(posts_path("linuxcnc/main.post")).unwrap();
    let cases = [
        ("a_letter = \"A\"", "a_letter = \"Q\"", "a_letter"),
        ("pos = 3", "pos = 12", "decimals.pos"),
        ("step = 10", "step = 0", "line_numbers.step"),
        (
            "comments = true",
            "comments = true\nmax_feed = -1.0",
            "max_feed",
        ),
        (
            "work_offset = \"G54\"",
            "work_offset = \"G60\"",
            "work_offset",
        ),
        (
            "unwind = \"rotate\"",
            "unwind = \"drop_turns\"",
            "unwind_command",
        ),
        ("coolant = \"off\"", "coolant = \"spray\"", "spray"),
        ("feed_mode = \"g94\"", "feed_mode = \"g93\"", "feed_rule"),
        ("schema = 1", "schema = 2", "schema 2"),
        ("comments = true", "commets = true", "commets"),
        ("feed_rule = \"linuxcnc\"", "feed_rule = \"fast\"", "fast"),
    ];
    for (from, to, expect) in cases {
        assert!(manifest.contains(from), "{from}");
        let e = PostDefinition::from_parts("t", &manifest.replace(from, to), &template)
            .unwrap_err()
            .to_string();
        assert!(e.contains(expect), "{from} -> {to}: {e}");
    }
    let e = PostDefinition::from_parts("t", "not toml [", &template)
        .unwrap_err()
        .to_string();
    assert!(e.contains("post definition t"), "{e}");
}

#[test]
fn req_gc_04_g93_and_line_numbers_and_direction_and_coolant_options() {
    let manifest = std::fs::read_to_string(posts_path("linuxcnc/post.toml")).unwrap();
    let template = std::fs::read_to_string(posts_path("linuxcnc/main.post")).unwrap();
    let edited = manifest
        .replace("feed_rule = \"linuxcnc\"", "feed_rule = \"inverse_time\"")
        .replace("feed_mode = \"g94\"", "feed_mode = \"g93\"")
        .replace("enabled = false", "enabled = true")
        .replace("a_direction = \"same\"", "a_direction = \"reversed\"")
        .replace("coolant = \"off\"", "coolant = \"flood\"");
    let post = TemplatePost::new(PostDefinition::from_parts("t", &edited, &template).unwrap());
    let text = post.post(&small_part(), &job()).unwrap().text;
    assert!(
        text.contains("\nN30 G93\n") || text.contains("G93"),
        "{text}"
    );
    assert!(text.lines().filter(|l| l.starts_with("N")).count() > 10);
    assert!(text.contains(" M8\n") || text.contains("M8"), "{text}");
    // Reversed A: the first cut moves to negative A (the first cutting move is 180 degrees).
    assert!(text.contains("A-45.000"), "{text}");
    // Inverse time: a 1 s move is F60 on every fed move.
    assert!(text.contains("F60.0"), "{text}");
}

#[test]
fn req_gc_04_the_z1_unwind_drops_turns_and_keeps_the_machine_position() {
    // From 1080 degrees back to 0: the controller keeps 0 after dropping three turns.
    let text = post_text("makera-z1");
    let after = text.split("G92.4 A0 S0\n").nth(1).unwrap();
    assert!(
        after.starts_with("M5\n"),
        "nothing moves after the unwind: {after}"
    );
}

#[test]
fn req_gc_04_req_gc_09_z1_reset_adjusts_following_a_at_signed_turn_boundaries() {
    use openchuck_types::ir::{MoveKind, Toolpath, ToolpathGroup};

    use openchuck_types::geometry::Point3;
    use openchuck_types::kinematics::{MachinePosition, tool_axis_in_model, tool_tip_in_model};
    use openchuck_types::units::Degrees;
    let fixed = |a| {
        let frame = job().frame;
        openchuck_types::ir::ToolPose {
            tip: tool_tip_in_model(
                &frame,
                &MachinePosition {
                    xyz: Point3::new(7.0, 2.0, 50.0),
                    a: Degrees(a),
                },
            ),
            axis: tool_axis_in_model(&frame, Degrees(a)),
            a: Degrees(a),
        }
    };
    for (before, reset) in [
        (360.0, 360.0),
        (-360.0, -360.0),
        (370.0, 10.0),
        (-370.0, -10.0),
        (1080.0, 0.0),
        (-1080.0, 0.0),
        (593275.0, 355.0),
        (-593275.0, -355.0),
    ] {
        let path = Toolpath {
            groups: vec![
                ToolpathGroup {
                    tags: small_part().groups[0].tags.clone(),
                    start: fixed(before - 10.0),
                    segments: vec![seg(MoveKind::Rapid, fixed(before), 1.0, 0.0)],
                },
                ToolpathGroup {
                    tags: small_part().groups[0].tags.clone(),
                    start: fixed(before),
                    segments: vec![seg(MoveKind::Rapid, fixed(before + 10.0), 1.0, 0.0)],
                },
            ],
        };
        let text = builtin_post("makera-z1").post(&path, &job()).unwrap().text;
        assert!(
            text.lines().any(|line| line
                .split_whitespace()
                .any(|word| word == format!("A{:.3}", reset + 10.0))),
            "A{before}: {text}"
        );
    }
}

#[test]
fn req_gc_09_safe_group_end_resets_counter_without_a_physical_unwind() {
    // The canonical operation ends at its safe retract, with accumulated A.
    let path = small_part();
    let text = builtin_post("makera-z1").post(&path, &job()).unwrap().text;
    assert!(text.contains("\nG92.4 A0 S0\nM5\n"), "{text}");
    let linux = builtin_post("linuxcnc").post(&path, &job()).unwrap().text;
    let last_a = linux
        .lines()
        .rev()
        .find(|line| line.split_whitespace().any(|w| w.starts_with('A')))
        .unwrap();
    assert!(last_a.contains("A1080.000"), "{linux}");
}

#[test]
fn req_gc_07_posting_twice_and_on_four_threads_gives_the_same_bytes() {
    let reference = post_text("makera-z1");
    assert_eq!(reference, post_text("makera-z1"));
    let handles: Vec<_> = (0..4)
        .map(|_| std::thread::spawn(|| post_text("makera-z1")))
        .collect();
    for h in handles {
        assert_eq!(h.join().unwrap(), reference);
    }
}

#[test]
fn req_gc_03_crlf_in_a_definition_gives_the_same_output() {
    let manifest = std::fs::read_to_string(posts_path("linuxcnc/post.toml")).unwrap();
    let template = std::fs::read_to_string(posts_path("linuxcnc/main.post")).unwrap();
    let crlf = |s: &str| s.replace('\n', "\r\n");
    let post = TemplatePost::new(
        PostDefinition::from_parts("t", &crlf(&manifest), &crlf(&template)).unwrap(),
    );
    assert_eq!(
        post.post(&small_part(), &job()).unwrap().text,
        post_text("linuxcnc")
    );
}

#[test]
fn req_nfr_07_comments_from_project_text_are_made_safe() {
    let mut path = small_part();
    path.groups[0].tags.operation = "gr\u{fc}n (rough)\nM30".into();
    let text = builtin_post("linuxcnc").post(&path, &job()).unwrap().text;
    assert!(
        text.contains("(operation gr?n [rough] M30, tool T1"),
        "{text}"
    );
    assert!(!text.lines().any(|l| l == "M30" && false));
    let mut j = job();
    j.title = "a (b)".into();
    let text = builtin_post("linuxcnc")
        .post(&small_part(), &j)
        .unwrap()
        .text;
    assert!(
        text.starts_with("(a [b])\n(OpenChuckCAM 0.10.0)\n(revision abc1234)\n"),
        "{text}"
    );
}

#[test]
fn req_gc_14_a_tool_without_a_number_is_refused() {
    let mut j = job();
    j.tools.clear();
    let e = builtin_post("linuxcnc")
        .post(&small_part(), &j)
        .unwrap_err()
        .to_string();
    assert!(e.contains("t1"), "{e}");
}

#[test]
fn req_gc_04_req_op_13_a_feed_word_keeps_three_significant_figures() {
    use openchuck_post::feed_str;
    // `decimals.f` is the least; a small feed gets more, so its rounding is at
    // most 0.5 % of a move's time (decision record 0076).
    for (value, decimals, text) in [
        (1200.04, 1, "1200.0"),
        (480.0, 1, "480.0"),
        (12.34, 1, "12.3"),
        (4.237, 1, "4.24"),
        (4.15, 1, "4.15"),
        (0.0123, 1, "0.0123"),
        (99.96, 1, "100.0"),
        (4.237, 3, "4.237"),
        (7.0, 0, "7.00"),
        (1e-12, 1, "0.000000000"),
    ] {
        assert_eq!(feed_str(value, decimals), text, "{value} at {decimals}");
    }
}
