// SPDX-License-Identifier: AGPL-3.0-or-later
//! The tutorial's turned piece (the public baluster example) finished in
//! Rings with Lead entry and no grip at its end: the lead comes down onto
//! the first ring in the plane of the cut at the ring's X, A and Z moving
//! together, and leaves the last ring the same way (decision 0544, accepted;
//! issue #640; OP-11, OP-15). A public stand-in for the owner check of #640.
//! With the example's 8 mm grip the pass starts today with a short step
//! along X before its first ring (helical finishing's rings cut short at
//! the held region), and gets the line lead; the owner decided that those
//! rings start on a whole ring at the held region's edge (lane RINGSTEP),
//! after which the grip case gets this lead too.
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::path::{Path, PathBuf};

use openchuck_app::{Application, Threads};
use openchuck_types::ir::{MoveKind, Segment};

const ROUGH: &str = "operations/1a280b59-8da1-4ee1-97e9-1e2b40c356f5.toml";
const FINISH: &str = "operations/34e3aa5e-36cc-4614-be6d-ec39cf22f5f8.toml";
const SETUP: &str = "setups/57ad6798-2544-4dfb-90a0-caf67ec69484.toml";

/// A copy of the example with only its finishing operation, in Rings with
/// Lead entry, and no grip at the start of the stock.
fn example() -> PathBuf {
    let from = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/examples/baluster/baluster-project");
    let dir = std::env::temp_dir().join(format!("openchuck-lead-example-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    copy(&from, &dir);
    std::fs::remove_file(dir.join(ROUGH)).unwrap();
    let path = dir.join(FINISH);
    let text = std::fs::read_to_string(&path).unwrap();
    let mut out = String::new();
    for line in text.lines() {
        if ["entry =", "pass_shape ="]
            .iter()
            .any(|k| line.trim_start().starts_with(k))
        {
            continue;
        }
        out.push_str(line);
        out.push('\n');
        if line.trim() == "[parameters]" {
            out.push_str("entry = \"lead\"\npass_shape = \"rings\"\n");
        }
    }
    std::fs::write(&path, out).unwrap();
    // No grip: the held region at the start of the stock goes.
    let path = dir.join(SETUP);
    let text = std::fs::read_to_string(&path).unwrap();
    let mut out = String::new();
    let mut skip = false;
    for line in text.lines() {
        if line.trim() == "[workholding.start]" {
            skip = true;
            continue;
        }
        if skip && line.trim_start().starts_with('[') {
            skip = false;
        }
        if !skip {
            out.push_str(line);
            out.push('\n');
        }
    }
    std::fs::write(&path, out).unwrap();
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
            std::fs::copy(entry.path(), &target).unwrap();
        }
    }
}

/// Checks that the moves of `kind` from `at` on hold the model X of
/// `x`, turn A one way only (the way `sign` gives, as the pass runs) and
/// turn it at all, and returns how many there are.
fn assert_in_the_plane_of_the_cut(
    segments: &[Segment],
    at: usize,
    kind: MoveKind,
    x: f64,
) -> usize {
    let moves: Vec<&Segment> = segments[at..]
        .iter()
        .take_while(|s| s.kind == kind)
        .collect();
    assert!(moves.len() > 2, "{kind:?}: {} moves", moves.len());
    for s in &moves {
        assert!(
            (s.end.tip.x - x).abs() < 1e-6,
            "{kind:?} at X {} not {x}",
            s.end.tip.x
        );
    }
    let turns: Vec<f64> = moves
        .windows(2)
        .map(|w| w[1].end.a.0 - w[0].end.a.0)
        .collect();
    let sign = turns
        .iter()
        .copied()
        .find(|d| d.abs() > 1e-9)
        .unwrap()
        .signum();
    assert!(
        turns.iter().all(|d| d * sign >= -1e-9),
        "{kind:?}: A turns one way"
    );
    moves.len()
}

#[test]
fn req_op_11_req_op_15_the_tutorial_piece_in_rings_leads_in_the_plane_of_the_cut() {
    let dir = example();
    let app = Application::open(&dir, Threads::Exactly(4)).unwrap();
    let run = app.regenerate().unwrap_or_else(|e| panic!("{e}"));
    let _ = std::fs::remove_dir_all(&dir);
    let segments: Vec<Segment> = run
        .toolpath
        .groups
        .iter()
        .flat_map(|g| g.segments.iter().copied())
        .collect();
    // The first lead-in, and the first cut after it.
    let first_in = segments
        .iter()
        .position(|s| s.kind == MoveKind::LeadIn)
        .unwrap();
    let cut = segments[first_in..]
        .iter()
        .find(|s| s.kind == MoveKind::Cutting)
        .unwrap();
    assert_in_the_plane_of_the_cut(&segments, first_in, MoveKind::LeadIn, cut.end.tip.x);
    // The first lead-out leaves the cut before it at that cut's X.
    let first_out = segments
        .iter()
        .position(|s| s.kind == MoveKind::LeadOut)
        .unwrap();
    assert_eq!(segments[first_out - 1].kind, MoveKind::Cutting);
    let x = segments[first_out - 1].end.tip.x;
    assert_in_the_plane_of_the_cut(&segments, first_out, MoveKind::LeadOut, x);
    // The end faces' free-end passes, which go down to the rotary axis,
    // are entered by a plunge and named in a note, not given a lead
    // (decision 0543 point 2, the owner's answer, relayed).
    assert!(
        run.notes.iter().any(|n| n.contains("Lead entry")
            && n.contains("on the rotary axis, where a lead arc has no plane to follow")),
        "{:?}",
        run.notes
    );
}
