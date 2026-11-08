// SPDX-License-Identifier: AGPL-3.0-or-later
//! Rotary roughing on the Baluster example goes down along the tool axis no
//! faster than its plunge feed, in the path and in the LinuxCNC program, and
//! writes the same programs at 1, 2 and 4 threads (OP-11, OP-03, GC-07,
//! NFR-06; decision record 0548, issue #643).
//!
//! The example's plunge feed is lowered to 100 mm/min so that some of its
//! drops off the raised floors would otherwise go down faster, and its
//! Stepdown and Stepover are raised to 2 and 3 mm so the tests stay short.
//!
//! The G-code these tests produce is not checked on a machine: generated
//! G-code is the user's to verify before it is run (NFR-12).
#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::float_cmp,
    clippy::panic
)]

use std::path::{Path, PathBuf};

use openchuck_app::{Application, OutputGrouping, PostSelection, Regeneration, Threads};
use openchuck_types::ir::{MoveKind, ToolPose};

const ROUGH: &str = "1a280b59-8da1-4ee1-97e9-1e2b40c356f5";
const FINISH: &str = "34e3aa5e-36cc-4614-be6d-ec39cf22f5f8";
const PLUNGE: f64 = 100.0;

/// A copy of the Baluster example with only its Rough operation, at the
/// lowered plunge feed, in a folder of its own.
fn project(name: &str) -> PathBuf {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let unique = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "openchuck-descent-feed-{name}-{unique}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    copy(
        &Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../testdata/examples/baluster/baluster-project"),
        &dir,
    );
    let rough = dir.join(format!("operations/{ROUGH}.toml"));
    let text = std::fs::read_to_string(&rough).unwrap();
    assert!(
        text.contains("plunge_feed = 300.0\n"),
        "the example has changed"
    );
    assert!(text.contains("stepdown = 1.0\n"), "the example has changed");
    std::fs::write(
        &rough,
        text.replace(
            "plunge_feed = 300.0\n",
            &format!("plunge_feed = {PLUNGE:.1}\n"),
        )
        .replace("stepdown = 1.0\n", "stepdown = 2.0\n")
        .replace("stepover = 2.2225\n", "stepover = 3.0\n"),
    )
    .unwrap();
    let finish = dir.join(format!("operations/{FINISH}.toml"));
    let text = std::fs::read_to_string(&finish).unwrap();
    std::fs::write(
        &finish,
        text.replacen("\n[parameters]", "enabled = false\n\n[parameters]", 1),
    )
    .unwrap();
    dir
}

fn copy(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

fn post(dir: &Path, post: &str, threads: usize) -> Regeneration {
    let mut app = Application::open(dir, Threads::Exactly(threads)).unwrap();
    app.start_regenerate_with_post(PostSelection::Builtin(post.into()), OutputGrouping::Setup)
        .wait()
        .unwrap_or_else(|e| panic!("{post}: {e}"))
}

/// The tip's height along the tool axis: the example's tool axis passes
/// through the rotary axis.
fn height(p: &ToolPose) -> f64 {
    let a = p.axis.get();
    p.tip.x * a.x + p.tip.y * a.y + p.tip.z * a.z
}

/// The fastest descent along the tool axis, in mm/min, of the cutting moves
/// in the path.
fn fastest_cutting_descent(run: &Regeneration) -> f64 {
    let mut fastest: f64 = 0.0;
    for group in &run.toolpath.groups {
        let mut from = group.start;
        for s in &group.segments {
            if s.kind == MoveKind::Cutting {
                fastest = fastest.max(60.0 * (height(&from) - height(&s.end)) / s.duration.0);
            }
            from = s.end;
        }
    }
    fastest
}

/// The fastest descent along Z of the fed moves in a LinuxCNC program, in
/// mm/min: LinuxCNC applies F to the X, Y and Z line of a move that has one.
fn fastest_written_descent(program: &str) -> f64 {
    let (mut at, mut feed, mut rapid) = ([0.0f64; 3], 0.0f64, true);
    let mut fastest: f64 = 0.0;
    for line in program.lines() {
        let code = line.split(['(', ';']).next().unwrap_or("");
        let mut next = at;
        for word in code.split_whitespace() {
            let (letter, value) = word.split_at(1);
            let Ok(v) = value.parse::<f64>() else {
                continue;
            };
            match letter {
                "G" if v == 0.0 => rapid = true,
                "G" if v == 1.0 => rapid = false,
                "X" => next[0] = v,
                "Y" => next[1] = v,
                "Z" => next[2] = v,
                "F" => feed = v,
                _ => {}
            }
        }
        let d = [next[0] - at[0], next[1] - at[1], next[2] - at[2]];
        let line_length = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        if !rapid && d[2] < 0.0 && line_length > 0.0 {
            fastest = fastest.max(feed * -d[2] / line_length);
        }
        at = next;
    }
    fastest
}

/// Decision 0548 on the Baluster: no cutting move of the path, and no fed
/// move of the LinuxCNC program, goes down faster than the plunge feed, read
/// from the written words themselves to the F word's written resolution of
/// 0.1 mm/min (the assembly allows for rounding the heights to three
/// decimals; the F word is rounded to one, which on `main` already writes
/// an entry plunge at this plunge feed as `F100.1`).
#[test]
fn req_op_11_req_op_03_baluster_roughing_goes_down_no_faster_than_the_plunge_feed() {
    let run = post(&project("rate"), "linuxcnc", 2);
    let path = fastest_cutting_descent(&run);
    assert!(
        path <= PLUNGE * (1.0 + 1e-9),
        "a cutting move goes down at {path:.3} mm/min"
    );
    assert_eq!(run.gcode.len(), 1);
    let written = fastest_written_descent(&run.gcode[0].1);
    assert!(
        written <= PLUNGE + 0.1,
        "the program goes down at {written:.3} mm/min"
    );
    let slowed = run
        .toolpath
        .groups
        .iter()
        .flat_map(|g| &g.segments)
        .filter(|s| s.kind == MoveKind::Cutting && s.contact_feed.is_some_and(|f| f.0 < 1000.0))
        .count();
    assert!(slowed > 0, "the case must have a drop that is slowed");
}

/// The same programs at 1, 2 and 4 threads, with both posts (GC-07, NFR-06).
#[test]
fn req_gc_07_req_nfr_06_descent_feed_is_deterministic_at_1_2_n_threads() {
    let dir = project("threads");
    for name in ["linuxcnc", "makera-z1"] {
        let one = post(&dir, name, 1).gcode;
        for threads in [2, 4] {
            assert_eq!(
                post(&dir, name, threads).gcode,
                one,
                "{name} at {threads} threads"
            );
        }
    }
}
