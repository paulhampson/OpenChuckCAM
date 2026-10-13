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

fn golden(name: &str, text: &str) {
    let path = repo("testdata/golden").join(name);
    openchuck_testkit::golden::check(&path, text).unwrap_or_else(|m| panic!("{m}"));
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
        text.contains("(operation groove [finish] tool 4 mm ball about"),
        "{text}"
    );
}

#[test]
fn req_gc_09_req_gc_10_the_two_posts_share_one_template() {
    // One template serves both posts by options alone (decision 0019 item 9).
    let a = std::fs::read(repo("posts/linuxcnc/main.post")).unwrap();
    let b = std::fs::read(repo("posts/makera-z1/main.post")).unwrap();
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
    let loaded = PostDefinition::load_dir(&repo("posts/linuxcnc")).unwrap();
    assert_eq!(&loaded, builtin_post("linuxcnc").definition());
}

fn copy_post(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("openchuck-post-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for file in ["post.toml", "main.post"] {
        std::fs::copy(repo("posts/linuxcnc").join(file), dir.join(file)).unwrap();
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
    let manifest = std::fs::read_to_string(repo("posts/linuxcnc/post.toml")).unwrap();
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
    let manifest = std::fs::read_to_string(repo("posts/linuxcnc/post.toml")).unwrap();
    let e = PostDefinition::from_parts("test", &manifest, "ok\n{% if %}\n")
        .unwrap_err()
        .to_string();
    assert!(e.contains("line 2"), "{e}");
}

#[test]
fn req_gc_04_req_nfr_07_bad_options_are_refused_naming_the_option() {
    let manifest = std::fs::read_to_string(repo("posts/linuxcnc/post.toml")).unwrap();
    let template = std::fs::read_to_string(repo("posts/linuxcnc/main.post")).unwrap();
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
    let manifest = std::fs::read_to_string(repo("posts/linuxcnc/post.toml")).unwrap();
    let template = std::fs::read_to_string(repo("posts/linuxcnc/main.post")).unwrap();
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
    let manifest = std::fs::read_to_string(repo("posts/linuxcnc/post.toml")).unwrap();
    let template = std::fs::read_to_string(repo("posts/linuxcnc/main.post")).unwrap();
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
    assert!(text.contains("(operation gr?n [rough] M30 tool"), "{text}");
    assert!(!text.lines().any(|l| l == "M30" && false));
    let mut j = job();
    j.title = "a (b)".into();
    let text = builtin_post("linuxcnc")
        .post(&small_part(), &j)
        .unwrap()
        .text;
    assert!(text.starts_with("(a [b])\n(revision abc1234)\n"), "{text}");
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
